//! Independently authored finite practice event over validated lap events.
//! Not original GRID race rules. Countdown freezes the car; racing uses tick time.
use crate::{
    Error,
    lap_timing::{Completed, Event},
};

#[derive(Clone, Debug, PartialEq)]
pub struct Results {
    pub seconds: f64,
    pub laps: Vec<Completed>,
    pub invalid_attempts: u32,
}
impl Results {
    pub fn best_lap(&self) -> f64 {
        self.laps
            .iter()
            .map(|l| l.seconds)
            .fold(f64::INFINITY, f64::min)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Countdown(u32),
    Running,
    Finished(Results),
}
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    target: u32,
    delta: f64,
    countdown: u32,
    phase: Phase,
    ticks: u64,
    start: Option<f64>,
    attempts: u32,
    laps: Vec<Completed>,
    /// A time-limited event (Freestyle drift): finished when the event
    /// clock reaches it, with the laps completed so far.
    time_limit: Option<f64>,
}
impl State {
    pub fn new(target: u32, delta: f64, countdown_seconds: f64) -> Result<Self, Error> {
        if !(1..=99).contains(&target)
            || !delta.is_finite()
            || !(0.001..=0.1).contains(&delta)
            || !countdown_seconds.is_finite()
            || !(0.0..=10.).contains(&countdown_seconds)
        {
            return Err(Error("invalid time-trial configuration"));
        }
        let countdown = (countdown_seconds / delta).ceil() as u32;
        Ok(Self {
            target,
            delta,
            countdown,
            phase: if countdown == 0 {
                Phase::Running
            } else {
                Phase::Countdown(countdown)
            },
            ticks: 0,
            start: None,
            attempts: 0,
            laps: Vec::new(),
            time_limit: None,
        })
    }
    /// The event ends `seconds` after GO whatever the laps (Freestyle drift:
    /// 90 s in the career [D], 1-10 minutes in a free race).
    pub fn with_time_limit(mut self, seconds: f64) -> Result<Self, Error> {
        if !seconds.is_finite() || !(1. ..=86400.).contains(&seconds) {
            return Err(Error("invalid time-trial time limit"));
        }
        self.time_limit = Some(seconds);
        Ok(self)
    }
    pub fn time_limit(&self) -> Option<f64> {
        self.time_limit
    }
    pub fn phase(&self) -> &Phase {
        &self.phase
    }
    pub fn target(&self) -> u32 {
        self.target
    }
    pub fn completed_laps(&self) -> usize {
        self.laps.len()
    }
    pub fn elapsed(&self) -> f64 {
        match &self.phase {
            Phase::Countdown(_) => 0.,
            Phase::Running => self.ticks as f64 * self.delta,
            Phase::Finished(r) => r.seconds,
        }
    }
    pub fn countdown_label(&self) -> Option<u32> {
        if let Phase::Countdown(left) = self.phase {
            Some(((f64::from(left.saturating_sub(1)) * self.delta).ceil() as u32).max(1))
        } else {
            None
        }
    }
    /// Called only on unpaused fixed updates. No vehicle tick is performed here.
    pub fn countdown_tick(&mut self) {
        if let Phase::Countdown(left) = self.phase {
            self.phase = if left == 1 {
                Phase::Running
            } else {
                Phase::Countdown(left - 1)
            };
        }
    }
    pub fn reset(&mut self) {
        self.phase = if self.countdown == 0 {
            Phase::Running
        } else {
            Phase::Countdown(self.countdown)
        };
        self.ticks = 0;
        self.start = None;
        self.attempts = 0;
        self.laps.clear();
    }
    /// Supplied tick must be the next successful vehicle tick. Stage before commit.
    pub fn finish_tick(&mut self, tick: u64, events: &[Event]) -> Result<(), Error> {
        if !matches!(self.phase, Phase::Running)
            || self.ticks.checked_add(1) != Some(tick)
            || tick as f64 * self.delta > 86400.
        {
            return Err(Error("invalid time-trial tick"));
        }
        let mut next = self.clone();
        next.ticks = tick;
        let end = tick as f64 * self.delta;
        let begin = (tick - 1) as f64 * self.delta;
        for event in events {
            match event {
                Event::Started { when } => {
                    if next.start.is_some()
                        || !when.is_finite()
                        || *when < begin - 1e-7
                        || *when > end + 1e-7
                    {
                        return Err(Error("invalid time-trial lap start"));
                    }
                    next.start = Some(*when);
                }
                Event::Finished(lap) => {
                    let start = next.start.ok_or(Error("time-trial finish without start"))?;
                    let finish = start + lap.seconds;
                    if next.attempts.checked_add(1) != Some(lap.number)
                        || !lap.seconds.is_finite()
                        || lap.seconds <= 0.
                        || !finish.is_finite()
                        || finish < begin - 1e-7
                        || finish > end + 1e-7
                        || (lap.valid
                            && (lap.missed_gate.is_some()
                                || lap.sectors.is_empty()
                                || lap
                                    .sectors
                                    .iter()
                                    .any(|v| v.is_none_or(|s| !s.is_finite() || s <= 0.))))
                    {
                        return Err(Error("invalid time-trial completed lap"));
                    }
                    next.attempts = lap.number;
                    next.start = None;
                    if lap.valid {
                        next.laps.push(lap.clone());
                    }
                    if next.laps.len() == next.target as usize {
                        next.phase = Phase::Finished(Results {
                            seconds: finish,
                            laps: next.laps.clone(),
                            invalid_attempts: next.attempts - next.target,
                        });
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(limit) = next.time_limit
            && matches!(next.phase, Phase::Running)
            && end >= limit - 1e-9
        {
            next.phase = Phase::Finished(Results {
                seconds: end,
                laps: next.laps.clone(),
                invalid_attempts: next.attempts.saturating_sub(next.laps.len() as u32),
            });
        }
        *self = next;
        Ok(())
    }
    /// Recovery after [`Self::finish_tick`] refused `tick` (authored; the
    /// original has no such path): the clock moves on to `tick`, a lap
    /// finished in it counts as an invalid attempt, and a lap start in it is
    /// kept only when it is valid. Returns whether a lap is still timed.
    pub fn skip_tick(&mut self, tick: u64, events: &[Event]) -> bool {
        if !matches!(self.phase, Phase::Running) {
            return false;
        }
        self.ticks = self.ticks.max(tick);
        let end = tick as f64 * self.delta;
        let begin = tick.saturating_sub(1) as f64 * self.delta;
        for event in events {
            match event {
                Event::Started { when } => {
                    self.start = (when.is_finite() && *when >= begin - 1e-7 && *when <= end + 1e-7)
                        .then_some(*when);
                }
                Event::Finished(lap) => {
                    self.attempts = self.attempts.max(lap.number);
                    self.start = None;
                }
                _ => {}
            }
        }
        self.start.is_some()
    }
}
#[cfg(test)]
mod time_limit_tests {
    use super::*;
    #[test]
    fn a_time_limited_event_finishes_at_its_limit_without_laps() {
        let mut s = State::new(1, 0.1, 0.).unwrap().with_time_limit(1.).unwrap();
        for tick in 1..=9 {
            s.finish_tick(tick, &[]).unwrap();
            assert_eq!(s.phase(), &Phase::Running);
        }
        s.finish_tick(10, &[]).unwrap();
        let Phase::Finished(r) = s.phase() else {
            panic!("{:?}", s.phase());
        };
        assert!((r.seconds - 1.).abs() < 1e-9);
        assert_eq!((r.laps.len(), r.invalid_attempts), (0, 0));
        assert!(s.finish_tick(11, &[]).is_err());
        // Without a limit the same ticks keep running.
        let mut t = State::new(1, 0.1, 0.).unwrap();
        for tick in 1..=20 {
            t.finish_tick(tick, &[]).unwrap();
        }
        assert_eq!(t.phase(), &Phase::Running);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn lap(number: u32, seconds: f64, valid: bool) -> Event {
        Event::Finished(Completed {
            number,
            seconds,
            sectors: vec![Some(seconds)],
            valid,
            missed_gate: (!valid).then_some(1),
        })
    }
    #[test]
    fn countdown_invalid_attempts_fractional_finish_and_reset() {
        assert_eq!(
            State::new(3, f64::from(0.016f32), 3.)
                .unwrap()
                .countdown_label(),
            Some(3)
        );
        let mut s = State::new(3, 0.1, 0.3).unwrap();
        let initial = s.clone();
        for _ in 0..3 {
            assert!(matches!(s.phase(), Phase::Countdown(_)));
            s.countdown_tick();
            assert_eq!(s.elapsed(), 0.);
        }
        s.finish_tick(1, &[Event::Started { when: 0.05 }]).unwrap();
        s.finish_tick(2, &[lap(1, 0.10, false), Event::Started { when: 0.15 }])
            .unwrap();
        assert_eq!(s.completed_laps(), 0);
        s.finish_tick(3, &[lap(2, 0.10, true), Event::Started { when: 0.25 }])
            .unwrap();
        s.finish_tick(4, &[lap(3, 0.10, true), Event::Started { when: 0.35 }])
            .unwrap();
        s.finish_tick(5, &[lap(4, 0.10, true), Event::Started { when: 0.45 }])
            .unwrap();
        let Phase::Finished(r) = s.phase() else {
            panic!()
        };
        assert_eq!(r.laps.len(), 3);
        assert_eq!(r.invalid_attempts, 1);
        assert!((r.seconds - 0.45).abs() < 1e-12);
        let before = s.clone();
        assert!(s.finish_tick(6, &[]).is_err());
        assert_eq!(s, before);
        s.reset();
        assert_eq!(s, initial);
    }
    #[test]
    fn missed_tick_and_late_bad_event_preserve_state() {
        let mut s = State::new(2, 0.1, 0.).unwrap();
        s.finish_tick(1, &[Event::Started { when: 0.05 }]).unwrap();
        let before = s.clone();
        assert!(s.finish_tick(3, &[]).is_err());
        assert_eq!(s, before);
        assert!(
            s.finish_tick(2, &[lap(1, 0.10, true), Event::Started { when: f64::NAN }])
                .is_err()
        );
        assert_eq!(s, before);
        assert!(s.finish_tick(2, &[lap(2, 0.10, true)]).is_err());
        assert_eq!(s, before);
    }
    #[test]
    fn skipped_tick_invalidates_the_lap_and_timing_resumes() {
        let mut s = State::new(2, 0.1, 0.).unwrap();
        s.finish_tick(1, &[Event::Started { when: 0.05 }]).unwrap();
        // A refused finish (bad start) counts as an invalid attempt; the
        // valid start after it keeps timing the next lap.
        let events = [lap(1, 0.10, true), Event::Started { when: f64::NAN }];
        assert!(s.finish_tick(2, &events).is_err());
        assert!(!s.skip_tick(2, &events));
        assert_eq!((s.ticks, s.attempts, s.completed_laps()), (2, 1, 0));
        // Without a start the next finish is refused and skipped too.
        assert!(s.finish_tick(3, &[lap(2, 0.2, true)]).is_err());
        assert!(s.skip_tick(3, &[lap(2, 0.2, true), Event::Started { when: 0.25 }]));
        s.finish_tick(4, &[lap(3, 0.10, true), Event::Started { when: 0.35 }])
            .unwrap();
        s.finish_tick(5, &[lap(4, 0.10, true)]).unwrap();
        let Phase::Finished(r) = s.phase() else {
            panic!()
        };
        assert_eq!((r.laps.len(), r.invalid_attempts), (2, 2));
        // A clock gap resynchronises instead of refusing every later tick.
        let mut s = State::new(2, 0.1, 0.).unwrap();
        assert!(s.finish_tick(3, &[]).is_err());
        assert!(!s.skip_tick(3, &[]));
        s.finish_tick(4, &[Event::Started { when: 0.35 }]).unwrap();
    }
}
