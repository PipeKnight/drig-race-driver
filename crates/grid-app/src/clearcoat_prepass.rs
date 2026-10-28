//! Narrow in-memory repair for the pinned Bevy0.19.1 shader library.
//! No installed dependency or renderer validation settings are modified.
use bevy::{
    prelude::*,
    shader::{ShaderImport, Source},
};

const ORIGINAL: &str = "#ifdef LOAD_PREPASS_NORMALS\n    pbr_input.N = prepass_utils::prepass_normal(in.position, 0u);\n#else";
const REPAIRED: &str = "#ifdef LOAD_PREPASS_NORMALS\n    pbr_input.N = prepass_utils::prepass_normal(in.position, 0u);\n    pbr_input.clearcoat_N = normalize(pbr_input.world_normal);\n#else";

fn repair(source: &str) -> Result<String, &'static str> {
    if source.matches(ORIGINAL).count() != 1 {
        return Err("pinned Bevy prepass signature absent or ambiguous");
    }
    Ok(source.replacen(ORIGINAL, REPAIRED, 1))
}

pub fn install(app: &mut App) {
    app.add_systems(Update, patch_loaded_library);
}

pub(crate) fn patch_loaded_library(
    mut shaders: ResMut<Assets<Shader>>,
    mut reported: Local<bool>,
    views: Query<
        Entity,
        (
            With<super::ViewerCamera>,
            With<bevy::core_pipeline::prepass::NormalPrepass>,
        ),
    >,
) {
    if *reported {
        return;
    }
    let targets:Vec<_>=shaders.iter().filter_map(|(id,shader)| matches!(&shader.import_path,ShaderImport::Custom(path) if path=="bevy_pbr::pbr_fragment").then_some(id)).collect();
    if targets.is_empty() {
        return;
    }
    *reported = true;
    if targets.len() != 1 {
        eprintln!("clearcoat_prepass repaired=false reason=ambiguous-shader-library");
        return;
    }
    let mut shader = shaders.get_mut(targets[0]).expect("selected shader exists");
    let Source::Wgsl(source) = &shader.source else {
        eprintln!("clearcoat_prepass repaired=false reason=not-wgsl");
        return;
    };
    match repair(source) {
        Ok(source) => {
            // Keep all import/dependency/definition/validation metadata intact.
            shader.source = Source::Wgsl(source.into());
            println!(
                "clearcoat_prepass repaired=true bevy=0.19.1 normal_prepass_views={} normal=geometric shader_validation=unchanged",
                views.iter().count()
            );
        }
        Err(error) => eprintln!("clearcoat_prepass repaired=false reason={error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_signature_and_single_branch_only() {
        let input = format!("prefix\n{ORIGINAL}\n    pbr_input.N = normal;\n#endif\nsuffix");
        let output = repair(&input).unwrap();
        assert_eq!(
            output,
            format!("prefix\n{REPAIRED}\n    pbr_input.N = normal;\n#endif\nsuffix")
        );
        assert!(repair(&output).is_err());
        assert!(repair("different-version").is_err());
        assert!(repair(&format!("{ORIGINAL}\n{ORIGINAL}")).is_err());
    }
}
