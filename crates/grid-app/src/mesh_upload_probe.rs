//! Mesh upload probe (`GRID_MESH_UPLOADS=1`): which meshes are added or modified, how often.
//!
//! Every `AssetEvent::Added` or `Modified` of a `Mesh` makes the render world prepare and
//! upload that mesh again (once per frame per id, however many events). With the variable set,
//! every five seconds the app prints the meshes with the most uploads per second:
//! `mesh_uploads window_s= ids= added_s= modified_s= kb_s=` and per mesh
//! `mesh_upload id= per_s= added= vertices= kb= users= owner=`, where `owner` lists the
//! components of one entity drawing it (the module that changes it is usually plain from them).
//! Diagnostic only; nothing runs without the variable.

use bevy::asset::AssetId;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

#[derive(Resource)]
struct Tally {
    since: Instant,
    frames: u64,
    /// id -> (frames with an upload, frames with Added).
    per_id: HashMap<AssetId<Mesh>, (u64, u64)>,
    added: u64,
    modified: u64,
    bytes: u64,
    main_only: u64,
}

fn collect(
    mut events: MessageReader<AssetEvent<Mesh>>,
    meshes: Res<Assets<Mesh>>,
    mut tally: ResMut<Tally>,
) {
    let mut seen = HashSet::new();
    let mut added = HashSet::new();
    for event in events.read() {
        match event {
            AssetEvent::Added { id } => {
                seen.insert(*id);
                added.insert(*id);
            }
            AssetEvent::Modified { id } => {
                seen.insert(*id);
            }
            _ => {}
        }
    }
    tally.frames += 1;
    for id in &seen {
        // Kept for the main world only: changed, but not uploaded.
        if meshes.get(*id).is_some_and(|m| !crate::frame_split::uploaded(m)) {
            tally.main_only += 1;
            continue;
        }
        let a = added.contains(id);
        let e = tally.per_id.entry(*id).or_default();
        e.0 += 1;
        e.1 += u64::from(a);
        if a {
            tally.added += 1;
        } else {
            tally.modified += 1;
        }
        tally.bytes += meshes.get(*id).map_or(0, crate::frame_split::mesh_bytes);
    }
}

fn report(world: &mut World) {
    let secs = world.resource::<Tally>().since.elapsed().as_secs_f64();
    if secs < 5.0 {
        return;
    }
    let tally = std::mem::replace(
        &mut *world.resource_mut::<Tally>(),
        Tally {
            since: Instant::now(),
            frames: 0,
            per_id: HashMap::new(),
            added: 0,
            modified: 0,
            bytes: 0,
            main_only: 0,
        },
    );
    println!(
        "mesh_uploads window_s={secs:.1} frames={} ids={} added_s={:.1} modified_s={:.1} kb_s={:.0} main_only_s={:.1}",
        tally.frames,
        tally.per_id.len(),
        tally.added as f64 / secs,
        tally.modified as f64 / secs,
        tally.bytes as f64 / 1024. / secs,
        tally.main_only as f64 / secs,
    );
    // Owners: entities drawing each mesh.
    let mut users: HashMap<AssetId<Mesh>, (u32, Entity)> = HashMap::new();
    let mut query = world.query::<(Entity, &Mesh3d)>();
    for (entity, mesh) in query.iter(world) {
        let e = users.entry(mesh.id()).or_insert((0, entity));
        e.0 += 1;
    }
    let mut top: Vec<_> = tally.per_id.iter().collect();
    top.sort_by_key(|a| std::cmp::Reverse(a.1.0));
    // Group the rest by owner signature so many small meshes of one kind show up too.
    let mut by_owner: HashMap<String, (u64, u64)> = HashMap::new();
    let meshes = world.resource::<Assets<Mesh>>();
    let mut lines = Vec::new();
    for (id, (frames, added)) in &top {
        let (count, owner) = match users.get(id) {
            Some((n, e)) => (*n, owner_label(world, *e)),
            None => (0, "none".to_string()),
        };
        let mesh = meshes.get(**id);
        let bytes = mesh.map_or(0, crate::frame_split::mesh_bytes);
        let g = by_owner.entry(owner.clone()).or_default();
        g.0 += frames;
        g.1 += bytes * frames;
        if lines.len() < 15 {
            lines.push(format!(
                "mesh_upload id={id:?} per_s={:.1} added={added} vertices={} kb={:.1} users={count} owner={owner}",
                *frames as f64 / secs,
                mesh.map_or(0, Mesh::count_vertices),
                bytes as f64 / 1024.,
            ));
        }
    }
    for line in lines {
        println!("{line}");
    }
    // In-place vertex writes (mesh_stream): no preparation, but the bytes still cross to the GPU.
    let mut streamed: HashMap<String, (u64, u32)> = HashMap::new();
    for (id, bytes) in crate::mesh_stream::take_per_mesh() {
        let owner = users
            .get(&id)
            .map_or_else(|| "none".to_string(), |(_, e)| owner_label(world, *e));
        let s = streamed.entry(owner).or_default();
        s.0 += bytes;
        s.1 += 1;
    }
    let mut streamed: Vec<_> = streamed.into_iter().collect();
    streamed.sort_by_key(|a| std::cmp::Reverse(a.1.0));
    for (owner, (bytes, ids)) in streamed.into_iter().take(10) {
        println!(
            "mesh_stream_group kb_s={:.0} meshes={ids} owner={owner}",
            bytes as f64 / 1024. / secs
        );
    }
    let mut groups: Vec<_> = by_owner.into_iter().collect();
    groups.sort_by_key(|a| std::cmp::Reverse(a.1.0));
    for (owner, (n, bytes)) in groups.into_iter().take(20) {
        println!(
            "mesh_upload_group per_s={:.1} kb_s={:.0} owner={owner}",
            n as f64 / secs,
            bytes as f64 / 1024. / secs
        );
    }
}

/// The entity's name, else the nearest named ancestor's (`parent/`), digits folded to `#`
/// so cars and cells of one kind group together.
fn owner_label(world: &World, entity: Entity) -> String {
    let mut current = entity;
    let mut prefix = String::new();
    for _ in 0..6 {
        if let Some(name) = world.get::<Name>(current) {
            let folded: String = name
                .as_str()
                .chars()
                .map(|c| if c.is_ascii_digit() { '#' } else { c })
                .collect::<String>()
                .split('#')
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .join("#");
            return format!("{prefix}{}", folded.replace(' ', "_"));
        }
        match world.get::<ChildOf>(current) {
            Some(parent) => {
                current = parent.parent();
                prefix.push('^');
            }
            None => break,
        }
    }
    let material = if world.get::<MeshMaterial3d<StandardMaterial>>(entity).is_some() {
        "standard"
    } else {
        "custom-material"
    };
    format!("unnamed({material})")
}

pub fn install(app: &mut App) {
    if std::env::var("GRID_MESH_UPLOADS").as_deref() != Ok("1") {
        return;
    }
    crate::mesh_stream::tally_per_mesh();
    app.insert_resource(Tally {
        since: Instant::now(),
        frames: 0,
        per_id: HashMap::new(),
        added: 0,
        modified: 0,
        bytes: 0,
        main_only: 0,
    })
    .add_systems(Last, (collect, report).chain());
}
