//! DRIG patch (F886) check: the vendored wgpu-core merges a bind group into a render pass's usage
//! scope and registers its memory init actions only the first time the pass sets it
//! (third_party/README.md). A headless Bevy app renders overlapping alpha-blended quads whose
//! transparent sort alternates two material types, so every draw switches pipeline and material
//! bind group, as the car panes and their reflection overlays do; the textured quads use
//! several textures that the GPU first sees in this frame. The test runs itself twice as a child
//! process, with the dedupe on and with `GRID_WGPU_BIND_DEDUPE=0` (upstream path), and the first
//! frames and a later one must be identical. Needs a GPU adapter, so it is ignored by default and
//! runs in prepush's full tier (`--include-ignored`).

use bevy::app::PluginsState;
use bevy::asset::RenderAssetUsages;
use bevy::camera::RenderTarget;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::render::render_resource::{
    AsBindGroup, Extent3d, TextureDimension, TextureFormat, TextureUsages,
};
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;
use std::sync::{Arc, Mutex};

const SIZE: u32 = 128;
const CHILD: &str = "GRID_BIND_DEDUPE_TEST_OUT";

/// A second material type: its pipeline and bind group differ from `StandardMaterial`'s.
#[derive(Asset, AsBindGroup, TypePath, Clone, Default)]
struct Tint {
    #[uniform(100)]
    strength: f32,
}

impl MaterialExtension for Tint {}

type Tinted = ExtendedMaterial<StandardMaterial, Tint>;

fn checker(app: &mut App, seed: u8) -> Handle<Image> {
    let mut data = Vec::with_capacity(16 * 16 * 4);
    for y in 0..16u8 {
        for x in 0..16u8 {
            let on = (x / 4 + y / 4 + seed).is_multiple_of(2);
            data.extend_from_slice(&[
                if on { 230 } else { 40 },
                seed.wrapping_mul(60),
                if on { 50 } else { 210 },
                if on { 200 } else { 120 },
            ]);
        }
    }
    let image = Image::new(
        Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    );
    app.world_mut().resource_mut::<Assets<Image>>().add(image)
}

/// Renders the scene and returns frames 6, 7 and 12 after start-up.
fn render() -> Vec<Vec<u8>> {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: None,
                exit_condition: ExitCondition::DontExit,
                ..default()
            })
            .set(RenderPlugin {
                synchronous_pipeline_compilation: true,
                ..default()
            })
            .disable::<WinitPlugin>()
            .disable::<bevy::audio::AudioPlugin>()
            .disable::<PipelinedRenderingPlugin>(),
        MaterialPlugin::<Tinted>::default(),
    ));
    while app.plugins_state() == PluginsState::Adding {
        bevy::tasks::tick_global_task_pools_on_main_thread();
    }
    app.finish();
    app.cleanup();

    let mut image = Image::new_target_texture(SIZE, SIZE, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let textures: Vec<_> = (0..4).map(|i| checker(&mut app, i)).collect();
    let quad = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Rectangle::new(1.6, 1.6));
    let ground = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Plane3d::default().mesh().size(10.0, 10.0));
    let ground_material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: Color::srgb(0.6, 0.6, 0.6),
            ..default()
        });
    let world = app.world_mut();
    world.spawn((
        Camera3d::default(),
        RenderTarget::Image(target.clone().into()),
        Transform::from_xyz(0.0, 1.5, 6.0).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
    ));
    world.spawn((
        DirectionalLight {
            illuminance: 8000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(3.0, 8.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    world.spawn((Mesh3d(ground), MeshMaterial3d(ground_material)));
    // Twelve overlapping quads at increasing depth; the back-to-front sort alternates
    // StandardMaterial and Tinted, so each draw switches pipeline and material bind group.
    for i in 0..12 {
        let t = textures[i % textures.len()].clone();
        let alpha = 0.35 + 0.05 * (i % 5) as f32;
        let base = StandardMaterial {
            base_color: Color::srgba(1.0, 1.0, 1.0, alpha),
            base_color_texture: Some(t),
            alpha_mode: AlphaMode::Blend,
            unlit: i % 3 == 0,
            ..default()
        };
        let transform = Transform::from_xyz(
            (i as f32 * 0.37).sin() * 0.9,
            0.9 + (i as f32 * 0.53).cos() * 0.4,
            -1.5 + i as f32 * 0.25,
        );
        if i % 2 == 0 {
            let m = world.resource_mut::<Assets<StandardMaterial>>().add(base);
            world.spawn((Mesh3d(quad.clone()), MeshMaterial3d(m), transform));
        } else {
            let m = world.resource_mut::<Assets<Tinted>>().add(Tinted {
                base,
                extension: Tint { strength: 0.5 },
            });
            world.spawn((Mesh3d(quad.clone()), MeshMaterial3d(m), transform));
        }
    }

    let data = Arc::new(Mutex::new(Vec::new()));
    let sink = data.clone();
    for _ in 0..5 {
        app.update();
    }
    app.world_mut()
        .spawn(Readback::texture(target))
        .observe(move |event: On<ReadbackComplete>| {
            sink.lock().unwrap().push(event.data.clone());
        });
    for _ in 0..60 {
        app.update();
        if data.lock().unwrap().len() >= 8 {
            let got = std::mem::take(&mut *data.lock().unwrap());
            return vec![got[0].clone(), got[1].clone(), got[7].clone()];
        }
    }
    panic!("fewer than 8 readbacks within 60 frames");
}

/// Child mode: renders and writes the frames to the file named by `CHILD`.
#[test]
#[ignore = "needs a GPU adapter; run with --include-ignored"]
fn bind_dedupe_child() {
    let Some(out) = std::env::var_os(CHILD) else {
        return;
    };
    std::fs::write(out, render().concat()).unwrap();
}

fn child(dedupe: bool, out: &std::path::Path) -> Vec<u8> {
    let mut cmd = std::process::Command::new(std::env::current_exe().unwrap());
    cmd.args(["bind_dedupe_child", "--exact", "--include-ignored", "--test-threads=1"])
        .env(CHILD, out);
    if dedupe {
        cmd.env_remove("GRID_WGPU_BIND_DEDUPE");
    } else {
        cmd.env("GRID_WGPU_BIND_DEDUPE", "0");
    }
    let status = cmd.status().unwrap();
    assert!(status.success(), "child render (dedupe {dedupe}) failed: {status}");
    std::fs::read(out).unwrap()
}

#[test]
#[ignore = "needs a GPU adapter; run with --include-ignored"]
fn bind_dedupe_matches_upstream_path() {
    if std::env::var_os(CHILD).is_some() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("grid-bind-dedupe-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let on = child(true, &dir.join("on.bin"));
    let off = child(false, &dir.join("off.bin"));
    let _ = std::fs::remove_dir_all(&dir);
    let frame = (SIZE * SIZE * 4) as usize;
    assert_eq!(on.len(), 3 * frame);
    assert_eq!(off.len(), 3 * frame);
    // The quads must cover part of the image with blended texture colours.
    let covered = on[..frame]
        .chunks(4)
        .zip(off[..frame].chunks(4))
        .filter(|(p, _)| p[0].abs_diff(p[2]) > 40)
        .count();
    assert!(covered > 500, "only {covered} pixels show the quads");
    for f in 0..3 {
        let (a, b) = (&on[f * frame..][..frame], &off[f * frame..][..frame]);
        let differing = a.chunks(4).zip(b.chunks(4)).filter(|(p, q)| p != q).count();
        assert_eq!(differing, 0, "frame {f}: {differing} pixels differ between dedupe on and off");
    }
}
