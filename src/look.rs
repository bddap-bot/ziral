use crate::Item;
use crate::sim::{AtomKind, BondKind, GlyphKind, Hex};
use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, Image, ImageSampler, ImageType};
use bevy::prelude::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glaze {
    Clay,
    Brass,
    Terracotta,
    BlueGreen,
    Amber,
    Plum,
    Ivory,
}

impl Glaze {
    pub const ALL: [Glaze; 7] = [
        Glaze::Clay,
        Glaze::Brass,
        Glaze::Terracotta,
        Glaze::BlueGreen,
        Glaze::Amber,
        Glaze::Plum,
        Glaze::Ivory,
    ];

    pub const fn color(self) -> Color {
        match self {
            Glaze::Clay => Color::srgb_u8(0xD8, 0xC3, 0xA5),
            Glaze::Brass => Color::srgb_u8(0x6B, 0x4F, 0x3A),
            Glaze::Terracotta => Color::srgb_u8(0xC8, 0x55, 0x3D),
            Glaze::BlueGreen => Color::srgb_u8(0x4F, 0x8A, 0x8B),
            Glaze::Amber => Color::srgb_u8(0xE0, 0xA4, 0x58),
            Glaze::Plum => Color::srgb_u8(0x7D, 0x5B, 0xA6),
            Glaze::Ivory => Color::srgb_u8(0xF4, 0xED, 0xE4),
        }
    }
}

#[derive(Clone, Copy)]
pub struct Token {
    pub face: Glaze,
    pub field: Glaze,
}

#[derive(Clone, Copy)]
pub struct Skin {
    pub name: &'static str,
    pub(crate) png: &'static [u8],
    pub alpha: bool,
}

impl PartialEq for Skin {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Skin {}

impl std::fmt::Debug for Skin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "art/{}.png", self.name)
    }
}

macro_rules! skin {
    ($name:expr) => {
        Skin {
            name: $name,
            png: include_bytes!(concat!("../art/", $name, ".png")),
            alpha: false,
        }
    };
}
pub(crate) use skin;

macro_rules! sprite {
    ($name:expr) => {{
        let mut skin = skin!($name);
        skin.alpha = true;
        skin
    }};
}

macro_rules! tiles {
    ($($n:literal),*) => {
        [$(skin!(concat!("textures/tile-", $n))),*]
    };
}

pub const TILES: [Skin; 24] = tiles![
    "00", "01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11", "12", "13", "14", "15",
    "16", "17", "18", "19", "20", "21", "22", "23"
];

