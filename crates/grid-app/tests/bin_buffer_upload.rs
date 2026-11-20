//! DRIG patch (F875) check: the vendored bevy_render uploads a multidrawable
//! batch set's bin buffers only when Bevy changed them (third_party/README.md).
//! A headless Bevy app renders lit, shadowed meshes in several bins and batch
//! sets, then changes the scene the way play does: despawns (a bin empties),
//! spawns with a new mesh (a new bin), material swaps (an entity moves to
//! another batch set, as car-glass proxy materials do), hiding and showing,
//! and shadow views that go and come back. Each change lands on one frame; the
//! two frames after it and a later one must equal a fresh app's render of the
//! same scene, so no stale bin buffer is drawn. Needs a GPU adapter, so it is
//! ignored by default and runs in prepush's full tier (`--include-ignored`).

use bevy::app::PluginsState;
use bevy::camera::RenderTarget;
use bevy::prelude::*;
use bevy::render::RenderPlugin;
use bevy::render::gpu_readback::{Readback, ReadbackComplete};
use bevy::render::pipelined_rendering::PipelinedRenderingPlugin;
use bevy::render::render_resource::{TextureFormat, TextureUsages};
use bevy::window::ExitCondition;
use bevy::winit::WinitPlugin;
use std::sync::{Arc, Mutex};

const SIZE: u32 = 128;

/// One object: grid slot, mesh and material index, visible.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Item {
    slot: usize,
    mesh: usize,
    material: usize,
    visible: bool,
}

#[derive(Component)]
struct Slot(usize);

struct Scene {
    app: App,
    target: Handle<Image>,
    meshes: Vec<Handle<Mesh>>,
    materials: Vec<Handle<StandardMaterial>>,
    light: Entity,
}

fn new_scene() -> Scene {
    let mut app = App::new();
    app.add_plugins(
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
    );
    while app.plugins_state() == PluginsState::Adding {
        bevy::tasks::tick_global_task_pools_on_main_thread();
    }
    app.finish();
    app.cleanup();

    // The readback copies from the target, so it needs COPY_SRC.
    let mut image = Image::new_target_texture(SIZE, SIZE, TextureFormat::Rgba8UnormSrgb, None);
    image.texture_descriptor.usage |= TextureUsages::COPY_SRC;
    let target = app.world_mut().resource_mut::<Assets<Image>>().add(image);
    let meshes = {
        let mut assets = app.world_mut().resource_mut::<Assets<Mesh>>();
        vec![
            assets.add(Cuboid::new(0.8, 0.8, 0.8)),
            assets.add(Sphere::new(0.45)),
            assets.add(Cylinder::new(0.35, 0.9)),
            assets.add(Torus::new(0.2, 0.4)),
        ]
    };
    let materials = {
        let mut assets = app.world_mut().resource_mut::<Assets<StandardMaterial>>();
        [
            Color::srgb(0.9, 0.2, 0.2),
            Color::srgb(0.2, 0.8, 0.3),
            Color::srgb(0.2, 0.3, 0.9),
        ]
        .into_iter()
        .map(|color| {
            assets.add(StandardMaterial {
                base_color: color,
                ..default()
            })
        })
        .collect::<Vec<_>>()
    };
    let ground_mesh = app
        .world_mut()
        .resource_mut::<Assets<Mesh>>()
        .add(Plane3d::default().mesh().size(12.0, 12.0));
    let ground_material = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: Color::srgb(0.7, 0.7, 0.7),
            ..default()
        });
    let world = app.world_mut();
    world.spawn((
        Camera3d::default(),
        RenderTarget::Image(target.clone().into()),
        Transform::from_xyz(0.0, 7.0, 9.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    let light = world
        .spawn((
            DirectionalLight {
                illuminance: 8000.0,
                shadow_maps_enabled: true,
                ..default()
            },
            Transform::from_xyz(3.0, 8.0, 2.0).looking_at(Vec3::ZERO, Vec3::Y),
        ))
        .id();
    world.spawn((
        Mesh3d(ground_mesh),
        MeshMaterial3d(ground_material),
        Transform::default(),
    ));
    Scene {
        app,
        target,
        meshes,
        materials,
        light,
    }
}

fn slot_position(slot: usize) -> Vec3 {
    let (x, z) = ((slot % 4) as f32, (slot / 4) as f32);
    Vec3::new(x * 1.6 - 2.4, 0.5, z * 1.6 - 1.6)
}

/// Switches the light's shadow maps: the shadow views (and their phases)
/// go away and come back.
fn set_shadows(scene: &mut Scene, on: bool) {
    let world = scene.app.world_mut();
    let mut light = world
        .get_mut::<DirectionalLight>(scene.light)
        .expect("light");
    if light.shadow_maps_enabled != on {
        light.shadow_maps_enabled = on;
    }
}

/// Makes the scene's objects match `items`: despawns, spawns, re-meshes,
/// re-materials and hides only what differs.
fn apply(scene: &mut Scene, items: &[Item]) {
    let world = scene.app.world_mut();
    let mut query = world.query::<(Entity, &Slot)>();
    let existing: Vec<(Entity, usize)> = query.iter(world).map(|(e, s)| (e, s.0)).collect();
    for (entity, slot) in &existing {
        match items.iter().find(|item| item.slot == *slot) {
            None => {
                world.despawn(*entity);
            }
            Some(item) => {
                let mut e = world.entity_mut(*entity);
                if e.get::<Mesh3d>().map(|m| m.0.clone()) != Some(scene.meshes[item.mesh].clone()) {
                    e.insert(Mesh3d(scene.meshes[item.mesh].clone()));
                }
                if e.get::<MeshMaterial3d<StandardMaterial>>()
                    .map(|m| m.0.clone())
                    != Some(scene.materials[item.material].clone())
                {
                    e.insert(MeshMaterial3d(scene.materials[item.material].clone()));
                }
                let visibility = if item.visible {
                    Visibility::Inherited
                } else {
                    Visibility::Hidden
                };
                if e.get::<Visibility>() != Some(&visibility) {
                    e.insert(visibility);
                }
            }
        }
    }
    for item in items {
        if existing.iter().any(|(_, slot)| *slot == item.slot) {
            continue;
        }
        world.spawn((
            Slot(item.slot),
            Mesh3d(scene.meshes[item.mesh].clone()),
            MeshMaterial3d(scene.materials[item.material].clone()),
            Transform::from_translation(slot_position(item.slot)),
            if item.visible {
                Visibility::Inherited
            } else {
                Visibility::Hidden
            },
        ));
    }
}

/// Reads back the next `count` consecutive frames, starting with the next
/// update.
fn frames(scene: &mut Scene, count: usize) -> Vec<Vec<u8>> {
    let data = Arc::new(Mutex::new(Vec::new()));
    let sink = data.clone();
    let reader = scene
        .app
        .world_mut()
        .spawn(Readback::texture(scene.target.clone()))
        .observe(move |event: On<ReadbackComplete>| {
            sink.lock().unwrap().push(event.data.clone());
        })
        .id();
    for _ in 0..40 {
        scene.app.update();
        if data.lock().unwrap().len() >= count {
            scene.app.world_mut().despawn(reader);
            let mut got = std::mem::take(&mut *data.lock().unwrap());
            got.truncate(count);
            return got;
        }
    }
    panic!("fewer than {count} readbacks within 40 frames");
}

fn settle(scene: &mut Scene) {
    for _ in 0..5 {
        scene.app.update();
    }
}

fn fresh_render(items: &[Item], shadows: bool) -> Vec<u8> {
    let mut scene = new_scene();
    set_shadows(&mut scene, shadows);
    apply(&mut scene, items);
    settle(&mut scene);
    frames(&mut scene, 1).remove(0)
}

/// Pixels whose largest channel difference exceeds 2.
fn differing(a: &[u8], b: &[u8]) -> usize {
    assert_eq!(a.len(), b.len());
    a.chunks(4)
        .zip(b.chunks(4))
        .filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| x.abs_diff(*y) > 2))
        .count()
}

