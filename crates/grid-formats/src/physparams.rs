//! Entity physics properties (`raceload.jpk#physparams.xml`, plain XML):
//! shape, rigid-body and behaviour property records referenced by
//! objecttypes.pssg as `physparams.xml#id`. Values stay attribute strings
//! until read through the typed accessors; see DYNAMIC_PROPS_RESEARCH.md.
use crate::{Error, Result, fail};
use quick_xml::{Reader, events::Event};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct Properties {
    pub shapes: BTreeMap<String, BTreeMap<String, String>>,
    pub bodies: BTreeMap<String, BTreeMap<String, String>>,
    pub behaviours: BTreeMap<String, BTreeMap<String, String>>,
    pub constraints: BTreeMap<String, BTreeMap<String, String>>,
}

/// Shape mass source: a density (`d700`, kg/m^3) or a fixed mass (`1`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MassSource {
    Density(f32),
    Mass(f32),
}

/// Rigid-body properties consumed by the original body descriptor (0x7B8F40).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub sphericalise: f32,
    pub spherical_radius: f32,
    pub spherical_mass: f32,
    pub max_angular_velocity: [f32; 3],
    pub max_linear_speed: f32,
    pub angular_damping: [f32; 3],
    pub linear_damping: f32,
    pub forward_air_resistance: [f32; 3],
    pub backward_air_resistance: [f32; 3],
    pub linear_stationary: f32,
    pub angular_stationary: f32,
    pub wake_up_force: f32,
    pub wake_up_torque: f32,
    pub turn_off_time: f32,
    pub gravity_scale: f32,
}

/// Original numeric token: `+inf`, `-inf`, a trailing `f` suffix (`10.0f`).
fn number(text: &str) -> Option<f32> {
    match text.trim() {
        "+inf" | "inf" => Some(f32::INFINITY),
        "-inf" => Some(f32::NEG_INFINITY),
        t => t.strip_suffix('f').unwrap_or(t).parse().ok(),
    }
}

/// `physparams.xml#id` or `#id` to `id`.
pub fn reference(uri: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(uri).ok()?;
    let id = text.rsplit_once('#')?.1;
    (!id.is_empty()).then_some(id)
}

impl Properties {
    pub fn read(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 4 << 20 {
            return fail(0, "physics properties exceed bound");
        }
        let mut reader = Reader::from_reader(bytes);
        let mut result = Self::default();
        let mut buffer = Vec::new();
        loop {
            let event = reader.read_event_into(&mut buffer).map_err(|_| Error {
                offset: reader.buffer_position() as usize,
                message: "invalid physics properties XML",
            })?;
            let element = match &event {
                Event::Start(e) | Event::Empty(e) => e,
                Event::Eof => break,
                _ => {
                    buffer.clear();
                    continue;
                }
            };
            let table = match element.name().as_ref() {
                b"TEMPLATESHAPEPROPS" => &mut result.shapes,
                b"TEMPLATERIGIDBODYPROPS" => &mut result.bodies,
                b"TEMPLATEBEHAVIOURPROPS" => &mut result.behaviours,
                b"TEMPLATECONSTRAINTPROPS" => &mut result.constraints,
                _ => {
                    buffer.clear();
                    continue;
                }
            };
            let mut attributes = BTreeMap::new();
            for a in element.attributes() {
                let a = a.map_err(|_| Error {
                    offset: reader.buffer_position() as usize,
                    message: "invalid physics property attribute",
                })?;
                let key = String::from_utf8(a.key.as_ref().to_vec()).map_err(|_| Error {
                    offset: reader.buffer_position() as usize,
                    message: "non-UTF-8 physics property name",
                })?;
                let value = String::from_utf8(a.value.to_vec()).map_err(|_| Error {
                    offset: reader.buffer_position() as usize,
                    message: "non-UTF-8 physics property value",
                })?;
                attributes.insert(key, value);
            }
            let id = attributes.get("id").cloned().ok_or(Error {
                offset: reader.buffer_position() as usize,
                message: "physics property without ID",
            })?;
            if table.insert(id, attributes).is_some() {
                return fail(
                    reader.buffer_position() as usize,
                    "duplicate physics property",
                );
            }
            buffer.clear();
        }
        if result.shapes.is_empty() || result.bodies.is_empty() || result.behaviours.is_empty() {
            return fail(0, "incomplete physics properties");
        }
        Ok(result)
    }

    /// Behaviour impulse threshold and explosion factor.
    pub fn behaviour(&self, id: &str) -> Result<(f32, f32)> {
        let b = self.behaviours.get(id).ok_or(Error {
            offset: 0,
            message: "unresolved behaviour properties",
        })?;
        let get = |k: &str| b.get(k).and_then(|v| number(v));
        match (get("impulse"), get("explosion")) {
            (Some(i), Some(e)) if i >= 0. && e.is_finite() => Ok((i, e)),
            _ => fail(0, "invalid behaviour properties"),
        }
    }

