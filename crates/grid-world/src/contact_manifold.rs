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

#[derive(Clone, Debug, PartialEq)]
pub struct Contact {
    pub point: V,
    /// Published normal, including static-reference sign correction.
    pub normal: V,
    pub penetration: f32,
    pub flipped: bool,
}
pub struct Settings {
    pub margin: f32,
    /// Margin of the static second shape: the triangle's (its packed
    /// shape's +0x220) or the box's.
    pub shape_margin: f32,
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

/// Reoriented query witnesses alongside the complete retained manifold.
pub struct OrientedOutput {
    pub manifold: Output,
    pub normal: V,
    pub witnesses: [V; 2],
}

pub(crate) fn witness_length(square: f32) -> Result<f32, Error> {
    if square == 0. {
        return Ok(0.);
    }
    let seed = crate::spawn::reference_seed(square, false)?;
    let product = seed * square;
    Ok(((1. - product * seed) * product) * 0.5 + product)
}

pub(crate) fn orient(
    triangle_anchor: V,
    mesh_anchor: V,
    normal: V,
    witnesses: [V; 2],
) -> Result<(V, [V; 2]), Error> {
    if [triangle_anchor, mesh_anchor, witnesses[0], witnesses[1]]
        .iter()
        .any(|p| p[3] != 1. || p.iter().any(|x| !x.is_finite() || x.abs() > 1e5))
        || normal[3] != 0.
        || normal.iter().any(|x| !x.is_finite() || x.abs() > 10000.)
    {
        return Err(Error("invalid manifold orientation controls"));
    }
    let a = sub(witnesses[0], triangle_anchor);
    let b = sub(mesh_anchor, witnesses[1]);
    let squares = [dot(a, a), dot(b, b)];
    let lengths = [witness_length(squares[0])?, witness_length(squares[1])?];
    // AEE640's 3727C5AC (1e-5) "degenerate" length is below the rounding of
    // world coordinates on every circuit (one ulp is 6.1e-5 at 600 m), so a
    // displacement counts as degenerate up to 32 ulps of its largest
    // endpoint coordinate as well (AUTHORED, F342).
    let degenerate = |p: V, q: V| {
        let scale = p.iter().chain(&q).take(7).fold(0f32, |m, v| m.max(v.abs()));
        f32::from_bits(0x3727_c5ac).max(32. * f32::EPSILON * scale)
    };
    let tolerance = [
        degenerate(witnesses[0], triangle_anchor),
        degenerate(mesh_anchor, witnesses[1]),
    ];
    let score = if lengths[0] <= tolerance[0] || lengths[1] <= tolerance[1] {
        // AEE640 (0xAEEB40 CMPLTPS, 0x4682A0 lane-0 mask) takes the sign of
        // `dot(normal, smaller displacement)` here, and the smaller one is
        // the degenerate one by construction: zero when the triangle witness
        // is the anchor vertex, else rounding. So the original keeps a margin
        // contact's inverted witness normal whenever the closest feature is
        // vertex 0, and flips by noise otherwise, and its normalized branch
        // trusts a rounding-sized vector just above 1e-5; all of them publish
        // contacts through the whole car or piece (F342: kerb vertices at
        // Nuerburgring and Donington). Port the intended rule instead (owner
        // rule F320; deviation recorded): orient by the other, informative
        // displacement, and keep the normal only if both are degenerate.
        let (other, informative) = if lengths[0] <= tolerance[0] {
            (b, lengths[1] > tolerance[1])
        } else {
            (a, true)
        };
        if !informative { 0. } else { dot(normal, other) }
    } else {
        let mut scores = [0.; 2];
        for (i, v) in [a, b].into_iter().enumerate() {
            let reciprocal = crate::spawn::reference_seed(lengths[i], true)?;
            let correction = 1. - reciprocal * lengths[i];
            let normalized = v.map(|x| {
                let partial = reciprocal * x;
                correction * partial + partial
            });
            scores[i] = dot(normal, normalized);
        }
        if scores[1].abs() < scores[0].abs() {
            scores[0]
        } else {
            scores[1]
        }
    };
    if !score.is_finite() {
        return Err(Error("nonfinite manifold orientation result"));
    }
    if score < 0. {
        #[allow(clippy::neg_multiply)] // Original multiply preserves signed zeros.
        let normal = normal.map(|x| -1. * x);
        Ok((normal, [witnesses[1], witnesses[0]]))
    } else {
        Ok((normal, witnesses))
    }
}

/// Selected AEE640 -> AE2FA0 composition, retaining the prior manifold.
/// `triangle_witness` and `s.fallback_point` are the two query witnesses;
/// normal and depth are caller supplied. The second orientation anchor is the
/// moving mesh transform origin. Packed normal selection/correction is separate.
pub fn calculate_oriented_append(
    context: &mut Context,
    mesh: &Mesh,
    matrix: Matrix,
    triangle: [V; 3],
    s: &Settings,
    existing: &[Contact],
    triangle_witness: V,
) -> Result<OrientedOutput, Error> {
    let (normal, witnesses) = orient(
        triangle[0],
        matrix[3],
        s.normal,
        [triangle_witness, s.fallback_point],
    )?;
    let settings = Settings {
        margin: s.margin,
        shape_margin: s.shape_margin,
        normal,
        fallback_point: witnesses[1],
        fallback_depth: s.fallback_depth,
        capacity: s.capacity,
    };
    let manifold = calculate_append(context, mesh, matrix, triangle, &settings, existing)?;
    Ok(OrientedOutput {
        manifold,
        normal,
        witnesses,
    })
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
    let (mesh_feature, cache) = mesh_feature(context, mesh, matrix, s)?;
    let normal = normalize(cross(
        sub(triangle[1], triangle[0]),
        sub(triangle[2], triangle[0]),
    ))?;
    let triangle_feature = Feature::new(
        normal,
        triangle,
        f32::from_bits(0x3f7d_70a4) < dot(s.normal, normal).abs(),
        s.shape_margin,
    );
    Ok(([mesh_feature, triangle_feature], cache))
}

/// B45310: the mesh face incident to its support vertex along `-normal`.
#[allow(clippy::neg_multiply)] // Match the original SIMD multiply and signed zero.
pub(crate) fn mesh_feature(
    context: &mut Context,
    mesh: &Mesh,
    matrix: Matrix,
    s: &Settings,
) -> Result<(Feature, [usize; 7]), Error> {
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
    Ok((mesh_feature, cache))
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

/// A polygon feature: unit normal, convex vertices in order, the published
/// orientation flag and the shape margin.
pub(crate) struct Face<'a> {
    pub normal: V,
    pub vertices: &'a [V],
    pub oriented: bool,
    pub margin: f32,
}

impl Feature {
    pub(crate) fn face(&self) -> Face<'_> {
        Face {
            normal: self.normal,
            vertices: &self.vertices,
            oriented: self.oriented,
            margin: self.margin,
        }
    }
}

