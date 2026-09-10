use crate::sim::{Machine, TickEvent};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Motion {
    Clamp,
    Turn,
    Dilate,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Event {
    Fired,
    Rotated,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Mask {
    Outside,
    Inside,
}

impl Event {
    pub fn matches(self, event: &TickEvent, index: usize) -> bool {
        match (self, event) {
            (Event::Fired, TickEvent::Fired { glyph }) => *glyph == index,
            (Event::Rotated, TickEvent::Rotated { arm, .. }) => *arm == index,
            _ => false,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Part {
    pub name: String,
    pub mask: Mask,
    pub pivot: [f32; 2],
    pub score: u8,
    pub motion: Option<Motion>,
    pub event: Option<Event>,
}

#[derive(Deserialize)]
struct Entry {
    parts: Vec<Part>,
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

pub fn parts(machine: Machine) -> &'static [Part] {
    let name = crate::look::machine(machine)
        .skin
        .name
        .split('/')
        .nth(1)
        .expect("a machine skin lives in art/machines/<name>/");
    let parts = &manifest()
        .machine
        .get(name)
        .unwrap_or_else(|| panic!("machine {name} has no rig"))
        .parts;
    assert!(
        (2..=4).contains(&parts.len()),
        "machine {name} rig has {} parts",
        parts.len()
    );
    for part in parts {
        assert_eq!(
            part.motion.is_some(),
            part.event.is_some(),
            "machine {name} part {} has an incomplete driver",
            part.name
        );
    }
    parts
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
    fn every_machine_has_two_to_four_parts_and_every_motion_resolves_to_a_tick_event() {
        for machine in Machine::ALL {
            let parts = parts(machine);
            assert!((2..=4).contains(&parts.len()));
            assert_eq!(parts.iter().filter(|part| part.motion.is_none()).count(), 1);
            for part in parts {
                assert!(part.score >= 8);
                assert_eq!(part.motion.is_some(), part.event.is_some());
                if let Some(event) = part.event {
                    let sample = match event {
                        Event::Fired => TickEvent::Fired { glyph: 7 },
                        Event::Rotated => TickEvent::Rotated {
                            arm: 7,
                            spin: crate::sim::Spin::Cw,
                        },
                    };
                    assert!(event.matches(&sample, 7));
                }
            }
        }
    }

    #[test]
    fn every_part_has_an_equal_sized_albedo_and_normal_map_without_stretching() {
        for machine in Machine::ALL {
            for part in parts(machine) {
                let (albedo, normal) = crate::look::rig(machine, &part.name);
                assert_ne!(albedo.name, normal.name);
                let albedo = albedo.decode();
                let normal = normal.decode();
                assert_eq!(
                    (albedo.width(), albedo.height()),
                    (normal.width(), normal.height())
                );
                assert_eq!(albedo.width(), albedo.height());
                let visible = albedo
                    .data
                    .as_ref()
                    .expect("a decoded part has pixels")
                    .chunks_exact(4)
                    .filter(|pixel| pixel[3] > 0)
                    .count();
                assert!(visible > 10, "{machine:?} {} is empty", part.name);
                assert!(
                    visible < (albedo.width() * albedo.height()) as usize,
                    "{machine:?} {} has no transparency",
                    part.name
                );
            }
        }
    }

    #[test]
    fn motion_exists_only_for_its_event_and_returns_to_rest_inside_the_tick() {
        assert_eq!(pulse(false, 0.5), 0.0);
        assert!(pulse(true, 0.5) > 0.99);
        assert!(pulse(true, 1.0).abs() < 0.000_001);
    }
}
