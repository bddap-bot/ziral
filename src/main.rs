mod sim;

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use sim::{Arm, BondKind, DIRS, Glyph, GlyphKind, Hex, Instr, Sim};

const HEX: f32 = 20.0;
const TICK_HZ: f64 = 6.0;
const MICRO_SCALE: f32 = 0.5;
const MAX_GRID_CELLS: f32 = 6000.0;
const STRIP_ROWS: usize = 8;
const DRAG_PX: f32 = 6.0;
const PANEL: Color = Color::srgb(0.12, 0.12, 0.12);
const PANEL_LIT: Color = Color::srgb(0.3, 0.3, 0.3);

#[derive(Clone, Copy, PartialEq, Eq, Component)]
enum Item {
    Arm,
    Glyph(GlyphKind),
}

const PALETTE: [(Item, &str); 5] = [
    (Item::Arm, "arm"),
    (Item::Glyph(GlyphKind::Bonder), "bonder"),
    (Item::Glyph(GlyphKind::SecondBond), "second bond"),
    (Item::Glyph(GlyphKind::Source), "source"),
    (Item::Glyph(GlyphKind::Output), "output"),
];

impl Item {
    fn name(self) -> &'static str {
        PALETTE
            .iter()
            .find(|(i, _)| *i == self)
            .map_or("", |(_, n)| n)
    }
}

const KEYS: [(KeyCode, Instr, char, &str); 5] = [
    (KeyCode::KeyF, Instr::Grab, 'F', "grab"),
    (KeyCode::KeyR, Instr::Drop, 'R', "drop"),
    (KeyCode::KeyE, Instr::RotCw, 'E', "cw"),
    (KeyCode::KeyQ, Instr::RotCcw, 'Q', "ccw"),
    (KeyCode::KeyX, Instr::Wait, '.', "wait"),
];

fn instr_char(instr: Instr) -> char {
    KEYS.iter().find(|k| k.1 == instr).map_or('?', |k| k.2)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Focus {
    Arm { arm: usize, cursor: usize },
    Glyph(usize),
}

impl Focus {
    fn arm(self) -> Option<usize> {
        match self {
            Focus::Arm { arm, .. } => Some(arm),
            Focus::Glyph(_) => None,
        }
    }
}

#[derive(Clone, Copy)]
struct Held {
    item: Item,
    dir: usize,
    from: Option<Focus>,
}

#[derive(Resource)]
struct World {
    sim: Sim,
    running: bool,
    focus: Option<Focus>,
    held: Option<Held>,
    press: Option<Vec2>,
    pointer: Option<Vec2>,
}

impl World {
    fn new() -> Self {
        World {
            sim: sim::preloaded(),
            running: true,
            focus: None,
            held: None,
            press: None,
            pointer: None,
        }
    }

    fn focus_arm(&mut self, arm: usize) {
        let cursor = self.sim.arms[arm].tape.len();
        self.focus = Some(Focus::Arm { arm, cursor });
    }

    fn item(&self, f: Focus) -> (Item, usize) {
        match f {
            Focus::Arm { arm, .. } => (Item::Arm, self.sim.arms[arm].dir),
            Focus::Glyph(i) => (Item::Glyph(self.sim.glyphs[i].kind), self.sim.glyphs[i].dir),
        }
    }

    fn turn(&mut self, f: Focus, dir: usize) {
        match f {
            Focus::Arm { arm, .. } => {
                self.sim.arms[arm].dir = dir;
                self.sim.arms[arm].held = None;
            }
            Focus::Glyph(i) => self.sim.glyphs[i].dir = dir,
        }
    }

    fn remove(&mut self, f: Focus) {
        match f {
            Focus::Arm { arm, .. } => {
                self.sim.arms.remove(arm);
            }
            Focus::Glyph(i) => {
                self.sim.glyphs.remove(i);
            }
        }
        self.focus = None;
        self.press = None;
    }

    fn lift(&mut self, item: Item, dir: usize, from: Option<Focus>) {
        self.held = Some(Held { item, dir, from });
        self.focus = None;
    }

    fn drop_at(&mut self, at: Option<Hex>) {
        let Some(held) = self.held.take() else { return };
        match (held.from, at) {
            (Some(f), None) => self.focus = Some(f),
            (Some(f), Some(at)) => {
                match f {
                    Focus::Arm { arm, .. } => {
                        self.sim.arms[arm].pivot = at;
                        self.sim.arms[arm].held = None;
                    }
                    Focus::Glyph(i) => self.sim.glyphs[i].at = at,
                }
                self.turn(f, held.dir);
                self.focus = Some(f);
            }
            (None, Some(at)) => match held.item {
                Item::Arm => {
                    self.sim.arms.push(Arm::new(at, held.dir, Vec::new()));
                    self.focus_arm(self.sim.arms.len() - 1);
                }
                Item::Glyph(kind) => {
                    self.sim.glyphs.push(Glyph {
                        kind,
                        at,
                        dir: held.dir,
                    });
                    self.focus = Some(Focus::Glyph(self.sim.glyphs.len() - 1));
                }
            },
            (None, None) => {}
        }
    }

    fn hit(&self, at: Vec2) -> Option<Focus> {
        let nearest = |cells: &mut dyn Iterator<Item = Hex>| {
            cells
                .enumerate()
                .map(|(i, h)| (i, px(h).distance(at)))
                .filter(|(_, d)| *d < HEX)
                .min_by(|x, y| x.1.total_cmp(&y.1))
                .map(|(i, _)| i)
        };
        nearest(&mut self.sim.arms.iter().map(|a| a.pivot))
            .map(|arm| Focus::Arm {
                arm,
                cursor: self.sim.arms[arm].tape.len(),
            })
            .or_else(|| nearest(&mut self.sim.glyphs.iter().map(|g| g.at)).map(Focus::Glyph))
    }
}

fn px(h: Hex) -> Vec2 {
    let q = h.q as f32;
    let r = h.r as f32;
    Vec2::new(HEX * 3f32.sqrt() * (q + r / 2.0), HEX * 1.5 * r)
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

fn main() {
    let mut app = App::new();
    app.insert_resource(World::new())
        .insert_resource(Time::<Fixed>::from_hz(TICK_HZ))
        .insert_resource(ClearColor(Color::srgb(0.08, 0.08, 0.08)))
        .add_systems(Startup, spawn_ui)
        .add_systems(FixedUpdate, run_ticks)
        .add_systems(Update, (view, edit, strip, draw, text).chain());
    #[cfg(not(target_arch = "wasm32"))]
    if shot::configure(&mut app) {
        app.run();
        return;
    }
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "ziral".into(),
            canvas: Some("#ziral".into()),
            fit_canvas_to_parent: true,
            ..default()
        }),
        ..default()
    }))
    .add_systems(Startup, spawn_camera)
    .run();
}

