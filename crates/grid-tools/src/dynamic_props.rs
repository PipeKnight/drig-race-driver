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
//! A damage state's constraints become world joints when the entity breaks
//! (0x7D3AF0/0x7CC1E0, docs/JOINTS_RESEARCH.md): between its own pieces (tyre
//! walls), to the world's fixed object, and through an ENS link to the linked
//! entity's piece (armco, 0x7D5540), with the constraints' body overrides
//! (0x7B85A0). Uproots and other unported cases keep the entity static; see
//! `unported`. Pieces sleep and park (docs/BODY_SLEEPING.md). A cylinder piece
//! is round against the world, the cars and other cylinder or box pieces
//! (F549); box against box keeps an authored face-axis test.
use crate::{
    body_geometry::BodyGeometry, contact_surfaces::Surfaces, entity_geometry::Props, route::Route,
    track::Track,
};
use grid_formats::{entities, jpk, physparams, pssg};
use grid_world::{
    Error, box_contacts::{self, BoxShape}, collision_mesh, contact_groups, convex_distance,
    grouped_physical_frame, joint,
    joint_template, phantom_post, physical_constructor, physical_pose::Matrix,
    round_contacts::{self, CylinderShape, RoundShape, SphereShape},
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
/// Spare pool slots after the parallel piece contacts for them to equal the
/// serial ones: the largest capacity of one call (`Track::body_contacts`
/// allows 16).
const POOL_SLACK: usize = 16;
/// Live pieces from which [`PropWorld::contacts`] may use the compute pool.
const PARALLEL_PIECES: usize = 8;
/// Pool slots added per live piece, up to the world bound.
const SLOTS_PER_PIECE: usize = 12;
/// AUTHORED: a piece this far below the lowest road chunk has tunnelled
/// through the ground and is held (see [`PropWorld::hold_lost`]).
pub const LOST_DEPTH: f32 = 100.;
/// AUTHORED (owner's request of 23 October 2026, F564): the deepest a piece's
/// hull may end a step below the road under it (m) before
/// [`PropWorld::lift_sunk`] lifts it out. The original only recovers the
/// penetration through the solver over later steps; a tyre stack pressed by a
/// car then showed 3-30 cm under the road for a tick or two.
pub const SINK_TOLERANCE: f32 = 0.01;
/// Body words 0x1C0..0x230: mass centre, pose, angular and linear velocity.
const KINEMATICS: std::ops::Range<usize> = 0x1c0 / 4..0x230 / 4;
const CYLINDER_SIDES: usize = 12;
/// AUTHORED: solver rows reserved per live joint (three linear degrees of
/// freedom with a limit and a spring row each, the rotation and twist rows
/// and the friction rows stay below it).
const JOINT_ROWS: usize = 16;
/// Body byte +0x416: pairs with static geometry are scheduled (B18300,
/// B15B70); `dontCollideWithFixedObjects` clears it (0xA69560).
const COLLIDES_WITH_FIXED: usize = 0x416;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Primitive {
    Box([f32; 3]),
    Cylinder { radius: f32, height: f32 },
    Sphere { radius: f32 },
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
    /// Body descriptor +0xE8 (constructor +0xEC): 50 for entity pieces,
    /// the descriptor default FLT_MAX for a loose wheel (6174F0).
    pub limit: f32,
    hull: collision_mesh::Mesh,
    points: Vec<[f32; 4]>,
    triangles: Vec<[u32; 3]>,
    /// Entity cylinders and spheres collide as the original's round kinds 5
    /// and 3 ([`Self::collider`]); `hull` then serves the piece-pair test only.
    round: bool,
}

#[derive(Clone)]
pub struct Breakable {
    pub entity: usize,
    pub template: String,
    pub placement: [f32; 16],
    /// Route prop box indices of the intact (static) state.
    pub intact: Vec<usize>,
    /// Threshold and explosion of each intact box's shape behaviour (a shape
    /// without one: FLT_MAX); the close-drift flag's pole and base differ.
    pub limits: Vec<(f32, f32)>,
    /// The first behaviour's threshold and explosion.
    pub threshold: f32,
    pub explosion: f32,
    /// Route prop boxes of the damage state's static (`infMass`) pieces,
    /// switched on at the break.
    pub damaged: Vec<usize>,
    pub pieces: Vec<PieceSpec>,
    /// Template rigid-body id of each piece.
    pub bodies: Vec<String>,
    /// Pieces built static at the break (0x7CDCD0: the entity is a link
    /// primary, entity +0xF4, and the body a link target, template +0x42);
    /// 0x7D2D90 rebuilds one dynamic when a holder joins it.
    pub static_pieces: Vec<bool>,
    /// The damage state's constraints, in change order.
    pub joints: Vec<JointSpec>,
    /// The ENS links this entity holds that a `link<index>` constraint of it
    /// uses, in file order.
    pub links: Vec<LinkSpec>,
    /// Joints with `specificPostCollision` on a `close_drift_flag` entity
    /// (0x7CC1E0): their dynamic piece is a phantom post (collision
    /// cleared, shape +0x15); the post itself is not ported yet.
    pub posts: Vec<usize>,
}

/// A held ENS link (`TEMPLATEENTITYLINK`) to a primary breakable.
#[derive(Clone, Debug)]
pub struct LinkSpec {
    pub primary: usize,
    pub index: u32,
    /// The primary's piece the `TEMPLATECONSTRAINTLINK` names.
    pub piece: usize,
}

/// A `TEMPLATECONSTRAINT` of a breakable's damage state.
#[derive(Clone)]
pub struct JointSpec {
    /// Piece indices of `body1uri` (A) and `body2uri` (B); B `None` for
    /// "none"/"world" and `link<index>` (resolved when the joint is built).
    pub a: usize,
    pub b: Option<usize>,
    pub link: Option<u32>,
    /// The joint request (0x7C7B10) at the entity's unit scale.
    pub request: joint_template::Request,
    /// `dontCollideWithAny`: the shapes of A and B get the pair filter
    /// 0x7B62C0 (pieces of the entity joined through joints do not collide).
    pub dont_collide_with_any: bool,
    /// `dontCollideWithFixedObjects`: A and B lose body byte +0x416.
    pub dont_collide_with_fixed: bool,
}

/// A live world joint of a broken breakable.
#[derive(Clone, Debug)]
pub struct PropJoint {
    pub breakable: usize,
    /// Its constraint in `Breakable::joints`.
    pub spec: usize,
    /// Index in the world's joint list and the world bodies of A and B (B
    /// `None`: the world's fixed object or a static piece).
    pub world: usize,
    pub a: usize,
    pub b: Option<usize>,
    pub dont_collide_with_any: bool,
    /// Byte +0x48 after the last committed step.
    pub broken: bool,
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

/// A registered phantom post (0x7B6B10 / 0x7B6340): a close-drift flag's
/// pole joint, its pole body and the post geometry.
#[derive(Clone, Debug)]
pub struct PropPost {
    pub breakable: usize,
    /// Its joint in [`PropWorld::joints`].
    pub joint: usize,
    pub body: usize,
    pub post: phantom_post::Post,
}

/// A car's box for the post sweep (0x7C4830): the `CAR_` shape's world
/// matrix and half extents, and the body's motion this tick (centre +0x1C0
/// minus the previous +0x2A0).
#[derive(Clone, Copy, Debug)]
pub struct PostCar {
    pub shape: phantom_post::Box,
    pub motion: [f32; 4],
}

/// The manager's post list holds at most ten (0x7B6340).
const MAX_POSTS: usize = 10;

/// A break observed this session: the entity and the tick it broke on.
#[derive(Clone, Debug, PartialEq)]
pub struct BreakEvent {
    pub breakable: usize,
    pub template: String,
    pub tick: u64,
    pub force: f32,
}

/// [`PropWorld::road_depths`]: one body's hull bottom against the road.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoadDepth {
    pub body: usize,
    /// Road height minus the hull's lowest point (m); positive is sunk.
    pub depth: f32,
    pub centre: [f32; 3],
    pub ground: f32,
    pub asleep: bool,
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
    /// Pieces of broken entities still static (`Breakable::static_pieces`),
    /// as (breakable, piece).
    pub statics: std::collections::BTreeSet<(usize, usize)>,
    pub joints: Vec<PropJoint>,
    pub posts: Vec<PropPost>,
    /// The cars the last post update reported, once per touched post.
    pub post_touches: Vec<usize>,
    pub events: Vec<BreakEvent>,
    pending: Vec<(usize, [f32; 4], f32)>,
    base_slots: Option<usize>,
    /// Held kinematics of lost bodies, by world body index. Not by position
    /// among the pieces and wheels: a break appends pieces before the
    /// wheels, and a held wheel's kinematics then went to a new piece
    /// (FIDELITY_BACKLOG row 90).
    lost: BTreeMap<usize, [u32; KINEMATICS.end - KINEMATICS.start]>,
    /// Wheels lost by cars, after the pieces in contact order.
    pub wheels: Vec<LooseWheel>,
    /// Road casts of asleep pieces, shared by the world's copies.
    depth_memo: std::sync::Arc<std::sync::Mutex<DepthMemo>>,
}

/// A piece's road cast (`PropWorld::depths`) is a function of its collider,
/// its pose bits (+0x1D0..+0x210) and the static road chunks alone, so an
/// asleep piece's cast is looked up instead of repeated every tick: by world
/// body, kept with its (breakable, piece) and pose bits and used only on an
/// exact match, so a memo hit gives the cast's own result.
#[derive(Default)]
struct DepthMemo {
    road: Option<crate::track::Road>,
    entries: std::collections::HashMap<usize, MemoCast>,
}

/// (breakable, piece), pose bits, (depth, ground) or none (no road hit).
type MemoCast = ((usize, usize), [u32; 16], Option<(f32, f32)>);

fn f(b: &[u32], o: usize) -> f32 {
    f32::from_bits(b[o / 4])
}
fn set(b: &mut [u32], o: usize, v: f32) {
    b[o / 4] = v.to_bits();
}
fn byte(b: &[u32], o: usize) -> u8 {
    b[o / 4].to_le_bytes()[o % 4]
}
fn set_byte(b: &mut [u32], o: usize, v: u8) {
    let mut w = b[o / 4].to_le_bytes();
    w[o % 4] = v;
    b[o / 4] = u32::from_le_bytes(w);
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
        // An icosahedron through the sphere: the AUTHORED piece-pair test
        // only (the world, the cars and the road see the round sphere).
        Primitive::Sphere { radius } => {
            let phi = (1. + 5f32.sqrt()) * 0.5;
            let scale = radius / (1. + phi * phi).sqrt();
            let mut points = Vec::with_capacity(12);
            for (a, b) in [(1., phi), (-1., phi), (1., -phi), (-1., -phi)] {
                points.push([0., a * scale, b * scale, 1.]);
                points.push([a * scale, b * scale, 0., 1.]);
                points.push([b * scale, 0., a * scale, 1.]);
            }
            // Every triple of mutually adjacent vertices (edge length 2).
            let edge = 2. * scale;
            let near = |i: usize, j: usize| {
                let d: f32 = (0..3).map(|k| (points[i][k] - points[j][k]).powi(2)).sum();
                (d.sqrt() - edge).abs() < 1e-3 * edge
            };
            let mut triangles = Vec::new();
            for i in 0..12 {
                for j in i + 1..12 {
                    for k in j + 1..12 {
                        if near(i, j) && near(j, k) && near(i, k) {
                            triangles.push([i as u32, j as u32, k as u32]);
                        }
                    }
                }
            }
            let triangles = orient(&points, triangles);
            (points, triangles)
        }
    }
}

/// Unit-length placement rows (no entity scale).
fn unit_scale(m: &[f32; 16]) -> bool {
    (0..3).all(|r| {
        let l = (m[4 * r] * m[4 * r] + m[4 * r + 1] * m[4 * r + 1] + m[4 * r + 2] * m[4 * r + 2]).sqrt();
        (l - 1.).abs() < 1e-4
    })
}

fn reference(uri: &[u8]) -> Result<&str, &'static str> {
    physparams::reference(uri).ok_or("invalid physics property reference")
}

