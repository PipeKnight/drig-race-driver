//! A route's environment: time of day, sky, fog and weather as the original's
//! data defines them (docs/TRACK_ENVIRONMENT.md). Data and selection only;
//! the renderer turns it into light (graphics side).
//!
//! - Lighting preset: the route folder's `lighting.xml` when it has one
//!   (Haruna routes 1, 3, 6, 8 and 9: the night touge presets), else the
//!   track folder's [D shipped layout; I: the loader's search order was not
//!   traced, the same layout as `sky.pssg`].
//! - Flags from the route's `track_model` row [D]: `headlights_on`
//!   (forced night with headlights, the Haruna night routes),
//!   `supports_time_of_day` (the Le Mans day/night timeline, see
//!   `time_of_day`) and `vehicle_lights_on` (the cars' lights at race start
//!   when the track is not a headlights track, 0x596440).
//! - Weather: the original reads rain from an optional `<WEATHER rainfall>`
//!   element of the lighting file (0x72AC90) and has rain effects
//!   (`effects/weather_fx.xml`, `environment_rain` sound 0x5C7860,
//!   `rainEffectLevel` material constant 0x9D01E0), but no shipped lighting
//!   file has the element and `weather_fx.xml` disables rainfall, windscreen
//!   drops and wipers: every shipped race is dry. `sky_rain_*` only scroll
//!   the sky's cloud layer.
use crate::{route::Route, time_of_day};
use grid_formats::track_environment::{self, Lighting, WeatherFx};
use std::path::Path;

/// The flags of a route's `track_model` row.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TrackFlags {
    pub headlights_on: bool,
    pub supports_time_of_day: bool,
    pub vehicle_lights_on: bool,
}

impl TrackFlags {
    /// The lighting manager's mode from these flags (0x60C040, as
    /// `time_of_day::Mode::for_route`).
    pub fn mode(&self) -> time_of_day::Mode {
        if self.supports_time_of_day {
            time_of_day::Mode::Timeline
        } else if self.headlights_on {
            time_of_day::Mode::ForcedNight
        } else {
            time_of_day::Mode::None
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Environment {
    /// The lighting file used, relative to the game root.
    pub lighting_file: String,
    pub lighting: Lighting,
    pub flags: TrackFlags,
    pub weather_fx: WeatherFx,
}

impl Environment {
    /// The time-of-day mode: fixed lighting, the Le Mans timeline
    /// (`time_of_day::Timeline`) or forced night.
    pub fn mode(&self) -> time_of_day::Mode {
        self.flags.mode()
    }
    /// Forced night with headlights (the Haruna night routes). The Le Mans
    /// timeline decides its own night (`time_of_day`).
    pub fn night(&self) -> bool {
        self.mode() == time_of_day::Mode::ForcedNight
    }
    /// Rain falls: the lighting file's `WEATHER rainfall` and the enabled
    /// rainfall effect [I: both are needed for visible rain].
    pub fn raining(&self) -> bool {
        self.lighting.rainfall == Some(true) && self.weather_fx.enabled && self.weather_fx.rainfall
    }
    /// Car lights on at the start: `vehicle_lights_on` (the Japanese street
    /// tracks and Haruna) when the track is not a headlights track [D reads of
    /// both flags at 0x596440; I: the branch order from damaged pseudocode].
    pub fn vehicle_lights(&self) -> bool {
        self.flags.vehicle_lights_on && !self.flags.headlights_on
    }

    pub fn for_route(root: &Path, route: &Route) -> Result<Self, Box<dyn std::error::Error>> {
        let own = route.file("lighting.xml");
        let lighting_file = if root.join(&own).is_file() {
            own
        } else {
            route.track_file("lighting.xml")
        };
        let lighting = track_environment::read_lighting(&std::fs::read(root.join(&lighting_file))?)
            .map_err(|e| format!("{lighting_file}: {e}"))?;
        let weather_fx = track_environment::read_weather_fx(&std::fs::read(root.join("effects/weather_fx.xml"))?)
            .map_err(|e| format!("effects/weather_fx.xml: {e}"))?;
        Ok(Self {
            lighting_file,
            lighting,
            flags: flags(root, route)?,
            weather_fx,
        })
    }
}

/// The route's `track_model` row flags (folder, file and route strings).
pub fn flags(root: &Path, route: &Route) -> Result<TrackFlags, Box<dyn std::error::Error>> {
    let db = crate::read_database(root)?;
    let parts: Vec<&str> = route.path().split('/').collect();
    let [_, folder, file, layout] = parts[..] else {
        return Err(format!("route {} is not tracks/<region>/<track>/<route>", route.path()).into());
    };
    let row = db
        .table("track_model")
        .ok_or("database track_model absent")?
        .rows()
        .find(|m| {
            let eq = |col: &str, v: &str| m.text(col).is_some_and(|t| t.eq_ignore_ascii_case(v));
            eq("folder_string", folder) && eq("file_string", file) && eq("route_string", layout)
        })
        .ok_or(format!("no track model for {}", route.path()))?;
    let on = |col: &str| row.int(col) == Some(1);
    Ok(TrackFlags {
        headlights_on: on("headlights_on"),
        supports_time_of_day: on("supports_time_of_day"),
        vehicle_lights_on: on("vehicle_lights_on"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires the purchased installation (GRID_GAME_ROOT)"]
    fn shipped_route_environments() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"));
        // (route, own lighting file, night, time of day, sun height).
        for (path, own, night, timeline, height) in [
            ("tracks/europe/donington/route_0", false, false, false, 144.9),
            ("tracks/europe/le_mans/route_0", false, false, true, 139.),
            ("tracks/japan/haruna/route_0", false, false, false, 158.2),
            ("tracks/japan/haruna/route_1", true, true, false, 90.5),
            ("tracks/japan/haruna/route_9", true, true, false, 90.5),
            ("tracks/japan/shibuya/route_0", false, false, false, 93.6),
        ] {
            let route = Route::new(path).unwrap();
            let e = Environment::for_route(&root, &route).unwrap();
            assert_eq!(e.lighting_file.starts_with(path), own, "{path}");
            assert_eq!(e.night(), night, "{path}");
            assert_eq!(e.flags.supports_time_of_day, timeline, "{path}");
            assert_eq!(e.lighting.scalar("heightangle"), Some(height), "{path}");
            assert!(!e.raining(), "{path}");
            assert_eq!(e.mode(), time_of_day::Mode::for_route(&root, &route).unwrap(), "{path}");
        }
    }
}
