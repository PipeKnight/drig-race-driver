//! The original's virtual race: the result estimator a drift event uses for
//! a car whose score is still zero when the race ends. Research, addresses
//! and labels: docs/DISCIPLINES.md, section "Result estimator".
//!
//! Original objects [V, hash-pinned analysis image]:
//! - the virtual race (constructor 0x67D590, vtable 0xCE87FC; debug strings
//!   "Virtual race not initialised", "Competitor not in virtual race",
//!   `virtualRace::updateVirtualLapBaseResult`) at race world +0x21A0 →
//!   +0xF2D8;
//! - its set-up from the event (0x646320 free race, 0x646A90 career): the
//!   `difficulty` row's `base_time_factor` (+0x10), `top_time_factor`
//!   (+0x14) and `performance_variance` (+0x18) [V row reader 0x60A7B0], the
//!   track pool's `base_result_list` summed into +0x20 and the format's
//!   `time_based` flag (+0xAB8) (0x680CE0);
//! - add a competitor with its skill clamped to 0..1 (0x67FF20), its result
//!   (0x6850F0 → 0x67D6C0 → 0x680B20), remove it (0x67D660);
//! - the skill of a driver (0x62B170): `ai_driver.Virtual_Performance`, 1.0
//!   for a car without a driver row;
//! - results are 64-bit microseconds (0x908450 from seconds × 1e6 truncated,
//!   0x908820 back × 1e-6).
//!
//! Only the score branch (`time_based` = 0, the drift formats) is ported;
//! the time branch scales by championship counts not needed here.
//!
//! [`LapEstimator`] is the same object's lap estimator of a circuit race
//! (0x67FFB0; docs/RACE_RULES_RESEARCH.md section 1).
use crate::ai_pacing::Mt;

/// One `difficulty` row of `database.bin` [D].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Difficulty {
    pub career_rank: u8,
    pub star_level: u8,
    pub ai_ratio: f32,
    pub base_time_factor: f32,
    pub top_time_factor: f32,
    pub performance_variance: f32,
}

const fn row(
    career_rank: u8,
    star_level: u8,
    ai_ratio: f32,
    base_time_factor: f32,
    top_time_factor: f32,
    performance_variance: f32,
) -> Difficulty {
    Difficulty {
        career_rank,
        star_level,
        ai_ratio,
        base_time_factor,
        top_time_factor,
        performance_variance,
    }
}

/// [D] `difficulty`, ids 1..24 (career rank 1..4 × star level 0..5).
pub const DIFFICULTY: [Difficulty; 24] = [
    row(1, 0, 0.475, 99., 33., 0.11),
    // 49.799999 in the decoded table: the same f32 as 49.8.
    row(1, 1, 0.575, 49.8, 16.6, 0.11),
    row(1, 2, 0.675, 16., 8., 0.11),
    row(1, 3, 0.775, 12., 4., 0.11),
    row(1, 4, 0.88, 7.5, 2.5, 0.11),
    row(1, 5, 1., 3.45, 1.15, 0.11),
    row(2, 0, 0.506, 93., 31., 0.13),
    row(2, 1, 0.606, 43., 14., 0.13),
    row(2, 2, 0.706, 15.4, 7.7, 0.13),
    row(2, 3, 0.8, 11.4, 3.8, 0.13),
    row(2, 4, 0.9, 7.2, 2.4, 0.13),
    row(2, 5, 1., 3.3, 1.1, 0.13),
    row(3, 0, 0.542, 87., 29., 0.15),
    row(3, 1, 0.642, 42., 14., 0.15),
    row(3, 2, 0.742, 14.8, 7.4, 0.15),
    row(3, 3, 0.825, 10.8, 3.6, 0.15),
    row(3, 4, 0.92, 6.9, 2.3, 0.15),
    row(3, 5, 1., 3.15, 1.05, 0.15),
    row(4, 0, 0.575, 81., 27., 0.17),
    row(4, 1, 0.675, 30., 9., 0.17),
    row(4, 2, 0.775, 14.2, 7.1, 0.17),
    row(4, 3, 0.85, 10.2, 3.4, 0.17),
    row(4, 4, 0.94, 6.6, 2.2, 0.17),
    row(4, 5, 1., 3., 1., 0.17),
];

