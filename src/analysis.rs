use crate::WorldAccess;
use crate::session::{Input, Record};
use bevy::prelude::KeyCode;
use serde_json::{Value, json};

pub fn run(args: &[String]) -> Option<i32> {
    if args.get(1).is_none_or(|command| command != "--analyze") {
        return None;
    }
    let [_, _, path] = args else {
        eprintln!("usage: ziral --analyze <record.json>");
        return Some(2);
    };
    let result = std::fs::read_to_string(path)
        .map_err(|error| error.to_string())
        .and_then(|text| Record::decode(&text));
    Some(match result {
        Ok(record) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&summarize(&record)).unwrap()
            );
            0
        }
        Err(error) => {
            eprintln!("{error}");
            1
        }
    })
}

fn marks(record: &Record) -> Vec<Value> {
    record
        .inputs
        .iter()
        .filter(|(_, input)| matches!(input, Input::Key(KeyCode::KeyM, _)))
        .map(|(at, _)| {
            json!({
                "at": at, "window": [(at - 15.0).max(0.0), (at + 15.0).min(record.duration())],
            })
        })
        .collect()
}

#[derive(serde::Serialize)]
struct ArmEdits {
    arm: usize,
    first_seen_at: f64,
    cell: crate::sim::Hex,
    edits: u64,
}

