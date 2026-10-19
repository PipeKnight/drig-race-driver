//! The route's environment data for the renderer (docs/TRACK_ENVIRONMENT.md):
//! the lighting preset, night and car-light flags and the (always dry) weather
//! of `grid_tools::environment`. Data only: no system here changes light,
//! sky or fog; the graphics side reads [`TrackEnvironment`].
use bevy::prelude::*;

/// The route's environment, inserted when a drive starts.
#[derive(Resource, Clone, Debug)]
#[allow(dead_code, reason = "the hand-off to the graphics side, which reads it")]
pub struct TrackEnvironment(pub grid_tools::environment::Environment);

pub fn install(app: &mut App, root: &std::path::Path, route: &grid_tools::route::Route) {
    match grid_tools::environment::Environment::for_route(root, route) {
        Ok(e) => {
            let l = &e.lighting;
            let n = |name: &str| l.scalar(name).map_or("-".to_owned(), |v| format!("{v}"));
            log_line!(
                "drive_environment version=1 lighting={} mode={:?} night={} vehicle_lights={} rain={} sun={},{} sky={},{} fog={},{} sky_rain={}",
                e.lighting_file,
                e.mode(),
                e.night(),
                e.vehicle_lights(),
                e.raining(),
                n("heightangle"),
                n("rotationangle"),
                n("skyscale"),
                n("skygamma"),
                n("fogDistance"),
                n("fogDensityCap"),
                l.sky_rain()
            );
            app.insert_resource(TrackEnvironment(e));
        }
        Err(error) => warn!("Track environment unavailable: {error}"),
    }
}
