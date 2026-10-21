//! Continuous collision of a fast body against the packed road chunks: the
//! original world's stage 2 (AD8A40 phase 2 -> B18E20 -> B15B70 -> B3A700 ->
//! AE4700 -> B80CE0/B801C0), applied by B18220 -> B10680/B14550 before the
//! ordinary narrow phase (phase 3, B18300). docs/ROBUSTNESS_RESEARCH.md.
//!
//! Scope: the moving shape is a collision mesh (kind 7) or a round piece
//! (kind 5/3, `convex_distance::Hull`) and the other shape a
//! packed road chunk (kind 11, swept callback AE4700, matrix entry +8).
use crate::{
    Error,
    contact_manifold,
    convex_distance::{self, ContactSettings},
    packed_candidates, packed_query,
    physical_pose::Matrix,
};

type V = [f32; 4];

/// 0xCAFE64: B15B70 sweeps a body when its step displacement exceeds this
/// fraction of a shape's radius.
pub const RADIUS_FACTOR: f32 = 0.75;
/// 0xCB22E4: B801C0 stops when |v|^2 <= 1e-6 x the simplex's largest |w|^2.
const RAY_TOLERANCE: f32 = f32::from_bits(0x3586_37bd);
/// Scratch +0x200 set by B7E440: the ray cast's iteration cap.
const RAY_ITERATIONS: u32 = 100;
/// 0xCB2294: AE4700 keeps a start-pose overlap only beyond this length.
const MIN_DEPTH: f32 = f32::from_bits(0x3a83_126f);
/// 0xCBA8F0: B3A700 scales the time of impact by 1.01 (clamped to 1).
const TIME_SCALE: f32 = 1.01;
/// Collision world +0x958, `maxContinuousCollisions` (config +0x3C, 0x6195F0
/// default 1000; the live value was not read): records per step.
pub const MAX_RECORDS: usize = 1000;

fn dot(a: V, b: V) -> f32 {
    (a[0] * b[0] + a[1] * b[1]) + (a[2] * b[2] + a[3] * b[3])
}
fn sub(a: V, b: V) -> V {
    std::array::from_fn(|j| a[j] - b[j])
}
fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
        0.,
    ]
}
/// Refined RSQRTPS normalisation as the original writes it (zero stays zero).
fn normalize(v: V) -> Result<V, Error> {
    let square = dot(v, v);
    if square == 0. {
        return Ok(v);
    }
    let seed = crate::spawn::reference_seed(square, false)?;
    let inverse = ((1. - (seed * square) * seed) * seed) * 0.5 + seed;
    Ok(v.map(|x| inverse * x))
}

/// The shape radius B15B70 compares against: the mesh shape's vtable +0x18
/// (B41F50) returns mesh data +0x5C, the least absolute coordinate among
/// the six extrema (B42540).
/// For a round body the least half extent [I: the kind-5/3 vtable +0x18 was
/// not read].
pub fn radius<'m>(hull: impl Into<convex_distance::Hull<'m>>) -> f32 {
    match hull.into() {
        convex_distance::Hull::Mesh(mesh) => {
            let b = mesh.bytes();
            f32::from_bits(u32::from_le_bytes(b[0x5c..0x60].try_into().unwrap()))
        }
        convex_distance::Hull::Cylinder {
            radius,
            half_height,
        } => radius.min(half_height),
        convex_distance::Hull::Sphere { radius } => radius,
    }
}

/// B15B70: a body is swept when (0.75 r)^2 < |d|^2 for one of its shapes
/// (x87 extended products, here in f64).
pub fn is_fast(radius: f32, displacement: V) -> bool {
    let r = f64::from(radius) * f64::from(RADIUS_FACTOR);
    let d = displacement;
    let square = f64::from((d[1] * d[1] + d[0] * d[0]) + (d[3] * d[3] + d[2] * d[2]));
    r * r < square
}

/// The ray cast's simplex at scratch +0 (B81620 reset, B81700 add, B81E30
/// closest, B817C0 compact): points w (+0x10), supports (+0x60), closest
/// support (+0xB0), largest |w|^2 (+0xC0) and each |w|^2 (+0xC4).
struct RaySimplex {
    count: usize,
    points: [V; 4],
    supports: [V; 4],
    closest_support: V,
    largest: f32,
    squares: [f32; 4],
}

