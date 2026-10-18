//! Time of day on Circuit de la Sarthe (docs/LE_MANS.md §3): the race clock,
//! the `daynight.xml` keyframe curves and the headlight switching recovered
//! from the original (static analysis, `local/lemans/time-of-day.md`).
//!
//! - The track's mode comes from its `track_model` row: 1 (timeline) when
//!   `supports_time_of_day`, else 2 (forced night) when `headlights_on`, else
//!   0 [V 0x60C040]. Only `le_mans` Route_0 has mode 1.
//! - A time-limited Le Mans race runs 24 h of day in its real time limit:
//!   scale = 86400 s / limit (720 s → ×120), starting at 15:00; outside a
//!   time-limited race the clock stays at 15:00 [V 0x669700, 0x90D5A0,
//!   0x911DC3]. The clock is the race clock (from GO, stops with the race).
//! - Each daynight attribute is its own sparse curve, wrapped across
//!   midnight, unwrapped for `rotationangle` (360) and `skyTime` (1), and
//!   evaluated linearly [V 0x9DD900, 0x98CE20, 0x9ACE90].
//! - Headlights follow `enableHeadlights > 0.5` through a global fade with a
//!   3–9 s delay per car [V 0x73B890, 0x727B20, 0x736020].
//!
//! Rendering (sky, sun, fog, post-process, the headlight projectors) is the
//! graphics side; [`Timeline::values`] is its hand-off.
use crate::route::Route;
use grid_formats::daynight::DayNight;
use std::{collections::BTreeMap, path::Path};

/// Time of day at the start of every race: 15:00 [V 0xCB3BA8].
pub const START_HOURS: f64 = 15.;
/// One day in seconds; the time-limited race ends at 24:00:00 of its clock.
pub const DAY_SECONDS: f64 = 86_400.;
/// `enableHeadlights` and the other boolean outputs are on above 0.5 (strict).
pub const THRESHOLD: f32 = 0.5;

/// The track's time-of-day mode (lighting manager +0x8D58).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Fixed lighting (lighting.xml only).
    None,
    /// The daynight.xml timeline (`supports_time_of_day`).
    Timeline,
    /// Night with headlights always on (`headlights_on`, Haruna touge routes).
    ForcedNight,
}

impl Mode {
    /// From the route's `track_model` row (folder, file and route strings).
    pub fn for_route(root: &Path, route: &Route) -> Result<Self, Box<dyn std::error::Error>> {
        let schema = std::fs::read(root.join("database/schema.bin"))?;
        let data = std::fs::read(root.join("database/database.bin"))?;
        let db = grid_formats::database::read(&schema, &data)?;
        let parts: Vec<&str> = route.path().split('/').collect();
        let (folder, file, layout) = (parts[1], parts[2], parts[3]);
        let row = db
            .table("track_model")
            .ok_or("database track_model absent")?
            .rows()
            .find(|m| {
                let eq = |col: &str, v: &str| m.text(col).is_some_and(|t| t.eq_ignore_ascii_case(v));
                eq("folder_string", folder) && eq("file_string", file) && eq("route_string", layout)
            })
            .ok_or(format!("no track model for {}", route.path()))?;
        Ok(if row.int("supports_time_of_day") == Some(1) {
            Mode::Timeline
        } else if row.int("headlights_on") == Some(1) {
            Mode::ForcedNight
        } else {
            Mode::None
        })
    }
}

/// Clock multiplier of a time-limited Le Mans race: 24 h over the real time
/// limit (career 720 s → 120; free race 12 min … 24 h → 120 … 1) [V].
pub fn clock_scale(limit_seconds: f64) -> f64 {
    DAY_SECONDS / limit_seconds
}

/// Time of day (fraction of a day, 0..1) after `race_seconds` of race time
/// at clock multiplier `scale` (`None`: not a time-limited race, the clock
/// stays at the start) [V 0x90D5A0].
pub fn day_fraction(race_seconds: f64, scale: Option<f64>) -> f64 {
    let start = START_HOURS / 24.;
    match scale {
        Some(scale) => (race_seconds.max(0.) * scale / DAY_SECONDS + start).rem_euclid(1.),
        None => start,
    }
}

