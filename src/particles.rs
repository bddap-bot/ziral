use crate::rig::Event;
use crate::sim::{ActivationEnergy, Machine, TickEvent};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Look {
    Spark,
    Steam,
    Dust,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Layer {
    Behind,
    On,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Emitter {
    pub look: Look,
    pub count: u8,
    pub lifetime: u8,
    pub event: Event,
    pub layer: Layer,
    #[serde(default)]
    pub energy_scaled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Response {
    Default(Emitter),
    Rig(Emitter),
}

pub fn response(machine: Machine) -> Response {
    let entry = crate::rig::entry(machine);
    match entry.rig_emitter {
        Some(emitter) => Response::Rig(emitter),
        None => Response::Default(entry.emitter),
    }
}

pub fn burst(
    machine: Machine,
    index: usize,
    energy: ActivationEnergy,
    events: &[TickEvent],
) -> Option<(Emitter, usize)> {
    let emitter = match response(machine) {
        Response::Default(emitter) | Response::Rig(emitter) => emitter,
    };
    if machine == Machine::Arm
        && events
            .iter()
            .any(|event| matches!(event, TickEvent::Stalled { arm, .. } if *arm == index))
    {
        return None;
    }
    let starts = events
        .iter()
        .any(|event| emitter.event.matches(event, index));
    if energy.level() == 0 || (energy == ActivationEnergy::FULL && !starts) {
        return None;
    }
    let age = ActivationEnergy::FULL
        .level()
        .saturating_sub(energy.level());
    if age >= emitter.lifetime as usize {
        return None;
    }
    let count = if emitter.energy_scaled {
        usize::from(emitter.count) * energy.level() / ActivationEnergy::FULL.level()
    } else {
        usize::from(emitter.count)
    };
    (count > 0).then_some((emitter, count))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{GlyphKind, Instr, Stall};

    #[test]
    fn every_machine_kind_resolves_to_exactly_one_default_emitter_or_rig_override() {
        let mut defaults = 0;
        let mut overrides = 0;
        for machine in Machine::ALL {
            let response = response(machine);
            let emitter = match response {
                Response::Default(emitter) => {
                    defaults += 1;
                    emitter
                }
                Response::Rig(emitter) => {
                    overrides += 1;
                    emitter
                }
            };
            assert!((1..=12).contains(&emitter.count));
            assert!((1..=ActivationEnergy::FULL.level()).contains(&(emitter.lifetime as usize)));
        }
        assert_eq!((defaults, overrides), (7, 3));
    }

    #[test]
    fn bursts_start_on_the_driving_tick_finish_in_their_lifetime_and_stalls_start_nothing() {
        let machine = Machine::Glyph(GlyphKind::Bonder);
        let fired = [TickEvent::Fired {
            glyph: 4,
            machine,
            at: crate::sim::ORIGIN,
        }];
        let (emitter, _) = burst(machine, 4, ActivationEnergy::FULL, &fired).unwrap();
        let mut energy = ActivationEnergy::FULL;
        for _ in 1..emitter.lifetime {
            energy = energy.decayed();
            assert!(burst(machine, 4, energy, &[]).is_some());
        }
        energy = energy.decayed();
        assert!(burst(machine, 4, energy, &[]).is_none());
        let stalled = [TickEvent::Stalled {
            arm: 4,
            instruction: Instr::Grab,
            reason: Stall::Illegal,
        }];
        assert!(burst(Machine::Arm, 4, ActivationEnergy::FULL.decayed(), &stalled).is_none());
        assert!(burst(Machine::Arm, 4, ActivationEnergy::FULL, &stalled).is_none());
        assert!(burst(machine, 4, ActivationEnergy::FULL.decayed(), &stalled).is_some());
    }
}
