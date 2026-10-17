//! Static collision boxes of the purchased route entities (objects.ens with
//! objecttypes.pssg): tyre walls, cushions, armco, tyre stacks and marker boards.
//! Each box is a box primitive (shape kind 0) queried as a whole against a
//! body hull, as the original's AE8AD0 does (`grid_world::box_contacts`).
//! All placements start infinite-mass in the data; breakable boxes carry a
//! contact limit and are disabled when broken (`dynamic_props`,
//! DYNAMIC_PROPS_RESEARCH.md).
use crate::route::Route;
use grid_formats::{entities, pssg};
use grid_world::{Error, box_contacts::BoxShape};
use std::{collections::BTreeMap, path::Path};

pub struct PropBox {
    pub template: String,
    pub bounds: [f32; 6],
    pub positions: Vec<[f32; 3]>,
    /// Entity placement index (objects.ens order) for entity boxes.
    pub entity: Option<usize>,
    /// Contact impulse limit written to contact +0x3C: the shape behaviour's
    /// threshold for an armed breakable box, otherwise FLT_MAX.
    pub limit: f32,
    /// Broken boxes no longer collide.
    pub enabled: bool,
    /// The primitive: axes, centre, half extents and the shape `shell` margin.
    shape: BoxShape,
}

pub struct Props {
    pub boxes: Vec<PropBox>,
}

/// Row-vector placement: translation in elements 12..14.
fn transform(m: &[f32; 16], p: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|j| p[0] * m[j] + p[1] * m[4 + j] + p[2] * m[8 + j] + m[12 + j])
}

