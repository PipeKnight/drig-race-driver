//! Car-glass panes draw through a copy of their material in a small bindless
//! slab of their own (docs/PERFORMANCE.md "Car-glass panes off the bindless slab").
//!
//! A blended car part with a reflection overlay (window, head and rear light
//! glass of every car) sorts next to its overlay in the transparent phase, so
//! the pass switches between the StandardMaterial slab and the overlay's slab
//! for every pane. Each switch binds the whole slab (about 500 textures on a
//! full grid), and wgpu's `CommandEncoder::finish` pays per bound resource.
//! The pane copies share one small slab that holds only the panes' textures,
//! so the switch is cheap, and the panes still batch with each other in the
//! shadow passes (a non-bindless copy per pane broke those batches: F876).
//!
//! The part keeps its `MeshMaterial3d<StandardMaterial>`: damage, lamps,
//! batching and the overlay go on reading and writing it. A [`PaneDraw`]
//! names the copy, which follows every change of the source material in the
//! same frame. The render world points the part's material instance at the
//! copy once the copy is prepared, and back at the source when the part
//! stops being a pane. The copy runs the same StandardMaterial shader, which
//! ignores the extension's placeholder binding, so the picture is the same.
//!
//! Copies are made only where StandardMaterial is bindless. Elsewhere there is
//! no slab to re-bind, and Bevy's `ExtendedMaterial` would mix
//! StandardMaterial's non-bindless layout with this extension's bindless one:
//! the extension has no samplers, so it passes the sampler binding-array limit
//! that StandardMaterial fails (DX12 here: 2048 elements), and creating the
//! copy's bind group fails validation (F881).

use bevy::{
    asset::{AssetEventSystems, UntypedAssetId},
    pbr::{
        ExtendedMaterial, MaterialExtension, MaterialExtractionSystems, MeshExtractionSystems,
        MeshesToReextractNextFrame, RenderMaterialBindings, RenderMaterialInstances,
        RenderMeshInstances,
    },
    platform::collections::HashMap,
    prelude::*,
    render::{
        Extract, ExtractSchedule, RenderApp,
        camera::{DirtySpecializationSystems, DirtySpecializations},
        render_resource::{AsBindGroup, ShaderType},
        renderer::RenderDevice,
        sync_world::{MainEntity, MainEntityHashSet},
    },
};

use crate::car_reflection_overlay::HasOverlay;

/// StandardMaterial with a placeholder bindless extension. An extended
/// material is bindless only when both halves are, and it needs its own
/// material type to get a slab apart from StandardMaterial's. The extension's
/// index table and data sit at bindings 100 and 101, clear of StandardMaterial's.
pub(crate) type Pane = ExtendedMaterial<StandardMaterial, PaneSlab>;

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug, Default)]
#[data(50, PaneSlabUniform, binding_array(101))]
#[bindless(index_table(range(50..51), binding(100)))]
pub struct PaneSlab {}

/// The extension's GPU data; nothing reads it.
#[derive(Clone, Default, ShaderType)]
pub struct PaneSlabUniform {
    unused: u32,
}

impl From<&PaneSlab> for PaneSlabUniform {
    fn from(_: &PaneSlab) -> Self {
        Self::default()
    }
}

impl MaterialExtension for PaneSlab {}

/// The pane's own copy of its StandardMaterial.
#[derive(Component, Clone, Debug)]
pub(crate) struct PaneDraw(Handle<Pane>);

/// One copy per source material.
#[derive(Resource, Default)]
struct Copies(HashMap<AssetId<StandardMaterial>, Handle<Pane>>);

pub(crate) fn install(app: &mut App) {
    app.add_plugins(MaterialPlugin::<Pane>::default())
        .init_resource::<Copies>()
        .add_systems(
            PostUpdate,
            (assign, follow)
                .chain()
                .run_if(pane_bindless)
                .after(crate::car_reflection_overlay::Systems)
                .after(AssetEventSystems),
        );
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(
            ExtractSchedule,
            redirect
                .after(MaterialExtractionSystems)
                .after(DirtySpecializationSystems::Clear)
                .before(MeshExtractionSystems),
        );
    }
}

