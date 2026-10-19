//! A track's or route's `lighting.xml` as a whole (text XML on some tracks,
//! sectioned binary XML on others) and `effects/weather_fx.xml`: the data
//! behind each track's time of day, sky, fog and weather
//! (docs/TRACK_ENVIRONMENT.md). Rendering formulas are separate
//! (`track_lighting` holds the graphics side's typed fog/sun readers).
//!
//! [D] `<lighting>` holds `<param NAME="v [v v v]"/>` entries, one value per
//! element. The executable also reads an optional `<WEATHER rainfall=…>`
//! element (0x72AC90); no shipped lighting file has one.
use crate::{Error, Result, binxml, fail};
use quick_xml::{Reader, events::Event};
use std::collections::BTreeMap;

const LIMIT: usize = 1024 * 1024;

/// One XML element: name and attributes (text or binary form).
struct Element {
    name: String,
    depth: usize,
    attributes: Vec<(String, String)>,
}

fn elements(bytes: &[u8]) -> Result<Vec<Element>> {
    if bytes.len() > LIMIT {
        return fail(0, "environment XML exceeds bound");
    }
    let text = bytes.iter().find(|b| !b.is_ascii_whitespace() && **b != 0xEF && **b != 0xBB && **b != 0xBF);
    if text != Some(&b'<') {
        let doc = binxml::read(bytes)?;
        let depth = |mut i: usize| {
            let mut d = 0;
            while let Some(p) = doc.elements[i].parent {
                d += 1;
                i = p;
            }
            d
        };
        return Ok(doc
            .elements
            .iter()
            .enumerate()
            .map(|(i, e)| Element {
                name: e.name.to_owned(),
                depth: depth(i),
                attributes: e.attributes.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect(),
            })
            .collect());
    }
    let mut reader = Reader::from_reader(bytes);
    let mut out = Vec::new();
    let mut depth = 0usize;
    let invalid = |reader: &Reader<&[u8]>| Error {
        offset: reader.buffer_position() as usize,
        message: "invalid environment XML",
    };
    loop {
        let event = reader.read_event().map_err(|_| invalid(&reader))?;
        let (node, opens) = match &event {
            Event::Start(n) => (n, true),
            Event::Empty(n) => (n, false),
            Event::End(_) => {
                depth = depth.saturating_sub(1);
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let mut attributes = Vec::new();
        for attr in node.attributes() {
            let attr = attr.map_err(|_| invalid(&reader))?;
            let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
            let value = attr
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|_| invalid(&reader))?
                .into_owned();
            attributes.push((key, value));
        }
        out.push(Element {
            name: String::from_utf8_lossy(node.name().as_ref()).into_owned(),
            depth,
            attributes,
        });
        if opens {
            depth += 1;
        }
    }
    Ok(out)
}

/// A `lighting.xml`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Lighting {
    /// Every `param`, values as written.
    pub params: BTreeMap<String, String>,
    /// `<WEATHER rainfall>` when present.
    pub rainfall: Option<bool>,
}

fn floats(value: &str) -> Option<Vec<f32>> {
    value
        .split_whitespace()
        .map(|v| v.parse::<f32>().ok().filter(|f| f.is_finite()))
        .collect()
}

impl Lighting {
    pub fn values(&self, name: &str) -> Option<Vec<f32>> {
        floats(self.params.get(name)?)
    }
    pub fn scalar(&self, name: &str) -> Option<f32> {
        self.values(name)?.first().copied()
    }
    pub fn rgb(&self, name: &str) -> Option<[f32; 3]> {
        let v = self.values(name)?;
        (v.len() >= 3).then(|| [v[0], v[1], v[2]])
    }
    /// Sun angles in degrees (`heightangle`, `rotationangle`).
    pub fn sun(&self) -> Option<[f32; 2]> {
        Some([self.scalar("heightangle")?, self.scalar("rotationangle")?])
    }
    /// The night-time headlight flag (0 on every shipped file).
    pub fn headlights(&self) -> bool {
        self.scalar("headlights").is_some_and(|v| v != 0.)
    }
    /// The sky's rain layer scrolls (`sky_rain_scale_u` > 0).
    pub fn sky_rain(&self) -> bool {
        self.scalar("sky_rain_scale_u").is_some_and(|v| v > 0.)
    }
}

pub fn read_lighting(bytes: &[u8]) -> Result<Lighting> {
    let elements = elements(bytes)?;
    if elements.first().is_none_or(|e| e.name != "lighting") {
        return fail(0, "lighting XML root is not <lighting>");
    }
    let mut lighting = Lighting::default();
    for e in &elements[1..] {
        match (e.name.as_str(), e.depth) {
            ("param", 1) => {
                for (k, v) in &e.attributes {
                    lighting.params.insert(k.clone(), v.clone());
                }
            }
            ("WEATHER", 1) => {
                lighting.rainfall = e
                    .attributes
                    .iter()
                    .find(|(k, _)| k == "rainfall")
                    .map(|(_, v)| v.trim() != "0" && !v.trim().eq_ignore_ascii_case("false"));
            }
            _ => {}
        }
    }
    if lighting.params.is_empty() {
        return fail(0, "lighting XML has no params");
    }
    Ok(lighting)
}

