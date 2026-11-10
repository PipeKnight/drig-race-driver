//! Track street lights (`tracks/<region>/<track>/street_lights.xml`):
//! `streetlights` > `light name= position= direction= range= brightness=
//! maprangemultiple=`, read as the original's parser 0x732970 does. Unnamed
//! lights are the night-light sources; named ones go to a separate table of
//! 32 that nothing found here draws, so they are kept apart. How they light
//! the scene is separate (docs/NIGHT_LIGHTS.md).
use crate::{
    Result,
    binxml::{Document, Element},
    fail,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Light {
    /// `name=`; empty for the street lights the night pass draws.
    pub name: String,
    pub position: [f32; 3],
    /// Unit vector (the parser normalises a non-zero `direction=`).
    pub direction: [f32; 3],
    /// `range=`, default 8.
    pub range: f32,
    /// `brightness=` as the original stores it: `(i16)(b * 1024) / 1024`,
    /// default 1.
    pub brightness: f32,
    /// `maprangemultiple=` as stored (`* 1024` to i16), default 2.
    pub map_range_multiple: f32,
}

/// The original's street-light table holds 2048 unnamed lights.
pub const MAX_STREET_LIGHTS: usize = 2048;

fn attr<'a>(e: &Element<'a>, name: &str) -> Option<&'a str> {
    e.attributes
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value)
}

fn triple(text: Option<&str>) -> Result<[f32; 3]> {
    let mut out = [0.; 3];
    let mut parts = text.unwrap_or("").split_whitespace();
    for value in &mut out {
        *value = parts
            .next()
            .and_then(|p| p.parse().ok())
            .filter(|v: &f32| v.is_finite())
            .map_or(fail(0, "invalid street-light vector"), Ok)?;
    }
    Ok(out)
}

fn scalar(e: &Element<'_>, name: &str, default: f32) -> Result<f32> {
    match attr(e, name) {
        None => Ok(default),
        Some(text) => text
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|v| v.is_finite())
            .map_or(fail(0, "invalid street-light scalar"), Ok),
    }
}

/// The original's 16-bit fixed point (`(short)(int)(x * 1024)`).
fn fixed(x: f32) -> f32 {
    f32::from((x * 1024.) as i32 as i16) / 1024.
}

pub fn read(doc: &Document<'_>) -> Result<Vec<Light>> {
    match doc.elements.first() {
        Some(e) if e.name == "streetlights" => {}
        _ => return fail(0, "expected streetlights root"),
    }
    let mut lights = Vec::new();
    for i in doc.elements[0].children.clone() {
        let Some(e) = doc.elements.get(i) else {
            return fail(i, "street-light child out of range");
        };
        if e.name != "light" {
            continue;
        }
        let mut direction = triple(attr(e, "direction"))?;
        let length = direction.iter().map(|v| v * v).sum::<f32>().sqrt();
        if length > 0. {
            direction = direction.map(|v| v / length);
        }
        lights.push(Light {
            name: attr(e, "name").unwrap_or("").to_owned(),
            position: triple(attr(e, "position"))?,
            direction,
            range: scalar(e, "range", 8.)?,
            brightness: fixed(scalar(e, "brightness", 1.)?),
            map_range_multiple: fixed(scalar(e, "maprangemultiple", 2.)?),
        });
    }
    if lights.iter().filter(|l| l.name.is_empty()).count() > MAX_STREET_LIGHTS
        || lights.iter().filter(|l| !l.name.is_empty()).count() > 32
    {
        return fail(0, "more street lights than the original's tables hold");
    }
    Ok(lights)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_point_matches_the_original_store() {
        assert_eq!(fixed(5.), 5.);
        assert_eq!(fixed(0.8), 819. / 1024.);
        assert_eq!(fixed(2.), 2.);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_street_lights() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        for (track, count) in [("europe/le_mans", 162), ("japan/haruna", 4)] {
            let bytes = std::fs::read(root.join(format!("tracks/{track}/street_lights.xml"))).unwrap();
            let lights = read(&crate::binxml::read(&bytes).unwrap()).unwrap();
            assert_eq!(lights.len(), count, "{track}");
            for l in &lights {
                assert!(l.name.is_empty(), "{track}: named light");
                let n = l.direction.iter().map(|v| v * v).sum::<f32>();
                assert!((n - 1.).abs() < 1e-4, "{track}: {l:?}");
            }
            let ext = |f: fn(&Light) -> f32| lights.iter().map(f).fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v), b.max(v)));
            let (range, brightness) = (ext(|l| l.range), ext(|l| l.brightness));
            let upward = lights.iter().filter(|l| l.direction[1] >= 0.).count();
            eprintln!("{track}: range {range:?} brightness {brightness:?} upward {upward}");
            assert!(range.0 >= 5. && range.1 <= 60., "{track}: range {range:?}");
            assert!(brightness.0 >= 0.5 && brightness.1 <= 5., "{track}: brightness {brightness:?}");
        }
    }
}
