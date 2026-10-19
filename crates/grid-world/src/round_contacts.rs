//! AE8AD0 for a moving collision mesh (kind 7) against a static kind-5
//! cylinder or kind-3 sphere (route entity primitives: Yokohama's barrels,
//! the Milan and Detroit balloons). The query is the box path's
//! (`box_contacts`) with the shape's own support: B47A70 for the cylinder,
//! B4B0A0 for the sphere (its centre, pushed by radius plus shell). The
//! manifold is AE2FA0 with the cylinder's typed B47590 feature; the sphere's
//! feature function is the default ADB8F0, which offers none, so a sphere
//! contact is AE2FA0's single fallback contact.
//!
//! B47590 picks the feature from the axis component `c` of the direction:
//! - `|c| > 0.707`: the cap disk (type 2), a clipping reference above 0.99;
//! - `0.044 < |c| <= 0.707`: the side generator nearest the direction, a
//!   two-vertex segment (type 1);
//! - otherwise the side itself (type 3).
//!
//! AE2FA0 turns the side into its generator segment (AE2F10) when the mesh
//! face is a reference, and otherwise runs AE2710: every mesh face edge is
//! cut by the cylinder (AE16A0) and the cut's middle and ends become
//! contacts. A cap disk against a mesh face is AEB8D0: the face clipped to
//! the cap circle (AE1BD0) when the disk is the reference, else the disk as a
//! triangle (AE2210) and polygon clipping (AEB630).
use crate::{
    Error,
    collision_mesh::Mesh,
    contact_manifold::{self, Contact, Face, Settings},
    convex_distance::{ContactSettings, Context, Extra, Static},
    physical_pose::Matrix,
};

type V = [f32; 4];
fn dot(a: V, b: V) -> f32 {
    (a[0] * b[0] + a[1] * b[1]) + (a[2] * b[2] + a[3] * b[3])
}
fn sub(a: V, b: V) -> V {
    std::array::from_fn(|j| a[j] - b[j])
}
/// 0x67C200.
fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        b[0] * a[2] - b[2] * a[0],
        b[1] * a[0] - a[1] * b[0],
        0.,
    ]
}
/// p + t * d, per lane.
fn along(p: V, t: f32, d: V) -> V {
    std::array::from_fn(|j| t * d[j] + p[j])
}
/// 0x770AB0: `v` in the frame of rows 0..2 of `m`.
fn local(v: V, m: &[V; 3]) -> V {
    let [a, b, c] = m;
    [
        ((v[3] * a[3] + v[2] * a[2]) + v[1] * a[1]) + v[0] * a[0],
        ((v[1] * b[1] + v[0] * b[0]) + v[3] * b[3]) + v[2] * b[2],
        ((v[3] * c[3] + v[2] * c[2]) + v[1] * c[1]) + v[0] * c[0],
        0.,
    ]
}

/// A kind-5 cylinder: unit axes in rows 0..2 (the axis is row 1), centre in
/// row 3, radius, half height (0x7B96D0 stores height x scale.y x 0.5) and
/// the shape `shell` margin.
#[derive(Clone, Debug)]
pub struct CylinderShape {
    pub matrix: Matrix,
    pub radius: f32,
    pub half_height: f32,
    pub margin: f32,
}

/// A kind-3 sphere: unit axes in rows 0..2 (B7ED10 samples them), centre in
/// row 3, radius and shell.
#[derive(Clone, Debug)]
pub struct SphereShape {
    pub matrix: Matrix,
    pub radius: f32,
    pub margin: f32,
}

#[derive(Clone, Debug)]
pub enum RoundShape {
    Cylinder(CylinderShape),
    Sphere(SphereShape),
}

fn valid_matrix(m: &Matrix) -> bool {
    m.iter().flatten().all(|v| v.is_finite() && v.abs() <= 1e5)
        && m[3][3] == 1.
        && m[..3].iter().all(|r| r[3] == 0.)
}

