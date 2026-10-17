//! Breakable route props in the shared physical world (DYNAMIC_PROPS_RESEARCH.md).
//!
//! Every route entity starts static. A breakable entity's intact box carries
//! its shape behaviour threshold as the contact impulse limit (contact +0x3C);
//! when a solved contact reaches `threshold <= 1.01 x contact+0x3C` (0x7BDFD0)
//! the entity is queued and, after the step (0x7D64B0), its intact box stops
//! colliding and the damage behaviour's subentities enter the world as dynamic
//! bodies built by the ported body constructor from the rigid-body and shape
//! properties (0x7CDCD0 / 0x7B8F40). Pieces stay until the world is reset.
//!
//! Selected scope: entities whose damage state adds constraints (tyre walls,
//! armco) have no joint rows in the port and stay static; see `unported`.
//! Pieces never deactivate (the ported grouped frame admits only
//! turnOffTime = FLT_MAX); cylinders are twelve-sided prisms.
use crate::{
    body_geometry::BodyGeometry, contact_surfaces::Surfaces, entity_geometry::Props, route::Route,
    track::Track,
};
use grid_formats::{entities, jpk, physparams, pssg};
use grid_world::{
    Error, collision_mesh, contact_groups, convex_distance, grouped_physical_frame,
    physical_constructor, physical_pose::Matrix,
};
use std::{collections::BTreeMap, path::Path};

/// 0x00CBA8F0: a contact at 1/1.01 of the limit already breaks the shape.
const BREAK_TOLERANCE: f32 = 1.01;
/// Descriptor +0xEC set by 0x7B8F40 (0x00CB1104).
const DESCRIPTOR_EC: f32 = 50.;
/// Authored contact budgets per piece pair (the original's broadphase and
/// pool sizes are not recovered); a piece gets `TRACK_CONTACTS` from each
/// track source (props, boundary, every road chunk).
const TRACK_CONTACTS: usize = 8;
const PAIR_CONTACTS: usize = 4;
/// Pool slots added per live piece, up to the world bound.
const SLOTS_PER_PIECE: usize = 12;
/// AUTHORED: a piece this far below the lowest road chunk has tunnelled
/// through the ground and is held (see [`PropWorld::hold_lost`]).
pub const LOST_DEPTH: f32 = 100.;
/// Body words 0x1C0..0x230: mass centre, pose, angular and linear velocity.
const KINEMATICS: std::ops::Range<usize> = 0x1c0 / 4..0x230 / 4;
const CYLINDER_SIDES: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Primitive {
    Box([f32; 3]),
    Cylinder { radius: f32, height: f32 },
}

/// One dynamic piece a breakable entity turns into.
#[derive(Clone)]
pub struct PieceSpec {
    /// Damage renderable node in objects.pssg and the body's scene anchor.
    pub renderable: String,
    pub anchor: String,
    pub primitive: Primitive,
    /// World pose of the shape centre in the intact placement (row vectors).
    pub pose: Matrix,
    pub mass: f32,
    pub inertia: [f32; 3],
    pub body: physparams::Body,
    pub shell: f32,
    hull: collision_mesh::Mesh,
    points: Vec<[f32; 4]>,
    triangles: Vec<[u32; 3]>,
}

#[derive(Clone)]
pub struct Breakable {
    pub entity: usize,
    pub template: String,
    pub placement: [f32; 16],
    /// Route prop box indices of the intact (static) state.
    pub intact: Vec<usize>,
    pub threshold: f32,
    pub explosion: f32,
    pub pieces: Vec<PieceSpec>,
}

#[derive(Clone, Default)]
pub struct Catalog {
    pub breakables: Vec<Breakable>,
    /// Breakable templates whose damage state needs joints (kept static).
    pub unported: BTreeMap<String, usize>,
}

/// A live piece: catalog indices and its world body/shape identity.
#[derive(Clone, Debug)]
pub struct Piece {
    pub breakable: usize,
    pub spec: usize,
    pub body: usize,
    pub token: u32,
}

/// A break observed this session: the entity and the tick it broke on.
#[derive(Clone, Debug, PartialEq)]
pub struct BreakEvent {
    pub breakable: usize,
    pub template: String,
    pub tick: u64,
    pub force: f32,
}

/// A recorded fresh contact with a route prop box, for break detection.
#[derive(Clone, Copy)]
pub struct PropContact {
    pub contact: contact_groups::Contact,
    pub prop: usize,
    pub body: usize,
}

#[derive(Clone)]
pub struct PropWorld {
    pub catalog: std::sync::Arc<Catalog>,
    /// Breakable index by route prop box.
    by_box: std::sync::Arc<BTreeMap<usize, usize>>,
    pub broken: Vec<bool>,
    pub pieces: Vec<Piece>,
    pub events: Vec<BreakEvent>,
    pending: Vec<(usize, [f32; 4], f32)>,
    base_slots: Option<usize>,
    /// Held kinematics of lost pieces, by piece index.
    lost: BTreeMap<usize, [u32; KINEMATICS.end - KINEMATICS.start]>,
}

fn f(b: &[u32], o: usize) -> f32 {
    f32::from_bits(b[o / 4])
}
fn set(b: &mut [u32], o: usize, v: f32) {
    b[o / 4] = v.to_bits();
}
fn finite_or_max(v: f32) -> f32 {
    if v.is_finite() { v } else { f32::MAX }
}
fn dot3(a: [f32; 4], b: [f32; 4]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Row-vector product a * b.
fn compose(a: &Matrix, b: &Matrix) -> Matrix {
    std::array::from_fn(|r| std::array::from_fn(|c| (0..4).map(|k| a[r][k] * b[k][c]).sum()))
}
fn matrix(m: &[f32; 16]) -> Matrix {
    std::array::from_fn(|r| std::array::from_fn(|c| m[r * 4 + c]))
}

/// Inward-wound triangles (the car hull convention of `collision_mesh::build`).
fn orient(points: &[[f32; 4]], triangles: Vec<[u32; 3]>) -> Vec<[u32; 3]> {
    triangles
        .into_iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| points[i as usize]);
            let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
            ];
            let centroid = [0, 1, 2].map(|j| (a[j] + b[j] + c[j]) / 3.);
            if n[0] * centroid[0] + n[1] * centroid[1] + n[2] * centroid[2] > 0. {
                [t[0], t[2], t[1]]
            } else {
                t
            }
        })
        .collect()
}

