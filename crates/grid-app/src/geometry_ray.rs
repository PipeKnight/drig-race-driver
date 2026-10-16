//! CPU identification of geometry under a diagnostic pixel; no renderer/input use.
use bevy::{mesh::VertexAttributeValues, prelude::*};

pub fn audit(car: &super::Car, camera: Transform, size: Vec2, pixel: Vec2) {
    let fov = PerspectiveProjection::default().fov;
    let ndc = Vec2::new(
        2. * (pixel.x + 0.5) / size.x - 1.,
        1. - 2. * (pixel.y + 0.5) / size.y,
    );
    let direction = (*camera.forward()
        + *camera.right() * ndc.x * (fov * 0.5).tan() * size.x / size.y
        + *camera.up() * ndc.y * (fov * 0.5).tan())
    .normalize();
    let origin = camera.translation;
    let mut hits = Vec::new();
    for part in &car.parts {
        let Some(VertexAttributeValues::Float32x3(positions)) =
            part.mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            continue;
        };
        let Some(indices) = part.mesh.indices() else {
            continue;
        };
        let indices: Vec<_> = indices.iter().collect();
        for (instance, &raw) in part.matrices.iter().enumerate() {
            // Match the current renderer's TRS decomposition, including any loss of shear.
            let world = Transform::from_matrix(raw).to_matrix();
            let points: Vec<_> = positions
                .iter()
                .map(|p| world.transform_point3(Vec3::from_array(*p)))
                .collect();
            for (triangle, ix) in indices.as_chunks::<3>().0.iter().enumerate() {
                let [a, b, c] = [points[ix[0]], points[ix[1]], points[ix[2]]];
                let e1 = b - a;
                let e2 = c - a;
                let h = direction.cross(e2);
                let det = e1.dot(h);
                if det.abs() < 1e-8 {
                    continue;
                }
                let s = origin - a;
                let u = s.dot(h) / det;
                if !(0. ..=1.).contains(&u) {
                    continue;
                }
                let q = s.cross(e1);
                let v = direction.dot(q) / det;
                if v < 0. || u + v > 1. {
                    continue;
                }
                let distance = e2.dot(q) / det;
                if distance > 0. {
                    hits.push((
                        distance,
                        &part.name,
                        &part.material,
                        instance,
                        triangle,
                        det > 0.,
                        [a, b, c],
                        (world - raw)
                            .to_cols_array()
                            .into_iter()
                            .map(f32::abs)
                            .fold(0f32, f32::max),
                    ));
                }
            }
        }
    }
    hits.sort_by(|a, b| a.0.total_cmp(&b.0));
    println!(
        "geometry_ray pixel={pixel:?} size={size:?} eye={origin:?} direction={direction:?} fov={fov} hits={} scope=selected-CPU-triangles-not-GPU-visibility",
        hits.len()
    );
    for (distance, name, material, instance, triangle, front, points, trs_error) in
        hits.iter().take(30)
    {
        println!(
            "geometry_hit distance={distance} draw={name:?} material={material:?} instance={instance} triangle={triangle} front={front} vertices={points:?} trs_max_error={trs_error}"
        );
    }
}
