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
use crate::static_batch::{ATTRIBUTES, MAX_VERTICES, Part, build, supported};
use bevy::{
    asset::AssetEventSystems,
    camera::{
        primitives::{Aabb, MeshAabb},
        visibility::{NoAutoAabb, RenderLayers, VisibilityRange, VisibilitySystems},
    },
    light::NotShadowCaster,
    mesh::{MeshVertexAttributeId, PrimitiveTopology, VertexFormat},
    prelude::*,
    transform::TransformSystems,
};
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
    std::env::var("GRID_CAR_BATCH").map_or(true, |v| v != "0")
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
    vertices: usize,
    batched: bool,
    /// In place this frame.
    placed: bool,
    /// Still spawned this frame.
    alive: bool,
    /// Consecutive frames in place.
    stable: u32,
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
    /// Cars with a [`ShadowBody`].
    shadow_cars: HashSet<usize>,
    /// Car-frames per level shown, level changes and time in [`refresh`],
    /// for the log.
    shown_frames: [u64; 6],
    level_changes: u64,
    refresh_ns: u64,
    frames: u64,
    rebuilds: u64,
    /// Rebuilds for a deformed member mesh only, and the time of all.
    reshaped: u64,
    rebuild_ns: u64,
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
        Has<RenderLayers>,
        Has<NotShadowCaster>,
        Has<VisibilityRange>,
        Has<ChildOf>,
        Has<Children>,
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
    parts: Query<Placement, Without<Batch>>,
    mut batches: Query<(&mut Transform, &mut Visibility, &Mesh3d, &mut Aabb), With<Batch>>,
    mut bodies: Query<Body, (Without<Batch>, Without<Batchable>)>,
    camera: Query<(&Transform, &Projection), ViewerFilter>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<Assets<StandardMaterial>>,
    mut events: MessageReader<AssetEvent<Mesh>>,
    own: OwnCarView,
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
            && (layers.is_none() || ours)
            && (!shadowless || ours || group.shadow_body)
            && !ranged
            && material.0 == group.material
            && mesh.0 == member.mesh
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
    // The level whose loose parts were posed last frame is shown now; the
    // other levels' loose parts stop being posed.
    for (slot, pose) in poses.iter().enumerate() {
        let level = state.posed[slot];
        let Some(pose) = pose.filter(|_| state.shown[slot] != level) else {
            continue;
        };
        state.shown[slot] = level;
        state.level_changes += 1;
        let car = state.groups.iter().find(|g| g.slot == slot).map(|g| g.car);
        for group in state.groups.iter().filter(|g| g.slot == slot) {
            for &batch in &group.batches {
                show(&mut commands, batch, group.level == level);
                if group.level == level
                    && let Ok((mut transform, ..)) = batches.get_mut(batch)
                {
                    transform.set_if_neq(Transform::from_matrix(pose));
                }
            }
        }
        for &(_, part_level, entity) in state.extras.iter().filter(|e| Some(e.0) == car) {
            show(&mut commands, entity, part_level == level);
            if part_level != level
                && let Ok(mut entity) = commands.get_entity(entity)
            {
                entity.try_insert(Unposed);
            }
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
                continue;
            }
            stats[1] += group.members.iter().filter(|m| m.batched).count();
            // A deformed member rebuilds once the car is placed again.
            group.dirty |= group
                .members
                .iter()
                .any(|m| m.batched && modified.contains(&m.mesh.id()));
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
            } else if batched && modified.contains(&member.mesh.id()) {
                reshaped |= !group.dirty;
                group.dirty = true;
            }
            stats[1] += usize::from(member.batched);
        }
        let pose = poses[group.slot];
        if std::mem::take(&mut group.dirty) {
            state.rebuilds += 1;
            state.reshaped += u64::from(reshaped);
            let start = std::time::Instant::now();
            let shown = state.shown.get(group.slot).copied().unwrap_or(0) == group.level;
            rebuild(&mut commands, group, pose, shown, &mut meshes, &mut batches);
            state.rebuild_ns += start.elapsed().as_nanos() as u64;
        }
        // Batches of a level the car does not show keep their pose.
        if let Some(pose) = pose.filter(|_| state.shown[group.slot] == group.level) {
            let next = Transform::from_matrix(pose);
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
            "car_batch version=2 frame={} cars={} groups={} parts={} batched={} batches={spawned} extras={} rebuilds={} reshaped={} rebuild_ms={:.1} refresh_ms={:.1} level_changes={} level_car_frames={:?}",
            state.frames,
            state.anchors.len(),
            state.groups.len(),
            stats[0],
            stats[1],
            state.extras.len(),
            state.rebuilds,
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
        let (layers, shadowless, ranged, child, parent, facing, sidewall, interior) = special;
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
            vertices,
            batched: false,
            placed: false,
            alive: true,
            stable: REJOIN_FRAMES,
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

/// The group's batched parts in batches of at most [`MAX_VERTICES`], in
/// spawn order; unused batches are hidden. New batches of a level the car
/// does not show start on the hidden layer.
fn rebuild(
    commands: &mut Commands,
    group: &mut Group,
    pose: Option<Mat4>,
    shown: bool,
    meshes: &mut Assets<Mesh>,
    batches: &mut Query<(&mut Transform, &mut Visibility, &Mesh3d, &mut Aabb), With<Batch>>,
) {
    let mut chunks: Vec<Vec<Part<StandardMaterial>>> = Vec::new();
    let mut count = MAX_VERTICES;
    for member in group.members.iter().filter(|m| m.batched) {
        if count + member.vertices > MAX_VERTICES {
            chunks.push(Vec::new());
            count = 0;
        }
        count += member.vertices;
        chunks.last_mut().expect("chunk").push(Part {
            entity: member.entity,
            material: group.material.clone(),
            intact: false,
            mesh: member.mesh.clone(),
            matrix: member.rest,
            vertices: member.vertices,
        });
    }
    for (i, chunk) in chunks.iter().enumerate() {
        let (mesh, _, _) = build(chunk, meshes, Vec3::ZERO, &group.layout);
        let bounds = mesh.compute_aabb().unwrap_or_default();
        match group.batches.get(i).and_then(|&e| batches.get_mut(e).ok()) {
            Some((_, mut visibility, handle, mut aabb)) => {
                if let Some(mut target) = meshes.get_mut(&handle.0) {
                    *target = mesh;
                }
                *aabb = bounds;
                visibility.set_if_neq(Visibility::Inherited);
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
