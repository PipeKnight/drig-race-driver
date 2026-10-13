//! Ordered convex plane intersection used by deformation cell integration.
//! This module is not yet admitted into source vehicle startup.
use crate::{Error, spawn::reference_seed};
pub const VERTICES: usize = 128;
pub const TRIANGLES: usize = 256;
type V = [f32; 4];
pub struct Output {
    pub vertices: Vec<V>,
    pub triangles: Vec<[u32; 3]>,
    pub vertex_count: usize,
    pub triangle_count: usize,
    pub volume: f64,
}
fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
        0.,
    ]
}
fn dot(a: V, b: V) -> f32 {
    (a[1] * b[1] + a[0] * b[0]) + (a[3] * b[3] + a[2] * b[2])
}
fn difference(a: V, b: V) -> V {
    std::array::from_fn(|i| a[i] - b[i])
}
fn length(v: V) -> Result<f32, Error> {
    let square = dot(v, v);
    if square == 0. {
        return Ok(0.);
    }
    let seed = reference_seed(square, false)?;
    let root = seed * square;
    Ok((1. - root * seed) * root * 0.5 + root)
}
fn normalize(v: V) -> Result<V, Error> {
    let square = dot(v, v);
    if square <= 0. {
        return Err(Error("degenerate deformation face basis"));
    }
    let seed = reference_seed(square, false)?;
    let inverse = (1. - seed * square * seed) * seed * 0.5 + seed;
    Ok(v.map(|v| inverse * v))
}
/// Complete supplied output pools preserve writes beyond the final counts.
#[allow(clippy::assign_op_pattern)] // Keep observed operand order explicit.
pub fn calculate(
    planes: &[V],
    offsets: &[f32],
    vertices: &[V],
    triangles: &[[u32; 3]],
) -> Result<Output, Error> {
    if !(3..=12).contains(&planes.len())
        || offsets.len() != planes.len()
        || vertices.len() != VERTICES
        || triangles.len() != TRIANGLES
        || planes
            .iter()
            .any(|p| p.iter().any(|v| !v.is_finite() || v.abs() > 2.) || p[3] != 0.)
        || offsets.iter().any(|v| !v.is_finite() || v.abs() > 1000.)
    {
        return Err(Error("invalid deformation volume input"));
    }
    let mut result = Output {
        vertices: vertices.to_vec(),
        triangles: triangles.to_vec(),
        vertex_count: 0,
        triangle_count: 0,
        volume: 0.,
    };
    let mut owners = [[0usize; 3]; VERTICES];
    for i in 0..planes.len() - 2 {
        let origin = planes[i].map(|v| offsets[i] * v);
        for j in i + 1..planes.len() - 1 {
            let direction = cross(planes[i], planes[j]);
            if length(direction)? <= 0. {
                continue;
            }
            let perpendicular = cross(direction, planes[i]);
            let denominator = dot(perpendicular, planes[j]);
            let initial_dot = ((origin[3] * planes[j][3] + origin[2] * planes[j][2])
                + origin[1] * planes[j][1])
                + origin[0] * planes[j][0];
            let t = (offsets[j] - initial_dot) / denominator;
            let point: V = std::array::from_fn(|a| t * perpendicular[a] + origin[a]);
            for k in j + 1..planes.len() {
                let d = dot(direction, planes[k]);
                if d == 0. {
                    continue;
                }
                let t = (offsets[k] - dot(point, planes[k])) / d;
                let mut vertex: V = std::array::from_fn(|a| t * direction[a] + point[a]);
                if planes
                    .iter()
                    .enumerate()
                    .any(|(p, n)| p != i && p != j && p != k && dot(vertex, *n) > offsets[p])
                {
                    continue;
                }
                if !vertex.iter().all(|v| v.is_finite()) || result.vertex_count == VERTICES {
                    return Err(Error(
                        "deformation intersection capacity or numeric failure",
                    ));
                }
                vertex[3] = 1.;
                result.vertices[result.vertex_count] = vertex;
                owners[result.vertex_count] = [i, j, k];
                result.vertex_count += 1;
            }
        }
    }
    // Early welding copies positions only: plane membership remains per triple.
    for i in 0..result.vertex_count.saturating_sub(1) {
        for j in i + 1..result.vertex_count {
            if length(difference(result.vertices[j], result.vertices[i]))?
                < f32::from_bits(0x3d4ccccd)
            {
                result.vertices[j] = result.vertices[i];
            }
        }
    }
    for (plane, normal) in planes.iter().enumerate() {
        let mut face = Vec::new();
        let mut sum = [0., 0., 0., 1.];
        for (i, membership) in owners.iter().enumerate().take(result.vertex_count) {
            if membership.contains(&plane) {
                face.push(i);
                for (a, v) in sum.iter_mut().enumerate() {
                    *v = result.vertices[i][a] + *v;
                }
            }
        }
        if face.len() <= 2 {
            continue;
        }
        let reciprocal = 1. / face.len() as f32;
        let center = sum.map(|v| reciprocal * v);
        let up = if normal[1].abs() >= f32::from_bits(0x3f666666) {
            [0., 0., 1., 0.]
        } else {
            [0., 1., 0., 0.]
        };
        let basis0 = cross(up, *normal);
        let basis1 = normalize(cross(*normal, basis0))?;
        let basis0 = normalize(basis0)?;
        let mut angles: Vec<_> = face
            .iter()
            .map(|i| {
                let d = difference(result.vertices[*i], center);
                (
                    f64::from(dot(d, basis1)).atan2(f64::from(dot(d, basis0))) as f32,
                    *i,
                )
            })
            .collect();
        // Stable bubble order preserves original handling of equal angles.
        for _ in 0..angles.len() {
            for i in 0..angles.len() - 1 {
                if angles[i + 1].0 < angles[i].0 {
                    angles.swap(i, i + 1);
                }
            }
        }
        face = angles.into_iter().map(|v| v.1).collect();
        let mut unique = vec![face[0]];
        for i in 1..face.len() {
            if length(difference(
                result.vertices[face[i]],
                result.vertices[face[i - 1]],
            ))? >= f32::from_bits(0x3851b718)
            {
                unique.push(face[i]);
            }
        }
        for i in 0..unique.len().saturating_sub(2) {
            if result.triangle_count == TRIANGLES {
                return Err(Error("deformation face capacity exceeded"));
            }
            result.triangles[result.triangle_count] =
                [unique[0] as u32, unique[i + 1] as u32, unique[i + 2] as u32];
            result.triangle_count += 1;
        }
    }
    let mut i = 0;
    while i + 1 < result.vertex_count {
        let mut j = i + 1;
        while j < result.vertex_count {
            if length(difference(result.vertices[j], result.vertices[i]))?
                >= f32::from_bits(0x3d4ccccd)
            {
                j += 1;
                continue;
            }
            for triangle in result.triangles.iter_mut().take(result.triangle_count) {
                for v in triangle {
                    if *v == j as u32 {
                        *v = i as u32;
                    }
                    if *v == (result.vertex_count - 1) as u32 {
                        *v = j as u32;
                    }
                }
            }
            result.vertices[j] = result.vertices[result.vertex_count - 1];
            result.vertex_count -= 1;
        }
        i += 1;
    }
    let mut accumulated = 0f32;
    for t in result.triangles.iter().take(result.triangle_count) {
        let [a, b, c] = t.map(|i| result.vertices[i as usize]);
        let normal = cross(difference(b, a), difference(c, a));
        accumulated = (a[0] + b[0] + c[0]) * normal[0] + accumulated;
    }
    if accumulated > 0. {
        result.volume = f64::from(accumulated) * f64::from(f32::from_bits(0x3e2aaaab));
    }
    if !result.volume.is_finite() {
        return Err(Error("nonfinite deformation volume output"));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ordered_closed_cube_has_eight_vertices_and_positive_volume() {
        let mut planes = [[0.; 4]; 6];
        for a in 0..3 {
            planes[a * 2][a] = -1.;
            planes[a * 2 + 1][a] = 1.;
        }
        let vertices = vec![[f32::from_bits(0x7fc01234); 4]; VERTICES];
        let triangles = vec![[u32::MAX; 3]; TRIANGLES];
        let out = calculate(&planes, &[1.; 6], &vertices, &triangles).unwrap();
        assert_eq!((out.vertex_count, out.triangle_count), (8, 12));
        assert_eq!(out.volume, 48f64 * f64::from(f32::from_bits(0x3e2aaaab)));
        assert_eq!(out.triangles[12], [u32::MAX; 3]);
        assert_eq!(vertices[0][0].to_bits(), 0x7fc01234);
    }
}
