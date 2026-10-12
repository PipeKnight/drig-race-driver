//! Five named NED handling regions, separate from wheel regions and HDM settings.
use crate::{Result, binxml::Document, fail};

pub const NAMES: [&str; 5] = ["GEARBOX", "DRIVESHAFT", "TURBO", "RADIATOR", "EXHAUST"];
pub const OFFSETS: [usize; 5] = [0x120, 0x150, 0x180, 0x1b0, 0x1e0];

#[derive(Clone, Copy)]
pub struct Region {
    pub center: [f32; 3],
    /// Original loader stores source m_size values directly; no division by two.
    pub half: [f32; 3],
}

pub fn read(document: &Document<'_>) -> Result<[Option<Region>; 5]> {
    let Some(root) = document.elements.first() else {
        return fail(0, "missing NED root");
    };
    if root.name != "DAMAGE" {
        return fail(0, "unsupported NED root");
    }
    let parent_count = document
        .elements
        .iter()
        .filter(|e| e.name == "HANDLINGDAMAGE")
        .count();
    if parent_count > 1 {
        return fail(0, "ambiguous handling region parent");
    }
    let mut nodes = document.elements[root.children.clone()]
        .iter()
        .filter(|e| e.name == "HANDLINGDAMAGE");
    let node = nodes.next();
    if nodes.next().is_some() {
        return fail(0, "ambiguous handling region parent");
    }
    let mut result = [None; 5];
    let Some(node) = node else {
        if parent_count != 0 {
            return fail(0, "unsupported nested handling region parent");
        }
        return Ok(result);
    };
    for (slot, name) in result.iter_mut().zip(NAMES) {
        let mut regions = document.elements[node.children.clone()]
            .iter()
            .filter(|e| e.name == name);
        let Some(region) = regions.next() else {
            continue;
        };
        if regions.next().is_some() {
            return fail(0, "ambiguous handling region");
        }
        let mut values = [0.; 6];
        for (index, key) in [
            "m_centre_x",
            "m_centre_y",
            "m_centre_z",
            "m_size_x",
            "m_size_y",
            "m_size_z",
        ]
        .into_iter()
        .enumerate()
        {
            let mut attributes = region.attributes.iter().filter(|(k, _)| *k == key);
            let Some((_, text)) = attributes.next() else {
                return fail(0, "incomplete handling region attributes");
            };
            if attributes.next().is_some() {
                return fail(0, "duplicate handling region attribute");
            }
            let value = text.parse::<f64>().map_err(|_| crate::Error {
                offset: 0,
                message: "invalid handling region number",
            })?;
            if !value.is_finite() || value.abs() > 100. || (index >= 3 && value < 0.) {
                return fail(0, "unsupported handling region extent");
            }
            values[index] = value as f32;
        }
        *slot = Some(Region {
            center: values[..3].try_into().unwrap(),
            half: values[3..].try_into().unwrap(),
        });
    }
    Ok(result)
}

/// Apply a selected region, preserving all padding or the entire absent record.
pub fn apply(region: Option<Region>, offset: [f32; 4], record: &mut [u8; 48]) -> Result<()> {
    if offset.iter().any(|v| !v.is_finite() || v.abs() > 100.) || offset[3] != 0. {
        return fail(0, "unsupported handling region offset");
    }
    let Some(region) = region else { return Ok(()) };
    if region
        .center
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 100.)
        || region.half.iter().any(|v| !(0. ..=100.).contains(v))
    {
        return fail(0, "unsupported handling region values");
    }
    for j in 0..4 {
        let center = if j == 3 {
            1. + offset[j]
        } else {
            region.center[j] + offset[j]
        };
        let half = if j == 3 { 1. } else { region.half[j] };
        record[j * 4..j * 4 + 4].copy_from_slice(&center.to_le_bytes());
        record[16 + j * 4..20 + j * 4].copy_from_slice(&half.to_le_bytes());
    }
    record[32] = 1;
    Ok(())
}
