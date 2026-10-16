//! Optional track-inspired display grade; not the original complete EGO postprocess.
use bevy::{
    asset::uuid_handle,
    core_pipeline::{Core3dSystems, FullscreenShader, schedule::Core3d, tonemapping::tonemapping},
    prelude::*,
    render::{
        RenderApp, RenderStartup,
        extract_component::{
            ComponentUniforms, DynamicUniformIndex, ExtractComponent, ExtractComponentPlugin,
            UniformComponentPlugin,
        },
        render_resource::{
            binding_types::{sampler, texture_2d, uniform_buffer},
            *,
        },
        renderer::{RenderContext, RenderDevice, ViewQuery},
        view::ViewTarget,
    },
};

const SHADER: Handle<Shader> = uuid_handle!("7869d3a3-9cc4-40e0-938b-5f80a0886655");

#[derive(Component, Clone, Copy, ExtractComponent, ShaderType)]
pub struct TrackGrade {
    controls: Vec4,
    tint: Vec4,
}
impl From<grid_formats::graphics_grade::Grade> for TrackGrade {
    fn from(g: grid_formats::graphics_grade::Grade) -> Self {
        let tint = Vec3::ONE.lerp(Vec3::from_array(g.tint_rgb), g.tint_amount);
        Self {
            controls: Vec4::new(g.saturation, g.contrast, g.brightness, 1. / g.gamma),
            tint: tint.extend(g.maximum),
        }
    }
}

pub fn install(app: &mut App) {
    app.world_mut()
        .resource_mut::<Assets<Shader>>()
        .insert(
            SHADER.id(),
            Shader::from_wgsl(include_str!("track_grade.wgsl"), "grid_track_grade.wgsl"),
        )
        .expect("UUID shader asset");
    app.add_plugins((
        ExtractComponentPlugin::<TrackGrade>::default(),
        UniformComponentPlugin::<TrackGrade>::default(),
    ));
    if let Some(render) = app.get_sub_app_mut(RenderApp) {
        render.add_systems(RenderStartup, initialize).add_systems(
            Core3d,
            draw.in_set(Core3dSystems::PostProcess).after(tonemapping),
        );
    }
}

#[derive(Resource)]
struct Pipeline {
    layout: BindGroupLayoutDescriptor,
    sampler: Sampler,
    id: CachedRenderPipelineId,
}
fn initialize(
    mut commands: Commands,
    device: Res<RenderDevice>,
    fullscreen: Res<FullscreenShader>,
    cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "track grade",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<TrackGrade>(true),
            ),
        ),
    );
    let id = cache.queue_render_pipeline(RenderPipelineDescriptor {
        label: Some("track-inspired grade".into()),
        layout: vec![layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: SHADER,
            targets: vec![Some(ColorTargetState {
                format: TextureFormat::Rgba16Float,
                blend: None,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        ..default()
    });
    commands.insert_resource(Pipeline {
        layout,
        sampler: device.create_sampler(&SamplerDescriptor::default()),
        id,
    });
}

fn draw(
    view: ViewQuery<(&ViewTarget, &TrackGrade, &DynamicUniformIndex<TrackGrade>)>,
    pipeline: Option<Res<Pipeline>>,
    cache: Res<PipelineCache>,
    uniforms: Res<ComponentUniforms<TrackGrade>>,
    mut ctx: RenderContext,
    mut reported: Local<bool>,
) {
    let Some(pipeline) = pipeline else {
        return;
    };
    let Some(render_pipeline) = cache.get_render_pipeline(pipeline.id) else {
        return;
    };
    let Some(binding) = uniforms.uniforms().binding() else {
        return;
    };
    let (target, _, index) = view.into_inner();
    // This optional pass is installed only on the enhanced HDR camera.
    if target.main_texture_format() != TextureFormat::Rgba16Float {
        if !*reported {
            warn!(
                "track grade skipped: unsupported target {:?}",
                target.main_texture_format()
            );
            *reported = true;
        }
        return;
    }
    if !*reported {
        println!("graphics_grade_pass active=true format=Rgba16Float");
        *reported = true;
    }
    let write = target.post_process_write();
    let group = ctx.render_device().create_bind_group(
        "track grade",
        &cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((write.source, &pipeline.sampler, binding)),
    );
    let mut pass = ctx
        .command_encoder()
        .begin_render_pass(&RenderPassDescriptor {
            label: Some("track grade"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: write.destination,
                depth_slice: None,
                resolve_target: None,
                ops: Operations::default(),
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
    pass.set_pipeline(render_pipeline);
    pass.set_bind_group(0, &group, &[index.index()]);
    pass.draw(0..3, 0..1);
}
