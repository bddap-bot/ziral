use crate::look::{self, ATOM_RADIUS, HEX, Role, hex_at, hex_norm, px};
use crate::sim::{DIRS, Hex, Machine, ORIGIN};
use bevy::math::Vec2;
use image::codecs::png::{CompressionType, FilterType, PngEncoder};
use image::{ExtendedColorType, ImageEncoder, RgbaImage, imageops};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const PX_PER_HEX: f32 = 256.0;
const LAYOUT_PX: u32 = 1024;
const BEAM: f32 = 0.1;
const ATTEMPTS: usize = 4;
const BLEED: usize = 16;
const SPILL: f32 = 0.005;
const MARGIN: f32 = 0.25;
const WORKERS: usize = 4;
const KEY: [u8; 3] = [0, 255, 0];
const BALL: [u8; 3] = [255, 0, 255];
const TOLERANCE: f32 = 0.03;
const VISIBLE: f32 = 0.5;
const FRINGE: f32 = 2.0;
const OPAQUE: f32 = 0.15;
const CLEAR: f32 = 0.4;
const HELD: f32 = 0.3;
const CONSUMED: f32 = 0.6;
const EMERGES: f32 = 0.55;
const INSET: f32 = 0.8;
const LIP: f32 = 0.7;
const GENERATOR: &str = include_str!("../art/paint/default");
const DERIVE: &str = "art/paint/reference.sh";

#[derive(Deserialize)]
struct Manifest {
    layout: String,
    family: String,
    ball: String,
    design: String,
    style: String,
    machine: BTreeMap<String, Entry>,
}

#[derive(Deserialize)]
struct Entry {
    caption: String,
    like: Option<String>,
}

fn manifest() -> Manifest {
    toml::from_str(include_str!("../art/machines/manifest.toml"))
        .unwrap_or_else(|error| panic!("art/machines/manifest.toml: {error}"))
}

fn caption(manifest: &Manifest, item: Machine) -> &str {
    let name = look::name(item);
    &manifest
        .machine
        .get(name)
        .unwrap_or_else(|| panic!("machine {name} has no caption"))
        .caption
}

fn prompt(manifest: &Manifest, item: Machine) -> String {
    if design(Path::new(env!("CARGO_MANIFEST_DIR")), item).is_some() {
        return format!(
            "{} {} {}",
            manifest.layout,
            caption(manifest, item),
            manifest.design
        );
    }
    let family = match like(manifest, item) {
        Some(_) => format!(" {}", manifest.family),
        None => String::new(),
    };
    let ball = match hand(item) {
        Some(_) => format!(" {}", manifest.ball),
        None => String::new(),
    };
    format!(
        "{}{ball} {}{family} {}",
        manifest.layout,
        caption(manifest, item),
        manifest.style
    )
}

fn hand(item: Machine) -> Option<Vec2> {
    look::footprint(item)
        .iter()
        .find(|cell| cell.role == Role::Hand)
        .map(|cell| px(cell.at))
}

fn like(manifest: &Manifest, item: Machine) -> Option<Machine> {
    manifest.machine[look::name(item)]
        .like
        .as_deref()
        .map(look::named)
}

fn side(item: Machine) -> u32 {
    (look::quad(item).side / HEX * PX_PER_HEX).round() as u32
}

#[derive(Clone, Copy)]
struct Frame {
    centre: Vec2,
    side: f32,
    px: f32,
}

impl Frame {
    fn new(item: Machine, px: u32) -> Self {
        let quad = look::quad(item);
        Frame {
            centre: quad.centre,
            side: quad.side,
            px: px as f32,
        }
    }

    fn world(self, x: u32, y: u32) -> Vec2 {
        let at = (Vec2::new(x as f32, y as f32) + 0.5) / self.px - 0.5;
        self.centre + Vec2::new(at.x, -at.y) * self.side
    }

    fn cover(self, depth: f32) -> f32 {
        (depth * self.px / self.side + 0.5).clamp(0.0, 1.0)
    }
}

struct Footprint {
    hexes: Vec<Hex>,
    cells: Vec<Vec2>,
    ring: Vec<Vec2>,
}

