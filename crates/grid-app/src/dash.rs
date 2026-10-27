//! The cockpit's dash displays (docs/COCKPIT.md "Gauges"): the digital
//! readouts, motec bars, indicator lights and needles of `int_<stem>.xml`,
//! fed from the player's car while the cockpit is shown (0x9FDE40, feeder
//! 0xA04DB0). Presentation only: materials and part poses, never the
//! simulation.
use bevy::{
    asset::RenderAssetUsages,
    math::Affine2,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use grid_formats::{interior, material, pssg, scene, texture};
use std::{collections::BTreeMap, error::Error, path::Path};

/// The emissive factor of the interior programs: never set by the code, the
/// constant tables' default (docs/COCKPIT.md "Gauges") [I].
const EMISSIVE_FACTOR: f32 = 2.7;

/// The displays of one cockpit, read at load.
#[derive(Clone, Debug, Default)]
pub struct Dash {
    displays: Vec<Display>,
}

#[derive(Clone, Debug)]
struct Display {
    source: interior::Display,
    /// The cockpit parts the display's node draws.
    parts: Vec<String>,
    kind: Kind,
}

#[derive(Clone, Debug)]
enum Kind {
    /// A readout of `chars` glyphs; `lcd` draws dark digits by their alpha
    /// (lcd_text.fx), otherwise the glyphs are emissive (the gear).
    Digital { font: Font, chars: usize, lcd: bool },
    /// The two diffuse maps and the mask of motec.fx (RGBA8, RGBA8, R8).
    Motec { size: UVec2, off: Vec<u8>, on: Vec<u8>, mask: Vec<u8> },
    /// The light's TEmissiveMap (RGBA8): the "on" glow sits in the half
    /// ScrollU moves to (EmissiveMapScale 1.0, not a backlight shader).
    Indicator { emissive: Option<(UVec2, Vec<u8>)> },
    /// Needle about its node's local Z: the node's scene world at rest,
    /// each part's frame relative to the node and the sweep (radians).
    Needle { rest: Mat4, frames: BTreeMap<String, Mat4>, sweep: f32, dummy: bool },
}

/// A simple font: glyphs top to bottom, upright, RGBA8 rows.
#[derive(Clone, Debug)]
struct Font {
    size: UVec2,
    glyphs: Vec<Vec<u8>>,
    /// Character → glyph (0x99DE70's table; anything else is glyph 0).
    table: BTreeMap<char, usize>,
}

impl Font {
    /// The readout of `n` on `chars` glyphs (0x9D07F0): leading zeros,
    /// the leftmost characters on overflow, unknown characters as glyph 0.
    fn readout(&self, n: i32, chars: usize) -> Vec<u8> {
        let digits: String = n.to_string().chars().take(chars).collect();
        let text = format!("{}{digits}", "0".repeat(chars - digits.chars().count()));
        let (w, h) = (self.size.x as usize, self.size.y as usize);
        let mut out = vec![0; chars * w * h * 4];
        for (i, c) in text.chars().enumerate() {
            let glyph = &self.glyphs[self.table.get(&c).copied().unwrap_or(0).min(self.glyphs.len() - 1)];
            for y in 0..h {
                let row = (y * chars * w + i * w) * 4;
                out[row..row + w * 4].copy_from_slice(&glyph[y * w * 4..(y + 1) * w * 4]);
            }
        }
        out
    }
}

/// The values the feeder gives the displays (0xA04DB0), from the player's
/// vehicle words.
#[derive(Clone, Copy, Debug, Default)]
struct Values {
    /// Engine speed, rad/s: car +0x1894 [I: the original shows the engine
    /// audio's displayed speed × the limiter, 0x414C70 is not ported].
    tach: f32,
    /// car +0x1648 × 0.62137121 (the xml factor 3.6 then gives mph).
    speed: f32,
    /// car +0x196C (0 = N, 9 = R).
    gear: i32,
}

impl Values {
    fn read(vehicle: &[u32]) -> Self {
        let word = |o: usize| vehicle.get(o / 4).copied().map_or(0., f32::from_bits);
        Self {
            tach: word(0x1894),
            speed: word(0x1648) * 0.621_371_2,
            gear: vehicle.get(0x196c / 4).copied().unwrap_or(0) as i32,
        }
    }

    /// drift_angle, drift_peak and boost are not fed yet (0).
    fn get(&self, name: &str) -> Option<f32> {
        match name {
            "tach" => Some(self.tach),
            "speed" => Some(self.speed),
            "drift_angle" | "drift_peak" | "boost" => Some(0.),
            _ => None,
        }
    }
}

/// Reads the displays of `int_<stem>.xml` against the cockpit scene.
pub fn prepare(root: &Path, xml: &str, tree: &pssg::Tree, draws: &[scene::Draw]) -> Result<Dash, Box<dyn Error>> {
    let utf8 = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
    let mut displays = Vec::new();
    let bindings = material::read_texture_bindings(tree)?;
    for source in interior::read_displays(xml)? {
        let Some(node) = crate::cockpit::named(tree, b"RENDERNODE", source.node.as_bytes())
            .or_else(|| crate::cockpit::named(tree, b"NODE", source.node.as_bytes()))
        else {
            continue; // 0xA13AC0 creates no display for a missing node
        };
        let under: Vec<&scene::Draw> = draws
            .iter()
            .filter(|d| crate::cockpit::named(tree, b"RENDERNODE", d.joint).is_some_and(|r| crate::cockpit::descends(tree, r, node)))
            .collect();
        let parts: Vec<String> = under.iter().map(|d| format!("interior/{}", utf8(d.id))).collect();
        let first = under.first().map(|d| d.material);
        let kind = match source.kind.as_str() {
            "digital" => {
                let Some(name) = &source.font else { continue };
                let lcd = first.is_some_and(|m| group(tree, m) == b"lcd_text.fx");
                Kind::Digital {
                    font: font(root, name)?,
                    chars: source.num_chars.unwrap_or(0.).max(0.) as usize,
                    lcd,
                }
            }
            "motec" => {
                let Some(shader) = first else { continue };
                let named = |sampler: &[u8]| {
                    bindings.iter().find(|b| b.shader == shader && b.sampler == sampler && b.container.is_none()).map(|b| b.texture)
                };
                let (Some(off), Some(on), Some(mask)) =
                    (named(b"TDiffuseAlphaMap"), named(b"TDiffuseAlphaMap_2"), named(b"TAlphaMaskMap"))
                else {
                    continue;
                };
                let (size, off) = rgba(tree, off)?;
                let (_, on) = rgba(tree, on)?;
                let (mask_size, mask) = r8(tree, mask)?;
                if mask_size != size {
                    continue;
                }
                Kind::Motec { size, off, on, mask }
            }
            "indicator_light" => {
                let emissive = first
                    .and_then(|shader| {
                        bindings.iter().find(|b| b.shader == shader && b.sampler == b"TEmissiveMap" && b.container.is_none())
                    })
                    .map(|b| rgba(tree, b.texture))
                    .transpose()?;
                Kind::Indicator { emissive }
            }
            "analog" | "analog_dummy" => {
                let dummy = source.kind == "analog_dummy";
                let Some(max) = crate::cockpit::named(tree, b"NODE", format!("{}_max", source.node).as_bytes())
                    .or_else(|| crate::cockpit::named(tree, b"RENDERNODE", format!("{}_max", source.node).as_bytes()))
                else {
                    continue;
                };
                let world = Mat4::from_cols_array(&scene::node_matrix(tree, node)?);
                let local = scene::matrix(tree, node)?;
                let max_local = scene::matrix(tree, max)?;
                // 0x99D900: acos of the local Y rows' dot product.
                let dot: f32 = (4..8).map(|i| local[i] * max_local[i]).sum();
                let mut sweep = dot.clamp(-1., 1.).acos();
                if source.long_way.is_some_and(|v| v != 0.) {
                    sweep = std::f32::consts::TAU - sweep;
                }
                // analog_dummy rests at the _max pose (0x99DB40).
                let rest = if dummy {
                    let parent = tree.nodes[node].parent.map_or(Ok(Mat4::IDENTITY), |p| {
                        scene::node_matrix(tree, p).map(|m| Mat4::from_cols_array(&m))
                    })?;
                    parent * Mat4::from_cols_array(&max_local)
                } else {
                    world
                };
                let frames = under
                    .iter()
                    .map(|d| (format!("interior/{}", utf8(d.id)), world.inverse() * Mat4::from_cols_array(&d.transform)))
                    .collect();
                Kind::Needle { rest, frames, sweep, dummy }
            }
            _ => continue,
        };
        displays.push(Display { source, parts, kind });
    }
    Ok(Dash { displays })
}

/// The shader group of a shader instance.
fn group<'a>(tree: &pssg::Tree<'a>, instance: &[u8]) -> &'a [u8] {
    (0..tree.nodes.len())
        .find(|&i| tree.name(i) == b"SHADERINSTANCE" && tree.string(i, b"id").ok() == Some(instance))
        .and_then(|i| tree.string(i, b"shaderGroup").ok())
        .map_or(&[][..], |g| g.strip_prefix(b"#").unwrap_or(g))
}

