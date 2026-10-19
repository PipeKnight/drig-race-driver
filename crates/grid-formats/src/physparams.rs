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

/// `TEMPLATECONSTRAINTPROPS` as the original's props object holds it
/// (constructor 0x7B7FC0, reader 0x7D00F0; docs/JOINTS_RESEARCH.md).
#[derive(Clone, Debug, PartialEq)]
pub struct Constraint {
    /// `linearLimitAABBMin/Max`.
    pub linear_limit: [[f32; 3]; 2],
    /// `linearSpringAABBMin/Max`.
    pub linear_spring: [[f32; 3]; 2],
    pub spring_k: [f32; 3],
    pub spring_s: [f32; 3],
    /// `linearRotation`, `angularRotation0`, `angularRotation1` (x y z w).
    pub rotations: [[f32; 4]; 3],
    /// `rotationType`: 2 `cone`, 1 `plane`, 0 anything else (free).
    pub rotation_type: u32,
    /// Limit angle, spring angle, spring K, spring S, tangent spring S.
    pub rotation: [f32; 5],
    /// `twistType`: 1 `circle`, 0 anything else.
    pub twist_type: u32,
    /// Limit angle, spring angle, spring K, spring S.
    pub twist: [f32; 4],
    /// `+inf` reads as FLT_MAX (0x7D8600); absent is FLT_MAX.
    pub break_linear: f32,
    pub break_angular: f32,
    pub hard_friction: f32,
    pub soft_friction: f32,
    pub soft_impulse_factor: f32,
    pub break_at_angular_extent: bool,
    /// `uprootProps`: other constraint props after an uproot.
    pub uproot: Option<String>,
    pub dont_collide_with_any: bool,
    pub dont_collide_with_fixed: bool,
    pub specific_post_collision: bool,
    /// `bodyOverride` (rigid-body props id) and its priority (-1 absent).
    pub body_override: Option<String>,
    pub body_override_priority: i32,
}

/// 0x7D8600: `+inf`/`-inf` are FLT_MAX and -FLT_MAX, otherwise the number.
fn impulse(text: &str) -> Option<f32> {
    match text.trim() {
        "+inf" => Some(f32::MAX),
        "-inf" => Some(-f32::MAX),
        t => number(t).filter(|v| v.is_finite()),
    }
}