/// Local hull points and triangles about the shape centre (cylinder axis Y).
fn hull(primitive: Primitive) -> (Vec<[f32; 4]>, Vec<[u32; 3]>) {
    match primitive {
        Primitive::Box(size) => {
            let points: Vec<_> = (0..8)
                .map(|i| {
                    let mut p = [0., 0., 0., 1.];
                    for j in 0..3 {
                        p[j] = if i >> j & 1 == 0 { -0.5 } else { 0.5 } * size[j];
                    }
                    p
                })
                .collect();
            let faces = [
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
            let triangles = orient(&points, faces.to_vec());
            (points, triangles)
        }
        Primitive::Cylinder { radius, height } => {
            let n = CYLINDER_SIDES;
            let mut points = Vec::with_capacity(2 * n);
            for y in [-0.5 * height, 0.5 * height] {
                for i in 0..n {
                    let a = std::f32::consts::TAU * i as f32 / n as f32;
                    points.push([radius * a.cos(), y, radius * a.sin(), 1.]);
                }
            }
            let mut triangles = Vec::new();
            for i in 0..n as u32 {
                let j = (i + 1) % n as u32;
                let n = n as u32;
                triangles.push([i, j, n + j]);
                triangles.push([i, n + j, n + i]);
            }
            for i in 1..n as u32 - 1 {
                triangles.push([0, i, i + 1]);
                triangles.push([n as u32, n as u32 + i + 1, n as u32 + i]);
            }
            let triangles = orient(&points, triangles);
            (points, triangles)
        }
    }
}

fn reference(uri: &[u8]) -> Result<&str, &'static str> {
    physparams::reference(uri).ok_or("invalid physics property reference")
}

fn mass_properties(primitive: Primitive, density: f32) -> (f32, [f32; 3]) {
    match primitive {
        Primitive::Box([x, y, z]) => {
            let m = density * x * y * z;
            (
                m,
                [
                    m * (y * y + z * z) / 12.,
                    m * (x * x + z * z) / 12.,
                    m * (x * x + y * y) / 12.,
                ],
            )
        }
        Primitive::Cylinder { radius, height } => {
            let m = density * std::f32::consts::PI * radius * radius * height;
            let side = m * (3. * radius * radius + height * height) / 12.;
            (m, [side, m * radius * radius * 0.5, side])
        }
    }
}

impl Catalog {
    pub fn read(
        root: &Path,
        route: &Route,
        props: &Props,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let read = |p: &str| -> Result<Vec<u8>, Box<dyn std::error::Error>> {
            let bytes = std::fs::read(root.join(p))?;
            if bytes.len() > 256 << 20 {
                return Err("dynamic prop source exceeds bound".into());
            }
            Ok(bytes)
        };
        let archive_bytes = read("raceload.jpk")?;
        let archive = jpk::inspect(&archive_bytes)?;
        let entry = archive
            .entries
            .iter()
            .find(|e| e.name == b"physparams.xml")
            .ok_or("missing physparams.xml")?;
        if entry.uncompressed_size as usize != entry.stored_data.len() {
            return Err("compressed physparams.xml unsupported".into());
        }
        let parameters = physparams::Properties::read(&archive_bytes[entry.stored_data.clone()])?;
        let placements = read(&route.file("objects.ens"))?;
        let types = read(&route.track_file("objecttypes.pssg"))?;
        let placements = pssg::read_tree(&placements)?;
        let types = pssg::read_tree(&types)?;
        let mut boxes: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (i, b) in props.boxes.iter().enumerate() {
            if let Some(e) = b.entity {
                boxes.entry(e).or_default().push(i);
            }
        }
        let mut catalog = Self::default();
        for (index, e) in entities::read(&placements, &types)?.into_iter().enumerate() {
            let template = String::from_utf8_lossy(e.template).into_owned();
            let behaviours: Vec<_> = e
                .renderables
                .iter()
                .flat_map(|r| {
                    r.shapes
                        .iter()
                        .filter_map(move |s| Some((r, s.behaviour.as_ref()?)))
                })
                .collect();
            let [(owner, behaviour)] = behaviours[..] else {
                if behaviours.len() > 1 {
                    *catalog.unported.entry(template).or_default() += 1;
                }
                continue;
            };
            if behaviour.constraints != 0
                || !behaviour.deleted.contains(&owner.component)
                || behaviour.added.is_empty()
            {
                *catalog.unported.entry(template).or_default() += 1;
                continue;
            }
            let (threshold, explosion) = parameters.behaviour(reference(behaviour.properties)?)?;
            let placement = matrix(&e.transform);
            let mut pieces = Vec::new();
            for added in &behaviour.added {
                let [shape] = &added.shapes[..] else {
                    return Err("unsupported multi-shape prop piece".into());
                };
                if added.infinite_mass || shape.phantom {
                    return Err("unsupported static prop piece".into());
                }
                let body = parameters.body(reference(added.body_properties)?)?;
                if body.sphericalise != 0. {
                    return Err("unsupported sphericalised prop piece".into());
                }
                let (_, source) = parameters.shape(reference(shape.properties)?)?;
                let physparams::MassSource::Density(density) = source else {
                    return Err("unsupported fixed-mass prop piece".into());
                };
                let primitive = match shape.primitive {
                    entities::Primitive::Box {
                        width,
                        height,
                        length,
                    } => Primitive::Box([width, height, length]),
                    entities::Primitive::Cylinder { radius, height } => {
                        Primitive::Cylinder { radius, height }
                    }
                };
                let (mass, inertia) = mass_properties(primitive, density);
                if !(mass.is_finite() && mass > 0.) {
                    return Err("invalid prop piece mass".into());
                }
                // 0x7D3C40 hands the entity matrix to the body constructor
                // unnormalised (only the render path splits off the row
                // lengths), so a scaled placement (Nürburgring tyre stacks,
                // 0.9718) keeps its scale in the body basis; mass stays the
                // template's density x unscaled volume.
                let pose = compose(&matrix(&shape.transform), &placement);
                let [x, y, z] = [0, 1, 2].map(|i| pose[i]);
                let determinant = x[0] * (y[1] * z[2] - y[2] * z[1])
                    - x[1] * (y[0] * z[2] - y[2] * z[0])
                    + x[2] * (y[0] * z[1] - y[1] * z[0]);
                if !(determinant.is_finite() && determinant > 1e-3) {
                    return Err("degenerate prop piece placement".into());
                }
                let (points, triangles) = hull(primitive);
                let hull = collision_mesh::build(&points, &triangles, 0)?;
                pieces.push(PieceSpec {
                    renderable: String::from_utf8_lossy(added.node).into_owned(),
                    anchor: String::from_utf8_lossy(added.body_node).into_owned(),
                    primitive,
                    pose,
                    mass,
                    inertia,
                    body,
                    shell: shape.shell,
                    hull,
                    points,
                    triangles,
                });
            }
            let intact = boxes.get(&index).cloned().unwrap_or_default();
            if intact.is_empty() {
                return Err("breakable entity without a collision box".into());
            }
            catalog.breakables.push(Breakable {
                entity: index,
                template,
                placement: e.transform,
                intact,
                threshold,
                explosion,
                pieces,
            });
        }
        Ok(catalog)
    }
}

