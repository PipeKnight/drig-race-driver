//! The original ColourBalance post effect (CheapFinalMerge with its
//! vignette), drawn only while a camera carries [`BalancePass`]: the
//! Flashback transition's IG_whiteout and FlashbackFX_01 (docs/FLASHBACK.md
//! §2.5). Without the component nothing is drawn and the image is unchanged.
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
use grid_formats::graphics_grade::Balance;

const SHADER: Handle<Shader> = uuid_handle!("3f0b8f6e-58a4-4c55-9d0e-2b7c1e9a4d61");
/// The view formats Bevy renders into: HDR and the ordinary sRGB target.
const FORMATS: [TextureFormat; 2] = [TextureFormat::Rgba16Float, TextureFormat::Rgba8UnormSrgb];

#[derive(Component, Clone, Copy, Debug, PartialEq, ExtractComponent, ShaderType)]
pub struct BalancePass {
    balance: Vec4,
    tint: Vec4,
    vignette: Vec4,
    frame: Vec4,
    rotation: Vec4,
}

impl From<&Balance> for BalancePass {
    fn from(b: &Balance) -> Self {
        let tint = Vec3::ONE.lerp(Vec3::from_array(b.tint_rgb), b.tint_amount);
        let angle = b.vignette_rotation.to_radians();
        Self {
            balance: Vec4::new(b.contrast, b.brightness, b.maximum, b.colour),
            tint: tint.extend(b.vignette_amount),
            vignette: Vec3::from_array(b.vignette_rgb).extend(b.vignette_shape),
            frame: Vec4::new(
                b.vignette_scale[0],
                b.vignette_scale[1],
                b.vignette_offset[0],
                b.vignette_offset[1],
            ),
            rotation: Vec4::new(angle.cos(), angle.sin(), b.vignette_transition, 0.),
        }
    }
}

