use super::*;
use sim::{Atom, AtomKind, Bond, BondKind, Portal, Spin, Tier};

pub const TOKEN: &str = "bench";

const FACTORIES: [Hex; 4] = [
    Hex::new(-12, -10),
    Hex::new(-1, -10),
    Hex::new(10, -10),
    Hex::new(21, -10),
];

const CAROUSELS: [(Hex, ArmLength, [AtomKind; 2], BondKind, Spin); 6] = [
    (
        Hex::new(-23, 7),
        ArmLength::One,
        [AtomKind::Base, AtomKind::Amber],
        BondKind::Single,
        Spin::Cw,
    ),
    (
        Hex::new(-16, 7),
        ArmLength::Two,
        [AtomKind::Plum, AtomKind::Plum],
        BondKind::Double,
        Spin::Ccw,
    ),
    (
        Hex::new(-8, 7),
        ArmLength::Three,
        [AtomKind::Cobalt, AtomKind::Base],
        BondKind::Single,
        Spin::Cw,
    ),
    (
        Hex::new(1, 7),
        ArmLength::Three,
        [AtomKind::Amber, AtomKind::Cobalt],
        BondKind::Double,
        Spin::Ccw,
    ),
    (
        Hex::new(9, 7),
        ArmLength::Two,
        [AtomKind::Base, AtomKind::Base],
        BondKind::Single,
        Spin::Cw,
    ),
    (
        Hex::new(16, 7),
        ArmLength::One,
        [AtomKind::Cobalt, AtomKind::Plum],
        BondKind::Single,
        Spin::Ccw,
    ),
];

const GLYPHS: [(GlyphKind, Hex); 8] = [
    (GlyphKind::SourceTwo, Hex::new(-21, -1)),
    (GlyphKind::Bonder, Hex::new(-16, -1)),
    (GlyphKind::SecondBond, Hex::new(-12, -1)),
    (GlyphKind::Converter(AtomKind::Amber), Hex::new(-8, -1)),
    (GlyphKind::Resonator, Hex::new(-4, -1)),
    (GlyphKind::Reification, Hex::new(1, -1)),
    (GlyphKind::Output(Tier::Two), Hex::new(8, -1)),
    (GlyphKind::Output(Tier::Three), Hex::new(16, -1)),
];

const PORTALS: [Hex; 2] = [Hex::new(-16, 14), Hex::new(2, 14)];

const CARDS: [Machine; 3] = [
    Machine::Glyph(GlyphKind::Converter(AtomKind::Cobalt)),
    Machine::Arm(ArmLength::Three),
    Machine::Portal,
];

const SCENE_SHARE: f32 = 0.6;
const MARGIN: f32 = 1.1;

fn factory(sim: &mut Sim, at: Hex) {
    use Instr::{Drop, Grab, Rot};
    let (cw, ccw) = (Rot(Spin::Cw), Rot(Spin::Ccw));
    let place = |cell: Hex| at.add(cell);
    sim.glyphs.push(Some(Glyph::new(
        GlyphKind::Source,
        place(Hex::new(-1, 0)),
        0,
    )));
    sim.glyphs.push(Some(Glyph::new(
        GlyphKind::Converter(AtomKind::Cobalt),
        place(Hex::new(1, 0)),
        0,
    )));
    sim.glyphs.push(Some(Glyph::new(
        GlyphKind::Output(Tier::One),
        place(Hex::new(4, 3)),
        0,
    )));
    sim.arms.push(Arm::new(
        ArmLength::One,
        place(ORIGIN),
        3,
        vec![Grab, cw, cw, cw, Drop, ccw, ccw, ccw],
    ));
    sim.arms.push(Arm::new(
        ArmLength::Two,
        place(Hex::new(2, 3)),
        2,
        vec![Grab, ccw, ccw, Drop, cw, cw],
    ));
}

fn carousel(
    sim: &mut Sim,
    (pivot, length, kinds, kind, spin): (Hex, ArmLength, [AtomKind; 2], BondKind, Spin),
) {
    let mut arm = Arm::new(length, pivot, 0, vec![Instr::Rot(spin)]);
    let hand = arm.hand();
    let a = sim.spawn(Atom {
        kind: kinds[0],
        pos: hand,
    });
    let b = sim.spawn(Atom {
        kind: kinds[1],
        pos: hand.add(DIRS[0]),
    });
    sim.bonds.push(Bond { a, b, kind });
    arm.holding = true;
    sim.arms.push(arm);
}

pub fn world() -> Sim {
    let mut sim = Sim::empty();
    for at in FACTORIES {
        factory(&mut sim, at);
    }
    for spec in CAROUSELS {
        carousel(&mut sim, spec);
    }
    for (kind, at) in GLYPHS {
        sim.glyphs.push(Some(Glyph::new(kind, at, 0)));
    }
    for at in PORTALS {
        let mut portal = Portal::new(at);
        factory(&mut portal.sim, ORIGIN);
        sim.portals.push(Some(portal));
    }
    sim.fill_inventory();
    let grab = Item::Token(Instr::Grab);
    while sim.inventory.spend(grab) {}
    sim.inventory.set_cap(grab, sim::MAX_CAP.ilog2() as i32);
    sim
}

