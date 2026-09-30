use crate::sim::{Machine, TickEvent, TickEvents};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

const RATE: u32 = 48_000;

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use super::*;
    use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};

    pub type Clip = Handle<AudioSource>;

    pub fn load(mut commands: Commands, mut assets: ResMut<Assets<AudioSource>>) {
        commands.insert_resource(bank(|samples| {
            assets.add(AudioSource {
                bytes: wav_bytes(samples, RATE).into(),
            })
        }));
    }

    pub fn emit(commands: &mut Commands, clip: &Clip, gain: f32, speed: f32) {
        commands.spawn((
            AudioPlayer::new(clip.clone()),
            PlaybackSettings {
                volume: Volume::Linear(gain),
                speed,
                ..PlaybackSettings::DESPAWN
            },
        ));
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;

    #[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
const mixer = `registerProcessor("mixer", class extends AudioWorkletProcessor {
    clips = [];
    voices = [];
    constructor() {
        super();
        this.port.onmessage = ({ data }) => {
            if (data.samples) this.clips[data.clip] = data.samples;
            else if (this.clips[data[0]]) this.voices.push({ samples: this.clips[data[0]], gain: data[1], speed: data[2], at: 0 });
        };
    }
    process(_, [[out]]) {
        for (let v = this.voices.length - 1; v >= 0; v--) {
            const voice = this.voices[v];
            const end = Math.min(out.length, Math.ceil((voice.samples.length - voice.at) / voice.speed));
            for (let i = 0; i < end; i++) {
                const at = voice.at + i * voice.speed;
                const whole = Math.floor(at);
                const a = voice.samples[whole];
                const b = voice.samples[whole + 1] ?? 0;
                out[i] += voice.gain * (a + (b - a) * (at - whole));
            }
            voice.at += end * voice.speed;
            if (voice.at >= voice.samples.length) this.voices[v] = this.voices[this.voices.length - 1], this.voices.pop();
        }
        return true;
    }
});`;
const inputEvents = ["pointerdown", "pointerup", "keydown", "touchend"];
let context;
let port;
let ready;
let clips = 0;
function resume() {
    context.resume().then(() => {
        if (context.state === "running")
            for (const type of inputEvents)
                globalThis.removeEventListener(type, resume, { capture: true });
    });
}
function open(rate) {
    context = new AudioContext({ sampleRate: rate });
    for (const type of inputEvents)
        globalThis.addEventListener(type, resume, { capture: true });
    const url = URL.createObjectURL(new Blob([mixer], { type: "text/javascript" }));
    ready = context.audioWorklet.addModule(url).then(() => {
        URL.revokeObjectURL(url);
        const node = new AudioWorkletNode(context, "mixer", { numberOfInputs: 0, outputChannelCount: [1] });
        node.connect(context.destination);
        return port = node.port;
    });
}
export function add_clip(samples, rate) {
    if (!context) open(rate);
    const clip = clips++;
    const copy = samples.slice();
    ready.then(port => port.postMessage({ clip, samples: copy }, [copy.buffer]));
    return clip;
}
export function play_clip(clip, gain, speed) {
    if (context.state === "running") port?.postMessage([clip, gain, speed]);
}
"#)]
    extern "C" {
        fn add_clip(samples: &[f32], rate: u32) -> u32;
        fn play_clip(clip: u32, gain: f32, speed: f32);
    }

    pub type Clip = u32;

    pub fn load(mut commands: Commands) {
        commands.insert_resource(bank(|samples| {
            let samples: Vec<f32> = samples
                .iter()
                .map(|sample| f32::from(*sample) / 32_768.0)
                .collect();
            add_clip(&samples, RATE)
        }));
    }

    pub fn emit(_: &mut Commands, clip: &Clip, gain: f32, speed: f32) {
        play_clip(*clip, gain, speed);
    }
}

#[cfg(not(target_arch = "wasm32"))]
use native as out;
#[cfg(target_arch = "wasm32")]
use web as out;

pub use out::load;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Voice {
    Brass,
    Ceramic,
    Wood,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Instrument {
    voice: Voice,
    note: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub machine: Machine,
    pub instrument: Instrument,
    pub upgrade: bool,
    pub at: crate::sim::Hex,
}

#[derive(Clone, Copy)]
pub struct View {
    pub center: Vec2,
    pub half: Vec2,
    pub scale: f32,
}

pub fn gain(cell: crate::sim::Hex, view: View) -> Option<f32> {
    let inside = (crate::look::px(cell) - view.center)
        .abs()
        .cmple(view.half)
        .all();
    inside.then(|| 1.0 / (1.0 + view.scale))
}

#[derive(Deserialize)]
struct Entry {
    instrument: Instrument,
    #[serde(default)]
    actions: ActionSounds,
}

#[derive(Deserialize)]
struct Manifest {
    machine: BTreeMap<String, Entry>,
}

fn manifest() -> &'static Manifest {
    static MANIFEST: OnceLock<Manifest> = OnceLock::new();
    MANIFEST.get_or_init(|| {
        toml::from_str(include_str!("../art/machines/manifest.toml"))
            .unwrap_or_else(|error| panic!("art/machines/manifest.toml: {error}"))
    })
}

