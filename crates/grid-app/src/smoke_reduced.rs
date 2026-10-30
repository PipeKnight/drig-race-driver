//! Reduced-resolution tyre smoke buffer (F645), after the original's
//! pfx_half_smoke path (docs/TYRE_SMOKE_RESEARCH.md §7, docs/SMOKE_RENDER_COST.md).
//! Where the automatic boundary (smoke_boundary/live.rs) replays the five
//! smoke groups, they draw at half width and height into a colour and
//! transmittance target, tested against and faded in front of the nearest
//! scene depth under each texel (the original's soft fade, tyre_smoke.wgsl),
//! and one full-size pass composes `smoke + transmittance * scene` with a
//! depth-weighted upsample at edges. Pipelines still compiling, a source
//! pipeline this does not cover or a scene depth that cannot be sampled keep
//! the full-size replay for that frame.
use crate::smoke_boundary::attachments::Capability;
use crate::tyre_smoke::{SmokeDepth, SmokeMaterial};
use bevy::{
    asset::uuid_handle,
    core_pipeline::FullscreenShader,
    prelude::*,
    render::{
        RenderApp, RenderStartup,
        extract_resource::ExtractResourcePlugin,
        render_phase::TrackedRenderPass,
        render_resource::{
            binding_types::{texture_2d, texture_depth_2d, texture_depth_2d_multisampled},
            *,
        },
        renderer::{RenderContext, RenderDevice},
        texture::{CachedTexture, GpuImage, TextureCache},
        view::{ViewDepthTexture, ViewTarget},
    },
};
use std::collections::HashMap;

const SHADER: Handle<Shader> = uuid_handle!("8b4e2d71-3f5a-4c09-a6e8-1d7c9b2f5e40");
/// Smoke colour weighted by coverage (RGB) and transmittance (A).
const FORMAT: TextureFormat = TextureFormat::Rgba16Float;
/// Reduction per axis. The original's layer is named half size; its creator
/// divides the configured 1280x720 by four per axis (§7.2).
const SCALE: u32 = 2;

/// pfx_half_smoke's colour ONE/ONE_MINUS_SRC_ALPHA and alpha
/// ZERO/ONE_MINUS_SRC_ALPHA, for a fragment whose colour is not yet
/// multiplied by its alpha.
const ACCUMULATE: BlendState = BlendState {
    color: BlendComponent {
        src_factor: BlendFactor::SrcAlpha,
        dst_factor: BlendFactor::OneMinusSrcAlpha,
        operation: BlendOperation::Add,
    },
    alpha: BlendComponent {
        src_factor: BlendFactor::Zero,
        dst_factor: BlendFactor::OneMinusSrcAlpha,
        operation: BlendOperation::Add,
    },
};
/// pfx_compose_no_alpha: scene × transmittance + smoke, scene alpha kept.
const COMPOSE: BlendState = BlendState {
    color: BlendComponent {
        src_factor: BlendFactor::One,
        dst_factor: BlendFactor::SrcAlpha,
        operation: BlendOperation::Add,
    },
    alpha: BlendComponent {
        src_factor: BlendFactor::Zero,
        dst_factor: BlendFactor::One,
        operation: BlendOperation::Add,
    },
};

/// The reduction and composition pipelines and the reduced variant of each
/// source smoke pipeline, queued once and kept.
#[derive(Resource)]
pub(crate) struct Pipelines {
    vertex: VertexState,
    /// Indexed by a multisampled scene depth.
    reduce_layouts: [BindGroupLayoutDescriptor; 2],
    compose_layouts: [BindGroupLayoutDescriptor; 2],
    reduce: HashMap<bool, CachedRenderPipelineId>,
    compose: HashMap<(TextureFormat, u32), CachedRenderPipelineId>,
    smoke: HashMap<CachedRenderPipelineId, Option<CachedRenderPipelineId>>,
    refused: bool,
}

/// This frame's reduced targets, bind groups and pipelines.
pub(crate) struct Frame {
    reduce: CachedRenderPipelineId,
    compose: CachedRenderPipelineId,
    smoke: Vec<(CachedRenderPipelineId, CachedRenderPipelineId)>,
    colour: CachedTexture,
    depth: TextureView,
    reduce_group: BindGroup,
    compose_group: BindGroup,
    viewport: Vec2,
}