/// 0x7CDCD0 explosion scatter: the new body gains a seeded fraction of `e`
/// plus a perpendicular random component of a quarter of that magnitude. The
/// seed is the fractional part of |x|+|y|+|z|+|e| of the body position.
pub fn explosion_velocity(position: [f32; 3], e: [f32; 4]) -> [f32; 4] {
    let length = dot3(e, e).sqrt();
    if length <= 0. || !length.is_finite() {
        return [0.; 4];
    }
    let unit = e.map(|v| v / length);
    let hash = position[0].abs() + position[1].abs() + position[2].abs() + length;
    let fraction = hash - hash.trunc();
    let draw = |scale: u32, modulus: i64| {
        ((f32::from_bits(scale) * fraction) as i64).rem_euclid(modulus) as f32
    };
    let s = draw(0x4a8d78f0, 0x1948) * f32::from_bits(0x3922046e);
    let tau = f32::from_bits(0x40c90fd8);
    let theta = draw(0x4c7732e2, 0x14f7) * f32::from_bits(0x39435fec) * tau;
    let phi = draw(0x4b4a2bcd, 0xc54) * f32::from_bits(0x39a61fcc) * tau;
    let (sp, cp) = (phi.sin(), phi.cos());
    let r = [theta.cos() * sp, theta.sin() * sp, cp, 0.];
    let projection = dot3(r, unit);
    let magnitude = length * s * 0.25;
    std::array::from_fn(|k| (r[k] - projection * unit[k]) * magnitude + e[k] * s)
}

/// Ported B11F40 settings for a piece (0x7B8F40 field mapping; see research).
fn body_settings(spec: &PieceSpec, linear: [f32; 4]) -> [u32; 88] {
    let mut s = [0u32; 88];
    let b = &spec.body;
    set(&mut s, 0, spec.mass);
    for (i, value) in spec.inertia.iter().enumerate() {
        set(&mut s, 0x10 + 20 * i, *value);
    }
    set(&mut s, 0x4c, 1.);
    s[0x60 / 4] = 1;
    set(&mut s, 0x90, b.wake_up_force);
    set(&mut s, 0x94, b.wake_up_torque);
    set(&mut s, 0x98, b.linear_stationary);
    set(&mut s, 0x9c, b.angular_stationary);
    // turnOffTime: the ported frame only admits never-sleeping bodies.
    set(&mut s, 0xa0, f32::MAX);
    for i in 0..3 {
        set(
            &mut s,
            0xa4 + 4 * i,
            finite_or_max(b.max_angular_velocity[i]),
        );
        set(&mut s, 0xb4 + 4 * i, b.angular_damping[i]);
        set(&mut s, 0xc0 + 4 * i, b.forward_air_resistance[i]);
        set(&mut s, 0xcc + 4 * i, b.backward_air_resistance[i]);
    }
    set(&mut s, 0xb0, finite_or_max(b.max_linear_speed));
    set(&mut s, 0xd8, b.linear_damping);
    set(&mut s, 0xdc, b.gravity_scale);
    set(&mut s, 0xe0, b.sphericalise);
    set(&mut s, 0xe4, b.spherical_mass);
    set(&mut s, 0xe8, b.spherical_radius);
    set(&mut s, 0xec, DESCRIPTOR_EC);
    for r in 0..4 {
        for c in 0..4 {
            set(&mut s, 0xf0 + r * 16 + c * 4, spec.pose[r][c]);
        }
    }
    // Supplied pose and active; directional friction and angular scale off.
    s[0x130 / 4] = 0x0000_0101;
    for (k, v) in linear.iter().take(3).enumerate() {
        set(&mut s, 0x150 + 4 * k, *v);
    }
    s
}

type Properties = (f32, Matrix);

fn piece_body(
    spec: &PieceSpec,
    linear: [f32; 4],
) -> Result<(contact_groups::Body, Properties), Error> {
    let out = physical_constructor::initialize(&body_settings(spec, linear), 0, 0)?;
    let mut body: contact_groups::Body = out.body[..0x424 / 4].try_into().unwrap();
    for o in [0x32c, 0x334, 0x338, 0x33c, 0x340] {
        body[o / 4] = 0;
    }
    body[0x334 / 4] = 0x1c0;
    let properties = (
        f(&body, 0x3f4),
        std::array::from_fn(|i| std::array::from_fn(|j| f(&body, 0x150 + 16 * i + 4 * j))),
    );
    Ok((body, properties))
}

/// Current world pose of a piece's shape (body published matrix).
pub fn body_pose(body: &contact_groups::Body) -> Matrix {
    std::array::from_fn(|r| std::array::from_fn(|c| f(body, 0x1d0 + r * 16 + c * 4)))
}

/// A piece hull in world space with its outward face planes.
struct Convex {
    points: Vec<[f32; 4]>,
    planes: Vec<([f32; 4], f32)>,
    centre: [f32; 4],
}

impl Convex {
    fn new(spec: &PieceSpec, pose: &Matrix) -> Self {
        Self::build(&spec.points, &spec.triangles, pose)
    }
    fn build(local: &[[f32; 4]], triangles: &[[u32; 3]], pose: &Matrix) -> Self {
        let points: Vec<[f32; 4]> = local
            .iter()
            .map(|q| {
                let mut w = [0., 0., 0., 1.];
                for (j, value) in w.iter_mut().take(3).enumerate() {
                    *value = q[0] * pose[0][j] + q[1] * pose[1][j] + q[2] * pose[2][j] + pose[3][j];
                }
                w
            })
            .collect();
        let centre = [pose[3][0], pose[3][1], pose[3][2], 1.];
        let mut planes: Vec<([f32; 4], f32)> = Vec::new();
        for t in triangles {
            let [a, b, c] = t.map(|i| points[i as usize]);
            let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let ac = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let mut n = [
                ab[1] * ac[2] - ab[2] * ac[1],
                ab[2] * ac[0] - ab[0] * ac[2],
                ab[0] * ac[1] - ab[1] * ac[0],
                0.,
            ];
            let length = dot3(n, n).sqrt();
            if length <= 0. {
                continue;
            }
            n = n.map(|v| v / length);
            if dot3(
                n,
                [a[0] - centre[0], a[1] - centre[1], a[2] - centre[2], 0.],
            ) < 0.
            {
                n = n.map(|v| -v);
            }
            if planes.iter().any(|(m, _)| dot3(*m, n) > 0.9999) {
                continue;
            }
            planes.push((n, dot3(n, a)));
        }
        Self {
            points,
            planes,
            centre,
        }
    }
    fn extent(&self, axis: [f32; 4]) -> (f32, f32) {
        self.points
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), p| {
                let d = dot3(*p, axis);
                (lo.min(d), hi.max(d))
            })
    }
    /// Inside every face plane not facing along `axis`, with tolerance.
    fn contains(&self, p: [f32; 4], axis: [f32; 4], tolerance: f32) -> bool {
        self.planes
            .iter()
            .filter(|(n, _)| dot3(*n, axis).abs() < 0.999)
            .all(|(n, d)| dot3(*n, p) <= d + tolerance)
    }
}

