//! Adds the authored lamp signal in the Type2 scene's display domain.
use bevy::{
    prelude::*,
    shader::{ShaderImport, Source as ShaderSource},
};

pub(super) const DISPLAY_SIGNAL: f32 = -1.;

#[derive(Default, Resource)]
pub(super) struct DisplayLamp {
    pub ready: bool,
    refused: bool,
}

pub(super) fn install(app: &mut App, track_grade: bool) {
    if track_grade && std::env::var("GRID_LAMP_SOURCE_DISPLAY").as_deref() != Ok("legacy") {
        // Track light specialization runs in Update. Extend the material
        // module only after it has installed its point/spot routines.
        app.init_resource::<DisplayLamp>()
            .add_systems(PostUpdate, patch_material_shader);
    }
}

fn patch_material_shader(world: &mut World) {
    let state = world.resource::<DisplayLamp>();
    if state.ready || state.refused || !crate::night_lights::shader_extensions_ready(world) {
        return;
    }
    let found = {
        let shaders = world.resource::<Assets<Shader>>();
        let mut matching = shaders.iter().filter(|(_, shader)| {
            shader.import_path == ShaderImport::Custom("bevy_pbr::pbr_functions".into())
        });
        let Some((id, shader)) = matching.next() else {
            return;
        };
        matching.next().is_none().then(|| (id, shader.clone()))
    };
    let Some((id, mut original)) = found else {
        world.resource_mut::<DisplayLamp>().refused = true;
        eprintln!("lamp_display shader=refused reason=ambiguous-material-module");
        return;
    };
    let patched = match &original.source {
        ShaderSource::Wgsl(wgsl) => patch_output(wgsl),
        _ => None,
    };
    if let Some(patched) = patched {
        original.source = ShaderSource::Wgsl(patched.into());
        world
            .resource_mut::<Assets<Shader>>()
            .insert(id, original)
            .expect("existing material shader ID");
        world.resource_mut::<DisplayLamp>().ready = true;
        println!("lamp_display shader=patched domain=display composition=source-signal");
    } else {
        world.resource_mut::<DisplayLamp>().refused = true;
        eprintln!("lamp_display shader=refused reason=unsupported-material-output");
    }
}

fn patch_output(source: &str) -> Option<String> {
    const IMPORT: &str = "#import bevy_render::maths::{E, powsafe}";
    const STORAGE: &str =
        "    emissive_light = emissive_light * mix(1.0, view_bindings::view.exposure, emissive.a);";
    const OUTPUT: &str = "        (view_bindings::view.exposure * (transmitted_light + direct_light + indirect_light)) + emissive_light,";
    const HEAD: &str = "    // Total light\n    output_color = vec4<f32>(\n";
    if [IMPORT, STORAGE, OUTPUT, HEAD]
        .iter()
        .any(|marker| source.matches(marker).count() != 1)
    {
        return None;
    }
    Some(source
        .replacen(IMPORT, "#import bevy_render::maths::{E, powsafe}\n#import bevy_render::color_operations as grid_srgb", 1)
        .replacen(STORAGE, "    emissive_light *= mix(1.0, view_bindings::view.exposure, max(emissive.a, 0.0));", 1)
        .replacen(HEAD, r#"    // Total light
    let grid_lit = view_bindings::view.exposure * (transmitted_light + direct_light + indirect_light);
    var grid_colour = grid_lit + emissive_light;
    if emissive.a < -0.5 {
        let encoded = grid_srgb::linear_to_srgb(max(grid_lit, vec3(0.0)));
        grid_colour = grid_srgb::srgb_to_linear(encoded + emissive_light);
    }
    output_color = vec4<f32>(
"#, 1)
        .replacen(OUTPUT, "        grid_colour,", 1))
}
