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
const SPHERE: f32 = 0.75;
const SPHERE_ALBEDO: f32 = 0.6;
const SPHERE_LIT: f32 = 0.15;
const SPHERE_CAP: f32 = 0.5;
const SAMPLES: usize = 4;

#[derive(Serialize, Deserialize)]
struct Manifest {
    candidates: u32,
    style: Style,
    thresholds: Thresholds,
    machine: BTreeMap<String, Machine>,
}

#[derive(Serialize, Deserialize)]
struct Style {
    shared: String,
    edges: Vec<String>,
    relight: String,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
struct Thresholds {
    outside: f32,
    seat: f32,
    palette: f32,
    sphere: f32,
}

#[derive(Serialize, Deserialize)]
struct Machine {
    prompt: String,
    kept: Option<u32>,
}

fn manifest_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("art/machines/manifest.toml")
}

impl Manifest {
    fn read() -> Manifest {
        let path = manifest_path();
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        toml::from_str(&text).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
    }

    fn write(&self) {
        let text = toml::to_string_pretty(self).expect("a manifest serialises");
        let path = manifest_path();
        let part = path.with_extension("toml.part");
        std::fs::write(&part, text).expect("the manifest is writable");
        std::fs::rename(&part, &path).expect("the manifest is replaceable");
    }

    fn machine(&self, name: &str) -> &Machine {
        self.machine
            .get(name)
            .unwrap_or_else(|| panic!("manifest has no [machine.{name}]"))
    }

    fn machine_mut(&mut self, name: &str) -> &mut Machine {
        self.machine
            .get_mut(name)
            .unwrap_or_else(|| panic!("manifest has no [machine.{name}]"))
    }
}

fn name(item: Item) -> &'static str {
    look::machine(item)
        .skin
        .name
        .split('/')
        .nth(1)
        .expect("a machine skin lives in art/machines/<name>/")
}