impl CylinderShape {
    /// B47A70 without the shell (the query adds margins separately).
    pub(crate) fn support(&self, direction: V) -> V {
        let m = &self.matrix;
        let l = local(direction, &[m[0], m[1], m[2]]);
        let radial = (l[2] * l[2] + l[0] * l[0]).sqrt();
        let y = if 0. <= l[1] {
            self.half_height
        } else {
            -0. - self.half_height
        };
        let (x, z) = if radial <= f32::from_bits(0x3586_37bd) {
            (0., 0.)
        } else {
            let scale = self.radius / radial;
            (l[0] * scale, l[2] * scale)
        };
        std::array::from_fn(|j| ((m[3][j] + y * m[1][j]) + z * m[2][j]) + x * m[0][j])
    }

    fn valid(&self) -> bool {
        valid_matrix(&self.matrix)
            && [self.radius, self.half_height]
                .iter()
                .all(|v| v.is_finite() && *v > 0. && *v < 1e3)
            && self.margin.is_finite()
            && (0. ..=1.).contains(&self.margin)
    }

    /// B47590 for `direction`.
    fn feature(&self, direction: V) -> Result<Feature, Error> {
        let m = &self.matrix;
        let axis = m[1];
        let h = self.half_height;
        let c = ((direction[3] * axis[3] + direction[2] * axis[2]) + direction[1] * axis[1])
            + direction[0] * axis[0];
        let radial =
            || contact_manifold::normalize(std::array::from_fn(|j| direction[j] - c * axis[j]));
        if c.abs() > f32::from_bits(0x3f34_fdf4) {
            let side = if 0. < c { 1. } else { -1. };
            return Ok(Feature::Disk {
                normal: axis,
                x_axis: m[0],
                centre: std::array::from_fn(|j| m[3][j] + side * h * axis[j]),
                radius: self.radius,
                oriented: c.abs() > f32::from_bits(0x3f7d_70a4),
            });
        }
        if c.abs() > f32::from_bits(0x3d34_3958) {
            let u = radial()?;
            let q: V = std::array::from_fn(|j| u[j] * self.radius + m[3][j]);
            return Ok(Feature::Segment([
                std::array::from_fn(|j| h * axis[j] + q[j]),
                std::array::from_fn(|j| q[j] - h * axis[j]),
            ]));
        }
        Ok(Feature::Side(Side {
            normal: radial()?,
            axis,
            bottom: std::array::from_fn(|j| m[3][j] - h * axis[j]),
            radius: self.radius + self.margin,
            height: h * 2.,
        }))
    }
}

impl SphereShape {
    fn valid(&self) -> bool {
        valid_matrix(&self.matrix)
            && self.radius.is_finite()
            && self.radius > 0.
            && self.radius < 1e3
            && self.margin.is_finite()
            && (0. ..=1.).contains(&self.margin)
    }
}

/// B47590's typed cylinder feature.
enum Feature {
    /// Type 2: the cap facing the direction, margin = shell.
    Disk {
        normal: V,
        x_axis: V,
        centre: V,
        radius: f32,
        oriented: bool,
    },
    /// Type 1 with two vertices, margin = shell, never a reference.
    Segment([V; 2]),
    /// Type 3, always a reference candidate.
    Side(Side),
}

/// The side: radial normal, axis, bottom centre, radius plus shell and the
/// full height (margin already in the radius).
struct Side {
    normal: V,
    axis: V,
    bottom: V,
    radius: f32,
    height: f32,
}

/// AE16A0 state: parameter interval with entry/exit codes (ADBAC0).
#[derive(Default)]
struct Interval {
    set: bool,
    t: [f32; 2],
    code: [i32; 2],
}

impl Interval {
    /// ADBAC0.
    fn add(&mut self, t: f32, code: i32) {
        if !self.set {
            *self = Self {
                set: true,
                t: [t, t],
                code: [code, code],
            };
            return;
        }
        if t < self.t[0] {
            self.t[0] = t;
            self.code[0] = code;
        }
        if self.t[1] <= t && t != self.t[1] {
            self.t[1] = t;
            self.code[1] = code;
        }
    }
}

