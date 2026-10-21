//! Impact-contact-only main manager: construction, solve, writeback and velocity update.
//! The caller supplies contacts, inverse properties and the five step scalars.
use crate::{
    Error, contact_groups,
    contact_rows::{self, Contact, Material, Row},
    joint::{self, Bodies, Joint},
    joint_rows, main_contact_response,
};

#[derive(Clone)]
pub struct Body {
    pub contact: contact_rows::Body,
    pub inverse_mass: f32,
    pub inverse_inertia: [[f32; 4]; 4],
    pub force_inverse_mass: f32,
    pub torque_matrix: [[f32; 4]; 4],
    pub impulse_accumulator: [f32; 4],
    pub torque: [f32; 4],
    pub force: [f32; 4],
    pub impulse_active: bool,
    pub apply_torque: bool,
    pub apply_force: bool,
}
/// One of the island's joints (B7B850's joint pools).
#[derive(Clone)]
pub struct IslandJoint {
    pub joint: Joint,
    /// Local index of body A, and of B when B is a dynamic island body.
    pub a: usize,
    pub b: Option<usize>,
    /// B's record when B is a fixed object (joint +0xC).
    pub fixed: Option<Box<contact_groups::Body>>,
}

impl IslandJoint {
    /// B7B850's pool: two dynamic bodies, or one.
    pub fn two_body(&self) -> bool {
        self.joint.byte(0xc) == 0 && self.b.is_some()
    }
}

/// The island's joints in group order, with the island's body records in
/// local order (the joints read their poses).
#[derive(Clone, Default)]
pub struct Joints {
    pub bodies: Vec<contact_groups::Body>,
    pub list: Vec<IslandJoint>,
}

impl Joints {
    pub(crate) fn bodies<'a>(&'a self, j: &'a IslandJoint) -> Bodies<'a> {
        Bodies {
            a: &self.bodies[j.a],
            b: j.b.map(|i| &self.bodies[i]).or(j.fixed.as_deref()),
            fixed: j.joint.byte(0xc) != 0,
        }
    }
}

#[derive(Clone)]
pub struct Input {
    /// Delta, time factor, bias factor, compliance scale, writeback scale,
    /// default softness, fallback bias, threshold, relaxation, angular scale,
    /// angular normal-Y threshold. Time/force units are caller-owned.
    pub parameters: [f32; 11],
    pub bodies: Vec<Body>,
    pub materials: Vec<Material>,
    /// Fresh metadata requires every supplied group_end flag to be false.
    pub contacts: Vec<Contact>,
    pub rows: Vec<Row>,
    pub capacity: usize,
    pub iterations: usize,
    pub initial_ends: [usize; 4],
    pub workspace: Vec<[[f32; 4]; 2]>,
    pub cached_pair: Option<[usize; 2]>,
    pub cached_material: Material,
    pub joints: Joints,
}
pub struct Output {
    pub success: bool,
    pub ends: [usize; 4],
    pub bodies: Vec<Body>,
    pub contacts: Vec<Contact>,
    /// Total row count, then normal row count, per original contact.
    pub metadata: Vec<[u32; 2]>,
    pub rows: Vec<Row>,
    pub workspace: Vec<[[f32; 4]; 2]>,
    pub cached_pair: Option<[usize; 2]>,
    pub cached_material: Material,
    /// The joints after prepare and, when solved, their impulse sums and
    /// break flags.
    pub joints: Joints,
}
fn transform(v: [f32; 4], m: [[f32; 4]; 4]) -> [f32; 4] {
    std::array::from_fn(|i| (v[3] * m[3][i] + v[1] * m[1][i]) + (v[2] * m[2][i] + v[0] * m[0][i]))
}
#[allow(clippy::assign_op_pattern)] // Preserve original multiplication/addition order.
fn force_update(body: &mut Body, delta: f32) {
    if body.apply_force {
        let scale = body.force_inverse_mass * delta;
        for i in 0..4 {
            body.contact.linear[i] = scale * body.force[i] + body.contact.linear[i];
        }
    }
    if body.apply_torque {
        let torque = transform(body.torque, body.torque_matrix);
        for (angular, torque) in body.contact.angular.iter_mut().zip(torque) {
            *angular = torque * delta + *angular;
        }
    }
}
pub(crate) fn validate(input: &Input) -> Result<(), Error> {
    validate_rows(input, &input.rows)
}

