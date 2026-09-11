mod form;
mod look;
#[cfg(not(target_arch = "wasm32"))]
mod machines;
mod particles;
mod persist;
mod rig;
mod sim;
mod sound;

use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::camera::visibility::RenderLayers;
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
use bevy::ui::IsDefaultUiCamera;
use bevy::window::{CursorLeft, PrimaryWindow};
use form::{Form, recipes};
use look::{Finish, Glaze, HEX, Look, MANUAL, MachineMark, Shape, Skin, px, skin};
use sim::{
    Arm, BondKind, DIRS, Fixture, Glyph, GlyphKind, Hex, Id, Instr, Item, Machine, ORIGIN, Short,
    Sim, Spin, Stall, fixture,
};

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
const STEP_PX: f32 = 2.0 * MARK_PX;
const TALLY_PX: f32 = 86.0;
const PALETTE_WIDTH: f32 = PALETTE_PX + SYMBOL_PX + 2.0 * (TALLY_PX + 24.0) + 8.0;
const CARD_SCALE: f32 = 1.0;
const CARD_PAD: f32 = 12.0;
const CARD: RenderLayers = RenderLayers::layer(1);

fn atom_index(kind: sim::AtomKind) -> usize {
    sim::AtomKind::ALL
        .iter()
        .position(|other| *other == kind)
        .unwrap()
}

fn atom_layer(kind: sim::AtomKind) -> RenderLayers {
    RenderLayers::layer(2 + atom_index(kind))
}

#[cfg(test)]
static RENDER_TEST: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn brass(lift: f32) -> Color {
    Glaze::Brass.color().mix(&Glaze::Clay.color(), lift)
}

fn strip(lit: bool) -> Color {
    if lit { brass(0.3) } else { brass(0.0) }
}

const IVORY: Color = Glaze::Ivory.color();

#[derive(Clone, Copy, Component)]
struct PaletteRow(Item);

fn palette() -> impl Iterator<Item = Item> {
    recipes()
        .iter()
        .map(|(item, _)| *item)
        .chain(sim::AtomKind::ALL.map(Item::Atom))
}

#[derive(Clone, Copy)]
pub struct Key {
    code: KeyCode,
    instr: Instr,
    pub symbol: Skin,
}

impl Key {
    const fn new(code: KeyCode, instr: Instr, symbol: Skin) -> Key {
        Key {
            code,
            instr,
            symbol,
        }
    }

    fn shifted(&self) -> bool {
        matches!(self.instr, Instr::Move(_))
    }
}

const UPPER_LEFT: usize = 4;

