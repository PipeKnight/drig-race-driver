//! Several source cars in one shared physical world (opponents). Each car keeps
//! its own source state, wheel query cache and lap timing; per tick every car
//! manager runs with its own controls and track queries, its wheel contacts are
//! re-identified to its world body and car shape, and one common advance solves
//! all bodies with track, route-prop and car-to-car body contacts together.
//! Selected scope: body events dispatch per car with no peer registration;
//! optional per-car deformation runs only for a car alone in its contact group
//! (static peers); groups of touching cars solve without deformation phases.
use super::*;
use std::sync::Arc;
use grid_world::{
    body_impact, contact_groups, deformation_callback_state, deformation_contact_limit,
    deformation_world, physical_world, wheel_contacts,
};

/// Private deformation parameters shared by every car (DF01).
pub type DeformationParameters = (
    deformation_contact_limit::Parameters,
    deformation_callback_state::Parameters,
);

/// Cloning copies the live world; the start world and each car's body
/// geometry (replaced whole when a publication rebuilds it) are shared.
#[derive(Clone)]
pub struct MultiRuntime {
    pub cars: Vec<Runtime>,
    geometry: Vec<Arc<crate::body_geometry::BodyGeometry>>,
    world: grouped_physical_frame::Output,
    shapes: Vec<contact_groups::Shape>,
    statics: usize,
    initial_world: Arc<grouped_physical_frame::Output>,
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
    /// Per car: track, prop and car body records of the last tick.
    body_counts: Vec<usize>,
    /// Body-to-prop records of the last tick: (contact slot, route prop box).
    prop_slots: Vec<(usize, usize)>,
    car_normals: Vec<(usize, usize, [f32; 3])>,
    /// Car speeds at the end of the last tick (the app swaps car 0 out
    /// between ticks).
    speeds: Vec<f32>,
    /// Opt-in breakable route props; pieces are bodies after the cars.
    props: Option<crate::dynamic_props::PropWorld>,
    initial_shapes: Arc<Vec<contact_groups::Shape>>,
    /// Per car: why and since when it is out of the shared world.
    retired: Vec<Option<Retirement>>,
    /// Cars retired by the last tick.
    newly_retired: Vec<usize>,
    /// Independent per-car stages run on the shared compute pool
    /// ([`crate::parallel`]); results are identical either way.
    pub parallel: bool,
    #[cfg(test)]
    fail_lap_timing: Option<(usize, u64)>,
}

/// What a car body record touched (see [`MultiRuntime::contact_impacts`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Contacted {
    /// Route prop box index.
    Prop(usize),
    /// Another car of the shared world.
    Car(usize),
    /// A packed track collision chunk (`qt_*.vcqtc`, shape kind 11: road,
    /// kerbs, verges).
    Track,
    /// The boundary sheet or other static geometry.
    Static,
}

/// A car taken out after its own pipeline failed (see [`MultiRuntime::tick`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Retirement {
    /// Shared-world tick whose step failed for this car.
    pub tick: u64,
    pub error: Error,
    /// Held body kinematics: the last committed record, velocities zeroed.
    kinematics: [u32; KINEMATIC_WORDS],
}

/// Body words 0x1C0..0x230: mass centre, pose, angular and linear velocity.
const KINEMATICS: usize = 0x1c0 / 4;
const KINEMATIC_WORDS: usize = (0x230 - 0x1c0) / 4;
const VELOCITIES: std::ops::Range<usize> = (0x210 - 0x1c0) / 4..KINEMATIC_WORDS;

fn held_kinematics(body: &contact_groups::Body) -> [u32; KINEMATIC_WORDS] {
    let mut held: [u32; KINEMATIC_WORDS] = std::array::from_fn(|k| body[KINEMATICS + k]);
    held[VELOCITIES].fill(0);
    held
}

fn hold(body: &mut contact_groups::Body, held: &[u32; KINEMATIC_WORDS]) {
    body[KINEMATICS..KINEMATICS + KINEMATIC_WORDS].copy_from_slice(held);
}

/// One car's vehicle stage, staged before the common advance.
struct Staged {
    state: vehicle_manager::State,
    cache: rectangle_cull::Cache,
    contacts: Vec<contact_groups::Contact>,
    published: Option<crate::body_geometry::BodyGeometry>,
    registration: Option<deformation_world::Registration>,
}

/// Controls, track queries and the vehicle manager of one car against its
/// body record from the previous world.
fn vehicle_stage(
    car: &Runtime,
    base: &crate::body_geometry::BodyGeometry,
    controls: Controls,
    physical: contact_groups::Body,
    delta: f32,
    pool: &wheel_contacts::Pool,
) -> Result<Staged, Error> {
    let mut state = car.staged_car(controls)?;
    state.owner.physical = physical;
    state
        .frame
        .body
        .copy_from_slice(&state.owner.vehicle[..vehicle_force::BODY_WORDS]);
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
    let mut published = None;
    let mut registration = None;
    let manager = if car.deformation_enabled {
        // The body geometry is copied only when a publication rebuilds it.
        let mut rebuilt: Option<crate::body_geometry::BodyGeometry> = None;
        let manager = vehicle_manager::calculate_with_world_publication_staged(
            state,
            pool,
            car.pair,
            delta,
            GRAVITY,
            None,
            query,
            material,
            &mut |owner, delta| {
                let (geometry, next) = rebuilt
                    .as_ref()
                    .unwrap_or(base)
                    .deformation_publication(owner, delta)?;
                if geometry.is_some() {
                    rebuilt = geometry;
                }
                Ok(next)
            },
        )?;
        published = rebuilt;
        if let Some(current) = &car.deformation_registration {
            registration = Some(deformation_world::Registration {
                owner: manager.job.state.owner.vehicle.clone(),
                limit: current.limit,
                bulk: current.bulk.clone(),
                random_seed: current.random_seed,
                bulk_calls: current.bulk_calls,
                records: current.records,
                owner_toggle: current.owner_toggle,
            });
        }
        manager
    } else {
        vehicle_manager::calculate_with_world_contacts_staged(
            state, pool, car.pair, delta, GRAVITY, None, query, material,
        )?
    };
    Ok(Staged {
        contacts: manager.contacts.slots[..manager.contacts.used].to_vec(),
        state: manager.job.state,
        cache,
        published,
        registration,
    })
}

