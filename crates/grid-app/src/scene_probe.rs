//! First static HDR scene-reflection experiment. No driving/physics changes.
use bevy::{
    asset::RenderAssetUsages,
    camera::{Exposure, Hdr, RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    light::{EnvironmentMapLight, GeneratedEnvironmentMapLight, LightProbe, ParallaxCorrection},
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        gpu_readback::{Readback, ReadbackComplete},
        render_asset::RenderAssets,
        render_resource::*,
        renderer::{RenderDevice, RenderQueue},
        settings::WgpuFeatures,
        texture::GpuImage,
    },
};
use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

const EV: f32 = 13.;
#[derive(Component)]
struct ProbeCamera;
#[derive(Resource, Clone, ExtractResource)]
pub struct Cube {
    size: u32,
    faces: Vec<Handle<Image>>,
    cube: Handle<Image>,
    copied: Arc<AtomicBool>,
    read: Arc<AtomicBool>,
    bound: Arc<AtomicBool>,
}
impl Cube {
    pub fn ready(&self) -> bool {
        self.read.load(Ordering::Acquire) && self.bound.load(Ordering::Acquire)
    }
}
#[derive(Resource)]
struct Position(Vec3);

#[derive(Resource)]
struct CalibrationPosition(Vec3);

/// Explicit diagnostic only: a polished sphere makes off-screen scene content
/// recognizable without changing the Lacetti's authored paint parameters.
pub fn install_calibration(app: &mut App, root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let center = super::grid_slot(root, "slot_00")?.transform_point3(Vec3::new(0., 1.3, 0.))
        + Vec3::new(-3., 0., 0.);
    app.insert_resource(CalibrationPosition(center))
        .add_systems(PostStartup, calibration);
    Ok(())
}

fn calibration(
    mut commands: Commands,
    position: Res<CalibrationPosition>,
    main: Query<Entity, With<super::ViewerCamera>>,
    sun: Query<Entity, With<DirectionalLight>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for entity in &main {
        commands
            .entity(entity)
            .insert(RenderLayers::layer(0).with(1));
    }
    for entity in &sun {
        commands
            .entity(entity)
            .insert(RenderLayers::layer(0).with(1));
    }
    commands.spawn((
        Name::new("Reflection calibration sphere (diagnostic)"),
        Mesh3d(meshes.add(Sphere::new(0.65).mesh().uv(128, 64))),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color: Color::srgb(0.9, 0.9, 0.9),
            metallic: 1.,
            perceptual_roughness: 0.02,
            ..default()
        })),
        Transform::from_translation(position.0),
        RenderLayers::layer(1),
        bevy::light::NotShadowCaster,
    ));
    println!(
        "reflection_calibration sphere=true center={:?} metallic=1 roughness=0.02 excluded_from_probe=true casts_shadow=false physics=false",
        position.0
    );
}

pub fn install(app: &mut App, root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let slot = super::grid_slot(root, "slot_00")?;
    let center = slot.transform_point3(Vec3::new(0., 1.3, 0.));
    app.insert_resource(Position(center))
        .add_plugins(ExtractResourcePlugin::<Cube>::default())
        .add_systems(Update, (setup, activate).chain());
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(
            Render,
            (copy_faces, report_binding)
                .chain()
                .in_set(RenderSystems::Cleanup),
        );
    }
    Ok(())
}

// Match sky_environment::direction / Bevy's environment-sampling Z flip.
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

fn face_transform(face: usize, center: Vec3) -> Transform {
    let forward = direction(face, 0.5, 0.5);
    let right = (direction(face, 0.75, 0.5) - direction(face, 0.25, 0.5)).normalize();
    let up = (direction(face, 0.5, 0.25) - direction(face, 0.5, 0.75)).normalize();
    Transform {
        translation: center,
        rotation: Quat::from_mat3(&Mat3::from_cols(right, up, -forward)),
        ..default()
    }
}

