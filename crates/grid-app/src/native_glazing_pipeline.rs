//! Exact alternate PBR pipeline certification for an ordinary rigid native-DLAA pane.
//! Call after PrepareAssets and current view keys, before native mesh collection.
use bevy::{
    anti_alias::dlss::{Dlss, DlssPerfQualityMode},
    asset::{AssetId, UntypedAssetId},
    camera::MainPassResolutionOverride,
    core_pipeline::prepass::ViewPrepassTextures,
    diagnostic::FrameCount,
    material::{
        RenderPhaseType,
        key::{ErasedMaterialPipelineKey, ErasedMeshPipelineKey},
    },
    pbr::{
        MeshPipelineKey, MorphIndices, PreparedMaterial, RenderMeshInstanceFlags,
        RenderMeshInstances, SkinUniforms, ViewKeyCache, alpha_mode_pipeline_key,
    },
    prelude::*,
    render::{
        erased_render_asset::ErasedRenderAssets,
        mesh::RenderMesh,
        render_asset::RenderAssets,
        render_resource::{CachedRenderPipelineId, PipelineCache},
        sync_world::MainEntity,
        view::{ExtractedView, RetainedViewEntity, visibility::RenderVisibilityRanges},
    },
};

use crate::{native_dlaa::DlaaView, window_response::Window};

