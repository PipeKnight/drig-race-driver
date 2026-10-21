//! Fresh cold body/packed contacts for the selected inactive correction bounds.
use crate::{
    Error, contact_manifold, convex_distance, packed_candidates,
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
    /// The moving mesh's margin and the sampled directions.
    pub query: convex_distance::ContactSettings,
    /// The packed shape's +0x220 margin, which AEA1F0 gives every candidate
    /// triangle (B212D0); road chunks have 0.02.
    pub shape_margin: f32,
    pub contact_capacity: usize,
    pub cache_enabled: bool,
    pub candidate_capacity: usize,
    /// Shape +0x290, a dword tested with `1 << material` (AEAFE5 [V]); the
    /// material field is four bits, so up to 16 keys.
    pub sheet_mask: u32,
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
pub fn calculate<'m>(
    mesh: impl Into<convex_distance::Hull<'m>>,
    matrix: Matrix,
    track: &packed_query::Mesh<'_>,
    materials: &[u8],
    settings: &Settings,
) -> Result<Output, Error> {
    calculate_owned(mesh.into(), matrix, track, materials, settings, false)
}

/// Checks of one packed mesh and its material indices that do not depend on
/// the moving body. An immutable track mesh evaluates them once ([`check`]).
#[derive(Clone, Copy, Debug, Default)]
pub struct Checked {
    materials: bool,
    mesh: bool,
    /// Within the narrow (undecoded) collector's vertex bound.
    narrow: bool,
    /// Vertex bounds of all reachable triangles of a structurally sound tree.
    reach: Option<[f32; 6]>,
}

pub fn check(track: &packed_query::Mesh<'_>, materials: &[u8]) -> Checked {
    let mesh = packed_candidates::mesh_valid(track);
    Checked {
        materials: materials_valid(track, materials),
        mesh,
        narrow: track.vertices.len() <= 1024,
        reach: if mesh {
            packed_candidates::reach(track)
        } else {
            None
        },
    }
}

/// Distance beyond which a body's bounds cannot reach a triangle through the
/// collector's centred overlap test despite f32 rounding (coordinates are
/// bounded by 1e6, where one ulp is 0.0625).
const REACH_MARGIN: f32 = 4.;

fn materials_valid(track: &packed_query::Mesh<'_>, materials: &[u8]) -> bool {
    materials.len() == track.triangles.len() && materials.iter().all(|&x| x < 16)
}

/// Sampling directions accepted by the pair query.
pub fn directions_valid(query: &convex_distance::ContactSettings) -> bool {
    !query.directions.iter().any(|d| {
        d[3] != 0.
            || d.iter().any(|x| !x.is_finite() || x.abs() > 2.)
            || !(0.99..=1.01).contains(&dot(*d, *d))
    })
}

/// Values the caller already holds for repeated pair queries of one moving
/// body: [`check`] of this unchanged packed mesh/materials,
/// [`directions_valid`] of `settings.query`, and the successful
/// `mesh.world_bounds(matrix, settings.query.margin)`.
pub struct Known {
    pub checked: Checked,
    pub directions: bool,
    pub bounds: [V; 2],
}

impl Known {
    /// True when [`calculate_known`] (`decoded` false) or
    /// [`calculate_decoded_known`] provably returns no contacts and no
    /// error with these settings: every check passes and every reachable
    /// triangle lies more than [`REACH_MARGIN`] beyond the body bounds on
    /// some axis, so no candidate passes the overlap test.
    pub fn misses(&self, settings: &Settings, decoded: bool) -> bool {
        let b = self.bounds;
        let Some(reach) = self.checked.reach else {
            return false;
        };
        settings.contact_capacity <= 16
            && settings.candidate_capacity <= 32768
            && settings.correction_bounds == [f32::MAX; 4]
            && self.checked.materials
            && self.directions
            && self.checked.mesh
            && (decoded || self.checked.narrow)
            && packed_candidates::bounds_valid(b)
            && (0..3)
                .any(|j| reach[j + 3] < b[0][j] - REACH_MARGIN || reach[j] > b[1][j] + REACH_MARGIN)
    }
}

