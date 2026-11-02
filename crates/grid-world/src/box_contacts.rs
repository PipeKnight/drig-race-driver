//! AE8AD0 for a moving collision mesh (shape kind 7: a car or piece hull)
//! against a static box primitive (kind 0: route prop boxes). The original
//! world constructor B18E70 dispatches every pair of convex kinds {0, 3..7}
//! to AE8AD0: one B7F740/B7F520 query over the whole box (B7EA60 distance,
//! B7ED10 sampled penetration), AEE640 orientation and the AE2FA0 manifold
//! with the box face of B3D890. Packed road chunks (kind 11) instead go
//! triangle by triangle through AEA1F0 (`packed_contacts`).
//!
//! The box takes the packed path's static role (shape B of the query, second
//! manifold feature), so the published records keep the packed convention:
//! the normal pushes the mesh. AE8AD0 itself runs with the mesh as A; the two
//! assignments differ only in signs and witness order, which AEE640 and
//! AE2FA0 treat symmetrically. B7F740 expresses the box in the mesh's frame
//! for the distance and penetration queries, so the 44 sampled directions
//! stay in that frame, and the witnesses go back to the world.
use crate::{
    Error,
    contact_manifold::{self, Contact, Face, Settings},
    convex_distance::{ContactSettings, Context, Extra, Hull},
    physical_pose::Matrix,
};

type V = [f32; 4];
fn dot(a: V, b: V) -> f32 {
    (a[0] * b[0] + a[1] * b[1]) + (a[2] * b[2] + a[3] * b[3])
}
fn sub(a: V, b: V) -> V {
    std::array::from_fn(|j| a[j] - b[j])
}

/// B7F740's frame change for a rigid pose `matrix` (unit axes in rows 0..2,
/// origin in row 3): a world point or direction (`w` = 0) in the pose's
/// frame, and back. The distance loop then works near the moving body, not
/// a few kilometres from the origin.
pub(crate) fn to_frame(matrix: &Matrix, p: V) -> V {
    let d = if p[3] == 0. { p } else { sub(p, matrix[3]) };
    let local = |k: usize| (d[0] * matrix[k][0] + d[1] * matrix[k][1]) + d[2] * matrix[k][2];
    [local(0), local(1), local(2), p[3]]
}
pub(crate) fn from_frame(matrix: &Matrix, p: V) -> V {
    std::array::from_fn(|j| {
        if j == 3 {
            p[3]
        } else {
            (p[0] * matrix[0][j] + p[1] * matrix[1][j]) + (p[2] * matrix[2][j] + p[3] * matrix[3][j])
        }
    })
}
/// B7F740: a cached direction shorter than 1e-3 starts along world +x.
pub(crate) fn cold_start(direction: V) -> V {
    if dot(direction, direction) < 1e-6 { [1., 0., 0., 0.] } else { direction }
}
pub(crate) const IDENTITY: Matrix = [[1., 0., 0., 0.], [0., 1., 0., 0.], [0., 0., 1., 0.], [0., 0., 0., 1.]];

/// A box primitive: unit axes in rows 0..2, centre in row 3, half extents
/// along the axes and the shape margin (TEMPLATESHAPEBOX `shell`).
#[derive(Clone, Debug)]
pub struct BoxShape {
    pub matrix: Matrix,
    pub half: [f32; 3],
    pub margin: f32,
}

impl BoxShape {
    /// Corner `i` has local sign bits (x, y, z) = bits 0, 1, 2 of `i`.
    fn corner(&self, sign: [f32; 3]) -> V {
        let m = &self.matrix;
        let mut p = m[3];
        for k in 0..3 {
            let h = sign[k] * self.half[k];
            for (j, value) in p.iter_mut().enumerate().take(3) {
                *value += h * m[k][j];
            }
        }
        p[3] = 1.;
        p
    }
    pub(crate) fn corners(&self) -> [V; 8] {
        std::array::from_fn(|i| {
            self.corner(std::array::from_fn(
                |k| {
                    if i >> k & 1 == 0 { -1. } else { 1. }
                },
            ))
        })
    }

