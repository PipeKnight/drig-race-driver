//! Independent Moller–Trumbore primitive with observed GRID f32 tolerances.
//! This does not implement its chunk selection, normal transform or contact solver.

#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub fraction: f32,
    pub u: f32,
    pub v: f32,
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] * b[0] + a[1] * b[1]) + a[2] * b[2]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Lowest and highest fraction (and barycentric) the query accepts: GRID's
/// f32 tolerances, so a hit at the segment start can report a tiny negative
/// fraction.
pub const FRACTION_LOW: f32 = -1e-6;
pub const FRACTION_HIGH: f32 = f32::from_bits(0x3f800008);

/// Direction is the finite segment displacement, not a unit ray.
/// Small accepted negative/above-one fractions are retained, not clamped.
pub fn query(start: [f32; 3], direction: [f32; 3], vertices: [[f32; 3]; 3]) -> Option<Hit> {
    if start
        .into_iter()
        .chain(direction)
        .chain(vertices.into_iter().flatten())
        .any(|v| !v.is_finite())
    {
        return None;
    }
    const LOW: f32 = FRACTION_LOW;
    const HIGH: f32 = FRACTION_HIGH;
    let [a, b, c] = vertices;
    let ab = sub(b, a);
    let ac = sub(c, a);
    let p = cross(direction, ac);
    let determinant = dot(ab, p);
    if !determinant.is_finite() || (determinant > LOW && determinant < -LOW) {
        return None;
    }
    let inverse = 1. / determinant;
    let origin = sub(start, a);
    let u = dot(origin, p) * inverse;
    if !(LOW..=HIGH).contains(&u) {
        return None;
    }
    let q = cross(origin, ab);
    let v = dot(direction, q) * inverse;
    if v < LOW || u + v > HIGH {
        return None;
    }
    let fraction = dot(ac, q) * inverse;
    if !(LOW..=HIGH).contains(&fraction) {
        return None;
    }
    Some(Hit { fraction, u, v })
}

#[cfg(test)]
mod tests {
    use super::*;
    const FLAT: [[f32; 3]; 3] = [[0., 0., 0.], [1., 0., 0.], [0., 0., 1.]];
    #[test]
    fn analytic_two_sided_hit_and_observed_edge_tolerance() {
        for direction in [[0., -2., 0.], [0., 2., 0.]] {
            let hit = query([0.25, -direction[1] * 0.5, 0.25], direction, FLAT).unwrap();
            assert_eq!(hit.fraction, 0.5);
            assert_eq!(hit.u, 0.25);
            assert_eq!(hit.v, 0.25);
        }
        assert!(query([-5e-7, 1., 0.5], [0., -2., 0.], FLAT).is_some());
        assert!(query([-2e-6, 1., 0.5], [0., -2., 0.], FLAT).is_none());
        assert!(
            query([0.25, -5e-7, 0.25], [0., -1., 0.], FLAT)
                .unwrap()
                .fraction
                < 0.
        );
    }
    #[test]
    fn absolute_parallel_threshold_and_invalid_inputs() {
        assert!(query([0.25, 1., 0.25], [0., -1e-7, 0.], FLAT).is_none());
        assert!(query([0.25, 0., 0.25], [1., 0., 0.], FLAT).is_none());
        assert!(query([f32::NAN, 1., 0.25], [0., -2., 0.], FLAT).is_none());
    }
}