/// A cockpit texture's base level as RGBA8.
fn rgba(tree: &pssg::Tree, id: &[u8]) -> Result<(UVec2, Vec<u8>), Box<dyn Error>> {
    let t = texture::read_named(tree, &[id])?.into_iter().next().ok_or("dash texture missing")?;
    let pixels = match t.format {
        texture::Format::Bc1 => crate::texture_upload::decode_levels(t.base_blocks, t.width, t.height, 1, false),
        texture::Format::Bc3 => crate::texture_upload::decode_levels(t.base_blocks, t.width, t.height, 1, true),
        texture::Format::Bgra8 => Some(t.base_blocks.as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], p[3]]).collect()),
    }
    .ok_or("dash texture undecodable")?;
    Ok((UVec2::new(t.width, t.height), pixels))
}

/// A `u8` texture's base level (the motec mask; `texture::read_named`
/// takes only the colour formats).
fn r8(tree: &pssg::Tree, id: &[u8]) -> Result<(UVec2, Vec<u8>), Box<dyn Error>> {
    let i = (0..tree.nodes.len())
        .find(|&i| tree.name(i) == b"TEXTURE" && tree.string(i, b"id").ok() == Some(id))
        .ok_or("dash mask missing")?;
    if tree.string(i, b"texelFormat")? != b"u8" {
        return Err("dash mask is not u8".into());
    }
    let (w, h) = (tree.integer(i, b"width")?, tree.integer(i, b"height")?);
    let data = tree.nodes[i]
        .children
        .iter()
        .filter(|&&b| tree.name(b) == b"TEXTUREIMAGEBLOCK")
        .flat_map(|&b| tree.nodes[b].children.iter().copied())
        .find(|&d| tree.name(d) == b"TEXTUREIMAGEBLOCKDATA")
        .and_then(|d| tree.nodes[d].data)
        .ok_or("dash mask data missing")?;
    let n = (w * h) as usize;
    if data.len() < n {
        return Err("dash mask truncated".into());
    }
    Ok((UVec2::new(w, h), data[..n].to_vec()))
}

