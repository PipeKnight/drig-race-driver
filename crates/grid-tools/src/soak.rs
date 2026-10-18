//! Headless soak/stress harness shared by `grid-soak` ([docs](../../../docs/SOAK_TESTING.md)).
//!
//! [`Session`] mirrors the windowed prototype's `--drive` tick (grid-app
//! `drive_tick`/`drive_actions`): the player runtime is built and configured
//! exactly as the app does, is swapped into slot 0 of the shared world for each
//! common tick, opponents are driven by the ported AI (grid revving during the
//! countdown) and `R` resets every car, the AI and the countdown. Keep the two
//! in step when the app's tick changes. [`Chaos`] is an AUTHORED adversarial
//! player driven only by a seeded [`Rng`] and the session state, so a seed
//! reproduces a run; [`Step`] records are the per-tick inputs a replay feeds
//! back without the chaos driver.
use crate::{
    ai_driver::{AiData, AiDriver, NORMAL_DIFFICULTY, Other, Parameters, View},
    car::Car,
    route::Route,
    source_vehicle::{
        Controls, DELTA, Runtime,
        assists::{Assists, Input, Shift, Transmission},
        multi::{DeformationParameters, MultiRuntime},
        player_reset::PlayerReset,
    },
    time_trial::{Phase, State},
};
use std::{collections::BTreeMap, path::Path};

pub use crate::route::{
    DETROIT_STADIUM, DETROIT_STADIUM_SHORT, DONINGTON, HARUNA, ISTANBUL_PARK, JARAMA, LONG_BEACH,
    MILAN, NURBURGRING, SAN_FRANCISCO, SHIBUYA, SPA, YOKOHAMA,
};
/// Donington's race start-light sequence; every race holds the grid for its
/// track's shipped sequence time (`start_lights::load`, grid-app StartLights).
pub const RACE_COUNTDOWN: f64 = 4.0;
/// The app's solo time-trial countdown (grid-app time_trial::Trial::new).
pub const TRIAL_COUNTDOWN: f64 = 3.0;
/// The shared world accepts 1..=12 cars (MultiRuntime, app `--opponents 1..11`).
pub const MAX_SHARED_CARS: usize = 12;

/// SplitMix64: small, seedable and identical on every platform.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
    /// Uniform in [0, 1).
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.unit()
    }
    /// Uniform in 0..n (n > 0).
    pub fn below(&mut self, n: u64) -> u64 {
        self.next_u64() % n.max(1)
    }
    pub fn chance(&mut self, p: f64) -> bool {
        ((self.next_u64() >> 11) as f64 / (1u64 << 53) as f64) < p
    }
    pub fn sign(&mut self) -> f32 {
        if self.chance(0.5) { 1. } else { -1. }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    Quick,
    Long,
}

impl Profile {
    pub fn name(self) -> &'static str {
        match self {
            Profile::Quick => "quick",
            Profile::Long => "long",
        }
    }
    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "quick" => Ok(Profile::Quick),
            "long" => Ok(Profile::Long),
            _ => Err(format!("unknown profile `{s}` (quick or long)")),
        }
    }
}

/// Which private inputs exist on this machine (they gate scenario features).
#[derive(Clone, Copy, Debug, Default)]
pub struct Available {
    pub events: bool,
    pub deformation: bool,
    pub characteristics: bool,
}

/// One seeded run. Every field is written to the inputs file so a replay
/// rebuilds the same setup without the generator.
#[derive(Clone, Debug, PartialEq)]
pub struct Scenario {
    pub seed: u64,
    pub profile: Profile,
    pub route: String,
    /// Cars in the world including the player (car 0).
    pub cars: usize,
    /// Player grid position, 1-based (app `--grid-position`).
    pub grid_position: usize,
    pub assists: Assists,
    /// Body contacts (CD01) with breakable props, as the app's `--body-contacts`.
    pub body_contacts: bool,
    /// Breakable route props with body contacts (the app always enables them).
    pub props: bool,
    pub body_events: bool,
    pub deformation: bool,
    pub characteristics: bool,
    /// Event laps; `Some` runs the app's countdown hold first.
    pub laps: Option<u32>,
    /// Simulation ticks to run (resets do not count).
    pub ticks: u64,
    /// Expected chaos resets per 10 000 ticks.
    pub reset_rate: f64,
    /// Force one chaos mode for the whole run (`None`: weighted mix).
    pub chaos: Option<Mode>,
    /// The player's car model (app `--car`).
    pub car: Car,
    /// Opponent models cycled over the opponents (app `--opponent-cars`);
    /// empty: the player's model.
    pub opponent_cars: Vec<Car>,
}

/// Car counts the quick profile cycles through: the owner's 12-car race
/// first, then solo and mid-size fields.
const QUICK_CARS: [usize; 8] = [12, 1, 12, 6, 2, 12, 9, 4];
/// Quick seeds 1..=12 keep their F326 routes; 13..=16 run Istanbul Park and
/// 17..=20 Long Beach (in the default set since its barrier stand-ins were
/// fixed, docs/LONG_BEACH.md section 5) and 21..=24 Jarama (in the default set
/// since its two piece losses count as expected losses,
/// docs/ROBUSTNESS_RESEARCH.md). Seeds 25..=28 run Yokohama, outside the
/// default set for now (docs/YOKOHAMA.md); `--seeds 25..28`. Seeds 29..=32
/// run Haruna's point-to-point road with one or two cars, also outside it
/// (docs/TOUGE.md); `--seeds 29..32`. Seeds 33..=36 run 12, 12, 6 and 12
/// stock cars on the Detroit stadium routes 8 and 9 (heavy contact, docs/
/// DEMOLITION_DERBY.md), also outside the default set; `--seeds 33..36`.
/// Seeds 37..=40 run Spa (docs/SPA.md), 41..=44 Milan, 45..=48 Shibuya and
/// 49..=52 San Francisco (docs/STREET_CIRCUITS.md), also outside it;
/// `--seeds 37..52`.
pub const QUICK_SEEDS: (u64, u64) = (1, 24);

impl Scenario {
    /// Pure function of (seed, profile, availability, max cars).
    pub fn generate(seed: u64, profile: Profile, available: Available, max_cars: usize) -> Self {
        let mut rng = Rng::new(seed ^ 0x5eed_50a4_0000_0000);
        let max_cars = max_cars.max(1);
        let cars = match profile {
            // Derby seeds 33..=36: 12, 12, 6 and 12 stock cars.
            Profile::Quick if (33..=36).contains(&seed) => {
                [12, 12, 6, 12][((seed - 33) % 4) as usize].min(max_cars)
            }
            // Haruna's touge grids hold two cars.
            Profile::Quick if (29..=32).contains(&seed) => {
                [2, 1][(seed % 2) as usize].min(max_cars)
            }
            Profile::Quick => QUICK_CARS[(seed.wrapping_sub(1) % QUICK_CARS.len() as u64) as usize]
                .min(max_cars),
            // Full fields are weighted up: they are the owner's race.
            Profile::Long if rng.chance(0.35) => max_cars.min(MAX_SHARED_CARS),
            Profile::Long => 1 + rng.below(max_cars as u64) as usize,
        };
        let route = match profile {
            // Quick seeds 49..=52 on San Francisco.
            Profile::Quick if seed > 48 => SAN_FRANCISCO,
            // Quick seeds 45..=48 on Shibuya.
            Profile::Quick if seed > 44 => SHIBUYA,
            // Quick seeds 41..=44 on Milan.
            Profile::Quick if seed > 40 => MILAN,
            // Quick seeds 37..=40 on Spa.
            Profile::Quick if seed > 36 => SPA,
            // Quick seeds 33..=36 on the Detroit stadium, long and short in turn.
            Profile::Quick if seed > 32 && seed % 2 == 1 => DETROIT_STADIUM,
            Profile::Quick if seed > 32 => DETROIT_STADIUM_SHORT,
            // Quick seeds 29..=32 (1, 2, 1 and 2 cars) on Haruna's downhill run.
            Profile::Quick if seed > 28 => HARUNA,
            // Quick seeds 25..=28 (12, 1, 12 and 6 cars) on Yokohama.
            Profile::Quick if seed > 24 => YOKOHAMA,
            // Quick seeds 21..=24 (2, 12, 9 and 4 cars) on Jarama.
            Profile::Quick if seed > 20 => JARAMA,
            // Quick seeds 17..=20 (12, 1, 12 and 6 cars) on Long Beach.
            Profile::Quick if seed > 16 => LONG_BEACH,
            // Quick seeds 13..=16 (2, 12, 9 and 4 cars) on Istanbul Park.
            Profile::Quick if seed > 12 => ISTANBUL_PARK,
            // Odd quick seeds on Donington; the 12-car seeds 1 and 3 cover both.
            Profile::Quick if seed % 4 == 3 || seed % 4 == 2 => NURBURGRING,
            Profile::Quick => DONINGTON,
            Profile::Long if rng.chance(0.5) => NURBURGRING,
            Profile::Long => DONINGTON,
        };
        // A third of the long runs move to Istanbul Park, drawn from their own
        // stream so every other field of a seed stays as before.
        let route = if profile == Profile::Long
            && Rng::new(seed ^ 0x157a_0000_0000_0000).chance(1. / 3.)
        {
            ISTANBUL_PARK
        } else {
            route
        };
        // A quarter of the long runs then move to Long Beach (own stream too).
        let route = if profile == Profile::Long
            && Rng::new(seed ^ 0x10b3_0000_0000_0000).chance(0.25)
        {
            LONG_BEACH
        } else {
            route
        };
        // A fifth of the long runs then move to Jarama (own stream too).
        let route = if profile == Profile::Long
            && Rng::new(seed ^ 0x7a2a_0000_0000_0000).chance(0.2)
        {
            JARAMA
        } else {
            route
        };
        // A sixth of the long runs then move to Yokohama (own stream too).
        let route = if profile == Profile::Long
            && Rng::new(seed ^ 0x70c0_0000_0000_0000).chance(1. / 6.)
        {
            YOKOHAMA
        } else {
            route
        };
        // A seventh of the long runs then move to Spa (own stream too).
        let route = if profile == Profile::Long
            && Rng::new(seed ^ 0x5ba0_0000_0000_0000).chance(1. / 7.)
        {
            SPA
        } else {
            route
        };
        // An eighth, a ninth and a tenth of the long runs then move to Milan,
        // Shibuya and San Francisco (own streams too).
        let route = if profile == Profile::Long
            && Rng::new(seed ^ 0x3a10_0000_0000_0000).chance(1. / 8.)
        {
            MILAN
        } else {
            route
        };
        let route = if profile == Profile::Long
            && Rng::new(seed ^ 0x5b1b_0000_0000_0000).chance(1. / 9.)
        {
            SHIBUYA
        } else {
            route
        };
        let route = if profile == Profile::Long
            && Rng::new(seed ^ 0x5f40_0000_0000_0000).chance(0.1)
        {
            SAN_FRANCISCO
        } else {
            route
        }
        .to_string();
        let mut assists = Assists {
            traction_control: rng.chance(0.5).into(),
            stability_control: rng.chance(0.5),
            braking_assist: rng.chance(0.5).into(),
            transmission: if rng.chance(0.3) {
                Transmission::Manual
            } else {
                Transmission::Automatic
            },
            input: if rng.chance(0.2) {
                Input::Wheel
            } else {
                Input::Keyboard
            },
            ..Assists::default()
        };
        if assists.input == Input::Wheel {
            // Option `wheelLinearity` L in 0..1 gives (L - 0.5) * 2.
            assists.wheel_response = ((rng.below(11) as f32 / 10.) - 0.5) * 2.;
        }
        let grid_position = 1 + rng.below(cars as u64) as usize;
        let laps = if cars > 1 || rng.chance(0.5) {
            Some(3)
        } else {
            None
        };
        let ticks = match profile {
            Profile::Quick => 5000,
            Profile::Long => 6000 + rng.below(24_001),
        };
        Self {
            seed,
            profile,
            route,
            cars,
            grid_position,
            assists,
            body_contacts: true,
            props: true,
            body_events: available.events,
            deformation: available.deformation,
            characteristics: available.characteristics && cars > 1,
            laps,
            ticks,
            reset_rate: match profile {
                Profile::Quick => 2.,
                Profile::Long => 1.5,
            },
            chaos: None,
            car: Car::default(),
            opponent_cars: Vec::new(),
        }
        .with_long_models(seed, profile)
        .with_derby_models(seed, profile)
    }

