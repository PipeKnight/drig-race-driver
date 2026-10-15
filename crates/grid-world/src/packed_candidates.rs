//! Selected untransformed packed body-contact candidate collection.
//! Cold collection; not yet wired into the driving app.
use crate::{Error, packed_query::Mesh, spawn::reference_seed};
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
fn overlaps(center: V, half: V, triangle: [V; 3]) -> Result<bool, Error> {
    let [a, b, c] = triangle.map(|p| sub(p, center));
    for j in [1, 0, 2] {
        let low = a[j].min(b[j]).min(c[j]);
        let high = a[j].max(b[j]).max(c[j]);
        if low > half[j] || high < -half[j] {
            return Ok(false);
        }
    }
    let n = cross(sub(a, b), sub(a, c));
    let square = dot(n, n);
    if square == 0. {
        return Ok(false);
    }
    let seed = reference_seed(square, false)?;
    let factor = (1. - (seed * square) * seed) * (seed * 0.5) + seed;
    let n = n.map(|x| factor * x);
    let plane = dot(n, a);
    let radius = (n[2].abs() * half[2] + half[1] * n[1].abs()) + n[0].abs() * half[0];
    if plane > radius || plane < -radius {
        return Ok(false);
    }
    for (i, e) in [sub(b, a), sub(c, b), sub(a, c)].into_iter().enumerate() {
        let pairs = match i {
            0 => [[a, c], [a, c], [b, c]],
            1 => [[a, c], [a, c], [a, b]],
            _ => [[a, b], [a, b], [b, c]],
        };
        let radii = [
            e[1].abs() * half[2] + half[1] * e[2].abs(),
            half[0] * e[2].abs() + half[2] * e[0].abs(),
            e[0].abs() * half[1] + half[0] * e[1].abs(),
        ];
        for (axis, points) in pairs.into_iter().enumerate() {
            let p = points.map(|v| match axis {
                0 => e[2] * v[1] - e[1] * v[2],
                1 => (-e[2]) * v[0] + e[0] * v[2],
                _ => v[0] * e[1] - v[1] * e[0],
            });
            if p[0].min(p[1]) > radii[axis] || p[0].max(p[1]) < -radii[axis] {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

pub struct Output {
    pub accepted: Vec<usize>,
    pub visited: Vec<usize>,
    pub stamps: [u16; 4096],
}
/// Cold generation-one candidate order and bounded visited/accepted stores.
pub fn collect(mesh: &Mesh<'_>, bounds: [V; 2], capacity: usize) -> Result<Output, Error> {
    Ok(collect_set(mesh, bounds, capacity, None, false)?.output())
}

/// Input checks on the query bounds.
pub(crate) fn bounds_valid(bounds: [V; 2]) -> bool {
    !(bounds
        .iter()
        .flatten()
        .any(|x| !x.is_finite() || x.abs() > 1e6)
        || (0..3).any(|j| bounds[0][j] > bounds[1][j])
        || bounds.iter().any(|p| p[3] != 1.))
}

/// Vertex bounds of every triangle reachable from the root when a traversal
/// without rectangle culling meets no structural error, else `None`. A
/// culled traversal visits a subset of those nodes, triangles and vertices
/// at the same depths, so it meets no structural error either.
pub(crate) fn reach(mesh: &Mesh<'_>) -> Option<[f32; 6]> {
    let mut reach = [f32::MAX, f32::MAX, f32::MAX, f32::MIN, f32::MIN, f32::MIN];
    let mut stack = vec![(0usize, 0usize)];
    let mut visits = 0;
    while let Some((index, depth)) = stack.pop() {
        visits += 1;
        if depth > 64 || visits > 1_000_000 {
            return None;
        }
        let node = *mesh.nodes.get(index)?;
        if node == 0xffff {
            continue;
        }
        if node & 0x8000 != 0 {
            for &id in mesh.leaves.get(&((node & 0x7fff) as usize))? {
                for vertex in *mesh.triangles.get(id as usize)? {
                    let p = mesh.vertices.get(vertex as usize)?;
                    for j in 0..3 {
                        reach[j] = reach[j].min(p[j]);
                        reach[j + 3] = reach[j + 3].max(p[j]);
                    }
                }
            }
        } else {
            let child = node as usize;
            if child + 4 > mesh.nodes.len() {
                return None;
            }
            stack.extend((0..4).map(|i| (child + i, depth + 1)));
        }
    }
    Some(reach)
}

/// Input checks that depend only on the packed mesh (tree, counts, root
/// bounds and every vertex). Immutable track meshes evaluate them once.
pub fn mesh_valid(mesh: &Mesh<'_>) -> bool {
    !(mesh.nodes.is_empty()
        || mesh.triangles.len() > 32768
        || mesh.vertices.len() > 65535
        || mesh
            .bounds
            .iter()
            .chain(mesh.vertices.iter().flatten())
            .any(|x| !x.is_finite() || x.abs() > 1e6)
        || (0..3).any(|j| mesh.bounds[j] >= mesh.bounds[j + 3]))
}

/// The same ordered collector on an independently decoded tree with up to
/// 65535 vertices. Wider source node encodings must be admitted/transcoded by
/// the source adapter before calling this selected representation.
pub fn collect_decoded(mesh: &Mesh<'_>, bounds: [V; 2], capacity: usize) -> Result<Output, Error> {
    Ok(collect_set(mesh, bounds, capacity, None, true)?.output())
}

/// Collector result with the stamps of identities below 4096 as a bit set
/// (the published stamp table holds 1 exactly for those identities).
pub(crate) struct Collected {
    pub accepted: Vec<usize>,
    pub visited: Vec<usize>,
    seen: [u64; 64],
}

impl Collected {
    pub(crate) fn output(self) -> Output {
        let mut stamps = [0; 4096];
        for (id, stamp) in stamps.iter_mut().enumerate() {
            *stamp = u16::from(self.seen[id / 64] >> (id % 64) & 1 != 0);
        }
        Output {
            accepted: self.accepted,
            visited: self.visited,
            stamps,
        }
    }
}

/// [`collect`] (`decoded` false) or [`collect_decoded`], with [`mesh_valid`]
/// already evaluated on this unchanged mesh (`None` evaluates it here).
pub(crate) fn collect_set(
    mesh: &Mesh<'_>,
    bounds: [V; 2],
    capacity: usize,
    mesh_checked: Option<bool>,
    decoded: bool,
) -> Result<Collected, Error> {
    if !decoded && mesh.vertices.len() > 1024 {
        return Err(Error("packed candidate vertex count exceeds bound"));
    }
    // All checks share one error, so the order of evaluation is free.
    if capacity > 32768
        || !bounds_valid(bounds)
        || !mesh_checked.unwrap_or_else(|| mesh_valid(mesh))
    {
        return Err(Error("invalid packed candidate controls"));
    }
    let half: V = std::array::from_fn(|j| 0.5 * (bounds[1][j] - bounds[0][j]));
    let center: V = std::array::from_fn(|j| bounds[0][j] + half[j]);
    let mut output = Collected {
        accepted: Vec::new(),
        visited: Vec::new(),
        seen: [0; 64],
    };
    let mut stack = vec![(
        0usize,
        [
            mesh.bounds[0],
            mesh.bounds[2],
            mesh.bounds[3],
            mesh.bounds[5],
        ],
        0usize,
    )];
    let mut visits = 0;
    while let Some((index, r, depth)) = stack.pop() {
        visits += 1;
        if depth > 64 || visits > 1_000_000 {
            return Err(Error("packed candidate traversal bound exceeded"));
        }
        if bounds[0][0] > r[2] || r[0] > bounds[1][0] || bounds[0][2] > r[3] || r[1] > bounds[1][2]
        {
            continue;
        }
        let node = *mesh
            .nodes
            .get(index)
            .ok_or(Error("packed candidate node absent"))?;
        if node == 0xffff {
            continue;
        }
        if node & 0x8000 != 0 {
            let order = mesh
                .leaves
                .get(&((node & 0x7fff) as usize))
                .ok_or(Error("packed candidate leaf absent"))?;
            for &id in order {
                let id = id as usize;
                if id < 4096 {
                    let (word, bit) = (id / 64, 1u64 << (id % 64));
                    if output.seen[word] & bit != 0 {
                        continue;
                    }
                    output.seen[word] |= bit;
                } else if output.visited.contains(&id) {
                    continue;
                }
                if output.visited.len() < capacity {
                    output.visited.push(id);
                }
                let indices = *mesh
                    .triangles
                    .get(id)
                    .ok_or(Error("packed candidate triangle absent"))?;
                let mut triangle = [[0.; 4]; 3];
                for (j, id) in indices.into_iter().enumerate() {
                    let p = *mesh
                        .vertices
                        .get(id as usize)
                        .ok_or(Error("packed candidate vertex absent"))?;
                    triangle[j] = [p[0], p[1], p[2], 1.];
                }
                if overlaps(center, half, triangle)? && output.accepted.len() < capacity {
                    output.accepted.push(id);
                }
            }
        } else {
            let child = node as usize;
            if child + 4 > mesh.nodes.len() {
                return Err(Error("packed candidate children absent"));
            }
            let x = (r[2] - r[0]) * 0.5;
            let z = (r[3] - r[1]) * 0.5;
            let quadrants = [
                [r[0], r[1] + z, r[2] - x, r[3]],
                [r[0] + x, r[1] + z, r[2], r[3]],
                [r[0], r[1], r[2] - x, r[3] - z],
                [r[0] + x, r[1], r[2], r[3] - z],
            ];
            for i in (0..4).rev() {
                stack.push((child + i, quadrants[i], depth + 1));
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn triangle_box_touching_and_separating_axes() {
        let center = [0.; 4];
        let half = [1., 1., 1., 0.];
        let face = [[-1., 1., -1., 1.], [1., 1., -1., 1.], [0., 1., 1., 1.]];
        assert!(overlaps(center, half, face).unwrap());
        let outside = face.map(|mut p| {
            p[1] = f32::from_bits(1f32.to_bits() + 1);
            p
        });
        assert!(!overlaps(center, half, outside).unwrap());
        // All coordinate ranges intersect, but the plane separates this triangle.
        let diagonal = [[3., 0., 1., 1.], [1., 3., 0., 1.], [0., 1., 3., 1.]];
        assert!(!overlaps(center, half, diagonal).unwrap());
        assert!(!overlaps(center, half, [[0., 0., 0., 1.]; 3]).unwrap());
    }

    #[test]
    fn visited_rejected_triangles_and_full_store_do_not_end_traversal() {
        let vertices = [
            [-0.5, 0., -0.5],
            [0.5, 0., -0.5],
            [0., 0., 0.5],
            [-0.5, 5., -0.5],
            [0.5, 5., -0.5],
            [0., 5., 0.5],
        ];
        let triangles = [[0, 1, 2], [3, 4, 5]];
        let leaves = BTreeMap::from([(0, vec![1, 0, 1]), (1, vec![0, 1])]);
        let nodes = [1, 0x8000, 0x8001, 0xffff, 0xffff];
        let mesh = Mesh {
            bounds: [-2., -10., -2., 2., 10., 2.],
            vertices: &vertices,
            triangles: &triangles,
            nodes: &nodes,
            leaves: &leaves,
        };
        let bounds = [[-1., -1., -1., 1.], [1., 1., 1., 1.]];
        let result = collect(&mesh, bounds, 1).unwrap();
        assert_eq!(result.visited, [1]);
        assert_eq!(result.accepted, [0]);
        assert_eq!(&result.stamps[..2], &[1, 1]);
        let zero = collect(&mesh, bounds, 0).unwrap();
        assert!(zero.accepted.is_empty() && zero.visited.is_empty());
        assert_eq!(&zero.stamps[..2], &[1, 1]);
        let mut cycle = mesh;
        cycle.nodes = &[0, 0xffff, 0xffff, 0xffff];
        assert!(collect(&cycle, [[-2., -1., -2., 1.], [2., 1., 2., 1.]], 1).is_err());
    }

    #[test]
    fn large_triangle_ids_use_only_the_bounded_visited_store() {
        let vertices = [[-0.5, 0., -0.5], [0.5, 0., -0.5], [0., 0., 0.5]];
        let triangles = vec![[0, 1, 2]; 4098];
        let leaves = BTreeMap::from([(0, vec![4096, 4097, 4097, 4096])]);
        let mesh = Mesh {
            bounds: [-2., -2., -2., 2., 2., 2.],
            vertices: &vertices,
            triangles: &triangles,
            nodes: &[0x8000],
            leaves: &leaves,
        };
        let bounds = [[-1., -1., -1., 1.], [1., 1., 1., 1.]];
        let result = collect(&mesh, bounds, 1).unwrap();
        assert_eq!(result.visited, [4096]);
        assert_eq!(result.accepted, [4096]);
        assert_eq!(result.stamps, [0; 4096]);
        let result = collect(&mesh, bounds, 3).unwrap();
        assert_eq!(result.visited, [4096, 4097]);
        assert_eq!(result.accepted, [4096, 4097]);
    }
}
