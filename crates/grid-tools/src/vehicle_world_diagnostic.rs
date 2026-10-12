//! Bounded private wire adapter for the single-car serial-world comparison.
use grid_world::{
    Error, contact_groups, contact_ray, grouped_physical_frame, impact_frame,
    suspension_prepare::Contact, vehicle_contact_force::Material, vehicle_handling,
    vehicle_manager, vehicle_world, wheel_contacts,
};

pub const SIZE: usize = 3500;
fn u(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn f(b: &[u8], o: usize) -> f32 {
    f32::from_bits(u(b, o))
}
fn fs<const N: usize>(b: &[u8], o: usize) -> [f32; N] {
    std::array::from_fn(|i| f(b, o + 4 * i))
}
fn store(b: &mut [u8], o: usize, values: impl IntoIterator<Item = u32>) {
    for (i, v) in values.into_iter().enumerate() {
        b[o + 4 * i..o + 4 * i + 4].copy_from_slice(&v.to_le_bytes());
    }
}

#[allow(clippy::too_many_arguments)]
pub fn calculate(
    car: vehicle_manager::State,
    pool: &mut wheel_contacts::Pool,
    wire: &mut [u8],
    delta: f32,
    pair: [usize; 2],
    gravity: f32,
    queries: Option<&[vehicle_handling::QueryRecord]>,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    material: impl FnMut(u32) -> Result<Option<Material>, Error>,
) -> Result<vehicle_manager::Output, Error> {
    calculate_owned(
        car, pool, wire, delta, pair, gravity, queries, query, material, false,
    )
}

/// Source factory's initial self-rooted group and active-body flag retained.
#[allow(clippy::too_many_arguments)]
pub fn calculate_registered(
    car: vehicle_manager::State,
    pool: &mut wheel_contacts::Pool,
    wire: &mut [u8],
    delta: f32,
    pair: [usize; 2],
    gravity: f32,
    queries: Option<&[vehicle_handling::QueryRecord]>,
    query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    material: impl FnMut(u32) -> Result<Option<Material>, Error>,
) -> Result<vehicle_manager::Output, Error> {
    calculate_owned(
        car, pool, wire, delta, pair, gravity, queries, query, material, true,
    )
}

#[allow(clippy::too_many_arguments)]
fn calculate_owned(
    mut car: vehicle_manager::State,
    pool: &mut wheel_contacts::Pool,
    wire: &mut [u8],
    delta: f32,
    pair: [usize; 2],
    gravity: f32,
    queries: Option<&[vehicle_handling::QueryRecord]>,
    mut query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
    mut material: impl FnMut(u32) -> Result<Option<Material>, Error>,
    registered: bool,
) -> Result<vehicle_manager::Output, Error> {
    if wire.len() != SIZE {
        return Err(Error("invalid vehicle world wire size"));
    }
    let steps = u(wire, 0) as usize;
    let p = fs::<11>(wire, 4);
    let physical = &mut car.owner.physical;
    let links = [
        0, 4, 8, 0xc, 0x10, 0x14, 0x18, 0x1c, 0x28, 0x2f0, 0x2f4, 0x2fc, 0x300, 0x304, 0x308,
        0x30c, 0x318, 0x320, 0x324, 0x328, 0x32c, 0x334, 0x338, 0x33c, 0x340, 0x400,
    ];
    let byte = |o: usize| (physical[o / 4] >> (8 * (o % 4))) as u8;
    if !(1..=if registered { 4096 } else { 64 }).contains(&steps)
        || delta <= 0.
        || p[0].to_bits() != delta.to_bits()
        || p.iter().any(|x| !x.is_finite())
        || pool.used != 0
        || pool.slots.len() != 4
        || !(1..=512).contains(&pool.shapes)
        || links
            .iter()
            .any(|&o| physical[o / 4] != u32::from(registered && [8, 0xc, 0x18].contains(&o)))
        || [0x2c, 0x2d, 0x412, 0x413, 0x415]
            .iter()
            .any(|&o| byte(o) != u8::from(registered && o == 0x2c))
        || physical[0x3b4 / 4] != f32::MAX.to_bits()
        || wire[3368..].iter().any(|&v| v != 0)
        || u(wire, 48) > 32
        || (0..4).any(|i| u(wire, 3252 + i * 4) > 16)
        || (52..3192).step_by(4).any(|o| !f(wire, o).is_finite())
        || (3200..3252).step_by(4).any(|o| !f(wire, o).is_finite())
        || (3268..3368).step_by(4).any(|o| !f(wire, o).is_finite())
    {
        return Err(Error("invalid cold vehicle world ownership"));
    }
    let cached_pair = match [u(wire, 3192), u(wire, 3196)] {
        [u32::MAX, u32::MAX] => None,
        [0, 0] => Some([0, 0]),
        _ => return Err(Error("invalid vehicle world material cache")),
    };
    physical[0x334 / 4] = 0x1c0;
    let manager = grouped_physical_frame::Manager {
        parameters: [p[0], 1., 1., 1., 1., p[5], p[6], p[7], p[8], p[9], p[10]],
        gravity: [p[1], p[2], p[3], p[4]],
        correction: Some(impact_frame::Correction {
            iterations: u(wire, 48) as usize,
            softness: f(wire, 52),
            scale: f(wire, 56),
            threshold: f(wire, 60),
            relaxation: f(wire, 64),
        }),
        materials: vec![fs(wire, 68)],
        rows: (0..16)
            .map(|r| std::array::from_fn(|i| u(wire, 120 + r * 192 + i * 4)))
            .collect(),
        capacity: 16,
        iterations: 4,
        ends: std::array::from_fn(|i| u(wire, 3252 + i * 4) as usize),
        properties: vec![(
            f(wire, 3268),
            std::array::from_fn(|i| fs(wire, 3272 + i * 16)),
        )],
        workspace: vec![[fs(wire, 3336), fs(wire, 3352)]],
        cached_pair,
        cached_material: fs(wire, 3200),
    };
    let mut shapes = vec![contact_groups::Shape {
        flag229: true,
        ..Default::default()
    }];
    shapes.extend((0..pool.shapes).map(|_| contact_groups::Shape {
        angular: true,
        ..Default::default()
    }));
    let mut state = if registered {
        // from_registered owns the center rebinding, without resetting groups.
        car.owner.physical[0x334 / 4] = 0;
        let mut state = vehicle_world::from_registered(car, manager, pool.shapes)?;
        state.world.groups.contacts = pool.slots.clone();
        state
    } else {
        vehicle_world::State {
            world: grouped_physical_frame::Output {
                groups: contact_groups::Output {
                    bodies: vec![car.owner.physical],
                    contacts: pool.slots.clone(),
                    world: [0, 0, 0, 0, 0, 0, 1, 1, 0, 1, 0],
                },
                manager,
                pool_used: 0,
                visits: Vec::new(),
            },
            car,
            shapes,
        }
    };
    let mut row_refs = [0u32; 16];
    let mut materials = [None; 4];
    for step in 0..steps {
        let result =
            vehicle_world::calculate(&state, pair, gravity, queries, &mut query, &mut material)
                .map_err(|error| {
                    eprintln!("vehicle world step {}/{}: {}", step + 1, steps, error);
                    error
                })?;
        state = result.state;
        materials = result.materials;
        let count = state
            .world
            .visits
            .first()
            .ok_or(Error("missing vehicle world visit"))?
            .2;
        let constructed: usize = state.world.groups.contacts[..count]
            .iter()
            .map(|c| c[0x14 / 4] as usize)
            .sum();
        let normals: usize = state.world.groups.contacts[..count]
            .iter()
            .map(|c| c[0x18 / 4] as usize)
            .sum();
        if constructed > 16 || normals > constructed {
            return Err(Error("invalid constructed vehicle world rows"));
        }
        for (i, r) in row_refs[..constructed].iter_mut().enumerate() {
            *r = if i >= normals { normals as u32 } else { 0 };
        }
    }
    let m = &state.world.manager;
    for (i, row) in m.rows.iter().enumerate() {
        store(wire, 120 + i * 192, *row);
    }
    store(
        wire,
        3192,
        m.cached_pair.map_or([u32::MAX; 2], |v| v.map(|x| x as u32)),
    );
    store(wire, 3200, m.cached_material.map(f32::to_bits));
    store(wire, 3252, m.ends.map(|v| v as u32));
    store(wire, 3268, [m.properties[0].0.to_bits()]);
    store(
        wire,
        3272,
        m.properties[0].1.into_iter().flatten().map(f32::to_bits),
    );
    store(
        wire,
        3336,
        m.workspace[0].into_iter().flatten().map(f32::to_bits),
    );
    store(wire, 3368, state.world.groups.world);
    store(wire, 3412, [0, delta.to_bits(), 0, 0, 0, 0]);
    store(wire, 3436, row_refs);
    pool.slots = state.world.groups.contacts;
    pool.used = 0;
    // Normalize the owned center identity only at the private output boundary.
    state.car.owner.physical[0x334 / 4] = 0;
    Ok(vehicle_manager::Output {
        state: state.car,
        materials,
    })
}