/// AE16A0: the segment `origin + t * delta` (t in 0..1) against the finite
/// cylinder with `frame` rows (radial, axis, tangent), `centre`, half height
/// `h` and radius `r`. Codes: 2/-2 top/bottom cap, 1 side.
fn cut(origin: V, delta: V, frame: &[V; 3], centre: V, h: f32, r: f32) -> Option<Interval> {
    let r2 = r * r;
    let o = local(sub(origin, centre), frame);
    let d = local(delta, frame);
    let mut out = Interval {
        set: false,
        t: [0.; 2],
        code: [-999; 2],
    };
    let tiny = f32::from_bits(0x3586_37bd);
    if tiny < d[1].abs() {
        let top = (h - o[1]) / d[1];
        let bottom = ((-0. - h) - o[1]) / d[1];
        if top < 0. && bottom < 0. {
            return None;
        }
        let q = along(o, top, d);
        if q[2] * q[2] + q[0] * q[0] <= r2 {
            out.add(top, 2);
        }
        let q = along(o, bottom, d);
        if q[0] * q[0] + q[2] * q[2] <= r2 {
            out.add(bottom, -2);
        }
    }
    let a = d[0] * d[0] + d[2] * d[2];
    if tiny < d[2].abs() + d[0].abs() && tiny < a.abs() {
        let b = (o[2] * d[2] + d[0] * o[0]) * 2.;
        let c = o[0] * o[0] + o[2] * o[2];
        let disc = b * b - (c - r2) * a * 4.;
        if disc < 0. {
            return None;
        }
        let s = disc.sqrt();
        let roots = [(s - b) / (a * 2.), ((-0. - b) - s) / (a * 2.)];
        for t in roots {
            let y = t * d[1] + o[1];
            if -0. - h <= y && y <= h {
                out.add(t, 1);
            }
        }
    }
    let [low, high] = out.t;
    if !out.set || !(0. <= low || 0. <= high) || !(low <= 1. || high <= 0.) {
        return None;
    }
    out.t[0] = low.clamp(0., 1.);
    out.t[1] = high.clamp(0., 1.);
    Some(out)
}

/// Remaining-capacity publication shared by the handlers: AE2FA0's records
/// for a cylinder-first call are flipped and carry `-normal` in their own
/// frame (the published normal pushes the mesh).
struct Sink<'a> {
    contacts: &'a mut Vec<Contact>,
    limit: usize,
    normal: V,
}

impl Sink<'_> {
    #[allow(clippy::neg_multiply)] // Original multiply preserves signed zeros.
    fn push(&mut self, point: V, depth: f32) {
        let own = self.normal.map(|x| -1. * x);
        if self.contacts.len() < self.limit
            && !contact_manifold::duplicate(self.contacts, point, own, depth)
        {
            self.contacts.push(Contact {
                point,
                normal: self.normal,
                penetration: depth,
                flipped: true,
            });
        }
    }
    fn full(&self) -> bool {
        self.contacts.len() >= self.limit
    }
}

