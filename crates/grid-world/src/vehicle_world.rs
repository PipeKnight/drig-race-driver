//! Single-car manager, wheel contact production and carried serial physical world.
//! Verified against repeated original manager/world calls on owned buffers.
//! Selected serial scope; the live asynchronous frame schedule remains external.
//! Geometry/material queries and source initialization are supplied by the caller.
use crate::{
    Error, body_impact, contact_groups, contact_ray, grouped_physical_frame, physical_world,
    suspension_prepare::Contact, vehicle_contact_force::Material, vehicle_handling,
    vehicle_manager, wheel_contacts,
};

#[derive(Clone)]
pub struct State {
    pub car: vehicle_manager::State,
    pub world: grouped_physical_frame::Output,
    pub shapes: Vec<contact_groups::Shape>,
}

pub struct Output {
    pub state: State,
    pub materials: [Option<Material>; 4],
    /// Occupied transient contact records of this tick (wheel prefix plus
    /// fresh geometry); records past it in `state.world` are stale pool slots.
    pub active_contacts: usize,
}

/// Attach a source factory body to one owned serial world. The factory's initial
/// self-rooted group, active flag, tensors and pose are retained. External world
/// and object links must already have been converted to canonical owned tokens.
pub fn from_registered(
    mut car: vehicle_manager::State,
    manager: grouped_physical_frame::Manager,
    static_shapes: usize,
) -> Result<State, Error> {
    let p = &mut car.owner.physical;
    if !(1..=512).contains(&static_shapes)
        || car.owner.vehicle.len() != crate::vehicle_publication::VEHICLE_WORDS
        || car.owner.vehicle[..car.frame.body.len()] != car.frame.body
        || [8, 0xc, 0x18, 0x20].iter().any(|&o| p[o / 4] != 1)
        || [
            0, 4, 0x10, 0x14, 0x1c, 0x24, 0x28, 0x2f0, 0x2f4, 0x2fc, 0x300, 0x304, 0x308, 0x30c,
            0x318, 0x320, 0x324, 0x328, 0x32c, 0x334, 0x338, 0x33c, 0x340, 0x400,
        ]
        .iter()
        .any(|&o| p[o / 4] != 0)
        || p[0x2c / 4] & 0xffff != 1
        || p[0x410 / 4] & 0xffff0000 != 0
        || p[0x414 / 4] & 0xff00 != 0
    {
        return Err(Error("invalid registered single-car world attachment"));
    }
    p[0x334 / 4] = 0x1c0;
    let mut shapes = vec![contact_groups::Shape {
        flag229: true,
        ..Default::default()
    }];
    shapes.extend((0..static_shapes).map(|_| contact_groups::Shape {
        angular: true,
        ..Default::default()
    }));
    Ok(State {
        world: grouped_physical_frame::Output {
            groups: contact_groups::Output {
                bodies: vec![car.owner.physical],
                contacts: vec![[0; 32]; 4],
                world: [0, 0, 0, 0, 0, 0, 1, 1, 0, 1, 0],
            },
            manager,
            pool_used: 0,
            visits: Vec::new(),
        },
        car,
        shapes,
    })
}

/// One common positive timestep: registered car manager (including contacts),
/// then the selected complete serial physical world and pool consumption.
/// Complete car/world/pool/RNG/manager state remains unchanged on any failure.
pub fn calculate(
    input: &State,
    pair: [usize; 2],
    gravity: f32,
    handling_queries: Option<&[vehicle_handling::QueryRecord]>,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    material: impl FnMut(u32) -> Result<Option<Material>, Error>,
) -> Result<Output, Error> {
    calculate_owned(
        input,
        pair,
        gravity,
        handling_queries,
        query,
        material,
        None,
        None,
        None,
        None,
    )
}

/// Selected world with the verified body handler on every occupied contact.
/// Static peers have no registrations; notification slots are explicitly empty.
#[allow(clippy::too_many_arguments)]
pub fn calculate_with_events(
    input: &State,
    pair: [usize; 2],
    gravity: f32,
    handling_queries: Option<&[vehicle_handling::QueryRecord]>,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    material: impl FnMut(u32) -> Result<Option<Material>, Error>,
    events: Option<body_impact::Parameters>,
) -> Result<Output, Error> {
    calculate_owned(
        input,
        pair,
        gravity,
        handling_queries,
        query,
        material,
        None,
        events,
        None,
        None,
    )
}