impl Footprint {
    fn of(item: Machine) -> Self {
        let hexes: Vec<Hex> = look::footprint(item).iter().map(|cell| cell.at).collect();
        let mut ring: Vec<Hex> = hexes
            .iter()
            .flat_map(|hex| DIRS.map(|dir| hex.add(dir)))
            .filter(|hex| !hexes.contains(hex))
            .collect();
        ring.sort();
        ring.dedup();
        Footprint {
            cells: hexes.iter().copied().map(px).collect(),
            hexes,
            ring: ring.into_iter().map(px).collect(),
        }
    }

    fn depth(&self, at: Vec2) -> f32 {
        let gap = |cells: &[Vec2]| {
            cells
                .iter()
                .map(|cell| {
                    let d = (at - *cell) / HEX;
                    hex_norm(d.x, d.y) - 1.0
                })
                .fold(f32::INFINITY, f32::min)
                * 3f32.sqrt()
                / 2.0
                * HEX
        };
        if self.hexes.contains(&hex_at(at)) {
            gap(&self.ring)
        } else {
            -gap(&self.cells)
        }
    }
}

fn layout(item: Machine) -> RgbaImage {
    let frame = Frame::new(item, LAYOUT_PX);
    let footprint = look::footprint(item);
    let shape = Footprint::of(item);
    let body = look::machine(item)
        .glaze
        .rgb()
        .map(|c| (c * 255.0).round() as u8);
    let role = |wanted: Role| {
        footprint
            .iter()
            .find(|cell| cell.role == wanted)
            .map(|cell| px(cell.at))
    };
    let beam = role(Role::Pivot).zip(role(Role::Hand));
    let ball = hand(item);
    let emerges = |at: Hex| match item {
        Machine::Glyph(kind) => kind.product() == Some(at) || kind.is_source(),
        _ => false,
    };
    let marks: Vec<(Vec2, [u8; 3], f32)> = footprint
        .iter()
        .filter_map(|cell| match cell.role {
            Role::Seat(_) if emerges(cell.at) => Some((px(cell.at), [255; 3], EMERGES)),
            Role::Seat(slot) if slot.consumed => Some((px(cell.at), [0; 3], CONSUMED)),
            Role::Seat(_) => Some((px(cell.at), [0; 3], HELD)),
            _ => None,
        })
        .collect();
    RgbaImage::from_fn(LAYOUT_PX, LAYOUT_PX, |x, y| {
        let at = frame.world(x, y);
        let solid = frame.cover(match beam {
            Some((pivot, hand)) => {
                let along = (at - pivot).dot(hand - pivot) / (hand - pivot).length_squared();
                let d = (at - pivot) / HEX;
                (BEAM * HEX - at.distance(pivot + (hand - pivot) * along.clamp(0.0, 1.0)))
                    .max((INSET - hex_norm(d.x, d.y)) * HEX)
            }
            None if item == Machine::Portal => {
                shape.depth(at).min(look::OPENING.body(at - px(ORIGIN)))
            }
            None => shape.depth(at),
        });
        let hole = match item {
            Machine::Portal => frame.cover(look::OPENING.depth(at - px(ORIGIN))),
            _ => 0.0,
        };
        let mix = |a: [u8; 3], b: [u8; 3], t: f32| {
            std::array::from_fn(|c| {
                (f32::from(a[c]) * (1.0 - t) + f32::from(b[c]) * t).round() as u8
            })
        };
        let mut rgb = mix(KEY, body, solid * (1.0 - hole));
        for (centre, tint, amount) in &marks {
            let d = (at - *centre) / HEX;
            let inside = frame.cover((INSET - hex_norm(d.x, d.y)) * HEX);
            rgb = mix(rgb, *tint, amount * inside);
        }
        if let Some(hand) = ball {
            rgb = mix(rgb, BALL, frame.cover(ATOM_RADIUS - at.distance(hand)));
        }
        let [r, g, b] = rgb;
        image::Rgba([r, g, b, 255])
    })
}