/// AE2710: mesh face edges (`polygon`, margin-shifted) against the side.
/// `normal` is the published normal (from the cylinder to the mesh).
fn side_contacts(side: &Side, polygon: &[V], sink: &mut Sink<'_>) -> Result<(), Error> {
    let h = side.height * 0.5;
    let tangent = contact_manifold::normalize(cross(side.normal, side.axis))?;
    let frame = [side.normal, side.axis, tangent];
    let centre = along(side.bottom, h, side.axis);
    let r = side.radius;
    // AE2710 probes along its own (cylinder-first) normal times -2r.
    let probe = sink.normal.map(|x| r * 2. * x);
    let edges = if polygon.len() < 3 { 1 } else { polygon.len() };
    for i in 0..edges {
        if sink.full() {
            break;
        }
        let start = polygon[i];
        let delta = sub(polygon[(i + 1) % polygon.len()], start);
        let Some(hit) = cut(start, delta, &frame, centre, h, r) else {
            continue;
        };
        let middle = along(start, (hit.t[0] + hit.t[1]) * 0.5, delta);
        if let Some(out) = cut(middle, probe, &frame, centre, h * 2., r) {
            sink.push(middle, r * 2. * out.t[1]);
        }
        for k in 0..2 {
            if sink.full() {
                break;
            }
            let t = hit.t[k];
            let point = along(start, t, delta);
            // The original probes the end's depth from the edge start
            // (0xAE2B8F passes the same stack slot as the edge cut).
            let depth = if t <= 0. || 1. <= t || hit.code[k] != 1 {
                cut(start, probe, &frame, centre, h * 2., r).map_or(0., |o| r * 2. * o.t[1])
            } else {
                0.
            };
            sink.push(point, depth);
        }
    }
    Ok(())
}

/// AE1BD0: polygon vertices inside the cap circle plus its edge crossings.
fn circle_clip(polygon: &[V], centre: V, u: V, v: V, r: f32) -> Vec<V> {
    let mut out = Vec::new();
    let n = polygon.len();
    for i in 0..n {
        let p = polygon[i];
        let q = polygon[(i + 1) % n];
        let rp = sub(p, centre);
        let rq = sub(q, centre);
        let (a0, b0) = (dot(u, rp), dot(v, rp));
        let (da, db) = (dot(u, rq) - a0, dot(v, rq) - b0);
        if a0 * a0 + b0 * b0 < r * r {
            out.push(p);
        }
        let tiny = f32::from_bits(0x3586_37bd);
        let a = db * db + da * da;
        if tiny < da.abs() + db.abs() && tiny < a.abs() {
            let b = (b0 * db + a0 * da) * 2.;
            let disc = b * b - ((a0 * a0 + b0 * b0) - r * r) * a * 4.;
            if 0. <= disc {
                let s = disc.sqrt();
                let mut first = (s - b) / (a * 2.);
                let mut second = ((-0. - b) - s) / (a * 2.);
                if second < first {
                    std::mem::swap(&mut first, &mut second);
                }
                for t in [first, second] {
                    if 0. < t && t < 1. {
                        out.push(std::array::from_fn(|j| t * (q[j] - p[j]) + p[j]));
                    }
                }
            }
        }
    }
    out
}

/// AE2210: the cap disk as a triangle on its rim, first vertex towards
/// `direction` in the cap plane (the x axis for a reference disk or a
/// direction along the axis), then rotated by 2.0944 rad about the normal.
fn disk_triangle(
    normal: V,
    x_axis: V,
    centre: V,
    radius: f32,
    oriented: bool,
    direction: V,
) -> Result<[V; 3], Error> {
    let c = dot(direction, normal);
    let first = if !oriented && c.abs() <= f32::from_bits(0x3f7f_f972) {
        contact_manifold::normalize(std::array::from_fn(|j| direction[j] - c * normal[j]))?
    } else {
        x_axis
    };
    let angle = f32::from_bits(0x4006_0a92);
    let (sin, cos) = angle.sin_cos();
    let rotate = |v: V| -> V {
        let k = cross(normal, v);
        let d = dot(normal, v);
        std::array::from_fn(|j| {
            if j == 3 {
                0.
            } else {
                v[j] * cos + k[j] * sin + normal[j] * d * (1. - cos)
            }
        })
    };
    let a = first.map(|x| radius * x);
    let b = rotate(a);
    let c = rotate(b);
    Ok([
        std::array::from_fn(|j| centre[j] + a[j]),
        std::array::from_fn(|j| centre[j] + b[j]),
        std::array::from_fn(|j| centre[j] + c[j]),
    ])
}

