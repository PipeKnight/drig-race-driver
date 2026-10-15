//! Several source cars in one shared physical world (opponents). Each car keeps
//! its own source state, wheel query cache and lap timing; per tick every car
//! manager runs with its own controls and track queries, its wheel contacts are
//! re-identified to its world body and car shape, and one common advance solves
//! all bodies with track, route-prop and car-to-car body contacts together.
//! Selected scope: body events dispatch per car with no peer registration;
//! deformation registration is single-owner and not enabled here.
use super::*;
use grid_world::{body_impact, contact_groups, physical_world, wheel_contacts};

pub struct MultiRuntime {
    pub cars: Vec<Runtime>,
    geometry: Vec<crate::body_geometry::BodyGeometry>,
    world: grouped_physical_frame::Output,
    shapes: Vec<contact_groups::Shape>,
    statics: usize,
    initial_world: grouped_physical_frame::Output,
    pub ticks: u64,
    /// Track/prop/boundary body records and car-to-car records of the last tick.
    pub last_body_contacts: usize,
    pub last_car_contacts: usize,
}

const POOL: usize = 64;

impl MultiRuntime {
    /// Source cars on the named grid slots; `table` is the private CD01 body
    /// sampling input shared by every car.
    pub fn lacetti(
        root: &Path,
        slots: &[&str],
        table: &[u8],
        events: Option<body_impact::Parameters>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if !(1..=8).contains(&slots.len()) {
            return Err("unsupported shared-world car count".into());
        }
        let mut cars = Vec::new();
        let mut geometry = Vec::new();
        for (i, slot) in slots.iter().enumerate() {
            let mut car = Runtime::lacetti(root, slot, 200679 + i as u32)?;
            geometry.push(car.prepare_body_geometry(root, table)?);
            car.body_events = events;
            car.enable_lap_timing(root)?;
            cars.push(car);
        }
        let first = &cars[0];
        let statics = first.state.shapes.len() - 1;
        let mut shapes = first.state.shapes.clone();
        for _ in 1..cars.len() {
            shapes.push(contact_groups::Shape {
                flag229: true,
                ..Default::default()
            });
        }
        let mut world = first.state.world.clone();
        let n = cars.len();
        world.groups.bodies = cars.iter().map(|c| c.state.car.owner.physical).collect();
        // One active chain over all cars: heads0 = 1, tails0 = n.
        for i in 0..n {
            world.groups.bodies[i][0x300 / 4] = if i + 1 < n { i as u32 + 2 } else { 0 };
            world.groups.bodies[i][0x304 / 4] = i as u32;
        }
        world.groups.world = [0, 0, 0, 0, 0, 0, 1, 1, 0, n as u32, 0];
        world.groups.contacts.resize(POOL, [0; 32]);
        world.manager.rows.resize(POOL * 4, [0; 48]);
        world.manager.capacity = POOL * 4;
        world.manager.properties = cars
            .iter()
            .map(|c| c.state.world.manager.properties[0])
            .collect();
        world.manager.workspace = vec![[[0.; 4]; 2]; n];
        Ok(Self {
            cars,
            geometry,
            initial_world: world.clone(),
            world,
            shapes,
            statics,
            ticks: 0,
            last_body_contacts: 0,
            last_car_contacts: 0,
        })
    }

    /// World shape token (one-based) of car `i`'s body shape.
    fn token(&self, i: usize) -> u32 {
        if i == 0 {
            1
        } else {
            (self.statics + 1 + i) as u32
        }
    }

    pub fn reset(&mut self) {
        for car in &mut self.cars {
            car.reset();
        }
        self.world = self.initial_world.clone();
        self.ticks = 0;
        self.last_body_contacts = 0;
        self.last_car_contacts = 0;
    }

