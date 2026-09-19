use crate::look::{self, AMBIENT, Cell, Glaze, HEX, Quad, Role, px};
use crate::sim::Machine;
use crate::sim::Slot;
use bevy::math::{Vec2, Vec3};
use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const PX_PER_HEX: f32 = 256.0;
const BAND: f32 = 0.5;
const KEY: [f32; 3] = [0.0, 1.0, 0.0];
const SPILL: [f32; 2] = [0.1, 0.6];
const SEAT: f32 = 0.4;
const SEAT_RING: f32 = 0.32;
const SEAT_DOT: f32 = 0.12;
const SEAT_AROUND: [f32; 2] = [0.45, 0.65];
const SEAT_SEARCH: f32 = 0.5;
const SEAT_STEP: f32 = 0.02;
const SEAT_SPOKES: usize = 36;
const SPHERE: f32 = 0.75;
const SPHERE_ALBEDO: f32 = 0.6;
const SPHERE_LIT: f32 = 0.15;
const SPHERE_CAP: f32 = 0.5;
const PAD: f32 = 1.6;
const PAD_ALBEDO: f32 = 0.45;
const SAMPLES: usize = 4;
const CRITIC_PX: u32 = 512;
const RUBBER: [f32; 3] = [
    0x42 as f32 / 255.0,
    0x3B as f32 / 255.0,
    0x37 as f32 / 255.0,
];

#[derive(Serialize, Deserialize)]
struct Manifest {
    candidates: u32,
    style: Style,
    thresholds: Thresholds,
    machine: BTreeMap<String, Entry>,
    #[serde(default)]
    texture: BTreeMap<String, TextureEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Style {
    shared: String,
    arm: String,
    critic: String,
    facings: [Facing; 6],
    elevation: f32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
enum Facing {
    Right,
    UpperRight,
    UpperLeft,
    Left,
    LowerLeft,
    LowerRight,
}

impl Facing {
    const ALL: [Facing; 6] = [
        Facing::UpperLeft,
        Facing::UpperRight,
        Facing::Right,
        Facing::LowerRight,
        Facing::LowerLeft,
        Facing::Left,
    ];

    fn name(self) -> &'static str {
        match self {
            Facing::Right => "right",
            Facing::UpperRight => "upper-right",
            Facing::UpperLeft => "upper-left",
            Facing::Left => "left",
            Facing::LowerLeft => "lower-left",
            Facing::LowerRight => "lower-right",
        }
    }

    fn azimuth(self) -> f32 {
        match self {
            Facing::Right => 0.0,
            Facing::UpperRight => 60.0,
            Facing::UpperLeft => 120.0,
            Facing::Left => 180.0,
            Facing::LowerLeft => 240.0,
            Facing::LowerRight => 300.0,
        }
    }

    fn light(self, elevation: f32) -> Vec3 {
        let (azimuth, elevation) = (self.azimuth().to_radians(), elevation.to_radians());
        Vec3::new(
            azimuth.cos() * elevation.cos(),
            azimuth.sin() * elevation.cos(),
            elevation.sin(),
        )
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct TextureEntry {
    source: String,
    relit: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
struct Thresholds {
    outside: f32,
    seat: f32,
    palette: f32,
    off_centre: f32,
    critic: u8,
    sphere: f32,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct Entry {
    direction: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    references: Vec<String>,
    kept: Option<u32>,
    briefed: Option<String>,
    painted: Option<String>,
    relit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    motion: Option<crate::rig::Motion>,
    instrument: crate::sound::Instrument,
    emitter: crate::particles::Emitter,
    #[serde(skip_serializing_if = "Option::is_none")]
    rig_emitter: Option<crate::particles::Emitter>,
    #[serde(default)]
    parts: Vec<crate::rig::Part>,
}

struct Art {
    dir: PathBuf,
}

impl Art {
    fn shipped() -> Art {
        Art {
            dir: Path::new(env!("CARGO_MANIFEST_DIR")).join("art/machines"),
        }
    }

    fn manifest(&self) -> PathBuf {
        self.dir.join("manifest.toml")
    }

    fn read(&self) -> Manifest {
        let path = self.manifest();
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        toml::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    fn split(&self, manifest: &Manifest, names: &[String]) -> bool {
        let names: Vec<_> = names
            .iter()
            .filter(|name| {
                manifest
                    .machine
                    .get(name.as_str())
                    .is_some_and(|entry| !entry.parts.is_empty())
            })
            .collect();
        names.is_empty()
            || std::process::Command::new(self.dir.join("rig.sh"))
                .args(names)
                .status()
                .is_ok_and(|status| status.success())
    }

    fn write(&self, manifest: &Manifest) {
        let text = toml::to_string_pretty(manifest).expect("a manifest serialises");
        let path = self.manifest();
        let part = path.with_extension("toml.part");
        std::fs::write(&part, text).expect("the manifest is writable");
        std::fs::rename(&part, &path).expect("the manifest is replaceable");
    }

    fn machine(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }

    fn texture(&self, name: &str) -> PathBuf {
        self.dir.join("../textures/relit").join(name)
    }

    fn candidate(&self, name: &str, index: u32) -> PathBuf {
        self.machine(name)
            .join(format!("candidates/{name}-{index}.png"))
    }

    fn paint_sh(&self) -> PathBuf {
        self.dir.join("../paint.sh")
    }

    fn ask_sh(&self) -> PathBuf {
        self.dir.join("../ask.sh")
    }

    fn direct_sh(&self) -> PathBuf {
        self.dir.join("../direct.sh")
    }

    fn prompt(&self, name: &str) -> PathBuf {
        self.machine(name).join("prompt.txt")
    }

    fn judged(&self, name: &str, index: u32) -> PathBuf {
        self.machine(name)
            .join(format!("judged/{name}-{index}.png"))
    }

    fn round(&self, name: &str, round: usize) -> PathBuf {
        self.machine(name)
            .join(format!("candidates/round-{round}.txt"))
    }

    fn candidates(&self, name: &str) -> Vec<u32> {
        let mut indices: Vec<u32> = std::fs::read_dir(self.machine(name).join("candidates"))
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter_map(|e| {
                        let file = e.file_name();
                        let stem = file
                            .to_str()?
                            .strip_prefix(name)?
                            .strip_prefix('-')?
                            .strip_suffix(".png")?;
                        stem.parse()
                            .ok()
                            .filter(|i: &u32| *i > 0 && i.to_string() == stem)
                    })
                    .collect()
            })
            .unwrap_or_default();
        indices.sort_unstable();
        indices
    }
}

pub(crate) fn name(item: Machine) -> &'static str {
    look::machine(item)
        .skin
        .name
        .split('/')
        .nth(1)
        .expect("a machine skin lives in art/machines/<name>/")
}

fn item(name: &str) -> Machine {
    Machine::ALL
        .into_iter()
        .find(|item| self::name(*item) == name)
        .unwrap_or_else(|| panic!("no machine is named {name}"))
}

#[derive(Debug)]
struct Scaffold {
    cells: Vec<Cell>,
    quad: Quad,
    canvas: u32,
    mask: Vec<f32>,
}

const fn scale() -> f32 {
    PX_PER_HEX / HEX
}

fn canvas(item: Machine) -> u32 {
    (look::quad(item).side * scale() + 2.0 * band()).round() as u32
}

const fn band() -> f32 {
    BAND * HEX * scale()
}

impl Scaffold {
    fn of(item: Machine) -> Scaffold {
        let mut scaffold = Scaffold {
            cells: look::footprint(item),
            quad: look::quad(item),
            canvas: canvas(item),
            mask: Vec::new(),
        };
        scaffold.mask = scaffold.mask();
        scaffold
    }

    fn texture(side: u32) -> Scaffold {
        let mut scaffold = Scaffold {
            cells: Vec::new(),
            quad: Quad {
                centre: Vec2::ZERO,
                side: side as f32 / scale(),
            },
            canvas: side + 2 * band() as u32,
            mask: Vec::new(),
        };
        let (origin, side) = scaffold.crop();
        let n = scaffold.canvas as usize;
        scaffold.mask = vec![0.0; n * n];
        for y in origin..origin + side {
            for x in origin..origin + side {
                scaffold.mask[y as usize * n + x as usize] = 1.0;
            }
        }
        scaffold
    }

    fn mount(&self, image: &RgbaImage) -> RgbaImage {
        let (origin, side) = self.crop();
        assert_eq!((image.width(), image.height()), (side, side));
        RgbaImage::from_fn(self.canvas, self.canvas, |x, y| {
            if (origin..origin + side).contains(&x) && (origin..origin + side).contains(&y) {
                *image.get_pixel(x - origin, y - origin)
            } else {
                grey(PAD_ALBEDO)
            }
        })
    }

    fn cropped(&self, image: &RgbaImage) -> RgbaImage {
        let (origin, side) = self.crop();
        RgbaImage::from_fn(side, side, |x, y| *image.get_pixel(x + origin, y + origin))
    }

    fn pixel(&self, world: Vec2) -> Vec2 {
        let half = self.canvas as f32 / 2.0;
        let d = (world - self.quad.centre) * scale();
        Vec2::new(half + d.x, half - d.y)
    }

    fn world(&self, pixel: Vec2) -> Vec2 {
        let half = self.canvas as f32 / 2.0;
        self.quad.centre + Vec2::new(pixel.x - half, half - pixel.y) / scale()
    }

    fn crop(&self) -> (u32, u32) {
        let side = (self.quad.side * scale()).round() as u32;
        let origin = (self.canvas - side) / 2;
        assert_eq!(origin, band() as u32);
        (origin, side)
    }

    fn sphere(&self) -> (Vec2, f32) {
        let inset = band() / 2.0;
        (Vec2::splat(self.canvas as f32 - inset), inset * SPHERE)
    }

    fn in_cell(&self, world: Vec2, radius: f32) -> Option<&Cell> {
        self.cells.iter().find(|c| in_hex(world - px(c.at), radius))
    }

    fn covered(&self, world: Vec2) -> bool {
        self.in_cell(world, HEX).is_some()
    }

    fn marked(&self) -> impl Iterator<Item = &Cell> {
        self.cells.iter().filter(|cell| cell.role != Role::Body)
    }

    fn outside(&self, world: Vec2) -> f32 {
        if self.covered(world) {
            return 0.0;
        }
        self.cells
            .iter()
            .map(|c| hex_distance(world - px(c.at), HEX))
            .fold(f32::INFINITY, f32::min)
            / HEX
    }

    fn mask(&self) -> Vec<f32> {
        let n = self.canvas as usize;
        let mut mask = vec![0f32; n * n];
        for (i, m) in mask.iter_mut().enumerate() {
            let (x, y) = ((i % n) as f32, (i / n) as f32);
            let hits = (0..SAMPLES * SAMPLES)
                .filter(|s| {
                    let dx = ((s % SAMPLES) as f32 + 0.5) / SAMPLES as f32;
                    let dy = ((s / SAMPLES) as f32 + 0.5) / SAMPLES as f32;
                    self.covered(self.world(Vec2::new(x + dx, y + dy)))
                })
                .count();
            *m = hits as f32 / (SAMPLES * SAMPLES) as f32;
        }
        mask
    }

    fn mark(&self, cell: &Cell, world: Vec2) -> Option<Glaze> {
        let d = world - px(cell.at);
        let r = d.length() / HEX;
        let ring = (SEAT_RING..=SEAT).contains(&r);
        let dot = r <= SEAT_DOT;
        match cell.role {
            Role::Seat(Slot {
                consumed: false, ..
            }) => (ring || dot).then_some(Glaze::BlueGreen),
            Role::Seat(Slot { consumed: true, .. }) => (ring || dot).then_some(Glaze::Terracotta),
            Role::Body => None,
            Role::Pivot => (r <= SEAT).then_some(Glaze::Brass),
            Role::Hand => {
                let pivot = self
                    .cells
                    .iter()
                    .find(|c| c.role == Role::Pivot)
                    .map(|c| px(c.at))
                    .expect("a hand has its pivot");
                let open = d.angle_to(px(cell.at) - pivot).abs() < std::f32::consts::FRAC_PI_4;
                ((ring && !open) || dot).then_some(Glaze::Terracotta)
            }
        }
    }

    fn paint(&self, world: Vec2) -> Rgba<u8> {
        let glaze = self
            .in_cell(world, HEX)
            .and_then(|cell| self.mark(cell, world));
        let guide = if self.covered(world) && !self.cells.iter().any(|c| c.role == Role::Pivot) {
            [0.7; 3]
        } else {
            KEY
        };
        rgba(glaze.map_or(guide, Glaze::rgb), 1.0)
    }

    fn render(&self) -> RgbaImage {
        RgbaImage::from_fn(self.canvas, self.canvas, |x, y| {
            self.paint(self.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5)))
        })
    }

    fn alpha(&self, candidate: &RgbaImage) -> Vec<f32> {
        candidate.pixels().map(pixel_opacity).collect()
    }

    fn cut(&self, candidate: &RgbaImage) -> RgbaImage {
        let (origin, side) = self.crop();
        RgbaImage::from_fn(side, side, |x, y| {
            let p = candidate.get_pixel(x + origin, y + origin);
            rgba(unspill(rgb(p)), pixel_opacity(p))
        })
    }

    fn board(&self, candidate: &RgbaImage) -> RgbaImage {
        let side = self.quad.side.round() as u32;
        let sprite = shrink(&self.cut(candidate), side);
        let (tiles, grout) = tiles();
        let half = side as f32 / 2.0;
        let shipped = RgbaImage::from_fn(side, side, |x, y| {
            let world =
                self.quad.centre + Vec2::new(x as f32 + 0.5 - half, half - (y as f32 + 0.5));
            let h = crate::hex_at(world);
            let u = (world - px(h)) / HEX * *look::ring().end();
            let tile = if look::hex_norm(u.x, u.y) >= *look::ring().start() {
                grout
            } else {
                let skin = look::tile(h).skin;
                &tiles[look::TILES
                    .iter()
                    .position(|t| *t == skin)
                    .expect("a tile skin is one of TILES")]
            };
            let at = Vec2::new((u.x + 1.0) / 2.0, 1.0 - (u.y + 1.0) / 2.0) * tile.width() as f32;
            let ground = rgb(&bilinear(tile, at));
            let s = sprite.get_pixel(x, y);
            let a = f32::from(s[3]) / 255.0;
            let c = rgb(s);
            rgba([0, 1, 2].map(|i| ground[i] * (1.0 - a) + c[i] * a), 1.0)
        });
        magnify(&shipped, CRITIC_PX.div_ceil(side))
    }

    fn register(&self, candidate: &RgbaImage) -> Capture {
        let keyed = RgbaImage::from_fn(candidate.width(), candidate.height(), |x, y| {
            let pixel = candidate.get_pixel(x, y);
            let alpha = f32::from(pixel[3]) / 255.0;
            let color = rgb(pixel);
            rgba(
                [0, 1, 2].map(|i| color[i] * alpha + KEY[i] * (1.0 - alpha)),
                1.0,
            )
        });
        let candidate = &keyed;
        assert_eq!(
            (candidate.width(), candidate.height()),
            (self.canvas, self.canvas),
            "a candidate is painted over the whole scaffold"
        );
        let cells: Vec<Vec2> = self.marked().map(|c| px(c.at)).collect();
        let seats: Option<Vec<Vec2>> = self.marked().map(|c| self.seat(candidate, c)).collect();
        let Some(seats) = seats else {
            return Capture {
                image: candidate.clone(),
                off_centre: f32::INFINITY,
            };
        };
        if cells.is_empty() {
            let points: Vec<_> = candidate
                .enumerate_pixels()
                .filter(|(_, _, p)| pixel_opacity(p) > 0.0)
                .map(|(x, y, _)| self.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5)))
                .collect();
            let lo = points
                .iter()
                .copied()
                .fold(Vec2::splat(f32::INFINITY), Vec2::min);
            let hi = points
                .iter()
                .copied()
                .fold(Vec2::splat(f32::NEG_INFINITY), Vec2::max);
            let center = (lo + hi) / 2.0;
            let scale = points
                .iter()
                .map(|p| {
                    let p = (*p - center) / HEX;
                    look::hex_norm(p.x, p.y)
                })
                .fold(0.0, f32::max);
            let image = RgbaImage::from_fn(self.canvas, self.canvas, |x, y| {
                let world = self.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
                bilinear(candidate, self.pixel(center + world * scale))
            });
            return Capture {
                image,
                off_centre: 0.0,
            };
        }
        let n = cells.len() as f32;
        let (c0, s0) = (
            cells.iter().sum::<Vec2>() / n,
            seats.iter().sum::<Vec2>() / n,
        );
        let spread: f32 = cells.iter().map(|c| (*c - c0).length_squared()).sum();
        let k = if spread > 0.0 {
            cells
                .iter()
                .zip(&seats)
                .map(|(c, s)| (*c - c0).dot(*s - s0))
                .sum::<f32>()
                / spread
        } else {
            1.0
        };
        let onto = |w: Vec2| s0 + k * (w - c0);
        let image = RgbaImage::from_fn(self.canvas, self.canvas, |x, y| {
            let world = self.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
            bilinear(candidate, self.pixel(onto(world)))
        });
        let off_centre = self
            .marked()
            .map(|c| {
                self.seat(&image, c)
                    .map_or(f32::INFINITY, |s| s.distance(px(c.at)) / HEX)
            })
            .fold(0.0, f32::max);
        Capture { image, off_centre }
    }

    fn score(&self, capture: &Capture) -> Score {
        let candidate = &capture.image;
        let alpha = self.alpha(candidate);
        let (origin, side) = self.crop();
        let n = self.canvas as usize;
        let mut outside = 0f32;
        let mut inside = Mean::default();
        let mut hand = Mean::default();
        let mut body = Mean::default();
        for y in origin..origin + side {
            for x in origin..origin + side {
                let i = y as usize * n + x as usize;
                if alpha[i] > 0.0 {
                    let world = self.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
                    outside = outside.max(self.outside(world));
                }
                if self.mask[i] == 1.0 && alpha[i] == 1.0 {
                    let color = rgb(candidate.get_pixel(x, y));
                    inside.add_rgb(color);
                    let world = self.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
                    if self
                        .in_cell(world, HEX)
                        .is_some_and(|c| c.role == Role::Hand)
                    {
                        hand.add_rgb(color);
                    } else {
                        body.add_rgb(color);
                    }
                }
            }
        }
        let seat = self
            .marked()
            .map(|cell| self.seat_contrast(candidate, cell, self.seat_bounds(cell)))
            .fold(f32::INFINITY, f32::min);
        let glaze_distance = |mean: &Mean| {
            Glaze::ALL
                .iter()
                .map(|g| apart(mean.rgb(), g.rgb()))
                .fold(f32::INFINITY, f32::min)
        };
        let mut palette = if inside.n > 0.0 {
            glaze_distance(&inside)
        } else {
            f32::INFINITY
        };
        if hand.n > 0.0 {
            let rubber = apart(hand.rgb(), RUBBER);
            let body = if body.n > 0.0 {
                glaze_distance(&body)
            } else {
                0.0
            };
            palette = palette.min(rubber.max(body));
        }
        Score {
            outside,
            seat,
            palette,
            off_centre: capture.off_centre,
            judged: None,
        }
    }

    fn seat_bounds(&self, cell: &Cell) -> [u32; 4] {
        let centre = self.pixel(px(cell.at));
        let reach = Vec2::splat(SEAT_AROUND[1] * HEX * scale() + 2.0);
        let lower = (centre - reach).floor().max(Vec2::ZERO);
        let upper = (centre + reach).ceil().min(Vec2::splat(self.canvas as f32));
        [
            lower.x as u32,
            lower.y as u32,
            upper.x as u32,
            upper.y as u32,
        ]
    }

    fn seat_contrast(&self, candidate: &RgbaImage, cell: &Cell, [x0, y0, x1, y1]: [u32; 4]) -> f32 {
        let mut mark = Mean::default();
        let mut centre = Mean::default();
        let mut around = Mean::default();
        for y in y0..y1 {
            for x in x0..x1 {
                let p = candidate.get_pixel(x, y);
                let world = self.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
                let r = (world - px(cell.at)).length() / HEX;
                if r <= SEAT_DOT {
                    centre.add_rgb(rgb(p));
                } else if self.mark(cell, world).is_some() {
                    mark.add_rgb(rgb(p));
                } else if (SEAT_AROUND[0]..=SEAT_AROUND[1]).contains(&r) {
                    around.add_rgb(rgb(p));
                }
            }
        }
        apart(mark.rgb(), around.rgb()).max(apart(centre.rgb(), around.rgb()))
    }

    fn seat(&self, candidate: &RgbaImage, cell: &Cell) -> Option<Vec2> {
        let sample = |world: Vec2| {
            let p = self.pixel(world);
            let inside = p.min_element() >= 0.0 && p.max_element() < self.canvas as f32;
            let pixel = inside.then(|| candidate.get_pixel(p.x as u32, p.y as u32))?;
            (pixel_opacity(pixel) > 0.0).then(|| Vec3::from_array(rgb(pixel)))
        };
        let step = SEAT_STEP * HEX;
        let rings = ((SEAT_AROUND[1] - SEAT_DOT) / SEAT_STEP).ceil() as usize;
        let rim = |centre: Vec2| {
            (0..rings)
                .map(|k| {
                    let r = SEAT_DOT * HEX + step * k as f32;
                    let mut jump = Vec3::ZERO;
                    for s in 0..SEAT_SPOKES {
                        let ray =
                            Vec2::from_angle(std::f32::consts::TAU * s as f32 / SEAT_SPOKES as f32);
                        if let (Some(outer), Some(inner)) = (
                            sample(centre + ray * (r + step)),
                            sample(centre + ray * (r - step)),
                        ) {
                            jump += outer - inner;
                        }
                    }
                    jump.length() / SEAT_SPOKES as f32
                })
                .fold(0.0, f32::max)
        };
        let search = |around: Vec2, radius: f32, pitch: f32| {
            let reach = (radius / pitch).ceil() as i32;
            let mut offsets: Vec<Vec2> = (-reach..=reach)
                .flat_map(|i| (-reach..=reach).map(move |j| Vec2::new(i as f32, j as f32)))
                .map(|o| o * pitch)
                .filter(|o| o.length() <= radius)
                .collect();
            offsets.sort_by(|a, b| a.length().total_cmp(&b.length()));
            let mut best = (0.0, None);
            for offset in offsets {
                let edge = rim(around + offset);
                if edge > best.0 {
                    best = (edge, Some(around + offset));
                }
            }
            best.1
        };
        let coarse = search(px(cell.at), SEAT_SEARCH * HEX, 2.0 * step)?;
        search(coarse, 2.0 * step, step)
    }

    fn surface_normals(&self, candidate: &RgbaImage) -> RgbaImage {
        let height = RgbaImage::from_fn(self.canvas, self.canvas, |x, y| {
            let p = candidate.get_pixel(x, y);
            grey(opacity(rgb(p)) * f32::from(p[3]) / 255.0 * (0.85 + 0.15 * luminance(p)))
        });
        let height = image::imageops::blur(&height, 4.0);
        let sample = |x: u32, y: u32| luminance(height.get_pixel(x, y));
        RgbaImage::from_fn(self.canvas, self.canvas, |x, y| {
            let dx = sample((x + 1).min(self.canvas - 1), y) - sample(x.saturating_sub(1), y);
            let dy = sample(x, (y + 1).min(self.canvas - 1)) - sample(x, y.saturating_sub(1));
            encode(
                Vec3::new(-24.0 * dx, 24.0 * dy, 1.0).normalize(),
                opacity(rgb(candidate.get_pixel(x, y))) * f32::from(candidate.get_pixel(x, y)[3])
                    / 255.0,
            )
        })
    }

    fn render_light(&self, candidate: &RgbaImage, normal: &RgbaImage, light: Vec3) -> RgbaImage {
        let master = self.master(candidate);
        let (centre, radius) = self.sphere();
        RgbaImage::from_fn(self.canvas, self.canvas, |x, y| {
            let sphere = self.sphere_normal(x, y);
            let pad = (Vec2::new(x as f32 + 0.5, y as f32 + 0.5) - centre)
                .abs()
                .max_element()
                < radius * PAD;
            let n = sphere.unwrap_or_else(|| {
                if pad {
                    Vec3::Z
                } else {
                    decode(normal.get_pixel(x, y))
                }
            });
            let p = master.get_pixel(x, y);
            let strength = AMBIENT + (1.0 - AMBIENT) * n.dot(light).max(0.0);
            let alpha = if pad || sphere.is_some() {
                1.0
            } else {
                f32::from(normal.get_pixel(x, y)[3]) / 255.0
            };
            rgba(unspill(rgb(p)).map(|c| c * strength), alpha)
        })
    }

    fn sphere_normal(&self, x: u32, y: u32) -> Option<Vec3> {
        let (centre, radius) = self.sphere();
        ball(centre, radius, x, y)
    }

    fn master(&self, candidate: &RgbaImage) -> RgbaImage {
        let (centre, radius) = self.sphere();
        let mut out = candidate.clone();
        for (x, y, p) in out.enumerate_pixels_mut() {
            if self.sphere_normal(x, y).is_some() {
                *p = grey(SPHERE_ALBEDO);
            } else if (Vec2::new(x as f32 + 0.5, y as f32 + 0.5) - centre)
                .abs()
                .max_element()
                < radius * PAD
            {
                *p = grey(PAD_ALBEDO);
            }
        }
        out
    }

    fn light(&self, edit: &RgbaImage) -> Light {
        let pixels: Vec<(Vec3, f32)> = edit
            .enumerate_pixels()
            .filter_map(|(x, y, p)| Some((self.sphere_normal(x, y)?, luminance(p))))
            .collect();
        let mut lit: Vec<bool> = pixels.iter().map(|(n, _)| n.z > SPHERE_LIT).collect();
        let mut coef = [0f32; 4];
        for _ in 0..3 {
            coef = fit(pixels
                .iter()
                .zip(&lit)
                .filter(|(_, l)| **l)
                .map(|(p, _)| *p));
            let dir = Vec3::from_slice(&coef[..3]);
            let floor = coef[3] + 0.02 * (dir.x.abs() + dir.y.abs() + dir.z.abs());
            for (l, (n, _)) in lit.iter_mut().zip(&pixels) {
                *l = n.dot(dir) + coef[3] > floor;
            }
        }
        let dir = Vec3::from_slice(&coef[..3]);
        let ambient = coef[3] / (coef[3] + dir.length());
        Light {
            direction: dir.normalize(),
            ambient,
        }
    }

    fn calibration(&self, edits: &[RgbaImage]) -> Calibration {
        let lights: Vec<Light> = edits.iter().map(|e| self.light(e)).collect();
        let rows: Vec<[f32; 3]> = std::iter::once([0.0, 0.0, 1.0])
            .chain(lights.iter().map(Light::row))
            .collect();
        let solver = invert(gram(rows.iter().map(|r| (*r, 0.0))).0);
        let mut error = Mean::default();
        for (x, y, _) in edits[0].enumerate_pixels() {
            let Some(want) = self.sphere_normal(x, y).filter(|n| n.z > SPHERE_CAP) else {
                continue;
            };
            let values = std::iter::once(SPHERE_ALBEDO)
                .chain(edits.iter().map(|image| luminance(image.get_pixel(x, y))));
            let mut rhs = [0f64; 3];
            for (lum, row) in values.zip(&rows) {
                for (r, a) in rhs.iter_mut().zip(row) {
                    *r += f64::from(*a * lum);
                }
            }
            let [gx, gy, rho] = apply(&solver, &rhs);
            let tangent = if rho > 0.0 {
                (Vec2::new(gx as f32, gy as f32) / rho as f32).clamp_length_max(1.0)
            } else {
                Vec2::ZERO
            };
            let got = tangent.extend((1.0 - tangent.length_squared()).max(0.0).sqrt());
            error.add(got.dot(want).clamp(-1.0, 1.0).acos().to_degrees());
        }
        Calibration {
            lights,
            sphere: error.value(),
        }
    }
}

