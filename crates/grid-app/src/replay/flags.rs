//! Local-player CrowdState cue history for race and replay shown ticks.
//! Car, prop and collision records keep their existing binary codec.
use grid_tools::crowd_flags::{Cue, InZone};
use std::collections::VecDeque;

// A flag transition is a single three-bit state sample. Leave the frame
// stream's memory/spool limits untouched. A pathological cue overflow loses
// only the older cue prefix, which callers can detect via the view's seed.
const MAX_CHANGES: usize = 16_384;

#[derive(Clone, Copy)]
struct Change {
    tick: u64,
    zone: InZone,
}

#[derive(Default)]
struct Track {
    seed: Option<Change>,
    changes: VecDeque<Change>,
    prefix_missing: bool,
}

impl Track {
    fn last(&self) -> Option<Change> {
        self.changes.back().copied().or(self.seed)
    }

    fn observe(&mut self, tick: u64, zone: InZone) {
        let next = Change { tick, zone };
        let Some(last) = self.last() else {
            self.seed = Some(next);
            return;
        };
        if last.zone == zone {
            return;
        }
        if self.changes.len() == MAX_CHANGES {
            self.seed = self.changes.pop_front();
            self.prefix_missing = true;
        }
        self.changes.push_back(next);
    }

    fn discard_before(&mut self, tick: u64) {
        while self
            .changes
            .front()
            .is_some_and(|change| change.tick < tick)
        {
            self.seed = self.changes.pop_front();
            self.prefix_missing = true;
        }
    }

    fn cut(&mut self, tick: u64) {
        if self.seed.is_some_and(|seed| seed.tick > tick) {
            *self = Self {
                prefix_missing: true,
                ..Self::default()
            };
            return;
        }
        self.changes
            .truncate(self.changes.partition_point(|change| change.tick <= tick));
    }

    fn view(&self, segment: u64, tick: u64) -> Option<View<'_>> {
        let seed = self.seed.filter(|seed| seed.tick <= tick)?;
        let len = self.changes.partition_point(|change| change.tick <= tick);
        let current = len
            .checked_sub(1)
            .and_then(|index| self.changes.get(index).copied())
            .unwrap_or(seed);
        Some(View {
            segment,
            tick,
            initial_tick: seed.tick,
            initial_zone: seed.zone,
            zone: current.zone,
            prefix_missing: self.prefix_missing,
            older_changes: None,
            changes: &self.changes,
            len,
        })
    }
}

/// The real observed zone at this time and original-order edges since its seed.
/// A compacted prefix can preserve the zone without defining an earlier phase.
#[allow(
    dead_code,
    reason = "Retained cue query for the future authored marshal pose consumer"
)]
pub(crate) struct View<'a> {
    pub segment: u64,
    pub tick: u64,
    pub initial_tick: u64,
    pub initial_zone: InZone,
    pub zone: InZone,
    pub prefix_missing: bool,
    older_changes: Option<&'a VecDeque<Change>>,
    changes: &'a VecDeque<Change>,
    len: usize,
}

#[allow(
    dead_code,
    reason = "Retained cue query for the future authored marshal pose consumer"
)]
impl View<'_> {
    pub(crate) fn edges(&self) -> impl Iterator<Item = (u64, Cue)> + '_ {
        let mut previous = self.initial_zone;
        self.older_changes
            .into_iter()
            .flat_map(|changes| changes.iter())
            .chain(self.changes.iter().take(self.len))
            .flat_map(move |change| {
                let old = std::mem::replace(&mut previous, change.zone);
                let tick = change.tick;
                old.edges(change.zone).map(move |cue| (tick, cue))
            })
    }
}

#[derive(Default)]
pub(super) struct Lane {
    stamp: Option<(u64, u64)>,
    recorded: Track,
    after_recording: Option<Track>,
}

impl Lane {
    pub(super) fn observe(
        &mut self,
        segment: u64,
        tick: u64,
        zone: InZone,
        recording: bool,
        oldest_frame: Option<u64>,
    ) {
        if self.stamp == Some((segment, tick)) {
            return;
        }
        if self
            .stamp
            .is_some_and(|(old_segment, old_tick)| old_segment != segment || old_tick > tick)
        {
            *self = Self::default();
        }
        self.stamp = Some((segment, tick));

        if recording {
            self.recorded.observe(tick, zone);
            if let Some(oldest) = oldest_frame {
                self.recorded.discard_before(oldest);
            }
        } else {
            // Once car/prop recording is full, live cues remain available
            // without evicting the last replayable cues from the replay lane.
            let seed = self.recorded.last();
            let live = self.after_recording.get_or_insert_with(|| Track {
                seed,
                prefix_missing: seed.is_some(),
                ..Track::default()
            });
            live.observe(tick, zone);
        }
    }

    pub(super) fn cut(&mut self, segment: u64, tick: u64) {
        self.recorded.cut(tick);
        self.after_recording = None;
        self.stamp = Some((segment, tick));
    }

    pub(super) fn restore_missing_seed(&mut self, segment: u64, tick: u64, zone: InZone) {
        if self.stamp == Some((segment, tick)) && self.recorded.seed.is_none() {
            self.recorded.observe(tick, zone);
        }
    }