    /// Shape material key and mass source.
    pub fn shape(&self, id: &str) -> Result<([u8; 4], MassSource)> {
        let s = self.shapes.get(id).ok_or(Error {
            offset: 0,
            message: "unresolved shape properties",
        })?;
        let material: [u8; 4] = s
            .get("material")
            .and_then(|m| m.as_bytes().try_into().ok())
            .ok_or(Error {
                offset: 0,
                message: "invalid shape material",
            })?;
        let source = s.get("massSource").ok_or(Error {
            offset: 0,
            message: "shape without mass source",
        })?;
        let mass = match source.strip_prefix('d') {
            Some(d) => number(d).map(MassSource::Density),
            None => number(source).map(MassSource::Mass),
        };
        match mass {
            Some(MassSource::Density(v) | MassSource::Mass(v)) if v > 0. => {
                Ok((material, mass.unwrap()))
            }
            _ => fail(0, "invalid shape mass source"),
        }
    }

    pub fn body(&self, id: &str) -> Result<Body> {
        let b = self.bodies.get(id).ok_or(Error {
            offset: 0,
            message: "unresolved rigid-body properties",
        })?;
        let get = |k: &str| -> Result<f32> {
            b.get(k).and_then(|v| number(v)).ok_or(Error {
                offset: 0,
                message: "invalid rigid-body property",
            })
        };
        let vector = |k: &str| -> Result<[f32; 3]> {
            let v: Vec<f32> = b
                .get(k)
                .map(|v| v.split_whitespace().filter_map(number).collect())
                .unwrap_or_default();
            v.try_into().map_err(|_| Error {
                offset: 0,
                message: "invalid rigid-body vector",
            })
        };
        Ok(Body {
            sphericalise: get("sphericalise")?,
            spherical_radius: get("sphericalradius")?,
            spherical_mass: get("sphericalmass")?,
            max_angular_velocity: [get("maxAngVelX")?, get("maxAngVelY")?, get("maxAngVelZ")?],
            max_linear_speed: get("maxLinSpeed")?,
            angular_damping: vector("angDamping")?,
            linear_damping: get("linDamping")?,
            forward_air_resistance: vector("forwardAirRes")?,
            backward_air_resistance: vector("backwardAirRes")?,
            linear_stationary: get("linearStationary")?,
            angular_stationary: get("angularStationary")?,
            wake_up_force: get("wakeUpForce")?,
            wake_up_torque: get("wakeUpTorque")?,
            turn_off_time: get("turnOffTime")?,
            gravity_scale: get("gravityScale")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &[u8] = br#"<?xml version="1.0"?>
<PSSGFILE><PSSGDATABASE>
<TEMPLATESHAPEPROPS id="sA" material="PLAS" massSource="d70"/>
<TEMPLATESHAPEPROPS id="sB" material="SIGN" massSource="d+inf"/>
<TEMPLATESHAPEPROPS id="sC" material="PLAS" massSource="1"/>
<TEMPLATERIGIDBODYPROPS id="rb" sphericalise="0.0" sphericalradius="1.0" sphericalmass="1.0"
 maxAngVelX="+inf" maxAngVelY="+inf" maxAngVelZ="+inf" maxLinSpeed="+inf"
 angDamping="1.0 1.0 1.0" linDamping="1.0" forwardAirRes="0.0 0.0 0.0"
 backwardAirRes="0.0 0.0 0.0" linearStationary="0.3" angularStationary="0.3"
 wakeUpForce="0.0" wakeUpTorque="0.0" turnOffTime="1.0" gravityScale="1.0"/>
<TEMPLATEBEHAVIOURPROPS id="b" impulse="50" explosion="0.35"/>
<TEMPLATECONSTRAINTPROPS id="c" breakLinearImpulse="10.0f"/>
</PSSGDATABASE></PSSGFILE>"#;

    #[test]
    fn typed_properties_and_tokens() {
        let p = Properties::read(SAMPLE).unwrap();
        assert_eq!(p.behaviour("b").unwrap(), (50., 0.35));
        assert_eq!(p.shape("sA").unwrap(), (*b"PLAS", MassSource::Density(70.)));
        assert_eq!(p.shape("sB").unwrap().1, MassSource::Density(f32::INFINITY));
        assert_eq!(p.shape("sC").unwrap().1, MassSource::Mass(1.));
        let body = p.body("rb").unwrap();
        assert_eq!(body.max_linear_speed, f32::INFINITY);
        assert_eq!(body.angular_damping, [1.; 3]);
        assert_eq!(body.turn_off_time, 1.);
        assert_eq!(number(&p.constraints["c"]["breakLinearImpulse"]), Some(10.));
        assert_eq!(reference(b"physparams.xml#bRubber"), Some("bRubber"));
        assert!(p.behaviour("missing").is_err());
    }

    #[test]
    fn duplicate_and_incomplete_rejected() {
        assert!(Properties::read(b"<A/>").is_err());
        let duplicate = [
            SAMPLE,
            br#"<TEMPLATEBEHAVIOURPROPS id="b" impulse="1" explosion="0"/>"#,
        ]
        .concat();
        assert!(Properties::read(&duplicate).is_err());
    }
}
