struct Parameters { identity: vec4u, allocation: vec4u, settings: vec4u }
@group(0) @binding(0) var<storage, read> attempts: u32;
@group(0) @binding(1) var<uniform> capacity: u32;
@group(0) @binding(2) var<uniform> parameters: Parameters;
@group(0) @binding(3) var<storage, read_write> result: array<vec4u>;
@compute @workgroup_size(1)
fn main() {
    result[0] = parameters.identity;
    result[1] = vec4u(attempts, capacity, parameters.allocation.y, parameters.allocation.z);
    result[2] = parameters.settings;
}
