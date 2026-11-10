//! A front-end 3D world (docs/FRONT_END_FLOW.md §2): the drawn meshes below
//! one node of a front-end PSSG package, seen through one of the package's
//! target cameras as its animation channels move it. The loading screen
//! uses it for `loading_root` and `load_cam`; any other front-end world
//! (another root, camera or lighting group) goes through the same [`Spec`].
//!
//! - Surfaces: the `fe3d_tracksurface` pixel program [D, shaderpack
//!   disassembly, `local/front-end-flow/shaders/`]: colour = texture ×
//!   instance colour × (ambient + Emmisive × (1 − ambient)), plus a
//!   box-projected cube reflection (`r_min`/`r_max`, `r_gamma`, `r_scale`)
//!   times a Fresnel term of at most `FresnelMax` and the texture's alpha;
//!   alpha = the instance colour's. Its vertex program passes no lighting, so
//!   the group's directional lights do not reach these surfaces [D]. Ported
//!   whole in `fe_world.wgsl` ([`Surface`]), with the material's
//!   `DiffuseColour` as the instance colour [I]; the box, gamma and scale
//!   come from the parameter group and its zone ([`Spec::zone`]), the cube
//!   from the bundle ([`Spec::reflection`]).
//! - Camera: `camera=NAME` looks at `target=NAME` (the saved matrix's view
//!   axis points at the target, roll free [D]); both follow their
//!   `Translation` channels from the world's start (held before the first
//!   key and after the last); vertical field of view `FOV` [I, as the OSD
//!   camera]. On top, the camera noise of the world's `camera_tweaks`
//!   block orbits it slowly about its target ([`crate::camera_noise`]); the
//!   stick spring (player-turned view) is open.
//! - Instances: a node `kind/glyph=instance` draws the glyph `<glyph>_root`
//!   below its transform (`grid_formats::osd_glyph`, F787), except a
//!   `screen/` instance: that is a front-end screen's panel, which its screen
//!   shows while it is active (`ScreenLoadingStats` looks up `loading_stats`
//!   by name [D]), not part of the world [I]. `loading_root` places 16 of
//!   them (2434 meshes, mostly the `fe3d_generic` family); none is drawn here.
use bevy::{
    asset::{RenderAssetUsages, uuid_handle},
    core_pipeline::tonemapping::Tonemapping,
    mesh::MeshVertexBufferLayoutRef,
    pbr::{MaterialPipeline, MaterialPipelineKey},
    prelude::*,
    render::render_resource::{AsBindGroup, RenderPipelineDescriptor, ShaderType, SpecializedMeshPipelineError},
    shader::ShaderRef,
};
use grid_formats::osd;
use grid_formats::osd_glyph::{self, Placed};
use std::collections::HashMap;
use std::path::Path;

/// One front-end world: where it is and how it is lit.
pub struct Spec<'a> {
    /// The PSSG package (the `FE::DatabaseHandler` "database"), relative
    /// to the install root.
    pub package: &'a str,
    /// The package loaded beside it as its "bundle"
    /// (`system/render_persistent_data.xml`): the database's textures are
    /// 4 × 4 stubs, the bundle holds the full ones under the same ids [D].
    /// A texture the bundle lacks keeps the database's copy.
    pub bundle: &'a str,
    /// The node whose descendants are drawn.
    pub root: &'a str,
    /// `NAME` of `camera=NAME` and `target=NAME`.
    pub camera: &'a str,
    /// The `fe_paramcontrol.xml` parameter group that lights it.
    pub lights: &'a str,
    /// The group's zone the world is seen in, whose values replace the
    /// group's own.
    pub zone: Option<&'a str>,
    /// The bundle's cubemap its surfaces reflect.
    pub reflection: &'a str,
    /// The `frontend/persistentdatarender.xml` `<camera_tweaks>` block
    /// whose `noise_tweaks` move its camera.
    pub tweaks: &'a str,
}

