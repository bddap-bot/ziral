use crate::sim::Sim;
use serde::{Deserialize, Serialize};

pub const BUILD_TAG: &str = match option_env!("ZIRAL_BUILD_TAG") {
    Some(tag) => tag,
    None => env!("CARGO_PKG_VERSION"),
};

#[derive(Deserialize, Serialize)]
struct Save {
    build: String,
    sim: Sim,
    #[serde(default)]
    portals: Vec<Sim>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct State {
    pub sim: Sim,
    pub portals: Vec<Sim>,
}

pub fn encode(sim: &Sim) -> Result<String, serde_json::Error> {
    serde_json::to_string(&Save {
        build: BUILD_TAG.to_owned(),
        sim: sim.clone(),
        portals: Vec::new(),
    })
}

pub fn decode(text: &str) -> Result<Sim, String> {
    decode_for(text, BUILD_TAG)
}

fn decode_for(text: &str, build: &str) -> Result<Sim, String> {
    decode_state_for(text, build).map(|state| state.sim)
}

pub fn encode_state(state: &State) -> Result<String, serde_json::Error> {
    serde_json::to_string(&Save {
        build: BUILD_TAG.to_owned(),
        sim: state.sim.clone(),
        portals: state.portals.clone(),
    })
}

pub fn decode_state(text: &str) -> Result<State, String> {
    decode_state_for(text, BUILD_TAG)
}

fn decode_state_for(text: &str, build: &str) -> Result<State, String> {
    let mut save: Save =
        serde_json::from_str(text).map_err(|_| "The save file is not valid.".to_owned())?;
    if save.build != build {
        return Err("The save belongs to a different build.".to_owned());
    }
    if !save.sim.inventory.snap_caps() {
        return Err("The save file is not valid.".to_owned());
    }
    validate(&save.sim)?;
    if !save.portals.is_empty() && save.portals.len() != save.sim.portals.len() {
        return Err("The save file is not valid.".to_owned());
    }
    for sim in &mut save.portals {
        if !sim.portals.is_empty() || !sim.inventory.snap_caps() {
            return Err("The save file is not valid.".to_owned());
        }
        validate(sim)?;
    }
    Ok(State {
        sim: save.sim,
        portals: save.portals,
    })
}

fn validate(sim: &Sim) -> Result<(), String> {
    let directions = sim.arms.iter().all(|arm| arm.dir < crate::sim::DIRS.len())
        && sim
            .glyphs
            .iter()
            .flatten()
            .all(|glyph| glyph.dir < crate::sim::DIRS.len());
    let moves = sim.arms.iter().flat_map(|arm| &arm.tape).all(
        |instr| !matches!(instr, crate::sim::Instr::Move(dir) if *dir >= crate::sim::DIRS.len()),
    );
    let energy = sim
        .arms
        .iter()
        .all(|arm| arm.energy.level() <= crate::sim::ActivationEnergy::FULL.level())
        && sim
            .glyphs
            .iter()
            .flatten()
            .all(|glyph| glyph.energy.level() <= crate::sim::ActivationEnergy::FULL.level());
    let stalls = sim.arms.iter().all(
        |arm| !matches!(arm.stall, Some(crate::sim::Stall::Hand(other)) if other >= sim.arms.len()),
    );
    let glyphs = sim.glyphs.iter().flatten().all(|glyph| {
        !matches!(
            glyph.kind,
            crate::sim::GlyphKind::Converter(crate::sim::AtomKind::Base)
        )
    });
    let bonds = sim.bonds.iter().all(|bond| {
        bond.a != bond.b
            && sim.atoms.get(bond.a).is_some_and(Option::is_some)
            && sim.atoms.get(bond.b).is_some_and(Option::is_some)
    });
    if directions && moves && energy && stalls && glyphs && bonds && sim.inventory.valid() {
        Ok(())
    } else {
        Err("The save file is not valid.".to_owned())
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(inline_js = r#"
export function stored_save() {
    try { return localStorage.getItem("ziral-save"); } catch (_) { return null; }
}
export function store_save(save) {
    try { localStorage.setItem("ziral-save", save); return null; } catch (error) { return String(error); }
}
export function download_save(save) {
    const url = URL.createObjectURL(new Blob([save], {type: "application/json"}));
    const link = document.createElement("a");
    link.href = url;
    link.download = "ziral.json";
    link.click();
    URL.revokeObjectURL(url);
}
export function choose_save() {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "application/json,.json";
    input.onchange = () => {
        const file = input.files[0];
        if (file) file.text().then(save => globalThis.ziralImportedSave = save).catch(error => refuse_save(String(error)));
    };
    input.click();
}
export function imported_save() {
    const save = globalThis.ziralImportedSave;
    delete globalThis.ziralImportedSave;
    return save;
}
export function refuse_save(reason) {
    console.error(reason);
    alert(reason);
}
"#)]
extern "C" {
    fn stored_save() -> Option<String>;
    fn store_save(save: &str) -> Option<String>;
    fn download_save(save: &str);
    fn choose_save();
    fn imported_save() -> Option<String>;
    fn refuse_save(reason: &str);
}

#[cfg(not(target_arch = "wasm32"))]
fn stored_save() -> Option<String> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn store_save(_: &str) -> Option<String> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn download_save(_: &str) {}

#[cfg(not(target_arch = "wasm32"))]
fn choose_save() {}

#[cfg(not(target_arch = "wasm32"))]
fn imported_save() -> Option<String> {
    None
}

#[cfg(not(target_arch = "wasm32"))]
fn refuse_save(_: &str) {}

pub fn restore(fallback: State) -> State {
    let Some(save) = stored_save() else {
        return fallback;
    };
    decode_state(&save).unwrap_or_else(|reason| {
        refuse_save(&reason);
        fallback
    })
}

pub fn store(state: &State) -> Result<(), String> {
    let save = encode_state(state).map_err(|error| error.to_string())?;
    store_save(&save).map_or(Ok(()), Err)
}

pub fn refuse(reason: &str) {
    refuse_save(reason);
}

pub fn download(state: &State) {
    match encode_state(state) {
        Ok(save) => download_save(&save),
        Err(error) => refuse_save(&error.to_string()),
    }
}

pub fn choose() {
    choose_save();
}

pub fn take() -> Option<String> {
    imported_save()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_round_trip_preserves_the_simulation() {
        let mut before = crate::sim::start();
        before.step();
        let save = encode(&before).unwrap();
        assert_eq!(decode(&save), Ok(before));
    }

    #[test]
    fn save_from_another_build_is_refused() {
        let save = encode(&crate::sim::start()).unwrap();
        assert_eq!(
            decode_for(&save, "another-build"),
            Err("The save belongs to a different build.".to_owned())
        );
    }

    #[test]
    fn legacy_inventory_caps_snap_to_the_nearest_whole_pip() {
        let save = encode(&crate::sim::start()).unwrap();
        let mut save: serde_json::Value = serde_json::from_str(&save).unwrap();
        let caps = save["sim"]["inventory"]["cap"].as_array_mut().unwrap();
        let old = [0, 1, 2, 3, 6, 7, 9, 12, 15, 17, 255];
        let expected = [1, 1, 2, 4, 8, 8, 8, 16, 16, 16, 256];
        for (cap, value) in caps.iter_mut().zip(old.into_iter().cycle()) {
            *cap = value.into();
        }
        let decoded = decode(&serde_json::to_string(&save).unwrap()).unwrap();
        let encoded: serde_json::Value = serde_json::from_str(&encode(&decoded).unwrap()).unwrap();
        let snapped = encoded["sim"]["inventory"]["cap"].as_array().unwrap();
        for (cap, expected) in snapped.iter().zip(expected.into_iter().cycle()) {
            assert_eq!(cap.as_u64(), Some(expected));
        }
    }

    #[test]
    fn an_inventory_cap_above_the_bound_remains_an_invalid_save() {
        let save = encode(&crate::sim::start()).unwrap();
        let mut save: serde_json::Value = serde_json::from_str(&save).unwrap();
        save["sim"]["inventory"]["cap"][0] = (crate::sim::MAX_CAP + 1).into();
        assert_eq!(
            decode(&serde_json::to_string(&save).unwrap()),
            Err("The save file is not valid.".to_owned())
        );
    }
}
