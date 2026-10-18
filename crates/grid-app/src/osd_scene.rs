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
//! The scene renders through its own camera into a transparent image, which
//! is drawn under every other UI node, so the pause menu, the post-race
//! screens and the touch zones stay on top and the 3D post-processing does
//! not touch it (AUTHORED composition).
use bevy::{
    asset::{RenderAssetUsages, uuid_handle},
    camera::{RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, MeshVertexBufferLayoutRef, PrimitiveTopology},
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{
        AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, CompareFunction,
        Extent3d, RenderPipelineDescriptor, SpecializedMeshPipelineError, TextureDimension,
        TextureFormat, TextureViewDescriptor, TextureViewDimension,
    },
    shader::ShaderRef,
    ui_render::prelude::{MaterialNode, UiMaterial, UiMaterialKey, UiMaterialPlugin},
};
use grid_formats::osd;
use std::collections::HashMap;
use std::path::Path;

const SHADER: Handle<Shader> = uuid_handle!("6a4a8f0e-1d0f-4f63-9a4e-6c8f1e7b2d31");
const DISPLAY_SHADER: Handle<Shader> = uuid_handle!("0c7e5a52-93a4-4b4e-8f63-2a9d6b1e4c70");

/// The render layer of the OSD meshes and camera.
pub const LAYER: usize = 23;

/// The fonts of `Osd/fontDescriptions.xml`, in order; a text node's
/// `rd4text parameter` indexes this list [D order, I meaning of the index].
pub const FONTS: [&str; 5] = [
    "microgramma_osd.PSSG",
    "swiss_bold_osd_2d.PSSG",
    "micro_drift.pssg",
    "micro_lap.pssg",
    "swiss_time.pssg",
];

/// One shader instance of the scene.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct OsdMaterial {
    #[texture(0)]
    #[sampler(1)]
    diffuse: Handle<Image>,
    #[texture(2)]
    #[sampler(3)]
    glow: Handle<Image>,
    #[texture(4, dimension = "cube")]
    #[sampler(5)]
    soft: Handle<Image>,
    #[texture(6, dimension = "cube")]
    #[sampler(7)]
    sharp: Handle<Image>,
    #[uniform(8)]
    params: Params,
}

/// `osd.wgsl` parameters.
#[derive(Clone, Copy, Debug, Default, PartialEq, bevy::render::render_resource::ShaderType)]
pub struct Params {
    colour: Vec4,
    a: Vec4,
    b: Vec4,
    c: Vec4,
}

impl Material for OsdMaterial {
    fn vertex_shader() -> ShaderRef {
        SHADER.into()
    }
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
    }
    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }
    fn enable_prepass() -> bool {
        false
    }
    fn enable_shadows() -> bool {
        false
    }
    fn specialize(
        _: &MaterialPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _: &MeshVertexBufferLayoutRef,
        _: MaterialPipelineKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = None;
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            depth.depth_write_enabled = Some(false);
            depth.depth_compare = Some(CompareFunction::Always);
        }
        // Premultiplied result over a transparent target.
        if let Some(target) = descriptor
            .fragment
            .as_mut()
            .and_then(|f| f.targets.first_mut())
            .and_then(|t| t.as_mut())
        {
            target.blend = Some(BlendState {
                color: BlendComponent {
                    src_factor: BlendFactor::SrcAlpha,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
                alpha: BlendComponent {
                    src_factor: BlendFactor::One,
                    dst_factor: BlendFactor::OneMinusSrcAlpha,
                    operation: BlendOperation::Add,
                },
            });
        }
        Ok(())
    }
}

/// The OSD image drawn as a UI node (premultiplied blend).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct OsdDisplay {
    #[texture(0)]
    #[sampler(1)]
    image: Handle<Image>,
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
struct Font {
    glyphs: HashMap<char, Glyph>,
    ascender: f32,
    descender: f32,
}