/// `effects/weather_fx.xml`: the rain and windscreen effect settings.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeatherFx {
    pub enabled: bool,
    pub rainfall: bool,
    pub windscreen: bool,
    pub windscreen_drops: bool,
    pub wipers: bool,
    /// Every attribute by element (`rainfall`, `windscreen`).
    pub attributes: BTreeMap<String, BTreeMap<String, String>>,
}

pub fn read_weather_fx(bytes: &[u8]) -> Result<WeatherFx> {
    let elements = elements(bytes)?;
    let Some(root) = elements.first().filter(|e| e.name == "weather_fx_list") else {
        return fail(0, "weather fx root is not <weather_fx_list>");
    };
    let on = |attrs: &[(String, String)], name: &str| {
        attrs
            .iter()
            .find(|(k, _)| k == name)
            .and_then(|(_, v)| v.trim().parse::<f32>().ok())
            .is_some_and(|v| v != 0.)
    };
    let mut fx = WeatherFx {
        enabled: on(&root.attributes, "enable"),
        ..WeatherFx::default()
    };
    for e in &elements[1..] {
        if e.depth != 1 {
            continue;
        }
        match e.name.as_str() {
            "rainfall" => fx.rainfall = on(&e.attributes, "enable"),
            "windscreen" => {
                fx.windscreen = on(&e.attributes, "enable");
                fx.windscreen_drops = on(&e.attributes, "drop_enable");
                fx.wipers = on(&e.attributes, "wiper_enable");
            }
            _ => {}
        }
        fx.attributes
            .insert(e.name.clone(), e.attributes.iter().cloned().collect());
    }
    Ok(fx)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_lighting_and_weather() {
        let xml = br#"<?xml version="1.0"?>
<lighting>
  <param heightangle="144.9" />
  <param rotationangle="163.6" />
  <param fogColour="0.68 0.74 0.90" />
  <param headlights="0" />
  <WEATHER rainfall="1" />
</lighting>"#;
        let l = read_lighting(xml).unwrap();
        assert_eq!(l.sun(), Some([144.9, 163.6]));
        assert_eq!(l.rgb("fogColour"), Some([0.68, 0.74, 0.9]));
        assert!(!l.headlights());
        assert_eq!(l.rainfall, Some(true));
        assert!(read_lighting(b"<other><param a=\"1\"/></other>").is_err());
        assert!(read_lighting(b"<lighting></lighting>").is_err());
        let fx = read_weather_fx(
            br#"<weather_fx_list enable="1"><rainfall enable="0" fall_speed="1.50"/><windscreen enable="1" drop_enable="0" wiper_enable="0"/></weather_fx_list>"#,
        )
        .unwrap();
        assert!(fx.enabled && !fx.rainfall && fx.windscreen && !fx.windscreen_drops && !fx.wipers);
        assert_eq!(fx.attributes["rainfall"]["fall_speed"], "1.50");
    }

    #[test]
    #[ignore = "requires the purchased installation (GRID_GAME_ROOT)"]
    fn shipped_lighting_files() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"));
        // (file, binary, sun height, sky scale, fog distance, sky rain).
        for (file, binary, height, sky, fog, rain) in [
            ("europe/donington/lighting.xml", false, 144.9, 0.8, 500., false),
            ("europe/jarama/lighting.xml", true, 151.9, 1., 300., false),
            ("europe/milan/lighting.xml", true, 136.3, 0.7, 350., true),
            ("usa/long_beach/lighting.xml", false, 131.1, 0.8, 400., true),
            ("japan/haruna/lighting.xml", true, 158.2, 1., 250., false),
            ("japan/haruna/route_1/lighting.xml", true, 90.5, 0., 1000., false),
            ("japan/shibuya/lighting.xml", true, 93.6, 0.03, 100., false),
        ] {
            let bytes = std::fs::read(root.join("tracks").join(file)).unwrap();
            assert_eq!(bytes[0] != b'<', binary, "{file}");
            let l = read_lighting(&bytes).unwrap();
            assert_eq!(l.scalar("heightangle"), Some(height), "{file}");
            assert_eq!(l.scalar("skyscale"), Some(sky), "{file}");
            assert_eq!(l.scalar("fogDistance"), Some(fog), "{file}");
            assert_eq!(l.sky_rain(), rain, "{file}");
            assert!(!l.headlights(), "{file}");
            assert_eq!(l.rainfall, None, "{file}");
        }
        let fx = read_weather_fx(&std::fs::read(root.join("effects/weather_fx.xml")).unwrap()).unwrap();
        assert!(fx.enabled && !fx.rainfall && fx.windscreen && !fx.windscreen_drops && !fx.wipers);
    }
}