impl RaySimplex {
    fn new() -> Self {
        Self {
            count: 0,
            points: [[0.; 4]; 4],
            supports: [[0.; 4]; 4],
            closest_support: [0.; 4],
            largest: 0.,
            squares: [0.; 4],
        }
    }
    /// B81620.
    fn reset(&mut self) {
        self.largest = 0.;
        self.count = 0;
        self.supports = [[0.; 4]; 4];
    }
    /// B81700.
    fn add(&mut self, w: V, p: V) {
        let n = self.count;
        self.points[n] = w;
        self.supports[n] = p;
        self.squares[n] = ((w[3] * w[3] + w[2] * w[2]) + w[1] * w[1]) + w[0] * w[0];
        self.count += 1;
    }
    /// B817C0: keep the active points in order and take the largest square.
    fn compact(&mut self, active: [bool; 4]) {
        self.largest = 0.;
        let mut kept = 0;
        for (i, &keep) in active.iter().enumerate() {
            if keep {
                if kept != i {
                    self.points[kept] = self.points[i];
                    self.supports[kept] = self.supports[i];
                    self.squares[kept] = self.squares[i];
                }
                let s = self.squares[kept];
                if self.largest <= s && s != self.largest {
                    self.largest = s;
                }
                kept += 1;
            }
        }
        self.count = kept;
    }
    fn weighted(&self, weights: V, n: usize) -> V {
        std::array::from_fn(|j| {
            (0..n).fold(0., |sum, i| sum + weights[i] * self.supports[i][j])
        })
    }
    /// B81E30: the point of the simplex closest to the origin; false when the
    /// feature lies outside (a negative weight) or the tetrahedron is
    /// degenerate.
    fn closest(&mut self, v: &mut V) -> bool {
        match self.count {
            1 => {
                *v = self.points[0];
                self.closest_support = self.supports[0];
                self.largest = self.squares[0];
                true
            }
            2 => {
                let a = self.points[0];
                let d = sub(self.points[1], a);
                let t = d[3] * (1. - a[3]) + (0. - a[2]) * d[2] + d[1] * (0. - a[1])
                    + (0. - a[0]) * d[0];
                if t <= 0. {
                    *v = a;
                } else {
                    let square = d[1] * d[1] + d[0] * d[0] + d[3] * d[3] + d[2] * d[2];
                    if t < square {
                        let t = t / square;
                        *v = std::array::from_fn(|j| t * d[j] + a[j]);
                        let (s0, s1) = (self.supports[0], self.supports[1]);
                        self.closest_support = std::array::from_fn(|j| t * (s1[j] - s0[j]) + s0[j]);
                        self.largest = if self.squares[1] < self.squares[0] {
                            self.squares[0]
                        } else {
                            self.squares[1]
                        };
                        return true;
                    }
                    self.points[0] = self.points[1];
                    self.supports[0] = self.supports[1];
                    self.squares[0] = self.squares[1];
                    *v = self.points[0];
                }
                self.closest_support = self.supports[0];
                self.largest = self.squares[0];
                self.count = 1;
                true
            }
            3 => {
                let [a, b, c, _] = self.points;
                let f = convex_distance::triangle_feature(a, b, c, sub(b, a), sub(c, a));
                *v = f.point;
                self.closest_support = self.weighted(f.weights, 3);
                let inside = f.weights[..3].iter().all(|&w| 0. <= w);
                self.compact(f.active);
                inside
            }
            4 => match convex_distance::tetrahedron_feature(self.points) {
                Ok(Some(f)) => {
                    *v = f.point;
                    self.closest_support = self.weighted(f.weights, 4);
                    let inside = f.weights.iter().all(|&w| 0. <= w);
                    self.compact(f.active);
                    inside
                }
                Ok(None) => {
                    *v = [0.; 4];
                    true
                }
                Err(()) => false,
            },
            _ => false,
        }
    }
}

/// A swept hit: time of impact in the step, point and the normal that pushes
/// the moving mesh out (AE4700 record words 0..3, 4..7 and 8).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub time: f32,
    pub point: V,
    pub normal: V,
}

