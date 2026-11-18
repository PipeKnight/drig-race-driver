//! The original's OSD scene, read at run time from the owner's install
//! (`Osd/dial2.pssg`, `Osd/sceneconfig.xml`, the five OSD fonts of
//! `Osd/fontDescriptions.xml`; docs/RACE_HUD.md "Original look").
//!
//! [D] The scene's node tree, quads, textures, shader parameters,
//! animations with their named ranges and the `camera_dialcam` camera
//! (vertical field of view 12.3068 degrees at z 3320) are used as saved.
//! [I] What each shader family does with its parameters (`osd.wgsl`), the
//! Euler order (X, Y, Z) and the text layout inside a text node's box.
//!
//! The scene is drawn by an overlay pass ([`overlay`]) at the start of the
//! frame's render graph, with the scene camera's view, into a transparent
//! image. The viewer camera's UI draws that image under every other UI node,
//! after tonemapping, post-processing and upscaling, so bloom, anti-aliasing
//! and grading never touch the HUD, and the pause menu, the post-race screens
//! and the touch zones stay on top (AUTHORED composition). No second camera.
use bevy::{
    asset::{RenderAssetUsages, uuid_handle},
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{
        AsBindGroup, BlendState, Extent3d, RenderPipelineDescriptor, TextureDimension,
        TextureFormat, TextureViewDescriptor, TextureViewDimension,
    },
    shader::ShaderRef,
    ui_render::prelude::{MaterialNode, UiMaterial, UiMaterialKey, UiMaterialPlugin},
};
use crate::mesh_stream::MeshStream;
use bevy::camera::primitives::MeshAabb;
use grid_formats::osd;
use std::collections::HashMap;
use std::path::Path;

const SHADER: Handle<Shader> = uuid_handle!("6a4a8f0e-1d0f-4f63-9a4e-6c8f1e7b2d31");
const DISPLAY_SHADER: Handle<Shader> = uuid_handle!("0c7e5a52-93a4-4b4e-8f63-2a9d6b1e4c70");

/// The fonts of `Osd/fontDescriptions.xml`, in order; a text node's
/// `rd4text parameter` indexes this list [D order, I meaning of the index].
pub const FONTS: [&str; 5] = [
    "microgramma_osd.PSSG",
    "swiss_bold_osd_2d.PSSG",
    "micro_drift.pssg",
    "micro_lap.pssg",
    "swiss_time.pssg",
];

/// One shader instance of the scene: the textures and values `osd.wgsl`
/// reads (the overlay pass binds them).
#[derive(Asset, TypePath, Debug, Clone)]
pub struct OsdMaterial {
    diffuse: Handle<Image>,
    glow: Handle<Image>,
    soft: Handle<Image>,
    sharp: Handle<Image>,
    params: Params,
}

/// A mesh node of the OSD scene, drawn by the overlay pass with its
/// [`OsdMaterial`] while visible.
#[derive(Component, Clone, Debug)]
#[require(Transform, Visibility)]
pub struct OsdDraw {
    pub mesh: Handle<Mesh>,
    pub material: Handle<OsdMaterial>,
    /// Local blend-sort centre of a streamed mesh (a text's current glyphs,
    /// F398; the minimap ribbon, F854); otherwise the mesh's bounds centre.
    centre: Option<Vec3>,
}

impl OsdDraw {
    pub fn new(mesh: Handle<Mesh>, material: Handle<OsdMaterial>) -> Self {
        OsdDraw { mesh, material, centre: None }
    }

    /// Sets the local blend-sort centre of a streamed, padded mesh to the
    /// bounds centre of its drawn vertices.
    pub(crate) fn set_centre(&mut self, centre: Vec3) {
        self.centre = Some(centre);
    }
}

/// `osd.wgsl` parameters.
#[derive(Clone, Copy, Debug, Default, PartialEq, bevy::render::render_resource::ShaderType)]
pub struct Params {
    colour: Vec4,
    a: Vec4,
    b: Vec4,
    c: Vec4,
}

/// The OSD image drawn as a UI node (premultiplied blend).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct OsdDisplay {
    #[texture(0)]
    #[sampler(1)]
    image: Handle<Image>,
}

impl OsdDisplay {
    /// The display of a scene rendered into `image` (premultiplied; the
    /// results art draws its own scene this way, results_art.rs).
    pub(crate) fn new(image: Handle<Image>) -> Self {
        OsdDisplay { image }
    }
    /// Re-points the display at its image after a resize (F737).
    pub(crate) fn touch(&mut self) {
        self.image = self.image.clone();
    }
}

/// Registers the display material and its shader (once).
pub(crate) fn display_plugin(app: &mut App) {
    if app.is_plugin_added::<UiMaterialPlugin<OsdDisplay>>() {
        return;
    }
    app.add_plugins(UiMaterialPlugin::<OsdDisplay>::default());
    let shader = Shader::from_wgsl(include_str!("osd_display.wgsl"), "grid_osd_display.wgsl");
    let _ = app.world_mut().resource_mut::<Assets<Shader>>().insert(DISPLAY_SHADER.id(), shader);
}

impl UiMaterial for OsdDisplay {
    fn fragment_shader() -> ShaderRef {
        DISPLAY_SHADER.into()
    }
    fn specialize(descriptor: &mut RenderPipelineDescriptor, _: UiMaterialKey<Self>) {
        if let Some(target) = descriptor
            .fragment
            .as_mut()
            .and_then(|f| f.targets.first_mut())
            .and_then(|t| t.as_mut())
        {
            target.blend = Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING);
        }
    }
}

/// Shader mode of an effect file (see `osd.wgsl`).
fn mode(effect: &str) -> f32 {
    match effect.trim_end_matches(".fx") {
        "osd3d_flat_image" => 0.,
        "osd3d_flat_colour" | "osd3d_flat_colour_text" => 1.,
        "osd3d_special_image" => 2.,
        "osd3d_maskramp" => 3.,
        "osd3d_needleblur" => 4.,
        "osd3d_reflect_colour" => 5.,
        "osd3d_reflect_colour_text" | "osd3d_reflect_colour_text_flat" => 6.,
        "osd3d_reflect_image" => 7.,
        _ => 8.,
    }
}

/// A node's own copy of its scene material with `scrollX`, `scrollY` or a
/// faded alpha set, as the original's per-instance shader parameters (the
/// combodigit attach 0x6A7AC0, the name tag 0x6A80A0). The copy follows the
/// scene material's animated values.
#[derive(Clone, Debug)]
struct Scroll {
    source: usize,
    x: Option<f32>,
    y: Option<f32>,
    /// Factor on the scene material's `alphaScaler`.
    fade: Option<f32>,
    handle: Option<Handle<OsdMaterial>>,
    dirty: bool,
}

impl Scroll {
    fn apply(&self, mut params: Params) -> Params {
        if let Some(x) = self.x {
            params.a.x = x;
        }
        if let Some(y) = self.y {
            params.a.y = y;
        }
        if let Some(f) = self.fade {
            params.colour.w *= f;
        }
        params
    }
}

/// CPU copy of a material's animatable values.
#[derive(Clone, Debug)]
struct MaterialState {
    handle: Handle<OsdMaterial>,
    params: Params,
    dirty: bool,
}

fn params(m: &osd::Material) -> Params {
    let f = |name: &str, i: usize, default: f32| m.floats(name).and_then(|v| v.get(i).copied()).unwrap_or(default);
    let alpha = m.floats("alphaScaler").or(m.floats("AlphaScaler")).and_then(|v| v.first().copied()).unwrap_or(1.);
    Params {
        colour: Vec4::new(f("DiffuseColour", 0, 1.), f("DiffuseColour", 1, 1.), f("DiffuseColour", 2, 1.), alpha),
        a: Vec4::new(f("scrollX", 0, 0.), f("scrollY", 0, 0.), f("glowScaler", 0, 0.), f("Ramp", 0, 0.)),
        b: Vec4::new(mode(&m.effect), f("Feather", 0, 0.01), f("ReflectionScaler", 0, 1.), f("LightScaler", 0, 1.)),
        c: Vec4::new(f("ReflectionContrast", 0, 1.), f("ReflectionBrightness", 0, 0.), f("ReflectionDesaturate", 0, 0.), f("blurScaler", 0, 1.)),
    }
}

/// Sets a named parameter from an animation sample.
fn set_param(p: &mut Params, name: &str, v: &[f32]) {
    let x = v.first().copied().unwrap_or(0.);
    match name {
        "DiffuseColour" => {
            p.colour.x = x;
            p.colour.y = v.get(1).copied().unwrap_or(x);
            p.colour.z = v.get(2).copied().unwrap_or(x);
        }
        "alphaScaler" | "AlphaScaler" => p.colour.w = x,
        "scrollX" => p.a.x = x,
        "scrollY" => p.a.y = x,
        "glowScaler" => p.a.z = x,
        "Ramp" => p.a.w = x,
        "Feather" => p.b.y = x,
        "blurScaler" => p.c.w = x,
        _ => {}
    }
}

