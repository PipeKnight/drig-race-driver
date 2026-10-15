//! Selected CQTC boundary tree representation for shared cold contact queries.
//! Wheel/segment queries remain on the separately registered packed track.
use grid_formats::cqtc;
use grid_world::{Error, packed_query::Mesh};
use std::collections::BTreeMap;

pub struct Geometry {
    pub bounds: [f32; 6],
    pub positions: Vec<[f32; 3]>,
    pub triangles: Vec<[u16; 3]>,
    pub material_indices: Vec<u8>,
    pub material_keys: Vec<[u8; 4]>,
    nodes: Vec<u16>,
    leaves: BTreeMap<usize, Vec<u16>>,
}

impl Geometry {
    pub fn read(bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        let c = cqtc::read(bytes)?;
        if c.nodes.len() > 32768 || c.references.values().any(|ids| ids.len() > 256) {
            return Err("CQTC selected tree/list bound exceeded".into());
        }
        let mut nodes = Vec::with_capacity(c.nodes.len());
        for n in c.nodes {
            let node = if n == 0xffffff {
                0xffff
            } else if n & 0x800000 != 0 {
                let offset = n & 0x7fffff;
                if offset >= 32767 {
                    return Err("CQTC leaf offset cannot fit selected representation".into());
                }
                0x8000 | offset as u16
            } else {
                if n >= 32768 {
                    return Err("CQTC child index cannot fit selected representation".into());
                }
                n as u16
            };
            nodes.push(node);
        }
        // Reject cycles even when a particular query rectangle would cull them.
        let mut active = vec![false; nodes.len()];
        let mut stack = vec![(0usize, 0usize, false)];
        let mut visits = 0;
        while let Some((index, depth, leaving)) = stack.pop() {
            if leaving {
                active[index] = false;
                continue;
            }
            visits += 1;
            if depth > 64 || visits > 1_000_000 || active[index] {
                return Err("CQTC selected tree cycle/depth/visit bound exceeded".into());
            }
            let n = nodes[index];
            if n & 0x8000 == 0 {
                active[index] = true;
                stack.push((index, depth, true));
                for child in (0..4).rev() {
                    stack.push((n as usize + child, depth + 1, false));
                }
            }
        }
        Ok(Self {
            bounds: c.bounds,
            positions: c.positions,
            triangles: c.triangles.iter().map(|t| t.indices).collect(),
            material_indices: c.triangles.iter().map(|t| t.material).collect(),
            material_keys: c.materials,
            nodes,
            leaves: c
                .references
                .into_iter()
                .map(|(offset, ids)| (offset, ids.into_iter().map(|x| x as u16).collect()))
                .collect(),
        })
    }
    pub fn mesh(&self) -> Mesh<'_> {
        Mesh {
            bounds: self.bounds,
            vertices: &self.positions,
            triangles: &self.triangles,
            nodes: &self.nodes,
            leaves: &self.leaves,
        }
    }
    pub fn contacts(
        &self,
        mesh: &grid_world::collision_mesh::Mesh,
        matrix: grid_world::physical_pose::Matrix,
        settings: &grid_world::packed_contacts::Settings,
    ) -> Result<grid_world::packed_contacts::Output, Error> {
        grid_world::packed_contacts::calculate_decoded(
            mesh,
            matrix,
            &self.mesh(),
            &self.material_indices,
            settings,
        )
    }
    /// Body-query checks of the current tree and materials.
    pub fn checked(&self) -> grid_world::packed_contacts::Checked {
        grid_world::packed_contacts::check(&self.mesh(), &self.material_indices)
    }
    /// The contacts of [`Self::contacts`], reusing values the caller holds for
    /// this unchanged geometry and pose (see [`grid_world::packed_contacts::Known`]).
    pub fn contacts_known(
        &self,
        mesh: &grid_world::collision_mesh::Mesh,
        matrix: grid_world::physical_pose::Matrix,
        settings: &grid_world::packed_contacts::Settings,
        known: &grid_world::packed_contacts::Known,
    ) -> Result<Vec<grid_world::contact_manifold::Contact>, Error> {
        grid_world::packed_contacts::calculate_decoded_known(
            mesh,
            matrix,
            &self.mesh(),
            &self.material_indices,
            settings,
            known,
        )
    }
}
