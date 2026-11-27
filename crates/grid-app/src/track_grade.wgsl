// Type2 race postprocess over the normalized scene range.
// Intermediate unorm attachments store display values, without sRGB conversion.
#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput

struct RaceEffect {
    thresholds: vec4<f32>,   // lower, upper, intensity, history weight
    mix_amounts: vec4<f32>,  // scene, bloom, softening, medium bloom
    exposure: vec4<f32>,     // lumMin, lumMax, adaptMax, large bloom
    balance: vec4<f32>,      // contrast, brightness, maximum, colour
    tint: vec4<f32>,         // prepared tint, vignette amount
    vignette: vec4<f32>,     // vignette rgb, shape
    frame: vec4<f32>,        // scale xy, offset xy
    rotation: vec4<f32>,     // cos, sin, transition, timeline luminance scale
    tone_threshold: vec4<f32>, // luminance threshold, prepared scale, enabled, no blur
}
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var extra: texture_2d<f32>;
@group(0) @binding(2) var history: texture_2d<f32>;
@group(0) @binding(3) var linear_sampler: sampler;
@group(0) @binding(4) var<uniform> fx: RaceEffect;
@group(0) @binding(5) var adapted_luminance: texture_2d<f32>;

const LUMA = vec3<f32>(0.2125, 0.7154, 0.0721);

fn to_display(c: vec3<f32>) -> vec3<f32> {
    return select(1.055 * pow(max(c, vec3(0.0)), vec3(1.0 / 2.4)) - 0.055,
                  c * 12.92, c <= vec3(0.0031308));
}

fn to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3(2.4)),
                  c / 12.92, c <= vec3(0.04045));
}

fn source_display_codes(colour: vec3<f32>) -> vec3<f32> {
    // The D3D9 display target truncates to a 12-bit bucket before its UNORM8
    // conversion. At a half-code boundary the smaller code wins.
    let bucket = vec3<u32>(saturate(colour) * 4096.0);
    let code = (bucket * 255u + 2047u) >> vec3<u32>(12u);
    return vec3<f32>(code) / 255.0;
}

fn sample_src(uv: vec2<f32>) -> vec3<f32> {
    return textureSampleLevel(src, linear_sampler, uv, 0.0).rgb;
}

@fragment
fn bound_main_colour(input: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let base = textureLoad(src, vec2<i32>(input.position.xy), 0);
    return vec4(saturate(base.rgb), base.a);
}

@fragment
fn display_x4(input: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    // Samples form the four bilinear quadrants of a 4x4 pixel area.
    let d = 1.0 / vec2<f32>(textureDimensions(src));
    var rgb = vec3<f32>(0.0);
    for (var y = 0; y < 2; y++) {
        for (var x = 0; x < 2; x++) {
            rgb += to_display(sample_src(input.uv + (vec2<f32>(f32(x), f32(y)) * 2.0 - 1.0) * d));
        }
    }
    return vec4(0.25 * rgb, 1.0);
}

@fragment
fn down_x4(input: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let d = 1.0 / vec2<f32>(textureDimensions(src));
    return vec4(0.25 * (
        sample_src(input.uv + d * vec2(-1.0, -1.0)) +
        sample_src(input.uv + d * vec2(1.0, -1.0)) +
        sample_src(input.uv + d * vec2(-1.0, 1.0)) +
        sample_src(input.uv + d * vec2(1.0, 1.0))), 1.0);
}

@fragment
fn luminance(input: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    var sum = vec3<f32>(0.0);
    for (var y = 0; y < 4; y++) {
        for (var x = 0; x < 4; x++) {
            sum += sample_src((vec2<f32>(f32(x), f32(y)) + 0.5) * 0.25);
        }
    }
    return vec4(vec3(dot(sum / 16.0, LUMA)), 1.0);
}

@fragment
fn adapt(input: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let value = textureLoad(src, vec2<i32>(0), 0).x;
    let previous = textureLoad(history, vec2<i32>(0), 0).xy;
    let amount = min(max(value, fx.exposure.x * fx.rotation.w), fx.exposure.y * fx.rotation.w);
    let delta = amount / (value + 0.0001) - previous.y;
    let speed = select(0.0, min(fx.exposure.z, previous.x + fx.exposure.z * fx.exposure.z),
                       abs(delta) >= 0.01);
    return vec4(speed, previous.y + speed * delta, 0.0, 0.0);
}

@fragment
fn bloom_x2(input: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let intensity = fx.thresholds.z;
    // ps_3_0 LOG maps zero to -FLT_MAX, so EXP(LOG(x) * 0) is one
    // for every thresholded input, including zero.
    if intensity == 0.0 {
        return vec4(1.0);
    }
    let lower = fx.thresholds.x;
    let inverse_span = 1.0 / (fx.thresholds.y - lower);
    let d = 0.5 / vec2<f32>(textureDimensions(src));
    var rgb = vec3<f32>(0.0);
    for (var y = -1; y <= 1; y += 2) {
        for (var x = -1; x <= 1; x += 2) {
            let sample_rgb = sample_src(input.uv + vec2<f32>(f32(x), f32(y)) * d);
            // Tone_dsX2 applies its LOG/MUL/EXP curve to each sample before averaging.
            let normalized = saturate((sample_rgb - lower) * inverse_span);
            rgb += pow(normalized, vec3(intensity));
        }
    }
    return vec4(rgb * 0.25, 1.0);
}

