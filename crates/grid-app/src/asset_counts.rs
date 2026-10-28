//! `GRID_ASSET_COUNTS=1`: spawned entities, asset counts and wgpu resources of
//! the main and render worlds, printed every 600 frames (memory leak checks,
//! docs/PERFORMANCE.md "Memory growth"). Nothing is added without the variable.
use bevy::ecs::entity_disabling::Disabled;
use bevy::ecs::query::Allow;
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
    // The mesh report needs every kind registered, spawned or not.
    let world = app.world_mut();
    world.register_component::<crate::static_batch::Batch>();
    world.register_component::<crate::car_batch::Batch>();
    world.register_component::<crate::track_surface::SourcePart>();
    world.register_component::<crate::dynamic_props::IntactProp>();
    world.register_component::<crate::dynamic_props::PropPiece>();
    world.register_component::<crate::source_trees::Facing>();
    world.register_component::<crate::MovingPart>();
    world.register_component::<crate::OpponentPart>();
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
    mesh_report(world, *frames);
}

/// Mesh entities by kind: all, disabled, and drawn in the last frame.
fn mesh_report(world: &World, frame: u32) {
    type Kinds = (
        Has<Disabled>,
        Has<crate::static_batch::Batch>,
        Has<crate::car_batch::Batch>,
        Has<crate::track_surface::SourcePart>,
        Has<crate::dynamic_props::IntactProp>,
        Has<crate::dynamic_props::PropPiece>,
        Has<crate::source_trees::Facing>,
        Has<crate::MovingPart>,
        Has<crate::OpponentPart>,
        &'static ViewVisibility,
        &'static Mesh3d,
    );
    let Some(mut query) = world.try_query_filtered::<Kinds, (With<Mesh3d>, Allow<Disabled>)>() else {
        return;
    };
    let names = ["batch", "car_batch", "intact", "piece", "foliage", "player", "opponent", "track", "other"];
    let mut counts = [[0usize; 3]; 9];
    // Distinct mesh assets per kind, and all that some entity draws.
    let mut assets: [std::collections::HashSet<AssetId<Mesh>>; 9] = Default::default();
    for (disabled, batch, car_batch, track, intact, piece, foliage, player, opponent, seen, mesh) in query.iter(world) {
        let kind = [batch, car_batch, intact, piece, foliage, player, opponent, track]
            .iter()
            .position(|&k| k)
            .unwrap_or(8);
        counts[kind][0] += 1;
        counts[kind][1] += usize::from(disabled);
        counts[kind][2] += usize::from(!disabled && seen.get());
        assets[kind].insert(mesh.id());
    }
    let used: std::collections::HashSet<_> = assets.iter().flatten().collect();
    let fields: Vec<String> = names
        .iter()
        .zip(counts)
        .zip(&assets)
        .map(|((n, [all, off, seen]), meshes)| format!("{n}={all}/{off}/{seen}/{}", meshes.len()))
        .collect();
    println!(
        "mesh_entities frame={frame} kind=all/disabled/drawn/meshes {} meshes_on_entities={} meshes_without_entity={}",
        fields.join(" "),
        used.len(),
        len::<Mesh>(world).saturating_sub(used.len()),
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
