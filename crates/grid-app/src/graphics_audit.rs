//! Graphics audit of one route (`--graphics-audit DIR`, docs/GRAPHICS_AUDIT.md):
//! black or missing textures, voids and wrong-material surfaces.
//!
//! Two parts, both in the normal offscreen render of the playable preset:
//! - at load, `materials.tsv`: every drawn track, route, land and object
//!   material with its original shader group, its diffuse texture and that
//!   texture's measured mean luminance and alpha (a small mip level decoded on
//!   the CPU), flagged when the diffuse is unbound or missing, nearly black,
//!   nearly fully cut out by the alpha mask, or drawn without UVs;
//! - after the normal warm-up, a camera sweep along the route's gate line
//!   (`progress_track.xml`): at N points a chase view down the road and a view
//!   to each side. Each frame is measured in process (8x8 blocks): the largest
//!   connected near-black region and the largest region where nothing was
//!   drawn (the clear colour is set to a magenta sentinel in this mode). The
//!   frames go to `frames.tsv`, flagged frames to `flag-NNN.png` (every frame,
//!   the others as `frame-NNN.png`, with `GRID_GRAPHICS_AUDIT_FRAMES=all`), and every
//!   frame to a thumbnail sheet `sheet.png` (rows = points, columns = views).
//!
//! One summary line `graphics_audit verdict=...` ends the run; the final
//! `--render` PNG is taken after the sweep.
//!
//! `GRID_GRAPHICS_AUDIT_RAYS=FILE` (lines `eye look px py`) prints, per line,
//! the loaded draws and the collision that the ray through that pixel hits.
use bevy::{
    asset::RenderAssetUsages,
    image::{CompressedImageFormats, ImageSampler, ImageType},
    prelude::*,
    render::{
        render_resource::{Extent3d, TextureDimension, TextureFormat},
        view::screenshot::{Screenshot, ScreenshotCaptured},
    },
};
use grid_formats::{material, pssg};
use grid_tools::route::Route;
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

/// Frames a pose is held before its screenshot: ornament LOD fades (0.4 s),
/// shadow cascades and temporal filters settle.
const SETTLE: u32 = 40;
/// Block side for the frame measures, in pixels.
const BLOCK: usize = 8;
/// A pixel is near-black below this channel maximum (sRGB bytes). The
/// defects found so far were all but pure black (the F738 void 1, a camera
/// inside a Milan building 0.9), while authored deep shade (Milan's arcade
/// floor, forest interiors) sits near 13; 12 flagged that shade.
const BLACK: u8 = 6;
/// A frame is flagged when its largest near-black region covers this
/// fraction of the frame, or its largest undrawn region `VOID_FLAG`.
const BLACK_FLAG: f32 = 0.01;
const VOID_FLAG: f32 = 0.002;
/// Thumbnail size on the sheet.
const THUMB: (usize, usize) = (256, 144);
/// The views at each point: name and yaw from the road direction (degrees).
const VIEWS: [(&str, f32); 3] = [("chase", 0.), ("left", 75.), ("right", -75.)];

#[derive(Clone)]
pub struct Config {
    pub directory: PathBuf,
    pub points: usize,
}

/// Shader groups the port refers to by name (object_materials.rs, water.rs,
/// track_decals.rs, light_shafts.rs, the track surface paths). Every other
/// group is drawn by the generic diffuse path; `materials.tsv` marks those.
const NAMED_GROUPS: [&str; 8] = [
    "decal", "glass_simple", "god_rays", "object_simple", "object_simple_mesh",
    "track_infield", "track_surface", "water",
];

/// Whether the port draws a material's emissive map: the start-light board
/// (main.rs `StartLights`) and the groups object_materials::emissive_materials
/// reads (night ones scaled by `EmissiveTime`, dark by day as in the
/// original); `object_emissive_ripple` is not drawn (its `EmissiveTime` is
/// never set, docs/GRAPHICS_AUDIT.md).
fn emissive_drawn(name: &str, group: &str) -> bool {
    name.rsplit('/').next() == Some("start_lights")
        || ["object_simple_emissive", "object_lod_emissive", "object_animated_emissive", "object_simple_night_emissive"]
            .iter()
            .any(|g| group.starts_with(g))
}

struct Stats {
    format: String,
    size: (u32, u32),
    luminance: f32,
    alpha: f32,
    cut: f32,
}

