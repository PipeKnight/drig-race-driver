//! The original's static water environment maps (`WaterEnvMap`,
//! docs/WATER.md "Environment map"): one cube per child of land.pssg's `water`
//! node, centred on that child's world origin (sea level: y 0, Detroit
//! −8.9), 128 texels a face, cleared to grey 0.2 and drawn once while the
//! race is set up with the water hidden, far plane 2000 m (0x5FBA30,
//! 0x64C9B0). The water draws of a child sample its cube.
//!
//! The port draws the six faces on the default layer while the pipeline
//! warm-up's cover is still up after the scene is restored
//! ([`crate::pipeline_warmup::Warmup::restoring`]), until the cover would
//! lift, then removes the cameras. Meanwhile every sun drops its shadow maps
//! (Bevy would otherwise build each face its own four cascades) and the sky
//! dome sits on the first probe's centre. Without the warm-up
//! (`GRID_PIPELINE_WARMUP=0`) the faces draw for [`FALLBACK_FRAMES`] frames
//! from the start, visibly.
//!
//! AUTHORED: Rgba16Float (the original's format 0xC is not identified),
//! near 0.1 m, sunlit without shadows; cars on the grid and the main camera's
//! choice of track levels are drawn; several probes share one sky position.
//! `GRID_WATER_PROBE_DUMP=1` writes each cube as a PNG strip to the data
//! directory's `graphics/`.
use bevy::{
    camera::{Exposure, Hdr, RenderTarget},
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

/// Texels a face (0x9853C0's render target).
pub const SIZE: u32 = 128;
/// The env pass's far distance (0x5FBA30: 2000.0).
pub const FAR: f32 = 2000.;
/// AUTHORED near plane.
const NEAR: f32 = 0.1;
/// The cube's clear colour (0x9853C0: 0.2, 0.2, 0.2, 1).
const CLEAR: f32 = 0.2;
/// Frames drawn without the warm-up's cover [authored].
pub const FALLBACK_FRAMES: u32 = 120;
/// Copies needed before a capture may end [authored].
const MIN_COPIES: u64 = 2;

/// A water draw; hidden while the probes draw (the original hides the water
/// root).
#[derive(Component)]
pub struct WaterSurface;

#[derive(Component)]
struct Face;

/// The cubes and their faces' render targets.
#[derive(Resource, Clone, ExtractResource)]
pub struct Probes {
    origins: Vec<Vec3>,
    cubes: Vec<Handle<Image>>,
    faces: Vec<Vec<Handle<Image>>>,
    /// The faces are drawn and copied this frame.
    active: bool,
    copies: Arc<AtomicU64>,
}

/// The sky dome's centre while the probes draw (`sky::follow_camera`).
#[derive(Resource, Default)]
pub struct SkyCentre(pub Option<Vec3>);

#[derive(Default, PartialEq, Eq, Debug, Clone, Copy)]
enum Stage {
    #[default]
    Waiting,
    Capturing,
    Done,
}

#[derive(Resource, Default)]
struct Capture {
    stage: Stage,
    frames: u32,
    suns: Vec<Entity>,
    hidden: Vec<(Entity, Visibility)>,
    dump: bool,
}

/// One cube per origin, filled with the clear colour until drawn; returns
/// their handles in origin order.
pub fn install(app: &mut App, origins: Vec<Vec3>) -> Vec<Handle<Image>> {
    if origins.is_empty() {
        return Vec::new();
    }
    let mut images = app.world_mut().resource_mut::<Assets<Image>>();
    let grey = half_bits(CLEAR).to_le_bytes();
    let one = half_bits(1.).to_le_bytes();
    let pixel = [grey[0], grey[1], grey[0], grey[1], grey[0], grey[1], one[0], one[1]];
    let mut cubes = Vec::new();
    let mut faces = Vec::new();
    for _ in &origins {
        let mut cube = Image::new_fill(
            Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 6 },
            TextureDimension::D2,
            &pixel,
            TextureFormat::Rgba16Float,
            bevy::asset::RenderAssetUsages::RENDER_WORLD,
        );
        cube.texture_descriptor.usage =
            TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::COPY_SRC;
        cube.texture_view_descriptor = Some(TextureViewDescriptor {
            dimension: Some(TextureViewDimension::Cube),
            ..default()
        });
        cubes.push(images.add(cube));
        faces.push(
            (0..6)
                .map(|_| {
                    let mut face = Image::new_target_texture(SIZE, SIZE, TextureFormat::Rgba16Float, None);
                    face.texture_descriptor.usage |= TextureUsages::COPY_SRC;
                    images.add(face)
                })
                .collect(),
        );
    }
    println!(
        "water_probe probes={} size={SIZE} far={FAR} origins={:?}",
        origins.len(),
        origins.iter().map(|o| o.to_array()).collect::<Vec<_>>()
    );
    app.insert_resource(Probes {
        origins,
        cubes: cubes.clone(),
        faces,
        active: false,
        copies: Arc::default(),
    })
    .insert_resource(Capture {
        dump: std::env::var("GRID_WATER_PROBE_DUMP").is_ok_and(|v| v == "1"),
        ..default()
    })
    .init_resource::<SkyCentre>()
    .add_plugins(ExtractResourcePlugin::<Probes>::default())
    .add_systems(Update, capture);
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(Render, copy_faces.in_set(RenderSystems::Cleanup));
    }
    cubes
}

/// A positive normal f32 as half-float bits (truncated mantissa).
fn half_bits(value: f32) -> u16 {
    let bits = value.to_bits();
    let exponent = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    ((exponent.clamp(1, 30) as u16) << 10) | ((bits >> 13) & 0x3ff) as u16
}

