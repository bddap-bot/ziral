mod look;
#[cfg(not(target_arch = "wasm32"))]
mod machines;
mod sim;

use bevy::asset::RenderAssetUsages;
use bevy::color::{Alpha, Mix};
use bevy::image::Image;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::math::Affine2;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::render::render_resource::TextureFormat;
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dPlugin};
use bevy::window::PrimaryWindow;
use look::{Finish, Glaze, HEX, Look, MANUAL, MachineMark, Shape, Skin, Token, px, skin};
use sim::{Arm, BondKind, DIRS, Glyph, GlyphKind, Hex, Instr, ORIGIN, Sim, Spin, Stall};

const TICK_MS: f32 = 400.0;
const MOTION: f32 = 1.0;
const MICRO_SCALE: f32 = 0.5;
const FOCUS: Hex = Hex::new(0, -1);
const MAX_GRID_CELLS: f32 = 6000.0;
const STRIP_ROWS: usize = 8;
const DRAG_PX: f32 = 6.0;
const LINE_PX: f32 = 3.0;
const SYMBOL_PX: f32 = 26.0;
const CURSOR_PX: f32 = 2.0;
const PALETTE_PX: f32 = 48.0;
const MARK_PX: f32 = 6.0;

fn brass(lift: f32) -> Color {
    Glaze::Brass.color().mix(&Glaze::Clay.color(), lift)
}

fn strip(lit: bool) -> Color {
    if lit { brass(0.3) } else { brass(0.0) }
}

const IVORY: Color = Glaze::Ivory.color();

#[derive(Clone, Copy, Debug, PartialEq, Eq, Component)]
pub enum Item {
    Arm,
    Glyph(GlyphKind),
}

pub const PALETTE: [Item; 6] = [
    Item::Arm,
    Item::Glyph(GlyphKind::Bonder),
    Item::Glyph(GlyphKind::SecondBond),
    Item::Glyph(GlyphKind::Source),
    Item::Glyph(GlyphKind::Output),
    Item::Glyph(GlyphKind::Cleanup),
];

#[derive(Clone, Copy)]
pub struct Key {
    code: KeyCode,
    instr: Instr,
    pub symbol: Skin,
    pub token: Token,
}

impl Key {
    const fn new(code: KeyCode, instr: Instr, symbol: Skin, face: Glaze, field: Glaze) -> Key {
        Key {
            code,
            instr,
            symbol,
            token: Token { face, field },
        }
    }

    fn shifted(&self) -> bool {
        matches!(self.instr, Instr::Move(_))
    }
}

const UPPER_LEFT: usize = 4;

pub const KEYS: [Key; 13] = [
    Key::new(
        KeyCode::KeyF,
        Instr::Grab,
        skin!("symbols/f"),
        Glaze::Terracotta,
        Glaze::Ivory,
    ),
    Key::new(
        KeyCode::KeyR,
        Instr::Drop,
        skin!("symbols/r"),
        Glaze::BlueGreen,
        Glaze::Ivory,
    ),
    Key::new(
        KeyCode::KeyA,
        Instr::Rot(Spin::Ccw),
        skin!("symbols/a"),
        Glaze::Amber,
        Glaze::Brass,
    ),
    Key::new(
        KeyCode::KeyD,
        Instr::Rot(Spin::Cw),
        skin!("symbols/d"),
        Glaze::Brass,
        Glaze::Amber,
    ),
    Key::new(
        KeyCode::KeyQ,
        Instr::Pivot(Spin::Ccw),
        skin!("symbols/q"),
        Glaze::Plum,
        Glaze::Ivory,
    ),
    Key::new(
        KeyCode::KeyE,
        Instr::Pivot(Spin::Cw),
        skin!("symbols/e"),
        Glaze::Ivory,
        Glaze::Plum,
    ),
    Key::new(
        KeyCode::KeyX,
        Instr::Wait,
        skin!("symbols/x"),
        Glaze::Ivory,
        Glaze::Brass,
    ),
    Key::new(
        KeyCode::KeyW,
        Instr::Move(UPPER_LEFT),
        skin!("symbols/shift-w"),
        Glaze::Amber,
        Glaze::Ivory,
    ),
    Key::new(
        KeyCode::KeyE,
        Instr::Move((UPPER_LEFT + 1) % 6),
        skin!("symbols/shift-e"),
        Glaze::BlueGreen,
        Glaze::Clay,
    ),
    Key::new(
        KeyCode::KeyF,
        Instr::Move((UPPER_LEFT + 2) % 6),
        skin!("symbols/shift-f"),
        Glaze::Plum,
        Glaze::Clay,
    ),
    Key::new(
        KeyCode::KeyC,
        Instr::Move((UPPER_LEFT + 3) % 6),
        skin!("symbols/shift-c"),
        Glaze::Ivory,
        Glaze::Amber,
    ),
    Key::new(
        KeyCode::KeyX,
        Instr::Move((UPPER_LEFT + 4) % 6),
        skin!("symbols/shift-x"),
        Glaze::Clay,
        Glaze::BlueGreen,
    ),
    Key::new(
        KeyCode::KeyA,
        Instr::Move((UPPER_LEFT + 5) % 6),
        skin!("symbols/shift-a"),
        Glaze::Clay,
        Glaze::Plum,
    ),
];

fn key_of(instr: Instr) -> Key {
    *KEYS
        .iter()
        .find(|k| k.instr == instr)
        .unwrap_or_else(|| panic!("no key writes {instr:?}"))
}