#[allow(clippy::too_many_arguments)]
fn setup(
    mut commands: Commands,
    existing: Option<Res<Cube>>,
    position: Res<Position>,
    camera: Query<(Entity, &EnvironmentMapLight), With<super::ViewerCamera>>,
    parts: Query<(Entity, &super::track_surface::SourcePart)>,
    sun: Query<Entity, With<DirectionalLight>>,
    mut images: ResMut<Assets<Image>>,
    device: Res<RenderDevice>,
    settings: Res<super::graphics::Settings>,
) {
    if existing.is_some() {
        return;
    }
    let Ok((main, environment)) = camera.single() else {
        return;
    };
    // Waiting for the source-sky generator's output handles avoids a black IBL.
    if images.get(&environment.diffuse_map).is_none()
        || images.get(&environment.specular_map).is_none()
    {
        return;
    }
    let limits = device.limits();
    println!(
        "scene_probe binding_array_limits textures={} samplers={}",
        limits.max_binding_array_elements_per_shader_stage,
        limits.max_binding_array_sampler_elements_per_shader_stage
    );
    assert!(
        device.features().contains(
            WgpuFeatures::TEXTURE_BINDING_ARRAY
                | WgpuFeatures::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING
        ),
        "scene probe requires native texture binding arrays"
    );
    commands.entity(main).insert(RenderLayers::layer(0).with(1));
    let mut excluded = 0;
    for (entity, part) in &parts {
        if part.0.starts_with("lacetti/") {
            commands.entity(entity).insert(RenderLayers::layer(1));
            excluded += 1;
        }
    }
    assert!(
        excluded > 0,
        "scene probe requires the reference/player Lacetti"
    );
    // The owner's car must still receive light and cast main-view shadows.
    for entity in &sun {
        commands
            .entity(entity)
            .insert(RenderLayers::layer(0).with(1));
    }
    let size = settings.probe_size;
    let mut cube = Image::new_fill(
        Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 6,
        },
        TextureDimension::D2,
        &[0; 8],
        TextureFormat::Rgba16Float,
        RenderAssetUsages::all(),
    );
    cube.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::COPY_SRC;
    cube.texture_view_descriptor = Some(TextureViewDescriptor {
        dimension: Some(TextureViewDimension::Cube),
        ..default()
    });
    let cube = images.add(cube);
    let mut faces = Vec::new();
    for face in 0..6 {
        let mut image = Image::new_target_texture(size, size, TextureFormat::Rgba16Float, None);
        image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
        let target = images.add(image);
        faces.push(target.clone());
        commands.spawn((
            ProbeCamera,
            Name::new(format!("HDR probe face {face}")),
            Camera3d::default(),
            Camera {
                order: -10 + face as isize,
                ..default()
            },
            RenderTarget::Image(target.into()),
            Projection::Perspective(PerspectiveProjection {
                fov: std::f32::consts::FRAC_PI_2,
                aspect_ratio: 1.,
                near: 0.05,
                far: 10000.,
                ..default()
            }),
            face_transform(face, position.0),
            Hdr,
            Exposure { ev100: EV },
            Tonemapping::None,
            Msaa::Off,
            RenderLayers::layer(0),
            environment.clone(),
        ));
    }
    commands.insert_resource(Cube {
        size,
        faces,
        cube,
        copied: Arc::new(AtomicBool::new(false)),
        read: Arc::new(AtomicBool::new(false)),
        bound: Arc::new(AtomicBool::new(false)),
    });
    println!(
        "scene_probe capture_setup=true size={size} center={:?} excluded_player_parts={excluded} format=Rgba16Float ev100={EV} tonemapping=None static=true",
        position.0
    );
}

fn copy_faces(
    cube: Option<Res<Cube>>,
    images: Res<RenderAssets<GpuImage>>,
    device: Res<RenderDevice>,
    queue: Res<RenderQueue>,
    cache: Res<PipelineCache>,
    mut frames: Local<u32>,
) {
    let Some(cube) = cube else {
        return;
    };
    if cube.copied.load(Ordering::Acquire) {
        return;
    }
    *frames += 1;
    if *frames < 60 || cache.waiting_pipelines().next().is_some() {
        return;
    }
    let Some(target) = images.get(&cube.cube) else {
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
        label: Some("HDR probe face copy"),
    });
    for (face, source) in faces.iter().enumerate() {
        assert_eq!(source.texture_descriptor.format, TextureFormat::Rgba16Float);
        let mut dst = target.texture.as_image_copy();
        dst.origin.z = face as u32;
        encoder.copy_texture_to_texture(
            source.texture.as_image_copy(),
            dst,
            Extent3d {
                width: cube.size,
                height: cube.size,
                depth_or_array_layers: 1,
            },
        );
    }
    queue.submit([encoder.finish()]);
    cube.copied.store(true, Ordering::Release);
    println!(
        "scene_probe GPU_copy_submitted=true faces=6 size={} HDR=true scope=queue-submission-not-readback",
        cube.size
    );
}

fn activate(
    mut commands: Commands,
    cube: Option<Res<Cube>>,
    cameras: Query<Entity, With<ProbeCamera>>,
    position: Res<Position>,
    mut applied: Local<bool>,
) {
    let Some(cube) = cube else {
        return;
    };
    if *applied || !cube.copied.load(Ordering::Acquire) {
        return;
    }
    // Freeze the snapshot before using it, so capture shading cannot feed back.
    for entity in &cameras {
        commands.entity(entity).despawn();
    }
    let intensity = 1. / Exposure { ev100: EV }.exposure();
    commands.spawn((
        Name::new("Static Donington HDR reflection probe"),
        LightProbe {
            falloff: Vec3::splat(0.25),
        },
        // Bevy computes the cubemap sample ray in probe space. An anisotropic
        // transform would stretch the captured world directions (including sky).
        // Keep the transform isotropic; specify the shorter Y box separately.
        Transform::from_translation(position.0).with_scale(Vec3::splat(60.)),
        ParallaxCorrection::Custom(Vec3::new(0.5, 0.25, 0.5)),
        GeneratedEnvironmentMapLight {
            environment_map: cube.cube.clone(),
            intensity,
            ..default()
        },
    ));
    commands
        .spawn(Readback::texture(cube.cube.clone()))
        .observe(audit_readback);
    *applied = true;
    println!(
        "scene_probe applied=true static=true influence=60x60x60 parallax_box=60x30x60 transform=isotropic falloff=0.25 sky_fallback=retained intensity={intensity} radiometry=inverse-capture-exposure filtering=Bevy-generator"
    );
}