/// Start the faces once the scene is restored under the warm-up's cover and
/// end them before it lifts.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn capture(
    mut commands: Commands,
    mut state: ResMut<Capture>,
    mut probes: ResMut<Probes>,
    mut sky: ResMut<SkyCentre>,
    warmup: Option<Res<crate::pipeline_warmup::Warmup>>,
    viewer: Query<
        (Option<&Exposure>, Option<&EnvironmentMapLight>, Option<&DistanceFog>),
        With<crate::ViewerCamera>,
    >,
    faces: Query<Entity, With<Face>>,
    mut suns: Query<(Entity, &mut DirectionalLight)>,
    mut water: Query<(Entity, &mut Visibility), With<WaterSurface>>,
) {
    match state.stage {
        Stage::Done => {}
        Stage::Waiting => {
            if warmup.as_ref().is_some_and(|w| !w.restoring()) {
                return;
            }
            let Ok((exposure, environment, fog)) = viewer.single() else {
                return;
            };
            for (probe, (origin, targets)) in probes.origins.iter().zip(&probes.faces).enumerate() {
                for (face, target) in targets.iter().enumerate() {
                    let mut camera = commands.spawn((
                        Face,
                        Name::new(format!("water probe {probe} face {face}")),
                        Camera3d::default(),
                        Camera {
                            order: -40 + (probe * 6 + face) as isize,
                            clear_color: ClearColorConfig::Custom(Color::linear_rgb(CLEAR, CLEAR, CLEAR)),
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
                        crate::car_reflections::face_transform(face, *origin),
                        Hdr,
                        exposure.cloned().unwrap_or_default(),
                        Tonemapping::None,
                        Msaa::Off,
                        bevy::light::cluster::ClusterConfig::Single,
                    ));
                    if let Some(e) = environment {
                        camera.insert(e.clone());
                    }
                    if let Some(f) = fog {
                        camera.insert(f.clone());
                    }
                }
            }
            for (entity, mut light) in &mut suns {
                if light.shadow_maps_enabled {
                    light.shadow_maps_enabled = false;
                    state.suns.push(entity);
                }
            }
            for (entity, mut visibility) in &mut water {
                state.hidden.push((entity, *visibility));
                *visibility = Visibility::Hidden;
            }
            sky.0 = probes.origins.first().copied();
            probes.active = true;
            state.stage = Stage::Capturing;
            println!(
                "water_probe capture=start cameras={} suns_unshadowed={} water_hidden={} warmup={}",
                probes.origins.len() * 6,
                state.suns.len(),
                state.hidden.len(),
                warmup.is_some()
            );
        }
        Stage::Capturing => {
            state.frames += 1;
            let copies = probes.copies.load(Ordering::Acquire);
            let end = match &warmup {
                Some(w) => w.ready() || (w.finishing() && copies >= MIN_COPIES),
                None => state.frames >= FALLBACK_FRAMES,
            };
            if !end {
                return;
            }
            for face in &faces {
                commands.entity(face).despawn();
            }
            let restored = std::mem::take(&mut state.suns);
            for entity in &restored {
                if let Ok((_, mut light)) = suns.get_mut(*entity) {
                    light.shadow_maps_enabled = true;
                }
            }
            for (entity, visibility) in std::mem::take(&mut state.hidden) {
                if let Ok((_, mut shown)) = water.get_mut(entity) {
                    *shown = visibility;
                }
            }
            sky.0 = None;
            probes.active = false;
            state.stage = Stage::Done;
            println!(
                "water_probe capture=done frames={} copies={copies} suns_restored={}",
                state.frames,
                restored.len()
            );
            if state.dump {
                for (i, cube) in probes.cubes.iter().enumerate() {
                    commands.spawn(Readback::texture(cube.clone())).observe(
                        move |event: On<ReadbackComplete>, mut commands: Commands| {
                            commands.entity(event.entity).despawn();
                            let path = crate::settings::data_dir()
                                .join("graphics")
                                .join(format!("water-probe-{i}.png"));
                            match crate::car_reflections::save_cube_strip(&event.data, SIZE as usize, &path) {
                                Ok(()) => println!("water_probe dump={}", path.display()),
                                Err(error) => eprintln!("water_probe dump=failed {error}"),
                            }
                        },
                    );
                }
            }
        }
    }
}

/// The frame's faces into their cubes, after every camera has drawn.
fn copy_faces(
    probes: Option<Res<Probes>>,
    images: Res<RenderAssets<GpuImage>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
) {
    let Some(probes) = probes.filter(|p| p.active) else {
        return;
    };
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("water probe faces"),
    });
    let mut copied = false;
    for (cube, faces) in probes.cubes.iter().zip(&probes.faces) {
        let Some(target) = images.get(cube) else {
            continue;
        };
        for (face, source) in faces.iter().enumerate() {
            let Some(source) = images.get(source) else {
                continue;
            };
            let mut destination = target.texture.as_image_copy();
            destination.origin.z = face as u32;
            encoder.copy_texture_to_texture(
                source.texture.as_image_copy(),
                destination,
                Extent3d { width: SIZE, height: SIZE, depth_or_array_layers: 1 },
            );
            copied = true;
        }
    }
    if copied {
        queue.submit([encoder.finish()]);
        probes.copies.fetch_add(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clear_fill_is_grey_in_half_floats() {
        assert_eq!(half_bits(CLEAR), 0x3266);
        assert_eq!(half_bits(1.), 0x3C00);
    }
}