pub const KEYS: [Key; 13] = [
    Key::new(KeyCode::KeyF, Instr::Grab, skin!("symbols/f")),
    Key::new(KeyCode::KeyR, Instr::Drop, skin!("symbols/r")),
    Key::new(KeyCode::KeyA, Instr::Rot(Spin::Ccw), skin!("symbols/a")),
    Key::new(KeyCode::KeyD, Instr::Rot(Spin::Cw), skin!("symbols/d")),
    Key::new(KeyCode::KeyQ, Instr::Pivot(Spin::Ccw), skin!("symbols/q")),
    Key::new(KeyCode::KeyE, Instr::Pivot(Spin::Cw), skin!("symbols/e")),
    Key::new(KeyCode::KeyX, Instr::Wait, skin!("symbols/x")),
    Key::new(
        KeyCode::KeyW,
        Instr::Move(UPPER_LEFT),
        skin!("symbols/shift-w"),
    ),
    Key::new(
        KeyCode::KeyE,
        Instr::Move((UPPER_LEFT + 1) % 6),
        skin!("symbols/shift-e"),
    ),
    Key::new(
        KeyCode::KeyF,
        Instr::Move((UPPER_LEFT + 2) % 6),
        skin!("symbols/shift-f"),
    ),
    Key::new(
        KeyCode::KeyC,
        Instr::Move((UPPER_LEFT + 3) % 6),
        skin!("symbols/shift-c"),
    ),
    Key::new(
        KeyCode::KeyX,
        Instr::Move((UPPER_LEFT + 4) % 6),
        skin!("symbols/shift-x"),
    ),
    Key::new(
        KeyCode::KeyA,
        Instr::Move((UPPER_LEFT + 5) % 6),
        skin!("symbols/shift-a"),
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

fn fresh(item: Item) -> Sim {
    let mut set = Sim::empty();
    match item {
        Item::Machine(Machine::Arm) => set.arms.push(Arm::new(ORIGIN, 0, Vec::new())),
        Item::Machine(Machine::Glyph(kind)) => set.glyphs.push(Some(Glyph::new(kind, ORIGIN, 0))),
        Item::Atom(kind) => {
            set.spawn(sim::Atom { kind, pos: ORIGIN });
        }
        Item::Step | Item::Token(_) => {}
    }
    set
}

fn machines(ids: &[Id]) -> Vec<Id> {
    ids.iter()
        .copied()
        .filter(|id| !matches!(id, Id::Atom(_)))
        .collect()
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
    Inventory,
    Ghost,
    Pick(Vec<Id>),
    Cell { cell: Hex, turns: usize },
}

impl Back {
    fn picked(&self) -> &[Id] {
        match self {
            Back::Pick(ids) => ids,
            _ => &[],
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Focus {
    Pick(Vec<Id>),
    Tape { arm: usize, cursor: usize },
    Hold { set: Box<Sim>, back: Back },
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
        if let Focus::Pick(ids) = &mut self {
            ids.retain(|id| !matches!(id, Id::Atom(i) if sim.atoms[*i].is_none()));
            if ids.is_empty() {
                return None;
            }
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct Refused {
    at: Hex,
    short: Vec<Short>,
}

#[derive(Resource)]
struct Saved {
    attempted: Sim,
    refused: bool,
}

impl Saved {
    fn store(&mut self, sim: Sim) {
        self.attempted = sim;
        match persist::store(&self.attempted) {
            Ok(()) => self.refused = false,
            Err(reason) if !self.refused => {
                persist::refuse(&reason);
                self.refused = true;
            }
            Err(_) => {}
        }
    }
}

#[derive(Clone, Copy, Component)]
enum SaveAction {
    Export,
    Import,
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
    hover: Option<Item>,
    play: Option<Play>,
    events: Vec<sim::TickEvents>,
    score: Option<sim::TickEvents>,
    refused: Option<Refused>,
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
            hover: None,
            play: None,
            events: Vec::new(),
            score: None,
            refused: None,
        }
    }

    fn advance(&mut self, dt: f32) {
        self.since = (self.since + dt).min(self.period);
        if self.running && self.saveable() && self.since >= self.period {
            self.since = 0.0;
            self.step();
        }
        let Some(Item::Machine(machine)) = self.hover else {
            self.play = None;
            return;
        };
        let play = match &mut self.play {
            Some(play) if play.machine == machine => play,
            _ => self.play.insert(Play::at(machine, 0)),
        };
        play.since = (play.since + dt).min(self.period);
        if play.since >= self.period {
            play.since = 0.0;
            play.step();
        }
    }

    fn lift_inventory(&mut self, item: Item) {
        self.refused = None;
        if matches!(item, Item::Machine(_) | Item::Atom(_))
            && self
                .sim
                .inventory
                .count(item)
                .is_some_and(|count| count > 0)
        {
            self.lift(fresh(item), Back::Inventory);
        }
    }

    fn set_cap(&mut self, item: Item, notches: i32) {
        self.sim.inventory.set_cap(item, notches);
        self.resim(self.ghosts());
    }

    fn item(&self, id: Id) -> Option<Machine> {
        match id {
            Id::Arm(_) => Some(Machine::Arm),
            Id::Glyph(i) => Some(Machine::Glyph(self.glyph(i).kind)),
            Id::Atom(_) => None,
        }
    }

    fn shown(&self) -> &Sim {
        self.ghost.as_ref().unwrap_or(&self.sim)
    }

    fn ghosts(&self) -> u64 {
        self.shown().tick - self.sim.tick
    }

    fn unpick_atoms(&mut self) {
        if let Some(Focus::Pick(ids)) = &mut self.focus {
            ids.retain(|id| !matches!(id, Id::Atom(_)));
            if ids.is_empty() {
                self.focus = None;
            }
        }
    }

    fn step(&mut self) {
        if self.ghost.is_some() {
            self.unpick_atoms();
        }
        self.ghost = None;
        self.prev = self.sim.clone();
        let tick = self.sim.step();
        self.score = Some(tick.clone());
        self.events = vec![tick];

        self.focus = self.focus.take().and_then(|f| f.survive(&self.sim));
    }

    fn resim(&mut self, n: u64) {
        if n > 0 || self.ghost.is_some() {
            self.unpick_atoms();
        }
        let (ghost, events) = self.sim.replayed(n);
        self.events = events;
        self.ghost = (n > 0).then_some(ghost);
        self.prev = self.shown().clone();
    }

    fn editable(&self, moves: bool) -> bool {
        !moves || self.ghost.is_none()
    }

    fn phase(&self) -> f32 {
        phase(self.since, self.period, self.motion)
    }

    fn holding(&self) -> bool {
        matches!(self.focus, Some(Focus::Hold { .. }))
    }

    fn saveable(&self) -> bool {
        !matches!(
            self.focus,
            Some(Focus::Hold {
                back: Back::Cell { .. },
                ..
            })
        )
    }

    fn snapshot(&self) -> Sim {
        let Some(Focus::Hold {
            set,
            back: Back::Cell { cell, turns },
        }) = &self.focus
        else {
            return self.sim.clone();
        };
        let mut set = (**set).clone();
        for _ in 0..*turns {
            turn(&mut set, Spin::Ccw);
        }
        let mut sim = self.sim.clone();
        sim.place(&set, *cell);
        sim
    }

    fn focus_tape(&mut self, arm: usize) {
        self.refused = None;
        if self.holding() {
            return;
        }
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
            Id::Atom(_) => unreachable!("an atom has no direction"),
        }
    }

    fn anchor(&self, id: Id) -> Hex {
        match id {
            Id::Arm(i) => self.shown().arms[i].pivot,
            Id::Glyph(i) => self.glyph(i).at,
            Id::Atom(i) => self.shown().atoms[i].unwrap().pos,
        }
    }

    fn cells(&self, id: Id) -> Vec<Hex> {
        let hand = match id {
            Id::Arm(i) => Some(self.shown().arms[i].hand()),
            _ => None,
        };
        self.shown().stands(id).chain(hand).collect()
    }

    fn hand_ids(&self) -> impl Iterator<Item = Id> + '_ {
        let sim = self.shown();
        let arms = (0..sim.arms.len()).map(Id::Arm);
        let glyphs = sim
            .glyphs
            .iter()
            .enumerate()
            .filter(|(_, g)| g.is_some_and(|g| g.kind != GlyphKind::Source));
        arms.chain(glyphs.map(|(i, _)| Id::Glyph(i)))
    }

    fn hit(&self, cell: Hex) -> Option<Id> {
        self.hand_ids()
            .find(|id| self.anchor(*id) == cell)
            .or_else(|| self.hand_ids().find(|id| self.cells(*id).contains(&cell)))
    }

    fn marquee(&self, a: Vec2, b: Vec2) -> Vec<Id> {
        let (lo, hi) = (a.min(b), a.max(b));
        let inside = |c: Hex| {
            let p = px(c);
            p.cmpge(lo).all() && p.cmple(hi).all()
        };
        let atoms = self.shown().atoms.iter().enumerate();
        self.hand_ids()
            .chain(atoms.filter_map(|(i, a)| a.map(|_| Id::Atom(i))))
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
                Id::Atom(_) => {}
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
            Id::Atom(_) => unreachable!("an atom is not posed"),
        }
    }

    fn unstall(&mut self) {
        for a in &mut self.sim.arms {
            a.stall = None;
        }
    }

    fn edits(&self, ids: &[Id]) -> bool {
        self.editable(ids.iter().any(|id| id.moves()))
    }

    fn remove(&mut self, ids: &[Id]) {
        if !self.edits(ids) {
            return;
        }
        let (mut arms, mut glyphs, mut atoms) = (Vec::new(), Vec::new(), Vec::new());
        for id in ids {
            match id {
                Id::Arm(i) => arms.push(*i),
                Id::Glyph(i) => glyphs.push(*i),
                Id::Atom(i) => atoms.push(*i),
            }
        }
        for i in glyphs {
            self.sim.glyphs[i] = None;
        }
        self.sim.consume(&atoms);
        arms.sort_unstable_by(|a, b| b.cmp(a));
        if !arms.is_empty() {
            self.unstall();
        }
        for i in arms {
            self.sim.arms.remove(i);
        }
        self.focus = None;
        self.down = None;
        self.resim(self.ghosts());
    }

    fn delete(&mut self, ids: &[Id]) {
        if !self.edits(ids) {
            return;
        }
        for id in ids {
            if let Some(item) = self.item(*id) {
                self.sim.inventory.add(Item::Machine(item));
            }
            if let Id::Arm(i) = id {
                for instr in std::mem::take(&mut self.sim.arms[*i].tape) {
                    self.sim.inventory.add(Item::Token(instr));
                }
            }
        }
        self.remove(ids);
    }

    fn copy(&mut self, ids: &[Id]) {
        let machines = machines(ids);
        if let Some(first) = machines.first() {
            self.clipboard = Some(self.lifted(&machines, self.anchor(*first)));
        }
    }

    fn compounds(&self, ids: &[Id]) -> String {
        let sim = self.shown();
        let mut seen: Vec<usize> = Vec::new();
        let mut lines: Vec<String> = Vec::new();
        for id in ids {
            let Id::Atom(i) = id else { continue };
            if seen.contains(i) {
                continue;
            }
            let compound = sim.component(*i);
            seen.extend(&compound);
            lines.push(Form::of(&sim.fragment(&compound, ORIGIN)).to_string());
        }
        lines.sort_unstable();
        lines.join("\n")
    }

    fn paste(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if clipboard_text().is_some_and(|text| self.paste_text(&text)) {
                return;
            }
            self.paste_machines();
        }

        #[cfg(target_arch = "wasm32")]
        clipboard_text();
    }

    fn paste_machines(&mut self) {
        if let Some(set) = self.clipboard.clone() {
            self.lift(set, Back::Inventory);
        }
    }

    fn paste_text(&mut self, text: &str) -> bool {
        let Ok(form) = text.parse::<Form>() else {
            return false;
        };
        self.lift(form.sim(), Back::Inventory);
        true
    }

    fn lift(&mut self, set: Sim, back: Back) {
        if self.holding() {
            return;
        }
        if let Back::Pick(ids) = &back {
            let machines = set.glyphs.iter().flatten().count() + set.arms.len();
            debug_assert_eq!(ids.len(), machines);
        }
        self.focus = Some(Focus::Hold {
            set: Box::new(set),
            back,
        });
        self.down = None;
    }

    fn press(&mut self, screen: Vec2, point: Vec2) {
        self.refused = None;
        let cell = hex_at(point);
        if self.holding() {
            self.place(Some(cell));
            return;
        }
        let hit = self.hit(cell);
        let atom = self.shown().atom_at(cell);
        let picked = hit.is_some_and(|id| self.picks(id));
        let in_pick = hit
            .is_some_and(|id| matches!(&self.focus, Some(Focus::Pick(ids)) if ids.contains(&id)));
        match hit {
            Some(_) if in_pick => {}
            Some(Id::Arm(arm)) => self.focus_tape(arm),
            Some(id) => self.pick(vec![id]),
            None => {
                if let Some(i) = atom {
                    self.pick(vec![Id::Atom(i)]);
                }
            }
        }
        self.down = Some(match hit {
            _ if atom.is_some() && !picked => Press::Atom { screen, cell },
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
                let ids = machines(&self.focus.as_ref().map_or(Vec::new(), Focus::picked));
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
        let legal = |at: &Hex| {
            back != Back::Ghost
                && self.editable(runs(&set))
                && self.sim.fits(&set, *at, back.picked())
        };
        let Some(at) = at.filter(legal) else {
            self.pop(*set, back);
            return;
        };
        if back == Back::Inventory
            && let Err(short) = self.sim.inventory.spend_all(&set.bill())
        {
            self.refused = Some(Refused { at, short });
            self.pop(*set, back);
            return;
        }
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
            Back::Inventory | Back::Ghost | Back::Cell { .. } => {
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
            Back::Inventory | Back::Ghost => {}
            Back::Pick(ids) => self.pick(ids),
            Back::Cell { cell, turns } => {
                for _ in 0..turns {
                    turn(&mut set, Spin::Ccw);
                }
                if self.sim.fits(&set, cell, &[]) {
                    self.sim.place(&set, cell);
                    self.resim(self.ghosts());
                } else {
                    let back = Back::Cell { cell, turns: 0 };
                    self.focus = Some(Focus::Hold {
                        set: Box::new(set),
                        back,
                    });
                }
            }
        }
    }

    fn key(&mut self, key: KeyCode, shift: bool) {
        use KeyCode::*;
        self.refused = None;
        match key {
            Space => {
                self.running = !self.running;
                if self.running {
                    self.resim(0);
                }
                return;
            }
            KeyG => {
                if !self.running && self.sim.inventory.spend(Item::Step) {
                    self.unpick_atoms();
                    let (prev, mut events) = self.sim.replayed(self.ghosts());
                    let mut ghost = prev.clone();
                    events.push(ghost.step());
                    self.prev = prev;
                    self.ghost = Some(ghost);
                    self.events = events;
                    self.since = 0.0;
                }
                return;
            }
            KeyS => {
                if let (false, Some(n)) = (self.running, self.ghosts().checked_sub(1))
                    && self.sim.inventory.spend(Item::Step)
                {
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
                    Back::Inventory | Back::Ghost => self.focus = None,
                    Back::Pick(ids) => self.delete(&ids),
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
                KeyZ => self.delete(&ids),
                KeyX | KeyC if !self.edits(&machines(&ids)) => {}
                KeyX | KeyC => {
                    let text = self.compounds(&ids);
                    if !text.is_empty() {
                        clipboard(&text);
                    }
                    self.copy(&ids);
                    if key == KeyX {
                        self.delete(&machines(&ids));
                    }
                }
                KeyV => self.paste(),
                _ => {
                    if let ([id], Some(Instr::Rot(spin))) = (ids.as_slice(), instr)
                        && !matches!(id, Id::Atom(_))
                        && self.editable(id.moves())
                    {
                        let at = self.anchor(*id);
                        let mut set = self.lifted(&ids, at);
                        turn(&mut set, spin);
                        if self.sim.fits(&set, at, &ids) {
                            self.set_pose(*id, at, spin.turn(self.dir(*id)));
                            self.resim(self.ghosts());
                        }
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
                        let erased = tape.remove(cursor - 1);
                        self.sim.inventory.add(Item::Token(erased));
                        cursor - 1
                    }
                    _ => match instr {
                        Some(instr) if self.sim.inventory.spend(Item::Token(instr)) => {
                            tape.insert(cursor, instr);
                            cursor + 1
                        }
                        _ => cursor,
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

#[cfg(not(target_arch = "wasm32"))]
fn clipboard(text: &str) {
    static HELD: std::sync::Mutex<Option<arboard::Clipboard>> = std::sync::Mutex::new(None);
    let mut held = HELD.lock().unwrap();
    let written = match &mut *held {
        Some(c) => c.set_text(text),
        None => arboard::Clipboard::new().and_then(|mut c| {
            let written = c.set_text(text);
            *held = Some(c);
            written
        }),
    };
    written.unwrap_or_else(|e| panic!("the clipboard refused the compound: {e}"));
}

#[cfg(not(target_arch = "wasm32"))]
fn clipboard_text() -> Option<String> {
    arboard::Clipboard::new()
        .and_then(|mut clipboard| clipboard.get_text())
        .ok()
}

#[cfg(target_arch = "wasm32")]
fn clipboard(text: &str) {
    let written = web_sys::window()
        .expect("a window")
        .navigator()
        .clipboard()
        .write_text(text);
    wasm_bindgen_futures::spawn_local(async move {
        wasm_bindgen_futures::JsFuture::from(written)
            .await
            .unwrap_or_else(|e| panic!("the clipboard refused the compound: {e:?}"));
    });
}

#[cfg(target_arch = "wasm32")]
fn clipboard_text() {
    let read = web_sys::window()
        .expect("a window")
        .navigator()
        .clipboard()
        .read_text();
    wasm_bindgen_futures::spawn_local(async move {
        let text = wasm_bindgen_futures::JsFuture::from(read)
            .await
            .ok()
            .and_then(|text| text.as_string());
        *PASTED.lock().unwrap() = Some(text);
    });
}

#[cfg(target_arch = "wasm32")]
static PASTED: std::sync::Mutex<Option<Option<String>>> = std::sync::Mutex::new(None);

#[cfg(target_arch = "wasm32")]
fn clipboard_paste(mut world: ResMut<World>) {
    if let Some(text) = PASTED.lock().unwrap().take() {
        if !text.is_some_and(|text| world.paste_text(&text)) {
            world.paste_machines();
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn clipboard_paste() {}

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

    fn screen(&self, p: Vec2) -> Vec2 {
        let d = (p - self.cam) / self.scale;
        Vec2::new(self.size.x / 2.0 + d.x, self.size.y / 2.0 - d.y)
    }

    fn shows(&self, p: Vec2) -> bool {
        (p - self.cam).abs().cmplt(self.half()).all()
    }

    fn sound(&self) -> sound::View {
        sound::View {
            center: self.cam,
            half: self.half(),
            scale: self.scale,
        }
    }
}

fn app(world: World) -> App {
    let mut app = App::new();
    let saved = Saved {
        attempted: world.sim.clone(),
        refused: false,
    };
    app.insert_resource(world)
        .insert_resource(saved)
        .insert_resource(ClearColor(brass(0.65)))
        .insert_gizmo_config(
            DefaultGizmoConfigGroup,
            GizmoConfig {
                line: GizmoLineConfig {
                    width: LINE_PX,
                    ..default()
                },
                ..default()
            },
        )
        .insert_gizmo_config(
            CardGizmos,
            GizmoConfig {
                render_layers: CARD,
                line: GizmoLineConfig {
                    width: LINE_PX,
                    ..default()
                },
                ..default()
            },
        )
        .add_systems(Startup, (fire_kiln, spawn_ui).chain())
        .add_systems(
            Update,
            (
                clipboard_paste,
                hover,
                sound::unlock,
                view,
                run_ticks,
                play_sound,
                edit,
                persistence,
                tapes,
                tally,
                refusal,
                board,
                draw,
                card,
                manual,
            )
                .chain(),
        );
    app
}

#[derive(Component)]
struct CardCamera;

fn card_camera(target: RenderTarget) -> impl Bundle {
    (CardCamera, card_view(target, CARD_SCALE, false))
}

fn card_view(target: RenderTarget, scale: f32, active: bool) -> impl Bundle {
    let mut projection = OrthographicProjection::default_2d();
    projection.scale = scale;
    (
        Camera2d,
        Camera {
            order: 1,
            is_active: active,
            clear_color: ClearColorConfig::Custom(brass(0.65)),
            ..default()
        },
        target,
        Projection::Orthographic(projection),
        CARD,
    )
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(status) = machines::configure(&args) {
        std::process::exit(status);
    }
    #[cfg(not(target_arch = "wasm32"))]
    if let Some(status) = shot::sound(&args) {
        std::process::exit(status);
    }
    let mut app = match shot::parse(&args) {
        Some((world, shot)) => shot::app(world, shot),
        None => {
            let mut app = app(World::new(persist::restore(sim::start())));
            app.add_plugins(DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "ziral".into(),
                    canvas: Some("#ziral".into()),
                    fit_canvas_to_parent: true,
                    ..default()
                }),
                ..default()
            }))
            .add_systems(Startup, (spawn_camera, sound::load));
            app
        }
    };
    lit_plugin(&mut app);
    app.run();
}

fn play_sound(
    mut commands: Commands,
    mut world: ResMut<World>,
    bank: Option<Res<sound::Bank>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Transform, &Projection), With<IsDefaultUiCamera>>,
) {
    let Some(tick) = world.score.take() else {
        return;
    };
    let Some(bank) = bank.filter(|bank| bank.unlocked) else {
        return;
    };
    let (transform, projection) = camera.into_inner();
    let Some(view) = Viewport::of(&window, transform, projection) else {
        return;
    };
    sound::play(&mut commands, &bank, &sound::score(&tick), view.sound());
}

fn spawn_camera(mut commands: Commands) {
    let mut projection = OrthographicProjection::default_2d();
    projection.scale = MICRO_SCALE;
    commands.spawn((
        Camera2d,
        IsDefaultUiCamera,
        Projection::Orthographic(projection),
        Transform::from_translation(px(FOCUS).extend(0.0)),
    ));
    commands.spawn(card_camera(RenderTarget::default()));
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
    (Button, plate(node))
}

fn plate(node: Node) -> impl Bundle {
    (
        Node {
            border: UiRect::all(Val::Px(1.0)),
            ..node
        },
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

fn text(s: String) -> impl Bundle {
    (
        Text::new(s),
        TextColor(IVORY),
        TextFont::from_font_size(15.0),
    )
}

fn picture(entry: &mut ChildSpawnerCommands, kiln: &Kiln, item: Item) {
    let (square, field) = picture_square(item);
    match item {
        Item::Machine(machine) => {
            let skin = look::machine(machine).skin;
            entry.spawn((ImageNode::new(kiln.image(skin)), square, field));
        }
        Item::Atom(kind) => {
            entry.spawn((
                AtomPreview(kind),
                ImageNode::new(kiln.atom(kind)),
                square,
                field,
            ));
        }
        Item::Step => {
            entry
                .spawn((square, field))
                .with_child(mark(0, STEP_PX, true));
        }
        Item::Token(instr) => {
            let skin = key_of(instr).symbol;
            entry.spawn((ImageNode::new(kiln.image(skin)), square, field));
        }
    }
}

fn picture_square(item: Item) -> (Node, BackgroundColor) {
    let side = match item {
        Item::Machine(_) => PALETTE_PX,
        Item::Atom(_) => PALETTE_PX,
        Item::Step | Item::Token(_) => SYMBOL_PX,
    };
    let world = matches!(item, Item::Machine(_) | Item::Atom(_));
    (
        Node {
            width: Val::Px(side),
            height: Val::Px(side),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border_radius: if world {
                BorderRadius::MAX
            } else {
                BorderRadius::default()
            },
            ..default()
        },
        BackgroundColor(if world {
            Glaze::Clay.color()
        } else {
            Color::NONE
        }),
    )
}

#[derive(Component)]
struct Refusal {
    shown: Option<Refused>,
}

fn refusal(
    mut commands: Commands,
    kiln: Res<Kiln>,
    world: Res<World>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Transform, &Projection), With<IsDefaultUiCamera>>,
    line: Single<(Entity, &mut Refusal, &mut Node, &mut Visibility)>,
) {
    let (entity, mut line, mut node, mut vis) = line.into_inner();
    let (transform, projection) = camera.into_inner();
    let Some(viewport) = Viewport::of(&window, transform, projection) else {
        return;
    };
    let Some(refused) = &world.refused else {
        *vis = Visibility::Hidden;
        return;
    };
    *vis = Visibility::Inherited;
    let at = viewport.screen(px(refused.at)) + Vec2::splat(HEX / viewport.scale);
    node.left = Val::Px(at.x);
    node.top = Val::Px(at.y);
    if line.shown.as_ref() == Some(refused) {
        return;
    }
    line.shown = Some(refused.clone());
    commands
        .entity(entity)
        .despawn_children()
        .with_children(|line| {
            for short in &refused.short {
                picture(line, &kiln, short.item);
                line.spawn(row(2.0))
                    .with_children(|marks| stock(marks, short.have, short.need));
            }
        });
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

fn mark(k: u64, px: f32, filled: bool) -> impl Bundle {
    let gap = if k > 0 && k.is_multiple_of(5) {
        MARK_PX
    } else {
        0.0
    };
    (
        Node {
            width: Val::Px(px),
            height: Val::Px(px),
            margin: UiRect::left(Val::Px(gap)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::MAX,
            ..default()
        },
        BackgroundColor(if filled { IVORY } else { Color::NONE }),
        BorderColor::all(if filled { IVORY } else { brass(0.5) }),
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

#[derive(Component)]
struct Palette;

#[derive(Component)]
struct Tally {
    item: Item,
    shown: Option<(u32, u32)>,
}

fn tally(mut commands: Commands, world: Res<World>, mut rows: Query<(Entity, &mut Tally)>) {
    for (entity, mut tally) in &mut rows {
        let inventory = &world.sim.inventory;
        let Some(count) = inventory.count(tally.item) else {
            continue;
        };
        let cap = if world.hover == Some(tally.item) {
            inventory.cap(tally.item).unwrap_or(0)
        } else {
            0
        };
        if tally.shown == Some((count, cap)) {
            continue;
        }
        tally.shown = Some((count, cap));
        commands
            .entity(entity)
            .despawn_children()
            .with_children(|row| stock(row, count, cap));
    }
}

fn stock(row: &mut ChildSpawnerCommands, filled: u32, upto: u32) {
    for k in 0..u64::from(filled.max(upto).min(sim::MAX_CAP)) {
        row.spawn(mark(k, MARK_PX, k < u64::from(filled)));
    }
}

fn hover(
    mut world: ResMut<World>,
    window: Single<&Window, With<PrimaryWindow>>,
    rows: Query<(&PaletteRow, &Interaction)>,
    scroll: Res<AccumulatedMouseScroll>,
    mut left: MessageReader<CursorLeft>,
) {
    if left.read().next().is_some() {
        world.hover = None;
    }
    if window.cursor_position().is_some() {
        world.hover = rows
            .iter()
            .find(|(_, i)| **i != Interaction::None)
            .map(|(row, _)| row.0);
    }
    let notches = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    };
    if let (Some(item), true) = (world.hover, notches.round() != 0.0) {
        world.set_cap(item, notches.round() as i32);
    }
}

const RECIPE_BOUND: Machine = Machine::Glyph(GlyphKind::Output(sim::Tier::One));

fn recipe_side() -> f32 {
    look::quad(RECIPE_BOUND).side
}

fn picture_side(item: Item) -> f32 {
    match item {
        Item::Machine(machine) => look::quad(machine).side,
        Item::Atom(_) => PALETTE_PX,
        Item::Step | Item::Token(_) => SYMBOL_PX,
    }
}

struct Layout {
    size: Vec2,
    picture: Vec2,
    recipe: Vec2,
    field: Option<Vec2>,
}

fn layout(item: Item) -> Layout {
    let picture = picture_side(item);
    let recipe = recipe_side();
    let bounds = match item {
        Item::Machine(machine) => Some(play_bounds(&playfield(machine))),
        Item::Atom(_) | Item::Step | Item::Token(_) => None,
    };
    let span = bounds.map_or(Vec2::ZERO, |(lo, hi)| hi - lo);
    let width = 3.0 * CARD_PAD + picture + recipe + bounds.map_or(0.0, |_| CARD_PAD + span.x);
    let size = Vec2::new(width, 2.0 * CARD_PAD + picture.max(recipe).max(span.y));
    let left = -size.x / 2.0 + CARD_PAD;
    let recipe_at = Vec2::new(left + picture + CARD_PAD + recipe / 2.0, 0.0);
    Layout {
        size,
        picture: Vec2::new(left + picture / 2.0, 0.0),
        recipe: recipe_at,
        field: bounds
            .map(|(lo, _)| Vec2::new(recipe_at.x + recipe / 2.0 + CARD_PAD, -span.y / 2.0) - lo),
    }
}

fn card_size(item: Item) -> Vec2 {
    layout(item).size
}

fn card(
    world: Res<World>,
    window: Single<&Window, With<PrimaryWindow>>,
    column: Single<(&ComputedNode, &UiGlobalTransform), With<Palette>>,
    camera: Single<&mut Camera, With<CardCamera>>,
) {
    let mut camera = camera.into_inner();
    let Some(item) = world.hover else {
        camera.is_active = false;
        return;
    };
    let (node, transform) = column.into_inner();
    let scale = window.scale_factor();
    let size = card_size(item) * scale / CARD_SCALE;
    let top_left = transform.translation - node.size() / 2.0;
    let target = window.physical_size().as_vec2();
    let size = size.min(target);
    let at = Vec2::new(top_left.x, top_left.y - CARD_PAD * scale - size.y)
        .clamp(Vec2::ZERO, target - size);
    camera.viewport = Some(bevy::camera::Viewport {
        physical_position: at.as_uvec2(),
        physical_size: size.as_uvec2(),
        ..default()
    });
    camera.is_active = true;
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
    let (machines, consumables): (Vec<Item>, Vec<Item>) =
        palette().partition(|item| matches!(item, Item::Machine(_)));
    commands
        .spawn((
            Palette,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(8.0),
                bottom: Val::Px(8.0),
                align_items: AlignItems::FlexEnd,
                ..row(8.0)
            },
        ))
        .with_children(|palette| {
            for items in [machines, consumables] {
                palette
                    .spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        ..default()
                    })
                    .with_children(|col| {
                        for item in items {
                            col.spawn((
                                PaletteRow(item),
                                button(Node {
                                    padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
                                    ..row(6.0)
                                }),
                            ))
                            .with_children(|entry| {
                                picture(entry, &kiln, item);
                                entry.spawn((
                                    Tally { item, shown: None },
                                    Node {
                                        width: Val::Px(TALLY_PX),
                                        flex_wrap: FlexWrap::Wrap,
                                        column_gap: Val::Px(2.0),
                                        row_gap: Val::Px(2.0),
                                        ..default()
                                    },
                                ));
                            });
                        }
                    });
            }
        });
    commands.spawn((
        Refusal { shown: None },
        plate(Node {
            position_type: PositionType::Absolute,
            padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
            ..row(6.0)
        }),
        Visibility::Hidden,
        GlobalZIndex(1),
    ));
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(8.0),
            top: Val::Px(8.0),
            ..row(6.0)
        })
        .with_children(|row| {
            for (action, up) in [(SaveAction::Export, false), (SaveAction::Import, true)] {
                row.spawn((
                    action,
                    button(Node {
                        width: Val::Px(34.0),
                        height: Val::Px(34.0),
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::Center,
                        ..default()
                    }),
                ))
                .with_children(|button| save_icon(button, up));
            }
        });
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(PALETTE_WIDTH + 16.0),
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
                        ..row(4.0)
                    }),
                    Visibility::Hidden,
                ));
            }
        });
}

fn save_icon(button: &mut ChildSpawnerCommands, up: bool) {
    let edge = if up { Val::Px(5.0) } else { Val::Auto };
    let opposite = if up { Val::Auto } else { Val::Px(5.0) };
    button
        .spawn(Node {
            position_type: PositionType::Relative,
            width: Val::Px(18.0),
            height: Val::Px(22.0),
            ..default()
        })
        .with_children(|icon| {
            icon.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(8.0),
                    top: edge,
                    bottom: opposite,
                    width: Val::Px(2.0),
                    height: Val::Px(12.0),
                    ..default()
                },
                BackgroundColor(IVORY),
            ));
            icon.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(4.0),
                    top: if up { Val::Px(5.0) } else { Val::Px(15.0) },
                    width: Val::Px(10.0),
                    height: Val::Px(2.0),
                    ..default()
                },
                BackgroundColor(IVORY),
            ));
            icon.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(2.0),
                    bottom: Val::Px(1.0),
                    width: Val::Px(14.0),
                    height: Val::Px(5.0),
                    border: UiRect::axes(Val::Px(2.0), Val::Px(2.0)),
                    ..default()
                },
                BorderColor::all(IVORY),
            ));
        });
}

