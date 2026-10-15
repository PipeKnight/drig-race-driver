//! Several source cars in one shared physical world (opponents). Each car keeps
//! its own source state, wheel query cache and lap timing; per tick every car
//! manager runs with its own controls and track queries, its wheel contacts are
//! re-identified to its world body and car shape, and one common advance solves
//! all bodies with track, route-prop and car-to-car body contacts together.
//! Selected scope: body events dispatch per car with no peer registration;
//! optional per-car deformation runs only for a car alone in its contact group
//! (static peers); groups of touching cars solve without deformation phases.
use super::*;
use grid_world::{
    body_impact, contact_groups, deformation_callback_state, deformation_contact_limit,
    deformation_world, physical_world, wheel_contacts,
};

/// Private deformation parameters shared by every car (DF01).
pub type DeformationParameters = (
    deformation_contact_limit::Parameters,
    deformation_callback_state::Parameters,
);

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
    /// Active contact slots of the last tick (wheel, body and car records).
    last_active: usize,
    /// Per car: summed unit normals (pushing the car) of the last tick's body
    /// contacts with static geometry, and the car-to-car normals with the
    /// other car's index (see [`Self::obstruction`]).
    statics_normals: Vec<[f32; 3]>,
    car_normals: Vec<(usize, usize, [f32; 3])>,
    /// Car speeds at the end of the last tick (the app swaps car 0 out
    /// between ticks).
    speeds: Vec<f32>,
}

/// Body centre (translation row of the physical pose).
fn body_centre(body: &contact_groups::Body) -> [f32; 3] {
    std::array::from_fn(|k| f32::from_bits(body[0x1d0 / 4 + 12 + k]))
}

/// Contact normal oriented to push the car away from the contact point.
fn pushing_normal(contact: &contact_groups::Contact, centre: [f32; 3]) -> [f32; 3] {
    let point: [f32; 3] = std::array::from_fn(|k| f32::from_bits(contact[0x50 / 4 + k]));
    let normal: [f32; 3] = std::array::from_fn(|k| f32::from_bits(contact[0x60 / 4 + k]));
    let toward: f32 = (0..3).map(|k| normal[k] * (centre[k] - point[k])).sum();
    if toward < 0. {
        normal.map(|x| -x)
    } else {
        normal
    }
}

/// Speed below which another car counts as an obstruction (0x4022F0, 2 m/s).
const OBSTRUCTION_SPEED: f32 = 2.;

/// Contact pool slots for `n` cars: 64 for one or two cars (the single-car
/// layout), 24 per car above that, up to the world bound.
fn pool_slots(n: usize) -> usize {
    (24 * n).clamp(64, grid_world::MAX_WORLD_CONTACTS)
}

impl MultiRuntime {
    /// Source cars on the named grid slots; `table` is the private CD01 body
    /// sampling input shared by every car.
    pub fn lacetti(
        root: &Path,
        slots: &[&str],
        table: &[u8],
        events: Option<body_impact::Parameters>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::lacetti_deforming(root, slots, table, events, None)
    }

    /// As [`Self::lacetti`], on the grid slots of the given route.
    pub fn lacetti_on(
        root: &Path,
        route: &Route,
        slots: &[&str],
        table: &[u8],
        events: Option<body_impact::Parameters>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::lacetti_deforming_on(root, route, slots, table, events, None)
    }

    /// As [`Self::lacetti`], with source deformation and callbacks on every car.
    pub fn lacetti_deforming(
        root: &Path,
        slots: &[&str],
        table: &[u8],
        events: Option<body_impact::Parameters>,
        deformation: Option<DeformationParameters>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::lacetti_deforming_on(root, &Route::default(), slots, table, events, deformation)
    }

