//! Bounded synthetic4x sample-coverage core; not a scene material or source-level2 port.
//! No scene material, asset file, shader parity, source-level2 or defaults claim.
use bevy::{
    prelude::*,
    render::{
        render_resource::*,
        renderer::{RenderAdapter, RenderContext, RenderDevice},
        storage::GpuShaderBuffer,
    },
};
use std::fmt::Write as _;
pub const WIDTH: u32 = 64;
pub const HEIGHT: u32 = 4;
pub const SAMPLES: u32 = 4;
pub const WORDS: usize = (WIDTH * HEIGHT * SAMPLES * 4) as usize;
pub const BYTES: usize = WORDS * 4;
pub const SENTINEL: u32 = 0xfeed_face;
pub struct Probe {
    _colour: Texture,
    _depth: Texture,
    colour: TextureView,
    depth: TextureView,
    draw: [CachedRenderPipelineId; 5],
    read: [CachedComputePipelineId; 2],
    layout: BindGroupLayoutDescriptor,
}
fn replace_zero_alpha() -> BlendState {
    BlendState {
        color: BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::Zero,
            operation: BlendOperation::Add,
        },
        alpha: BlendComponent {
            src_factor: BlendFactor::Zero,
            dst_factor: BlendFactor::Zero,
            operation: BlendOperation::Add,
        },
    }
}
/// Future RenderStartup helper; shader is installed from include_str, no AssetServer path.
pub fn prepare(
    device: &RenderDevice,
    adapter: &RenderAdapter,
    cache: &PipelineCache,
    shader: Handle<Shader>,
) -> Probe {
    for format in [TextureFormat::Rgba8Unorm, TextureFormat::Depth32Float] {
        let capabilities = adapter.get_texture_format_features(format);
        assert!(
            capabilities
                .flags
                .contains(TextureFormatFeatureFlags::MULTISAMPLE_X4)
                && capabilities
                    .allowed_usages
                    .contains(TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING),
            "coverage probe requires actual render+sample4 format capabilities"
        );
    }
    let descriptor = |format| TextureDescriptor {
        label: Some("bounded coverage probe64x4"),
        size: Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: SAMPLES,
        dimension: TextureDimension::D2,
        format,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    };
    let colour = device.create_texture(&descriptor(TextureFormat::Rgba8Unorm));
    let depth = device.create_texture(&descriptor(TextureFormat::Depth32Float));
    let colour_view = colour.create_view(&TextureViewDescriptor::default());
    let depth_view = depth.create_view(&TextureViewDescriptor::default());
    let draw = std::array::from_fn(|policy| {
        cache.queue_render_pipeline(RenderPipelineDescriptor {
            label: Some(format!("coverage probe policy{policy}").into()),
            vertex: VertexState {
                shader: shader.clone(),
                entry_point: Some(if policy == 4 { "behind" } else { "front" }.into()),
                ..default()
            },
            fragment: Some(FragmentState {
                shader: shader.clone(),
                entry_point: Some(["opaque", "mask", "sharpened", "raw", "red"][policy].into()),
                targets: vec![Some(ColorTargetState {
                    format: TextureFormat::Rgba8Unorm,
                    blend: Some(replace_zero_alpha()),
                    write_mask: ColorWrites::ALL,
                })],
                ..default()
            }),
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(CompareFunction::GreaterEqual),
                stencil: default(),
                bias: default(),
            }),
            multisample: MultisampleState {
                count: SAMPLES,
                alpha_to_coverage_enabled: policy == 2 || policy == 3,
                ..default()
            },
            ..default()
        })
    });
    let layout = BindGroupLayoutDescriptor::new(
        "coverage per-sample snapshot",
        &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: false },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: true,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Texture {
                    sample_type: TextureSampleType::Depth,
                    view_dimension: TextureViewDimension::D2,
                    multisampled: true,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
        ],
    );
    let read = std::array::from_fn(|stage| {
        cache.queue_compute_pipeline(ComputePipelineDescriptor {
            label: Some(format!("coverage sample stage{stage}").into()),
            shader: shader.clone(),
            entry_point: Some(
                if stage == 0 {
                    "snapshot_before"
                } else {
                    "snapshot_after"
                }
                .into(),
            ),
            layout: vec![layout.clone()],
            ..default()
        })
    });
    Probe {
        _colour: colour,
        _depth: depth,
        colour: colour_view,
        depth: depth_view,
        draw,
        read,
        layout,
    }
}
/// One-shot caller MUST wait for all cached pipelines, then enqueue readback only after true.
/// Writes no main scene/view attachment. No multisample resolve is used.
pub fn encode(
    context: &mut RenderContext,
    device: &RenderDevice,
    cache: &PipelineCache,
    p: &Probe,
    output: &GpuShaderBuffer,
) -> bool {
    if p.draw
        .iter()
        .any(|id| cache.get_render_pipeline(*id).is_none())
        || p.read
            .iter()
            .any(|id| cache.get_compute_pipeline(*id).is_none())
    {
        return false;
    }
    assert_eq!(
        output.buffer.size(),
        BYTES as u64,
        "coverage output allocation exact bound"
    );
    let bindings = device.create_bind_group(
        None,
        &cache.get_bind_group_layout(&p.layout),
        &BindGroupEntries::sequential((
            &p.colour,
            &p.depth,
            output.buffer.as_entire_buffer_binding(),
        )),
    );
    for stage in 0..2 {
        {
            let attachment = [Some(RenderPassColorAttachment {
                view: &p.colour,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: if stage == 0 {
                        LoadOp::Clear(LinearRgba::new(0., 0., 0., 0.).into())
                    } else {
                        LoadOp::Load
                    },
                    store: StoreOp::Store,
                },
            })];
            let depth = RenderPassDepthStencilAttachment {
                view: &p.depth,
                depth_ops: Some(Operations {
                    load: if stage == 0 {
                        LoadOp::Clear(0.)
                    } else {
                        LoadOp::Load
                    },
                    store: StoreOp::Store,
                }),
                stencil_ops: None,
            };
            let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("coverage before/behind"),
                color_attachments: &attachment,
                depth_stencil_attachment: Some(depth),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if stage == 0 {
                for row in 0..4 {
                    pass.set_render_pipeline(
                        cache.get_render_pipeline(p.draw[row as usize]).unwrap(),
                    );
                    pass.set_scissor_rect(0, row, WIDTH, 1);
                    pass.draw(0..3, 0..1);
                }
            } else {
                pass.set_render_pipeline(cache.get_render_pipeline(p.draw[4]).unwrap());
                pass.draw(0..3, 0..1);
            }
        }
        {
            let mut pass = context
                .command_encoder()
                .begin_compute_pass(&ComputePassDescriptor {
                    label: Some("coverage per-sample colour/depth, no resolve"),
                    ..default()
                });
            pass.set_pipeline(cache.get_compute_pipeline(p.read[stage]).unwrap());
            pass.set_bind_group(0, &bindings, &[]);
            pass.dispatch_workgroups(WIDTH / 8, HEIGHT, 1);
        }
    }
    true
}
fn alpha(x: u32) -> f32 {
    match x {
        0..=7 => 0.,
        8..=15 => 1. / 255.,
        16..=23 => 0.125,
        24..=31 => 0.25,
        32..=39 => 0.5,
        40..=47 => 0.75,
        48..=55 => 1.,
        56..=59 => (x - 56) as f32 / 3.,
        _ => (x % 2) as f32,
    }
}
/// Successful JSON≤64KiB; validates SAMPLE colour/depth ownership, never stored alpha opacity.
pub fn verify_json(bytes: &[u8]) -> Result<String, &'static str> {
    if bytes.len() != BYTES {
        return Err("readback length");
    }
    let words: Vec<_> = bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect();
    let mut text = String::from(
        "{\"scope\":\"authored4x-not-source-level2-parity\",\"width\":64,\"height\":4,\"samples\":4,\"policy_rows\":[\"opaque\",\"mask05\",\"bevy_sharpened\",\"raw_gt0\"],\"records_beforeRGBA_depth_afterRGBA_depth\":[",
    );
    const WHITE: u32 = 0x00ff_ffff;
    const RED: u32 = 0x0000_00ff;
    let mut masks = Vec::with_capacity((WIDTH * HEIGHT) as usize);
    for pixel in 0..(WIDTH * HEIGHT) as usize {
        let policy = pixel / WIDTH as usize;
        let x = (pixel % WIDTH as usize) as u32;
        let a = alpha(x);
        let mut mask = 0u8;
        for sample in 0..4 {
            let i = (pixel * 4 + sample) * 4;
            let r = &words[i..i + 4];
            if r.contains(&SENTINEL) {
                return Err("unwritten stage/sample");
            }
            let covered = match (r[0], r[1]) {
                (WHITE, 0x3f00_0000) => true,
                (0, 0) => false,
                _ => return Err("pre-sample not binary coverage/depth"),
            };
            if policy == 0 && !covered {
                return Err("opaque expected every sample");
            }
            if policy == 1 && covered != (a >= 0.5) {
                return Err("Mask threshold/equality");
            }
            if policy >= 2 && ((a == 0. && covered) || (a == 1. && !covered)) {
                return Err("A2C alpha0/1 extremes");
            }
            if (r[2], r[3])
                != if covered {
                    (WHITE, 0x3f00_0000)
                } else {
                    (RED, 0x3e80_0000)
                }
            {
                return Err("behind colour/depth ownership");
            }
            if covered {
                mask |= 1 << sample;
            }
            if pixel != 0 || sample != 0 {
                text.push(',');
            }
            let _ = write!(text, "[{},{},{},{}]", r[0], r[1], r[2], r[3]);
        }
        masks.push(mask);
    }
    text.push_str("],\"measured_masks\":[");
    for (i, mask) in masks.iter().enumerate() {
        if i != 0 {
            text.push(',');
        }
        let _ = write!(text, "{mask}");
    }
    let differences = (0..WIDTH as usize)
        .filter(|x| masks[2 * WIDTH as usize + x] != masks[3 * WIDTH as usize + x])
        .count();
    let _ = write!(
        text,
        "],\"raw_vs_sharpened_different_masks\":{differences},\"status\":\"sample-invariants-pass-intermediate-counts-measured\"}}"
    );
    if text.len() > 65536 {
        return Err("JSON evidence bound");
    }
    Ok(text)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn authored_samples() -> Vec<u8> {
        let mut values = Vec::with_capacity(WORDS);
        for pixel in 0..(WIDTH * HEIGHT) as usize {
            let row = pixel / WIDTH as usize;
            let a = alpha((pixel % WIDTH as usize) as u32);
            for sample in 0..4 {
                // Authored arbitrary intermediate2/4 mask, NOT predicted GPU A2C behavior.
                let on = match row {
                    0 => true,
                    1 => a >= 0.5,
                    _ => a == 1. || (a > 0. && sample < 2),
                };
                values.extend_from_slice(if on {
                    &[0x00ff_ffff, 0x3f00_0000, 0x00ff_ffff, 0x3f00_0000]
                } else {
                    &[0, 0, 0xff, 0x3e80_0000]
                });
            }
        }
        values.into_iter().flat_map(u32::to_le_bytes).collect()
    }
    #[test]
    fn sample_decoder_checks_extremes_threshold_depth_and_size() {
        let good = authored_samples();
        assert_eq!(good.len(), BYTES);
        assert!(verify_json(&good).unwrap().len() < 65536);
        assert!(verify_json(&good[..good.len() - 1]).is_err());
        for index in [0usize, 1, 2, 3] {
            let mut bad = good.clone();
            bad[index * 4..index * 4 + 4].copy_from_slice(&SENTINEL.to_le_bytes());
            assert!(verify_json(&bad).is_err());
        }
        for (pixel, on) in [
            ((WIDTH * 3) as usize, true),
            ((WIDTH + 1) as usize, true),
            ((WIDTH * 3 + 50) as usize, false),
            ((WIDTH * 2 + 50) as usize, false),
        ] {
            let mut bad = good.clone();
            let first = pixel * 4 * 4 * 4;
            let valid_covered = if on {
                [0x00ff_ffffu32, 0x3f00_0000, 0x00ff_ffff, 0x3f00_0000]
            } else {
                [0, 0, 0xff, 0x3e80_0000]
            };
            for (lane, word) in valid_covered.iter().enumerate() {
                bad[first + lane * 4..first + lane * 4 + 4].copy_from_slice(&word.to_le_bytes());
            }
            assert!(verify_json(&bad).is_err());
        }
        let mut bad = good;
        bad[12..16].copy_from_slice(&0u32.to_le_bytes());
        assert!(verify_json(&bad).is_err());
    }
}
