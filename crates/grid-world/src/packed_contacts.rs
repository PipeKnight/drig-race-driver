//! Fresh cold body/packed contacts for the selected inactive correction bounds.
use crate::{
    Error, collision_mesh::Mesh, contact_manifold, convex_distance, packed_candidates,
    packed_query, physical_pose::Matrix, spawn::reference_seed,
};
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
pub struct Settings {
    pub query: convex_distance::ContactSettings,
    pub contact_capacity: usize,
    pub cache_enabled: bool,
    pub candidate_capacity: usize,
    pub sheet_mask: u8,
    /// Selected constructor state only. Populated correction bounds are unsupported.
    pub correction_bounds: V,
}
pub struct Output {
    pub contacts: Vec<contact_manifold::Contact>,
    /// Carried distance-query direction, independent of the publication normal.
    pub direction: V,
    pub candidates: Option<packed_candidates::Output>,
}
/// One fresh pair query, preserving its support cache and manifold across triangles.
/// Source face normals must already have undergone moving-shape initialization.
pub fn calculate(
    mesh: &Mesh,
    matrix: Matrix,
    track: &packed_query::Mesh<'_>,
    materials: &[u8],
    settings: &Settings,
) -> Result<Output, Error> {
    if settings.contact_capacity > 16
        || settings.candidate_capacity > 32768
        || settings.correction_bounds != [f32::MAX; 4]
        || materials.len() != track.triangles.len()
        || materials.iter().any(|&x| x >= 8)
        || settings.query.directions.iter().any(|d| {
            d[3] != 0.
                || d.iter().any(|x| !x.is_finite() || x.abs() > 2.)
                || !(0.99..=1.01).contains(&dot(*d, *d))
        })
    {
        return Err(Error("unsupported packed contact controls"));
    }
    let bounds = mesh.world_bounds(matrix, settings.query.margin)?;
    let mut result = Output {
        contacts: Vec::new(),
        direction: [0.; 4],
        candidates: None,
    };
    if settings.contact_capacity == 0 || !settings.cache_enabled {
        return Ok(result);
    }
    let candidates = packed_candidates::collect(track, bounds, settings.candidate_capacity)?;
    let mut context = convex_distance::Context::new();
    for &id in &candidates.accepted {
        let triangle = track.triangles[id].map(|index| {
            let p = track.vertices[index as usize];
            [p[0], p[1], p[2], 1.]
        });
        let query = context.contact(mesh, matrix, triangle, result.direction, &settings.query)?;
        result.direction = query.direction;
        if let Some(witnesses) = query.witnesses {
            let difference = sub(witnesses[0], witnesses[1]);
            let square = dot(difference, difference);
            let depth = if square == 0. {
                0.
            } else {
                let seed = reference_seed(square, false)?;
                let product = seed * square;
                ((1. - product * seed) * product) * 0.5 + product
            };
            if depth <= f32::from_bits(0x3a83_126f) {
                continue;
            }
            let normal = if settings.sheet_mask & (1 << materials[id]) == 0 {
                let inverse = 1. / depth;
                difference.map(|x| inverse * x)
            } else {
                let n = cross(sub(triangle[0], triangle[1]), sub(triangle[0], triangle[2]));
                let square = dot(n, n);
                let seed = reference_seed(square, false)?;
                let inverse = ((1. - (seed * square) * seed) * seed) * 0.5 + seed;
                n.map(|x| inverse * x)
            };
            let manifold = contact_manifold::calculate_oriented_append(
                &mut context,
                mesh,
                matrix,
                triangle,
                &contact_manifold::Settings {
                    margin: settings.query.margin,
                    normal,
                    fallback_point: witnesses[1],
                    fallback_depth: depth,
                    capacity: settings.contact_capacity - result.contacts.len(),
                },
                &result.contacts,
                witnesses[0],
            )?;
            result.contacts = manifold.manifold.contacts;
            if result.contacts.len() == settings.contact_capacity {
                break;
            }
        }
    }
    result.candidates = Some(candidates);
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[test]
    fn fresh_contacts_capacity_separation_and_scope_guards() {
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
        let span = mesh.bytes().len() as u32;
        let shape = mesh
            .initialize_shape(
                crate::collision_mesh::ShapeSettings {
                    span,
                    scalar220: 0.01,
                    layer: 0,
                    flag_c0: false,
                },
                0,
            )
            .unwrap();
        let matrix = [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ];
        let vertices = [[-5., 0., -5.], [5., 0., -5.], [0., 0., 5.]];
        let leaves = BTreeMap::from([(0, vec![0])]);
        let track = packed_query::Mesh {
            bounds: [-5., -5., -5., 5., 5., 5.],
            vertices: &vertices,
            triangles: &[[0, 1, 2]],
            nodes: &[0x8000],
            leaves: &leaves,
        };
        let axes = [
            [1., 0., 0., 0.],
            [-1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., -1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., -1., 0.],
        ];
        let mut settings = Settings {
            query: convex_distance::ContactSettings {
                margin: 0.01,
                directions: std::array::from_fn(|i| axes[i % 6]),
            },
            contact_capacity: 16,
            cache_enabled: true,
            candidate_capacity: 16,
            sheet_mask: 0,
            correction_bounds: [f32::MAX; 4],
        };
        let full = calculate(shape.mesh(), matrix, &track, &[0], &settings).unwrap();
        assert!(!full.contacts.is_empty());
        assert_eq!(full.candidates.unwrap().accepted, [0]);
        settings.contact_capacity = 1;
        let limited = calculate(shape.mesh(), matrix, &track, &[0], &settings).unwrap();
        assert_eq!(limited.contacts.len(), 1);
        let mut far = matrix;
        far[3][1] = 10.;
        assert!(
            calculate(shape.mesh(), far, &track, &[0], &settings)
                .unwrap()
                .contacts
                .is_empty()
        );
        settings.contact_capacity = 0;
        assert!(
            calculate(shape.mesh(), matrix, &track, &[0], &settings)
                .unwrap()
                .candidates
                .is_none()
        );
        settings.query.directions[0][0] = f32::NAN;
        assert!(calculate(shape.mesh(), matrix, &track, &[0], &settings).is_err());
        settings.query.directions[0] = axes[0];
        settings.correction_bounds = [-5., 5., -5., 5.];
        assert!(calculate(shape.mesh(), matrix, &track, &[0], &settings).is_err());
    }
}
