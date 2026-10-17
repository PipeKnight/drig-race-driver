//! Shipped per-route `tracks/<area>/<track>/route_N/replay_camera_config.xml`
//! (sectioned binary XML), the original's replay director data
//! (`ReplayCameraConfiguration`, strings at 0x90EEA4/0x90EEC0 of the unpacked
//! image). Layout [D]:
//!
//! - `<ReplayCameraConfiguration route initialCamera>`;
//! - `<Camera type=zoom|static ident [sourcePath]>` with `<Parameter name type
//!   value|x y z [w]>` children (`Position`, `orientation` quaternion,
//!   `defaultFov`/`minimumFov`/`maximumFov` or `Fov`, `radius`, `targetOffset`,
//!   `zoomPerSecond`, `turnEnabled`, `lookahead`, `roll`, shake values …);
//! - `<ActivationZone type=box ident>` with `size`, `position`, `orientation`
//!   and `<EventHandler name=entry>` holding `<ActivateCamera ident>` entries.
//!   A zone usually names one `TERMINAL_` camera (terminal-damage replay) and
//!   one ordinary camera; `target.NAME` names a view of the target car's
//!   `cameras.xml` (`replay_wing1` …);
//! - `<Path ident type=spline local duration loop …>` with `<Point x y z>`
//!   children, and `<Path type=linearPercentage|bezierPercentage>` timing
//!   curves with `<Value time value>`.
//!
//! The camera behaviour behind the parameters is the app's (docs/REPLAY.md).
use crate::{Result, binxml, fail};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Scalar(f32),
    Bool(bool),
    Vector([f32; 3]),
    Quaternion([f32; 4]),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Zoom,
    Static,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Camera {
    pub kind: Kind,
    pub ident: String,
    /// `sourcePath`: the path the camera moves along.
    pub path: Option<String>,
    pub parameters: Vec<(String, Value)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    pub ident: String,
    /// Box size, metres (x, y, z of the box frame).
    pub size: [f32; 3],
    pub position: [f32; 3],
    /// x, y, z, w.
    pub orientation: [f32; 4],
    /// `ActivateCamera ident`s of the `entry` handler, in file order.
    pub entry: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub ident: String,
    pub kind: String,
    /// `duration` (spline paths).
    pub duration: Option<f32>,
    pub looped: bool,
    pub attributes: Vec<(String, String)>,
    /// `Point` children (spline paths).
    pub points: Vec<[f32; 3]>,
    /// `Value time value` children (percentage curves).
    pub values: Vec<[f32; 2]>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub route: String,
    pub initial: String,
    pub cameras: Vec<Camera>,
    pub zones: Vec<Zone>,
    pub paths: Vec<Path>,
}

fn get<'a>(parameters: &'a [(String, Value)], name: &str) -> Option<&'a Value> {
    parameters.iter().find(|(n, _)| n == name).map(|(_, v)| v)
}

impl Camera {
    pub fn get(&self, name: &str) -> Option<Value> {
        get(&self.parameters, name).copied()
    }
    pub fn scalar(&self, name: &str) -> Option<f32> {
        match self.get(name)? {
            Value::Scalar(v) => Some(v),
            _ => None,
        }
    }
    pub fn flag(&self, name: &str) -> Option<bool> {
        match self.get(name)? {
            Value::Bool(v) => Some(v),
            _ => None,
        }
    }
    pub fn vector(&self, name: &str) -> Option<[f32; 3]> {
        match self.get(name)? {
            Value::Vector(v) => Some(v),
            _ => None,
        }
    }
    pub fn quaternion(&self, name: &str) -> Option<[f32; 4]> {
        match self.get(name)? {
            Value::Quaternion(v) => Some(v),
            _ => None,
        }
    }
}

impl Config {
    pub fn camera(&self, ident: &str) -> Option<&Camera> {
        self.cameras.iter().find(|c| c.ident == ident)
    }
    pub fn path(&self, ident: &str) -> Option<&Path> {
        self.paths.iter().find(|p| p.ident == ident)
    }
}

fn number(text: Option<&str>) -> Result<f32> {
    text.unwrap_or_default()
        .trim()
        .trim_end_matches('f')
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
        .map_or_else(|| fail(0, "replay camera value is not a finite number"), Ok)
}

