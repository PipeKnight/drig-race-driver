//! Independently decoded NED strength/settings and explicit player LOD1 bounds.
//! These readers do not claim the original resource factory's selection policy.
use crate::{
    Result,
    binxml::{Document, Element},
    fail,
    pssg::Tree,
    scene,
};
pub const SETTINGS_WORDS: usize = 0x1054 / 4;
/// Additional reached wheel/view fields, separate from the proven1054 prefix.
#[derive(Clone, Copy, Debug)]
pub struct ConsumerSettings {
    pub light_cluster_threshold: f32,
    pub wheel_deformation_amount: f32,
    pub wheel_movement_level: f32,
}
pub fn consumer_settings(document: &Document<'_>) -> Result<ConsumerSettings> {
    if document.elements.first().is_none_or(|e| e.name != "DAMAGE") {
        return fail(0, "unsupported deformation consumer root");
    }
    let one = |name: &str| -> Result<Option<&Element<'_>>> {
        let elements: Vec<_> = document
            .elements
            .iter()
            .filter(|e| e.name == name)
            .collect();
        if elements.len() > 1 || elements.first().is_some_and(|e| e.parent != Some(0)) {
            return fail(0, "unsupported deformation consumer element");
        }
        Ok(elements.first().copied())
    };
    let fems = one("FEMS")?.ok_or(crate::Error {
        offset: 0,
        message: "missing deformation consumer FEMS",
    })?;
    let parts = one("PARTS")?;
    Ok(ConsumerSettings {
        light_cluster_threshold: parts
            .map(|e| scalar(e, "m_smashLightCluster", 0.1))
            .transpose()?
            .unwrap_or(0.1),
        wheel_deformation_amount: scalar(fems, "m_wheel_deformation_amount", 0.25)?,
        wheel_movement_level: scalar(fems, "m_wheel_movement_level", 0.8)?,
    })
}
pub struct Bounds {
    pub minimum: [f32; 4],
    pub maximum: [f32; 4],
    pub selected_joints: usize,
    pub excluded_aerials: usize,
}
fn attribute<'a>(e: &Element<'a>, name: &str) -> Result<Option<&'a str>> {
    let mut values = e.attributes.iter().filter(|(k, _)| *k == name);
    let out = values.next().map(|(_, v)| *v);
    if values.next().is_some() {
        return fail(0, "duplicate deformation attribute");
    }
    Ok(out)
}
fn number(text: &str) -> Result<f32> {
    let v = text.parse::<f64>().map_err(|_| crate::Error {
        offset: 0,
        message: "invalid deformation number",
    })?;
    if !v.is_finite() || v.abs() > 10000. {
        return fail(0, "unsupported deformation number");
    }
    Ok(v as f32)
}
fn scalar(e: &Element<'_>, name: &str, default: f32) -> Result<f32> {
    attribute(e, name)?
        .map(number)
        .transpose()
        .map(|v| v.unwrap_or(default))
}
fn array<const N: usize>(e: &Element<'_>, name: &str, default: [f32; N]) -> Result<[f32; N]> {
    let Some(text) = attribute(e, name)? else {
        return Ok(default);
    };
    let values: Vec<_> = text.split_whitespace().map(number).collect::<Result<_>>()?;
    values.try_into().map_err(|_| crate::Error {
        offset: 0,
        message: "invalid deformation vector length",
    })
}
fn put(b: &mut [u32], at: usize, v: f32) {
    b[at / 4] = v.to_bits();
}
fn value(b: &[u32], at: usize) -> f32 {
    f32::from_bits(b[at / 4])
}
/// Cold known fields; unconsumed records/padding start at zero by caller policy.
pub fn settings(document: &Document<'_>) -> Result<Vec<u32>> {
    if document.elements.first().is_none_or(|e| e.name != "DAMAGE") {
        return fail(0, "unsupported deformation root");
    }
    let choices: Vec<_> = document
        .elements
        .iter()
        .enumerate()
        .filter(|(_, e)| e.name == "FEMS")
        .collect();
    if choices.len() != 1 || choices[0].1.parent != Some(0) {
        return fail(0, "expected one direct FEMS element");
    }
    let (index, fems) = choices[0];
    let geometry: Vec<_> = document
        .elements
        .iter()
        .filter(|e| e.parent == Some(index) && e.name == "GEOMETRY")
        .filter_map(|e| match attribute(e, "id") {
            Ok(Some("strength_main")) => Some(Ok(e)),
            Ok(_) => None,
            Err(e) => Some(Err(e)),
        })
        .collect::<Result<_>>()?;
    if geometry.len() != 1 {
        return fail(0, "expected one strength_main geometry");
    }
    let boxes = document
        .elements
        .get(geometry[0].children.clone())
        .ok_or(crate::Error {
            offset: 0,
            message: "strength child range outside document",
        })?;
    if boxes.len() > 32 || boxes.iter().any(|e| e.name != "BOX") {
        return fail(0, "unsupported strength box count or type");
    }
    let mut result = vec![0; SETTINGS_WORDS];
    let identity: [f32; 16] =
        std::array::from_fn(|i| if [0, 5, 10, 15].contains(&i) { 1. } else { 0. });
    for (i, b) in boxes.iter().enumerate() {
        let matrix = array(b, "transform", identity)?;
        let size = array(b, "size", [0.1; 3])?;
        if [3, 7, 11].iter().any(|a| matrix[*a] != 0.)
            || matrix[15] != 1.
            || size.iter().any(|v| *v <= 0. || *v > 100.)
        {
            return fail(0, "unsupported strength box transform or size");
        }
        for (a, v) in matrix.into_iter().enumerate() {
            put(&mut result, i * 0x80 + a * 4, v);
        }
        for (a, v) in size.into_iter().enumerate() {
            put(&mut result, i * 0x80 + 0x40 + a * 4, v);
        }
        for (at, name, default) in [
            (0x50, "crush", 0.1),
            (0x54, "crY", 1.),
            (0x58, "crZ", 1.),
            (0x5c, "damage", 1.),
        ] {
            let v = scalar(b, name, default)?;
            if !(0. ..=100.).contains(&v) {
                return fail(0, "unsupported strength weight");
            }
            put(&mut result, i * 0x80 + at, v);
        }
    }
    result[0x1000 / 4] = boxes.len() as u32;
    for at in [0x1014, 0x1018, 0x1030, 0x103c, 0x1040] {
        put(&mut result, at, 1.);
    }
    for (at, v) in [
        (0x101c, 0.45),
        (0x1020, 0.45),
        (0x1028, 20.),
        (0x102c, 0.05),
        (0x1034, 0.95),
        (0x1038, 0.15),
        (0x1048, 0.2),
        (0x104c, 0.8),
    ] {
        put(&mut result, at, v);
    }
    for (at, name) in [
        (0x1018, "m_damage_multiple"),
        (0x1014, "max_compression_factor"),
        (0x1040, "m_wing_pushout_multiple"),
        (0x103c, "m_bonnet_uppush_multiple"),
        (0x1044, "m_bonnet_uppush_z"),
        (0x1048, "m_deformation_linear_threshold"),
        (0x104c, "m_deformation_multiple_slow"),
        (0x1050, "m_deformation_multiple_very_slow"),
        (0x101c, "m_wheel_burst_dist_f"),
        (0x1020, "m_wheel_burst_dist_r"),
        (0x102c, "m_normalMapDistance"),
        (0x102c, "m_normalMapDistanceFull"),
        (0x1024, "m_normalMapDistanceStart"),
        (0x1030, "m_scratchMapVelThreshold"),
        (0x1034, "m_scratchMapVelScale"),
        (0x1038, "m_scratchMapImpactScale"),
    ] {
        let old = value(&result, at);
        put(&mut result, at, scalar(fems, name, old)?);
    }
    if scalar(fems, "m_enableEditorExtraParameters", 0.)? < 0.5 {
        put(&mut result, 0x1014, 1.);
        put(&mut result, 0x1018, 1.);
    }
    let wing = value(&result, 0x1040) * 0.5;
    put(&mut result, 0x1040, wing);
    let span = value(&result, 0x102c) - value(&result, 0x1024);
    if span <= 0. {
        return fail(0, "invalid normal-map distance span");
    }
    put(&mut result, 0x1028, 1. / span);
    Ok(result)
}

