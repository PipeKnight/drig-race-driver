//! Purchased track route selection. A route folder such as
//! `tracks/europe/donington/route_0` holds the per-layout files (track.jpk,
//! grids.pssg, progress_track.xml, AI lines, boundary, ornaments, entities);
//! its parent track folder holds the shared scene (tracksplit.pssg,
//! objects.pssg, objecttypes.pssg, texture packages).

/// The prototype's original circuit and the default everywhere.
pub const DONINGTON: &str = "tracks/europe/donington/route_0";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    path: String,
    split: usize,
}

impl Route {
    /// A relative `tracks/<region>/<track>/<route>` folder of lower-case ASCII
    /// letters, digits and underscores.
    pub fn new(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() != 4
            || parts[0] != "tracks"
            || !parts.iter().all(|p| {
                !p.is_empty()
                    && p.len() <= 48
                    && p.bytes()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_')
            })
        {
            return Err("route must be tracks/<region>/<track>/<route>".into());
        }
        Ok(Self {
            path: path.into(),
            split: path.rfind('/').unwrap(),
        })
    }
    /// The route folder.
    pub fn path(&self) -> &str {
        &self.path
    }
    /// The track folder shared by the track's routes.
    pub fn track(&self) -> &str {
        &self.path[..self.split]
    }
    /// The track folder's last component, e.g. `jarama`.
    pub fn track_name(&self) -> &str {
        let track = self.track();
        &track[track.rfind('/').unwrap() + 1..]
    }
    pub fn file(&self, name: &str) -> String {
        format!("{}/{name}", self.path)
    }
    pub fn track_file(&self, name: &str) -> String {
        format!("{}/{name}", self.track())
    }
    pub fn is_default(&self) -> bool {
        self.path == DONINGTON
    }
}

impl Default for Route {
    fn default() -> Self {
        Self::new(DONINGTON).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn route_folders() {
        let r = Route::default();
        assert!(r.is_default());
        assert_eq!(r.track(), "tracks/europe/donington");
        assert_eq!(r.track_name(), "donington");
        assert_eq!(r.file("track.jpk"), "tracks/europe/donington/route_0/track.jpk");
        assert_eq!(
            r.track_file("objects.pssg"),
            "tracks/europe/donington/objects.pssg"
        );
        let j = Route::new("tracks/europe/jarama/route_0").unwrap();
        assert!(!j.is_default());
        assert_eq!(j.track_name(), "jarama");
        for bad in [
            "",
            "tracks/europe/jarama",
            "tracks/europe/jarama/route_0/",
            "/tracks/europe/jarama/route_0",
            "tracks/europe/../route_0",
            "tracks\\europe\\jarama\\route_0",
            "cars/europe/jarama/route_0",
            "tracks/Europe/jarama/route_0",
        ] {
            assert!(Route::new(bad).is_err(), "{bad}");
        }
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn second_circuit_sources_load() {
        use crate::{ai_driver::AiData, source_vehicle::Runtime};
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = std::path::Path::new(&root);
        let route = Route::new("tracks/europe/nurburgring/route_0").unwrap();
        let mut car = Runtime::lacetti_on(root, &route, "slot_11", 200679).unwrap();
        assert_eq!(car.route(), &route);
        car.enable_lap_timing(root).unwrap();
        assert!(car.lap_gate_count() > 0);
        let data = AiData::load(root, route.path(), "touring", "lac").unwrap();
        assert!((data.line.length - 5038.5).abs() < 1., "{}", data.line.length);
        // Jarama ships drift racing lines beside the optimal ones; only the
        // optimal line is read.
        let jarama = AiData::load(root, "tracks/europe/jarama/route_0", "touring", "lac").unwrap();
        assert!((jarama.line.length - 3830.7).abs() < 1., "{}", jarama.line.length);
    }
}
