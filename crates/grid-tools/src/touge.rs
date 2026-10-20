//! Pro Touge (docs/TOUGE.md §7): two runs of a Haruna road, the second in
//! the reverse direction with the start order swapped, collision penalties
//! added to each run, and the result by total time.
//!
//! [V] from the executable (static analysis, F412):
//! - `gameconfig/touge.xml` is read into the touge state's settings
//!   (state +0xB0; reader 0x630FD0): minor and major penalty seconds,
//!   `ignoreCollisionTime`, `alongsideLengthOffset`,
//!   `alongsideThresholdTweakMax`.
//! - The touge state's round index (+4) is 0 in run 1 and selects subroute
//!   1 when it is 1 (0x65B2C0); per-car penalty counters are indexed by it.
//! - A contact with the other touge car opens a penalty window of
//!   `ignoreCollisionTime` when none is open (0x631210 minor, 0x631240
//!   major); a damaging contact (the impact handler's applied-damage byte
//!   +0xBA5, set on either car) makes it major, also inside an open window.
//! - Each tick (pro touge only, state +8 == 0; 0x64E1E0) the window counts
//!   down; when it runs out a pending penalty counts once, minor or major,
//!   for the current run (only while the car still races, +0x2E64 [I]).
//! - A run's penalty time is minor × minorPenaltyTimePenalty + major ×
//!   majorPenaltyTimePenalty (0x63C830).
//!
//! AUTHORED / [I]:
//! - Which car of a contact is penalised: the original decides with the
//!   alongside geometry of 0x6441B0/0x63C940 (not ported); here the car
//!   behind on race progress takes the penalty, both cars when they are
//!   alongside (closer than `ALONGSIDE` m).
//! - Run 2 swaps the two grid slots [I: "swapped start order"].
//! - The result orders by total time (both runs plus penalties); a car
//!   without both runs comes last [I: the post-race "Round 1 / Round 2 /
//!   Total Times" columns].
use std::path::Path;

/// `gameconfig/touge.xml` (`TougeSettings`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub minor_penalty: f32,
    pub major_penalty: f32,
    pub ignore_collision_time: f32,
    pub alongside_length_offset: f32,
    pub alongside_threshold_tweak_max: f32,
}

impl Settings {
    pub fn load(root: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = std::fs::read(root.join("gameconfig/touge.xml"))?;
        let doc = grid_formats::binxml::read(&bytes)?;
        let e = doc.elements.first().ok_or("touge.xml empty")?;
        if e.name != "TougeSettings" {
            return Err("expected TougeSettings".into());
        }
        let get = |name: &str| -> Result<f32, Box<dyn std::error::Error>> {
            let v: f32 = e
                .attributes
                .iter()
                .find(|(k, _)| *k == name)
                .ok_or_else(|| format!("touge.xml {name} absent"))?
                .1
                .parse()?;
            if !v.is_finite() || v < 0. {
                return Err(format!("touge.xml {name} out of range").into());
            }
            Ok(v)
        };
        Ok(Self {
            minor_penalty: get("minorPenaltyTimePenalty")?,
            major_penalty: get("majorPenaltyTimePenalty")?,
            ignore_collision_time: get("ignoreCollisionTime")?,
            alongside_length_offset: get("alongsideLengthOffset")?,
            alongside_threshold_tweak_max: get("alongsideThresholdTweakMax")?,
        })
    }
}

/// AUTHORED: cars closer than this along the road are alongside, and a
/// contact between them penalises both.
pub const ALONGSIDE: f64 = 4.5;

/// One car's penalty record (the touge state's 0x50-byte car record).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Penalties {
    /// Open window (record +0x38), seconds.
    window: f64,
    /// A penalty waits for the window's end (+0x40), and whether it is
    /// major (+0x41).
    pending: bool,
    major: bool,
    /// Minor and major counts per run (record +0x1C/+0x20 + 12 × run).
    pub counts: [[u32; 2]; 2],
}

impl Penalties {
    /// A contact with the other touge car; `damaged` is the applied-damage
    /// byte of either car (0x631210 / 0x631240).
    pub fn contact(&mut self, settings: &Settings, damaged: bool) {
        if self.window <= 0. {
            self.window = f64::from(settings.ignore_collision_time);
            self.pending = true;
            self.major = damaged;
        } else if damaged && !self.major && self.pending {
            self.major = true;
        }
    }

    /// The window counts down; at its end a pending penalty counts for run
    /// `run` while the car still `racing` (0x64E1E0).
    pub fn tick(&mut self, dt: f64, run: usize, racing: bool) {
        if self.window > 0. {
            self.window -= dt;
        }
        if self.window < 0. {
            self.window = 0.;
            if self.pending {
                if racing {
                    self.counts[run][usize::from(self.major)] += 1;
                }
                self.pending = false;
                self.major = false;
            }
        }
    }

    /// New run: no open window (0x63C710 keeps the counts).
    pub fn reset_window(&mut self) {
        self.window = 0.;
        self.pending = false;
        self.major = false;
    }

