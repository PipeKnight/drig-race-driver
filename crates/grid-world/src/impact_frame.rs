//! Whole outer impact-only frame; joints and links remain external.
//! Includes body preparation and final damping/clamping/pending-shift publication.
use crate::{
    Error, impact_correction,
    impact_step::{self, Input as Step, Output as StepOutput},
    main_contact_response,
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
pub struct Correction {
    pub iterations: usize,
    pub softness: f32,
    pub scale: f32,
    pub threshold: f32,
    pub relaxation: f32,
}
#[derive(Clone)]
pub struct Input {
    /// Step scalars1..4 are supplied by the outer frame's mass normalization.
    pub step: Step,
    pub gravity: [f32; 4],
    pub bodies: Vec<Body>,
    pub correction: Option<Correction>,
    /// B7B850's parking test: set when parking is enabled (frame +0x11C)
    /// and the island has a one-body or static contact.
    pub park: Option<Park>,
}

/// The park reference (frame +0x120 point, +0x130 scale) and each body's
/// squared parking radius (+0x3FC), in island order.
#[derive(Clone)]
pub struct Park {
    pub point: [f32; 4],
    pub scale: f32,
    pub radii: Vec<f32>,
}

/// B106E0: whether a body at `position` (the vector body +0x334 points
/// to, +0x1C0) is outside its parking radius: (scale * scale) times the
/// squared distance (four lanes, the shuffle-sum order of `dot`) above the
/// squared radius; equal or unordered is in range.
pub fn out_of_range(position: [f32; 4], point: [f32; 4], scale: f32, radius: f32) -> bool {
    let d = std::array::from_fn(|i| point[i] - position[i]);
    (scale * scale) * dot(d, d) > radius
}

pub struct Output {
    pub status: u32,
    pub started: bool,
    pub step: StepOutput,
    pub bodies: Vec<Body>,
    /// None means original published/derived matrices are retained.
    pub publications: Vec<Option<physical_pose::Output>>,
    pub correction_invoked: bool,
    pub correction_active: bool,
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
pub(crate) fn validate(input: &Input) -> Result<(), Error> {
    validate_rows(input, &input.step.rows)
}

/// [`validate`] with the row buffer the step will use.
fn validate_rows(input: &Input, rows: &[crate::contact_rows::Row]) -> Result<(), Error> {
    let delta = input.step.parameters[0];
    if input.bodies.len() != input.step.bodies.len()
        || input.correction.as_ref().is_some_and(|c| {
            c.iterations > 32
                || [c.softness, c.scale, c.threshold, c.relaxation]
                    .iter()
                    .any(|v| !v.is_finite())
        })
        || input.bodies.is_empty()
        || input.bodies.len() > crate::MAX_WORLD_BODIES
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
    impact_step::validate_rows(&input.step, rows)
}

pub fn calculate(input: &Input) -> Result<Output, Error> {
    calculate_with_callbacks(input, &NoCallbacks).map(|(out, _)| out)
}

/// Internal owned callback context. Clone before either phase so a subsequent
/// solver/publication error cannot commit callback state to the caller.
pub(crate) trait Callbacks: Clone {
    fn prepare(&mut self, _step: &mut Step, _bodies: &mut [Body]) -> Result<(), Error> {
        Ok(())
    }
    fn solved(&mut self, _step: &mut StepOutput, _bodies: &mut [Body]) -> Result<(), Error> {
        Ok(())
    }
}
#[derive(Clone)]
struct NoCallbacks;
impl Callbacks for NoCallbacks {}

pub(crate) fn calculate_with_callbacks<C: Callbacks>(
    input: &Input,
    callbacks: &C,
) -> Result<(Output, C), Error> {
    validate(input)?;
    run(input, None, callbacks)
}

/// [`calculate_with_callbacks`] with the shared row buffer given away by
/// the caller (`input.step.rows` is then unused), so it moves through the
/// step instead of being copied; `None` copies `input.step.rows`.
pub(crate) fn calculate_with_callbacks_rows<C: Callbacks>(
    input: &Input,
    rows: Option<Vec<crate::contact_rows::Row>>,
    callbacks: &C,
) -> Result<(Output, C), Error> {
    match rows {
        Some(rows) => {
            validate_rows(input, &rows)?;
            run(input, Some(rows), callbacks)
        }
        None => calculate_with_callbacks(input, callbacks),
    }
}

/// [`calculate`] consuming the input, so the shared row buffer moves through
/// the step instead of being copied (one call per contact group per tick).
pub(crate) fn calculate_owned(mut input: Input) -> Result<Output, Error> {
    validate(&input)?;
    let rows = std::mem::take(&mut input.step.rows);
    run(&input, Some(rows), &NoCallbacks).map(|(out, _)| out)
}

/// `rows` replaces `input.step.rows` when the caller gave the buffer away.
#[allow(clippy::assign_op_pattern)] // Preserve original force and damping operand order.
fn run<C: Callbacks>(
    input: &Input,
    mut rows: Option<Vec<crate::contact_rows::Row>>,
    callbacks: &C,
) -> Result<(Output, C), Error> {
    let mut callbacks = callbacks.clone();
    let delta = input.step.parameters[0];
    let s = &input.step;
    let mut out = Output {
        status: 0,
        started: false,
        bodies: input.bodies.clone(),
        publications: (0..input.bodies.len()).map(|_| None).collect(),
        correction_invoked: false,
        correction_active: false,
        step: StepOutput {
            success: true,
            ends: s.initial_ends,
            bodies: s.bodies.clone(),
            contacts: s.contacts.clone(),
            metadata: vec![[0; 2]; s.contacts.len()],
            rows: Vec::new(),
            workspace: s.workspace.clone(),
            cached_pair: s.cached_pair,
            cached_material: s.cached_material,
            joints: s.joints.clone(),
        },
    };
    if s.iterations == 0 {
        out.step.rows = rows.take().unwrap_or_else(|| s.rows.clone());
        return Ok((out, callbacks));
    }
    let mut sleeping = 0;
    let mut outside = 0;
    let mut minimum = f32::MAX;
    for (k, (b, s)) in out.bodies.iter_mut().zip(&mut out.step.bodies).enumerate() {
        sleeping += usize::from(asleep(b, s, delta));
        if let Some(park) = &input.park {
            // AUTHORED (owner's request of 23 October 2026, BODY_SLEEPING.md
            // section 8): B7B850 counts a body outside on range alone, so an
            // island far from the player parked in flight on its first wall
            // contact and hung there. A body counts only once its sleep
            // timer has reached turnOffTime, i.e. it has rested as long as
            // sleeping needs; a body in flight cannot stay under its
            // stationary thresholds that long.
            outside += usize::from(
                b.sleep[0] >= b.sleep[5]
                    && out_of_range(b.basis[3], park.point, park.scale, park.radii[k]),
            );
        }
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
        out.step.rows = rows.take().unwrap_or_else(|| s.rows.clone());
        return Ok((out, callbacks));
    }
    // Every body outside its radius: status 2, the island parks (B24DF0).
    if input.park.is_some() && outside == out.bodies.len() {
        out.status = 2;
        out.step.rows = rows.take().unwrap_or_else(|| s.rows.clone());
        return Ok((out, callbacks));
    }
    let inverse = 1. / minimum;
    for s in &mut out.step.bodies {
        s.inverse_mass *= inverse;
        for v in s.inverse_inertia.iter_mut().flatten() {
            *v *= inverse;
        }
    }
    let mut step = s.with_rows(rows.take().unwrap_or_else(|| s.rows.clone()));
    step.bodies = out.step.bodies;
    step.parameters[1] = 1.;
    step.parameters[2] = 1.;
    step.parameters[3] = minimum;
    step.parameters[4] = inverse;
    callbacks.prepare(&mut step, &mut out.bodies)?;
    out.step = impact_step::calculate_owned(step)?;
    out.started = true;
    if !out.step.success {
        out.status = 1;
        return Ok((out, callbacks));
    }
    callbacks.solved(&mut out.step, &mut out.bodies)?;
    // The file-backed D85A50 threshold is positive zero. Preserve the outer
    // iteration/nonpositive-relaxation guard before entering the optional pass.
    if let Some(c) = input
        .correction
        .as_ref()
        .filter(|c| c.iterations != 0 && c.relaxation > 0.)
    {
        out.correction_invoked = true;
        let contacts = |two| {
            out.step
                .contacts
                .iter()
                .filter(|v| v.two_bodies == two)
                .map(|v| impact_correction::Contact {
                    disabled: v.disabled,
                    accumulated: v.accumulated,
                })
                .collect()
        };
        let poses = out
            .bodies
            .iter()
            .zip(&out.step.bodies)
            .map(|(b, s)| {
                let mut basis = b.basis;
                basis[3] = s.contact.position;
                physical_pose::Input {
                    basis,
                    offset: b.offset,
                    local_matrices: b.local_matrices,
                    group_head: false,
                    shapes: vec![],
                    linear: [0.; 4],
                    angular: [0.; 4],
                }
            })
            .collect();
        // Empty contact pools retain the manager's previous span ends. Those
        // ends need not describe the current row allocation on this bypass.
        let joints = |two| {
            out.step
                .joints
                .list
                .iter()
                .filter(|j| j.two_body() == two)
                .map(|j| j.joint.clone())
                .collect::<Vec<_>>()
        };
        let (two_body_joints, single_body_joints) = (joints(true), joints(false));
        let n = out.step.contacts.len()
            + two_body_joints
                .iter()
                .chain(&single_body_joints)
                .map(|j| j.words[0x18 / 4] as usize)
                .sum::<usize>();
        let result = impact_correction::calculate(&impact_correction::Input {
            rows: out.step.rows[..n]
                .iter()
                .map(main_contact_response::typed)
                .collect(),
            two_body_contacts: contacts(true),
            single_body_contacts: contacts(false),
            two_body_joints,
            single_body_joints,
            poses,
            workspace: out.step.workspace.clone(),
            span_ends: [out.step.ends[3], out.step.ends[2]],
            iterations: c.iterations,
            softness: c.softness,
            scale: c.scale,
            threshold: c.threshold,
            relaxation: c.relaxation,
        })?;
        out.correction_active = result.active;
        out.step.workspace = result.workspace;
        out.step.ends[3] = result.span_ends[0];
        out.step.ends[2] = result.span_ends[1];
        for (raw, row) in out.step.rows[..n].iter_mut().zip(result.rows) {
            for (i, v) in row
                .jacobian
                .iter()
                .chain(&row.response)
                .flatten()
                .enumerate()
            {
                raw[i] = v.to_bits();
            }
            if result.rows_reset {
                raw[32] = 0;
            }
            for (i, v) in [
                (33, row.softness_input),
                (34, row.lower),
                (35, row.upper),
                (36, row.bias),
                (37, row.impulse),
                (38, row.cumulative),
                (39, row.softness),
            ] {
                raw[i] = v.to_bits();
            }
        }
        for (i, pose) in result.poses.into_iter().enumerate() {
            if let Some(pose) = pose {
                out.bodies[i].basis = pose.corrected;
                out.step.bodies[i].contact.position = pose.corrected[3];
                out.step.bodies[i].torque_matrix = pose.derived[1];
                out.publications[i] = Some(pose);
            }
        }
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
    Ok((out, callbacks))
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
            park: None,
            correction: None,
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
                joints: Default::default(),
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
    #[test]
    fn empty_correction_retains_stale_spans_without_indexing_rows() {
        let mut input = input();
        input.step.initial_ends = [3, 7, 11, 13];
        input.correction = Some(Correction {
            iterations: 2,
            softness: 0.1,
            scale: 0.001,
            threshold: 0.,
            relaxation: 1.,
        });
        let o = calculate(&input).unwrap();
        assert!(o.correction_invoked);
        assert!(!o.correction_active);
        assert_eq!(o.step.ends, input.step.initial_ends);
        assert_eq!(o.step.rows, input.step.rows);
        assert_eq!(o.step.workspace, input.step.workspace);
        assert!(o.publications[0].is_none());
    }
    #[test]
    fn outer_correction_guards_skip_the_pass() {
        for (iterations, relaxation) in [(0, 1.), (2, 0.), (2, -1.)] {
            let mut input = input();
            input.correction = Some(Correction {
                iterations,
                softness: 0.1,
                scale: 0.001,
                threshold: 0.,
                relaxation,
            });
            let o = calculate(&input).unwrap();
            assert!(!o.correction_invoked);
            assert!(!o.correction_active);
            assert!(o.publications[0].is_none());
        }
    }
    #[test]
    fn physical_tick_advances_before_zero_iteration_bypass() {
        let mut input = input();
        input.step.iterations = 0;
        input.step.bodies[0].contact.linear = [2., 0., 0., 0.];
        let o = crate::physical_tick::calculate(&input).unwrap();
        assert!(!o.frame.started);
        assert_eq!(o.advanced[0].corrected[3], [0.2, 0., 0., 1.]);
        assert_eq!(o.frame.step.bodies[0].contact.position, [0.2, 0., 0., 1.]);
        assert_eq!(o.frame.step.bodies[0].contact.linear, [2., 0., 0., 0.]);
    }
    #[test]
    fn physical_tick_uses_incoming_velocity_before_new_gravity() {
        let mut input = input();
        input.step.bodies[0].contact.linear = [1., 0., 0., 0.];
        let o = crate::physical_tick::calculate(&input).unwrap();
        assert!(o.frame.started);
        assert_eq!(o.advanced[0].corrected[3], [0.1, 0., 0., 1.]);
        assert_eq!(o.frame.step.bodies[0].contact.position, [0.1, 0., 0., 1.]);
        assert_eq!(o.frame.step.bodies[0].contact.linear, [1., -1., 0., 0.]);
    }
    #[test]
    fn owned_callbacks_observe_solver_phases_and_roll_back_on_late_publication_failure() {
        #[derive(Clone)]
        struct Context {
            phases: Vec<u8>,
            overflow: bool,
        }
        impl Callbacks for Context {
            fn prepare(&mut self, step: &mut Step, _: &mut [Body]) -> Result<(), Error> {
                assert_eq!(step.bodies[0].force, [0., -20., 0., 0.]);
                assert_eq!(step.bodies[0].contact.linear, [0.; 4]);
                assert_eq!(step.bodies[0].inverse_mass, 1.);
                self.phases.push(1);
                Ok(())
            }
            fn solved(&mut self, step: &mut StepOutput, bodies: &mut [Body]) -> Result<(), Error> {
                assert_eq!(step.bodies[0].contact.linear, [0., -1., 0., 0.]);
                bodies[0].pending_shift = if self.overflow {
                    [f32::MAX, 0., 0., 0.]
                } else {
                    [0., 0.5, 0., 0.]
                };
                bodies[0].shift_active = true;
                self.phases.push(2);
                Ok(())
            }
        }
        let context = Context {
            phases: vec![],
            overflow: false,
        };
        let (out, committed) = calculate_with_callbacks(&input(), &context).unwrap();
        assert_eq!(committed.phases, vec![1, 2]);
        assert!(context.phases.is_empty());
        assert_eq!(out.publications[0].as_ref().unwrap().published[3][1], 0.5);
        assert!(!out.bodies[0].shift_active);
        let mut bad = input();
        bad.bodies[0].basis[3][0] = f32::MAX;
        bad.step.bodies[0].contact.position[0] = f32::MAX;
        let context = Context {
            overflow: true,
            ..context
        };
        assert!(calculate_with_callbacks(&bad, &context).is_err());
        assert!(context.phases.is_empty());
        assert_eq!(bad.bodies[0].pending_shift, [0.; 4]);
        assert_eq!(bad.step.bodies[0].contact.linear, [0.; 4]);
    }
    #[test]
    fn serial_world_reorders_advances_without_reindexing_bodies_or_publishing_failure() {
        let mut input = input();
        input.step.iterations = 0;
        input.bodies = vec![input.bodies[0].clone(); 3];
        input.step.bodies = vec![input.step.bodies[0].clone(); 3];
        input.step.workspace = vec![input.step.workspace[0]; 3];
        for (i, body) in input.step.bodies.iter_mut().enumerate() {
            body.contact.linear[0] = (i + 1) as f32;
        }
        let links = [Some(2), None, Some(1)];
        let out = crate::physical_tick::calculate_serial_world(&input, &links, 0).unwrap();
        assert_eq!(out.update_head, 1);
        assert_eq!(out.update_next, [None, Some(2), Some(0)]);
        for (i, publication) in out.tick.advanced.iter().enumerate() {
            assert_eq!(publication.corrected[3][0], (i + 1) as f32 * 0.1);
            assert_eq!(input.step.bodies[i].contact.position[0], 0.);
        }
        for invalid in [
            [Some(0), None, Some(1)],
            [Some(3), None, Some(1)],
            [None; 3],
        ] {
            assert!(crate::physical_tick::calculate_serial_world(&input, &invalid, 0).is_err());
        }
        // Body1 advances first; body2 then overflows. Input body1 stays intact.
        input.bodies[2].basis[3][0] = f32::MAX;
        input.step.bodies[2].contact.position[0] = f32::MAX;
        input.step.bodies[2].contact.linear[0] = f32::MAX;
        assert!(crate::physical_tick::calculate_serial_world(&input, &links, 0).is_err());
        assert_eq!(input.step.bodies[1].contact.position[0], 0.);
        assert_eq!(input.bodies[1].basis[3][0], 0.);
    }
}