/// A simple-font strip: `ui8x4` texels in one `TEXTUREIMAGE` (the older
/// layout of `simple_fonts.pssg`, no image blocks), as RGBA8.
fn strip(tree: &pssg::Tree, id: &[u8]) -> Result<(UVec2, Vec<u8>), Box<dyn Error>> {
    let i = (0..tree.nodes.len())
        .find(|&i| tree.name(i) == b"TEXTURE" && tree.string(i, b"id").ok() == Some(id))
        .ok_or("simple font texture missing")?;
    if tree.string(i, b"texelFormat")? != b"ui8x4" {
        return Err("simple font texture is not ui8x4".into());
    }
    let (w, h) = (tree.integer(i, b"width")?, tree.integer(i, b"height")?);
    let data = tree.nodes[i]
        .children
        .iter()
        .find(|&&c| tree.name(c) == b"TEXTUREIMAGE")
        .and_then(|&c| tree.nodes[c].data)
        .ok_or("simple font texels missing")?;
    let n = (w * h * 4) as usize;
    if data.len() < n {
        return Err("simple font texels truncated".into());
    }
    Ok((UVec2::new(w, h), data[..n].as_chunks::<4>().0.iter().flat_map(|p| [p[2], p[1], p[0], p[3]]).collect()))
}