fn gauss9(uv: vec2<f32>, step_uv: vec2<f32>) -> vec4<f32> {
    let near = step_uv * 1.407333374;
    let far = step_uv * 3.29421496;
    return vec4(sample_src(uv) * 0.199471146 +
        (sample_src(uv - near) + sample_src(uv + near)) * 0.297018021 +
        (sample_src(uv - far) + sample_src(uv + far)) * 0.09175428, 1.0);
}

@fragment
fn gauss9_h(input: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return gauss9(input.uv, vec2(1.0 / f32(textureDimensions(src).x), 0.0));
}

@fragment
fn gauss9_v(input: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    return gauss9(input.uv, vec2(0.0, 1.0 / f32(textureDimensions(src).y)));
}

@fragment
fn bloom_large(input: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let d = 1.182425499 / vec2<f32>(textureDimensions(src));
    let uv = input.uv;
    let sides = sample_src(uv + vec2(d.x, 0.0)) + sample_src(uv - vec2(d.x, 0.0)) +
                sample_src(uv + vec2(0.0, d.y)) + sample_src(uv - vec2(0.0, d.y));
    let corners = sample_src(uv + d) + sample_src(uv - d) +
                  sample_src(uv + vec2(d.x, -d.y)) + sample_src(uv + vec2(-d.x, d.y));
    return vec4(sample_src(uv) * 0.159154937 + sides * 0.118071631 + corners * 0.087593325, 1.0);
}

@fragment
fn bloom_merge(input: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let medium = sample_src(input.uv);
    let large = textureSampleLevel(extra, linear_sampler, input.uv, 0.0).rgb;
    let previous = textureSampleLevel(history, linear_sampler, input.uv, 0.0).rgb;
    return vec4(mix(medium * fx.mix_amounts.w + large * fx.exposure.w,
                    previous, fx.thresholds.w), 1.0);
}

@fragment
fn soften(input: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    // The source softening quad samples the corner of each quarter-resolution texel.
    let source_uv = input.uv - 0.5 * fwidth(input.uv);
    let exposure = textureLoad(extra, vec2<i32>(0), 0).y;
    return vec4(sample_src(source_uv) * exposure, 1.0);
}

fn merge_uv(uv: vec2<f32>, size: vec2<u32>) -> vec2<f32> {
    return uv + 0.5 / vec2<f32>(size);
}

@fragment
fn final_merge(input: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    // The source quad supplies (pixel + 0.4) / output size. Each input
    // then adds half a texel measured in its own texture dimensions.
    let pixel_uv = input.uv - 0.1 * fwidth(input.uv);
    let texel = textureSampleLevel(src, linear_sampler, merge_uv(pixel_uv, textureDimensions(src)), 0.0);
    let soft = textureSampleLevel(extra, linear_sampler, merge_uv(pixel_uv, textureDimensions(extra)), 0.0).rgb;
    let bloom = textureSampleLevel(history, linear_sampler, merge_uv(pixel_uv, textureDimensions(history)), 0.0).rgb;
    let scene_rgb = to_display(texel.rgb);
    let exposure = textureLoad(adapted_luminance, vec2<i32>(0), 0).y;
    let no_blur = fx.tone_threshold.w > 0.0;
    // CheapFinalMerge applies colourAmount after SwigMotionBlur's exposure.
    // FinalMergeNoBlur weights the raw scene first, including its threshold.
    let exposure_input = select(scene_rgb, scene_rgb * fx.mix_amounts.x, no_blur);
    let peak = max(exposure_input.r, max(exposure_input.g, exposure_input.b));
    let threshold_weight = saturate(peak - fx.tone_threshold.x) * fx.tone_threshold.y;
    let factor = select(exposure, 1.0 + threshold_weight * (exposure - 1.0),
                        fx.tone_threshold.z > 0.0 && exposure > 1.0);
    // Swig has an integer output inherited from MainColourTarget.
    // FinalMergeNoBlur computes exposure inside this pass.
    let exposed = exposure_input * factor;
    let scene = select(saturate(exposed) * fx.mix_amounts.x, exposed, no_blur) +
        soft * fx.mix_amounts.z;
    let inverse_luminance = 1.0 - dot(scene, LUMA);
    let bloom_gate = select(saturate(inverse_luminance), max(inverse_luminance, 0.0), no_blur);
    let merged = scene + bloom * fx.mix_amounts.y * bloom_gate;
    let tinted = mix(vec3(dot(merged, LUMA)), merged, fx.balance.w) * fx.tint.rgb;

    let p = 2.0 * pixel_uv - vec2(1.0);
    let c = fx.rotation.x;
    let s = fx.rotation.y;
    let r = vec2(c * p.x - s * p.y, c * p.y + s * p.x);
    let q = 0.89 * fx.frame.xy * (fx.vignette.w * r + vec2(-fx.frame.z, fx.frame.w));
    let tau = fx.rotation.z;
    let z = saturate(saturate(dot(q, q)) / tau + 1.0 - 1.0 / tau);
    let vignette_weight = fx.tint.w * z * z * (3.0 - 2.0 * z);
    let shaded = mix(tinted, fx.vignette.rgb, vignette_weight);
    let graded = clamp(shaded * fx.balance.x + fx.balance.y, vec3(0.0), vec3(fx.balance.z));
    return vec4(to_linear(source_display_codes(graded)), texel.a);
}