/// The row of a career rank (1..4) and star level (0..5).
pub fn difficulty(career_rank: usize, star_level: usize) -> &'static Difficulty {
    let rank = career_rank.clamp(1, 4);
    &DIFFICULTY[(rank - 1) * 6 + star_level.min(5)]
}

/// Free-race drift formats with track pools.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    DriftBattle,
    DriftGrandPrix,
    DriftFreestyle,
}

impl Format {
    /// `competition_format.format` [D].
    pub fn name(self) -> &'static str {
        match self {
            Format::DriftBattle => "drift_battle",
            Format::DriftGrandPrix => "drift_grand_prix",
            Format::DriftFreestyle => "drift_freestyle",
        }
    }
    pub fn from_name(name: &str) -> Option<Self> {
        [
            Format::DriftBattle,
            Format::DriftGrandPrix,
            Format::DriftFreestyle,
        ]
        .into_iter()
        .find(|f| f.name() == name)
    }
}

/// One `free_race_track_pool` row of a drift discipline [D]: route folder
/// (`tracks/<region>/<track>/<route>`, lower case), `max_cars` and the first
/// `base_result_list` value (the other three are 0 in every drift row).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pool {
    pub format: Format,
    pub route: &'static str,
    pub max_cars: u32,
    pub base_result: f32,
}

const fn pool(format: Format, route: &'static str, max_cars: u32, base_result: f32) -> Pool {
    Pool {
        format,
        route,
        max_cars,
        base_result,
    }
}

use Format::{DriftBattle as B, DriftFreestyle as F, DriftGrandPrix as G};

/// [D] `free_race_track_pool` rows of `drift_battle`, `drift_gp` and
/// `Free_Drift`.
pub const DRIFT_POOLS: [Pool; 53] = [
    pool(B, "tracks/europe/istanbul_park/route_1", 8, 3_210_000.),
    pool(B, "tracks/europe/istanbul_park/route_3", 8, 3_210_000.),
    pool(B, "tracks/europe/jarama/route_0", 8, 3_420_000.),
    pool(B, "tracks/europe/jarama/route_1", 8, 3_420_000.),
    pool(B, "tracks/europe/nurburgring/route_1", 8, 4_900_000.),
    pool(B, "tracks/europe/nurburgring/route_3", 8, 4_900_000.),
    pool(B, "tracks/usa/long_beach/route_1", 8, 5_300_000.),
    pool(B, "tracks/usa/long_beach/route_4", 8, 5_300_000.),
    pool(B, "tracks/usa/washington/route_2", 8, 1_200_000.),
    pool(B, "tracks/usa/washington/route_3", 8, 3_900_000.),
    pool(B, "tracks/usa/washington/route_7", 8, 1_200_000.),
    pool(B, "tracks/usa/washington/route_8", 8, 3_900_000.),
    pool(B, "tracks/japan/okutama/route_2", 8, 5_300_000.),
    pool(B, "tracks/japan/okutama/route_5", 8, 5_300_000.),
    pool(B, "tracks/japan/yokohama_docks/route_0", 8, 4_000_000.),
    pool(B, "tracks/japan/yokohama_docks/route_1", 8, 810_000.),
    pool(B, "tracks/japan/yokohama_docks/route_2", 8, 2_100_000.),
    pool(B, "tracks/japan/yokohama_docks/route_7", 8, 4_000_000.),
    pool(B, "tracks/japan/yokohama_docks/route_8", 8, 810_000.),
    pool(B, "tracks/japan/yokohama_docks/route_9", 8, 2_100_000.),
    pool(B, "tracks/japan/shibuya/route_0", 8, 3_420_000.),
    pool(B, "tracks/japan/shibuya/route_1", 8, 3_420_000.),
    pool(B, "tracks/japan/shibuya/route_2", 8, 3_230_000.),
    pool(B, "tracks/japan/shibuya/route_3", 8, 3_230_000.),
    pool(G, "tracks/europe/istanbul_park/route_1", 2, 1_750_000.),
    pool(G, "tracks/europe/istanbul_park/route_3", 2, 1_750_000.),
    pool(G, "tracks/europe/jarama/route_0", 2, 1_900_000.),
    pool(G, "tracks/europe/jarama/route_1", 2, 1_900_000.),
    pool(G, "tracks/europe/nurburgring/route_1", 2, 1_250_000.),
    pool(G, "tracks/europe/nurburgring/route_3", 2, 1_250_000.),
    pool(G, "tracks/usa/long_beach/route_1", 2, 1_890_000.),
    pool(G, "tracks/usa/long_beach/route_4", 2, 1_890_000.),
    pool(G, "tracks/usa/washington/route_2", 2, 1_270_000.),
    pool(G, "tracks/usa/washington/route_3", 2, 1_800_000.),
    pool(G, "tracks/usa/washington/route_7", 2, 1_270_000.),
    pool(G, "tracks/usa/washington/route_8", 2, 1_800_000.),
    pool(G, "tracks/japan/okutama/route_2", 8, 1_890_000.),
    pool(G, "tracks/japan/okutama/route_5", 8, 1_890_000.),
    pool(G, "tracks/japan/yokohama_docks/route_0", 2, 1_750_000.),
    pool(G, "tracks/japan/yokohama_docks/route_1", 2, 964_000.),
    pool(G, "tracks/japan/yokohama_docks/route_2", 2, 1_000_000.),
    pool(G, "tracks/japan/yokohama_docks/route_7", 2, 1_750_000.),
    pool(G, "tracks/japan/yokohama_docks/route_8", 2, 964_000.),
    pool(G, "tracks/japan/yokohama_docks/route_9", 2, 1_000_000.),
    pool(G, "tracks/japan/shibuya/route_0", 2, 1_000_000.),
    pool(G, "tracks/japan/shibuya/route_1", 2, 1_000_000.),
    pool(G, "tracks/japan/shibuya/route_2", 2, 1_180_000.),
    pool(G, "tracks/japan/shibuya/route_3", 2, 1_180_000.),
    pool(G, "tracks/japan/haruna/route_0", 2, 1_700_000.),
    pool(F, "tracks/japan/yokohama_docks/route_3", 12, 1_148_000.),
    pool(F, "tracks/japan/yokohama_docks/route_4", 12, 1_148_000.),
    pool(F, "tracks/japan/yokohama_docks/route_5", 12, 1_148_000.),
    pool(F, "tracks/japan/yokohama_docks/route_6", 12, 1_148_000.),
];

