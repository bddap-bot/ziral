mod sim;

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use sim::{Arm, BondKind, Glyph, GlyphKind, Hex, Instr, Sim};

const HEX: f32 = 20.0;
const TICK_HZ: f64 = 6.0;
const MICRO_SCALE: f32 = 0.5;
const MAX_GRID_CELLS: f32 = 6000.0;

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

#[derive(Resource)]
struct World {
    sim: Sim,
    running: bool,
    selected: Option<usize>,
    cursor: usize,
    placing: Option<Item>,
}

impl World {
    fn new() -> Self {
        World {
            sim: sim::preloaded(),
            running: true,
            selected: None,
            cursor: 0,
            placing: None,
        }
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
        Instr::Grab => 'G',
        Instr::Drop => 'R',
        Instr::RotCw => 'E',
        Instr::RotCcw => 'Q',
        Instr::Wait => '.',
    }
}

fn main() {
    let mut app = App::new();
    app.insert_resource(World::new())
        .insert_resource(Time::<Fixed>::from_hz(TICK_HZ))
        .insert_resource(ClearColor(Color::srgb(0.08, 0.08, 0.08)))
        .add_systems(Startup, spawn_ui)
        .add_systems(FixedUpdate, run_ticks)
        .add_systems(Update, (view, palette, edit, draw, text));
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
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(8.0), Val::Px(2.0)),
                        border: UiRect::all(Val::Px(1.0)),
                        ..default()
                    },
                    BorderColor::all(Color::srgb(0.5, 0.5, 0.5)),
                    BackgroundColor(Color::srgb(0.12, 0.12, 0.12)),
                    children![(Text::new(name), TextFont::from_font_size(16.0))],
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
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    scroll: Res<AccumulatedMouseScroll>,
    motion: Res<AccumulatedMouseMotion>,
    time: Res<Time>,
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
    let mut pan = Vec2::ZERO;
    for (key, d) in [
        (KeyCode::KeyW, Vec2::Y),
        (KeyCode::KeyS, Vec2::NEG_Y),
        (KeyCode::KeyA, Vec2::NEG_X),
        (KeyCode::KeyD, Vec2::X),
    ] {
        if keys.pressed(key) {
            pan += d * 600.0 * time.delta_secs();
        }
    }
    if buttons.pressed(MouseButton::Middle) {
        pan += Vec2::new(-motion.delta.x, motion.delta.y);
    }
    transform.translation += (pan * ortho.scale).extend(0.0);
}

fn palette(
    mut world: ResMut<World>,
    mut buttons: Query<(&Item, &Interaction, &mut BackgroundColor), With<Button>>,
) {
    for (item, interaction, _) in &buttons {
        if *interaction == Interaction::Pressed {
            world.placing = Some(*item);
            world.selected = None;
        }
    }
    for (item, _, mut color) in &mut buttons {
        color.0 = if world.placing == Some(*item) {
            Color::srgb(0.4, 0.4, 0.4)
        } else {
            Color::srgb(0.12, 0.12, 0.12)
        };
    }
}

fn place(sim: &mut Sim, item: Item, at: Hex) -> Option<usize> {
    match item {
        Item::Arm => {
            sim.arms.push(Arm::new(at, 0, Vec::new()));
            return Some(sim.arms.len() - 1);
        }
        Item::Bonder | Item::SecondBond => sim.glyphs.push(Glyph {
            kind: if item == Item::Bonder {
                GlyphKind::Bonder
            } else {
                GlyphKind::SecondBond
            },
            at,
            dir: 0,
        }),
        Item::Source => sim.sources.push(at),
        Item::Output => sim.outputs.push(at),
    }
    None
}

