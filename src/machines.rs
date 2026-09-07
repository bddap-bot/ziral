use crate::look::{self, Cell, Glaze, HEX, Quad, Role, px};
use crate::sim::Slot;
use crate::{Item, PALETTE};
use bevy::math::Vec2;
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
}

#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct Thresholds {
    pub candidates: u32,
    pub outside: f32,
    pub seat: f32,
    pub palette: f32,
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
    const USAGE: &str = "usage: ziral --plan | ziral --scaffold NAME | ziral --score NAME PNG... | ziral --keep NAME INDEX";
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
        }
        (Some("--plan" | "--scaffold" | "--score" | "--keep"), _) => panic!("{USAGE}"),
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::{Arm, Glyph, Hex, ORIGIN};

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
            let png = open(dir.join("albedo.png").to_str().unwrap());
            assert_eq!(
                (png.width(), png.height()),
                (side, side),
                "{name}/albedo.png"
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
}
