//! Body advance followed by a supplied-contact physical frame.
//! Contact generation/group construction, group-head normalization and active
//! callbacks are external. The caller retains previous published/center poses.
use crate::{Error, dynamics::BodyState, impact_frame, physical_pose, pose};

pub struct Output {
    /// Advance-stage publications, before contact correction or pending shifts.
    pub advanced: Vec<physical_pose::Output>,
    pub frame: impact_frame::Output,
}

pub fn calculate(input: &impact_frame::Input) -> Result<Output, Error> {
    calculate_ordered(input, &(0..input.bodies.len()).collect::<Vec<_>>())
}

pub struct SerialWorldOutput {
    pub tick: Output,
    pub update_head: usize,
    pub update_next: Vec<Option<usize>>,
}

/// Whole selected synchronous world advance: reverse the active +300 list into
/// the +318 update list, advance in that order, then solve supplied contacts.
/// All bodies must occur once. Async jobs, empty/partial lists and group heads
/// are outside this selected boundary; no captured address is followed.
pub fn calculate_serial_world(
    input: &impact_frame::Input,
    active_next: &[Option<usize>],
    active_head: usize,
) -> Result<SerialWorldOutput, Error> {
    if active_next.len() != input.bodies.len()
        || active_next.is_empty()
        || active_next.len() > crate::MAX_WORLD_BODIES
    {
        return Err(Error("invalid serial world body list"));
    }
    let mut seen = vec![false; active_next.len()];
    let mut order = Vec::new();
    let mut current = Some(active_head);
    while let Some(i) = current {
        if i >= seen.len() || seen[i] {
            return Err(Error("foreign or cyclic serial world body list"));
        }
        seen[i] = true;
        order.push(i);
        current = active_next[i];
    }
    if seen.iter().any(|&found| !found) {
        return Err(Error("incomplete serial world body list"));
    }
    order.reverse();
    let mut update_next = vec![None; order.len()];
    for pair in order.windows(2) {
        update_next[pair[0]] = Some(pair[1]);
    }
    Ok(SerialWorldOutput {
        tick: calculate_ordered(input, &order)?,
        update_head: order[0],
        update_next,
    })
}

fn calculate_ordered(input: &impact_frame::Input, order: &[usize]) -> Result<Output, Error> {
    impact_frame::validate(input)?;
    let mut next = input.clone();
    let delta = input.step.parameters[0];
    let mut advanced: Vec<_> = (0..next.bodies.len()).map(|_| None).collect();
    for &i in order {
        let settings = &mut next.bodies[i];
        let body = &mut next.step.bodies[i];
        let corrected = pose::predict(
            settings.basis,
            BodyState {
                linear: body.contact.linear,
                angular: body.contact.angular,
                position: body.contact.position,
            },
            delta,
        )?;
        let publication =
            physical_pose::publish(corrected, settings.offset, settings.local_matrices, &[])?;
        settings.basis = publication.corrected;
        body.contact.position = publication.corrected[3];
        body.torque_matrix = publication.derived[1];
        advanced[i] = Some(publication);
    }
    Ok(Output {
        advanced: advanced.into_iter().map(Option::unwrap).collect(),
        frame: impact_frame::calculate(&next)?,
    })
}
