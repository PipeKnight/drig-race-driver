//! Uncompressed VCQTC archive to source-built, untransformed collision world.
use grid_formats::{collision, jpk};
use grid_world::{
    candidate_grid::Grid, grid_population, packed_contacts, packed_query::Mesh, packed_world,
    rectangle_cull::Cache,
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
    /// Body-query checks of this immutable chunk, evaluated once.
    checked: packed_contacts::Checked,
}

impl Chunk {
    fn mesh(&self) -> Mesh<'_> {
        Mesh {
            bounds: self.bounds,
            vertices: &self.positions,
            triangles: &self.triangles,
            nodes: &self.nodes,
            leaves: &self.leaves,
        }
    }
}

/// The road chunks' shape margin (+0x220): 0.02 in all eight chunk shapes
/// read from the running original (local/behavior/runtime-collision-*.json,
/// 6 October 2026). AEA1F0 gives it to every road triangle (B212D0), on top
/// of the body's own margin (BOX_CONTACTS.md, F350).
pub const ROAD_MARGIN: f32 = 0.02;

pub struct Track {
    pub grid: Grid,
    chunks: Vec<Chunk>,
    triangle_count: usize,
    boundary: Option<crate::cqtc_geometry::Geometry>,
    props: Option<crate::entity_geometry::Props>,
    /// Body-query checks of the attached boundary and of every prop box.
    boundary_checked: Option<packed_contacts::Checked>,
    props_index: Option<crate::entity_geometry::BoxIndex>,
}

