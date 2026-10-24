//! Diagnostic geometry fixture with authored axes and attachment, independent of the source clock.
//! Reuses source signed corners/UV and extent/opacity endpoints; normalization is authored.
#[derive(Clone, Copy, Debug)]
pub struct Anchor {
    pub source_index: u32,
    pub source_cell: u32,
    pub position: [f32; 3],
}
#[derive(Clone, Copy, Debug)]
pub struct Quad {
    pub positions: [[f32; 3]; 4],
    pub uv: [[f32; 2]; 4],
    pub opacity: f32,
}
fn finite(v: [f32; 3]) -> bool {
    v.into_iter().all(f32::is_finite)
}
fn unit(v: [f32; 3]) -> Result<[f32; 3], &'static str> {
    if !finite(v) {
        return Err("nonfinite diagnostic axis");
    }
    let len = v
        .into_iter()
        .map(|x| f64::from(x) * f64::from(x))
        .sum::<f64>()
        .sqrt();
    if !len.is_finite() || len < 1e-6 {
        return Err("degenerate diagnostic axis");
    }
    let out = v.map(|x| (f64::from(x) / len) as f32);
    if !finite(out) {
        return Err("nonfinite normalized diagnostic axis");
    };
    Ok(out)
}
pub fn quad(anchor: Anchor, eye: [f32; 3], up: [f32; 3], phase: f32) -> Result<Quad, &'static str> {
    if !finite(anchor.position)
        || !finite(eye)
        || !finite(up)
        || !phase.is_finite()
        || !(0. ..1.).contains(&phase)
    {
        return Err("unsupported flash diagnostic input");
    }
    let up_len = up
        .into_iter()
        .map(|v| f64::from(v) * f64::from(v))
        .sum::<f64>();
    if (up_len - 1.).abs() > 1e-4 {
        return Err("diagnostic U must be explicit unit XYZ/w0");
    }
    let delta = std::array::from_fn(|i| eye[i] - anchor.position[i]);
    let direction = unit(delta)?;
    let side = unit([
        direction[1] * up[2] - direction[2] * up[1],
        direction[2] * up[0] - direction[0] * up[2],
        direction[0] * up[1] - direction[1] * up[0],
    ])?;
    // Source endpoints, no extra0.5dimension factor/gain/fade-to-zero.
    let extent = 1. + (0.5 - 1.) * phase;
    let opacity = 1. + (0.5 - 1.) * phase;
    let mut positions = [[0.; 3]; 4];
    for (dst, (u, s)) in positions
        .iter_mut()
        .zip([(1., -1.), (1., 1.), (-1., -1.), (-1., 1.)])
    {
        *dst =
            std::array::from_fn(|i| anchor.position[i] + extent * up[i] * u + extent * side[i] * s);
        if !finite(*dst) {
            return Err("nonfinite diagnostic flash corner");
        }
    }
    Ok(Quad {
        positions,
        uv: [[1., 1.], [1., 0.], [0., 1.], [0., 0.]],
        opacity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn anchor() -> Anchor {
        Anchor {
            source_index: 7,
            source_cell: 3,
            position: [0.; 3],
        }
    }
    #[test]
    fn signed_corner_uv_and_full_extent_contract() {
        let q = quad(anchor(), [0., 0., 10.], [0., 1., 0.], 0.).unwrap();
        assert_eq!(
            q.positions,
            [[1., 1., 0.], [-1., 1., 0.], [1., -1., 0.], [-1., -1., 0.]]
        );
        assert_eq!(q.uv, [[1., 1.], [1., 0.], [0., 1.], [0., 0.]]);
        assert_eq!(q.opacity, 1.);
        let half = quad(anchor(), [0., 0., 10.], [0., 1., 0.], 0.5).unwrap();
        assert_eq!(half.positions[0], [0.75, 0.75, 0.]);
        assert_eq!(half.opacity, 0.75);
    }
    #[test]
    fn nonfinite_degenerate_axis_and_expired_phase_reject() {
        for phase in [f32::NAN, f32::INFINITY, -0.1, 1.] {
            assert!(quad(anchor(), [0., 0., 10.], [0., 1., 0.], phase).is_err());
        }
        for (eye, up) in [
            ([0., 0., 0.], [0., 1., 0.]),
            ([0., 10., 0.], [0., 1., 0.]),
            ([0., 0., 10.], [0., 2., 0.]),
            ([f32::NAN, 0., 10.], [0., 1., 0.]),
        ] {
            assert!(quad(anchor(), eye, up, 0.5).is_err());
        }
        let mut a = anchor();
        a.position = [-f32::MAX, 0., 0.];
        assert!(quad(a, [f32::MAX, 0., 0.], [0., 1., 0.], 0.5).is_err());
    }
}