/// The loading screen's world [D fePersistentDatabase.pssg, b_persistent.pssg,
/// fe_paramcontrol.xml]. Its stats screen sits in `l_zone_loading_stats`,
/// zone 9 [D flow.xml]. The cube is the renderer's default one,
/// `reflection_load.dds` (loaded by 0x4E2750, chosen by 0x4DCFA0 outside the
/// garage and track modes) [D]; that the tracksurface's `TR1` is that
/// default is [I] (its materials name no cube).
pub const LOADING: Spec<'static> = Spec {
    package: "frontend/fePersistentDatabase.pssg",
    bundle: "frontend/b_persistent.pssg",
    root: "loading_root",
    camera: "load_cam",
    lights: "loading",
    zone: Some("zone_09"),
    reflection: "reflection_load.dds",
    tweaks: "loading",
};

const SHADER: Handle<Shader> = uuid_handle!("3d1c6f0a-8b52-4e7d-9a41-5f2e0c7b9d18");

/// A `fe3d_tracksurface` surface (`fe_world.wgsl`).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct Surface {
    #[texture(0)]
    #[sampler(1)]
    diffuse: Handle<Image>,
    #[texture(2, dimension = "cube")]
    #[sampler(3)]
    reflection: Handle<Image>,
    #[uniform(4)]
    params: SurfaceParams,
}

/// The pixel program's constants: `_refMinGamma`, `_refMaxScale`, the
/// texture factor (with the instance alpha) and FresnelMin/Max/Power.
#[derive(Clone, Copy, Debug, Default, PartialEq, ShaderType)]
pub struct SurfaceParams {
    min_gamma: Vec4,
    max_scale: Vec4,
    factor: Vec4,
    fresnel: Vec4,
}

impl Material for Surface {
    fn fragment_shader() -> ShaderRef {
        SHADER.into()
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
        Ok(())
    }
}

/// A position over time: three curves in the parent's space.
struct Path3 {
    curves: Option<[osd::Curve; 3]>,
    rest: Vec3,
    parent: Mat4,
}

impl Path3 {
    fn at(&self, t: f32) -> Vec3 {
        let local = match &self.curves {
            Some([x, y, z]) => Vec3::new(x.sample(t), y.sample(t), z.sample(t)),
            None => self.rest,
        };
        self.parent.transform_point3(local)
    }
}

/// The world's camera and its paths.
#[derive(Component)]
pub struct WorldCamera {
    eye: Path3,
    target: Path3,
    noise: Option<crate::camera_noise::CameraNoise>,
}

impl WorldCamera {
    /// The camera's transform `t` seconds into the world.
    pub fn transform(&self, t: f32) -> Transform {
        Transform::from_translation(self.eye.at(t)).looking_at(self.target.at(t), Vec3::Y)
    }

    /// The transform `t` seconds in, `dt` after the last call, with the
    /// noise orbit on the path's place.
    pub fn advance(&mut self, t: f32, dt: f32) -> Transform {
        let (eye, target) = (self.eye.at(t), self.target.at(t));
        let eye = match &mut self.noise {
            Some(noise) => {
                let (pitch, yaw) = noise.angles(dt);
                crate::camera_noise::orbit(eye, target, Vec3::Y, pitch, yaw)
            }
            None => eye,
        };
        Transform::from_translation(eye).looking_at(target, Vec3::Y)
    }
}

/// The matrix of a saved row-vector array.
fn mat(m: &[f32; 16]) -> Mat4 {
    Mat4::from_cols_array(m)
}

/// The `Translation` channel of node `id`, if an animation moves it.
fn translation(scene: &osd::Scene<'_>, id: &str) -> Option<[osd::Curve; 3]> {
    scene.animations.iter().flat_map(|a| &a.channels).find_map(|c| {
        (c.target == id && c.semantic == osd::Semantic::Translation && c.components.len() == 3)
            .then(|| [c.components[0].clone(), c.components[1].clone(), c.components[2].clone()])
    })
}

fn path(scene: &osd::Scene<'_>, node: usize) -> Path3 {
    let n = &scene.nodes[node];
    Path3 {
        curves: translation(scene, &n.id),
        rest: Vec3::new(n.local[12], n.local[13], n.local[14]),
        parent: n.parent.map_or(Mat4::IDENTITY, |p| mat(&scene.world(p))),
    }
}