pub fn load(world: &mut Game, session: &mut session::Session, size: Vec2) -> (Vec2, f32) {
    let sim = self::world();
    let (lo, hi) = PortalView::extent(&sim);
    let room = Vec2::new(size.x - PALETTE_WIDTH, size.y * SCENE_SHARE);
    let scale = ((hi - lo) / room).max_element() * MARGIN;
    let centre = Vec2::new(PALETTE_WIDTH + room.x / 2.0, room.y / 2.0);
    let mut view = Viewport {
        cam: Vec2::ZERO,
        size,
        scale,
    };
    view.cam = (lo + hi) / 2.0 - view.world(centre);
    session.send(world, session::Input::Import(Box::new(sim)));
    for (k, machine) in CARDS.into_iter().enumerate() {
        let slot = (k as f32 + 0.5) / CARDS.len() as f32;
        let screen = Vec2::new(PALETTE_WIDTH + room.x * slot, (room.y + size.y) / 2.0);
        session.send(
            world,
            session::Input::Pin(
                Item::Machine(machine),
                view.world(screen),
                MouseButton::Right,
            ),
        );
        session.send(world, session::Input::EndCard(false));
    }
    (view.cam, scale)
}

#[derive(Resource)]
pub struct Bench {
    pub cam: Vec2,
    pub scale: f32,
}

const PAN_TILES: i32 = 4;
const ZOOM: f32 = 0.2;
const PAN_SECONDS: f32 = 4.0;

pub fn pan(
    bench: Option<Res<Bench>>,
    time: Res<Time>,
    camera: Single<(&mut Transform, &mut Projection), With<IsDefaultUiCamera>>,
) {
    let Some(bench) = bench else { return };
    let (mut transform, mut projection) = camera.into_inner();
    let turn = std::f32::consts::TAU * time.elapsed_secs() / PAN_SECONDS;
    transform.translation.x = bench.cam.x + px(Hex::new(PAN_TILES, 0)).x * turn.sin();
    if let Projection::Orthographic(ortho) = &mut *projection {
        ortho.scale = bench.scale * (1.0 + ZOOM * (turn / 2.0).sin());
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use bevy::app::AppExit;
    use bevy::render::render_resource::TextureFormat;
    use shot::SHOT_PX as SIZE;

    #[derive(Resource)]
    struct Run {
        path: String,
        end: f64,
        marks: Vec<f64>,
    }

    pub fn run(args: &[String]) -> Option<i32> {
        if args.get(1).is_none_or(|flag| flag != "--bench") {
            return None;
        }
        let marks: Result<Vec<f64>, _> = args.iter().skip(4).map(|s| s.parse()).collect();
        let (Some(path), Some(Ok(end)), Ok(mut marks)) =
            (args.get(2), args.get(3).map(|s| s.parse::<f64>()), marks)
        else {
            eprintln!("usage: ziral --bench <record.json> <end_seconds> <mark_seconds>...");
            return Some(2);
        };
        marks.sort_by(|a, b| b.total_cmp(a));
        let mut app = game_app(Game::new(sim::start()));
        shot::headless(&mut app, false);
        app.insert_resource(Run {
            path: path.clone(),
            end,
            marks,
        })
        .add_systems(Startup, start)
        .add_systems(Last, drive);
        lit_plugin(&mut app);
        app.run();
        Some(0)
    }

    fn start(
        mut commands: Commands,
        mut images: ResMut<Assets<Image>>,
        mut world: ResMut<Game>,
        mut session: ResMut<session::Session>,
    ) {
        let image = Image::new_target_texture(SIZE.x, SIZE.y, TextureFormat::Rgba8UnormSrgb, None);
        let (cam, scale) = load(&mut world, &mut session, SIZE.as_vec2());
        commands.spawn((
            Camera2d,
            Camera::default(),
            Projection::Orthographic(OrthographicProjection {
                scale,
                ..OrthographicProjection::default_2d()
            }),
            Transform::from_translation(cam.extend(0.0)),
            RenderTarget::Image(images.add(image).into()),
            IsDefaultUiCamera,
        ));
        commands.insert_resource(Bench { cam, scale });
    }

    fn drive(
        mut run: ResMut<Run>,
        mut world: ResMut<Game>,
        mut session: ResMut<session::Session>,
        mut exit: MessageWriter<AppExit>,
    ) {
        while run
            .marks
            .last()
            .is_some_and(|mark| session.elapsed >= *mark)
        {
            run.marks.pop();
            session.send(&mut world, session::Input::Key(KeyCode::KeyM, false));
        }
        if session.elapsed >= run.end {
            let text = serde_json::to_string(&session.record).expect("a record serializes");
            std::fs::write(&run.path, text).unwrap_or_else(|error| panic!("{}: {error}", run.path));
            exit.write(AppExit::Success);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::run;

#[cfg(test)]
mod tests {
    use super::*;
    use sim::TickEvent;

    #[test]
    fn the_bench_world_keeps_every_factory_and_carousel_moving() {
        let mut sim = world();
        let outputs: Vec<usize> = sim
            .glyphs
            .iter()
            .enumerate()
            .filter(|(_, g)| g.is_some_and(|g| g.kind == GlyphKind::Output(Tier::One)))
            .map(|(i, _)| i)
            .collect();
        assert_eq!(outputs.len(), FACTORIES.len());
        let carousels = FACTORIES.len() * 2..sim.arms.len();
        let mut crafted = vec![0; outputs.len()];
        let mut rotated = vec![0; carousels.len()];
        for _ in 0..400 {
            for event in sim.step().events {
                match event {
                    TickEvent::Fired { glyph, .. } => {
                        if let Some(i) = outputs.iter().position(|o| *o == glyph) {
                            crafted[i] += 1;
                        }
                    }
                    TickEvent::Rotated { arm, .. } if carousels.contains(&arm) => {
                        rotated[arm - carousels.start] += 1;
                    }
                    _ => {}
                }
            }
        }
        assert!(crafted.iter().all(|n| *n >= 45), "{crafted:?}");
        assert!(rotated.iter().all(|n| *n == 400), "{rotated:?}");
    }
}
