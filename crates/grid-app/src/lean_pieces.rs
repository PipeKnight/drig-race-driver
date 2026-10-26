//! Dormant prop pieces (AUTHORED, P12, F574,
//! docs/PERFORMANCE.md "Dormant pieces"). Every breakable's pieces are
//! spawned with the track, hidden until it breaks: 14,463 drawn pieces on
//! Donington, 11,962 piece entities on the Nürburgring. Hidden, they still
//! cost CPU every frame in every pass that walks the mesh entities
//! (visibility per view, the directional light's casters, the mesh-asset
//! change checks, bounds). Here a hidden piece of an unbroken breakable gets
//! Bevy's `Disabled`, which the default query filter skips, and loses it
//! when its breakable breaks, before `dynamic_props::present` places and
//! shows it in the same frame. The picture is unchanged: such a piece is
//! never drawn.
//!
//! Pieces are disabled only after [`SETTLE_FRAMES`], so the systems that set
//! up new entities (`Added<..>` filters) have seen them. On every build
//! (owner, 2026-10-26: one render path); `GRID_LAZY_PIECES=0` turns it off.
use crate::dynamic_props::{PropPiece, present};
use bevy::ecs::entity_disabling::Disabled;
use bevy::prelude::*;
use std::collections::BTreeMap;

/// Frames from the start before the first pieces are disabled.
const SETTLE_FRAMES: u32 = 30;
/// Hidden pieces of unbroken breakables are disabled again this often
/// (after a restart clears the breaks).
const REST_EVERY: u32 = 60;

pub fn enabled() -> bool {
    std::env::var("GRID_LAZY_PIECES").map_or(true, |v| v != "0")
}

/// Disabled pieces by breakable.
#[derive(Resource, Default)]
struct Dormant {
    pieces: BTreeMap<usize, Vec<Entity>>,
    frames: u32,
}

pub fn install(app: &mut App) {
    if !enabled() {
        return;
    }
    app.init_resource::<Dormant>()
        .add_systems(Update, (wake.before(present), rest.after(present)));
    log_line!("lean_pieces version=1 settle_frames={SETTLE_FRAMES} rest_every={REST_EVERY}");
}

/// Breakables broken in the running race.
fn broken(driving: &crate::Driving) -> Option<&[bool]> {
    match &driving.opponents {
        Some(o) => o.multi.dynamic_props().map(|p| &p.broken[..]),
        None => driving.runtime.dynamic_props().map(|p| &p.broken[..]),
    }
}

/// Re-enable the pieces of breakables that broke since the last frame.
fn wake(mut commands: Commands, driving: Option<Res<crate::Driving>>, mut dormant: ResMut<Dormant>) {
    let Some(broken) = driving.as_deref().and_then(broken) else {
        return;
    };
    let woken: Vec<usize> = (dormant.pieces.keys())
        .copied()
        .filter(|&b| broken.get(b).copied().unwrap_or(false))
        .collect();
    for b in woken {
        for entity in dormant.pieces.remove(&b).unwrap_or_default() {
            if let Ok(mut entity) = commands.get_entity(entity) {
                entity.remove::<Disabled>();
            }
        }
    }
}

/// Disable the hidden pieces of unbroken breakables.
fn rest(
    mut commands: Commands,
    driving: Option<Res<crate::Driving>>,
    mut dormant: ResMut<Dormant>,
    pieces: Query<(Entity, &PropPiece, &Visibility)>,
) {
    dormant.frames += 1;
    if dormant.frames < SETTLE_FRAMES || !(dormant.frames - SETTLE_FRAMES).is_multiple_of(REST_EVERY) {
        return;
    }
    let Some(broken) = driving.as_deref().and_then(broken) else {
        return;
    };
    let mut count = 0;
    for (entity, piece, visibility) in &pieces {
        if *visibility == Visibility::Hidden && !broken.get(piece.breakable).copied().unwrap_or(false) {
            commands.entity(entity).insert(Disabled);
            dormant.pieces.entry(piece.breakable).or_default().push(entity);
            count += 1;
        }
    }
    if count > 0 {
        let total: usize = dormant.pieces.values().map(Vec::len).sum();
        log_line!("lean_pieces version=1 event=rest frame={} disabled={count} dormant={total}", dormant.frames);
    }
}
