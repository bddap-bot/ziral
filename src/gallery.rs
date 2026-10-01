#![cfg(not(target_arch = "wasm32"))]

use super::*;
use bevy::render::view::window::screenshot::{Screenshot, ScreenshotCaptured};
use image::{ImageEncoder, RgbaImage, imageops};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize)]
struct Picture {
    name: String,
    atom: Option<usize>,
    rect: [u32; 4],
}

struct Scene {
    name: String,
    world: World,
    frames: u32,
}

fn scenes() -> Vec<Scene> {
    let empty = || {
        let mut world = World::new(Sim::empty());
        world.period = f32::INFINITY;
        world.pointer = None;
        world
    };
    let mut scenes = vec![Scene {
        name: "palette".into(),
        world: empty(),
        frames: 2,
    }];
    let mut glazes = empty();
    glazes.sim = sim::fixture(Machine::Glyph(GlyphKind::Bonder)).sim;
    glazes.sim.step();
    let materials = glazes.sim.clone();
    glazes.sim = Sim::empty();
    glazes.sim.place(&materials, Hex::new(0, 2));
    glazes.prev = glazes.sim.clone();
    scenes.push(Scene {
        name: "glazes".into(),
        world: glazes,
        frames: 2,
    });
    let mut atoms = empty();
    for (i, kind) in sim::AtomKind::ALL.into_iter().enumerate() {
        atoms.sim.spawn(sim::Atom {
            kind,
            pos: Hex::new(i as i32 * 2 - 2, -1),
        });
    }
    atoms.prev = atoms.sim.clone();
    scenes.push(Scene {
        name: "atoms".into(),
        world: atoms,
        frames: 1,
    });
    let mut hover = empty();
    let machine = Machine::Glyph(GlyphKind::Bonder);
    hover.edit().set_hover(Some(Item::Machine(machine)));
    hover.play = Some(Play::at(machine, 0));
    scenes.push(Scene {
        name: "hover".into(),
        world: hover,
        frames: 1,
    });
    for machine in Machine::ALL {
        let mut world = empty();
        match machine {
            Machine::Portal => world.sim.portals.push(Some(sim::Portal::new(ORIGIN))),
            Machine::Arm(length) => {
                world
                    .sim
                    .arms
                    .push(Arm::new(length, ORIGIN, 0, vec![Instr::Wait]))
            }
            Machine::Glyph(kind) => world.sim.glyphs.push(Some(Glyph::new(kind, ORIGIN, 0))),
        }
        if machine == Machine::Glyph(GlyphKind::Converter(sim::AtomKind::Amber)) {
            world.sim = Sim::empty();
            world.sim.glyphs.push(Some(Glyph::new(
                GlyphKind::Converter(sim::AtomKind::Amber),
                Hex::new(-3, 0),
                0,
            )));
            let input = sim::fixture(machine).sim;
            world.sim.place(&input, ORIGIN);
            let mut output = input;
            output.step();
            world.sim.place(&output, Hex::new(3, 0));
        }
        world.prev = world.sim.clone();
        scenes.push(Scene {
            name: look::name(machine).into(),
            world,
            frames: 1,
        });
    }
    let mut arm = empty();
    arm.period = TICK_MS / 1000.0;
    arm.sim.arms.push(Arm::new(
        ArmLength::One,
        ORIGIN,
        0,
        vec![Instr::Rot(Spin::Ccw), Instr::Wait, Instr::Wait],
    ));
    arm.prev = arm.sim.clone();
    scenes.push(Scene {
        name: "swing".into(),
        world: arm,
        frames: 60,
    });
    for ghost in [false, true] {
        let mut world = empty();
        world.sim = sim::fixture(Machine::Glyph(GlyphKind::Bonder)).sim;
        world.prev = world.sim.clone();
        if ghost {
            world.edit().resim(10);
        } else {
            for _ in 0..10 {
                world.edit().step();
            }
            world.prev = world.sim.clone();
        }
        scenes.push(Scene {
            name: if ghost { "ghost" } else { "live" }.into(),
            world,
            frames: 1,
        });
    }
    scenes
}

