//! Car part batching (F584). Each car's body parts are merged into one mesh
//! per material in the car's frame, so a 12-car field reaches the per-entity
//! passes (visibility, shadow cascades, mesh collection, draw queues) as about
//! a hundred batches instead of over a thousand parts. Vertices are placed by
//! the part's rest matrix as the vertex shader would, and the batch follows
//! the car's pose. The picture differs only by float rounding: single MSAA
//! samples flip where stickers lie coplanar on the body (docs/PERFORMANCE.md
//! "Car part batching (F584)").
//!
//! The parts keep their entities and every component the simulation's
//! presentation writes (transform, visibility, material, deformed mesh); a
//! batched part only moves to a render layer no view draws. Each frame a part
//! stays in its batch while it sits at its rest place on the car, is not
//! hidden and keeps its material and mesh; otherwise it leaves (a loose or
//! detached panel, smashed glass, a cracked-glass material) and is drawn on
//! its own again. A part that is back in place for [`REJOIN_FRAMES`] (a
//! restart) rejoins. A deformed member mesh rebuilds its batch. Wheels and
//! blended materials keep their own entities. Common build;
//! `GRID_CAR_BATCH=0` keeps every part separate (comparisons).
//!
//! An opponent whose model has a low-detail file casts its shadow with a
//! [`ShadowBody`], the model's coarsest level, as the original does
//! (docs/CAR_LOD.md "Shadows"); its parts and batches cast none. The body
//! sits on a layer only the sun takes casters from, and follows the car.
//!
//! Such an opponent also carries its low-detail levels 2..5 as [`LodPart`]s
//! on the hidden layer, batched like its LOD0 parts. Each frame the original's
//! selection ([`crate::car_lod`]) picks the level of every opponent whose LOD0
//! parts are all batched; its batches and loose parts of the other levels move
//! to the hidden layer. The low body parts deform with the car's field like
//! its LOD0 parts. An opponent with a part off its place (a loose or detached
//! panel) stays at LOD0 (AUTHORED: the low files have the panels, but how the
//! original moves them is not traced).
//!
//! The original switches a car's level within one frame (0x8F4F30). Here a
//! level change (by distance, or to LOD0 for a loose panel) fades over `ornament_lod::FADE_SECONDS` with
//! Bevy's dither, the old level following the car until it is gone (owner,
//! 2026-11-03: nothing should appear in view; F708). A camera cut switches at
//! once; `GRID_ORNAMENT_FADE=0` keeps the hard switch.
use crate::static_batch::{ATTRIBUTES, MAX_VERTICES, Part, build, supported};
use bevy::{
    asset::AssetEventSystems,
    camera::{
        primitives::{Aabb, MeshAabb},
        visibility::{NoAutoAabb, RenderLayers, VisibilityRange, VisibilitySystems},
    },
    light::NotShadowCaster,
    mesh::{MeshVertexAttributeId, PrimitiveTopology, VertexAttributeValues, VertexFormat},
    prelude::*,
    transform::TransformSystems,
};
use bevy::asset::AssetId;
use std::collections::HashSet;

/// The layer of batched parts: no camera or light draws it.
const HIDDEN_LAYER: usize = 30;
/// The layer of the shadow bodies: the sun casts from it, no camera draws it.
const SHADOW_LAYER: usize = 29;
/// Frames a part must be back in place before it rejoins its batch.
const REJOIN_FRAMES: u32 = 30;
/// A part this far (metres, or matrix units for the rotation) from its rest
/// place on the car has left it; float rounding of the pose stays far below.
const PLACE_METRES: f32 = 1e-3;
const PLACE_AXES: f32 = 1e-4;
/// Frames between `car_batch` log lines.
const REPORT_FRAMES: u64 = 1800;

pub fn enabled() -> bool {
    crate::dev::var("GRID_CAR_BATCH").map_or(true, |v| v != "0")
}

pub fn install(app: &mut App) {
    if !enabled() {
        println!("car_batch version=1 enabled=false");
        return;
    }
    app.init_resource::<CarBatches>()
        .init_resource::<Demoted>()
        .add_systems(
            PostUpdate,
            (refresh, restore)
                .chain()
                .after(AssetEventSystems)
                .before(TransformSystems::Propagate)
                .before(VisibilitySystems::VisibilityPropagate),
        );
}

/// Part meshes written for the main world only: a batched part is drawn by
/// its batch, so a dent need not clone and upload its own mesh too.
/// `drive_present` records each; [`restore`] gives the part its render copy
/// back in the frame it leaves the hidden layer (a loose panel, a level
/// shown on its own), before extraction.
#[derive(Resource, Default)]
pub struct Demoted(pub std::collections::HashMap<Entity, AssetId<Mesh>>);

/// Whether a part sits on the hidden layer of batched parts.
pub fn hidden(layers: Option<&RenderLayers>) -> bool {
    layers == Some(&RenderLayers::layer(HIDDEN_LAYER))
}

fn restore(mut demoted: ResMut<Demoted>, parts: Query<Option<&RenderLayers>>, mut meshes: ResMut<Assets<Mesh>>) {
    demoted.0.retain(|&entity, &mut mesh| match parts.get(entity) {
        Ok(layers) if hidden(layers) => true,
        Ok(_) => {
            if let Some(mut mesh) = meshes.get_mut(mesh) {
                mesh.asset_usage |= bevy::asset::RenderAssetUsages::RENDER_WORLD;
            }
            false
        }
        Err(_) => false,
    });
}

/// A merged mesh of one car's parts of one material.
#[derive(Component)]
pub struct Batch {
    /// The car, 0 the player's.
    pub car: usize,
}

/// Opponent `car`'s shadow caster: its model's coarsest level.
#[derive(Component)]
pub struct ShadowBody {
    pub car: usize,
}

pub fn shadow_layer() -> RenderLayers {
    RenderLayers::layer(SHADOW_LAYER)
}

/// The sun's layers with shadow bodies: the default layer and theirs.
pub fn sun_layers() -> RenderLayers {
    RenderLayers::from_layers(&[0, SHADOW_LAYER])
}

/// An opponent's part of low-detail `level` (2..5); spawned on
/// [`hidden_layer`] and shown with its level.
#[derive(Component)]
pub struct LodPart {
    pub level: u8,
}

/// The presentation skips this opponent part's pose: a low-detail part merged
/// into its batches (the batch follows the car), or a loose part (wheel,
/// glass) of a level its car does not show. A level's loose parts are posed
/// one frame before the level is shown, so they appear in place.
#[derive(Component)]
pub struct Unposed;

/// The layer no camera or light draws.
pub fn hidden_layer() -> RenderLayers {
    RenderLayers::layer(HIDDEN_LAYER)
}

/// Half extents of the box whose corners weigh a car partly off the screen,
/// in the car's frame (metres; a saloon, AUTHORED).
const CAR_BOX: Vec3 = Vec3::new(1., 0.75, 2.3);
const CAR_BOX_CENTRE_Y: f32 = 0.7;

/// A part that can be batched: its place in [`CarBatches`], so the per-frame
/// check walks the parts in storage order.
#[derive(Component)]
struct Batchable {
    group: u32,
    member: u32,
}

struct Member {
    entity: Entity,
    rest: Mat4,
    rest_inverse: Mat4,
    mesh: Handle<Mesh>,
    /// `mesh`'s id, kept so the per-frame checks need not follow the handle.
    id: AssetId<Mesh>,
    vertices: usize,
    batched: bool,
    /// In place this frame.
    placed: bool,
    /// Still spawned this frame.
    alive: bool,
    /// Consecutive frames in place.
    stable: u32,
    /// Its vertices in the batch may differ from a rebuild's: its mesh
    /// changed, or it left or joined the batch since the last rebuild.
    stale: bool,
}

/// Where the last full rebuild put a group's members: per member its batch,
/// first vertex and the bounds of its vertices there. A rebuild for changed
/// members rewrites only theirs ([`patch`]).
struct Built {
    slots: Vec<Slot>,
    batches: usize,
}

