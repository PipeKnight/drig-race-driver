//! Source physical materials and mechanics for contact, tyre and drag consumers.
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
    pub physical: Physical,
    /// Missing mechanics stays explicit here; contact consumers use
    /// [`Mechanics::initial`], as the original does.
    pub mechanics: Option<Mechanics<'a>>,
}

/// Source physical-contact coefficients, separate from tyre mechanics.
#[derive(Clone, Copy, Debug)]
pub struct Physical {
    pub restitution: f32,
    pub friction: f32,
    pub cfm: f32,
    pub correction_velocity: f32,
    pub minimum_bounce_velocity: f32,
    pub minimum_soft_k: f32,
    pub minimum_soft_s: f32,
    pub maximum_soft_k: f32,
    pub maximum_soft_s: f32,
    pub speed_for_maximum_softness: f32,
    pub dynamic_scale: f32,
    pub dynamic_threshold: f32,
    pub color: [u8; 4],
    pub sheet: bool,
}

impl Physical {
    /// Original row-material layout. The first word is an opaque packed key.
    pub fn row_words(&self, code: [u8; 4]) -> [u32; 13] {
        let values = [
            self.restitution,
            self.friction,
            self.cfm,
            self.correction_velocity,
            self.minimum_bounce_velocity,
            self.minimum_soft_k,
            self.minimum_soft_s,
            self.maximum_soft_k,
            self.maximum_soft_s,
            self.speed_for_maximum_softness,
            self.dynamic_scale,
            self.dynamic_threshold,
        ];
        std::array::from_fn(|i| {
            if i == 0 {
                u32::from_le_bytes(code)
            } else {
                values[i - 1].to_bits()
            }
        })
    }
}

fn color_component(element: &Element<'_>, key: &str, default: u8) -> Result<u8> {
    let Some(text) = attr(element, key) else {
        return Ok(default);
    };
    let text = text.trim_start_matches(|c: char| c.is_ascii_whitespace());
    let (negative, digits) = if let Some(s) = text.strip_prefix('-') {
        (true, s)
    } else {
        (false, text.strip_prefix('+').unwrap_or(text))
    };
    let (radix, digits) = if let Some(s) = digits
        .strip_prefix("0x")
        .or_else(|| digits.strip_prefix("0X"))
    {
        (16, s)
    } else if digits.len() > 1 && digits.starts_with('0') {
        (8, &digits[1..])
    } else {
        (10, digits)
    };
    let magnitude = i64::from_str_radix(digits, radix).map_err(|_| Error {
        offset: 0,
        message: "invalid physical material color",
    })?;
    let value = if negative { -magnitude } else { magnitude };
    if i32::try_from(value).is_err() {
        return fail(0, "physical material color outside signed range");
    }
    Ok(value.clamp(0, 255) as u8)
}

