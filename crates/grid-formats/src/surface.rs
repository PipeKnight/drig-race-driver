//! Source mechanics fields for contacts and production tyre/drag consumers.
use crate::{
    Error, Result,
    binxml::{Document, Element},
    fail,
};
use std::collections::BTreeSet;

#[derive(Clone, Debug)]
pub struct Mechanics<'a> {
    pub name: &'a str,
    pub grip_per_ms: f32,
    pub ms_for_max_effect: f32,
    pub grip: f32,
    pub camera_factor: f32,
    pub roughness: f32,
    pub minimum_speed_for_slowdown: f32,
    pub speed_for_maximum_slowdown: f32,
    pub slowdown: f32,
    pub roll_induce: f32,
    pub hardness: f32,
    pub heat: f32,
    pub wear: f32,
    pub curved: bool,
    pub dirt_build_up: bool,
    pub high_downforce: bool,
    pub time_based: bool,
    pub wavelength: f32,
    pub magnitude: f32,
    pub real_proportion: f32,
    pub car_proportion: f32,
    pub depth: f32,
    pub tyre_table: [u8; 4],
}

#[derive(Clone, Debug)]
pub struct Material<'a> {
    pub code: [u8; 4],
    /// Missing mechanics stays explicit; no complete default object is inferred.
    pub mechanics: Option<Mechanics<'a>>,
}

/// Original packed-code table policy: first exact key, otherwise first record.
/// An inactive or empty supplied table has no result. No padding normalization.
pub fn packed_index(keys: &[[u8; 4]], code: [u8; 4], active: bool) -> Option<usize> {
    if !active || keys.is_empty() {
        None
    } else {
        Some(keys.iter().position(|key| *key == code).unwrap_or(0))
    }
}

fn attr<'a>(element: &Element<'a>, key: &str) -> Option<&'a str> {
    element
        .attributes
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, v)| *v)
}
fn number(element: &Element<'_>, key: &str, fallback: f32) -> Result<f32> {
    let Some(text) = attr(element, key) else {
        return Ok(fallback);
    };
    // Original conversion produces double before the destination float store.
    let value = text.parse::<f64>().map_err(|_| Error {
        offset: 0,
        message: "invalid surface mechanics number",
    })? as f32;
    if !value.is_finite() {
        return fail(0, "nonfinite surface mechanics number");
    }
    Ok(value)
}
fn mechanics<'a>(element: &Element<'a>) -> Result<Mechanics<'a>> {
    let name = attr(element, "name").ok_or(Error {
        offset: 0,
        message: "surface mechanics name absent",
    })?;
    if name.is_empty() || name.len() > 59 || !name.is_ascii() {
        return fail(0, "invalid surface mechanics name");
    }
    let boolean = |key, fallback| match attr(element, key) {
        None => Ok(fallback),
        Some(v) if v.eq_ignore_ascii_case("true") => Ok(true),
        Some(v) if v.eq_ignore_ascii_case("false") => Ok(false),
        // The original accessor ignores unrecognized text and retains defaults.
        // In particular, installed "0" is not parsed as a numeric boolean.
        _ => Ok(fallback),
    };
    let mut tyre_table = *b"DEFA";
    if let Some(text) = attr(element, "tyreTable") {
        if !text.is_ascii() {
            return fail(0, "invalid tyre table name");
        }
        if text.len() >= 4 {
            tyre_table.copy_from_slice(&text.as_bytes()[..4]);
        }
    }
    Ok(Mechanics {
        name,
        grip_per_ms: number(element, "grip_per_ms", 0.)?,
        ms_for_max_effect: number(element, "ms_for_max_effect", 1.)?,
        grip: number(element, "grip", 1.)?,
        camera_factor: number(element, "camera_factor", 0.)?,
        roughness: number(element, "roughness", 0.)?,
        minimum_speed_for_slowdown: number(element, "minimumSpeedForSlowdown", 20.)?,
        speed_for_maximum_slowdown: number(element, "speedForMaximumSlowdown", 40.)?,
        slowdown: number(element, "slowdown", 0.)?,
        roll_induce: number(element, "roll_induce", 0.)?,
        hardness: number(element, "hardness", 1.)?,
        heat: number(element, "heat", 1.)?,
        wear: number(element, "wear", 1.)?,
        curved: boolean("curved", false)?,
        dirt_build_up: boolean("dirt_build_up", false)?,
        high_downforce: boolean("high_downforce", false)?,
        time_based: boolean("bumpstimebased", true)?,
        wavelength: number(element, "bumpswavelength", f32::from_bits(0x3d4c_cccd))?,
        magnitude: number(element, "bumpsmagnitude", 0.)?,
        real_proportion: number(element, "bumpsrealproportion", 0.)?.clamp(0., 1.),
        car_proportion: number(element, "bumpsproportionCarDependent", 0.)?,
        depth: number(element, "depth", 0.)?,
        tyre_table,
    })
}

