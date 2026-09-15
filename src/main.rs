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
use bevy::camera::visibility::RenderLayers;
use bevy::camera::{RenderTarget, ScalingMode};
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
use form::{Form, Fragment, atom_machine, recipes};
use look::{Finish, Glaze, HEX, Look, MANUAL, MachineMark, Shape, Skin, px, skin};
use sim::{
    Arm, BondKind, DIRS, Fixture, Glyph, GlyphKind, Hex, Id, Instr, Item, Machine, ORIGIN, Short,
    Sim, Spin, Stall, fixture,
};

const TICK_MS: f32 = 400.0;
const MOTION: f32 = 1.0;
const TURN_MOTION: f32 = 0.6;
const MICRO_SCALE: f32 = 0.5;
const FOCUS: Hex = Hex::new(0, -1);
const MAX_GRID_CELLS: f32 = 6000.0;
const STRIP_ROWS: usize = 8;
const DRAG_PX: f32 = 6.0;
const ATOM_RADIUS: f32 = HEX * 0.4;
const LINE_PX: f32 = 3.0;
const SYMBOL_PX: f32 = 26.0;
const MANUAL_PX: f32 = 1024.0;
const MANUAL_SYMBOL_PX: f32 = 128.0;
const CURSOR_PX: f32 = 2.0;
const PALETTE_PX: f32 = 48.0;
const MARK_PX: f32 = 6.0;
const TALLY_PX: f32 = 70.0;
const PALETTE_GAP_PX: f32 = 8.0;
const PALETTE_ROW_GAP_PX: f32 = 6.0;
const PALETTE_ROW_PAD_X: f32 = 8.0;
const BORDER_PX: f32 = 1.0;
const PALETTE_WIDTH: f32 = 2.0
    * (PALETTE_PX + PALETTE_ROW_GAP_PX + TALLY_PX + 2.0 * (PALETTE_ROW_PAD_X + BORDER_PX))
    + PALETTE_GAP_PX;
const CARD_PAD: f32 = 12.0;
const CARD: RenderLayers = RenderLayers::layer(1);
const CARD_PITCH: f32 = 1024.0;
const CARD_BORDER_PX: f32 = 2.0;
const HOVER_SLOT: usize = 0;
const INVENTORY_TOKEN: &str = "5J7bZuSjtiUSsQg-OdG3iiu3";

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