fn report_binding(
    cube: Option<Res<Cube>>,
    views: Query<&bevy::pbr::RenderViewLightProbes<EnvironmentMapLight>>,
    generators: Query<&bevy::pbr::generate::GeneratorBindGroups>,
    pipelines: Option<Res<bevy::pbr::generate::GeneratorPipelines>>,
    cache: Res<PipelineCache>,
) {
    let Some(cube) = cube else {
        return;
    };
    if cube.bound.load(Ordering::Acquire) || !cube.copied.load(Ordering::Acquire) {
        return;
    }
    let Some(p) = pipelines else {
        return;
    };
    if [
        p.downsample_first,
        p.downsample_second,
        p.copy,
        p.radiance,
        p.irradiance,
    ]
    .iter()
    .any(|&id| cache.get_compute_pipeline(id).is_none())
    {
        return;
    }
    let bound = views
        .iter()
        .filter(|v| !v.main_entity_to_render_light_probe_index.is_empty())
        .count();
    if bound > 0 && generators.iter().count() >= 2 {
        cube.bound.store(true, Ordering::Release);
        println!(
            "scene_probe render_views_with_local_probe={bound} filter_bind_groups_ready=true sky_fallback=true scope=assignment-and-pipeline-readiness"
        );
    }
}

fn half(bits: u16) -> f32 {
    let sign = if bits & 0x8000 == 0 { 1. } else { -1. };
    let exponent = (bits >> 10) & 31;
    let mantissa = f32::from(bits & 1023) / 1024.;
    match exponent {
        0 => sign * mantissa * 2f32.powi(-14),
        31 => {
            if mantissa == 0. {
                sign * f32::INFINITY
            } else {
                f32::NAN
            }
        }
        e => sign * (1. + mantissa) * 2f32.powi(i32::from(e) - 15),
    }
}

fn audit_readback(event: On<ReadbackComplete>, mut commands: Commands, cube: Res<Cube>) {
    if cube.read.swap(true, Ordering::AcqRel) {
        return;
    }
    assert_eq!(event.data.len(), (cube.size * cube.size * 6 * 8) as usize);
    let mut maximum = 0f32;
    let mut above_one = 0;
    let mut face_nonzero = [0usize; 6];
    for (i, pixel) in event.data.as_chunks::<8>().0.iter().enumerate() {
        let rgb: [f32; 3] =
            std::array::from_fn(|c| half(u16::from_le_bytes([pixel[c * 2], pixel[c * 2 + 1]])));
        assert!(
            rgb.iter().all(|x| x.is_finite()),
            "HDR probe has nonfinite radiance"
        );
        let brightest = rgb.into_iter().fold(0f32, f32::max);
        maximum = maximum.max(brightest);
        above_one += usize::from(brightest > 1.);
        face_nonzero[i / (cube.size * cube.size) as usize] += usize::from(brightest > 0.00001);
    }
    assert!(
        face_nonzero.iter().all(|&n| n > 0),
        "HDR probe contains empty faces"
    );
    use std::io::Write;
    let path = std::path::PathBuf::from("local/graphics").join(format!(
        "scene-probe-{}-{}.rgba16f",
        super::BUILD_REVISION,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .expect("private HDR probe output");
    file.write_all(&event.data)
        .expect("private HDR probe write");
    println!(
        "scene_probe GPU_readback_verified=true bytes={} max_rgb={maximum} pixels_above_one={above_one} nonzero_per_face={face_nonzero:?} evidence={}",
        event.data.len(),
        path.display()
    );
    commands.entity(event.entity).despawn();
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn face_basis_and_half_radiance() {
        for face in 0..6 {
            let t = face_transform(face, Vec3::ZERO);
            assert!((t.rotation.length() - 1.).abs() < 1e-6);
            assert!((*t.forward() - direction(face, 0.5, 0.5)).length() < 1e-6);
            let d = direction(face, 0.7, 0.3);
            let projected = t.rotation.inverse() * d;
            let uv = Vec2::new(projected.x / -projected.z, projected.y / projected.z) * 0.5
                + Vec2::splat(0.5);
            assert!((uv - Vec2::new(0.7, 0.3)).length() < 1e-6);
        }
        assert_eq!(half(0x3c00), 1.);
        assert_eq!(half(0x4000), 2.);
        assert_eq!(half(0xbc00), -1.);
        assert!(half(0x7c00).is_infinite());
    }
}