fn run_ticks(mut world: ResMut<World>, time: Res<Time>) {
    world.advance(time.delta_secs());
}

const HOLD: u64 = 3;

#[derive(Clone, Debug, PartialEq)]
struct Play {
    machine: Machine,
    at: u64,
    since: f32,
    frames: Vec<Sim>,
    events: Vec<sim::TickEvents>,
}

impl Play {
    fn at(machine: Machine, at: u64) -> Play {
        let fixture = fixture(machine);
        let mut sim = fixture.sim;
        let mut frames = vec![sim.clone()];
        let mut events = Vec::new();
        for _ in 0..fixture.ticks {
            events.push(sim.step());
            frames.push(sim.clone());
        }
        Play {
            machine,
            at: at.min(fixture.ticks),
            since: 0.0,
            frames,
            events,
        }
    }

    fn step(&mut self) {
        let ticks = self.events.len() as u64;
        self.at = (self.at + 1) % (ticks + HOLD + 1);
    }

    fn sims(&self) -> (Sim, Sim) {
        let Fixture { done, .. } = fixture(self.machine);
        let ticks = self.events.len() as u64;
        let shown = self.at.min(ticks);
        let sim = self.frames[shown as usize].clone();
        debug_assert!(shown < ticks || done(&sim), "{:?}", self.machine);
        let prev = if self.at > ticks || shown == 0 {
            sim.clone()
        } else {
            self.frames[shown as usize - 1].clone()
        };
        (prev, sim)
    }
}

