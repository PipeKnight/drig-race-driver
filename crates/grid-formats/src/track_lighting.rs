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

fn parameters(bytes: &[u8]) -> Result<BTreeMap<String, String>> {
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
    Ok(values)
}

pub fn read_fog(bytes: &[u8]) -> Result<Fog> {
    let values = parameters(bytes)?;
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

/// Saved sky shader controls. Despite the uniform name InvGamma, the saved
/// gamma is uploaded directly as the shader exponent in the examined path.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sky {
    pub scale: f32,
    pub exponent: f32,
}

pub fn read_sky(bytes: &[u8]) -> Result<Sky> {
    let values = parameters(bytes)?;
    let scalar = |key: &str| -> Result<f32> {
        values
            .get(key)
            .and_then(|v| v.parse::<f32>().ok())
            .ok_or(Error {
                offset: 0,
                message: "missing or invalid sky scalar",
            })
    };
    let sky = Sky {
        scale: scalar("skyscale")?,
        exponent: scalar("skygamma")?,
    };
    if !sky.scale.is_finite()
        || !(0.0..=16.0).contains(&sky.scale)
        || !sky.exponent.is_finite()
        || !(0.01..=16.0).contains(&sky.exponent)
    {
        return fail(0, "sky controls outside supported range");
    }
    Ok(sky)
}

/// Saved diffuse-sun angles. The source shader uses the third basis vector of
/// Ry(-rotation) * Rx(-height) as its direction towards the light.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sun {
    pub height_degrees: f32,
    pub rotation_degrees: f32,
}

impl Sun {
    pub fn direction_to_light(self) -> [f32; 3] {
        let (h_sin, h_cos) = self.height_degrees.to_radians().sin_cos();
        let (r_sin, r_cos) = self.rotation_degrees.to_radians().sin_cos();
        [-r_sin * h_cos, h_sin, r_cos * h_cos]
    }
}

pub fn read_sun(bytes: &[u8]) -> Result<Sun> {
    let values = parameters(bytes)?;
    let angle = |name: &str| -> Result<f32> {
        let value = values
            .get(name)
            .ok_or(Error {
                offset: 0,
                message: "sun angle absent",
            })?
            .parse::<f32>()
            .map_err(|_| Error {
                offset: 0,
                message: "invalid sun angle",
            })?;
        if !value.is_finite() || !(-36_000.0..=36_000.0).contains(&value) {
            return fail(0, "sun angle outside supported range");
        }
        Ok(value)
    };
    Ok(Sun {
        height_degrees: angle("heightangle")?,
        rotation_degrees: angle("rotationangle")?,
    })
}

/// Preserve the saved HDR colour ratios while keeping its peak scale out of
/// the independently authored renderer's lux setting. No gamma conversion is
/// inferred from these values; the renderer must label its interpretation.
pub fn read_sun_colour(bytes: &[u8]) -> Result<[f32; 3]> {
    let values = parameters(bytes)?;
    let text = values.get("backlight_suncolour").ok_or(Error {
        offset: 0,
        message: "sun colour absent",
    })?;
    let components = text
        .split_whitespace()
        .map(str::parse::<f32>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| Error {
            offset: 0,
            message: "invalid sun colour",
        })?;
    let rgb: [f32; 3] = components.try_into().map_err(|_| Error {
        offset: 0,
        message: "sun colour needs three components",
    })?;
    if rgb
        .iter()
        .any(|v| !v.is_finite() || !(0.0..=32.0).contains(v))
    {
        return fail(0, "sun colour outside supported range");
    }
    let peak = rgb.into_iter().fold(1.0_f32, f32::max);
    Ok(rgb.map(|v| v / peak))
}

/// The particle light colours (`pfx_suncolour`, `pfx_ambient`,
/// `pfx_unlitambient`). XML completion 0x72C134..0x72C1A0 parses them into
/// lighting +0x130/+0x140/+0x150, which the uniform dispatcher (0x743418..
/// 0x743543) submits as `pfxSunColour`, `pfxPreMultipliedAmbientColour` and
/// `pfxUnlitAmbientColour`. Absent keys keep the constructor's (1, 1, 1)
/// (0x741386..0x7413AF). The completion also packs each colour to 8 bits by
/// 0x742B00; that rounding (under 0.4 %) is not reproduced.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleLight {
    pub sun: [f32; 3],
    pub ambient: [f32; 3],
    pub unlit_ambient: [f32; 3],
}

impl Default for ParticleLight {
    fn default() -> Self {
        Self {
            sun: [1.; 3],
            ambient: [1.; 3],
            unlit_ambient: [1.; 3],
        }
    }
}