/// The pool row of a format on a route folder (case-insensitive).
pub fn drift_pool(format: Format, route: &str) -> Option<&'static Pool> {
    DRIFT_POOLS
        .iter()
        .find(|p| p.format == format && p.route.eq_ignore_ascii_case(route))
}

/// 0x908450: seconds to the 64-bit microsecond count (truncated).
pub fn micros(seconds: f64) -> i64 {
    (seconds * 1_000_000.) as i64
}

/// 0x908820: the microsecond count in seconds (an unsigned conversion).
pub fn seconds(micros: i64) -> f64 {
    micros as u64 as f64 * 1e-6
}

/// The virtual race of a drift event (score branch).
#[derive(Clone, Debug, PartialEq)]
pub struct VirtualRace {
    /// Sum of the `base_result_list` values in microseconds (+0x20).
    pub base: i64,
    /// `base_time_factor`, `top_time_factor`, `performance_variance`
    /// (+0x10/+0x14/+0x18).
    pub base_time_factor: f32,
    pub top_time_factor: f32,
    pub performance_variance: f32,
    /// The base results were found (+0xAB9).
    pub loaded: bool,
    /// AUTHORED: the original draws from its global generator seeded with
    /// the clock (0x67D590 → 0xA68440); the port uses a seeded LCG.
    rng: u32,
}

impl VirtualRace {
    /// The set-up of 0x646320 for a free-race track pool row (`None`: no
    /// base results, every estimate is 0).
    pub fn new(pool: Option<&Pool>, difficulty: &Difficulty, seed: u32) -> Self {
        Self {
            // 0x680CE0: the list values in seconds, each truncated to
            // microseconds and summed.
            base: pool.map_or(0, |p| micros(f64::from(p.base_result))),
            base_time_factor: difficulty.base_time_factor,
            top_time_factor: difficulty.top_time_factor,
            performance_variance: difficulty.performance_variance,
            loaded: pool.is_some(),
            rng: seed,
        }
    }

    /// 0xA683B0: U(a, b) = a + u·(b − a), u = 32 random bits / 2^32.
    fn uniform(&mut self, a: f32, b: f32) -> f32 {
        self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let u = f64::from(self.rng) * f64::from(2f32.powi(-32));
        (u * (f64::from(b) - f64::from(a)) + f64::from(a)) as f32
    }

