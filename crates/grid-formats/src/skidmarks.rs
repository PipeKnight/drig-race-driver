//! Shipped `skidmarks/*_tm.xml` profiles: a root height offset and per-surface
//! tread, skid and side-skid layers (atlas U range, fade and width scale).
//! See docs/GROUND_EFFECTS_RESEARCH.md for the meaning of each attribute.
use crate::{Error, Result, fail};
use quick_xml::{Reader, events::Event};

/// One mark layer of a surface; `fade` and `normal_depth` are lower/upper bounds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Layer {
    pub enabled: bool,
    pub start_u: f32,
    pub end_u: f32,
    pub normal_depth: [f32; 2],
    pub fade: [f32; 2],
    pub width_scale: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Surface {
    /// Four-character surface code, e.g. `STDR` or `GRAS`.
    pub code: [u8; 4],
    pub tread: Layer,
    pub skid: Layer,
    pub side_skid: Layer,
    pub wet_lighting_scale: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    /// Root `offset` attribute (metres above the ground).
    pub offset: f32,
    pub surfaces: Vec<Surface>,
}

impl Profile {
    /// Exact code match, then the `DEFA` record, as a lookup convenience.
    pub fn surface(&self, code: [u8; 4]) -> Option<&Surface> {
        self.surfaces
            .iter()
            .find(|s| s.code == code)
            .or_else(|| self.surfaces.iter().find(|s| &s.code == b"DEFA"))
    }
}

const MAXIMUM_SURFACES: usize = 256;

fn number(value: &str, at: usize) -> Result<f32> {
    match value.trim().parse::<f32>() {
        Ok(v) if v.is_finite() => Ok(v),
        _ => fail(at, "invalid skidmark number"),
    }
}

pub fn read(text: &str) -> Result<Profile> {
    if text.len() > 1 << 20 {
        return fail(0, "skidmark profile exceeds bound");
    }
    let mut reader = Reader::from_str(text);
    let mut offset = None;
    let mut surfaces = Vec::new();
    loop {
        let at = reader.buffer_position() as usize;
        let event = reader.read_event().map_err(|_| Error {
            offset: at,
            message: "malformed skidmark XML",
        })?;
        let element = match &event {
            Event::Start(e) | Event::Empty(e) => e,
            Event::Eof => break,
            _ => continue,
        };
        let mut attributes = Vec::new();
        for attribute in element.attributes() {
            let attribute = attribute.map_err(|_| Error {
                offset: at,
                message: "malformed skidmark attribute",
            })?;
            // Shipped values are plain names and numbers; entities are rejected.
            let value = std::str::from_utf8(&attribute.value)
                .ok()
                .filter(|v| !v.contains('&'))
                .ok_or(Error {
                    offset: at,
                    message: "malformed skidmark attribute value",
                })?;
            attributes.push((attribute.key.as_ref().to_vec(), value.to_owned()));
        }
        let get = |name: &str| {
            attributes
                .iter()
                .find(|(k, _)| k == name.as_bytes())
                .map(|(_, v)| v.as_str())
        };
        match element.name().as_ref() {
            b"SKIDMARKS" => {
                if offset.is_some() {
                    return fail(at, "duplicate SKIDMARKS root");
                }
                offset = Some(number(get("offset").unwrap_or("0"), at)?);
            }
            b"SURFACE" => {
                if offset.is_none() {
                    return fail(at, "SURFACE outside SKIDMARKS");
                }
                if surfaces.len() == MAXIMUM_SURFACES {
                    return fail(at, "too many skidmark surfaces");
                }
                let name = get("name").ok_or(Error {
                    offset: at,
                    message: "unnamed skidmark surface",
                })?;
                let code: [u8; 4] = name.as_bytes().try_into().map_err(|_| Error {
                    offset: at,
                    message: "skidmark surface name is not four bytes",
                })?;
                let value = |key: &str, default: f32| -> Result<f32> {
                    get(key).map_or(Ok(default), |v| number(v, at))
                };
                let layer = |prefix: &str| -> Result<Layer> {
                    let key = |suffix: &str| {
                        let mut name = prefix.to_owned();
                        if suffix.is_empty() {
                            return name;
                        }
                        name.push_str(suffix);
                        name
                    };
                    Ok(Layer {
                        enabled: value(&key(""), 0.)? != 0.,
                        start_u: value(&key("StartU"), 0.)?,
                        end_u: value(&key("EndU"), 0.)?,
                        normal_depth: [
                            value(&key("NormalDepthLowerBound"), 0.)?,
                            value(&key("NormalDepthUpperBound"), 0.)?,
                        ],
                        fade: [
                            value(&key("FadeLowerBound"), 0.)?,
                            value(&key("FadeUpperBound"), 0.)?,
                        ],
                        width_scale: value(&key("WidthScale"), 1.)?,
                    })
                };
                surfaces.push(Surface {
                    code,
                    tread: layer("tread")?,
                    skid: layer("skid")?,
                    side_skid: layer("sideSkid")?,
                    wet_lighting_scale: value("wetLightingScale", 1.)?,
                });
            }
            _ => {}
        }
    }
    let Some(offset) = offset else {
        return fail(0, "missing SKIDMARKS root");
    };
    Ok(Profile { offset, surfaces })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_layers_defaults_and_fallback() {
        let profile = read(
            r#"<?xml version="1.0"?>
<SKIDMARKS offset="0.04">
<!-- comment -->
<SURFACE name="DEFA" tread="0" skid="0" />
<SURFACE name="GRAS" tread="1" treadStartU="0.5" treadEndU="0.5625"
  treadFadeLowerBound="0.25" treadFadeUpperBound="0.5"
  skid="1" skidStartU="0.6" skidEndU="0.7" skidWidthScale="2.0"
  sideSkid="0" sideSkidWidthScale="2.25" wetLightingScale="1.0"/>
</SKIDMARKS>"#,
        )
        .unwrap();
        assert_eq!(profile.offset, 0.04);
        assert_eq!(profile.surfaces.len(), 2);
        let grass = profile.surface(*b"GRAS").unwrap();
        assert!(grass.tread.enabled && grass.skid.enabled && !grass.side_skid.enabled);
        assert_eq!(grass.tread.fade, [0.25, 0.5]);
        assert_eq!(grass.skid.width_scale, 2.0);
        assert_eq!(grass.tread.width_scale, 1.0);
        assert_eq!(grass.side_skid.width_scale, 2.25);
        assert_eq!(&profile.surface(*b"STDR").unwrap().code, b"DEFA");
    }

    #[test]
    fn rejects_malformed_profiles() {
        assert!(read("<SURFACE name=\"GRAS\"/>").is_err());
        assert!(read("<SKIDMARKS offset=\"x\"/>").is_err());
        assert!(read("<SKIDMARKS><SURFACE name=\"GRASS\"/></SKIDMARKS>").is_err());
        assert!(read("<SKIDMARKS><SURFACE/></SKIDMARKS>").is_err());
        assert!(read("").is_err());
    }
}
