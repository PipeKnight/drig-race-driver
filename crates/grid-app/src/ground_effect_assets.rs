//! Textures for ground effects, read at runtime from the purchased
//! installation's PSSG packages (never copied into the repository).
use bevy::{
    asset::RenderAssetUsages,
    image::{
        CompressedImageFormats, ImageAddressMode, ImageSampler, ImageSamplerDescriptor, ImageType,
    },
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};
use grid_formats::{pssg, texture};
use std::{
    collections::BTreeMap,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

/// Validated DDS bytes of the listed textures of one PSSG package, keyed by id.
pub(crate) fn package(
    root: &Path,
    relative: &str,
    ids: &[&str],
) -> Result<BTreeMap<String, Vec<u8>>, Box<dyn std::error::Error>> {
    let bytes = super::read(&root.join(relative))?;
    let tree = pssg::read_tree(&bytes)?;
    let ids: Vec<&[u8]> = ids.iter().map(|id| id.as_bytes()).collect();
    let mut textures = BTreeMap::new();
    for texture in texture::read_named(&tree, &ids)? {
        let (dds, _) = texture.validated_dds();
        textures.insert(String::from_utf8(texture.id.to_vec())?, dds);
    }
    Ok(textures)
}

pub(crate) fn image(dds: &[u8], repeat: bool) -> Result<Image, Box<dyn std::error::Error>> {
    let mode = if repeat {
        ImageAddressMode::Repeat
    } else {
        ImageAddressMode::ClampToEdge
    };
    Ok(Image::from_buffer(
        dds,
        ImageType::Extension("dds"),
        CompressedImageFormats::BC,
        true,
        ImageSampler::Descriptor(ImageSamplerDescriptor {
            address_mode_u: mode,
            address_mode_v: ImageAddressMode::Repeat,
            ..ImageSamplerDescriptor::linear()
        }),
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )?)
}

/// Scripted validation only: `GRID_EFFECTS_CAMERA=x,y,z,lx,ly,lz` holds the
/// driving camera at a fixed eye/target after the chase camera update.
pub(crate) fn validation_camera(
    mut camera: Single<&mut Transform, With<super::ViewerCamera>>,
    mut fixed: Local<Option<Option<(Vec3, Vec3)>>>,
) {
    let fixed = fixed.get_or_insert_with(|| {
        let text = std::env::var("GRID_EFFECTS_CAMERA").ok()?;
        let v: Vec<f32> = text
            .split(',')
            .filter_map(|x| x.trim().parse().ok())
            .collect();
        (v.len() == 6).then(|| (Vec3::new(v[0], v[1], v[2]), Vec3::new(v[3], v[4], v[5])))
    });
    if let Some((eye, at)) = *fixed {
        **camera = Transform::from_translation(eye).looking_at(at, Vec3::Y);
    }
}

/// `GRID_GROUND_EFFECTS=0` turns skid marks, wheel debris and ground cover off
/// (A/B performance checks).
pub(crate) fn enabled() -> bool {
    std::env::var("GRID_GROUND_EFFECTS").map_or(true, |v| v != "0")
}

/// Accumulated CPU time of the ground-effect systems: skid marks, debris,
/// ground cover and the per-tick wheel sampling/simulation.
pub(crate) static COST: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
pub(crate) const SKID: usize = 0;
pub(crate) const DEBRIS: usize = 1;
pub(crate) const GRASS: usize = 2;
pub(crate) const TICK: usize = 3;

pub(crate) fn charge(slot: usize, start: Instant) {
    COST[slot].fetch_add(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
}

#[derive(Resource)]
struct TimingInstalled;

/// With `GRID_GROUND_TRACE=1`, print the mean per-frame cost every 600 frames.
pub(crate) fn install_timing(app: &mut App) {
    if app.world().contains_resource::<TimingInstalled>()
        || std::env::var_os("GRID_GROUND_TRACE").is_none()
    {
        return;
    }
    app.insert_resource(TimingInstalled)
        .add_systems(Last, report);
}

fn report(mut frames: Local<u64>) {
    *frames += 1;
    if !(*frames).is_multiple_of(600) {
        return;
    }
    let ms = |slot: usize| COST[slot].swap(0, Ordering::Relaxed) as f64 / 1e6 / 600.;
    println!(
        "ground_effects_cost frames={} skid_ms={:.3} debris_ms={:.3} grass_ms={:.3} tick_ms={:.3}",
        *frames,
        ms(SKID),
        ms(DEBRIS),
        ms(GRASS),
        ms(TICK)
    );
}

/// Dynamic effect mesh. Bevy's mesh allocator cannot hold zero-length
/// buffers (it logs a use-after-free on every update), so an empty mesh
/// carries one invisible degenerate triangle.
pub(crate) fn effect_mesh(
    mut positions: Vec<[f32; 3]>,
    mut normals: Vec<[f32; 3]>,
    mut uvs: Vec<[f32; 2]>,
    mut colors: Vec<[f32; 4]>,
    mut indices: Vec<u32>,
) -> Mesh {
    if indices.is_empty() {
        let base = positions.len() as u32;
        for _ in 0..3 {
            positions.push([0.; 3]);
            normals.push([0., 1., 0.]);
            uvs.push([0.; 2]);
            colors.push([0.; 4]);
        }
        indices.extend([base, base, base]);
    }
    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, colors);
    mesh.insert_indices(Indices::U32(indices));
    mesh
}