    /// B3D890: the face along the dominant local axis of `direction` (x on
    /// ties, then y), oriented when that component exceeds 0.99, four
    /// vertices in cyclic order.
    fn face(&self, direction: V) -> (V, [V; 4], bool) {
        let local: [f32; 3] = std::array::from_fn(|k| dot(direction, self.matrix[k]));
        let [x, y, z] = local.map(f32::abs);
        let k = if x < y || x < z {
            if z <= y { 1 } else { 2 }
        } else {
            0
        };
        let (i, j) = match k {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        };
        let side = if local[k] <= 0. { -1. } else { 1. };
        let vertex = |si: f32, sj: f32| {
            let mut sign = [0.; 3];
            sign[k] = side;
            sign[i] = si;
            sign[j] = sj;
            self.corner(sign)
        };
        let normal = self.matrix[k].map(|v| side * v);
        let vertices = [
            vertex(1., 1.),
            vertex(1., -1.),
            vertex(-1., -1.),
            vertex(-1., 1.),
        ];
        (
            [normal[0], normal[1], normal[2], 0.],
            vertices,
            f32::from_bits(0x3f7d_70a4) < local[k].abs(),
        )
    }

    fn valid(&self) -> bool {
        self.matrix
            .iter()
            .flatten()
            .all(|v| v.is_finite() && v.abs() <= 1e5)
            && self.matrix[3][3] == 1.
            && self.matrix[..3].iter().all(|r| r[3] == 0.)
            && self
                .half
                .iter()
                .all(|h| h.is_finite() && *h > 0. && *h < 1e3)
            && self.margin.is_finite()
            && (0. ..=1.).contains(&self.margin)
    }
}

/// Contacts of `mesh` at `matrix` (margin `query.margin`, sampled directions
/// in the mesh frame) against `shape`, at most `capacity` (<= 16). Empty when
/// the shapes are farther apart than their margins. A pair met for the first
/// time: see [`contacts_cached`].
pub fn contacts<'m>(
    mesh: impl Into<Hull<'m>>,
    matrix: Matrix,
    shape: &BoxShape,
    query: &ContactSettings,
    capacity: usize,
) -> Result<Vec<Contact>, Error> {
    contacts_cached(mesh, matrix, shape, query, capacity, &mut [0.; 4])
}

/// [`contacts`] with the pair's cached direction (world space): B39500 hands
/// AE8AD0 the broadphase pair's +0x10, and B7F740 starts there (zero: +x),
/// rotated into the mesh's frame, and stores the final distance direction
/// back in world space, hit or miss: a body sliding along a wall starts each
/// tick from the last tick's direction. Unchanged when the query fails.
pub fn contacts_cached<'m>(
    mesh: impl Into<Hull<'m>>,
    matrix: Matrix,
    shape: &BoxShape,
    query: &ContactSettings,
    capacity: usize,
    direction: &mut V,
) -> Result<Vec<Contact>, Error> {
    let mesh = mesh.into();
    if capacity > 16 || !shape.valid() {
        return Err(Error("invalid box contact controls"));
    }
    if capacity == 0 {
        return Ok(Vec::new());
    }
    let mut context = Context::new();
    let Touch::Deep(normal, witnesses, depth) =
        witness(&mut context, mesh, matrix, shape, query, direction)?
    else {
        return Ok(Vec::new());
    };
    let s = Settings {
        margin: query.margin,
        shape_margin: shape.margin,
        normal,
        fallback_point: witnesses[1],
        fallback_depth: depth,
        capacity,
    };
    let (box_normal, box_vertices, oriented) = shape.face(normal);
    let box_face = Face {
        normal: box_normal,
        vertices: &box_vertices,
        oriented,
        margin: shape.margin,
    };
    let Hull::Mesh(mesh) = mesh else {
        // A round piece: its B47590 feature (or a sphere's fallback).
        return crate::round_contacts::moving_round_polygon(mesh, matrix, box_face, &s, &[]);
    };
    let (mesh_feature, _) = contact_manifold::mesh_feature(&mut context, mesh, matrix, &s)?;
    contact_manifold::assemble_faces([mesh_feature.face(), box_face], &s, &[])
}