    /// Derby quick seeds (33..=36) run the stock car (`stk`) for every car:
    /// heavy contact in the stadium's crossing (docs/DEMOLITION_DERBY.md).
    fn with_derby_models(mut self, seed: u64, profile: Profile) -> Self {
        if profile == Profile::Quick && (33..=36).contains(&seed) {
            // An empty opponent list gives every car the player's model.
            self.car = Car::new("stk").unwrap();
            self.opponent_cars.clear();
        }
        self
    }

    /// Long runs also drive the BMW 320si: half the races are a mixed
    /// Touring_cars field, some solo runs use the BMW. Drawn from their own
    /// stream, so every other field of a seed stays as before; quick runs
    /// stay all-Lacetti (pass `--car`/`--opponent-cars` to mix them).
    fn with_long_models(mut self, seed: u64, profile: Profile) -> Self {
        if profile != Profile::Long {
            return self;
        }
        let mut rng = Rng::new(seed ^ 0xca75_0000_0000_0000);
        let bmw = || Car::new(crate::car::BMW).unwrap();
        if self.cars > 1 && rng.chance(0.5) {
            if rng.chance(0.5) {
                self.car = bmw();
            }
            self.opponent_cars = if rng.chance(0.5) {
                vec![bmw(), Car::default()]
            } else {
                vec![Car::default(), bmw()]
            };
        } else if rng.chance(0.25) {
            self.car = bmw();
        }
        self
    }

    /// Model of every car, the player's first (app `--opponent-cars` order).
    pub fn field_models(&self) -> Vec<Car> {
        std::iter::once(self.car.clone())
            .chain((0..self.cars.saturating_sub(1)).map(|i| {
                if self.opponent_cars.is_empty() {
                    self.car.clone()
                } else {
                    self.opponent_cars[i % self.opponent_cars.len()].clone()
                }
            }))
            .collect()
    }

    /// Countdown seconds the app runs before GO (0 without an event); a race
    /// holds the grid for the track's start-light sequence `race`.
    pub fn countdown(&self, race: f64) -> f64 {
        match self.laps {
            Some(_) if self.cars > 1 => race,
            Some(_) => TRIAL_COUNTDOWN,
            None => 0.,
        }
    }

    pub fn header(&self) -> String {
        let flag = |b: bool| u8::from(b);
        format!(
            "seed={} profile={} route={} cars={} grid_position={} assists={} body_contacts={} props={} body_events={} deformation={} characteristics={} laps={} ticks={} reset_rate={} chaos={}",
            self.seed,
            self.profile.name(),
            self.route,
            self.cars,
            self.grid_position,
            self.assists,
            flag(self.body_contacts),
            flag(self.props),
            flag(self.body_events),
            flag(self.deformation),
            flag(self.characteristics),
            self.laps.map_or_else(|| "none".into(), |l| l.to_string()),
            self.ticks,
            self.reset_rate,
            self.chaos.map_or("mixed", Mode::name)
        ) + &self.models_field()
    }

    /// ` car=.. opponent_cars=..` for a non-Lacetti field (as the app's
    /// `drive_replay`), empty otherwise so default headers keep their form.
    fn models_field(&self) -> String {
        if self.car.is_default() && self.opponent_cars.iter().all(Car::is_default) {
            return String::new();
        }
        let list = if self.opponent_cars.is_empty() {
            self.car.to_string()
        } else {
            self.opponent_cars
                .iter()
                .map(Car::stem)
                .collect::<Vec<_>>()
                .join(",")
        };
        format!(" car={} opponent_cars={list}", self.car)
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let fields: BTreeMap<&str, &str> = text
            .split_whitespace()
            .filter_map(|item| item.split_once('='))
            .collect();
        let get = |k: &str| fields.get(k).copied().ok_or(format!("scenario lacks {k}"));
        let num = |k: &str| -> Result<u64, String> {
            get(k)?.parse().map_err(|e| format!("scenario {k}: {e}"))
        };
        let flag = |k: &str| -> Result<bool, String> { Ok(get(k)? == "1") };
        // Inputs files written before mixed fields are all-Lacetti.
        let car = match fields.get("car") {
            Some(stem) => Car::new(stem).map_err(|e| format!("scenario car: {e}"))?,
            None => Car::default(),
        };
        Ok(Self {
            seed: num("seed")?,
            profile: Profile::parse(get("profile")?)?,
            route: get("route")?.to_string(),
            cars: num("cars")? as usize,
            grid_position: num("grid_position")? as usize,
            assists: Assists::parse(get("assists")?)?,
            body_contacts: flag("body_contacts")?,
            // Inputs files written before the switch always had props.
            props: fields.get("props").is_none_or(|v| *v == "1"),
            body_events: flag("body_events")?,
            deformation: flag("deformation")?,
            characteristics: flag("characteristics")?,
            laps: match get("laps")? {
                "none" => None,
                l => Some(l.parse().map_err(|e| format!("scenario laps: {e}"))?),
            },
            ticks: num("ticks")?,
            reset_rate: get("reset_rate")?
                .parse()
                .map_err(|e| format!("scenario reset_rate: {e}"))?,
            chaos: match fields.get("chaos").copied() {
                None | Some("mixed") => None,
                Some(name) => Some(Mode::parse(name)?),
            },
            car: car.clone(),
            // `opponent_cars=<car>` is the header form of "the player's model".
            opponent_cars: match fields.get("opponent_cars") {
                Some(list) => {
                    let list =
                        Car::list(list).map_err(|e| format!("scenario opponent_cars: {e}"))?;
                    if list == [car] { Vec::new() } else { list }
                }
                None => Vec::new(),
            },
        })
    }
}

/// Private machine-local inputs (never committed).
#[derive(Clone, Default)]
pub struct Private {
    pub table: Option<Vec<u8>>,
    pub events: Option<grid_world::body_impact::Parameters>,
    pub deformation: Option<DeformationParameters>,
    pub characteristics: Vec<BTreeMap<String, f32>>,
}