/// `simplefonts/<name>_font.xml` with its strip from `simple_fonts.pssg`.
/// The strip is stored bottom-up: read upright, glyph 0 is at the top.
fn font(root: &Path, name: &str) -> Result<Font, Box<dyn Error>> {
    let xml = std::fs::read_to_string(root.join(format!("simplefonts/{name}_font.xml")))?;
    let f = interior::read_simple_font(&xml)?;
    let bytes = crate::read(&root.join("simplefonts/simple_fonts.pssg"))?;
    let tree = pssg::read_tree(&bytes)?;
    let (size, pixels) = strip(&tree, f.texture.as_bytes())?;
    let [cw, ch] = f.char_size;
    if size.x != cw || size.y < ch * f.glyphs {
        return Err("simple font strip size mismatch".into());
    }
    let row = (cw * 4) as usize;
    let upright: Vec<u8> = pixels.chunks_exact(row).rev().flatten().copied().collect();
    let glyph = row * ch as usize;
    let glyphs = (0..f.glyphs as usize).map(|g| upright[g * glyph..(g + 1) * glyph].to_vec()).collect();
    let table = f.translate.chars().enumerate().map(|(i, c)| (c, i)).collect();
    Ok(Font { size: UVec2::new(cw, ch), glyphs, table })
}

pub fn install(app: &mut App) {
    app.add_systems(Update, (attach, feed).chain().before(crate::drive_present));
}

/// A display at run time: its entities, material and what it shows.
struct Live {
    display: usize,
    entities: Vec<Entity>,
    /// Needles: each entity's frame relative to the node.
    frames: Vec<Mat4>,
    material: Handle<StandardMaterial>,
    image: Option<Handle<Image>>,
    /// Digital: the value drawn and the one fed last (drawn one feed late).
    drawn: Option<i32>,
    fed: i32,
    /// Motec: the mask level lit; indicator: on.
    level: Option<u8>,
    on: Option<bool>,
    /// analog_dummy clock, seconds while the cockpit is shown.
    clock: f32,
}

#[derive(Resource, Default)]
struct Running(Vec<Live>);