    /// 0x67D6C0: a random draw of 1 − skill within half the variance.
    fn draw(&mut self, skill: f32) -> f32 {
        let half = self.performance_variance * 0.5;
        let upper = (1. - (skill - half)).clamp(0., 1.);
        let lower = (1. - (half + skill)).clamp(0., 1.);
        self.uniform(lower, upper)
    }

    /// 0x6850F0 with 0x680B20 (`time_based` = 0): the result of a competitor
    /// of skill `performance` (clamped to 0..1 by 0x67FF20), microseconds.
    /// s = 0 (best) gives base / top_time_factor, s = 1 base /
    /// base_time_factor.
    pub fn result(&mut self, performance: f32) -> i64 {
        if !self.loaded {
            return 0;
        }
        let skill = performance.clamp(0., 1.);
        let s = self.draw(skill);
        let base = seconds(self.base);
        let top = base * (1. / f64::from(self.top_time_factor));
        let bottom = base * (1. / f64::from(self.base_time_factor));
        let range = (top - bottom).clamp(0., f64::MAX);
        micros(f64::from(1. - s) * range + bottom)
    }

    /// The race end (0x64D940): a car whose shown drift score is still zero
    /// gets the estimate times the event laps (both stored as float).
    pub fn drift_score(&mut self, performance: f32, laps: u32) -> f32 {
        (seconds(self.result(performance)) * f64::from(laps)) as f32
    }
}

/// One lap record of a competitor as the lap estimator reads and fills it
/// (the original's lap timing object: a 64-bit total and up to four sector
/// splits, each in microseconds, and a valid flag).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LapRecord {
    pub total: i64,
    pub splits: Vec<i64>,
    pub valid: bool,
}

impl LapRecord {
    /// A lap of `total` microseconds without sector splits.
    pub fn new(total: i64) -> Self {
        Self {
            total,
            splits: Vec::new(),
            valid: false,
        }
    }
    /// Split `i`, 0 for a split not recorded [I 0x908C60].
    fn split(&self, i: usize) -> i64 {
        self.splits.get(i).copied().unwrap_or(0)
    }
}

/// 0x908470: a microsecond count times a factor (as double), truncated.
fn scale(micros: i64, factor: f64) -> i64 {
    (micros as f64 * factor) as i64
}

/// 0xCB0948: the base lap of a track model without a `world_record` row.
pub const NO_WORLD_RECORD: f32 = -1.;

/// The virtual race of a circuit race as the race manager's lap estimator:
/// the race object's virtual race (+0x23F4, handed to the race manager at
/// +0xF2D8), whose vf 0x18 = 0x67FFB0 predicts the remaining laps of a car
/// still racing when the race is ended (0x90FCD0). The race manager adds
/// every competitor with skill 1.0 (0x916FE0).
#[derive(Clone)]
pub struct LapEstimator {
    /// Base lap (+0x20, µs): the track model's world record (0x67D890).
    pub base: i64,
    /// Sector bases (+0x28 + 8·i): the base lap × 0.25 each (0xCE8828).
    pub sectors: [i64; 4],
    /// Sector count (+0x48): 4.
    pub sector_count: u32,
    /// Draw period (+8): 1 from the constructor 0x67D590.
    pub period: u32,
    /// `performance_variance` of the difficulty row (+0x18).
    pub performance_variance: f32,
    /// The generator (MT19937, 0xA683B0). AUTHORED: seeded per race by the
    /// caller; the original seeds it from the clock × 1000.
    rng: Mt,
}

impl std::fmt::Debug for LapEstimator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LapEstimator")
            .field("base", &self.base)
            .field("performance_variance", &self.performance_variance)
            .finish_non_exhaustive()
    }
}

impl LapEstimator {
    /// The set-up of `virtual_race_new` (0x646320) for a circuit: the
    /// difficulty row's variance and 0x67D890's base lap from the track
    /// model's world record (s; [`NO_WORLD_RECORD`] without one).
    pub fn new(world_record: f32, difficulty: &Difficulty, seed: u32) -> Self {
        let base = micros(f64::from(world_record));
        let sector = scale(base, 0.25);
        Self {
            base,
            sectors: [sector; 4],
            sector_count: 4,
            period: 1,
            performance_variance: difficulty.performance_variance,
            rng: Mt::new(seed),
        }
    }

