//! The cars' dynamic environment cube, after the original's `dynamic_env_map`
//! (docs/CAR_REFLECTIONS.md): one cube a frame, every face redrawn, centred
//! 0.5 m above the focused car's origin (0x5FBC10), far plane 1000 m (the far
//! map, 0x602400). Six cameras draw [`LAYER`], which holds every mesh of the
//! default layer except the focused car's and a copy of the sun without
//! shadow maps: Bevy gives a view the shadow cascades of every sun on its
//! layers, so the probe takes none of the main sun's. After the frame the
//! faces are copied into one cube texture.
//!
//! Levels as a setting ([`Level`]): the original's 256 per face, 512 and 1024.
//! Common build; off unless `--car-reflections` or `GRID_CAR_REFLECTIONS`
//! names a level (until the owner has compared them).
//!
//! AUTHORED: the downward face is drawn with its objects (the original's face
//! 3 has no visibility list); the focused car is the player's; the probe
//! draws the main camera's choice of track and car levels; point and spot
//! lights stay out of the probe.
use bevy::{
    camera::{Exposure, Hdr, RenderTarget, primitives::Aabb, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    light::EnvironmentMapLight,
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        gpu_readback::{Readback, ReadbackComplete},
        render_asset::RenderAssets,
        render_resource::*,
        renderer::{RenderDevice, RenderQueue},
        texture::GpuImage,
    },
};
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

/// The reflection cameras' layer.
pub const LAYER: usize = 27;
/// Height of the cube's centre above the focused car's origin (0x5FBC10).
const CENTRE_HEIGHT: f32 = 0.5;
/// Far plane of the far map (0x602400: 0.1, 1000, cull 600).
const NEAR: f32 = 0.1;
const FAR: f32 = 1000.;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Level {
    #[default]
    Off,
    /// 256 per face, the original's medium/high.
    Original,
    /// 512, the original's ultra.
    High,
    /// 1024.
    Ultra,
}

impl Level {
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "off" => Self::Off,
            "original" | "256" => Self::Original,
            "high" | "512" => Self::High,
            "ultra" | "1024" => Self::Ultra,
            _ => return None,
        })
    }

    /// `GRID_CAR_REFLECTIONS`, when set and valid.
    pub fn from_env() -> Option<Self> {
        Self::parse(&std::env::var("GRID_CAR_REFLECTIONS").ok()?)
    }

    pub fn size(self) -> u32 {
        match self {
            Self::Off => 0,
            Self::Original => 256,
            Self::High => 512,
            Self::Ultra => 1024,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Original => "original",
            Self::High => "high",
            Self::Ultra => "ultra",
        }
    }
}

/// One face camera; the face index in Bevy's cube order (+X, −X, +Y, −Y,
/// +Z, −Z).
#[derive(Component, Clone, Copy)]
pub struct FaceCamera(pub usize);

/// The sun's copy that lights the probe, without shadow maps.
#[derive(Component)]
struct ProbeSun;

/// The cube and its faces' render targets.
#[derive(Resource, Clone, ExtractResource)]
pub struct Cube {
    pub image: Handle<Image>,
    pub size: u32,
    faces: Vec<Handle<Image>>,
    /// Copies submitted; the cube holds the scene from the second one on.
    pub copies: Arc<AtomicU64>,
}

/// Which meshes the probe draws: the original's reflection object selection
/// (docs/TRACK_LOD.md 3.3) with the cube's centre as the eye, re-chosen when
/// the centre has moved [`RESELECT`] metres.
#[derive(Resource)]
struct Selection {
    /// Smallest bounding radius over distance drawn (`refmap_obj_size`).
    min_size: f32,
    /// Farthest object drawn (`refmap_dist`, at most the far plane).
    far: f32,
    centre: Option<Vec3>,
    chosen_at: Option<Vec3>,
}
const RESELECT: f32 = 2.;
/// `route_overrides.xml` defaults when the route sets none (0x64C070), and
/// the clamp of `refmap_dist` (0x62E680).
const REFMAP_OBJ_SIZE: f32 = 0.2;
const REFMAP_DIST: f32 = 2500.;
const REFMAP_DIST_MAX: f32 = 500.;
/// `main_obj_size`, the main view's smallest object (0x64C070): ultra draws
/// what the main view would.
const MAIN_OBJ_SIZE: f32 = 0.004;

