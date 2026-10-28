//! Bounded neutral full-resolution colour/transmittance plumbing; independently authored.
//! Existing smoke stays in its original main phase. No source-halfres parity or quality fix.
use bevy::{
    asset::{RenderAssetUsages, uuid_handle},
    camera::{MainPassResolutionOverride, NormalizedRenderTarget, RenderTarget},
    core_pipeline::{
        FullscreenShader,
        core_3d::main_transparent_pass_3d,
        schedule::{Core3d, Core3dSystems},
    },
    diagnostic::FrameCount,
    prelude::*,
    render::{
        Extract, ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems,
        camera::ExtractedCamera,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        gpu_readback::{Readback, ReadbackComplete},
        render_asset::RenderAssets,
        render_resource::{binding_types::texture_2d, *},
        renderer::{RenderContext, RenderDevice, ViewQuery},
        texture::GpuImage,
        view::{ExtractedView, ViewTarget},
    },
};
use std::{
    fmt::Write as _,
    fs::{File, OpenOptions},
    io::Write as _,
    path::Path,
    sync::{Arc, Mutex, MutexGuard},
};
const SHADER: Handle<Shader> = uuid_handle!("c2e371af-50db-40de-9d02-8b07bbbcd481");
const WIDTH: u32 = 1920;
const HEIGHT: u32 = 1080;
const SHOTS: usize = 120;
// Four exact binary16 C/T inputs; no dependency on game textures or half libraries.
const PROBE_CT: [u16; 16] = [
    0, 0, 0, 0x3c00, 0x3000, 0x3400, 0x3600, 0, 0x3000, 0x3000, 0x3000, 0x3800, 0, 0, 0, 0x3400,
];
// D=(1/2,1/4,1/8), destination alpha3/4. RGB=C+T*D; destination alpha stays3/4.
const PROBE_EXPECTED: [u16; 16] = [
    0x3800, 0x3400, 0x3000, 0x3a00, 0x3000, 0x3400, 0x3600, 0x3a00, 0x3600, 0x3400, 0x3200, 0x3a00,
    0x3000, 0x2c00, 0x2800, 0x3a00,
];
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Mode {
    #[default]
    Off,
    Neutral,
}
impl Mode {
    pub(crate) fn parse(text: &str) -> Result<Self, &'static str> {
        match text {
            "off" => Ok(Self::Off),
            "neutral" => Ok(Self::Neutral),
            _ => Err("--smoke-ct needs off or neutral"),
        }
    }
}
#[derive(Clone, Debug)]
struct Row {
    entity: u64,
    frame: u32,
    tick: u64,
    submitted: bool,
}
#[derive(Default, Debug)]
struct Ledger {
    rows: Vec<Row>,
    probe_frame: Option<u32>,
    probe_verified: bool,
    probe_words: Option<[u16; 16]>,
    error: Option<String>,
    published: bool,
}
impl Ledger {
    fn observe(
        &mut self,
        entity: u64,
        frame: u32,
        tick: u64,
    ) -> Result<Option<usize>, &'static str> {
        if self.rows.iter().any(|r| r.entity == entity) {
            return Ok(None);
        }
        if self.rows.len() >= SHOTS
            || self
                .rows
                .last()
                .is_some_and(|r| frame <= r.frame || tick < r.tick)
        {
            return Err("CT unexpected/reordered screenshot extraction");
        }
        self.rows.push(Row {
            entity,
            frame,
            tick,
            submitted: false,
        });
        Ok(Some(self.rows.len() - 1))
    }
    fn submit(
        &mut self,
        index: usize,
        entity: u64,
        frame: u32,
        tick: u64,
    ) -> Result<(), &'static str> {
        if self.rows.iter().take(index).any(|r| !r.submitted) {
            return Err("CT reordered compose submission");
        }
        let row = self
            .rows
            .get_mut(index)
            .ok_or("CT submission without expected shot")?;
        if (row.entity, row.frame, row.tick) != (entity, frame, tick) || row.submitted {
            return Err("CT wrong/duplicate frame association");
        }
        row.submitted = true;
        Ok(())
    }
    fn complete(&self) -> bool {
        self.error.is_none()
            && self.probe_verified
            && self.rows.len() == SHOTS
            && self.rows.iter().all(|r| r.submitted)
    }
}
#[derive(Resource, Clone, ExtractResource)]
pub(crate) struct Audit {
    ct: Handle<Image>,
    probe_input: Handle<Image>,
    probe_output: Handle<Image>,
    state: Arc<Mutex<Ledger>>,
    output: Arc<Mutex<File>>,
    probe_queued: bool,
}
impl Audit {
    fn state(&self) -> MutexGuard<'_, Ledger> {
        self.state.lock().unwrap_or_else(|e| {
            let mut s = e.into_inner();
            s.error = Some("CT ledger mutex poisoned".into());
            s
        })
    }
    fn fail(&self, text: impl Into<String>) {
        self.state().error.get_or_insert_with(|| text.into());
    }
    pub(crate) fn ready(&self) -> bool {
        let s = self.state();
        s.published && s.complete()
    }
    pub(crate) fn failed(&self) -> bool {
        self.state().error.is_some()
    }
}
#[derive(Resource, Default)]
struct Snapshot {
    frame: u32,
    tick: u64,
    camera: Option<Entity>,
    target: Option<NormalizedRenderTarget>,
    shot: Option<(usize, u64)>,
}
#[derive(Resource)]
struct Pipelines {
    layout: BindGroupLayoutDescriptor,
    scene: CachedRenderPipelineId,
    probe: CachedRenderPipelineId,
}
#[derive(Resource)]
struct Bindings {
    ct: BindGroup,
    probe: BindGroup,
    ct_texture: TextureId,
    probe_texture: TextureId,
}
#[derive(Component)]
struct ProbeOnce(bool);
fn blend() -> BlendState {
    BlendState {
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
    }
}
fn target(width: u32, height: u32) -> Image {
    let mut image = Image::new_uninit(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        TextureFormat::Rgba16Float,
        RenderAssetUsages::RENDER_WORLD,
    );
    image.texture_descriptor.usage |= TextureUsages::RENDER_ATTACHMENT;
    image
}
/// Off returns before touching paths, Assets, renderer, shaders, nodes or allocations.
pub(crate) fn install(app: &mut App, mode: Mode, capture: Option<&Path>) -> std::io::Result<()> {
    if mode == Mode::Off {
        return Ok(());
    }
    if app.get_sub_app(RenderApp).is_none() {
        return Err(std::io::Error::other("neutral CT needs renderer"));
    }
    let capture =
        capture.ok_or_else(|| std::io::Error::other("neutral CT needs bounded capture"))?;
    let local = Path::new("local").canonicalize()?;
    let parent = capture
        .parent()
        .ok_or_else(|| std::io::Error::other("CT evidence parent absent"))?
        .canonicalize()?;
    if !parent.starts_with(local) {
        return Err(std::io::Error::other(
            "CT evidence must remain under ignored local/",
        ));
    }
    let path = parent.join(
        capture
            .with_extension("ct-neutral.txt")
            .file_name()
            .ok_or_else(|| std::io::Error::other("CT filename absent"))?,
    );
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    writeln!(
        file,
        "smoke_ct version=1 status=pending expected={SHOTS} smoke_draws_unchanged=true"
    )?;
    file.flush()?;
    app.world_mut()
        .resource_mut::<Assets<Shader>>()
        .insert(
            SHADER.id(),
            Shader::from_wgsl(
                include_str!("smoke_ct_neutral.wgsl"),
                "smoke_ct_neutral.wgsl",
            ),
        )
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut images = app.world_mut().resource_mut::<Assets<Image>>();
    let ct = images.add(target(WIDTH, HEIGHT));
    let bytes = PROBE_CT.iter().flat_map(|v| v.to_le_bytes()).collect();
    let probe_input = images.add(Image::new(
        Extent3d {
            width: 4,
            height: 1,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        bytes,
        TextureFormat::Rgba16Float,
        RenderAssetUsages::RENDER_WORLD,
    ));
    let probe_output = images.add(target(4, 1));
    app.insert_resource(Audit {
        ct,
        probe_input,
        probe_output,
        state: Arc::new(Mutex::new(Ledger::default())),
        output: Arc::new(Mutex::new(file)),
        probe_queued: false,
    })
    .add_plugins(ExtractResourcePlugin::<Audit>::default())
    .add_systems(Update, (queue_probe, publish));
    app.get_sub_app_mut(RenderApp)
        .expect("renderer checked")
        .init_resource::<Snapshot>()
        .add_systems(ExtractSchedule, extract)
        .add_systems(RenderStartup, init_pipelines)
        .add_systems(
            Render,
            prepare_bindings.in_set(RenderSystems::PrepareBindGroups),
        )
        .add_systems(
            Core3d,
            compose
                .after(main_transparent_pass_3d)
                .in_set(Core3dSystems::MainPass),
        );
    println!(
        "smoke_ct enabled=true mode=Neutral fullres={WIDTH}x{HEIGHT} default=false output={} smoke_draws_unchanged=true",
        path.display()
    );
    Ok(())
}
fn init_pipelines(
    mut commands: Commands,
    cache: Res<PipelineCache>,
    fullscreen: Res<FullscreenShader>,
) {
    let layout = BindGroupLayoutDescriptor::new(
        "neutral CT",
        &BindGroupLayoutEntries::single(
            ShaderStages::FRAGMENT,
            texture_2d(TextureSampleType::Float { filterable: false }),
        ),
    );
    let descriptor = |format, samples| RenderPipelineDescriptor {
        label: Some(format!("neutral CT {format:?} samples{samples}").into()),
        layout: vec![layout.clone()],
        vertex: fullscreen.to_vertex_state(),
        fragment: Some(FragmentState {
            shader: SHADER,
            entry_point: Some("fragment".into()),
            targets: vec![Some(ColorTargetState {
                format,
                blend: Some(blend()),
                write_mask: ColorWrites::RED | ColorWrites::GREEN | ColorWrites::BLUE,
            })],
            ..default()
        }),
        multisample: MultisampleState {
            count: samples,
            ..default()
        },
        ..default()
    };
    let scene = cache.queue_render_pipeline(descriptor(TextureFormat::Rgba8UnormSrgb, 4));
    let probe = cache.queue_render_pipeline(descriptor(TextureFormat::Rgba16Float, 1));
    commands.insert_resource(Pipelines {
        layout,
        scene,
        probe,
    });
}
fn prepare_bindings(
    mut commands: Commands,
    audit: Res<Audit>,
    pipelines: Option<Res<Pipelines>>,
    cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    images: Res<RenderAssets<GpuImage>>,
    existing: Option<Res<Bindings>>,
) {
    let Some(pipelines) = pipelines else {
        return;
    };
    let (Some(ct), Some(probe)) = (images.get(&audit.ct), images.get(&audit.probe_input)) else {
        return;
    };
    if existing
        .as_ref()
        .is_some_and(|b| b.ct_texture == ct.texture.id() && b.probe_texture == probe.texture.id())
    {
        return;
    }
    let layout = cache.get_bind_group_layout(&pipelines.layout);
    let ct_group = device.create_bind_group(
        "neutral CT fullres",
        &layout,
        &BindGroupEntries::single(&ct.texture_view),
    );
    let probe_group = device.create_bind_group(
        "neutral CT exact probe",
        &layout,
        &BindGroupEntries::single(&probe.texture_view),
    );
    commands.insert_resource(Bindings {
        ct: ct_group,
        probe: probe_group,
        ct_texture: ct.texture.id(),
        probe_texture: probe.texture.id(),
    });
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)] // Bounded extraction with explicit main camera/target and source screenshot stamp identity.
fn extract(
    frame: Extract<Res<FrameCount>>,
    driving: Extract<Res<super::Driving>>,
    audit: Extract<Res<Audit>>,
    shots: Extract<
        Query<(
            Entity,
            &super::skid_phase_ledger::Stamp,
            &bevy::render::view::screenshot::Screenshot,
        )>,
    >,
    cameras: Extract<Query<(Entity, &RenderTarget), With<super::ViewerCamera>>>,
    mut snapshot: ResMut<Snapshot>,
) {
    snapshot.frame = frame.0;
    snapshot.tick = driving.runtime.ticks;
    snapshot.camera = None;
    snapshot.target = None;
    snapshot.shot = None;
    let mut state = audit.state();
    if state.error.is_some() {
        return;
    }
    let mut new = Vec::new();
    for (entity, _, shot) in &shots {
        match state.observe(entity.to_bits(), frame.0, driving.runtime.ticks) {
            Ok(Some(index)) => new.push((index, entity.to_bits(), shot.0.normalize(None))),
            Ok(None) => {}
            Err(e) => {
                state.error = Some(e.into());
                return;
            }
        }
    }
    if new.len() > 1 {
        state.error = Some("CT multiple fresh screenshot stamps in one extraction".into());
        return;
    }
    let Ok((camera, target)) = cameras.single() else {
        if !new.is_empty() {
            state.error = Some("CT selected shot has no unique main camera".into());
        }
        return;
    };
    let normalized = target.normalize(None);
    if let Some((index, entity, expected)) = new.pop() {
        if expected.is_none() || expected != normalized {
            state.error = Some("CT screenshot/main target mismatch".into());
            return;
        }
        snapshot.shot = Some((index, entity));
    }
    snapshot.camera = Some(camera);
    snapshot.target = normalized;
}
type Inputs = (
    &'static ExtractedView,
    &'static ExtractedCamera,
    &'static ViewTarget,
    &'static Msaa,
    Option<&'static MainPassResolutionOverride>,
);
fn selected_skip(audit: &Audit, snapshot: &Snapshot, text: &str) {
    if snapshot.shot.is_some() {
        audit.fail(text);
    }
}
#[allow(clippy::too_many_arguments)] // Per-view render node, fixed resources; no generic renderer changes.
fn compose(
    view: ViewQuery<Inputs>,
    mut context: RenderContext,
    audit: Res<Audit>,
    snapshot: Res<Snapshot>,
    pipelines: Option<Res<Pipelines>>,
    bindings: Option<Res<Bindings>>,
    cache: Res<PipelineCache>,
    images: Res<RenderAssets<GpuImage>>,
) {
    if audit.failed() {
        return;
    }
    let (extracted, camera, target, msaa, resolution_override) = view.into_inner();
    if Some(extracted.retained_view_entity.main_entity.id()) != snapshot.camera
        || extracted.retained_view_entity.subview_index != 0
    {
        return;
    }
    if camera.target.as_ref() != snapshot.target.as_ref()
        || !matches!(
            camera.target.as_ref(),
            Some(NormalizedRenderTarget::Image(_))
        )
        || camera.physical_target_size != Some(UVec2::new(WIDTH, HEIGHT))
        || camera.viewport.is_some()
        || resolution_override.is_some()
        || extracted.viewport != UVec4::new(0, 0, WIDTH, HEIGHT)
        || msaa.samples() != 4
        || target.main_texture_format() != TextureFormat::Rgba8UnormSrgb
    {
        selected_skip(
            &audit,
            &snapshot,
            "CT unexpected selected main view/target/viewport/format/MSAA",
        );
        return;
    }
    let (Some(pipelines), Some(bindings)) = (pipelines, bindings) else {
        selected_skip(&audit, &snapshot, "CT selected pipelines/bindings absent");
        return;
    };
    let (Some(scene_pipeline), Some(probe_pipeline)) = (
        cache.get_render_pipeline(pipelines.scene),
        cache.get_render_pipeline(pipelines.probe),
    ) else {
        selected_skip(&audit, &snapshot, "CT selected pipeline unready");
        return;
    };
    let (Some(ct), Some(probe_input), Some(probe_output)) = (
        images.get(&audit.ct),
        images.get(&audit.probe_input),
        images.get(&audit.probe_output),
    ) else {
        selected_skip(&audit, &snapshot, "CT selected GPU images absent");
        return;
    };
    if bindings.ct_texture != ct.texture.id()
        || bindings.probe_texture != probe_input.texture.id()
        || ct.texture_descriptor.size.width != WIDTH
        || ct.texture_descriptor.size.height != HEIGHT
        || ct.texture_descriptor.format != TextureFormat::Rgba16Float
        || ct.texture_descriptor.sample_count != 1
        || probe_output.texture_descriptor.size
            != (Extent3d {
                width: 4,
                height: 1,
                depth_or_array_layers: 1,
            })
        || probe_output.texture_descriptor.format != TextureFormat::Rgba16Float
        || probe_output.texture_descriptor.sample_count != 1
        || probe_input.texture_descriptor.size
            != (Extent3d {
                width: 4,
                height: 1,
                depth_or_array_layers: 1,
            })
        || probe_input.texture_descriptor.format != TextureFormat::Rgba16Float
        || probe_input.texture_descriptor.sample_count != 1
    {
        selected_skip(&audit, &snapshot, "CT selected texture contract differs");
        return;
    }
    // Clear fullres C=0,T=1. No scene copy, smoke redraw, depth or other view changes.
    {
        let attachments = [Some(RenderPassColorAttachment {
            view: &ct.texture_view,
            depth_slice: None,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(LinearRgba::new(0., 0., 0., 1.).into()),
                store: StoreOp::Store,
            },
        })];
        let descriptor = RenderPassDescriptor {
            label: Some("neutral CT clear C0 T1"),
            color_attachments: &attachments,
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        };
        let _pass = context.begin_tracked_render_pass(descriptor);
    }
    let need_probe = { audit.state().probe_frame.is_none() };
    if need_probe {
        {
            let attachments = [Some(RenderPassColorAttachment {
                view: &probe_output.texture_view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(LinearRgba::new(0.5, 0.25, 0.125, 0.75).into()),
                    store: StoreOp::Store,
                },
            })];
            let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
                label: Some("neutral CT four exact blend contracts"),
                color_attachments: &attachments,
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_render_pipeline(probe_pipeline);
            pass.set_bind_group(0, &bindings.probe, &[]);
            pass.draw(0..3, 0..1);
        }
        audit.state().probe_frame = Some(snapshot.frame);
    }
    {
        let attachments = [Some(target.get_color_attachment())];
        let mut pass = context.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("neutral CT compose RGB C+Tscene"),
            color_attachments: &attachments,
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_render_pipeline(scene_pipeline);
        pass.set_bind_group(0, &bindings.ct, &[]);
        pass.draw(0..3, 0..1);
    }
    if let Some((index, entity)) = snapshot.shot {
        let mut state = audit.state();
        if let Err(e) = state.submit(index, entity, snapshot.frame, snapshot.tick) {
            state.error = Some(e.into());
        }
    }
}
fn queue_probe(mut commands: Commands, mut audit: ResMut<Audit>) {
    if audit.probe_queued {
        return;
    }
    let pending = {
        let state = audit.state();
        state.probe_frame.is_none() || state.error.is_some()
    };
    if pending {
        return;
    }
    audit.probe_queued = true;
    commands
        .spawn((
            Readback::texture(audit.probe_output.clone()),
            ProbeOnce(false),
        ))
        .observe(probe_readback);
}
// CPU reference is used only by the one-time probe verifier, never smoke rendering.
// C is associated colour and T is transmittance; incoming colour below is straight.
fn over_straight(destination: [f32; 4], colour: [f32; 3], opacity: f32) -> [f32; 4] {
    let t = 1. - opacity;
    [
        opacity * colour[0] + t * destination[0],
        opacity * colour[1] + t * destination[1],
        opacity * colour[2] + t * destination[2],
        t * destination[3],
    ]
}
fn compose_contract(ct: [f32; 4], scene: [f32; 4]) -> [f32; 4] {
    [
        ct[0] + ct[3] * scene[0],
        ct[1] + ct[3] * scene[1],
        ct[2] + ct[3] * scene[2],
        scene[3],
    ]
}
// Probe constants are positive finite binary16; reject other encodings instead of
// quietly admitting a malformed NaN/negative constant to the reference contract.
fn positive_probe_half(bits: u16) -> Option<f32> {
    let exponent = u32::from((bits >> 10) & 31);
    let mantissa = u32::from(bits & 1023);
    if bits & 0x8000 != 0 || exponent == 31 {
        return None;
    }
    Some(if exponent == 0 {
        mantissa as f32 * f32::from_bits(103 << 23)
    } else {
        f32::from_bits(((exponent + 112) << 23) | (mantissa << 13))
    })
}
fn verify_probe_constants() -> Result<(), &'static str> {
    let clear = [0., 0., 0., 1.];
    let cases = [
        clear,
        over_straight(clear, [0.125, 0.25, 0.375], 1.),
        over_straight(clear, [0.25, 0.25, 0.25], 0.5),
        over_straight(clear, [0., 0., 0.], 0.75),
    ];
    for (case, ct) in cases.into_iter().enumerate() {
        let expected = compose_contract(ct, [0.5, 0.25, 0.125, 0.75]);
        for (channel, (input, output)) in ct.into_iter().zip(expected).enumerate() {
            if positive_probe_half(PROBE_CT[case * 4 + channel]) != Some(input)
                || positive_probe_half(PROBE_EXPECTED[case * 4 + channel]) != Some(output)
            {
                return Err("CT hardcoded probe constants violate reference algebra");
            }
        }
    }
    Ok(())
}
fn verify_probe(data: &[u8]) -> Result<(), &'static str> {
    verify_probe_constants()?;
    // Pinned GpuReadback allocates aligned256B for one32B row; trailing padding is undefined, not pixels.
    if data.len() != 256 {
        return Err("CT probe requires exactly one padded256B row");
    }
    for (i, expected) in PROBE_EXPECTED.iter().enumerate() {
        if u16::from_le_bytes([data[i * 2], data[i * 2 + 1]]) != *expected {
            return Err("CT GPU blend/alpha contract differs from exact binary16 expectation");
        }
    }
    Ok(())
}
fn probe_readback(
    event: On<ReadbackComplete>,
    audit: Res<Audit>,
    mut once: Query<&mut ProbeOnce>,
    mut commands: Commands,
    mut exit: MessageWriter<AppExit>,
) {
    let Ok(mut once) = once.get_mut(event.entity) else {
        return;
    };
    if std::mem::replace(&mut once.0, true) {
        return;
    }
    match verify_probe(&event.data) {
        Ok(()) => {
            let words = std::array::from_fn(|i| {
                u16::from_le_bytes([event.data[i * 2], event.data[i * 2 + 1]])
            });
            {
                let mut state = audit.state();
                state.probe_words = Some(words);
                state.probe_verified = true;
            }
            println!(
                "smoke_ct probe_verified=true pixels=4 format=Rgba16Float payload_bytes=32 padded_readback_bytes=256 exact_bits=true"
            );
        }
        Err(e) => {
            audit.fail(e);
            error!("smoke_ct probe failed: {e}");
            exit.write(AppExit::error());
        }
    }
    commands.entity(event.entity).despawn();
}
fn publish(audit: Res<Audit>, driving: Res<super::Driving>, mut exit: MessageWriter<AppExit>) {
    let mut state = audit.state();
    if !state.published
        && !state.complete()
        && (driving.failure.is_some()
            || driving.paused
            || driving
                .scripted_ticks
                .is_some_and(|n| driving.runtime.ticks >= n))
    {
        state
            .error
            .get_or_insert_with(|| "incomplete neutral CT at driving stop/script limit".into());
    }
    if state.published || (!state.complete() && state.error.is_none()) {
        return;
    }
    let mut text = String::new();
    let _ = writeln!(
        text,
        "ct_probe submitted_frame={:?} verified={} cases=4 exact_binary16=true actual_bits={:?}",
        state.probe_frame, state.probe_verified, state.probe_words
    );
    for (index, row) in state.rows.iter().enumerate() {
        let _ = writeln!(
            text,
            "ct_shot index={index} screenshot_entity_bits={} extract_frame={} tick={} compose_submitted={} ct_format=Rgba16Float scene_format=Rgba8UnormSrgb scene_samples=4 rgb_only=true smoke_draws_unchanged=true",
            row.entity, row.frame, row.tick, row.submitted
        );
    }
    let _ = writeln!(
        text,
        "smoke_ct status={} expected={SHOTS} observed={} error={:?} proof=CPU-command-submission-plus-contract-readback-not-fragment-trace",
        if state.complete() {
            "complete"
        } else {
            "failed"
        },
        state.rows.len(),
        state.error
    );
    let result = audit
        .output
        .lock()
        .map_err(|_| std::io::Error::other("CT file lock poisoned"))
        .and_then(|mut f| f.write_all(text.as_bytes()).and_then(|_| f.flush()));
    if let Err(e) = result {
        state.error = Some(format!("CT evidence write: {e}"));
    }
    state.published = true;
    if let Some(e) = &state.error {
        error!("smoke_ct failed: {e}");
        exit.write(AppExit::error());
    } else {
        println!(
            "smoke_ct complete=true shots={SHOTS} exact_probe_verified=true stage=after-main-transparent smoke_draws_unchanged=true"
        );
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_installer_needs_no_path_assets_renderer_or_nodes() {
        let mut app = App::new();
        install(&mut app, Mode::Off, None).unwrap();
        assert!(app.get_sub_app(RenderApp).is_none());
        assert!(!app.world().contains_resource::<Audit>());
        assert!(!app.world().contains_resource::<Assets<Image>>());
        assert!(!app.world().contains_resource::<Assets<Shader>>());
    }
    #[test]
    fn probe_rejects_wrong_rgb_alpha_short_or_extra_data_and_ignores_only_padding() {
        let mut data = vec![0xa5; 256];
        for (i, v) in PROBE_EXPECTED.iter().enumerate() {
            data[i * 2..i * 2 + 2].copy_from_slice(&v.to_le_bytes());
        }
        assert!(verify_probe(&data).is_ok());
        for i in [0, 3, 4, 7, 8, 11, 12, 15] {
            let mut bad = data.clone();
            bad[i * 2] ^= 1;
            assert!(verify_probe(&bad).is_err());
        }
        assert!(verify_probe(&data[..32]).is_err());
        data.push(0);
        assert!(verify_probe(&data).is_err());
    }
    #[test]
    fn stage_acks_must_match_full_identity_and_missing_node_or_probe_cannot_complete() {
        let mut s = Ledger::default();
        for i in 0..SHOTS {
            assert_eq!(
                s.observe(1000 + i as u64, 200 + i as u32, 350 + i as u64)
                    .unwrap(),
                Some(i)
            );
        }
        assert_eq!(s.observe(1000, 500, 700).unwrap(), None); // Old pending stamp, not a new PNG.
        assert!(!s.complete());
        assert!(s.submit(1, 1001, 201, 351).is_err());
        assert!(s.submit(0, 1001, 200, 350).is_err());
        for i in 0..SHOTS - 1 {
            s.submit(i, 1000 + i as u64, 200 + i as u32, 350 + i as u64)
                .unwrap();
        }
        s.probe_verified = true;
        assert!(!s.complete());
        s.submit(SHOTS - 1, 1119, 319, 469).unwrap();
        assert!(s.complete());
        assert!(s.submit(0, 1000, 200, 350).is_err());
        s.error = Some("failed".into());
        assert!(!s.complete());
    }
    #[test]
    fn fresh_stamp_rejects_duplicate_frame_and_tick_rollback() {
        let mut state = Ledger::default();
        state.observe(1, 20, 350).unwrap();
        assert!(state.observe(2, 20, 352).is_err());
        assert!(state.observe(2, 21, 349).is_err());
        state.observe(2, 21, 352).unwrap();
        assert!(state.observe(3, 19, 354).is_err());
    }
    #[test]
    fn production_probe_reference_empty_opaque_and_two_ordered_fragments() {
        let clear = [0., 0., 0., 1.];
        let scene = [0.5, 0.25, 0.125, 0.75];
        assert_eq!(compose_contract(clear, scene), scene);
        let opaque = over_straight(clear, [0.25, 0.5, 0.75], 1.);
        assert_eq!(opaque, [0.25, 0.5, 0.75, 0.]);
        assert_eq!(compose_contract(opaque, scene), [0.25, 0.5, 0.75, 0.75]);
        let back = over_straight(clear, [1., 0., 0.], 0.25);
        let front = over_straight(back, [0., 0., 1.], 0.5);
        assert_eq!(front, [0.125, 0., 0.5, 0.375]);
        assert_eq!(
            compose_contract(front, scene),
            [0.3125, 0.09375, 0.546875, 0.75]
        );
        let reversed = over_straight(over_straight(clear, [0., 0., 1.], 0.5), [1., 0., 0.], 0.25);
        assert_eq!(
            compose_contract(reversed, scene),
            [0.4375, 0.09375, 0.421875, 0.75]
        );
        assert_ne!(front, reversed);
        assert_eq!(front[3], reversed[3]);
        assert!(verify_probe_constants().is_ok());
    }
}
