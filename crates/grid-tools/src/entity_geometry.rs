//! Static collision boxes of the purchased route entities (objects.ens with
//! objecttypes.pssg): tyre walls, cushions, armco, tyre stacks and marker boards.
//! Each box is twelve triangles in a one-leaf decoded tree, queried through the
//! shared cold body contact path. All placements are infinite-mass in the data;
//! breakable/dynamic behaviour (physparams.xml thresholds) is not implemented.
use crate::route::Route;
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

impl PropBox {
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

impl PropBox {
    /// Eight world corners indexed by bits (x, y, z) of the local box.
    fn new(template: String, positions: Vec<[f32; 3]>) -> Result<Self, Box<dyn std::error::Error>> {
        if positions.len() != 8 || positions.iter().flatten().any(|x| !x.is_finite()) {
            return Err("invalid route collision box".into());
        }
        let mut bounds = [f32::MAX, f32::MAX, f32::MAX, f32::MIN, f32::MIN, f32::MIN];
        for p in &positions {
            for j in 0..3 {
                bounds[j] = bounds[j].min(p[j]);
                bounds[j + 3] = bounds[j + 3].max(p[j]);
            }
        }
        Ok(Self {
            template,
            bounds,
            positions,
            triangles: FACES.to_vec(),
            nodes: vec![0x8000],
            leaves: BTreeMap::from([(0, (0..12).collect())]),
        })
    }
}

/// Visible trackside barrier ornaments (route_0/ornaments.bin instances of
/// objects.pssg prototypes). Their meshes stand up to ~0.5 m (concrete blocks)
/// in front of the boundary sheet, which otherwise lets the car sink into them
/// (owner playtest 15 October). The original's collision for these instances
/// is not recovered; this solid oriented box of each prototype's mesh extent is
/// an authored stand-in. Poles, lights and other ornaments stay non-solid.
const BARRIER_FAMILIES: [&str; 4] = ["concrete_", "chnfen", "gg_ad_brd_", "don_ad_brd_"];

fn ornament_barriers(
    root: &Path,
    route: &Route,
) -> Result<Vec<PropBox>, Box<dyn std::error::Error>> {
    use grid_formats::{mesh, ornaments, scene};
    let objects = std::fs::read(root.join(route.track_file("objects.pssg")))?;
    let tree = pssg::read_tree(&objects)?;
    let sources = mesh::read_static_meshes(&tree)?;
    let mut extents: BTreeMap<Vec<u8>, ([f32; 3], [f32; 3])> = BTreeMap::new();
    for d in scene::read_object_draws(&tree, &sources)? {
        let Some(prototype) = d.prototype else {
            continue;
        };
        let source = &sources[d.source];
        let e = extents
            .entry(prototype.to_vec())
            .or_insert(([f32::MAX; 3], [f32::MIN; 3]));
        for &i in &d.indices {
            let p = scene::transform_point(&d.transform, source.positions[i as usize]);
            for ((low, high), x) in e.0.iter_mut().zip(e.1.iter_mut()).zip(p) {
                *low = low.min(x);
                *high = high.max(x);
            }
        }
    }
    let placement_bytes = std::fs::read(root.join(route.file("ornaments.bin")))?;
    let placements = ornaments::read(&placement_bytes)?;
    let mut boxes = Vec::new();
    for p in &placements.prototypes {
        let name = String::from_utf8_lossy(p.name).into_owned();
        if !BARRIER_FAMILIES.iter().any(|f| name.starts_with(f)) {
            continue;
        }
        let Some((lo, hi)) = extents.get(format!("{name} Root").as_bytes()) else {
            continue;
        };
        if (0..3).any(|j| hi[j] <= lo[j] || hi[j] - lo[j] > 100. || !(hi[j] - lo[j]).is_finite()) {
            return Err("invalid barrier ornament extent".into());
        }
        for m in &p.transforms {
            let positions = (0..8)
                .map(|i| {
                    let local =
                        std::array::from_fn(|j| if i >> j & 1 == 0 { lo[j] } else { hi[j] });
                    scene::transform_point(m, local)
                })
                .collect();
            boxes.push(PropBox::new(name.clone(), positions)?);
        }
    }
    Ok(boxes)
}

impl Props {
    /// The default (Donington) route's props.
    pub fn read(root: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        Self::read_route(root, &Route::default())
    }
    pub fn read_route(root: &Path, route: &Route) -> Result<Self, Box<dyn std::error::Error>> {
        let read = |p: &str| -> Result<Vec<u8>, Box<dyn std::error::Error>> {
            let bytes = std::fs::read(root.join(p))?;
            if bytes.len() > 64 << 20 {
                return Err("entity source exceeds bound".into());
            }
            Ok(bytes)
        };
        let placements = read(&route.file("objects.ens"))?;
        let types = read(&route.track_file("objecttypes.pssg"))?;
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
                    let positions = (0..8)
                        .map(|i| {
                            let local = std::array::from_fn(|j| {
                                if i >> j & 1 == 0 { -half[j] } else { half[j] }
                            });
                            transform(&e.transform, transform(&s.transform, local))
                        })
                        .collect();
                    boxes.push(PropBox::new(
                        String::from_utf8_lossy(e.template).into_owned(),
                        positions,
                    )?);
                }
            }
        }
        boxes.extend(ornament_barriers(root, route)?);
        if boxes.is_empty() || boxes.len() > 8192 {
            return Err("unsupported entity collision box count".into());
        }
        Ok(Self {
            boxes,
            materials: vec![0; 12],
        })
    }

    /// Diagnostic: deepest penetration of any point inside any box, as
    /// (box index, depth to the nearest face).
    pub fn deepest_overlap(&self, points: &[[f32; 3]]) -> Option<(usize, f32)> {
        let mut best: Option<(usize, f32)> = None;
        for (index, b) in self.boxes.iter().enumerate() {
            let o = b.positions[0];
            let axes = [1, 2, 4].map(|k| {
                let p = b.positions[k];
                [p[0] - o[0], p[1] - o[1], p[2] - o[2]]
            });
            for p in points {
                if (0..3).any(|j| p[j] < b.bounds[j] || p[j] > b.bounds[j + 3]) {
                    continue;
                }
                let d = [p[0] - o[0], p[1] - o[1], p[2] - o[2]];
                let mut depth = f32::MAX;
                for a in &axes {
                    let length = (a[0] * a[0] + a[1] * a[1] + a[2] * a[2]).sqrt();
                    let u = (d[0] * a[0] + d[1] * a[1] + d[2] * a[2]) / length;
                    depth = depth.min(u).min(length - u);
                }
                if depth > 0. && best.is_none_or(|(_, x)| depth > x) {
                    best = Some((index, depth));
                }
            }
        }
        best
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
        self.contacts_known(mesh, matrix, query, bounds, capacity, None)
    }

    /// Body-query checks of every box, in box order.
    pub fn checked(&self) -> Vec<grid_world::packed_contacts::Checked> {
        self.boxes
            .iter()
            .map(|b| grid_world::packed_contacts::check(&b.mesh(), &self.materials))
            .collect()
    }

    /// [`Self::contacts`] reusing values the caller holds for these unchanged
    /// boxes ([`KnownBoxes`]); `bounds` must then be
    /// `mesh.world_bounds(matrix, query.margin)`. Identical results.
    pub fn contacts_known(
        &self,
        mesh: &grid_world::collision_mesh::Mesh,
        matrix: grid_world::physical_pose::Matrix,
        query: &grid_world::convex_distance::ContactSettings,
        bounds: [[f32; 4]; 2],
        capacity: usize,
        known: Option<KnownBoxes<'_>>,
    ) -> Result<Vec<grid_world::contact_manifold::Contact>, Error> {
        if known.as_ref().is_some_and(|k| {
            k.checked.len() != self.boxes.len() || k.index.boxes != self.boxes.len()
        }) {
            return Err(Error("prop box checks differ from the boxes"));
        }
        // Boxes outside the index candidates cannot overlap the bounds and
        // would be skipped; candidates keep data order.
        let candidates = match &known {
            Some(k) => k.index.candidates(bounds),
            None => (0..self.boxes.len()).collect(),
        };
        let mut contacts = Vec::new();
        for index in candidates {
            let b = &self.boxes[index];
            if contacts.len() >= capacity {
                break;
            }
            if (0..3).any(|j| bounds[0][j] > b.bounds[j + 3] || bounds[1][j] < b.bounds[j]) {
                continue;
            }
            let settings = grid_world::packed_contacts::Settings {
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
            };
            let (track, materials) = (&b.mesh(), &self.materials);
            let generated = match &known {
                Some(k) => grid_world::packed_contacts::calculate_decoded_known(
                    mesh,
                    matrix,
                    track,
                    materials,
                    &settings,
                    &grid_world::packed_contacts::Known {
                        checked: k.checked[index],
                        directions: k.directions,
                        bounds,
                    },
                )?,
                None => {
                    grid_world::packed_contacts::calculate_decoded(
                        mesh, matrix, track, materials, &settings,
                    )?
                    .contacts
                }
            };
            contacts.extend(generated);
        }
        Ok(contacts)
    }
}

