use crate::sim::Machine;
use crate::sim::{Arm, AtomKind, BondKind, DIRS, GlyphKind, Hex, ORIGIN, Slot, Tier};
use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, Image, ImageSampler, ImageType};
use bevy::math::{Vec2, Vec3};
use bevy::prelude::Color;
use std::ops::RangeInclusive;

pub const HEX: f32 = 20.0;
const MARGIN: f32 = 1.0;
const FACE: f32 = 458.0 / 512.0;
const STROKE: f32 = 34.0 / 512.0;

pub fn ring() -> RangeInclusive<f32> {
    let half = STROKE / 3f32.sqrt();
    FACE - half..=FACE + half
}

pub fn hex_norm(x: f32, y: f32) -> f32 {
    let (x, y) = (x.abs(), y.abs());
    f32::max(2.0 * x / 3f32.sqrt(), x / 3f32.sqrt() + y)
}

fn hex_distance(w: usize, h: usize, x: usize, y: usize) -> f32 {
    hex_norm(
        2.0 * x as f32 / (w - 1) as f32 - 1.0,
        2.0 * y as f32 / (h - 1) as f32 - 1.0,
    )
}

pub fn grout(tile: &mut Image, template: &Image) {
    let (w, h) = (tile.width() as usize, tile.height() as usize);
    assert_eq!(
        (template.width() as usize, template.height() as usize),
        (w, h),
        "{GROUT:?} is not the size of the tile it grouts"
    );
    let ring = template
        .data
        .as_ref()
        .expect("a decoded image carries its pixels");
    let data = tile
        .data
        .as_mut()
        .expect("a decoded image carries its pixels");
    assert_eq!(data.len(), w * h * 4, "a tile is rgba8");
    assert_eq!(ring.len(), w * h * 4, "{GROUT:?} is rgba8");
    let inner = *self::ring().start();
    for y in 0..h {
        for x in 0..w {
            if hex_distance(w, h, x, y) >= inner {
                let i = (y * w + x) * 4;
                data[i..i + 4].copy_from_slice(&ring[i..i + 4]);
            }
        }
    }
}
pub fn light() -> Vec3 {
    Vec3::new(-1.0, 1.0, 1.4).normalize()
}

pub const AMBIENT: f32 = 0.25;

pub fn px(h: Hex) -> Vec2 {
    let q = h.q as f32;
    let r = h.r as f32;
    Vec2::new(HEX * 3f32.sqrt() * (q + r / 2.0), HEX * 1.5 * r)
}

pub fn turn(dir: usize) -> f32 {
    px(DIRS[dir % 6]).to_angle()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Seat(Slot),
    Pivot,
    Hand,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub at: Hex,
    pub role: Role,
}

pub fn footprint(item: Machine) -> Vec<Cell> {
    match item {
        Machine::Arm => {
            let [pivot, hand] = Arm::new(ORIGIN, 0, Vec::new()).cells();
            vec![
                Cell {
                    at: pivot,
                    role: Role::Pivot,
                },
                Cell {
                    at: hand,
                    role: Role::Hand,
                },
            ]
        }
        Machine::Glyph(kind) => kind
            .rule()
            .slots
            .iter()
            .map(|slot| Cell {
                at: slot.at,
                role: Role::Seat(*slot),
            })
            .collect(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    pub centre: Vec2,
    pub side: f32,
}

pub fn quad(item: Machine) -> Quad {
    let at: Vec<Vec2> = footprint(item).iter().map(|c| px(c.at)).collect();
    let centre = at.iter().sum::<Vec2>() / at.len() as f32;
    let radius = at.iter().map(|p| p.distance(centre)).fold(0.0, f32::max);
    Quad {
        centre,
        side: 2.0 * (radius + MARGIN * HEX),
    }
}

impl Quad {
    pub fn size(self) -> Vec2 {
        Vec2::splat(self.side)
    }
}

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

    pub fn rgb(self) -> [f32; 3] {
        let c = self.color().to_srgba();
        [c.red, c.green, c.blue]
    }
}

#[derive(Clone, Copy)]
pub struct Token {
    pub face: Glaze,
    pub field: Glaze,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    Plain,
    Grouted,
    Sprite,
    Relief,
}

#[derive(Clone, Copy)]
pub struct Skin {
    pub name: &'static str,
    pub(crate) png: &'static [u8],
    pub finish: Finish,
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
            finish: Finish::Plain,
        }
    };
}
pub(crate) use skin;

macro_rules! finish {
    ($name:expr, $finish:expr) => {{
        let mut skin = skin!($name);
        skin.finish = $finish;
        skin
    }};
}

macro_rules! tiles {
    ($($n:literal),*) => {
        [$(finish!(concat!("textures/tile-", $n), Finish::Grouted)),*]
    };
}