impl Private {
    pub fn available(&self) -> Available {
        Available {
            events: self.events.is_some(),
            deformation: self.deformation.is_some(),
            characteristics: !self.characteristics.is_empty(),
        }
    }
}

/// One recorded input step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step {
    /// A simulation tick with the player's controls and an optional gear request.
    Tick(Controls, Option<Shift>),
    /// `R`: reset every car, the AI and the countdown before the next tick.
    Reset,
}

pub fn shift_name(shift: Option<Shift>) -> String {
    match shift {
        None => "-".into(),
        Some(Shift::Up) => "up".into(),
        Some(Shift::Down) => "down".into(),
        Some(Shift::Gear(g)) => format!("g{g}"),
    }
}

pub fn parse_shift(s: &str) -> Result<Option<Shift>, String> {
    Ok(match s {
        "-" | "none" => None,
        "up" => Some(Shift::Up),
        "down" => Some(Shift::Down),
        g => Some(Shift::Gear(
            g.strip_prefix('g')
                .and_then(|g| g.parse().ok())
                .ok_or(format!("bad shift `{s}`"))?,
        )),
    })
}

impl Step {
    /// One TSV line; floats use Rust's round-trip formatting.
    pub fn line(&self, note: &str) -> String {
        match self {
            Step::Tick(c, shift) => format!(
                "T\t{}\t{}\t{}\t{}\t{}\t{note}",
                c.throttle,
                c.brake,
                c.steering,
                u8::from(c.handbrake),
                shift_name(*shift)
            ),
            Step::Reset => format!("R\t{note}"),
        }
    }

    pub fn parse(line: &str) -> Result<Self, String> {
        let f: Vec<&str> = line.split('\t').collect();
        match f.first() {
            Some(&"R") => Ok(Step::Reset),
            Some(&"T") if f.len() >= 6 => {
                let num = |i: usize| -> Result<f32, String> {
                    f[i].parse().map_err(|e| format!("input `{line}`: {e}"))
                };
                Ok(Step::Tick(
                    Controls {
                        throttle: num(1)?,
                        brake: num(2)?,
                        steering: num(3)?,
                        handbrake: f[4] == "1",
                    },
                    parse_shift(f[5])?,
                ))
            }
            _ => Err(format!("bad input line `{line}`")),
        }
    }
}

/// The app's opponents: the shared world and its AI drivers.
pub struct Race {
    pub multi: MultiRuntime,
    pub drivers: Vec<AiDriver>,
    /// AI data of each opponent's model (driver `i` drives car `i + 1`).
    pub car_ai: Vec<AiData>,
    /// The app's race manager with its default `--ai` settings.
    pub pacing: crate::ai_pacing::Field,
}

impl Race {
    /// Grid/current positions; car 0 is the player runtime held outside `multi`.
    fn positions(&self, player: &Runtime) -> Vec<[f32; 3]> {
        std::iter::once(player)
            .chain(self.multi.cars.iter().skip(1))
            .map(|c| {
                let p = c.pose();
                [p[12], p[13], p[14]]
            })
            .collect()
    }
}

/// Failure classes the harness reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// The tick returned an error: the app shows "driving paused".
    WorldError,
    Panic,
    NonFinite,
    OutOfBounds,
    Speed,
    /// Set-up refused the scenario.
    Setup,
    /// The scenario process aborted or hung (orchestrator only).
    Crash,
    Hang,
    /// A car's own tick stage failed and the shared world retired it
    /// (the race goes on without it, MultiRuntime S3).
    Retired,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::WorldError => "world-error",
            Kind::Panic => "panic",
            Kind::NonFinite => "non-finite",
            Kind::OutOfBounds => "out-of-bounds",
            Kind::Speed => "implausible-speed",
            Kind::Setup => "setup",
            Kind::Crash => "crash",
            Kind::Hang => "hang",
            Kind::Retired => "car-retired",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        [
            Kind::WorldError,
            Kind::Panic,
            Kind::NonFinite,
            Kind::OutOfBounds,
            Kind::Speed,
            Kind::Setup,
            Kind::Crash,
            Kind::Hang,
            Kind::Retired,
        ]
        .into_iter()
        .find(|k| k.name() == s)
    }
}

/// Grouping key: the failure text without a trailing ` (detail)` and with
/// every number replaced by `#`.
pub fn group_key(kind: Kind, text: &str) -> String {
    let text = match text.rfind(" (") {
        Some(i) if text.ends_with(')') => &text[..i],
        _ => text,
    };
    let mut out = String::with_capacity(text.len() + 16);
    out.push_str(kind.name());
    out.push_str(": ");
    let mut digits = false;
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            if !digits {
                out.push('#');
            }
            digits = true;
        } else {
            // Keep "1.5" as one number.
            if !(digits && ch == '.') {
                digits = false;
            }
            if ch != '.' || !digits {
                out.push(ch);
            }
        }
    }
    out
}

/// Speeds no Lacetti or prop piece reaches without a solver explosion (AUTHORED).
pub const MAX_CAR_SPEED: f32 = 120.;
pub const MAX_PIECE_SPEED: f32 = 300.;
/// Downward speed that only a body falling through the ground reaches (AUTHORED).
pub const MAX_FALL_SPEED: f32 = 25.;
/// Margin around the road chunks before a body counts as outside the world.
pub const BOUNDS_MARGIN: [f32; 3] = [150., 60., 150.];

/// Physical body words: pose 0x1D0 (16), angular velocity 0x210, velocity 0x220.
fn body_floats(body: &grid_world::contact_groups::Body, offset: usize, n: usize) -> Vec<f32> {
    (0..n)
        .map(|k| f32::from_bits(body[offset / 4 + k]))
        .collect()
}

/// The app's player runtime plus, with opponents, the shared world; see the module docs.
pub struct Session {
    pub scenario: Scenario,
    pub runtime: Runtime,
    pub geometry: Option<crate::body_geometry::BodyGeometry>,
    pub race: Option<Race>,
    pub ai: AiData,
    pub countdown: Option<State>,
    /// The track's start-light sequence seconds (race countdown).
    pub start_sequence: f64,
    /// Route centreline (distance, position) for the chaos driver.
    pub centreline: Vec<(f32, [f32; 3])>,
    /// Allowed region (min xyz, max xyz).
    pub bounds: [f32; 6],
    /// Ticks run in the current segment / in the whole session.
    pub total_ticks: u64,
    pub resets: u64,
    /// The player's auto-reset (grid-app `drive_tick`, F341).
    pub auto_reset: PlayerReset,
    /// Auto-resets performed in the whole session.
    pub auto_resets: u64,
    /// Expected losses: world bodies that fell below the world after the
    /// original's own static geometry pressed them down at their spawn pose
    /// (`PropWorld::pressed_down_at_spawn`, docs/ROBUSTNESS_RESEARCH.md).
    /// They are reported, not failed. Body indices are reused after a reset,
    /// so each entry carries its segment (`resets` at the time).
    pub expected_losses: std::cell::RefCell<Vec<ExpectedLoss>>,
}

/// A body counted as an expected loss: segment, session tick, body and position.
#[derive(Clone, Debug)]
pub struct ExpectedLoss {
    pub segment: u64,
    pub tick: u64,
    pub body: usize,
    pub position: [f32; 3],
    pub pressed: crate::dynamic_props::Pressed,
}

/// Snapshot used by the chaos driver and the failure checks.
pub struct Snapshot<'a> {
    pub bodies: &'a [grid_world::contact_groups::Body],
    pub cars: usize,
}

