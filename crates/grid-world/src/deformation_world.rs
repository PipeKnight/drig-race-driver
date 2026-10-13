//! Explicit single-car/static-peer registration for the carried physical world.
use crate::{
    Error, contact_groups, deformation_callback_state, deformation_contact_limit,
    deforming_physical_frame, impact_frame,
};
#[derive(Clone)]
pub struct Registration {
    pub owner: Vec<u32>,
    pub limit: deformation_contact_limit::Parameters,
    pub bulk: deformation_callback_state::Parameters,
    /// Independent CRT stream; never aliases the body-event global RNG.
    pub random_seed: u32,
    pub bulk_calls: u64,
    pub records: u64,
}
pub(crate) fn calculate_group(
    frame: &impact_frame::Input,
    physical: &contact_groups::Body,
    contacts: &[contact_groups::Contact],
    shapes: &[contact_groups::Shape],
    registration: &mut Registration,
) -> Result<(impact_frame::Output, Vec<contact_groups::Contact>), Error> {
    let owner = &registration.owner;
    if owner.len() != crate::vehicle_publication::VEHICLE_WORDS
        || owner[0xdb0 / 4] != 0
        || owner[0xc7f8 / 4] != 0xd240
        || contacts
            .iter()
            .any(|r| r[1] != 1 || r[2] != 0 || r[0xc / 4] & 255 > 1)
    {
        return Err(Error("invalid carried deformation registration"));
    }
    let settings = owner[0xd240 / 4..(0xd240 + 0x1054) / 4].to_vec();
    let mut owner = owner.clone();
    owner[0xc7f8 / 4] = 0;
    let raw: Vec<_> = contacts
        .iter()
        .map(|r| {
            let mut c = *r;
            c[1] = 0;
            c[2] = 0;
            c[0x1c / 4] = 0;
            c[0x20 / 4] = 0;
            c
        })
        .collect();
    let result = deforming_physical_frame::calculate(deforming_physical_frame::Input {
        frame,
        owner: &owner,
        physical,
        contacts: &raw,
        settings: &settings,
        shapes,
        limit: registration.limit,
        bulk: &registration.bulk,
        random_seed: registration.random_seed,
    })?;
    let mut returned = contacts.to_vec();
    for (output, raw) in returned.iter_mut().zip(&result.contacts) {
        for at in [0x38, 0x3c, 0x40, 0x44] {
            output[at / 4] = raw[at / 4];
        }
    }
    registration.owner = result.owner;
    registration.owner[0xc7f8 / 4] = 0xd240;
    registration.random_seed = result.random_seed;
    registration.bulk_calls += u64::from(result.bulk_invoked);
    registration.records += result.records as u64;
    Ok((result.frame, returned))
}
