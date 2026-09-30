mod ming;

use crate::look::{self, Cell, Glaze, HEX, Quad, Role, px};
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
#[derive(Serialize, Deserialize)]
struct Manifest {
    attempts: u32,
    thresholds: Thresholds,
    machine: BTreeMap<String, Entry>,
}

#[derive(Serialize, Deserialize, Clone, Copy)]
struct Thresholds {
    seat: f32,
    off_centre: f32,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct Entry {
    #[serde(default = "default_generator")]
    generator: String,
    kept: Option<u32>,
    painted: Option<String>,
    relief: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    motion: Option<crate::rig::Motion>,
    instrument: crate::sound::Instrument,
    emitter: crate::particles::Emitter,
    #[serde(skip_serializing_if = "Option::is_none")]
    rig_emitter: Option<crate::particles::Emitter>,
    #[serde(default)]
    parts: Vec<crate::rig::Part>,
}

fn default_generator() -> String {
    "ming".to_string()
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
        Scaffold {
            cells: look::footprint(item),
            quad: look::quad(item),
            canvas: canvas(item),
        }
    }

    fn mount(&self, image: &RgbaImage) -> RgbaImage {
        let (origin, side) = self.crop();
        assert_eq!((image.width(), image.height()), (side, side));
        RgbaImage::from_fn(self.canvas, self.canvas, |x, y| {
            if (origin..origin + side).contains(&x) && (origin..origin + side).contains(&y) {
                *image.get_pixel(x - origin, y - origin)
            } else {
                Rgba([0; 4])
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

    fn register(&self, candidate: &RgbaImage) -> Capture {
        let image = self.fit(candidate);
        let off_centre = self.seat_offset(&image);
        Capture { image, off_centre }
    }

    fn seat_offset(&self, image: &RgbaImage) -> f32 {
        self.marked()
            .map(|cell| {
                let ring = self
                    .seat(image, cell)
                    .map_or(f32::INFINITY, |s| s.distance(px(cell.at)) / HEX);
                let [x0, y0, x1, y1] = self.seat_bounds(cell);
                let mut sum = Vec2::ZERO;
                let mut count = 0.0;
                for y in y0..y1 {
                    for x in x0..x1 {
                        let w = self.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
                        if w.distance(px(cell.at)) < SEAT_AROUND[1] * HEX
                            && image.get_pixel(x, y)[3] == 0
                        {
                            sum += w;
                            count += 1.0;
                        }
                    }
                }
                let aperture = if count > 0.0 {
                    (sum / count).distance(px(cell.at)) / HEX
                } else {
                    f32::INFINITY
                };
                ring.max(aperture)
            })
            .fold(0.0, f32::max)
    }

    fn fit(&self, candidate: &RgbaImage) -> RgbaImage {
        let small =
            image::imageops::resize(candidate, 128, 128, image::imageops::FilterType::Triangle);
        let mut colors: Vec<_> = small.pixels().filter(|p| p[3] > 192).map(rgb).collect();
        if colors.is_empty() {
            colors.push([0.42, 0.31, 0.23]);
        }
        colors.sort_by(|a, b| {
            let chroma = |c: &[f32; 3]| {
                c.iter().copied().fold(0.0, f32::max) - c.iter().copied().fold(1.0, f32::min)
            };
            chroma(a).total_cmp(&chroma(b))
        });
        let mut material = Mean::default();
        for c in &colors[colors.len() / 2..] {
            material.add_rgb(*c);
        }
        let detail =
            image::imageops::resize(candidate, 512, 512, image::imageops::FilterType::Triangle);
        let mut patch = (0, 0);
        let mut best = f32::NEG_INFINITY;
        for y in (0..448).step_by(16) {
            for x in (0..448).step_by(16) {
                let mut mean = Mean::default();
                let mut square = 0.0;
                let mut opaque = true;
                for py in (y..y + 64).step_by(4) {
                    for px in (x..x + 64).step_by(4) {
                        let p = detail.get_pixel(px, py);
                        opaque &= p[3] > 240;
                        mean.add_rgb(rgb(p));
                        square += luminance(p).powi(2);
                    }
                }
                let c = mean.rgb();
                let brightness = c.iter().sum::<f32>() / 3.0;
                let chroma =
                    c.iter().copied().fold(0.0, f32::max) - c.iter().copied().fold(1.0, f32::min);
                let variance = (square / 256.0 - brightness.powi(2)).abs();
                let score = chroma - variance * 4.0 - (brightness - 0.48).abs() * 0.3;
                if opaque && score > best {
                    best = score;
                    patch = (x, y);
                }
            }
        }
        let texture = image::imageops::crop_imm(&detail, patch.0, patch.1, 64, 64).to_image();
        let texture =
            image::imageops::resize(&texture, 256, 256, image::imageops::FilterType::Triangle);
        let texture_mean = texture.pixels().map(luminance).sum::<f32>() / (256.0 * 256.0);
        let base = Vec3::from_array(material.rgb());
        let unseated = self.marked().next().is_none();
        let (lo, hi) = self.cells.iter().map(|c| px(c.at)).fold(
            (Vec2::splat(f32::INFINITY), Vec2::splat(f32::NEG_INFINITY)),
            |(lo, hi), p| (lo.min(p), hi.max(p)),
        );
        let lo = lo - Vec2::new(3f32.sqrt() / 2.0, 1.0) * HEX;
        let hi = hi + Vec2::new(3f32.sqrt() / 2.0, 1.0) * HEX;
        RgbaImage::from_fn(self.canvas, self.canvas, |x, y| {
            let world = self.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
            let Some(cell) = self.in_cell(world, HEX) else {
                return Rgba([0; 4]);
            };
            let d = (world - px(cell.at)) / HEX;
            let h = look::hex_norm(d.x, d.y);
            let r = d.length();
            let mirror = |p: u32| {
                let v = p % 512;
                if v < 256 { v } else { 511 - v }
            };
            let sample = texture.get_pixel(mirror(x), mirror(y));
            let grain = if sample[3] > 192 {
                (luminance(sample) - texture_mean) * 1.4
            } else {
                0.0
            };
            let bevel = (-d.x + d.y) * 0.10;
            let mut color = base * (0.86 + grain + 0.06 * d.y);
            if unseated {
                let uv = (world - lo) / (hi - lo);
                let sx = (uv.x * candidate.width() as f32) as u32;
                let sy = ((1.0 - uv.y) * candidate.height() as f32) as u32;
                let p = candidate.get_pixel(
                    sx.min(candidate.width() - 1),
                    sy.min(candidate.height() - 1),
                );
                color = color.lerp(Vec3::from_array(rgb(p)), alpha(p));
            }
            if h > 0.975 {
                color = Vec3::new(0.18, 0.14, 0.10);
            } else if h > 0.92 {
                color = Vec3::new(0.55, 0.43, 0.27) * (1.0 + bevel + grain * 0.4);
            } else if h > 0.90 {
                color = base * 0.45;
            }
            for i in 0..6 {
                let angle = std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::TAU / 6.0;
                let bolt = d - Vec2::from_angle(angle) * 0.81;
                let distance = bolt.length();
                if distance < 0.042 {
                    color = if distance > 0.031 {
                        Vec3::splat(0.12)
                    } else {
                        Vec3::new(0.65, 0.51, 0.32) * (1.0 + bolt.y * 7.0)
                    };
                }
            }
            if cell.role != Role::Body {
                if r < 0.30 {
                    return Rgba([0; 4]);
                } else if r < 0.325 {
                    color = Vec3::new(0.12, 0.10, 0.08);
                } else if r < 0.405 {
                    color = Vec3::new(0.78, 0.65, 0.43) * (1.0 + bevel + grain * 0.25);
                } else if r < 0.44 {
                    let glaze = match cell.role {
                        Role::Seat(Slot {
                            consumed: false, ..
                        }) => Glaze::BlueGreen,
                        Role::Pivot => Glaze::Brass,
                        _ => Glaze::Terracotta,
                    }
                    .color()
                    .to_srgba();
                    color = Vec3::new(glaze.red, glaze.green, glaze.blue);
                } else if r < 0.46 {
                    color = base * 0.4;
                }
            }
            rgba(color.to_array(), 1.0)
        })
    }

    fn score(&self, capture: &Capture) -> Score {
        let candidate = &capture.image;
        let (origin, side) = self.crop();
        let mut outside = 0f32;
        for y in origin..origin + side {
            for x in origin..origin + side {
                if candidate.get_pixel(x, y)[3] > 0 {
                    let world = self.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
                    outside = outside.max(self.outside(world));
                }
            }
        }
        let seat = self
            .marked()
            .map(|cell| self.seat_contrast(candidate, cell, self.seat_bounds(cell)))
            .fold(f32::INFINITY, f32::min);
        Score {
            outside,
            seat,
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
            inside
                .then(|| Vec3::from_array(over_black(candidate.get_pixel(p.x as u32, p.y as u32))))
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
}

struct Capture {
    image: RgbaImage,
    off_centre: f32,
}

fn encode(v: Vec3, alpha: f32) -> Rgba<u8> {
    rgba(
        [(v.x + 1.0) / 2.0, (v.y + 1.0) / 2.0, (v.z + 1.0) / 2.0],
        alpha,
    )
}

#[derive(Debug, Clone, PartialEq)]
struct Score {
    outside: f32,
    seat: f32,
    off_centre: f32,
}

impl Score {
    fn measured(&self, t: &Thresholds) -> Option<String> {
        if self.outside > 0.0 {
            Some(format!("outside {:.3} > 0", self.outside))
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

#[cfg(test)]
fn save(image: &RgbaImage, path: &Path) {
    image
        .save(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
}

enum Refusal {
    Spent(String),
    Failed(String),
}

type Painter<'a> = &'a (dyn Fn(&Path, &Path) -> Result<RgbaImage, Refusal> + Sync);

fn external(script: &Path, caption: &Path, out: &Path) -> Result<RgbaImage, Refusal> {
    let image = out.join("image.png");
    let output = std::process::Command::new("timeout")
        .arg("900")
        .arg(script)
        .args([caption, &image])
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| Refusal::Failed(e.to_string()))?;
    let why = String::from_utf8_lossy(&output.stderr).to_string();
    let logged = std::fs::write(
        out.join("generator.log"),
        [&output.stdout[..], &output.stderr[..]].concat(),
    );
    if output.status.code() == Some(3) {
        return Err(Refusal::Spent(why));
    }
    logged.map_err(|e| Refusal::Failed(e.to_string()))?;
    if !output.status.success() {
        return Err(Refusal::Failed(format!("{}: {why}", output.status)));
    }
    image::open(image)
        .map(|i| i.into_rgba8())
        .map_err(|e| Refusal::Failed(e.to_string()))
}

#[derive(Debug, Clone, PartialEq)]
struct Attempt {
    index: u32,
    seconds: f32,
    outcome: Result<Score, String>,
}

impl Attempt {
    fn verdict(&self, t: &Thresholds) -> String {
        match &self.outcome {
            Ok(score) => score
                .measured(t)
                .map_or("pass".to_string(), |rule| format!("fail {rule}")),
            Err(why) => why.replace(['\t', '\n'], " "),
        }
    }

    fn row(&self, t: &Thresholds) -> String {
        let measured = self.outcome.as_ref().map_or("-\t-\t-".to_string(), |s| {
            format!("{}\t{}\t{}", s.outside, s.seat, s.off_centre)
        });
        format!(
            "{}\t{}\t{measured}\t{:.3}",
            self.index,
            self.verdict(t),
            self.seconds
        )
    }

    fn parse(row: &str) -> Option<Attempt> {
        let cols: Vec<&str> = row.split('\t').collect();
        let [index, verdict, outside, seat, off_centre, seconds] = cols[..] else {
            return None;
        };
        let outcome = if outside == "-" {
            Err(verdict.to_string())
        } else {
            Ok(Score {
                outside: outside.parse().ok()?,
                seat: seat.parse().ok()?,
                off_centre: off_centre.parse().ok()?,
            })
        };
        Some(Attempt {
            index: index.parse().ok()?,
            seconds: seconds.parse().ok()?,
            outcome,
        })
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

fn painted_key(caption: &str, generator: &str) -> String {
    key(&[
        caption.as_bytes(),
        generator.as_bytes(),
        b"rgba-generator-v1",
    ])
}

fn relief_key(albedo: &[u8]) -> String {
    key(&[
        albedo,
        b"surface-normals:blur=4,height=alpha*(0.85+0.15*luma),slope=24",
    ])
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

fn generated_image(image: &RgbaImage) -> Result<(), String> {
    let (w, h) = image.dimensions();
    if w == 0 || w != h || !image.pixels().any(|p| p[3] > 0) || !image.pixels().any(|p| p[3] == 0) {
        return Err(format!(
            "generator must return a nonempty square RGBA cutout, got {w}x{h}"
        ));
    }
    Ok(())
}

fn finish(
    art: &Art,
    name: &str,
    image: &RgbaImage,
    mut entry: Entry,
    attempt: (u32, f32),
    rows: &mut Vec<Attempt>,
    thresholds: &Thresholds,
) -> Result<Entry, String> {
    let (index, seconds) = attempt;
    let scaffold = Scaffold::of(item(name));
    let capture = scaffold.register(image);
    let stage = art.attempt(name, index);
    std::fs::create_dir_all(&stage).map_err(|e| e.to_string())?;
    let albedo = stage.join("albedo.png");
    scaffold
        .cropped(&capture.image)
        .save(&albedo)
        .map_err(|e| e.to_string())?;
    quantise(&albedo)?;
    let mut encoded = open(&albedo);
    let (origin, _) = scaffold.crop();
    let mut clipped = false;
    for (x, y, pixel) in encoded.enumerate_pixels_mut() {
        let world = scaffold.world(Vec2::new(
            (x + origin) as f32 + 0.5,
            (y + origin) as f32 + 0.5,
        ));
        if pixel[3] > 0 && !scaffold.covered(world) {
            pixel[3] = 0;
            clipped = true;
        }
    }
    if clipped {
        encoded.save(&albedo).map_err(|e| e.to_string())?;
    }
    let mounted = scaffold.mount(&encoded);
    let off_centre = scaffold.seat_offset(&mounted);
    let score = scaffold.score(&Capture {
        image: mounted.clone(),
        off_centre,
    });
    if score.off_centre * PX_PER_HEX > 6.0 {
        return Err(format!(
            "encoded seat offset exceeds 6 asset pixels: {}",
            score.off_centre * PX_PER_HEX
        ));
    }
    if score.outside != 0.0 {
        return Err(format!(
            "encoded sprite escaped footprint: {}",
            score.outside
        ));
    }
    println!(
        "{name}\tbounds pass\tworst seat offset {:.2}px\tgenerator {:.3}s",
        score.off_centre * PX_PER_HEX,
        seconds
    );
    rows.push(Attempt {
        index,
        seconds,
        outcome: Ok(score),
    });
    scaffold
        .cropped(&scaffold.surface_normals(&mounted))
        .save(stage.join("normal.png"))
        .map_err(|e| e.to_string())?;
    for file in ["albedo.png", "normal.png"] {
        std::fs::rename(stage.join(file), art.machine(name).join(file))
            .map_err(|e| e.to_string())?;
    }
    std::fs::write(
        art.machine(name).join("attempts.tsv"),
        rows.iter()
            .map(|a| a.row(thresholds) + "\n")
            .collect::<String>(),
    )
    .map_err(|e| e.to_string())?;
    entry.kept = Some(index);
    entry.relief = Some(relief_key(encoded.as_raw()));
    Ok(entry)
}

fn generate_one(
    art: &Art,
    name: &str,
    manifest: &Manifest,
    generator: &str,
    painter: Painter,
    stopped: &std::sync::Mutex<Option<String>>,
    fit: bool,
) -> Result<Entry, String> {
    let started = std::time::Instant::now();
    let mut entry = manifest.machine[name].clone();
    let caption = art.caption(name);
    let text = std::fs::read_to_string(&caption).map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    if fit {
        let image = open(art.machine(name).join("albedo.png"));
        entry = finish(
            art,
            name,
            &image,
            entry.clone(),
            (entry.kept.ok_or("no retained image")?, 0.0),
            &mut rows,
            &manifest.thresholds,
        )?;
    } else {
        entry.generator = generator.to_string();
        entry.painted = Some(painted_key(&text, generator));
        std::fs::remove_dir_all(art.machine(name).join("attempts"))
            .or_else(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Ok(())
                } else {
                    Err(e)
                }
            })
            .map_err(|e| e.to_string())?;
        let mut success = None;
        for index in 1..=manifest.attempts {
            if let Some(why) = stopped.lock().unwrap().as_ref() {
                return Err(format!("stopped: {why}"));
            }
            let out = art.attempt(name, index);
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            let call = std::time::Instant::now();
            let generated = painter(&caption, &out).and_then(|image| {
                generated_image(&image)
                    .map(|()| image)
                    .map_err(Refusal::Failed)
            });
            let seconds = call.elapsed().as_secs_f32();
            match generated {
                Ok(image) => {
                    success = Some(finish(
                        art,
                        name,
                        &image,
                        entry,
                        (index, seconds),
                        &mut rows,
                        &manifest.thresholds,
                    )?);
                    break;
                }
                Err(Refusal::Spent(why)) => {
                    *stopped.lock().unwrap() = Some(why.clone());
                    return Err(format!("stopped: {why}"));
                }
                Err(Refusal::Failed(why)) => {
                    eprintln!(
                        "{name}: generator attempt {index}/{} failed: {why}",
                        manifest.attempts
                    );
                    rows.push(Attempt {
                        index,
                        seconds,
                        outcome: Err(why),
                    });
                    std::fs::write(
                        art.machine(name).join("attempts.tsv"),
                        rows.iter()
                            .map(|a| a.row(&manifest.thresholds) + "\n")
                            .collect::<String>(),
                    )
                    .map_err(|e| e.to_string())?;
                }
            }
        }
        entry = success
            .ok_or_else(|| format!("generator failed all {} attempts", manifest.attempts))?;
    }
    if !art.split(manifest, &[name.to_string()]) {
        return Err("rig splitting failed".to_string());
    }
    println!("{name}\tcomplete\t{:.3}s", started.elapsed().as_secs_f32());
    Ok(entry)
}

fn generate_set(
    art: &Art,
    names: &[String],
    generator: &str,
    painter: Painter,
    jobs: usize,
    fit: bool,
) -> bool {
    let mut manifest = art.read();
    let stop_file = art
        .dir
        .join(format!(".generation-stop-{}", std::process::id()));
    let _ = std::fs::remove_file(&stop_file);
    let next = std::sync::atomic::AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    let stopped = std::sync::Mutex::new(None);
    std::thread::scope(|scope| {
        for _ in 0..jobs.min(names.len()) {
            let (next, results, stopped, manifest) = (&next, &results, &stopped, &manifest);
            scope.spawn(move || {
                loop {
                    let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let Some(name) = names.get(index) else { break };
                    let result =
                        generate_one(art, name, manifest, generator, painter, stopped, fit);
                    results.lock().unwrap().push((name.clone(), result));
                }
            });
        }
    });
    let mut ok = true;
    for (name, result) in results.into_inner().unwrap() {
        match result {
            Ok(entry) => {
                manifest.machine.insert(name, entry);
            }
            Err(why) => {
                eprintln!("{name}: generation failed: {why}");
                ok = false;
            }
        }
    }
    let _ = std::fs::remove_file(stop_file);
    art.write(&manifest);
    if let Err(why) = sheet(art, &manifest) {
        eprintln!("sheet: {why}");
        ok = false;
    }
    ok
}

fn sheet(art: &Art, m: &Manifest) -> Result<(), String> {
    let mut args: Vec<std::ffi::OsString> = vec!["montage".into()];
    for item in Machine::ALL {
        let name = name(item);
        let dir = art.machine(name);
        let (Some(kept), true) = (
            m.machine.get(name).and_then(|e| e.kept),
            dir.join("albedo.png").exists(),
        ) else {
            continue;
        };
        let verdict = attempts(&dir).get(&kept).map_or("-".to_string(), |a| {
            let offset = a
                .outcome
                .as_ref()
                .map_or(f32::INFINITY, |s| s.off_centre * PX_PER_HEX);
            let bounds = a.outcome.as_ref().is_ok_and(|s| s.outside == 0.0);
            format!(
                "bounds {}; worst seat offset {:.1}px",
                if bounds { "pass" } else { "FAIL" },
                offset,
            )
        });
        args.push("-label".into());
        args.push(format!("{name}  attempt {kept}/{}\n{verdict}", m.attempts).into());
        args.push(dir.join("albedo.png").into());
    }
    if args.len() == 1 {
        return Ok(());
    }
    let part = art.dir.join("sheet.png.part");
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
    std::fs::rename(&part, art.dir.join("sheet.png")).map_err(|e| e.to_string())
}

pub fn configure(args: &[String]) -> Option<i32> {
    if args.get(1).map(String::as_str) != Some("--gen") {
        return None;
    }
    let started = std::time::Instant::now();
    let art = Art::shipped();
    let manifest = art.read();
    if args.get(2).map(String::as_str) == Some("--sheet") && args.len() == 3 {
        return Some(match sheet(&art, &manifest) {
            Ok(()) => 0,
            Err(why) => {
                eprintln!("sheet: {why}");
                1
            }
        });
    }
    let mut generator = default_generator();
    let mut jobs = 3;
    let mut fit = false;
    let mut names = Vec::new();
    let mut rest = args.iter().skip(2);
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--generator" => generator = rest.next().cloned().unwrap_or_default(),
            "--jobs" => jobs = rest.next().and_then(|n| n.parse().ok()).unwrap_or(0),
            "--fit" => fit = true,
            "--all" => names.extend(manifest.machine.keys().cloned()),
            name if manifest.machine.contains_key(name) => names.push(name.to_string()),
            _ => {
                eprintln!("unknown generator argument: {arg}");
                return Some(2);
            }
        }
    }
    if names.is_empty()
        || !(1..=16).contains(&jobs)
        || generator.is_empty()
        || manifest.attempts == 0
    {
        eprintln!(
            "usage: ziral --gen [--generator ming|EXECUTABLE] [--jobs 1..16] [--fit] NAME...|--all"
        );
        return Some(2);
    }
    names.sort();
    names.dedup();
    let painter = |caption: &Path, out: &Path| {
        if generator == "ming" {
            ming::generate(&art, caption, out)
        } else {
            external(Path::new(&generator), caption, out)
        }
    };
    let ok = generate_set(&art, &names, &generator, &painter, jobs, fit);
    println!(
        "set\t{} machines\t{:.3}s\t{}",
        names.len(),
        started.elapsed().as_secs_f32(),
        if ok { "complete" } else { "FAILED" }
    );
    Some(i32::from(!ok))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::look::{AMBIENT, light};
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
    fn machines_registration_builds_the_hex_surface() {
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
        assert!(score.outside <= 0.05);
    }

    #[test]
    fn machines_clip_every_alpha_level_to_the_exact_footprint() {
        for item in Machine::ALL {
            let scaffold = Scaffold::of(item);
            let candidate = RgbaImage::from_fn(65, 65, |x, y| {
                Rgba([150, 90, 60, if x == 0 || y == 0 { 1 } else { 255 }])
            });
            let fitted = scaffold.fit(&candidate);
            assert!(fitted.pixels().any(|p| p[3] == 255));
            for (x, y, pixel) in fitted.enumerate_pixels() {
                if pixel[3] > 0 {
                    assert!(
                        scaffold.covered(scaffold.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5))),
                        "{} at {x},{y}",
                        name(item)
                    );
                }
            }
        }
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
            let clean = scaffold.fit(&fired(&scaffold, &|w| w));
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
                    if inside && scaffold.covered(px(h)) {
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
    fn a_candidate_painted_beyond_its_footprint_is_rejected() {
        let scaffold = Scaffold::of(Machine::Glyph(crate::sim::GlyphKind::Bonder));
        let thresholds = Art::shipped().read().thresholds;
        let clean = scaffold.fit(&fired(&scaffold, &|w| w));
        let score = scaffold.score(&Capture {
            image: clean.clone(),
            off_centre: 0.0,
        });
        assert!(score.measured(&thresholds).is_none(), "{score:?}");
        assert!(score.outside <= SEAT_STEP, "{score:?}");
        let face = px(scaffold.cells[0].at) - Vec2::X * HEX * 3f32.sqrt() / 2.0;
        let dot = 2.0;
        for (k, ok) in [(0.5, false), (1.5, false)] {
            let at = face - Vec2::X * k * 0.05 * HEX;
            assert!(
                (scaffold.outside(at) - k * 0.05).abs() < SEAT_STEP,
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
            let score = scaffold.score(&Capture {
                image: painted,
                off_centre: 0.0,
            });
            assert_eq!(score.measured(&thresholds).is_none(), ok, "{score:?}");
            assert!(
                (score.outside - k * 0.05).abs() <= SEAT_STEP,
                "{score:?} against {k} tolerances"
            );
        }
    }

    #[test]
    fn machines_construct_exact_hex_edges_and_seats_for_arbitrary_generators() {
        for item in Machine::ALL {
            let scaffold = Scaffold::of(item);
            for color in [[0, 0, 0, 255], [255, 255, 255, 255], [200, 40, 80, 255]] {
                let candidate = RgbaImage::from_fn(32, 32, |x, y| {
                    if x > 8 && y < 24 {
                        Rgba(color)
                    } else {
                        Rgba([0; 4])
                    }
                });
                let capture = scaffold.register(&candidate);
                assert!(
                    capture.off_centre * PX_PER_HEX <= 6.0,
                    "{}: {}",
                    name(item),
                    capture.off_centre * PX_PER_HEX
                );
                for (x, y, p) in capture.image.enumerate_pixels() {
                    let world = scaffold.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
                    let aperture = scaffold
                        .marked()
                        .any(|c| world.distance(px(c.at)) < 0.30 * HEX);
                    assert_eq!(
                        p[3] > 0,
                        scaffold.covered(world) && !aperture,
                        "{} at {x},{y}",
                        name(item)
                    );
                }
                for cell in scaffold.marked() {
                    let mut sum = Vec2::ZERO;
                    let mut count = 0.0;
                    for (x, y, p) in capture.image.enumerate_pixels() {
                        let world = scaffold.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
                        if world.distance(px(cell.at)) < 0.5 * HEX && p[3] == 0 {
                            sum += world;
                            count += 1.0;
                        }
                    }
                    assert!(count > 0.0);
                    assert!((sum / count).distance(px(cell.at)) * scale() < 1.0);
                }
            }
        }
    }

    #[test]
    fn machines_measure_missing_and_displaced_encoded_apertures_as_failures() {
        let scaffold = Scaffold::of(Machine::Glyph(crate::sim::GlyphKind::Bonder));
        let clean = scaffold.fit(&RgbaImage::from_pixel(8, 8, Rgba([120, 80, 60, 255])));
        let shifted = RgbaImage::from_fn(scaffold.canvas, scaffold.canvas, |x, y| {
            if x >= 16 {
                *clean.get_pixel(x - 16, y)
            } else {
                Rgba([0; 4])
            }
        });
        assert!(scaffold.seat_offset(&shifted) * PX_PER_HEX > 6.0);
        let mut filled = clean;
        for (x, y, p) in filled.enumerate_pixels_mut() {
            let w = scaffold.world(Vec2::new(x as f32 + 0.5, y as f32 + 0.5));
            if scaffold.covered(w) {
                p[3] = 255;
            }
        }
        assert!(scaffold.seat_offset(&filled).is_infinite());
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

    fn decode(p: &Rgba<u8>) -> Vec3 {
        let c = rgb(p);
        Vec3::new(c[0] * 2.0 - 1.0, c[1] * 2.0 - 1.0, c[2] * 2.0 - 1.0)
    }

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
        for item in Machine::ALL {
            let name = name(item);
            let entry = &manifest.machine[name];
            let dir = art.machine(name);
            let caption = std::fs::read_to_string(art.caption(name))
                .unwrap_or_else(|e| panic!("{name}/caption.txt: {e}"));
            assert_eq!(
                entry.painted.as_deref(),
                Some(painted_key(&caption, &entry.generator).as_str()),
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
            let scaffold = Scaffold::of(item);
            let (_, side) = scaffold.crop();
            let albedo = open(dir.join("albedo.png"));
            assert_eq!(albedo.dimensions(), (side, side), "{name}/albedo.png");
            let mounted = scaffold.mount(&albedo);
            let score = scaffold.score(&Capture {
                image: mounted.clone(),
                off_centre: scaffold.seat_offset(&mounted),
            });
            assert_eq!(
                score.outside, 0.0,
                "{name}: nonzero alpha outside footprint"
            );
            assert!(
                (score.outside - recorded.outside).abs() <= SHIPPED,
                "{name}: albedo.png measures {score:?}, attempts.tsv records {recorded:?}"
            );
            assert!(score.off_centre * PX_PER_HEX <= 6.0, "{name}: {score:?}");
            assert!(
                (score.off_centre - recorded.off_centre).abs() < 0.001,
                "{name}: stale seat measurement"
            );
            for map in ["albedo", "normal"] {
                let png = open(dir.join(format!("{map}.png")));
                let placed = scaffold.quad.size();
                let aspect = png.width() as f32 / png.height() as f32 / (placed.x / placed.y);
                assert!((aspect - 1.0).abs() <= ASPECT, "{name}/{map}.png");
                assert_eq!(png.dimensions(), (side, side), "{name}/{map}.png");
                if !entry.parts.is_empty() {
                    let base = open(dir.join(format!("parts/{map}-base.png")));
                    let moving = open(dir.join(format!("parts/{map}-moving.png")));
                    assert_eq!(base.dimensions(), png.dimensions(), "{name}/{map} base");
                    assert_eq!(moving.dimensions(), png.dimensions(), "{name}/{map} moving");
                    for ((whole, base), moving) in
                        png.pixels().zip(base.pixels()).zip(moving.pixels())
                    {
                        let sum = u16::from(base[3]) + u16::from(moving[3]);
                        assert!(
                            sum.abs_diff(u16::from(whole[3])) <= 1,
                            "{name}/{map}: stale part alpha"
                        );
                        for part in [base, moving] {
                            if part[3] > 0 {
                                assert_eq!(
                                    &part.0[..3],
                                    &whole.0[..3],
                                    "{name}/{map}: stale part colour"
                                );
                            }
                        }
                    }
                }
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
            assert_eq!(
                entry.relief.as_deref(),
                Some(relief_key(albedo.as_raw()).as_str()),
                "{name}: normal.png is stale against albedo.png: run ziral --gen {name}"
            );
            let normal = scaffold.surface_normals(&mounted);
            assert!(
                shipped_normal.as_raw() == scaffold.cropped(&normal).as_raw(),
                "{name}: shipped normals differ from the computed relief"
            );
        }
    }

    fn studio(tag: &str, names: &[&str], attempts: u32) -> Art {
        let root = std::env::temp_dir().join(format!("ziral-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let art = Art {
            dir: root.join("machines"),
        };
        let mut manifest = Art::shipped().read();
        manifest.attempts = attempts;
        manifest
            .machine
            .retain(|name, _| names.contains(&name.as_str()));
        for name in names {
            std::fs::create_dir_all(art.machine(name)).unwrap();
            std::fs::write(art.caption(name), format!("a {name}")).unwrap();
            manifest.machine.get_mut(*name).unwrap().parts.clear();
        }
        art.write(&manifest);
        art
    }

    #[test]
    fn machines_retry_generator_failures_then_construct_seats_without_repainting() {
        let art = studio("generator-retry", &["bonder"], 3);
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let painter = |_: &Path, _: &Path| {
            if calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) == 0 {
                Err(Refusal::Failed("HTTP 502".to_string()))
            } else {
                Ok(RgbaImage::from_fn(65, 65, |x, y| {
                    if (8..56).contains(&x) && (8..56).contains(&y) {
                        Rgba([150, 90, 60, 255])
                    } else {
                        Rgba([0; 4])
                    }
                }))
            }
        };
        assert!(generate_set(
            &art,
            &["bonder".to_string()],
            "fixture",
            &painter,
            1,
            false
        ));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 2);
        let rows = attempts(&art.machine("bonder"));
        assert!(rows[&1].outcome.is_err());
        assert_eq!(rows[&2].outcome.as_ref().unwrap().outside, 0.0);
        assert!(
            rows[&2]
                .outcome
                .as_ref()
                .unwrap()
                .measured(&art.read().thresholds)
                .is_none()
        );
        assert_eq!(art.read().machine["bonder"].kept, Some(2));
        std::fs::remove_dir_all(art.dir.parent().unwrap()).unwrap();
    }

    #[test]
    fn machines_stop_on_policy_refusal_and_bound_failed_calls() {
        let art = studio("generator-stop", &["bonder", "resonator"], 3);
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let refusal = |_: &Path, _: &Path| {
            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(Refusal::Spent("usage is nonzero".to_string()))
        };
        let names = vec!["bonder".to_string(), "resonator".to_string()];
        assert!(!generate_set(&art, &names, "fixture", &refusal, 1, false));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        calls.store(0, std::sync::atomic::Ordering::SeqCst);
        let failure = |_: &Path, _: &Path| {
            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Err(Refusal::Failed("HTTP 502".to_string()))
        };
        assert!(!generate_set(
            &art,
            &names[..1],
            "fixture",
            &failure,
            1,
            false
        ));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 3);
        std::fs::remove_dir_all(art.dir.parent().unwrap()).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn machines_downstream_failure_never_retries_the_generator() {
        use std::os::unix::fs::PermissionsExt;
        let art = studio("rig-failure", &["bonder"], 4);
        let mut manifest = art.read();
        manifest.machine.get_mut("bonder").unwrap().parts =
            Art::shipped().read().machine["arm"].parts.clone();
        art.write(&manifest);
        let script = art.dir.join("rig.sh");
        std::fs::write(&script, "#!/usr/bin/env bash\nexit 7\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let painter = |_: &Path, _: &Path| {
            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(RgbaImage::from_fn(32, 32, |x, y| {
                if (4..28).contains(&x) && (4..28).contains(&y) {
                    Rgba([120, 70, 50, 255])
                } else {
                    Rgba([0; 4])
                }
            }))
        };
        assert!(!generate_set(
            &art,
            &["bonder".to_string()],
            "fixture",
            &painter,
            1,
            false
        ));
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        std::fs::remove_dir_all(art.dir.parent().unwrap()).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn machines_external_generator_contract_and_policy_exit() {
        use std::os::unix::fs::PermissionsExt;
        let art = studio("generator-contract", &["portal"], 1);
        let script = art.dir.join("fixture.sh");
        let source = art.dir.join("fixture.png");
        save(
            &RgbaImage::from_fn(16, 16, |x, y| {
                if x > 2 && y > 2 && x < 13 && y < 13 {
                    Rgba([120, 70, 50, 255])
                } else {
                    Rgba([0; 4])
                }
            }),
            &source,
        );
        std::fs::write(&script, "#!/usr/bin/env bash\nset -eu\ntest -s \"$1\"\ncp \"$(dirname \"$0\")/fixture.png\" \"$2\"\n").unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let out = art.attempt("portal", 1);
        std::fs::create_dir_all(&out).unwrap();
        let generated = external(&script, &art.caption("portal"), &out)
            .unwrap_or_else(|_| panic!("external generator failed"));
        assert_eq!(generated, open(source));
        std::fs::write(&script, "#!/usr/bin/env bash\necho policy >&2\nexit 3\n").unwrap();
        assert!(matches!(
            external(&script, &art.caption("portal"), &out),
            Err(Refusal::Spent(_))
        ));
        std::fs::remove_dir_all(art.dir.parent().unwrap()).unwrap();
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
