use crate::sim::{Machine, TickEvent, TickEvents};
use bevy::audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

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
                TickEvent::Fired { machine, at, .. } => (*machine, *at),
                TickEvent::Grabbed { at, .. }
                | TickEvent::Dropped { at, .. }
                | TickEvent::Rotated { at, .. }
                | TickEvent::Pivoted { at, .. } => (Machine::Arm, *at),
                TickEvent::Moved { to, .. } => (Machine::Arm, *to),
                TickEvent::BondWritten { .. }
                | TickEvent::Consumed { .. }
                | TickEvent::Spawned { .. }
                | TickEvent::Stalled { .. } => return None,
            };
            Some(Hit {
                machine,
                instrument: instrument(machine),
                at,
            })
        })
        .collect()
}

#[derive(Resource)]
pub struct Bank {
    voices: Vec<(Machine, Handle<AudioSource>)>,
    silence: Handle<AudioSource>,
    pub unlocked: bool,
}

pub fn load(mut commands: Commands, mut assets: ResMut<Assets<AudioSource>>) {
    let voices = Machine::ALL
        .into_iter()
        .map(|machine| {
            let handle = assets.add(AudioSource {
                bytes: wav(instrument(machine)).into(),
            });
            (machine, handle)
        })
        .collect();
    let silence = assets.add(AudioSource {
        bytes: wav_bytes(&[0; 32], 48_000).into(),
    });
    commands.insert_resource(Bank {
        voices,
        silence,
        unlocked: false,
    });
}

pub fn unlock(
    mut commands: Commands,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    touches: Res<Touches>,
    bank: Option<ResMut<Bank>>,
) {
    let Some(mut bank) = bank else { return };
    let pressed = buttons.get_just_pressed().next().is_some()
        || keys.get_just_pressed().next().is_some()
        || touches.any_just_pressed();
    if !bank.unlocked && pressed {
        bank.unlocked = true;
        commands.spawn((
            AudioPlayer::new(bank.silence.clone()),
            PlaybackSettings::DESPAWN,
        ));
    }
}

pub fn play(commands: &mut Commands, bank: &Bank, hits: &[Hit], view: View) {
    for hit in hits {
        let Some(gain) = gain(hit.at, view) else {
            continue;
        };
        let handle = bank
            .voices
            .iter()
            .find(|(machine, _)| *machine == hit.machine)
            .expect("every machine has a loaded instrument")
            .1
            .clone();
        commands.spawn((
            AudioPlayer::new(handle),
            PlaybackSettings {
                volume: Volume::Linear(gain),
                ..PlaybackSettings::DESPAWN
            },
        ));
    }
}

fn wav(instrument: Instrument) -> Vec<u8> {
    wav_bytes(&samples(instrument), 48_000)
}

fn samples(instrument: Instrument) -> Vec<i16> {
    let rate = 48_000;
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
    let rate = 48_000;
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
            for (at, sample) in samples(hit.instrument).into_iter().enumerate() {
                mixed[index * beat + at] += (f32::from(sample) * gain) as i32;
            }
        }
    }
    let mixed = mixed
        .into_iter()
        .map(|sample| sample.clamp(i16::MIN.into(), i16::MAX.into()) as i16)
        .collect::<Vec<_>>();
    (text, wav_bytes(&mixed, rate as u32))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{GlyphKind, Instr, Stall};

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
                    spin: crate::sim::Spin::Cw,
                    at: crate::sim::ORIGIN,
                },
            ],
        };
        assert_eq!(
            score(&tick),
            vec![
                Hit {
                    machine: source,
                    instrument: instrument(source),
                    at: crate::sim::ORIGIN,
                },
                Hit {
                    machine: Machine::Arm,
                    instrument: instrument(Machine::Arm),
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
