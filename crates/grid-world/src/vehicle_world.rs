//! Single-car manager, wheel contact production and carried serial physical world.
//! Verified against repeated original manager/world calls on owned buffers.
//! Selected serial scope; the live asynchronous frame schedule remains external.
//! Geometry/material queries and source initialization are supplied by the caller.
use crate::{
    Error, contact_groups, contact_ray, grouped_physical_frame, physical_world,
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
    if input.world.groups.bodies.len() != 1
        || input.car.owner.physical != input.world.groups.bodies[0]
        || input.world.pool_used != 0
        || input.world.groups.contacts.len() > 4
        || input.shapes.len() < 2
        || input.shapes.len() > 513
        || input.shapes[0]
            != (contact_groups::Shape {
                angular: false,
                flag217: false,
                flag229: true,
                flag230: false,
            })
        || input.shapes[1..].iter().any(|s| {
            *s != (contact_groups::Shape {
                angular: true,
                ..Default::default()
            })
        })
    {
        return Err(Error("inconsistent single-car physical world ownership"));
    }
    let pool = wheel_contacts::Pool {
        slots: input.world.groups.contacts.clone(),
        used: 0,
        shapes: input.shapes.len() - 1,
    };
    let manager = vehicle_manager::calculate_with_world_contacts(
        &input.car,
        &pool,
        pair,
        input.world.manager.parameters[0],
        gravity,
        handling_queries,
        query,
        material,
    )?;
    let mut previous = input.world.clone();
    previous.groups.bodies[0] = manager.job.state.owner.physical;
    let world = physical_world::advance_with_pool(
        &previous,
        &manager.contacts.slots,
        manager.contacts.used,
        &input.shapes,
    )?;
    let mut car = manager.job.state;
    car.owner.physical = world.groups.bodies[0];
    Ok(Output {
        state: State {
            car,
            world,
            shapes: input.shapes.clone(),
        },
        materials: manager.job.materials,
    })
}
