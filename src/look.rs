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
pub struct Skin {
    pub name: &'static str,
    png: &'static [u8],
}

impl PartialEq for Skin {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
    }
}

impl Eq for Skin {}

impl std::fmt::Debug for Skin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.png", self.name)
    }
}

macro_rules! skin {
    ($name:expr) => {
        Skin {
            name: $name,
            png: include_bytes!(concat!("../art/textures/", $name, ".png")),
        }
    };
}

macro_rules! tiles {
    ($($n:literal),*) => {
        [$(skin!(concat!("tile-", $n))),*]
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
        .unwrap_or_else(|e| panic!("art/textures/{}.png does not decode: {e}", self.name))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tile {
    pub skin: Skin,
    pub turn: usize,
}

pub fn tile(h: Hex) -> Tile {
    let x = h.scramble();
    Tile {
        skin: TILES[(x % TILES.len() as u32) as usize],
        turn: ((x >> 8) % 6) as usize,
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
pub enum AtomMark {
    Highlight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineMark {
    Hand(Glaze),
    Dot,
    Spokes(usize),
    Cup,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Look<M> {
    pub glaze: Glaze,
    pub skin: Skin,
    pub shape: Shape,
    pub marking: M,
}

pub fn atom(kind: AtomKind) -> Look<AtomMark> {
    match kind {
        AtomKind::Base => Look {
            glaze: Glaze::BlueGreen,
            skin: skin!("atom-base"),
            shape: Shape::Bead,
            marking: AtomMark::Highlight,
        },
    }
}

pub fn bond(kind: BondKind) -> Look<()> {
    let (glaze, skin, bars) = match kind {
        BondKind::Single => (Glaze::Brass, skin!("bond-single"), 1),
        BondKind::Double => (Glaze::Plum, skin!("bond-double"), 2),
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
                skin: skin!("arm"),
                shape: Shape::Radial,
                marking: MachineMark::Hand(Glaze::Terracotta),
            };
        }
        Item::Glyph(kind) => kind,
    };
    let (glaze, skin, marking) = match kind {
        GlyphKind::Source => (Glaze::BlueGreen, skin!("source"), MachineMark::Dot),
        GlyphKind::Bonder => (Glaze::Terracotta, skin!("bonder"), MachineMark::Spokes(1)),
        GlyphKind::SecondBond => (Glaze::Plum, skin!("second-bond"), MachineMark::Spokes(2)),
        GlyphKind::Output => (Glaze::Ivory, skin!("output"), MachineMark::Cup),
    };
    Look {
        glaze,
        skin,
        shape: Shape::Cells(kind.rule().slots.len()),
        marking,
    }
}

pub fn skins() -> impl Iterator<Item = Skin> {
    AtomKind::ALL
        .into_iter()
        .map(|k| atom(k).skin)
        .chain(BondKind::ALL.into_iter().map(|k| bond(k).skin))
        .chain(
            crate::PALETTE
                .into_iter()
                .map(|(item, _)| machine(item).skin),
        )
        .chain(TILES)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::PALETTE;
    use bevy::color::{Hsva, Luminance};

    const HUE_APART: f32 = 40.0;
    const CHROMA_FLOOR: f32 = 0.15;
    const VALUE_APART: f32 = 0.15;
    const THUMB: usize = 8;
    const TILES_APART: f32 = 0.023;
    const SHADING: f32 = 2.0 * VALUE_APART;

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

    fn thumbnail(skin: Skin) -> Vec<f32> {
        let (w, h, data) = pixels(skin);
        let mut sums = vec![0f32; THUMB * THUMB * 3];
        let mut counts = vec![0f32; THUMB * THUMB];
        for y in 0..h {
            for x in 0..w {
                let cell = (y * THUMB / h) * THUMB + x * THUMB / w;
                for c in 0..3 {
                    sums[cell * 3 + c] += f32::from(data[(y * w + x) * 4 + c]) / 255.0;
                }
                counts[cell] += 1.0;
            }
        }
        sums.iter()
            .enumerate()
            .map(|(i, s)| s / counts[i / 3])
            .collect()
    }

    fn mean(skin: Skin) -> Color {
        let thumb = thumbnail(skin);
        let channel =
            |c: usize| thumb.iter().skip(c).step_by(3).sum::<f32>() / (THUMB * THUMB) as f32;
        Color::srgb(channel(0), channel(1), channel(2))
    }

    fn differences<M: PartialEq>(a: &Look<M>, b: &Look<M>) -> [bool; 4] {
        let [hue, value] = hue_and_value_differ(a.glaze.color(), b.glaze.color());
        [hue, value, a.shape != b.shape, a.marking != b.marking]
    }

    fn distinct<M: PartialEq>(a: &Look<M>, b: &Look<M>) -> bool {
        differences(a, b).iter().filter(|d| **d).count() >= 2
    }

    fn pairwise<M: PartialEq>(class: &str, looks: &[(String, Look<M>)]) {
        for (i, (a, x)) in looks.iter().enumerate() {
            for (b, y) in &looks[i + 1..] {
                assert_ne!(x.skin, y.skin, "{class}: {a} and {b} wear the same texture");
                assert!(
                    distinct(x, y),
                    "{class}: {a} and {b} differ in fewer than two of hue, value, shape, marking: {:?}",
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
        let glyphs = PALETTE.iter().filter_map(|(item, _)| match item {
            Item::Glyph(kind) => Some(*kind),
            Item::Arm => None,
        });
        pairwise("glyphs", &named(glyphs, |kind| machine(Item::Glyph(kind))));
    }

    #[test]
    fn every_machine_is_distinct() {
        pairwise("machines", &named(PALETTE.map(|(item, _)| item), machine));
    }

    #[test]
    fn every_glyph_kind_is_on_the_palette() {
        for kind in GlyphKind::ALL {
            assert!(PALETTE.iter().any(|(item, _)| *item == Item::Glyph(kind)));
        }
    }

    fn wears(skin: Skin, glaze: Glaze) {
        let mean = mean(skin);
        let [hue, _] = hue_and_value_differ(mean, glaze.color());
        let shaded = (mean.luminance() - glaze.color().luminance()).abs();
        assert!(
            !hue && shaded <= SHADING,
            "{skin:?} averages {:?}, not its {glaze:?} glaze",
            Hsva::from(mean)
        );
    }

    #[test]
    fn every_skin_wears_its_glaze() {
        for kind in AtomKind::ALL {
            wears(atom(kind).skin, atom(kind).glaze);
        }
        for kind in BondKind::ALL {
            wears(bond(kind).skin, bond(kind).glaze);
        }
        for (item, _) in PALETTE {
            wears(machine(item).skin, machine(item).glaze);
        }
        for tile in TILES {
            wears(tile, Glaze::Clay);
        }
    }

    #[test]
    fn every_skin_is_fired_once() {
        let all: Vec<Skin> = skins().collect();
        for (i, a) in all.iter().enumerate() {
            assert!(!all[i + 1..].contains(a), "{a:?} is listed twice");
        }
        assert_eq!(
            all.len(),
            AtomKind::ALL.len() + BondKind::ALL.len() + PALETTE.len() + TILES.len()
        );
    }

    #[test]
    fn tiles_vary() {
        let thumbs: Vec<Vec<f32>> = TILES.iter().map(|t| thumbnail(*t)).collect();
        let mut alike = Vec::new();
        for (i, a) in thumbs.iter().enumerate() {
            for (j, b) in thumbs.iter().enumerate().skip(i + 1) {
                let apart =
                    a.iter().zip(b).map(|(x, y)| (x - y).abs()).sum::<f32>() / a.len() as f32;
                if apart < TILES_APART {
                    alike.push(format!(
                        "{:?} and {:?} are {apart:.3} apart",
                        TILES[i], TILES[j]
                    ));
                }
            }
        }
        assert!(
            alike.is_empty(),
            "tiles alike under {TILES_APART}: {alike:#?}"
        );
    }

    #[test]
    fn a_cell_keeps_its_tile_and_a_patch_shows_every_tile() {
        let mut seen = vec![false; TILES.len()];
        let mut turns = vec![false; 6];
        for q in -6..6 {
            for r in -6..6 {
                let t = tile(Hex::new(q, r));
                assert_eq!(t, tile(Hex::new(q, r)));
                seen[TILES.iter().position(|s| *s == t.skin).unwrap()] = true;
                turns[t.turn] = true;
            }
        }
        assert!(
            seen.iter().all(|s| *s),
            "a 12 by 12 patch misses a tile: {seen:?}"
        );
        assert!(
            turns.iter().all(|s| *s),
            "a 12 by 12 patch misses a turn: {turns:?}"
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
