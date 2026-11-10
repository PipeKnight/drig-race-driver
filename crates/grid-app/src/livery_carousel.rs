//! The livery designer's 3D pill carousel (docs/LIVERY_CAROUSEL.md): the
//! `livery_editor_root` glyphs of feNonPersistentDatabase, seen through
//! `carousel_cam_1` and drawn with the shared front-end materials (`fe3d`)
//! into an image under the career screen's Team Branding panel. One path for
//! every carousel kind, car and livery table: the career screen names the
//! kind, the table's choices and the selected index ([`View`]).
//!
//! [D] The tree, materials, text layout and camera; the ring layout of
//! 0x50D080 (item i of a carousel is the template turned by i × separation
//! about the ring pivot); the separation 0.0145 and the carousel glyph names
//! of the `ScreenLiveryDesignerRoot` settings (states.bin); the colour pill's
//! RGB / 255 (0x52B760 → 0x514950); the clamp at the ends (0x500E40); the
//! lighting zone `zone_22` (livery editor) of `fe_paramcontrol.xml`.
//! [I] The pivot is the template's parent node; the pattern texture is the
//! pattern library's name + `_` + the row's `fe_image_name` (b_garage.pssg);
//! the arrows' pose at t = 1 of their fold animation; the 0.2 s turn eased by
//! t + t² − t³.
use crate::fe3d::{self, Fe3dMaterial};
use crate::osd_scene::{self, OsdDisplay};
use crate::results_art::{IDENTITY, PACKAGES, PARAMS, animated, cube, targets, white};
use bevy::{
    asset::RenderAssetUsages,
    camera::{RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    light::{Cascades, SimulationLightSystems},
    prelude::*,
    render::render_resource::{Extent3d, TextureFormat},
    transform::TransformSystems,
    ui_render::prelude::MaterialNode,
};
use grid_formats::{osd, osd_glyph, pssg, texture};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The render layer of the carousel (free of the OSD's 23 and the results
/// art's 24).
pub const LAYER: usize = 25;
const ROOT: &str = "livery_editor_root";
/// `ScreenLiveryDesignerRoot` settings `seperation_angle` (states.bin [D],
/// read by 0x50CE30 into +0x3B8; the port has no states.bin reader).
pub const SEPARATION: f32 = 0.0145;
/// The carousel glyphs of the same settings (`colourcarousel`,
/// `patterncarousel`, `numbercarousel`), as `component/<glyph>=<name>`.
const PILLS: &str = "component/livery_carousel_pills=";
const ITEM: &str = "gadget/liv_pill_items=";
const PILL: &str = "kitpart/liv_pill=";
const PATTERN: &str = "null=pattern";
const NUMBERS: &str = "text=numbers";
/// The pattern library node; its name prefixes the pattern textures [I].
const LIBRARY: &str = "library/";
const BORDER: &str = "component/liv_pill_boarder_anim=";
/// The node `Anim!53` folds away (t = 0, 2) and shows (t = 1) [I].
const BORDER_FOLD: &str = "kitpart/liv_pill_boarder=pill_boarder";
const SHOWN_AT: f32 = 1.;
const CAMERA: &str = "camera=carousel_cam_1";
const TARGET: &str = "target=carousel_cam_1";
/// The garage bundles: pattern pictures, and each garage's reflection cubes.
const PATTERNS: &str = "frontend/b_garage.pssg";
const GARAGE: &str = "frontend/b_garage_01.pssg";
const GARAGE_CUBES: [&str; 2] = ["ref_garage_soft.dds", "ref_garage_sharp.dds"];
const LIGHTS: (&str, &str) = ("garage_01", "zone_22");
/// Pills drawn on each side of the front one (AUTHORED: 0.0145 × 590 ≈ 8.6
/// units apart, about eight on each side fit the view).
pub const REACH: i32 = 12;
/// The turn's length (AUTHORED from the one `carousel_rotate` event with a
/// `durationOverride`, 0.2 s).
pub const TURN: f32 = 0.2;

// ------------------------------------------------------------------ view

/// A carousel kind: its glyph name in the settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Colour,
    Pattern,
    Number,
}

impl Kind {
    const ALL: [Kind; 3] = [Kind::Colour, Kind::Pattern, Kind::Number];

    fn glyph(self) -> &'static str {
        match self {
            Kind::Colour => "colour_pills",
            Kind::Pattern => "pattern_pills",
            Kind::Number => "number_pills",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Kind::Colour => "colour",
            Kind::Pattern => "pattern",
            Kind::Number => "number",
        }
    }
}

/// What one pill shows.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Content {
    Colour([u8; 3]),
    /// The row's `fe_image_name`.
    Pattern(String),
    /// The row's `number_string`.
    Number(String),
}

/// One carousel: its pills in table order and the selected one.
#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub kind: Kind,
    pub pills: Vec<Content>,
    pub index: usize,
}