fn instr_of(key: KeyCode, shift: bool) -> Option<Instr> {
    KEYS.iter()
        .find(|k| k.code == key && k.shifted() == shift)
        .map(|k| k.instr)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Id {
    Arm(usize),
    Glyph(usize),
}

fn fresh(item: Item) -> Sim {
    let mut set = Sim::empty();
    match item {
        Item::Arm => set.arms.push(Arm::new(ORIGIN, 0, Vec::new())),
        Item::Glyph(kind) => set.glyphs.push(Some(Glyph {
            kind,
            at: ORIGIN,
            dir: 0,
        })),
    }
    set
}

fn runs(set: &Sim) -> bool {
    !set.arms.is_empty() || set.atoms.iter().any(Option::is_some)
}

fn turn(set: &mut Sim, spin: Spin) {
    for g in set.glyphs.iter_mut().flatten() {
        (g.at, g.dir) = (g.at.rotate(ORIGIN, spin), spin.turn(g.dir));
    }
    for a in &mut set.arms {
        (a.pivot, a.dir) = (a.pivot.rotate(ORIGIN, spin), spin.turn(a.dir));
    }
    for atom in set.atoms.iter_mut().flatten() {
        atom.pos = atom.pos.rotate(ORIGIN, spin);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Back {
    Nowhere,
    Ghost,
    Pick(Vec<Id>),
    Cell { cell: Hex, turns: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Focus {
    Pick(Vec<Id>),
    Tape { arm: usize, cursor: usize },
    Hold { set: Sim, back: Back },
}

impl Focus {
    fn picks(&self, id: Id) -> bool {
        match self {
            Focus::Pick(ids) => ids.contains(&id),
            Focus::Tape { arm, .. } => id == Id::Arm(*arm),
            Focus::Hold { .. } => false,
        }
    }

    fn picked(&self) -> Vec<Id> {
        match self {
            Focus::Pick(ids) => ids.clone(),
            Focus::Tape { arm, .. } => vec![Id::Arm(*arm)],
            Focus::Hold { .. } => Vec::new(),
        }
    }

    fn survive(mut self, sim: &Sim) -> Option<Focus> {
        let lost = |id: &Id| matches!(id, Id::Glyph(i) if sim.glyphs[*i].is_none());
        match &mut self {
            Focus::Pick(ids) => {
                ids.retain(|id| !lost(id));
                if ids.is_empty() {
                    return None;
                }
            }
            Focus::Hold {
                back: Back::Pick(ids),
                ..
            } if ids.iter().any(lost) => return None,
            _ => {}
        }
        Some(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Press {
    Atom { screen: Vec2, cell: Hex },
    Cell { screen: Vec2, cell: Hex },
    Ground { screen: Vec2, world: Vec2 },
    Marquee { from: Vec2 },
}

#[derive(Resource)]
struct World {
    sim: Sim,
    prev: Sim,
    ghost: Option<Sim>,
    since: f32,
    period: f32,
    motion: f32,
    running: bool,
    focus: Option<Focus>,
    down: Option<Press>,
    clipboard: Option<Sim>,
    pointer: Option<Vec2>,
}

impl World {
    fn new(sim: Sim) -> Self {
        World {
            prev: sim.clone(),
            sim,
            ghost: None,
            since: 0.0,
            period: TICK_MS / 1000.0,
            motion: MOTION,
            running: true,
            focus: None,
            down: None,
            clipboard: None,
            pointer: None,
        }
    }

    fn shown(&self) -> &Sim {
        self.ghost.as_ref().unwrap_or(&self.sim)
    }

    fn ghosts(&self) -> u64 {
        self.shown().tick - self.sim.tick
    }

    fn step(&mut self) {
        self.ghost = None;
        self.prev = self.sim.clone();
        self.sim.step();

        self.focus = self.focus.take().and_then(|f| f.survive(&self.sim));
    }

    fn resim(&mut self, n: u64) {
        self.ghost = (n > 0).then(|| self.sim.replay(n));
        self.prev = self.shown().clone();
    }

    fn editable(&self, runs: bool) -> bool {
        !runs || self.ghost.is_none()
    }

    fn phase(&self) -> f32 {
        let span = self.period * self.motion;
        if span > 0.0 { self.since / span } else { 1.0 }
    }

    fn focus_tape(&mut self, arm: usize) {
        let cursor = self.shown().arms[arm].tape.len();
        self.focus = Some(Focus::Tape { arm, cursor });
    }

    fn pick(&mut self, ids: Vec<Id>) {
        self.focus = (!ids.is_empty()).then_some(Focus::Pick(ids));
    }

    fn picks(&self, id: Id) -> bool {
        self.focus.as_ref().is_some_and(|f| f.picks(id))
    }

    fn glyph(&self, i: usize) -> Glyph {
        self.sim.glyphs[i].unwrap()
    }

    fn dir(&self, id: Id) -> usize {
        match id {
            Id::Arm(i) => self.shown().arms[i].dir,
            Id::Glyph(i) => self.glyph(i).dir,
        }
    }

    fn anchor(&self, id: Id) -> Hex {
        match id {
            Id::Arm(i) => self.shown().arms[i].pivot,
            Id::Glyph(i) => self.glyph(i).at,
        }
    }

    fn cells(&self, id: Id) -> Vec<Hex> {
        match id {
            Id::Arm(i) => self.shown().arms[i].cells().to_vec(),
            Id::Glyph(i) => self.glyph(i).slots().collect(),
        }
    }

    fn ids(&self) -> impl Iterator<Item = Id> + '_ {
        let sim = self.shown();
        let arms = (0..sim.arms.len()).map(Id::Arm);
        let glyphs = sim.glyphs.iter().enumerate().filter(|(_, g)| g.is_some());
        arms.chain(glyphs.map(|(i, _)| Id::Glyph(i)))
    }

    fn hit(&self, cell: Hex) -> Option<Id> {
        self.ids()
            .find(|id| self.anchor(*id) == cell)
            .or_else(|| self.ids().find(|id| self.cells(*id).contains(&cell)))
    }

    fn marquee(&self, a: Vec2, b: Vec2) -> Vec<Id> {
        let (lo, hi) = (a.min(b), a.max(b));
        let inside = |c: Hex| {
            let p = px(c);
            p.cmpge(lo).all() && p.cmple(hi).all()
        };
        self.ids()
            .filter(|id| self.cells(*id).into_iter().any(inside))
            .collect()
    }

    fn lifted(&self, ids: &[Id], grab: Hex) -> Sim {
        let mut set = Sim::empty();
        for id in ids {
            match *id {
                Id::Arm(i) => {
                    let a = &self.shown().arms[i];
                    set.arms
                        .push(Arm::new(a.pivot.sub(grab), a.dir, a.tape.clone()));
                }
                Id::Glyph(i) => {
                    let g = self.glyph(i);
                    set.glyphs.push(Some(Glyph {
                        at: g.at.sub(grab),
                        ..g
                    }));
                }
            }
        }
        set
    }

    fn set_pose(&mut self, id: Id, at: Hex, dir: usize) {
        match id {
            Id::Arm(i) => {
                (self.sim.arms[i].pivot, self.sim.arms[i].dir) = (at, dir);
                self.unstall();
            }
            Id::Glyph(i) => {
                if let Some(g) = &mut self.sim.glyphs[i] {
                    (g.at, g.dir) = (at, dir);
                }
            }
        }
    }

    fn unstall(&mut self) {
        for a in &mut self.sim.arms {
            a.stall = None;
        }
    }

    fn remove(&mut self, ids: &[Id]) {
        if !self.editable(ids.iter().any(|id| matches!(id, Id::Arm(_)))) {
            return;
        }
        let mut arms: Vec<usize> = ids
            .iter()
            .filter_map(|id| match id {
                Id::Arm(i) => Some(*i),
                Id::Glyph(_) => None,
            })
            .collect();
        arms.sort_unstable_by(|a, b| b.cmp(a));
        if !arms.is_empty() {
            self.unstall();
        }
        for i in arms {
            self.sim.arms.remove(i);
        }
        for id in ids {
            if let Id::Glyph(i) = id {
                self.sim.glyphs[*i] = None;
            }
        }
        self.focus = None;
        self.down = None;
        self.resim(self.ghosts());
    }

    fn copy(&mut self, ids: &[Id]) {
        self.clipboard = Some(self.lifted(ids, self.anchor(ids[0])));
    }

    fn paste(&mut self) {
        if let Some(set) = self.clipboard.clone() {
            self.lift(set, Back::Nowhere);
        }
    }

    fn lift(&mut self, set: Sim, back: Back) {
        if let Back::Pick(ids) = &back {
            let machines = set.glyphs.iter().flatten().count() + set.arms.len();
            debug_assert_eq!(ids.len(), machines);
        }
        self.focus = Some(Focus::Hold { set, back });
        self.down = None;
    }

    fn press(&mut self, screen: Vec2, point: Vec2) {
        let cell = hex_at(point);
        if matches!(self.focus, Some(Focus::Hold { .. })) {
            self.place(Some(cell));
            return;
        }
        let hit = self.hit(cell);
        let atom = self.shown().atom_at(cell).is_some();
        let picked = hit
            .is_some_and(|id| matches!(&self.focus, Some(Focus::Pick(ids)) if ids.contains(&id)));
        match hit {
            Some(_) if picked => {}
            Some(Id::Arm(arm)) => self.focus_tape(arm),
            Some(id) => self.pick(vec![id]),
            None if atom => self.focus = None,
            None => {}
        }
        self.down = Some(match hit {
            _ if atom && !picked => Press::Atom { screen, cell },
            Some(_) => Press::Cell { screen, cell },
            None => Press::Ground {
                screen,
                world: point,
            },
        });
    }

    fn drag(&mut self, screen: Vec2) {
        match self.down {
            Some(Press::Atom {
                screen: start,
                cell,
            }) if start.distance(screen) > DRAG_PX => {
                let Some(id) = self.shown().atom_at(cell) else {
                    self.down = None;
                    return;
                };
                let compound = self.shown().component(id);
                let set = self.shown().fragment(&compound, cell);
                let back = if self.editable(true) {
                    self.sim.consume(&compound);
                    self.resim(0);
                    Back::Cell { cell, turns: 0 }
                } else {
                    Back::Ghost
                };
                self.lift(set, back);
            }
            Some(Press::Cell {
                screen: start,
                cell,
            }) if start.distance(screen) > DRAG_PX => {
                let ids = self.focus.as_ref().map_or(Vec::new(), Focus::picked);
                if ids.is_empty() {
                    self.down = None;
                    return;
                }
                let set = self.lifted(&ids, cell);
                self.lift(set, Back::Pick(ids));
            }
            Some(Press::Ground {
                screen: start,
                world,
            }) if start.distance(screen) > DRAG_PX => {
                self.down = Some(Press::Marquee { from: world });
            }
            _ => {}
        }
    }

    fn release(&mut self, at: Option<Hex>) {
        match self.down.take() {
            Some(Press::Marquee { from }) => {
                let to = at.and(self.pointer);
                let ids = to.map_or(Vec::new(), |to| self.marquee(from, to));
                self.pick(ids);
            }
            Some(Press::Ground { .. }) => self.focus = None,
            _ => self.place(at),
        }
    }

    fn place(&mut self, at: Option<Hex>) {
        let Some(Focus::Hold { set, back }) =
            self.focus.take_if(|f| matches!(f, Focus::Hold { .. }))
        else {
            return;
        };
        let legal =
            |at: &Hex| back != Back::Ghost && self.editable(runs(&set)) && self.sim.fits(&set, *at);
        let Some(at) = at.filter(legal) else {
            self.pop(set, back);
            return;
        };
        let ids = match back {
            Back::Pick(ids) => {
                let arms = ids.iter().filter(|id| matches!(id, Id::Arm(_)));
                for (id, a) in arms.zip(&set.arms) {
                    self.set_pose(*id, at.add(a.pivot), a.dir);
                }
                let glyphs = ids.iter().filter(|id| matches!(id, Id::Glyph(_)));
                for (id, g) in glyphs.zip(set.glyphs.iter().flatten()) {
                    self.set_pose(*id, at.add(g.at), g.dir);
                }
                ids
            }
            Back::Nowhere | Back::Ghost | Back::Cell { .. } => {
                let arms = self.sim.arms.len()..self.sim.arms.len() + set.arms.len();
                let glyphs = self.sim.place(&set, at).into_iter().map(Id::Glyph);
                arms.map(Id::Arm).chain(glyphs).collect()
            }
        };
        self.pick(ids);
        self.resim(self.ghosts());
    }

    fn pop(&mut self, mut set: Sim, back: Back) {
        match back {
            Back::Nowhere | Back::Ghost => {}
            Back::Pick(ids) => self.pick(ids),
            Back::Cell { cell, turns } => {
                for _ in 0..turns {
                    turn(&mut set, Spin::Ccw);
                }
                if self.sim.fits(&set, cell) {
                    self.sim.place(&set, cell);
                    self.resim(self.ghosts());
                } else {
                    let back = Back::Cell { cell, turns: 0 };
                    self.focus = Some(Focus::Hold { set, back });
                }
            }
        }
    }

    fn key(&mut self, key: KeyCode, shift: bool) {
        use KeyCode::*;
        match key {
            Space => {
                self.running = !self.running;
                if self.running {
                    self.resim(0);
                }
                return;
            }
            KeyG => {
                if !self.running {
                    self.prev = self.shown().clone();
                    self.ghost = Some(self.prev.replay(1));
                    self.since = 0.0;
                }
                return;
            }
            KeyS => {
                if let (false, Some(n)) = (self.running, self.ghosts().checked_sub(1)) {
                    self.resim(n);
                }
                return;
            }
            _ => {}
        }
        let instr = instr_of(key, shift);
        match self.focus.clone() {
            Some(Focus::Hold { back, .. }) => match (key, instr, &mut self.focus) {
                (Escape, _, _) => self.place(None),
                (KeyZ, _, _) => match back {
                    Back::Nowhere | Back::Ghost => self.focus = None,
                    Back::Pick(ids) => self.remove(&ids),
                    Back::Cell { .. } => {}
                },
                (_, Some(Instr::Rot(spin)), Some(Focus::Hold { set, back })) => {
                    turn(set, spin);
                    if let Back::Cell { turns, .. } = back {
                        *turns = spin.turn(*turns);
                    }
                }
                _ => {}
            },
            Some(Focus::Pick(ids)) => match key {
                _ if shift => {}
                Escape => self.focus = None,
                KeyZ => self.remove(&ids),
                KeyX | KeyC if !self.editable(ids.iter().any(|id| matches!(id, Id::Arm(_)))) => {}
                KeyX => {
                    self.copy(&ids);
                    self.remove(&ids);
                }
                KeyC => self.copy(&ids),
                KeyV => self.paste(),
                _ => {
                    if let ([id], Some(Instr::Rot(spin))) = (ids.as_slice(), instr)
                        && self.editable(matches!(id, Id::Arm(_)))
                    {
                        self.set_pose(*id, self.anchor(*id), spin.turn(self.dir(*id)));
                        self.resim(self.ghosts());
                    }
                }
            },
            Some(Focus::Tape { arm, cursor }) => {
                if key == Escape {
                    self.focus = None;
                    return;
                }
                let tape = &mut self.sim.arms[arm].tape;
                let len = tape.len();
                let cursor = cursor.min(len);
                let cursor = match key {
                    ArrowLeft => cursor.saturating_sub(1),
                    ArrowRight => (cursor + 1).min(len),
                    Home => 0,
                    End => len,
                    KeyZ | Backspace if cursor > 0 => {
                        tape.remove(cursor - 1);
                        cursor - 1
                    }
                    _ => match instr {
                        Some(instr) => {
                            tape.insert(cursor, instr);
                            cursor + 1
                        }
                        None => cursor,
                    },
                };
                let edited = tape.len() != len;
                self.focus = Some(Focus::Tape { arm, cursor });
                if edited && self.ghost.is_some() {
                    self.resim(self.ghosts());
                }
            }
            None => {
                if key == KeyV {
                    self.paste();
                }
            }
        }
    }
}

fn hex_at(p: Vec2) -> Hex {
    let r = p.y / (HEX * 1.5);
    let q = p.x / (HEX * 3f32.sqrt()) - r / 2.0;
    let y = -q - r;
    let (mut rq, ry, mut rr) = (q.round(), y.round(), r.round());
    let (dq, dy, dr) = ((rq - q).abs(), (ry - y).abs(), (rr - r).abs());
    if dq > dy && dq > dr {
        rq = -ry - rr;
    } else if dr > dy {
        rr = -rq - ry;
    }
    Hex::new(rq as i32, rr as i32)
}

fn corners(center: Vec2, size: f32) -> [Vec2; 7] {
    std::array::from_fn(|k| {
        let a = (30.0 + 60.0 * k as f32).to_radians();
        center + size * Vec2::new(a.cos(), a.sin())
    })
}

struct Viewport {
    cam: Vec2,
    size: Vec2,
    scale: f32,
}

impl Viewport {
    fn of(window: &Window, transform: &Transform, projection: &Projection) -> Option<Viewport> {
        let Projection::Orthographic(ortho) = projection else {
            return None;
        };
        Some(Viewport {
            cam: transform.translation.truncate(),
            size: window.size(),
            scale: ortho.scale,
        })
    }

    fn half(&self) -> Vec2 {
        self.size * self.scale / 2.0
    }

    fn world(&self, screen: Vec2) -> Vec2 {
        self.cam
            + Vec2::new(screen.x - self.size.x / 2.0, self.size.y / 2.0 - screen.y) * self.scale
    }

    fn shows(&self, p: Vec2) -> bool {
        (p - self.cam).abs().cmplt(self.half()).all()
    }
}

fn app(world: World) -> App {
    let mut app = App::new();
    app.insert_resource(world)
        .insert_resource(ClearColor(brass(0.65)))
        .add_systems(Startup, (fire_kiln, spawn_ui).chain())
        .add_systems(
            Update,
            (run_ticks, view, edit, tapes, board, draw, manual).chain(),
        );
    app
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    #[cfg(not(target_arch = "wasm32"))]
    if machines::configure(&args) {
        return;
    }
    let mut app = match shot::parse(&args) {
        Some((world, shot)) => shot::app(world, shot),
        None => {
            let mut app = app(World::new(sim::preloaded()));
            app.add_plugins(DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "ziral".into(),
                    canvas: Some("#ziral".into()),
                    fit_canvas_to_parent: true,
                    ..default()
                }),
                ..default()
            }))
            .add_systems(Startup, spawn_camera);
            app
        }
    };
    lit_plugin(&mut app);
    app.run();
}

fn spawn_camera(mut commands: Commands) {
    let mut projection = OrthographicProjection::default_2d();
    projection.scale = MICRO_SCALE;
    commands.spawn((
        Camera2d,
        Projection::Orthographic(projection),
        Transform::from_translation(px(FOCUS).extend(0.0)),
    ));
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TapeLine {
    arm: usize,
    stalled: bool,
    tape: Vec<Instr>,
    pc: usize,
    cursor: Option<usize>,
}

#[derive(Component)]
struct TapeRow {
    slot: usize,
    line: Option<TapeLine>,
}

impl TapeRow {
    fn arm(&self) -> Option<usize> {
        self.line.as_ref().map(|l| l.arm)
    }
}

fn button(node: Node) -> impl Bundle {
    (
        Button,
        node,
        BorderColor::all(brass(0.5)),
        BackgroundColor(strip(false)),
    )
}
fn row(gap: f32) -> Node {
    Node {
        align_items: AlignItems::Center,
        column_gap: Val::Px(gap),
        ..default()
    }
}

fn symbol(kiln: &Kiln, skin: Skin, lit: bool) -> impl Bundle {
    (
        ImageNode::new(kiln.image(skin)),
        Node {
            width: Val::Px(SYMBOL_PX),
            height: Val::Px(SYMBOL_PX),
            ..default()
        },
        Outline {
            width: Val::Px(CURSOR_PX),
            offset: Val::ZERO,
            color: if lit { IVORY } else { Color::NONE },
        },
    )
}

#[derive(Component)]
struct Manual;

fn manual(keys: Res<ButtonInput<KeyCode>>, mut page: Single<&mut Node, With<Manual>>) {
    let display = if keys.pressed(KeyCode::Tab) {
        Display::Flex
    } else {
        Display::None
    };
    if page.display != display {
        page.display = display;
    }
}

#[derive(Component)]
struct Marks(u64);

fn mark(k: u64) -> impl Bundle {
    let gap = if k > 0 && k.is_multiple_of(5) {
        MARK_PX
    } else {
        0.0
    };
    (
        Node {
            width: Val::Px(MARK_PX),
            height: Val::Px(MARK_PX),
            margin: UiRect::left(Val::Px(gap)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(IVORY),
    )
}

fn cursor() -> impl Bundle {
    (
        Node {
            width: Val::Px(CURSOR_PX),
            height: Val::Px(SYMBOL_PX + 2.0 * CURSOR_PX),
            ..default()
        },
        BackgroundColor(IVORY),
    )
}

fn spawn_ui(mut commands: Commands, kiln: Res<Kiln>) {
    commands
        .spawn((
            Manual,
            Node {
                position_type: PositionType::Absolute,
                left: Val::ZERO,
                right: Val::ZERO,
                top: Val::ZERO,
                bottom: Val::ZERO,
                display: Display::None,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(1),
        ))
        .with_children(|page| {
            page.spawn((
                ImageNode::new(kiln.image(MANUAL)),
                Node {
                    width: Val::VMin(92.0),
                    height: Val::VMin(92.0),
                    ..default()
                },
            ));
        });
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(8.0),
            bottom: Val::Px(8.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        })
        .with_children(|col| {
            for item in PALETTE {
                col.spawn((
                    item,
                    button(Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    }),
                    children![(
                        ImageNode::new(kiln.image(look::machine(item).skin)),
                        Node {
                            width: Val::Px(PALETTE_PX),
                            height: Val::Px(PALETTE_PX),
                            ..default()
                        }
                    )],
                ));
            }
        });
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(PALETTE_PX + 34.0),
            right: Val::Px(8.0),
            bottom: Val::Px(8.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|col| {
            col.spawn((
                Marks(0),
                Node {
                    flex_wrap: FlexWrap::Wrap,
                    ..row(3.0)
                },
            ));
            for slot in 0..STRIP_ROWS {
                col.spawn((
                    TapeRow { slot, line: None },
                    button(Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..row(4.0)
                    }),
                    Visibility::Hidden,
                ));
            }
        });
}

fn run_ticks(mut world: ResMut<World>, time: Res<Time>) {
    world.since = (world.since + time.delta_secs()).min(world.period);
    if world.running && world.since >= world.period {
        world.since = 0.0;
        world.step();
    }
}

fn view(
    buttons: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    motion: Res<AccumulatedMouseMotion>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&mut Transform, &mut Projection), With<Camera2d>>,
) {
    let (mut transform, mut projection) = camera.into_inner();
    let Some(mut viewport) = Viewport::of(&window, &transform, &projection) else {
        return;
    };
    let Projection::Orthographic(ortho) = &mut *projection else {
        return;
    };
    if scroll.delta.y != 0.0
        && let Some(c) = window.cursor_position()
    {
        let before = viewport.world(c);
        let notches = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
        };
        ortho.scale = (ortho.scale * (-notches * 0.15).exp()).clamp(0.05, 40.0);
        viewport.scale = ortho.scale;
        transform.translation += (before - viewport.world(c)).extend(0.0);
    }
    if buttons.pressed(MouseButton::Middle) || buttons.pressed(MouseButton::Right) {
        let pan = Vec2::new(-motion.delta.x, motion.delta.y);
        transform.translation += (pan * ortho.scale).extend(0.0);
    }
}

fn edit(
    mut world: ResMut<World>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Transform, &Projection), With<Camera2d>>,
    palette: Query<(&Item, &Interaction), With<Button>>,
    rows: Query<(&TapeRow, &Interaction), With<Button>>,
) {
    let (transform, projection) = camera.into_inner();
    let Some(viewport) = Viewport::of(&window, transform, projection) else {
        return;
    };
    let screen = window.cursor_position();
    if let Some(c) = screen {
        world.pointer = Some(viewport.world(c));
    }
    let over_ui = keys.pressed(KeyCode::Tab)
        || palette.iter().any(|(_, i)| *i != Interaction::None)
        || rows.iter().any(|(_, i)| *i != Interaction::None);
    let at = world.pointer.map(hex_at);

    if buttons.just_pressed(MouseButton::Left) {
        if let Some((item, _)) = palette.iter().find(|(_, i)| **i == Interaction::Pressed) {
            world.lift(fresh(*item), Back::Nowhere);
        } else if let Some((row, _)) = rows.iter().find(|(_, i)| **i == Interaction::Pressed) {
            if let Some(arm) = row.arm().filter(|a| *a < world.shown().arms.len()) {
                world.focus_tape(arm);
            }
        } else if !over_ui && let (Some(c), Some(p)) = (screen, world.pointer) {
            world.press(c, p);
        }
    }
    if buttons.pressed(MouseButton::Left)
        && let Some(c) = screen
    {
        world.drag(c);
    }
    if buttons.just_released(MouseButton::Left) {
        let valid = !over_ui && screen.is_some();
        world.release(at.filter(|_| valid));
    }

    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let mut pressed: Vec<KeyCode> = keys.get_just_pressed().copied().collect();
    pressed.sort_unstable();
    for key in pressed {
        world.key(key, shift);
    }
}

fn tape_line(world: &World, i: usize) -> TapeLine {
    let arm = &world.shown().arms[i];
    let cursor = match &world.focus {
        Some(Focus::Tape { arm, cursor }) if *arm == i => Some(*cursor),
        _ => None,
    };
    TapeLine {
        arm: i,
        stalled: arm.stall.is_some(),
        tape: arm.tape.clone(),
        pc: if arm.tape.is_empty() {
            0
        } else {
            arm.pc % arm.tape.len()
        },
        cursor,
    }
}

fn tapes(
    mut commands: Commands,
    kiln: Res<Kiln>,
    world: Res<World>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Transform, &Projection), With<Camera2d>>,
    mut rows: Query<(Entity, &mut TapeRow, &mut Visibility, &mut BackgroundColor)>,
    marks: Single<(Entity, &mut Marks)>,
) {
    let (transform, projection) = camera.into_inner();
    let Some(viewport) = Viewport::of(&window, transform, projection) else {
        return;
    };
    let (entity, mut marks) = marks.into_inner();
    if marks.0 != world.ghosts() {
        marks.0 = world.ghosts();
        commands
            .entity(entity)
            .despawn_children()
            .with_children(|strip| {
                for k in 0..marks.0 {
                    strip.spawn(mark(k));
                }
            });
    }
    let listed: Vec<usize> = world
        .shown()
        .arms
        .iter()
        .enumerate()
        .filter(|(_, a)| viewport.shows(px(a.pivot)))
        .map(|(i, _)| i)
        .take(STRIP_ROWS)
        .collect();
    for (entity, mut row, mut vis, mut bg) in &mut rows {
        let Some(arm) = listed.get(row.slot).copied() else {
            *vis = Visibility::Hidden;
            row.line = None;
            continue;
        };
        *vis = Visibility::Inherited;
        bg.0 = strip(world.picks(Id::Arm(arm)));
        let line = tape_line(&world, arm);
        if row.line.as_ref() == Some(&line) {
            continue;
        }
        commands
            .entity(entity)
            .despawn_children()
            .with_children(|strip| {
                let stalled = if line.stalled { "!" } else { " " };
                strip.spawn((
                    Text::new(format!("{arm:<3}{stalled}")),
                    TextColor(IVORY),
                    TextFont::from_font_size(15.0),
                ));
                for (k, instr) in line.tape.iter().enumerate() {
                    if line.cursor == Some(k) {
                        strip.spawn(cursor());
                    }
                    strip.spawn(symbol(&kiln, key_of(*instr).symbol, k == line.pc));
                }
                if line.cursor.is_some_and(|c| c >= line.tape.len()) {
                    strip.spawn(cursor());
                }
            });
        row.line = Some(line);
    }
}

#[derive(Component)]
struct Fill;

#[derive(Component)]
struct Board;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tiling {
    Cells { r0: i32, r1: i32, x0: i32, x1: i32 },
    Slab(IVec2, IVec2),
}

const BOND_WIDTH: f32 = 0.14;

#[derive(Resource)]
struct Kiln {
    circle: Handle<Mesh>,
    hexagon: Handle<Mesh>,
    bar: Handle<Mesh>,
    bond: Handle<Mesh>,
    rim: Handle<Mesh>,
    tiled: Option<Tiling>,
    glaze: [Handle<ColorMaterial>; 7],
    patina: [Handle<ColorMaterial>; 2],
    skins: Vec<(Skin, Handle<Image>, [Handle<ColorMaterial>; 2])>,
    lit: Vec<(Skin, Handle<Lit>)>,
}

#[derive(Asset, TypePath, AsBindGroup, Clone)]
struct Lit {
    #[uniform(0)]
    light: Vec4,
    #[texture(1)]
    #[sampler(2)]
    albedo: Handle<Image>,
    #[texture(3)]
    #[sampler(4)]
    relief: Handle<Image>,
}

impl Material2d for Lit {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(
            bevy::asset::AssetPath::from_path_buf(bevy::asset::embedded_path!("lit.wgsl"))
                .with_source("embedded"),
        )
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }
}

fn lit_plugin(app: &mut App) {
    bevy::asset::embedded_asset!(app, "lit.wgsl");
    app.add_plugins(Material2dPlugin::<Lit>::default());
}

impl Kiln {
    fn material(&self, glaze: Glaze) -> &Handle<ColorMaterial> {
        &self.glaze[glaze as usize]
    }