fn encoded(image: &RgbaImage) -> Vec<u8> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
    bytes.into_inner()
}

fn digest(bytes: &[u8]) -> String {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .into()
}

#[derive(Resource)]
struct Capture {
    scenes: std::collections::VecDeque<(PathBuf, Scene)>,
    current: Option<(PathBuf, String, u32)>,
    frame: u32,
    pending: u32,
    period: f32,
}

#[derive(Component)]
struct Swatch;

fn advance(
    mut capture: ResMut<Capture>,
    mut world: ResMut<Game>,
    mut commands: Commands,
    swatches: Query<Entity, With<Swatch>>,
    kiln: Res<Kiln>,
    mut images: Query<&mut ImageNode>,
) {
    if capture.current.is_none() {
        if let Some((path, scene)) = capture.scenes.pop_front() {
            for entity in &swatches {
                commands.entity(entity).despawn();
            }
            for mut image in &mut images {
                image.color = Color::WHITE;
            }
            *world = Game::from_world(scene.world);
            capture.period = world.period;
            world.period = f32::INFINITY;
            world.running = false;
            if scene.name == "glazes" {
                for (i, glaze) in Glaze::ALL.into_iter().enumerate() {
                    commands.spawn((
                        Swatch,
                        Mesh2d(kiln.bar.clone()),
                        MeshMaterial2d(kiln.material(glaze).clone()),
                        Transform::from_translation((glaze_position(i)).extend(0.8))
                            .with_scale(Vec3::new(25.0, 20.0, 1.0)),
                    ));
                }
            }
            capture.current = Some((path, scene.name, scene.frames));
            capture.frame = 0;
        } else {
            return;
        }
    }
    capture.frame += 1;
    if capture.frame == 24 {
        world.period = capture.period;
        world.running = true;
        world.since = 0.0;
    }
}

fn capture(
    mut state: ResMut<Capture>,
    target: Res<shot::Shot>,
    mut commands: Commands,
    mut pictures: Query<(&mut ImageNode, &UiGlobalTransform, &ComputedNode, &ChildOf)>,
    rows: Query<&PaletteRow>,
    swatches: Query<Entity, With<Swatch>>,
    mut exit: MessageWriter<AppExit>,
) {
    let Some((path, name, count)) = state.current.clone() else {
        if state.pending == 0 {
            exit.write(AppExit::Success);
        }
        return;
    };
    if state.frame < 24 {
        return;
    }
    let n = state.frame - 24;
    if n >= count {
        state.current = None;
        return;
    }
    if name == "glazes" && n == 1 {
        for entity in &swatches {
            commands.entity(entity).despawn();
        }
    }
    if name == "palette" {
        let mut records = Vec::new();
        for (mut image, transform, node, parent) in &mut pictures {
            let Ok(row) = rows.get(parent.parent()) else {
                continue;
            };
            if n == 1 {
                image.color = Color::NONE;
            }
            let size = node.size();
            let corner = transform.translation - size / 2.0;
            assert!(
                corner.cmpge(Vec2::ZERO).all()
                    && (corner + size).cmple(shot::SHOT_PX.as_vec2()).all(),
                "{:?} is clipped",
                row.0
            );
            records.push(Picture {
                name: format!("{:?}", row.0),
                atom: if let Item::Atom(kind) = row.0 {
                    Some(atom_index(kind))
                } else {
                    None
                },
                rect: [
                    corner.x.round() as u32,
                    corner.y.round() as u32,
                    size.x.round() as u32,
                    size.y.round() as u32,
                ],
            });
        }
        records.sort_by(|a, b| a.name.cmp(&b.name));
        std::fs::write(
            path.join("pictures.json"),
            serde_json::to_vec(&records).unwrap(),
        )
        .unwrap();
    }
    let file = path.join(format!("{n:05}.png"));
    state.pending += 1;
    commands
        .spawn(Screenshot::image(shot::target(target)))
        .observe(
            move |shot: On<ScreenshotCaptured>, mut state: ResMut<Capture>| {
                shot.image
                    .clone()
                    .try_into_dynamic()
                    .unwrap()
                    .save(&file)
                    .unwrap();
                state.pending -= 1;
            },
        );
}