fn matte(rgb: [f32; 3]) -> f32 {
    let [r, g, b] = rgb;
    1.0 - ((g - r.max(b) - OPAQUE) / (CLEAR - OPAQUE)).clamp(0.0, 1.0)
}

fn ball(rgb: [f32; 3]) -> f32 {
    let [r, g, b] = rgb;
    ((r.min(b) - g - OPAQUE) / (CLEAR - OPAQUE)).clamp(0.0, 1.0)
}

fn unmix(rgb: [f32; 3], ball: f32) -> [f32; 3] {
    let key = BALL.map(|c| f32::from(c) / 255.0);
    std::array::from_fn(|c| ((rgb[c] - ball * key[c]) / (1.0 - ball)).clamp(0.0, 1.0))
}

fn locate(item: Machine, image: &RgbaImage) -> Result<Option<f32>, String> {
    let Some(hand) = hand(item) else {
        return Ok(None);
    };
    let frame = Frame::new(item, image.width());
    let (mut area, mut sum) = (0.0, Vec2::ZERO);
    for (x, y, pixel) in image.enumerate_pixels() {
        let weight = ball([0, 1, 2].map(|c| f32::from(pixel[c]) / 255.0));
        area += weight;
        sum += frame.world(x, y) * weight;
    }
    let texel = frame.side / frame.px;
    let expected = std::f32::consts::PI * (ATOM_RADIUS / texel).powi(2);
    if area < VISIBLE * expected {
        return Err(format!(
            "the ball shows {:.0}% of its area",
            100.0 * area / expected
        ));
    }
    let offset = (sum / area).distance(hand) / HEX;
    if offset > TOLERANCE {
        return Err(format!("the ball is {offset:.3} hex off its tile centre"));
    }
    Ok(Some(offset))
}

fn despill(rgb: [f32; 3], keyed: f32) -> [f32; 3] {
    let [r, g, b] = rgb;
    [r, if keyed < 1.0 { g.min(r.max(b)) } else { g }, b]
}

// fire() averages colour regardless of alpha when it builds mips, so a clear texel must carry
// its neighbours' colour rather than the key's.
fn bleed(image: &mut RgbaImage) {
    let (w, h) = image.dimensions();
    let mut known: Vec<bool> = image.pixels().map(|p| p[3] > 0).collect();
    for _ in 0..BLEED {
        let mut grown = Vec::new();
        for y in 0..h {
            for x in 0..w {
                if known[(y * w + x) as usize] {
                    continue;
                }
                let mut sum = [0u32; 3];
                let mut n = 0;
                for (nx, ny) in [(-1, 0), (1, 0), (0, -1), (0, 1)]
                    .map(|(dx, dy)| (x as i64 + dx, y as i64 + dy))
                {
                    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                        continue;
                    }
                    if known[(ny as u32 * w + nx as u32) as usize] {
                        let p = image.get_pixel(nx as u32, ny as u32);
                        for c in 0..3 {
                            sum[c] += u32::from(p[c]);
                        }
                        n += 1;
                    }
                }
                if n > 0 {
                    grown.push((x, y, sum.map(|s| (s / n) as u8)));
                }
            }
        }
        if grown.is_empty() {
            break;
        }
        for (x, y, [r, g, b]) in grown {
            *image.get_pixel_mut(x, y) = image::Rgba([r, g, b, 0]);
            known[(y * w + x) as usize] = true;
        }
    }
}

struct Sprite {
    image: RgbaImage,
    fill: f32,
    spill: f32,
    offset: Option<f32>,
}