/// The route's `refmap_obj_size` and `refmap_dist` (`route_overrides.xml`
/// `default`), else the engine defaults.
fn route_refmap(root: &std::path::Path, route: &grid_tools::route::Route) -> (f32, f32) {
    let mut size = REFMAP_OBJ_SIZE;
    let mut dist = REFMAP_DIST;
    if let Ok(bytes) = std::fs::read(root.join(route.file("route_overrides.xml")))
        && let Ok(document) = grid_formats::binxml::read(&bytes)
        && let Some(values) = document.elements.iter().find(|e| e.name == "default")
    {
        let field = |key: &str| {
            values
                .attributes
                .iter()
                .find(|(k, _)| *k == key)
                .and_then(|(_, v)| v.trim().parse::<f32>().ok())
                .filter(|v| *v > 0.)
        };
        size = field("refmap_obj_size").unwrap_or(size);
        dist = field("refmap_dist").unwrap_or(dist);
    }
    (size, dist.min(REFMAP_DIST_MAX))
}

#[derive(Resource)]
struct Dump {
    after: u64,
    done: bool,
}

pub fn install(
    app: &mut App,
    level: Level,
    root: &std::path::Path,
    route: &grid_tools::route::Route,
) {
    if level == Level::Off {
        return;
    }
    let size = level.size();
    let (refmap_size, refmap_dist) = route_refmap(root, route);
    let (min_size, far) = match level {
        Level::Original => (refmap_size, refmap_dist),
        Level::High => (refmap_size / 4., refmap_dist),
        _ => (MAIN_OBJ_SIZE, FAR),
    };
    let mut images = app.world_mut().resource_mut::<Assets<Image>>();
    let mut cube = Image::new_fill(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        &[0; 8],
        TextureFormat::Rgba16Float,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    cube.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::COPY_SRC;
    cube.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    let image = images.add(cube);
    let faces = (0..6)
        .map(|_| {
            let mut face = Image::new_target_texture(size, size, TextureFormat::Rgba16Float, None);
            face.texture_descriptor.usage |= TextureUsages::COPY_SRC;
            images.add(face)
        })
        .collect();
    app.insert_resource(Cube {
        image,
        size,
        faces,
        copies: Arc::default(),
    })
    .insert_resource(Selection {
        min_size,
        far,
        centre: None,
        chosen_at: None,
    })
    .add_plugins(ExtractResourcePlugin::<Cube>::default())
    .add_systems(Update, spawn)
    .add_systems(
        PostUpdate,
        (
            assign_layers
                .after(bevy::transform::TransformSystems::Propagate)
                .after(bevy::camera::visibility::VisibilitySystems::CalculateBounds)
                .before(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
            drop_cascades
                .after(bevy::light::SimulationLightSystems::UpdateDirectionalLightCascades)
                .before(bevy::light::SimulationLightSystems::UpdateLightFrusta),
            follow
                .after(crate::replay::present)
                .before(bevy::transform::TransformSystems::Propagate),
        ),
    );
    if let Some(after) = std::env::var("GRID_CAR_REFLECTIONS_DUMP")
        .ok()
        .and_then(|v| v.parse().ok())
    {
        app.insert_resource(Dump { after, done: false })
            .add_systems(Last, dump);
    }
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(Render, copy_faces.in_set(RenderSystems::Cleanup));
    }
    println!(
        "car_reflections level={} size={size} faces=6 far={far} min_object_size={min_size} layer={LAYER} centre=focused-car+{CENTRE_HEIGHT}",
        level.name()
    );
}

/// Bevy's cube face directions (the environment map's sampling convention),
/// as scene_probe::direction.
fn direction(face: usize, u: f32, v: f32) -> Vec3 {
    let (u, v) = (2. * u - 1., 2. * v - 1.);
    match face {
        0 => Vec3::new(1., -v, u),
        1 => Vec3::new(-1., -v, -u),
        2 => Vec3::new(u, 1., -v),
        3 => Vec3::new(u, -1., v),
        4 => Vec3::new(u, -v, -1.),
        5 => Vec3::new(-u, -v, 1.),
        _ => unreachable!(),
    }
    .normalize()
}

fn face_transform(face: usize, centre: Vec3) -> Transform {
    let forward = direction(face, 0.5, 0.5);
    let right = (direction(face, 0.75, 0.5) - direction(face, 0.25, 0.5)).normalize();
    let up = (direction(face, 0.5, 0.25) - direction(face, 0.5, 0.75)).normalize();
    Transform {
        translation: centre,
        rotation: Quat::from_mat3(&Mat3::from_cols(right, up, -forward)),
        ..default()
    }
}

/// The face cameras and the probe's sun, once the viewer camera exists.
fn spawn(
    mut commands: Commands,
    cube: Res<Cube>,
    cameras: Query<(), With<FaceCamera>>,
    viewer: Query<Option<&Exposure>, With<crate::ViewerCamera>>,
    mut done: Local<bool>,
) {
    if *done || !cameras.is_empty() {
        return;
    }
    let Ok(exposure) = viewer.single() else {
        return;
    };
    for (face, target) in cube.faces.iter().enumerate() {
        commands.spawn((
            FaceCamera(face),
            Name::new(format!("car reflection face {face}")),
            Camera3d::default(),
            Camera {
                order: -20 + face as isize,
                clear_color: ClearColorConfig::Custom(Color::BLACK),
                ..default()
            },
            RenderTarget::Image(target.clone().into()),
            Projection::Perspective(PerspectiveProjection {
                fov: std::f32::consts::FRAC_PI_2,
                aspect_ratio: 1.,
                near: NEAR,
                far: FAR,
                ..default()
            }),
            face_transform(face, Vec3::ZERO),
            Hdr,
            exposure.cloned().unwrap_or_default(),
            Tonemapping::None,
            Msaa::Off,
            RenderLayers::layer(LAYER),
            // No point or spot lights reach the probe: one cluster (None fails GPU clustering).
            bevy::light::cluster::ClusterConfig::Single,
        ));
    }
    commands.spawn((
        ProbeSun,
        Name::new("car reflection sun"),
        DirectionalLight {
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::default(),
        RenderLayers::layer(LAYER),
    ));
    *done = true;
}

/// Meshes on the default layer are drawn by the probe too when they are big
/// enough seen from the cube's centre ([`Selection`]), except the focused
/// car's; a mesh that leaves the default layer leaves the probe. Every mesh
/// is checked when the centre has moved; otherwise the new and re-layered
/// ones.
#[allow(clippy::type_complexity)]
fn assign_layers(
    mut commands: Commands,
    mut selection: ResMut<Selection>,
    meshes: Query<
        (
            Entity,
            Ref<Mesh3d>,
            Option<Ref<RenderLayers>>,
            &GlobalTransform,
            Option<&Aabb>,
            Has<crate::MovingPart>,
            Option<&crate::car_batch::Batch>,
        ),
        (
            Without<FaceCamera>,
            Without<crate::car_reflection_overlay::OverlayOf>,
        ),
    >,
) {
    let Some(centre) = selection.centre else {
        return;
    };
    let all = selection
        .chosen_at
        .is_none_or(|at| at.distance(centre) >= RESELECT);
    if all {
        selection.chosen_at = Some(centre);
    }
    for (entity, mesh, layers, transform, aabb, own, batch) in &meshes {
        if !all && !mesh.is_added() && !layers.as_ref().is_some_and(Ref::is_changed) {
            continue;
        }
        let own = own || batch.is_some_and(|b| b.car == 0);
        let current = layers.as_deref().cloned().unwrap_or_default();
        // Bounds not computed yet: drawn until the next choice.
        let big = aabb.is_none_or(|aabb| {
            let world = transform.affine();
            let radius = world.transform_vector3(aabb.half_extents.into()).length();
            let distance = world.transform_point3(aabb.center.into()).distance(centre);
            distance - radius <= selection.far
                && radius >= selection.min_size * distance.max(f32::EPSILON)
        });
        let wanted = if current.intersects(&RenderLayers::layer(0)) && !own && big {
            current.clone().with(LAYER)
        } else {
            current.clone().without(LAYER)
        };
        if layers.is_none() || wanted != current {
            commands.entity(entity).insert(wanted);
        }
    }
}

/// Bevy builds every sun's cascades for every active camera and then finds
/// each cascade's casters on the CPU, whether or not the view takes that sun.
/// The faces take none: drop theirs before the casters are found (4 cascades
/// of 6 faces doubled the render schedule's CPU time).
fn drop_cascades(
    faces: Query<Entity, With<FaceCamera>>,
    mut lights: Query<&mut bevy::light::Cascades>,
) {
    for mut cascades in &mut lights {
        for face in &faces {
            cascades.cascades.remove(&face);
        }
    }
}

/// Centre the faces on the focused car and keep the probe's sun, exposure,
/// sky lighting and fog those of the scene.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn follow(
    mut commands: Commands,
    driving: Option<Res<crate::Driving>>,
    replay: Option<Res<crate::replay::Replay>>,
    mut faces: Query<(Entity, &FaceCamera, &mut Transform, &mut Exposure)>,
    viewer: Query<
        (
            Option<Ref<Exposure>>,
            Option<Ref<EnvironmentMapLight>>,
            Option<Ref<DistanceFog>>,
        ),
        (With<crate::ViewerCamera>, Without<FaceCamera>),
    >,
    suns: Query<(&DirectionalLight, &Transform), (Without<ProbeSun>, Without<FaceCamera>)>,
    mut probe_sun: Query<(&mut DirectionalLight, &mut Transform), (With<ProbeSun>, Without<FaceCamera>)>,
    mut selection: ResMut<Selection>,
) {
    // A replay presents snapshot poses while the live runtime is paused.
    let pose = replay
        .as_deref()
        .and_then(|r| r.session.as_ref())
        .and_then(|s| s.shown.as_ref())
        .and_then(|s| s.cars.first())
        .map(|car| car.pose.to_mat4())
        .or_else(|| {
            driving
                .as_deref()
                .map(|d| Mat4::from_cols_array(&d.runtime.pose()))
        });
    let centre = pose.map(|p| p.transform_point3(Vec3::Y * CENTRE_HEIGHT));
    if centre.is_some() {
        selection.centre = centre;
    }
    let view = viewer.single().ok();
    for (entity, face, mut transform, mut exposure) in &mut faces {
        if let Some(centre) = centre {
            *transform = face_transform(face.0, centre);
        }
        let Some((viewer_exposure, environment, fog)) = &view else {
            continue;
        };
        if let Some(e) = viewer_exposure
            && e.is_changed()
        {
            *exposure = **e;
        }
        if let Some(e) = environment
            && e.is_changed()
        {
            commands.entity(entity).insert((**e).clone());
        }
        if let Some(f) = fog
            && f.is_changed()
        {
            commands.entity(entity).insert((**f).clone());
        }
    }
    // The brightest sun of the default layer.
    let sun = suns
        .iter()
        .filter(|(light, _)| light.illuminance > 0.)
        .max_by(|a, b| a.0.illuminance.total_cmp(&b.0.illuminance));
    if let (Some((light, transform)), Ok((mut copy, mut copy_transform))) =
        (sun, probe_sun.single_mut())
    {
        *copy = DirectionalLight {
            shadow_maps_enabled: false,
            contact_shadows_enabled: false,
            ..*light
        };
        *copy_transform = *transform;
    }
}

/// The frame's faces into the cube, after every camera has drawn.
fn copy_faces(
    cube: Option<Res<Cube>>,
    images: Res<RenderAssets<GpuImage>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let Some(cube) = cube else {
        return;
    };
    let Some(target) = images.get(&cube.image) else {
        return;
    };
    let Some(faces) = cube
        .faces
        .iter()
        .map(|h| images.get(h))
        .collect::<Option<Vec<_>>>()
    else {
        return;
    };
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("car reflection faces"),
    });
    for (face, source) in faces.iter().enumerate() {
        let mut destination = target.texture.as_image_copy();
        destination.origin.z = face as u32;
        encoder.copy_texture_to_texture(
            source.texture.as_image_copy(),
            destination,
            Extent3d {
                width: cube.size,
                height: cube.size,
                depth_or_array_layers: 1,
            },
        );
    }
    queue.submit([encoder.finish()]);
    cube.copies.fetch_add(1, Ordering::AcqRel);
}