/// Body events of one car's solved contacts (owned damage RNG).
fn body_events(
    state: &mut vehicle_manager::State,
    parameters: body_impact::Parameters,
    contacts: &[contact_groups::Contact],
    token: u32,
    parameter: f32,
) -> Result<(), Error> {
    for contact in contacts {
        let side = match [contact[0x28 / 4] == token, contact[0x2c / 4] == token] {
            [true, false] => 0,
            [false, true] => 1,
            _ => continue,
        };
        let result = body_impact::calculate(
            &state.owner,
            body_impact::Event {
                point: std::array::from_fn(|j| f32::from_bits(contact[0x50 / 4 + j])),
                normal: std::array::from_fn(|j| f32::from_bits(contact[0x60 / 4 + j])),
                impact: f32::from_bits(contact[0x3c / 4]),
                parameter,
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
                .owner
                .damage_random_state
                .ok_or(Error("body impact requires owned damage RNG"))?,
        )?;
        state.owner = result.state;
        state.owner.damage_random_state = Some(result.random_state);
    }
    state
        .frame
        .body
        .copy_from_slice(&state.owner.vehicle[..vehicle_force::BODY_WORDS]);
    Ok(())
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
/// layout), 24 per car above that, up to 256 (AUTHORED budget, below the
/// original world bound; prop pieces add their own slots).
fn pool_slots(n: usize) -> usize {
    (24 * n).clamp(64, 256)
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
        let lacetti = Car::default();
        let entries: Vec<(&str, &Car)> = slots.iter().map(|s| (*s, &lacetti)).collect();
        Self::load_deforming_on(root, route, &entries, table, events, deformation)
    }

    /// Mixed-model field: each entry is a grid slot and the purchased car on
    /// it (docs/CAR_GENERALIZATION.md). Car `i` keeps seed 200679 + i.
    pub fn load_deforming_on(
        root: &Path,
        route: &Route,
        entries: &[(&str, &Car)],
        table: &[u8],
        events: Option<body_impact::Parameters>,
        deformation: Option<DeformationParameters>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Up to the routes' 20 grid slots: the Le Mans Series fields have
        // 15-17 cars (docs/SERIES.md, F409).
        if !(1..=20).contains(&entries.len()) {
            return Err("unsupported shared-world car count".into());
        }
        let mut cars = Vec::new();
        let mut geometry = Vec::new();
        // Every car is on `route`: parse its props once (most of a field's
        // load time, F382); each car's track shares them copy-on-write.
        let props = crate::entity_geometry::Props::read_route(root, route)?;
        for (i, (slot, model)) in entries.iter().enumerate() {
            let mut car = Runtime::load(root, route, model, slot, 200679 + i as u32)?;
            let mut body = car.prepare_body_geometry_with(root, table, props.clone())?;
            if let Some((limit, bulk)) = &deformation {
                car.enable_deformation(root, &mut body)?;
                car.register_deformation_callbacks(*limit, bulk.clone(), 1)?;
            }
            geometry.push(Arc::new(body));
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
        world
            .groups
            .contacts
            .resize(pool_slots(n), contact_groups::EMPTY_CONTACT);
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
            initial_world: Arc::new(world.clone()),
            world,
            initial_shapes: Arc::new(shapes.clone()),
            shapes,
            statics,
            ticks: 0,
            last_body_contacts: 0,
            last_car_contacts: 0,
            last_active: 0,
            statics_normals: vec![[0.; 3]; n],
            body_counts: vec![0; n],
            prop_slots: Vec::new(),
            car_normals: Vec::new(),
            speeds: vec![0.; n],
            props: None,
            retired: vec![None; n],
            newly_retired: Vec::new(),
            parallel: true,
            #[cfg(test)]
            fail_lap_timing: None,
        })
    }

    /// Why car `i` is out of the shared world, if it is.
    pub fn retirement(&self, i: usize) -> Option<&Retirement> {
        self.retired.get(i)?.as_ref()
    }

    /// Track, prop and car body records of car `i` in the last tick.
    pub fn body_contacts_of(&self, i: usize) -> usize {
        self.body_counts.get(i).copied().unwrap_or(0)
    }

    /// Cars retired by the last tick, in car order.
    pub fn newly_retired(&self) -> &[usize] {
        &self.newly_retired
    }

    /// Opt in to breakable route props before the first tick (see
    /// `Runtime::enable_dynamic_props`); every car's track is armed.
    pub fn enable_dynamic_props(&mut self, root: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if self.ticks != 0 || self.props.is_some() {
            return Err("dynamic props must be enabled once before the first tick".into());
        }
        let first = &self.cars[0];
        let props = first
            .track
            .props()
            .ok_or("dynamic props require prepared body geometry")?;
        let catalog = crate::dynamic_props::Catalog::read(root, first.route(), props)?;
        let world = crate::dynamic_props::PropWorld::new(catalog);
        for car in &mut self.cars {
            world.arm(&mut car.track)?;
        }
        self.props = Some(world);
        Ok(())
    }

    /// Breakable route props, when enabled.
    pub fn dynamic_props(&self) -> Option<&crate::dynamic_props::PropWorld> {
        self.props.as_ref()
    }

    /// Shared world bodies: the cars, then live prop pieces.
    pub fn bodies(&self) -> &[contact_groups::Body] {
        &self.world.groups.bodies
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
        let n = self.cars.len();
        self.world.groups.contacts[..self.last_active.min(self.world.groups.contacts.len())]
            .iter()
            .filter(|c| c[0xc / 4] & 0xff == 0 && c[2] != 0 && c[1] != 0)
            .map(|c| (c[1] as usize - 1, c[2] as usize - 1))
            .filter(|&(a, b)| a < n && b < n)
            .collect()
    }

    /// Solved contact notifications of car `i` in the last tick (presentation
    /// only): the same records the body-event loop dispatches for that car.
    pub fn impacts(&self, i: usize) -> Vec<crate::body_impacts::Impact> {
        let contacts = &self.world.groups.contacts;
        crate::body_impacts::collect(
            &contacts[..self.last_active.min(contacts.len())],
            self.token(i),
        )
    }

    /// [`Self::impacts`] with what each record touched: a route prop box,
    /// another car or other static geometry.
    pub fn contact_impacts(&self, i: usize) -> Vec<(crate::body_impacts::Impact, Contacted)> {
        let contacts = &self.world.groups.contacts;
        let token = self.token(i);
        let cars: Vec<u32> = (0..self.cars.len()).map(|j| self.token(j)).collect();
        contacts[..self.last_active.min(contacts.len())]
            .iter()
            .enumerate()
            .filter_map(|(slot, c)| {
                let impact = crate::body_impacts::collect(std::slice::from_ref(c), token).pop()?;
                let other = if c[0x28 / 4] == token {
                    c[0x2c / 4]
                } else {
                    c[0x28 / 4]
                };
                let contacted = match self.prop_slots.iter().find(|&&(s, _)| s == slot) {
                    Some(&(_, p)) => Contacted::Prop(p),
                    None => match cars.iter().position(|&t| t == other) {
                        Some(j) => Contacted::Car(j),
                        None if self.cars[i].track.is_chunk_token(other) => Contacted::Track,
                        None => Contacted::Static,
                    },
                };
                Some((impact, contacted))
            })
            .collect()
    }

    /// Route prop boxes that are `close_drift_flag` entities.
    pub fn close_drift_flag_boxes(&self) -> Vec<usize> {
        self.cars[0].track.props().map_or_else(Vec::new, |p| {
            p.boxes
                .iter()
                .enumerate()
                .filter(|(_, b)| b.template.contains("close_drift_flag"))
                .map(|(k, _)| k)
                .collect()
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
        self.world = (*self.initial_world).clone();
        self.shapes = (*self.initial_shapes).clone();
        if let Some(props) = &mut self.props {
            let mut tracks: Vec<&mut Track> = self.cars.iter_mut().map(|c| &mut c.track).collect();
            // Re-arming only fails without route props, which enabling requires.
            let _ = props.reset(&mut tracks);
        }
        self.ticks = 0;
        self.last_active = 0;
        self.last_body_contacts = 0;
        self.last_car_contacts = 0;
        self.statics_normals = vec![[0.; 3]; self.cars.len()];
        self.body_counts = vec![0; self.cars.len()];
        self.prop_slots.clear();
        self.car_normals.clear();
        self.speeds = vec![0.; self.cars.len()];
        self.retired = vec![None; self.cars.len()];
        self.newly_retired.clear();
    }

    /// [`Runtime::place_at_rest`] for car `i`, on its body in the shared
    /// world. A retired car is refused: it stays held.
    pub fn place_at_rest(
        &mut self,
        i: usize,
        point: [f32; 3],
        heading: [f32; 3],
    ) -> Result<(), Error> {
        if i >= self.cars.len() || self.retired[i].is_some() {
            return Err(Error("no racing car to re-place"));
        }
        let placement = self.cars[i].placement(&self.world.groups.bodies[i], point, heading)?;
        apply_placement(&mut self.world.groups.bodies[i], &placement.physical);
        self.cars[i].commit_placement(placement);
        self.speeds[i] = 0.;
        Ok(())
    }

    /// Test set-up: put car `i` at `pose` at rest in the shared world.
    #[cfg(test)]
    pub(crate) fn set_pose_at_rest(&mut self, i: usize, pose: [f32; 16]) {
        self.cars[i].set_pose_at_rest(pose);
        let placed = self.cars[i].state.car.owner.physical;
        apply_placement(&mut self.world.groups.bodies[i], &placed);
        self.cars[i].state.car.owner.physical = self.world.groups.bodies[i];
    }

    /// One common timestep for all cars.
    ///
    /// A car whose own stage fails (controls, vehicle manager, its track or
    /// car contacts, body events, lap timing) keeps its previous state and is
    /// retired: its body is held at its last committed pose with zero velocity
    /// from then on, still colliding with the others, and the rest of the
    /// field steps on. This is the port's nearest equivalent to the original
    /// skipping an over-limit island for a step (B7B850 status 1) without
    /// stopping the race; the original has no per-car retirement. Shared
    /// stages (the world advance, prop pieces) still fail the whole tick, and
    /// then every car, the shared world and all query caches stay unchanged.
    pub fn tick(&mut self, controls: &[Controls]) -> Result<(), Error> {
        let n = self.cars.len();
        if controls.len() != n {
            return Err(Error("one control set per car is required"));
        }
        let _tick_timer = crate::perf::Timer::start(crate::perf::Stage::WorldTick);
        let tokens: Vec<u32> = (0..n).map(|i| self.token(i)).collect();
        let mut previous = self.world.clone();
        let mut slots = self.world.groups.contacts.clone();
        let mut used = 0;
        let mut failed: Vec<Option<Error>> = vec![None; n];
        let mut staged: Vec<Option<vehicle_manager::State>> = Vec::with_capacity(n);
        let mut caches = Vec::with_capacity(n);
        let mut published: Vec<Option<crate::body_geometry::BodyGeometry>> = vec![None; n];
        let mut registrations: Vec<Option<deformation_world::Registration>> = vec![None; n];
        let vehicle_timer = crate::perf::Timer::start(crate::perf::Stage::VehicleStage);
        // A car's vehicle stage reads only that car and its own body record
        // in the previous world, so every racing car's stage runs at once.
        // Only the shared wheel-contact pool offset depends on the cars
        // before it; the contacts produced into a blank pool are placed in
        // their real slots below, in car order (wheel_contacts::overlay).
        let blank = wheel_contacts::Pool {
            slots: vec![contact_groups::EMPTY_CONTACT; 4],
            used: 0,
            shapes: self.statics,
        };
        let stages = {
            let (cars, geometry, retired, previous) =
                (&self.cars, &self.geometry, &self.retired, &previous);
            crate::parallel::map(n, self.parallel, |i| {
                retired[i].is_none().then(|| {
                    vehicle_stage(
                        &cars[i],
                        &geometry[i],
                        controls[i],
                        previous.groups.bodies[i],
                        previous.manager.parameters[0],
                        &blank,
                    )
                })
            })
        };
        for ((i, car), stage) in self.cars.iter().enumerate().zip(stages) {
            if let Some(stage) = stage {
                if used + 4 > pool_slots(n) {
                    return Err(Error("shared wheel contact pool exhausted"));
                }
                match stage {
                    Ok(stage) => {
                        for c in stage.contacts {
                            let mut c = wheel_contacts::overlay(&slots[used], &c);
                            c[1] = i as u32 + 1;
                            c[0x28 / 4] = tokens[i];
                            slots[used] = c;
                            used += 1;
                        }
                        previous.groups.bodies[i] = stage.state.owner.physical;
                        published[i] = stage.published;
                        registrations[i] = stage.registration;
                        staged.push(Some(stage.state));
                        caches.push(stage.cache);
                        continue;
                    }
                    Err(error) => failed[i] = Some(error),
                }
            }
            // Retired or failing: the body holds still through the advance.
            let held = self.held(i);
            hold(&mut previous.groups.bodies[i], &held);
            staged.push(None);
            caches.push(car.cache);
        }
        drop(vehicle_timer);
        let wheel_contacts = used;
        let parallel = self.parallel;
        let mut provider_nanos = 0u64;
        let mut body_contacts = 0;
        let mut car_contacts = 0;
        let geometry: Vec<&crate::body_geometry::BodyGeometry> = published
            .iter()
            .zip(&self.geometry)
            .map(|(p, g)| p.as_ref().unwrap_or(g))
            .collect();
        let cars = &self.cars;
        let props = self.props.as_ref();
        let moving: Vec<bool> = staged.iter().map(Option::is_some).collect();
        let mut recorded = Vec::new();
        let mut piece_contacts = 0;
        let mut statics_normals = vec![[0f32; 3]; n];
        let mut body_counts = vec![0; n];
        let mut prop_slots = Vec::new();
        let mut car_normals = Vec::new();
        let mut contact_failed: Vec<Option<Error>> = vec![None; n];
        let mut provider = |bodies: &mut [contact_groups::Body], remaining: usize| {
            let provider_started = crate::perf::enabled().then(std::time::Instant::now);
            // World phase 2 (continuous) before the contacts of phase 3.
            if let Some(props) = props.filter(|p| p.has_bodies()) {
                props.continuous(
                    &cars[0].track,
                    &cars[0].surfaces,
                    bodies,
                    geometry[0].query(),
                )?;
            }
            let bodies: &[contact_groups::Body] = bodies;
            let body_timer = crate::perf::Timer::start(crate::perf::Stage::BodyContacts);
            let mut out: Vec<contact_groups::Contact> = Vec::new();
            statics_normals.iter_mut().for_each(|s| *s = [0.; 3]);
            body_counts.iter_mut().for_each(|c| *c = 0);
            car_normals.clear();
            recorded.clear();
            prop_slots.clear();
            // A held body needs no static contacts; the others still hit it.
            let wanted: Vec<bool> = (0..n)
                .map(|i| moving[i] && contact_failed[i].is_none())
                .collect();
            // Every car's static contacts at once with the whole capacity;
            // the sequential pass below reuses a result only where the
            // smaller capacity left to that car gives the same one.
            let full = crate::parallel::map(n, parallel, |i| {
                wanted[i].then(|| {
                    geometry[i].contacts_with_props(
                        &cars[i].track,
                        &cars[i].surfaces,
                        &bodies[i..=i],
                        remaining,
                    )
                })
            });
            for (i, full) in full.into_iter().enumerate() {
                if out.len() >= remaining {
                    break;
                }
                let Some(full) = full else {
                    continue;
                };
                let room = remaining - out.len();
                let fresh = if room == remaining
                    || crate::body_geometry::BodyGeometry::reusable_with(&full, room)
                {
                    full
                } else {
                    geometry[i].contacts_with_props(
                        &cars[i].track,
                        &cars[i].surfaces,
                        &bodies[i..=i],
                        room,
                    )
                };
                let fresh = match fresh {
                    Ok(fresh) => fresh,
                    Err(error) => {
                        contact_failed[i] = Some(error);
                        continue;
                    }
                };
                body_counts[i] += fresh.len();
                let centre = body_centre(&bodies[i]);
                for (mut c, prop) in fresh {
                    c[1] = i as u32 + 1;
                    c[0x28 / 4] = tokens[i];
                    let normal = pushing_normal(&c, centre);
                    if normal[1] < 0.1 {
                        for k in 0..3 {
                            statics_normals[i][k] += normal[k];
                        }
                    }
                    if let Some(prop) = prop {
                        prop_slots.push((wheel_contacts + out.len(), prop));
                    }
                    if let Some(prop) = prop
                        && props.is_some()
                    {
                        recorded.push((
                            wheel_contacts + out.len(),
                            crate::dynamic_props::PropContact {
                                contact: c,
                                prop,
                                body: i,
                            },
                        ));
                    }
                    out.push(c);
                }
            }
            body_contacts = out.len();
            drop(body_timer);
            let pairs_timer = crate::perf::Timer::start(crate::perf::Stage::CarPairs);
            // Each car's world hull is built once, when a pair first needs it,
            // and a pair query depends on the capacity only through
            // `capacity.min(PAIR)` (zero never reaches it). Both are pure, so
            // the hulls and then each car's row of pair queries run at once
            // beforehand with the full pair capacity; the pass below keeps the
            // original order, failures and capacities and takes a prepared
            // result exactly where the original would compute the same one.
            // A pair first needs car j's hull while j's failure state is still
            // the one at this point: only j's own build or j's own pairs,
            // which come later, can change it.
            const PAIR: usize = 16;
            let built = crate::parallel::map(n, parallel, |j| {
                (j > 0 && contact_failed[j].is_none()).then(|| {
                    geometry[j]
                        .hull_world_triangles(&bodies[j])
                        .map(|triangles| crate::body_geometry::WorldHull::new(&triangles))
                })
            });
            let rows = {
                let built = &built;
                crate::parallel::map(n, parallel, |i| {
                    (contact_failed[i].is_none() && remaining - out.len() >= PAIR).then(|| {
                        (0..n)
                            .map(|j| match &built[j] {
                                Some(Ok(hull)) if j > i => {
                                    Some(geometry[i].car_contacts_hull(&bodies[i], hull, PAIR))
                                }
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                    })
                })
            };
            let mut rows: Vec<Vec<Option<_>>> = rows
                .into_iter()
                .map(|row| row.unwrap_or_else(|| (0..n).map(|_| None).collect()))
                .collect();
            let mut needed = vec![false; n];
            for i in 0..n {
                for j in i + 1..n {
                    if out.len() >= remaining || contact_failed[i].is_some() {
                        break;
                    }
                    if contact_failed[j].is_some() {
                        continue;
                    }
                    if !needed[j] {
                        needed[j] = true;
                        if let Some(Err(error)) = &built[j] {
                            contact_failed[j] = Some(error.clone());
                            continue;
                        }
                    }
                    let Some(Ok(other)) = &built[j] else {
                        unreachable!("hulls are built for every car that can be met");
                    };
                    let room = remaining - out.len();
                    let pair = match rows[i][j].take() {
                        Some(prepared) if room >= PAIR => prepared,
                        _ => geometry[i].car_contacts_hull(&bodies[i], other, room),
                    };
                    let pair = match pair {
                        Ok(pair) => pair,
                        Err(error) => {
                            contact_failed[i] = Some(error);
                            break;
                        }
                    };
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
                        body_counts[i] += 1;
                        body_counts[j] += 1;
                    }
                }
            }
            drop(pairs_timer);
            let _pieces_timer = crate::perf::Timer::start(crate::perf::Stage::PieceContacts);
            piece_contacts = 0;
            if let Some(props) = props.filter(|p| p.has_bodies()) {
                let hulls: Vec<_> = (0..n)
                    .map(|i| crate::dynamic_props::CarHull {
                        geometry: geometry[i],
                        body: i,
                        token: tokens[i],
                    })
                    .collect();
                let mut local = Vec::new();
                let more = props.contacts(
                    &cars[0].track,
                    &cars[0].surfaces,
                    bodies,
                    &hulls,
                    geometry[0].query(),
                    remaining - out.len(),
                    &mut local,
                )?;
                let base = wheel_contacts + out.len();
                recorded.extend(local.into_iter().map(|(k, r)| (base + k, r)));
                piece_contacts = more.len();
                out.extend(more);
            }
            if let Some(started) = provider_started {
                provider_nanos += started.elapsed().as_nanos() as u64;
            }
            Ok(out)
        };
        let advance_started = crate::perf::enabled().then(std::time::Instant::now);
        let world = if registrations.iter().any(Option::is_some) {
            let mut owned: Vec<Option<&mut deformation_world::Registration>> =
                registrations.iter_mut().map(Option::as_mut).collect();
            // Prop pieces follow the cars and never deform.
            owned.resize_with(previous.groups.bodies.len(), || None);
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
        if let Some(started) = advance_started {
            let total = started.elapsed().as_nanos() as u64;
            crate::perf::add(
                crate::perf::Stage::WorldAdvance,
                total.saturating_sub(provider_nanos),
            );
        }
        let _commit_timer = crate::perf::Timer::start(crate::perf::Stage::Commit);
        let mut world = world;
        let mut shapes = None;
        let mut props_next = None;
        if let Some(current) = &self.props {
            let mut next = current.clone();
            next.detect(&world.groups.contacts, &recorded, &world.groups.bodies);
            let mut changed = self.shapes.clone();
            next.hold_lost(&mut world.groups.bodies, self.cars[0].track.bounds()[1]);
            let disabled = next.apply(&mut world, &mut changed, self.ticks + 1)?;
            if !disabled.is_empty() {
                shapes = Some(changed);
            }
            props_next = Some((next, disabled));
        }
        let active = wheel_contacts + body_contacts + car_contacts + piece_contacts;
        for (i, error) in contact_failed.into_iter().enumerate() {
            if let Some(error) = error
                && failed[i].is_none()
            {
                failed[i] = Some(error);
                staged[i] = None;
            }
        }
        for (i, slot) in staged.iter_mut().enumerate() {
            let Some(state) = slot else {
                continue;
            };
            // The registration's owner is moved into the car and refilled below.
            if let Some(registration) = &mut registrations[i] {
                std::mem::swap(&mut state.owner.vehicle, &mut registration.owner);
            }
            state.owner.physical = world.groups.bodies[i];
            if let Some(parameters) = self.cars[i].body_events
                && let Err(error) = body_events(
                    state,
                    parameters,
                    &world.groups.contacts[..active.min(world.groups.contacts.len())],
                    tokens[i],
                    world.manager.parameters[0],
                )
            {
                failed[i] = Some(error);
                *slot = None;
                continue;
            }
            if let Some(registration) = &mut registrations[i] {
                state
                    .frame
                    .body
                    .copy_from_slice(&state.owner.vehicle[..vehicle_force::BODY_WORDS]);
                registration.owner.clone_from(&state.owner.vehicle);
            }
        }
        // Lap timing per car, staged before any commit.
        let mut laps = Vec::with_capacity(n);
        for (i, car) in self.cars.iter().enumerate() {
            let timed = match (&staged[i], &car.lap_course, &car.lap) {
                (Some(state), Some(course), Some(lap)) => {
                    let position = std::array::from_fn(|k| {
                        f32::from_bits(state.owner.physical[0x1d0 / 4 + 12 + k])
                    });
                    #[cfg(test)]
                    let injected = self.fail_lap_timing == Some((i, self.ticks + 1));
                    #[cfg(not(test))]
                    let injected = false;
                    if injected {
                        Err(Error("injected lap-timing failure"))
                    } else {
                        lap.advance(course, position, (self.ticks + 1) as f64 * f64::from(DELTA))
                            .map(Some)
                    }
                }
                _ => Ok(None),
            };
            laps.push(timed.unwrap_or_else(|error| {
                failed[i] = Some(error);
                staged[i] = None;
                None
            }));
        }
        drop(geometry);
        // A failing car's body holds still and its staged results are dropped.
        let mut retiring = Vec::new();
        for i in 0..n {
            if staged[i].is_none() {
                let held = self.held(i);
                hold(&mut world.groups.bodies[i], &held);
                published[i] = None;
                registrations[i] = None;
                if let Some(error) = failed[i].take() {
                    retiring.push((i, error, held));
                }
            }
        }
        for (i, body) in published.into_iter().enumerate() {
            if let Some(body) = body {
                self.geometry[i] = Arc::new(body);
            }
        }
        for (i, ((state, cache), timed)) in staged.into_iter().zip(caches).zip(laps).enumerate() {
            let car = &mut self.cars[i];
            // A held car's clock still runs with the race.
            car.ticks += 1;
            car.lap_events.clear();
            let Some(state) = state else {
                continue;
            };
            if let Some(registration) = registrations[i].take() {
                car.deformation_registration = Some(registration);
            }
            car.state.car = state;
            car.cache = cache;
            if let Some(timed) = timed {
                car.lap = Some(timed.state);
                car.lap_events = timed.events;
            }
        }
        self.newly_retired.clear();
        for (i, error, kinematics) in retiring {
            hold(&mut self.cars[i].state.car.owner.physical, &kinematics);
            self.retired[i] = Some(Retirement {
                tick: self.ticks + 1,
                error,
                kinematics,
            });
            self.newly_retired.push(i);
        }
        if let Some((next, disabled)) = props_next {
            for car in &mut self.cars {
                crate::dynamic_props::PropWorld::disable(&mut car.track, &disabled);
            }
            self.props = Some(next);
        }
        if let Some(shapes) = shapes {
            self.shapes = shapes;
        }
        self.world = world;
        self.ticks += 1;
        self.last_body_contacts = body_contacts;
        self.last_car_contacts = car_contacts;
        self.last_active = active;
        self.statics_normals = statics_normals;
        self.body_counts = body_counts;
        self.prop_slots = prop_slots;
        self.car_normals = car_normals;
        self.speeds = self
            .cars
            .iter()
            .map(|c| {
                let v = c.velocity();
                (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
            })
            .collect();
        // The loose-part wheel rule of every car still in the world; a lost
        // wheel becomes a body of the shared world (loose_wheel).
        for i in 0..n {
            if self.retired[i].is_none() {
                let impacts = self.impacts(i);
                let lost = self.cars[i].detach_wheels(&impacts);
                self.add_loose_wheels(i, lost)?;
            }
        }
        Ok(())
    }

    /// Diagnostics (grid-opponent-race GRID_LOSE_WHEEL): car `i` loses
    /// `wheel` now, with its debris body. False when it cannot (damage off,
    /// already lost, retired).
    pub fn force_wheel_loss(&mut self, i: usize, wheel: usize) -> Result<bool, Error> {
        if i >= self.cars.len() || self.retired[i].is_some() {
            return Ok(false);
        }
        let lost = self.cars[i].force_detach(wheel);
        self.add_loose_wheels(i, lost)?;
        Ok(lost.iter().any(|l| *l))
    }

    /// Add car `i`'s wheels `lost` this tick as bodies of the shared world.
    pub(crate) fn add_loose_wheels(&mut self, i: usize, lost: [bool; 4]) -> Result<(), Error> {
        let spawns = self.cars[i].loose_wheels(lost, i)?;
        if spawns.is_empty() {
            return Ok(());
        }
        let props = self
            .props
            .get_or_insert_with(|| crate::dynamic_props::PropWorld::new(Default::default()));
        for (wheel, s) in spawns {
            props.add_wheel(
                &mut self.world,
                &mut self.shapes,
                i,
                wheel,
                s.spec,
                s.linear,
                s.angular,
            )?;
        }
        // Appending relinks the chain tail; keep the cars' copies in step.
        for (j, car) in self.cars.iter_mut().enumerate() {
            car.state.car.owner.physical[0x300 / 4] = self.world.groups.bodies[j][0x300 / 4];
        }
        Ok(())
    }

    /// Held kinematics of car `i`: its retirement's, or the last committed
    /// world body with zero velocities.
    fn held(&self, i: usize) -> [u32; KINEMATIC_WORDS] {
        self.retired[i].as_ref().map_or_else(
            || held_kinematics(&self.world.groups.bodies[i]),
            |r| r.kinematics,
        )
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
    fn mixed_field_bmw_equals_its_single_runtime_until_contact() {
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
                let (lac, bmw) = (Car::default(), Car::new(crate::car::BMW).unwrap());
                let mut multi = MultiRuntime::load_deforming_on(
                    root,
                    &Route::default(),
                    &[("slot_00", &lac), ("slot_01", &bmw)],
                    &table,
                    events,
                    None,
                )
                .unwrap();
                assert_eq!(multi.cars[0].car(), &lac);
                assert_eq!(multi.cars[1].car(), &bmw);
                // Each car keeps its own wheel geometry. Both CSVs give the
                // same mass and inertia (docs/BODY_PARAMETERS.md).
                let anchors = |c: &Runtime| -> Vec<[f32; 3]> {
                    c.wheels().iter().map(|w| w.anchor).collect()
                };
                assert_ne!(anchors(&multi.cars[0]), anchors(&multi.cars[1]));
                assert_eq!(
                    multi.world.manager.properties[0],
                    multi.world.manager.properties[1]
                );
                for (i, a) in anchors(&multi.cars[1]).iter().enumerate() {
                    assert_eq!(a[0] > 0., i % 2 == 0, "BMW wheel {i} side");
                    assert_eq!(a[2] > 0., i >= 2, "BMW wheel {i} axle");
                }
                // Car 1 (BMW) drives, car 0 idles: car 1 equals its single runtime.
                let mut single =
                    Runtime::load(root, &Route::default(), &bmw, "slot_01", 200680).unwrap();
                let geometry = single.prepare_body_geometry(root, &table).unwrap();
                single.body_events = events;
                single.enable_lap_timing(root).unwrap();
                let drive = Controls {
                    throttle: 1.,
                    ..Default::default()
                };
                for _ in 0..200 {
                    single.tick_with_body_geometry(drive, &geometry).unwrap();
                    multi.tick(&[Controls::default(), drive]).unwrap();
                    assert_eq!(
                        single.state.car.owner.physical[0x1d0 / 4..0x230 / 4],
                        multi.cars[1].state.car.owner.physical[0x1d0 / 4..0x230 / 4]
                    );
                }
                let v = single.velocity();
                assert!(v[0] * v[0] + v[2] * v[2] > 8. * 8., "BMW launch {v:?}");
            })
            .unwrap()
            .join()
            .unwrap();
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

    /// Restart Race (grid-app pause menu, R): broken props are gone, the
    /// intact boxes are armed again and the world replays a fresh one.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS"]
    fn reset_restores_intact_props_and_replays_a_fresh_world() {
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
                let load = || {
                    let mut multi =
                        MultiRuntime::lacetti(root, &["slot_00", "slot_01"], &table, events)
                            .unwrap();
                    multi.enable_dynamic_props(root).unwrap();
                    multi
                };
                let hash = |multi: &MultiRuntime| {
                    multi
                        .bodies()
                        .iter()
                        .flatten()
                        .chain(multi.cars.iter().flat_map(|c| &c.state.car.owner.vehicle))
                        .fold(0xcbf29ce484222325u64, |h, &w| {
                            (h ^ u64::from(w)).wrapping_mul(0x100000001b3)
                        })
                };
                let drive = [Controls {
                    throttle: 0.6,
                    ..Default::default()
                }; 2];
                let mut multi = load();
                let (start, bodies) = (hash(&multi), multi.bodies().len());
                let props = multi.props.as_mut().unwrap();
                let stacks: Vec<usize> = (props.catalog.breakables.iter().enumerate())
                    .filter(|(_, b)| b.template.starts_with("tyrestack"))
                    .map(|(i, _)| i)
                    .take(2)
                    .collect();
                for &b in &stacks {
                    props.queue_break(b);
                }
                for _ in 0..200 {
                    multi.tick(&drive).unwrap();
                }
                let props = multi.dynamic_props().unwrap();
                assert_eq!(props.events.len(), stacks.len());
                assert!(multi.bodies().len() > bodies);
                multi.reset();
                let props = multi.dynamic_props().unwrap();
                assert!(props.events.is_empty() && props.pieces.is_empty());
                assert_eq!(multi.bodies().len(), bodies);
                assert_eq!(hash(&multi), start);
                let mut fresh = load();
                for _ in 0..200 {
                    multi.tick(&drive).unwrap();
                    fresh.tick(&drive).unwrap();
                }
                assert_eq!(hash(&multi), hash(&fresh));
            })
            .unwrap()
            .join()
            .unwrap();
    }

    /// F484: a broken Donington tyre wall's six blocks stay joined by their
    /// seven springs (cDeformableRubber*, unbreakable) and settle; joined
    /// blocks do not collide with each other (dontCollideWithAny).
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS"]
    fn a_broken_tyre_wall_stays_joined_and_settles() {
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
                let mut multi = MultiRuntime::lacetti(root, &["slot_00"], &table, events).unwrap();
                multi.enable_dynamic_props(root).unwrap();
                let props = multi.props.as_mut().unwrap();
                let wall = (props.catalog.breakables.iter())
                    .position(|b| b.template == "tyre_wall.max")
                    .unwrap();
                props.queue_break(wall);
                let drive = [Controls::default(); 1];
                multi.tick(&drive).unwrap();
                let pose = |multi: &MultiRuntime, body: usize| {
                    crate::dynamic_props::body_pose(&multi.bodies()[body])[3]
                };
                let distance = |a: [f32; 4], b: [f32; 4]| {
                    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
                };
                let joints = multi.dynamic_props().unwrap().joints.clone();
                assert_eq!(joints.len(), 7);
                assert_eq!(multi.world.groups.joints.len(), 7);
                let rest: Vec<f32> = joints
                    .iter()
                    .map(|j| distance(pose(&multi, j.bodies[0]), pose(&multi, j.bodies[1])))
                    .collect();
                for _ in 0..300 {
                    multi.tick(&drive).unwrap();
                }
                let props = multi.dynamic_props().unwrap();
                assert_eq!(props.pieces.len(), 22);
                for (j, rest) in props.joints.iter().zip(&rest) {
                    assert!(!j.broken);
                    let now = distance(pose(&multi, j.bodies[0]), pose(&multi, j.bodies[1]));
                    eprintln!("tyre_wall joint {:?} rest={rest:.3} now={now:.3}", j.bodies);
                    assert!((now - rest).abs() < 0.3, "{j:?} {rest} {now}");
                }
                for piece in &props.pieces {
                    let spawn =
                        props.catalog.breakables[piece.breakable].pieces[piece.spec].pose[3];
                    let now = pose(&multi, piece.body);
                    assert!(now.iter().all(|v| v.is_finite()));
                    assert!((now[1] - spawn[1]).abs() < 1.5, "{piece:?} {spawn:?} {now:?}");
                    // Settled: no piece still moving fast after five seconds.
                    let v = &multi.bodies()[piece.body][0x220 / 4..0x22c / 4];
                    let speed = v.iter().map(|&w| f32::from_bits(w).powi(2)).sum::<f32>().sqrt();
                    assert!(speed < 1., "{piece:?} {speed}");
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    /// Playtest 7f22c32: a side-swiped board in a 12-car race stopped physics
    /// ("invalid contact-group counts or selector") because the pieces pushed
    /// the shared world past the port's old 16-body guard.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS"]
    fn twelve_car_world_keeps_stepping_after_props_break_into_many_pieces() {
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
                let names: Vec<String> = (0..12).map(|i| format!("slot_{i:02}")).collect();
                let slots: Vec<&str> = names.iter().map(String::as_str).collect();
                let mut multi = MultiRuntime::lacetti(root, &slots, &table, events).unwrap();
                multi.enable_dynamic_props(root).unwrap();
                let props = multi.props.as_mut().unwrap();
                // Every tyre stack (12 tyres each) and two boards (2 halves each).
                let chosen: Vec<usize> = {
                    let all = &props.catalog.breakables;
                    let of = |prefix: &str| {
                        all.iter()
                            .enumerate()
                            .filter(|(_, b)| b.template.starts_with(prefix))
                            .map(|(i, _)| i)
                            .collect::<Vec<_>>()
                    };
                    let mut chosen = of("tyrestack");
                    chosen.extend(of("gg_brake_marker").into_iter().take(2));
                    chosen
                };
                let pieces: usize = chosen
                    .iter()
                    .map(|&b| props.catalog.breakables[b].pieces.len())
                    .sum();
                for &b in &chosen {
                    props.queue_break(b);
                }
                let before = multi.bodies().len();
                let drive = Controls {
                    throttle: 0.6,
                    ..Default::default()
                };
                let started = std::time::Instant::now();
                for tick in 0..400 {
                    multi
                        .tick(&[drive; 12])
                        .unwrap_or_else(|e| panic!("12-car tick {}: {e}", tick + 1));
                }
                let props = multi.dynamic_props().unwrap();
                let hash = multi
                    .bodies()
                    .iter()
                    .flatten()
                    .fold(0xcbf29ce484222325u64, |h, &w| {
                        (h ^ u64::from(w)).wrapping_mul(0x100000001b3)
                    });
                eprintln!(
                    "twelve_car_props breaks={} pieces={} bodies={}->{} hash={hash:016x} elapsed={:?}",
                    props.events.len(),
                    props.pieces.len(),
                    before,
                    multi.bodies().len(),
                    started.elapsed()
                );
                assert_eq!(props.events.len(), chosen.len());
                assert_eq!(props.pieces.len(), pieces);
                assert!(pieces > 16);
                assert_eq!(multi.bodies().len(), before + pieces);
                // Every piece rests near where it spawned (none fell through).
                for piece in &props.pieces {
                    let spawn =
                        props.catalog.breakables[piece.breakable].pieces[piece.spec].pose[3];
                    let now = crate::dynamic_props::body_pose(&multi.bodies()[piece.body])[3];
                    assert!(now.iter().all(|v| v.is_finite()));
                    assert!(
                        (now[1] - spawn[1]).abs() < 1.5,
                        "{piece:?} {spawn:?} {now:?}"
                    );
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    /// S3 (docs/RUNTIME_LIMITS_AUDIT.md): before containment any one car's
    /// failure stopped every car. Car 5 fails in its vehicle stage (invalid
    /// controls) and car 9 after the advance (lap timing); both are held
    /// and retired while the other ten race on, reproducibly.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS"]
    fn twelve_car_world_retires_failing_cars_and_races_on() {
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
                let names: Vec<String> = (0..12).map(|i| format!("slot_{i:02}")).collect();
                let slots: Vec<&str> = names.iter().map(String::as_str).collect();
                let hash = |multi: &MultiRuntime| {
                    let words = multi.bodies().iter().flatten().chain(
                        multi
                            .cars
                            .iter()
                            .flat_map(|c| c.state.car.owner.vehicle.iter()),
                    );
                    words.fold(0xcbf29ce484222325u64, |h, &w| {
                        (h ^ u64::from(w)).wrapping_mul(0x100000001b3)
                    })
                };
                const TICKS: u64 = 400;
                // Returns (retirements by tick, poses before and after, final hash).
                let run = |multi: &mut MultiRuntime| {
                    multi.fail_lap_timing = Some((9, 200));
                    let mut retired = Vec::new();
                    let mut held = Vec::new();
                    let mut at_200 = Vec::new();
                    for tick in 1..=TICKS {
                        let mut controls = [Controls {
                            throttle: 0.6,
                            ..Default::default()
                        }; 12];
                        if tick == 120 {
                            controls[5].throttle = f32::NAN;
                        }
                        multi
                            .tick(&controls)
                            .unwrap_or_else(|e| panic!("12-car tick {tick}: {e}"));
                        assert_eq!(multi.ticks, tick);
                        assert!(multi.cars.iter().all(|c| c.ticks == tick));
                        if !multi.newly_retired().is_empty() {
                            retired.push((tick, multi.newly_retired().to_vec()));
                            for &i in multi.newly_retired() {
                                held.push((i, multi.cars[i].pose()));
                            }
                        }
                        if tick == 200 {
                            at_200 = multi.cars.iter().map(Runtime::pose).collect();
                        }
                    }
                    (retired, held, at_200, hash(multi))
                };
                let mut multi = MultiRuntime::lacetti(root, &slots, &table, events).unwrap();
                let started = std::time::Instant::now();
                let (retired, held, at_200, first) = run(&mut multi);
                let elapsed = started.elapsed();
                assert_eq!(retired, [(120, vec![5]), (200, vec![9])]);
                let five = multi.retirement(5).unwrap();
                assert_eq!((five.tick, five.error.0), (120, "invalid driving controls"));
                let nine = multi.retirement(9).unwrap();
                assert_eq!((nine.tick, nine.error.0), (200, "injected lap-timing failure"));
                // Held cars stay where they failed, at rest, with no lap events;
                // the shared world body agrees with the car.
                for (i, pose) in held {
                    assert_eq!(multi.cars[i].pose(), pose, "car {i}");
                    assert_eq!(multi.cars[i].velocity(), [0.; 4], "car {i}");
                    assert_eq!(
                        multi.bodies()[i][KINEMATICS..KINEMATICS + KINEMATIC_WORDS],
                        multi.cars[i].state.car.owner.physical
                            [KINEMATICS..KINEMATICS + KINEMATIC_WORDS]
                    );
                    assert!(multi.cars[i].lap_events.is_empty());
                }
                // The rest of the field keeps driving after both failures.
                let moved: Vec<f32> = (0..12)
                    .map(|i| {
                        let (a, b) = (at_200[i], multi.cars[i].pose());
                        ((b[12] - a[12]).powi(2) + (b[14] - a[14]).powi(2)).sqrt()
                    })
                    .collect();
                eprintln!(
                    "twelve_car_retirement moved_after_200={moved:.1?} hash={first:016x} elapsed={elapsed:?} errors={:?}",
                    [five.error.0, nine.error.0]
                );
                for (i, m) in moved.iter().enumerate() {
                    if i == 5 || i == 9 {
                        assert_eq!(*m, 0.);
                    } else {
                        assert!(multi.retirement(i).is_none());
                        assert!(multi.cars[i].pose().iter().all(|v| v.is_finite()));
                    }
                }
                assert!(moved.iter().filter(|&&m| m > 5.).count() >= 8);
                // Deterministic: a fresh world and a reset world replay it exactly.
                let mut again = MultiRuntime::lacetti(root, &slots, &table, events).unwrap();
                assert_eq!(run(&mut again).3, first);
                multi.reset();
                assert!((0..12).all(|i| multi.retirement(i).is_none()));
                assert_eq!(run(&mut multi).3, first);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    /// Drift Battle AI (docs/DISCIPLINES.md §4.1): a solo AI BMW with the
    /// drift_battle config on longbeach_cl follows the drift line, initiates
    /// and holds drifts that the drift scorer detects, deterministically.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS"]
    fn a_drift_battle_ai_car_drifts_and_is_scored() {
        use crate::ai_driver::{AiData, AiDriver, Behaviour, NORMAL_DIFFICULTY, Parameters, RaceView, View};
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
                let route = Route::new("tracks/usa/long_beach/route_1").unwrap();
                let bmw = Car::new(crate::car::BMW).unwrap();
                let data =
                    AiData::load_for(root, route.path(), "touring", "bmw", Some("drift_battle"))
                        .unwrap();
                let settings = crate::drift::Settings::load(root).unwrap();
                let flags = crate::drift::close_drift_flags(root, &route).unwrap();
                let progress = crate::lap_progress::load_route(root, &route).unwrap().progress;
                let gate = &progress.gates[0];
                let start = std::array::from_fn(|k| (gate.left[k] + gate.right[k]) * 0.5);
                let run = || {
                    let mut multi =
                        MultiRuntime::load_deforming_on(root, &route, &[("slot_00", &bmw)], &table, events, None)
                            .unwrap();
                    let at = |m: &MultiRuntime| {
                        let p = m.cars[0].pose();
                        vec![[p[12], p[13], p[14]]]
                    };
                    let mut standings = crate::race::Standings::new(&data.line, 2, start, &at(&multi));
                    let p = Parameters::new(&data, NORMAL_DIFFICULTY, |_| 1.).unwrap();
                    let mut driver = AiDriver::new(p, 0x5eed);
                    let mut entry = crate::drift::Entry::new(true, Some(1), false);
                    let flag_boxes = multi.close_drift_flag_boxes();
                    let (mut states, mut slip, mut detected, mut drift_line) = ([0u32; 5], 0f32, 0u32, 0u32);
                    for _ in 0..2600 {
                        driver.race = RaceView {
                            place: 0,
                            cars: 1,
                            progress: standings.laps_covered(0) as f32,
                        };
                        multi.cars[0].set_ai_grip(driver.request_grip());
                        let view = View::of(&multi.cars[0], multi.obstruction(0));
                        let c = driver.controls(&data, &view, &[], DELTA);
                        multi.tick(&[c]).unwrap();
                        standings.update(&data.line, &at(&multi), 0., &[]);
                        let contacts: Vec<_> = multi
                            .contact_impacts(0)
                            .iter()
                            .map(|(x, o)| (x.impact, *o))
                            .collect();
                        let (impact, touched) = crate::drift::split_contacts(&contacts, &flag_boxes);
                        if touched {
                            entry.detector.touch_flag();
                        }
                        let sample = crate::drift::Sample::of(
                            &multi.cars[0],
                            standings.laps_covered(0) as f32,
                            0,
                            1,
                            impact,
                        );
                        entry.update(&settings, &flags, &sample, DELTA);
                        let m = driver.telemetry;
                        if m.behaviour == Behaviour::Drift {
                            states[m.drift_state as usize] += 1;
                        }
                        slip = slip.max(m.slip.to_degrees());
                        detected += u32::from(entry.detector.drifting);
                        drift_line += u32::from(m.drift_line);
                    }
                    (states, slip, detected, drift_line, multi.cars[0].pose())
                };
                let first = run();
                eprintln!(
                    "drift_battle_ai states={:?} max_slip={:.1} detected_ticks={} drift_line_ticks={}",
                    first.0, first.1, first.2, first.3
                );
                let (states, slip, detected, drift_line, _) = first;
                // Approach, initiation and drifting all occur, on the drift line.
                assert!(states[1] > 0 && states[2] > 0 && states[3] > 0, "{states:?}");
                // About 22 degrees before the F417 engine reset, 19.1 with
                // engine braking (states [0, 866, 454, 140, 6]).
                assert!(slip > 15., "{slip}");
                // F406: with the original's collision floor lift the car holds
                // slides of about 22 degrees without spinning and the scorer's
                // detector does not latch in these 2600 ticks (states
                // [0, 846, 459, 156, 6]). Before, the body scraped the road,
                // the car spun (179 degrees) and those ticks were detected.
                assert!(slip < 90., "the car spun: {slip} (detected {detected})");
                assert!(drift_line > 2400, "{drift_line}");
                // Deterministic: an identical second run.
                assert_eq!(run().4, first.4);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    /// The original never respawns an AI car (rcResetAllowed is false, so
    /// the reset request req+0xB0 stays 0; docs/RESET_AND_RESPAWN_RESEARCH.md):
    /// a car that cannot drive on is left standing as an obstacle, and the
    /// cars behind it get past with their own avoid and recovery behaviours.
    /// Car 5 is retired and held on the grid at tick 1; car 7 starts right
    /// behind it and, driven by the ported AI, avoids it and races on.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS"]
    fn twelve_car_ai_field_passes_a_car_held_on_the_grid() {
        use crate::ai_driver::{
            AiData, AiDriver, Behaviour, NORMAL_DIFFICULTY, Other, Parameters, View,
        };
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
                let route = crate::route::Route::new("tracks/europe/donington/route_0").unwrap();
                let names: Vec<String> = (0..12).map(|i| format!("slot_{i:02}")).collect();
                let slots: Vec<&str> = names.iter().map(String::as_str).collect();
                let mut multi = MultiRuntime::lacetti(root, &slots, &table, events).unwrap();
                let data = AiData::load(root, route.path(), "touring", "lac").unwrap();
                let mut drivers: Vec<AiDriver> = (0..12)
                    .map(|i| {
                        let p = Parameters::new(&data, NORMAL_DIFFICULTY, |_| 1.).unwrap();
                        AiDriver::new(p, 0x5eed + i as u32)
                    })
                    .collect();
                let positions = |multi: &MultiRuntime| -> Vec<[f32; 3]> {
                    multi.cars.iter().map(|c| {
                        let p = c.pose();
                        [p[12], p[13], p[14]]
                    }).collect()
                };
                let progress = crate::lap_progress::load_route(root, &route).unwrap().progress;
                let gate = &progress.gates[0];
                let start = std::array::from_fn(|k| (gate.left[k] + gate.right[k]) * 0.5);
                let mut standings =
                    crate::race::Standings::new(&data.line, 2, start, &positions(&multi));
                let mut avoided = false;
                let mut contacts_with_held = 0;
                const TICKS: u64 = 900;
                for tick in 1..=TICKS {
                    let others: Vec<Other> = multi
                        .cars
                        .iter()
                        .enumerate()
                        .map(|(i, c)| Other::of(i as u32 + 1, c))
                        .collect();
                    let mut controls: Vec<Controls> = (0..12)
                        .map(|i| {
                            let seen: Vec<Other> = others
                                .iter()
                                .enumerate()
                                .filter(|&(j, _)| j != i)
                                .map(|(_, o)| *o)
                                .collect();
                            let view = View::of(&multi.cars[i], multi.obstruction(i));
                            drivers[i].controls(&data, &view, &seen, DELTA)
                        })
                        .collect();
                    if tick == 1 {
                        controls[5].throttle = f32::NAN;
                    }
                    avoided |= drivers[7].telemetry.behaviour == Behaviour::Avoid;
                    multi
                        .tick(&controls)
                        .unwrap_or_else(|e| panic!("12-car AI tick {tick}: {e}"));
                    if tick == 1 {
                        assert_eq!(multi.newly_retired(), [5]);
                    }
                    contacts_with_held += multi
                        .contact_pairs()
                        .iter()
                        .filter(|&&(a, b)| a == 5 || b == 5)
                        .count();
                    standings.update(
                        &data.line,
                        &positions(&multi),
                        tick as f64 * f64::from(DELTA),
                        &[],
                    );
                }
                let covered: Vec<f64> = (0..12).map(|i| standings.laps_covered(i)).collect();
                eprintln!(
                    "held_car_on_grid laps_covered={covered:.3?} car7_avoided={avoided} contacts_with_held={contacts_with_held}"
                );
                assert!(multi.retirement(5).is_some());
                assert_eq!(multi.cars[5].velocity(), [0.; 4]);
                assert!(avoided, "car 7 never chose avoid");
                // About 80 m of line past the held car for every other car.
                for (i, &c) in covered.iter().enumerate() {
                    if i != 5 {
                        assert!(multi.retirement(i).is_none(), "car {i}");
                        assert!(c > covered[5] + 0.02, "car {i} at {c:.3} laps, held car at {:.3}", covered[5]);
                    }
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    /// P12 (docs/PERFORMANCE.md): the per-car stages that run on the compute
    /// pool give bit-identical results to the serial tick. A deforming
    /// 12-car AI race with breakable props and a car retired on the grid runs
    /// twice, serially and in parallel; every body and every car's vehicle
    /// record are compared tick by tick.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS / GRID_DEFORMATION_PARAMETERS"]
    fn twelve_car_race_is_identical_on_one_thread_and_many() {
        use crate::ai_driver::{AiData, AiDriver, NORMAL_DIFFICULTY, Other, Parameters, View};
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
                let route = crate::route::Route::new("tracks/europe/donington/route_0").unwrap();
                let data = AiData::load(root, route.path(), "touring", "lac").unwrap();
                let names: Vec<String> = (0..12).map(|i| format!("slot_{i:02}")).collect();
                let slots: Vec<&str> = names.iter().map(String::as_str).collect();
                let field = |parallel: bool| {
                    let mut multi = MultiRuntime::lacetti_deforming_on(
                        root,
                        &route,
                        &slots,
                        &table,
                        events,
                        Some(deformation.clone()),
                    )
                    .unwrap();
                    multi.enable_dynamic_props(root).unwrap();
                    multi.parallel = parallel;
                    let drivers: Vec<AiDriver> = (0..12)
                        .map(|i| {
                            let p = Parameters::new(&data, NORMAL_DIFFICULTY, |_| 1.).unwrap();
                            AiDriver::new(p, 0x5eed + i as u32)
                        })
                        .collect();
                    (multi, drivers)
                };
                let step = |(multi, drivers): &mut (MultiRuntime, Vec<AiDriver>), tick: u64| {
                    let others: Vec<Other> = multi
                        .cars
                        .iter()
                        .enumerate()
                        .map(|(i, c)| Other::of(i as u32 + 1, c))
                        .collect();
                    let mut controls: Vec<Controls> = (0..12)
                        .map(|i| {
                            let seen: Vec<Other> = others
                                .iter()
                                .enumerate()
                                .filter(|&(j, _)| j != i)
                                .map(|(_, o)| *o)
                                .collect();
                            let view = View::of(&multi.cars[i], multi.obstruction(i));
                            drivers[i].controls(&data, &view, &seen, DELTA)
                        })
                        .collect();
                    // Car 5 fails in its vehicle stage and is held on the grid.
                    if tick == 1 {
                        controls[5].throttle = f32::NAN;
                    }
                    multi
                        .tick(&controls)
                        .unwrap_or_else(|e| panic!("tick {tick}: {e}"));
                };
                let words = |multi: &MultiRuntime| -> Vec<u32> {
                    let mut all: Vec<u32> = multi.bodies().iter().flatten().copied().collect();
                    for car in &multi.cars {
                        all.extend(&car.state.car.owner.vehicle);
                        all.extend(car.state.car.owner.physical);
                        all.extend(&car.state.car.frame.body);
                        if let Some(r) = &car.deformation_registration {
                            all.extend([r.bulk_calls as u32, r.records as u32]);
                        }
                    }
                    all
                };
                let (mut serial, mut parallel) = (field(false), field(true));
                let mut car_contacts = 0;
                for tick in 1..=1500 {
                    step(&mut serial, tick);
                    step(&mut parallel, tick);
                    assert_eq!(
                        serial.0.last_active, parallel.0.last_active,
                        "active contacts at tick {tick}"
                    );
                    assert!(words(&serial.0) == words(&parallel.0), "state at tick {tick}");
                    car_contacts += serial.0.last_car_contacts;
                }
                let threads = crate::parallel::threads();
                eprintln!(
                    "parallel_identity threads={threads} car_contacts={car_contacts} bodies={} retired={:?}",
                    serial.0.bodies().len(),
                    serial.0.retired.iter().map(Option::is_some).collect::<Vec<_>>()
                );
                assert!(threads > 1, "the compute pool has one thread");
                assert!(car_contacts > 0, "no car-to-car contact was compared");
                assert!(serial.0.retired[5].is_some());
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