fn physical(element: &Element<'_>) -> Result<Physical> {
    Ok(Physical {
        restitution: number(element, "restitution", 0.)?,
        friction: number(element, "friction", f32::from_bits(0x3ecc_cccd))?,
        cfm: number(element, "cfm", f32::from_bits(0x3c23_d70a))?,
        correction_velocity: number(element, "correctionVel", 1.)?,
        minimum_bounce_velocity: number(element, "minBounceVel", 0.)?,
        minimum_soft_k: number(element, "minSoftK", 0.)?,
        minimum_soft_s: number(element, "minSoftS", 0.)?,
        maximum_soft_k: number(element, "maxSoftK", 0.)?,
        maximum_soft_s: number(element, "maxSoftS", 0.)?,
        speed_for_maximum_softness: number(element, "speedForMaxSoftness", 0.)?,
        dynamic_scale: number(element, "dynamicScale", 0.)?,
        dynamic_threshold: number(element, "dynamicThreshold", 10.)?,
        color: [
            color_component(element, "red", 255)?,
            color_component(element, "green", 255)?,
            color_component(element, "blue", 255)?,
            color_component(element, "alpha", 180)?,
        ],
        sheet: attr(element, "sheet").is_some_and(|s| s.eq_ignore_ascii_case("true")),
    })
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
    let d = Mechanics::initial();
    Ok(Mechanics {
        name,
        grip_per_ms: number(element, "grip_per_ms", d.grip_per_ms)?,
        ms_for_max_effect: number(element, "ms_for_max_effect", d.ms_for_max_effect)?,
        grip: number(element, "grip", d.grip)?,
        camera_factor: number(element, "camera_factor", d.camera_factor)?,
        roughness: number(element, "roughness", d.roughness)?,
        minimum_speed_for_slowdown: number(
            element,
            "minimumSpeedForSlowdown",
            d.minimum_speed_for_slowdown,
        )?,
        speed_for_maximum_slowdown: number(
            element,
            "speedForMaximumSlowdown",
            d.speed_for_maximum_slowdown,
        )?,
        slowdown: number(element, "slowdown", d.slowdown)?,
        roll_induce: number(element, "roll_induce", d.roll_induce)?,
        hardness: number(element, "hardness", d.hardness)?,
        heat: number(element, "heat", d.heat)?,
        wear: number(element, "wear", d.wear)?,
        curved: boolean("curved", d.curved)?,
        dirt_build_up: boolean("dirt_build_up", d.dirt_build_up)?,
        high_downforce: boolean("high_downforce", d.high_downforce)?,
        time_based: boolean("bumpstimebased", d.time_based)?,
        wavelength: number(element, "bumpswavelength", d.wavelength)?,
        magnitude: number(element, "bumpsmagnitude", d.magnitude)?,
        real_proportion: number(element, "bumpsrealproportion", d.real_proportion)?.clamp(0., 1.),
        car_proportion: number(element, "bumpsproportionCarDependent", d.car_proportion)?,
        depth: number(element, "depth", d.depth)?,
        tyre_table,
    })
}

impl Mechanics<'_> {
    /// The record of a material without MECHANICS: the material array is
    /// zeroed (0xB3B400 memset), then the loader 0x9B90F0 runs the
    /// initializer 0x99C8B0 on every material and reads attributes only when
    /// MECHANICS exists. The wheel contact producer 0x9C1660 reads this record
    /// with no presence test. The same values are the defaults of absent
    /// attributes. Name bytes +24 are "?????" (0x3F3F3F3F, 0x003F).
    pub fn initial() -> Self {
        Self {
            name: "?????",
            grip_per_ms: 0.,
            ms_for_max_effect: 1.,
            grip: 1.,
            camera_factor: 0.,
            roughness: 0.,
            minimum_speed_for_slowdown: 20.,
            speed_for_maximum_slowdown: 40.,
            slowdown: 0.,
            roll_induce: 0.,
            hardness: 1.,
            heat: 1.,
            wear: 1.,
            curved: false,
            dirt_build_up: false,
            high_downforce: false,
            time_based: true,
            wavelength: f32::from_bits(0x3d4c_cccd),
            magnitude: 0.,
            real_proportion: 0.,
            car_proportion: 0.,
            depth: 0.,
            tyre_table: *b"DEFA",
        }
    }
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
            physical: physical(e)?,
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
    fn a_material_without_mechanics_reads_the_initializer_record() {
        // 0x99C8B0 on zeroed memory: the values an empty MECHANICS gets.
        let empty = read(&document(vec![("name", "TEST")])).unwrap();
        let e = empty[0].mechanics.as_ref().unwrap();
        let i = Mechanics::initial();
        assert_eq!(i.name, "?????");
        assert_eq!(
            format!("{:?}", Mechanics { name: "?????", ..e.clone() }),
            format!("{i:?}")
        );
        assert!(i.time_based);
        assert_eq!(i.wavelength, f32::from_bits(0x3d4c_cccd));
        assert_eq!((i.magnitude, i.real_proportion, i.car_proportion, i.depth), (0., 0., 0., 0.));
        assert_eq!((i.grip, i.hardness, i.ms_for_max_effect), (1., 1., 1.));
        assert_eq!((i.minimum_speed_for_slowdown, i.speed_for_maximum_slowdown), (20., 40.));
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
