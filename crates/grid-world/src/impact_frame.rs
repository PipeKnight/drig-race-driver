//! Whole outer impact-only frame with no registered callbacks, joints or links.
//! Includes body preparation and final damping/clamping/pending-shift publication.
use crate::{
    Error,
    impact_step::{self, Input as Step, Output as StepOutput},
    physical_pose::{self, Matrix},
};
#[derive(Clone)]
pub struct Body {
    pub basis: Matrix,
    pub local_matrices: [Matrix; 2],
    pub offset: [f32; 4],
    /// Timer, linear/angular/force/torque squared thresholds, sleep duration.
    pub sleep: [f32; 6],
    /// Three local angular limits, then world linear magnitude limit.
    pub limits: [f32; 4],
    /// Three local angular rates, then world linear deceleration.
    pub damping: [f32; 4],
    /// Positive XYZ and negative XYZ quadratic drag coefficients.
    pub drag: [f32; 6],
    pub mass: f32,
    pub gravity_scale: f32,
    pub pending_shift: [f32; 4],
    pub shift_active: bool,
}
#[derive(Clone)]
pub struct Input {
    /// Step scalars1..4 are supplied by the outer frame's mass normalization.
    pub step: Step,
    pub gravity: [f32; 4],
    pub bodies: Vec<Body>,
}
pub struct Output {
    pub status: u32,
    pub started: bool,
    pub step: StepOutput,
    pub bodies: Vec<Body>,
    /// None means original published/derived matrices are retained.
    pub publications: Vec<Option<physical_pose::Output>>,
}
fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[0] * b[0] + a[1] * b[1]) + (a[2] * b[2] + a[3] * b[3])
}
fn transform(v: [f32; 4], m: Matrix) -> [f32; 4] {
    std::array::from_fn(|i| (v[3] * m[3][i] + v[1] * m[1][i]) + (v[2] * m[2][i] + v[0] * m[0][i]))
}
fn local(v: [f32; 4], m: Matrix) -> [f32; 4] {
    [dot(v, m[0]), dot(v, m[1]), dot(v, m[2]), 0.]
}
fn asleep(b: &mut Body, s: &impact_step::Body, delta: f32) -> bool {
    let reset = b.sleep[5] == f32::MAX
        || b.sleep[2] <= dot(s.contact.angular, s.contact.angular)
        || b.sleep[1] <= dot(s.contact.linear, s.contact.linear)
        || (s.apply_force && dot(s.force, s.force) > b.sleep[3])
        || (s.apply_torque && dot(s.torque, s.torque) > b.sleep[4]);
    if reset {
        b.sleep[0] = 0.;
        false
    } else {
        b.sleep[0] += delta;
        if b.sleep[0] >= b.sleep[5] {
            b.sleep[0] = b.sleep[5];
            true
        } else {
            false
        }
    }
}
#[allow(clippy::assign_op_pattern)] // Preserve original force and damping operand order.
pub fn calculate(input: &Input) -> Result<Output, Error> {
    let delta = input.step.parameters[0];
    if input.bodies.len() != input.step.bodies.len()
        || input.bodies.is_empty()
        || input.bodies.len() > 16
        || !delta.is_finite()
        || delta <= 0.
        || delta > 0.25
        || input.gravity.iter().any(|v| !v.is_finite())
        || input
            .bodies
            .iter()
            .zip(&input.step.bodies)
            .any(|(b, s)| b.basis[3].map(f32::to_bits) != s.contact.position.map(f32::to_bits))
        || input.bodies.iter().any(|b| {
            b.basis
                .iter()
                .flatten()
                .chain(b.local_matrices.iter().flatten().flatten())
                .chain(&b.offset)
                .chain(&b.sleep)
                .chain(&b.limits)
                .chain(&b.damping)
                .chain(&b.drag)
                .chain(&b.pending_shift)
                .chain([b.mass, b.gravity_scale].iter())
                .any(|v| !v.is_finite())
        })
    {
        return Err(Error("invalid impact frame input"));
    }
    let s = &input.step;
    impact_step::validate(s)?;
    let mut out = Output {
        status: 0,
        started: false,
        bodies: input.bodies.clone(),
        publications: (0..input.bodies.len()).map(|_| None).collect(),
        step: StepOutput {
            success: true,
            ends: s.initial_ends,
            bodies: s.bodies.clone(),
            contacts: s.contacts.clone(),
            metadata: vec![[0; 2]; s.contacts.len()],
            rows: s.rows.clone(),
            workspace: s.workspace.clone(),
            cached_pair: s.cached_pair,
            cached_material: s.cached_material,
        },
    };
    if s.iterations == 0 {
        return Ok(out);
    }
    let mut sleeping = 0;
    let mut minimum = f32::MAX;
    for (b, s) in out.bodies.iter_mut().zip(&mut out.step.bodies) {
        sleeping += usize::from(asleep(b, s, delta));
        if b.gravity_scale != 0. {
            let scale = b.mass * b.gravity_scale;
            let added = input.gravity.map(|v| scale * v);
            for (force, v) in s.force.iter_mut().zip(added) {
                *force = v + *force;
            }
            s.apply_force = true;
        }
        if b.drag.iter().any(|&v| v > 0.) {
            let v = local(s.contact.linear, b.basis);
            let mut drag = [0.; 4];
            for i in 0..3 {
                let coefficient = b.drag[i + if v[i] < 0. { 3 } else { 0 }];
                if coefficient > 0. {
                    let scale = s.force_inverse_mass * delta;
                    let mut force = (v[i] * v[i]) * coefficient;
                    if v[i].abs() < scale * force {
                        force = v[i].abs() / scale;
                    }
                    drag[i] = if v[i] >= 0. { -0. - force } else { force };
                }
            }
            let added = transform(drag, b.basis);
            for (force, v) in s.force.iter_mut().zip(added) {
                *force = v + *force;
            }
            s.apply_force = true;
        }
        s.impulse_accumulator = [0.; 4];
        s.impulse_active = false;
        s.inverse_mass = s.force_inverse_mass;
        s.inverse_inertia = s.torque_matrix;
        if s.force_inverse_mass > 0. && s.force_inverse_mass < minimum {
            minimum = s.force_inverse_mass;
        }
    }
    if out.step.bodies.iter().any(|s| {
        s.force
            .iter()
            .chain(&s.torque)
            .chain(s.inverse_inertia.iter().flatten())
            .chain([s.inverse_mass].iter())
            .any(|v| !v.is_finite())
    }) {
        return Err(Error("nonfinite prepared impact body"));
    }
    if sleeping == out.bodies.len() {
        out.status = 1;
        return Ok(out);
    }
    let inverse = 1. / minimum;
    for s in &mut out.step.bodies {
        s.inverse_mass *= inverse;
        for v in s.inverse_inertia.iter_mut().flatten() {
            *v *= inverse;
        }
    }
    let mut step = input.step.clone();
    step.bodies = out.step.bodies;
    step.parameters[1] = 1.;
    step.parameters[2] = 1.;
    step.parameters[3] = minimum;
    step.parameters[4] = inverse;
    out.step = impact_step::calculate(&step)?;
    out.started = true;
    if !out.step.success {
        out.status = 1;
        return Ok(out);
    }
    for (i, (b, s)) in out.bodies.iter_mut().zip(&mut out.step.bodies).enumerate() {
        s.force = [0.; 4];
        s.torque = [0.; 4];
        s.apply_force = false;
        s.apply_torque = false;
        if b.damping[3] > 0. {
            let correction = s.contact.linear.map(|v| -v);
            let squared = dot(correction, correction);
            let change = delta * b.damping[3];
            let correction = if change * change < squared {
                let factor = change / squared.sqrt();
                correction.map(|v| factor * v)
            } else {
                correction
            };
            for (v, c) in s.contact.linear.iter_mut().zip(correction) {
                *v = *v + c;
            }
        }
        if b.damping[..3].iter().any(|&v| v > 0.) {
            let mut v = local(s.contact.angular, b.basis);
            for (i, value) in v.iter_mut().enumerate().take(3) {
                if b.damping[i] > 0. {
                    let change = b.damping[i] * delta;
                    let mut correction = -0. - *value;
                    if change < correction.abs() {
                        correction = if correction <= 0. {
                            -0. - change
                        } else {
                            change
                        };
                    }
                    *value = correction + *value;
                }
            }
            s.contact.angular = transform(v, b.basis);
        }
        if b.limits[3] < f32::MAX {
            let squared = dot(s.contact.linear, s.contact.linear);
            let limit = b.limits[3];
            if limit * limit < squared {
                let factor = limit / squared.sqrt();
                s.contact.linear = s.contact.linear.map(|v| factor * v);
            }
        }
        if b.limits[..3].iter().any(|&v| v < f32::MAX) {
            let mut v = local(s.contact.angular, b.basis);
            for (i, value) in v.iter_mut().enumerate().take(3) {
                if b.limits[i] < f32::MAX && b.limits[i] < value.abs() {
                    *value = if *value <= 0. {
                        -0. - b.limits[i]
                    } else {
                        b.limits[i]
                    };
                }
            }
            s.contact.angular = transform(v, b.basis);
        }
        if b.shift_active {
            b.basis[3] = s.contact.position;
            let publication = physical_pose::calculate(&physical_pose::Input {
                basis: b.basis,
                offset: b.offset,
                local_matrices: b.local_matrices,
                group_head: false,
                shapes: vec![],
                linear: b.pending_shift,
                angular: [0.; 4],
            })?;
            b.basis = publication.corrected;
            s.contact.position = b.basis[3];
            s.torque_matrix = publication.derived[1];
            out.publications[i] = Some(publication);
            b.pending_shift = [0.; 4];
            b.shift_active = false;
        }
    }
    if out.step.bodies.iter().any(|s| {
        s.contact
            .linear
            .iter()
            .chain(&s.contact.angular)
            .any(|v| !v.is_finite())
    }) {
        return Err(Error("nonfinite impact frame output"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    const IDENTITY: Matrix = [
        [1., 0., 0., 0.],
        [0., 1., 0., 0.],
        [0., 0., 1., 0.],
        [0., 0., 0., 1.],
    ];
    fn input() -> Input {
        Input {
            gravity: [0., -10., 0., 0.],
            bodies: vec![Body {
                basis: IDENTITY,
                local_matrices: [IDENTITY; 2],
                offset: [0.; 4],
                sleep: [0., 0., 0., 0., 0., f32::MAX],
                limits: [f32::MAX; 4],
                damping: [0.; 4],
                drag: [0.; 6],
                mass: 2.,
                gravity_scale: 1.,
                pending_shift: [0.; 4],
                shift_active: false,
            }],
            step: Step {
                parameters: [0.1, 1., 1., 1., 1., 0.01, 0.2, 0.125, 0.8, 0.2, 0.2],
                bodies: vec![impact_step::Body {
                    contact: crate::contact_rows::Body {
                        position: [0., 0., 0., 1.],
                        angular: [0.; 4],
                        linear: [0.; 4],
                        scale_angular: false,
                        directional_friction: false,
                    },
                    inverse_mass: 7.,
                    inverse_inertia: IDENTITY,
                    force_inverse_mass: 0.5,
                    torque_matrix: IDENTITY,
                    impulse_accumulator: [3.; 4],
                    torque: [0.; 4],
                    force: [0.; 4],
                    impulse_active: true,
                    apply_torque: false,
                    apply_force: false,
                }],
                materials: vec![[0.; 13]],
                contacts: vec![],
                rows: vec![[0; 48]],
                capacity: 1,
                iterations: 1,
                initial_ends: [0; 4],
                workspace: vec![[[7.; 4]; 2]],
                cached_pair: None,
                cached_material: [0.; 13],
            },
        }
    }
    #[test]
    fn empty_awake_frame_integrates_gravity_and_consumes_forces() {
        let o = calculate(&input()).unwrap();
        assert_eq!(o.status, 0);
        assert!(o.started);
        assert_eq!(o.step.bodies[0].contact.linear, [0., -1., 0., 0.]);
        assert_eq!(o.step.bodies[0].force, [0.; 4]);
        assert!(!o.step.bodies[0].apply_force);
        assert!(!o.step.bodies[0].impulse_active);
        assert_eq!(o.step.bodies[0].inverse_mass, 1.);
    }
    #[test]
    fn zero_iterations_bypass_preparation_and_cleanup() {
        let mut input = input();
        input.step.iterations = 0;
        let o = calculate(&input).unwrap();
        assert!(!o.started);
        assert_eq!(o.status, 0);
        assert_eq!(o.step.bodies[0].impulse_accumulator, [3.; 4]);
        assert_eq!(o.step.bodies[0].inverse_mass, 7.);
        assert_eq!(o.step.workspace, input.step.workspace);
    }
    #[test]
    fn sleeping_frame_retains_prepared_gravity_forces_without_solving() {
        let mut input = input();
        input.bodies[0].sleep = [0., 1., 1., 1., 1., 0.];
        let o = calculate(&input).unwrap();
        assert_eq!(o.status, 1);
        assert!(!o.started);
        assert_eq!(o.step.bodies[0].force, [0., -20., 0., 0.]);
        assert!(o.step.bodies[0].apply_force);
        assert_eq!(o.step.bodies[0].contact.linear, [0.; 4]);
        assert_eq!(o.step.bodies[0].inverse_mass, 0.5);
    }
    #[test]
    fn inconsistent_pose_positions_are_rejected() {
        let mut input = input();
        input.bodies[0].basis[3][0] = 1.;
        assert!(calculate(&input).is_err());
    }
}
