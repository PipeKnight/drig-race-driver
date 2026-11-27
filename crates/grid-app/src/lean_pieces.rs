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
//! Pieces are disabled only [`SETTLE_FRAMES`] after the pipeline warm-up is
//! done, so the systems that set up new entities (`Added<..>` filters) have
//! seen them, and only once the renderer has seen them hidden (their
//! `ViewVisibility` is off). `Disabled` freezes an entity's whole state:
//! every system that would undo a change skips it (the warm-up's restore,
//! visibility, render extraction). On the phone the warm-up still showed
//! every piece at frame 30, so the old rule disabled them while the renderer
//! drew them (backlog row 149); a piece woke with that frozen state and no
//! visibility change. A piece skipped here is disabled at a later round. On
//! every build (owner, 2026-10-26: one render path); `GRID_LAZY_PIECES=0`
//! turns it off.
use crate::dynamic_props::{PropPiece, present};
use crate::pipeline_warmup::Warmup;
use bevy::ecs::entity_disabling::Disabled;
use bevy::prelude::*;
use std::collections::BTreeMap;

/// Frames after the warm-up before the first pieces are disabled.
const SETTLE_FRAMES: u32 = 30;
/// Hidden pieces of unbroken breakables are disabled again this often
/// (after a restart clears the breaks, or once the renderer hid a piece it
/// still showed).
const REST_EVERY: u32 = 60;

pub fn enabled() -> bool {
    crate::dev::var("GRID_LAZY_PIECES").map_or(true, |v| v != "0")
}

/// Disabled pieces by breakable.
#[derive(Resource, Default)]
struct Dormant {
    pieces: BTreeMap<usize, Vec<Entity>>,
    /// Frames since the warm-up was done (or since the start without one).
    frames: u32,
}

impl Dormant {
    /// Counts a frame; whether a rest round runs in it. Frames before the
    /// warm-up is done do not count.
    fn round(&mut self, warm_ready: bool) -> bool {
        if !warm_ready {
            return false;
        }
        self.frames += 1;
        self.frames >= SETTLE_FRAMES && (self.frames - SETTLE_FRAMES).is_multiple_of(REST_EVERY)
    }
}

/// Whether a piece may be disabled now: hidden, its breakable unbroken, and
/// already hidden for the renderer, whose state for a disabled entity stays
/// as it was.
fn restable(visibility: Visibility, view: Option<ViewVisibility>, broken: bool) -> bool {
    visibility == Visibility::Hidden && !broken && !view.is_some_and(ViewVisibility::get)
}

pub fn install(app: &mut App) {
    if !enabled() {
        return;
    }
    app.init_resource::<Dormant>()
        .add_systems(Update, (wake.before(present), rest.after(present)));
    log_line!("lean_pieces version=2 settle_frames={SETTLE_FRAMES} after=warmup rest_every={REST_EVERY}");
}

/// Breakables broken in the running race.
fn broken(driving: &crate::Driving) -> Option<&[bool]> {
    match &driving.opponents {
        Some(o) => o.multi.dynamic_props().map(|p| &p.broken[..]),
        None => driving.runtime.dynamic_props().map(|p| &p.broken[..]),
    }
}

/// Re-enable the pieces of breakables that broke since the last frame. Each
/// wake is logged with the pieces the renderer still took for visible
/// (`stale_visible`, 0 unless a piece was disabled while drawn).
fn wake(
    mut commands: Commands,
    driving: Option<Res<crate::Driving>>,
    mut dormant: ResMut<Dormant>,
    views: Query<&ViewVisibility, With<Disabled>>,
) {
    let Some(broken) = driving.as_deref().and_then(broken) else {
        return;
    };
    let woken: Vec<usize> = (dormant.pieces.keys())
        .copied()
        .filter(|&b| broken.get(b).copied().unwrap_or(false))
        .collect();
    for b in woken {
        let pieces = dormant.pieces.remove(&b).unwrap_or_default();
        let stale = pieces.iter().filter(|&&e| views.get(e).is_ok_and(|v| v.get())).count();
        log_line!("lean_pieces version=2 event=wake breakable={b} pieces={} stale_visible={stale}", pieces.len());
        for entity in pieces {
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
    warm: Option<Res<Warmup>>,
    mut dormant: ResMut<Dormant>,
    pieces: Query<(Entity, &PropPiece, &Visibility, Option<&ViewVisibility>)>,
) {
    if !dormant.round(warm.is_none_or(|w| w.ready())) {
        return;
    }
    let Some(broken) = driving.as_deref().and_then(broken) else {
        return;
    };
    let (mut count, mut shown) = (0, 0);
    for (entity, piece, visibility, view) in &pieces {
        let broken = broken.get(piece.breakable).copied().unwrap_or(false);
        if restable(*visibility, view.copied(), broken) {
            commands.entity(entity).insert(Disabled);
            dormant.pieces.entry(piece.breakable).or_default().push(entity);
            count += 1;
        } else if *visibility == Visibility::Hidden && !broken {
            shown += 1;
        }
    }
    if count > 0 || shown > 0 {
        let total: usize = dormant.pieces.values().map(Vec::len).sum();
        log_line!(
            "lean_pieces version=2 event=rest frame={} disabled={count} still_shown={shown} dormant={total}",
            dormant.frames
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounds_wait_for_the_warm_up() {
        // The warm-up shows every hidden piece; on the phone it still ran at
        // frame 30 (row 149). No round runs before it is done.
        let mut dormant = Dormant::default();
        assert!((0..500).all(|_| !dormant.round(false)));
        let rounds: Vec<u32> = (1..=200).filter(|_| dormant.round(true)).collect();
        assert_eq!(rounds, [SETTLE_FRAMES, SETTLE_FRAMES + REST_EVERY, SETTLE_FRAMES + 2 * REST_EVERY]);
    }

    #[test]
    fn a_piece_the_renderer_still_shows_is_not_disabled() {
        // Disabled freezes the renderer's state: a piece disabled while
        // drawn wakes as "visible" without a visibility change.
        assert!(!restable(Visibility::Hidden, Some(ViewVisibility::VISIBLE), false));
        assert!(restable(Visibility::Hidden, Some(ViewVisibility::HIDDEN), false));
        assert!(restable(Visibility::Hidden, None, false));
        // Shown or broken pieces stay enabled.
        assert!(!restable(Visibility::Inherited, Some(ViewVisibility::HIDDEN), false));
        assert!(!restable(Visibility::Hidden, Some(ViewVisibility::HIDDEN), true));
    }
}