struct Capture {
    image: RgbaImage,
    off_centre: f32,
}

struct Calibration {
    lights: Vec<Light>,
    sphere: f32,
}

#[derive(Debug, Clone, Copy)]
struct Light {
    direction: Vec3,
    ambient: f32,
}

impl Light {
    fn row(&self) -> [f32; 3] {
        let d = self.direction * (1.0 - self.ambient);
        [d.x, d.y, self.ambient + d.z]
    }
}

fn ball(centre: Vec2, radius: f32, x: u32, y: u32) -> Option<Vec3> {
    let d = (Vec2::new(x as f32 + 0.5, y as f32 + 0.5) - centre) / radius;
    let d = Vec2::new(d.x, -d.y);
    let rr = d.length_squared();
    (rr < 1.0).then(|| Vec3::new(d.x, d.y, (1.0 - rr).sqrt()))
}

fn encode(v: Vec3, alpha: f32) -> Rgba<u8> {
    rgba(
        [(v.x + 1.0) / 2.0, (v.y + 1.0) / 2.0, (v.z + 1.0) / 2.0],
        alpha,
    )
}

fn decode(p: &Rgba<u8>) -> Vec3 {
    let c = rgb(p);
    Vec3::new(c[0] * 2.0 - 1.0, c[1] * 2.0 - 1.0, c[2] * 2.0 - 1.0)
}

fn gram<const N: usize>(rows: impl Iterator<Item = ([f32; N], f32)>) -> ([[f64; N]; N], [f64; N]) {
    let mut ata = [[0f64; N]; N];
    let mut atb = [0f64; N];
    for (row, b) in rows {
        let row = row.map(f64::from);
        for i in 0..N {
            for j in 0..N {
                ata[i][j] += row[i] * row[j];
            }
            atb[i] += row[i] * f64::from(b);
        }
    }
    (ata, atb)
}

fn invert<const N: usize>(a: [[f64; N]; N]) -> [[f64; N]; N] {
    let mut m: Vec<Vec<f64>> = (0..N)
        .map(|i| {
            a[i].iter()
                .copied()
                .chain((0..N).map(|k| f64::from(u8::from(i == k))))
                .collect()
        })
        .collect();
    for col in 0..N {
        let pivot = (col..N)
            .max_by(|a, b| m[*a][col].abs().total_cmp(&m[*b][col].abs()))
            .expect("a square system");
        m.swap(col, pivot);
        let lead = m[col][col];
        assert!(lead.abs() > 1e-9, "the system is singular");
        for v in &mut m[col][col..] {
            *v /= lead;
        }
        let lead_row = m[col].clone();
        for (row, r) in m.iter_mut().enumerate() {
            if row != col {
                let f = r[col];
                for (v, p) in r[col..].iter_mut().zip(&lead_row[col..]) {
                    *v -= f * p;
                }
            }
        }
    }
    std::array::from_fn(|i| std::array::from_fn(|j| m[i][N + j]))
}

fn apply<const N: usize>(m: &[[f64; N]; N], v: &[f64; N]) -> [f64; N] {
    std::array::from_fn(|i| m[i].iter().zip(v).map(|(a, b)| a * b).sum())
}

fn fit(pixels: impl Iterator<Item = (Vec3, f32)>) -> [f32; 4] {
    let (ata, atb) = gram(pixels.map(|(n, lum)| ([n.x, n.y, n.z, 1.0], lum)));
    apply(&invert(ata), &atb).map(|v| v as f32)
}

#[derive(Debug, Clone, PartialEq)]
struct Score {
    outside: f32,
    seat: f32,
    palette: f32,
    off_centre: f32,
    judged: Option<Judged>,
}

#[derive(Debug, Clone, PartialEq)]
struct Judged {
    key: String,
    critic: Critic,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
struct Critic {
    compound: bool,
    score: u8,
    issues: Vec<String>,
}

impl Critic {
    fn parse(text: &str) -> Option<Critic> {
        let object = text.get(text.find('{')?..=text.rfind('}')?)?;
        let mut judgement: Critic = serde_json::from_str(object).ok()?;
        judgement.issues.truncate(5);
        (judgement.score <= 10).then_some(judgement)
    }
}

impl Score {
    fn measured(&self, t: &Thresholds) -> Option<String> {
        if self.outside > t.outside {
            Some(format!("outside {:.3} > {}", self.outside, t.outside))
        } else if self.seat < t.seat {
            Some(format!("seat {:.3} < {}", self.seat, t.seat))
        } else if self.palette > t.palette {
            Some(format!("palette {:.3} > {}", self.palette, t.palette))
        } else if self.off_centre > t.off_centre {
            Some(format!(
                "off_centre {:.3} > {}",
                self.off_centre, t.off_centre
            ))
        } else {
            None
        }
    }

    fn failing(&self, t: &Thresholds) -> Option<String> {
        self.measured(t).or_else(|| {
            self.judged
                .as_ref()
                .filter(|j| j.critic.compound)
                .map(|_| "reads as a compound".to_string())
        })
    }

