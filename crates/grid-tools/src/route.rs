//! Purchased track route selection. A route folder such as
//! `tracks/europe/donington/route_0` holds the per-layout files (track.jpk,
//! grids.pssg, progress_track.xml, AI lines, boundary, ornaments, entities);
//! its parent track folder holds the shared scene (tracksplit.pssg,
//! objects.pssg, objecttypes.pssg, texture packages).

/// The prototype's original circuit and the default everywhere.
pub const DONINGTON: &str = "tracks/europe/donington/route_0";
/// The second circuit (F304): Nürburgring GP.
pub const NURBURGRING: &str = "tracks/europe/nurburgring/route_0";
/// The third circuit: Istanbul Park GP (docs/ISTANBUL_PARK.md).
pub const ISTANBUL_PARK: &str = "tracks/europe/istanbul_park/route_0";
/// The fourth circuit: Long Beach street circuit (docs/LONG_BEACH.md).
pub const LONG_BEACH: &str = "tracks/usa/long_beach/route_0";
/// The fifth circuit: Jarama, the shipped route that runs the circuit the
/// other way round and finishes at gate 67 (docs/JARAMA.md).
pub const JARAMA: &str = "tracks/europe/jarama/route_1";
/// Every circuit the app, the soak and the cross-circuit tests cover.
pub const CIRCUITS: [&str; 5] = [DONINGTON, NURBURGRING, ISTANBUL_PARK, LONG_BEACH, JARAMA];

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
        assert_eq!(
            r.file("track.jpk"),
            "tracks/europe/donington/route_0/track.jpk"
        );
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
        let route = Route::new(NURBURGRING).unwrap();
        let mut car = Runtime::lacetti_on(root, &route, "slot_11", 200679).unwrap();
        assert_eq!(car.route(), &route);
        car.enable_lap_timing(root).unwrap();
        assert!(car.lap_gate_count() > 0);
        let data = AiData::load(root, route.path(), "touring", "lac").unwrap();
        assert!(
            (data.line.length - 5038.5).abs() < 1.,
            "{}",
            data.line.length
        );
        // Jarama ships drift racing lines beside the optimal ones; only the
        // optimal line is read.
        let jarama = AiData::load(root, "tracks/europe/jarama/route_0", "touring", "lac").unwrap();
        assert!(
            (jarama.line.length - 3830.7).abs() < 1.,
            "{}",
            jarama.line.length
        );
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn third_circuit_sources_load() {
        use crate::{
            ai_driver::AiData, dynamic_props::Catalog, entity_geometry::Props, lap_progress,
            source_vehicle::Runtime,
        };
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = std::path::Path::new(&root);
        let route = Route::new(ISTANBUL_PARK).unwrap();
        assert_eq!(route.track_name(), "istanbul_park");
        let progress = lap_progress::load_route(root, &route).unwrap();
        let splits: Vec<_> = progress.progress.routes[0]
            .splits
            .iter()
            .map(|s| (s.kind, s.gate))
            .collect();
        // 109 gates over 5336.8 m; finish at gate 0, sector splits at 33 and 74.
        use grid_formats::progress::SplitKind::{Finish, Time};
        assert_eq!(progress.progress.gates.len(), 109);
        assert!((progress.progress.total_distance - 5336.79).abs() < 0.01);
        assert_eq!(splits, [(Finish, 0), (Time, 33), (Time, 74)]);
        for slot in 0..12 {
            let mut car =
                Runtime::lacetti_on(root, &route, &format!("slot_{slot:02}"), 200679).unwrap();
            car.enable_lap_timing(root).unwrap();
            assert_eq!(car.lap_gate_count(), progress.progress.gates.len());
        }
        let data = AiData::load(root, route.path(), "touring", "lac").unwrap();
        assert!(
            (data.line.length - 5226.9).abs() < 1.,
            "{}",
            data.line.length
        );
        let props = Props::read_route(root, &route).unwrap();
        let catalog = Catalog::read(root, &route, &props).unwrap();
        let mut templates = std::collections::BTreeMap::<&str, usize>::new();
        for b in &catalog.breakables {
            *templates.entry(&b.template).or_default() += 1;
        }
        // Brake boards and double tyre walls break; armco and single tyre walls
        // need the joint rows the port lacks and stay static, as on Donington.
        assert_eq!(props.boxes.len(), 1689);
        assert_eq!(templates["tyrewall_black_double.max"], 222);
        assert_eq!(
            templates
                .keys()
                .filter(|t| t.starts_with("gg_brake_marker"))
                .count(),
            6
        );
        assert_eq!(
            catalog.unported.keys().collect::<Vec<_>>(),
            ["armco_main_section.max", "tyrewall_black.max"]
        );
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn fourth_circuit_sources_load() {
        use crate::{
            ai_driver::AiData, dynamic_props::Catalog, entity_geometry::Props, lap_progress,
            source_vehicle::Runtime,
        };
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = std::path::Path::new(&root);
        let route = Route::new(LONG_BEACH).unwrap();
        assert_eq!(route.track_name(), "long_beach");
        let progress = lap_progress::load_route(root, &route).unwrap();
        let splits: Vec<_> = progress.progress.routes[0]
            .splits
            .iter()
            .map(|s| (s.kind, s.gate))
            .collect();
        // 101 gates over 3655.8 m; finish at gate 0, sector splits at 33 and 69.
        use grid_formats::progress::SplitKind::{Finish, Time};
        assert_eq!(progress.progress.gates.len(), 101);
        assert!((progress.progress.total_distance - 3655.78).abs() < 0.01);
        assert_eq!(splits, [(Finish, 0), (Time, 33), (Time, 69)]);
        for slot in 0..12 {
            let mut car =
                Runtime::lacetti_on(root, &route, &format!("slot_{slot:02}"), 200679).unwrap();
            car.enable_lap_timing(root).unwrap();
            assert_eq!(car.lap_gate_count(), progress.progress.gates.len());
        }
        let data = AiData::load(root, route.path(), "touring", "lac").unwrap();
        assert!(
            (data.line.length - 3580.55).abs() < 1.,
            "{}",
            data.line.length
        );
        let props = Props::read_route(root, &route).unwrap();
        let catalog = Catalog::read(root, &route, &props).unwrap();
        let mut templates = std::collections::BTreeMap::<&str, usize>::new();
        for b in &catalog.breakables {
            *templates.entry(&b.template).or_default() += 1;
        }
        // Tyre stacks (single and double, three colours) break; the tyre walls
        // need the joint rows the port lacks and stay static, as on Donington.
        assert_eq!(props.boxes.len(), 1325);
        assert_eq!(templates.values().sum::<usize>(), 124);
        assert!(templates.keys().all(|t| t.starts_with("tyrestack_")));
        assert_eq!(catalog.unported.values().sum::<usize>(), 275);
        assert!(catalog.unported.keys().all(|t| t.starts_with("tyrewall_")));
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn fifth_circuit_sources_load() {
        use crate::{
            ai_driver::AiData, dynamic_props::Catalog, entity_geometry::Props, lap_progress,
            source_vehicle::Runtime,
        };
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = std::path::Path::new(&root);
        let route = Route::new(JARAMA).unwrap();
        assert_eq!(route.track_name(), "jarama");
        let progress = lap_progress::load_route(root, &route).unwrap();
        let splits: Vec<_> = progress.progress.routes[0]
            .splits
            .iter()
            .map(|s| (s.kind, s.gate))
            .collect();
        // 68 gates over 3903.3 m. The file finishes at gate 67 with splits at 23
        // and 45; re-indexed, the finish is gate 0 and the splits follow.
        use grid_formats::progress::SplitKind::{Finish, Time};
        assert_eq!(progress.progress.gates.len(), 68);
        assert!((progress.progress.total_distance - 3903.32).abs() < 0.01);
        assert_eq!(splits, [(Finish, 0), (Time, 24), (Time, 46)]);
        for slot in 0..12 {
            let mut car =
                Runtime::lacetti_on(root, &route, &format!("slot_{slot:02}"), 200679).unwrap();
            car.enable_lap_timing(root).unwrap();
            assert_eq!(car.lap_gate_count(), progress.progress.gates.len());
        }
        let data = AiData::load(root, route.path(), "touring", "lac").unwrap();
        assert!(
            (data.line.length - 3829.38).abs() < 1.,
            "{}",
            data.line.length
        );
        let props = Props::read_route(root, &route).unwrap();
        let catalog = Catalog::read(root, &route, &props).unwrap();
        let mut templates = std::collections::BTreeMap::<&str, usize>::new();
        for b in &catalog.breakables {
            *templates.entry(&b.template).or_default() += 1;
        }
        // Brake boards and tyre stacks break; armco and the drift flags need the
        // joint rows the port lacks and stay static, as on Donington.
        assert_eq!(props.boxes.len(), 2235);
        assert_eq!(templates.values().sum::<usize>(), 665);
        assert_eq!(
            templates
                .keys()
                .filter(|t| t.starts_with("gg_brake_marker"))
                .count(),
            8
        );
        assert!(templates
            .keys()
            .all(|t| t.starts_with("gg_brake_marker") || t.starts_with("tyrestack_")));
        assert_eq!(catalog.unported.values().sum::<usize>(), 576);
        assert!(catalog
            .unported
            .keys()
            .all(|t| t.starts_with("armco_") || *t == "close_drift_flag.max"));
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn circuit_barrier_keys_mix_to_the_car_row() {
        // The car body registers only the CAR_ solver row (source_vehicle.rs);
        // that is exact while every key it meets mixes to the same values.
        use grid_formats::{collision, cqtc, jpk};
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = std::path::Path::new(&root);
        let surfaces = crate::contact_surfaces::Surfaces::read(
            &std::fs::read(root.join("surface_materials.xml")).unwrap(),
        )
        .unwrap();
        let car = surfaces.physical(*b"CAR_").unwrap().1;
        for path in CIRCUITS {
            let route = Route::new(path).unwrap();
            let mut keys = std::collections::BTreeSet::new();
            let bytes = std::fs::read(root.join(route.file("track.jpk"))).unwrap();
            for entry in jpk::inspect(&bytes).unwrap().entries {
                keys.extend(
                    collision::read_vcqtc(&bytes[entry.stored_data])
                        .unwrap()
                        .materials,
                );
            }
            let boundary = std::fs::read(root.join(route.file("boundarylines.cqtc"))).unwrap();
            keys.extend(cqtc::read(&boundary).unwrap().materials);
            let different: Vec<_> = keys
                .iter()
                .filter(|k| surfaces.physical(**k).unwrap().1[1..] != car[1..])
                .map(|k| String::from_utf8_lossy(k).into_owned())
                .collect();
            // Only grass differs, on every circuit; Istanbul's extra HGDR
            // matches the car row.
            assert_eq!(different, ["GRAS"], "{}", route.track_name());
        }
    }
}
