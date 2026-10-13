//! Independent CPU body-dent presentation; the original renderer is not emulated.
use bevy::{mesh::VertexAttributeValues, prelude::*};
use grid_tools::body_deformation::Field;
pub fn project(source: &Mesh, raw_bind: Mat4, field: &Field) -> Result<Mesh, &'static str> {
    if !raw_bind.is_finite() || raw_bind.determinant().abs() < 1e-8 {
        return Err("invalid body damage bind transform");
    }
    let inverse = raw_bind.inverse();
    if !inverse.is_finite() {
        return Err("invalid body damage inverse transform");
    }
    let Some(VertexAttributeValues::Float32x3(original)) =
        source.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        return Err("missing body damage source positions");
    };
    if original.is_empty()
        || original.len() > 1_000_000
        || original.iter().flatten().any(|v| !v.is_finite())
    {
        return Err("invalid body damage source positions");
    }
    let queries: Vec<_> = original
        .iter()
        .map(|p| {
            let p = raw_bind.transform_point3(Vec3::from_array(*p));
            [p.x, p.y, p.z, 1.]
        })
        .collect();
    let deltas = field.displacements(&queries).map_err(|e| e.0)?;
    if deltas.iter().all(|d| *d == [0.; 3]) {
        return Ok(source.clone());
    }
    let positions: Vec<_> = original
        .iter()
        .zip(deltas)
        .map(|(p, d)| {
            (Vec3::from_array(*p) + inverse.transform_vector3(Vec3::from_array(d))).to_array()
        })
        .collect();
    if positions
        .iter()
        .flatten()
        .any(|v| !v.is_finite() || v.abs() > 1e6)
    {
        return Err("body damage positions exceed bounds");
    }
    let mut mesh = source.clone();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.compute_smooth_normals();
    let Some(VertexAttributeValues::Float32x3(normals)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
    else {
        return Err("missing body damage normals");
    };
    if normals.iter().flatten().any(|v| !v.is_finite()) {
        return Err("nonfinite body damage normals");
    }
    Ok(mesh)
}
