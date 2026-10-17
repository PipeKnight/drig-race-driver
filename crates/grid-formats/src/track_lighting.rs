//! Saved distance/height fog controls; rendering formulas are separate.
use crate::{Error, Result, fail};
use quick_xml::{Reader, events::Event};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fog {
    pub base_height: f32,
    pub top_height: f32,
    pub distance: f32,
    pub density_cap: f32,
    pub colour: [f32; 3],
    pub brightness: f32,
}

pub fn read_fog(bytes: &[u8]) -> Result<Fog> {
    if bytes.len() > 1024 * 1024 {
        return fail(0, "lighting XML exceeds bound");
    }
    let mut reader = Reader::from_reader(bytes);
    let (mut opened, mut closed) = (false, false);
    let mut values = BTreeMap::new();
    loop {
        let event = reader.read_event().map_err(|_| Error {
            offset: reader.buffer_position() as usize,
            message: "invalid lighting XML",
        })?;
        match event {
            Event::Start(node) if node.name().as_ref() == b"lighting" && !opened => {
                opened = true;
            }
            Event::Empty(node) if opened && !closed && node.name().as_ref() == b"param" => {
                let mut count = 0;
                for attr in node.attributes() {
                    count += 1;
                    let attr = attr.map_err(|_| Error {
                        offset: 0,
                        message: "invalid lighting attribute",
                    })?;
                    let key = std::str::from_utf8(attr.key.as_ref())
                        .map_err(|_| Error {
                            offset: 0,
                            message: "invalid lighting key",
                        })?
                        .to_string();
                    let value = attr
                        .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                        .map_err(|_| Error {
                            offset: 0,
                            message: "invalid lighting value",
                        })?
                        .into_owned();
                    if values.insert(key, value).is_some() {
                        return fail(0, "duplicate lighting parameter");
                    }
                }
                if count != 1 || values.len() > 256 {
                    return fail(0, "lighting parameter shape/count invalid");
                }
            }
            Event::End(node) if opened && !closed && node.name().as_ref() == b"lighting" => {
                closed = true;
            }
            Event::Decl(_) | Event::Comment(_) => {}
            Event::Text(text) if text.as_ref().iter().all(u8::is_ascii_whitespace) => {}
            Event::Eof if opened && closed => break,
            _ => return fail(0, "unexpected lighting structure"),
        }
    }
    let scalar = |name: &str| -> Result<f32> {
        let value = values
            .get(name)
            .ok_or(Error {
                offset: 0,
                message: "fog parameter absent",
            })?
            .parse::<f32>()
            .map_err(|_| Error {
                offset: 0,
                message: "invalid fog scalar",
            })?;
        if !value.is_finite() {
            return fail(0, "nonfinite fog scalar");
        }
        Ok(value)
    };
    let rgb = values
        .get("fogColour")
        .ok_or(Error {
            offset: 0,
            message: "fog colour absent",
        })?
        .split_whitespace()
        .map(str::parse::<f32>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| Error {
            offset: 0,
            message: "invalid fog colour",
        })?;
    let colour: [f32; 3] = rgb.try_into().map_err(|_| Error {
        offset: 0,
        message: "fog colour must have three components",
    })?;
    let fog = Fog {
        base_height: scalar("fogBaseLevel")?,
        top_height: scalar("fogTopLevel")?,
        distance: scalar("fogDistance")?,
        density_cap: scalar("fogDensityCap")?,
        colour,
        brightness: scalar("fogColourBrightness")?,
    };
    if fog.top_height <= fog.base_height
        || !(0.01..=1_000_000.).contains(&fog.distance)
        || !(0.0..=1.).contains(&fog.density_cap)
        || !(0.0..=16.).contains(&fog.brightness)
        || fog
            .colour
            .iter()
            .any(|v| !v.is_finite() || !(0.0..=16.).contains(v))
    {
        return fail(0, "fog values outside supported range");
    }
    Ok(fog)
}

#[cfg(test)]
mod tests {
    use super::*;
    const XML: &str = r#"<lighting><param fogBaseLevel="0"/><param fogTopLevel="2000"/><param fogDistance="500"/><param fogDensityCap="0.4"/><param fogColour="0.6 0.7 0.9"/><param fogColourBrightness="0.5"/></lighting>"#;
    #[test]
    fn reads_controls_and_rejects_ambiguous_or_invalid_xml() {
        let f = read_fog(XML.as_bytes()).unwrap();
        assert_eq!(f.distance, 500.);
        assert_eq!(f.colour, [0.6, 0.7, 0.9]);
        for (from, to) in [
            ("500", "NaN"),
            ("500", "0"),
            ("0.4", "1.1"),
            ("2000", "-1"),
            ("0.6 0.7 0.9", "0.6 0.7"),
            ("0.6 0.7 0.9", "0.6 inf 0.9"),
            ("<lighting>", "<wrong>"),
            ("</lighting>", "<param fogDistance=\"2\"/></lighting>"),
        ] {
            assert!(
                read_fog(XML.replace(from, to).as_bytes()).is_err(),
                "{from}->{to}"
            );
        }
        assert!(
            read_fog(
                XML.replace("</lighting>", "</lighting><param fogDistance=\"3\"/>")
                    .as_bytes()
            )
            .is_err()
        );
        assert!(read_fog(b"<!DOCTYPE lighting><lighting/>").is_err());
        assert!(read_fog(&vec![b' '; 1024 * 1024 + 1]).is_err());
    }
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_donington_saved_fog_controls() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let bytes =
            std::fs::read(std::path::Path::new(&root).join("tracks/europe/donington/lighting.xml"))
                .unwrap();
        let fog = read_fog(&bytes).unwrap();
        assert_eq!(fog.distance, 500.);
        assert_eq!(fog.density_cap, 0.4);
        assert_eq!(fog.base_height, 0.);
        assert_eq!(fog.top_height, 2000.);
        assert_eq!(fog.brightness, 0.5);
    }
}
