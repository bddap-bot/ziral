mod look;
mod sim;

use bevy::asset::RenderAssetUsages;
use bevy::color::{Alpha, Mix};
use bevy::image::Image;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;
use bevy::render::render_resource::TextureFormat;
use bevy::sprite_render::AlphaMode2d;
use bevy::window::PrimaryWindow;
use look::{AtomMark, Glaze, Look, MachineMark, Shape, Skin};
use sim::{Arm, BondKind, DIRS, Glyph, GlyphKind, Hex, Instr, ORIGIN, Sim, Stall};

const HEX: f32 = 20.0;
const TICK_MS: f32 = 400.0;
const MOTION: f32 = 1.0;
const MICRO_SCALE: f32 = 0.5;
const MAX_GRID_CELLS: f32 = 6000.0;
const STRIP_ROWS: usize = 8;
const DRAG_PX: f32 = 6.0;
const LINE_PX: f32 = 3.0;

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

pub const PALETTE: [(Item, &str); 5] = [
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
    grip: Hex,
}

impl Held {
    fn fresh(item: Item) -> Self {
        Held {
            item,
            dir: 0,
            from: None,
            grip: ORIGIN,
        }
    }

    fn turn(&mut self, step: usize) {
        self.dir = (self.dir + step) % 6;
        self.grip = self.grip.turned(step);
    }
}

#[derive(Clone, Copy)]
struct Press {
    screen: Vec2,
    grip: Hex,
}

#[derive(Resource)]
struct World {
    sim: Sim,
    prev: Sim,
    since: f32,
    period: f32,
    motion: f32,
    running: bool,
    focus: Option<Focus>,
    held: Option<Held>,
    down: Option<Press>,
    pointer: Option<Vec2>,
}

impl World {
    fn new(sim: Sim) -> Self {
        World {
            prev: sim.clone(),
            sim,
            since: 0.0,
            period: TICK_MS / 1000.0,
            motion: MOTION,
            running: true,
            focus: None,
            held: None,
            down: None,
            pointer: None,
        }
    }

    fn step(&mut self) {
        self.prev = self.sim.clone();
        self.sim.step();
    }