/// [`calculate`] reusing [`Known`] values: the same contacts, without the
/// carried direction and candidate stores.
pub fn calculate_known<'m>(
    mesh: impl Into<convex_distance::Hull<'m>>,
    matrix: Matrix,
    track: &packed_query::Mesh<'_>,
    materials: &[u8],
    settings: &Settings,
    known: &Known,
) -> Result<Vec<contact_manifold::Contact>, Error> {
    Ok(
        generate(mesh.into(), matrix, track, materials, settings, false, Some(known))?
            .map_or_else(Vec::new, |g| g.contacts),
    )
}

/// [`calculate_decoded`] reusing [`Known`] values: the same contacts.
pub fn calculate_decoded_known<'m>(
    mesh: impl Into<convex_distance::Hull<'m>>,
    matrix: Matrix,
    track: &packed_query::Mesh<'_>,
    materials: &[u8],
    settings: &Settings,
    known: &Known,
) -> Result<Vec<contact_manifold::Contact>, Error> {
    Ok(
        generate(mesh.into(), matrix, track, materials, settings, true, Some(known))?
            .map_or_else(Vec::new, |g| g.contacts),
    )
}

/// Selected source-decoded wider-vertex tree using the shared cold query and
/// manifold path. The adapter must retain child order and material sheet flags.
pub fn calculate_decoded<'m>(
    mesh: impl Into<convex_distance::Hull<'m>>,
    matrix: Matrix,
    track: &packed_query::Mesh<'_>,
    materials: &[u8],
    settings: &Settings,
) -> Result<Output, Error> {
    calculate_owned(mesh.into(), matrix, track, materials, settings, true)
}

fn calculate_owned(
    mesh: convex_distance::Hull<'_>,
    matrix: Matrix,
    track: &packed_query::Mesh<'_>,
    materials: &[u8],
    settings: &Settings,
    decoded: bool,
) -> Result<Output, Error> {
    Ok(
        match generate(mesh, matrix, track, materials, settings, decoded, None)? {
            Some(g) => Output {
                contacts: g.contacts,
                direction: g.direction,
                candidates: Some(g.candidates.output()),
            },
            None => Output {
                contacts: Vec::new(),
                direction: [0.; 4],
                candidates: None,
            },
        },
    )
}

struct Generated {
    contacts: Vec<contact_manifold::Contact>,
    direction: V,
    candidates: packed_candidates::Collected,
}