impl Session {
    /// Build the app's driving state for `scenario` (grid-app `run`, `--drive`).
    pub fn new(
        root: &Path,
        scenario: &Scenario,
        private: &Private,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if !(1..=99).contains(&scenario.cars) || scenario.grid_position == 0 {
            return Err("scenario needs at least one car and a 1-based grid position".into());
        }
        if scenario.grid_position > scenario.cars {
            return Err("grid position beyond the field".into());
        }
        let route = Route::new(&scenario.route)?;
        let start_sequence = f64::from(crate::start_lights::load(root, &route)?.seconds);
        let opponents = scenario.cars - 1;
        if opponents > 0 && !scenario.body_contacts {
            return Err("opponents require body contacts (app --opponents)".into());
        }
        let player_slot = scenario.grid_position - 1;
        let models = scenario.field_models();
        let mut runtime = Runtime::load(
            root,
            &route,
            &models[0],
            &format!("slot_{player_slot:02}"),
            200679,
        )?;
        runtime.set_assists(scenario.assists)?;
        runtime.enable_lap_timing(root)?;
        if scenario.body_events {
            runtime.body_events = Some(private.events.ok_or("missing private BP01")?);
        }
        let deformation = if scenario.deformation {
            Some(
                private
                    .deformation
                    .clone()
                    .ok_or("missing private DF01")?,
            )
        } else {
            None
        };
        let geometry = if scenario.body_contacts {
            let table = private.table.as_ref().ok_or("missing private CD01")?;
            let mut geometry = runtime.prepare_body_geometry(root, table)?;
            if let Some((limit, bulk)) = &deformation {
                runtime.enable_deformation(root, &mut geometry)?;
                runtime.register_deformation_callbacks(*limit, bulk.clone(), 1)?;
            }
            if scenario.props {
                runtime.enable_dynamic_props(root)?;
            }
            Some(geometry)
        } else {
            None
        };
        // Every model's own AI classes and statistics (database vehicle row).
        let mut by_model = BTreeMap::new();
        for model in &models {
            if !by_model.contains_key(model) {
                let data = AiData::load_route(root, &route, model, None)?;
                by_model.insert(model.clone(), data);
            }
        }
        let ai = by_model[&models[0]].clone();
        let race = if opponents > 0 {
            let table = private.table.as_ref().ok_or("missing private CD01")?;
            let slots: Vec<String> = std::iter::once(player_slot)
                .chain((0..=opponents).filter(|&i| i != player_slot))
                .map(|i| format!("slot_{i:02}"))
                .collect();
            let entries: Vec<(&str, &Car)> =
                slots.iter().map(String::as_str).zip(models.iter()).collect();
            let mut multi = MultiRuntime::load_deforming_on(
                root,
                &route,
                &entries,
                table,
                runtime.body_events,
                deformation,
            )?;
            if scenario.props {
                multi.enable_dynamic_props(root)?;
            }
            let characteristics: &[BTreeMap<String, f32>] = if scenario.characteristics {
                &private.characteristics
            } else {
                &[]
            };
            let car_ai: Vec<AiData> = models[1..].iter().map(|m| by_model[m].clone()).collect();
            // Opponent k uses driver row k (row 0 is the player's slot).
            let drivers = (0..opponents)
                .map(|i| -> Result<AiDriver, String> {
                    let row = characteristics.get((i + 1) % characteristics.len().max(1));
                    let p = Parameters::new(&car_ai[i], NORMAL_DIFFICULTY, |name| {
                        row.and_then(|r| r.get(name).copied()).unwrap_or(1.)
                    })?;
                    Ok(AiDriver::new(p, 0x5eed + i as u32 + 1))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let settings = crate::ai_pacing::Settings::default();
            let start = {
                let gate = &crate::lap_progress::load_route(root, &route)?
                    .progress
                    .gates[0];
                std::array::from_fn(|k| (gate.left[k] + gate.right[k]) * 0.5)
            };
            let mut race = Race {
                multi,
                drivers,
                car_ai,
                pacing: crate::ai_pacing::Field::new(
                    crate::ai_pacing::Data::load(root, &settings)?,
                    settings,
                    &ai.line,
                    scenario.laps.unwrap_or(1),
                    start,
                    &[],
                    0,
                    (0..=opponents)
                        .map(|k| {
                            characteristics
                                .get(k % characteristics.len().max(1))
                                .and_then(|r| r.get("mistakes").copied())
                                .unwrap_or(1.)
                        })
                        .collect(),
                    0x5eed,
                ),
            };
            let positions = race.positions(&runtime);
            race.pacing.reset(&ai.line, &positions);
            Some(race)
        } else {
            None
        };
        let countdown = match scenario.laps {
            Some(laps) => Some(State::new(
                laps,
                f64::from(DELTA),
                scenario.countdown(start_sequence),
            )?),
            None => None,
        };
        let progress = crate::lap_progress::load_route(root, &route)?.progress;
        let centreline = progress
            .points
            .iter()
            .map(|p| (p.distance, p.position))
            .collect();
        let road = runtime.track().bounds();
        let bounds = std::array::from_fn(|k| {
            if k < 3 {
                road[k] - BOUNDS_MARGIN[k]
            } else {
                road[k] + BOUNDS_MARGIN[k - 3] * if k == 4 { 3. } else { 1. }
            }
        });
        Ok(Self {
            scenario: scenario.clone(),
            runtime,
            geometry,
            race,
            ai,
            countdown,
            start_sequence,
            centreline,
            bounds,
            total_ticks: 0,
            resets: 0,
            auto_reset: PlayerReset::default(),
            auto_resets: 0,
            expected_losses: Default::default(),
        })
    }

    /// The countdown holds the cars (app `held`).
    pub fn held(&self) -> bool {
        matches!(
            self.countdown.as_ref().map(State::phase),
            Some(Phase::Countdown(_))
        )
    }

    /// One app tick (grid-app `drive_tick`) with the player's controls.
    pub fn step(&mut self, player: Controls, shift: Option<Shift>) -> Result<(), grid_world::Error> {
        let held = self.held();
        if held && let Some(c) = &mut self.countdown {
            c.countdown_tick();
        }
        if let Some(shift) = shift {
            // The app ignores rejected requests (only invalid direct gears fail).
            let _ = self.runtime.request_shift(shift);
        }
        let controls = if held {
            Controls {
                throttle: player.throttle,
                handbrake: true,
                ..Default::default()
            }
        } else {
            player
        };
        let result = if let Some(race) = &mut self.race {
            let mut all = vec![controls];
            let cars: Vec<Other> = std::iter::once(&self.runtime)
                .chain(race.multi.cars.iter().skip(1))
                .enumerate()
                .map(|(i, c)| Other::of(i as u32 + 1, c))
                .collect();
            // Gentle blips first, hard revving in the second half of the countdown.
            let rev_state = match self.countdown.as_ref().map(State::phase) {
                Some(Phase::Countdown(left))
                    if f64::from(*left) * f64::from(DELTA) > self.start_sequence / 2. =>
                {
                    2
                }
                _ => 3,
            };
            // The app's race manager (no standings here: retired cars stop racing).
            if !held {
                let racing: Vec<bool> = (0..race.multi.cars.len())
                    .map(|i| race.multi.retirement(i).is_none())
                    .collect();
                race.pacing.tick_with_player(
                    &self.ai.line,
                    &self.runtime,
                    &mut race.multi,
                    &mut race.drivers,
                    &racing,
                    DELTA,
                );
            }
            for (i, driver) in race.drivers.iter_mut().enumerate() {
                let car = &race.multi.cars[i + 1];
                all.push(if held {
                    Controls {
                        throttle: driver.grid_rev(rev_state, DELTA),
                        handbrake: true,
                        ..Default::default()
                    }
                } else {
                    let others: Vec<Other> = cars
                        .iter()
                        .enumerate()
                        .filter(|&(j, _)| j != i + 1)
                        .map(|(_, o)| *o)
                        .collect();
                    let view = View::of(car, race.multi.obstruction(i + 1));
                    driver.controls(&race.car_ai[i], &view, &others, DELTA)
                });
            }
            std::mem::swap(&mut self.runtime, &mut race.multi.cars[0]);
            let result = race.multi.tick(&all);
            std::mem::swap(&mut self.runtime, &mut race.multi.cars[0]);
            if result.is_ok() {
                // The player's own records, as the app reports them.
                self.runtime.last_body_contacts = race.multi.body_contacts_of(0);
            }
            result
        } else if let Some(geometry) = &mut self.geometry {
            if self.runtime.deformation_enabled {
                self.runtime
                    .tick_with_deforming_body_geometry(controls, geometry)
            } else {
                self.runtime.tick_with_body_geometry(controls, geometry)
            }
        } else {
            self.runtime.tick(controls)
        };
        if result.is_ok() {
            self.total_ticks += 1;
            // The app logs a failed placement and drives on; here it is a
            // failure to report.
            if self
                .auto_reset
                .update(
                    &mut self.runtime,
                    self.race.as_mut().map(|r| &mut r.multi),
                    &self.ai.line,
                )?
                .is_some()
            {
                self.auto_resets += 1;
            }
        }
        result
    }

    /// `R` (grid-app `drive_actions`).
    pub fn reset(&mut self) {
        self.runtime.reset();
        if let Some(race) = &mut self.race {
            race.multi.reset();
            for driver in &mut race.drivers {
                driver.reset();
            }
            let positions = race.positions(&self.runtime);
            race.pacing.reset(&self.ai.line, &positions);
        }
        if let Some(c) = &mut self.countdown {
            c.reset();
        }
        self.auto_reset.reset();
        self.resets += 1;
    }

    /// World bodies after the last tick: cars first, then live prop pieces.
    pub fn snapshot(&self) -> Snapshot<'_> {
        match &self.race {
            Some(race) => Snapshot {
                bodies: race.multi.bodies(),
                cars: race.multi.cars.len(),
            },
            None => Snapshot {
                bodies: &self.runtime.state.world.groups.bodies,
                cars: 1,
            },
        }
    }

    pub fn props(&self) -> Option<&crate::dynamic_props::PropWorld> {
        match &self.race {
            Some(race) => race.multi.dynamic_props(),
            None => self.runtime.dynamic_props(),
        }
    }

    /// Car `i` runtime (0 is the player).
    pub fn car(&self, i: usize) -> &Runtime {
        match (&self.race, i) {
            (_, 0) | (None, _) => &self.runtime,
            (Some(race), i) => &race.multi.cars[i],
        }
    }

    pub fn car_count(&self) -> usize {
        self.race.as_ref().map_or(1, |r| r.multi.cars.len())
    }

    /// Whether prop body `i`, found below the world or falling through it,
    /// is an expected loss (see `expected_losses`); records it the first time.
    fn expected_loss(&self, i: usize, position: [f32; 3]) -> bool {
        if self
            .expected_losses
            .borrow()
            .iter()
            .any(|l| l.body == i && l.segment == self.resets)
        {
            return true;
        }
        let (Some(props), Some(geometry)) = (self.props(), &self.geometry) else {
            return false;
        };
        let car = match &self.race {
            Some(race) => &race.multi.cars[0],
            None => &self.runtime,
        };
        let Ok(Some(pressed)) =
            props.pressed_down_at_spawn(i, car.track(), car.surfaces(), geometry.query())
        else {
            return false;
        };
        self.expected_losses.borrow_mut().push(ExpectedLoss {
            segment: self.resets,
            tick: self.total_ticks,
            body: i,
            position,
            pressed,
        });
        true
    }

    /// Distance (xz) to the nearest route centreline point and its height.
    fn line_note(&self, position: [f32; 3]) -> String {
        self.centreline
            .iter()
            .map(|(_, p)| ((p[0] - position[0]).hypot(p[2] - position[2]), p[1]))
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map_or_else(String::new, |(d, y)| {
                format!("{d:.0} m from the centreline at height {y:.0}")
            })
    }

    /// A car retired by its own failed stage, non-finite state, a body outside
    /// the world or an implausible speed.
    pub fn check(&self) -> Option<(Kind, String)> {
        if let Some(race) = &self.race
            && let Some(&i) = race.multi.newly_retired().first()
            && let Some(retirement) = race.multi.retirement(i)
        {
            // The app keeps racing without the car (S3); it is still a failed stage.
            return Some((
                Kind::Retired,
                format!(
                    "car retired: {} (car {i} at tick {})",
                    retirement.error, retirement.tick
                ),
            ));
        }
        let snapshot = self.snapshot();
        for (i, body) in snapshot.bodies.iter().enumerate() {
            let what = if i < snapshot.cars {
                format!("car {i}")
            } else {
                format!("prop piece body {i}")
            };
            let pose = body_floats(body, 0x1d0, 16);
            let angular = body_floats(body, 0x210, 3);
            let velocity = body_floats(body, 0x220, 3);
            if pose
                .iter()
                .chain(&angular)
                .chain(&velocity)
                .any(|v| !v.is_finite())
            {
                return Some((
                    Kind::NonFinite,
                    format!("non-finite pose or velocity of {what}"),
                ));
            }
            let position = [pose[12], pose[13], pose[14]];
            let lost = (0..3)
                .any(|k| position[k] < self.bounds[k] || position[k] > self.bounds[k + 3])
                || velocity[1] < -MAX_FALL_SPEED;
            if lost && i >= snapshot.cars && self.expected_loss(i, position) {
                continue;
            }
            if (0..3).any(|k| position[k] < self.bounds[k] || position[k] > self.bounds[k + 3]) {
                let way = if position[1] < self.bounds[1] {
                    "fell below the world"
                } else if position[1] > self.bounds[4] {
                    "rose above the world"
                } else {
                    "left the world bounds horizontally"
                };
                return Some((
                    Kind::OutOfBounds,
                    format!(
                        "{} {way} (body {i} at {:.0},{:.0},{:.0}, {})",
                        if i < snapshot.cars { "car" } else { "prop piece" },
                        position[0],
                        position[1],
                        position[2],
                        self.line_note(position)
                    ),
                ));
            }
            // No circuit has a drop long enough for 25 m/s of free fall
            // (about 32 m): the body has lost the ground.
            if velocity[1] < -MAX_FALL_SPEED {
                return Some((
                    Kind::OutOfBounds,
                    format!(
                        "{} falling through the world (body {i} at {:.0},{:.0},{:.0}, {})",
                        if i < snapshot.cars { "car" } else { "prop piece" },
                        position[0],
                        position[1],
                        position[2],
                        self.line_note(position)
                    ),
                ));
            }
            let speed = velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
            let limit = if i < snapshot.cars {
                MAX_CAR_SPEED
            } else {
                MAX_PIECE_SPEED
            };
            if speed > limit {
                return Some((
                    Kind::Speed,
                    format!(
                        "{} faster than {limit} m/s",
                        if i < snapshot.cars { "car" } else { "prop piece" }
                    ),
                ));
            }
        }
        None
    }
}

fn speed_of(runtime: &Runtime) -> f32 {
    let v = runtime.velocity();
    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
}

/// Chaos player modes (AUTHORED adversarial behaviour, not original GRID).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    /// The ported AI drives the player's car (app GRID_AUTOPILOT), so chaos
    /// happens all around the lap.
    Autopilot,
    /// Random held inputs, often at the extremes.
    Random,
    /// Full lock and full throttle, random handbrake.
    FullLock,
    /// Aim at the nearest intact breakable prop ahead.
    Prop,
    /// Aim at a point well beyond the track edge (barriers, walls).
    Wall,
    /// Aim at the nearest other car.
    Ram,
    /// Steady part lock off the road.
    OffTrack,
    /// Brake-to-reverse (automatic) or reverse gear (manual).
    Reverse,
    /// GRID_SPIN_CAR0 style: full lock, throttle and handbrake.
    Spin,
    /// Stop on the racing line and wait (GRID_STOP_CAR0 style).
    Stop,
}

impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Mode::Autopilot => "autopilot",
            Mode::Random => "random",
            Mode::FullLock => "full-lock",
            Mode::Prop => "prop",
            Mode::Wall => "wall",
            Mode::Ram => "ram",
            Mode::OffTrack => "off-track",
            Mode::Reverse => "reverse",
            Mode::Spin => "spin",
            Mode::Stop => "stop",
        }
    }
    pub fn parse(s: &str) -> Result<Self, String> {
        MODES
            .iter()
            .map(|m| m.0)
            .find(|m| m.name() == s)
            .ok_or(format!("unknown chaos mode `{s}`"))
    }
}

const MODES: [(Mode, u32); 10] = [
    (Mode::Autopilot, 26),
    (Mode::Random, 10),
    (Mode::FullLock, 7),
    (Mode::Prop, 18),
    (Mode::Wall, 9),
    (Mode::Ram, 10),
    (Mode::OffTrack, 6),
    (Mode::Reverse, 5),
    (Mode::Spin, 6),
    (Mode::Stop, 3),
];

pub struct Decision {
    pub step: Step,
    pub mode: Mode,
}

/// The adversarial player. Every choice comes from its seeded RNG and the
/// session state, so (scenario, seed) reproduces the run exactly.
pub struct Chaos {
    rng: Rng,
    pub mode: Mode,
    left: u32,
    held: Controls,
    held_left: u32,
    side: f32,
    pilot: AiDriver,
    parameters: Parameters,
    reversing: bool,
    /// Breakable being aimed at and the ticks spent on it; given-up targets.
    aim: Option<(usize, u32)>,
    skipped: Vec<usize>,
}

/// Angle of a world point from the car's forward axis (positive to the left).
fn bearing(pose: [f32; 16], aim: [f32; 3]) -> f32 {
    let to = [aim[0] - pose[12], aim[2] - pose[14]];
    let x = to[0] * pose[0] + to[1] * pose[2];
    let z = to[0] * pose[8] + to[1] * pose[10];
    x.atan2(z)
}

/// Steering toward a world point (as `dynamic_props::PropPursuit`).
fn steer_to(pose: [f32; 16], aim: [f32; 3]) -> f32 {
    (1.5 * bearing(pose, aim)).clamp(-1., 1.)
}

fn distance2(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).powi(2) + (a[2] - b[2]).powi(2)
}

impl Chaos {
    pub fn new(seed: u64, session: &Session) -> Result<Self, String> {
        let parameters = Parameters::new(&session.ai, NORMAL_DIFFICULTY, |_| 1.)?;
        Ok(Self {
            rng: Rng::new(seed ^ 0xc4a0_5000_0000_0001),
            mode: Mode::Autopilot,
            left: 0,
            held: Controls::default(),
            held_left: 0,
            side: 1.,
            pilot: AiDriver::new(parameters.clone(), 0x5eed),
            parameters,
            reversing: false,
            aim: None,
            skipped: Vec::new(),
        })
    }

    /// After `R`: the app's autopilot is rebuilt and the countdown restarts.
    pub fn on_reset(&mut self) {
        self.pilot = AiDriver::new(self.parameters.clone(), 0x5eed);
        self.left = 0;
        self.reversing = false;
        self.aim = None;
        self.skipped.clear();
    }

    fn pick_mode(&mut self, forced: Option<Mode>) {
        let total: u32 = MODES.iter().map(|m| m.1).sum();
        let mut r = self.rng.below(u64::from(total)) as u32;
        for (mode, weight) in MODES {
            if r < weight {
                self.mode = mode;
                break;
            }
            r -= weight;
        }
        if let Some(mode) = forced {
            self.mode = mode;
        }
        self.left = match self.mode {
            Mode::Spin => 60,
            Mode::Autopilot => 150 + self.rng.below(600) as u32,
            Mode::Stop => 120 + self.rng.below(300) as u32,
            _ => 60 + self.rng.below(300) as u32,
        };
        self.side = self.rng.sign();
        self.held_left = 0;
    }

    fn autopilot(&mut self, session: &Session) -> Controls {
        let car = &session.runtime;
        let (others, obstruction) = match &session.race {
            Some(race) => (
                race.multi
                    .cars
                    .iter()
                    .enumerate()
                    .skip(1)
                    .map(|(i, c)| Other::of(i as u32 + 1, c))
                    .collect(),
                race.multi.obstruction(0),
            ),
            None => (Vec::new(), [0.; 3]),
        };
        let view = View::of(car, obstruction);
        self.pilot.controls(&session.ai, &view, &others, DELTA)
    }

