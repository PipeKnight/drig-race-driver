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
/// The sixth circuit and first Japanese track: Yokohama Docks `yoko_v1`, the
/// 2.5 km street circuit of the Japan Street GT and Drift Battle events
/// (docs/YOKOHAMA.md).
pub const YOKOHAMA: &str = "tracks/japan/yokohama_docks/route_0";
/// The first point-to-point road: Haruna route 1 (`haruna_ill_mid_bot`,
/// illegal touge, mid to bottom, 5.86 km), run downhill (subroute 0) or
/// uphill (subroute 1) (docs/TOUGE.md).
pub const HARUNA: &str = "tracks/japan/haruna/route_1";
/// Every circuit the app, the soak and the cross-circuit tests cover.
pub const CIRCUITS: [&str; 6] = [
    DONINGTON,
    NURBURGRING,
    ISTANBUL_PARK,
    LONG_BEACH,
    JARAMA,
    YOKOHAMA,
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Route {
    path: String,
    split: usize,
    subroute: u8,
    ignore_progress: bool,
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
            subroute: 0,
            ignore_progress: false,
        })
    }
    /// The same folder for an event that ignores race progress: [V] for the
    /// `drift_freestyle` format the race setup (0x5F84E0, case 0x5F8705)
    /// sets byte +0x1D0 of the drift manager, which is drift settings
    /// +0x1C8 (`ignore_progress`; the manager 0x668720 builds its settings
    /// at +8). The Yokohama arenas' progress files are placeholders (2-3
    /// gates); the port then times no laps.
    pub fn ignoring_progress(mut self) -> Self {
        self.ignore_progress = true;
        self
    }
    pub fn ignores_progress(&self) -> bool {
        self.ignore_progress
    }
    /// The same folder run as progress route `subroute` of its
    /// `progress_track.xml` (0 or 1). Haruna's point-to-point routes ship a
    /// `forwards` route 0 and a `reverse` route 1; the original selects
    /// `subroute_0` for a race and `subroute_1` for a reverse run (0x65B2C0)
    /// [V]. Circuits have only subroute 0.
    pub fn with_subroute(mut self, subroute: u8) -> Result<Self, Box<dyn std::error::Error>> {
        if subroute > 1 {
            return Err("subroute must be 0 or 1".into());
        }
        self.subroute = subroute;
        Ok(self)
    }
    pub fn subroute(&self) -> u8 {
        self.subroute
    }
    /// The `ai_track.xml` / `ai_vehicle_track.xml` track of the subroute:
    /// `default` or `reverse` [I: the AI files name their two tracks so, and
    /// the reverse track's gates run the other way; the original's selection
    /// code is not traced].
    pub fn ai_track(&self) -> &'static str {
        if self.subroute == 1 { "reverse" } else { "default" }
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
    /// The track's `additional_ground_clearance` (database `track`, through
    /// its `track_model` rows by folder and file) [D]: 0.05 on the city
    /// tracks (Long Beach, ...), 0 elsewhere. The original stores it in a
    /// global (0x997280, called from 0x60BEF0) that lifts every car's collision
    /// floor (F406, `body_geometry::FloorLift`).
    pub fn additional_ground_clearance(
        &self,
        root: &std::path::Path,
    ) -> Result<f32, Box<dyn std::error::Error>> {
        let schema = std::fs::read(root.join("database/schema.bin"))?;
        let data = std::fs::read(root.join("database/database.bin"))?;
        let db = grid_formats::database::read(&schema, &data)?;
        let mut parts = self.path().split('/');
        let (Some("tracks"), Some(folder), Some(file)) = (parts.next(), parts.next(), parts.next())
        else {
            return Err(format!("route {} is not tracks/<region>/<track>/<route>", self.path()).into());
        };
        let models = db.table("track_model").ok_or("database track_model absent")?;
        let tracks = db.table("track").ok_or("database track absent")?;
        let track_id = models
            .rows()
            .find(|m| {
                m.text("folder_string").is_some_and(|f| f.eq_ignore_ascii_case(folder))
                    && m.text("file_string").is_some_and(|f| f.eq_ignore_ascii_case(file))
            })
            .and_then(|m| m.int("track_id"))
            .ok_or(format!("no database track model for {folder}/{file}"))?;
        tracks
            .rows()
            .find(|t| t.int("id") == Some(track_id))
            .and_then(|t| t.float("additional_ground_clearance"))
            .ok_or_else(|| format!("database track {track_id} absent").into())
    }

    pub fn is_default(&self) -> bool {
        self.path == DONINGTON && self.subroute == 0
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
    #[ignore = "requires GRID_GAME_ROOT"]
    fn additional_ground_clearance_comes_from_the_track_row() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = std::path::Path::new(&root);
        for (path, expected) in [
            (DONINGTON, 0.),
            (NURBURGRING, 0.),
            (ISTANBUL_PARK, 0.),
            (LONG_BEACH, 0.05),
            ("tracks/usa/long_beach/route_5", 0.05),
            (JARAMA, 0.),
            (YOKOHAMA, 0.05),
        ] {
            let route = Route::new(path).unwrap();
            assert_eq!(route.additional_ground_clearance(root).unwrap(), expected, "{path}");
        }
    }
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
    fn sixth_circuit_sources_load() {
        use crate::{
            ai_driver::AiData, dynamic_props::Catalog, entity_geometry::Props, lap_progress,
            source_vehicle::Runtime,
        };
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = std::path::Path::new(&root);
        let route = Route::new(YOKOHAMA).unwrap();
        assert_eq!(route.track_name(), "yokohama_docks");
        let progress = lap_progress::load_route(root, &route).unwrap();
        let splits: Vec<_> = progress.progress.routes[0]
            .splits
            .iter()
            .map(|s| (s.kind, s.gate))
            .collect();
        // 57 gates over 2528.6 m, finish gate 0, time splits at 21 and 35.
        use grid_formats::progress::SplitKind::{Finish, Time};
        assert_eq!(progress.progress.gates.len(), 57);
        assert!((progress.progress.total_distance - 2528.63).abs() < 0.01);
        assert_eq!(splits, [(Finish, 0), (Time, 21), (Time, 35)]);
        for slot in 0..12 {
            let mut car =
                Runtime::lacetti_on(root, &route, &format!("slot_{slot:02}"), 200679).unwrap();
            car.enable_lap_timing(root).unwrap();
            assert_eq!(car.lap_gate_count(), progress.progress.gates.len());
        }
        let data = AiData::load(root, route.path(), "touring", "lac").unwrap();
        assert!(
            (data.line.length - 2328.57).abs() < 1.,
            "{}",
            data.line.length
        );
        let props = Props::read_route(root, &route).unwrap();
        let catalog = Catalog::read(root, &route, &props).unwrap();
        let mut templates = std::collections::BTreeMap::<&str, usize>::new();
        for b in &catalog.breakables {
            *templates.entry(&b.template).or_default() += 1;
        }
        // Nothing breaks yet: the barrels stand on cylinders and the crates,
        // chevrons, armco and drift flags need joint rows (all static).
        assert_eq!(props.boxes.len(), 1503);
        assert!(templates.is_empty(), "{templates:?}");
        assert_eq!(catalog.unported.values().sum::<usize>(), 569);
        assert_eq!(catalog.unported["gg_barrel.max"], 77);
        assert_eq!(catalog.unported["close_drift_flag.max"], 21);
    }

    /// Haruna route 1 (`haruna_ill_mid_bot`), both directions: one run from
    /// the subroute's start to its finish over an open AI line, from its own
    /// two-car grid (docs/TOUGE.md).
    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn haruna_runs_both_ways_from_their_own_grids() {
        use crate::{ai_driver::Line, lap_progress, source_vehicle::Runtime};
        use grid_formats::progress::{SplitKind, TrackKind};
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = std::path::Path::new(&root);
        for (subroute, sectors) in [(0, [56, 119]), (1, [29, 92])] {
            let route = Route::new(HARUNA).unwrap().with_subroute(subroute).unwrap();
            let prepared = lap_progress::load_route(root, &route).unwrap();
            let p = &prepared.progress;
            assert_eq!(p.kind, TrackKind::PointToPoint);
            assert!(prepared.course.is_open());
            assert_eq!(p.gates.len(), 149, "{subroute}");
            assert!((p.total_distance - 5859.21).abs() < 0.01, "{}", p.total_distance);
            assert_eq!(p.gates[0].distance, 0.);
            let splits: Vec<_> = p.routes[0].splits.iter().map(|s| (s.kind, s.gate)).collect();
            assert_eq!(
                splits,
                [
                    (SplitKind::Start, 0),
                    (SplitKind::Time, sectors[0]),
                    (SplitKind::Time, sectors[1]),
                    (SplitKind::Finish, 148)
                ]
            );
            let line = Line::load_route(root, &route).unwrap();
            assert!(line.is_open());
            for slot in ["slot_00", "slot_01"] {
                let mut car = Runtime::lacetti_on(root, &route, slot, 200679).unwrap();
                car.enable_lap_timing(root).unwrap();
                assert_eq!(car.lap_gate_count(), 149);
            }
            // Touge grids hold two cars.
            assert!(Runtime::lacetti_on(root, &route, "slot_02", 200679).is_err());
        }
        // The forward AI track is `default`, the other `reverse`, and the
        // reverse run starts where the forward one ends.
        let forward = Route::new(HARUNA).unwrap();
        let reverse = forward.clone().with_subroute(1).unwrap();
        let ends = |r: &Route| {
            let p = lap_progress::load_route(root, r).unwrap().progress;
            let c = |g: &grid_formats::progress::Gate| [g.left[0] + g.right[0], g.left[2] + g.right[2]];
            (c(&p.gates[0]), c(p.gates.last().unwrap()))
        };
        let (a, b) = (ends(&forward), ends(&reverse));
        assert_eq!((a.0, a.1), (b.1, b.0));
        let (fl, rl) = (
            Line::load_route(root, &forward).unwrap(),
            Line::load_route(root, &reverse).unwrap(),
        );
        let near = |l: &Line, x: f32, z: f32| l.arc([x * 0.5, 0., z * 0.5]);
        // Along each line its own start comes before its finish.
        assert!(near(&fl, a.0[0], a.0[1]) < near(&fl, a.1[0], a.1[1]));
        assert!(near(&rl, b.0[0], b.0[1]) < near(&rl, b.1[0], b.1[1]));
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
            // matches the car row. Yokohama's SCRB has the grass row
            // (friction 0.4, dynamicScale 4.0, dynamicThreshold 5.81).
            let expected: &[&str] = if path == YOKOHAMA {
                &["GRAS", "SCRB"]
            } else {
                &["GRAS"]
            };
            assert_eq!(different, expected, "{}", route.track_name());
        }
    }
}