pub(super) fn summarize(record: &Record) -> Value {
    use crate::{Back, Focus, Game as World, Location, card_item, palette, sim};
    use sim::{GlyphKind, Id, Machine, TickEvent};
    let mut world = World::new(sim::start());
    let rows: Vec<_> = palette().collect();
    let mut touched = vec![false; rows.len()];
    let mut dwell: Vec<(sim::Item, f64)> = Vec::new();
    let mut arm_ids: Vec<usize> = Vec::new();
    let mut held_ids: Option<Vec<usize>> = None;
    let mut worlds = std::collections::BTreeMap::new();
    let mut edits: Vec<ArmEdits> = Vec::new();
    let mut first_machine = None;
    let mut first_bond = None;
    let mut first_recipe = None;
    let mut stalls = Vec::new();
    let mut steps = Vec::new();
    let mut acts = Vec::new();
    let mut last_act = 0.0;
    let mut paused = 0.0;
    for (at, input) in &record.inputs {
        if let Input::Frame(dt) = input {
            if world.inside() && !world.running {
                paused += f64::from(*dt);
            }
            if let Some(item) = world.hover {
                if let Some((_, seconds)) = dwell.iter_mut().find(|(other, _)| *other == item) {
                    *seconds += f64::from(*dt);
                } else {
                    dwell.push((item, f64::from(*dt)));
                }
            }
        }
        let touched_item = match input {
            Input::Palette(Some(item))
            | Input::Inventory(item)
            | Input::Cap(item, _)
            | Input::Pin(item, ..) => Some(*item),
            _ => None,
        };
        if let Some(index) = touched_item.and_then(|item| rows.iter().position(|row| *row == item))
        {
            touched[index] = true;
        }
        let old_count = world.sim().arms.len();
        let old_machines = old_count + world.sim().glyphs.iter().flatten().count();
        let picked = world.focus.as_ref().map_or(Vec::new(), Focus::picked);
        let inventory = matches!(
            world.focus,
            Some(Focus::Hold {
                back: Back::Inventory,
                ..
            })
        );
        let tape = if matches!(input, Input::Key(..)) {
            if let Some(Focus::Tape { arm, .. }) = world.focus {
                Some((arm, world.sim().arms[arm].tape.clone()))
            } else {
                None
            }
        } else {
            None
        };
        let tick = world.shown().tick;
        let real_tick = world.overworld.sim.tick;
        let location = match world.location {
            Location::Overworld => None,
            Location::Interior { portal, .. } => Some(portal),
        };
        let stalled: Vec<_> = world
            .shown()
            .arms
            .iter()
            .enumerate()
            .filter(|(_, arm)| arm.stall.is_some())
            .map(|(index, _)| arm_ids[index])
            .collect();
        let held = held_ids.is_some();
        input.apply(&mut world);
        if matches!(input, Input::Import(_) | Input::Restore(_)) {
            worlds.clear();
            arm_ids.clear();
            held_ids = None;
        } else if matches!(input, Input::Focus(_)) {
            let next = match world.location {
                Location::Overworld => None,
                Location::Interior { portal, .. } => Some(portal),
            };
            worlds.insert(location, std::mem::take(&mut arm_ids));
            arm_ids = worlds.remove(&next).unwrap_or_default();
            held_ids = None;
        } else {
            let holding = matches!(
                world.focus,
                Some(Focus::Hold {
                    back: Back::Pick { .. },
                    ..
                })
            );
            if !holding
                && let Some(previous) = held_ids.take()
                && previous.len() == world.sim().arms.len()
            {
                arm_ids = previous;
            }
            if world.sim().arms.len() < old_count {
                if holding {
                    held_ids = Some(arm_ids.clone());
                }
                arm_ids = arm_ids
                    .into_iter()
                    .enumerate()
                    .filter(|(index, _)| !picked.contains(&Id::Arm(*index)))
                    .map(|(_, id)| id)
                    .collect();
            }
        }
        for arm in &world.sim().arms[arm_ids.len()..] {
            arm_ids.push(edits.len());
            edits.push(ArmEdits {
                arm: edits.len(),
                first_seen_at: *at,
                cell: arm.pivot,
                edits: 0,
            });
        }
        if let Some((arm, before)) = tape
            && let Some(after) = world.sim().arms.get(arm)
            && before != after.tape
        {
            edits[arm_ids[arm]].edits += 1;
        }
        if !world.inside()
            && inventory
            && matches!(input, Input::Press { .. } | Input::Release(_))
            && old_machines < world.sim().arms.len() + world.sim().glyphs.iter().flatten().count()
        {
            first_machine.get_or_insert(*at);
        }
        if !matches!(
            input,
            Input::Import(_) | Input::Restore(_) | Input::Focus(_)
        ) && world.overworld.sim.tick > real_tick
            && let Some(events) = world.overworld.events.last()
        {
            for event in &events.events {
                match event {
                    TickEvent::BondWritten { .. } => {
                        first_bond.get_or_insert(*at);
                    }
                    TickEvent::Fired {
                        machine: Machine::Glyph(GlyphKind::Output(_)),
                        ..
                    } => {
                        first_recipe.get_or_insert(*at);
                    }
                    _ => {}
                }
            }
        }
        if !matches!(
            input,
            Input::Import(_) | Input::Restore(_) | Input::Focus(_)
        ) && !held
            && held_ids.is_none()
        {
            for (index, arm) in world.shown().arms.iter().enumerate() {
                if arm.stall.is_some() && !stalled.contains(&arm_ids[index]) {
                    stalls.push((*at, arm_ids[index]));
                }
            }
        }
        if matches!(input, Input::Key(KeyCode::KeyG | KeyCode::KeyS, _))
            && world.shown().tick != tick
        {
            steps.push(*at);
        }
        let act = match input {
            Input::Key(KeyCode::KeyM | KeyCode::ShiftLeft | KeyCode::ShiftRight, _) => None,
            Input::Key(key, shift) => Some(json!({"key": key, "shift": shift})),
            Input::Press { point, .. } => Some(json!({"cell": crate::hex_at(*point)})),
            Input::Release(cell) => Some(json!({"release": cell})),
            Input::Inventory(item) | Input::Pin(item, ..) => Some(json!({"palette": item})),
            Input::Tape { arm, cursor } => Some(json!({"arm": arm_ids[*arm], "cursor": cursor})),
            Input::Cap(item, notches) => Some(json!({"cap": item, "notches": notches})),
            Input::Paste(_) => Some(json!({"paste": true})),
            Input::Drag => Some(json!({"drag": true})),
            Input::Card(id, at, button) => Some(json!({"card": id, "at": at, "button": button})),
            Input::EndCard(panel) => Some(json!({"end_card": panel})),
            Input::ScaleCard(id, scale) => Some(json!({"card": id, "scale": scale})),
            Input::Wheel(delta, pixels) => Some(json!({"wheel": delta, "pixels": pixels})),
            _ => None,
        };
        if let Some(act) = act {
            acts.push(json!({"at": at, "seconds": at - last_act, "act": act}));
            last_act = *at;
        }
    }
    let duration = record.duration();
    let around_stalls: Vec<_> = stalls
        .iter()
        .map(|(at, arm)| {
            json!({
                "at": at, "arm": arm,
                "window": [(at - 15.0).max(0.0), (at + 15.0).min(duration)],
                "steps": steps.iter().filter(|step| (**step - at).abs() <= 15.0).count(),
            })
        })
        .collect();
    json!({
        "token": record.token,
        "duration_seconds": duration,
        "first_machine_placed_seconds": first_machine,
        "first_bond_seconds": first_bond,
        "first_automated_recipe_seconds": first_recipe,
        "stalls_per_minute": if duration > 0.0 { Some(stalls.len() as f64 * 60.0 / duration) } else { None },
        "tape_edits_per_arm": edits,
        "palette_rows_never_touched": rows.iter().zip(touched).filter(|(_, touched)| !touched).map(|(item, _)| item).collect::<Vec<_>>(),
        "hesitation_before_each_act": acts,
        "hover_card_dwell_seconds": dwell.into_iter().map(|(item, seconds)| json!({"card": card_item(item), "seconds": seconds})).collect::<Vec<_>>(),
        "steps_around_stalls": around_stalls,
        "paused_seconds": paused,
        "marks": marks(record),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Game as World, Refused, session::Session, sim};

    #[test]
    fn analysis_requires_one_record_path() {
        assert_eq!(run(&["ziral".into(), "--analyze".into()]), Some(2));
        assert_eq!(
            run(&[
                "ziral".into(),
                "--analyze".into(),
                "one".into(),
                "two".into()
            ]),
            Some(2)
        );
        assert_eq!(run(&["ziral".into()]), None);
    }

    #[test]
    fn mark_preserves_world_and_reports_thirty_second_windows() {
        let mut world = World::new(sim::start());
        let mut session = Session::new(&world.state());
        session.send(&mut world, Input::Key(KeyCode::Space, false));
        session.send(&mut world, Input::Frame(5.0));
        world.refused = Some(Refused {
            at: sim::ORIGIN,
            short: Vec::new(),
        });
        let before = format!("{world:?}");
        session.send(&mut world, Input::Key(KeyCode::KeyM, false));
        assert_eq!(format!("{world:?}"), before);
        session.send(&mut world, Input::Frame(40.0));
        session.send(&mut world, Input::Key(KeyCode::KeyM, false));
        session.send(&mut world, Input::Frame(5.0));
        assert_eq!(
            marks(&session.record),
            vec![
                json!({"at": 5.0, "window": [0.0, 20.0]}),
                json!({"at": 45.0, "window": [30.0, 50.0]}),
            ]
        );
    }
    #[test]
    fn metrics_recompute_placement_dwell_pause_and_hesitation() {
        use sim::{ArmLength, Item, Machine};
        let mut world = World::new(sim::start());
        let mut session = Session::new(&world.state());
        let arm = Item::Machine(Machine::Arm(ArmLength::One));
        for input in [
            Input::Key(KeyCode::Space, false),
            Input::Frame(2.0),
            Input::Hover(Some(arm)),
            Input::Palette(Some(arm)),
            Input::Frame(3.0),
            Input::Refill,
            Input::Inventory(arm),
            Input::Release(Some(sim::Hex::new(10, 10))),
            Input::Hover(None),
            Input::Frame(5.0),
        ] {
            session.send(&mut world, input);
        }
        session.record.token = Some("a".repeat(192));
        let result = summarize(&session.record);
        assert_eq!(result["token"], "a".repeat(192));
        assert_eq!(result["first_machine_placed_seconds"], 5.0);
        assert_eq!(result["paused_seconds"], 0.0);
        assert_eq!(
            result["hover_card_dwell_seconds"],
            json!([{"card": arm, "seconds": 3.0}])
        );
        assert_eq!(result["hesitation_before_each_act"][1]["seconds"], 5.0);
        assert!(
            !result["palette_rows_never_touched"]
                .as_array()
                .unwrap()
                .contains(&json!(arm))
        );
        assert_eq!(result["first_bond_seconds"], Value::Null);
        assert_eq!(result["stalls_per_minute"], 0.0);
        let mut world = World::new(sim::Sim::empty());
        let mut session = Session::new(&world.state());
        session.send(&mut world, Input::Inventory(arm));
        session.send(
            &mut world,
            Input::Import(Box::new(
                sim::fixture(Machine::Glyph(sim::GlyphKind::Bonder)).sim,
            )),
        );
        assert_eq!(
            summarize(&session.record)["first_machine_placed_seconds"],
            Value::Null
        );
        session.send(
            &mut world,
            Input::Pin(
                arm,
                bevy::prelude::Vec2::ZERO,
                bevy::prelude::MouseButton::Left,
            ),
        );
        session.send(&mut world, Input::EndCard(false));
        session.send(&mut world, Input::Frame(2.0));
        session.send(
            &mut world,
            Input::Card(
                0,
                bevy::prelude::Vec2::ZERO,
                bevy::prelude::MouseButton::Left,
            ),
        );
        session.send(&mut world, Input::Frame(3.0));
        session.send(&mut world, Input::ScaleCard(0, 2.0));
        let acts = summarize(&session.record)["hesitation_before_each_act"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(acts[acts.len() - 2]["seconds"], 2.0);
        assert_eq!(acts[acts.len() - 1]["seconds"], 3.0);

        assert_eq!(
            summarize(&Session::new(&World::new(sim::start()).state()).record)["stalls_per_minute"],
            Value::Null
        );
    }

    #[test]
    fn metrics_count_real_milestones_and_stall_entries_with_steps() {
        use sim::{Arm, ArmLength, GlyphKind, Instr, Machine, Tier};
        let initial = sim::fixture(Machine::Glyph(GlyphKind::Bonder)).sim;
        let mut world = World::new(initial);
        let mut session = Session::new(&world.state());
        for _ in 0..13 {
            session.send(&mut world, Input::Frame(0.4));
        }
        let result = summarize(&session.record);
        assert!(
            (result["first_bond_seconds"].as_f64().unwrap() - 4.8).abs() < 0.00001,
            "{result}"
        );
        assert_eq!(result["first_machine_placed_seconds"], Value::Null);
        let initial = sim::fixture(Machine::Glyph(GlyphKind::Output(Tier::One))).sim;
        let mut world = World::new(initial);
        let mut session = Session::new(&world.state());
        session.send(&mut world, Input::Key(KeyCode::Space, false));
        for _ in 0..3 {
            session.send(&mut world, Input::Key(KeyCode::KeyG, false));
        }
        assert_eq!(
            summarize(&session.record)["first_automated_recipe_seconds"],
            Value::Null
        );
        session.send(&mut world, Input::Key(KeyCode::Space, false));
        for _ in 0..3 {
            session.send(&mut world, Input::Frame(0.4));
        }
        assert!(
            (summarize(&session.record)["first_automated_recipe_seconds"]
                .as_f64()
                .unwrap()
                - 0.8)
                .abs()
                < 0.00001
        );
        let mut initial = sim::Sim::empty();
        initial
            .arms
            .push(Arm::new(ArmLength::One, sim::ORIGIN, 0, vec![Instr::Grab]));
        let mut world = World::new(initial);
        let mut session = Session::new(&world.state());
        for _ in 0..5 {
            session.send(&mut world, Input::Frame(0.4));
        }
        session.send(&mut world, Input::Key(KeyCode::Space, false));
        session.send(&mut world, Input::Key(KeyCode::KeyG, false));
        session.send(&mut world, Input::Key(KeyCode::KeyS, false));
        let result = summarize(&session.record);
        assert_eq!(result["steps_around_stalls"].as_array().unwrap().len(), 1);
        assert_eq!(result["steps_around_stalls"][0]["steps"], 0);
        let mut initial = sim::Sim::empty();
        initial.fill_inventory();
        initial
            .arms
            .push(Arm::new(ArmLength::One, sim::ORIGIN, 0, Vec::new()));
        let mut world = World::new(sim::start());
        world.overworld.sim.portals[0].as_mut().unwrap().sim = initial;
        let mut session = Session::new(&world.state());
        session.send(&mut world, Input::Focus(Some(0)));
        session.send(&mut world, Input::Key(KeyCode::KeyG, false));
        session.send(&mut world, Input::Tape { arm: 0, cursor: 0 });
        let grab = crate::KEYS
            .iter()
            .find(|key| key.instr == Instr::Grab)
            .unwrap()
            .code;
        session.send(&mut world, Input::Key(grab, false));
        assert_eq!(
            summarize(&session.record)["steps_around_stalls"]
                .as_array()
                .unwrap()
                .len(),
            1
        );

        assert!((result["stalls_per_minute"].as_f64().unwrap() - 30.0).abs() < 0.00001);
    }

    #[test]
    fn tape_metrics_preserve_arm_identity_through_move_and_deletion() {
        use sim::{Arm, ArmLength, Hex, Instr};
        let mut initial = sim::Sim::empty();
        initial.fill_inventory();
        initial
            .arms
            .push(Arm::new(ArmLength::One, sim::ORIGIN, 0, Vec::new()));
        initial
            .arms
            .push(Arm::new(ArmLength::One, Hex::new(5, 0), 0, Vec::new()));
        let mut world = World::new(initial);
        let mut session = Session::new(&world.state());
        let grab = crate::KEYS
            .iter()
            .find(|key| key.instr == Instr::Grab)
            .unwrap()
            .code;
        for input in [
            Input::Key(KeyCode::Space, false),
            Input::Tape { arm: 1, cursor: 0 },
            Input::Key(grab, false),
            Input::Press {
                point: crate::px(Hex::new(5, 0)),
                target: Some(sim::Id::Arm(1)),
            },
            Input::Drag,
            Input::Release(Some(Hex::new(8, 0))),
            Input::Tape { arm: 1, cursor: 1 },
            Input::Key(grab, false),
            Input::Press {
                point: crate::px(sim::ORIGIN),
                target: Some(sim::Id::Arm(0)),
            },
            Input::Drag,
            Input::Key(KeyCode::KeyZ, false),
            Input::Tape { arm: 0, cursor: 2 },
            Input::Key(grab, false),
        ] {
            session.send(&mut world, input);
        }
        assert_eq!(world.sim().arms.len(), 1);
        assert_eq!(world.sim().arms[0].pivot, Hex::new(8, 0));
        let result = summarize(&session.record);
        let arms = result["tape_edits_per_arm"].as_array().unwrap();
        assert_eq!(arms.len(), 2);
        assert_eq!(arms[0]["edits"], 0);
        assert_eq!(arms[1]["edits"], 3);
    }
}