/// A font glyph ready for layout (em units).
#[derive(Clone, Debug)]
struct Glyph {
    mesh: osd::Mesh,
    advance: f32,
    bearing_x: f32,
    bearing_y: f32,
    kerning: HashMap<char, f32>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Font {
    glyphs: HashMap<char, Glyph>,
    ascender: f32,
    descender: f32,
}

pub(crate) fn font(f: osd::Font) -> Font {
    let glyphs = f
        .glyphs
        .into_iter()
        .filter_map(|g| {
            let c = char::from_u32(g.code)?;
            Some((
                c,
                Glyph {
                    advance: g.advance,
                    bearing_x: g.bearing_x,
                    bearing_y: g.bearing_y,
                    kerning: g.kerning.iter().filter_map(|&(k, o)| Some((char::from_u32(k)?, o))).collect(),
                    mesh: g.mesh,
                },
            ))
        })
        .collect();
    Font {
        glyphs,
        ascender: f.ascender,
        descender: f.descender,
    }
}

/// Advance for a character the font lacks (em, AUTHORED).
const MISSING: f32 = 0.35;

/// Glyph meshes of `text` in a text node's box (local x0..x1, y0..y1) with
/// the font's metrics: the box spans the font's descender to ascender [I],
/// glyphs sit on the baseline by their bearings with kerning [D metrics],
/// aligned left, right or centre by where the box sits around its origin [I].
fn layout(font: &Font, text: &str, x0: f32, x1: f32, y0: f32, y1: f32) -> osd::Mesh {
    let em = (y1 - y0).max(1e-3) / (font.ascender - font.descender).max(0.1);
    let chars: Vec<char> = text.chars().collect();
    let step = |i: usize| -> f32 {
        let Some(g) = font.glyphs.get(&chars[i]) else {
            return MISSING;
        };
        let kern = chars.get(i + 1).and_then(|n| g.kerning.get(n)).copied().unwrap_or(0.);
        g.advance + kern
    };
    let width: f32 = (0..chars.len()).map(step).sum::<f32>() * em;
    let eps = 1e-2 * (x1 - x0).abs().max(1.);
    let pen = if x0.abs() < eps {
        x0
    } else if x1.abs() < eps {
        x1 - width
    } else {
        (x0 + x1) / 2. - width / 2.
    };
    glyphs(font, &chars, em, 0., pen, y0 - font.descender * em)
}

/// Glyph meshes of `text` in a front-end text node (`FETEXTLAYOUT`,
/// docs/RACE_HUD.md §6.7): glyphs `height` from the font's descender to its
/// ascender [I], `tracking` em apart [I], the case applied [D enum], and the
/// line aligned in the node's bounds by `h_align` / `v_align` [D enums;
/// I: against the bounds].
pub(crate) fn text_mesh(font: &Font, text: &str, t: &osd::TextLayout, bounds: &[f32; 6]) -> osd::Mesh {
    let (chars, em, pen, _) = line(font, text, t, bounds);
    // A text box with no y extent but a z extent lies in the node's x-z
    // plane, up along -z: the front-end's pill labels (`pill_header`) sit
    // under a node turned 90 degrees about x, with bounds z -1.15..0 [O]. The
    // glyphs are laid out in x-y over -z and turned -90 degrees about x
    // (y -> -z, z -> y), so they face the screen like the pill meshes [I].
    let [_, y0, z0, _, y1, z1] = *bounds;
    let flat = y1 - y0 <= 1e-4 && z1 - z0 > 1e-4;
    let (y0, y1) = if flat { (-z1, -z0) } else { (y0, y1) };
    let baseline = match t.v_align {
        0 => y1 - font.ascender * em,
        1 => y0 - font.descender * em,
        _ => (y0 + y1) / 2. - (font.ascender + font.descender) * em / 2.,
    };
    let mut m = glyphs(font, &chars, em, t.tracking, pen, baseline);
    if flat {
        for p in m.positions.iter_mut().chain(m.normals.iter_mut()) {
            *p = [p[0], p[2], -p[1]];
        }
    }
    m
}

/// The local x span [`text_mesh`] sets `text` over: the pen's start and end
/// (advances, kerning and tracking).
pub(crate) fn text_span(font: &Font, text: &str, t: &osd::TextLayout, bounds: &[f32; 6]) -> [f32; 2] {
    let (_, _, pen, width) = line(font, text, t, bounds);
    [pen, pen + width]
}

/// The characters of `text` in the layout's case, the em size, the pen's
/// start and the line's width.
fn line(font: &Font, text: &str, t: &osd::TextLayout, bounds: &[f32; 6]) -> (Vec<char>, f32, f32, f32) {
    let text = match t.case {
        1 => text.to_lowercase(),
        2 => text.to_uppercase(),
        _ => text.to_owned(),
    };
    let chars: Vec<char> = text.chars().collect();
    let em = t.height.max(1e-3) / (font.ascender - font.descender).max(0.1);
    let width = chars
        .iter()
        .enumerate()
        .map(|(i, c)| match font.glyphs.get(c) {
            Some(g) => g.advance + chars.get(i + 1).and_then(|n| g.kerning.get(n)).copied().unwrap_or(0.),
            None => MISSING,
        })
        .sum::<f32>()
        * em
        + t.tracking * em * chars.len().saturating_sub(1) as f32;
    let [x0, _, _, x1, _, _] = *bounds;
    let pen = match t.h_align {
        1 => x1 - width,
        2 => (x0 + x1 - width) / 2.,
        _ => x0,
    };
    (chars, em, pen, width)
}

/// The glyphs of `chars` from `pen` along `baseline`, `em` units per em,
/// `tracking` em added after each.
fn glyphs(font: &Font, chars: &[char], em: f32, tracking: f32, mut pen: f32, baseline: f32) -> osd::Mesh {
    let mut out = osd::Mesh::default();
    for (i, c) in chars.iter().enumerate() {
        let advance = match font.glyphs.get(c) {
            Some(g) => {
                let base = out.positions.len() as u16;
                let (ox, oy) = (pen + g.bearing_x * em, baseline + g.bearing_y * em);
                for (k, p) in g.mesh.positions.iter().enumerate() {
                    out.positions.push([ox + p[0] * em, oy + p[1] * em, p[2] * em]);
                    out.normals.push(g.mesh.normals.get(k).copied().unwrap_or([0., 0., 1.]));
                    out.uvs.push(g.mesh.uvs.get(k).copied().unwrap_or([0., 0.]));
                }
                out.indices.extend(g.mesh.indices.iter().map(|i| i + base));
                g.advance + chars.get(i + 1).and_then(|n| g.kerning.get(n)).copied().unwrap_or(0.)
            }
            None => MISSING,
        };
        pen += (advance + tracking) * em;
    }
    // An empty text keeps one degenerate triangle: Bevy's mesh allocator
    // does not accept empty buffers.
    if out.positions.is_empty() {
        out.positions = vec![[0.; 3]; 3];
        out.normals = vec![[0., 0., 1.]; 3];
        out.uvs = vec![[0.; 2]; 3];
        out.indices = vec![0, 1, 2];
    }
    out
}

/// A Bevy mesh of OSD triangles (missing normals face +z, missing UVs 0).
pub fn mesh(m: &osd::Mesh) -> Mesh {
    bevy_mesh(m)
}

fn bevy_mesh(m: &osd::Mesh) -> Mesh {
    let n = m.positions.len();
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD);
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, m.positions.clone());
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        if m.normals.len() == n { m.normals.clone() } else { vec![[0., 0., 1.]; n] },
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        if m.uvs.len() == n { m.uvs.clone() } else { vec![[0., 0.]; n] },
    );
    mesh.insert_indices(Indices::U16(m.indices.clone()));
    mesh
}

/// Bevy transform of a saved row-vector matrix.
fn transform(m: &[f32; 16]) -> Transform {
    Transform::from_matrix(Mat4::from_cols_array(m))
}

