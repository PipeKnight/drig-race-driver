//! Untransformed packed shape outputs; world dispatch and population are separate.
use crate::{Error, packed_query::Mesh, rectangle_cull::Cache, spawn::reference_seed};

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    /// Native identity retained from the verified packed traversal.
    pub triangle: usize,
    pub fraction: f32,
    pub normal: [f32; 4],
    pub material: [u8; 4],
}

/// Query a supplied mesh and return the selected triangle's material and normal.
/// Normal follows source winding and the original SSE reciprocal-root refinement.
pub fn query(
    mesh: &Mesh<'_>,
    triangle_materials: &[[u8; 4]],
    start: [f32; 4],
    end: [f32; 4],
    cache: &mut Cache,
) -> Result<Option<Hit>, Error> {
    if triangle_materials.len() != mesh.triangles.len() {
        return Err(Error("packed-shape material count mismatch"));
    }
    let Some(hit) = mesh.query(start, end, cache)? else {
        return Ok(None);
    };
    let [a, b, c] = mesh.triangles[hit.triangle].map(|i| mesh.vertices[i as usize]);
    let ab: [f32; 3] = std::array::from_fn(|i| b[i] - a[i]);
    let ac: [f32; 3] = std::array::from_fn(|i| c[i] - a[i]);
    let cross = [
        ab[1] * ac[2] - ab[2] * ac[1],
        ac[0] * ab[2] - ac[2] * ab[0],
        ac[1] * ab[0] - ac[0] * ab[1],
        0.,
    ];
    let squared =
        (cross[1] * cross[1] + cross[0] * cross[0]) + (cross[3] * cross[3] + cross[2] * cross[2]);
    if !squared.is_finite() || squared <= 0. {
        return Err(Error("packed-shape normal exceeds supported range"));
    }
    let seed = reference_seed(squared, false)?;
    let inverse = (1. - (squared * seed) * seed) * (seed * 0.5) + seed;
    let normal = cross.map(|v| inverse * v);
    if normal.into_iter().any(|v| !v.is_finite()) {
        return Err(Error("packed-shape normal is nonfinite"));
    }
    Ok(Some(Hit {
        triangle: hit.triangle,
        fraction: hit.fraction,
        normal,
        material: triangle_materials[hit.triangle],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    #[test]
    fn winding_and_later_equal_hit_material_are_preserved() {
        let vertices = [[0., 0., 0.], [1., 0., 0.], [0., 0., 1.]];
        let triangles = [[0, 1, 2], [0, 1, 2]];
        let leaves = BTreeMap::from([(0, vec![0, 1])]);
        let mesh = Mesh {
            bounds: [0., -1., 0., 1., 1., 1.],
            vertices: &vertices,
            triangles: &triangles,
            nodes: &[0x8000],
            leaves: &leaves,
        };
        let hit = query(
            &mesh,
            &[*b"ONE1", *b"TWO2"],
            [0.25, 1., 0.25, 1.],
            [0.25, -1., 0.25, 1.],
            &mut Cache::default(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(hit.material, *b"TWO2");
        assert_eq!(hit.fraction, 0.5);
        assert!(hit.normal[1] < -0.9999);
        assert_eq!(hit.normal[3].to_bits(), 0);
        assert!(query(&mesh, &[], [0.; 4], [0.; 4], &mut Cache::default()).is_err());
    }
}
