//! Uncompressed VCQTC archive to source-built, untransformed collision world.
use grid_formats::{collision, jpk};
use grid_world::{
    candidate_grid::Grid, grid_population, packed_query::Mesh, packed_world, rectangle_cull::Cache,
};
use std::collections::BTreeMap;

struct Chunk {
    name: String,
    bounds: [f32; 6],
    positions: Vec<[f32; 3]>,
    triangles: Vec<[u16; 3]>,
    materials: Vec<[u8; 4]>,
    material_indices: Vec<u8>,
    material_keys: Vec<[u8; 4]>,
    nodes: Vec<u16>,
    leaves: BTreeMap<usize, Vec<u16>>,
    first_triangle: usize,
}

pub struct Track {
    pub grid: Grid,
    chunks: Vec<Chunk>,
    triangle_count: usize,
}

/// Fresh geometric record and the same one-based packed shape identity used
/// by wheel queries. Solver material registration remains a separate adapter.
pub struct BodyContact {
    pub shape: u32,
    pub contact: grid_world::contact_manifold::Contact,
}

impl Track {
    pub fn read(bytes: &[u8]) -> Result<Self, Box<dyn std::error::Error>> {
        if bytes.len() > 256 * 1024 * 1024 {
            return Err("collision archive exceeds bound".into());
        }
        let archive = jpk::inspect(bytes)?;
        if archive.entries.is_empty() || archive.entries.len() > 512 {
            return Err("collision archive entry count unsupported".into());
        }
        let mut chunks = Vec::new();
        let mut entries = Vec::new();
        let mut triangle_count = 0;
        for (i, entry) in archive.entries.into_iter().enumerate() {
            if entry.stored_data.len() != entry.uncompressed_size as usize {
                return Err("compressed collision entry unsupported".into());
            }
            let name = std::str::from_utf8(entry.name)?;
            entries.push((i as u32 + 1, grid_population::Path::from_name(name)?));
            let chunk = collision::read_vcqtc(&bytes[entry.stored_data])?;
            if triangle_count + chunk.triangles.len() > 1000000 {
                return Err("collision triangle count exceeds bound".into());
            }
            let first_triangle = triangle_count;
            triangle_count += chunk.triangles.len();
            chunks.push(Chunk {
                name: name.to_owned(),
                bounds: chunk.bounds,
                positions: chunk.positions,
                triangles: chunk.triangles.iter().map(|t| t.indices).collect(),
                materials: chunk
                    .triangles
                    .iter()
                    .map(|t| chunk.materials[t.material as usize])
                    .collect(),
                material_indices: chunk.triangles.iter().map(|t| t.material).collect(),
                material_keys: chunk.materials,
                nodes: chunk.nodes,
                leaves: chunk.references,
                first_triangle,
            });
        }
        let b = chunks[0].bounds;
        let grid = grid_population::build([b[0], b[2], b[3], b[5]], &entries)?;
        Ok(Self {
            grid,
            chunks,
            triangle_count,
        })
    }
    pub fn triangle_count(&self) -> usize {
        self.triangle_count
    }
    pub fn shape_count(&self) -> usize {
        self.chunks.len()
    }
    pub fn shape_name(&self, id: u32) -> Option<&str> {
        id.checked_sub(1)
            .and_then(|i| self.chunks.get(i as usize))
            .map(|c| c.name.as_str())
    }
    pub fn triangle_index(&self, id: u32, local: usize) -> Option<usize> {
        let c = self.chunks.get(id.checked_sub(1)? as usize)?;
        (local < c.triangles.len()).then_some(c.first_triangle + local)
    }

    /// Selected cold pair queries in archive order, with inclusive root AABB
    /// culling and at most 16 contacts per pair / 64 in the returned world pool.
    /// Archive order is authored scheduling, not the original broadphase order.
    pub fn body_contacts(
        &self,
        mesh: &grid_world::collision_mesh::Mesh,
        matrix: grid_world::physical_pose::Matrix,
        query: &grid_world::convex_distance::ContactSettings,
        capacity: usize,
        mut sheet: impl FnMut([u8; 4]) -> Result<bool, grid_world::Error>,
    ) -> Result<Vec<BodyContact>, grid_world::Error> {
        if capacity > 64 {
            return Err(grid_world::Error(
                "body world contact capacity exceeds bound",
            ));
        }
        let bounds = mesh.world_bounds(matrix, query.margin)?;
        let mut contacts = Vec::new();
        for (i, c) in self.chunks.iter().enumerate() {
            if contacts.len() == capacity {
                break;
            }
            if (0..3).any(|j| bounds[0][j] > c.bounds[j + 3] || bounds[1][j] < c.bounds[j]) {
                continue;
            }
            if c.material_keys.len() > 8 {
                return Err(grid_world::Error(
                    "packed body material count exceeds bound",
                ));
            }
            let mut mask = 0;
            for (j, &key) in c.material_keys.iter().enumerate() {
                mask |= u8::from(sheet(key)?) << j;
            }
            let generated = grid_world::packed_contacts::calculate(
                mesh,
                matrix,
                &Mesh {
                    bounds: c.bounds,
                    vertices: &c.positions,
                    triangles: &c.triangles,
                    nodes: &c.nodes,
                    leaves: &c.leaves,
                },
                &c.material_indices,
                &grid_world::packed_contacts::Settings {
                    query: grid_world::convex_distance::ContactSettings {
                        margin: query.margin,
                        directions: query.directions,
                    },
                    contact_capacity: (capacity - contacts.len()).min(16),
                    cache_enabled: true,
                    candidate_capacity: 4096,
                    sheet_mask: mask,
                    correction_bounds: [f32::MAX; 4],
                },
            )?;
            contacts.extend(generated.contacts.into_iter().map(|contact| BodyContact {
                shape: i as u32 + 1,
                contact,
            }));
        }
        Ok(contacts)
    }
    pub fn query(
        &self,
        start: [f32; 4],
        end: [f32; 4],
        cache: &mut Cache,
    ) -> Result<packed_world::Query, grid_world::Error> {
        let shapes: Vec<_> = self
            .chunks
            .iter()
            .enumerate()
            .map(|(i, c)| packed_world::Shape {
                id: i as u32 + 1,
                mesh: Mesh {
                    bounds: c.bounds,
                    vertices: &c.positions,
                    triangles: &c.triangles,
                    nodes: &c.nodes,
                    leaves: &c.leaves,
                },
                materials: &c.materials,
            })
            .collect();
        packed_world::query(&self.grid, &shapes, start, end, cache)
    }
}
