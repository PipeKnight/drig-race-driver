//! Supported named car collision mesh and its local bounds; no contact solver.
use crate::{
    Result,
    binxml::{Document, Element},
    fail,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Bounds {
    /// Description fields: center X, bottom Y, front Z, left X, right X, rear Z, roof Y.
    /// Corner/roof-edge fields require additional mesh queries and are not included.
    pub fn description_extents(&self) -> Result<[f32; 7]> {
        let center = (self.min[0] + self.max[0]) * 0.5;
        if !center.is_finite() {
            return fail(0, "collision center exceeds finite f32 range");
        }
        Ok([
            center,
            self.min[1],
            self.max[2],
            self.max[0],
            self.min[0],
            self.min[2],
            self.max[1],
        ])
    }
}

#[derive(Debug)]
pub struct Mesh {
    pub points: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub bounds: Bounds,
}

fn attribute<'a>(element: &Element<'a>, name: &str) -> Result<&'a str> {
    let mut values = element.attributes.iter().filter(|(key, _)| *key == name);
    let value = values
        .next()
        .ok_or(crate::Error {
            offset: 0,
            message: "required collision attribute is absent",
        })?
        .1;
    if values.next().is_some() {
        return fail(0, "duplicate collision attribute");
    }
    Ok(value)
}

