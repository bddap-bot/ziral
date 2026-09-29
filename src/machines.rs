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
const RUBBER: [f32; 3] = [
    0x42 as f32 / 255.0,
    0x3B as f32 / 255.0,
    0x37 as f32 / 255.0,
];

#[derive(Serialize, Deserialize)]
struct Manifest {
    attempts: u32,
    style: Style,
    thresholds: Thresholds,
    machine: BTreeMap<String, Entry>,
    #[serde(default)]
    texture: BTreeMap<String, TextureEntry>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Style {
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
    off_centre: f32,
    sphere: f32,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct Entry {
    kept: Option<u32>,
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

    fn ming_sh(&self) -> PathBuf {
        self.dir.join("../ming.sh")
    }

    fn caption(&self, name: &str) -> PathBuf {
        self.machine(name).join("caption.txt")
    }

    fn attempt(&self, name: &str, index: u32) -> PathBuf {
        self.machine(name).join(format!("attempts/{index}"))
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

    fn mount(&self, image: &RgbaImage, pad: Rgba<u8>) -> RgbaImage {
        let (origin, side) = self.crop();
        assert_eq!((image.width(), image.height()), (side, side));
        RgbaImage::from_fn(self.canvas, self.canvas, |x, y| {
            if (origin..origin + side).contains(&x) && (origin..origin + side).contains(&y) {
                *image.get_pixel(x - origin, y - origin)
            } else {
                pad
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

    fn alpha(&self, candidate: &RgbaImage) -> Vec<f32> {
        candidate.pixels().map(alpha).collect()
    }

    fn register(&self, candidate: &RgbaImage) -> Capture {
        let fitted = self.fit(candidate);
        let cells: Vec<Vec2> = self.marked().map(|c| px(c.at)).collect();
        if cells.is_empty() {
            return Capture {
                image: fitted,
                off_centre: 0.0,
            };
        }
        let seats: Option<Vec<Vec2>> = self.marked().map(|c| self.seat(&fitted, c)).collect();
        let Some(seats) = seats else {
            return Capture {
                image: fitted,
                off_centre: f32::INFINITY,
            };
        };
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
            bilinear(&fitted, self.pixel(onto(world)))
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

    fn fit(&self, candidate: &RgbaImage) -> RgbaImage {
        let (width, height) = candidate.dimensions();
        let step = (width.max(height) / FIT_GRID).max(1);
        let solid = |x: u32, y: u32| x < width && y < height && candidate.get_pixel(x, y)[3] >= 128;
        let mut lo = Vec2::splat(f32::INFINITY);
        let mut hi = Vec2::splat(f32::NEG_INFINITY);
        let mut edge = Vec::new();
        for y in (0..height).step_by(step as usize) {
            for x in (0..width).step_by(step as usize) {
                if !solid(x, y) {
                    continue;
                }
                let p = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                lo = lo.min(p);
                hi = hi.max(p);
                let inner = x >= step
                    && y >= step
                    && solid(x - step, y)
                    && solid(x + step, y)
                    && solid(x, y - step)
                    && solid(x, y + step);
                if !inner {
                    edge.push(p);
                }
            }
        }
        if edge.is_empty() {
            return RgbaImage::new(self.canvas, self.canvas);
        }
        let centre = (lo + hi) / 2.0;
        let world =
            |p: Vec2, k: f32| self.quad.centre + Vec2::new(p.x - centre.x, centre.y - p.y) / k;
        let over = |k: f32| {
            edge.iter()
                .map(|p| self.outside(world(*p, k)))
                .fold(0.0, f32::max)
        };
        let guess = (hi - lo).max_element() / self.quad.side;
        let (mut small, mut large) = (guess / 100.0, guess * 100.0);
        for _ in 0..40 {
            let mid = (small * large).sqrt();
            if over(mid) > 0.0 {
                small = mid;
            } else {
                large = mid;
            }
        }
        RgbaImage::from_fn(self.canvas, self.canvas, |x, y| {
            let w = self.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5)) - self.quad.centre;
            let p = centre + Vec2::new(w.x, -w.y) * large;
            if p.x < 0.0 || p.y < 0.0 || p.x >= width as f32 || p.y >= height as f32 {
                Rgba([0; 4])
            } else {
                bilinear(candidate, p)
            }
        })
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
                let p = over_black(candidate.get_pixel(x, y));
                let world = self.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
                let r = (world - px(cell.at)).length() / HEX;
                if r <= SEAT_DOT {
                    centre.add_rgb(p);
                } else if self.mark(cell, world).is_some() {
                    mark.add_rgb(p);
                } else if (SEAT_AROUND[0]..=SEAT_AROUND[1]).contains(&r) {
                    around.add_rgb(p);
                }
            }
        }
        apart(mark.rgb(), around.rgb()).max(apart(centre.rgb(), around.rgb()))
    }

    fn seat(&self, candidate: &RgbaImage, cell: &Cell) -> Option<Vec2> {
        let sample = |world: Vec2| {
            let p = self.pixel(world);
            let inside = p.min_element() >= 0.0 && p.max_element() < self.canvas as f32;
            inside.then(|| Vec3::from_array(over_black(candidate.get_pixel(p.x as u32, p.y as u32))))
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
            grey(alpha(p) * (0.85 + 0.15 * luminance(p)))
        });
        let height = image::imageops::blur(&height, 4.0);
        let sample = |x: u32, y: u32| luminance(height.get_pixel(x, y));
        RgbaImage::from_fn(self.canvas, self.canvas, |x, y| {
            let dx = sample((x + 1).min(self.canvas - 1), y) - sample(x.saturating_sub(1), y);
            let dy = sample(x, (y + 1).min(self.canvas - 1)) - sample(x, y.saturating_sub(1));
            encode(
                Vec3::new(-24.0 * dx, 24.0 * dy, 1.0).normalize(),
                alpha(candidate.get_pixel(x, y)),
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
            rgba(rgb(p).map(|c| c * strength), alpha)
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
}

impl Score {
    fn measured(&self, t: &Thresholds) -> Option<String> {
        if self.outside > t.outside {
            Some(format!("outside {:.3} > {}", self.outside, t.outside))
        } else if self.seat < t.seat {
            Some(format!("seat {:.3} < {}", self.seat, t.seat))
        } else if self.off_centre > t.off_centre {
            Some(format!(
                "off_centre {:.3} > {}",
                self.off_centre, t.off_centre
            ))
        } else {
            None
        }
    }

    fn passes(&self, t: &Thresholds) -> bool {
        self.measured(t).is_none()
    }

    fn excess(&self, t: &Thresholds) -> f32 {
        (self.outside / t.outside)
            .max(t.seat / self.seat)
            .max(self.off_centre / t.off_centre)
    }
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

fn alpha(p: &Rgba<u8>) -> f32 {
    f32::from(p[3]) / 255.0
}

fn over_black(p: &Rgba<u8>) -> [f32; 3] {
    rgb(p).map(|c| c * alpha(p))
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

const PAINTERS: usize = 5;
const FIT_GRID: u32 = 512;
const COVER: [f32; 2] = [0.03, 0.9];
const FRAME_EDGE: f32 = 0.01;
const EDGE_ALPHA: f32 = 0.02;
const PLAN: &str = "Decompose this image into 3 layers with the following specifications:\n\nNumber of layers: 3\nLayer 1: The complete object, every part and fitting of it, with any hole cut through it left empty.\nLayer 2: The soft shadow beneath the object, if any.\nLayer 3: The plain background.\n";

#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Calls {
    design: f32,
    layer: f32,
    cost: f64,
}

impl Calls {
    fn read(out: &Path) -> Option<Calls> {
        if !out.join("design.png").exists() || !out.join("layer-1.png").exists() {
            return None;
        }
        let text = std::fs::read_to_string(out.join("calls.tsv")).ok()?;
        let mut calls = Calls::default();
        let mut seen = [false; 2];
        for line in text.lines() {
            let cols: Vec<&str> = line.split('\t').collect();
            let [model, seconds, cost, ..] = cols[..] else {
                return None;
            };
            let (seconds, cost): (f32, f64) = (seconds.parse().ok()?, cost.parse().ok()?);
            match model {
                "design" => (calls.design, seen[0]) = (seconds, true),
                "design-layer" => (calls.layer, seen[1]) = (seconds, true),
                _ => return None,
            }
            calls.cost += cost;
        }
        (seen == [true, true]).then_some(calls)
    }
}

enum Refusal {
    Spent(String),
    Failed(String),
}

type Painter<'a> = &'a (dyn Fn(&Path, &Path) -> Result<Calls, Refusal> + Sync);

fn ming(art: &Art, caption: &Path, out: &Path) -> Result<Calls, Refusal> {
    let failed = |e: std::io::Error| Refusal::Failed(format!("{}: {e}", out.display()));
    std::fs::create_dir_all(out).map_err(failed)?;
    let plan = out.join("plan.txt");
    std::fs::write(&plan, PLAN).map_err(failed)?;
    let design = out.join("design.png");
    let mut log = ming_sh(art, &["design".as_ref(), design.as_ref(), caption.as_ref()])?;
    log += &ming_sh(
        art,
        &["design-layer".as_ref(), out.as_ref(), design.as_ref(), plan.as_ref()],
    )?;
    std::fs::write(out.join("calls.tsv"), log).map_err(failed)?;
    Calls::read(out).ok_or_else(|| Refusal::Failed(format!("{}: incomplete", out.display())))
}

fn ming_sh(art: &Art, args: &[&std::ffi::OsStr]) -> Result<String, Refusal> {
    let script = art.ming_sh();
    let output = std::process::Command::new(&script)
        .args(args)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| Refusal::Failed(format!("{}: {e}", script.display())))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stderr.lines().filter(|l| !l.trim().is_empty()) {
        eprintln!("{line}");
    }
    let last = stderr
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("ming.sh failed without a word")
        .to_string();
    match output.status.code() {
        Some(0) => Ok(String::from_utf8_lossy(&output.stdout).into_owned()),
        Some(3) => Err(Refusal::Spent(last)),
        _ => Err(Refusal::Failed(last)),
    }
}

fn matte(out: &Path) -> Result<RgbaImage, String> {
    let design = open(out.join("design.png"));
    let (w, h) = design.dimensions();
    if w != h {
        return Err(format!("the design is {w}x{h}, not square"));
    }
    if !out.join("layer-2.png").exists() {
        return Err("the split returned one layer".to_string());
    }
    let object = open(out.join("layer-1.png"));
    let (ow, oh) = object.dimensions();
    let cover = object.pixels().map(alpha).sum::<f32>() / (ow * oh) as f32;
    if !(COVER[0]..=COVER[1]).contains(&cover) {
        return Err(format!("the object layer covers {cover:.3} of the frame"));
    }
    let band = ((ow.min(oh) as f32 * FRAME_EDGE).ceil() as u32).max(1);
    let mut edge = Mean::default();
    for (x, y, p) in object.enumerate_pixels() {
        if x < band || y < band || x >= ow - band || y >= oh - band {
            edge.add(alpha(p));
        }
    }
    if edge.value() > EDGE_ALPHA {
        return Err(format!(
            "the object layer reaches the frame edge at {:.3} alpha",
            edge.value()
        ));
    }
    let up = image::imageops::resize(&object, w, h, image::imageops::FilterType::CatmullRom);
    Ok(RgbaImage::from_fn(w, h, |x, y| {
        let (o, d) = (up.get_pixel(x, y), design.get_pixel(x, y));
        let a = alpha(o);
        let t = ((a - 0.9) / 0.1).clamp(0.0, 1.0);
        let (layer, design) = (rgb(o), rgb(d));
        rgba([0, 1, 2].map(|i| design[i] * t + layer[i] * (1.0 - t)), a)
    }))
}

#[derive(Debug, Clone, PartialEq)]
struct Attempt {
    index: u32,
    calls: Calls,
    outcome: Result<Score, String>,
}

impl Attempt {
    fn passes(&self, t: &Thresholds) -> bool {
        self.outcome.as_ref().is_ok_and(|s| s.passes(t))
    }

    fn verdict(&self, t: &Thresholds) -> String {
        match &self.outcome {
            Ok(score) => score
                .measured(t)
                .map_or("pass".to_string(), |rule| format!("fail {rule}")),
            Err(why) => why.replace(['\t', '\n'], " "),
        }
    }

    fn row(&self, t: &Thresholds) -> String {
        let measured = self.outcome.as_ref().map_or("-\t-\t-\t-".to_string(), |s| {
            format!(
                "{:.3}\t{:.3}\t{:.3}\t{:.3}",
                s.outside, s.seat, s.palette, s.off_centre
            )
        });
        format!(
            "{}\t{}\t{measured}\t{:.1}\t{:.1}\t{}",
            self.index,
            self.verdict(t),
            self.calls.design,
            self.calls.layer,
            self.calls.cost
        )
    }

    fn parse(row: &str) -> Option<Attempt> {
        let cols: Vec<&str> = row.split('\t').collect();
        let [index, verdict, outside, seat, palette, off_centre, design, layer, cost] = cols[..]
        else {
            return None;
        };
        let outcome = if outside == "-" {
            Err(verdict.to_string())
        } else {
            Ok(Score {
                outside: outside.parse().ok()?,
                seat: seat.parse().ok()?,
                palette: palette.parse().ok()?,
                off_centre: off_centre.parse().ok()?,
            })
        };
        Some(Attempt {
            index: index.parse().ok()?,
            calls: Calls {
                design: design.parse().ok()?,
                layer: layer.parse().ok()?,
                cost: cost.parse().ok()?,
            },
            outcome,
        })
    }
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

fn painted_key(caption: &str, attempts: u32) -> String {
    key(&[caption.as_bytes(), &attempts.to_le_bytes(), PLAN.as_bytes()])
}

fn relit_key(albedo: &[u8], style: &Style, facings: &[Facing]) -> String {
    let facings: Vec<&str> = facings.iter().map(|facing| facing.name()).collect();
    key(&[
        albedo,
        b"computed-relief-3:blur=4,height=alpha*(0.85+0.15*luma),slope=24,lambert,rgba",
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

fn attempts(dir: &Path) -> BTreeMap<u32, Attempt> {
    std::fs::read_to_string(dir.join("attempts.tsv"))
        .unwrap_or_default()
        .lines()
        .filter_map(Attempt::parse)
        .map(|a| (a.index, a))
        .collect()
}

struct Remake<'a> {
    art: &'a Art,
    manifest: std::sync::Mutex<Manifest>,
    painter: Painter<'a>,
    cap: Cap,
    spent: std::sync::Mutex<Option<String>>,
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
    fn entry(&self, name: &str) -> Result<(Style, Thresholds, u32, Entry), String> {
        let m = self.manifest.lock().expect("the manifest is unpoisoned");
        let entry = m
            .machine
            .get(name)
            .ok_or_else(|| format!("manifest has no [machine.{name}]"))?;
        Ok((m.style.clone(), m.thresholds, m.attempts, entry.clone()))
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
        source: &[u8],
        style: &Style,
    ) -> Result<String, String> {
        assert_eq!(style.facings, Facing::ALL);
        let relit = dir.join("relit");
        let expected = relit_key(source, style, &style.facings);
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

    fn attempt(
        &self,
        name: &str,
        index: u32,
        scaffold: &Scaffold,
        thresholds: &Thresholds,
    ) -> Result<Attempt, String> {
        let out = self.art.attempt(name, index);
        let calls = match Calls::read(&out) {
            Some(calls) => calls,
            None => {
                let result = {
                    let _permit = self.cap.take();
                    if let Some(why) = self.spent.lock().expect("unpoisoned").clone() {
                        return Err(format!("stopped: {why}"));
                    }
                    let _ = std::fs::remove_dir_all(&out);
                    (self.painter)(&self.art.caption(name), &out)
                };
                match result {
                    Ok(calls) => calls,
                    Err(Refusal::Spent(why)) => {
                        *self.spent.lock().expect("unpoisoned") = Some(why.clone());
                        return Err(format!("stopped: {why}"));
                    }
                    Err(Refusal::Failed(why)) => {
                        return Ok(Attempt {
                            index,
                            calls: Calls::default(),
                            outcome: Err(format!("paint: {why}")),
                        });
                    }
                }
            }
        };
        let outcome = matte(&out)
            .map(|sprite| scaffold.score(&scaffold.register(&sprite)))
            .map_err(|why| format!("matte: {why}"));
        let attempt = Attempt {
            index,
            calls,
            outcome,
        };
        println!("{name}\t{}", attempt.row(thresholds));
        Ok(attempt)
    }

    fn paint(&self, name: &str, scaffold: &Scaffold) -> Result<(), String> {
        let (_, thresholds, count, entry) = self.entry(name)?;
        let dir = self.art.machine(name);
        let caption = self.art.caption(name);
        let text = std::fs::read_to_string(&caption)
            .map_err(|e| format!("{}: {e}", caption.display()))?;
        let painted = painted_key(&text, count);
        let mut rows = if entry.painted.as_deref() == Some(painted.as_str()) {
            attempts(&dir)
        } else {
            let _ = std::fs::remove_dir_all(dir.join("attempts"));
            let _ = std::fs::remove_file(dir.join("attempts.tsv"));
            self.record(name, |e| {
                e.painted = Some(painted.clone());
                e.kept = None;
                e.relit = None;
            });
            BTreeMap::new()
        };
        rows.retain(|i, _| *i <= count && self.art.attempt(name, *i).join("design.png").exists());
        for index in 1..=count {
            if rows.values().any(|a| a.passes(&thresholds)) {
                break;
            }
            if rows.contains_key(&index) {
                continue;
            }
            let attempt = self.attempt(name, index, scaffold, &thresholds)?;
            rows.insert(index, attempt);
            let table: Vec<String> = rows.values().map(|a| a.row(&thresholds)).collect();
            std::fs::write(dir.join("attempts.tsv"), table.join("\n") + "\n")
                .map_err(|e| e.to_string())?;
        }
        let (kept, _) = rows
            .values()
            .filter_map(|a| Some((a.index, a.outcome.as_ref().ok()?.excess(&thresholds))))
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
            .ok_or_else(|| {
                let verdicts: Vec<String> =
                    rows.values().map(|a| a.verdict(&thresholds)).collect();
                format!("no attempt made a sprite: {}", verdicts.join("; "))
            })?;
        let capture = scaffold.register(&matte(&self.art.attempt(name, kept))?);
        let albedo = dir.join("albedo.png");
        save(&scaffold.cropped(&capture.image), &albedo);
        quantise(&albedo)?;
        self.record(name, |e| e.kept = Some(kept));
        self.redraw.store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    fn machine(&self, name: &str) -> Result<bool, String> {
        let started = std::time::Instant::now();
        let scaffold = Scaffold::of(item(name));
        let dir = self.art.machine(name);
        let (_, _, count, entry) = self.entry(name)?;
        let caption = self.art.caption(name);
        let text = std::fs::read_to_string(&caption)
            .map_err(|e| format!("{}: {e}", caption.display()))?;
        let mut changed = false;
        if entry.painted.as_deref() != Some(painted_key(&text, count).as_str())
            || entry.kept.is_none()
            || !dir.join("albedo.png").exists()
        {
            self.paint(name, &scaffold)?;
            changed = true;
        }
        let (style, thresholds, _, entry) = self.entry(name)?;
        let albedo = open(dir.join("albedo.png"));
        let candidate = scaffold.mount(&albedo, Rgba([0; 4]));
        let relit = self.prepare_relief(&dir, &scaffold, &candidate, albedo.as_raw(), &style)?;
        let relit_dir = dir.join("relit");
        let current = entry.relit.as_deref() == Some(relit.as_str())
            && dir.join("normal.png").exists()
            && relit_dir.join("albedo.png").exists()
            && relit_dir.join("normal.png").exists();
        if !current {
            self.relief(
                name,
                &Relight {
                    dir: &dir,
                    scaffold: &scaffold,
                    candidate: &candidate,
                    style: &style,
                    facings: &style.facings,
                    threshold: thresholds.sphere,
                    source: RelightSource::Machine,
                },
            )?;
            self.record(name, |e| e.relit = Some(relit.clone()));
            changed = true;
        }
        println!(
            "{name}\t{}\tkept attempt {}\t{:.0}s",
            if changed { "landed" } else { "up to date" },
            entry.kept.map_or("-".to_string(), |k| k.to_string()),
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
                &request.scaffold.cropped(normal),
                &request.dir.join("normal.png"),
            );
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
        let candidate = scaffold.mount(&image, grey(PAD_ALBEDO));
        let dir = self.art.texture(name);
        let key = self.prepare_relief(&dir, &scaffold, &candidate, image.as_raw(), &style)?;
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
            let dir = self.art.machine(name);
            let (Some(kept), true) = (
                m.machine.get(name).and_then(|e| e.kept),
                dir.join("albedo.png").exists(),
            ) else {
                continue;
            };
            let verdict = attempts(&dir)
                .get(&kept)
                .map_or("-".to_string(), |a| a.verdict(&m.thresholds));
            args.push("-label".into());
            args.push(format!("{name}  attempt {kept}/{}  {verdict}", m.attempts).into());
            args.push(dir.join("albedo.png").into());
        }
        if args.len() == 1 {
            return Ok(());
        }
        let part = self.art.dir.join("sheet.png.part");
        args.extend(
            [
                "-tile",
                "5x",
                "-geometry",
                "400x400+8+8",
                "-background",
                "#D8C3A5",
                "-fill",
                "#423B37",
                "-font",
                "DejaVu-Sans",
                "-pointsize",
                "13",
            ]
            .map(Into::into),
        );
        args.push(format!("png:{}", part.display()).into());
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

fn remake(art: &Art, names: &[String], painter: Painter) -> Vec<(String, Result<bool, String>)> {
    let remake = Remake {
        art,
        manifest: std::sync::Mutex::new(art.read()),
        painter,
        cap: Cap::new(PAINTERS),
        spent: std::sync::Mutex::new(None),
        redraw: std::sync::atomic::AtomicBool::new(false),
    };
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
                        remake.machine(name)
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
    let painter = |caption: &Path, out: &Path| ming(&art, caption, out);
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
    let generated = landed(&remake(&art, &names, &painter));
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
    fn machines_preserve_painted_alpha_in_footprint_measurement_and_cutting() {
        let scaffold = Scaffold::of(Machine::Glyph(crate::sim::GlyphKind::SourceTwo));
        let mut candidate = fired(&scaffold, &|w| w);
        let (origin, _) = scaffold.crop();
        candidate.put_pixel(origin, origin, Rgba([255, 0, 0, 0]));
        assert_eq!(scaffold.cropped(&candidate).get_pixel(0, 0)[3], 0);
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
        let art = studio("static-split", &["portal", "bonder"], 1);
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
                matches!(
                    machine,
                    Machine::Portal | Machine::Glyph(crate::sim::GlyphKind::Resonator)
                ),
                "{name}"
            );
        }
    }
    const REGISTERED: f32 = 0.02;

    fn fired(scaffold: &Scaffold, at: &dyn Fn(Vec2) -> Vec2) -> RgbaImage {
        RgbaImage::from_fn(scaffold.canvas, scaffold.canvas, |x, y| {
            let world = at(scaffold.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5)));
            let glaze = scaffold
                .cells
                .iter()
                .find_map(|cell| scaffold.mark(cell, world));
            match glaze {
                Some(glaze) => rgba(glaze.rgb(), 1.0),
                None if scaffold.covered(world) => rgba(Glaze::Clay.rgb(), 1.0),
                None => Rgba([0; 4]),
            }
        })
    }

    fn shade(n: Vec3) -> f32 {
        (AMBIENT + (1.0 - AMBIENT) * n.dot(light()).max(0.0))
            / (AMBIENT + (1.0 - AMBIENT) * light().z)
    }

    #[test]
    fn machines_palette_measurement_does_not_reject_or_rank_art() {
        let thresholds = Art::shipped().read().thresholds;
        let scaffold = Scaffold::of(item("bonder"));
        let mut score = scaffold.score(&scaffold.register(&fired(&scaffold, &|w| w)));
        assert!(score.measured(&thresholds).is_none());
        let excess = score.excess(&thresholds);
        assert!(excess <= 1.0, "{excess}");
        score.palette = 100.0;
        assert!(score.measured(&thresholds).is_none());
        assert_eq!(score.excess(&thresholds), excess);
        score.outside = thresholds.outside + 1.0;
        assert!(score.measured(&thresholds).unwrap().starts_with("outside"));
    }

    #[test]
    fn machines_body_only_registration_uniformly_fits_the_silhouette() {
        let scaffold = Scaffold::of(Machine::Portal);
        let candidate = RgbaImage::from_fn(scaffold.canvas, scaffold.canvas, |x, y| {
            let world = scaffold.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
            if look::hex_norm(world.x / HEX, world.y / HEX) < 0.9 {
                rgba(Glaze::Plum.rgb(), 1.0)
            } else {
                Rgba([0; 4])
            }
        });
        let capture = scaffold.register(&candidate);

        assert_eq!(capture.off_centre, 0.0);
        let score = scaffold.score(&capture);
        assert!(score.palette.is_finite(), "body palette is measurable");
        assert!(score.outside <= 0.05);
    }

    #[test]
    fn machines_output_surfaces_keep_their_full_footprints_without_seats() {
        use crate::sim::{GlyphKind, Tier};

        let thresholds = Art::shipped().read().thresholds;
        for tier in [Tier::One, Tier::Two, Tier::Three] {
            let kind = GlyphKind::Output(tier);
            let scaffold = Scaffold::of(Machine::Glyph(kind));
            assert_eq!(scaffold.marked().count(), 0);
            assert_eq!(
                scaffold.cells.iter().map(|c| c.at).collect::<Vec<_>>(),
                kind.cells()
            );
            let clean = fired(&scaffold, &|w| w);
            let painted = fired(&scaffold, &|w| (w - Vec2::new(0.1, -0.1) * HEX) / 0.8);
            let capture = scaffold.register(&painted);
            let score = scaffold.score(&capture);
            assert!(score.measured(&thresholds).is_none(), "{tier:?}: {score:?}");
            let area = |image: &RgbaImage| image.pixels().map(alpha).sum::<f32>();
            assert!((area(&capture.image) / area(&clean) - 1.0).abs() < REGISTERED);
        }
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
        let slipped = RgbaImage::from_fn(scaffold.canvas, scaffold.canvas, |x, y| {
            let world = scaffold.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
            match scaffold
                .cells
                .iter()
                .find_map(|cell| scaffold.mark(cell, world - beyond))
            {
                Some(glaze) => rgba(glaze.rgb(), 1.0),
                None if scaffold.covered(world) => rgba(Glaze::Clay.rgb(), 1.0),
                None => Rgba([0; 4]),
            }
        });
        let lost = scaffold.score(&scaffold.register(&slipped));
        assert!(
            lost.off_centre > thresholds.off_centre,
            "{lost:?}: seats beyond the search reach of the silhouette"
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
    fn bounded_measurement_matches_the_exhaustive_pixel_and_hex_walks_bit_for_bit() {
        for name in ["arm", "bonder", "source", "reification", "output-3"] {
            let scaffold = Scaffold::of(item(name));
            let candidate = RgbaImage::from_fn(scaffold.canvas, scaffold.canvas, |x, y| {
                Rgba([(x % 251) as u8, (y % 241) as u8, ((x + y) % 239) as u8, 255])
            });
            for cell in scaffold.marked() {
                let bounded = scaffold.seat_contrast(&candidate, cell, scaffold.seat_bounds(cell));
                let exhaustive = scaffold.seat_contrast(
                    &candidate,
                    cell,
                    [0, 0, scaffold.canvas, scaffold.canvas],
                );
                assert_eq!(
                    bounded.to_bits(),
                    exhaustive.to_bits(),
                    "{name}/{:?}",
                    cell.at
                );
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

    const SHIPPED: f32 = 0.02;

    #[test]
    fn every_machine_ships_its_caption_its_best_attempt_and_the_relief_of_its_albedo() {
        let art = Art::shipped();
        let manifest = art.read();
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
        let t = &manifest.thresholds;
        for item in Machine::ALL {
            let name = name(item);
            let entry = &manifest.machine[name];
            let dir = art.machine(name);
            let caption = std::fs::read_to_string(art.caption(name))
                .unwrap_or_else(|e| panic!("{name}/caption.txt: {e}"));
            assert_eq!(
                entry.painted.as_deref(),
                Some(painted_key(&caption, manifest.attempts).as_str()),
                "{name}: the art is stale against its caption: run ziral --gen {name}"
            );
            let kept = entry
                .kept
                .unwrap_or_else(|| panic!("{name} keeps no attempt"));
            let rows = attempts(&dir);
            assert!(
                rows.keys().all(|i| (1..=manifest.attempts).contains(i)),
                "{name}"
            );
            let recorded = rows
                .get(&kept)
                .and_then(|a| a.outcome.clone().ok())
                .unwrap_or_else(|| panic!("{name}: attempts.tsv has no sprite for attempt {kept}"));
            for row in rows.values() {
                if let Ok(score) = &row.outcome {
                    assert!(
                        score.excess(t) >= recorded.excess(t),
                        "{name}: attempt {} measures better than the kept {kept}",
                        row.index
                    );
                }
            }
            let scaffold = Scaffold::of(item);
            let (_, side) = scaffold.crop();
            let albedo = open(dir.join("albedo.png"));
            assert_eq!(albedo.dimensions(), (side, side), "{name}/albedo.png");
            let mounted = scaffold.mount(&albedo, Rgba([0; 4]));
            let score = scaffold.score(&Capture {
                image: mounted.clone(),
                off_centre: recorded.off_centre,
            });
            assert!(
                (score.outside - recorded.outside).abs() <= SHIPPED,
                "{name}: albedo.png measures {score:?}, attempts.tsv records {recorded:?}"
            );
            for map in ["albedo", "normal"] {
                let png = open(dir.join(format!("{map}.png")));
                let placed = scaffold.quad.size();
                let aspect = png.width() as f32 / png.height() as f32 / (placed.x / placed.y);
                assert!((aspect - 1.0).abs() <= ASPECT, "{name}/{map}.png");
                assert_eq!(png.dimensions(), (side, side), "{name}/{map}.png");
            }
            let shipped_normal = open(dir.join("normal.png"));
            let tilted = shipped_normal
                .pixels()
                .filter(|p| decode(p).truncate().length() > RELIEF)
                .count();
            assert!(
                tilted as f32 > shipped_normal.pixels().len() as f32 * 0.01,
                "{name}/normal.png is flat"
            );
            let facings = &manifest.style.facings;
            assert_eq!(
                entry.relit.as_deref(),
                Some(relit_key(albedo.as_raw(), &manifest.style, facings).as_str()),
                "{name}: the maps are stale against albedo.png: run ziral --gen {name}"
            );
            let atlas = open(dir.join("relit/albedo.png"));
            assert_eq!(atlas.width(), side, "{name}");
            assert_eq!(atlas.height(), side * facings.len() as u32, "{name}");
            let normal = scaffold.surface_normals(&mounted);
            assert!(
                shipped_normal.as_raw() == scaffold.cropped(&normal).as_raw(),
                "{name}: shipped normals differ from the computed relief"
            );
            let edits: Vec<_> = facings
                .iter()
                .map(|facing| {
                    let rendered = open(dir.join(format!("relit/{}.png", facing.name())));
                    let expected = scaffold.render_light(
                        &mounted,
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

    fn studio(tag: &str, machines: &[&str], attempts: u32) -> Art {
        let shipped = Art::shipped().read();
        let root = std::env::temp_dir().join(format!("ziral-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let art = Art {
            dir: root.join("machines"),
        };
        for name in machines {
            std::fs::create_dir_all(art.machine(name)).expect("a studio is creatable");
            std::fs::write(art.caption(name), format!("a {name}\n")).expect("a caption");
        }
        art.write(&Manifest {
            attempts,
            style: shipped.style,
            thresholds: shipped.thresholds,
            machine: machines
                .iter()
                .map(|name| {
                    (
                        name.to_string(),
                        Entry {
                            kept: None,
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

    const GROUND: Rgba<u8> = Rgba([180, 180, 178, 255]);

    fn drawn(out: &Path, sprite: &RgbaImage, layer: Option<RgbaImage>) -> Calls {
        std::fs::create_dir_all(out).expect("an attempt dir");
        let side = sprite.width() * 5 / 4;
        let offset = i64::from((side - sprite.width()) / 2);
        let mut design = RgbaImage::from_pixel(side, side, GROUND);
        image::imageops::overlay(&mut design, sprite, offset, offset);
        let mut object = RgbaImage::new(side, side);
        image::imageops::replace(&mut object, sprite, offset, offset);
        let half = |image: &RgbaImage| {
            image::imageops::resize(
                image,
                side / 2,
                side / 2,
                image::imageops::FilterType::Triangle,
            )
        };
        save(&design, &out.join("design.png"));
        save(
            &layer.unwrap_or_else(|| half(&object)),
            &out.join("layer-1.png"),
        );
        save(
            &half(&RgbaImage::from_pixel(side, side, GROUND)),
            &out.join("layer-2.png"),
        );
        std::fs::write(
            out.join("calls.tsv"),
            "design\t1.5\t0\t0\t0\ndesign-layer\t2.5\t0\t0\t0\n",
        )
        .expect("calls.tsv");
        Calls::read(out).expect("a complete attempt")
    }

    fn turned(scaffold: &Scaffold, angle: f32) -> RgbaImage {
        let centre = scaffold.quad.centre;
        let turn = Vec2::from_angle(angle);
        fired(scaffold, &|w| centre + turn.rotate(w - centre))
    }

    #[test]
    fn attempts_run_until_one_passes_and_an_unchanged_caption_paints_nothing() {
        let art = studio("attempts", &["bonder"], 4);
        let scaffold = Scaffold::of(item("bonder"));
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let painter = |caption: &Path, out: &Path| {
            assert_eq!(caption, art.caption("bonder"));
            let n = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
            let clean = fired(&scaffold, &|w| w);
            Ok(match n {
                1 => {
                    let side = clean.width() * 5 / 8;
                    drawn(out, &clean, Some(RgbaImage::from_pixel(side, side, GROUND)))
                }
                2 => drawn(out, &turned(&scaffold, 0.3), None),
                _ => drawn(out, &clean, None),
            })
        };
        let count = || calls.load(std::sync::atomic::Ordering::SeqCst);
        let run = || remake(&art, &["bonder".to_string()], &painter);
        assert!(landed(&run()));
        assert_eq!(count(), 3);
        let thresholds = art.read().thresholds;
        let rows = attempts(&art.machine("bonder"));
        let verdicts: Vec<String> = rows.values().map(|a| a.verdict(&thresholds)).collect();
        assert!(verdicts[0].starts_with("matte: "), "{verdicts:?}");
        assert!(verdicts[1].starts_with("fail off_centre"), "{verdicts:?}");
        assert_eq!(verdicts[2], "pass");
        assert_eq!(rows[&3].calls.design, 1.5);
        assert_eq!(art.read().machine["bonder"].kept, Some(3));
        assert!(art.machine("bonder").join("relit/albedo.png").exists());
        assert!(art.dir.join("sheet.png").exists());
        let again = run();
        assert!(landed(&again));
        assert_eq!(again[0].1, Ok(false));
        assert_eq!(count(), 3);
        std::fs::write(art.caption("bonder"), "another bonder\n").unwrap();
        assert!(landed(&run()));
        assert_eq!(count(), 4);
        assert_eq!(art.read().machine["bonder"].kept, Some(1));
        assert_eq!(attempts(&art.machine("bonder")).len(), 1);
        std::fs::remove_dir_all(art.dir.parent().unwrap()).unwrap();
    }

    #[test]
    fn with_no_attempt_passing_the_one_measured_closest_is_kept() {
        let art = studio("closest", &["converter-amber"], 3);
        let scaffold = Scaffold::of(item("converter-amber"));
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let painter = |_: &Path, out: &Path| {
            let n = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(drawn(out, &turned(&scaffold, [0.4, 0.25, 0.3][n]), None))
        };
        assert!(landed(&remake(
            &art,
            &["converter-amber".to_string()],
            &painter
        )));
        let thresholds = art.read().thresholds;
        let rows = attempts(&art.machine("converter-amber"));
        assert_eq!(rows.len(), 3);
        assert!(rows.values().all(|a| !a.passes(&thresholds)), "{rows:?}");
        assert_eq!(art.read().machine["converter-amber"].kept, Some(2));
        std::fs::remove_dir_all(art.dir.parent().unwrap()).unwrap();
    }

    #[test]
    fn a_spent_call_stops_painting_and_failed_calls_use_up_the_attempts() {
        let art = studio("spent", &["bonder"], 4);
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let spent = |_: &Path, _: &Path| {
            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(Refusal::Spent("usage went from 0 to 0.01".to_string()))
        };
        let results = remake(&art, &["bonder".to_string()], &spent);
        assert!(!landed(&results));
        assert!(
            results[0]
                .1
                .as_ref()
                .unwrap_err()
                .starts_with("stopped: usage"),
            "{results:?}"
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(!art.machine("bonder").join("albedo.png").exists());
        let failed = |_: &Path, _: &Path| {
            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(Refusal::Failed("HTTP 502".to_string()))
        };
        let results = remake(&art, &["bonder".to_string()], &failed);
        assert!(
            results[0]
                .1
                .as_ref()
                .unwrap_err()
                .contains("paint: HTTP 502"),
            "{results:?}"
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 5);
        std::fs::remove_dir_all(art.dir.parent().unwrap()).unwrap();
    }

    #[test]
    fn the_matte_takes_design_colour_under_the_layer_alpha_and_refuses_a_framing_layer() {
        let root = std::env::temp_dir().join(format!("ziral-matte-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let sprite = RgbaImage::from_fn(200, 200, |x, y| {
            if (Vec2::new(x as f32, y as f32) - 100.0).length() < 60.0 {
                Rgba([200, 40, 30, 255])
            } else {
                Rgba([0; 4])
            }
        });
        drawn(&root, &sprite, None);
        let sprite = matte(&root).expect("a sprite");
        assert_eq!(sprite.dimensions(), (250, 250));
        assert_eq!(sprite.get_pixel(125, 125).0, [200, 40, 30, 255]);
        assert_eq!(sprite.get_pixel(5, 5)[3], 0);
        drawn(&root, &sprite, Some(RgbaImage::from_pixel(125, 125, GROUND)));
        assert!(matte(&root).unwrap_err().contains("covers 1.000"));
        let mut touching = RgbaImage::new(125, 125);
        for y in 0..125 {
            for x in 0..30 {
                touching.put_pixel(x, y, Rgba([200, 40, 30, 255]));
            }
        }
        drawn(&root, &sprite, Some(touching));
        assert!(matte(&root).unwrap_err().contains("frame edge"));
        std::fs::remove_file(root.join("layer-2.png")).unwrap();
        assert!(matte(&root).unwrap_err().contains("one layer"));
        std::fs::remove_dir_all(&root).unwrap();
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
                std::fs::read_to_string(&prompt).is_ok_and(|text| !text.trim().is_empty()),
                "{}: no prompt beside the texture",
                prompt.display()
            );
        }
    }
}