#[derive(Clone, Copy)]
struct Slot {
    batch: usize,
    first: usize,
    min: Vec3,
    max: Vec3,
}

struct Group {
    car: usize,
    /// Index of the car in [`CarBatches::anchors`].
    slot: usize,
    material: Handle<StandardMaterial>,
    layout: Vec<(MeshVertexAttributeId, VertexFormat)>,
    members: Vec<Member>,
    batches: Vec<Entity>,
    dirty: bool,
    /// The layout of the last full rebuild; `None` until the first.
    built: Option<Built>,
    /// The car casts with its [`ShadowBody`]: parts and batches cast none.
    shadow_body: bool,
    /// Level of detail: 0, or a low level 2..5.
    level: u8,
}

#[derive(Resource, Default)]
struct CarBatches {
    groups: Vec<Group>,
    /// Per car: the (group, member) whose placement gives the car's pose.
    anchors: Vec<(usize, usize)>,
    /// Per car (slot): the level shown, and the level whose loose parts are
    /// posed (the next shown).
    shown: Vec<u8>,
    posed: Vec<u8>,
    /// Opponent parts drawn on their own (wheels, glass): car, level, entity.
    extras: Vec<(usize, u8, Entity)>,
    /// Per car (slot): the level fading out while [`Self::shown`] fades in.
    fades: Vec<Option<LevelFade>>,
    /// The camera position of the last frame (cut detection).
    eye: Option<Vec3>,
    /// Cars with a [`ShadowBody`].
    shadow_cars: HashSet<usize>,
    /// Car-frames per level shown, level changes and time in [`refresh`],
    /// for the log.
    shown_frames: [u64; 6],
    level_changes: u64,
    refresh_ns: u64,
    frames: u64,
    rebuilds: u64,
    /// Rebuilds that rewrote only their changed members ([`patch`]).
    patched: u64,
    /// Batches written in place (only their changed parts' vertices).
    streamed: u64,
    /// Rebuilds for a deformed member mesh only, and the time of all.
    reshaped: u64,
    rebuild_ns: u64,
}

/// A car's level change in progress: `from` dithers out while the shown
/// level dithers in, `t` from 0 to 1; `step` is the dither step last set.
#[derive(Clone, Copy)]
struct LevelFade {
    from: u8,
    t: f32,
    step: u8,
}

type Candidate = (
    Entity,
    &'static Mesh3d,
    &'static MeshMaterial3d<StandardMaterial>,
    Option<&'static crate::OpponentPart>,
    Option<&'static crate::MovingPart>,
    Option<&'static crate::track_surface::SourcePart>,
    Option<&'static LodPart>,
    (
        Option<&'static RenderLayers>,
        Has<NotShadowCaster>,
        Has<VisibilityRange>,
        Has<ChildOf>,
        Option<&'static Children>,
        Has<crate::source_trees::Facing>,
        Has<crate::sidewall_filter::Sidewall>,
        Has<crate::cockpit::InteriorPart>,
    ),
);
type NewPart = Or<(
    bevy::ecs::query::Added<crate::OpponentPart>,
    bevy::ecs::query::Added<crate::MovingPart>,
)>;
type Placement = (
    &'static Batchable,
    &'static Transform,
    &'static Visibility,
    &'static Mesh3d,
    &'static MeshMaterial3d<StandardMaterial>,
    Option<&'static RenderLayers>,
    Has<NotShadowCaster>,
    Has<VisibilityRange>,
);
type Body = (
    Ref<'static, ShadowBody>,
    &'static mut Transform,
    &'static mut Visibility,
);
/// What the view draws of the player car.
type OwnCarView<'w> = (
    Option<Res<'w, crate::camera::Cameras>>,
    Option<Res<'w, crate::replay::Replay>>,
    Option<Res<'w, crate::cockpit::Cockpit>>,
);
/// The viewer camera, disjoint from the batch and body transforms.
type ViewerFilter = (With<crate::ViewerCamera>, Without<Batch>, Without<ShadowBody>);