    fn phase(&self) -> f32 {
        let span = self.period * self.motion;
        if span > 0.0 { self.since / span } else { 1.0 }
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

    fn anchor(&self, f: Focus) -> Hex {
        match f {
            Focus::Arm { arm, .. } => self.sim.arms[arm].pivot,
            Focus::Glyph(i) => self.sim.glyphs[i].at,
        }
    }

    fn turn(&mut self, f: Focus, dir: usize) {
        match f {
            Focus::Arm { arm, .. } => {
                self.sim.arms[arm].dir = dir;
                self.unstall();
            }
            Focus::Glyph(i) => self.sim.glyphs[i].dir = dir,
        }
        self.prev = self.sim.clone();
    }

    fn unstall(&mut self) {
        for a in &mut self.sim.arms {
            a.stall = None;
        }
    }

    fn remove(&mut self, f: Focus) {
        match f {
            Focus::Arm { arm, .. } => {
                self.sim.arms.remove(arm);
                self.unstall();
            }
            Focus::Glyph(i) => {
                self.sim.glyphs.remove(i);
            }
        }
        self.prev = self.sim.clone();
        self.focus = None;
        self.down = None;
    }

    fn lift(&mut self, held: Held) {
        self.held = Some(held);
        self.focus = None;
    }

    fn press(&mut self, screen: Vec2, cell: Hex) {
        self.focus = self.hit(cell);
        let grip = self.focus.map_or(ORIGIN, |f| cell.sub(self.anchor(f)));
        self.down = Some(Press { screen, grip });
    }

    fn drag(&mut self, screen: Vec2) {
        let (Some(down), Some(from)) = (self.down, self.focus) else {
            return;
        };
        if down.screen.distance(screen) <= DRAG_PX {
            return;
        }
        let (item, dir) = self.item(from);
        self.lift(Held {
            item,
            dir,
            from: Some(from),
            grip: down.grip,
        });
    }

    fn release(&mut self, at: Option<Hex>) {
        self.down = None;
        self.drop_at(at);
    }

    fn drop_at(&mut self, at: Option<Hex>) {
        let Some(held) = self.held.take() else { return };
        let at = at.map(|c| c.sub(held.grip));
        match (held.from, at) {
            (Some(f), None) => self.focus = Some(f),
            (Some(f), Some(at)) => {
                match f {
                    Focus::Arm { arm, .. } => self.sim.arms[arm].pivot = at,
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
        self.prev = self.sim.clone();
    }

    fn covers(&self, f: Focus, cell: Hex) -> bool {
        match f {
            Focus::Arm { arm, .. } => self.sim.arms[arm].cells().contains(&cell),
            Focus::Glyph(i) => self.sim.glyphs[i].slots().any(|s| s == cell),
        }
    }

    fn machines(&self) -> impl Iterator<Item = Focus> + '_ {
        let arms = self.sim.arms.iter().enumerate().map(|(arm, a)| Focus::Arm {
            arm,
            cursor: a.tape.len(),
        });
        arms.chain((0..self.sim.glyphs.len()).map(Focus::Glyph))
    }

    fn hit(&self, cell: Hex) -> Option<Focus> {
        self.machines()
            .find(|f| self.anchor(*f) == cell)
            .or_else(|| self.machines().find(|f| self.covers(*f, cell)))
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
    app.insert_resource(World::new(sim::preloaded()))
        .insert_resource(ClearColor(brass(0.65)))
        .add_systems(Startup, (fire_kiln, spawn_ui))
        .add_systems(
            Update,
            (run_ticks, view, edit, tapes, board, draw, text).chain(),
        );
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
        BorderColor::all(brass(0.5)),
        BackgroundColor(strip(false)),
    )
}

fn spawn_ui(mut commands: Commands) {
    commands.spawn((
        Hud,
        Text::new(""),
        TextColor(IVORY),
        TextFont::from_font_size(16.0),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(8.0),
            top: Val::Px(8.0),
            padding: UiRect::axes(Val::Px(6.0), Val::Px(2.0)),
            ..default()
        },
        BackgroundColor(strip(false)),
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
                    children![(
                        Text::new(name),
                        TextColor(IVORY),
                        TextFont::from_font_size(16.0)
                    )],
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
                    children![(
                        Text::new(""),
                        TextColor(IVORY),
                        TextFont::from_font_size(15.0)
                    )],
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
    if keys.just_pressed(KeyCode::Space) {
        world.running = !world.running;
    }
    if keys.just_pressed(KeyCode::Period) {
        world.running = false;
        world.since = 0.0;
        world.step();
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
            world.lift(Held::fresh(*item));
        } else if let Some((row, _)) = rows.iter().find(|(_, i)| **i == Interaction::Pressed) {
            if let Some(arm) = row.arm.filter(|a| *a < world.sim.arms.len()) {
                world.focus_arm(arm);
            }
        } else if !over_ui && let (Some(c), Some(cell)) = (screen, at) {
            world.press(c, cell);
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

    if keys.just_pressed(KeyCode::Escape) {
        world.release(None);
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
            held.turn(step);
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
    let mut out = format!(
        "arm {i:<3}{}",
        if arm.stall.is_some() { "! " } else { "  " }
    );
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

fn tapes(
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
        bg.0 = strip(world.focus.and_then(Focus::arm) == Some(arm));
        if let Some(mut t) = children.first().and_then(|c| texts.get_mut(*c).ok()) {
            t.0 = tape_line(&world, arm);
        }
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

const LINK_WIDTH: f32 = 0.22;
const BOND_WIDTH: f32 = 0.14;

#[derive(Resource)]
struct Kiln {
    circle: Handle<Mesh>,
    hexagon: Handle<Mesh>,
    bar: Handle<Mesh>,
    link: Handle<Mesh>,
    bond: Handle<Mesh>,
    rim: Handle<Mesh>,
    ring: Handle<Mesh>,
    tiled: Option<Tiling>,
    glaze: [Handle<ColorMaterial>; 7],
    patina: Handle<ColorMaterial>,
    skins: Vec<(Skin, [Handle<ColorMaterial>; 2])>,
    highlight: Handle<ColorMaterial>,
}

impl Kiln {
    fn material(&self, glaze: Glaze) -> &Handle<ColorMaterial> {
        &self.glaze[glaze as usize]
    }

    fn skin(&self, skin: Skin, faint: bool) -> &Handle<ColorMaterial> {
        let (_, fired) = self
            .skins
            .iter()
            .find(|(s, _)| *s == skin)
            .unwrap_or_else(|| panic!("{skin:?} was never fired"));
        &fired[usize::from(faint)]
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

fn fire(skin: Skin) -> Image {
    let mut image = skin.decode();
    assert_eq!(
        image.texture_descriptor.format,
        TextureFormat::Rgba8UnormSrgb,
        "{skin:?} must decode to rgba8"
    );
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
                    next.push(if c == 3 {
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
    mut images: ResMut<Assets<Image>>,
    mut gizmo: ResMut<GizmoConfigStore>,
) {
    gizmo.config_mut::<DefaultGizmoConfigGroup>().0.line.width = LINE_PX;
    let skins = look::skins()
        .map(|skin| {
            let texture = images.add(fire(skin));
            let fired = [(1.0, AlphaMode2d::Opaque), (0.5, AlphaMode2d::Blend)].map(
                |(alpha, alpha_mode)| {
                    materials.add(ColorMaterial {
                        color: Color::WHITE.with_alpha(alpha),
                        alpha_mode,
                        texture: Some(texture.clone()),
                        ..default()
                    })
                },
            );
            (skin, fired)
        })
        .collect();
    commands.insert_resource(Kiln {
        circle: meshes.add(Circle::new(1.0)),
        hexagon: meshes.add(RegularPolygon::new(1.0, 6)),
        bar: meshes.add(Rectangle::new(1.0, 1.0)),
        link: meshes.add(band(LINK_WIDTH / 3f32.sqrt())),
        bond: meshes.add(band(BOND_WIDTH / 3f32.sqrt())),
        rim: meshes.add(Annulus::new(0.85, 1.0)),
        ring: meshes.add(Annulus::new(0.7, 1.0)),
        tiled: None,
        glaze: Glaze::ALL.map(|g| materials.add(g.color())),
        patina: materials.add(Glaze::Brass.color().with_alpha(0.5)),
        skins,
        highlight: materials.add(IVORY.with_alpha(0.7)),
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
}

impl Painter<'_, '_, '_, '_, '_> {
    fn fill(
        &mut self,
        mesh: &Handle<Mesh>,
        material: &Handle<ColorMaterial>,
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

    fn channel(&mut self, a: Vec2, b: Vec2, glaze: Glaze, z: f32) {
        let kiln = self.kiln;
        self.bar(&kiln.bar, kiln.material(glaze), a, b, 2.0, z);
    }

    fn bead(&mut self, at: Vec2, look: Look<AtomMark>) {
        let kiln = self.kiln;
        self.stamp(
            &kiln.circle,
            kiln.skin(look.skin, false),
            at,
            HEX * 0.4,
            0.4,
        );
        self.stamp(&kiln.rim, &kiln.patina, at, HEX * 0.4, 0.42);
        match look.marking {
            AtomMark::Highlight => {
                let up_left = at + Vec2::new(-0.15, 0.15) * HEX;
                self.stamp(&kiln.circle, &kiln.highlight, up_left, HEX * 0.1, 0.45);
            }
        }
    }

    fn bond(&mut self, a: Vec2, c: Vec2, kind: BondKind, faint: bool) {
        let look = look::bond(kind);
        let Shape::Bars(n) = look.shape else {
            unworn(look)
        };
        let kiln = self.kiln;
        let material = kiln.skin(look.skin, faint);
        let side = (c - a).perp().normalize_or_zero() * HEX * 0.16;
        let z = if faint { 0.12 } else { 0.2 };
        for k in 0..n {
            let off = side * (2.0 * k as f32 - (n as f32 - 1.0));
            self.bar(&kiln.bond, material, a + off, c + off, HEX * BOND_WIDTH, z);
        }
    }

    fn horseshoe(&mut self, at: Vec2, r: f32, toward: Vec2, glaze: Glaze) {
        use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};
        let turn = toward.to_angle() - (FRAC_PI_2 + 3.0 * FRAC_PI_4);
        let iso = Isometry2d::new(at, Rot2::radians(turn));
        self.gizmos.arc_2d(iso, 3.0 * FRAC_PI_2, r, glaze.color());
    }

    fn arm(&mut self, pivot: Vec2, hand: Vec2, ring: f32, look: Look<MachineMark>) {
        let kiln = self.kiln;
        let material = kiln.skin(look.skin, false);
        self.bar(&kiln.link, material, pivot, hand, HEX * LINK_WIDTH, 0.28);
        self.stamp(&kiln.circle, material, pivot, HEX * 0.3, 0.3);
        self.stamp(
            &kiln.circle,
            kiln.material(Glaze::Clay),
            pivot,
            HEX * 0.1,
            0.31,
        );
        match look.marking {
            MachineMark::Hand(glaze) => self.horseshoe(hand, HEX * ring, pivot - hand, glaze),
            _ => unworn(look),
        }
    }

    fn machine(&mut self, item: Item, at: Hex, dir: usize) {
        let look = look::machine(item);
        let kind = match (item, look.marking) {
            (Item::Arm, MachineMark::Hand(_)) => {
                let hand = px(at.add(DIRS[dir % 6]));
                return self.arm(px(at), hand, RING_OPEN, look);
            }
            (Item::Arm, _) | (Item::Glyph(_), MachineMark::Hand(_)) => unworn(look),
            (Item::Glyph(kind), _) => kind,
        };
        let kiln = self.kiln;
        let surface = kiln.skin(look.skin, false);
        let glaze = kiln.material(look.glaze);
        let slots: Vec<Vec2> = Glyph { kind, at, dir }.slots().map(px).collect();
        for s in &slots {
            self.stamp(&kiln.hexagon, surface, *s, HEX * 0.8, 0.1);
        }
        match look.marking {
            MachineMark::Dot => {
                self.stamp(&kiln.ring, glaze, slots[0], HEX * 0.6, 0.15);
                self.stamp(&kiln.circle, glaze, slots[0], HEX * 0.15, 0.15);
            }
            MachineMark::Spokes(n) => {
                let c = slots.iter().sum::<Vec2>() / slots.len() as f32;
                for (i, s) in slots.iter().enumerate() {
                    let next = slots[(i + 1) % slots.len()];
                    self.channel(*s, next, look.glaze, 0.13);
                    let side = (*s - c).perp().normalize_or_zero() * 2.5;
                    for k in 0..n {
                        let off = side * (2.0 * k as f32 - (n as f32 - 1.0));
                        self.channel(c + off, *s + off, look.glaze, 0.13);
                    }
                }
                for (s, slot) in slots.iter().zip(kind.rule().slots) {
                    if slot.consumed {
                        self.stamp(&kiln.ring, glaze, *s, HEX * 0.2, 0.14);
                    }
                }
            }
            MachineMark::Cup => {
                for s in &slots {
                    self.stamp(&kiln.circle, surface, *s, HEX * 0.5, 0.15);
                    self.stamp(&kiln.rim, kiln.material(Glaze::Brass), *s, HEX * 0.5, 0.16);
                }
                for (a, b, kind) in kind.rule().before {
                    if let Some(kind) = kind {
                        self.bond(slots[*a], slots[*b], *kind, true);
                    }
                }
            }
            MachineMark::Hand(_) => unworn(look),
        }
    }
}

const RING_CLOSED: f32 = 0.5;
const RING_OPEN: f32 = 0.9;

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

fn reach(arm: &Arm) -> Vec2 {
    px(arm.hand()) - px(arm.pivot)
}

fn turn_between(from: &Arm, to: &Arm) -> f32 {
    use std::f32::consts::{PI, TAU};
    (reach(to).to_angle() - reach(from).to_angle() + PI).rem_euclid(TAU) - PI
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
            let pivot = frame.arms[i].pivot;
            let turn = turn_between(a, b) * e;
            let sweep = |v: Vec2| pivot + Vec2::from_angle(turn).rotate(v - pivot);
            let pose = &mut frame.arms[i];
            pose.hand = sweep(pose.hand);
            pose.ring = grip(a.holding) + (grip(b.holding) - grip(a.holding)) * e;
            if turn != 0.0
                && a.holding
                && let Some(id) = prev.atom_at(a.hand())
            {
                for id in prev.component(id) {
                    frame.atoms[id] = frame.atoms[id].map(sweep);
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
        fn draw(bits: &mut u32, steps: u32) -> u32 {
            let d = *bits % steps;
            *bits /= steps;
            d
        }
        let bits = &mut look::scramble(cell);
        let mut unit = |half: f32| half * (draw(bits, 64) as f32 / 31.5 - 1.0);
        Swing {
            creep: SWING.creep + unit(SPREAD.creep),
            release: SWING.release + unit(SPREAD.release),
            run: SWING.run + unit(SPREAD.run),
            decay: SWING.decay + unit(SPREAD.decay),
            half_bounces: SWING.half_bounces - SPREAD.half_bounces
                + draw(bits, 2 * SPREAD.half_bounces + 1),
        }
    }

    fn at(&self, t: f32) -> f32 {
        if t >= 1.0 {
            return 1.0;
        }
        if t < self.release {
            let u = t / self.release;
            return self.creep * u * u * (3.0 - 2.0 * u);
        }
        if t < self.release + self.run {
            return self.creep.lerp(1.0, (t - self.release) / self.run);
        }
        let speed = (1.0 - self.creep) / self.run;
        let omega =
            std::f32::consts::PI * self.half_bounces as f32 / (1.0 - self.release - self.run);
        let after = t - self.release - self.run;
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
                            rotation: Quat::from_rotation_z(((60 * tile.turn) as f32).to_radians()),
                            scale: Vec3::splat(HEX * 0.95),
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
    };
    let s = &world.sim;
    for (i, g) in s.glyphs.iter().enumerate() {
        p.machine(Item::Glyph(g.kind), g.at, g.dir);
        if world.focus == Some(Focus::Glyph(i)) {
            p.gizmos.linestrip_2d(corners(px(g.at), HEX * 0.9), IVORY);
        }
    }
    let f = Frame::between(&world.prev, s, world.phase());
    for b in &f.sim.bonds {
        let (Some(a), Some(c)) = (f.atoms[b.a], f.atoms[b.b]) else {
            continue;
        };
        p.bond(a, c, b.kind, false);
    }
    for (kept, lost, kind) in &f.sim.torn {
        let (a, b) = (px(*kept), px(*lost));
        p.bond(a, a.lerp(b, 0.5), *kind, true);
    }
    for (at, atom) in f.atoms.iter().zip(&f.sim.atoms) {
        if let (Some(at), Some(atom)) = (at, atom) {
            p.bead(*at, look::atom(atom.kind));
        }
    }
    for (i, arm) in f.arms.iter().enumerate() {
        p.arm(arm.pivot, arm.hand, arm.ring, look::machine(Item::Arm));
        if world.focus.and_then(Focus::arm) == Some(i) {
            p.gizmos.linestrip_2d(corners(arm.pivot, HEX * 0.9), IVORY);
        }
        let stall = f.sim.arms[i].stall;
        if stall.is_some() {
            p.gizmos.circle_2d(arm.pivot, HEX * 0.5, IVORY);
        }
        if let Some(Stall::Hand(j)) = stall {
            p.gizmos.circle_2d(f.arms[j].hand, HEX * 0.65, IVORY);
        }
    }
    if let (Some(held), Some(at)) = (world.held, world.pointer) {
        let at = hex_at(at).sub(held.grip);
        p.gizmos.linestrip_2d(corners(px(at), HEX * 0.9), IVORY);
        p.machine(held.item, at, held.dir);
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

    #[derive(Resource)]
    struct Shot {
        path: PathBuf,
        clip: Option<u32>,
        wide: bool,
        keys: Vec<KeyCode>,
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
        sim.glyphs.push(glyph);
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

    fn scene(name: &str, ticks: u64) -> (World, bool, Vec<KeyCode>) {
        use KeyCode::*;
        let mut world = World::new(sim::preloaded());
        world.running = false;
        world.pointer = Some(px(Hex::new(3, -3)));
        let mut keys = Vec::new();
        let mut wide = false;
        match name {
            "micro" => world.focus_arm(0),
            "wide" => wide = true,
            "bonders" => world.sim = phased(&[(Hex::new(-3, 0), 14), (Hex::new(3, 0), 15)]),
            "focus" => {
                world
                    .sim
                    .arms
                    .push(Arm::new(Hex::new(3, -3), 0, Vec::new()));
                world.focus_arm(world.sim.arms.len() - 1);
                keys = vec![KeyF, KeyE, KeyE, KeyR, KeyQ, KeyQ];
            }
            "hold" => {
                world.lift(Held::fresh(Item::Glyph(GlyphKind::Bonder)));
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
                let (mut sim, _) = second_bond(&[]);
                sim.arms
                    .push(Arm::new(Hex::new(1, 0), 1, vec![Instr::Grab, Instr::Wait]));
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
                let (mut sim, ids) =
                    second_bond(&[Hex::new(-1, -1), Hex::new(0, -1), Hex::new(1, 0)]);
                for (a, b, kind) in [
                    (0, 1, BondKind::Single),
                    (1, 3, BondKind::Double),
                    (3, 2, BondKind::Single),
                ] {
                    sim.bonds.push(Bond {
                        a: ids[a],
                        b: ids[b],
                        kind,
                    });
                }
                world.sim = sim;
            }
            "heldtear" => {
                let (mut sim, ids) = second_bond(&[Hex::new(0, -1)]);
                sim.bonds.push(Bond {
                    a: ids[0],
                    b: ids[1],
                    kind: BondKind::Single,
                });
                let mut arm = Arm::new(Hex::new(-1, -1), 0, vec![Instr::Wait]);
                arm.holding = true;
                sim.arms.push(arm);
                world.sim = sim;
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
                        Instr::RotCcw,
                        Instr::Wait,
                    ],
                );
                eaten.holding = true;
                sim.arms.push(eaten);
                sim.arms.push(Arm::new(
                    Hex::new(1, 0),
                    3,
                    vec![Instr::Grab, Instr::RotCcw, Instr::Drop, Instr::Wait],
                ));
                world.sim = sim;
                world.focus_arm(0);
            }
            "heldout" => {
                let mut sim = Sim::empty();
                sim.glyphs.push(Glyph {
                    kind: GlyphKind::Output,
                    at: Hex::new(1, -1),
                    dir: 0,
                });
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
                world.focus_arm(0);
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
                    vec![Instr::Grab, Instr::RotCw, Instr::RotCw, Instr::RotCw],
                ));
                world.sim = sim;
                world.focus_arm(1);
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
                    vec![Instr::Grab, Instr::Wait, Instr::RotCw, Instr::Wait],
                ));
                sim.arms.push(Arm::new(
                    Hex::new(2, 0),
                    2,
                    vec![Instr::Grab, Instr::RotCw, Instr::Drop, Instr::Wait],
                ));
                world.sim = sim;
                world.focus_arm(0);
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
                world.focus_arm(usize::from(dropper_first));
            }
            other => panic!("unknown scene {other}"),
        }
        for _ in 0..ticks {
            world.sim.step();
        }
        world.prev = world.sim.clone();
        (world, wide, keys)
    }

    pub fn configure(app: &mut App) -> bool {
        const USAGE: &str = "usage: ziral --shot <png> <scene> <ticks> | ziral --shot <dir> <scene> <ticks> <play> <tick_ms> <motion>";
        let args: Vec<String> = std::env::args().collect();
        let num = |s: &String| s.parse::<f32>().expect(USAGE);
        let (path, view, ticks, clip) = match args.as_slice() {
            [_] => return false,
            [_, flag, path, view, ticks] if flag == "--shot" => (path, view, ticks, None),
            [_, flag, path, view, ticks, play, tick_ms, motion] if flag == "--shot" => (
                path,
                view,
                ticks,
                Some((num(play), num(tick_ms), num(motion))),
            ),
            _ => panic!("{USAGE}"),
        };
        let (mut world, wide, keys) = scene(view, ticks.parse().expect(USAGE));
        let clip = clip.map(|(play, tick_ms, motion)| {
            app.insert_resource(TimeUpdateStrategy::ManualDuration(FRAME));
            world.period = tick_ms / 1000.0;
            world.motion = motion;
            (play * world.period / FRAME.as_secs_f32()).round() as u32
        });
        app.insert_resource(world)
            .insert_resource(Shot {
                path: PathBuf::from(path),
                clip,
                wide,
                keys,
                frames: 0,
            })
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
        mut world: ResMut<World>,
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
        if shot.frames == WARM && shot.clip.is_some() {
            world.running = true;
            world.since = 0.0;
        }
        let n = shot.frames.wrapping_sub(WARM);
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
    use sim::{Atom, AtomKind};

    fn rotating_arm_with_atom() -> (Sim, Sim) {
        let mut prev = Sim::empty();
        prev.arms
            .push(Arm::new(Hex::new(2, 1), 2, vec![Instr::RotCw]));
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
    fn what_a_glyph_makes_appears_only_when_the_transition_ends() {
        let mut prev = Sim::empty();
        prev.glyphs.push(Glyph {
            kind: GlyphKind::Source,
            at: Hex::new(0, 0),
            dir: 0,
        });
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
        let clench = argmax(&swing);
        assert!(ring(clench) < RING_CLOSED);
        assert!((ring(clench) - RING_OPEN.lerp(RING_CLOSED, swing.at(clench))).abs() < 1e-6);
        assert_eq!(ring(1.0), RING_CLOSED);
    }

    fn argmax(swing: &Swing) -> f32 {
        (1..1000)
            .map(|i| i as f32 / 1000.0)
            .max_by(|a, b| swing.at(*a).total_cmp(&swing.at(*b)))
            .unwrap()
    }

    fn patch() -> Vec<Hex> {
        (-4..4)
            .flat_map(|q| (-4..4).map(move |r| Hex::new(q, r)))
            .collect()
    }

    fn corners() -> Vec<Swing> {
        (0..32u32)
            .map(|bits| {
                let sign = |bit: u32| if bits >> bit & 1 == 1 { 1.0 } else { -1.0 };
                Swing {
                    creep: SWING.creep + SPREAD.creep * sign(0),
                    release: SWING.release + SPREAD.release * sign(1),
                    run: SWING.run + SPREAD.run * sign(2),
                    half_bounces: if bits >> 3 & 1 == 1 {
                        SWING.half_bounces + SPREAD.half_bounces
                    } else {
                        SWING.half_bounces - SPREAD.half_bounces
                    },
                    decay: SWING.decay + SPREAD.decay * sign(4),
                }
            })
            .collect()
    }

    fn family() -> Vec<Swing> {
        let mut all = vec![SWING];
        all.extend(corners());
        all.extend(patch().into_iter().map(Swing::from_cell));
        all
    }

    #[test]
    fn a_cell_keeps_its_swing_and_a_patch_has_no_two_alike() {
        let swings: Vec<Swing> = patch().into_iter().map(Swing::from_cell).collect();
        for (cell, swing) in patch().into_iter().zip(&swings) {
            assert_eq!(Swing::from_cell(cell), *swing);
        }
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
        assert!(span(|s| s.half_bounces as f32) == 2.0 * SPREAD.half_bounces as f32);
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
            assert!(peak > 1.04 && peak < 1.16, "{swing:?} overshoot {peak}");
            let first_crossing = samples.iter().position(|s| *s >= 1.0).unwrap();
            let sign_changes = samples[first_crossing..]
                .windows(2)
                .filter(|w| (w[0] - 1.0).signum() != (w[1] - 1.0).signum())
                .count();
            assert!(sign_changes >= 2, "{swing:?} {sign_changes} sign changes");
            let bounces = samples[first_crossing..]
                .windows(3)
                .filter(|w| w[1] > w[0] && w[1] > w[2] && w[1] > 1.01)
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

    fn lone(glyphs: Vec<Glyph>, arms: Vec<Arm>) -> World {
        let mut sim = Sim::empty();
        sim.glyphs = glyphs;
        sim.arms = arms;
        World::new(sim)
    }

    fn drag(w: &mut World, from: Hex, to: Hex) {
        w.press(Vec2::ZERO, from);
        w.drag(Vec2::new(DRAG_PX * 2.0, 0.0));
        w.release(Some(to));
    }

    #[test]
    fn a_drag_from_a_non_anchor_slot_moves_the_glyph_and_keeps_that_slot_under_the_cursor() {
        let bonder = Glyph {
            kind: GlyphKind::Bonder,
            at: Hex::new(1, 1),
            dir: 2,
        };
        let mut w = lone(vec![bonder], vec![]);
        let slot = bonder.slots().nth(1).unwrap();
        assert_ne!(slot, bonder.at);
        let to = Hex::new(-4, 3);
        drag(&mut w, slot, to);
        let moved = w.sim.glyphs[0];
        assert_eq!(moved.slots().nth(1), Some(to));
        assert_eq!(moved.at, to.add(bonder.at.sub(slot)));
        assert_eq!(w.focus, Some(Focus::Glyph(0)));
    }

    #[test]
    fn a_drag_from_an_empty_cell_moves_nothing() {
        let bonder = Glyph {
            kind: GlyphKind::Bonder,
            at: ORIGIN,
            dir: 0,
        };
        let arm = Arm::new(Hex::new(4, 0), 0, vec![]);
        let mut w = lone(vec![bonder], vec![arm.clone()]);
        drag(&mut w, Hex::new(0, 3), Hex::new(-4, 3));
        assert_eq!(w.sim.glyphs, vec![bonder]);
        assert_eq!(w.sim.arms, vec![arm]);
        assert!(w.held.is_none() && w.focus.is_none());
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
        let bonder = Glyph {
            kind: GlyphKind::Bonder,
            at: Hex::new(3, -2),
            dir: 4,
        };
        let mut w = lone(vec![bonder], vec![]);
        let slot = bonder.slots().nth(1).unwrap();
        w.press(Vec2::ZERO, slot);
        w.drag(Vec2::new(DRAG_PX * 2.0, 0.0));
        w.held.as_mut().unwrap().turn(5);
        let to = Hex::new(-1, -1);
        w.release(Some(to));
        let moved = w.sim.glyphs[0];
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
        let bonder = Glyph {
            kind: GlyphKind::Bonder,
            at: ORIGIN,
            dir: 0,
        };
        let arm = Arm::new(ORIGIN, 0, vec![]);
        assert!(bonder.slots().any(|s| s == source.at));
        assert_eq!(arm.hand(), source.at);
        assert_eq!(
            lone(vec![bonder, source], vec![arm.clone()]).hit(source.at),
            Some(Focus::Glyph(1))
        );
        assert_eq!(
            lone(vec![bonder, source], vec![arm]).hit(ORIGIN),
            Some(Focus::Arm { arm: 0, cursor: 0 })
        );
        assert_eq!(
            lone(vec![source, source], vec![]).hit(source.at),
            Some(Focus::Glyph(0))
        );
    }

    #[test]
    fn a_click_on_a_body_cell_focuses_the_machine_without_moving_it() {
        let bonder = Glyph {
            kind: GlyphKind::Bonder,
            at: Hex::new(2, 2),
            dir: 1,
        };
        let mut w = lone(vec![bonder], vec![]);
        let slot = bonder.slots().nth(1).unwrap();
        w.press(Vec2::ZERO, slot);
        w.drag(Vec2::new(DRAG_PX / 2.0, 0.0));
        w.release(Some(slot));
        assert_eq!(w.focus, Some(Focus::Glyph(0)));
        assert_eq!(w.sim.glyphs, vec![bonder]);
    }

    #[test]
    fn a_palette_placement_lands_its_anchor_on_the_cursor_cell() {
        let mut w = lone(vec![], vec![]);
        w.lift(Held::fresh(Item::Arm));
        let to = Hex::new(-2, 3);
        w.release(Some(to));
        assert_eq!(w.sim.arms[0].pivot, to);
    }
}