    fn passes(&self, t: &Thresholds) -> bool {
        self.failing(t).is_none()
    }

    fn reaches(&self, t: &Thresholds) -> bool {
        self.passes(t)
            && self
                .judged
                .as_ref()
                .is_some_and(|j| j.critic.score >= t.critic)
    }

    fn rank(&self) -> (u8, f32) {
        (
            self.judged.as_ref().map_or(0, |j| j.critic.score),
            self.seat - self.outside - self.palette,
        )
    }

    fn issues(&self) -> &[String] {
        self.judged.as_ref().map_or(&[], |j| &j.critic.issues)
    }
}

fn shrink(image: &RgbaImage, side: u32) -> RgbaImage {
    let (w, h) = image.dimensions();
    let mut bins = vec![[0f32; 5]; (side * side) as usize];
    for (x, y, p) in image.enumerate_pixels() {
        let bin = &mut bins[(y * side / h * side + x * side / w) as usize];
        let a = f32::from(p[3]) / 255.0;
        for (i, c) in rgb(p).iter().enumerate() {
            bin[i] += c * a;
        }
        bin[3] += a;
        bin[4] += 1.0;
    }
    RgbaImage::from_fn(side, side, |x, y| {
        let b = bins[(y * side + x) as usize];
        if b[3] > 0.0 {
            rgba([b[0] / b[3], b[1] / b[3], b[2] / b[3]], b[3] / b[4])
        } else {
            Rgba([0, 0, 0, 0])
        }
    })
}

fn magnify(image: &RgbaImage, by: u32) -> RgbaImage {
    RgbaImage::from_fn(image.width() * by, image.height() * by, |x, y| {
        *image.get_pixel(x / by, y / by)
    })
}

fn tiles() -> &'static (Vec<RgbaImage>, RgbaImage) {
    static TILES: std::sync::OnceLock<(Vec<RgbaImage>, RgbaImage)> = std::sync::OnceLock::new();
    TILES.get_or_init(|| {
        let side = (2.0 * HEX) as u32;
        let decode = |skin: &look::Skin| {
            shrink(
                &image::load_from_memory(skin.png)
                    .unwrap_or_else(|e| panic!("{skin:?} does not decode: {e}"))
                    .to_rgba8(),
                side,
            )
        };
        (
            look::TILES.iter().map(decode).collect(),
            decode(&look::GROUT),
        )
    })
}

#[derive(Default)]
struct Mean {
    sum: [f32; 3],
    n: f32,
}

impl Mean {
    fn add(&mut self, v: f32) {
        self.add_rgb([v; 3]);
    }

    fn add_rgb(&mut self, c: [f32; 3]) {
        for (s, c) in self.sum.iter_mut().zip(c) {
            *s += c;
        }
        self.n += 1.0;
    }

    fn rgb(&self) -> [f32; 3] {
        assert!(self.n > 0.0, "a mean of nothing");
        self.sum.map(|s| s / self.n)
    }

    fn value(&self) -> f32 {
        self.rgb()[0]
    }
}

fn bilinear(image: &RgbaImage, at: Vec2) -> Rgba<u8> {
    let q = at - 0.5;
    let (x0, y0) = (q.x.floor(), q.y.floor());
    let (fx, fy) = (q.x - x0, q.y - y0);
    let mut sum = [0f32; 4];
    for (dx, dy, w) in [
        (0.0, 0.0, (1.0 - fx) * (1.0 - fy)),
        (1.0, 0.0, fx * (1.0 - fy)),
        (0.0, 1.0, (1.0 - fx) * fy),
        (1.0, 1.0, fx * fy),
    ] {
        let x = (x0 + dx).clamp(0.0, image.width() as f32 - 1.0) as u32;
        let y = (y0 + dy).clamp(0.0, image.height() as f32 - 1.0) as u32;
        let p = *image.get_pixel(x, y);
        for (s, c) in sum.iter_mut().zip(p.0) {
            *s += w * f32::from(c);
        }
    }
    Rgba(sum.map(|s| s.round() as u8))
}

fn in_hex(d: Vec2, radius: f32) -> bool {
    let (dx, dy) = (d.x.abs(), d.y.abs());
    dx <= radius * 3f32.sqrt() / 2.0 && dy <= radius - dx / 3f32.sqrt()
}

