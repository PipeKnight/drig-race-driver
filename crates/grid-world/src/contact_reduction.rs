//! Ordered mean point and weighted direction used by the collision callback.
use crate::{Error, impact_generation::Contact};
pub struct Prepared {
    pub point: [f32; 4],
    pub direction: [f32; 4],
}
#[allow(clippy::assign_op_pattern)] // Preserve contact-first SSE addition order.
#[allow(clippy::neg_multiply)] // Original MULPS multiplies each reversed lane by -1.
pub fn reduce(contacts: &[Contact]) -> Result<Prepared, Error> {
    if contacts.is_empty()
        || contacts.len() > 80
        || contacts.iter().any(|c| {
            c.point
                .into_iter()
                .chain(c.direction)
                .chain([c.value_3c])
                .any(|v| !v.is_finite())
        })
    {
        return Err(Error("invalid contact reduction input"));
    }
    let mut point = [0.; 4];
    let mut direction = [0.; 4];
    // All contacts participate, including disabled/nonpositive generation inputs.
    for c in contacts {
        let normal = if c.reverse {
            c.direction.map(|v| -1. * v)
        } else {
            c.direction
        };
        for i in 0..4 {
            direction[i] = c.value_3c * normal[i] + direction[i];
            point[i] = c.point[i] + point[i];
        }
    }
    let inverse = 1. / contacts.len() as f32;
    point = point.map(|v| inverse * v);
    if point.into_iter().chain(direction).any(|v| !v.is_finite()) {
        return Err(Error("nonfinite contact reduction result"));
    }
    Ok(Prepared { point, direction })
}