impl View {
    /// The carousel of `kind` over the livery tables with the row `id`
    /// selected (the first when the table lacks it, as `step_livery`).
    pub fn of(kind: Kind, choices: &grid_tools::career::LiveryChoices, id: i32) -> Self {
        let (ids, pills): (Vec<i32>, Vec<Content>) = match kind {
            Kind::Colour => choices
                .colours
                .iter()
                .map(|c| (c.0, Content::Colour(c.1)))
                .unzip(),
            Kind::Pattern => choices
                .patterns
                .iter()
                .map(|p| (p.0, Content::Pattern(p.1.clone())))
                .unzip(),
            Kind::Number => choices
                .numbers
                .iter()
                .map(|n| (n.0, Content::Number(n.1.clone())))
                .unzip(),
        };
        View {
            kind,
            index: ids.iter().position(|&i| i == id).unwrap_or(0),
            pills,
        }
    }
}

/// Written by the career screen: the carousel to draw, `None` to hide.
#[derive(Resource, Default)]
pub struct Shown {
    pub view: Option<View>,
}

// ------------------------------------------------------------------ ring

/// The world matrix of a ring item turned by `angle` about the pivot's y
/// axis (`pivot` the ring node's world, `template` the item's world).
fn ring(pivot: Mat4, template: Mat4, angle: f32) -> Mat4 {
    pivot * Mat4::from_rotation_y(angle) * pivot.inverse() * template
}

/// The turn's progress curve: slope 1 at the start, flat at the end [I].
fn ease(p: f32) -> f32 {
    let p = p.clamp(0., 1.);
    p + p * p - p * p * p
}

/// The ring's position in pills (pill `i` is at (i − at) × separation).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Turn {
    from: f32,
    to: f32,
    t: f32,
}

impl Turn {
    fn at(&self) -> f32 {
        self.from + (self.to - self.from) * ease(self.t / TURN)
    }

    /// Turns towards `index` from where the ring is.
    fn retarget(&mut self, index: f32) {
        if index != self.to {
            *self = Turn {
                from: self.at(),
                to: index,
                t: 0.,
            };
        }
    }

    fn snap(&mut self, index: f32) {
        *self = Turn {
            from: index,
            to: index,
            t: TURN,
        };
    }

    fn advance(&mut self, dt: f32) {
        self.t = (self.t + dt).min(TURN);
    }
}

/// The pill a ring slot shows: slot `k` of the slots around the front pill.
fn slot_pill(at: f32, k: i32, count: usize) -> Option<usize> {
    let i = at.round() as i32 + k;
    (0..count as i32).contains(&i).then_some(i as usize)
}

// ------------------------------------------------------------------ layout

/// What a template item draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Role {
    /// The pill body; a colour pill tints it.
    Pill,
    /// The pattern picture (pattern pills only).
    Picture,
    /// The number text (number pills only).
    Number,
    /// The selection arrows (fixed in front of the camera).
    Arrow,
}

/// The kind and role of a placed node from its name and its ancestors'
/// (`names[0]` the node, up to the root).
fn role(names: &[&str]) -> Option<(Option<Kind>, Role)> {
    let has = |p: &str| names.iter().any(|n| n.starts_with(p));
    if has(BORDER) {
        return Some((None, Role::Arrow));
    }
    let glyph = names.iter().find_map(|n| n.strip_prefix(PILLS))?;
    let kind = Kind::ALL.into_iter().find(|k| k.glyph() == glyph)?;
    let role = if has(PILL) {
        Role::Pill
    } else if names.contains(&NUMBERS) {
        Role::Number
    } else if names.contains(&PATTERN) {
        Role::Picture
    } else {
        return None;
    };
    let wanted = match role {
        Role::Picture => kind == Kind::Pattern,
        Role::Number => kind == Kind::Number,
        _ => true,
    };
    wanted.then_some((Some(kind), role))
}

#[derive(Clone, Debug)]
struct Item {
    kind: Option<Kind>,
    role: Role,
    world: Mat4,
    material: usize,
    family: u32,
    bounds: [f32; 6],
    mesh: osd::Mesh,
    text: Option<osd::TextLayout>,
}

#[derive(Clone, Debug)]
struct MaterialDef {
    values: fe3d::Values,
    sharp: bool,
    dirt: Option<String>,
    pattern: Option<String>,
    diffuse: Option<String>,
}

#[derive(Debug)]
struct Layout {
    eye: Vec3,
    target: Vec3,
    fov_degrees: f32,
    near: f32,
    far: f32,
    items: Vec<Item>,
    materials: Vec<MaterialDef>,
    /// The ring pivot of each kind.
    pivots: HashMap<Kind, Mat4>,
    /// The pattern library's name (the pattern textures' prefix).
    library: Option<String>,
}