    /// 0x67D6C0 for a competitor of `skill`: U(clamp(1 − (skill + v/2)),
    /// clamp(1 − (skill − v/2))).
    fn draw(&mut self, skill: f32) -> f32 {
        let half = self.performance_variance * 0.5;
        let upper = (1. - (skill - half)).clamp(0., 1.);
        let lower = (1. - (half + skill)).clamp(0., 1.);
        self.rng.uniform(lower, upper)
    }

    /// A lap without a time to go by: base + base × U(0, variance).
    fn fresh_lap(&mut self) -> i64 {
        let u = self.rng.uniform(0., self.performance_variance);
        self.base.wrapping_add(scale(self.base, f64::from(u)))
    }

    /// 0x67FFB0: complete `lap` (the lap a car is on: its splits so far and
    /// its time so far in `total`) into a whole-lap estimate. `previous` is
    /// the car's previous lap record, `fraction` the part of `lap` covered,
    /// `unfinished_human` the finalisation flag (some human did not finish);
    /// `skill` is the competitor's (clamped to 0..1 by 0x67FF20).
    pub fn estimate(
        &mut self,
        lap: &mut LapRecord,
        skill: f32,
        previous: Option<&LapRecord>,
        fraction: f32,
        unfinished_human: bool,
    ) {
        let skill = skill.clamp(0., 1.);
        let n = if (1..=3).contains(&self.sector_count) {
            self.sector_count as usize
        } else {
            4
        };
        let k = lap.splits.len();
        // A draw whose value is not used.
        self.draw(skill);
        let covered = lap.splits.iter().fold(0i64, |a, &s| a.wrapping_add(s));
        // The whole lap: the time so far over the fraction covered (1e-4 at
        // 0xCBA1F0), else the previous lap, else a fresh lap.
        let mut total = if fraction > 1e-4 {
            scale(lap.total, f64::from(1. / fraction))
        } else if let Some(p) = previous {
            p.total
        } else {
            self.fresh_lap()
        };
        if unfinished_human {
            // Implausible against the base lap: under half of it, or over
            // twice it with under 0.1 (0xCAFCD4) of the lap covered.
            let (t, b) = (seconds(total), seconds(self.base));
            if !(b * 0.5 <= t && (t <= b + b || 0.1 <= fraction)) {
                total = match previous {
                    Some(p) => p.total,
                    None => self.fresh_lap(),
                };
            }
        }
        if k + 1 >= n && fraction > 0.5 && total > covered {
            lap.splits.push(total.wrapping_sub(covered));
        } else {
            let elapsed = lap.total.wrapping_sub(covered);
            for i in k..n {
                let mut split = 0;
                if let Some(p) = previous {
                    // At most twice the sector base, else redrawn below it.
                    let twice = scale(self.sectors[i], 2.);
                    split = p.split(i);
                    if split > twice {
                        let u = self.rng.uniform(0.5, 1.);
                        split = scale(twice, f64::from(u));
                    }
                }
                if split == 0 {
                    split = scale(total, f64::from(1. / n as f32));
                }
                if i % self.period.max(1) as usize == 0 {
                    // A draw whose value is not used.
                    self.draw(skill);
                }
                if i == k && elapsed > split {
                    split = elapsed;
                }
                lap.splits.push(split);
            }
        }
        // The last sector takes up the rest, at least 2 s (0xCB9B78).
        let sum = (0..n).fold(0i64, |a, i| a.wrapping_add(lap.split(i)));
        let last = seconds(total.wrapping_sub(sum).wrapping_add(lap.split(n - 1))).max(2.);
        if let Some(s) = lap.splits.get_mut(n - 1) {
            *s = micros(last);
        }
        lap.total = total;
        lap.valid = true;
    }
}

