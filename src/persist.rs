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
}

pub fn encode(sim: &Sim) -> Result<String, serde_json::Error> {
    serde_json::to_string(&Save {
        build: BUILD_TAG.to_owned(),
        sim: sim.clone(),
    })
}

pub fn decode(text: &str) -> Result<Sim, String> {
    decode_for(text, BUILD_TAG)
}

fn decode_for(text: &str, build: &str) -> Result<Sim, String> {
    let save: Save =
        serde_json::from_str(text).map_err(|_| "The save file is not valid.".to_owned())?;
    if save.build != build {
        return Err("The save belongs to a different build.".to_owned());
    }
    validate(&save.sim)?;
    Ok(save.sim)
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

pub fn restore(fallback: Sim) -> Sim {
    let Some(save) = stored_save() else {
        return fallback;
    };
    decode(&save).unwrap_or_else(|reason| {
        refuse_save(&reason);
        fallback
    })
}

pub fn store(sim: &Sim) -> Result<(), String> {
    let save = encode(sim).map_err(|error| error.to_string())?;
    store_save(&save).map_or(Ok(()), Err)
}

pub fn refuse(reason: &str) {
    refuse_save(reason);
}

pub fn download(sim: &Sim) {
    match encode(sim) {
        Ok(save) => download_save(&save),
        Err(error) => refuse_save(&error.to_string()),
    }
}

pub fn choose() {
    choose_save();
}

pub fn take() -> Option<Sim> {
    let save = imported_save()?;
    match decode(&save) {
        Ok(sim) => Some(sim),
        Err(reason) => {
            refuse_save(&reason);
            None
        }
    }
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
}