/// The `<Param name="NAME" … />` element of a parameter group: the zone's
/// own when `zone` names one that sets it, else the group's own (before its
/// first `Zone`). A zone replaces the group's value once the screen has
/// entered it [I; the blend between zones is not ported].
fn param<'x>(xml: &'x str, group: &str, zone: Option<&str>, name: &str) -> Option<&'x str> {
    let start = xml.find(&format!("<ParameterGroup name=\"{group}\""))?;
    let body = &xml[start..];
    let body = &body[..body.find("</ParameterGroup>")?];
    let find = |scope: &'x str| -> Option<&'x str> {
        let at = scope.find(&format!("name=\"{name}\""))?;
        Some(&scope[at..at + scope[at..].find("/>")?])
    };
    let zoned = zone.and_then(|zone| {
        let z = &body[body.find(&format!("<Zone id=\"{zone}\""))?..];
        find(&z[..z.find("</Zone>")?])
    });
    zoned.or_else(|| find(&body[..body.find("<Zone").unwrap_or(body.len())]))
}

/// The number in `key="…"` of a `Param` element.
fn attr(line: &str, key: &str) -> Option<f32> {
    let v = &line[line.find(&format!("{key}=\""))? + key.len() + 2..];
    v[..v.find('"')?].trim().parse().ok()
}

/// An RGB parameter (`valueR`/`valueG`/`valueB`, 0–255) as 0–1.
pub fn group_colour(xml: &str, group: &str, zone: Option<&str>, name: &str) -> Option<[f32; 3]> {
    let line = param(xml, group, zone, name)?;
    Some([attr(line, "valueR")? / 255., attr(line, "valueG")? / 255., attr(line, "valueB")? / 255.])
}

/// A scalar parameter (`value`).
pub fn group_value(xml: &str, group: &str, zone: Option<&str>, name: &str) -> Option<f32> {
    attr(param(xml, group, zone, name)?, "value")
}

/// A vector parameter (`valueX`/`valueY`/`valueZ`), read as x, y, z: the
/// reader 0x4C9A70 stores them in that order, whatever the file's comment
/// about swapped axes says [D].
pub fn group_vector(xml: &str, group: &str, zone: Option<&str>, name: &str) -> Option<Vec3> {
    let line = param(xml, group, zone, name)?;
    Some(Vec3::new(attr(line, "valueX")?, attr(line, "valueY")?, attr(line, "valueZ")?))
}

/// The unlit factor of a `fe3d_tracksurface` surface per channel: instance
/// colour × (ambient + emissive × (1 − ambient)).
pub fn surface_factor(colour: [f32; 3], emissive: f32, ambient: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| colour[i] * (ambient[i] + emissive * (1. - ambient[i])))
}

/// Whether an instance node belongs to a front-end screen (`screen/…=…`).
fn is_screen(id: &str) -> bool {
    osd_glyph::instance_glyph(id).is_some() && id.starts_with("screen/")
}

/// The nodes of the world below `root` (the root excluded), each instance's
/// glyph grafted in except a screen's; returns them and the number of
/// screen instances left out.
pub fn drawn(scene: &osd::Scene<'_>, root: usize) -> (Vec<Placed>, usize) {
    let mut screens = 0;
    // A graft follows its instance directly (the instance is a leaf), so the
    // last depth-0 node owns every deeper one.
    let mut skip = false;
    let nodes = osd_glyph::expand(&[scene], 0, root)
        .into_iter()
        .skip(1)
        .filter(|p| {
            if p.depth == 0 {
                skip = is_screen(&scene.nodes[p.node].id);
                screens += usize::from(skip);
                true
            } else {
                !skip
            }
        })
        .collect();
    (nodes, screens)
}

