//! Private file-backed defaults for both registered physical deformation phases.
use grid_world::{
    Error, deformation_callback_state::Parameters as Bulk,
    deformation_contact_limit::Parameters as Limit,
};
pub fn read(bytes: &[u8]) -> Result<(Limit, Bulk), Error> {
    if bytes.len() != 252 || &bytes[..4] != b"DF01" {
        return Err(Error("invalid deformation parameter framing"));
    }
    let a = &bytes[4..100];
    let b = &bytes[100..];
    let word = |b: &[u8], at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
    let f = |b: &[u8], at| f32::from_bits(word(b, at));
    if (0..23).any(|i| !f(a, i * 4).is_finite())
        || (0..38).any(|i| ![14, 15, 18, 37].contains(&i) && !f(b, i * 4).is_finite())
        || [word(a, 92), word(b, 60), word(b, 72), word(b, 148)]
            .iter()
            .any(|&x| x > 1)
        || !(1..=64).contains(&word(b, 56))
        || f(a, 20) != f32::MAX
        || f(b, 32) <= 0.
    {
        return Err(Error("invalid deformation parameter values"));
    }
    let limit = Limit {
        low_mass_threshold: f(a, 0),
        low_mass_speed_cap: f(a, 4),
        corner_scale: f(a, 8),
        velocity_projection: f(a, 12),
        stiffness_scale: f(a, 16),
        unlimited: f(a, 20),
        damage_scale: f(a, 24),
        table: std::array::from_fn(|i| f(a, 28 + i * 4)),
        global_disabled: word(a, 92) != 0,
    };
    let bulk = Bulk {
        generation: std::array::from_fn(|i| f(b, i * 4)),
        body: std::array::from_fn(|i| f(b, 24 + i * 4)),
        grid: std::array::from_fn(|i| f(b, 44 + i * 4)),
        grid_iterations: word(b, 56),
        clip_each_iteration: word(b, 60) != 0,
        response_scale: f(b, 64),
        correction_gain: f(b, 68),
        subtract_baseline: word(b, 72) != 0,
        policy_global_factor: f(b, 76),
        policy_scale: f(b, 80),
        policy_table: std::array::from_fn(|i| f(b, 84 + i * 4)),
        global_disabled: word(b, 148) != 0,
    };
    if limit.corner_scale.to_bits() != bulk.generation[0].to_bits()
        || limit.unlimited.to_bits() != bulk.generation[1].to_bits()
        || limit.damage_scale.to_bits() != bulk.policy_scale.to_bits()
        || limit.table.map(f32::to_bits) != bulk.policy_table.map(f32::to_bits)
        || limit.global_disabled != bulk.global_disabled
    {
        return Err(Error("inconsistent aliased deformation parameters"));
    }
    Ok((limit, bulk))
}
