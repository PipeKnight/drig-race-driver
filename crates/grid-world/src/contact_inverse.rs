//! Pose inverse used before collision descriptor preparation.
use crate::Error;

/// Transpose the supplied basis and invert translation in original SSE order.
/// The original assumes a rigid basis; this does not validate orthonormality.
pub fn calculate(pose: [[f32; 4]; 4]) -> Result<[[f32; 4]; 4], Error> {
    if pose.iter().flatten().any(|v| !v.is_finite()) {
        return Err(Error("invalid contact inverse pose"));
    }
    let mut inverse = [[0.; 4]; 4];
    for (axis, row) in inverse.iter_mut().take(3).enumerate() {
        for (i, value) in row.iter_mut().take(3).enumerate() {
            *value = pose[i][axis];
        }
    }
    let negative = pose[3].map(|v| f32::from_bits(v.to_bits() ^ 0x8000_0000));
    inverse[3] = std::array::from_fn(|i| {
        if i == 3 {
            1.
        } else {
            (negative[1] * inverse[1][i] + negative[0] * inverse[0][i])
                + negative[2] * inverse[2][i]
        }
    });
    if inverse.iter().flatten().any(|v| !v.is_finite()) {
        return Err(Error("nonfinite contact inverse result"));
    }
    Ok(inverse)
}
