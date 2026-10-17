//! Optional three-lap event presentation; free practice remains available.
use bevy::prelude::Resource;
use grid_tools::time_trial::{Key, Mode, Phase, Records, State};

#[derive(Resource)]
pub(crate) struct Trial {
    pub state: State,
    records: Option<Records>,
    key: Key,
    save_failed: bool,
    /// Runtime tick at GO. The vehicle also ticks during the countdown, while
    /// the event clock and lap start times count from GO.
    base: Option<u64>,
    /// Race against opponents: no personal-best presentation.
    pub race: bool,
}
impl Trial {
    pub fn new(laps: u32, mode: Mode, persist: bool) -> Result<Self, Box<dyn std::error::Error>> {
        let state = State::new(laps, f64::from(grid_tools::source_vehicle::DELTA), 3.)?;
        let records = if persist {
            let load = || -> Result<Records, Box<dyn std::error::Error>> {
                let dir = crate::settings::data_dir().join("playtest");
                std::fs::create_dir_all(&dir)?;
                Records::load(dir.join("time-trial-records.tsv"))
            };
            match load() {
                Ok(records) => Some(records),
                Err(error) => {
                    eprintln!("drive_trial_records event=load_failed error={error}");
                    None
                }
            }
        } else {
            None
        };
        println!(
            "drive_trial_event version=1 type=countdown laps={laps} mode={} persistent={}",
            mode.name(),
            records.is_some()
        );
        Ok(Self {
            state,
            save_failed: persist && records.is_none(),
            records,
            key: Key { laps, mode },
            base: None,
            race: false,
        })
    }
    /// The clock for a Flashback checkpoint (docs/FLASHBACK.md); the
    /// records file is not part of it.
    pub fn checkpoint(&self) -> (State, Option<u64>) {
        (self.state.clone(), self.base)
    }
    pub fn restore(&mut self, (state, base): &(State, Option<u64>)) {
        self.state = state.clone();
        self.base = *base;
    }
    pub fn driving(&self) -> bool {
        matches!(self.state.phase(), Phase::Running)
    }
    pub fn finished(&self) -> bool {
        matches!(self.state.phase(), Phase::Finished(_))
    }
    pub fn countdown(&mut self) {
        let before = self.state.countdown_label();
        self.state.countdown_tick();
        if before != self.state.countdown_label() {
            log_line!(
                "drive_trial_event version=1 type=countdown remaining={}",
                self.state
                    .countdown_label()
                    .map_or_else(|| "GO".into(), |s| s.to_string())
            );
        }
    }
    /// Race time (s from GO) at runtime tick `tick`; keeps counting after the
    /// player's finish while the opponents race on. `None` before GO.
    pub fn race_time(&self, tick: u64) -> Option<f64> {
        self.base.map(|base| {
            tick.saturating_sub(base) as f64 * f64::from(grid_tools::source_vehicle::DELTA)
        })
    }
    pub fn reset(&mut self) {
        self.state.reset();
        self.base = None;
        log_line!(
            "drive_trial_event version=1 type=reset laps={}",
            self.key.laps
        );
    }
    pub fn tick(
        &mut self,
        tick: u64,
        events: &[grid_tools::lap_progress::Event],
    ) -> Result<(), grid_formats::Error> {
        use grid_tools::lap_progress::Event;
        let failed = grid_formats::Error {
            offset: 0,
            message: "time-trial state failed after successful vehicle tick",
        };
        let base = *self
            .base
            .get_or_insert(tick.checked_sub(1).ok_or(failed.clone())?);
        let shift = base as f64 * f64::from(grid_tools::source_vehicle::DELTA);
        let events: Vec<_> = events
            .iter()
            .map(|event| match event {
                Event::Started { when } => Event::Started { when: when - shift },
                Event::Checkpoint { gate, when } => Event::Checkpoint {
                    gate: *gate,
                    when: when - shift,
                },
                other => other.clone(),
            })
            .collect();
        let old = self.state.completed_laps();
        self.state
            .finish_tick(tick - base, &events)
            .map_err(|_| failed)?;
        if self.state.completed_laps() != old {
            log_line!(
                "drive_trial_event version=1 type=lap tick={tick} completed={} target={}",
                self.state.completed_laps(),
                self.key.laps
            );
        }
        if let Phase::Finished(result) = self.state.phase() {
            log_line!(
                "drive_trial_event version=1 type=finish tick={tick} laps={} seconds={:.9} best_lap={:.9} invalid_attempts={}",
                result.laps.len(),
                result.seconds,
                result.best_lap(),
                result.invalid_attempts
            );
            if let Some(records) = &mut self.records {
                match records.record(self.key, result, super::BUILD_REVISION) {
                    Ok(()) => {
                        self.save_failed = false;
                        log_line!(
                            "drive_trial_records event=saved mode={} laps={}",
                            self.key.mode.name(),
                            self.key.laps
                        );
                    }
                    Err(error) => {
                        self.save_failed = true;
                        eprintln!("drive_trial_records event=save_failed error={error}");
                    }
                }
            }
        }
        Ok(())
    }
    /// After [`Self::tick`] refused `tick`: count the lap as an invalid attempt
    /// and keep the clock running (see `State::skip_tick`). Returns whether
    /// the lap now in progress is still timed.
    pub fn invalidate(&mut self, tick: u64, events: &[grid_tools::lap_progress::Event]) -> bool {
        use grid_tools::lap_progress::Event;
        let base = *self.base.get_or_insert(tick.saturating_sub(1));
        let shift = base as f64 * f64::from(grid_tools::source_vehicle::DELTA);
        let events: Vec<_> = events
            .iter()
            .map(|event| match event {
                Event::Started { when } => Event::Started { when: when - shift },
                other => other.clone(),
            })
            .collect();
        let timed = self.state.skip_tick(tick.saturating_sub(base), &events);
        log_line!("drive_trial_event version=1 type=lap_invalidated tick={tick} timed={timed}");
        timed
    }
    pub fn hud(&self) -> String {
        let best = self.records.as_ref().and_then(|r| r.best(self.key));
        let previous = best.map_or_else(
            || "Personal best  --".into(),
            |b| {
                format!(
                    "PB lap  {}\nPB event  {}",
                    super::lap_time(b.lap_ns as f64 / 1e9),
                    super::lap_time(b.event_ns as f64 / 1e9)
                )
            },
        );
        let body = match self.state.phase() {
            Phase::Countdown(_) => format!(
                "{} laps - ready in {}\nHeld on the handbrake: hold throttle to rev, launch at GO",
                self.key.laps,
                self.state.countdown_label().unwrap()
            ),
            Phase::Running => format!(
                "{}Lap {}/{}   Event {}",
                if self.state.elapsed() < 1. {
                    "GO!\n"
                } else {
                    ""
                },
                self.state.completed_laps() + 1,
                self.key.laps,
                super::lap_time(self.state.elapsed())
            ),
            Phase::Finished(result) => {
                let mut rows = format!("FINISHED\nEvent  {}", super::lap_time(result.seconds));
                for (i, lap) in result.laps.iter().take(6).enumerate() {
                    rows.push_str(&format!(
                        "\nLap {}  {}",
                        i + 1,
                        super::lap_time(lap.seconds)
                    ));
                }
                if result.laps.len() > 6 {
                    rows.push_str(&format!("\n... {} more laps", result.laps.len() - 6));
                }
                if result.invalid_attempts > 0 {
                    rows.push_str(&format!("\nInvalid attempts  {}", result.invalid_attempts));
                }
                rows.push_str("\nR: restart event | Escape: exit");
                rows
            }
        };
        if self.race {
            return body;
        }
        format!(
            "{body}\n{previous}{}",
            if self.save_failed {
                "\nBest times unavailable"
            } else {
                ""
            }
        )
    }
    pub fn title(&self) -> String {
        match self.state.phase() {
            Phase::Countdown(_) => format!(" | READY {}", self.state.countdown_label().unwrap()),
            Phase::Running => format!(
                " | LAP {}/{} | EVENT {}",
                self.state.completed_laps() + 1,
                self.key.laps,
                super::lap_time(self.state.elapsed())
            ),
            Phase::Finished(r) => format!(" | FINISHED {}", super::lap_time(r.seconds)),
        }
    }
}
