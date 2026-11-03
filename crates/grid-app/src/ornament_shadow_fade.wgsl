// Ornament shadow fades (ornament_lod.rs `shadow_proxies`): the shadow pass
// of a shadow-level part whose instance crosses D. The step of Bevy's
// visibility-range crossfade comes in the mesh tag (bits 0..4; bit 5 set:
// appearing), and the same 4x4 pattern dithers the caster in the shadow map,
// which the shadow filter then softens.
#import bevy_pbr::{
    prepass_io,
    pbr_prepass_functions,
    mesh_functions,
}

// Bevy's DITHER_THRESHOLD_MAP (pbr_functions.wgsl, 0.19.1).
const THRESHOLDS: vec4<u32> = vec4(0x0a020800u, 0x060e040cu, 0x09010b03u, 0x050d070fu);

fn shadow_fade(in: prepass_io::VertexOutput) {
    pbr_prepass_functions::prepass_alpha_discard(in);
    let tag = mesh_functions::get_tag(in.instance_index);
    let step = i32(tag & 31u);
    let dither = select(step, step - 16, (tag & 32u) != 0u);
    if dither == 0 {
        return;
    }
    if dither <= -16 || dither >= 16 {
        discard;
    }
    let coords = vec2<u32>(floor(in.position.xy)) % 4u;
    let threshold = i32((THRESHOLDS[coords.y] >> (coords.x * 8u)) & 0xffu);
    if (dither >= 0 && dither + threshold >= 16) || (dither < 0 && 1 + dither + threshold <= 0) {
        discard;
    }
}

#ifdef PREPASS_FRAGMENT
@fragment
fn fragment(in: prepass_io::VertexOutput) -> prepass_io::FragmentOutput {
    shadow_fade(in);
    var out: prepass_io::FragmentOutput;
#ifdef UNCLIPPED_DEPTH_ORTHO_EMULATION
    out.frag_depth = in.unclipped_depth;
#endif
    return out;
}
#else
@fragment
fn fragment(in: prepass_io::VertexOutput) {
    shadow_fade(in);
}
#endif