/// `GRID_CAR_REFLECTIONS_DUMP=N`: after N copies, the cube's six faces side
/// by side (+X, −X, +Y, −Y, +Z, −Z) as a PNG in the data directory's
/// `graphics/`, tone mapped x/(1+x) for viewing.
fn dump(mut commands: Commands, cube: Res<Cube>, mut state: ResMut<Dump>) {
    if state.done || cube.copies.load(Ordering::Acquire) < state.after {
        return;
    }
    state.done = true;
    commands
        .spawn(Readback::texture(cube.image.clone()))
        .observe(write_dump);
}

fn write_dump(event: On<ReadbackComplete>, mut commands: Commands, cube: Res<Cube>) {
    commands.entity(event.entity).despawn();
    let size = cube.size as usize;
    if event.data.len() != size * size * 6 * 8 {
        eprintln!("car_reflections dump=failed bytes={}", event.data.len());
        return;
    }
    let mut rgba = vec![255u8; size * 6 * size * 4];
    for (i, pixel) in event.data.as_chunks::<8>().0.iter().enumerate() {
        let (face, rest) = (i / (size * size), i % (size * size));
        let (y, x) = (rest / size, rest % size);
        let at = (y * size * 6 + face * size + x) * 4;
        for c in 0..3 {
            let v = crate::scene_probe::half(u16::from_le_bytes([pixel[c * 2], pixel[c * 2 + 1]]));
            let v = (v / (1. + v)).clamp(0., 1.);
            let srgb = if v <= 0.003_130_8 {
                12.92 * v
            } else {
                1.055 * v.powf(1. / 2.4) - 0.055
            };
            rgba[at + c] = (srgb * 255. + 0.5) as u8;
        }
    }
    let image = Image::new(
        Extent3d {
            width: (size * 6) as u32,
            height: size as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        bevy::asset::RenderAssetUsages::MAIN_WORLD,
    );
    let directory = crate::settings::data_dir().join("graphics");
    let path = directory.join(format!(
        "car-reflections-{}.png",
        cube.copies.load(Ordering::Acquire)
    ));
    let result = std::fs::create_dir_all(&directory)
        .map_err(|e| e.to_string())
        .and_then(|_| image.try_into_dynamic().map_err(|e| format!("{e:?}")))
        .and_then(|image| image.to_rgb8().save(&path).map_err(|e| e.to_string()));
    match result {
        Ok(()) => println!("car_reflections dump={}", path.display()),
        Err(error) => eprintln!("car_reflections dump=failed {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_follow_the_original_sizes() {
        assert_eq!(Level::parse("original").map(Level::size), Some(256));
        assert_eq!(Level::parse("high").map(Level::size), Some(512));
        assert_eq!(Level::parse("ultra").map(Level::size), Some(1024));
        assert_eq!(Level::parse("off"), Some(Level::Off));
        assert_eq!(Level::parse("2048"), None);
    }

    #[test]
    fn faces_look_along_the_cube_directions() {
        for face in 0..6 {
            let t = face_transform(face, Vec3::new(1., 2., 3.));
            // A camera looks down its −Z.
            let forward = t.rotation * Vec3::NEG_Z;
            assert!(forward.distance(direction(face, 0.5, 0.5)) < 1e-5, "face {face}");
            assert_eq!(t.translation, Vec3::new(1., 2., 3.));
        }
    }
}
