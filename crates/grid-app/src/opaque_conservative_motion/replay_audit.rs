//! Default-off single-transaction diagnostic; no quality fix or normal-path rollout.
use bevy::{
    asset::uuid_handle,
    prelude::*,
    render::{
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        gpu_readback::{Readback, ReadbackComplete},
        render_asset::RenderAssets,
        render_resource::*,
        renderer::{RenderAdapter, RenderContext, RenderDevice, RenderQueue},
        storage::{GpuShaderBuffer, ShaderBuffer},
        texture::GpuImage,
    },
    shader::{ShaderDefVal, ShaderRef, Source},
};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
const STOCK: &str = "22874194824dd1826ee328cc0c53fcf2afca355109d073dfd3419d46e63ad548";
const AUX: &str = "3f906413c93b4ca345031825769b3349d90cfe1f3f4415dc3b4e88486df98516";
const VARIANTS: [Handle<Shader>; 2] = [
    uuid_handle!("4898ae12-4dd8-4cbd-a6b2-920dfa8efb05"),
    uuid_handle!("67e5279c-4d33-4761-8259-a2b4497906b9"),
];
const WIDTH: u32 = 512;
const HEIGHT: u32 = 384;
#[derive(Clone, Default)]
pub(crate) struct Signals {
    complete: Arc<AtomicBool>,
    failed: Arc<AtomicBool>,
}
impl Signals {
    pub(crate) fn ready(&self) -> bool {
        self.complete.load(Ordering::Acquire)
    }
    pub(crate) fn failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }
}
#[derive(Clone)]
struct Frozen {
    header: [u32; 12],
    text: String,
    tokens: String,
}
#[derive(Resource, Clone, ExtractResource)]
pub(crate) struct Audit {
    tick: u64,
    directory: std::path::PathBuf,
    image: Handle<Image>,
    buffer: Handle<ShaderBuffer>,
    sources: [Handle<Shader>; 2],
    ready: Arc<AtomicBool>,
    signals: Signals,
    submitted: Arc<AtomicBool>,
    frozen: Arc<Mutex<Option<Frozen>>>,
    queued: bool,
}
#[derive(Component)]
struct Once(bool);
#[derive(Default, Resource)]
pub(crate) struct GpuState {
    pipelines: Mutex<HashMap<CachedRenderPipelineId, (CachedRenderPipelineId, u32)>>,
    stamp: Mutex<Option<(Entity, u32)>>,
    written: Mutex<Option<(Entity, crate::temporal_audit::SourceTransaction)>>,
}
impl GpuState {
    pub(super) fn selection(
        &self,
        view: Entity,
        frame: u32,
        original: CachedRenderPipelineId,
    ) -> Option<CachedRenderPipelineId> {
        if *self.stamp.lock().ok()? != Some((view, frame)) {
            return None;
        }
        self.pipelines.lock().ok()?.get(&original).map(|p| p.0)
    }
    pub(super) fn is_selected(&self, view: Entity, frame: u32) -> bool {
        self.stamp.lock().is_ok_and(|s| *s == Some((view, frame)))
    }
}
fn digest(bytes: &[u8]) -> String {
    use grid_tools::private_parameters as p;
    p::hex(&p::sha256(bytes))
}
fn fail(a: &Audit, why: &str) {
    if !a.signals.failed.swap(true, Ordering::AcqRel) {
        eprintln!("opaque_replay_audit verified=false reason={why}");
    }
}
fn source_handle(app: &App, r: ShaderRef) -> std::io::Result<Handle<Shader>> {
    match r {
        ShaderRef::Handle(h) => Ok(h),
        ShaderRef::Path(p) => Ok(app.world().resource::<AssetServer>().load(p)),
        ShaderRef::Default => Err(std::io::Error::other("audit shader ABI default")),
    }
}
pub(crate) fn install(
    app: &mut App,
    tick: u64,
    directory: &Path,
) -> std::io::Result<Option<Signals>> {
    let requested = match std::env::var("GRID_OPAQUE_REPLAY_AUDIT") {
        Err(std::env::VarError::NotPresent) => return Ok(None),
        Ok(v) => v.parse::<u64>().ok(),
        _ => None,
    };
    if requested != Some(tick)
        || tick != 1652
        || !super::requested().map_err(std::io::Error::other)?
    {
        return Err(std::io::Error::other(
            "replay audit requires candidate On and temporal audit tick1652",
        ));
    }
    use bevy::pbr::{Material, MaterialExtension};
    let sources = [
        source_handle(
            app,
            <StandardMaterial as Material>::prepass_fragment_shader(),
        )?,
        source_handle(
            app,
            <crate::foundation_backing::RawPass as MaterialExtension>::prepass_fragment_shader(),
        )?,
    ];
    let directory = directory.join("replay-fragment");
    std::fs::create_dir(&directory)?;
    let mut image = Image::new_target_texture(1920, 1080, TextureFormat::Rgba32Float, None);
    image.texture_descriptor.usage = TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC;
    let image = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let mut buffer = ShaderBuffer::from(vec![UVec4::ZERO; (3 + WIDTH * HEIGHT) as usize]);
    buffer.buffer_description.usage |= BufferUsages::COPY_SRC;
    let buffer = app
        .world_mut()
        .resource_mut::<Assets<ShaderBuffer>>()
        .add(buffer);
    let signals = Signals::default();
    app.insert_resource(Audit {
        tick,
        directory,
        image,
        buffer,
        sources,
        ready: Arc::default(),
        signals: signals.clone(),
        submitted: Arc::default(),
        frozen: Arc::default(),
        queued: false,
    })
    .add_plugins(ExtractResourcePlugin::<Audit>::default())
    .add_systems(Update, (prepare_sources, queue_readback).chain());
    if let Some(render) = app.get_sub_app_mut(bevy::render::RenderApp) {
        render.init_resource::<GpuState>();
    }
    Ok(Some(signals))
}
const TAIL: &str = r#"
    var replay_instance = -1.0;
