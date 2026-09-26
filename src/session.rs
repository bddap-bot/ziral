use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pointer {
    pub screen: Option<Vec2>,
    pub viewport: Viewport,
    pub world: Option<Vec2>,
    pub cell: Option<Hex>,
    pub panel: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Input {
    #[serde(rename = "f")]
    Frame(f32),
    Pointer(Pointer),
    Press {
        point: Vec2,
        target: Option<Id>,
    },
    Drag,
    Release(Option<Hex>),
    Inventory(Item),
    Tape {
        arm: usize,
        cursor: usize,
    },
    Key(KeyCode, bool),
    Hover(Option<Item>),
    Palette(Option<Item>),
    Cap(Item, i32),
    Pin(Item, Vec2, MouseButton),
    Card(u64, Vec2, MouseButton),
    MoveCard(Vec2),
    EndCard(bool),
    ScaleCard(u64, f32),
    Wheel(Vec2, bool),
    Import(Box<Sim>),
    Paste(Box<Sim>),
    Button(MouseButton, bool),
    KeyUp(KeyCode),
    Refill,
    Focus(Option<usize>),
    Restore(Box<persist::State>),
}

impl Input {
    pub fn apply(&self, world: &mut Game) {
        match self {
            Self::Frame(dt) => {
                world.clipboard = None;
                world.advance(*dt);
            }
            Self::Pointer(p) => {
                world.pointer = p.world;
                world.over_ui = p.panel;
            }
            Self::Press { point, target } => world.press_target(Vec2::ZERO, *point, *target),
            Self::Drag => world.begin_drag(),
            Self::Release(cell) => world.release(*cell),
            Self::Inventory(item) => world.lift_inventory(*item),
            Self::Tape { arm, cursor } => {
                world.focus_tape(*arm);
                if let Some(Focus::Tape { cursor: at, .. }) = &mut world.focus {
                    *at = *cursor;
                }
            }
            Self::Key(key, shift) => world.key(*key, *shift),
            Self::Hover(item) => world.set_hover(*item),
            Self::Palette(item) => world.palette_hover = *item,
            Self::Cap(item, n) => world.set_cap(*item, *n),
            Self::Pin(item, at, button) => world.pin_at(*item, *at, *button),
            Self::Card(id, at, button) => world.card_press(*id, *at, *button),
            Self::MoveCard(at) => world.move_card(*at),
            Self::EndCard(panel) => {
                if let Some(CardDrag::New { id, .. }) = world.card_drag.take()
                    && *panel
                {
                    world.unpin(id);
                }
            }
            Self::ScaleCard(id, scale) => {
                if let Some(card) = world.pinned.iter_mut().find(|card| card.id == *id) {
                    card.scale = *scale;
                }
            }
            Self::Import(sim) => world.restore(persist::State { sim: *sim.clone() }),
            Self::Paste(sim) => world.lift(*sim.clone(), Back::Inventory),
            Self::Focus(portal) => {
                world.enter(*portal);
            }
            Self::Restore(state) => world.restore(*state.clone()),
            Self::Refill => world.refill(),
            Self::Wheel(..) | Self::Button(..) | Self::KeyUp(_) => {}
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Record {
    pub build: String,
    pub seed: u64,
    pub inputs: Vec<(f64, Input)>,
}

#[derive(Resource)]
pub struct Session {
    pub record: Record,
    pub elapsed: f64,
    sampled: f64,
    pointer: Option<Pointer>,
    mode: Mode,
    sent: usize,
}

impl Session {
    pub fn new(initial: &persist::State) -> Self {
        begin_record();
        let mut record = Record {
            build: persist::BUILD_TAG.to_owned(),
            seed: 0,
            inputs: Vec::new(),
        };
        if initial != &Game::new(sim::start()).state() {
            record
                .inputs
                .push((0.0, Input::Restore(Box::new(initial.clone()))));
        }
        Self {
            record,
            elapsed: 0.0,
            sampled: -1.0,
            pointer: None,
            mode: Mode::Recording,
            sent: 0,
        }
    }

    pub fn send(&mut self, world: &mut Game, input: Input) {
        if self.replaying() {
            return;
        }
        if let Input::Frame(dt) = input {
            self.elapsed += f64::from(dt);
        }
        if matches!(input, Input::Import(_)) {
            self.pointer = None;
        }
        input.apply(world);
        self.record.inputs.push((self.elapsed, input));
    }

    pub fn paste(&mut self, world: &mut Game, text: &str) {
        if !world.holding()
            && let Ok(fragment) = text.parse::<Fragment>()
        {
            self.send(world, Input::Paste(Box::new(fragment.into_sim())));
        }
    }

    pub fn pointer(&mut self, world: &mut Game, pointer: Pointer, force: bool) -> bool {
        let resized = self
            .pointer
            .as_ref()
            .is_none_or(|p| p.viewport != pointer.viewport);
        if force || resized || self.elapsed - self.sampled >= 0.05 {
            self.sampled = self.elapsed;
            if force || self.pointer.as_ref() != Some(&pointer) {
                self.pointer = Some(pointer.clone());
                self.send(world, Input::Pointer(pointer));
            }
            true
        } else {
            false
        }
    }
}

pub fn flush(mut session: ResMut<Session>) {
    session.publish();
}

impl Session {
    pub fn publish(&mut self) {
        if self.replaying() || self.sent == self.record.inputs.len() {
            return;
        }
        if let Ok(text) = serde_json::to_string(&self.record.inputs[self.sent..]) {
            append_record(&self.record.build, self.record.seed, &text);
            self.sent = self.record.inputs.len();
            #[cfg(target_arch = "wasm32")]
            {
                self.record.inputs.clear();
                self.sent = 0;
            }
        }
    }

    pub fn finish(&mut self) {
        self.publish();
        finish_record();
    }

    pub fn bench(&mut self) {
        self.publish();
        bench_record();
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(module = "/web/record.js")]
extern "C" {
    fn begin_record();
    fn finish_record();
    fn bench_record();
    fn append_record(build: &str, seed: u64, inputs: &str);
}

#[cfg(not(target_arch = "wasm32"))]
fn begin_record() {}
#[cfg(not(target_arch = "wasm32"))]
fn finish_record() {}
#[cfg(not(target_arch = "wasm32"))]
fn bench_record() {}
#[cfg(not(target_arch = "wasm32"))]
fn append_record(_: &str, _: u64, _: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_keeps_import_targets_raw_pointer_viewport_and_clock() {
        let mut initial = sim::start();
        initial.fill_inventory();
        let mut world = Game::new(initial.clone());
        let mut session = Session::new(&Game::new(initial.clone()).state());
        let pointer = Pointer {
            screen: Some(Vec2::new(23.0, 51.0)),
            viewport: Viewport {
                cam: Vec2::new(3.0, 5.0),
                size: Vec2::new(900.0, 600.0),
                scale: 0.5,
            },
            world: Some(px(Hex::new(4, 7))),
            cell: Some(Hex::new(4, 7)),
            panel: false,
        };
        session.pointer(&mut world, pointer.clone(), true);
        session.send(&mut world, Input::Frame(0.025));
        let mut next = pointer.clone();
        next.screen = Some(Vec2::new(24.0, 51.0));
        assert!(!session.pointer(&mut world, next.clone(), false));
        session.send(&mut world, Input::Frame(0.025));
        assert!(session.pointer(&mut world, next.clone(), false));
        let key = Input::Key(KeyCode::Space, false);
        session.send(&mut world, key.clone());
        let record: Record =
            serde_json::from_str(&serde_json::to_string(&session.record).unwrap()).unwrap();
        assert_eq!(record.build, persist::BUILD_TAG);
        assert_eq!(record.seed, 0);
        assert_eq!(
            record.inputs[0].1,
            Input::Restore(Box::new(Game::new(initial).state()))
        );
        assert_eq!(record.inputs[1].1, Input::Pointer(pointer));
        assert_eq!(record.inputs[4].1, Input::Pointer(next));
        assert_eq!(record.inputs[5], (f64::from(0.025f32) * 2.0, key));
        assert!(world.running);
    }
}
impl Record {
    pub fn decode(text: &str) -> Result<Self, String> {
        let mut record: Self = serde_json::from_str(text).map_err(|error| error.to_string())?;
        if record.build != persist::BUILD_TAG || record.seed != 0 {
            return Err("The record belongs to a different build or seed.".into());
        }
        let mut elapsed = 0.0;
        for (at, input) in &mut record.inputs {
            if let Input::Frame(dt) = input {
                if !dt.is_finite() || *dt < 0.0 {
                    return Err("Invalid clock.".into());
                }
                elapsed += f64::from(*dt);
            }
            if !at.is_finite() || (*at - elapsed).abs() > 0.000001 || !input.valid() {
                return Err("Invalid input.".into());
            }
            *at = elapsed;
        }
        let mut world = Game::new(sim::start());
        for (_, input) in &record.inputs {
            if !input.valid_target(&world) {
                return Err("Invalid target.".into());
            }
            input.apply(&mut world);
        }
        Ok(record)
    }

    pub fn duration(&self) -> f64 {
        self.inputs.last().map_or(0.0, |(at, _)| *at)
    }

    fn world_at(&self, end: usize) -> Game {
        let mut world = Game::new(sim::start());
        for (_, input) in &self.inputs[..end] {
            input.apply(&mut world);
        }
        world.clipboard = None;
        world.score = None;
        world
    }
}

impl Input {
    fn valid(&self) -> bool {
        let point = |point: &Vec2| point.is_finite() && point.abs().max_element() <= 1_000_000.0;
        fn cell(cell: Hex) -> bool {
            cell.q.unsigned_abs() <= 1_000_000 && cell.r.unsigned_abs() <= 1_000_000
        }
        fn coordinates(sim: &Sim) -> bool {
            sim.arms.iter().all(|arm| cell(arm.pivot))
                && sim.glyphs.iter().flatten().all(|glyph| cell(glyph.at))
                && sim.atoms.iter().flatten().all(|atom| cell(atom.pos))
                && sim
                    .portals
                    .iter()
                    .flatten()
                    .all(|portal| cell(portal.at) && coordinates(&portal.sim))
        }
        match self {
            Self::Pointer(p) => {
                p.world.as_ref().is_none_or(point)
                    && p.screen.as_ref().is_none_or(point)
                    && p.cell.is_none_or(cell)
                    && point(&p.viewport.cam)
                    && point(&p.viewport.size)
                    && p.viewport.size.cmpgt(Vec2::ZERO).all()
                    && p.viewport.scale.is_finite()
                    && p.viewport.scale > 0.0
            }
            Self::Press { point: at, .. } => point(at),
            Self::Release(at) => at.is_none_or(cell),
            Self::Pin(_, at, _) | Self::Card(_, at, _) | Self::MoveCard(at) => point(at),
            Self::ScaleCard(_, scale) => scale.is_finite() && *scale > 0.0,
            Self::Wheel(delta, _) => point(delta),
            Self::Import(sim) | Self::Paste(sim) => {
                coordinates(sim)
                    && persist::encode(sim)
                        .ok()
                        .is_some_and(|text| persist::decode(&text).is_ok())
            }
            Self::Restore(state) => {
                coordinates(&state.sim)
                    && persist::encode_state(state)
                        .ok()
                        .is_some_and(|text| persist::decode_state(&text).is_ok())
            }
            _ => true,
        }
    }

    fn valid_target(&self, world: &Game) -> bool {
        match self {
            Self::Press { target, .. } => target.is_none_or(|id| match id {
                Id::Portal(i) => world.shown().portals.get(i).is_some_and(Option::is_some),
                Id::Arm(i) => i < world.shown().arms.len(),
                Id::Glyph(i) => world.shown().glyphs.get(i).is_some_and(Option::is_some),
                Id::Atom(i) => world.shown().atoms.get(i).is_some_and(Option::is_some),
            }),
            Self::Tape { arm, cursor } => world
                .shown()
                .arms
                .get(*arm)
                .is_some_and(|a| *cursor <= a.tape.len()),
            _ => true,
        }
    }
}

#[derive(Default)]
enum Mode {
    #[default]
    Recording,
    Replay {
        next: usize,
        paused: bool,
    },
}

impl Session {
    pub fn replay(record: Record) -> Self {
        Self {
            record,
            elapsed: 0.0,
            sampled: -1.0,
            pointer: None,
            mode: Mode::Replay {
                next: 0,
                paused: false,
            },
            sent: 0,
        }
    }

    pub fn replaying(&self) -> bool {
        matches!(self.mode, Mode::Replay { .. })
    }

    pub fn advance(&mut self, world: &mut Game, dt: f32) {
        let Mode::Replay { next, paused } = &mut self.mode else {
            self.send(world, Input::Frame(dt));
            return;
        };
        if *paused {
            return;
        }
        self.elapsed = (self.elapsed + f64::from(dt)).min(self.record.duration());
        while let Some((at, input)) = self.record.inputs.get(*next) {
            if *at > self.elapsed {
                break;
            }
            input.apply(world);
            *next += 1;
        }
    }

    pub fn control(&mut self, world: &mut Game, key: KeyCode) {
        let Mode::Replay { next, paused } = &mut self.mode else {
            return;
        };
        match key {
            KeyCode::Space => *paused = !*paused,
            KeyCode::KeyG | KeyCode::KeyS => {
                *paused = true;
                let current = *next;
                let mut probe = Game::new(sim::start());
                let initial = self.record.inputs.partition_point(|(at, _)| *at == 0.0);
                let current = current.max(initial);
                let mut stops = vec![initial];
                for (index, (_, input)) in self.record.inputs.iter().enumerate() {
                    let before = (probe.overworld.sim.tick, probe.shown().tick);
                    input.apply(&mut probe);
                    if index + 1 > initial
                        && !matches!(
                            input,
                            Input::Focus(_) | Input::Import(_) | Input::Restore(_)
                        )
                        && (probe.overworld.sim.tick, probe.shown().tick) != before
                    {
                        stops.push(index + 1);
                    }
                }
                let current_tick_start = stops
                    .iter()
                    .rev()
                    .find(|stop| **stop <= current)
                    .copied()
                    .unwrap_or(initial);
                let end = if key == KeyCode::KeyG {
                    stops
                        .into_iter()
                        .find(|stop| *stop > current)
                        .unwrap_or(self.record.inputs.len())
                } else {
                    stops
                        .into_iter()
                        .rev()
                        .find(|stop| *stop < current_tick_start)
                        .unwrap_or(initial)
                };
                world.replace_with(self.record.world_at(end));
                self.elapsed = end.checked_sub(1).map_or(0.0, |i| self.record.inputs[i].0);
                *next = end;
            }
            _ => {}
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn argument(args: &mut Vec<String>) -> Option<Record> {
    let at = args.iter().position(|arg| arg == "--replay")?;
    let path = args
        .get(at + 1)
        .unwrap_or_else(|| {
            eprintln!("usage: ziral --replay <record.json>");
            std::process::exit(2);
        })
        .clone();
    args.drain(at..=at + 1);
    Some(
        std::fs::read_to_string(path)
            .map_err(|error| error.to_string())
            .and_then(|text| Record::decode(&text))
            .unwrap_or_else(|error| {
                eprintln!("{error}");
                std::process::exit(1);
            }),
    )
}

pub fn open(record: Record, world: &mut Game, end: bool) -> Session {
    let next = if end { record.inputs.len() } else { 0 };
    world.replace_with(record.world_at(next));
    let mut session = Session::replay(record);
    if end {
        session.elapsed = session.record.duration();
        session.mode = Mode::Replay { next, paused: true };
    }
    session
}

#[cfg(test)]
mod replay_tests {
    use super::*;

    #[test]
    fn recorded_actions_reproduce_world_bytes_at_every_tick() {
        let mut initial = fixture(Machine::Glyph(GlyphKind::Bonder)).sim;
        initial.fill_inventory();
        let mut live = Game::new(initial.clone());
        let mut recording = Session::new(&Game::new(initial.clone()).state());
        let mut frames = Vec::new();
        for frame in 0..80 {
            recording.send(&mut live, Input::Frame(0.05));
            let action = match frame {
                8 | 13 | 25 | 27 => Some(Input::Key(KeyCode::Space, false)),
                9 => Some(Input::Tape { arm: 0, cursor: 0 }),
                10 | 11 => Some(Input::Key(KeyCode::KeyG, false)),
                12 => Some(Input::Key(KeyCode::KeyS, false)),
                15 => Some(Input::Pointer(Pointer {
                    screen: Some(Vec2::new(52.0, 92.0)),
                    viewport: Viewport {
                        cam: Vec2::new(1.0, 2.0),
                        size: Vec2::new(900.0, 600.0),
                        scale: 0.5,
                    },
                    world: Some(px(Hex::new(10, 10))),
                    cell: Some(Hex::new(10, 10)),
                    panel: false,
                })),
                16 => Some(Input::Inventory(Item::Machine(Machine::Arm(
                    sim::ArmLength::One,
                )))),
                17 => Some(Input::Release(Some(Hex::new(10, 10)))),
                26 => Some(Input::Key(KeyCode::KeyX, false)),
                45 => Some(Input::Import(Box::new(initial.clone()))),
                _ => None,
            };
            if let Some(action) = action {
                recording.send(&mut live, action);
            }
            frames.push(format!("{live:?}").into_bytes());
        }
        assert!(live.sim().tick > 3);
        let text = serde_json::to_string(&recording.record).unwrap();
        let mut record = Record::decode(&text).unwrap();
        for (_, input) in &mut record.inputs {
            if let Input::Pointer(pointer) = input {
                pointer.viewport.size = Vec2::new(400.0, 1000.0);
                pointer.viewport.scale = 2.0;
                pointer.screen = Some(Vec2::new(300.0, 700.0));
            }
        }
        let mut replayed = Game::new(sim::start());
        let mut playback = Session::replay(record);
        for bytes in frames {
            playback.advance(&mut replayed, 0.025);
            playback.advance(&mut replayed, 0.025);
            assert_eq!(format!("{replayed:?}").into_bytes(), bytes);
        }
    }

    #[test]
    fn decode_checks_all_inputs_before_replay_and_rejects_extreme_coordinates() {
        let mut record = Session::new(&Game::new(sim::start()).state()).record;
        for value in [f32::MAX, 1e30, 1_000_001.0, -1_000_001.0, -1e30, f32::MIN] {
            for point in [Vec2::new(value, 0.0), Vec2::new(0.0, value)] {
                let mut initial = Sim::empty();
                initial.arms.push(Arm::new(
                    sim::ArmLength::One,
                    Hex::new(-10, -10),
                    0,
                    Vec::new(),
                ));
                record.inputs = vec![
                    (
                        0.0,
                        Input::Restore(Box::new(persist::State { sim: initial })),
                    ),
                    (0.0, Input::Key(KeyCode::Space, false)),
                    (
                        0.0,
                        Input::Press {
                            point,
                            target: Some(Id::Arm(0)),
                        },
                    ),
                    (0.0, Input::Drag),
                ];
                assert_eq!(
                    Record::decode(&serde_json::to_string(&record).unwrap()).unwrap_err(),
                    "Invalid input."
                );
                record.inputs = vec![
                    (
                        0.0,
                        Input::Tape {
                            arm: usize::MAX,
                            cursor: 0,
                        },
                    ),
                    (
                        0.0,
                        Input::Press {
                            point,
                            target: None,
                        },
                    ),
                    (0.0, Input::Drag),
                ];
                assert_eq!(
                    Record::decode(&serde_json::to_string(&record).unwrap()).unwrap_err(),
                    "Invalid input."
                );
            }
        }
        for value in [i32::MIN, i32::MAX] {
            record.inputs = vec![(0.0, Input::Release(Some(Hex::new(value, 0))))];
            assert!(Record::decode(&serde_json::to_string(&record).unwrap()).is_err());
        }
        record.inputs = vec![
            (
                0.0,
                Input::Press {
                    point: Vec2::new(1000.0, -1000.0),
                    target: None,
                },
            ),
            (0.0, Input::Drag),
        ];
        assert!(Record::decode(&serde_json::to_string(&record).unwrap()).is_ok());
    }

    #[test]
    fn replay_refuses_other_builds_and_invalid_clocks() {
        let mut record = Session::new(&Game::new(sim::start()).state()).record;
        record.build = "another-build".into();
        assert!(Record::decode(&serde_json::to_string(&record).unwrap()).is_err());
        record.build = persist::BUILD_TAG.into();
        record.inputs.push((0.0, Input::Frame(-0.1)));
        assert!(Record::decode(&serde_json::to_string(&record).unwrap()).is_err());
    }

    #[test]
    fn scrubbing_reconstructs_ticks_and_stays_paused() {
        let initial = fixture(Machine::Glyph(GlyphKind::Bonder)).sim;
        let mut world = Game::new(initial.clone());
        let mut recording = Session::new(&world.state());
        for _ in 0..48 {
            recording.send(&mut world, Input::Frame(0.1));
        }
        let mut playback = Session::replay(recording.record);
        let mut viewer = Game::new(sim::start());
        playback.control(&mut viewer, KeyCode::KeyG);
        assert_eq!(viewer.sim().tick, 1);
        playback.control(&mut viewer, KeyCode::KeyG);
        assert_eq!(viewer.sim().tick, 2);
        playback.control(&mut viewer, KeyCode::KeyS);
        assert_eq!(viewer.sim().tick, 1);
        let before = serde_json::to_vec(viewer.sim()).unwrap();
        playback.advance(&mut viewer, 9.0);
        assert_eq!(serde_json::to_vec(viewer.sim()).unwrap(), before);
        playback.control(&mut viewer, KeyCode::KeyS);
        assert_eq!(viewer.sim(), Game::new(initial).sim());
        playback.control(&mut viewer, KeyCode::Space);
        playback.advance(&mut viewer, 0.95);
        assert_eq!(viewer.sim().tick, 2);
        playback.control(&mut viewer, KeyCode::Space);
        playback.control(&mut viewer, KeyCode::KeyS);
        assert_eq!(viewer.sim().tick, 1);
        let before = format!("{viewer:?}");
        playback.send(&mut viewer, Input::Key(KeyCode::Space, false));
        assert_eq!(format!("{viewer:?}"), before);
    }
}
