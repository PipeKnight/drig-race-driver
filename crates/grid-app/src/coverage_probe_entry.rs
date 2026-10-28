//! Isolated desktop sample-coverage probe; no gameplay/profile/game-root loading.
use super::coverage_probe as probe;
use bevy::{
    app::{AppExit, ScheduleRunnerPlugin}, asset::uuid_handle, prelude::*,
    core_pipeline::{core_3d::main_transparent_pass_3d, schedule::{Core3d, Core3dSystems}},
    render::{RenderApp, RenderStartup, extract_component::{ExtractComponent, ExtractComponentPlugin},
        extract_resource::{ExtractResource, ExtractResourcePlugin}, gpu_readback::{Readback, ReadbackComplete},
        render_asset::RenderAssets, render_resource::*, renderer::{RenderAdapter, RenderContext, RenderDevice, ViewQuery},
        storage::{GpuShaderBuffer, ShaderBuffer}},
    window::ExitCondition, winit::WinitPlugin,
};
use std::{fs::{File, OpenOptions}, io::Write, path::{Component, Path},
    sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}}, time::{Duration, Instant}};
const SHADER: Handle<Shader> = uuid_handle!("91d74f4c-8151-479a-bf16-a536757a9a75");
#[derive(Component, Clone, ExtractComponent)] struct ProbeView;
#[derive(Component)] struct Once(bool);
#[derive(Resource)] struct Prepared(probe::Probe);
#[derive(Resource, Clone, ExtractResource)]
struct Audit {
    buffer: Handle<ShaderBuffer>, file: Arc<Mutex<File>>, started: Instant,
    submitted: Arc<AtomicBool>, finished: Arc<AtomicBool>, verified: Arc<AtomicBool>, queued: bool,
}
/// Caller branches to this BEFORE settings::load/game_root/menu paths; exact two-argument CLI only.
pub(crate) fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() != 2 || args[0] != "--coverage-sample-probe" {
        return Err("usage: --coverage-sample-probe local/.../fresh-result.json (no other args)".into());
    }
    let path = Path::new(&args[1]);
    if path.components().any(|c| matches!(c, Component::ParentDir))
        || path.components().any(|c| c.as_os_str().to_string_lossy().eq_ignore_ascii_case("assets-clear")) {
        return Err("probe output rejects parent components/Steam alias".into());
    }
    let cwd = std::env::current_dir()?;
    let requested = if path.is_absolute() {path.to_owned()} else {cwd.join(path)};
    let local = cwd.join("local");
    if !requested.starts_with(&local) || path.extension().is_none_or(|s|s != "json") {
        return Err("probe output must be a fresh JSON inside this checkout ignored local".into());
    }
    // Reject links before canonicalization: do not traverse a local link to Steam.
    let parent = requested.parent().ok_or("probe output parent")?;
    let mut ancestor = Some(parent);
    while let Some(dir) = ancestor {
        let meta = std::fs::symlink_metadata(dir)?;
        #[cfg(windows)] {
            use std::os::windows::fs::MetadataExt;
            if meta.file_attributes() & 0x400 != 0 {return Err("reparse output ancestor rejected".into());}
        }
        if meta.file_type().is_symlink() {return Err("symlink output ancestor rejected".into());}
        ancestor = dir.parent();
    }
    if !parent.canonicalize()?.starts_with(local.canonicalize()?) {return Err("output parent escaped local".into());}
    let file = OpenOptions::new().write(true).create_new(true).open(&requested)?;
    let mut app = App::new();
    app.add_plugins(DefaultPlugins
        .set(bevy::asset::AssetPlugin {file_path:parent.to_string_lossy().into_owned(),
            processed_file_path:parent.to_string_lossy().into_owned(),watch_for_changes_override:Some(false),
            mode:bevy::asset::AssetMode::Unprocessed,..default()})
        .set(WindowPlugin {primary_window:None,exit_condition:ExitCondition::DontExit,..default()})
        .disable::<WinitPlugin>().disable::<bevy::audio::AudioPlugin>())
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_millis(5)));
    app.world_mut().resource_mut::<Assets<Shader>>().insert(SHADER.id(),
        Shader::from_wgsl(include_str!("coverage_probe.wgsl"), "coverage_sample_fixture.wgsl")).map_err(|e|std::io::Error::other(e.to_string()))?;
    let mut output = ShaderBuffer::from(vec![UVec4::splat(probe::SENTINEL); probe::WORDS/4]);
    output.buffer_description.usage |= BufferUsages::COPY_SRC;
    let buffer = app.world_mut().resource_mut::<Assets<ShaderBuffer>>().add(output);
    let audit = Audit {buffer,file:Arc::new(Mutex::new(file)),started:Instant::now(),
        submitted:Arc::default(),finished:Arc::default(),verified:Arc::default(),queued:false};
    let verified = audit.verified.clone();
    app.insert_resource(audit).add_plugins((ExtractResourcePlugin::<Audit>::default(),
        ExtractComponentPlugin::<ProbeView>::default())).add_systems(Startup, camera)
        .add_systems(Update, queue_or_timeout);
    app.get_sub_app_mut(RenderApp).ok_or("probe renderer absent")?
        .add_systems(RenderStartup, prepare)
        .add_systems(Core3d, encode.after(main_transparent_pass_3d).in_set(Core3dSystems::MainPass));
    let exit = app.run();
    if exit.is_error() || !verified.load(Ordering::Acquire) {return Err("coverage probe failed or no verified readback".into());}
    Ok(())
}
fn camera(mut commands: Commands, mut images: ResMut<Assets<Image>>) {
    let image = images.add(Image::new_target_texture(1,1,TextureFormat::Rgba8UnormSrgb,None));
    commands.spawn((Camera3d::default(),bevy::camera::RenderTarget::Image(image.into()),Msaa::Sample4,ProbeView));
}
fn prepare(mut commands: Commands, device: Res<RenderDevice>, adapter: Res<RenderAdapter>, cache: Res<PipelineCache>) {
    commands.insert_resource(Prepared(probe::prepare(&device,&adapter,&cache,SHADER)));
}
fn encode(_: ViewQuery<&ProbeView>, mut context: RenderContext, audit: Res<Audit>,
    prepared: Option<Res<Prepared>>, buffers: Res<RenderAssets<GpuShaderBuffer>>,
    cache: Res<PipelineCache>, device: Res<RenderDevice>) {
    if audit.submitted.load(Ordering::Acquire) || audit.finished.load(Ordering::Acquire) {return;}
    let (Some(prepared),Some(output)) = (prepared,buffers.get(&audit.buffer)) else {return;};
    if probe::encode(&mut context,&device,&cache,&prepared.0,output) {
        audit.submitted.store(true,Ordering::Release);
        println!("coverage_probe submitted=true private_extent=64x4 samples=4 buffer_bytes={} no_resolve=true no_scene=true",probe::BYTES);
    }
}
fn finish(audit: &Audit, result: Result<String, &'static str>) -> AppExit {
    if audit.finished.swap(true,Ordering::AcqRel) {return AppExit::error();}
    let good = result.is_ok();
    let text = result.unwrap_or_else(|reason|format!("{{\"status\":\"failed\",\"reason\":\"{reason}\"}}"));
    if text.len()>65536 {eprintln!("coverage_probe JSON exceeds owned bound");return AppExit::error();}
    let written = audit.file.lock().ok().is_some_and(|mut f| f.write_all(text.as_bytes()).and_then(|_|f.flush()).is_ok());
    if !written || !good {eprintln!("coverage_probe verified=false written={written}");return AppExit::error();}
    audit.verified.store(true,Ordering::Release);
    println!("coverage_probe verified=true json_bytes={} scope=synthetic_sample_invariants_not_stadium_fix",text.len());
    AppExit::Success
}
fn queue_or_timeout(mut commands: Commands, mut audit: ResMut<Audit>, mut exit: MessageWriter<AppExit>) {
    if audit.finished.load(Ordering::Acquire) {return;}
    if audit.started.elapsed()>Duration::from_secs(60) {
        exit.write(finish(&audit,Err("pipeline-or-readback-timeout"))); return;
    }
    if audit.submitted.load(Ordering::Acquire) && !audit.queued {
        audit.queued=true;
        commands.spawn((Readback::buffer(audit.buffer.clone()),Once(false))).observe(readback);
    }
}
fn readback(event: On<ReadbackComplete>, audit: Res<Audit>, mut commands: Commands,
    mut once: Query<&mut Once>, mut exit: MessageWriter<AppExit>) {
    let Ok(mut state)=once.get_mut(event.entity) else {return;};
    if std::mem::replace(&mut state.0,true) {return;}
    exit.write(finish(&audit,probe::verify_json(&event.data)));
    commands.entity(event.entity).despawn();
}
