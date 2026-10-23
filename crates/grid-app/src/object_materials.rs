//! Source static glass coverage and an owner-approved solid grandstand backing.
//! Other object families retain existing cutouts.
//! glass_simple colour uses SRC_ALPHA/ONE_MINUS_SRC_ALPHA with alpha test off.
//! Bevy PBR depth/separate-alpha remain adaptations, not original pass parity.
use bevy::prelude::*;
use grid_formats::pssg;
use std::collections::BTreeSet;

pub(crate) fn glass_materials(tree: &pssg::Tree<'_>) -> grid_formats::Result<BTreeSet<String>> {
    let mut result = BTreeSet::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) == b"SHADERINSTANCE"
            && tree.string(i, b"shaderGroup")? == b"#glass_simple.fx"
        {
            let id = String::from_utf8_lossy(tree.string(i, b"id")?);
            result.insert(format!("objects/{id}"));
        }
    }
    Ok(result)
}

pub(crate) fn configure(name: &str, glass: &BTreeSet<String>, material: &mut StandardMaterial) {
    if glass.contains(name) {
        material.alpha_mode = AlphaMode::Blend;
    } else if name == "objects/building_grandstand_01_j_01" {
        // Owner requests solid backing if the source coverage is not reproduced.
        // Source uses object_simple_mesh A2C; this one panel is an authored fallback,
        // not a family-wide opaque conversion. Keep its original RGB/UV/geometry.
        material.alpha_mode = AlphaMode::Opaque;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn detroit_stand_glass_does_not_use_the_concrete_fence_cutout() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let bytes = std::fs::read(std::path::Path::new(&root).join("tracks/usa/detroit/objects.pssg")).unwrap();
        let tree = pssg::read_tree(&bytes).unwrap();
        let glass = glass_materials(&tree).unwrap();
        let stand = "objects/gg_building_grandstand_01_j_04";
        let fence = "objects/concrete_fence_white_end_01";
        let backing = "objects/building_grandstand_01_j_01";
        assert!((0..tree.nodes.len()).any(|i| tree.name(i) == b"SHADERINSTANCE"
            && tree.string(i, b"id").is_ok_and(|id| id == b"building_grandstand_01_j_01")
            && tree.string(i, b"shaderGroup").is_ok_and(|group| group == b"#object_simple_mesh.fx")));
        assert!(glass.contains(stand));
        assert!(!glass.contains(fence));
        for (name, expected) in [(stand, AlphaMode::Blend), (fence, AlphaMode::Mask(0.5)), (backing, AlphaMode::Opaque), ("trees/leaf", AlphaMode::Mask(0.5))] {
            let mut material = StandardMaterial { alpha_mode: AlphaMode::Mask(0.5), ..default() };
            configure(name, &glass, &mut material);
            assert_eq!(material.alpha_mode, expected);
        }
    }
}
