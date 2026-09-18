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
            Self::Frame(dt) => world.advance(*dt),
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
}

impl Session {
    pub fn new(initial: &Sim) -> Self {
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
        }
    }

    pub fn send(&mut self, world: &mut World, input: Input) {
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

pub fn flush(session: Res<Session>, mut last: Local<usize>) {
    if *last == session.record.inputs.len() {
        return;
    }
    if let Ok(text) = serde_json::to_string(&session.record.inputs[*last..]) {
        append_record(&session.record.build, session.record.seed, &text);
        *last = session.record.inputs.len();
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(module = "/web/record.js")]
extern "C" {
    fn append_record(build: &str, seed: u64, inputs: &str);
}

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