fn zoomed(scale: f32, notches: f32) -> f32 {
    scale * (notches * 0.15).exp()
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
    const fn new(code: KeyCode, instr: Instr, mut symbol: Skin) -> Key {
        symbol.finish = Finish::Sprite;
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

const MANUAL_SLOTS: [(Instr, f32, f32); 13] = [
    (Instr::Grab, 96.0, 64.0),
    (Instr::Drop, 544.0, 64.0),
    (Instr::Rot(Spin::Ccw), 96.0, 200.0),
    (Instr::Rot(Spin::Cw), 544.0, 200.0),
    (Instr::Pivot(Spin::Ccw), 96.0, 336.0),
    (Instr::Pivot(Spin::Cw), 544.0, 336.0),
    (Instr::Wait, 96.0, 472.0),
    (Instr::Move(UPPER_LEFT), 96.0, 608.0),
    (Instr::Move((UPPER_LEFT + 1) % 6), 544.0, 608.0),
    (Instr::Move((UPPER_LEFT + 2) % 6), 96.0, 744.0),
    (Instr::Move((UPPER_LEFT + 3) % 6), 544.0, 744.0),
    (Instr::Move((UPPER_LEFT + 4) % 6), 96.0, 880.0),
    (Instr::Move((UPPER_LEFT + 5) % 6), 544.0, 880.0),
];

fn instruction_symbol(instr: Instr) -> Skin {
    KEYS.iter()
        .find(|k| k.instr == instr)
        .map(|key| key.symbol)
        .unwrap_or_else(|| panic!("no symbol draws {instr:?}"))
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
        Item::Token(_) => {}
    }
    set
}

fn machines(ids: &[Id]) -> Vec<Id> {
    ids.iter()
        .copied()
        .filter(|id| !matches!(id, Id::Atom(_)))
        .collect()
}

fn held_machine_poses(set: &Sim, pointer: Vec2) -> Vec<(Machine, Vec2, usize)> {
    set.glyphs
        .iter()
        .flatten()
        .map(|g| (Machine::Glyph(g.kind), pointer + px(g.at), g.dir))
        .chain(
            set.arms
                .iter()
                .map(|a| (Machine::Arm, pointer + px(a.pivot), a.dir)),
        )
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MachinePose {
    item: Machine,
    at: Vec2,
    angle: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TurnTarget {
    Board(Id),
    Held,
}

#[derive(Clone, Debug, PartialEq)]
struct FacingTween {
    target: TurnTarget,
    centre: Vec2,
    cell: Hex,
    from: Vec<MachinePose>,
    angle: f32,
    resume: Option<f32>,
}

impl FacingTween {
    fn progress(&self, t: f32) -> f32 {
        Swing::from_cell(self.cell).at(t)
    }

    fn poses(&self, t: f32, centre: Vec2) -> Vec<MachinePose> {
        let progress = self.progress(t);
        let angle = self.angle * progress;
        self.from
            .iter()
            .map(|pose| MachinePose {
                item: pose.item,
                at: match self.target {
                    TurnTarget::Board(_) => pose.at.lerp(centre, progress),
                    TurnTarget::Held => {
                        sweep(self.centre, angle, 1.0)(pose.at) + centre - self.centre
                    }
                },
                angle: pose.angle + angle,
            })
            .collect()
    }
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
    Pick {
        ids: Vec<Id>,
        sim: Box<Sim>,
        prev: Box<Sim>,
        ghost: Option<Box<Sim>>,
        events: Vec<sim::TickEvents>,
    },
    Cell {
        cell: Hex,
        turns: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Focus {
    Pick(Vec<Id>),
    Tape { arm: usize, cursor: usize },
    Hold { set: Box<Sim>, back: Back },
    Card(u64),
}

impl Focus {
    fn picks(&self, id: Id) -> bool {
        match self {
            Focus::Pick(ids) => ids.contains(&id),
            Focus::Tape { arm, .. } => id == Id::Arm(*arm),
            Focus::Hold { .. } | Focus::Card(_) => false,
        }
    }

    fn picked(&self) -> Vec<Id> {
        match self {
            Focus::Pick(ids) => ids.clone(),
            Focus::Tape { arm, .. } => vec![Id::Arm(*arm)],
            Focus::Hold { .. } | Focus::Card(_) => Vec::new(),
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
    Atom { screen: Vec2, id: usize },
    Cell { screen: Vec2, cell: Hex },
    Ground { screen: Vec2, world: Vec2 },
    Marquee { from: Vec2 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Refused {
    at: Hex,
    short: Vec<Short>,
}

#[derive(Clone, Debug, PartialEq)]
struct Pinned {
    id: u64,
    item: Item,
    anchor: Vec2,
    scale: f32,
}

impl Pinned {
    fn screen_rect(&self, viewport: &Viewport) -> (Vec2, Vec2) {
        let size = card_size(self.item) * self.scale;
        (viewport.screen(self.anchor) - size / 2.0, size)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum CardDrag {
    Move {
        id: u64,
        start: Vec2,
        moved: bool,
        button: MouseButton,
    },
    New {
        id: u64,
        button: MouseButton,
    },
}

impl CardDrag {
    fn button(self) -> MouseButton {
        match self {
            CardDrag::New { button, .. } | CardDrag::Move { button, .. } => button,
        }
    }
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
    pointer: Option<Vec2>,
    over_ui: bool,
    hover: Option<Item>,
    palette_hover: Option<Item>,
    play: Option<Play>,
    pinned: Vec<Pinned>,
    pinned_play: Vec<Play>,
    next_card: u64,
    card_drag: Option<CardDrag>,
    events: Vec<sim::TickEvents>,
    score: Option<sim::TickEvents>,
    refused: Option<Refused>,
    turn: Option<FacingTween>,
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
            pointer: None,
            over_ui: false,
            hover: None,
            palette_hover: None,
            play: None,
            pinned: Vec::new(),
            pinned_play: Vec::new(),
            next_card: 0,
            card_drag: None,
            events: Vec::new(),
            score: None,
            refused: None,
            turn: None,
        }
    }

    fn advance(&mut self, dt: f32) {
        if !self.has_machine_rollback() || self.turn.is_some() {
            self.since = (self.since + dt).min(self.period);
        }
        if self.turn.is_some() && self.turn_phase() >= 1.0 {
            self.end_turn();
        }
        if self.running && self.saveable() && self.since >= self.period {
            self.since = 0.0;
            self.step();
        }
        if let Some(play) = &mut self.play {
            play.advance(dt, self.period);
        }
        for play in &mut self.pinned_play {
            play.advance(dt, self.period);
        }
    }

    fn pin_world(&mut self, item: Item, anchor: Vec2) -> u64 {
        let item = card_item(item);
        let id = self.next_card;
        self.next_card += 1;
        if let Item::Machine(machine) = item
            && self.pinned_play.iter().all(|play| play.machine != machine)
        {
            self.pinned_play.push(Play::at(machine, 0));
        }
        self.pinned.push(Pinned {
            id,
            item,
            anchor,
            scale: 1.0,
        });
        self.focus = Some(Focus::Card(id));
        id
    }

    fn begin_pin(&mut self, item: Item, pointer: Vec2, viewport: &Viewport, button: MouseButton) {
        if self.holding() || self.down.is_some() {
            return;
        }
        let id = self.pin_world(item, viewport.world(pointer));
        self.card_drag = Some(CardDrag::New { id, button });
    }

    fn press_inventory(&mut self, item: Item, pointer: Vec2, viewport: &Viewport) {
        if self.sim.inventory.count(item) == Some(0) {
            self.begin_pin(item, pointer, viewport, MouseButton::Left);
        } else {
            self.lift_inventory(item);
        }
    }

    fn card_press(&mut self, id: u64, pointer: Vec2, button: MouseButton) {
        if self.holding() || self.down.is_some() || self.card_drag.is_some() {
            return;
        }
        if self.pinned.iter().all(|card| card.id != id) {
            return;
        }
        self.focus = Some(Focus::Card(id));
        self.card_drag = Some(CardDrag::Move {
            id,
            start: pointer,
            moved: false,
            button,
        });
    }

    fn card_drag(&mut self, pointer: Vec2, viewport: &Viewport) {
        let Some(drag) = self.card_drag else { return };
        let (id, moving) = match drag {
            CardDrag::Move {
                id, start, moved, ..
            } => (id, moved || pointer != start),
            CardDrag::New { id, .. } => (id, true),
        };
        if !moving {
            return;
        }
        let Some(card) = self.pinned.iter_mut().find(|card| card.id == id) else {
            self.card_drag = None;
            return;
        };
        card.anchor = viewport.world(pointer);
        if let Some(CardDrag::Move { moved, .. }) = &mut self.card_drag {
            *moved = true;
        }
    }

    fn end_card_drag(&mut self, pointer: Option<Vec2>, viewport: &Viewport) -> Option<CardDrag> {
        if let Some(pointer) = pointer {
            self.card_drag(pointer, viewport);
        }
        self.card_drag.take()
    }

    fn card_at(&self, pointer: Vec2, viewport: &Viewport) -> Option<u64> {
        let covers = |card: &Pinned| {
            let (at, size) = card.screen_rect(viewport);
            let rim = Vec2::splat(CARD_BORDER_PX);
            pointer.cmpge(at - rim).all() && pointer.cmple(at + size + rim).all()
        };
        let focused = match self.focus {
            Some(Focus::Card(id)) => Some(id),
            _ => None,
        };
        focused
            .filter(|id| {
                self.pinned
                    .iter()
                    .any(|card| card.id == *id && covers(card))
            })
            .or_else(|| {
                self.pinned
                    .iter()
                    .rev()
                    .find(|card| covers(card))
                    .map(|card| card.id)
            })
    }

    fn resize_card(&mut self, pointer: Vec2, notches: f32, viewport: &Viewport) -> bool {
        let Some(id) = self.card_at(pointer, viewport) else {
            return false;
        };
        let card = self.pinned.iter_mut().find(|card| card.id == id).unwrap();
        let base = card_size(card.item);
        let limit = (viewport.size / base).min_element().max(0.05);
        card.scale = zoomed(card.scale, notches).clamp(0.05, limit);
        true
    }

    fn unpin(&mut self, id: u64) {
        self.pinned.retain(|card| card.id != id);
        self.pinned_play.retain(|play| {
            self.pinned
                .iter()
                .any(|card| card.item == Item::Machine(play.machine))
        });
        self.card_drag = None;
        self.focus = None;
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
        if self.has_machine_rollback() {
            return;
        }
        self.sim.inventory.set_cap(item, notches);
        self.resim(self.ghosts());
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
        if let Some(Press::Atom { id, .. }) = self.down
            && tick.events.iter().any(
                |event| matches!(event, sim::TickEvent::Consumed { atoms, .. } if atoms.contains(&id)),
            )
        {
            self.down = None;
        }
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

    fn turn_phase(&self) -> f32 {
        phase(self.since, self.period, TURN_MOTION)
    }

    fn board_phase(&self) -> f32 {
        self.turn.as_ref().and_then(|turn| turn.resume).map_or_else(
            || self.phase(),
            |since| phase(since, self.period, self.motion),
        )
    }

    fn end_turn(&mut self) {
        if let Some(since) = self.turn.take().and_then(|turn| turn.resume) {
            self.since = since;
        }
    }

    fn resting_poses(&self, target: TurnTarget, centre: Vec2) -> Vec<MachinePose> {
        match target {
            TurnTarget::Board(id) => {
                let (item, at, dir) = match id {
                    Id::Arm(i) => match self.sim.arms.get(i) {
                        Some(arm) => (Machine::Arm, arm.pivot, arm.dir),
                        None => return Vec::new(),
                    },
                    Id::Glyph(i) => match self.sim.glyphs.get(i).copied().flatten() {
                        Some(glyph) => (Machine::Glyph(glyph.kind), glyph.at, glyph.dir),
                        None => return Vec::new(),
                    },
                    Id::Atom(_) => unreachable!("an atom has no facing"),
                };
                vec![MachinePose {
                    item,
                    at: px(at),
                    angle: look::turn(dir),
                }]
            }
            TurnTarget::Held => match &self.focus {
                Some(Focus::Hold { set, .. }) => held_machine_poses(set, centre)
                    .into_iter()
                    .map(|(item, at, dir)| MachinePose {
                        item,
                        at,
                        angle: look::turn(dir),
                    })
                    .collect(),
                _ => Vec::new(),
            },
        }
    }

    fn facing_poses(&self, target: TurnTarget, centre: Vec2) -> Vec<MachinePose> {
        match &self.turn {
            Some(turn) if turn.target == target && self.turn_phase() < 1.0 => {
                turn.poses(self.turn_phase(), centre)
            }
            _ => self.resting_poses(target, centre),
        }
    }

    fn turn_source(&self, target: TurnTarget, centre: Vec2) -> Vec<MachinePose> {
        let TurnTarget::Board(Id::Arm(i)) = target else {
            return self.resting_poses(target, centre);
        };
        let frame = Frame::between(&self.prev, self.shown(), self.phase());
        frame.arms.get(i).map_or_else(Vec::new, |arm| {
            vec![MachinePose {
                item: Machine::Arm,
                at: arm.pivot,
                angle: (arm.hand - arm.pivot).to_angle(),
            }]
        })
    }

    fn begin_turn(&mut self, target: TurnTarget, spin: Spin, centre: Vec2) {
        let phase = self.turn_phase();
        let resume = self
            .turn
            .as_ref()
            .filter(|turn| turn.target == target)
            .and_then(|turn| turn.resume)
            .or_else(|| (target == TurnTarget::Held).then_some(self.since));
        let (from, remaining) = match &self.turn {
            Some(turn) if turn.target == target && phase < 1.0 => (
                turn.poses(phase, centre),
                turn.angle * (1.0 - turn.progress(phase)),
            ),
            _ => {
                let from = self.turn_source(target, centre);
                let remaining = from
                    .first()
                    .zip(self.resting_poses(target, centre).first())
                    .map_or(0.0, |(from, rest)| {
                        Vec2::from_angle(from.angle).angle_to(Vec2::from_angle(rest.angle))
                    });
                (from, remaining)
            }
        };
        let angle = spin_angle(spin);
        self.turn = Some(FacingTween {
            target,
            centre,
            cell: hex_at(centre),
            from,
            angle: remaining + angle,
            resume,
        });
        self.since = 0.0;
    }

    fn board_turn_pose(&self) -> Option<(Id, MachinePose)> {
        let TurnTarget::Board(id) = self.turn.as_ref()?.target else {
            return None;
        };
        let centre = self
            .resting_poses(TurnTarget::Board(id), Vec2::ZERO)
            .first()?
            .at;
        self.facing_poses(TurnTarget::Board(id), centre)
            .into_iter()
            .next()
            .map(|pose| (id, pose))
    }

    fn holding(&self) -> bool {
        matches!(self.focus, Some(Focus::Hold { .. }))
    }

    fn has_machine_rollback(&self) -> bool {
        matches!(
            self.focus,
            Some(Focus::Hold {
                back: Back::Pick { .. },
                ..
            })
        )
    }

    fn saveable(&self) -> bool {
        !matches!(
            self.focus,
            Some(Focus::Hold {
                back: Back::Cell { .. } | Back::Pick { .. },
                ..
            })
        )
    }

    fn snapshot(&self) -> Sim {
        match &self.focus {
            Some(Focus::Hold {
                set,
                back: Back::Cell { cell, turns },
            }) => {
                let mut set = (**set).clone();
                for _ in 0..*turns {
                    turn(&mut set, Spin::Ccw);
                }
                let mut sim = self.sim.clone();
                sim.place(&set, *cell);
                sim
            }
            Some(Focus::Hold {
                back: Back::Pick { sim, .. },
                ..
            }) => (**sim).clone(),
            _ => self.sim.clone(),
        }
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

    fn hit(&self, point: Vec2, frame: &Frame) -> Option<Id> {
        let cell = hex_at(point);
        let changing = frame.sim != self.shown();
        let machine = self
            .hand_ids()
            .filter(|id| self.cells(*id).contains(&cell))
            .min_by_key(|id| {
                let anchor = self.anchor(*id);
                (anchor != cell, anchor.r, anchor.q)
            });
        let mut cell_atom = None;
        let mut body_atom: Option<((i32, i32), Id)> = None;
        for (i, atom) in self.shown().atoms.iter().enumerate() {
            let Some(atom) = atom else { continue };
            let key = (atom.pos.r, atom.pos.q);
            if atom.pos == cell {
                cell_atom = Some(Id::Atom(i));
            }
            if frame
                .atoms
                .get(i)
                .copied()
                .flatten()
                .is_some_and(|at| point.distance(at) <= ATOM_RADIUS)
                && !(changing
                    && self.events.last().is_some_and(|tick| {
                        tick.events.iter().any(|event| {
                            matches!(event, sim::TickEvent::Consumed { atoms, .. } if atoms.contains(&i))
                        })
                    }))
                && body_atom.is_none_or(|(other, _)| key < other)
            {
                body_atom = Some((key, Id::Atom(i)));
            }
        }
        let body_atom = body_atom.map(|(_, id)| id);
        body_atom.or(machine).or(cell_atom)
    }

    fn target_item(&self, point: Vec2, frame: &Frame) -> Option<Item> {
        self.hit(point, frame).map(|id| match id {
            Id::Arm(_) => Item::Machine(Machine::Arm),
            Id::Glyph(i) => Item::Machine(Machine::Glyph(self.glyph(i).kind)),
            Id::Atom(i) => Item::Atom(self.shown().atoms[i].unwrap().kind),
        })
    }

    fn set_hover(&mut self, item: Option<Item>) {
        let item = item.map(card_item);
        if self.hover == item {
            return;
        }
        self.hover = item;
        self.play = match item {
            Some(Item::Machine(machine)) => Some(Play::at(machine, 0)),
            Some(Item::Token(_)) | None => None,
            Some(Item::Atom(_)) => unreachable!("an atom resolves to its route before hover"),
        };
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
                    let mut arm = Arm::new(a.pivot.sub(grab), a.dir, a.tape.clone());
                    arm.pc = a.pc;
                    set.arms.push(arm);
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
        self.end_turn();
        self.remove_from_sim(ids);
        self.focus = None;
        self.down = None;
        self.resim(self.ghosts());
    }

    fn remove_from(sim: &mut Sim, ids: &[Id]) {
        let (mut arms, mut glyphs, mut atoms) = (Vec::new(), Vec::new(), Vec::new());
        for id in ids {
            match id {
                Id::Arm(i) => arms.push(*i),
                Id::Glyph(i) => glyphs.push(*i),
                Id::Atom(i) => atoms.push(*i),
            }
        }
        for i in glyphs {
            sim.glyphs[i] = None;
        }
        sim.consume(&atoms);
        arms.sort_unstable_by(|a, b| b.cmp(a));
        if !arms.is_empty() {
            for arm in &mut sim.arms {
                arm.stall = None;
            }
        }
        for i in arms {
            sim.arms.remove(i);
        }
    }

    fn remove_from_sim(&mut self, ids: &[Id]) {
        Self::remove_from(&mut self.sim, ids);
    }

    fn begin_machine_drag(&mut self, ids: Vec<Id>, cell: Hex) {
        debug_assert!(ids.iter().all(|id| !matches!(id, Id::Atom(_))));
        self.end_turn();
        let set = self.lifted(&ids, cell);
        let sim = Box::new(self.sim.clone());
        let prev = Box::new(self.prev.clone());
        let rollback_ghost = self.ghost.clone().map(Box::new);
        let events = self.events.clone();
        Self::remove_from(&mut self.sim, &ids);
        Self::remove_from(&mut self.prev, &ids);
        if let Some(ghost) = &mut self.ghost {
            Self::remove_from(ghost, &ids);
        }
        self.events.clear();
        self.focus = Some(Focus::Hold {
            set: Box::new(set),
            back: Back::Pick {
                ids,
                sim,
                prev,
                ghost: rollback_ghost,
                events,
            },
        });
        self.down = None;
    }

    fn delete(&mut self, ids: &[Id]) {
        if !self.edits(ids) {
            return;
        }
        let set = self.lifted(&machines(ids), ORIGIN);
        self.return_to_inventory(&set);
        self.remove(ids);
    }

    fn return_to_inventory(&mut self, set: &Sim) {
        for item in set.bill() {
            self.sim.inventory.add(item);
        }
    }

    fn copy(&self, ids: &[Id]) -> Option<String> {
        let machine_ids = machines(ids);
        let mut set = self.lifted(&machine_ids, ORIGIN);
        let sim = self.shown();
        let mut seen: Vec<usize> = Vec::new();
        for id in ids {
            let Id::Atom(i) = id else { continue };
            if seen.contains(i) {
                continue;
            }
            let compound = sim.component(*i);
            seen.extend(&compound);
            set.place(&sim.fragment(&compound, ORIGIN), ORIGIN);
        }
        if machine_ids.is_empty() && seen.is_empty() {
            None
        } else {
            Fragment::of(&set).ok().map(|fragment| fragment.to_string())
        }
    }

    fn paste(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(text) = clipboard_text() {
                self.paste_text(&text);
            }
        }

        #[cfg(target_arch = "wasm32")]
        clipboard_text();
    }

    fn paste_text(&mut self, text: &str) -> bool {
        if self.holding() {
            return false;
        }
        let Ok(fragment) = text.parse::<Fragment>() else {
            return false;
        };
        self.lift(fragment.into_sim(), Back::Inventory);
        true
    }

    fn lift(&mut self, set: Sim, back: Back) {
        if self.holding() {
            return;
        }
        self.end_turn();
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
        let frame = Frame::between(&self.prev, self.shown(), self.phase());
        let target = self.hit(point, &frame);
        let in_pick = target
            .is_some_and(|id| matches!(&self.focus, Some(Focus::Pick(ids)) if ids.contains(&id)));
        match target {
            Some(_) if in_pick => {}
            Some(Id::Arm(arm)) => self.focus_tape(arm),
            Some(id) => self.pick(vec![id]),
            None => {}
        }
        self.down = Some(match target {
            Some(Id::Atom(id)) => Press::Atom { screen, id },
            Some(Id::Arm(_) | Id::Glyph(_)) => Press::Cell { screen, cell },
            None => Press::Ground {
                screen,
                world: point,
            },
        });
    }

    fn drag(&mut self, screen: Vec2) {
        match self.down {
            Some(Press::Atom { screen: start, id }) if start.distance(screen) > DRAG_PX => {
                let Some(cell) = self
                    .shown()
                    .atoms
                    .get(id)
                    .copied()
                    .flatten()
                    .map(|atom| atom.pos)
                else {
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
                self.begin_machine_drag(ids, cell);
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
        self.end_turn();
        let Some(Focus::Hold { set, back }) =
            self.focus.take_if(|f| matches!(f, Focus::Hold { .. }))
        else {
            return;
        };
        let legal = |at: &Hex| {
            back != Back::Ghost && self.editable(runs(&set)) && self.sim.fits(&set, *at, &[])
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
        let ghosts = self.ghosts();
        let ids = match back {
            Back::Pick { ids, sim, .. } => {
                self.sim = *sim;
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
        self.resim(ghosts);
    }

    fn pop(&mut self, mut set: Sim, back: Back) {
        self.end_turn();
        match back {
            Back::Inventory | Back::Ghost => {}
            Back::Pick {
                ids,
                sim,
                prev,
                ghost,
                events,
            } => {
                self.sim = *sim;
                self.prev = *prev;
                self.ghost = ghost.map(|ghost| *ghost);
                self.events = events;
                self.pick(ids);
            }
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
        if matches!(key, Space | KeyG | KeyS) && self.has_machine_rollback() {
            return;
        }
        match key {
            Space => {
                self.running = !self.running;
                if self.running {
                    self.down = None;
                    self.resim(0);
                }
                return;
            }
            KeyG => {
                if !self.running {
                    self.end_turn();
                    self.down = None;
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
                if let (false, Some(n)) = (self.running, self.ghosts().checked_sub(1)) {
                    self.end_turn();
                    self.down = None;
                    self.resim(n);
                }
                return;
            }
            _ => {}
        }
        let instr = instr_of(key, shift);
        if self.holding() {
            if let (Some(Instr::Rot(spin)), Some(pointer)) = (instr, self.pointer) {
                self.begin_turn(TurnTarget::Held, spin, pointer);
            }
            match (key, instr) {
                (Escape, _) => {
                    let Some(Focus::Hold { set, back }) = self.focus.take() else {
                        unreachable!()
                    };
                    self.pop(*set, back);
                }
                (KeyZ, _) => {
                    let deletes = match &self.focus {
                        Some(Focus::Hold {
                            back: Back::Inventory | Back::Ghost,
                            ..
                        }) => true,
                        Some(Focus::Hold {
                            back: Back::Pick { ids, .. },
                            ..
                        }) => self.edits(ids),
                        _ => false,
                    };
                    if deletes {
                        self.end_turn();
                        let Some(Focus::Hold { set, back }) = self.focus.take() else {
                            unreachable!()
                        };
                        if matches!(back, Back::Pick { .. }) {
                            self.return_to_inventory(&set);
                            self.resim(self.ghosts());
                        }
                    }
                }
                (_, Some(Instr::Rot(spin))) => {
                    let Some(Focus::Hold { set, back }) = &mut self.focus else {
                        unreachable!()
                    };
                    turn(set, spin);
                    if let Back::Cell { turns, .. } = back {
                        *turns = spin.turn(*turns);
                    }
                }
                _ => {}
            }
            return;
        }
        match self.focus.clone() {
            Some(Focus::Hold { .. }) => unreachable!(),
            Some(Focus::Pick(ids)) => match key {
                _ if shift => {}
                Escape => self.focus = None,
                KeyZ => self.delete(&ids),
                KeyX | KeyC if !self.edits(&machines(&ids)) => {}
                KeyX | KeyC => {
                    if let Some(text) = self.copy(&ids) {
                        clipboard(&text);
                    }
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
                            self.begin_turn(TurnTarget::Board(*id), spin, px(at));
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
            Some(Focus::Card(id)) => match key {
                Escape => self.focus = None,
                KeyZ => self.unpin(id),
                _ => {}
            },
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
    written.unwrap_or_else(|e| panic!("the clipboard refused the fragment: {e}"));
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
            .unwrap_or_else(|e| panic!("the clipboard refused the fragment: {e:?}"));
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
        if let Some(text) = text {
            *PASTED.lock().unwrap() = Some(text);
        }
    });
}

#[cfg(target_arch = "wasm32")]
static PASTED: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

#[cfg(target_arch = "wasm32")]
fn clipboard_paste(mut world: ResMut<World>) {
    if let Some(text) = PASTED.lock().unwrap().take() {
        world.paste_text(&text);
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn clipboard_paste() {}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
const inventoryFragments = [];
export function listen_inventory_fragment() {
    addEventListener("hashchange", event => {
        inventoryFragments.push(new URL(event.newURL).hash.slice(1));
    });
    inventoryFragments.push(location.hash.slice(1));
}
export function take_inventory_fragment() {
    return inventoryFragments.shift();
}
export function clear_inventory_fragment(fragment) {
    if (location.hash.slice(1) === fragment)
        history.replaceState(null, "", location.pathname + location.search);
}
"#)]
extern "C" {
    fn listen_inventory_fragment();
    fn take_inventory_fragment() -> Option<String>;
    fn clear_inventory_fragment(fragment: &str);
}

#[cfg(not(target_arch = "wasm32"))]
fn listen_inventory_fragment() {}

#[cfg(not(target_arch = "wasm32"))]
fn take_inventory_fragment() -> Option<String> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn clear_inventory_fragment(_: &str) {}

fn consume_inventory_fragment(world: &mut World, fragment: &str, clear: impl FnOnce(&str)) {
    if fragment != INVENTORY_TOKEN {
        return;
    }
    world.sim.inventory.fill();
    clear(fragment);
}

fn refill_inventory(mut world: ResMut<World>) {
    if world.has_machine_rollback() {
        return;
    }
    while let Some(fragment) = take_inventory_fragment() {
        consume_inventory_fragment(&mut world, &fragment, clear_inventory_fragment);
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
        Self::from_size(window.size(), transform, projection)
    }

    fn from_size(size: Vec2, transform: &Transform, projection: &Projection) -> Option<Viewport> {
        let Projection::Orthographic(ortho) = &projection else {
            return None;
        };
        Some(Viewport {
            cam: transform.translation.truncate(),
            size,
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

    fn zoom_about(&mut self, screen: Vec2, scale: f32) {
        let before = self.world(screen);
        self.scale = scale;
        self.cam += before - self.world(screen);
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
        .add_systems(
            Startup,
            (listen_inventory_fragment, fire_kiln, spawn_ui).chain(),
        )
        .add_systems(
            Update,
            (
                clipboard_paste,
                refill_inventory,
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
                render_instruction_symbols,
            )
                .chain(),
        );
    app
}

#[derive(Clone, Copy, Component, PartialEq, Eq)]
enum CardCamera {
    Hover,
    Pin(u64),
}

struct Placement {
    kind: CardCamera,
    item: Item,
    at: Vec2,
    scale: f32,
    order: isize,
}

impl Placement {
    fn aim(
        &self,
        (target, factor): (UVec2, f32),
        (camera, projection, transform): (&mut Camera, &mut Projection, &mut Transform),
    ) {
        let size = card_size(self.item);
        let window = target.as_vec2() / factor;
        let min = self.at.max(Vec2::ZERO);
        let max = (self.at + size * self.scale).min(window);
        let physical_min = (min * factor).round();
        let physical_max = (max * factor).round().min(target.as_vec2());
        if physical_min.cmpge(physical_max).any() {
            camera.is_active = false;
            camera.viewport = None;
            return;
        }
        camera.viewport = Some(bevy::camera::Viewport {
            physical_position: physical_min.as_uvec2(),
            physical_size: (physical_max - physical_min).as_uvec2(),
            ..default()
        });
        camera.order = self.order;
        camera.is_active = true;
        let visible = (max - min) / self.scale;
        *projection = Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::Fixed {
                width: visible.x,
                height: visible.y,
            },
            ..OrthographicProjection::default_2d()
        });
        let slot = match self.kind {
            CardCamera::Hover => HOVER_SLOT,
            CardCamera::Pin(_) => card_slot(self.item),
        };
        let from = (min - self.at) / self.scale;
        let to = (max - self.at) / self.scale;
        let offset = Vec2::new(
            (from.x + to.x - size.x) / 2.0,
            (size.y - from.y - to.y) / 2.0,
        );
        transform.translation = (card_slot_at(slot) + offset).extend(0.0);
    }
}

fn card_camera(kind: CardCamera, target: RenderTarget) -> impl Bundle {
    (
        kind,
        Camera2d,
        Camera {
            is_active: false,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        target,
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
            border: UiRect::all(Val::Px(BORDER_PX)),
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
        Item::Token(instr) => {
            entry.spawn((InstructionSymbol::Ui(instr), square, field));
        }
    }
}

fn picture_square(item: Item) -> (Node, BackgroundColor) {
    let side = match item {
        Item::Machine(_) => PALETTE_PX,
        Item::Atom(_) => PALETTE_PX,
        Item::Token(_) => SYMBOL_PX,
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

#[derive(Clone, Copy, Component)]
enum InstructionSymbol {
    Ui(Instr),
    Card(Instr),
}

impl InstructionSymbol {
    fn instr(self) -> Instr {
        match self {
            InstructionSymbol::Ui(instr) | InstructionSymbol::Card(instr) => instr,
        }
    }
}

fn render_instruction_symbols(
    mut commands: Commands,
    kiln: Res<Kiln>,
    symbols: Query<(Entity, &InstructionSymbol), Added<InstructionSymbol>>,
) {
    for (entity, symbol) in &symbols {
        let skin = instruction_symbol(symbol.instr());
        match symbol {
            InstructionSymbol::Ui(_) => {
                commands
                    .entity(entity)
                    .insert(ImageNode::new(kiln.image(skin)));
            }
            InstructionSymbol::Card(_) => {
                commands.entity(entity).insert((
                    Mesh2d(kiln.bar.clone()),
                    MeshMaterial2d(kiln.skin(skin, false).clone()),
                ));
            }
        }
    }
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

fn mark(row: &mut ChildSpawnerCommands, px: f32, fill: f32, gap: f32) {
    row.spawn((
        Node {
            width: Val::Px(px),
            height: Val::Px(px),
            margin: UiRect::left(Val::Px(gap)),
            border: UiRect::all(Val::Px(1.0)),
            border_radius: BorderRadius::MAX,
            overflow: Overflow::clip(),
            ..default()
        },
        BackgroundColor(if fill == 1.0 { IVORY } else { Color::NONE }),
        BorderColor::all(if fill == 1.0 { IVORY } else { brass(0.5) }),
    ))
    .with_children(|pip| {
        if fill > 0.0 && fill < 1.0 {
            pip.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(1.0),
                    top: Val::Px(1.0),
                    width: Val::Px((px - 2.0) * fill),
                    height: Val::Px(px - 2.0),
                    ..default()
                },
                BackgroundColor(IVORY),
            ));
        }
    });
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
        let cap = if world.palette_hover == Some(tally.item) {
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
    let cap = filled.max(upto).min(sim::MAX_CAP);
    let (full, fraction) = pips(filled, cap);
    let (cap_full, cap_fraction) = pips(cap, cap);
    let slots = cap_full + u32::from(cap_fraction > 0.0);
    for k in 0..slots {
        let fill = if k < full {
            1.0
        } else if k == full {
            fraction
        } else {
            0.0
        };
        mark(row, MARK_PX, fill, 0.0);
    }
}

fn pips(count: u32, cap: u32) -> (u32, f32) {
    let count = count.min(cap);
    if count == 0 {
        return (0, 0.0);
    }
    let full = count.ilog2() + 1;
    let base = 1 << (full - 1);
    (full, (count - base) as f32 / base as f32)
}

fn hover(
    mut world: ResMut<World>,
    window: Single<&Window, With<PrimaryWindow>>,
    rows: Query<(&PaletteRow, &Interaction)>,
    scroll: Res<AccumulatedMouseScroll>,
    mut left: MessageReader<CursorLeft>,
) {
    if left.read().next().is_some() {
        world.set_hover(None);
        world.palette_hover = None;
        world.pointer = None;
    }
    if window.cursor_position().is_some() {
        world.palette_hover = rows
            .iter()
            .find(|(_, i)| **i != Interaction::None)
            .map(|(row, _)| row.0);
    }
    let notches = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    };
    if let (Some(item), true) = (world.palette_hover, notches.round() != 0.0) {
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
        Item::Atom(_) => unreachable!("an atom resolves to its route before sizing"),
        Item::Token(_) => SYMBOL_PX,
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
        Item::Token(_) => None,
        Item::Atom(_) => unreachable!("an atom resolves to its route before layout"),
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

fn card_item(item: Item) -> Item {
    match item {
        Item::Atom(kind) => Item::Machine(atom_machine(kind)),
        item => item,
    }
}

fn card_slot(item: Item) -> usize {
    HOVER_SLOT
        + 1
        + palette()
            .map(card_item)
            .position(|entry| entry == item)
            .unwrap()
}

fn card_slot_at(slot: usize) -> Vec2 {
    Vec2::new(slot as f32 * CARD_PITCH, 0.0)
}

#[derive(Clone, Copy, Component)]
struct PinnedCard(u64);

type CardCameras<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static CardCamera,
        &'static mut Camera,
        &'static mut Projection,
        &'static mut Transform,
    ),
>;

type BoardCamera<'w, 's> = Single<
    'w,
    's,
    (
        &'static Camera,
        &'static RenderTarget,
        &'static Transform,
        &'static Projection,
    ),
    (With<IsDefaultUiCamera>, Without<CardCamera>),
>;

type PinnedNodes<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static PinnedCard,
        &'static mut Node,
        &'static mut BorderColor,
        &'static mut GlobalZIndex,
    ),
>;

fn card(
    mut commands: Commands,
    world: Res<World>,
    board: BoardCamera,
    column: Single<(&ComputedNode, &UiGlobalTransform), With<Palette>>,
    mut cameras: CardCameras,
    mut nodes: PinnedNodes,
) {
    let (board, target, transform, projection) = board.into_inner();
    let (Some(physical), Some(factor)) = (
        board
            .physical_target_size()
            .filter(|size| size.cmpgt(UVec2::ZERO).all()),
        board.target_scaling_factor(),
    ) else {
        return;
    };
    let window = physical.as_vec2() / factor;
    let Some(viewport) = Viewport::from_size(window, transform, projection) else {
        return;
    };
    let hover = world.hover.map(|item| {
        let (column, transform) = column.into_inner();
        let size = card_size(item);
        let top_left = (transform.translation - column.size() / 2.0) / factor;
        let scale = (window / size).min_element().min(1.0);
        let at = Vec2::new(top_left.x, top_left.y - CARD_PAD - size.y * scale)
            .clamp(Vec2::ZERO, (window - size * scale).max(Vec2::ZERO));
        Placement {
            kind: CardCamera::Hover,
            item,
            at,
            scale,
            order: 1,
        }
    });
    let pins: Vec<(u64, Placement)> = world
        .pinned
        .iter()
        .enumerate()
        .map(|(index, card)| {
            let focused = world.focus == Some(Focus::Card(card.id));
            let rank = if focused { world.pinned.len() } else { index };
            let (at, _) = card.screen_rect(&viewport);
            let placed = Placement {
                kind: CardCamera::Pin(card.id),
                item: card.item,
                at,
                scale: card.scale,
                order: 2 + rank as isize,
            };
            (card.id, placed)
        })
        .collect();
    let placed_pin = |id: u64| pins.iter().find(|(pin, _)| *pin == id).map(|(_, p)| p);

    for (entity, kind, mut camera, mut projection, mut transform) in &mut cameras {
        let placed = match kind {
            CardCamera::Hover => hover.as_ref(),
            CardCamera::Pin(id) => placed_pin(*id),
        };
        match (placed, kind) {
            (Some(placed), _) => placed.aim(
                (physical, factor),
                (&mut camera, &mut projection, &mut transform),
            ),
            (None, CardCamera::Hover) => {
                camera.is_active = false;
                camera.viewport = None;
            }
            (None, CardCamera::Pin(_)) => commands.entity(entity).despawn(),
        }
    }
    if !cameras
        .iter()
        .any(|(_, kind, ..)| *kind == CardCamera::Hover)
    {
        commands.spawn(card_camera(CardCamera::Hover, target.clone()));
    }
    for (id, _) in &pins {
        if !cameras
            .iter()
            .any(|(_, kind, ..)| *kind == CardCamera::Pin(*id))
        {
            commands.spawn(card_camera(CardCamera::Pin(*id), target.clone()));
        }
        if !nodes.iter().any(|(_, shown, ..)| shown.0 == *id) {
            commands.spawn((
                PinnedCard(*id),
                Node {
                    position_type: PositionType::Absolute,
                    border: UiRect::all(Val::Px(CARD_BORDER_PX)),
                    ..default()
                },
                BorderColor::all(brass(0.5)),
                GlobalZIndex(2),
            ));
        }
    }
    for (entity, shown, mut node, mut border, mut z) in &mut nodes {
        let Some(placed) = placed_pin(shown.0) else {
            commands.entity(entity).despawn();
            continue;
        };
        let size = card_size(placed.item) * placed.scale;
        node.left = Val::Px(placed.at.x - CARD_BORDER_PX);
        node.top = Val::Px(placed.at.y - CARD_BORDER_PX);
        node.width = Val::Px(size.x + 2.0 * CARD_BORDER_PX);
        node.height = Val::Px(size.y + 2.0 * CARD_BORDER_PX);
        let focused = world.focus == Some(Focus::Card(shown.0));
        *border = BorderColor::all(if focused { IVORY } else { brass(0.5) });
        z.0 = if focused { 3 } else { 2 };
    }
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
                    position_type: PositionType::Relative,
                    width: Val::VMin(92.0),
                    height: Val::VMin(92.0),
                    ..default()
                },
            ))
            .with_children(|manual| {
                for (instr, left, top) in MANUAL_SLOTS {
                    manual.spawn((
                        InstructionSymbol::Ui(instr),
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(100.0 * left / MANUAL_PX),
                            top: Val::Percent(100.0 * top / MANUAL_PX),
                            width: Val::Percent(100.0 * MANUAL_SYMBOL_PX / MANUAL_PX),
                            height: Val::Percent(100.0 * MANUAL_SYMBOL_PX / MANUAL_PX),
                            ..default()
                        },
                    ));
                }
            });
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
                ..row(PALETTE_GAP_PX)
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
                                    padding: UiRect::axes(Val::Px(PALETTE_ROW_PAD_X), Val::Px(2.0)),
                                    ..row(PALETTE_ROW_GAP_PX)
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

    fn advance(&mut self, dt: f32, period: f32) {
        self.since = (self.since + dt).min(period);
        if self.since >= period {
            self.since = 0.0;
            self.step();
        }
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

type ViewInput<'w> = (
    Res<'w, ButtonInput<MouseButton>>,
    Res<'w, AccumulatedMouseScroll>,
    Res<'w, AccumulatedMouseMotion>,
);

fn wheel(
    world: &mut World,
    pointer: Vec2,
    notches: f32,
    viewport: &Viewport,
    board: &mut f32,
) -> bool {
    if world.resize_card(pointer, notches, viewport) {
        true
    } else {
        *board = zoomed(*board, -notches).clamp(0.05, 40.0);
        false
    }
}

fn view(
    mut world: ResMut<World>,
    input: ViewInput,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&mut Transform, &mut Projection), With<IsDefaultUiCamera>>,
    palette: Query<&Interaction, With<PaletteRow>>,
    mut right_pan: Local<bool>,
) {
    let (buttons, scroll, motion) = input;
    let (mut transform, mut projection) = camera.into_inner();
    let Some(mut viewport) = Viewport::of(&window, &transform, &projection) else {
        return;
    };
    let Projection::Orthographic(ortho) = &mut *projection else {
        return;
    };
    if scroll.delta.y != 0.0
        && world.palette_hover.is_none()
        && let Some(c) = window.cursor_position()
    {
        let notches = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
        };
        let card = wheel(&mut world, c, notches, &viewport, &mut ortho.scale);
        if !card {
            viewport.zoom_about(c, ortho.scale);
            transform.translation = viewport.cam.extend(transform.translation.z);
        }
    }
    if buttons.just_pressed(MouseButton::Right) {
        *right_pan = world.card_drag.is_none()
            && palette
                .iter()
                .all(|interaction| *interaction == Interaction::None)
            && window
                .cursor_position()
                .is_none_or(|c| world.card_at(c, &viewport).is_none());
    }
    if buttons.just_released(MouseButton::Right) {
        *right_pan = false;
    }
    if board_pan_active(&buttons, *right_pan) {
        let pan = Vec2::new(-motion.delta.x, motion.delta.y);
        transform.translation += (pan * ortho.scale).extend(0.0);
    }
}

fn board_pan_active(buttons: &ButtonInput<MouseButton>, right_pan: bool) -> bool {
    !buttons.just_pressed(MouseButton::Right)
        && (buttons.pressed(MouseButton::Middle)
            || buttons.pressed(MouseButton::Right) && right_pan)
}

type EditUi<'w, 's> = (
    Query<
        'w,
        's,
        (
            Option<&'static PaletteRow>,
            Option<&'static TapeRow>,
            &'static Interaction,
        ),
    >,
    Query<
        'w,
        's,
        (
            &'static PaletteRow,
            &'static ComputedNode,
            &'static UiGlobalTransform,
        ),
    >,
    Query<
        'w,
        's,
        (
            &'static ComputedNode,
            &'static UiGlobalTransform,
            &'static InheritedVisibility,
        ),
        Or<(With<PaletteRow>, With<TapeRow>, With<SaveAction>)>,
    >,
    Query<'w, 's, &'static Interaction, With<SaveAction>>,
);

fn edit(
    mut world: ResMut<World>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Transform, &Projection), With<IsDefaultUiCamera>>,
    edit_ui: EditUi,
) {
    let (ui, inventory, panels, save) = edit_ui;
    let (transform, projection) = camera.into_inner();
    let Some(viewport) = Viewport::of(&window, transform, projection) else {
        return;
    };
    let screen = window.cursor_position();
    if let Some(c) = screen {
        world.pointer = Some(viewport.world(c));
    }
    let covered = screen.is_some_and(|point| world.card_at(point, &viewport).is_some());
    let physical = screen.map(|point| point * window.scale_factor());
    let inventory_at = physical
        .filter(|_| !covered && !keys.pressed(KeyCode::Tab))
        .and_then(|point| {
            inventory
                .iter()
                .find(|(_, node, transform)| node.contains_point(**transform, point))
                .map(|(row, _, _)| row.0)
        });
    let over_panel = keys.pressed(KeyCode::Tab)
        || physical.is_some_and(|point| {
            panels.iter().any(|(node, transform, visible)| {
                visible.get() && node.contains_point(*transform, point)
            })
        })
        || ui.iter().any(|(_, _, i)| *i != Interaction::None)
        || save.iter().any(|i| *i != Interaction::None);
    let over_ui = over_panel || covered || inventory_at.is_some();
    world.over_ui = over_ui;
    let at = world.pointer.map(hex_at);

    if buttons.just_pressed(MouseButton::Right)
        && world.card_drag.is_none()
        && let Some(pointer) = screen
    {
        if let Some(item) = inventory_at {
            world.begin_pin(item, pointer, &viewport, MouseButton::Right);
        } else if let Some(id) = world.card_at(pointer, &viewport) {
            world.card_press(id, pointer, MouseButton::Right);
        }
    }
    if buttons.just_pressed(MouseButton::Left)
        && !matches!(world.card_drag, Some(CardDrag::New { .. }))
    {
        let pressed = ui.iter().find(|(_, _, i)| **i == Interaction::Pressed);
        if let Some(id) = screen.and_then(|pointer| world.card_at(pointer, &viewport)) {
            world.card_press(id, screen.unwrap(), MouseButton::Left);
        } else if let Some((Some(entry), _, _)) = pressed {
            if let Some(pointer) = screen {
                world.press_inventory(entry.0, pointer, &viewport);
            }
        } else if let Some((_, Some(row), _)) = pressed {
            if let Some(arm) = row.arm().filter(|a| *a < world.shown().arms.len()) {
                world.focus_tape(arm);
            }
        } else if !over_ui && let (Some(c), Some(p)) = (screen, world.pointer) {
            world.press(c, p);
        }
    }
    if (buttons.pressed(MouseButton::Left) || buttons.pressed(MouseButton::Right))
        && let Some(c) = screen
    {
        if world.card_drag.is_some() {
            world.card_drag(c, &viewport);
        } else {
            world.drag(c);
        }
    }
    if world
        .card_drag
        .is_some_and(|drag| buttons.just_released(drag.button()))
    {
        let drag = world.end_card_drag(screen, &viewport).unwrap();
        if let (CardDrag::New { id, .. }, true) = (drag, over_panel) {
            world.unpin(id);
        }
    } else if buttons.just_released(MouseButton::Left) && world.card_drag.is_none() {
        let valid = !over_ui && screen.is_some();
        world.release(at.filter(|_| valid));
    }

    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let mut pressed: Vec<KeyCode> = keys.get_just_pressed().copied().collect();
    pressed.sort_unstable();
    for key in pressed {
        world.key(key, shift);
    }
    if screen.is_some() {
        let item = if let Some(item) = inventory_at {
            Some(item)
        } else if !over_ui {
            let frame = Frame::between(&world.prev, world.shown(), world.phase());
            if world.down.is_none() && !world.holding() {
                world
                    .pointer
                    .and_then(|point| world.target_item(point, &frame))
            } else {
                None
            }
        } else {
            None
        };
        world.set_hover(item);
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
                    let gap = if k > 0 && k.is_multiple_of(5) {
                        MARK_PX
                    } else {
                        0.0
                    };
                    mark(strip, MARK_PX, 1.0, gap);
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
                    strip.spawn((
                        InstructionSymbol::Ui(*instr),
                        Node {
                            width: Val::Px(SYMBOL_PX),
                            height: Val::Px(SYMBOL_PX),
                            ..default()
                        },
                        Outline {
                            width: Val::Px(CURSOR_PX),
                            offset: Val::ZERO,
                            color: if k == line.pc { IVORY } else { Color::NONE },
                        },
                    ));
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
        commands.spawn((
            AtomPreview(kind),
            Camera2d,
            Camera {
                order: -1,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
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
        card: [Glaze::Clay.color(), brass(0.5)].map(|c| materials.add(c)),
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
        let was = self.shift;
        self.shift += by;
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

    fn instruction(&mut self, instr: Instr, at: Vec2, side: f32, z: f32) {
        self.commands.spawn((
            InstructionSymbol::Card(instr),
            Fill,
            self.layers.clone(),
            Transform {
                translation: (at + self.shift).extend(z),
                scale: Vec3::splat(side),
                ..default()
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
        self.stamp(&kiln.circle, skin, at, ATOM_RADIUS, z);
        self.stamp(&kiln.rim, patina, at, ATOM_RADIUS, z + layer::RIM);
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
        at: Vec2,
        angle: f32,
        z: f32,
        response: (bool, f32, sim::ActivationEnergy),
    ) {
        let look = look::machine(item);
        match (item, look.marking) {
            (Machine::Arm, MachineMark::Hand(_, _)) => {
                let hand = at + Vec2::from_angle(angle) * px(DIRS[0]).length();
                self.arm(at, hand, RING_OPEN, look, z, response);
            }
            (Machine::Glyph(_), MachineMark::Sprite(_)) => {
                self.rig(item, at, angle, z, response);
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

fn spin_angle(spin: Spin) -> f32 {
    px(DIRS[0]).angle_to(px(DIRS[spin.turn(0)]))
}

fn sweep(centre: Vec2, angle: f32, e: f32) -> impl Fn(Vec2) -> Vec2 {
    let angle = angle * e;
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
            let carried =
                |v: Vec2| about.map_or(v, |(c, spin)| sweep(c, spin_angle(spin), e)(v)) + shift;
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
    let board_phase = world.board_phase();
    let f = Frame::between(&world.prev, world.shown(), board_phase);
    let events = world
        .events
        .last()
        .map_or(&[][..], |tick| tick.events.as_slice());
    scene(
        &mut p,
        &f,
        0.0,
        events,
        board_phase,
        true,
        world.board_turn_pose(),
    );
    if world.down.is_none()
        && !world.holding()
        && !world.over_ui
        && let Some(target) = world.pointer.and_then(|point| world.hit(point, &f))
    {
        match target {
            Id::Atom(i) => {
                if let Some(at) = f.atoms.get(i).copied().flatten() {
                    p.ring(at, ATOM_RADIUS + LINE_PX);
                }
            }
            Id::Arm(_) | Id::Glyph(_) => p.outline(px(world.anchor(target)), HEX * 0.9),
        }
    }
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
        if let Some(Focus::Hold { set, .. }) = &world.focus {
            let grab = hex_at(pointer);
            p.outline(px(grab), HEX * 0.9);
            for id in world.sim.blocked(set, grab, &[]) {
                for cell in world.sim.stands(id) {
                    p.outline(px(cell), HEX * 0.9);
                }
            }
            let machines = world.facing_poses(TurnTarget::Held, pointer);
            for (i, pose) in machines.iter().enumerate() {
                let z = layer::z(layer::HELD, i, machines.len());
                p.machine(
                    pose.item,
                    pose.at,
                    pose.angle,
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
    let mut p = Painter {
        gizmos: &mut card_gizmos,
        commands: &mut commands,
        kiln: &kiln,
        ghost: false,
        layers: CARD,
        shift: Vec2::ZERO,
    };
    if let Some(item) = world.hover {
        rendered_card(&mut p, item, world.play.as_ref(), HOVER_SLOT, &world);
    }
    for (index, card) in world.pinned.iter().enumerate() {
        if world.pinned[..index]
            .iter()
            .any(|shown| shown.item == card.item)
        {
            continue;
        }
        let play = match card.item {
            Item::Machine(machine) => world
                .pinned_play
                .iter()
                .find(|play| play.machine == machine),
            _ => None,
        };
        rendered_card(&mut p, card.item, play, card_slot(card.item), &world);
    }
}

fn rendered_card<G: GizmoConfigGroup>(
    p: &mut Painter<G>,
    item: Item,
    play: Option<&Play>,
    slot: usize,
    world: &World,
) {
    let sims = play.map(|play| (play, play.sims()));
    let frame = sims.as_ref().map(|(play, (prev, sim))| {
        Frame::between(prev, sim, phase(play.since, world.period, world.motion))
    });
    let events = play.and_then(|play| {
        let shown = play.at.min(play.events.len() as u64);
        shown
            .checked_sub(1)
            .and_then(|index| play.events.get(index as usize))
    });
    p.shifted(card_slot_at(slot), |p| {
        hover_card(
            p,
            item,
            frame.as_ref(),
            events.map_or(&[][..], |tick| tick.events.as_slice()),
            play.map_or(1.0, |play| phase(play.since, world.period, world.motion)),
        );
    });
}

fn scene<G: GizmoConfigGroup>(
    p: &mut Painter<G>,
    f: &Frame,
    lift: f32,
    events: &[sim::TickEvent],
    phase: f32,
    particles: bool,
    turn: Option<(Id, MachinePose)>,
) {
    for (index, glyph) in f.sim.glyphs.iter().enumerate() {
        let Some(g) = glyph else { continue };
        let item = Machine::Glyph(g.kind);
        let fired = events
            .iter()
            .any(|event| rig::activation(item).matches(event, index));
        let pose = turn.filter(|(id, _)| *id == Id::Glyph(index)).map_or(
            MachinePose {
                item,
                at: px(g.at),
                angle: look::turn(g.dir),
            },
            |(_, pose)| pose,
        );
        p.machine(
            pose.item,
            pose.at,
            pose.angle,
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
        let (pivot, hand) =
            turn.filter(|(id, _)| *id == Id::Arm(i))
                .map_or((arm.pivot, arm.hand), |(_, pose)| {
                    (
                        pose.at,
                        pose.at + Vec2::from_angle(pose.angle) * px(DIRS[0]).length(),
                    )
                });
        p.arm(
            pivot,
            hand,
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
        Item::Atom(_) => unreachable!("an atom resolves to its route before painting"),
        Item::Token(instr) => p.instruction(instr, at, picture_side(item), z(2)),
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
        scene(p, f, layer::LIFT, events, phase, false, None);
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
    use bevy::input::mouse::{MouseButtonInput, MouseMotion};
    use bevy::render::RenderPlugin;
    use bevy::render::render_resource::{TextureFormat, TextureUsages};
    use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
    use bevy::time::TimeUpdateStrategy;

    use sim::{Atom, AtomKind, Bond};
    use std::path::PathBuf;
    use std::time::Duration;

    const WIDE_SCALE: f32 = 1.5;
    pub const REGRAB_STEP: Vec2 = Vec2::new(-18.0, -6.0);

    const WARM: u32 = 24;
    const FRAME: Duration = Duration::from_nanos(16_666_667);

    #[derive(Clone, Copy)]
    pub enum Act {
        Down(KeyCode),
        Up(KeyCode),
        Press(Hex),
        PressPoint(Vec2),
        Drag(Hex),
        DragPoint(Vec2),
        Release(Hex),
        Lift(Machine),
        Paste(&'static str),
        BeginPin(Item, Vec2),
        MoveCard(usize, Vec2),
        WheelCard(usize, f32),
        FocusCard(usize),
        CursorOnCard(usize, Vec2),
        PressInventory(Item),
        EndCard,
        Nudge(Vec2),
        Mouse(MouseButton, ButtonState),
        PanBoard(Vec2),
        ZoomBoard(f32),
    }

    impl Act {
        pub(super) fn card(self, world: &mut World, viewport: &Viewport) -> bool {
            match self {
                Act::BeginPin(item, pointer) => {
                    world.begin_pin(item, pointer, viewport, MouseButton::Right)
                }
                Act::MoveCard(index, delta) => {
                    if let Some(card) = world.pinned.get(index) {
                        let id = card.id;
                        let pointer = viewport.screen(card.anchor);
                        world.card_press(id, pointer, MouseButton::Left);
                        world.end_card_drag(Some(pointer + delta), viewport);
                    }
                }
                Act::WheelCard(index, notches) => {
                    if let Some(card) = world.pinned.get(index) {
                        let pointer = viewport.screen(card.anchor);
                        let mut board = 1.0;
                        wheel(world, pointer, notches, viewport, &mut board);
                    }
                }
                Act::FocusCard(index) => {
                    if let Some(card) = world.pinned.get(index) {
                        world.focus = Some(Focus::Card(card.id));
                    }
                }
                _ => return false,
            }
            true
        }
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

    pub const SCENES: [&str; 55] = [
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
        "hand",
        "start",
        "craft",
        "copy",
        "clipboard-103",
        "walk",
        "ghost",
        "hold",
        "select",
        "output",
        "bonding",
        "amber-chain-97",
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
        "cards",
        "card-wheel",
        "card-regrab",
        "inventory-drags",
        "instruction-sites",
        "instruction-sites-manual",
        "machine-drag-88",
        "atom-machine-pick-92",
        "atom-card-94",
        "machine-turn-89",
        "pinned-world-93",
        "arm-local-move-81",
        "exponential-pips-83",
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

    const SHOT_PX: UVec2 = UVec2::new(1280, 720);

    #[derive(Clone, Copy)]
    pub enum Frame {
        Micro,
        Wide,
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
            "exponential-pips-83" => {
                world.sim = Sim::empty();
                let counts = [0, 1, 2, 3, 4, 5, 7, 8];
                for (item, count) in palette().zip(counts.into_iter().cycle()) {
                    world.sim.inventory.set_cap(item, -1);
                    for _ in 0..count {
                        world.sim.inventory.add(item);
                    }
                }
                world.palette_hover = palette().nth(7);
            }
            "pinned-world-93" => {
                world.hover = None;
                world.pin_world(
                    Machine::Glyph(GlyphKind::Bonder).into(),
                    px(Hex::new(-3, 1)),
                );
                world.pin_world(Machine::Arm.into(), px(Hex::new(4, -2)));
                for step in 0..20 {
                    script.push((28 + step * 2, Act::PanBoard(Vec2::new(2.0, -1.0))));
                }
                for step in 0..12 {
                    script.push((72 + step * 2, Act::ZoomBoard(0.5)));
                    script.push((102 + step * 2, Act::ZoomBoard(-0.5)));
                }
            }
            "instruction-sites" | "instruction-sites-manual" => {
                world.sim = Sim::empty();
                world.sim.arms.push(Arm::new(
                    Hex::new(3, -3),
                    0,
                    KEYS.map(|key| key.instr).to_vec(),
                ));
                world.focus_tape(0);
                world.set_hover(Some(Item::Token(Instr::Grab)));
                let viewport = Viewport {
                    cam: px(FOCUS),
                    size: SHOT_PX.as_vec2(),
                    scale: MICRO_SCALE,
                };
                let item = Item::Token(Instr::Rot(Spin::Cw));
                let center = Vec2::new(760.0, 120.0) + card_size(item) / 2.0;
                world.pin_world(item, viewport.world(center));
                world.refused = Some(Refused {
                    at: Hex::new(8, -3),
                    short: vec![Short {
                        item: Item::Token(Instr::Wait),
                        have: 0,
                        need: 1,
                    }],
                });
                if name == "instruction-sites-manual" {
                    script.push((2, Act::Down(Tab)));
                }
            }
            "card-regrab" => {
                world.set_hover(None);
                let bonder: Item = Machine::Glyph(GlyphKind::Bonder).into();
                script.push((4, Act::BeginPin(bonder, Vec2::new(90.0, 610.0))));
                for step in 0..10 {
                    script.push((6 + step * 2, Act::MoveCard(0, Vec2::new(24.0, -22.0))));
                }
                for step in 0..20 {
                    script.push((28 + step * 2, Act::WheelCard(0, 0.25)));
                }
                script.push((70, Act::CursorOnCard(0, Vec2::splat(20.0))));
                script.push((72, Act::Mouse(MouseButton::Right, ButtonState::Pressed)));
                for step in 1..=10 {
                    script.push((74 + step * 2, Act::Nudge(REGRAB_STEP)));
                }
                script.push((96, Act::Mouse(MouseButton::Right, ButtonState::Released)));
            }
            "atom-card-94" => {
                world.sim = Sim::empty();
                world.sim.spawn(Atom {
                    kind: AtomKind::Amber,
                    pos: ORIGIN,
                });
                let centre = SHOT_PX.as_vec2() / 2.0;
                let cursor = centre + (px(ORIGIN) - px(FOCUS)) * Vec2::new(1.0, -1.0) / MICRO_SCALE;
                script.push((1, Act::Nudge(cursor)));
                script.push((96, Act::PressInventory(Item::Atom(AtomKind::Amber))));
                for step in 1..=10 {
                    script.push((96 + step * 2, Act::Nudge(Vec2::new(44.0, -26.0))));
                }
                script.push((120, Act::EndCard));
            }
            "inventory-drags" => {
                let empty = Item::from(Machine::Glyph(GlyphKind::Bonder));
                let one = Item::from(Machine::Glyph(GlyphKind::SecondBond));
                world.sim = Sim::empty();
                world.sim.inventory.add(one);
                script.push((26, Act::PressInventory(empty)));
                for step in 1..=10 {
                    script.push((26 + step * 2, Act::Nudge(Vec2::new(45.0, -28.0))));
                }
                script.push((50, Act::EndCard));
                script.push((70, Act::PressInventory(one)));
                for step in 1..=10 {
                    script.push((70 + step * 2, Act::Nudge(Vec2::new(80.0, 25.0))));
                }
                script.push((94, Act::EndCard));
            }
            "cards" | "card-wheel" => {
                world.set_hover(None);
                script.push((
                    4,
                    Act::BeginPin(
                        Machine::Glyph(GlyphKind::Bonder).into(),
                        Vec2::new(90.0, 610.0),
                    ),
                ));
                for step in 0..10 {
                    script.push((6 + step * 2, Act::MoveCard(0, Vec2::new(24.0, -22.0))));
                }
                if name == "card-wheel" {
                    for step in 0..10 {
                        script.push((36 + step * 2, Act::WheelCard(0, 0.25)));
                    }
                    for step in 0..10 {
                        script.push((76 + step * 2, Act::WheelCard(0, -0.25)));
                    }
                    return (world, frame, script, WARM);
                }
                script.push((
                    32,
                    Act::BeginPin(Machine::Arm.into(), Vec2::new(90.0, 610.0)),
                ));
                for step in 0..10 {
                    script.push((34 + step * 2, Act::MoveCard(1, Vec2::new(38.0, -9.0))));
                }
                for step in 0..10 {
                    script.push((62 + step * 2, Act::WheelCard(1, 0.25)));
                }
                script.push((92, Act::FocusCard(0)));
                script.extend(tap(100, KeyZ));
            }
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
                world.sim = Sim::empty();
                world.sim.arms.push(Arm::new(
                    Hex::new(3, -3),
                    0,
                    KEYS.map(|key| key.instr).to_vec(),
                ));
                world.focus_tape(world.sim.arms.len() - 1);
            }
            "write" => {
                let arm = Hex::new(-2, 0);
                world.sim = Sim::empty();
                world.sim.arms.push(Arm::new(arm, 0, Vec::new()));
                world.sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: arm.add(DIRS[0]),
                });
                world.sim.inventory.add(Item::Token(Instr::Grab));
                script.extend(tap(30, Space));
                script.push((54, Act::Press(arm)));
                script.push((60, Act::Release(arm)));
                script.extend(tap(96, KeyF));
                script.extend(tap(132, KeyG));
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
            "clipboard-103" => {
                let text = "B0,3 A1,3 0,3-1,3\narm 2,0 1 FwR 1\nbonder 0,2 4";
                let set = text.parse::<Fragment>().unwrap().into_sim();
                let mut sim = Sim::empty();
                for item in set.bill() {
                    sim.inventory.add(item);
                }
                world.sim = sim;
                world.running = false;
                script.push((2, Act::Paste(text)));
                script.push((4, Act::Press(Hex::new(-1, -1))));
            }
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
                world.set_hover(Some(Item::Machine(machine)));
                world.play = Some(Play::at(machine, ticks));
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
            "arm-local-move-81" => {
                let tape = [0, 0, 1, 2, 3, 3, 4, 5].map(Instr::Move).to_vec();
                let mut blueprint = Sim::empty();
                blueprint.arms.push(Arm::new(ORIGIN, 0, tape));
                let mut rotated = blueprint.clone();
                turn(&mut rotated, Spin::Cw);
                turn(&mut rotated, Spin::Cw);
                let mut sim = Sim::empty();
                sim.place(&blueprint, Hex::new(-4, 1));
                sim.place(&rotated, Hex::new(4, -1));
                world.sim = sim;
                world.focus_tape(0);
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
            "machine-drag-88" => {
                let from = Hex::new(-3, 0);
                let to = Hex::new(3, 0);
                world.sim = Sim::empty();
                world
                    .sim
                    .glyphs
                    .push(Some(Glyph::new(GlyphKind::Bonder, from, 0)));
                world
                    .sim
                    .glyphs
                    .push(Some(Glyph::new(GlyphKind::SecondBond, Hex::new(0, 3), 1)));
                script.push((20, Act::Press(from)));
                for step in 0..=120 {
                    let point = px(from).lerp(px(to), step as f32 / 120.0);
                    script.push((21 + step, Act::DragPoint(point)));
                }
                script.push((151, Act::Release(to)));
            }
            "atom-machine-pick-92" => {
                world.sim = Sim::empty();
                world
                    .sim
                    .glyphs
                    .push(Some(Glyph::new(GlyphKind::Bonder, ORIGIN, 0)));
                world.sim.spawn(Atom {
                    kind: AtomKind::Base,
                    pos: ORIGIN,
                });
                world.pointer = Some(px(ORIGIN));
                script.push((20, Act::Press(ORIGIN)));
                for step in 0..=40 {
                    let point = px(ORIGIN).lerp(px(Hex::new(-3, 0)), step as f32 / 40.0);
                    script.push((21 + step, Act::DragPoint(point)));
                }
                for step in 0..=40 {
                    let point = px(Hex::new(-3, 0)).lerp(px(ORIGIN), step as f32 / 40.0);
                    script.push((62 + step, Act::DragPoint(point)));
                }
                script.push((103, Act::Release(ORIGIN)));
                let beside = px(ORIGIN) + Vec2::new(ATOM_RADIUS + 4.0, 0.0);
                script.push((120, Act::DragPoint(beside)));
                script.push((132, Act::PressPoint(beside)));
                script.push((140, Act::DragPoint(beside + Vec2::new(DRAG_PX * 2.0, 0.0))));
                for step in 0..=60 {
                    let point = beside.lerp(px(Hex::new(3, 0)), step as f32 / 60.0);
                    script.push((141 + step, Act::DragPoint(point)));
                }
                script.push((202, Act::Release(Hex::new(3, 0))));
            }
            "machine-turn-89" => {
                world.sim = Sim::empty();
                world
                    .sim
                    .glyphs
                    .push(Some(Glyph::new(GlyphKind::Bonder, FOCUS, 0)));
                world.focus = Some(Focus::Pick(vec![Id::Glyph(0)]));
                for frame in [20, 32, 44, 56, 68, 80] {
                    script.extend(tap(frame, KeyD));
                }
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
            "amber-chain-97" => {
                let forms = ["A0,0", "B0,0 B0,1 B0,2 0,0-0,1 0,1=0,2"]
                    .map(|text| text.parse::<Form>().unwrap());
                let amber = forms[0].sim();
                let mut sim = forms[1].sim();
                let base = sim
                    .atoms
                    .iter()
                    .enumerate()
                    .find(|(id, _)| {
                        sim.bonds
                            .iter()
                            .filter(|bond| bond.a == *id || bond.b == *id)
                            .count()
                            == 1
                    })
                    .map(|(_, atom)| atom.unwrap().pos)
                    .unwrap();
                let (dir, amber_at) = DIRS
                    .iter()
                    .enumerate()
                    .map(|(dir, step)| (dir, base.add(*step)))
                    .find(|(_, at)| sim.atom_at(*at).is_none())
                    .unwrap();
                sim.place(&amber, amber_at);
                sim.glyphs
                    .push(Some(Glyph::new(GlyphKind::Bonder, base, dir)));
                world.sim = sim;
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
                script.push((72, Act::Paste("arm 0,0 2 F 0\nbonder 0,3 2")));
                script.push((96, Act::Press(Hex::new(2, -4))));
                script.push((150, Act::Press(Hex::new(-2, 0))));
                script.extend(tap(168, KeyZ));
                script.push((192, Act::Paste("arm 0,0 2 F 0\nbonder 0,3 2")));
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
            .add_systems(Update, capture.after(run_ticks).before(edit));
        app
    }

    fn spawn_offscreen_camera(
        mut commands: Commands,
        mut images: ResMut<Assets<Image>>,
        mut shot: ResMut<Shot>,
    ) {
        let mut image =
            Image::new_target_texture(SHOT_PX.x, SHOT_PX.y, TextureFormat::Rgba8UnormSrgb, None);
        image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let handle = images.add(image);
        let mut projection = OrthographicProjection::default_2d();
        let center = match shot.frame {
            Frame::Wide => {
                projection.scale = WIDE_SCALE;
                let pivots: Vec<Vec2> = sim::PLACEMENTS.iter().map(|h| px(*h)).collect();
                pivots.iter().sum::<Vec2>() / pivots.len() as f32
            }
            Frame::Micro => {
                projection.scale = MICRO_SCALE;
                px(FOCUS)
            }
        };
        commands.spawn((
            Camera2d,
            Camera::default(),
            Projection::Orthographic(projection),
            Transform::from_translation(center.extend(0.0)),
            RenderTarget::Image(handle.clone().into()),
            IsDefaultUiCamera,
        ));
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

    type Board<'w, 's> = Single<
        'w,
        's,
        (&'static mut Transform, &'static mut Projection),
        (With<IsDefaultUiCamera>, Without<CardCamera>),
    >;

    type Input<'w, 's> = (
        MessageWriter<'w, KeyboardInput>,
        MessageWriter<'w, MouseButtonInput>,
        MessageWriter<'w, MouseMotion>,
        Query<
            'w,
            's,
            (
                &'static PaletteRow,
                &'static UiGlobalTransform,
                &'static mut Interaction,
            ),
        >,
        ResMut<'w, ButtonInput<MouseButton>>,
        Board<'w, 's>,
    );

    fn capture(
        mut commands: Commands,
        mut shot: ResMut<Shot>,
        mut world: ResMut<World>,
        window: Single<(Entity, &mut Window), With<PrimaryWindow>>,
        cards: Query<(&CardCamera, &Camera), With<CardCamera>>,
        input: Input,
        mut exit: MessageWriter<AppExit>,
    ) {
        let (mut keyboard, mut mouse, mut motion, mut inventory, mut buttons, board) = input;
        let hover = cards
            .iter()
            .find(|(kind, _)| **kind == CardCamera::Hover)
            .and_then(|(_, camera)| camera.viewport.as_ref());
        if let (true, Some(v)) = (shot.frames == shot.warm, hover) {
            println!(
                "card {} {} {} {}",
                v.physical_position.x, v.physical_position.y, v.physical_size.x, v.physical_size.y
            );
        }
        let (window, mut primary) = window.into_inner();
        let (mut board_transform, mut board_projection) = board.into_inner();
        shot.frames += 1;
        for (frame, act) in shot.script.clone() {
            if frame != shot.frames {
                continue;
            }
            let viewport = Viewport::of(&primary, &board_transform, &board_projection).unwrap();
            if act.card(&mut world, &viewport) {
                continue;
            }
            match act {
                Act::Down(code) => {
                    keyboard.write(key(code, ButtonState::Pressed, window));
                }
                Act::Up(code) => {
                    keyboard.write(key(code, ButtonState::Released, window));
                }
                Act::Press(cell) => {
                    world.pointer = Some(px(cell));
                    world.press(px(cell), px(cell));
                }
                Act::PressPoint(point) => {
                    world.pointer = Some(point);
                    world.press(point, point);
                }
                Act::Drag(cell) => {
                    world.pointer = Some(px(cell));
                    world.drag(px(cell));
                }
                Act::DragPoint(point) => {
                    world.pointer = Some(point);
                    world.drag(point);
                }
                Act::Release(cell) => {
                    world.pointer = Some(px(cell));
                    world.release(Some(cell));
                }
                Act::Lift(item) => world.lift_inventory(item.into()),
                Act::Paste(text) => {
                    world.paste_text(text);
                }
                Act::CursorOnCard(index, offset) => {
                    let card = &world.pinned[index];
                    primary.set_cursor_position(Some(viewport.screen(card.anchor) + offset));
                }
                Act::PressInventory(item) => {
                    let mut pointer = None;
                    for (row, transform, mut interaction) in &mut inventory {
                        *interaction = if row.0 == item {
                            pointer = Some(transform.translation / primary.scale_factor());
                            Interaction::Pressed
                        } else {
                            Interaction::None
                        };
                    }
                    primary.set_cursor_position(pointer);
                    buttons.press(MouseButton::Left);
                }
                Act::Nudge(delta) => {
                    for (_, _, mut interaction) in &mut inventory {
                        *interaction = Interaction::None;
                    }
                    let from = primary.cursor_position().unwrap_or_default();
                    primary.set_cursor_position(Some(from + delta));
                    motion.write(MouseMotion { delta });
                }
                Act::Mouse(button, state) => {
                    mouse.write(MouseButtonInput {
                        button,
                        state,
                        window,
                    });
                }
                Act::EndCard => {
                    buttons.release(MouseButton::Left);
                }
                Act::PanBoard(delta) => {
                    board_transform.translation += delta.extend(0.0);
                }
                Act::ZoomBoard(notches) => {
                    let Projection::Orthographic(ortho) = &mut *board_projection else {
                        unreachable!()
                    };
                    ortho.scale = zoomed(ortho.scale, -notches).clamp(0.05, 40.0);
                }
                Act::BeginPin(_, _)
                | Act::MoveCard(_, _)
                | Act::WheelCard(_, _)
                | Act::FocusCard(_) => unreachable!(),
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

    type InstructionSymbols<'w, 's> = Query<
        'w,
        's,
        (
            &'static InstructionSymbol,
            Option<&'static ImageNode>,
            Option<&'static MeshMaterial2d<ColorMaterial>>,
        ),
    >;

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

    fn assert_surface_at_corners(
        frame: &image::RgbaImage,
        site: &str,
        corners: [(u32, u32); 4],
        surface: [[u8; 3]; 4],
    ) {
        for (corner, expected) in corners.into_iter().zip(surface) {
            let actual = &frame[corner].0[..3];
            let distance = actual
                .iter()
                .zip(expected)
                .map(|(actual, expected)| actual.abs_diff(expected))
                .max()
                .unwrap();
            assert!(
                distance <= 24,
                "{site} corner {corner:?} is {actual:?}, {distance} from its exposed surface {expected:?}"
            );
        }
        let plum = Glaze::Plum
            .rgb()
            .map(|channel| (255.0 * channel).round() as u8);
        let [(left, top), (right, _), (_, bottom), _] = corners;
        let visible = (top..=bottom)
            .flat_map(|y| (left..=right).map(move |x| (x, y)))
            .filter(|point| {
                frame[*point].0[..3]
                    .iter()
                    .zip(plum)
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 50)
            })
            .count();
        assert!(visible >= 4, "{site} has only {visible} plum symbol pixels");
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

    fn hit(w: &World, point: Vec2) -> Option<Id> {
        let frame = Frame::between(&w.prev, w.shown(), w.phase());
        w.hit(point, &frame)
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
    fn a_machine_drag_across_three_cells_draws_one_sprite_at_each_raw_pointer_position() {
        let from = Hex::new(-1, 0);
        let mut w = lone(vec![bonder(from, 0)], vec![]);
        w.running = false;
        w.press(px(from), px(from));
        let points = [
            px(from) + Vec2::new(DRAG_PX * 2.0, 0.0),
            px(from).lerp(px(Hex::new(0, 0)), 0.5),
            px(Hex::new(0, 0)).lerp(px(Hex::new(1, 0)), 0.5),
            px(Hex::new(1, 0)).lerp(px(Hex::new(2, 0)), 0.5),
        ];
        for pointer in points {
            w.pointer = Some(pointer);
            w.drag(pointer);
            let Some(Focus::Hold { set, .. }) = &w.focus else {
                panic!("the machine is not held")
            };
            let poses = held_machine_poses(set, pointer);
            assert_eq!(poses.len(), 1);
            assert_eq!(poses[0].1, pointer);
        }
        let between = points[2];
        assert_ne!(between, px(hex_at(between)));
    }

    #[test]
    fn the_origin_cell_draws_no_machine_while_its_machine_is_held() {
        let mut w = lone(vec![bonder(ORIGIN, 0)], vec![]);
        w.running = false;
        w.press(px(ORIGIN), px(ORIGIN));
        let pointer = px(Hex::new(2, 0)) + Vec2::new(7.0, 3.0);
        w.pointer = Some(pointer);
        w.drag(pointer);
        assert_eq!(w.shown().glyphs, [None]);
        let Some(Focus::Hold { set, .. }) = &w.focus else {
            panic!("the machine is not held")
        };
        assert_eq!(held_machine_poses(set, pointer).len(), 1);
        assert!(w.shown().ids().all(|id| w.anchor(id) != ORIGIN));
    }

    #[test]
    fn a_refused_machine_drop_restores_the_world_byte_for_byte() {
        let mover = bonder(ORIGIN, 0);
        let other = bonder(Hex::new(2, 0), 0);
        let mut w = lone(vec![mover, other], vec![]);
        w.running = false;
        let before = w.sim.clone();
        w.press(px(ORIGIN), px(ORIGIN));
        let pointer = px(Hex::new(1, 0)) + Vec2::new(8.0, 2.0);
        w.pointer = Some(pointer);
        w.drag(pointer);
        assert_ne!(w.sim, before);
        w.release(Some(Hex::new(1, 0)));
        assert_eq!(w.sim, before);
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
    fn a_frame_sampled_mid_sweep_draws_a_placed_sprite_strictly_between_rest_angles() {
        let mut w = lone(vec![bonder(ORIGIN, 0)], vec![]);
        w.running = false;
        w.since = w.period;
        w.focus = picked(&[Id::Glyph(0)]);
        let start = look::turn(0);
        let end = look::turn(1);
        w.key(KeyCode::KeyD, false);
        assert_eq!(w.sim.glyphs[0].unwrap().dir, 1);
        assert_eq!(w.turn_phase(), 0.0);
        w.advance(w.period * TURN_MOTION * 0.3);
        let angle = w.facing_poses(TurnTarget::Board(Id::Glyph(0)), px(ORIGIN))[0].angle;
        assert!(end < angle && angle < start, "{end} < {angle} < {start}");
    }

    #[test]
    fn a_held_machine_turn_changes_its_state_before_its_sprite_finishes_sweeping() {
        let mut w = lone(vec![bonder(ORIGIN, 0)], vec![]);
        w.running = false;
        w.since = w.period * 0.4;
        w.press(px(ORIGIN), px(ORIGIN));
        let pointer = px(ORIGIN) + Vec2::new(DRAG_PX * 2.0, 0.0);
        w.pointer = Some(pointer);
        w.drag(pointer);
        assert!(w.has_machine_rollback());
        w.key(KeyCode::KeyD, false);
        assert!((w.board_phase() - 0.4).abs() < 1e-5);
        let Some(Focus::Hold { set, .. }) = &w.focus else {
            panic!("the machine is not held")
        };
        assert_eq!(set.glyphs[0].unwrap().dir, 1);
        assert_eq!(w.turn_phase(), 0.0);
        let start = look::turn(0);
        let end = look::turn(1);
        w.advance(w.period * TURN_MOTION * 0.3);
        let angle = w.facing_poses(TurnTarget::Held, pointer)[0].angle;
        assert!(end < angle && angle < start, "{end} < {angle} < {start}");
        w.advance(w.period * TURN_MOTION);
        assert!((w.since - w.period * 0.4).abs() < 1e-5);
    }

    #[test]
    fn six_turn_presses_land_the_sprite_exactly_on_its_start_angle() {
        let mut w = lone(vec![bonder(ORIGIN, 0)], vec![]);
        w.running = false;
        w.since = w.period;
        w.focus = picked(&[Id::Glyph(0)]);
        let start = look::turn(0);
        for _ in 0..6 {
            w.key(KeyCode::KeyD, false);
        }
        assert_eq!(w.sim.glyphs[0].unwrap().dir, 0);
        w.advance(w.period * TURN_MOTION);
        assert!(w.turn.is_none());
        let angle = w.facing_poses(TurnTarget::Board(Id::Glyph(0)), px(ORIGIN))[0].angle;
        assert_eq!(angle, start);
    }

    #[test]
    fn an_editor_turn_during_an_arm_sweep_finishes_at_the_new_simulation_facing() {
        let mut w = lone(vec![], vec![Arm::new(ORIGIN, 0, vec![])]);
        w.running = false;
        w.sim.arms[0].pivot = DIRS[0];
        w.sim.arms[0].dir = 1;
        w.since = w.period * 0.3;
        w.focus = picked(&[Id::Arm(0)]);
        w.key(KeyCode::KeyD, false);
        assert_eq!(w.sim.arms[0].dir, 2);
        let turn = w.turn.as_ref().unwrap();
        assert!((turn.from[0].angle + turn.angle - look::turn(2)).abs() < 1e-5);
        assert_eq!(turn.poses(1.0, px(DIRS[0]))[0].at, px(DIRS[0]));
        w.advance(w.period * TURN_MOTION);
        let angle = w.facing_poses(TurnTarget::Board(Id::Arm(0)), px(DIRS[0]))[0].angle;
        assert_eq!(angle, look::turn(2));
    }

    #[test]
    fn an_anchor_under_the_cursor_wins_over_a_body_cell() {
        let bonder = bonder(Hex::new(1, 0), 0);
        let arm = Arm::new(ORIGIN, 0, vec![]);
        assert_eq!(arm.hand(), bonder.at);
        assert_eq!(
            hit(&lone(vec![bonder], vec![arm.clone()]), px(bonder.at)),
            Some(Id::Glyph(0))
        );
        assert_eq!(
            hit(&lone(vec![bonder], vec![arm]), px(ORIGIN)),
            Some(Id::Arm(0))
        );
    }

    #[test]
    fn overlapping_arm_hands_pick_by_position_in_every_storage_order() {
        let left = Arm::new(DIRS[3], 0, vec![]);
        let right = Arm::new(DIRS[0], 3, vec![]);
        assert_eq!(left.hand(), ORIGIN);
        assert_eq!(right.hand(), ORIGIN);
        for arms in [vec![left.clone(), right.clone()], vec![right, left]] {
            let w = lone(vec![], arms);
            assert_eq!(w.anchor(hit(&w, px(ORIGIN)).unwrap()), DIRS[3]);
        }
    }

    fn atom_over_machine(reverse_atoms: bool, reverse_glyphs: bool) -> World {
        let near_atom = Some(Atom {
            kind: AtomKind::Base,
            pos: ORIGIN,
        });
        let far_atom = Some(Atom {
            kind: AtomKind::Amber,
            pos: Hex::new(8, 8),
        });
        let near_glyph = Some(bonder(ORIGIN, 0));
        let far_glyph = Some(bonder(Hex::new(-8, -8), 0));
        let mut sim = Sim::empty();
        sim.atoms = if reverse_atoms {
            vec![far_atom, near_atom]
        } else {
            vec![near_atom, far_atom]
        };
        sim.glyphs = if reverse_glyphs {
            vec![far_glyph, near_glyph]
        } else {
            vec![near_glyph, far_glyph]
        };
        World::new(sim)
    }

    #[test]
    fn an_atom_body_and_the_rest_of_its_machine_cell_pick_the_same_targets_the_hover_outlines_in_every_storage_order()
     {
        let centre = px(ORIGIN);
        let corner = centre + Vec2::new(ATOM_RADIUS + 4.0, 0.0);
        assert_eq!(hex_at(corner), ORIGIN);
        for reverse_atoms in [false, true] {
            for reverse_glyphs in [false, true] {
                let mut w = atom_over_machine(reverse_atoms, reverse_glyphs);
                let hovered = hit(&w, centre).unwrap();
                assert!(matches!(hovered, Id::Atom(_)));
                assert_eq!(w.anchor(hovered), ORIGIN);
                w.press(centre, centre);
                assert!(w.picks(hovered));
                assert!(matches!(w.down, Some(Press::Atom { id, .. }) if Id::Atom(id) == hovered));
                w.drag(centre + Vec2::new(DRAG_PX * 2.0, 0.0));
                assert!(matches!(
                    w.focus,
                    Some(Focus::Hold {
                        back: Back::Cell { cell: ORIGIN, .. },
                        ..
                    })
                ));
                assert_eq!(w.sim.glyphs.iter().flatten().count(), 2);

                let mut w = atom_over_machine(reverse_atoms, reverse_glyphs);
                let hovered = hit(&w, corner).unwrap();
                assert!(matches!(hovered, Id::Glyph(_)));
                assert_eq!(w.anchor(hovered), ORIGIN);
                w.press(corner, corner);
                assert!(w.picks(hovered));
                assert!(matches!(w.down, Some(Press::Cell { .. })));
                w.drag(corner + Vec2::new(DRAG_PX * 2.0, 0.0));
                assert!(matches!(
                    w.focus,
                    Some(Focus::Hold {
                        back: Back::Pick { .. },
                        ..
                    })
                ));
                assert_eq!(w.sim.atoms.iter().flatten().count(), 2);
            }
        }
    }

    #[test]
    fn an_atom_over_a_machine_raises_the_card_selected_by_the_point_aware_target() {
        let centre = px(ORIGIN);
        let corner = centre + Vec2::new(ATOM_RADIUS + 4.0, 0.0);
        for reverse_atoms in [false, true] {
            for reverse_glyphs in [false, true] {
                let mut w = atom_over_machine(reverse_atoms, reverse_glyphs);
                let item = {
                    let frame = Frame::between(&w.prev, w.shown(), w.phase());
                    w.target_item(centre, &frame)
                };
                w.set_hover(item);
                assert_eq!(
                    w.hover,
                    Some(Item::Machine(Machine::Glyph(GlyphKind::Source)))
                );
                let item = {
                    let frame = Frame::between(&w.prev, w.shown(), w.phase());
                    w.target_item(corner, &frame)
                };
                w.set_hover(item);
                assert_eq!(
                    w.hover,
                    Some(Item::Machine(Machine::Glyph(GlyphKind::Bonder)))
                );
            }
        }
    }

    #[test]
    fn a_lone_atom_or_machine_owns_its_whole_cell() {
        let point = px(ORIGIN) + Vec2::new(ATOM_RADIUS + 4.0, 0.0);
        let mut atom = Sim::empty();
        atom.atoms.push(Some(Atom {
            kind: AtomKind::Base,
            pos: ORIGIN,
        }));
        assert_eq!(hit(&World::new(atom), point), Some(Id::Atom(0)));
        assert_eq!(
            hit(&lone(vec![bonder(ORIGIN, 0)], vec![]), point),
            Some(Id::Glyph(0))
        );
    }

    #[test]
    fn a_moving_atoms_drawn_body_wins_over_the_machine_beneath_it() {
        let mut prev = Sim::empty();
        prev.glyphs.push(Some(bonder(ORIGIN, 0)));
        let mut arm = Arm::new(DIRS[3], 0, vec![]);
        arm.holding = true;
        prev.arms.push(arm);
        prev.atoms.push(Some(Atom {
            kind: AtomKind::Base,
            pos: ORIGIN,
        }));
        let mut sim = prev.clone();
        sim.arms[0].pivot = ORIGIN;
        sim.atoms[0].as_mut().unwrap().pos = DIRS[0];
        let mut w = World::new(sim);
        w.prev = prev;
        w.since = w.period * 0.1;
        let drawn = Frame::between(&w.prev, w.shown(), w.phase()).atoms[0].unwrap();
        assert_eq!(hit(&w, drawn), Some(Id::Atom(0)));
        assert!(matches!(
            hit(&w, px(DIRS[0])),
            Some(Id::Arm(_) | Id::Glyph(_))
        ));
        w.press(drawn, drawn);
        w.drag(drawn + Vec2::new(DRAG_PX * 2.0, 0.0));
        assert!(
            matches!(w.focus, Some(Focus::Hold { back: Back::Cell { cell, .. }, .. }) if cell == DIRS[0])
        );
    }

    #[test]
    fn consuming_a_pressed_atom_cancels_the_drag_before_its_slot_is_reused() {
        let mut w = World::new(fixture(Machine::Glyph(GlyphKind::Converter(AtomKind::Amber))).sim);
        w.press(px(ORIGIN), px(ORIGIN));
        assert!(matches!(w.down, Some(Press::Atom { id: 0, .. })));
        w.step();
        assert_eq!(
            w.sim.atoms[0],
            Some(Atom {
                kind: AtomKind::Amber,
                pos: ORIGIN,
            })
        );
        w.drag(px(ORIGIN) + Vec2::new(DRAG_PX * 2.0, 0.0));
        assert!(w.down.is_none());
        assert!(!matches!(w.focus, Some(Focus::Hold { .. })));
    }

    #[test]
    fn a_consumed_slot_cannot_borrow_its_previous_atoms_drawn_body() {
        let mut w = World::new(fixture(Machine::Glyph(GlyphKind::Converter(AtomKind::Amber))).sim);
        w.step();
        w.since = w.period * 0.1;
        assert_eq!(hit(&w, px(ORIGIN)), Some(Id::Glyph(0)));
        w.since = w.period;
        assert_eq!(hit(&w, px(ORIGIN)), Some(Id::Atom(0)));

        let mut replay =
            World::new(fixture(Machine::Glyph(GlyphKind::Converter(AtomKind::Amber))).sim);
        replay.running = false;
        replay.key(KeyCode::KeyG, false);
        replay.key(KeyCode::KeyG, false);
        replay.key(KeyCode::KeyS, false);
        assert_eq!(replay.prev, *replay.shown());
        assert_eq!(hit(&replay, px(ORIGIN)), Some(Id::Atom(0)));
    }

    #[test]
    fn changing_the_preview_cancels_a_pending_atom_press() {
        let mut w = World::new(fixture(Machine::Glyph(GlyphKind::Converter(AtomKind::Amber))).sim);
        w.running = false;
        w.press(px(ORIGIN), px(ORIGIN));
        w.key(KeyCode::KeyG, false);
        assert!(w.down.is_none());
        w.press(px(ORIGIN), px(ORIGIN));
        w.key(KeyCode::KeyS, false);
        assert!(w.down.is_none());
        w.press(px(ORIGIN), px(ORIGIN));
        w.key(KeyCode::KeyG, false);
        w.press(px(ORIGIN), px(ORIGIN));
        w.key(KeyCode::Space, false);
        assert!(w.down.is_none());
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
            matches!(&w.focus, Some(Focus::Hold { set, back: Back::Pick { ids, .. } }) if set.arms.len() == 1 && *ids == [Id::Arm(1)])
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
    fn one_move_driven_blueprint_pasted_at_all_six_turns_reaches_rotated_cells_tick_for_tick() {
        let tape = vec![
            Instr::Move(0),
            Instr::Move(1),
            Instr::Move(1),
            Instr::Move(5),
            Instr::Move(4),
        ];
        let mut blueprint = Sim::empty();
        blueprint.arms.push(Arm::new(ORIGIN, 0, tape.clone()));
        let placements = [
            Hex::new(-30, 0),
            Hex::new(-18, 0),
            Hex::new(-6, 0),
            Hex::new(6, 0),
            Hex::new(18, 0),
            Hex::new(30, 0),
        ];
        let mut sim = Sim::empty();
        for (facing, placement) in placements.iter().enumerate() {
            let mut pasted = blueprint.clone();
            for _ in 0..facing {
                turn(&mut pasted, Spin::Cw);
            }
            assert_eq!(pasted.arms[0].tape, tape);
            sim.place(&pasted, *placement);
        }
        for tick in 0..=tape.len() {
            let original = sim.arms[0].pivot.sub(placements[0]);
            for (facing, (arm, placement)) in sim.arms.iter().zip(placements).enumerate() {
                assert_eq!(
                    arm.pivot.sub(placement),
                    original.turned(facing),
                    "turn {facing}, tick {tick}"
                );
            }
            if tick < tape.len() {
                sim.step();
            }
        }
    }

    #[test]
    fn turning_a_placed_arm_turns_its_future_moves_without_rewriting_its_tape() {
        let tape = vec![Instr::Move(0)];
        let mut w = lone(vec![], vec![Arm::new(ORIGIN, 0, tape.clone())]);
        w.pick(vec![Id::Arm(0)]);
        w.key(KeyCode::KeyD, false);
        assert_eq!(w.sim.arms[0].dir, 1);
        assert_eq!(w.sim.arms[0].tape, tape);
        w.step();
        assert_eq!(w.sim.arms[0].pivot, DIRS[1]);
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
        w.sim.arms[0].pc = 1;
        let text = w.copy(&INSIDE).unwrap();
        let mut expected = text.parse::<Fragment>().unwrap().into_sim();
        turn(&mut expected, Spin::Cw);
        let (other_arm, other_glyph) = (w.sim.arms[1].clone(), w.sim.glyphs[1]);
        w.delete(&INSIDE);
        assert_eq!(w.sim.arms, vec![other_arm.clone()]);
        assert_eq!(w.sim.glyphs, vec![None, other_glyph]);
        assert_eq!(w.focus, None);
        assert!(w.paste_text(&text));
        assert!(matches!(
            &w.focus,
            Some(Focus::Hold { set, back: Back::Inventory }) if set.arms.len() == 1 && set.glyphs.len() == 1
        ));
        w.key(KeyCode::KeyD, false);
        let to = Hex::new(5, 5);
        w.press(px(to), px(to));
        assert_eq!(w.sim.arms.len(), 2);
        assert_eq!(w.sim.glyphs.len(), 2);
        let pasted = [Id::Arm(1), Id::Glyph(0)];
        let after = offsets(&w, &pasted);
        assert_eq!(
            after[0],
            (to.add(expected.arms[0].pivot), expected.arms[0].dir)
        );
        let expected_glyph = expected.glyphs[0].unwrap();
        assert_eq!(after[1], (to.add(expected_glyph.at), expected_glyph.dir));
        assert_eq!(w.sim.arms[1].tape, expected.arms[0].tape);
        assert_eq!(w.sim.arms[1].pc, 1);
        assert_eq!(w.focus, picked(&pasted));
        assert!(w.copy(&pasted).is_some());
        assert_eq!(w.sim.arms.len(), 2);
    }

    fn stock_consumables(w: &mut World) {
        for item in KEYS.map(|k| Item::Token(k.instr)) {
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
    fn a_copied_arm_keeps_its_counter_but_not_transient_running_state() {
        let mut w = cluster();
        w.sim.arms[0].pc = 1;
        w.sim.arms[0].holding = true;
        w.sim.arms[0].stall = Some(Stall::Illegal);
        w.pick(vec![Id::Arm(0)]);
        let text = w.copy(&[Id::Arm(0)]).unwrap();
        w.focus = None;
        assert!(w.paste_text(&text));
        w.press(px(Hex::new(6, 6)), px(Hex::new(6, 6)));
        let pasted = &w.sim.arms[2];
        let mut expected = Arm::new(Hex::new(6, 6), 0, pasted.tape.clone());
        expected.pc = 1;
        assert_eq!(*pasted, expected);
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
        assert_eq!(w.sim, ghost0);
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
        let beside = px(pivot) + Vec2::new(ATOM_RADIUS + 4.0, 0.0);
        w.press(beside, beside);
        w.drag(beside + Vec2::new(DRAG_PX * 2.0, 0.0));
        let to = pivot.add(Hex::new(3, 0));
        w.pointer = Some(px(to));
        w.release(Some(to));
        assert_eq!(w.focus, picked(&[Id::Arm(0)]));
        w.key(KeyCode::KeyD, false);
        w.key(KeyCode::KeyC, false);
        w.key(KeyCode::KeyX, false);
        w.key(KeyCode::KeyZ, false);
        w.lift(fresh(Item::Machine(Machine::Arm)), Back::Inventory);
        w.place(Some(Hex::new(5, 5)));
        assert_eq!(w.sim, ghost0);
        assert_eq!(*w.shown(), ghost4);
        assert_eq!(w.focus, None);
        assert_eq!(w.refused, None);
    }

    #[test]
    fn z_on_an_arm_held_at_a_ghost_frame_keeps_it_out_of_the_world_and_in_the_hand() {
        let mut w = paused(4);
        let pivot = w.shown().arms[0].pivot;
        let start = px(pivot) + Vec2::new(ATOM_RADIUS + 4.0, 0.0);
        w.press(start, start);
        let pointer = start + Vec2::new(DRAG_PX * 2.0, 0.0);
        w.pointer = Some(pointer);
        w.drag(pointer);
        let before = w.focus.clone();
        w.key(KeyCode::KeyZ, false);
        assert_eq!(w.focus, before);
        let Some(Focus::Hold { set, .. }) = &w.focus else {
            panic!("the arm is not held")
        };
        assert_eq!(held_machine_poses(set, pointer).len(), 1);
        assert_eq!(w.shown().arms.len(), 1);
    }

    #[test]
    fn playback_and_inventory_changes_during_a_machine_drag_change_nothing() {
        let mut w = paused(0);
        let pivot = w.shown().arms[0].pivot;
        w.press(px(pivot), px(pivot));
        let pointer = px(pivot) + Vec2::new(DRAG_PX * 2.0, 0.0);
        w.pointer = Some(pointer);
        w.drag(pointer);
        let item = Item::Machine(Machine::Glyph(GlyphKind::Bonder));
        let (sim, ghost, focus, running, since) = (
            w.sim.clone(),
            w.ghost.clone(),
            w.focus.clone(),
            w.running,
            w.since,
        );
        w.key(KeyCode::KeyG, false);
        w.key(KeyCode::KeyS, false);
        w.key(KeyCode::Space, false);
        w.set_cap(item, 1);
        w.advance(w.period);
        assert_eq!(
            (w.sim, w.ghost, w.focus, w.running, w.since),
            (sim, ghost, focus, running, since)
        );
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
        let beside = px(DIRS[0]) + Vec2::new(ATOM_RADIUS + 4.0, 0.0);
        w.press(beside, beside);
        assert_eq!(w.focus, picked(&[Id::Glyph(0)]));
        w.drag(beside + Vec2::new(DRAG_PX * 2.0, 0.0));
        assert!(matches!(held(&w).2, Back::Pick { ids, .. } if ids == [Id::Glyph(0)]));
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
        let text = "bonder 0,0 0";
        w.sim.inventory.add(item);
        assert!(w.paste_text(text));
        w.release(Some(Hex::new(4, 0)));
        assert_eq!(w.sim.glyphs.len(), 2);
        assert_eq!(w.sim.inventory.count(item), Some(1));
        assert!(w.paste_text(text));
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
        let Some(Focus::Hold { set, .. }) = w.focus.clone() else {
            panic!("not holding: {:?}", w.focus)
        };
        let blocked = |at: Hex| w.sim.blocked(&set, at, &[]).collect::<Vec<Id>>();
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
        let text = w.copy(&[Id::Glyph(0)]).unwrap();
        w.delete(&[Id::Glyph(0)]);
        assert_eq!(w.sim.glyphs[0], None);
        assert_eq!(count(&w, bonder), sim::DEFAULT_CAP + 1);
        assert!(w.paste_text(&text));
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
        assert_eq!(w.sim.inventory.cap(bonder), Some(1));
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
        stocked(&mut w, bonder, 1);
        pair(&mut w, at, BondKind::Single);
        w.step();
        assert_eq!(atoms(&w).len(), 2);
        assert_eq!(count(&w, bonder), 1);
        w.set_cap(bonder, 1);
        w.step();
        assert_eq!(atoms(&w), vec![]);
        assert_eq!(count(&w, bonder), 2);
    }

    #[test]
    fn counts_at_cap_eight_have_their_exponential_full_pips_and_fraction() {
        let expected = [
            (0, (0, 0.0)),
            (1, (1, 0.0)),
            (2, (2, 0.0)),
            (3, (2, 0.5)),
            (4, (3, 0.0)),
            (5, (3, 0.25)),
            (7, (3, 0.75)),
            (8, (4, 0.0)),
        ];
        for (count, representation) in expected {
            assert_eq!(pips(count, 8), representation);
        }
    }

    #[test]
    fn a_wheel_notch_from_eight_reaches_sixteen_or_four() {
        let item = Item::from(Machine::Glyph(GlyphKind::Bonder));
        let mut inventory = sim::Inventory::EMPTY;
        inventory.set_cap(item, -1);
        assert_eq!(inventory.cap(item), Some(8));
        inventory.set_cap(item, 1);
        assert_eq!(inventory.cap(item), Some(16));
        inventory.set_cap(item, -2);
        assert_eq!(inventory.cap(item), Some(4));
    }

    #[test]
    fn the_shipped_palette_is_narrower_at_the_same_caps() {
        assert_eq!(TALLY_PX, 70.0);
        assert_eq!(PALETTE_WIDTH, 292.0);
    }

    #[test]
    fn the_exact_inventory_fragment_fills_every_entry_clears_itself_and_changes_nothing_else() {
        let mut sim = sim::preloaded();
        for (i, item) in palette().enumerate() {
            sim.inventory.set_cap(item, i as i32 % 5);
            let cap = sim.inventory.cap(item).unwrap();
            for _ in 1..cap {
                sim.inventory.add(item);
            }
        }
        let before = sim.clone();
        let mut world = World::new(sim);
        let mut location = INVENTORY_TOKEN.to_owned();
        let fragment = location.clone();

        consume_inventory_fragment(&mut world, &fragment, |_| location.clear());
        assert!(location.is_empty());
        let mut without_inventory = world.sim.clone();
        without_inventory.inventory = before.inventory;
        assert_eq!(without_inventory, before);
        for item in palette() {
            assert_eq!(
                world.sim.inventory.count(item),
                world.sim.inventory.cap(item)
            );
        }

        let mut world = World::new(before.clone());
        let mut location = format!("{INVENTORY_TOKEN}x");
        let fragment = location.clone();
        consume_inventory_fragment(&mut world, &fragment, |_| location.clear());
        assert_eq!(location, format!("{INVENTORY_TOKEN}x"));
        assert_eq!(world.sim, before);
    }

    #[test]
    fn the_palette_lists_every_machine_but_the_source() {
        let listed: Vec<Item> = palette().collect();
        let all: Vec<Item> = Machine::ALL
            .into_iter()
            .filter(|m| *m != Machine::Glyph(GlyphKind::Source))
            .map(Item::Machine)
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
    fn every_display_site_uses_one_renderer_and_exposes_all_four_instruction_symbol_corners() {
        let _render = RENDER_TEST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir =
            std::env::temp_dir().join(format!("ziral-instruction-sites-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = shot::still("instruction-sites-manual", dir.clone(), 1);
        lit_plugin(&mut app);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(false));
        let probe = seen.clone();
        app.add_systems(
            Last,
            move |symbols: InstructionSymbols,
                  kiln: Res<Kiln>,
                  materials: Res<Assets<ColorMaterial>>,
                  manual: Single<&Node, With<Manual>>| {
                let mut ui = Vec::new();
                let mut cards = Vec::new();
                for (symbol, image, material) in &symbols {
                    let skin = instruction_symbol(symbol.instr());
                    match (symbol, image, material) {
                        (InstructionSymbol::Ui(instr), Some(image), None) => {
                            assert_eq!(kiln.image(skin), image.image);
                            ui.push(*instr);
                        }
                        (InstructionSymbol::Card(instr), None, Some(material)) => {
                            assert_eq!(kiln.skin(skin, false), &material.0);
                            assert_eq!(
                                materials.get(&material.0).unwrap().alpha_mode,
                                AlphaMode2d::Blend
                            );
                            cards.push(*instr);
                        }
                        _ => panic!("an instruction symbol has zero or two draw surfaces"),
                    }
                }
                let count =
                    |shown: &[Instr], instr| shown.iter().filter(|shown| **shown == instr).count();
                if ui.len() == 40 && cards.len() == 2 && manual.display == Display::Flex {
                    for key in KEYS {
                        assert_eq!(
                            count(&ui, key.instr),
                            3 + usize::from(key.instr == Instr::Wait)
                        );
                    }
                    assert_eq!(count(&cards, Instr::Grab), 1);
                    assert_eq!(count(&cards, Instr::Rot(Spin::Cw)), 1);
                    *probe.lock().unwrap() = true;
                }
            },
        );
        assert_eq!(app.run(), bevy::app::AppExit::Success);
        let manual = image::load_from_memory(&std::fs::read(dir.join("00000.png")).unwrap())
            .unwrap()
            .into_rgba8();
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(*seen.lock().unwrap());

        let dir =
            std::env::temp_dir().join(format!("ziral-instruction-corners-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = shot::still("instruction-sites", dir.clone(), 1);
        lit_plugin(&mut app);
        assert_eq!(app.run(), bevy::app::AppExit::Success);
        let frame = image::load_from_memory(&std::fs::read(dir.join("00000.png")).unwrap())
            .unwrap()
            .into_rgba8();
        std::fs::remove_dir_all(&dir).unwrap();

        let strip = [[107, 79, 58]; 4];
        let clay = [[216, 195, 165]; 4];
        assert_surface_at_corners(
            &frame,
            "palette",
            [(179, 75), (204, 75), (179, 100), (204, 100)],
            strip,
        );
        assert_surface_at_corners(
            &frame,
            "tape",
            [(355, 642), (380, 642), (355, 667), (380, 667)],
            strip,
        );
        assert_surface_at_corners(
            &frame,
            "shortage",
            [(1174, 523), (1199, 523), (1174, 548), (1199, 548)],
            strip,
        );
        assert_surface_at_corners(
            &frame,
            "hover card",
            [(20, 55), (45, 55), (20, 80), (45, 80)],
            clay,
        );
        assert_surface_at_corners(
            &frame,
            "pinned card",
            [(773, 175), (798, 175), (773, 200), (798, 200)],
            clay,
        );
        assert_surface_at_corners(
            &manual,
            "manual",
            [(371, 70), (453, 70), (371, 152), (453, 152)],
            [
                [196, 170, 133],
                [218, 185, 128],
                [225, 207, 169],
                [100, 67, 23],
            ],
        );
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
        assert_eq!(w.copy(&[Id::Atom(first)]).as_deref(), Some(text));
        let (lo, hi) = (px(Hex::new(-7, 0)), px(Hex::new(5, 5)));
        w.press(lo, lo);
        w.drag(hi);
        w.pointer = Some(hi);
        w.release(Some(Hex::new(5, 5)));
        let ids = w.focus.as_ref().unwrap().picked();
        assert!(ids.contains(&Id::Glyph(0)));
        assert_eq!(ids.iter().filter(|id| matches!(id, Id::Atom(_))).count(), 6);
        let copied = w.copy(&ids).unwrap();
        let fragment = copied.parse::<Fragment>().unwrap().into_sim();
        assert_eq!(fragment.glyphs.iter().flatten().count(), 1);
        assert_eq!(fragment.atoms.iter().flatten().count(), 6);
        assert_eq!(copied.lines().count(), 3);
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
                  cameras: Query<(&CardCamera, &Camera)>| {
                let Some((_, camera)) =
                    cameras.iter().find(|(kind, _)| **kind == CardCamera::Hover)
                else {
                    return;
                };
                if !camera.is_active {
                    return;
                }
                let viewport = camera.viewport.as_ref().unwrap();
                let size = viewport.physical_size.as_vec2() - card_size(Item::Machine(machine));
                assert!(size.abs().max_element() <= 1.0, "{size}");
                *probe.lock().unwrap() = fills
                    .iter()
                    .filter(|(l, _)| **l == CARD)
                    .map(|(_, t)| (t.translation - card_slot_at(0).extend(0.0), t.scale.x))
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
        w.set_hover(Some(bonder.into()));
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
        w.set_hover(Some(Machine::Arm.into()));
        assert_eq!(w.play.as_ref().unwrap().sims().1, fixture(Machine::Arm).sim);
        w.set_hover(None);
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
    fn hovering_and_pinning_each_atom_uses_the_machine_on_its_route() {
        let routes = [
            (AtomKind::Base, Machine::Glyph(GlyphKind::Source)),
            (
                AtomKind::Amber,
                Machine::Glyph(GlyphKind::Converter(AtomKind::Amber)),
            ),
            (
                AtomKind::Plum,
                Machine::Glyph(GlyphKind::Converter(AtomKind::Plum)),
            ),
        ];
        for (kind, machine) in routes {
            let mut sim = Sim::empty();
            sim.spawn(Atom { kind, pos: ORIGIN });
            let mut world = World::new(sim);
            let frame = Frame::between(&world.prev, world.shown(), world.phase());
            let hover = world.target_item(px(ORIGIN), &frame);
            world.set_hover(hover);
            assert_eq!(
                world.play.as_ref().map(|play| (play.machine, play.at)),
                Some((machine, 0))
            );
            world.advance(world.period);
            assert_eq!(world.hover, Some(Item::Machine(machine)), "{kind:?} hover");
            assert_eq!(
                world.play.as_ref().map(|play| (play.machine, play.at)),
                Some((machine, 1))
            );
            let id = world.pin_world(Item::Atom(kind), Vec2::splat(20.0));
            let pinned = world.pinned.iter().find(|card| card.id == id).unwrap();
            assert_eq!(pinned.item, Item::Machine(machine), "{kind:?} pin");
            assert!(
                world.pinned_play.iter().any(|play| play.machine == machine),
                "{kind:?} playback"
            );
        }
    }

    #[test]
    fn right_drags_pin_several_inventory_cards_and_z_removes_only_the_focused_card() {
        let mut world = World::new(Sim::empty());
        let viewport = Viewport {
            cam: Vec2::ZERO,
            size: Vec2::new(1280.0, 720.0),
            scale: 1.0,
        };
        for (item, pointer) in [
            (Machine::Glyph(GlyphKind::Bonder).into(), Vec2::splat(20.0)),
            (Machine::Arm.into(), Vec2::splat(60.0)),
        ] {
            world.begin_pin(item, pointer, &viewport, MouseButton::Right);
            assert!(matches!(world.card_drag, Some(CardDrag::New { .. })));
            world.end_card_drag(None, &viewport);
        }
        assert_eq!(world.pinned.len(), 2);
        assert_ne!(
            card_slot(world.pinned[0].item),
            card_slot(world.pinned[1].item)
        );
        let kept = world.pinned[0].id;
        assert_eq!(world.focus, Some(Focus::Card(world.pinned[1].id)));
        world.key(KeyCode::KeyZ, false);
        assert_eq!(
            world.pinned.iter().map(|card| card.id).collect::<Vec<_>>(),
            [kept]
        );
        assert_eq!(world.focus, None);
    }

    #[test]
    fn a_left_drag_from_a_zero_count_palette_row_keeps_the_sim_and_pins_its_card_at_the_drop() {
        let item = Item::from(Machine::Glyph(GlyphKind::Bonder));
        let mut world = World::new(Sim::empty());
        let before = world.sim.clone();
        let drop = Vec2::new(640.0, 240.0);
        let viewport = Viewport {
            cam: Vec2::new(80.0, -30.0),
            size: Vec2::new(1280.0, 720.0),
            scale: 1.5,
        };
        world.press_inventory(item, Vec2::new(90.0, 610.0), &viewport);
        assert_eq!(world.sim, before);
        assert!(matches!(
            world.card_drag,
            Some(CardDrag::New {
                button: MouseButton::Left,
                ..
            })
        ));
        world.end_card_drag(Some(drop), &viewport);
        assert_eq!(world.sim, before);
        assert_eq!(world.pinned.len(), 1);
        assert_eq!(world.pinned[0].item, item);
        assert_eq!(world.pinned[0].anchor, viewport.world(drop));
    }

    #[test]
    fn a_left_drag_from_a_one_count_palette_row_lifts_the_item_and_pins_nothing() {
        let item = Item::from(Machine::Glyph(GlyphKind::Bonder));
        let mut world = World::new(Sim::empty());
        world.sim.inventory.add(item);
        let before = world.sim.clone();
        let drop = Hex::new(-2, 3);
        let viewport = Viewport {
            cam: Vec2::ZERO,
            size: Vec2::new(1280.0, 720.0),
            scale: 1.0,
        };
        world.press_inventory(item, Vec2::new(90.0, 610.0), &viewport);
        assert_eq!(world.sim, before);
        assert!(matches!(world.focus, Some(Focus::Hold { .. })));
        assert!(world.pinned.is_empty());
        world.pointer = Some(px(drop));
        world.release(Some(drop));
        assert_eq!(world.sim.inventory.count(item), Some(0));
        assert_eq!(world.sim.glyphs[0].unwrap().at, drop);
        assert!(world.pinned.is_empty());
    }

    #[test]
    fn a_dragged_cards_anchor_is_the_world_point_under_the_drop_and_the_wheel_changes_only_its_pixel_size()
     {
        let item = Machine::Glyph(GlyphKind::Bonder).into();
        let mut world = World::new(Sim::empty());
        world.pin_world(item, Vec2::splat(20.0));
        let viewport = Viewport {
            cam: Vec2::new(80.0, -30.0),
            size: Vec2::new(1280.0, 720.0),
            scale: 1.5,
        };
        let id = world.pinned[0].id;
        let press = viewport.screen(world.pinned[0].anchor) + Vec2::splat(10.0);
        let anchor = world.pinned[0].anchor;
        world.card_press(id, press, MouseButton::Left);
        world.end_card_drag(Some(press), &viewport);
        assert_eq!(world.pinned[0].anchor, anchor);
        world.card_press(id, press, MouseButton::Left);
        let drop = Vec2::new(310.0, 170.0);
        world.card_drag(drop - Vec2::splat(20.0), &viewport);
        world.end_card_drag(Some(drop), &viewport);
        assert_eq!(world.pinned[0].anchor, viewport.world(drop));
        let anchor = world.pinned[0].anchor;
        let base = card_size(item);
        let mut board = 1.25;
        world.focus = Some(Focus::Hold {
            set: Box::new(Sim::empty()),
            back: Back::Ghost,
        });
        let card = wheel(
            &mut world,
            viewport.screen(anchor),
            2.0,
            &viewport,
            &mut board,
        );
        assert!(card);
        assert_eq!(board, 1.25);
        assert!(matches!(world.focus, Some(Focus::Hold { .. })));
        let grown = zoomed(1.0, 2.0);
        assert!((world.pinned[0].scale - grown).abs() < 1e-5);
        assert_eq!(card_size(item) * world.pinned[0].scale, base * grown);
        assert_eq!(world.pinned[0].anchor, anchor);
        let card = wheel(
            &mut world,
            viewport.screen(anchor),
            -2.0,
            &viewport,
            &mut board,
        );
        assert!(card);
        assert_eq!(board, 1.25);
        assert!((world.pinned[0].scale - 1.0).abs() < 1e-5);
        let card = wheel(
            &mut world,
            Vec2::new(1200.0, 40.0),
            2.0,
            &viewport,
            &mut board,
        );
        assert!(!card);
        assert!((board - zoomed(1.25, -2.0)).abs() < 1e-5);
        assert!((world.pinned[0].scale - 1.0).abs() < 1e-5);
    }

    #[derive(Clone)]
    struct Shown {
        card: Option<(Pinned, bevy::camera::Viewport)>,
        board: Vec3,
    }

    fn regrab_frames() -> &'static [Shown] {
        static FRAMES: std::sync::OnceLock<Vec<Shown>> = std::sync::OnceLock::new();
        FRAMES.get_or_init(|| {
            let _render = RENDER_TEST
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let dir =
                std::env::temp_dir().join(format!("ziral-card-regrab-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let mut app = shot::still("card-regrab", dir.clone(), 1);
            lit_plugin(&mut app);
            let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
            let probe = seen.clone();
            app.add_systems(
                Last,
                move |world: Res<World>,
                      cameras: Query<(&CardCamera, &Camera)>,
                      board: Single<&Transform, With<IsDefaultUiCamera>>| {
                    let pins: Vec<&Camera> = cameras
                        .iter()
                        .filter(|(kind, _)| matches!(kind, CardCamera::Pin(_)))
                        .map(|(_, camera)| camera)
                        .collect();
                    assert!(pins.len() <= world.pinned.len());
                    let card = world.pinned.first().and_then(|card| {
                        let (_, camera) = cameras
                            .iter()
                            .find(|(kind, _)| **kind == CardCamera::Pin(card.id))?;
                        Some((card.clone(), camera.viewport.clone()?))
                    });
                    probe.lock().unwrap().push(Shown {
                        card,
                        board: board.translation,
                    });
                },
            );
            assert_eq!(app.run(), bevy::app::AppExit::Success);
            std::fs::remove_dir_all(&dir).unwrap();
            let frames = seen.lock().unwrap().clone();
            assert!(frames.iter().filter(|shown| shown.card.is_some()).count() > 60);
            frames
        })
    }

    #[test]
    fn a_pinned_card_camera_frames_the_card_at_its_size_on_screen_through_the_wheel() {
        let size = card_size(Machine::Glyph(GlyphKind::Bonder).into());
        let mut largest = 1.0f32;
        for (frame, shown) in regrab_frames().iter().enumerate() {
            let Some((card, camera)) = shown.card.as_ref() else {
                continue;
            };
            let board = Viewport {
                cam: shown.board.truncate(),
                size: Vec2::new(1280.0, 720.0),
                scale: MICRO_SCALE,
            };
            let (at, _) = card.screen_rect(&board);
            assert_eq!(
                camera.physical_position,
                at.round().as_uvec2(),
                "frame {frame}: {card:?} {:?}",
                shown.board
            );
            let min = at.max(Vec2::ZERO).round();
            let max = (at + size * card.scale)
                .min(Vec2::new(1280.0, 720.0))
                .round();
            assert_eq!(camera.physical_size, (max - min).max(Vec2::ONE).as_uvec2());
            largest = largest.max(card.scale);
        }
        assert!(largest > 2.0, "{largest}");
    }

    #[test]
    fn two_pinned_cards_stay_over_their_world_points_through_a_pan_and_a_zoom_in_and_out_without_changing_pixel_size()
     {
        let cards = [
            Pinned {
                id: 0,
                item: Machine::Glyph(GlyphKind::Bonder).into(),
                anchor: Vec2::new(-120.0, 40.0),
                scale: 1.0,
            },
            Pinned {
                id: 1,
                item: Machine::Arm.into(),
                anchor: Vec2::new(180.0, -90.0),
                scale: 0.8,
            },
        ];
        let viewports = [
            Viewport {
                cam: Vec2::ZERO,
                size: Vec2::new(1280.0, 720.0),
                scale: 1.0,
            },
            Viewport {
                cam: Vec2::new(70.0, -25.0),
                size: Vec2::new(1280.0, 720.0),
                scale: 1.0,
            },
            Viewport {
                cam: Vec2::new(70.0, -25.0),
                size: Vec2::new(1280.0, 720.0),
                scale: 0.5,
            },
            Viewport {
                cam: Vec2::new(70.0, -25.0),
                size: Vec2::new(1280.0, 720.0),
                scale: 1.0,
            },
        ];
        for card in &cards {
            let size = card_size(card.item) * card.scale;
            for viewport in &viewports {
                let (at, shown_size) = card.screen_rect(viewport);
                assert_eq!(at + shown_size / 2.0, viewport.screen(card.anchor));
                assert_eq!(shown_size, size);
            }
            assert_ne!(
                card.screen_rect(&viewports[0]).0,
                card.screen_rect(&viewports[1]).0
            );
            assert_ne!(
                card.screen_rect(&viewports[1]).0,
                card.screen_rect(&viewports[2]).0
            );
            assert_eq!(
                card.screen_rect(&viewports[1]),
                card.screen_rect(&viewports[3])
            );
        }
    }

    #[test]
    fn a_card_camera_frames_the_card_in_the_targets_own_pixels() {
        let item = Machine::Arm.into();
        let size = card_size(item);
        let mut camera = Camera::default();
        let mut projection = Projection::default();
        let mut transform = Transform::default();
        let placed = Placement {
            kind: CardCamera::Pin(7),
            item,
            at: Vec2::new(100.5, 40.0),
            scale: 0.5,
            order: 2,
        };
        placed.aim(
            (UVec2::new(2560, 1440), 2.0),
            (&mut camera, &mut projection, &mut transform),
        );
        let viewport = camera.viewport.clone().unwrap();
        assert_eq!(viewport.physical_position, UVec2::new(201, 80));
        assert_eq!(
            viewport.physical_size,
            ((Vec2::new(100.5, 40.0) + size * 0.5) * 2.0)
                .round()
                .as_uvec2()
                - UVec2::new(201, 80)
        );
        assert_eq!(
            transform.translation.truncate(),
            card_slot_at(card_slot(item))
        );
        let Projection::Orthographic(ortho) = &projection else {
            panic!()
        };
        assert!(matches!(
            ortho.scaling_mode,
            ScalingMode::Fixed { width, height }
                if (width - size.x).abs() < 1e-5 && (height - size.y).abs() < 1e-5
        ));
        let flush = Placement {
            at: Vec2::new(1280.0, 720.0) - size * 0.5,
            ..placed
        };
        flush.aim(
            (UVec2::new(1280, 720), 1.0),
            (&mut camera, &mut projection, &mut transform),
        );
        let viewport = camera.viewport.clone().unwrap();
        assert!(
            (viewport.physical_position + viewport.physical_size)
                .cmple(UVec2::new(1280, 720))
                .all()
        );
        assert!(camera.is_active);
        let partial = Placement {
            at: Vec2::new(-size.x * 0.25, 40.0),
            ..flush
        };
        partial.aim(
            (UVec2::new(1280, 720), 1.0),
            (&mut camera, &mut projection, &mut transform),
        );
        let viewport = camera.viewport.clone().unwrap();
        assert_eq!(viewport.physical_position.x, 0);
        assert_eq!(viewport.physical_size.x, (size.x * 0.25).round() as u32);
        let Projection::Orthographic(ortho) = &projection else {
            panic!()
        };
        assert!(matches!(
            ortho.scaling_mode,
            ScalingMode::Fixed { width, .. } if (width - size.x / 2.0).abs() < 1e-5
        ));
        let shifted = transform.translation.truncate() - card_slot_at(card_slot(item));
        assert!((shifted.x - size.x / 4.0).abs() < 1e-3, "{shifted}");
        let outside = Placement {
            at: Vec2::new(-size.x, 40.0),
            ..partial
        };
        outside.aim(
            (UVec2::new(1280, 720), 1.0),
            (&mut camera, &mut projection, &mut transform),
        );
        assert!(!camera.is_active);
        assert!(camera.viewport.is_none());
        let rounded_outside = Placement {
            at: Vec2::new(1279.75, 40.0),
            ..outside
        };
        rounded_outside.aim(
            (UVec2::new(1280, 720), 1.0),
            (&mut camera, &mut projection, &mut transform),
        );
        assert!(!camera.is_active);
        assert!(camera.viewport.is_none());
    }

    #[test]
    fn a_right_drag_begun_on_a_pinned_card_moves_the_card_and_leaves_the_board_camera() {
        let frames = regrab_frames();
        let board = frames[0].board;
        assert!(
            frames.iter().all(|shown| shown.board == board),
            "{:?}",
            frames.iter().map(|shown| shown.board).collect::<Vec<_>>()
        );
        let cards: Vec<&Pinned> = frames
            .iter()
            .filter_map(|shown| shown.card.as_ref().map(|(card, _)| card))
            .collect();
        let last = cards[cards.len() - 1];
        let grown = cards.iter().find(|card| card.scale == last.scale).unwrap();
        let moved = last.anchor - grown.anchor;
        let screen = Vec2::splat(20.0) + shot::REGRAB_STEP * 10.0;
        let expected = Vec2::new(screen.x, -screen.y) * MICRO_SCALE;
        assert!((moved - expected).abs().max_element() < 1e-3, "{moved}");
    }

    #[test]
    fn a_right_press_waits_for_targeting_before_board_pan_motion_starts() {
        let mut buttons = ButtonInput::default();
        buttons.press(MouseButton::Right);
        buttons.press(MouseButton::Middle);
        assert!(!board_pan_active(&buttons, true));
        buttons.clear_just_pressed(MouseButton::Right);
        assert!(board_pan_active(&buttons, true));
    }

    #[test]
    fn hover_and_pinned_cards_share_the_clay_surface_and_a_pin_has_no_corner_handle() {
        let _render = RENDER_TEST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let dir = std::env::temp_dir().join(format!("ziral-card-handle-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = shot::still("cards", dir.clone(), 1);
        lit_plugin(&mut app);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(None));
        let probe = seen.clone();
        app.add_systems(
            Last,
            move |cards: Query<Option<&Children>, With<PinnedCard>>,
                  kiln: Res<Kiln>,
                  materials: Res<Assets<ColorMaterial>>| {
                if !cards.is_empty() {
                    let color = materials.get(&kiln.card[0]).unwrap().color;
                    *probe.lock().unwrap() = Some((
                        color == Glaze::Clay.color(),
                        cards.iter().all(|children| children.is_none()),
                    ));
                }
            },
        );
        assert_eq!(app.run(), bevy::app::AppExit::Success);
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(*seen.lock().unwrap(), Some((true, true)));
    }

    #[test]
    fn every_card_fits_between_its_slot_and_the_next() {
        for item in palette().map(card_item) {
            assert!(card_size(item).x < CARD_PITCH, "{item:?}");
        }
    }

    #[test]
    fn a_token_card_ends_at_its_recipe_and_a_machine_card_holds_its_playfield() {
        let item = Item::Token(Instr::Grab);
        let card = layout(item);
        assert_eq!(card.field, None);
        assert_eq!(
            card.size.x,
            3.0 * CARD_PAD + picture_side(item) + recipe_side()
        );
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
        w.since = w.period;
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
        assert_eq!(w.sim, ghost0);
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
    fn an_arm_covered_by_an_atom_gives_its_body_to_the_atom_and_the_rest_of_its_cell_to_the_arm() {
        let mut w = lone(vec![], vec![Arm::new(Hex::new(-1, 0), 0, vec![])]);
        w.running = false;
        pair(&mut w, ORIGIN, BondKind::Single);
        lift_at(&mut w, ORIGIN);
        assert_eq!(held(&w).2, taken(ORIGIN));
        w.release(None);
        assert_eq!(w.focus, None);
        let beside = px(ORIGIN) + Vec2::new(ATOM_RADIUS + 4.0, 0.0);
        w.press(beside, beside);
        w.release(Some(ORIGIN));
        assert_eq!(w.focus, Some(Focus::Tape { arm: 0, cursor: 0 }));
        w.press(beside, beside);
        w.drag(beside + Vec2::new(DRAG_PX * 2.0, 0.0));
        assert!(matches!(held(&w).2, Back::Pick { ids, .. } if ids == [Id::Arm(0)]));
        assert_eq!(atoms(&w).len(), 2);
    }

    fn played(name: &str, frames: u32) -> World {
        let (mut w, _, script, warm) = shot::scene(name, 0);
        let mut viewport = Viewport {
            cam: px(FOCUS),
            size: Vec2::new(1280.0, 720.0),
            scale: MICRO_SCALE,
        };
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
                if act.card(&mut w, &viewport) {
                    continue;
                }
                match *act {
                    shot::Act::Down(key) => w.key(key, false),
                    shot::Act::Up(_) => {}
                    shot::Act::Press(cell) => {
                        w.pointer = Some(px(cell));
                        w.press(px(cell), px(cell));
                    }
                    shot::Act::PressPoint(point) => {
                        w.pointer = Some(point);
                        w.press(point, point);
                    }
                    shot::Act::Drag(cell) => {
                        w.pointer = Some(px(cell));
                        w.drag(px(cell));
                    }
                    shot::Act::DragPoint(point) => {
                        w.pointer = Some(point);
                        w.drag(point);
                    }
                    shot::Act::Release(cell) => {
                        w.pointer = Some(px(cell));
                        w.release(Some(cell));
                    }
                    shot::Act::Lift(item) => w.lift_inventory(item.into()),
                    shot::Act::Paste(text) => {
                        w.paste_text(text);
                    }
                    shot::Act::CursorOnCard(_, _)
                    | shot::Act::Nudge(_)
                    | shot::Act::Mouse(_, _) => {}
                    shot::Act::PanBoard(delta) => viewport.cam += delta,
                    shot::Act::ZoomBoard(notches) => {
                        viewport.scale = zoomed(viewport.scale, -notches).clamp(0.05, 40.0)
                    }
                    shot::Act::BeginPin(_, _)
                    | shot::Act::MoveCard(_, _)
                    | shot::Act::WheelCard(_, _)
                    | shot::Act::FocusCard(_)
                    | shot::Act::PressInventory(_)
                    | shot::Act::EndCard => unreachable!(),
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
    fn g_steps_forward_with_empty_or_full_inventory_without_changing_it() {
        for full in [false, true] {
            let mut w = paused(0);
            w.sim.inventory = sim::Inventory::EMPTY;
            if full {
                w.sim.inventory.fill();
            }
            let inventory = w.sim.inventory;
            for _ in 0..8 {
                w.key(KeyCode::KeyG, false);
            }
            assert_eq!(w.ghosts(), 8);
            assert_eq!(w.sim.inventory, inventory);
            assert_eq!(*w.shown(), w.sim.replay(8));
        }
    }

    #[test]
    fn s_steps_back_with_empty_or_full_inventory_without_changing_it() {
        for full in [false, true] {
            let mut w = paused(0);
            w.resim(8);
            w.sim.inventory = sim::Inventory::EMPTY;
            if full {
                w.sim.inventory.fill();
            }
            let inventory = w.sim.inventory;
            for expected in (0..8).rev() {
                w.key(KeyCode::KeyS, false);
                assert_eq!(w.ghosts(), expected);
            }
            w.key(KeyCode::KeyS, false);
            assert_eq!(w.ghosts(), 0);
            assert_eq!(w.sim.inventory, inventory);
        }
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
        let copied = w.copy(&[Id::Arm(0)]).unwrap();
        w.delete(&[Id::Arm(0)]);
        assert!(w.sim.arms.is_empty());
        assert_eq!(counts(&w), (1, 2, 1, 1, 0));
        assert!(copied.starts_with("arm "));
    }

    const BONDER: Item = Item::Machine(Machine::Glyph(GlyphKind::Bonder));
    const ARM: Item = Item::Machine(Machine::Arm);
    const GRAB: Item = Item::Token(Instr::Grab);

    const SECOND: Item = Item::Machine(Machine::Glyph(GlyphKind::SecondBond));
    const SECOND_AT: Hex = Hex::new(0, -3);

    fn copied() -> (World, String) {
        let arm = Arm::new(Hex::new(3, 0), 0, vec![Instr::Grab, Instr::Grab]);
        let second = Glyph::new(GlyphKind::SecondBond, SECOND_AT, 0);
        let mut w = lone(vec![bonder(ORIGIN, 0), second], vec![arm]);
        w.running = false;
        let ids = [Id::Glyph(0), Id::Glyph(1), Id::Arm(0)];
        w.pick(ids.to_vec());
        let text = w.copy(&ids).unwrap();
        (w, text)
    }

    fn counts(w: &World) -> (u32, u32, u32, u32) {
        (
            count(w, BONDER),
            count(w, SECOND),
            count(w, ARM),
            count(w, GRAB),
        )
    }

    fn pasted(w: &mut World, text: &str, at: Hex) {
        assert!(w.paste_text(text));
        w.press(px(at), px(at));
    }

    fn short(item: Item, have: u32, need: u32) -> Short {
        Short { item, have, need }
    }

    #[test]
    fn a_paste_pays_its_whole_bill_and_a_short_one_is_refused_whole_naming_the_short_items() {
        let at = Hex::new(0, 6);
        let (mut w, text) = copied();
        for (item, n) in [(BONDER, 1), (SECOND, 1), (ARM, 1), (GRAB, 1)] {
            stocked(&mut w, item, n);
        }
        let before = w.sim.clone();
        pasted(&mut w, &text, at);
        assert_eq!(w.sim, before);
        assert_eq!(w.focus, None);
        let refused = |short: Vec<Short>| Some(Refused { at, short });
        assert_eq!(w.refused, refused(vec![short(GRAB, 1, 2)]));
        assert!(w.sim.inventory.spend(BONDER));
        pasted(&mut w, &text, at);
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
        let set = text.parse::<Fragment>().unwrap().into_sim();
        pasted(&mut w, &text, at);
        assert_eq!(w.refused, None);
        assert_eq!(w.sim.arms.len(), 2);
        assert_eq!(w.sim.arms[1].tape, vec![Instr::Grab, Instr::Grab]);
        let expected: Vec<Glyph> = set
            .glyphs
            .iter()
            .flatten()
            .map(|g| Glyph::new(g.kind, at.add(g.at), g.dir))
            .collect();
        assert_eq!(w.sim.glyphs[2], Some(expected[0]));
        assert_eq!(w.sim.glyphs[3], Some(expected[1]));
        assert_eq!(counts(&w), (0, 0, 0, 0));
        let pasted = [Id::Arm(1), Id::Glyph(2), Id::Glyph(3)];
        assert_eq!(w.focus, picked(&pasted));
        let over = at.add(DIRS[0]);
        drag(&mut w, at, over);
        assert_eq!(
            w.sim.glyphs[2],
            Some(Glyph::new(
                expected[0].kind,
                expected[0].at.add(DIRS[0]),
                expected[0].dir
            ))
        );
        assert_eq!(counts(&w), (0, 0, 0, 0));
        assert_eq!(w.refused, None);
        assert_eq!(w.focus, picked(&pasted));
        w.key(KeyCode::KeyZ, false);
        assert_eq!(counts(&w), (1, 1, 1, 2));
        assert_eq!(w.sim.arms.len(), 1);
        assert_eq!(w.sim.glyphs[0], Some(bonder(ORIGIN, 0)));
        assert_eq!(
            w.sim.glyphs[1],
            Some(Glyph::new(GlyphKind::SecondBond, SECOND_AT, 0))
        );
        assert_eq!(&w.sim.glyphs[2..], &[None, None]);
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
    fn one_notation_copies_and_pastes_two_machines_a_mid_program_tape_and_a_compound() {
        let mut source = lone(
            vec![bonder(Hex::new(-3, 2), 4)],
            vec![Arm::new(
                Hex::new(2, -1),
                1,
                vec![Instr::Grab, Instr::Move(4), Instr::Drop],
            )],
        );
        source.sim.arms[0].pc = 1;
        let a = source.sim.spawn(Atom {
            kind: AtomKind::Base,
            pos: Hex::new(0, 3),
        });
        let b = source.sim.spawn(Atom {
            kind: AtomKind::Amber,
            pos: Hex::new(1, 3),
        });
        source.sim.bonds.push(sim::Bond {
            a,
            b,
            kind: BondKind::Single,
        });
        let ids = [Id::Arm(0), Id::Glyph(0), Id::Atom(a)];
        let text = source.copy(&ids).unwrap();
        let fragment = text.parse::<Fragment>().unwrap().into_sim();
        assert_eq!(Fragment::of(&fragment).unwrap().to_string(), text);

        let mut target = lone(vec![], vec![]);
        for item in fragment.bill() {
            target.sim.inventory.add(item);
        }
        assert!(target.paste_text(&text));
        target.key(KeyCode::KeyD, false);
        let at = Hex::new(8, 5);
        target.place(Some(at));
        assert_eq!(target.sim.arms[0].pc, 1);
        assert_eq!(target.sim.arms[0].tape, source.sim.arms[0].tape);
        assert_eq!(target.sim.arms[0].dir, (fragment.arms[0].dir + 1) % 6);
        assert_eq!(
            target.sim.arms[0].pivot,
            at.add(fragment.arms[0].pivot.rotate(ORIGIN, Spin::Cw))
        );
        assert_eq!(Fragment::of(&target.sim).unwrap().to_string(), text);
    }

    #[test]
    fn one_malformed_machine_line_refuses_the_whole_paste_byte_equal() {
        let mut w = lone(vec![bonder(Hex::new(2, 2), 0)], vec![]);
        stocked(&mut w, Machine::Arm, 1);
        stocked(&mut w, Item::Token(Instr::Grab), 1);
        stocked(&mut w, Item::Atom(AtomKind::Base), 1);
        let before = persist::encode(&w.sim).unwrap();
        let text = "B0,0\narm 1,0 0 F nope\nbonder 2,0 0";
        assert!(!w.paste_text(text));
        assert_eq!(persist::encode(&w.sim).unwrap(), before);
        assert_eq!(w.focus, None);
        assert_eq!(w.refused, None);
    }

    #[test]
    fn an_unrepresentable_fragment_translation_is_refused_byte_equal() {
        let mut w = lone(vec![], vec![]);
        stocked(&mut w, Item::Atom(AtomKind::Base), 2);
        let before = persist::encode(&w.sim).unwrap();
        assert!(w.paste_text("B0,0\nB0,2147483647"));
        w.place(Some(Hex::new(0, 1)));
        assert_eq!(persist::encode(&w.sim).unwrap(), before);
        assert_eq!(w.focus, None);
        assert_eq!(w.refused, None);
    }

    #[test]
    fn a_paste_does_not_replace_a_held_fragment() {
        let mut w = lone(vec![], vec![]);
        assert!(w.paste_text("bonder 0,0 0"));
        assert!(!w.paste_text("arm 0,0 0 - 0"));
        let Some(Focus::Hold { set, .. }) = &w.focus else {
            panic!("a held fragment")
        };
        assert_eq!(set.arms.len(), 0);
        assert_eq!(set.glyphs[0].unwrap().kind, GlyphKind::Bonder);
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
        let (mut w, text) = copied();
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
        pasted(&mut w, &text, at);
        assert_eq!(w.sim, before);
        assert_eq!(w.focus, None);
        assert_eq!(w.refused, line);
        let ground = Hex::new(6, 6);
        w.press(px(ground), px(ground));
        assert_eq!(w.refused, None);
        w.release(Some(ground));
        pasted(&mut w, &text, at);
        assert_eq!(w.refused, line);
        w.key(KeyCode::Escape, false);
        assert_eq!(w.refused, None);
    }

    #[test]
    fn the_refusal_line_shows_each_shortfall_as_beads_and_writes_nothing() {
        let _render = RENDER_TEST
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (mut w, text) = copied();
        for item in [SECOND, ARM, GRAB] {
            stocked(&mut w, item, 1);
        }
        pasted(&mut w, &text, Hex::new(0, 6));
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
    fn zoom_about_a_pointer_keeps_card_hit_testing_on_the_updated_camera() {
        let mut viewport = Viewport {
            cam: Vec2::new(30.0, -70.0),
            size: Vec2::new(1280.0, 720.0),
            scale: 1.0,
        };
        let pointer = Vec2::new(180.0, 140.0);
        let fixed = viewport.world(pointer);
        let mut world = World::new(Sim::empty());
        let anchor = viewport.world(Vec2::new(900.0, 460.0));
        let id = world.pin_world(Machine::Arm.into(), anchor);
        viewport.zoom_about(pointer, 0.5);
        assert_eq!(viewport.world(pointer), fixed);
        assert_eq!(world.card_at(viewport.screen(anchor), &viewport), Some(id));
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
        assert_eq!(w.sim.glyphs, vec![Some(bonder(Hex::new(2, -1), 2))]);
        assert_eq!(w.sim.arms.len(), 2);
        assert_eq!(w.sim.arms[1].tape, vec![Instr::Grab]);
        assert_eq!(
            (count(&w, BONDER), count(&w, ARM), count(&w, GRAB)),
            (0, 0, 0)
        );
    }
}