/// B801C0 as AE4700 calls it through B80CE0: shape 1 the triangle (support
/// B7C830, first maximum), shape 2 the mesh rotated about the origin, both
/// with zero margin; the ray runs from the mesh's start translation `from`
/// to its end translation `to`. Returns (time, point, normal) or None for a
/// miss. Time 0 means the shapes already touch at the start.
fn ray_cast(
    triangle: [V; 3],
    mesh: convex_distance::Hull<'_>,
    rotation: Matrix,
    from: V,
    to: V,
) -> Result<Option<(f32, V, V)>, Error> {
    let mut cache = [0; 7];
    let mut support = |v: V| -> Result<V, Error> {
        let mut t = triangle[0];
        let mut best = dot(t, v);
        for &p in &triangle[1..] {
            let d = dot(p, v);
            if best < d {
                best = d;
                t = p;
            }
        }
        let mut m = convex_distance::mesh_support(mesh, rotation, v.map(|x| -x), &mut cache)?;
        // B4B0A0's support carries the sphere's radius.
        if let convex_distance::Hull::Sphere { radius } = mesh {
            let length = dot(v, v).sqrt();
            if length > 0. {
                let scale = radius / length;
                for j in 0..3 {
                    m[j] -= scale * v[j];
                }
            }
        }
        let mut p = sub(t, m);
        p[3] = 1.;
        Ok(p)
    };
    let mut start = from;
    start[3] = 1.;
    let r = sub(to, from);
    let mut lambda = 0f32;
    let mut normal = [0.; 4];
    let mut x = start;
    let mut v = sub(start, support(r)?);
    let mut simplex = RaySimplex::new();
    simplex.reset();
    let mut iterations = 0;
    while dot(v, v) > simplex.largest * RAY_TOLERANCE && iterations < RAY_ITERATIONS {
        iterations += 1;
        let p = support(v)?;
        let mut w = sub(x, p);
        let vw = dot(v, w);
        if 0. < vw {
            let vr = dot(v, r);
            if 0. <= vr {
                return Ok(None);
            }
            lambda -= vw / vr;
            if 1. < lambda {
                return Ok(None);
            }
            x = std::array::from_fn(|j| lambda * r[j] + start[j]);
            w = sub(x, p);
            normal = v;
            simplex.reset();
        }
        simplex.add(w, p);
        if !simplex.closest(&mut v) {
            break;
        }
        v[3] = 0.;
    }
    if !(lambda.is_finite() && x.iter().chain(&normal).all(|c| c.is_finite())) {
        return Err(Error("nonfinite continuous ray cast"));
    }
    Ok(Some((lambda, x, normal)))
}