    fn random_controls(&mut self) -> Controls {
        let pick = |rng: &mut Rng, lo: f32| match rng.below(4) {
            0 => lo,
            1 => 1.,
            2 => 0.,
            _ => rng.range(lo, 1.),
        };
        Controls {
            throttle: pick(&mut self.rng, 0.),
            brake: if self.rng.chance(0.3) {
                pick(&mut self.rng, 0.)
            } else {
                0.
            },
            steering: pick(&mut self.rng, -1.),
            handbrake: self.rng.chance(0.2),
        }
    }

    /// Manual gearbox: shift with speed, plus occasional random requests.
    fn manual_shift(&mut self, session: &Session) -> Option<Shift> {
        if session.runtime.assists.transmission != Transmission::Manual || self.reversing {
            return None;
        }
        if self.rng.chance(0.002) {
            return Some(match self.rng.below(3) {
                0 => Shift::Up,
                1 => Shift::Down,
                _ => Shift::Gear(1 + self.rng.below(6) as i8),
            });
        }
        if !session.total_ticks.is_multiple_of(30) {
            return None;
        }
        let gear = session.runtime.gear();
        let target = (1 + (speed_of(&session.runtime) / 9.) as u32).clamp(1, 6);
        if gear == 0 || gear == 9 {
            Some(Shift::Gear(1))
        } else if gear < target {
            Some(Shift::Up)
        } else if gear > target + 1 {
            Some(Shift::Down)
        } else {
            None
        }
    }

    /// The next step: a reset or a tick with the player's inputs.
    pub fn decide(&mut self, session: &Session) -> Decision {
        let segment_ticks = session.runtime.ticks;
        if segment_ticks > 300
            && self
                .rng
                .chance(session.scenario.reset_rate / 10_000.)
        {
            return Decision {
                step: Step::Reset,
                mode: self.mode,
            };
        }
        // Leaving manual reverse selects first gear again.
        let mut shift = None;
        if self.left == 0 {
            if self.reversing {
                self.reversing = false;
                shift = Some(Shift::Gear(1));
            }
            self.pick_mode(session.scenario.chaos);
        }
        self.left -= 1;
        let pose = session.runtime.pose();
        let position = [pose[12], pose[13], pose[14]];
        let controls = match self.mode {
            Mode::Autopilot => self.autopilot(session),
            Mode::Random => {
                if self.held_left == 0 {
                    self.held = self.random_controls();
                    self.held_left = 1 + self.rng.below(20) as u32;
                }
                self.held_left -= 1;
                self.held
            }
            Mode::FullLock => Controls {
                throttle: 1.,
                steering: self.side,
                handbrake: self.rng.chance(0.1),
                ..Default::default()
            },
            Mode::Spin => Controls {
                throttle: 1.,
                steering: self.side,
                handbrake: true,
                ..Default::default()
            },
            Mode::Prop => {
                // Follow the AI line until an intact breakable is close and
                // nearly ahead, then drive straight at it; give up after 3 s.
                let props = session.props();
                if let Some((index, ticks)) = self.aim {
                    let broken = props.is_none_or(|p| p.broken.get(index).copied().unwrap_or(true));
                    if broken || ticks >= 190 {
                        if !broken {
                            self.skipped.push(index);
                        }
                        self.aim = None;
                    }
                }
                if self.aim.is_none()
                    && let Some(props) = props
                {
                    self.aim = props
                        .catalog
                        .breakables
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| !props.broken[*i] && !self.skipped.contains(i))
                        .map(|(i, b)| (i, [b.placement[12], b.placement[13], b.placement[14]]))
                        .filter(|(_, p)| {
                            distance2(*p, position) < 45. * 45. && bearing(pose, *p).abs() < 0.9
                        })
                        .min_by(|a, b| {
                            distance2(a.1, position).total_cmp(&distance2(b.1, position))
                        })
                        .map(|(i, _)| (i, 0));
                }
                match (self.aim, props) {
                    (Some((index, ticks)), Some(props)) => {
                        self.aim = Some((index, ticks + 1));
                        let b = &props.catalog.breakables[index];
                        Controls {
                            throttle: 1.,
                            steering: steer_to(pose, [b.placement[12], b.placement[13], b.placement[14]]),
                            ..Default::default()
                        }
                    }
                    _ => self.autopilot(session),
                }
            }
            Mode::Ram => {
                let target = (1..session.car_count())
                    .map(|i| {
                        let p = session.car(i).pose();
                        [p[12], p[13], p[14]]
                    })
                    .filter(|p| distance2(*p, position) < 40. * 40. && bearing(pose, *p).abs() < 1.2)
                    .min_by(|a, b| distance2(*a, position).total_cmp(&distance2(*b, position)));
                match target {
                    Some(p) => Controls {
                        throttle: 1.,
                        steering: steer_to(pose, p),
                        ..Default::default()
                    },
                    None => self.autopilot(session),
                }
            }
            Mode::Wall => {
                // 40 m sideways from the nearest centreline point, 15 m ahead.
                let nearest = session
                    .centreline
                    .iter()
                    .enumerate()
                    .min_by(|a, b| {
                        distance2(a.1.1, position).total_cmp(&distance2(b.1.1, position))
                    })
                    .map_or(0, |(i, _)| i);
                let n = session.centreline.len().max(1);
                let a = session.centreline.get(nearest).map_or(position, |p| p.1);
                let b = session
                    .centreline
                    .get((nearest + 3) % n)
                    .map_or(position, |p| p.1);
                let d = [b[0] - a[0], b[2] - a[2]];
                let l = (d[0] * d[0] + d[1] * d[1]).sqrt().max(1e-3);
                let (dx, dz) = (d[0] / l, d[1] / l);
                let aim = [
                    a[0] + dx * 15. - dz * 40. * self.side,
                    a[1],
                    a[2] + dz * 15. + dx * 40. * self.side,
                ];
                Controls {
                    throttle: 1.,
                    steering: steer_to(pose, aim),
                    ..Default::default()
                }
            }
            Mode::OffTrack => Controls {
                throttle: 0.8,
                steering: 0.45 * self.side,
                ..Default::default()
            },
            Mode::Reverse => {
                let manual = session.runtime.assists.transmission == Transmission::Manual;
                if manual && !self.reversing {
                    self.reversing = true;
                    shift = Some(Shift::Gear(-1));
                }
                Controls {
                    throttle: if manual { 1. } else { 0. },
                    brake: if manual { 0. } else { 1. },
                    steering: 0.6 * self.side,
                    ..Default::default()
                }
            }
            Mode::Stop => Controls {
                brake: 1.,
                ..Default::default()
            },
        };
        if shift.is_none() {
            shift = self.manual_shift(session);
        }
        Decision {
            step: Step::Tick(controls, shift),
            mode: self.mode,
        }
    }
}

/// One segment of an app playtest log (between resets).
#[derive(Clone, Debug, Default)]
pub struct LogSegment {
    /// (1-based tick index, controls, shift) change points.
    pub inputs: Vec<(u64, Controls, Option<Shift>)>,
    /// Ticks the app completed in this segment.
    pub ticks: u64,
    /// The failure the app reported at tick `ticks + 1`, if any.
    pub failure: Option<String>,
    /// Logged player poses after tick n (drive_sample), for verification.
    pub poses: Vec<(u64, [f32; 16])>,
    /// How the segment ended (reset, failure, exit or end of log).
    pub end: String,
}

/// Setup and inputs recovered from a grid-app `--drive` log.
#[derive(Clone, Debug, Default)]
pub struct AppLog {
    pub route: Option<String>,
    /// `drive_replay` car and opponent_cars (absent: all-Lacetti).
    pub car: Option<Car>,
    pub opponent_cars: Option<Vec<Car>>,
    pub opponents: usize,
    pub grid_position: Option<usize>,
    pub assists: Option<Assists>,
    pub body_contacts: bool,
    pub body_events: bool,
    pub deformation: bool,
    pub characteristics: bool,
    pub laps: Option<u32>,
    /// The log carries explicit `drive_input` lines (otherwise the
    /// `drive_sample` control changes are used).
    pub explicit_inputs: bool,
    pub segments: Vec<LogSegment>,
}

fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.split_whitespace()
        .find_map(|item| item.strip_prefix(key)?.strip_prefix('='))
}

fn bracket<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let start = line.find(&format!(" {key}=["))? + key.len() + 3;
    let end = line[start..].find(']')? + start;
    Some(&line[start..end])
}

fn controls_of(line: &str) -> Option<Controls> {
    Some(Controls {
        throttle: field(line, "throttle")?.parse().ok()?,
        brake: field(line, "brake")?.parse().ok()?,
        steering: field(line, "steering")?.parse().ok()?,
        handbrake: field(line, "handbrake")? == "true",
    })
}

