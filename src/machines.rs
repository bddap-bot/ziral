use crate::form::Form;
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

#[derive(Serialize, Deserialize)]
struct Manifest {
    candidates: u32,
    style: Style,
    thresholds: Thresholds,
    machine: BTreeMap<String, Entry>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Style {
    shared: String,
    recipe: String,
    edges: Vec<String>,
    relight: String,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
struct Thresholds {
    outside: f32,
    seat: f32,
    palette: f32,
    off_centre: f32,
    sphere: f32,
}

#[derive(Serialize, Deserialize, Clone)]
struct Entry {
    #[serde(flatten)]
    direction: Direction,
    kept: Option<u32>,
    painted: Option<String>,
    relit: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(try_from = "DirectionKeys", into = "DirectionKeys")]
enum Direction {
    Placeholder(String),
    Given(String),
}

#[derive(Serialize, Deserialize, Default)]
struct DirectionKeys {
    #[serde(skip_serializing_if = "Option::is_none")]
    placeholder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    given: Option<String>,
}

impl TryFrom<DirectionKeys> for Direction {
    type Error = String;

    fn try_from(d: DirectionKeys) -> Result<Direction, String> {
        match (d.placeholder, d.given) {
            (Some(text), None) => Ok(Direction::Placeholder(text)),
            (None, Some(text)) => Ok(Direction::Given(text)),
            (Some(_), Some(_)) => Err("placeholder and given direction both set; keep one".into()),
            (None, None) => Err("neither placeholder nor given direction".into()),
        }
    }
}

impl From<Direction> for DirectionKeys {
    fn from(d: Direction) -> DirectionKeys {
        match d {
            Direction::Placeholder(text) => DirectionKeys {
                placeholder: Some(text),
                ..DirectionKeys::default()
            },
            Direction::Given(text) => DirectionKeys {
                given: Some(text),
                ..DirectionKeys::default()
            },
        }
    }
}

impl Direction {
    fn kind(&self) -> &'static str {
        match self {
            Direction::Placeholder(_) => "placeholder",
            Direction::Given(_) => "given",
        }
    }

    fn text(&self) -> &str {
        match self {
            Direction::Placeholder(text) | Direction::Given(text) => text,
        }
    }
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

    fn candidate(&self, name: &str, index: u32) -> PathBuf {
        self.machine(name)
            .join(format!("candidates/{name}-{index}.png"))
    }

    fn paint_sh(&self) -> PathBuf {
        self.dir.join("../paint.sh")
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

pub(crate) const fn scale() -> f32 {
    PX_PER_HEX / HEX
}

pub(crate) fn canvas(item: Machine) -> u32 {
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

    fn outside(&self, world: Vec2) -> f32 {
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
        let glaze = self.in_cell(world, HEX).and_then(|c| self.mark(c, world));
        rgba(glaze.map_or(KEY, Glaze::rgb), 1.0)
    }

    fn render(&self) -> RgbaImage {
        RgbaImage::from_fn(self.canvas, self.canvas, |x, y| {
            self.paint(self.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5)))
        })
    }

    fn alpha(&self, candidate: &RgbaImage) -> Vec<f32> {
        candidate.pixels().map(|p| opacity(rgb(p))).collect()
    }

    fn cut(&self, candidate: &RgbaImage) -> RgbaImage {
        let (origin, side) = self.crop();
        RgbaImage::from_fn(side, side, |x, y| {
            let c = rgb(candidate.get_pixel(x + origin, y + origin));
            rgba(unspill(c), opacity(c))
        })
    }

    fn register(&self, candidate: &RgbaImage) -> Capture {
        assert_eq!(
            (candidate.width(), candidate.height()),
            (self.canvas, self.canvas),
            "a candidate is painted over the whole scaffold"
        );
        let cells: Vec<Vec2> = self.cells.iter().map(|c| px(c.at)).collect();
        let seats: Option<Vec<Vec2>> = self.cells.iter().map(|c| self.seat(candidate, c)).collect();
        let Some(seats) = seats else {
            return Capture {
                image: candidate.clone(),
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
            bilinear(candidate, self.pixel(onto(world)))
        });
        let off_centre = self
            .cells
            .iter()
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
        for y in origin..origin + side {
            for x in origin..origin + side {
                let i = y as usize * n + x as usize;
                if alpha[i] > 0.0 {
                    let world = self.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
                    outside = outside.max(self.outside(world));
                }
                if self.mask[i] == 1.0 && alpha[i] == 1.0 {
                    inside.add_rgb(rgb(candidate.get_pixel(x, y)));
                }
            }
        }
        let seat = self
            .cells
            .iter()
            .map(|cell| {
                let mut mark = Mean::default();
                let mut centre = Mean::default();
                let mut around = Mean::default();
                for (x, y, p) in candidate.enumerate_pixels() {
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
                apart(mark.rgb(), around.rgb()).max(apart(centre.rgb(), around.rgb()))
            })
            .fold(f32::INFINITY, f32::min);
        let palette = if inside.n > 0.0 {
            Glaze::ALL
                .iter()
                .map(|g| apart(inside.rgb(), g.rgb()))
                .fold(f32::INFINITY, f32::min)
        } else {
            f32::INFINITY
        };
        Score {
            outside,
            seat,
            palette,
            off_centre: capture.off_centre,
        }
    }

    fn seat(&self, candidate: &RgbaImage, cell: &Cell) -> Option<Vec2> {
        let sample = |world: Vec2| {
            let p = self.pixel(world);
            let inside = p.min_element() >= 0.0 && p.max_element() < self.canvas as f32;
            let c = inside.then(|| rgb(candidate.get_pixel(p.x as u32, p.y as u32)))?;
            (opacity(c) > 0.0).then(|| Vec3::from_array(c))
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

    fn sphere_normal(&self, x: u32, y: u32) -> Option<Vec3> {
        let (centre, radius) = self.sphere();
        ball(centre, radius, x, y)
    }

    fn master(&self, candidate: &RgbaImage, light: Option<Vec3>) -> RgbaImage {
        let (centre, radius) = self.sphere();
        let mut out = candidate.clone();
        for (x, y, p) in out.enumerate_pixels_mut() {
            if let Some(n) = self.sphere_normal(x, y) {
                let lit = light.map_or(1.0, |l| AMBIENT + (1.0 - AMBIENT) * n.dot(l).max(0.0));
                *p = grey(SPHERE_ALBEDO * lit);
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

    fn normals(&self, master: &RgbaImage, edits: &[RgbaImage]) -> Relief {
        let master = &self.master(master, None);
        for edit in std::iter::once(master).chain(edits) {
            assert_eq!(
                (edit.width(), edit.height()),
                (self.canvas, self.canvas),
                "a relit edit covers the whole master"
            );
        }
        assert!(edits.len() >= 2, "two relit edits span the tangent plane");
        let lights: Vec<Light> = edits.iter().map(|e| self.light(e)).collect();
        let images: Vec<&RgbaImage> = std::iter::once(master).chain(edits).collect();
        let rows: Vec<[f32; 3]> = std::iter::once([0.0, 0.0, 1.0])
            .chain(lights.iter().map(Light::row))
            .collect();
        let solver = invert(gram(rows.iter().map(|r| (*r, 0.0))).0);
        let n = self.canvas as usize;
        let alpha = self.alpha(master);
        let mut normals = vec![Vec3::Z; n * n];
        let mut mean = Vec3::ZERO;
        for (i, out) in normals.iter_mut().enumerate() {
            let (x, y) = ((i % n) as u32, (i / n) as u32);
            let mut rhs = [0f64; 3];
            for (image, row) in images.iter().zip(&rows) {
                let lum = f64::from(luminance(image.get_pixel(x, y)));
                for (r, a) in rhs.iter_mut().zip(row) {
                    *r += f64::from(*a) * lum;
                }
            }
            let [gx, gy, rho] = apply(&solver, &rhs);
            let tangent = if rho > 0.0 {
                (Vec2::new(gx as f32, gy as f32) / rho as f32).clamp_length_max(1.0)
            } else {
                Vec2::ZERO
            };
            *out = tangent.extend((1.0 - tangent.length_squared()).max(0.0).sqrt());
            if self.mask[i] == 1.0 && alpha[i] == 1.0 {
                mean += *out;
            }
        }
        let sphere = self.sphere_error(&normals);
        let flatten = rotation_to_z(mean.normalize_or(Vec3::Z));
        let (origin, side) = self.crop();
        let normal = RgbaImage::from_fn(side, side, |x, y| {
            let i = (y + origin) as usize * n + (x + origin) as usize;
            let v = if alpha[i] > 0.0 {
                flatten * normals[i]
            } else {
                Vec3::Z
            };
            encode(v.normalize_or(Vec3::Z), alpha[i])
        });
        Relief {
            normal,
            lights,
            sphere,
        }
    }

    fn sphere_error(&self, normals: &[Vec3]) -> f32 {
        let n = self.canvas as usize;
        let mut mean = Mean::default();
        for (i, got) in normals.iter().enumerate() {
            let (x, y) = ((i % n) as u32, (i / n) as u32);
            if let Some(want) = self.sphere_normal(x, y).filter(|w| w.z > SPHERE_CAP) {
                mean.add(got.dot(want).clamp(-1.0, 1.0).acos().to_degrees());
            }
        }
        mean.value()
    }
}

struct Capture {
    image: RgbaImage,
    off_centre: f32,
}

struct Relief {
    normal: RgbaImage,
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
        [d.x, d.y, self.ambient]
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

#[cfg(test)]
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

fn rotation_to_z(from: Vec3) -> bevy::math::Mat3 {
    use bevy::math::{Mat3, Quat};
    Mat3::from_quat(Quat::from_rotation_arc(from, Vec3::Z))
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Score {
    outside: f32,
    seat: f32,
    palette: f32,
    off_centre: f32,
}

impl Score {
    fn failing(&self, t: &Thresholds) -> Option<(&'static str, f32, &'static str, f32)> {
        if self.outside > t.outside {
            Some(("outside", self.outside, ">", t.outside))
        } else if self.seat < t.seat {
            Some(("seat", self.seat, "<", t.seat))
        } else if self.palette > t.palette {
            Some(("palette", self.palette, ">", t.palette))
        } else if self.off_centre > t.off_centre {
            Some(("off_centre", self.off_centre, ">", t.off_centre))
        } else {
            None
        }
    }

    fn passes(&self, t: &Thresholds) -> bool {
        self.failing(t).is_none()
    }

    fn total(&self) -> f32 {
        self.seat - self.outside - self.palette
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

fn spill(c: [f32; 3]) -> f32 {
    c[1] - c[0].max(c[2])
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

const PAINTERS: usize = 6;
const RELIGHTS: u32 = 3;

struct Paint {
    images: Vec<PathBuf>,
    output: PathBuf,
    prompt: String,
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
        .arg(&job.prompt)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("{}: {e}", art.paint_sh().display()))?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    for line in stderr.lines().filter(|l| !l.trim().is_empty()) {
        eprintln!("{stem}: {line}");
    }
    if output.status.success() {
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

fn painted_key(prompt: &str, count: u32, scaffold: &RgbaImage, recipe: Option<&Form>) -> String {
    let recipe = recipe.map(ToString::to_string).unwrap_or_default();
    key(&[
        prompt.as_bytes(),
        &count.to_le_bytes(),
        &scaffold.width().to_le_bytes(),
        scaffold.as_raw(),
        recipe.as_bytes(),
    ])
}

fn prompt(style: &Style, item: Machine, direction: &Direction) -> String {
    match item.recipe() {
        Some(_) => format!("{} {} {}", style.shared, style.recipe, direction.text()),
        None => format!("{} {}", style.shared, direction.text()),
    }
}

fn relit_key(kept: &[u8], style: &Style) -> String {
    let edges = style.edges.join("\n");
    key(&[kept, style.relight.as_bytes(), edges.as_bytes()])
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
            Some((rule, got, sign, bound)) => format!("fail {rule} {got:.3} {sign} {bound}"),
        }
    }

    fn row(&self, candidate: &str, t: &Thresholds) -> String {
        format!(
            "{candidate}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{}",
            self.outside,
            self.seat,
            self.palette,
            self.off_centre,
            self.verdict(t)
        )
    }
}

struct Prepared {
    scaffold: Scaffold,
    style: Style,
    thresholds: Thresholds,
    count: u32,
    entry: Entry,
    prompt: String,
    painted: String,
    images: Vec<PathBuf>,
    wipe: bool,
    changed: bool,
}

struct Remake<'a> {
    art: &'a Art,
    manifest: std::sync::Mutex<Manifest>,
    painter: Painter<'a>,
    cap: Cap,
    painted: std::sync::atomic::AtomicBool,
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
                                self.painted
                                    .store(true, std::sync::atomic::Ordering::SeqCst);
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

    fn record(&self, name: &str, kept: Option<u32>, painted: &str, relit: Option<&str>) {
        let mut m = self.manifest.lock().expect("the manifest is unpoisoned");
        let entry = m.machine.get_mut(name).expect("the entry read above");
        let now = (kept, Some(painted.to_string()), relit.map(str::to_string));
        if (entry.kept, entry.painted.clone(), entry.relit.clone()) != now {
            (entry.kept, entry.painted, entry.relit) = now;
            self.art.write(&m);
        }
    }

    fn score(&self, name: &str, scaffold: &Scaffold, count: u32) -> Vec<(u32, Score)> {
        (1..=count)
            .filter(|i| self.art.candidate(name, *i).exists())
            .map(|i| {
                let png = open(self.art.candidate(name, i));
                (i, scaffold.score(&scaffold.register(&png)))
            })
            .collect()
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
        let prompt = prompt(&style, item(name), &entry.direction);
        let painted = painted_key(&prompt, count, &rendered, item(name).recipe());
        let wipe = entry.painted.as_deref() != Some(painted.as_str());
        let mut images = vec![scaffold_png];
        if item(name).recipe().is_some() {
            let recipe_png = dir.join("recipe.png");
            if wipe || !recipe_png.exists() {
                crate::shot::recipe(item(name), &recipe_png);
                changed = true;
            }
            images.push(recipe_png);
        }
        Ok(Prepared {
            scaffold,
            style,
            thresholds,
            count,
            entry,
            prompt,
            painted,
            images,
            wipe,
            changed,
        })
    }

    fn machine(&self, name: &str, prepared: Prepared) -> Result<bool, String> {
        let started = std::time::Instant::now();
        let Prepared {
            scaffold,
            style,
            thresholds,
            count,
            entry,
            prompt,
            painted,
            images,
            mut wipe,
            mut changed,
        } = prepared;
        let dir = self.art.machine(name);
        let candidates = dir.join("candidates");
        let mut kept = entry.kept.filter(|k| self.art.candidate(name, *k).exists());
        let mut landed = None;
        for _ in 0..2 {
            if wipe {
                let _ = std::fs::remove_dir_all(&candidates);
                std::fs::create_dir_all(&candidates).map_err(|e| e.to_string())?;
                kept = None;
            }
            let jobs: Vec<(String, Paint)> = (1..=count)
                .filter(|i| !self.art.candidate(name, *i).exists())
                .map(|i| {
                    (
                        format!("{name}-{i}"),
                        Paint {
                            images: images.clone(),
                            output: self.art.candidate(name, i),
                            prompt: prompt.clone(),
                            size: scaffold.canvas,
                        },
                    )
                })
                .collect();
            let asked = jobs.len();
            let failed: Vec<String> = self
                .paint(jobs)
                .into_iter()
                .filter_map(|(label, r)| r.err().map(|e| format!("{label}: {e}")))
                .collect();
            let painted_now = asked > failed.len();
            changed |= painted_now;
            if painted_now || wipe {
                self.record(name, None, &painted, None);
            }
            let rows = |scored: &[(u32, Score)]| -> Vec<String> {
                let rows: Vec<String> = scored
                    .iter()
                    .map(|(i, s)| s.row(&format!("{name}-{i}"), &thresholds))
                    .collect();
                for row in &rows {
                    println!("{row}");
                }
                rows
            };
            let only_kept = kept.filter(|_| asked == 0);
            let mut scored = match only_kept {
                Some(k) => {
                    let png = open(self.art.candidate(name, k));
                    vec![(k, scaffold.score(&scaffold.register(&png)))]
                }
                None => self.score(name, &scaffold, count),
            };
            let mut printed = rows(&scored);
            let passing = |scored: &[(u32, Score)], i: u32| {
                scored.iter().any(|(k, s)| *k == i && s.passes(&thresholds))
            };
            if let Some(k) = kept.filter(|k| passing(&scored, *k)) {
                if asked > 0 || wipe {
                    std::fs::write(dir.join("scores.tsv"), printed.join("\n") + "\n")
                        .map_err(|e| e.to_string())?;
                }
                landed = Some(k);
                break;
            }
            if only_kept.is_some() {
                scored = self.score(name, &scaffold, count);
                printed = rows(&scored);
            }
            std::fs::write(dir.join("scores.tsv"), printed.join("\n") + "\n")
                .map_err(|e| e.to_string())?;
            if let Some(k) = scored
                .iter()
                .filter(|(_, s)| s.passes(&thresholds))
                .max_by(|a, b| a.1.total().total_cmp(&b.1.total()))
                .map(|(i, _)| *i)
            {
                landed = Some(k);
                break;
            }
            if asked > 0 {
                let why = if failed.is_empty() {
                    String::new()
                } else {
                    format!(
                        "; {} of {count} paints failed: {}",
                        failed.len(),
                        failed.join(", ")
                    )
                };
                return Err(format!("no candidate passes{why}"));
            }
            wipe = true;
        }
        let kept = landed.ok_or_else(|| "no candidate passes".to_string())?;
        let kept_png = read(&self.art.candidate(name, kept));
        let relit = relit_key(&kept_png, &style);
        let current = entry.relit.as_deref() == Some(relit.as_str())
            && dir.join("albedo.png").exists()
            && dir.join("normal.png").exists();
        if !current {
            self.relief(name, &scaffold, kept, &style, thresholds)?;
            changed = true;
        }
        self.record(name, Some(kept), &painted, Some(&relit));
        println!(
            "{name}\t{}\tkept {name}-{kept}\t{:.0}s",
            if changed { "landed" } else { "up to date" },
            started.elapsed().as_secs_f32()
        );
        Ok(changed)
    }

    fn relief(
        &self,
        name: &str,
        scaffold: &Scaffold,
        kept: u32,
        style: &Style,
        thresholds: Thresholds,
    ) -> Result<(), String> {
        let dir = self.art.machine(name);
        let capture = scaffold.register(&open(self.art.candidate(name, kept)));
        let relit = dir.join("relit");
        let _ = std::fs::remove_dir_all(&relit);
        std::fs::create_dir_all(&relit).map_err(|e| e.to_string())?;
        let master = scaffold.master(&capture.image, Some(Vec3::Z));
        save(&master, &relit.join("master.png"));
        let mut why = String::new();
        for _ in 0..RELIGHTS {
            let jobs = style
                .edges
                .iter()
                .map(|edge| {
                    (
                        format!("{name}-{edge}"),
                        Paint {
                            images: vec![relit.join("master.png")],
                            output: relit.join(format!("{edge}.png")),
                            prompt: style.relight.replace("{edge}", edge),
                            size: scaffold.canvas,
                        },
                    )
                })
                .collect();
            let painted = self.paint(jobs);
            let edits: Vec<(&str, RgbaImage)> = painted
                .iter()
                .zip(&style.edges)
                .filter(|((_, r), _)| r.is_ok())
                .map(|(_, edge)| (edge.as_str(), open(relit.join(format!("{edge}.png")))))
                .collect();
            if edits.len() < style.edges.len() {
                why = format!(
                    "{} of {} relights failed",
                    painted.len() - edits.len(),
                    painted.len()
                );
                continue;
            }
            let images: Vec<RgbaImage> = edits.iter().map(|(_, e)| e.clone()).collect();
            let relief = scaffold.normals(&master, &images);
            let mut lights: Vec<String> = edits
                .iter()
                .zip(&relief.lights)
                .map(|((edge, _), l)| {
                    let d = l.direction;
                    format!(
                        "{edge}\tlight=({:+.2},{:+.2},{:+.2})\tambient={:.2}",
                        d.x, d.y, d.z, l.ambient
                    )
                })
                .collect();
            lights.push(format!("sphere error {:.1} degrees", relief.sphere));
            println!("{name}\t{}", lights.last().expect("the sphere line"));
            std::fs::write(relit.join("lights.txt"), lights.join("\n") + "\n")
                .map_err(|e| e.to_string())?;
            if relief.sphere <= thresholds.sphere {
                save(&scaffold.cut(&capture.image), &dir.join("albedo.png"));
                save(&relief.normal, &dir.join("normal.png"));
                quantise(&dir.join("albedo.png"))?;
                return Ok(());
            }
            why = format!(
                "the calibration sphere came back {:.1} degrees off, over {}",
                relief.sphere, thresholds.sphere
            );
        }
        Err(format!("{why}, {RELIGHTS} relights over"))
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
                let cols: Vec<&str> = row.split('\t').collect();
                let [candidate, outside, seat, palette, off_centre, verdict] = cols[..] else {
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
                let rule: Vec<&str> = verdict.split(' ').take(2).collect();
                args.push("-label".into());
                args.push(
                    format!(
                        "{kept}{candidate}  {outside} {seat} {palette} {off_centre}  {}",
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

fn remake(art: &Art, names: &[String], painter: Painter) -> Vec<(String, Result<bool, String>)> {
    let remake = Remake {
        art,
        manifest: std::sync::Mutex::new(art.read()),
        painter,
        cap: Cap::new(PAINTERS),
        painted: std::sync::atomic::AtomicBool::new(false),
    };
    let mut names: Vec<&String> = names.iter().collect();
    names.sort();
    names.dedup();
    let prepared: Vec<(String, Result<Prepared, String>)> = names
        .iter()
        .map(|name| (name.to_string(), remake.prepare(name)))
        .collect();
    let remake = &remake;
    let results: Vec<(String, Result<bool, String>)> = std::thread::scope(|s| {
        let handles: Vec<_> = prepared
            .into_iter()
            .map(|(name, prepared)| {
                s.spawn(move || {
                    let result = prepared.and_then(|p| remake.machine(&name, p));
                    (name, result)
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|h| h.join().expect("a machine returns"))
            .collect()
    });
    let sheet = art.dir.join("sheet.png");
    if (!sheet.exists() || remake.painted.load(std::sync::atomic::Ordering::SeqCst))
        && let Err(e) = remake.sheet()
    {
        eprintln!("sheet: {e}");
    }
    results
}

fn violators(art: &Art, manifest: &Manifest) -> Vec<String> {
    let violates = |name: &str| {
        let dir = art.machine(name);
        let Some(kept) = manifest.machine.get(name).and_then(|m| m.kept) else {
            println!("{name}\tkeeps no candidate");
            return true;
        };
        let png = art.candidate(name, kept);
        if !png.exists() || !dir.join("albedo.png").exists() || !dir.join("normal.png").exists() {
            println!("{name}\t{name}-{kept} or its maps are missing");
            return true;
        }
        let scaffold = Scaffold::of(item(name));
        let score = scaffold.score(&scaffold.register(&open(png)));
        println!("{name}-{kept}\t{}", score.verdict(&manifest.thresholds));
        !score.passes(&manifest.thresholds)
    };
    std::thread::scope(|s| {
        let handles: Vec<_> = Machine::ALL
            .into_iter()
            .map(name)
            .map(|name| s.spawn(move || violates(name).then(|| name.to_string())))
            .collect();
        handles
            .into_iter()
            .filter_map(|h| h.join().expect("a verdict returns"))
            .collect()
    })
}

fn plan(manifest: &Manifest) -> String {
    let mut out = String::new();
    let mut placeholders = 0;
    for item in Machine::ALL {
        let name = name(item);
        let entry = &manifest.machine[name];
        placeholders += usize::from(matches!(entry.direction, Direction::Placeholder(_)));
        let recipe = match item.recipe() {
            Some(_) => "recipe",
            None => "no recipe",
        };
        out += &format!("{name}\t{}\t{recipe}\n", entry.direction.kind());
    }
    out += &format!("{placeholders} placeholder\n");
    out
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
    const USAGE: &str =
        "usage: ziral --plan | ziral --gen NAME... | ziral --gen --all | ziral --gen --violators";
    let art = Art::shipped();
    if args.get(1).map(String::as_str) == Some("--plan") {
        print!("{}", plan(&art.read()));
        return Some(0);
    }
    if args.get(1).map(String::as_str) != Some("--gen") {
        return None;
    }
    let rest: Vec<&str> = args.iter().skip(2).map(String::as_str).collect();
    let known = |n: &str| Machine::ALL.into_iter().map(name).any(|k| k == n);
    let names: Vec<String> = match rest.as_slice() {
        ["--all"] => Machine::ALL
            .into_iter()
            .map(|i| name(i).to_string())
            .collect(),
        ["--violators"] => violators(&art, &art.read()),
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
    let painter = |job: &Paint| paint_sh(&art, job);
    Some(i32::from(!landed(&remake(&art, &names, &painter))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::light;
    use crate::sim::{Arm, Glyph, Hex, ORIGIN};

    const BUMP: f32 = 0.3;
    const RELIEF: f32 = 0.1;
    const TILT: f32 = 0.15;
    const ASPECT: f32 = 0.02;
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
                Machine::Arm => Arm::new(ORIGIN, 0, Vec::new()).cells().to_vec(),
                Machine::Glyph(kind) => Glyph {
                    kind,
                    at: ORIGIN,
                    dir: 0,
                }
                .slots()
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
            assert!(score.passes(&manifest.thresholds), "{name}: {score:?}");
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
            let prompt = prompt(&manifest.style, item, &machine.direction);
            assert_eq!(
                machine.painted.as_deref(),
                Some(painted_key(&prompt, manifest.candidates, &want, item.recipe()).as_str()),
                "{name}: the candidates are stale against the manifest: run ziral --gen {name}"
            );
            let kept_png = read(&dir.join(format!("candidates/{name}-{kept}.png")));
            assert_eq!(
                machine.relit.as_deref(),
                Some(relit_key(&kept_png, &manifest.style).as_str()),
                "{name}: the maps are stale against the kept candidate: run ziral --gen {name}"
            );
        }
    }

    #[test]
    fn a_candidate_painted_beyond_its_footprint_is_rejected() {
        let scaffold = Scaffold::of(Machine::Glyph(crate::sim::GlyphKind::Bonder));
        let thresholds = Art::shipped().read().thresholds;
        let clean = fired(&scaffold, &|w| w);
        let score = scaffold.score(&scaffold.register(&clean));
        assert!(score.passes(&thresholds), "{score:?}");
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
            assert_eq!(score.passes(&thresholds), ok, "{score:?}");
            assert!(
                (score.outside - k * thresholds.outside).abs() <= SEAT_STEP,
                "{score:?} against {k} tolerances"
            );
        }
    }

    #[test]
    fn a_capture_whose_seats_sit_off_their_cell_centres_is_rejected() {
        let scaffold = Scaffold::of(Machine::Glyph(crate::sim::GlyphKind::SecondBond));
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
        assert!(score.passes(&thresholds), "{score:?}");
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
            assert!(score.passes(&thresholds), "{name}: {score:?}");
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
        assert!(!score.passes(&thresholds), "{score:?}");
        let off = Score {
            off_centre: score.off_centre,
            ..scaffold.score(&scaffold.register(&clean))
        };
        assert!(
            !off.passes(&thresholds),
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
    fn a_lambertian_sphere_returns_its_own_lights_and_a_bump_its_own_normals() {
        let scaffold = Scaffold::of(Machine::Glyph(crate::sim::GlyphKind::Source));
        let base = RgbaImage::from_pixel(scaffold.canvas, scaffold.canvas, grey(SPHERE_ALBEDO));
        let bump = scaffold.pixel(px(scaffold.cells[0].at));
        let radius = BUMP * HEX * scale();
        let lights = [
            Vec3::new(1.0, 0.2, 0.2),
            Vec3::new(0.1, 1.0, 0.2),
            Vec3::new(-1.0, 0.3, 0.2),
            Vec3::new(0.0, -1.0, 0.2),
        ]
        .map(Vec3::normalize);
        let edits: Vec<RgbaImage> = lights
            .iter()
            .map(|l| {
                let mut e = base.clone();
                for (x, y, p) in e.enumerate_pixels_mut() {
                    let n = scaffold
                        .sphere_normal(x, y)
                        .or_else(|| ball(bump, radius, x, y))
                        .unwrap_or(Vec3::Z);
                    *p = grey(SPHERE_ALBEDO * (AMBIENT + (1.0 - AMBIENT) * n.dot(*l).max(0.0)));
                }
                e
            })
            .collect();
        let relief = scaffold.normals(&scaffold.master(&base, None), &edits);
        for (got, want) in relief.lights.iter().zip(lights) {
            assert!(got.direction.dot(want) > 0.999, "{got:?} vs {want:?}");
            assert!((got.ambient - AMBIENT).abs() < 0.02, "{got:?}");
        }
        let thresholds = Art::shipped().read().thresholds;
        assert!(relief.sphere <= thresholds.sphere, "{}", relief.sphere);
        let (origin, _) = scaffold.crop();
        let at = |dx: f32, dy: f32| {
            let p = bump + Vec2::new(dx, dy) * radius / 2.0 - Vec2::splat(origin as f32);
            decode(relief.normal.get_pixel(p.x as u32, p.y as u32))
        };
        assert!(at(1.0, 0.0).x > TILT, "{:?}", at(1.0, 0.0));
        assert!(at(-1.0, 0.0).x < -TILT, "{:?}", at(-1.0, 0.0));
        assert!(at(0.0, -1.0).y > TILT, "{:?}", at(0.0, -1.0));
        assert!(at(0.0, 1.0).y < -TILT, "{:?}", at(0.0, 1.0));
    }

    fn studio(tag: &str, edges: &[&str], machines: &[&str]) -> Art {
        let shipped = Art::shipped();
        let root = std::env::temp_dir().join(format!("ziral-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let art = Art {
            dir: root.join("machines"),
        };
        std::fs::create_dir_all(&art.dir).expect("a studio is creatable");
        std::fs::copy(shipped.paint_sh(), art.paint_sh()).expect("paint.sh copies");
        let style = shipped.read().style;
        art.write(&Manifest {
            candidates: 2,
            style: Style {
                edges: edges.iter().map(|e| e.to_string()).collect(),
                ..style
            },
            thresholds: shipped.read().thresholds,
            machine: machines
                .iter()
                .map(|name| {
                    (
                        name.to_string(),
                        Entry {
                            direction: Direction::Placeholder(format!("a {name}")),
                            kept: None,
                            painted: None,
                            relit: None,
                        },
                    )
                })
                .collect(),
        });
        art
    }

    fn fake(job: &Paint) -> Result<(), String> {
        let stem = job
            .output
            .file_stem()
            .and_then(|s| s.to_str())
            .expect("an output stem");
        let input = &job.images[0];
        if input.ends_with("scaffold.png") {
            let (name, index) = stem.rsplit_once('-').expect("name-index");
            let scaffold = Scaffold::of(item(name));
            let shift = Vec2::X * (index.parse::<f32>().expect("an index") - 1.0);
            save(&fired(&scaffold, &|w| w - shift), &job.output);
        } else {
            let name = input
                .parent()
                .and_then(Path::parent)
                .and_then(Path::file_name)
                .and_then(|s| s.to_str())
                .expect("relit/master.png under the machine dir");
            let scaffold = Scaffold::of(item(name));
            let light = match stem {
                "right" => Vec3::new(1.0, 0.0, 0.2),
                "top" => Vec3::new(0.0, 1.0, 0.2),
                "left" => Vec3::new(-1.0, 0.0, 0.2),
                "bottom" => Vec3::new(0.0, -1.0, 0.2),
                edge => panic!("no light for {edge}"),
            }
            .normalize();
            let mut edit = open(input);
            for (x, y, p) in edit.enumerate_pixels_mut() {
                if let Some(n) = scaffold.sphere_normal(x, y) {
                    *p = grey(SPHERE_ALBEDO * (AMBIENT + (1.0 - AMBIENT) * n.dot(light).max(0.0)));
                }
            }
            save(&edit, &job.output);
        }
        Ok(())
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
        let art = studio("key", &["right", "top", "left", "bottom"], &["source"]);
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let painter = counted(&calls, &fake);
        let names = ["source".to_string()];
        let run = |calls: &std::sync::atomic::AtomicUsize| {
            calls.store(0, std::sync::atomic::Ordering::SeqCst);
            let results = remake(&art, &names, &painter);
            assert_eq!(results.len(), 1, "{results:?}");
            let changed = results[0].1.clone().expect("source lands");
            (changed, calls.load(std::sync::atomic::Ordering::SeqCst))
        };
        assert_eq!(run(&calls), (true, 6));
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
        m.machine.get_mut("source").expect("source").direction =
            Direction::Placeholder("another source".into());
        art.write(&m);
        assert_eq!(run(&calls), (true, 2));
        let mut m = art.read();
        m.machine.get_mut("source").expect("source").kept = Some(3 - kept);
        art.write(&m);
        assert_eq!(run(&calls), (true, 4));
        assert_eq!(art.read().machine["source"].kept, Some(3 - kept));
        assert_ne!(read(&dir.join("albedo.png")), albedo);
        assert_eq!(run(&calls), (false, 0));
    }

    #[test]
    fn given_direction_survives_the_manifest_byte_identical_under_the_shared_text() {
        let art = studio("given", &["right", "top"], &["source"]);
        let text =
            "  \"Quoted\", back\\slash, tab\t, a line\nbreak, an em—dash, and trailing spaces   ";
        let mut m = art.read();
        m.machine.get_mut("source").expect("source").direction = Direction::Given(text.into());
        art.write(&m);
        let written = std::fs::read_to_string(art.manifest()).expect("the manifest");
        assert!(written.contains("\ngiven = "), "{written}");
        assert!(!written.contains("placeholder"), "{written}");
        let read = art.read();
        let direction = &read.machine["source"].direction;
        assert_eq!(*direction, Direction::Given(text.into()));
        assert_eq!(
            prompt(
                &read.style,
                Machine::Glyph(crate::sim::GlyphKind::Source),
                direction
            ),
            format!("{} {text}", read.style.shared)
        );
    }

    #[test]
    fn an_entry_with_both_or_neither_direction_is_refused() {
        let shipped = std::fs::read_to_string(Art::shipped().manifest()).expect("the manifest");
        let source = shipped.find("[machine.source]").expect("a source entry");
        let line = shipped[source..]
            .find("\nplaceholder = ")
            .map(|i| source + i + 1)
            .expect("the source's placeholder line");
        let end = line + shipped[line..].find('\n').expect("a line end");
        let both = format!(
            "{}\ngiven = \"a source\"{}",
            &shipped[..end],
            &shipped[end..]
        );
        let neither = format!("{}{}", &shipped[..line], &shipped[end + 1..]);
        assert!(toml::from_str::<Manifest>(&shipped).is_ok());
        let err = |text: &str| {
            toml::from_str::<Manifest>(text)
                .err()
                .expect("refused")
                .to_string()
        };
        assert!(
            err(&both).contains("placeholder and given"),
            "{}",
            err(&both)
        );
        assert!(err(&neither).contains("neither"), "{}", err(&neither));
    }

    #[test]
    fn the_plan_names_every_placeholder_machine_and_counts_them() {
        let mut m = Art::shipped().read();
        let names: Vec<&str> = Machine::ALL.into_iter().map(name).collect();
        let lines = |m: &Manifest| plan(m).lines().map(str::to_string).collect::<Vec<_>>();
        let all = lines(&m);
        assert_eq!(all.len(), names.len() + 1);
        for (line, name) in all.iter().zip(&names) {
            assert!(
                line.starts_with(&format!("{name}\tplaceholder\t")),
                "{line}"
            );
        }
        assert_eq!(all.last().map(String::as_str), Some("7 placeholder"));
        m.machine.get_mut("arm").expect("arm").direction = Direction::Given("an arm".into());
        let one_given = lines(&m);
        assert!(
            one_given.contains(&"arm\tgiven\trecipe".to_string()),
            "{one_given:?}"
        );
        assert!(one_given.contains(&"source\tplaceholder\tno recipe".to_string()));
        assert_eq!(
            one_given
                .iter()
                .filter(|l| l.contains("\tplaceholder\t"))
                .count(),
            names.len() - 1
        );
        assert_eq!(one_given.last().map(String::as_str), Some("6 placeholder"));
    }

    #[test]
    fn a_machine_with_a_recipe_paints_over_its_recipe_render_and_one_without_over_the_scaffold_alone()
     {
        let art = studio(
            "recipe",
            &["right", "top", "left", "bottom"],
            &["bonder", "source"],
        );
        let jobs: std::sync::Mutex<Vec<(String, Vec<PathBuf>, String)>> =
            std::sync::Mutex::new(Vec::new());
        let painter = |job: &Paint| {
            let stem = job
                .output
                .file_stem()
                .unwrap()
                .to_string_lossy()
                .into_owned();
            jobs.lock()
                .unwrap()
                .push((stem, job.images.clone(), job.prompt.clone()));
            fake(job)
        };
        let names = ["bonder".to_string(), "source".to_string()];
        assert!(landed(&remake(&art, &names, &painter)));
        let recorded = jobs.lock().unwrap();
        let style = art.read().style;
        for (name, item) in [
            ("bonder", Machine::Glyph(crate::sim::GlyphKind::Bonder)),
            ("source", Machine::Glyph(crate::sim::GlyphKind::Source)),
        ] {
            let dir = art.machine(name);
            let candidates: Vec<_> = recorded
                .iter()
                .filter(|(stem, _, _)| {
                    stem.starts_with(&format!("{name}-"))
                        && stem[name.len() + 1..].parse::<u32>().is_ok()
                })
                .collect();
            assert_eq!(candidates.len(), 2, "{name}");
            let mut want = vec![dir.join("scaffold.png")];
            if item.recipe().is_some() {
                want.push(dir.join("recipe.png"));
            }
            for (_, images, prompt) in &candidates {
                assert_eq!(*images, want, "{name}");
                assert_eq!(
                    prompt.contains(&style.recipe),
                    item.recipe().is_some(),
                    "{name}"
                );
            }
            let relights: Vec<_> = recorded
                .iter()
                .filter(|(stem, images, _)| {
                    style.edges.contains(stem) && images[0].starts_with(&dir)
                })
                .collect();
            assert_eq!(relights.len(), style.edges.len(), "{name}");
            assert!(relights.iter().all(|(_, images, _)| images.len() == 1));
            assert_eq!(
                dir.join("recipe.png").exists(),
                item.recipe().is_some(),
                "{name}"
            );
        }
        let recipe_png = art.machine("bonder").join("recipe.png");
        let before = read(&recipe_png);
        let recipe = open(&recipe_png);
        let canvas = canvas(Machine::Glyph(crate::sim::GlyphKind::Output(
            crate::sim::Tier::One,
        )));
        assert_eq!((recipe.width(), recipe.height()), (canvas, canvas));
        let corner = *recipe.get_pixel(0, 0);
        let drawn: Vec<(u32, u32)> = recipe
            .enumerate_pixels()
            .filter(|(_, _, p)| **p != corner)
            .map(|(x, y, _)| (x, y))
            .collect();
        let share = drawn.len() as f32 / recipe.pixels().len() as f32;
        assert!(
            share > 0.02,
            "the render is blank: {share:.3} differs from the corner"
        );
        let (lo, hi) = drawn.iter().fold(
            ((u32::MAX, u32::MAX), (0, 0)),
            |((x0, y0), (x1, y1)), (x, y)| ((x0.min(*x), y0.min(*y)), (x1.max(*x), y1.max(*y))),
        );
        let inset = band() as u32;
        assert!(
            lo.0 >= inset && lo.1 >= inset && hi.0 < canvas - inset && hi.1 < canvas - inset,
            "the compound reaches the frame: {lo:?}..{hi:?} on {canvas}"
        );
        let jobs_before = recorded.len();
        drop(recorded);
        assert!(landed(&remake(&art, &names, &painter)));
        assert_eq!(
            jobs.lock().unwrap().len(),
            jobs_before,
            "a second run paints nothing"
        );
        assert_eq!(read(&recipe_png), before, "a second run leaves the render");
    }

    #[test]
    fn a_machine_whose_candidates_all_fail_to_paint_does_not_land_and_the_others_still_do() {
        let art = studio(
            "exit",
            &["right", "top", "left", "bottom"],
            &["bonder", "source"],
        );
        let painter = |job: &Paint| {
            if job.output.to_string_lossy().contains("bonder") {
                Err("boom".to_string())
            } else {
                fake(job)
            }
        };
        let names = ["bonder".to_string(), "source".to_string()];
        let results = remake(&art, &names, &painter);
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
        let results = remake(&art, &names, &fake);
        assert!(landed(&results), "{results:?}");
        assert_eq!(by_name(&results, "source"), Ok(false));
        assert_eq!(by_name(&results, "bonder"), Ok(true));
        assert!(art.read().machine["bonder"].kept.is_some());
        assert!(landed(&remake(&art, &names[1..], &fake)));
        assert!(landed(&[]));
    }

    #[test]
    fn paint_sh_retries_the_image_tool_with_backoff_and_reports_every_failed_attempt() {
        let root = std::env::temp_dir().join(format!("ziral-paint-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("bin")).expect("a fake bin");
        let codex = root.join("bin/codex");
        std::fs::write(
            &codex,
            "#!/bin/sh\n\
             n=$(cat \"$HOME/attempts\" 2>/dev/null || echo 0)\n\
             n=$((n + 1))\n\
             printf %s \"$n\" > \"$HOME/attempts\"\n\
             [ \"$n\" -ge \"$PASS_ON\" ] || { echo \"codex: boom $n\" >&2; exit 1; }\n\
             mkdir -p \"$HOME/.codex/generated_images/t$n\"\n\
             magick -size 64x64 xc:'#00ff00' \"$HOME/.codex/generated_images/t$n/a.png\"\n\
             echo \"{\\\"type\\\":\\\"thread.started\\\",\\\"thread_id\\\":\\\"t$n\\\"}\"\n",
        )
        .expect("a fake codex");
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&codex, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        let path = format!(
            "{}:{}",
            root.join("bin").display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let paint = |pass_on: u32| {
            let _ = std::fs::remove_file(root.join("attempts"));
            let _ = std::fs::remove_file(root.join("out.png"));
            let started = std::time::Instant::now();
            let out = std::process::Command::new(Art::shipped().paint_sh())
                .args(["-s", "64"])
                .arg(root.join("out.png"))
                .arg("a prompt")
                .env("PATH", &path)
                .env("HOME", &root)
                .env("PASS_ON", pass_on.to_string())
                .output()
                .expect("paint.sh runs");
            let attempts: u32 = std::fs::read_to_string(root.join("attempts"))
                .expect("attempts")
                .parse()
                .expect("a count");
            (
                out.status.success(),
                attempts,
                String::from_utf8_lossy(&out.stderr).into_owned(),
                started.elapsed().as_secs_f32(),
            )
        };
        let (ok, attempts, stderr, took) = paint(3);
        assert!(ok, "{stderr}");
        assert!(root.join("out.png").exists());
        assert_eq!(attempts, 3);
        for line in [
            "codex: boom 1",
            "attempt 1 of 4 failed, retrying in 1s",
            "codex: boom 2",
            "attempt 2 of 4 failed, retrying in 2s",
        ] {
            assert!(stderr.contains(line), "{line:?} missing from {stderr}");
        }
        assert!(!stderr.contains("attempt 3 of 4"), "{stderr}");
        assert!(took >= 3.0, "{took}s: no backoff");
        let (ok, attempts, stderr, took) = paint(99);
        assert!(!ok);
        assert!(!root.join("out.png").exists());
        assert_eq!(attempts, 4);
        assert!(
            stderr.contains("attempt 3 of 4 failed, retrying in 4s"),
            "{stderr}"
        );
        assert!(stderr.contains("gave up after 4 attempts"), "{stderr}");
        assert!(took >= 7.0, "{took}s: no backoff");
    }
}
