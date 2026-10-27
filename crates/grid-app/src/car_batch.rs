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
use crate::static_batch::{ATTRIBUTES, MAX_VERTICES, Part, build, supported};
use bevy::{
    asset::AssetEventSystems,
    camera::{
        primitives::{Aabb, MeshAabb},
        visibility::{RenderLayers, VisibilityRange, VisibilitySystems},
    },
    light::NotShadowCaster,
    mesh::{MeshVertexAttributeId, PrimitiveTopology, VertexFormat},
    prelude::*,
    transform::TransformSystems,
};
use std::collections::HashSet;

/// The layer of batched parts: no camera or light draws it.
const HIDDEN_LAYER: usize = 30;
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
    app.init_resource::<CarBatches>().add_systems(
        PostUpdate,
        refresh
            .after(AssetEventSystems)
            .before(TransformSystems::Propagate)
            .before(VisibilitySystems::VisibilityPropagate),
    );
}

/// A merged mesh of one car's parts of one material.
#[derive(Component)]
pub struct Batch;

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
}

#[derive(Resource, Default)]
struct CarBatches {
    groups: Vec<Group>,
    /// Per car: the (group, member) whose placement gives the car's pose.
    anchors: Vec<(usize, usize)>,
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
    (
        Has<RenderLayers>,
        Has<NotShadowCaster>,
        Has<VisibilityRange>,
        Has<ChildOf>,
        Has<Children>,
        Has<crate::source_trees::Facing>,
        Has<crate::sidewall_filter::Sidewall>,
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

#[allow(clippy::too_many_arguments)]
fn refresh(
    mut commands: Commands,
    mut state: ResMut<CarBatches>,
    added: Query<Candidate, NewPart>,
    parts: Query<Placement, Without<Batch>>,
    mut batches: Query<(&mut Transform, &mut Visibility, &Mesh3d, &mut Aabb), With<Batch>>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Res<Assets<StandardMaterial>>,
    mut events: MessageReader<AssetEvent<Mesh>>,
) {
    let state = &mut *state;
    // New parts take their place from the next frame (their marker is
    // inserted by command).
    if register(
        &mut commands,
        &added,
        &meshes,
        &materials,
        &mut state.groups,
        &mut state.anchors,
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
        member.placed = *visibility != Visibility::Hidden
            && layers.is_none_or(|l| *l == hidden)
            && !shadowless
            && !ranged
            && material.0 == group.material
            && mesh.0 == member.mesh
            && pose.is_some_and(|p| in_place(transform.to_matrix(), p * member.rest));
    }
    let mut votes = vec![[0usize; 2]; anchors.len()];
    for group in &state.groups {
        for member in &group.members {
            votes[group.slot][usize::from(member.placed)] += 1;
        }
    }
    let held: Vec<bool> = poses
        .iter()
        .zip(&votes)
        .map(|(pose, [out, placed])| pose.is_none() || out > placed)
        .collect();
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
                    commands.entity(member.entity).insert(hidden.clone());
                } else if parts.get(member.entity).is_ok_and(|p| p.5 == Some(&hidden)) {
                    commands.entity(member.entity).remove::<RenderLayers>();
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
            rebuild(&mut commands, group, pose, &mut meshes, &mut batches);
            state.rebuild_ns += start.elapsed().as_nanos() as u64;
        }
        if let Some(pose) = pose {
            let next = Transform::from_matrix(pose);
            for &batch in &group.batches {
                if let Ok((mut transform, ..)) = batches.get_mut(batch) {
                    transform.set_if_neq(next);
                }
            }
        }
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
    if state.frames == 1 || state.frames.is_multiple_of(REPORT_FRAMES) {
        let spawned: usize = state.groups.iter().map(|g| g.batches.len()).sum();
        println!(
            "car_batch version=1 frame={} cars={} groups={} parts={} batched={} batches={spawned} rebuilds={} reshaped={} rebuild_ms={:.1}",
            state.frames,
            state.anchors.len(),
            state.groups.len(),
            stats[0],
            stats[1],
            state.rebuilds,
            state.reshaped,
            state.rebuild_ns as f64 / 1e6,
        );
    }
}

/// New car parts that can share a mesh: no wheel (it turns on the car), an
/// opaque or masked material, a supported indexed triangle list, and no
/// per-entity render setting.
fn register(
    commands: &mut Commands,
    added: &Query<Candidate, NewPart>,
    meshes: &Assets<Mesh>,
    materials: &Assets<StandardMaterial>,
    groups: &mut Vec<Group>,
    anchors: &mut Vec<(usize, usize)>,
) -> bool {
    let before = groups.iter().map(|g| g.members.len()).sum::<usize>();
    for (entity, mesh, material, opponent, moving, source, special) in added {
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
        let (layers, shadowless, ranged, child, parent, facing, sidewall) = special;
        if wheel.is_some()
            || layers
            || shadowless
            || ranged
            || child
            || parent
            || facing
            || sidewall
        {
            continue;
        }
        if !materials
            .get(&material.0)
            .is_some_and(|m| matches!(m.alpha_mode, AlphaMode::Opaque | AlphaMode::Mask(_)))
        {
            continue;
        }
        let Some(source) = meshes.get(&mesh.0) else {
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
        let found = groups
            .iter()
            .position(|g| g.car == car && g.material == material.0 && g.layout == layout);
        let index = found.unwrap_or(groups.len());
        let place = Batchable {
            group: index as u32,
            member: found.map_or(0, |g| groups[g].members.len()) as u32,
        };
        commands.entity(entity).insert(place);
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
            .filter(|(_, g)| g.slot == slot)
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

/// The group's batched parts in batches of at most [`MAX_VERTICES`], in
/// spawn order; unused batches are hidden.
fn rebuild(
    commands: &mut Commands,
    group: &mut Group,
    pose: Option<Mat4>,
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
                let entity = commands
                    .spawn((
                        Name::new(format!("car_batch/{}/{i}", group.car)),
                        Mesh3d(meshes.add(mesh)),
                        MeshMaterial3d(group.material.clone()),
                        pose.map_or_else(Transform::default, Transform::from_matrix),
                        bounds,
                        Batch,
                    ))
                    .id();
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
}