/// AE4700 for one mesh body (shell `margin`) against one packed chunk
/// (`shape_margin`, sheet bits `sheet_mask`), its shape moving from `start`
/// to `end` (translation only): at most `capacity` hits, triangle by triangle
/// in candidate order. A triangle the mesh already touches at the start gives
/// the B7F740 contact with time 0; otherwise the ray cast's time of impact.
#[allow(clippy::too_many_arguments)]
pub fn sweep_packed<'m>(
    mesh: impl Into<convex_distance::Hull<'m>>,
    start: Matrix,
    end: Matrix,
    track: &packed_query::Mesh<'_>,
    materials: &[u8],
    sheet_mask: u32,
    shape_margin: f32,
    query: &ContactSettings,
    capacity: usize,
) -> Result<Vec<Hit>, Error> {
    let mesh = mesh.into();
    let mut hits = Vec::new();
    if capacity == 0 {
        return Ok(hits);
    }
    // B212D0 with the motion: the candidates of the swept bounds [I].
    let margin = query.margin + shape_margin;
    let [a0, a1] = mesh.world_bounds(start, margin)?;
    let [b0, b1] = mesh.world_bounds(end, margin)?;
    let bounds = [
        std::array::from_fn(|j| a0[j].min(b0[j])),
        std::array::from_fn(|j| a1[j].max(b1[j])),
    ];
    let candidates = packed_candidates::collect_set(track, bounds, 4096, None, false)?;
    let mut rotation = start;
    rotation[3] = [0., 0., 0., 1.];
    let mut context = convex_distance::Context::new();
    for &id in &candidates.accepted {
        let triangle = track.triangles[id].map(|index| {
            let p = track.vertices[index as usize];
            [p[0], p[1], p[2], 1.]
        });
        let material = *materials.get(id).ok_or(Error("packed material absent"))?;
        let sheet = sheet_mask & (1 << (material & 0xf)) != 0;
        let face = || {
            let first = sub(triangle[0], triangle[1]);
            let second = sub(triangle[0], triangle[2]);
            cross(first, second)
        };
        let overlap = context.contact_triangle(
            mesh,
            start,
            triangle,
            shape_margin,
            [1., 0., 0., 0.],
            query,
        )?;
        if let Some(witnesses) = overlap.witnesses {
            let difference = sub(witnesses[0], witnesses[1]);
            let length = contact_manifold::witness_length(dot(difference, difference))?;
            if length <= MIN_DEPTH {
                continue;
            }
            let normal = if sheet {
                normalize(face())?
            } else {
                let inverse = 1. / length;
                difference.map(|x| inverse * x)
            };
            let (normal, witnesses) =
                contact_manifold::orient(triangle[0], start[3], normal, witnesses)?;
            hits.push(Hit {
                time: 0.,
                point: witnesses[1],
                normal,
            });
        } else if let Some((time, point, cast)) =
            ray_cast(triangle, mesh, rotation, start[3], end[3])?
            && 0. < time
        {
            let normal = if sheet {
                let n = face();
                if dot(n, cast) <= 0. {
                    #[allow(clippy::neg_multiply)] // Original multiply by -1.
                    n.map(|x| -1. * x)
                } else {
                    n
                }
            } else {
                cast
            };
            hits.push(Hit {
                time,
                point,
                normal: normalize(normal)?,
            });
        } else {
            continue;
        }
        if hits.len() == capacity {
            break;
        }
    }
    Ok(hits)
}

/// B3A700 for a hit between a moving body (inverse mass `inverse_mass`,
/// step displacement `displacement`, start position `previous`) and static
/// geometry: the corrected position, or None when the body separates.
pub fn response(hit: &Hit, inverse_mass: f32, displacement: V, previous: V) -> Option<V> {
    let n = hit.normal;
    let d = displacement;
    let along = (d[1] * n[1] + n[0] * d[0]) + (d[3] * n[3] + n[2] * d[2]);
    let square = (n[1] * n[1] + n[0] * n[0]) + (n[3] * n[3] + n[2] * n[2]);
    let k = -along / ((0. + inverse_mass) * square);
    if k <= 0. {
        return None;
    }
    let t = (hit.time * TIME_SCALE).min(1.);
    let push = k * inverse_mass;
    let rest = 1. - t;
    Some(std::array::from_fn(|j| {
        (push * n[j] + d[j]) * rest + t * d[j] + previous[j]
    }))
}

/// B14550's mean of the corrected positions: sum x refined RCPPS(count),
/// with w = 1.
pub fn mean(positions: &[V]) -> Result<V, Error> {
    let count = positions.len() as f32;
    let sum: V = std::array::from_fn(|j| positions.iter().fold(0., |s, p| s + p[j]));
    let seed = crate::spawn::reference_seed(count, true)?;
    let mut out: V = std::array::from_fn(|j| {
        let scaled = seed * sum[j];
        (1. - seed * count) * scaled + scaled
    });
    out[3] = 1.;
    Ok(out)
}

fn vector(body: &crate::contact_groups::Body, offset: usize) -> V {
    std::array::from_fn(|j| f32::from_bits(body[offset / 4 + j]))
}
fn matrix(body: &crate::contact_groups::Body, offset: usize) -> Matrix {
    std::array::from_fn(|r| vector(body, offset + r * 16))
}

/// A body's step: (displacement, previous position, start pose, end pose).
/// B15B70/B3A700: the displacement is the predicted centre (+0x334 -> +0x1C0)
/// minus the step's start (+0x2A0); the swept shape keeps its end rotation
/// and starts at its end pose minus the displacement.
pub fn step(body: &crate::contact_groups::Body) -> (V, V, Matrix, Matrix) {
    let previous = vector(body, 0x2a0);
    let displacement = sub(vector(body, 0x1c0), previous);
    let end = matrix(body, 0x1d0);
    let mut start = end;
    start[3] = sub(end[3], displacement);
    (displacement, previous, start, end)
}

