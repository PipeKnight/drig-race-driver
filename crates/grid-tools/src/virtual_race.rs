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
    fn without_base_results_the_estimate_is_zero() {
        let mut race = VirtualRace::new(None, difficulty(2, 1), 1);
        assert!(!race.loaded);
        assert_eq!(race.drift_score(1., 2), 0.);
    }
}