pub fn read<'a>(doc: &Document<'a>) -> Result<Vec<Material<'a>>> {
    let root = doc.elements.first().ok_or(Error {
        offset: 0,
        message: "surface root absent",
    })?;
    if root.name != "MATERIALS" || root.children.len() > 4096 {
        return fail(0, "invalid surface material root");
    }
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for index in root.children.clone() {
        let e = doc.elements.get(index).ok_or(Error {
            offset: index,
            message: "surface material element absent",
        })?;
        if e.name == "MUD" {
            continue;
        }
        if e.name != "MATERIAL" {
            return fail(index, "unexpected surface material child");
        }
        let name = attr(e, "name").ok_or(Error {
            offset: index,
            message: "surface material code absent",
        })?;
        if name.len() != 4 || !name.is_ascii() || !seen.insert(name) {
            return fail(index, "invalid or duplicate material code");
        }
        let code = name.as_bytes().try_into().unwrap();
        let mut found = None;
        for child in e.children.clone() {
            let m = doc.elements.get(child).ok_or(Error {
                offset: child,
                message: "surface child absent",
            })?;
            if m.name == "MECHANICS" {
                if found.is_some() || !m.children.is_empty() {
                    return fail(child, "duplicate or nested material mechanics");
                }
                found = Some(mechanics(m)?);
            }
        }
        result.push(Material {
            code,
            mechanics: found,
        });
    }
    if result.is_empty() {
        return fail(0, "no surface materials");
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packed_keys_keep_padding_and_first_record_fallback() {
        let keys = [*b"CAR_", *b"MUD ", *b"TEST", *b"TEST"];
        assert_eq!(packed_index(&keys, *b"MUD\0", true), Some(0));
        assert_eq!(packed_index(&keys, *b"MUD ", true), Some(1));
        assert_eq!(packed_index(&keys, *b"TEST", true), Some(2));
        assert_eq!(packed_index(&keys, *b"NONE", false), None);
        assert_eq!(packed_index(&[], *b"NONE", true), None);
    }
    fn document(attrs: Vec<(&'static str, &'static str)>) -> Document<'static> {
        Document {
            strings: vec![],
            string_padding: &[],
            elements: vec![
                Element {
                    name: "MATERIALS",
                    value: None,
                    attributes: vec![],
                    children: 1..2,
                    parent: None,
                },
                Element {
                    name: "MATERIAL",
                    value: None,
                    attributes: vec![("name", "TEST")],
                    children: 2..3,
                    parent: Some(0),
                },
                Element {
                    name: "MECHANICS",
                    value: None,
                    attributes: attrs,
                    children: 3..3,
                    parent: Some(1),
                },
            ],
        }
    }
    #[test]
    fn defaults_clamps_and_four_byte_tyre_names() {
        let doc = document(vec![
            ("name", "TEST"),
            ("bumpsrealproportion", "2"),
            ("tyreTable", "TARM_more"),
        ]);
        let result = read(&doc).unwrap();
        let m = result[0].mechanics.as_ref().unwrap();
        assert!(m.time_based);
        assert_eq!(m.wavelength, 0.05);
        assert_eq!(m.magnitude, 0.);
        assert_eq!(m.real_proportion, 1.);
        assert_eq!(m.car_proportion, 0.);
        assert_eq!(m.depth, 0.);
        assert_eq!(m.tyre_table, *b"TARM");
        assert_eq!((m.grip, m.hardness), (1., 1.));
    }
    #[test]
    fn missing_mechanics_and_invalid_values_stay_explicit() {
        let mut doc = document(vec![("name", "TEST")]);
        doc.elements[1].children = 2..2;
        assert!(read(&doc).unwrap()[0].mechanics.is_none());
        let bad = document(vec![("name", "TEST"), ("bumpsmagnitude", "NaN")]);
        assert!(read(&bad).is_err());
        for field in ["heat", "wear", "slowdown"] {
            assert!(read(&document(vec![("name", "TEST"), (field, "inf")])).is_err());
        }
        for value in ["0", "1", "yes"] {
            let doc = document(vec![
                ("name", "TEST"),
                ("dirt_build_up", value),
                ("bumpstimebased", value),
            ]);
            let material = read(&doc).unwrap();
            let m = material[0].mechanics.as_ref().unwrap();
            assert!(!m.dirt_build_up);
            assert!(m.time_based);
        }
        let lower = document(vec![
            ("name", "TEST"),
            ("bumpstimebased", "FALSE"),
            ("bumpsrealproportion", "-2"),
        ]);
        let result = read(&lower).unwrap();
        let m = result[0].mechanics.as_ref().unwrap();
        assert!(!m.time_based);
        assert_eq!(m.real_proportion, 0.);
    }
}
