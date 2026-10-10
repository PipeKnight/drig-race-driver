//! Force-vector preparation at the actual earlier boundary, then source contacts/forces.
use crate::{
    Error, contact_ray, force_view,
    suspension_prepare::Contact,
    vehicle_contact_force::{self, Material},
    vehicle_force::Frame,
};

/// Selected Standard wheel matrices are the body's own basis. The caller owns
/// earlier controls/history, world material-table installation and later world scheduling.
pub fn apply(
    frame: &mut Frame,
    pair: [usize; 2],
    delta: f32,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    material: impl FnMut(u32) -> Result<Option<Material>, Error>,
) -> Result<[Option<Material>; 4], Error> {
    if !delta.is_finite() || delta <= 0. || delta > 0.1 {
        return Err(Error("invalid contact update delta"));
    }
    let mut next = frame.clone();
    let f = |o| f32::from_bits(next.body[o / 4]);
    let v = |o| std::array::from_fn(|i| f(o + i * 4));
    let scale = f(0x3d0);
    if !scale.is_finite() || scale <= 0. {
        return Err(Error("invalid suspension recovery scale"));
    }
    let matrix = std::array::from_fn(|i| v(0x15e0 + i * 16));
    let vectors = force_view::prepare(force_view::Input {
        body_axis_0: v(0x15e0),
        body_axis_2: v(0x1600),
        linear: v(0x15c0),
        angular: v(0x15d0),
        wheels: std::array::from_fn(|i| {
            let w = 0x1990 + i * 0x150;
            force_view::Wheel {
                coordinate: v(w),
                scalar_40: f(w + 0x40),
                scalar_e0: f(w + 0xe0),
                blend_20: f(w + 0x20),
                blend_24: f(w + 0x24),
                matrix,
            }
        }),
    })?;
    for (i, w) in vectors.into_iter().enumerate() {
        for (base, vector) in [
            (0x190, w.velocity),
            (0x1d0, w.longitudinal),
            (0x210, w.lateral),
            (0x250, w.lever),
        ] {
            for (j, x) in vector.into_iter().enumerate() {
                next.view[(base + i * 16 + j * 4) / 4] = x.to_bits();
            }
        }
    }
    let amount = delta * 0.25;
    let scale = if scale < 1. {
        (scale + amount).min(1.)
    } else {
        (scale - amount).max(1.)
    };
    next.body[0x3d0 / 4] = scale.to_bits();
    let materials = vehicle_contact_force::apply(&mut next, pair, delta, query, material)?;
    *frame = next;
    Ok(materials)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preparation_order_and_late_failure_preserve_all_owned_state() {
        let mut frame = crate::vehicle_contact_force::tests::failure_frame();
        frame.body[0x3d0 / 4] = 0.99999f32.to_bits();
        frame.view[0x190 / 4..0x290 / 4].fill(f32::NAN.to_bits());
        let before = frame.clone();
        let mut queries = 0;
        let error = apply(
            &mut frame,
            [2, 3],
            0.01,
            |_, _| {
                queries += 1;
                Ok(None)
            },
            |_| Ok(None),
        )
        .unwrap_err();
        assert_eq!(queries, 4);
        assert_eq!(error.0, "invalid selected control input");
        assert_eq!(frame.body, before.body);
        assert_eq!(frame.view, before.view);
        assert_eq!(frame.solver, before.solver);
        assert_eq!(frame.diagnostics, before.diagnostics);
        assert_eq!(frame.material_multipliers, before.material_multipliers);
    }
}
