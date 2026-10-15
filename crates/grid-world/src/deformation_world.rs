//! Explicit car registration for the carried physical world: one car with
//! static peers, or several touching cars in one contact group.
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
    /// Process-global bulk owner-order alternation (F5E1A4), shared like the seed.
    pub owner_toggle: bool,
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
    // The frame sees the owner with its +C7F8 link cleared; the registration
    // is restored before any failure returns, so it stays unchanged.
    registration.owner[0xc7f8 / 4] = 0;
    let (owner, settings) = (
        &registration.owner,
        &registration.owner[0xd240 / 4..(0xd240 + 0x1054) / 4],
    );
    let result = deforming_physical_frame::calculate(deforming_physical_frame::Input {
        frame,
        owner,
        physical,
        contacts: &raw,
        settings,
        shapes,
        limit: registration.limit,
        bulk: &registration.bulk,
        random_seed: registration.random_seed,
    });
    registration.owner[0xc7f8 / 4] = 0xd240;
    let result = result?;
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

/// A contact group of several bodies with at least one registration
/// (`registrations` in local body order). Contacts keep their world tokens;
/// +38/+3C/+40/+44 and the +0E byte are returned.
pub(crate) fn calculate_multi_group(
    frame: &impact_frame::Input,
    bodies: &[&contact_groups::Body],
    contacts: &[contact_groups::Contact],
    shapes: &[contact_groups::Shape],
    registrations: &mut [Option<&mut Registration>],
    random_seed: u32,
    owner_toggle: bool,
) -> Result<
    (
        impact_frame::Output,
        Vec<contact_groups::Contact>,
        u32,
        bool,
    ),
    Error,
> {
    if registrations.len() != bodies.len() || contacts.iter().any(|r| r[0xc / 4] & 255 > 1) {
        return Err(Error("invalid deforming group registrations"));
    }
    let mut owners = Vec::new();
    for (local, registration) in registrations.iter().enumerate() {
        let Some(registration) = registration else {
            continue;
        };
        let owner = &registration.owner;
        if owner.len() != crate::vehicle_publication::VEHICLE_WORDS
            || owner[0xdb0 / 4] != 0
            || owner[0xc7f8 / 4] != 0xd240
        {
            return Err(Error("invalid carried deformation registration"));
        }
        let mut owner = owner.clone();
        owner[0xc7f8 / 4] = 0;
        owners.push(crate::deforming_group_frame::Owner {
            local,
            settings: owner[0xd240 / 4..(0xd240 + 0x1054) / 4].to_vec(),
            owner,
            physical: *bodies[local],
            limit: registration.limit,
            bulk: registration.bulk.clone(),
            bulk_invoked: false,
            records: 0,
        });
    }
    let result = crate::deforming_group_frame::calculate(
        frame,
        owners,
        bodies.iter().map(|b| **b).collect(),
        contacts,
        shapes,
        random_seed,
        owner_toggle,
    )?;
    for o in result.owners {
        let registration = registrations[o.local]
            .as_deref_mut()
            .expect("registered owner");
        registration.owner = o.owner;
        registration.owner[0xc7f8 / 4] = 0xd240;
        registration.bulk_calls += u64::from(o.bulk_invoked);
        registration.records += o.records as u64;
    }
    let mut returned = contacts.to_vec();
    for (output, raw) in returned.iter_mut().zip(&result.contacts) {
        for at in [0x38, 0x3c, 0x40, 0x44] {
            output[at / 4] = raw[at / 4];
        }
        output[0xc / 4] = (output[0xc / 4] & !0xff_0000) | (raw[0xc / 4] & 0xff_0000);
    }
    Ok((result.frame, returned, result.random_seed, result.toggle))
}