/// Mean luminance, mean alpha and the fraction of alpha below one half of a
/// small mip level (about 32 texels wide) of a DDS diffuse.
fn texture_stats(dds: &[u8]) -> Result<Stats, String> {
    let image = Image::from_buffer(
        dds,
        ImageType::Extension("dds"),
        CompressedImageFormats::BC,
        true,
        ImageSampler::Default,
        RenderAssetUsages::MAIN_WORLD,
    )
    .map_err(|e| format!("{e:?}"))?;
    let d = &image.texture_descriptor;
    let (w, h, levels) = (d.size.width, d.size.height, d.mip_level_count.max(1));
    let format = format!("{:?}", d.format);
    let data = image.data.as_deref().ok_or("no data")?;
    let (block, alpha) = match d.format {
        TextureFormat::Bc1RgbaUnormSrgb | TextureFormat::Bc1RgbaUnorm => (Some(8), false),
        TextureFormat::Bc3RgbaUnormSrgb | TextureFormat::Bc3RgbaUnorm => (Some(16), true),
        TextureFormat::Rgba8UnormSrgb | TextureFormat::Rgba8Unorm
        | TextureFormat::Bgra8UnormSrgb | TextureFormat::Bgra8Unorm => (None, true),
        _ => return Err(format),
    };
    let level_size = |k: u32| -> (usize, usize) {
        (((w >> k).max(1)) as usize, ((h >> k).max(1)) as usize)
    };
    let bytes = |k: u32| -> usize {
        let (lw, lh) = level_size(k);
        match block {
            Some(b) => lw.div_ceil(4) * lh.div_ceil(4) * b,
            None => lw * lh * 4,
        }
    };
    let mut level = 0;
    while level + 1 < levels && level_size(level).0 > 256 {
        level += 1;
    }
    let offset: usize = (0..level).map(bytes).sum();
    let (lw, lh) = level_size(level);
    let slice = data.get(offset..offset + bytes(level)).ok_or("truncated")?;
    let rgba = match block {
        Some(_) => super::texture_upload::decode_levels(slice, lw as u32, lh as u32, 1, alpha)
            .ok_or("bad BC level")?,
        None => slice.to_vec(),
    };
    let n = (lw * lh) as f32;
    let (mut lum, mut a, mut cut) = (0f32, 0f32, 0f32);
    for t in rgba.as_chunks::<4>().0 {
        lum += (0.2126 * t[0] as f32 + 0.7152 * t[1] as f32 + 0.0722 * t[2] as f32) / 255.;
        a += t[3] as f32 / 255.;
        cut += f32::from(u8::from(t[3] < 128));
    }
    Ok(Stats { format, size: (w, h), luminance: lum / n, alpha: a / n, cut: cut / n })
}

fn kind(material: &str) -> &'static str {
    match material.split_once('/') {
        Some(("route", _)) => "route",
        Some(("land", _)) => "land",
        Some(("objects", _)) => "object",
        Some(("trees", _)) => "tree",
        _ => "track",
    }
}