#[allow(clippy::too_many_arguments)]
fn refresh(
    mut commands: Commands,
    mut state: ResMut<CarBatches>,
    added: Query<Candidate, NewPart>,
    overlays: Query<(), With<crate::car_reflection_overlay::OverlayOf>>,
    parts: Query<Placement, Without<Batch>>,
    mut batches: Query<(&mut Transform, &mut Visibility, &Mesh3d, &mut Aabb), With<Batch>>,
    mut bodies: Query<Body, (Without<Batch>, Without<Batchable>)>,
    camera: Query<(&Transform, &Projection), ViewerFilter>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<Assets<StandardMaterial>>,
    mut events: MessageReader<AssetEvent<Mesh>>,
    own: OwnCarView,
    time: Option<Res<Time>>,
    stream: Option<Res<crate::mesh_stream::MeshStream>>,
) {
    let state = &mut *state;
    // A view that hides some of the player car draws its parts on their own
    // (docs/CAMERA_VIEWS.md "Own car per view").
    let loose = crate::camera::own_car(own.0.as_deref(), own.1.as_deref(), own.2.is_some())
        != crate::camera::OwnCar::Whole;
    let loose_slot = state.groups.iter().find(|g| g.car == 0 && loose).map(|g| g.slot);
    let start = std::time::Instant::now();
    for (body, ..) in &bodies {
        if body.is_added() {
            state.shadow_cars.insert(body.car);
        }
    }
    // New parts take their place from the next frame (their marker is
    // inserted by command).
    if register(
        &mut commands,
        &added,
        &overlays,
        &meshes,
        &materials,
        &mut state.groups,
        &mut state.anchors,
        &state.shadow_cars,
        &mut state.extras,
    ) || state.groups.is_empty()
    {
        return;
    }
    let modified: HashSet<AssetId<Mesh>> = events
        .read()
        .filter_map(|e| match e {
            AssetEvent::Modified { id } => Some(*id),
            _ => None,
        })
        .collect();
    let hidden = RenderLayers::layer(HIDDEN_LAYER);
    // Each car's pose from its anchor part (its largest). When the anchor is
    // hidden, or most of the car's parts disagree with it (it came loose),
    // the car's batches hold this frame and the next part anchors.
    let anchors = &mut state.anchors;
    let poses: Vec<Option<Mat4>> = anchors
        .iter()
        .map(|&(g, m)| {
            let member = state.groups[g].members.get(m)?;
            let (_, transform, visibility, ..) = parts.get(member.entity).ok()?;
            (*visibility != Visibility::Hidden).then(|| transform.to_matrix() * member.rest_inverse)
        })
        .collect();
    for member in state.groups.iter_mut().flat_map(|g| &mut g.members) {
        member.placed = false;
        member.alive = false;
    }
    for (slot, transform, visibility, mesh, material, layers, shadowless, ranged) in &parts {
        let group = &mut state.groups[slot.group as usize];
        let pose = poses[group.slot];
        let member = &mut group.members[slot.member as usize];
        member.alive = true;
        // A batched part carries our layer and no shadow of its own.
        let ours = layers.is_some_and(|l| *l == hidden);
        member.placed = Some(group.slot) != loose_slot
            && *visibility != Visibility::Hidden
            && (ours || crate::car_reflections::default_layers(layers))
            && (!shadowless || ours || group.shadow_body)
            && !ranged
            && same(&material.0, &group.material)
            && same(&mesh.0, &member.mesh)
            // A merged low-detail part keeps no pose: it is on its car.
            && pose.is_some_and(|p| group.level > 0 || in_place(transform.to_matrix(), p * member.rest));
    }
    // Placement votes count the LOD0 parts (the low levels always follow).
    let mut votes = vec![[0usize; 2]; anchors.len()];
    for group in state.groups.iter().filter(|g| g.level == 0) {
        for member in &group.members {
            votes[group.slot][usize::from(member.placed)] += 1;
        }
    }
    let held: Vec<bool> = poses
        .iter()
        .zip(&votes)
        .enumerate()
        .map(|(slot, (pose, [out, placed]))| Some(slot) != loose_slot && (pose.is_none() || out > placed))
        .collect();
    state.shown.resize(anchors.len(), 0);
    state.posed.resize(anchors.len(), 0);
    state.fades.resize(anchors.len(), None);
    let eye = camera.single().ok().map(|(t, _)| t.translation);
    let cut = !crate::ornament_lod::fade_enabled()
        || state.eye.zip(eye).is_none_or(|(a, b)| a.distance(b) > crate::ornament_lod::CUT_METRES);
    state.eye = eye;
    // The level whose loose parts were posed last frame is shown now; the
    // other levels' loose parts stop being posed.
    for (slot, pose) in poses.iter().enumerate() {
        let level = state.posed[slot];
        let Some(pose) = pose.filter(|_| state.shown[slot] != level) else {
            continue;
        };
        let old = state.shown[slot];
        state.shown[slot] = level;
        state.level_changes += 1;
        // The change fades; a change during a fade drops that fade's old level.
        let from = (!cut).then_some(old);
        state.fades[slot] = from.map(|from| LevelFade { from, t: 0., step: u8::MAX });
        if crate::ornament_lod::trace_enabled()
            && let Ok((eye, Projection::Perspective(lens))) = camera.single()
        {
            // Research: where the car is on screen (`GRID_ORNAMENT_LOD_TRACE=1`).
            let p = eye.to_matrix().inverse().transform_point3(pose.w_axis.truncate());
            let half = (lens.fov * 0.5).tan();
            println!(
                "car_level_change frame={} slot={slot} from={old} to={level} faded={} distance={:.1} ndc={:.2},{:.2}",
                state.frames,
                from.is_some(),
                -p.z,
                p.x / (-p.z * half * lens.aspect_ratio),
                p.y / (-p.z * half)
            );
        }
        let car = state.groups.iter().find(|g| g.slot == slot).map(|g| g.car);
        for group in state.groups.iter().filter(|g| g.slot == slot) {
            for &batch in &group.batches {
                if Some(group.level) != from {
                    show(&mut commands, batch, group.level == level);
                    unrange(&mut commands, batch);
                }
                if group.level == level
                    && let Ok((mut transform, ..)) = batches.get_mut(batch)
                {
                    transform.set_if_neq(Transform::from_matrix(pose));
                }
            }
        }
        for &(_, part_level, entity) in state.extras.iter().filter(|e| Some(e.0) == car) {
            if Some(part_level) == from {
                continue;
            }
            show(&mut commands, entity, part_level == level);
            unrange(&mut commands, entity);
            if part_level != level
                && let Ok(mut entity) = commands.get_entity(entity)
            {
                entity.try_insert(Unposed);
            }
        }
    }
    // Fades in progress: the old and the shown level dither, complementary.
    let step = time.map_or(0., |t| t.delta_secs()) / crate::ornament_lod::FADE_SECONDS;
    for slot in 0..state.fades.len() {
        let Some(fade) = state.fades[slot].as_mut() else {
            continue;
        };
        fade.t = if cut { 1. } else { (fade.t + step).min(1.) };
        let (from, to, done) = (fade.from, state.shown[slot], fade.t >= 1.);
        let dither = (fade.t * 16.).round() as u8;
        if !done && dither == fade.step {
            continue;
        }
        fade.step = dither;
        let car = state.groups.iter().find(|g| g.slot == slot).map(|g| g.car);
        let batches_of = state
            .groups
            .iter()
            .filter(|g| g.slot == slot)
            .flat_map(|g| g.batches.iter().map(move |&b| (g.level, b, false)));
        let extras_of = state.extras.iter().filter(|e| Some(e.0) == car).map(|&(_, l, e)| (l, e, true));
        for (level, entity, extra) in batches_of.chain(extras_of) {
            let Ok(mut entity) = commands.get_entity(entity) else {
                continue;
            };
            if level == from && done {
                entity.try_insert(RenderLayers::layer(HIDDEN_LAYER));
                entity.try_remove::<VisibilityRange>();
                if extra {
                    entity.try_insert(Unposed);
                }
            } else if level == to && done {
                entity.try_remove::<VisibilityRange>();
            } else if level == from || level == to {
                entity.try_insert(crate::ornament_lod::fade_range(level == to, dither));
            }
        }
        if done {
            state.fades[slot] = None;
        }
    }
    let levels = select_levels(&state.groups, anchors.len(), &poses, &held, &state.posed, camera.single().ok());
    for (slot, &level) in levels.iter().enumerate() {
        state.shown_frames[usize::from(state.shown[slot]).min(5)] += 1;
        if state.posed[slot] == level {
            continue;
        }
        // Pose the new level's loose parts this frame; it shows next frame.
        state.posed[slot] = level;
        let car = state.groups.iter().find(|g| g.slot == slot).map(|g| g.car);
        for &(_, part_level, entity) in state.extras.iter().filter(|e| Some(e.0) == car) {
            if part_level == level
                && let Ok(mut entity) = commands.get_entity(entity)
            {
                entity.try_remove::<Unposed>();
            }
        }
    }
    // The batches' transform, once per car.
    let transforms: Vec<Option<Transform>> = poses.iter().map(|p| p.map(Transform::from_matrix)).collect();
    let mut stats = [0usize; 2]; // members, batched
    for group in &mut state.groups {
        stats[0] += group.members.len();
        if held[group.slot] {
            // A car whose parts are all gone (its scene ended) drops its batches.
            if group.members.iter().all(|m| !m.alive) {
                for batch in group.batches.drain(..) {
                    commands.entity(batch).despawn();
                }
                group.members.clear();
                group.built = None;
                continue;
            }
            stats[1] += group.members.iter().filter(|m| m.batched).count();
            // A deformed member rebuilds once the car is placed again.
            for member in &mut group.members {
                if modified.contains(&member.id) {
                    member.stale = true;
                    group.dirty |= member.batched;
                }
            }
            continue;
        }
        let mut reshaped = false;
        for member in &mut group.members {
            member.stable = if member.placed {
                member.stable.saturating_add(1)
            } else {
                0
            };
            let batched = member.placed && (member.batched || member.stable >= REJOIN_FRAMES);
            let deformed = modified.contains(&member.id);
            member.stale |= deformed || batched != member.batched;
            if batched != member.batched {
                member.batched = batched;
                group.dirty = true;
                if batched {
                    commands
                        .entity(member.entity)
                        .insert((hidden.clone(), NotShadowCaster));
                } else if group.level > 0 {
                    // A low-detail part is never drawn on its own.
                } else if parts.get(member.entity).is_ok_and(|p| p.5 == Some(&hidden)) {
                    // A car with a shadow body keeps its parts shadowless.
                    if group.shadow_body {
                        commands.entity(member.entity).remove::<RenderLayers>();
                    } else {
                        commands
                            .entity(member.entity)
                            .remove::<(RenderLayers, NotShadowCaster)>();
                    }
                }
            } else if batched && deformed {
                reshaped |= !group.dirty;
                group.dirty = true;
            }
            stats[1] += usize::from(member.batched);
        }
        let pose = poses[group.slot];
        let fading = state.fades.get(group.slot).copied().flatten().is_some_and(|f| f.from == group.level);
        let wanted = drawn_soon(group, &state.shown, &state.posed, fading);
        if wanted && std::mem::take(&mut group.dirty) {
            state.rebuilds += 1;
            state.reshaped += u64::from(reshaped);
            let start = std::time::Instant::now();
            let shown = state.shown.get(group.slot).copied().unwrap_or(0) == group.level;
            let (streamed, patched) =
                rebuild(&mut commands, group, pose, shown, &mut meshes, &mut batches, stream.as_deref());
            state.streamed += streamed;
            state.patched += u64::from(patched);
            state.rebuild_ns += start.elapsed().as_nanos() as u64;
        }
        // Batches of a level the car does not show keep their pose; a level
        // fading out still follows the car.
        if let Some(next) = transforms[group.slot].filter(|_| state.shown[group.slot] == group.level || fading) {
            for &batch in &group.batches {
                if let Ok((mut transform, ..)) = batches.get_mut(batch) {
                    transform.set_if_neq(next);
                }
            }
        }
    }
    // Shadow bodies follow their car while any of its parts is in place.
    for (body, mut transform, mut visibility) in &mut bodies {
        let Some(slot) = state.groups.iter().find(|g| g.car == body.car).map(|g| g.slot) else {
            continue;
        };
        if let Some(pose) = poses[slot] {
            transform.set_if_neq(Transform::from_matrix(pose));
        }
        visibility.set_if_neq(if votes[slot][1] > 0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        });
    }
    // The next anchor of a held car: its next part, in registration order.
    for (slot, _) in held.iter().enumerate().filter(|(_, h)| **h) {
        let (g, m) = anchors[slot];
        let car = state.groups[g].car;
        anchors[slot] = if m + 1 < state.groups[g].members.len() {
            (g, m + 1)
        } else {
            let next = (1..=state.groups.len())
                .map(|k| (g + k) % state.groups.len())
                .find(|&k| state.groups[k].car == car)
                .unwrap_or(g);
            (next, 0)
        };
    }
    state.frames += 1;
    state.refresh_ns += start.elapsed().as_nanos() as u64;
    if state.frames == 1 || state.frames.is_multiple_of(REPORT_FRAMES) {
        let spawned: usize = state.groups.iter().map(|g| g.batches.len()).sum();
        println!(
            "car_batch version=2 frame={} cars={} groups={} parts={} batched={} batches={spawned} extras={} rebuilds={} patched={} streamed={} reshaped={} rebuild_ms={:.1} refresh_ms={:.1} level_changes={} level_car_frames={:?}",
            state.frames,
            state.anchors.len(),
            state.groups.len(),
            stats[0],
            stats[1],
            state.extras.len(),
            state.rebuilds,
            state.patched,
            state.streamed,
            state.reshaped,
            state.rebuild_ns as f64 / 1e6,
            state.refresh_ns as f64 / 1e6,
            state.level_changes,
            state.shown_frames,
        );
    }
}