fn render(root: &Path) -> Vec<(String, Vec<RgbaImage>)> {
    let mut queue = std::collections::VecDeque::new();
    let mut names = Vec::new();
    for pass in ["first", "second"] {
        for scene in scenes() {
            let path = root.join(pass).join(&scene.name);
            std::fs::create_dir_all(&path).unwrap();
            if pass == "first" {
                names.push((scene.name.clone(), scene.frames));
            }
            queue.push_back((path, scene));
        }
    }
    let mut app = shot::gallery(World::new(Sim::empty()));
    lit_plugin(&mut app);
    app.insert_resource(Capture {
        scenes: queue,
        current: None,
        frame: 0,
        pending: 0,
        period: f32::INFINITY,
    })
    .add_systems(Update, advance.before(run_ticks))
    .add_systems(Last, capture);
    assert_eq!(app.run(), AppExit::Success);
    let load = |pass: &str| {
        names
            .iter()
            .map(|(name, count)| {
                let frames: Vec<RgbaImage> = (0..*count)
                    .map(|i| {
                        image::open(root.join(pass).join(name).join(format!("{i:05}.png")))
                            .unwrap()
                            .into_rgba8()
                    })
                    .collect();
                (name.clone(), frames)
            })
            .collect::<Vec<_>>()
    };
    let first = load("first");
    let second = load("second");
    for ((name, a), (_, b)) in first.iter().zip(&second) {
        assert_eq!(a.len(), b.len());
        for (i, (a, b)) in a.iter().zip(b).enumerate() {
            assert!(
                a == b,
                "headless gallery is nondeterministic: {name} frame {i}"
            );
        }
    }
    first
}

fn canonical(frames: &[(String, Vec<RgbaImage>)]) -> Vec<(String, RgbaImage)> {
    let left = (PALETTE_WIDTH + 16.0).ceil() as u32;
    let width = shot::SHOT_PX.x - left;
    frames
        .iter()
        .filter(|(name, _)| name != "live")
        .map(|(name, images)| {
            let image = match name.as_str() {
                "ghost" => {
                    let mut pair = RgbaImage::new(width * 2, shot::SHOT_PX.y);
                    let live =
                        imageops::crop_imm(frame(frames, "live"), left, 0, width, shot::SHOT_PX.y)
                            .to_image();
                    let ghost =
                        imageops::crop_imm(&images[0], left, 0, width, shot::SHOT_PX.y).to_image();
                    imageops::replace(&mut pair, &live, 0, 0);
                    imageops::replace(&mut pair, &ghost, width as i64, 0);
                    pair
                }
                "swing" => {
                    let center = screen(ORIGIN).as_uvec2();
                    let side = 180;
                    let columns = 8;
                    let mut strip = RgbaImage::new(
                        side * columns,
                        side * images.len().div_ceil(columns as usize) as u32,
                    );
                    for (i, image) in images.iter().enumerate() {
                        let crop =
                            imageops::crop_imm(image, center.x - 30, center.y - 140, side, side)
                                .to_image();
                        imageops::replace(
                            &mut strip,
                            &crop,
                            (i % columns as usize * side as usize) as i64,
                            (i / columns as usize * side as usize) as i64,
                        );
                    }
                    strip
                }
                "palette" | "hover" => images[0].clone(),
                "glazes" => {
                    imageops::crop_imm(&images[1], left, 0, width, shot::SHOT_PX.y).to_image()
                }
                _ => imageops::crop_imm(&images[0], left, 0, width, shot::SHOT_PX.y).to_image(),
            };
            (name.clone(), image)
        })
        .collect()
}