/// `GRID_GRAPHICS_AUDIT_RAYS=FILE` (docs/GRAPHICS_AUDIT.md): for a flagged
/// region, which loaded parts lie under a pixel. Each line of FILE is
/// `eye look px py` (the frame's `eye` and `look` columns, a pixel of the
/// 16:9 frame at Bevy's default 45-degree vertical field of view); one line
/// `graphics_audit_ray` per input with the nearest hits (part, material,
/// distance) of every loaded draw instance, whether or not it is shown at
/// that distance. No hit: nothing was loaded there; a hit: the part is loaded
/// and something at run time (draw distance, ornament level) left it out.
pub(crate) fn ray_report(
    root: &Path,
    route: &Route,
    parts: &[super::Part],
    file: &Path,
    width: u32,
    height: u32,
) -> Result<(), Box<dyn std::error::Error>> {
    let track = grid_tools::track::Track::read(&std::fs::read(root.join(route.file("track.jpk")))?)?;
    let vec = |s: &str| -> Result<Vec3, Box<dyn std::error::Error>> {
        let v: Vec<f32> = s.split(',').map(str::parse).collect::<Result<_, _>>()?;
        Ok(Vec3::new(*v.first().ok_or("x")?, *v.get(1).ok_or("y")?, *v.get(2).ok_or("z")?))
    };
    for line in std::fs::read_to_string(file)?.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<_> = line.split_whitespace().collect();
        let [eye, look, px, py] = f[..] else { return Err(format!("ray line {line:?}: eye look px py").into()) };
        let (eye, look, px, py) = (vec(eye)?, vec(look)?, px.parse::<f32>()?, py.parse::<f32>()?);
        let view = Transform::from_translation(eye).looking_at(look, Vec3::Y);
        let tan = (std::f32::consts::FRAC_PI_4 / 2.).tan();
        let x = ((px + 0.5) / width as f32 * 2. - 1.) * tan * width as f32 / height as f32;
        let y = (1. - (py + 0.5) / height as f32 * 2.) * tan;
        let dir = (view.rotation * Vec3::new(x, y, -1.)).normalize();
        let mut hits = Vec::new();
        for part in parts {
            let Some(bevy::mesh::VertexAttributeValues::Float32x3(positions)) = part.mesh.attribute(Mesh::ATTRIBUTE_POSITION) else { continue };
            let indices: Vec<usize> = match part.mesh.indices() {
                Some(i) => i.iter().collect(),
                None => (0..positions.len()).collect(),
            };
            for matrix in &part.matrices {
                let p: Vec<Vec3> = positions.iter().map(|v| matrix.transform_point3(Vec3::from(*v))).collect();
                let best = indices
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .filter_map(|t| triangle_hit(eye, dir, p[t[0]], p[t[1]], p[t[2]]))
                    .fold(f32::INFINITY, f32::min);
                if best.is_finite() {
                    hits.push((best, part.name.as_str(), part.material.as_str()));
                }
            }
        }
        hits.sort_by(|a, b| a.0.total_cmp(&b.0));
        // The collision along the same ray (1 km): the road or ground the
        // original's physics has there.
        let end = eye + dir * 1000.;
        let mut cache = grid_world::rectangle_cull::Cache::default();
        let collision = track
            .query([eye.x, eye.y, eye.z, 1.], [end.x, end.y, end.z, 1.], &mut cache)
            .map_err(|e| e.0)?
            .hit
            .map_or("-".to_owned(), |h| {
                let at = eye + dir * 1000. * h.contact.fraction;
                format!("{:.1}@{:.1},{:.1},{:.1}", 1000. * h.contact.fraction, at.x, at.y, at.z)
            });
        let list: Vec<_> = hits.iter().take(4).map(|(d, n, m)| format!("{n}|{m}@{d:.1}")).collect();
        println!("graphics_audit_ray eye={eye} px={px},{py} dir={dir:.3} collision={collision} hits={} nearest={}", hits.len(), if list.is_empty() { "-".into() } else { list.join(" ") });
    }
    Ok(())
}