impl AppLog {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut log = AppLog {
            explicit_inputs: text.contains("drive_input version=1"),
            ..Default::default()
        };
        let mut current = LogSegment::default();
        let mut segment = 0u64;
        let mut pending_failure: Option<String> = None;
        let finish = |log: &mut AppLog, current: &mut LogSegment, end: &str| {
            current.end = end.to_string();
            log.segments.push(std::mem::take(current));
        };
        for raw in text.lines() {
            let line = raw.trim();
            if let Some(rest) = line.strip_prefix("drive_replay ") {
                log.route = field(rest, "route").map(str::to_string);
                log.grid_position = field(rest, "grid_position").and_then(|v| v.parse().ok());
                log.car = field(rest, "car")
                    .map(Car::new)
                    .transpose()
                    .map_err(|e| format!("drive_replay car: {e}"))?;
                log.opponent_cars = field(rest, "opponent_cars")
                    .map(Car::list)
                    .transpose()
                    .map_err(|e| format!("drive_replay opponent_cars: {e}"))?;
            } else if let Some(rest) = line.strip_prefix("drive_assists ") {
                let mut assists = Assists::parse(rest.trim())?;
                // Builds before F333 drove the player at damage gain 1.0.
                if !rest.contains("damage=") {
                    assists.damage = grid_world::mechanical_damage::Damage::Normal;
                }
                log.assists = Some(assists);
            } else if line.starts_with("drive_opponents enabled=true") {
                log.opponents = field(line, "count")
                    .and_then(|v| v.parse().ok())
                    .ok_or("drive_opponents without count")?;
            } else if line.starts_with("drive_body_geometry enabled=true") {
                log.body_contacts = true;
            } else if line.starts_with("drive_body_events enabled=true") {
                log.body_events = true;
            } else if line.starts_with("drive_deformation enabled=true") {
                log.deformation = true;
            } else if line.starts_with("drive_ai_characteristics ") {
                log.characteristics = true;
            } else if line.starts_with("drive_trial_event version=1 type=countdown")
                && log.laps.is_none()
                && let Some(laps) = field(line, "laps")
            {
                log.laps = Some(laps.parse().map_err(|_| "bad trial laps")?);
            } else if let Some(i) = line.find("driving paused: ") {
                pending_failure = Some(line[i + "driving paused: ".len()..].to_string());
            } else if line.starts_with("drive_input version=1") {
                let tick: u64 = field(line, "tick")
                    .and_then(|v| v.parse().ok())
                    .ok_or("drive_input without tick")?;
                let c = controls_of(line).ok_or("drive_input without controls")?;
                let shift = parse_shift(field(line, "shift").unwrap_or("-"))?;
                current.inputs.push((tick, c, shift));
            } else if line.starts_with("drive_sample version=1") {
                let event = field(line, "event").unwrap_or("");
                let seg: u64 = field(line, "segment")
                    .and_then(|v| v.parse().ok())
                    .ok_or("drive_sample without segment")?;
                let tick: u64 = field(line, "tick")
                    .and_then(|v| v.parse().ok())
                    .ok_or("drive_sample without tick")?;
                if seg != segment {
                    // A reset sample always precedes a new segment; be robust anyway.
                    if !current.inputs.is_empty() || current.ticks > 0 {
                        finish(&mut log, &mut current, "segment-change");
                    }
                    segment = seg;
                }
                let c = controls_of(line).ok_or("drive_sample without controls")?;
                match event {
                    "tick" => {
                        current.ticks = current.ticks.max(tick);
                        if !log.explicit_inputs
                            && current.inputs.last().is_none_or(|last| last.1 != c)
                        {
                            current.inputs.push((tick, c, None));
                        }
                        if let Some(pose) = bracket(line, "pose") {
                            let values: Vec<f32> = pose
                                .split(',')
                                .filter_map(|v| v.trim().parse().ok())
                                .collect();
                            if let Ok(pose) = <[f32; 16]>::try_from(values) {
                                current.poses.push((tick, pose));
                            }
                        }
                    }
                    "failure" => {
                        current.ticks = tick;
                        if !log.explicit_inputs
                            && current.inputs.last().is_none_or(|last| last.1 != c)
                        {
                            current.inputs.push((tick + 1, c, None));
                        }
                        current.failure = Some(
                            pending_failure
                                .take()
                                .unwrap_or_else(|| "unknown failure".into()),
                        );
                    }
                    "reset" => {
                        current.ticks = tick;
                        finish(&mut log, &mut current, "reset");
                        segment += 1;
                    }
                    "exit" => {
                        current.ticks = tick;
                        finish(&mut log, &mut current, "exit");
                    }
                    _ => {}
                }
            }
        }
        if !current.inputs.is_empty() || current.ticks > 0 || current.failure.is_some() {
            finish(&mut log, &mut current, "end-of-log");
        }
        if log.segments.is_empty() {
            return Err("no drive_sample or drive_input lines: not a grid-app --drive log".into());
        }
        Ok(log)
    }

    /// The scenario the app ran (grid position defaults to the race
    /// launcher's back of the grid when the log predates `drive_replay`).
    pub fn scenario(&self, grid_position: Option<usize>, route: Option<&str>) -> Scenario {
        let cars = self.opponents + 1;
        Scenario {
            seed: 0,
            profile: Profile::Quick,
            route: route
                .map(str::to_string)
                .or_else(|| self.route.clone())
                .unwrap_or_else(|| DONINGTON.into()),
            cars,
            grid_position: grid_position.or(self.grid_position).unwrap_or(cars),
            assists: self.assists.unwrap_or_default(),
            body_contacts: self.body_contacts,
            props: self.body_contacts,
            body_events: self.body_events,
            deformation: self.deformation,
            characteristics: self.characteristics,
            laps: self.laps,
            ticks: self.segments.iter().map(|s| s.ticks + 1).sum(),
            reset_rate: 0.,
            chaos: None,
            car: self.car.clone().unwrap_or_default(),
            opponent_cars: self.opponent_cars.clone().unwrap_or_default(),
        }
    }

    /// The input steps of every segment, resets in between. A segment that
    /// ended in a failure includes the failing tick.
    pub fn steps(&self) -> Vec<Step> {
        let mut steps = Vec::new();
        for (k, segment) in self.segments.iter().enumerate() {
            if k > 0 {
                steps.push(Step::Reset);
            }
            let end = segment.ticks + u64::from(segment.failure.is_some());
            let mut next = 0;
            let mut controls = Controls::default();
            for tick in 1..=end {
                let mut shift = None;
                while next < segment.inputs.len() && segment.inputs[next].0 <= tick {
                    controls = segment.inputs[next].1;
                    if segment.inputs[next].0 == tick {
                        shift = shift.or(segment.inputs[next].2);
                    }
                    next += 1;
                }
                steps.push(Step::Tick(controls, shift));
            }
        }
        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn rng_is_reproducible_and_in_range() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        for _ in 0..1000 {
            let (x, y) = (a.unit(), b.unit());
            assert_eq!(x.to_bits(), y.to_bits());
            assert!((0. ..1.).contains(&x));
        }
        assert_ne!(Rng::new(1).next_u64(), Rng::new(2).next_u64());
    }

    #[test]
    fn scenarios_round_trip_through_the_header() {
        let available = Available {
            events: true,
            deformation: true,
            characteristics: true,
        };
        for seed in 1..200 {
            for profile in [Profile::Quick, Profile::Long] {
                let s = Scenario::generate(seed, profile, available, 12);
                assert_eq!(Scenario::parse(&s.header()).unwrap(), s, "{}", s.header());
                assert!((1..=s.cars).contains(&s.grid_position));
                assert!((1..=12).contains(&s.cars));
            }
        }
        assert_eq!(Scenario::generate(1, Profile::Quick, available, 12).cars, 12);
    }

    #[test]
    fn long_runs_mix_models_and_quick_runs_stay_lacetti() {
        let available = Available {
            events: true,
            deformation: true,
            characteristics: true,
        };
        let mixed = (1..200)
            .map(|seed| Scenario::generate(seed, Profile::Long, available, 12))
            .filter(|s| s.field_models().iter().collect::<BTreeSet<_>>().len() == 2)
            .count();
        assert!((60..140).contains(&mixed), "{mixed}");
        for seed in 1..=12 {
            let s = Scenario::generate(seed, Profile::Quick, available, 12);
            assert!(s.field_models().iter().all(Car::is_default));
            assert!(!s.header().contains("car="));
        }
        let mut s = Scenario::generate(1, Profile::Quick, available, 12);
        s.car = Car::new("bmw").unwrap();
        s.cars = 4;
        let stems = |s: &Scenario| s.field_models().iter().map(|c| c.to_string()).collect::<Vec<_>>();
        assert_eq!(stems(&s), ["bmw"; 4]);
        s.opponent_cars = Car::list("lac,bmw").unwrap();
        assert_eq!(stems(&s), ["bmw", "lac", "bmw", "lac"]);
        assert!(s.header().ends_with(" car=bmw opponent_cars=lac,bmw"));
        assert_eq!(Scenario::parse(&s.header()).unwrap(), s);
    }

    #[test]
    fn new_circuits_join_without_moving_the_earlier_seeds() {
        let available = Available {
            events: true,
            deformation: true,
            characteristics: true,
        };
        let quick = |seed| Scenario::generate(seed, Profile::Quick, available, 12);
        // F326 assignment of quick seeds 1..=12.
        for seed in 1..=12 {
            let route = if seed % 4 >= 2 { NURBURGRING } else { DONINGTON };
            assert_eq!(quick(seed).route, route, "{seed}");
        }
        let istanbul: Vec<_> = (13..=16).map(quick).collect();
        assert!(istanbul.iter().all(|s| s.route == ISTANBUL_PARK));
        assert_eq!(istanbul.iter().map(|s| s.cars).collect::<Vec<_>>(), [2, 12, 9, 4]);
        assert_eq!(QUICK_SEEDS, (1, 24));
        let long_beach: Vec<_> = (17..=20).map(quick).collect();
        assert!(long_beach.iter().all(|s| s.route == LONG_BEACH));
        assert_eq!(
            long_beach.iter().map(|s| s.cars).collect::<Vec<_>>(),
            [12, 1, 12, 6]
        );
        let jarama: Vec<_> = (21..=24).map(quick).collect();
        assert!(jarama.iter().all(|s| s.route == JARAMA));
        assert_eq!(
            jarama.iter().map(|s| s.cars).collect::<Vec<_>>(),
            [2, 12, 9, 4]
        );
        let derby: Vec<_> = (33..=36).map(quick).collect();
        assert_eq!(
            derby.iter().map(|s| (s.route.as_str(), s.cars)).collect::<Vec<_>>(),
            [
                (DETROIT_STADIUM, 12),
                (DETROIT_STADIUM_SHORT, 12),
                (DETROIT_STADIUM, 6),
                (DETROIT_STADIUM_SHORT, 12)
            ]
        );
        assert!(derby.iter().all(|s| s.car.stem() == "stk"));
        let haruna: Vec<_> = (29..=32).map(quick).collect();
        assert!(haruna.iter().all(|s| s.route == HARUNA));
        assert_eq!(haruna.iter().map(|s| s.cars).collect::<Vec<_>>(), [1, 2, 1, 2]);
        for (seeds, route, cars) in [
            (37..=40, SPA, [2, 12, 9, 4]),
            (41..=44, MILAN, [12, 1, 12, 6]),
            (45..=48, SHIBUYA, [2, 12, 9, 4]),
            (49..=52, SAN_FRANCISCO, [12, 1, 12, 6]),
        ] {
            let s: Vec<_> = seeds.map(quick).collect();
            assert!(s.iter().all(|s| s.route == route), "{route}");
            assert_eq!(s.iter().map(|s| s.cars).collect::<Vec<_>>(), cars, "{route}");
            assert!(s.iter().all(|s| s.car.stem() == "lac"), "{route}");
        }
        let yokohama: Vec<_> = (25..=28).map(quick).collect();
        assert!(yokohama.iter().all(|s| s.route == YOKOHAMA));
        assert_eq!(
            yokohama.iter().map(|s| s.cars).collect::<Vec<_>>(),
            [12, 1, 12, 6]
        );
        // Long runs: about a sixth each on Istanbul Park (a third, less the later
        // moves), Long Beach (a quarter, less the later moves), Jarama (a fifth,
        // less Yokohama's), Yokohama and Spa (a seventh), from their own
        // streams, so the other fields of a seed keep their F326 draws.
        let on = |route: &str| {
            (1001..1401)
                .filter(|&seed| Scenario::generate(seed, Profile::Long, available, 12).route == route)
                .count()
        };
        for route in [ISTANBUL_PARK, LONG_BEACH, JARAMA, YOKOHAMA, SPA] {
            assert!((30..95).contains(&on(route)), "{route} {}", on(route));
        }
        for route in [MILAN, SHIBUYA, SAN_FRANCISCO] {
            assert!((25..70).contains(&on(route)), "{route} {}", on(route));
        }
    }

    #[test]
    fn app_logs_name_a_mixed_field() {
        let parsed = AppLog::parse(
            "drive_replay version=1 route=tracks/europe/donington/route_0 opponents=3 grid_position=2 car=bmw opponent_cars=lac,bmw\n\
             drive_opponents enabled=true count=3 world=shared\n\
             drive_sample version=1 event=tick segment=0 tick=1 seconds=0.016 wall_seconds=1 throttle=0 brake=0 steering=0 handbrake=true gear=1\n",
        )
        .unwrap();
        let s = parsed.scenario(None, None);
        assert_eq!(s.car, Car::new("bmw").unwrap());
        assert_eq!(s.opponent_cars, Car::list("lac,bmw").unwrap());
        assert_eq!(s.grid_position, 2);
        let default = AppLog::parse(
            "drive_replay version=1 route=r opponents=0 grid_position=1\ndrive_sample version=1 event=tick segment=0 tick=1 seconds=0.016 wall_seconds=1 throttle=0 brake=0 steering=0 handbrake=true gear=1\n",
        )
        .unwrap()
        .scenario(None, None);
        assert!(default.car.is_default() && default.opponent_cars.is_empty());
    }

    #[test]
    fn steps_round_trip_exactly() {
        let steps = [
            Step::Tick(
                Controls {
                    throttle: 0.1 + 0.2,
                    brake: 1.,
                    steering: -0.333_333_34,
                    handbrake: true,
                },
                Some(Shift::Gear(-1)),
            ),
            Step::Tick(Controls::default(), Some(Shift::Up)),
            Step::Reset,
        ];
        for s in steps {
            assert_eq!(Step::parse(&s.line("note")).unwrap(), s);
        }
    }

    #[test]
    fn failures_group_without_numbers() {
        assert_eq!(
            group_key(Kind::Panic, "index out of bounds: the len is 16 but the index is 17"),
            "panic: index out of bounds: the len is # but the index is #"
        );
        assert_eq!(
            group_key(Kind::Speed, "car faster than 1.5 m/s"),
            "implausible-speed: car faster than # m/s"
        );
    }

    #[test]
    fn app_logs_rebuild_inputs_resets_and_failures() {
        let log = "\
drive_assists tcs=on,stability=off,braking=on,gearbox=manual,input=keyboard
drive_body_geometry enabled=true pool=20
drive_opponents enabled=true count=11 world=shared
drive_trial_event version=1 type=countdown laps=3 mode=deformation persistent=false
drive_sample version=1 event=tick segment=0 tick=1 seconds=0.016 wall_seconds=1 throttle=0 brake=0 steering=0 handbrake=true gear=1 wheel_hits=4 pose=[1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 5.0, 6.0, 7.0, 1.0] velocity=[0.0, 0.0, 0.0, 0.0]
drive_sample version=1 event=tick segment=0 tick=3 seconds=0.048 wall_seconds=1 throttle=1 brake=0 steering=0 handbrake=true gear=1
drive_sample version=1 event=reset segment=0 tick=4 seconds=0.064 wall_seconds=1 throttle=1 brake=0 steering=0 handbrake=true gear=1
drive_sample version=1 event=tick segment=1 tick=1 seconds=0.016 wall_seconds=1 throttle=0 brake=0 steering=-1 handbrake=false gear=1
\x1b[31mERROR\x1b[0m grid_app: driving paused: tick 3: invalid contact-group counts or selector; R resets
drive_sample version=1 event=failure segment=1 tick=2 seconds=0.032 wall_seconds=1 throttle=0 brake=1 steering=-1 handbrake=false gear=1
";
        let parsed = AppLog::parse(log).unwrap();
        assert_eq!(parsed.opponents, 11);
        assert_eq!(parsed.laps, Some(3));
        assert_eq!(parsed.segments.len(), 2);
        assert_eq!(parsed.segments[0].ticks, 4);
        assert_eq!(parsed.segments[0].poses[0].1[12], 5.);
        assert_eq!(
            parsed.segments[1].failure.as_deref(),
            Some("tick 3: invalid contact-group counts or selector; R resets")
        );
        let s = parsed.scenario(None, None);
        assert_eq!((s.cars, s.grid_position), (12, 12));
        assert_eq!(s.assists.transmission, Transmission::Manual);
        let throttle: Vec<f32> = parsed
            .steps()
            .iter()
            .map(|s| match s {
                Step::Tick(c, _) => c.throttle + c.brake * 10.,
                Step::Reset => -1.,
            })
            .collect();
        // Segment 0: ticks 1-2 idle, 3-4 throttle; reset; segment 1: tick 1-2
        // steer only, failing tick 3 brakes.
        assert_eq!(throttle, [0., 0., 1., 1., -1., 0., 0., 10.]);
    }

    #[test]
    fn explicit_input_lines_carry_shifts() {
        let log = "\
drive_input version=1 segment=0 tick=1 throttle=1 brake=0 steering=0 handbrake=false shift=up hold=0
drive_input version=1 segment=0 tick=3 throttle=0.5 brake=0 steering=0.25 handbrake=false shift=- hold=0
drive_sample version=1 event=tick segment=0 tick=2 seconds=0.032 wall_seconds=1 throttle=1 brake=0 steering=0 handbrake=false gear=2
drive_sample version=1 event=exit segment=0 tick=3 seconds=0.048 wall_seconds=1 throttle=0.5 brake=0 steering=0.25 handbrake=false gear=2
";
        let parsed = AppLog::parse(log).unwrap();
        assert!(parsed.explicit_inputs);
        assert_eq!(
            parsed.steps(),
            [
                Step::Tick(
                    Controls {
                        throttle: 1.,
                        ..Default::default()
                    },
                    Some(Shift::Up)
                ),
                Step::Tick(
                    Controls {
                        throttle: 1.,
                        ..Default::default()
                    },
                    None
                ),
                Step::Tick(
                    Controls {
                        throttle: 0.5,
                        steering: 0.25,
                        ..Default::default()
                    },
                    None
                ),
            ]
        );
    }
}