fn layout(scenes: &[&osd::Scene]) -> Result<Layout, String> {
    let (si, root) = scenes
        .iter()
        .enumerate()
        .find_map(|(si, s)| {
            s.nodes
                .iter()
                .position(|n| n.name() == ROOT)
                .map(|n| (si, n))
        })
        .ok_or(format!("no {ROOT}"))?;
    let local = |s: usize, n: usize| -> [f32; 16] {
        let node = &scenes[s].nodes[n];
        if node.name() != BORDER_FOLD {
            return node.local;
        }
        let channels: Vec<&osd::Channel> = scenes[s]
            .animations
            .iter()
            .flat_map(|a| &a.channels)
            .filter(|c| targets(c, node))
            .collect();
        animated(&node.local, &channels, SHOWN_AT)
    };
    let parent_world = match scenes[si].nodes[root].parent {
        Some(p) => scenes[si].world(p),
        None => IDENTITY,
    };
    let placed = osd_glyph::expand_with(scenes, si, root, &parent_world, &local);
    let name = |p: &osd_glyph::Placed| scenes[p.scene].nodes[p.node].name();
    let world = |p: &osd_glyph::Placed| Vec3::new(p.world[12], p.world[13], p.world[14]);
    let find = |n: &str| {
        placed
            .iter()
            .find(|p| name(p) == n)
            .ok_or(format!("no {n}"))
    };
    let (camera, target) = (find(CAMERA)?, find(TARGET)?);
    let cam = scenes[camera.scene]
        .cameras
        .iter()
        .find(|c| c.node == camera.node);
    let mut pivots = HashMap::new();
    let mut library = None;
    let mut materials = Vec::new();
    let mut material_index: HashMap<(usize, usize), usize> = HashMap::new();
    let mut items = Vec::new();
    for p in &placed {
        let node = &scenes[p.scene].nodes[p.node];
        let mut names = vec![node.name()];
        let mut up = p.parent;
        while let Some(q) = up {
            names.push(name(&placed[q]));
            up = placed[q].parent;
        }
        // The ring pivot: the parent of each carousel's template item.
        if let Some(glyph) = names.iter().find_map(|n| n.strip_prefix(PILLS))
            && node.name().starts_with(ITEM)
            && let Some(kind) = Kind::ALL.into_iter().find(|k| k.glyph() == glyph)
            && let Some(parent) = p.parent
        {
            pivots
                .entry(kind)
                .or_insert(Mat4::from_cols_array(&placed[parent].world));
        }
        if library.is_none()
            && let Some(l) = node.name().strip_prefix(LIBRARY)
            && names.contains(&PATTERN)
        {
            library = l.split('=').next().map(str::to_owned);
        }
        let Some(render) = &node.render else {
            continue;
        };
        let Some((kind, role)) = role(&names) else {
            continue;
        };
        let m = &scenes[p.scene].materials[render.material];
        let (Some(family), Some(values)) = (fe3d::family(&m.effect), fe3d::values(m)) else {
            continue;
        };
        let material = *material_index
            .entry((p.scene, render.material))
            .or_insert_with(|| {
                let name = |n: &str| m.texture(n).map(str::to_owned);
                materials.push(MaterialDef {
                    values,
                    sharp: m.texture("TR1").is_some_and(|t| t.contains("sharp")),
                    dirt: name("TDirt1"),
                    pattern: name("TPattern"),
                    diffuse: name("TDiffuseMap"),
                });
                materials.len() - 1
            });
        items.push(Item {
            kind,
            role,
            world: Mat4::from_cols_array(&p.world),
            material,
            family,
            bounds: node.bounds,
            mesh: render.mesh.clone(),
            text: node.text.clone(),
        });
    }
    for k in Kind::ALL {
        if !pivots.contains_key(&k)
            || !items
                .iter()
                .any(|i| i.kind == Some(k) && i.role == Role::Pill)
        {
            return Err(format!("no {} carousel", k.glyph()));
        }
    }
    Ok(Layout {
        eye: world(camera),
        target: world(target),
        fov_degrees: cam.map_or(60., |c| c.fov_degrees),
        near: cam.map_or(0.1, |c| c.near).max(0.1),
        far: cam.map_or(10000., |c| c.far),
        items,
        materials,
        pivots,
        library,
    })
}

/// The texture id of a pattern pill [I]: `liv_pattern` + `_` + `pattern_0`.
fn pattern_texture(library: &str, image: &str) -> String {
    format!("{library}_{image}")
}

// ------------------------------------------------------------------ loading

struct Prepared {
    layout: Layout,
    images: HashMap<String, Image>,
    /// Pattern pictures by `fe_image_name`.
    patterns: HashMap<String, Image>,
    cubes: [Image; 2],
    cubes_from: &'static str,
    font: Option<osd_scene::Font>,
    lights: fe3d::Lights,
}

fn read_tree<'a>(bytes: &'a [u8], path: &str) -> Result<pssg::Tree<'a>, String> {
    pssg::read_tree(bytes).map_err(|e| format!("{path}: {e}"))
}

fn upload(tree: &pssg::Tree, ids: &[&[u8]], path: &str) -> Result<HashMap<String, Image>, String> {
    let mut out = HashMap::new();
    for t in texture::read_named(tree, ids).map_err(|e| format!("{path}: {e}"))? {
        let wrap = texture::wrap(tree, t.id).unwrap_or([texture::Wrap::Repeat; 2]);
        let (dds, _) = t.validated_dds();
        let image = crate::texture_upload::image_linear(
            &dds,
            osd_scene::sampler(wrap),
            RenderAssetUsages::RENDER_WORLD,
        )
        .map_err(|e| format!("{}: {e}", String::from_utf8_lossy(t.id)))?;
        out.insert(String::from_utf8_lossy(t.id).into_owned(), image);
    }
    Ok(out)
}

