//! Authored marshal view-demand observations on the port's fixed-tick clock.
use std::collections::VecDeque;

const MAX_SAMPLES: usize = 16_384;

// Bit 3 distinguishes an observed empty selection from a missing observation.
// Bits 0..2 follow the three CrowdState role indices (start/danger/finish).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "Input for the future authored marshal pose consumer"
)]
pub(crate) struct Demand(u8);

#[allow(
    dead_code,
    reason = "Input for the future authored marshal pose consumer"
)]
impl Demand {
    pub(crate) const fn observed(roles: u8) -> Option<Self> {
        if roles & !7 == 0 {
            Some(Self(8 | roles))
        } else {
            None
        }
    }

    pub(crate) const fn roles(self) -> Option<u8> {
        if self.0 & 8 != 0 {
            Some(self.0 & 7)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Sample {
    pub tick: u64,
    pub demand: Demand,
}

#[derive(Clone, Copy)]
struct Stamped {
    segment: u64,
    sample: Sample,
}

/// Samples from this view's earliest retained anchor through the shown tick.
/// A missing prefix can preserve demand without reconstructing a prior phase.
#[allow(
    dead_code,
    reason = "History for the future authored marshal pose consumer"
)]
pub(crate) struct View<'a> {
    pub segment: u64,
    pub shown: Sample,
    pub prefix_missing: bool,
    transitions: &'a VecDeque<Sample>,
    count: usize,
}

#[allow(
    dead_code,
    reason = "History for the future authored marshal pose consumer"
)]
impl View<'_> {
    pub(crate) fn samples(&self) -> impl Iterator<Item = Sample> + '_ {
        self.transitions.iter().copied().take(self.count)
    }
}

/// Records independently of flag cues so rapid camera movement cannot evict
/// a rare source cue. Once the frame spool stops, preserve the archive and
/// just retain the last live input; a fixed-tick actor can advance from it.
#[derive(Default)]
pub(super) struct History {
    published: Option<Stamped>,
    live: Option<Stamped>,
    samples: VecDeque<Sample>,
    archived_through: Option<u64>,
    prefix_missing: bool,
}