/// 0x67D890's base lap source: the `world_record` (s) of the first
/// `world_record` row of the route's track model (any vehicle grade, in
/// table order [I]), [`NO_WORLD_RECORD`] without one. `route` is
/// `tracks/{folder}/{file}/{route}`.
pub fn world_record(db: &grid_formats::database::Database, route: &str) -> f32 {
    let parts: Vec<&str> = route.split('/').collect();
    let [_, folder, file, layout] = parts[..] else {
        return NO_WORLD_RECORD;
    };
    let eq = |r: &grid_formats::database::Row<'_>, col: &str, v: &str| {
        r.text(col).is_some_and(|t| t.eq_ignore_ascii_case(v))
    };
    let model = db.table("track_model").and_then(|t| {
        t.rows()
            .find(|m| {
                eq(m, "folder_string", folder) && eq(m, "file_string", file) && eq(m, "route_string", layout)
            })
            .and_then(|m| m.int("id"))
    });
    model
        .and_then(|id| {
            db.table("world_record")?
                .rows()
                .find(|r| r.int("track_model_id") == Some(id))
                .and_then(|r| r.float("world_record"))
        })
        .unwrap_or(NO_WORLD_RECORD)
}

/// [`world_record`] from the game's `database/schema.bin` and
/// `database/database.bin` under `root`.
pub fn load_world_record(root: &std::path::Path, route: &str) -> Result<f32, Box<dyn std::error::Error>> {
    let schema = std::fs::read(root.join("database/schema.bin"))?;
    let data = std::fs::read(root.join("database/database.bin"))?;
    Ok(world_record(&grid_formats::database::read(&schema, &data)?, route))
}

