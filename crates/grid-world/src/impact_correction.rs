//! Whole optional correction pass (B7B5E0) over the impact-contact and joint
//! pools. Row creation and main velocity integration are external.
use crate::{Error, contact_bias, joint::Joint, joint_rows, physical_pose, physical_response};

#[derive(Clone, Copy)]
pub struct Contact {
    pub disabled: bool,
    pub accumulated: f32,
}

pub struct Input {
    /// The normal rows: one per two-body contact, the two-body joints' rows,
    /// one per single-body contact, the single-body joints' rows.
    pub rows: Vec<physical_response::Row>,
    pub two_body_contacts: Vec<Contact>,
    pub single_body_contacts: Vec<Contact>,
    /// The joints after the main response, each pool in group order.
    pub two_body_joints: Vec<Joint>,
    pub single_body_joints: Vec<Joint>,
    /// Input correction vectors in these poses must be zero. Solver supplies them.
    pub poses: Vec<physical_pose::Input>,
    pub workspace: Vec<[[f32; 4]; 2]>,
    /// Original C0/C8 span ends, preserved when both contact pools are empty.
    pub span_ends: [usize; 2],
    pub iterations: usize,
    pub softness: f32,
    pub scale: f32,
    pub threshold: f32,
    pub relaxation: f32,
}

pub struct Output {
    pub rows: Vec<physical_response::Row>,
    pub workspace: Vec<[[f32; 4]; 2]>,
    /// None means the original pass did not publish this body at all.
    pub poses: Vec<Option<physical_pose::Output>>,
    pub span_ends: [usize; 2],
    /// The row80/90 reset and row84 softness write occurred before bias refresh.
    pub rows_reset: bool,
    pub active: bool,
}

/// AEF930 / AEFBB0 over one pool's joint rows (only their response and bias
/// change).
fn joint_pass(
    joints: &[Joint],
    two_body: bool,
    rows: &mut [physical_response::Row],
    scale: f32,
    threshold: f32,
) -> bool {
    let mut raw: Vec<crate::contact_rows::Row> = rows
        .iter()
        .map(|r| {
            let mut w = [0; 48];
            for (i, v) in r.response.iter().flatten().enumerate() {
                w[16 + i] = v.to_bits();
            }
            w[36] = r.bias.to_bits();
            w
        })
        .collect();
    let mut cursor = 0;
    let mut active = false;
    for j in joints {
        active |= joint_rows::correct(j, two_body, scale, threshold, &mut raw, &mut cursor);
    }
    for (r, w) in rows.iter_mut().zip(&raw) {
        for (i, v) in r.response.iter_mut().flatten().enumerate() {
            *v = f32::from_bits(w[16 + i]);
        }
        r.bias = f32::from_bits(w[36]);
    }
    active
}

fn joint_rows(joints: &[Joint]) -> usize {
    joints.iter().map(|j| j.words[0x18 / 4] as usize).sum()
}