/// [`validate`] with the row buffer the step will use (the caller may have
/// taken it out of `input`).
pub(crate) fn validate_rows(input: &Input, rows: &[Row]) -> Result<(), Error> {
    let n = input.contacts.len();
    let delta = input.parameters[0] * input.parameters[1];
    if n > crate::MAX_WORLD_CONTACTS
        || input.bodies.is_empty()
        || input.bodies.len() > crate::MAX_WORLD_BODIES
        || input.materials.is_empty()
        || input.materials.len() > 8
        || rows.is_empty()
        || rows.len() > crate::MAX_SOLVER_ROWS
        || input.capacity > rows.len()
        || input.iterations > 32
        || input.workspace.len() != input.bodies.len()
        || input
            .parameters
            .iter()
            .chain(&input.cached_material)
            .chain(input.materials.iter().flatten())
            .any(|v| !v.is_finite())
        || delta <= 0.
        || input.parameters[0] <= 0.
        || input.parameters[1] <= 0.
        || !delta.is_finite()
        || !(1. / delta).is_finite()
        || !((1. / delta) * input.parameters[1]).is_finite()
        || input
            .cached_pair
            .is_some_and(|p| p.iter().any(|&i| i >= input.materials.len()))
        || input.bodies.iter().any(|b| {
            b.contact
                .position
                .iter()
                .chain(&b.contact.angular)
                .chain(&b.contact.linear)
                .chain(b.inverse_inertia.iter().flatten())
                .chain(b.torque_matrix.iter().flatten())
                .chain(&b.impulse_accumulator)
                .chain(&b.torque)
                .chain(&b.force)
                .chain([b.inverse_mass, b.force_inverse_mass].iter())
                .any(|v| !v.is_finite())
        })
        || input
            .workspace
            .iter()
            .flatten()
            .flatten()
            .any(|v| !v.is_finite())
        || input.contacts.iter().any(|c| {
            c.group_end
                || c.bodies.iter().any(|&i| i >= input.bodies.len())
                || c.materials.iter().any(|&i| i >= input.materials.len())
                || c.point
                    .iter()
                    .chain(&c.normal)
                    .chain([c.accumulated, c.previous_impulse].iter())
                    .any(|v| !v.is_finite())
                || c.previous_impulse < 0.
        })
        || input.joints.list.iter().any(|j| {
            j.a >= input.bodies.len()
                || j.b.is_some_and(|b| b >= input.bodies.len() || b == j.a)
                || (j.joint.byte(0xc) != 0 && j.b.is_some())
                || (j.joint.byte(0xc) == 0 && j.fixed.is_some())
        })
        || (!input.joints.list.is_empty() && input.joints.bodies.len() != input.bodies.len())
    {
        return Err(Error("invalid impact step input"));
    }
    Ok(())
}
pub fn calculate(input: &Input) -> Result<Output, Error> {
    validate(input)?;
    solve(input, input.rows.clone())
}

/// [`calculate`] consuming the input, so the row buffer moves instead of
/// being copied (one call per contact group per tick).
pub(crate) fn calculate_owned(mut input: Input) -> Result<Output, Error> {
    validate(&input)?;
    let rows = std::mem::take(&mut input.rows);
    solve(&input, rows)
}

impl Input {
    /// Every field but the row buffer, which the caller supplies.
    pub(crate) fn with_rows(&self, rows: Vec<Row>) -> Self {
        Self {
            parameters: self.parameters,
            bodies: self.bodies.clone(),
            materials: self.materials.clone(),
            contacts: self.contacts.clone(),
            rows,
            capacity: self.capacity,
            iterations: self.iterations,
            initial_ends: self.initial_ends,
            workspace: self.workspace.clone(),
            cached_pair: self.cached_pair,
            cached_material: self.cached_material,
            joints: self.joints.clone(),
        }
    }
}