/// `hh:mm:ss` of a day fraction.
pub fn clock_text(t: f64) -> String {
    let s = (t.rem_euclid(1.) * DAY_SECONDS).round() as u64 % 86_400;
    format!("{:02}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// Day between 05:00 and 20:00 inclusive, else night [V 0x4178E0].
pub fn is_day(t: f64) -> bool {
    (5. ..=20.).contains(&(t * 24.))
}

/// One attribute's curve: keys (day fraction, outgoing value, incoming value)
/// after the midnight wrap and the angle unwrap.
#[derive(Clone, Debug, PartialEq)]
pub struct Curve {
    keys: Vec<(f64, f32, f32)>,
}

impl Curve {
    /// Keys in minutes of the day (as read from daynight.xml). `period` is
    /// 360 for `rotationangle`, 1 for `skyTime`, else `None`.
    pub fn new(minutes: &[(u16, f32)], period: Option<f32>) -> Self {
        let mut keys: Vec<(f64, f32, f32)> = minutes
            .iter()
            .map(|&(m, v)| (f64::from(m) / 1440., v, v))
            .collect();
        if let (Some(&lo), Some(&hi)) = (keys.first(), keys.last()) {
            // [V 0x98CE20] copies of the last key a day before and of the
            // first a day after (the file's keys are in time order).
            if lo.0 != 0. {
                keys.push((hi.0 - 1., hi.1, hi.2));
            }
            if hi.0 != 1. {
                keys.push((lo.0 + 1., lo.1, lo.2));
            }
            keys.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        if let Some(p) = period {
            let mut out = Vec::with_capacity(keys.len() * 2);
            for w in keys.windows(2) {
                let (a, b) = (w[0], w[1]);
                out.push(a);
                let d = b.1 - a.1;
                if d > p / 2. {
                    let f = a.1 / (a.1 - (b.1 - p));
                    out.push((a.0 + (b.0 - a.0) * f64::from(f), p, 0.));
                } else if d < -p / 2. {
                    let f = (p - a.1) / ((b.1 + p) - a.1);
                    out.push((a.0 + (b.0 - a.0) * f64::from(f), 0., p));
                }
            }
            out.extend(keys.last());
            keys = out;
        }
        Self { keys }
    }

    /// Linear value at day fraction `t` (0..1) [V 0x9ACE90]: from key i's
    /// outgoing value to key i+1's incoming value.
    pub fn value(&self, t: f64) -> f32 {
        let k = &self.keys;
        if k.len() < 2 {
            return k.first().map_or(0., |k| k.1);
        }
        let mut i = 0;
        while i + 2 < k.len() && k[i + 1].0 < t {
            i += 1;
        }
        let (a, b) = (k[i], k[i + 1]);
        let f = ((t - a.0) / (b.0 - a.0)) as f32;
        a.1 + (b.2 - a.1) * f
    }
}

/// The daynight timeline of a track: one curve per attribute, keyed by the
/// lower-case attribute name (the original matches names case-insensitively).
#[derive(Clone, Debug, PartialEq)]
pub struct Timeline {
    pub curves: BTreeMap<String, Curve>,
}

impl Timeline {
    pub fn from_daynight(data: &DayNight) -> Self {
        let curves = data
            .tracks
            .iter()
            .map(|(name, keys)| {
                let name = name.to_ascii_lowercase();
                let period = match name.as_str() {
                    "rotationangle" => Some(360.),
                    "skytime" => Some(1.),
                    _ => None,
                };
                let curve = Curve::new(keys, period);
                (name, curve)
            })
            .collect();
        Self { curves }
    }

    /// The track's `daynight.xml`; `None` when it ships none.
    pub fn load(root: &Path, route: &Route) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let path = root.join(route.track_file("daynight.xml"));
        if !path.exists() {
            return Ok(None);
        }
        let bytes = std::fs::read(path)?;
        if bytes.len() > 1024 * 1024 {
            return Err("daynight source exceeds bound".into());
        }
        let document = grid_formats::binxml::read(&bytes)?;
        Ok(Some(Self::from_daynight(&grid_formats::daynight::read(&document)?)))
    }

    /// The attribute's value at `t`; `None` when it has no keys (its
    /// lighting.xml value stays).
    pub fn value(&self, name: &str, t: f64) -> Option<f32> {
        self.curves
            .get(&name.to_ascii_lowercase())
            .map(|c| c.value(t))
    }

    /// A boolean attribute (`enableHeadlights`, `enableShadowSystem`,
    /// `toggleThreshold`): value > 0.5.
    pub fn flag(&self, name: &str, t: f64) -> Option<bool> {
        self.value(name, t).map(|v| v > THRESHOLD)
    }

    /// Every attribute's value at `t` (the renderer's hand-off).
    pub fn values(&self, t: f64) -> BTreeMap<&str, f32> {
        self.curves
            .iter()
            .map(|(n, c)| (n.as_str(), c.value(t)))
            .collect()
    }
}

/// Sky texture pair and blend from `skyTime` [V 0x95FCE0]:
/// `f = fmod((skyTime + 10) * 6, 6)`, first texture `clamp(f, 0, 5)`, second
/// the next of six, blend `f - first`.
pub fn sky_textures(sky_time: f32) -> (usize, usize, f32) {
    let f = ((sky_time + 10.) * 6.).rem_euclid(6.);
    let i = (f as usize).min(5);
    (i, (i + 1) % 6, f - i as f32)
}

/// Fade value meaning "every light off" [V].
const OFF: f64 = -999_999.;

/// The original's headlight state: one global fade `F` (s) and a 3–9 s
/// delay per car slot [V 0x73B890, 0x727B20, 0x736020].
#[derive(Clone, Debug, PartialEq)]
pub struct Headlights {
    fade: f64,
    delays: Vec<f64>,
    forced: bool,
}

impl Headlights {
    /// `cars` slots with delays `3 + u8 * 6 / 255` from `draw` (the
    /// original's `rand`; AUTHORED source: the caller's generator). In
    /// [`Mode::ForcedNight`] the lights are on from the start.
    pub fn new(mode: Mode, cars: usize, mut draw: impl FnMut() -> u8) -> Self {
        let delays = (0..cars)
            .map(|_| 3. + f64::from(draw()) * 6. / 255.)
            .collect();
        let forced = mode == Mode::ForcedNight;
        Self {
            fade: if forced { 9. } else { 0. },
            delays,
            forced,
        }
    }

    /// One frame: `wanted` is `enableHeadlights` (> 0.5), `since_start` the
    /// seconds since the race (re)start and `dt` the frame time.
    pub fn step(&mut self, wanted: bool, since_start: f64, dt: f64) {
        let dt = dt.abs();
        if self.forced || wanted {
            if self.fade < 0. {
                self.fade = 0.;
            }
            self.fade += dt;
            if since_start < 0.25 || self.forced {
                self.fade = 9.;
            }
        } else {
            self.fade = self.fade.min(9.);
            if self.fade >= 0. && since_start >= 0.25 {
                self.fade -= dt;
            } else {
                self.fade = OFF;
            }
        }
    }

    /// Global headlight intensity 0..1.
    pub fn intensity(&self) -> f64 {
        self.fade.clamp(0., 1.)
    }

    /// Projector weight of car `i`: `None` when its lights are off, else
    /// `clamp((F - d_i) * 4, 0.001, 1)`.
    pub fn car(&self, i: usize) -> Option<f64> {
        let x = (self.fade - self.delays.get(i).copied().unwrap_or(0.)) * 4.;
        (x > 0.).then(|| x.clamp(0.001, 1.))
    }

    pub fn delays(&self) -> &[f64] {
        &self.delays
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_career_race_runs_a_day_from_three_in_the_afternoon() {
        let scale = clock_scale(720.);
        assert_eq!(scale, 120.);
        assert_eq!(clock_text(day_fraction(0., Some(scale))), "15:00:00");
        // 360 s of a 720 s race is half the day: 03:00.
        assert_eq!(clock_text(day_fraction(360., Some(scale))), "03:00:00");
        assert_eq!(clock_text(day_fraction(720., Some(scale))), "15:00:00");
        assert_eq!(clock_text(day_fraction(500., None)), "15:00:00");
        assert_eq!(clock_scale(24. * 3600.), 1.);
        assert!(is_day(day_fraction(0., None)));
        assert!(!is_day(day_fraction(360., Some(scale))));
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn le_mans_night_runs_from_dusk_to_dawn() {
        use crate::route::{DONINGTON, HARUNA, LE_MANS};
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let route = Route::new(LE_MANS).unwrap();
        assert_eq!(Mode::for_route(root, &route).unwrap(), Mode::Timeline);
        let classic = Route::new("tracks/europe/le_mans/route_1").unwrap();
        assert_eq!(Mode::for_route(root, &classic).unwrap(), Mode::None);
        let donington = Route::new(DONINGTON).unwrap();
        assert_eq!(Mode::for_route(root, &donington).unwrap(), Mode::None);
        assert!(Timeline::load(root, &donington).unwrap().is_none());
        let haruna = Route::new(HARUNA).unwrap();
        assert_eq!(Mode::for_route(root, &haruna).unwrap(), Mode::ForcedNight);
        let timeline = Timeline::load(root, &route).unwrap().unwrap();
        assert_eq!(timeline.curves.len(), 67);
        // Minute by minute over a day: when each flag changes.
        let changes = |name: &str| {
            let mut out = Vec::new();
            let mut last = timeline.flag(name, 0.).unwrap();
            for s in 1..=86_400u32 {
                let on = timeline.flag(name, f64::from(s) / DAY_SECONDS).unwrap();
                if on != last {
                    out.push((clock_text(f64::from(s) / DAY_SECONDS), on));
                    last = on;
                }
            }
            out
        };
        // enableHeadlights 0.992857 at 04:43 → 0.007143 at 05:40, 0 at 20:13
        // → 0.985714 at 21:18 (crossing 0.5 at 20:45:58).
        assert_eq!(
            changes("enableHeadlights"),
            [("05:11:30".into(), false), ("20:45:59".into(), true)]
        );
        assert_eq!(
            changes("enableShadowSystem"),
            [("05:11:27".into(), true), ("21:26:30".into(), false)]
        );
        // At the start (15:00) it is day, the sun high, the sky on the day slot.
        let t = day_fraction(0., None);
        assert_eq!(timeline.flag("enableheadlights", t), Some(false));
        assert_eq!(sky_textures(timeline.value("skyTime", t).unwrap()).0, 2);
        let rotation = timeline.value("rotationAngle", t).unwrap();
        assert!((0. ..360.).contains(&rotation), "{rotation}");
    }

    #[test]
    fn curves_wrap_midnight_and_interpolate_linearly() {
        // Keys at 06:00 (0) and 18:00 (12): the night half runs 12 → 0.
        let c = Curve::new(&[(360, 0.), (1080, 12.)], None);
        assert_eq!(c.value(0.5), 6.);
        assert_eq!(c.value(0.), 6.);
        assert_eq!(c.value(0.875), 9.);
        assert_eq!(c.value(0.125), 3.);
    }

    #[test]
    fn angles_unwrap_the_short_way_round() {
        // 350° at 00:00 to 10° at 12:00 passes through 360/0, not 180.
        let c = Curve::new(&[(0, 350.), (720, 10.)], Some(360.));
        assert!((c.value(0.125) - 355.).abs() < 1e-3);
        assert!((c.value(0.375) - 5.).abs() < 1e-3);
        let plain = Curve::new(&[(0, 350.), (720, 10.)], None);
        assert!((plain.value(0.25) - 180.).abs() < 1e-3);
    }

    #[test]
    fn sky_textures_blend_six_slots() {
        assert_eq!(sky_textures(0.), (0, 1, 0.));
        let (a, b, f) = sky_textures(0.25);
        assert_eq!((a, b), (1, 2));
        assert!((f - 0.5).abs() < 1e-4);
        assert_eq!(sky_textures(0.9).0, 5);
        assert_eq!(sky_textures(0.9).1, 0);
    }

    #[test]
    fn headlights_fade_in_per_car_and_switch_off_together() {
        let mut draws = [0u8, 255].into_iter();
        let mut h = Headlights::new(Mode::Timeline, 2, || draws.next().unwrap());
        assert_eq!(h.delays(), &[3., 9.]);
        h.step(true, 10., 3.1);
        assert!((h.intensity() - 1.).abs() < 1e-12);
        assert!((h.car(0).unwrap() - 0.4).abs() < 1e-9);
        assert_eq!(h.car(1), None);
        h.step(true, 10., 6.);
        assert!((h.car(1).unwrap() - 0.4).abs() < 1e-9);
        // Off: the fade is capped at 9 s and runs down; car 1 goes first.
        h.step(false, 20., 0.5);
        assert!(h.car(1).is_none());
        assert!(h.car(0).is_some());
        h.step(false, 20., 9.);
        assert!(h.car(0).is_none());
        h.step(false, 20., 0.1);
        assert_eq!(h.intensity(), 0.);
        // Right after a (re)start the lights snap on.
        h.step(true, 0.1, 0.016);
        assert_eq!(h.car(0), Some(1.));
        let forced = Headlights::new(Mode::ForcedNight, 1, || 0);
        assert!(forced.car(0).is_some());
    }
}