fn artifacts(root: &Path, frames: &[(String, Vec<RgbaImage>)]) -> String {
    let images = canonical(frames);
    let width = images.iter().map(|(_, i)| i.width()).max().unwrap();
    let mut positions = Vec::new();
    let (mut x, mut y, mut row_height) = (0, 0, 0);
    for (_, image) in &images {
        if x + image.width() > width {
            x = 0;
            y += row_height;
            row_height = 0;
        }
        positions.push((x, y));
        x += image.width();
        row_height = row_height.max(image.height());
    }
    let mut sheet = RgbaImage::new(width, y + row_height);
    let mut hashes = String::new();
    for ((name, image), (x, y)) in images.into_iter().zip(positions) {
        let bytes = encoded(&image);
        std::fs::write(root.join(format!("{name}.png")), &bytes).unwrap();
        hashes += &format!("{name} {}\n", digest(&bytes));
        imageops::replace(&mut sheet, &image, x as i64, y as i64);
    }
    image::codecs::png::PngEncoder::new_with_quality(
        std::io::BufWriter::new(std::fs::File::create(root.join("sheet.png")).unwrap()),
        image::codecs::png::CompressionType::Fast,
        image::codecs::png::FilterType::NoFilter,
    )
    .write_image(
        &sheet,
        sheet.width(),
        sheet.height(),
        image::ExtendedColorType::Rgba8,
    )
    .unwrap();
    std::fs::write(root.join("hashes.txt"), &hashes).unwrap();
    hashes
}

fn run(root: &Path) -> String {
    let frames = render(root);
    let hashes = artifacts(root, &frames);
    contrast(&root.join("first"), &frames);
    atoms(&root.join("first"), &frames);
    glazes(&frames);
    swing(&frames);
    std::fs::write(root.join("passed"), &hashes).unwrap();
    hashes
}

pub fn command(args: &[String]) -> bool {
    let [_, flag, path] = args else { return false };
    if flag != "--gallery" {
        return false;
    }
    let root = PathBuf::from(path);
    assert!(
        root.starts_with(".gallery")
            && !root
                .components()
                .any(|part| part == std::path::Component::ParentDir),
        "gallery output belongs under ignored .gallery/"
    );
    assert!(!root.exists(), "gallery output must be a new directory");
    run(&root);
    println!("{}", root.join("sheet.png").display());
    true
}

fn frame<'a>(frames: &'a [(String, Vec<RgbaImage>)], name: &str) -> &'a RgbaImage {
    &frames.iter().find(|(key, _)| key == name).unwrap().1[0]
}

fn screen(cell: Hex) -> Vec2 {
    project(px(cell))
}

fn glaze_position(index: usize) -> Vec2 {
    px(FOCUS) + Vec2::new(index as f32 * 25.0 - 75.0, 0.0)
}

fn project(position: Vec2) -> Vec2 {
    shot::SHOT_PX.as_vec2() / 2.0 + (position - px(FOCUS)) * Vec2::new(1.0, -1.0) / MICRO_SCALE
}

fn rgb(pixel: &image::Rgba<u8>) -> Vec3 {
    Vec3::new(pixel[0] as f32, pixel[1] as f32, pixel[2] as f32) / 255.0
}