/// New car parts that can share a mesh: no wheel (it turns on the car), an
/// opaque or masked material, a supported indexed triangle list, and no
/// per-entity render setting.
#[allow(clippy::too_many_arguments)]
fn register(
    commands: &mut Commands,
    added: &Query<Candidate, NewPart>,
    overlays: &Query<(), With<crate::car_reflection_overlay::OverlayOf>>,
    meshes: &Assets<Mesh>,
    materials: &Assets<StandardMaterial>,
    groups: &mut Vec<Group>,
    anchors: &mut Vec<(usize, usize)>,
    shadow_cars: &HashSet<usize>,
    extras: &mut Vec<(usize, u8, Entity)>,
) -> bool {
    let before = groups.iter().map(|g| g.members.len()).sum::<usize>();
    for (entity, mesh, material, opponent, moving, source, lod, special) in added {
        // Player-only view/probe policies retain these exact source entities.
        // A model prefix alone does not identify an opponent as the player.
        if opponent.is_none() && source.is_some_and(|p| p.0.starts_with("car0/")) {
            continue;
        }
        let (car, rest, wheel) = match (opponent, moving) {
            (Some(o), _) => (o.car, o.rest, o.wheel),
            (_, Some(m)) => (0, m.rest, m.wheel),
            _ => continue,
        };
        let (layers, shadowless, ranged, child, children, facing, sidewall, interior) = special;
        // The reflection layer and overlays do not set a part apart.
        let layers = !crate::car_reflections::default_layers(layers);
        let parent = children.is_some_and(|c| c.iter().any(|e| !overlays.contains(e)));
        // A car with a shadow body casts no shadow from its parts.
        let shadow_body = opponent.is_some() && shadow_cars.contains(&car);
        if shadow_body && !shadowless {
            commands.entity(entity).insert(NotShadowCaster);
        }
        // Low-detail parts arrive on our layer and without a shadow.
        let level = lod.map_or(0, |l| l.level);
        // Hidden until its level is chosen, merged or not.
        if level > 0 {
            commands.entity(entity).insert(Unposed);
        }
        let own_settings = level == 0 && (layers || shadowless);
        // An opponent part drawn on its own follows its car's level.
        let extra = |extras: &mut Vec<(usize, u8, Entity)>| {
            if opponent.is_some() && (level > 0 || !layers) {
                extras.push((car, level, entity));
            }
        };
        if wheel.is_some() || own_settings || ranged || child || parent || facing || sidewall || interior {
            extra(extras);
            continue;
        }
        if !materials
            .get(&material.0)
            .is_some_and(|m| matches!(m.alpha_mode, AlphaMode::Opaque | AlphaMode::Mask(_)))
        {
            extra(extras);
            continue;
        }
        let Some(source) = meshes.get(&mesh.0) else {
            extra(extras);
            continue;
        };
        let layout: Vec<_> = source
            .attributes()
            .map(|(a, v)| (a.id, VertexFormat::from(v)))
            .collect();
        let vertices = source.count_vertices();
        if source.primitive_topology() != PrimitiveTopology::TriangleList
            || source.indices().is_none()
            || vertices >= MAX_VERTICES
            || !layout
                .iter()
                .all(|(a, f)| ATTRIBUTES.contains(a) && supported(*f))
            || rest.determinant().abs() < 1e-12
        {
            extra(extras);
            continue;
        }
        let member = Member {
            entity,
            rest,
            rest_inverse: rest.inverse(),
            mesh: mesh.0.clone(),
            id: mesh.0.id(),
            vertices,
            batched: false,
            placed: false,
            alive: true,
            stable: REJOIN_FRAMES,
            stale: false,
        };
        let found = groups.iter().position(|g| {
            g.car == car && g.level == level && g.material == material.0 && g.layout == layout
        });
        let index = found.unwrap_or(groups.len());
        let place = Batchable {
            group: index as u32,
            member: found.map_or(0, |g| groups[g].members.len()) as u32,
        };
        // Its bounds follow its mesh where the presentation changes it
        // (deformation), so Bevy's per-frame bounds pass can skip it.
        commands.entity(entity).insert((place, NoAutoAabb));
        match found.map(|g| &mut groups[g]) {
            Some(group) => group.members.push(member),
            None => {
                let slot = match groups.iter().find(|g| g.car == car) {
                    Some(g) => g.slot,
                    None => {
                        anchors.push((groups.len(), 0));
                        anchors.len() - 1
                    }
                };
                groups.push(Group {
                    car,
                    slot,
                    material: material.0.clone(),
                    layout,
                    members: vec![member],
                    batches: Vec::new(),
                    dirty: true,
                    built: None,
                    shadow_body,
                    level,
                });
            }
        }
    }
    if groups.iter().map(|g| g.members.len()).sum::<usize>() == before {
        return false;
    }
    for (slot, anchor) in anchors.iter_mut().enumerate() {
        let largest = groups
            .iter()
            .enumerate()
            .filter(|(_, g)| g.slot == slot && g.level == 0)
            .flat_map(|(g, group)| {
                group
                    .members
                    .iter()
                    .enumerate()
                    .map(move |(m, x)| (x.vertices, g, m))
            })
            .max_by_key(|&(vertices, g, m)| (vertices, std::cmp::Reverse((g, m))));
        if let Some((_, g, m)) = largest {
            *anchor = (g, m);
        }
    }
    true
}

/// Draws `entity` (no layer: the default one) or moves it to the hidden
/// layer. The entity may be gone with its scene.
/// Ends a level fade on `entity` (`ornament_lod::respecialize_ranged`
/// re-specializes its pipeline).
fn unrange(commands: &mut Commands, entity: Entity) {
    if let Ok(mut entity) = commands.get_entity(entity) {
        entity.try_remove::<VisibilityRange>();
    }
}

fn show(commands: &mut Commands, entity: Entity, visible: bool) {
    let Ok(mut entity) = commands.get_entity(entity) else {
        return;
    };
    if visible {
        entity.try_remove::<RenderLayers>();
    } else {
        entity.try_insert(RenderLayers::layer(HIDDEN_LAYER));
    }
}