#[allow(clippy::assign_op_pattern)] // Retain original multiply/add operand order.
fn solve(input: &Input, rows: Vec<Row>) -> Result<Output, Error> {
    let n = input.contacts.len();
    let delta = input.parameters[0] * input.parameters[1];
    let mut out = Output {
        success: true,
        ends: input.initial_ends,
        bodies: input.bodies.clone(),
        contacts: input.contacts.clone(),
        metadata: vec![[0; 2]; n],
        rows,
        workspace: input.workspace.clone(),
        cached_pair: input.cached_pair,
        cached_material: input.cached_material,
        joints: input.joints.clone(),
    };
    if n != 0 || !out.joints.list.is_empty() {
        // B7ACC0 prepares every joint (AF4890) while counting its rows into
        // the normal spans after the contacts' (joints have no friction rows).
        let mut joint_rows = [0usize; 2];
        let joints = &mut out.joints;
        for k in 0..joints.list.len() {
            let mut j = joints.list[k].joint.clone();
            // B82850: the solver indices of the joint's bodies.
            j.words[0x1c / 4] = joints.list[k].a as u32;
            j.words[0x20 / 4] = joints.list[k].b.map_or(u32::MAX, |b| b as u32);
            joint::prepare(&mut j, &joints.bodies(&joints.list[k]));
            joint_rows[usize::from(joints.list[k].two_body())] += j.words[0x18 / 4] as usize;
            joints.list[k].joint = j;
        }
        let mut order = Vec::with_capacity(n);
        let mut counts = [0usize; 4];
        for two in [true, false] {
            let pool: Vec<_> = input
                .contacts
                .iter()
                .enumerate()
                .filter(|(_, c)| c.two_bodies == two)
                .map(|(i, _)| i)
                .collect();
            for (j, &index) in pool.iter().enumerate() {
                let c = &mut out.contacts[index];
                c.group_end = pool.get(j + 1).is_none_or(|&next| {
                    input.contacts[next].bodies[0] != c.bodies[0]
                        || (two && input.contacts[next].bodies[1] != c.bodies[1])
                });
                out.metadata[index] = [if c.group_end { 4 } else { 1 }, 1];
                let p = if two { 2 } else { 0 };
                counts[p] += 1;
                if c.group_end {
                    counts[p + 1] += 3;
                }
                order.push(index);
            }
        }
        let contact_counts = counts;
        counts[2] += joint_rows[1];
        counts[0] += joint_rows[0];
        out.ends = [
            counts[2],
            counts[2] + counts[0],
            counts[2] + counts[0] + counts[3],
            counts.iter().sum(),
        ];
        if out.ends[3] > input.capacity {
            out.success = false;
            return Ok(out);
        }
        for row in &mut out.rows[..out.ends[3]] {
            row[32] = 0;
            row[33] = input.parameters[5].to_bits();
            row[34] = (-f32::MAX).to_bits();
            row[35] = f32::MAX.to_bits();
            row[40] = u32::MAX;
            row[41] = u32::MAX;
            row[42] = 0;
            row[45] = 0;
        }
        let mut offsets = [0usize; 4];
        let mut start = 0;
        for p in [2, 0, 3, 1] {
            offsets[p] = start;
            start += counts[p];
        }
        let inverse = 1. / delta;
        let time = inverse * input.parameters[1];
        let mut heads = [None; 4];
        let mut batch_counts = [0; 4];
        if n != 0 {
            let mut builder_rows: [Vec<Row>; 4] =
                std::array::from_fn(|p| vec![[0; 48]; n * if p % 2 == 0 { 1 } else { 3 }]);
            for p in [2, 0, 3, 1] {
                builder_rows[p][..contact_counts[p]]
                    .copy_from_slice(&out.rows[offsets[p]..offsets[p] + contact_counts[p]]);
            }
            let built = contact_rows::calculate(&contact_rows::Input {
                parameters: [
                    time,
                    input.parameters[6],
                    input.parameters[7],
                    input.parameters[3],
                    input.parameters[9],
                    input.parameters[10],
                ],
                bodies: input.bodies.iter().map(|b| b.contact.clone()).collect(),
                materials: input.materials.clone(),
                contacts: order.iter().map(|&i| out.contacts[i].clone()).collect(),
                rows: builder_rows,
                cached_pair: input.cached_pair,
                cached_material: input.cached_material,
            })?;
            out.cached_pair = built.cached_pair;
            out.cached_material = built.cached_material;
            let built_rows = built.rows;
            for p in [2, 0, 3, 1] {
                out.rows[offsets[p]..offsets[p] + contact_counts[p]]
                    .copy_from_slice(&built_rows[p][..contact_counts[p]]);
            }
            heads = built.heads;
            batch_counts = built.batch_counts;
        }
        // AF71C0 / AF9430 continue the normal pools' batches after the
        // contacts' rows; B7ACC0 then closes each pool's last batch.
        let mut context = joint_rows::Context {
            inverse_step: time,
            reduction: input.parameters[6],
            step: input.parameters[3],
            impulse_scale: input.parameters[4],
            two_batch: batch_counts[2],
            one_batch: batch_counts[0],
            total: 0,
            one_cursor: contact_counts[0],
            two_cursor: contact_counts[2],
            one_head: heads[0],
            two_head: heads[2],
        };
        {
            let (two, rest) = out.rows.split_at_mut(counts[2]);
            let mut pools = joint_rows::Pools {
                one: &mut rest[..counts[0]],
                two,
            };
            for two_body in [true, false] {
                for j in out.joints.list.iter().filter(|j| j.two_body() == two_body) {
                    joint_rows::write(
                        &j.joint,
                        &out.joints.bodies(j),
                        two_body,
                        &mut context,
                        &mut pools,
                    );
                }
            }
        }
        heads[2] = context.two_head;
        heads[0] = context.one_head;
        batch_counts[2] = context.two_batch;
        batch_counts[0] = context.one_batch;
        for p in [2, 0, 3, 1] {
            if let Some(head) = heads[p] {
                out.rows[offsets[p] + head][45] = batch_counts[p];
            }
        }
        let mut references = vec![None; out.ends[3]];
        for (i, row) in out.rows[..out.ends[3]]
            .iter_mut()
            .enumerate()
            .skip(out.ends[1])
        {
            let pool = (row[42] >> 24) - 1;
            let index = (row[42] & 0xffffff) as usize / 192;
            let index = index + if pool == 0 { counts[2] } else { 0 };
            row[42] = index as u32;
            references[i] = Some(index);
        }
        let bodies = input
            .bodies
            .iter()
            .map(|b| {
                let mut linear = b.contact.linear.map(|v| inverse * v);
                let mut angular = b.contact.angular.map(|v| inverse * v);
                if b.apply_force {
                    for (linear, force) in linear.iter_mut().zip(b.force) {
                        *linear = b.force_inverse_mass * force + *linear;
                    }
                }
                if b.apply_torque {
                    let torque = transform(b.torque, b.torque_matrix);
                    for i in 0..4 {
                        angular[i] = torque[i] + angular[i];
                    }
                }
                main_contact_response::Body {
                    workspace: [linear, angular],
                    inverse_mass: b.inverse_mass,
                    inverse_inertia: b.inverse_inertia,
                }
            })
            .collect();
        let solved = main_contact_response::calculate(&main_contact_response::Input {
            rows: out.rows[..out.ends[3]].to_vec(),
            ends: out.ends,
            references,
            bodies,
            softness_scale: time,
            bias_scale: inverse * input.parameters[2],
            relaxation: input.parameters[8],
            iterations: input.iterations,
        })?;
        out.rows[..out.ends[3]].copy_from_slice(&solved.rows);
        out.workspace = solved.bodies;
        let mut normal_indices = [0usize; 2];
        normal_indices[1] = counts[2];
        for index in order {
            let c = &mut out.contacts[index];
            let p = usize::from(!c.two_bodies);
            let row = &out.rows[normal_indices[p]];
            normal_indices[p] += 1;
            let impulse = (f32::from_bits(row[37]) * input.parameters[4]) / time;
            c.previous_impulse = impulse;
            let vector = c.normal.map(|v| impulse * v);
            let a = &mut out.bodies[c.bodies[0]];
            a.impulse_active = true;
            for (accum, value) in a.impulse_accumulator.iter_mut().zip(vector) {
                *accum = value + *accum;
            }
            if c.two_bodies {
                let b = &mut out.bodies[c.bodies[1]];
                b.impulse_active = true;
                for (accum, value) in b.impulse_accumulator.iter_mut().zip(vector) {
                    *accum = -value + *accum;
                }
            }
        }
        // AFB280 / AFB800: the joints' impulse sums and break flags.
        context.two_cursor = contact_counts[2];
        context.one_cursor = contact_counts[0];
        {
            let (two, rest) = out.rows.split_at_mut(counts[2]);
            let pools = joint_rows::Pools {
                one: &mut rest[..counts[0]],
                two,
            };
            for two_body in [true, false] {
                for j in out
                    .joints
                    .list
                    .iter_mut()
                    .filter(|j| j.two_body() == two_body)
                {
                    joint_rows::write_back(&mut j.joint, two_body, &mut context, &pools);
                }
            }
        }
        for (b, workspace) in out.bodies.iter_mut().zip(&out.workspace) {
            for (v, &correction) in b.contact.linear.iter_mut().zip(&workspace[0]) {
                *v = delta * correction + *v;
            }
            for (v, &correction) in b.contact.angular.iter_mut().zip(&workspace[1]) {
                *v = *v + delta * correction;
            }
        }
    }
    for body in &mut out.bodies {
        force_update(body, delta);
    }
    if out.bodies.iter().any(|b| {
        b.contact
            .linear
            .iter()
            .chain(&b.contact.angular)
            .chain(&b.impulse_accumulator)
            .any(|v| !v.is_finite())
    }) || out.contacts.iter().any(|c| !c.previous_impulse.is_finite())
    {
        return Err(Error("nonfinite impact step output"));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Input {
        Input {
            parameters: [0.5, 2., 0.25, 1., 1., 0.01, 0.2, 0.125, 0.8, 0.2, 0.2],
            bodies: vec![Body {
                contact: contact_rows::Body {
                    position: [0.; 4],
                    angular: [0.; 4],
                    linear: [1.; 4],
                    scale_angular: false,
                    directional_friction: false,
                },
                inverse_mass: 1.,
                inverse_inertia: [[0.; 4]; 4],
                force_inverse_mass: 2.,
                torque_matrix: std::array::from_fn(|i| std::array::from_fn(|j| f32::from(i == j))),
                impulse_accumulator: [0.; 4],
                torque: [1., 2., 3., 4.],
                force: [1.; 4],
                impulse_active: false,
                apply_torque: true,
                apply_force: true,
            }],
            materials: vec![[0.; 13]],
            contacts: vec![],
            rows: vec![[0x12345678; 48]; 8],
            capacity: 0,
            iterations: 1,
            initial_ends: [1, 2, 3, 4],
            workspace: vec![[[7.; 4]; 2]],
            cached_pair: None,
            cached_material: [0.; 13],
            joints: Joints::default(),
        }
    }
    #[test]
    fn empty_step_applies_forces_and_retains_workspace_and_rows() {
        let input = input();
        let o = calculate(&input).unwrap();
        assert!(o.success);
        assert_eq!(o.bodies[0].contact.linear, [3.; 4]);
        assert_eq!(o.bodies[0].contact.angular, [1., 2., 3., 4.]);
        assert_eq!(o.ends, input.initial_ends);
        assert_eq!(o.workspace, input.workspace);
        assert_eq!(o.rows, input.rows);
        assert!(!o.bodies[0].impulse_active);
    }
    #[test]
    fn capacity_failure_publishes_metadata_but_skips_forces() {
        let mut input = input();
        let c = Contact {
            two_bodies: true,
            bodies: [0, 0],
            materials: [0, 0],
            shapes: [None; 2],
            special: false,
            group_end: false,
            disabled: false,
            accumulated: 1.,
            previous_impulse: f32::MAX,
            point: [0.; 4],
            normal: [0., 1., 0., 0.],
        };
        input.contacts = vec![c.clone(), c];
        let o = calculate(&input).unwrap();
        assert!(!o.success);
        assert_eq!(o.metadata, [[1, 1], [4, 1]]);
        assert_eq!(o.ends, [2, 2, 5, 5]);
        assert!(!o.contacts[0].group_end);
        assert!(o.contacts[1].group_end);
        assert_eq!(o.bodies[0].contact.linear, [1.; 4]);
        assert_eq!(o.bodies[0].contact.angular, [0.; 4]);
        assert_eq!(o.rows, input.rows);
        assert_eq!(o.workspace, input.workspace);
    }
    #[test]
    fn disabled_force_flags_preserve_empty_body_velocities() {
        let mut input = input();
        input.bodies[0].apply_force = false;
        input.bodies[0].apply_torque = false;
        let o = calculate(&input).unwrap();
        assert_eq!(o.bodies[0].contact.linear, [1.; 4]);
        assert_eq!(o.bodies[0].contact.angular, [0.; 4]);
    }
    /// Two bodies joined by a locked joint and no contacts: the island still
    /// solves, the limit row holds the leaving body, and a small
    /// `breakLinearImpulse` breaks the joint in the write-back.
    #[test]
    fn a_joint_alone_holds_its_bodies_and_breaks_past_its_impulse() {
        use crate::joint::tests::{body_at, descriptor};
        let run = |break_linear: f32| {
            let mut i = input();
            let mut a = i.bodies[0].clone();
            a.apply_force = false;
            a.apply_torque = false;
            a.contact.linear = [0.; 4];
            let mut b = a.clone();
            b.contact.position = [2.5, 0., 0., 1.];
            b.contact.linear = [1., 0., 0., 0.];
            i.bodies = vec![a, b];
            i.workspace = vec![[[0.; 4]; 2]; 2];
            i.parameters = [1. / 60., 1., 1., 1., 1., 0., 0.2, 0.003, 1., 0.2, 0.2];
            i.iterations = 5;
            i.capacity = 8;
            let (ra, rb) = (body_at([0., 0., 0.]), body_at([2., 0., 0.]));
            let mut d = descriptor([1., 0., 0.], -0.05, 0.05);
            d[0x154 / 4] = break_linear.to_bits();
            let mut j = Joint::filled(0);
            joint::construct(
                &mut j,
                &mut d,
                &Bodies {
                    a: &ra,
                    b: Some(&rb),
                    fixed: false,
                },
                0,
            );
            i.joints = Joints {
                bodies: vec![ra, body_at([2.5, 0., 0.])],
                list: vec![IslandJoint {
                    joint: j,
                    a: 0,
                    b: Some(1),
                    fixed: None,
                }],
            };
            calculate(&i).unwrap()
        };
        let held = run(f32::MAX);
        assert!(held.success);
        assert_eq!(held.ends, [1, 1, 1, 1]);
        assert!(
            held.bodies[1].contact.linear[0] < 1.,
            "{:?}",
            held.bodies[1].contact.linear
        );
        assert!(held.bodies[0].contact.linear[0] > 0.);
        assert_eq!(held.joints.list[0].joint.byte(0x48), 0);
        assert!(held.joints.list[0].joint.get(0x160) != 0.);
        let broken = run(1e-3);
        assert_eq!(broken.joints.list[0].joint.byte(0x48), 1);
    }
    #[test]
    fn invalid_time_is_rejected_even_on_empty_path() {
        let mut input = input();
        input.parameters[1] = 0.;
        assert!(calculate(&input).is_err());
        input.parameters[0] = -1.;
        input.parameters[1] = -1.;
        assert!(calculate(&input).is_err());
    }
}