#ifdef VERTEX_OUTPUT_INSTANCE_INDEX
    if in.instance_index <= 16777216u { replay_instance = f32(in.instance_index); }
#endif
    let replay_current = view.unjittered_clip_from_world * in.world_position;
    let replay_previous = previous_view_uniforms.clip_from_world * in.previous_world_position;
    return ReplayAuditOutput(vec4<f32>(replay_current.w, replay_previous.w,
        f32(#{OPAQUE_REPLAY_AUDIT_TOKEN}), replay_instance), out.motion_vector);
"#;
fn instrument(source: &str, expected: &str) -> Result<String, &'static str> {
    if digest(source.as_bytes()) != expected {
        return Err("source-hash");
    }
    let output = "-> prepass_io::FragmentOutput";
    if source.matches(output).count() != 1 || source.matches("    return out;").count() != 1 {
        return Err("source-anchor");
    }
    // This NEW module owns no original import identity, including future qualified variants.
    let clean = source
        .lines()
        .filter(|l| !l.trim_start().starts_with("#define_import_path "))
        .collect::<Vec<_>>()
        .join("\n");
    let body =
        clean
            .replacen(output, "-> ReplayAuditOutput", 1)
            .replacen("    return out;", TAIL, 1);
    Ok(format!(
        "#import bevy_pbr::prepass_bindings::previous_view_uniforms\nstruct ReplayAuditOutput {{ @location(0) audit:vec4<f32>, @location(1) motion:vec2<f32>, }};\n{body}\n"
    ))
}
fn prepare_sources(a: Res<Audit>, mut shaders: ResMut<Assets<Shader>>) {
    if a.ready.load(Ordering::Acquire) || a.signals.failed() {
        return;
    }
    let mut texts = Vec::new();
    let mut originals = Vec::new();
    for (i, h) in a.sources.iter().enumerate() {
        let Some(s) = shaders.get(h) else {
            return;
        };
        let Source::Wgsl(src) = &s.source else {
            fail(&a, "non-WGSL-source");
            return;
        };
        match instrument(src, [STOCK, AUX][i]) {
            Ok(t) => {
                texts.push(t);
                originals.push(s.clone());
            }
            Err(e) => {
                fail(&a, e);
                return;
            }
        }
    }
    for (i, text) in texts.into_iter().enumerate() {
        let mut variant = Shader::from_wgsl(text, format!("opaque-replay-audit-{i}.wgsl"));
        // Preserve original asset-side imports/defs/dependencies as well as descriptor-side defs.
        variant.additional_imports = originals[i].additional_imports.clone();
        variant.shader_defs = originals[i].shader_defs.clone();
        variant.file_dependencies = originals[i].file_dependencies.clone();
        variant.validate_shader = bevy::shader::ValidateShader::Enabled;
        if shaders.insert(VARIANTS[i].id(), variant).is_err() {
            fail(&a, "shader-insert");
            return;
        }
    }
    a.ready.store(true, Ordering::Release);
}
pub(super) fn reset(world: &mut World) {
    if world.contains_resource::<GpuState>() {
        world.insert_resource(GpuState::default());
    }
}
pub(super) fn prewarm(
    a: &Audit,
    state: &GpuState,
    cache: &PipelineCache,
    originals: &HashSet<CachedRenderPipelineId>,
    normal: &super::Derived,
) -> Result<(), &'static str> {
    if !a.ready.load(Ordering::Acquire) {
        return Ok(());
    }
    let normal = normal.0.lock().map_err(|_| "normal-cache-poison")?;
    let mut map = state.pipelines.lock().map_err(|_| "audit-cache-poison")?;
    let mut ids = originals.iter().copied().collect::<Vec<_>>();
    ids.sort_by_key(|id| id.id());
    for original in ids {
        if map.contains_key(&original) {
            continue;
        }
        if map.len() >= super::MAX_PIPELINES {
            return Err("audit-cache-cap");
        }
        let Some(normal_id) = normal.get(&original) else {
            continue;
        };
        let mut d = cache.get_render_pipeline_descriptor(*normal_id).clone();
        let f = d.fragment.as_mut().ok_or("audit-fragment-absent")?;
        let family = a
            .sources
            .iter()
            .position(|h| *h == f.shader)
            .ok_or("audit-unreviewed-shader")?;
        if f.shader_defs.iter().any(|d| match d {
            ShaderDefVal::Bool(n, _) | ShaderDefVal::Int(n, _) | ShaderDefVal::UInt(n, _) => {
                n == "OPAQUE_REPLAY_AUDIT_TOKEN"
            }
        }) {
            return Err("audit-token-spoof");
        }
        let token = map.len() as u32 + 1;
        f.shader = VARIANTS[family].clone();
        f.shader_defs.push(ShaderDefVal::UInt(
            "OPAQUE_REPLAY_AUDIT_TOKEN".into(),
            token,
        ));
        f.targets[0] = Some(ColorTargetState {
            format: TextureFormat::Rgba32Float,
            blend: None,
            write_mask: ColorWrites::ALL,
        });
        f.targets.truncate(2); // Only two actual MRT slots; preserve ordinary descriptor elsewhere.
        d.label = Some("single-tick actual opaque replay fragment audit".into());
        let id = cache.queue_render_pipeline(d);
        map.insert(original, (id, token));
    }
    Ok(())
}
fn budget(attachments: u32, bytes: u32, width: u32, height: u32) -> Result<(), &'static str> {
    if attachments < 2 || bytes < 20 || width < 1920 || height < 1080 {
        Err("audit-MRT-limits")
    } else {
        Ok(())
    }
}
#[allow(clippy::too_many_arguments)]
pub(super) fn target(
    a: &Audit,
    state: &GpuState,
    cache: &PipelineCache,
    ids: &HashSet<CachedRenderPipelineId>,
    adapter: &RenderAdapter,
    device: &RenderDevice,
    images: &RenderAssets<GpuImage>,
    view: Entity,
    frame: u32,
    size: UVec2,
) -> Result<TextureView, &'static str> {
    if a.signals.failed() || !a.ready.load(Ordering::Acquire) || size != UVec2::new(1920, 1080) {
        return Err("audit-not-ready-or-size");
    }
    let info = adapter.get_info();
    if format!("{:?}", info.backend) != "Vulkan" {
        return Err("audit-requires-Vulkan");
    }
    let l = device.limits();
    budget(
        l.max_color_attachments,
        l.max_color_attachment_bytes_per_sample,
        l.max_texture_dimension_2d,
        l.max_texture_dimension_2d,
    )?;
    let features = adapter.get_texture_format_features(TextureFormat::Rgba32Float);
    let guaranteed = TextureFormat::Rgba32Float.guaranteed_format_features(device.features());
    let required = TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC;
    if !guaranteed.allowed_usages.contains(required)
        && !device
            .features()
            .contains(WgpuFeatures::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES)
    {
        return Err("audit-format-not-enabled-on-device");
    }
    // Effective byte-cost order is slot0RGBA32 then slot1RG16; no ignored target gaps.
    let cost = TextureFormat::Rgba32Float
        .target_pixel_byte_cost()
        .ok_or("audit-format-cost")?
        + TextureFormat::Rg16Float
            .target_pixel_byte_cost()
            .ok_or("audit-MV-cost")?;
    if cost > l.max_color_attachment_bytes_per_sample {
        return Err("audit-effective-byte-budget");
    }

    if !features
        .allowed_usages
        .contains(TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC)
    {
        return Err("audit-format-usage");
    }
    let map = state.pipelines.lock().map_err(|_| "audit-cache-poison")?;
    for original in ids {
        let id = map.get(original).ok_or("audit-pipeline-absent")?.0;
        if matches!(
            cache.get_render_pipeline_state(id),
            CachedPipelineState::Err(_)
        ) {
            return Err("audit-pipeline-error");
        }
        if cache.get_render_pipeline(id).is_none() {
            return Err("audit-pipeline-not-ready");
        }
    }
    let image = images.get(&a.image).ok_or("audit-image-not-ready")?;
    if image.texture.format() != TextureFormat::Rgba32Float
        || image.texture.sample_count() != 1
        || image.texture.size()
            != (Extent3d {
                width: 1920,
                height: 1080,
                depth_or_array_layers: 1,
            })
    {
        return Err("audit-actual-image-context");
    }

    *state.stamp.lock().map_err(|_| "audit-stamp-poison")? = Some((view, frame));
    Ok(image.texture_view.clone())
}
pub(super) fn written(
    state: &GpuState,
    view: Entity,
    render_frame: u32,
    source: crate::temporal_audit::SourceTransaction,
) -> Result<(), &'static str> {
    if *state.stamp.lock().map_err(|_| "audit-stamp-poison")? != Some((view, render_frame)) {
        return Err("audit-written-render-view-join");
    }
    *state.written.lock().map_err(|_| "audit-stamp-poison")? = Some((view, source));
    Ok(())
}
fn validate_capture_stamp(
    state: &GpuState,
    view: Entity,
    header: [u32; 12],
    tick: u64,
) -> Result<(), &'static str> {
    let source = crate::temporal_audit::SourceTransaction {
        source_frame: header[1],
        source_tick: header[2] as u64,
    };
    if header[2] as u64 != tick
        || *state.written.lock().map_err(|_| "audit-stamp-poison")? != Some((view, source))
        || header[4..10] != [1920, 1080, 704, 348, 512, 384]
    {
        return Err("audit-frame-view-ROI-join");
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)]
pub(crate) fn capture(
    a: &Audit,
    state: &GpuState,
    context: &mut RenderContext,
    queue: &RenderQueue,
    images: &RenderAssets<GpuImage>,
    buffers: &RenderAssets<GpuShaderBuffer>,
    view: Entity,
    header: [u32; 12],
    text: &str,
    cache: &PipelineCache,
) -> Result<(), &'static str> {
    if a.submitted.load(Ordering::Acquire) {
        return Err("audit-duplicate-capture");
    }
    validate_capture_stamp(state, view, header, a.tick)?;
    // Observational clock-domain metadata only: do not derive or shift either frame number.
    let render_stamp = *state.stamp.lock().map_err(|_| "audit-stamp-poison")?;
    let image = images.get(&a.image).ok_or("audit-image-absent")?;
    let buffer = buffers.get(&a.buffer).ok_or("audit-buffer-absent")?;
    // Same frozen raw header and matrices; stage4 distinguishes the extra float4 payload.
    let mut audit_header = header;
    audit_header[3] = 4;
    let bytes = audit_header
        .iter()
        .flat_map(|n| n.to_le_bytes())
        .collect::<Vec<_>>();
    queue.write_buffer(&buffer.buffer, 0, &bytes);
    context.command_encoder().copy_texture_to_buffer(
        TexelCopyTextureInfo {
            texture: &image.texture,
            mip_level: 0,
            origin: Origin3d {
                x: header[6],
                y: header[7],
                z: 0,
            },
            aspect: TextureAspect::All,
        },
        TexelCopyBufferInfo {
            buffer: &buffer.buffer,
            layout: TexelCopyBufferLayout {
                offset: 48,
                bytes_per_row: Some(WIDTH * 16),
                rows_per_image: Some(HEIGHT),
            },
        },
        Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
    );
    let map = state.pipelines.lock().map_err(|_| "audit-cache-poison")?;
    let mut tokens = map
        .iter()
        .map(|(original, (_, token))| {
            let d = cache.get_render_pipeline_descriptor(*original);
            format!(
                "token={token} original_id={} descriptor_sha256={} descriptor={d:?}\n",
                original.id(),
                digest(format!("{d:?}").as_bytes())
            )
        })
        .collect::<Vec<_>>();
    tokens.sort();
    *a.frozen.lock().map_err(|_| "audit-frozen-poison")? = Some(Frozen {
        header: audit_header,
        text: format!(
            "{text}audit_stage=4 source_raw_header={header:?} payload=current_clip_w,previous_clip_w,pipeline_token,optional_existing_instance_index\nstock_source_sha256={STOCK}\nraw_aux_source_sha256={AUX}\naudit_clock render_stamp={render_stamp:?} source_frame={} source_tick={}\n",
            header[1], header[2]
        ),
        tokens: tokens.concat(),
    });
    a.submitted.store(true, Ordering::Release);
    Ok(())
}
pub(crate) fn capture_failed(a: &Audit, why: &str) {
    fail(a, why);
}
fn queue_readback(mut commands: Commands, mut a: ResMut<Audit>) {
    if !a.submitted.load(Ordering::Acquire) || a.queued {
        return;
    }
    a.queued = true;
    commands
        .spawn((Readback::buffer(a.buffer.clone()), Once(false)))
        .observe(readback);
}
fn readback(
    event: On<ReadbackComplete>,
    a: Res<Audit>,
    mut commands: Commands,
    mut once: Query<&mut Once>,
) {
    let Ok(mut once) = once.get_mut(event.entity) else {
        return;
    };
    if std::mem::replace(&mut once.0, true) {
        return;
    }
    let result = (|| -> Result<(), String> {
        let frozen = a
            .frozen
            .lock()
            .map_err(|_| "frozen-poison")?
            .clone()
            .ok_or("frozen-absent")?;
        if event.data.len() != ((3 + WIDTH * HEIGHT) * 16) as usize {
            return Err("audit-buffer-size".into());
        }
        let words = event.data[..48]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect::<Vec<_>>();
        if words != frozen.header {
            return Err("audit-header-mismatch".into());
        }
        // Preserve all raw float bits, including actual nonfinite w. Never sanitize diagnostic evidence.
        std::fs::write(a.directory.join("fragment.bin"), &event.data).map_err(|e| e.to_string())?;
        std::fs::write(a.directory.join("metadata.txt"), frozen.text).map_err(|e| e.to_string())?;
        std::fs::write(a.directory.join("pipeline-tokens.txt"), frozen.tokens)
            .map_err(|e| e.to_string())?;
        println!(
            "opaque_replay_audit verified=true frame={} tick={} bytes={} stage=4 finite_policy=preserve_actual_bits",
            frozen.header[1],
            frozen.header[2],
            event.data.len()
        );
        Ok(())
    })();
    match result {
        Ok(()) => a.signals.complete.store(true, Ordering::Release),
        Err(e) => fail(&a, &e),
    }
    commands.entity(event.entity).despawn();
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_wrong_source_before_anchor_edit() {
        let src = "fn x()-> prepass_io::FragmentOutput {\n    return out;\n}";
        assert_eq!(instrument(src, "wrong"), Err("source-hash"));
    }
    #[test]
    fn duplicate_return_refuses_and_original_mv_body_survives() {
        let src = "#define_import_path original::owned\nfn x()-> prepass_io::FragmentOutput {\n out.motion_vector = KEEP_ORIGINAL_CALC;\n    return out;\n}";
        let got = instrument(src, &digest(src.as_bytes())).unwrap();
        assert!(!got.contains("define_import_path"));
        assert!(got.contains("out.motion_vector = KEEP_ORIGINAL_CALC;"));
        assert!(got.contains("out.motion_vector)"));
        let bad = src.replace("    return out;", "    return out;\n    return out;");
        assert_eq!(
            instrument(&bad, &digest(bad.as_bytes())),
            Err("source-anchor")
        );
    }
    #[test]
    fn mrt_budget_rejects_unsupported_and_accepts_exact_boundary() {
        assert!(budget(1, 32, 4096, 4096).is_err());
        assert!(budget(2, 19, 4096, 4096).is_err());
        assert!(budget(2, 20, 1919, 1080).is_err());
        assert!(budget(2, 20, 1920, 1080).is_ok());
    }
    #[test]
    fn audit_selection_stamp_never_spills_to_other_view_or_frame() {
        let state = GpuState::default();
        let a = Entity::from_bits(11);
        let b = Entity::from_bits(12);
        assert!(!state.is_selected(a, 793));
        *state.stamp.lock().unwrap() = Some((a, 793));
        assert!(state.is_selected(a, 793));
        assert!(!state.is_selected(b, 793));
        assert!(!state.is_selected(a, 794));
        let reset = GpuState::default();
        assert!(!reset.is_selected(a, 793));
    }
    #[test]
    fn source_transaction_survives_real_last_frame_advance_and_rejects_stale_capture() {
        let mut app = crate::temporal_audit::test_audit_clock_app();
        app.update(); // Actual FrameCountPlugin increments in Last after our PostUpdate source snapshot.
        let source = crate::temporal_audit::replay_request(app.world())
            .unwrap()
            .unwrap();
        let render_frame = app.world().resource::<bevy::diagnostic::FrameCount>().0;
        assert_ne!(source.source_frame, render_frame); // No guessed +1 mapping or relaxed source equality.
        let mut entities = World::new();
        let view = entities.spawn_empty().id();
        let other = entities.spawn_empty().id();
        let state = GpuState::default();
        *state.stamp.lock().unwrap() = Some((view, render_frame));
        assert!(state.is_selected(view, render_frame));
        assert!(!state.is_selected(other, render_frame));
        written(&state, view, render_frame, source).unwrap();
        fn header(source: crate::temporal_audit::SourceTransaction) -> [u32; 12] {
            [
                1,
                source.source_frame,
                source.source_tick as u32,
                0,
                1920,
                1080,
                704,
                348,
                512,
                384,
                0,
                0,
            ]
        }
        assert!(validate_capture_stamp(&state, view, header(source), 1652).is_ok());
        assert!(validate_capture_stamp(&state, other, header(source), 1652).is_err());
        let mut wrong_roi = header(source);
        wrong_roi[6] += 1;
        assert!(validate_capture_stamp(&state, view, wrong_roi, 1652).is_err());
        let mut stale_tick = header(source);
        stale_tick[2] = 1651;
        assert!(validate_capture_stamp(&state, view, stale_tick, 1652).is_err());
        app.update(); // Real next main frame, same requested tick for this bounded clock fixture.
        let next = crate::temporal_audit::replay_request(app.world())
            .unwrap()
            .unwrap();
        let next_render = app.world().resource::<bevy::diagnostic::FrameCount>().0;
        assert_ne!(next.source_frame, source.source_frame);
        assert!(!state.is_selected(view, next_render));
        assert!(validate_capture_stamp(&state, view, header(next), 1652).is_err());
        // No stale guard widening: a new source stamp is required before the new exact header can match.
        assert!(written(&state, other, next_render, next).is_err());
        assert!(written(&state, view, next_render, next).is_err()); // Old render stamp cannot authorize next frame.
        *state.stamp.lock().unwrap() = Some((view, next_render));
        written(&state, view, next_render, next).unwrap();
        assert!(validate_capture_stamp(&state, view, header(source), 1652).is_err());
        assert!(validate_capture_stamp(&state, view, header(next), 1652).is_ok());
    }
}