/// Adds the pass where a renderer exists; without one the component only
/// records the state.
pub fn install(app: &mut App) {
    if app.get_sub_app(RenderApp).is_none() {
        return;
    }
    let Some(mut shaders) = app.world_mut().get_resource_mut::<Assets<Shader>>() else {
        return;
    };
    shaders
        .insert(
            SHADER.id(),
            Shader::from_wgsl(include_str!("balance_pass.wgsl"), "grid_balance_pass.wgsl"),
        )
        .expect("UUID shader asset");
    app.add_plugins((
        ExtractComponentPlugin::<BalancePass>::default(),
        UniformComponentPlugin::<BalancePass>::default(),
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
    ids: [CachedRenderPipelineId; FORMATS.len()],
}

fn initialize(
    mut commands: Commands,
    device: Res<RenderDevice>,
    fullscreen: Res<FullscreenShader>,
    cache: Res<PipelineCache>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "balance pass",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: true }),
                sampler(SamplerBindingType::Filtering),
                uniform_buffer::<BalancePass>(true),
            ),
        ),
    );
    let ids = FORMATS.map(|format| {
        cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some("colour balance".into()),
            layout: vec![layout.clone()],
            vertex: fullscreen.to_vertex_state(),
            fragment: Some(FragmentState {
                shader: SHADER,
                targets: vec![Some(ColorTargetState {
                    format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            ..default()
        })
    });
    commands.insert_resource(Pipeline {
        layout,
        sampler: device.create_sampler(&SamplerDescriptor::default()),
        ids,
    });
}

fn draw(
    view: ViewQuery<(&ViewTarget, &BalancePass, &DynamicUniformIndex<BalancePass>)>,
    pipeline: Option<Res<Pipeline>>,
    cache: Res<PipelineCache>,
    uniforms: Res<ComponentUniforms<BalancePass>>,
    mut ctx: RenderContext,
    mut reported: Local<Option<TextureFormat>>,
) {
    let Some(pipeline) = pipeline else {
        return;
    };
    let (target, _, index) = view.into_inner();
    let format = target.main_texture_format();
    let Some(slot) = FORMATS.iter().position(|f| *f == format) else {
        if *reported != Some(format) {
            warn!("balance pass skipped: unsupported target {format:?}");
            *reported = Some(format);
        }
        return;
    };
    let Some(render_pipeline) = cache.get_render_pipeline(pipeline.ids[slot]) else {
        return;
    };
    let Some(binding) = uniforms.uniforms().binding() else {
        return;
    };
    if *reported != Some(format) {
        println!("graphics_balance_pass active=true format={format:?}");
        *reported = Some(format);
    }
    let write = target.post_process_write();
    let group = ctx.render_device().create_bind_group(
        "balance pass",
        &cache.get_bind_group_layout(&pipeline.layout),
        &BindGroupEntries::sequential((write.source, &pipeline.sampler, binding)),
    );
    let mut pass = ctx
        .command_encoder()
        .begin_render_pass(&RenderPassDescriptor {
            label: Some("colour balance"),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The shader's arithmetic on the CPU, for one display-space colour.
    fn apply(p: &BalancePass, rgb: Vec3, uv: Vec2) -> Vec3 {
        let pos = Vec2::new(2. * uv.x - 1., 1. - 2. * uv.y);
        let (c, s) = (p.rotation.x, p.rotation.y);
        let r = Vec2::new(c * pos.x - s * pos.y, c * pos.y + s * pos.x);
        let q = 0.89
            * Vec2::new(p.frame.x, p.frame.y)
            * (p.vignette.w * r + Vec2::new(-p.frame.z, p.frame.w));
        let tau = p.rotation.z;
        let z = (q.dot(q).clamp(0., 1.) / tau + 1. - 1. / tau).clamp(0., 1.);
        let w = p.tint.w * z * z * (3. - 2. * z);
        let luma = rgb.dot(Vec3::new(0.2125, 0.7154, 0.0721));
        let t = Vec3::splat(luma).lerp(rgb, p.balance.w) * p.tint.truncate();
        let v = t.lerp(p.vignette.truncate(), w);
        (v * p.balance.x + p.balance.y).clamp(Vec3::ZERO, Vec3::splat(p.balance.z))
    }

    #[test]
    fn neutral_leaves_colours_and_whiteout_is_flat_grey() {
        let neutral = BalancePass::from(&Balance::NEUTRAL);
        for rgb in [Vec3::ZERO, Vec3::new(0.2, 0.5, 0.9), Vec3::ONE] {
            for uv in [Vec2::ZERO, Vec2::splat(0.5), Vec2::ONE] {
                assert!(apply(&neutral, rgb, uv).abs_diff_eq(rgb, 1e-6));
            }
        }
        // IG_whiteout: contrast 0.5, brightness 0.75, maximum 0.75.
        let white = Balance {
            contrast: 0.5,
            brightness: 0.75,
            maximum: 0.75,
            ..Balance::NEUTRAL
        };
        let white = BalancePass::from(&white);
        for rgb in [Vec3::ZERO, Vec3::new(0.2, 0.5, 0.9), Vec3::ONE] {
            assert!(apply(&white, rgb, Vec2::splat(0.3)).abs_diff_eq(Vec3::splat(0.75), 1e-6));
        }
    }

    #[test]
    fn vignette_darkens_the_corners_and_leaves_its_centre() {
        let fx = Balance {
            vignette_amount: 0.6623,
            vignette_shape: 0.8571,
            vignette_scale: [0.961, 1.0649],
            vignette_offset: [0.0779, 0.2338],
            ..Balance::NEUTRAL
        };
        let fx = BalancePass::from(&fx);
        let grey = Vec3::splat(0.5);
        // q = 0 where shape * pos = (offset x, -offset y).
        let pos = Vec2::new(0.0779, -0.2338) / 0.8571;
        let centre = Vec2::new((pos.x + 1.) / 2., (1. - pos.y) / 2.);
        assert!(apply(&fx, grey, centre).abs_diff_eq(grey, 1e-5));
        let corner = apply(&fx, grey, Vec2::ZERO);
        assert!(corner.x < 0.5 * (1. - 0.6623) + 1e-3, "{corner}");
    }
}