pub fn read(bytes: &[u8]) -> Result<Config> {
    let doc = binxml::read(bytes)?;
    let attr = |i: usize, name: &str| {
        doc.elements[i]
            .attributes
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| *v)
    };
    let Some(root) = doc
        .elements
        .iter()
        .position(|e| e.parent.is_none())
        .filter(|&i| doc.elements[i].name == "ReplayCameraConfiguration")
    else {
        return fail(0, "replay camera root is not ReplayCameraConfiguration");
    };
    let parameter = |p: usize| -> Result<(String, Value)> {
        let name = attr(p, "name").unwrap_or_default().to_owned();
        let v = |k| number(attr(p, k));
        let value = match attr(p, "type") {
            Some("scalar") => Value::Scalar(v("value")?),
            Some("bool") => Value::Bool(attr(p, "value") == Some("true")),
            Some("vector3") => Value::Vector([v("x")?, v("y")?, v("z")?]),
            Some("Quaternion") => Value::Quaternion([v("x")?, v("y")?, v("z")?, v("w")?]),
            _ => return fail(0, "unknown replay camera parameter type"),
        };
        Ok((name, value))
    };
    let mut config = Config {
        route: attr(root, "route").unwrap_or_default().to_owned(),
        initial: attr(root, "initialCamera").unwrap_or_default().to_owned(),
        cameras: Vec::new(),
        zones: Vec::new(),
        paths: Vec::new(),
    };
    for i in doc.elements[root].children.clone() {
        let e = &doc.elements[i];
        let ident = attr(i, "ident").unwrap_or_default().to_owned();
        match e.name {
            "Camera" => {
                let kind = match attr(i, "type") {
                    Some("zoom") => Kind::Zoom,
                    Some("static") => Kind::Static,
                    _ => return fail(0, "unknown replay camera type"),
                };
                let mut parameters = Vec::new();
                for p in e.children.clone() {
                    if doc.elements[p].name != "Parameter" {
                        return fail(0, "replay Camera child is not a Parameter");
                    }
                    parameters.push(parameter(p)?);
                }
                config.cameras.push(Camera {
                    kind,
                    ident,
                    path: attr(i, "sourcePath").map(str::to_owned),
                    parameters,
                });
            }
            "ActivationZone" => {
                if attr(i, "type") != Some("box") {
                    return fail(0, "replay activation zone is not a box");
                }
                let mut parameters = Vec::new();
                let mut entry = Vec::new();
                for c in e.children.clone() {
                    match doc.elements[c].name {
                        "Parameter" => parameters.push(parameter(c)?),
                        "EventHandler" => {
                            if attr(c, "name") != Some("entry") {
                                return fail(0, "unknown replay zone event");
                            }
                            for a in doc.elements[c].children.clone() {
                                if doc.elements[a].name != "ActivateCamera" {
                                    return fail(0, "replay zone action is not ActivateCamera");
                                }
                                entry.push(attr(a, "ident").unwrap_or_default().to_owned());
                            }
                        }
                        _ => return fail(0, "unknown replay zone child"),
                    }
                }
                let vector = |name| match get(&parameters, name) {
                    Some(Value::Vector(v)) => Ok(*v),
                    _ => fail(0, "replay zone lacks a vector"),
                };
                let orientation = match get(&parameters, "orientation") {
                    Some(Value::Quaternion(q)) => *q,
                    _ => return fail(0, "replay zone lacks its orientation"),
                };
                config.zones.push(Zone {
                    ident,
                    size: vector("size")?,
                    position: vector("position")?,
                    orientation,
                    entry,
                });
            }
            "Path" => {
                let mut points = Vec::new();
                let mut values = Vec::new();
                for c in e.children.clone() {
                    match doc.elements[c].name {
                        "Point" => points.push([
                            number(attr(c, "x"))?,
                            number(attr(c, "y"))?,
                            number(attr(c, "z"))?,
                        ]),
                        "Value" => values.push([number(attr(c, "time"))?, number(attr(c, "value"))?]),
                        _ => return fail(0, "unknown replay path child"),
                    }
                }
                config.paths.push(Path {
                    ident,
                    kind: attr(i, "type").unwrap_or_default().to_owned(),
                    duration: attr(i, "duration").map(|d| number(Some(d))).transpose()?,
                    looped: attr(i, "loop") == Some("true"),
                    attributes: e
                        .attributes
                        .iter()
                        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                        .collect(),
                    points,
                    values,
                });
            }
            _ => return fail(0, "unknown replay camera configuration element"),
        }
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped(track: &str) -> Config {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let path = std::path::Path::new(&root)
            .join("tracks")
            .join(track)
            .join("route_0/replay_camera_config.xml");
        read(&std::fs::read(path).unwrap()).unwrap()
    }

    #[test]
    #[ignore = "requires the purchased installation (GRID_GAME_ROOT)"]
    fn shipped_route_0_directors() {
        // (track, initial camera, cameras, zones, static cameras).
        for (track, initial, cameras, zones, statics) in [
            ("europe/donington", "Initial_Camera_R01", 76, 27, 2),
            ("europe/nurburgring", "initial_camera", 88, 63, 13),
            ("europe/istanbul_park", "Initial_Camera_R01", 86, 59, 15),
            ("usa/long_beach", "initial_camera_01", 86, 36, 7),
        ] {
            let config = shipped(track);
            assert_eq!(config.route, "0", "{track}");
            assert_eq!(config.initial, initial, "{track}");
            assert_eq!(config.cameras.len(), cameras, "{track}");
            assert_eq!(config.zones.len(), zones, "{track}");
            let n = config
                .cameras
                .iter()
                .filter(|c| c.kind == Kind::Static)
                .count();
            assert_eq!(n, statics, "{track}");
            assert!(config.camera(initial).is_some(), "{track}");
            for zone in &config.zones {
                assert!(!zone.entry.is_empty(), "{track} {}", zone.ident);
                for ident in &zone.entry {
                    assert!(
                        ident.starts_with("target.") || config.camera(ident).is_some(),
                        "{track} {ident}"
                    );
                }
            }
            for camera in &config.cameras {
                assert!(camera.vector("Position").is_some(), "{track}");
                if let Some(path) = &camera.path {
                    let path = config.path(path).unwrap();
                    assert_eq!(path.kind, "spline");
                    // Cubic Bézier chain: four points per segment.
                    assert!(path.points.len() >= 4 && path.points.len().is_multiple_of(4), "{track}");
                }
            }
        }
        // Donington's first terminal camera [D].
        let d = shipped("europe/donington");
        let c = d.camera("TERMINAL_Replay_Cam_R01_00").unwrap();
        assert_eq!(c.kind, Kind::Zoom);
        assert_eq!(c.vector("Position"), Some([-236.013_12, -10.7, 16.135_307]));
        assert_eq!(c.scalar("defaultFov"), Some(7.5));
        assert_eq!(c.scalar("radius"), Some(2.));
        assert_eq!(
            d.zones[0].entry,
            ["TERMINAL_Replay_Cam_R01_00", "Replay_Cam_R01_05"]
        );
    }
}