/// Authored separating-axis contacts between two convex piece hulls (face
/// axes of both). The original's primitive box/cylinder colliders are not
/// recovered; the triangle-soup pair query loses stacked pieces once they
/// overlap beyond the margin. Same convention as the packed pair query:
/// the normal pushes `a`, penetration includes the margin.
fn convex_contacts(
    a: &Convex,
    b: &Convex,
    margin: f32,
    capacity: usize,
) -> Vec<grid_world::contact_manifold::Contact> {
    let mut best: Option<([f32; 4], f32)> = None;
    for (n, _) in a.planes.iter().chain(&b.planes) {
        let (a_low, a_high) = a.extent(*n);
        let (b_low, b_high) = b.extent(*n);
        let overlap = (b_high - a_low).min(a_high - b_low);
        if overlap < -margin {
            return Vec::new();
        }
        if best.is_none_or(|(_, o)| overlap < o) {
            best = Some((*n, overlap));
        }
    }
    let Some((mut n, _)) = best else {
        return Vec::new();
    };
    let towards = [
        a.centre[0] - b.centre[0],
        a.centre[1] - b.centre[1],
        a.centre[2] - b.centre[2],
        0.,
    ];
    if dot3(n, towards) < 0. {
        n = n.map(|v| -v);
    }
    let (a_low, _) = a.extent(n);
    let (_, b_high) = b.extent(n);
    let tolerance = margin + 0.01;
    let mut candidates: Vec<([f32; 4], f32)> = Vec::new();
    for p in &a.points {
        let depth = b_high - dot3(*p, n);
        if depth >= -margin && b.contains(*p, n, tolerance) {
            candidates.push((*p, depth));
        }
    }
    for q in &b.points {
        let depth = dot3(*q, n) - a_low;
        if depth >= -margin && a.contains(*q, n, tolerance) {
            candidates.push((*q, depth));
        }
    }
    candidates.sort_by(|x, y| y.1.total_cmp(&x.1));
    let mut chosen: Vec<([f32; 4], f32)> = Vec::new();
    for c in candidates {
        if chosen.len() == capacity {
            break;
        }
        if chosen.iter().all(|(p, _)| {
            let d = [p[0] - c.0[0], p[1] - c.0[1], p[2] - c.0[2], 0.];
            dot3(d, d) > 1e-4
        }) {
            chosen.push(c);
        }
    }
    chosen
        .into_iter()
        .map(|(point, depth)| grid_world::contact_manifold::Contact {
            point,
            normal: n,
            penetration: depth + margin,
            flipped: false,
        })
        .collect()
}

fn world_triangles(spec: &PieceSpec, pose: &Matrix) -> Vec<[[f32; 3]; 3]> {
    let p: Vec<[f32; 3]> = spec
        .points
        .iter()
        .map(|q| {
            std::array::from_fn(|j| {
                q[0] * pose[0][j] + q[1] * pose[1][j] + q[2] * pose[2][j] + pose[3][j]
            })
        })
        .collect();
    spec.triangles
        .iter()
        .map(|t| t.map(|i| p[i as usize]))
        .collect()
}

/// Two-body contact record (MultiRuntime car-to-car layout).
fn pair_record(
    hit: &grid_world::contact_manifold::Contact,
    bodies: [usize; 2],
    tokens: [u32; 2],
) -> contact_groups::Contact {
    let mut c = [0; 32];
    c[1] = bodies[0] as u32 + 1;
    c[2] = bodies[1] as u32 + 1;
    c[0xc / 4] = 0x01000000;
    c[0x1c / 4] = u32::MAX;
    c[0x20 / 4] = u32::MAX;
    c[0x24 / 4] = 1;
    c[0x28 / 4] = tokens[0];
    c[0x2c / 4] = tokens[1];
    c[0x38 / 4] = hit.penetration.to_bits();
    c[0x3c / 4] = f32::MAX.to_bits();
    c[0x50 / 4..0x60 / 4].copy_from_slice(&hit.point.map(f32::to_bits));
    c[0x60 / 4..0x70 / 4].copy_from_slice(&hit.normal.map(f32::to_bits));
    c[0x70 / 4] = (u32::from(hit.flipped) << 8) | (1 << 24);
    c
}

/// A car hull taking part in piece contacts.
pub struct CarHull<'a> {
    pub geometry: &'a BodyGeometry,
    pub body: usize,
    pub token: u32,
}

impl PropWorld {
    pub fn new(catalog: Catalog) -> Self {
        let mut by_box = BTreeMap::new();
        for (i, b) in catalog.breakables.iter().enumerate() {
            for &k in &b.intact {
                by_box.insert(k, i);
            }
        }
        Self {
            broken: vec![false; catalog.breakables.len()],
            catalog: std::sync::Arc::new(catalog),
            by_box: std::sync::Arc::new(by_box),
            pieces: Vec::new(),
            events: Vec::new(),
            pending: Vec::new(),
            base_slots: None,
            lost: BTreeMap::new(),
        }
    }

    /// Hold every piece that has fallen more than [`LOST_DEPTH`] below
    /// `floor` (the lowest road chunk bound): it keeps its position with zero
    /// velocity from then on. The original lets such a piece fall forever,
    /// harmlessly, as nothing there bounds coordinates; the port's 1e5
    /// coordinate domain (`Mesh::world_bounds`) would instead stop the whole
    /// world about 150 s later (F318 R06). Call after each committed step.
    pub fn hold_lost(&mut self, bodies: &mut [contact_groups::Body], floor: f32) {
        for (k, piece) in self.pieces.iter().enumerate() {
            let Some(body) = bodies.get_mut(piece.body) else {
                continue;
            };
            let held = self.lost.entry(k);
            let held = match held {
                std::collections::btree_map::Entry::Occupied(e) => e.into_mut(),
                std::collections::btree_map::Entry::Vacant(e) => {
                    if f(body, 0x1d0 + 52) >= floor - LOST_DEPTH {
                        continue;
                    }
                    let mut kinematics: [u32; KINEMATICS.end - KINEMATICS.start] =
                        body[KINEMATICS].try_into().unwrap();
                    kinematics[(0x210 - 0x1c0) / 4..].fill(0);
                    e.insert(kinematics)
                }
            };
            body[KINEMATICS].copy_from_slice(held);
        }
    }

    /// Whether piece `k` has been lost below the world and is held.
    pub fn is_lost(&self, k: usize) -> bool {
        self.lost.contains_key(&k)
    }

    /// Arm the intact breakable boxes: their threshold becomes the contact limit.
    pub fn arm(&self, track: &mut Track) -> Result<(), Error> {
        let props = track
            .props_mut()
            .ok_or(Error("dynamic props require route props"))?;
        for (i, b) in self.catalog.breakables.iter().enumerate() {
            for &k in &b.intact {
                let prop = props
                    .boxes
                    .get_mut(k)
                    .ok_or(Error("breakable prop box out of range"))?;
                prop.limit = b.threshold;
                prop.enabled = !self.broken[i];
            }
        }
        Ok(())
    }