/// Spawns the world and its camera; returns the camera (for the UI over it).
pub fn install(app: &mut App, install_root: &Path, spec: &Spec) -> Result<Entity, String> {
    let read = |p: &str| std::fs::read(install_root.join(p)).map_err(|e| format!("{p}: {e}"));
    let bytes = read(spec.package)?;
    let tree = grid_formats::pssg::read_tree(&bytes).map_err(|e| format!("{}: {e}", spec.package))?;
    let scene = osd::read_scene(&tree).map_err(|e| format!("{}: {e}", spec.package))?;
    let bundle_bytes = read(spec.bundle)?;
    let bundle = grid_formats::pssg::read_tree(&bundle_bytes).map_err(|e| format!("{}: {e}", spec.bundle))?;
    let params = String::from_utf8_lossy(&read("frontend/fe_paramcontrol.xml")?).into_owned();
    let lights = |name: &str| format!("no {} {name}", spec.lights);
    let ambient = group_colour(&params, spec.lights, spec.zone, "ambient").ok_or_else(|| lights("ambient"))?;
    let value = |name: &str| group_value(&params, spec.lights, spec.zone, name).ok_or_else(|| lights(name));
    let vector = |name: &str| group_vector(&params, spec.lights, spec.zone, name).ok_or_else(|| lights(name));
    let min_gamma = vector("r_min")?.extend(value("r_gamma")?);
    let max_scale = vector("r_max")?.extend(value("r_scale")?);
    let cubes = grid_formats::osd::read_cubemaps(&bundle).map_err(|e| format!("{}: {e}", spec.bundle))?;
    let cube = cubes
        .iter()
        .find(|c| c.id == spec.reflection)
        .ok_or(format!("{}: no {}", spec.bundle, spec.reflection))?;
    let root = scene.node(spec.root).ok_or(format!("no {}", spec.root))?;
    let (nodes, screens) = drawn(&scene, root);
    let camera_id = format!("camera={}", spec.camera);
    let camera = scene
        .cameras
        .iter()
        .find(|c| scene.nodes[c.node].id == camera_id)
        .ok_or(format!("no {camera_id}"))?;
    let target = scene.node(&format!("target={}", spec.camera)).ok_or(format!("no target={}", spec.camera))?;
    let render = String::from_utf8_lossy(&read("frontend/persistentdatarender.xml")?).into_owned();
    let tweaks = crate::camera_noise::Tweaks::read(&render, spec.tweaks).ok_or(format!("no {} camera tweaks", spec.tweaks))?;
    let world_camera = WorldCamera {
        eye: path(&scene, camera.node),
        target: path(&scene, target),
        noise: Some(crate::camera_noise::CameraNoise::new(tweaks)),
    };
    let start = world_camera.transform(0.);
    let projection = Projection::Perspective(PerspectiveProjection {
        fov: camera.fov_degrees.to_radians(),
        near: camera.near.max(0.1),
        far: camera.far,
        ..default()
    });

    if !app.is_plugin_added::<MaterialPlugin<Surface>>() {
        app.add_plugins(MaterialPlugin::<Surface>::default());
    }
    let world = app.world_mut();
    world
        .resource_mut::<Assets<Shader>>()
        .insert(SHADER.id(), Shader::from_wgsl(include_str!("fe_world.wgsl"), "grid_fe_world.wgsl"))
        .map_err(|e| e.to_string())?;
    let reflection = world.resource_mut::<Assets<Image>>().add(crate::osd_scene::cube(cube));
    let mut textures = HashMap::new();
    let mut materials = HashMap::new();
    let mut parts = Vec::new();
    let mut grafted = 0;
    for placed in &nodes {
        let Some(render) = scene.nodes[placed.node].render.as_ref() else {
            continue;
        };
        grafted += usize::from(placed.depth > 0);
        let m = &scene.materials[render.material];
        let material = match materials.get(&render.material) {
            Some(h) => Handle::clone(h),
            None => {
                let name = m.texture("TDiffuseMap").or_else(|| m.texture("DiffuseMap"));
                let texture = match name {
                    Some(name) if !textures.contains_key(name) => {
                        let full = grid_formats::texture::read_named(&bundle, &[name.as_bytes()])
                            .map_err(|e| format!("{}: {name}: {e}", spec.bundle))?;
                        let stub = scene.textures.iter().zip(&scene.wraps).find(|(t, _)| t.id == name.as_bytes());
                        // The bundle's copy, with the database node's addressing.
                        let found = match (full.first(), stub) {
                            (Some(t), stub) => {
                                let wrap = match stub {
                                    Some((_, &wrap)) => wrap,
                                    None => grid_formats::texture::wrap(&bundle, t.id)
                                        .unwrap_or([grid_formats::texture::Wrap::Repeat; 2]),
                                };
                                Some((t.validated_dds().0, wrap))
                            }
                            (None, stub) => stub.map(|(t, &wrap)| (t.validated_dds().0, wrap)),
                        };
                        let image = match found {
                            Some((dds, wrap)) => {
                                let image = crate::texture_upload::image(
                                    &dds,
                                    crate::osd_scene::sampler(wrap),
                                    RenderAssetUsages::RENDER_WORLD,
                                )
                                .map_err(|e| format!("{name}: {e}"))?;
                                Some(world.resource_mut::<Assets<Image>>().add(image))
                            }
                            None => None,
                        };
                        textures.insert(name.to_owned(), image.clone());
                        image
                    }
                    Some(name) => textures[name].clone(),
                    None => None,
                };
                let colour = m.floats("DiffuseColour").filter(|c| c.len() >= 3).map_or([1.; 3], |c| [c[0], c[1], c[2]]);
                let emissive = m.floats("Emmisive").and_then(|v| v.first().copied()).unwrap_or(0.);
                let fresnel = |name: &str, default: f32| m.floats(name).and_then(|v| v.first().copied()).unwrap_or(default);
                let handle = world.resource_mut::<Assets<Surface>>().add(Surface {
                    // No texture: white (the default image).
                    diffuse: texture.unwrap_or_default(),
                    reflection: reflection.clone(),
                    params: SurfaceParams {
                        min_gamma,
                        max_scale,
                        // The instance colour is the material's DiffuseColour, opaque [I].
                        factor: Vec3::from(surface_factor(colour, emissive, ambient)).extend(1.),
                        fresnel: Vec4::new(fresnel("FresnelMin", 0.), fresnel("FresnelMax", 0.), fresnel("FresnelPower", 1.), 0.),
                    },
                });
                materials.insert(render.material, handle.clone());
                handle
            }
        };
        let mesh = world.resource_mut::<Assets<Mesh>>().add(crate::osd_scene::mesh(&render.mesh));
        parts.push((Mesh3d(mesh), MeshMaterial3d(material), Transform::from_matrix(mat(&placed.world))));
    }
    println!(
        "fe_world root={} parts={} grafted={grafted} screens_left={screens} materials={} textures={} reflection={} r_gamma={} r_scale={} camera={} fov={:.1}",
        spec.root,
        parts.len(),
        materials.len(),
        textures.len(),
        spec.reflection,
        min_gamma.w,
        max_scale.w,
        spec.camera,
        camera.fov_degrees
    );
    world.spawn_batch(parts);
    let entity = world
        .spawn((Camera3d::default(), projection, start, Tonemapping::None, world_camera))
        .id();
    app.add_systems(Update, follow);
    Ok(entity)
}

