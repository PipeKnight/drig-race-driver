//! Forward authored marshal actions on the port's recorded fixed-tick clock.
use super::flag_demand::{Demand, Sample};
use grid_tools::crowd_flags::{Cue, Role};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Phase {
    #[default]
    Idle,
    Start,
    Mid,
    End,
}

#[derive(Clone, Copy)]
pub(crate) struct Clip {
    pub duration: f64,
    pub samples: usize,
}

pub(crate) struct Clips {
    stage: [Clip; 4],
    boundary: [u64; 4],
    fixed_delta: f64,
}

impl Clips {
    pub(crate) fn new(stage: [Clip; 4], fixed_delta: f64) -> Option<Self> {
        if !fixed_delta.is_finite() || fixed_delta <= 0. {
            return None;
        }
        let mut boundary = [0; 4];
        for (ticks, clip) in boundary.iter_mut().zip(stage) {
            let due = (clip.duration / fixed_delta).ceil();
            if !due.is_finite() || due < 1. || due >= u64::MAX as f64 || clip.samples == 0 {
                return None;
            }
            *ticks = due as u64;
        }
        Some(Self {
            stage,
            boundary,
            fixed_delta,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Frame {
    pub phase: Phase,
    pub sample: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct State {
    phase: Phase,
    elapsed: u64,
    pending: bool,
    stop: bool,
}

impl State {
    pub(crate) fn frame(self, clips: &Clips) -> Frame {
        let clip = clips.stage[self.phase as usize];
        let fraction = self.elapsed as f64 * clips.fixed_delta / clip.duration;
        let sample = (fraction * clip.samples as f64) as usize;
        Frame {
            phase: self.phase,
            sample: sample.min(clip.samples - 1),
        }
    }

    fn cue(&mut self, role: Role, cue: Cue) {
        match cue {
            Cue::Reset => *self = Self::default(),
            Cue::Begin(target) if target == role => self.pending = true,
            Cue::End(target) if target == role && self.phase != Phase::Idle => self.stop = true,
            _ => {}
        }
    }

    fn start_pending(&mut self) -> bool {
        if self.phase == Phase::Idle && self.pending {
            self.phase = Phase::Start;
            self.elapsed = 0;
            self.pending = false;
            self.stop = false;
            return true;
        }
        false
    }

    fn finish(&mut self) {
        *self = Self {
            pending: self.pending,
            ..Self::default()
        };
    }

    /// One fixed input, matching the historical rule/demand model below.
    pub(crate) fn step(&mut self, clips: &Clips, role: Role, demand: Demand, cues: &[Cue]) {
        for &cue in cues {
            self.cue(role, cue);
        }
        self.advance(clips, role, demand, 1);
    }

    fn advance(&mut self, clips: &Clips, role: Role, demand: Demand, mut ticks: u64) {
        if ticks == 0 {
            return;
        }
        let demanded = demand
            .roles()
            .map(|mask| mask & (1 << (role as u8 - 1)) != 0);
        // The source cancels pending stops before testing the clip boundary
        // when no slot of this role is demanded. Unknown holds the clock
        // without choosing the out-of-view cancellation.
        if self.stop && demanded == Some(false) {
            self.finish();
        }
        if self.start_pending() {
            ticks -= 1; // Show the first transition sample on its entry tick.
        }
        if demanded != Some(true) {
            return;
        }
        while ticks > 0 {
            let boundary = clips.boundary[self.phase as usize];
            if self.phase == Phase::Idle || (self.phase == Phase::Mid && !self.stop) {
                self.elapsed =
                    ((u128::from(self.elapsed) + u128::from(ticks)) % u128::from(boundary)) as u64;
                return;
            }
            let remaining = boundary - self.elapsed;
            if ticks < remaining {
                self.elapsed += ticks;
                return;
            }
            ticks -= remaining;
            // Port policy: a phase changes at a fixed-tick boundary. Each
            // next clip starts at its authored first coordinate.
            self.elapsed = 0;
            match self.phase {
                Phase::Idle => unreachable!(),
                Phase::Start => self.phase = Phase::Mid,
                Phase::Mid => self.phase = Phase::End,
                Phase::End => {
                    self.finish();
                    self.start_pending();
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Seed {
    pub tick: u64,
    pub state: State,
    pub demand: Demand,
}

/// Rebuild the recorded forward pose. The output is independent of seek
/// direction; old history without a trustworthy seed is refused by the caller.
/// Inputs are chronological, with cues in source order at equal ticks.
pub(crate) fn replay_at(
    clips: &Clips,
    role: Role,
    mut seed: Seed,
    shown: u64,
    cues: impl Iterator<Item = (u64, Cue)>,
    demand: impl Iterator<Item = Sample>,
) -> Option<Seed> {
    let initial = seed.tick;
    if shown < initial {
        return None;
    }
    let mut cues = cues.filter(|(t, _)| *t > initial && *t <= shown).peekable();
    let mut demand = demand
        .filter(|s| s.tick > initial && s.tick <= shown)
        .peekable();
    while let Some(tick) = cues
        .peek()
        .map(|(t, _)| *t)
        .into_iter()
        .chain(demand.peek().map(|s| s.tick))
        .min()
    {
        let skipped = tick.checked_sub(seed.tick)?.checked_sub(1)?;
        seed.state.advance(clips, role, seed.demand, skipped);
        while demand.peek().is_some_and(|s| s.tick == tick) {
            seed.demand = demand.next()?.demand;
        }
        while cues.peek().is_some_and(|(t, _)| *t == tick) {
            seed.state.cue(role, cues.next()?.1);
        }
        seed.state.advance(clips, role, seed.demand, 1);
        seed.tick = tick;
    }
    seed.state
        .advance(clips, role, seed.demand, shown - seed.tick);
    seed.tick = shown;
    Some(seed)
}

#[cfg(test)]
mod tests {
    use super::super::flag_demand::History;
    use super::*;

    fn clips() -> Clips {
        Clips::new(
            [
                Clip {
                    duration: 2.,
                    samples: 2,
                },
                Clip {
                    duration: 3.,
                    samples: 6,
                },
                Clip {
                    duration: 4.,
                    samples: 8,
                },
                Clip {
                    duration: 2.,
                    samples: 4,
                },
            ],
            1.,
        )
        .unwrap()
    }

    fn seen(mask: u8) -> Demand {
        Demand::observed(mask).unwrap()
    }

    #[test]
    fn each_role_uses_start_held_mid_end_and_loop_boundaries() {
        for (role, bits) in [(Role::Start, 1), (Role::Danger, 2), (Role::Finish, 4)] {
            let clips = clips();
            let mut state = State::default();
            state.step(&clips, role, seen(bits), &[Cue::Begin(role)]);
            assert_eq!(
                state.frame(&clips),
                Frame {
                    phase: Phase::Start,
                    sample: 0
                }
            );
            state.advance(&clips, role, seen(bits), 3);
            assert_eq!(
                state.frame(&clips),
                Frame {
                    phase: Phase::Mid,
                    sample: 0
                }
            );
            state.advance(&clips, role, seen(bits), 5);
            assert_eq!(
                state.frame(&clips),
                Frame {
                    phase: Phase::Mid,
                    sample: 2
                }
            );
            state.step(&clips, role, seen(bits), &[Cue::End(role)]);
            assert_eq!(
                state.frame(&clips),
                Frame {
                    phase: Phase::Mid,
                    sample: 4
                }
            );
            state.advance(&clips, role, seen(bits), 2);
            assert_eq!(
                state.frame(&clips),
                Frame {
                    phase: Phase::End,
                    sample: 0
                }
            );
            state.advance(&clips, role, seen(bits), 2);
            assert_eq!(state.frame(&clips).phase, Phase::Idle);
        }
    }

    #[test]
    fn cold_cull_holds_pose_until_demanded_and_stop_cancels_when_absent() {
        let clips = clips();
        let mut state = State::default();
        state.step(&clips, Role::Start, seen(1), &[Cue::Begin(Role::Start)]);
        state.advance(&clips, Role::Start, seen(1), 1);
        let hold = state.frame(&clips);
        state.advance(&clips, Role::Start, seen(4), 100); // a different role can be visible
        assert_eq!(state.frame(&clips), hold);
        state.step(
            &clips,
            Role::Start,
            Demand::default(),
            &[Cue::End(Role::Start)],
        );
        assert_eq!(state.frame(&clips), hold); // missing input is distinct from known cull
        state.step(&clips, Role::Start, seen(0), &[]);
        assert_eq!(state.frame(&clips).phase, Phase::Idle);

        state.step(&clips, Role::Start, seen(1), &[Cue::Begin(Role::Danger)]);
        assert_eq!(state.frame(&clips).phase, Phase::Idle);
        state.step(&clips, Role::Start, seen(1), &[Cue::Begin(Role::Start)]);
        state.step(
            &clips,
            Role::Start,
            seen(1),
            &[Cue::Begin(Role::Start), Cue::Reset],
        );
        assert_eq!(state.frame(&clips).phase, Phase::Idle);
    }

    #[test]
    fn reverse_seek_and_flashback_cut_follow_recorded_forward_pose() {
        let clips = clips();
        let role = Role::Start;
        let mut history = History::default();
        history.publish(2, 0, seen(1));
        history.record(2, 1, true, Some(1));
        let seed = Seed {
            tick: 1,
            state: State::default(),
            demand: seen(1),
        };
        let cues = [(2, Cue::Begin(role)), (7, Cue::End(role))];
        for tick in 2..=9 {
            if tick == 7 {
                history.publish(2, 6, seen(0));
            }
            history.record(2, tick, true, Some(1));
        }
        let archive = |h: &History, t| {
            replay_at(
                &clips,
                role,
                seed,
                t,
                cues.into_iter(),
                h.recorded(t).unwrap().samples(),
            )
            .unwrap()
        };
        let before = archive(&history, 6);
        assert_eq!(before.state.frame(&clips).phase, Phase::Mid);
        assert_eq!(archive(&history, 8).state.frame(&clips).phase, Phase::Idle);
        assert_eq!(
            archive(&history, 6).state.frame(&clips),
            before.state.frame(&clips)
        );

        history.cut(2, 6);
        assert!(history.recorded(7).is_none());
        let cut = archive(&history, 6);
        assert_eq!(cut.state, before.state);
        history.publish(2, 6, seen(1));
        history.record(2, 7, true, Some(1));
        let next = replay_at(
            &clips,
            role,
            cut,
            7,
            std::iter::empty(),
            history.recorded(7).unwrap().samples(),
        )
        .unwrap();
        assert_eq!(next.state.frame(&clips).phase, Phase::Mid);
        assert!(replay_at(&clips, role, cut, 5, std::iter::empty(), std::iter::empty()).is_none());
    }

    #[test]
    fn sparse_seek_matches_each_fixed_tick_and_rejects_an_unusable_clock() {
        let clips = clips();
        let role = Role::Danger;
        let seed = Seed {
            tick: 0,
            state: State::default(),
            demand: Demand::default(),
        };
        let cues = [
            (2, Cue::Begin(role)),
            (4, Cue::End(Role::Finish)),
            (15, Cue::End(role)),
            (16, Cue::Begin(role)),
            (27, Cue::Reset),
        ];
        let changes = [
            Sample {
                tick: 1,
                demand: seen(2),
            },
            Sample {
                tick: 5,
                demand: seen(0),
            },
            Sample {
                tick: 11,
                demand: seen(2),
            },
            Sample {
                tick: 21,
                demand: Demand::default(),
            },
            Sample {
                tick: 22,
                demand: seen(2),
            },
        ];
        let mut sequential = seed.state;
        let mut demand = seed.demand;
        for tick in 1..=33 {
            if let Some(change) = changes.iter().find(|change| change.tick == tick) {
                demand = change.demand;
            }
            let events: Vec<_> = cues
                .iter()
                .filter_map(|&(t, cue)| (t == tick).then_some(cue))
                .collect();
            sequential.step(&clips, role, demand, &events);
            assert_eq!(
                replay_at(
                    &clips,
                    role,
                    seed,
                    tick,
                    cues.into_iter(),
                    changes.into_iter()
                )
                .unwrap()
                .state,
                sequential,
            );
        }
        assert!(
            Clips::new(
                [Clip {
                    duration: 1.,
                    samples: 1
                }; 4],
                f64::NAN
            )
            .is_none()
        );
        assert!(
            Clips::new(
                [Clip {
                    duration: 1.,
                    samples: 0
                }; 4],
                1.
            )
            .is_none()
        );
    }

    #[test]
    fn pending_action_reenters_after_a_stop_and_long_spans_are_bounded() {
        let clips = clips();
        let mut state = State::default();
        state.step(&clips, Role::Finish, seen(4), &[Cue::Begin(Role::Finish)]);
        state.advance(&clips, Role::Finish, seen(4), 3);
        state.step(&clips, Role::Finish, seen(4), &[Cue::End(Role::Finish)]);
        state.step(&clips, Role::Finish, seen(4), &[Cue::Begin(Role::Finish)]);
        state.advance(&clips, Role::Finish, seen(4), 4);
        assert_eq!(
            state.frame(&clips),
            Frame {
                phase: Phase::Start,
                sample: 0
            }
        );
        state.advance(&clips, Role::Finish, seen(4), u64::MAX);
        assert_eq!(state.frame(&clips).phase, Phase::Mid);
    }
}
