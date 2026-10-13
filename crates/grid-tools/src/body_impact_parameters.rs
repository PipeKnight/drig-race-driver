//! Explicit private parameter data for the selected registered body handler.
use grid_world::{Error, body_impact::Parameters};

pub fn read(bytes: &[u8]) -> Result<Parameters, Error> {
    if bytes.len() != 28 || &bytes[..4] != b"BP01" {
        return Err(Error("invalid body impact parameter framing"));
    }
    let f = |i: usize| f32::from_le_bytes(bytes[4 + i * 4..8 + i * 4].try_into().unwrap());
    let values: [f32; 6] = std::array::from_fn(f);
    if values.iter().any(|v| !(0. ..=1000.).contains(v)) {
        return Err(Error("invalid body impact parameter range"));
    }
    Ok(Parameters {
        impact_scale: f(0),
        timer_cap: f(1),
        linear_squared_threshold: f(2),
        suspension_scale: f(3),
        suspension_gain: f(4),
        suspension_offset: f(5),
    })
}