pub fn read_particle_light(bytes: &[u8]) -> Result<ParticleLight> {
    let values = parameters(bytes)?;
    let colour = |name: &str| -> Result<[f32; 3]> {
        let Some(text) = values.get(name) else {
            return Ok([1.; 3]);
        };
        let components = text
            .split_whitespace()
            .map(str::parse::<f32>)
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|_| Error {
                offset: 0,
                message: "invalid particle light colour",
            })?;
        let rgb: [f32; 3] = components.try_into().map_err(|_| Error {
            offset: 0,
            message: "particle light colour needs three components",
        })?;
        if rgb.iter().any(|v| !v.is_finite() || !(0.0..=32.0).contains(v)) {
            return fail(0, "particle light colour outside supported range");
        }
        Ok(rgb)
    };
    Ok(ParticleLight {
        sun: colour("pfx_suncolour")?,
        ambient: colour("pfx_ambient")?,
        unlit_ambient: colour("pfx_unlitambient")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sun_angles_follow_source_basis_and_reject_bad_inputs() {
        for (height, rotation, expected) in [
            (0., 0., [0., 0., 1.]),
            (90., 0., [0., 1., 0.]),
            (0., 90., [-1., 0., 0.]),
            (0., -90., [1., 0., 0.]),
        ] {
            let xml = format!(
                r#"<lighting><param heightangle="{height}"/><param rotationangle="{rotation}"/></lighting>"#
            );
            let direction = read_sun(xml.as_bytes()).unwrap().direction_to_light();
            for i in 0..3 {
                assert!((direction[i] - expected[i]).abs() < 0.000001);
            }
        }
        let xml = r#"<lighting><param heightangle="20"/><param rotationangle="30"/></lighting>"#;
        for (from, to) in [
            ("20", "NaN"),
            ("30", "inf"),
            ("20", "36001"),
            ("30", "-36001"),
            ("heightangle", "missing"),
        ] {
            assert!(read_sun(xml.replace(from, to).as_bytes()).is_err());
        }
        assert!(
            read_sun(
                xml.replace("</lighting>", "<param heightangle=\"1\"/></lighting>")
                    .as_bytes()
            )
            .is_err()
        );
    }
    #[test]
    fn hdr_sun_colour_is_not_scaled_twice_and_invalid_rgb_is_rejected() {
        let xml = r#"<lighting><param backlight_suncolour="2 1 0.5"/><param backlight_sunscale="2"/></lighting>"#;
        assert_eq!(read_sun_colour(xml.as_bytes()).unwrap(), [1., 0.5, 0.25]);
        // A stored scalar is not an additional multiplier in this adaptation.
        assert_eq!(
            read_sun_colour(xml.replace("sunscale=\"2", "sunscale=\"3").as_bytes()).unwrap(),
            [1., 0.5, 0.25]
        );
        for value in ["1 2", "1 2 3 4", "NaN 1 1", "1 inf 1", "1 -1 1", "33 1 1"] {
            assert!(read_sun_colour(xml.replace("2 1 0.5", value).as_bytes()).is_err());
        }
        assert!(
            read_sun_colour(
                xml.replace(
                    "</lighting>",
                    "<param backlight_suncolour=\"1 1 1\"/></lighting>"
                )
                .as_bytes()
            )
            .is_err()
        );
    }
    #[test]
    fn particle_light_reads_saved_colours_and_defaults_absent_keys_to_one() {
        let xml = r#"<lighting><param pfx_suncolour="0.75 0.71 0.66"/><param pfx_ambient="1.06 1.08 1.13"/></lighting>"#;
        let light = read_particle_light(xml.as_bytes()).unwrap();
        assert_eq!(light.sun, [0.75, 0.71, 0.66]);
        assert_eq!(light.ambient, [1.06, 1.08, 1.13]);
        assert_eq!(light.unlit_ambient, [1.; 3]);
        assert_eq!(
            read_particle_light(b"<lighting></lighting>").unwrap(),
            ParticleLight::default()
        );
        for value in ["1 2", "NaN 1 1", "1 -1 1", "33 1 1"] {
            assert!(read_particle_light(xml.replace("0.75 0.71 0.66", value).as_bytes()).is_err());
        }
    }
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
    fn sky_exponent_is_saved_value_and_malformed_controls_are_rejected() {
        let xml = r#"<lighting><param skyscale="0.8"/><param skygamma="0.7"/></lighting>"#;
        assert_eq!(
            read_sky(xml.as_bytes()).unwrap(),
            Sky {
                scale: 0.8,
                exponent: 0.7
            }
        );
        for (from, to) in [
            ("0.8", "NaN"),
            ("0.8", "-1"),
            ("0.7", "inf"),
            ("0.7", "0"),
            ("0.7", "16.1"),
            ("skygamma", "missing"),
        ] {
            assert!(read_sky(xml.replace(from, to).as_bytes()).is_err());
        }
        assert!(
            read_sky(
                xml.replace("</lighting>", "<param skygamma=\"1\"/></lighting>")
                    .as_bytes()
            )
            .is_err()
        );
        assert!(read_sky(b"<!DOCTYPE lighting><lighting/>").is_err());
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
        assert_eq!(
            read_sky(&bytes).unwrap(),
            Sky {
                scale: 0.8,
                exponent: 0.7
            }
        );
    }
}
