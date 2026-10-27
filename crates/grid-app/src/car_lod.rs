//! Car level-of-detail selection as in the original (0x9F42A0,
//! docs/CAR_LOD.md "Selection"): each frame every car gets a distance band
//! from its FOV-scaled distance to the camera, cars are served nearest first,
//! and a level whose per-frame cap is used up pushes the car one level
//! coarser. No hysteresis. The bands and caps are `cars/carlodsettings.xml`'s
//! race set for the vehicle LOD quality (default 2, "high", 0x5F48B0).

//!
//! Common build; `GRID_CAR_LOD=0` keeps every car at LOD0 (comparisons).
use std::path::Path;
use std::sync::OnceLock;

/// Levels the settings file can name (`lod_cap_0..7`, `lod_distance_0..7`).
pub const LEVELS: usize = 8;
/// Vehicle LOD quality: the option's default, 2 = "high" (0x5F48B0).
const QUALITY: usize = 2;

static SETTING: OnceLock<Option<Setting>> = OnceLock::new();

/// Reads the race set from the game's `cars/carlodsettings.xml` once.
pub fn load(root: &Path) {
    let enabled = std::env::var("GRID_CAR_LOD").map_or(true, |v| v != "0");
    let setting = enabled
        .then(|| std::fs::read_to_string(root.join("cars/carlodsettings.xml")).ok())
        .flatten()
        .and_then(|text| Setting::parse(&text, race_set(QUALITY)));
    match &setting {
        Some(s) => println!(
            "car_lod version=1 set={} caps={:?} distances={:?}",
            race_set(QUALITY),
            s.caps,
            s.distances
        ),
        None => println!("car_lod version=1 enabled=false"),
    }
    let _ = SETTING.set(setting);
}

/// The race set in use, none when car LOD is off.
pub fn setting() -> Option<&'static Setting> {
    SETTING.get().and_then(Option::as_ref)
}
/// FOV of the reference distances: 55 degrees (0x9F42A0, tan(0.47996554)).
const REFERENCE_HALF_FOV: f32 = 0.479_965_54;
/// FOV-scaled distance of a car outside the view: it sorts last.
pub const CULLED_METRES: f32 = 10_000.;

/// One `lod_setting` set.
#[derive(Clone, Debug, PartialEq)]
pub struct Setting {
    pub caps: [i32; LEVELS],
    /// Upper bound of band N in FOV-scaled metres.
    pub distances: [f32; LEVELS],
}

impl Default for Setting {
    /// The constructor's values (0xA13994): caps 0, `distance[i] = (i+1)*5`.
    fn default() -> Self {
        Self {
            caps: [0; LEVELS],
            distances: std::array::from_fn(|i| (i + 1) as f32 * 5.),
        }
    }
}

impl Setting {
    /// The set `name` of a `carlodsettings.xml` text; entries the file leaves
    /// out keep the constructor's values.
    pub fn parse(text: &str, name: &str) -> Option<Self> {
        let open = format!("name=\"{name}\"");
        let start = text.find(&open)?;
        let block = &text[start..];
        let block = &block[..block.find("/>")?];
        let mut setting = Self::default();
        for i in 0..LEVELS {
            if let Some(v) = attribute(block, &format!("lod_cap_{i}")) {
                setting.caps[i] = v.parse().ok()?;
            }
            if let Some(v) = attribute(block, &format!("lod_distance_{i}")) {
                setting.distances[i] = v.parse().ok()?;
            }
        }
        Some(setting)
    }

    /// The band of FOV-scaled distance `m`: the first level whose distance
    /// is above it, else the last (0xA14FD0).
    pub fn band(&self, m: f32) -> usize {
        self.distances.iter().position(|&d| m < d).unwrap_or(LEVELS - 1)
    }
}

fn attribute<'a>(block: &'a str, key: &str) -> Option<&'a str> {
    let at = block.find(&format!("{key}=\""))? + key.len() + 2;
    let rest = &block[at..];
    Some(&rest[..rest.find('"')?])
}

/// The race set's name for vehicle LOD quality 0..3.
pub fn race_set(quality: usize) -> &'static str {
    ["low", "medium", "high", "ultra"][quality.min(3)]
}