/// The body's mass and diagonal inertia from its one shape (B11520 sums the
/// shapes' vtable +8 mass and +0xC inertia; F549). Every kind measures the
/// shape grown by its shell (+0x220): box B3D480/B40880 over half extents
/// plus shell, cylinder B46460/B46C90 over radius plus shell and twice the
/// half height plus shell, sphere B4A5F0/B4AAB0 over radius plus shell.
/// The x87 products are evaluated in f64 and rounded once (the caller
/// stores the result as a float). The template dimensions are unscaled [I:
/// the piece shape descriptor's scale rule was not read].
fn mass_properties(primitive: Primitive, density: f32, shell: f32) -> (f32, [f32; 3]) {
    let (density, shell) = (f64::from(density), f64::from(shell));
    match primitive {
        Primitive::Box([x, y, z]) => {
            // B3D480: 2(hx + s) 2(hy + s) 2(hz + s) x density.
            let [x, y, z] = [x, y, z].map(|v| v * 0.5 + shell as f32);
            let m = ((2. * f64::from(x)) * f64::from(y) * 2. * f64::from(z) * 2. * density) as f32;
            // B40880: m / 12 (0.083333336) x the other two full extents squared.
            let k = m * f32::from_bits(0x3daa_aaab);
            let [x, y, z] = [x, y, z].map(|v| v * 2.);
            (m, [(z * z + y * y) * k, (x * x + z * z) * k, (y * y + x * x) * k])
        }
        Primitive::Cylinder { radius, height } => {
            // Shape +0x240 radius, +0x244 half height (B46180).
            let (r, h) = (radius + shell as f32, height * 0.5 + shell as f32);
            // B46460: (r + s)^2 x 2(h + s) x pi x density.
            let wide = f64::from(height * 0.5) + f64::from(shell as f32);
            let m = (f64::from(r) * f64::from(r) * (wide + wide)
                * f64::from(std::f32::consts::PI)
                * density) as f32;
            // B46C90: side m ((2(h + s))^2 / 12 + (r + s)^2 / 4), axis m (r + s)^2 / 2.
            let (l, r2) = (h * 2., r * r);
            let side = (l * l * f32::from_bits(0x3daa_aaab) + r2 * 0.25) * m;
            (m, [side, r2 * m * 0.5, side])
        }
        Primitive::Sphere { radius } => {
            // B4A5F0: (r + s)^3 x 4.18879 (CFAAF4) x density.
            let r = f64::from(radius) + shell;
            let m = (r * r * r * f64::from(f32::from_bits(0x4086_0a92)) * density) as f32;
            // B4AAB0: 0.4 m (r + s)^2.
            let r = r as f32;
            (m, [r * r * m * 0.4; 3])
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
            let bytes = std::fs::read(root.join(p)).map_err(|e| format!("{p}: {e}"))?;
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
        let mut damaged: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (i, b) in props.boxes.iter().enumerate() {
            if let Some(e) = b.entity {
                if b.damaged { &mut damaged } else { &mut boxes }
                    .entry(e)
                    .or_default()
                    .push(i);
            }
        }
        let all = entities::read(&placements, &types)?;
        let ids: BTreeMap<&[u8], usize> = all.iter().enumerate().map(|(i, e)| (e.id, i)).collect();
        // 0x7BDE30 links each resolved ENS link: the primary gets entity
        // +0xF4 and every `TEMPLATECONSTRAINTLINK` body template +0x42.
        let mut primaries = std::collections::BTreeSet::new();
        let mut targets = std::collections::BTreeSet::new();
        for e in &all {
            for l in &e.entity_links {
                if let Some(&p) = ids.get(l.primary) {
                    primaries.insert(p);
                    targets.extend(l.bodies.iter().copied());
                }
            }
        }
        let mut catalog = Self::default();
        'entities: for (index, e) in all.iter().enumerate() {
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
            // Several shape behaviours (the close-drift flag's pole and base)
            // break the same body into the same damage state.
            let Some(&(owner, behaviour)) = behaviours.first() else {
                continue;
            };
            // Only the flags are checked with several (other templates with
            // them, such as Yokohama's crates and signs, stay intact).
            if (behaviours.len() > 1 && !crate::entity_geometry::static_damage_ported(&template))
                || behaviours.iter().any(|(o, b)| {
                    o.component != owner.component || b.behaviour_type != behaviour.behaviour_type
                })
            {
                *catalog.unported.entry(template).or_default() += 1;
                continue;
            }
            if !behaviour.deleted.contains(&owner.component) || behaviour.added.is_empty() {
                *catalog.unported.entry(template).or_default() += 1;
                continue;
            }
            if behaviour.added.iter().any(|a| a.infinite_mass)
                && !crate::entity_geometry::static_damage_ported(&template)
            {
                *catalog.unported.entry(template).or_default() += 1;
                continue;
            }
            let (threshold, explosion) = parameters.behaviour(reference(behaviour.properties)?)?;
            // Per intact box, in the route reader's shape order.
            let mut limits = Vec::new();
            for shape in e.renderables.iter().flat_map(|r| &r.shapes).filter(|s| !s.phantom) {
                limits.push(match &shape.behaviour {
                    Some(b) => parameters.behaviour(reference(b.properties)?)?,
                    None => (f32::MAX, 0.),
                });
            }
            let placement = matrix(&e.transform);
            let mut pieces = Vec::new();
            let mut static_pieces = Vec::new();
            for added in &behaviour.added {
                // 0x7CDCD0 builds an `infMass` body static: no piece body; its
                // shapes are route boxes switched on at the break.
                if added.infinite_mass {
                    pieces.push(PieceSpec::shapeless(
                        String::from_utf8_lossy(added.node).into_owned(),
                        String::from_utf8_lossy(added.body_node).into_owned(),
                        placement,
                        parameters.body(reference(added.body_properties)?)?,
                    )?);
                    static_pieces.push(true);
                    continue;
                }
                if added.shapes.is_empty() {
                    let body = parameters.body(reference(added.body_properties)?)?;
                    if body.sphericalise != 0. {
                        *catalog.unported.entry(template).or_default() += 1;
                        continue 'entities;
                    }
                    pieces.push(PieceSpec::shapeless(
                        String::from_utf8_lossy(added.node).into_owned(),
                        String::from_utf8_lossy(added.body_node).into_owned(),
                        placement,
                        body,
                    )?);
                    static_pieces.push(primaries.contains(&index) && targets.contains(added.body));
                    continue;
                }
                // A piece kind the port cannot break (several shapes, static,
                // sphericalised, fixed mass, a sphere; first seen on Haruna):
                // the template stays static, like armco.
                let [shape] = &added.shapes[..] else {
                    *catalog.unported.entry(template).or_default() += 1;
                    continue 'entities;
                };
                if shape.phantom {
                    *catalog.unported.entry(template).or_default() += 1;
                    continue 'entities;
                }
                let body = parameters.body(reference(added.body_properties)?)?;
                if body.sphericalise != 0. {
                    *catalog.unported.entry(template).or_default() += 1;
                    continue 'entities;
                }
                let (_, source) = parameters.shape(reference(shape.properties)?)?;
                let physparams::MassSource::Density(density) = source else {
                    *catalog.unported.entry(template).or_default() += 1;
                    continue 'entities;
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
                    entities::Primitive::Sphere { radius } => Primitive::Sphere { radius },
                };
                let (mass, inertia) = mass_properties(primitive, density, shape.shell);
                if !(mass.is_finite() && mass > 0.) {
                    return Err("invalid prop piece mass".into());
                }
                // 0x7D3C40 hands the entity matrix to the body constructor
                // unnormalised (only the render path splits off the row
                // lengths), so a scaled placement (Nürburgring tyre stacks,
                // 0.9718) keeps its scale in the body basis; mass stays the
                // template's density x the unscaled shape grown by its shell.
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
                    limit: DESCRIPTOR_EC,
                    hull,
                    points,
                    triangles,
                    round: !matches!(primitive, Primitive::Box(_)),
                });
                static_pieces.push(primaries.contains(&index) && targets.contains(added.body));
            }
            let mut joints = Vec::new();
            let mut posts = Vec::new();
            // Body override per piece (0x7B85A0): the highest
            // `bodyOverridePriority` above -1 among the constraints on it.
            let mut overrides: Vec<Option<(i32, String)>> = vec![None; pieces.len()];
            if !behaviour.constraints.is_empty() && !unit_scale(&e.transform) {
                // 0x7CC1E0 scales the joint by the entity's scale vector (the
                // lane choice is not ported): scaled joined entities stay static.
                *catalog.unported.entry(template).or_default() += 1;
                continue;
            }
            for c in &behaviour.constraints {
                let body = |uri: &[u8]| {
                    let id = uri.strip_prefix(b"#")?;
                    behaviour.added.iter().position(|a| a.body == id)
                };
                // B: a piece, the world's fixed object ("none"/"world",
                // 0x958200) or the linked entity's piece (`link<index>`).
                let link = (c.body2.strip_prefix(b"link"))
                    .and_then(|n| std::str::from_utf8(n).ok()?.parse::<u32>().ok());
                let world = matches!(c.body2, b"none" | b"world");
                let b = body(c.body2);
                let (Some(a), true) = (body(c.body1), b.is_some() || world || link.is_some())
                else {
                    *catalog.unported.entry(template).or_default() += 1;
                    continue 'entities;
                };
                let props = parameters.constraint(reference(c.properties)?)?;
                // Uproots and the soft rotational friction callback (0x7C5210)
                // are not ported.
                if Some(a) == b
                    || props.uproot.is_some()
                    || props.soft_friction != props.hard_friction
                {
                    *catalog.unported.entry(template).or_default() += 1;
                    continue 'entities;
                }
                // 0x7B7EE0: a static A with a dynamic B is swapped.
                let (a, b) = match b {
                    Some(k) if static_pieces[a] && !static_pieces[k] => (k, Some(a)),
                    _ => (a, b),
                };
                // 0x7CC1E0: the post path only for an entity whose name holds
                // `close_drift_flag` (strstr against 0xCE8084; the template
                // name stands for the entity's [I]). A static A makes no joint,
                // so no post (the base's own constraint to the world).
                if props.specific_post_collision
                    && template.contains("close_drift_flag")
                    && !static_pieces[a]
                {
                    posts.push(joints.len());
                }
                if let Some(id) = &props.body_override {
                    let priority = props.body_override_priority;
                    for k in std::iter::once(a).chain(b) {
                        if priority > overrides[k].as_ref().map_or(-1, |o| o.0) {
                            overrides[k] = Some((priority, id.clone()));
                        }
                    }
                }
                let template_props = joint_template::Props {
                    linear_limit: props.linear_limit,
                    linear_spring: props.linear_spring,
                    spring_k: props.spring_k,
                    spring_s: props.spring_s,
                    rotation_type: props.rotation_type,
                    rotation: props.rotation,
                    twist_type: props.twist_type,
                    twist: props.twist,
                    break_linear: props.break_linear,
                    break_angular: props.break_angular,
                    hard_friction: props.hard_friction,
                    break_at_angular_extent: props.break_at_angular_extent,
                    rotations: props.rotations,
                };
                let transform = joint_template::template_transform(&c.transform);
                let frames = joint_template::template_frames(&transform, &template_props);
                joints.push(JointSpec {
                    a,
                    b,
                    link,
                    request: joint_template::request(&frames, &template_props, 1., c.display),
                    dont_collide_with_any: props.dont_collide_with_any,
                    dont_collide_with_fixed: props.dont_collide_with_fixed,
                });
            }
            // The post's dynamic body (A after the swap) collides with nothing
            // (0xA6A290 clears shape +0x15) and its body +0x3FC is FLT_MAX
            // (0xA692F0).
            for &k in &posts {
                let piece = &mut pieces[joints[k].a];
                piece.points.clear();
                piece.triangles.clear();
                piece.limit = f32::MAX;
            }
            for (spec, o) in pieces.iter_mut().zip(overrides) {
                let Some((_, id)) = o else { continue };
                let body = parameters.body(&id)?;
                // 0xB11520 after the shape sums: mass and inertia blend
                // towards a sphere of `sphericalmass` and `sphericalradius`.
                let s = body.sphericalise;
                if s > 0. {
                    let sphere = body.spherical_radius * body.spherical_radius
                        * body.spherical_mass
                        * f32::from_bits(0x3ecc_cccd);
                    for i in &mut spec.inertia {
                        *i += (sphere - *i) * s;
                    }
                    spec.mass = (body.spherical_mass - spec.mass) * s + spec.mass;
                }
                spec.body = body;
            }
            let intact = boxes.get(&index).cloned().unwrap_or_default();
            if intact.is_empty() {
                // No solid intact route shape (only phantom ones): the
                // template stays unported (static).
                *catalog.unported.entry(template).or_default() += 1;
                continue;
            }
            if limits.len() != intact.len() {
                return Err("breakable shape and box counts differ".into());
            }
            catalog.breakables.push(Breakable {
                entity: index,
                template,
                placement: e.transform,
                intact,
                limits,
                threshold,
                explosion,
                damaged: damaged.get(&index).cloned().unwrap_or_default(),
                pieces,
                bodies: behaviour
                    .added
                    .iter()
                    .map(|a| String::from_utf8_lossy(a.body).into_owned())
                    .collect(),
                static_pieces,
                joints,
                links: Vec::new(),
                posts,
            });
        }
        // The held links a `link<index>` constraint uses, to ported primaries.
        let by_entity: BTreeMap<usize, usize> = (catalog.breakables.iter().enumerate())
            .map(|(i, b)| (b.entity, i))
            .collect();
        for i in 0..catalog.breakables.len() {
            let mut links = Vec::new();
            for l in &all[catalog.breakables[i].entity].entity_links {
                let Some(index) = l.index else { continue };
                if !catalog.breakables[i].joints.iter().any(|j| j.link == Some(index)) {
                    continue;
                }
                let Some(&primary) = ids.get(l.primary).and_then(|p| by_entity.get(p)) else {
                    continue;
                };
                for body in &l.bodies {
                    let names = &catalog.breakables[primary].bodies;
                    if let Some(piece) = names.iter().position(|b| b.as_bytes() == *body) {
                        links.push(LinkSpec {
                            primary,
                            index,
                            piece,
                        });
                    }
                }
            }
            catalog.breakables[i].links = links;
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
fn body_settings(spec: &PieceSpec, linear: [f32; 4], angular: [f32; 4]) -> [u32; 88] {
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
    // turnOffTime (body +0x3B4): pieces and lost wheels sleep after this
    // long at rest (B0FE60, B24D70).
    set(&mut s, 0xa0, b.turn_off_time);
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
    set(&mut s, 0xec, spec.limit);
    for r in 0..4 {
        for c in 0..4 {
            set(&mut s, 0xf0 + r * 16 + c * 4, spec.pose[r][c]);
        }
    }
    // Supplied pose and active; directional friction and angular scale off.
    s[0x130 / 4] = 0x0000_0101;
    for k in 0..3 {
        set(&mut s, 0x140 + 4 * k, angular[k]);
        set(&mut s, 0x150 + 4 * k, linear[k]);
    }
    s
}

type Properties = (f32, Matrix);

fn piece_body(
    spec: &PieceSpec,
    linear: [f32; 4],
    angular: [f32; 4],
) -> Result<(contact_groups::Body, Properties), Error> {
    let out = physical_constructor::initialize(&body_settings(spec, linear, angular), 0, 0)?;
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

impl PieceSpec {
    /// The detached wheel's debris body (9D9F80 for wheel parts 0x2A..0x31):
    /// a cylinder (9A8BB0) about local Y, at world `pose`, with the 0.02
    /// shell, the given mass, a solid cylinder's inertia [I: the
    /// constructor derives it from the shape when no tensor is supplied] and
    /// 9D9F80's body words: angular damping 0.2, air resistance 0.01 x mass,
    /// no linear damping, stationary 0.1, no wake-up force, turn-off time 1,
    /// gravity scale 1, unbounded velocities.
    pub fn loose_wheel(radius: f32, height: f32, mass: f32, pose: Matrix) -> Result<Self, Error> {
        if ![radius, height, mass]
            .iter()
            .all(|v| v.is_finite() && *v > 0.)
        {
            return Err(Error("invalid loose wheel shape"));
        }
        let primitive = Primitive::Cylinder { radius, height };
        let side = mass * (3. * radius * radius + height * height) / 12.;
        let air = mass * f32::from_bits(0x3c23_d70a); // 0.01 (CB0DE4)
        let body = physparams::Body {
            sphericalise: 0.,
            spherical_radius: 1.,
            spherical_mass: 1.,
            max_angular_velocity: [f32::MAX; 3],
            max_linear_speed: f32::MAX,
            angular_damping: [f32::from_bits(0x3e4c_cccd); 3], // 0.2 (CB870C)
            linear_damping: 0.,
            forward_air_resistance: [air; 3],
            backward_air_resistance: [air; 3],
            linear_stationary: f32::from_bits(0x3dcc_cccd), // 0.1 (CAFCD4)
            angular_stationary: f32::from_bits(0x3dcc_cccd),
            wake_up_force: 0.,
            wake_up_torque: 0.,
            turn_off_time: 1.,
            gravity_scale: 1.,
        };
        let (points, triangles) = hull(primitive);
        let hull = collision_mesh::build(&points, &triangles, 0)?;
        Ok(Self {
            renderable: String::new(),
            anchor: String::new(),
            primitive,
            pose,
            mass,
            inertia: [side, mass * radius * radius * 0.5, side],
            body,
            shell: f32::from_bits(0x3ca3_d70a), // 0.02 (CB0934, 9A8BB0)
            limit: f32::MAX,
            hull,
            points,
            triangles,
            // The lost wheel keeps its prism (FIDELITY_BACKLOG row 24).
            round: false,
        })
    }

    /// A detached body panel's debris body (9D9F80 -> 9A87B0 for every part
    /// but the wheels): a box of full `size` (the inner box; the rounding
    /// `shell` grows it back to the node bounds), at world `pose`, with the
    /// given mass, a solid box's inertia over the node bounds and the lost
    /// wheel's body words but `angular_damping` (0.5 for the mirrors).
    pub fn loose_panel(
        size: [f32; 3],
        shell: f32,
        mass: f32,
        angular_damping: f32,
        pose: Matrix,
    ) -> Result<Self, Error> {
        if !size
            .iter()
            .chain([&shell, &mass])
            .all(|v| v.is_finite() && *v >= 0.)
            || mass <= 0.
        {
            return Err(Error("invalid loose panel shape"));
        }
        let mut spec = Self::loose_wheel(1., 1., mass, pose)?;
        let primitive = Primitive::Box(size);
        let outer = size.map(|v| v + 2. * shell);
        let [x, y, z] = outer.map(|v| v * v);
        let (points, triangles) = hull(primitive);
        spec.hull = collision_mesh::build(&points, &triangles, 0)?;
        spec.points = points;
        spec.triangles = triangles;
        spec.primitive = primitive;
        spec.inertia = [
            mass * (y + z) / 12.,
            mass * (x + z) / 12.,
            mass * (x + y) / 12.,
        ];
        spec.body.angular_damping = [angular_damping; 3];
        spec.shell = shell;
        Ok(spec)
    }

    /// A rigid body without shapes (the armco posts of Istanbul Park, Le
    /// Mans and the Nürburgring): 0xB11520 finds no shape mass and keeps
    /// mass 1 and a unit inertia before sphericalising; it collides with
    /// nothing. Its pose is the entity placement [I: the subentity carries
    /// no transform].
    fn shapeless(renderable: String, anchor: String, pose: Matrix, body: physparams::Body) -> Result<Self, Error> {
        let primitive = Primitive::Sphere { radius: 0. };
        // A placeholder mesh; `points` stay empty, so the piece never
        // enters the contact passes.
        let (points, triangles) = hull(Primitive::Box([0.01; 3]));
        let hull = collision_mesh::build(&points, &triangles, 0)?;
        Ok(Self {
            renderable,
            anchor,
            primitive,
            pose,
            mass: 1.,
            inertia: [1.; 3],
            body,
            shell: 0.,
            limit: DESCRIPTOR_EC,
            hull,
            points: Vec::new(),
            triangles: Vec::new(),
            round: false,
        })
    }

    /// Whether the piece has a collision shape.
    pub fn collides(&self) -> bool {
        !self.points.is_empty()
    }

    /// The piece's collision shape for the world, the cars and the road.
    pub fn collider(&self) -> convex_distance::Hull<'_> {
        match (self.round, self.primitive) {
            (true, Primitive::Cylinder { radius, height }) => convex_distance::Hull::Cylinder {
                radius,
                half_height: height * 0.5,
            },
            (true, Primitive::Sphere { radius }) => convex_distance::Hull::Sphere { radius },
            _ => convex_distance::Hull::Mesh(&self.hull),
        }
    }

    /// The round shape at world `pose`, for queries in which the piece is
    /// the static side (a car hull against it).
    fn round_shape(&self, pose: &Matrix) -> Option<RoundShape> {
        match (self.round, self.primitive) {
            (true, Primitive::Cylinder { radius, height }) => {
                Some(RoundShape::Cylinder(CylinderShape {
                    matrix: *pose,
                    radius,
                    half_height: height * 0.5,
                    margin: self.shell,
                }))
            }
            (true, Primitive::Sphere { radius }) => Some(RoundShape::Sphere(SphereShape {
                matrix: *pose,
                radius,
                margin: self.shell,
            })),
            _ => None,
        }
    }
}

/// [`PropWorld::pressed_down_at_spawn`] for one spec at its spawn pose;
/// `own` are the intact boxes of the piece's own breakable.
fn spawn_pressed(
    spec: &PieceSpec,
    own: &[usize],
    track: &Track,
    surfaces: &Surfaces,
    directions: &convex_distance::ContactSettings,
) -> Result<Option<Pressed>, Error> {
    let p = pressing_at(spec, spec.pose, own, track, surfaces, directions)?;
    Ok(if p.road_down {
        Some(Pressed::Road)
    } else if p.entity_down {
        Some(Pressed::EntityBox)
    } else {
        None
    })
}

/// Road penetration above which a piece counts as driven into the ground
/// (soak diagnostics, AUTHORED; resting pieces show about 0.03 m).
const DEEP_ROAD: f32 = 0.1;

/// The world contacts of a piece hull at `pose` that press it down: a road
/// contact with a downward normal (y < −0.5), a downward contact from a route
/// entity box other than `own` (authored stand-ins have no entity and do not
/// count), and whether any road contact is deeper than [`DEEP_ROAD`].
fn pressing_at(
    spec: &PieceSpec,
    pose: Matrix,
    own: &[usize],
    track: &Track,
    surfaces: &Surfaces,
    directions: &convex_distance::ContactSettings,
) -> Result<Pressing, Error> {
    let query = convex_distance::ContactSettings {
        margin: spec.shell,
        directions: directions.directions,
    };
    let boxes = track.props().map(|p| &p.boxes);
    let hits = track.body_contacts(
        spec.collider(),
        pose,
        &query,
        TRACK_CONTACTS,
        grid_world::MAX_WORLD_CONTACTS,
        |code| surfaces.sheet(code),
    )?;
    let down = |h: &&crate::track::BodyContact| h.contact.normal[1] < -0.5;
    Ok(Pressing {
        road: hits.iter().any(|h| h.prop.is_none()),
        road_down: hits.iter().filter(down).any(|h| h.prop.is_none()),
        entity_down: hits.iter().filter(down).any(|h| {
            h.prop.is_some_and(|i| {
                !own.contains(&i) && boxes.is_some_and(|b| b[i].entity.is_some())
            })
        }),
        deep_road: hits
            .iter()
            .any(|h| h.prop.is_none() && h.contact.penetration > DEEP_ROAD),
    })
}

/// [`PropWorld::pressing_now`]: what presses a body at its current pose.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pressing {
    /// Any road chunk or boundary sheet contact.
    pub road: bool,
    pub road_down: bool,
    pub entity_down: bool,
    pub deep_road: bool,
}