    fn fired(&self, skin: Skin) -> &(Skin, Handle<Image>, [Handle<ColorMaterial>; 2]) {
        self.skins
            .iter()
            .find(|(s, _, _)| *s == skin)
            .unwrap_or_else(|| panic!("{skin:?} was never fired"))
    }

    fn skin(&self, skin: Skin, ghost: bool) -> &Handle<ColorMaterial> {
        &self.fired(skin).2[usize::from(ghost)]
    }

    fn image(&self, skin: Skin) -> Handle<Image> {
        self.fired(skin).1.clone()
    }

    fn lit(&self, skin: Skin) -> &Handle<Lit> {
        self.lit
            .iter()
            .find(|(s, _)| *s == skin)
            .map(|(_, lit)| lit)
            .unwrap_or_else(|| panic!("{skin:?} carries no relief"))
    }
}

fn to_linear(byte: u8) -> f32 {
    let c = f32::from(byte) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(linear: f32) -> u8 {
    let c = if linear <= 0.003_130_8 {
        linear * 12.92
    } else {
        1.055 * linear.powf(1.0 / 2.4) - 0.055
    };
    (c * 255.0).round() as u8
}

fn fire(mut image: Image, skin: Skin) -> Image {
    assert_eq!(
        image.texture_descriptor.format,
        TextureFormat::Rgba8UnormSrgb,
        "{skin:?} must decode to rgba8"
    );
    let relief = skin.finish == Finish::Relief;
    if relief {
        image.texture_descriptor.format = TextureFormat::Rgba8Unorm;
    }
    let linear: [f32; 256] = std::array::from_fn(|b| to_linear(b as u8));
    let (mut w, mut h) = (image.width() as usize, image.height() as usize);
    let mut data = image
        .data
        .take()
        .expect("a decoded image carries its pixels");
    let mut start = 0;
    let mut levels = 1;
    while w > 1 && h > 1 {
        let (nw, nh) = (w / 2, h / 2);
        let level = &data[start..start + w * h * 4];
        let mut next = Vec::with_capacity(nw * nh * 4);
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let at = |dx: usize, dy: usize| level[((2 * y + dy) * w + 2 * x + dx) * 4 + c];
                    let four = [at(0, 0), at(1, 0), at(0, 1), at(1, 1)];
                    next.push(if c == 3 || relief {
                        (four.iter().map(|a| u32::from(*a)).sum::<u32>() / 4) as u8
                    } else {
                        to_srgb(four.iter().map(|a| linear[usize::from(*a)]).sum::<f32>() / 4.0)
                    });
                }
            }
        }
        start += w * h * 4;
        data.extend_from_slice(&next);
        (w, h) = (nw, nh);
        levels += 1;
    }
    image.data = Some(data);
    image.texture_descriptor.mip_level_count = levels;
    image
}

fn fire_kiln(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut lits: ResMut<Assets<Lit>>,
    mut images: ResMut<Assets<Image>>,
    mut gizmo: ResMut<GizmoConfigStore>,
) {
    gizmo.config_mut::<DefaultGizmoConfigGroup>().0.line.width = LINE_PX;
    let grout = look::GROUT.decode();
    let skins: Vec<(Skin, Handle<Image>, [Handle<ColorMaterial>; 2])> = look::skins()
        .map(|skin| {
            let mut image = skin.decode();
            let crop = match skin.finish {
                Finish::Grouted => {
                    look::grout(&mut image, &grout);
                    *look::ring().end()
                }
                _ => 1.0,
            };
            let texture = images.add(fire(image, skin));
            let alpha_mode = if skin.finish == Finish::Sprite {
                AlphaMode2d::Blend
            } else {
                AlphaMode2d::Opaque
            };
            let uv_transform = Affine2::from_scale_angle_translation(
                Vec2::splat(crop),
                0.0,
                Vec2::splat(0.5 * (1.0 - crop)),
            );
            let fired = materials.add(ColorMaterial {
                color: Color::WHITE,
                alpha_mode,
                texture: Some(texture.clone()),
                uv_transform,
            });
            let ghost = materials.add(ColorMaterial {
                color: Color::WHITE.with_alpha(GHOST),
                alpha_mode: AlphaMode2d::Blend,
                texture: Some(texture.clone()),
                uv_transform,
            });
            (skin, texture, [fired, ghost])
        })
        .collect();
    let image = |skin: Skin| {
        skins
            .iter()
            .find(|(s, _, _)| *s == skin)
            .map(|(_, image, _)| image.clone())
            .unwrap_or_else(|| panic!("{skin:?} was never fired"))
    };
    let lit = PALETTE
        .into_iter()
        .map(|item| {
            let look = look::machine(item);
            let lit = lits.add(Lit {
                light: look::light().extend(look::AMBIENT),
                albedo: image(look.skin),
                relief: image(look.marking.normal()),
            });
            (look.skin, lit)
        })
        .collect();
    commands.insert_resource(Kiln {
        circle: meshes.add(Circle::new(1.0)),
        hexagon: meshes.add(RegularPolygon::new(1.0, 6)),
        bar: meshes.add(
            Rectangle::new(1.0, 1.0)
                .mesh()
                .build()
                .with_inserted_attribute(Mesh::ATTRIBUTE_TANGENT, vec![[1.0, 0.0, 0.0, 1.0]; 4]),
        ),
        bond: meshes.add(band(BOND_WIDTH / 3f32.sqrt())),
        rim: meshes.add(Annulus::new(0.85, 1.0)),
        tiled: None,
        glaze: Glaze::ALL.map(|g| materials.add(g.color())),
        patina: [0.5, 0.5 * GHOST].map(|a| materials.add(Glaze::Brass.color().with_alpha(a))),
        skins,
        lit,
    });
}

fn band(aspect: f32) -> Mesh {
    let v = aspect / 2.0;
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        vec![
            [-0.5, -0.5, 0.0],
            [0.5, -0.5, 0.0],
            [0.5, 0.5, 0.0],
            [-0.5, 0.5, 0.0],
        ],
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_UV_0,
        vec![
            [0.0, 0.5 + v],
            [1.0, 0.5 + v],
            [1.0, 0.5 - v],
            [0.0, 0.5 - v],
        ],
    )
    .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]))
}

fn unworn<M: std::fmt::Debug>(look: Look<M>) -> ! {
    panic!("nothing draws {look:?}")
}

struct Painter<'a, 'gw, 'gs, 'cw, 'cs> {
    gizmos: &'a mut Gizmos<'gw, 'gs>,
    commands: &'a mut Commands<'cw, 'cs>,
    kiln: &'a Kiln,
    ghost: bool,
}

impl<'a> Painter<'a, '_, '_, '_, '_> {
    fn line(&self, color: Color) -> Color {
        if self.ghost {
            color.with_alpha(GHOST)
        } else {
            color
        }
    }

    fn skin(&self, skin: Skin) -> &'a Handle<ColorMaterial> {
        self.kiln.skin(skin, self.ghost)
    }

    fn fill<M: Material2d>(
        &mut self,
        mesh: &Handle<Mesh>,
        material: &Handle<M>,
        at: Vec2,
        angle: f32,
        scale: Vec2,
        z: f32,
    ) {
        self.commands.spawn((
            Fill,
            Mesh2d(mesh.clone()),
            MeshMaterial2d(material.clone()),
            Transform {
                translation: at.extend(z),
                rotation: Quat::from_rotation_z(angle),
                scale: scale.extend(1.0),
            },
        ));
    }

    fn stamp(
        &mut self,
        mesh: &Handle<Mesh>,
        material: &Handle<ColorMaterial>,
        at: Vec2,
        r: f32,
        z: f32,
    ) {
        self.fill(mesh, material, at, 0.0, Vec2::splat(r), z);
    }

    fn bar(
        &mut self,
        mesh: &Handle<Mesh>,
        material: &Handle<ColorMaterial>,
        a: Vec2,
        b: Vec2,
        width: f32,
        z: f32,
    ) {
        let d = b - a;
        let scale = Vec2::new(d.length(), width);
        self.fill(mesh, material, (a + b) / 2.0, d.to_angle(), scale, z);
    }

    fn bead(&mut self, at: Vec2, look: Look<()>, z: f32) {
        let kiln = self.kiln;
        let (skin, patina) = (self.skin(look.skin), &kiln.patina[usize::from(self.ghost)]);
        self.stamp(&kiln.circle, skin, at, HEX * 0.4, z);
        self.stamp(&kiln.rim, patina, at, HEX * 0.4, z + layer::RIM);
    }

    fn bond(&mut self, a: Vec2, c: Vec2, kind: BondKind, z: f32) {
        let look = look::bond(kind);
        let Shape::Bars(n) = look.shape else {
            unworn(look)
        };
        let kiln = self.kiln;
        let material = self.skin(look.skin);
        let side = (c - a).perp().normalize_or_zero() * HEX * 0.16;
        for k in 0..n {
            let off = side * (2.0 * k as f32 - (n as f32 - 1.0));
            self.bar(&kiln.bond, material, a + off, c + off, HEX * BOND_WIDTH, z);
        }
    }

    fn horseshoe(&mut self, at: Vec2, r: f32, toward: Vec2, glaze: Glaze) {
        use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};
        let turn = toward.to_angle() - (FRAC_PI_2 + 3.0 * FRAC_PI_4);
        let iso = Isometry2d::new(at, Rot2::radians(turn));
        let color = self.line(glaze.color());
        self.gizmos.arc_2d(iso, 3.0 * FRAC_PI_2, r, color);
    }

    fn sprite(&mut self, item: Item, origin: Vec2, angle: f32, z: f32) {
        let quad = look::quad(item);
        let centre = origin + Vec2::from_angle(angle).rotate(quad.centre);
        let kiln = self.kiln;
        let skin = look::machine(item).skin;
        if self.ghost {
            self.fill(&kiln.bar, self.skin(skin), centre, angle, quad.size(), z);
        } else {
            self.fill(&kiln.bar, kiln.lit(skin), centre, angle, quad.size(), z);
        }
    }

    fn arm(&mut self, pivot: Vec2, hand: Vec2, ring: f32, look: Look<MachineMark>, z: f32) {
        self.sprite(Item::Arm, pivot, (hand - pivot).to_angle(), z);
        match look.marking {
            MachineMark::Hand(glaze, _) => self.horseshoe(hand, HEX * ring, pivot - hand, glaze),
            _ => unworn(look),
        };
    }

    fn machine(&mut self, item: Item, at: Hex, dir: usize, z: f32) {
        let look = look::machine(item);
        match (item, look.marking) {
            (Item::Arm, MachineMark::Hand(_, _)) => {
                let hand = px(at.add(DIRS[dir % 6]));
                self.arm(px(at), hand, RING_OPEN, look, z);
            }
            (Item::Glyph(_), MachineMark::Sprite(_)) => {
                self.sprite(item, px(at), look::turn(dir), z);
            }
            _ => unworn(look),
        }
    }
}

mod layer {
    use std::ops::Range;

    pub const GLYPHS: Range<f32> = 0.1..0.12;
    pub const BOND: f32 = 0.2;
    pub const ARMS: Range<f32> = 0.28..0.38;
    pub const BEAD: f32 = 0.4;
    pub const RIM: f32 = 0.02;
    pub const HELD: Range<f32> = 0.44..0.5;

    pub fn z(band: Range<f32>, i: usize, n: usize) -> f32 {
        band.start + (band.end - band.start) * (i as f32 / n as f32)
    }
}

const RING_CLOSED: f32 = 0.5;
const RING_OPEN: f32 = 0.9;
const GHOST: f32 = 0.45;

struct ArmPose {
    pivot: Vec2,
    hand: Vec2,
    ring: f32,
}

struct Frame<'a> {
    sim: &'a Sim,
    atoms: Vec<Option<Vec2>>,
    arms: Vec<ArmPose>,
}

fn sweep(centre: Vec2, spin: Spin, e: f32) -> impl Fn(Vec2) -> Vec2 {
    let angle = px(DIRS[0]).angle_to(px(DIRS[spin.turn(0)])) * e;
    move |v| centre + Vec2::from_angle(angle).rotate(v - centre)
}

impl Frame<'_> {
    fn settled(s: &Sim) -> Frame<'_> {
        Frame {
            sim: s,
            atoms: s.atoms.iter().map(|a| a.map(|a| px(a.pos))).collect(),
            arms: s
                .arms
                .iter()
                .map(|a| ArmPose {
                    pivot: px(a.pivot),
                    hand: px(a.hand()),
                    ring: grip(a.holding),
                })
                .collect(),
        }
    }

    fn between<'a>(prev: &'a Sim, cur: &'a Sim, t: f32) -> Frame<'a> {
        if t >= 1.0 {
            return Frame::settled(cur);
        }
        let mut frame = Frame::settled(prev);
        for (i, (a, b)) in prev.arms.iter().zip(&cur.arms).enumerate() {
            let e = Swing::from_cell(a.pivot).at(t);
            let pose = &mut frame.arms[i];
            pose.ring = grip(a.holding) + (grip(b.holding) - grip(a.holding)) * e;
            let (about, shift) = match (a.swung(b), px(b.pivot) - px(a.pivot)) {
                (Some((centre, spin)), _) => (Some((px(centre), spin)), Vec2::ZERO),
                (None, step) if step != Vec2::ZERO => (None, step * e),
                _ => continue,
            };
            let carried = |v: Vec2| about.map_or(v, |(c, spin)| sweep(c, spin, e)(v)) + shift;
            pose.pivot += shift;
            pose.hand = carried(pose.hand);
            if a.holding
                && let Some(id) = prev.atom_at(a.hand())
            {
                for id in prev.component(id) {
                    frame.atoms[id] = frame.atoms[id].map(&carried);
                }
            }
        }
        frame
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Swing {
    creep: f32,
    release: f32,
    run: f32,
    half_bounces: u32,
    decay: f32,
}

const SWING: Swing = Swing {
    creep: 0.15,
    release: 0.25,
    run: 0.25,
    half_bounces: 5,
    decay: 4.5,
};

const SPREAD: Swing = Swing {
    creep: 0.025,
    release: 0.05,
    run: 0.03,
    half_bounces: 1,
    decay: 1.5,
};

impl Swing {
    fn from_cell(cell: Hex) -> Swing {
        const STEPS: u32 = 65;
        const FEWEST_BOUNCES: u32 = SWING.half_bounces - SPREAD.half_bounces;
        fn draw(bits: &mut u32, steps: u32) -> u32 {
            let d = *bits % steps;
            *bits /= steps;
            d
        }
        let bits = &mut cell.scramble();
        let mut unit =
            |half: f32| half * (draw(bits, STEPS) as f32 / ((STEPS - 1) / 2) as f32 - 1.0);
        Swing {
            creep: SWING.creep + unit(SPREAD.creep),
            release: SWING.release + unit(SPREAD.release),
            run: SWING.run + unit(SPREAD.run),
            decay: SWING.decay + unit(SPREAD.decay),
            half_bounces: FEWEST_BOUNCES + draw(bits, 2 * SPREAD.half_bounces + 1),
        }
    }

    fn arrival(&self) -> f32 {
        self.release + self.run
    }

    fn at(&self, t: f32) -> f32 {
        if t >= 1.0 {
            return 1.0;
        }
        if t < self.release {
            let u = t / self.release;
            return self.creep * u * u * (3.0 - 2.0 * u);
        }
        if t < self.arrival() {
            return self.creep.lerp(1.0, (t - self.release) / self.run);
        }
        let speed = (1.0 - self.creep) / self.run;
        let omega = std::f32::consts::PI * self.half_bounces as f32 / (1.0 - self.arrival());
        let after = t - self.arrival();
        1.0 + speed / omega * (-self.decay * after).exp() * (omega * after).sin()
    }
}

fn grip(holding: bool) -> f32 {
    if holding { RING_CLOSED } else { RING_OPEN }
}

fn board(
    mut commands: Commands,
    mut kiln: ResMut<Kiln>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Transform, &Projection), With<Camera2d>>,
    laid: Query<Entity, With<Board>>,
) {
    let (transform, projection) = camera.into_inner();
    let Some(v) = Viewport::of(&window, transform, projection) else {
        return;
    };
    let col = HEX * 3f32.sqrt();
    let row = HEX * 1.5;
    let (lo, hi) = (v.cam - v.half(), v.cam + v.half());
    let span = hi - lo;
    let tiling = if (span.x / col) * (span.y / row) < MAX_GRID_CELLS {
        Tiling::Cells {
            r0: (lo.y / row).floor() as i32 - 1,
            r1: (hi.y / row).ceil() as i32 + 1,
            x0: (lo.x / col).floor() as i32,
            x1: (hi.x / col).ceil() as i32,
        }
    } else {
        Tiling::Slab(lo.floor().as_ivec2(), hi.ceil().as_ivec2())
    };
    if kiln.tiled == Some(tiling) {
        return;
    }
    for e in &laid {
        commands.entity(e).despawn();
    }
    let mut lay = |mesh: &Handle<Mesh>, material: &Handle<ColorMaterial>, transform: Transform| {
        commands.spawn((
            Board,
            Mesh2d(mesh.clone()),
            MeshMaterial2d(material.clone()),
            transform,
        ));
    };
    match tiling {
        Tiling::Cells { r0, r1, x0, x1 } => {
            for r in r0..=r1 {
                let (q0, q1) = (x0 - r.div_euclid(2) - 2, x1 - r.div_euclid(2) + 2);
                for q in q0..=q1 {
                    let h = Hex::new(q, r);
                    let tile = look::tile(h);
                    lay(
                        &kiln.hexagon,
                        kiln.skin(tile.skin, false),
                        Transform {
                            translation: px(h).extend(0.0),
                            rotation: Quat::IDENTITY,
                            scale: Vec3::splat(HEX),
                        },
                    );
                }
            }
        }
        Tiling::Slab(lo, hi) => {
            let (lo, hi) = (lo.as_vec2(), hi.as_vec2());
            lay(
                &kiln.bar,
                kiln.material(Glaze::Clay),
                Transform {
                    translation: ((lo + hi) / 2.0).extend(0.0),
                    scale: (hi - lo).extend(1.0),
                    ..default()
                },
            );
        }
    }
    kiln.tiled = Some(tiling);
}

