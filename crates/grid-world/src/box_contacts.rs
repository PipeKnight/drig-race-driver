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
//! AE2FA0 treat symmetrically. The sampled directions are rotated from the
//! mesh frame, where the original evaluates them, into the world.
use crate::{
    Error,
    collision_mesh::Mesh,
    contact_manifold::{self, Contact, Face, Settings},
    convex_distance::{ContactSettings, Context, Extra},
    physical_pose::Matrix,
};

type V = [f32; 4];
fn dot(a: V, b: V) -> f32 {
    (a[0] * b[0] + a[1] * b[1]) + (a[2] * b[2] + a[3] * b[3])
}
fn sub(a: V, b: V) -> V {
    std::array::from_fn(|j| a[j] - b[j])
}

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
    fn corners(&self) -> [V; 8] {
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
/// the shapes are farther apart than their margins.
pub fn contacts(
    mesh: &Mesh,
    matrix: Matrix,
    shape: &BoxShape,
    query: &ContactSettings,
    capacity: usize,
) -> Result<Vec<Contact>, Error> {
    if capacity > 16 || !shape.valid() {
        return Err(Error("invalid box contact controls"));
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
    let corners = shape.corners();
    let axes = [shape.matrix[0], shape.matrix[1], shape.matrix[2]];
    let mut context = Context::new();
    let query = context.contact_points(
        mesh,
        matrix,
        &corners,
        shape.margin,
        Extra::Axes(axes),
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
    let (normal, witnesses) =
        contact_manifold::orient(shape.matrix[3], matrix[3], normal, witnesses)?;
    let s = Settings {
        margin: settings.margin,
        shape_margin: shape.margin,
        normal,
        fallback_point: witnesses[1],
        fallback_depth: depth,
        capacity,
    };
    let (mesh_feature, _) = contact_manifold::mesh_feature(&mut context, mesh, matrix, &s)?;
    let (box_normal, box_vertices, oriented) = shape.face(normal);
    let box_face = Face {
        normal: box_normal,
        vertices: &box_vertices,
        oriented,
        margin: shape.margin,
    };
    contact_manifold::assemble_faces([mesh_feature.face(), box_face], &s, &[])
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