pub fn calculate(input: &Input) -> Result<Output, Error> {
    let contacts = input.two_body_contacts.len() + input.single_body_contacts.len();
    let two_joint_rows = joint_rows(&input.two_body_joints);
    let split = input.two_body_contacts.len() + two_joint_rows;
    let n = contacts + two_joint_rows + joint_rows(&input.single_body_joints);
    let items = contacts + input.two_body_joints.len() + input.single_body_joints.len();
    if contacts > crate::MAX_WORLD_CONTACTS
        || n > crate::MAX_SOLVER_ROWS
        || n != input.rows.len()
        || input.poses.len() > crate::MAX_WORLD_BODIES
        || input.workspace.len() != input.poses.len()
        || input.iterations > 32
        || [
            input.softness,
            input.scale,
            input.threshold,
            input.relaxation,
        ]
        .iter()
        .any(|v| !v.is_finite())
        || input
            .workspace
            .iter()
            .flatten()
            .flatten()
            .any(|v| !v.is_finite())
        || input
            .two_body_contacts
            .iter()
            .chain(&input.single_body_contacts)
            .any(|c| !c.accumulated.is_finite())
        || input
            .poses
            .iter()
            .any(|p| p.linear.iter().chain(&p.angular).any(|&v| v != 0.))
    {
        return Err(Error("invalid impact correction input"));
    }
    // Validate supplied layout even on the bypass branch. This is an adapter
    // precondition, not a claim that the original dereferences bypassed fields.
    physical_response::calculate(&physical_response::Input {
        rows: input.rows.clone(),
        two_body_rows: split,
        body_count: input.poses.len(),
        iterations: 0,
        normalize: false,
        relaxation: input.relaxation,
    })?;
    for p in &input.poses {
        physical_pose::calculate(p)?;
    }
    let mut rows = input.rows.clone();
    let mut span_ends = input.span_ends;
    let mut active = false;
    if items != 0 {
        span_ends = [n; 2];
    }
    if n != 0 {
        for row in &mut rows {
            row.bias = 0.;
            row.softness_input = input.softness;
        }
        // B0CE10 / B0CEC0 for the contacts; AEF930 / AEFBB0 for the joints
        // (the original interleaves the pools; each only adds to `active`).
        let two_contacts = input.two_body_contacts.len();
        let indices = (0..two_contacts).chain(split..split + input.single_body_contacts.len());
        for (i, contact) in indices.zip(
            input
                .two_body_contacts
                .iter()
                .chain(&input.single_body_contacts),
        ) {
            let row = &mut rows[i];
            let bias = contact_bias::calculate(&contact_bias::Input {
                two_bodies: i < split,
                disabled: contact.disabled,
                accumulated: contact.accumulated,
                threshold: input.threshold,
                scale: input.scale,
                upper_limit: f32::MAX,
                active,
                constraint: contact_bias::Constraint {
                    vectors: row.response,
                    lower: row.lower,
                    upper: row.upper,
                    bias: row.bias,
                },
            })?;
            row.response = bias.constraint.vectors;
            row.lower = bias.constraint.lower;
            row.upper = bias.constraint.upper;
            row.bias = bias.constraint.bias;
            active = bias.active;
        }
        active |= joint_pass(
            &input.two_body_joints,
            true,
            &mut rows[two_contacts..split],
            input.scale,
            input.threshold,
        );
        active |= joint_pass(
            &input.single_body_joints,
            false,
            &mut rows[split + input.single_body_contacts.len()..],
            input.scale,
            input.threshold,
        );
    }
    let (rows, workspace, poses) = if active {
        let solved = physical_response::calculate(&physical_response::Input {
            rows,
            two_body_rows: split,
            body_count: input.poses.len(),
            iterations: input.iterations,
            normalize: true,
            relaxation: input.relaxation,
        })?;
        let poses = input
            .poses
            .iter()
            .zip(&solved.bodies)
            .map(|(p, correction)| {
                let mut supplied = p.clone();
                supplied.linear = correction[0];
                supplied.angular = correction[1];
                physical_pose::calculate(&supplied).map(Some)
            })
            .collect::<Result<Vec<_>, _>>()?;
        (solved.rows, solved.bodies, poses)
    } else {
        (
            rows,
            input.workspace.clone(),
            (0..input.poses.len()).map(|_| None).collect(),
        )
    };
    Ok(Output {
        rows,
        workspace,
        poses,
        span_ends,
        rows_reset: n != 0,
        active,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> Input {
        let identity = [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 0., 0., 1.],
        ];
        Input {
            rows: vec![],
            two_body_contacts: vec![],
            single_body_contacts: vec![],
            two_body_joints: vec![],
            single_body_joints: vec![],
            poses: vec![physical_pose::Input {
                basis: identity,
                offset: [0.; 4],
                local_matrices: [identity; 2],
                group_head: false,
                shapes: vec![None],
                linear: [0.; 4],
                angular: [0.; 4],
            }],
            workspace: vec![[[7.; 4]; 2]],
            span_ends: [13, 17],
            iterations: 2,
            softness: 0.1,
            scale: 1.,
            threshold: 1.,
            relaxation: 1.,
        }
    }
    fn row() -> physical_response::Row {
        physical_response::Row {
            jacobian: [[0.; 4]; 4],
            response: [[1.; 4]; 4],
            softness_input: 2.,
            lower: -1.,
            upper: 1.,
            bias: 3.,
            impulse: 4.,
            cumulative: 5.,
            softness: 6.,
            bodies: [0, 0],
            normalization: 1.,
            denominator: 2.,
            batch_len: 1,
        }
    }
    #[test]
    fn empty_pools_preserve_workspace_spans_and_skip_publication() {
        let v = input();
        let out = calculate(&v).unwrap();
        assert_eq!(out.span_ends, [13, 17]);
        assert_eq!(out.workspace, v.workspace);
        assert!(out.poses[0].is_none());
        assert!(!out.rows_reset);
        assert!(!out.active);
    }
    #[test]
    fn disabled_contact_resets_row_but_preserves_body_workspace() {
        let mut v = input();
        v.rows.push(row());
        v.single_body_contacts.push(Contact {
            disabled: true,
            accumulated: 2.,
        });
        let out = calculate(&v).unwrap();
        assert!(out.rows_reset);
        assert!(!out.active);
        assert_eq!(out.span_ends, [1, 1]);
        assert_eq!(out.rows[0].bias.to_bits(), 0);
        assert_eq!(out.rows[0].impulse, 4.);
        assert_eq!(out.rows[0].response[..2], [[0.; 4]; 2]);
        assert_eq!(out.workspace, v.workspace);
        assert!(out.poses[0].is_none());
    }
    #[test]
    fn equality_activates_even_with_zero_iterations() {
        let mut v = input();
        v.rows.push(row());
        v.single_body_contacts.push(Contact {
            disabled: false,
            accumulated: 1.,
        });
        v.iterations = 0;
        let out = calculate(&v).unwrap();
        assert!(out.active);
        assert_eq!(out.workspace, vec![[[0.; 4]; 2]]);
        assert!(out.poses[0].is_some());
        assert_eq!(out.rows[0].impulse.to_bits(), 0);
    }
}