fn draw(
    world: Res<World>,
    mut gizmos: Gizmos,
    mut commands: Commands,
    kiln: Res<Kiln>,
    fills: Query<Entity, With<Fill>>,
) {
    for e in &fills {
        commands.entity(e).despawn();
    }
    let mut p = Painter {
        gizmos: &mut gizmos,
        commands: &mut commands,
        kiln: &kiln,
        ghost: world.ghosts() > 0,
    };
    let f = Frame::between(&world.prev, world.shown(), world.phase());
    for (i, g) in f.sim.glyphs.iter().enumerate() {
        let Some(g) = g else { continue };
        let z = layer::z(layer::GLYPHS, i, f.sim.glyphs.len());
        p.machine(Item::Glyph(g.kind), g.at, g.dir, z);
    }
    for (i, g) in world.shown().glyphs.iter().enumerate() {
        if let Some(g) = g
            && world.picks(Id::Glyph(i))
        {
            p.gizmos.linestrip_2d(corners(px(g.at), HEX * 0.9), IVORY);
        }
    }
    for b in &f.sim.bonds {
        let (Some(a), Some(c)) = (f.atoms[b.a], f.atoms[b.b]) else {
            continue;
        };
        p.bond(a, c, b.kind, layer::BOND);
    }
    for (at, atom) in f.atoms.iter().zip(&f.sim.atoms) {
        if let (Some(at), Some(atom)) = (at, atom) {
            p.bead(*at, look::atom(atom.kind), layer::BEAD);
        }
    }
    let look = look::machine(Item::Arm);
    for (i, arm) in f.arms.iter().enumerate() {
        let z = layer::z(layer::ARMS, i, f.arms.len());
        p.arm(arm.pivot, arm.hand, arm.ring, look, z);
        if world.picks(Id::Arm(i)) {
            p.gizmos.linestrip_2d(corners(arm.pivot, HEX * 0.9), IVORY);
        }
        let stall = f.sim.arms[i].stall;
        let ivory = p.line(IVORY);
        if stall.is_some() {
            p.gizmos.circle_2d(arm.pivot, HEX * 0.5, ivory);
        }
        if let Some(Stall::Hand(j)) = stall {
            p.gizmos.circle_2d(f.arms[j].hand, HEX * 0.65, ivory);
        }
    }
    let Some(pointer) = world.pointer else { return };
    p.ghost = false;
    if let Some(Focus::Hold { set, .. }) = &world.focus {
        let grab = hex_at(pointer);
        p.gizmos.linestrip_2d(corners(px(grab), HEX * 0.9), IVORY);
        let glyphs = set.glyphs.iter().flatten();
        let machines: Vec<(Item, Hex, usize)> = glyphs
            .map(|g| (Item::Glyph(g.kind), g.at, g.dir))
            .chain(set.arms.iter().map(|a| (Item::Arm, a.pivot, a.dir)))
            .collect();
        for (i, (item, at, dir)) in machines.iter().enumerate() {
            let z = layer::z(layer::HELD, i, machines.len());
            p.machine(*item, grab.add(*at), *dir, z);
        }
        let at = |id: usize| px(grab.add(set.atoms[id].unwrap().pos));
        for b in &set.bonds {
            p.bond(at(b.a), at(b.b), b.kind, layer::z(layer::HELD, 0, 2));
        }
        for (id, atom) in set.atoms.iter().enumerate() {
            if let Some(atom) = atom {
                p.bead(at(id), look::atom(atom.kind), layer::z(layer::HELD, 1, 2));
            }
        }
        if set.atoms.iter().any(Option::is_some) {
            let look = look::machine(Item::Arm);
            let MachineMark::Hand(glaze, _) = look.marking else {
                unworn(look)
            };
            p.horseshoe(px(grab), HEX * RING_CLOSED, Vec2::Y, glaze);
        }
    }
    if let Some(Press::Marquee { from }) = world.down {
        let centre = Isometry2d::from_translation((from + pointer) / 2.0);
        p.gizmos.rect_2d(centre, (pointer - from).abs(), IVORY);
    }
}

#[cfg(target_arch = "wasm32")]
mod shot {
    pub enum Shot {}

    pub fn parse(_: &[String]) -> Option<(super::World, Shot)> {
        None
    }

    pub fn app(_: super::World, shot: Shot) -> super::App {
        match shot {}
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod shot {
    use super::*;
    use bevy::app::{AppExit, ScheduleRunnerPlugin};
    use bevy::camera::RenderTarget;
    use bevy::image::Image;
    use bevy::input::ButtonState;
    use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
    use bevy::render::RenderPlugin;
    use bevy::render::render_resource::{TextureFormat, TextureUsages};
    use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
    use bevy::time::TimeUpdateStrategy;
    use bevy::ui::IsDefaultUiCamera;
    use sim::{Atom, AtomKind, Bond};
    use std::path::PathBuf;
    use std::time::Duration;

    const WIDE_SCALE: f32 = 1.5;

    const WARM: u32 = 12;
    const FRAME: Duration = Duration::from_nanos(16_666_667);

    #[derive(Clone, Copy)]
    pub enum Act {
        Down(KeyCode),
        Up(KeyCode),
        Press(Hex),
        Drag(Hex),
        Release(Hex),
    }

    fn tap(frame: u32, key: KeyCode) -> [(u32, Act); 2] {
        [(frame, Act::Down(key)), (frame + 1, Act::Up(key))]
    }

    fn typed(keys: &[(KeyCode, bool)]) -> Vec<(u32, Act)> {
        let mut script = Vec::new();
        let mut frame = 2;
        for (key, shift) in keys {
            if *shift {
                script.push((frame, Act::Down(KeyCode::ShiftLeft)));
                frame += 1;
            }
            script.extend(tap(frame, *key));
            frame += 2;
            if *shift {
                script.push((frame, Act::Up(KeyCode::ShiftLeft)));
                frame += 1;
            }
        }
        script
    }

    #[derive(Resource)]
    pub struct Shot {
        path: PathBuf,
        clip: Option<u32>,
        wide: bool,
        script: Vec<(u32, Act)>,
        warm: u32,
        frames: u32,
    }

    #[derive(Resource)]
    struct Target(Handle<Image>);

    fn second_bond(extra: &[Hex]) -> (Sim, Vec<usize>) {
        let glyph = Glyph {
            kind: GlyphKind::SecondBond,
            at: Hex::new(1, -1),
            dir: 0,
        };
        let mut sim = Sim::empty();
        sim.glyphs.push(Some(glyph));
        let ids: Vec<usize> = extra
            .iter()
            .copied()
            .chain(glyph.slots())
            .map(|pos| {
                sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos,
                })
            })
            .collect();
        let bonded: Vec<usize> = glyph
            .slots()
            .skip(1)
            .map(|at| sim.atom_at(at).unwrap())
            .collect();
        sim.bonds.push(Bond {
            a: bonded[0],
            b: bonded[1],
            kind: BondKind::Single,
        });
        (sim, ids)
    }

    fn phased(copies: &[(Hex, u64)]) -> Sim {
        let mut world = Sim::empty();
        for (at, ticks) in copies {
            let mut one = sim::layout();
            for _ in 0..*ticks {
                one.step();
            }
            world.place(&one, *at);
        }
        world
    }

