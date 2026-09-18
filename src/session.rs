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
}

impl Input {
    pub fn apply(&self, world: &mut World) {
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
            Self::Import(sim) => *world = World::new(*sim.clone()),
            Self::Paste(sim) => world.lift(*sim.clone(), Back::Inventory),
            Self::Refill => world.sim.inventory.fill(),
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
    pub fn new(initial: &Sim) -> Self {
        begin_record();
        let mut record = Record {
            build: persist::BUILD_TAG.to_owned(),
            seed: 0,
            inputs: Vec::new(),
        };
        if initial != &sim::start() {
            record
                .inputs
                .push((0.0, Input::Import(Box::new(initial.clone()))));
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

    pub fn send(&mut self, world: &mut World, input: Input) {
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

    pub fn paste(&mut self, world: &mut World, text: &str) {
        if !world.holding()
            && let Ok(fragment) = text.parse::<Fragment>()
        {
            self.send(world, Input::Paste(Box::new(fragment.into_sim())));
        }
    }

    pub fn pointer(&mut self, world: &mut World, pointer: Pointer, force: bool) -> bool {
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
        }
    }

    pub fn finish(&mut self) {
        self.publish();
        finish_record();
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(module = "/web/record.js")]
extern "C" {
    fn begin_record();
    fn finish_record();
    fn append_record(build: &str, seed: u64, inputs: &str);
}

#[cfg(not(target_arch = "wasm32"))]
fn begin_record() {}
#[cfg(not(target_arch = "wasm32"))]
fn finish_record() {}
#[cfg(not(target_arch = "wasm32"))]
fn append_record(_: &str, _: u64, _: &str) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_keeps_import_targets_raw_pointer_viewport_and_clock() {
        let mut initial = sim::start();
        initial.inventory.fill();
        let mut world = World::new(initial.clone());
        let mut session = Session::new(&initial);
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
        assert_eq!(record.inputs[0].1, Input::Import(Box::new(initial)));
        assert_eq!(record.inputs[1].1, Input::Pointer(pointer));
        assert_eq!(record.inputs[4].1, Input::Pointer(next));
        assert_eq!(record.inputs[5], (f64::from(0.025f32) * 2.0, key));
        assert!(!world.running);
    }
}
impl Record {
    pub fn decode(text: &str) -> Result<Self, String> {
        let mut record: Self = serde_json::from_str(text).map_err(|error| error.to_string())?;
        if record.build != persist::BUILD_TAG || record.seed != 0 {
            return Err("The record belongs to a different build or seed.".into());
        }
        let mut elapsed = 0.0;
        let mut world = World::new(sim::start());
        for (at, input) in &mut record.inputs {
            if let Input::Frame(dt) = input {
                if !dt.is_finite() || *dt < 0.0 {
                    return Err("Invalid clock.".into());
                }
                elapsed += f64::from(*dt);
            }
            if !at.is_finite() || (*at - elapsed).abs() > 0.000001 || !input.valid(&world) {
                return Err("Invalid input.".into());
            }
            *at = elapsed;
            input.apply(&mut world);
        }
        Ok(record)
    }

    pub fn duration(&self) -> f64 {
        self.inputs.last().map_or(0.0, |(at, _)| *at)
    }

    fn world_at(&self, end: usize) -> World {
        let mut world = World::new(sim::start());
        for (_, input) in &self.inputs[..end] {
            input.apply(&mut world);
        }
        world.clipboard = None;
        world.score = None;
        world
    }
}

impl Input {
    fn valid(&self, world: &World) -> bool {
        let point = |point: &Vec2| point.is_finite();
        match self {
            Self::Pointer(p) => {
                p.world.as_ref().is_none_or(point)
                    && p.screen.as_ref().is_none_or(point)
                    && p.viewport.cam.is_finite()
                    && p.viewport.size.is_finite()
                    && p.viewport.size.cmpgt(Vec2::ZERO).all()
                    && p.viewport.scale.is_finite()
                    && p.viewport.scale > 0.0
            }
            Self::Press { point: at, target } => {
                point(at)
                    && target.is_none_or(|id| match id {
                        Id::Arm(i) => i < world.shown().arms.len(),
                        Id::Glyph(i) => world.shown().glyphs.get(i).is_some_and(Option::is_some),
                        Id::Atom(i) => world.shown().atoms.get(i).is_some_and(Option::is_some),
                    })
            }
            Self::Tape { arm, cursor } => world
                .shown()
                .arms
                .get(*arm)
                .is_some_and(|a| *cursor <= a.tape.len()),
            Self::Pin(_, at, _) | Self::Card(_, at, _) | Self::MoveCard(at) => point(at),
            Self::ScaleCard(_, scale) => scale.is_finite() && *scale > 0.0,
            Self::Wheel(delta, _) => point(delta),
            Self::Import(sim) | Self::Paste(sim) => persist::encode(sim)
                .ok()
                .is_some_and(|text| persist::decode(&text).is_ok()),
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

    pub fn advance(&mut self, world: &mut World, dt: f32) {
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

    pub fn control(&mut self, world: &mut World, key: KeyCode) {
        let Mode::Replay { next, paused } = &mut self.mode else {
            return;
        };
        match key {
            KeyCode::Space => *paused = !*paused,
            KeyCode::KeyG | KeyCode::KeyS => {
                *paused = true;
                let current = *next;
                let mut probe = World::new(sim::start());
                let initial = self.record.inputs.partition_point(|(at, _)| *at == 0.0);
                let current = current.max(initial);
                let mut stops = vec![initial];
                for (index, (_, input)) in self.record.inputs.iter().enumerate() {
                    let before = probe.shown().tick;
                    input.apply(&mut probe);
                    if index + 1 > initial && probe.shown().tick != before {
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
                *world = self.record.world_at(end);
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
        .expect("--replay requires a record path")
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

pub fn open(record: Record, world: &mut World, end: bool) -> Session {
    let next = if end { record.inputs.len() } else { 0 };
    *world = record.world_at(next);
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
        initial.inventory.fill();
        let mut live = World::new(initial.clone());
        let mut recording = Session::new(&initial);
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
        assert!(live.sim.tick > 3);
        let text = serde_json::to_string(&recording.record).unwrap();
        let mut record = Record::decode(&text).unwrap();
        for (_, input) in &mut record.inputs {
            if let Input::Pointer(pointer) = input {
                pointer.viewport.size = Vec2::new(400.0, 1000.0);
                pointer.viewport.scale = 2.0;
                pointer.screen = Some(Vec2::new(300.0, 700.0));
            }
        }
        let mut replayed = World::new(sim::start());
        let mut playback = Session::replay(record);
        for bytes in frames {
            playback.advance(&mut replayed, 0.025);
            playback.advance(&mut replayed, 0.025);
            assert_eq!(format!("{replayed:?}").into_bytes(), bytes);
        }
    }

    #[test]
    fn replay_refuses_other_builds_and_invalid_clocks() {
        let mut record = Session::new(&sim::start()).record;
        record.build = "another-build".into();
        assert!(Record::decode(&serde_json::to_string(&record).unwrap()).is_err());
        record.build = persist::BUILD_TAG.into();
        record.inputs.push((0.0, Input::Frame(-0.1)));
        assert!(Record::decode(&serde_json::to_string(&record).unwrap()).is_err());
    }

    #[test]
    fn scrubbing_reconstructs_ticks_and_stays_paused() {
        let initial = fixture(Machine::Glyph(GlyphKind::Bonder)).sim;
        let mut world = World::new(initial.clone());
        let mut recording = Session::new(&world.sim);
        for _ in 0..48 {
            recording.send(&mut world, Input::Frame(0.1));
        }
        let mut playback = Session::replay(recording.record);
        let mut viewer = World::new(sim::start());
        playback.control(&mut viewer, KeyCode::KeyG);
        assert_eq!(viewer.sim.tick, 1);
        playback.control(&mut viewer, KeyCode::KeyG);
        assert_eq!(viewer.sim.tick, 2);
        playback.control(&mut viewer, KeyCode::KeyS);
        assert_eq!(viewer.sim.tick, 1);
        let before = serde_json::to_vec(&viewer.sim).unwrap();
        playback.advance(&mut viewer, 9.0);
        assert_eq!(serde_json::to_vec(&viewer.sim).unwrap(), before);
        playback.control(&mut viewer, KeyCode::KeyS);
        assert_eq!(viewer.sim, initial);
        playback.control(&mut viewer, KeyCode::Space);
        playback.advance(&mut viewer, 0.95);
        assert_eq!(viewer.sim.tick, 2);
        playback.control(&mut viewer, KeyCode::Space);
        playback.control(&mut viewer, KeyCode::KeyS);
        assert_eq!(viewer.sim.tick, 1);
        let before = format!("{viewer:?}");
        playback.send(&mut viewer, Input::Key(KeyCode::Space, false));
        assert_eq!(format!("{viewer:?}"), before);
    }
}