/// A DXT1 cubemap as an sRGB cube image (also the front-end world's reflection).
pub(crate) fn cube(c: &osd::Cubemap<'_>) -> Image {
    let mut data = Vec::new();
    for face in c.faces {
        data.extend(crate::texture_upload::decode_levels(face, c.size, c.size, 1, false).unwrap_or_default());
    }
    let mut image = Image::new(
        Extent3d { width: c.size, height: c.size, depth_or_array_layers: 6 },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    image.sampler = ImageSampler::linear();
    image
}

/// The reflection cubes for fe3d's env slot (`Fe3dMaterial`), from the first
/// source that holds all of its cube ids, with that source's path.
///
/// - Raw (gamma-space) texels, as fe3d reads them; [`cube`] is the sRGB view.
/// - Each source is a package path and its cube ids, e.g. the garage's
///   `ref_garage_soft/sharp.dds`, then the race OSD's `env_soft/sharp.dds`
///   as the stand-in (results_art). Any PSSG with `CUBEMAPTEXTURE` nodes works.
/// - The error names every source and why it failed.
pub(crate) fn cubes<const N: usize>(
    read: &dyn Fn(&str) -> Result<Vec<u8>, String>,
    sources: &[(&'static str, [&str; N])],
) -> Result<([Image; N], &'static str), String> {
    let mut failed = Vec::new();
    for &(path, ids) in sources {
        let from = || -> Result<[Image; N], String> {
            let bytes = read(path)?;
            let tree = grid_formats::pssg::read_tree(&bytes).map_err(|e| format!("{path}: {e}"))?;
            let all = osd::read_cubemaps(&tree).map_err(|e| format!("{path}: {e}"))?;
            let mut out = Vec::with_capacity(N);
            for id in ids {
                let c = all.iter().find(|c| c.id == id).ok_or(format!("{path}: no {id}"))?;
                let mut image = cube(c);
                image.texture_descriptor.format = TextureFormat::Rgba8Unorm;
                out.push(image);
            }
            out.try_into().map_err(|_| format!("{path}: cube count"))
        };
        match from() {
            Ok(images) => return Ok((images, path)),
            Err(e) => failed.push(e),
        }
    }
    Err(if failed.is_empty() { "no reflection cube source".to_owned() } else { failed.join("; ") })
}

struct TextNode {
    font: usize,
    bounds: [f32; 4],
    mesh: Handle<Mesh>,
    /// The node entity (alpha-blended: its sort centre, F398).
    entity: Entity,
    /// Vertex count and triangles of the mesh asset: a new string with the
    /// same is streamed into it instead of replacing it.
    vertices: usize,
    indices: Vec<u16>,
    text: String,
    want: String,
}

/// A running or held animation.
struct Anim {
    data: osd::Animation,
    time: f32,
    stop: f32,
    /// Resolved channel targets.
    targets: Vec<Target>,
}

enum Target {
    Node(Entity),
    Param(usize, String),
    None,
}

/// The loaded scene, before its entities exist.
#[derive(Resource)]
struct Loaded {
    nodes: Vec<osd::Node>,
    camera: osd::Camera,
    meshes: Vec<Option<Handle<Mesh>>>,
    materials: Vec<MaterialState>,
    material_ids: HashMap<String, usize>,
    animations: Vec<osd::Animation>,
    text_fonts: std::collections::BTreeMap<String, u32>,
    fonts: Vec<Font>,
    cubes: (Handle<Image>, Handle<Image>),
}

/// The live OSD scene. Other modules set texts, visibility and animation
/// times; the systems here apply them each frame.
/// Marks an app with the OSD scene loaded.
#[derive(Resource)]
pub struct Installed;

#[derive(Resource)]
pub struct Osd {
    entities: HashMap<String, Entity>,
    /// Node ids, parents and local bounds, in tree order.
    tree: Vec<(String, Option<usize>, [f32; 6])>,
    /// Saved local translation of each node of `tree`.
    rest: Vec<Vec3>,
    /// The environment cubemaps (soft, sharp) for new materials.
    cubes: (Handle<Image>, Handle<Image>),
    anims: Vec<Anim>,
    anim_index: HashMap<String, usize>,
    materials: Vec<MaterialState>,
    texts: HashMap<String, TextNode>,
    fonts: Vec<Font>,
    shown: HashMap<Entity, bool>,
    /// Added to a node's translation (authored layout moves, the drift ring
    /// shake): the wanted offset and the one now in the node's transform.
    offsets: HashMap<Entity, (Vec3, Vec3)>,
    /// The scene material of each render node.
    node_materials: HashMap<Entity, usize>,
    /// Per-node `scrollX`, `scrollY` and alpha over the node's scene material.
    scrolls: HashMap<Entity, Scroll>,
    image: Handle<Image>,
    /// The UI material that shows `image` over the 3D view.
    display: Handle<OsdDisplay>,
    /// The scene camera's placement and lens (`camera_dialcam`); the
    /// overlay pass draws with them.
    view: GlobalTransform,
    projection: PerspectiveProjection,
    /// False stops the overlay pass (perf_test `hud`); the image keeps its
    /// last contents.
    active: bool,
    size: UVec2,
}

impl Osd {
    /// The scene camera's world placement.
    pub fn view(&self) -> &GlobalTransform {
        &self.view
    }
    /// The scene camera's lens, with the image's aspect ratio.
    pub fn projection(&self) -> &PerspectiveProjection {
        &self.projection
    }
    /// Draws the scene each frame (true) or leaves the image as it is.
    pub fn set_active(&mut self, on: bool) {
        self.active = on;
    }
    /// The aspect ratio of the OSD image, once sized.
    pub fn aspect(&self) -> Option<f32> {
        (self.size.x > 0 && self.size.y > 0).then(|| self.size.x as f32 / self.size.y as f32)
    }
    /// The id of the first node named `name` (id without its `!n` suffix)
    /// below the node `under`.
    pub fn find(&self, under: &str, name: &str) -> Option<String> {
        let root = self.tree.iter().position(|(id, ..)| id == under)?;
        let mut stack = vec![root];
        while let Some(i) = stack.pop() {
            let id = &self.tree[i].0;
            if i != root && (id == name || id.split('!').next() == Some(name)) {
                return Some(id.clone());
            }
            stack.extend((0..self.tree.len()).rev().filter(|&c| self.tree[c].1 == Some(i)));
        }
        None
    }
    /// The entity of a node.
    pub fn entity(&self, node: &str) -> Option<Entity> {
        self.entities.get(node).copied()
    }
    /// The ids of a node's direct children, in tree order.
    pub fn children(&self, node: &str) -> Vec<String> {
        let Some(p) = self.tree.iter().position(|(id, ..)| id == node) else {
            return Vec::new();
        };
        self.tree.iter().filter(|t| t.1 == Some(p)).map(|t| t.0.clone()).collect()
    }
    /// The saved local translation of a node.
    pub fn rest(&self, node: &str) -> Option<Vec3> {
        self.tree.iter().position(|(id, ..)| id == node).and_then(|i| self.rest.get(i).copied())
    }
    /// The saved local bounds of a node (min xyz, max xyz).
    pub fn bounds(&self, node: &str) -> Option<[f32; 6]> {
        self.tree.iter().find(|(id, ..)| id == node).map(|t| t.2)
    }
    /// A flat colour material (`osd3d_flat_colour`) for authored additions.
    pub fn colour_material(&self, colour: Vec3, alpha: f32) -> OsdMaterial {
        let params = Params {
            colour: colour.extend(alpha),
            a: Vec4::ZERO,
            b: Vec4::new(1., 0.01, 1., 1.),
            c: Vec4::new(1., 0., 0., 1.),
        };
        OsdMaterial {
            diffuse: Handle::default(),
            glow: Handle::default(),
            soft: self.cubes.0.clone(),
            sharp: self.cubes.1.clone(),
            params,
        }
    }
    pub fn range_start(&self, anim: &str, range: &str) -> Option<f32> {
        let a = &self.anims[*self.anim_index.get(anim)?].data;
        a.range(range).map(|r| r.start)
    }
    /// The end of a range (`end` 0 is the animation's end).
    pub fn range_end(&self, anim: &str, range: &str) -> Option<f32> {
        let a = &self.anims[*self.anim_index.get(anim)?].data;
        a.range(range).map(|r| if r.end > r.start { r.end } else { a.duration() })
    }
    /// Sets the text of a text node (by node id).
    pub fn set_text(&mut self, node: &str, text: &str) {
        if let Some(t) = self.texts.get_mut(node)
            && t.want != text
        {
            t.want = text.to_owned();
        }
    }
    /// Moves a node by `offset` on top of its animated (or rest) translation.
    pub fn offset(&mut self, node: &str, offset: Vec3) {
        if let Some(&e) = self.entities.get(node) {
            self.offsets.entry(e).or_default().0 = offset;
        }
    }
    /// Sets a render node's own `scrollY` material value (a digit strip's
    /// glyph), leaving other nodes that share its material unchanged.
    pub fn scroll_y(&mut self, node: &str, y: f32) {
        self.look(node, |s| &mut s.y, y);
    }
    /// Sets a render node's own `scrollX` material value (a flag atlas
    /// column).
    pub fn scroll_x(&mut self, node: &str, x: f32) {
        self.look(node, |s| &mut s.x, x);
    }
    /// Scales a render or text node's own alpha: its scene material's
    /// `alphaScaler` times `factor`.
    pub fn fade(&mut self, node: &str, factor: f32) {
        self.look(node, |s| &mut s.fade, factor);
    }
    fn look(&mut self, node: &str, field: impl Fn(&mut Scroll) -> &mut Option<f32>, value: f32) {
        let Some(&e) = self.entities.get(node) else {
            return;
        };
        let Some(&source) = self.node_materials.get(&e) else {
            return;
        };
        let s = self.scrolls.entry(e).or_insert(Scroll { source, x: None, y: None, fade: None, handle: None, dirty: true });
        let slot = field(s);
        if *slot != Some(value) {
            *slot = Some(value);
            s.dirty = true;
        }
    }
    /// A node's own `scrollY`, once set.
    pub fn scroll_of(&self, node: &str) -> Option<f32> {
        self.scrolls.get(self.entities.get(node)?).and_then(|s| s.y)
    }
    /// Shows or hides a node and its children.
    pub fn show(&mut self, node: &str, on: bool) {
        if let Some(&e) = self.entities.get(node) {
            self.shown.insert(e, on);
        }
    }
    /// Plays a named range of an animation from its start to its end.
    pub fn play(&mut self, anim: &str, range: &str) {
        if let Some(a) = self.anim_index.get(anim).map(|&i| &mut self.anims[i])
            && let Some(r) = a.data.range(range).cloned()
        {
            a.time = r.start;
            a.stop = if r.end > r.start { r.end } else { a.data.duration() };
        }
    }
    /// Plays a named range unless that range is already playing or done.
    pub fn play_once(&mut self, anim: &str, range: &str) {
        if let Some(a) = self.anim_index.get(anim).map(|&i| &self.anims[i])
            && let Some(r) = a.data.range(range)
        {
            let end = if r.end > r.start { r.end } else { a.data.duration() };
            if a.stop == end && a.time >= r.start && a.time <= end {
                return;
            }
        }
        self.play(anim, range);
    }
    /// Plays a named range from fraction `from` to fraction `to` of its
    /// length, unless the animation already plays or stopped at `to`. The
    /// original seeks attach animations by a fraction of the selected range
    /// (0x8D5820).
    pub fn play_part_once(&mut self, anim: &str, range: &str, from: f32, to: f32) {
        if let Some(a) = self.anim_index.get(anim).map(|&i| &mut self.anims[i])
            && let Some(r) = a.data.range(range)
        {
            let end = if r.end > r.start { r.end } else { a.data.duration() };
            let at = |f: f32| r.start + (end - r.start) * f;
            let (start, stop) = (at(from), at(to));
            if a.stop == stop && a.time >= start.min(stop) && a.time <= stop {
                return;
            }
            a.time = start;
            a.stop = stop;
        }
    }
    /// Whether an animation has reached the end of what it plays.
    pub fn finished(&self, anim: &str) -> bool {
        self.anim_index.get(anim).is_none_or(|&i| self.anims[i].time >= self.anims[i].stop)
    }
    /// Holds an animation at the start of a named range.
    pub fn hold(&mut self, anim: &str, range: &str) {
        if let Some(a) = self.anim_index.get(anim).map(|&i| &mut self.anims[i])
            && let Some(r) = a.data.range(range)
        {
            a.time = r.start;
            a.stop = r.start;
        }
    }
    /// Holds an animation at a time.
    pub fn seek(&mut self, anim: &str, time: f32) {
        if let Some(a) = self.anim_index.get(anim).map(|&i| &mut self.anims[i]) {
            a.time = time;
            a.stop = time;
        }
    }
    pub fn has_range(&self, anim: &str, range: &str) -> bool {
        self.anim_index.get(anim).is_some_and(|&i| self.anims[i].data.range(range).is_some())
    }
    /// The duration of an animation (its last key).
    pub fn duration(&self, anim: &str) -> f32 {
        self.anim_index.get(anim).map_or(0., |&i| self.anims[i].data.duration())
    }
}

/// Each texture's own addressing (`wrapS` / `wrapT`). A sprite that repeated
/// drew a thin line along its far edges from the opposite edge (bilinear
/// filtering and mips across the wrap): two lines beside the rotated needle
/// and its blur (F739).
pub(crate) fn sampler([u, v]: [grid_formats::texture::Wrap; 2]) -> ImageSampler {
    let mode = |w| match w {
        grid_formats::texture::Wrap::Repeat => ImageAddressMode::Repeat,
        grid_formats::texture::Wrap::Clamp => ImageAddressMode::ClampToEdge,
    };
    ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: mode(u),
        address_mode_v: mode(v),
        ..ImageSamplerDescriptor::linear()
    })
}

/// Loads the scene and fonts; `Err` leaves the app without the OSD (the
/// plain HUD panels then stay).
pub fn install(app: &mut App, root: &Path) -> Result<(), String> {
    let read = |p: &str| std::fs::read(root.join(p)).map_err(|e| format!("{p}: {e}"));
    let bytes = read("Osd/dial2.pssg")?;
    let tree = grid_formats::pssg::read_tree(&bytes).map_err(|e| format!("dial2.pssg: {e}"))?;
    let scene = osd::read_scene(&tree).map_err(|e| format!("dial2.pssg: {e}"))?;
    let camera = scene.camera.clone().ok_or("dial2.pssg has no camera")?;
    let text_fonts = osd::text_fonts(&read("Osd/sceneconfig.xml")?).map_err(|e| format!("sceneconfig.xml: {e}"))?;
    let mut fonts = Vec::new();
    for f in FONTS {
        let bytes = read(&format!("frontend/fonts/{f}"))?;
        let tree = grid_formats::pssg::read_tree(&bytes).map_err(|e| format!("{f}: {e}"))?;
        fonts.push(font(osd::read_font(&tree).map_err(|e| format!("{f}: {e}"))?));
    }
    app.init_asset::<OsdMaterial>();
    display_plugin(app);
    let world = app.world_mut();
    world.resource_mut::<Assets<Shader>>().insert(SHADER.id(), Shader::from_wgsl(include_str!("osd.wgsl"), "grid_osd.wgsl")).map_err(|e| e.to_string())?;
    let mut images = HashMap::new();
    {
        let mut assets = world.resource_mut::<Assets<Image>>();
        for (t, &wrap) in scene.textures.iter().zip(&scene.wraps) {
            let (dds, _) = t.validated_dds();
            let image = crate::texture_upload::image(&dds, sampler(wrap), RenderAssetUsages::RENDER_WORLD).map_err(|e| e.to_string())?;
            images.insert(String::from_utf8_lossy(t.id).into_owned(), assets.add(image));
        }
        for c in &scene.cubemaps {
            images.insert(c.id.clone(), assets.add(cube(c)));
        }
    }
    let fallback_cube = images.get("env_sharp.dds").cloned().unwrap_or_default();
    let mut materials = Vec::new();
    let mut material_ids = HashMap::new();
    {
        let mut assets = world.resource_mut::<Assets<OsdMaterial>>();
        for (i, m) in scene.materials.iter().enumerate() {
            let texture = |name: &str| m.texture(name).and_then(|t| images.get(t)).cloned();
            let diffuse = texture("DiffuseMap").or_else(|| texture("RampMap")).unwrap_or_default();
            let p = params(m);
            let handle = assets.add(OsdMaterial {
                glow: texture("GlowMap").unwrap_or_else(|| diffuse.clone()),
                diffuse,
                soft: texture("Emap").unwrap_or_else(|| fallback_cube.clone()),
                sharp: texture("LightMap").unwrap_or_else(|| fallback_cube.clone()),
                params: p,
            });
            material_ids.insert(m.id.clone(), i);
            materials.push(MaterialState { handle, params: p, dirty: false });
        }
    }
    let meshes = {
        let mut assets = world.resource_mut::<Assets<Mesh>>();
        scene
            .nodes
            .iter()
            .map(|n| n.render.as_ref().map(|r| assets.add(bevy_mesh(&r.mesh))))
            .collect()
    };
    println!(
        "osd_scene nodes={} materials={} textures={} animations={} fonts={}",
        scene.nodes.len(),
        scene.materials.len(),
        scene.textures.len(),
        scene.animations.len(),
        fonts.len()
    );
    world.insert_resource(Loaded {
        nodes: scene.nodes,
        camera,
        meshes,
        materials,
        material_ids,
        animations: scene.animations,
        text_fonts,
        fonts,
        cubes: (images.get("env_soft.dds").cloned().unwrap_or_default(), fallback_cube.clone()),
    });
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render
            .add_systems(ExtractSchedule, extract_display_ids)
            .add_systems(Render, check_display.in_set(RenderSystems::Queue));
        overlay::install(render);
    }
    app.insert_resource(Installed)
        .init_resource::<crate::mesh_stream::MeshStream>()
        .add_systems(Update, spawn)
        .add_systems(PostUpdate, (resize, animate, texts, visibility).chain().before(TransformSystems::Propagate));
    Ok(())
}