/// Explicit investigated player policy: LOD1 joint bounds excluding aerials.
/// Aggregate transformed corners; no saved native positions or bounds constants.
pub fn player_lod1_bounds(tree: &Tree<'_>) -> Result<Bounds> {
    let mut selected = None;
    for i in 0..tree.nodes.len() {
        if tree.name(i) == b"MATRIXPALETTEBUNDLENODE"
            && tree.string(i, b"id")? == b"LOD1_"
            && selected.replace(i).is_some()
        {
            return fail(tree.nodes[i].offset, "ambiguous LOD1 bundle");
        }
    }
    let bundle = selected.ok_or(crate::Error {
        offset: 0,
        message: "missing LOD1 bundle",
    })?;
    let parent = tree.nodes[bundle].parent.ok_or(crate::Error {
        offset: 0,
        message: "missing bundle parent",
    })?;
    let identity: [f32; 16] =
        std::array::from_fn(|i| if [0, 5, 10, 15].contains(&i) { 1. } else { 0. });
    if scene::matrix(tree, bundle)? != identity || scene::matrix(tree, parent)? != identity {
        return fail(0, "unsupported LOD1 ancestor transforms");
    }
    let mut result = Bounds {
        minimum: [f32::MAX, f32::MAX, f32::MAX, 1.],
        maximum: [-f32::MAX, -f32::MAX, -f32::MAX, 1.],
        selected_joints: 0,
        excluded_aerials: 0,
    };
    for &i in &tree.nodes[bundle].children {
        if tree.name(i) != b"MATRIXPALETTEJOINTNODE" {
            continue;
        }
        if tree.string(i, b"id")?.windows(6).any(|v| v == b"aerial") {
            result.excluded_aerials += 1;
            continue;
        }
        result.selected_joints += 1;
        if result.selected_joints > 256 {
            return fail(0, "too many LOD1 joints");
        }
        let choices: Vec<_> = tree.nodes[i]
            .children
            .iter()
            .copied()
            .filter(|j| tree.name(*j) == b"BOUNDINGBOX")
            .collect();
        if choices.len() != 1 {
            return fail(tree.nodes[i].offset, "expected one joint bounding box");
        }
        let raw = tree.nodes[choices[0]].data.ok_or(crate::Error {
            offset: 0,
            message: "missing joint bound payload",
        })?;
        if raw.len() != 24 {
            return fail(0, "invalid joint bound payload");
        }
        let q: [f32; 6] =
            std::array::from_fn(|a| f32::from_be_bytes(raw[a * 4..a * 4 + 4].try_into().unwrap()));
        if q.iter().any(|v| !v.is_finite() || v.abs() > 1000.) || (0..3).any(|a| q[a] > q[a + 3]) {
            return fail(0, "invalid joint bound extent");
        }
        let m = scene::matrix(tree, i)?;
        for corner in 0..8 {
            let xyz: [f32; 3] = std::array::from_fn(|a| q[a + 3 * ((corner >> (2 - a)) & 1)]);
            for a in 0..3 {
                let p = ((xyz[0] * m[a] + xyz[1] * m[4 + a]) + xyz[2] * m[8 + a]) + m[12 + a];
                if !p.is_finite() {
                    return fail(0, "nonfinite transformed joint bound");
                }
                if p < result.minimum[a] {
                    result.minimum[a] = p;
                }
                if p > result.maximum[a] {
                    result.maximum[a] = p;
                }
            }
        }
    }
    if result.selected_joints == 0 {
        return fail(0, "empty selected LOD1 joints");
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn document() -> Document<'static> {
        Document {
            strings: vec![],
            string_padding: &[],
            elements: vec![
                Element {
                    name: "DAMAGE",
                    value: None,
                    attributes: vec![],
                    children: 1..2,
                    parent: None,
                },
                Element {
                    name: "FEMS",
                    value: None,
                    attributes: vec![],
                    children: 2..3,
                    parent: Some(0),
                },
                Element {
                    name: "GEOMETRY",
                    value: None,
                    attributes: vec![("id", "strength_main")],
                    children: 3..4,
                    parent: Some(1),
                },
                Element {
                    name: "BOX",
                    value: None,
                    attributes: vec![],
                    children: 4..4,
                    parent: Some(2),
                },
            ],
        }
    }
    #[test]
    fn consumer_fields_keep_distinct_defaults_and_reject_duplicates() {
        let mut d = document();
        let s = consumer_settings(&d).unwrap();
        assert_eq!(
            [
                s.light_cluster_threshold,
                s.wheel_deformation_amount,
                s.wheel_movement_level
            ],
            [0.1, 0.25, 0.8]
        );
        d.elements[1].attributes = vec![
            ("m_wheel_deformation_amount", "0.001"),
            ("m_wheel_movement_level", "1"),
        ];
        let s = consumer_settings(&d).unwrap();
        assert_eq!(
            [s.wheel_deformation_amount, s.wheel_movement_level],
            [0.001, 1.]
        );
        d.elements[1]
            .attributes
            .push(("m_wheel_movement_level", "2"));
        assert!(consumer_settings(&d).is_err());
        d.elements[1].attributes = vec![("m_wheel_movement_level", "NaN")];
        assert!(consumer_settings(&d).is_err());
    }
    #[test]
    fn defaults_editor_gate_and_normal_map_alias_follow_source_fields() {
        let mut d = document();
        let defaults = settings(&d).unwrap();
        assert_eq!(defaults[0x1000 / 4], 1);
        assert_eq!(value(&defaults, 0), 1.);
        assert_eq!(value(&defaults, 0x40), 0.1);
        assert_eq!(value(&defaults, 0x50), 0.1);
        assert_eq!(value(&defaults, 0x1040), 0.5);
        assert_eq!(value(&defaults, 0x1028), 20.);
        d.elements[1].attributes = vec![
            ("m_damage_multiple", "3"),
            ("max_compression_factor", "2"),
            ("m_wing_pushout_multiple", "8"),
            ("m_normalMapDistance", "0.3"),
            ("m_normalMapDistanceFull", "0.2"),
            ("m_normalMapDistanceStart", "0.1"),
        ];
        let gated = settings(&d).unwrap();
        assert_eq!(value(&gated, 0x1014), 1.);
        assert_eq!(value(&gated, 0x1018), 1.);
        assert_eq!(value(&gated, 0x1040), 4.);
        assert_eq!(value(&gated, 0x102c), 0.2);
        assert_eq!(value(&gated, 0x1028), 10.);
        d.elements[1]
            .attributes
            .push(("m_enableEditorExtraParameters", "1"));
        let enabled = settings(&d).unwrap();
        assert_eq!(value(&enabled, 0x1014), 2.);
        assert_eq!(value(&enabled, 0x1018), 3.);
        for bad in ["NaN 1 1", "-1 1 1", "0 1 1"] {
            let mut d = document();
            d.elements[3].attributes.push(("size", bad));
            assert!(settings(&d).is_err());
        }
        let mut d = document();
        d.elements[3].attributes = vec![("crush", "1"), ("crush", "2")];
        assert!(settings(&d).is_err());
        let mut d = document();
        d.elements[2].children = 3..99;
        assert!(settings(&d).is_err());
    }
}