/// The reflection cubes: the garage's, else the race OSD's (as results_art).
fn cubes(
    read: &dyn Fn(&str) -> Result<Vec<u8>, String>,
) -> Result<([Image; 2], &'static str), String> {
    let from = |path: &'static str, ids: [&str; 2]| -> Result<([Image; 2], &'static str), String> {
        let bytes = read(path)?;
        let tree = read_tree(&bytes, path)?;
        let scene = osd::read_scene(&tree).map_err(|e| format!("{path}: {e}"))?;
        let of = |id: &str| {
            scene
                .cubemaps
                .iter()
                .find(|c| c.id == id)
                .map(cube)
                .ok_or(format!("{path}: no {id}"))
        };
        Ok(([of(ids[0])?, of(ids[1])?], path))
    };
    from(GARAGE, GARAGE_CUBES)
        .or_else(|_| from(crate::results_art::CUBES, ["env_soft.dds", "env_sharp.dds"]))
}

fn prepare(root: &Path, pattern_names: &[String]) -> Result<Prepared, String> {
    let read = |p: &str| std::fs::read(root.join(p)).map_err(|e| format!("{p}: {e}"));
    let bytes: Vec<Vec<u8>> = PACKAGES.iter().map(|p| read(p)).collect::<Result<_, _>>()?;
    let trees: Vec<pssg::Tree> = bytes
        .iter()
        .zip(PACKAGES)
        .map(|(b, p)| read_tree(b, p))
        .collect::<Result<_, _>>()?;
    let scenes: Vec<osd::Scene> = trees
        .iter()
        .zip(PACKAGES)
        .map(|(t, p)| osd::read_scene(t).map_err(|e| format!("{p}: {e}")))
        .collect::<Result<_, _>>()?;
    let refs: Vec<&osd::Scene> = scenes.iter().collect();
    let layout = layout(&refs)?;
    let wanted: HashSet<&str> = layout
        .materials
        .iter()
        .flat_map(|m| [&m.dirt, &m.pattern, &m.diffuse])
        .filter_map(|t| t.as_deref())
        .collect();
    let bundle_bytes = read(crate::results_art::BUNDLE)?;
    let bundle = read_tree(&bundle_bytes, crate::results_art::BUNDLE)?;
    let ids: Vec<&[u8]> = wanted.iter().map(|w| w.as_bytes()).collect();
    let images = upload(&bundle, &ids, crate::results_art::BUNDLE)?;
    let mut patterns = HashMap::new();
    if let Some(library) = &layout.library {
        let garage_bytes = read(PATTERNS)?;
        let garage = read_tree(&garage_bytes, PATTERNS)?;
        let names: Vec<String> = pattern_names
            .iter()
            .map(|n| pattern_texture(library, n))
            .collect();
        let ids: Vec<&[u8]> = names.iter().map(|n| n.as_bytes()).collect();
        let mut by_id = upload(&garage, &ids, PATTERNS)?;
        for n in pattern_names {
            if let Some(image) = by_id.remove(&pattern_texture(library, n)) {
                patterns.insert(n.clone(), image);
            }
        }
    }
    let (cubes, cubes_from) = cubes(&read)?;
    let font = match layout.items.iter().find_map(|i| i.text.as_ref()) {
        Some(t) => {
            let path = ["PSSG", "pssg"]
                .iter()
                .map(|ext| PathBuf::from(format!("frontend/fonts/{}.{ext}", t.font)))
                .find(|p| root.join(p).is_file())
                .ok_or(format!("font {} missing", t.font))?;
            let bytes =
                std::fs::read(root.join(&path)).map_err(|e| format!("{}: {e}", path.display()))?;
            let tree = read_tree(&bytes, &path.display().to_string())?;
            Some(osd_scene::font(
                osd::read_font(&tree).map_err(|e| format!("{}: {e}", path.display()))?,
            ))
        }
        None => None,
    };
    let xml = String::from_utf8_lossy(&read(PARAMS)?).into_owned();
    let lights = fe3d::Lights::read(&xml, LIGHTS.0, Some(LIGHTS.1)).unwrap_or_default();
    Ok(Prepared {
        layout,
        images,
        patterns,
        cubes,
        cubes_from,
        font,
        lights,
    })
}

#[derive(Resource)]
struct Loading(Arc<Mutex<Option<Result<Prepared, String>>>>);

