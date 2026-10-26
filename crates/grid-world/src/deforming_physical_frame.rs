//! Both registered static-contact deformation phases inside the physical frame.
//! Source construction, grouping and dirty consumer publication stay external.
use crate::{
    Error, contact_groups::Shape, deformation_callback_state as bulk,
    deformation_contact_limit as limit, deformation_preparation, impact_frame, impact_step,
};

#[derive(Clone)]
struct Context {
    owner: Vec<u32>,
    physical: [u32; 0x424 / 4],
    contacts: Vec<[u32; 32]>,
    settings: Vec<u32>,
    shapes: Vec<Shape>,
    limit: limit::Parameters,
    bulk: bulk::Parameters,
    scalar: f32,
    random_seed: u32,
    bulk_invoked: bool,
    records: usize,
}
impl impact_frame::Callbacks for Context {
    fn prepare(
        &mut self,
        step: &mut impact_step::Input,
        _: &mut [impact_frame::Body],
    ) -> Result<(), Error> {
        let result = deformation_preparation::calculate(
            &self.owner[0x2300 / 4..0xc800 / 4],
            &self.physical,
            std::array::from_fn(|i| f32::from_bits(self.physical[0x1c0 / 4 + i])),
            &self.contacts,
            0,
            10000,
            0,
            self.limit,
        )?;
        if !result.complete {
            return Err(Error("incomplete deforming frame preparation"));
        }
        self.owner[0x2300 / 4..0xc800 / 4].copy_from_slice(&result.descriptor);
        self.contacts = result.contacts;
        for (c, raw) in step.contacts.iter_mut().zip(&self.contacts) {
            c.previous_impulse = f32::from_bits(raw[0x3c / 4]);
        }
        Ok(())
    }
    fn solved(
        &mut self,
        step: &mut impact_step::Output,
        bodies: &mut [impact_frame::Body],
    ) -> Result<(), Error> {
        for (c, raw) in step.contacts.iter().zip(&mut self.contacts) {
            raw[0x3c / 4] = c.previous_impulse.to_bits();
            raw[0x38 / 4] = c.accumulated.to_bits();
        }
        let selected: Vec<_> = self
            .contacts
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let excluded = [c[0x28 / 4], c[0x2c / 4]]
                    .into_iter()
                    .any(|id| self.shapes[id as usize - 1].flag230);
                (!excluded).then_some(i)
            })
            .collect();
        if selected.is_empty() {
            return Ok(());
        }
        self.physical[0x210 / 4..0x220 / 4]
            .copy_from_slice(&step.bodies[0].contact.angular.map(f32::to_bits));
        self.physical[0x220 / 4..0x230 / 4]
            .copy_from_slice(&step.bodies[0].contact.linear.map(f32::to_bits));
        self.physical[0x380 / 4..0x390 / 4]
            .copy_from_slice(&bodies[0].pending_shift.map(f32::to_bits));
        self.physical[0x390 / 4] =
            (self.physical[0x390 / 4] & !255) | u32::from(bodies[0].shift_active);
        self.physical[0x334 / 4] = 0x1c0;
        let contacts: Vec<_> = selected.iter().map(|&i| self.contacts[i]).collect();
        let result = bulk::calculate(
            &self.owner,
            &self.physical,
            &contacts,
            &self.settings,
            self.scalar,
            &self.bulk,
            self.random_seed,
        )?;
        self.owner = result.owner;
        self.physical = result.physical;
        self.random_seed = result.random_seed;
        self.bulk_invoked = true;
        self.records = result.records;
        for (&i, c) in selected.iter().zip(result.contacts) {
            self.contacts[i] = c;
            step.contacts[i].accumulated = f32::from_bits(c[0x38 / 4]);
        }
        step.bodies[0].contact.angular =
            std::array::from_fn(|i| f32::from_bits(self.physical[0x210 / 4 + i]));
        step.bodies[0].contact.linear =
            std::array::from_fn(|i| f32::from_bits(self.physical[0x220 / 4 + i]));
        bodies[0].pending_shift =
            std::array::from_fn(|i| f32::from_bits(self.physical[0x380 / 4 + i]));
        bodies[0].shift_active = self.physical[0x390 / 4] & 255 != 0;
        Ok(())
    }
}
pub struct Input<'a> {
    pub frame: &'a impact_frame::Input,
    pub owner: &'a [u32],
    pub physical: &'a [u32; 0x424 / 4],
    pub contacts: &'a [[u32; 32]],
    pub settings: &'a [u32],
    pub shapes: &'a [Shape],
    pub limit: limit::Parameters,
    pub bulk: &'a bulk::Parameters,
    pub random_seed: u32,
}
pub struct Output {
    pub frame: impact_frame::Output,
    pub owner: Vec<u32>,
    pub contacts: Vec<[u32; 32]>,
    pub random_seed: u32,
    pub bulk_invoked: bool,
    pub records: usize,
}
pub fn calculate(input: Input<'_>) -> Result<Output, Error> {
    calculate_rows(input, None)
}

