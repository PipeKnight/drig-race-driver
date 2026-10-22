//! Saved decal.fx compositing: texture alpha times vertex alpha, no opaque shadows.
//! Original shader lighting/normal/specular response is not reproduced here.
use bevy::prelude::*;
use grid_formats::pssg::Tree;
use std::{collections::BTreeSet, error::Error};

pub(crate) fn materials(tree: &Tree<'_>) -> Result<BTreeSet<String>, Box<dyn Error>> {
    let mut names = BTreeSet::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) == b"SHADERINSTANCE" && tree.string(i, b"shaderGroup")? == b"#decal.fx" {
            names.insert(String::from_utf8(tree.string(i, b"id")?.to_vec())?);
        }
    }
    Ok(names)
}

pub(crate) fn colours(source: &[[u8; 4]]) -> Vec<[f32; 4]> {
    // decal.decal.cg.vp forwards D3DCOLOR.w, decal.cg.fp multiplies sampled alpha.
    // RGB channels are shader controls, not a black multiplier on decal albedo.
    source.iter().map(|argb| [1., 1., 1., f32::from(argb[0]) / 255.]).collect()
}

pub(crate) fn configure(material: &mut StandardMaterial) {
    // Saved decal pass: SRC_ALPHA / ONE_MINUS_SRC_ALPHA, depth writes disabled.
    material.alpha_mode = AlphaMode::Blend;
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn installed_decal_group_and_vertex_alpha() {
        let root = std::env::var("GRID_GAME_ROOT").expect("GRID_GAME_ROOT purchased resource root");
        let bytes = std::fs::read(std::path::Path::new(&root).join("tracks/europe/donington/tracksplit.pssg")).unwrap();
        let tree = grid_formats::pssg::read_tree(&bytes).unwrap();
        let names = materials(&tree).unwrap();
        assert_eq!(names, BTreeSet::from(["don_decals_01".into(), "don_decals_02".into()]));
        assert!(!names.contains("track1/2!3"));
        let meshes = grid_formats::mesh::read_meshes(&tree).unwrap();
        let draws = grid_formats::scene::read_static_draws(&tree, &meshes).unwrap();
        let draw = draws.iter().find(|d| d.id == b"!eQ").unwrap();
        assert!(names.contains(std::str::from_utf8(draw.material).unwrap()));
        let source = &meshes[draw.source].colors_argb[draw.vertices.clone()];
        assert!(source.iter().any(|a| a[0] > 0 && a[0] < 255));
        assert!(source.iter().all(|a| a[1..] == [0, 0, 0]));
        let output = colours(source);
        assert!(output.iter().all(|c| c[..3] == [1., 1., 1.]));
        assert!(output.iter().any(|c| c[3] > 0. && c[3] < 1.));
    }
}