/// Registered car/wheel job followed by advanced-pose body geometry and solving.
/// The stable full pool may have up to 64 slots. Only its first four slots are
/// exposed to wheel production; fresh geometric records follow the used prefix.
#[allow(clippy::too_many_arguments)]
pub fn calculate_with_geometry(
    input: &State,
    pair: [usize; 2],
    gravity: f32,
    handling_queries: Option<&[vehicle_handling::QueryRecord]>,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    material: impl FnMut(u32) -> Result<Option<Material>, Error>,
    mut geometry: impl FnMut(
        &[contact_groups::Body],
        usize,
    ) -> Result<Vec<contact_groups::Contact>, Error>,
) -> Result<Output, Error> {
    calculate_owned(
        input,
        pair,
        gravity,
        handling_queries,
        query,
        material,
        Some(&mut geometry),
        None,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn calculate_with_geometry_events(
    input: &State,
    pair: [usize; 2],
    gravity: f32,
    handling_queries: Option<&[vehicle_handling::QueryRecord]>,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    material: impl FnMut(u32) -> Result<Option<Material>, Error>,
    mut geometry: impl FnMut(
        &[contact_groups::Body],
        usize,
    ) -> Result<Vec<contact_groups::Contact>, Error>,
    events: Option<body_impact::Parameters>,
) -> Result<Output, Error> {
    calculate_owned(
        input,
        pair,
        gravity,
        handling_queries,
        query,
        material,
        Some(&mut geometry),
        events,
        None,
        None,
    )
}

/// Explicit source-resource publication at the original car-job boundary.
/// External geometry must be staged by the caller until this whole world returns.
#[allow(clippy::too_many_arguments)]
pub fn calculate_with_geometry_events_publication(
    input: &State,
    pair: [usize; 2],
    gravity: f32,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    material: impl FnMut(u32) -> Result<Option<Material>, Error>,
    mut geometry: impl FnMut(
        &[contact_groups::Body],
        usize,
    ) -> Result<Vec<contact_groups::Contact>, Error>,
    events: Option<body_impact::Parameters>,
    publication: &mut crate::vehicle_job::Publication<'_>,
) -> Result<Output, Error> {
    calculate_owned(
        input,
        pair,
        gravity,
        None,
        query,
        material,
        Some(&mut geometry),
        events,
        Some(publication),
        None,
    )
}

/// Both physical deformation phases and staged source publication. Registration
/// is copied and returned only with a successful complete world.
#[allow(clippy::too_many_arguments)]
pub fn calculate_with_deformation(
    input: &State,
    pair: [usize; 2],
    gravity: f32,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    material: impl FnMut(u32) -> Result<Option<Material>, Error>,
    mut geometry: impl FnMut(
        &[contact_groups::Body],
        usize,
    ) -> Result<Vec<contact_groups::Contact>, Error>,
    events: Option<body_impact::Parameters>,
    publication: &mut crate::vehicle_job::Publication<'_>,
    registration: &crate::deformation_world::Registration,
) -> Result<(Output, crate::deformation_world::Registration), Error> {
    let mut next = registration.clone();
    let output = calculate_owned(
        input,
        pair,
        gravity,
        None,
        query,
        material,
        Some(&mut geometry),
        events,
        Some(publication),
        Some(&mut next),
    )?;
    Ok((output, next))
}

#[allow(clippy::too_many_arguments)]
fn calculate_owned(
    input: &State,
    pair: [usize; 2],
    gravity: f32,
    handling_queries: Option<&[vehicle_handling::QueryRecord]>,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    material: impl FnMut(u32) -> Result<Option<Material>, Error>,
    geometry: Option<&mut grouped_physical_frame::GeometryProvider<'_>>,
    events: Option<body_impact::Parameters>,
    publication: Option<&mut crate::vehicle_job::Publication<'_>>,
    mut registration: Option<&mut crate::deformation_world::Registration>,
) -> Result<Output, Error> {
    // Body 0 is the car. With a geometry provider, further dynamic bodies
    // (broken route props) may follow; their shapes trail the static shapes.
    let static_shape = contact_groups::Shape {
        angular: true,
        ..Default::default()
    };
    let dynamic_shape = contact_groups::Shape {
        flag229: true,
        ..Default::default()
    };
    let statics = input.shapes[1.min(input.shapes.len())..]
        .iter()
        .take_while(|s| **s == static_shape)
        .count();
    if input.world.groups.bodies.is_empty()
        || (input.world.groups.bodies.len() > 1 && geometry.is_none())
        || input.car.owner.physical != input.world.groups.bodies[0]
        || input.world.pool_used != 0
        || input.world.groups.contacts.len()
            > if geometry.is_none() {
                4
            } else if input.world.groups.bodies.len() > 1 {
                crate::MAX_WORLD_CONTACTS
            } else {
                64
            }
        || input.shapes.len() < 2
        || input.shapes.len() > 513
        || statics == 0
        || input.shapes[0] != dynamic_shape
        || input.shapes[1 + statics..]
            .iter()
            .any(|s| *s != dynamic_shape)
    {
        return Err(Error("inconsistent single-car physical world ownership"));
    }
    let pool = wheel_contacts::Pool {
        slots: input.world.groups.contacts[..input.world.groups.contacts.len().min(4)].to_vec(),
        used: 0,
        shapes: statics,
    };
    let manager = if let Some(publication) = publication {
        vehicle_manager::calculate_with_world_publication(
            &input.car,
            &pool,
            pair,
            input.world.manager.parameters[0],
            gravity,
            handling_queries,
            query,
            material,
            publication,
        )?
    } else {
        vehicle_manager::calculate_with_world_contacts(
            &input.car,
            &pool,
            pair,
            input.world.manager.parameters[0],
            gravity,
            handling_queries,
            query,
            material,
        )?
    };
    let mut previous = input.world.clone();
    previous.groups.bodies[0] = manager.job.state.owner.physical;
    if let Some(registration) = registration.as_deref_mut() {
        registration.owner = manager.job.state.owner.vehicle.clone();
    }
    let mut active_contacts = manager.contacts.used;
    let world = if let Some(geometry) = geometry {
        let mut slots = input.world.groups.contacts.clone();
        slots[..manager.contacts.slots.len()].copy_from_slice(&manager.contacts.slots);
        let mut provider = |bodies: &[contact_groups::Body], remaining| {
            let contacts = geometry(bodies, remaining)?;
            active_contacts += contacts.len();
            Ok(contacts)
        };
        if let Some(registration) = registration.as_deref_mut() {
            // One registration slot per body; further bodies do not deform.
            let mut registrations = vec![Some(registration)];
            registrations.extend((1..previous.groups.bodies.len()).map(|_| None));
            physical_world::advance_with_deformation(
                &previous,
                &slots,
                manager.contacts.used,
                &input.shapes,
                &mut provider,
                &mut registrations,
            )?
        } else {
            physical_world::advance_with_geometry_provider(
                &previous,
                &slots,
                manager.contacts.used,
                &input.shapes,
                provider,
            )?
        }
    } else {
        physical_world::advance_with_pool(
            &previous,
            &manager.contacts.slots,
            manager.contacts.used,
            &input.shapes,
        )?
    };
    let mut car = manager.job.state;
    if let Some(registration) = registration.as_deref_mut() {
        car.owner.vehicle = registration.owner.clone();
    }
    car.owner.physical = world.groups.bodies[0];
    if let Some(parameters) = events {
        if active_contacts > world.groups.contacts.len() || handling_queries.is_some() {
            return Err(Error("unsupported body event pool or query ownership"));
        }
        for contact in &world.groups.contacts[..active_contacts] {
            let side = match [contact[0x28 / 4] == 1, contact[0x2c / 4] == 1] {
                [true, false] => 0,
                [false, true] => 1,
                // Contacts between other dynamic bodies (broken props).
                [false, false] if input.world.groups.bodies.len() > 1 => continue,
                _ => return Err(Error("unsupported body event registrations")),
            };
            let result = body_impact::calculate(
                &car.owner,
                body_impact::Event {
                    point: std::array::from_fn(|j| f32::from_bits(contact[0x50 / 4 + j])),
                    normal: std::array::from_fn(|j| f32::from_bits(contact[0x60 / 4 + j])),
                    impact: f32::from_bits(contact[0x3c / 4]),
                    parameter: world.manager.parameters[0],
                    other_registration: 0,
                    other_word: contact[if side == 0 { 0x34 / 4 } else { 0x30 / 4 }],
                },
                side,
                parameters,
                &[None; 11],
                car.owner
                    .damage_random_state
                    .ok_or(Error("body impact requires owned damage RNG"))?,
            )?;
            car.owner = result.state;
            car.owner.damage_random_state = Some(result.random_state);
        }
        car.frame
            .body
            .copy_from_slice(&car.owner.vehicle[..crate::vehicle_force::BODY_WORDS]);
    }
    if let Some(registration) = registration {
        car.frame
            .body
            .copy_from_slice(&car.owner.vehicle[..crate::vehicle_force::BODY_WORDS]);
        registration.owner = car.owner.vehicle.clone();
    }
    Ok(Output {
        state: State {
            car,
            world,
            shapes: input.shapes.clone(),
        },
        materials: manager.job.materials,
        active_contacts,
    })
}