fn finish(item: Machine, raw: &RgbaImage) -> Result<Sprite, String> {
    let (w, h) = raw.dimensions();
    if w != h {
        return Err(format!("off-aspect {w}x{h} return"));
    }
    let side = side(item);
    let mut image = if w == side {
        raw.clone()
    } else {
        imageops::resize(raw, side, side, imageops::FilterType::Lanczos3)
    };
    let offset = locate(item, &image)?;
    let frame = Frame::new(item, side);
    let shape = Footprint::of(item);
    let (mut covered, mut filled, mut outside, mut spilled) = (0.0, 0.0, 0.0, 0.0);
    let (mut lip, mut lined) = (0.0, 0.0);
    for (x, y, pixel) in image.enumerate_pixels_mut() {
        let rgb = [0, 1, 2].map(|c| f32::from(pixel[c]) / 255.0);
        let depth = shape.depth(frame.world(x, y));
        let hole = match item {
            Machine::Portal => look::OPENING.depth(frame.world(x, y) - px(ORIGIN)),
            _ => f32::NEG_INFINITY,
        };
        let core = hole - look::OPENING.fringe;
        let open = core > 0.0;
        let inside = frame.cover(depth) * (1.0 - frame.cover(core + 0.5 * frame.side / frame.px));
        let keyed = matte(rgb);
        let held = match (offset, hand(item)) {
            (Some(_), Some(hand)) => ball(rgb).max(frame.cover(
                ATOM_RADIUS + FRINGE * frame.side / frame.px - frame.world(x, y).distance(hand),
            )),
            _ => 0.0,
        };
        let painted = keyed * (1.0 - held) * f32::from(pixel[3]) / 255.0;
        let alpha = painted * inside;
        covered += inside;
        filled += alpha;
        if (-look::OPENING.rim..0.0).contains(&hole) {
            lip += 1.0;
            lined += alpha;
        }
        if depth < -MARGIN * HEX || open {
            outside += 1.0;
            spilled += painted;
        }
        let [r, g, b] = if alpha > 0.0 {
            despill(unmix(rgb, held), keyed).map(|c| (c * 255.0).round() as u8)
        } else {
            [0, 0, 0]
        };
        *pixel = image::Rgba([r, g, b, (alpha * 255.0).round() as u8]);
    }
    if spilled > SPILL * outside {
        return Err(format!(
            "{:.1}% of the surround is painted, not key",
            100.0 * spilled / outside
        ));
    }
    if lined < LIP * lip {
        return Err(format!(
            "the rim covers {:.0}% of the opening's edge",
            100.0 * lined / lip
        ));
    }
    bleed(&mut image);
    Ok(Sprite {
        image,
        fill: filled / covered,
        spill: spilled / outside,
        offset,
    })
}

fn generate(
    generator: &str,
    prompt: &str,
    side: u32,
    images: &[PathBuf],
    out: &Path,
) -> (f64, Result<(), String>) {
    let child = Command::new("sh")
        .arg("-c")
        .arg(format!("{generator} \"$@\""))
        .arg("paint")
        .arg(out)
        .arg(side.to_string())
        .args(images)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(child) => child,
        Err(error) => return (0.0, Err(format!("{generator}: {error}"))),
    };
    let _ = child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(prompt.as_bytes());
    let output = match child.wait_with_output() {
        Ok(output) => output,
        Err(error) => return (0.0, Err(format!("{generator}: {error}"))),
    };
    let printed = String::from_utf8_lossy(&output.stdout);
    let cost = printed.trim().parse().ok();
    let result = if !output.status.success() {
        Err(format!("{generator} failed: {}", output.status))
    } else if cost.is_none() {
        Err(format!("{generator} printed {printed:?}, not its cost"))
    } else {
        Ok(())
    };
    (cost.unwrap_or(0.0), result)
}

struct Painted {
    sprite: Sprite,
    raw: (u32, u32),
    attempts: usize,
}

fn paint(
    item: Machine,
    generator: &str,
    prompt: &str,
    reference: Option<PathBuf>,
    scratch: &Path,
) -> (f64, Result<Painted, String>) {
    let name = look::name(item);
    let layout = scratch.join(format!("{name}.layout.png"));
    if let Err(error) = self::layout(item).save(&layout) {
        return (0.0, Err(format!("{}: {error}", layout.display())));
    }
    let mut cost = 0.0;
    let mut failures = Vec::new();
    for attempt in 1..=ATTEMPTS {
        let out = scratch.join(format!("{name}.{attempt}.png"));
        let _ = std::fs::remove_file(&out);
        let images: Vec<PathBuf> = std::iter::once(layout.clone())
            .chain(reference.clone())
            .collect();
        let (spent, generated) = generate(generator, prompt, side(item), &images, &out);
        cost += spent;
        let result = generated
            .and_then(|()| {
                image::open(&out)
                    .map(|raw| raw.to_rgba8())
                    .map_err(|error| format!("{}: {error}", out.display()))
            })
            .and_then(|raw| finish(item, &raw).map(|sprite| (raw.dimensions(), sprite)));
        match result {
            Ok((raw, sprite)) => {
                return (
                    cost,
                    Ok(Painted {
                        sprite,
                        raw,
                        attempts: attempt,
                    }),
                );
            }
            Err(error) => {
                eprintln!("{name} attempt {attempt}: {error}");
                failures.push(error);
            }
        }
    }
    (cost, Err(format!("{name}: {}", failures.join("; "))))
}

