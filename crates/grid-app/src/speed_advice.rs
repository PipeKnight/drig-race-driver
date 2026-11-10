//! The minimap's excessive-speed brake light (docs/RACE_HUD.md): the
//! route's recommended speeds from `reducedmechanics.jpk` and the light's
//! severity and flashing.
//!
//! Table [D: the "rm track" load 0x5F68E0, lookup 0x99B9F0, projection
//! 0x9C78D0 and 0x9C7550]: `track.rm` holds 17-byte nodes about 1 m apart
//! (centre as three big-endian 24-bit values, offset 0x800000, times
//! 0.005); `<class>.rm`, one per `ai_vehicle_type_track` class, holds a
//! big-endian float scale, 512 big-endian floats the light does not read,
//! then two bytes per node cell (full pace, slower pace). The recommended
//! speed at a distance is the full-pace byte, interpolated linearly
//! between cells, times the scale (m/s [I]). A route without the player's
//! class (Haruna has only `dummy.rm`) has no recommendation, and the light
//! stays at severity 0.
//!
//! Light [D: 0x6BA460, the `excessivespeedindicator` values of
//! `Osd/config.xml` parsed by 0x69C540]: the recommendation is read
//! `speedmodifier` × speed + `lookahead` metres ahead of the car;
//! severity = (speed − recommended) / `threshold`, clamped to 0..1. The
//! severity drives the `brake_light` animation [I: the original passes it
//! to the animation's slot 0x2C; the port seeks that fraction of it, as the
//! needle blur]. Up to `startflashSeverity` the light is on; above it, it
//! flashes with period `flashinterval`, off for the first
//! (1 − `lightoninterval`) of each period.
use bevy::prelude::*;

/// `Osd/config.xml` `excessivespeedindicator` [D values].
pub const THRESHOLD: f32 = 10.;
pub const LOOKAHEAD: f32 = 2.;
pub const SPEED_MODIFIER: f32 = 1.;
pub const FLASH_INTERVAL: f32 = 0.3;
pub const START_FLASH_SEVERITY: f32 = 0.5;
pub const LIGHT_ON_INTERVAL: f32 = 0.7;

/// Bytes per `track.rm` node.
const NODE: usize = 17;
/// Header of a class table: the scale and 512 floats.
const CLASS_HEADER: usize = 4 + 512 * 4;
/// Node spacing (m) [I: 0x9C7550 multiplies by a table field the load
/// sets; the node chains measure 1.000 m on average].
const SPACING: f32 = 1.;
/// A closing gap above this makes the route a point-to-point one with no
/// lap wrap [D 0x5F68E0].
const MAX_CLOSING_GAP: f32 = 100.;
/// Squared distance beyond which the local search counts as lost (60 m) [D].
const LOST: f32 = 3600.;
/// Node step of the coarse scan once lost [D]; the first fix scans every node.
const LOST_STEP: usize = 30;

pub struct Table {
    centres: Vec<Vec3>,
    /// Lap length (m), 0 for a point-to-point route.
    lap: f32,
    scale: f32,
    /// Full-pace byte per cell (nodes + 1 cells).
    speeds: Vec<u8>,
}

fn coordinate(b: &[u8]) -> f32 {
    let raw = (i32::from(b[0]) << 16) | (i32::from(b[1]) << 8) | i32::from(b[2]);
    (raw - 0x80_0000) as f32 * 0.005
}

impl Table {
    /// The table of `class` from the bytes of a route's `reducedmechanics.jpk`;
    /// `None` without `track.rm` or the class.
    pub fn parse(jpk: &[u8], class: &str) -> Result<Option<Self>, String> {
        let pack = grid_formats::jpk::inspect(jpk).map_err(|e| format!("reducedmechanics.jpk: {e:?}"))?;
        let entry = |name: &str| {
            pack.entries
                .iter()
                .find(|e| e.name.eq_ignore_ascii_case(name.as_bytes()))
                .map(|e| &jpk[e.stored_data.clone()])
        };
        let (Some(track), Some(cells)) = (entry("track.rm"), entry(&format!("{class}.rm"))) else {
            return Ok(None);
        };
        if track.len() < 2 * NODE || track.len() % NODE != 0 || cells.len() < CLASS_HEADER + 4 {
            return Err(format!("reducedmechanics.jpk: bad sizes {} / {}", track.len(), cells.len()));
        }
        let centres: Vec<Vec3> = track
            .as_chunks::<NODE>()
            .0
            .iter()
            .map(|n| Vec3::new(coordinate(&n[0..3]), coordinate(&n[3..6]), coordinate(&n[6..9])))
            .collect();
        let gap = centres[0].distance(centres[centres.len() - 1]);
        let lap = if gap > MAX_CLOSING_GAP { 0. } else { (centres.len() - 1) as f32 * SPACING + gap };
        let scale = f32::from_be_bytes([cells[0], cells[1], cells[2], cells[3]]);
        let speeds = cells[CLASS_HEADER..].as_chunks::<2>().0.iter().map(|c| c[0]).collect();
        Ok(Some(Self { centres, lap, scale, speeds }))
    }