fn luminance(pixel: &image::Rgba<u8>) -> f32 {
    let linear = |value: u8| {
        let value = value as f32 / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(pixel[0]) + 0.7152 * linear(pixel[1]) + 0.0722 * linear(pixel[2])
}

fn contrast(root: &Path, frames: &[(String, Vec<RgbaImage>)]) {
    let image = frame(frames, "palette");
    let background = &frames.iter().find(|(name, _)| name == "palette").unwrap().1[1];
    let pictures: Vec<Picture> =
        serde_json::from_slice(&std::fs::read(root.join("palette/pictures.json")).unwrap())
            .unwrap();
    assert_eq!(
        pictures.len(),
        palette().count(),
        "every palette picture must be measured"
    );
    for picture in pictures {
        let [x, y, width, height] = picture.rect;
        assert!(
            width > 0 && height > 0 && x + width <= image.width() && y + height <= image.height(),
            "{} is clipped",
            picture.name
        );
        let mut ratios = Vec::new();
        for py in y..y + height {
            for px in x..x + width {
                let ink = image.get_pixel(px, py);
                let plate = background.get_pixel(px, py);
                if rgb(ink).distance(rgb(plate)) < 0.04 {
                    continue;
                }
                let ink = luminance(ink);
                let plate = luminance(plate);
                ratios.push((ink.max(plate) + 0.05) / (ink.min(plate) + 0.05));
            }
        }
        assert!(
            ratios.len() >= 16,
            "{} has no rendered foreground",
            picture.name
        );
        ratios.sort_by(f32::total_cmp);
        let ratio = ratios[ratios.len() * 3 / 4];
        assert!(
            ratio >= 1.5,
            "{} rendered picture/plate contrast {ratio:.3} < 1.5",
            picture.name
        );
    }
}

fn disc(mask: &[bool], width: usize, name: &str) -> Vec<bool> {
    let points: Vec<_> = mask
        .iter()
        .enumerate()
        .filter(|(_, on)| **on)
        .map(|(i, _)| (i % width, i / width))
        .collect();
    assert!(!points.is_empty(), "{name}: empty silhouette");
    let x0 = points.iter().map(|p| p.0).min().unwrap();
    let x1 = points.iter().map(|p| p.0).max().unwrap();
    let y0 = points.iter().map(|p| p.1).min().unwrap();
    let y1 = points.iter().map(|p| p.1).max().unwrap();
    let w = x1 - x0 + 1;
    let h = y1 - y0 + 1;
    let aspect = w as f32 / h as f32;
    assert!(
        (0.95..=1.05).contains(&aspect),
        "{name}: stretched silhouette {aspect}"
    );
    let mut normalized = Vec::new();
    let mut mismatch = 0;
    for y in 0..64 {
        for x in 0..64 {
            let on = mask[(y0 + y * h / 64) * width + x0 + x * w / 64];
            let ideal = (x as f32 + 0.5 - 32.0).powi(2) + (y as f32 + 0.5 - 32.0).powi(2)
                <= 32.0f32.powi(2);
            mismatch += usize::from(on != ideal);
            normalized.push(on);
        }
    }
    let circularity = 1.0 - mismatch as f32 / 4096.0;
    assert!(
        circularity >= 0.94,
        "{name}: alpha-mask circularity {circularity:.3} < 0.94"
    );
    normalized
}

fn atoms(root: &Path, frames: &[(String, Vec<RgbaImage>)]) {
    let world = frame(frames, "atoms");
    let background = frame(frames, "palette");
    let pictures: Vec<Picture> =
        serde_json::from_slice(&std::fs::read(root.join("palette/pictures.json")).unwrap())
            .unwrap();
    let palette = frame(frames, "palette");
    let blank = &frames.iter().find(|(name, _)| name == "palette").unwrap().1[1];
    let mut colors = Vec::new();
    for (i, kind) in sim::AtomKind::ALL.into_iter().enumerate() {
        let picture = pictures.iter().find(|p| p.atom == Some(i)).unwrap();
        let [x, y, width, height] = picture.rect;
        let mut mask = Vec::new();
        for py in y..y + height {
            for px in x..x + width {
                mask.push(
                    rgb(palette.get_pixel(px, py)).distance(rgb(blank.get_pixel(px, py))) > 0.04,
                );
            }
        }
        let inventory_area = mask.iter().filter(|on| **on).count() as f32;
        let preview = disc(&mask, width as usize, &format!("{kind:?} inventory"));
        let center = screen(Hex::new(i as i32 * 2 - 2, -1));
        let radius = (ATOM_RADIUS / MICRO_SCALE + 3.0).ceil() as i32;
        let mut mask = Vec::new();
        let mut color = Vec3::ZERO;
        let mut count = 0;
        for y in -radius..=radius {
            for x in -radius..=radius {
                let p = (center + Vec2::new(x as f32, y as f32)).round().as_uvec2();
                mask.push(
                    rgb(world.get_pixel(p.x, p.y)).distance(rgb(background.get_pixel(p.x, p.y)))
                        > 0.04,
                );
                if x * x + y * y < radius * radius / 4 {
                    color += rgb(world.get_pixel(p.x, p.y));
                    count += 1;
                }
            }
        }
        let world_area = mask.iter().filter(|on| **on).count() as f32;
        let scale = world_area * MICRO_SCALE.powi(2) / inventory_area;
        assert!(
            (0.8..=1.2).contains(&scale),
            "{kind:?}: inventory/world silhouette size ratio {scale}"
        );
        let actual = disc(&mask, (radius * 2 + 1) as usize, &format!("{kind:?} world"));
        let overlap = preview.iter().zip(&actual).filter(|(a, b)| a == b).count() as f32 / 4096.0;
        assert!(
            overlap >= 0.94,
            "{kind:?}: inventory/world silhouette agreement {overlap:.3}"
        );
        let color = color / count as f32;
        let hsv = Hsva::from(Color::srgb(color.x, color.y, color.z));
        let chroma = hsv.saturation * hsv.value;
        let expected = Hsva::from(look::atom(kind).glaze.color());
        if kind == sim::AtomKind::Amber {
            assert!(
                expected.saturation * expected.value - chroma <= 0.15,
                "{kind:?}: rendered chroma is washed out"
            );
        }
        colors.push((kind, Vec2::from_angle(hsv.hue.to_radians()) * chroma));
    }
    for (i, (kind, color)) in colors.iter().enumerate() {
        for (other, other_color) in &colors[i + 1..] {
            let gap = color.distance(*other_color);
            assert!(
                gap >= 0.12,
                "{kind:?}/{other:?}: rendered hue/chroma distance {gap:.3} < 0.12"
            );
        }
    }
}

fn swing(frames: &[(String, Vec<RgbaImage>)]) {
    let background = frame(frames, "palette");
    let frames = &frames.iter().find(|(name, _)| name == "swing").unwrap().1;
    let center = screen(ORIGIN);
    let mut angles = Vec::new();
    for image in frames {
        let mut sum = Vec2::ZERO;
        let mut count = 0;
        for y in 0..image.height() {
            for x in 260..image.width() {
                let d = Vec2::new(x as f32, y as f32) - center;
                if !(35.0..110.0).contains(&d.length()) {
                    continue;
                }
                if rgb(image.get_pixel(x, y)).distance(rgb(background.get_pixel(x, y))) > 0.04 {
                    sum += d;
                    count += 1;
                }
            }
        }
        assert!(count > 10, "arm has no readable rendered link");
        angles.push((sum / count as f32).to_angle());
    }
    let target = -spin_angle(Spin::Ccw);
    let settle = &angles[45..];
    assert!(
        (settle.last().unwrap() - target).abs() < 0.08,
        "rendered arm misses target: {settle:?}, target {target}"
    );
    assert!(
        settle.windows(2).all(|pair| pair[1] <= pair[0] + 0.005),
        "rendered arm does not settle monotonically: {settle:?}"
    );
    assert!(
        (angles[0] - angles.last().unwrap()).abs() > 0.8,
        "arm strip omits the swing"
    );
}

fn glazes(frames: &[(String, Vec<RgbaImage>)]) {
    let image = frame(frames, "glazes");
    let colors: Vec<_> = Glaze::ALL
        .into_iter()
        .enumerate()
        .map(|(i, glaze)| {
            let at = project(glaze_position(i)).as_uvec2();
            let p = rgb(image.get_pixel(at.x, at.y));
            let hsv = Hsva::from(Color::srgb(p.x, p.y, p.z));
            (
                glaze,
                Vec2::from_angle(hsv.hue.to_radians()) * hsv.saturation * hsv.value,
            )
        })
        .collect();
    for (i, (a, x)) in colors.iter().enumerate() {
        for (b, y) in &colors[i + 1..] {
            assert!(
                x.distance(*y) >= 0.03,
                "{a:?}/{b:?}: rendered glaze hue/chroma distance {} < 0.03",
                x.distance(*y)
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendered_gallery_is_deterministic_and_matches_reviewed_hashes() {
        let _lock = render_test::lock();
        let root = Path::new(".gallery/test");
        if root.exists() {
            std::fs::remove_dir_all(root).unwrap();
        }
        let current = run(root);
        assert_eq!(
            current,
            std::fs::read_to_string("gallery/hashes.txt").unwrap_or_default(),
            "rendered frames changed; run gallery/critic.sh and commit approved hashes"
        );
    }
}
