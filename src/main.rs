mod sim;

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use sim::{Arm, BondKind, Glyph, GlyphKind, Hex, Instr, Sim};

const HEX: f32 = 20.0;
const TICK_HZ: f64 = 6.0;
const MICRO_SCALE: f32 = 0.5;
const MAX_GRID_CELLS: f32 = 6000.0;
const STRIP_ROWS: usize = 8;
const DRAG_PX: f32 = 6.0;

#[derive(Clone, Copy, PartialEq, Eq, Component)]
enum Item {
    Arm,
    Bonder,
    SecondBond,
    Source,
    Output,
}

const PALETTE: [(Item, &str); 5] = [
    (Item::Arm, "arm"),
    (Item::Bonder, "bonder"),
    (Item::SecondBond, "second bond"),
    (Item::Source, "source"),
    (Item::Output, "output"),
];

#[derive(Clone, Debug)]
enum Machine {
    Arm(Arm),
    Glyph(Glyph),
}

impl Item {
    fn machine(self, at: Hex) -> Machine {
        let glyph = |kind| Machine::Glyph(Glyph { kind, at, dir: 0 });
        match self {
            Item::Arm => Machine::Arm(Arm::new(at, 0, Vec::new())),
            Item::Bonder => glyph(GlyphKind::Bonder),
            Item::SecondBond => glyph(GlyphKind::SecondBond),
            Item::Source => glyph(GlyphKind::Source),
            Item::Output => glyph(GlyphKind::Output),
        }
    }
}

impl Machine {
    fn at(&self) -> Hex {
        match self {
            Machine::Arm(a) => a.pivot,
            Machine::Glyph(g) => g.at,
        }
    }

    fn moved(&self, at: Hex) -> Machine {
        match self {
            Machine::Arm(a) => Machine::Arm(Arm {
                pivot: at,
                held: None,
                ..a.clone()
            }),
            Machine::Glyph(g) => Machine::Glyph(Glyph { at, ..*g }),
        }
    }

    fn turn(&mut self, cw: bool) {
        let dir = match self {
            Machine::Arm(a) => &mut a.dir,
            Machine::Glyph(g) => &mut g.dir,
        };
        *dir = (*dir + if cw { 1 } else { 5 }) % 6;
        if let Machine::Arm(a) = self {
            a.held = None;
        }
    }