fn hex_distance(d: Vec2, radius: f32) -> f32 {
    if in_hex(d, radius) {
        return 0.0;
    }
    let corner = |k: i32| Vec2::from_angle((k as f32 + 0.5) * std::f32::consts::FRAC_PI_3) * radius;
    (0..6)
        .map(|k| {
            let (a, b) = (corner(k), corner(k + 1));
            let t = ((d - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
            d.distance(a + (b - a) * t)
        })
        .fold(f32::INFINITY, f32::min)
}

fn rgb(p: &Rgba<u8>) -> [f32; 3] {
    [p[0], p[1], p[2]].map(|c| f32::from(c) / 255.0)
}

fn luminance(p: &Rgba<u8>) -> f32 {
    let c = rgb(p);
    (c[0] + c[1] + c[2]) / 3.0
}

fn grey(v: f32) -> Rgba<u8> {
    rgba([v; 3], 1.0)
}

fn rgba(c: [f32; 3], alpha: f32) -> Rgba<u8> {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Rgba([q(c[0]), q(c[1]), q(c[2]), q(alpha)])
}

fn apart(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn spill(c: [f32; 3]) -> f32 {
    c[1] - c[0].max(c[2])
}

fn pixel_opacity(p: &Rgba<u8>) -> f32 {
    opacity(rgb(p)) * f32::from(p[3]) / 255.0
}

fn opacity(c: [f32; 3]) -> f32 {
    1.0 - ((spill(c) - SPILL[0]) / (SPILL[1] - SPILL[0])).clamp(0.0, 1.0)
}

fn unspill(c: [f32; 3]) -> [f32; 3] {
    [c[0], c[1] - spill(c).max(0.0), c[2]]
}

fn open(path: impl AsRef<Path>) -> RgbaImage {
    let path = path.as_ref();
    image::open(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .into_rgba8()
}

fn save(image: &RgbaImage, path: &Path) {
    image
        .save(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

fn stack(images: &[RgbaImage]) -> RgbaImage {
    let first = images.first().expect("a relight set is not empty");
    assert!(
        images
            .iter()
            .all(|image| image.dimensions() == first.dimensions())
    );
    RgbaImage::from_fn(
        first.width(),
        first.height() * images.len() as u32,
        |x, y| *images[(y / first.height()) as usize].get_pixel(x, y % first.height()),
    )
}

const PAINTERS: usize = 6;
const ROUNDS: usize = 3;

struct Paint {
    images: Vec<PathBuf>,
    output: PathBuf,
    prompt_path: PathBuf,
    size: u32,
}

type Painter<'a> = &'a (dyn Fn(&Paint) -> Result<(), String> + Sync);

fn paint_sh(art: &Art, job: &Paint) -> Result<(), String> {
    let stem = job
        .output
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut command = std::process::Command::new(art.paint_sh());
    command.arg("-s").arg(job.size.to_string());
    for image in &job.images {
        command.arg("-i").arg(image);
    }
    let output = command
        .arg(&job.output)
        .arg(&job.prompt_path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("{}: {e}", art.paint_sh().display()))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stderr.lines().filter(|l| !l.trim().is_empty()) {
        eprintln!("{stem}: {line}");
    }
    if output.status.success() {
        let mut image = open(&job.output);
        if image.pixels().any(|p| p[3] != 255) {
            for p in image.pixels_mut() {
                let a = f32::from(p[3]) / 255.0;
                let c = rgb(p);
                *p = rgba([0, 1, 2].map(|i| c[i] * a + KEY[i] * (1.0 - a)), 1.0);
            }
            save(&image, &job.output);
        }
        return Ok(());
    }
    Err(stderr
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("paint.sh failed without a word")
        .to_string())
}

struct Cap {
    free: std::sync::Mutex<usize>,
    freed: std::sync::Condvar,
}

struct Permit<'a>(&'a Cap);

impl Drop for Permit<'_> {
    fn drop(&mut self) {
        *self.0.free.lock().expect("the cap is unpoisoned") += 1;
        self.0.freed.notify_one();
    }
}

impl Cap {
    fn new(n: usize) -> Cap {
        Cap {
            free: std::sync::Mutex::new(n),
            freed: std::sync::Condvar::new(),
        }
    }

    fn take(&self) -> Permit<'_> {
        let mut free = self.free.lock().expect("the cap is unpoisoned");
        while *free == 0 {
            free = self.freed.wait(free).expect("the cap is unpoisoned");
        }
        *free -= 1;
        Permit(self)
    }
}

fn key(parts: &[&[u8]]) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: u8| {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for part in parts {
        for b in (part.len() as u64).to_le_bytes() {
            eat(b);
        }
        for b in *part {
            eat(*b);
        }
    }
    format!("{h:016x}")
}

fn input_key(text: &str, images: &[PathBuf]) -> String {
    let contents: Vec<Vec<u8>> = images.iter().map(|path| read(path)).collect();
    let mut parts = vec![text.as_bytes()];
    parts.extend(contents.iter().map(Vec::as_slice));
    key(&parts)
}

fn painted_key(prompt: &str, count: u32, scaffold: &RgbaImage, references: &[PathBuf]) -> String {
    let count = count.to_le_bytes();
    let width = scaffold.width().to_le_bytes();
    let contents: Vec<Vec<u8>> = references.iter().map(|path| read(path)).collect();
    let mut parts = vec![prompt.as_bytes(), &count, &width, scaffold.as_raw()];
    parts.extend(contents.iter().map(Vec::as_slice));
    key(&parts)
}

fn brief(name: &str, style: &Style, direction: &str) -> String {
    let shared = if name.starts_with("arm") {
        &style.arm
    } else {
        &style.shared
    };
    let palette = include_str!("../art/BIBLE.md")
        .split_once("## 1. Palette\n")
        .unwrap()
        .1
        .split_once("## 2. Language")
        .unwrap()
        .0;
    format!("{shared} {direction}\nPalette table from art/BIBLE.md:\n{palette}")
}

fn caption(text: &str) -> &str {
    text.rsplit_once("\n\nImage inputs:\n")
        .filter(|(_, inputs)| {
            let inputs = inputs.trim_end();
            !inputs.is_empty()
                && inputs.lines().all(|line| {
                    line == "none"
                        || line == "unknown"
                        || line
                            .strip_prefix("unknown ")
                            .is_some_and(|path| serde_json::from_str::<String>(path).is_ok())
                        || line
                            .strip_prefix("sha256 ")
                            .and_then(|line| line.split_once(' '))
                            .is_some_and(|(hash, path)| {
                                hash.len() == 64
                                    && hash.bytes().all(|c| c.is_ascii_hexdigit())
                                    && serde_json::from_str::<String>(path).is_ok()
                            })
                })
        })
        .map_or(text, |(caption, _)| caption)
}

fn stored(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(caption(&text).trim().to_string()).filter(|text| !text.is_empty())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

fn store(path: &Path, prompt: &str) -> Result<String, String> {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err(format!("{}: an empty prompt was written", path.display()));
    }
    std::fs::write(path, format!("{prompt}\n\nImage inputs:\nunknown\n"))
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(prompt.to_string())
}

const JUDGE: &str = "Additional required glyph gate: does the machine read as atoms joined by bonds, a compound? If yes, compound is true and the candidate fails regardless of score. A glyph is one continuous built housing with seats as openings. This gate takes precedence over the rubric's exclusion of how parts connect. Arms are exempt from this glyph gate. Answer with exactly one JSON object containing score (integer 0 through 10), compound (boolean), and issues (at most five ranked strings). Do not run commands, edit anything or write files.";
const CRITIC_SCHEMA: &str = r#"{"type":"object","properties":{"compound":{"type":"boolean"},"score":{"type":"integer","minimum":0,"maximum":10},"issues":{"type":"array","maxItems":5,"items":{"type":"string"}}},"required":["score","compound","issues"],"additionalProperties":false}"#;

fn relit_key(kept: &[u8], style: &Style, facings: &[Facing]) -> String {
    let facings: Vec<&str> = facings.iter().map(|facing| facing.name()).collect();
    key(&[
        kept,
        b"computed-relief-2:blur=4,height=0.85+0.15*luma,slope=24,lambert,rgba",
        facings.join("\n").as_bytes(),
        &style.elevation.to_le_bytes(),
        &AMBIENT.to_le_bytes(),
    ])
}

fn direction_error(facings: &[Facing], elevation: f32, lights: &[Light]) -> f32 {
    facings
        .iter()
        .zip(lights)
        .map(|(facing, light)| {
            light
                .direction
                .dot(facing.light(elevation))
                .clamp(-1.0, 1.0)
                .acos()
                .to_degrees()
        })
        .fold(0.0, f32::max)
}

fn judged_key(name: &str, critic: &str, candidate: &[u8]) -> String {
    if name.starts_with("arm") {
        key(&[critic.as_bytes(), candidate])
    } else {
        key(&[critic.as_bytes(), candidate, JUDGE.as_bytes()])
    }
}

fn rebrief(brief: &str, prompt: &str, issues: &[String], round: usize) -> Option<String> {
    if round == ROUNDS {
        return Some(format!(
            "{brief} Write a new prompt from this brief alone: the whole object's straight-down gameplay read and its seat layout come first, and any detail that competes with them is simplified or left out."
        ));
    }
    if issues.is_empty() {
        return None;
    }
    Some(format!(
        "{brief} The current prompt: \"{prompt}\" The best candidate has these measured or visual issues, most important first: {} Rewrite the prompt so the next picture fixes them.",
        issues.join(" ")
    ))
}

fn best(scored: &[(u32, Score)], keep: impl Fn(&Score) -> bool) -> Option<(u32, &Score)> {
    scored
        .iter()
        .filter(|(_, s)| keep(s))
        .max_by(|a, b| {
            let (ra, rb) = (a.1.rank(), b.1.rank());
            ra.0.cmp(&rb.0)
                .then(ra.1.total_cmp(&rb.1))
                .then_with(|| a.0.cmp(&b.0).reverse())
        })
        .map(|(i, s)| (*i, s))
}

type Ask<'a> = &'a (dyn Fn(&[PathBuf], &str) -> Result<String, String> + Sync);

fn run(script: &Path, images: &[PathBuf], args: &[&str]) -> Result<String, String> {
    let mut command = std::process::Command::new(script);
    for image in images {
        command.arg("-i").arg(image);
    }
    let output = command
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("{}: {e}", script.display()))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stderr.lines().filter(|l| !l.trim().is_empty()) {
        eprintln!("{line}");
    }
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    Err(stderr
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("{} failed without a word", script.display())))
}

fn read(path: &Path) -> Vec<u8> {
    std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn quantise(png: &Path) -> Result<(), String> {
    let status = std::process::Command::new("pngquant")
        .args(["--quality", "70-95", "--speed", "1", "--force", "--output"])
        .arg(png)
        .arg(png)
        .status()
        .map_err(|e| format!("pngquant: {e}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("pngquant {status} on {}", png.display()))
}

impl Score {
    fn verdict(&self, t: &Thresholds) -> String {
        match self.failing(t) {
            None => "pass".to_string(),
            Some(rule) => format!("fail {rule}"),
        }
    }

    fn row(&self, candidate: &str, t: &Thresholds) -> String {
        format!(
            "{candidate}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{}\t{}\t{}\t{}\t{}",
            self.outside,
            self.seat,
            self.palette,
            self.off_centre,
            self.judged
                .as_ref()
                .map_or("-".to_string(), |j| j.critic.score.to_string()),
            self.verdict(t),
            serde_json::to_string(self.issues()).expect("issues serialise"),
            self.judged.as_ref().map_or("-", |j| j.key.as_str()),
            self.judged
                .as_ref()
                .map_or("-", |j| if j.critic.compound { "true" } else { "false" })
        )
    }

    fn parse(row: &str) -> Option<(String, Score)> {
        let mut cols: Vec<&str> = row.split('\t').collect();
        let compound = if cols.len() == 10 {
            cols.pop()?.parse().ok()
        } else {
            Some(false)
        };
        let [
            candidate,
            outside,
            seat,
            palette,
            off_centre,
            critic,
            _,
            issues,
            judged,
        ] = cols[..]
        else {
            return None;
        };
        let judged = match (critic, judged) {
            ("-", "-") => None,
            (_, "-") | ("-", _) => return None,
            (score, key) => Some(Judged {
                key: key.to_string(),
                critic: Critic {
                    compound: compound?,
                    score: score.parse().ok()?,
                    issues: serde_json::from_str(issues).ok()?,
                },
            }),
        };
        Some((
            candidate.to_string(),
            Score {
                outside: outside.parse().ok()?,
                seat: seat.parse().ok()?,
                palette: palette.parse().ok()?,
                off_centre: off_centre.parse().ok()?,
                judged,
            },
        ))
    }
}

struct Prepared {
    scaffold: Scaffold,
    style: Style,
    thresholds: Thresholds,
    count: u32,
    entry: Entry,
    brief: String,
    prompt: Option<String>,
    rendered: RgbaImage,
    scaffold_png: PathBuf,
    references: Vec<PathBuf>,
    changed: bool,
}

struct Remake<'a> {
    art: &'a Art,
    manifest: std::sync::Mutex<Manifest>,
    director: Ask<'a>,
    painter: Painter<'a>,
    critic: Ask<'a>,
    cap: Cap,
    redraw: std::sync::atomic::AtomicBool,
}

struct Relight<'a> {
    dir: &'a Path,
    scaffold: &'a Scaffold,
    candidate: &'a RgbaImage,
    style: &'a Style,
    facings: &'a [Facing],
    threshold: f32,
    source: RelightSource,
}

#[derive(Clone, Copy)]
enum RelightSource {
    Machine,
    Texture,
}

impl Remake<'_> {
    fn paint(&self, jobs: Vec<(String, Paint)>) -> Vec<(String, Result<(), String>)> {
        std::thread::scope(|s| {
            let handles: Vec<_> = jobs
                .into_iter()
                .map(|(label, job)| {
                    s.spawn(move || {
                        let out = {
                            let _permit = self.cap.take();
                            (self.painter)(&job)
                        };
                        match &out {
                            Ok(()) => {
                                self.redraw.store(true, std::sync::atomic::Ordering::SeqCst);
                                println!("{label}\tpainted");
                            }
                            Err(e) => println!("{label}\tpaint failed: {e}"),
                        }
                        (label, out)
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("a paint returns"))
                .collect()
        })
    }

    fn entry(&self, name: &str) -> Result<(Style, Thresholds, u32, Entry), String> {
        let m = self.manifest.lock().expect("the manifest is unpoisoned");
        let entry = m
            .machine
            .get(name)
            .ok_or_else(|| format!("manifest has no [machine.{name}]"))?;
        Ok((m.style.clone(), m.thresholds, m.candidates, entry.clone()))
    }

    fn record(&self, name: &str, patch: impl FnOnce(&mut Entry)) {
        let mut m = self.manifest.lock().expect("the manifest is unpoisoned");
        let entry = m.machine.get_mut(name).expect("the entry read above");
        let before = entry.clone();
        patch(entry);
        if *entry != before {
            self.art.write(&m);
        }
    }

    fn record_texture(&self, name: &str, relit: String) {
        let mut m = self.manifest.lock().expect("the manifest is unpoisoned");
        let entry = m
            .texture
            .get_mut(name)
            .expect("the texture entry read above");
        if entry.relit.as_deref() != Some(&relit) {
            entry.relit = Some(relit);
            self.art.write(&m);
        }
    }

    fn prepare_relief(
        &self,
        dir: &Path,
        scaffold: &Scaffold,
        candidate: &RgbaImage,
        keyed: &[u8],
        style: &Style,
        facings: &[Facing],
    ) -> Result<String, String> {
        assert_eq!(style.facings, Facing::ALL);
        let relit = dir.join("relit");
        let expected = relit_key(keyed, style, facings);
        if std::fs::read_to_string(relit.join("render-key.txt"))
            .ok()
            .as_deref()
            != Some(&expected)
            && relit.exists()
        {
            std::fs::remove_dir_all(&relit).map_err(|e| e.to_string())?;
        }
        std::fs::create_dir_all(&relit).map_err(|e| e.to_string())?;
        save(&scaffold.master(candidate), &relit.join("master.png"));
        std::fs::write(relit.join("render-key.txt"), &expected).map_err(|e| e.to_string())?;
        Ok(expected)
    }

    fn score(&self, name: &str, scaffold: &Scaffold) -> Vec<(u32, Score)> {
        self.art
            .candidates(name)
            .into_iter()
            .map(|i| {
                let png = open(self.art.candidate(name, i));
                (i, scaffold.score(&scaffold.register(&png)))
            })
            .collect()
    }

    fn assess(
        &self,
        name: &str,
        scaffold: &Scaffold,
        style: &Style,
        thresholds: Thresholds,
        scored: Vec<(u32, Score)>,
    ) -> Result<Vec<(u32, Score)>, String> {
        let dir = self.art.machine(name);
        let previous: BTreeMap<u32, Score> = std::fs::read_to_string(dir.join("scores.tsv"))
            .unwrap_or_default()
            .lines()
            .filter_map(Score::parse)
            .filter_map(|(label, score)| {
                let index: u32 = label.rsplit('-').next()?.parse().ok()?;
                self.art
                    .candidate(name, index)
                    .exists()
                    .then_some((index, score))
            })
            .collect();
        let scaffold_png = dir.join("scaffold.png");
        std::fs::create_dir_all(dir.join("judged")).map_err(|e| e.to_string())?;
        let scored: Vec<(u32, Score)> = std::thread::scope(|s| {
            let handles: Vec<_> = scored
                .into_iter()
                .map(|(i, mut score)| {
                    let (previous, scaffold_png, style) =
                        (previous.get(&i), scaffold_png.clone(), &style);
                    s.spawn(move || {
                        let key =
                            judged_key(name, &style.critic, &read(&self.art.candidate(name, i)));
                        if let Some(judged) = previous
                            .and_then(|p| p.judged.clone())
                            .filter(|j| j.key == key)
                        {
                            score.judged = Some(judged);
                            return (i, score);
                        }
                        if score.measured(&thresholds).is_some() {
                            return (i, score);
                        }
                        let board = self.art.judged(name, i);
                        let capture = scaffold.register(&open(self.art.candidate(name, i)));
                        save(&scaffold.board(&capture.image), &board);
                        let label = format!("{name}-{i}");
                        self.redraw.store(true, std::sync::atomic::Ordering::SeqCst);
                        let reply = {
                            let _permit = self.cap.take();
                            (self.critic)(&[board, scaffold_png], &style.critic)
                        };
                        score.judged = match reply {
                            Ok(text) => {
                                let critic = Critic::parse(&text);
                                if critic.is_none() {
                                    println!("{label}\tcritic returned no score: {}", text.trim());
                                }
                                critic.map(|critic| Judged { key, critic })
                            }
                            Err(e) => {
                                println!("{label}\tcritic failed: {e}");
                                None
                            }
                        };
                        (i, score)
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| h.join().expect("a judgement returns"))
                .collect()
        });
        let mut rows = previous;
        rows.extend(scored.iter().cloned());
        let text: Vec<String> = rows
            .iter()
            .map(|(i, s)| s.row(&format!("{name}-{i}"), &thresholds))
            .collect();
        std::fs::write(dir.join("scores.tsv"), text.join("\n") + "\n")
            .map_err(|e| e.to_string())?;
        for (i, s) in &scored {
            println!("{}", s.row(&format!("{name}-{i}"), &thresholds));
        }
        Ok(scored)
    }

    fn prepare(&self, name: &str) -> Result<Prepared, String> {
        let scaffold = Scaffold::of(item(name));
        let dir = self.art.machine(name);
        let (style, thresholds, count, entry) = self.entry(name)?;
        if count == 0 {
            return Err("the manifest asks for no candidates".to_string());
        }
        std::fs::create_dir_all(dir.join("candidates")).map_err(|e| e.to_string())?;
        let rendered = scaffold.render();
        let scaffold_png = dir.join("scaffold.png");
        let mut changed = false;
        if !scaffold_png.exists() || open(&scaffold_png).as_raw() != rendered.as_raw() {
            save(&rendered, &scaffold_png);
            changed = true;
        }
        let brief = brief(name, &style, &entry.direction);
        let references: Vec<PathBuf> = entry
            .references
            .iter()
            .map(|reference| self.art.dir.join(reference))
            .collect();
        for reference in &references {
            if !reference.is_file() {
                return Err(format!("{} is not a reference image", reference.display()));
            }
        }
        let briefed = input_key(&brief, &references);
        let prompt = stored(&self.art.prompt(name))?
            .filter(|_| entry.briefed.as_deref() == Some(briefed.as_str()));
        Ok(Prepared {
            scaffold,
            style,
            thresholds,
            count,
            entry,
            brief,
            prompt,
            rendered,
            scaffold_png,
            references,
            changed,
        })
    }

    fn author(&self, images: &[PathBuf], text: &str) -> Result<String, String> {
        let _permit = self.cap.take();
        (self.director)(images, text)
    }

    fn machine(&self, name: &str, prepared: Prepared) -> Result<bool, String> {
        let started = std::time::Instant::now();
        let Prepared {
            scaffold,
            style,
            thresholds,
            count,
            entry,
            brief,
            prompt,
            rendered,
            scaffold_png,
            references,
            mut changed,
        } = prepared;
        let dir = self.art.machine(name);
        let candidates = dir.join("candidates");
        let mut kept = entry.kept.filter(|k| self.art.candidate(name, *k).exists());
        let mut issues: Vec<String> = Vec::new();
        let mut prompt = match prompt {
            Some(prompt) => prompt,
            None => {
                changed = true;
                let images = std::iter::once(scaffold_png.clone())
                    .chain(references.iter().cloned())
                    .collect::<Vec<_>>();
                let written = self.author(&images, &brief)?;
                let stored = store(&self.art.prompt(name), &written)?;
                self.record(name, |e| {
                    let references: Vec<PathBuf> = e
                        .references
                        .iter()
                        .map(|reference| self.art.dir.join(reference))
                        .collect();
                    e.briefed = Some(input_key(&brief, &references));
                });
                stored
            }
        };
        let painted = painted_key(&prompt, count, &rendered, &references);
        if entry.painted.as_deref() != Some(painted.as_str()) {
            let _ = std::fs::remove_dir_all(&candidates);
            let _ = std::fs::remove_dir_all(dir.join("judged"));
            let _ = std::fs::remove_file(dir.join("scores.tsv"));
            kept = None;
        }
        std::fs::create_dir_all(&candidates).map_err(|e| e.to_string())?;
        let kept = 'keep: {
            for round in 1..=ROUNDS {
                let slots = (round as u32 - 1) * count + 1..=round as u32 * count;
                let empty = slots.clone().all(|i| !self.art.candidate(name, i).exists());
                if empty
                    && round > 1
                    && let Some(text) = rebrief(&brief, &prompt, &issues, round)
                {
                    let images = std::iter::once(scaffold_png.clone())
                        .chain(references.iter().cloned())
                        .collect::<Vec<_>>();
                    prompt = self.author(&images, &text)?;
                    changed = true;
                }
                let failed: Vec<String> = if empty {
                    store(&self.art.round(name, round), &prompt)?;
                    let jobs = slots
                        .map(|i| {
                            (
                                format!("{name}-{i}"),
                                Paint {
                                    images: std::iter::once(scaffold_png.clone())
                                        .chain(references.iter().cloned())
                                        .collect(),
                                    output: self.art.candidate(name, i),
                                    prompt_path: self.art.round(name, round),
                                    size: scaffold.canvas,
                                },
                            )
                        })
                        .collect();
                    self.paint(jobs)
                        .into_iter()
                        .filter_map(|(label, r)| r.err().map(|e| format!("{label}: {e}")))
                        .collect()
                } else {
                    Vec::new()
                };
                let painted_now = empty && failed.len() < count as usize;
                changed |= painted_now;
                if painted_now && round == 1 {
                    std::fs::copy(self.art.round(name, round), self.art.prompt(name))
                        .map_err(|e| e.to_string())?;
                }
                if painted_now {
                    self.record(name, |e| {
                        e.kept = None;
                        e.painted = Some(painted.clone());
                        e.relit = None;
                    });
                }
                let only_kept = kept.filter(|_| !empty);
                let mut scored = match only_kept {
                    Some(k) => {
                        let png = open(self.art.candidate(name, k));
                        vec![(k, scaffold.score(&scaffold.register(&png)))]
                    }
                    None => self.score(name, &scaffold),
                };
                let assess = |scored| self.assess(name, &scaffold, &style, thresholds, scored);
                scored = assess(scored)?;
                let keeping = |scored: &[(u32, Score)], i: u32| {
                    scored
                        .iter()
                        .any(|(k, s)| *k == i && s.passes(&thresholds) && s.judged.is_some())
                };
                if let Some(k) = kept.filter(|k| keeping(&scored, *k)) {
                    break 'keep k;
                }
                if only_kept.is_some() {
                    scored = assess(self.score(name, &scaffold))?;
                }
                if let Some((k, _)) = best(&scored, |s| s.reaches(&thresholds)) {
                    break 'keep k;
                }
                if empty && failed.len() == count as usize {
                    return Err(format!(
                        "no candidate passes; {} of {count} paints failed: {}",
                        failed.len(),
                        failed.join(", ")
                    ));
                }
                let measured = scored
                    .iter()
                    .filter(|(_, s)| s.measured(&thresholds).is_none());
                if measured.clone().count() > 0 && measured.clone().all(|(_, s)| s.judged.is_none())
                {
                    return Err("the critic read no candidate; nothing repainted".to_string());
                }
                if round == ROUNDS
                    && let Some((k, _)) =
                        best(&scored, |s| s.passes(&thresholds) && s.judged.is_some())
                {
                    break 'keep k;
                }
                issues = best(&scored, |s| s.measured(&thresholds).is_none())
                    .or_else(|| best(&scored, |_| true))
                    .map(|(_, s)| {
                        s.measured(&thresholds)
                            .into_iter()
                            .chain(s.issues().iter().cloned())
                            .collect()
                    })
                    .unwrap_or_default();
            }
            return Err(format!(
                "no candidate passes the measured rules in {ROUNDS} rounds"
            ));
        };
        let capture = scaffold.register(&open(self.art.candidate(name, kept)));
        let facings = &style.facings;
        let relit = self.prepare_relief(
            &dir,
            &scaffold,
            &capture.image,
            &read(&self.art.candidate(name, kept)),
            &style,
            facings,
        )?;
        let relit_dir = dir.join("relit");
        let current = entry.relit.as_deref() == Some(relit.as_str())
            && dir.join("albedo.png").exists()
            && dir.join("normal.png").exists()
            && relit_dir.join("albedo.png").exists()
            && relit_dir.join("normal.png").exists();
        if !current {
            self.relief(
                name,
                &Relight {
                    dir: &dir,
                    scaffold: &scaffold,
                    candidate: &capture.image,
                    style: &style,
                    facings,
                    threshold: thresholds.sphere,
                    source: RelightSource::Machine,
                },
            )?;
            changed = true;
        }
        self.record(name, |e| {
            e.kept = Some(kept);
            e.relit = Some(relit.clone());
        });
        println!(
            "{name}\t{}\tkept {name}-{kept}\t{:.0}s",
            if changed { "landed" } else { "up to date" },
            started.elapsed().as_secs_f32()
        );
        Ok(changed)
    }

    fn relief(&self, name: &str, request: &Relight<'_>) -> Result<(), String> {
        let relit = request.dir.join("relit");
        std::fs::create_dir_all(&relit).map_err(|e| e.to_string())?;
        let normal = request.scaffold.surface_normals(request.candidate);
        let edits: Vec<_> = request
            .facings
            .iter()
            .map(|facing| {
                let image = request.scaffold.render_light(
                    request.candidate,
                    &normal,
                    facing.light(request.style.elevation),
                );
                save(&image, &relit.join(format!("{}.png", facing.name())));
                (facing.name(), image)
            })
            .collect();
        let error = self.accept_relief(request, &edits, &normal)?;
        println!("{name}\tcomputed relight error {error:.3} degrees");
        if !error.is_finite() || error > request.threshold {
            return Err(format!(
                "computed calibration error {error} exceeds {}",
                request.threshold
            ));
        }
        Ok(())
    }

    fn accept_relief(
        &self,
        request: &Relight<'_>,
        edits: &[(&str, RgbaImage)],
        normal: &RgbaImage,
    ) -> Result<f32, String> {
        let images: Vec<RgbaImage> = edits.iter().map(|(_, image)| image.clone()).collect();
        let relief = request.scaffold.calibration(&images);
        let mut lights: Vec<String> = edits
            .iter()
            .zip(&relief.lights)
            .zip(request.facings)
            .map(|(((edge, _), light), facing)| {
                let d = light.direction;
                let want = facing.light(request.style.elevation);
                let error = d.dot(want).clamp(-1.0, 1.0).acos().to_degrees();
                format!(
                    "{edge}\trequested=({:+.6},{:+.6},{:+.6})\tmeasured=({:+.6},{:+.6},{:+.6})\terror={error:.3}\tambient={:.6}",
                    want.x, want.y, want.z, d.x, d.y, d.z, light.ambient
                )
            })
            .collect();
        let direction = direction_error(request.facings, request.style.elevation, &relief.lights);
        let error = relief.sphere.max(direction);
        lights.push(format!("sphere error {:.3} degrees", relief.sphere));
        lights.push(format!("direction error {direction:.3} degrees"));
        let relit = request.dir.join("relit");
        std::fs::write(relit.join("lights.txt"), lights.join("\n") + "\n")
            .map_err(|e| e.to_string())?;
        if !error.is_finite() || error > request.threshold {
            return Ok(error);
        }
        save(&request.scaffold.cropped(normal), &relit.join("normal.png"));
        if matches!(request.source, RelightSource::Machine) {
            save(
                &request.scaffold.cut(request.candidate),
                &request.dir.join("albedo.png"),
            );
            save(
                &request.scaffold.cropped(normal),
                &request.dir.join("normal.png"),
            );
            quantise(&request.dir.join("albedo.png"))?;
        }
        let extract = |image: &RgbaImage| request.scaffold.cropped(image);
        save(
            &stack(&images.iter().map(extract).collect::<Vec<_>>()),
            &relit.join("albedo.png"),
        );
        quantise(&relit.join("albedo.png"))?;
        Ok(error)
    }

    fn texture(&self, name: &str) -> Result<bool, String> {
        let (style, thresholds, entry) = {
            let m = self.manifest.lock().expect("the manifest is unpoisoned");
            let entry = m
                .texture
                .get(name)
                .ok_or_else(|| format!("manifest has no [texture.{name}]"))?;
            (m.style.clone(), m.thresholds, entry.clone())
        };
        let source = self.art.dir.join("..").join(&entry.source);
        let image = open(&source);
        if image.width() != image.height() {
            return Err(format!("{} is not square", source.display()));
        }
        let scaffold = Scaffold::texture(image.width());
        let candidate = scaffold.mount(&image);
        let dir = self.art.texture(name);
        let key = self.prepare_relief(
            &dir,
            &scaffold,
            &candidate,
            image.as_raw(),
            &style,
            &style.facings,
        )?;
        let relit = dir.join("relit");
        let current = entry.relit.as_deref() == Some(&key)
            && relit.join("albedo.png").exists()
            && relit.join("normal.png").exists();
        if current {
            println!("{name}\tup to date");
            return Ok(false);
        }
        self.relief(
            name,
            &Relight {
                dir: &dir,
                scaffold: &scaffold,
                candidate: &candidate,
                style: &style,
                facings: &style.facings,
                threshold: thresholds.sphere,
                source: RelightSource::Texture,
            },
        )?;
        self.record_texture(name, key);
        println!("{name}\tlanded");
        Ok(true)
    }

    fn sheet(&self) -> Result<(), String> {
        let m = self.manifest.lock().expect("the manifest is unpoisoned");
        let mut args: Vec<std::ffi::OsString> = vec!["montage".into()];
        for item in Machine::ALL {
            let name = name(item);
            let scores = self.art.machine(name).join("scores.tsv");
            let (Some(entry), Ok(text)) = (m.machine.get(name), std::fs::read_to_string(&scores))
            else {
                continue;
            };
            for row in text.lines().filter(|l| !l.is_empty()) {
                let Some((candidate, score)) = Score::parse(row) else {
                    return Err(format!("{}: bad row {row:?}", scores.display()));
                };
                let index: u32 = candidate
                    .rsplit('-')
                    .next()
                    .and_then(|i| i.parse().ok())
                    .ok_or_else(|| format!("{}: bad candidate {candidate:?}", scores.display()))?;
                let kept = if entry.kept == Some(index) {
                    "KEPT "
                } else {
                    ""
                };
                let verdict = score.verdict(&m.thresholds);
                let rule: Vec<&str> = verdict.split(' ').take(2).collect();
                args.push("-label".into());
                args.push(
                    format!(
                        "{kept}{candidate}  {:.3} {:.3} {:.3} {:.3} {}  {}",
                        score.outside,
                        score.seat,
                        score.palette,
                        score.off_centre,
                        score
                            .judged
                            .as_ref()
                            .map_or("-".to_string(), |j| j.critic.score.to_string()),
                        rule.join(" ")
                    )
                    .into(),
                );
                args.push(self.art.candidate(name, index).into());
            }
        }
        if args.len() == 1 {
            return Ok(());
        }
        let part = self.art.dir.join("sheet.png.part");
        args.extend(
            [
                "-tile",
                "4x",
                "-geometry",
                "340x340+6+6",
                "-background",
                "#6B4F3A",
                "-fill",
                "#F4EDE4",
                "-font",
                "DejaVu-Sans",
                "-pointsize",
                "12",
            ]
            .map(Into::into),
        );
        args.push(part.clone().into());
        let status = std::process::Command::new("magick")
            .args(&args)
            .status()
            .map_err(|e| format!("magick: {e}"))?;
        if !status.success() {
            return Err(format!("magick montage {status}"));
        }
        quantise(&part)?;
        std::fs::rename(&part, self.art.dir.join("sheet.png")).map_err(|e| e.to_string())
    }
}

impl<'a> Remake<'a> {
    fn new(art: &'a Art, director: Ask<'a>, painter: Painter<'a>, critic: Ask<'a>) -> Remake<'a> {
        Remake {
            art,
            manifest: std::sync::Mutex::new(art.read()),
            director,
            painter,
            critic,
            cap: Cap::new(PAINTERS),
            redraw: std::sync::atomic::AtomicBool::new(false),
        }
    }
}

fn remake(
    art: &Art,
    names: &[String],
    director: Ask,
    painter: Painter,
    critic: Ask,
) -> Vec<(String, Result<bool, String>)> {
    let remake = Remake::new(art, director, painter, critic);
    let mut names: Vec<&String> = names.iter().collect();
    names.sort();
    names.dedup();
    let remake = &remake;
    let results: Vec<(String, Result<bool, String>)> = std::thread::scope(|s| {
        let handles: Vec<_> = names
            .into_iter()
            .map(|name| {
                s.spawn(move || {
                    let machine = remake
                        .manifest
                        .lock()
                        .expect("the manifest is unpoisoned")
                        .machine
                        .contains_key(name);
                    let result = if machine {
                        remake.prepare(name).and_then(|p| remake.machine(name, p))
                    } else {
                        remake.texture(name)
                    };
                    (name.to_string(), result)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("a machine returns"))
            .collect()
    });
    let sheet = art.dir.join("sheet.png");
    if (!sheet.exists() || remake.redraw.load(std::sync::atomic::Ordering::SeqCst))
        && let Err(e) = remake.sheet()
    {
        eprintln!("sheet: {e}");
    }
    results
}

fn landed(results: &[(String, Result<bool, String>)]) -> bool {
    let mut landed = true;
    for (name, result) in results {
        if let Err(e) = result {
            landed = false;
            eprintln!("{name}: did not land: {e}");
        }
    }
    landed
}

pub fn configure(args: &[String]) -> Option<i32> {
    const USAGE: &str = "usage: ziral --gen NAME... | ziral --gen --all";
    let art = Art::shipped();
    if args.get(1).map(String::as_str) != Some("--gen") {
        return None;
    }
    let rest: Vec<&str> = args.iter().skip(2).map(String::as_str).collect();
    let manifest = art.read();
    let known = |n: &str| manifest.machine.contains_key(n) || manifest.texture.contains_key(n);
    let director = |images: &[PathBuf], brief: &str| run(&art.direct_sh(), images, &[brief]);
    let painter = |job: &Paint| paint_sh(&art, job);
    let judging = std::sync::Mutex::new(());
    let critic = |images: &[PathBuf], rubric: &str| {
        let _judging = judging.lock().expect("one critic at a time");
        run(
            &art.ask_sh(),
            images,
            &[CRITIC_SCHEMA, &format!("{rubric} {JUDGE}")],
        )
    };
    let names: Vec<String> = match rest.as_slice() {
        ["--all"] => manifest
            .machine
            .keys()
            .chain(manifest.texture.keys())
            .cloned()
            .collect(),
        [_, ..] if rest.iter().all(|n| known(n)) => rest.iter().map(|n| n.to_string()).collect(),
        _ => {
            eprintln!("{USAGE}");
            return Some(2);
        }
    };
    if names.is_empty() {
        println!("nothing to do");
        return Some(0);
    }
    let generated = landed(&remake(&art, &names, &director, &painter, &critic));
    let split = art.split(&manifest, &names);
    Some(i32::from(!(generated && split)))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::look::light;
    use crate::sim::{Arm, Glyph, Hex, ORIGIN};

    const RELIEF: f32 = 0.1;
    const ASPECT: f32 = 0.02;

    #[test]
    fn machines_register_transparent_paints_over_the_scaffold_key_before_sampling() {
        let scaffold = Scaffold::of(Machine::Glyph(crate::sim::GlyphKind::SourceTwo));
        let keyed = scaffold.render();
        let transparent = RgbaImage::from_fn(keyed.width(), keyed.height(), |x, y| {
            let p = *keyed.get_pixel(x, y);
            if p == rgba(KEY, 1.0) {
                Rgba([255, 0, 255, 0])
            } else {
                p
            }
        });
        assert_eq!(
            scaffold.register(&keyed).image,
            scaffold.register(&transparent).image
        );
    }

    #[test]
    fn machines_preserve_painted_alpha_in_footprint_measurement_and_cutting() {
        let scaffold = Scaffold::of(Machine::Glyph(crate::sim::GlyphKind::SourceTwo));
        let mut candidate = scaffold.render();
        let (origin, _) = scaffold.crop();
        candidate.put_pixel(origin, origin, Rgba([255, 0, 0, 0]));
        assert_eq!(scaffold.cut(&candidate).get_pixel(0, 0)[3], 0);
        let capture = Capture {
            image: candidate.clone(),
            off_centre: 0.0,
        };
        assert!(scaffold.score(&capture).outside <= 0.05);
        candidate.put_pixel(origin, origin, Rgba([255, 0, 0, 255]));
        let capture = Capture {
            image: candidate,
            off_centre: 0.0,
        };
        assert!(scaffold.score(&capture).outside > 0.05);
    }

    #[test]
    fn static_art_does_not_depend_on_part_generation() {
        let art = studio("static-split", &["portal", "bonder"]);
        let manifest = Art::shipped().read();
        assert!(!art.dir.join("rig.sh").exists());
        assert!(art.split(&manifest, &["portal".into()]));
        assert!(art.split(&manifest, &["atom-base".into()]));
        assert!(!art.split(&manifest, &["bonder".into()]));
        std::fs::remove_dir_all(art.dir.parent().unwrap()).unwrap();
    }

    #[test]
    fn machine_generator_manifest_round_trip_keeps_every_rig_part_and_motion() {
        let manifest = Art::shipped().read();
        let text = toml::to_string_pretty(&manifest).unwrap();
        let round: Manifest = toml::from_str(&text).unwrap();
        for machine in Machine::ALL {
            let name = name(machine);
            assert_eq!(
                round.machine[name].motion,
                crate::rig::entry(machine).motion
            );
            assert_eq!(
                round.machine[name].parts, manifest.machine[name].parts,
                "{name}"
            );
            assert_eq!(
                round.machine[name].parts.is_empty(),
                machine == Machine::Portal,
                "{name}"
            );
        }
    }
    const KEYED: f32 = 0.01;
    const REGISTERED: f32 = 0.02;

    fn fired(scaffold: &Scaffold, at: &dyn Fn(Vec2) -> Vec2) -> RgbaImage {
        RgbaImage::from_fn(scaffold.canvas, scaffold.canvas, |x, y| {
            let world = at(scaffold.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5)));
            let glaze = scaffold
                .cells
                .iter()
                .find_map(|cell| scaffold.mark(cell, world));
            let ground = if scaffold.covered(world) {
                Glaze::Clay.rgb()
            } else {
                KEY
            };
            rgba(glaze.map_or(ground, Glaze::rgb), 1.0)
        })
    }

    fn shade(n: Vec3) -> f32 {
        (AMBIENT + (1.0 - AMBIENT) * n.dot(light()).max(0.0))
            / (AMBIENT + (1.0 - AMBIENT) * light().z)
    }

    #[test]
    fn machines_palette_accepts_rubber_only_in_the_hand() {
        let scaffold = Scaffold::of(Machine::Arm(crate::sim::ArmLength::Two));
        for role in [Role::Hand, Role::Pivot, Role::Body] {
            let image = RgbaImage::from_fn(scaffold.canvas, scaffold.canvas, |x, y| {
                let world = scaffold.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
                let color = if scaffold.in_cell(world, HEX).is_some_and(|c| c.role == role) {
                    RUBBER
                } else {
                    KEY
                };
                rgba(color, 1.0)
            });
            let score = scaffold.score(&Capture {
                image,
                off_centre: 0.0,
            });
            if role == Role::Hand {
                assert!(score.palette < 0.001, "rubber hand: {}", score.palette);
            } else {
                let expected = Glaze::ALL
                    .iter()
                    .map(|g| apart(RUBBER, g.rgb()))
                    .fold(f32::INFINITY, f32::min);
                assert!((score.palette - expected).abs() < 0.001);
            }
        }
    }

    #[test]
    fn machines_body_only_registration_uniformly_fits_the_silhouette() {
        let scaffold = Scaffold::of(Machine::Portal);
        let candidate = RgbaImage::from_fn(scaffold.canvas, scaffold.canvas, |x, y| {
            let world = scaffold.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
            rgba(
                if look::hex_norm(world.x / HEX, world.y / HEX) < 0.9 {
                    Glaze::Plum.rgb()
                } else {
                    KEY
                },
                1.0,
            )
        });
        let capture = scaffold.register(&candidate);

        assert_eq!(capture.off_centre, 0.0);
        let score = scaffold.score(&capture);
        assert!(score.palette.is_finite(), "body palette is measurable");
        assert!(score.outside <= 0.05);
    }

    #[test]
    fn scaffold_cells_match_the_footprint() {
        for item in Machine::ALL {
            let scaffold = Scaffold::of(item);
            let n = scaffold.canvas as usize;
            let mut covered = Vec::new();
            for q in -4..=4 {
                for r in -4..=4 {
                    let h = Hex::new(q, r);
                    let p = scaffold.pixel(px(h));
                    let inside = p.x >= 0.0 && p.y >= 0.0 && p.x < n as f32 && p.y < n as f32;
                    if inside && scaffold.mask[p.y as usize * n + p.x as usize] == 1.0 {
                        covered.push(h);
                    }
                }
            }
            let mut footprint: Vec<Hex> = match item {
                Machine::Portal => vec![ORIGIN],
                Machine::Arm(length) => Arm::new(length, ORIGIN, 0, Vec::new()).cells(),
                Machine::Glyph(kind) => Glyph {
                    kind,
                    at: ORIGIN,
                    dir: 0,
                    energy: crate::sim::ActivationEnergy::default(),
                }
                .cells()
                .collect(),
            };
            let key = |h: &Hex| (h.q, h.r);
            covered.sort_by_key(key);
            footprint.sort_by_key(key);
            assert_eq!(covered, footprint, "{item:?}");
        }
    }

    #[test]
    fn every_machine_has_a_manifest_entry_a_scaffold_a_kept_candidate_that_passes_and_relief() {
        let manifest = Art::shipped().read();
        let names: Vec<&str> = Machine::ALL.into_iter().map(name).collect();
        assert_eq!(
            manifest
                .machine
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            {
                let mut sorted = names.clone();
                sorted.sort_unstable();
                sorted
            }
        );
        for item in Machine::ALL {
            let name = name(item);
            let machine = &manifest.machine[name];
            let kept = machine
                .kept
                .unwrap_or_else(|| panic!("{name} keeps no candidate"));
            let dir = Art::shipped().machine(name);
            let scaffold = Scaffold::of(item);
            let want = scaffold.render();
            let shipped = open(dir.join("scaffold.png"));
            assert!(
                want.as_raw() == shipped.as_raw(),
                "{name}/scaffold.png is stale: regenerate it"
            );
            let capture =
                scaffold.register(&open(dir.join(format!("candidates/{name}-{kept}.png"))));
            let score = scaffold.score(&capture);
            assert!(
                score.measured(&manifest.thresholds).is_none(),
                "{name}: {score:?}"
            );
            let rows = std::fs::read_to_string(dir.join("scores.tsv")).expect("scores.tsv");
            let judged = rows
                .lines()
                .filter_map(Score::parse)
                .find(|(label, _)| *label == format!("{name}-{kept}"))
                .and_then(|(_, s)| s.judged)
                .unwrap_or_else(|| panic!("{name}-{kept} has no critic score in scores.tsv"));
            assert!(judged.critic.score <= 10, "{name}-{kept}: {judged:?}");
            assert!(!judged.critic.compound, "{name}-{kept} reads as a compound");
            if !name.starts_with("arm") {
                assert!(
                    machine.relit.is_some(),
                    "{name} has no computed calibration set"
                );
            }
            assert_eq!(
                judged.key,
                judged_key(
                    name,
                    &manifest.style.critic,
                    &read(&dir.join(format!("candidates/{name}-{kept}.png")))
                ),
                "{name}: the judgement is stale against the candidate or the critic prompt: run ziral --gen {name}"
            );
            let round = (kept as usize - 1) / manifest.candidates as usize + 1;
            assert!(
                dir.join(format!("candidates/round-{round}.txt")).exists(),
                "{name}: candidates/round-{round}.txt, the prompt that painted {name}-{kept}, is missing"
            );
            let (_, side) = scaffold.crop();
            let albedo = open(dir.join("albedo.png"));
            let cut = scaffold.cut(&capture.image);
            assert_eq!(
                (albedo.width(), albedo.height()),
                (side, side),
                "{name}/albedo.png"
            );
            for map in ["albedo", "normal"] {
                let png = open(dir.join(format!("{map}.png")));
                let mut drift = Mean::default();
                for (shipped, keyed) in png.pixels().zip(cut.pixels()) {
                    drift.add((f32::from(shipped[3]) - f32::from(keyed[3])).abs() / 255.0);
                }
                assert!(
                    drift.value() <= KEYED,
                    "{name}/{map}.png is not the kept candidate keyed: alpha drifts {:.3}",
                    drift.value()
                );
            }
            for map in ["albedo", "normal"] {
                let png = open(dir.join(format!("{map}.png")));
                let placed = scaffold.quad.size();
                let aspect = png.width() as f32 / png.height() as f32 / (placed.x / placed.y);
                assert!(
                    (aspect - 1.0).abs() <= ASPECT,
                    "{name}/{map}.png is {}x{} on a {placed} quad",
                    png.width(),
                    png.height()
                );
                assert_eq!(
                    (png.width(), png.height()),
                    (side, side),
                    "{name}/{map}.png"
                );
            }
            let normal = open(dir.join("normal.png"));
            let tilted = normal
                .pixels()
                .filter(|p| decode(p).truncate().length() > RELIEF)
                .count();
            assert!(
                tilted as f32 > normal.pixels().len() as f32 * 0.01,
                "{name}/normal.png is flat"
            );
            let prompt = stored(&Art::shipped().prompt(name))
                .expect("prompt.txt is readable")
                .unwrap_or_else(|| panic!("{name} has no prompt.txt: run ziral --gen {name}"));
            let references: Vec<PathBuf> = machine
                .references
                .iter()
                .map(|reference| Art::shipped().dir.join(reference))
                .collect();
            assert_eq!(
                machine.briefed.as_deref(),
                Some(
                    input_key(
                        &brief(name, &manifest.style, &machine.direction),
                        &references
                    )
                    .as_str()
                ),
                "{name}: the prompt was written from another brief: run ziral --gen {name}"
            );
            assert_eq!(
                machine.painted.as_deref(),
                Some(painted_key(&prompt, manifest.candidates, &want, &references,).as_str(),),
                "{name}: the candidates are stale against the prompt: run ziral --gen {name}"
            );
            let kept_png = read(&dir.join(format!("candidates/{name}-{kept}.png")));
            let facings = &manifest.style.facings;
            if let Some(relit) = &machine.relit {
                assert_eq!(
                    relit,
                    &relit_key(&kept_png, &manifest.style, facings),
                    "{name}: the maps are stale against the kept candidate: run ziral --gen {name}"
                );
                let atlas = open(dir.join("relit/albedo.png"));
                let albedo = open(dir.join("albedo.png"));
                assert_eq!(atlas.width(), albedo.width(), "{name}");
                assert_eq!(
                    atlas.height(),
                    albedo.height() * manifest.style.facings.len() as u32
                );
                let normal = scaffold.surface_normals(&capture.image);
                assert!(
                    open(dir.join("normal.png")).as_raw() == scaffold.cropped(&normal).as_raw(),
                    "{name}: shipped normals differ from the computed relief"
                );
                let edits: Vec<_> = facings
                    .iter()
                    .map(|facing| {
                        let rendered = open(dir.join(format!("relit/{}.png", facing.name())));
                        let expected = scaffold.render_light(
                            &capture.image,
                            &normal,
                            facing.light(manifest.style.elevation),
                        );
                        assert!(
                            rendered.as_raw() == expected.as_raw(),
                            "{name}/{}: machine and sphere must be one computed render",
                            facing.name()
                        );
                        rendered
                    })
                    .collect();
                let measured = scaffold.calibration(&edits);
                assert!(
                    direction_error(facings, manifest.style.elevation, &measured.lights)
                        <= manifest.thresholds.sphere,
                    "{name}: light direction"
                );
                assert!(
                    measured.sphere <= manifest.thresholds.sphere,
                    "{name}: sphere shape"
                );
            }
        }
    }

    #[test]
    fn the_manifest_keys_six_facing_reliefs_for_every_rotatable_texture() {
        let art = Art::shipped();
        let manifest = art.read();
        assert_eq!(manifest.style.facings, Facing::ALL);
        assert_eq!(manifest.style.elevation, 45.0);
        let names: Vec<&str> = manifest.texture.keys().map(String::as_str).collect();
        assert_eq!(
            names,
            [
                "atom-amber",
                "atom-base",
                "atom-cobalt",
                "atom-plum",
                "bond-double",
                "bond-single"
            ]
        );
        for (name, entry) in &manifest.texture {
            let source = art.dir.join("..").join(&entry.source);
            assert!(source.exists(), "{name}");
            if let Some(relit) = &entry.relit {
                let dir = art.texture(name).join("relit");
                assert_eq!(
                    relit,
                    &relit_key(
                        open(&source).as_raw(),
                        &manifest.style,
                        &manifest.style.facings,
                    ),
                    "{name}"
                );
                assert!(dir.join("albedo.png").exists(), "{name}");
                assert!(dir.join("normal.png").exists(), "{name}");
            }
        }
    }

    #[test]
    fn relight_keys_change_with_the_pixels_elevation_and_facings() {
        let mut style = Art::shipped().read().style;
        let original = relit_key(b"pixels", &style, &style.facings);
        assert_ne!(relit_key(b"other pixels", &style, &style.facings), original);
        style.elevation += 1.0;
        assert_ne!(relit_key(b"pixels", &style, &style.facings), original);
        style.elevation -= 1.0;
        style.facings.swap(0, 1);
        assert_ne!(relit_key(b"pixels", &style, &style.facings), original);
    }

    #[test]
    fn a_relight_set_must_match_every_requested_direction() {
        let style = Art::shipped().read().style;
        let lights: Vec<Light> = style
            .facings
            .iter()
            .map(|facing| Light {
                direction: facing.light(style.elevation),
                ambient: AMBIENT,
            })
            .collect();
        assert!(direction_error(&style.facings, style.elevation, &lights) < 0.1);
        let mut swapped = lights;
        swapped.swap(0, 1);
        assert!(direction_error(&style.facings, style.elevation, &swapped) > 25.0);
    }

    #[test]
    fn a_candidate_painted_beyond_its_footprint_is_rejected() {
        let scaffold = Scaffold::of(Machine::Glyph(crate::sim::GlyphKind::Bonder));
        let thresholds = Art::shipped().read().thresholds;
        let clean = fired(&scaffold, &|w| w);
        let score = scaffold.score(&scaffold.register(&clean));
        assert!(score.measured(&thresholds).is_none(), "{score:?}");
        assert!(score.outside <= SEAT_STEP, "{score:?}");
        let face = px(scaffold.cells[0].at) - Vec2::X * HEX * 3f32.sqrt() / 2.0;
        let dot = 2.0;
        for (k, ok) in [(0.5, true), (1.5, false)] {
            let at = face - Vec2::X * k * thresholds.outside * HEX;
            assert!(
                (scaffold.outside(at) - k * thresholds.outside).abs() < SEAT_STEP,
                "{at} lies {} beyond the footprint",
                scaffold.outside(at)
            );
            let mut painted = clean.clone();
            let centre = scaffold.pixel(at);
            for (x, y, p) in painted.enumerate_pixels_mut() {
                if Vec2::new(x as f32 + 0.5, y as f32 + 0.5).distance(centre) <= dot {
                    *p = rgba(Glaze::Brass.rgb(), 1.0);
                }
            }
            let score = scaffold.score(&scaffold.register(&painted));
            assert_eq!(score.measured(&thresholds).is_none(), ok, "{score:?}");
            assert!(
                (score.outside - k * thresholds.outside).abs() <= SEAT_STEP,
                "{score:?} against {k} tolerances"
            );
        }
    }

    #[test]
    fn a_capture_whose_seats_sit_off_their_cell_centres_is_rejected() {
        let scaffold = Scaffold::of(Machine::Glyph(crate::sim::GlyphKind::Converter(
            crate::sim::AtomKind::Amber,
        )));
        let thresholds = Art::shipped().read().thresholds;
        let drift = |a: &RgbaImage, b: &RgbaImage| {
            let mut mean = Mean::default();
            for (p, q) in a.pixels().zip(b.pixels()) {
                mean.add(apart(rgb(p), rgb(q)));
            }
            mean.value()
        };
        let clean = fired(&scaffold, &|w| w);
        let score = scaffold.score(&scaffold.register(&clean));
        assert!(score.measured(&thresholds).is_none(), "{score:?}");
        assert!(score.off_centre <= SEAT_STEP, "{score:?}");
        let shift = Vec2::new(2.0, -1.0).normalize() * thresholds.off_centre * 2.0 * HEX;
        let centroid = scaffold.quad.centre;
        for (name, moved) in [
            ("shifted", fired(&scaffold, &|w| w - shift)),
            (
                "spread",
                fired(&scaffold, &|w| centroid + (w - centroid) / 1.15),
            ),
        ] {
            let capture = scaffold.register(&moved);
            let score = scaffold.score(&capture);
            assert!(score.measured(&thresholds).is_none(), "{name}: {score:?}");
            assert!(score.off_centre <= SEAT_STEP, "{name}: {score:?}");
            let drift = drift(&capture.image, &clean);
            assert!(
                drift <= REGISTERED,
                "{name} registers {drift:.3} from clean"
            );
        }
        let turn = Vec2::from_angle(-shift.length() / HEX);
        let turned =
            scaffold.register(&fired(&scaffold, &|w| centroid + turn.rotate(w - centroid)));
        let score = scaffold.score(&turned);
        assert!(score.off_centre > thresholds.off_centre, "{score:?}");
        assert!(score.off_centre <= shift.length() / HEX * 1.5, "{score:?}");
        assert!(score.measured(&thresholds).is_some(), "{score:?}");
        let off = Score {
            off_centre: score.off_centre,
            ..scaffold.score(&scaffold.register(&clean))
        };
        assert!(
            off.measured(&thresholds).is_some(),
            "{off:?} passes on off-centre seats alone"
        );
        let beyond = Vec2::X * (SEAT_SEARCH + 2.0 * thresholds.off_centre) * HEX;
        let lost = scaffold.score(&scaffold.register(&fired(&scaffold, &|w| w - beyond)));
        assert!(
            lost.off_centre > thresholds.off_centre,
            "{lost:?} beyond the search reach"
        );
    }

    #[test]
    fn a_rotated_sprite_keeps_its_lit_side_on_the_world_light() {
        let r = 64;
        let centroid = |turn: f32| {
            let rot = Vec2::from_angle(turn);
            let mut sum = Vec2::ZERO;
            let mut weight = 0.0;
            for y in -r..r {
                for x in -r..r {
                    let d = Vec2::new(x as f32, y as f32) / r as f32;
                    let rr = d.length_squared();
                    if rr >= 1.0 {
                        continue;
                    }
                    let n = Vec3::new(d.x, d.y, (1.0 - rr).sqrt());
                    let world = rot.rotate(n.truncate()).extend(n.z);
                    let s = shade(world);
                    sum += rot.rotate(d) * s;
                    weight += s;
                }
            }
            (sum / weight).normalize()
        };
        let light = light().truncate().normalize();
        for k in 0..6 {
            let c = centroid(look::turn(k));
            assert!(c.dot(light) > 0.99, "turn {k}: lit side at {c:?}");
        }
        assert!(include_str!("lit.wgsl").contains("mesh.world_tangent.xy"));
    }

    #[test]
    fn computed_relights_shade_machine_and_calibration_with_the_same_light() {
        let scaffold = Scaffold::texture(128);
        let candidate =
            RgbaImage::from_pixel(scaffold.canvas, scaffold.canvas, grey(SPHERE_ALBEDO));
        let centre = Vec2::splat(scaffold.canvas as f32 / 2.0);
        let normal = RgbaImage::from_fn(scaffold.canvas, scaffold.canvas, |x, y| {
            encode(ball(centre, 48.0, x, y).unwrap_or(Vec3::Z), 1.0)
        });
        let facings = Facing::ALL;
        let edits: Vec<_> = facings
            .iter()
            .map(|facing| {
                let light = facing.light(45.0);
                let rendered = scaffold.render_light(&candidate, &normal, light);
                for (x, y) in [(200, 190), (180, 175), (205, 210)] {
                    let n = decode(normal.get_pixel(x, y));
                    let want = SPHERE_ALBEDO * (AMBIENT + (1.0 - AMBIENT) * n.dot(light).max(0.0));
                    assert!((luminance(rendered.get_pixel(x, y)) - want).abs() <= 1.0 / 255.0);
                }
                rendered
            })
            .collect();
        let calibration = scaffold.calibration(&edits);
        assert!(direction_error(&facings, 45.0, &calibration.lights) < 0.2);
        assert!(calibration.sphere < Art::shipped().read().thresholds.sphere);
        assert!(direction_error(&facings, 20.0, &calibration.lights) > 24.0);
        let mut changed = edits;
        changed.swap(0, 3);
        assert!(direction_error(&facings, 45.0, &scaffold.calibration(&changed).lights) > 25.0);
    }

    #[test]
    fn a_compound_fails_even_with_a_perfect_critic_score() {
        let scaffold = Scaffold::of(item("bonder"));
        let mut score = scaffold.score(&scaffold.register(&fired(&scaffold, &|w| w)));
        score.judged = Some(Judged {
            key: "test".into(),
            critic: Critic {
                compound: true,
                score: 10,
                issues: vec![],
            },
        });
        let thresholds = Art::shipped().read().thresholds;
        assert!(score.measured(&thresholds).is_none());
        assert!(!score.passes(&thresholds));
        assert!(!score.reaches(&thresholds));
        let (_, recovered) = Score::parse(&score.row("bonder-1", &thresholds)).unwrap();
        assert!(!recovered.passes(&thresholds));
        assert!(Critic::parse(r#"{"score":10,"issues":[]}"#).is_none());
    }

    #[test]
    fn bounded_measurement_matches_the_exhaustive_pixel_and_hex_walks_bit_for_bit() {
        for name in ["arm", "bonder", "source", "output-3"] {
            let scaffold = Scaffold::of(item(name));
            let candidate = RgbaImage::from_fn(scaffold.canvas, scaffold.canvas, |x, y| {
                Rgba([(x % 251) as u8, (y % 241) as u8, ((x + y) % 239) as u8, 255])
            });
            let marks: Vec<_> = scaffold.marked().collect();
            for index in [0, marks.len() / 2, marks.len() - 1] {
                let cell = marks[index];
                let bounded = scaffold.seat_contrast(&candidate, cell, scaffold.seat_bounds(cell));
                let exhaustive = scaffold.seat_contrast(
                    &candidate,
                    cell,
                    [0, 0, scaffold.canvas, scaffold.canvas],
                );
                assert_eq!(bounded.to_bits(), exhaustive.to_bits(), "{name}/{index}");
            }
            for x in -20..=20 {
                for y in -20..=20 {
                    let world = Vec2::new(x as f32, y as f32) * HEX * 0.4;
                    let exhaustive = scaffold
                        .cells
                        .iter()
                        .map(|cell| hex_distance(world - px(cell.at), HEX))
                        .fold(f32::INFINITY, f32::min)
                        / HEX;
                    assert_eq!(
                        scaffold.outside(world).to_bits(),
                        exhaustive.to_bits(),
                        "{name}/{world}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_failed_measurement_rebriefs_the_next_round_without_resetting_the_budget() {
        let art = studio("measured-feedback", &["bonder"]);
        let briefs = std::sync::Mutex::new(Vec::new());
        let director = |_: &[PathBuf], text: &str| {
            briefs.lock().unwrap().push(text.to_string());
            Ok(text.to_string())
        };
        let paints = std::sync::atomic::AtomicUsize::new(0);
        let painter = |job: &Paint| {
            paints.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let index = job
                .output
                .file_stem()
                .unwrap()
                .to_str()
                .unwrap()
                .rsplit('-')
                .next()
                .unwrap()
                .parse::<u32>()
                .unwrap();
            if index <= 2 {
                let scaffold = Scaffold::of(item("bonder"));
                save(&fired(&scaffold, &|w| w - Vec2::X * HEX * 2.0), &job.output);
                Ok(())
            } else {
                fake(job)
            }
        };
        assert!(landed(&remake(
            &art,
            &["bonder".to_string()],
            &director,
            &painter,
            &judge
        )));
        let briefs = briefs.lock().unwrap();
        assert_eq!(briefs.len(), 2);
        assert!(briefs[1].contains("outside ") && briefs[1].contains("> 0.05"));
        assert_eq!(paints.load(std::sync::atomic::Ordering::SeqCst), 4);
        assert!(art.read().machine["bonder"].kept.unwrap() >= 3);
    }

    fn studio(tag: &str, machines: &[&str]) -> Art {
        let shipped = Art::shipped();
        let root = std::env::temp_dir().join(format!("ziral-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let art = Art {
            dir: root.join("machines"),
        };
        std::fs::create_dir_all(&art.dir).expect("a studio is creatable");
        std::fs::copy(shipped.paint_sh(), art.paint_sh()).expect("paint.sh copies");
        art.write(&Manifest {
            candidates: 2,
            style: shipped.read().style,
            thresholds: shipped.read().thresholds,
            machine: machines
                .iter()
                .map(|name| {
                    (
                        name.to_string(),
                        Entry {
                            direction: format!("a {name}"),
                            references: Vec::new(),
                            kept: None,
                            briefed: None,
                            painted: None,
                            relit: None,
                            motion: None,
                            instrument: crate::sound::instrument(item(name)),
                            emitter: crate::rig::entry(item(name)).emitter,
                            rig_emitter: crate::rig::entry(item(name)).rig_emitter,
                            parts: Vec::new(),
                        },
                    )
                })
                .collect(),
            texture: BTreeMap::new(),
        });
        art
    }

    fn fake(job: &Paint) -> Result<(), String> {
        let stem = job
            .output
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("an output stem");
        let input = job.images.first().expect("a paint input");
        assert!(
            input.ends_with("scaffold.png"),
            "only albedo candidates reach the painter"
        );
        let (name, index) = stem.rsplit_once('-').expect("name-index");
        let scaffold = Scaffold::of(item(name));
        let count = Art {
            dir: input
                .parent()
                .and_then(Path::parent)
                .expect("the art dir")
                .into(),
        }
        .read()
        .candidates;
        let shift = Vec2::X * ((index.parse::<u32>().expect("an index") - 1) % count) as f32;
        save(&fired(&scaffold, &|w| w - shift), &job.output);
        Ok(())
    }

    fn author(_: &[PathBuf], brief: &str) -> Result<String, String> {
        Ok(brief.to_string())
    }

    fn judge(_: &[PathBuf], _: &str) -> Result<String, String> {
        Ok(r#"{"compound": false, "score": 10, "issues": []}"#.to_string())
    }

    fn counted<'a>(
        calls: &'a std::sync::atomic::AtomicUsize,
        painter: &'a (dyn Fn(&Paint) -> Result<(), String> + Sync),
    ) -> impl Fn(&Paint) -> Result<(), String> + Sync + 'a {
        move |job| {
            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            painter(job)
        }
    }

    #[test]
    fn unchanged_inputs_are_skipped_and_a_changed_prompt_or_kept_is_remade() {
        let art = studio("key", &["source"]);
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let painter = counted(&calls, &fake);
        let names = ["source".to_string()];
        let run = |calls: &std::sync::atomic::AtomicUsize| {
            calls.store(0, std::sync::atomic::Ordering::SeqCst);
            let results = remake(&art, &names, &author, &painter, &judge);
            assert_eq!(results.len(), 1, "{results:?}");
            let changed = results[0].1.clone().expect("source lands");
            (changed, calls.load(std::sync::atomic::Ordering::SeqCst))
        };
        assert_eq!(run(&calls), (true, 2));
        let dir = art.machine("source");
        for made in [
            "scaffold.png",
            "candidates/source-1.png",
            "candidates/source-2.png",
            "scores.tsv",
            "albedo.png",
            "normal.png",
            "relit/lights.txt",
            "../sheet.png",
        ] {
            assert!(dir.join(made).exists(), "{made}");
        }
        let manifest = std::fs::read_to_string(art.manifest()).expect("a manifest");
        let kept = art.read().machine["source"].kept.expect("a kept candidate");
        let albedo = read(&dir.join("albedo.png"));
        let sheet = |dir: &Path| {
            std::fs::metadata(dir.join("../sheet.png"))
                .and_then(|m| m.modified())
                .expect("a sheet")
        };
        let made = sheet(&dir);
        assert_eq!(run(&calls), (false, 0));
        assert_eq!(sheet(&dir), made);
        assert_eq!(
            std::fs::read_to_string(art.manifest()).expect("a manifest"),
            manifest
        );
        assert_eq!(read(&dir.join("albedo.png")), albedo);
        let mut m = art.read();
        m.thresholds.outside += 0.01;
        art.write(&m);
        assert_eq!(run(&calls), (false, 0));
        let mut m = art.read();
        m.machine.get_mut("source").expect("source").direction = "another source".into();
        art.write(&m);
        assert_eq!(run(&calls), (true, 2));
        assert_eq!(
            stored(&art.prompt("source")).expect("readable").as_deref(),
            Some(brief("source", &art.read().style, "another source").trim_end())
        );
        store(&art.prompt("source"), "a hand-written caption").expect("writable");
        assert_eq!(run(&calls), (true, 2));
        assert_eq!(
            stored(&art.prompt("source")).expect("readable").as_deref(),
            Some("a hand-written caption")
        );
        assert_eq!(run(&calls), (false, 0));
        let mut m = art.read();
        m.machine.get_mut("source").expect("source").kept = Some(3 - kept);
        art.write(&m);
        assert_eq!(run(&calls), (true, 0));
        assert_eq!(art.read().machine["source"].kept, Some(3 - kept));
        assert_ne!(read(&dir.join("albedo.png")), albedo);
        assert_eq!(run(&calls), (false, 0));
    }

    #[test]
    fn only_candidates_are_painted_and_relights_are_computed() {
        let art = studio("scaffold", &["bonder", "source"]);
        let jobs: std::sync::Mutex<Vec<(String, Vec<PathBuf>)>> = std::sync::Mutex::new(Vec::new());
        let painter = |job: &Paint| {
            let stem = job
                .output
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            jobs.lock().unwrap().push((stem, job.images.clone()));
            fake(job)
        };
        let names = ["bonder".to_string(), "source".to_string()];
        assert!(landed(&remake(&art, &names, &author, &painter, &judge)));
        let recorded = jobs.lock().unwrap();
        let style = art.read().style;
        for name in ["bonder", "source"] {
            let dir = art.machine(name);
            let facings = &style.facings;
            let candidates: Vec<_> = recorded
                .iter()
                .filter(|(stem, _)| {
                    stem.starts_with(&format!("{name}-"))
                        && stem[name.len() + 1..].parse::<u32>().is_ok()
                })
                .collect();
            assert_eq!(candidates.len(), 2, "{name}");
            for (_, images) in &candidates {
                assert_eq!(*images, [dir.join("scaffold.png")], "{name}");
            }
            let relights: Vec<_> = recorded
                .iter()
                .filter(|(stem, images)| {
                    facings.iter().any(|facing| facing.name() == stem)
                        && *images == [dir.join("relit/master.png")]
                })
                .collect();
            assert!(
                relights.is_empty(),
                "{name}: relights must never reach the painter"
            );
        }
    }

    #[test]
    fn a_machine_whose_candidates_all_fail_to_paint_does_not_land_and_the_others_still_do() {
        let art = studio("exit", &["bonder", "source"]);
        let painter = |job: &Paint| {
            if job.output.to_string_lossy().contains("bonder") {
                Err("boom".to_string())
            } else {
                fake(job)
            }
        };
        let names = ["bonder".to_string(), "source".to_string()];
        let results = remake(&art, &names, &author, &painter, &judge);
        assert!(!landed(&results));
        let by_name = |results: &[(String, Result<bool, String>)], name: &str| {
            results
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, r)| r.clone())
                .unwrap_or_else(|| panic!("{name} in {results:?}"))
        };
        assert_eq!(by_name(&results, "source"), Ok(true));
        let reason = by_name(&results, "bonder").expect_err("bonder did not land");
        assert!(
            reason.contains("no candidate passes")
                && reason.contains("2 of 2 paints failed")
                && reason.contains("bonder-1: boom"),
            "{reason}"
        );
        assert_eq!(art.read().machine["bonder"].kept, None);
        assert!(art.read().machine["source"].kept.is_some());
        let results = remake(&art, &names, &author, &fake, &judge);
        assert!(landed(&results), "{results:?}");
        assert_eq!(by_name(&results, "source"), Ok(false));
        assert_eq!(by_name(&results, "bonder"), Ok(true));
        assert!(art.read().machine["bonder"].kept.is_some());
        assert!(landed(&remake(&art, &names[1..], &author, &fake, &judge)));
        assert!(landed(&[]));
    }

    fn rows(art: &Art, name: &str) -> BTreeMap<String, (String, Score)> {
        std::fs::read_to_string(art.machine(name).join("scores.tsv"))
            .expect("scores.tsv")
            .lines()
            .map(|row| {
                let (label, score) = Score::parse(row).unwrap_or_else(|| panic!("{row:?}"));
                let verdict = row.split('\t').nth(6).expect("a verdict").to_string();
                (label, (verdict, score))
            })
            .collect()
    }

    fn index(images: &[PathBuf]) -> u32 {
        images[0]
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.rsplit('-').next())
            .and_then(|i| i.parse().ok())
            .expect("judged/NAME-INDEX.png")
    }

    #[test]
    fn the_critic_reply_is_one_json_object_or_nothing() {
        let parsed = |text: &str| Critic::parse(text);
        assert_eq!(
            parsed(
                "```json\n{\"compound\": false, \"score\": 7, \"issues\": [\"The cup at the left shows its far wall.\"]}\n```"
            ),
            Some(Critic {
                compound: false,
                score: 7,
                issues: vec!["The cup at the left shows its far wall.".to_string()],
            })
        );
        assert_eq!(
            parsed(r#"{"compound": false, "score": 10, "issues": []}"#),
            Some(Critic {
                compound: false,
                score: 10,
                issues: vec![]
            })
        );
        assert_eq!(
            parsed(r#"{"compound": false, "score": 11, "issues": []}"#),
            None
        );
        assert_eq!(parsed(r#"{"compound": false, "score": 8}"#), None);
        assert_eq!(parsed(r#"{"score": "eight", "issues": []}"#), None);
        assert_eq!(parsed("A fine machine, 9/10."), None);
        assert_eq!(parsed(""), None);
        let six = r#"{"compound": false, "score": 3, "issues": ["a", "b", "c", "d", "e", "f"]}"#;
        assert_eq!(parsed(six).expect("parses").issues.len(), 5);
    }

    #[test]
    fn the_critic_sees_the_board_and_the_scaffold_and_a_malformed_reply_fails_only_that_candidate()
    {
        let art = studio("critic", &["source"]);
        let seen: std::sync::Mutex<Vec<(Vec<PathBuf>, String)>> = std::sync::Mutex::new(vec![]);
        let critic = |images: &[PathBuf], prompt: &str| {
            seen.lock()
                .unwrap()
                .push((images.to_vec(), prompt.to_string()));
            Ok(match index(images) {
                1 => r#"{"compound": false, "score": 9, "issues": ["Slight glare on the rim."]}"#
                    .to_string(),
                _ => "I would rather not say.".to_string(),
            })
        };
        let names = ["source".to_string()];
        let results = remake(&art, &names, &author, &fake, &critic);
        assert!(landed(&results), "{results:?}");
        assert_eq!(art.read().machine["source"].kept, Some(1));
        let rows = rows(&art, "source");
        assert_eq!(rows["source-1"].0, "pass");
        assert_eq!(
            rows["source-1"].1.judged,
            Some(Judged {
                key: judged_key(
                    "source",
                    &art.read().style.critic,
                    &read(&art.candidate("source", 1))
                ),
                critic: Critic {
                    compound: false,
                    score: 9,
                    issues: vec!["Slight glare on the rim.".to_string()],
                },
            })
        );
        assert_eq!(rows["source-2"].0, "pass");
        assert_eq!(rows["source-2"].1.judged, None);
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        let style = art.read().style;
        for (images, prompt) in seen.iter() {
            assert_eq!(*prompt, style.critic);
            let i = index(images);
            assert_eq!(
                *images,
                vec![
                    art.judged("source", i),
                    art.machine("source").join("scaffold.png")
                ]
            );
            let board = open(&images[0]);
            let scaffold = Scaffold::of(item("source"));
            let side = scaffold.quad.side.round() as u32;
            let by = CRITIC_PX.div_ceil(side);
            assert_eq!((board.width(), board.height()), (side * by, side * by));
            assert!(
                board.pixels().all(|p| p[3] == 255),
                "the board shows through nowhere"
            );
            let corner = *board.get_pixel(0, 0);
            assert!(
                board.pixels().any(|p| apart(rgb(p), rgb(&corner)) > 0.2),
                "the sprite is not on the board"
            );
        }
    }

    #[test]
    fn a_candidate_at_the_target_ends_the_rounds_and_the_best_scored_one_is_kept() {
        let art = studio("bar", &["source"]);
        let critic = |images: &[PathBuf], _: &str| {
            Ok(match index(images) {
                1 => r#"{"compound": false, "score": 7, "issues": ["The hopper shows its back wall."]}"#,
                _ => r#"{"compound": false, "score": 8, "issues": []}"#,
            }
            .to_string())
        };
        let names = ["source".to_string()];
        assert!(landed(&remake(&art, &names, &author, &fake, &critic)));
        assert_eq!(art.read().machine["source"].kept, Some(2));
        let rows = rows(&art, "source");
        assert_eq!(rows["source-1"].0, "pass");
        assert_eq!(rows["source-2"].0, "pass");
        let art = studio("rank", &["source"]);
        let scaffold = Scaffold::of(item("source"));
        let measured = |shift: f32| {
            scaffold
                .score(&scaffold.register(&fired(&scaffold, &|w| w - Vec2::X * shift)))
                .rank()
                .1
        };
        let lower = if measured(0.0) < measured(1.0) { 1 } else { 2 };
        let critic = |images: &[PathBuf], _: &str| {
            Ok(if index(images) == lower {
                r#"{"compound": false, "score": 10, "issues": []}"#
            } else {
                r#"{"compound": false, "score": 9, "issues": []}"#
            }
            .to_string())
        };
        assert!(landed(&remake(&art, &names, &author, &fake, &critic)));
        assert_eq!(art.read().machine["source"].kept, Some(lower));
    }

    #[test]
    fn the_best_candidates_issues_rebrief_the_author_then_the_last_round_starts_again() {
        let art = studio("revise", &["source"]);
        let direction = "  art direction with a tab\t, a line\nbreak and trailing spaces   ";
        let mut m = art.read();
        m.machine.get_mut("source").expect("source").direction = direction.to_string();
        art.write(&m);
        let briefs: std::sync::Mutex<Vec<(Vec<PathBuf>, String)>> = std::sync::Mutex::new(vec![]);
        let author = |images: &[PathBuf], text: &str| {
            briefs
                .lock()
                .unwrap()
                .push((images.to_vec(), text.to_string()));
            Ok(format!("a caption from: {}", text.trim()))
        };
        let prompts: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(vec![]);
        let painter = |job: &Paint| {
            if job
                .images
                .first()
                .is_some_and(|image| image.ends_with("scaffold.png"))
            {
                prompts
                    .lock()
                    .unwrap()
                    .push(stored(&job.prompt_path).unwrap().unwrap());
            }
            fake(job)
        };
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let critic = |images: &[PathBuf], _: &str| {
            let call = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(if call < 4 {
                match index(images) {
                    1 => r#"{"compound": false, "score": 3, "issues": ["Worse first.", "Worse second."]}"#,
                    _ => r#"{"compound": false, "score": 5, "issues": ["Tipped: the hopper shows its back wall.", "A plate under it.", "Text on the gate."]}"#,
                }
            } else {
                r#"{"compound": false, "score": 10, "issues": []}"#
            }
            .to_string())
        };
        let names = ["source".to_string()];
        let results = remake(&art, &names, &author, &painter, &critic);
        assert!(landed(&results), "{results:?}");
        let style = art.read().style;
        let base = brief("source", &style, direction);
        let first = format!("a caption from: {}", base.trim());
        let briefs = briefs.lock().unwrap();
        let scaffold = art.machine("source").join("scaffold.png");
        let briefs: Vec<_> = briefs
            .iter()
            .filter(|(images, _)| *images == [scaffold.clone()])
            .collect();
        assert_eq!(briefs.len(), 3, "{briefs:?}");
        assert_eq!(briefs[0].1, base);
        assert!(briefs[0].1.contains(direction));
        assert!(briefs[0].1.contains("| cobalt |"));
        assert!(briefs[0].1.contains("| charcoal rubber |"));
        assert_eq!(
            briefs[1].1,
            format!(
                "{base} The current prompt: \"{first}\" The best candidate has these measured or visual issues, most important first: Tipped: the hopper shows its back wall. A plate under it. Text on the gate. Rewrite the prompt so the next picture fixes them."
            )
        );
        assert_eq!(
            briefs[2].1,
            format!(
                "{base} Write a new prompt from this brief alone: the whole object's straight-down gameplay read and its seat layout come first, and any detail that competes with them is simplified or left out."
            )
        );
        let prompts = prompts.lock().unwrap();
        assert_eq!(prompts.len(), 6, "{prompts:?}");
        assert_eq!(&prompts[..2], &[first.clone(), first.clone()]);
        let second = format!("a caption from: {}", briefs[1].1.trim());
        assert_eq!(&prompts[2..4], &[second.clone(), second.clone()]);
        let third = format!("a caption from: {}", briefs[2].1.trim());
        assert_eq!(&prompts[4..], &[third.clone(), third.clone()]);
        assert_eq!(
            stored(&art.prompt("source")).expect("readable").as_deref(),
            Some(first.as_str())
        );
        for (round, prompt) in [(1, &first), (2, &second), (3, &third)] {
            assert_eq!(
                stored(&art.round("source", round))
                    .expect("readable")
                    .as_deref(),
                Some(prompt.as_str())
            );
        }
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 6);
        assert_eq!(art.read().machine["source"].direction, direction);
        assert!(art.read().machine["source"].kept.is_some());
    }

    #[test]
    fn three_rounds_then_keep_the_best_of_every_round_with_its_score_and_issues() {
        let art = studio("rounds", &["source"]);
        let paints = std::sync::atomic::AtomicUsize::new(0);
        let prompts: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(vec![]);
        let painter = |job: &Paint| {
            if job
                .images
                .first()
                .is_some_and(|image| image.ends_with("scaffold.png"))
            {
                prompts
                    .lock()
                    .unwrap()
                    .push(stored(&job.prompt_path).unwrap().unwrap());
            }
            fake(job)
        };
        let painter = counted(&paints, &painter);
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let critic = |images: &[PathBuf], _: &str| {
            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let round = (index(images) - 1) / 2 + 1;
            Ok(match index(images) {
                1 => r#"{"compound": false, "score": 5, "issues": ["Low.", "Still tipped."]}"#.to_string(),
                2 => r#"{"compound": false, "score": 7, "issues": ["Round 1 best.", "Still a plate."]}"#.to_string(),
                _ => format!(r#"{{"compound": false, "score": 6, "issues": ["Round {round} worse."]}}"#),
            })
        };
        let names = ["source".to_string()];
        let results = remake(&art, &names, &author, &painter, &critic);
        assert!(landed(&results), "{results:?}");
        assert_eq!(paints.load(std::sync::atomic::Ordering::SeqCst), 6);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 6);
        assert_eq!(art.read().machine["source"].kept, Some(2));
        assert!(art.machine("source").join("albedo.png").exists());
        let scored = rows(&art, "source");
        assert_eq!(scored.len(), 6);
        for i in 1..=6 {
            let row = &scored[&format!("source-{i}")];
            assert_eq!(row.0, "pass", "source-{i}");
            assert!(row.1.judged.is_some(), "source-{i}");
        }
        assert_eq!(scored["source-2"].1.rank().0, 7);
        assert_eq!(
            scored["source-2"].1.issues(),
            ["Round 1 best.", "Still a plate."]
        );
        assert_eq!(scored["source-6"].1.issues(), ["Round 3 worse."]);
        let first = brief("source", &art.read().style, "a source")
            .trim_end()
            .to_string();
        assert_eq!(
            stored(&art.prompt("source")).expect("readable").as_deref(),
            Some(first.as_str())
        );
        let painted = prompts.lock().unwrap();
        assert_eq!(painted.len(), 6, "{painted:?}");
        assert_eq!(&painted[..2], &[first.clone(), first.clone()]);
        assert!(painted[2..].iter().all(|p| *p != first), "{painted:?}");
        let third = painted[5].clone();
        drop(painted);
        let round = |r: usize| stored(&art.round("source", r)).expect("readable");
        assert_eq!(round(1).as_deref(), Some(first.as_str()));
        assert_eq!(round(3).as_deref(), Some(third.as_str()));
        let results = remake(&art, &names, &author, &painter, &critic);
        assert!(landed(&results), "{results:?}");
        assert_eq!(paints.load(std::sync::atomic::Ordering::SeqCst), 6);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 6);
        assert_eq!(art.read().machine["source"].kept, Some(2));
        let mut m = art.read();
        m.machine.get_mut("source").expect("source").kept = Some(5);
        art.write(&m);
        let results = remake(&art, &names, &author, &painter, &critic);
        assert!(landed(&results), "{results:?}");
        assert_eq!(paints.load(std::sync::atomic::Ordering::SeqCst), 6);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 6);
        assert_eq!(art.read().machine["source"].kept, Some(5));
        assert_eq!(round(1).as_deref(), Some(first.as_str()));
        assert_eq!(round(3).as_deref(), Some(third.as_str()));
        let mut m = art.read();
        m.machine.get_mut("source").expect("source").kept = None;
        art.write(&m);
        let again = |_: &[PathBuf], brief: &str| Ok(format!("{brief} again"));
        let results = remake(&art, &names, &again, &painter, &critic);
        assert!(landed(&results), "{results:?}");
        assert_eq!(paints.load(std::sync::atomic::Ordering::SeqCst), 6);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 6);
        assert_eq!(art.read().machine["source"].kept, Some(2));
        assert_eq!(round(1).as_deref(), Some(first.as_str()));
        assert_eq!(round(3).as_deref(), Some(third.as_str()));
        assert_eq!(
            stored(&art.prompt("source")).expect("readable").as_deref(),
            Some(first.as_str())
        );
        let art = studio("ties", &["source"]);
        let critic = |_: &[PathBuf], _: &str| {
            Ok(r#"{"compound": false, "score": 6, "issues": []}"#.to_string())
        };
        assert!(landed(&remake(&art, &names, &author, &fake, &critic)));
        assert!(art.read().machine["source"].kept.expect("kept") <= 2);
        assert_eq!(rows(&art, "source").len(), 6);
    }

    #[test]
    fn an_unchanged_candidate_is_never_judged_again_and_a_changed_critic_prompt_is_judged_anew() {
        let art = studio("judged", &["source"]);
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let critic = |images: &[PathBuf], prompt: &str| {
            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            judge(images, prompt)
        };
        let names = ["source".to_string()];
        let run = || {
            calls.store(0, std::sync::atomic::Ordering::SeqCst);
            assert!(landed(&remake(&art, &names, &author, &fake, &critic)));
            calls.load(std::sync::atomic::Ordering::SeqCst)
        };
        assert_eq!(run(), 2);
        let judged = |i: u32| {
            rows(&art, "source")[&format!("source-{i}")]
                .1
                .judged
                .clone()
        };
        let first = (judged(1), judged(2));
        assert!(first.0.is_some() && first.1.is_some());
        assert_eq!(run(), 0);
        assert_eq!((judged(1), judged(2)), first);
        let mut m = art.read();
        m.style.critic += " Be harsher.";
        art.write(&m);
        assert_eq!(run(), 1);
        let kept = art.read().machine["source"].kept.expect("kept");
        assert_ne!(
            judged(kept),
            if kept == 1 {
                first.0.clone()
            } else {
                first.1.clone()
            }
        );
        assert_eq!(
            judged(3 - kept),
            if kept == 1 {
                first.1.clone()
            } else {
                first.0.clone()
            }
        );
        assert_eq!(run(), 0);
        let kept = art.read().machine["source"].kept.expect("kept");
        let mut m = art.read();
        m.machine.get_mut("source").expect("source").kept = Some(3 - kept);
        art.write(&m);
        assert_eq!(run(), 1);
        assert_eq!(run(), 0);
    }

    #[test]
    fn ask_sh_returns_the_message_and_retries_a_failed_call_with_backoff() {
        let root = std::env::temp_dir().join(format!("ziral-critic-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("bin")).expect("a fake bin");
        let codex = root.join("bin/codex");
        std::fs::write(
            &codex,
            "#!/bin/sh\n\
             n=$(cat \"$HOME/attempts\" 2>/dev/null || echo 0)\n\
             n=$((n + 1))\n\
             printf %s \"$n\" > \"$HOME/attempts\"\n\
             printf '%s\\n' \"$@\" > \"$HOME/args-$n\"\n\
             cp \"$6\" \"$HOME/schema-$n\"\n\
             [ \"$n\" -ge \"$PASS_ON\" ] || { echo \"codex: boom $n\" >&2; exit 1; }\n\
             echo '{\"type\":\"thread.started\",\"thread_id\":\"t1\"}'\n\
             echo '{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"{\\\"score\\\":6,\\\"issues\\\":[\\\"A far wall.\\\"]}\"}}'\n",
        )
        .expect("a fake codex");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&codex, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let path = format!(
            "{}:{}",
            root.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let image = root.join("board.png");
        std::fs::write(&image, b"png").expect("an image");
        let out = std::process::Command::new(Art::shipped().ask_sh())
            .arg("-i")
            .arg(&image)
            .arg(CRITIC_SCHEMA)
            .arg("Judge this.")
            .env("PATH", &path)
            .env("HOME", &root)
            .env("PASS_ON", "2")
            .output()
            .expect("ask.sh runs");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "{stderr}");
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            "{\"score\":6,\"issues\":[\"A far wall.\"]}\n"
        );
        assert!(stderr.contains("codex: boom 1"), "{stderr}");
        assert!(
            stderr.contains("attempt 1 of 4 failed, retrying in 1s"),
            "{stderr}"
        );
        let args = std::fs::read_to_string(root.join("args-2")).expect("the call's args");
        assert!(args.contains("--output-schema"), "{args}");
        assert!(
            args.contains(&format!("--image\n{}", image.display())),
            "{args}"
        );
        assert!(args.lines().any(|l| l == "Judge this."), "{args}");
        assert_eq!(
            std::fs::read_to_string(root.join("schema-2")).expect("the call's schema"),
            format!("{CRITIC_SCHEMA}\n")
        );
        let _ = std::fs::remove_file(root.join("attempts"));
        let out = std::process::Command::new(Art::shipped().ask_sh())
            .arg(CRITIC_SCHEMA)
            .arg("Judge this.")
            .env("PATH", &path)
            .env("HOME", &root)
            .env("PASS_ON", "99")
            .output()
            .expect("ask.sh runs");
        assert!(!out.status.success());
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("gave up after 4 attempts"),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    #[test]
    fn a_critic_that_reads_no_candidate_repaints_nothing_and_keeps_the_keep() {
        let art = studio("outage", &["source"]);
        let names = ["source".to_string()];
        assert!(landed(&remake(&art, &names, &author, &fake, &judge)));
        let kept = art.read().machine["source"].kept;
        let mut m = art.read();
        m.style.critic += " Be harsher.";
        art.write(&m);
        let paints = std::sync::atomic::AtomicUsize::new(0);
        let painter = counted(&paints, &fake);
        let down = |_: &[PathBuf], _: &str| Err("codex: boom".to_string());
        let results = remake(&art, &names, &author, &painter, &down);
        assert!(!landed(&results));
        let reason = results[0].1.clone().expect_err("source does not land");
        assert_eq!(reason, "the critic read no candidate; nothing repainted");
        assert_eq!(paints.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(art.read().machine["source"].kept, kept);
        assert!(art.candidate("source", 1).exists() && art.candidate("source", 2).exists());
        let rows = rows(&art, "source");
        assert_eq!(rows["source-1"].0, "pass");
        assert!(landed(&remake(&art, &names, &author, &painter, &judge)));
        assert_eq!(paints.load(std::sync::atomic::Ordering::SeqCst), 0);
    }
    #[test]
    fn direct_sh_prints_the_caption_the_director_writes_even_fenced_and_fails_on_no_caption() {
        let root = std::env::temp_dir().join(format!("ziral-direct-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("bin")).expect("a fake bin");
        let codex = root.join("bin/codex");
        std::fs::write(
            &codex,
            "#!/bin/sh\n\
             printf '%s\\n' \"$@\" > \"$HOME/args\"\n\
             echo '{\"type\":\"thread.started\",\"thread_id\":\"t1\"}'\n\
             printf '{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":%s}}\\n' \"$REPLY\"\n",
        )
        .expect("a fake codex");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&codex, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let path = format!(
            "{}:{}",
            root.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let image = root.join("scaffold.png");
        std::fs::write(&image, b"png").expect("an image");
        let direct = |reply: &str| {
            std::process::Command::new(Art::shipped().direct_sh())
                .arg("-i")
                .arg(&image)
                .arg("A source.")
                .env("PATH", &path)
                .env("HOME", &root)
                .env("REPLY", reply)
                .output()
                .expect("direct.sh runs")
        };
        let out = direct(r#""```json\n{\"prompt\": \"A flat plan.\"}\n```""#);
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&out.stdout), "A flat plan.\n");
        let args = std::fs::read_to_string(root.join("args")).expect("the call's args");
        assert!(args.contains("--output-schema"), "{args}");
        assert!(
            args.contains(&format!("--image\n{}", image.display())),
            "{args}"
        );
        assert!(
            args.lines()
                .any(|l| l.starts_with("You are the art director")
                    && l.ends_with("Brief: A source.")),
            "{args}"
        );
        for reply in [r#""{\"caption\": \"A flat plan.\"}""#, r#""A flat plan.""#] {
            let out = direct(reply);
            assert!(!out.status.success(), "{reply}");
            assert_eq!(String::from_utf8_lossy(&out.stdout), "", "{reply}");
        }
    }

    #[test]
    fn every_painted_texture_has_the_prompt_that_painted_it_beside_it() {
        let art = Path::new(env!("CARGO_MANIFEST_DIR")).join("art");
        let mut painted = vec![
            ("overlay/page", "overlay/page"),
            (look::GROUT.name, look::GROUT.name),
        ];
        painted.extend(
            crate::sim::AtomKind::ALL
                .map(|k| look::atom(k).skin.name)
                .map(|n| (n, n)),
        );
        painted.extend(
            crate::sim::BondKind::ALL
                .map(|k| look::bond(k).skin.name)
                .map(|n| (n, n)),
        );
        painted.extend(look::TILES.iter().map(|t| {
            let (family, index) = t.name.rsplit_once('-').expect("tile-NN");
            assert!(index.parse::<u32>().is_ok(), "{}", t.name);
            (t.name, family)
        }));
        for (png, name) in painted {
            let prompt = art.join(format!("{name}.prompt.txt"));
            assert!(art.join(format!("{png}.png")).exists(), "{png}.png");
            assert!(
                stored(&prompt).expect("readable").is_some(),
                "{}: no prompt beside the texture",
                prompt.display()
            );
        }
    }
}