/// [`calculate`] with the frame's row buffer given away (see
/// `impact_frame::calculate_with_callbacks_rows`).
pub(crate) fn calculate_rows(
    input: Input<'_>,
    rows: Option<Vec<crate::contact_rows::Row>>,
) -> Result<Output, Error> {
    let Input {
        frame,
        owner,
        physical,
        contacts,
        settings,
        shapes,
        limit,
        bulk,
        random_seed,
    } = input;
    if frame.bodies.len() != 1
        || frame.step.bodies.len() != 1
        || contacts.len() != frame.step.contacts.len()
        || !(0xd230 / 4..=0x21000 / 4).contains(&owner.len())
        || owner[0xdb0 / 4] > 1
        || owner[0xc7f8 / 4] != 0
        || physical[0x334 / 4] != 0x1c0
        || [0x418, 0x41c, 0x420].iter().any(|at| physical[at / 4] != 0)
        || physical[0x3f0 / 4] != frame.bodies[0].mass.to_bits()
        || (0..4).any(|i| {
            physical[0x210 / 4 + i] != frame.step.bodies[0].contact.angular[i].to_bits()
                || physical[0x220 / 4 + i] != frame.step.bodies[0].contact.linear[i].to_bits()
                || physical[0x1c0 / 4 + i] != frame.step.bodies[0].contact.position[i].to_bits()
        })
        || contacts.iter().zip(&frame.step.contacts).any(|(r, c)| {
            c.two_bodies
                || c.bodies != [0, 0]
                || r[1] != 0
                || r[2] != 0
                || r[3] & 255 > 1
                || c.accumulated.to_bits() != r[0x38 / 4]
                || c.previous_impulse.to_bits() != r[0x3c / 4]
                || (0..4).any(|i| {
                    c.point[i].to_bits() != r[0x50 / 4 + i]
                        || c.normal[i].to_bits() != r[0x60 / 4 + i]
                })
                || [r[0x2c / 4], r[0x28 / 4]]
                    .into_iter()
                    .enumerate()
                    .any(|(i, id)| {
                        id == 0
                            || id as usize > shapes.len()
                            || c.shapes[i] != Some(shapes[id as usize - 1].angular)
                    })
        })
        || limit.corner_scale.to_bits() != bulk.generation[0].to_bits()
        || limit.unlimited.to_bits() != bulk.generation[1].to_bits()
        || limit.damage_scale.to_bits() != bulk.policy_scale.to_bits()
        || limit.table.map(f32::to_bits) != bulk.policy_table.map(f32::to_bits)
        || limit.global_disabled != bulk.global_disabled
    {
        return Err(Error(
            "invalid deforming frame correspondence or aliased globals",
        ));
    }
    let mut physical = *physical;
    physical[0x334 / 4] = 0;
    deformation_preparation::calculate(
        &owner[0x2300 / 4..0xc800 / 4],
        &physical,
        std::array::from_fn(|i| f32::from_bits(physical[0x1c0 / 4 + i])),
        contacts,
        0,
        0,
        0,
        limit,
    )?;
    let context = Context {
        owner: owner.to_vec(),
        physical,
        contacts: contacts.to_vec(),
        settings: settings.to_vec(),
        shapes: shapes.to_vec(),
        limit,
        bulk: bulk.clone(),
        scalar: frame.step.parameters[0],
        random_seed,
        bulk_invoked: false,
        records: 0,
    };
    let (frame, c) = impact_frame::calculate_with_callbacks_rows(frame, rows, context)?;
    Ok(Output {
        frame,
        owner: c.owner,
        contacts: c.contacts,
        random_seed: c.random_seed,
        bulk_invoked: c.bulk_invoked,
        records: c.records,
    })
}
