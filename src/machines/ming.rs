use super::{Art, Mean, Refusal, alpha, rgb, rgba};
use image::RgbaImage;
use std::path::Path;

const COVER: [f32; 2] = [0.03, 0.9];
const FRAME_EDGE: f32 = 0.01;
const EDGE_ALPHA: f32 = 0.02;
const NOISE: f32 = 8.0 / 255.0;
const PLAN: &str = "Decompose this image into 3 layers with the following specifications:\n\nNumber of layers: 3\nLayer 1: The complete object, every part and fitting of it, with any hole cut through it left empty.\nLayer 2: The soft shadow beneath the object, if any.\nLayer 3: The plain background.\n";

#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Calls {
    design: f32,
    layer: f32,
    cost: f64,
}

impl Calls {
    fn logged(out: &Path) -> Option<(Calls, [bool; 2])> {
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
        Some((calls, seen))
    }

    fn read(out: &Path) -> Option<Calls> {
        let (calls, seen) = Self::logged(out)?;
        (seen == [true, true]
            && out.join("design.png").exists()
            && out.join("layer-1.png").exists())
        .then_some(calls)
    }
}

pub(super) fn generate(art: &Art, caption: &Path, out: &Path) -> Result<RgbaImage, Refusal> {
    let failed = |e: std::io::Error| Refusal::Failed(format!("{}: {e}", out.display()));
    std::fs::create_dir_all(out).map_err(failed)?;
    let plan = out.join("plan.txt");
    std::fs::write(&plan, PLAN).map_err(failed)?;
    let design = out.join("design.png");
    let log = out.join("calls.tsv");
    ming_sh(
        art,
        &["design".as_ref(), design.as_ref(), caption.as_ref()],
        &log,
    )?;
    ming_sh(
        art,
        &[
            "design-layer".as_ref(),
            out.as_ref(),
            design.as_ref(),
            plan.as_ref(),
        ],
        &log,
    )?;
    Calls::read(out).ok_or_else(|| Refusal::Failed(format!("{}: incomplete", out.display())))?;
    matte(out).map_err(Refusal::Failed)
}

fn ming_sh(art: &Art, args: &[&std::ffi::OsStr], log: &Path) -> Result<(), Refusal> {
    use std::io::Write;
    let script = art.ming_sh();
    let output = std::process::Command::new(&script)
        .args(args)
        .env(
            "ZIRAL_GENERATOR_STOP",
            art.dir
                .join(format!(".generation-stop-{}", std::process::id())),
        )
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(|e| Refusal::Failed(format!("{}: {e}", script.display())))?;
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log)
        .and_then(|mut file| file.write_all(&output.stdout))
        .map_err(|e| {
            let why = format!(
                "{}: {e}; {}",
                log.display(),
                String::from_utf8_lossy(&output.stderr)
            );
            if output.status.code() == Some(3) {
                Refusal::Spent(why)
            } else {
                Refusal::Failed(why)
            }
        })?;
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
        Some(0) => Ok(()),
        Some(3) => Err(Refusal::Spent(last)),
        _ => Err(Refusal::Failed(last)),
    }
}

fn matte(out: &Path) -> Result<RgbaImage, String> {
    let design = image::open(out.join("design.png"))
        .map_err(|e| format!("design: {e}"))?
        .into_rgba8();
    let (w, h) = design.dimensions();
    if w != h {
        return Err(format!("the design is {w}x{h}, not square"));
    }
    if !out.join("layer-2.png").exists() {
        return Err("the split returned one layer".to_string());
    }
    let object = image::open(out.join("layer-1.png"))
        .map_err(|e| format!("object: {e}"))?
        .into_rgba8();
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
    if ow != oh {
        return Err(format!("the object layer is {ow}x{oh}, not square"));
    }
    let mask = image::imageops::resize(
        &image::GrayImage::from_fn(ow, oh, |x, y| image::Luma([object.get_pixel(x, y)[3]])),
        w,
        h,
        image::imageops::FilterType::CatmullRom,
    );
    Ok(RgbaImage::from_fn(w, h, |x, y| {
        let a = f32::from(mask.get_pixel(x, y)[0]) / 255.0;
        rgba(rgb(design.get_pixel(x, y)), a * f32::from(a >= NOISE))
    }))
}

#[cfg(test)]
mod tests {
    use super::super::save;
    use super::*;
    use bevy::math::Vec2;
    use image::Rgba;
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
        drawn(
            &root,
            &sprite,
            Some(RgbaImage::from_pixel(125, 125, GROUND)),
        );
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
}
