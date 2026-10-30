//! Static collision shapes of the purchased route entities (objects.ens with
//! objecttypes.pssg): tyre walls, cushions, armco, tyre stacks and marker
//! boards (box primitives, kind 0), cones, posts, poles and barrels
//! (cylinders, kind 5) and balloons (spheres, kind 3). Each is queried as a
//! whole against a body hull, as the original's AE8AD0 does
//! (`grid_world::box_contacts`, `grid_world::round_contacts`).
//! All placements start infinite-mass in the data; breakable boxes carry a
//! contact limit and are disabled when broken (`dynamic_props`,
//! DYNAMIC_PROPS_RESEARCH.md).
//! The basic entities of the same file (`TEMPLATEBASICENTITYINSTANCE`:
//! concrete blocks and walls, chain and catch fences, armco runs, ad boards,
//! buildings, signs) add their templates' shapes as fixed boxes, as the
//! original's level setup does (0x7CB440; docs/BARRIER_COLLISION.md). They
//! replace F284's authored ornament stand-ins.
use crate::route::Route;
use grid_formats::{entities, pssg};
use grid_world::{
    Error,
    box_contacts::BoxShape,
    round_contacts::{CylinderShape, RoundShape, SphereShape},
};
use std::path::Path;

#[derive(Clone)]
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
    /// A static (`infMass`) shape of the entity's damage state: off until
    /// the entity breaks (`dynamic_props`; the close-drift flag's base).
    pub damaged: bool,
    /// The primitive: axes, centre, extents and the shape `shell` margin.
    shape: Shape,
}

#[derive(Clone)]
enum Shape {
    Box(BoxShape),
    Round(RoundShape),
}

/// Shared between clones until one changes a box (`Arc::make_mut`).
#[derive(Clone)]
pub struct Props {
    pub boxes: std::sync::Arc<Vec<PropBox>>,
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
            if !(length > 0. && length < MAX_EDGE) {
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
            damaged: false,
            shape: Shape::Box(BoxShape {
                matrix,
                half,
                margin,
            }),
        })
    }

    /// A cylinder or sphere at `matrix` (unit axes, centre): `positions`
    /// are the corners of its local bounding box (for bounds and the
    /// overlap diagnostic).
    fn round(template: String, shape: RoundShape) -> Result<Self, Box<dyn std::error::Error>> {
        let (matrix, half, margin) = match &shape {
            RoundShape::Cylinder(c) => (c.matrix, [c.radius, c.half_height, c.radius], c.margin),
            RoundShape::Sphere(s) => (s.matrix, [s.radius; 3], s.margin),
        };
        let positions = (0..8)
            .map(|i| {
                std::array::from_fn(|j| {
                    (0..3).fold(matrix[3][j], |p, k| {
                        let h = if i >> k & 1 == 0 { -half[k] } else { half[k] };
                        p + h * matrix[k][j]
                    })
                })
            })
            .collect();
        let mut b = Self::new(template, positions, margin)?;
        b.shape = Shape::Round(shape);
        Ok(b)
    }
}

/// Whether a template's damage state with static (`infMass`) pieces, or a
/// body with several shape behaviours, is ported: so far only the close-drift
/// flags. Others (the Long Beach and Spa tyre walls, Yokohama's crates and
/// signs) stay intact until their breaks are checked.
pub fn static_damage_ported(template: &str) -> bool {
    template.contains("close_drift_flag")
}

/// The collision box of one entity shape at the entity placement.
fn entity_box(
    template: String,
    placement: &[f32; 16],
    s: &entities::Shape<'_>,
) -> Result<PropBox, Box<dyn std::error::Error>> {
    let (width, height, length) = match s.primitive {
        entities::Primitive::Box {
            width,
            height,
            length,
        } => (width, height, length),
        entities::Primitive::Cylinder { radius, height } => {
            return PropBox::round(
                template,
                round_shape(placement, &s.transform, s.shell, radius, Some(height))?,
            );
        }
        entities::Primitive::Sphere { radius } => {
            return PropBox::round(
                template,
                round_shape(placement, &s.transform, s.shell, radius, None)?,
            );
        }
    };
    let half = [width * 0.5, height * 0.5, length * 0.5];
    if !half.iter().all(|h| h.is_finite() && *h > 0. && *h < MAX_EDGE * 0.5) {
        return Err("invalid entity collision box".into());
    }
    let positions = (0..8)
        .map(|i| {
            let local = std::array::from_fn(|j| {
                if i >> j & 1 == 0 { -half[j] } else { half[j] }
            });
            transform(placement, transform(&s.transform, local))
        })
        .collect();
    PropBox::new(template, positions, s.shell)
}

/// AUTHORED sanity bound on a collision box edge (m): the longest source
/// shape is Shibuya's `bridge_glass_c` deck, 315 m.
const MAX_EDGE: f32 = 1000.;

/// Bound on a route's prop collision boxes: the original's fixed-object pool
/// (10,000, AD9220; docs/RUNTIME_LIMITS_AUDIT.md).
const MAX_BOXES: usize = 10_000;