impl History {
    /// A finished live render selection at tick T supplies demand from
    /// fixed tick T+1. Multiple presentations at T may publish; the last wins.
    #[allow(
        dead_code,
        reason = "Called by the future authored marshal pose consumer"
    )]
    pub(super) fn publish(&mut self, segment: u64, tick: u64, demand: Demand) {
        if self
            .published
            .is_some_and(|old| old.segment == segment && old.sample.tick > tick)
        {
            return;
        }
        self.published = Some(Stamped {
            segment,
            sample: Sample { tick, demand },
        });
    }

    pub(super) fn record(
        &mut self,
        segment: u64,
        tick: u64,
        recording: bool,
        oldest_frame: Option<u64>,
    ) {
        if let Some(previous) = self.live {
            if previous.segment == segment && previous.sample.tick == tick {
                return;
            }
            if previous.segment != segment || previous.sample.tick > tick {
                let publication = self
                    .published
                    .filter(|p| previous.segment != segment && p.segment == segment);
                *self = Self {
                    published: publication,
                    ..Self::default()
                };
            }
        }
        let demand = self
            .published
            .filter(|p| p.segment == segment && p.sample.tick < tick)
            .map_or_else(Demand::default, |p| p.sample.demand);
        let sample = Sample { tick, demand };
        self.live = Some(Stamped { segment, sample });
        if !recording {
            return;
        }
        self.archive(sample);
        if let Some(oldest) = oldest_frame {
            // Preserve one anchor for the oldest remaining car frame.
            while self.samples.get(1).is_some_and(|next| next.tick <= oldest) {
                self.samples.pop_front();
                self.prefix_missing = true;
            }
        }
    }

    fn archive(&mut self, sample: Sample) {
        self.archived_through = Some(sample.tick);
        if self
            .samples
            .back()
            .is_some_and(|last| last.demand == sample.demand)
        {
            return;
        }
        if self.samples.len() == MAX_SAMPLES {
            self.samples.pop_front();
            self.prefix_missing = true;
        }
        self.samples.push_back(sample);
    }

    /// A successful exact-frame Flashback keeps its historical demand value.
    /// Direct Update publication is suspended during replay/resim, so dropping
    /// the current publication invalidates all live input from the old branch.
    pub(super) fn cut(&mut self, segment: u64, tick: u64) {
        self.published = None;
        let Some(active) = self.live else { return };
        if active.segment != segment {
            *self = Self::default();
            return;
        }
        let count = self.samples.partition_point(|s| s.tick <= tick);
        self.prefix_missing |= count == 0 && !self.samples.is_empty();
        self.samples.truncate(count);
        let demand = self
            .samples
            .back()
            .map_or_else(Demand::default, |s| s.demand);
        let sample = Sample { tick, demand };
        self.live = Some(Stamped { segment, sample });
        self.archive(sample);
    }

    #[allow(
        dead_code,
        reason = "Called by the future authored marshal pose consumer"
    )]
    pub(super) fn live(&self) -> Option<Sample> {
        self.live.map(|live| live.sample)
    }

    #[allow(
        dead_code,
        reason = "Called by the future authored marshal pose consumer"
    )]
    pub(super) fn recorded(&self, tick: u64) -> Option<View<'_>> {
        let segment = self.live?.segment;
        if tick > self.archived_through? {
            return None;
        }
        let count = self.samples.partition_point(|s| s.tick <= tick);
        let demand = self.samples.get(count.checked_sub(1)?)?.demand;
        Some(View {
            segment,
            shown: Sample { tick, demand },
            prefix_missing: self.prefix_missing,
            transitions: &self.samples,
            count,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observed(roles: u8) -> Demand {
        Demand::observed(roles).unwrap()
    }

    #[test]
    fn next_distinct_fixed_tick_observes_latest_live_presentation() {
        let mut history = History::default();
        assert!(Demand::observed(8).is_none());
        history.record(7, 9, true, Some(9));
        assert_eq!(history.live().unwrap().demand.roles(), None);
        history.publish(7, 9, observed(1));
        history.publish(7, 9, observed(2));
        history.record(7, 9, true, Some(9)); // paused/repeated tick
        assert_eq!(history.live().unwrap().demand.roles(), None);
        history.record(7, 10, true, Some(9)); // between two 30 Hz frames
        history.record(7, 11, true, Some(9)); // another fixed step in one render frame
        assert_eq!(history.recorded(10).unwrap().shown.demand.roles(), Some(2));
        assert_eq!(history.recorded(11).unwrap().shown.demand.roles(), Some(2));
        history.publish(7, 11, observed(0));
        history.record(7, 12, true, Some(9));
        let earlier = history.recorded(10).unwrap();
        assert_eq!(earlier.segment, 7);
        assert_eq!(
            earlier.samples().collect::<Vec<_>>(),
            [
                Sample {
                    tick: 9,
                    demand: Demand::default()
                },
                Sample {
                    tick: 10,
                    demand: observed(2)
                },
            ]
        );
        assert_eq!(history.live().unwrap().demand.roles(), Some(0)); // seek is read-only
    }

    #[test]
    fn cut_keeps_exact_history_and_discards_publication_from_the_abandoned_branch() {
        let mut history = History::default();
        history.publish(7, 1, observed(1));
        history.record(7, 2, true, Some(2));
        history.publish(7, 2, observed(2));
        history.record(7, 3, true, Some(2));
        history.publish(7, 3, observed(4));
        history.record(7, 4, false, Some(2)); // car spool already stopped
        assert_eq!(history.live().unwrap().demand.roles(), Some(4));
        assert!(history.recorded(4).is_none());
        assert_eq!(history.recorded(3).unwrap().shown.demand.roles(), Some(2));

        history.cut(7, 2);
        assert_eq!(history.live().unwrap().demand.roles(), Some(1));
        assert!(history.recorded(3).is_none());
        history.record(7, 3, true, Some(2));
        assert_eq!(history.live().unwrap().demand.roles(), None);
        history.publish(7, 3, observed(0));
        history.record(7, 4, true, Some(2));
        assert_eq!(history.live().unwrap().demand.roles(), Some(0));

        history.publish(8, 0, observed(4)); // first observation of a new race
        history.record(8, 1, true, Some(1));
        assert_eq!(history.recorded(1).unwrap().segment, 8);
        assert_eq!(history.live().unwrap().demand.roles(), Some(4));
        assert!(history.recorded(2).is_none());
    }

    #[test]
    fn can_seek_to_the_oldest_frame_and_reports_a_capped_prefix() {
        let mut history = History::default();
        history.publish(0, 0, observed(1));
        history.record(0, 1, true, Some(1));
        history.publish(0, 1, observed(0));
        history.record(0, 2, true, Some(1));
        history.record(0, 3, true, Some(3));
        let rolled = history.recorded(3).unwrap();
        assert!(rolled.prefix_missing);
        assert_eq!(
            rolled.samples().next(),
            Some(Sample {
                tick: 2,
                demand: observed(0)
            })
        );

        for i in 0..MAX_SAMPLES as u64 {
            history.publish(0, 3 + i, observed(1 + (i % 2) as u8));
            history.record(0, 4 + i, true, Some(3));
        }
        assert!(std::mem::size_of::<Sample>() <= 16);
        assert_eq!(history.samples.len(), MAX_SAMPLES);
        assert!(history.recorded(3).is_none());
        assert!(history.recorded(4).unwrap().prefix_missing);
        history.cut(0, 3);
        let recovered = history.recorded(3).unwrap();
        assert!(recovered.prefix_missing);
        assert_eq!(recovered.shown.demand.roles(), None);
        assert!(history.recorded(4).is_none());
    }
}