impl PropBox {
    /// Eight world corners indexed by bits (x, y, z) of the local box, and
    /// the shape margin; `bounds` include the margin.
    fn new(
        template: String,
        positions: Vec<[f32; 3]>,
        margin: f32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if positions.len() != 8
            || positions.iter().flatten().any(|x| !x.is_finite())
            || !(0. ..=1.).contains(&margin)
        {
            return Err("invalid route collision box".into());
        }
        let mut bounds = [f32::MAX, f32::MAX, f32::MAX, f32::MIN, f32::MIN, f32::MIN];
        for p in &positions {
            for j in 0..3 {
                bounds[j] = bounds[j].min(p[j] - margin);
                bounds[j + 3] = bounds[j + 3].max(p[j] + margin);
            }
        }
        let o = positions[0];
        let mut matrix = [[0.; 4]; 4];
        let mut half = [0.; 3];
        for k in 0..3 {
            let p = positions[1 << k];
            let edge = [p[0] - o[0], p[1] - o[1], p[2] - o[2]];
            let length = (edge[0] * edge[0] + edge[1] * edge[1] + edge[2] * edge[2]).sqrt();
            if !(length > 0. && length < 200.) {
                return Err("invalid route collision box".into());
            }
            matrix[k] = [edge[0] / length, edge[1] / length, edge[2] / length, 0.];
            half[k] = length * 0.5;
        }
        let far = positions[7];
        matrix[3] = [
            (o[0] + far[0]) * 0.5,
            (o[1] + far[1]) * 0.5,
            (o[2] + far[2]) * 0.5,
            1.,
        ];
        Ok(Self {
            template,
            bounds,
            positions,
            entity: None,
            limit: f32::MAX,
            enabled: true,
            shape: BoxShape {
                matrix,
                half,
                margin,
            },
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
/// AUTHORED tolerances of [`barrier_extent`]: a part within this height of the
/// prototype's lowest point stands on the ground; a part starting within it
/// of the base's top is mounted on the base.
const BASE_TOLERANCE: f32 = 0.05;
/// AUTHORED: a mounted part reaching this far past the base footprint leans out.
const OVERHANG_TOLERANCE: f32 = 0.01;

/// Lowest and highest corner of an axis-aligned extent.
type Extent = ([f32; 3], [f32; 3]);

/// Mesh points (prototype space, Y up) and triangles of one prototype.
#[derive(Default)]
struct PrototypeMesh {
    points: Vec<[f32; 3]>,
    triangles: Vec<[usize; 3]>,
}

/// The stand-in extent of a barrier prototype: its mesh extent without the
/// parts mounted on top of the base that lean out past the base's footprint.
/// Long Beach's `concrete_fence_*` carry a catch fence whose posts and panels
/// start on the 1.0 m concrete wall and lean 0.63 m out over the track at
/// 3.4-4.1 m; with them the box stood 0.63 m in front of the visible concrete
/// at road level, where the tyre stacks stand, so their pieces spawned up to
/// 0.76 m inside it (docs/LONG_BEACH.md section 5). Prototypes without such
/// parts (every barrier of the other circuits) keep their whole extent.
fn barrier_extent(mesh: &PrototypeMesh) -> Extent {
    // Connected parts by welded position (1 mm), across draws.
    let mut welded = BTreeMap::new();
    let ids: Vec<usize> = (mesh.points.iter())
        .map(|p| {
            let n = welded.len();
            *welded
                .entry(p.map(|x| (x * 1000.).round() as i64))
                .or_insert(n)
        })
        .collect();
    let mut parent: Vec<usize> = (0..welded.len()).collect();
    fn find(parent: &mut [usize], mut x: usize) -> usize {
        while parent[x] != x {
            parent[x] = parent[parent[x]];
            x = parent[x];
        }
        x
    }
    for t in &mesh.triangles {
        let a = find(&mut parent, ids[t[0]]);
        for &k in &t[1..] {
            let b = find(&mut parent, ids[k]);
            parent[b] = a;
        }
    }
    let mut parts: BTreeMap<usize, Extent> = BTreeMap::new();
    for (i, p) in mesh.points.iter().enumerate() {
        let e = parts
            .entry(find(&mut parent, ids[i]))
            .or_insert(([f32::MAX; 3], [f32::MIN; 3]));
        *e = (
            std::array::from_fn(|j| e.0[j].min(p[j])),
            std::array::from_fn(|j| e.1[j].max(p[j])),
        );
    }
    let union = |keep: &dyn Fn(&Extent) -> bool| {
        parts
            .values()
            .filter(|e| keep(e))
            .fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), e| {
                (
                    std::array::from_fn(|j| lo[j].min(e.0[j])),
                    std::array::from_fn(|j| hi[j].max(e.1[j])),
                )
            })
    };
    let whole = union(&|_| true);
    // Across the barrier: the shorter horizontal axis of the whole mesh.
    let across = if whole.1[0] - whole.0[0] <= whole.1[2] - whole.0[2] {
        0
    } else {
        2
    };
    let base = union(&|e| e.0[1] <= whole.0[1] + BASE_TOLERANCE);
    let mounted_overhang = |e: &Extent| {
        e.0[1] >= base.1[1] - BASE_TOLERANCE
            && (e.0[across] < base.0[across] - OVERHANG_TOLERANCE
                || e.1[across] > base.1[across] + OVERHANG_TOLERANCE)
    };
    union(&|e| !mounted_overhang(e))
}

/// Closest-LOD meshes of the barrier family prototypes, by root node name.
fn barrier_meshes(
    root: &Path,
    route: &Route,
) -> Result<BTreeMap<Vec<u8>, PrototypeMesh>, Box<dyn std::error::Error>> {
    use grid_formats::{mesh, scene};
    let objects = std::fs::read(root.join(route.track_file("objects.pssg")))?;
    let tree = pssg::read_tree(&objects)?;
    let sources = mesh::read_static_meshes(&tree)?;
    let mut meshes: BTreeMap<Vec<u8>, PrototypeMesh> = BTreeMap::new();
    for d in scene::read_object_draws(&tree, &sources)? {
        let Some(prototype) = d.prototype else {
            continue;
        };
        if !BARRIER_FAMILIES
            .iter()
            .any(|f| prototype.starts_with(f.as_bytes()))
        {
            continue;
        }
        let source = &sources[d.source];
        let m = meshes.entry(prototype.to_vec()).or_default();
        let mut local = BTreeMap::new();
        let mut index = |i: u16| {
            *local.entry(i).or_insert_with(|| {
                m.points.push(scene::transform_point(
                    &d.transform,
                    source.positions[i as usize],
                ));
                m.points.len() - 1
            })
        };
        let triangles: Vec<[usize; 3]> = (d.indices.as_chunks::<3>().0.iter())
            .map(|t| [index(t[0]), index(t[1]), index(t[2])])
            .collect();
        m.triangles.extend(triangles);
    }
    Ok(meshes)
}

fn ornament_barriers(
    root: &Path,
    route: &Route,
) -> Result<Vec<PropBox>, Box<dyn std::error::Error>> {
    use grid_formats::{ornaments, scene};
    let meshes = barrier_meshes(root, route)?;
    let placement_bytes = std::fs::read(root.join(route.file("ornaments.bin")))?;
    let placements = ornaments::read(&placement_bytes)?;
    let mut boxes = Vec::new();
    for p in &placements.prototypes {
        let name = String::from_utf8_lossy(p.name).into_owned();
        if !BARRIER_FAMILIES.iter().any(|f| name.starts_with(f)) {
            continue;
        }
        let Some((lo, hi)) = meshes
            .get(format!("{name} Root").as_bytes())
            .map(barrier_extent)
        else {
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
            boxes.push(PropBox::new(name.clone(), positions, 0.)?);
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
        for (index, e) in entities::read(&placements, &types)?.into_iter().enumerate() {
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
                    let mut b = PropBox::new(
                        String::from_utf8_lossy(e.template).into_owned(),
                        positions,
                        s.shell,
                    )?;
                    b.entity = Some(index);
                    boxes.push(b);
                }
            }
        }
        boxes.extend(ornament_barriers(root, route)?);
        if boxes.is_empty() || boxes.len() > 8192 {
            return Err("unsupported entity collision box count".into());
        }
        Ok(Self { boxes })
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

    /// Contacts against every enabled box whose bounds overlap the body's
    /// margin-inflated world bounds, in data order, up to `capacity` (<=16 per
    /// box), each with its box index.
    pub fn contacts(
        &self,
        mesh: &grid_world::collision_mesh::Mesh,
        matrix: grid_world::physical_pose::Matrix,
        query: &grid_world::convex_distance::ContactSettings,
        bounds: [[f32; 4]; 2],
        capacity: usize,
    ) -> Result<Vec<(usize, grid_world::contact_manifold::Contact)>, Error> {
        self.contacts_known(mesh, matrix, query, bounds, capacity, None)
    }

    /// [`Self::contacts`] with a [`BoxIndex`] the caller holds for these
    /// boxes; `bounds` must be `mesh.world_bounds(matrix, query.margin)`.
    /// Identical results.
    pub fn contacts_known(
        &self,
        mesh: &grid_world::collision_mesh::Mesh,
        matrix: grid_world::physical_pose::Matrix,
        query: &grid_world::convex_distance::ContactSettings,
        bounds: [[f32; 4]; 2],
        capacity: usize,
        known: Option<KnownBoxes<'_>>,
    ) -> Result<Vec<(usize, grid_world::contact_manifold::Contact)>, Error> {
        if known
            .as_ref()
            .is_some_and(|k| k.index.boxes != self.boxes.len())
        {
            return Err(Error("prop box index differs from the boxes"));
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
            if !b.enabled
                || (0..3).any(|j| bounds[0][j] > b.bounds[j + 3] || bounds[1][j] < b.bounds[j])
            {
                continue;
            }
            let generated = grid_world::box_contacts::contacts(
                mesh,
                matrix,
                &b.shape,
                query,
                (capacity - contacts.len()).min(16),
            )?;
            contacts.extend(generated.into_iter().map(|c| (index, c)));
        }
        Ok(contacts)
    }
}

/// Values a caller holds for repeated body queries of unchanged boxes: a
/// [`BoxIndex`] of them.
pub struct KnownBoxes<'a> {
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
                    entity: None,
                    limit: f32::MAX,
                    enabled: true,
                    shape: BoxShape {
                        matrix: [[0.; 4]; 4],
                        half: [1.; 3],
                        margin: 0.,
                    },
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

    /// Soak seed 1087 (F342 build, both routes): the player hit a tyre wall
    /// at 39-45 m/s 25 m off the line. Queried triangle by triangle, the
    /// wall's box gave contacts with downward normals (up to 0.9 m deep),
    /// which pushed the car through the road; it then fell out of the world.
    /// As one box primitive (AE8AD0) every contact pushes up or sideways.
    /// A wall with a catch fence: a 0.42 m base on the ground, a post on it
    /// leaning 0.63 m out across the wall, and a fence plane above the base.
    fn fenced_wall(lean: f32) -> PrototypeMesh {
        let mut mesh = PrototypeMesh::default();
        let mut add = |lo: [f32; 3], hi: [f32; 3]| {
            let base = mesh.points.len();
            mesh.points.extend(
                (0..8)
                    .map(|i| std::array::from_fn(|j| if i >> j & 1 == 0 { lo[j] } else { hi[j] })),
            );
            // Two triangles per box connect all eight corners.
            mesh.triangles
                .extend([[0, 1, 3], [3, 2, 7], [7, 6, 4], [4, 5, 1]].map(|t| t.map(|k| base + k)));
        };
        add([0., 0., -0.22], [4., 1., 0.2]);
        add([0.1, 0.98, -0.03], [0.2, 4.09, lean]);
        add([0.2, 1.1, 0.04], [3.9, 3.1, 0.06]);
        mesh
    }

    #[test]
    fn a_catch_fence_leaning_out_is_not_part_of_the_stand_in() {
        // Long Beach concrete_fence_*: the post leans out across the wall.
        let (lo, hi) = barrier_extent(&fenced_wall(0.83));
        assert_eq!((lo, hi), ([0., 0., -0.22], [4., 3.1, 0.2]));
        // A post that stays over the base is kept, and so is the whole mesh.
        let (lo, hi) = barrier_extent(&fenced_wall(0.2));
        assert_eq!((lo, hi), ([0., 0., -0.22], [4., 4.09, 0.2]));
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn only_long_beach_fences_lose_their_leaning_catch_fence() {
        // Long Beach soak seeds 1..12 and 17..20 lost tyre-stack pieces through
        // the road: with the leaning catch fence the stand-in reached 0.63 m in
        // front of the concrete, and the pieces spawned inside it.
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        for path in [
            crate::route::DONINGTON,
            crate::route::NURBURGRING,
            crate::route::ISTANBUL_PARK,
            crate::route::LONG_BEACH,
        ] {
            let route = Route::new(path).unwrap();
            let meshes = barrier_meshes(root, &route).unwrap();
            assert!(!meshes.is_empty());
            for (name, mesh) in meshes {
                let name = String::from_utf8_lossy(&name).into_owned();
                let whole =
                    mesh.points
                        .iter()
                        .fold(([f32::MAX; 3], [f32::MIN; 3]), |(lo, hi), p| {
                            (
                                std::array::from_fn(|j| lo[j].min(p[j])),
                                std::array::from_fn(|j| hi[j].max(p[j])),
                            )
                        });
                let (lo, hi) = barrier_extent(&mesh);
                if path == crate::route::LONG_BEACH && name.starts_with("concrete_fence_") {
                    // The concrete wall's front face, not the fence top (0.83).
                    assert!(whole.1[2] > 0.8, "{name}");
                    assert!((0.19..0.2).contains(&hi[2]), "{name} {hi:?}");
                    assert_eq!((lo[0], lo[1], hi[0]), (whole.0[0], whole.0[1], whole.1[0]));
                } else {
                    assert_eq!((lo, hi), whole, "{path} {name}");
                }
            }
        }
    }

    /// The poses are the recorded hull matrices of the failing replays.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and GRID_CONTACT_DIRECTIONS"]
    fn a_car_deep_in_a_tyre_wall_is_never_pushed_down() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let table = std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
        let cases: [(&str, usize, [[f32; 4]; 4]); 2] = [
            (
                // Nuerburgring tyrewall_black_yok, replay tick 2239 (0.9 m deep).
                "tracks/europe/nurburgring/route_0",
                1508,
                [
                    [-0.093455344, -0.023044754, 0.99535704, 0.0],
                    [-0.00970464, 0.9997056, 0.022234295, 0.0],
                    [-0.9955721, -0.007581843, -0.09364965, 0.0],
                    [-283.1356, 607.23224, 28.229782, 1.0],
                ],
            ),
            (
                // Donington tyre_wall, an earlier hit of the same run.
                "tracks/europe/donington/route_0",
                2,
                [
                    [-0.71347827, 0.1078137, 0.69231504, 0.0],
                    [-0.061344933, 0.97467995, -0.21500538, 0.0],
                    [-0.69796693, -0.19587311, -0.68880314, 0.0],
                    [343.10107, -25.791117, -124.746376, 1.0],
                ],
            ),
        ];
        for (route, wall, pose) in cases {
            let route = Route::new(route).unwrap();
            let mut runtime =
                crate::source_vehicle::Runtime::lacetti_on(root, &route, "slot_00", 200679)
                    .unwrap();
            let geometry = runtime.prepare_body_geometry(root, &table).unwrap();
            let props = runtime.track().props().unwrap();
            assert!(props.boxes[wall].template.starts_with("tyre"));
            let bounds = geometry
                .mesh()
                .world_bounds(pose, geometry.query().margin)
                .unwrap();
            let hits: Vec<_> = props
                .contacts(geometry.mesh(), pose, geometry.query(), bounds, 64)
                .unwrap()
                .into_iter()
                .filter(|(b, _)| *b == wall)
                .collect();
            eprintln!("{route:?} wall {wall}: {hits:?}");
            assert!(!hits.is_empty());
            for (_, c) in &hits {
                assert!(c.normal[1] > -0.1, "{c:?}");
            }
        }
    }
}
