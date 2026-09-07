use crate::look::{self, Cell, Glaze, HEX, Quad, Role, px};
use crate::sim::Slot;
use crate::{Item, PALETTE};
use bevy::math::{Vec2, Vec3};
use image::{Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const PX_PER_HEX: f32 = 256.0;
const BAND: f32 = 0.5;
const WELL: f32 = 0.95;
const SEAT: f32 = 0.4;
const SEAT_RING: f32 = 0.32;
const SEAT_DOT: f32 = 0.12;
const SEAT_AROUND: [f32; 2] = [0.45, 0.65];
const SPHERE_R: f32 = 48.0;
const SPHERE_ALBEDO: f32 = 0.6;
const SPHERE_LIT: f32 = 0.15;
const SPHERE_CAP: f32 = 0.5;
const SAMPLES: usize = 4;

#[derive(Serialize, Deserialize)]
pub struct Manifest {
    pub style: Style,
    pub thresholds: Thresholds,
    pub machine: BTreeMap<String, Machine>,
}

#[derive(Serialize, Deserialize)]
pub struct Style {
    pub shared: String,
    pub relights: Vec<String>,
    pub relight: String,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct Thresholds {
    pub candidates: u32,
    pub outside: f32,
    pub seat: f32,
    pub palette: f32,
    pub sphere: f32,
}

#[derive(Serialize, Deserialize)]
pub struct Machine {
    pub prompt: String,
    pub kept: Option<u32>,
}

fn manifest_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("art/machines/manifest.toml")
}

impl Manifest {
    pub fn read() -> Manifest {
        let path = manifest_path();
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        toml::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    pub fn write(&self) {
        let text = toml::to_string_pretty(self).expect("a manifest serialises");
        let path = manifest_path();
        let part = path.with_extension("toml.part");
        std::fs::write(&part, text).expect("the manifest is writable");
        std::fs::rename(&part, &path).expect("the manifest is replaceable");
    }

    pub fn machine(&self, name: &str) -> &Machine {
        self.machine
            .get(name)
            .unwrap_or_else(|| panic!("manifest has no [machine.{name}]"))
    }
}

pub fn name(item: Item) -> &'static str {
    look::machine(item)
        .skin
        .name
        .split('/')
        .nth(1)
        .expect("a machine skin lives in art/machines/<name>/")
}

pub fn item(name: &str) -> Item {
    PALETTE
        .into_iter()
        .find(|item| self::name(*item) == name)
        .unwrap_or_else(|| panic!("no machine is named {name}"))
}

#[derive(Debug)]
pub struct Scaffold {
    pub cells: Vec<Cell>,
    pub quad: Quad,
    pub canvas: u32,
}

const fn scale() -> f32 {
    PX_PER_HEX / HEX
}

impl Scaffold {
    pub fn of(item: Item) -> Scaffold {
        let cells = look::footprint(item);
        let quad = look::quad(&cells);
        Scaffold {
            cells,
            quad,
            canvas: ((quad.side + 2.0 * BAND * HEX) * scale()).round() as u32,
        }
    }

    #[cfg(test)]
    fn pixel(&self, world: Vec2) -> Vec2 {
        let half = self.canvas as f32 / 2.0;
        let d = (world - self.quad.centre) * scale();
        Vec2::new(half + d.x, half - d.y)
    }

    fn world(&self, pixel: Vec2) -> Vec2 {
        let half = self.canvas as f32 / 2.0;
        self.quad.centre + Vec2::new(pixel.x - half, half - pixel.y) / scale()
    }

    pub fn crop(&self) -> (u32, u32) {
        let side = (self.quad.side * scale()).round() as u32;
        let band = self.canvas - side;
        assert_eq!(band, 2 * (BAND * HEX * scale()) as u32);
        (band / 2, side)
    }

    fn sphere(&self) -> Vec2 {
        let inset = BAND * HEX * scale() / 2.0;
        Vec2::splat(self.canvas as f32 - inset)
    }

    fn in_cell(&self, world: Vec2, radius: f32) -> Option<&Cell> {
        self.cells.iter().find(|c| in_hex(world - px(c.at), radius))
    }

    fn covered(&self, world: Vec2) -> bool {
        self.in_cell(world, HEX).is_some()
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
        match cell.role {
            Role::Seat(Slot {
                consumed: false, ..
            }) => ring.then_some(Glaze::BlueGreen),
            Role::Seat(Slot { consumed: true, .. }) => {
                (ring || r <= SEAT_DOT).then_some(Glaze::Terracotta)
            }
            Role::Pivot => (r <= SEAT).then_some(Glaze::Brass),
            Role::Hand => {
                let pivot = self
                    .cells
                    .iter()
                    .find(|c| c.role == Role::Pivot)
                    .map(|c| px(c.at))
                    .expect("a hand has its pivot");
                let open = d.angle_to(px(cell.at) - pivot).abs() < std::f32::consts::FRAC_PI_4;
                (ring && !open).then_some(Glaze::Terracotta)
            }
        }
    }

    fn paint(&self, world: Vec2) -> Rgba<u8> {
        let Some(cell) = self.in_cell(world, WELL * HEX) else {
            return rgba(Glaze::Clay.rgb(), 1.0);
        };
        match self.mark(cell, world) {
            Some(glaze) => rgba(glaze.rgb(), 1.0),
            None => rgba(mix(Glaze::Brass.rgb(), Glaze::Clay.rgb(), 0.85), 1.0),
        }
    }

    pub fn render(&self) -> RgbaImage {
        RgbaImage::from_fn(self.canvas, self.canvas, |x, y| {
            self.paint(self.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5)))
        })
    }

    pub fn cut(&self, candidate: &RgbaImage) -> RgbaImage {
        let mask = self.mask();
        let (origin, side) = self.crop();
        let n = self.canvas as usize;
        RgbaImage::from_fn(side, side, |x, y| {
            let (cx, cy) = (x + origin, y + origin);
            let Rgba([r, g, b, _]) = *candidate.get_pixel(cx, cy);
            let alpha = mask[cy as usize * n + cx as usize];
            Rgba([r, g, b, (alpha * 255.0).round() as u8])
        })
    }

    pub fn score(&self, candidate: &RgbaImage) -> Score {
        assert_eq!(
            (candidate.width(), candidate.height()),
            (self.canvas, self.canvas),
            "a candidate is painted over the whole scaffold"
        );
        let mask = self.mask();
        let (origin, side) = self.crop();
        let n = self.canvas as usize;
        let clay = Glaze::Clay.rgb();
        let mut outside = Mean::default();
        let mut inside = Mean::default();
        for y in origin..origin + side {
            for x in origin..origin + side {
                let c = rgb(candidate.get_pixel(x, y));
                let m = mask[y as usize * n + x as usize];
                if m == 0.0 {
                    outside.add(apart(c, clay));
                } else if m == 1.0 {
                    inside.add_rgb(c);
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
                    if self.mark(cell, world).is_some() {
                        mark.add_rgb(rgb(p));
                    } else if r <= SEAT_DOT {
                        centre.add_rgb(rgb(p));
                    } else if (SEAT_AROUND[0]..=SEAT_AROUND[1]).contains(&r) {
                        around.add_rgb(rgb(p));
                    }
                }
                apart(mark.rgb(), around.rgb()).max(apart(centre.rgb(), around.rgb()))
            })
            .fold(f32::INFINITY, f32::min);
        let mean = inside.rgb();
        let palette = Glaze::ALL
            .iter()
            .map(|g| apart(mean, g.rgb()))
            .fold(f32::INFINITY, f32::min);
        Score {
            outside: outside.value(),
            seat,
            palette,
        }
    }

    fn sphere_normal(&self, x: u32, y: u32) -> Option<Vec3> {
        let c = self.sphere();
        let d = (Vec2::new(x as f32 + 0.5, y as f32 + 0.5) - c) / SPHERE_R;
        let d = Vec2::new(d.x, -d.y);
        let rr = d.length_squared();
        (rr < 1.0).then(|| Vec3::new(d.x, d.y, (1.0 - rr).sqrt()))
    }

    pub fn master(&self, candidate: &RgbaImage) -> RgbaImage {
        let mut out = candidate.clone();
        for (x, y, p) in out.enumerate_pixels_mut() {
            if self.sphere_normal(x, y).is_some() {
                *p = rgba([SPHERE_ALBEDO; 3], 1.0);
            }
        }
        out
    }

    fn light(&self, edit: &RgbaImage) -> Light {
        let pixels: Vec<(Vec3, f32)> = edit
            .enumerate_pixels()
            .filter_map(|(x, y, p)| {
                let n = self.sphere_normal(x, y)?;
                let c = rgb(p);
                Some((n, (c[0] + c[1] + c[2]) / 3.0))
            })
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

    pub fn normals(&self, master: &RgbaImage, edits: &[RgbaImage]) -> Relief {
        for edit in std::iter::once(master).chain(edits) {
            assert_eq!(
                (edit.width(), edit.height()),
                (self.canvas, self.canvas),
                "a relit edit covers the whole master"
            );
        }
        assert!(edits.len() >= 2, "two relit edits span the tangent plane");
        let lights: Vec<Light> = edits.iter().map(|e| self.light(e)).collect();
        let edits: Vec<&RgbaImage> = std::iter::once(master).chain(edits).collect();
        let rows: Vec<[f32; 3]> = std::iter::once([0.0, 0.0, 1.0])
            .chain(lights.iter().map(Light::row))
            .collect();
        let solve = invert(gram(rows.iter().map(|r| (*r, 0.0))).0);
        let mask = self.mask();
        let n = self.canvas as usize;
        let mut normals = vec![Vec3::Z; n * n];
        let mut mean = Vec3::ZERO;
        for (i, out) in normals.iter_mut().enumerate() {
            let (x, y) = ((i % n) as u32, (i / n) as u32);
            let mut rhs = [0f64; 3];
            for (e, row) in edits.iter().zip(&rows) {
                let c = rgb(e.get_pixel(x, y));
                let lum = f64::from((c[0] + c[1] + c[2]) / 3.0);
                for (r, a) in rhs.iter_mut().zip(row) {
                    *r += f64::from(*a) * lum;
                }
            }
            let [gx, gy, rho] = apply(&solve, &rhs);
            let tangent = if rho > 0.0 {
                (Vec2::new(gx as f32, gy as f32) / rho as f32).clamp_length_max(1.0)
            } else {
                Vec2::ZERO
            };
            *out = tangent.extend((1.0 - tangent.length_squared()).max(0.0).sqrt());
            if mask[i] == 1.0 {
                mean += *out;
            }
        }
        let sphere = self.sphere_error(&normals);
        let flatten = rotation_to_z(mean.normalize_or(Vec3::Z));
        let (origin, side) = self.crop();
        let normal = RgbaImage::from_fn(side, side, |x, y| {
            let i = (y + origin) as usize * n + (x + origin) as usize;
            let v = if mask[i] > 0.0 {
                flatten * normals[i]
            } else {
                Vec3::Z
            };
            let v = v.normalize_or(Vec3::Z);
            rgba(
                [(v.x + 1.0) / 2.0, (v.y + 1.0) / 2.0, (v.z + 1.0) / 2.0],
                1.0,
            )
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

pub struct Relief {
    pub normal: RgbaImage,
    pub lights: Vec<Light>,
    pub sphere: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Light {
    pub direction: Vec3,
    pub ambient: f32,
}

impl Light {
    fn row(&self) -> [f32; 3] {
        let d = self.direction * (1.0 - self.ambient);
        [d.x, d.y, self.ambient]
    }
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
pub struct Score {
    pub outside: f32,
    pub seat: f32,
    pub palette: f32,
}

impl Score {
    pub fn passes(&self, t: &Thresholds) -> bool {
        self.outside <= t.outside && self.seat >= t.seat && self.palette <= t.palette
    }

    pub fn total(&self) -> f32 {
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
        self.sum.map(|s| s / self.n.max(1.0))
    }

    fn value(&self) -> f32 {
        self.rgb()[0]
    }
}

fn in_hex(d: Vec2, radius: f32) -> bool {
    let (dx, dy) = (d.x.abs(), d.y.abs());
    dx <= radius * 3f32.sqrt() / 2.0 && dy <= radius - dx / 3f32.sqrt()
}

fn rgb(p: &Rgba<u8>) -> [f32; 3] {
    [p[0], p[1], p[2]].map(|c| f32::from(c) / 255.0)
}

fn rgba(c: [f32; 3], alpha: f32) -> Rgba<u8> {
    let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Rgba([q(c[0]), q(c[1]), q(c[2]), q(alpha)])
}

fn apart(a: [f32; 3], b: [f32; 3]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
}

fn open(path: &str) -> RgbaImage {
    image::open(path)
        .unwrap_or_else(|e| panic!("{path}: {e}"))
        .into_rgba8()
}

fn save(image: &RgbaImage, path: &Path) {
    image
        .save(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

fn dir(name: &str) -> PathBuf {
    manifest_path().with_file_name(name)
}

pub fn configure(args: &[String]) -> bool {
    const USAGE: &str = "usage: ziral --plan | ziral --scaffold NAME | ziral --score NAME PNG... | ziral --keep NAME INDEX | ziral --normals NAME PNG...";
    let rest: Vec<&str> = args.iter().skip(2).map(String::as_str).collect();
    match (args.get(1).map(String::as_str), rest.as_slice()) {
        (Some("--plan"), []) => plan(),
        (Some("--scaffold"), [name]) => {
            let scaffold = Scaffold::of(item(name));
            let dir = dir(name);
            std::fs::create_dir_all(&dir).expect("the machine dir is creatable");
            save(&scaffold.render(), &dir.join("scaffold.png"));
            println!("{}", scaffold.canvas);
        }
        (Some("--score"), [name, pngs @ ..]) => {
            let item = item(name);
            let scaffold = Scaffold::of(item);
            let thresholds = Manifest::read().thresholds;
            for png in pngs {
                let score = scaffold.score(&open(png));
                println!(
                    "{png}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{}",
                    score.outside,
                    score.seat,
                    score.palette,
                    score.total(),
                    if score.passes(&thresholds) {
                        "pass"
                    } else {
                        "fail"
                    }
                );
            }
        }
        (Some("--keep"), [name, index]) => {
            let index: u32 = index.parse().expect(USAGE);
            let item = item(name);
            let scaffold = Scaffold::of(item);
            let dir = dir(name);
            let candidate = open(
                dir.join(format!("candidates/{name}-{index}.png"))
                    .to_str()
                    .expect("utf-8 paths"),
            );
            let mut manifest = Manifest::read();
            manifest
                .machine
                .get_mut(*name)
                .unwrap_or_else(|| panic!("manifest has no [machine.{name}]"))
                .kept = Some(index);
            manifest.write();
            save(&scaffold.cut(&candidate), &dir.join("albedo.png"));
            std::fs::create_dir_all(dir.join("relit")).expect("the relit dir is creatable");
            save(&scaffold.master(&candidate), &dir.join("relit/master.png"));
        }
        (Some("--normals"), [name, pngs @ ..]) => {
            let item = item(name);
            let scaffold = Scaffold::of(item);
            let master = open(
                dir(name)
                    .join("relit/master.png")
                    .to_str()
                    .expect("utf-8 paths"),
            );
            let edits: Vec<RgbaImage> = pngs.iter().map(|p| open(p)).collect();
            let relief = scaffold.normals(&master, &edits);
            let thresholds = Manifest::read().thresholds;
            for (png, light) in pngs.iter().zip(&relief.lights) {
                let d = light.direction;
                println!(
                    "{png}\tlight=({:+.2},{:+.2},{:+.2})\tambient={:.2}",
                    d.x, d.y, d.z, light.ambient
                );
            }
            println!("sphere error {:.1} degrees", relief.sphere);
            assert!(
                relief.sphere <= thresholds.sphere,
                "the calibration sphere came back {:.1} degrees off, over {}",
                relief.sphere,
                thresholds.sphere
            );
            save(&relief.normal, &dir(name).join("normal.png"));
        }
        (Some("--plan" | "--scaffold" | "--score" | "--keep" | "--normals"), _) => {
            panic!("{USAGE}")
        }
        _ => return false,
    }
    true
}

fn plan() {
    let manifest = Manifest::read();
    for item in PALETTE {
        let name = name(item);
        let machine = manifest.machine(name);
        println!(
            "machine\t{name}\t{}\t{}\t{} {}",
            manifest.thresholds.candidates,
            machine.kept.map_or("-".to_string(), |k| k.to_string()),
            manifest.style.shared,
            machine.prompt
        );
    }
    for from in &manifest.style.relights {
        println!(
            "relight\t{from}\t{}",
            manifest.style.relight.replace("{from}", from)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::{AMBIENT, light};
    use crate::sim::{Arm, Glyph, Hex, ORIGIN};

    fn shade(n: Vec3) -> f32 {
        (AMBIENT + (1.0 - AMBIENT) * n.dot(light()).max(0.0))
            / (AMBIENT + (1.0 - AMBIENT) * light().z)
    }

    #[test]
    fn scaffold_cells_match_the_footprint() {
        for item in PALETTE {
            let scaffold = Scaffold::of(item);
            let mask = scaffold.mask();
            let n = scaffold.canvas as usize;
            let mut covered = Vec::new();
            for q in -4..=4 {
                for r in -4..=4 {
                    let h = Hex::new(q, r);
                    let p = scaffold.pixel(px(h));
                    let inside = p.x >= 0.0 && p.y >= 0.0 && p.x < n as f32 && p.y < n as f32;
                    if inside && mask[p.y as usize * n + p.x as usize] == 1.0 {
                        covered.push(h);
                    }
                }
            }
            let mut footprint: Vec<Hex> = match item {
                Item::Arm => Arm::new(ORIGIN, 0, Vec::new()).cells().to_vec(),
                Item::Glyph(kind) => Glyph {
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
    fn every_machine_has_a_manifest_entry_a_scaffold_and_a_kept_candidate_that_passes() {
        let manifest = Manifest::read();
        let names: Vec<&str> = PALETTE.into_iter().map(name).collect();
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
        for item in PALETTE {
            let name = name(item);
            let machine = manifest.machine(name);
            let kept = machine
                .kept
                .unwrap_or_else(|| panic!("{name} keeps no candidate"));
            let dir = dir(name);
            let scaffold = Scaffold::of(item);
            let want = scaffold.render();
            let shipped = open(dir.join("scaffold.png").to_str().unwrap());
            assert!(
                want.as_raw() == shipped.as_raw(),
                "{name}/scaffold.png is stale: regenerate it"
            );
            let candidate = open(
                dir.join(format!("candidates/{name}-{kept}.png"))
                    .to_str()
                    .unwrap(),
            );
            let score = scaffold.score(&candidate);
            assert!(score.passes(&manifest.thresholds), "{name}: {score:?}");
            let (_, side) = scaffold.crop();
            for map in ["albedo", "normal"] {
                let png = open(dir.join(format!("{map}.png")).to_str().unwrap());
                assert_eq!(
                    (png.width(), png.height()),
                    (side, side),
                    "{name}/{map}.png"
                );
            }
        }
    }

    #[test]
    fn a_candidate_painted_outside_the_mask_is_rejected() {
        let item = Item::Glyph(crate::sim::GlyphKind::Bonder);
        let scaffold = Scaffold::of(item);
        let thresholds = Manifest::read().thresholds;
        let clean = scaffold.render();
        assert!(scaffold.score(&clean).passes(&thresholds));
        let mut spilled = clean.clone();
        let (origin, side) = scaffold.crop();
        let brass = rgba(Glaze::Brass.rgb(), 1.0);
        for y in origin..origin + side {
            for x in origin..origin + side / 6 {
                spilled.put_pixel(x, y, brass);
            }
        }
        let score = scaffold.score(&spilled);
        assert!(!score.passes(&thresholds), "{score:?}");
        assert!(score.outside > thresholds.outside, "{score:?}");
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
    fn a_lambertian_sphere_returns_its_own_lights_and_normals() {
        let scaffold = Scaffold::of(Item::Glyph(crate::sim::GlyphKind::Source));
        let base = scaffold.render();
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
                    let n = scaffold.sphere_normal(x, y).unwrap_or(Vec3::Z);
                    let lum = SPHERE_ALBEDO * (AMBIENT + (1.0 - AMBIENT) * n.dot(*l).max(0.0));
                    *p = rgba([lum; 3], 1.0);
                }
                e
            })
            .collect();
        let relief = scaffold.normals(&scaffold.master(&base), &edits);
        for (got, want) in relief.lights.iter().zip(lights) {
            assert!(got.direction.dot(want) > 0.999, "{got:?} vs {want:?}");
            assert!((got.ambient - AMBIENT).abs() < 0.02, "{got:?}");
        }
        let thresholds = Manifest::read().thresholds;
        assert!(relief.sphere <= thresholds.sphere, "{}", relief.sphere);
    }
}