    pub fn scene(name: &str, ticks: u64) -> (World, bool, Vec<(u32, Act)>, u32) {
        use KeyCode::*;
        let mut world = World::new(sim::preloaded());
        world.running = false;
        world.pointer = Some(px(Hex::new(3, -3)));
        let mut keys = Vec::new();
        let mut script = Vec::new();
        let mut wide = false;
        match name {
            "micro" => world.focus_tape(0),
            "tab-held" => script.push((2, Act::Down(Tab))),
            "tab-released" => keys = vec![(Tab, false)],
            "texture-micro" => {
                let mut sim = Sim::empty();
                let mut arm = Arm::new(Hex::new(-1, 0), 0, Vec::new());
                arm.holding = true;
                sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: arm.hand(),
                });
                sim.arms.push(arm);
                sim.glyphs.push(Some(Glyph {
                    kind: GlyphKind::Bonder,
                    at: Hex::new(0, 0),
                    dir: 0,
                }));
                world.sim = sim;
                world.focus_tape(0);
            }
            "texture-wide" => {
                let mut sim = Sim::empty();
                for (k, pivot) in sim::PLACEMENTS.into_iter().enumerate() {
                    let dir = k % 6;
                    let mut arm = Arm::new(pivot, dir, Vec::new());
                    arm.holding = true;
                    sim.spawn(Atom {
                        kind: AtomKind::Base,
                        pos: arm.hand(),
                    });
                    sim.arms.push(arm);
                    sim.glyphs.push(Some(Glyph {
                        kind: GlyphKind::ALL[k % GlyphKind::ALL.len()],
                        at: pivot.add(DIRS[(dir + 2) % 6]),
                        dir,
                    }));
                }
                world.sim = sim;
                wide = true;
            }
            "wide" => wide = true,
            "board" => world.sim = Sim::empty(),
            "bonders" => world.sim = phased(&[(Hex::new(-3, 0), 14), (Hex::new(3, 0), 15)]),
            "focus" => {
                world
                    .sim
                    .arms
                    .push(Arm::new(Hex::new(3, -3), 0, Vec::new()));
                world.focus_tape(world.sim.arms.len() - 1);
                keys = KEYS.iter().map(|k| (k.code, k.shifted())).collect();
            }
            "write" => {
                let arm = Hex::new(-2, 0);
                world.sim = Sim::empty();
                world.sim.arms.push(Arm::new(arm, 0, Vec::new()));
                world.sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: arm.add(DIRS[0]),
                });
                script.extend(tap(30, Space));
                script.push((54, Act::Press(arm)));
                script.push((60, Act::Release(arm)));
                script.extend(tap(96, KeyF));
                script.extend(tap(132, KeyG));
            }
            "hand" => {
                let source = Hex::new(-4, 1);
                let mut sim = Sim::empty();
                let glyph = |kind, at, dir| Some(Glyph { kind, at, dir });
                sim.glyphs.push(glyph(GlyphKind::Source, source, 0));
                sim.glyphs
                    .push(glyph(GlyphKind::Bonder, Hex::new(-1, 1), 0));
                sim.glyphs
                    .push(glyph(GlyphKind::SecondBond, Hex::new(2, 0), 1));
                sim.glyphs
                    .push(glyph(GlyphKind::Output, Hex::new(-1, -3), 1));
                sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: source,
                });
                world.sim = sim;
                let carry = |f0: u32, path: &[(i32, i32)], turn: Option<usize>| {
                    let cell = |k: usize| Hex::new(path[k].0, path[k].1);
                    let mut acts = vec![(f0, Act::Press(cell(0)))];
                    for k in 1..path.len() {
                        acts.push((f0 + 6 * k as u32, Act::Drag(cell(k))));
                    }
                    let last = f0 + 6 * (path.len() as u32 - 1);
                    if let Some(k) = turn {
                        acts.extend(tap(f0 + 6 * k as u32 + 3, KeyD));
                    }
                    acts.push((last + 6, Act::Release(cell(path.len() - 1))));
                    acts
                };
                script.extend(carry(20, &[(-4, 1), (-3, 1), (-2, 1), (-1, 1)], None));
                script.extend(carry(
                    56,
                    &[(-4, 1), (-3, 1), (-2, 1), (-1, 1), (0, 1)],
                    None,
                ));
                script.extend(carry(116, &[(0, 1), (1, 0), (2, 0), (3, -1)], None));
                script.extend(carry(
                    150,
                    &[(-4, 1), (-3, 1), (-2, 1), (-1, 1), (0, 1), (1, 0), (2, 0)],
                    None,
                ));
                script.extend(carry(
                    212,
                    &[(2, -1), (1, -1), (0, -2), (-1, -2), (-1, -3)],
                    Some(2),
                ));
            }
            "walk" | "ghost" => {
                let mut sim = Sim::empty();
                let mut arm = Arm::new(
                    Hex::new(-4, 0),
                    0,
                    [0, 0, 0, 2, 2, 3, 4, 4].map(Instr::Move).to_vec(),
                );
                arm.holding = true;
                sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: arm.hand(),
                });
                sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: ORIGIN,
                });
                sim.arms.push(arm);
                sim.arms.push(Arm::new(
                    Hex::new(1, -1),
                    4,
                    vec![
                        Instr::Wait,
                        Instr::Wait,
                        Instr::Wait,
                        Instr::Grab,
                        Instr::Rot(Spin::Cw),
                        Instr::Wait,
                        Instr::Wait,
                        Instr::Rot(Spin::Ccw),
                        Instr::Drop,
                        Instr::Wait,
                        Instr::Wait,
                    ],
                ));
                world.sim = sim;
                world.focus_tape(0);
                if name == "ghost" {
                    script.extend(tap(60, Space));
                    for k in 0..5 {
                        script.extend(tap(84 + 24 * k, KeyG));
                    }
                    script.extend(tap(204, Home));
                    script.extend(tap(212, KeyD));
                    for k in 0..3 {
                        script.extend(tap(236 + 24 * k, KeyS));
                    }
                    script.extend(tap(308, Space));
                }
            }
            "hold" => {
                world.lift(fresh(Item::Glyph(GlyphKind::Bonder)), Back::Nowhere);
                keys = vec![(KeyD, false); 2];
            }
            "select" => {
                let walk = |f0: u32, cells: &[(i32, i32)], act: fn(Hex) -> Act| {
                    cells
                        .iter()
                        .enumerate()
                        .map(|(k, (q, r))| (f0 + 6 * k as u32, act(Hex::new(*q, *r))))
                        .collect::<Vec<_>>()
                };
                script = [(16, Act::Press(Hex::new(-3, -3)))].to_vec();
                script.extend(walk(
                    22,
                    &[(-2, -2), (-1, -1), (0, 0), (1, 1), (2, 2)],
                    Act::Drag,
                ));
                script.push((54, Act::Release(Hex::new(2, 2))));
                script.push((70, Act::Press(Hex::new(0, 0))));
                script.extend(walk(
                    76,
                    &[(1, -1), (2, -1), (3, -2), (4, -2), (5, -2)],
                    Act::Drag,
                ));
                script.extend(tap(114, KeyD));
                script.extend(tap(132, KeyD));
                script.push((152, Act::Release(Hex::new(5, -2))));
            }
            "output" => {
                let mut sim = Sim::empty();
                for (k, kind) in [BondKind::Single, BondKind::Double, BondKind::Double]
                    .into_iter()
                    .enumerate()
                {
                    let at = Hex::new(k as i32 * 3 - 4, -1);
                    sim.glyphs.push(Some(Glyph {
                        kind: GlyphKind::Output,
                        at,
                        dir: if k == 2 { 3 } else { 0 },
                    }));
                    let a = sim.spawn(Atom {
                        kind: AtomKind::Base,
                        pos: at,
                    });
                    let b = sim.spawn(Atom {
                        kind: AtomKind::Base,
                        pos: at.add(DIRS[0]),
                    });
                    sim.bonds.push(Bond { a, b, kind });
                }
                world.sim = sim;
            }
            "bonding" => {
                let (mut sim, _) = second_bond(&[]);
                sim.arms
                    .push(Arm::new(Hex::new(1, 0), 1, vec![Instr::Grab, Instr::Wait]));
                world.sim = sim;
                world.focus_tape(0);
            }
            "chorus" => {
                let mut sim = Sim::empty();
                for q in [-8, -4, 0, 4, 8] {
                    let mut arm = Arm::new(Hex::new(q, -1), 2, vec![Instr::Rot(Spin::Cw)]);
                    arm.holding = true;
                    sim.spawn(Atom {
                        kind: AtomKind::Base,
                        pos: arm.hand(),
                    });
                    sim.arms.push(arm);
                }
                world.sim = sim;
            }
            "machines" => {
                let mut sim = Sim::empty();
                for (k, item) in PALETTE.into_iter().enumerate() {
                    let at = Hex::new(3 * k as i32 - 7, -1);
                    match item {
                        Item::Arm => sim.arms.push(Arm::new(at, 0, Vec::new())),
                        Item::Glyph(kind) => sim.glyphs.push(Some(Glyph { kind, at, dir: 0 })),
                    }
                }
                world.sim = sim;
            }
            "rotation" => {
                let mut sim = Sim::empty();
                for (q, dir) in [(-4, 0), (4, 2)] {
                    let pivot = Hex::new(q, 0);
                    let mut arm = Arm::new(pivot, dir, Vec::new());
                    arm.holding = true;
                    sim.spawn(Atom {
                        kind: AtomKind::Base,
                        pos: arm.hand(),
                    });
                    sim.arms.push(arm);
                    let glyph = Glyph {
                        kind: GlyphKind::Bonder,
                        at: Hex::new(q, 2),
                        dir,
                    };
                    sim.glyphs.push(Some(glyph));
                    let ends: Vec<usize> = glyph
                        .slots()
                        .map(|pos| {
                            sim.spawn(Atom {
                                kind: AtomKind::Base,
                                pos,
                            })
                        })
                        .collect();
                    sim.bonds.push(Bond {
                        a: ends[0],
                        b: ends[1],
                        kind: BondKind::Single,
                    });
                }
                world.sim = sim;
            }
            "cleanup" => {
                let mut sim = Sim::empty();
                let mut arm = Arm::new(
                    Hex::new(0, 0),
                    0,
                    vec![Instr::Rot(Spin::Cw), Instr::Drop, Instr::Wait],
                );
                arm.holding = true;
                sim.glyphs.push(Some(Glyph {
                    kind: GlyphKind::Cleanup,
                    at: arm.hand().rotate(arm.pivot, Spin::Cw),
                    dir: 0,
                }));
                let chain: Vec<usize> = [DIRS[1], ORIGIN, DIRS[4]]
                    .iter()
                    .map(|off| {
                        sim.spawn(Atom {
                            kind: AtomKind::Base,
                            pos: arm.hand().add(*off),
                        })
                    })
                    .collect();
                for pair in chain.windows(2) {
                    sim.bonds.push(Bond {
                        a: pair[0],
                        b: pair[1],
                        kind: BondKind::Single,
                    });
                }
                sim.arms.push(arm);
                world.sim = sim;
            }
            "pivot" => {
                let mut sim = Sim::empty();
                let mut arm = Arm::new(
                    Hex::new(0, 0),
                    0,
                    vec![
                        Instr::Rot(Spin::Ccw),
                        Instr::Pivot(Spin::Cw),
                        Instr::Pivot(Spin::Cw),
                        Instr::Rot(Spin::Cw),
                    ],
                );
                arm.holding = true;
                let a = sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: arm.hand(),
                });
                let b = sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: arm.hand().add(DIRS[0]),
                });
                sim.bonds.push(Bond {
                    a,
                    b,
                    kind: BondKind::Single,
                });
                sim.arms.push(arm);
                world.sim = sim;
            }
            "twohands" => {
                let mut sim = Sim::empty();
                sim.arms.push(Arm::new(
                    Hex::new(0, 0),
                    0,
                    vec![Instr::Grab, Instr::Rot(Spin::Cw)],
                ));
                sim.arms
                    .push(Arm::new(Hex::new(2, -2), 4, vec![Instr::Grab, Instr::Wait]));
                let a = sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: Hex::new(1, 0),
                });
                let b = sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: Hex::new(1, -1),
                });
                sim.bonds.push(Bond {
                    a,
                    b,
                    kind: BondKind::Single,
                });
                world.sim = sim;
                world.focus_tape(0);
            }
            "heldeat" => {
                let (mut sim, _) = second_bond(&[Hex::new(0, 0)]);
                let mut eaten = Arm::new(
                    Hex::new(0, -1),
                    0,
                    vec![
                        Instr::Wait,
                        Instr::Wait,
                        Instr::Wait,
                        Instr::Rot(Spin::Ccw),
                        Instr::Wait,
                    ],
                );
                eaten.holding = true;
                sim.arms.push(eaten);
                sim.arms.push(Arm::new(
                    Hex::new(1, 0),
                    3,
                    vec![Instr::Grab, Instr::Rot(Spin::Ccw), Instr::Drop, Instr::Wait],
                ));
                world.sim = sim;
                world.focus_tape(0);
            }
            "heldout" => {
                let mut sim = Sim::empty();
                sim.glyphs.push(Some(Glyph {
                    kind: GlyphKind::Output,
                    at: Hex::new(1, -1),
                    dir: 0,
                }));
                let a = sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: Hex::new(1, -1),
                });
                let b = sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: Hex::new(2, -1),
                });
                sim.bonds.push(Bond {
                    a,
                    b,
                    kind: BondKind::Double,
                });
                let mut arm = Arm::new(Hex::new(0, -1), 0, vec![Instr::Wait]);
                arm.holding = true;
                sim.arms.push(arm);
                world.sim = sim;
                world.focus_tape(0);
            }
            "caught" => {
                let mut sim = Sim::empty();
                sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: Hex::new(3, -1),
                });
                let mut closed = Arm::new(Hex::new(0, 0), 0, vec![Instr::Wait]);
                closed.holding = true;
                sim.arms.push(closed);
                sim.arms.push(Arm::new(
                    Hex::new(2, 0),
                    1,
                    vec![
                        Instr::Grab,
                        Instr::Rot(Spin::Cw),
                        Instr::Rot(Spin::Cw),
                        Instr::Rot(Spin::Cw),
                    ],
                ));
                world.sim = sim;
                world.focus_tape(1);
            }
            "grabnothing" => {
                let mut sim = Sim::empty();
                sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: Hex::new(2, -1),
                });
                sim.arms.push(Arm::new(
                    Hex::new(0, 0),
                    0,
                    vec![Instr::Grab, Instr::Wait, Instr::Rot(Spin::Cw), Instr::Wait],
                ));
                sim.arms.push(Arm::new(
                    Hex::new(2, 0),
                    2,
                    vec![Instr::Grab, Instr::Rot(Spin::Cw), Instr::Drop, Instr::Wait],
                ));
                world.sim = sim;
                world.focus_tape(0);
            }
            "dropfirst" | "grabfirst" => {
                let mut sim = Sim::empty();
                sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: Hex::new(1, -1),
                });
                let dropper = Arm::new(
                    Hex::new(1, 0),
                    2,
                    vec![Instr::Grab, Instr::Drop, Instr::Wait],
                );
                let grabber = Arm::new(
                    Hex::new(1, -2),
                    5,
                    vec![Instr::Wait, Instr::Grab, Instr::Wait],
                );
                let dropper_first = name == "dropfirst";
                sim.arms = if dropper_first {
                    vec![dropper, grabber]
                } else {
                    vec![grabber, dropper]
                };
                world.sim = sim;
                world.focus_tape(usize::from(dropper_first));
            }
            other => panic!("unknown scene {other}"),
        }
        world.sim = world.sim.replay(ticks);
        world.prev = world.sim.clone();
        let typed = typed(&keys);
        let warm = typed
            .last()
            .map_or(WARM, |(frame, _)| (frame + 2).max(WARM));
        script.extend(typed);
        (world, wide, script, warm)
    }

    pub fn parse(args: &[String]) -> Option<(World, Shot)> {
        const USAGE: &str = "usage: ziral --shot <png> <scene> <ticks> | ziral --shot <dir> <scene> <ticks> <play> <tick_ms> <motion>";
        let num = |s: &String| s.parse::<f32>().expect(USAGE);
        let (path, view, ticks, clip) = match args {
            [_] => return None,
            [_, flag, path, view, ticks] if flag == "--shot" => (path, view, ticks, None),
            [_, flag, path, view, ticks, play, tick_ms, motion] if flag == "--shot" => (
                path,
                view,
                ticks,
                Some((num(play), num(tick_ms), num(motion))),
            ),
            _ => panic!("{USAGE}"),
        };
        let (mut world, wide, script, warm) = scene(view, ticks.parse().expect(USAGE));
        let clip = clip.map(|(play, tick_ms, motion)| {
            world.period = tick_ms / 1000.0;
            world.motion = motion;
            (play * world.period / FRAME.as_secs_f32()).round() as u32
        });
        let shot = Shot {
            path: PathBuf::from(path),
            clip,
            wide,
            script,
            warm,
            frames: 0,
        };
        Some((world, shot))
    }

    #[cfg(test)]
    pub fn still(view: &str, dir: PathBuf, frames: u32) -> App {
        let (mut world, wide, script, warm) = scene(view, 0);
        world.period = f32::INFINITY;
        let shot = Shot {
            path: dir,
            clip: Some(frames),
            wide,
            script,
            warm,
            frames: 0,
        };
        app(world, shot)
    }

    pub fn app(world: World, shot: Shot) -> App {
        let mut app = super::app(world);
        if shot.clip.is_some() {
            app.insert_resource(TimeUpdateStrategy::ManualDuration(FRAME));
        }
        app.insert_resource(shot)
            .add_plugins(
                DefaultPlugins
                    .set(RenderPlugin {
                        synchronous_pipeline_compilation: true,
                        ..default()
                    })
                    .set(WindowPlugin {
                        primary_window: Some(Window {
                            resolution: (1280, 720).into(),
                            ..default()
                        }),
                        exit_condition: bevy::window::ExitCondition::DontExit,
                        ..default()
                    })
                    .disable::<bevy::winit::WinitPlugin>(),
            )
            .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::ZERO))
            .add_systems(Startup, spawn_offscreen_camera)
            .add_systems(Update, capture.after(run_ticks).before(draw));
        app
    }

    fn spawn_offscreen_camera(
        mut commands: Commands,
        mut images: ResMut<Assets<Image>>,
        shot: Res<Shot>,
    ) {
        let mut image = Image::new_target_texture(1280, 720, TextureFormat::Rgba8UnormSrgb, None);
        image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let handle = images.add(image);
        let mut projection = OrthographicProjection::default_2d();
        let center = if shot.wide {
            projection.scale = WIDE_SCALE;
            let pivots: Vec<Vec2> = sim::PLACEMENTS.iter().map(|h| px(*h)).collect();
            pivots.iter().sum::<Vec2>() / pivots.len() as f32
        } else {
            projection.scale = MICRO_SCALE;
            px(FOCUS)
        };
        commands.spawn((
            Camera2d,
            Projection::Orthographic(projection),
            Transform::from_translation(center.extend(0.0)),
            RenderTarget::Image(handle.clone().into()),
            IsDefaultUiCamera,
        ));
        commands.insert_resource(Target(handle));
    }

    fn key(key_code: KeyCode, state: ButtonState, window: Entity) -> KeyboardInput {
        KeyboardInput {
            key_code,
            logical_key: Key::Unidentified(NativeKey::Unidentified),
            state,
            text: None,
            repeat: false,
            window,
        }
    }

    fn capture(
        mut commands: Commands,
        mut shot: ResMut<Shot>,
        mut world: ResMut<World>,
        target: Res<Target>,
        window: Single<Entity, With<PrimaryWindow>>,
        mut keyboard: MessageWriter<KeyboardInput>,
        mut exit: MessageWriter<AppExit>,
    ) {
        shot.frames += 1;
        for (frame, act) in shot.script.clone() {
            if frame != shot.frames {
                continue;
            }
            match act {
                Act::Down(code) => {
                    keyboard.write(key(code, ButtonState::Pressed, *window));
                }
                Act::Up(code) => {
                    keyboard.write(key(code, ButtonState::Released, *window));
                }
                Act::Press(cell) => {
                    world.pointer = Some(px(cell));
                    world.press(px(cell), px(cell));
                }
                Act::Drag(cell) => {
                    world.pointer = Some(px(cell));
                    world.drag(px(cell));
                }
                Act::Release(cell) => {
                    world.pointer = Some(px(cell));
                    world.release(Some(cell));
                }
            }
        }
        let warm = shot.warm;
        if shot.frames == warm && shot.clip.is_some() {
            world.running = true;
            world.since = 0.0;
        }
        let n = shot.frames.wrapping_sub(warm);
        let count = shot.clip.unwrap_or(1);
        if n < count {
            let path = match shot.clip {
                Some(_) => shot.path.join(format!("{n:05}.png")),
                None => shot.path.clone(),
            };
            commands
                .spawn(Screenshot::image(target.0.clone()))
                .observe(save_to_disk(path));
        }
        if n == count + 28 {
            exit.write(AppExit::Success);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::{Atom, AtomKind, Bond};

    const SEAM_TONE: f32 = 0.05;
    const SEAM_GRAIN: f32 = 0.015;
    const BLUR_PX: f32 = 1.0;

    fn still_frames(view: &str, n: u32) -> Vec<image::RgbaImage> {
        let dir = std::env::temp_dir().join(format!("ziral-{view}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = shot::still(view, dir.clone(), n);
        lit_plugin(&mut app);
        let exit = app.run();
        let frames: Vec<_> = (0..n)
            .map(|k| std::fs::read(dir.join(format!("{k:05}.png"))))
            .collect();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(exit, bevy::app::AppExit::Success);
        frames
            .into_iter()
            .map(|png| image::load_from_memory(&png.unwrap()).unwrap().into_rgba8())
            .collect()
    }

    #[test]
    fn two_frames_of_a_still_scene_with_overlapping_arms_are_pixel_identical() {
        let frames = still_frames("wide", 2);
        let (a, b) = (&frames[0], &frames[1]);
        let changed = a
            .enumerate_pixels()
            .zip(b.pixels())
            .filter(|((_, _, p), q)| p != q)
            .map(|((x, y, _), _)| (x, y))
            .collect::<Vec<_>>();
        assert!(
            changed.is_empty(),
            "{} pixels differ between two frames of a still scene, first at {:?}",
            changed.len(),
            changed[0]
        );
    }

    struct Seam {
        cells: [Hex; 2],
        samples: usize,
        mean: [f32; 3],
        grain: f32,
    }

    fn seams(frame: &image::RgbaImage) -> Vec<Seam> {
        let centre = Vec2::new(frame.width() as f32, frame.height() as f32) / 2.0;
        let to_frame = |w: Vec2| centre + (w - px(FOCUS)) * Vec2::new(1.0, -1.0) / MICRO_SCALE;
        let ring = look::ring();
        let half_grout =
            HEX * 3f32.sqrt() / 2.0 * (ring.end() - ring.start()) / MICRO_SCALE - BLUR_PX;
        let half_edge = HEX / 2.0 / MICRO_SCALE - 2.0 * BLUR_PX;
        let reach = half_grout.max(half_edge).ceil();
        let inside = |p: Vec2| {
            p.x >= 2.0 * PALETTE_PX + reach
                && p.x < frame.width() as f32 - reach
                && p.y >= reach
                && p.y < frame.height() as f32 - reach
        };
        let mut out = Vec::new();
        for q in -40..40 {
            for r in -20..20 {
                let h = Hex::new(q, r);
                for dir in &DIRS[..3] {
                    let n = h.add(*dir);
                    let mid = to_frame((px(h) + px(n)) / 2.0);
                    if !inside(mid) {
                        continue;
                    }
                    let across = (to_frame(px(n)) - to_frame(px(h))).normalize();
                    let along = across.perp();
                    let (x0, y0) = ((mid.x - reach) as u32, (mid.y - reach) as u32);
                    let shades: Vec<[f32; 3]> = (y0..y0 + 2 * reach as u32)
                        .flat_map(|y| (x0..x0 + 2 * reach as u32).map(move |x| (x, y)))
                        .filter(|(x, y)| {
                            let v = Vec2::new(*x as f32 + 0.5, *y as f32 + 0.5) - mid;
                            v.dot(across).abs() <= half_grout && v.dot(along).abs() <= half_edge
                        })
                        .map(|(x, y)| [0, 1, 2].map(|c| f32::from(frame[(x, y)].0[c]) / 255.0))
                        .collect();
                    let mean = [0, 1, 2]
                        .map(|c| shades.iter().map(|s| s[c]).sum::<f32>() / shades.len() as f32);
                    let shade = |s: &[f32; 3]| s.iter().sum::<f32>() / 3.0;
                    let m = shade(&mean);
                    let grain = (shades.iter().map(|s| (shade(s) - m).powi(2)).sum::<f32>()
                        / shades.len() as f32)
                        .sqrt();
                    out.push(Seam {
                        cells: [h, n],
                        samples: shades.len(),
                        mean,
                        grain,
                    });
                }
            }
        }
        out
    }

    #[test]
    fn every_seam_between_tiles_is_grout() {
        let frame = &still_frames("board", 1)[0];
        let seams = seams(frame);
        assert!(seams.len() > 500, "only {} seams in view", seams.len());
        let thin = seams.iter().min_by_key(|s| s.samples).unwrap();
        assert!(
            thin.samples as f32 >= HEX / MICRO_SCALE,
            "only {} pixels sampled between {:?}",
            thin.samples,
            thin.cells
        );
        let expected = look::tests::grout_color().to_srgba();
        let expected = [expected.red, expected.green, expected.blue];
        let name = |s: &Seam| s.cells.map(|h| (h, look::tile(h).skin));
        let tone = |s: &Seam| {
            (0..3)
                .map(|c| (s.mean[c] - expected[c]).abs())
                .fold(0.0, f32::max)
        };
        let off = seams
            .iter()
            .max_by(|a, b| tone(a).total_cmp(&tone(b)))
            .unwrap();
        assert!(
            tone(off) <= SEAM_TONE,
            "the seam between {:?} is {:?}, {:.3} from grout {expected:?}",
            name(off),
            off.mean,
            tone(off)
        );
        let flat = seams
            .iter()
            .min_by(|a, b| a.grain.total_cmp(&b.grain))
            .unwrap();
        assert!(
            flat.grain >= SEAM_GRAIN,
            "the seam between {:?} is a solid colour: {:.3} grain",
            name(flat),
            flat.grain
        );
    }

    fn rotating_arm_with_atom() -> (Sim, Sim) {
        let mut prev = Sim::empty();
        prev.arms
            .push(Arm::new(Hex::new(2, 1), 2, vec![Instr::Rot(Spin::Cw)]));
        prev.arms[0].holding = true;
        prev.spawn(Atom {
            kind: AtomKind::Base,
            pos: prev.arms[0].hand(),
        });
        let mut cur = prev.clone();
        cur.step();
        (prev, cur)
    }

    #[test]
    fn mid_rotate_the_hand_and_its_atom_sit_where_the_swing_says() {
        let (prev, cur) = rotating_arm_with_atom();
        let reach = |arm: &Arm| px(arm.hand()) - px(arm.pivot);
        let (start, end) = (reach(&prev.arms[0]), reach(&cur.arms[0]));
        let swing = Swing::from_cell(prev.arms[0].pivot);
        for t in [0.4, 0.55] {
            let f = Frame::between(&prev, &cur, t);
            let expected = Vec2::from_angle(start.angle_to(end) * swing.at(t)).rotate(start);
            assert!((f.arms[0].hand - f.arms[0].pivot).abs_diff_eq(expected, 1e-3));
            assert!(
                f.atoms[0]
                    .unwrap()
                    .abs_diff_eq(f.arms[0].pivot + expected, 1e-3)
            );
            assert_eq!(f.arms[0].ring, RING_CLOSED);
        }
    }

    #[test]
    fn between_ticks_only_the_turn_an_arm_just_ran_sweeps() {
        let mut prev = Sim::empty();
        prev.arms.push(Arm::new(
            Hex::new(0, 0),
            0,
            vec![Instr::Pivot(Spin::Cw), Instr::Wait],
        ));
        prev.arms
            .push(Arm::new(Hex::new(4, 0), 0, vec![Instr::Rot(Spin::Cw)]));
        prev.arms.push(Arm::new(Hex::new(8, 0), 0, Vec::new()));
        for arm in &mut prev.arms {
            arm.holding = true;
            prev.atoms.push(Some(Atom {
                kind: AtomKind::Base,
                pos: arm.hand(),
            }));
        }
        prev.spawn(Atom {
            kind: AtomKind::Base,
            pos: Hex::new(5, -1),
        });
        let far = prev.spawn(Atom {
            kind: AtomKind::Base,
            pos: Hex::new(2, -1),
        });
        prev.bonds.push(sim::Bond {
            a: 0,
            b: far,
            kind: BondKind::Single,
        });
        prev.step();
        let mut cur = prev.clone();
        cur.step();
        assert_eq!(prev.arms[0].pc, 1);
        assert_eq!(cur.arms[1].stall, Some(Stall::Illegal));
        let f = Frame::between(&prev, &cur, 0.5);
        let still = Frame::settled(&prev);
        assert_eq!(f.atoms, still.atoms);
        for (a, b) in f.arms.iter().zip(&still.arms) {
            assert_eq!(a.hand, b.hand);
        }
    }

    #[test]
    fn mid_pivot_the_far_atom_sweeps_about_the_still_hand() {
        let mut prev = Sim::empty();
        prev.arms
            .push(Arm::new(Hex::new(2, 1), 2, vec![Instr::Pivot(Spin::Cw)]));
        prev.arms[0].holding = true;
        let hand = prev.arms[0].hand();
        prev.spawn(Atom {
            kind: AtomKind::Base,
            pos: hand,
        });
        prev.spawn(Atom {
            kind: AtomKind::Base,
            pos: hand.add(DIRS[3]),
        });
        prev.bonds.push(sim::Bond {
            a: 0,
            b: 1,
            kind: BondKind::Single,
        });
        let mut cur = prev.clone();
        cur.step();
        let (start, end) = (px(DIRS[3]), px(DIRS[4]));
        let swing = Swing::from_cell(prev.arms[0].pivot);
        for t in [0.4, 0.55] {
            let f = Frame::between(&prev, &cur, t);
            let expected = Vec2::from_angle(start.angle_to(end) * swing.at(t)).rotate(start);
            assert_eq!(f.arms[0].hand, px(hand));
            assert_eq!(f.atoms[0], Some(px(hand)));
            assert!(f.atoms[1].unwrap().abs_diff_eq(px(hand) + expected, 1e-3));
            assert!(!f.atoms[1].unwrap().abs_diff_eq(px(hand) + start, 1e-3));
        }
    }

    #[test]
    fn what_a_glyph_makes_appears_only_when_the_transition_ends() {
        let mut prev = Sim::empty();
        prev.glyphs.push(Some(Glyph {
            kind: GlyphKind::Source,
            at: Hex::new(0, 0),
            dir: 0,
        }));
        let mut cur = prev.clone();
        cur.step();
        assert!(Frame::between(&prev, &cur, 0.99).atoms.is_empty());
        assert_eq!(
            Frame::between(&prev, &cur, 1.0).atoms,
            vec![Some(px(Hex::new(0, 0)))]
        );
    }

    #[test]
    fn a_grab_closes_the_ring_over_the_transition() {
        let mut prev = Sim::empty();
        prev.arms
            .push(Arm::new(Hex::new(0, 0), 0, vec![Instr::Grab]));
        prev.spawn(Atom {
            kind: AtomKind::Base,
            pos: Hex::new(1, 0),
        });
        let mut cur = prev.clone();
        cur.step();
        let swing = Swing::from_cell(Hex::new(0, 0));
        let ring = |t| Frame::between(&prev, &cur, t).arms[0].ring;
        assert_eq!(ring(0.0), RING_OPEN);
        let creeping = swing.release / 2.0;
        assert!(ring(creeping) > RING_OPEN + (RING_CLOSED - RING_OPEN) * creeping);
        let running = swing.release + swing.run / 2.0;
        assert!(ring(running) < RING_OPEN && ring(running) > RING_CLOSED);
        let clenched = (1..100)
            .map(|i| ring(i as f32 / 100.0))
            .fold(f32::MAX, f32::min);
        assert!(clenched < RING_CLOSED);
        assert_eq!(ring(1.0), RING_CLOSED);
    }

    const OVERSHOOT_LEAST: f32 = 1.04;
    const OVERSHOOT_MOST: f32 = 1.16;
    const VISIBLE_BOUNCE: f32 = 1.01;

    fn patch() -> Vec<Hex> {
        (-4..4)
            .flat_map(|q| (-4..4).map(move |r| Hex::new(q, r)))
            .collect()
    }

    fn family() -> Vec<Swing> {
        let mut all = vec![SWING];
        all.extend(patch().into_iter().map(Swing::from_cell));
        all
    }

    #[test]
    fn a_cell_keeps_its_swing_and_a_patch_has_no_two_alike() {
        assert_eq!(
            Swing::from_cell(Hex::new(2, -3)),
            Swing {
                creep: 0.1640625,
                release: 0.2734375,
                run: 0.2509375,
                half_bounces: 6,
                decay: 3.515625,
            }
        );
        let swings: Vec<Swing> = patch().into_iter().map(Swing::from_cell).collect();
        for (i, a) in swings.iter().enumerate() {
            for b in &swings[i + 1..] {
                assert_ne!(a, b);
            }
        }
        let span = |f: fn(&Swing) -> f32| {
            let v: Vec<f32> = swings.iter().map(f).collect();
            v.iter().cloned().fold(f32::MIN, f32::max) - v.iter().cloned().fold(f32::MAX, f32::min)
        };
        assert!(span(|s| s.creep) > SPREAD.creep);
        assert!(span(|s| s.release) > SPREAD.release);
        assert!(span(|s| s.run) > SPREAD.run);
        assert!(span(|s| s.decay) > SPREAD.decay);
        assert_eq!(
            span(|s| s.half_bounces as f32),
            2.0 * SPREAD.half_bounces as f32
        );
    }

    #[test]
    fn every_swing_creeps_lets_go_overshoots_and_settles_exactly() {
        for swing in family() {
            assert_eq!(swing.at(0.0), 0.0);
            assert_eq!(swing.at(1.0), 1.0);
            assert_eq!(swing.at(1.23), 1.0);
            let samples: Vec<f32> = (1..1000).map(|i| swing.at(i as f32 / 1000.0)).collect();
            let released = (swing.release * 1000.0) as usize;
            for (i, s) in samples.iter().enumerate().take(released) {
                assert!(
                    *s > 0.0 && *s < (i + 1) as f32 / 1000.0,
                    "{swing:?} creep at {i}: {s}"
                );
            }
            let peak = samples.iter().cloned().fold(0.0, f32::max);
            assert!(
                peak > OVERSHOOT_LEAST && peak < OVERSHOOT_MOST,
                "{swing:?} overshoot {peak}"
            );
            let first_crossing = samples.iter().position(|s| *s >= 1.0).unwrap();
            let sign_changes = samples[first_crossing..]
                .windows(2)
                .filter(|w| (w[0] - 1.0).signum() != (w[1] - 1.0).signum())
                .count();
            assert!(sign_changes >= 2, "{swing:?} {sign_changes} sign changes");
            let bounces = samples[first_crossing..]
                .windows(3)
                .filter(|w| w[1] > w[0] && w[1] > w[2] && w[1] > VISIBLE_BOUNCE)
                .count();
            assert!(bounces >= 2, "{swing:?} {bounces} visible bounces");
        }
    }

    #[test]
    fn the_centre_swing_holds_its_golden_shape() {
        let golden = [
            0.02344, 0.12656, 0.3625, 0.7875, 1.07547, 0.98219, 0.98985, 1.01396,
        ];
        for (i, g) in golden.iter().enumerate() {
            let t = (2 * i + 1) as f32 / 16.0;
            assert!(
                (SWING.at(t) - g).abs() < 1e-4,
                "swing({t}) = {} not {g}",
                SWING.at(t)
            );
        }
    }

    fn held_dir(w: &World) -> usize {
        match &w.focus {
            Some(Focus::Hold { set, .. }) => set.arms[0].dir,
            other => panic!("not holding: {other:?}"),
        }
    }

    fn lone(glyphs: Vec<Glyph>, arms: Vec<Arm>) -> World {
        let mut sim = Sim::empty();
        sim.glyphs = glyphs.into_iter().map(Some).collect();
        sim.arms = arms;
        World::new(sim)
    }

    fn drag(w: &mut World, from: Hex, to: Hex) {
        w.press(px(from), px(from));
        w.drag(px(from) + Vec2::new(DRAG_PX * 2.0, 0.0));
        w.pointer = Some(px(to));
        w.release(Some(to));
    }

    fn bonder(at: Hex, dir: usize) -> Glyph {
        Glyph {
            kind: GlyphKind::Bonder,
            at,
            dir,
        }
    }

    fn picked(ids: &[Id]) -> Option<Focus> {
        Some(Focus::Pick(ids.to_vec()))
    }

    #[test]
    fn a_drag_from_a_non_anchor_slot_moves_the_glyph_and_keeps_that_slot_under_the_cursor() {
        let bonder = bonder(Hex::new(1, 1), 2);
        let mut w = lone(vec![bonder], vec![]);
        let slot = bonder.slots().nth(1).unwrap();
        assert_ne!(slot, bonder.at);
        let to = Hex::new(-4, 3);
        drag(&mut w, slot, to);
        let moved = w.sim.glyphs[0].unwrap();
        assert_eq!(moved.slots().nth(1), Some(to));
        assert_eq!(moved.at, to.add(bonder.at.sub(slot)));
        assert_eq!(w.focus, picked(&[Id::Glyph(0)]));
    }

    #[test]
    fn a_drag_from_an_empty_cell_moves_nothing() {
        let bonder = bonder(ORIGIN, 0);
        let arm = Arm::new(Hex::new(4, 0), 0, vec![]);
        let mut w = lone(vec![bonder], vec![arm.clone()]);
        drag(&mut w, Hex::new(0, 3), Hex::new(-4, 4));
        assert_eq!(w.sim.glyphs, vec![Some(bonder)]);
        assert_eq!(w.sim.arms, vec![arm]);
        assert!(w.focus.is_none() && w.down.is_none());
    }

    #[test]
    fn an_arm_is_grabbed_by_its_hand_cell_and_that_cell_lands_under_the_cursor() {
        let arm = Arm::new(Hex::new(2, -1), 3, vec![]);
        let mut w = lone(vec![], vec![arm.clone()]);
        let to = Hex::new(0, 5);
        drag(&mut w, arm.hand(), to);
        assert_eq!(w.sim.arms[0].hand(), to);
        assert_eq!(w.sim.arms[0].pivot, to.add(arm.pivot.sub(arm.hand())));
    }

    #[test]
    fn turning_while_held_keeps_the_grabbed_slot_under_the_cursor() {
        let bonder = bonder(Hex::new(3, -2), 4);
        let mut w = lone(vec![bonder], vec![]);
        let slot = bonder.slots().nth(1).unwrap();
        w.press(px(slot), px(slot));
        w.drag(px(slot) + Vec2::new(DRAG_PX * 2.0, 0.0));
        w.key(KeyCode::KeyA, false);
        w.key(KeyCode::KeyA, false);
        w.key(KeyCode::KeyD, false);
        let to = Hex::new(-1, -1);
        w.release(Some(to));
        let moved = w.sim.glyphs[0].unwrap();
        assert_eq!(moved.dir, 3);
        assert_eq!(moved.slots().nth(1), Some(to));
    }

    #[test]
    fn an_anchor_under_the_cursor_wins_over_a_body_cell_and_stacked_anchors_go_to_the_first_listed()
    {
        let source = Glyph {
            kind: GlyphKind::Source,
            at: Hex::new(1, 0),
            dir: 0,
        };
        let bonder = bonder(ORIGIN, 0);
        let arm = Arm::new(ORIGIN, 0, vec![]);
        assert!(bonder.slots().any(|s| s == source.at));
        assert_eq!(arm.hand(), source.at);
        assert_eq!(
            lone(vec![bonder, source], vec![arm.clone()]).hit(source.at),
            Some(Id::Glyph(1))
        );
        assert_eq!(
            lone(vec![bonder, source], vec![arm]).hit(ORIGIN),
            Some(Id::Arm(0))
        );
        assert_eq!(
            lone(vec![source, source], vec![]).hit(source.at),
            Some(Id::Glyph(0))
        );
    }

    #[test]
    fn a_click_on_a_body_cell_focuses_the_machine_without_moving_it() {
        let bonder = bonder(Hex::new(2, 2), 1);
        let mut w = lone(vec![bonder], vec![]);
        let slot = bonder.slots().nth(1).unwrap();
        w.press(px(slot), px(slot));
        w.drag(px(slot) + Vec2::new(DRAG_PX / 2.0, 0.0));
        w.release(Some(slot));
        assert_eq!(w.focus, picked(&[Id::Glyph(0)]));
        assert_eq!(w.sim.glyphs, vec![Some(bonder)]);
    }

    fn cluster() -> World {
        let source = Glyph {
            kind: GlyphKind::Source,
            at: Hex::new(0, 3),
            dir: 0,
        };
        lone(
            vec![bonder(ORIGIN, 0), source],
            vec![
                Arm::new(Hex::new(3, 0), 3, vec![Instr::Grab, Instr::Rot(Spin::Cw)]),
                Arm::new(Hex::new(-3, 0), 0, vec![]),
            ],
        )
    }

    const CORNER_A: Hex = Hex::new(-1, -1);
    const CORNER_B: Hex = Hex::new(2, 1);
    const INSIDE: [Id; 2] = [Id::Arm(0), Id::Glyph(0)];

    #[test]
    fn the_marquee_picks_every_machine_with_a_body_cell_inside_and_none_outside() {
        let w = cluster();
        assert_eq!(w.sim.arms[0].hand(), Hex::new(2, 0));
        assert_eq!(w.sim.arms[1].hand(), Hex::new(-2, 0));
        assert_eq!(w.marquee(px(CORNER_A), px(CORNER_B)), INSIDE);
        assert_eq!(w.marquee(px(CORNER_B), px(CORNER_A)), INSIDE);
    }

    #[test]
    fn a_drag_from_empty_ground_selects_and_a_click_on_empty_ground_clears() {
        let mut w = cluster();
        let before = w.sim.clone();
        drag(&mut w, CORNER_A, CORNER_B);
        assert_eq!(w.focus, picked(&INSIDE));
        assert_eq!(w.sim, before);
        w.press(px(CORNER_A), px(CORNER_A));
        w.release(Some(CORNER_A));
        assert_eq!(w.focus, None);
        assert_eq!(w.sim, before);
    }

    #[test]
    fn holding_a_selected_machine_lifts_the_set_and_letting_go_moves_it_by_the_cursor_delta() {
        let mut w = cluster();
        w.pick(INSIDE.to_vec());
        let before = w.sim.clone();
        let grab = Hex::new(2, 0);
        let to = Hex::new(-2, 4);
        drag(&mut w, grab, to);
        let delta = to.sub(grab);
        assert_eq!(w.sim.arms[0].pivot, before.arms[0].pivot.add(delta));
        assert_eq!(w.sim.arms[0].dir, before.arms[0].dir);
        assert_eq!(w.sim.arms[0].tape, before.arms[0].tape);
        assert_eq!(
            w.sim.glyphs[0].unwrap().at,
            before.glyphs[0].unwrap().at.add(delta)
        );
        assert_eq!(w.sim.arms[1], before.arms[1]);
        assert_eq!(w.sim.glyphs[1], before.glyphs[1]);
        assert_eq!(w.focus, picked(&INSIDE));
    }

    #[test]
    fn a_press_on_an_unselected_arm_focuses_it_alone_and_a_press_on_the_ground_starts_a_marquee() {
        let mut w = cluster();
        w.pick(INSIDE.to_vec());
        let lone_arm = w.sim.arms[1].pivot;
        w.press(px(lone_arm), px(lone_arm));
        assert_eq!(w.focus, Some(Focus::Tape { arm: 1, cursor: 0 }));
        w.drag(px(lone_arm) + Vec2::new(DRAG_PX * 2.0, 0.0));
        assert!(
            matches!(&w.focus, Some(Focus::Hold { set, back: Back::Pick(from) }) if set.arms.len() == 1 && *from == [Id::Arm(1)])
        );
        w.release(None);
        w.pick(INSIDE.to_vec());
        w.press(px(CORNER_A), px(CORNER_A));
        w.drag(px(CORNER_A) + Vec2::new(DRAG_PX * 2.0, 0.0));
        assert!(matches!(w.down, Some(Press::Marquee { .. })));
        assert!(!matches!(w.focus, Some(Focus::Hold { .. })));
    }

    fn offsets(w: &World, ids: &[Id]) -> Vec<(Hex, usize)> {
        ids.iter().map(|id| (w.anchor(*id), w.dir(*id))).collect()
    }

    #[test]
    fn a_rotated_array_keeps_every_pairwise_offset_up_to_rotation_and_advances_every_orientation() {
        let mut w = cluster();
        w.pick(INSIDE.to_vec());
        let before = offsets(&w, &INSIDE);
        let grab = ORIGIN;
        w.press(px(grab), px(grab));
        w.drag(px(grab) + Vec2::new(DRAG_PX * 2.0, 0.0));
        w.key(KeyCode::KeyD, false);
        w.key(KeyCode::KeyD, false);
        let to = Hex::new(4, -3);
        w.release(Some(to));
        let after = offsets(&w, &INSIDE);
        for (i, (at_i, dir_i)) in before.iter().enumerate() {
            assert_eq!(after[i].1, (dir_i + 2) % 6);
            assert_eq!(after[i].0, to.add(at_i.sub(grab).turned(2)));
            for (j, (at_j, _)) in before.iter().enumerate() {
                assert_eq!(after[j].0.sub(after[i].0), at_j.sub(*at_i).turned(2));
            }
        }
        assert_eq!(w.focus, picked(&INSIDE));
    }

    #[test]
    fn escape_while_holding_puts_the_set_back_untouched() {
        let mut w = cluster();
        w.pointer = Some(px(Hex::new(5, 5)));
        w.pick(INSIDE.to_vec());
        let before = w.sim.clone();
        w.press(px(ORIGIN), px(ORIGIN));
        w.drag(px(ORIGIN) + Vec2::new(DRAG_PX * 2.0, 0.0));
        w.key(KeyCode::KeyD, false);
        w.key(KeyCode::Escape, false);
        assert_eq!(w.sim, before);
        assert_eq!(w.focus, picked(&INSIDE));
    }

    #[test]
    fn cut_removes_the_selection_and_paste_reproduces_it_under_the_cursor() {
        let mut w = cluster();
        w.pick(INSIDE.to_vec());
        let before = offsets(&w, &INSIDE);
        let (arm, glyph) = (w.sim.arms[0].clone(), w.sim.glyphs[0]);
        let (other_arm, other_glyph) = (w.sim.arms[1].clone(), w.sim.glyphs[1]);
        w.key(KeyCode::KeyX, false);
        assert_eq!(w.sim.arms, vec![other_arm.clone()]);
        assert_eq!(w.sim.glyphs, vec![None, other_glyph]);
        assert_eq!(w.focus, None);
        assert!(w.clipboard.is_some());
        w.key(KeyCode::KeyV, false);
        assert!(matches!(
            &w.focus,
            Some(Focus::Hold { set, back: Back::Nowhere }) if set.arms.len() == 1 && set.glyphs.len() == 1
        ));
        let to = Hex::new(5, 5);
        w.press(px(to), px(to));
        assert_eq!(w.sim.arms.len(), 2);
        assert_eq!(w.sim.glyphs.len(), 2);
        let pasted = [Id::Arm(1), Id::Glyph(0)];
        let after = offsets(&w, &pasted);
        let shift = to.sub(before[0].0);
        for ((at, dir), (was, was_dir)) in after.iter().zip(&before) {
            assert_eq!(*at, was.add(shift));
            assert_eq!(dir, was_dir);
        }
        assert_eq!(w.sim.arms[1].tape, arm.tape);
        assert_eq!(w.sim.arms[1], Arm::new(after[0].0, after[0].1, arm.tape));
        assert_eq!(w.sim.glyphs[0].unwrap().kind, glyph.unwrap().kind);
        assert_eq!(w.focus, picked(&pasted));
        w.key(KeyCode::KeyC, false);
        assert!(w.clipboard.is_some());
        assert_eq!(w.sim.arms.len(), 2);
    }

    fn armed(tape: Vec<Instr>) -> World {
        let mut w = lone(vec![], vec![Arm::new(ORIGIN, 0, tape)]);
        w.sim.spawn(Atom {
            kind: AtomKind::Base,
            pos: DIRS[0],
        });
        w
    }

    #[test]
    fn a_click_on_an_arm_focuses_its_tape_with_the_cursor_at_the_end_and_a_reclick_puts_it_there_again()
     {
        let mut w = armed(vec![Instr::Wait, Instr::Wait]);
        w.press(px(ORIGIN), px(ORIGIN));
        w.release(Some(ORIGIN));
        assert_eq!(w.focus, Some(Focus::Tape { arm: 0, cursor: 2 }));
        w.key(KeyCode::Home, false);
        w.press(px(ORIGIN), px(ORIGIN));
        w.release(Some(ORIGIN));
        assert_eq!(w.focus, Some(Focus::Tape { arm: 0, cursor: 2 }));
    }

    #[test]
    fn f_and_d_on_a_clicked_fresh_arm_write_grab_and_rotate_that_run_on_the_steps_and_not_before() {
        let mut w = armed(vec![]);
        w.running = false;
        w.press(px(ORIGIN), px(ORIGIN));
        w.release(Some(ORIGIN));
        let before = w.sim.clone();
        w.key(KeyCode::KeyF, false);
        w.key(KeyCode::KeyD, false);
        assert_eq!(w.sim.arms[0].tape, vec![Instr::Grab, Instr::Rot(Spin::Cw)]);
        assert_eq!(w.focus, Some(Focus::Tape { arm: 0, cursor: 2 }));
        assert!(!w.sim.arms[0].holding);
        assert_eq!(w.sim.arms[0].dir, before.arms[0].dir);
        assert_eq!(w.sim.atoms, before.atoms);
        assert_eq!(w.ghost, None);
        w.key(KeyCode::KeyG, false);
        assert!(w.shown().arms[0].holding);
        assert_eq!(w.shown().atoms[0].unwrap().pos, DIRS[0]);
        w.key(KeyCode::KeyG, false);
        assert_eq!(w.shown().arms[0].dir, 1);
        assert_eq!(w.shown().atoms[0].unwrap().pos, DIRS[1]);
        assert_eq!(w.focus, Some(Focus::Tape { arm: 0, cursor: 2 }));
    }

    #[test]
    fn z_deletes_the_focused_machine() {
        let mut w = armed(vec![]);
        w.sim.glyphs.push(Some(Glyph {
            kind: GlyphKind::Bonder,
            at: Hex::new(3, 3),
            dir: 0,
        }));
        w.pick(vec![Id::Glyph(0)]);
        w.key(KeyCode::KeyZ, false);
        assert_eq!(w.sim.glyphs, vec![None]);
        w.pick(vec![Id::Arm(0)]);
        w.key(KeyCode::KeyZ, false);
        assert!(w.sim.arms.is_empty());
        assert_eq!(w.focus, None);
    }

    #[test]
    fn the_shifted_keys_write_the_six_moves_to_a_focused_tape() {
        let mut w = armed(vec![Instr::Wait]);
        w.focus_tape(0);
        for key in [KeyCode::KeyW, KeyCode::KeyC, KeyCode::KeyF] {
            w.key(key, true);
        }
        assert_eq!(
            w.sim.arms[0].tape,
            vec![Instr::Wait, Instr::Move(4), Instr::Move(1), Instr::Move(0)]
        );
        assert_eq!(w.focus, Some(Focus::Tape { arm: 0, cursor: 4 }));
        w.key(KeyCode::KeyF, false);
        assert_eq!(w.sim.arms[0].tape[4], Instr::Grab);
    }

    #[test]
    fn the_six_move_keys_ring_the_pivot_clockwise_from_the_upper_left_and_sum_to_nothing() {
        let moves: Vec<(KeyCode, usize)> = KEYS
            .iter()
            .filter_map(|k| match k.instr {
                Instr::Move(d) => Some((k.code, d)),
                _ => None,
            })
            .collect();
        let (codes, dirs): (Vec<KeyCode>, Vec<usize>) = moves.into_iter().unzip();
        use KeyCode::*;
        assert_eq!(codes, [KeyW, KeyE, KeyF, KeyC, KeyX, KeyA]);
        assert_eq!(dirs, [4, 5, 0, 1, 2, 3]);
        for pair in dirs.windows(2) {
            assert_eq!(pair[1], Spin::Cw.turn(pair[0]));
        }
        assert_eq!(px(DIRS[dirs[0]]).x.signum(), -1.0);
        assert_eq!(px(DIRS[dirs[0]]).y.signum(), 1.0);
        let sum = dirs.iter().fold(ORIGIN, |h, d| h.add(DIRS[*d]));
        assert_eq!(sum, ORIGIN);
    }

    #[test]
    fn the_walk_scene_repeats_its_lap_with_the_same_stall() {
        let (mut world, _, _, _) = shot::scene("walk", 0);
        let lap = 11;
        let stalls = |world: &mut World| -> Vec<Option<Stall>> {
            (0..lap)
                .map(|_| {
                    world.sim.step();
                    world.sim.arms[0].stall
                })
                .collect()
        };

        let first = stalls(&mut world);
        let home = world.sim.arms[0].clone();
        assert_eq!(home.pivot, Hex::new(-4, 0));
        assert_eq!(first.iter().filter(|s| s.is_some()).count(), 3);
        assert_eq!(first[2..5], [Some(Stall::Illegal); 3]);
        assert_eq!(stalls(&mut world), first);
        assert_eq!(world.sim.arms[0].pivot, home.pivot);
        assert!(world.sim.arms[1].stall.is_none());
    }

    #[test]
    fn mid_move_the_base_hand_and_held_atom_slide_together() {
        let mut prev = Sim::empty();
        let mut arm = Arm::new(Hex::new(0, 0), 0, vec![Instr::Move(4), Instr::Wait]);
        arm.holding = true;
        prev.atoms.push(Some(Atom {
            kind: AtomKind::Base,
            pos: arm.hand(),
        }));
        prev.arms.push(arm);
        let mut cur = prev.clone();
        cur.step();
        assert_eq!(cur.arms[0].pivot, DIRS[4]);
        let f = Frame::between(&prev, &cur, 0.3);
        let still = Frame::settled(&prev);
        let slid = f.arms[0].pivot - still.arms[0].pivot;
        assert!(slid.length() > 0.0 && slid.length() < px(DIRS[4]).length());
        let near = |a: Vec2, b: Vec2| (a - b).length() < 1e-3;
        assert!(near(f.arms[0].hand - still.arms[0].hand, slid));
        assert!(near(f.atoms[0].unwrap() - still.atoms[0].unwrap(), slid));
        let f = Frame::between(&prev, &cur, 1.0);
        assert_eq!(f.arms[0].pivot, px(DIRS[4]));
    }

    #[test]
    fn tape_focus_keys_insert_at_the_cursor_and_leave_the_arm_alone() {
        let mut w = armed(vec![Instr::Wait, Instr::Drop]);
        w.focus_tape(0);
        w.key(KeyCode::ArrowLeft, false);
        w.key(KeyCode::KeyF, false);
        w.key(KeyCode::KeyA, false);
        w.key(KeyCode::KeyD, false);
        w.key(KeyCode::KeyX, false);
        assert_eq!(
            w.sim.arms[0].tape,
            vec![
                Instr::Wait,
                Instr::Grab,
                Instr::Rot(Spin::Ccw),
                Instr::Rot(Spin::Cw),
                Instr::Wait,
                Instr::Drop
            ]
        );
        assert_eq!(w.focus, Some(Focus::Tape { arm: 0, cursor: 5 }));
        assert!(!w.sim.arms[0].holding);
        assert_eq!(w.sim.arms[0].dir, 0);
        assert_eq!(w.sim.atoms[0].unwrap().pos, DIRS[0]);
    }

    #[test]
    fn q_and_e_write_pivots_to_a_focused_tape_and_turn_nothing_else() {
        let mut w = armed(vec![Instr::Wait]);
        w.focus_tape(0);
        w.key(KeyCode::KeyQ, false);
        w.key(KeyCode::KeyE, false);
        assert_eq!(
            w.sim.arms[0].tape,
            vec![Instr::Wait, Instr::Pivot(Spin::Ccw), Instr::Pivot(Spin::Cw)]
        );
        assert_eq!(w.focus, Some(Focus::Tape { arm: 0, cursor: 3 }));
        w.sim.glyphs.push(Some(Glyph {
            kind: GlyphKind::Bonder,
            at: Hex::new(3, 3),
            dir: 2,
        }));
        w.pick(vec![Id::Glyph(0)]);
        w.key(KeyCode::KeyQ, false);
        w.key(KeyCode::KeyE, false);
        assert_eq!(w.sim.glyphs[0].unwrap().dir, 2);
        w.lift(fresh(Item::Arm), Back::Nowhere);
        w.key(KeyCode::KeyQ, false);
        w.key(KeyCode::KeyE, false);
        assert_eq!(held_dir(&w), 0);
    }

    #[test]
    fn glyph_focus_and_a_held_item_turn_with_a_and_d() {
        let bonder = Glyph {
            kind: GlyphKind::Bonder,
            at: ORIGIN,
            dir: 0,
        };
        let mut w = lone(vec![bonder], vec![]);
        w.pick(vec![Id::Glyph(0)]);
        w.key(KeyCode::KeyA, false);
        assert_eq!(w.sim.glyphs[0].unwrap().dir, 5);
        w.key(KeyCode::KeyD, false);
        w.key(KeyCode::KeyD, false);
        assert_eq!(w.sim.glyphs[0].unwrap().dir, 1);
        assert_eq!(w.prev, w.sim);
        w.lift(fresh(Item::Arm), Back::Nowhere);
        w.key(KeyCode::KeyD, false);
        assert_eq!(held_dir(&w), 1);
        w.key(KeyCode::KeyA, false);
        w.key(KeyCode::KeyA, false);
        assert_eq!(held_dir(&w), 5);
    }

    #[test]
    fn z_in_tape_focus_removes_the_instruction_before_the_cursor() {
        let mut w = armed(vec![Instr::Grab, Instr::Rot(Spin::Cw), Instr::Drop]);
        w.focus_tape(0);
        w.key(KeyCode::ArrowLeft, false);
        w.key(KeyCode::KeyZ, false);
        assert_eq!(w.sim.arms[0].tape, vec![Instr::Grab, Instr::Drop]);
        assert_eq!(w.focus, Some(Focus::Tape { arm: 0, cursor: 1 }));
        w.key(KeyCode::Home, false);
        w.key(KeyCode::KeyZ, false);
        assert_eq!(w.sim.arms[0].tape, vec![Instr::Grab, Instr::Drop]);
        assert_eq!(w.focus, Some(Focus::Tape { arm: 0, cursor: 0 }));
        w.key(KeyCode::End, false);
        assert_eq!(w.focus, Some(Focus::Tape { arm: 0, cursor: 2 }));
    }

    #[test]
    fn a_copied_arm_is_a_blueprint_without_its_running_state() {
        let mut w = cluster();
        w.sim.arms[0].pc = 1;
        w.sim.arms[0].holding = true;
        w.sim.arms[0].stall = Some(Stall::Illegal);
        w.pick(vec![Id::Arm(0)]);
        w.key(KeyCode::KeyC, false);
        w.key(KeyCode::KeyV, false);
        w.press(px(Hex::new(6, 6)), px(Hex::new(6, 6)));
        let pasted = &w.sim.arms[2];
        assert_eq!(*pasted, Arm::new(Hex::new(6, 6), 3, pasted.tape.clone()));
    }

    #[test]
    fn a_press_whose_selection_was_cleared_before_the_drag_lifts_nothing() {
        let mut w = cluster();
        w.press(px(ORIGIN), px(ORIGIN));
        w.key(KeyCode::Escape, false);
        w.drag(px(ORIGIN) + Vec2::new(DRAG_PX * 2.0, 0.0));
        assert_eq!(w.focus, None);
        assert_eq!(w.down, None);
    }

    #[test]
    fn a_palette_placement_lands_its_anchor_on_the_cursor_cell() {
        let mut w = lone(vec![], vec![]);
        w.lift(fresh(Item::Arm), Back::Nowhere);
        let to = Hex::new(-2, 3);
        w.release(Some(to));
        assert_eq!(w.sim.arms[0].pivot, to);
        assert_eq!(w.focus, picked(&[Id::Arm(0)]));
    }

    fn fed_cleanup() -> World {
        let mut w = World::new(Sim::empty());
        let glyph = |kind, q| Glyph {
            kind,
            at: Hex::new(q, 0),
            dir: 0,
        };
        w.sim.glyphs = vec![
            Some(glyph(GlyphKind::Source, -4)),
            Some(glyph(GlyphKind::Cleanup, 0)),
            Some(glyph(GlyphKind::Bonder, 4)),
        ];
        w.sim.spawn(Atom {
            kind: AtomKind::Base,
            pos: ORIGIN,
        });
        w
    }

    #[test]
    fn a_spent_glyph_leaves_the_pick_and_the_rest_keep_their_ids() {
        let mut w = fed_cleanup();
        w.pick(vec![Id::Glyph(0), Id::Glyph(1), Id::Glyph(2)]);
        w.step();
        assert_eq!(w.focus, picked(&[Id::Glyph(0), Id::Glyph(2)]));
        assert_eq!(w.sim.glyphs[1], None);
        assert_eq!(w.sim.glyphs[2].unwrap().kind, GlyphKind::Bonder);
        assert_eq!(
            w.ids().collect::<Vec<Id>>(),
            vec![Id::Glyph(0), Id::Glyph(2)]
        );
        assert!(w.prev.glyphs[1].is_some());
    }

    #[test]
    fn a_hold_keeps_its_glyph_id_and_a_hold_on_a_spent_glyph_cancels() {
        let mut w = fed_cleanup();
        w.lift(
            w.lifted(&[Id::Glyph(2)], ORIGIN),
            Back::Pick(vec![Id::Glyph(2)]),
        );
        w.step();
        let Some(Focus::Hold { back, set }) = w.focus.clone() else {
            panic!("{:?}", w.focus)
        };
        assert_eq!(back, Back::Pick(vec![Id::Glyph(2)]));
        assert_eq!(set.glyphs.len(), 1);
        let mut w = fed_cleanup();
        let ids = [Id::Glyph(1), Id::Glyph(2)];
        w.lift(w.lifted(&ids, ORIGIN), Back::Pick(ids.to_vec()));
        w.step();
        assert_eq!(w.focus, None);
    }

    #[test]
    fn focus_on_a_spent_glyph_alone_clears() {
        let mut w = fed_cleanup();
        w.pick(vec![Id::Glyph(1)]);
        w.step();
        assert_eq!(w.focus, None);
    }

    fn paused(n: usize) -> World {
        let (mut w, _, _, _) = shot::scene("walk", 2);
        for _ in 0..n {
            w.key(KeyCode::KeyG, false);
        }
        w
    }

    #[test]
    fn n_presses_of_g_show_one_replay_of_n_ticks_from_ghost0_and_leave_it_untouched() {
        let w = paused(0);
        let ghost0 = w.sim.clone();
        let w = paused(5);
        assert_eq!(w.ghosts(), 5);
        assert_eq!(w.sim, ghost0);
        assert_eq!(*w.shown(), ghost0.replay(5));
        assert_eq!(w.prev, ghost0.replay(4));
        assert_ne!(*w.shown(), ghost0);
        assert_eq!(w.since, 0.0);
    }

    #[test]
    fn a_tape_edit_at_n_re_sims_to_the_ghost_n_of_stepping_from_scratch_with_that_tape() {
        let mut w = paused(3);
        w.focus_tape(0);
        w.key(KeyCode::Home, false);
        w.key(KeyCode::KeyD, false);
        assert_eq!(w.sim.arms[0].tape[0], Instr::Rot(Spin::Cw));
        let mut scratch = paused(0);
        scratch.sim.arms[0].tape.insert(0, Instr::Rot(Spin::Cw));
        assert_eq!(w.sim, scratch.sim);
        for _ in 0..3 {
            scratch.key(KeyCode::KeyG, false);
        }
        assert_eq!(w.ghosts(), 3);
        assert_eq!(*w.shown(), *scratch.shown());
        assert_eq!(*w.shown(), w.sim.replay(3));
        assert_eq!(w.prev, *w.shown());
        assert_eq!(w.focus, Some(Focus::Tape { arm: 0, cursor: 1 }));
    }

    #[test]
    fn an_arm_edit_at_n_is_refused_and_ghost0_is_unchanged() {
        let mut w = paused(4);
        let ghost0 = w.sim.clone();
        let ghost4 = w.shown().clone();
        let pivot = ghost4.arms[0].pivot;
        assert_ne!(pivot, ghost0.arms[0].pivot);
        drag(&mut w, pivot, pivot.add(Hex::new(3, 0)));
        assert_eq!(w.focus, picked(&[Id::Arm(0)]));
        w.key(KeyCode::KeyD, false);
        w.key(KeyCode::KeyC, false);
        w.key(KeyCode::KeyX, false);
        assert!(w.clipboard.is_none());
        w.key(KeyCode::KeyZ, false);
        w.lift(fresh(Item::Arm), Back::Nowhere);
        w.place(Some(Hex::new(5, 5)));
        assert_eq!(w.sim, ghost0);
        assert_eq!(*w.shown(), ghost4);
        assert_eq!(w.focus, None);
    }

    #[test]
    fn a_glyph_placed_at_n_appears_in_ghost0_and_in_the_re_simmed_ghost_n() {
        let mut w = paused(2);
        let ghost0 = w.sim.clone();
        let at = Hex::new(5, 5);
        w.lift(fresh(Item::Glyph(GlyphKind::Bonder)), Back::Nowhere);
        w.place(Some(at));
        let placed = Some(bonder(at, 0));
        assert_eq!(w.sim.glyphs.last().copied(), Some(placed));
        assert_eq!(w.shown().glyphs.last().copied(), Some(placed));
        assert_eq!(w.ghosts(), 2);
        assert_eq!(*w.shown(), w.sim.replay(2));
        assert_eq!(w.sim.arms, ghost0.arms);
        assert_eq!(w.focus, picked(&[Id::Glyph(0)]));
        w.key(KeyCode::KeyZ, false);
        let mut cleared = ghost0.clone();
        cleared.glyphs.push(None);
        assert_eq!(w.sim, cleared);
        assert_eq!(*w.shown(), cleared.replay(2));
    }

    #[test]
    fn unpause_restores_ghost0_exactly_and_the_ghosts_are_gone() {
        let mut w = paused(3);
        let ghost0 = w.sim.clone();
        w.key(KeyCode::Space, false);
        assert!(w.running);
        assert_eq!(w.ghost, None);
        assert_eq!(w.sim, ghost0);
        assert_eq!(w.prev, ghost0);
        assert_eq!(w.ghosts(), 0);
        w.key(KeyCode::KeyG, false);
        w.key(KeyCode::KeyS, false);
        assert_eq!(w.sim, ghost0);
        w.step();
        assert_eq!(w.sim, ghost0.replay(1));
        assert_eq!(w.ghost, None);
        let mut w = paused(3);
        w.running = true;
        w.step();
        assert_eq!(w.ghost, None);
        assert_eq!(w.ghosts(), 0);
    }

    #[test]
    fn the_step_keys_never_reach_the_tape_editor() {
        assert!(
            KEYS.iter()
                .all(|k| k.code != KeyCode::KeyS && k.code != KeyCode::KeyG)
        );
        let mut w = paused(0);

        w.focus_tape(0);
        w.key(KeyCode::ArrowLeft, false);
        w.key(KeyCode::KeyF, false);
        let focus = w.focus.clone();
        let tape = w.sim.arms[0].tape.clone();
        w.key(KeyCode::KeyG, false);
        w.key(KeyCode::KeyG, false);
        w.key(KeyCode::KeyS, false);
        assert_eq!(w.ghosts(), 1);
        assert_eq!(w.sim.arms[0].tape, tape);
        assert_eq!(w.shown().arms[0].tape, tape);
        assert_eq!(w.focus, focus);
    }

    #[test]
    fn s_at_ghost0_is_a_no_op() {
        let mut w = paused(0);
        w.focus_tape(0);
        let (sim, prev, focus, since) = (w.sim.clone(), w.prev.clone(), w.focus.clone(), w.since);
        w.key(KeyCode::KeyS, false);
        assert_eq!(w.ghosts(), 0);
        assert_eq!(w.ghost, None);
        assert_eq!((w.sim, w.prev, w.focus, w.since), (sim, prev, focus, since));
    }

    fn glyph(kind: GlyphKind, at: Hex, dir: usize) -> Glyph {
        Glyph { kind, at, dir }
    }

    fn pair(w: &mut World, at: Hex, kind: BondKind) -> [usize; 2] {
        let atom = |pos| Atom {
            kind: AtomKind::Base,
            pos,
        };
        let a = w.sim.spawn(atom(at));
        let b = w.sim.spawn(atom(at.add(DIRS[0])));
        w.sim.bonds.push(Bond { a, b, kind });
        w.prev = w.sim.clone();
        [a, b]
    }

    fn lift_at(w: &mut World, cell: Hex) {
        w.press(px(cell), px(cell));
        w.drag(px(cell) + Vec2::new(DRAG_PX * 2.0, 0.0));
    }

    fn held(w: &World) -> (Vec<Hex>, Vec<Bond>, Back) {
        let Some(Focus::Hold { set, back }) = &w.focus else {
            panic!("not holding: {:?}", w.focus)
        };
        let cells = set.atoms.iter().flatten().map(|a| a.pos).collect();
        (cells, set.bonds.clone(), back.clone())
    }

    fn taken(cell: Hex) -> Back {
        Back::Cell { cell, turns: 0 }
    }

    fn atoms(w: &World) -> Vec<Hex> {
        w.sim.atoms.iter().flatten().map(|a| a.pos).collect()
    }

    #[test]
    fn a_drag_from_an_atom_takes_its_whole_compound_off_the_grid_and_a_drag_from_the_picked_glyph_under_it_takes_the_glyph()
     {
        let mut w = lone(vec![bonder(ORIGIN, 0)], vec![]);
        w.running = false;
        pair(&mut w, ORIGIN, BondKind::Single);
        let loose = w.sim.spawn(Atom {
            kind: AtomKind::Base,
            pos: Hex::new(3, 3),
        });
        lift_at(&mut w, DIRS[0]);
        let (cells, bonds, back) = held(&w);
        assert_eq!(cells, vec![ORIGIN, DIRS[3]]);
        assert_eq!(
            bonds,
            vec![Bond {
                a: 1,
                b: 0,
                kind: BondKind::Single
            }]
        );
        assert_eq!(back, taken(DIRS[0]));
        assert_eq!(atoms(&w), vec![Hex::new(3, 3)]);
        assert_eq!(w.sim.atom_at(Hex::new(3, 3)), Some(loose));
        assert!(w.sim.bonds.is_empty());
        assert_eq!(w.sim.glyphs[0], Some(bonder(ORIGIN, 0)));
        assert_eq!(w.prev, w.sim);
        assert_eq!(w.down, None);
        w.release(None);
        assert_eq!(w.focus, None);
        w.press(px(DIRS[0]), px(DIRS[0]));
        assert_eq!(w.focus, picked(&[Id::Glyph(0)]));
        lift_at(&mut w, DIRS[0]);
        assert_eq!(held(&w).2, Back::Pick(vec![Id::Glyph(0)]));
        assert_eq!(atoms(&w).len(), 3);
        w.release(None);
        w.press(px(Hex::new(3, 3)), px(Hex::new(3, 3)));
        assert_eq!(w.focus, None);
        assert!(matches!(w.down, Some(Press::Atom { .. })));
    }

    #[test]
    fn a_and_d_turn_the_lifted_compound_about_the_grabbed_atom_and_escape_puts_it_back_as_it_lay() {
        let mut w = lone(vec![], vec![]);
        pair(&mut w, ORIGIN, BondKind::Single);
        let before = w.sim.clone();
        lift_at(&mut w, ORIGIN);
        w.key(KeyCode::KeyD, false);
        assert_eq!(held(&w).0, vec![ORIGIN, DIRS[1]]);
        w.key(KeyCode::KeyA, false);
        w.key(KeyCode::KeyA, false);
        assert_eq!(held(&w).0, vec![ORIGIN, DIRS[5]]);
        assert_eq!(
            held(&w).2,
            Back::Cell {
                cell: ORIGIN,
                turns: 5
            }
        );
        w.key(KeyCode::Escape, false);
        assert_eq!(w.sim, before);
        assert_eq!(w.focus, None);
    }

    #[test]
    fn a_legal_drop_moves_every_atom_and_its_bonds() {
        let mut w = lone(vec![], vec![]);
        w.running = false;
        pair(&mut w, ORIGIN, BondKind::Double);
        let to = Hex::new(2, 3);
        drag(&mut w, DIRS[0], to);
        assert_eq!(w.focus, None);
        let a = w.sim.atom_at(to.sub(DIRS[0])).unwrap();
        let b = w.sim.atom_at(to).unwrap();
        assert_eq!(atoms(&w).len(), 2);
        assert_eq!(
            w.sim.bonds,
            vec![Bond {
                a,
                b,
                kind: BondKind::Double
            }]
        );
        assert_eq!(w.prev, w.sim);
    }

    #[test]
    fn a_drop_on_an_atom_or_a_base_pops_back_and_so_does_a_release_over_the_panels() {
        let squats = [
            (Hex::new(5, 5), Some(Hex::new(5, 5))),
            (Hex::new(5, 5), Some(Hex::new(6, 5))),
            (Hex::new(4, 0), None),
        ];
        for (blocked, squatter) in squats {
            let mut w = lone(vec![], vec![Arm::new(Hex::new(5, 0), 0, vec![])]);
            w.running = false;
            pair(&mut w, ORIGIN, BondKind::Double);
            if let Some(pos) = squatter {
                w.sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos,
                });
                w.prev = w.sim.clone();
            }
            let before = w.sim.clone();
            drag(&mut w, ORIGIN, blocked);
            assert_eq!(w.sim, before, "a drop at {blocked:?} must pop back");
            assert_eq!(w.focus, None);
            lift_at(&mut w, ORIGIN);
            w.release(None);
            assert_eq!(w.sim, before);
            assert_eq!(w.focus, None);
        }
    }

    #[test]
    fn a_drop_that_would_not_fit_where_it_was_lifted_stays_in_the_hand() {
        let mut w = lone(vec![], vec![]);
        w.running = false;
        pair(&mut w, ORIGIN, BondKind::Single);
        lift_at(&mut w, ORIGIN);
        let squatter = w.sim.spawn(Atom {
            kind: AtomKind::Base,
            pos: DIRS[0],
        });
        w.release(Some(DIRS[0]));
        assert_eq!(held(&w).2, taken(ORIGIN));
        assert_eq!(atoms(&w), vec![DIRS[0]]);
        w.key(KeyCode::KeyZ, false);
        assert_eq!(held(&w).2, taken(ORIGIN));
        w.sim.consume(&[squatter]);
        w.press(px(ORIGIN), px(ORIGIN));
        assert_eq!(w.focus, None);
        assert_eq!(atoms(&w), vec![ORIGIN, DIRS[0]]);
    }

    #[test]
    fn no_glyph_fires_on_a_carried_compound_and_a_drop_on_an_output_is_eaten_at_the_end_of_that_tick()
     {
        let at = Hex::new(2, -2);
        let mut w = lone(vec![glyph(GlyphKind::Output, at, 0)], vec![]);
        w.running = false;
        pair(&mut w, at, BondKind::Double);
        lift_at(&mut w, at);
        w.step();
        assert_eq!(w.sim.delivered, 0);
        w.pointer = Some(px(at));
        w.release(Some(at));
        assert_eq!(w.sim.delivered, 0);
        assert_eq!(atoms(&w).len(), 2);
        w.step();
        assert_eq!(w.sim.delivered, 1);
        assert_eq!(atoms(&w), vec![]);
    }

    #[test]
    fn an_arm_whose_compound_the_pointer_lifts_is_left_closed_on_nothing_and_its_rotate_runs() {
        let mut w = lone(
            vec![],
            vec![Arm::new(Hex::new(-1, 0), 0, vec![Instr::Rot(Spin::Cw)])],
        );
        w.running = false;
        w.sim.arms[0].holding = true;
        pair(&mut w, ORIGIN, BondKind::Single);
        assert_eq!(w.sim.replay(1).arms[0].stall, None);
        lift_at(&mut w, DIRS[0]);
        w.step();
        let arm = &w.sim.arms[0];
        assert_eq!(
            (arm.stall, arm.dir, arm.holding, arm.pc),
            (None, 1, true, 1)
        );
        assert_eq!(atoms(&w), vec![]);
        w.release(None);
        assert_eq!(atoms(&w), vec![DIRS[0], ORIGIN]);
        assert_eq!(w.sim.atom_at(w.sim.arms[0].hand()), None);
    }

    #[test]
    fn an_atom_drag_at_ghost_n_is_refused_and_both_frames_are_unchanged_even_after_the_ghost_is_stepped_away()
     {
        let mut w = paused(6);
        let ghost0 = w.sim.clone();
        let ghost6 = w.shown().clone();
        let carried = ghost6
            .atoms
            .iter()
            .flatten()
            .map(|a| a.pos)
            .find(|c| ghost0.atom_at(*c).is_none())
            .unwrap();
        lift_at(&mut w, carried);
        assert_eq!(held(&w).2, Back::Ghost);
        assert_eq!(w.sim, ghost0);
        w.pointer = Some(px(Hex::new(5, 5)));
        w.release(Some(Hex::new(5, 5)));
        assert_eq!(w.sim, ghost0);
        assert_eq!(*w.shown(), ghost6);
        assert_eq!(w.focus, None);
        lift_at(&mut w, carried);
        for _ in 0..6 {
            w.key(KeyCode::KeyS, false);
        }
        assert_eq!(w.ghosts(), 0);
        w.release(Some(Hex::new(5, 5)));
        assert_eq!(w.sim, ghost0);
        assert_eq!(w.focus, None);
    }

    fn played(name: &str, frames: u32) -> World {
        let (mut w, _, script, warm) = shot::scene(name, 0);
        for frame in 1..=frames {
            w.since = (w.since + 1.0 / 60.0).min(w.period);
            if w.running && w.since >= w.period {
                w.since = 0.0;
                w.step();
            }
            for (at, act) in &script {
                if *at != frame {
                    continue;
                }
                match *act {
                    shot::Act::Down(key) => w.key(key, false),
                    shot::Act::Up(_) => {}
                    shot::Act::Press(cell) => {
                        w.pointer = Some(px(cell));
                        w.press(px(cell), px(cell));
                    }
                    shot::Act::Drag(cell) => {
                        w.pointer = Some(px(cell));
                        w.drag(px(cell));
                    }
                    shot::Act::Release(cell) => {
                        w.pointer = Some(px(cell));
                        w.release(Some(cell));
                    }
                }
            }
            if frame == warm {
                w.running = true;
                w.since = 0.0;
            }
        }
        w
    }

    #[test]
    fn the_hand_scene_delivers_one_compound_built_by_hand_with_no_arm_on_the_board() {
        let w = played("hand", 300);
        assert!(w.sim.arms.is_empty());
        assert_eq!(w.sim.delivered, 1, "{:?}", w.sim);
        assert_eq!(w.focus, None);
        assert_eq!(atoms(&w).len(), 1);
    }
}