fn sprite(root: &Path, item: Machine) -> PathBuf {
    root.join(format!("art/{}.png", look::machine(item).skin.name))
}

fn design(root: &Path, item: Machine) -> Option<PathBuf> {
    Some(sprite(root, item).with_file_name("design.png")).filter(|path| path.exists())
}

fn generator<'a>(chosen: Option<&'a str>, root: &Path, item: Machine) -> &'a str {
    chosen.unwrap_or(match design(root, item) {
        Some(_) => DERIVE,
        None => GENERATOR.trim(),
    })
}

fn reference(manifest: &Manifest, root: &Path, item: Machine) -> Option<PathBuf> {
    design(root, item).or_else(|| like(manifest, item).map(|sibling| sprite(root, sibling)))
}

fn save(image: &RgbaImage, path: &Path) -> Result<(), String> {
    let mut png = Vec::new();
    PngEncoder::new_with_quality(&mut png, CompressionType::Fast, FilterType::Adaptive)
        .write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ExtendedColorType::Rgba8,
        )
        .map_err(|error| format!("{}: {error}", path.display()))?;
    // Dithering diffuses alpha error into clear texels outside the footprint.
    let part = path.with_extension("png.part");
    let mut quantise = Command::new("pngquant")
        .args(["--quality", "70-95", "--speed", "1", "--nofs", "--output"])
        .arg(&part)
        .arg("-")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|error| format!("pngquant: {error}"))?;
    let _ = quantise
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(&png);
    let status = quantise
        .wait()
        .map_err(|error| format!("pngquant: {error}"))?;
    if !status.success() {
        return Err(format!("pngquant failed on {}: {status}", path.display()));
    }
    std::fs::rename(&part, path).map_err(|error| format!("{}: {error}", path.display()))
}