fn children<'a, 'b>(doc: &'b Document<'a>, element: &Element<'a>) -> Result<&'b [Element<'a>]> {
    doc.elements
        .get(element.children.clone())
        .ok_or(crate::Error {
            offset: element.children.start,
            message: "collision child range outside document",
        })
}

/// The COLLISION `m_groundClearence` attribute: the reader 0x9EFB30 stores it
/// at description +0x1B20 (body +0xED60) after presetting 0.17 (0xCE770C), so
/// a NED without it keeps 0.17 [V, F406]. Every shipped car declares 0.07 or
/// 0.05 [D].
pub fn ground_clearance(document: &Document<'_>) -> Result<f32> {
    let mut candidates = document.elements.iter().filter(|e| e.name == "COLLISION");
    let collision = candidates.next().ok_or(crate::Error {
        offset: 0,
        message: "car COLLISION element is absent",
    })?;
    if candidates.next().is_some() {
        return fail(0, "car COLLISION element is ambiguous");
    }
    let mut values = collision
        .attributes
        .iter()
        .filter(|(key, _)| *key == "m_groundClearence");
    let Some((_, text)) = values.next() else {
        return Ok(f32::from_bits(0x3e2e_147b));
    };
    if values.next().is_some() {
        return fail(0, "duplicate collision attribute");
    }
    let value = text.trim().parse::<f64>().map_err(|_| crate::Error {
        offset: 0,
        message: "invalid collision ground clearance",
    })? as f32;
    if !value.is_finite() {
        return fail(0, "nonfinite collision ground clearance");
    }
    Ok(value)
}

/// Read exactly one COLLISION/GEOMETRY with id collision_main.
/// Reject oversized meshes rather than reproducing the original's silent truncation.
pub fn read(document: &Document<'_>) -> Result<Mesh> {
    let mut candidates = document.elements.iter().filter(|e| e.name == "COLLISION");
    let collision = candidates.next().ok_or(crate::Error {
        offset: 0,
        message: "car COLLISION element is absent",
    })?;
    if candidates.next().is_some() {
        return fail(0, "car COLLISION element is ambiguous");
    }
    let mut geometry = None;
    for child in children(document, collision)? {
        if child.name == "GEOMETRY"
            && attribute(child, "id")? == "collision_main"
            && geometry.replace(child).is_some()
        {
            return fail(0, "collision_main geometry is ambiguous");
        }
    }
    let geometry = geometry.ok_or(crate::Error {
        offset: 0,
        message: "collision_main geometry is absent",
    })?;
    let mut points = Vec::new();
    let mut triangles = Vec::new();
    for child in children(document, geometry)? {
        match child.name {
            "POINT" => {
                if points.len() == 80 {
                    return fail(0, "car collision point limit exceeded");
                }
                let fields: Vec<_> = attribute(child, "pos")?
                    .split_whitespace()
                    .take(4)
                    .collect();
                if fields.len() != 3 {
                    return fail(0, "collision point requires three coordinates");
                }
                let mut point = [0.; 3];
                for (value, field) in point.iter_mut().zip(fields) {
                    *value = field.parse::<f64>().map_err(|_| crate::Error {
                        offset: 0,
                        message: "invalid collision coordinate",
                    })? as f32;
                    if !value.is_finite() {
                        return fail(0, "nonfinite collision coordinate");
                    }
                }
                points.push(point);
            }
            "TRIANGLE" => {
                if triangles.len() == 110 {
                    return fail(0, "car collision triangle limit exceeded");
                }
                let fields: Vec<_> = attribute(child, "points")?
                    .split_whitespace()
                    .take(4)
                    .collect();
                if fields.len() != 3 {
                    return fail(0, "collision triangle requires three indices");
                }
                let mut triangle = [0; 3];
                for (value, field) in triangle.iter_mut().zip(fields) {
                    *value = field.parse::<u32>().map_err(|_| crate::Error {
                        offset: 0,
                        message: "collision index is not an unsigned integer",
                    })?;
                }
                triangles.push(triangle);
            }
            _ => return fail(0, "unsupported car collision geometry child"),
        }
    }
    if points.is_empty() || triangles.is_empty() {
        return fail(0, "empty car collision mesh");
    }
    if triangles
        .iter()
        .flatten()
        .any(|&index| index as usize >= points.len())
    {
        return fail(0, "car collision index outside point array");
    }
    let mut bounds = Bounds {
        min: points[0],
        max: points[0],
    };
    for point in &points[1..] {
        for (axis, &value) in point.iter().enumerate() {
            // Equality selects the later operand, including signed zero, as MINPS/MAXPS do.
            bounds.min[axis] = if bounds.min[axis] < value {
                bounds.min[axis]
            } else {
                value
            };
            bounds.max[axis] = if bounds.max[axis] > value {
                bounds.max[axis]
            } else {
                value
            };
        }
    }
    bounds.description_extents()?;
    Ok(Mesh {
        points,
        triangles,
        bounds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_ground_clearances() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        for (stem, expected) in [("lac", 0.07f32), ("lc7", 0.07), ("spk", 0.05), ("b05", 0.05)] {
            let bytes = std::fs::read(root.join(format!("cars/{stem}/{stem}.ned"))).unwrap();
            let doc = crate::binxml::read(&bytes).unwrap();
            assert_eq!(ground_clearance(&doc).unwrap(), expected, "{stem}");
        }
    }
    fn fixture() -> Document<'static> {
        let element = |name, attributes, children| Element {
            name,
            attributes,
            children,
            parent: None,
            value: None,
        };
        Document {
            strings: vec![],
            string_padding: &[],
            elements: vec![
                element("COLLISION", vec![], 1..2),
                element("GEOMETRY", vec![("id", "collision_main")], 2..6),
                element("POINT", vec![("pos", "-1 0 -2")], 6..6),
                element("POINT", vec![("pos", "3 1 2")], 6..6),
                element("POINT", vec![("pos", "0 4 0")], 6..6),
                element("TRIANGLE", vec![("points", "0 1 2")], 6..6),
            ],
        }
    }
    #[test]
    fn bounds_use_mesh_points_and_ignore_named_defaults() {
        let mut doc = fixture();
        doc.elements[0]
            .attributes
            .push(("m_cs_front_centre_z", "99"));
        let mesh = read(&doc).unwrap();
        assert_eq!(
            mesh.bounds.description_extents().unwrap(),
            [1., 0., 2., 3., -1., -2., 4.]
        );
        assert_eq!(mesh.triangles, [[0, 1, 2]]);
        doc.elements[2].attributes[0].1 = "-0 0 -2";
        doc.elements[3].attributes[0].1 = "0 1 2";
        doc.elements[4].attributes[0].1 = "-0 4 0";
        let mesh = read(&doc).unwrap();
        assert_eq!(mesh.bounds.min[0].to_bits(), (-0.0_f32).to_bits());
        assert_eq!(mesh.bounds.max[0].to_bits(), (-0.0_f32).to_bits());
    }
    #[test]
    fn malformed_ambiguous_and_oversized_meshes_are_rejected() {
        for invalid in ["NaN 0 0", "1e300 0 0", "0 0", "0 0 0 0"] {
            let mut doc = fixture();
            doc.elements[2].attributes[0].1 = invalid;
            assert!(read(&doc).is_err());
        }
        for invalid in ["0 1 3", "-1 1 2", "0.5 1 2", "0 1", "0 1 2 3"] {
            let mut doc = fixture();
            doc.elements[5].attributes[0].1 = invalid;
            assert!(read(&doc).is_err());
        }
        let mut doc = fixture();
        doc.elements[2].attributes.push(("pos", "0 0 0"));
        assert!(read(&doc).is_err());
        let mut doc = fixture();
        doc.elements[1].attributes[0].1 = "other";
        assert!(read(&doc).is_err());
        let mut doc = fixture();
        doc.elements[0].children = 1..99;
        assert!(read(&doc).is_err());
        let mut doc = fixture();
        doc.elements[5].name = "COLLISION";
        assert!(read(&doc).is_err());
        let mut doc = fixture();
        doc.elements.extend((0..78).map(|_| Element {
            name: "POINT",
            attributes: vec![("pos", "0 0 0")],
            children: 6..6,
            parent: None,
            value: None,
        }));
        doc.elements[1].children.end = doc.elements.len();
        assert!(read(&doc).is_err());
    }
}