/// Starts loading the carousel in the background; `pattern_names` are the
/// `fe_image_name`s of the pattern table (their pictures are loaded once).
pub fn install(app: &mut App, root: &Path, pattern_names: Vec<String>) {
    app.init_resource::<Shown>();
    if crate::settings::diagnostics().livery_carousel_off {
        return;
    }
    let slot = Arc::new(Mutex::new(None));
    let fill = slot.clone();
    let root = root.to_path_buf();
    std::thread::spawn(move || {
        let result = prepare(&root, &pattern_names);
        if let Ok(mut s) = fill.lock() {
            *s = Some(result);
        }
    });
    fe3d::plugin(app);
    osd_scene::display_plugin(app);
    app.insert_resource(Loading(slot))
        .add_systems(Update, spawn)
        .add_systems(
            PostUpdate,
            (resize, update).chain().before(TransformSystems::Propagate),
        )
        .add_systems(
            PostUpdate,
            drop_sun_cascades
                .after(SimulationLightSystems::UpdateDirectionalLightCascades)
                .before(SimulationLightSystems::UpdateLightFrusta),
        );
}

// ------------------------------------------------------------------ live

#[derive(Component)]
struct CarouselCamera;

/// One drawn copy of a template item: ring slot `slot` around the front
/// pill (arrows: none).
#[derive(Component)]
struct Pill {
    item: usize,
    slot: Option<i32>,
}

#[derive(Resource)]
struct Live {
    items: Vec<Item>,
    pivots: HashMap<Kind, Mat4>,
    materials: Vec<Handle<Fe3dMaterial>>,
    /// Per-content copies of the pill and picture materials.
    tinted: HashMap<(usize, Content), Handle<Fe3dMaterial>>,
    number_meshes: HashMap<(usize, String), Handle<Mesh>>,
    patterns: HashMap<String, Handle<Image>>,
    font: Option<osd_scene::Font>,
    lights: fe3d::Lights,
    pulse: fe3d::Pulse,
    camera_world: Mat4,
    camera: Entity,
    node: Entity,
    image: Handle<Image>,
    display: Handle<OsdDisplay>,
    size: UVec2,
    turn: Turn,
    /// The carousel last shown (kind and table length).
    showing: Option<(Kind, usize)>,
    /// The selection last logged.
    logged: Option<(Kind, usize)>,
}

#[allow(clippy::too_many_arguments)]
fn spawn(
    mut commands: Commands,
    loading: Option<Res<Loading>>,
    viewer: Query<Entity, With<crate::ViewerCamera>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Fe3dMaterial>>,
    mut displays: ResMut<Assets<OsdDisplay>>,
) {
    let (Some(loading), Some(viewer)) = (loading, viewer.iter().next()) else {
        return;
    };
    let Some(result) = loading.0.lock().ok().and_then(|mut s| s.take()) else {
        return;
    };
    commands.remove_resource::<Loading>();
    let p = match result {
        Ok(p) => p,
        Err(e) => {
            log_line!("livery_carousel version=1 loaded=false error={e:?}");
            return;
        }
    };
    let l = &p.layout;
    let count = |r: Role| l.items.iter().filter(|i| i.role == r).count();
    log_line!(
        "livery_carousel version=1 loaded=true items={} pills={} pictures={} numbers={} arrows={} materials={} patterns={} cubes={} eye={:.1},{:.1},{:.1} fov={:.2}",
        l.items.len(),
        count(Role::Pill),
        count(Role::Picture),
        count(Role::Number),
        count(Role::Arrow),
        l.materials.len(),
        p.patterns.len(),
        p.cubes_from,
        l.eye.x,
        l.eye.y,
        l.eye.z,
        l.fov_degrees
    );
    let white = images.add(white());
    let [soft, sharp] = p.cubes.map(|c| images.add(c));
    let named: HashMap<String, Handle<Image>> = p
        .images
        .into_iter()
        .map(|(k, v)| (k, images.add(v)))
        .collect();
    let texture = |n: &Option<String>| {
        n.as_ref()
            .and_then(|n| named.get(n))
            .cloned()
            .unwrap_or_else(|| white.clone())
    };
    let constants = p.lights.constants(None, None, 0.);
    let handles: Vec<Handle<Fe3dMaterial>> = l
        .materials
        .iter()
        .map(|m| {
            materials.add(Fe3dMaterial {
                reflection: if m.sharp { sharp.clone() } else { soft.clone() },
                dirt: texture(&m.dirt),
                pattern: texture(&m.pattern),
                values: m.values,
                constants,
                diffuse: texture(&m.diffuse),
            })
        })
        .collect();
    let image = images.add(Image::new_target_texture(
        16,
        16,
        TextureFormat::Rgba8UnormSrgb,
        None,
    ));
    let display = displays.add(OsdDisplay::new(image.clone()));
    let transform = Transform::from_translation(l.eye).looking_at(l.target, Vec3::Y);
    let camera = commands
        .spawn((
            CarouselCamera,
            Camera3d::default(),
            Camera {
                order: -18,
                is_active: false,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(image.clone().into()),
            Projection::Perspective(PerspectiveProjection {
                fov: l.fov_degrees.to_radians(),
                near: l.near,
                far: l.far,
                aspect_ratio: 16. / 9.,
                ..default()
            }),
            transform,
            Tonemapping::None,
            if cfg!(target_os = "android") {
                Msaa::Off
            } else {
                Msaa::Sample4
            },
            RenderLayers::layer(LAYER),
        ))
        .id();
    // Under the career panel (a root node without a z index).
    let node = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.),
                height: percent(100.),
                ..default()
            },
            MaterialNode(display.clone()),
            GlobalZIndex(-1),
            Visibility::Hidden,
            Pickable::IGNORE,
            UiTargetCamera(viewer),
        ))
        .id();
    for (i, item) in l.items.iter().enumerate() {
        let mesh = if item.text.is_some() {
            Handle::default()
        } else {
            meshes.add(fe3d::mesh(&item.mesh, &item.bounds, item.family))
        };
        let slots: Vec<Option<i32>> = if item.role == Role::Arrow {
            vec![None]
        } else {
            (-REACH..=REACH).map(Some).collect()
        };
        for slot in slots {
            commands.spawn((
                Pill { item: i, slot },
                Transform::from_matrix(item.world),
                Visibility::Hidden,
                Mesh3d(mesh.clone()),
                MeshMaterial3d(handles[item.material].clone()),
                RenderLayers::layer(LAYER),
                bevy::camera::visibility::NoFrustumCulling,
            ));
        }
    }
    let patterns = p
        .patterns
        .into_iter()
        .map(|(k, v)| (k, images.add(v)))
        .collect();
    commands.insert_resource(Live {
        items: l.items.clone(),
        pivots: l.pivots.clone(),
        materials: handles,
        tinted: HashMap::new(),
        number_meshes: HashMap::new(),
        patterns,
        font: p.font,
        lights: p.lights,
        pulse: fe3d::Pulse::default(),
        camera_world: transform.to_matrix(),
        camera,
        node,
        image,
        display,
        size: UVec2::ZERO,
        turn: Turn::default(),
        showing: None,
        logged: None,
    });
}