fn spawn_camera(mut commands: Commands) {
    let mut projection = OrthographicProjection::default_2d();
    projection.scale = MICRO_SCALE;
    commands.spawn((
        Camera2d,
        Projection::Orthographic(projection),
        Transform::from_translation(px(Hex::new(0, -1)).extend(0.0)),
    ));
}

#[derive(Component)]
struct Hud;

#[derive(Component)]
struct TapeRow {
    slot: usize,
    arm: Option<usize>,
}

fn button(node: Node) -> impl Bundle {
    (
        Button,
        node,
        BorderColor::all(Color::srgb(0.5, 0.5, 0.5)),
        BackgroundColor(PANEL),
    )
}

fn spawn_ui(mut commands: Commands) {
    commands.spawn((
        Hud,
        Text::new(""),
        TextFont::from_font_size(16.0),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(8.0),
            top: Val::Px(8.0),
            ..default()
        },
    ));
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
            for (item, name) in PALETTE {
                col.spawn((
                    item,
                    button(Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    }),
                    children![(Text::new(name), TextFont::from_font_size(16.0))],
                ));
            }
        });
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: Val::Px(140.0),
            right: Val::Px(8.0),
            bottom: Val::Px(8.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(2.0),
            ..default()
        })
        .with_children(|col| {
            for slot in 0..STRIP_ROWS {
                col.spawn((
                    TapeRow { slot, arm: None },
                    button(Node {
                        padding: UiRect::axes(Val::Px(6.0), Val::Px(1.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    }),
                    Visibility::Hidden,
                    children![(Text::new(""), TextFont::from_font_size(15.0))],
                ));
            }
        });
}

fn run_ticks(mut world: ResMut<World>) {
    if world.running {
        world.sim.step();
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
    if keys.just_pressed(KeyCode::Space) {
        world.running = !world.running;
    }
    if keys.just_pressed(KeyCode::Period) {
        world.running = false;
        world.sim.step();
    }
    let (transform, projection) = camera.into_inner();
    let Some(viewport) = Viewport::of(&window, transform, projection) else {
        return;
    };
    let screen = window.cursor_position();
    if let Some(c) = screen {
        world.pointer = Some(viewport.world(c));
    }
    let over_ui = palette.iter().any(|(_, i)| *i != Interaction::None)
        || rows.iter().any(|(_, i)| *i != Interaction::None);
    let at = world.pointer.map(hex_at);

    if buttons.just_pressed(MouseButton::Left) {
        if let Some((item, _)) = palette.iter().find(|(_, i)| **i == Interaction::Pressed) {
            world.lift(*item, 0, None);
        } else if let Some((row, _)) = rows.iter().find(|(_, i)| **i == Interaction::Pressed) {
            if let Some(arm) = row.arm.filter(|a| *a < world.sim.arms.len()) {
                world.focus_arm(arm);
            }
        } else if !over_ui && let (Some(c), Some(p)) = (screen, world.pointer) {
            world.focus = world.hit(p);
            world.press = Some(c);
        }
    }
    if buttons.pressed(MouseButton::Left)
        && world.held.is_none()
        && let (Some(press), Some(c), Some(f)) = (world.press, screen, world.focus)
        && press.distance(c) > DRAG_PX
    {
        let (item, dir) = world.item(f);
        world.lift(item, dir, Some(f));
    }
    if buttons.just_released(MouseButton::Left) {
        world.press = None;
        if world.held.is_some() {
            let valid = !over_ui && screen.is_some();
            world.drop_at(at.filter(|_| valid));
        }
    }

    if keys.just_pressed(KeyCode::Escape) {
        world.drop_at(None);
        world.focus = None;
        return;
    }
    let turn = if keys.just_pressed(KeyCode::KeyA) {
        Some(5)
    } else if keys.just_pressed(KeyCode::KeyD) {
        Some(1)
    } else {
        None
    };
    let delete = keys.just_pressed(KeyCode::KeyZ);
    if let Some(held) = &mut world.held {
        if let Some(step) = turn {
            held.dir = (held.dir + step) % 6;
        }
        if delete {
            let from = held.from;
            world.held = None;
            if let Some(f) = from {
                world.remove(f);
            }
        }
        return;
    }
    let Some(focus) = world.focus else {
        return;
    };
    if delete {
        world.remove(focus);
        return;
    }
    if let Some(step) = turn {
        let (_, dir) = world.item(focus);
        world.turn(focus, (dir + step) % 6);
    }
    let Focus::Arm { arm, cursor } = focus else {
        return;
    };
    let len = world.sim.arms[arm].tape.len();
    let mut cursor = cursor.min(len);
    if keys.just_pressed(KeyCode::ArrowLeft) {
        cursor = cursor.saturating_sub(1);
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        cursor = (cursor + 1).min(len);
    }
    let typed = KEYS
        .into_iter()
        .find(|k| keys.just_pressed(k.0))
        .map(|k| k.1);
    let tape = &mut world.sim.arms[arm].tape;
    if let Some(instr) = typed {
        if cursor < tape.len() {
            tape[cursor] = instr;
        } else {
            tape.push(instr);
        }
        cursor += 1;
    } else if keys.just_pressed(KeyCode::Backspace) && cursor > 0 {
        tape.remove(cursor - 1);
        cursor -= 1;
    }
    world.focus = Some(Focus::Arm { arm, cursor });
}

fn tape_line(world: &World, i: usize) -> String {
    let arm = &world.sim.arms[i];
    let cursor = match world.focus {
        Some(Focus::Arm { arm, cursor }) if arm == i => Some(cursor),
        _ => None,
    };
    let mut out = format!("arm {i:<3}{}", if arm.stalled { "! " } else { "  " });
    let pc = if arm.tape.is_empty() {
        0
    } else {
        arm.pc % arm.tape.len()
    };
    for (k, instr) in arm.tape.iter().enumerate() {
        if cursor == Some(k) {
            out.push('|');
        }
        let g = instr_char(*instr);
        if k == pc {
            out.push_str(&format!("({g})"));
        } else {
            out.push(g);
        }
        out.push(' ');
    }
    if cursor.is_some_and(|c| c >= arm.tape.len()) {
        out.push('|');
    }
    out
}

fn strip(
    world: Res<World>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Transform, &Projection), With<Camera2d>>,
    mut rows: Query<(
        &mut TapeRow,
        &mut Visibility,
        &mut BackgroundColor,
        &Children,
    )>,
    mut texts: Query<&mut Text>,
) {
    let (transform, projection) = camera.into_inner();
    let Some(viewport) = Viewport::of(&window, transform, projection) else {
        return;
    };
    let shown: Vec<usize> = world
        .sim
        .arms
        .iter()
        .enumerate()
        .filter(|(_, a)| viewport.shows(px(a.pivot)))
        .map(|(i, _)| i)
        .take(STRIP_ROWS)
        .collect();
    for (mut row, mut vis, mut bg, children) in &mut rows {
        row.arm = shown.get(row.slot).copied();
        let Some(arm) = row.arm else {
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Inherited;
        bg.0 = if world.focus.and_then(Focus::arm) == Some(arm) {
            PANEL_LIT
        } else {
            PANEL
        };
        if let Some(mut t) = children.first().and_then(|c| texts.get_mut(*c).ok()) {
            t.0 = tape_line(&world, arm);
        }
    }
}

fn draw_machine(gizmos: &mut Gizmos, item: Item, at: Hex, dir: usize, color: Color) {
    match item {
        Item::Arm => {
            let pivot = px(at);
            gizmos.circle_2d(pivot, HEX * 0.25, color);
            gizmos.line_2d(pivot, px(at.add(DIRS[dir % 6])), color);
        }
        Item::Glyph(kind) => {
            let g = Glyph { kind, at, dir };
            let slots: Vec<Vec2> = g.slots().map(px).collect();
            match kind {
                GlyphKind::Source => gizmos.linestrip_2d(corners(slots[0], HEX * 0.8), color),
                GlyphKind::Output => {
                    for s in &slots {
                        gizmos.linestrip_2d(corners(*s, HEX * 0.8), color);
                        gizmos.circle_2d(*s, HEX * 0.15, color);
                    }
                    for (a, b, kind) in kind.rule().before {
                        if let Some(kind) = kind {
                            draw_bond(gizmos, slots[*a], slots[*b], *kind, color);
                        }
                    }
                }
                GlyphKind::Bonder | GlyphKind::SecondBond => {
                    let tri: Vec<Vec2> = slots.iter().chain(&slots[..1]).copied().collect();
                    gizmos.linestrip_2d(tri.iter().copied(), color);
                    gizmos.circle_2d(tri[0], HEX * 0.12, color);
                    if kind == GlyphKind::SecondBond {
                        let c = (tri[0] + tri[1] + tri[2]) / 3.0;
                        gizmos.linestrip_2d(tri.iter().map(|p| c + (*p - c) * 0.6), color);
                    }
                }
            }
        }
    }
}

fn draw_bond(gizmos: &mut Gizmos, a: Vec2, c: Vec2, kind: BondKind, color: Color) {
    match kind {
        BondKind::Single => gizmos.line_2d(a, c, color),
        BondKind::Double => {
            let n = (c - a).perp().normalize() * 3.0;
            gizmos.line_2d(a + n, c + n, color);
            gizmos.line_2d(a - n, c - n, color);
        }
    }
}

fn draw(
    world: Res<World>,
    mut gizmos: Gizmos,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Transform, &Projection), With<Camera2d>>,
) {
    let dim = Color::srgb(0.18, 0.18, 0.18);
    let pad = Color::srgb(0.75, 0.75, 0.75);
    let atom = Color::srgb(0.85, 0.85, 0.85);
    let torn = Color::srgb(0.4, 0.4, 0.4);
    let arm_color = Color::srgb(0.55, 0.55, 0.55);
    let picked = Color::WHITE;
    let s = &world.sim;

    let (transform, projection) = camera.into_inner();
    if let Some(v) = Viewport::of(&window, transform, projection) {
        let col = HEX * 3f32.sqrt();
        let row = HEX * 1.5;
        if (v.size.x / col) * (v.size.y / row) < MAX_GRID_CELLS {
            let (lo, hi) = (v.cam - v.half(), v.cam + v.half());
            for r in ((lo.y / row).floor() as i32 - 1)..=((hi.y / row).ceil() as i32 + 1) {
                let q0 = (lo.x / col - r as f32 / 2.0).floor() as i32 - 1;
                let q1 = (hi.x / col - r as f32 / 2.0).ceil() as i32 + 1;
                for q in q0..=q1 {
                    gizmos.linestrip_2d(corners(px(Hex::new(q, r)), HEX * 0.95), dim);
                }
            }
        }
    }
    for (i, g) in s.glyphs.iter().enumerate() {
        let color = if world.focus == Some(Focus::Glyph(i)) {
            picked
        } else {
            pad
        };
        draw_machine(&mut gizmos, Item::Glyph(g.kind), g.at, g.dir, color);
    }
    for b in &s.bonds {
        let (Some(a), Some(c)) = (s.atoms[b.a], s.atoms[b.b]) else {
            continue;
        };
        draw_bond(&mut gizmos, px(a.pos), px(c.pos), b.kind, atom);
    }
    for (kept, lost, kind) in &s.torn {
        draw_bond(&mut gizmos, px(*kept), px(*lost), *kind, torn);
    }
    for (_, a) in s.live_atoms() {
        gizmos.circle_2d(px(a.pos), HEX * 0.4, atom);
    }
    for (i, arm) in s.arms.iter().enumerate() {
        let color = if world.focus.and_then(Focus::arm) == Some(i) {
            picked
        } else {
            arm_color
        };
        draw_machine(&mut gizmos, Item::Arm, arm.pivot, arm.dir, color);
        if arm.held.is_some() {
            gizmos.circle_2d(px(arm.hand()), HEX * 0.5, color);
        }
        if arm.stalled {
            gizmos.circle_2d(px(arm.pivot), HEX * 0.5, picked);
            if let Some(j) = s.second_hand(i) {
                gizmos.circle_2d(px(s.arms[j].hand()), HEX * 0.65, picked);
            }
        }
    }
    if let (Some(held), Some(p)) = (world.held, world.pointer) {
        let at = hex_at(p);
        gizmos.linestrip_2d(corners(px(at), HEX * 0.9), picked);
        draw_machine(&mut gizmos, held.item, at, held.dir, picked);
    }
}

fn text(world: Res<World>, mut label: Single<&mut Text, With<Hud>>) {
    let s = &world.sim;
    let mut out = format!(
        "tick {}  delivered {}  {}\n",
        s.tick,
        s.delivered,
        if world.running { "running" } else { "paused" }
    );
    if let Some(held) = world.held {
        out.push_str(&format!(
            "holding {}: A/D turn  Z delete  release on a hex to drop, elsewhere to put back\n",
            held.item.name()
        ));
    } else if let Some(f) = world.focus {
        out.push_str(&format!(
            "focus {}: A/D turn  Z delete  esc done\n",
            world.item(f).0.name()
        ));
        if f.arm().is_some() {
            out.push_str("tape");
            for (_, _, c, name) in KEYS {
                out.push_str(&format!("  {c} {name}"));
            }
            out.push_str("  arrows move  backspace remove\n");
        }
    }
    out.push_str(
        "space pause/run  . step  wheel zoom  right/middle-drag pan  click a machine to focus  drag to move  drag from the palette to place",
    );
    label.0 = out;
}

#[cfg(not(target_arch = "wasm32"))]
mod shot {
    use super::*;
    use bevy::app::{AppExit, ScheduleRunnerPlugin};
    use bevy::camera::RenderTarget;
    use bevy::image::Image;
    use bevy::input::ButtonState;
    use bevy::input::keyboard::{Key, KeyboardInput, NativeKey};
    use bevy::render::render_resource::{TextureFormat, TextureUsages};
    use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
    use bevy::ui::IsDefaultUiCamera;
    use sim::{Atom, AtomKind, Bond};
    use std::path::PathBuf;
    use std::time::Duration;

    const WIDE_SCALE: f32 = 1.5;

    #[derive(Resource)]
    struct Shot {
        path: PathBuf,
        wide: bool,
        keys: Vec<KeyCode>,
        frames: u32,
    }

    #[derive(Resource)]
    struct Target(Handle<Image>);

    fn scene(name: &str, ticks: u64) -> (World, bool, Vec<KeyCode>) {
        use KeyCode::*;
        let mut world = World::new();
        world.running = false;
        world.pointer = Some(px(Hex::new(3, -3)));
        let mut keys = Vec::new();
        let mut wide = false;
        match name {
            "micro" => world.focus_arm(0),
            "wide" => wide = true,
            "focus" => {
                world
                    .sim
                    .arms
                    .push(Arm::new(Hex::new(3, -3), 0, Vec::new()));
                world.focus_arm(world.sim.arms.len() - 1);
                keys = vec![KeyF, KeyE, KeyE, KeyR, KeyQ, KeyQ];
            }
            "hold" => {
                world.lift(Item::Glyph(GlyphKind::Bonder), 0, None);
                keys = vec![KeyD, KeyD];
            }
            "output" => {
                let mut sim = Sim::empty();
                for (k, kind) in [BondKind::Single, BondKind::Double, BondKind::Double]
                    .into_iter()
                    .enumerate()
                {
                    let at = Hex::new(k as i32 * 3 - 4, -1);
                    sim.glyphs.push(Glyph {
                        kind: GlyphKind::Output,
                        at,
                        dir: if k == 2 { 3 } else { 0 },
                    });
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
                let mut sim = Sim::empty();
                sim.glyphs.push(Glyph {
                    kind: GlyphKind::Bonder,
                    at: Hex::new(1, -1),
                    dir: 0,
                });
                sim.arms
                    .push(Arm::new(Hex::new(1, 0), 1, vec![Instr::Grab, Instr::Wait]));
                for pos in [Hex::new(1, -1), Hex::new(2, -1), Hex::new(2, -2)] {
                    sim.spawn(Atom {
                        kind: AtomKind::Base,
                        pos,
                    });
                }
                world.sim = sim;
                world.focus_arm(0);
            }
            "twohands" => {
                let mut sim = Sim::empty();
                sim.arms
                    .push(Arm::new(Hex::new(0, 0), 0, vec![Instr::Grab, Instr::RotCw]));
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
                world.focus_arm(0);
            }
            "tear" => {
                let mut sim = Sim::empty();
                sim.glyphs.push(Glyph {
                    kind: GlyphKind::Bonder,
                    at: Hex::new(1, -1),
                    dir: 0,
                });
                let chain: Vec<usize> = [Hex::new(-1, -1), Hex::new(0, -1), Hex::new(1, -1)]
                    .into_iter()
                    .chain([Hex::new(2, -1), Hex::new(2, -2)])
                    .map(|pos| {
                        sim.spawn(Atom {
                            kind: AtomKind::Base,
                            pos,
                        })
                    })
                    .collect();
                for (k, kind) in [BondKind::Single, BondKind::Double].into_iter().enumerate() {
                    sim.bonds.push(Bond {
                        a: chain[k],
                        b: chain[k + 1],
                        kind,
                    });
                }
                world.sim = sim;
            }
            other => panic!("unknown scene {other}"),
        }
        for _ in 0..ticks {
            world.sim.step();
        }
        (world, wide, keys)
    }

    pub fn configure(app: &mut App) -> bool {
        let args: Vec<String> = std::env::args().collect();
        let [_, flag, path, view, ticks] = args.as_slice() else {
            return false;
        };
        assert_eq!(
            flag, "--shot",
            "usage: ziral --shot <png> micro|wide|focus|hold|output|bonding|twohands|tear <ticks>"
        );
        let ticks: u64 = ticks.parse().expect("ticks must be an integer");
        let (world, wide, keys) = scene(view, ticks);
        app.insert_resource(world)
            .insert_resource(Shot {
                path: PathBuf::from(path),
                wide,
                keys,
                frames: 0,
            })
            .add_plugins(
                DefaultPlugins
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
            .add_systems(Update, capture);
        true
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
            px(Hex::new(0, -1))
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
        target: Res<Target>,
        window: Single<Entity, With<PrimaryWindow>>,
        mut keyboard: MessageWriter<KeyboardInput>,
        mut exit: MessageWriter<AppExit>,
    ) {
        shot.frames += 1;
        let k = shot.frames as usize;
        if k >= 3 && k - 3 < shot.keys.len() {
            keyboard.write(key(shot.keys[k - 3], ButtonState::Released, *window));
        }
        if k >= 2 && k - 2 < shot.keys.len() {
            keyboard.write(key(shot.keys[k - 2], ButtonState::Pressed, *window));
        }
        if shot.frames == 12 {
            commands
                .spawn(Screenshot::image(target.0.clone()))
                .observe(save_to_disk(shot.path.clone()));
        }
        if shot.frames == 40 {
            exit.write(AppExit::Success);
        }
    }
}
