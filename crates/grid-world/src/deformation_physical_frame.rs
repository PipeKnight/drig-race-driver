//! One owned static-contact deformation preparation inside a physical frame.
//! Admitted shapes exclude every contact from the post-solver bulk collector.
use crate::{Error, deformation_contact_limit as limit, deformation_preparation, impact_frame};

#[derive(Clone)]
struct Preparation {
    descriptor: Vec<u32>,
    body: [u32; limit::PHYSICAL_WORDS],
    contacts: Vec<[u32; 32]>,
    parameters: limit::Parameters,
}
impl impact_frame::Callbacks for Preparation {
    fn prepare(
        &mut self,
        step: &mut crate::impact_step::Input,
        _bodies: &mut [impact_frame::Body],
    ) -> Result<(), Error> {
        let center = std::array::from_fn(|i| f32::from_bits(self.body[0x1c0 / 4 + i]));
        let answer = deformation_preparation::calculate(
            &self.descriptor,
            &self.body,
            center,
            &self.contacts,
            0,
            10000,
            0,
            self.parameters,
        )?;
        if !answer.complete {
            return Err(Error("incomplete physical deformation preparation"));
        }
        self.descriptor = answer.descriptor;
        self.contacts = answer.contacts;
        for (c, raw) in step.contacts.iter_mut().zip(&self.contacts) {
            c.previous_impulse = f32::from_bits(raw[0x3c / 4]);
        }
        Ok(())
    }
}
pub struct Output {
    pub frame: impact_frame::Output,
    pub descriptor: Vec<u32>,
    /// Preparation fields40/44 are retained alongside downstream typed contacts.
    pub prepared_contacts: Vec<[u32; 32]>,
}

pub fn calculate(
    input: &impact_frame::Input,
    descriptor: &[u32],
    body: &[u32; limit::PHYSICAL_WORDS],
    contacts: &[[u32; 32]],
    parameters: limit::Parameters,
    shapes: &[crate::contact_groups::Shape],
) -> Result<Output, Error> {
    let f = |at: usize| f32::from_bits(body[at / 4]);
    if input.bodies.len() != 1
        || input.step.bodies.len() != 1
        || contacts.len() != input.step.contacts.len()
        || body[0x334 / 4] != 0x1c0
        || [0x418, 0x41c, 0x420].iter().any(|at| body[at / 4] != 0)
        || f(0x3f0).to_bits() != input.bodies[0].mass.to_bits()
        || (0..4).any(|i| {
            body[0x210 / 4 + i] != input.step.bodies[0].contact.angular[i].to_bits()
                || body[0x220 / 4 + i] != input.step.bodies[0].contact.linear[i].to_bits()
                || body[0x1c0 / 4 + i] != input.step.bodies[0].contact.position[i].to_bits()
        })
        || input.step.contacts.iter().zip(contacts).any(|(c, r)| {
            c.two_bodies
                || c.bodies != [0, 0]
                || c.shapes.iter().any(Option::is_none)
                || [r[0x2c / 4], r[0x28 / 4]]
                    .into_iter()
                    .enumerate()
                    .any(|(i, id)| {
                        id == 0
                            || id as usize > shapes.len()
                            || c.shapes[i] != Some(shapes[id as usize - 1].angular)
                    })
                || ![r[0x2c / 4], r[0x28 / 4]]
                    .into_iter()
                    .any(|id| id != 0 && shapes.get(id as usize - 1).is_some_and(|s| s.flag230))
                || r[1] != 0
                || r[2] != 0
                || (r[3] & 255) == 0
                || c.accumulated.to_bits() != r[0x38 / 4]
                || c.previous_impulse.to_bits() != r[0x3c / 4]
                || (0..4).any(|i| {
                    c.point[i].to_bits() != r[0x50 / 4 + i]
                        || c.normal[i].to_bits() != r[0x60 / 4 + i]
                })
        })
    {
        return Err(Error("invalid physical deformation correspondence"));
    }
    let mut canonical = *body;
    canonical[0x334 / 4] = 0;
    // Validate the supplied grid/contact domain even when zero iterations or
    // sleeping will bypass the original callback phase. Zero budget performs
    // no updates; the independently staged invocation below owns all changes.
    deformation_preparation::calculate(
        descriptor,
        &canonical,
        std::array::from_fn(|i| f(0x1c0 + i * 4)),
        contacts,
        0,
        0,
        0,
        parameters,
    )?;
    let context = Preparation {
        descriptor: descriptor.to_vec(),
        body: canonical,
        contacts: contacts.to_vec(),
        parameters,
    };
    let (frame, context) = impact_frame::calculate_with_callbacks(input, &context)?;
    Ok(Output {
        frame,
        descriptor: context.descriptor,
        prepared_contacts: context.contacts,
    })
}