/// Fresh geometric record and the same one-based packed shape identity used
/// by wheel queries. Solver material registration remains a separate adapter.
pub struct BodyContact {
    pub shape: u32,
    pub contact: grid_world::contact_manifold::Contact,
    /// Route prop box index for prop contacts.
    pub prop: Option<usize>,
    /// Contact impulse limit (contact +0x3C), FLT_MAX unless a breakable prop.
    pub limit: f32,
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
            let mut chunk = Chunk {
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
                checked: packed_contacts::Checked::default(),
            };
            chunk.checked = packed_contacts::check(&chunk.mesh(), &chunk.material_indices);
            chunks.push(chunk);
        }
        let b = chunks[0].bounds;
        let grid = grid_population::build([b[0], b[2], b[3], b[5]], &entries)?;
        Ok(Self {
            grid,
            chunks,
            triangle_count,
            boundary: None,
            props: None,
            boundary_checked: None,
            props_index: None,
        })
    }
    pub fn triangle_count(&self) -> usize {
        self.triangle_count
    }
    /// Union of the packed road chunk bounds: min x, y, z, max x, y, z.
    pub fn bounds(&self) -> [f32; 6] {
        self.chunks.iter().fold(
            [f32::MAX, f32::MAX, f32::MAX, f32::MIN, f32::MIN, f32::MIN],
            |mut b, c| {
                for k in 0..3 {
                    b[k] = b[k].min(c.bounds[k]);
                    b[k + 3] = b[k + 3].max(c.bounds[k + 3]);
                }
                b
            },
        )
    }
    pub fn shape_count(&self) -> usize {
        self.chunks.len() + usize::from(self.boundary.is_some()) + usize::from(self.props.is_some())
    }
    fn props_shape(&self) -> u32 {
        (self.chunks.len() + usize::from(self.boundary.is_some()) + 1) as u32
    }
    pub fn shape_name(&self, id: u32) -> Option<&str> {
        if self.boundary.is_some() && id as usize == self.chunks.len() + 1 {
            return Some("boundarylines.cqtc");
        }
        if self.props.is_some() && id == self.props_shape() {
            return Some("objects.ens");
        }
        id.checked_sub(1)
            .and_then(|i| self.chunks.get(i as usize))
            .map(|c| c.name.as_str())
    }

    pub(crate) fn attach_boundary(&mut self, boundary: crate::cqtc_geometry::Geometry) {
        self.boundary_checked = Some(boundary.checked());
        self.boundary = Some(boundary);
    }
    pub(crate) fn attach_props(&mut self, props: crate::entity_geometry::Props) {
        self.props_index = Some(crate::entity_geometry::BoxIndex::new(&props.boxes));
        self.props = Some(props);
    }
    pub fn props(&self) -> Option<&crate::entity_geometry::Props> {
        self.props.as_ref()
    }
    /// Mutable route props: breakable limits and broken boxes.
    pub fn props_mut(&mut self) -> Option<&mut crate::entity_geometry::Props> {
        self.props.as_mut()
    }
    pub fn has_props(&self) -> bool {
        self.props.is_some()
    }
    pub fn has_boundary(&self) -> bool {
        self.boundary.is_some()
    }
    pub fn triangle_index(&self, id: u32, local: usize) -> Option<usize> {
        let c = self.chunks.get(id.checked_sub(1)? as usize)?;
        (local < c.triangles.len()).then_some(c.first_triangle + local)
    }

    /// Selected cold pair queries in boundary-first then archive order, with inclusive root AABB
    /// culling and at most 16 contacts per pair / 64 in the returned world pool.
    /// Archive order is authored scheduling, not the original broadphase order.
    /// `sheet` is consulted only for shapes the body can reach.
    /// Contacts of one moving hull with the route props, the boundary sheet
    /// and every packed road chunk. As in the original, where each shape
    /// pair's generator fills its own manifold (AEA1F0, capacity per call),
    /// every one of these sources gets up to `pair_capacity` contacts; only
    /// the world pool share `pool` bounds the sum, so a wall or prop the body
    /// leans on can no longer use up the road's slots (F342).
    pub fn body_contacts(
        &self,
        mesh: &grid_world::collision_mesh::Mesh,
        matrix: grid_world::physical_pose::Matrix,
        query: &grid_world::convex_distance::ContactSettings,
        pair_capacity: usize,
        pool: usize,
        mut sheet: impl FnMut([u8; 4]) -> Result<bool, grid_world::Error>,
    ) -> Result<Vec<BodyContact>, grid_world::Error> {
        if pair_capacity > 16 || pool > grid_world::MAX_WORLD_CONTACTS {
            return Err(grid_world::Error(
                "body world contact capacity exceeds bound",
            ));
        }
        let room = |used: usize| pair_capacity.min(pool - used);
        let bounds = mesh.world_bounds(matrix, query.margin)?;
        // Checks and bounds shared by every pair query of this body.
        let directions = packed_contacts::directions_valid(query);
        let known = |checked| packed_contacts::Known {
            checked,
            directions,
            bounds,
        };
        let mut contacts = Vec::new();
        // Visible route barriers and props first: the car must meet their
        // surfaces before the invisible boundary sheet behind them.
        if let Some(props) = &self.props
            && room(0) != 0
        {
            let shape = self.props_shape();
            contacts.extend(
                props
                    .contacts_known(
                        mesh,
                        matrix,
                        query,
                        bounds,
                        room(0),
                        self.props_index
                            .as_ref()
                            .map(|index| crate::entity_geometry::KnownBoxes { index }),
                    )?
                    .into_iter()
                    .map(|(index, contact)| BodyContact {
                        shape,
                        contact,
                        prop: Some(index),
                        limit: props.boxes[index].limit,
                    }),
            );
        }
        let settings = |remaining: usize, shape_margin: f32| packed_contacts::Settings {
            query: grid_world::convex_distance::ContactSettings {
                margin: query.margin,
                directions: query.directions,
            },
            shape_margin,
            contact_capacity: remaining.min(16),
            cache_enabled: true,
            candidate_capacity: 4096,
            sheet_mask: 0,
            correction_bounds: [f32::MAX; 4],
        };
        // Pair queries that provably yield nothing (no reachable triangle
        // near the body) are skipped before their sheet flags are consulted.
        // Authored boundary-first scheduling keeps packed road contacts from
        // exhausting the selected pool before this separate source shape.
        if let Some(boundary) = &self.boundary
            && room(contacts.len()) != 0
            && (0..3).all(|j| {
                bounds[0][j] <= boundary.bounds[j + 3] && bounds[1][j] >= boundary.bounds[j]
            })
        {
            let known = known(self.boundary_checked.unwrap_or_else(|| boundary.checked()));
            // The boundary sheet's shape margin is not established; none.
            let mut settings = settings(room(contacts.len()), 0.);
            if !known.misses(&settings, true) {
                for (j, &key) in boundary.material_keys.iter().enumerate() {
                    settings.sheet_mask |= u8::from(sheet(key)?) << j;
                }
                let generated = boundary.contacts_known(mesh, matrix, &settings, &known)?;
                contacts.extend(generated.into_iter().map(|contact| BodyContact {
                    shape: self.chunks.len() as u32 + 1,
                    contact,
                    prop: None,
                    limit: f32::MAX,
                }));
            }
        }
        let road_bounds = mesh.world_bounds(matrix, query.margin + ROAD_MARGIN)?;
        for (i, c) in self.chunks.iter().enumerate() {
            if contacts.len() == pool {
                break;
            }
            if (0..3)
                .any(|j| road_bounds[0][j] > c.bounds[j + 3] || road_bounds[1][j] < c.bounds[j])
            {
                continue;
            }
            if c.material_keys.len() > 8 {
                return Err(grid_world::Error(
                    "packed body material count exceeds bound",
                ));
            }
            let known = packed_contacts::Known {
                checked: c.checked,
                directions,
                bounds: road_bounds,
            };
            let mut settings = settings(room(contacts.len()), ROAD_MARGIN);
            if known.misses(&settings, false) {
                continue;
            }
            for (j, &key) in c.material_keys.iter().enumerate() {
                settings.sheet_mask |= u8::from(sheet(key)?) << j;
            }
            let generated = packed_contacts::calculate_known(
                mesh,
                matrix,
                &c.mesh(),
                &c.material_indices,
                &settings,
                &known,
            )?;
            contacts.extend(generated.into_iter().map(|contact| BodyContact {
                shape: i as u32 + 1,
                contact,
                prop: None,
                limit: f32::MAX,
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
        packed_world::query_sequential(
            &self.grid,
            self.chunks.len(),
            |i| {
                let c = &self.chunks[i];
                packed_world::Shape {
                    id: i as u32 + 1,
                    mesh: c.mesh(),
                    materials: &c.materials,
                }
            },
            start,
            end,
            cache,
        )
    }
}