fn is_pane(material: &StandardMaterial) -> bool { material.alpha_mode == AlphaMode::Blend }

fn copy_of(
    source: &Handle<StandardMaterial>,
    materials: &Assets<StandardMaterial>,
    panes: &mut Assets<Pane>,
    copies: &mut Copies,
) -> Option<Handle<Pane>> {
    let material = materials.get(source).filter(|m| is_pane(m))?;
    Some(
        copies
            .0
            .entry(source.id())
            .or_insert_with(|| panes.add(Pane { base: material.clone(), extension: PaneSlab {} }))
            .clone(),
    )
}

/// Give each blended part with an overlay its copy; take it back when the
/// part loses the overlay or its material stops blending.
#[allow(clippy::type_complexity)]
/// Whether pane copies run on this device: only where StandardMaterial itself
/// is bindless (Bevy's own condition) and the extension is too, so the two
/// halves of [`Pane`] never disagree (see the module notes).
pub(crate) fn copies_enabled(device: &RenderDevice) -> bool {
    <StandardMaterial as AsBindGroup>::bindless_supported(device)
        && <PaneSlab as AsBindGroup>::bindless_supported(device)
}

/// Run condition over [`copies_enabled`]; logged once.
fn pane_bindless(device: Option<Res<RenderDevice>>, mut known: Local<Option<bool>>) -> bool {
    if let Some(on) = *known {
        return on;
    }
    let Some(device) = device else { return false };
    let on = copies_enabled(&device);
    println!("pane_slab version=1 enabled={on}");
    *known = Some(on);
    on
}

/// Overlaid parts whose material changed or that just got an overlay.
type Reassigned = (With<HasOverlay>, Or<(Changed<MeshMaterial3d<StandardMaterial>>, Added<HasOverlay>)>);

fn assign(
    mut commands: Commands,
    changed: Query<(Entity, &MeshMaterial3d<StandardMaterial>, Option<&PaneDraw>), Reassigned>,
    mut lost: RemovedComponents<HasOverlay>,
    drawn: Query<(), (With<PaneDraw>, Without<HasOverlay>)>,
    materials: Res<Assets<StandardMaterial>>,
    mut panes: ResMut<Assets<Pane>>,
    mut copies: ResMut<Copies>,
) {
    for (entity, material, current) in &changed {
        match copy_of(&material.0, &materials, &mut panes, &mut copies) {
            Some(copy) if current.is_none_or(|c| c.0 != copy) => {
                commands.entity(entity).insert(PaneDraw(copy));
            }
            Some(_) => {}
            None if current.is_some() => {
                commands.entity(entity).remove::<PaneDraw>();
            }
            None => {}
        }
    }
    for entity in lost.read() {
        if drawn.contains(entity) {
            commands.entity(entity).remove::<PaneDraw>();
        }
    }
}

/// Copy each change of a source material into its pane copy in the same
/// frame. The copy's change message is written here directly: a tracked
/// write would reach the render world only one frame later, behind the lamp
/// that changed it.
fn follow(
    mut events: MessageReader<AssetEvent<StandardMaterial>>,
    materials: Res<Assets<StandardMaterial>>,
    mut panes: ResMut<Assets<Pane>>,
    mut copies: ResMut<Copies>,
    mut changed: MessageWriter<AssetEvent<Pane>>,
) {
    for event in events.read() {
        match *event {
            AssetEvent::Modified { id } => {
                let (Some(copy), Some(source)) = (copies.0.get(&id), materials.get(id)) else {
                    continue;
                };
                if let Some(pane) = panes.get_mut_untracked(copy) {
                    pane.base = source.clone();
                    changed.write(AssetEvent::Modified { id: copy.id() });
                }
            }
            AssetEvent::Removed { id } | AssetEvent::Unused { id } => {
                copies.0.remove(&id);
            }
            _ => {}
        }
    }
}