/// Each car's level this frame (docs/CAR_LOD.md "Selection"): opponents with
/// low levels whose LOD0 parts are all batched are served by
/// FOV-scaled distance to the camera under the set's caps; every other car
/// shows LOD0, and a held car keeps its level.
fn select_levels(
    groups: &[Group],
    cars: usize,
    poses: &[Option<Mat4>],
    held: &[bool],
    shown: &[u8],
    camera: Option<(&Transform, &Projection)>,
) -> Vec<u8> {
    let mut levels: Vec<u8> = (0..cars).map(|s| shown.get(s).copied().unwrap_or(0)).collect();
    let (Some(setting), Some((eye, Projection::Perspective(lens)))) = (crate::car_lod::setting(), camera)
    else {
        levels.fill(0);
        return levels;
    };
    let view = eye.to_matrix().inverse();
    let half = (lens.fov * 0.5).tan();
    let side = half * lens.aspect_ratio;
    let mut requests = Vec::new();
    let mut slots = Vec::new();
    for slot in 0..cars {
        let (mut finest, mut coarsest) = (u8::MAX, 0);
        let mut intact = true;
        for group in groups.iter().filter(|g| g.slot == slot) {
            if group.level > 0 {
                finest = finest.min(group.level);
                coarsest = coarsest.max(group.level);
            } else if group.shadow_body {
                intact &= group.members.iter().all(|m| m.batched);
            }
        }
        if held[slot] {
            continue;
        }
        let Some(pose) = poses[slot].filter(|_| coarsest > 0 && intact) else {
            levels[slot] = 0;
            continue;
        };
        // Box corners in view space: off to the side, or all behind or beyond
        // the sides (culled, sorted last).
        let mut outside = 0;
        let mut culled = true;
        for i in 0..8 {
            let sign = Vec3::new(
                if i & 1 == 0 { -1. } else { 1. },
                if i & 2 == 0 { -1. } else { 1. },
                if i & 4 == 0 { -1. } else { 1. },
            );
            let corner = pose.transform_point3(Vec3::Y * CAR_BOX_CENTRE_Y + sign * CAR_BOX);
            let p = view.transform_point3(corner);
            let beside = p.z >= 0. || p.x.abs() > -p.z * side;
            outside += usize::from(beside);
            culled &= beside || p.y.abs() > -p.z * half;
        }
        let metres = if culled {
            crate::car_lod::CULLED_METRES
        } else {
            crate::car_lod::scaled(pose.w_axis.truncate().distance(eye.translation), lens.fov)
        };
        requests.push(crate::car_lod::Request {
            metres,
            key: metres * crate::car_lod::offscreen_weight(outside),
            finest: usize::from(finest),
            coarsest: usize::from(coarsest),
        });
        slots.push(slot);
    }
    for (slot, level) in slots.into_iter().zip(crate::car_lod::select(setting, &requests)) {
        levels[slot] = level as u8;
    }
    levels
}

/// Whether a group's batches are drawn this frame or the next: its level is
/// shown, fading out, or posed (it shows next frame). Only such a group
/// rebuilds when its parts change; another level keeps its batches until it
/// is posed, as a dent would otherwise rebuild and upload every low level of
/// the car each time. A group without batches yet builds them at once.
fn drawn_soon(group: &Group, shown: &[u8], posed: &[u8], fading: bool) -> bool {
    group.batches.is_empty()
        || shown.get(group.slot) == Some(&group.level)
        || posed.get(group.slot) == Some(&group.level)
        || fading
}

/// The group's parts in batches of at most [`MAX_VERTICES`], in spawn
/// order; unused batches are hidden. A part out of its batch keeps its
/// vertices there, collapsed ([`collapse`]), so a part leaving or rejoining
/// changes no batch's layout and streams only its own vertices instead of
/// uploading the whole batch again. A batch with no part in it is hidden.
/// New batches of a level the car does not show start on the hidden layer.
/// When the batches stand as the last full rebuild left them, only the
/// changed members' vertices are rewritten ([`patch`]), to the same bytes.
/// Returns the number of batches streamed in place and whether it patched.
fn rebuild(
    commands: &mut Commands,
    group: &mut Group,
    pose: Option<Mat4>,
    shown: bool,
    meshes: &mut Assets<Mesh>,
    batches: &mut Query<(&mut Transform, &mut Visibility, &Mesh3d, &mut Aabb), With<Batch>>,
    stream: Option<&crate::mesh_stream::MeshStream>,
) -> (u64, bool) {
    if patch_enabled()
        && let Some(streamed) = patch(group, meshes, batches, stream)
    {
        for member in &mut group.members {
            member.stale = false;
        }
        return (streamed, true);
    }
    let mut streamed = 0;
    let mut slots = Vec::with_capacity(group.members.len());
    let mut chunks: Vec<Vec<Part<StandardMaterial>>> = Vec::new();
    let mut kept: Vec<Vec<bool>> = Vec::new();
    let mut count = MAX_VERTICES;
    for member in &group.members {
        if count + member.vertices > MAX_VERTICES {
            chunks.push(Vec::new());
            kept.push(Vec::new());
            count = 0;
        }
        count += member.vertices;
        kept.last_mut().expect("chunk").push(member.batched);
        chunks.last_mut().expect("chunk").push(Part {
            entity: member.entity,
            material: group.material.clone(),
            intact: false,
            mesh: member.mesh.clone(),
            matrix: member.rest,
            vertices: member.vertices,
        });
    }
    for (i, (chunk, kept)) in chunks.iter().zip(&kept).enumerate() {
        let (mut mesh, _, _) = build(chunk, meshes, Vec3::ZERO, &group.layout);
        let parts: Vec<usize> = chunk.iter().map(|p| p.vertices).collect();
        collapse(&mut mesh, &parts, kept);
        if let Some(VertexAttributeValues::Float32x3(positions)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
            let mut first = 0;
            for &count in &parts {
                let (min, max) = bounds(&positions[first..first + count]);
                slots.push(Slot { batch: i, first, min, max });
                first += count;
            }
        }
        let used = if kept.contains(&true) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        let bounds = mesh.compute_aabb().unwrap_or_default();
        match group.batches.get(i).and_then(|&e| batches.get_mut(e).ok()) {
            Some((_, mut visibility, handle, mut aabb)) => {
                // Dents move vertices, not the batch's structure: the changed
                // parts' vertices are written over the batch's buffer and the
                // asset is kept current without being prepared again.
                let written = stream.zip(meshes.get(&handle.0)).and_then(|(stream, old)| {
                    crate::mesh_stream::stream_changed(old, &mesh, &parts, handle.0.id(), stream)
                });
                if written.is_some() {
                    streamed += 1;
                    if let Some(target) = meshes.get_mut_untracked(&handle.0) {
                        *target = mesh;
                    }
                } else if let Some(mut target) = meshes.get_mut(&handle.0) {
                    *target = mesh;
                }
                *aabb = bounds;
                visibility.set_if_neq(used);
            }
            None => {
                let mut entity = commands.spawn((
                    Name::new(format!("car_batch/{}/{i}", group.car)),
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(group.material.clone()),
                    pose.map_or_else(Transform::default, Transform::from_matrix),
                    bounds,
                    Batch { car: group.car },
                    NoAutoAabb,
                    used,
                ));
                if group.shadow_body {
                    entity.insert(NotShadowCaster);
                }
                if !shown {
                    entity.insert(RenderLayers::layer(HIDDEN_LAYER));
                }
                let entity = entity.id();
                group.batches.push(entity);
            }
        }
    }
    for &unused in &group.batches[chunks.len().min(group.batches.len())..] {
        if let Ok((_, mut visibility, ..)) = batches.get_mut(unused) {
            visibility.set_if_neq(Visibility::Hidden);
        }
    }
    group.built = (slots.len() == group.members.len()).then_some(Built { slots, batches: chunks.len() });
    for member in &mut group.members {
        member.stale = false;
    }
    (streamed, false)
}