fn item(slot: usize, mesh: usize, material: usize) -> Item {
    Item {
        slot,
        mesh,
        material,
        visible: true,
    }
}

#[test]
#[ignore = "needs a GPU adapter"]
fn bin_buffers_follow_scene_changes() {
    // Several meshes (bins) and materials (batch sets), lit and shadowed.
    let s0: Vec<Item> = (0..10).map(|slot| item(slot, slot % 3, slot % 3)).collect();
    // Only an insert: one entity joins an existing bin, nothing else in its
    // batch set changes (materials can share one bindless batch set, so a
    // step that also removes something would flag the set anyway).
    let mut spawn_only = s0.clone();
    spawn_only.push(item(11, 1, 1));
    // Two entities of one bin go (slots 3 and 6), a new mesh appears (a new
    // bin), an entity moves to another batch set, one is hidden.
    let mut s1 = s0.clone();
    s1.retain(|it| it.slot != 3 && it.slot != 6);
    s1.push(item(10, 3, 0));
    s1.iter_mut().find(|it| it.slot == 4).unwrap().material = 2;
    s1.iter_mut().find(|it| it.slot == 7).unwrap().visible = false;
    // Only an insert again: the hidden entity is shown.
    let mut show_only = s1.clone();
    show_only
        .iter_mut()
        .find(|it| it.slot == 7)
        .unwrap()
        .visible = true;
    // The whole red batch set goes, everything else shown again, one mesh swap.
    let mut s2 = show_only.clone();
    s2.retain(|it| it.material != 0);
    s2.iter_mut().for_each(|it| it.visible = true);
    s2.iter_mut().find(|it| it.slot == 5).unwrap().mesh = 3;
    // Then the shadow views go and come back (shadow-view count changes).
    let states = [
        ("s0", s0.clone(), true),
        ("spawn into an existing bin", spawn_only, true),
        ("s1", s1, true),
        ("show a hidden entity", show_only, true),
        ("s2", s2, true),
        ("s0 without shadows", s0.clone(), false),
        ("s0 again", s0, true),
    ];

    let mut scene = new_scene();
    apply(&mut scene, &states[0].1);
    settle(&mut scene);
    let mut previous: Option<Vec<u8>> = None;
    for (name, items, shadows) in &states {
        // The change lands on one frame only. The two frames after it, and a
        // later one, must all draw the new bins: an upload that reached one
        // frame's buffers but not the next would show here.
        set_shadows(&mut scene, *shadows);
        apply(&mut scene, items);
        let got = frames(&mut scene, 3);
        settle(&mut scene);
        let later = frames(&mut scene, 1).remove(0);
        let want = fresh_render(items, *shadows);
        if let Some(previous) = &previous {
            assert!(
                differing(&got[2], previous) > 20,
                "{name}: the change is visible"
            );
        }
        for (label, image) in [
            ("change frame + 1", &got[1]),
            ("change frame + 2", &got[2]),
            ("later", &later),
        ] {
            let diff = differing(image, &want);
            assert_eq!(
                diff, 0,
                "{name}, {label}: {diff} pixels differ from a fresh render of the same scene"
            );
        }
        previous = Some(later);
    }
}