fn edit(
    mut world: ResMut<World>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(&Transform, &Projection), With<Camera2d>>,
    hovered: Query<&Interaction, With<Button>>,
) {
    if keys.just_pressed(KeyCode::Space) {
        world.running = !world.running;
    }
    if keys.just_pressed(KeyCode::Period) {
        world.running = false;
        world.sim.step();
    }
    if keys.just_pressed(KeyCode::Escape) {
        world.selected = None;
        world.placing = None;
        return;
    }
    let (transform, projection) = camera.into_inner();
    let over_ui = hovered.iter().any(|i| *i != Interaction::None);
    if buttons.just_pressed(MouseButton::Left)
        && !over_ui
        && let Some(c) = window.cursor_position()
        && let Projection::Orthographic(ortho) = projection
    {
        let at = screen_to_world(
            transform.translation.truncate(),
            window.size(),
            c,
            ortho.scale,
        );
        if let Some(item) = world.placing {
            let placed = place(&mut world.sim, item, hex_at(at));
            world.placing = None;
            world.selected = placed;
            world.cursor = 0;
            return;
        }
        let hit = world
            .sim
            .arms
            .iter()
            .enumerate()
            .map(|(a, arm)| (a, px(arm.pivot).distance(at)))
            .filter(|(_, d)| *d < HEX)
            .min_by(|x, y| x.1.total_cmp(&y.1));
        world.selected = hit.map(|(a, _)| a);
        world.cursor = world.selected.map_or(0, |a| world.sim.arms[a].tape.len());
    }
    let Some(arm) = world.selected else {
        return;
    };
    let len = world.sim.arms[arm].tape.len();
    let at = world.cursor;
    if keys.just_pressed(KeyCode::ArrowLeft) {
        world.cursor = at.saturating_sub(1);
    }
    if keys.just_pressed(KeyCode::ArrowRight) {
        world.cursor = (at + 1).min(len);
    }
    let typed = [
        (KeyCode::KeyG, Instr::Grab),
        (KeyCode::KeyR, Instr::Drop),
        (KeyCode::KeyE, Instr::RotCw),
        (KeyCode::KeyQ, Instr::RotCcw),
        (KeyCode::KeyX, Instr::Wait),
    ]
    .into_iter()
    .find(|(k, _)| keys.just_pressed(*k))
    .map(|(_, i)| i);
    let remove = keys.just_pressed(KeyCode::Delete) || keys.just_pressed(KeyCode::Backspace);
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
        if world.placing.is_some()
            && let Some(c) = window.cursor_position()
        {
            let at = hex_at(screen_to_world(cam, window.size(), c, ortho.scale));
            gizmos.linestrip_2d(corners(px(at), HEX * 0.9), picked);
        }
    }
    for src in &s.sources {
        gizmos.linestrip_2d(corners(px(*src), HEX * 0.8), pad);
    }
    for out in &s.outputs {
        gizmos.linestrip_2d(corners(px(*out), HEX * 0.8), pad);
        gizmos.linestrip_2d(corners(px(*out), HEX * 0.6), pad);
    }
    for g in &s.glyphs {
        let slots = g.slots();
        let tri: Vec<Vec2> = slots.iter().chain(&slots[..1]).map(|h| px(*h)).collect();
        gizmos.linestrip_2d(tri.iter().copied(), arm_color);
        gizmos.circle_2d(tri[0], HEX * 0.12, arm_color);
        if g.kind == GlyphKind::SecondBond {
            let c = (tri[0] + tri[1] + tri[2]) / 3.0;
            gizmos.linestrip_2d(tri.iter().map(|p| c + (*p - c) * 0.6), arm_color);
        }
    }
    for b in &s.bonds {
        let (Some(a), Some(c)) = (s.atoms[b.a], s.atoms[b.b]) else {
            continue;
        };
        let (a, c) = (px(a.pos), px(c.pos));
        match b.kind {
            BondKind::Single => gizmos.line_2d(a, c, atom),
            BondKind::Double => {
                let n = (c - a).perp().normalize() * 3.0;
                gizmos.line_2d(a + n, c + n, atom);
                gizmos.line_2d(a - n, c - n, atom);
            }
        }
    }
    for (_, a) in s.live_atoms() {
        gizmos.circle_2d(px(a.pos), HEX * 0.4, atom);
    }
    for (a, arm) in s.arms.iter().enumerate() {
        let color = if world.selected == Some(a) {
            picked
        } else {
            arm_color
        };
        let pivot = px(arm.pivot);
        gizmos.circle_2d(pivot, HEX * 0.25, color);
        gizmos.line_2d(pivot, px(arm.hand()), color);
        if arm.held.is_some() {
            gizmos.circle_2d(px(arm.hand()), HEX * 0.5, color);
        }
        if arm.stalled {
            gizmos.circle_2d(pivot, HEX * 0.5, picked);
        }
    }
}

fn text(world: Res<World>, mut label: Single<&mut Text, With<Hud>>) {
    let s = &world.sim;
    let mut out = format!(
        "tick {}  {}\n",
        s.tick,
        if world.running { "running" } else { "paused" }
    );
    if let Some(item) = world.placing {
        let name = PALETTE
            .iter()
            .find(|(i, _)| *i == item)
            .map_or("", |(_, n)| n);
        out.push_str(&format!("placing {name}: click a hex, esc cancels\n"));
    }
    if let Some(a) = world.selected {
        let arm = &s.arms[a];
        out.push_str("tape ");
        let pc = if arm.tape.is_empty() {
            0
        } else {
            arm.pc % arm.tape.len()
        };
        for (k, instr) in arm.tape.iter().enumerate() {
            if k == world.cursor {
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
        if world.cursor == arm.tape.len() {
            out.push('|');
        }
        out.push('\n');
        out.push_str(
            "keys G grab  R release  E cw  Q ccw  X wait  arrows move  backspace remove  esc done\n",
        );
    }
    out.push_str("space pause/run  . step  wheel zoom  wasd/middle-drag pan  click an arm to edit its tape  palette: click, then click a hex");
    label.0 = out;
}

#[cfg(not(target_arch = "wasm32"))]
mod shot {
    use super::*;
    use bevy::app::{AppExit, ScheduleRunnerPlugin};
    use bevy::camera::RenderTarget;
    use bevy::image::Image;
    use bevy::render::render_resource::{TextureFormat, TextureUsages};
    use bevy::render::view::window::screenshot::{Screenshot, save_to_disk};
    use bevy::ui::IsDefaultUiCamera;
    use std::path::PathBuf;
    use std::time::Duration;

    const WIDE_SCALE: f32 = 1.5;

    #[derive(Resource)]
    struct Shot {
        path: PathBuf,
        wide: bool,
        frames: u32,
    }

    #[derive(Resource)]
    struct Target(Handle<Image>);

    pub fn configure(app: &mut App) -> bool {
        let args: Vec<String> = std::env::args().collect();
        let [_, flag, path, view, ticks] = args.as_slice() else {
            return false;
        };
        assert_eq!(
            flag, "--shot",
            "usage: ziral --shot <png> micro|wide <ticks>"
        );
        let ticks: u64 = ticks.parse().expect("ticks must be an integer");
        let mut world = World::new();
        world.running = false;
        for _ in 0..ticks {
            world.sim.step();
        }
        if view != "wide" {
            world.selected = Some(0);
        }
        app.insert_resource(world)
            .insert_resource(Shot {
                path: PathBuf::from(path),
                wide: view == "wide",
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
        mut exit: MessageWriter<AppExit>,
    ) {
        shot.frames += 1;
        if shot.frames == 10 {
            commands
                .spawn(Screenshot::image(target.0.clone()))
                .observe(save_to_disk(shot.path.clone()));
        }
        if shot.frames == 40 {
            exit.write(AppExit::Success);
        }
    }
}
