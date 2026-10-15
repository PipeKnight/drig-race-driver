//! Static collision boxes of the purchased route entities (objects.ens with
//! objecttypes.pssg): tyre walls, cushions, armco, tyre stacks and marker boards.
//! Each box is twelve triangles in a one-leaf decoded tree, queried through the
//! shared cold body contact path. All placements are infinite-mass in the data;
//! breakable/dynamic behaviour (physparams.xml thresholds) is not implemented.
use grid_formats::{entities, pssg};
use grid_world::{Error, packed_query::Mesh};
use std::{collections::BTreeMap, path::Path};

pub struct PropBox {
    pub template: String,
    pub bounds: [f32; 6],
    pub positions: Vec<[f32; 3]>,
    triangles: Vec<[u16; 3]>,
    nodes: Vec<u16>,
    leaves: BTreeMap<usize, Vec<u16>>,
}

pub struct Props {
    pub boxes: Vec<PropBox>,
    materials: Vec<u8>,
}

/// Row-vector placement: translation in elements 12..14.
fn transform(m: &[f32; 16], p: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|j| p[0] * m[j] + p[1] * m[4 + j] + p[2] * m[8 + j] + m[12 + j])
}

// Outward faces of the unit cube corners indexed by bits (x, y, z).
const FACES: [[u16; 3]; 12] = [
    [0, 2, 3],
    [0, 3, 1],
    [4, 5, 7],
    [4, 7, 6],
    [0, 1, 5],
    [0, 5, 4],
    [2, 6, 7],
    [2, 7, 3],
    [0, 4, 6],
    [0, 6, 2],
    [1, 3, 7],
    [1, 7, 5],
];

impl Props {
    pub fn read(root: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let read = |p: &str| -> Result<Vec<u8>, Box<dyn std::error::Error>> {
            let bytes = std::fs::read(root.join(p))?;
            if bytes.len() > 64 << 20 {
                return Err("entity source exceeds bound".into());
            }
            Ok(bytes)
        };
        let placements = read("tracks/europe/donington/route_0/objects.ens")?;
        let types = read("tracks/europe/donington/objecttypes.pssg")?;
        let placements = pssg::read_tree(&placements)?;
        let types = pssg::read_tree(&types)?;
        let mut boxes = Vec::new();
        for e in entities::read(&placements, &types)? {
            for r in &e.renderables {
                for s in &r.shapes {
                    let entities::Primitive::Box {
                        width,
                        height,
                        length,
                    } = s.primitive
                    else {
                        continue;
                    };
                    if s.phantom {
                        continue;
                    }
                    let half = [width * 0.5, height * 0.5, length * 0.5];
                    if !half.iter().all(|h| h.is_finite() && *h > 0. && *h < 100.) {
                        return Err("invalid entity collision box".into());
                    }
                    let positions: Vec<[f32; 3]> = (0..8)
                        .map(|i| {
                            let local = std::array::from_fn(|j| {
                                if i >> j & 1 == 0 { -half[j] } else { half[j] }
                            });
                            transform(&e.transform, transform(&s.transform, local))
                        })
                        .collect();
                    if positions.iter().flatten().any(|x| !x.is_finite()) {
                        return Err("nonfinite entity collision box".into());
                    }
                    let mut bounds = [f32::MAX, f32::MAX, f32::MAX, f32::MIN, f32::MIN, f32::MIN];
                    for p in &positions {
                        for j in 0..3 {
                            bounds[j] = bounds[j].min(p[j]);
                            bounds[j + 3] = bounds[j + 3].max(p[j]);
                        }
                    }
                    boxes.push(PropBox {
                        template: String::from_utf8_lossy(e.template).into_owned(),
                        bounds,
                        positions,
                        triangles: FACES.to_vec(),
                        nodes: vec![0x8000],
                        leaves: BTreeMap::from([(0, (0..12).collect())]),
                    });
                }
            }
        }
        if boxes.is_empty() || boxes.len() > 8192 {
            return Err("unsupported entity collision box count".into());
        }
        Ok(Self {
            boxes,
            materials: vec![0; 12],
        })
    }

    /// Cold contacts against every box whose bounds overlap the car's margin-
    /// inflated world bounds, in data order, up to `capacity` (<=16 per box).
    pub fn contacts(
        &self,
        mesh: &grid_world::collision_mesh::Mesh,
        matrix: grid_world::physical_pose::Matrix,
        query: &grid_world::convex_distance::ContactSettings,
        bounds: [[f32; 4]; 2],
        capacity: usize,
    ) -> Result<Vec<grid_world::contact_manifold::Contact>, Error> {
        let mut contacts = Vec::new();
        for b in &self.boxes {
            if contacts.len() >= capacity {
                break;
            }
            if (0..3).any(|j| bounds[0][j] > b.bounds[j + 3] || bounds[1][j] < b.bounds[j]) {
                continue;
            }
            let generated = grid_world::packed_contacts::calculate_decoded(
                mesh,
                matrix,
                &Mesh {
                    bounds: b.bounds,
                    vertices: &b.positions,
                    triangles: &b.triangles,
                    nodes: &b.nodes,
                    leaves: &b.leaves,
                },
                &self.materials,
                &grid_world::packed_contacts::Settings {
                    query: grid_world::convex_distance::ContactSettings {
                        margin: query.margin,
                        directions: query.directions,
                    },
                    contact_capacity: (capacity - contacts.len()).min(16),
                    cache_enabled: true,
                    candidate_capacity: 64,
                    // Witness normals: solid boxes, not one-sided sheets.
                    sheet_mask: 0,
                    correction_bounds: [f32::MAX; 4],
                },
            )?;
            contacts.extend(generated.contacts);
        }
        Ok(contacts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn donington_route_props_are_solid_boxes() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let props = Props::read(Path::new(&root)).unwrap();
        let mut templates = BTreeMap::<&str, usize>::new();
        for b in &props.boxes {
            *templates.entry(b.template.as_str()).or_default() += 1;
            assert!((0..3).all(|j| b.bounds[j] < b.bounds[j + 3]));
        }
        eprintln!(
            "route_props boxes={} templates={templates:?}",
            props.boxes.len()
        );
        assert_eq!(props.boxes.len(), 1021);
    }
}
