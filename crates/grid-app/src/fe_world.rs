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
//!   the group's directional lights do not reach these surfaces [D]. Ported:
//!   the first term, unlit, with the material's `DiffuseColour` as the
//!   instance colour [I]; the reflection is open (at most 0.2 × 0.8 of the
//!   cube at grazing angles).
//! - Camera: `camera=NAME` looks at `target=NAME` (the saved matrix's view
//!   axis points at the target, roll free [D]); both follow their
//!   `Translation` channels from the world's start (held before the first
//!   key and after the last); vertical field of view `FOV` [I, as the OSD
//!   camera]. The original's noise rotation and stick spring are open.
//! - Instances: a node `kind/glyph=instance` draws the glyph `<glyph>_root`
//!   below its transform (`grid_formats::osd_glyph`, F787), except a
//!   `screen/` instance: that is a front-end screen's panel, which its screen
//!   shows while it is active (`ScreenLoadingStats` looks up `loading_stats`
//!   by name [D]), not part of the world [I]. `loading_root` places 16 of
//!   them (2434 meshes, mostly the `fe3d_generic` family); none is drawn here.
use bevy::{
    asset::RenderAssetUsages,
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
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
}

/// The loading screen's world [D fePersistentDatabase.pssg, b_persistent.pssg,
/// fe_paramcontrol.xml].
pub const LOADING: Spec<'static> = Spec {
    package: "frontend/fePersistentDatabase.pssg",
    bundle: "frontend/b_persistent.pssg",
    root: "loading_root",
    camera: "load_cam",
    lights: "loading",
};

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
}

impl WorldCamera {
    /// The camera's transform `t` seconds into the world.
    pub fn transform(&self, t: f32) -> Transform {
        Transform::from_translation(self.eye.at(t)).looking_at(self.target.at(t), Vec3::Y)
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

/// An RGB parameter (`valueR`/`valueG`/`valueB`, 0–255) of a parameter
/// group's own values (before its first `Zone`), as 0–1.
pub fn group_colour(xml: &str, group: &str, param: &str) -> Option<[f32; 3]> {
    let start = xml.find(&format!("<ParameterGroup name=\"{group}\""))?;
    let body = &xml[start..];
    let end = [body.find("<Zone"), body.find("</ParameterGroup>")].into_iter().flatten().min()?;
    let body = &body[..end];
    let at = body.find(&format!("name=\"{param}\""))?;
    let line = &body[at..at + body[at..].find("/>")?];
    let value = |key: &str| -> Option<f32> {
        let v = &line[line.find(&format!("{key}=\""))? + key.len() + 2..];
        v[..v.find('"')?].trim().parse::<f32>().ok().map(|v| v / 255.)
    };
    Some([value("valueR")?, value("valueG")?, value("valueB")?])
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
    let ambient = group_colour(&params, spec.lights, "ambient").ok_or(format!("no {} ambient", spec.lights))?;
    let root = scene.node(spec.root).ok_or(format!("no {}", spec.root))?;
    let (nodes, screens) = drawn(&scene, root);
    let camera_id = format!("camera={}", spec.camera);
    let camera = scene
        .cameras
        .iter()
        .find(|c| scene.nodes[c.node].id == camera_id)
        .ok_or(format!("no {camera_id}"))?;
    let target = scene.node(&format!("target={}", spec.camera)).ok_or(format!("no target={}", spec.camera))?;
    let world_camera = WorldCamera {
        eye: path(&scene, camera.node),
        target: path(&scene, target),
    };
    let start = world_camera.transform(0.);
    let projection = Projection::Perspective(PerspectiveProjection {
        fov: camera.fov_degrees.to_radians(),
        near: camera.near.max(0.1),
        far: camera.far,
        ..default()
    });

    let world = app.world_mut();
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
                let [r, g, b] = surface_factor(colour, emissive, ambient);
                // The original works on the texture's stored (sRGB) values;
                // an sRGB factor on linear texels keeps that product [I].
                let handle = world.resource_mut::<Assets<StandardMaterial>>().add(StandardMaterial {
                    base_color: Color::srgb(r, g, b),
                    base_color_texture: texture,
                    unlit: true,
                    cull_mode: None,
                    ..default()
                });
                materials.insert(render.material, handle.clone());
                handle
            }
        };
        let mesh = world.resource_mut::<Assets<Mesh>>().add(crate::osd_scene::mesh(&render.mesh));
        parts.push((Mesh3d(mesh), MeshMaterial3d(material), Transform::from_matrix(mat(&placed.world))));
    }
    println!(
        "fe_world root={} parts={} grafted={grafted} screens_left={screens} materials={} textures={} camera={} fov={:.1}",
        spec.root,
        parts.len(),
        materials.len(),
        textures.len(),
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
fn follow(time: Res<Time>, mut cameras: Query<(&WorldCamera, &mut Transform)>) {
    for (camera, mut transform) in &mut cameras {
        *transform = camera.transform(time.elapsed_secs());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_group_colour_skips_zones() {
        let xml = r#"<ParameterGroup name="loading" >
            <Param name="ambient" valueR="5" valueG="20" valueB="36" />
            <Zone id="zone_01"><Param name="ambient" valueR="15" valueG="36" valueB="58" /></Zone>
            </ParameterGroup>"#;
        let c = group_colour(xml, "loading", "ambient").unwrap();
        assert!((c[0] - 5. / 255.).abs() < 1e-6 && (c[2] - 36. / 255.).abs() < 1e-6);
        assert!(group_colour(xml, "other", "ambient").is_none());
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
        let camera = WorldCamera {
            eye: path(&scene, cam.node),
            target: path(&scene, target),
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
    }
}
