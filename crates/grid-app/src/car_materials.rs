//! Authored material responses, not direct mappings of GRID shader constants.
use bevy::prelude::*;

/// Author an orthonormal Bevy frame from saved vectors. Original skew is not
/// representable by tangent+sign; this deliberately approximates that basis.
pub fn tread_tangents(
    source: &grid_formats::mesh::Mesh<'_>,
    range: std::ops::Range<usize>,
) -> Option<Vec<[f32; 4]>> {
    let mut output = Vec::with_capacity(range.len());
    for i in range {
        if *source.colors_argb.get(i)? != [255, 0, 255, 255] {
            return None;
        }
        let n = Vec3::from_array(*source.normals.get(i)?).try_normalize()?;
        let raw = Vec3::from_array(*source.tangents.get(i)?);
        let t = (raw - n * raw.dot(n)).try_normalize()?;
        let b = Vec3::from_array(*source.binormals.get(i)?).try_normalize()?;
        let alignment = n.cross(t).dot(b);
        if !alignment.is_finite() || alignment.abs() < 0.95 {
            return None;
        }
        output.push([t.x, t.y, t.z, if alignment < 0. { -1. } else { 1. }]);
    }
    Some(output)
}

/// The car programs' per-vertex controls, from the saved vertex colour
/// (docs/CAR_SHADER_MASKS.md): the paint vertex programs pass
/// (1 - red, 1 - green) on, where 1 - red cuts the reflection (damage /
/// normal-map strength) and 1 - green lets the scratch map in. None when the
/// range has no colours.
pub fn shader_controls(
    source: &grid_formats::mesh::Mesh<'_>,
    range: std::ops::Range<usize>,
) -> Option<Vec<[f32; 2]>> {
    let colours = source.colors_argb.get(range)?;
    (!colours.is_empty()).then(|| {
        colours
            .iter()
            .map(|&[_, r, g, _]| [1. - f32::from(r) / 255., 1. - f32::from(g) / 255.])
            .collect()
    })
}

pub fn apply(name: &str, material: &mut StandardMaterial) {
    match name {
        "Car Body Paint" => {
            material.perceptual_roughness = 0.3;
            material.metallic = 0.0;
            material.clearcoat = 0.2;
            material.clearcoat_perceptual_roughness = 0.18;
        }
        "Window Glass" | "Head Lights Glass" | "Rear Lights Glass" => {
            material.perceptual_roughness = 0.08;
            material.metallic = 0.0;
            material.ior = 1.5;
        }
        "Tyre Wall" | "Tyre Tread" => {
            material.perceptual_roughness = if name == "Tyre Tread" { 0.9 } else { 0.82 };
            material.reflectance = 0.3;
            material.metallic = 0.0;
        }
        "Wheel" => {
            // White painted alloy wheel: keep a dielectric coating rather than
            // treating all of its diffuse livery as bare reflective metal.
            material.perceptual_roughness = 0.32;
            material.metallic = 0.15;
            material.clearcoat = 0.1;
            material.clearcoat_perceptual_roughness = 0.2;
        }
        "Bare Metal" | "Shiny Metal" | "Discs" => {
            material.metallic = 0.95;
            material.perceptual_roughness = match name {
                "Shiny Metal" => 0.22,
                "Discs" => 0.48,
                _ => 0.35,
            };
        }
        "Cabin" | "Matt" | "Aerials" => {
            material.metallic = 0.0;
            material.perceptual_roughness = 0.85;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shader_controls_invert_red_and_green() {
        let mesh = grid_formats::mesh::Mesh {
            id: b"",
            positions: vec![[0.; 3]; 3],
            normals: vec![],
            tangents: vec![],
            binormals: vec![],
            colors_argb: vec![[255, 255, 255, 255], [255, 0, 255, 255], [254, 255, 0, 255]],
            nonfinite_normals: 0,
            nonfinite_texcoords: 0,
            texcoords: vec![],
            additional_texcoords: vec![],
            skin_indices: None,
            skin_weights: None,
            indices: vec![],
        };
        assert_eq!(
            shader_controls(&mesh, 0..3),
            Some(vec![[0., 0.], [1., 0.], [0., 1.]])
        );
        assert_eq!(shader_controls(&mesh, 1..2), Some(vec![[1., 0.]]));
        let bare = grid_formats::mesh::Mesh { colors_argb: vec![], ..mesh };
        assert_eq!(shader_controls(&bare, 0..3), None);
    }
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn source_tread_frames_and_masks() {
        use grid_formats::{mesh, pssg, scene};
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let bytes =
            std::fs::read(std::path::Path::new(&root).join("cars/lac/lac_highLOD.pssg")).unwrap();
        let tree = pssg::read_tree(&bytes).unwrap();
        let meshes = mesh::read_meshes(&tree).unwrap();
        let mut draws = 0;
        let (mut positive_draws, mut negative_draws) = (0, 0);
        for draw in scene::read_draws(&tree, &meshes, b"LOD0_").unwrap() {
            if draw.material != b"Tyre Tread" {
                continue;
            }
            let m = &meshes[draw.source];
            assert_eq!(m.tangents.len(), m.positions.len());
            assert_eq!(m.binormals.len(), m.positions.len());
            assert_eq!(m.colors_argb.len(), m.positions.len());
            let (mut positive, mut negative) = (0, 0);
            let mut minimum_cos = 1.0_f32;
            let mut maximum_nt = 0.0_f32;
            for i in draw.vertices.clone() {
                assert_eq!(m.colors_argb[i], [255, 0, 255, 255]);
                let n = Vec3::from_array(m.normals[i]).normalize();
                let t = Vec3::from_array(m.tangents[i]).normalize();
                let b = Vec3::from_array(m.binormals[i]).normalize();
                let cosine = n.cross(t).normalize().dot(b);
                assert!(cosine.is_finite());
                minimum_cos = minimum_cos.min(cosine.abs());
                maximum_nt = maximum_nt.max(n.dot(t).abs());
                if cosine > 0. {
                    positive += 1;
                } else {
                    negative += 1;
                }
            }
            println!(
                "source_tread_frame draw={:?} positive={} negative={} min_binormal_cos={} max_normal_dot_tangent={} uv_range={:?}",
                std::str::from_utf8(draw.id).unwrap(),
                positive,
                negative,
                minimum_cos,
                maximum_nt,
                draw.vertices.clone().fold(
                    ([f32::INFINITY; 2], [f32::NEG_INFINITY; 2]),
                    |(mut lo, mut hi), i| {
                        for a in 0..2 {
                            lo[a] = lo[a].min(m.texcoords[i][a]);
                            hi[a] = hi[a].max(m.texcoords[i][a]);
                        }
                        (lo, hi)
                    }
                )
            );
            // The purchased basis is not perfectly orthogonal. Verify finite
            // consistent handedness, but do not claim Bevy's reconstructed
            // binormal exactly reproduces the original stored vector.
            assert!(positive == 0 || negative == 0);
            if positive > 0 {
                positive_draws += 1
            } else {
                negative_draws += 1
            }
            draws += 1;
        }
        assert_eq!(draws, 4);
        assert_eq!((positive_draws, negative_draws), (2, 2));
    }
}
