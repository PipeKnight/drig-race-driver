//! `GRID_ASSET_COUNTS=1`: spawned entities, asset counts and wgpu resources of
//! the main and render worlds, printed every 600 frames (memory leak checks,
//! docs/PERFORMANCE.md "Memory growth"). Nothing is added without the variable.
use bevy::mesh::Mesh;
use bevy::prelude::*;
use bevy::render::mesh::RenderMesh;
use bevy::render::render_asset::RenderAssets;
use bevy::render::renderer::{RenderDevice, RenderInstance};
use bevy::render::texture::GpuImage;
use bevy::render::{Render, RenderApp, RenderSystems};

const REPORT_FRAMES: u32 = 600;

pub fn install(app: &mut App) {
    if std::env::var_os("GRID_ASSET_COUNTS").is_none() {
        return;
    }
    app.add_systems(Last, main_report);
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(Render, render_report.after(RenderSystems::Cleanup));
    }
}

fn len<A: Asset>(world: &World) -> usize {
    world.get_resource::<Assets<A>>().map_or(0, |a| a.len())
}

fn main_report(world: &World, mut frames: Local<u32>) {
    *frames += 1;
    if !(*frames).is_multiple_of(REPORT_FRAMES) {
        return;
    }
    println!(
        "asset_counts world=main frame={} entities={} meshes={} standard_materials={} images={}",
        *frames,
        world.entities().count_spawned(),
        len::<Mesh>(world),
        len::<StandardMaterial>(world),
        len::<Image>(world),
    );
}

fn render_report(world: &World, mut frames: Local<u32>) {
    *frames += 1;
    if !(*frames).is_multiple_of(REPORT_FRAMES) {
        return;
    }
    if let Some(instance) = world.get_resource::<RenderInstance>()
        && let Some(report) = instance.0.generate_report()
    {
        let h = report.hub_report();
        println!(
            "wgpu_counts frame={} buffers={} textures={} views={} bind_groups={} samplers={} pipelines={}",
            *frames,
            h.buffers.num_allocated,
            h.textures.num_allocated,
            h.texture_views.num_allocated,
            h.bind_groups.num_allocated,
            h.samplers.num_allocated,
            h.render_pipelines.num_allocated,
        );
    }
    if let Some(device) = world.get_resource::<RenderDevice>()
        && let Some(report) = device.wgpu_device().generate_allocator_report()
    {
        println!(
            "wgpu_memory frame={} allocations={} allocated_bytes={} reserved_bytes={}",
            *frames,
            report.allocations.len(),
            report.total_allocated_bytes,
            report.total_reserved_bytes,
        );
    }
    println!(
        "asset_counts world=render frame={} entities={} meshes={} images={}",
        *frames,
        world.entities().count_spawned(),
        world
            .get_resource::<RenderAssets<RenderMesh>>()
            .map_or(0, |a| a.iter().count()),
        world
            .get_resource::<RenderAssets<GpuImage>>()
            .map_or(0, |a| a.iter().count()),
    );
}
