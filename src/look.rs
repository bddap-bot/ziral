use crate::sim::Machine;
use crate::sim::{Arm, ArmLength, AtomKind, BondKind, DIRS, GlyphKind, Hex, ORIGIN, Slot, Tier};
use bevy::asset::RenderAssetUsages;
use bevy::image::{CompressedImageFormats, Image, ImageSampler, ImageType};
use bevy::math::Vec2;
use bevy::prelude::Color;
use std::ops::RangeInclusive;

pub const HEX: f32 = 20.0;
pub const HOLE: f32 = 0.55 * HEX;
pub const ATOM_RADIUS: f32 = 0.4 * HEX;
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

pub fn px(h: Hex) -> Vec2 {
    let q = h.q as f32;
    let r = h.r as f32;
    Vec2::new(HEX * 3f32.sqrt() * (q + r / 2.0), HEX * 1.5 * r)
}

pub fn hex_at(p: Vec2) -> Hex {
    let r = p.y / (HEX * 1.5);
    let q = p.x / (HEX * 3f32.sqrt()) - r / 2.0;
    let y = -q - r;
    let (mut rq, ry, mut rr) = (q.round(), y.round(), r.round());
    let (dq, dy, dr) = ((rq - q).abs(), (ry - y).abs(), (rr - r).abs());
    if dq > dy && dq > dr {
        rq = -ry - rr;
    } else if dr > dy {
        rr = -rq - ry;
    }
    Hex::new(rq as i32, rr as i32)
}

pub fn turn(dir: usize) -> f32 {
    px(DIRS[dir % 6]).to_angle()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Seat(Slot),
    Body,
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
        Machine::Portal => vec![Cell {
            at: ORIGIN,
            role: Role::Body,
        }],
        Machine::Arm(length) => Arm::new(length, ORIGIN, 0, Vec::new())
            .cells()
            .into_iter()
            .enumerate()
            .map(|(i, at)| Cell {
                at,
                role: if i == 0 {
                    Role::Pivot
                } else if i == length.cells() as usize {
                    Role::Hand
                } else {
                    Role::Body
                },
            })
            .collect(),
        Machine::Glyph(kind @ GlyphKind::Output(_)) => kind
            .cells()
            .into_iter()
            .map(|at| Cell {
                at,
                role: Role::Body,
            })
            .collect(),
        Machine::Glyph(kind @ GlyphKind::Converter(AtomKind::Cobalt)) => {
            let input = kind.rule().slots[0];
            let output = kind.product().expect("a converter has an output");
            kind.cells()
                .into_iter()
                .map(|at| Cell {
                    at,
                    role: if at == input.at {
                        Role::Seat(input)
                    } else if at == output {
                        Role::Seat(Slot {
                            at,
                            kind: Some(AtomKind::Cobalt),
                            consumed: false,
                            bonds: Some(0),
                        })
                    } else {
                        Role::Body
                    },
                })
                .collect()
        }
        Machine::Glyph(kind) => kind
            .rule()
            .slots
            .iter()
            .map(|slot| Cell {
                at: slot.at,
                role: Role::Seat(*slot),
            })
            .chain(kind.body().iter().map(|at| Cell {
                at: *at,
                role: Role::Body,
            }))
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
    Cobalt,
    Jade,
    Ivory,
}

