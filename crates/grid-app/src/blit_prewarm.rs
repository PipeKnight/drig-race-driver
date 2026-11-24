//! Upscaling (blit) pipelines made before the cameras that need them
//! (item 6 of the Mac performance list; docs/PIPELINE_WARMUP.md, "Blit
//! pre-warm").
//!
//! Bevy's `prepare_view_upscaling_pipelines` specializes the final blit of a
//! view the first frame that view's `BlitPipelineKey` (output format, blend,
//! compositing space) is new, then calls `block_on_render_pipeline`, which
//! stops the render thread until that compile finishes. A compile queued that
//! late waits behind every pipeline already queued, so a camera that appears
//! during the scene warm-up (the water probe's six Rgba16Float faces, spawned
//! at the Restore phase) blocked one frame for 0.9 s with a warm driver cache
//! and 1.7 s with a cold one (F-entry of item 6).
//!
//! This module queues those keys as soon as a possible target exists: every
//! window surface format and the view format of every image that can be a
//! render attachment, with the replace blend (the first camera on a target)
//! and alpha blending (later cameras on the same target), for the default
//! compositing space and any space a camera uses. Specializing is cached by
//! key, so the camera later gets the same pipeline id, already compiled, and
//! the block costs nothing. The image is unchanged: it is the same pipeline,
//! made earlier. Works for every track, car and graphics mode, since the keys
//! come from the targets that exist, not from a list.

use bevy::camera::CompositingSpace;
use bevy::core_pipeline::blit::{BlitPipeline, BlitPipelineKey};
use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::render::{
    camera::ExtractedCamera,
    render_asset::RenderAssets,
    render_resource::{
        BlendState, PipelineCache, SpecializedRenderPipelines, TextureFormat, TextureFormatFeatureFlags,
        TextureSampleType, TextureUsages,
    },
    renderer::RenderAdapter,
    texture::GpuImage,
    view::ExtractedWindows,
    Render, RenderApp, RenderSystems,
};

/// Output formats and compositing spaces already queued.
#[derive(Resource, Default)]
struct Queued {
    formats: HashSet<TextureFormat>,
    spaces: HashSet<Option<CompositingSpace>>,
}

pub fn install(app: &mut App) {
    if crate::dev::var("GRID_BLIT_PREWARM").is_ok_and(|v| v == "0") {
        return; // comparison runs
    }
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.init_resource::<Queued>().add_systems(
            Render,
            prewarm.in_set(RenderSystems::Queue),
        );
    }
}

/// Whether a blit (fullscreen pass writing `vec4<f32>`) can target `format`;
/// `blendable` adds the alpha-blended variant.
pub fn blit_keys(
    format: TextureFormat,
    blendable: bool,
    spaces: impl IntoIterator<Item = Option<CompositingSpace>>,
) -> Vec<BlitPipelineKey> {
    let float_colour = !format.is_depth_stencil_format()
        && matches!(format.sample_type(None, None), Some(TextureSampleType::Float { .. }));
    if !float_colour {
        return Vec::new();
    }
    let blends: &[Option<BlendState>] =
        if blendable { &[None, Some(BlendState::ALPHA_BLENDING)] } else { &[None] };
    spaces
        .into_iter()
        .flat_map(|source_space| {
            blends.iter().map(move |&blend_state| BlitPipelineKey { target_format: format, blend_state, samples: 1, source_space })
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn prewarm(
    mut queued: ResMut<Queued>,
    images: Res<RenderAssets<GpuImage>>,
    windows: Res<ExtractedWindows>,
    cameras: Query<&ExtractedCamera>,
    adapter: Res<RenderAdapter>,
    cache: Res<PipelineCache>,
    blit: Option<Res<BlitPipeline>>,
    mut pipelines: ResMut<SpecializedRenderPipelines<BlitPipeline>>,
) {
    let Some(blit) = blit else {
        return;
    };
    if queued.spaces.is_empty() {
        queued.spaces.insert(None);
    }
    let new_spaces: Vec<_> =
        cameras.iter().map(|c| c.compositing_space).filter(|s| !queued.spaces.contains(s)).collect();
    let mut formats: HashSet<TextureFormat> =
        windows.values().filter_map(|w| w.swap_chain_texture_view_format).collect();
    formats.extend(
        images
            .iter()
            .filter(|(_, image)| image.texture_descriptor.usage.contains(TextureUsages::RENDER_ATTACHMENT))
            .map(|(_, image)| image.view_format()),
    );
    // A new space applies to every known format; a new format to every space.
    let all_formats: Vec<_> = if new_spaces.is_empty() {
        formats.into_iter().filter(|f| !queued.formats.contains(f)).collect()
    } else {
        queued.formats.iter().copied().chain(formats).collect::<HashSet<_>>().into_iter().collect()
    };
    if all_formats.is_empty() {
        return;
    }
    queued.spaces.extend(new_spaces);
    let spaces: Vec<_> = queued.spaces.iter().copied().collect();
    for format in all_formats {
        let blendable = adapter.get_texture_format_features(format).flags.contains(TextureFormatFeatureFlags::BLENDABLE);
        let keys = blit_keys(format, blendable, spaces.iter().copied());
        for &key in &keys {
            pipelines.specialize(&cache, &blit, key);
        }
        if queued.formats.insert(format) {
            println!("blit_prewarm format={format:?} keys={}", keys.len());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colour_targets_get_replace_and_blend_keys_and_depth_or_integer_targets_none() {
        let keys = blit_keys(TextureFormat::Rgba16Float, true, [None]);
        assert_eq!(keys.len(), 2);
        assert!(keys.iter().all(|k| k.target_format == TextureFormat::Rgba16Float && k.samples == 1));
        assert!(keys.iter().any(|k| k.blend_state.is_none()));
        assert!(keys.iter().any(|k| k.blend_state == Some(BlendState::ALPHA_BLENDING)));
        assert_eq!(blit_keys(TextureFormat::Rgba32Float, false, [None]).len(), 1);
        assert_eq!(blit_keys(TextureFormat::Bgra8UnormSrgb, true, [None, Some(CompositingSpace::Srgb)]).len(), 4);
        for format in [TextureFormat::Depth32Float, TextureFormat::Depth24PlusStencil8, TextureFormat::R32Uint, TextureFormat::Rgba8Sint] {
            assert!(blit_keys(format, true, [None]).is_empty(), "{format:?}");
        }
    }
}