    /// As [`Self::lacetti_deforming`], on the grid slots of the given route.
    pub fn lacetti_deforming_on(
        root: &Path,
        route: &Route,
        slots: &[&str],
        table: &[u8],
        events: Option<body_impact::Parameters>,
        deformation: Option<DeformationParameters>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if !(1..=12).contains(&slots.len()) {
            return Err("unsupported shared-world car count".into());
        }
        let mut cars = Vec::new();
        let mut geometry = Vec::new();
        for (i, slot) in slots.iter().enumerate() {
            let mut car = Runtime::lacetti_on(root, route, slot, 200679 + i as u32)?;
            let mut body = car.prepare_body_geometry(root, table)?;
            if let Some((limit, bulk)) = &deformation {
                car.enable_deformation(root, &mut body)?;
                car.register_deformation_callbacks(*limit, bulk.clone(), 1)?;
            }
            geometry.push(body);
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
        world.groups.contacts.resize(pool_slots(n), [0; 32]);
        world.manager.rows.resize(pool_slots(n) * 4, [0; 48]);
        world.manager.capacity = pool_slots(n) * 4;
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
            last_active: 0,
            statics_normals: vec![[0.; 3]; n],
            car_normals: Vec::new(),
            speeds: vec![0.; n],
        })
    }

    /// Obstruction contact sum of car `i` for the AI's collided_obstruction
    /// test (callback 0x4022F0 accumulated by 0x77B1F0): the last tick's body
    /// contacts with static geometry plus contacts with cars slower than
    /// 2 m/s, as summed unit normals pushing the car. Floor-like normals
    /// (y >= 0.1) are skipped. Unit weights stand in for the original
    /// callback magnitude [I]; only the sum's direction and non-zero length
    /// are read by the test.
    pub fn obstruction(&self, i: usize) -> [f32; 3] {
        let mut sum = self.statics_normals.get(i).copied().unwrap_or_default();
        for &(a, b, normal) in &self.car_normals {
            let (other, sign) = match (a == i, b == i) {
                (true, _) => (b, 1.),
                (_, true) => (a, -1.),
                _ => continue,
            };
            if self
                .speeds
                .get(other)
                .is_some_and(|&s| s < OBSTRUCTION_SPEED)
                && sign * normal[1] < 0.1
            {
                for k in 0..3 {
                    sum[k] += sign * normal[k];
                }
            }
        }
        sum
    }

    /// Car index pairs (first, second) of the last tick's car-to-car contacts.
    pub fn contact_pairs(&self) -> Vec<(usize, usize)> {
        let n = self.world.groups.bodies.len();
        self.world.groups.contacts[..self.last_active.min(self.world.groups.contacts.len())]
            .iter()
            .filter(|c| c[0xc / 4] & 0xff == 0 && c[2] != 0 && c[1] != 0)
            .map(|c| (c[1] as usize - 1, c[2] as usize - 1))
            .filter(|&(a, b)| a < n && b < n)
            .collect()
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
        self.last_active = 0;
        self.last_body_contacts = 0;
        self.last_car_contacts = 0;
        self.statics_normals = vec![[0.; 3]; self.cars.len()];
        self.car_normals.clear();
        self.speeds = vec![0.; self.cars.len()];
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
        let mut published: Vec<Option<crate::body_geometry::BodyGeometry>> = vec![None; n];
        let mut registrations: Vec<Option<deformation_world::Registration>> = vec![None; n];
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
            if used + 4 > pool_slots(n) {
                return Err(Error("shared wheel contact pool exhausted"));
            }
            let pool = wheel_contacts::Pool {
                slots: slots[used..used + 4].to_vec(),
                used: 0,
                shapes: self.statics,
            };
            let manager = if car.deformation_enabled {
                let mut body = self.geometry[i].clone();
                let manager = vehicle_manager::calculate_with_world_publication(
                    &state.car,
                    &pool,
                    car.pair,
                    previous.manager.parameters[0],
                    GRAVITY,
                    None,
                    query,
                    material,
                    &mut |owner, delta| body.publish_deformation(owner, delta),
                )?;
                published[i] = Some(body);
                if let Some(registration) = &car.deformation_registration {
                    let mut next = registration.clone();
                    next.owner = manager.job.state.owner.vehicle.clone();
                    registrations[i] = Some(next);
                }
                manager
            } else {
                vehicle_manager::calculate_with_world_contacts(
                    &state.car,
                    &pool,
                    car.pair,
                    previous.manager.parameters[0],
                    GRAVITY,
                    None,
                    query,
                    material,
                )?
            };
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
        let geometry: Vec<&crate::body_geometry::BodyGeometry> = published
            .iter()
            .zip(&self.geometry)
            .map(|(p, g)| p.as_ref().unwrap_or(g))
            .collect();
        let cars = &self.cars;
        let mut statics_normals = vec![[0f32; 3]; n];
        let mut car_normals = Vec::new();
        let mut provider = |bodies: &[contact_groups::Body], remaining: usize| {
            let mut out: Vec<contact_groups::Contact> = Vec::new();
            statics_normals.iter_mut().for_each(|s| *s = [0.; 3]);
            car_normals.clear();
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
                let centre = body_centre(&bodies[i]);
                for mut c in fresh {
                    c[1] = i as u32 + 1;
                    c[0x28 / 4] = tokens[i];
                    let normal = pushing_normal(&c, centre);
                    if normal[1] < 0.1 {
                        for k in 0..3 {
                            statics_normals[i][k] += normal[k];
                        }
                    }
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
                        car_normals.push((i, j, pushing_normal(&c, body_centre(&bodies[i]))));
                        out.push(c);
                        car_contacts += 1;
                    }
                }
            }
            Ok(out)
        };
        let world = if registrations.iter().any(Option::is_some) {
            let mut owned: Vec<Option<&mut deformation_world::Registration>> =
                registrations.iter_mut().map(Option::as_mut).collect();
            physical_world::advance_with_deformation(
                &previous,
                &slots,
                wheel_contacts,
                &self.shapes,
                &mut provider,
                &mut owned,
            )?
        } else {
            physical_world::advance_with_geometry_provider(
                &previous,
                &slots,
                wheel_contacts,
                &self.shapes,
                provider,
            )?
        };
        let active = wheel_contacts + body_contacts + car_contacts;
        for (i, state) in staged.iter_mut().enumerate() {
            if let Some(registration) = &registrations[i] {
                state.car.owner.vehicle = registration.owner.clone();
            }
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
                            // Car-to-car: the other car's world body identity
                            // (A726B0 passes the peer side's registration).
                            other_registration: if contact[0xc / 4] & 0xff == 0 && contact[2] != 0 {
                                if side == 0 { contact[2] } else { contact[1] }
                            } else {
                                0
                            },
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
            if let Some(registration) = &mut registrations[i] {
                state
                    .car
                    .frame
                    .body
                    .copy_from_slice(&state.car.owner.vehicle[..vehicle_force::BODY_WORDS]);
                registration.owner = state.car.owner.vehicle.clone();
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
        drop(geometry);
        for (i, body) in published.into_iter().enumerate() {
            if let Some(body) = body {
                self.geometry[i] = body;
            }
        }
        for (i, ((state, cache), timed)) in staged.into_iter().zip(caches).zip(laps).enumerate() {
            let car = &mut self.cars[i];
            if let Some(registration) = registrations[i].take() {
                car.deformation_registration = Some(registration);
            }
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
        self.last_active = active;
        self.statics_normals = statics_normals;
        self.car_normals = car_normals;
        self.speeds = self
            .cars
            .iter()
            .map(|c| {
                let v = c.velocity();
                (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
            })
            .collect();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn wheel_records_are_rear_left_rear_right_front_left_front_right() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let car = Runtime::lacetti(Path::new(&root), "slot_00", 200679).unwrap();
        let anchors: Vec<[f32; 3]> = car.wheels().iter().map(|w| w.anchor).collect();
        eprintln!("wheel_anchors={anchors:?}");
        // Car frame: +X is the car's left (positive steering turns toward it), +Z forward.
        for (i, (left, front)) in [(true, false), (false, false), (true, true), (false, true)]
            .into_iter()
            .enumerate()
        {
            assert_eq!(anchors[i][0] > 0., left, "wheel {i} side");
            assert_eq!(anchors[i][2] > 0., front, "wheel {i} axle");
        }
    }

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

    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS / GRID_DEFORMATION_PARAMETERS"]
    fn shared_world_deformation_matches_single_car_and_survives_contact() {
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
                let deformation =
                    crate::deformation_parameters::read(&read("GRID_DEFORMATION_PARAMETERS"))
                        .unwrap();
                // One deforming car in the shared world equals the single runtime,
                // including a barrier impact with dents (same scenario as the
                // single-car callback test).
                let mut multi = MultiRuntime::lacetti_deforming(
                    root,
                    &["slot_00"],
                    &table,
                    events,
                    Some(deformation.clone()),
                )
                .unwrap();
                let mut single = Runtime::lacetti(root, "slot_00", 200679).unwrap();
                let mut geometry = single.prepare_body_geometry(root, &table).unwrap();
                single.enable_deformation(root, &mut geometry).unwrap();
                single.body_events = events;
                single
                    .register_deformation_callbacks(deformation.0, deformation.1.clone(), 1)
                    .unwrap();
                single.enable_lap_timing(root).unwrap();
                let pristine = single.state.car.owner.vehicle[0x4880 / 4..0xc580 / 4].to_vec();
                let drive = Controls {
                    throttle: 1.,
                    steering: 0.25,
                    ..Default::default()
                };
                for _ in 0..600 {
                    single
                        .tick_with_deforming_body_geometry(drive, &mut geometry)
                        .unwrap();
                    multi.tick(&[drive]).unwrap();
                    assert_eq!(
                        single.state.car.owner.physical, multi.cars[0].state.car.owner.physical,
                        "tick {}",
                        single.ticks
                    );
                    assert_eq!(
                        single.state.car.owner.vehicle,
                        multi.cars[0].state.car.owner.vehicle
                    );
                }
                let a = single.deformation_registration.as_ref().unwrap();
                let b = multi.cars[0].deformation_registration.as_ref().unwrap();
                assert_eq!(
                    (a.random_seed, a.bulk_calls, a.records),
                    (b.random_seed, b.bulk_calls, b.records)
                );
                assert!(a.bulk_calls > 0);
                assert_ne!(
                    multi.cars[0].state.car.owner.vehicle[0x4880 / 4..0xc580 / 4],
                    pristine[..]
                );
                // Two deforming cars: car 1 rams idle car 0 (several-car group,
                // no deformation phases), then both keep driving without failure.
                let mut multi = MultiRuntime::lacetti_deforming(
                    root,
                    &["slot_00", "slot_01"],
                    &table,
                    events,
                    Some(deformation),
                )
                .unwrap();
                let pristine = multi.cars[0].state.car.owner.vehicle[0x4880 / 4..0xc580 / 4].to_vec();
                let mut contacts = 0;
                let mut rammed_dent_tick = None;
                for tick in 0..600 {
                    if rammed_dent_tick.is_none()
                        && tick < 200
                        && multi.cars[0].state.car.owner.vehicle[0x4880 / 4..0xc580 / 4] != pristine[..]
                    {
                        rammed_dent_tick = Some(tick);
                    }
                    let ram = Controls {
                        throttle: 1.,
                        steering: if tick < 200 { 0.6 } else { 0.25 },
                        ..Default::default()
                    };
                    let first = if tick < 200 {
                        Controls::default()
                    } else {
                        drive
                    };
                    multi
                        .tick(&[first, ram])
                        .unwrap_or_else(|e| panic!("two-car tick {}: {e}", tick + 1));
                    contacts += multi.last_car_contacts;
                }
                let calls: Vec<u64> = multi
                    .cars
                    .iter()
                    .map(|c| c.deformation_registration.as_ref().unwrap().bulk_calls)
                    .collect();
                let seeds: Vec<u32> = multi
                    .cars
                    .iter()
                    .map(|c| c.deformation_registration.as_ref().unwrap().random_seed)
                    .collect();
                eprintln!(
                    "shared_world_deformation car_contacts={contacts} bulk_calls={calls:?} rammed_idle_car_dented_at={rammed_dent_tick:?} toggle={}",
                    multi.cars[0].deformation_registration.as_ref().unwrap().owner_toggle
                );
                assert!(contacts > 0);
                // The idle car is touched only by the other car before tick 200.
                assert!(rammed_dent_tick.is_some());
                assert_eq!(seeds[0], seeds[1]);
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