/// Moves the camera along its paths by the app's clock (each loading screen
/// is a new app, so the world starts at zero).
fn follow(time: Res<Time>, mut cameras: Query<(&mut WorldCamera, &mut Transform)>) {
    for (mut camera, mut transform) in &mut cameras {
        *transform = camera.advance(time.elapsed_secs(), time.delta_secs());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zone_replaces_only_the_values_it_sets() {
        let xml = r#"<ParameterGroup name="loading" >
            <Param name="ambient" valueR="5" valueG="20" valueB="36" />
            <Param name="r_gamma" value="1.5" />
            <!-- NOTE: valueY - max Z -->
            <Param name="r_min" valueX="-5.139" valueY="0.132" valueZ="-7.832" />
            <Zone id="zone_01"><Param name="ambient" valueR="15" valueG="36" valueB="58" /></Zone>
            <Zone id="zone_09"><Param name="r_gamma" value="1.0" /></Zone>
            </ParameterGroup>
            <ParameterGroup name="ingame" ><Param name="r_scale" value="3" /></ParameterGroup>"#;
        let c = group_colour(xml, "loading", None, "ambient").unwrap();
        assert!((c[0] - 5. / 255.).abs() < 1e-6 && (c[2] - 36. / 255.).abs() < 1e-6);
        assert_eq!(group_colour(xml, "loading", Some("zone_09"), "ambient"), Some(c));
        assert!((group_colour(xml, "loading", Some("zone_01"), "ambient").unwrap()[0] - 15. / 255.).abs() < 1e-6);
        assert_eq!(group_value(xml, "loading", None, "r_gamma"), Some(1.5));
        assert_eq!(group_value(xml, "loading", Some("zone_09"), "r_gamma"), Some(1.));
        assert_eq!(group_value(xml, "loading", Some("zone_01"), "r_gamma"), Some(1.5));
        assert_eq!(group_vector(xml, "loading", Some("zone_09"), "r_min"), Some(Vec3::new(-5.139, 0.132, -7.832)));
        assert!(group_value(xml, "loading", None, "r_scale").is_none(), "another group's value");
        assert!(group_colour(xml, "other", None, "ambient").is_none());
    }

    #[test]
    fn the_surface_factor_follows_the_pixel_program() {
        // Emmisive 0.5 with no ambient halves the texture; full ambient keeps it.
        assert_eq!(surface_factor([1.; 3], 0.5, [0.; 3]), [0.5; 3]);
        assert_eq!(surface_factor([1.; 3], 0.5, [1.; 3]), [1.; 3]);
        let f = surface_factor([1.; 3], 0.75, [0.2; 3]);
        assert!((f[0] - 0.8).abs() < 1e-6);
    }

    fn node(id: &str, parent: Option<usize>, x: f32) -> osd::Node {
        let mut local = Mat4::IDENTITY.to_cols_array();
        local[12] = x;
        osd::Node { id: id.into(), parent, local, bounds: [0.; 6], render: None, text: None }
    }

    #[test]
    fn worlds_graft_instances_but_leave_screens_to_their_screens() {
        let scene = osd::Scene {
            textures: vec![],
            wraps: vec![],
            cubemaps: vec![],
            materials: vec![],
            nodes: vec![
                node("world_root", None, 0.),
                node("null=floor", Some(0), 1.),
                node("object/cone=a", Some(0), 10.),
                node("screen/panel=p", Some(0), 20.),
                node("null=sky", Some(0), 30.),
                node("cone_root", None, 500.),
                node("mesh=cone", Some(5), 2.),
                node("panel_root", None, 0.),
                node("text=1", Some(7), 3.),
            ],
            camera: None,
            cameras: vec![],
            animations: vec![],
        };
        let (nodes, screens) = drawn(&scene, 0);
        let got: Vec<(&str, f32, usize)> =
            nodes.iter().map(|p| (scene.nodes[p.node].id.as_str(), p.world[12], p.depth)).collect();
        assert_eq!(
            got,
            [("null=floor", 1., 0), ("object/cone=a", 10., 0), ("mesh=cone", 12., 1), ("screen/panel=p", 20., 0), ("null=sky", 30., 0)]
        );
        assert_eq!(screens, 1);
    }

    /// The loading world from the install: its six drawn meshes and the
    /// camera's saved start looking at its target.
    #[test]
    #[ignore = "needs the GRID install (GRID_GAME_ROOT)"]
    fn the_loading_world_reads_from_the_install() {
        let root = std::env::var("GRID_GAME_ROOT").expect("GRID_GAME_ROOT");
        let bytes = std::fs::read(Path::new(&root).join(LOADING.package)).unwrap();
        let tree = grid_formats::pssg::read_tree(&bytes).unwrap();
        let scene = osd::read_scene(&tree).unwrap();
        let cam = scene.cameras.iter().find(|c| scene.nodes[c.node].id == "camera=load_cam").unwrap();
        assert!((cam.fov_degrees - 55.4).abs() < 0.1, "{}", cam.fov_degrees);
        let target = scene.node("target=load_cam").unwrap();
        let mut camera = WorldCamera {
            eye: path(&scene, cam.node),
            target: path(&scene, target),
            noise: None,
        };
        // Before its first key (1 s) the camera holds its saved place; its
        // forward axis matches the saved matrix's -Z.
        let t = camera.transform(0.);
        // Six meshes of its own; the 16 screen panels it places stay with their screens.
        let (nodes, screens) = drawn(&scene, scene.node(LOADING.root).unwrap());
        let meshes = nodes.iter().filter(|p| scene.nodes[p.node].render.is_some()).count();
        assert_eq!((meshes, screens), (6, 16));
        // Every drawn texture is a stub in the database and full in the bundle.
        let bundle_bytes = std::fs::read(Path::new(&root).join(LOADING.bundle)).unwrap();
        let bundle = grid_formats::pssg::read_tree(&bundle_bytes).unwrap();
        for name in ["extra_text.tga", "sky.tga", "tarmac_d.tga", "tarmac_logo.tga"] {
            let stub = scene.textures.iter().find(|t| t.id == name.as_bytes()).unwrap();
            let full = grid_formats::texture::read_named(&bundle, &[name.as_bytes()]).unwrap();
            assert_eq!((stub.width, full[0].width, full[0].height), (4, 512, 512), "{name}");
        }
        let saved = mat(&cam.world);
        assert!(t.translation.distance(saved.w_axis.truncate()) < 0.01);
        let forward = -saved.z_axis.truncate().normalize();
        assert!(t.forward().dot(forward) > 0.9999, "{:?} {forward:?}", t.forward());
        // It drifts to its last key by 18.333 s.
        assert!((camera.transform(30.).translation.x - -148.6768).abs() < 0.01);
        // The loading tweaks orbit it by at most about 0.1 rad about its
        // target, keeping the distance.
        let render = std::fs::read_to_string(Path::new(&root).join("frontend/persistentdatarender.xml")).unwrap();
        let tweaks = crate::camera_noise::Tweaks::read(&render, LOADING.tweaks).unwrap();
        assert_eq!((tweaks.rotation[0], tweaks.rate), (0.1, 0.066));
        camera.noise = Some(crate::camera_noise::CameraNoise::new(tweaks));
        let (eye, aim) = (camera.eye.at(30.), camera.target.at(30.));
        for _ in 0..600 {
            let moved = camera.advance(30., 0.05);
            assert!((moved.translation.distance(aim) - eye.distance(aim)).abs() < 0.01);
            assert!((moved.translation - aim).angle_between(eye - aim) < 0.2);
        }
        // The reflection: the bundle's one cube, and zone 9's gamma and
        // scale over the group's box.
        let cubes = grid_formats::osd::read_cubemaps(&bundle).unwrap();
        let ids: Vec<_> = cubes.iter().map(|c| (c.id.as_str(), c.size)).collect();
        assert_eq!(ids, [(LOADING.reflection, 128)]);
        let params = std::fs::read_to_string(Path::new(&root).join("frontend/fe_paramcontrol.xml")).unwrap();
        let get = |name| group_value(&params, LOADING.lights, LOADING.zone, name);
        assert_eq!((get("r_gamma"), get("r_scale")), (Some(1.), Some(1.)));
        assert_eq!(group_value(&params, LOADING.lights, None, "r_gamma"), Some(1.5));
        assert_eq!(group_vector(&params, LOADING.lights, LOADING.zone, "r_max"), Some(Vec3::new(3.737, 4.235, 9.674)));
        let ambient = group_colour(&params, LOADING.lights, LOADING.zone, "ambient").unwrap();
        assert!((ambient[2] - 36. / 255.).abs() < 1e-6, "zone 9 keeps the group's ambient");
        // Every drawn material has the program's Fresnel constants.
        for p in &nodes {
            if let Some(r) = &scene.nodes[p.node].render {
                let m = &scene.materials[r.material];
                let max = m.floats("FresnelMax").and_then(|v| v.first().copied());
                assert_eq!(max, Some(0.2), "{}", scene.nodes[p.node].id);
            }
        }
    }
}