/// Point each pane's material instance at its prepared copy, and a former
/// pane back at its own material. Only the GPU mesh building path resolves
/// the binding after extraction; the CPU path keeps the source material.
#[allow(clippy::too_many_arguments)]
fn redirect(
    drawn: Extract<Query<(Entity, &PaneDraw)>>,
    sources: Extract<Query<&MeshMaterial3d<StandardMaterial>>>,
    meshes: Res<RenderMeshInstances>,
    bindings: Res<RenderMaterialBindings>,
    mut instances: ResMut<RenderMaterialInstances>,
    mut dirty: ResMut<DirtySpecializations>,
    mut reextract: ResMut<MeshesToReextractNextFrame>,
    mut redirected: Local<MainEntityHashSet>,
    mut now: Local<MainEntityHashSet>,
) {
    if !matches!(*meshes, RenderMeshInstances::GpuBuilding(_)) {
        return;
    }
    now.clear();
    let mut point = |entity: MainEntity, id: UntypedAssetId| {
        if let Some(instance) = instances.instances.get_mut(&entity)
            && instance.asset_id != id
        {
            instance.asset_id = id;
            dirty.changed_renderables.insert(entity);
            reextract.insert(entity);
        }
    };
    for (entity, pane) in &drawn {
        let id = pane.0.id().untyped();
        // The source draws until the copy is prepared.
        if bindings.contains_key(&id) {
            now.insert(MainEntity::from(entity));
            point(MainEntity::from(entity), id);
        }
    }
    for &entity in redirected.difference(&now) {
        if let Ok(source) = sources.get(*entity) {
            point(entity, source.id().untyped());
        }
    }
    std::mem::swap(&mut *redirected, &mut *now);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real device of the first adapter of `backends`, with `drop` removed
    /// from the adapter's features; None when there is no such adapter.
    fn device(backends: wgpu::Backends, limit: impl FnOnce(&mut wgpu::Limits)) -> Option<RenderDevice> {
        let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
        desc.backends = backends;
        let instance = wgpu::Instance::new(desc);
        let Some(adapter) = bevy::tasks::block_on(instance.enumerate_adapters(backends)).into_iter().next() else {
            println!("{backends:?}: no adapter, skipped");
            return None;
        };
        let mut required_limits = adapter.limits();
        limit(&mut required_limits);
        // Experimental features need an unsafe opt-in; the bindless checks do not use them.
        let descriptor = wgpu::DeviceDescriptor {
            required_features: adapter.features() - wgpu::Features::all_experimental_mask(),
            required_limits,
            ..Default::default()
        };
        match bevy::tasks::block_on(adapter.request_device(&descriptor)) {
            Ok((device, _queue)) => Some(RenderDevice::from(device)),
            Err(error) => {
                println!("{backends:?}: no device ({error}), skipped");
                None
            }
        }
    }

    /// The gate is StandardMaterial's bindless condition on real devices. The
    /// data-only extension has no samplers, so it passes the sampler-array limit
    /// that StandardMaterial needs: a device below that limit (DX12 here) must
    /// get no copies. Skips the devices this machine does not have.
    #[test]
    fn copies_run_only_where_standard_material_is_bindless() {
        let full = device(wgpu::Backends::VULKAN, |_| ());
        let few_samplers =
            device(wgpu::Backends::VULKAN, |limits| limits.max_binding_array_sampler_elements_per_shader_stage = 1);
        let dx12 = device(wgpu::Backends::DX12, |_| ());
        let mut seen = 0;
        for (name, device) in [("vulkan", &full), ("vulkan-few-sampler-arrays", &few_samplers), ("dx12", &dx12)] {
            let Some(device) = device else { continue };
            seen += 1;
            let base = <StandardMaterial as AsBindGroup>::bindless_supported(device);
            let on = copies_enabled(device);
            let (features, limits) = (device.features(), device.limits());
            println!(
                "{name}: standard_bindless={base} extension_bindless={} copies={on} texture_arrays={} buffer_arrays={} storage_buffers={} sampler_array_elements={}",
                <PaneSlab as AsBindGroup>::bindless_supported(device),
                features.contains(wgpu::Features::TEXTURE_BINDING_ARRAY),
                features.contains(wgpu::Features::BUFFER_BINDING_ARRAY),
                limits.max_storage_buffers_per_shader_stage,
                limits.max_binding_array_sampler_elements_per_shader_stage,
            );
            assert_eq!(on, base, "{name}: the gate is StandardMaterial's condition");
        }
        if let Some(device) = &few_samplers {
            assert!(<PaneSlab as AsBindGroup>::bindless_supported(device), "the extension alone stays bindless");
            assert!(!<StandardMaterial as AsBindGroup>::bindless_supported(device));
            assert!(!copies_enabled(device), "base non-bindless: no copies");
        }
        println!("devices checked: {seen}");
    }

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Pane>>()
            .init_resource::<Copies>()
            .add_message::<AssetEvent<StandardMaterial>>()
            .add_message::<AssetEvent<Pane>>()
            .add_systems(Update, (assign, follow).chain());
        app
    }

    fn material(app: &mut App, alpha_mode: AlphaMode) -> Handle<StandardMaterial> {
        app.world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial { alpha_mode, ..default() })
    }

    #[test]
    fn blended_parts_with_an_overlay_draw_their_copy() {
        let mut app = app();
        let glass = material(&mut app, AlphaMode::Blend);
        let opaque = material(&mut app, AlphaMode::Opaque);
        let pane = app.world_mut().spawn((MeshMaterial3d(glass.clone()), HasOverlay(Entity::PLACEHOLDER))).id();
        let twin = app.world_mut().spawn((MeshMaterial3d(glass.clone()), HasOverlay(Entity::PLACEHOLDER))).id();
        let body = app.world_mut().spawn((MeshMaterial3d(opaque), HasOverlay(Entity::PLACEHOLDER))).id();
        let bare = app.world_mut().spawn(MeshMaterial3d(glass.clone())).id();
        app.update();
        let copy = app.world().get::<PaneDraw>(pane).expect("pane has a copy").0.clone();
        assert_eq!(app.world().get::<PaneDraw>(twin).unwrap().0, copy, "one copy per material");
        assert!(app.world().get::<PaneDraw>(body).is_none());
        assert!(app.world().get::<PaneDraw>(bare).is_none());

        // A source change reaches the copy in the same update.
        app.world_mut().resource_mut::<Assets<StandardMaterial>>().get_mut_untracked(&glass).unwrap().emissive =
            LinearRgba::rgb(2., 1., 0.);
        app.world_mut().write_message(AssetEvent::<StandardMaterial>::Modified { id: glass.id() });
        app.update();
        let panes = app.world().resource::<Assets<Pane>>();
        assert_eq!(panes.get(&copy).unwrap().base.emissive, LinearRgba::rgb(2., 1., 0.));

        app.world_mut().entity_mut(pane).remove::<HasOverlay>();
        app.update();
        assert!(app.world().get::<PaneDraw>(pane).is_none(), "no overlay, no copy");
        assert!(app.world().get::<PaneDraw>(twin).is_some());
    }

    #[test]
    fn a_part_that_stops_blending_draws_its_own_material() {
        let mut app = app();
        let glass = material(&mut app, AlphaMode::Blend);
        let pane = app.world_mut().spawn((MeshMaterial3d(glass), HasOverlay(Entity::PLACEHOLDER))).id();
        app.update();
        assert!(app.world().get::<PaneDraw>(pane).is_some());
        let solid = material(&mut app, AlphaMode::Opaque);
        app.world_mut().entity_mut(pane).insert(MeshMaterial3d(solid));
        app.update();
        assert!(app.world().get::<PaneDraw>(pane).is_none());
    }
}