/// The B7F740 half of AE8AD0 (see [`witness`]).
pub(crate) enum Touch {
    /// No witnesses: farther apart than the margins.
    Apart,
    /// Witnesses at most 1e-3 apart: AE8AD0 makes no contact and AE34B0
    /// neither a record nor a ray cast.
    Within,
    /// The oriented normal that pushes the mesh, the witnesses, their distance.
    Deep(V, [V; 2], f32),
}

/// The B7F740 half of AE8AD0, shared with the swept test AE34B0
/// (`continuous::sweep_fixed`): the oriented normal that pushes the mesh,
/// the witnesses and their distance, unless the shapes are apart or the
/// distance is at most 1e-3. Updates the pair's cached `direction`.
pub(crate) fn witness(
    context: &mut Context,
    mesh: Hull<'_>,
    matrix: Matrix,
    shape: &BoxShape,
    query: &ContactSettings,
    direction: &mut V,
) -> Result<Touch, Error> {
    // B7F740: the box in the mesh's frame; the directions already are.
    let corners = shape.corners().map(|c| to_frame(&matrix, c));
    let axes = [0, 1, 2].map(|k| to_frame(&matrix, shape.matrix[k]));
    let local = context.contact_points(
        mesh,
        IDENTITY,
        &corners,
        shape.margin,
        Extra::Axes(axes),
        to_frame(&matrix, cold_start(*direction)),
        query,
    )?;
    *direction = from_frame(&matrix, local.direction);
    let Some(witnesses) = local.witnesses else {
        return Ok(Touch::Apart);
    };
    let witnesses = witnesses.map(|w| from_frame(&matrix, w));
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
        return Ok(Touch::Within);
    }
    let inverse = 1. / depth;
    let normal = difference.map(|x| inverse * x);
    let (normal, witnesses) =
        contact_manifold::orient(shape.matrix[3], matrix[3], normal, witnesses)?;
    Ok(Touch::Deep(normal, witnesses, depth))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision_mesh::Mesh;

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
        // An icosahedron-like spread is not needed: the six axes plus the
        // box axes B7ED10 adds already contain every face normal.
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

    /// A wide, low box (a tyre wall lying on the road) and a mesh pushed 0.6 m
    /// into its side but only 0.1 m below its top: every contact pushes the
    /// mesh up out of the top, none down through the bottom face, as the
    /// per-triangle path did for Nuerburgring tyre wall 1508 (seed 1087).
    #[test]
    fn a_mesh_deep_in_a_box_side_is_never_pushed_down() {
        let mesh = cube(0.5);
        let wall = BoxShape {
            matrix: at(0., 0.5, 0.),
            half: [2., 0.5, 2.],
            margin: 0.02,
        };
        let hits = contacts(&mesh, at(-2.5 + 0.6 + 0.5, 1.4, 0.), &wall, &settings(), 16).unwrap();
        assert!(!hits.is_empty());
        for c in &hits {
            assert!(c.normal[1] > 0.99, "{c:?}");
        }
    }

    #[test]
    fn separated_shapes_give_nothing_and_margins_touch() {
        let mesh = cube(0.5);
        let block = BoxShape {
            matrix: at(0., 0., 0.),
            half: [0.5, 0.5, 0.5],
            margin: 0.02,
        };
        assert!(
            contacts(&mesh, at(0., 1.1, 0.), &block, &settings(), 16)
                .unwrap()
                .is_empty()
        );
        // 0.03 apart, inside the summed 0.04 margins: face contacts upward.
        let hits = contacts(&mesh, at(0., 1.03, 0.), &block, &settings(), 16).unwrap();
        assert!(!hits.is_empty());
        for c in &hits {
            assert!((c.normal[1] - 1.).abs() < 1e-4, "{c:?}");
            assert!(c.penetration > 0. && c.penetration < 0.02, "{c:?}");
        }
        assert_eq!(
            contacts(&mesh, at(0., 1.03, 0.), &block, &settings(), 1)
                .unwrap()
                .len(),
            1
        );
        assert!(contacts(&mesh, at(0., 1.03, 0.), &block, &settings(), 17).is_err());
    }

    /// Le Mans route 0, 8,689 m (BARRIER_COLLISION §5.3): a car sliding along
    /// an 80 m, 8.5 cm armco plate 2.5 km from the origin, its core a few
    /// millimetres from the plate. The f32 distance loop can stop there with
    /// a direction tens of degrees off the plate's normal, along which the
    /// shapes overlap by metres; the far face and a 4 m deep manifold
    /// followed, in world coordinates or the mesh's frame, cold or cached.
    /// With such results sent to the sampled penetration path (and that
    /// path's own runs checked the same way; the two negative gaps overlap
    /// the cores) every contact pushes the mesh straight out of the near
    /// face, and stays shallow.
    #[test]
    fn a_mesh_beside_a_long_thin_plate_far_from_the_origin_is_pushed_out_of_its_near_face() {
        let mesh = cube(1.);
        let u = [0.864_399, 0., 0.502_806, 0.];
        let n = [-0.502_806, 0., 0.864_399, 0.];
        let centre = [712.8, 14.2, 2498.1, 1.];
        let plate = BoxShape {
            matrix: [u, [0., 1., 0., 0.], n, centre],
            half: [39.98, 0.459, 0.0428],
            margin: 0.01,
        };
        let yaw = u[2].atan2(u[0]);
        let mut checked = 0;
        let mut worst = (1f32, 0f32);
        let mut failed = Vec::new();
        for k in 0..41 {
            let angle = yaw + (k as f32 - 20.) * 0.015;
            let (sin, cos) = angle.sin_cos();
            let x = [cos, 0., -sin, 0.];
            let z = [sin, 0., cos, 0.];
            let extent = dot(x, n).abs() + dot(z, n).abs();
            for along in [-30., -17., -3., 9., 26.] {
                for gap in [-0.015, -0.004, 0.0003, 0.0009, 0.0018, 0.004, 0.008] {
                    let offset = plate.half[2] + extent + gap;
                    let origin: V = std::array::from_fn(|j| {
                        if j == 3 { 1. } else { centre[j] + along * u[j] + offset * n[j] + if j == 1 { 0.3 } else { 0. } }
                    });
                    let matrix = [x, [0., 1., 0., 0.], z, origin];
                    // The pair's direction from the ticks before, approaching
                    // from 5 cm in 1 cm steps (the car's lateral speed is far
                    // lower; the plate is met at least this gradually).
                    let mut direction = [0.; 4];
                    for step in (1..=5).rev() {
                        let mut earlier = matrix;
                        for j in 0..3 {
                            earlier[3][j] += n[j] * 0.01 * step as f32;
                        }
                        contacts_cached(&mesh, earlier, &plate, &settings(), 16, &mut direction).unwrap();
                    }
                    let cached = contacts_cached(&mesh, matrix, &plate, &settings(), 16, &mut direction).unwrap();
                    let cold = contacts(&mesh, matrix, &plate, &settings(), 16).unwrap();
                    for c in cached.iter().chain(&cold) {
                        worst.0 = worst.0.min(dot(c.normal, n));
                        worst.1 = worst.1.max(c.penetration);
                        if dot(c.normal, n) < 0.99 || c.penetration >= 0.05 {
                            failed.push(format!("angle {angle} along {along} gap {gap}: {c:?}"));
                        }
                        checked += 1;
                    }
                }
            }
        }
        eprintln!("plate contacts {checked}, least alignment {}, deepest {}, failed {}", worst.0, worst.1, failed.len());
        assert!(failed.is_empty(), "{}", failed[..failed.len().min(5)].join("
"));
        assert!(checked > 1000, "{checked}");
    }

    #[test]
    fn box_faces_follow_the_dominant_local_axis() {
        let b = BoxShape {
            matrix: at(1., 2., 3.),
            half: [1., 2., 3.],
            margin: 0.,
        };
        let (n, v, oriented) = b.face([0., -1., 0., 0.]);
        assert_eq!(n, [0., -1., 0., 0.]);
        assert!(oriented);
        assert!(v.iter().all(|p| p[1] == 0.));
        let (n, _, oriented) = b.face([0.7, 0.7, 0., 0.]);
        assert_eq!(n, [1., 0., 0., 0.]);
        assert!(!oriented);
    }
}