    /// The table for the player's class on a route folder (`None` when the
    /// route or the class has none).
    pub fn load(root: &std::path::Path, route: &str, class: &str) -> Result<Option<Self>, String> {
        let path = root.join(route).join("reducedmechanics.jpk");
        match std::fs::read(&path) {
            Ok(bytes) => Self::parse(&bytes, class),
            Err(_) => Ok(None),
        }
    }

    fn wrap(&self, mut d: f32) -> f32 {
        if self.lap > 0. {
            d = d.rem_euclid(self.lap);
        }
        d
    }

    /// Distance along the chain and squared distance of `p` to segment `i`.
    fn segment(&self, i: usize, p: Vec3) -> (f32, f32) {
        let (a, b) = (self.centres[i], self.centres[i + 1]);
        let ab = b - a;
        let t = ((p - a).dot(ab) / ab.length_squared()).clamp(0., 1.);
        let t = if t.is_nan() { 0. } else { t };
        ((i as f32 + t) * SPACING, (p - (a + ab * t)).length_squared())
    }

    fn best(&self, nodes: impl Iterator<Item = usize>, p: Vec3) -> Option<(f32, f32)> {
        nodes
            .filter(|&i| i + 1 < self.centres.len())
            .map(|i| self.segment(i, p))
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// The car's distance along the chain [D 0x9C78D0]: from `previous` a
    /// search of the segments within 2|v|dt + 3 m, else (first fix, or
    /// farther than 60 m from the chain) a scan of every node, then of
    /// every 30th once a fix exists.
    pub fn locate(&self, previous: Option<f32>, p: Vec3, speed: f32, dt: f32) -> f32 {
        let n = self.centres.len();
        let mut found = None;
        if let Some(d) = previous {
            let reach = ((speed.abs() * dt * 2. + 3.) / SPACING) as i64 + 1;
            let at = (d / SPACING) as i64;
            let nodes = (at - reach..=at + reach).map(|i| i.rem_euclid(n as i64) as usize);
            found = self.best(nodes, p).filter(|b| b.1 <= LOST);
        }
        let (d, _) = found.unwrap_or_else(|| {
            let step = if previous.is_some() { LOST_STEP } else { 1 };
            self.best((0..n).step_by(step), p).unwrap_or((0., 0.))
        });
        self.wrap(d)
    }

    /// Recommended speed `ahead` metres past distance `d` [D 0x99B9F0].
    pub fn speed_at(&self, d: f32, ahead: f32) -> f32 {
        let d = self.wrap(d + ahead).max(0.);
        let cell = d / SPACING;
        let i = (cell as usize).min(self.speeds.len().saturating_sub(2));
        let (a, b) = (f32::from(self.speeds[i]), f32::from(self.speeds[i + 1]));
        (a + (b - a) * (cell - i as f32).clamp(0., 1.)) * self.scale
    }
}

/// Severity 0..1 of `speed` over `recommended` (both m/s) [D 0x6BA460].
pub fn severity(speed: f32, recommended: Option<f32>) -> f32 {
    match recommended {
        Some(r) if speed > r => ((speed - r) / THRESHOLD).min(1.),
        _ => 0.,
    }
}

/// The flash timer after `dt` and whether the light is on [D 0x6BA460].
pub fn flash(severity: f32, timer: &mut f32, dt: f32) -> bool {
    if severity <= START_FLASH_SEVERITY {
        return true;
    }
    *timer -= dt;
    if *timer < 0. {
        *timer = FLASH_INTERVAL;
    }
    *timer < LIGHT_ON_INTERVAL * FLASH_INTERVAL
}

/// The player's table and position on it.
#[derive(Resource, Default)]
pub struct SpeedAdvice {
    table: Option<Table>,
    distance: Option<f32>,
    /// The latest severity (the HUD reads it).
    pub severity: f32,
}

impl SpeedAdvice {
    pub fn new(table: Option<Table>) -> Self {
        Self { table, distance: None, severity: 0. }
    }
}

/// Each frame: locate the player, read the recommendation ahead and set
/// the severity.
pub fn update(driving: Res<crate::Driving>, advice: Option<ResMut<SpeedAdvice>>, time: Res<Time>) {
    let Some(mut advice) = advice else {
        return;
    };
    let advice = &mut *advice;
    let Some(table) = &advice.table else {
        advice.severity = 0.;
        return;
    };
    let pose = driving.runtime.pose();
    let v = driving.runtime.velocity();
    let speed = Vec3::new(v[0], v[1], v[2]).length();
    let d = table.locate(advice.distance, Vec3::new(pose[12], pose[13], pose[14]), speed, time.delta_secs());
    advice.distance = Some(d);
    let recommended = table.speed_at(d, SPEED_MODIFIER * speed + LOOKAHEAD);
    advice.severity = severity(speed, Some(recommended));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(p: [f32; 3]) -> Vec<u8> {
        let mut n = Vec::new();
        for c in p {
            let raw = (c / 0.005).round() as i32 + 0x80_0000;
            n.extend_from_slice(&raw.to_be_bytes()[1..]);
        }
        n.resize(NODE, 0);
        n
    }

    fn jpk(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut head = b"JPAK".to_vec();
        for v in [0u32, entries.len() as u32, 16, 0, 0, 0, 0] {
            head.extend_from_slice(&v.to_le_bytes());
        }
        let mut names = Vec::new();
        let mut data = Vec::new();
        let base = 32 + 32 * entries.len();
        let name_len: usize = entries.iter().map(|(n, _)| n.len() + 1).sum();
        for (n, d) in entries {
            let no = base + names.len();
            names.extend_from_slice(n.as_bytes());
            names.push(0);
            let doff = base + name_len + data.len();
            data.extend_from_slice(d);
            for v in [no, d.len(), doff, d.len(), 0, 0, 0, 0] {
                head.extend_from_slice(&(v as u32).to_le_bytes());
            }
        }
        head.extend(names);
        head.extend(data);
        head
    }

    /// A straight 11-node chain along x and a class table whose full-pace
    /// byte rises by 10 per cell, scale 0.5.
    fn table() -> Table {
        let track: Vec<u8> = (0..11).flat_map(|i| node([i as f32, 0., 0.])).collect();
        let mut class = 0.5f32.to_be_bytes().to_vec();
        class.resize(CLASS_HEADER, 0);
        for i in 0..12u8 {
            class.extend_from_slice(&[i * 10, 0]);
        }
        let bytes = jpk(&[("track.rm", track), ("touring.rm", class)]);
        assert!(Table::parse(&bytes, "gt1").unwrap().is_none());
        Table::parse(&bytes, "touring").unwrap().unwrap()
    }

    #[test]
    fn parse_reads_the_chain_and_the_full_pace_bytes() {
        let t = table();
        assert_eq!(t.centres.len(), 11);
        assert_eq!(t.centres[3], Vec3::new(3., 0., 0.));
        // Closing gap 10 m: a lap of 10 + 10 m.
        assert_eq!(t.lap, 20.);
        assert_eq!(t.speed_at(2.5, 0.), 12.5);
        assert_eq!(t.speed_at(1., 1.), 10.);
    }

    #[test]
    fn locate_projects_onto_the_nearest_segment() {
        let t = table();
        let first = t.locate(None, Vec3::new(4.25, 1., 0.), 0., 0.);
        assert!((first - 4.25).abs() < 1e-4, "{first}");
        let next = t.locate(Some(first), Vec3::new(5.5, -2., 0.), 20., 1. / 60.);
        assert!((next - 5.5).abs() < 1e-4, "{next}");
    }

    #[test]
    fn severity_and_flashing_follow_the_config() {
        assert_eq!(severity(30., None), 0.);
        assert_eq!(severity(30., Some(35.)), 0.);
        assert_eq!(severity(33., Some(30.)), 0.3);
        assert_eq!(severity(60., Some(30.)), 1.);
        let mut timer = 0.;
        assert!(flash(0.5, &mut timer, 0.1));
        // Above 0.5: the timer restarts at 0.3, off while at or above 0.21.
        assert!(!flash(0.8, &mut timer, 0.1));
        assert!(!flash(0.8, &mut timer, 0.05));
        assert!(flash(0.8, &mut timer, 0.05));
    }

    /// Every route with a class table: the AI racing line lies on the node
    /// chain (same world frame) and the recommendations are plausible speeds.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn route_tables_share_the_world_frame_of_the_ai_line() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"));
        let mut checked = 0;
        for region in std::fs::read_dir(root.join("tracks")).unwrap().flatten() {
            for track in std::fs::read_dir(region.path()).into_iter().flatten().flatten() {
                for route in std::fs::read_dir(track.path()).into_iter().flatten().flatten() {
                    let rel = route.path().strip_prefix(&root).unwrap().to_string_lossy().replace('\\', "/");
                    let Ok(Some(table)) = Table::load(&root, &rel, "touring") else {
                        continue;
                    };
                    let line = grid_tools::ai_driver::Line::load(&root, &rel).unwrap();
                    let mut d = None;
                    let mut worst = 0f32;
                    for p in line.points().iter().step_by(25) {
                        let p = Vec3::from(*p);
                        let at = table.locate(d, p, 0., 0.);
                        d = Some(at);
                        let (_, dist2) = table.best(0..table.centres.len(), p).unwrap();
                        worst = worst.max(dist2.sqrt());
                        let v = table.speed_at(at, 0.);
                        assert!((3. ..120.).contains(&v), "{rel}: {v} m/s at {at}");
                    }
                    assert!(worst < 20., "{rel}: AI line {worst} m off the chain");
                    checked += 1;
                }
            }
        }
        assert!(checked > 50, "{checked} routes");
    }
}