/// What presses a body down at its spawn pose
/// ([`PropWorld::pressed_down_at_spawn`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pressed {
    /// A road chunk triangle or the boundary sheet.
    Road,
    /// A route entity box (armco, tyre wall, another stack).
    EntityBox,
    /// No road chunk under the body's xz: it left the collision ground,
    /// which the original shares [I] (grid-soak's expected loss (i)).
    NoGround,
    /// No road chunk under the body's xz at its last contact of any kind,
    /// where its free fall began: it left the collision ground there and
    /// drifted over road while falling (grid-soak's expected loss (i) at the
    /// fall start, coordinator's approval of 26 October 2026).
    NoGroundAtFall,
    /// During the run, a downward route entity box contact in the same or
    /// the previous step as the body's first deep road contact: a body
    /// driven into the ground in one step (grid-soak's separate counter
    /// (ii); the open step-size family, docs/ROBUSTNESS_RESEARCH.md §9).
    EntityBoxInRun,
    /// Parked (status 2, B24DF0) while a deep road contact pressed it, then
    /// woken with no road contact since: the port's margin-based road
    /// contact cannot recover a body that deep (docs/FIDELITY_BACKLOG.md;
    /// parking freezes it there in the original too, F546).
    ParkedDeep,
}

/// A wheel a car lost (car +22E4 + `wheel`), or a body panel it lost
/// (`panel`, loose_panel), as a dynamic body.
#[derive(Clone)]
pub struct LooseWheel {
    pub car: usize,
    pub wheel: usize,
    /// The detached body part (`loose_parts::PARTS`); `wheel` is unused then.
    pub panel: Option<&'static str>,
    pub spec: std::sync::Arc<PieceSpec>,
    pub body: usize,
    pub token: u32,
}