fn item(name: &str) -> Item {
    PALETTE
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

const fn band() -> f32 {
    BAND * HEX * scale()
}

impl Scaffold {
    fn of(item: Item) -> Scaffold {
        let cells = look::footprint(item);
        let quad = look::quad(item);
        let canvas = (quad.side * scale() + 2.0 * band()).round() as u32;
        let mut scaffold = Scaffold {
            cells,
            quad,
            canvas,
            mask: Vec::new(),
        };
        scaffold.mask = scaffold.mask();
        scaffold
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

    fn render(&self) -> RgbaImage {
        RgbaImage::from_fn(self.canvas, self.canvas, |x, y| {
            self.paint(self.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5)))
        })
    }

    fn cut(&self, candidate: &RgbaImage) -> RgbaImage {
        let (origin, side) = self.crop();
        let n = self.canvas as usize;
        RgbaImage::from_fn(side, side, |x, y| {
            let (cx, cy) = (x + origin, y + origin);
            let Rgba([r, g, b, _]) = *candidate.get_pixel(cx, cy);
            let alpha = self.mask[cy as usize * n + cx as usize];
            Rgba([r, g, b, (alpha * 255.0).round() as u8])
        })
    }

    fn score(&self, candidate: &RgbaImage) -> Score {
        assert_eq!(
            (candidate.width(), candidate.height()),
            (self.canvas, self.canvas),
            "a candidate is painted over the whole scaffold"
        );
        let (origin, side) = self.crop();
        let n = self.canvas as usize;
        let clay = Glaze::Clay.rgb();
        let mut outside = Mean::default();
        let mut inside = Mean::default();
        for y in origin..origin + side {
            for x in origin..origin + side {
                let c = rgb(candidate.get_pixel(x, y));
                let m = self.mask[y as usize * n + x as usize];
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
        let (centre, radius) = self.sphere();
        ball(centre, radius, x, y)
    }

    fn master(&self, candidate: &RgbaImage) -> RgbaImage {
        let mut out = candidate.clone();
        for (x, y, p) in out.enumerate_pixels_mut() {
            if self.sphere_normal(x, y).is_some() {
                *p = grey(SPHERE_ALBEDO);
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
            if self.mask[i] == 1.0 {
                mean += *out;
            }
        }
        let sphere = self.sphere_error(&normals);
        let flatten = rotation_to_z(mean.normalize_or(Vec3::Z));
        let (origin, side) = self.crop();
        let normal = RgbaImage::from_fn(side, side, |x, y| {
            let i = (y + origin) as usize * n + (x + origin) as usize;
            let v = if self.mask[i] > 0.0 {
                flatten * normals[i]
            } else {
                Vec3::Z
            };
            encode(v.normalize_or(Vec3::Z))
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

fn encode(v: Vec3) -> Rgba<u8> {
    rgba(
        [(v.x + 1.0) / 2.0, (v.y + 1.0) / 2.0, (v.z + 1.0) / 2.0],
        1.0,
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
}

impl Score {
    fn passes(&self, t: &Thresholds) -> bool {
        self.outside <= t.outside && self.seat >= t.seat && self.palette <= t.palette
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

fn in_hex(d: Vec2, radius: f32) -> bool {
    let (dx, dy) = (d.x.abs(), d.y.abs());
    dx <= radius * 3f32.sqrt() / 2.0 && dy <= radius - dx / 3f32.sqrt()
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

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|i| a[i] + (b[i] - a[i]) * t)
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
            let scaffold = Scaffold::of(item(name));
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
            let scaffold = Scaffold::of(item(name));
            let dir = dir(name);
            let candidate = open(dir.join(format!("candidates/{name}-{index}.png")));
            let mut manifest = Manifest::read();
            manifest.machine_mut(name).kept = Some(index);
            manifest.write();
            let _ = std::fs::remove_file(dir.join("normal.png"));
            let _ = std::fs::remove_dir_all(dir.join("relit"));
            std::fs::create_dir_all(dir.join("relit")).expect("the relit dir is creatable");
            save(&scaffold.cut(&candidate), &dir.join("albedo.png"));
            save(&scaffold.master(&candidate), &dir.join("relit/master.png"));
        }
        (Some("--normals"), [name, pngs @ ..]) => {
            let thresholds = Manifest::read().thresholds;
            let scaffold = Scaffold::of(item(name));
            let master = open(dir(name).join("relit/master.png"));
            let edits: Vec<RgbaImage> = pngs.iter().map(open).collect();
            let relief = scaffold.normals(&master, &edits);
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
            manifest.candidates,
            machine.kept.map_or("-".to_string(), |k| k.to_string()),
            manifest.style.shared,
            machine.prompt
        );
    }
    for edge in &manifest.style.edges {
        println!(
            "relight\t{edge}\t{}",
            manifest.style.relight.replace("{edge}", edge)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::{AMBIENT, light};
    use crate::sim::{Arm, Glyph, Hex, ORIGIN};

    const BUMP: f32 = 0.3;
    const RELIEF: f32 = 0.1;
    const TILT: f32 = 0.15;
    const ASPECT: f32 = 0.02;

    fn shade(n: Vec3) -> f32 {
        (AMBIENT + (1.0 - AMBIENT) * n.dot(light()).max(0.0))
            / (AMBIENT + (1.0 - AMBIENT) * light().z)
    }

    #[test]
    fn scaffold_cells_match_the_footprint() {
        for item in PALETTE {
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
    fn every_machine_has_a_manifest_entry_a_scaffold_a_kept_candidate_that_passes_and_relief() {
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
            let shipped = open(dir.join("scaffold.png"));
            assert!(
                want.as_raw() == shipped.as_raw(),
                "{name}/scaffold.png is stale: regenerate it"
            );
            let candidate = open(dir.join(format!("candidates/{name}-{kept}.png")));
            let score = scaffold.score(&candidate);
            assert!(score.passes(&manifest.thresholds), "{name}: {score:?}");
            let (_, side) = scaffold.crop();
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
    fn a_lambertian_sphere_returns_its_own_lights_and_a_bump_its_own_normals() {
        let scaffold = Scaffold::of(Item::Glyph(crate::sim::GlyphKind::Source));
        let base = scaffold.render();
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
        let relief = scaffold.normals(&scaffold.master(&base), &edits);
        for (got, want) in relief.lights.iter().zip(lights) {
            assert!(got.direction.dot(want) > 0.999, "{got:?} vs {want:?}");
            assert!((got.ambient - AMBIENT).abs() < 0.02, "{got:?}");
        }
        let thresholds = Manifest::read().thresholds;
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
}