    #[allow(
        dead_code,
        reason = "Called by the future authored marshal pose consumer"
    )]
    pub(super) fn recorded(&self, tick: u64) -> Option<View<'_>> {
        let (segment, _) = self.stamp?;
        self.recorded.view(segment, tick)
    }

    #[allow(
        dead_code,
        reason = "Called by the future authored marshal pose consumer"
    )]
    pub(super) fn live(&self) -> Option<View<'_>> {
        let (segment, tick) = self.stamp?;
        let Some(tail) = &self.after_recording else {
            return self.recorded.view(segment, tick);
        };
        let mut view = tail.view(segment, tick)?;
        if self
            .recorded
            .last()
            .is_some_and(|last| last.tick == view.initial_tick)
            && let Some(seed) = self.recorded.seed
        {
            // Preserve the source prefix when the live tail still starts at
            // the last recorded change; a tail overflow advertises its gap.
            view.initial_tick = seed.tick;
            view.initial_zone = seed.zone;
            view.prefix_missing = self.recorded.prefix_missing;
            view.older_changes = Some(&self.recorded.changes);
        }
        Some(view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_tools::race_flags::{CarSample, RaceFlags, RaceState, RuleSet};

    fn active_zones() -> (InZone, InZone) {
        let mut flags = RaceFlags::on_line(RuleSet::single_race(), 3000., false, 1);
        let cars = [CarSample {
            position: 10.,
            angle: 0.,
            speed: 0.,
            racing: true,
        }];
        flags.update(
            11.,
            &RaceState {
                started: true,
                ended: false,
                cars: &cars,
                leader_to_line: Some(10.),
                line: 0.,
                chequered: true,
            },
        );
        (InZone::sample(&flags, 0.), InZone::sample(&flags, 50.))
    }

    fn codes(view: View<'_>) -> Vec<(u64, u32)> {
        view.edges()
            .map(|(tick, cue)| (tick, cue.native_code()))
            .collect()
    }

    #[test]
    fn intervening_ticks_and_historical_cut_preserve_native_order() {
        let (all, no_danger) = active_zones();
        let outside = InZone::default();
        let mut lane = Lane::default();
        lane.observe(7, 9, outside, true, Some(9));
        assert!(!super::super::sample_due(Some(9), 10));
        lane.observe(7, 10, all, true, Some(9));
        lane.observe(7, 10, outside, true, Some(9)); // repeated paused tick
        lane.observe(7, 11, no_danger, true, Some(9));
        assert_eq!(
            codes(lane.recorded(10).unwrap()),
            [(10, 1), (10, 3), (10, 2)]
        );
        assert_eq!(lane.recorded(11).unwrap().zone, no_danger);
        assert_eq!(lane.recorded(9).unwrap().zone, outside);
        assert_eq!(lane.live().unwrap().zone, no_danger); // seek is read-only

        lane.cut(7, 10);
        assert_eq!(lane.live().unwrap().zone, all);
        assert_eq!(
            codes(lane.recorded(11).unwrap()),
            [(10, 1), (10, 3), (10, 2)]
        );
        lane.observe(7, 11, outside, true, Some(9));
        assert_eq!(
            codes(lane.live().unwrap()),
            [(10, 1), (10, 3), (10, 2), (11, 4), (11, 6), (11, 5)]
        );
        lane.observe(8, 1, outside, true, Some(1));
        let reset = lane.live().unwrap();
        assert_eq!(reset.segment, 8);
        assert_eq!(reset.initial_tick, 1);
        assert!(codes(reset).is_empty());
    }

    #[test]
    fn spooling_stop_keeps_replay_history_and_memory_trim_keeps_zone() {
        let (all, _) = active_zones();
        let outside = InZone::default();
        let mut lane = Lane::default();
        lane.observe(0, 1, outside, true, Some(1));
        lane.observe(0, 2, all, true, Some(1));
        lane.observe(0, 3, all, true, Some(1));
        lane.observe(0, 4, outside, false, Some(1));
        assert_eq!(lane.live().unwrap().zone, outside);
        assert_eq!(lane.recorded(3).unwrap().zone, all);
        assert_eq!(
            codes(lane.live().unwrap()),
            [(2, 1), (2, 3), (2, 2), (4, 4), (4, 6), (4, 5)]
        );

        lane.cut(0, 3);
        assert_eq!(lane.live().unwrap().zone, all);
        lane.observe(0, 4, all, true, Some(3)); // memory-only frame window rolled
        let shown = lane.recorded(3).unwrap();
        assert_eq!(shown.initial_zone, all);
        assert!(shown.prefix_missing);
        assert!(codes(shown).is_empty());
    }

    #[test]
    fn retained_cue_prefix_is_bounded_and_old_cut_does_not_invent_an_edge() {
        let (all, _) = active_zones();
        let outside = InZone::default();
        let mut lane = Lane::default();
        lane.observe(0, 1, outside, true, Some(1));
        for i in 0..=MAX_CHANGES {
            lane.observe(
                0,
                2 + i as u64,
                if i % 2 == 0 { all } else { outside },
                true,
                Some(1),
            );
        }
        assert_eq!(lane.recorded.changes.len(), MAX_CHANGES);
        assert!(lane.recorded(1).is_none());
        assert!(lane.recorded(2).unwrap().prefix_missing);

        lane.cut(0, 1);
        assert!(lane.live().is_none());
        lane.restore_missing_seed(0, 1, all);
        let recovered = lane.live().unwrap();
        assert!(recovered.prefix_missing);
        assert_eq!(recovered.zone, all);
        assert!(codes(recovered).is_empty());
        lane.observe(0, 2, outside, true, Some(1));
        assert_eq!(codes(lane.live().unwrap()), [(2, 4), (2, 6), (2, 5)]);
    }
}