/// B14550: move a swept body to `position` (+0x1C0), record the correction
/// in +0x360 and republish its pose from the unchanged rotation.
pub fn apply(body: &mut crate::contact_groups::Body, position: V) -> Result<(), Error> {
    if position.iter().any(|v| !v.is_finite()) {
        return Err(Error("nonfinite continuous correction"));
    }
    let correction = sub(position, vector(body, 0x1c0));
    let mut corrected = matrix(body, 0x190);
    corrected[3] = position;
    let out = crate::physical_pose::publish(
        corrected,
        vector(body, 0x230),
        [matrix(body, 0x90), matrix(body, 0x150)],
        &[],
    )?;
    let mut store = |offset: usize, m: Matrix| {
        for (i, v) in m.iter().flatten().enumerate() {
            body[offset / 4 + i] = v.to_bits();
        }
    };
    store(0x190, out.corrected);
    store(0x1d0, out.published);
    store(0xd0, out.derived[0]);
    store(0x110, out.derived[1]);
    for (j, v) in correction.iter().enumerate() {
        body[0x360 / 4 + j] = v.to_bits();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision_mesh::Mesh;

    fn cube(half: f32) -> Mesh {
        let points: Vec<V> = (0..8)
            .map(|i| {
                std::array::from_fn(|j| {
                    if j == 3 {
                        1.
                    } else if i >> j & 1 == 0 {
                        -half
                    } else {
                        half
                    }
                })
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
        crate::collision_mesh::build(&points, &faces, 0).unwrap()
    }

    fn at(y: f32) -> Matrix {
        [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., y, 0., 1.],
        ]
    }

    #[test]
    fn a_thin_body_is_swept_beyond_three_quarters_of_its_radius() {
        let mesh = cube(0.075);
        assert_eq!(radius(&mesh), 0.075);
        assert!(!is_fast(0.075, [0., -0.05, 0., 0.]));
        assert!(is_fast(0.075, [0., -0.16, 0., 0.]));
    }

    #[test]
    fn the_ray_cast_finds_the_time_a_falling_cube_meets_a_floor_triangle() {
        let mesh = cube(0.075);
        let floor = [
            [-10., 0., -10., 1.],
            [10., 0., -10., 1.],
            [0., 0., 10., 1.],
        ];
        // Bottom face at 0.1 - 0.075 = 0.025 at the start, -0.135 at the end.
        let (time, _, normal) = ray_cast(floor, (&mesh).into(), at(0.), [0., 0.1, 0., 1.], [0., -0.06, 0., 1.])
            .unwrap()
            .unwrap();
        // B801C0 stops at |v|^2 <= 1e-6 x the largest |w|^2 (about 1 cm for
        // this 20 m triangle), so the time lands just before the contact.
        assert!(time <= 0.025 / 0.16 && 0.15 < time, "{time}");
        let n = normalize(normal).unwrap();
        assert!(n[1] > 0.999, "{n:?}");
        // Moving away misses.
        assert!(
            ray_cast(floor, (&mesh).into(), at(0.), [0., 0.1, 0., 1.], [0., 0.3, 0., 1.])
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn the_response_removes_the_approach_after_the_time_of_impact() {
        let hit = Hit {
            time: 0.25,
            point: [0.; 4],
            normal: [0., 1., 0., 0.],
        };
        let corrected = response(&hit, 1. / 17., [1., -0.16, 0., 0.], [0., 0.1, 0., 1.]).unwrap();
        // 1.01 x 0.25 of the fall, all of the horizontal motion.
        assert!((corrected[1] - (0.1 - 0.16 * 0.2525)).abs() < 1e-6, "{corrected:?}");
        assert!((corrected[0] - 1.).abs() < 1e-6);
        assert_eq!(corrected[3], 1.);
        // Separating: no correction.
        assert!(response(&hit, 1., [0., 0.2, 0., 0.], [0., 0.1, 0., 1.]).is_none());
        let m = mean(&[corrected]).unwrap();
        assert!((m[1] - corrected[1]).abs() < 1e-6);
    }
}