    /// One common timestep for all cars. Every car, the shared world and all
    /// query caches stay unchanged if any stage fails.
    pub fn tick(&mut self, controls: &[Controls]) -> Result<(), Error> {
        let n = self.cars.len();
        if controls.len() != n {
            return Err(Error("one control set per car is required"));
        }
        let tokens: Vec<u32> = (0..n).map(|i| self.token(i)).collect();
        let mut previous = self.world.clone();
        let mut slots = self.world.groups.contacts.clone();
        let mut used = 0;
        let mut staged = Vec::with_capacity(n);
        let mut caches = Vec::with_capacity(n);
        for (i, car) in self.cars.iter().enumerate() {
            let mut state = car.staged_controls(controls[i])?;
            state.car.owner.physical = previous.groups.bodies[i];
            state
                .car
                .frame
                .body
                .copy_from_slice(&state.car.owner.vehicle[..vehicle_force::BODY_WORDS]);
            let mut cache = car.cache;
            let query = |_, ray: grid_world::contact_ray::Ray| {
                car.track
                    .query(ray.start, ray.end, &mut cache)?
                    .hit
                    .map(|h| {
                        let (index, material) = car
                            .surfaces
                            .contact(h.contact.material)
                            .map_err(|_| Error("missing source mechanics"))?;
                        Ok(suspension_prepare::Contact {
                            normal: h.contact.normal,
                            fraction: h.contact.fraction,
                            material,
                            tokens: [index as u32 + 1, h.shape + 1, 1],
                        })
                    })
                    .transpose()
            };
            let material = |token| car.surfaces.joined(token);
            if used + 4 > POOL {
                return Err(Error("shared wheel contact pool exhausted"));
            }
            let pool = wheel_contacts::Pool {
                slots: slots[used..used + 4].to_vec(),
                used: 0,
                shapes: self.statics,
            };
            let manager = vehicle_manager::calculate_with_world_contacts(
                &state.car,
                &pool,
                car.pair,
                previous.manager.parameters[0],
                GRAVITY,
                None,
                query,
                material,
            )?;
            for k in 0..manager.contacts.used {
                let mut c = manager.contacts.slots[k];
                c[1] = i as u32 + 1;
                c[0x28 / 4] = tokens[i];
                slots[used + k] = c;
            }
            used += manager.contacts.used;
            previous.groups.bodies[i] = manager.job.state.owner.physical;
            state.car = manager.job.state;
            staged.push(state);
            caches.push(cache);
        }
        let wheel_contacts = used;
        let mut body_contacts = 0;
        let mut car_contacts = 0;
        let geometry = &self.geometry;
        let cars = &self.cars;
        let provider = |bodies: &[contact_groups::Body], remaining: usize| {
            let mut out: Vec<contact_groups::Contact> = Vec::new();
            for i in 0..bodies.len() {
                if out.len() >= remaining {
                    break;
                }
                let fresh = geometry[i].contacts(
                    &cars[i].track,
                    &cars[i].surfaces,
                    &bodies[i..=i],
                    (remaining - out.len()).min(32),
                )?;
                for mut c in fresh {
                    c[1] = i as u32 + 1;
                    c[0x28 / 4] = tokens[i];
                    out.push(c);
                }
            }
            body_contacts = out.len();
            for i in 0..bodies.len() {
                for j in i + 1..bodies.len() {
                    if out.len() >= remaining {
                        break;
                    }
                    let other = geometry[j].hull_world_triangles(&bodies[j])?;
                    let pair =
                        geometry[i].car_contacts(&bodies[i], &other, remaining - out.len())?;
                    for hit in pair {
                        let mut c = [0; 32];
                        c[1] = i as u32 + 1;
                        c[2] = j as u32 + 1;
                        c[0xc / 4] = 0x01000000;
                        c[0x1c / 4] = u32::MAX;
                        c[0x20 / 4] = u32::MAX;
                        c[0x24 / 4] = 1;
                        c[0x28 / 4] = tokens[i];
                        c[0x2c / 4] = tokens[j];
                        c[0x38 / 4] = hit.penetration.to_bits();
                        c[0x3c / 4] = f32::MAX.to_bits();
                        c[0x50 / 4..0x60 / 4].copy_from_slice(&hit.point.map(f32::to_bits));
                        c[0x60 / 4..0x70 / 4].copy_from_slice(&hit.normal.map(f32::to_bits));
                        c[0x70 / 4] = (u32::from(hit.flipped) << 8) | (1 << 24);
                        out.push(c);
                        car_contacts += 1;
                    }
                }
            }
            Ok(out)
        };
        let world = physical_world::advance_with_geometry_provider(
            &previous,
            &slots,
            wheel_contacts,
            &self.shapes,
            provider,
        )?;
        let active = wheel_contacts + body_contacts + car_contacts;
        for (i, state) in staged.iter_mut().enumerate() {
            state.car.owner.physical = world.groups.bodies[i];
            if let Some(parameters) = self.cars[i].body_events {
                for contact in &world.groups.contacts[..active.min(world.groups.contacts.len())] {
                    let side = match [
                        contact[0x28 / 4] == tokens[i],
                        contact[0x2c / 4] == tokens[i],
                    ] {
                        [true, false] => 0,
                        [false, true] => 1,
                        _ => continue,
                    };
                    let result = body_impact::calculate(
                        &state.car.owner,
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
                        state
                            .car
                            .owner
                            .damage_random_state
                            .ok_or(Error("body impact requires owned damage RNG"))?,
                    )?;
                    state.car.owner = result.state;
                    state.car.owner.damage_random_state = Some(result.random_state);
                }
                state
                    .car
                    .frame
                    .body
                    .copy_from_slice(&state.car.owner.vehicle[..vehicle_force::BODY_WORDS]);
            }
        }
        // Lap timing per car, staged before any commit.
        let mut laps = Vec::with_capacity(n);
        for (i, car) in self.cars.iter().enumerate() {
            laps.push(
                if let (Some(course), Some(lap)) = (&car.lap_course, &car.lap) {
                    let position = std::array::from_fn(|k| {
                        f32::from_bits(staged[i].car.owner.physical[0x1d0 / 4 + 12 + k])
                    });
                    Some(lap.advance(
                        course,
                        position,
                        (self.ticks + 1) as f64 * f64::from(DELTA),
                    )?)
                } else {
                    None
                },
            );
        }
        for (i, ((state, cache), timed)) in staged.into_iter().zip(caches).zip(laps).enumerate() {
            let car = &mut self.cars[i];
            car.state.car = state.car;
            car.cache = cache;
            car.ticks += 1;
            car.lap_events.clear();
            if let Some(timed) = timed {
                car.lap = Some(timed.state);
                car.lap_events = timed.events;
            }
        }
        self.world = world;
        self.ticks += 1;
        self.last_body_contacts = body_contacts;
        self.last_car_contacts = car_contacts;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS"]
    fn shared_world_is_independent_until_contact_and_pushes_on_contact() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
                let root = Path::new(&root);
                let read = |name| std::fs::read(std::env::var_os(name).unwrap()).unwrap();
                let table = read("GRID_CONTACT_DIRECTIONS");
                let events = Some(
                    crate::body_impact_parameters::read(&read("GRID_BODY_EVENT_PARAMETERS"))
                        .unwrap(),
                );
                // Car 0 drives, car 1 idles beside it: car 0 equals the single runtime.
                let mut multi =
                    MultiRuntime::lacetti(root, &["slot_00", "slot_01"], &table, events).unwrap();
                let mut single = Runtime::lacetti(root, "slot_00", 200679).unwrap();
                let geometry = single.prepare_body_geometry(root, &table).unwrap();
                single.body_events = events;
                single.enable_lap_timing(root).unwrap();
                let drive = Controls {
                    throttle: 1.,
                    ..Default::default()
                };
                for _ in 0..200 {
                    single.tick_with_body_geometry(drive, &geometry).unwrap();
                    multi.tick(&[drive, Controls::default()]).unwrap();
                    assert_eq!(
                        single.state.car.owner.physical[0x1d0 / 4..0x230 / 4],
                        multi.cars[0].state.car.owner.physical[0x1d0 / 4..0x230 / 4]
                    );
                    assert_eq!(
                        single.state.car.owner.vehicle,
                        multi.cars[0].state.car.owner.vehicle
                    );
                }
                // Car 1 steers into idle car 0: coupled contacts move car 0.
                multi.reset();
                let start = multi.cars[0].pose();
                let mut contacts = 0;
                for _ in 0..200 {
                    multi
                        .tick(&[
                            Controls::default(),
                            Controls {
                                throttle: 1.,
                                steering: 0.6,
                                ..Default::default()
                            },
                        ])
                        .unwrap();
                    contacts += multi.last_car_contacts;
                }
                let end = multi.cars[0].pose();
                let moved = ((end[12] - start[12]).powi(2) + (end[14] - start[14]).powi(2)).sqrt();
                eprintln!("shared_world car_contacts={contacts} car0_moved={moved}");
                assert!(contacts > 0 && moved > 0.5);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
