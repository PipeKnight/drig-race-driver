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
    /// Parking around the player (on in the game; tests measuring a pile
    /// near the player switch it off).
    parking: bool,
    /// Time each detached panel's body has been turned over (loose_panel::settle).
    panel_tipping: std::collections::BTreeMap<usize, f32>,
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

/// A body [`MultiRuntime::unsupported`] found asleep or parked without
/// support, or a held car.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Unsupported {
    pub body: usize,
    pub asleep: bool,
    pub parked: bool,
    /// A retired car, held at its last pose.
    pub held: bool,
    /// How far it fell in the woken clone (m).
    pub drop: f32,
    /// Height of its centre above the track surface below it.
    pub gap: Option<f32>,
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
        let mut manager = vehicle_manager::calculate_with_world_publication_staged(
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
                    .deformation_publication_owned(owner, delta)?;
                if geometry.is_some() {
                    rebuilt = geometry;
                }
                Ok(next)
            },
        )?;
        published = rebuilt;
        if let Some(current) = &car.deformation_registration {
            registration = Some(deformation_world::Registration {
                // Moved, not copied: the staged state's vehicle is not read
                // again before the body events take the registration's back.
                owner: std::mem::take(&mut manager.job.state.owner.vehicle),
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
        let random = state
            .owner
            .damage_random_state
            .ok_or(Error("body impact requires owned damage RNG"))?;
        // In place: an error discards this car's staged state.
        let (random, _) = body_impact::calculate_in_place(
            &mut state.owner,
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
            random,
        )?;
        state.owner.damage_random_state = Some(random);
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
            parking: true,
            panel_tipping: Default::default(),
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

    /// Diagnostic: the last tick's world contacts that name body `body`
    /// (index), as (body a, body b or none for static, normal, point,
    /// penetration +0x38, solved force +0x3C, shape tokens +0x28/+0x2C).
    #[allow(clippy::type_complexity)]
    pub fn contacts_with(&self, body: usize) -> Vec<(usize, Option<usize>, [f32; 3], [f32; 3], f32, f32, [u32; 2])> {
        let g = |c: &contact_groups::Contact, o: usize| f32::from_bits(c[o / 4]);
        (self.world.groups.contacts.iter())
            .filter(|c| **c != contact_groups::EMPTY_CONTACT)
            .filter(|c| c[1] as usize == body + 1 || c[2] as usize == body + 1)
            .map(|c| {
                (
                    c[1] as usize - 1,
                    (c[2] != 0).then(|| c[2] as usize - 1),
                    [g(c, 0x60), g(c, 0x64), g(c, 0x68)],
                    [g(c, 0x50), g(c, 0x54), g(c, 0x58)],
                    g(c, 0x38),
                    g(c, 0x3c),
                    [c[0x28 / 4], c[0x2c / 4]],
                )
            })
            .collect()
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

    /// Diagnostic (owner's report of 23 October 2026, bodies asleep in the
    /// air): steps two clones `ticks` times with idle controls, one as it
    /// is and one with every asleep piece or lost wheel woken and parking
    /// off. A body that is asleep or parked here, stays put in the first
    /// clone and falls more than 0.2 m in the second lacked support. Held
    /// (retired) cars are listed too, with their height above the track.
    pub fn unsupported(&self, ticks: usize) -> Result<Vec<Unsupported>, Error> {
        let cars = self.cars.len();
        let n = self.bodies().len();
        let height = |m: &Self, i: usize| f32::from_bits(m.bodies()[i][0x204 / 4]);
        let state = |i: usize| {
            let b = &self.bodies()[i];
            (b[0x400 / 4] == contact_groups::ASLEEP, (b[0x410 / 4] >> 24) & 255 != 0)
        };
        let gap = |i: usize| {
            let p = [0x1c0, 0x1c4, 0x1c8].map(|o| f32::from_bits(self.bodies()[i][o / 4]));
            self.cars[0].ground_below(p).map(|y| p[1] - y)
        };
        let mut out: Vec<Unsupported> = (0..cars)
            .filter(|&i| self.retired[i].is_some())
            .map(|i| Unsupported { body: i, asleep: false, parked: false, held: true, drop: 0., gap: gap(i) })
            .collect();
        let (mut kept, mut woken) = (self.clone(), self.clone());
        woken.parking = false;
        for i in cars..n {
            if grouped_physical_frame::asleep(&woken.world, i) {
                grouped_physical_frame::wake(&mut woken.world, i);
            }
        }
        let idle = vec![Controls::default(); cars];
        for _ in 0..ticks {
            kept.tick(&idle)?;
            woken.tick(&idle)?;
        }
        for i in cars..n {
            let (asleep, parked) = state(i);
            let before = height(self, i);
            let drop = before - height(&woken, i);
            if (asleep || parked) && (height(&kept, i) - before).abs() < 1e-3 && drop > 0.2 {
                out.push(Unsupported { body: i, asleep, parked, held: false, drop, gap: gap(i) });
            }
        }
        Ok(out)
    }

    /// Diagnostics: every detached panel's body and how it lies on the
    /// track ([`crate::loose_panel::poses`]).
    pub fn panels(&self) -> Vec<crate::loose_panel::PanelPose> {
        let track = &self.cars[0];
        self.props.as_ref().map_or_else(Vec::new, |props| {
            crate::loose_panel::poses(props, self.bodies(), |p| track.ground_below(p))
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

    /// Whether car `i` pushed a broken flag's pole this tick: the 1e-6 body
    /// contact 0x7C4830 sends the car (0xA01910) for each touched post.
    pub fn touched_post(&self, i: usize) -> bool {
        self.props.as_ref().is_some_and(|p| p.post_touches.contains(&i))
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
        self.panel_tipping.clear();
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
        // The one copy of the world per tick: the step consumes it, and on
        // an error `self.world` keeps the last committed bodies and contacts.
        // The solver rows (four per contact slot, megabytes with a few
        // hundred pieces) move into the step instead of being copied: after
        // a failed tick the world has none and refuses to step again until
        // `reset` (every caller stops or restarts the event on an error).
        // The wheel contacts go straight into the copy's pool prefix.
        self.world.manager.park = (self.props.is_some() && self.parking)
            .then(|| crate::dynamic_props::park_reference(&self.world.groups.bodies[0]));
        let rows = std::mem::take(&mut self.world.manager.rows);
        let clone_timer = crate::perf::Timer::start(crate::perf::Stage::WorldClone);
        let mut previous = self.world.clone();
        drop(clone_timer);
        previous.manager.rows = rows;
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
                            let mut c = wheel_contacts::overlay(&previous.groups.contacts[used], &c);
                            c[1] = i as u32 + 1;
                            c[0x28 / 4] = tokens[i];
                            previous.groups.contacts[used] = c;
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
        let mut provider_calls = 0;
        let mut provider = |bodies: &mut [contact_groups::Body], remaining: usize| {
            let provider_started = crate::perf::enabled().then(std::time::Instant::now);
            // A second call collects again after a wake (B184F0); the
            // continuous stage ran in the first.
            provider_calls += 1;
            // World phase 2 (continuous) before the contacts of phase 3.
            if let Some(props) = props.filter(|p| p.has_bodies() && provider_calls == 1) {
                let _continuous_timer = crate::perf::Timer::start(crate::perf::Stage::Continuous);
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
                let more = props.contacts(
                    &cars[0].track,
                    &cars[0].surfaces,
                    bodies,
                    &hulls,
                    geometry[0].query(),
                    remaining - out.len(),
                    parallel,
                )?;
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
            physical_world::advance_with_deformation_in(
                previous,
                wheel_contacts,
                &self.shapes,
                &mut provider,
                &mut owned,
            )?
        } else {
            physical_world::advance_with_geometry_provider_in(
                previous,
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
            let _prop_timer = crate::perf::Timer::start(crate::perf::Stage::PropCommit);
            let clone_timer = crate::perf::Timer::start(crate::perf::Stage::PropClone);
            let mut next = current.clone();
            drop(clone_timer);
            let detect_timer = crate::perf::Timer::start(crate::perf::Stage::PropDetect);
            next.detect(&world.groups.contacts, &recorded, &world.groups.bodies);
            drop(detect_timer);
            let clone_timer = crate::perf::Timer::start(crate::perf::Stage::PropClone);
            let mut changed = self.shapes.clone();
            drop(clone_timer);
            let detect_timer = crate::perf::Timer::start(crate::perf::Stage::PropDetect);
            next.hold_lost(&mut world.groups.bodies, self.cars[0].track.bounds()[1]);
            drop(detect_timer);
            let lift_timer = crate::perf::Timer::start(crate::perf::Stage::LiftSunk);
            next.lift_sunk(&self.cars[0].track, &mut world.groups.bodies, self.parallel)?;
            drop(lift_timer);
            let apply_timer = crate::perf::Timer::start(crate::perf::Stage::PropApply);
            let disabled = next.apply(&mut world, &mut changed, self.ticks + 1)?;
            drop(apply_timer);
            // 0x7C4830: the phantom posts after the step and the breaks.
            next.post_touches.clear();
            if !next.posts.is_empty() {
                let mut boxes = Vec::with_capacity(geometry.len());
                for (i, g) in geometry.iter().enumerate() {
                    let body = &world.groups.bodies[i];
                    let motion = std::array::from_fn(|k| {
                        f32::from_bits(body[0x1c0 / 4 + k]) - f32::from_bits(body[0x2a0 / 4 + k])
                    });
                    boxes.push(crate::dynamic_props::PostCar {
                        shape: g.car_box(body)?,
                        motion,
                    });
                }
                // The touched cars' 1e-6 contacts (0xA01910) are not sent yet.
                next.post_touches = next.update_posts(&mut world, &boxes)?;
            }
            if !disabled.is_empty() {
                shapes = Some(changed);
            }
            props_next = Some((next, disabled));
        }
        let active = wheel_contacts + body_contacts + car_contacts + piece_contacts;
        if crate::perf::enabled() {
            let bodies = &world.groups.bodies;
            // +0x400 = -1 asleep; parked bodies also carry byte +0x413.
            let asleep = bodies.iter().filter(|b| b[0x400 / 4] == u32::MAX).count();
            let parked = bodies
                .iter()
                .filter(|b| (b[0x410 / 4] >> 24) & 255 != 0)
                .count();
            crate::perf::world(
                bodies.len(),
                bodies.len() - n,
                [wheel_contacts, body_contacts, car_contacts, piece_contacts],
                [asleep, parked],
            );
        }
        for (i, error) in contact_failed.into_iter().enumerate() {
            if let Some(error) = error
                && failed[i].is_none()
            {
                failed[i] = Some(error);
                staged[i] = None;
            }
        }
        let events_timer = crate::perf::Timer::start(crate::perf::Stage::BodyEvents);
        // Each car's body events touch only its own staged state, registration
        // and damage RNG, so the cars run on the compute pool, one run of cars
        // per thread, and are joined in car order.
        type Job = (vehicle_manager::State, Option<deformation_world::Registration>);
        let jobs: Vec<std::sync::Mutex<Option<Job>>> = staged
            .iter_mut()
            .zip(registrations.iter_mut())
            .map(|(slot, registration)| {
                std::sync::Mutex::new(slot.take().map(|state| (state, registration.take())))
            })
            .collect();
        let runs = if self.parallel { crate::parallel::threads().min(n) } else { 1 };
        let per_run = n.div_ceil(runs.max(1));
        let events = {
            let (cars, bodies, tokens, jobs) = (&self.cars, &world.groups.bodies, &tokens, &jobs);
            let contacts = &world.groups.contacts[..active.min(world.groups.contacts.len())];
            let parameter = world.manager.parameters[0];
            crate::parallel::map(runs, runs > 1, |r| {
                (r * per_run..((r + 1) * per_run).min(n))
                    .map(|i| {
                        let (mut state, mut registration) =
                            jobs[i].lock().unwrap_or_else(|e| e.into_inner()).take()?;
                        // The registration's owner is moved into the car; the registration
                        // keeps an empty owner until the next vehicle stage moves the car's
                        // record back in (`deformation_world::Registration::owner`).
                        if let Some(registration) = &mut registration {
                            std::mem::swap(&mut state.owner.vehicle, &mut registration.owner);
                        }
                        state.owner.physical = bodies[i];
                        if let Some(parameters) = cars[i].body_events
                            && let Err(error) =
                                body_events(&mut state, parameters, contacts, tokens[i], parameter)
                        {
                            return Some(Err((error, registration)));
                        }
                        if registration.is_some() {
                            state
                                .frame
                                .body
                                .copy_from_slice(&state.owner.vehicle[..vehicle_force::BODY_WORDS]);
                        }
                        Some(Ok((state, registration)))
                    })
                    .collect::<Vec<_>>()
            })
        };
        for (i, event) in events.into_iter().flatten().enumerate() {
            match event {
                None => {}
                Some(Ok((state, registration))) => {
                    staged[i] = Some(state);
                    registrations[i] = registration;
                }
                Some(Err((error, registration))) => {
                    failed[i] = Some(error);
                    registrations[i] = registration;
                }
            }
        }
        drop(events_timer);
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
        // Detached panels standing on an edge are turned over (AUTHORED).
        if let Some(props) = &self.props {
            let _settle_timer = crate::perf::Timer::start(crate::perf::Stage::PanelSettle);
            let track = &self.cars[0];
            if crate::loose_panel::settle(
                props,
                &mut self.world,
                |p| track.ground_below(p),
                &mut self.panel_tipping,
                DELTA,
            ) {
                // Waking relinks the active list; keep the cars' copies in step.
                for (j, car) in self.cars.iter_mut().enumerate() {
                    car.state.car.owner.physical[0x300 / 4] =
                        self.world.groups.bodies[j][0x300 / 4];
                }
            }
        }
        // The loose-part rules of every car still in the world; a lost wheel
        // or a detached panel becomes a body of the shared world.
        let _parts_timer = crate::perf::Timer::start(crate::perf::Stage::LooseParts);
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

    /// Add car `i`'s wheels `lost` this tick and its body parts detached this
    /// tick as bodies of the shared world.
    pub(crate) fn add_loose_wheels(&mut self, i: usize, lost: [bool; 4]) -> Result<(), Error> {
        let spawns = self.cars[i].loose_wheels(lost, i)?;
        let panels = self.cars[i].loose_panels()?;
        if spawns.is_empty() && panels.is_empty() {
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
        for (part, s) in panels {
            props.add_panel(
                &mut self.world,
                &mut self.shapes,
                i,
                part,
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
                    .map(|j| distance(pose(&multi, j.a), pose(&multi, j.b.unwrap())))
                    .collect();
                for _ in 0..300 {
                    multi.tick(&drive).unwrap();
                }
                let props = multi.dynamic_props().unwrap();
                assert_eq!(props.pieces.len(), 22);
                for (j, rest) in props.joints.iter().zip(&rest) {
                    assert!(!j.broken);
                    let now = distance(pose(&multi, j.a), pose(&multi, j.b.unwrap()));
                    eprintln!("tyre_wall joint {:?} rest={rest:.3} now={now:.3}", (j.a, j.b));
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

    /// An armco section in the middle of a chain breaks its two linked
    /// neighbours too (0x7D5540); the rails hang between the posts and the
    /// world and stay near their places.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS"]
    fn a_broken_armco_breaks_its_neighbours_and_hangs_together() {
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
                let all = &props.catalog.breakables;
                // A section that holds a link and is itself linked to.
                let x = (0..all.len())
                    .find(|&i| {
                        all[i].template.starts_with("armco")
                            && !all[i].links.is_empty()
                            && all.iter().any(|h| h.links.iter().any(|l| l.primary == i))
                    })
                    .unwrap();
                props.queue_break(x);
                let drive = [Controls::default(); 1];
                multi.tick(&drive).unwrap();
                let props = multi.dynamic_props().unwrap();
                let broken = props.broken.iter().filter(|b| **b).count();
                eprintln!(
                    "armco {x}: broken={broken} pieces={} statics={:?} joints={:?}",
                    props.pieces.len(),
                    props.statics,
                    props.joints
                );
                assert_eq!(broken, 3);
                // The middle section's post is dynamic and joined to the
                // holder's last rail; the outer posts are still static.
                assert!(!props.statics.iter().any(|s| s.0 == x));
                assert_eq!(props.statics.len(), 1);
                for _ in 0..300 {
                    multi.tick(&drive).unwrap();
                }
                let props = multi.dynamic_props().unwrap();
                for piece in &props.pieces {
                    let spawn =
                        props.catalog.breakables[piece.breakable].pieces[piece.spec].pose[3];
                    let now = crate::dynamic_props::body_pose(&multi.bodies()[piece.body])[3];
                    assert!(now.iter().all(|v| v.is_finite()));
                    let moved = ((now[0] - spawn[0]).powi(2)
                        + (now[1] - spawn[1]).powi(2)
                        + (now[2] - spawn[2]).powi(2))
                    .sqrt();
                    eprintln!("{piece:?} moved {moved:.3}");
                    assert!(moved < 1.5, "{piece:?} {spawn:?} {now:?}");
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    /// A broken close-drift flag's pole is a phantom post: a car beside it
    /// pushes it aside about its pivot (0x7BD240), the pole keeps zero
    /// velocity and the car is reported as touching it.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS"]
    fn a_car_beside_a_broken_drift_flag_pushes_its_pole_aside() {
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
                let route = Route::new("tracks/europe/jarama/route_0").unwrap();
                let mut multi =
                    MultiRuntime::lacetti_on(root, &route, &["slot_00"], &table, events).unwrap();
                multi.enable_dynamic_props(root).unwrap();
                let props = multi.props.as_mut().unwrap();
                let flag = (props.catalog.breakables.iter())
                    .position(|b| b.template == "close_drift_flag.max")
                    .unwrap();
                props.queue_break(flag);
                let drive = [Controls::default(); 1];
                multi.tick(&drive).unwrap();
                let props = multi.dynamic_props().unwrap();
                assert_eq!(props.posts.len(), 1);
                let post = props.posts[0].clone();
                let pivot = post.post.pivot;
                eprintln!("post {post:?}");
                // Standing on its spring, about upright.
                assert!(post.post.direction[1] > 0.99, "{post:?}");
                assert!(props.post_touches.is_empty());
                // The car beside the pole, its side box over it.
                multi.place_at_rest(0, [pivot[0] + 0.7, pivot[1], pivot[2]], [0., 0., 1.]).unwrap();
                multi.tick(&drive).unwrap();
                let props = multi.dynamic_props().unwrap();
                let after = &props.posts[0];
                eprintln!("after {after:?} touches {:?}", props.post_touches);
                assert_eq!(props.post_touches, [0]);
                assert!(multi.touched_post(0));
                assert!(after.post.direction[1] < post.post.direction[1], "{after:?}");
                // Turned about the pivot: the pivot stays, the pole's centre is
                // still half the pole away from it.
                for (now, was) in after.post.pivot.iter().zip(&pivot).take(3) {
                    assert!((now - was).abs() < 1e-3, "{after:?}");
                }
                assert!((after.post.length - post.post.length).abs() < 1e-3, "{after:?}");
            })
            .unwrap()
            .join()
            .unwrap();
    }

    /// Career run 13 (Le Mans, F512): a car crashing into a yellow tyre wall
    /// broke it and its linked neighbours into 68 pieces, and two ticks later
    /// one piece's angular correction passed the port's old 64 rad bound
    /// (lifted; the original has none). A Lacetti driven into the same wall
    /// from either side breaks it and the field keeps stepping; these runs
    /// stay below 3 rad, so the bound itself is tested in `physical_pose`.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS"]
    fn a_le_mans_tyre_wall_broken_around_a_car_keeps_stepping() {
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
                let route = Route::new("tracks/europe/le_mans/route_0").unwrap();
                let mut multi =
                    MultiRuntime::lacetti_on(root, &route, &["slot_00", "slot_01"], &table, events)
                        .unwrap();
                multi.enable_dynamic_props(root).unwrap();
                let props = multi.props.as_mut().unwrap();
                let wall = 969;
                let entry = &props.catalog.breakables[wall];
                assert_eq!(entry.template, "tyrewall_yellow.max");
                eprintln!("pieces {} joints {}", entry.pieces.len(), entry.joints.len());
                let at = entry.placement;
                let (x, z) = ([at[0], at[1], at[2]], [at[8], at[9], at[10]]);
                // Into the wall from either side, at speed, across its length.
                for (side, along) in [(1f32, 0f32), (-1., 0.), (1., 1.5), (-1., -1.5), (1., 3.), (-1., 3.)] {
                    multi.reset();
                    let start: [f32; 3] =
                        std::array::from_fn(|k| [at[12], at[13], at[14]][k] + side * 40. * z[k] + along * x[k]);
                    let heading = z.map(|v| -side * v);
                    multi.place_at_rest(0, start, heading).unwrap();
                    eprintln!("VARIANT side={side} along={along}");
                    let mut drive = [Controls::default(); 2];
                    drive[0].throttle = 1.;
                    for tick in 0..420 {
                        if let Err(e) = multi.tick(&drive) {
                            panic!("side {side} along {along} tick {tick}: {}", e.0);
                        }
                    }
                    let props = multi.dynamic_props().unwrap();
                    eprintln!("VARIANT broken={} pieces={}", props.broken.iter().filter(|b| **b).count(), props.pieces.len());
                }
                let props = multi.dynamic_props().unwrap();
                eprintln!("pieces {}", props.pieces.len());
            })
            .unwrap()
            .join()
            .unwrap();
    }

    /// Far pieces rest before they stop (F561, owner's request; BODY_SLEEPING
    /// section 8): car 1 breaks a Le Mans tyre wall more than 80 m from the
    /// player's car 0. B7B850 parked the moving pieces on their first static
    /// contact (status 2, B24DF0); now a body counts as outside only after
    /// turnOffTime at rest, so none parks in flight, the pile settles and
    /// sleeps (status 1 comes first), and nothing asleep lacks support.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS"]
    fn pieces_far_from_the_player_settle_before_they_stop() {
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
                let route = Route::new("tracks/europe/le_mans/route_0").unwrap();
                let mut multi =
                    MultiRuntime::lacetti_on(root, &route, &["slot_00", "slot_01"], &table, events)
                        .unwrap();
                multi.enable_dynamic_props(root).unwrap();
                let at = multi.props.as_ref().unwrap().catalog.breakables[969].placement;
                let z = [at[8], at[9], at[10]];
                let centre = [at[12], at[13], at[14]];
                let player = crate::dynamic_props::park_reference(&multi.bodies()[0]).0;
                let away = (0..3)
                    .map(|k| (player[k] - centre[k]).powi(2))
                    .sum::<f32>()
                    .sqrt();
                assert!(away > 80., "the player is {away} m from the wall");
                let start: [f32; 3] = std::array::from_fn(|k| centre[k] + 40. * z[k]);
                multi.place_at_rest(1, start, z.map(|v| -v)).unwrap();
                let cars = multi.cars.len();
                let count = |multi: &MultiRuntime, f: &dyn Fn(&contact_groups::Body) -> bool| {
                    multi.bodies()[cars..].iter().filter(|b| f(b)).count()
                };
                let mut drive = [Controls::default(); 2];
                drive[1].throttle = 1.;
                for tick in 0..900 {
                    if tick == 240 {
                        drive[1] = Controls { brake: 1., ..Controls::default() };
                    }
                    multi.tick(&drive).unwrap();
                    let parked = count(&multi, &|b| (b[0x410 / 4] >> 24) & 255 != 0);
                    assert_eq!(parked, 0, "tick {tick}: parked before resting");
                }
                let pieces = multi.bodies().len() - cars;
                let asleep = count(&multi, &|b| b[0x400 / 4] == contact_groups::ASLEEP);
                let hanging = multi.unsupported(120).unwrap();
                eprintln!("pieces {pieces} asleep {asleep} unsupported {hanging:?}");
                assert!(pieces > 0 && asleep > 0, "{asleep} of {pieces} asleep");
                assert!(hanging.is_empty(), "{hanging:?}");
            })
            .unwrap()
            .join()
            .unwrap();
    }

    /// No piece ends a step sunk into the road (AUTHORED, owner's request,
    /// F564): the car driven into the Le Mans tyre wall 969 at full throttle
    /// climbs the tyres and presses them down; every tick each piece's hull
    /// bottom stays within [`crate::dynamic_props::SINK_TOLERANCE`] of the
    /// road under it.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS"]
    fn pieces_pressed_by_a_car_stay_above_the_road() {
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
                let route = Route::new("tracks/europe/le_mans/route_0").unwrap();
                let mut multi =
                    MultiRuntime::lacetti_on(root, &route, &["slot_00", "slot_01"], &table, events)
                        .unwrap();
                multi.enable_dynamic_props(root).unwrap();
                let at = multi.props.as_ref().unwrap().catalog.breakables[969].placement;
                let z = [at[8], at[9], at[10]];
                let centre = [at[12], at[13], at[14]];
                let start: [f32; 3] = std::array::from_fn(|k| centre[k] + 40. * z[k]);
                multi.place_at_rest(1, start, z.map(|v| -v)).unwrap();
                let mut drive = [Controls::default(); 2];
                drive[1].throttle = 1.;
                let (mut deepest, mut checked) = ((0., 0, 0), 0);
                for tick in 0..600 {
                    multi.tick(&drive).unwrap();
                    let props = multi.props.as_ref().unwrap();
                    for d in props.road_depths(multi.cars[0].track(), multi.bodies()) {
                        checked += 1;
                        if d.depth > deepest.0 {
                            deepest = (d.depth, d.body, tick);
                        }
                    }
                }
                eprintln!("deepest {deepest:?} of {checked} piece ticks");
                assert!(checked > 1000, "{checked}");
                assert!(
                    deepest.0 <= crate::dynamic_props::SINK_TOLERANCE + 1e-3,
                    "body {} sank {} m at tick {}",
                    deepest.1,
                    deepest.0,
                    deepest.2
                );
            })
            .unwrap()
            .join()
            .unwrap();
    }

    /// Only a car breaks a prop (0x7C46A0 pairs a car shape's byte 0x17 with
    /// the prop's 0x18; F557): the car driven through the Nürburgring
    /// tyrestack_white 2276 the owner's phone hit, then braked to a stop in
    /// the pile, leaves the broken set unchanged while it stands. Before,
    /// pieces resting against the neighbouring stacks broke them at the
    /// 50 N threshold, 83 pieces became 207 with the car at rest.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS"]
    fn pieces_resting_against_intact_stacks_do_not_break_them() {
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
                let route = Route::new("tracks/europe/nurburgring/route_1").unwrap();
                let mut multi =
                    MultiRuntime::lacetti_on(root, &route, &["slot_00", "slot_01"], &table, events)
                        .unwrap();
                multi.enable_dynamic_props(root).unwrap();
                multi.parking = false;
                let props = multi.props.as_ref().unwrap();
                let at = props.catalog.breakables[2276].placement;
                assert!(props.catalog.breakables[2276].template.starts_with("tyrestack_white"));
                let start: [f32; 3] = std::array::from_fn(|k| at[12 + k] + 60. * at[8 + k]);
                multi.place_at_rest(0, start, [-at[8], -at[9], -at[10]]).unwrap();
                let mut go = [Controls::default(); 2];
                go[0].throttle = 1.;
                let mut stop = [Controls::default(); 2];
                stop[0].brake = 1.;
                let broken = |multi: &MultiRuntime| multi.dynamic_props().unwrap().broken.clone();
                for tick in 0..600 {
                    multi.tick(if tick < 300 { &go } else { &stop }).unwrap();
                }
                let speed: f32 = (0..3)
                    .map(|k| f32::from_bits(multi.bodies()[0][0x220 / 4 + k]).powi(2))
                    .sum::<f32>()
                    .sqrt();
                let stopped = broken(&multi);
                let count = stopped.iter().filter(|b| **b).count();
                eprintln!("car speed {speed:.3} broken {count}");
                assert!(speed < 0.5, "{speed}");
                assert!(stopped[2276] && count >= 2, "{count}");
                for _ in 0..1200 {
                    multi.tick(&stop).unwrap();
                }
                let later = broken(&multi);
                let more: Vec<usize> = (0..later.len()).filter(|&i| later[i] && !stopped[i]).collect();
                assert!(more.is_empty(), "broken with the car at rest: {more:?}");
            })
            .unwrap()
            .join()
            .unwrap();
    }

    /// Pile settle probe (F549): the Nürburgring tyrewall_black_yok
    /// breakables the owner's phone broke (1732..1752, log 004650), every one
    /// within 30 m of 1745 broken at rest, or (`GRID_PILE_DRIVE`) the player
    /// driving through 1745. Prints the asleep and moving counts and the tick
    /// cost every second, the ticks until half, 90 % and all pieces sleep, at
    /// `GRID_PILE_DUMP` the awake pieces with their contacts and with
    /// `GRID_PILE_TRACE` one body per tick. Parking is off. The default pile
    /// at rest must sleep whole within 15 s.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS"]
    fn tyre_pile_settle_probe() {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(|| {
                let f = |b: &contact_groups::Body, o: usize| f32::from_bits(b[o / 4]);
                let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
                let root = Path::new(&root);
                let read = |name| std::fs::read(std::env::var_os(name).unwrap()).unwrap();
                let table = read("GRID_CONTACT_DIRECTIONS");
                let events = Some(
                    crate::body_impact_parameters::read(&read("GRID_BODY_EVENT_PARAMETERS"))
                        .unwrap(),
                );
                let env = |name: &str, default: usize| {
                    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
                };
                let route = std::env::var("GRID_PILE_ROUTE")
                    .unwrap_or_else(|_| "tracks/europe/nurburgring/route_1".into());
                let route = Route::new(&route).unwrap();
                let mut multi =
                    MultiRuntime::lacetti_on(root, &route, &["slot_00", "slot_01"], &table, events)
                        .unwrap();
                multi.enable_dynamic_props(root).unwrap();
                multi.parking = false;
                let props = multi.props.as_mut().unwrap();
                let template =
                    std::env::var("GRID_PILE_TEMPLATE").unwrap_or_else(|_| "tyrewall_black_yok".into());
                let centre = env("GRID_PILE_CENTRE", 1745);
                let at = props.catalog.breakables[centre].placement;
                let radius = env("GRID_PILE_RADIUS", 30) as f32;
                // GRID_PILE_DRIVE=ticks: car 0 drives into the centre wall at full
                // throttle for that long, then brakes; nothing breaks at rest.
                let drive = env("GRID_PILE_DRIVE", 0);
                let mut chosen = Vec::new();
                for b in 0..props.catalog.breakables.len() {
                    let e = &props.catalog.breakables[b];
                    let p = e.placement;
                    let d: f32 = (12..15).map(|k| (p[k] - at[k]).powi(2)).sum::<f32>().sqrt();
                    if drive == 0 && e.template.starts_with(&template) && !e.pieces.is_empty() && d < radius {
                        chosen.push(b);
                        props.queue_break(b);
                    }
                }
                eprintln!(
                    "pile centre={centre} template={} breaks={chosen:?}",
                    props.catalog.breakables[centre].template
                );
                for p in &props.catalog.breakables[centre].pieces {
                    eprintln!("pile piece {:?} shell={} mass={}", p.primitive, p.shell, p.mass);
                }
                if drive > 0 {
                    let start: [f32; 3] = std::array::from_fn(|k| at[12 + k] + 60. * at[8 + k]);
                    multi.place_at_rest(0, start, [-at[8], -at[9], -at[10]]).unwrap();
                }
                let mut go = [Controls::default(); 2];
                go[0].throttle = 1.;
                let mut stop = [Controls::default(); 2];
                stop[0].brake = if drive > 0 { 1. } else { 0. };
                let cars = multi.cars.len();
                let ticks = env("GRID_PILE_TICKS", 3600);
                let dump = env("GRID_PILE_DUMP", 600);
                let (mut window, mut total) = (std::time::Duration::ZERO, std::time::Duration::ZERO);
                let mut reached = [None; 3];
                for tick in 0..ticks {
                    let t = std::time::Instant::now();
                    multi.tick(if tick < drive { &go } else { &stop }).unwrap();
                    let e = t.elapsed();
                    window += e;
                    total += e;
                    let pieces = multi.bodies().len() - cars;
                    let asleep = multi.bodies()[cars..]
                        .iter()
                        .filter(|b| b[0x400 / 4] == contact_groups::ASLEEP)
                        .count();
                    for (k, share) in [0.5f32, 0.9, 1.].iter().enumerate() {
                        if reached[k].is_none() && pieces > 0 && asleep as f32 >= share * pieces as f32 {
                            reached[k] = Some(tick + 1);
                        }
                    }
                    if (tick + 1) % 60 == 0 {
                        let spin = multi.bodies()[cars..]
                            .iter()
                            .filter(|b| b[0x400 / 4] != contact_groups::ASLEEP)
                            .map(|b| (0..3).map(|k| f(b, 0x210 + 4 * k).powi(2)).sum::<f32>().sqrt())
                            .fold(0f32, f32::max);
                        eprintln!(
                            "pile t={} pieces={pieces} asleep={asleep} ms={:.3} max_w={spin:.3} moving={} used={} pool={}",
                            (tick + 1) / 60,
                            window.as_secs_f64() * 1e3 / 60.,
                            multi.bodies()[cars..].iter().filter(|b| b[0x400 / 4] != contact_groups::ASLEEP && ((0..3).map(|k| f(b, 0x210 + 4 * k).powi(2)).sum::<f32>() >= 0.09 || (0..3).map(|k| f(b, 0x220 + 4 * k).powi(2)).sum::<f32>() >= 0.09)).count(),
                            multi.world.groups.contacts.iter().filter(|c| **c != contact_groups::EMPTY_CONTACT).count(),
                            multi.world.groups.contacts.len(),
                        );
                        window = std::time::Duration::ZERO;
                    }
                    // GRID_PILE_TRACE=body: that body every tick for 60 ticks after the dump.
                    if let Some(t) = std::env::var("GRID_PILE_TRACE").ok().and_then(|v| v.parse::<usize>().ok())
                        && tick + 1 >= dump
                        && tick + 1 < dump + 60
                    {
                            let b = &multi.bodies()[t];
                            let c0: [f32; 3] = std::array::from_fn(|k| f(b, 0x1c0 + 4 * k));
                            let mut line = format!(
                                "trace {} w={:.3},{:.3},{:.3} v={:.3},{:.3},{:.3} y={:.4} up={:.3},{:.3},{:.3} asleep={}",
                                tick + 1,
                                f(b, 0x210), f(b, 0x214), f(b, 0x218),
                                f(b, 0x220), f(b, 0x224), f(b, 0x228),
                                c0[1], f(b, 0x1e0), f(b, 0x1e4), f(b, 0x1e8),
                                b[0x400 / 4] == contact_groups::ASLEEP
                            );
                            for c in &multi.world.groups.contacts {
                                let (a, o) = (c[1] as usize, c[2] as usize);
                                if a != t + 1 && o != t + 1 {
                                    continue;
                                }
                                let g = |off: usize| f32::from_bits(c[off / 4]);
                                line += &format!(
                                    " | {}-{} r={:.2},{:.2} pen={:.4} f={:.2}",
                                    a as i64 - 1, o as i64 - 1,
                                    g(0x50) - c0[0], g(0x58) - c0[2], g(0x38), g(0x3c)
                                );
                            }
                            eprintln!("{line}");
                    }
                    if tick + 1 == dump {
                        let bodies = multi.bodies();
                        for i in cars..bodies.len() {
                            let b = &bodies[i];
                            if b[0x400 / 4] == contact_groups::ASLEEP {
                                if let Some(p) = multi.props.as_ref().and_then(|p| p.pieces.iter().find(|q| q.body == i).map(|q| (q, p))) {
                                    eprintln!("asleep {i} pos={:.3},{:.3},{:.3} {:?}", f(b, 0x1c0), f(b, 0x1c4), f(b, 0x1c8), p.1.catalog.breakables[p.0.breakable].pieces[p.0.spec].primitive);
                                }
                                continue;
                            }
                            let w: f32 = (0..3).map(|k| f(b, 0x210 + 4 * k).powi(2)).sum::<f32>().sqrt();
                            let v: f32 = (0..3).map(|k| f(b, 0x220 + 4 * k).powi(2)).sum::<f32>().sqrt();
                            let axis = [f(b, 0x1d0 + 16), f(b, 0x1d0 + 20), f(b, 0x1d0 + 24)];
                            eprintln!(
                                "body {i} w={w:.3} v={v:.3} timer={:.2} up={:.2},{:.2},{:.2} wvec={:.2},{:.2},{:.2} pos={:.3},{:.3},{:.3}",
                                f(b, 0x3a0),
                                axis[0],
                                axis[1],
                                axis[2],
                                f(b, 0x210),
                                f(b, 0x214),
                                f(b, 0x218),
                                f(b, 0x1c0),
                                f(b, 0x1c4),
                                f(b, 0x1c8)
                            );
                            if let Some(p) = multi.props.as_ref().and_then(|p| p.pieces.iter().find(|q| q.body == i).map(|q| (q, p))) {
                                eprintln!("  piece {:?} breakable {}", p.1.catalog.breakables[p.0.breakable].pieces[p.0.spec].primitive, p.0.breakable);
                            }
                            for c in &multi.world.groups.contacts {
                                let (a, o) = (c[1] as usize, c[2] as usize);
                                if a != i + 1 && o != i + 1 {
                                    continue;
                                }
                                let g = |off: usize| f32::from_bits(c[off / 4]);
                                eprintln!(
                                    "  contact a={} b={} n={:.3},{:.3},{:.3} pen={:.4} p={:.2},{:.2},{:.2} f={:.2} ca={:.3},{:.3},{:.3}",
                                    a as i64 - 1,
                                    o as i64 - 1,
                                    g(0x60),
                                    g(0x64),
                                    g(0x68),
                                    g(0x38),
                                    g(0x50),
                                    g(0x54),
                                    g(0x58),
                                    g(0x3c),
                                    f(&bodies[a - 1], 0x1c0),
                                    f(&bodies[a - 1], 0x1c4),
                                    f(&bodies[a - 1], 0x1c8)
                                );
                            }
                        }
                    }
                }
                eprintln!(
                    "pile settle half={:?} ninety={:?} all={:?} mean_ms={:.3}",
                    reached[0],
                    reached[1],
                    reached[2],
                    total.as_secs_f64() * 1e3 / ticks as f64
                );
                // The default pile at rest sleeps whole within 15 s (4.9 s at F549;
                // before, with the prism face-axis pairs, 31 of 330 after 60 s).
                if drive == 0 && std::env::var_os("GRID_PILE_CENTRE").is_none() {
                    assert!(reached[2].is_some_and(|t| t <= 900), "pile awake: {reached:?}");
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    /// Sleeping cost, paired in one process: the Nürburgring drift circuit
    /// with its tyre stacks and armco broken at rest (the owner's phone
    /// field of 21 October, about 220 pieces), settled, then cloned; one
    /// copy keeps sleeping, the other has every piece forced awake with
    /// turnOffTime FLT_MAX. The copies tick alternately so the PC's load
    /// falls on both; prints the mean tick time of each.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS"]
    fn sleeping_cost_on_a_broken_nurburgring_field() {
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
                let route = Route::new("tracks/europe/nurburgring/route_1").unwrap();
                let mut multi =
                    MultiRuntime::lacetti_on(root, &route, &["slot_00", "slot_01"], &table, events)
                        .unwrap();
                multi.enable_dynamic_props(root).unwrap();
                // GRID_SLEEP_AB_PARK=off: the pile as if near the player.
                multi.parking = std::env::var("GRID_SLEEP_AB_PARK").as_deref() != Ok("off");
                let props = multi.props.as_mut().unwrap();
                let limit: usize = std::env::var("GRID_SLEEP_AB_PIECES")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(250);
                // GRID_SLEEP_AB_KINDS: template prefixes (an armco break takes
                // its linked run, hundreds of pieces).
                let kinds = std::env::var("GRID_SLEEP_AB_KINDS")
                    .unwrap_or_else(|_| "tyrestack,armco".into());
                let mut pieces = 0;
                for b in 0..props.catalog.breakables.len() {
                    let entry = &props.catalog.breakables[b];
                    if pieces < limit
                        && kinds.split(',').any(|k| entry.template.starts_with(k))
                        && !entry.pieces.is_empty()
                    {
                        pieces += entry.pieces.len();
                        props.queue_break(b);
                    }
                }
                let stop = [Controls::default(); 2];
                let settle: usize = std::env::var("GRID_SLEEP_AB_SETTLE")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(300);
                let cars = multi.cars.len();
                for tick in 0..settle {
                    multi.tick(&stop).unwrap();
                    if tick % 300 == 299 {
                        let n = multi.bodies()[cars..]
                            .iter()
                            .filter(|b| b[0x400 / 4] == contact_groups::ASLEEP)
                            .count();
                        eprintln!("sleep_ab settle tick={} asleep={n}", tick + 1);
                    }
                }
                let asleep = |multi: &MultiRuntime| {
                    multi.bodies()[cars..]
                        .iter()
                        .filter(|b| b[0x400 / 4] == contact_groups::ASLEEP)
                        .count()
                };
                let mut awake = multi.clone();
                awake.parking = false;
                for i in cars..awake.world.groups.bodies.len() {
                    if grouped_physical_frame::asleep(&awake.world, i) {
                        grouped_physical_frame::wake(&mut awake.world, i);
                    }
                    awake.world.groups.bodies[i][0x3b4 / 4] = f32::MAX.to_bits();
                }
                assert_eq!(asleep(&awake), 0);
                let ticks: usize = std::env::var("GRID_SLEEP_AB_TICKS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(600);
                let (mut slept, mut kept) = (std::time::Duration::ZERO, std::time::Duration::ZERO);
                let mut sleeping = 0;
                for _ in 0..ticks {
                    let t = std::time::Instant::now();
                    multi.tick(&stop).unwrap();
                    slept += t.elapsed();
                    // GRID_SLEEP_AB_ONLY=sleeping: profile the sleeping copy alone.
                    if std::env::var_os("GRID_SLEEP_AB_ONLY").is_none() {
                        let t = std::time::Instant::now();
                        awake.tick(&stop).unwrap();
                        kept += t.elapsed();
                    }
                    sleeping += asleep(&multi);
                }
                let ms = |d: std::time::Duration| d.as_secs_f64() * 1e3 / ticks as f64;
                eprintln!(
                    "sleep_ab bodies={} pieces={} asleep_mean={:.1} sleeping_ms={:.3} awake_ms={:.3} ratio={:.3}",
                    multi.bodies().len(),
                    multi.bodies().len() - cars,
                    sleeping as f64 / ticks as f64,
                    ms(slept),
                    ms(kept),
                    ms(kept) / ms(slept),
                );
                // World hashes: an optimisation must leave both unchanged.
                let hash = |m: &MultiRuntime| {
                    m.bodies()
                        .iter()
                        .flatten()
                        .chain(m.world.groups.contacts.iter().flatten())
                        .fold(0xcbf29ce484222325u64, |h, &w| {
                            (h ^ u64::from(w)).wrapping_mul(0x100000001b3)
                        })
                };
                eprintln!(
                    "sleep_ab hash sleeping={:016x} awake={:016x}",
                    hash(&multi),
                    hash(&awake)
                );
                assert!(asleep(&multi) * 2 > multi.bodies().len() - cars);
            })
            .unwrap()
            .join()
            .unwrap();
    }

    /// No piece sleeps or parks without support (owner's report of 23
    /// October: tyres "fall asleep in the air"). The player breaks a tyre
    /// wall at full throttle and, three ticks later while the pieces fly,
    /// is put back 80 m away, as if it had driven on: every piece is beyond
    /// the 50 m parking radius. After ten seconds a clone with every piece
    /// woken is the reference; no asleep or parked piece may stay put where
    /// it falls in the clone. With B7B850's range-only park (before F561) 13
    /// parked pieces hung here, up to 3.5 m above where they fall.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS"]
    fn no_piece_sleeps_or_parks_in_the_air_after_the_player_drives_on() {
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
                let mut total = 0;
                for (route, wall) in [
                    ("tracks/europe/nurburgring/route_1", 1237),
                    ("tracks/europe/nurburgring/route_1", 2276),
                    ("tracks/europe/donington/route_0", 167),
                ] {
                    let route = Route::new(route).unwrap();
                    let go = [Controls { throttle: 1., ..Controls::default() }, Controls::default()];
                    let stop = [Controls { brake: 1., ..Controls::default() }, Controls::default()];
                    let mut found = None;
                    for side in [1f32, -1.] {
                        let mut m = MultiRuntime::lacetti_on(
                            root,
                            &route,
                            &["slot_00", "slot_01"],
                            &table,
                            events,
                        )
                        .unwrap();
                        m.enable_dynamic_props(root).unwrap();
                        let at = m.props.as_ref().unwrap().catalog.breakables[wall].placement;
                        let z = [at[8], at[9], at[10]].map(|v| v * side);
                        let start: [f32; 3] = std::array::from_fn(|k| at[12 + k] + 80. * z[k]);
                        m.place_at_rest(0, start, z.map(|v| -v)).unwrap();
                        let mut broke = false;
                        for _ in 0..450 {
                            m.tick(&go).unwrap();
                            if m.bodies().len() > m.cars.len() {
                                broke = true;
                                break;
                            }
                        }
                        if broke {
                            found = Some((m, start, z.map(|v| -v)));
                            break;
                        }
                    }
                    let (mut multi, start, heading) = found.expect("the car breaks the wall");
                    for _ in 0..3 {
                        multi.tick(&go).unwrap();
                    }
                    multi.place_at_rest(0, start, heading).unwrap();
                    for _ in 0..600 {
                        multi.tick(&stop).unwrap();
                    }
                    let cars = multi.cars.len();
                    let n = multi.bodies().len();
                    let count = |f: &dyn Fn(&contact_groups::Body) -> bool| {
                        multi.bodies()[cars..].iter().filter(|b| f(b)).count()
                    };
                    let asleep = count(&|b| b[0x400 / 4] == contact_groups::ASLEEP);
                    let parked = count(&|b| (b[0x410 / 4] >> 24) & 255 != 0);
                    let hanging = multi.unsupported(120).unwrap();
                    eprintln!(
                        "wall {wall} pieces={} asleep={asleep} parked={parked} hanging={} {:?}",
                        n - cars,
                        hanging.len(),
                        &hanging[..hanging.len().min(10)]
                    );
                    total += hanging.len();
                }
                assert_eq!(total, 0, "pieces asleep or parked in the air");
            })
            .unwrap()
            .join()
            .unwrap();
    }

    /// The player drives into a tyre wall at full throttle (the owner's phone
    /// reports of 22 October, Nürburgring tyrestack_white 1237 and Donington
    /// tyre_wall 167); after the pile has settled, a clone with every piece
    /// woken is the reference: a piece that falls there but stays put here
    /// was hanging, asleep or parked (F561: islands park only at rest).
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS"]
    #[allow(clippy::needless_range_loop)]
    fn a_car_driven_through_a_tyre_wall_leaves_no_piece_hanging() {
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
                for (route, wall) in [
                    ("tracks/europe/nurburgring/route_1", 1237),
                    ("tracks/europe/donington/route_0", 167),
                ] {
                    let route = Route::new(route).unwrap();
                    let mut drive = [Controls::default(); 2];
                    let (mut multi, mut fastest, mut car_speed) = (None, 0f32, 0f32);
                    // Approach along the wall's normal, from either side.
                    for side in [1f32, -1.] {
                        let mut m = MultiRuntime::lacetti_on(
                            root,
                            &route,
                            &["slot_00", "slot_01"],
                            &table,
                            events,
                        )
                        .unwrap();
                        m.enable_dynamic_props(root).unwrap();
                        let at = m.props.as_ref().unwrap().catalog.breakables[wall].placement;
                        let centre = [at[12], at[13], at[14]];
                        let z = [at[8], at[9], at[10]].map(|v| v * side);
                        let start: [f32; 3] = std::array::from_fn(|k| centre[k] + 80. * z[k]);
                        m.place_at_rest(0, start, z.map(|v| -v)).unwrap();
                        drive[0] = Controls { throttle: 1., ..Controls::default() };
                        let mut after = None;
                        for tick in 0..450 {
                            if after.is_some_and(|t| tick > t + 60) {
                                break;
                            }
                            let v = |m: &MultiRuntime, i: usize| {
                                let b = &m.bodies()[i];
                                (0..3)
                                    .map(|k| f32::from_bits(b[0x220 / 4 + k]).powi(2))
                                    .sum::<f32>()
                                    .sqrt()
                            };
                            let cars = m.cars.len();
                            if m.bodies().len() == cars {
                                car_speed = v(&m, 0);
                            }
                            m.tick(&drive).unwrap();
                            for i in cars..m.bodies().len() {
                                fastest = fastest.max(v(&m, i));
                            }
                            if m.bodies().len() > cars && after.is_none() {
                                after = Some(tick);
                            }
                        }
                        if m.bodies().len() > m.cars.len() {
                            multi = Some(m);
                            break;
                        }
                    }
                    let mut multi = multi.expect("the car breaks the wall from one side");
                    eprintln!("wall {wall} car {car_speed:.1} m/s at the break, fastest piece {fastest:.1} m/s");
                    drive[0] = Controls { brake: 1., ..Controls::default() };
                    for _ in 0..600 {
                        multi.tick(&drive).unwrap();
                    }
                    let cars = multi.cars.len();
                    let height = |m: &MultiRuntime, i: usize| f32::from_bits(m.bodies()[i][0x204 / 4]);
                    let n = multi.bodies().len();
                    let state = |m: &MultiRuntime, i: usize| {
                        let b = &m.bodies()[i];
                        (b[0x400 / 4] == contact_groups::ASLEEP, (b[0x410 / 4] >> 24) & 255 != 0)
                    };
                    let car = &multi.bodies()[0];
                    let f = |o: usize| f32::from_bits(car[o / 4]);
                    eprintln!(
                        "car0 +1C0 {:.1},{:.1},{:.1} +200 {:.1},{:.1},{:.1} piece +1C0 {:.1},{:.1},{:.1}",
                        f(0x1c0), f(0x1c4), f(0x1c8), f(0x200), f(0x204), f(0x208),
                        f32::from_bits(multi.bodies()[cars][0x1c0 / 4]),
                        f32::from_bits(multi.bodies()[cars][0x1c4 / 4]),
                        f32::from_bits(multi.bodies()[cars][0x1c8 / 4])
                    );
                    let asleep = (cars..n).filter(|&i| state(&multi, i).0).count();
                    let parked = (cars..n).filter(|&i| state(&multi, i).1).count();
                    let before: Vec<f32> = (0..n).map(|i| height(&multi, i)).collect();
                    let start: Vec<[f32; 3]> = (0..n)
                        .map(|i| [0x1c0, 0x1c4, 0x1c8].map(|o| f32::from_bits(multi.bodies()[i][o / 4])))
                        .collect();
                    let mut woken = multi.clone();
                    woken.parking = false;
                    for i in cars..n {
                        if grouped_physical_frame::asleep(&woken.world, i) {
                            grouped_physical_frame::wake(&mut woken.world, i);
                        }
                    }
                    for _ in 0..120 {
                        multi.tick(&drive).unwrap();
                        woken.tick(&drive).unwrap();
                    }
                    // Awake pieces after the settle: speed, spin and how far they moved.
                    let pos = |m: &MultiRuntime, i: usize| {
                        let b = &m.bodies()[i];
                        [0x1c0, 0x1c4, 0x1c8].map(|o| f32::from_bits(b[o / 4]))
                    };
                    let mut rows = Vec::new();
                    for i in cars..n {
                        if state(&multi, i).0 {
                            continue;
                        }
                        let b = &multi.bodies()[i];
                        let len = |o: usize| {
                            (0..3).map(|k| f32::from_bits(b[o / 4 + k]).powi(2)).sum::<f32>().sqrt()
                        };
                        let (p0, p1) = (start[i], pos(&multi, i));
                        let moved = (0..3).map(|k| (p0[k] - p1[k]).powi(2)).sum::<f32>().sqrt();
                        rows.push(format!(
                            "{i}:v{:.2},w{:.2},t{:.2},h{:.2},moved{:.2}",
                            len(0x220),
                            len(0x210),
                            f32::from_bits(b[0x3a0 / 4]),
                            p1[1],
                            moved
                        ));
                    }
                    eprintln!("awake after settle: {}", rows.join(" "));
                    let hanging: Vec<(usize, bool, bool, f32)> = (cars..n)
                        .filter(|&i| {
                            (height(&multi, i) - before[i]).abs() < 1e-3
                                && height(&woken, i) < before[i] - 0.2
                        })
                        .map(|i| {
                            let (a, p) = state(&multi, i);
                            (i, a, p, before[i] - height(&woken, i))
                        })
                        .collect();
                    eprintln!(
                        "wall {wall} pieces={} asleep={asleep} parked={parked} hanging={} {:?}",
                        n - cars,
                        hanging.len(),
                        &hanging[..hanging.len().min(10)]
                    );
                    assert!(hanging.is_empty(), "wall {wall}: {} pieces hanging", hanging.len());
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    /// A piece asleep on an intact stack must not stay in the air when that
    /// stack breaks: Nürburgring tyre stacks, every other stack of each
    /// cluster broken at rest and settled (pieces fall onto their intact
    /// neighbours and sleep), then the neighbours break. A copy with every
    /// piece woken right after that break is the reference: a piece that
    /// falls there but stays put asleep here was left hanging.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS"]
    fn pieces_asleep_on_a_stack_fall_when_the_stack_breaks() {
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
                let route = Route::new("tracks/europe/nurburgring/route_1").unwrap();
                let mut multi =
                    MultiRuntime::lacetti_on(root, &route, &["slot_00", "slot_01"], &table, events)
                        .unwrap();
                multi.enable_dynamic_props(root).unwrap();
                multi.parking = false;
                let props = multi.props.as_mut().unwrap();
                let stacks: Vec<(usize, [f32; 3])> = (0..props.catalog.breakables.len())
                    .filter(|&b| {
                        let e = &props.catalog.breakables[b];
                        e.template.starts_with("tyrestack") && !e.pieces.is_empty()
                    })
                    .map(|b| {
                        let p = props.catalog.breakables[b].placement;
                        (b, [p[12], p[13], p[14]])
                    })
                    .collect();
                // One region (the body budget holds about 1,000 pieces).
                let region = stacks[stacks.len() / 2].1;
                let stacks: Vec<(usize, [f32; 3])> = stacks
                    .into_iter()
                    .filter(|&(_, p)| (0..3).map(|k| (p[k] - region[k]).powi(2)).sum::<f32>() < 80. * 80.)
                    .collect();
                let near = |a: [f32; 3], b: [f32; 3]| {
                    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>() < 2.5 * 2.5
                };
                // First wave: no two neighbours; second: their neighbours.
                let mut first: Vec<(usize, [f32; 3])> = Vec::new();
                for &(b, p) in &stacks {
                    if !first.iter().any(|&(_, q)| near(p, q)) {
                        first.push((b, p));
                    }
                }
                let second: Vec<usize> = stacks
                    .iter()
                    .filter(|&&(b, p)| {
                        !first.iter().any(|&(f, _)| f == b) && first.iter().any(|&(_, q)| near(p, q))
                    })
                    .map(|&(b, _)| b)
                    .collect();
                for &(b, _) in &first {
                    props.queue_break(b);
                }
                let stop = [Controls::default(); 2];
                for _ in 0..3000 {
                    multi.tick(&stop).unwrap();
                }
                let cars = multi.cars.len();
                let height = |m: &MultiRuntime, i: usize| f32::from_bits(m.bodies()[i][0x204 / 4]);
                let settled = multi.bodies().len();
                let asleep_before: Vec<usize> = (cars..settled)
                    .filter(|&i| multi.bodies()[i][0x400 / 4] == contact_groups::ASLEEP)
                    .collect();
                let before: Vec<f32> = (0..settled).map(|i| height(&multi, i)).collect();
                for &b in &second {
                    multi.props.as_mut().unwrap().queue_break(b);
                }
                let mut woken = multi.clone();
                // Both copies break the second wave on this tick.
                multi.tick(&stop).unwrap();
                woken.tick(&stop).unwrap();
                for i in cars..woken.world.groups.bodies.len() {
                    if grouped_physical_frame::asleep(&woken.world, i) {
                        grouped_physical_frame::wake(&mut woken.world, i);
                    }
                }
                for _ in 0..300 {
                    multi.tick(&stop).unwrap();
                    woken.tick(&stop).unwrap();
                }
                let hanging: Vec<(usize, f32)> = asleep_before
                    .iter()
                    .filter(|&&i| {
                        (height(&multi, i) - before[i]).abs() < 1e-4
                            && height(&woken, i) < before[i] - 0.2
                    })
                    .map(|&i| (i, before[i] - height(&woken, i)))
                    .collect();
                eprintln!(
                    "stacks first={} second={} pieces={} asleep_before={} hanging={} {:?}",
                    first.len(),
                    second.len(),
                    settled - cars,
                    asleep_before.len(),
                    hanging.len(),
                    &hanging[..hanging.len().min(12)]
                );
                assert!(hanging.is_empty(), "{} pieces left hanging", hanging.len());
            })
            .unwrap()
            .join()
            .unwrap();
    }

    /// Sleeping (B0FE60, B24D70, B24E70): the pieces of a broken tyre wall
    /// that come to rest leave the active list after turnOffTime (1 s); the
    /// cars never sleep; a car driving into the pile wakes pieces again.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS / GRID_BODY_EVENT_PARAMETERS"]
    fn pieces_at_rest_sleep_and_a_car_wakes_them() {
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
                let route = Route::new("tracks/europe/le_mans/route_0").unwrap();
                let mut multi =
                    MultiRuntime::lacetti_on(root, &route, &["slot_00", "slot_01"], &table, events)
                        .unwrap();
                multi.enable_dynamic_props(root).unwrap();
                let at = multi.props.as_ref().unwrap().catalog.breakables[969].placement;
                let (x, z) = ([at[0], at[1], at[2]], [at[8], at[9], at[10]]);
                let centre = [at[12], at[13], at[14]];
                let start: [f32; 3] = std::array::from_fn(|k| centre[k] + 40. * z[k]);
                multi.place_at_rest(0, start, z.map(|v| -v)).unwrap();
                let mut drive = [Controls::default(); 2];
                drive[0].throttle = 1.;
                for _ in 0..240 {
                    multi.tick(&drive).unwrap();
                }
                let asleep = |multi: &MultiRuntime| {
                    multi.bodies()[multi.cars.len()..]
                        .iter()
                        .filter(|b| b[0x400 / 4] == contact_groups::ASLEEP)
                        .count()
                };
                let pieces = multi.bodies().len() - multi.cars.len();
                assert!(pieces > 10, "the wall broke into {pieces} pieces");
                // Back away and stop; the pile settles.
                multi.place_at_rest(0, start, z.map(|v| -v)).unwrap();
                let stop = [Controls::default(); 2];
                for _ in 0..600 {
                    multi.tick(&stop).unwrap();
                }
                let slept = asleep(&multi);
                eprintln!("pieces {pieces} asleep {slept}");
                assert!(slept * 2 > pieces, "{slept} of {pieces} pieces asleep");
                // The asleep pieces' road depths now come from the memo; they
                // equal fresh casts bit for bit.
                let props = multi.props.as_ref().unwrap();
                let bits = |p: &crate::dynamic_props::PropWorld| {
                    p.road_depths(multi.cars[0].track(), multi.bodies())
                        .iter()
                        .map(|d| {
                            (
                                d.body,
                                d.depth.to_bits(),
                                d.ground.to_bits(),
                                d.centre.map(f32::to_bits),
                                d.asleep,
                            )
                        })
                        .collect::<Vec<_>>()
                };
                let memo = bits(props);
                assert_eq!(memo, bits(&props.without_depth_memo()));
                assert!(memo.iter().any(|d| d.4));
                assert!(
                    multi.bodies()[..multi.cars.len()]
                        .iter()
                        .all(|b| b[0x400 / 4] != contact_groups::ASLEEP)
                );
                // The second car (the first is wrecked) drives at an asleep
                // piece from 20 m away.
                let n = multi.cars.len();
                // The asleep piece furthest out on the cars' side of the wall.
                let along = |p: [f32; 3]| (0..3).map(|k| (p[k] - centre[k]) * z[k]).sum::<f32>();
                let target = (multi.bodies()[n..].iter())
                    .filter(|b| b[0x400 / 4] == contact_groups::ASLEEP)
                    .map(|b| std::array::from_fn::<f32, 3, _>(|k| f32::from_bits(b[(0x1d0 + 48) / 4 + k])))
                    .max_by(|a, b| along(*a).total_cmp(&along(*b)))
                    .unwrap();
                eprintln!("target {:.1} m out", along(target));
                // On the first car's approach line, which it cleared (a start
                // beside the wall can catch on its remains).
                let side: [f32; 3] = std::array::from_fn(|k| centre[k] + (along(target) + 15.) * z[k]);

                let away: [f32; 3] = std::array::from_fn(|k| centre[k] + 40. * z[k] + 6. * x[k]);
                multi.place_at_rest(0, away, z.map(|v| -v)).unwrap();
                multi.place_at_rest(1, side, z.map(|v| -v)).unwrap();
                let mut drive = [Controls::default(); 2];
                drive[1].throttle = 1.;
                let mut fewest = slept;
                for _ in 0..240 {
                    multi.tick(&drive).unwrap();
                    fewest = fewest.min(asleep(&multi));
                }
                eprintln!("fewest asleep while driving through {fewest}");
                assert!(fewest < slept, "no piece woke");
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
                        multi.cars[0].set_ai_packet(driver.reverse_request());
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
                    for (car, d) in multi.cars.iter_mut().zip(drivers.iter()) {
                        car.set_ai_packet(d.reverse_request());
                    }
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
                    for (car, d) in multi.cars.iter_mut().zip(drivers.iter()) {
                        car.set_ai_packet(d.reverse_request());
                    }
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