fn image(size: UVec2, data: Vec<u8>) -> Image {
    Image::new(
        Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

/// Once the cockpit's parts exist: each display gets its own material (a
/// copy of the part's, so the material path is the car's) and texture.
#[allow(clippy::type_complexity)]
fn attach(
    mut commands: Commands,
    cockpit: Option<Res<crate::cockpit::Cockpit>>,
    running: Option<Res<Running>>,
    parts: Query<(Entity, &Name, &MeshMaterial3d<StandardMaterial>), With<crate::cockpit::InteriorPart>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let Some(dash) = cockpit.as_ref().map(|c| &c.dash) else {
        return;
    };
    if running.is_some() || parts.is_empty() {
        return;
    }
    let mut live = Vec::new();
    for (index, display) in dash.displays.iter().enumerate() {
        let found: Vec<(Entity, Handle<StandardMaterial>, String)> = parts
            .iter()
            .filter(|(_, name, _)| display.parts.iter().any(|p| p == name.as_str()))
            .map(|(e, name, m)| (e, m.0.clone(), name.as_str().to_string()))
            .collect();
        let Some((_, original, _)) = found.first() else { continue };
        let mut m = materials.get(original).cloned().unwrap_or_default();
        let mut image_handle = None;
        match &display.kind {
            Kind::Digital { font, chars, lcd } => {
                let size = UVec2::new(font.size.x * *chars as u32, font.size.y);
                let handle = images.add(image(size, font.readout(0, *chars)));
                if *lcd {
                    // lcd_text.fx: the readout's alpha over the dash, no
                    // emissive by day (EmissiveMapScale 0).
                    m.base_color = Color::BLACK;
                    m.base_color_texture = Some(handle.clone());
                    m.alpha_mode = AlphaMode::Blend;
                } else {
                    // interior.fx emissive variant: TEmissiveMap × 1.0 × 2.7.
                    m.base_color = Color::BLACK;
                    m.base_color_texture = None;
                    m.emissive = LinearRgba::rgb(EMISSIVE_FACTOR, EMISSIVE_FACTOR, EMISSIVE_FACTOR);
                    m.emissive_texture = Some(handle.clone());
                }
                image_handle = Some(handle);
            }
            Kind::Motec { size, off, .. } => {
                let handle = images.add(image(*size, off.clone()));
                m.base_color_texture = Some(handle.clone());
                image_handle = Some(handle);
            }
            Kind::Indicator { emissive: Some((size, data)) } => {
                m.emissive = LinearRgba::rgb(EMISSIVE_FACTOR, EMISSIVE_FACTOR, EMISSIVE_FACTOR);
                m.emissive_texture = Some(images.add(image(*size, data.clone())));
            }
            Kind::Indicator { emissive: None } | Kind::Needle { .. } => {}
        }
        let material = materials.add(m);
        let needle = matches!(display.kind, Kind::Needle { .. });
        if !needle {
            for (entity, _, _) in &found {
                commands.entity(*entity).insert(MeshMaterial3d(material.clone()));
            }
        }
        let frames = found
            .iter()
            .map(|(_, _, name)| match &display.kind {
                Kind::Needle { frames, .. } => frames.get(name).copied().unwrap_or(Mat4::IDENTITY),
                _ => Mat4::IDENTITY,
            })
            .collect();
        live.push(Live {
            display: index,
            entities: found.iter().map(|(e, _, _)| *e).collect(),
            frames,
            material,
            image: image_handle,
            drawn: Some(0),
            fed: 0,
            level: None,
            on: None,
            clock: 0.,
        });
    }
    println!(
        "cockpit_dash displays={} attached={} kinds={}",
        dash.displays.len(),
        live.len(),
        live.iter().map(|l| dash.displays[l.display].source.kind.as_str()).collect::<Vec<_>>().join(",")
    );
    commands.insert_resource(Running(live));
}

/// 0x9FDE40 / 0xA04DB0 each frame while the cockpit is shown and the race
/// runs (a paused frame, dt 0, feeds nothing).
#[allow(clippy::too_many_arguments)]
fn feed(
    time: Res<Time>,
    driving: Option<Res<crate::Driving>>,
    cockpit: Option<Res<crate::cockpit::Cockpit>>,
    own: (Option<Res<crate::camera::Cameras>>, Option<Res<crate::replay::Replay>>),
    running: Option<ResMut<Running>>,
    mut parts: Query<&mut crate::MovingPart>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(driving), Some(cockpit), Some(mut running)) = (driving, cockpit, running) else {
        return;
    };
    let shown = crate::camera::own_car(own.0.as_deref(), own.1.as_deref(), true) == crate::camera::OwnCar::Interior;
    let dt = time.delta_secs();
    if !shown || driving.paused || dt <= 0. {
        return;
    }
    let values = Values::read(&driving.runtime.state.car.owner.vehicle);
    for live in &mut running.0 {
        let display = &cockpit.dash.displays[live.display];
        let s = &display.source;
        let value = values.get(&s.value);
        match &display.kind {
            Kind::Digital { font, chars, .. } => {
                // set_value: trunc(v × factor); gear feeds set_int directly.
                let n = if s.value == "gear" {
                    values.gear
                } else if let Some(v) = value {
                    (v * s.factor.unwrap_or(1.)) as i32
                } else {
                    continue;
                };
                // The bound texture shows the previous feed (double buffer).
                let show = live.fed;
                live.fed = n;
                if live.drawn != Some(show)
                    && let Some(handle) = &live.image
                    && let Some(mut img) = images.get_mut(handle)
                {
                    img.data = Some(font.readout(show, *chars));
                    live.drawn = Some(show);
                }
            }
            Kind::Motec { size, off, on, mask } => {
                let Some(v) = value else { continue };
                // AlphaRef = (v − min) · slope · factor + base; a texel is
                // lit when its mask is at most AlphaRef.
                let (min_a, max_a) = (s.min_alpha.unwrap_or(0.), s.max_alpha.unwrap_or(0.));
                let (min_v, max_v) = (s.min_value.unwrap_or(0.), s.max_value.unwrap_or(0.));
                let slope = ((max_a - min_a) / 255.) / (max_v - min_v);
                let alpha_ref = (v - min_v) * slope * s.factor.unwrap_or(0.) + min_a / 255.;
                let level = (alpha_ref * 255.).floor().clamp(-1., 255.);
                let level = if level < 0. { None } else { Some(level as u8) };
                if live.level == level && live.on.is_some() {
                    continue;
                }
                live.level = level;
                live.on = Some(true);
                let mut data = off.clone();
                if let Some(level) = level {
                    for (i, &m) in mask.iter().enumerate() {
                        if m <= level {
                            data[i * 4..i * 4 + 4].copy_from_slice(&on[i * 4..i * 4 + 4]);
                        }
                    }
                }
                if let Some(handle) = &live.image
                    && let Some(mut img) = images.get_mut(handle)
                {
                    debug_assert_eq!(data.len(), (size.x * size.y * 4) as usize);
                    img.data = Some(data);
                }
            }
            Kind::Indicator { .. } => {
                let Some(v) = value else { continue };
                let x = s.factor.unwrap_or(0.) * v;
                let on = s.cond_min.unwrap_or(0.) <= x && x <= s.cond_max.unwrap_or(0.);
                if live.on != Some(on)
                    && let Some(mut m) = materials.get_mut(&live.material)
                {
                    // ScrollU moves every lookup by `shift` (default 0.5).
                    let u = if on { s.shift.unwrap_or(0.5) } else { 0. };
                    m.uv_transform = Affine2::from_translation(Vec2::new(u, 0.));
                    live.on = Some(on);
                }
            }
            Kind::Needle { rest, sweep, dummy, .. } => {
                let angle = if *dummy {
                    live.clock += dt;
                    let (speed, deviation) = (s.speed.unwrap_or(0.), s.deviation.unwrap_or(0.));
                    (std::f32::consts::TAU * speed * live.clock).sin() * deviation * std::f32::consts::TAU
                } else {
                    let Some(v) = value else { continue };
                    let range = s.max_value.unwrap_or(0.) - s.min_value.unwrap_or(0.);
                    (s.factor.unwrap_or(1.) * v * sweep / range).clamp(0., *sweep)
                };
                let node = cockpit.anchor * *rest * Mat4::from_rotation_z(angle);
                for (&entity, frame) in live.entities.iter().zip(&live.frames) {
                    if let Ok(mut part) = parts.get_mut(entity) {
                        part.rest = node * *frame;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Leading zeros, the leftmost digits on overflow, '-' as glyph 0.
    #[test]
    fn readouts_follow_0x9d07f0() {
        let glyph = |g: u8| vec![g; 4];
        let font = Font {
            size: UVec2::new(1, 1),
            glyphs: (0..10).map(glyph).collect(),
            table: "0123456789".chars().enumerate().map(|(i, c)| (c, i)).collect(),
        };
        let digits = |n, chars| font.readout(n, chars).chunks(4).map(|p| p[0]).collect::<Vec<_>>();
        assert_eq!(digits(7, 3), [0, 0, 7]);
        assert_eq!(digits(1234, 3), [1, 2, 3]);
        assert_eq!(digits(-5, 3), [0, 0, 5]);
        assert_eq!(digits(-12, 3), [0, 1, 2]);
    }

    /// The needles of the cockpits that have them: turning the node by its
    /// sweep about local Z (long way) reaches the `_max` node's Y axis
    /// (docs/COCKPIT.md "Gauges", private `needles.py`).
    #[test]
    #[ignore = "needs GRID_GAME_ROOT"]
    fn shipped_needles_sweep_to_their_max_nodes() {
        let root = std::path::PathBuf::from(std::env::var("GRID_GAME_ROOT").expect("GRID_GAME_ROOT"));
        let mut checked = 0;
        for stem in ["350", "cam", "gto", "ply", "rx7", "s15", "sky"] {
            let bytes = std::fs::read(root.join(format!("interiors/{stem}/int_{stem}.pssg"))).unwrap();
            let tree = pssg::read_tree(&bytes).unwrap();
            let sources = grid_formats::mesh::read_static_meshes(&tree).unwrap();
            let draws = scene::read_static_draws(&tree, &sources).unwrap();
            let xml = std::fs::read_to_string(root.join(format!("interiors/{stem}/int_{stem}.xml"))).unwrap();
            let dash = prepare(&root, &xml, &tree, &draws).unwrap();
            for d in &dash.displays {
                let Kind::Needle { sweep, dummy: false, .. } = d.kind else { continue };
                let local = |name: &str| {
                    let node = crate::cockpit::named(&tree, b"RENDERNODE", name.as_bytes())
                        .or_else(|| crate::cockpit::named(&tree, b"NODE", name.as_bytes()))
                        .unwrap();
                    Mat4::from_cols_array(&scene::matrix(&tree, node).unwrap())
                };
                let (n, max) = (local(&d.source.node), local(&format!("{}_max", d.source.node)));
                if n.z_axis.truncate().dot(max.z_axis.truncate()) < 0.99 {
                    continue; // a tilted _max frame: only the angle matches
                }
                let turned = n * Mat4::from_rotation_z(sweep);
                let error = (turned.y_axis - max.y_axis).truncate().length();
                assert!(error < 2e-3, "{stem} {}: {error}", d.source.node);
                checked += 1;
            }
        }
        assert!(checked >= 7, "{checked}");
    }
}