/// Whether a rebuild may rewrite only its changed members (`GRID_BATCH_PATCH=0`
/// rebuilds every batch whole, for comparisons).
fn patch_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| crate::dev::var("GRID_BATCH_PATCH").map_or(true, |v| v != "0"))
}

/// Smallest and largest corner of `positions` (as `Aabb::enclosing`); no
/// positions give an empty box (min above max) that merges as nothing.
fn bounds(positions: &[[f32; 3]]) -> (Vec3, Vec3) {
    positions
        .iter()
        .map(|p| Vec3::from(*p))
        .fold((Vec3::INFINITY, Vec3::NEG_INFINITY), |(min, max), v| (min.min(v), max.max(v)))
}

/// A rebuild that rewrites only the stale members' vertices, and every
/// member out of its batch (a part off the batch may have been dented
/// without an event, mesh_stream::replace), with the same bytes as the full
/// path of [`rebuild`]: the same placement (`static_batch::place`), collapse
/// and bounds. A dent moves vertices and keeps the part's topology, so the
/// batches keep their indices. Vertices that differ are streamed in place
/// when the batch is uploaded, else the asset is changed. `None` (nothing
/// written): the group changed shape since its last full rebuild (a member
/// added, a batch or a source mesh not as built), so it rebuilds whole.
fn patch(
    group: &mut Group,
    meshes: &mut Assets<Mesh>,
    batches: &mut Query<(&mut Transform, &mut Visibility, &Mesh3d, &mut Aabb), With<Batch>>,
    stream: Option<&crate::mesh_stream::MeshStream>,
) -> Option<u64> {
    let built = group.built.as_mut()?;
    if built.slots.len() != group.members.len() || group.batches.len() < built.batches {
        return None;
    }
    let position = group.layout.iter().position(|(id, _)| *id == Mesh::ATTRIBUTE_POSITION.id)?;
    let mut handles = Vec::with_capacity(built.batches);
    for (i, &entity) in group.batches[..built.batches].iter().enumerate() {
        let handle = batches.get(entity).ok()?.2.0.clone();
        let size: usize = built
            .slots
            .iter()
            .zip(&group.members)
            .filter(|(s, _)| s.batch == i)
            .map(|(_, m)| m.vertices)
            .sum();
        if meshes.get(&handle).is_none_or(|m| m.try_attributes().is_err() || m.count_vertices() != size) {
            return None;
        }
        handles.push(handle);
    }
    // Placed first: the sources are read before a batch is written.
    let mut writes = Vec::new();
    for (m, member) in group.members.iter().enumerate() {
        if !member.stale && member.batched {
            continue;
        }
        let source = meshes.get(&member.mesh)?;
        if source.count_vertices() != member.vertices {
            return None;
        }
        let mut values = crate::static_batch::place(member.rest, source, Vec3::ZERO, &group.layout);
        if !member.batched
            && let VertexAttributeValues::Float32x3(positions) = &mut values[position]
            && let Some(&at) = positions.first()
        {
            positions.fill(at);
        }
        writes.push((m, values));
    }
    let mut streamed = 0;
    for (i, handle) in handles.iter().enumerate() {
        let streaming = stream.filter(|_| meshes.get(handle).is_some_and(crate::frame_split::uploaded));
        let target = if streaming.is_some() {
            meshes.get_mut_untracked(handle)?
        } else {
            meshes.get_mut(handle)?.into_inner()
        };
        let mut changed: Vec<std::ops::Range<usize>> = Vec::new();
        for (m, values) in &writes {
            if built.slots[*m].batch != i {
                continue;
            }
            let slot = &mut built.slots[*m];
            let count = group.members[*m].vertices;
            let mut differs = false;
            for ((id, _), new) in group.layout.iter().zip(values) {
                let Some(old) = target.attribute_mut(*id) else {
                    continue;
                };
                differs |= write_range(old, new, slot.first, count);
            }
            if differs {
                if let VertexAttributeValues::Float32x3(positions) = &values[position] {
                    (slot.min, slot.max) = bounds(positions);
                }
                changed.push(slot.first..slot.first + count);
            }
        }
        let members = built.slots.iter().zip(&group.members).filter(|(s, _)| s.batch == i);
        let used = if members.clone().any(|(_, m)| m.batched) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        let (min, max) = members.fold((Vec3::INFINITY, Vec3::NEG_INFINITY), |(min, max), (s, _)| {
            (min.min(s.min), max.max(s.max))
        });
        // As `compute_aabb().unwrap_or_default()` of the whole batch.
        let bounds = if min.cmple(max).all() { Aabb::from_min_max(min, max) } else { Aabb::default() };
        if let Ok((_, mut visibility, _, mut aabb)) = batches.get_mut(group.batches[i]) {
            *aabb = bounds;
            visibility.set_if_neq(used);
        }
        let Some(stream) = streaming else { continue };
        if changed.is_empty() {
            continue;
        }
        changed.sort_by_key(|r| r.start);
        let mut runs: Vec<std::ops::Range<usize>> = Vec::new();
        for range in changed {
            match runs.last_mut() {
                Some(last) if last.end == range.start => last.end = range.end,
                _ => runs.push(range),
            }
        }
        let target = meshes.get(handle)?;
        let stride = crate::mesh_stream::VertexLayout::of(target).stride;
        for run in runs {
            stream.write_at(handle.id(), run.start as u32, stride, |bytes| {
                crate::mesh_stream::pack_vertices(target, run.clone(), bytes);
            });
        }
        streamed += 1;
    }
    Some(streamed)
}

/// Writes `new` over vertices `first..first + count` of `old` where their
/// bytes differ; returns whether they did.
fn write_range(old: &mut VertexAttributeValues, new: &VertexAttributeValues, first: usize, count: usize) -> bool {
    fn put<const N: usize>(old: &mut [[f32; N]], new: &[[f32; N]]) -> bool {
        let same = old.iter().flatten().map(|x| x.to_bits()).eq(new.iter().flatten().map(|x| x.to_bits()));
        if !same {
            old.copy_from_slice(new);
        }
        !same
    }
    let span = first..first + count;
    match (old, new) {
        (VertexAttributeValues::Float32x2(o), VertexAttributeValues::Float32x2(n)) => put(&mut o[span], n),
        (VertexAttributeValues::Float32x3(o), VertexAttributeValues::Float32x3(n)) => put(&mut o[span], n),
        (VertexAttributeValues::Float32x4(o), VertexAttributeValues::Float32x4(n)) => put(&mut o[span], n),
        _ => unreachable!("layout checked by register"),
    }
}

/// Moves each vertex of a part out of its batch (`kept` false) onto the
/// part's first vertex: its triangles have no area and draw nothing, and the
/// batch keeps the same vertices and indices. `parts`: vertices per part.
fn collapse(mesh: &mut Mesh, parts: &[usize], kept: &[bool]) {
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_POSITION)
    else {
        return;
    };
    let mut first = 0;
    for (&count, &kept) in parts.iter().zip(kept) {
        if !kept && count > 0 {
            let at = positions[first];
            positions[first..first + count].fill(at);
        }
        first += count;
    }
}

/// Handle equality that compares two strong handles by pointer first. A
/// part's handle and its member's are clones of one `Arc`; reading each id
/// follows the handle into the heap, for every part every frame.
fn same<A: Asset>(a: &Handle<A>, b: &Handle<A>) -> bool {
    matches!((a, b), (Handle::Strong(x), Handle::Strong(y)) if std::sync::Arc::ptr_eq(x, y)) || a == b
}