pub fn instrument(machine: Machine) -> Instrument {
    let name = crate::look::machine(machine)
        .skin
        .name
        .split('/')
        .nth(1)
        .expect("a machine skin lives in art/machines/<name>/");
    manifest()
        .machine
        .get(name)
        .unwrap_or_else(|| panic!("machine {name} has no instrument"))
        .instrument
}

pub fn score(tick: &TickEvents) -> Vec<Hit> {
    tick.events
        .iter()
        .filter_map(|event| {
            let (machine, at) = match event {
                TickEvent::Copied { at, .. } => (Machine::Portal, *at),
                TickEvent::Fired { machine, at, .. } => (*machine, *at),
                TickEvent::Upgraded { at, .. } => {
                    (Machine::Glyph(crate::sim::GlyphKind::SourceTwo), *at)
                }
                TickEvent::Grabbed { length, at, .. }
                | TickEvent::Dropped { length, at, .. }
                | TickEvent::Rotated { length, at, .. }
                | TickEvent::Pivoted { length, at, .. } => (Machine::Arm(*length), *at),
                TickEvent::Moved { length, to, .. } => (Machine::Arm(*length), *to),
                TickEvent::BondWritten { .. }
                | TickEvent::Consumed { .. }
                | TickEvent::Spawned { .. }
                | TickEvent::Stalled { .. } => return None,
            };
            Some(Hit {
                machine,
                instrument: instrument(machine),
                upgrade: matches!(event, TickEvent::Upgraded { .. }),
                at,
            })
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Pickup,
    Drop,
    Rotate,
    Delete,
    Edit,
}

impl Action {
    const ALL: [Self; 5] = [
        Self::Pickup,
        Self::Drop,
        Self::Rotate,
        Self::Delete,
        Self::Edit,
    ];
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct ActionSounds {
    pickup: Option<Instrument>,
    drop: Option<Instrument>,
    rotate: Option<Instrument>,
    delete: Option<Instrument>,
    edit: Option<Instrument>,
}

impl ActionSounds {
    fn get(&self, action: Action) -> Option<Instrument> {
        match action {
            Action::Pickup => self.pickup,
            Action::Drop => self.drop,
            Action::Rotate => self.rotate,
            Action::Delete => self.delete,
            Action::Edit => self.edit,
        }
    }
}

fn action_instrument(machine: Option<Machine>, action: Action) -> Instrument {
    machine
        .and_then(|machine| {
            let name = crate::look::machine(machine)
                .skin
                .name
                .split('/')
                .nth(1)
                .unwrap();
            manifest().machine[name].actions.get(action)
        })
        .unwrap_or(Instrument {
            voice: Voice::Ceramic,
            note: [62, 50, 57, 43, 67][action as usize],
        })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ActionCue {
    action: Action,
    machine: Option<Machine>,
    count: usize,
    kinds: usize,
}

impl ActionCue {
    pub fn picked(
        action: Action,
        sim: &crate::sim::Sim,
        ids: impl IntoIterator<Item = crate::sim::Id>,
    ) -> Self {
        use crate::sim::{Id, Item};
        Self::new(
            action,
            ids.into_iter().map(|id| match id {
                Id::Portal(_) => Item::Machine(Machine::Portal),
                Id::Arm(i) => Item::Machine(sim.arms[i].machine()),
                Id::Glyph(i) => Item::Machine(Machine::Glyph(sim.glyphs[i].unwrap().kind)),
                Id::Atom(i) => Item::Atom(sim.atoms[i].unwrap().kind),
            }),
        )
    }

    pub fn of(action: Action, sim: &crate::sim::Sim) -> Self {
        Self::picked(action, sim, sim.ids())
    }

    pub fn new(action: Action, items: impl IntoIterator<Item = crate::sim::Item>) -> Self {
        let mut counts = [0usize; Machine::ALL.len()];
        let mut count = 0;
        for item in items {
            match item {
                crate::sim::Item::Machine(machine) => {
                    counts[Machine::ALL
                        .iter()
                        .position(|kind| *kind == machine)
                        .unwrap()] += 1;
                    count += 1;
                }
                crate::sim::Item::Atom(_) => count += 1,
                crate::sim::Item::Token(_) => {}
            }
        }
        let machine = counts
            .iter()
            .enumerate()
            .max_by_key(|(_, n)| **n)
            .filter(|(_, n)| **n > 0)
            .map(|(i, _)| Machine::ALL[i]);
        Self {
            action,
            machine,
            count: count.max(1),
            kinds: counts.iter().filter(|n| **n > 0).count(),
        }
    }

    fn settings(self, serial: u32, gain: f32) -> (f32, f32) {
        let variation = (serial as f32 * 0.618_034).fract();
        let weight = (self.count as f32).log2().min(6.0);
        (
            (0.09 + variation * 0.025) * gain,
            0.97 + variation * 0.06 - weight * 0.035 + self.kinds as f32 * 0.004,
        )
    }
}

fn action_samples(instrument: Instrument) -> Vec<i16> {
    samples(instrument)
        .into_iter()
        .take(4_320)
        .enumerate()
        .map(|(i, sample)| {
            let attack = (i as f32 / 240.0).min(1.0);
            let release = ((4_320 - i) as f32 / 1_440.0).min(1.0);
            (sample as f32 * attack * release) as i16
        })
        .collect()
}

pub fn play_actions(commands: &mut Commands, bank: &mut Bank, cues: &[ActionCue], gain: f32) {
    for cue in cues.iter().take(8) {
        bank.serial = bank.serial.wrapping_add(1) % 65_536;
        let kind = cue
            .machine
            .and_then(|machine| Machine::ALL.iter().position(|kind| *kind == machine))
            .unwrap_or(Machine::ALL.len());
        let (gain, speed) = cue.settings(bank.serial, gain);
        out::emit(
            commands,
            &bank.actions[kind][cue.action as usize],
            gain,
            speed,
        );
    }
}

#[derive(Resource)]
pub struct Bank {
    actions: Vec<[out::Clip; 5]>,
    serial: u32,
    voices: Vec<(Machine, out::Clip)>,
    upgrade: out::Clip,
    pub unlocked: bool,
}

fn bank(mut clip: impl FnMut(&[i16]) -> out::Clip) -> Bank {
    Bank {
        actions: Machine::ALL
            .into_iter()
            .map(Some)
            .chain([None])
            .map(|machine| {
                Action::ALL.map(|action| clip(&action_samples(action_instrument(machine, action))))
            })
            .collect(),
        serial: 0,
        voices: Machine::ALL
            .into_iter()
            .map(|machine| (machine, clip(&samples(instrument(machine)))))
            .collect(),
        upgrade: clip(&upgrade_samples()),
        unlocked: false,
    }
}

fn activates(key: KeyCode) -> bool {
    use KeyCode::*;
    !matches!(
        key,
        ShiftLeft
            | ShiftRight
            | ControlLeft
            | ControlRight
            | AltLeft
            | AltRight
            | SuperLeft
            | SuperRight
            | Meta
            | Hyper
            | Fn
            | FnLock
            | CapsLock
            | NumLock
            | ScrollLock
            | Escape
    )
}

pub fn unlock(
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    bank: Option<ResMut<Bank>>,
) {
    let Some(mut bank) = bank else { return };
    let pressed = buttons.get_just_pressed().next().is_some()
        || keys.get_just_pressed().any(|key| activates(*key))
        || touches.any_just_released();
    if pressed {
        bank.unlocked = true;
    }
}

pub fn play(commands: &mut Commands, bank: &Bank, hits: &[Hit], view: View) {
    for hit in hits {
        let Some(gain) = gain(hit.at, view) else {
            continue;
        };
        let clip = if hit.upgrade {
            &bank.upgrade
        } else {
            &bank
                .voices
                .iter()
                .find(|(machine, _)| *machine == hit.machine)
                .expect("every machine has a loaded instrument")
                .1
        };
        out::emit(commands, clip, gain, 1.0);
    }
}

fn upgrade_samples() -> Vec<i16> {
    let tones = [48, 55, 60, 64, 67].map(|note| {
        samples(Instrument {
            voice: Voice::Brass,
            note,
        })
    });
    (0..tones[0].len())
        .map(|i| {
            (tones.iter().map(|tone| i32::from(tone[i])).sum::<i32>() / 3)
                .clamp(i16::MIN as i32, i16::MAX as i32) as i16
        })
        .collect()
}

fn samples(instrument: Instrument) -> Vec<i16> {
    let rate = RATE;
    let count = rate * 3 / 20;
    let frequency = 440.0 * 2.0_f32.powf((instrument.note as f32 - 69.0) / 12.0);
    (0..count)
        .map(|i| {
            let t = i as f32 / rate as f32;
            let phase = std::f32::consts::TAU * frequency * t;
            let wave = match instrument.voice {
                Voice::Brass => phase.sin() + 0.35 * (phase * 2.0).sin(),
                Voice::Ceramic => phase.sin() + 0.55 * (phase * 2.7).sin(),
                Voice::Wood => phase.sin().signum() * 0.7 + phase.sin() * 0.3,
            };
            let envelope = (-28.0 * t).exp();
            (wave * envelope * 8_000.0) as i16
        })
        .collect()
}

#[cfg(not(target_arch = "wasm32"))]
fn wav_bytes(samples: &[i16], rate: u32) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

#[cfg(not(target_arch = "wasm32"))]
pub fn proof(ticks: &[(TickEvents, View)]) -> (String, Vec<u8>) {
    let rate = RATE as usize;
    let beat = rate * 2 / 5;
    let tail = rate * 3 / 20;
    let mut mixed = vec![0_i32; ticks.len() * beat + tail];
    let mut text = String::new();
    for (index, (tick, view)) in ticks.iter().enumerate() {
        let heard = score(tick)
            .into_iter()
            .filter_map(|hit| gain(hit.at, *view).map(|gain| (hit, gain)))
            .collect::<Vec<_>>();
        let names = heard
            .iter()
            .map(|(hit, _)| crate::machines::name(hit.machine))
            .collect::<Vec<_>>()
            .join(", ");
        text.push_str(&format!("tick {} -> [{}]\n", tick.tick, names));
        for (hit, gain) in heard {
            for (at, sample) in if hit.upgrade {
                upgrade_samples()
            } else {
                samples(hit.instrument)
            }
            .into_iter()
            .enumerate()
            {
                mixed[index * beat + at] += (f32::from(sample) * gain) as i32;
            }
        }
    }
    let mixed = mixed
        .into_iter()
        .map(|sample| sample.clamp(i16::MIN.into(), i16::MAX.into()) as i16)
        .collect::<Vec<_>>();
    (text, wav_bytes(&mixed, RATE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::ArmLength;
    use crate::sim::{GlyphKind, Instr, Stall};

    #[test]
    fn action_defaults_overrides_and_manifest_roundtrip() {
        let entry: Entry = toml::from_str(
            r#"
instrument = { voice = "brass", note = 43 }
actions = { pickup = { voice = "wood", note = 60 } }
"#,
        )
        .unwrap();
        assert_eq!(
            entry.actions.get(Action::Pickup),
            Some(Instrument {
                voice: Voice::Wood,
                note: 60
            })
        );
        assert_eq!(entry.actions.get(Action::Drop), None);
        let text = toml::to_string(&entry.actions).unwrap();
        assert_eq!(
            toml::from_str::<ActionSounds>(&text).unwrap(),
            entry.actions
        );
        for machine in Machine::ALL.into_iter().map(Some).chain([None]) {
            for action in Action::ALL {
                let samples = action_samples(action_instrument(machine, action));
                assert_eq!(samples.len(), 4_320);
                assert_eq!(samples[0], 0);
                assert!(samples.last().unwrap().abs() < 10);
                assert!(samples.iter().any(|sample| sample.abs() > 100));
            }
        }
    }

    #[test]
    fn action_groups_are_order_independent_quiet_and_vary_for_a_minute() {
        use crate::sim::Item;
        let a = Item::Machine(Machine::ALL[0]);
        let b = Item::Machine(Machine::ALL[1]);
        let solo = ActionCue::new(Action::Drop, [a]);
        let group = ActionCue::new(Action::Drop, [a, b, a]);
        assert_eq!(group, ActionCue::new(Action::Drop, [b, a, a]));
        assert_eq!(group.machine, Some(Machine::ALL[0]));
        assert_eq!((group.count, group.kinds), (3, 2));
        assert!(group.settings(1, 1.0).1 < solo.settings(1, 1.0).1);
        let mut speeds = std::collections::BTreeSet::new();
        for i in 1..=240 {
            let (gain, speed) = group.settings(i, 0.5);
            assert!(gain < 0.06);
            speeds.insert(speed.to_bits());
        }
        assert_eq!(speeds.len(), 240);
    }

    #[test]
    fn an_action_group_spawns_one_player_with_a_prebuilt_sample() {
        use bevy::ecs::system::SystemState;
        let mut world = World::new();
        let mut state: SystemState<Commands> = SystemState::new(&mut world);
        let mut bank = Bank {
            actions: vec![std::array::from_fn(|_| Handle::default()); Machine::ALL.len() + 1],
            serial: 0,
            voices: Vec::new(),
            upgrade: Handle::default(),
            unlocked: true,
        };
        let cue = ActionCue::new(
            Action::Pickup,
            [crate::sim::Item::Machine(Machine::ALL[0]); 100],
        );
        play_actions(
            &mut state.get_mut(&mut world).unwrap(),
            &mut bank,
            &[cue],
            0.5,
        );
        state.apply(&mut world);
        assert_eq!(
            world
                .query::<&bevy::audio::AudioPlayer>()
                .iter(&world)
                .count(),
            1
        );
    }

    #[test]
    fn sound_begins_only_on_a_press_the_page_counts_as_activation() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<Touches>()
            .insert_resource(Bank {
                actions: Vec::new(),
                serial: 0,
                voices: Vec::new(),
                upgrade: Default::default(),
                unlocked: false,
            })
            .add_systems(Update, unlock);
        for key in [
            KeyCode::ShiftLeft,
            KeyCode::ControlLeft,
            KeyCode::Escape,
            KeyCode::KeyM,
        ] {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.clear();
            keys.press(key);
            app.update();
            assert_eq!(
                app.world().resource::<Bank>().unlocked,
                key == KeyCode::KeyM,
                "{key:?}"
            );
        }
        app.world_mut().resource_mut::<Bank>().unlocked = false;
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.add_message::<TouchInput>()
            .add_systems(PreUpdate, bevy::input::touch::touch_screen_input_system);
        let window = app.world_mut().spawn_empty().id();
        for (phase, unlocked) in [
            (bevy::input::touch::TouchPhase::Started, false),
            (bevy::input::touch::TouchPhase::Moved, false),
            (bevy::input::touch::TouchPhase::Ended, true),
        ] {
            app.world_mut().write_message(TouchInput {
                phase,
                position: Vec2::ZERO,
                window,
                force: None,
                id: 0,
            });
            app.update();
            assert_eq!(
                app.world().resource::<Bank>().unlocked,
                unlocked,
                "{phase:?}"
            );
        }
    }

    #[test]
    fn every_machine_kind_resolves_to_one_typed_instrument() {
        let instruments = Machine::ALL.map(instrument);
        assert_eq!(instruments.len(), Machine::ALL.len());
        for (index, left) in instruments.iter().enumerate() {
            assert!(instruments[index + 1..].iter().all(|right| left != right));
        }
    }

    #[test]
    fn score_at_tick_is_exact_and_a_stall_is_a_dropped_beat() {
        let source = Machine::Glyph(GlyphKind::Source);
        let tick = TickEvents {
            tick: 7,
            events: vec![
                TickEvent::Fired {
                    glyph: 2,
                    machine: source,
                    at: crate::sim::ORIGIN,
                },
                TickEvent::Spawned {
                    glyph: 2,
                    atom: 4,
                    kind: crate::sim::AtomKind::Base,
                    at: crate::sim::ORIGIN,
                },
                TickEvent::Stalled {
                    arm: 0,
                    instruction: Instr::Grab,
                    reason: Stall::Illegal,
                },
                TickEvent::Rotated {
                    arm: 1,
                    length: ArmLength::One,
                    spin: crate::sim::Spin::Cw,
                    at: crate::sim::ORIGIN,
                },
            ],
        };
        assert_eq!(
            score(&tick),
            vec![
                Hit {
                    upgrade: false,
                    machine: source,
                    instrument: instrument(source),
                    at: crate::sim::ORIGIN,
                },
                Hit {
                    upgrade: false,
                    machine: Machine::Arm(ArmLength::One),
                    instrument: instrument(Machine::Arm(ArmLength::One)),
                    at: crate::sim::ORIGIN,
                },
            ]
        );
    }

    #[test]
    fn view_keeps_on_screen_hits_and_rejects_off_screen_hits() {
        let view = View {
            center: Vec2::ZERO,
            half: Vec2::splat(crate::look::HEX * 2.0),
            scale: 0.5,
        };
        assert!(gain(crate::sim::ORIGIN, view).is_some());
        assert!(gain(crate::sim::Hex::new(3, 0), view).is_none());
    }

    #[test]
    fn closer_view_has_more_gain_than_wider_view() {
        let view = |scale| View {
            center: Vec2::ZERO,
            half: Vec2::splat(1000.0),
            scale,
        };
        let close = gain(crate::sim::ORIGIN, view(0.5)).unwrap();
        let wide = gain(crate::sim::ORIGIN, view(2.0)).unwrap();
        assert!(close > wide);
    }
}