/// The material a pill or picture shows `content` with: a copy of the
/// template's with the colour or the pattern picture.
fn content_material(
    live: &mut Live,
    item: usize,
    content: &Content,
    materials: &mut Assets<Fe3dMaterial>,
) -> Option<Handle<Fe3dMaterial>> {
    let it = &live.items[item];
    let base = live.materials[it.material].clone();
    let edit: Box<dyn Fn(&mut Fe3dMaterial)> = match (it.role, content) {
        (Role::Pill, Content::Colour(rgb)) => {
            let c = Vec3::from_array(rgb.map(|v| f32::from(v) / 255.));
            Box::new(move |m: &mut Fe3dMaterial| m.values.colour = c.extend(m.values.colour.w))
        }
        (Role::Picture, Content::Pattern(name)) => {
            let image = live.patterns.get(name)?.clone();
            Box::new(move |m: &mut Fe3dMaterial| m.diffuse = image.clone())
        }
        _ => return Some(base),
    };
    if let Some(h) = live.tinted.get(&(item, content.clone())) {
        return Some(h.clone());
    }
    let mut m = materials.get(&base)?.clone();
    edit(&mut m);
    let h = materials.add(m);
    live.tinted.insert((item, content.clone()), h.clone());
    Some(h)
}

#[allow(clippy::too_many_arguments)]
fn update(
    live: Option<ResMut<Live>>,
    shown: Res<Shown>,
    time: Option<Res<Time<Real>>>,
    mut cameras: Query<&mut Camera, With<CarouselCamera>>,
    mut nodes: Query<&mut Visibility, Without<Pill>>,
    mut pills: Query<(
        &Pill,
        &mut Transform,
        &mut Visibility,
        &mut Mesh3d,
        &mut MeshMaterial3d<Fe3dMaterial>,
    )>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<Fe3dMaterial>>,
) {
    let Some(mut live) = live else {
        return;
    };
    let live = &mut *live;
    let on = shown.view.is_some();
    if let Ok(mut camera) = cameras.get_mut(live.camera)
        && camera.is_active != on
    {
        camera.is_active = on;
    }
    if let Ok(mut v) = nodes.get_mut(live.node) {
        v.set_if_neq(if on {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    let Some(view) = &shown.view else {
        live.showing = None;
        return;
    };
    let dt = time.map_or(0., |t| t.delta_secs());
    let showing = Some((view.kind, view.pills.len()));
    if live.showing != showing {
        live.turn.snap(view.index as f32);
        live.showing = showing;
    } else {
        live.turn.retarget(view.index as f32);
        live.turn.advance(dt);
    }
    if live.logged != Some((view.kind, view.index)) {
        log_line!(
            "livery_carousel version=1 event=shown kind={} pills={} index={}",
            view.kind.name(),
            view.pills.len(),
            view.index
        );
        live.logged = Some((view.kind, view.index));
    }
    let at = live.turn.at();
    let pivot = live.pivots[&view.kind];
    for (pill, mut transform, mut visibility, mut mesh, mut material) in &mut pills {
        let it = &live.items[pill.item];
        let (kind, template, bounds, family, text_layout) =
            (it.kind, it.world, it.bounds, it.family, it.text.clone());
        let shown_pill = match pill.slot {
            None => Some(None),
            Some(_) if kind != Some(view.kind) => None,
            Some(k) => slot_pill(at, k, view.pills.len()).map(Some),
        };
        let Some(index) = shown_pill else {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        };
        if let Some(i) = index {
            let world = ring(pivot, template, (i as f32 - at) * SEPARATION);
            *transform = Transform::from_matrix(world);
            let content = &view.pills[i];
            if let Some(h) = content_material(live, pill.item, content, &mut materials)
                && material.0 != h
            {
                material.0 = h;
            }
            if let (Some(layout), Content::Number(text), Some(font)) =
                (&text_layout, content, &live.font)
            {
                let key = (pill.item, text.clone());
                let h = live
                    .number_meshes
                    .entry(key)
                    .or_insert_with(|| {
                        let m = osd_scene::text_mesh(font, text, layout, &bounds);
                        meshes.add(fe3d::mesh(&m, &bounds, family))
                    })
                    .clone();
                if mesh.0 != h {
                    mesh.0 = h;
                }
            }
        }
        visibility.set_if_neq(Visibility::Inherited);
    }
    live.pulse.advance(dt, live.lights.g_rate);
    let constants = live.lights.constants(
        Some(&live.camera_world),
        None,
        live.pulse.value(live.lights.g_swing),
    );
    for h in live.materials.iter().chain(live.tinted.values()) {
        if let Some(mut m) = materials.get_mut(h)
            && m.constants != constants
        {
            m.constants = constants;
        }
    }
}

/// Keeps the carousel image at the UI camera's size (as results_art's).
fn resize(
    live: Option<ResMut<Live>>,
    viewer: Query<&Camera, (With<crate::ViewerCamera>, Without<CarouselCamera>)>,
    mut images: ResMut<Assets<Image>>,
    mut displays: ResMut<Assets<OsdDisplay>>,
    mut projection: Query<&mut Projection, With<CarouselCamera>>,
) {
    let (Some(mut live), Some(camera)) = (live, viewer.iter().next()) else {
        return;
    };
    let Some(size) = camera.physical_target_size().filter(|s| s.x > 0 && s.y > 0) else {
        return;
    };
    if size == live.size {
        return;
    }
    live.size = size;
    if let Some(mut image) = images.get_mut(&live.image) {
        image.resize(Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        });
    }
    if let Some(mut display) = displays.get_mut(&live.display) {
        display.touch();
    }
    if let Ok(mut p) = projection.get_mut(live.camera)
        && let Projection::Perspective(p) = p.as_mut()
    {
        p.aspect_ratio = size.x as f32 / size.y as f32;
    }
}

/// No sun shadow cascades for the carousel camera (unlit materials).
fn drop_sun_cascades(art: Query<Entity, With<CarouselCamera>>, mut lights: Query<&mut Cascades>) {
    for camera in &art {
        for mut cascades in &mut lights {
            if cascades.cascades.contains_key(&camera) {
                cascades.cascades.remove(&camera);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_tools::career::LiveryChoices;

    fn choices() -> LiveryChoices {
        LiveryChoices {
            patterns: vec![(1, "pattern_0".into()), (2, "pattern_1".into())],
            pattern_files: vec![(1, "simple_01.pssg".into()), (2, "simple_02.pssg".into())],
            colours: vec![(7, [255, 0, 0]), (3, [0, 128, 255]), (9, [10, 20, 30])],
            numbers: vec![(1, "1".into()), (2, "22".into())],
        }
    }

    #[test]
    fn views_follow_the_tables_in_order() {
        let c = choices();
        let v = View::of(Kind::Colour, &c, 3);
        assert_eq!(v.index, 1);
        assert_eq!(
            v.pills,
            [
                Content::Colour([255, 0, 0]),
                Content::Colour([0, 128, 255]),
                Content::Colour([10, 20, 30])
            ]
        );
        assert_eq!(
            View::of(Kind::Pattern, &c, 2).pills[1],
            Content::Pattern("pattern_1".into())
        );
        assert_eq!(
            View::of(Kind::Number, &c, 99).index,
            0,
            "an unknown id selects the first, as step_livery"
        );
        assert_eq!(
            pattern_texture("liv_pattern", "pattern_0"),
            "liv_pattern_pattern_0"
        );
    }

    #[test]
    fn ring_turns_items_about_the_pivot() {
        // The shipped layout: pivot (-270.1, -4.4, 0), template 590 in front.
        let pivot = Mat4::from_translation(Vec3::new(-270.1, -4.4, 0.));
        let template = Mat4::from_translation(Vec3::new(319.9, -4.6, 0.));
        assert!(ring(pivot, template, 0.).abs_diff_eq(template, 1e-4));
        let next = ring(pivot, template, SEPARATION).w_axis.truncate();
        let radius = 590.;
        assert!(
            (next.z + radius * SEPARATION.sin()).abs() < 1e-2,
            "the next pill lies to screen right (-z): {next}"
        );
        assert!((next.y + 4.6).abs() < 1e-4);
        assert!(((next - Vec3::new(-270.1, -4.6, 0.)).length() - radius).abs() < 1e-2);
    }

    #[test]
    fn turns_ease_to_the_index_and_restart_from_where_they_are() {
        assert_eq!((ease(0.), ease(1.), ease(2.)), (0., 1., 1.));
        assert!((ease(0.5) - 0.625).abs() < 1e-6);
        let mut t = Turn::default();
        t.snap(3.);
        assert_eq!(t.at(), 3.);
        t.retarget(4.);
        assert_eq!(t.at(), 3.);
        t.advance(TURN / 2.);
        let mid = t.at();
        assert!((mid - 3.625).abs() < 1e-5);
        t.retarget(5.);
        assert_eq!(t.at(), mid, "a new step starts from the ring's position");
        t.advance(1.);
        assert_eq!(t.at(), 5.);
    }

    #[test]
    fn slots_show_pills_inside_the_table_only() {
        assert_eq!(slot_pill(0., -1, 5), None);
        assert_eq!(slot_pill(0., 0, 5), Some(0));
        assert_eq!(slot_pill(3.4, 1, 5), Some(4));
        assert_eq!(slot_pill(3.6, 1, 5), None);
        assert_eq!(slot_pill(2., REACH, 52), Some(14));
    }

    #[test]
    fn roles_follow_the_tree() {
        let pill = [
            "null=liv_pill!0",
            "kitpart/liv_pill=liv_pill!0",
            "gadget/liv_pill_items=liv_pill_0",
            "component/livery_carousel_pills=colour_pills",
        ];
        assert_eq!(role(&pill), Some((Some(Kind::Colour), Role::Pill)));
        let picture = [
            "unique=default!9",
            "library/liv_pattern=pattern",
            PATTERN,
            "library_simple=liv_pills",
            "component/livery_carousel_pills=pattern_pills",
        ];
        assert_eq!(role(&picture), Some((Some(Kind::Pattern), Role::Picture)));
        let mut colour_picture = picture;
        colour_picture[4] = "component/livery_carousel_pills=colour_pills";
        assert_eq!(role(&colour_picture), None, "colour pills hide the library");
        let number = [
            NUMBERS,
            "null=numbers",
            "library_simple=liv_pills",
            "component/livery_carousel_pills=number_pills",
        ];
        assert_eq!(role(&number), Some((Some(Kind::Number), Role::Number)));
        assert_eq!(
            role(&[
                "null=arrow",
                "component/tab_arrow=arrow_right",
                BORDER_FOLD,
                "component/liv_pill_boarder_anim=pill_b"
            ]),
            Some((None, Role::Arrow))
        );
        assert_eq!(
            role(&[
                "null=liv_pill!0",
                "kitpart/liv_pill=liv_pill",
                "object/liv_titles=liv_titles"
            ]),
            None
        );
    }

    #[test]
    #[ignore = "needs GRID_GAME_ROOT"]
    fn the_shipped_carousel_loads() {
        let root = PathBuf::from(std::env::var("GRID_GAME_ROOT").expect("GRID_GAME_ROOT"));
        let cat = grid_tools::career::Catalogue::load(&root).expect("catalogue");
        let names: Vec<String> = cat
            .livery_choices
            .patterns
            .iter()
            .map(|p| p.1.clone())
            .collect();
        let p = prepare(&root, &names).expect("prepare");
        let l = &p.layout;
        for k in Kind::ALL {
            assert_eq!(
                l.items
                    .iter()
                    .filter(|i| i.kind == Some(k) && i.role == Role::Pill)
                    .count(),
                1,
                "{k:?}"
            );
            let pivot = l.pivots[&k].w_axis.truncate();
            assert!(
                (pivot - Vec3::new(-270.1, -4.4, 0.)).length() < 0.2,
                "{k:?} pivot {pivot}"
            );
        }
        assert_eq!(
            l.items.iter().filter(|i| i.role == Role::Picture).count(),
            1
        );
        assert_eq!(l.items.iter().filter(|i| i.role == Role::Number).count(), 1);
        assert_eq!(l.items.iter().filter(|i| i.role == Role::Arrow).count(), 2);
        assert_eq!(l.library.as_deref(), Some("liv_pattern"));
        assert_eq!(
            p.patterns.len(),
            names.len(),
            "every pattern row has its picture"
        );
        assert_eq!(p.cubes_from, GARAGE, "the garage cubes read");
        assert!(
            (l.eye - Vec3::new(378.1, 10., 0.)).length() < 0.2,
            "{}",
            l.eye
        );
        assert!((l.fov_degrees - 68.04).abs() < 0.05);
        assert!(p.font.is_some());
        // The arrows sit beside the front pill in their shown pose.
        let pill = l
            .items
            .iter()
            .find(|i| i.role == Role::Pill)
            .unwrap()
            .world
            .w_axis
            .truncate();
        for a in l.items.iter().filter(|i| i.role == Role::Arrow) {
            let w = a.world.w_axis.truncate();
            assert!(
                (w.y - pill.y).abs() < 4. && (w.z.abs() - 5.3).abs() < 0.2,
                "arrow {w} pill {pill}"
            );
        }
    }
}
