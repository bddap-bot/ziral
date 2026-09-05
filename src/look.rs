use crate::Item;
use crate::sim::{AtomKind, BondKind, GlyphKind};
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
    pub shape: Shape,
    pub marking: M,
}

pub fn atom(kind: AtomKind) -> Look<AtomMark> {
    match kind {
        AtomKind::Base => Look {
            glaze: Glaze::BlueGreen,
            shape: Shape::Bead,
            marking: AtomMark::Highlight,
        },
    }
}

pub fn bond(kind: BondKind) -> Look<()> {
    let (glaze, bars) = match kind {
        BondKind::Single => (Glaze::Brass, 1),
        BondKind::Double => (Glaze::Plum, 2),
    };
    Look {
        glaze,
        shape: Shape::Bars(bars),
        marking: (),
    }
}

pub fn machine(item: Item) -> Look<MachineMark> {
    let kind = match item {
        Item::Arm => {
            return Look {
                glaze: Glaze::Brass,
                shape: Shape::Radial,
                marking: MachineMark::Hand(Glaze::Terracotta),
            };
        }
        Item::Glyph(kind) => kind,
    };
    let (glaze, marking) = match kind {
        GlyphKind::Source => (Glaze::BlueGreen, MachineMark::Dot),
        GlyphKind::Bonder => (Glaze::Terracotta, MachineMark::Spokes(1)),
        GlyphKind::SecondBond => (Glaze::Plum, MachineMark::Spokes(2)),
        GlyphKind::Output => (Glaze::Ivory, MachineMark::Cup),
    };
    Look {
        glaze,
        shape: Shape::Cells(kind.rule().slots.len()),
        marking,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PALETTE;
    use bevy::color::{Hsva, Luminance};

    const HUE_APART: f32 = 40.0;
    const CHROMA_FLOOR: f32 = 0.15;
    const VALUE_APART: f32 = 0.15;

    fn differences<M: PartialEq>(a: &Look<M>, b: &Look<M>) -> [bool; 4] {
        let (ca, cb) = (Hsva::from(a.glaze.color()), Hsva::from(b.glaze.color()));
        let chroma = |c: Hsva| c.saturation * c.value;
        let turn = (ca.hue - cb.hue).abs();
        let hue = chroma(ca) >= CHROMA_FLOOR
            && chroma(cb) >= CHROMA_FLOOR
            && turn.min(360.0 - turn) >= HUE_APART;
        let value =
            (a.glaze.color().luminance() - b.glaze.color().luminance()).abs() >= VALUE_APART;
        [hue, value, a.shape != b.shape, a.marking != b.marking]
    }

    fn distinct<M: PartialEq>(a: &Look<M>, b: &Look<M>) -> bool {
        differences(a, b).iter().filter(|d| **d).count() >= 2
    }

    fn pairwise<M: PartialEq>(class: &str, looks: &[(String, Look<M>)]) {
        for (i, (a, x)) in looks.iter().enumerate() {
            for (b, y) in &looks[i + 1..] {
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

    fn bead(glaze: Glaze) -> Look<()> {
        Look {
            glaze,
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
            differences(&bead(Glaze::Ivory), &bead(Glaze::Plum)),
            [false, true, false, false]
        );
    }

    #[test]
    fn near_hues_do_not_count() {
        assert_eq!(
            differences(&bead(Glaze::Brass), &bead(Glaze::Terracotta)),
            [false, false, false, false]
        );
    }
}