/// FOV-scaled distance: `distance * tan(fov/2) / tan(27.5 deg)`, with the
/// camera's vertical FOV in radians [I: the original's FOV axis is unverified].
pub fn scaled(distance: f32, fov: f32) -> f32 {
    distance * (fov * 0.5).tan() / REFERENCE_HALF_FOV.tan()
}

/// A car to serve: its FOV-scaled distance, sort key (the distance with the
/// off-screen weight) and the finest and coarsest levels its model has.
#[derive(Clone, Copy, Debug)]
pub struct Request {
    pub metres: f32,
    pub key: f32,
    pub finest: usize,
    pub coarsest: usize,
}

/// Sort-key weight of a car with `outside` of its 8 box corners beyond the
/// side planes (race weighting, 0x9F42A0).
pub fn offscreen_weight(outside: usize) -> f32 {
    if outside >= 6 {
        2.5
    } else if outside >= 4 {
        1.5
    } else {
        1.
    }
}

/// The level of each car, nearest key first, under the set's caps.
pub fn select(setting: &Setting, cars: &[Request]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..cars.len()).collect();
    order.sort_by(|&a, &b| cars[a].key.total_cmp(&cars[b].key));
    let mut caps = setting.caps;
    let mut levels = vec![0; cars.len()];
    for i in order {
        let car = cars[i];
        let band = setting.band(car.metres);
        let mut lod = if band > car.coarsest {
            car.coarsest
        } else {
            band.max(car.finest)
        };
        while caps[lod] <= 0 && lod < car.coarsest {
            lod += 1;
        }
        caps[lod] -= 1;
        levels[i] = lod.min(car.coarsest);
    }
    levels
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = r#"<xml>
	<lod_setting name="low" lod_cap_0="1" lod_cap_1="1" lod_cap_2="1" lod_cap_3="1" lod_cap_4="6" lod_cap_5="99"
		lod_distance_0="-10.0" lod_distance_1="20.0" lod_distance_2="30.0" lod_distance_3="50.0" lod_distance_4="70.0" lod_distance_5="150.0" lod_distance_6="5000.0" />
	<lod_setting name="high"
		lod_cap_0="1"
		lod_cap_1="1"
		lod_cap_2="3"
		lod_cap_3="3"
		lod_cap_4="8"
		lod_cap_5="99"
		lod_distance_0="-10.0"
		lod_distance_1="20.0"
		lod_distance_2="30.0"
		lod_distance_3="50.0"
		lod_distance_4="70.0"
		lod_distance_5="150.0"
		lod_distance_6="5000.0"
	/>
</xml>"#;

    #[test]
    fn parses_a_set_and_keeps_constructor_values_for_missing_entries() {
        let high = Setting::parse(FILE, "high").unwrap();
        assert_eq!(high.caps, [1, 1, 3, 3, 8, 99, 0, 0]);
        assert_eq!(high.distances, [-10., 20., 30., 50., 70., 150., 5000., 40.]);
        assert!(Setting::parse(FILE, "ultra").is_none());
        assert_eq!(high.band(5.), 1);
        assert_eq!(high.band(20.), 2);
        assert_eq!(high.band(149.), 5);
        assert_eq!(high.band(6000.), 7);
    }

    #[test]
    fn nearest_cars_take_the_capped_levels_and_the_rest_go_coarser() {
        let high = Setting::parse(FILE, "high").unwrap();
        // Twelve low-file opponents (levels 2..5) from 4 m to 90 m.
        let cars: Vec<Request> = [4., 6., 9., 12., 15., 18., 25., 33., 45., 60., 80., 90.]
            .iter()
            .map(|&m| Request { metres: m, key: m, finest: 2, coarsest: 5 })
            .collect();
        let levels = select(&high, &cars);
        // Band 1 clamps to level 2: three cars there, three at 3, then 4.
        assert_eq!(levels, [2, 2, 2, 3, 3, 3, 4, 4, 4, 4, 5, 5]);
        // A car mostly off to the side sorts later and loses its level-2 slot.
        let mut side = cars.clone();
        side[0].key = 8. * offscreen_weight(6);
        assert_eq!(select(&high, &side)[0], 3);
    }

    #[test]
    fn fov_scaling_is_neutral_at_the_reference_fov() {
        assert!((scaled(30., 55f32.to_radians()) - 30.).abs() < 1e-3);
        assert!(scaled(30., 30f32.to_radians()) < 16.);
    }
}
