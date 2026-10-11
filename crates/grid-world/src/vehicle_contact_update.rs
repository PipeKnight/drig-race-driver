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

/// Extend the selected contact/force interval through its following speed,
/// RPM, camber and assistance stages. Chassis-motion publication remains separate.
pub fn apply_with_post_force(
    frame: &mut Frame,
    pair: [usize; 2],
    delta: f32,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    material: impl FnMut(u32) -> Result<Option<Material>, Error>,
) -> Result<[Option<Material>; 4], Error> {
    let mut next = frame.clone();
    let materials = apply(&mut next, pair, delta, query, material)?;
    crate::vehicle_post_force::apply(&mut next.body, delta)?;
    *frame = next;
    Ok(materials)
}

/// Extend the selected source contact/force/assist interval through local
/// chassis-motion publication. The physical pose retains its existing schedule.
pub fn apply_with_chassis(
    frame: &mut Frame,
    pair: [usize; 2],
    delta: f32,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    material: impl FnMut(u32) -> Result<Option<Material>, Error>,
) -> Result<[Option<Material>; 4], Error> {
    let mut next = frame.clone();
    let materials = apply_with_post_force(&mut next, pair, delta, query, material)?;
    crate::vehicle_chassis::apply(&mut next.body, delta)?;
    *frame = next;
    Ok(materials)
}

/// Selected engaged assisted-automatic driver, including the earlier history,
/// gravity/aerodynamic interval and all source contact/force/chassis stages.
/// World synchronization, source initialization and respawn remain caller work.
pub fn apply_driver(
    frame: &mut Frame,
    pair: [usize; 2],
    delta: f32,
    gravity: f32,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    mut material: impl FnMut(u32) -> Result<Option<Material>, Error>,
) -> Result<[Option<Material>; 4], Error> {
    let mut next = frame.clone();
    let mut high = [false; 4];
    for (i, slot) in high.iter_mut().enumerate() {
        let token = next.body[(0x1990 + i * 0x150 + 0xc0) / 4];
        if token != 0 {
            *slot = material(token)?
                .ok_or(Error("missing previous wheel mechanics"))?
                .high_downforce;
        }
    }
    crate::vehicle_driver_prepare::apply(&mut next, delta, gravity, high)?;
    let materials = apply_with_chassis(&mut next, pair, delta, query, material)?;
    *frame = next;
    Ok(materials)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn valid_prior_frame() -> Frame {
        let mut frame = crate::vehicle_contact_force::tests::failure_frame();
        frame.body[0x1598 / 4] = 1;
        frame.body[0x196c / 4] = 1;
        frame.body[0xe48 / 4] = 10;
        for (o, x) in [
            (0x15a4, 0.001f32),
            (0x17a0, 0.22),
            (0x17a4, 900.),
            (0x17a8, 105.),
            (0x1f5c, 1.),
            (0x1f98, 3600.),
            (0x1fa0, 3600.),
            (0x125c, 1.),
            (0x1260, 1.),
            (0xe54, 0.4),
            (0x21d4, 1.6),
            (0x21d8, 2.6),
            (0x2298, 1.),
            (0x229c, 1.),
            (0x2274, 0.),
        ] {
            frame.body[o / 4] = x.to_bits();
        }
        for i in 1..=9 {
            frame.body[(0x18f4 + i * 4) / 4] = 0.5f32.to_bits();
        }
        for i in 0..4 {
            let w = 0x1990 + i * 0x150;
            for (o, x) in [
                (0, if i % 2 == 0 { -0.8 } else { 0.8 }),
                (8, if i < 2 { -1.3 } else { 1.3 }),
                (0xf0, 1f32),
                (0xf4, 1.),
                (0x100, 1.),
            ] {
                frame.body[(w + o) / 4] = x.to_bits();
            }
        }
        frame
    }
    fn valid_driver_frame() -> Frame {
        let mut frame = valid_prior_frame();
        frame.body[0x1970 / 4] = 0x101;
        for (o, x) in [(0x1ef0, 10f32), (0xe14, 0.5), (0x21c4, 1.2)] {
            frame.body[o / 4] = x.to_bits();
        }
        frame
    }
    #[test]
    fn driver_late_failure_rolls_back_gravity_contact_forces_and_chassis() {
        let mut frame = valid_driver_frame();
        frame.body[0x1494 / 4] = f32::NAN.to_bits();
        let before = frame.clone();
        let mut prepared = frame.clone();
        crate::vehicle_driver_prepare::apply(&mut prepared, 0.01, 9.81, [false; 4]).unwrap();
        assert_ne!(prepared.body[0x15c4 / 4], before.body[0x15c4 / 4]);
        let mut queries = 0;
        let error = apply_driver(
            &mut frame,
            [2, 3],
            0.01,
            9.81,
            |_, _| {
                queries += 1;
                Ok(None)
            },
            |_| Ok(None),
        )
        .unwrap_err();
        assert_eq!(queries, 4);
        assert_eq!(error.0, "invalid chassis scalar or span");
        assert_eq!(frame.body, before.body);
        assert_eq!(frame.view, before.view);
        assert_eq!(frame.solver, before.solver);
        assert_eq!(frame.material_multipliers, before.material_multipliers);
        assert_eq!(frame.diagnostics, before.diagnostics);
        assert_eq!(frame.driven, before.driven);
    }
    #[test]
    fn driver_periodic_overflow_rolls_back_after_gravity_and_history_writes() {
        let mut frame = valid_driver_frame();
        frame.body[0x300 / 4] = 1; // Positive subnormal; reciprocal overflows later.
        let before = frame.clone();
        let error =
            crate::vehicle_driver_prepare::apply(&mut frame, 0.01, 9.81, [false; 4]).unwrap_err();
        assert_eq!(error.0, "nonfinite driver preparation output");
        assert_eq!(frame.body, before.body);
        assert_eq!(frame.view, before.view);
        assert_eq!(frame.solver, before.solver);
        assert_eq!(frame.material_multipliers, before.material_multipliers);
        assert_eq!(frame.diagnostics, before.diagnostics);
    }
    #[test]
    fn chassis_failure_preserves_completed_contact_controls_assists_and_solver() {
        let mut frame = valid_prior_frame();
        frame.body[0x1494 / 4] = f32::NAN.to_bits();
        let before = frame.clone();
        let mut queries = 0;
        let error = apply_with_chassis(
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
        assert_eq!(error.0, "invalid chassis scalar or span");
        assert_eq!(frame.body, before.body);
        assert_eq!(frame.view, before.view);
        assert_eq!(frame.solver, before.solver);
        assert_eq!(frame.material_multipliers, before.material_multipliers);
        assert_eq!(frame.diagnostics, before.diagnostics);
    }
    #[test]
    fn post_force_failure_preserves_completed_contact_controls_and_solver() {
        let mut frame = valid_prior_frame();
        frame.body[0x2274 / 4] = f32::NAN.to_bits();
        let before = frame.clone();
        let mut queries = 0;
        let error = apply_with_post_force(
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
        assert_eq!(error.0, "invalid post-force scalar");
        assert_eq!(frame.body, before.body);
        assert_eq!(frame.view, before.view);
        assert_eq!(frame.solver, before.solver);
        assert_eq!(frame.material_multipliers, before.material_multipliers);
        assert_eq!(frame.diagnostics, before.diagnostics);
    }
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
