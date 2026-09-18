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

fn summarize(record: &Record) -> Value {
    json!({"token": record.token, "marks": marks(record)})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Refused, World, session::Session, sim};

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
        let mut session = Session::new(&world.sim);
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
}