    fn name(&self) -> &'static str {
        match self {
            Machine::Arm(_) => "arm",
            Machine::Glyph(g) => match g.kind {
                GlyphKind::Source => "source",
                GlyphKind::Bonder => "bonder",
                GlyphKind::SecondBond => "second bond",
                GlyphKind::Output => "output",
            },
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Focus {
    Arm(usize),
    Glyph(usize),
}

#[derive(Clone, Debug)]
struct Held {
    machine: Machine,
    from: Option<Focus>,
}

#[derive(Clone, Copy)]
struct Press {
    screen: Vec2,
    target: Option<Focus>,
}

#[derive(Resource)]
struct World {
    sim: Sim,
    running: bool,
    focus: Option<Focus>,
    cursor: usize,
    held: Option<Held>,
    press: Option<Press>,
    pointer: Option<Vec2>,
    rows: Vec<usize>,
}

impl World {
    fn new() -> Self {
        World {
            sim: sim::preloaded(),
            running: true,
            focus: None,
            cursor: 0,
            held: None,
            press: None,
            pointer: None,
            rows: Vec::new(),
        }
    }

    fn machine(&self, f: Focus) -> Machine {
        match f {
            Focus::Arm(i) => Machine::Arm(self.sim.arms[i].clone()),
            Focus::Glyph(i) => Machine::Glyph(self.sim.glyphs[i]),
        }
    }

    fn put(&mut self, f: Option<Focus>, m: Machine) -> Focus {
        match (f, m) {
            (Some(Focus::Arm(i)), Machine::Arm(a)) => {
                self.sim.arms[i] = a;
                Focus::Arm(i)
            }
            (Some(Focus::Glyph(i)), Machine::Glyph(g)) => {
                self.sim.glyphs[i] = g;
                Focus::Glyph(i)
            }
            (_, Machine::Arm(a)) => {
                self.sim.arms.push(a);
                Focus::Arm(self.sim.arms.len() - 1)
            }
            (_, Machine::Glyph(g)) => {
                self.sim.glyphs.push(g);
                Focus::Glyph(self.sim.glyphs.len() - 1)
            }
        }
    }

    fn remove(&mut self, f: Focus) {
        match f {
            Focus::Arm(i) => {
                self.sim.arms.remove(i);
            }
            Focus::Glyph(i) => {
                self.sim.glyphs.remove(i);
            }
        }
        self.focus = None;
    }

    fn lift(&mut self, from: Option<Focus>, machine: Machine) {
        self.held = Some(Held { machine, from });
        self.focus = None;
    }

    fn drop_at(&mut self, at: Option<Hex>) {
        let Some(held) = self.held.take() else { return };
        let target = match (at, held.from) {
            (Some(at), _) => held.machine.moved(at),
            (None, Some(_)) => held.machine.moved(held.machine.at()),
            (None, None) => return,
        };
        let f = self.put(held.from, target);
        self.focus = Some(f);
        self.cursor = self.tape_len(f);
    }

    fn tape_len(&self, f: Focus) -> usize {
        match f {
            Focus::Arm(i) => self.sim.arms[i].tape.len(),
            Focus::Glyph(_) => 0,
        }
    }

    fn hit(&self, at: Vec2) -> Option<Focus> {
        let near = |h: Hex| px(h).distance(at) < HEX;
        let arm = self
            .sim
            .arms
            .iter()
            .enumerate()
            .filter(|(_, a)| near(a.pivot))
            .min_by(|x, y| {
                px(x.1.pivot)
                    .distance(at)
                    .total_cmp(&px(y.1.pivot).distance(at))
            })
            .map(|(i, _)| Focus::Arm(i));
        arm.or_else(|| {
            self.sim
                .glyphs
                .iter()
                .position(|g| near(g.at))
                .map(Focus::Glyph)
        })
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

fn screen_to_world(cam: Vec2, size: Vec2, p: Vec2, scale: f32) -> Vec2 {
    cam + Vec2::new(p.x - size.x / 2.0, size.y / 2.0 - p.y) * scale
}

fn glyph_of(instr: Instr) -> char {
    match instr {
        Instr::Grab => 'F',
        Instr::Drop => 'R',
        Instr::RotCw => 'E',
        Instr::RotCcw => 'Q',
        Instr::Wait => '.',
    }
}

const TYPED: [(KeyCode, Instr); 5] = [
    (KeyCode::KeyF, Instr::Grab),
    (KeyCode::KeyR, Instr::Drop),
    (KeyCode::KeyE, Instr::RotCw),
    (KeyCode::KeyQ, Instr::RotCcw),
    (KeyCode::KeyX, Instr::Wait),
];

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
struct TapeRow(usize);

fn button(node: Node) -> impl Bundle {
    (
        Button,
        node,
        BorderColor::all(Color::srgb(0.5, 0.5, 0.5)),
        BackgroundColor(Color::srgb(0.12, 0.12, 0.12)),
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
            for k in 0..STRIP_ROWS {
                col.spawn((
                    TapeRow(k),
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
    let Projection::Orthographic(ortho) = &mut *projection else {
        return;
    };
    let size = window.size();
    if scroll.delta.y != 0.0
        && let Some(c) = window.cursor_position()
    {
        let cam = transform.translation.truncate();
        let before = screen_to_world(cam, size, c, ortho.scale);
        let notches = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
        };
        ortho.scale = (ortho.scale * (-notches * 0.15).exp()).clamp(0.05, 40.0);
        let after = screen_to_world(cam, size, c, ortho.scale);
        transform.translation += (before - after).extend(0.0);
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
    let Projection::Orthographic(ortho) = projection else {
        return;
    };
    let screen = window.cursor_position();
    if let Some(c) = screen {
        world.pointer = Some(screen_to_world(
            transform.translation.truncate(),
            window.size(),
            c,
            ortho.scale,
        ));
    }
    let over_ui = palette.iter().any(|(_, i)| *i != Interaction::None)
        || rows.iter().any(|(_, i)| *i != Interaction::None);
    let at = world.pointer.map(hex_at);

    if buttons.just_pressed(MouseButton::Left) {
        if let Some((item, _)) = palette.iter().find(|(_, i)| **i == Interaction::Pressed) {
            let machine = item.machine(at.unwrap_or(Hex::new(0, 0)));
            world.lift(None, machine);
        } else if let Some((row, _)) = rows.iter().find(|(_, i)| **i == Interaction::Pressed) {
            if let Some(&arm) = world.rows.get(row.0) {
                world.focus = Some(Focus::Arm(arm));
                world.cursor = world.sim.arms[arm].tape.len();
            }
        } else if !over_ui && let (Some(c), Some(p)) = (screen, world.pointer) {
            let target = world.hit(p);
            world.press = Some(Press { screen: c, target });
        }
    }
    if buttons.pressed(MouseButton::Left)
        && world.held.is_none()
        && let (Some(press), Some(c)) = (world.press, screen)
        && let Some(target) = press.target
        && press.screen.distance(c) > DRAG_PX
    {
        let machine = world.machine(target);
        world.lift(Some(target), machine);
    }
    if buttons.just_released(MouseButton::Left) {
        let press = world.press.take();
        if world.held.is_some() {
            let valid = !over_ui && screen.is_some();
            world.drop_at(at.filter(|_| valid));
        } else if let Some(press) = press
            && !over_ui
        {
            world.focus = press.target;
            world.cursor = press.target.map_or(0, |f| world.tape_len(f));
        }
    }

    if keys.just_pressed(KeyCode::Escape) {
        world.drop_at(None);
        world.focus = None;
        return;
    }
    let turn = if keys.just_pressed(KeyCode::KeyA) {
        Some(false)
    } else if keys.just_pressed(KeyCode::KeyD) {
        Some(true)
    } else {
        None
    };
    let delete = keys.just_pressed(KeyCode::KeyZ);
    if let Some(held) = &mut world.held {
        if let Some(cw) = turn {
            held.machine.turn(cw);
        }
        if delete {
            world.held = None;
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
    if let Some(cw) = turn {
        let mut m = world.machine(focus);
        m.turn(cw);
        world.put(Some(focus), m);
    }
    let Focus::Arm(arm) = focus else {
        return;
    };
    let len = world.sim.arms[arm].tape.len();
    let at = world.cursor.min(len);
    if keys.just_pressed(KeyCode::ArrowLeft) {
        world.cursor = at.saturating_sub(1);
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        world.cursor = (at + 1).min(len);
    }
    let typed = TYPED
        .into_iter()
        .find(|(k, _)| keys.just_pressed(*k))
        .map(|(_, i)| i);
    let remove = keys.just_pressed(KeyCode::Backspace);
    let tape = &mut world.sim.arms[arm].tape;
    if let Some(instr) = typed {
        if at < tape.len() {
            tape[at] = instr;
        } else {
            tape.push(instr);
        }
        world.cursor = at + 1;
    } else if remove && at > 0 {
        tape.remove(at - 1);
        world.cursor = at - 1;
    }
}

fn tape_line(world: &World, i: usize) -> String {
    let arm = &world.sim.arms[i];
    let focused = world.focus == Some(Focus::Arm(i));
    let mut out = format!("arm {i:<3}{}", if arm.stalled { "! " } else { "  " });
    let pc = if arm.tape.is_empty() {
        0
    } else {
        arm.pc % arm.tape.len()
    };
    for (k, instr) in arm.tape.iter().enumerate() {
        if focused && k == world.cursor {
            out.push('|');
        }
        let g = glyph_of(*instr);
        if k == pc {
            out.push_str(&format!("({g})"));
        } else {
            out.push(g);
        }
        out.push(' ');
    }
    if focused && world.cursor >= arm.tape.len() {
        out.push('|');
    }
    out
}

fn strip(
    mut world: ResMut<World>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Transform, &Projection), With<Camera2d>>,
    mut rows: Query<(&TapeRow, &mut Visibility, &mut BackgroundColor, &Children)>,
    mut texts: Query<&mut Text>,
) {
    let (transform, projection) = camera.into_inner();
    let Projection::Orthographic(ortho) = projection else {
        return;
    };
    let cam = transform.translation.truncate();
    let half = window.size() * ortho.scale / 2.0;
    world.rows = world
        .sim
        .arms
        .iter()
        .enumerate()
        .filter(|(_, a)| (px(a.pivot) - cam).abs().cmplt(half).all())
        .map(|(i, _)| i)
        .take(STRIP_ROWS)
        .collect();
    for (row, mut vis, mut bg, children) in &mut rows {
        let Some(&arm) = world.rows.get(row.0) else {
            *vis = Visibility::Hidden;
            continue;
        };
        *vis = Visibility::Inherited;
        bg.0 = if world.focus == Some(Focus::Arm(arm)) {
            Color::srgb(0.3, 0.3, 0.3)
        } else {
            Color::srgb(0.12, 0.12, 0.12)
        };
        if let Some(mut t) = children.first().and_then(|c| texts.get_mut(*c).ok()) {
            t.0 = tape_line(&world, arm);
        }
    }
}

fn draw_machine(gizmos: &mut Gizmos, m: &Machine, color: Color) {
    match m {
        Machine::Arm(arm) => {
            let pivot = px(arm.pivot);
            gizmos.circle_2d(pivot, HEX * 0.25, color);
            gizmos.line_2d(pivot, px(arm.hand()), color);
        }
        Machine::Glyph(g) => {
            let slots: Vec<Vec2> = g.slots().iter().map(|h| px(*h)).collect();
            match g.kind {
                GlyphKind::Source => gizmos.linestrip_2d(corners(slots[0], HEX * 0.8), color),
                GlyphKind::Output => {
                    for s in &slots {
                        gizmos.linestrip_2d(corners(*s, HEX * 0.8), color);
                        gizmos.circle_2d(*s, HEX * 0.15, color);
                    }
                    for (a, b, kind) in g.kind.rule().before {
                        draw_bond(gizmos, slots[*a], slots[*b], kind.unwrap(), color);
                    }
                }
                GlyphKind::Bonder | GlyphKind::SecondBond => {
                    let tri: Vec<Vec2> = slots.iter().chain(&slots[..1]).copied().collect();
                    gizmos.linestrip_2d(tri.iter().copied(), color);
                    gizmos.circle_2d(tri[0], HEX * 0.12, color);
                    if g.kind == GlyphKind::SecondBond {
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
    let arm_color = Color::srgb(0.55, 0.55, 0.55);
    let picked = Color::WHITE;
    let s = &world.sim;

    let (transform, projection) = camera.into_inner();
    if let Projection::Orthographic(ortho) = projection {
        let cam = transform.translation.truncate();
        let half = window.size() * ortho.scale / 2.0;
        let col = HEX * 3f32.sqrt();
        let row = HEX * 1.5;
        if (half.x * 2.0 / col) * (half.y * 2.0 / row) < MAX_GRID_CELLS {
            let (lo, hi) = (cam - half, cam + half);
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
        draw_machine(&mut gizmos, &Machine::Glyph(*g), color);
    }
    for b in &s.bonds {
        let (Some(a), Some(c)) = (s.atoms[b.a], s.atoms[b.b]) else {
            continue;
        };
        draw_bond(&mut gizmos, px(a.pos), px(c.pos), b.kind, atom);
    }
    for (_, a) in s.live_atoms() {
        gizmos.circle_2d(px(a.pos), HEX * 0.4, atom);
    }
    for (a, arm) in s.arms.iter().enumerate() {
        let color = if world.focus == Some(Focus::Arm(a)) {
            picked
        } else {
            arm_color
        };
        draw_machine(&mut gizmos, &Machine::Arm(arm.clone()), color);
        if arm.held.is_some() {
            gizmos.circle_2d(px(arm.hand()), HEX * 0.5, color);
        }
        if arm.stalled {
            gizmos.circle_2d(px(arm.pivot), HEX * 0.5, picked);
        }
    }
    if let (Some(held), Some(p)) = (&world.held, world.pointer) {
        let at = hex_at(p);
        gizmos.linestrip_2d(corners(px(at), HEX * 0.9), picked);
        draw_machine(&mut gizmos, &held.machine.moved(at), picked);
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
    if let Some(held) = &world.held {
        out.push_str(&format!(
            "holding {}: A/D turn  Z delete  release on a hex to drop, elsewhere to put back\n",
            held.machine.name()
        ));
    } else if let Some(f) = world.focus {
        out.push_str(&format!(
            "focus {}: A/D turn  Z delete  esc done\n",
            world.machine(f).name()
        ));
        if matches!(f, Focus::Arm(_)) {
            out.push_str(
                "tape F grab  R drop  E cw  Q ccw  X wait  arrows move  backspace remove\n",
            );
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
    use bevy::input::keyboard::{Key, KeyboardInput};
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
            "micro" => world.focus = Some(Focus::Arm(0)),
            "wide" => wide = true,
            "focus" => {
                world
                    .sim
                    .arms
                    .push(Arm::new(Hex::new(3, -3), 0, Vec::new()));
                world.focus = Some(Focus::Arm(world.sim.arms.len() - 1));
                keys = vec![KeyF, KeyE, KeyE, KeyR, KeyQ, KeyQ];
            }
            "hold" => {
                world.lift(None, Item::Bonder.machine(Hex::new(3, -3)));
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
                        pos: at.add(sim::DIRS[0]),
                    });
                    sim.bonds.push(Bond { a, b, kind });
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
            "usage: ziral --shot <png> micro|wide|focus|hold|output <ticks>"
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
        if k >= 2 && k - 2 < shot.keys.len() {
            keyboard.write(KeyboardInput {
                key_code: shot.keys[k - 2],
                logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
                state: ButtonState::Pressed,
                text: None,
                repeat: false,
                window: *window,
            });
        }
        if k >= 3 && k - 3 < shot.keys.len() {
            keyboard.write(KeyboardInput {
                key_code: shot.keys[k - 3],
                logical_key: Key::Unidentified(bevy::input::keyboard::NativeKey::Unidentified),
                state: ButtonState::Released,
                text: None,
                repeat: false,
                window: *window,
            });
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
