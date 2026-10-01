use crate::sim::{Machine, TickEvent};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Motion {
    Dilate,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Event {
    Fired,
    Rotated,
    Copied,
}

impl Event {
    pub fn matches(self, event: &TickEvent, index: usize) -> bool {
        match (self, event) {
            (Event::Fired, TickEvent::Fired { glyph, .. } | TickEvent::Upgraded { glyph, .. }) => {
                *glyph == index
            }
            (Event::Rotated, TickEvent::Rotated { arm, .. }) => *arm == index,
            (Event::Copied, TickEvent::Copied { portal, .. }) => *portal == index,
            _ => false,
        }
    }
}

#[derive(Deserialize)]
pub struct Entry {
    pub motion: Option<Motion>,
    pub emitter: crate::particles::Emitter,
    pub rig_emitter: Option<crate::particles::Emitter>,
}

#[derive(Deserialize)]
struct Manifest {
    machine: BTreeMap<String, Entry>,
}

fn manifest() -> &'static Manifest {
    static RIGS: OnceLock<Manifest> = OnceLock::new();
    RIGS.get_or_init(|| {
        toml::from_str(include_str!("../art/machines/manifest.toml"))
            .unwrap_or_else(|error| panic!("art/machines/manifest.toml: {error}"))
    })
}

pub fn entry(machine: Machine) -> &'static Entry {
    let name = crate::look::machine(machine)
        .skin
        .name
        .split('/')
        .nth(1)
        .expect("a machine skin lives in art/machines/<name>/");
    let entry = manifest()
        .machine
        .get(name)
        .unwrap_or_else(|| panic!("machine {name} has no rig"));
    let activation = entry.emitter.event;
    if let Some(emitter) = entry.rig_emitter {
        assert_eq!(
            emitter.event, activation,
            "machine {name} emitters disagree"
        );
    }
    entry
}

pub fn activation(machine: Machine) -> Event {
    entry(machine).emitter.event
}

impl Motion {
    pub fn scale(self, pulse: f32) -> f32 {
        match self {
            Self::Dilate => 1.0 + 0.12 * pulse,
        }
    }
}

pub fn pulse(fired: bool, phase: f32) -> f32 {
    if fired {
        (std::f32::consts::PI * phase.min(1.0)).sin()
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn motion_exists_only_for_its_event_and_returns_to_rest_inside_the_tick() {
        assert_eq!(pulse(false, 0.5), 0.0);
        assert!(pulse(true, 0.5) > 0.99);
        assert!(pulse(true, 1.0).abs() < 0.000_001);
    }
}