/// Makes the viewer's scene depth sampleable, sizes the reduced depth image
/// and queues the shared pipelines.
pub(crate) fn install(app: &mut App) -> std::io::Result<()> {
    if !app.world().contains_resource::<SmokeDepth>() {
        // No tyre smoke loaded: the full-size replay has nothing to draw either.
        println!("smoke_reduced enabled=false reason=no-tyre-smoke");
        return Ok(());
    }
    app.world_mut()
        .resource_mut::<Assets<Shader>>()
        .insert(
            SHADER.id(),
            Shader::from_wgsl(include_str!("smoke_reduced.wgsl"), "smoke_reduced.wgsl"),
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    app.add_systems(PostUpdate, (sampleable_depth, size_depth))
        .add_plugins(ExtractResourcePlugin::<SmokeDepth>::default());
    app.get_sub_app_mut(RenderApp)
        .ok_or_else(|| std::io::Error::other("reduced smoke needs renderer"))?
        .add_systems(RenderStartup, init);
    println!("smoke_reduced scale=1/{SCALE} format={FORMAT:?} depth=nearest fade=2.5/m compose=depth-weighted");
    Ok(())
}

fn sampleable_depth(mut cameras: Query<&mut Camera3d, With<crate::ViewerCamera>>) {
    for mut camera in &mut cameras {
        let usage = TextureUsages::from(camera.depth_texture_usages);
        if !usage.contains(TextureUsages::TEXTURE_BINDING) {
            camera.depth_texture_usages = (usage | TextureUsages::TEXTURE_BINDING).into();
        }
    }
}

/// Keeps the reduced depth image at the reduced size of the viewer's target.
fn size_depth(
    cameras: Query<&Camera, With<crate::ViewerCamera>>,
    depth: Res<SmokeDepth>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<SmokeMaterial>>,
    smoke: Query<&MeshMaterial3d<SmokeMaterial>>,
) {
    let Some(size) = cameras
        .iter()
        .filter(|c| c.is_active)
        .find_map(|c| c.physical_target_size())
    else {
        return;
    };
    let size = size.map(|v| v.div_ceil(SCALE));
    if images.get(&depth.0).is_some_and(|i| i.size() == size) {
        return;
    }
    if images.insert(&depth.0, SmokeDepth::image(size)).is_ok() {
        // The smoke materials bind the image: prepare them again.
        for material in &smoke {
            let _ = materials.get_mut(&material.0);
        }
    }
}

fn init(mut commands: Commands, fullscreen: Res<FullscreenShader>) {
    let layouts = |multisampled: bool| {
        let scene = if multisampled {
            texture_depth_2d_multisampled()
        } else {
            texture_depth_2d()
        };
        (
            BindGroupLayoutDescriptor::new(
                "tyre smoke depth reduction",
                &BindGroupLayoutEntries::single(ShaderStages::FRAGMENT, scene),
            ),
            BindGroupLayoutDescriptor::new(
                "tyre smoke compose",
                &BindGroupLayoutEntries::sequential(
                    ShaderStages::FRAGMENT,
                    (
                        scene,
                        texture_2d(TextureSampleType::Float { filterable: false }),
                        texture_depth_2d(),
                    ),
                ),
            ),
        )
    };
    let (single, multi) = (layouts(false), layouts(true));
    commands.insert_resource(Pipelines {
        vertex: fullscreen.to_vertex_state(),
        reduce_layouts: [single.0, multi.0],
        compose_layouts: [single.1, multi.1],
        reduce: HashMap::new(),
        compose: HashMap::new(),
        smoke: HashMap::new(),
        refused: false,
    });
}

impl Pipelines {
    fn fullscreen(
        &self,
        label: &'static str,
        entry: &'static str,
        layout: &BindGroupLayoutDescriptor,
        multisampled: bool,
    ) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some(label.into()),
            layout: vec![layout.clone()],
            vertex: self.vertex.clone(),
            fragment: Some(FragmentState {
                shader: SHADER,
                shader_defs: if multisampled {
                    vec!["MULTISAMPLED".into()]
                } else {
                    vec![]
                },
                entry_point: Some(entry.into()),
                targets: vec![],
            }),
            ..default()
        }
    }

    fn reduce(&mut self, cache: &PipelineCache, multisampled: bool) -> CachedRenderPipelineId {
        if let Some(id) = self.reduce.get(&multisampled) {
            return *id;
        }
        let layout = &self.reduce_layouts[multisampled as usize];
        let mut descriptor =
            self.fullscreen("tyre smoke depth reduction", "downsample", layout, multisampled);
        descriptor.depth_stencil = Some(DepthStencilState {
            format: TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(CompareFunction::Always),
            stencil: default(),
            bias: default(),
        });
        let id = cache.queue_render_pipeline(descriptor);
        self.reduce.insert(multisampled, id);
        id
    }

    fn compose(&mut self, cache: &PipelineCache, format: TextureFormat, samples: u32) -> CachedRenderPipelineId {
        if let Some(id) = self.compose.get(&(format, samples)) {
            return *id;
        }
        let multisampled = samples > 1;
        let layout = &self.compose_layouts[multisampled as usize];
        let mut descriptor = self.fullscreen("tyre smoke compose", "compose", layout, multisampled);
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.targets = vec![Some(ColorTargetState {
                format,
                blend: Some(COMPOSE),
                write_mask: ColorWrites::COLOR,
            })];
        }
        descriptor.multisample.count = samples;
        let id = cache.queue_render_pipeline(descriptor);
        self.compose.insert((format, samples), id);
        id
    }

    /// The reduced variant of a source smoke pipeline, queued once.
    fn smoke(&mut self, cache: &PipelineCache, source: CachedRenderPipelineId) -> Option<CachedRenderPipelineId> {
        *self.smoke.entry(source).or_insert_with(|| {
            reduced_smoke(cache.get_render_pipeline_descriptor(source))
                .map(|descriptor| cache.queue_render_pipeline(descriptor))
        })
    }
}