fn view(
    buttons: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    motion: Res<AccumulatedMouseMotion>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&mut Transform, &mut Projection), With<IsDefaultUiCamera>>,
    palette: Query<&Interaction, With<PaletteRow>>,
) {
    let (mut transform, mut projection) = camera.into_inner();
    let Some(mut viewport) = Viewport::of(&window, &transform, &projection) else {
        return;
    };
    let Projection::Orthographic(ortho) = &mut *projection else {
        return;
    };
    if scroll.delta.y != 0.0
        && palette.iter().all(|i| *i == Interaction::None)
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
    camera: Single<(&Transform, &Projection), With<IsDefaultUiCamera>>,
    ui: Query<(Option<&PaletteRow>, Option<&TapeRow>, &Interaction)>,
    save: Query<&Interaction, With<SaveAction>>,
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
        || ui.iter().any(|(_, _, i)| *i != Interaction::None)
        || save.iter().any(|i| *i != Interaction::None);
    let at = world.pointer.map(hex_at);

    if buttons.just_pressed(MouseButton::Left) {
        let pressed = ui.iter().find(|(_, _, i)| **i == Interaction::Pressed);
        if let Some((Some(entry), _, _)) = pressed {
            world.lift_inventory(entry.0);
        } else if let Some((_, Some(row), _)) = pressed {
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

fn persistence(
    mut world: ResMut<World>,
    mut saved: ResMut<Saved>,
    actions: Query<(&SaveAction, &Interaction), Changed<Interaction>>,
) {
    for (action, interaction) in &actions {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            SaveAction::Export if world.saveable() => persist::download(&world.sim),
            SaveAction::Export => persist::download(&world.snapshot()),
            SaveAction::Import => persist::choose(),
        }
    }
    if let Some(sim) = persist::take() {
        *world = World::new(sim);
    }
    if world.saveable() && saved.attempted != world.sim {
        saved.store(world.sim.clone());
    } else if !world.saveable() {
        let sim = world.snapshot();
        if saved.attempted != sim {
            saved.store(sim);
        }
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
    camera: Single<(&Transform, &Projection), With<IsDefaultUiCamera>>,
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
                    strip.spawn(mark(k, MARK_PX, true));
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
                strip.spawn(text(format!("{arm:<3}{stalled}")));
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
    card: [Handle<ColorMaterial>; 2],
    atoms: [Handle<Image>; 3],
    skins: Vec<(Skin, Handle<Image>, [Handle<ColorMaterial>; 2])>,
    lit: Vec<(Skin, [Handle<Lit>; 4])>,
}

impl Kiln {
    fn atom(&self, kind: sim::AtomKind) -> Handle<Image> {
        self.atoms[atom_index(kind)].clone()
    }
}

#[derive(Clone, Copy, Component)]
struct AtomPreview(sim::AtomKind);

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
    #[texture(5)]
    #[sampler(6)]
    emissive: Handle<Image>,
    #[uniform(7)]
    response: Vec4,
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

    fn lit(&self, skin: Skin, energy: sim::ActivationEnergy) -> &Handle<Lit> {
        self.lit
            .iter()
            .find(|(s, _)| *s == skin)
            .map(|(_, lit)| &lit[energy.level()])
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
) {
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
    let lit = Machine::ALL
        .into_iter()
        .flat_map(|item| rig::parts(item).iter().map(move |part| (item, part)))
        .map(|(item, part)| {
            let (skin, normal, emissive) = look::rig(item, &part.name);
            let lit = std::array::from_fn(|level| {
                lits.add(Lit {
                    light: look::light().extend(look::AMBIENT),
                    albedo: image(skin),
                    relief: image(normal),
                    emissive: image(emissive),
                    response: Vec4::new(
                        level as f32 / sim::ActivationEnergy::FULL.level() as f32,
                        Glaze::Amber.rgb()[0],
                        Glaze::Amber.rgb()[1],
                        Glaze::Amber.rgb()[2],
                    ),
                })
            });
            (skin, lit)
        })
        .collect();
    let atoms = sim::AtomKind::ALL.map(|kind| {
        let image = images.add(Image::new_target_texture(
            PALETTE_PX as u32,
            PALETTE_PX as u32,
            TextureFormat::Rgba8UnormSrgb,
            None,
        ));
        let mut projection = OrthographicProjection::default_2d();
        projection.scale = CARD_SCALE;
        commands.spawn((
            AtomPreview(kind),
            Camera2d,
            Camera {
                order: -1,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            Projection::Orthographic(projection),
            RenderTarget::Image(image.clone().into()),
            atom_layer(kind),
        ));
        image
    });
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
        card: [strip(false), brass(0.5)].map(|c| materials.add(c)),
        atoms,
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

fn phase(since: f32, period: f32, motion: f32) -> f32 {
    let span = period * motion;
    if span > 0.0 { since / span } else { 1.0 }
}

#[derive(Default, Reflect, GizmoConfigGroup)]
struct CardGizmos;

fn tile(kiln: &Kiln, h: Hex) -> (Mesh2d, MeshMaterial2d<ColorMaterial>, Transform) {
    (
        Mesh2d(kiln.hexagon.clone()),
        MeshMaterial2d(kiln.skin(look::tile(h).skin, false).clone()),
        Transform {
            translation: px(h).extend(0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::splat(HEX),
        },
    )
}

struct Painter<'a, 'gw, 'gs, 'cw, 'cs, G: GizmoConfigGroup = DefaultGizmoConfigGroup> {
    gizmos: &'a mut Gizmos<'gw, 'gs, G>,
    commands: &'a mut Commands<'cw, 'cs>,
    kiln: &'a Kiln,
    ghost: bool,
    layers: RenderLayers,
    shift: Vec2,
}

impl<'a, G: GizmoConfigGroup> Painter<'a, '_, '_, '_, '_, G> {
    fn shifted(&mut self, by: Vec2, draw: impl FnOnce(&mut Self)) {
        let was = std::mem::replace(&mut self.shift, by);
        draw(self);
        self.shift = was;
    }

    fn tile(&mut self, h: Hex, z: f32) {
        let (mesh, material, mut transform) = tile(self.kiln, h);
        transform.translation += self.shift.extend(z);
        self.commands
            .spawn((Fill, self.layers.clone(), mesh, material, transform));
    }

    fn outline(&mut self, at: Vec2, size: f32) {
        self.gizmos
            .linestrip_2d(corners(at + self.shift, size), IVORY);
    }

    fn ring(&mut self, at: Vec2, r: f32) {
        let ivory = self.line(IVORY);
        self.gizmos.circle_2d(at + self.shift, r, ivory);
    }

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
            self.layers.clone(),
            Mesh2d(mesh.clone()),
            MeshMaterial2d(material.clone()),
            Transform {
                translation: (at + self.shift).extend(z),
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
        let iso = Isometry2d::new(at + self.shift, Rot2::radians(turn));
        let color = self.line(glaze.color());
        self.gizmos.arc_2d(iso, 3.0 * FRAC_PI_2, r, color);
    }

    fn rig(
        &mut self,
        item: Machine,
        origin: Vec2,
        angle: f32,
        z: f32,
        response: (bool, f32, sim::ActivationEnergy),
    ) {
        let quad = look::quad(item);
        let kiln = self.kiln;
        let pulse = rig::pulse(response.0, response.1);
        for (index, part) in rig::parts(item).iter().enumerate() {
            let (skin, _, _) = look::rig(item, &part.name);
            let pivot = Vec2::new(part.pivot[0], part.pivot[1]) * HEX;
            let (shift, turn, scale) = match part.motion {
                Some(rig::Motion::Clamp) => (Vec2::new(-0.16 * HEX * pulse, 0.0), 0.0, 1.0),
                Some(rig::Motion::Turn) => (Vec2::ZERO, 0.35 * pulse, 1.0),
                Some(rig::Motion::Dilate) => (Vec2::ZERO, 0.0, 1.0 + 0.12 * pulse),
                None => (Vec2::ZERO, 0.0, 1.0),
            };
            let local = Vec2::from_angle(turn).rotate(quad.centre - pivot) + pivot + shift;
            let at = origin + Vec2::from_angle(angle).rotate(local);
            let part_z = z + index as f32 * 0.0001;
            if self.ghost {
                self.fill(
                    &kiln.bar,
                    self.skin(skin),
                    at,
                    angle + turn,
                    quad.size() * scale,
                    part_z,
                );
            } else {
                self.fill(
                    &kiln.bar,
                    kiln.lit(skin, response.2),
                    at,
                    angle + turn,
                    quad.size() * scale,
                    part_z,
                );
            }
        }
    }

    fn arm(
        &mut self,
        pivot: Vec2,
        hand: Vec2,
        ring: f32,
        look: Look<MachineMark>,
        z: f32,
        motion: (bool, f32, sim::ActivationEnergy),
    ) {
        self.rig(Machine::Arm, pivot, (hand - pivot).to_angle(), z, motion);
        match look.marking {
            MachineMark::Hand(glaze, _) => self.horseshoe(hand, HEX * ring, pivot - hand, glaze),
            _ => unworn(look),
        };
    }

    fn machine(
        &mut self,
        item: Machine,
        at: Hex,
        dir: usize,
        z: f32,
        response: (bool, f32, sim::ActivationEnergy),
    ) {
        let look = look::machine(item);
        match (item, look.marking) {
            (Machine::Arm, MachineMark::Hand(_, _)) => {
                let hand = px(at.add(DIRS[dir % 6]));
                self.arm(px(at), hand, RING_OPEN, look, z, response);
            }
            (Machine::Glyph(_), MachineMark::Sprite(_)) => {
                self.rig(item, px(at), look::turn(dir), z, response);
            }
            _ => unworn(look),
        }
    }

    fn particles(
        &mut self,
        machine: Machine,
        index: usize,
        events: &[sim::TickEvent],
        state: (Vec2, sim::ActivationEnergy, f32, f32),
    ) {
        let (at, energy, phase, z) = state;
        let Some((emitter, count)) = particles::burst(machine, index, energy, events) else {
            return;
        };
        let glaze = match emitter.look {
            particles::Look::Spark => Glaze::Amber,
            particles::Look::Steam => Glaze::Ivory,
            particles::Look::Dust => Glaze::Clay,
        };
        let age = (sim::ActivationEnergy::FULL.level() - energy.level()) as f32 + phase;
        for i in 0..count {
            let seed = (index as u32).wrapping_mul(31).wrapping_add(i as u32 * 17);
            let angle = seed as f32 * 2.399_963;
            let distance = HEX * (0.18 + 0.13 * age + 0.035 * i as f32);
            let position = at + Vec2::from_angle(angle) * distance;
            let radius = HEX * (0.045 + 0.012 * ((seed % 3) as f32));
            self.fill(
                &self.kiln.circle,
                self.kiln.material(glaze),
                position,
                0.0,
                Vec2::splat(radius),
                z,
            );
        }
    }
}

mod layer {
    use std::ops::Range;

    pub const GLYPHS: f32 = 0.1;
    pub const BOND: f32 = 0.2;
    pub const ARMS: Range<f32> = 0.28..0.38;
    pub const BEAD: f32 = 0.4;
    pub const RIM: f32 = 0.02;
    pub const HELD: Range<f32> = 0.44..0.5;
    pub const CARD: Range<f32> = 0.6..0.7;
    pub const LIFT: f32 = 0.8;

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
        let t = t.min(1.0);
        if t < self.release {
            let u = t / self.release;
            return self.creep * u * u * (3.0 - 2.0 * u);
        }
        if t < self.arrival() {
            return self.creep.lerp(1.0, (t - self.release) / self.run);
        }
        let speed = (1.0 - self.creep) / self.run;
        let duration = 1.0 - self.arrival();
        let omega = std::f32::consts::PI * self.half_bounces as f32 / duration;
        let after = t - self.arrival();
        let wave = (omega * after).sin() - after / duration * (omega * duration).sin();
        1.0 + speed / omega * (-self.decay * after).exp() * wave
    }
}

fn grip(holding: bool) -> f32 {
    if holding { RING_CLOSED } else { RING_OPEN }
}

fn board(
    mut commands: Commands,
    mut kiln: ResMut<Kiln>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Transform, &Projection), With<IsDefaultUiCamera>>,
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
    match tiling {
        Tiling::Cells { r0, r1, x0, x1 } => {
            for r in r0..=r1 {
                let (q0, q1) = (x0 - r.div_euclid(2) - 2, x1 - r.div_euclid(2) + 2);
                for q in q0..=q1 {
                    commands.spawn((Board, tile(&kiln, Hex::new(q, r))));
                }
            }
        }
        Tiling::Slab(lo, hi) => {
            let (lo, hi) = (lo.as_vec2(), hi.as_vec2());
            commands.spawn((
                Board,
                Mesh2d(kiln.bar.clone()),
                MeshMaterial2d(kiln.material(Glaze::Clay).clone()),
                Transform {
                    translation: ((lo + hi) / 2.0).extend(0.0),
                    scale: (hi - lo).extend(1.0),
                    ..default()
                },
            ));
        }
    }
    kiln.tiled = Some(tiling);
}

fn draw(
    world: Res<World>,
    mut gizmos: Gizmos,
    mut card_gizmos: Gizmos<CardGizmos>,
    mut commands: Commands,
    kiln: Res<Kiln>,
    fills: Query<Entity, With<Fill>>,
    previews: Query<(&AtomPreview, &RenderLayers), With<Camera>>,
) {
    for e in &fills {
        commands.entity(e).despawn();
    }
    let mut p = Painter {
        gizmos: &mut gizmos,
        commands: &mut commands,
        kiln: &kiln,
        ghost: world.ghosts() > 0,
        layers: RenderLayers::default(),
        shift: Vec2::ZERO,
    };
    let f = Frame::between(&world.prev, world.shown(), world.phase());
    let events = world
        .events
        .last()
        .map_or(&[][..], |tick| tick.events.as_slice());
    scene(&mut p, &f, 0.0, events, world.phase(), true);
    for (i, g) in world.shown().glyphs.iter().enumerate() {
        if let Some(g) = g
            && world.picks(Id::Glyph(i))
        {
            p.outline(px(g.at), HEX * 0.9);
        }
    }
    for (i, at) in f.atoms.iter().enumerate() {
        if let Some(at) = at
            && world.picks(Id::Atom(i))
        {
            p.outline(*at, HEX * 0.9);
        }
    }
    for (i, arm) in f.arms.iter().enumerate() {
        if world.picks(Id::Arm(i)) {
            p.outline(arm.pivot, HEX * 0.9);
        }
    }
    p.ghost = false;
    if let Some(pointer) = world.pointer {
        if let Some(Focus::Hold { set, back }) = &world.focus {
            let grab = hex_at(pointer);
            p.outline(px(grab), HEX * 0.9);
            for id in world.sim.blocked(set, grab, back.picked()) {
                for cell in world.sim.stands(id) {
                    p.outline(px(cell), HEX * 0.9);
                }
            }
            let glyphs = set.glyphs.iter().flatten();
            let machines: Vec<(Machine, Hex, usize)> = glyphs
                .map(|g| (Machine::Glyph(g.kind), g.at, g.dir))
                .chain(set.arms.iter().map(|a| (Machine::Arm, a.pivot, a.dir)))
                .collect();
            for (i, (item, at, dir)) in machines.iter().enumerate() {
                let z = layer::z(layer::HELD, i, machines.len());
                p.machine(
                    *item,
                    grab.add(*at),
                    *dir,
                    z,
                    (false, 1.0, sim::ActivationEnergy::default()),
                );
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
                let look = look::machine(Machine::Arm);
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
    for (atom, layers) in &previews {
        let mut preview = Painter {
            gizmos: &mut gizmos,
            commands: &mut commands,
            kiln: &kiln,
            ghost: false,
            layers: layers.clone(),
            shift: Vec2::ZERO,
        };
        preview.bead(Vec2::ZERO, look::atom(atom.0), layer::BEAD);
    }
    if let Some(item) = world.hover {
        let mut p = Painter {
            gizmos: &mut card_gizmos,
            commands: &mut commands,
            kiln: &kiln,
            ghost: false,
            layers: CARD,
            shift: Vec2::ZERO,
        };
        let sims = world.play.as_ref().map(|play| (play, play.sims()));
        let play = sims.as_ref().map(|(play, (prev, sim))| {
            Frame::between(prev, sim, phase(play.since, world.period, world.motion))
        });
        let card_events = world.play.as_ref().and_then(|play| {
            let shown = play.at.min(play.events.len() as u64);
            shown
                .checked_sub(1)
                .and_then(|index| play.events.get(index as usize))
        });
        hover_card(
            &mut p,
            item,
            play.as_ref(),
            card_events.map_or(&[][..], |tick| tick.events.as_slice()),
            world
                .play
                .as_ref()
                .map_or(1.0, |play| phase(play.since, world.period, world.motion)),
        );
    }
}

fn scene<G: GizmoConfigGroup>(
    p: &mut Painter<G>,
    f: &Frame,
    lift: f32,
    events: &[sim::TickEvent],
    phase: f32,
    particles: bool,
) {
    for (index, glyph) in f.sim.glyphs.iter().enumerate() {
        let Some(g) = glyph else { continue };
        let item = Machine::Glyph(g.kind);
        let fired = events
            .iter()
            .any(|event| rig::activation(item).matches(event, index));
        p.machine(
            item,
            g.at,
            g.dir,
            layer::GLYPHS + lift,
            (fired, phase, g.energy),
        );
        if particles {
            let particle_z = match particles::response(item) {
                particles::Response::Default(e) | particles::Response::Rig(e) => match e.layer {
                    particles::Layer::Behind => layer::GLYPHS - 0.01,
                    particles::Layer::On => layer::GLYPHS + 0.01,
                },
            } + lift;
            p.particles(item, index, events, (px(g.at), g.energy, phase, particle_z));
        }
    }
    for b in &f.sim.bonds {
        let (Some(a), Some(c)) = (f.atoms[b.a], f.atoms[b.b]) else {
            continue;
        };
        p.bond(a, c, b.kind, layer::BOND + lift);
    }
    for (at, atom) in f.atoms.iter().zip(&f.sim.atoms) {
        if let (Some(at), Some(atom)) = (at, atom) {
            p.bead(*at, look::atom(atom.kind), layer::BEAD + lift);
        }
    }
    let look = look::machine(Machine::Arm);
    for (i, arm) in f.arms.iter().enumerate() {
        let z = layer::z(layer::ARMS, i, f.arms.len()) + lift;
        let fired = events
            .iter()
            .any(|event| rig::activation(Machine::Arm).matches(event, i));
        p.arm(
            arm.pivot,
            arm.hand,
            arm.ring,
            look,
            z,
            (fired, phase, f.sim.arms[i].energy),
        );
        if particles {
            let particle_z = match particles::response(Machine::Arm) {
                particles::Response::Default(e) | particles::Response::Rig(e) => match e.layer {
                    particles::Layer::Behind => layer::ARMS.start - 0.01,
                    particles::Layer::On => layer::ARMS.end + 0.01,
                },
            } + lift;
            p.particles(
                Machine::Arm,
                i,
                events,
                (arm.pivot, f.sim.arms[i].energy, phase, particle_z),
            );
        }
        let stall = f.sim.arms[i].stall;
        if stall.is_some() {
            p.ring(arm.pivot, HEX * 0.5);
        }
        if let Some(Stall::Hand(j)) = stall {
            p.ring(f.arms[j].hand, HEX * 0.65);
        }
    }
}

fn playfield(machine: Machine) -> Vec<Hex> {
    let sim = fixture(machine).sim;
    let mut cells: Vec<Hex> = sim.ids().flat_map(|id| sim.stands(id)).collect();
    cells.extend(sim.arms.iter().flat_map(|a| DIRS.map(|d| a.pivot.add(d))));
    let mut tiles: Vec<Hex> = cells
        .iter()
        .flat_map(|c| DIRS.iter().map(|d| c.add(*d)).chain([*c]))
        .collect();
    tiles.sort_by_key(|h| (h.r, h.q));
    tiles.dedup();
    tiles
}

fn play_bounds(tiles: &[Hex]) -> (Vec2, Vec2) {
    let reach = Vec2::new(HEX * 3f32.sqrt() / 2.0, HEX);
    tiles
        .iter()
        .map(|h| px(*h))
        .fold((Vec2::INFINITY, Vec2::NEG_INFINITY), |(lo, hi), p| {
            (lo.min(p - reach), hi.max(p + reach))
        })
}

fn hover_card<G: GizmoConfigGroup>(
    p: &mut Painter<G>,
    item: Item,
    play: Option<&Frame>,
    events: &[sim::TickEvent],
    phase: f32,
) {
    let kiln = p.kiln;
    let card = layout(item);
    let z = |k: usize| layer::z(layer::CARD, k, 5);
    p.fill(
        &kiln.bar,
        &kiln.card[1],
        Vec2::ZERO,
        0.0,
        card.size + 2.0,
        z(0),
    );
    p.fill(&kiln.bar, &kiln.card[0], Vec2::ZERO, 0.0, card.size, z(1));
    let at = card.picture;
    match item {
        Item::Machine(machine) => p.rig(
            machine,
            at - look::quad(machine).centre,
            0.0,
            z(2),
            (false, 1.0, sim::ActivationEnergy::default()),
        ),
        Item::Atom(kind) => p.bead(at, look::atom(kind), z(2)),
        Item::Step => p.fill(
            &kiln.circle,
            kiln.material(Glaze::Ivory),
            at,
            0.0,
            Vec2::splat(STEP_PX),
            z(2),
        ),
        Item::Token(instr) => p.fill(
            &kiln.bar,
            p.skin(key_of(instr).symbol),
            at,
            0.0,
            Vec2::splat(picture_side(item)),
            z(2),
        ),
    }
    if let Some(recipe) = item.recipe() {
        compound(p, card.recipe, recipe);
    }
    let (Item::Machine(machine), Some(field), Some(f)) = (item, card.field, play) else {
        return;
    };
    p.shifted(field, |p| {
        for h in playfield(machine) {
            p.tile(h, layer::LIFT);
        }
        scene(p, f, layer::LIFT, events, phase, false);
    });
}

fn compound<G: GizmoConfigGroup>(p: &mut Painter<G>, centre: Vec2, form: &Form) {
    let z = |k: usize| layer::z(layer::CARD, k, 5);
    let set = form.sim();
    let centroid =
        form.atoms().iter().map(|(at, _)| px(*at)).sum::<Vec2>() / form.atoms().len() as f32;
    let at = |id: usize| centre - centroid + px(set.atoms[id].unwrap().pos);
    for b in &set.bonds {
        p.bond(at(b.a), at(b.b), b.kind, z(3));
    }
    for (id, atom) in set.atoms.iter().enumerate() {
        if let Some(atom) = atom {
            p.bead(at(id), look::atom(atom.kind), z(4));
        }
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

    use bevy::image::Image;
    use bevy::input::ButtonState;
    use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
    use bevy::render::RenderPlugin;
    use bevy::render::render_resource::{TextureFormat, TextureUsages};
    use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
    use bevy::time::TimeUpdateStrategy;

    use sim::{Atom, AtomKind, Bond};
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    const WIDE_SCALE: f32 = 1.5;

    const WARM: u32 = 24;
    const FRAME: Duration = Duration::from_nanos(16_666_667);

    #[derive(Clone, Copy)]
    pub enum Act {
        Down(KeyCode),
        Up(KeyCode),
        Press(Hex),
        Drag(Hex),
        Release(Hex),
        Lift(Machine),
        Paste(&'static str),
    }

    fn tap(frame: u32, key: KeyCode) -> [(u32, Act); 2] {
        [(frame, Act::Down(key)), (frame + 1, Act::Up(key))]
    }

    fn carry(f0: u32, path: &[(i32, i32)], turn: Option<usize>) -> Vec<(u32, Act)> {
        let cell = |k: usize| Hex::new(path[k].0, path[k].1);
        let mut acts = vec![(f0, Act::Press(cell(0)))];
        for k in 1..path.len() {
            acts.push((f0 + 6 * k as u32, Act::Drag(cell(k))));
        }
        let last = f0 + 6 * (path.len() as u32 - 1);
        if let Some(k) = turn {
            acts.extend(tap(f0 + 6 * k as u32 + 3, KeyCode::KeyD));
        }
        acts.push((last + 6, Act::Release(cell(path.len() - 1))));
        acts
    }

    pub const SCENES: [&str; 41] = [
        "micro",
        "tab-held",
        "tab-released",
        "texture-micro",
        "texture-wide",
        "wide",
        "board",
        "bonders",
        "focus",
        "write",
        "spend",
        "hand",
        "start",
        "craft",
        "copy",
        "walk",
        "ghost",
        "hold",
        "select",
        "output",
        "bonding",
        "chorus",
        "rotation",
        "delete",
        "overlap",
        "refuse",
        "pivot",
        "twohands",
        "heldeat",
        "heldout",
        "caught",
        "grabnothing",
        "dropfirst",
        "grabfirst",
        "base",
        "converters",
        "converter-sheet",
        "reification",
        "rig",
        "particles",
        "sound",
    ];

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

    #[derive(Clone, Copy)]
    pub enum Frame {
        Micro,
        Wide,
        Recipe(Machine),
    }

    impl Frame {
        fn size(self) -> UVec2 {
            match self {
                Frame::Micro | Frame::Wide => UVec2::new(1280, 720),
                Frame::Recipe(_) => UVec2::splat(machines::canvas(RECIPE_BOUND)),
            }
        }
    }

    #[derive(Resource)]
    pub struct Shot {
        path: PathBuf,
        clip: Option<u32>,
        frame: Frame,
        script: Vec<(u32, Act)>,
        warm: u32,
        frames: u32,
        target: Option<Handle<Image>>,
        moving_view: bool,
    }

    fn second_bond(extra: &[Hex]) -> (Sim, Vec<usize>) {
        let glyph = Glyph::new(GlyphKind::SecondBond, Hex::new(1, -1), 0);
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

    pub fn scene(name: &str, ticks: u64) -> (World, Frame, Vec<(u32, Act)>, u32) {
        use KeyCode::*;
        let mut world = World::new(sim::preloaded());
        world.running = false;
        world.pointer = Some(px(Hex::new(3, -3)));
        let mut keys = Vec::new();
        let mut script = Vec::new();
        let mut frame = Frame::Micro;
        assert!(
            name.contains(':') || SCENES.contains(&name),
            "unknown scene {name}"
        );
        let machine = |name: &str| {
            Machine::ALL
                .into_iter()
                .find(|item| machines::name(*item) == name)
                .unwrap_or_else(|| panic!("unknown machine {name}"))
        };
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
                sim.glyphs
                    .push(Some(Glyph::new(GlyphKind::Bonder, Hex::new(0, 0), 0)));
                world.sim = sim;
                world.focus_tape(0);
            }
            "texture-wide" => {
                let mut sim = Sim::empty();
                for (k, pivot) in sim::PLACEMENTS.into_iter().enumerate() {
                    let dir = k % 6;
                    let away = DIRS[(dir + 2) % 6];
                    let mut arm = Arm::new(pivot, dir, Vec::new());
                    arm.holding = true;
                    sim.spawn(Atom {
                        kind: AtomKind::Base,
                        pos: arm.hand(),
                    });
                    sim.arms.push(arm);
                    sim.glyphs.push(Some(Glyph {
                        kind: GlyphKind::ALL[k % GlyphKind::ALL.len()],
                        at: pivot.add(Hex::new(away.q * 4, away.r * 4)),
                        dir,
                        energy: sim::ActivationEnergy::default(),
                    }));
                }
                world.sim = sim;
                frame = Frame::Wide;
            }
            "wide" => frame = Frame::Wide,
            "board" => world.sim = Sim::empty(),
            "bonders" => world.sim = phased(&[(Hex::new(-3, 0), 16), (Hex::new(3, 0), 18)]),
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
                world.sim.inventory.add(Item::Step);
                world.sim.inventory.add(Item::Token(Instr::Grab));
                script.extend(tap(30, Space));
                script.push((54, Act::Press(arm)));
                script.push((60, Act::Release(arm)));
                script.extend(tap(96, KeyF));
                script.extend(tap(132, KeyG));
            }
            "spend" => {
                let arm = Hex::new(-2, 0);
                let mut sim = Sim::empty();
                sim.arms.push(Arm::new(arm, 0, Vec::new()));
                sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: arm.add(DIRS[0]),
                });
                for _ in 0..3 {
                    sim.inventory.add(Item::Step);
                }
                sim.inventory.add(Item::Token(Instr::Grab));
                world.sim = sim;
                script.extend(tap(30, Space));
                for k in 0..4 {
                    script.extend(tap(60 + 24 * k, KeyG));
                }
                script.push((170, Act::Press(arm)));
                script.push((176, Act::Release(arm)));
                script.extend(tap(200, KeyF));
                script.extend(tap(240, Backspace));
            }
            "hand" => {
                let source = Hex::new(-4, 1);
                let mut sim = Sim::empty();
                sim.glyphs
                    .push(Some(Glyph::new(GlyphKind::Source, source, 0)));
                sim.glyphs
                    .push(Some(Glyph::new(GlyphKind::Bonder, Hex::new(-1, 1), 0)));
                sim.glyphs
                    .push(Some(Glyph::new(GlyphKind::SecondBond, Hex::new(2, 0), 1)));
                sim.glyphs.push(Some(Glyph::new(
                    GlyphKind::Output(sim::Tier::One),
                    Hex::new(-1, -3),
                    1,
                )));
                sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: source,
                });
                world.sim = sim;
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
            "start" => world.sim = sim::start(),
            "converters" => {
                let mut sim = Sim::empty();
                sim.place(
                    &sim::fixture(Machine::Glyph(GlyphKind::Converter(AtomKind::Amber))).sim,
                    Hex::new(-2, 0),
                );
                sim.place(
                    &sim::fixture(Machine::Glyph(GlyphKind::Converter(AtomKind::Plum))).sim,
                    Hex::new(2, 0),
                );
                world.sim = sim;
            }
            "rig" | "particles" => {
                let mut sim = Sim::empty();
                let bonder = Glyph::new(GlyphKind::Bonder, Hex::new(-2, 0), 0);
                sim.glyphs.push(Some(bonder));
                for pos in bonder.slots() {
                    sim.spawn(Atom {
                        kind: AtomKind::Base,
                        pos,
                    });
                }
                let (applicator, _) = second_bond(&[]);
                sim.place(&applicator, Hex::new(3, 0));
                sim.glyphs
                    .push(Some(Glyph::new(GlyphKind::Source, Hex::new(-5, 1), 0)));
                if name == "particles" {
                    sim.arms
                        .push(Arm::new(Hex::new(0, 3), 0, vec![Instr::Grab]));
                }
                world.sim = sim;
                world.prev = world.sim.clone();
                world.period = TICK_MS / 1000.0;
            }
            "sound" => {
                let mut sim = Sim::empty();
                sim.place(
                    &sim::fixture(Machine::Glyph(GlyphKind::Bonder)).sim,
                    Hex::new(-8, 0),
                );
                let shared = Hex::new(8, 0);
                let left = Arm::new(Hex::new(7, 0), 0, vec![Instr::Grab, Instr::Drop]);
                let mut right = Arm::new(Hex::new(9, 0), 3, vec![Instr::Drop, Instr::Grab]);
                right.holding = true;
                sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: shared,
                });
                sim.arms.push(left);
                sim.arms.push(right);
                world.sim = sim;
                world.prev = world.sim.clone();
                world.period = TICK_MS / 1000.0;
            }
            "converter-sheet" => {
                let mut sim = Sim::empty();
                for (kind, at) in [
                    (AtomKind::Base, Hex::new(-2, -1)),
                    (AtomKind::Amber, Hex::new(0, -1)),
                    (AtomKind::Plum, Hex::new(2, -1)),
                ] {
                    sim.spawn(Atom { kind, pos: at });
                }
                sim.glyphs.push(Some(Glyph::new(
                    GlyphKind::Converter(AtomKind::Amber),
                    Hex::new(-1, 1),
                    0,
                )));
                sim.glyphs.push(Some(Glyph::new(
                    GlyphKind::Converter(AtomKind::Plum),
                    Hex::new(2, 1),
                    0,
                )));
                world.sim = sim;
                world.period = f32::INFINITY;
            }
            "craft" | "copy" => {
                let mut sim = sim::start();
                sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: Hex::new(-4, 1),
                });
                world.sim = sim;
                script.extend(carry(20, &[(-4, 1), (-3, 1), (-2, 1), (-1, 1)], None));
                script.extend(carry(
                    56,
                    &[(-4, 1), (-3, 1), (-2, 1), (-1, 1), (0, 1)],
                    None,
                ));
                if name == "craft" {
                    script.extend(carry(110, &[(-1, 1), (0, 0), (1, -1), (1, -2)], None));
                    let bonder = Machine::Glyph(GlyphKind::Bonder);
                    script.push((190, Act::Lift(bonder)));
                    script.push((196, Act::Drag(Hex::new(-3, -3))));
                    script.push((202, Act::Drag(Hex::new(-2, -3))));
                    script.push((214, Act::Release(Hex::new(-2, -3))));
                } else {
                    script.extend(carry(
                        110,
                        &[(-1, 1), (-2, 0), (-3, -1), (-3, -2), (-3, -3)],
                        None,
                    ));
                    script.push((160, Act::Press(Hex::new(-5, -2))));
                    script.push((166, Act::Drag(Hex::new(-4, -3))));
                    script.push((178, Act::Drag(Hex::new(-2, -4))));
                    script.push((184, Act::Release(Hex::new(-2, -4))));
                    script.extend(tap(200, KeyCode::KeyC));
                }
            }
            name if name.starts_with("card:") => {
                let machine = machine(&name[5..]);
                world.sim = Sim::empty();
                world.period = f32::INFINITY;
                world.motion = 0.0;
                world.hover = Some(Item::Machine(machine));
                world.play = Some(Play::at(machine, ticks));
            }
            name if name.starts_with("recipe:") => {
                let item = machine(&name[7..]);
                assert!(item.recipe().is_some(), "{name} has no recipe");
                world.sim = Sim::empty();
                frame = Frame::Recipe(item);
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
                    for _ in 0..5 {
                        world.sim.inventory.add(Item::Step);
                    }
                    world.sim.inventory.add(Item::Token(Instr::Rot(Spin::Cw)));
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
                world.lift(
                    fresh(Item::Machine(Machine::Glyph(GlyphKind::Bonder))),
                    Back::Inventory,
                );
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
                for (k, (_, recipe)) in recipes().iter().take(3).enumerate() {
                    let at = Hex::new(k as i32 * 4 - 4, -1);
                    sim.glyphs.push(Some(Glyph {
                        kind: GlyphKind::Output(sim::Tier::One),
                        at,
                        dir: k,
                        energy: sim::ActivationEnergy::default(),
                    }));
                    sim.place(&recipe.sim(), at.sub(recipe.centre(1).unwrap()));
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
                        energy: sim::ActivationEnergy::default(),
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
            "delete" => {
                let mut sim = Sim::empty();
                let chain: Vec<usize> = [DIRS[3], ORIGIN, DIRS[0]]
                    .iter()
                    .map(|pos| {
                        sim.spawn(Atom {
                            kind: AtomKind::Base,
                            pos: *pos,
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
                world.sim = sim;
                script.extend(tap(30, Space));
                script.push((54, Act::Press(ORIGIN)));
                script.push((60, Act::Release(ORIGIN)));
                script.extend(tap(96, KeyZ));
            }
            "overlap" => {
                let mut sim = Sim::empty();
                sim.glyphs
                    .push(Some(Glyph::new(GlyphKind::Bonder, Hex::new(-3, 0), 0)));
                sim.glyphs
                    .push(Some(Glyph::new(GlyphKind::SecondBond, Hex::new(1, 0), 1)));
                world.sim = sim;
                script.extend(carry(30, &[(-3, 0), (-2, 0), (-1, 0), (0, 0)], None));
                script.extend(carry(96, &[(-3, 0), (-2, 0), (-1, 0)], None));
            }
            "base" => {
                let mut sim = Sim::empty();
                sim.glyphs
                    .push(Some(Glyph::new(GlyphKind::Bonder, Hex::new(-2, 0), 0)));
                sim.arms.push(Arm::new(Hex::new(2, 0), 0, Vec::new()));
                world.sim = sim;
                script.extend(carry(30, &[(2, 0), (1, 0), (0, 0), (-1, 0)], None));
                script.extend(carry(96, &[(2, 0), (1, 0), (0, 0)], None));
            }
            "refuse" => {
                let mut sim = Sim::empty();
                sim.glyphs
                    .push(Some(Glyph::new(GlyphKind::Bonder, Hex::new(-2, 0), 0)));
                sim.arms
                    .push(Arm::new(Hex::new(1, 0), 0, vec![Instr::Grab]));
                sim.inventory.add(Item::Machine(Machine::Arm));
                sim.inventory.add(Item::Token(Instr::Grab));
                world.sim = sim;
                script.extend(tap(20, Space));
                script.push((30, Act::Press(Hex::new(-5, -3))));
                script.push((36, Act::Drag(Hex::new(0, 1))));
                script.push((42, Act::Drag(Hex::new(4, 3))));
                script.push((48, Act::Release(Hex::new(4, 3))));
                script.extend(tap(60, KeyC));
                script.extend(tap(72, KeyV));
                script.push((96, Act::Press(Hex::new(2, -4))));
                script.push((150, Act::Press(Hex::new(-2, 0))));
                script.extend(tap(168, KeyZ));
                script.extend(tap(192, KeyV));
                script.push((216, Act::Press(Hex::new(2, -4))));
            }
            "reification" => {
                let machine = Machine::Glyph(GlyphKind::Reification);
                let form = machine.recipe().unwrap();
                let centre = form.centre(sim::Tier::Two.radius()).unwrap();
                let mut sim = Sim::empty();
                sim.place(&form.sim(), Hex::new(-4, 0).sub(centre));
                let centre_atom = sim.atom_at(Hex::new(-4, 0)).unwrap();
                sim.atoms[centre_atom].as_mut().unwrap().kind = AtomKind::Amber;
                sim.glyphs
                    .push(Some(Glyph::new(GlyphKind::Reification, Hex::new(-4, 0), 0)));
                sim.inventory.add(Item::Atom(AtomKind::Base));
                sim.inventory.add(Item::Atom(AtomKind::Base));
                world.sim = sim;
                script.push((52, Act::Paste("B0,0 B1,0 0,0-1,0")));
                script.push((58, Act::Press(Hex::new(3, 0))));
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
                sim.glyphs.push(Some(Glyph::new(
                    GlyphKind::Output(sim::Tier::One),
                    Hex::new(2, -1),
                    0,
                )));
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
        (world, frame, script, warm)
    }

    pub fn recipe(item: Machine, path: &Path) {
        #[cfg(test)]
        let _render = crate::RENDER_TEST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (world, frame, script, warm) = scene(&format!("recipe:{}", machines::name(item)), 0);
        let shot = Shot {
            path: path.to_path_buf(),
            clip: None,
            frame,
            script,
            warm,
            frames: 0,
            target: None,
            moving_view: false,
        };
        let mut app = app(world, shot);
        lit_plugin(&mut app);
        assert_eq!(app.run(), AppExit::Success, "{}", path.display());
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
        let (mut world, frame, script, warm) = scene(view, ticks.parse().expect(USAGE));
        let clip = clip.map(|(play, tick_ms, motion)| {
            world.period = tick_ms / 1000.0;
            world.motion = motion;
            (play * world.period / FRAME.as_secs_f32()).round() as u32
        });
        let shot = Shot {
            path: PathBuf::from(path),
            clip,
            frame,
            script,
            warm,
            frames: 0,
            target: None,
            moving_view: view == "sound",
        };
        Some((world, shot))
    }

    pub fn sound(args: &[String]) -> Option<i32> {
        const USAGE: &str = "usage: ziral --sound-proof <wav> <score> <ticks>";
        let [_, flag, wav, score, ticks] = args else {
            return None;
        };
        if flag != "--sound-proof" {
            return None;
        }
        let (mut world, _, _, _) = scene("sound", 0);
        let count = ticks.parse().expect(USAGE);
        let ticks = (0..count)
            .map(|tick| {
                (
                    world.sim.step(),
                    sound_view(tick as f32 / count.max(1) as f32),
                )
            })
            .collect::<Vec<_>>();
        let (text, audio) = crate::sound::proof(&ticks);
        std::fs::write(wav, audio).unwrap_or_else(|error| panic!("{wav}: {error}"));
        std::fs::write(score, text).unwrap_or_else(|error| panic!("{score}: {error}"));
        Some(0)
    }

    #[cfg(test)]
    pub fn still(view: &str, dir: PathBuf, frames: u32) -> App {
        let (mut world, frame, script, warm) = scene(view, 0);
        world.period = f32::INFINITY;
        let shot = Shot {
            path: dir,
            clip: Some(frames),
            frame,
            script,
            warm,
            frames: 0,
            target: None,
            moving_view: false,
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
            .add_systems(Update, move_sound_view.before(view))
            .add_systems(Update, capture.after(run_ticks).before(draw))
            .add_systems(Update, recipe_frame.after(draw));
        app
    }

    fn recipe_frame(shot: Res<Shot>, mut gizmos: Gizmos, mut commands: Commands, kiln: Res<Kiln>) {
        let Frame::Recipe(item) = shot.frame else {
            return;
        };
        let mut p = Painter {
            gizmos: &mut gizmos,
            commands: &mut commands,
            kiln: &kiln,
            ghost: false,
            layers: CARD,
            shift: Vec2::ZERO,
        };
        let recipe = item
            .recipe()
            .expect("a recipe frame draws a machine that has one");
        compound(&mut p, Vec2::ZERO, recipe);
    }

    fn spawn_offscreen_camera(
        mut commands: Commands,
        mut images: ResMut<Assets<Image>>,
        mut shot: ResMut<Shot>,
    ) {
        let size = shot.frame.size();
        let mut image =
            Image::new_target_texture(size.x, size.y, TextureFormat::Rgba8UnormSrgb, None);
        image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let handle = images.add(image);
        let mut projection = OrthographicProjection::default_2d();
        let center = match shot.frame {
            Frame::Wide => {
                projection.scale = WIDE_SCALE;
                let pivots: Vec<Vec2> = sim::PLACEMENTS.iter().map(|h| px(*h)).collect();
                pivots.iter().sum::<Vec2>() / pivots.len() as f32
            }
            Frame::Micro | Frame::Recipe(_) => {
                projection.scale = MICRO_SCALE;
                px(FOCUS)
            }
        };
        let recipe = matches!(shot.frame, Frame::Recipe(_));
        if recipe {
            commands.spawn(card_view(
                RenderTarget::Image(handle.clone().into()),
                1.0 / machines::scale(),
                true,
            ));
        }
        commands.spawn((
            Camera2d,
            Camera {
                is_active: !recipe,
                ..default()
            },
            Projection::Orthographic(projection),
            Transform::from_translation(center.extend(0.0)),
            RenderTarget::Image(handle.clone().into()),
            IsDefaultUiCamera,
        ));
        commands.spawn(card_camera(RenderTarget::Image(handle.clone().into())));
        shot.target = Some(handle);
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
        window: Single<Entity, With<PrimaryWindow>>,
        card: Single<&Camera, With<CardCamera>>,
        mut keyboard: MessageWriter<KeyboardInput>,
        mut exit: MessageWriter<AppExit>,
    ) {
        if let (true, Some(v)) = (shot.frames == shot.warm, &card.viewport) {
            println!(
                "card {} {} {} {}",
                v.physical_position.x, v.physical_position.y, v.physical_size.x, v.physical_size.y
            );
        }
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
                Act::Lift(item) => world.lift_inventory(item.into()),
                Act::Paste(text) => {
                    world.paste_text(text);
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
                .spawn(Screenshot::image(
                    shot.target
                        .clone()
                        .expect("the offscreen camera spawned first"),
                ))
                .observe(save_to_disk(path));
        }
        if n == count + 120 {
            exit.write(AppExit::Success);
        }
    }

    fn move_sound_view(
        shot: Res<Shot>,
        mut world: ResMut<World>,
        mut camera: Single<&mut Transform, (With<IsDefaultUiCamera>, Without<CardCamera>)>,
    ) {
        if !shot.moving_view {
            return;
        }
        let count = shot.clip.unwrap_or(1);
        let progress =
            shot.frames.saturating_add(1).saturating_sub(shot.warm) as f32 / count.max(1) as f32;
        camera.translation = sound_view(progress).center.extend(0.0);
        if shot.clip.is_some() && shot.frames + 1 == shot.warm {
            world.running = true;
            world.since = world.period;
        }
    }

    fn sound_view(progress: f32) -> crate::sound::View {
        crate::sound::View {
            center: px(Hex::new(-8, 0)).lerp(px(Hex::new(8, 0)), progress),
            half: Vec2::new(1280.0, 720.0) * MICRO_SCALE / 2.0,
            scale: MICRO_SCALE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sim::{Atom, AtomKind, Bond, Tier};

    const SEAM_TONE: f32 = 0.05;
    const SEAM_GRAIN: f32 = 0.015;
    const BLUR_PX: f32 = 1.0;

    fn still_frames(view: &str, n: u32) -> Vec<image::RgbaImage> {
        let _render = RENDER_TEST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
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
            p.x >= PALETTE_WIDTH + 16.0 + reach
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
        assert!(seams.len() > 400, "only {} seams in view", seams.len());
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
        prev.glyphs
            .push(Some(Glyph::new(GlyphKind::Source, Hex::new(0, 0), 0)));
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
        for q in -8..8 {
            for r in -8..8 {
                let swing = Swing::from_cell(Hex::new(q, r));
                assert_eq!(swing.at(1.0), 1.0);
                assert_eq!(swing.at(1.23), 1.0);
            }
        }
        for swing in family() {
            assert_eq!(swing.at(0.0), 0.0);
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
            energy: sim::ActivationEnergy::default(),
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
    fn an_anchor_under_the_cursor_wins_over_a_body_cell() {
        let bonder = bonder(Hex::new(1, 0), 0);
        let arm = Arm::new(ORIGIN, 0, vec![]);
        assert_eq!(arm.hand(), bonder.at);
        assert_eq!(
            lone(vec![bonder], vec![arm.clone()]).hit(bonder.at),
            Some(Id::Glyph(0))
        );
        assert_eq!(lone(vec![bonder], vec![arm]).hit(ORIGIN), Some(Id::Arm(0)));
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
        let source = Glyph::new(GlyphKind::Source, Hex::new(0, 3), 0);
        let mut w = lone(
            vec![bonder(ORIGIN, 0), source],
            vec![
                Arm::new(Hex::new(3, 0), 3, vec![Instr::Grab, Instr::Rot(Spin::Cw)]),
                Arm::new(Hex::new(-3, 0), 0, vec![]),
            ],
        );
        stock_consumables(&mut w);
        stocked(&mut w, Machine::Arm, sim::DEFAULT_CAP);
        stocked(&mut w, Machine::Glyph(GlyphKind::Bonder), sim::DEFAULT_CAP);
        w
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
            Some(Focus::Hold { set, back: Back::Inventory }) if set.arms.len() == 1 && set.glyphs.len() == 1
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

    fn stock_consumables(w: &mut World) {
        for item in [Item::Step]
            .into_iter()
            .chain(KEYS.map(|k| Item::Token(k.instr)))
        {
            stocked(w, item, sim::DEFAULT_CAP);
        }
    }

    fn after_spending(sim: &Sim, item: Item, n: u32) -> Sim {
        let mut sim = sim.clone();
        for _ in 0..n {
            assert!(sim.inventory.spend(item));
        }
        sim
    }

    fn armed(tape: Vec<Instr>) -> World {
        let mut w = lone(vec![], vec![Arm::new(ORIGIN, 0, tape)]);
        stock_consumables(&mut w);
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
        w.sim
            .glyphs
            .push(Some(Glyph::new(GlyphKind::Bonder, Hex::new(3, 3), 0)));
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
        w.sim
            .glyphs
            .push(Some(Glyph::new(GlyphKind::Bonder, Hex::new(3, 3), 2)));
        w.pick(vec![Id::Glyph(0)]);
        w.key(KeyCode::KeyQ, false);
        w.key(KeyCode::KeyE, false);
        assert_eq!(w.sim.glyphs[0].unwrap().dir, 2);
        w.lift(fresh(Item::Machine(Machine::Arm)), Back::Inventory);
        w.key(KeyCode::KeyQ, false);
        w.key(KeyCode::KeyE, false);
        assert_eq!(held_dir(&w), 0);
    }

    #[test]
    fn glyph_focus_and_a_held_item_turn_with_a_and_d() {
        let bonder = Glyph::new(GlyphKind::Bonder, ORIGIN, 0);
        let mut w = lone(vec![bonder], vec![]);
        w.pick(vec![Id::Glyph(0)]);
        w.key(KeyCode::KeyA, false);
        assert_eq!(w.sim.glyphs[0].unwrap().dir, 5);
        w.key(KeyCode::KeyD, false);
        w.key(KeyCode::KeyD, false);
        assert_eq!(w.sim.glyphs[0].unwrap().dir, 1);
        assert_eq!(w.prev, w.sim);
        w.lift(fresh(Item::Machine(Machine::Arm)), Back::Inventory);
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
        stocked(&mut w, Machine::Arm, 1);
        w.lift(fresh(Item::Machine(Machine::Arm)), Back::Inventory);
        let to = Hex::new(-2, 3);
        w.release(Some(to));
        assert_eq!(w.sim.arms[0].pivot, to);
        assert_eq!(w.focus, picked(&[Id::Arm(0)]));
    }

    fn chain(w: &mut World) -> [usize; 3] {
        let [mid, right] = pair(w, ORIGIN, BondKind::Single);
        let left = w.sim.spawn(Atom {
            kind: AtomKind::Base,
            pos: DIRS[3],
        });
        w.sim.bonds.push(Bond {
            a: left,
            b: mid,
            kind: BondKind::Double,
        });
        w.prev = w.sim.clone();
        [left, mid, right]
    }

    #[test]
    fn a_click_on_a_bare_atom_picks_it_and_z_deletes_it_severing_its_bonds_returning_nothing_where_a_glyph_returns_itself()
     {
        let mut w = lone(vec![bonder(Hex::new(4, 4), 0)], vec![]);
        w.running = false;
        let [left, mid, right] = chain(&mut w);
        let stock = w.sim.inventory;
        w.press(px(ORIGIN), px(ORIGIN));
        assert_eq!(w.focus, picked(&[Id::Atom(mid)]));
        w.release(Some(ORIGIN));
        assert_eq!(w.focus, picked(&[Id::Atom(mid)]));
        w.key(KeyCode::KeyZ, false);
        assert_eq!(w.sim.atoms[mid], None);
        assert_eq!(w.sim.atoms[left].unwrap().pos, DIRS[3]);
        assert_eq!(w.sim.atoms[right].unwrap().pos, DIRS[0]);
        assert!(w.sim.bonds.is_empty());
        assert_eq!(w.sim.inventory, stock);
        assert_eq!(w.focus, None);
        w.press(px(Hex::new(4, 4)), px(Hex::new(4, 4)));
        w.key(KeyCode::KeyZ, false);
        assert_eq!(w.sim.glyphs, vec![None]);
        assert_eq!(count(&w, Machine::Glyph(GlyphKind::Bonder)), 1);
    }

    #[test]
    fn z_on_an_atom_at_a_ghost_frame_leaves_both_frames_as_they_were() {
        let mut w = lone(vec![], vec![]);
        w.running = false;
        let [_, mid, _] = chain(&mut w);
        w.resim(1);
        let (sim, ghost) = (w.sim.clone(), w.ghost.clone());
        w.press(px(ORIGIN), px(ORIGIN));
        assert_eq!(w.focus, picked(&[Id::Atom(mid)]));
        w.key(KeyCode::KeyZ, false);
        assert_eq!(w.sim, sim);
        assert_eq!(w.ghost, ghost);
        assert_eq!(w.shown().bonds.len(), 2);
    }

    #[test]
    fn g_drops_a_picked_atom_from_the_pick_as_s_does() {
        let mut w = lone(vec![], vec![]);
        w.running = false;
        stock_consumables(&mut w);
        let [_, mid, _] = chain(&mut w);
        w.press(px(ORIGIN), px(ORIGIN));
        assert_eq!(w.focus, picked(&[Id::Atom(mid)]));
        w.key(KeyCode::KeyG, false);
        assert_eq!(w.ghosts(), 1);
        assert_eq!(w.focus, None);
    }

    #[test]
    fn the_delete_scene_leaves_the_ends_of_the_chain_unbonded() {
        let w = played("delete", 90);
        assert_eq!(w.focus, picked(&[Id::Atom(1)]));
        assert_eq!(atoms(&w).len(), 3);
        let w = played("delete", 100);
        assert_eq!(w.focus, None);
        assert_eq!(atoms(&w), vec![DIRS[3], DIRS[0]]);
        assert!(w.sim.bonds.is_empty());
        assert!(!w.running);
    }

    fn paused(n: usize) -> World {
        let (mut w, _, _, _) = shot::scene("walk", 2);
        stock_consumables(&mut w);
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
        let (_, expected_events) = w.sim.replayed(5);
        assert_eq!(w.ghosts(), 5);
        assert_eq!(w.sim, after_spending(&ghost0, Item::Step, 5));
        assert_eq!(*w.shown(), w.sim.replay(5));
        assert_eq!(w.prev, w.sim.replay(4));
        assert_eq!(w.events, expected_events);
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
        let mut scratch = paused(3);
        scratch.sim.arms[0].tape.insert(0, Instr::Rot(Spin::Cw));
        scratch.sim = after_spending(&scratch.sim, Item::Token(Instr::Rot(Spin::Cw)), 1);
        assert_eq!(w.sim, scratch.sim);
        scratch.resim(3);
        assert_eq!(w.ghosts(), 3);
        assert_eq!(*w.shown(), *scratch.shown());
        assert_eq!(*w.shown(), w.sim.replay(3));
        assert_eq!(w.prev, *w.shown());
        assert_eq!(w.focus, Some(Focus::Tape { arm: 0, cursor: 1 }));
    }

    #[test]
    fn an_arm_edit_at_n_is_refused_and_ghost0_is_unchanged() {
        let mut w = paused(4);
        stocked(&mut w, Machine::Arm, 1);
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
        w.lift(fresh(Item::Machine(Machine::Arm)), Back::Inventory);
        w.place(Some(Hex::new(5, 5)));
        assert_eq!(w.sim, ghost0);
        assert_eq!(*w.shown(), ghost4);
        assert_eq!(w.focus, None);
        assert_eq!(w.refused, None);
    }

    #[test]
    fn a_glyph_placed_at_n_appears_in_ghost0_and_in_the_re_simmed_ghost_n() {
        let mut w = paused(2);
        let ghost0 = w.sim.clone();
        let at = Hex::new(5, 5);
        stocked(&mut w, Machine::Glyph(GlyphKind::Bonder), 1);
        w.lift(
            fresh(Item::Machine(Machine::Glyph(GlyphKind::Bonder))),
            Back::Inventory,
        );
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
        cleared
            .inventory
            .add(Item::Machine(Machine::Glyph(GlyphKind::Bonder)));
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
        assert_eq!(
            w.focus,
            picked(&[Id::Atom(w.sim.atom_at(Hex::new(3, 3)).unwrap())])
        );
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
    fn a_glyph_dragged_onto_another_glyphs_cell_pops_back_picked_and_slid_one_cell_over_its_own_place_lands()
     {
        let mover = bonder(ORIGIN, 0);
        let other = bonder(Hex::new(3, 0), 0);
        let mut w = lone(vec![mover, other], vec![]);
        w.running = false;
        drag(&mut w, ORIGIN, Hex::new(2, 0));
        assert_eq!(w.sim.glyphs, vec![Some(mover), Some(other)]);
        assert_eq!(w.focus, picked(&[Id::Glyph(0)]));
        drag(&mut w, ORIGIN, Hex::new(1, 0));
        assert_eq!(
            w.sim.glyphs,
            vec![Some(bonder(Hex::new(1, 0), 0)), Some(other)]
        );
    }

    #[test]
    fn a_picked_glyph_turned_onto_another_glyphs_cell_keeps_its_turn_and_turned_onto_free_cells_turns()
     {
        let mover = bonder(ORIGIN, 0);
        let other = bonder(Hex::new(1, -1), 0);
        let mut w = lone(vec![mover, other], vec![]);
        w.running = false;
        w.press(px(ORIGIN), px(ORIGIN));
        w.release(Some(ORIGIN));
        assert_eq!(w.focus, picked(&[Id::Glyph(0)]));
        w.key(KeyCode::KeyD, false);
        assert_eq!(w.sim.glyphs[0], Some(mover));
        w.key(KeyCode::KeyA, false);
        assert_eq!(w.sim.glyphs[0], Some(bonder(ORIGIN, 5)));
        w.sim.arms.push(Arm::new(Hex::new(3, -1), 3, vec![]));
        w.pick(vec![Id::Arm(0)]);
        w.key(KeyCode::KeyD, false);
        assert_eq!(w.sim.arms[0].dir, 4);
    }

    #[test]
    fn the_overlap_scene_refuses_the_first_drop_and_lands_the_second() {
        let w = played("overlap", 70);
        assert_eq!(w.sim.glyphs[0], Some(bonder(Hex::new(-3, 0), 0)));
        let w = played("overlap", 130);
        assert_eq!(w.sim.glyphs[0], Some(bonder(Hex::new(-1, 0), 0)));
        assert_eq!(w.sim.glyphs.len(), 2);
    }

    #[test]
    fn a_palette_glyph_or_a_paste_on_another_glyphs_cell_is_refused_unspent_and_beside_it_lands() {
        let other = bonder(Hex::new(3, 0), 0);
        let item = Item::Machine(Machine::Glyph(GlyphKind::Bonder));
        let mut w = lone(vec![other], vec![]);
        w.running = false;
        w.sim.inventory.add(item);
        w.lift_inventory(item);
        w.release(Some(Hex::new(2, 0)));
        assert_eq!(w.sim.glyphs, vec![Some(other)]);
        assert_eq!(w.sim.inventory.count(item), Some(1));
        assert_eq!(w.focus, None);
        w.lift_inventory(item);
        w.release(Some(Hex::new(1, 0)));
        assert_eq!(
            w.sim.glyphs,
            vec![Some(other), Some(bonder(Hex::new(1, 0), 0))]
        );
        assert_eq!(w.sim.inventory.count(item), Some(0));
        w.clipboard = Some(fresh(Item::Machine(Machine::Glyph(GlyphKind::Bonder))));
        w.sim.inventory.add(item);
        w.paste();
        w.release(Some(Hex::new(4, 0)));
        assert_eq!(w.sim.glyphs.len(), 2);
        assert_eq!(w.sim.inventory.count(item), Some(1));
        w.paste();
        w.release(Some(Hex::new(5, 0)));
        assert_eq!(w.sim.glyphs[2], Some(bonder(Hex::new(5, 0), 0)));
        assert_eq!(w.sim.inventory.count(item), Some(0));
    }

    #[test]
    fn an_arm_whose_hand_reaches_over_a_glyph_lands() {
        let other = bonder(Hex::new(3, 0), 0);
        let mut w = lone(vec![other], vec![]);
        w.running = false;
        stocked(&mut w, Machine::Arm, 1);
        w.lift(fresh(Item::Machine(Machine::Arm)), Back::Inventory);
        w.release(Some(Hex::new(2, 0)));
        assert_eq!(w.sim.arms[0].hand(), other.at);
    }

    #[test]
    fn a_base_dragged_onto_a_glyph_a_base_or_an_atom_pops_back_picked_and_beside_them_lands() {
        let other = Arm::new(Hex::new(0, 2), 3, vec![]);
        let mut w = lone(
            vec![bonder(ORIGIN, 0)],
            vec![Arm::new(Hex::new(3, 0), 0, vec![]), other.clone()],
        );
        w.running = false;
        w.sim.spawn(Atom {
            kind: AtomKind::Base,
            pos: Hex::new(0, -2),
        });
        w.prev = w.sim.clone();
        let before = w.sim.clone();
        for blocked in [Hex::new(1, 0), Hex::new(0, 2), Hex::new(0, -2)] {
            drag(&mut w, Hex::new(3, 0), blocked);
            assert_eq!(
                w.sim, before,
                "a base set down at {blocked:?} must pop back"
            );
            assert_eq!(w.focus, picked(&[Id::Arm(0)]));
        }
        drag(&mut w, Hex::new(3, 0), Hex::new(2, 0));
        assert_eq!(w.sim.arms[0].pivot, Hex::new(2, 0));
        assert_eq!(w.sim.arms[1], other);
    }

    #[test]
    fn a_glyph_dragged_onto_a_base_pops_back_picked_and_a_palette_arm_on_a_glyph_is_refused_unspent()
     {
        let mover = bonder(Hex::new(-3, 0), 0);
        let mut w = lone(vec![mover], vec![Arm::new(ORIGIN, 0, vec![])]);
        w.running = false;
        drag(&mut w, Hex::new(-3, 0), Hex::new(-1, 0));
        assert_eq!(w.sim.glyphs, vec![Some(mover)]);
        assert_eq!(w.focus, picked(&[Id::Glyph(0)]));
        let item = Item::Machine(Machine::Arm);
        w.sim.inventory.add(item);
        w.lift_inventory(item);
        w.release(Some(Hex::new(-2, 0)));
        assert_eq!(w.sim.arms.len(), 1);
        assert_eq!(w.sim.inventory.count(item), Some(1));
        assert_eq!(w.focus, None);
        w.lift_inventory(item);
        w.release(Some(Hex::new(-2, 1)));
        assert_eq!(w.sim.arms.len(), 2);
        assert_eq!(w.sim.inventory.count(item), Some(0));
    }

    #[test]
    fn the_preview_names_the_glyph_a_held_base_would_stand_on_and_nothing_where_it_lands() {
        let mut w = lone(
            vec![bonder(ORIGIN, 0)],
            vec![Arm::new(Hex::new(3, 0), 0, vec![])],
        );
        w.running = false;
        lift_at(&mut w, Hex::new(3, 0));
        let Some(Focus::Hold { set, back }) = w.focus.clone() else {
            panic!("not holding: {:?}", w.focus)
        };
        let blocked = |at: Hex| w.sim.blocked(&set, at, back.picked()).collect::<Vec<Id>>();
        assert_eq!(blocked(Hex::new(1, 0)), vec![Id::Glyph(0)]);
        assert_eq!(blocked(Hex::new(2, 0)), Vec::<Id>::new());
        assert_eq!(blocked(Hex::new(3, 0)), Vec::<Id>::new());
    }

    #[test]
    fn the_base_scene_refuses_the_first_drop_and_lands_the_second() {
        let w = played("base", 70);
        assert_eq!(w.sim.arms[0].pivot, Hex::new(2, 0));
        let w = played("base", 130);
        assert_eq!(w.sim.arms[0].pivot, ORIGIN);
        assert_eq!(w.sim.glyphs[0], Some(bonder(Hex::new(-2, 0), 0)));
    }

    #[test]
    fn nothing_in_a_shipped_layout_shares_a_cell_but_an_atom_on_a_glyph() {
        let scenes = shot::SCENES
            .iter()
            .map(|name| (name.to_string(), shot::scene(name, 0).0.sim));
        let sims = [sim::layout(), sim::start(), sim::preloaded()];
        let named = ["layout", "start", "preloaded"]
            .iter()
            .map(|n| n.to_string())
            .zip(sims);
        let fixtures = Machine::ALL
            .into_iter()
            .map(|m| (format!("{m:?} fixture"), fixture(m).sim));
        for (name, sim) in scenes.chain(named).chain(fixtures) {
            for id in sim.ids() {
                for cell in sim.stands(id) {
                    for other in sim.on(cell) {
                        assert!(
                            other == id || id.may_share(other),
                            "{name}: {id:?} shares {cell:?} with {other:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn no_glyph_fires_on_a_carried_compound_and_a_drop_on_an_output_is_eaten_at_the_end_of_that_tick()
     {
        let at = Hex::new(2, -2);
        let mut w = lone(
            vec![Glyph::new(GlyphKind::Output(Tier::One), at, 0)],
            vec![],
        );
        w.running = false;
        pair(&mut w, at, BondKind::Double);
        lift_at(&mut w, at);
        w.step();
        assert_eq!(count(&w, Machine::Arm), 0);
        w.pointer = Some(px(at));
        w.release(Some(at));
        assert_eq!(count(&w, Machine::Arm), 0);
        assert_eq!(atoms(&w).len(), 2);
        w.step();
        assert_eq!(count(&w, Machine::Arm), 1);
        assert_eq!(atoms(&w), vec![]);
    }

    fn count(w: &World, item: impl Into<Item>) -> u32 {
        w.sim.inventory.count(item.into()).unwrap()
    }

    fn stocked(w: &mut World, item: impl Into<Item>, n: u32) {
        let item = item.into();
        for _ in 0..n {
            w.sim.inventory.add(item);
        }
    }

    #[test]
    fn a_machine_lifted_from_the_inventory_is_spent_at_the_drop_and_returned_by_z_and_by_x() {
        let bonder = Item::from(Machine::Glyph(GlyphKind::Bonder));
        let mut w = lone(vec![], vec![]);
        w.lift_inventory(bonder);
        assert_eq!(w.focus, None);
        stocked(&mut w, bonder, 1);
        w.lift_inventory(bonder);
        assert!(matches!(
            w.focus,
            Some(Focus::Hold {
                back: Back::Inventory,
                ..
            })
        ));
        assert_eq!(count(&w, bonder), 1);
        w.key(KeyCode::Escape, false);
        assert_eq!(w.focus, None);
        assert_eq!(count(&w, bonder), 1);
        w.lift_inventory(bonder);
        let to = Hex::new(-2, 3);
        w.release(Some(to));
        assert_eq!(count(&w, bonder), 0);
        assert_eq!(w.sim.glyphs[0].unwrap().at, to);
        assert_eq!(w.focus, picked(&[Id::Glyph(0)]));
        w.lift_inventory(bonder);
        assert_eq!(w.focus, picked(&[Id::Glyph(0)]));
        stocked(&mut w, bonder, sim::DEFAULT_CAP);
        w.key(KeyCode::KeyX, false);
        assert_eq!(w.sim.glyphs[0], None);
        assert_eq!(count(&w, bonder), sim::DEFAULT_CAP + 1);
        w.key(KeyCode::KeyV, false);
        w.release(Some(to));
        assert_eq!(count(&w, bonder), sim::DEFAULT_CAP);
        w.key(KeyCode::KeyZ, false);
        assert_eq!(w.sim.glyphs[0], None);
        assert_eq!(count(&w, bonder), sim::DEFAULT_CAP + 1);
        assert!(w.sim.inventory.full(bonder));
    }

    #[test]
    fn a_scroll_over_an_entry_changes_only_that_cap_and_the_glyph_reads_it() {
        let bonder = Item::from(Machine::Glyph(GlyphKind::Bonder));
        let at = Hex::new(2, -2);
        let mut w = lone(
            vec![Glyph::new(GlyphKind::Output(Tier::One), at, 0)],
            vec![],
        );
        w.running = false;
        w.set_cap(bonder, -(sim::DEFAULT_CAP as i32) - 5);
        assert_eq!(w.sim.inventory.cap(bonder), Some(0));
        for item in palette().filter(|item| *item != bonder) {
            assert_eq!(
                w.sim.inventory.cap(item),
                Some(sim::DEFAULT_CAP),
                "{item:?}"
            );
        }
        w.set_cap(bonder, i32::MAX);
        assert_eq!(w.sim.inventory.cap(bonder), Some(sim::MAX_CAP));
        w.set_cap(bonder, -(sim::MAX_CAP as i32));
        pair(&mut w, at, BondKind::Single);
        w.step();
        assert_eq!(atoms(&w).len(), 2);
        assert_eq!(count(&w, bonder), 0);
        w.set_cap(bonder, 1);
        w.step();
        assert_eq!(atoms(&w), vec![]);
        assert_eq!(count(&w, bonder), 1);
    }

    #[test]
    fn the_palette_lists_every_machine_but_the_source() {
        let listed: Vec<Item> = palette().collect();
        let all: Vec<Item> = Machine::ALL
            .into_iter()
            .filter(|m| *m != Machine::Glyph(GlyphKind::Source))
            .map(Item::Machine)
            .chain([Item::Step])
            .chain(KEYS.map(|k| Item::Token(k.instr)))
            .chain(AtomKind::ALL.map(Item::Atom))
            .collect();
        assert_eq!(listed.len(), all.len());
        assert!(all.iter().all(|item| listed.contains(item)));
    }

    #[test]
    fn world_pictures_in_inventory_rows_have_one_circular_clay_field() {
        for item in palette() {
            let (node, field) = picture_square(item);
            if matches!(item, Item::Machine(_) | Item::Atom(_)) {
                assert_eq!(field.0, Glaze::Clay.color());
                assert_eq!(node.border_radius, BorderRadius::MAX);
            } else {
                assert_eq!(field.0, Color::NONE);
                assert_eq!(node.border_radius, BorderRadius::default());
            }
        }
    }

    #[test]
    fn inventory_atoms_use_the_world_bead_circle_and_rim() {
        let _render = RENDER_TEST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir =
            std::env::temp_dir().join(format!("ziral-inventory-atoms-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = shot::still("start", dir.clone(), 1);
        lit_plugin(&mut app);
        app.add_systems(
            Last,
            |kiln: Res<Kiln>,
             pictures: Query<(&AtomPreview, &ImageNode), Without<Camera>>,
             fills: Query<(&RenderLayers, &Mesh2d), With<Fill>>| {
                for kind in sim::AtomKind::ALL {
                    let images: Vec<&ImageNode> = pictures
                        .iter()
                        .filter(|(preview, _)| preview.0 == kind)
                        .map(|(_, image)| image)
                        .collect();
                    assert_eq!(images.len(), 1);
                    assert_eq!(images[0].image, kiln.atom(kind));
                    let meshes: Vec<&Handle<Mesh>> = fills
                        .iter()
                        .filter(|(layers, _)| **layers == atom_layer(kind))
                        .map(|(_, mesh)| &mesh.0)
                        .collect();
                    assert_eq!(meshes.len(), 2);
                    assert!(meshes.contains(&&kiln.circle));
                    assert!(meshes.contains(&&kiln.rim));
                }
            },
        );
        assert_eq!(app.run(), bevy::app::AppExit::Success);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn spawn(w: &mut World, q: i32, r: i32) -> usize {
        w.sim.spawn(Atom {
            kind: AtomKind::Base,
            pos: Hex::new(q, r),
        })
    }

    #[test]
    fn a_marquee_over_one_atom_of_a_compound_copies_the_whole_compound_and_the_machines_beside_it()
    {
        let mut w = lone(vec![bonder(Hex::new(4, 4), 0)], vec![]);
        let bent = |w: &mut World, q: i32, r: i32| {
            let a = spawn(w, q, r);
            let b = spawn(w, q, r + 1);
            let c = spawn(w, q + 1, r + 1);
            w.sim.bonds.push(sim::Bond {
                a,
                b,
                kind: BondKind::Single,
            });
            w.sim.bonds.push(sim::Bond {
                a: b,
                b: c,
                kind: BondKind::Single,
            });
            a
        };
        let first = bent(&mut w, 0, 0);
        bent(&mut w, -6, 2);
        let output = Item::Machine(Machine::Glyph(GlyphKind::Output(sim::Tier::One)));
        let text = form::RECIPES
            .iter()
            .find(|(item, _)| *item == output)
            .unwrap()
            .1;
        let corner = px(Hex::new(0, 0));
        let from = px(Hex::new(-1, 0));
        w.press(from, from);
        w.drag(corner + Vec2::splat(1.0));
        w.pointer = Some(corner + Vec2::splat(1.0));
        w.release(Some(Hex::new(0, 0)));
        assert_eq!(w.focus, picked(&[Id::Atom(first)]));
        assert_eq!(w.compounds(&[Id::Atom(first)]), text);
        w.copy(&[Id::Atom(first)]);
        assert!(w.clipboard.is_none());
        let (lo, hi) = (px(Hex::new(-7, 0)), px(Hex::new(5, 5)));
        w.press(lo, lo);
        w.drag(hi);
        w.pointer = Some(hi);
        w.release(Some(Hex::new(5, 5)));
        let ids = w.focus.as_ref().unwrap().picked();
        assert!(ids.contains(&Id::Glyph(0)));
        assert_eq!(ids.iter().filter(|id| matches!(id, Id::Atom(_))).count(), 6);
        assert_eq!(w.compounds(&ids), format!("{text}\n{text}"));
        w.copy(&ids);
        assert_eq!(w.clipboard.as_ref().unwrap().glyphs.len(), 1);
        assert!(w.clipboard.as_ref().unwrap().atoms.is_empty());
    }

    fn card_fills(machine: Machine, ticks: u64) -> Vec<(Vec3, f32)> {
        let _render = RENDER_TEST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let name = machines::name(machine);
        let dir =
            std::env::temp_dir().join(format!("ziral-card-{name}-{ticks}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = shot::still(&format!("card:{name}"), dir.clone(), 1);
        lit_plugin(&mut app);
        app.world_mut().resource_mut::<World>().play = Some(Play::at(machine, ticks));
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let probe = seen.clone();
        app.add_systems(
            Last,
            move |fills: Query<(&RenderLayers, &Transform), With<Fill>>,
                  camera: Single<&Camera, With<CardCamera>>| {
                assert!(camera.is_active);
                assert_eq!(
                    camera.viewport.as_ref().unwrap().physical_size,
                    card_size(machine.into()).as_uvec2()
                );
                *probe.lock().unwrap() = fills
                    .iter()
                    .filter(|(l, _)| **l == CARD)
                    .map(|(_, t)| (t.translation, t.scale.x))
                    .collect();
            },
        );
        assert_eq!(app.run(), bevy::app::AppExit::Success);
        std::fs::remove_dir_all(&dir).unwrap();
        seen.lock().unwrap().clone()
    }

    fn bars(sim: &Sim) -> usize {
        sim.bonds
            .iter()
            .map(|bond| match look::bond(bond.kind).shape {
                Shape::Bars(n) => n,
                _ => unreachable!(),
            })
            .sum()
    }

    #[test]
    fn the_card_draws_the_picture_the_recipe_and_the_fixture_at_tick_t_as_the_sim_has_it() {
        let bonder = Machine::Glyph(GlyphKind::Bonder);
        for (machine, t) in [(bonder, 0), (bonder, 4), (bonder, 9), (Machine::Arm, 2)] {
            let fills = card_fills(machine, t);
            let f = fixture(machine);
            let sim = f.sim.replay(t);
            let card = layout(machine.into());
            let recipe = machine.recipe().unwrap().sim();
            let at = |z: f32| {
                fills
                    .iter()
                    .filter(|(p, _)| (p.z - z).abs() < 1e-3)
                    .map(|(p, s)| (p.truncate(), *s))
                    .collect::<Vec<_>>()
            };
            let name = format!("{machine:?} at {t}");
            assert_eq!(
                at(layer::LIFT).len(),
                playfield(machine).len(),
                "{name} tiles"
            );
            assert_eq!(
                at(layer::GLYPHS + layer::LIFT).len(),
                sim.glyphs
                    .iter()
                    .flatten()
                    .map(|glyph| rig::parts(Machine::Glyph(glyph.kind)).len())
                    .sum::<usize>(),
                "{name} glyphs"
            );
            assert_eq!(
                at(layer::BOND + layer::LIFT).len(),
                bars(&sim),
                "{name} bonds"
            );
            let beads = at(layer::BEAD + layer::LIFT);
            let atoms: Vec<Vec2> = sim
                .atoms
                .iter()
                .flatten()
                .map(|a| card.field.unwrap() + px(a.pos))
                .collect();
            assert_eq!(beads.len(), atoms.len(), "{name} beads");
            for atom in &atoms {
                assert!(
                    beads
                        .iter()
                        .any(|(p, s)| p.distance(*atom) < 1e-3 && *s == HEX * 0.4),
                    "{name}: no bead at {atom}"
                );
            }
            let arms: Vec<(Vec2, f32)> = fills
                .iter()
                .filter(|(p, _)| {
                    (layer::ARMS.start + layer::LIFT..layer::ARMS.end + layer::LIFT).contains(&p.z)
                })
                .map(|(p, s)| (p.truncate(), *s))
                .collect();
            assert_eq!(
                arms.len(),
                sim.arms.len() * rig::parts(Machine::Arm).len(),
                "{name} arms"
            );
            for arm in &sim.arms {
                let quad = look::quad(Machine::Arm);
                let angle = (px(arm.hand()) - px(arm.pivot)).to_angle();
                let centre = card.field.unwrap()
                    + px(arm.pivot)
                    + Vec2::from_angle(angle).rotate(quad.centre);
                assert!(
                    arms.iter().any(|(p, _)| p.distance(centre) < 1e-3),
                    "{name}: no arm at {centre}"
                );
            }
            let recipe_beads: Vec<Vec2> = at(layer::z(layer::CARD, 4, 5))
                .iter()
                .map(|(p, _)| *p)
                .collect();
            assert_eq!(
                recipe_beads.len(),
                recipe.atoms.len(),
                "{name} recipe beads"
            );
            assert_eq!(
                at(layer::z(layer::CARD, 3, 5)).len(),
                bars(&recipe),
                "{name} recipe bars"
            );
            assert_eq!(
                fills.len(),
                2 + rig::parts(machine).len()
                    + bars(&recipe)
                    + 2 * recipe.atoms.len()
                    + playfield(machine).len()
                    + sim
                        .glyphs
                        .iter()
                        .flatten()
                        .map(|glyph| rig::parts(Machine::Glyph(glyph.kind)).len())
                        .sum::<usize>()
                    + bars(&sim)
                    + 2 * atoms.len()
                    + sim.arms.len() * rig::parts(Machine::Arm).len(),
                "{name}: something else on the card"
            );
        }
    }

    #[test]
    fn the_playback_ticks_with_the_world_clock_holds_its_last_frame_and_loops() {
        let bonder = Machine::Glyph(GlyphKind::Bonder);
        let f = fixture(bonder);
        let mut w = World::new(Sim::empty());
        w.hover = Some(bonder.into());
        w.advance(0.0);
        for t in 0..=f.ticks + HOLD + 1 {
            let (sim, prev) = match t {
                0 => (0, 0),
                t if t <= f.ticks => (t, t - 1),
                t if t <= f.ticks + HOLD => (f.ticks, f.ticks),
                _ => (0, 0),
            };
            let play = w.play.as_ref().unwrap();
            let (before, now) = play.sims();
            assert_eq!(now, f.sim.replay(sim), "tick {t}");
            assert_eq!(before, f.sim.replay(prev), "tick {t} prev");
            w.advance(w.period);
        }
        w.hover = Some(Machine::Arm.into());
        w.advance(0.0);
        assert_eq!(w.play.as_ref().unwrap().sims().1, fixture(Machine::Arm).sim);
        w.hover = None;
        w.advance(0.0);
        assert!(w.play.is_none());
        let past = Play::at(bonder, f.ticks + 99);
        assert_eq!((past.at, past.sims().1), (f.ticks, f.sim.replay(f.ticks)));
    }

    #[test]
    fn the_card_playback_keeps_the_fixture_tick_stream_that_made_its_frames() {
        let machine = Machine::Glyph(GlyphKind::Bonder);
        let play = Play::at(machine, 0);
        let (_, expected) = fixture(machine).sim.replayed(fixture(machine).ticks);
        assert_eq!(play.events, expected);
        assert_eq!(play.frames.len(), play.events.len() + 1);
        assert!(play.events.iter().any(|tick| {
            tick.events
                .iter()
                .any(|event| matches!(event, sim::TickEvent::Fired { glyph: 0, .. }))
        }));
    }

    #[test]
    fn a_token_or_step_card_ends_at_its_recipe_and_a_machine_card_holds_its_playfield() {
        for item in [Item::Step, Item::Token(Instr::Grab)] {
            let card = layout(item);
            assert_eq!(card.field, None, "{item:?}");
            assert_eq!(
                card.size.x,
                3.0 * CARD_PAD + picture_side(item) + recipe_side(),
                "{item:?}"
            );
        }
        let bonder = Machine::Glyph(GlyphKind::Bonder);
        let (lo, hi) = play_bounds(&playfield(bonder));
        let card = layout(bonder.into());
        assert_eq!(
            card.size.x,
            4.0 * CARD_PAD + picture_side(bonder.into()) + recipe_side() + (hi - lo).x
        );
        let corner = Vec2::new(card.size.x / 2.0 - CARD_PAD, (hi - lo).y / 2.0);
        assert!((card.field.unwrap() + hi).distance(corner) < 1e-3);
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
        w.focus = None;
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
        assert_eq!(w.sim, after_spending(&ghost0, Item::Step, 6));
        assert_eq!(w.focus, None);
    }

    #[test]
    fn a_compound_lifted_at_ghost0_and_dropped_after_a_step_forward_is_refused_and_pops_back() {
        let mut w = paused(0);
        pair(&mut w, Hex::new(6, 6), BondKind::Single);
        let ghost0 = w.sim.clone();
        lift_at(&mut w, Hex::new(6, 6));
        w.key(KeyCode::KeyG, false);
        assert_eq!(w.ghosts(), 1);
        w.pointer = Some(px(Hex::new(8, 8)));
        w.release(Some(Hex::new(8, 8)));
        assert_eq!(w.sim, after_spending(&ghost0, Item::Step, 1));
        assert_eq!(*w.shown(), w.sim.replay(1));
        assert_eq!(w.focus, None);
    }

    #[test]
    fn a_palette_lift_or_a_tape_focus_while_a_compound_is_in_the_hand_leaves_it_there() {
        let mut w = lone(vec![], vec![Arm::new(Hex::new(5, 5), 0, vec![])]);
        w.running = false;
        pair(&mut w, ORIGIN, BondKind::Single);
        lift_at(&mut w, ORIGIN);
        let hold = w.focus.clone();
        w.lift(fresh(Item::Machine(Machine::Arm)), Back::Inventory);
        assert_eq!(w.focus, hold);
        w.focus_tape(0);
        assert_eq!(w.focus, hold);
        assert_eq!(atoms(&w), vec![]);
        w.release(None);
        assert_eq!(atoms(&w), vec![ORIGIN, DIRS[0]]);
    }

    #[test]
    fn an_arm_covered_by_an_atom_gives_the_atom_to_a_drag_and_itself_once_its_tape_is_focused() {
        let mut w = lone(vec![], vec![Arm::new(Hex::new(-1, 0), 0, vec![])]);
        w.running = false;
        pair(&mut w, ORIGIN, BondKind::Single);
        lift_at(&mut w, ORIGIN);
        assert_eq!(held(&w).2, taken(ORIGIN));
        w.release(None);
        assert_eq!(w.focus, None);
        w.press(px(ORIGIN), px(ORIGIN));
        w.release(Some(ORIGIN));
        assert_eq!(w.focus, Some(Focus::Tape { arm: 0, cursor: 0 }));
        lift_at(&mut w, ORIGIN);
        assert_eq!(held(&w).2, Back::Pick(vec![Id::Arm(0)]));
        assert_eq!(atoms(&w).len(), 2);
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
                    shot::Act::Lift(item) => w.lift_inventory(item.into()),
                    shot::Act::Paste(text) => {
                        w.paste_text(text);
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
    fn the_hand_scene_crafts_one_arm_by_hand_with_no_arm_on_the_board() {
        let w = played("hand", 300);
        assert!(w.sim.arms.is_empty());
        assert_eq!(count(&w, Machine::Arm), 1, "{:?}", w.sim);
        assert_eq!(w.focus, None);
        assert_eq!(atoms(&w).len(), 1);
    }

    #[test]
    fn a_source_has_no_palette_row_and_no_inventory_entry_so_nothing_lifts_it() {
        let source = Item::from(Machine::Glyph(GlyphKind::Source));
        let bonder = Item::from(Machine::Glyph(GlyphKind::Bonder));
        let mut w = World::new(sim::start());
        assert_eq!(w.sim.inventory.count(source), None);
        w.sim.inventory.add(source);
        assert_eq!(w.sim.inventory, sim::Inventory::EMPTY);
        w.lift_inventory(source);
        assert_eq!(w.focus, None);
        w.sim.inventory.add(bonder);
        w.lift_inventory(bonder);
        assert!(w.holding());
    }

    #[test]
    fn the_hand_never_picks_lifts_or_deletes_a_source() {
        let mut w = World::new(sim::start());
        w.running = false;
        let glyphs = w.sim.glyphs.clone();
        let source = glyphs
            .iter()
            .flatten()
            .find(|g| g.kind == GlyphKind::Source)
            .unwrap()
            .at;
        w.pointer = Some(px(source));
        w.press(px(source), px(source));
        w.release(Some(source));
        assert_eq!(w.focus, None);
        w.press(px(source), px(source));
        w.key(KeyCode::KeyZ, false);
        assert_eq!(w.sim.glyphs, glyphs);
        lift_at(&mut w, source);
        w.pointer = Some(px(source.add(DIRS[0])));
        w.release(Some(source.add(DIRS[0])));
        assert_eq!(w.sim.glyphs, glyphs);
        assert_eq!(w.focus, None);
        w.step();
        let glyphs = w.sim.glyphs.clone();
        lift_at(&mut w, source);
        assert_eq!(held(&w).2, taken(source));
        w.release(Some(source.add(DIRS[0])));
        assert_eq!(w.sim.glyphs, glyphs);
        assert_eq!(atoms(&w), vec![source.add(DIRS[0])]);
        let far = Vec2::splat(1000.0);
        w.pointer = Some(far);
        w.press(-far, -far);
        w.drag(far);
        w.release(Some(hex_at(far)));
        assert_eq!(w.focus, picked(&[Id::Glyph(1), Id::Glyph(2), Id::Atom(0)]));
        w.key(KeyCode::KeyZ, false);
        assert_eq!(w.sim.glyphs, vec![glyphs[0], None, None]);
        assert_eq!(atoms(&w), vec![]);
        w.step();
        let on_source = w.sim.atom_at(source).unwrap();
        w.press(px(source), px(source));
        assert_eq!(w.focus, picked(&[Id::Atom(on_source)]));
        w.key(KeyCode::KeyZ, false);
        assert_eq!(atoms(&w), vec![]);
        assert_eq!(w.sim.glyphs, vec![glyphs[0], None, None]);
    }

    #[test]
    fn the_craft_scene_crafts_the_first_bonder_by_hand_from_the_t0_board_then_places_it() {
        let bonder = Machine::Glyph(GlyphKind::Bonder);
        let w = played("craft", 189);
        assert_eq!(count(&w, bonder), 1, "{:?}", w.sim);
        assert_eq!(w.sim.glyphs.len(), 3);
        let w = played("craft", 240);
        assert_eq!(count(&w, bonder), 0);
        assert_eq!(w.sim.glyphs[3].unwrap().kind, GlyphKind::Bonder);
        assert_eq!(w.sim.glyphs[3].unwrap().at, Hex::new(-2, -3));
    }
    #[test]
    fn the_keys_write_exactly_the_tokens_the_inventory_counts_in_the_same_order() {
        let tokens: Vec<Item> = recipes()
            .iter()
            .filter(|(item, _)| matches!(item, Item::Token(_)))
            .map(|(item, _)| *item)
            .collect();
        assert_eq!(tokens, KEYS.map(|k| Item::Token(k.instr)));
    }

    #[test]
    fn a_step_spends_one_step_item_and_refuses_at_zero_leaving_every_frame_as_it_was() {
        let step = Item::Step;
        let mut w = paused(0);
        w.sim.inventory = sim::Inventory::EMPTY;
        stocked(&mut w, step, 2);
        let ghost0 = w.sim.clone();
        w.key(KeyCode::KeyG, false);
        assert_eq!((w.ghosts(), count(&w, step)), (1, 1));
        w.key(KeyCode::KeyG, false);
        assert_eq!((w.ghosts(), count(&w, step)), (2, 0));
        let (sim, shown, prev) = (w.sim.clone(), w.shown().clone(), w.prev.clone());
        w.key(KeyCode::KeyG, false);
        w.key(KeyCode::KeyS, false);
        assert_eq!(w.ghosts(), 2);
        assert_eq!((&w.sim, w.shown(), &w.prev), (&sim, &shown, &prev));
        assert_eq!(w.sim, after_spending(&ghost0, step, 2));
        stocked(&mut w, step, 1);
        w.key(KeyCode::KeyS, false);
        assert_eq!((w.ghosts(), count(&w, step)), (1, 0));
        assert_eq!(*w.shown(), w.sim.replay(1));
    }

    #[test]
    fn an_insert_spends_the_token_is_refused_at_zero_and_backspace_returns_it() {
        let grab = Item::Token(Instr::Grab);
        let mut w = armed(vec![]);
        w.sim.inventory = sim::Inventory::EMPTY;
        stocked(&mut w, grab, 1);
        w.focus_tape(0);
        w.key(KeyCode::KeyF, false);
        assert_eq!(w.sim.arms[0].tape, vec![Instr::Grab]);
        assert_eq!(count(&w, grab), 0);
        assert_eq!(w.focus, Some(Focus::Tape { arm: 0, cursor: 1 }));
        let before = w.sim.clone();
        w.key(KeyCode::KeyF, false);
        w.key(KeyCode::KeyD, false);
        assert_eq!(w.sim, before);
        assert_eq!(w.focus, Some(Focus::Tape { arm: 0, cursor: 1 }));
        w.key(KeyCode::Backspace, false);
        assert_eq!(w.sim.arms[0].tape, vec![]);
        assert_eq!(count(&w, grab), 1);
        assert_eq!(w.focus, Some(Focus::Tape { arm: 0, cursor: 0 }));
        w.key(KeyCode::KeyF, false);
        w.key(KeyCode::KeyZ, false);
        assert_eq!((w.sim.arms[0].tape.len(), count(&w, grab)), (0, 1));
    }

    #[test]
    fn a_refused_insert_at_ghost_n_leaves_ghost0_and_the_ghost_as_they_were() {
        let mut w = paused(2);
        w.sim.inventory = sim::Inventory::EMPTY;
        let (ghost0, ghost2) = (w.sim.clone(), w.shown().clone());
        w.focus_tape(0);
        w.key(KeyCode::KeyF, false);
        assert_eq!((&w.sim, w.shown()), (&ghost0, &ghost2));
        stocked(&mut w, Item::Token(Instr::Grab), 1);
        w.key(KeyCode::KeyF, false);
        assert_eq!(count(&w, Item::Token(Instr::Grab)), 0);
        assert_eq!(*w.shown(), w.sim.replay(2));
        assert_ne!(*w.shown(), ghost2);
    }

    #[test]
    fn deleting_or_cutting_an_arm_returns_the_arm_and_every_token_on_its_tape() {
        let tape = vec![
            Instr::Grab,
            Instr::Rot(Spin::Cw),
            Instr::Move(0),
            Instr::Grab,
        ];
        let counts = |w: &World| {
            (
                count(w, Machine::Arm),
                count(w, Item::Token(Instr::Grab)),
                count(w, Item::Token(Instr::Rot(Spin::Cw))),
                count(w, Item::Token(Instr::Move(0))),
                count(w, Item::Token(Instr::Wait)),
            )
        };
        let mut w = armed(tape.clone());
        w.sim.inventory = sim::Inventory::EMPTY;
        w.pick(vec![Id::Arm(0)]);
        w.key(KeyCode::KeyZ, false);
        assert!(w.sim.arms.is_empty());
        assert_eq!(counts(&w), (1, 2, 1, 1, 0));
        let mut w = armed(tape);
        w.sim.inventory = sim::Inventory::EMPTY;
        w.pick(vec![Id::Arm(0)]);
        w.key(KeyCode::KeyX, false);
        assert!(w.sim.arms.is_empty());
        assert_eq!(counts(&w), (1, 2, 1, 1, 0));
        assert!(w.clipboard.is_some());
    }

    #[test]
    fn a_craft_inside_a_ghost_frame_never_reaches_the_canonical_count() {
        let step = Item::Step;
        let mut w = lone(
            vec![Glyph::new(GlyphKind::Output(Tier::One), Hex::new(4, 4), 0)],
            vec![],
        );
        w.sim.spawn(Atom {
            kind: AtomKind::Base,
            pos: Hex::new(4, 4),
        });
        w.running = false;
        stocked(&mut w, step, 1);
        w.key(KeyCode::KeyG, false);
        assert_eq!(w.ghosts(), 1);
        assert_eq!(count(&w, step), 0);
        assert_eq!(w.shown().inventory.count(step), Some(1));
        assert!(w.shown().atoms.iter().flatten().next().is_none());
        assert!(w.sim.atoms.iter().flatten().next().is_some());
        w.key(KeyCode::Space, false);
        assert_eq!(count(&w, step), 0);
        w.step();
        assert_eq!(count(&w, step), 1);
    }

    #[test]
    fn the_spend_scene_runs_three_steps_refuses_the_fourth_and_returns_the_erased_grab() {
        let w = played("spend", 150);
        assert_eq!(w.ghosts(), 3);
        assert_eq!(count(&w, Item::Step), 0);
        let w = played("spend", 220);
        assert_eq!(w.sim.arms[0].tape, vec![Instr::Grab]);
        assert_eq!(count(&w, Item::Token(Instr::Grab)), 0);
        let w = played("spend", 260);
        assert_eq!(w.sim.arms[0].tape, vec![]);
        assert_eq!(count(&w, Item::Token(Instr::Grab)), 1);
        assert_eq!(w.ghosts(), 3);
    }

    const BONDER: Item = Item::Machine(Machine::Glyph(GlyphKind::Bonder));
    const ARM: Item = Item::Machine(Machine::Arm);
    const GRAB: Item = Item::Token(Instr::Grab);

    const SECOND: Item = Item::Machine(Machine::Glyph(GlyphKind::SecondBond));
    const SECOND_AT: Hex = Hex::new(0, -3);

    fn copied() -> World {
        let arm = Arm::new(Hex::new(3, 0), 0, vec![Instr::Grab, Instr::Grab]);
        let second = Glyph::new(GlyphKind::SecondBond, SECOND_AT, 0);
        let mut w = lone(vec![bonder(ORIGIN, 0), second], vec![arm]);
        w.running = false;
        w.pick(vec![Id::Glyph(0), Id::Glyph(1), Id::Arm(0)]);
        w.key(KeyCode::KeyC, false);
        w
    }

    fn counts(w: &World) -> (u32, u32, u32, u32) {
        (
            count(w, BONDER),
            count(w, SECOND),
            count(w, ARM),
            count(w, GRAB),
        )
    }

    fn pasted(w: &mut World, at: Hex) {
        w.key(KeyCode::KeyV, false);
        w.press(px(at), px(at));
    }

    fn short(item: Item, have: u32, need: u32) -> Short {
        Short { item, have, need }
    }

    #[test]
    fn a_paste_pays_its_whole_bill_and_a_short_one_is_refused_whole_naming_the_short_items() {
        let at = Hex::new(0, 6);
        let mut w = copied();
        for (item, n) in [(BONDER, 1), (SECOND, 1), (ARM, 1), (GRAB, 1)] {
            stocked(&mut w, item, n);
        }
        let before = w.sim.clone();
        pasted(&mut w, at);
        assert_eq!(w.sim, before);
        assert_eq!(w.focus, None);
        let refused = |short: Vec<Short>| Some(Refused { at, short });
        assert_eq!(w.refused, refused(vec![short(GRAB, 1, 2)]));
        assert!(w.sim.inventory.spend(BONDER));
        pasted(&mut w, at);
        assert_eq!(
            w.sim.inventory,
            after_spending(&before, BONDER, 1).inventory
        );
        assert_eq!(w.sim.glyphs, before.glyphs);
        assert_eq!(w.sim.arms, before.arms);
        assert_eq!(
            w.refused,
            refused(vec![short(BONDER, 0, 1), short(GRAB, 1, 2)])
        );
        stocked(&mut w, BONDER, 1);
        stocked(&mut w, GRAB, 1);
        pasted(&mut w, at);
        assert_eq!(w.refused, None);
        assert_eq!(w.sim.arms.len(), 2);
        assert_eq!(w.sim.arms[1].tape, vec![Instr::Grab, Instr::Grab]);
        let second = |at: Hex| Some(Glyph::new(GlyphKind::SecondBond, at, 0));
        assert_eq!(w.sim.glyphs[2], Some(bonder(at, 0)));
        assert_eq!(w.sim.glyphs[3], second(at.add(SECOND_AT)));
        assert_eq!(counts(&w), (0, 0, 0, 0));
        let pasted = [Id::Arm(1), Id::Glyph(2), Id::Glyph(3)];
        assert_eq!(w.focus, picked(&pasted));
        let over = at.add(DIRS[0]);
        drag(&mut w, at, over);
        assert_eq!(w.sim.glyphs[2], Some(bonder(over, 0)));
        assert_eq!(counts(&w), (0, 0, 0, 0));
        assert_eq!(w.refused, None);
        assert_eq!(w.focus, picked(&pasted));
        w.key(KeyCode::KeyZ, false);
        assert_eq!(counts(&w), (1, 1, 1, 2));
        assert_eq!(w.sim.arms.len(), 1);
        assert_eq!(
            w.sim.glyphs,
            vec![Some(bonder(ORIGIN, 0)), second(SECOND_AT), None, None]
        );
    }

    #[test]
    fn a_compound_paste_pays_for_its_atoms_and_double_bonds_or_changes_nothing() {
        let base = Item::Atom(AtomKind::Base);
        let text = "B0,0 B1,0 0,0=1,0";
        let at = Hex::new(4, 4);
        let mut affordable = lone(vec![], vec![]);
        stocked(&mut affordable, base, 3);
        assert!(affordable.paste_text(text));
        affordable.place(Some(at));
        assert_eq!(count(&affordable, base), 0);
        assert_eq!(affordable.sim.atoms.iter().flatten().count(), 2);
        assert_eq!(affordable.sim.bonds.len(), 1);
        assert_eq!(affordable.sim.bonds[0].kind, BondKind::Double);

        let mut unaffordable = lone(vec![], vec![]);
        stocked(&mut unaffordable, base, 2);
        let before = unaffordable.sim.clone();
        assert!(unaffordable.paste_text(text));
        unaffordable.place(Some(at));
        assert_eq!(unaffordable.sim, before);
        assert_eq!(
            unaffordable.refused,
            Some(Refused {
                at,
                short: vec![short(base, 2, 3)]
            })
        );
    }

    #[test]
    fn an_atom_drag_from_the_palette_places_one_atom_and_spends_one() {
        let item = Item::Atom(AtomKind::Amber);
        let at = Hex::new(4, 4);
        let mut w = lone(vec![], vec![]);
        stocked(&mut w, item, 1);
        w.lift_inventory(item);
        w.place(Some(at));
        assert_eq!(count(&w, item), 0);
        assert_eq!(
            w.sim.atom_at(at).map(|id| w.sim.atoms[id].unwrap().kind),
            Some(AtomKind::Amber)
        );
    }

    #[test]
    fn a_compound_paste_is_refused_on_an_atom_and_beyond_ghost_zero() {
        let base = Item::Atom(AtomKind::Base);
        let text = "B0,0 B1,0 0,0-1,0";
        let at = Hex::new(4, 4);
        let mut occupied = lone(vec![], vec![]);
        stocked(&mut occupied, base, 4);
        occupied.sim.spawn(Atom {
            kind: AtomKind::Base,
            pos: at,
        });
        let before = occupied.sim.clone();
        assert!(occupied.paste_text(text));
        occupied.place(Some(at));
        assert_eq!(occupied.sim, before);
        assert_eq!(occupied.refused, None);

        let mut on_base = lone(vec![], vec![Arm::new(at, 0, vec![])]);
        stocked(&mut on_base, base, 2);
        let before = on_base.sim.clone();
        assert!(on_base.paste_text(text));
        on_base.place(Some(at));
        assert_eq!(on_base.sim, before);
        assert_eq!(count(&on_base, base), 2);

        let mut ghost = lone(vec![], vec![]);
        stocked(&mut ghost, base, 2);
        ghost.resim(1);
        let before = ghost.sim.clone();
        assert!(ghost.paste_text(text));
        ghost.place(Some(at));
        assert_eq!(ghost.sim, before);
        assert_eq!(count(&ghost, base), 2);
    }

    #[test]
    fn the_refusal_line_goes_on_the_next_press_and_on_the_next_key() {
        let at = Hex::new(0, 6);
        let mut w = copied();
        let before = w.sim.clone();
        let line = Some(Refused {
            at,
            short: vec![
                short(BONDER, 0, 1),
                short(SECOND, 0, 1),
                short(ARM, 0, 1),
                short(GRAB, 0, 2),
            ],
        });
        pasted(&mut w, at);
        assert_eq!(w.sim, before);
        assert_eq!(w.focus, None);
        assert_eq!(w.refused, line);
        let ground = Hex::new(6, 6);
        w.press(px(ground), px(ground));
        assert_eq!(w.refused, None);
        w.release(Some(ground));
        pasted(&mut w, at);
        assert_eq!(w.refused, line);
        w.key(KeyCode::Escape, false);
        assert_eq!(w.refused, None);
    }

    #[test]
    fn the_refusal_line_shows_each_shortfall_as_beads_and_writes_nothing() {
        let _render = RENDER_TEST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut w = copied();
        for item in [SECOND, ARM, GRAB] {
            stocked(&mut w, item, 1);
        }
        pasted(&mut w, Hex::new(0, 6));
        assert_eq!(
            w.refused.as_ref().map(|r| r.short.clone()),
            Some(vec![short(BONDER, 0, 1), short(GRAB, 1, 2)])
        );
        let dir = std::env::temp_dir().join(format!("ziral-refusal-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = shot::still("wide", dir.clone(), 1);
        lit_plugin(&mut app);
        app.insert_resource(w);
        let seen = std::sync::Arc::new(std::sync::Mutex::new((0, 0, Vec::new())));
        let probe = seen.clone();
        app.add_systems(
            Last,
            move |texts: Query<&Text>,
                  line: Single<&Children, With<Refusal>>,
                  rows: Query<&Children>,
                  marks: Query<&BackgroundColor>| {
                let beads = line
                    .iter()
                    .filter_map(|child| rows.get(child).ok())
                    .map(|row| {
                        let filled = row
                            .iter()
                            .filter(|m| marks.get(*m).is_ok_and(|b| b.0 == IVORY))
                            .count();
                        (filled, row.len())
                    })
                    .collect::<Vec<_>>();
                let written = line
                    .iter()
                    .flat_map(|child| {
                        std::iter::once(child).chain(rows.get(child).into_iter().flatten().copied())
                    })
                    .filter(|e| texts.get(*e).is_ok())
                    .count();
                *probe.lock().unwrap() = (written, line.len(), beads);
            },
        );
        assert_eq!(app.run(), bevy::app::AppExit::Success);
        std::fs::remove_dir_all(&dir).unwrap();
        let (written, children, beads) = seen.lock().unwrap().clone();
        assert_eq!((written, children), (0, 4));
        assert_eq!(beads, vec![(0, 1), (1, 2)]);
    }

    #[test]
    fn a_viewport_maps_screen_to_world_and_back() {
        let v = Viewport {
            cam: Vec2::new(30.0, -70.0),
            size: Vec2::new(1280.0, 720.0),
            scale: 0.5,
        };
        for p in [Vec2::ZERO, Vec2::new(1280.0, 720.0), Vec2::new(17.0, 400.0)] {
            assert_eq!(v.screen(v.world(p)), p);
        }
        assert_eq!(v.screen(v.cam), Vec2::new(640.0, 360.0));
    }

    #[test]
    fn the_refuse_scene_refuses_its_first_paste_and_pays_for_its_second_after_a_delete() {
        let w = played("refuse", 120);
        assert_eq!(w.sim.glyphs, vec![Some(bonder(Hex::new(-2, 0), 0))]);
        assert_eq!(w.sim.arms.len(), 1);
        assert_eq!((count(&w, ARM), count(&w, GRAB)), (1, 1));
        assert_eq!(
            w.refused,
            Some(Refused {
                at: Hex::new(2, -4),
                short: vec![short(BONDER, 0, 1)],
            })
        );
        let w = played("refuse", 160);
        assert_eq!(w.refused, None);
        let w = played("refuse", 200);
        assert_eq!(w.sim.glyphs, vec![None]);
        assert_eq!(count(&w, BONDER), 1);
        let w = played("refuse", 240);
        assert_eq!(w.refused, None);
        assert_eq!(w.sim.glyphs, vec![Some(bonder(Hex::new(-1, -4), 0))]);
        assert_eq!(w.sim.arms.len(), 2);
        assert_eq!(w.sim.arms[1].tape, vec![Instr::Grab]);
        assert_eq!(
            (count(&w, BONDER), count(&w, ARM), count(&w, GRAB)),
            (0, 0, 0)
        );
    }
}