fn assemble(
    features: &[Feature; 2],
    s: &Settings,
    existing: &[Contact],
) -> Result<Vec<Contact>, Error> {
    assemble_faces([features[0].face(), features[1].face()], s, existing)
}

/// Polygon/polygon clipping (AE2FA0's D3AD70 entry for two face features):
/// `a` belongs to the moving body, `b` to the static shape.
#[allow(clippy::neg_multiply)] // Original margin/sign operations retain -0 lanes.
pub(crate) fn assemble_faces(
    features: [Face<'_>; 2],
    s: &Settings,
    existing: &[Contact],
) -> Result<Vec<Contact>, Error> {
    let [a, b] = &features;
    let mut contacts = existing.to_vec();
    let limit = contacts.len() + s.capacity;
    if s.capacity == 0 {
        return Ok(contacts);
    }
    let clipping = a.oriented || b.oriented;
    if clipping {
        // The mesh has a moving body; the selected triangle has no body.
        let reference_a = a.oriented && !b.oriented;
        let mut normal = if reference_a { a.normal } else { b.normal };
        if dot(normal, s.normal) < 0. {
            normal = normal.map(|x| -1. * x);
        }
        let negative = normal.map(|x| -1. * x);
        let av: Vec<V> = a
            .vertices
            .iter()
            .map(|p| std::array::from_fn(|j| p[j] + a.margin * negative[j]))
            .collect();
        let bv: Vec<V> = b
            .vertices
            .iter()
            .map(|p| std::array::from_fn(|j| p[j] + b.margin * normal[j]))
            .collect();
        let (reference, incident, plane, feature_normal) = if reference_a {
            (av, bv, normal, normal)
        } else {
            (bv, av, negative, b.normal)
        };
        let mut polygon = clip(&incident, reference[0], plane)?;
        let n = reference.len();
        for i in 0..n {
            let next = reference[(i + 1) % n];
            let third = reference[(i + 2) % n];
            let mut edge = cross(sub(reference[i], next), feature_normal);
            if dot(sub(next, third), edge) > 0. {
                edge = edge.map(|x| -1. * x);
            }
            polygon = clip(&polygon, reference[i], normalize(edge)?)?;
        }
        for point in polygon {
            let depth = dot(sub(point, reference[0]), plane);
            let depth = if depth < 0. { 0. } else { depth };
            if contacts.len() < limit && !duplicate(&mut contacts, point, plane, depth) {
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
                if contacts.len() == limit {
                    break;
                }
            }
        }
    }
    if (!clipping || contacts.len() <= 2)
        && contacts.len() != limit
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
    calculate_append(context, mesh, matrix, triangle, s, &[])
}

/// Append to a retained manifold, merging duplicates across prior triangles.
/// `capacity` is the number of remaining slots; the complete result is bounded
/// to sixteen contacts. Existing publication metadata belongs to the caller;
/// merging changes only penetration. Errors preserve the context and input.
pub fn calculate_append(
    context: &mut Context,
    mesh: &Mesh,
    matrix: Matrix,
    triangle: [V; 3],
    s: &Settings,
    existing: &[Contact],
) -> Result<Output, Error> {
    if existing.len() > 16
        || s.capacity > 16 - existing.len()
        || existing.iter().any(|c| {
            !c.penetration.is_finite()
                || c.penetration.abs() > 1e6
                || c.point.iter().any(|x| !x.is_finite() || x.abs() > 1e5)
                || c.point[3] != 1.
                || c.normal.iter().any(|x| !x.is_finite())
                || c.normal[3] != 0.
                || (dot(c.normal, c.normal) - 1.).abs() > 0.001
        })
        || !s.margin.is_finite()
        || !(0. ..=1e6).contains(&s.margin)
        || !s.shape_margin.is_finite()
        || !(0. ..=1e6).contains(&s.shape_margin)
        || !s.fallback_depth.is_finite()
        || s.fallback_depth.abs() > 10000.
        || s.normal.iter().any(|x| !x.is_finite() || x.abs() > 10000.)
        || s.normal[3] != 0.
        || (dot(s.normal, s.normal) - 1.).abs() > 0.001
        // Port guards: AE2FA0/B45310 bound no coordinate. World positions
        // share the 1e5 domain of the triangle and existing points.
        || s.fallback_point
            .iter()
            .any(|x| !x.is_finite() || x.abs() > 1e5)
        || s.fallback_point[3] != 1.
        || matrix
            .iter()
            .flatten()
            .any(|x| !x.is_finite() || x.abs() > 1e5)
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
    let contacts = assemble(&features, s, existing)?;
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
    fn orientation_swaps_witnesses_and_preserves_signed_normal_lanes() {
        let anchor = [0., 0., 0., 1.];
        let normal = [0., 1., 0., 0.];
        let witnesses = [[1., 1., 0., 1.], [0., 2., 0., 1.]];
        let (out, points) = orient(anchor, anchor, normal, witnesses).unwrap();
        assert_eq!(
            out.map(f32::to_bits),
            [
                (-0f32).to_bits(),
                (-1f32).to_bits(),
                (-0f32).to_bits(),
                (-0f32).to_bits()
            ]
        );
        assert_eq!(points, [witnesses[1], witnesses[0]]);
        // A witness exactly at the triangle anchor: the original kept the
        // normal (zero projection); the mesh side now decides (F342).
        let (out, points) = orient(anchor, anchor, normal, [anchor, witnesses[1]]).unwrap();
        assert_eq!(out[1], -1.);
        assert_eq!(points, [witnesses[1], anchor]);
        let (out, points) = orient(anchor, anchor, normal, [anchor, [0., -2., 0., 1.]]).unwrap();
        assert_eq!(out, normal);
        assert_eq!(points, [anchor, [0., -2., 0., 1.]]);
        // A sub-threshold displacement (zero or rounding at track
        // coordinates) carries no orientation: the other one decides, whatever
        // the short one's sign (F342; the original used the short one).
        for short in [-1e-6, 0., 1e-6, -9e-6] {
            for long in [-1f32, 1.] {
                let w = [[0., short, 0., 1.], [0., long, 0., 1.]];
                let (out, _) = orient(anchor, anchor, normal, w).unwrap();
                // b = anchor - w[1] = -long along the normal.
                assert_eq!(out[1], -long.signum(), "{short} {long}");
            }
        }
        // At track height one ulp is 6e-5: a 1e-4 offset from the anchor is
        // rounding and no longer outvotes the mesh side (seed 1020, F342).
        let high = [0., 612., 0., 1.];
        let w = [[0., 612. - 1e-4, 0., 1.], [0., 611.8, 0., 1.]];
        assert_eq!(orient(high, high, normal, w).unwrap(), (normal, w));
        // Both degenerate: unchanged.
        let w = [[0., -1e-6, 0., 1.], [0., 1e-6, 0., 1.]];
        assert_eq!(orient(anchor, anchor, normal, w).unwrap(), (normal, w));
        // Above the threshold both displacements still decide.
        let (out, _) = orient(
            anchor,
            anchor,
            normal,
            [[0., -1e-3, 0., 1.], [0., 2., 0., 1.]],
        )
        .unwrap();
        assert_eq!(out[1], -1.);
        assert!(orient(anchor, anchor, normal, [[f32::NAN, 0., 0., 1.], anchor]).is_err());
    }
    #[test]
    fn retained_count_controls_fallback_and_duplicate_depth() {
        let a = Feature::new(
            [0., 1., 0., 0.],
            [[0., 0., 0., 1.], [1., 0., 0., 1.], [0., 0., 1., 1.]],
            true,
            0.,
        );
        let b = Feature::new(
            [0., 1., 0., 0.],
            [[10., 1., 10., 1.], [11., 1., 10., 1.], [10., 1., 11., 1.]],
            false,
            0.,
        );
        let prior: Vec<_> = (0..3)
            .map(|i| Contact {
                point: [i as f32, -2., 0., 1.],
                normal: [0., 1., 0., 0.],
                penetration: 0.1,
                flipped: false,
            })
            .collect();
        let mut s = Settings {
            margin: 0.,
            shape_margin: 0.,
            normal: [0., 1., 0., 0.],
            fallback_point: [20., 0., 0., 1.],
            fallback_depth: 0.5,
            capacity: 13,
        };
        let mut features = [a, b];
        assert_eq!(assemble(&features, &s, &prior).unwrap(), prior);
        assert_eq!(assemble(&features, &s, &prior[..2]).unwrap().len(), 3);
        features[0].oriented = false;
        assert_eq!(assemble(&features, &s, &prior).unwrap().len(), 4);
        s.fallback_point = prior[1].point;
        let merged = assemble(&features, &s, &prior).unwrap();
        assert_eq!(merged.len(), 3);
        assert_eq!(merged[1].penetration, 0.5);
        assert_eq!(merged[1].point, prior[1].point);
        s.capacity = 0;
        assert_eq!(assemble(&features, &s, &prior).unwrap(), prior);
    }

    #[test]
    fn append_failure_preserves_retained_contacts_and_cache() {
        let mesh = crate::collision_mesh::build(
            &[
                [1., 1., 1., 1.],
                [-1., -1., 1., 1.],
                [-1., 1., -1., 1.],
                [1., -1., -1., 1.],
            ],
            &[[0, 1, 2], [0, 3, 1], [0, 2, 3], [1, 3, 2]],
            0,
        )
        .unwrap();
        let matrix = [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ];
        let prior = vec![Contact {
            point: [0., 0., 0., 1.],
            normal: [0., 1., 0., 0.],
            penetration: 0.25,
            flipped: false,
        }];
        let saved = prior.clone();
        let mut context = Context::new();
        let cache = context.cached_support();
        let mut s = Settings {
            margin: 0.,
            shape_margin: 0.,
            normal: [0., 1., 0., 0.],
            fallback_point: prior[0].point,
            fallback_depth: 1.,
            capacity: 15,
        };
        // Degenerate static feature fails after staged moving-feature support.
        assert!(
            calculate_append(
                &mut context,
                &mesh,
                matrix,
                [[0., 0., 0., 1.]; 3],
                &s,
                &prior
            )
            .is_err()
        );
        assert_eq!(context.cached_support(), cache);
        assert_eq!(prior, saved);
        s.capacity = 16;
        assert!(
            calculate_append(
                &mut context,
                &mesh,
                matrix,
                [[0., 0., 0., 1.]; 3],
                &s,
                &prior
            )
            .is_err()
        );
        assert_eq!(context.cached_support(), cache);
        s.capacity = 15;
        assert!(
            calculate_oriented_append(
                &mut context,
                &mesh,
                matrix,
                [[0., 0., 0., 1.]; 3],
                &s,
                &prior,
                [1., 1., 0., 1.]
            )
            .is_err()
        );
        assert_eq!(context.cached_support(), cache);
        assert_eq!(prior, saved);
    }

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