    /// Fresh contacts of every live piece: track (packed road, boundary and
    /// enabled route props), other pieces and the given car hulls. Prop
    /// contacts are recorded for break detection.
    #[allow(clippy::too_many_arguments)]
    pub fn contacts(
        &self,
        track: &Track,
        surfaces: &Surfaces,
        bodies: &[contact_groups::Body],
        cars: &[CarHull<'_>],
        directions: &convex_distance::ContactSettings,
        remaining: usize,
        recorded: &mut Vec<(usize, PropContact)>,
    ) -> Result<Vec<contact_groups::Contact>, Error> {
        let mut out = Vec::new();
        let poses = self
            .pieces
            .iter()
            .map(|p| {
                bodies
                    .get(p.body)
                    .map(body_pose)
                    .ok_or(Error("missing prop piece body"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        // World hulls once per call, with the largest centre-to-vertex reach.
        let convexes: Vec<(Convex, f32)> = self
            .pieces
            .iter()
            .zip(&poses)
            .map(|(p, pose)| {
                let convex =
                    Convex::new(&self.catalog.breakables[p.breakable].pieces[p.spec], pose);
                let reach = convex
                    .points
                    .iter()
                    .map(|q| {
                        let d = [
                            q[0] - convex.centre[0],
                            q[1] - convex.centre[1],
                            q[2] - convex.centre[2],
                            0.,
                        ];
                        dot3(d, d).sqrt()
                    })
                    .fold(0., f32::max);
                (convex, reach)
            })
            .collect();
        for (k, piece) in self.pieces.iter().enumerate() {
            let spec = &self.catalog.breakables[piece.breakable].pieces[piece.spec];
            let query = convex_distance::ContactSettings {
                margin: spec.shell,
                directions: directions.directions,
            };
            let pose = poses[k];
            let room = remaining - out.len();
            if room == 0 {
                break;
            }
            for hit in
                track.body_contacts(&spec.hull, pose, &query, TRACK_CONTACTS, room, |code| {
                    surfaces.sheet(code)
                })?
            {
                let mut c = [0; 32];
                c[1] = piece.body as u32 + 1;
                c[0xc / 4] = 0x01000000;
                c[0x1c / 4] = u32::MAX;
                c[0x20 / 4] = u32::MAX;
                c[0x24 / 4] = 1;
                c[0x28 / 4] = piece.token;
                c[0x2c / 4] = hit.shape + 1;
                c[0x38 / 4] = hit.contact.penetration.to_bits();
                c[0x3c / 4] = hit.limit.to_bits();
                c[0x50 / 4..0x60 / 4].copy_from_slice(&hit.contact.point.map(f32::to_bits));
                c[0x60 / 4..0x70 / 4].copy_from_slice(&hit.contact.normal.map(f32::to_bits));
                c[0x70 / 4] = (u32::from(hit.contact.flipped) << 8) | (1 << 24);
                if let Some(prop) = hit.prop {
                    recorded.push((
                        out.len(),
                        PropContact {
                            contact: c,
                            prop,
                            body: piece.body,
                        },
                    ));
                }
                out.push(c);
            }
            for (j, other) in self.pieces.iter().enumerate().skip(k + 1) {
                let room = remaining - out.len();
                if room == 0 {
                    break;
                }
                let other_spec = &self.catalog.breakables[other.breakable].pieces[other.spec];
                let margin = spec.shell.max(other_spec.shell);
                let ((a, a_reach), (b, b_reach)) = (&convexes[k], &convexes[j]);
                // Each hull's face normals hold an orthonormal triple (box
                // faces; prism cap and radials 90 degrees apart), so beyond
                // sqrt(3) x the summed reach one of them already separates
                // the pair and the face-axis test returns nothing.
                let d = [
                    a.centre[0] - b.centre[0],
                    a.centre[1] - b.centre[1],
                    a.centre[2] - b.centre[2],
                    0.,
                ];
                let reach = a_reach + b_reach + margin + 0.01;
                if dot3(d, d) > 3. * reach * reach {
                    continue;
                }
                for hit in convex_contacts(a, b, margin, room.min(PAIR_CONTACTS)) {
                    out.push(pair_record(
                        &hit,
                        [piece.body, other.body],
                        [piece.token, other.token],
                    ));
                }
            }
            let mut hull = None;
            for car in cars {
                let room = remaining - out.len();
                if room == 0 {
                    break;
                }
                let body = bodies.get(car.body).ok_or(Error("missing car body"))?;
                let hull = hull.get_or_insert_with(|| {
                    crate::body_geometry::WorldHull::new(&world_triangles(spec, &pose))
                });
                for hit in car
                    .geometry
                    .car_contacts_hull(body, hull, room.min(TRACK_CONTACTS))?
                {
                    out.push(pair_record(
                        &hit,
                        [car.body, piece.body],
                        [car.token, piece.token],
                    ));
                }
            }
        }
        Ok(out)
    }

    /// Queue breaks from solved prop contacts: `recorded` holds the pool
    /// index of each fresh prop contact in `solved` (verified by point,
    /// normal and shapes before use).
    pub fn detect(
        &mut self,
        solved: &[contact_groups::Contact],
        recorded: &[(usize, PropContact)],
        bodies: &[contact_groups::Body],
    ) {
        for (index, record) in recorded {
            let Some(c) = solved.get(*index).filter(|c| {
                c[0x50 / 4..0x70 / 4] == record.contact[0x50 / 4..0x70 / 4]
                    && c[0x28 / 4..0x30 / 4] == record.contact[0x28 / 4..0x30 / 4]
            }) else {
                continue;
            };
            let Some(&breakable) = self.by_box.get(&record.prop) else {
                continue;
            };
            let entry = &self.catalog.breakables[breakable];
            let force = f(c, 0x3c);
            if self.broken[breakable]
                || self.pending.iter().any(|p| p.0 == breakable)
                // NaN never breaks, as in the original comparison.
                || entry.threshold.partial_cmp(&(force * BREAK_TOLERANCE))
                    .is_none_or(|o| o == std::cmp::Ordering::Greater)
            {
                continue;
            }
            // Other body's velocity projected on the contact normal (0x7BDFD0).
            let normal: [f32; 4] = std::array::from_fn(|k| f(c, 0x60 + 4 * k));
            let velocity: [f32; 4] = bodies
                .get(record.body)
                .map(|b| std::array::from_fn(|k| f(b, 0x220 + 4 * k)))
                .unwrap_or_default();
            let along = dot3(velocity, normal);
            let explosion = normal.map(|n| entry.explosion * along * n);
            self.pending.push((breakable, explosion, force));
        }
    }

    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }

    /// Queue a break at rest, as if a contact had saturated the threshold.
    #[cfg(test)]
    pub(crate) fn queue_break(&mut self, breakable: usize) {
        let threshold = self.catalog.breakables[breakable].threshold;
        self.pending.push((breakable, [0.; 4], threshold));
    }

    /// Apply queued breaks after the step: append the pieces as dynamic bodies
    /// and return the intact boxes the caller must `disable` in every track.
    /// On failure the caller discards `world`/`shapes`.
    pub fn apply(
        &mut self,
        world: &mut grouped_physical_frame::Output,
        shapes: &mut Vec<contact_groups::Shape>,
        tick: u64,
    ) -> Result<Vec<usize>, Error> {
        if self.pending.is_empty() {
            return Ok(Vec::new());
        }
        let base = *self.base_slots.get_or_insert(world.groups.contacts.len());
        let mut disabled = Vec::new();
        for (breakable, explosion, force) in std::mem::take(&mut self.pending) {
            let entry = &self.catalog.breakables[breakable];
            // 0x7D5180 swaps only when 0x7D4940 finds room for the new
            // components; otherwise the entity stays intact. The original
            // first restores the farthest broken entities (0x7D4770), which
            // the port omits: its 312 possible pieces cannot fill the pool.
            if world.groups.bodies.len() + entry.pieces.len() > grid_world::MAX_WORLD_BODIES {
                continue;
            }
            disabled.extend(entry.intact.iter().copied());
            for (spec_index, spec) in entry.pieces.iter().enumerate() {
                let linear = explosion_velocity(
                    [spec.pose[3][0], spec.pose[3][1], spec.pose[3][2]],
                    explosion,
                );
                let (mut body, properties) = piece_body(spec, linear)?;
                // Append to the active chain after its current tail. The
                // world double-buffers its active heads/tails by selector.
                let selector = world.groups.world[5] as usize;
                if selector > 1 {
                    return Err(Error("invalid physical world selector"));
                }
                let mut tail = world.groups.world[7 + selector] as usize;
                let mut guard = 0;
                while tail != 0 && world.groups.bodies[tail - 1][0x300 / 4] != 0 {
                    tail = world.groups.bodies[tail - 1][0x300 / 4] as usize;
                    guard += 1;
                    if guard > world.groups.bodies.len() {
                        return Err(Error("cyclic physical body chain"));
                    }
                }
                if tail == 0 || world.groups.world[9 + selector] as usize != tail {
                    return Err(Error("unsupported physical body chain"));
                }
                body[0x400 / 4] = world.groups.bodies[tail - 1][0x400 / 4];
                let id = world.groups.bodies.len() + 1;
                body[0x300 / 4] = 0;
                body[0x304 / 4] = tail as u32;
                world.groups.bodies[tail - 1][0x300 / 4] = id as u32;
                world.groups.bodies.push(body);
                world.groups.world[9 + selector] = id as u32;
                world.manager.properties.push(properties);
                world.manager.workspace.push([[0.; 4]; 2]);
                shapes.push(contact_groups::Shape {
                    flag229: true,
                    ..Default::default()
                });
                self.pieces.push(Piece {
                    breakable,
                    spec: spec_index,
                    body: id - 1,
                    token: shapes.len() as u32,
                });
            }
            self.broken[breakable] = true;
            self.events.push(BreakEvent {
                breakable,
                template: entry.template.clone(),
                tick,
                force,
            });
        }
        let slots =
            (base + SLOTS_PER_PIECE * self.pieces.len()).min(grid_world::MAX_WORLD_CONTACTS);
        if slots > world.groups.contacts.len() {
            world.groups.contacts.resize(slots, [0; 32]);
            world.manager.rows.resize(slots * 4, [0; 48]);
            world.manager.capacity = slots * 4;
        }
        Ok(disabled)
    }

    /// Stop the broken intact boxes colliding in a track.
    pub fn disable(track: &mut Track, boxes: &[usize]) {
        if let Some(props) = track.props_mut() {
            for &k in boxes {
                if let Some(b) = props.boxes.get_mut(k) {
                    b.enabled = false;
                }
            }
        }
    }

    /// Restore the intact state in the given tracks (the caller restores its
    /// own initial world).
    pub fn reset(&mut self, tracks: &mut [&mut Track]) -> Result<(), Error> {
        self.broken.iter_mut().for_each(|b| *b = false);
        self.pieces.clear();
        self.events.clear();
        self.pending.clear();
        self.base_slots = None;
        self.lost.clear();
        for track in tracks {
            self.arm(track)?;
        }
        Ok(())
    }

    /// Piece world poses for presentation: (breakable, piece, pose).
    pub fn poses(&self, bodies: &[contact_groups::Body]) -> Vec<(usize, usize, Matrix)> {
        self.pieces
            .iter()
            .filter_map(|p| Some((p.breakable, p.spec, body_pose(bodies.get(p.body)?))))
            .collect()
    }
}

/// Authored validation driving (not original behaviour): follow the route
/// centreline until 45 m before the point nearest a chosen breakable, steer
/// straight at it, and half a second after the first break brake to a stop.
pub struct PropPursuit {
    points: Vec<(f64, [f64; 3])>,
    pub goal: [f64; 3],
    switch: f64,
    travelled: f64,
    last: Option<[f64; 3]>,
    speed: f64,
}

impl PropPursuit {
    /// `target` is `TEMPLATE_PREFIX:INDEX` among the catalog's breakables.
    pub fn new(
        root: &Path,
        route: &Route,
        catalog: &Catalog,
        target: &str,
        speed: f64,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (prefix, index) = target.rsplit_once(':').ok_or("target is TEMPLATE:INDEX")?;
        let index: usize = index.parse()?;
        let b = catalog
            .breakables
            .iter()
            .filter(|b| b.template.starts_with(prefix))
            .nth(index)
            .ok_or("no such breakable")?;
        let goal = [12, 13, 14].map(|k| f64::from(b.placement[k]));
        let points: Vec<_> = crate::lap_progress::load_route(root, route)?
            .progress
            .points
            .iter()
            .map(|p| (f64::from(p.distance), p.position.map(f64::from)))
            .collect();
        let near = points
            .iter()
            .min_by(|a, b| {
                let d = |p: &[f64; 3]| (p[0] - goal[0]).powi(2) + (p[2] - goal[2]).powi(2);
                d(&a.1).total_cmp(&d(&b.1))
            })
            .ok_or("empty route centreline")?
            .0;
        Ok(Self {
            points,
            goal,
            switch: near - 45.,
            travelled: 0.,
            last: None,
            speed,
        })
    }

    pub fn controls(
        &mut self,
        pose: [f32; 16],
        velocity: [f32; 4],
        broken_at: Option<u64>,
        tick: u64,
    ) -> crate::source_vehicle::Controls {
        let pose = pose.map(f64::from);
        let position = [pose[12], pose[13], pose[14]];
        if let Some(p) = self.last {
            self.travelled += ((position[0] - p[0]).powi(2) + (position[2] - p[2]).powi(2)).sqrt();
        }
        self.last = Some(position);
        let speed = f64::from(velocity[0]).hypot(f64::from(velocity[2]));
        if broken_at.is_some_and(|t| tick > t + 30) {
            return crate::source_vehicle::Controls {
                brake: if speed > 0.5 { 1. } else { 0. },
                ..Default::default()
            };
        }
        let aim = if self.travelled < self.switch {
            let nearest = self
                .points
                .iter()
                .enumerate()
                .min_by(|a, b| {
                    let d =
                        |p: &[f64; 3]| (p[0] - position[0]).powi(2) + (p[2] - position[2]).powi(2);
                    d(&a.1.1).total_cmp(&d(&b.1.1))
                })
                .map_or(0, |(i, _)| i);
            let ahead = self.points[nearest].0 + 12.;
            let i = self
                .points
                .partition_point(|p| p.0 <= ahead)
                .min(self.points.len() - 1);
            self.points[i].1
        } else {
            self.goal
        };
        let to = [aim[0] - position[0], aim[2] - position[2]];
        let x = to[0] * pose[0] + to[1] * pose[2];
        let z = to[0] * pose[8] + to[1] * pose[10];
        crate::source_vehicle::Controls {
            throttle: if speed < self.speed { 1. } else { 0. },
            brake: if speed > self.speed + 3. { 0.5 } else { 0. },
            steering: (1.5 * x.atan2(z)).clamp(-1., 1.) as f32,
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hulls_face_outward_and_masses_follow_density() {
        for primitive in [
            Primitive::Box([0.94, 0.47, 0.16]),
            Primitive::Cylinder {
                radius: 0.28,
                height: 0.15,
            },
        ] {
            let (points, triangles) = hull(primitive);
            let mesh = collision_mesh::build(&points, &triangles, 0).unwrap();
            for (i, t) in triangles.iter().enumerate() {
                let n = mesh.face_normal(i).unwrap();
                let c: Vec<f32> = (0..3)
                    .map(|j| t.iter().map(|&k| points[k as usize][j]).sum::<f32>())
                    .collect();
                assert!(
                    n[0] * c[0] + n[1] * c[1] + n[2] * c[2] > 0.,
                    "{primitive:?} {i}"
                );
            }
        }
        let (m, i) = mass_properties(Primitive::Box([1., 2., 3.]), 700.);
        assert_eq!(m, 4200.);
        assert_eq!(i, [4550., 3500., 1750.]);
        let (m, _) = mass_properties(
            Primitive::Cylinder {
                radius: 1.,
                height: 1.,
            },
            1.,
        );
        assert!((m - std::f32::consts::PI).abs() < 1e-6);
    }

    #[test]
    fn stacked_pieces_push_apart_vertically_at_any_overlap() {
        for primitive in [
            Primitive::Cylinder {
                radius: 0.28,
                height: 0.14,
            },
            Primitive::Box([0.94, 0.47, 0.16]),
        ] {
            let (points, triangles) = hull(primitive);
            let height = match primitive {
                Primitive::Cylinder { height, .. } => height,
                Primitive::Box(s) => s[1],
            };
            let mut lower = [[0.; 4]; 4];
            for (i, row) in lower.iter_mut().enumerate() {
                row[i] = 1.;
            }
            let mut upper = lower;
            let below = Convex::build(&points, &triangles, &lower);
            for overlap in [-0.03f32, -0.01, 0.005, 0.02, 0.05, 0.1] {
                upper[3] = [0.01, height - overlap, 0.02, 1.];
                let above = Convex::build(&points, &triangles, &upper);
                let hits = convex_contacts(&above, &below, 0.02, 4);
                if overlap < -0.02 {
                    assert!(hits.is_empty());
                    continue;
                }
                assert_eq!(hits.len(), 4, "{primitive:?} {overlap}");
                for h in hits {
                    assert!(
                        h.normal[1] > 0.999,
                        "{primitive:?} {overlap} {:?}",
                        h.normal
                    );
                    assert!((h.penetration - (overlap + 0.02)).abs() < 1e-4);
                }
            }
        }
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn donington_breakables_follow_the_shape_behaviours() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let route = Route::default();
        let props = Props::read_route(root, &route).unwrap();
        let catalog = Catalog::read(root, &route, &props).unwrap();
        let mut by_template = BTreeMap::<String, (usize, f32, usize)>::new();
        for b in &catalog.breakables {
            let key = if b.template.starts_with("gg_brake_marker") {
                "gg_brake_marker".to_string()
            } else {
                b.template.clone()
            };
            let e = by_template
                .entry(key)
                .or_insert((0, b.threshold, b.pieces.len()));
            e.0 += 1;
            assert_eq!((e.1, e.2), (b.threshold, b.pieces.len()));
            assert_eq!(b.explosion, 0.);
            assert!(
                b.intact
                    .iter()
                    .all(|&k| props.boxes[k].entity == Some(b.entity))
            );
        }
        eprintln!("breakables={by_template:?} unported={:?}", catalog.unported);
        assert_eq!(by_template["gg_brake_marker"], (34, 50., 2));
        assert_eq!(by_template["tyrestack_black.max"], (8, 50., 12));
        assert_eq!(by_template["cushion_blue.max"], (148, 250., 1));
        assert_eq!(catalog.unported["tyre_wall.max"], 588);
        assert_eq!(
            catalog.unported["armco_white_main.max"] + catalog.unported["armco_blue_main.max"],
            243
        );
        let board = &catalog
            .breakables
            .iter()
            .find(|b| b.template.starts_with("gg_brake_marker"))
            .unwrap()
            .pieces[0];
        assert!(
            (board.mass - 70. * 0.94 * 0.47 * 0.16).abs() < 0.01,
            "{}",
            board.mass
        );
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn nurburgring_scaled_tyre_stacks_keep_their_placement_scale() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let route = Route::new("tracks/europe/nurburgring/route_0").unwrap();
        let props = Props::read_route(root, &route).unwrap();
        let catalog = Catalog::read(root, &route, &props).unwrap();
        let mut scaled = BTreeMap::<String, usize>::new();
        for b in &catalog.breakables {
            for p in &b.pieces {
                let lengths = [0, 1, 2].map(|i| dot3(p.pose[i], p.pose[i]).sqrt());
                if lengths.iter().any(|l| (l - 1.).abs() > 1e-3) {
                    // Uniform: the entity scale, not a shear.
                    assert!(lengths.iter().all(|l| (l - lengths[0]).abs() < 1e-4));
                    assert!((lengths[0] - 0.9718).abs() < 1e-3, "{lengths:?}");
                    *scaled.entry(b.template.clone()).or_default() += 1;
                }
                if b.template.starts_with("tyrestack") {
                    // sRubber d700 over the unscaled template cylinder.
                    assert_eq!(p.mass, mass_properties(p.primitive, 700.).0);
                }
            }
        }
        eprintln!("scaled={scaled:?} unported={:?}", catalog.unported);
        assert_eq!(scaled["tyrestack_white.max"] % 12, 0);
        assert!(scaled.contains_key("tyrestack_red.max"));
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT and GRID_CONTACT_DIRECTIONS (private CD01)"]
    fn a_hull_against_a_prop_box_keeps_its_road_contacts() {
        // Soak seed 9 (F342): a board standing against a static prop box got
        // all its track slots from the box and slid down through the road.
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let table = std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
        let mut runtime = crate::source_vehicle::Runtime::lacetti(root, "slot_00", 200679).unwrap();
        let geometry = runtime.prepare_body_geometry(root, &table).unwrap();
        let track = runtime.track();
        let query = convex_distance::ContactSettings {
            margin: 0.02,
            directions: geometry.query().directions,
        };
        let (points, triangles) = hull(Primitive::Box([0.3, 0.8, 0.3]));
        let mesh = collision_mesh::build(&points, &triangles, 0).unwrap();
        let mut cache = grid_world::rectangle_cull::Cache::default();
        let (mut full, mut with_road) = (0, 0);
        for b in &track.props().unwrap().boxes {
            // A hull straddling the box's -x face, 0.1 m into the road under it.
            let (x, z) = (b.bounds[0], 0.5 * (b.bounds[2] + b.bounds[5]));
            let (top, bottom) = (b.bounds[4] + 1., b.bounds[1] - 2.);
            let Some(hit) = track
                .query([x, top, z, 1.], [x, bottom, z, 1.], &mut cache)
                .unwrap()
                .hit
            else {
                continue;
            };
            let road = top + (bottom - top) * hit.contact.fraction;
            if (road - b.bounds[1]).abs() > 0.3 || b.bounds[4] - road < 1. {
                continue;
            }
            let mut pose: Matrix =
                std::array::from_fn(|r| std::array::from_fn(|c| f32::from(r == c)));
            pose[3] = [x, road + 0.3, z, 1.];
            let count = |pool| {
                let hits = track
                    .body_contacts(&mesh, pose, &query, TRACK_CONTACTS, pool, |code| {
                        runtime.surfaces().sheet(code)
                    })
                    .unwrap();
                let props = hits.iter().filter(|h| h.prop.is_some()).count();
                let road = hits
                    .iter()
                    .filter(|h| h.prop.is_none() && h.contact.normal[1] > 0.7)
                    .count();
                (props, road)
            };
            let (props, road) = count(64);
            if props < TRACK_CONTACTS {
                continue;
            }
            // The prop pair is full. One shared budget of the same size (the
            // old behaviour) leaves the road nothing; its own pair does not.
            assert_eq!(count(TRACK_CONTACTS), (TRACK_CONTACTS, 0));
            full += 1;
            with_road += usize::from(road > 0);
        }
        eprintln!("prop boxes with a full pair: {full}, with road contacts: {with_road}");
        assert!(full > 100 && with_road * 10 >= full * 9);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT and GRID_CONTACT_DIRECTIONS"]
    fn every_brake_marker_board_touches_the_ground_when_it_spawns() {
        // Soak seeds 12, 1012 and 1029 (F345, F350): the lower board of the
        // 150 m marker at (-406.7, -16.0) stands 2.14 cm above the grass,
        // just outside its own 2 cm shell. With road triangles of margin 0
        // it had no ground contact in its first step, a car still inside it
        // pushed it 0.38 m down and it sank through. The original gives road
        // triangles their chunk's 0.02 margin (ROAD_MARGIN), so the board
        // meets the ground from the first step.
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let table = std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
        let mut runtime = crate::source_vehicle::Runtime::lacetti(root, "slot_00", 200679).unwrap();
        let geometry = runtime.prepare_body_geometry(root, &table).unwrap();
        let track = runtime.track();
        let catalog = Catalog::read(root, &Route::default(), track.props().unwrap()).unwrap();
        let mut boards = 0;
        for (i, b) in catalog.breakables.iter().enumerate() {
            if !b.template.starts_with("gg_brake_marker") {
                continue;
            }
            let lower = b
                .pieces
                .iter()
                .min_by(|p, q| p.pose[3][1].total_cmp(&q.pose[3][1]))
                .unwrap();
            let query = convex_distance::ContactSettings {
                margin: lower.shell,
                directions: geometry.query().directions,
            };
            let ground = track
                .body_contacts(
                    &lower.hull,
                    lower.pose,
                    &query,
                    TRACK_CONTACTS,
                    64,
                    |code| runtime.surfaces().sheet(code),
                )
                .unwrap()
                .into_iter()
                .filter(|h| h.prop.is_none() && h.contact.normal[1] > 0.9)
                .count();
            assert!(ground > 0, "breakable {i} at {:?}", lower.pose[3]);
            boards += 1;
        }
        assert_eq!(boards, 34);
    }

    #[test]
    fn a_piece_lost_below_the_world_is_held_until_reset() {
        let mut world = PropWorld::new(Catalog::default());
        world.pieces = (0..2)
            .map(|k| Piece {
                breakable: 0,
                spec: 0,
                body: k + 1,
                token: k as u32 + 2,
            })
            .collect();
        let mut bodies = vec![[0u32; 0x424 / 4]; 3];
        let at = |b: &mut contact_groups::Body, y: f32, vy: f32| {
            set(b, 0x1d0 + 52, y);
            set(b, 0x224, vy);
            set(b, 0x210, 3.);
        };
        at(&mut bodies[1], -20. - LOST_DEPTH - 0.5, -40.);
        at(&mut bodies[2], -20. - LOST_DEPTH + 0.5, -40.);
        world.hold_lost(&mut bodies, -20.);
        assert!(world.is_lost(0) && !world.is_lost(1));
        assert_eq!(f(&bodies[1], 0x224), 0.);
        assert_eq!(f(&bodies[1], 0x210), 0.);
        assert_eq!(f(&bodies[1], 0x1d0 + 52), -20. - LOST_DEPTH - 0.5);
        assert_eq!(f(&bodies[2], 0x224), -40.);
        // The next step moves it again; holding puts it back.
        at(&mut bodies[1], -200., -0.2);
        world.hold_lost(&mut bodies, -20.);
        assert_eq!(f(&bodies[1], 0x1d0 + 52), -20. - LOST_DEPTH - 0.5);
        assert_eq!(f(&bodies[1], 0x224), 0.);
        world.reset(&mut []).unwrap();
        assert!(!world.is_lost(0));
    }

    #[test]
    fn explosion_scatter_is_seeded_and_zero_without_impulse() {
        assert_eq!(explosion_velocity([1., 2., 3.], [0.; 4]), [0.; 4]);
        let e = [0., 0., -3., 0.];
        let a = explosion_velocity([10.5, 1., -7.25], e);
        assert_eq!(a, explosion_velocity([10.5, 1., -7.25], e));
        // Bounded by the seeded fraction of |e| plus a quarter perpendicular.
        let length = dot3(a, a).sqrt();
        assert!(length <= 3. * 1.25 + 1e-4, "{a:?}");
        assert!(a[2] <= 0.);
    }

    #[test]
    fn piece_body_carries_mass_pose_and_body_properties() {
        let (points, triangles) = hull(Primitive::Box([1., 1., 1.]));
        let mut pose = [[0.; 4]; 4];
        for (i, row) in pose.iter_mut().enumerate() {
            row[i] = 1.;
        }
        pose[3] = [5., 2., -3., 1.];
        let spec = PieceSpec {
            renderable: String::new(),
            anchor: String::new(),
            primitive: Primitive::Box([1., 1., 1.]),
            pose,
            mass: 70.,
            inertia: [70. / 6.; 3],
            body: physparams::Body {
                sphericalise: 0.,
                spherical_radius: 1.,
                spherical_mass: 1.,
                max_angular_velocity: [f32::INFINITY; 3],
                max_linear_speed: f32::INFINITY,
                angular_damping: [1.; 3],
                linear_damping: 1.,
                forward_air_resistance: [0.; 3],
                backward_air_resistance: [0.; 3],
                linear_stationary: 0.3,
                angular_stationary: 0.3,
                wake_up_force: 0.,
                wake_up_torque: 0.,
                turn_off_time: 1.,
                gravity_scale: 1.,
            },
            shell: 0.02,
            hull: collision_mesh::build(&points, &triangles, 0).unwrap(),
            points,
            triangles,
        };
        let (body, (inverse_mass, inverse_inertia)) = piece_body(&spec, [0., 1., 0., 0.]).unwrap();
        assert!((inverse_mass - 1. / 70.).abs() < 1e-6);
        assert!((inverse_inertia[0][0] - 6. / 70.).abs() < 1e-5);
        assert_eq!(body_pose(&body)[3], [5., 2., -3., 1.]);
        assert_eq!(f(&body, 0x224), 1.);
        assert_eq!(f(&body, 0x3f8), 1.);
        assert_eq!(f(&body, 0x3d4), 1.);
        assert_eq!(f(&body, 0x3b4), f32::MAX);
    }
}