fn constraint_vector<const N: usize>(c: &BTreeMap<String, String>, k: &str) -> Result<[f32; N]> {
    let v: Vec<f32> = c
        .get(k)
        .map(|v| v.split_whitespace().filter_map(number).collect())
        .unwrap_or_default();
    v.try_into()
        .ok()
        .filter(|v: &[f32; N]| v.iter().all(|x| x.is_finite()))
        .ok_or(Error {
            offset: 0,
            message: "invalid constraint vector",
        })
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

    pub fn constraint(&self, id: &str) -> Result<Constraint> {
        let c = self.constraints.get(id).ok_or(Error {
            offset: 0,
            message: "unresolved constraint properties",
        })?;
        let invalid = || Error {
            offset: 0,
            message: "invalid constraint property",
        };
        let get = |k: &str| -> Result<f32> {
            c.get(k)
                .and_then(|v| number(v))
                .filter(|v| v.is_finite())
                .ok_or_else(invalid)
        };
        let optional = |k: &str| -> Result<f32> {
            if c.contains_key(k) { get(k) } else { Ok(0.) }
        };
        let flag = |k: &str| -> Result<bool> {
            match c.get(k).map(|v| v.trim()) {
                None | Some("0") => Ok(false),
                Some("1") => Ok(true),
                _ => Err(invalid()),
            }
        };
        let uri = |k: &str| -> Result<Option<String>> {
            match c.get(k) {
                None => Ok(None),
                Some(v) => reference(v.as_bytes())
                    .map(|r| Some(r.to_owned()))
                    .ok_or_else(invalid),
            }
        };
        let limit = |k: &str| -> Result<f32> {
            match c.get(k) {
                None => Ok(f32::MAX),
                Some(v) => impulse(v).ok_or_else(invalid),
            }
        };
        let kind = |k: &str| c.get(k).map(String::as_str);
        let vector3 = |k: &str| constraint_vector::<3>(c, k);
        let vector4 = |k: &str| constraint_vector::<4>(c, k);
        Ok(Constraint {
            linear_limit: [vector3("linearLimitAABBMin")?, vector3("linearLimitAABBMax")?],
            linear_spring: [vector3("linearSpringAABBMin")?, vector3("linearSpringAABBMax")?],
            spring_k: vector3("linearSpringK")?,
            spring_s: vector3("linearSpringS")?,
            rotations: [
                vector4("linearRotation")?,
                vector4("angularRotation0")?,
                vector4("angularRotation1")?,
            ],
            rotation_type: match kind("rotationType") {
                Some("cone") => 2,
                Some("plane") => 1,
                _ => 0,
            },
            rotation: [
                get("rotationLimitAngle")?,
                get("rotationSpringAngle")?,
                get("rotationSpringK")?,
                get("rotationSpringS")?,
                get("rotationTanSpringS")?,
            ],
            twist_type: u32::from(kind("twistType") == Some("circle")),
            twist: [
                get("twistLimitAngle")?,
                get("twistSpringAngle")?,
                get("twistSpringK")?,
                get("twistSpringS")?,
            ],
            break_linear: limit("breakLinearImpulse")?,
            break_angular: limit("breakAngularImpulse")?,
            hard_friction: optional("hardRotationalFriction")?,
            soft_friction: optional("softRotationalFriction")?,
            soft_impulse_factor: optional("softImpulseFactor")?,
            break_at_angular_extent: flag("breakIfAngExtentHit")?,
            uproot: uri("uprootProps")?,
            dont_collide_with_any: flag("dontCollideWithAny")?,
            dont_collide_with_fixed: flag("dontCollideWithFixedObjects")?,
            specific_post_collision: flag("specificPostCollision")?,
            body_override: uri("bodyOverride")?,
            body_override_priority: match c.get("bodyOverridePriority") {
                None => -1,
                Some(v) => v.trim().parse().map_err(|_| invalid())?,
            },
        })
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
<TEMPLATECONSTRAINTPROPS id="cWall" linearLimitAABBMin="0.0 0.0 -1.0"
 linearLimitAABBMax="0.0 0.0 0.0" linearSpringAABBMin="0.0 0.0 0"
 linearSpringAABBMax="0.0 0.0 0.0" linearSpringK="0.0 0.0 100000.0"
 linearSpringS="0.0 0.0 10000.0" linearRotation="0.0 0.0 0.0 1.0"
 angularRotation0="0.0 0.0 0.0 1.0" angularRotation1="0.0 0.0 0.0 1.0"
 rotationType="plane" rotationLimitAngle="0.1" rotationSpringAngle="0.0"
 rotationSpringK="0.0" rotationSpringS="0.0" rotationTanSpringS="0.0" twistType="circle"
 twistLimitAngle="0.1" twistSpringAngle="0.0" twistSpringK="0.0" twistSpringS="0.0"
 breakLinearImpulse="+inf" breakAngularImpulse="10.0f" dontCollideWithAny="1"
 bodyOverride="physparams.xml#_rbX" bodyOverridePriority="1"/>
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
        let wall = p.constraint("cWall").unwrap();
        assert_eq!(wall.linear_limit, [[0., 0., -1.], [0.; 3]]);
        assert_eq!(wall.rotations[2], [0., 0., 0., 1.]);
        assert_eq!((wall.rotation_type, wall.twist_type), (1, 1));
        assert_eq!((wall.break_linear, wall.break_angular), (f32::MAX, 10.));
        assert!(wall.dont_collide_with_any && !wall.dont_collide_with_fixed);
        assert_eq!(wall.body_override.as_deref(), Some("_rbX"));
        assert_eq!(wall.body_override_priority, 1);
        assert!(p.constraint("c").is_err());
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