    /// Penalty seconds of run `run` (0x63C830).
    pub fn seconds(&self, settings: &Settings, run: usize) -> f64 {
        let [minor, major] = self.counts[run];
        f64::from(minor as f32 * settings.minor_penalty + major as f32 * settings.major_penalty)
    }
}

/// Which cars of a contact between `a` and `b` (race progress in metres)
/// take a penalty (AUTHORED, see the module notes).
pub fn penalised(progress_a: f64, progress_b: f64) -> (bool, bool) {
    if (progress_a - progress_b).abs() < ALONGSIDE {
        (true, true)
    } else {
        (progress_a < progress_b, progress_b < progress_a)
    }
}

/// One car's result over both runs.
#[derive(Clone, Debug, PartialEq)]
pub struct Standing {
    pub car: usize,
    /// Run times without penalties; `None` when the run did not finish.
    pub runs: [Option<f64>; 2],
    pub penalties: [f64; 2],
}

impl Standing {
    /// Run time plus its penalty.
    pub fn run_total(&self, run: usize) -> Option<f64> {
        self.runs[run].map(|t| t + self.penalties[run])
    }
    pub fn total(&self) -> Option<f64> {
        Some(self.run_total(0)? + self.run_total(1)?)
    }
}

/// Results ordered by total time; cars without both runs last, by the
/// runs they finished, then car order.
pub fn classify(mut results: Vec<Standing>) -> Vec<Standing> {
    results.sort_by(|a, b| match (a.total(), b.total()) {
        (Some(x), Some(y)) => x.total_cmp(&y).then(a.car.cmp(&b.car)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => {
            let done = |r: &Standing| r.runs.iter().flatten().count();
            done(b).cmp(&done(a)).then(a.car.cmp(&b.car))
        }
    });
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> Settings {
        Settings {
            minor_penalty: 2.,
            major_penalty: 10.,
            ignore_collision_time: 2.,
            alongside_length_offset: 0.05,
            alongside_threshold_tweak_max: 0.2,
        }
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn the_shipped_touge_settings_are_read() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        assert_eq!(Settings::load(Path::new(&root)).unwrap(), shipped());
    }

    #[test]
    fn contacts_within_the_window_count_once_and_damage_makes_them_major() {
        let s = shipped();
        let mut p = Penalties::default();
        // Three light touches inside 2 s: one minor penalty when it closes.
        for _ in 0..3 {
            p.contact(&s, false);
            p.tick(0.3, 0, true);
        }
        assert_eq!(p.counts, [[0, 0]; 2]);
        for _ in 0..4 {
            p.tick(0.3, 0, true);
        }
        assert_eq!(p.counts[0], [1, 0]);
        // A light touch, then a damaging hit in the same window: one major.
        p.contact(&s, false);
        p.tick(0.3, 1, true);
        p.contact(&s, true);
        for _ in 0..7 {
            p.tick(0.3, 1, true);
        }
        assert_eq!(p.counts[1], [0, 1]);
        assert_eq!(p.seconds(&s, 0), 2.);
        assert_eq!(p.seconds(&s, 1), 10.);
        // A finished car is not penalised.
        let mut q = Penalties::default();
        q.contact(&s, true);
        for _ in 0..8 {
            q.tick(0.3, 0, false);
        }
        assert_eq!(q.counts, [[0, 0]; 2]);
    }

    #[test]
    fn the_car_behind_is_penalised_unless_alongside() {
        assert_eq!(penalised(100., 120.), (true, false));
        assert_eq!(penalised(120., 100.), (false, true));
        assert_eq!(penalised(100., 102.), (true, true));
    }

    #[test]
    fn results_order_by_total_time_with_penalties() {
        let r = |car, runs: [Option<f64>; 2], penalties| Standing {
            car,
            runs,
            penalties,
        };
        let order = classify(vec![
            r(0, [Some(150.), Some(140.)], [0., 10.]),
            r(1, [Some(152.), Some(141.)], [2., 0.]),
            r(2, [Some(149.), None], [0., 0.]),
        ]);
        assert_eq!(order.iter().map(|r| r.car).collect::<Vec<_>>(), [1, 0, 2]);
        assert_eq!(order[0].total(), Some(295.));
        assert_eq!(order[1].run_total(1), Some(150.));
    }
}

/// Inputs of a headless Pro Touge between two AI cars.
pub struct Event<'a> {
    pub root: &'a Path,
    /// A point-to-point route with both subroutes (Haruna 1-8).
    pub route: &'a crate::route::Route,
    pub cars: [&'a crate::car::Car; 2],
    pub table: &'a [u8],
    pub events: Option<grid_world::body_impact::Parameters>,
    pub deformation: Option<crate::source_vehicle::multi::DeformationParameters>,
    /// Driver characteristics, row k for car k (empty: 1.0).
    pub characteristics: &'a [std::collections::BTreeMap<String, f32>],
    /// Tick budget of each run.
    pub ticks: u64,
}

/// One run's report: finish times (`None` unfinished), penalty counts and
/// the ticks it took.
#[derive(Clone, Debug)]
pub struct Run {
    pub times: [Option<f64>; 2],
    pub ticks: u64,
    pub contacts: usize,
}

/// The runs, the final penalties and the classification of a Pro Touge.
pub type Outcome = (Vec<Run>, [Penalties; 2], Vec<Standing>);

/// Both runs of a Pro Touge; car k keeps its index, run 2 starts it from the
/// other grid slot in the reverse direction. Returns the runs, the final
/// penalties and the classification.
pub fn run_pro_touge(
    e: &Event<'_>,
) -> std::result::Result<Outcome, Box<dyn std::error::Error>> {
    use crate::ai_driver::{AiData, AiDriver, NORMAL_DIFFICULTY, Other, Parameters, View};
    use crate::race::Status;
    use crate::source_vehicle::{DELTA, multi::MultiRuntime};
    let settings = Settings::load(e.root)?;
    let mut penalties = [Penalties::default(), Penalties::default()];
    let mut runs = Vec::new();
    for run in 0..2usize {
        let route = e.route.clone().with_subroute(run as u8)?;
        let slots = if run == 0 { ["slot_00", "slot_01"] } else { ["slot_01", "slot_00"] };
        let entries = [(slots[0], e.cars[0]), (slots[1], e.cars[1])];
        let mut multi = MultiRuntime::load_deforming_on(
            e.root,
            &route,
            &entries,
            e.table,
            e.events,
            e.deformation.clone(),
        )?;
        let data: Vec<AiData> = e
            .cars
            .iter()
            // The pro_touge section of ai_driver_config.xml [D].
            .map(|c| AiData::load_route(e.root, &route, c, Some("pro_touge")))
            .collect::<std::result::Result<_, _>>()?;
        let mut drivers: Vec<AiDriver> = (0..2)
            .map(|i| -> std::result::Result<AiDriver, String> {
                let row = e.characteristics.get(i % e.characteristics.len().max(1));
                let p = Parameters::new(&data[i], NORMAL_DIFFICULTY, |name| {
                    row.and_then(|r| r.get(name).copied()).unwrap_or(1.)
                })?;
                Ok(AiDriver::new(p, 0x5eed + i as u32))
            })
            .collect::<std::result::Result<_, _>>()?;
        let progress = crate::lap_progress::load_route(e.root, &route)?.progress;
        if progress.kind != grid_formats::progress::TrackKind::PointToPoint {
            return Err("Pro Touge needs a point-to-point route".into());
        }
        let centre = |g: &grid_formats::progress::Gate| -> [f32; 3] {
            std::array::from_fn(|k| (g.left[k] + g.right[k]) * 0.5)
        };
        let positions = |m: &MultiRuntime| -> Vec<[f32; 3]> {
            m.cars
                .iter()
                .map(|c| {
                    let p = c.pose();
                    [p[12], p[13], p[14]]
                })
                .collect()
        };
        let line = &data[0].line;
        let mut standings = crate::race::Standings::new_run(
            line,
            centre(&progress.gates[0]),
            centre(progress.gates.last().unwrap()),
            &positions(&multi),
        );
        let length = f64::from(progress.total_distance);
        for p in &mut penalties {
            p.reset_window();
        }
        let mut contacts = 0;
        while multi.ticks < e.ticks && !standings.over() {
            let others: Vec<Other> = multi
                .cars
                .iter()
                .enumerate()
                .map(|(i, c)| Other::of(i as u32 + 1, c))
                .collect();
            let controls: Vec<_> = (0..2)
                .map(|i| {
                    let view = View::of(&multi.cars[i], multi.obstruction(i));
                    drivers[i].controls(&data[i], &view, &[others[1 - i]], DELTA)
                })
                .collect();
            for (car, driver) in multi.cars.iter_mut().zip(drivers.iter()) {
                car.set_ai_packet(driver.reverse_request());
            }
            multi.tick(&controls)?;
            for &i in multi.newly_retired() {
                standings.retire(i, crate::race::Out::Wrecked);
            }
            let time = multi.ticks as f64 * f64::from(DELTA);
            standings.update(line, &positions(&multi), time, &[]);
            if !multi.contact_pairs().is_empty() {
                contacts += 1;
                // The impact handler's applied-damage byte (+0xBA5) of
                // either car after this tick.
                let damaged = multi
                    .cars
                    .iter()
                    .any(|c| (c.state.car.owner.vehicle[0xba5 / 4] >> 8) & 255 != 0);
                let at = |i| standings.laps_covered(i) * length;
                let (a, b) = penalised(at(0), at(1));
                for (i, hit) in [a, b].into_iter().enumerate() {
                    if hit {
                        penalties[i].contact(&settings, damaged);
                    }
                }
            }
            for (i, p) in penalties.iter_mut().enumerate() {
                p.tick(f64::from(DELTA), run, standings.status(i) == Status::Racing);
            }
        }
        runs.push(Run {
            times: [0, 1].map(|i| standings.finished(i)),
            ticks: multi.ticks,
            contacts,
        });
    }
    let standings = classify(
        (0..2)
            .map(|car| Standing {
                car,
                runs: [runs[0].times[car], runs[1].times[car]],
                penalties: [0, 1].map(|r| penalties[car].seconds(&settings, r)),
            })
            .collect(),
    );
    Ok((runs, penalties, standings))
}
