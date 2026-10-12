//! Selected moving one-level mesh versus static source triangle manifold.
//! Features, clipping, duplicate merging and fallback precede physical solving.
use crate::{Error, collision_mesh::Mesh, convex_distance::Context, physical_pose::Matrix};

type V = [f32; 4];
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
fn normalize(v: V) -> Result<V, Error> {
    let square = dot(v, v);
    if !square.is_finite() || square <= 0. {
        return Err(Error("degenerate contact feature"));
    }
    let seed = crate::spawn::reference_seed(square, false)?;
    let factor = (1. - (seed * square) * seed) * (seed * 0.5) + seed;
    let out = v.map(|x| factor * x);
    if out.iter().any(|x| !x.is_finite()) {
        return Err(Error("contact feature exceeds range"));
    }
    Ok(out)
}
fn transform(v: V, m: Matrix) -> V {
    std::array::from_fn(|j| (v[3] * m[3][j] + v[1] * m[1][j]) + (v[2] * m[2][j] + v[0] * m[0][j]))
}

/// Whole selected 0xB8 feature storage plus eight diagnostic padding bytes.
/// Undefined bytes retain their explicit input fill in this representation.
#[derive(Clone)]
pub struct Feature {
    pub bytes: [u8; 0xc0],
    normal: V,
    vertices: [V; 3],
    oriented: bool,
    margin: f32,
}
impl Feature {
    fn new(normal: V, vertices: [V; 3], oriented: bool, margin: f32) -> Self {
        let mut bytes = [0xcd; 0xc0];
        for (o, v) in [
            (0, normal),
            (0x10, [0.; 4]),
            (0x20, vertices[0]),
            (0x30, vertices[1]),
            (0x40, vertices[2]),
        ] {
            for (j, x) in v.into_iter().enumerate() {
                bytes[o + j * 4..o + j * 4 + 4].copy_from_slice(&x.to_le_bytes());
            }
        }
        bytes[0xa0] = 3;
        for (o, v) in [(0xa4, 0), (0xa8, 0), (0xac, margin.to_bits()), (0xb4, 1)] {
            bytes[o..o + 4].copy_from_slice(&v.to_le_bytes());
        }
        bytes[0xb0] = oriented as u8;
        bytes[0xb1] = 0;
        Self {
            bytes,
            normal,
            vertices,
            oriented,
            margin,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Contact {
    pub point: V,
    /// Published normal, including static-reference sign correction.
    pub normal: V,
    pub penetration: f32,
    pub flipped: bool,
}
pub struct Settings {
    pub margin: f32,
    pub normal: V,
    pub fallback_point: V,
    pub fallback_depth: f32,
    pub capacity: usize,
}
pub struct Output {
    pub features: [Feature; 2],
    pub support_vertices: [usize; 7],
    pub contacts: Vec<Contact>,
}

fn word(b: &[u8], o: usize) -> usize {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap()) as usize
}
#[allow(clippy::neg_multiply)] // Match the original SIMD multiply and signed zero.
fn features(
    context: &mut Context,
    mesh: &Mesh,
    matrix: Matrix,
    triangle: [V; 3],
    s: &Settings,
) -> Result<([Feature; 2], [usize; 7]), Error> {
    let direction = s.normal.map(|x| -1. * x);
    let local = [
        dot(direction, matrix[0]),
        dot(direction, matrix[1]),
        dot(direction, matrix[2]),
        0.,
    ];
    let cache = context.feature_support(mesh, matrix, direction)?;
    let b = mesh.bytes();
    let vertex = 0x60 + cache[6] * 32;
    let faces = word(b, vertex + 0x14);
    let incidence = word(b, vertex + 0x18);
    let face_start = 0x60 + mesh.point_count() * 48;
    let mut best = None;
    for i in 0..faces {
        let face = (word(b, incidence + i * 4) - face_start) / 32;
        let score = dot(mesh.face_normal(face).unwrap(), local);
        if best.is_none_or(|(_, value)| value < score) {
            best = Some((face, score));
        }
    }
    let (face, score) = best.ok_or(Error("mesh support has no incident feature"))?;
    let ids: [usize; 3] = std::array::from_fn(|j| word(b, face_start + face * 32 + 16 + j * 4));
    let mesh_feature = Feature::new(
        transform(mesh.face_normal(face).unwrap(), matrix),
        ids.map(|i| transform(mesh.point(i).unwrap(), matrix)),
        f32::from_bits(0x3f7d_70a4) < score.abs(),
        s.margin,
    );
    let normal = normalize(cross(
        sub(triangle[1], triangle[0]),
        sub(triangle[2], triangle[0]),
    ))?;
    let triangle_feature = Feature::new(
        normal,
        triangle,
        f32::from_bits(0x3f7d_70a4) < dot(s.normal, normal).abs(),
        0.,
    );
    Ok(([mesh_feature, triangle_feature], cache))
}

/// Plane clipping preserves the original rotation of all-inside polygons.
fn clip(points: &[V], point: V, normal: V) -> Result<Vec<V>, Error> {
    let mut out = Vec::with_capacity(points.len() + 1);
    for i in 0..points.len() {
        let a = points[i];
        let b = points[(i + 1) % points.len()];
        let da = dot(sub(a, point), normal);
        let db = dot(sub(b, point), normal);
        let inside_a = 0. <= da;
        let inside_b = 0. <= db;
        if inside_a != inside_b {
            let t = (-1. / (db - da)) * da;
            out.push(std::array::from_fn(|j| t * (b[j] - a[j]) + a[j]));
        }
        if inside_b {
            out.push(b);
        }
    }
    if out.len() > 20 || out.iter().flatten().any(|x| !x.is_finite()) {
        return Err(Error("contact clipping exceeds range"));
    }
    Ok(out)
}

fn duplicate(contacts: &mut [Contact], point: V, normal: V, depth: f32) -> bool {
    for c in contacts {
        let score = dot(normal, c.normal);
        let score = if c.flipped { -0. - score } else { score };
        if (0..3).all(|j| (point[j] - c.point[j]).abs() < f32::from_bits(0x3c23_d70a))
            && f32::from_bits(0x3f7f_be77) < score
        {
            if c.penetration < depth {
                c.penetration = depth;
            }
            return true;
        }
    }
    false
}

#[allow(clippy::neg_multiply)] // Original margin/sign operations retain -0 lanes.
fn assemble(features: &[Feature; 2], s: &Settings) -> Result<Vec<Contact>, Error> {
    let [a, b] = features;
    let mut contacts = Vec::new();
    if s.capacity == 0 {
        return Ok(contacts);
    }
    if a.oriented || b.oriented {
        // The mesh has a moving body; the selected triangle has no body.
        let reference_a = a.oriented && !b.oriented;
        let mut normal = if reference_a { a.normal } else { b.normal };
        if dot(normal, s.normal) < 0. {
            normal = normal.map(|x| -1. * x);
        }
        let negative = normal.map(|x| -1. * x);
        let av = a
            .vertices
            .map(|p| std::array::from_fn(|j| p[j] + a.margin * negative[j]));
        let bv = b
            .vertices
            .map(|p| std::array::from_fn(|j| p[j] + b.margin * normal[j]));
        let (reference, incident, plane, feature_normal) = if reference_a {
            (av, bv, normal, normal)
        } else {
            (bv, av, negative, b.normal)
        };
        let mut polygon = clip(&incident, reference[0], plane)?;
        for i in 0..3 {
            let next = reference[(i + 1) % 3];
            let third = reference[(i + 2) % 3];
            let mut edge = cross(sub(reference[i], next), feature_normal);
            if dot(sub(next, third), edge) > 0. {
                edge = edge.map(|x| -1. * x);
            }
            polygon = clip(&polygon, reference[i], normalize(edge)?)?;
        }
        for point in polygon {
            let depth = dot(sub(point, reference[0]), plane);
            let depth = if depth < 0. { 0. } else { depth };
            if contacts.len() < s.capacity && !duplicate(&mut contacts, point, plane, depth) {
                contacts.push(Contact {
                    point,
                    normal: if reference_a {
                        plane
                    } else {
                        plane.map(|x| -1. * x)
                    },
                    penetration: depth,
                    flipped: !reference_a,
                });
                if contacts.len() == s.capacity {
                    break;
                }
            }
        }
    }
    if contacts.len() <= 2
        && contacts.len() != s.capacity
        && !duplicate(&mut contacts, s.fallback_point, s.normal, s.fallback_depth)
    {
        contacts.push(Contact {
            point: s.fallback_point,
            normal: s.normal,
            penetration: s.fallback_depth,
            flipped: false,
        });
    }
    Ok(contacts)
}

/// Complete selected B45310/B7C5D0 -> AE2FA0 -> polygon clipping/publication.
/// The normal, fallback witness and depth are supplied by the caller; this does
/// not yet implement the packed dispatcher's witness reorientation or traversal.
pub fn calculate(
    context: &mut Context,
    mesh: &Mesh,
    matrix: Matrix,
    triangle: [V; 3],
    s: &Settings,
) -> Result<Output, Error> {
    if s.capacity > 16
        || !s.margin.is_finite()
        || !(0. ..=1e6).contains(&s.margin)
        || !s.fallback_depth.is_finite()
        || s.fallback_depth.abs() > 10000.
        || s.normal.iter().any(|x| !x.is_finite() || x.abs() > 10000.)
        || s.normal[3] != 0.
        || (dot(s.normal, s.normal) - 1.).abs() > 0.001
        || s.fallback_point
            .iter()
            .any(|x| !x.is_finite() || x.abs() > 10000.)
        || s.fallback_point[3] != 1.
        || matrix
            .iter()
            .flatten()
            .any(|x| !x.is_finite() || x.abs() > 10000.)
        || triangle
            .iter()
            .flatten()
            .any(|x| !x.is_finite() || x.abs() > 1e5)
        || triangle.iter().any(|p| p[3] != 1.)
    {
        return Err(Error("invalid selected manifold controls"));
    }
    let mut staged = context.clone();
    let (features, support_vertices) = features(&mut staged, mesh, matrix, triangle, s)?;
    let contacts = assemble(&features, s)?;
    if s.capacity > 0 {
        *context = staged;
    }
    Ok(Output {
        features,
        support_vertices: if s.capacity > 0 {
            support_vertices
        } else {
            context.cached_support()
        },
        contacts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clipping_preserves_rotation_and_boundary_intersections() {
        let p = [[1., 0., 0., 1.], [1., 1., 0., 1.], [-1., 0., 0., 1.]];
        let q = clip(&p, [0., 0., 0., 1.], [1., 0., 0., 0.]).unwrap();
        assert_eq!(q, vec![p[1], [0., 0.5, 0., 1.], [0., 0., 0., 1.], p[0]]);
        assert_eq!(
            clip(&p, [-2., 0., 0., 1.], [1., 0., 0., 0.]).unwrap(),
            vec![p[1], p[2], p[0]]
        );
        let mut c = vec![Contact {
            point: p[0],
            normal: [0., 1., 0., 0.],
            penetration: 0.1,
            flipped: true,
        }];
        assert!(duplicate(&mut c, p[0], [0., -1., 0., 0.], 0.5));
        assert_eq!(c[0].penetration, 0.5);
        assert!(!duplicate(&mut c, p[0], [0., 1., 0., 0.], 1.));
    }
}