/// The fallback contact AE2FA0 adds after a short manifold.
fn fallback(contacts: &mut Vec<Contact>, s: &Settings, limit: usize) {
    if contacts.len() < limit
        && !contact_manifold::duplicate(contacts, s.fallback_point, s.normal, s.fallback_depth)
    {
        contacts.push(Contact {
            point: s.fallback_point,
            normal: s.normal,
            penetration: s.fallback_depth,
            flipped: false,
        });
    }
}

/// AE2FA0 for the mesh face against the cylinder feature.
#[allow(clippy::neg_multiply)] // Original margin/sign operations retain -0 lanes.
fn cylinder_manifold(
    mesh_face: Face<'_>,
    cylinder: &CylinderShape,
    s: &Settings,
) -> Result<Vec<Contact>, Error> {
    let limit = s.capacity;
    let segment_face = |vertices: &[V; 2], margin: f32| -> Result<Vec<Contact>, Error> {
        let face = Face {
            normal: [0., 0., 0., 1.],
            vertices,
            oriented: false,
            margin,
        };
        contact_manifold::assemble_rule([mesh_face, face], s, &[], true)
    };
    // Common normal: the reference's, signed along the query normal.
    let signed = |n: V| {
        if dot(n, s.normal) < 0. {
            n.map(|x| -1. * x)
        } else {
            n
        }
    };
    let shifted = |n: V| -> Vec<V> {
        let negative = n.map(|x| -1. * x);
        mesh_face
            .vertices
            .iter()
            .map(|p| std::array::from_fn(|j| p[j] + mesh_face.margin * negative[j]))
            .collect()
    };
    let mut contacts = Vec::new();
    match cylinder.feature(s.normal)? {
        Feature::Side(side) if mesh_face.oriented => {
            // AE2F10: the generator nearest the direction.
            let p0 = along(side.bottom, side.radius, side.normal);
            let p1 = along(p0, side.height, side.axis);
            return segment_face(&[p0, p1], 0.);
        }
        Feature::Side(side) => {
            let normal = signed(side.normal);
            let polygon = shifted(normal);
            let mut sink = Sink {
                contacts: &mut contacts,
                limit,
                normal,
            };
            side_contacts(&side, &polygon, &mut sink)?;
        }
        Feature::Segment(vertices) => {
            if !mesh_face.oriented {
                fallback(&mut contacts, s, limit);
                return Ok(contacts);
            }
            return segment_face(&vertices, cylinder.margin);
        }
        Feature::Disk {
            normal,
            x_axis,
            centre,
            radius,
            oriented,
        } => {
            if !mesh_face.oriented && !oriented {
                fallback(&mut contacts, s, limit);
                return Ok(contacts);
            }
            let common = signed(if oriented { normal } else { mesh_face.normal });
            if oriented {
                // AEB8D0 with a reference disk: the face inside the circle,
                // then below the cap plane.
                let disk_centre = along(centre, cylinder.margin, common);
                let v = contact_manifold::normalize(cross(x_axis, normal))?;
                let inside = circle_clip(&shifted(common), disk_centre, x_axis, v, radius);
                if inside.len() > 1 {
                    let plane = common.map(|x| -1. * x);
                    let below = contact_manifold::clip(&inside, disk_centre, plane)?;
                    let mut sink = Sink {
                        contacts: &mut contacts,
                        limit,
                        normal: common,
                    };
                    for p in below {
                        let depth = dot(sub(p, disk_centre), plane).max(0.);
                        sink.push(p, depth);
                        if sink.full() {
                            break;
                        }
                    }
                    if contacts.len() <= 2 {
                        fallback(&mut contacts, s, limit);
                    }
                    return Ok(contacts);
                }
            }
            let triangle = disk_triangle(normal, x_axis, centre, radius, oriented, common)?;
            let face = Face {
                normal,
                vertices: &triangle,
                oriented,
                margin: cylinder.margin,
            };
            return contact_manifold::assemble_faces([mesh_face, face], s, &[]);
        }
    }
    if contacts.len() <= 2 {
        fallback(&mut contacts, s, limit);
    }
    Ok(contacts)
}