/// The collision boxes of one placement's initial components, without
/// phantom shapes; `entity` is the entity placement index, or none for a
/// basic entity.
fn placement_boxes(
    e: &entities::Entity<'_>,
    entity: Option<usize>,
    boxes: &mut Vec<PropBox>,
) -> Result<(), Box<dyn std::error::Error>> {
    let template = String::from_utf8_lossy(e.template).into_owned();
    for r in &e.renderables {
        for s in r.shapes.iter().filter(|s| !s.phantom) {
            let mut b = entity_box(template.clone(), &e.transform, s)?;
            b.entity = entity;
            boxes.push(b);
        }
    }
    Ok(())
}

impl Props {
    /// The default (Donington) route's props.
    pub fn read(root: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        Self::read_route(root, &Route::default())
    }
    pub fn read_route(root: &Path, route: &Route) -> Result<Self, Box<dyn std::error::Error>> {
        let read = |p: &str| -> Result<Vec<u8>, Box<dyn std::error::Error>> {
            let bytes = std::fs::read(root.join(p)).map_err(|e| format!("{p}: {e}"))?;
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
        // Kept after every other box, so the active boxes keep their indices.
        let mut damaged = Vec::new();
        for (index, e) in entities::read(&placements, &types)?.into_iter().enumerate() {
            placement_boxes(&e, Some(index), &mut boxes)?;
            let template = String::from_utf8_lossy(e.template).into_owned();
            // The static pieces of the damage state, once per behaviour type.
            let mut types = Vec::new();
            if !static_damage_ported(&template) {
                continue;
            }
            for s in e.renderables.iter().flat_map(|r| &r.shapes) {
                let Some(behaviour) = &s.behaviour else { continue };
                if types.contains(&behaviour.behaviour_type) {
                    continue;
                }
                types.push(behaviour.behaviour_type);
                for added in behaviour.added.iter().filter(|a| a.infinite_mass) {
                    for s in added.shapes.iter().filter(|s| !s.phantom) {
                        let mut b = entity_box(template.clone(), &e.transform, s)?;
                        b.entity = Some(index);
                        b.enabled = false;
                        b.damaged = true;
                        damaged.push(b);
                    }
                }
            }
        }
        // Basic entities: the fixed barriers, fences, walls, buildings and
        // signs whose meshes `ornaments.bin` draws. Their shapes stay intact
        // (their damage behaviours, e.g. Le Mans armco, are not ported).
        for e in entities::read_basic(&placements, &types)? {
            placement_boxes(&e, None, &mut boxes)?;
        }
        boxes.extend(damaged);
        if boxes.is_empty() || boxes.len() > MAX_BOXES {
            return Err(format!("unsupported entity collision box count {}", boxes.len()).into());
        }
        Ok(Self {
            boxes: std::sync::Arc::new(boxes),
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

    /// Contacts against every enabled box whose bounds overlap the body's
    /// margin-inflated world bounds, in data order, up to `capacity` (<=16 per
    /// box), each with its box index.
    pub fn contacts<'m>(
        &self,
        mesh: impl Into<grid_world::convex_distance::Hull<'m>>,
        matrix: grid_world::physical_pose::Matrix,
        query: &grid_world::convex_distance::ContactSettings,
        bounds: [[f32; 4]; 2],
        capacity: usize,
    ) -> Result<Vec<(usize, grid_world::contact_manifold::Contact)>, Error> {
        let mesh = mesh.into();
        self.contacts_known(mesh, matrix, query, bounds, capacity, None)
    }

    /// [`Self::contacts`] with a [`BoxIndex`] the caller holds for these
    /// boxes; `bounds` must be `mesh.world_bounds(matrix, query.margin)`.
    /// Identical results.
    pub fn contacts_known<'m>(
        &self,
        mesh: impl Into<grid_world::convex_distance::Hull<'m>>,
        matrix: grid_world::physical_pose::Matrix,
        query: &grid_world::convex_distance::ContactSettings,
        bounds: [[f32; 4]; 2],
        capacity: usize,
        known: Option<KnownBoxes<'_>>,
    ) -> Result<Vec<(usize, grid_world::contact_manifold::Contact)>, Error> {
        let mesh = mesh.into();
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
            let room = (capacity - contacts.len()).min(16);
            let generated = match &b.shape {
                Shape::Box(shape) => {
                    grid_world::box_contacts::contacts(mesh, matrix, shape, query, room)?
                }
                Shape::Round(shape) => {
                    grid_world::round_contacts::contacts(mesh, matrix, shape, query, room)?
                }
            };
            contacts.extend(generated.into_iter().map(|c| (index, c)));
        }
        Ok(contacts)
    }
}

/// A cylinder (`height` given) or sphere primitive placed by the shape and
/// entity transforms. 0x7B96D0 scales the template radius by the smaller
/// horizontal scale and the height by the vertical one, and stores half the
/// height; the axes are normalised (a sphere takes its smallest scale).
fn round_shape(
    entity: &[f32; 16],
    shape: &[f32; 16],
    shell: f32,
    radius: f32,
    height: Option<f32>,
) -> Result<RoundShape, Box<dyn std::error::Error>> {
    let origin = transform(entity, transform(shape, [0.; 3]));
    let mut matrix = [[0.; 4]; 4];
    let mut scale = [0.; 3];
    for k in 0..3 {
        let mut unit = [0.; 3];
        unit[k] = 1.;
        let p = transform(entity, transform(shape, unit));
        let axis = [p[0] - origin[0], p[1] - origin[1], p[2] - origin[2]];
        scale[k] = (axis[0] * axis[0] + axis[1] * axis[1] + axis[2] * axis[2]).sqrt();
        if !(scale[k].is_finite() && scale[k] > 1e-3) {
            return Err("degenerate entity round shape".into());
        }
        matrix[k] = [
            axis[0] / scale[k],
            axis[1] / scale[k],
            axis[2] / scale[k],
            0.,
        ];
    }
    matrix[3] = [origin[0], origin[1], origin[2], 1.];
    let valid = |v: f32| v.is_finite() && v > 0. && v < 100.;
    if !valid(radius) || height.is_some_and(|h| !valid(h)) || !(0. ..=1.).contains(&shell) {
        return Err("invalid entity round shape".into());
    }
    Ok(match height {
        Some(height) => RoundShape::Cylinder(CylinderShape {
            matrix,
            radius: radius * scale[0].min(scale[2]),
            half_height: height * scale[1] * 0.5,
            margin: shell,
        }),
        None => RoundShape::Sphere(SphereShape {
            matrix,
            radius: radius * scale[0].min(scale[1]).min(scale[2]),
            margin: shell,
        }),
    })
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
    use std::collections::BTreeMap;

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
                    damaged: false,
                    shape: Shape::Box(BoxShape {
                        matrix: [[0.; 4]; 4],
                        half: [1.; 3],
                        margin: 0.,
                    }),
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
        for b in props.boxes.iter() {
            *templates.entry(b.template.as_str()).or_default() += 1;
            assert!((0..3).all(|j| b.bounds[j] < b.bounds[j + 3]));
        }
        eprintln!(
            "route_props boxes={} templates={templates:?}",
            props.boxes.len()
        );
        let entity_boxes = props.boxes.iter().filter(|b| b.entity.is_some()).count();
        assert_eq!(entity_boxes, 1021);
        // Basic entities (fences, concrete blocks, walls, ad boards), each
        // shape of its template once per placement.
        assert_eq!(templates.get("concrete_block_long.max"), Some(&264));
        assert_eq!(props.boxes.len(), 1021 + 972);
        assert!(
            !templates.contains_key("tyre_wall"),
            "no duplicated entity walls"
        );
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn basic_entities_leave_every_racing_line_open() {
        // Before F284's stand-ins were replaced, Washington's chnfen_e_group3b
        // and group4 layouts (one ornament placement each) became a box
        // across route 0's line at (833, 7, 834). The basic-entity shapes
        // are per placement and never stand on a racing line.
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        for path in crate::route::CIRCUITS {
            let route = Route::new(path).unwrap();
            let props = Props::read_route(root, &route).unwrap();
            assert!(props.boxes.len() <= MAX_BOXES, "{path} {}", props.boxes.len());
            let basic: Vec<PropBox> =
                props.boxes.iter().filter(|b| b.entity.is_none()).cloned().collect();
            assert!(basic.len() > 300, "{path} {}", basic.len());
            let basic = Props {
                boxes: std::sync::Arc::new(basic),
            };
            let line = crate::ai_driver::Line::load_route(root, &route).unwrap();
            // The line every 0.5 m or so, 0.5 m up.
            let points = line.points();
            let lifted: Vec<[f32; 3]> = points
                .iter()
                .zip(points.iter().cycle().skip(1))
                .flat_map(|(a, b)| {
                    (0..20).map(move |k| {
                        let t = k as f32 / 20.;
                        std::array::from_fn(|j| {
                            a[j] + (b[j] - a[j]) * t + if j == 1 { 0.5 } else { 0. }
                        })
                    })
                })
                .collect();
            let blocked: Vec<String> = lifted
                .iter()
                .filter_map(|p| {
                    basic
                        .deepest_overlap(std::slice::from_ref(p))
                        .map(|(i, _)| format!("{} {p:?}", basic.boxes[i].template))
                })
                .collect();
            assert!(blocked.is_empty(), "{path} {blocked:?}");
        }
    }

    /// Soak seed 1087 (F342 build, both routes): the player hit a tyre wall
    /// at 39-45 m/s 25 m off the line. Queried triangle by triangle, the
    /// wall's box gave contacts with downward normals (up to 0.9 m deep),
    /// which pushed the car through the road; it then fell out of the world.
    /// As one box primitive (AE8AD0) every contact pushes up or sideways.
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
                1534,
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
