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
const CRITIC_PX: u32 = 512;

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
    critic: String,
    edges: Vec<String>,
    relight: String,
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
    kept: Option<u32>,
    briefed: Option<String>,
    painted: Option<String>,
    relit: Option<String>,
    judged: Option<String>,
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

    fn ask_sh(&self) -> PathBuf {
        self.dir.join("../ask.sh")
    }

    fn prompt(&self, name: &str) -> PathBuf {
        self.machine(name).join("prompt.txt")
    }

    fn judged(&self, name: &str, index: u32) -> PathBuf {
        self.machine(name)
            .join(format!("judged/{name}-{index}.png"))
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
            critic: None,
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

#[derive(Debug, Clone, PartialEq)]
struct Score {
    outside: f32,
    seat: f32,
    palette: f32,
    off_centre: f32,
    critic: Option<Critic>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Critic {
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
        self.measured(t)
    }

    fn passes(&self, t: &Thresholds) -> bool {
        self.failing(t).is_none()
    }

    fn reaches(&self, t: &Thresholds) -> bool {
        self.passes(t)
            && self
                .critic
                .as_ref()
                .is_some_and(|critic| critic.score >= t.critic)
    }

    fn rank(&self) -> (u8, f32) {
        (
            self.critic.as_ref().map_or(0, |j| j.score),
            self.seat - self.outside - self.palette,
        )
    }

    fn issues(&self) -> &[String] {
        self.critic.as_ref().map_or(&[], |j| &j.issues)
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
const ROUNDS: usize = 3;

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

fn brief(style: &Style, item: Machine, direction: &str) -> String {
    match item.recipe() {
        Some(_) => format!("{} {} {}", style.shared, style.recipe, direction),
        None => format!("{} {}", style.shared, direction),
    }
}

fn stored(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text.trim().to_string()).filter(|text| !text.is_empty())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

fn store(path: &Path, prompt: &str) -> Result<String, String> {
    let prompt = prompt.trim();
    if prompt.is_empty() {
        return Err(format!("{}: an empty prompt was written", path.display()));
    }
    std::fs::write(path, format!("{prompt}\n")).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(prompt.to_string())
}

const DIRECTOR: &str = "You are the art director for one game sprite and you write the prompt an image generation model will receive. The brief below states the facts the finished picture must have; the wording of its direction is a starting point you may change or drop as you see fit, and within those facts the art direction is yours: be creative. Write the prompt as a declarative caption describing the finished picture, its subject, layout, materials, light and what is absent, never as instructions to the model. Answer with exactly one JSON object and nothing else, {\"prompt\": the caption as one string}; do not generate an image, run commands, edit anything or write files. Brief:";
const CAPTION_SCHEMA: &str = r#"{"type":"object","properties":{"prompt":{"type":"string","minLength":1}},"required":["prompt"],"additionalProperties":false}"#;
const JUDGE: &str = "Answer with exactly one JSON object and nothing else, {\"score\": an integer from 0 to 10, \"issues\": a list of at most five strings, most important first, each one sentence naming what is wrong and where}; do not run commands, edit anything or write files.";
const CRITIC_SCHEMA: &str = r#"{"type":"object","properties":{"score":{"type":"integer","minimum":0,"maximum":10},"issues":{"type":"array","maxItems":5,"items":{"type":"string"}}},"required":["score","issues"],"additionalProperties":false}"#;

#[derive(Deserialize)]
struct Caption {
    prompt: String,
}

fn caption(text: &str) -> Result<String, String> {
    let object = text
        .find('{')
        .and_then(|a| text.get(a..=text.rfind('}')?))
        .ok_or_else(|| format!("no caption: {}", text.trim()))?;
    serde_json::from_str::<Caption>(object)
        .map(|c| c.prompt)
        .map_err(|e| format!("no caption: {e}"))
}

fn relit_key(kept: &[u8], style: &Style) -> String {
    let edges = style.edges.join("\n");
    key(&[kept, style.relight.as_bytes(), edges.as_bytes()])
}

fn judged_key(art: &Art, name: &str, count: u32, style: &Style) -> String {
    let candidates: Vec<Vec<u8>> = (1..=count)
        .map(|i| art.candidate(name, i))
        .filter(|p| p.exists())
        .map(|p| read(&p))
        .collect();
    let mut parts: Vec<&[u8]> = vec![style.critic.as_bytes()];
    parts.extend(candidates.iter().map(Vec::as_slice));
    key(&parts)
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
        "{brief} The current prompt: \"{prompt}\" A critic found these issues with the best picture it produced, most important first: {} Rewrite the prompt so the next picture fixes them.",
        issues.join(" ")
    ))
}

fn best(scored: &[(u32, Score)], keep: impl Fn(&Score) -> bool) -> Option<(u32, &Score)> {
    scored
        .iter()
        .filter(|(_, s)| keep(s))
        .max_by(|a, b| {
            let (a, b) = (a.1.rank(), b.1.rank());
            a.0.cmp(&b.0).then(a.1.total_cmp(&b.1))
        })
        .map(|(i, s)| (*i, s))
}

type Ask<'a> = &'a (dyn Fn(&[PathBuf], &str) -> Result<String, String> + Sync);

fn ask(art: &Art, schema: &str, images: &[PathBuf], prompt: &str) -> Result<String, String> {
    let mut command = std::process::Command::new(art.ask_sh());
    for image in images {
        command.arg("-i").arg(image);
    }
    let output = command
        .arg(schema)
        .arg(prompt)
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| format!("{}: {e}", art.ask_sh().display()))?;
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
        .unwrap_or("ask.sh failed without a word")
        .to_string())
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
            "{candidate}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{}\t{}\t{}",
            self.outside,
            self.seat,
            self.palette,
            self.off_centre,
            self.critic
                .as_ref()
                .map_or("-".to_string(), |j| j.score.to_string()),
            self.verdict(t),
            serde_json::to_string(self.issues()).expect("issues serialise")
        )
    }

    fn parse(row: &str) -> Option<(String, Score)> {
        let cols: Vec<&str> = row.split('\t').collect();
        let [
            candidate,
            outside,
            seat,
            palette,
            off_centre,
            critic,
            _,
            issues,
        ] = cols[..]
        else {
            return None;
        };
        let critic = match critic {
            "-" => None,
            score => Some(Critic {
                score: score.parse().ok()?,
                issues: serde_json::from_str(issues).ok()?,
            }),
        };
        Some((
            candidate.to_string(),
            Score {
                outside: outside.parse().ok()?,
                seat: seat.parse().ok()?,
                palette: palette.parse().ok()?,
                off_centre: off_centre.parse().ok()?,
                critic,
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
    images: Vec<PathBuf>,
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

    fn score(&self, name: &str, scaffold: &Scaffold, count: u32) -> Vec<(u32, Score)> {
        (1..=count)
            .filter(|i| self.art.candidate(name, *i).exists())
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
        count: u32,
        scored: Vec<(u32, Score)>,
    ) -> Result<Vec<(u32, Score)>, String> {
        let dir = self.art.machine(name);
        let judged = judged_key(self.art, name, count, style);
        let trusted = self.entry(name)?.3.judged.as_deref() == Some(judged.as_str());
        let previous: BTreeMap<u32, Score> = std::fs::read_to_string(dir.join("scores.tsv"))
            .unwrap_or_default()
            .lines()
            .filter_map(Score::parse)
            .filter_map(|(label, mut score)| {
                let index: u32 = label.rsplit('-').next()?.parse().ok()?;
                if !trusted {
                    score.critic = None;
                }
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
                        if score.measured(&thresholds).is_some() {
                            return (i, score);
                        }
                        if let Some(critic) = previous.and_then(|p| p.critic.clone()) {
                            score.critic = Some(critic);
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
                        score.critic = match reply {
                            Ok(text) => {
                                let judgement = Critic::parse(&text);
                                if judgement.is_none() {
                                    println!("{label}\tcritic returned no score: {}", text.trim());
                                }
                                judgement
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
        self.record(name, |e| e.judged = Some(judged));
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
        let brief = brief(&style, item(name), &entry.direction);
        let briefed = key(&[brief.as_bytes()]);
        let prompt = stored(&self.art.prompt(name))?
            .filter(|_| entry.briefed.as_deref() == Some(briefed.as_str()));
        let wipe = prompt.as_deref().is_none_or(|prompt| {
            let painted = painted_key(prompt, count, &rendered, item(name).recipe());
            entry.painted.as_deref() != Some(painted.as_str())
        });
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
            brief,
            prompt,
            rendered,
            images,
            changed,
        })
    }

    fn author(
        &self,
        name: &str,
        brief: &str,
        images: &[PathBuf],
        text: &str,
    ) -> Result<String, String> {
        let written = {
            let _permit = self.cap.take();
            (self.director)(images, text)?
        };
        let prompt = store(&self.art.prompt(name), &written)?;
        self.record(name, |e| e.briefed = Some(key(&[brief.as_bytes()])));
        Ok(prompt)
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
            images,
            mut changed,
        } = prepared;
        let dir = self.art.machine(name);
        let candidates = dir.join("candidates");
        let mut kept = entry.kept.filter(|k| self.art.candidate(name, *k).exists());
        let mut rounds = 0;
        let mut issues: Vec<String> = Vec::new();
        let mut prompt = match prompt {
            Some(prompt) => prompt,
            None => {
                changed = true;
                self.author(name, &brief, &images, &brief)?
            }
        };
        let key_of = |prompt: &str| painted_key(prompt, count, &rendered, item(name).recipe());
        let mut wipe = entry.painted.as_deref() != Some(key_of(&prompt).as_str());
        let kept = loop {
            if rounds > 0 && let Some(text) = rebrief(&brief, &prompt, &issues, rounds + 1) {
                prompt = self.author(name, &brief, &images, &text)?;
                changed = true;
            }
            let painted = key_of(&prompt);
            if wipe {
                let _ = std::fs::remove_dir_all(&candidates);
                let _ = std::fs::remove_file(dir.join("scores.tsv"));
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
            rounds += usize::from(asked > 0);
            let failed: Vec<String> = self
                .paint(jobs)
                .into_iter()
                .filter_map(|(label, r)| r.err().map(|e| format!("{label}: {e}")))
                .collect();
            let painted_now = asked > failed.len();
            changed |= painted_now;
            if painted_now || wipe {
                self.record(name, |e| {
                    e.kept = None;
                    e.painted = Some(painted.clone());
                    e.relit = None;
                });
            }
            let only_kept = kept.filter(|_| asked == 0);
            let mut scored = match only_kept {
                Some(k) => {
                    let png = open(self.art.candidate(name, k));
                    vec![(k, scaffold.score(&scaffold.register(&png)))]
                }
                None => self.score(name, &scaffold, count),
            };
            let assess = |scored| self.assess(name, &scaffold, &style, thresholds, count, scored);
            scored = assess(scored)?;
            let keeping = |scored: &[(u32, Score)], i: u32| {
                scored
                    .iter()
                    .any(|(k, s)| *k == i && s.passes(&thresholds) && s.critic.is_some())
            };
            if let Some(k) = kept.filter(|k| keeping(&scored, *k)) {
                break k;
            }
            if only_kept.is_some() {
                scored = assess(self.score(name, &scaffold, count))?;
            }
            if let Some((k, _)) = best(&scored, |s| s.reaches(&thresholds)) {
                break k;
            }
            if asked > 0 && failed.len() == asked {
                return Err(format!(
                    "no candidate passes; {} of {count} paints failed: {}",
                    failed.len(),
                    failed.join(", ")
                ));
            }
            let measured = scored
                .iter()
                .filter(|(_, s)| s.measured(&thresholds).is_none());
            if measured.clone().count() > 0 && measured.clone().all(|(_, s)| s.critic.is_none()) {
                return Err("the critic read no candidate; nothing repainted".to_string());
            }
            issues = best(&scored, |s| s.measured(&thresholds).is_none())
                .map(|(_, s)| s.issues().to_vec())
                .unwrap_or_default();
            if rounds == ROUNDS {
                if let Some((k, _)) = best(&scored, |s| s.passes(&thresholds)) {
                    break k;
                }
                return Err(format!(
                    "no candidate passes the measured rules in {ROUNDS} rounds"
                ));
            }
            wipe = true;
        };
        let relit = relit_key(&read(&self.art.candidate(name, kept)), &style);
        let current = entry.relit.as_deref() == Some(relit.as_str())
            && dir.join("albedo.png").exists()
            && dir.join("normal.png").exists();
        if !current {
            self.relief(name, &scaffold, kept, &style, thresholds)?;
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
                            .critic
                            .as_ref()
                            .map_or("-".to_string(), |c| c.score.to_string()),
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
    fn new(
        art: &'a Art,
        director: Ask<'a>,
        painter: Painter<'a>,
        critic: Ask<'a>,
    ) -> Remake<'a> {
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
    if (!sheet.exists() || remake.redraw.load(std::sync::atomic::Ordering::SeqCst))
        && let Err(e) = remake.sheet()
    {
        eprintln!("sheet: {e}");
    }
    results
}

fn plan() -> String {
    let mut out = String::new();
    for item in Machine::ALL {
        let name = name(item);
        let recipe = match item.recipe() {
            Some(_) => "recipe",
            None => "no recipe",
        };
        out += &format!("{name}\t{recipe}\n");
    }
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
    const USAGE: &str = "usage: ziral --plan | ziral --gen NAME... | ziral --gen --all";
    let art = Art::shipped();
    if args.get(1).map(String::as_str) == Some("--plan") {
        print!("{}", plan());
        return Some(0);
    }
    if args.get(1).map(String::as_str) != Some("--gen") {
        return None;
    }
    let rest: Vec<&str> = args.iter().skip(2).map(String::as_str).collect();
    let known = |n: &str| Machine::ALL.into_iter().map(name).any(|k| k == n);
    let director = |images: &[PathBuf], brief: &str| {
        ask(&art, CAPTION_SCHEMA, images, &format!("{DIRECTOR} {brief}")).and_then(|t| caption(&t))
    };
    let painter = |job: &Paint| paint_sh(&art, job);
    let critic = |images: &[PathBuf], rubric: &str| {
        ask(&art, CRITIC_SCHEMA, images, &format!("{rubric} {JUDGE}"))
    };
    let names: Vec<String> = match rest.as_slice() {
        ["--all"] => Machine::ALL
            .into_iter()
            .map(|i| name(i).to_string())
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
    let split = std::process::Command::new(art.dir.join("rig.sh"))
        .status()
        .is_ok_and(|status| status.success());
    Some(i32::from(!(generated && split)))
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

    #[test]
    fn machine_generator_manifest_round_trip_keeps_every_rig_part() {
        let manifest = Art::shipped().read();
        let text = toml::to_string_pretty(&manifest).unwrap();
        let round: Manifest = toml::from_str(&text).unwrap();
        for machine in Machine::ALL {
            let name = name(machine);
            assert_eq!(
                round.machine[name].parts, manifest.machine[name].parts,
                "{name}"
            );
            assert!(!round.machine[name].parts.is_empty(), "{name}");
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
                    energy: crate::sim::ActivationEnergy::default(),
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
            assert!(
                score.measured(&manifest.thresholds).is_none(),
                "{name}: {score:?}"
            );
            let rows = std::fs::read_to_string(dir.join("scores.tsv")).expect("scores.tsv");
            let judged = rows
                .lines()
                .filter_map(Score::parse)
                .find(|(label, _)| *label == format!("{name}-{kept}"))
                .and_then(|(_, s)| s.critic)
                .unwrap_or_else(|| panic!("{name}-{kept} has no critic score in scores.tsv"));
            assert!(judged.score <= 10, "{name}-{kept}: {judged:?}");
            assert_eq!(
                machine.judged.as_deref(),
                Some(
                    judged_key(&Art::shipped(), name, manifest.candidates, &manifest.style)
                        .as_str()
                ),
                "{name}: the judgement is stale against the candidates or the critic prompt: run ziral --gen {name}"
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
            assert_eq!(
                machine.briefed.as_deref(),
                Some(key(&[brief(&manifest.style, item, &machine.direction).as_bytes()]).as_str()),
                "{name}: the prompt was written from another brief: run ziral --gen {name}"
            );
            assert_eq!(
                machine.painted.as_deref(),
                Some(painted_key(&prompt, manifest.candidates, &want, item.recipe()).as_str()),
                "{name}: the candidates are stale against the prompt: run ziral --gen {name}"
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
                            direction: format!("a {name}"),
                            kept: None,
                            briefed: None,
                            painted: None,
                            relit: None,
                            judged: None,
                            instrument: crate::sound::instrument(item(name)),
                            emitter: crate::rig::entry(item(name)).emitter,
                            rig_emitter: crate::rig::entry(item(name)).rig_emitter,
                            parts: Vec::new(),
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

    fn author(_: &[PathBuf], brief: &str) -> Result<String, String> {
        Ok(brief.to_string())
    }

    fn judge(_: &[PathBuf], _: &str) -> Result<String, String> {
        Ok(r#"{"score": 10, "issues": []}"#.to_string())
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
            let results = remake(&art, &names, &author, &painter, &judge);
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
        m.machine.get_mut("source").expect("source").direction = "another source".into();
        art.write(&m);
        assert_eq!(run(&calls), (true, 2));
        assert_eq!(
            stored(&art.prompt("source")).expect("readable").as_deref(),
            Some(format!("{} another source", art.read().style.shared).as_str())
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
        assert_eq!(run(&calls), (true, 4));
        assert_eq!(art.read().machine["source"].kept, Some(3 - kept));
        assert_ne!(read(&dir.join("albedo.png")), albedo);
        assert_eq!(run(&calls), (false, 0));
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
        assert!(landed(&remake(&art, &names, &author, &painter, &judge)));
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
        assert!(landed(&remake(&art, &names, &author, &painter, &judge)));
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
             mkdir -p \"$HOME/.codex/generated_images/t$n\" \"$HOME/.codex/sessions\"\n\
             p=$(printf %s \"$4\" | sed -n '/^<<<PROMPT$/,/^PROMPT>>>$/p' | sed '1d;$d')\n\
             [ -z \"$REWRITE\" ] || p=\"Image 1 is the reference. $p\"\n\
             jq -nc --arg p \"$p\" '{type:\"response_item\",payload:{type:\"custom_tool_call\",name:\"exec\",input:(\"tools.image_gen__imagegen({prompt:\" + ($p|@json) + \"})\")}}' > \"$HOME/.codex/sessions/rollout-x-t$n.jsonl\"\n\
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
        let paint = |pass_on: u32, rewrite: bool| {
            let _ = std::fs::remove_file(root.join("attempts"));
            let _ = std::fs::remove_file(root.join("out.png"));
            let started = std::time::Instant::now();
            let out = std::process::Command::new(Art::shipped().paint_sh())
                .args(["-s", "64"])
                .arg(root.join("out.png"))
                .arg("a prompt\nwith a second line")
                .env("PATH", &path)
                .env("HOME", &root)
                .env("PASS_ON", pass_on.to_string())
                .env("REWRITE", if rewrite { "1" } else { "" })
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
        let (ok, attempts, stderr, took) = paint(3, false);
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
        let (ok, attempts, stderr, took) = paint(99, false);
        assert!(!ok);
        assert!(!root.join("out.png").exists());
        assert_eq!(attempts, 4);
        assert!(
            stderr.contains("attempt 3 of 4 failed, retrying in 4s"),
            "{stderr}"
        );
        assert!(stderr.contains("gave up after 4 attempts"), "{stderr}");
        assert!(took >= 7.0, "{took}s: no backoff");
        let (ok, attempts, stderr, _) = paint(1, true);
        assert!(!ok);
        assert!(!root.join("out.png").exists());
        assert_eq!(attempts, 4);
        assert!(
            stderr.contains("the image tool received another prompt:\nImage 1 is the reference. a prompt\nwith a second line"),
            "{stderr}"
        );
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
                "```json\n{\"score\": 7, \"issues\": [\"The cup at the left shows its far wall.\"]}\n```"
            ),
            Some(Critic {
                score: 7,
                issues: vec!["The cup at the left shows its far wall.".to_string()],
            })
        );
        assert_eq!(
            parsed(r#"{"score": 10, "issues": []}"#),
            Some(Critic {
                score: 10,
                issues: vec![]
            })
        );
        assert_eq!(parsed(r#"{"score": 11, "issues": []}"#), None);
        assert_eq!(parsed(r#"{"score": 8}"#), None);
        assert_eq!(parsed(r#"{"score": "eight", "issues": []}"#), None);
        assert_eq!(parsed("A fine machine, 9/10."), None);
        assert_eq!(parsed(""), None);
        let six = r#"{"score": 3, "issues": ["a", "b", "c", "d", "e", "f"]}"#;
        assert_eq!(parsed(six).expect("parses").issues.len(), 5);
    }

    #[test]
    fn the_caption_is_the_prompt_of_one_json_object_or_an_error() {
        assert_eq!(
            caption("```json\n{\"prompt\": \"A flat plan.\"}\n```").as_deref(),
            Ok("A flat plan.")
        );
        assert!(caption(r#"{"caption": "A flat plan."}"#).is_err());
        assert!(caption("A flat plan.").is_err());
    }

    #[test]
    fn the_critic_sees_the_board_and_the_scaffold_and_a_malformed_reply_fails_only_that_candidate()
    {
        let art = studio("critic", &["right", "top", "left", "bottom"], &["source"]);
        let seen: std::sync::Mutex<Vec<(Vec<PathBuf>, String)>> = std::sync::Mutex::new(vec![]);
        let critic = |images: &[PathBuf], prompt: &str| {
            seen.lock()
                .unwrap()
                .push((images.to_vec(), prompt.to_string()));
            Ok(match index(images) {
                1 => r#"{"score": 9, "issues": ["Slight glare on the rim."]}"#.to_string(),
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
            rows["source-1"].1.critic,
            Some(Critic {
                score: 9,
                issues: vec!["Slight glare on the rim.".to_string()]
            })
        );
        assert_eq!(rows["source-2"].0, "pass");
        assert_eq!(rows["source-2"].1.critic, None);
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
        let art = studio("bar", &["right", "top", "left", "bottom"], &["source"]);
        let critic = |images: &[PathBuf], _: &str| {
            Ok(match index(images) {
                1 => r#"{"score": 7, "issues": ["The hopper shows its back wall."]}"#,
                _ => r#"{"score": 8, "issues": []}"#,
            }
            .to_string())
        };
        let names = ["source".to_string()];
        assert!(landed(&remake(&art, &names, &author, &fake, &critic)));
        assert_eq!(art.read().machine["source"].kept, Some(2));
        let rows = rows(&art, "source");
        assert_eq!(rows["source-1"].0, "pass");
        assert_eq!(rows["source-2"].0, "pass");
        let art = studio("rank", &["right", "top", "left", "bottom"], &["source"]);
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
                r#"{"score": 10, "issues": []}"#
            } else {
                r#"{"score": 9, "issues": []}"#
            }
            .to_string())
        };
        assert!(landed(&remake(&art, &names, &author, &fake, &critic)));
        assert_eq!(art.read().machine["source"].kept, Some(lower));
    }

    #[test]
    fn the_best_candidates_issues_rebrief_the_author_then_the_last_round_starts_again() {
        let art = studio("revise", &["right", "top", "left", "bottom"], &["source"]);
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
            if job.images[0].ends_with("scaffold.png") {
                prompts.lock().unwrap().push(job.prompt.clone());
            }
            fake(job)
        };
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let critic = |images: &[PathBuf], _: &str| {
            let call = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(if call < 4 {
                match index(images) {
                    1 => r#"{"score": 3, "issues": ["Worse first.", "Worse second."]}"#,
                    _ => r#"{"score": 5, "issues": ["Tipped: the hopper shows its back wall.", "A plate under it.", "Text on the gate."]}"#,
                }
            } else {
                r#"{"score": 10, "issues": []}"#
            }
            .to_string())
        };
        let names = ["source".to_string()];
        let results = remake(&art, &names, &author, &painter, &critic);
        assert!(landed(&results), "{results:?}");
        let style = art.read().style;
        let base = format!("{} {direction}", style.shared);
        let first = format!("a caption from: {}", base.trim());
        let briefs = briefs.lock().unwrap();
        let scaffold = art.machine("source").join("scaffold.png");
        assert_eq!(briefs.len(), 3, "{briefs:?}");
        assert!(briefs.iter().all(|(images, _)| *images == [scaffold.clone()]));
        assert_eq!(briefs[0].1, base);
        assert_eq!(
            briefs[1].1,
            format!(
                "{base} The current prompt: \"{first}\" A critic found these issues with the best picture it produced, most important first: Tipped: the hopper shows its back wall. A plate under it. Text on the gate. Rewrite the prompt so the next picture fixes them."
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
        assert_eq!(&prompts[..2], &[first.clone(), first]);
        let second = format!("a caption from: {}", briefs[1].1.trim());
        assert_eq!(&prompts[2..4], &[second.clone(), second]);
        let third = format!("a caption from: {}", briefs[2].1.trim());
        assert_eq!(&prompts[4..], &[third.clone(), third.clone()]);
        assert_eq!(
            stored(&art.prompt("source")).expect("readable").as_deref(),
            Some(third.as_str())
        );
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 6);
        assert_eq!(art.read().machine["source"].direction, direction);
        assert!(art.read().machine["source"].kept.is_some());
    }

    #[test]
    fn three_rounds_then_keep_the_best_with_its_score_and_issues() {
        let art = studio("rounds", &["right", "top", "left", "bottom"], &["source"]);
        let paints = std::sync::atomic::AtomicUsize::new(0);
        let painter = counted(&paints, &fake);
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let critic = |images: &[PathBuf], _: &str| {
            let call = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(match index(images) {
                1 => format!(
                    r#"{{"score": 2, "issues": ["Round {} low.", "Still tipped."]}}"#,
                    call / 2 + 1
                ),
                _ => format!(
                    r#"{{"score": 4, "issues": ["Round {} best.", "Still a plate."]}}"#,
                    call / 2 + 1
                ),
            })
        };
        let names = ["source".to_string()];
        let results = remake(&art, &names, &author, &painter, &critic);
        assert!(landed(&results), "{results:?}");
        assert_eq!(paints.load(std::sync::atomic::Ordering::SeqCst), 10);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 6);
        assert_eq!(art.read().machine["source"].kept, Some(2));
        assert!(art.machine("source").join("albedo.png").exists());
        let rows = rows(&art, "source");
        assert_eq!(rows["source-1"].0, "pass");
        assert_eq!(rows["source-2"].0, "pass");
        assert_eq!(rows["source-2"].1.critic.as_ref().unwrap().score, 4);
        assert_eq!(
            rows["source-2"].1.issues(),
            ["Round 3 best.", "Still a plate."]
        );
        let results = remake(&art, &names, &author, &painter, &critic);
        assert!(landed(&results), "{results:?}");
        assert_eq!(paints.load(std::sync::atomic::Ordering::SeqCst), 10);
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 6);
    }

    #[test]
    fn an_unchanged_candidate_is_never_judged_again_and_a_changed_critic_prompt_is_judged_anew() {
        let art = studio("judged", &["right", "top", "left", "bottom"], &["source"]);
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
        let judged = art.read().machine["source"].judged.clone();
        assert!(judged.is_some());
        assert_eq!(run(), 0);
        assert_eq!(art.read().machine["source"].judged, judged);
        let mut m = art.read();
        m.style.critic += " Be harsher.";
        art.write(&m);
        assert_eq!(run(), 1);
        assert_ne!(art.read().machine["source"].judged, judged);
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
        let art = studio("outage", &["right", "top", "left", "bottom"], &["source"]);
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
}