/// The circuit race's lap estimator for an event on `route` at the AI
/// difficulty `ai_ratio`: the career rank 2 `difficulty` row of its star
/// level (as the drift virtual race, docs/DISCIPLINES.md) and the
/// AUTHORED seed 0x5EED in place of the original's clock.
pub fn circuit_estimator(
    root: &std::path::Path,
    route: &str,
    ai_ratio: f32,
) -> Result<LapEstimator, Box<dyn std::error::Error>> {
    let row = difficulty(2, crate::flashback::star_level(ai_ratio));
    Ok(LapEstimator::new(load_world_record(root, route)?, row, 0x5eed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_are_indexed_by_rank_and_star_level() {
        let normal = difficulty(2, 1);
        assert_eq!(normal.ai_ratio, 0.606);
        assert_eq!(
            (normal.base_time_factor, normal.top_time_factor),
            (43., 14.)
        );
        assert_eq!(difficulty(4, 9).star_level, 5);
        for (i, d) in DIFFICULTY.iter().enumerate() {
            assert_eq!(
                (usize::from(d.career_rank), usize::from(d.star_level)),
                (i / 6 + 1, i % 6)
            );
            // The score range is ordered: the top factor is the smaller.
            assert!(d.top_time_factor < d.base_time_factor);
        }
    }

    #[test]
    fn pools_are_unique_and_found_by_route() {
        let rows: Vec<&Pool> = DRIFT_POOLS.iter().collect();
        for (i, a) in rows.iter().enumerate() {
            for b in &rows[i + 1..] {
                assert!(!(a.format == b.format && a.route == b.route), "{a:?}");
            }
        }
        let p = drift_pool(Format::DriftBattle, "tracks/usa/long_beach/Route_1").unwrap();
        assert_eq!((p.max_cars, p.base_result), (8, 5_300_000.));
        let g = drift_pool(Format::DriftGrandPrix, "tracks/usa/long_beach/route_1").unwrap();
        assert_eq!((g.max_cars, g.base_result), (2, 1_890_000.));
        assert!(drift_pool(Format::DriftBattle, "tracks/europe/donington/route_0").is_none());
        assert_eq!(
            Format::from_name("drift_grand_prix"),
            Some(Format::DriftGrandPrix)
        );
    }

    #[test]
    fn microseconds_round_trip() {
        assert_eq!(micros(5_300_000.), 5_300_000_000_000);
        assert_eq!(seconds(5_300_000_000_000), 5_300_000.);
        assert_eq!(micros(1.234_567_9), 1_234_567);
    }

    #[test]
    fn estimates_lie_between_the_base_and_top_factors() {
        let pool = drift_pool(Format::DriftBattle, "tracks/usa/long_beach/route_1");
        let d = difficulty(2, 1);
        let mut race = VirtualRace::new(pool, d, 7);
        let (bottom, top) = (5_300_000. / 43., 5_300_000. / 14.);
        for _ in 0..200 {
            let best = seconds(race.result(1.));
            // Skill 1: s within 0..variance/2, the top end.
            assert!(
                best <= top + 1e-6 && best >= top - 0.065 * (top - bottom) - 1e-3,
                "{best}"
            );
            let worst = seconds(race.result(0.));
            assert!(worst >= bottom - 1e-6 && worst <= bottom + 0.065 * (top - bottom) + 1e-3);
            let mid = seconds(race.result(0.7));
            let s = 1. - (mid - bottom) / (top - bottom);
            assert!(
                (0.3 - 0.065 - 1e-4..=0.3 + 0.065 + 1e-4).contains(&s),
                "{s}"
            );
        }
        // Clamped skill.
        let mut a = VirtualRace::new(pool, d, 9);
        let mut b = VirtualRace::new(pool, d, 9);
        assert_eq!(a.result(3.), b.result(1.));
        // Times the event laps, as float.
        let score = VirtualRace::new(pool, d, 1).drift_score(1., 2);
        let one = seconds(VirtualRace::new(pool, d, 1).result(1.));
        assert_eq!(score, (one * 2.) as f32);
    }

    #[test]
    fn the_lap_estimator_fills_four_sectors_and_the_whole_lap() {
        let mut e = LapEstimator::new(80., difficulty(2, 1), 3);
        assert_eq!((e.base, e.sectors[3]), (80_000_000, 20_000_000));
        // Half a lap in 40 s: an 80 s lap; the first sector takes the time
        // so far, the last the rest (at least 2 s).
        let mut lap = LapRecord::new(micros(40.));
        e.estimate(&mut lap, 1., None, 0.5, false);
        assert!(lap.valid);
        assert_eq!(lap.total, 80_000_000);
        assert_eq!(lap.splits[..3], [40_000_000, 20_000_000, 20_000_000]);
        assert_eq!(lap.splits[3], 2_000_000);
        // Past half a lap with three sectors recorded: one more split.
        let mut lap = LapRecord {
            total: micros(60.),
            splits: vec![micros(20.); 3],
            valid: false,
        };
        e.estimate(&mut lap, 1., None, 0.75, false);
        // 1 / 0.75 in single precision: 80 000 002 us.
        assert_eq!((lap.total, lap.splits.len()), (80_000_002, 4));
        assert_eq!(lap.splits[3], 20_000_002);
        // No time to go by: the previous lap, with its long sectors redrawn
        // between one and two sector bases.
        let previous = LapRecord {
            total: micros(90.),
            splits: vec![micros(50.), micros(10.), micros(10.), micros(20.)],
            valid: true,
        };
        let mut lap = LapRecord::default();
        e.estimate(&mut lap, 1., Some(&previous), 0., false);
        assert_eq!(lap.total, 90_000_000);
        assert!((20_000_000..=40_000_000).contains(&lap.splits[0]), "{:?}", lap.splits);
        assert_eq!(lap.splits[1..3], [10_000_000, 10_000_000]);
        // Neither: base + base x U(0, variance).
        for _ in 0..50 {
            let mut lap = LapRecord::default();
            e.estimate(&mut lap, 1., None, 0., false);
            assert!((80_000_000..=90_400_000).contains(&lap.total), "{}", lap.total);
        }
    }

    #[test]
    fn the_unfinished_human_flag_rejects_implausible_laps() {
        let previous = LapRecord::new(micros(85.));
        let mut e = LapEstimator::new(80., difficulty(2, 1), 3);
        // Under half the base lap.
        let mut lap = LapRecord::new(micros(10.));
        e.estimate(&mut lap, 1., Some(&previous), 0.5, true);
        assert_eq!(lap.total, previous.total);
        // Over twice the base under 0.1 of the lap; at 0.1 it stands.
        let mut lap = LapRecord::new(micros(17.));
        e.estimate(&mut lap, 1., Some(&previous), 0.09, true);
        assert_eq!(lap.total, previous.total);
        let mut lap = LapRecord::new(micros(17.));
        e.estimate(&mut lap, 1., Some(&previous), 0.1, true);
        assert!(lap.total > micros(169.));
        // Without the flag the projection stands.
        let mut lap = LapRecord::new(micros(10.));
        e.estimate(&mut lap, 1., Some(&previous), 0.5, false);
        assert_eq!(lap.total, 20_000_000);
    }

    #[test]
    fn without_base_results_the_estimate_is_zero() {
        let mut race = VirtualRace::new(None, difficulty(2, 1), 1);
        assert!(!race.loaded);
        assert_eq!(race.drift_score(1., 2), 0.);
    }
}