impl Skin {
    pub fn decode(self) -> Image {
        Image::from_buffer(
            self.png,
            ImageType::Extension("png"),
            CompressedImageFormats::NONE,
            true,
            ImageSampler::linear(),
            RenderAssetUsages::RENDER_WORLD,
        )
        .unwrap_or_else(|e| panic!("{self:?} does not decode: {e}"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tile {
    pub skin: Skin,
}

pub fn tile(h: Hex) -> Tile {
    let x = h.scramble();
    Tile {
        skin: TILES[(x % TILES.len() as u32) as usize],
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    Bead,
    Bars(usize),
    Radial,
    Cells(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineMark {
    Hand(Glaze),
    Sprite,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look<M> {
    pub glaze: Glaze,
    pub skin: Skin,
    pub shape: Shape,
    pub marking: M,
}

pub fn atom(kind: AtomKind) -> Look<()> {
    match kind {
        AtomKind::Base => Look {
            glaze: Glaze::BlueGreen,
            skin: skin!("textures/atom-base"),
            shape: Shape::Bead,
            marking: (),
        },
    }
}

pub fn bond(kind: BondKind) -> Look<()> {
    let (glaze, skin, bars) = match kind {
        BondKind::Single => (Glaze::Brass, skin!("textures/bond-single"), 1),
        BondKind::Double => (Glaze::Plum, skin!("textures/bond-double"), 2),
    };
    Look {
        glaze,
        skin,
        shape: Shape::Bars(bars),
        marking: (),
    }
}

pub fn machine(item: Item) -> Look<MachineMark> {
    let kind = match item {
        Item::Arm => {
            return Look {
                glaze: Glaze::Brass,
                skin: sprite!("textures/arm"),
                shape: Shape::Radial,
                marking: MachineMark::Hand(Glaze::Terracotta),
            };
        }
        Item::Glyph(kind) => kind,
    };
    let (glaze, skin) = match kind {
        GlyphKind::Source => (Glaze::BlueGreen, sprite!("textures/source")),
        GlyphKind::Bonder => (Glaze::Terracotta, sprite!("textures/bonder")),
        GlyphKind::SecondBond => (Glaze::Plum, sprite!("textures/second-bond")),
        GlyphKind::Output => (Glaze::Ivory, sprite!("textures/output")),
        GlyphKind::Cleanup => (Glaze::Brass, sprite!("textures/cleanup")),
    };
    Look {
        glaze,
        skin,
        shape: Shape::Cells(kind.rule().slots.len()),
        marking: MachineMark::Sprite,
    }
}

pub fn skins() -> impl Iterator<Item = Skin> {
    AtomKind::ALL
        .into_iter()
        .map(|k| atom(k).skin)
        .chain(BondKind::ALL.into_iter().map(|k| bond(k).skin))
        .chain(crate::PALETTE.into_iter().map(|item| machine(item).skin))
        .chain(TILES)
        .chain(crate::KEYS.iter().map(|k| k.symbol))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KEYS, PALETTE, SYMBOL_PX};
    use bevy::color::{Hsva, Luminance};

    const HUE_APART: f32 = 40.0;
    const CHROMA_FLOOR: f32 = 0.15;
    const VALUE_APART: f32 = 0.15;
    const THUMB: usize = 8;
    const TILES_APART: f32 = 0.023;
    const SHADING: f32 = 2.0 * VALUE_APART;
    const SPLIT_ROUNDS: usize = 8;

    fn hue_and_value_differ(a: Color, b: Color) -> [bool; 2] {
        let (ca, cb) = (Hsva::from(a), Hsva::from(b));
        let chroma = |c: Hsva| c.saturation * c.value;
        let turn = (ca.hue - cb.hue).abs();
        let hue = chroma(ca) >= CHROMA_FLOOR
            && chroma(cb) >= CHROMA_FLOOR
            && turn.min(360.0 - turn) >= HUE_APART;
        let value = (a.luminance() - b.luminance()).abs() >= VALUE_APART;
        [hue, value]
    }

    fn pixels(skin: Skin) -> (usize, usize, Vec<u8>) {
        let image = skin.decode();
        let (w, h) = (image.width() as usize, image.height() as usize);
        let data = image.data.expect("a decoded image carries its pixels");
        assert_eq!(data.len(), w * h * 4, "{skin:?} is not rgba8");
        (w, h, data)
    }

    fn thumbnail(skin: Skin, side: usize) -> Vec<f32> {
        let (w, h, data) = pixels(skin);
        assert!(w >= side && h >= side, "{skin:?} is under {side} px");
        let mut sums = vec![0f32; side * side * 3];
        let mut counts = vec![0f32; side * side];
        let clay = [0xD8, 0xC3, 0xA5].map(crate::to_linear);
        for y in 0..h {
            for x in 0..w {
                let cell = (y * side / h) * side + x * side / w;
                let alpha = f32::from(data[(y * w + x) * 4 + 3]) / 255.0;
                for c in 0..3 {
                    let color = crate::to_linear(data[(y * w + x) * 4 + c]);
                    sums[cell * 3 + c] += alpha * color + (1.0 - alpha) * clay[c];
                }
                counts[cell] += 1.0;
            }
        }
        sums.iter()
            .enumerate()
            .map(|(i, s)| f32::from(crate::to_srgb(s / counts[i / 3])) / 255.0)
            .collect()
    }

    fn average(thumb: &[f32], cells: &[usize]) -> Color {
        let channel =
            |c: usize| cells.iter().map(|i| thumb[i * 3 + c]).sum::<f32>() / cells.len() as f32;
        Color::srgb(channel(0), channel(1), channel(2))
    }

    fn mean(skin: Skin) -> Color {
        let thumb = thumbnail(skin, THUMB);
        average(&thumb, &(0..THUMB * THUMB).collect::<Vec<usize>>())
    }

    fn tile_body(skin: Skin) -> Vec<[f32; 3]> {
        let (w, h, data) = pixels(skin);
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|(x, y)| {
                let x = (2.0 * *x as f32 / (w - 1) as f32 - 1.0).abs() / 0.82;
                let y = (2.0 * *y as f32 / (h - 1) as f32 - 1.0).abs() / 0.82;
                x <= 3f32.sqrt() / 2.0 && x / 3f32.sqrt() + y / 2.0 <= 0.5
            })
            .map(|(x, y)| {
                let i = (y * w + x) * 4;
                [0, 1, 2].map(|c| f32::from(data[i + c]) / 255.0)
            })
            .collect()
    }

    fn tile_mean(skin: Skin) -> Color {
        let body = tile_body(skin);
        let channel = |c: usize| body.iter().map(|pixel| pixel[c]).sum::<f32>() / body.len() as f32;
        Color::srgb(channel(0), channel(1), channel(2))
    }

    struct Pressed {
        face: Color,
        field: Color,
    }

    fn pressed(skin: Skin) -> Pressed {
        let side = SYMBOL_PX as usize;
        let thumb = thumbnail(skin, side);
        let apart = |i: usize, c: Color| {
            let c = c.to_srgba();
            (thumb[i * 3] - c.red).powi(2)
                + (thumb[i * 3 + 1] - c.green).powi(2)
                + (thumb[i * 3 + 2] - c.blue).powi(2)
        };
        let rim: Vec<usize> = (0..side * side)
            .filter(|i| {
                [i % side, i / side]
                    .iter()
                    .any(|c| *c == 0 || *c == side - 1)
            })
            .collect();
        let mut field = average(&thumb, &rim);
        let farthest = (0..side * side)
            .max_by(|a, b| apart(*a, field).total_cmp(&apart(*b, field)))
            .expect("a thumbnail has cells");
        let mut face = average(&thumb, &[farthest]);
        for _ in 0..SPLIT_ROUNDS {
            let (letter, ground): (Vec<usize>, Vec<usize>) =
                (0..side * side).partition(|i| apart(*i, face) < apart(*i, field));
            if letter.is_empty() || ground.is_empty() {
                break;
            }
            face = average(&thumb, &letter);
            field = average(&thumb, &ground);
        }
        Pressed { face, field }
    }

    fn differences<M>(a: &Look<M>, b: &Look<M>) -> [bool; 4] {
        let [hue, value] = hue_and_value_differ(a.glaze.color(), b.glaze.color());
        let x = thumbnail(a.skin, THUMB);
        let y = thumbnail(b.skin, THUMB);
        let texture = x.iter().zip(y).map(|(x, y)| (x - y).abs()).sum::<f32>() / x.len() as f32
            >= TILES_APART;
        [hue, value, a.shape != b.shape, texture]
    }

    fn distinct<M>(a: &Look<M>, b: &Look<M>) -> bool {
        differences(a, b).iter().filter(|d| **d).count() >= 2
    }

    fn pairwise<M>(class: &str, looks: &[(String, Look<M>)]) {
        for (i, (a, x)) in looks.iter().enumerate() {
            for (b, y) in &looks[i + 1..] {
                assert_ne!(x.skin, y.skin, "{class}: {a} and {b} wear the same texture");
                assert!(
                    distinct(x, y),
                    "{class}: {a} and {b} differ in fewer than two of hue, value, shape, texture: {:?}",
                    differences(x, y)
                );
            }
        }
    }

    fn named<T: Copy + std::fmt::Debug, M>(
        kinds: impl IntoIterator<Item = T>,
        look: fn(T) -> Look<M>,
    ) -> Vec<(String, Look<M>)> {
        kinds
            .into_iter()
            .map(|k| (format!("{k:?}"), look(k)))
            .collect()
    }

    #[test]
    fn every_atom_is_distinct() {
        pairwise("atoms", &named(AtomKind::ALL, atom));
    }

    #[test]
    fn every_bond_is_distinct() {
        pairwise("bonds", &named(BondKind::ALL, bond));
    }

    #[test]
    fn every_glyph_is_distinct() {
        let glyphs = PALETTE.iter().filter_map(|item| match item {
            Item::Glyph(kind) => Some(*kind),
            Item::Arm => None,
        });
        pairwise("glyphs", &named(glyphs, |kind| machine(Item::Glyph(kind))));
    }

    #[test]
    fn every_machine_is_distinct() {
        pairwise("machines", &named(PALETTE, machine));
    }

    #[test]
    fn every_glyph_kind_is_on_the_palette() {
        for kind in GlyphKind::ALL {
            assert!(PALETTE.contains(&Item::Glyph(kind)));
        }
    }

    fn wears(what: impl std::fmt::Display, color: Color, glaze: Glaze) {
        let [hue, _] = hue_and_value_differ(color, glaze.color());
        let shaded = (color.luminance() - glaze.color().luminance()).abs();
        assert!(
            !hue && shaded <= SHADING,
            "{what} is {:?}, not its {glaze:?} glaze",
            Hsva::from(color)
        );
    }

    fn skin_wears(skin: Skin, glaze: Glaze) {
        wears(format!("{skin:?} on average"), mean(skin), glaze);
    }

    #[test]
    fn every_skin_wears_its_glaze() {
        for kind in AtomKind::ALL {
            skin_wears(atom(kind).skin, atom(kind).glaze);
        }
        for kind in BondKind::ALL {
            skin_wears(bond(kind).skin, bond(kind).glaze);
        }
        for tile in TILES {
            wears(
                format!("{tile:?} inside its grout"),
                tile_mean(tile),
                Glaze::Clay,
            );
        }
    }

    #[test]
    fn tiles_are_one_clay_family_and_every_batch_differs() {
        let clay = Glaze::Clay.color().to_srgba();
        for (i, a) in TILES.iter().enumerate() {
            let color = tile_mean(*a).to_srgba();
            let gap = ((color.red - clay.red).powi(2)
                + (color.green - clay.green).powi(2)
                + (color.blue - clay.blue).powi(2))
            .sqrt();
            assert!(gap <= 0.26, "{a:?} leaves the clay family by {gap:.3}");
            let a_body = tile_body(*a);
            for b in &TILES[i + 1..] {
                assert!(a.png != b.png, "{a:?} and {b:?} are the same batch");
                let apart = a_body
                    .iter()
                    .zip(tile_body(*b))
                    .flat_map(|(a, b)| a.iter().zip(b).map(|(a, b)| (a - b).abs()))
                    .sum::<f32>()
                    / (a_body.len() * 3) as f32;
                assert!(
                    apart >= 0.01,
                    "{a:?} and {b:?} lack character at {apart:.3}"
                );
            }
        }
    }

    #[test]
    fn every_machine_sprite_has_visible_art_and_real_transparency() {
        for item in PALETTE {
            let skin = machine(item).skin;
            let (_, _, data) = pixels(skin);
            let visible = data.chunks_exact(4).filter(|pixel| pixel[3] > 230).count();
            let clear = data.chunks_exact(4).filter(|pixel| pixel[3] < 25).count();
            let area = data.len() / 4;
            assert!(visible > area / 20, "{skin:?} has no visible machine");
            assert!(clear > area / 20, "{skin:?} has no transparent surround");
        }
    }

    #[test]
    fn every_atom_bond_and_glyph_references_distinct_art() {
        let semantic: Vec<Skin> = AtomKind::ALL
            .into_iter()
            .map(|kind| atom(kind).skin)
            .chain(BondKind::ALL.into_iter().map(|kind| bond(kind).skin))
            .chain(
                GlyphKind::ALL
                    .into_iter()
                    .map(|kind| machine(Item::Glyph(kind)).skin),
            )
            .collect();
        for (i, a) in semantic.iter().enumerate() {
            for b in &semantic[i + 1..] {
                assert_ne!(a, b, "{a:?} and {b:?} reference one texture");
                assert!(a.png != b.png, "{a:?} and {b:?} contain one image");
            }
        }
    }

    #[test]
    fn every_symbol_wears_its_token_at_tape_size() {
        for key in KEYS {
            let Pressed { face, field } = pressed(key.symbol);
            wears(format!("{:?} letter", key.symbol), face, key.token.face);
            wears(format!("{:?} field", key.symbol), field, key.token.field);
        }
    }

    #[test]
    fn every_symbol_keeps_its_letter_edge_at_tape_size() {
        let dissolved: Vec<String> = KEYS
            .iter()
            .filter_map(|key| {
                let Pressed { face, field } = pressed(key.symbol);
                let [_, apart] = hue_and_value_differ(face, field);
                let gap = (face.luminance() - field.luminance()).abs();
                (!apart).then(|| format!("{:?} letter is {gap:.3} from its field", key.symbol))
            })
            .collect();
        assert!(
            dissolved.is_empty(),
            "letters under {VALUE_APART} in value at {SYMBOL_PX} px: {dissolved:#?}"
        );
    }

    #[test]
    fn every_symbol_is_told_apart_at_tape_size() {
        let tokens: Vec<Pressed> = KEYS.iter().map(|k| pressed(k.symbol)).collect();
        let mut alike = Vec::new();
        for (i, a) in KEYS.iter().enumerate() {
            for (j, b) in KEYS.iter().enumerate().skip(i + 1) {
                let letters = hue_and_value_differ(tokens[i].face, tokens[j].face);
                let fields = hue_and_value_differ(tokens[i].field, tokens[j].field);
                if !letters.contains(&true) && !fields.contains(&true) {
                    alike.push(format!(
                        "{:?} and {:?}: letters {:?} {:?}, fields {:?} {:?}",
                        a.symbol,
                        b.symbol,
                        Hsva::from(tokens[i].face),
                        Hsva::from(tokens[j].face),
                        Hsva::from(tokens[i].field),
                        Hsva::from(tokens[j].field)
                    ));
                }
            }
        }
        assert!(
            alike.is_empty(),
            "symbols alike in hue and value at {SYMBOL_PX} px: {alike:#?}"
        );
    }

    #[test]
    fn every_skin_is_fired_once() {
        let all: Vec<Skin> = skins().collect();
        for (i, a) in all.iter().enumerate() {
            assert!(!all[i + 1..].contains(a), "{a:?} is listed twice");
        }
        assert_eq!(
            all.len(),
            AtomKind::ALL.len() + BondKind::ALL.len() + PALETTE.len() + TILES.len() + KEYS.len()
        );
    }

    #[test]
    fn every_instruction_has_its_own_symbol() {
        for (i, a) in KEYS.iter().enumerate() {
            let (w, h, _) = pixels(a.symbol);
            assert_eq!((w, h), (512, 512), "{:?} is not 512 square", a.symbol);
            assert!(
                skins().any(|s| s == a.symbol),
                "{:?} is never fired",
                a.symbol
            );
            let mine = thumbnail(a.symbol, THUMB);
            for b in &KEYS[i + 1..] {
                assert_ne!(
                    a.symbol, b.symbol,
                    "{:?} and {:?} share a symbol",
                    a.instr, b.instr
                );
                let theirs = thumbnail(b.symbol, THUMB);
                let apart = mine
                    .iter()
                    .zip(&theirs)
                    .map(|(x, y)| (x - y).abs())
                    .sum::<f32>()
                    / mine.len() as f32;
                assert!(
                    apart >= TILES_APART,
                    "{:?} and {:?} look alike: {apart:.3} apart",
                    a.symbol,
                    b.symbol
                );
            }
        }
    }

    #[test]
    #[should_panic(expected = "does not decode")]
    fn a_symbol_that_is_not_a_png_panics_at_load() {
        Skin {
            name: "symbols/none",
            png: b"",
            alpha: false,
        }
        .decode();
    }

    #[test]
    fn a_cell_keeps_its_tile_and_a_patch_shows_every_tile() {
        let mut seen = vec![false; TILES.len()];
        for q in -6..6 {
            for r in -6..6 {
                let t = tile(Hex::new(q, r));
                assert_eq!(t, tile(Hex::new(q, r)));
                seen[TILES.iter().position(|s| *s == t.skin).unwrap()] = true;
            }
        }
        assert!(
            seen.iter().all(|s| *s),
            "a 12 by 12 patch misses a tile: {seen:?}"
        );
    }

    fn bead(glaze: Glaze) -> Look<()> {
        Look {
            glaze,
            skin: TILES[0],
            shape: Shape::Bead,
            marking: (),
        }
    }

    #[test]
    fn hue_alone_never_counts() {
        let (a, b) = (bead(Glaze::Terracotta), bead(Glaze::BlueGreen));
        assert_eq!(differences(&a, &b), [true, false, false, false]);
        assert!(!distinct(&a, &b));
    }

    #[test]
    fn ivory_has_no_hue() {
        assert_eq!(
            hue_and_value_differ(Glaze::Ivory.color(), Glaze::Plum.color()),
            [false, true]
        );
    }

    #[test]
    fn near_hues_do_not_count() {
        assert_eq!(
            hue_and_value_differ(Glaze::Brass.color(), Glaze::Terracotta.color()),
            [false, false]
        );
    }
}
