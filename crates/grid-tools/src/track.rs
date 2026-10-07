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
    nodes: Vec<u16>,
    leaves: BTreeMap<usize, Vec<u16>>,
    first_triangle: usize,
}

pub struct Track {
    pub grid: Grid,
    chunks: Vec<Chunk>,
    triangle_count: usize,
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