/// Values a caller holds for repeated body queries of unchanged boxes:
/// [`Props::checked`], `directions_valid(query)` and a [`BoxIndex`] of them.
pub struct KnownBoxes<'a> {
    pub checked: &'a [grid_world::packed_contacts::Checked],
    pub directions: bool,
    pub index: &'a BoxIndex,
}

/// Uniform horizontal grid over box bounds: every box overlapping a query's
/// x/z bounds is in a cell the query covers.
pub struct BoxIndex {
    boxes: usize,
    origin: [f32; 2],
    dimensions: [usize; 2],
    cells: Vec<Vec<u32>>,
}

const INDEX_CELL: f32 = 16.;

impl BoxIndex {
    pub fn new(boxes: &[PropBox]) -> Self {
        let mut low = [f32::MAX; 2];
        let mut high = [f32::MIN; 2];
        for b in boxes {
            for (k, j) in [0, 2].into_iter().enumerate() {
                low[k] = low[k].min(b.bounds[j]);
                high[k] = high[k].max(b.bounds[j + 3]);
            }
        }
        let finite = low.iter().chain(&high).all(|x| x.is_finite()) && !boxes.is_empty();
        let origin = if finite { low } else { [0.; 2] };
        let dimensions = std::array::from_fn(|k| {
            if finite {
                (((high[k] - low[k]) / INDEX_CELL) as usize + 1).min(1024)
            } else {
                1
            }
        });
        let mut index = Self {
            boxes: boxes.len(),
            origin,
            dimensions,
            cells: vec![Vec::new(); dimensions[0] * dimensions[1]],
        };
        for (i, b) in boxes.iter().enumerate() {
            let [x, z] = index.range([b.bounds[0], b.bounds[2]], [b.bounds[3], b.bounds[5]]);
            for cx in x[0]..=x[1] {
                for cz in z[0]..=z[1] {
                    index.cells[cx * dimensions[1] + cz].push(i as u32);
                }
            }
        }
        index
    }