/// The source smoke pipeline drawn into the reduced target: one sample,
/// accumulating blend, the soft fade on; shader, layout and depth test kept.
/// None for a pipeline this does not cover (not straight alpha blending).
fn reduced_smoke(source: &RenderPipelineDescriptor) -> Option<RenderPipelineDescriptor> {
    let mut descriptor = source.clone();
    let fragment = descriptor.fragment.as_mut()?;
    let target = fragment.targets.first_mut()?.as_mut()?;
    if target.blend != Some(BlendState::ALPHA_BLENDING) {
        return None;
    }
    *target = ColorTargetState {
        format: FORMAT,
        blend: Some(ACCUMULATE),
        write_mask: ColorWrites::ALL,
    };
    fragment.shader_defs.push("TYRE_SMOKE_REDUCED".into());
    descriptor.multisample = MultisampleState::default();
    descriptor.label = Some(
        format!("{} reduced", descriptor.label.as_deref().unwrap_or("tyre smoke")).into(),
    );
    Some(descriptor)
}

/// Readies this frame's reduced targets for the owned smoke draws whose
/// pipelines are `sources`, or None to keep the full-size replay.
#[allow(clippy::too_many_arguments)]
pub(crate) fn prepare(
    pipelines: &mut Pipelines,
    cache: &PipelineCache,
    device: &RenderDevice,
    textures: &mut TextureCache,
    depth: &ViewDepthTexture,
    reduced_depth: Option<&GpuImage>,
    capability: Capability,
    sources: impl IntoIterator<Item = CachedRenderPipelineId>,
) -> Option<Frame> {
    if pipelines.refused || !depth.texture.usage().contains(TextureUsages::TEXTURE_BINDING) {
        return None;
    }
    let size = Extent3d {
        width: capability.size.x.div_ceil(SCALE),
        height: capability.size.y.div_ceil(SCALE),
        depth_or_array_layers: 1,
    };
    // The image follows a resize one frame later.
    let reduced_depth = reduced_depth.filter(|i| i.texture.size() == size)?.texture_view.clone();
    let multisampled = capability.samples > 1;
    let reduce = pipelines.reduce(cache, multisampled);
    let compose = pipelines.compose(cache, capability.format, capability.samples);
    let mut smoke = Vec::new();
    for source in sources {
        smoke.push((source, pipelines.smoke(cache, source)?));
    }
    for id in [reduce, compose].into_iter().chain(smoke.iter().map(|s| s.1)) {
        match cache.get_render_pipeline_state(id) {
            CachedPipelineState::Ok(_) => {}
            CachedPipelineState::Err(error) => {
                println!("smoke_reduced event=refused full_size_replay=true error={error}");
                pipelines.refused = true;
                return None;
            }
            _ => return None,
        }
    }
    let colour = textures.get(
        device,
        TextureDescriptor {
            label: Some("tyre smoke reduced colour"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: FORMAT,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        },
    );
    let reduce_group = device.create_bind_group(
        "tyre smoke depth reduction",
        &cache.get_bind_group_layout(&pipelines.reduce_layouts[multisampled as usize]),
        &BindGroupEntries::single(depth.view()),
    );
    let compose_group = device.create_bind_group(
        "tyre smoke compose",
        &cache.get_bind_group_layout(&pipelines.compose_layouts[multisampled as usize]),
        &BindGroupEntries::sequential((
            depth.view(),
            &colour.default_view,
            &reduced_depth,
        )),
    );
    Some(Frame {
        reduce,
        compose,
        smoke,
        colour,
        depth: reduced_depth,
        reduce_group,
        compose_group,
        viewport: capability.size.as_vec2() / SCALE as f32,
    })
}

impl Frame {
    /// The reduced variant of a source smoke pipeline.
    pub(crate) fn pipeline(&self, source: CachedRenderPipelineId) -> Option<CachedRenderPipelineId> {
        self.smoke.iter().find(|s| s.0 == source).map(|s| s.1)
    }

    /// Writes the nearest scene depth under each reduced texel.
    pub(crate) fn reduce_depth(&self, context: &mut RenderContext, cache: &PipelineCache) -> Result<(), &'static str> {
        let pipeline = cache
            .get_render_pipeline(self.reduce)
            .ok_or("reduced smoke depth pipeline absent")?;
        let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("tyre smoke depth reduction"),
            color_attachments: &[],
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: Some(Operations {
                    load: LoadOp::Clear(0.),
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, &self.reduce_group, &[]);
        pass.draw(0..3, 0..1);
        Ok(())
    }

    /// The reduced smoke pass: colour cleared to no smoke and full
    /// transmittance, the reduced depth tested read-only (the smoke shader
    /// samples it too), the scene's projection mapped onto exactly half the
    /// full extent.
    pub(crate) fn smoke_pass<'a>(&'a self, context: &'a mut RenderContext) -> TrackedRenderPass<'a> {
        let colours = [Some(RenderPassColorAttachment {
            view: &self.colour.default_view,
            depth_slice: None,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(LinearRgba::new(0., 0., 0., 1.).into()),
                store: StoreOp::Store,
            },
        })];
        let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("tyre smoke reduced"),
            color_attachments: &colours,
            depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                view: &self.depth,
                depth_ops: None,
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_viewport(0., 0., self.viewport.x, self.viewport.y, 0., 1.);
        pass
    }

    /// Composes the reduced smoke over the scene.
    pub(crate) fn compose(
        &self,
        context: &mut RenderContext,
        cache: &PipelineCache,
        target: &ViewTarget,
    ) -> Result<(), &'static str> {
        let pipeline = cache
            .get_render_pipeline(self.compose)
            .ok_or("reduced smoke compose pipeline absent")?;
        let colours = [Some(target.get_color_attachment())];
        let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("tyre smoke compose"),
            color_attachments: &colours,
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, &self.compose_group, &[]);
        pass.draw(0..3, 0..1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(blend: Option<BlendState>) -> RenderPipelineDescriptor {
        RenderPipelineDescriptor {
            label: Some("smoke".into()),
            fragment: Some(FragmentState {
                shader: SHADER,
                shader_defs: vec!["MULTISAMPLED".into()],
                entry_point: Some("fragment".into()),
                targets: vec![Some(ColorTargetState {
                    format: TextureFormat::Rgba8UnormSrgb,
                    blend,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: Some(false),
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: default(),
                bias: default(),
            }),
            multisample: MultisampleState {
                count: 4,
                ..default()
            },
            ..default()
        }
    }

    #[test]
    fn reduced_smoke_pipeline_is_single_sample_accumulating_and_faded() {
        let reduced = reduced_smoke(&source(Some(BlendState::ALPHA_BLENDING))).unwrap();
        assert_eq!(reduced.multisample.count, 1);
        let fragment = reduced.fragment.unwrap();
        let target = fragment.targets[0].clone().unwrap();
        assert_eq!(target.format, FORMAT);
        assert_eq!(target.blend, Some(ACCUMULATE));
        assert!(fragment.shader_defs.contains(&"TYRE_SMOKE_REDUCED".into()));
        assert!(fragment.shader_defs.contains(&"MULTISAMPLED".into()));
        // The depth test stays; nothing is written.
        assert_eq!(reduced.depth_stencil, source(None).depth_stencil);
    }

    #[test]
    fn other_blending_keeps_the_full_size_replay() {
        assert!(reduced_smoke(&source(None)).is_none());
        assert!(reduced_smoke(&source(Some(BlendState::PREMULTIPLIED_ALPHA_BLENDING))).is_none());
    }

    #[test]
    fn accumulation_and_compose_give_smoke_over_transmitted_scene() {
        // Two layers of straight-alpha smoke over a cleared (0, 1) target,
        // composed over the scene, equal ordinary back-to-front blending.
        let blend = |c: BlendComponent, src: f32, src_a: f32, dst: f32| {
            let factor = |f: BlendFactor| match f {
                BlendFactor::One => 1.,
                BlendFactor::Zero => 0.,
                BlendFactor::SrcAlpha => src_a,
                BlendFactor::OneMinusSrcAlpha => 1. - src_a,
                _ => unreachable!(),
            };
            src * factor(c.src_factor) + dst * factor(c.dst_factor)
        };
        let layers = [(0.8, 0.5), (0.3, 0.25)];
        let (mut c, mut t) = (0f32, 1f32);
        for (colour, alpha) in layers {
            c = blend(ACCUMULATE.color, colour, alpha, c);
            t = blend(ACCUMULATE.alpha, alpha, alpha, t);
        }
        let scene = 0.6;
        let composed = blend(COMPOSE.color, c, t, scene);
        let mut direct = scene;
        for (colour, alpha) in layers {
            direct = colour * alpha + direct * (1. - alpha);
        }
        assert!((composed - direct).abs() < 1e-6, "{composed} {direct}");
    }
}
