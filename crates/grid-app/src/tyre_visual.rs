//! Authored circular interpolation of tyre surfaces, exclusively for rendering.
//! Physics radii, wheel anchors, motion and deformation kernels are unchanged.
use bevy::{
    mesh::{Indices, VertexAttributeValues},
    prelude::*,
};
use std::collections::BTreeMap;

pub fn refine(mesh: &mut Mesh) -> Result<(usize, usize), &'static str> {
    let Some(VertexAttributeValues::Float32x3(source)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return Err("tyre positions absent");
    };
    let Some(VertexAttributeValues::Float32x3(normals)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
    else {
        return Err("tyre normals absent");
    };
    let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0) else {
        return Err("tyre UVs absent");
    };
    if source.is_empty()
        || source.len() > 4096
        || normals.len() != source.len()
        || uvs.len() != source.len()
    {
        return Err("tyre stream size mismatch");
    }
    let mut positions: Vec<Vec3> = source.iter().copied().map(Vec3::from_array).collect();
    let mut normals: Vec<Vec3> = normals.iter().copied().map(Vec3::from_array).collect();
    let mut uvs: Vec<Vec2> = uvs.iter().copied().map(Vec2::from_array).collect();
    let mut tangents = match mesh.attribute(Mesh::ATTRIBUTE_TANGENT) {
        None => None,
        Some(VertexAttributeValues::Float32x4(values))
            if values.len() == positions.len()
                && values.iter().flatten().all(|v| v.is_finite()) =>
        {
            Some(
                values
                    .iter()
                    .copied()
                    .map(Vec4::from_array)
                    .collect::<Vec<_>>(),
            )
        }
        _ => return Err("invalid tyre tangent stream"),
    };
    if positions.iter().chain(&normals).any(|v| !v.is_finite())
        || uvs.iter().any(|v| !v.is_finite())
    {
        return Err("nonfinite tyre streams");
    }
    let mut indices: Vec<u32> = match mesh.indices() {
        Some(i) => i.iter().map(|i| i as u32).collect(),
        None => return Err("tyre indices absent"),
    };
    if indices.is_empty()
        || indices.len() > 24576
        || !indices.len().is_multiple_of(3)
        || indices.iter().any(|&i| i as usize >= positions.len())
    {
        return Err("invalid tyre triangles");
    }
    let before = indices.len() / 3;
    let lo = positions
        .iter()
        .copied()
        .fold(Vec3::splat(f32::INFINITY), Vec3::min);
    let hi = positions
        .iter()
        .copied()
        .fold(Vec3::splat(f32::NEG_INFINITY), Vec3::max);
    let center = ((lo + hi) * 0.5).yz();
    // Only curved strips are supported. Reject caps across the axis and large
    // angular jumps instead of inventing a surface or collapsing an edge.
    for tri in indices.as_chunks::<3>().0 {
        for e in 0..3 {
            let a = positions[tri[e] as usize].yz() - center;
            let b = positions[tri[(e + 1) % 3] as usize].yz() - center;
            if let Some(tangents) = &tangents
                && tangents[tri[e] as usize].w * tangents[tri[(e + 1) % 3] as usize].w <= 0.
            {
                return Err("tyre triangle crosses tangent handedness seam");
            }
            if a.length_squared() < 1e-8 || b.length_squared() < 1e-8 || a.dot(b) <= 0.0 {
                return Err("unsupported tyre edge through axis or over 90 degrees");
            }
        }
    }
    for _ in 0..2 {
        let mut edges = BTreeMap::new();
        let mut next = Vec::with_capacity(indices.len() * 4);
        for tri in indices.as_chunks::<3>().0 {
            let mut mid = [0; 3];
            for e in 0..3 {
                let a = tri[e].min(tri[(e + 1) % 3]);
                let b = tri[e].max(tri[(e + 1) % 3]);
                mid[e] = *edges.entry((a, b)).or_insert_with(|| {
                    let pa = positions[a as usize];
                    let pb = positions[b as usize];
                    let ra = pa.yz() - center;
                    let rb = pb.yz() - center;
                    let direction = (ra + rb).normalize_or_zero();
                    let ring = center + direction * ((ra.length() + rb.length()) * 0.5);
                    let p = Vec3::new((pa.x + pb.x) * 0.5, ring.x, ring.y);
                    let n = (normals[a as usize] + normals[b as usize]).normalize_or_zero();
                    let uv = (uvs[a as usize] + uvs[b as usize]) * 0.5;
                    let tangent = tangents.as_ref().map(|ts| {
                        let t = ts[a as usize].truncate() + ts[b as usize].truncate();
                        (t - n * t.dot(n))
                            .normalize_or_zero()
                            .extend(ts[a as usize].w)
                    });
                    let i = positions.len() as u32;
                    positions.push(p);
                    normals.push(n);
                    uvs.push(uv);
                    if let (Some(ts), Some(t)) = (&mut tangents, tangent) {
                        ts.push(t)
                    }
                    i
                });
            }
            let [a, b, c] = [tri[0], tri[1], tri[2]];
            let [ab, bc, ca] = mid;
            next.extend_from_slice(&[a, ab, ca, ab, b, bc, ca, bc, c, ab, bc, ca]);
        }
        indices = next;
    }
    let after = indices.len() / 3;
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_POSITION,
        positions
            .into_iter()
            .map(|v| v.to_array())
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        normals
            .into_iter()
            .map(|v| v.to_array())
            .collect::<Vec<_>>(),
    );
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_UV_0,
        uvs.into_iter().map(|v| v.to_array()).collect::<Vec<_>>(),
    );
    mesh.insert_indices(Indices::U32(indices));
    if let Some(tangents) = tangents {
        mesh.insert_attribute(
            Mesh::ATTRIBUTE_TANGENT,
            tangents
                .into_iter()
                .map(|v| v.to_array())
                .collect::<Vec<_>>(),
        );
    }
    Ok((before, after))
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology};
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn source_tyres_keep_original_vertices_and_wheel_transforms() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        for lod in ["LOD0_", "LOD1_"] {
            let car = crate::load(std::path::Path::new(&root), lod, false, false).unwrap();
            let mut count = 0;
            for mut part in car.parts {
                if part.wheel.is_none()
                    || !matches!(part.material.trim(), "Tyre Wall" | "Tyre Tread")
                {
                    continue;
                }
                let transform = part.matrices.clone();
                let before = part
                    .mesh
                    .attribute(Mesh::ATTRIBUTE_POSITION)
                    .unwrap()
                    .clone();
                let (a, b) = refine(&mut part.mesh).unwrap();
                assert_eq!(b, a * 16);
                let VertexAttributeValues::Float32x3(original) = before else {
                    panic!()
                };
                let VertexAttributeValues::Float32x3(refined) =
                    part.mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
                else {
                    panic!()
                };
                assert_eq!(&refined[..original.len()], original.as_slice());
                assert_eq!(part.matrices, transform);
                assert!(refined.iter().flatten().all(|v| v.is_finite()));
                if part.material == "Tyre Tread" && lod == "LOD0_" {
                    let VertexAttributeValues::Float32x4(tangents) = part
                        .mesh
                        .attribute(Mesh::ATTRIBUTE_TANGENT)
                        .expect("validated source tread tangent")
                    else {
                        panic!()
                    };
                    let VertexAttributeValues::Float32x3(normals) =
                        part.mesh.attribute(Mesh::ATTRIBUTE_NORMAL).unwrap()
                    else {
                        panic!()
                    };
                    assert_eq!(tangents.len(), refined.len());
                    for (t, n) in tangents.iter().zip(normals) {
                        let t = Vec4::from_array(*t);
                        let n = Vec3::from_array(*n).normalize();
                        assert!(t.is_finite() && n.dot(t.truncate()).abs() < 1e-5);
                        assert!((t.truncate().length() - 1.).abs() < 1e-5);
                        assert!(t.w == 1. || t.w == -1.);
                    }
                } else if part.material == "Tyre Tread" {
                    assert!(
                        part.mesh.attribute(Mesh::ATTRIBUTE_TANGENT).is_none(),
                        "unvalidated LOD keeps normal-map fallback"
                    );
                }
                count += 1;
            }
            assert_eq!(count, 8);
        }
    }
    #[test]
    fn circular_refinement_preserves_dimensions_and_seams() {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );
        let mut p = Vec::new();
        let mut n = Vec::new();
        let mut uv = Vec::new();
        let mut ix = Vec::new();
        for side in 0..2 {
            for i in 0..=8 {
                let a = i as f32 * std::f32::consts::TAU / 8.;
                let (s, c) = a.sin_cos();
                p.push([side as f32 * 0.2, s * 0.3, c * 0.3]);
                n.push([0., s, c]);
                uv.push([i as f32 / 8., side as f32]);
            }
        }
        for i in 0..8 {
            ix.extend_from_slice(&[i, i + 1, i + 9, i + 1, i + 10, i + 9]);
        }
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, p);
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, n);
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uv);
        mesh.insert_indices(Indices::U32(ix));
        assert_eq!(refine(&mut mesh).unwrap(), (16, 256));
        let VertexAttributeValues::Float32x3(p) = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
        else {
            panic!()
        };
        let VertexAttributeValues::Float32x2(uv) = mesh.attribute(Mesh::ATTRIBUTE_UV_0).unwrap()
        else {
            panic!()
        };
        for p in p {
            assert!((p[1].hypot(p[2]) - 0.3).abs() < 1e-6);
            assert!((0. ..=0.2).contains(&p[0]));
        }
        assert_eq!(uv[0], [0., 0.]);
        assert_eq!(uv[8], [1., 0.]);
        let mut angles: Vec<_> = p
            .iter()
            .map(|p| p[1].atan2(p[2]).rem_euclid(std::f32::consts::TAU))
            .collect();
        angles.sort_by(f32::total_cmp);
        angles.dedup_by(|a, b| (*a - *b).abs() < 1e-5);
        assert!(
            angles
                .windows(2)
                .all(|p| p[1] - p[0] < std::f32::consts::TAU / 31.9)
        );
    }
}