impl Glaze {
    pub const ALL: [Glaze; 9] = [
        Glaze::Clay,
        Glaze::Brass,
        Glaze::Terracotta,
        Glaze::BlueGreen,
        Glaze::Amber,
        Glaze::Plum,
        Glaze::Cobalt,
        Glaze::Jade,
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
            Glaze::Cobalt => Color::srgb_u8(0x36, 0x57, 0xA7),
            Glaze::Jade => Color::srgb_u8(0x5F, 0xA8, 0x3A),
            Glaze::Ivory => Color::srgb_u8(0xF4, 0xED, 0xE4),
        }
    }

    pub fn rgb(self) -> [f32; 3] {
        let c = self.color().to_srgba();
        [c.red, c.green, c.blue]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finish {
    Plain,
    Grouted,
    Sprite,
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

pub const MANUAL: Skin = skin!("overlay/page");

pub const ETHEREAL: Skin = finish!("textures/ethereal", Finish::Grouted);

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
    KnobbedBead,
    SpiralBead,
    Bars(usize),
    Radial(ArmLength),
    Cells(usize),
    Converter(AtomKind),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineMark {
    Hand(Glaze),
    Sprite,
}

macro_rules! machine {
    ($name:literal) => {
        finish!(concat!("machines/", $name, "/albedo"), Finish::Sprite)
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
        AtomKind::Cobalt => Look {
            glaze: Glaze::Cobalt,
            skin: skin!("textures/atom-cobalt"),
            shape: Shape::KnobbedBead,
            marking: (),
        },
        AtomKind::Jade => Look {
            glaze: Glaze::Jade,
            skin: skin!("textures/atom-jade"),
            shape: Shape::SpiralBead,
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
        Machine::Portal => {
            let skin = machine!("portal");
            return Look {
                glaze: Glaze::Plum,
                skin,
                shape: Shape::Cells(1),
                marking: MachineMark::Sprite,
            };
        }
        Machine::Arm(length) => {
            let skin = match length {
                ArmLength::One => machine!("arm"),
                ArmLength::Two => machine!("arm-2"),
                ArmLength::Three => machine!("arm-3"),
            };
            return Look {
                glaze: Glaze::Brass,
                skin,
                shape: Shape::Radial(length),
                marking: MachineMark::Hand(Glaze::Terracotta),
            };
        }
        Machine::Glyph(kind) => kind,
    };
    let (glaze, skin) = match kind {
        GlyphKind::Source => (Glaze::BlueGreen, machine!("source")),
        GlyphKind::SourceTwo => (Glaze::BlueGreen, machine!("source-2")),
        GlyphKind::Bonder => (Glaze::Terracotta, machine!("bonder")),
        GlyphKind::SecondBond => (Glaze::Plum, machine!("second-bond")),
        GlyphKind::Reification => (Glaze::Amber, machine!("reification")),
        GlyphKind::Converter(AtomKind::Amber) => (Glaze::Amber, machine!("converter-amber")),
        GlyphKind::Resonator => (Glaze::Plum, machine!("resonator")),
        GlyphKind::Converter(AtomKind::Cobalt) => (Glaze::Cobalt, machine!("converter-cobalt")),
        GlyphKind::Fuse => (Glaze::Terracotta, machine!("fuse")),
        GlyphKind::Converter(AtomKind::Base | AtomKind::Plum | AtomKind::Jade) => {
            panic!("only amber and cobalt have converters")
        }
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
        marking: MachineMark::Sprite,
    }
}

pub fn name(item: Machine) -> &'static str {
    machine(item)
        .skin
        .name
        .split('/')
        .nth(1)
        .expect("a machine skin lives in art/machines/<name>/")
}

pub fn named(name: &str) -> Machine {
    Machine::ALL
        .into_iter()
        .find(|item| self::name(*item) == name)
        .unwrap_or_else(|| panic!("no machine is named {name}"))
}

pub fn skins() -> impl Iterator<Item = Skin> {
    AtomKind::ALL
        .into_iter()
        .map(|k| atom(k).skin)
        .chain(BondKind::ALL.into_iter().map(|k| bond(k).skin))
        .chain(Machine::ALL.into_iter().map(|item| machine(item).skin))
        .chain(TILES)
        .chain([ETHEREAL])
        .chain(crate::KEYS.iter().map(|k| k.symbol))
        .chain([MANUAL])
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::{KEYS, SYMBOL_PX};
    use bevy::color::Luminance;
    use bevy::input::keyboard::KeyCode;
    use quick_xml::events::Event;

    const TILES_APART: f32 = 0.023;
    const BODY_OF_FACE: f32 = 0.95;
    const GROUT_AT_MOST: f32 = 0.65;
    const TEMPLATE_GRAIN: f32 = 0.04;
    const RING_STEPS: usize = 6;
    const SYMBOL_CONTRAST: f32 = 4.5;

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

    fn sampled(skin: Skin, side: usize) -> Vec<[f32; 4]> {
        let image = crate::fire(skin.decode(), skin);
        let (width, height) = (image.width() as usize, image.height() as usize);
        let data = image.data.unwrap();
        let lod = (width as f32 / side as f32).log2();
        let lower = lod.floor() as usize;
        let upper = lod.ceil() as usize;
        let between = lod.fract();
        let offset = |level: usize| {
            (0..level)
                .map(|level| (width >> level) * (height >> level) * 4)
                .sum::<usize>()
        };
        let sample = |level: usize, out_x: usize, out_y: usize, channel: usize| {
            let (width, height) = (width >> level, height >> level);
            let source = |out: usize, extent: usize| {
                ((out as f32 + 0.5) * extent as f32 / side as f32 - 0.5)
                    .clamp(0.0, extent as f32 - 1.0)
            };
            let (x, y) = (source(out_x, width), source(out_y, height));
            let (left, top) = (x.floor() as usize, y.floor() as usize);
            let (right, bottom) = ((left + 1).min(width - 1), (top + 1).min(height - 1));
            let (across, down) = (x.fract(), y.fract());
            let at = |x: usize, y: usize| {
                let value = data[offset(level) + (y * width + x) * 4 + channel];
                if channel == 3 {
                    f32::from(value) / 255.0
                } else {
                    crate::to_linear(value)
                }
            };
            let top = at(left, top) * (1.0 - across) + at(right, top) * across;
            let bottom = at(left, bottom) * (1.0 - across) + at(right, bottom) * across;
            top * (1.0 - down) + bottom * down
        };
        (0..side)
            .flat_map(|y| (0..side).map(move |x| (x, y)))
            .map(|(x, y)| {
                [0, 1, 2, 3].map(|channel| {
                    let linear = sample(lower, x, y, channel) * (1.0 - between)
                        + sample(upper, x, y, channel) * between;
                    if channel == 3 {
                        linear
                    } else {
                        f32::from(crate::to_srgb(linear)) / 255.0
                    }
                })
            })
            .collect()
    }

    fn rendered(skin: Skin, side: usize) -> Vec<[f32; 3]> {
        sampled(skin, side)
            .into_iter()
            .map(|pixel| [pixel[0], pixel[1], pixel[2]])
            .collect()
    }

    fn closest(rendered: &[[f32; 3]], cells: &[usize], glaze: Glaze) -> ([f32; 3], f32) {
        let target = glaze.rgb();
        let cell = *cells
            .iter()
            .min_by(|a, b| {
                let distance = |cell: usize| {
                    (0..3)
                        .map(|channel| (rendered[cell][channel] - target[channel]).powi(2))
                        .sum::<f32>()
                };
                distance(**a).total_cmp(&distance(**b))
            })
            .unwrap();
        let pixel = rendered[cell];
        let distance = (0..3)
            .map(|channel| (pixel[channel] - target[channel]).powi(2))
            .sum();
        (pixel, distance)
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

    fn near(pixel: &[u8], glaze: Glaze) -> bool {
        let target = glaze.color().to_srgba();
        let target = [target.red, target.green, target.blue];
        pixel[..3]
            .iter()
            .zip(target)
            .all(|(actual, expected)| (f32::from(*actual) / 255.0 - expected).abs() < 0.08)
    }

    fn region_count(
        data: &[u8],
        x: RangeInclusive<usize>,
        y: RangeInclusive<usize>,
        glaze: Glaze,
    ) -> usize {
        y.flat_map(|y| x.clone().map(move |x| (y * 512 + x) * 4))
            .filter(|i| near(&data[*i..*i + 4], glaze))
            .count()
    }

    #[test]
    fn every_symbol_uses_the_shared_square_letter_and_mark_geometry() {
        for key in KEYS {
            let (_, _, data) = pixels(key.symbol);
            let alpha = |x: usize, y: usize| data[(y * 512 + x) * 4 + 3];
            assert!(
                alpha(40, 40) < 25,
                "{:?} loses its large top-left radius",
                key.symbol
            );
            assert!(
                alpha(472, 40) > 230,
                "{:?} loses its small top-right radius",
                key.symbol
            );
            assert!(
                alpha(40, 472) > 230,
                "{:?} loses its small bottom-left radius",
                key.symbol
            );
            assert!(
                alpha(472, 472) < 25,
                "{:?} loses its large bottom-right radius",
                key.symbol
            );
            assert!(
                near(&data[(300 * 512 + 280) * 4..], Glaze::Brass),
                "{:?} loses its dark-brass field",
                key.symbol
            );
            assert!(
                near(&data[(256 * 512 + 24) * 4..], Glaze::Plum),
                "{:?} loses its plum edge",
                key.symbol
            );
            assert!(
                region_count(&data, 20..=280, 220..=470, Glaze::Ivory) > 1_500,
                "{:?} has no ivory letter at bottom-left",
                key.symbol
            );
            assert!(
                region_count(&data, 285..=475, 45..=250, Glaze::Ivory) > 1_000,
                "{:?} has no ivory mark at top-right",
                key.symbol
            );
        }
    }

    #[test]
    fn every_instruction_symbol_is_transparent_at_all_four_shipped_size_corners() {
        let side = SYMBOL_PX as usize;
        let corners = [0, side - 1, side * (side - 1), side * side - 1];
        for key in KEYS {
            assert_eq!(
                key.symbol.finish,
                Finish::Sprite,
                "{:?} is fired without alpha blending",
                key.symbol
            );
            let pixels = sampled(key.symbol, side);
            for corner in corners {
                assert!(
                    pixels[corner][3] < 0.05,
                    "{:?} corner {corner} has alpha {:.3} at {side} px",
                    key.symbol,
                    pixels[corner][3]
                );
            }
        }
    }

    #[test]
    fn every_symbol_mark_clears_the_contrast_floor_at_shipped_size() {
        let side = SYMBOL_PX as usize;
        let ground_cells = (0..side * side).collect::<Vec<_>>();
        let mark_cells = (3..=12)
            .flat_map(|y| (14..=23).map(move |x| y * side + x))
            .collect::<Vec<_>>();
        for key in KEYS {
            let (_, _, source) = pixels(key.symbol);
            let present = [
                Glaze::Terracotta,
                Glaze::BlueGreen,
                Glaze::Amber,
                Glaze::Ivory,
            ]
            .into_iter()
            .filter(|glaze| region_count(&source, 285..=475, 45..=250, *glaze) > 100)
            .collect::<Vec<_>>();
            assert_eq!(
                present,
                [Glaze::Ivory],
                "{:?} does not use one ivory mark: {present:?}",
                key.symbol
            );
            let pixels = rendered(key.symbol, side);
            let mark = closest(&pixels, &mark_cells, Glaze::Ivory).0;
            let ground = closest(&pixels, &ground_cells, Glaze::Brass).0;
            let mark = Color::srgb(mark[0], mark[1], mark[2]).luminance();
            let ground = Color::srgb(ground[0], ground[1], ground[2]).luminance();
            let ratio = (mark.max(ground) + 0.05) / (mark.min(ground) + 0.05);
            assert!(
                ratio >= SYMBOL_CONTRAST,
                "{:?} mark is {ratio:.2}:1 against its ground at {side} px, below {SYMBOL_CONTRAST}:1",
                key.symbol
            );
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
            AtomKind::ALL.len()
                + BondKind::ALL.len()
                + Machine::ALL.len()
                + TILES.len()
                + KEYS.len()
                + 2
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
            let side = SYMBOL_PX as usize;
            let mine = thumbnail(a.symbol, side);
            for b in &KEYS[i + 1..] {
                assert_ne!(
                    a.symbol, b.symbol,
                    "{:?} and {:?} share a symbol",
                    a.instr, b.instr
                );
                let theirs = thumbnail(b.symbol, side);
                let region = |x: RangeInclusive<usize>, y: RangeInclusive<usize>| {
                    let cells: Vec<usize> = y
                        .flat_map(|y| x.clone().map(move |x| y * side + x))
                        .collect();
                    cells
                        .iter()
                        .flat_map(|i| (0..3).map(move |c| i * 3 + c))
                        .map(|i| (mine[i] - theirs[i]).abs())
                        .sum::<f32>()
                        / (cells.len() * 3) as f32
                };
                let letter = region(0..=14, 11..=25);
                let mark = region(14..=25, 1..=13);
                assert!(
                    letter >= TILES_APART || mark >= TILES_APART,
                    "{:?} and {:?} look alike at tape size: letter {letter:.3}, mark {mark:.3}",
                    a.symbol,
                    b.symbol
                );
            }
        }
    }

    #[test]
    fn every_symbol_letter_matches_its_binding_and_shift_state() {
        let source = include_str!("../art/symbols/source.svg");
        for key in KEYS {
            let name = key.symbol.name.strip_prefix("symbols/").unwrap();
            let mut reader = quick_xml::Reader::from_str(source);
            reader.config_mut().trim_text(true);
            let mut depth = 0;
            let mut letters = Vec::new();
            loop {
                match reader.read_event().unwrap() {
                    Event::Start(tag) if tag.name().as_ref() == b"g" => {
                        if depth > 0 {
                            depth += 1;
                        } else if tag.attributes().map(Result::unwrap).any(|attribute| {
                            attribute.key.as_ref() == b"id"
                                && attribute.value.as_ref() == name.as_bytes()
                        }) {
                            depth = 1;
                        }
                    }
                    Event::End(tag) if depth > 0 && tag.name().as_ref() == b"g" => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    Event::Text(text) if depth > 0 => {
                        letters.push(text.decode().unwrap().into_owned());
                    }
                    Event::Eof => panic!("{name} has no source group"),
                    _ => {}
                }
            }
            let binding = match key.code {
                KeyCode::KeyA => 'a',
                KeyCode::KeyC => 'c',
                KeyCode::KeyD => 'd',
                KeyCode::KeyE => 'e',
                KeyCode::KeyF => 'f',
                KeyCode::KeyQ => 'q',
                KeyCode::KeyR => 'r',
                KeyCode::KeyW => 'w',
                KeyCode::KeyX => 'x',
                KeyCode::KeyZ => 'z',
                code => panic!("{code:?} is not a letter binding"),
            };
            let expected = if key.shifted() {
                binding.to_ascii_uppercase()
            } else {
                binding
            };
            assert_eq!(
                letters,
                [expected.to_string()],
                "{name} disagrees with its binding"
            );
        }
    }

    #[test]
    fn every_painted_texture_has_the_prompt_that_painted_it_beside_it() {
        let art = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("art");
        let mut painted = vec![("overlay/page", "overlay/page"), (GROUT.name, GROUT.name)];
        painted.extend(AtomKind::ALL.map(|k| atom(k).skin.name).map(|n| (n, n)));
        painted.extend(BondKind::ALL.map(|k| bond(k).skin.name).map(|n| (n, n)));
        painted.extend(TILES.iter().map(|t| {
            let (family, index) = t.name.rsplit_once('-').expect("tile-NN");
            assert!(index.parse::<u32>().is_ok(), "{}", t.name);
            (t.name, family)
        }));
        for (png, name) in painted {
            let prompt = art.join(format!("{name}.prompt.txt"));
            assert!(art.join(format!("{png}.png")).exists(), "{png}.png");
            assert!(
                std::fs::read_to_string(&prompt).is_ok_and(|text| !text.trim().is_empty()),
                "{}: no prompt beside the texture",
                prompt.display()
            );
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
}