pub fn run(args: &[String]) -> Option<i32> {
    let [_, flag, rest @ ..] = args else {
        return None;
    };
    if flag != "--paint" {
        return None;
    }
    let (chosen, names) = match rest {
        [option, generator, names @ ..] if option == "--generator" => {
            (Some(generator.as_str()), names)
        }
        names => (None, names),
    };
    let mut items: Vec<Machine> = Vec::new();
    for item in names.iter().map(|name| look::named(name)) {
        if !items.contains(&item) {
            items.push(item);
        }
    }
    if items.is_empty() {
        items = Machine::ALL.to_vec();
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let scratch = root.join(".paint");
    std::fs::create_dir_all(&scratch).expect(".paint is writable");
    let manifest = manifest();
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut rows: Vec<(usize, String)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..WORKERS)
            .map(|_| {
                scope.spawn(|| {
                    let mut rows = Vec::new();
                    while let Some(&item) =
                        items.get(next.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
                    {
                        let name = look::name(item);
                        let generator = generator(chosen, &root, item);
                        let words = caption(&manifest, item).split_whitespace().count();
                        let (cost, painted) =
                            paint(
                                item,
                                generator,
                                &prompt(&manifest, item),
                                reference(&manifest, &root, item),
                                &scratch,
                            );
                        let path = sprite(&root, item);
                        let row = match painted.and_then(|painted| {
                            save(&painted.sprite.image, &path).map(|()| painted)
                        }) {
                            Ok(painted) => {
                                format!(
                                    "{name}\t{generator}\t{}\t{cost:.4}\t{}x{}\t{}\t{:.3}\t{:.3}\t{}\t{words}\n",
                                    painted.attempts,
                                    painted.raw.0,
                                    painted.raw.1,
                                    side(item),
                                    painted.sprite.fill,
                                    painted.sprite.spill,
                                    painted
                                        .sprite
                                        .offset
                                        .map_or(String::new(), |offset| format!("{offset:.3}")),
                                )
                            }
                            Err(error) => {
                                eprintln!("{error}");
                                format!("{name}\t{generator}\tfailed\t{cost:.4}\t\t\t\t\t\t{words}\n")
                            }
                        };
                        rows.push((items.iter().position(|i| *i == item).unwrap_or(0), row));
                    }
                    rows
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("a paint worker panicked"))
            .collect()
    });
    rows.sort();
    let table = rows.into_iter().fold(
        String::from(
            "machine\tgenerator\tattempts\tcost_usd\treturn\tside\tfill\tspill\tball_offset_hex\tcaption_words\n",
        ),
        |table, (_, row)| table + &row,
    );
    print!("{table}");
    std::fs::write(scratch.join("table.tsv"), &table).expect(".paint is writable");
    Some(i32::from(table.contains("\tfailed\t")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::GlyphKind;

    fn shipped(item: Machine) -> RgbaImage {
        image::load_from_memory(look::machine(item).skin.png)
            .unwrap_or_else(|error| panic!("{:?}: {error}", look::machine(item).skin))
            .to_rgba8()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ziral-paint-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn bonder() -> Machine {
        Machine::Glyph(GlyphKind::Bonder)
    }

    #[test]
    fn every_sprite_is_square_at_its_size_and_clear_outside_its_footprint() {
        for item in Machine::ALL {
            let skin = look::machine(item).skin;
            let image = shipped(item);
            let side = side(item);
            assert_eq!(image.dimensions(), (side, side), "{skin:?}");
            let frame = Frame::new(item, side);
            let shape = Footprint::of(item);
            let outside = image
                .enumerate_pixels()
                .filter(|(x, y, p)| {
                    p[3] > 0 && frame.cover(shape.depth(frame.world(*x, *y))) == 0.0
                })
                .count();
            assert_eq!(
                outside, 0,
                "{skin:?} paints {outside} px outside its footprint"
            );
        }
    }

    #[test]
    fn edges_shared_by_two_footprint_hexes_are_fully_covered() {
        for item in Machine::ALL {
            let shape = Footprint::of(item);
            let frame = Frame::new(item, side(item));
            for a in &shape.cells {
                for b in &shape.cells {
                    if a.distance(*b) < 1.8 * HEX && a != b {
                        let midpoint = (*a + *b) / 2.0;
                        assert_eq!(
                            frame.cover(shape.depth(midpoint)),
                            1.0,
                            "{} seams between {a} and {b}",
                            look::name(item)
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn the_layout_plug_paints_every_machine_through_the_whole_pipeline() {
        let manifest = manifest();
        let dir = scratch("layout");
        for item in Machine::ALL {
            let (cost, painted) = paint(
                item,
                "art/paint/layout.sh",
                &prompt(&manifest, item),
                reference(&manifest, Path::new(env!("CARGO_MANIFEST_DIR")), item),
                &dir,
            );
            let painted = painted.unwrap();
            assert_eq!(cost, 0.0);
            assert_eq!(painted.attempts, 1);
            assert_eq!(painted.sprite.image.width(), side(item));
            if !matches!(item, Machine::Arm(_) | Machine::Portal) {
                assert!(painted.sprite.fill > 0.9, "{}", look::name(item));
            }
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    fn arms() -> [Machine; 3] {
        crate::sim::ArmLength::ALL.map(Machine::Arm)
    }

    #[test]
    fn no_glaze_and_not_the_key_reads_as_the_ball() {
        for glaze in look::Glaze::ALL {
            assert_eq!(ball(glaze.rgb()), 0.0, "{glaze:?}");
        }
        assert_eq!(ball(KEY.map(|c| f32::from(c) / 255.0)), 0.0);
    }

    #[test]
    fn the_ball_is_found_on_its_hand_and_cleared_from_the_sprite() {
        for item in arms() {
            let sprite = finish(item, &layout(item)).unwrap();
            let offset = sprite.offset.unwrap();
            assert!(offset < 0.005, "{} ball {offset} hex off", look::name(item));
            let frame = Frame::new(item, sprite.image.width());
            let hand = hand(item).unwrap();
            let kept = sprite
                .image
                .enumerate_pixels()
                .filter(|(x, y, p)| {
                    p[3] > 0 && frame.world(*x, *y).distance(hand) < 0.95 * ATOM_RADIUS
                })
                .count();
            assert_eq!(kept, 0, "{} keeps {kept} px of its ball", look::name(item));
        }
    }

    #[test]
    fn a_ball_moved_off_its_tile_centre_is_rejected() {
        for item in arms() {
            let raw = layout(item);
            let shift = (0.05 * HEX / look::quad(item).side * LAYOUT_PX as f32).round() as u32;
            let moved = RgbaImage::from_fn(LAYOUT_PX, LAYOUT_PX, |x, y| {
                *raw.get_pixel(x.saturating_sub(shift), y)
            });
            let error = finish(item, &moved).err().unwrap();
            assert!(error.contains("hex off its tile centre"), "{error}");
        }
    }

    #[test]
    fn a_return_that_drops_the_ball_is_rejected() {
        for item in arms() {
            let mut raw = layout(item);
            for pixel in raw.pixels_mut() {
                if ball([0, 1, 2].map(|c| f32::from(pixel[c]) / 255.0)) > 0.0 {
                    *pixel = image::Rgba([107, 79, 58, 255]);
                }
            }
            let error = finish(item, &raw).err().unwrap();
            assert!(error.contains("the ball shows 0%"), "{error}");
        }
    }

    #[test]
    fn only_arms_are_asked_to_hold_the_ball() {
        let manifest = manifest();
        for item in Machine::ALL {
            assert_eq!(
                prompt(&manifest, item).contains(&manifest.ball),
                matches!(item, Machine::Arm(_)),
                "{}",
                look::name(item)
            );
        }
    }

    #[test]
    fn the_portal_sprite_is_clear_across_its_hole() {
        let item = Machine::Portal;
        let image = shipped(item);
        let frame = Frame::new(item, image.width());
        let opaque = image
            .enumerate_pixels()
            .filter(|(x, y, p)| {
                p[3] > 0
                    && look::OPENING.depth(frame.world(*x, *y) - px(ORIGIN)) > look::OPENING.fringe
            })
            .count();
        assert_eq!(opaque, 0, "the portal paints {opaque} px inside its hole");
    }

    #[test]
    fn a_return_that_leaves_the_opening_unrimmed_is_rejected() {
        let item = Machine::Portal;
        let mut raw = layout(item);
        let frame = Frame::new(item, raw.width());
        for (x, y, pixel) in raw.enumerate_pixels_mut() {
            if look::OPENING.depth(frame.world(x, y) - px(ORIGIN)) > -2.0 * look::OPENING.rim {
                *pixel = image::Rgba([0, 255, 0, 255]);
            }
        }
        let error = finish(item, &raw).err().unwrap();
        assert!(error.contains("of the opening's edge"), "{error}");
    }

    #[test]
    fn an_off_aspect_return_is_rejected_not_stretched() {
        let raw = RgbaImage::new(1024, 1000);
        let error = finish(Machine::Portal, &raw).err().unwrap();
        assert!(error.contains("off-aspect 1024x1000"), "{error}");
    }

    #[test]
    fn a_return_that_paints_its_surround_is_rejected() {
        let filled = RgbaImage::from_pixel(1024, 1024, image::Rgba([128, 90, 60, 255]));
        let error = finish(Machine::Portal, &filled).err().unwrap();
        assert!(error.contains("surround is painted"), "{error}");
    }

    #[test]
    fn a_muted_green_surround_is_rejected_not_left_as_a_haze() {
        let muted = RgbaImage::from_pixel(1024, 1024, image::Rgba([122, 184, 102, 255]));
        let error = finish(Machine::Portal, &muted).err().unwrap();
        assert!(error.contains("surround is painted"), "{error}");
    }

    #[test]
    fn a_failed_attempt_is_painted_again_and_its_cost_counted() {
        let dir = scratch("retry");
        let once = dir.join("once");
        let plug = dir.join("plug.sh");
        std::fs::write(
            &plug,
            format!(
                "cat >/dev/null\nif [ ! -e {0} ]; then touch {0}; echo 0.25; exit 1; fi\ncp \"$3\" \"$1\"\necho 0.5\n",
                once.display()
            ),
        )
        .unwrap();
        let (cost, painted) = paint(bonder(), &format!("sh {}", plug.display()), "", None, &dir);
        assert_eq!(painted.unwrap().attempts, 2);
        assert_eq!(cost, 0.75);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_family_reference_reaches_the_generator_after_the_layout() {
        let dir = scratch("like");
        let plug = dir.join("plug.sh");
        let seen = dir.join("seen");
        std::fs::write(
            &plug,
            format!(
                "cat >/dev/null\necho \"$4\" > {}\ncp \"$3\" \"$1\"\necho 0\n",
                seen.display()
            ),
        )
        .unwrap();
        let reference = dir.join("sibling.png");
        let (_, painted) = paint(
            bonder(),
            &format!("sh {}", plug.display()),
            "",
            Some(reference.clone()),
            &dir,
        );
        painted.unwrap();
        assert_eq!(
            std::fs::read_to_string(&seen).unwrap().trim(),
            reference.display().to_string()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_designed_machine_is_painted_from_its_design_not_the_house_style() {
        let manifest = manifest();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let designed: Vec<Machine> = Machine::ALL
            .into_iter()
            .filter(|item| design(root, *item).is_some())
            .collect();
        assert!(designed.contains(&Machine::Portal));
        for item in designed {
            let path = design(root, item).unwrap();
            assert!(
                path.with_extension("md").exists(),
                "{} has no brief",
                path.display()
            );
            let prompt = prompt(&manifest, item);
            assert!(prompt.contains(&manifest.design), "{}", look::name(item));
            assert!(!prompt.contains(&manifest.style), "{}", look::name(item));
            assert_eq!(reference(&manifest, root, item), Some(path));
        }
    }

    #[test]
    fn a_designed_machine_ships_its_design_derived_unless_another_painter_is_chosen() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        for item in Machine::ALL {
            let Some(path) = design(root, item) else {
                assert_eq!(generator(None, root, item), GENERATOR.trim());
                continue;
            };
            assert_eq!(generator(None, root, item), DERIVE);
            assert_eq!(generator(Some("other"), root, item), "other");
            let derived = finish(item, &image::open(&path).unwrap().to_rgba8())
                .unwrap()
                .image;
            let shipped = shipped(item);
            let (mut error, mut n) = (0.0, 0.0);
            for (a, b) in derived.pixels().zip(shipped.pixels()) {
                let weight = f32::from(a[3].max(b[3])) / 255.0;
                for c in 0..4 {
                    error += weight * f32::from(a[c].abs_diff(b[c]));
                }
                n += 4.0 * weight;
            }
            let error = error / n;
            assert!(
                error < 4.0,
                "{} differs from its derived design by {error:.1} per channel",
                look::name(item)
            );
        }
    }

    #[test]
    fn every_like_names_another_machine() {
        let manifest = manifest();
        for item in Machine::ALL {
            if let Some(sibling) = like(&manifest, item) {
                assert_ne!(sibling, item, "{} is like itself", look::name(item));
            }
        }
    }

    #[test]
    fn a_failing_generator_fails_loudly_and_names_its_machine() {
        let dir = scratch("false");
        let (_, painted) = paint(bonder(), "false", "a caption", None, &dir);
        let error = painted.err().unwrap();
        assert!(error.starts_with("bonder: false failed"), "{error}");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn every_caption_is_a_few_dozen_words() {
        let manifest = manifest();
        for item in Machine::ALL {
            let words = caption(&manifest, item).split_whitespace().count();
            assert!(
                words <= 40,
                "{} has a {words} word caption",
                look::name(item)
            );
        }
    }
}