fn in_place(matrix: Mat4, expected: Mat4) -> bool {
    (matrix.w_axis - expected.w_axis).abs().max_element() <= PLACE_METRES
        && [
            matrix.x_axis - expected.x_axis,
            matrix.y_axis - expected.y_axis,
            matrix.z_axis - expected.z_axis,
        ]
        .iter()
        .all(|d| d.abs().max_element() <= PLACE_AXES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_compare_like_their_ids() {
        use bevy::asset::RenderAssetUsages;
        let mut meshes = Assets::<Mesh>::default();
        let empty = || Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        let (a, b) = (meshes.add(empty()), meshes.add(empty()));
        let weak = Handle::<Mesh>::default();
        for (x, y) in [(&a, &a.clone()), (&a, &b), (&b, &a), (&a, &weak), (&weak, &weak.clone())] {
            assert_eq!(same(x, y), x.id() == y.id());
        }
    }

    #[test]
    fn a_dent_streams_only_the_changed_parts_vertices() {
        use crate::mesh_stream::stream_changed;
        use bevy::{asset::RenderAssetUsages, mesh::Indices};
        let mesh = |dent: f32, indices: Vec<u16>| {
            let mut m = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
            let p: Vec<[f32; 3]> = (0..12)
                .map(|i| [i as f32, if (4..8).contains(&i) { dent } else { 0. }, 1.])
                .collect();
            m.insert_attribute(Mesh::ATTRIBUTE_POSITION, p);
            m.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 1., 0.]; 12]);
            m.insert_attribute(Mesh::ATTRIBUTE_UV_0, (0..12).map(|i| [i as f32 * 0.1, 0.]).collect::<Vec<_>>());
            m.insert_indices(Indices::U16(indices));
            m
        };
        let tris: Vec<u16> = vec![0, 1, 2, 4, 5, 6, 8, 9, 10];
        let (old, new) = (mesh(0., tris.clone()), mesh(-0.25, tris.clone()));
        let stream = crate::mesh_stream::MeshStream::default();
        let id = AssetId::<Mesh>::default();
        // Parts of 4 vertices each; only the middle one moved.
        assert_eq!(stream_changed(&old, &new, &[4, 4, 4], id, &stream), Some(4));
        let stride = crate::mesh_stream::VertexLayout::of(&new).stride;
        let packed = new.create_packed_vertex_buffer_data();
        assert_eq!(stream.queued(id).as_deref(), Some(&packed[4 * stride..8 * stride]));
        // Unchanged: nothing written; another structure: replaced instead.
        let quiet = crate::mesh_stream::MeshStream::default();
        assert_eq!(stream_changed(&new, &new, &[4, 4, 4], id, &quiet), Some(0));
        assert_eq!(quiet.queued(id), None);
        let other = mesh(-0.25, vec![0, 2, 1, 4, 5, 6, 8, 9, 10]);
        assert_eq!(stream_changed(&old, &other, &[4, 4, 4], id, &stream), None);
    }

    #[test]
    fn a_part_leaving_its_batch_streams_only_its_own_vertices() {
        use crate::mesh_stream::stream_changed;
        use bevy::{asset::RenderAssetUsages, mesh::Indices};
        let mut whole = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        let p: Vec<[f32; 3]> = (0..12).map(|i| [i as f32, (i * i) as f32, 1.]).collect();
        whole.insert_attribute(Mesh::ATTRIBUTE_POSITION, p);
        whole.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 1., 0.]; 12]);
        whole.insert_indices(Indices::U16(vec![0, 1, 2, 4, 5, 6, 8, 9, 10]));
        let mut left = whole.clone();
        collapse(&mut left, &[4, 4, 4], &[true, false, true]);
        // The middle part's triangle has no area; the others are untouched.
        let Some(VertexAttributeValues::Float32x3(moved)) = left.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("positions");
        };
        assert!(moved[4..8].iter().all(|v| *v == [4., 16., 1.]));
        assert_eq!(moved[8], [8., 64., 1.]);
        let stream = crate::mesh_stream::MeshStream::default();
        let id = AssetId::<Mesh>::default();
        assert_eq!(stream_changed(&whole, &left, &[4, 4, 4], id, &stream), Some(4));
        let stride = crate::mesh_stream::VertexLayout::of(&left).stride;
        let packed = left.create_packed_vertex_buffer_data();
        assert_eq!(stream.queued(id).as_deref(), Some(&packed[4 * stride..8 * stride]));
    }

    /// A dent and a part leaving rewrite only their members' vertices, to the
    /// bytes and bounds of a full rebuild, streamed or written to the asset.
    #[test]
    fn a_patched_batch_matches_a_full_rebuild() {
        use bevy::{asset::RenderAssetUsages, mesh::Indices};
        let part_mesh = |k: f32| {
            Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                .with_inserted_attribute(
                    Mesh::ATTRIBUTE_POSITION,
                    vec![[0., 0., 0.], [1., 0., k], [0., k, 1.], [1., 1., 1.]],
                )
                .with_inserted_attribute(
                    Mesh::ATTRIBUTE_NORMAL,
                    vec![[0., 1., 0.], [0., 0.6, 0.8], [1., 0., 0.], [0., 0., 1.]],
                )
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0., 0.], [1., 0.], [0., 1.], [k, 1.]])
                .with_inserted_indices(Indices::U16(vec![0, 2, 1, 1, 2, 3]))
        };
        // The batch as a full rebuild of the current sources makes it.
        let expected = |app: &App| {
            let group = &app.world().resource::<CarBatches>().groups[0];
            let chunk: Vec<Part<StandardMaterial>> = group
                .members
                .iter()
                .map(|m| Part {
                    entity: m.entity,
                    material: group.material.clone(),
                    intact: false,
                    mesh: m.mesh.clone(),
                    matrix: m.rest,
                    vertices: m.vertices,
                })
                .collect();
            let (mut mesh, _, _) = build(&chunk, app.world().resource::<Assets<Mesh>>(), Vec3::ZERO, &group.layout);
            let kept: Vec<bool> = group.members.iter().map(|m| m.batched).collect();
            collapse(&mut mesh, &[4, 4, 4], &kept);
            mesh
        };
        let check = |app: &App, member: usize, streamed: bool| {
            let want = expected(app);
            let state = app.world().resource::<CarBatches>();
            let batch = state.groups[0].batches[0];
            let handle = &app.world().get::<Mesh3d>(batch).unwrap().0;
            let got = app.world().resource::<Assets<Mesh>>().get(handle).unwrap();
            for ((a, x), (b, y)) in want.attributes().zip(got.attributes()) {
                assert_eq!(a.id, b.id);
                assert_eq!(x.get_bytes(), y.get_bytes(), "{:?}", a.name);
            }
            assert_eq!(want.indices(), got.indices());
            let (aabb, bounds) = (app.world().get::<Aabb>(batch).unwrap(), want.compute_aabb().unwrap());
            assert_eq!((aabb.center, aabb.half_extents), (bounds.center, bounds.half_extents));
            if streamed {
                // Only the changed member's vertices were written.
                let stream = app.world().resource::<crate::mesh_stream::MeshStream>();
                let stride = crate::mesh_stream::VertexLayout::of(got).stride;
                let packed = got.create_packed_vertex_buffer_data();
                assert_eq!(
                    stream.queued(handle.id()).as_deref(),
                    Some(&packed[4 * member * stride..4 * (member + 1) * stride])
                );
            }
        };
        for streamed in [false, true] {
            let mut app = App::new();
            app.init_resource::<Assets<Mesh>>()
                .init_resource::<Assets<StandardMaterial>>()
                .init_resource::<CarBatches>()
                .add_message::<AssetEvent<Mesh>>()
                .add_systems(PostUpdate, refresh);
            if streamed {
                app.init_resource::<crate::mesh_stream::MeshStream>();
            }
            let material = app
                .world_mut()
                .resource_mut::<Assets<StandardMaterial>>()
                .add(StandardMaterial::default());
            for k in 0..3 {
                let mesh = app.world_mut().resource_mut::<Assets<Mesh>>().add(part_mesh(k as f32 + 1.));
                let rest = Mat4::from_rotation_translation(
                    Quat::from_rotation_y(0.3 * k as f32),
                    Vec3::new(k as f32 * 2., 0.1, 0.),
                );
                app.world_mut().spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(material.clone()),
                    Transform::from_matrix(rest),
                    Visibility::Inherited,
                    Aabb::from_min_max(Vec3::ZERO, Vec3::ONE),
                    crate::OpponentPart { car: 1, rest, wheel: None, lost_with_wheel: false },
                ));
            }
            // Registers, builds whole, stays.
            for _ in 0..3 {
                app.update();
            }
            let (anchor, members) = {
                let state = app.world().resource::<CarBatches>();
                assert_eq!((state.rebuilds, state.patched), (1, 0));
                let group = &state.groups[0];
                assert!(group.members.iter().all(|m| m.batched));
                let members: Vec<(Entity, Handle<Mesh>)> =
                    group.members.iter().map(|m| (m.entity, m.mesh.clone())).collect();
                (state.anchors[0].1, members)
            };
            check(&app, 0, false);
            // A dent: only that member is rewritten.
            let dented = (anchor + 1) % 3;
            *app.world_mut().resource_mut::<Assets<Mesh>>().get_mut(&members[dented].1).unwrap() = part_mesh(2.5);
            app.world_mut().write_message(AssetEvent::<Mesh>::Modified { id: members[dented].1.id() });
            app.update();
            assert_eq!(app.world().resource::<CarBatches>().patched, 1);
            check(&app, dented, streamed);
            // A part comes loose: its vertices collapse; the bounds shrink.
            let loose = (anchor + 2) % 3;
            app.world_mut().get_mut::<Transform>(members[loose].0).unwrap().translation.y += 1.;
            app.update();
            let state = app.world().resource::<CarBatches>();
            assert_eq!((state.rebuilds, state.patched), (3, 2));
            assert!(!state.groups[0].members[loose].batched);
            check(&app, loose, streamed);
        }
    }

    #[test]
    fn a_hidden_level_keeps_its_batches_until_it_is_posed() {
        let group = |level, built| Group {
            car: 0,
            slot: 0,
            material: Handle::default(),
            layout: Vec::new(),
            members: Vec::new(),
            batches: if built { vec![Entity::PLACEHOLDER] } else { Vec::new() },
            dirty: true,
            built: None,
            shadow_body: false,
            level,
        };
        // Level 0 shown, 3 hidden: only the shown level rebuilds on a dent.
        assert!(drawn_soon(&group(0, true), &[0], &[0], false));
        assert!(!drawn_soon(&group(3, true), &[0], &[0], false));
        // Posed (shows next frame) or fading out: rebuilt now.
        assert!(drawn_soon(&group(3, true), &[0], &[3], false));
        assert!(drawn_soon(&group(3, true), &[0], &[0], true));
        // Not built yet: builds at once.
        assert!(drawn_soon(&group(3, false), &[0], &[0], false));
    }

    #[test]
    fn rounding_stays_in_place_and_a_loose_panel_leaves() {
        let pose = Mat4::from_rotation_translation(
            Quat::from_euler(EulerRot::YXZ, 2.1, 0.03, -0.02),
            Vec3::new(812.4, 31.7, -604.9),
        );
        let rest = Mat4::from_translation(Vec3::new(0.8, 0.4, 1.3));
        let drawn = Transform::from_matrix(pose * rest).to_matrix();
        assert!(in_place(drawn, pose * rest));
        let open = pose * Mat4::from_rotation_y(0.01) * rest;
        assert!(!in_place(open, pose * rest));
        let dropped = Mat4::from_translation(Vec3::new(0., -0.002, 0.)) * pose * rest;
        assert!(!in_place(dropped, pose * rest));
    }

    #[test]
    fn player_sources_stay_owned_while_opponent_parts_batch() {
        use bevy::{asset::RenderAssetUsages, mesh::Indices};
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<CarBatches>()
            .add_message::<AssetEvent<Mesh>>()
            // Exercise the actual register/refresh/rebuild path, without an
            // environment-dependent install gate or a renderer/GPU fixture.
            .add_systems(PostUpdate, refresh);
        let mesh = app.world_mut().resource_mut::<Assets<Mesh>>().add(
            Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION,
                    vec![[0., 0., 0.], [1., 0., 0.], [0., 0., 1.]])
                .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 1., 0.]; 3])
                .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0,
                    vec![[0., 0.], [1., 0.], [0., 1.]])
                .with_inserted_indices(Indices::U16(vec![0, 2, 1])),
        );
        let material = app.world_mut().resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let mut players = Vec::new();
        let mut opponents = Vec::new();
        for opponent in [false, true] {
            for part in 0..2 {
                let rest = Mat4::from_translation(Vec3::new(part as f32 * 2., 0., 0.));
                let mut entity = app.world_mut().spawn((
                    Mesh3d(mesh.clone()), MeshMaterial3d(material.clone()),
                    Transform::from_matrix(rest), Visibility::Inherited,
                    Aabb::from_min_max(Vec3::ZERO, Vec3::ONE),
                    // Even a shared model prefix must not disable opponents.
                    crate::track_surface::SourcePart("car0/Car Body Paint".into()),
                ));
                if opponent {
                    entity.insert(crate::OpponentPart {
                        car: 1, rest, wheel: None, lost_with_wheel: false,
                    });
                    opponents.push(entity.id());
                } else {
                    entity.insert(crate::MovingPart {
                        rest, wheel: None, lost_with_wheel: false,
                    });
                    players.push(entity.id());
                }
            }
        }
        // First update registers, second builds, third exercises stable reuse.
        for _ in 0..3 { app.update(); }
        for entity in players {
            let source = app.world().entity(entity);
            assert_eq!(source.get::<crate::track_surface::SourcePart>().unwrap().0,
                "car0/Car Body Paint");
            assert!(source.contains::<crate::MovingPart>());
            assert!(!source.contains::<Batchable>());
            assert!(source.get::<RenderLayers>().is_none());
            assert_eq!(*source.get::<Visibility>().unwrap(), Visibility::Inherited);
            assert_eq!(source.get::<Mesh3d>().unwrap().0, mesh);
            assert_eq!(source.get::<MeshMaterial3d<StandardMaterial>>().unwrap().0, material);
        }
        for entity in opponents {
            let source = app.world().entity(entity);
            assert!(source.contains::<Batchable>());
            assert_eq!(source.get::<RenderLayers>(), Some(&RenderLayers::layer(HIDDEN_LAYER)));
        }
        let state = app.world().resource::<CarBatches>();
        assert_eq!(state.groups.len(), 1);
        assert_eq!(state.groups[0].car, 1);
        assert_eq!(state.groups[0].members.len(), 2);
        assert_eq!(state.groups[0].batches.len(), 1);
        let batch = state.groups[0].batches[0];
        let rendered = app.world().get::<Mesh3d>(batch).unwrap();
        assert_eq!(app.world().resource::<Assets<Mesh>>()
            .get(&rendered.0).unwrap().count_vertices(), 6);
    }

    /// A demoted part mesh stays in the main world while its part is on the
    /// hidden layer and gets its render copy back in the frame it leaves.
    #[test]
    fn a_demoted_mesh_returns_to_the_render_world_when_its_part_shows() {
        use bevy::asset::RenderAssetUsages;
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Demoted>()
            .add_systems(PostUpdate, restore);
        let mesh = app.world_mut().resource_mut::<Assets<Mesh>>().add(
            Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::MAIN_WORLD),
        );
        let part = app.world_mut().spawn(RenderLayers::layer(HIDDEN_LAYER)).id();
        app.world_mut().resource_mut::<Demoted>().0.insert(part, mesh.id());
        let usage = |app: &App| app.world().resource::<Assets<Mesh>>().get(&mesh).unwrap().asset_usage;
        app.update();
        assert_eq!(usage(&app), RenderAssetUsages::MAIN_WORLD);
        assert_eq!(app.world().resource::<Demoted>().0.len(), 1);
        app.world_mut().entity_mut(part).remove::<RenderLayers>();
        app.update();
        assert!(usage(&app).contains(RenderAssetUsages::RENDER_WORLD));
        assert!(app.world().resource::<Demoted>().0.is_empty());
    }
}