use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::OwnedBindingResource;
use bevy::render::texture::GpuImage;
use bevy::render::{Extract, ExtractSchedule, Render, RenderApp, RenderSystems};
use bevy::transform::TransformSystems;
use bevy::ui_render::PreparedUiMaterial;

/// The OSD image and its display material, in the render world.
#[derive(Resource, Clone, Copy)]
struct DisplayIds {
    image: AssetId<Image>,
    display: AssetId<OsdDisplay>,
}

fn extract_display_ids(mut commands: Commands, osd: Extract<Option<Res<Osd>>>, mut done: Local<bool>) {
    if let (false, Some(osd)) = (*done, osd.as_ref()) {
        commands.insert_resource(DisplayIds { image: osd.image.id(), display: osd.display.id() });
        *done = true;
    }
}

/// Prints `osd_display bound=B size=WxH` when it changes: whether the display
/// material binds the OSD image's current GPU texture (false: the screen
/// shows a stale OSD, F737) and that texture's size. Interactive window
/// checks (tools/hud_window_check.py) read it after a resize.
fn check_display(
    ids: Option<Res<DisplayIds>>,
    images: Res<RenderAssets<GpuImage>>,
    materials: Res<RenderAssets<PreparedUiMaterial<OsdDisplay>>>,
    mut last: Local<Option<(bool, UVec2)>>,
) {
    let Some(ids) = ids else {
        return;
    };
    let (Some(gpu), Some(material)) = (images.get(ids.image), materials.get(ids.display)) else {
        return;
    };
    let view = gpu.texture_view.id();
    let bound = material
        .bindings
        .0
        .iter()
        .any(|(_, r)| matches!(r, OwnedBindingResource::TextureView(_, v) if v.id() == view));
    let size = UVec2::new(gpu.texture_descriptor.size.width, gpu.texture_descriptor.size.height);
    if *last != Some((bound, size)) {
        println!("osd_display bound={bound} size={}x{}", size.x, size.y);
        *last = Some((bound, size));
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn(
    mut commands: Commands,
    loaded: Option<Res<Loaded>>,
    viewer: Query<Entity, With<crate::ViewerCamera>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut displays: ResMut<Assets<OsdDisplay>>,
) {
    let (Some(loaded), Some(viewer)) = (loaded, viewer.iter().next()) else {
        return;
    };
    commands.remove_resource::<Loaded>();
    let image = images.add(Image::new_target_texture(16, 16, TextureFormat::Rgba8UnormSrgb, None));
    let display = displays.add(OsdDisplay { image: image.clone() });
    let c = &loaded.camera;
    let view = GlobalTransform::from(transform(&c.world));
    let projection = PerspectiveProjection {
        fov: c.fov_degrees.to_radians(),
        near: c.near.max(1.),
        far: c.far,
        aspect_ratio: 16. / 9.,
        ..default()
    };
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: percent(100.),
            height: percent(100.),
            ..default()
        },
        MaterialNode(display.clone()),
        GlobalZIndex(-1000),
        Pickable::IGNORE,
        UiTargetCamera(viewer),
    ));
    // Nodes, in tree order (parents first).
    let mut entities: Vec<Entity> = Vec::with_capacity(loaded.nodes.len());
    let mut by_id = HashMap::new();
    let mut node_materials = HashMap::new();
    let mut texts = HashMap::new();
    for (i, n) in loaded.nodes.iter().enumerate() {
        let mut e = commands.spawn((transform(&n.local), Visibility::Inherited));
        if let Some(p) = n.parent {
            e.insert(ChildOf(entities[p]));
        }
        if let (Some(r), Some(mesh)) = (&n.render, &loaded.meshes[i]) {
            let text = loaded.text_fonts.get(n.name()).copied().filter(|&f| (f as usize) < loaded.fonts.len());
            let mesh = match text {
                Some(font) => {
                    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
                    for p in &r.mesh.positions {
                        for k in 0..2 {
                            lo[k] = lo[k].min(p[k]);
                            hi[k] = hi[k].max(p[k]);
                        }
                    }
                    let empty = layout(&loaded.fonts[font as usize], "", lo[0], hi[0], lo[1], hi[1]);
                    let handle = meshes.add(bevy_mesh(&empty));
                    texts.insert(
                        n.id.clone(),
                        TextNode {
                            font: font as usize,
                            bounds: [lo[0], hi[0], lo[1], hi[1]],
                            mesh: handle.clone(),
                            entity: e.id(),
                            vertices: empty.positions.len(),
                            indices: empty.indices,
                            text: String::new(),
                            want: String::new(),
                        },
                    );
                    handle
                }
                None => mesh.clone(),
            };
            node_materials.insert(e.id(), r.material);
            e.insert(OsdDraw::new(mesh, loaded.materials[r.material].handle.clone()));
        }
        let id = e.id();
        entities.push(id);
        by_id.entry(n.id.clone()).or_insert(id);
    }
    // Animation targets: a node named in the owner's subtree, else any node
    // with that id; parameters by `material#name`.
    let subtree_of = |owner: usize, name: &str| -> Option<usize> {
        let mut stack = vec![owner];
        while let Some(i) = stack.pop() {
            if loaded.nodes[i].id == name || loaded.nodes[i].name() == name {
                return Some(i);
            }
            stack.extend((0..loaded.nodes.len()).filter(|&c| loaded.nodes[c].parent == Some(i)));
        }
        None
    };
    let mut anims = Vec::new();
    let mut anim_index = HashMap::new();
    for a in &loaded.animations {
        let owner = loaded.nodes.iter().position(|n| n.id == a.owner());
        let targets = a
            .channels
            .iter()
            .map(|c| {
                if c.semantic == osd::Semantic::Parameter {
                    let mut parts = c.target.splitn(2, '#');
                    let (m, p) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
                    return match loaded.material_ids.get(m) {
                        Some(&i) => Target::Param(i, p.to_owned()),
                        None => Target::None,
                    };
                }
                let node = owner
                    .and_then(|o| subtree_of(o, &c.target))
                    .or_else(|| loaded.nodes.iter().position(|n| n.id == c.target));
                match node {
                    Some(i) => Target::Node(entities[i]),
                    None => Target::None,
                }
            })
            .collect();
        anim_index.insert(a.owner().to_owned(), anims.len());
        anims.push(Anim { data: a.clone(), time: 0., stop: 0., targets });
    }
    commands.insert_resource(Osd {
        entities: by_id,
        tree: loaded.nodes.iter().map(|n| (n.id.clone(), n.parent, n.bounds)).collect(),
        rest: loaded.nodes.iter().map(|n| Vec3::new(n.local[12], n.local[13], n.local[14])).collect(),
        cubes: loaded.cubes.clone(),
        anims,
        anim_index,
        materials: loaded.materials.clone(),
        texts,
        fonts: loaded.fonts.clone(),
        shown: HashMap::new(),
        offsets: HashMap::new(),
        node_materials,
        scrolls: HashMap::new(),
        image,
        display,
        view,
        projection,
        active: true,
        size: UVec2::ZERO,
    });
}