    /// Inclusive cell ranges per axis, clamped to the grid; non-finite
    /// bounds cover the whole grid.
    fn range(&self, low: [f32; 2], high: [f32; 2]) -> [[usize; 2]; 2] {
        std::array::from_fn(|k| {
            let cell = |v: f32| {
                ((v - self.origin[k]) / INDEX_CELL)
                    .floor()
                    .clamp(0., (self.dimensions[k] - 1) as f32) as usize
            };
            if low[k].is_finite() && high[k].is_finite() {
                [cell(low[k]), cell(high[k])]
            } else {
                [0, self.dimensions[k] - 1]
            }
        })
    }

    /// Ascending box indices that may overlap `bounds` horizontally.
    pub fn candidates(&self, bounds: [[f32; 4]; 2]) -> Vec<usize> {
        let [x, z] = self.range([bounds[0][0], bounds[0][2]], [bounds[1][0], bounds[1][2]]);
        let mut out: Vec<usize> = Vec::new();
        for cx in x[0]..=x[1] {
            for cz in z[0]..=z[1] {
                out.extend(
                    self.cells[cx * self.dimensions[1] + cz]
                        .iter()
                        .map(|&i| i as usize),
                );
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_index_candidates_cover_every_overlap_in_order() {
        // Deterministic pseudo-random boxes over a 600 m square, one with NaN bounds.
        let mut seed = 0x1234_5678u32;
        let mut next = || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 8) as f32 / (1 << 24) as f32
        };
        let mut boxes: Vec<PropBox> = (0..400)
            .map(|_| {
                let (x, z, w) = (next() * 600. - 300., next() * 600. - 300., next() * 40.);
                PropBox {
                    template: String::new(),
                    bounds: [x, -1., z, x + w, 2., z + w * 0.5],
                    positions: Vec::new(),
                    triangles: Vec::new(),
                    nodes: Vec::new(),
                    leaves: BTreeMap::new(),
                }
            })
            .collect();
        boxes[7].bounds[3] = f32::NAN;
        let index = BoxIndex::new(&boxes);
        let mut queries: Vec<[[f32; 4]; 2]> = (0..2000)
            .map(|_| {
                let (x, z, w) = (next() * 700. - 350., next() * 700. - 350., next() * 30.);
                [[x, 0., z, 1.], [x + w, 1., z + w, 1.]]
            })
            .collect();
        // Non-finite query bounds are never culled on that axis.
        queries.push([[f32::NAN, 0., 0., 1.], [0., 0., 0., 1.]]);
        queries.push([[0., 0., f32::NEG_INFINITY, 1.], [5., 0., 0., 1.]]);
        for bounds in queries {
            let candidates = index.candidates(bounds);
            assert!(candidates.windows(2).all(|p| p[0] < p[1]));
            for (i, b) in boxes.iter().enumerate() {
                let culled =
                    (0..3).any(|j| bounds[0][j] > b.bounds[j + 3] || bounds[1][j] < b.bounds[j]);
                assert!(culled || candidates.contains(&i), "box {i} missing");
            }
        }
    }
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
        let entity_boxes: usize = templates
            .iter()
            .filter(|(t, _)| t.ends_with(".max"))
            .map(|(_, n)| n)
            .sum();
        assert_eq!(entity_boxes, 1021);
        // Barrier ornaments without entity counterparts, e.g. concrete blocks.
        assert_eq!(templates.get("concrete_block_long"), Some(&264));
        assert_eq!(props.boxes.len(), 2066);
        assert!(
            !templates.contains_key("tyre_wall"),
            "no duplicated entity walls"
        );
    }
}
