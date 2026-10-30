// Reduced tyre-smoke buffer (F645): scene depth reduction and composition.
// Independently authored after the original's pfx_downsample_depth and
// pfx_compose passes (docs/TYRE_SMOKE_RESEARCH.md §7).
#ifdef MULTISAMPLED
@group(0) @binding(0) var scene_depth: texture_depth_multisampled_2d;
#else
@group(0) @binding(0) var scene_depth: texture_depth_2d;
#endif
@group(0) @binding(1) var smoke: texture_2d<f32>;
@group(0) @binding(2) var smoke_depth: texture_depth_2d;

// Relative depth step at which a reduced texel's weight halves against one
// matching the pixel's depth: small enough that a surface behind an edge
// hardly counts, large enough that one surface's slope across two pixels
// keeps the plain bilinear weights.
const STEP: f32 = 0.01;
// Above this relative step no texel of the bilinear footprint lies on the
// pixel's surface.
const MISS: f32 = 0.05;

fn relative_step(d: f32, z: f32) -> f32 {
    return abs(d - z) / max(max(d, z), 1e-6);
}

// A pixel's scene depth at one sample.
fn scene(p: vec2<i32>, sample: u32) -> f32 {
#ifdef MULTISAMPLED
    return textureLoad(scene_depth, p, sample);
#else
    return textureLoad(scene_depth, p, 0);
#endif
}

// The nearest scene depth (largest, reverse Z) of the four pixels under each
// reduced texel. The original's reduction kept both bounds and faded against
// the far one, redrawing pixels between the bounds at full size; without that
// pass the near bound keeps smoke from crossing a near edge, and the compose
// takes a farther surface's smoke from a neighbouring texel. The first sample
// of each pixel stands for it.
@fragment
fn downsample(@builtin(position) pixel: vec4<f32>) -> @builtin(frag_depth) f32 {
    let last = vec2<i32>(textureDimensions(scene_depth)) - 1;
    let base = vec2<i32>(pixel.xy) * 2;
    var near = 0.0;
    for (var i = 0; i < 4; i++) {
        near = max(near, scene(min(base + vec2(i & 1, i >> 1u), last), 0u));
    }
    return near;
}

// Smoke colour (RGB, already weighted by coverage) and transmittance (A) for
// one full-size pixel: the four nearest reduced texels' bilinear weights,
// each divided by its relative depth step from the pixel, so smoke behind a
// near edge does not spread over it. Where none of the four lies on the
// pixel's surface, the closest match among the nine texels around the
// pixel's own. Multisampled, it runs once per sample: the samples of an edge
// pixel lie on both sides of the edge.
@fragment
fn compose(
    @builtin(position) pixel: vec4<f32>,
#ifdef MULTISAMPLED
    @builtin(sample_index) sample: u32,
#endif
) -> @location(0) vec4<f32> {
#ifndef MULTISAMPLED
    let sample = 0u;
#endif
    let last = vec2<i32>(textureDimensions(smoke)) - 1;
    let u = pixel.xy * 0.5 - 0.5;
    let base = vec2<i32>(floor(u));
    let f = u - floor(u);
    // No smoke in the footprint: nothing to compose, no depth to read.
    var c: array<vec4<f32>, 4>;
    var empty = true;
    for (var i = 0; i < 4; i++) {
        c[i] = textureLoad(smoke, clamp(base + vec2(i & 1, i >> 1u), vec2(0), last), 0);
        empty = empty && c[i].a >= 0.999 && max(c[i].r, max(c[i].g, c[i].b)) <= 0.001;
    }
    if empty {
        discard;
    }
    let z = scene(vec2<i32>(pixel.xy), sample);
    var sum = vec4(0.0);
    var total = 0.0;
    var best = 1e9;
    for (var i = 0; i < 4; i++) {
        let o = vec2(i & 1, i >> 1u);
        let step = relative_step(textureLoad(smoke_depth, clamp(base + o, vec2(0), last), 0), z);
        let b = select(1.0 - f, f, o == vec2(1));
        let w = (b.x * b.y + 1e-3) / (1.0 + step / STEP);
        sum += c[i] * w;
        total += w;
        best = min(best, step);
    }
    var out = sum / total;
    if best > MISS {
        let own = vec2<i32>(pixel.xy) / 2;
        for (var i = 0; i < 9; i++) {
            let p = clamp(own + vec2(i % 3 - 1, i / 3 - 1), vec2(0), last);
            let step = relative_step(textureLoad(smoke_depth, p, 0), z);
            if step < best {
                best = step;
                out = textureLoad(smoke, p, 0);
            }
        }
    }
    return out;
}