/// Keeps the OSD image at the UI camera's size and aspect: the 3D view, or
/// the window when the 3D view is a scaled image (render_scale, F556).
fn resize(
    osd: Option<ResMut<Osd>>,
    viewer: Query<&Camera, With<crate::ViewerCamera>>,
    ui: Query<&Camera, With<IsDefaultUiCamera>>,
    mut images: ResMut<Assets<Image>>,
    mut displays: ResMut<Assets<OsdDisplay>>,
) {
    let (Some(mut osd), Some(camera)) = (osd, ui.iter().next().or(viewer.iter().next())) else {
        return;
    };
    let Some(size) = camera.physical_target_size().filter(|s| s.x > 0 && s.y > 0) else {
        return;
    };
    if set_size(&mut osd, size, &mut images, &mut displays) {
        osd.projection.aspect_ratio = size.x as f32 / size.y as f32;
    }
}

/// Resizes the OSD image; false when it already has `size`. The resized
/// image is a new GPU texture under the same handle, but a UI material's
/// bind group is rebuilt only when the material asset changes (Bevy 0.19
/// `PreparedUiMaterial` tracks no image dependencies): without touching the
/// display material the screen keeps showing the old texture, the OSD as it
/// was at the resize (F737: a window maximised mid-race froze the dial,
/// timers and minimap).
fn set_size(osd: &mut Osd, size: UVec2, images: &mut Assets<Image>, displays: &mut Assets<OsdDisplay>) -> bool {
    if size == osd.size {
        return false;
    }
    osd.size = size;
    if let Some(mut image) = images.get_mut(&osd.image) {
        image.resize(Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 });
    }
    if let Some(mut display) = displays.get_mut(&osd.display) {
        display.image = osd.image.clone();
    }
    true
}

/// Advances the animations and writes node transforms and material values.
fn animate(
    osd: Option<ResMut<Osd>>,
    time: Option<Res<Time<Real>>>,
    mut transforms: Query<&mut Transform>,
    mut materials: ResMut<Assets<OsdMaterial>>,
    mut bound: Query<&mut OsdDraw>,
) {
    let Some(mut osd) = osd else {
        return;
    };
    let dt = time.map_or(0., |t| t.delta_secs());
    let osd = &mut *osd;
    // Offset nodes whose translation an animation wrote this frame.
    let mut moved: Vec<Entity> = Vec::new();
    for a in &mut osd.anims {
        if a.time < a.stop {
            a.time = (a.time + dt).min(a.stop);
        }
        for (c, target) in a.data.channels.iter().zip(&a.targets) {
            let v: Vec<f32> = c.components.iter().map(|k| k.sample(a.time)).collect();
            match target {
                Target::Node(e) => {
                    let Ok(mut t) = transforms.get_mut(*e) else {
                        continue;
                    };
                    let mut next = *t;
                    match c.semantic {
                        osd::Semantic::Translation if v.len() == 3 => {
                            let mut at = Vec3::new(v[0], v[1], v[2]);
                            if let Some(o) = osd.offsets.get_mut(e) {
                                at += o.0;
                                o.1 = o.0;
                                moved.push(*e);
                            }
                            next.translation = at;
                        }
                        osd::Semantic::Scale if v.len() == 3 => next.scale = Vec3::new(v[0], v[1], v[2]),
                        osd::Semantic::Rotation if v.len() == 3 => {
                            next.rotation = Quat::from_euler(EulerRot::ZYX, v[2], v[1], v[0]);
                        }
                        _ => {}
                    }
                    if next != *t {
                        *t = next;
                    }
                }
                Target::Param(m, name) => {
                    let state = &mut osd.materials[*m];
                    let before = state.params;
                    set_param(&mut state.params, name, &v);
                    if state.params != before {
                        state.dirty = true;
                    }
                }
                Target::None => {}
            }
        }
    }
    // A node no animation moves keeps its rest translation plus the offset.
    for (e, o) in &mut osd.offsets {
        if o.0 != o.1
            && !moved.contains(e)
            && let Ok(mut t) = transforms.get_mut(*e)
        {
            t.translation += o.0 - o.1;
            o.1 = o.0;
        }
    }
    for (e, s) in &mut osd.scrolls {
        let state = &osd.materials[s.source];
        if !(s.dirty || state.dirty) {
            continue;
        }
        s.dirty = false;
        let params = s.apply(state.params);
        match &s.handle {
            Some(h) => {
                if let Some(mut m) = materials.get_mut(h) {
                    m.params = params;
                }
            }
            None => {
                let Some(mut m) = materials.get(&state.handle).cloned() else {
                    continue;
                };
                m.params = params;
                let h = materials.add(m);
                if let Ok(mut b) = bound.get_mut(*e) {
                    b.material = h.clone();
                }
                s.handle = Some(h);
            }
        }
    }
    for state in &mut osd.materials {
        if state.dirty {
            state.dirty = false;
            if let Some(mut m) = materials.get_mut(&state.handle) {
                m.params = state.params;
            }
        }
    }
}

/// Rebuilds the glyph meshes of changed texts. A string with the same glyph
/// vertices and triangles as the mesh (a running timer) is written over the
/// mesh's GPU allocation instead of replacing the asset (mesh_stream, F398).
fn texts(
    osd: Option<ResMut<Osd>>,
    mut meshes: ResMut<Assets<Mesh>>,
    stream: Res<MeshStream>,
    mut draws: Query<&mut OsdDraw>,
) {
    let Some(mut osd) = osd else {
        return;
    };
    let osd = &mut *osd;
    for t in osd.texts.values_mut() {
        if t.text == t.want {
            continue;
        }
        t.text = t.want.clone();
        let [x0, x1, y0, y1] = t.bounds;
        let glyphs = layout(&osd.fonts[t.font], &t.text, x0, x1, y0, y1);
        let mesh = bevy_mesh(&glyphs);
        // Blended: sorted by the centre the replaced asset would have.
        let centre = mesh.compute_aabb().map_or(Vec3::ZERO, |aabb| aabb.center.into());
        if glyphs.positions.len() == t.vertices && glyphs.indices == t.indices {
            stream.write(t.mesh.id(), |bytes| bytes.extend(mesh.create_packed_vertex_buffer_data()));
        } else if let Some(mut m) = meshes.get_mut(&t.mesh) {
            t.vertices = glyphs.positions.len();
            t.indices = glyphs.indices;
            *m = mesh;
        }
        if let Ok(mut d) = draws.get_mut(t.entity) {
            d.centre = Some(centre);
        }
    }
}

