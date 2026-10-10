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
    impact_frame::validate(input)?;
    let mut next = input.clone();
    let delta = input.step.parameters[0];
    let mut advanced = Vec::with_capacity(next.bodies.len());
    for (settings, body) in next.bodies.iter_mut().zip(&mut next.step.bodies) {
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
        advanced.push(publication);
    }
    Ok(Output {
        advanced,
        frame: impact_frame::calculate(&next)?,
    })
}