fn font(f: osd::Font) -> Font {
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
    let mut pen = if x0.abs() < eps {
        x0
    } else if x1.abs() < eps {
        x1 - width
    } else {
        (x0 + x1) / 2. - width / 2.
    };
    let baseline = y0 - font.descender * em;
    let mut out = osd::Mesh::default();
    for (i, c) in chars.iter().enumerate() {
        if let Some(g) = font.glyphs.get(c) {
            let base = out.positions.len() as u16;
            let (ox, oy) = (pen + g.bearing_x * em, baseline + g.bearing_y * em);
            for (k, p) in g.mesh.positions.iter().enumerate() {
                out.positions.push([ox + p[0] * em, oy + p[1] * em, p[2] * em]);
                out.normals.push(g.mesh.normals.get(k).copied().unwrap_or([0., 0., 1.]));
                out.uvs.push(g.mesh.uvs.get(k).copied().unwrap_or([0., 0.]));
            }
            out.indices.extend(g.mesh.indices.iter().map(|i| i + base));
        }
        pen += step(i) * em;
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

fn cube(c: &osd::Cubemap<'_>) -> Image {
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

struct TextNode {
    font: usize,
    bounds: [f32; 4],
    mesh: Handle<Mesh>,
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
    /// The environment cubemaps (soft, sharp) for new materials.
    cubes: (Handle<Image>, Handle<Image>),
    anims: Vec<Anim>,
    anim_index: HashMap<String, usize>,
    materials: Vec<MaterialState>,
    texts: HashMap<String, TextNode>,
    fonts: Vec<Font>,
    shown: HashMap<Entity, bool>,
    /// Added to an animated node's translation (authored layout moves).
    offsets: HashMap<Entity, Vec3>,
    image: Handle<Image>,
    camera: Entity,
    size: UVec2,
}

impl Osd {
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
    /// The saved local bounds of a node (min xyz, max xyz).
    pub fn bounds(&self, node: &str) -> Option<[f32; 6]> {
        self.tree.iter().find(|(id, ..)| id == node).map(|t| t.2)
    }
    /// A flat colour material (`osd3d_flat_colour`) for authored additions.
    pub fn colour_material(&self, colour: Vec3, alpha: f32) -> OsdMaterial {
        let mut params = Params::default();
        params.colour = colour.extend(alpha);
        params.b.x = 1.;
        params.c.x = 1.;
        params.c.w = 1.;
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
    /// Moves an animated node by `offset` on top of its animated translation.
    pub fn offset(&mut self, node: &str, offset: Vec3) {
        if let Some(&e) = self.entities.get(node) {
            self.offsets.insert(e, offset);
        }
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
    app.add_plugins((MaterialPlugin::<OsdMaterial>::default(), UiMaterialPlugin::<OsdDisplay>::default()));
    let world = app.world_mut();
    world.resource_mut::<Assets<Shader>>().insert(SHADER.id(), Shader::from_wgsl(include_str!("osd.wgsl"), "grid_osd.wgsl")).map_err(|e| e.to_string())?;
    world
        .resource_mut::<Assets<Shader>>()
        .insert(DISPLAY_SHADER.id(), Shader::from_wgsl(include_str!("osd_display.wgsl"), "grid_osd_display.wgsl"))
        .map_err(|e| e.to_string())?;
    let sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    let mut images = HashMap::new();
    {
        let mut assets = world.resource_mut::<Assets<Image>>();
        for t in &scene.textures {
            let (dds, _) = t.validated_dds();
            let image = crate::texture_upload::image(&dds, sampler.clone(), RenderAssetUsages::RENDER_WORLD).map_err(|e| e.to_string())?;
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
    app.insert_resource(Installed)
        .add_systems(Update, spawn)
        .add_systems(PostUpdate, (resize, animate, texts, visibility).chain().before(TransformSystems::Propagate));
    Ok(())
}

use bevy::transform::TransformSystems;

#[derive(Component)]
pub struct OsdCamera;

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
    let c = &loaded.camera;
    let camera = commands
        .spawn((
            OsdCamera,
            Camera3d::default(),
            Camera {
                order: -20,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(image.clone().into()),
            Projection::Perspective(PerspectiveProjection {
                fov: c.fov_degrees.to_radians(),
                near: c.near.max(1.),
                far: c.far,
                aspect_ratio: 16. / 9.,
                ..default()
            }),
            transform(&c.world),
            Tonemapping::None,
            if cfg!(target_os = "android") { Msaa::Off } else { Msaa::Sample4 },
            RenderLayers::layer(LAYER),
        ))
        .id();
    commands.spawn((
        Node {
            position_type: PositionType::Absolute,
            width: percent(100.),
            height: percent(100.),
            ..default()
        },
        MaterialNode(displays.add(OsdDisplay { image: image.clone() })),
        GlobalZIndex(-1000),
        Pickable::IGNORE,
        UiTargetCamera(viewer),
    ));
    // Nodes, in tree order (parents first).
    let mut entities: Vec<Entity> = Vec::with_capacity(loaded.nodes.len());
    let mut by_id = HashMap::new();
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
                    let handle = meshes.add(bevy_mesh(&layout(&loaded.fonts[font as usize], "", lo[0], hi[0], lo[1], hi[1])));
                    texts.insert(
                        n.id.clone(),
                        TextNode {
                            font: font as usize,
                            bounds: [lo[0], hi[0], lo[1], hi[1]],
                            mesh: handle.clone(),
                            text: String::new(),
                            want: String::new(),
                        },
                    );
                    handle
                }
                None => mesh.clone(),
            };
            e.insert((
                Mesh3d(mesh),
                MeshMaterial3d(loaded.materials[r.material].handle.clone()),
                RenderLayers::layer(LAYER),
                bevy::camera::visibility::NoFrustumCulling,
            ));
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
        cubes: loaded.cubes.clone(),
        anims,
        anim_index,
        materials: loaded.materials.clone(),
        texts,
        fonts: loaded.fonts.clone(),
        shown: HashMap::new(),
        offsets: HashMap::new(),
        image,
        camera,
        size: UVec2::ZERO,
    });
}

/// Keeps the OSD image at the 3D view's size and the camera's aspect.
fn resize(
    osd: Option<ResMut<Osd>>,
    viewer: Query<&Camera, With<crate::ViewerCamera>>,
    mut images: ResMut<Assets<Image>>,
    mut projection: Query<&mut Projection, With<OsdCamera>>,
) {
    let (Some(mut osd), Some(camera)) = (osd, viewer.iter().next()) else {
        return;
    };
    let Some(size) = camera.physical_target_size().filter(|s| s.x > 0 && s.y > 0) else {
        return;
    };
    if size == osd.size {
        return;
    }
    osd.size = size;
    if let Some(mut image) = images.get_mut(&osd.image) {
        image.resize(Extent3d { width: size.x, height: size.y, depth_or_array_layers: 1 });
    }
    if let Ok(mut p) = projection.get_mut(osd.camera)
        && let Projection::Perspective(p) = p.as_mut()
    {
        p.aspect_ratio = size.x as f32 / size.y as f32;
    }
}

/// Advances the animations and writes node transforms and material values.
fn animate(
    osd: Option<ResMut<Osd>>,
    time: Option<Res<Time<Real>>>,
    mut transforms: Query<&mut Transform>,
    mut materials: ResMut<Assets<OsdMaterial>>,
) {
    let Some(mut osd) = osd else {
        return;
    };
    let dt = time.map_or(0., |t| t.delta_secs());
    let osd = &mut *osd;
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
                            next.translation = Vec3::new(v[0], v[1], v[2]) + osd.offsets.get(e).copied().unwrap_or_default();
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
    for state in &mut osd.materials {
        if state.dirty {
            state.dirty = false;
            if let Some(mut m) = materials.get_mut(&state.handle) {
                m.params = state.params;
            }
        }
    }
}

/// Rebuilds the glyph meshes of changed texts.
fn texts(osd: Option<ResMut<Osd>>, mut meshes: ResMut<Assets<Mesh>>) {
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
        let mesh = layout(&osd.fonts[t.font], &t.text, x0, x1, y0, y1);
        if let Some(mut m) = meshes.get_mut(&t.mesh) {
            *m = bevy_mesh(&mesh);
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
    #[ignore = "requires GRID_GAME_ROOT purchased source"]
    fn the_original_time_font_sets_a_lap_time_on_its_baseline() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let bytes = std::fs::read(root.join("frontend/fonts/swiss_time.pssg")).unwrap();
        let f = font(osd::read_font(&grid_formats::pssg::read_tree(&bytes).unwrap()).unwrap());
        // race_time's box: 0..105 x 0..24, left aligned.
        let m = layout(&f, "0:05.600", 0., 105.04, 0., 24.);
        assert_eq!(m.positions.len(), 8 * 4);
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