/// A body's centre matrix (+0x190), as the joints and 0xA69740 read it.
fn body_centre(body: &contact_groups::Body) -> Matrix {
    std::array::from_fn(|r| std::array::from_fn(|c| f(body, 0x190 + r * 16 + c * 4)))
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
/// axes of both), for the pairs without a cylinder (box against box; a
/// sphere's inscribed icosahedron): the original's box/box AE8AD0 is not
/// ported for two moving boxes, and the triangle-soup pair query loses
/// stacked pieces once they overlap beyond the margin. Same convention as
/// the packed pair query: the normal pushes `a`, penetration includes the
/// margin.
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

/// The parking reference for a step (5C6B90 -> AD7D60 each frame): the
/// player car's physical body (the first car with byte +0x28 set; the
/// port's car 0), its published translation (A69630 copies the +0x1D0
/// matrix; row 3 is +0x200), at scale 1. Pieces park beyond 50 m of it
/// (+0x3FC 2500) and wake when it comes back within (AD6480).
pub fn park_reference(player: &contact_groups::Body) -> ([f32; 4], f32) {
    (std::array::from_fn(|k| f(player, 0x200 + 4 * k)), 1.)
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

/// Append a dynamic body to the active chain after its current tail (the
/// world double-buffers its active heads and tails by selector) with its own
/// shape token; returns its body index and token.
fn append(
    world: &mut grouped_physical_frame::Output,
    shapes: &mut Vec<contact_groups::Shape>,
    mut body: contact_groups::Body,
    properties: Properties,
) -> Result<(usize, u32), Error> {
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
    Ok((id - 1, shapes.len() as u32))
}

/// Unbroken joint links, the bodies whose shapes carry the
/// `dontCollideWithAny` filter and each piece's entity.
struct Exclusion {
    links: BTreeMap<usize, Vec<usize>>,
    filtered: std::collections::BTreeSet<usize>,
    entity: BTreeMap<usize, usize>,
}

impl Exclusion {
    /// B38D80 skips a pair an unbroken joint joins directly; the filter
    /// 0x7B62C0 on either body's shapes skips two pieces of the same entity
    /// joined through unbroken joints (0xAD68F0 walks them).
    fn excluded(&self, x: usize, y: usize) -> bool {
        let neighbours = |k: usize| self.links.get(&k).map_or(&[][..], Vec::as_slice);
        if neighbours(x).contains(&y) {
            return true;
        }
        if !self.filtered.contains(&x) && !self.filtered.contains(&y) {
            return false;
        }
        if !self.entity.contains_key(&x) || self.entity.get(&x) != self.entity.get(&y) {
            return false;
        }
        let mut seen = vec![x];
        let mut stack = vec![x];
        while let Some(k) = stack.pop() {
            for &next in neighbours(k) {
                if next == y {
                    return true;
                }
                if !seen.contains(&next) {
                    seen.push(next);
                    stack.push(next);
                }
            }
        }
        false
    }
}

/// The world a break writes into.
struct Make<'a> {
    world: &'a mut grouped_physical_frame::Output,
    shapes: &'a mut Vec<contact_groups::Shape>,
    tick: u64,
    disabled: &'a mut Vec<usize>,
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
            statics: Default::default(),
            joints: Vec::new(),
            posts: Vec::new(),
            post_touches: Vec::new(),
            events: Vec::new(),
            pending: Vec::new(),
            base_slots: None,
            lost: BTreeMap::new(),
            wheels: Vec::new(),
            depth_memo: Default::default(),
        }
    }

    /// This world with an empty road-depth memo (tests: memo against casts).
    #[cfg(test)]
    pub(crate) fn without_depth_memo(&self) -> Self {
        Self {
            depth_memo: Default::default(),
            ..self.clone()
        }
    }

    /// Whether any dynamic body (piece or wheel) is live.
    pub fn has_bodies(&self) -> bool {
        !self.pieces.is_empty() || !self.wheels.is_empty()
    }

    /// Every live body as (spec, world body, shape token): pieces, then wheels.
    fn parts(&self) -> Vec<(&PieceSpec, usize, u32)> {
        self.pieces
            .iter()
            .filter(|p| self.catalog.breakables[p.breakable].pieces[p.spec].collides())
            .map(|p| {
                (
                    &self.catalog.breakables[p.breakable].pieces[p.spec],
                    p.body,
                    p.token,
                )
            })
            .chain(self.wheels.iter().map(|w| (&*w.spec, w.body, w.token)))
            .collect()
    }

    /// Hold every piece that has fallen more than [`LOST_DEPTH`] below
    /// `floor` (the lowest road chunk bound): it keeps its position with zero
    /// velocity from then on. The original lets such a piece fall forever,
    /// harmlessly, as nothing there bounds coordinates; the port's 1e5
    /// coordinate domain (`Mesh::world_bounds`) would instead stop the whole
    /// world about 150 s later (F318 R06). Call after each committed step.
    pub fn hold_lost(&mut self, bodies: &mut [contact_groups::Body], floor: f32) {
        let indices: Vec<usize> = (self.pieces.iter().map(|p| p.body))
            .chain(self.wheels.iter().map(|w| w.body))
            .collect();
        for index in indices {
            let Some(body) = bodies.get_mut(index) else {
                continue;
            };
            let held = self.lost.entry(index);
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

    /// The narrow phase's pair exclusion (see [`Exclusion::excluded`]) for
    /// the joint states after the last committed step.
    fn exclusion(&self) -> Exclusion {
        let mut links: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for j in self.joints.iter().filter(|j| !j.broken) {
            let Some(b) = j.b else { continue };
            links.entry(j.a).or_default().push(b);
            links.entry(b).or_default().push(j.a);
        }
        Exclusion {
            links,
            filtered: (self.joints.iter())
                .filter(|j| j.dont_collide_with_any)
                .flat_map(|j| std::iter::once(j.a).chain(j.b))
                .collect(),
            entity: self.pieces.iter().map(|p| (p.body, p.breakable)).collect(),
        }
    }

    /// Whether world body `body` collides: false only for a live piece
    /// without a collision shape.
    pub fn collides(&self, body: usize) -> bool {
        (self.pieces.iter().find(|p| p.body == body))
            .is_none_or(|p| self.catalog.breakables[p.breakable].pieces[p.spec].collides())
    }

    /// Diagnostic (owner's derby report of 23 October 2026, tyres sinking
    /// into the road): for every live piece and lost wheel, how far the
    /// lowest point of its collision hull (shell excluded) lies below the
    /// packed track surface under its centre, from a cast starting 5 cm
    /// above the centre (a higher start hits the ledges of walls a piece
    /// leans on). Positive is below the road; pieces with no road under
    /// them are skipped.
    pub fn road_depths(&self, track: &Track, bodies: &[contact_groups::Body]) -> Vec<RoadDepth> {
        self.depths(track, bodies, false)
    }

    /// [`Self::road_depths`], with `parallel` in one run of pieces per
    /// compute thread (F568). Each cast is vertical, so the rectangle-cull
    /// cache's plane order cannot change a result and a cache per piece gives
    /// the same depths as one shared cache.
    fn depths(&self, track: &Track, bodies: &[contact_groups::Body], parallel: bool) -> Vec<RoadDepth> {
        let parts = self.parts();
        // Memo keys of `parts` (pieces, then lost wheels, which are not kept).
        let keys: Vec<Option<(usize, usize)>> = self
            .pieces
            .iter()
            .filter(|p| self.catalog.breakables[p.breakable].pieces[p.spec].collides())
            .map(|p| Some((p.breakable, p.spec)))
            .chain(self.wheels.iter().map(|_| None))
            .collect();
        let asleep = |b: &contact_groups::Body| f(b, 0x400).to_bits() == contact_groups::ASLEEP;
        let pose_bits =
            |b: &contact_groups::Body| -> [u32; 16] { std::array::from_fn(|i| b[0x1d0 / 4 + i]) };
        // (depth, ground) per part once known; None for a missing body.
        let mut casts: Vec<Option<Option<(f32, f32)>>> = vec![None; parts.len()];
        let mut todo = Vec::new();
        {
            let mut memo = self.depth_memo.lock().unwrap_or_else(|e| e.into_inner());
            if !memo.road.as_ref().is_some_and(|r| r.is(track)) {
                memo.road = Some(track.road());
                memo.entries.clear();
            }
            for (k, &(_, body, _)) in parts.iter().enumerate() {
                let Some(b) = bodies.get(body) else {
                    continue;
                };
                if let Some(key) = keys[k]
                    && asleep(b)
                    && let Some(&(kept, pose, cast)) = memo.entries.get(&body)
                    && kept == key
                    && pose == pose_bits(b)
                {
                    casts[k] = Some(cast);
                    continue;
                }
                todo.push(k);
            }
        }
        let runs = if parallel && todo.len() >= PARALLEL_PIECES {
            crate::parallel::threads().min(todo.len())
        } else {
            1
        };
        let per_run = todo.len().div_ceil(runs.max(1));
        let cast = |k: usize| {
            let (spec, body, _) = parts[k];
            let mut cache = grid_world::rectangle_cull::Cache::default();
            let pose = body_pose(&bodies[body]);
            let centre = pose[3];
            let lowest = match spec.collider() {
                convex_distance::Hull::Cylinder { radius, half_height } => {
                    let a = pose[1][1].clamp(-1., 1.);
                    centre[1] - (half_height * a.abs() + radius * (1. - a * a).max(0.).sqrt())
                }
                convex_distance::Hull::Sphere { radius } => centre[1] - radius,
                hull => hull.world_bounds(pose, 0.).ok()?[0][1],
            };
            let (start, end) = (centre[1] + 0.05, centre[1] - 100.);
            let hit = track
                .query([centre[0], start, centre[2], 1.], [centre[0], end, centre[2], 1.], &mut cache)
                .ok()
                .and_then(|q| q.hit)?;
            let ground = start + (end - start) * hit.contact.fraction;
            Some((ground - lowest, ground))
        };
        let computed = crate::parallel::map(runs, runs > 1, |r| {
            todo[(r * per_run).min(todo.len())..((r + 1) * per_run).min(todo.len())]
                .iter()
                .map(|&k| (k, cast(k)))
                .collect::<Vec<_>>()
        });
        let mut memo = self.depth_memo.lock().unwrap_or_else(|e| e.into_inner());
        for (k, result) in computed.into_iter().flatten() {
            let body = parts[k].1;
            if let Some(key) = keys[k]
                && asleep(&bodies[body])
            {
                memo.entries
                    .insert(body, (key, pose_bits(&bodies[body]), result));
            }
            casts[k] = Some(result);
        }
        drop(memo);
        parts
            .iter()
            .zip(casts)
            .filter_map(|(&(_, body, _), cast)| {
                let (depth, ground) = cast??;
                let b = &bodies[body];
                let centre = body_pose(b)[3];
                Some(RoadDepth {
                    body,
                    depth,
                    centre: [centre[0], centre[1], centre[2]],
                    ground,
                    asleep: asleep(b),
                })
            })
            .collect()
    }

    /// AUTHORED (see [`SINK_TOLERANCE`]): lift every live piece and wheel
    /// whose hull ends the step more than the tolerance below the road under
    /// its centre back to that tolerance, and stop it moving further down.
    /// Call after each committed step, after [`Self::hold_lost`]. With
    /// `parallel` the depths come from the compute pool (same values).
    pub fn lift_sunk(
        &self,
        track: &Track,
        bodies: &mut [contact_groups::Body],
        parallel: bool,
    ) -> Result<(), Error> {
        for d in self.depths(track, bodies, parallel) {
            if d.depth <= SINK_TOLERANCE || self.lost.contains_key(&d.body) {
                continue;
            }
            let body = &mut bodies[d.body];
            let mut position: [f32; 4] = std::array::from_fn(|k| f(body, 0x1c0 + k * 4));
            position[1] += d.depth - SINK_TOLERANCE;
            grid_world::continuous::apply(body, position)?;
            if f(body, 0x214) < 0. {
                set(body, 0x214, 0.);
            }
        }
        Ok(())
    }

    /// Whether world body `body` (a piece or a lost wheel) has been lost
    /// below the world and is held.
    pub fn is_lost(&self, body: usize) -> bool {
        self.lost.contains_key(&body)
    }

    /// Arm the intact breakable boxes: their threshold becomes the contact limit.
    pub fn arm(&self, track: &mut Track) -> Result<(), Error> {
        let props = track
            .props_mut()
            .ok_or(Error("dynamic props require route props"))?;
        let boxes = std::sync::Arc::make_mut(&mut props.boxes);
        for (i, b) in self.catalog.breakables.iter().enumerate() {
            for (&k, &(limit, _)) in b.intact.iter().zip(&b.limits) {
                let prop = boxes
                    .get_mut(k)
                    .ok_or(Error("breakable prop box out of range"))?;
                prop.limit = limit;
                prop.enabled = !self.broken[i];
            }
            for &k in &b.damaged {
                let prop = boxes
                    .get_mut(k)
                    .ok_or(Error("breakable prop box out of range"))?;
                prop.enabled = self.broken[i];
            }
        }
        Ok(())
    }

    /// Fresh contacts of every live piece: track (packed road, boundary and
    /// enabled route props), other pieces and the given car hulls. None
    /// feeds break detection: only car contacts break route props.
    ///
    /// With `parallel`, each piece's contacts are generated on the compute
    /// pool and joined in piece order (F568). They equal the serial ones when
    /// the pool keeps [`POOL_SLACK`] spare slots after them: then no capacity
    /// limited any call. Otherwise, or on an error, the serial pass runs, so
    /// the result is always the serial result.
    #[allow(clippy::too_many_arguments)]
    pub fn contacts(
        &self,
        track: &Track,
        surfaces: &Surfaces,
        bodies: &[contact_groups::Body],
        cars: &[CarHull<'_>],
        directions: &convex_distance::ContactSettings,
        remaining: usize,
        parallel: bool,
    ) -> Result<Vec<contact_groups::Contact>, Error> {
        let parts = self.parts();
        let exclusion = self.exclusion();
        let poses = parts
            .iter()
            .map(|p| {
                bodies
                    .get(p.1)
                    .map(body_pose)
                    .ok_or(Error("missing prop piece body"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        // Pairs with static geometry need body byte +0x416 (B18300); an
        // asleep body makes no pair with static geometry or with another
        // asleep body (B18300 takes pairs only from the active list).
        let asleep = |body: usize| {
            bodies
                .get(body)
                .is_some_and(|b| b[0x400 / 4] == contact_groups::ASLEEP)
        };
        let sleeping: Vec<bool> = parts.iter().map(|p| asleep(p.1)).collect();
        let awake: Vec<usize> = (0..parts.len()).filter(|&k| !sleeping[k]).collect();
        // A cheap upper bound of each hull's reach, to skip far pairs before
        // building hulls: the hull's centre is its pose's translation and the
        // pose is rigid, so a world point lies |local point| from the centre;
        // the slack covers rounding. Skipping on it drops only pairs the exact
        // test below also drops, so the contacts are unchanged.
        let loose: Vec<f32> = parts
            .iter()
            .map(|p| {
                let r = (p.0.points.iter())
                    .map(|q| (q[0] * q[0] + q[1] * q[1] + q[2] * q[2]).sqrt())
                    .fold(0f32, f32::max);
                r * 1.001 + 0.01
            })
            .collect();
        // World hulls on first use, with the largest centre-to-vertex reach.
        let convexes: Vec<std::sync::OnceLock<(Convex, f32)>> =
            (0..parts.len()).map(|_| std::sync::OnceLock::new()).collect();
        let hull = |k: usize| {
            let (convex, reach) = convexes[k].get_or_init(|| {
                let convex = Convex::new(parts[k].0, &poses[k]);
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
            });
            (convex, *reach)
        };
        // Each car's pose and bounds once per call, on first use.
        let placed: Vec<std::sync::OnceLock<Result<crate::body_geometry::Placed, Error>>> =
            (0..cars.len()).map(|_| std::sync::OnceLock::new()).collect();
        let placed = &placed[..];
        // Piece `k`'s contacts with `room0` pool slots left before it.
        let piece = |k: usize, room0: usize| -> Result<Vec<contact_groups::Contact>, Error> {
            let (spec, piece_body, piece_token) = parts[k];
            let mut out = Vec::new();
            let query = convex_distance::ContactSettings {
                margin: spec.shell,
                directions: directions.directions,
            };
            let pose = poses[k];
            let room = room0 - out.len();
            if room == 0 {
                return Ok(out);
            }
            let fixed = bodies
                .get(piece_body)
                .is_some_and(|b| byte(b, COLLIDES_WITH_FIXED) != 0)
                && !asleep(piece_body);
            let track_hits = if fixed {
                track.body_contacts(spec.collider(), pose, &query, TRACK_CONTACTS, room, |code| {
                    surfaces.sheet(code)
                })?
            } else {
                Vec::new()
            };
            for hit in track_hits {
                let mut c = [0; 32];
                c[1] = piece_body as u32 + 1;
                c[0xc / 4] = 0x01000000;
                c[0x1c / 4] = u32::MAX;
                c[0x20 / 4] = u32::MAX;
                c[0x24 / 4] = 1;
                c[0x28 / 4] = piece_token;
                c[0x2c / 4] = hit.shape + 1;
                c[0x38 / 4] = hit.contact.penetration.to_bits();
                // A piece and a route prop are both entity shapes (0x7C6350:
                // bytes 0x17 = 0, 0x228 = 0), so B0EE80 leaves their contact
                // limit at FLT_MAX and 0x7C46A0 never calls the break check
                // 0x7BDFD0 for them: pieces neither break intact stacks nor
                // pass through them at the threshold cap. Only a car shape
                // (car_box, byte 0x17 = 1) pairs with a prop's 0x18.
                c[0x3c / 4] = if hit.prop.is_some() { f32::MAX } else { hit.limit }.to_bits();
                c[0x50 / 4..0x60 / 4].copy_from_slice(&hit.contact.point.map(f32::to_bits));
                c[0x60 / 4..0x70 / 4].copy_from_slice(&hit.contact.normal.map(f32::to_bits));
                c[0x70 / 4] = (u32::from(hit.contact.flipped) << 8) | (1 << 24);
                out.push(c);
            }
            // An asleep piece pairs only with the awake pieces after it (the
            // asleep ones would be skipped below), in the same order.
            let later: Vec<usize> = if sleeping[k] {
                awake.iter().copied().filter(|&j| j > k).collect()
            } else {
                (k + 1..parts.len()).collect()
            };
            for j in later {
                let (other_spec, other_body, other_token) = parts[j];
                let room = room0 - out.len();
                if room == 0 {
                    break;
                }
                let margin = spec.shell.max(other_spec.shell);
                // The hull centres are the poses' translations.
                let d = [
                    poses[k][3][0] - poses[j][3][0],
                    poses[k][3][1] - poses[j][3][1],
                    poses[k][3][2] - poses[j][3][2],
                    0.,
                ];
                let bound = loose[k] + loose[j] + margin + 0.01;
                if dot3(d, d) > 3. * bound * bound {
                    continue;
                }
                let (a, a_reach) = hull(k);
                let (b, b_reach) = hull(j);
                // Each hull's face normals hold an orthonormal triple (box
                // faces; prism cap and radials 90 degrees apart), so beyond
                // sqrt(3) x the summed reach one of them already separates
                // the pair and the face-axis test returns nothing.
                let reach = a_reach + b_reach + margin + 0.01;
                if dot3(d, d) > 3. * reach * reach {
                    continue;
                }
                if exclusion.excluded(piece_body, other_body)
                    || (asleep(piece_body) && asleep(other_body))
                {
                    continue;
                }
                // A cylinder piece against a cylinder or box piece: AE8AD0
                // with the round support and AE2FA0's handlers (F549), the
                // cylinder as the moving side and the box with its B3D890
                // face; other pairs keep the AUTHORED face-axis test.
                let capacity = room.min(PAIR_CONTACTS);
                let directions = directions.directions;
                let cylinder = |s: &PieceSpec| {
                    matches!(s.collider(), convex_distance::Hull::Cylinder { .. })
                };
                let block = |s: &PieceSpec, pose: &Matrix| match (s.round, s.primitive) {
                    (false, Primitive::Box(d)) => Some(BoxShape {
                        matrix: *pose,
                        half: d.map(|v| v * 0.5),
                        margin: s.shell,
                    }),
                    _ => None,
                };
                let (hits, order) = if let (true, Some(shape @ RoundShape::Cylinder(_))) =
                    (cylinder(spec), other_spec.round_shape(&poses[j]))
                {
                    let hits = round_contacts::pair_contacts(spec.collider(), pose, &shape, &query, capacity)?;
                    (hits, false)
                } else if let (true, Some(shape)) = (cylinder(spec), block(other_spec, &poses[j])) {
                    let hits = box_contacts::contacts(spec.collider(), pose, &shape, &query, capacity)?;
                    (hits, false)
                } else if let (Some(shape), true) = (block(spec, &pose), cylinder(other_spec)) {
                    let query = convex_distance::ContactSettings {
                        margin: other_spec.shell,
                        directions,
                    };
                    let hits = box_contacts::contacts(
                        other_spec.collider(),
                        poses[j],
                        &shape,
                        &query,
                        capacity,
                    )?;
                    (hits, true)
                } else {
                    (convex_contacts(a, b, margin, capacity), false)
                };
                // The normal pushes the first body: the moving cylinder.
                let (bodies, tokens) = if order {
                    ([other_body, piece_body], [other_token, piece_token])
                } else {
                    ([piece_body, other_body], [piece_token, other_token])
                };
                for hit in hits {
                    out.push(pair_record(&hit, bodies, tokens));
                }
            }
            let mut world_hull = None;
            for (c, car) in cars.iter().enumerate() {
                let room = room0 - out.len();
                if room == 0 {
                    break;
                }
                let body = bodies.get(car.body).ok_or(Error("missing car body"))?;
                let capacity = room.min(TRACK_CONTACTS);
                // The car's pose and bounds as `car_contacts_*` compute them
                // first, once per car; skipped where those return before them.
                let place = |placed: &[std::sync::OnceLock<Result<crate::body_geometry::Placed, Error>>]| {
                    placed[c].get_or_init(|| car.geometry.placed(body)).clone()
                };
                let hits = if let Some(shape) = spec.round_shape(&pose) {
                    if capacity == 0 {
                        Vec::new()
                    } else {
                        let at = place(placed)?;
                        car.geometry.car_contacts_round_at(&at, &shape, capacity)?
                    }
                } else if spec.triangles.is_empty() || spec.triangles.len() > 4096 || capacity == 0 {
                    Vec::new()
                } else {
                    let at = place(placed)?;
                    // The hull's bounds lie within its reach of the centre:
                    // a car clear of that box is clear of the hull's bounds,
                    // where `car_contacts_hull` returns nothing.
                    let centre = pose[3];
                    if (0..3).any(|j| {
                        at.bounds[0][j] > centre[j] + loose[k] || at.bounds[1][j] < centre[j] - loose[k]
                    }) {
                        Vec::new()
                    } else {
                        let hull = world_hull.get_or_insert_with(|| {
                            crate::body_geometry::WorldHull::new(&world_triangles(spec, &pose))
                        });
                        car.geometry.car_contacts_hull_at(&at, hull, capacity)?
                    }
                };
                for hit in hits {
                    out.push(pair_record(
                        &hit,
                        [car.body, piece_body],
                        [car.token, piece_token],
                    ));
                }
            }
            Ok(out)
        };
        if parallel && parts.len() >= PARALLEL_PIECES && crate::parallel::threads() > 1 {
            let lists = crate::parallel::map(parts.len(), true, |k| piece(k, remaining));
            let total = lists
                .iter()
                .try_fold(0usize, |n, l| l.as_ref().ok().map(|l| n + l.len()));
            if total.is_some_and(|n| n + POOL_SLACK <= remaining) {
                let mut out = Vec::with_capacity(total.unwrap_or(0));
                for list in lists {
                    out.extend(list?);
                }
                return Ok(out);
            }
        }
        let mut out = Vec::new();
        for k in 0..parts.len() {
            let room0 = remaining - out.len();
            if room0 == 0 {
                break;
            }
            out.extend(piece(k, room0)?);
        }
        Ok(out)
    }

    /// The original's continuous stage for the pieces and lost wheels
    /// (world phase 2, before the contacts; grid_world::continuous): a body
    /// that moved more than 0.75 of its hull radius this step is swept
    /// against the road chunks, and the time-of-impact responses (B3A700)
    /// move it back (B14550). Returns the number of bodies moved. Cars, the
    /// boundary sheet, route boxes and piece pairs are not swept here
    /// (docs/ROBUSTNESS_RESEARCH.md).
    pub fn continuous(
        &self,
        track: &Track,
        surfaces: &Surfaces,
        bodies: &mut [contact_groups::Body],
        directions: &convex_distance::ContactSettings,
    ) -> Result<usize, Error> {
        let mut records = 0;
        let mut moved = 0;
        for (spec, index, _) in self.parts() {
            let body = bodies.get(index).ok_or(Error("missing prop piece body"))?;
            // B15B70 sweeps against the road only with body byte +0x416.
            if byte(body, COLLIDES_WITH_FIXED) == 0 {
                continue;
            }
            let (displacement, previous, start, end) = grid_world::continuous::step(body);
            if !grid_world::continuous::is_fast(grid_world::continuous::radius(spec.collider()), displacement) {
                continue;
            }
            let query = convex_distance::ContactSettings {
                margin: spec.shell,
                directions: directions.directions,
            };
            let room = grid_world::continuous::MAX_RECORDS - records;
            let hits = track.sweep_road(spec.collider(), start, end, &query, room, |code| {
                surfaces.sheet(code)
            })?;
            records += hits.len();
            let inverse_mass = f32::from_bits(body[0x3f4 / 4]);
            let positions: Vec<_> = hits
                .iter()
                .filter_map(|h| {
                    grid_world::continuous::response(h, inverse_mass, displacement, previous)
                })
                .collect();
            if !positions.is_empty() {
                let position = grid_world::continuous::mean(&positions)?;
                grid_world::continuous::apply(&mut bodies[index], position)?;
                moved += 1;
            }
            if records == grid_world::continuous::MAX_RECORDS {
                break;
            }
        }
        Ok(moved)
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
            let Some(&(threshold, explosion)) = (entry.intact.iter())
                .position(|&k| k == record.prop)
                .and_then(|i| entry.limits.get(i))
            else {
                continue;
            };
            let force = f(c, 0x3c);
            if self.broken[breakable]
                || self.pending.iter().any(|p| p.0 == breakable)
                // NaN never breaks, as in the original comparison.
                || threshold.partial_cmp(&(force * BREAK_TOLERANCE))
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
            let explosion = normal.map(|n| explosion * along * n);
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
        for j in &mut self.joints {
            j.broken = world
                .groups
                .joints
                .get(j.world)
                .is_none_or(|w| w.joint.byte(0x48) != 0);
        }
        if self.pending.is_empty() {
            return Ok(Vec::new());
        }
        self.base_slots.get_or_insert(world.groups.contacts.len());
        let mut disabled = Vec::new();
        // 0x7D64B0: each queued entity breaks (0x7D5180), then its ENS links
        // are followed with propagation (0x7D5540).
        for (breakable, explosion, force) in std::mem::take(&mut self.pending) {
            if self.broken[breakable] {
                continue;
            }
            let mut make = Make {
                world: &mut *world,
                shapes: &mut *shapes,
                tick,
                disabled: &mut disabled,
            };
            if self.break_entity(&mut make, breakable, explosion, force)? {
                self.follow_links(&mut make, breakable, true)?;
            }
        }
        self.reserve(world);
        Ok(disabled)
    }

    /// The world body of a live dynamic piece.
    fn piece_body(&self, breakable: usize, spec: usize) -> Option<usize> {
        (self.pieces.iter())
            .find(|p| p.breakable == breakable && p.spec == spec)
            .map(|p| p.body)
    }

    fn spawn(
        &mut self,
        make: &mut Make<'_>,
        breakable: usize,
        spec_index: usize,
        explosion: [f32; 4],
    ) -> Result<usize, Error> {
        let spec = &self.catalog.breakables[breakable].pieces[spec_index];
        let linear = explosion_velocity(
            [spec.pose[3][0], spec.pose[3][1], spec.pose[3][2]],
            explosion,
        );
        let (body, properties) = piece_body(spec, linear, [0.; 4])?;
        let (body, token) = append(make.world, make.shapes, body, properties)?;
        self.pieces.push(Piece {
            breakable,
            spec: spec_index,
            body,
            token,
        });
        Ok(body)
    }

    /// 0x7D5180 / 0x7D3C40: the entity's pieces, then its constraints'
    /// joints. False when 0x7D4940 finds no room (the entity stays intact).
    fn break_entity(
        &mut self,
        make: &mut Make<'_>,
        breakable: usize,
        explosion: [f32; 4],
        force: f32,
    ) -> Result<bool, Error> {
        let entry = self.catalog.clone();
        let entry = &entry.breakables[breakable];
        // 0x7D5180 swaps only when 0x7D4940 finds room for the new
        // components; otherwise the entity stays intact. The original
        // first restores the farthest broken entities (0x7D4770), which
        // the port omits.
        let room = entry.pieces.len() + entry.static_pieces.iter().filter(|s| **s).count();
        if make.world.groups.bodies.len() + room > grid_world::MAX_WORLD_BODIES {
            return Ok(false);
        }
        make.disabled.extend(entry.intact.iter().chain(&entry.damaged).copied());
        for spec_index in 0..entry.pieces.len() {
            if entry.static_pieces[spec_index] {
                self.statics.insert((breakable, spec_index));
            } else {
                self.spawn(make, breakable, spec_index, explosion)?;
            }
        }
        self.broken[breakable] = true;
        for k in 0..entry.joints.len() {
            self.make_joint(make, breakable, k)?;
        }
        self.events.push(BreakEvent {
            breakable,
            template: entry.template.clone(),
            tick: make.tick,
            force,
        });
        Ok(true)
    }

    /// 0x7D3AF0 for constraint `k` of `breakable`: resolves A and B
    /// (0x7B7EE0; `link<index>` through the held links, 0x7B7160, the first
    /// with a broken primary in the holder's list order, newest link first),
    /// makes a static link target dynamic (0x7D2D90), then 0x7CC1E0 sets the
    /// collision flags and A74670 builds the joint (AEFEB0) on the current
    /// poses; B825E0 links it into the world list. A missing or static B is
    /// the world's fixed object; a static A makes no joint (its only case is
    /// a primary's post to the world).
    fn make_joint(&mut self, make: &mut Make<'_>, breakable: usize, k: usize) -> Result<(), Error> {
        let catalog = self.catalog.clone();
        let entry = &catalog.breakables[breakable];
        let spec = &entry.joints[k];
        let Some(a) = self.piece_body(breakable, spec.a) else {
            return Ok(());
        };
        let b = match (spec.b, spec.link) {
            (Some(piece), _) => self.piece_body(breakable, piece),
            (None, Some(index)) => {
                match (entry.links.iter().rev())
                    .find(|l| l.index == index && self.broken[l.primary])
                {
                    Some(l) => Some(self.make_dynamic(make, l.primary, l.piece)?),
                    None => None,
                }
            }
            (None, None) => None,
        };
        if spec.dont_collide_with_fixed {
            for body in std::iter::once(a).chain(b) {
                set_byte(&mut make.world.groups.bodies[body], COLLIDES_WITH_FIXED, 0);
            }
        }
        // B825E0: a joint between an asleep and an awake body wakes the
        // asleep one into the awake one's list.
        if let Some(b) = b {
            let (sa, sb) = (
                grouped_physical_frame::asleep(make.world, a),
                grouped_physical_frame::asleep(make.world, b),
            );
            if sa != sb {
                grouped_physical_frame::wake(make.world, if sa { a } else { b });
            }
        }
        let placement = matrix(&entry.placement);
        let mut descriptor =
            joint_template::descriptor(&spec.request, &placement, [1.; 3], b.is_none());
        let mut made = joint::Joint::filled(0);
        joint::construct(
            &mut made,
            &mut descriptor,
            &joint::Bodies {
                a: &make.world.groups.bodies[a],
                b: b.map(|b| &make.world.groups.bodies[b]),
                fixed: b.is_none(),
            },
            0,
        );
        // 0xAD9220 / 0x7B6B10 / 0x7B6340: the post, while the list has room.
        if entry.posts.contains(&k) && self.posts.len() < MAX_POSTS {
            self.posts.push(PropPost {
                breakable,
                joint: self.joints.len(),
                body: a,
                post: phantom_post::Post::default(),
            });
        }
        self.joints.push(PropJoint {
            breakable,
            spec: k,
            world: make.world.groups.joints.len(),
            a,
            b,
            dont_collide_with_any: spec.dont_collide_with_any,
            broken: false,
        });
        make.world.groups.joints.push(contact_groups::WorldJoint {
            joint: made,
            a: a as u32 + 1,
            b: b.map_or(0, |b| b as u32 + 1),
            fixed: None,
        });
        Ok(())
    }

    /// 0x7C4830, once per frame after the step and the breaks (0x5F0EE0):
    /// every post is refreshed from its joint's pivot and its pole's centre
    /// (0x7BB910); then each car's box sweeps every post (0x7BD240). A post a
    /// car turned past acos(0.99999) turns its pole about the pivot with zero
    /// velocity. Returns the cars, by index in `cars`, once per touched post:
    /// each gets the 1e-6 body contact (0xA01910) the drift listener sees.
    pub fn update_posts(
        &mut self,
        world: &mut grouped_physical_frame::Output,
        cars: &[PostCar],
    ) -> Result<Vec<usize>, Error> {
        // The world joint of each live post (a removed joint drops its post,
        // 0x7B6390).
        let worlds: Vec<Option<usize>> = (self.posts.iter())
            .map(|p| {
                let j = self.joints.get(p.joint)?;
                (!j.broken && j.world < world.groups.joints.len()).then_some(j.world)
            })
            .collect();
        let refresh = |world: &grouped_physical_frame::Output, p: &mut PropPost, w: usize| {
            let words = &world.groups.joints[w].joint.words;
            let anchor = std::array::from_fn(|k| f32::from_bits(words[0xa0 / 4 + k]));
            let centre = body_centre(&world.groups.bodies[p.body]);
            phantom_post::refresh(&mut p.post, phantom_post::pivot(anchor, centre), centre[3]);
        };
        for (p, w) in self.posts.iter_mut().zip(&worlds) {
            if let Some(w) = *w {
                refresh(world, p, w);
            }
        }
        let mut touched = Vec::new();
        for (i, car) in cars.iter().enumerate() {
            let bounds = phantom_post::swept_bounds(&car.shape, car.motion);
            let start = std::array::from_fn(|k| car.shape.centre[k] - car.motion[k]);
            for (p, w) in self.posts.iter_mut().zip(&worlds) {
                let Some(w) = *w else { continue };
                let Some(touch) = phantom_post::sweep(&mut p.post, &car.shape, start, bounds) else {
                    continue;
                };
                if let Some((axis, cosine)) = touch.turn {
                    let body = &mut world.groups.bodies[p.body];
                    let turned =
                        phantom_post::turn_body(body_centre(body), p.post.pivot, axis, cosine);
                    phantom_post::place(body, turned)?;
                }
                refresh(world, p, w);
                touched.push(i);
            }
        }
        Ok(touched)
    }

    /// Remove a live joint (0xAD9260): the port keeps its record broken.
    fn remove_joint(&mut self, world: &mut grouped_physical_frame::Output, j: usize) {
        // B82430: an asleep A wakes its island, an awake A and an awake
        // dynamic B restart their sleep timers.
        let (a, b) = (self.joints[j].a, self.joints[j].b);
        if !self.joints[j].broken && a < world.groups.bodies.len() {
            if grouped_physical_frame::asleep(world, a) {
                grouped_physical_frame::wake(world, a);
            } else {
                grouped_physical_frame::restart_timer(world, a);
            }
            if let Some(b) = b.filter(|&b| b < world.groups.bodies.len())
                && !grouped_physical_frame::asleep(world, b)
            {
                grouped_physical_frame::restart_timer(world, b);
            }
        }
        self.joints[j].broken = true;
        if let Some(w) = world.groups.joints.get_mut(self.joints[j].world) {
            set_byte(&mut w.joint.words, 0x48, 1);
        }
    }

    /// 0x7D2D90: a static link target becomes a dynamic body at its spawn
    /// pose; the entity's joints on it are removed and those between two
    /// bodies that were not broken are made again.
    fn make_dynamic(
        &mut self,
        make: &mut Make<'_>,
        breakable: usize,
        piece: usize,
    ) -> Result<usize, Error> {
        if !self.statics.remove(&(breakable, piece)) {
            return self
                .piece_body(breakable, piece)
                .ok_or(Error("link target piece missing"));
        }
        let catalog = self.catalog.clone();
        let specs = &catalog.breakables[breakable].joints;
        let on = |k: usize| specs[k].a == piece || specs[k].b == Some(piece);
        let mut was_broken = BTreeMap::new();
        for j in 0..self.joints.len() {
            let joint = &self.joints[j];
            if joint.breakable == breakable && on(joint.spec) {
                was_broken.insert(joint.spec, joint.broken);
                self.remove_joint(make.world, j);
            }
        }
        let body = self.spawn(make, breakable, piece, [0.; 4])?;
        for (k, spec) in specs.iter().enumerate() {
            if on(k) && spec.b.is_some() && was_broken.get(&k) != Some(&true) {
                self.make_joint(make, breakable, k)?;
            }
        }
        Ok(body)
    }

    /// 0x7D5540 after `x` broke. Links where `x` is the primary (its +0xBC
    /// list, newest first): a broken holder's unbroken joint of the link's
    /// constraint is made again (now to `x`'s piece); an intact holder
    /// breaks too when `propagate`. Then, with `propagate`, links `x` holds
    /// (+0xC0): an intact primary breaks. A break by propagation follows
    /// its own links without propagating.
    fn follow_links(&mut self, make: &mut Make<'_>, x: usize, propagate: bool) -> Result<(), Error> {
        let catalog = self.catalog.clone();
        for h in (0..catalog.breakables.len()).rev() {
            for link in catalog.breakables[h].links.iter().rev().filter(|l| l.primary == x) {
                for (k, spec) in catalog.breakables[h].joints.iter().enumerate() {
                    if spec.link != Some(link.index) {
                        continue;
                    }
                    if self.broken[h] {
                        let live = (0..self.joints.len()).rev().find(|&j| {
                            self.joints[j].breakable == h && self.joints[j].spec == k
                        });
                        if let Some(j) = live.filter(|&j| !self.joints[j].broken) {
                            self.remove_joint(make.world, j);
                            self.make_joint(make, h, k)?;
                        }
                    } else if propagate && self.break_entity(make, h, [0.; 4], 0.)? {
                        self.follow_links(make, h, false)?;
                    }
                }
            }
        }
        if propagate {
            for link in catalog.breakables[x].links.iter().rev() {
                let p = link.primary;
                let used = (catalog.breakables[x].joints.iter()).any(|j| j.link == Some(link.index));
                if used && !self.broken[p] && self.break_entity(make, p, [0.; 4], 0.)? {
                    self.follow_links(make, p, false)?;
                }
            }
        }
        Ok(())
    }

    /// Contact slots for every live body, beyond the cars' pool.
    fn reserve(&mut self, world: &mut grouped_physical_frame::Output) {
        let base = *self.base_slots.get_or_insert(world.groups.contacts.len());
        let count = self.pieces.len() + self.wheels.len();
        let slots = (base + SLOTS_PER_PIECE * count).min(grid_world::MAX_WORLD_CONTACTS);
        let rows = (slots * 4 + JOINT_ROWS * self.joints.len()).min(grid_world::MAX_SOLVER_ROWS);
        if slots > world.groups.contacts.len() {
            world
                .groups
                .contacts
                .resize(slots, grid_world::contact_groups::EMPTY_CONTACT);
        }
        if rows > world.manager.rows.len() {
            world.manager.rows.resize(rows, [0; 48]);
            world.manager.capacity = rows;
        }
    }

    /// Add a lost wheel as a dynamic body after a committed step (9DC270
    /// state 4 creates it in the scene update, so it takes part from the
    /// next step on). Refused beyond the world's body bound, as a break is.
    #[allow(clippy::too_many_arguments)]
    pub fn add_wheel(
        &mut self,
        world: &mut grouped_physical_frame::Output,
        shapes: &mut Vec<contact_groups::Shape>,
        car: usize,
        wheel: usize,
        spec: PieceSpec,
        linear: [f32; 4],
        angular: [f32; 4],
    ) -> Result<bool, Error> {
        if world.groups.bodies.len() + 1 > grid_world::MAX_WORLD_BODIES {
            return Ok(false);
        }
        self.base_slots.get_or_insert(world.groups.contacts.len());
        let (body, properties) = piece_body(&spec, linear, angular)?;
        let (body, token) = append(world, shapes, body, properties)?;
        self.wheels.push(LooseWheel {
            car,
            wheel,
            panel: None,
            spec: std::sync::Arc::new(spec),
            body,
            token,
        });
        self.reserve(world);
        Ok(true)
    }

    /// Add a detached body panel of car `car` as a dynamic body, as
    /// [`Self::add_wheel`] adds a lost wheel (9DC270 state 4 for a body part).
    #[allow(clippy::too_many_arguments)]
    pub fn add_panel(
        &mut self,
        world: &mut grouped_physical_frame::Output,
        shapes: &mut Vec<contact_groups::Shape>,
        car: usize,
        panel: &'static str,
        spec: PieceSpec,
        linear: [f32; 4],
        angular: [f32; 4],
    ) -> Result<bool, Error> {
        let added = self.add_wheel(world, shapes, car, usize::MAX, spec, linear, angular)?;
        if added && let Some(last) = self.wheels.last_mut() {
            last.panel = Some(panel);
        }
        Ok(added)
    }

    /// Detached panels for presentation: (car, part, pose at the loss, pose now).
    pub fn panel_poses(
        &self,
        bodies: &[contact_groups::Body],
    ) -> Vec<(usize, &'static str, Matrix, Matrix)> {
        self.wheels
            .iter()
            .filter_map(|w| Some((w.car, w.panel?, w.spec.pose, body_pose(bodies.get(w.body)?))))
            .collect()
    }

    /// Lost wheels for presentation: (car, wheel, pose at the loss, pose now).
    pub fn wheel_poses(
        &self,
        bodies: &[contact_groups::Body],
    ) -> Vec<(usize, usize, Matrix, Matrix)> {
        self.wheels
            .iter()
            .filter(|w| w.panel.is_none())
            .filter_map(|w| Some((w.car, w.wheel, w.spec.pose, body_pose(bodies.get(w.body)?))))
            .collect()
    }

    /// Switch a break's boxes in a track: the intact boxes stop colliding,
    /// the damage state's static boxes start.
    pub fn disable(track: &mut Track, boxes: &[usize]) {
        if let Some(props) = track.props_mut() {
            let all = std::sync::Arc::make_mut(&mut props.boxes);
            for &k in boxes {
                if let Some(b) = all.get_mut(k) {
                    b.enabled = b.damaged;
                }
            }
        }
    }

    /// Restore the intact state in the given tracks (the caller restores its
    /// own initial world).
    pub fn reset(&mut self, tracks: &mut [&mut Track]) -> Result<(), Error> {
        self.broken.iter_mut().for_each(|b| *b = false);
        self.pieces.clear();
        self.statics.clear();
        self.joints.clear();
        self.posts.clear();
        self.post_touches.clear();
        self.wheels.clear();
        self.events.clear();
        self.pending.clear();
        self.base_slots = None;
        self.lost.clear();
        for track in tracks {
            self.arm(track)?;
        }
        Ok(())
    }

    /// What presses world body `body` (a piece or a lost wheel) down at its
    /// spawn pose, if anything: a contact with a downward normal from the
    /// original's static geometry, a road chunk or the boundary sheet
    /// (`Pressed::Road`) or an entity box other than the piece's own intact
    /// boxes (`Pressed::EntityBox`). Authored ornament stand-ins do not
    /// count. The original spawns pieces at the raw placement (0x7D3C40, no
    /// ground snap) and orients a non-sheet triangle's normal by the shape
    /// origin (AEA1F0/AEE640), so such a body is pushed under in the original
    /// too [I] (docs/ROBUSTNESS_RESEARCH.md). Diagnostics only (grid-soak).
    pub fn pressed_down_at_spawn(
        &self,
        body: usize,
        track: &Track,
        surfaces: &Surfaces,
        directions: &convex_distance::ContactSettings,
    ) -> Result<Option<Pressed>, Error> {
        let (spec, own): (&PieceSpec, &[usize]) =
            if let Some(p) = self.pieces.iter().find(|p| p.body == body) {
                let entry = &self.catalog.breakables[p.breakable];
                (&entry.pieces[p.spec], &entry.intact)
            } else if let Some(w) = self.wheels.iter().find(|w| w.body == body) {
                (&w.spec, &[])
            } else {
                return Ok(None);
            };
        if !spec.collides() {
            return Ok(None);
        }
        spawn_pressed(spec, own, track, surfaces, directions)
    }

    /// What presses a live piece or lost wheel at its current pose in
    /// `bodies` (diagnostics only, grid-soak).
    pub fn pressing_now(
        &self,
        body: usize,
        bodies: &[contact_groups::Body],
        track: &Track,
        surfaces: &Surfaces,
        directions: &convex_distance::ContactSettings,
    ) -> Result<Option<Pressing>, Error> {
        let (spec, own): (&PieceSpec, &[usize]) =
            if let Some(p) = self.pieces.iter().find(|p| p.body == body) {
                let entry = &self.catalog.breakables[p.breakable];
                (&entry.pieces[p.spec], &entry.intact)
            } else if let Some(w) = self.wheels.iter().find(|w| w.body == body) {
                (&w.spec, &[])
            } else {
                return Ok(None);
            };
        let Some(state) = bodies.get(body).filter(|_| spec.collides()) else {
            return Ok(None);
        };
        pressing_at(spec, body_pose(state), own, track, surfaces, directions).map(Some)
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
        let (m, i) = mass_properties(Primitive::Box([1., 2., 3.]), 700., 0.);
        assert_eq!(m, 4200.);
        assert_eq!(i, [4550., 3500., 1750.]);
        let (m, _) = mass_properties(
            Primitive::Cylinder {
                radius: 1.,
                height: 1.,
            },
            1.,
            0.,
        );
        assert!((m - std::f32::consts::PI).abs() < 1e-6);
        // A Nürburgring tyre (sRubber d700, shell 0.02): the shape grown by
        // its shell, radius 0.256 and height 0.185 (B46460, B46C90).
        let tyre = Primitive::Cylinder {
            radius: 0.236,
            height: 0.145,
        };
        let (m, i) = mass_properties(tyre, 700., 0.02);
        let expected = std::f32::consts::PI * 0.256 * 0.256 * 0.185 * 700.;
        assert!((m - expected).abs() < 1e-3 * expected, "{m}");
        let side = m * (0.185 * 0.185 / 12. + 0.256 * 0.256 / 4.);
        assert!((i[0] - side).abs() < 1e-3 * side && i[0] == i[2], "{i:?}");
        assert!((i[1] - m * 0.256 * 0.256 * 0.5).abs() < 1e-3 * i[1], "{i:?}");
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
                Primitive::Sphere { radius } => 2. * radius,
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

    /// Each round piece's drawn mesh (its anchor's draws in the damage
    /// renderable's frame, `Renderables::draws_in`, as grid-app places them)
    /// is centred on its collision cylinder (F564). With the anchor's own
    /// transform only (`draws`), every Detroit and Nürburgring tyre was drawn
    /// 0.6 m to the side and 0.54 m below its body: sunk into the road at
    /// rest, in the air once it tilted.
    /// Every piece of every breakable on the phone tracks' routes
    /// (Nürburgring, Detroit, Le Mans, Donington) gets a draw (its anchor's
    /// draws in the damage renderable's frame, `Renderables::draws_in`, as
    /// grid-app places them), and that draw sits on its collision shape:
    /// a round piece's mesh is centred on its cylinder or sphere and fills
    /// it, a box piece's mesh centre lies inside its box (F564, F566).
    /// F564 found every Detroit and Nürburgring tyre drawn 0.6 m off its
    /// body; Nürburgring's tyrewall_black_yok (a BUNDLENODE renderable) and
    /// close_drift_flag (a skinned mesh) pieces were not drawn at all.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn breakable_pieces_draw_on_their_bodies() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let mut failures = Vec::new();
        let (mut checked, mut geometryless) = (0, 0);
        for (track, routes) in [
            ("europe/nurburgring", 0..4),
            ("usa/detroit", 0..10),
            ("europe/le_mans", 0..2),
            ("europe/donington", 0..4),
        ] {
            let mut seen = std::collections::BTreeSet::new();
            let first = Route::new(&format!("tracks/{track}/route_0")).unwrap();
            let bytes = std::fs::read(root.join(first.track_file("objects.pssg"))).unwrap();
            let tree = pssg::read_tree(&bytes).unwrap();
            let sources = grid_formats::mesh::read_object_meshes(&tree).unwrap();
            let renderables = grid_formats::scene::Renderables::new(&tree, &sources).unwrap();
            // grid-app needs each piece material's diffuse binding.
            let bound: std::collections::BTreeSet<&[u8]> =
                (grid_formats::material::read_texture_bindings(&tree).unwrap().into_iter())
                    .filter(|b| b.sampler == b"TDiffuseAlphaMap")
                    .map(|b| b.shader)
                    .collect();
            for r in routes {
                let route = Route::new(&format!("tracks/{track}/route_{r}")).unwrap();
                let props = Props::read_route(root, &route).unwrap();
                let catalog = Catalog::read(root, &route, &props).unwrap();
                for b in &catalog.breakables {
                    if !seen.insert(b.template.clone()) {
                        continue;
                    }
                    for (k, piece) in b.pieces.iter().enumerate() {
                        let what = format!("{track} route_{r} {} piece {k} ({})", b.template, piece.anchor);
                        let draws = renderables
                            .draws_in(piece.renderable.as_bytes(), piece.anchor.as_bytes());
                        let draws = match draws {
                            Ok(d) if !d.is_empty() => d,
                            // The Armco main sections' link post carries no
                            // geometry of its own: nothing to draw.
                            Ok(_) if piece.anchor.ends_with("section_dam_link1") => {
                                geometryless += 1;
                                continue;
                            }
                            other => {
                                failures.push(format!("{what}: no draw {:?}", other.map(|d| d.len())));
                                continue;
                            }
                        };
                        let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                        let p = piece.pose;
                        for draw in &draws {
                            if !bound.contains(draw.material) {
                                failures.push(format!("{what}: material {} unbound", String::from_utf8_lossy(draw.material)));
                            }
                            let m = grid_formats::scene::compose(&draw.transform, &b.placement);
                            for v in draw.indices.iter().map(|&i| i as usize) {
                                let w = grid_formats::scene::transform_point(
                                    &m,
                                    sources[draw.source].positions[v],
                                );
                                let d = [w[0] - p[3][0], w[1] - p[3][1], w[2] - p[3][2]];
                                for r in 0..3 {
                                    let l = (0..3).map(|j| d[j] * p[r][j]).sum::<f32>();
                                    lo[r] = lo[r].min(l);
                                    hi[r] = hi[r].max(l);
                                }
                            }
                        }
                        let mid: [f32; 3] = std::array::from_fn(|r| (lo[r] + hi[r]) * 0.5);
                        let half: [f32; 3] = std::array::from_fn(|r| (hi[r] - lo[r]) * 0.5);
                        let ok = match piece.primitive {
                            // A tyre fills its cylinder: radius across, height
                            // along. A post's mesh (Donington's Armco link,
                            // 1.01 m on a 0.76 m cylinder) may stand above it.
                            Primitive::Cylinder { radius, height } => {
                                mid[0].abs() < 0.05
                                    && mid[2].abs() < 0.05
                                    && mid[1].abs() < 0.05_f32.max(0.2 * height)
                                    && (half[0] - radius).abs() < 0.06
                                    && (half[2] - radius).abs() < 0.06
                                    && half[1] > height * 0.5 - 0.06
                                    && half[1] < height * 0.5 + 0.06_f32.max(0.2 * height)
                            }
                            // A shapeless piece (radius 0, the flag's base) only
                            // carries its mesh.
                            Primitive::Sphere { radius } => {
                                mid.iter().all(|v| v.abs() < 0.05)
                                    && (radius == 0. || half.iter().all(|h| (h - radius).abs() < 0.06))
                            }
                            Primitive::Box(size) => (0..3).all(|r| mid[r].abs() <= size[r] * 0.5 + 0.05),
                        };
                        if !ok {
                            failures.push(format!("{what}: mesh centre {mid:?} half {half:?} shape {:?}", piece.primitive));
                        }
                        checked += 1;
                    }
                }
            }
        }
        eprintln!("checked {checked} pieces, {geometryless} without geometry");
        for f in &failures {
            eprintln!("{f}");
        }
        assert!(checked > 100, "{checked}");
        assert!(failures.is_empty(), "{} of {} pieces fail", failures.len(), checked + failures.len());
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
            let walls = b.template == "tyre_wall.max";
            let armco = b.template.starts_with("armco");
            assert_eq!(b.explosion, if walls { 0.35 } else { 0. });
            assert_eq!(
                b.joints.len(),
                if walls {
                    7
                } else if armco {
                    6
                } else {
                    0
                }
            );
            if armco {
                // Post to the world, rail 4 to the linked post; the post
                // (`link1` body, piece 0) and the rails take the overrides.
                assert_eq!((b.joints[0].a, b.joints[0].b, b.joints[0].link), (0, None, None));
                assert_eq!((b.joints[5].a, b.joints[5].b, b.joints[5].link), (4, None, Some(1)));
                assert!(b.links.len() <= 1);
                for piece in &b.pieces {
                    assert_eq!(piece.mass, 7000.);
                    assert_eq!(piece.inertia, [2800.; 3]);
                    assert_eq!(piece.body.linear_damping, 14.);
                }
                assert_eq!(b.pieces[0].body.gravity_scale, 1.);
                assert_eq!(b.pieces[1].body.gravity_scale, 0.);
            }
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
        // Each wall: six blocks joined by seven cDeformableRubber springs,
        // and sixteen loose tyres.
        assert_eq!(by_template["tyre_wall.max"], (588, 1000., 22));
        assert!(!catalog.unported.contains_key("tyre_wall.max"));
        // 243 armco sections in two chains; every post that is a link
        // primary is static at the break.
        assert!(!catalog.unported.contains_key("armco_white_main.max"));
        assert!(!catalog.unported.contains_key("armco_blue_main.max"));
        let armco: Vec<_> =
            catalog.breakables.iter().filter(|b| b.template.starts_with("armco")).collect();
        let links: usize = armco.iter().map(|b| b.links.len()).sum();
        let statics = armco.iter().filter(|b| b.static_pieces[0]).count();
        eprintln!("armco={} links={links} static posts={statics}", armco.len());
        assert_eq!(armco.len(), 243);
        let board = &catalog
            .breakables
            .iter()
            .find(|b| b.template.starts_with("gg_brake_marker"))
            .unwrap()
            .pieces[0];
        assert!(
            // The box grown by its shell 0.02 (B3D480).
            (board.mass - 70. * 0.98 * 0.51 * 0.2).abs() < 0.01,
            "{}",
            board.mass
        );
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn jarama_close_drift_flags_break_into_a_static_base_and_a_phantom_pole() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let route = Route::new("tracks/europe/jarama/route_0").unwrap();
        let props = Props::read_route(root, &route).unwrap();
        let catalog = Catalog::read(root, &route, &props).unwrap();
        assert!(!catalog.unported.contains_key("close_drift_flag.max"), "{:?}", catalog.unported);
        let flags: Vec<_> =
            catalog.breakables.iter().filter(|b| b.template == "close_drift_flag.max").collect();
        assert_eq!(flags.len(), 16);
        for b in flags {
            // The pole (bPhantom) and the base (bHeavy) of the intact body.
            assert_eq!(b.limits, vec![(50., 0.), (2000., 0.)]);
            // The damage state: the static base's two shapes as route boxes,
            // off until the break.
            assert_eq!(b.damaged.len(), 2);
            assert!(b.damaged.iter().all(|&k| props.boxes[k].damaged && !props.boxes[k].enabled));
            assert_eq!(b.bodies, ["close_drift_flag_obj_02_rigidbody", "close_drift_flag_obj_01_rigidbody"]);
            assert_eq!(b.static_pieces, [false, true]);
            // The pole collides with nothing and floats on its spring.
            let pole = &b.pieces[0];
            assert!(!pole.collides());
            assert_eq!(pole.limit, f32::MAX);
            assert_eq!((pole.body.gravity_scale, pole.body.linear_damping), (0., 0.));
            assert!((pole.mass - 70. * std::f32::consts::PI * 0.12 * 0.12 * 2.97).abs() < 0.05, "{}", pole.mass);
            // Base to the world: a static A, no joint. Base to pole: swapped,
            // the pole held to the base's fixed object; the post.
            let joints: Vec<_> = b.joints.iter().map(|j| (j.a, j.b, j.link)).collect();
            assert_eq!(joints, [(1, None, None), (0, Some(1), None)]);
            assert_eq!(b.posts, [1]);
        }
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
                    // sRubber d700 over the unscaled template cylinder grown by its shell.
                    assert_eq!(p.mass, mass_properties(p.primitive, 700., p.shell).0);
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
        for b in track.props().unwrap().boxes.iter() {
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
        // Soak seeds 12, 1012 and 1029 (F345, F351): the lower board of the
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
                    lower.collider(),
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
    #[ignore = "requires GRID_GAME_ROOT and GRID_CONTACT_DIRECTIONS"]
    fn no_piece_spawns_pressed_down_by_a_barrier_stand_in() {
        // Long Beach soak seeds 1..12 and 17..20 (F354): the tyre stacks stand
        // in front of concrete_fence_* walls whose stand-in box reached 0.63 m
        // past the concrete (the catch fence leans out over the track). Their
        // pieces spawned up to 0.76 m inside it, the box pushed them down
        // through the road and they fell out of the world.
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let table = std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
        for path in crate::route::CIRCUITS {
            let route = Route::new(path).unwrap();
            let mut runtime =
                crate::source_vehicle::Runtime::lacetti_on(root, &route, "slot_00", 200679)
                    .unwrap();
            let geometry = runtime.prepare_body_geometry(root, &table).unwrap();
            let track = runtime.track();
            let props = track.props().unwrap();
            let catalog = Catalog::read(root, &route, props).unwrap();
            let (mut pieces, mut touching, mut deepest) = (0, 0, 0f32);
            for b in &catalog.breakables {
                // A shapeless post collides with nothing.
                for p in b.pieces.iter().filter(|p| p.collides()) {
                    pieces += 1;
                    let query = convex_distance::ContactSettings {
                        margin: p.shell,
                        directions: geometry.query().directions,
                    };
                    let hits = track
                        .body_contacts(p.collider(), p.pose, &query, TRACK_CONTACTS, 4096, |code| {
                            runtime.surfaces().sheet(code)
                        })
                        .unwrap();
                    for h in hits {
                        // Ornament stand-ins are the prop boxes without an entity.
                        if h.prop.is_none_or(|k| props.boxes[k].entity.is_some()) {
                            continue;
                        }
                        touching += 1;
                        deepest = deepest.max(h.contact.penetration);
                        assert!(
                            h.contact.normal[1] > -0.5,
                            "{path} {} at {:?}: {:?} {}",
                            b.template,
                            p.pose[3],
                            h.contact.normal,
                            h.contact.penetration
                        );
                    }
                }
            }
            println!("{path}: {pieces} pieces, {touching} stand-in contacts, deepest {deepest}");
            if path == crate::route::LONG_BEACH {
                // 1330 piece/stand-in pairs up to 0.76 m deep before the fix.
                assert!(pieces > 1000 && deepest < 0.15, "{deepest}");
            }
        }
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT and GRID_CONTACT_DIRECTIONS"]
    fn only_sunk_tyre_stacks_spawn_pieces_pressed_down_by_the_road() {
        // Jarama soak seed 24 (F361): tyrestack_yell_double entity 1297 is
        // authored 0.27 m into the rising verge, and its bottom tyre (piece
        // 14, body 90 in the replay) starts with its centre 3 cm below the
        // road. The road triangles press it down at its spawn pose
        // (docs/ROBUSTNESS_RESEARCH.md); grid-soak counts such a loss as
        // expected. No piece on the other circuits is pressed by the road.
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let table = std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
        for path in crate::route::CIRCUITS {
            let route = Route::new(path).unwrap();
            let mut runtime =
                crate::source_vehicle::Runtime::lacetti_on(root, &route, "slot_00", 200679)
                    .unwrap();
            let geometry = runtime.prepare_body_geometry(root, &table).unwrap();
            let track = runtime.track();
            let catalog = Catalog::read(root, &route, track.props().unwrap()).unwrap();
            let mut road = Vec::new();
            for b in &catalog.breakables {
                for (k, p) in b.pieces.iter().enumerate().filter(|(_, p)| p.collides()) {
                    let pressed =
                        spawn_pressed(p, &b.intact, track, runtime.surfaces(), geometry.query())
                            .unwrap();
                    if pressed == Some(Pressed::Road) {
                        road.push((b.entity, k, b.template.clone()));
                    }
                }
            }
            println!("{path}: {} pieces pressed down by the road", road.len());
            if path == crate::route::JARAMA {
                assert!(road.contains(&(1297, 14, "tyrestack_yell_double.max".into())));
                assert!(road.iter().all(|r| r.2.starts_with("tyrestack_")), "{road:?}");
            } else if path != crate::route::NURBURGRING {
                assert!(road.is_empty(), "{path}: {road:?}");
            }
        }
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
        assert!(world.is_lost(1) && !world.is_lost(2));
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
        assert!(!world.is_lost(1));
    }

    #[test]
    fn a_held_lost_wheel_stays_with_its_body_when_a_break_adds_pieces() {
        // Le Mans 24 Hours tick 30430 (FIDELITY_BACKLOG row 90): a break
        // appended armco pieces while lost wheels were held; the wheels'
        // held poses, 1 km away, went to the new pieces.
        let mut world = PropWorld::new(Catalog::default());
        let piece = |body: usize| Piece {
            breakable: 0,
            spec: 0,
            body,
            token: body as u32 + 1,
        };
        world.pieces = vec![piece(1)];
        world.wheels = vec![LooseWheel {
            car: 0,
            wheel: 2,
            panel: None,
            spec: std::sync::Arc::new(
                PieceSpec::loose_wheel(0.3, 0.2, 20., std::array::from_fn(|r| {
                    std::array::from_fn(|c| f32::from(r == c))
                }))
                .unwrap(),
            ),
            body: 2,
            token: 9,
        }];
        let mut bodies = vec![[0u32; 0x424 / 4]; 4];
        set(&mut bodies[2], 0x1d0 + 52, -20. - LOST_DEPTH - 1.);
        set(&mut bodies[2], 0x1d0 + 48, 300.);
        world.hold_lost(&mut bodies, -20.);
        assert!(world.is_lost(2));
        // A break adds a piece; it sits on the road.
        world.pieces.push(piece(3));
        set(&mut bodies[3], 0x1d0 + 52, 24.);
        set(&mut bodies[3], 0x1d0 + 48, 1.);
        world.hold_lost(&mut bodies, -20.);
        assert_eq!(f(&bodies[3], 0x1d0 + 52), 24.);
        assert_eq!(f(&bodies[3], 0x1d0 + 48), 1.);
        assert!(!world.is_lost(3) && world.is_lost(2));
        assert_eq!(f(&bodies[2], 0x1d0 + 48), 300.);
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
            limit: DESCRIPTOR_EC,
            hull: collision_mesh::build(&points, &triangles, 0).unwrap(),
            round: false,
            points,
            triangles,
        };
        let (body, (inverse_mass, inverse_inertia)) =
            piece_body(&spec, [0., 1., 0., 0.], [0.; 4]).unwrap();
        assert!((inverse_mass - 1. / 70.).abs() < 1e-6);
        assert!((inverse_inertia[0][0] - 6. / 70.).abs() < 1e-5);
        assert_eq!(body_pose(&body)[3], [5., 2., -3., 1.]);
        assert_eq!(f(&body, 0x224), 1.);
        assert_eq!(f(&body, 0x3f8), 1.);
        assert_eq!(f(&body, 0x3d4), 1.);
        assert_eq!(f(&body, 0x3b4), 1.);
    }
}