/// Source fields come from the current ExtractSchedule row for this Pane.
/// Lightmapped panes are excluded at that source boundary since Bevy keeps
/// its per-render-entity Lightmap table private.
#[derive(Clone, Copy)]
pub(super) struct PanePipeline {
    pub frame: u32,
    pub owner: MainEntity,
    pub mesh: AssetId<Mesh>,
    pub material: AssetId<Window>,
    pub view: Entity,
    pub retained_view: RetainedViewEntity,
    pub resolution: UVec2,
    pub has_source_lightmap: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Readiness {
    Stock,
    Waiting(CachedRenderPipelineId),
    Ready(CachedRenderPipelineId),
}

fn rigid_key(
    view_key: MeshPipelineKey,
    mesh_bits: MeshPipelineKey,
    material_bits: MeshPipelineKey,
    dither: bool,
) -> Option<MeshPipelineKey> {
    if (view_key | mesh_bits | material_bits).intersects(
        MeshPipelineKey::LIGHTMAPPED
            | MeshPipelineKey::LIGHTMAP_BICUBIC_SAMPLING
            | MeshPipelineKey::HAS_PREVIOUS_SKIN
            | MeshPipelineKey::HAS_PREVIOUS_MORPH,
    ) {
        return None;
    }
    let mut key = view_key | mesh_bits | material_bits;
    key |= alpha_mode_pipeline_key(
        AlphaMode::Blend,
        &Msaa::from_samples(view_key.msaa_samples()),
    );
    if dither {
        key |= MeshPipelineKey::VISIBILITY_RANGE_DITHER;
    }
    Some(key)
}

fn matching_native_view(world: &World, pane: PanePipeline) -> bool {
    let Ok(view) = world.get_entity(pane.view) else {
        return false;
    };
    let Some(camera) = view.get::<ExtractedView>() else {
        return false;
    };
    view.contains::<DlaaView>()
        && view
            .get::<Dlss>()
            .is_some_and(|feature| feature.perf_quality_mode == DlssPerfQualityMode::Dlaa)
        && view
            .get::<MainPassResolutionOverride>()
            .is_some_and(|input| input.0 == pane.resolution)
        && view
            .get::<ViewPrepassTextures>()
            .is_some_and(|p| p.depth.is_some() && p.motion_vectors.is_some())
        && camera.retained_view_entity == pane.retained_view
        && pane.retained_view == RetainedViewEntity::new(pane.retained_view.main_entity, None, 0)
        && pane.resolution == UVec2::new(camera.viewport.z, camera.viewport.w)
        && pane.resolution.min_element() != 0
}

fn has_animation(world: &World, pane: PanePipeline) -> Option<bool> {
    let skins = world.get_resource::<SkinUniforms>()?;
    let morphs = world.get_resource::<MorphIndices>()?;
    let morph = match morphs {
        MorphIndices::Uniform { current, prev } => {
            current.contains_key(&pane.owner) || prev.contains_key(&pane.owner)
        }
        MorphIndices::Storage {
            morph_weights_info, ..
        } => morph_weights_info.contains_key(&pane.owner),
    };
    Some(morph || skins.skin_index(pane.owner).is_some())
}

/// Ask the same Bevy specialization/cache used by ordinary transparent queueing.
/// A new key is queued asynchronously; the existing warmup counts it during
/// loading, and a mid-race material revision stays Stock until the exact ID is ready.
pub(super) fn certify(world: &mut World, pane: PanePipeline) -> Readiness {
    let work = (|| {
        if world.get_resource::<FrameCount>()?.0 != pane.frame
            || pane.has_source_lightmap
            || !matching_native_view(world, pane)
            || has_animation(world, pane)?
        {
            return None;
        }

        let instances = world.get_resource::<RenderMeshInstances>()?;
        if !matches!(instances, RenderMeshInstances::GpuBuilding(_)) {
            return None;
        }
        let current = instances.render_mesh_queue_data(pane.owner)?;
        if current.mesh_asset_id() != pane.mesh
            || current.shared.lightmap_slab_index().is_some()
            || current.flags().intersects(
                RenderMeshInstanceFlags::HAS_PREVIOUS_SKIN
                    | RenderMeshInstanceFlags::HAS_PREVIOUS_MORPH,
            )
        {
            return None;
        }
        let keys = world.get_resource::<ViewKeyCache>()?;
        let view_key = *keys.get(&pane.retained_view)?;
        let meshes = world.get_resource::<RenderAssets<RenderMesh>>()?;
        let mesh = meshes.get(pane.mesh)?;
        if mesh.has_morph_targets() {
            return None;
        }
        let materials = world.get_resource::<ErasedRenderAssets<PreparedMaterial>>()?;
        let candidate: UntypedAssetId = pane.material.into();
        let material = materials.get(candidate)?;
        let props = &material.properties;
        if props.alpha_mode != AlphaMode::Blend
            || !matches!(props.render_phase_type, RenderPhaseType::Transparent)
        {
            return None;
        }
        let visibility = world.get_resource::<RenderVisibilityRanges>()?;
        let key = rigid_key(
            view_key,
            MeshPipelineKey::from_bits_retain(mesh.key_bits.bits()),
            props.mesh_pipeline_key_bits.downcast(),
            visibility.entity_has_crossfading_visibility_ranges(pane.owner),
        )?;
        Some((
            props.base_specialize?,
            ErasedMaterialPipelineKey {
                type_id: candidate.type_id(),
                mesh_key: ErasedMeshPipelineKey::new(key),
                material_key: props.material_key.clone(),
            },
            mesh.layout.clone(),
            props.clone(),
        ))
    })();
    let Some((specialize, key, layout, props)) = work else {
        return Readiness::Stock;
    };
    let Ok(pipeline) = specialize(world, key, &layout, &props) else {
        return Readiness::Stock;
    };
    if world
        .resource::<PipelineCache>()
        .get_render_pipeline(pipeline)
        .is_some()
    {
        Readiness::Ready(pipeline)
    } else {
        Readiness::Waiting(pipeline)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rigid_blend_key_matches_bevy_components_and_refuses_unknown_subpaths() {
        let view = MeshPipelineKey::TEMPORAL_JITTER | MeshPipelineKey::MOTION_VECTOR_PREPASS;
        let mesh = MeshPipelineKey::MAY_DISCARD;
        let material = MeshPipelineKey::READS_VIEW_TRANSMISSION_TEXTURE;
        let normal = rigid_key(view, mesh, material, false).unwrap();
        assert_eq!(
            normal,
            view | mesh | material | MeshPipelineKey::BLEND_ALPHA
        );
        assert_eq!(
            rigid_key(view, mesh, material, true).unwrap(),
            normal | MeshPipelineKey::VISIBILITY_RANGE_DITHER,
        );
        for extended in [
            MeshPipelineKey::LIGHTMAPPED,
            MeshPipelineKey::LIGHTMAP_BICUBIC_SAMPLING,
            MeshPipelineKey::HAS_PREVIOUS_MORPH,
            MeshPipelineKey::HAS_PREVIOUS_SKIN,
        ] {
            assert!(rigid_key(view, mesh | extended, material, false).is_none());
        }
    }
}
