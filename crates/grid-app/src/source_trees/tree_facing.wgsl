// Faces source-tree billboards to the main camera in their mesh slabs (gpu_facing.rs).
// The same arithmetic as source_trees.rs `present` (CPU path): sphere quads along the
// camera's right/up, sheets and impostor cards about world Y towards the eye, cards also
// pick their atlas view, its blend weight and a radial normal. One workgroup per visible
// placement; its lanes loop over the vertices.

struct Placement {
    world: mat4x4<f32>,
    inverse: mat4x4<f32>,
    // kind (0 sphere, 1 sheet, 2 card), first source vertex, vertex count, scale (f32 bits)
    a: vec4<u32>,
    // stride, POSITION, NORMAL, UV0 (in u32 words; NONE when absent)
    b: vec4<u32>,
    // UV1, unused x3
    c: vec4<u32>,
    d: vec4<u32>,
}

struct View {
    // camera right; w = 1 to blend the two nearest card views (GRID_CARD_BLEND)
    right: vec4<f32>,
    up: vec4<f32>,
    eye: vec4<f32>,
}

@group(0) @binding(0) var<storage, read_write> slab: array<u32>;
@group(0) @binding(1) var<storage, read> placements: array<Placement>;
// Three vec4 per source vertex: (centre.xyz, offset.x), (offset.y, atlas.uv, o.x), (o.y, 0, 0, 0)
@group(0) @binding(2) var<storage, read> sources: array<vec4<f32>>;
// (placement, first word of the mesh's vertices in the bound slab range)
@group(0) @binding(3) var<storage, read> jobs: array<vec2<u32>>;
@group(0) @binding(4) var<uniform> view: View;

const NONE: u32 = 0xffffffffu;
const PI: f32 = 3.14159265358979323846;
const Y: vec3<f32> = vec3<f32>(0.0, 1.0, 0.0);

// glam's try_normalize: None unless 1/length is finite and positive.
fn try_normalize(v: vec3<f32>, fallback: vec3<f32>) -> vec3<f32> {
    let r = 1.0 / length(v);
    if r > 0.0 && r <= 3.4028234e38 {
        return v * r;
    }
    return fallback;
}

fn put3(at: u32, v: vec3<f32>) {
    slab[at] = bitcast<u32>(v.x);
    slab[at + 1u] = bitcast<u32>(v.y);
    slab[at + 2u] = bitcast<u32>(v.z);
}

fn put2(at: u32, v: vec2<f32>) {
    slab[at] = bitcast<u32>(v.x);
    slab[at + 1u] = bitcast<u32>(v.y);
}

@compute @workgroup_size(64)
fn face(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(num_workgroups) groups: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
) {
    let index = group.y * groups.x + group.x;
    if index >= arrayLength(&jobs) {
        return;
    }
    let job = jobs[index];
    let p = placements[job.x];
    let base = job.y;
    let kind = p.a.x;
    let first = p.a.y;
    let count = p.a.z;
    let scale = bitcast<f32>(p.a.w);
    let stride = p.b.x;
    let position_at = p.b.y;
    let normal_at = p.b.z;
    let uv0_at = p.b.w;
    let uv1_at = p.c.x;
    let eye = view.eye.xyz;
    let origin = p.world[3].xyz;

    // Card: the atlas view seen from the eye and the weight of the next one.
    var shift = 0.0;
    var weight = 0.0;
    var toward = vec3<f32>(0.0, 0.0, 1.0);
    var card_right = vec3<f32>(1.0, 0.0, 0.0);
    if kind == 2u {
        let d = try_normalize(eye - origin, vec3<f32>(0.0));
        let v4 = try_normalize(p.world[0].xyz, vec3<f32>(0.0));
        let v6 = try_normalize(p.world[2].xyz, vec3<f32>(0.0));
        let q = -4.0 * atan2(dot(d, v6), dot(d, v4)) / PI + 2.0;
        let whole = floor(q);
        var shown = u32(((i32(whole) % 8) + 8) % 8);
        weight = q - whole;
        if view.right.w < 0.5 {
            shown = (shown + select(0u, 1u, weight >= 0.5)) % 8u;
            weight = 0.0;
        }
        shift = f32(shown) / 8.0;
        let t = eye - origin;
        toward = try_normalize(vec3<f32>(t.x, 0.0, t.z), vec3<f32>(0.0, 0.0, 1.0));
        card_right = cross(Y, toward);
    }
    let upright = try_normalize((p.inverse * vec4<f32>(Y, 0.0)).xyz, Y);

    for (var i = lane; i < count; i += 64u) {
        let s0 = sources[(first + i) * 3u];
        let s1 = sources[(first + i) * 3u + 1u];
        let s2 = sources[(first + i) * 3u + 2u];
        let centre = s0.xyz;
        let offset = vec2<f32>(s0.w, s1.x);
        var right = view.right.xyz;
        var up = view.up.xyz;
        if kind != 0u {
            let placed = (p.world * vec4<f32>(centre, 1.0)).xyz;
            right = try_normalize(cross(Y, vec3<f32>(eye.x - placed.x, 0.0, eye.z - placed.z)), vec3<f32>(1.0, 0.0, 0.0));
            up = Y;
        }
        let point = centre + (p.inverse * vec4<f32>((right * offset.x + up * offset.y) * scale, 0.0)).xyz;
        let at = base + i * stride;
        put3(at + position_at, point);
        if normal_at != NONE {
            var normal = upright;
            if kind == 0u {
                normal = try_normalize(centre, Y);
            } else if kind == 2u {
                let radial = toward + card_right * s1.w + Y * s2.x;
                normal = try_normalize((p.inverse * vec4<f32>(radial, 0.0)).xyz, vec3<f32>(0.0, 0.0, 1.0));
            }
            put3(at + normal_at, normal);
        }
        if kind == 2u {
            if uv0_at != NONE {
                put2(at + uv0_at, vec2<f32>(s1.y + shift, s1.z));
            }
            if uv1_at != NONE {
                put2(at + uv1_at, vec2<f32>(weight, 0.0));
            }
        }
    }
}
