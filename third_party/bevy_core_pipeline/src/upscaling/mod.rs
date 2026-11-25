use crate::blit::{BlitPipeline, BlitPipelineKey};
use bevy_app::prelude::*;
use bevy_camera::{CameraOutputMode, NormalizedRenderTarget};
use bevy_ecs::prelude::*;
use bevy_render::{
    camera::ExtractedCamera, render_resource::*, view::ViewTarget, Render, RenderApp,
    RenderStartup, RenderSystems,
};

mod node;

pub use node::upscaling;

pub struct UpscalingPlugin;

impl Plugin for UpscalingPlugin {
    fn build(&self, app: &mut App) {
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app.add_systems(
                Render,
                // This system should probably technically be run *after* all of the other systems
                // that might modify `PipelineCache` via interior mutability, but for now,
                // we've chosen to simply ignore the ambiguities out of a desire for a better refactor
                // and aversion to extensive and intrusive system ordering.
                // See https://github.com/bevyengine/bevy/issues/14770 for more context.
                prepare_view_upscaling_pipelines
                    .in_set(RenderSystems::Prepare)
                    .ambiguous_with_all(),
            );
            render_app.add_systems(RenderStartup, clear_view_upscaling_pipelines);
        }
    }
}

// DRIG patch (F927): the third field is the pre-rotation of a window target (degrees).
#[derive(Component)]
pub struct ViewUpscalingPipeline(CachedRenderPipelineId, BlitPipelineKey, pub(crate) u32);

/// This is not required on first startup but is required during render recovery
fn clear_view_upscaling_pipelines(
    mut commands: Commands,
    views: Query<Entity, With<ViewUpscalingPipeline>>,
) {
    for entity in &views {
        commands.entity(entity).remove::<ViewUpscalingPipeline>();
    }
}

fn prepare_view_upscaling_pipelines(
    mut commands: Commands,
    mut pipeline_cache: ResMut<PipelineCache>,
    mut pipelines: ResMut<SpecializedRenderPipelines<BlitPipeline>>,
    // DRIG patch (F927): pre-rotated upscaling pipelines by key and degrees.
    mut rotated: Local<
        std::collections::HashMap<(BlitPipelineKey, u32), CachedRenderPipelineId>,
    >,
    blit_pipeline: Res<BlitPipeline>,
    view_targets: Query<(
        Entity,
        &ViewTarget,
        Option<&ExtractedCamera>,
        Option<&ViewUpscalingPipeline>,
    )>,
) {
    for (entity, view_target, camera, maybe_pipeline) in view_targets.iter() {
        let blend_state = if let Some(extracted_camera) = camera {
            match extracted_camera.output_mode {
                CameraOutputMode::Skip => None,
                CameraOutputMode::Write { blend_state, .. } => {
                    match blend_state {
                        None => {
                            // Auto-detect: the first camera to render to this output
                            // (sorted_camera_index_for_target == 0) uses replace mode;
                            // subsequent cameras default to alpha blending so they don't
                            // accidentally overwrite earlier cameras' output.
                            if extracted_camera.sorted_camera_index_for_target > 0 {
                                Some(BlendState::ALPHA_BLENDING)
                            } else {
                                None
                            }
                        }
                        _ => blend_state,
                    }
                }
            }
        } else {
            None
        };

        let Some(target_format) = view_target.out_texture_view_format() else {
            continue;
        };

        let key = BlitPipelineKey {
            target_format,
            blend_state,
            samples: 1,
            source_space: view_target.compositing_space,
        };

        // DRIG patch (F927): a camera drawing to a window draws rotated under Vulkan
        // pre-rotation (bevy_render::view::window::drig_pre_rotation_degrees).
        let degrees = match camera.and_then(|c| c.target.as_ref()) {
            Some(NormalizedRenderTarget::Window(_)) => {
                bevy_render::view::window::drig_pre_rotation_degrees()
            }
            _ => 0,
        };

        if maybe_pipeline.is_none_or(|ViewUpscalingPipeline(_, cached_key, cached_degrees)| {
            *cached_key != key || *cached_degrees != degrees
        }) {
            let pipeline = if degrees == 0 {
                pipelines.specialize(&pipeline_cache, &blit_pipeline, key)
            } else {
                let mut descriptor = blit_pipeline.specialize(key);
                if let Some(fragment) = descriptor.fragment.as_mut() {
                    fragment
                        .shader_defs
                        .push(format!("DRIG_PRE_ROTATE_{degrees}").as_str().into());
                }
                descriptor.label = Some("blit pipeline (pre-rotated)".into());
                *rotated.entry((key, degrees)).or_insert_with(|| {
                    pipeline_cache.queue_render_pipeline(descriptor)
                })
            };

            // Ensure the pipeline is loaded before continuing the frame to prevent frames without
            // any GPU work submitted
            pipeline_cache.block_on_render_pipeline(pipeline);

            commands
                .entity(entity)
                .insert(ViewUpscalingPipeline(pipeline, key, degrees));
        }
    }
}