pub const TILES: [Skin; 24] = tiles![
    "00", "01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11", "12", "13", "14", "15",
    "16", "17", "18", "19", "20", "21", "22", "23"
];

pub const MANUAL: Skin = skin!("overlay/manual");

pub const GROUT: Skin = skin!("textures/grout");

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
    RingedBead,
    FacetedBead,
    Bars(usize),
    Radial,
    Cells(usize),
    Converter(AtomKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineMark {
    Hand(Glaze, Skin),
    Sprite(Skin),
}

impl MachineMark {
    pub fn normal(self) -> Skin {
        match self {
            MachineMark::Hand(_, normal) | MachineMark::Sprite(normal) => normal,
        }
    }
}

macro_rules! machine {
    ($name:literal) => {
        (
            finish!(concat!("machines/", $name, "/albedo"), Finish::Sprite),
            finish!(concat!("machines/", $name, "/normal"), Finish::Relief),
        )
    };
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
        AtomKind::Amber => Look {
            glaze: Glaze::Amber,
            skin: skin!("textures/atom-amber"),
            shape: Shape::RingedBead,
            marking: (),
        },
        AtomKind::Plum => Look {
            glaze: Glaze::Plum,
            skin: skin!("textures/atom-plum"),
            shape: Shape::FacetedBead,
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

pub fn machine(item: Machine) -> Look<MachineMark> {
    let kind = match item {
        Machine::Arm => {
            let (skin, normal) = machine!("arm");
            return Look {
                glaze: Glaze::Brass,
                skin,
                shape: Shape::Radial,
                marking: MachineMark::Hand(Glaze::Terracotta, normal),
            };
        }
        Machine::Glyph(kind) => kind,
    };
    let (glaze, (skin, normal)) = match kind {
        GlyphKind::Source => (Glaze::BlueGreen, machine!("source")),
        GlyphKind::Bonder => (Glaze::Terracotta, machine!("bonder")),
        GlyphKind::SecondBond => (Glaze::Plum, machine!("second-bond")),
        GlyphKind::Reification => (Glaze::Amber, machine!("reification")),
        GlyphKind::Converter(AtomKind::Amber) => (Glaze::Amber, machine!("converter-amber")),
        GlyphKind::Converter(AtomKind::Plum) => (Glaze::Plum, machine!("converter-plum")),
        GlyphKind::Converter(AtomKind::Base) => panic!("the base atom has a source"),
        GlyphKind::Output(Tier::One) => (Glaze::Ivory, machine!("output-1")),
        GlyphKind::Output(Tier::Two) => (Glaze::Ivory, machine!("output-2")),
        GlyphKind::Output(Tier::Three) => (Glaze::Ivory, machine!("output-3")),
    };
    Look {
        glaze,
        skin,
        shape: match kind {
            GlyphKind::Converter(atom) => Shape::Converter(atom),
            _ => Shape::Cells(kind.rule().slots.len()),
        },
        marking: MachineMark::Sprite(normal),
    }
}

pub fn rig(item: Machine, part: &str) -> (Skin, Skin, Skin) {
    let name = match item {
        Machine::Arm => "arm",
        Machine::Glyph(GlyphKind::Source) => "source",
        Machine::Glyph(GlyphKind::Bonder) => "bonder",
        Machine::Glyph(GlyphKind::SecondBond) => "second-bond",
        Machine::Glyph(GlyphKind::Reification) => "reification",
        Machine::Glyph(GlyphKind::Converter(AtomKind::Amber)) => "converter-amber",
        Machine::Glyph(GlyphKind::Converter(AtomKind::Plum)) => "converter-plum",
        Machine::Glyph(GlyphKind::Converter(AtomKind::Base)) => {
            panic!("the base atom has a source")
        }
        Machine::Glyph(GlyphKind::Output(Tier::One)) => "output-1",
        Machine::Glyph(GlyphKind::Output(Tier::Two)) => "output-2",
        Machine::Glyph(GlyphKind::Output(Tier::Three)) => "output-3",
    };
    macro_rules! pair {
        ($machine:literal, $part:literal) => {{
            let albedo = finish!(
                concat!("machines/", $machine, "/parts/albedo-", $part),
                Finish::Sprite
            );
            let normal = finish!(
                concat!("machines/", $machine, "/parts/normal-", $part),
                Finish::Relief
            );
            let emissive = finish!(
                concat!("machines/", $machine, "/parts/emissive-", $part),
                Finish::Sprite
            );
            (albedo, normal, emissive)
        }};
    }
    match (name, part) {
        ("arm", "base") => pair!("arm", "base"),
        ("arm", "hand") => pair!("arm", "moving"),
        ("bonder", "base") => pair!("bonder", "base"),
        ("bonder", "bar") => pair!("bonder", "moving"),
        ("converter-amber", "base") => pair!("converter-amber", "base"),
        ("converter-amber", "ring") => pair!("converter-amber", "moving"),
        ("converter-plum", "base") => pair!("converter-plum", "base"),
        ("converter-plum", "ring") => pair!("converter-plum", "moving"),
        ("output-1", "base") => pair!("output-1", "base"),
        ("output-1", "rim") => pair!("output-1", "moving"),
        ("output-2", "base") => pair!("output-2", "base"),
        ("output-2", "rim") => pair!("output-2", "moving"),
        ("output-3", "base") => pair!("output-3", "base"),
        ("output-3", "rim") => pair!("output-3", "moving"),
        ("reification", "base") => pair!("reification", "base"),
        ("reification", "rim") => pair!("reification", "moving"),
        ("second-bond", "base") => pair!("second-bond", "base"),
        ("second-bond", "ring") => pair!("second-bond", "moving"),
        ("source", "base") => pair!("source", "base"),
        ("source", "rim") => pair!("source", "moving"),
        _ => panic!("machine {name} has no part {part}"),
    }
}

pub fn skins() -> impl Iterator<Item = Skin> {
    AtomKind::ALL
        .into_iter()
        .map(|k| atom(k).skin)
        .chain(BondKind::ALL.into_iter().map(|k| bond(k).skin))
        .chain(Machine::ALL.into_iter().map(|item| machine(item).skin))
        .chain(
            Machine::ALL
                .into_iter()
                .map(|item| machine(item).marking.normal()),
        )
        .chain(Machine::ALL.into_iter().flat_map(|item| {
            crate::rig::parts(item).iter().flat_map(move |part| {
                let (albedo, normal, emissive) = rig(item, &part.name);
                [albedo, normal, emissive]
            })
        }))
        .chain(TILES)
        .chain(crate::KEYS.iter().map(|k| k.symbol))
        .chain([MANUAL])
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{KEYS, SYMBOL_PX};
    use bevy::color::{Hsva, Luminance};

    const HUE_APART: f32 = 40.0;
    const CHROMA_FLOOR: f32 = 0.15;
    const VALUE_APART: f32 = 0.15;
    const THUMB: usize = 16;
    const TILES_APART: f32 = 0.023;
    const BODY_OF_FACE: f32 = 0.95;
    const GROUT_AT_MOST: f32 = 0.65;
    const TEMPLATE_GRAIN: f32 = 0.04;
    const RING_STEPS: usize = 6;
    const SHADING: f32 = 2.0 * VALUE_APART;
    const AMBER_MAX_CHROMA_LOSS: f32 = 0.15;
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

    fn painted(skin: Skin, side: usize) -> Vec<bool> {
        let (w, h, data) = pixels(skin);
        let mut painted = vec![false; side * side];
        for y in 0..h {
            for x in 0..w {
                if data[(y * w + x) * 4 + 3] > 0 {
                    painted[(y * side / h) * side + x * side / w] = true;
                }
            }
        }
        painted
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

    fn tile_band(skin: Skin, band: RangeInclusive<f32>) -> Vec<[f32; 3]> {
        let (w, h, data) = pixels(skin);
        (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .filter(|(x, y)| band.contains(&hex_distance(w, h, *x, *y)))
            .map(|(x, y)| {
                let i = (y * w + x) * 4;
                [0, 1, 2].map(|c| f32::from(data[i + c]) / 255.0)
            })
            .collect()
    }

    fn tile_body(skin: Skin) -> Vec<[f32; 3]> {
        tile_band(skin, 0.0..=ring().start() * BODY_OF_FACE)
    }

    fn mean_color(pixels: &[[f32; 3]]) -> Color {
        let channel =
            |c: usize| pixels.iter().map(|pixel| pixel[c]).sum::<f32>() / pixels.len() as f32;
        Color::srgb(channel(0), channel(1), channel(2))
    }

    fn tile_mean(skin: Skin) -> Color {
        mean_color(&tile_body(skin))
    }

    fn shade(pixels: &[[f32; 3]]) -> f32 {
        pixels.iter().flatten().sum::<f32>() / (3 * pixels.len()) as f32
    }

    pub fn grout_color() -> Color {
        mean_color(&tile_band(GROUT, ring()))
    }

    #[test]
    fn face_and_stroke_are_the_scaffold_hex_and_its_stroke() {
        let svg = include_str!("../art/textures/hex-scaffold.svg");
        let after = |key: &str| svg.split(key).nth(1).unwrap();
        let top: f32 = after("M512 ").split(' ').next().unwrap().parse().unwrap();
        let stroke: f32 = after("stroke-width=\"")
            .split('"')
            .next()
            .unwrap()
            .parse()
            .unwrap();
        assert_eq!(FACE, (512.0 - top) / 512.0);
        assert_eq!(STROKE, stroke / 512.0);
    }

    #[test]
    fn the_grout_template_is_clay_faced_and_granular_grout_on_every_edge_of_its_ring() {
        wears("the template face", tile_mean(GROUT), Glaze::Clay);
        let face = shade(&tile_body(GROUT));
        let (w, h, data) = pixels(GROUT);
        let ring = ring();
        let mut sum = [[0f32; RING_STEPS]; 6];
        let mut square = [[0f32; RING_STEPS]; 6];
        let mut count = [[0u32; RING_STEPS]; 6];
        for y in 0..h {
            for x in 0..w {
                let d = hex_distance(w, h, x, y);
                if !ring.contains(&d) {
                    continue;
                }
                let step = (d - ring.start()) / (ring.end() - ring.start()) * RING_STEPS as f32;
                let angle = (y as f32 - h as f32 / 2.0).atan2(x as f32 - w as f32 / 2.0);
                let edge =
                    ((angle / std::f32::consts::FRAC_PI_3 + 0.5).floor() as i32).rem_euclid(6);
                let i = (y * w + x) * 4;
                let s = data[i..i + 3].iter().map(|c| f32::from(*c)).sum::<f32>() / 765.0;
                let cell = (edge as usize, (step as usize).min(RING_STEPS - 1));
                sum[cell.0][cell.1] += s;
                square[cell.0][cell.1] += s * s;
                count[cell.0][cell.1] += 1;
            }
        }
        for edge in 0..6 {
            for step in 0..RING_STEPS {
                let n = count[edge][step] as f32;
                let mean = sum[edge][step] / n;
                let grain = (square[edge][step] / n - mean * mean).max(0.0).sqrt();
                assert!(
                    n > 0.0 && mean < face * GROUT_AT_MOST && grain >= TEMPLATE_GRAIN,
                    "{GROUT:?} on edge {edge} step {step} is {mean:.2} bright with {grain:.3} grain against a {face:.2} face"
                );
            }
        }
    }

    #[test]
    fn every_tile_keeps_its_clay_out_to_the_ring() {
        let strays: Vec<String> = TILES
            .iter()
            .filter_map(|tile| {
                let body = shade(&tile_body(*tile));
                let rim = shade(&tile_band(
                    *tile,
                    ring().start() * BODY_OF_FACE..=*ring().start(),
                ));
                (rim < body * GROUT_AT_MOST).then(|| {
                    format!("{tile:?} is {rim:.2} bright at its ring against a {body:.2} face")
                })
            })
            .collect();
        assert!(strays.is_empty(), "{}", strays.join("\n"));
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

    pub(crate) fn texture_apart<M>(a: &Look<M>, b: &Look<M>, side: usize) -> f32 {
        let x = thumbnail(a.skin, side);
        let y = thumbnail(b.skin, side);
        let painted: Vec<bool> = painted(a.skin, side)
            .iter()
            .zip(painted(b.skin, side))
            .map(|(p, q)| *p || q)
            .collect();
        let apart: f32 = x
            .iter()
            .zip(y)
            .enumerate()
            .filter(|(i, _)| painted[i / 3])
            .map(|(_, (x, y))| (x - y).abs())
            .sum();
        apart / (3.0 * painted.iter().filter(|p| **p).count() as f32)
    }

    fn differences<M>(a: &Look<M>, b: &Look<M>) -> [bool; 4] {
        let [hue, value] = hue_and_value_differ(a.glaze.color(), b.glaze.color());
        let texture = texture_apart(a, b, THUMB) >= TILES_APART;
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
        let glyphs = Machine::ALL.iter().filter_map(|item| match item {
            Machine::Glyph(kind) => Some(*kind),
            Machine::Arm => None,
        });
        pairwise(
            "glyphs",
            &named(glyphs, |kind| machine(Machine::Glyph(kind))),
        );
    }

    #[test]
    fn every_machine_is_distinct() {
        pairwise("machines", &named(Machine::ALL, machine));
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
    fn the_amber_atom_loses_at_most_point_one_five_chroma() {
        let look = atom(AtomKind::Amber);
        let chroma = |c: Hsva| c.saturation * c.value;
        let loss = chroma(Hsva::from(look.glaze.color())) - chroma(Hsva::from(mean(look.skin)));
        assert!(
            loss <= AMBER_MAX_CHROMA_LOSS,
            "{:?} loses {loss:.3} chroma from its {:?} glaze",
            look.skin,
            look.glaze
        );
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
        for item in Machine::ALL {
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
                    .map(|kind| machine(Machine::Glyph(kind)).skin),
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
            AtomKind::ALL.len()
                + BondKind::ALL.len()
                + 2 * Machine::ALL.len()
                + 3 * Machine::ALL
                    .into_iter()
                    .map(|machine| crate::rig::parts(machine).len())
                    .sum::<usize>()
                + TILES.len()
                + KEYS.len()
                + 1
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
            finish: Finish::Plain,
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