/// Contacts of `mesh` at `matrix` (margin `query.margin`, sampled directions
/// in the mesh frame) against `shape`, at most `capacity` (<= 16). Empty when
/// the shapes are farther apart than their margins. The published normal
/// pushes the mesh, as for boxes.
pub fn contacts(
    mesh: &Mesh,
    matrix: Matrix,
    shape: &RoundShape,
    query: &ContactSettings,
    capacity: usize,
) -> Result<Vec<Contact>, Error> {
    let valid = match shape {
        RoundShape::Cylinder(c) => c.valid(),
        RoundShape::Sphere(s) => s.valid(),
    };
    if capacity > 16 || !valid {
        return Err(Error("invalid round contact controls"));
    }
    if capacity == 0 {
        return Ok(Vec::new());
    }
    let rotate = |d: V| -> V {
        std::array::from_fn(|j| {
            if j == 3 {
                0.
            } else {
                (d[0] * matrix[0][j] + d[1] * matrix[1][j]) + d[2] * matrix[2][j]
            }
        })
    };
    let settings = ContactSettings {
        margin: query.margin,
        directions: query.directions.map(rotate),
    };
    let (pose, centre, margin, shape_margin) = match shape {
        RoundShape::Cylinder(c) => (c.matrix, [c.matrix[3]], c.margin, c.margin),
        // B4B0A0: the centre pushed by radius plus shell.
        RoundShape::Sphere(s) => (s.matrix, [s.matrix[3]], s.radius + s.margin, s.margin),
    };
    let support = match shape {
        RoundShape::Cylinder(c) => Static::Cylinder(c),
        RoundShape::Sphere(_) => Static::Points(&centre),
    };
    let mut context = Context::new();
    let query = context.contact_static(
        mesh,
        matrix,
        support,
        margin,
        Extra::Axes([pose[0], pose[1], pose[2]]),
        [0.; 4],
        &settings,
    )?;
    let Some(witnesses) = query.witnesses else {
        return Ok(Vec::new());
    };
    // AE8AD0: depth is the witness distance; nothing at or below 1e-3.
    let difference = sub(witnesses[0], witnesses[1]);
    let square = dot(difference, difference);
    let depth = if square == 0. {
        0.
    } else {
        let seed = crate::spawn::reference_seed(square, false)?;
        let product = seed * square;
        ((1. - product * seed) * product) * 0.5 + product
    };
    if depth <= f32::from_bits(0x3a83_126f) {
        return Ok(Vec::new());
    }
    let inverse = 1. / depth;
    let normal = difference.map(|x| inverse * x);
    let (normal, witnesses) = contact_manifold::orient(pose[3], matrix[3], normal, witnesses)?;
    let s = Settings {
        margin: settings.margin,
        shape_margin,
        normal,
        fallback_point: witnesses[1],
        fallback_depth: depth,
        capacity,
    };
    match shape {
        RoundShape::Sphere(_) => {
            let mut contacts = Vec::new();
            fallback(&mut contacts, &s, capacity);
            Ok(contacts)
        }
        RoundShape::Cylinder(c) => {
            let (mesh_feature, _) = contact_manifold::mesh_feature(&mut context, mesh, matrix, &s)?;
            cylinder_manifold(mesh_feature.face(), c, &s)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube(half: f32) -> Mesh {
        let points: Vec<V> = (0..8)
            .map(|i| {
                [
                    if i & 1 == 0 { -half } else { half },
                    if i & 2 == 0 { -half } else { half },
                    if i & 4 == 0 { -half } else { half },
                    1.,
                ]
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
        // collision_mesh face normals follow the reversed winding.
        let faces = faces.map(|[a, b, c]| [a, c, b]);
        crate::collision_mesh::build(&points, &faces, 0).unwrap()
    }

    fn at(x: f32, y: f32, z: f32) -> Matrix {
        [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [x, y, z, 1.],
        ]
    }

    fn settings() -> ContactSettings {
        let axes = [
            [1., 0., 0., 0.],
            [-1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., -1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., -1., 0.],
        ];
        ContactSettings {
            margin: 0.02,
            directions: std::array::from_fn(|i| axes[i % 6]),
        }
    }

    fn barrel() -> RoundShape {
        RoundShape::Cylinder(CylinderShape {
            matrix: at(0., 0.45, 0.),
            radius: 0.3,
            half_height: 0.45,
            margin: 0.02,
        })
    }

    #[test]
    fn cylinder_support_is_round_with_flat_caps() {
        let RoundShape::Cylinder(c) = barrel() else {
            unreachable!()
        };
        assert_eq!(c.support([1., 0.5, 0., 0.]), [0.3, 0.9, 0., 1.]);
        assert_eq!(c.support([0., -1., 0., 0.]), [0., 0., 0., 1.]);
        let p = c.support([1., 0., 1., 0.]);
        assert!((p[0] - 0.3 / 2f32.sqrt()).abs() < 1e-6 && p[1] == 0.9);
    }

    /// A cube pushed into a standing barrel's side: contacts push it away
    /// horizontally, not through the barrel.
    #[test]
    fn a_mesh_against_a_barrel_side_is_pushed_out_sideways() {
        let mesh = cube(0.5);
        for (x, z) in [(0.75, 0.), (0.7, 0.2), (-0.72, -0.1)] {
            let hits = contacts(&mesh, at(x, 0.5, z), &barrel(), &settings(), 16).unwrap();
            assert!(!hits.is_empty(), "{x} {z}");
            for c in &hits {
                assert!(c.normal[1].abs() < 0.2, "{c:?}");
                assert!(c.normal[0] * x.signum() > 0.7, "{c:?}");
                assert!(c.penetration >= 0. && c.penetration < 0.2, "{c:?}");
            }
        }
    }

    /// A cube resting on a barrel's top cap: an upward manifold.
    #[test]
    fn a_mesh_on_a_barrel_top_is_held_up() {
        let mesh = cube(0.5);
        let hits = contacts(&mesh, at(0.1, 1.38, 0.), &barrel(), &settings(), 16).unwrap();
        assert!(hits.len() >= 3, "{hits:?}");
        for c in &hits {
            assert!(c.normal[1] > 0.99, "{c:?}");
        }
        assert!(
            contacts(&mesh, at(0.1, 1.5, 0.), &barrel(), &settings(), 16)
                .unwrap()
                .is_empty()
        );
    }

    /// A sphere gives AE2FA0's single fallback contact.
    #[test]
    fn a_sphere_gives_one_contact_along_the_centre_line() {
        let mesh = cube(0.5);
        let ball = RoundShape::Sphere(SphereShape {
            matrix: at(0., 0., 0.),
            radius: 1.,
            margin: 0.02,
        });
        let hits = contacts(&mesh, at(1.45, 0., 0.), &ball, &settings(), 16).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].normal[0] > 0.99, "{hits:?}");
        assert!((hits[0].penetration - 0.09).abs() < 0.01, "{hits:?}");
        assert!(
            contacts(&mesh, at(1.6, 0., 0.), &ball, &settings(), 16)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn segment_cuts_report_entry_and_exit() {
        let frame = [[1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.]];
        let centre = [0., 0., 0., 1.];
        let hit = cut([-2., 0., 0., 1.], [4., 0., 0., 0.], &frame, centre, 1., 1.).unwrap();
        assert_eq!(hit.code, [1, 1]);
        assert!((hit.t[0] - 0.25).abs() < 1e-6 && (hit.t[1] - 0.75).abs() < 1e-6);
        let hit = cut([0.5, 2., 0., 1.], [0., -4., 0., 0.], &frame, centre, 1., 1.).unwrap();
        assert_eq!(hit.code, [2, -2]);
        assert!(cut([2., 0., 0., 1.], [0., 1., 0., 0.], &frame, centre, 1., 1.).is_none());
    }
}
