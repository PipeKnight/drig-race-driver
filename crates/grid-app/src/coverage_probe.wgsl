// Authored synthetic fixture only; no game texture or original backend mapping.
@vertex fn front(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32> {
    let xy=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
    return vec4(xy[i],0.5,1.);
}
@vertex fn behind(@builtin(vertex_index) i:u32)->@builtin(position) vec4<f32> {
    let xy=array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
    return vec4(xy[i],0.25,1.);
}
// Four rows are four policies; x-input is identical for all four.
fn alpha(x:u32)->f32 {
    if x<8u {return 0.;}
    if x<16u {return 1./255.;}
    if x<24u {return 0.125;}
    if x<32u {return 0.25;}
    if x<40u {return 0.5;}
    if x<48u {return 0.75;}
    if x<56u {return 1.;}
    if x<60u {return f32(x-56u)/3.;}
    return f32(x%2u);
}
@fragment fn opaque(@builtin(position) p:vec4<f32>)->@location(0) vec4<f32> {return vec4(1.);}
@fragment fn mask(@builtin(position) p:vec4<f32>)->@location(0) vec4<f32> {
    if alpha(u32(p.x))<0.5 {discard;}
    return vec4(1.);
}
@fragment fn sharpened(@builtin(position) p:vec4<f32>)->@location(0) vec4<f32> {
    // Pinned Bevy main-pass A2C arithmetic, no original parity claim.
    let a=alpha(u32(p.x));
    let coverage=(a-0.5)/max(fwidth(a),0.0001)+0.5;
    return vec4(1.,1.,1.,coverage);
}
@fragment fn raw(@builtin(position) p:vec4<f32>)->@location(0) vec4<f32> {
    let a=alpha(u32(p.x));
    if a<=0. {discard;}
    return vec4(1.,1.,1.,a);
}
@fragment fn red()->@location(0) vec4<f32> {return vec4(1.,0.,0.,0.);}
@group(0) @binding(0) var colour:texture_multisampled_2d<f32>;
@group(0) @binding(1) var depth:texture_depth_multisampled_2d;
// One record per pixel/sample: beforeRGBA, beforeDepthBits, afterRGBA, afterDepthBits.
@group(0) @binding(2) var<storage,read_write> result:array<vec4<u32>>;
fn index(p:vec2<u32>,s:u32)->u32 {return (p.y*64u+p.x)*4u+s;}
@compute @workgroup_size(8,1,1)
fn snapshot_before(@builtin(global_invocation_id) id:vec3<u32>) {
    if id.x>=64u || id.y>=4u {return;}
    for(var s=0u;s<4u;s++) {
        let row=index(id.xy,s);
        result[row].x=pack4x8unorm(textureLoad(colour,vec2<i32>(id.xy),i32(s)));
        result[row].y=bitcast<u32>(textureLoad(depth,vec2<i32>(id.xy),i32(s)));
    }
}
@compute @workgroup_size(8,1,1)
fn snapshot_after(@builtin(global_invocation_id) id:vec3<u32>) {
    if id.x>=64u || id.y>=4u {return;}
    for(var s=0u;s<4u;s++) {
        let row=index(id.xy,s);
        result[row].z=pack4x8unorm(textureLoad(colour,vec2<i32>(id.xy),i32(s)));
        result[row].w=bitcast<u32>(textureLoad(depth,vec2<i32>(id.xy),i32(s)));
    }
}