/// The pair query; `None` when the controls disable generation.
fn generate(
    mesh: convex_distance::Hull<'_>,
    matrix: Matrix,
    track: &packed_query::Mesh<'_>,
    materials: &[u8],
    settings: &Settings,
    decoded: bool,
    known: Option<&Known>,
) -> Result<Option<Generated>, Error> {
    if settings.contact_capacity > 16
        || settings.candidate_capacity > 32768
        || settings.correction_bounds != [f32::MAX; 4]
        || !(0. ..=1.).contains(&settings.shape_margin)
        || !known.map_or_else(
            || materials_valid(track, materials),
            |k| k.checked.materials,
        )
        || !known.map_or_else(|| directions_valid(&settings.query), |k| k.directions)
    {
        return Err(Error("unsupported packed contact controls"));
    }
    // B212D0 widens the body's bounds by the packed shape's margin.
    let bounds = match known {
        Some(k) => k.bounds,
        None => mesh.world_bounds(matrix, settings.query.margin + settings.shape_margin)?,
    };
    if settings.contact_capacity == 0 || !settings.cache_enabled {
        return Ok(None);
    }
    let candidates = packed_candidates::collect_set(
        track,
        bounds,
        settings.candidate_capacity,
        known.map(|k| k.checked.mesh),
        decoded,
    )?;
    let mut result = Generated {
        contacts: Vec::new(),
        direction: [0.; 4],
        candidates,
    };
    if result.candidates.accepted.is_empty() {
        return Ok(Some(result));
    }
    let mut context = convex_distance::Context::new();
    for &id in &result.candidates.accepted {
        let triangle = track.triangles[id].map(|index| {
            let p = track.vertices[index as usize];
            [p[0], p[1], p[2], 1.]
        });
        let query = context.contact_triangle(
            mesh,
            matrix,
            triangle,
            settings.shape_margin,
            result.direction,
            &settings.query,
        )?;
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
            let manifold_settings = contact_manifold::Settings {
                margin: settings.query.margin,
                shape_margin: settings.shape_margin,
                normal,
                fallback_point: witnesses[1],
                fallback_depth: depth,
                capacity: settings.contact_capacity - result.contacts.len(),
            };
            result.contacts = match mesh {
                convex_distance::Hull::Mesh(mesh) => {
                    contact_manifold::calculate_oriented_append(
                        &mut context,
                        mesh,
                        matrix,
                        triangle,
                        &manifold_settings,
                        &result.contacts,
                        witnesses[0],
                    )?
                    .manifold
                    .contacts
                }
                round => crate::round_contacts::round_triangle_append(
                    round,
                    matrix,
                    triangle,
                    &manifold_settings,
                    &result.contacts,
                    witnesses[0],
                )?,
            };
            if result.contacts.len() == settings.contact_capacity {
                break;
            }
        }
    }

    Ok(Some(result))
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
            shape_margin: 0.,
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

    #[test]
    fn known_queries_match_and_misses_are_empty() {
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
        // Two quadrant leaves of a 20 m square; the other quadrants are empty.
        let vertices = [
            [-10., 0., -10.],
            [-1., 0., -10.],
            [-10., 0., -1.],
            [1., 0., 1.],
            [10., 0., 1.],
            [1., 0., 10.],
        ];
        let leaves = BTreeMap::from([(0, vec![0]), (1, vec![1])]);
        let nodes = [1, 0xffff, 0x8001, 0x8000, 0xffff];
        let track = packed_query::Mesh {
            bounds: [-10., -5., -10., 10., 5., 10.],
            vertices: &vertices,
            triangles: &[[0, 1, 2], [3, 4, 5]],
            nodes: &nodes,
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
        let settings = Settings {
            query: convex_distance::ContactSettings {
                margin: 0.01,
                directions: std::array::from_fn(|i| axes[i % 6]),
            },
            shape_margin: 0.,
            contact_capacity: 16,
            cache_enabled: true,
            candidate_capacity: 16,
            sheet_mask: 0,
            correction_bounds: [f32::MAX; 4],
        };
        let checked = check(&track, &[0, 0]);
        assert_eq!(checked.reach, Some([-10., 0., -10., 10., 0., 10.]));
        let (mut misses, mut hits) = (0, 0);
        for x in -16..=16 {
            for y in [-8, 0, 1, 6] {
                for z in -16..=16 {
                    let mut matrix = [
                        [1., 0., 0., 0.],
                        [0., 1., 0., 0.],
                        [0., 0., 1., 0.],
                        [0., 0., 0., 1.],
                    ];
                    matrix[3][..3].copy_from_slice(&[x as f32, y as f32, z as f32]);
                    let full = calculate(shape.mesh(), matrix, &track, &[0, 0], &settings).unwrap();
                    let known = Known {
                        checked,
                        directions: directions_valid(&settings.query),
                        bounds: shape.mesh().world_bounds(matrix, 0.01).unwrap(),
                    };
                    let contacts =
                        calculate_known(shape.mesh(), matrix, &track, &[0, 0], &settings, &known)
                            .unwrap();
                    assert_eq!(contacts, full.contacts);
                    if known.misses(&settings, false) {
                        assert!(
                            contacts.is_empty() && full.candidates.unwrap().accepted.is_empty()
                        );
                        misses += 1;
                    } else if !contacts.is_empty() {
                        hits += 1;
                    }
                }
            }
        }
        assert!(misses > 0 && hits > 0);
        // A tree with a missing child is never skipped.
        let broken = packed_query::Mesh {
            nodes: &nodes[..3],
            ..track
        };
        assert!(check(&broken, &[0, 0]).reach.is_none());
    }
}