fn visibility(osd: Option<Res<Osd>>, mut query: Query<&mut Visibility>) {
    let Some(osd) = osd else {
        return;
    };
    if !osd.is_changed() {
        return;
    }
    for (&e, &on) in &osd.shown {
        if let Ok(mut v) = query.get_mut(e) {
            let want = if on { Visibility::Inherited } else { Visibility::Hidden };
            if *v != want {
                *v = want;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyph(code: char, width: f32, height: f32, advance: f32, by: f32) -> osd::Glyph {
        osd::Glyph {
            code: u32::from(code),
            mesh: osd::Mesh {
                positions: vec![[0., 0., 0.], [0., -height, 0.], [width, -height, 0.], [width, 0., 0.]],
                normals: vec![],
                uvs: vec![[0., 0.]; 4],
                colors_argb: vec![],
                texcoord1: vec![],
                indices: vec![0, 1, 2, 0, 2, 3],
            },
            bounds: [0., -height, 0., width, 0., 0.],
            advance,
            bearing_x: 0.05,
            bearing_y: by,
            kerning: if code == '0' { vec![(u32::from('.'), -0.1)] } else { vec![] },
        }
    }

    fn sample() -> Font {
        font(osd::Font {
            ascender: 0.8,
            descender: -0.2,
            glyphs: vec![glyph('0', 0.4, 0.7, 0.5, 0.7), glyph('.', 0.1, 0.1, 0.2, 0.1)],
        })
    }

    #[test]
    fn a_node_copy_sets_its_scroll_and_fades_the_scene_alpha() {
        let scene = Params { colour: Vec4::new(1., 0.5, 0.25, 0.8), a: Vec4::new(0.1, 0.2, 0.3, 0.4), b: Vec4::ONE, c: Vec4::ONE };
        let mut s = Scroll { source: 0, x: None, y: Some(0.5), fade: None, handle: None, dirty: true };
        // Only the set values change; the rest follows the scene material.
        let p = s.apply(scene);
        assert_eq!((p.a, p.colour), (Vec4::new(0.1, 0.5, 0.3, 0.4), scene.colour));
        s.x = Some(0.25);
        s.fade = Some(0.5);
        let p = s.apply(scene);
        assert_eq!(p.a, Vec4::new(0.25, 0.5, 0.3, 0.4));
        assert_eq!(p.colour, Vec4::new(1., 0.5, 0.25, 0.4));
    }

    #[test]
    fn glyphs_sit_on_the_baseline_by_their_metrics() {
        let f = sample();
        // A 10-unit box covers descender..ascender: em 10, baseline at y0 + 2.
        let m = layout(&f, "0.", 0., 100., 0., 10.);
        assert_eq!(m.positions.len(), 8);
        // '0' top at baseline + 7, left at the bearing.
        assert!((m.positions[0][1] - 9.).abs() < 1e-4);
        assert!((m.positions[0][0] - 0.5).abs() < 1e-4);
        // '.' bottom on the baseline, after the kerned advance 0.5 - 0.1.
        assert!((m.positions[5][1] - 2.).abs() < 1e-4);
        assert!((m.positions[4][0] - (4. + 0.5)).abs() < 1e-4);
        // Right and centre boxes.
        let r = layout(&f, "0", -100., 0., 0., 10.);
        assert!((r.positions[0][0] - (-5. + 0.5)).abs() < 1e-4);
        let c = layout(&f, "0", -50., 50., 0., 10.);
        assert!((c.positions[0][0] - (-2.5 + 0.5)).abs() < 1e-4);
        // An empty text is a degenerate triangle.
        assert_eq!(layout(&f, "", 0., 1., 0., 1.).indices, [0, 1, 2]);
    }

    #[test]
    fn a_text_box_in_the_x_z_plane_lays_its_glyphs_there() {
        let f = sample();
        let t = osd::TextLayout {
            font: String::new(),
            height: 10.,
            depth: 0.,
            h_align: 0,
            v_align: 0,
            wrapping: 0,
            line_spacing: 1.,
            tracking: 0.,
            case: 0,
        };
        let flat = text_mesh(&f, "0", &t, &[0., 0., -10., 100., 0., 0.]);
        let upright = text_mesh(&f, "0", &t, &[0., 0., 0., 100., 10., 0.]);
        // The same glyph, its y turned to -z (the box's top at z -10) and its
        // depth to y; x unchanged.
        for (p, q) in flat.positions.iter().zip(&upright.positions) {
            assert!((p[0] - q[0]).abs() < 1e-4 && (p[1] - q[2]).abs() < 1e-4 && (p[2] + q[1]).abs() < 1e-4, "{p:?} {q:?}");
        }
        assert!(flat.positions.iter().all(|p| (-10. ..=0.).contains(&p[2])));
        assert!(flat.normals.iter().all(|n| *n == [0., 1., -0.] || *n == [0., 1., 0.]));
    }

    /// A window resized mid-race (F737): the display material must change with
    /// the image, so the UI rebinds the resized texture instead of showing the
    /// OSD frozen as it was at the resize.
    #[test]
    fn a_resize_rebinds_the_display_material_to_the_resized_image() {
        #[derive(Resource, Default)]
        struct Seen(Vec<AssetEvent<OsdDisplay>>);
        fn record(mut seen: ResMut<Seen>, mut events: MessageReader<AssetEvent<OsdDisplay>>) {
            seen.0.extend(events.read().cloned());
        }
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::asset::AssetPlugin::default()))
            .init_asset::<Image>()
            .init_asset::<OsdDisplay>()
            .init_resource::<Seen>()
            .add_systems(Last, record);
        let world = app.world_mut();
        let image = world.resource_mut::<Assets<Image>>().add(Image::new_target_texture(16, 16, TextureFormat::Rgba8UnormSrgb, None));
        let display = world.resource_mut::<Assets<OsdDisplay>>().add(OsdDisplay { image: image.clone() });
        let mut osd = Osd {
            entities: HashMap::new(),
            tree: Vec::new(),
            rest: Vec::new(),
            cubes: (Handle::default(), Handle::default()),
            anims: Vec::new(),
            anim_index: HashMap::new(),
            materials: Vec::new(),
            texts: HashMap::new(),
            fonts: Vec::new(),
            shown: HashMap::new(),
            offsets: HashMap::new(),
            node_materials: HashMap::new(),
            scrolls: HashMap::new(),
            image: image.clone(),
            display: display.clone(),
            view: GlobalTransform::IDENTITY,
            projection: PerspectiveProjection::default(),
            active: true,
            size: UVec2::ZERO,
        };
        let mut resize = |app: &mut App, size: UVec2| {
            let world = app.world_mut();
            world.resource_scope(|world, mut images: Mut<Assets<Image>>| {
                set_size(&mut osd, size, &mut images, &mut world.resource_mut::<Assets<OsdDisplay>>())
            })
        };
        // The first size (spawn), then frames at that size.
        assert!(resize(&mut app, UVec2::new(1280, 800)));
        app.update();
        app.world_mut().resource_mut::<Seen>().0.clear();
        assert!(!resize(&mut app, UVec2::new(1280, 800)));
        app.update();
        assert!(app.world().resource::<Seen>().0.is_empty(), "an unchanged size leaves the material alone");
        // Maximised mid-race.
        assert!(resize(&mut app, UVec2::new(2560, 1369)));
        app.update();
        let modified = AssetEvent::Modified { id: display.id() };
        assert!(app.world().resource::<Seen>().0.contains(&modified), "{:?}", app.world().resource::<Seen>().0);
        let size = app.world().resource::<Assets<Image>>().get(&image).unwrap().size();
        assert_eq!(size, UVec2::new(2560, 1369));
    }

    /// The split pill flip (`playerN` `in`: -90, 0, +90 degrees): the pill
    /// flips in to the range's middle, stays there, then flips out.
    #[test]
    fn a_partial_range_plays_once_and_stops_at_its_fraction() {
        let key = |time: f32, value: f32| osd::Key { time, value, in_value: value, in_time: time, out_value: value, out_time: time };
        let flip = osd::Animation {
            id: "Anim_player1".into(),
            ranges: vec![osd::Range { name: "in".into(), start: 0., end: 1. / 3. }],
            channels: vec![osd::Channel {
                target: "player1".into(),
                semantic: osd::Semantic::Rotation,
                components: vec![osd::Curve(vec![key(0., -std::f32::consts::FRAC_PI_2), key(1. / 6., 0.), key(1. / 3., std::f32::consts::FRAC_PI_2)])],
            }],
            external: vec![],
            events: Vec::new(),
            default_frame: 0.,
        };
        let mut osd = Osd {
            entities: HashMap::new(),
            tree: Vec::new(),
            rest: Vec::new(),
            cubes: (Handle::default(), Handle::default()),
            anims: vec![Anim { data: flip, time: 0., stop: 0., targets: vec![Target::None] }],
            anim_index: HashMap::from([("player1".to_owned(), 0)]),
            materials: Vec::new(),
            texts: HashMap::new(),
            fonts: Vec::new(),
            shown: HashMap::new(),
            offsets: HashMap::new(),
            node_materials: HashMap::new(),
            scrolls: HashMap::new(),
            image: Handle::default(),
            display: Handle::default(),
            view: GlobalTransform::IDENTITY,
            projection: PerspectiveProjection::default(),
            active: true,
            size: UVec2::ZERO,
        };
        let step = |osd: &mut Osd, dt: f32| {
            let a = &mut osd.anims[0];
            a.time = (a.time + dt).min(a.stop);
        };
        osd.play_part_once("player1", "in", 0., 0.5);
        assert!(!osd.finished("player1"));
        step(&mut osd, 0.1);
        // Asked again every frame: it keeps playing, then rests at the middle.
        osd.play_part_once("player1", "in", 0., 0.5);
        assert!((osd.anims[0].time - 0.1).abs() < 1e-6);
        step(&mut osd, 1.);
        osd.play_part_once("player1", "in", 0., 0.5);
        assert!(osd.finished("player1"));
        assert!((osd.anims[0].time - 1. / 6.).abs() < 1e-6);
        assert!(osd.anims[0].data.channels[0].components[0].sample(osd.anims[0].time).abs() < 1e-4);
        // Out: from the middle to the end.
        osd.play_part_once("player1", "in", 0.5, 1.);
        assert!(!osd.finished("player1"));
        step(&mut osd, 1.);
        assert!(osd.finished("player1"));
        assert!((osd.anims[0].time - 1. / 3.).abs() < 1e-6);
        // In again restarts from the range start.
        osd.play_part_once("player1", "in", 0., 0.5);
        assert_eq!(osd.anims[0].time, 0.);
    }

    #[test]
    fn cubes_name_every_failed_source() {
        let read = |p: &str| -> Result<Vec<u8>, String> { Err(format!("{p}: missing")) };
        let e = cubes(&read, &[("a.pssg", ["x.dds"]), ("b.pssg", ["y.dds"])]).unwrap_err();
        assert_eq!(e, "a.pssg: missing; b.pssg: missing");
        assert!(cubes::<1>(&read, &[]).is_err());
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT purchased source"]
    fn the_original_cubes_fall_back_in_order() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let read = |p: &str| std::fs::read(root.join(p)).map_err(|e| format!("{p}: {e}"));
        let osd = ("Osd/dial2.pssg", ["env_soft.dds", "env_sharp.dds"]);
        // b_persistent holds only reflection_load.dds, so the results'
        // reflection_soft/sharp fall back to the race OSD's cubes.
        let (images, from) =
            cubes(&read, &[("frontend/b_persistent.pssg", ["reflection_soft.dds", "reflection_sharp.dds"]), osd]).unwrap();
        assert_eq!(from, "Osd/dial2.pssg");
        assert_eq!(images[0].texture_descriptor.format, TextureFormat::Rgba8Unorm);
        assert_eq!(images[0].texture_descriptor.size.depth_or_array_layers, 6);
        let (_, from) = cubes(&read, &[("frontend/b_persistent.pssg", ["reflection_load.dds"])]).unwrap();
        assert_eq!(from, "frontend/b_persistent.pssg");
        let (_, from) = cubes(&read, &[("frontend/b_garage_01.pssg", ["ref_garage_soft.dds", "ref_garage_sharp.dds"]), osd]).unwrap();
        assert_eq!(from, "frontend/b_garage_01.pssg");
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT purchased source"]
    fn the_original_time_font_sets_a_lap_time_on_its_baseline() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let bytes = std::fs::read(root.join("frontend/fonts/swiss_time.pssg")).unwrap();
        let f = font(osd::read_font(&grid_formats::pssg::read_tree(&bytes).unwrap()).unwrap());
        // race_time's box: 0..105 x 0..24, left aligned.
        let m = layout(&f, "0:05.600", 0., 105.04, 0., 24.);
        assert_eq!(m.positions.len(), 8 * 4);
        // The next frame's time has the same vertices and triangles, so
        // `texts` streams it instead of replacing the mesh (F398).
        let next = layout(&f, "0:05.617", 0., 105.04, 0., 24.);
        assert_eq!((next.positions.len(), &next.indices), (m.positions.len(), &m.indices));
        assert_ne!(next.positions, m.positions);
        let em = 24. / (f.ascender - f.descender);
        let baseline = -f.descender * em;
        // The quads carry atlas padding: the '.' (5th glyph) bottom sits
        // the padding below the baseline, as the '0' (first glyph) does.
        let bottom = |g: usize| m.positions[g * 4..g * 4 + 4].iter().map(|p| p[1]).fold(f32::MAX, f32::min);
        // (the 0 overshoots the baseline by 0.015 em: vertical bearing 757, height 772).
        assert!((bottom(4) - bottom(0)).abs() < 0.03 * em, "{} {}", bottom(4), bottom(0));
        assert!((bottom(0) - baseline).abs() < 0.15 * em);
        // Starts at the box's left edge and fits its width.
        let right = m.positions.iter().map(|p| p[0]).fold(f32::MIN, f32::max);
        assert!(m.positions[0][0] < 0.5 && right < 105.04 + 2., "{right}");
    }
}


/// The OSD overlay pass: draws the scene's visible mesh nodes into the OSD
/// image at the start of the frame's render graph (before
/// `camera_driver`), with the scene camera's view. It replaces the former
/// OSD camera (F853) and repeats what that camera did, so the image is the
/// same: blended draws back to front by the view-space depth of each mesh's
/// bounds centre (Bevy 0.19's retained `Transparent3d` order, including its
/// tie order), 4x MSAA resolved into the sRGB image (1 sample on Android),
/// a transparent clear, no depth test. The viewer camera's UI composites the
/// image after tonemapping and post-processing.
pub(crate) mod overlay {
    use super::{Osd, OsdDraw, OsdMaterial, Params, SHADER};
    use bevy::app::SubApp;
    use bevy::camera::CameraProjection;
    use bevy::core_pipeline::schedule::camera_driver;
    use bevy::math::{Affine3, Affine3Ext, FloatOrd};
    use bevy::prelude::*;
    use bevy::render::mesh::allocator::MeshAllocator;
    use bevy::render::mesh::{RenderMesh, RenderMeshBufferInfo};
    use bevy::render::render_asset::RenderAssets;
    use bevy::render::render_resource::binding_types::{sampler, texture_2d, texture_cube, uniform_buffer};
    use bevy::render::render_resource::{
        BindGroup, BindGroupEntries, BindGroupLayoutDescriptor, BindGroupLayoutEntries, BlendState,
        CachedRenderPipelineId, ColorTargetState, ColorWrites, DynamicUniformBuffer, Extent3d, FragmentState,
        LoadOp, MultisampleState, Operations, PipelineCache, PrimitiveState, RenderPassColorAttachment,
        RenderPassDescriptor, RenderPipelineDescriptor, SamplerBindingType, SamplerId, ShaderStages, ShaderType,
        StoreOp, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
        TextureViewId, UniformBuffer, VertexFormat, VertexState, VertexStepMode,
    };
    use bevy::render::renderer::{RenderContext, RenderDevice, RenderGraph, RenderGraphSystems, RenderQueue};
    use bevy::render::texture::{CachedTexture, GpuImage, TextureCache};
    use bevy::render::{Extract, ExtractSchedule, Render, RenderStartup, RenderSystems};
    use std::collections::{HashMap, HashSet};

    /// The former OSD camera's sample count.
    const SAMPLES: u32 = if cfg!(target_os = "android") { 1 } else { 4 };
    const FORMAT: TextureFormat = TextureFormat::Rgba8UnormSrgb;

    pub(super) fn install(render: &mut SubApp) {
        render
            .init_resource::<Frame>()
            .init_resource::<Prepared>()
            .add_systems(RenderStartup, initialize)
            .add_systems(ExtractSchedule, extract)
            .add_systems(Render, prepare.in_set(RenderSystems::PrepareBindGroups))
            .add_systems(RenderGraph, draw.in_set(RenderGraphSystems::Render).before(camera_driver));
    }

    /// One visible mesh node, as extracted.
    struct Item {
        entity: Entity,
        mesh: AssetId<Mesh>,
        world_from_local: Affine3,
        centre: Option<Vec3>,
        images: [AssetId<Image>; 4],
        params: Params,
    }

    /// The main world's OSD state for this frame.
    #[derive(Resource, Default)]
    struct Frame {
        active: bool,
        image: Option<AssetId<Image>>,
        world_from_view: GlobalTransform,
        clip_from_view: Mat4,
        items: Vec<Item>,
    }

    #[derive(Clone, Copy, Default, ShaderType)]
    struct ViewUniform {
        clip_from_world: Mat4,
        world_position: Vec3,
    }

    #[derive(Clone, Copy, Default, ShaderType)]
    struct DrawUniform {
        world_from_local: [Vec4; 3],
        p: Params,
    }

    type TextureKey = ([TextureViewId; 4], [SamplerId; 4]);

    /// One draw of the frame, in draw order.
    struct Ready {
        offset: u32,
        textures: TextureKey,
        mesh: AssetId<Mesh>,
    }

    #[derive(Resource, Default)]
    struct Prepared {
        /// Draw order kept across frames, as Bevy's retained sorted phase
        /// keeps its items (the order of equal depths depends on it).
        order: Vec<Entity>,
        view: UniformBuffer<ViewUniform>,
        draws: DynamicUniformBuffer<DrawUniform>,
        group: Option<BindGroup>,
        textures: HashMap<TextureKey, (BindGroup, bool)>,
        list: Vec<Ready>,
        msaa: Option<CachedTexture>,
    }

    #[derive(Resource)]
    struct Pipeline {
        layouts: [BindGroupLayoutDescriptor; 2],
        id: CachedRenderPipelineId,
    }

    fn initialize(mut commands: Commands, cache: Res<PipelineCache>) {
        let view = BindGroupLayoutDescriptor::new(
            "osd overlay view",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::VERTEX_FRAGMENT,
                (uniform_buffer::<ViewUniform>(false), uniform_buffer::<DrawUniform>(true)),
            ),
        );
        let float = TextureSampleType::Float { filterable: true };
        let textures = BindGroupLayoutDescriptor::new(
            "osd overlay textures",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    texture_2d(float),
                    sampler(SamplerBindingType::Filtering),
                    texture_2d(float),
                    sampler(SamplerBindingType::Filtering),
                    texture_cube(float),
                    sampler(SamplerBindingType::Filtering),
                    texture_cube(float),
                    sampler(SamplerBindingType::Filtering),
                ),
            ),
        );
        let id = cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some("osd overlay".into()),
            layout: vec![view.clone(), textures.clone()],
            vertex: VertexState {
                shader: SHADER,
                shader_defs: vec![],
                entry_point: Some("vertex".into()),
                buffers: vec![bevy::mesh::VertexBufferLayout::from_vertex_formats(
                    VertexStepMode::Vertex,
                    [VertexFormat::Float32x3, VertexFormat::Float32x3, VertexFormat::Float32x2],
                )],
            },
            fragment: Some(FragmentState {
                shader: SHADER,
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: FORMAT,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            primitive: PrimitiveState { cull_mode: None, ..default() },
            multisample: MultisampleState { count: SAMPLES, ..default() },
            ..default()
        });
        commands.insert_resource(Pipeline { layouts: [view, textures], id });
    }

    fn extract(
        mut frame: ResMut<Frame>,
        osd: Extract<Option<Res<Osd>>>,
        draws: Extract<Query<(Entity, &OsdDraw, &GlobalTransform, &InheritedVisibility)>>,
        materials: Extract<Res<Assets<OsdMaterial>>>,
    ) {
        frame.items.clear();
        let Some(osd) = osd.as_ref() else {
            frame.active = false;
            frame.image = None;
            return;
        };
        frame.active = osd.active;
        frame.image = Some(osd.image.id());
        frame.world_from_view = osd.view;
        frame.clip_from_view = osd.projection.get_clip_from_view();
        if !osd.active {
            return;
        }
        for (entity, draw, global, visible) in &draws {
            if !visible.get() {
                continue;
            }
            let Some(m) = materials.get(&draw.material) else {
                continue;
            };
            frame.items.push(Item {
                entity,
                mesh: draw.mesh.id(),
                world_from_local: Affine3::from(global.affine()),
                centre: draw.centre,
                images: [m.diffuse.id(), m.glow.id(), m.soft.id(), m.sharp.id()],
                params: m.params,
            });
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare(
        frame: Res<Frame>,
        pipeline: Option<Res<Pipeline>>,
        mut prepared: ResMut<Prepared>,
        images: Res<RenderAssets<GpuImage>>,
        meshes: Res<RenderAssets<RenderMesh>>,
        allocator: Res<MeshAllocator>,
        mut textures: ResMut<TextureCache>,
        device: Res<RenderDevice>,
        queue: Res<RenderQueue>,
        cache: Res<PipelineCache>,
    ) {
        let p = &mut *prepared;
        p.list.clear();
        p.msaa = None;
        let (Some(pipeline), Some(image), true) = (pipeline, frame.image, frame.active) else {
            return;
        };
        let Some(target) = images.get(image) else {
            return;
        };
        if SAMPLES > 1 {
            let size = target.texture_descriptor.size;
            p.msaa = Some(textures.get(
                &device,
                TextureDescriptor {
                    label: Some("osd overlay msaa"),
                    size: Extent3d { width: size.width, height: size.height, depth_or_array_layers: 1 },
                    mip_level_count: 1,
                    sample_count: SAMPLES,
                    dimension: TextureDimension::D2,
                    format: FORMAT,
                    usage: TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                },
            ));
        }
        // Drawable this frame: visible, mesh and all four images on the GPU
        // (Bevy queues an item only once its mesh and material are prepared).
        let mut ready: HashMap<Entity, (usize, [&GpuImage; 4], Vec3)> = HashMap::new();
        for (i, item) in frame.items.iter().enumerate() {
            let Some(mesh) = meshes.get(item.mesh) else {
                continue;
            };
            if !matches!(mesh.buffer_info, RenderMeshBufferInfo::Indexed { .. })
                || allocator.mesh_vertex_slice(&item.mesh).is_none()
                || allocator.mesh_index_slice(&item.mesh).is_none()
            {
                continue;
            }
            let [Some(a), Some(b), Some(c), Some(d)] = item.images.map(|id| images.get(id)) else {
                continue;
            };
            let local = item.centre.unwrap_or(mesh.aabb_center);
            ready.insert(item.entity, (i, [a, b, c, d], item.world_from_local.transform_point3(local)));
        }
        let row = Mat4::from(frame.world_from_view.affine().inverse()).row(2);
        let depth: HashMap<Entity, f32> = ready.iter().map(|(e, r)| (*e, row.dot(r.2.extend(1.0)) + 0.0)).collect();
        retained_order(&mut p.order, &depth);

        let world_from_view = frame.world_from_view.to_matrix();
        p.view.set(ViewUniform {
            clip_from_world: frame.clip_from_view * world_from_view.inverse(),
            world_position: frame.world_from_view.translation(),
        });
        p.view.write_buffer(&device, &queue);
        p.draws.clear();
        for v in p.textures.values_mut() {
            v.1 = false;
        }
        let texture_layout = cache.get_bind_group_layout(&pipeline.layouts[1]);
        for e in &p.order {
            let (i, [a, b, c, d], _) = ready[e];
            let item = &frame.items[i];
            let offset = p.draws.push(&DrawUniform {
                world_from_local: item.world_from_local.to_transpose(),
                p: item.params,
            });
            let key = (
                [a, b, c, d].map(|g| g.texture_view.id()),
                [a, b, c, d].map(|g| g.sampler.id()),
            );
            p.textures
                .entry(key)
                .or_insert_with(|| {
                    let group = device.create_bind_group(
                        "osd overlay textures",
                        &texture_layout,
                        &BindGroupEntries::sequential((
                            &a.texture_view,
                            &a.sampler,
                            &b.texture_view,
                            &b.sampler,
                            &c.texture_view,
                            &c.sampler,
                            &d.texture_view,
                            &d.sampler,
                        )),
                    );
                    (group, true)
                })
                .1 = true;
            p.list.push(Ready { offset, textures: key, mesh: item.mesh });
        }
        p.textures.retain(|_, v| v.1);
        p.draws.write_buffer(&device, &queue);
        p.group = match (p.view.binding(), p.draws.binding()) {
            (Some(view), Some(draws)) => Some(device.create_bind_group(
                "osd overlay view",
                &cache.get_bind_group_layout(&pipeline.layouts[0]),
                &BindGroupEntries::sequential((view, draws)),
            )),
            _ => None,
        };
    }

    /// Bevy 0.19's retained blended phase order, kept from frame to frame:
    /// items no longer drawn leave by swap-remove (in entity order), new ones
    /// join at the end (in entity order), then a stable sort by view depth.
    /// Equal depths therefore keep the order the phase had, as before.
    fn retained_order(order: &mut Vec<Entity>, depth: &HashMap<Entity, f32>) {
        let mut gone: Vec<Entity> = order.iter().copied().filter(|e| !depth.contains_key(e)).collect();
        gone.sort_unstable();
        for e in gone {
            if let Some(k) = order.iter().position(|x| *x == e) {
                order.swap_remove(k);
            }
        }
        let kept: HashSet<Entity> = order.iter().copied().collect();
        let mut new: Vec<Entity> = depth.keys().copied().filter(|e| !kept.contains(e)).collect();
        new.sort_unstable();
        order.extend(new);
        order.sort_by_key(|e| FloatOrd(depth[e]));
    }

    #[allow(clippy::too_many_arguments)]
    fn draw(
        frame: Res<Frame>,
        prepared: Res<Prepared>,
        pipeline: Option<Res<Pipeline>>,
        cache: Res<PipelineCache>,
        images: Res<RenderAssets<GpuImage>>,
        meshes: Res<RenderAssets<RenderMesh>>,
        allocator: Res<MeshAllocator>,
        mut ctx: RenderContext,
    ) {
        let (Some(pipeline), Some(image), true) = (pipeline, frame.image, frame.active) else {
            return;
        };
        let Some(target) = images.get(image) else {
            return;
        };
        let (view, resolve_target) = match &prepared.msaa {
            Some(msaa) => (&msaa.default_view, Some(&*target.texture_view)),
            None => (&target.texture_view, None),
        };
        // The pass always begins, so the image is cleared even when nothing
        // is drawn (as the former camera's pass).
        let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("osd overlay"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target,
                ops: Operations { load: LoadOp::Clear(LinearRgba::NONE.into()), store: StoreOp::Store },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        let (Some(render_pipeline), Some(group)) = (cache.get_render_pipeline(pipeline.id), &prepared.group) else {
            return;
        };
        pass.set_render_pipeline(render_pipeline);
        for r in &prepared.list {
            let (Some(mesh), Some(v), Some(i), Some((textures, _))) = (
                meshes.get(r.mesh),
                allocator.mesh_vertex_slice(&r.mesh),
                allocator.mesh_index_slice(&r.mesh),
                prepared.textures.get(&r.textures),
            ) else {
                continue;
            };
            let RenderMeshBufferInfo::Indexed { count, index_format } = mesh.buffer_info else {
                continue;
            };
            pass.set_bind_group(0, group, &[r.offset]);
            pass.set_bind_group(1, textures, &[]);
            pass.set_vertex_buffer(0, v.buffer.slice(..));
            pass.set_index_buffer(i.buffer.slice(..), index_format);
            pass.draw_indexed(i.range.start..i.range.start + count, v.range.start as i32, 0..1);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn retained_order_keeps_ties_and_swap_removes() {
            let e: Vec<Entity> = (1..=5).map(Entity::from_raw_u32).map(Option::unwrap).collect();
            let mut order = Vec::new();
            // Entity order is Bevy's `Ord` (by bits; the index is stored inverted, so for
            // generation 0 a higher index comes first), as in Bevy's phase.
            assert!(e[3] < e[2] && e[2] < e[1] && e[1] < e[0]);
            // First frame: all new, joined in entity order, then sorted (ties stay in entity order).
            let d = HashMap::from([(e[3], 1.0), (e[0], 1.0), (e[2], -2.0), (e[1], 1.0)]);
            retained_order(&mut order, &d);
            assert_eq!(order, [e[2], e[3], e[1], e[0]]);
            // e[3] leaves: swap-remove moves the last (e[0]) into its slot, so the tie between
            // e[0] and e[1] flips; e[4] joins at the end.
            let d = HashMap::from([(e[0], 1.0), (e[2], -2.0), (e[1], 1.0), (e[4], 1.0)]);
            retained_order(&mut order, &d);
            assert_eq!(order, [e[2], e[0], e[1], e[4]]);
        }
    }
}