/// Distance along `dir` to the triangle (both faces), Moller-Trumbore.
fn triangle_hit(origin: Vec3, dir: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<f32> {
    let (e1, e2) = (b - a, c - a);
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-9 {
        return None;
    }
    let s = origin - a;
    let u = s.dot(p) / det;
    let q = s.cross(e1);
    let v = dir.dot(q) / det;
    let t = e2.dot(q) / det;
    (u >= 0. && v >= 0. && u + v <= 1. && t > 0.).then_some(t)
}

/// Original shader group of each material, keyed like the loaded parts, and
/// its emissive map with the saved `EmissiveMapScale` (0 when unsaved).
fn shader_groups(root: &Path, route: &Route) -> BTreeMap<String, (String, Option<(String, f32)>)> {
    let mut groups = BTreeMap::new();
    for (prefix, file) in [
        ("", route.track_file("tracksplit.pssg")),
        ("route/", route.file("routesplit.pssg")),
        ("land/", route.track_file("land.pssg")),
        ("objects/", route.track_file("objects.pssg")),
    ] {
        let Ok(bytes) = std::fs::read(root.join(&file)) else { continue };
        let Ok(tree) = pssg::read_tree(&bytes) else { continue };
        let Ok(constants) = material::read_constants(&tree) else { continue };
        let bindings = material::read_texture_bindings(&tree).unwrap_or_default();
        for s in constants {
            let emissive = bindings
                .iter()
                .find(|b| b.shader == s.shader && b.sampler == b"TEmissiveMap")
                .map(|b| {
                    let scale = s.values.get(&b"EmissiveMapScale"[..]).and_then(|v| v.first().copied());
                    (String::from_utf8_lossy(b.texture).into_owned(), scale.unwrap_or(0.))
                });
            groups.insert(
                format!("{prefix}{}", String::from_utf8_lossy(s.shader)),
                (String::from_utf8_lossy(s.group).trim_end_matches(".fx").to_owned(), emissive),
            );
        }
    }
    groups
}

/// Writes `materials.tsv` and prints its summary line.
pub(crate) fn materials_report(
    root: &Path,
    route: &Route,
    parts: &[super::Part],
    bindings: &BTreeMap<String, String>,
    textures: &BTreeMap<String, Vec<u8>>,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(&config.directory)?;
    let groups = shader_groups(root, route);
    // Draws and draws without UVs per material.
    let mut draws: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for part in parts {
        if part.material.starts_with("car") || part.material == "batchmaterial" {
            continue;
        }
        let entry = draws.entry(&part.material).or_default();
        entry.0 += 1;
        entry.1 += usize::from(part.mesh.attribute(Mesh::ATTRIBUTE_UV_0).is_none());
    }
    let mut out = String::from(
        "material\tkind\tgroup\tdraws\tno_uv\ttexture\temissive\tformat\tsize\tluminance\talpha\tcut\tflags\n",
    );
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    let mut cache: BTreeMap<&str, Result<Stats, String>> = BTreeMap::new();
    for (&name, &(n, no_uv)) in &draws {
        let kind = kind(name);
        let (group, emissive) = groups.get(name).map_or(("?", None), |(g, e)| (g.as_str(), e.as_ref()));
        let mut flags = Vec::new();
        if !NAMED_GROUPS.iter().any(|g| group.starts_with(g)) {
            flags.push("unnamed_group");
        }
        let texture = bindings.get(name);
        let stats = match texture {
            None => {
                flags.push("unbound");
                None
            }
            Some(key) => match textures.get(key) {
                None => {
                    flags.push("missing");
                    None
                }
                Some(dds) => Some(cache.entry(key).or_insert_with(|| texture_stats(dds))),
            },
        };
        if no_uv > 0 && texture.is_some() {
            flags.push("no_uv");
        }
        let (format, size, lum, alpha, cut) = match stats {
            Some(Ok(s)) => {
                // Light-shaft cards take their colour from shader constants
                // (light_shafts.rs) and glass is blended, not alpha-tested.
                let shaft = group.starts_with("god_rays");
                // A dark diffuse under a drawn emissive map is a screen or
                // sign: the map is what shows.
                let lit = emissive.is_some() && emissive_drawn(name, group);
                if s.luminance < 0.04 && !shaft && !lit {
                    flags.push("dark");
                    // The original adds an emissive map the port does not draw.
                    if emissive.is_some() {
                        flags.push("dark_unlit_emissive");
                    }
                }
                // Other objects and trees are alpha-tested at one half.
                if matches!(kind, "object" | "tree") && !shaft && group != "glass_simple" && s.cut > 0.9 {
                    flags.push("cut_out");
                }
                (s.format.clone(), format!("{}x{}", s.size.0, s.size.1),
                 format!("{:.3}", s.luminance), format!("{:.3}", s.alpha), format!("{:.3}", s.cut))
            }
            Some(Err(format)) => {
                flags.push("undecoded");
                (format.clone(), "-".into(), "-".into(), "-".into(), "-".into())
            }
            None => ("-".into(), "-".into(), "-".into(), "-".into(), "-".into()),
        };
        for f in &flags {
            *counts.entry(f).or_default() += 1;
        }
        writeln!(
            out,
            "{name}\t{kind}\t{group}\t{n}\t{no_uv}\t{}\t{}\t{format}\t{size}\t{lum}\t{alpha}\t{cut}\t{}",
            texture.map_or("-", String::as_str),
            emissive.map_or("-".into(), |(map, scale)| format!("{map}:{scale}")),
            flags.join(",")
        )?;
    }
    std::fs::write(config.directory.join("materials.tsv"), out)?;
    let summary: Vec<String> = counts.iter().map(|(k, v)| format!("{k}={v}")).collect();
    println!(
        "graphics_audit_materials materials={} {} file={}",
        draws.len(),
        summary.join(" "),
        config.directory.join("materials.tsv").display()
    );
    Ok(())
}

#[derive(Clone)]
struct Pose {
    point: usize,
    view: &'static str,
    distance: f32,
    eye: Vec3,
    look: Vec3,
}

/// Puts each eye its view's height above the collision surface under it,
/// moving the look point with it: a gate centre is not always on the road
/// (crests, dips, wide gates), and an eye under the surface sees the scene
/// from below. `ground(p)` is the surface height under `p`'s column, searched
/// from a little above `p`. Returns how many eyes found a surface.
fn ground_poses(poses: &mut [Pose], ground: impl Fn(Vec3) -> Option<f32>) -> usize {
    let mut grounded = 0;
    for pose in poses {
        let height = if pose.view == "chase" { CHASE_HEIGHT } else { SIDE_HEIGHT };
        // Search from the gate line's height plus a few metres, not from the
        // eye, so a bridge deck above is not taken for the road.
        let from = Vec3::new(pose.eye.x, pose.eye.y - height + 3., pose.eye.z);
        if let Some(y) = ground(from) {
            let shift = y + height - pose.eye.y;
            pose.eye.y += shift;
            pose.look.y += shift;
            grounded += 1;
        }
    }
    grounded
}

const CHASE_HEIGHT: f32 = 2.6;
const SIDE_HEIGHT: f32 = 1.6;

/// Camera poses at the gates nearest `points` equal arc lengths of the gate
/// line. A gate centre lies on the road; a point between two gates can be off
/// it (inside a corner building on a city street).
fn poses(line: &[Vec3], points: usize) -> Vec<Pose> {
    let mut along = vec![0f32];
    for w in line.windows(2) {
        along.push(along.last().copied().unwrap_or(0.) + w[0].distance(w[1]));
    }
    let total = along.last().copied().unwrap_or(0.);
    let mut poses = Vec::new();
    if total <= 0. {
        return poses;
    }
    let mut used = None;
    for point in 0..points {
        let target = (point as f32 + 0.5) / points as f32 * total;
        let k = (0..line.len())
            .min_by(|&a, &b| (along[a] - target).abs().total_cmp(&(along[b] - target).abs()))
            .unwrap_or(0);
        if used == Some(k) {
            continue;
        }
        used = Some(k);
        let p = line[k];
        let mut d = line[(k + 1).min(line.len() - 1)] - line[k.saturating_sub(1)];
        d.y = 0.;
        let d = d.normalize_or(Vec3::Z);
        for (view, yaw) in VIEWS {
            let (eye, look) = if yaw == 0. {
                (p - 7. * d + CHASE_HEIGHT * Vec3::Y, p + 12. * d + 0.8 * Vec3::Y)
            } else {
                let side = Quat::from_rotation_y(yaw.to_radians()) * d;
                (p + SIDE_HEIGHT * Vec3::Y, p + 1.2 * Vec3::Y + 20. * side)
            };
            poses.push(Pose { point, view, distance: along[k], eye, look });
        }
    }
    poses
}

#[derive(Default)]
struct State {
    pending: bool,
    completed: usize,
    flagged: usize,
    failed: bool,
    rows: Vec<String>,
    /// Thumbnail of each completed pose (RGBA8).
    thumbs: Vec<(usize, Vec<u8>)>,
    worst_black: (f32, usize),
    worst_void: (f32, usize),
}

#[derive(Resource)]
pub(crate) struct Sweep {
    config: Config,
    poses: Vec<Pose>,
    next: usize,
    held: u32,
    finished: bool,
    state: Arc<Mutex<State>>,
}

impl Sweep {
    pub(crate) fn finished(&self) -> bool {
        self.finished
    }
    pub(crate) fn failed(&self) -> bool {
        self.state.lock().unwrap().failed
    }
    /// Seconds the sweep may add to the capture budget.
    pub(crate) fn budget_seconds(&self) -> f64 {
        self.poses.len() as f64 * f64::from(SETTLE + 10) / 60. * 3. + 60.
    }
}

pub(crate) fn install(
    app: &mut App,
    root: &Path,
    route: &Route,
    config: Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let line = super::float_audit::gate_line(root, route)?;
    let mut poses = poses(&line, config.points);
    if poses.is_empty() {
        return Err("graphics audit: the route has no gate line".into());
    }
    let track = grid_tools::track::Track::read(&std::fs::read(root.join(route.file("track.jpk")))?)?;
    let grounded = ground_poses(&mut poses, |p| super::float_audit::ground(&track, p).ok().flatten());
    std::fs::create_dir_all(&config.directory)?;
    println!(
        "graphics_audit sweep points={} views={} poses={} grounded={grounded} settle_frames={SETTLE} directory={}",
        config.points,
        VIEWS.len(),
        poses.len(),
        config.directory.display()
    );
    app.insert_resource(Sweep {
        config,
        poses,
        next: 0,
        held: 0,
        finished: false,
        state: default(),
    })
    // Undrawn pixels show the sentinel; the sky and land cover the rest.
    .add_systems(Startup, |mut clear: ResMut<ClearColor>| {
        clear.0 = Color::srgb(1., 0., 1.);
    })
    .add_systems(Update, step);
    Ok(())
}

fn step(
    mut commands: Commands,
    mut sweep: ResMut<Sweep>,
    capture: Res<super::Capture>,
    warmup: Option<Res<super::pipeline_warmup::Warmup>>,
    mut camera: Single<&mut Transform, With<super::ViewerCamera>>,
) {
    if sweep.finished
        || capture.frames < 180
        || warmup.as_ref().is_some_and(|w| !w.ready())
        || sweep.state.lock().unwrap().pending
    {
        return;
    }
    let Some(target) = capture.target.clone() else { return };
    if sweep.next == sweep.poses.len() {
        sweep.finished = true;
        if let Err(error) = finish(&sweep) {
            error!("graphics audit: {error}");
            sweep.state.lock().unwrap().failed = true;
        }
        return;
    }
    let pose = sweep.poses[sweep.next].clone();
    if sweep.held == 0 {
        **camera = Transform::from_translation(pose.eye).looking_at(pose.look, Vec3::Y);
    }
    sweep.held += 1;
    if sweep.held < SETTLE {
        return;
    }
    sweep.held = 0;
    let index = sweep.next;
    sweep.next += 1;
    let state = sweep.state.clone();
    state.lock().unwrap().pending = true;
    let directory = sweep.config.directory.clone();
    commands.spawn(Screenshot::image(target)).observe(
        move |event: On<ScreenshotCaptured>| {
            let mut state = state.lock().unwrap();
            state.pending = false;
            if let Err(error) = measure(&event.image, index, &pose, &directory, &mut state) {
                error!("graphics audit frame {index}: {error}");
                state.failed = true;
            }
            state.completed += 1;
        },
    );
}

/// Largest 4-connected region of set blocks: (blocks, bbox in blocks).
fn largest_region(mask: &[bool], bw: usize, bh: usize) -> (usize, [usize; 4]) {
    let mut seen = vec![false; mask.len()];
    let mut best = (0, [0; 4]);
    let mut stack = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] || seen[start] {
            continue;
        }
        seen[start] = true;
        stack.push(start);
        let (mut n, mut b) = (0, [usize::MAX, usize::MAX, 0, 0]);
        while let Some(i) = stack.pop() {
            n += 1;
            let (x, y) = (i % bw, i / bw);
            b = [b[0].min(x), b[1].min(y), b[2].max(x), b[3].max(y)];
            let mut push = |j: usize| {
                if mask[j] && !seen[j] {
                    seen[j] = true;
                    stack.push(j);
                }
            };
            if x > 0 { push(i - 1) }
            if x + 1 < bw { push(i + 1) }
            if y > 0 { push(i - bw) }
            if y + 1 < bh { push(i + bw) }
        }
        if n > best.0 {
            best = (n, b);
        }
    }
    best
}

fn measure(
    image: &Image,
    index: usize,
    pose: &Pose,
    directory: &Path,
    state: &mut State,
) -> Result<(), String> {
    let (w, h) = (image.width() as usize, image.height() as usize);
    let data = image.data.as_deref().ok_or("no screenshot data")?;
    let bgra = matches!(
        image.texture_descriptor.format,
        TextureFormat::Bgra8UnormSrgb | TextureFormat::Bgra8Unorm
    );
    if data.len() < w * h * 4 {
        return Err("short screenshot".into());
    }
    let rgb = |i: usize| -> [u8; 3] {
        let p = &data[i * 4..i * 4 + 3];
        if bgra { [p[2], p[1], p[0]] } else { [p[0], p[1], p[2]] }
    };
    let (bw, bh) = (w / BLOCK, h / BLOCK);
    let (mut black, mut void) = (vec![false; bw * bh], vec![false; bw * bh]);
    let mut lum = 0f64;
    for by in 0..bh {
        for bx in 0..bw {
            let (mut nb, mut nv) = (0, 0);
            for y in by * BLOCK..(by + 1) * BLOCK {
                for x in bx * BLOCK..(bx + 1) * BLOCK {
                    let [r, g, b] = rgb(y * w + x);
                    lum += 0.2126 * r as f64 + 0.7152 * g as f64 + 0.0722 * b as f64;
                    nb += usize::from(r.max(g).max(b) < BLACK);
                    nv += usize::from(r > 200 && b > 200 && g < 60);
                }
            }
            let full = BLOCK * BLOCK * 3 / 4;
            black[by * bw + bx] = nb >= full;
            void[by * bw + bx] = nv >= full;
        }
    }
    let blocks = (bw * bh).max(1) as f32;
    let black_total = black.iter().filter(|&&b| b).count() as f32 / blocks;
    let void_total = void.iter().filter(|&&b| b).count() as f32 / blocks;
    let (black_n, black_box) = largest_region(&black, bw, bh);
    let (void_n, void_box) = largest_region(&void, bw, bh);
    let (black_blob, void_blob) = (black_n as f32 / blocks, void_n as f32 / blocks);
    let flagged = black_blob >= BLACK_FLAG || void_blob >= VOID_FLAG;
    let px = |b: [usize; 4], n: usize| {
        if n == 0 { "-".to_owned() } else {
            format!("{},{}-{},{}", b[0] * BLOCK, b[1] * BLOCK, (b[2] + 1) * BLOCK, (b[3] + 1) * BLOCK)
        }
    };
    state.rows.push(format!(
        "{index}\t{}\t{}\t{:.0}\t{:.1},{:.1},{:.1}\t{:.1},{:.1},{:.1}\t{:.1}\t{black_total:.4}\t{black_blob:.4}\t{}\t{void_total:.4}\t{void_blob:.4}\t{}\t{}",
        pose.point, pose.view, pose.distance,
        pose.eye.x, pose.eye.y, pose.eye.z, pose.look.x, pose.look.y, pose.look.z,
        lum / (bw * bh * BLOCK * BLOCK).max(1) as f64,
        px(black_box, black_n), px(void_box, void_n),
        if flagged { "FLAG" } else { "ok" }
    ));
    if black_blob > state.worst_black.0 {
        state.worst_black = (black_blob, index);
    }
    if void_blob > state.worst_void.0 {
        state.worst_void = (void_blob, index);
    }
    if flagged {
        state.flagged += 1;
    }
    // GRID_GRAPHICS_AUDIT_FRAMES=all keeps every frame (matched before/after crops).
    if flagged || std::env::var("GRID_GRAPHICS_AUDIT_FRAMES").as_deref() == Ok("all") {
        image
            .clone()
            .try_into_dynamic()
            .map_err(|e| format!("{e:?}"))?
            .to_rgb8()
            .save(directory.join(format!("{}-{index:03}.png", if flagged { "flag" } else { "frame" })))
            .map_err(|e| e.to_string())?;
    }
    // Box-filtered thumbnail.
    let (tw, th) = THUMB;
    let mut thumb = vec![255u8; tw * th * 4];
    for ty in 0..th {
        for tx in 0..tw {
            let (x0, x1) = (tx * w / tw, ((tx + 1) * w / tw).max(tx * w / tw + 1));
            let (y0, y1) = (ty * h / th, ((ty + 1) * h / th).max(ty * h / th + 1));
            let mut sum = [0u32; 3];
            for y in y0..y1 {
                for x in x0..x1 {
                    let c = rgb(y * w + x);
                    for k in 0..3 {
                        sum[k] += u32::from(c[k]);
                    }
                }
            }
            let n = ((x1 - x0) * (y1 - y0)) as u32;
            for k in 0..3 {
                thumb[(ty * tw + tx) * 4 + k] = (sum[k] / n) as u8;
            }
        }
    }
    state.thumbs.push((index, thumb));
    Ok(())
}

fn finish(sweep: &Sweep) -> Result<(), Box<dyn std::error::Error>> {
    let state = sweep.state.lock().unwrap();
    let directory = &sweep.config.directory;
    let mut tsv = String::from(
        "index\tpoint\tview\tdistance\teye\tlook\tmean_luma\tblack_total\tblack_blob\tblack_box\tvoid_total\tvoid_blob\tvoid_box\tverdict\n",
    );
    for row in &state.rows {
        tsv.push_str(row);
        tsv.push('\n');
    }
    std::fs::write(directory.join("frames.tsv"), tsv)?;
    let (tw, th) = THUMB;
    let columns = VIEWS.len();
    let rows = sweep.config.points;
    let (sw, sh) = (columns * tw, rows * th);
    let mut sheet = vec![0u8; sw * sh * 4];
    for (index, thumb) in &state.thumbs {
        let (cx, cy) = (index % columns * tw, index / columns * th);
        for y in 0..th {
            let row = ((cy + y) * sw + cx) * 4;
            sheet[row..row + tw * 4].copy_from_slice(&thumb[y * tw * 4..(y + 1) * tw * 4]);
        }
    }
    let sheet = Image::new(
        Extent3d { width: sw as u32, height: sh as u32, depth_or_array_layers: 1 },
        TextureDimension::D2,
        sheet,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD,
    );
    sheet
        .try_into_dynamic()
        .map_err(|e| format!("{e:?}"))?
        .to_rgb8()
        .save(directory.join("sheet.png"))?;
    let verdict = if state.failed { "ERROR" } else if state.flagged > 0 { "FLAG" } else { "PASS" };
    println!(
        "graphics_audit verdict={verdict} poses={} measured={} flagged={} worst_black_blob={:.4}@{} worst_void_blob={:.4}@{} directory={}",
        sweep.poses.len(),
        state.completed,
        state.flagged,
        state.worst_black.0,
        state.worst_black.1,
        state.worst_void.0,
        state.worst_void.1,
        directory.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn poses_stand_on_gates_and_cover_the_line() {
        // Gates every 25 m along x: the four points' targets 25, 75, 125
        // and 175 m are gates 1, 3, 5 and 7.
        let line: Vec<Vec3> = (0..9).map(|i| Vec3::new(25. * i as f32, 0., 0.)).collect();
        let poses = poses(&line, 4);
        assert_eq!(poses.len(), 4 * VIEWS.len());
        let distances: Vec<f32> = poses.iter().step_by(VIEWS.len()).map(|p| p.distance).collect();
        assert_eq!(distances, [25., 75., 125., 175.]);
        let chase = &poses[VIEWS.len()];
        assert!((chase.eye - Vec3::new(68., CHASE_HEIGHT, 0.)).length() < 1e-3);
        assert!(chase.look.x > chase.eye.x);
        // A city corner: no pose between the gates, inside the block; two
        // points nearest the same gate give one set of views.
        let corner = [Vec3::ZERO, Vec3::new(100., 0., 0.), Vec3::new(100., 0., 100.)];
        let poses = super::poses(&corner, 4);
        assert_eq!(poses.len(), 3 * VIEWS.len());
        for pose in poses.iter().filter(|p| p.view != "chase") {
            assert!(corner.iter().any(|g| (pose.eye - *g - SIDE_HEIGHT * Vec3::Y).length() < 1e-3));
        }
    }

    #[test]
    fn eyes_follow_the_surface_and_keep_their_aim() {
        let line = [Vec3::ZERO, Vec3::new(100., 0., 0.)];
        let mut poses = poses(&line, 2);
        let before: Vec<Vec3> = poses.iter().map(|p| p.look - p.eye).collect();
        // The surface is 5 m above the gate line, except where nothing is.
        let n = ground_poses(&mut poses, |p| (p.x < 50.).then_some(5.));
        assert_eq!(n, VIEWS.len());
        for (pose, aim) in poses.iter().zip(before) {
            assert!((pose.look - pose.eye - aim).length() < 1e-4);
            let height = if pose.view == "chase" { CHASE_HEIGHT } else { SIDE_HEIGHT };
            let want = if pose.eye.x < 50. { 5. + height } else { height };
            assert!((pose.eye.y - want).abs() < 1e-4, "{} {}", pose.view, pose.eye);
        }
    }

    #[test]
    fn ripple_emissive_maps_are_the_undrawn_ones() {
        assert!(emissive_drawn("objects/big_screen_a_02", "object_simple_emissive"));
        assert!(emissive_drawn("objects/x", "object_lod_emissive_xfade"));
        assert!(emissive_drawn("objects/x", "object_animated_emissive"));
        assert!(emissive_drawn("objects/start_lights", "object_simple"));
        assert!(emissive_drawn("objects/x", "object_simple_night_emissive_skin"));
        assert!(!emissive_drawn("objects/x", "object_emissive_ripple"));
        assert!(!emissive_drawn("objects/x", "object_simple"));
    }

    #[test]
    fn largest_region_is_four_connected() {
        // Two diagonal blocks are separate regions; the L of three is one.
        #[rustfmt::skip]
        let mask = [
            true,  false, false, false,
            false, true,  true,  false,
            false, false, true,  false,
        ];
        let (n, b) = largest_region(&mask, 4, 3);
        assert_eq!(n, 3);
        assert_eq!(b, [1, 1, 2, 2]);
    }
}
