//! Source-only serial driving prototype of a purchased car (Lacetti by default)
//! on a purchased route (Donington by default). No saved native state.
//! The manager's single shared physical material and serial scheduling are explicit
//! prototype choices; body events are opt-in, full factory/deformation/racing external.
use crate::{
    car::Car, contact_surfaces::Surfaces, route::Route, source_physical, standard_car,
    track::Track,
};
use grid_formats::{
    binxml, car_collision, grid, handling_damage, jpk, pssg, tuning, vehicle, vehicle_properties,
};
use grid_world::{
    Error, grouped_physical_frame, impact_frame, rectangle_cull, spawn, suspension_prepare,
    vehicle_bridge, vehicle_force, vehicle_manager, vehicle_publication, vehicle_world,
};
use std::path::Path;

pub mod assists;
pub mod multi;
pub mod player_reset;
#[cfg(test)]
mod wheel_detach;

pub const DELTA: f32 = 0.016;
pub const GRAVITY: f32 = f32::from_bits(0x411c_f5c3);

type GeometryProvider<'a> = dyn FnMut(
        &mut [grid_world::contact_groups::Body],
        usize,
    ) -> Result<Vec<grid_world::contact_groups::Contact>, Error>
    + 'a;

enum Geometry<'a> {
    Callback(&'a mut GeometryProvider<'a>),
    Source(&'a crate::body_geometry::BodyGeometry),
    Deforming(&'a mut crate::body_geometry::BodyGeometry),
}

/// Cloning copies the live state; the start state for [`Runtime::reset`] and
/// the track geometry are shared (flashback checkpoints, docs/FLASHBACK.md).
#[derive(Clone)]
pub struct Runtime {
    pub state: vehicle_world::State,
    initial: std::sync::Arc<vehicle_world::State>,
    initial_cache: rectangle_cull::Cache,
    track: Track,
    surfaces: std::sync::Arc<Surfaces>,
    cache: rectangle_cull::Cache,
    pair: [usize; 2],
    pub ticks: u64,
    /// Fresh body records from the last successful tick; diagnostics only.
    /// Failure preserves this count and reset clears it.
    pub last_body_contacts: usize,
    /// Occupied contact records of the last successful tick; presentation only.
    pub last_active_contacts: usize,
    /// Opt-in owned static-peer event route; parameters are private source data.
    pub body_events: Option<grid_world::body_impact::Parameters>,
    /// Explicit selected source deformation ownership, never inferred from raw flags.
    pub deformation_enabled: bool,
    pub deformation_registration: Option<grid_world::deformation_world::Registration>,
    initial_deformation_registration: Option<std::sync::Arc<grid_world::deformation_world::Registration>>,
    lap_course: Option<std::sync::Arc<grid_world::lap_timing::Course>>,
    pub lap: Option<grid_world::lap_timing::State>,
    initial_lap: Option<grid_world::lap_timing::State>,
    pub lap_events: Vec<grid_world::lap_timing::Event>,
    /// Purchased route whose track, grid, boundary, props and gates are used.
    route: Route,
    /// Purchased car model whose tuning, description and deformation are used.
    car: Car,
    /// Player driver aids (see [`assists`]); the default is the earlier
    /// fixed configuration. Change with [`Runtime::set_assists`].
    pub assists: assists::Assists,
    /// Pending one-tick gear request and the tick it belongs to.
    shift: Option<(u64, assists::Shift)>,
    /// Opt-in breakable route props (`enable_dynamic_props`).
    props: Option<crate::dynamic_props::PropWorld>,
    /// `collision_main` bounds of the car's `.ned` (description extents).
    hull: car_collision::Bounds,
    /// The car's NED loose-part levels; the wheel rule changes driving.
    loose_parts: crate::loose_parts::Config,
    /// Tyre and wheel node bounds of the car model, read with the body
    /// geometry; a lost wheel becomes a body of these extents.
    wheel_nodes: Option<std::sync::Arc<crate::loose_wheel::WheelNodes>>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Controls {
    pub throttle: f32,
    pub brake: f32,
    /// Original signed command: positive turns toward local positive X (left).
    pub steering: f32,
    pub handbrake: bool,
}

/// Read-only presentation inputs in the same rear-left/right, front-left/right
/// order as source wheel initialization. These never feed back into physics.
#[derive(Clone, Copy, Debug)]
pub struct WheelPose {
    pub anchor: [f32; 3],
    pub axis: [f32; 3],
    pub displacement: f32,
    pub steering: [f32; 2],
    pub phase: f32,
    pub deformation_translation: [f32; 3],
    pub deformation_orientation: [[f32; 4]; 4],
}

fn read(root: &Path, relative: &str, bound: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(root.join(relative)).map_err(|e| format!("{relative}: {e}"))?
        .take(bound + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > bound {
        return Err("source resource exceeds bound".into());
    }
    Ok(bytes)
}
fn word(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn value(b: &[u8], o: usize) -> f32 {
    f32::from_bits(word(b, o))
}
fn byte(b: &mut [u32], o: usize, v: u8) {
    let shift = o % 4 * 8;
    b[o / 4] = (b[o / 4] & !(255 << shift)) | (u32::from(v) << shift);
}

/// Ground-placement wheels of 0x9DE4E0: anchor, radius (+E0) and the
/// current vertical displacement (+40).
fn placement_wheels(owner: &[u32]) -> [spawn::Wheel; 4] {
    std::array::from_fn(|i| {
        let w = (0x1990 + i * 0x150) / 4;
        spawn::Wheel {
            position: std::array::from_fn(|j| f32::from_bits(owner[w + j])),
            radius: f32::from_bits(owner[w + 0xe0 / 4]),
            vertical_displacement: f32::from_bits(owner[w + 0x40 / 4]),
        }
    })
}

/// Physical body words a re-placement writes (pose setter 0xA696F0 and the
/// velocity setters): published copy +40, derived +D0/+110, center +190,
/// published +1D0, angular +210 and linear velocity +220.
const PLACED_WORDS: [std::ops::Range<usize>; 3] = [
    0x40 / 4..0x80 / 4,
    0xd0 / 4..0x150 / 4,
    0x190 / 4..0x230 / 4,
];

/// Copy the words a re-placement wrote from `placed` into `body`.
pub(crate) fn apply_placement(
    body: &mut grid_world::contact_groups::Body,
    placed: &grid_world::contact_groups::Body,
) {
    for range in PLACED_WORDS {
        body[range.clone()].copy_from_slice(&placed[range]);
    }
}

/// A car re-placed at rest: its car words, physical body and query cache.
pub(crate) struct Placement {
    vehicle: Vec<u32>,
    physical: grid_world::contact_groups::Body,
    cache: rectangle_cull::Cache,
}

impl Runtime {
    /// Source terminal wheel state; ordinal order is rear L/R, front L/R.
    /// Status 3 disables driving controls on the following synchronization,
    /// while the physical simulation and reset remain active.
    /// The wheel of terminal damage: status 3 (9F2F80, wheel damage above
    /// terminalDamagePercentage) or 4 (9E65B0, a detached wheel).
    pub fn terminal_wheel(&self) -> Option<usize> {
        let b = &self.state.car.owner.vehicle;
        matches!(b[0xd234 / 4], 3 | 4).then_some((b[0xd238 / 4] & 255) as usize)
    }
    /// Whether the terminal damage is a lost wheel (status 4).
    pub fn lost_wheel(&self) -> bool {
        self.state.car.owner.vehicle[0xd234 / 4] == 4
    }

    pub fn wheels(&self) -> [WheelPose; 4] {
        let b = &self.state.car.owner.vehicle;
        let f = |o: usize| f32::from_bits(b[o / 4]);
        std::array::from_fn(|i| {
            let w = 0x1990 + i * 0x150;
            let c = 0x3d0 + i * 0x170;
            WheelPose {
                anchor: std::array::from_fn(|j| f(w + j * 4)),
                axis: std::array::from_fn(|j| f(c + 0x50 + j * 4)),
                displacement: f(w + 0x40).min(f(w + 0x4c)),
                steering: [f(w + 0x20), f(w + 0x24)],
                phase: f(w + 0x110),
                deformation_translation: std::array::from_fn(|a| {
                    if self.deformation_enabled {
                        f(0x1650 + i * 16 + a * 4)
                    } else {
                        0.
                    }
                }),
                deformation_orientation: std::array::from_fn(|a| {
                    std::array::from_fn(|j| {
                        if self.deformation_enabled {
                            f(0x1690 + i * 64 + a * 16 + j * 4)
                        } else if a == j {
                            1.
                        } else {
                            0.
                        }
                    })
                }),
            }
        })
    }

    /// Load purchased source files directly. The selected assisted automatic
    /// preferences/readiness and serial manager configuration are supplied here,
    /// rather than pretending to execute the original resource/preferences layer.
    pub fn lacetti(root: &Path, slot: &str, seed: u32) -> Result<Self, Box<dyn std::error::Error>> {
        Self::lacetti_on(root, &Route::default(), slot, seed)
    }

    /// As [`Self::lacetti`], on the grid slot of the given route.
    pub fn lacetti_on(
        root: &Path,
        route: &Route,
        slot: &str,
        seed: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::load(root, route, &Car::default(), slot, seed)
    }

    /// The given purchased car on the grid slot of the given route.
    pub fn load(
        root: &Path,
        route: &Route,
        model: &Car,
        slot: &str,
        seed: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        if !slot.starts_with("slot_")
            || slot.len() > 32
            || !slot.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            return Err("invalid grid slot".into());
        }
        let csv_bytes = read(root, &model.file(".csv"), 4 * 1024 * 1024)?;
        let csv = tuning::read(&csv_bytes)?;
        if csv.rows.len() != 1 {
            return Err("expected one car tuning row".into());
        }
        let properties = vehicle_properties::read(&csv.rows[0], 0x2008)?;
        let ned_bytes = read(root, &model.file(".ned"), 16 * 1024 * 1024)?;
        let ned = binxml::read(&ned_bytes)?;
        let geometry = vehicle::read_wheel_geometry(&ned)?;
        let loose_parts = crate::loose_parts::Config::from_ned(&ned)?;
        let widths_node = ned
            .elements
            .iter()
            .find(|e| e.name == "WHEELS")
            .ok_or("missing wheels")?;
        let mut widths = [0.; 4];
        for (slot, name) in widths.iter_mut().zip([
            "m_rear_l_width",
            "m_rear_r_width",
            "m_front_l_width",
            "m_front_r_width",
        ]) {
            *slot = widths_node
                .attributes
                .iter()
                .find(|(k, _)| *k == name)
                .ok_or("missing width")?
                .1
                .parse::<f64>()? as f32;
        }
        let archive_bytes = read(root, "raceload.jpk", 256 * 1024 * 1024)?;
        let archive = jpk::inspect(&archive_bytes)?;
        let hdm = archive
            .entries
            .iter()
            .find(|e| e.name == b"standard.hdm")
            .ok_or("missing Standard HDM")?;
        if hdm.uncompressed_size as usize != hdm.stored_data.len() {
            return Err("compressed HDM unsupported".into());
        }
        let settings =
            handling_damage::read_configuration(&archive_bytes[hdm.stored_data.clone()])?;
        let mut car = standard_car::initialize_with_outer_wheels(
            &properties,
            geometry,
            widths,
            GRAVITY,
            seed,
        )?;
        car.configure_suspension(geometry)?;
        car.configure_damage_regions(&ned)?;
        car.configure_handling(&settings, true)?;
        let mut owner = car.bytes().to_vec();
        owner.resize(source_physical::CAR_BYTES, 0);
        let factory = source_physical::initialize(
            &owner,
            std::array::from_fn(|i| value(properties.bytes(), 0x130 + 4 * i)),
            0,
            0,
        )?;
        let mut owner: Vec<u32> = factory.car[..0x21000]
            .as_chunks::<4>()
            .0
            .iter()
            .map(|w| u32::from_le_bytes(*w))
            .collect();
        // Explicit owned wrapper/shape identities, assisted automatic boot contract.
        owner[0xdb0 / 4] = 0;
        owner[0xdc8 / 4] = 1;
        owner[0x3d0 / 4] = 1f32.to_bits();
        // Observed outer A0E880 defaults preserved by the Standard constructor:
        // history factor and engine torque multiplier. Zero torque multiplier
        // leaves only idle/drag behavior despite an otherwise exact update.
        owner[0x2fc / 4] = 1f32.to_bits();
        owner[0x188c / 4] = 1f32.to_bits();
        // Running engine on the grid. The native no-input stationary reference
        // (F275 stationary-idle01) holds 232.6573..232.6873, the idle governor
        // target idle*1.01 that the reconstructed post-force stage maintains
        // once reached. Reset A0C9F0 zeroes RPM; a zero start only cranks after
        // throttle, delaying a standing start by about one second.
        let idle = f32::from_bits(owner[0x17a8 / 4]) * f32::from_bits(0x3f81_47ae);
        if !(idle.is_finite() && idle > 0.) {
            return Err("invalid source idle engine speed".into());
        }
        owner[0x1894 / 4] = idle.to_bits();
        owner[0x1f00 / 4] = idle.to_bits();
        // The runtime already owns all four wheel records by ordinal. The
        // context initializer's explicit relative owner links are not addresses
        // to retain or follow at this pointer-free contact boundary.
        for i in 0..4 {
            let o = (0x3d0 + i * 0x170 + 0x178) / 4;
            if owner[o] != (0x1990 + i * 0x150) as u32 {
                return Err("invalid initialized suspension wheel identity".into());
            }
            owner[o] = 0;
        }
        byte(&mut owner, 4, 0);
        byte(&mut owner, 0x1970, 1);
        byte(&mut owner, 0x1971, 1);
        // Explicit keyboard preference: brake selects reverse near rest; the
        // original automatic control helper consumes the two raw pedal slots.
        byte(&mut owner, 0x1972, 1);
        byte(&mut owner, 0x20fb4, 1);
        let mut physical: [u32; 0x424 / 4] = factory.physical.body[..0x424 / 4].try_into().unwrap();
        for o in [0x32c, 0x334, 0x338, 0x33c, 0x340] {
            physical[o / 4] = 0;
        }
        let track = Track::read(&read(root, &route.file("track.jpk"), 256 * 1024 * 1024)?)?;
        let surfaces = Surfaces::read(&read(root, "surface_materials.xml", 16 * 1024 * 1024)?)?;
        let grid_bytes = read(root, &route.file("grids.pssg"), 256 * 1024 * 1024)?;
        let markers = pssg::read_tree(&grid_bytes)?;
        let marker = grid::read_slot(&markers, slot.as_bytes(), route.subroute())?;
        let shape = car_collision::read(&ned)?;
        let front_extent = shape.bounds.max[2] + f32::from_bits(owner[0x1638 / 4]);
        let center = spawn::reference_front_aligned_center(marker.corners, front_extent, 0.)?;
        let heading = std::array::from_fn(|i| marker.corners[0][i] - marker.corners[2][i]);
        let mut cache = rectangle_cull::Cache::default();
        let pose = spawn::align_with_normalization(
            center,
            heading,
            placement_wheels(&owner),
            spawn::Normalization::ReferenceSse,
            |x, z, upper, lower| {
                Ok(track
                    .query([x, upper, z, 1.], [x, lower, z, 1.], &mut cache)?
                    .hit
                    .map(|h| upper - (upper - lower) * h.contact.fraction))
            },
        )?
        .matrix();
        owner[0x15e0 / 4..0x1620 / 4].copy_from_slice(&pose.map(f32::to_bits));
        let bridge = vehicle_bridge::calculate(
            &vehicle_bridge::State {
                physical,
                vehicle: suspension_prepare::State {
                    words: owner[..vehicle_force::BODY_WORDS].try_into().unwrap(),
                },
                shapes: Vec::new(),
            },
            &vehicle_bridge::Settings {
                notify: true,
                advance: None,
                correction: None,
                group_head: false,
            },
        )?
        .written;
        owner[..vehicle_force::BODY_WORDS].copy_from_slice(&bridge.vehicle.words);
        physical = bridge.physical;
        let pair =
            std::array::from_fn(|i| ((owner[(0x21cc + 4 * i) / 4] - 0x1990) / 0x150) as usize);
        // Solver settings captured live from the running original (Lacetti /
        // Donington race, world +788 block, WORLD_STEPPING_RESEARCH.md): globalCFM 0,
        // globalERP 0.2, minPenetrationDepth 0.003, quickStepW 1.0, 5 iterations;
        // first-order correction CFM 0, ERP 1.0, min penetration/constraint error
        // 0.02, W 1.0, 3 iterations. Indices 9/10 (angular scale/threshold) unchanged.
        let manager = grouped_physical_frame::Manager {
            parameters: [DELTA, 1., 1., 1., 1., 0., 0.2, 0.003, 1., 0.2, 0.2],
            gravity: [0., -GRAVITY, 0., 0.],
            correction: Some(impact_frame::Correction {
                iterations: 3,
                softness: 0.,
                scale: 1.,
                threshold: 0.02,
                relaxation: 1.,
            }),
            // Purchased surface_materials.xml row for the car body (CAR_). Every
            // barrier/track key it meets on Donington (CONC, PLAS, SIGN, SHTC,
            // ROCK, FENC, WALL, TARM) has the same solver values: restitution 0,
            // friction 0.4, cfm 0.01, correction velocity 1, no softness, dynamic
            // threshold 10, so the mixed pair equals this row. Formerly an authored
            // restitution-0.3 soft material.
            materials: vec![surfaces.physical(*b"CAR_")?.1],
            rows: vec![[0; 48]; 16],
            capacity: 16,
            iterations: 5,
            ends: [0; 4],
            properties: vec![(
                f32::from_bits(physical[0x3f4 / 4]),
                std::array::from_fn(|i| {
                    std::array::from_fn(|j| f32::from_bits(physical[0x150 / 4 + 4 * i + j]))
                }),
            )],
            workspace: vec![[[0.; 4]; 2]],
            cached_pair: None,
            cached_material: [0.; 13],
        };
        let state = vehicle_world::from_registered(
            vehicle_manager::State {
                frame: vehicle_force::Frame {
                    body: owner[..vehicle_force::BODY_WORDS].try_into().unwrap(),
                    view: [0; vehicle_force::VIEW_WORDS],
                    solver: [0; 884],
                    driven: car.driven,
                    material_multipliers: [None; 4],
                    diagnostics: [0.; 8],
                },
                owner: vehicle_publication::State {
                    damage_random_state: Some(car.random_state),
                    vehicle: owner,
                    physical,
                },
            },
            manager,
            track.shape_count(),
        )?;
        Ok(Self {
            initial: std::sync::Arc::new(state.clone()),
            initial_cache: cache,
            state,
            track,
            surfaces: std::sync::Arc::new(surfaces),
            cache,
            pair,
            ticks: 0,
            last_body_contacts: 0,
            last_active_contacts: 0,
            body_events: None,
            deformation_enabled: false,
            deformation_registration: None,
            initial_deformation_registration: None,
            lap_course: None,
            lap: None,
            initial_lap: None,
            lap_events: Vec::new(),
            route: route.clone(),
            car: model.clone(),
            assists: assists::Assists::default(),
            shift: None,
            props: None,
            hull: shape.bounds,
            loose_parts,
            wheel_nodes: None,
        })
    }

    /// Stage controls, queries and the complete world. A failed tick preserves
    /// all authoritative state, including the query cache and tick counter.
    pub fn tick(&mut self, controls: Controls) -> Result<(), Error> {
        self.tick_owned(controls, None)
    }
    pub fn enable_lap_timing(&mut self, root: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if self.ticks != 0 || self.lap_course.is_some() {
            return Err("invalid source lap timing startup boundary".into());
        }
        // Freestyle drift ignores race progress [V 0x5F8705]: no lap timing.
        if self.route.ignores_progress() {
            return Ok(());
        }
        let prepared = crate::lap_progress::load_route(root, &self.route)?;
        let position = std::array::from_fn(|i| self.pose()[12 + i]);
        let lap = grid_world::lap_timing::State::new(&prepared.course, position)?;
        self.initial_lap = Some(lap.clone());
        self.lap = Some(lap);
        self.lap_course = Some(std::sync::Arc::new(prepared.course));
        Ok(())
    }
    pub fn lap_gate_count(&self) -> usize {
        self.lap_course.as_ref().map_or(0, |c| c.gate_count())
    }
    pub fn lap_required_gate_count(&self) -> usize {
        self.lap_course
            .as_ref()
            .map_or(0, |c| c.required_gates().len())
    }

    /// Enlarge the initial stable pool and solver rows for a geometry provider.
    /// Reset retains this reservation. Call before the first driving tick.
    /// After enlarging beyond four slots, use `tick_with_geometry` for all ticks.
    pub fn reserve_geometry_pool(&mut self, capacity: usize) -> Result<(), Error> {
        if self.ticks != 0
            || !(4..=64).contains(&capacity)
            || self.state.world.groups.contacts.len() != 4
        {
            return Err(Error("invalid initial geometry reservation"));
        }
        self.state
            .world
            .groups
            .contacts
            .resize(capacity, grid_world::contact_groups::EMPTY_CONTACT);
        self.state.world.manager.rows.resize(capacity * 4, [0; 48]);
        self.state.world.manager.capacity = capacity * 4;
        self.initial = std::sync::Arc::new(self.state.clone());
        Ok(())
    }

    /// Prepare the selected source body shape and reserve four wheel plus sixteen
    /// geometric slots. Sampling remains an explicit locally prepared input.
    pub fn prepare_body_geometry(
        &mut self,
        root: &Path,
        table: &[u8],
    ) -> Result<crate::body_geometry::BodyGeometry, Box<dyn std::error::Error>> {
        let props = crate::entity_geometry::Props::read_route(root, &self.route)?;
        self.prepare_body_geometry_with(root, table, props)
    }

    /// [`Self::prepare_body_geometry`] with this runtime's route props already
    /// read (`Props::read_route`), so a field parses them once. The boxes are
    /// shared copy-on-write: a car that changes a box gets its own copy.
    pub fn prepare_body_geometry_with(
        &mut self,
        root: &Path,
        table: &[u8],
        props: crate::entity_geometry::Props,
    ) -> Result<crate::body_geometry::BodyGeometry, Box<dyn std::error::Error>> {
        let ned = read(root, &self.car.file(".ned"), 16 * 1024 * 1024)?;
        let offset =
            std::array::from_fn(|j| f32::from_bits(self.state.car.owner.vehicle[0x1630 / 4 + j]));
        let geometry = crate::body_geometry::BodyGeometry::read_on_track(
            &ned,
            offset,
            table,
            self.route.additional_ground_clearance(root)?,
        )?;
        let boundary = crate::cqtc_geometry::Geometry::read(&read(
            root,
            &self.route.file("boundarylines.cqtc"),
            2 * 1024 * 1024,
        )?)?;
        let wheel_nodes = crate::loose_wheel::WheelNodes::read(root, &self.car)?;
        if self.track.has_boundary() || self.state.shapes.len() != self.track.shape_count() + 1 {
            return Err("body boundary already prepared or shape registration differs".into());
        }
        // Four wheel slots, then sixteen per shape pair: route props, the
        // boundary and the road chunks under the car (the road pair had no
        // room once props and boundary were full, F342).
        self.reserve_geometry_pool(64)?;
        self.track.attach_boundary(boundary);
        self.state.shapes.push(grid_world::contact_groups::Shape {
            angular: true,
            ..Default::default()
        });
        // One registered static shape for all route entity boxes (objects.ens),
        // after the boundary so packed road and boundary identities are unchanged.
        self.track.attach_props(props);
        self.state.shapes.push(grid_world::contact_groups::Shape {
            angular: true,
            ..Default::default()
        });
        self.initial = std::sync::Arc::new(self.state.clone());
        self.wheel_nodes = Some(std::sync::Arc::new(wheel_nodes));
        Ok(geometry)
    }

    pub fn tick_with_geometry(
        &mut self,
        controls: Controls,
        mut geometry: impl FnMut(
            &mut [grid_world::contact_groups::Body],
            usize,
        ) -> Result<Vec<grid_world::contact_groups::Contact>, Error>,
    ) -> Result<(), Error> {
        self.tick_owned(controls, Some(Geometry::Callback(&mut geometry)))
    }

    /// Experimental actual-track body geometry. Uses the existing selected
    /// shared solver material and selected collision damage. Notifications remain
    /// explicitly guarded until their original event behavior is implemented.
    pub fn tick_with_body_geometry(
        &mut self,
        controls: Controls,
        geometry: &crate::body_geometry::BodyGeometry,
    ) -> Result<(), Error> {
        self.tick_owned(controls, Some(Geometry::Source(geometry)))
    }

    /// Selected player LOD1 source initialization and actual startup consumers.
    /// Retain Standard history/settings roles; no capture is used as state.
    pub fn enable_deformation(
        &mut self,
        root: &Path,
        geometry: &mut crate::body_geometry::BodyGeometry,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.ticks != 0 || self.deformation_enabled || !self.track.has_boundary() {
            return Err("invalid source deformation startup boundary".into());
        }
        let prepared = crate::source_deformation::load(root, self.car.stem())?;
        let mut next = self.state.clone();
        next.car.owner.vehicle =
            crate::source_deformation::initialize_existing(&prepared, &next.car.owner.vehicle)?;
        let mut staged_geometry = geometry.clone();
        next.car.owner = staged_geometry.publish_deformation(&next.car.owner, 0.)?;
        next.car
            .frame
            .body
            .copy_from_slice(&next.car.owner.vehicle[..vehicle_force::BODY_WORDS]);
        next.world.groups.bodies[0] = next.car.owner.physical;
        self.state = next;
        self.initial = std::sync::Arc::new(self.state.clone());
        self.deformation_enabled = true;
        *geometry = staged_geometry;
        Ok(())
    }

    pub fn tick_with_deforming_body_geometry(
        &mut self,
        controls: Controls,
        geometry: &mut crate::body_geometry::BodyGeometry,
    ) -> Result<(), Error> {
        self.tick_owned(controls, Some(Geometry::Deforming(geometry)))
    }
    pub fn register_deformation_callbacks(
        &mut self,
        limit: grid_world::deformation_contact_limit::Parameters,
        bulk: grid_world::deformation_callback_state::Parameters,
        random_seed: u32,
    ) -> Result<(), Error> {
        if !self.deformation_enabled || self.ticks != 0 || self.deformation_registration.is_some() {
            return Err(Error("invalid source deformation callback registration"));
        }
        let registration = grid_world::deformation_world::Registration {
            owner: self.state.car.owner.vehicle.clone(),
            limit,
            bulk,
            random_seed,
            bulk_calls: 0,
            records: 0,
            owner_toggle: false,
        };
        self.initial_deformation_registration = Some(std::sync::Arc::new(registration.clone()));
        self.deformation_registration = Some(registration);
        Ok(())
    }

    /// Validated controls written to a staged copy of the car state.
    fn staged_controls(&self, controls: Controls) -> Result<vehicle_world::State, Error> {
        Ok(vehicle_world::State {
            car: self.staged_car(controls)?,
            world: self.state.world.clone(),
            shapes: self.state.shapes.clone(),
        })
    }

    /// [`Self::staged_controls`] for the car alone (the shared world owns
    /// the bodies in [`multi::MultiRuntime`]).
    fn staged_car(&self, controls: Controls) -> Result<vehicle_manager::State, Error> {
        if [controls.throttle, controls.brake]
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
            || !controls.steering.is_finite()
            || !(-1. ..=1.).contains(&controls.steering)
        {
            return Err(Error("invalid driving controls"));
        }
        let mut car = self.state.car.clone();
        let steering = self.stage_driver(&mut car, controls.steering);
        if !(steering.is_finite() && (-1. ..=1.).contains(&steering)) {
            return Err(Error("invalid shaped steering command"));
        }
        for (o, v) in [
            (0xe1c, controls.throttle),
            (0xe20, controls.brake),
            (0xe3c, controls.throttle),
            (0xe40, controls.brake),
            (0xe30, steering),
        ] {
            car.owner.vehicle[o / 4] = v.to_bits();
            car.frame.body[o / 4] = v.to_bits();
        }
        byte(&mut car.owner.vehicle, 0xe38, u8::from(controls.handbrake));
        byte(&mut car.frame.body, 0xe38, u8::from(controls.handbrake));
        Ok(car)
    }

    fn tick_owned(
        &mut self,
        controls: Controls,
        geometry: Option<Geometry<'_>>,
    ) -> Result<(), Error> {
        if self.deformation_enabled != matches!(&geometry, Some(Geometry::Deforming(_))) {
            return Err(Error(
                "source deformation requires its owned geometry publication route",
            ));
        }
        let staged = self.staged_controls(controls)?;
        let mut cache = self.cache;
        let query = |_, ray: grid_world::contact_ray::Ray| {
            self.track
                .query(ray.start, ray.end, &mut cache)?
                .hit
                .map(|h| {
                    let (index, material) = self
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
        let material = |token| self.surfaces.joined(token);
        let mut body_contacts = 0;
        let mut recorded = Vec::new();
        let mut rebuilt_geometry = None;
        let mut next_registration = None;
        let output = match geometry {
            Some(Geometry::Deforming(geometry)) => {
                let resource = std::cell::RefCell::new(geometry.clone());
                let mut publication = |owner: &vehicle_publication::State, delta| {
                    resource.borrow_mut().publish_deformation(owner, delta)
                };
                let provider = |bodies: &mut [grid_world::contact_groups::Body], remaining| {
                    // World phase 2 (continuous) before the contacts.
                    if let Some(props) = self.props.as_ref().filter(|p| p.has_bodies()) {
                        props.continuous(
                            &self.track,
                            &self.surfaces,
                            bodies,
                            resource.borrow().query(),
                        )?;
                    }
                    let bodies: &[grid_world::contact_groups::Body] = bodies;
                    let contacts =
                        self.fresh_contacts(&resource.borrow(), bodies, remaining, &mut recorded)?;
                    body_contacts = contacts.len();
                    Ok(contacts)
                };
                let output = if let Some(registration) = &self.deformation_registration {
                    let (output, next) = vehicle_world::calculate_with_deformation(
                        &staged,
                        self.pair,
                        GRAVITY,
                        query,
                        material,
                        provider,
                        self.body_events,
                        &mut publication,
                        registration,
                    )?;
                    next_registration = Some(next);
                    output
                } else {
                    vehicle_world::calculate_with_geometry_events_publication(
                        &staged,
                        self.pair,
                        GRAVITY,
                        query,
                        material,
                        provider,
                        self.body_events,
                        &mut publication,
                    )?
                };
                rebuilt_geometry = Some((geometry, resource.into_inner()));
                output
            }
            Some(Geometry::Callback(geometry)) => vehicle_world::calculate_with_geometry_events(
                &staged,
                self.pair,
                GRAVITY,
                None,
                query,
                material,
                |bodies, remaining| {
                    let contacts = geometry(bodies, remaining)?;
                    body_contacts = contacts.len();
                    Ok(contacts)
                },
                self.body_events,
            )?,
            Some(Geometry::Source(geometry)) => vehicle_world::calculate_with_geometry_events(
                &staged,
                self.pair,
                GRAVITY,
                None,
                query,
                material,
                |bodies, remaining| {
                    let contacts =
                        self.fresh_contacts(geometry, bodies, remaining, &mut recorded)?;
                    body_contacts = contacts.len();
                    Ok(contacts)
                },
                self.body_events,
            )?,
            None => vehicle_world::calculate_with_events(
                &staged,
                self.pair,
                GRAVITY,
                None,
                query,
                material,
                self.body_events,
            )?,
        };
        let timed = if let (Some(course), Some(lap)) = (&self.lap_course, &self.lap) {
            let position = std::array::from_fn(|i| {
                f32::from_bits(output.state.car.owner.physical[0x1d0 / 4 + 12 + i])
            });
            Some(lap.advance(course, position, (self.ticks + 1) as f64 * f64::from(DELTA))?)
        } else if self.lap_course.is_some() || self.lap.is_some() {
            return Err(Error("inconsistent source lap timing ownership"));
        } else {
            None
        };
        let mut state = output.state;
        let mut props = None;
        if let Some(current) = &self.props {
            let mut next = current.clone();
            next.detect(
                &state.world.groups.contacts,
                &recorded,
                &state.world.groups.bodies,
            );
            next.hold_lost(&mut state.world.groups.bodies, self.track.bounds()[1]);
            let disabled = next.apply(&mut state.world, &mut state.shapes, self.ticks + 1)?;
            // Appending links the car body (chain tail) to the first piece.
            state.car.owner.physical = state.world.groups.bodies[0];
            props = Some((next, disabled));
        }
        if let Some((geometry, next)) = rebuilt_geometry {
            *geometry = next;
        }
        if let Some((next, disabled)) = props {
            crate::dynamic_props::PropWorld::disable(&mut self.track, &disabled);
            self.props = Some(next);
        }
        self.last_active_contacts = output.active_contacts;
        self.state = state;
        if let Some(registration) = next_registration {
            self.deformation_registration = Some(registration);
        }
        self.lap_events.clear();
        if let Some(timed) = timed {
            self.lap = Some(timed.state);
            self.lap_events = timed.events;
        }
        self.cache = cache;
        self.ticks += 1;
        self.last_body_contacts = body_contacts;
        let impacts = self.impacts();
        let lost = self.detach_wheels(&impacts);
        self.add_loose_wheels(lost)
    }

    /// Add the bodies of the wheels `lost` this tick to the world (creating
    /// an empty prop world if needed).
    pub(crate) fn add_loose_wheels(&mut self, lost: [bool; 4]) -> Result<(), Error> {
        let spawns = self.loose_wheels(lost, 0)?;
        if !spawns.is_empty() {
            let props = self
                .props
                .get_or_insert_with(|| crate::dynamic_props::PropWorld::new(Default::default()));
            for (wheel, s) in spawns {
                props.add_wheel(
                    &mut self.state.world,
                    &mut self.state.shapes,
                    0,
                    wheel,
                    s.spec,
                    s.linear,
                    s.angular,
                )?;
            }
            // Appending links the car body (chain tail) to the wheel.
            self.state.car.owner.physical = self.state.world.groups.bodies[0];
        }
        Ok(())
    }

    /// The debris bodies of the wheels `lost` this tick (9DC270 state 4,
    /// loose_wheel), by wheel record. `index` is the car's place in its
    /// world (the authored kick seed). Empty without body geometry.
    pub(crate) fn loose_wheels(
        &self,
        lost: [bool; 4],
        index: usize,
    ) -> Result<Vec<(usize, crate::loose_wheel::Spawn)>, Error> {
        let Some(nodes) = &self.wheel_nodes else {
            return Ok(Vec::new());
        };
        let v = &self.state.car.owner.vehicle;
        let p = &self.state.car.owner.physical;
        let f = |o: usize| f32::from_bits(v[o / 4]);
        let g = |o: usize| -> [f32; 3] { std::array::from_fn(|j| f32::from_bits(p[o / 4 + j])) };
        let car = crate::loose_wheel::CarMotion {
            pose: self.pose(),
            centre: g(0x1c0),
            velocity: g(0x220),
            angular: g(0x210),
            mass: f(0x15a0),
            offset: [f(0x1630), f(0x1634), f(0x1638)],
        };
        // +C754 while a deformation impact is active (+C7A0 = 1).
        let strength = (v[0xc7a0 / 4] == 1).then(|| f(0xc754));
        let poses = self.wheels();
        let mut out = Vec::new();
        for (i, _) in lost.iter().enumerate().filter(|(_, l)| **l) {
            // 98C5C0: the lost wheel takes the spin of its axle partner.
            let partner = [1, 0, 3, 2][i];
            let spin = f(0x1990 + partner * 0x150 + 0x104);
            let deflated = (v[(0x22e0 + i) / 4] >> ((0x22e0 + i) % 4 * 8)) & 255 != 0;
            let seed = (self.ticks as u32) ^ (((index * 4 + i) as u32) << 24);
            out.push((
                i,
                crate::loose_wheel::spawn(
                    nodes,
                    i,
                    deflated,
                    &car,
                    &crate::loose_wheel::wheel_motion(&poses[i]),
                    if spin.is_finite() { spin } else { 0. },
                    crate::loose_wheel::kick(strength, seed),
                )?,
            ));
        }
        Ok(out)
    }

    /// Wheels lost to the loose-part rule (car +22E4..+22E7, RL, RR, FL, FR).
    pub fn detached_wheels(&self) -> [bool; 4] {
        let v = &self.state.car.owner.vehicle;
        std::array::from_fn(|i| (v[(0x22e4 + i) / 4] >> ((0x22e4 + i) % 4 * 8)) & 255 != 0)
    }

    /// The loose-part user after a committed tick: 9A7670 -> 98B430 for each
    /// solved body notification of this car, then the 9EC9B0 wheel rule, which
    /// sets car +22E4 + i. The next tick's 9BD7F0 copy (vehicle_sync) makes
    /// the suspension, grip, contacts and handling treat the wheel as gone.
    /// Returns the newly detached wheels.
    pub(crate) fn detach_wheels(&mut self, impacts: &[crate::body_impacts::Impact]) -> [bool; 4] {
        let v = &self.state.car.owner.vehicle;
        let flag = |o: usize| (v[o / 4] >> (o % 4 * 8)) & 255 != 0;
        // 9A7670 forwards notifications only while car B13 and B14 are set;
        // the damage option's mode 0 clears B14.
        if !flag(0xb13) || !flag(0xb14) || impacts.is_empty() {
            return [false; 4];
        }
        let f = |o: usize| f32::from_bits(v[o / 4]);
        let frame = crate::loose_parts::Frame {
            pose: self.pose(),
            offset: [f(0x1630), f(0x1634), f(0x1638)],
        };
        let mut zones = [0.; crate::loose_parts::ZONES];
        for impact in impacts {
            frame.classify(&self.loose_parts, impact, &mut zones);
        }
        let mass = f32::from_bits(self.state.car.owner.physical[0x3f0 / 4]);
        let new = crate::loose_parts::wheels_to_detach(
            &self.loose_parts,
            &zones,
            mass,
            self.detached_wheels(),
        );
        for (i, &lost) in new.iter().enumerate() {
            if lost {
                // The frame keeps the car's body words in step with the owner.
                byte(&mut self.state.car.owner.vehicle, 0x22e4 + i, 1);
                byte(&mut self.state.car.frame.body, 0x22e4 + i, 1);
            }
        }
        new
    }

    /// Diagnostics: flag `wheel` detached now, as the loose-part rule would
    /// (+22E4 + wheel), while damage is on and the wheel is still attached.
    pub(crate) fn force_detach(&mut self, wheel: usize) -> [bool; 4] {
        let v = &self.state.car.owner.vehicle;
        let flag = |o: usize| (v[o / 4] >> (o % 4 * 8)) & 255 != 0;
        let mut lost = [false; 4];
        if wheel < 4 && flag(0xb13) && flag(0xb14) && !self.detached_wheels()[wheel] {
            byte(&mut self.state.car.owner.vehicle, 0x22e4 + wheel, 1);
            byte(&mut self.state.car.frame.body, 0x22e4 + wheel, 1);
            lost[wheel] = true;
        }
        lost
    }
    pub fn reset(&mut self) {
        self.state = (*self.initial).clone();
        self.deformation_registration = self.initial_deformation_registration.as_deref().cloned();
        self.lap = self.initial_lap.clone();
        self.lap_events.clear();
        self.shift = None;
        // The external geometry may still carry damage. Reconstruct it at the
        // next job publication before any advanced-pose contact query.
        if self.deformation_enabled {
            byte(&mut self.state.car.owner.vehicle, 0xc7f2, 1);
        }
        self.cache = self.initial_cache;
        self.ticks = 0;
        self.last_body_contacts = 0;
        self.last_active_contacts = 0;
        if let Some(props) = &mut self.props {
            // Re-arming only fails without route props, which enabling requires.
            let _ = props.reset(&mut [&mut self.track]);
        }
    }
    /// Mid-race re-placement at rest, the original reset placement 0x9145D0
    /// with speed 0 (docs/RESET_AND_RESPAWN_RESEARCH.md 3.3):
    /// - upper limit max(point Y + 1, ground + 1) from an upward vertical cast;
    /// - grid-spawn setter 0x9EF040: ground placement 0x9DE4E0 below the
    ///   limit with the current wheel displacements, heading along `heading`;
    ///   pose set, linear and angular velocity zero, wheel spin 0, gear
    ///   chosen again for speed 0.
    ///
    /// Damage, deformation, suspension and lap state are kept. A failed
    /// placement (center cast miss, steep ground) changes nothing.
    pub fn place_at_rest(&mut self, point: [f32; 3], heading: [f32; 3]) -> Result<(), Error> {
        let placement = self.placement(&self.state.car.owner.physical, point, heading)?;
        // The solo world's car body follows the car's physical record.
        let body = self
            .state
            .world
            .groups
            .bodies
            .first_mut()
            .ok_or(Error("missing car body"))?;
        apply_placement(body, &placement.physical);
        self.commit_placement(placement);
        Ok(())
    }

    /// The placement of [`Self::place_at_rest`] for the physical record
    /// `physical` (the shared world's body in [`multi::MultiRuntime`]).
    pub(crate) fn placement(
        &self,
        physical: &grid_world::contact_groups::Body,
        point: [f32; 3],
        heading: [f32; 3],
    ) -> Result<Placement, Error> {
        use grid_world::reset_lines;
        if point.iter().chain(&heading).any(|v| !v.is_finite()) {
            return Err(Error("invalid reset placement target"));
        }
        let mut cache = self.cache;
        let [start, end] = reset_lines::limit_cast(point[1]);
        let fraction = self
            .track
            .query(
                [point[0], start, point[2], 1.],
                [point[0], end, point[2], 1.],
                &mut cache,
            )?
            .hit
            .map(|h| h.contact.fraction);
        let limit = reset_lines::placement_limit(point[1], fraction);
        let vehicle = self.state.car.owner.vehicle.clone();
        let pose = spawn::align_below(
            [point[0], limit, point[2]],
            heading,
            placement_wheels(&vehicle),
            spawn::Normalization::ReferenceSse,
            limit,
            |x, z, upper, lower| {
                Ok(self
                    .track
                    .query([x, upper, z, 1.], [x, lower, z, 1.], &mut cache)?
                    .hit
                    .map(|h| upper - (upper - lower) * h.contact.fraction))
            },
        )?
        .matrix();
        let mut placement = Self::posed(physical, vehicle, pose)?;
        reset_lines::stop_drivetrain(&mut placement.vehicle);
        placement.cache = cache;
        Ok(placement)
    }

    /// The car words and `physical` with `pose` set through the pose setter
    /// and both velocities zero; the query cache is filled in by the caller.
    fn posed(
        physical: &grid_world::contact_groups::Body,
        mut vehicle: Vec<u32>,
        pose: [f32; 16],
    ) -> Result<Placement, Error> {
        vehicle[0x15e0 / 4..0x1620 / 4].copy_from_slice(&pose.map(f32::to_bits));
        // Linear +15C0 and angular +15D0: every lane zero (0xA69840/0xA69860).
        vehicle[0x15c0 / 4..0x15e0 / 4].fill(0);
        let mut bridged = *physical;
        // Interface-only zero slots of the bridge (as at construction); only
        // the pose and velocity words are taken back.
        for o in [0x328, 0x32c, 0x334, 0x338, 0x33c, 0x340] {
            bridged[o / 4] = 0;
        }
        let written = vehicle_bridge::calculate(
            &vehicle_bridge::State {
                physical: bridged,
                vehicle: suspension_prepare::State {
                    words: vehicle[..vehicle_force::BODY_WORDS].try_into().unwrap(),
                },
                shapes: Vec::new(),
            },
            &vehicle_bridge::Settings {
                notify: true,
                advance: None,
                correction: None,
                group_head: false,
            },
        )?
        .written;
        vehicle[..vehicle_force::BODY_WORDS].copy_from_slice(&written.vehicle.words);
        let mut placed = *physical;
        apply_placement(&mut placed, &written.physical);
        Ok(Placement {
            vehicle,
            physical: placed,
            cache: rectangle_cull::Cache::default(),
        })
    }

    /// Test set-up: put the car at `pose` at rest, keeping everything else.
    #[cfg(test)]
    pub(crate) fn set_pose_at_rest(&mut self, pose: [f32; 16]) {
        let mut placement = Self::posed(
            &self.state.car.owner.physical,
            self.state.car.owner.vehicle.clone(),
            pose,
        )
        .unwrap();
        placement.cache = self.cache;
        apply_placement(&mut self.state.world.groups.bodies[0], &placement.physical);
        self.commit_placement(placement);
    }

    pub(crate) fn commit_placement(&mut self, placement: Placement) {
        let car = &mut self.state.car;
        car.frame
            .body
            .copy_from_slice(&placement.vehicle[..vehicle_force::BODY_WORDS]);
        if let Some(registration) = &mut self.deformation_registration {
            registration.owner.clone_from(&placement.vehicle);
        }
        car.owner.vehicle = placement.vehicle;
        car.owner.physical = placement.physical;
        self.cache = placement.cache;
        self.shift = None;
    }

    /// Solved contact notifications of the last successful solo tick
    /// (presentation only; the shared world reports through `MultiRuntime`).
    pub fn impacts(&self) -> Vec<crate::body_impacts::Impact> {
        let contacts = &self.state.world.groups.contacts;
        crate::body_impacts::collect(
            &contacts[..self.last_active_contacts.min(contacts.len())],
            1,
        )
    }

    /// [`Self::impacts`] without the records against packed track chunks,
    /// which the drift collision listener 0x64D840 skips (type 11).
    pub fn impacts_off_track(&self) -> Vec<crate::body_impacts::Impact> {
        let contacts = &self.state.world.groups.contacts;
        contacts[..self.last_active_contacts.min(contacts.len())]
            .iter()
            .filter(|c| {
                let other = if c[0x28 / 4] == 1 { c[0x2c / 4] } else { c[0x28 / 4] };
                !self.track.is_chunk_token(other)
            })
            .flat_map(|c| crate::body_impacts::collect(std::slice::from_ref(c), 1))
            .collect()
    }

    /// Opt in to breakable route props (after `prepare_body_geometry`, before
    /// the first tick): breakable boxes get their threshold as contact limit
    /// and break into dynamic pieces (DYNAMIC_PROPS_RESEARCH.md).
    pub fn enable_dynamic_props(&mut self, root: &Path) -> Result<(), Box<dyn std::error::Error>> {
        if self.ticks != 0 || self.props.is_some() {
            return Err("dynamic props must be enabled once before the first tick".into());
        }
        let props = self
            .track
            .props()
            .ok_or("dynamic props require prepared body geometry")?;
        let catalog = crate::dynamic_props::Catalog::read(root, &self.route, props)?;
        let world = crate::dynamic_props::PropWorld::new(catalog);
        world.arm(&mut self.track)?;
        self.props = Some(world);
        Ok(())
    }

    /// Breakable route props when enabled, and the bodies of lost wheels.
    pub fn dynamic_props(&self) -> Option<&crate::dynamic_props::PropWorld> {
        self.props.as_ref()
    }

    /// World bodies: the car, then pieces and lost wheels.
    pub fn bodies(&self) -> &[grid_world::contact_groups::Body] {
        &self.state.world.groups.bodies
    }

    /// Car body contacts plus, with dynamic props, every live piece's contacts.
    /// Prop contacts are recorded with their pool index for break detection.
    fn fresh_contacts(
        &self,
        geometry: &crate::body_geometry::BodyGeometry,
        bodies: &[grid_world::contact_groups::Body],
        remaining: usize,
        recorded: &mut Vec<(usize, crate::dynamic_props::PropContact)>,
    ) -> Result<Vec<grid_world::contact_groups::Contact>, Error> {
        let Some(props) = &self.props else {
            return geometry.contacts(&self.track, &self.surfaces, bodies, remaining);
        };
        let first = self
            .state
            .world
            .groups
            .contacts
            .len()
            .checked_sub(remaining)
            .ok_or(Error("invalid geometry capacity"))?;
        let mut out = Vec::new();
        for (c, prop) in geometry.contacts_with_props(
            &self.track,
            &self.surfaces,
            bodies.get(..1).ok_or(Error("missing car body"))?,
            remaining,
        )? {
            if let Some(prop) = prop {
                recorded.push((
                    first + out.len(),
                    crate::dynamic_props::PropContact {
                        contact: c,
                        prop,
                        body: 0,
                    },
                ));
            }
            out.push(c);
        }
        if props.has_bodies() {
            let mut local = Vec::new();
            let more = props.contacts(
                &self.track,
                &self.surfaces,
                bodies,
                &[crate::dynamic_props::CarHull {
                    geometry,
                    body: 0,
                    token: 1,
                }],
                geometry.query(),
                remaining - out.len(),
                &mut local,
            )?;
            let base = first + out.len();
            recorded.extend(local.into_iter().map(|(i, r)| (base + i, r)));
            out.extend(more);
        }
        Ok(out)
    }
    pub fn pose(&self) -> [f32; 16] {
        std::array::from_fn(|i| f32::from_bits(self.state.car.owner.physical[0x1d0 / 4 + i]))
    }
    /// Read-only collision world (diagnostics and presentation).
    pub fn route(&self) -> &Route {
        &self.route
    }
    /// The purchased car model this runtime drives.
    pub fn car(&self) -> &Car {
        &self.car
    }
    /// The car's AI traffic extents (AI car state +0x110/+0x114 and bumper
    /// points), from its collision hull and current wheel words.
    pub fn ai_extents(&self) -> crate::ai_driver::Extents {
        crate::ai_driver::Extents::from_vehicle(self.hull, &self.state.car.owner.vehicle)
    }
    pub fn track(&self) -> &Track {
        &self.track
    }
    /// Registered static track shapes (packed chunks, boundary, route props).
    pub fn track_shape_count(&self) -> usize {
        self.track.shape_count()
    }
    /// Read-only purchased surface table (presentation such as tyre audio).
    pub fn surfaces(&self) -> &Surfaces {
        &self.surfaces
    }
    pub fn velocity(&self) -> [f32; 4] {
        std::array::from_fn(|i| f32::from_bits(self.state.car.owner.physical[0x220 / 4 + i]))
    }
    /// World angular velocity of the physical body (rad/s).
    pub fn angular_velocity(&self) -> [f32; 4] {
        std::array::from_fn(|i| f32::from_bits(self.state.car.owner.physical[0x210 / 4 + i]))
    }
    /// Steering lock (vehicle +E54, radians): the wheel angle of a full
    /// steering command; used to normalise AI steering.
    pub fn maximum_steering_angle(&self) -> f32 {
        f32::from_bits(self.state.car.owner.vehicle[0xe54 / 4])
    }
    /// The assisted automatic gearbox has selected reverse (vehicle +196C = 9).
    pub fn reverse_gear(&self) -> bool {
        self.state.car.owner.vehicle[0x196c / 4] == 9
    }
    /// AI race-manager inputs for the next tick (docs/AI_MISTAKES_PACING_RESEARCH.md
    /// §4): the engine torque factor at engine +0xE8 (vehicle +1888, already
    /// clamped as 0x98F2D0 does) and each wheel's grip scale at wheel +0x80
    /// (0x408710), rear L/R, front L/R. Written for AI cars only.
    pub fn set_ai_pacing(&mut self, torque: f32, grip: [f32; 4]) {
        self.state.car.owner.vehicle[0x1888 / 4] = torque.to_bits();
        self.set_ai_grip(grip);
    }

    /// Only the wheel grip scales of [`Self::set_ai_pacing`] (headless runs
    /// without a race manager apply the AI request grips this way).
    pub fn set_ai_grip(&mut self, grip: [f32; 4]) {
        let b = &mut self.state.car.owner.vehicle;
        for (i, g) in grip.iter().enumerate() {
            b[(0x1990 + i * 0x150 + 0x80) / 4] = g.to_bits();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires purchased GRID source progress and vehicle resources"]
    fn source_driving_lap_progress() {
        std::thread::Builder::new().stack_size(16*1024*1024).spawn(||{
            use grid_world::lap_timing::{State as Lap,Event};
            let root=std::env::var_os("GRID_GAME_ROOT").unwrap();let root=Path::new(&root);
            let prepared=crate::lap_progress::load(root).unwrap();assert_eq!(prepared.course.gate_count(),71);assert_eq!(prepared.progress.points.len(),389);
            let (center,forward)=prepared.course.gate_frame(0).unwrap();
            let at=|sign:f64|[(center[0]+sign*2.*forward[0]) as f32,center[1] as f32,(center[2]+sign*2.*forward[1]) as f32];
            let mut positions=vec![at(1.)];positions.extend(prepared.progress.points.iter().filter(|p|p.distance>2.).map(|p|p.position));positions.push(at(1.));
            let traverse=||{
                let mut state=Lap::new(&prepared.course,at(-1.)).unwrap();let mut events=Vec::new();
                for (i,&position) in positions.iter().enumerate(){let output=state.advance(&prepared.course,position,(i+1) as f64*0.1).unwrap();state=output.state;events.extend(output.events);}
                (state,events)
            };
            let (lap,events)=traverse();assert_eq!((lap.attempts,lap.valid_laps),(1,1));assert!(lap.last_lap.as_ref().unwrap().valid);assert_eq!(lap.last_lap.as_ref().unwrap().sectors.len(),3);assert!(lap.last_lap.as_ref().unwrap().sectors.iter().all(Option::is_some));assert_eq!(events.iter().filter(|e|matches!(e,Event::Checkpoint{..})).count(),2);
            assert_eq!(traverse(),(lap.clone(),events));
            let mut reverse=Lap::new(&prepared.course,at(-1.)).unwrap();
            let mut reversed:Vec<_>=prepared.progress.points.iter().filter(|p|p.distance>2.).rev().map(|p|p.position).collect();reversed.push(at(-1.));
            for (i,&p) in reversed.iter().enumerate(){reverse=reverse.advance(&prepared.course,p,(i+1) as f64*0.1).unwrap().state;}
            assert_eq!(reverse.valid_laps,0);assert!(reverse.elapsed().is_none());
            // Actual car ticks retain all existing vehicle/world behavior.
            let mut baseline=Runtime::lacetti(root,"slot_00",200679).unwrap();let mut timed=Runtime::lacetti(root,"slot_00",200679).unwrap();timed.enable_lap_timing(root).unwrap();
            let mut started=0;let controls=Controls{throttle:1.,..Default::default()};
            for _ in 0..300{
                baseline.tick(controls).unwrap();timed.tick(controls).unwrap();started+=timed.lap_events.iter().filter(|e|matches!(e,Event::Started{..})).count();
                assert_eq!(timed.state.car.owner.vehicle,baseline.state.car.owner.vehicle);assert_eq!(timed.state.car.owner.physical,baseline.state.car.owner.physical);assert_eq!(timed.state.world.groups.contacts,baseline.state.world.groups.contacts);assert_eq!(timed.state.world.manager.rows,baseline.state.world.manager.rows);
            }
            assert_eq!(started,1);assert_eq!(timed.lap.as_ref().unwrap().valid_laps,0);assert_eq!(timed.lap.as_ref().unwrap().passed_gates(),1);assert_eq!(timed.lap.as_ref().unwrap().next_gate,22);assert!(timed.lap.as_ref().unwrap().elapsed().unwrap()>0.);
            timed.lap.as_mut().unwrap().next_gate=71;
            let before=timed.state.clone();let before_lap=timed.lap.clone();let cache=timed.cache;let tick=timed.ticks;
            assert!(timed.tick(controls).is_err());assert_eq!(timed.state.car.owner.vehicle,before.car.owner.vehicle);assert_eq!(timed.state.car.owner.physical,before.car.owner.physical);assert_eq!(timed.state.world.groups.contacts,before.world.groups.contacts);assert_eq!(timed.state.world.manager.rows,before.world.manager.rows);assert_eq!(timed.lap,before_lap);assert_eq!(timed.cache.plane,cache.plane);assert_eq!(timed.ticks,tick);
            timed.reset();assert_eq!(timed.ticks,0);assert!(timed.lap.as_ref().unwrap().elapsed().is_none());assert!(timed.lap_events.is_empty());
            eprintln!("source_lap_progress gates=71 points=389 sectors=3 ordered_kinematic_lap_valid=1 source_path_repeat_exact=1 reverse_path_valid_laps=0 actual_driving_ticks=300 actual_start_events={started} no_vehicle_world_feedback=1 late_timing_failure_atomic=1 reset_waits_for_finish=1");
        }).unwrap().join().unwrap();
    }

    #[test]
    #[ignore = "requires purchased GRID car and route resources"]
    fn ai_extents_are_each_cars_hull_and_body_offset() {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
                let root = Path::new(&root);
                // collision_main bounds (docs/CAR_COLLISION.md) and body
                // offset Z (+0x1638) of each car; no tyre reaches past them.
                for (stem, half_width, max_z, min_z, offset_z) in [
                    (
                        "lac",
                        0.913279f32,
                        2.241661f32,
                        -2.237876f32,
                        -0.14642298f32,
                    ),
                    ("bmw", 0.947784, 2.258862, -2.237876, -0.1827681),
                ] {
                    let car = Car::new(stem).unwrap();
                    let runtime =
                        Runtime::load(root, &Route::default(), &car, "slot_01", 200679).unwrap();
                    assert_eq!(
                        f32::from_bits(runtime.state.car.owner.vehicle[0x1638 / 4]),
                        offset_z
                    );
                    let e = runtime.ai_extents();
                    assert_eq!(
                        (e.half_width, e.front, e.rear),
                        (half_width, max_z + offset_z, min_z + offset_z),
                        "{stem}"
                    );
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    #[ignore = "requires purchased sources and private contact/event/deformation parameters"]
    fn source_driving_deformation_callbacks() {
        std::thread::Builder::new().stack_size(16*1024*1024).spawn(||{
            let root=std::env::var_os("GRID_GAME_ROOT").unwrap();let root=Path::new(&root);
            let read_env=|name|std::fs::read(std::env::var_os(name).unwrap()).unwrap();
            let table=read_env("GRID_CONTACT_DIRECTIONS");
            let (limit,bulk)=crate::deformation_parameters::read(&read_env("GRID_DEFORMATION_PARAMETERS")).unwrap();
            let mut runtime=Runtime::lacetti(root,"slot_00",200679).unwrap();
            let mut geometry=runtime.prepare_body_geometry(root,&table).unwrap();
            runtime.enable_deformation(root,&mut geometry).unwrap();
            runtime.body_events=Some(crate::body_impact_parameters::read(&read_env("GRID_BODY_EVENT_PARAMETERS")).unwrap());
            runtime.register_deformation_callbacks(limit,bulk,1).unwrap();
            runtime.enable_lap_timing(root).unwrap();
            let pristine=runtime.state.car.owner.vehicle[0x4880/4..0xc580/4].to_vec();
            let controls=Controls{throttle:1.,steering:0.25,..Default::default()};
            // Inconsistent globals fail inside the physical phase after the
            // driver/publication/query stage, preserving all committed owners.
            runtime.deformation_registration.as_mut().unwrap().bulk.generation[0]+=1.;
            let before=runtime.state.clone();let seed=runtime.deformation_registration.as_ref().unwrap().random_seed;let cache=runtime.cache;
            let shape_contacts=geometry.contacts(&runtime.track,&runtime.surfaces,&before.world.groups.bodies,64).unwrap();
            assert!(runtime.tick_with_deforming_body_geometry(controls,&mut geometry).is_err());
            assert_eq!(runtime.state.car.owner.vehicle,before.car.owner.vehicle);assert_eq!(runtime.state.car.owner.physical,before.car.owner.physical);
            assert_eq!(runtime.state.world.groups.contacts,before.world.groups.contacts);assert_eq!(runtime.state.world.manager.rows,before.world.manager.rows);
            assert_eq!(runtime.deformation_registration.as_ref().unwrap().random_seed,seed);assert_eq!(runtime.ticks,0);assert_eq!(runtime.cache.plane,cache.plane);
            assert_eq!(geometry.contacts(&runtime.track,&runtime.surfaces,&before.world.groups.bodies,64).unwrap(),shape_contacts);
            runtime.reset();
            let drive=|runtime:&mut Runtime,geometry:&mut crate::body_geometry::BodyGeometry|{
                let mut contacts=0;
                // First mid-run tick (>=165) with pending deformed geometry
                // (C7F2). The idling launch changed its exact timing.
                let mut checked=None;
                for _ in 0..600 {
                    if checked.is_none() && runtime.ticks>=165 && runtime.state.car.owner.vehicle[0xc7f0/4]&0xff0000!=0 {
                        let tick=runtime.ticks;checked=Some(tick);
                        let good_lap=runtime.lap.clone();
                        runtime.lap.as_mut().unwrap().next_gate=71;
                        let before=runtime.state.clone();let before_lap=runtime.lap.clone();let cache=runtime.cache;
                        let registration=runtime.deformation_registration.as_ref().unwrap().clone();
                        let shape_contacts=geometry.contacts(&runtime.track,&runtime.surfaces,&before.world.groups.bodies,64).unwrap();
                        assert_eq!(runtime.tick_with_deforming_body_geometry(controls,geometry).err().unwrap().0,"invalid prototype lap interval");
                        assert_eq!(runtime.state.car.owner.vehicle,before.car.owner.vehicle);assert_eq!(runtime.state.car.owner.physical,before.car.owner.physical);
                        assert_eq!(runtime.state.world.groups.contacts,before.world.groups.contacts);assert_eq!(runtime.state.world.manager.rows,before.world.manager.rows);
                        assert_eq!(runtime.lap,before_lap);assert_eq!(runtime.ticks,tick);assert_eq!(runtime.cache.plane,cache.plane);
                        let after=runtime.deformation_registration.as_ref().unwrap();
                        assert_eq!((after.random_seed,after.bulk_calls,after.records),(registration.random_seed,registration.bulk_calls,registration.records));
                        assert_eq!(geometry.contacts(&runtime.track,&runtime.surfaces,&before.world.groups.bodies,64).unwrap(),shape_contacts);
                        runtime.lap=good_lap;
                    }
                    runtime.tick_with_deforming_body_geometry(controls,geometry).unwrap_or_else(|e|panic!("callback tick {}: {e}",runtime.ticks+1));contacts+=runtime.last_body_contacts;
                }
                eprintln!("source_deformation_pending_geometry_check_tick={checked:?}");
                assert!(checked.is_some());
                contacts
            };
            let contacts=drive(&mut runtime,&mut geometry);let first=runtime.state.clone();let registration=runtime.deformation_registration.as_ref().unwrap().clone();let first_cache=runtime.cache;
            assert!(contacts>0);assert!(registration.bulk_calls>0);
            let changed=runtime.state.car.owner.vehicle[0x4880/4..0xc580/4]!=pristine;
            let owner=&runtime.state.car.owner.vehicle;
            let node_count=(0..3).map(|i|owner[0xc714/4+i] as usize).product::<usize>();
            let moved_positions=(0..node_count).filter(|&i|(0..3).any(|a|owner[0x4880/4+i*20+a]!=pristine[i*20+a])).count();
            let maximum_displacement=(0..node_count).map(|i|{(0..3).map(|a|{let d=f32::from_bits(owner[0x4880/4+i*20+a])-f32::from_bits(pristine[i*20+a]);d*d}).sum::<f32>().sqrt()}).fold(0f32,f32::max);
            assert!(moved_positions>0 && maximum_displacement>0.);
            runtime.reset();let repeated=drive(&mut runtime,&mut geometry);
            assert_eq!(contacts,repeated);assert_eq!(runtime.state.car.owner.vehicle,first.car.owner.vehicle);assert_eq!(runtime.state.car.owner.physical,first.car.owner.physical);
            assert_eq!(runtime.state.car.owner.damage_random_state,first.car.owner.damage_random_state);
            assert_eq!(runtime.state.world.groups.bodies,first.world.groups.bodies);assert_eq!(runtime.state.world.groups.contacts,first.world.groups.contacts);assert_eq!(runtime.state.world.groups.world,first.world.groups.world);
            assert_eq!(runtime.state.world.manager.rows,first.world.manager.rows);assert_eq!(runtime.state.world.manager.workspace,first.world.manager.workspace);
            let replay=runtime.deformation_registration.as_ref().unwrap();assert_eq!((replay.random_seed,replay.bulk_calls,replay.records),(registration.random_seed,registration.bulk_calls,registration.records));assert_eq!(runtime.cache.plane,first_cache.plane);
            eprintln!("source_deformation_callbacks ticks=600 replay_ticks=600 body_contacts={contacts} bulk_calls={} records={} crt_seed={:08x} nodes_changed={changed} moved_positions={moved_positions} maximum_displacement={maximum_displacement} late_physical_failure_atomic=1 late_timing_failure_after_dirty_geometry_atomic=1 reset_replay_exact=1 authored_grid_changes=0",registration.bulk_calls,registration.records,registration.random_seed);
        }).unwrap().join().unwrap();
    }

    #[test]
    #[ignore = "requires purchased GRID sources and private contact directions"]
    fn source_driving_source_deformation() {
        std::thread::Builder::new().stack_size(16*1024*1024).spawn(||{
            let root=std::env::var_os("GRID_GAME_ROOT").unwrap();let root=Path::new(&root);
            let table=std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
            let mut runtime=Runtime::lacetti(root,"slot_00",200679).unwrap();
            let history=runtime.state.car.owner.vehicle[0x290/4..0x300/4].to_vec();
            let preserved=runtime.state.car.owner.vehicle[0xc780/4];
            let mut geometry=runtime.prepare_body_geometry(root,&table).unwrap();
            runtime.enable_deformation(root,&mut geometry).unwrap();
            assert_eq!(&runtime.state.car.owner.vehicle[0x290/4..0x300/4],history);
            assert_eq!(runtime.state.car.owner.vehicle[0xc780/4],preserved);
            assert_eq!(runtime.state.car.owner.vehicle[0xc7f8/4],0xd240);
            assert_eq!(f32::from_bits(runtime.state.car.owner.vehicle[0xee40/4]),0.1);
            assert_eq!(f32::from_bits(runtime.state.car.owner.vehicle[0xeef8/4]),0.001);
            assert_eq!(f32::from_bits(runtime.state.car.owner.vehicle[0xeefc/4]),1.);
            let controls=Controls{throttle:1.,steering:0.25,..Default::default()};
            assert!(runtime.tick_with_body_geometry(controls,&geometry).is_err());
            // Fail after the driver and staged geometry update, before world
            // commit. Preserve physical/world/cache and actual query geometry.
            byte(&mut runtime.state.car.owner.vehicle,0xc7f2,1);
            runtime.state.car.owner.vehicle[0xcd9c/4]=0;
            let before=runtime.state.clone();let cache=runtime.cache;
            let actual=geometry.contacts(&runtime.track,&runtime.surfaces,&before.world.groups.bodies,64).unwrap();
            assert!(runtime.tick_with_deforming_body_geometry(controls,&mut geometry).is_err());
            assert_eq!(runtime.state.car.owner.vehicle,before.car.owner.vehicle);
            assert_eq!(runtime.state.car.owner.physical,before.car.owner.physical);
            assert_eq!(runtime.state.world.groups.contacts,before.world.groups.contacts);
            assert_eq!(runtime.state.world.manager.rows,before.world.manager.rows);
            assert_eq!(runtime.cache.plane,cache.plane);assert_eq!(runtime.ticks,0);
            assert_eq!(geometry.contacts(&runtime.track,&runtime.surfaces,&before.world.groups.bodies,64).unwrap(),actual);
            runtime.reset();
            let drive=|runtime:&mut Runtime,geometry:&mut crate::body_geometry::BodyGeometry|{
                let mut contacts=0;
                for tick in 0..600 {
                    if tick==80 {
                        // Explicit authored grid displacement to exercise the
                        // integrated consumer; not a collision-production claim.
                        let b=&mut runtime.state.car.owner.vehicle;
                        let count=(0..3).map(|i|b[0xc714/4+i] as usize).product::<usize>();
                        for i in 0..count{let at=(0x4880+i*80)/4;b[at]=(f32::from_bits(b[at])+0.1).to_bits();}
                        byte(b,0xc7f2,1);
                    }
                    runtime.tick_with_deforming_body_geometry(controls,geometry).unwrap();
                    contacts+=runtime.last_body_contacts;
                    assert_eq!(runtime.state.car.owner.vehicle[0xc7f0/4]>>16&255,0);
                    assert_eq!(runtime.state.car.owner.vehicle[0xc7f8/4],0xd240);
                }
                contacts
            };
            let contacts=drive(&mut runtime,&mut geometry);assert!(contacts>0);
            let first=runtime.state.clone();let first_cache=runtime.cache;let last=runtime.last_body_contacts;
            runtime.reset();let repeated=drive(&mut runtime,&mut geometry);
            assert_eq!(contacts,repeated);assert_eq!(runtime.state.car.owner.vehicle,first.car.owner.vehicle);
            assert_eq!(runtime.state.car.owner.physical,first.car.owner.physical);
            assert_eq!(runtime.state.car.owner.damage_random_state,first.car.owner.damage_random_state);
            assert_eq!(runtime.state.car.frame.body,first.car.frame.body);assert_eq!(runtime.state.car.frame.solver,first.car.frame.solver);
            assert_eq!(runtime.state.world.groups.bodies,first.world.groups.bodies);
            assert_eq!(runtime.state.world.groups.contacts,first.world.groups.contacts);
            assert_eq!(runtime.state.world.groups.world,first.world.groups.world);
            assert_eq!(runtime.state.world.manager.rows,first.world.manager.rows);
            assert_eq!(runtime.state.world.manager.workspace,first.world.manager.workspace);
            assert_eq!(runtime.state.world.manager.ends,first.world.manager.ends);
            assert_eq!(runtime.cache.plane,first_cache.plane);assert_eq!(runtime.last_body_contacts,last);assert_eq!(runtime.ticks,600);
            eprintln!("source_deformation_runtime ticks=600 replay_ticks=600 body_contacts={contacts} source_history_preserved=1 late_failure_atomic=1 reset_replay_exact=1 supplied_grid_change=1 physical_bulk_callbacks=0");
        }).unwrap().join().unwrap();
    }

    #[test]
    #[ignore = "requires purchased GRID sources and private contact directions"]
    fn source_driving_deformed_body_geometry() {
        std::thread::Builder::new().stack_size(16*1024*1024).spawn(|| {
            let root=std::env::var_os("GRID_GAME_ROOT").unwrap();let root=Path::new(&root);
            let runtime=Runtime::lacetti(root,"slot_00",200679).unwrap();
            let table=std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
            let ned=read(root,"cars/lac/lac.ned",16*1024*1024).unwrap();
            let offset=std::array::from_fn(|j|f32::from_bits(runtime.state.car.owner.vehicle[0x1630/4+j]));
            let mut geometry=crate::body_geometry::BodyGeometry::read(&ned,offset,&table).unwrap();
            let prepared=crate::source_deformation::load(root,"lac").unwrap();let d=&prepared.construction.initialized;
            let dimensions:[usize;3]=std::array::from_fn(|i|d[0xa414/4+i] as usize);
            let spacing=f32::from_bits(d[0xa420/4]);let minimum=std::array::from_fn(|i|f32::from_bits(d[0xa430/4+i]));
            let positions:Vec<[f32;4]>=(0..dimensions.iter().product::<usize>()).map(|i|std::array::from_fn(|a|f32::from_bits(d[(0x2580+i*80)/4+a]))).collect();
            let grid=|positions|grid_world::deformation_grid::Grid{dimensions,spacing,minimum,positions};
            geometry.rebuild_from_deformation(&grid(&positions)).unwrap();
            let mut lowered=runtime.state.car.owner.physical;lowered[0x1c4/4]=(f32::from_bits(lowered[0x1c4/4])-0.5).to_bits();
            let contacts=|geometry:&crate::body_geometry::BodyGeometry|geometry.contacts(&runtime.track,&runtime.surfaces,&[lowered],64).unwrap();
            let pristine=contacts(&geometry);assert!(!pristine.is_empty());
            let displaced:Vec<_>=positions.iter().map(|p|[p[0]+0.25,p[1],p[2],p[3]]).collect();
            geometry.rebuild_from_deformation(&grid(&displaced)).unwrap();
            let deformed=contacts(&geometry);assert!(!deformed.is_empty());assert_ne!(deformed,pristine);
            let mut invalid=displaced.clone();invalid.last_mut().unwrap()[0]=f32::NAN;
            assert!(geometry.rebuild_from_deformation(&grid(&invalid)).is_err());
            assert_eq!(contacts(&geometry),deformed);
            geometry.rebuild_from_deformation(&grid(&positions)).unwrap();assert_eq!(contacts(&geometry),pristine);
            // Actual geometry + complete owned wheel/view publication, including
            // a failure reached after successful staged shape construction.
            let mut publication=runtime.state.car.owner.clone();
            publication.vehicle[0x2300/4..0xc800/4].copy_from_slice(d);
            publication.vehicle[0xd240/4..0xd240/4+prepared.settings.len()].copy_from_slice(&prepared.settings);
            publication.vehicle[0xc7f8/4]=0xd240;
            publication.vehicle[0xee40/4]=0.1f32.to_bits();
            publication.vehicle[0xeefc/4]=1f32.to_bits();
            for at in [0xcd90,0xcd94,0xcd98,0xcd9c]{publication.vehicle[at/4]=0.1f32.to_bits();}
            for i in 0..4 {
                let anchor: [u32;4]=publication.vehicle[(0x1990+i*0x150)/4..(0x1990+i*0x150)/4+4].try_into().unwrap();
                publication.vehicle[(0xcda0+i*16)/4..(0xcda0+i*16)/4+4].copy_from_slice(&anchor);
            }
            let published=geometry.publish_deformation(&publication,DELTA).unwrap();
            assert_eq!(contacts(&geometry),pristine);
            assert_eq!((published.vehicle[0xc7f0/4]>>16)&255,0);
            assert_eq!(published.vehicle[1]&255,1);
            let mut pending=published.clone();
            byte(&mut pending.vehicle,0xc7f2,1);
            for (i,p) in displaced.iter().enumerate(){pending.vehicle[(0x4880+i*80)/4..(0x4880+i*80)/4+4].copy_from_slice(&p.map(f32::to_bits));}
            // Invalid view range is checked after geometry and wheel updates.
            pending.vehicle[0xcd9c/4]=0;
            let before=pending.clone();
            assert!(geometry.publish_deformation(&pending,DELTA).is_err());
            assert_eq!(contacts(&geometry),pristine);
            assert_eq!(pending.vehicle,before.vehicle);assert_eq!(pending.physical,before.physical);
            pending.vehicle[0xcd9c/4]=0.1f32.to_bits();
            let published=geometry.publish_deformation(&pending,DELTA).unwrap();
            assert_eq!(contacts(&geometry),deformed);
            assert_ne!(&published.vehicle[0x1650/4..0x1690/4],&before.vehicle[0x1650/4..0x1690/4]);
            assert_eq!((published.vehicle[0xc7f0/4]>>16)&255,0);
            assert_eq!(publication.vehicle[0xc7f0/4]>>16&255,1);
            assert_eq!(runtime.ticks,0);
            eprintln!("deformed_source_geometry pristine_contacts={} deformed_contacts={} failure_preserves_contacts=1 source_rebuild_restores_contacts=1 atomic_geometry_wheels_view=1 late_view_failure_preserves_all=1",pristine.len(),deformed.len());
        }).unwrap().join().unwrap();
    }

    #[test]
    #[ignore = "requires purchased GRID sources and private body parameters"]
    fn source_driving_active_grid() {
        // Two complete owned worlds need more than the default test-thread stack.
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024)
            .spawn(|| {
                let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
                let root = Path::new(&root);
                let table =
                    std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
                let data =
                    std::fs::read(std::env::var_os("GRID_BODY_EVENT_PARAMETERS").unwrap()).unwrap();
                let parameters = crate::body_impact_parameters::read(&data).unwrap();
                let mut baseline = Runtime::lacetti(root, "slot_00", 200679).unwrap();
                let mut active = Runtime::lacetti(root, "slot_00", 200679).unwrap();
                let geometry = baseline.prepare_body_geometry(root, &table).unwrap();
                let active_geometry = active.prepare_body_geometry(root, &table).unwrap();
                baseline.body_events = Some(parameters);
                active.body_events = Some(parameters);
                // Explicit authored grid: this does not claim source initialization.
                let owner = &mut active.state.car.owner.vehicle;
                owner[0x4880 / 4..0xc580 / 4].fill(0);
                for (node, displacement) in [(0, 0.125f32), (1, 0.25), (4, 0.5), (5, 1.)] {
                    owner[(0x4880 + node * 80 + 12) / 4] = displacement.to_bits();
                }
                owner[0xc714 / 4..0xc720 / 4].fill(2);
                owner[0xc7c4 / 4..0xc7ec / 4].fill(0);
                byte(owner, 0xc7b1, 1);
                let pool = owner[0x4880 / 4..0xc580 / 4].to_vec();
                let mut contacts = 0;
                for _ in 0..600 {
                    let controls = Controls {
                        throttle: 1.,
                        steering: 0.25,
                        ..Default::default()
                    };
                    baseline
                        .tick_with_body_geometry(controls, &geometry)
                        .unwrap();
                    active
                        .tick_with_body_geometry(controls, &active_geometry)
                        .unwrap();
                    contacts += active.last_body_contacts;
                    assert_eq!(
                        active.state.car.owner.physical,
                        baseline.state.car.owner.physical
                    );
                    assert_eq!(
                        active.state.car.owner.damage_random_state,
                        baseline.state.car.owner.damage_random_state
                    );
                    assert_eq!(active.state.car.frame.body, baseline.state.car.frame.body);
                    assert_eq!(active.state.car.frame.view, baseline.state.car.frame.view);
                    assert_eq!(
                        active.state.car.frame.solver,
                        baseline.state.car.frame.solver
                    );
                    assert_eq!(
                        active.state.world.groups.bodies,
                        baseline.state.world.groups.bodies
                    );
                    assert_eq!(
                        active.state.world.groups.contacts,
                        baseline.state.world.groups.contacts
                    );
                    assert_eq!(active.last_body_contacts, baseline.last_body_contacts);
                    assert_eq!(active.ticks, baseline.ticks);
                    assert_eq!(active.cache.plane, baseline.cache.plane);
                    let mut expected = baseline.state.car.owner.vehicle.clone();
                    expected[0x4880 / 4..0xc580 / 4].copy_from_slice(&pool);
                    expected[0xc714 / 4..0xc720 / 4].fill(2);
                    for (i, value) in [0.125f32, 0.25, 0.5, 1.].into_iter().enumerate() {
                        expected[0xc7b4 / 4 + i] = value.to_bits();
                    }
                    assert_eq!(active.state.car.owner.vehicle, expected);
                }
                assert!(contacts > 0);
                println!(
                    "active_grid_source ticks={} contacts={} four_corner_scores_verified=1",
                    active.ticks, contacts
                );
                // A reached bad rest point rejects the entire runtime step,
                // including controls, RNG, frame buffers and metadata.
                byte(&mut active.state.car.owner.vehicle, 0xc7b1, 1);
                active.state.car.owner.vehicle[(0x4880 + 7 * 80 + 28) / 4] = f32::NAN.to_bits();
                let before = active.state.clone();
                let before_meta = (active.ticks, active.last_body_contacts, active.cache.plane);
                assert_eq!(
                    active
                        .tick_with_body_geometry(Controls::default(), &geometry)
                        .err()
                        .unwrap()
                        .0,
                    "invalid synchronized deformation node"
                );
                assert_eq!(active.state.car.owner.vehicle, before.car.owner.vehicle);
                assert_eq!(active.state.car.owner.physical, before.car.owner.physical);
                assert_eq!(
                    active.state.car.owner.damage_random_state,
                    before.car.owner.damage_random_state
                );
                assert_eq!(active.state.car.frame.body, before.car.frame.body);
                assert_eq!(active.state.car.frame.view, before.car.frame.view);
                assert_eq!(active.state.car.frame.solver, before.car.frame.solver);
                assert_eq!(
                    active.state.world.groups.contacts,
                    before.world.groups.contacts
                );
                assert_eq!(
                    (active.ticks, active.last_body_contacts, active.cache.plane),
                    before_meta
                );
                active.reset();
                baseline.reset();
                assert_eq!(
                    active.state.car.owner.vehicle,
                    baseline.state.car.owner.vehicle
                );
                assert_eq!(
                    active.state.car.owner.physical,
                    baseline.state.car.owner.physical
                );
                assert_eq!(
                    active.state.car.owner.damage_random_state,
                    baseline.state.car.owner.damage_random_state
                );
                assert_eq!(active.ticks, 0);
                assert_eq!(active.last_body_contacts, 0);
                assert!(active.body_events.is_some());
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    #[ignore = "requires purchased GRID sources and private body parameters"]
    fn source_driving_body_events() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let table = std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
        let data = std::fs::read(std::env::var_os("GRID_BODY_EVENT_PARAMETERS").unwrap()).unwrap();
        let parameters = crate::body_impact_parameters::read(&data).unwrap();
        let mut runtime = Runtime::lacetti(root, "slot_00", 200679).unwrap();
        let geometry = runtime.prepare_body_geometry(root, &table).unwrap();
        runtime.body_events = Some(parameters);
        let trace = |r: &Runtime| {
            let mut w = r.state.car.owner.vehicle.clone();
            w.extend(r.state.car.owner.physical);
            w.extend(r.state.car.frame.body);
            w.extend(r.state.car.frame.view);
            w.extend(r.state.car.frame.solver);
            for b in &r.state.world.groups.bodies {
                w.extend(b);
            }
            for c in &r.state.world.groups.contacts {
                w.extend(c);
            }
            w.extend(r.state.world.groups.world);
            for row in &r.state.world.manager.rows {
                w.extend(row);
            }
            for pair in &r.state.world.manager.workspace {
                w.extend(pair.iter().flatten().map(|v| v.to_bits()));
            }
            w.extend([
                r.state.car.owner.damage_random_state.unwrap_or(0),
                u32::from(r.state.car.owner.damage_random_state.is_some()),
                u32::from(r.cache.plane),
                r.ticks as u32,
                r.last_body_contacts as u32,
            ]);
            w
        };
        let initial = trace(&runtime);
        let mut expected = Vec::new();
        let mut contacts = 0;
        for _ in 0..600 {
            runtime
                .tick_with_body_geometry(
                    Controls {
                        throttle: 1.,
                        steering: 0.25,
                        ..Default::default()
                    },
                    &geometry,
                )
                .unwrap();
            contacts += runtime.last_body_contacts;
            expected.push(trace(&runtime));
        }
        assert!(contacts > 0);
        assert!(runtime.pose()[13] > -30.);
        println!(
            "body_event_source ticks={} contacts={} rng={:08x} suspension_gain={}",
            runtime.ticks,
            contacts,
            runtime.state.car.owner.damage_random_state.unwrap(),
            f32::from_bits(runtime.state.car.owner.vehicle[0x3d0 / 4])
        );
        runtime.reset();
        assert_eq!(trace(&runtime), initial);
        for frame in expected {
            runtime
                .tick_with_body_geometry(
                    Controls {
                        throttle: 1.,
                        steering: 0.25,
                        ..Default::default()
                    },
                    &geometry,
                )
                .unwrap();
            assert_eq!(trace(&runtime), frame);
        }
        runtime.reset();
        // Settle before an explicit valid-then-invalid geometric sequence.
        // A stationary car need not produce transient wheel records.
        for _ in 0..32 {
            runtime
                .tick_with_body_geometry(Controls::default(), &geometry)
                .unwrap();
        }
        runtime.state.car.owner.vehicle[0x1568 / 4] = 1.25f32.to_bits();
        runtime.state.car.frame.body[0x1568 / 4] = 1.25f32.to_bits();
        let before = trace(&runtime);
        let good_point: [f32; 4] = runtime.pose()[12..16].try_into().unwrap();
        let invalid = |_: &mut [grid_world::contact_groups::Body], _: usize| {
            let mut c = [0; 32];
            c[1] = 1;
            c[0xc / 4] = 0x01000000;
            c[0x1c / 4] = u32::MAX;
            c[0x20 / 4] = u32::MAX;
            c[0x24 / 4] = 1;
            c[0x28 / 4] = 1;
            c[0x2c / 4] = 2;
            c[0x3c / 4] = f32::MAX.to_bits();
            // A homogeneous W other than one is outside the event contract.
            c[0x50 / 4..0x60 / 4].copy_from_slice(&[0, 0, 0, 0.5f32.to_bits()]);
            c[0x60 / 4] = 1f32.to_bits();
            c[0x70 / 4] = 1 << 24;
            let mut good = c;
            good[0x50 / 4..0x60 / 4].copy_from_slice(&good_point.map(f32::to_bits));
            Ok(vec![good, c])
        };
        let error = runtime
            .tick_with_geometry(Controls::default(), invalid)
            .err()
            .unwrap();
        assert_eq!(error.0, "unsupported body impact context");
        assert_eq!(trace(&runtime), before);
        runtime.body_events = None;
        runtime
            .tick_with_geometry(Controls::default(), invalid)
            .unwrap();
        let first = &runtime.state.world.groups.contacts[0];
        println!(
            "late_event_probe first_point={:?} first_shapes={:?} first_impulse={} velocity={:?}",
            &first[0x50 / 4..0x60 / 4],
            &first[0x28 / 4..0x30 / 4],
            f32::from_bits(first[0x3c / 4]),
            runtime.velocity()
        );
        let early = grid_world::body_impact::calculate(
            &runtime.state.car.owner,
            grid_world::body_impact::Event {
                point: std::array::from_fn(|j| f32::from_bits(first[0x50 / 4 + j])),
                normal: std::array::from_fn(|j| f32::from_bits(first[0x60 / 4 + j])),
                impact: f32::from_bits(first[0x3c / 4]),
                parameter: DELTA,
                other_registration: 0,
                other_word: first[0x34 / 4],
            },
            0,
            parameters,
            &[None; 11],
            runtime.state.car.owner.damage_random_state.unwrap(),
        )
        .unwrap();
        assert_ne!(
            early.state.vehicle[0x1568 / 4],
            runtime.state.car.owner.vehicle[0x1568 / 4]
        );
        assert_eq!(early.state.vehicle[0x1568 / 4], 0);

        // 9E6660/9EF360 have no coordinate domain. The same car and impact
        // moved to Le Mans/Haruna distances give the same damage, up to the
        // f32 rounding of `point - translation`.
        let shift = [2928f32, 0., -2416., 0.];
        let point: [f32; 4] = std::array::from_fn(|j| f32::from_bits(first[0x50 / 4 + j]));
        let mut moved = runtime.state.car.owner.clone();
        for (word, offset) in moved.physical[0x200 / 4..0x20c / 4].iter_mut().zip(shift) {
            *word = (f32::from_bits(*word) + offset).to_bits();
        }
        let moved_point: [f32; 4] = std::array::from_fn(|j| point[j] + shift[j]);
        let rng = runtime.state.car.owner.damage_random_state.unwrap();
        let close = |a: &[u32], b: &[u32]| {
            a.len() == b.len()
                && a.iter()
                    .zip(b)
                    .all(|(&x, &y)| x == y || (f32::from_bits(x) - f32::from_bits(y)).abs() <= 1e-3)
        };
        for impact in [0., 1e4, 1e8] {
            let home = grid_world::collision_damage::calculate(
                &runtime.state.car.owner,
                point,
                impact,
                false,
                rng,
            )
            .unwrap();
            let away =
                grid_world::collision_damage::calculate(&moved, moved_point, impact, false, rng)
                    .unwrap();
            println!("far_damage impact={impact} applied={}", home.applied);
            assert_eq!(home.applied, away.applied);
            assert_eq!(home.random_state, away.random_state);
            assert!(close(&home.state.vehicle, &away.state.vehicle));
        }
        let event = |point| grid_world::body_impact::Event {
            point,
            normal: std::array::from_fn(|j| f32::from_bits(first[0x60 / 4 + j])),
            impact: f32::from_bits(first[0x3c / 4]),
            parameter: DELTA,
            other_registration: 0,
            other_word: first[0x34 / 4],
        };
        let away = grid_world::body_impact::calculate(
            &moved,
            event(moved_point),
            0,
            parameters,
            &[None; 11],
            rng,
        )
        .unwrap();
        assert_eq!(away.random_state, early.random_state);
        assert!(close(&away.state.vehicle, &early.state.vehicle));

        // A whole tick with a body event 3.8 km from the car no longer pauses.
        let far = |_: &mut [grid_world::contact_groups::Body], _: usize| {
            let mut c = invalid(&mut [], 0)?;
            c[1][0x50 / 4..0x60 / 4].copy_from_slice(&[2928f32, 0., -2416., 1.].map(f32::to_bits));
            Ok(c)
        };
        runtime.body_events = Some(parameters);
        runtime
            .tick_with_geometry(Controls::default(), far)
            .unwrap();
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
    fn source_driving_cqtc_boundary() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let table = std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
        let mut enabled = Runtime::lacetti(root, "slot_00", 200679).unwrap();
        let geometry = enabled.prepare_body_geometry(root, &table).unwrap();
        assert!(enabled.track.has_boundary());
        assert_eq!(enabled.track.shape_count(), 247);
        assert_eq!(enabled.state.shapes.len(), 248);
        assert_eq!(enabled.track.shape_name(246), Some("boundarylines.cqtc"));
        assert_eq!(enabled.track.shape_name(247), Some("objects.ens"));
        assert!(enabled.track.has_props());
        assert!(enabled.prepare_body_geometry(root, &table).is_err());
        enabled.reset();
        assert_eq!(enabled.state.shapes.len(), 248);
        assert_eq!(enabled.state.world.groups.contacts.len(), 64);
        let mut legacy = Runtime::lacetti(root, "slot_00", 200679).unwrap();
        for _ in 0..240 {
            let controls = Controls {
                throttle: 1.,
                ..Default::default()
            };
            legacy.tick(controls).unwrap();
            enabled
                .tick_with_body_geometry(controls, &geometry)
                .unwrap();
            assert_eq!(
                enabled.state.car.owner.physical,
                legacy.state.car.owner.physical
            );
            assert_eq!(enabled.last_body_contacts, 0);
        }
        enabled.reset();
        legacy.reset();
        legacy.reserve_geometry_pool(20).unwrap();
        let bytes = read(
            root,
            "tracks/europe/donington/route_0/boundarylines.cqtc",
            2 * 1024 * 1024,
        )
        .unwrap();
        let boundary = grid_formats::cqtc::read(&bytes).unwrap();
        let points = boundary.triangles[0]
            .indices
            .map(|i| boundary.positions[i as usize]);
        let center: [f32; 3] =
            std::array::from_fn(|j| ((points[0][j] + points[1][j]) + points[2][j]) / 3.);
        for runtime in [&mut enabled, &mut legacy] {
            let p = &mut runtime.state.car.owner.physical;
            for j in 0..3 {
                p[0x1c0 / 4 + j] = center[j].to_bits();
            }
            runtime.state.world.groups.bodies[0] = *p;
        }
        let hits = geometry
            .contacts(
                &enabled.track,
                &enabled.surfaces,
                &[enabled.state.car.owner.physical],
                20,
            )
            .unwrap();
        assert!(!hits.is_empty());
        assert!(hits.iter().any(|c| c[0x2c / 4] == 247));
        let one = geometry
            .contacts(
                &enabled.track,
                &enabled.surfaces,
                &[enabled.state.car.owner.physical],
                1,
            )
            .unwrap();
        assert_eq!(one, hits[..1]);
        let mut generated = 0;
        let mut responses = 0;
        for tick in 0..20 {
            legacy
                .tick_with_geometry(Controls::default(), |_, _| Ok(Vec::new()))
                .unwrap();
            enabled
                .tick_with_body_geometry(Controls::default(), &geometry)
                .unwrap_or_else(|e| panic!("source boundary tick {tick}: {e:?}"));
            generated += enabled.last_body_contacts;
            responses +=
                usize::from(enabled.state.car.owner.physical != legacy.state.car.owner.physical);
            assert!(enabled.velocity().iter().all(|v| v.is_finite()));
        }
        assert!(generated > 0 && responses > 0);
        eprintln!(
            "source_boundary ticks=20 contacts={generated} responses={responses} velocity={:?}",
            enabled.velocity()
        );
        enabled.reset();
        legacy.reset();
        let mut first_difference = None;
        let mut body_hits = 0;
        for tick in 0..600 {
            let controls = Controls {
                throttle: 1.,
                steering: 0.25,
                ..Default::default()
            };
            legacy.tick_with_body_geometry(controls, &geometry).unwrap();
            enabled
                .tick_with_body_geometry(controls, &geometry)
                .unwrap();
            body_hits += enabled.last_body_contacts;
            if first_difference.is_none()
                && enabled.state.car.owner.physical != legacy.state.car.owner.physical
            {
                assert!(enabled.last_body_contacts > 0);
                // Boundary sheet (247) or a visible route barrier/prop (248).
                assert!(
                    enabled
                        .state
                        .world
                        .groups
                        .contacts
                        .iter()
                        .any(|c| matches!(c[0x2c / 4], 247 | 248))
                );
                first_difference = Some(tick + 1);
            }
        }
        assert!(first_difference.is_some() && body_hits > 0);
        assert!(enabled.pose()[13] > -30. && legacy.pose()[13] < -100.);
        assert!(enabled.velocity().iter().all(|v| v.is_finite()));
        eprintln!(
            "source_boundary_turn ticks=600 first_difference={first_difference:?} body_contacts={body_hits} boundary_y={} packed_only_y={}",
            enabled.pose()[13],
            legacy.pose()[13]
        );
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
    fn dynamic_props_leave_an_untouched_drive_identical() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let root = Path::new(&root);
        let table = std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
        let mut plain = Runtime::lacetti(root, "slot_00", 200679).unwrap();
        let plain_geometry = plain.prepare_body_geometry(root, &table).unwrap();
        let mut armed = Runtime::lacetti(root, "slot_00", 200679).unwrap();
        let armed_geometry = armed.prepare_body_geometry(root, &table).unwrap();
        armed.enable_dynamic_props(root).unwrap();
        assert!(armed.enable_dynamic_props(root).is_err());
        let controls = Controls {
            throttle: 1.,
            ..Default::default()
        };
        for _ in 0..240 {
            plain
                .tick_with_body_geometry(controls, &plain_geometry)
                .unwrap();
            armed
                .tick_with_body_geometry(controls, &armed_geometry)
                .unwrap();
        }
        assert_eq!(
            armed.state.world.groups.bodies,
            plain.state.world.groups.bodies
        );
        assert_eq!(armed.state.car.owner.vehicle, plain.state.car.owner.vehicle);
        let props = armed.dynamic_props().unwrap();
        assert!(props.events.is_empty() && props.pieces.is_empty());
        armed.reset();
        assert_eq!(armed.state.world.groups.bodies.len(), 1);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
    fn source_driving_donington_body_geometry() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let root = Path::new(&root);
        let mut enabled = Runtime::lacetti(root, "slot_00", 200679).unwrap();
        let mut legacy = Runtime::lacetti(root, "slot_00", 200679).unwrap();
        let table = std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
        let ned = read(root, "cars/lac/lac.ned", 16 * 1024 * 1024).unwrap();
        let offset = std::array::from_fn(|j| {
            f32::from_bits(enabled.state.car.owner.vehicle[0x1630 / 4 + j])
        });
        let geometry = crate::body_geometry::BodyGeometry::read(&ned, offset, &table).unwrap();
        assert_eq!(enabled.track.shape_count(), 245);
        let body = enabled.state.car.owner.physical;
        let mut lowered = body;
        lowered[0x1c4 / 4] = (f32::from_bits(lowered[0x1c4 / 4]) - 0.5).to_bits();
        let full = geometry
            .contacts(&enabled.track, &enabled.surfaces, &[lowered], 64)
            .unwrap();
        assert!(!full.is_empty());
        assert!(full.len() <= 64);
        assert!(full.iter().all(|c| c[0x2c / 4] >= 2 && c[0x2c / 4] <= 246));
        let one = geometry
            .contacts(&enabled.track, &enabled.surfaces, &[lowered], 1)
            .unwrap();
        assert_eq!(one, full[..1]);
        assert!(
            geometry
                .contacts(&enabled.track, &enabled.surfaces, &[lowered], 0)
                .unwrap()
                .is_empty()
        );
        // The share is a world-pool bound now (sixteen per shape pair inside).
        assert!(
            geometry
                .contacts(
                    &enabled.track,
                    &enabled.surfaces,
                    &[lowered],
                    grid_world::MAX_WORLD_CONTACTS + 1
                )
                .is_err()
        );
        assert!(
            geometry
                .contacts(&enabled.track, &enabled.surfaces, &[], 20)
                .is_err()
        );
        eprintln!(
            "donington_lowered_body contacts={} shapes={:?} depths={:?}",
            full.len(),
            full.iter().map(|c| c[0x2c / 4]).collect::<Vec<_>>(),
            full.iter()
                .map(|c| f32::from_bits(c[0x38 / 4]))
                .collect::<Vec<_>>()
        );
        enabled.reserve_geometry_pool(20).unwrap();
        for tick in 0..240 {
            let controls = Controls {
                throttle: 1.,
                ..Default::default()
            };
            legacy.tick(controls).unwrap();
            enabled
                .tick_with_body_geometry(controls, &geometry)
                .unwrap_or_else(|e| panic!("Donington tick {tick}: {e:?}"));
        }
        assert_eq!(enabled.ticks, 240);
        assert!(enabled.velocity().iter().all(|v| v.is_finite()));
        assert!((enabled.pose()[12] - f32::from_bits(body[0x1c0 / 4])).abs() > 1.);
        eprintln!(
            "donington_body_drive ticks={} velocity={:?} unchanged_wheel_path={}",
            enabled.ticks,
            enabled.velocity(),
            enabled.state.car.owner.physical == legacy.state.car.owner.physical
        );
        enabled.reset();
        legacy.reset();
        for r in [&mut enabled, &mut legacy] {
            let p = &mut r.state.car.owner.physical;
            p[0x1c4 / 4] = (f32::from_bits(p[0x1c4 / 4]) - 0.12).to_bits();
            r.state.world.groups.bodies[0] = *p;
        }
        let mut generated = 0;
        let mut responses = 0;
        for tick in 0..20 {
            legacy.tick(Controls::default()).unwrap();
            enabled
                .tick_with_geometry(Controls::default(), |bodies, remaining| {
                    let contacts =
                        geometry.contacts(&legacy.track, &legacy.surfaces, bodies, remaining)?;
                    generated += contacts.len();
                    Ok(contacts)
                })
                .unwrap_or_else(|e| panic!("shallow road tick {tick}: {e:?}"));
            responses +=
                usize::from(enabled.state.car.owner.physical != legacy.state.car.owner.physical);
        }
        assert!(generated > 0);
        assert!(responses > 0);
        eprintln!(
            "donington_shallow_road ticks=20 contacts={generated} responses={responses} velocity={:?}",
            enabled.velocity()
        );
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
    fn source_driving_geometry_provider() {
        use grid_world::{
            collision_mesh, contact_groups, convex_distance, packed_contacts, packed_query,
        };
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let root = Path::new(&root);
        let mut legacy = Runtime::lacetti(root, "slot_00", 200679).unwrap();
        let mut enabled = Runtime::lacetti(root, "slot_00", 200679).unwrap();
        assert!(enabled.reserve_geometry_pool(65).is_err());
        enabled.reserve_geometry_pool(20).unwrap();
        let controls = Controls {
            throttle: 1.,
            steering: 0.03,
            ..Default::default()
        };
        let mut calls = 0;
        for _ in 0..240 {
            legacy.tick(controls).unwrap();
            enabled
                .tick_with_geometry(controls, |bodies, remaining| {
                    calls += 1;
                    assert_eq!(bodies.len(), 1);
                    assert!((16..=20).contains(&remaining));
                    Ok(Vec::new())
                })
                .unwrap();
            assert_eq!(
                legacy.state.car.owner.vehicle,
                enabled.state.car.owner.vehicle
            );
            assert_eq!(
                legacy.state.car.owner.physical,
                enabled.state.car.owner.physical
            );
            assert_eq!(
                legacy.state.world.groups.world,
                enabled.state.world.groups.world
            );
            assert_eq!(
                legacy.state.world.manager.workspace,
                enabled.state.world.manager.workspace
            );
        }
        assert_eq!(calls, 240);
        assert_eq!(enabled.state.world.groups.contacts.len(), 20);
        assert!(enabled.reserve_geometry_pool(20).is_err());
        let before = enabled.state.clone();
        let cache = enabled.cache.plane;
        assert!(
            enabled
                .tick_with_geometry(controls, |_, _| Err(Error("geometry failure")))
                .is_err()
        );
        assert_eq!(enabled.ticks, 240);
        assert_eq!(enabled.cache.plane, cache);
        assert_eq!(enabled.state.car.owner.vehicle, before.car.owner.vehicle);
        assert_eq!(enabled.state.car.owner.physical, before.car.owner.physical);
        assert_eq!(
            enabled.state.world.groups.contacts,
            before.world.groups.contacts
        );
        assert_eq!(enabled.state.world.groups.world, before.world.groups.world);
        assert_eq!(enabled.state.world.manager.rows, before.world.manager.rows);
        assert_eq!(
            enabled.state.world.manager.workspace,
            before.world.manager.workspace
        );
        enabled.reset();
        assert_eq!(enabled.ticks, 0);
        assert_eq!(enabled.state.world.groups.contacts.len(), 20);

        let ned_bytes = read(root, "cars/lac/lac.ned", 16 * 1024 * 1024).unwrap();
        let ned = binxml::read(&ned_bytes).unwrap();
        let raw = car_collision::read(&ned).unwrap();
        let points: Vec<_> = raw.points.iter().map(|p| [p[0], p[1], p[2], 1.]).collect();
        let offset: [f32; 4] = std::array::from_fn(|j| {
            f32::from_bits(enabled.state.car.owner.vehicle[0x1630 / 4 + j])
        });
        let mesh = collision_mesh::build_from_source(
            &points,
            &raw.triangles,
            offset,
            [raw.bounds.min[1], raw.bounds.max[1]],
            0,
        )
        .unwrap();
        let span = mesh.bytes().len() as u32;
        let shape = mesh
            .initialize_shape(
                collision_mesh::ShapeSettings {
                    span,
                    scalar220: 0.,
                    layer: 0,
                    flag_c0: false,
                },
                0,
            )
            .unwrap();
        let height = offset[1] + (raw.bounds.max[1] + raw.bounds.min[1]) * 0.5;
        let local = [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., height, 0., 1.],
        ];
        let table = std::fs::read(
            std::env::var_os("GRID_CONTACT_DIRECTIONS").expect("private directions required"),
        )
        .unwrap();
        assert_eq!(table.len(), 712);
        assert_eq!(&table[..8], b"CD01\x2c\0\0\0");
        let directions = std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                f32::from_le_bytes(
                    table[8 + i * 16 + j * 4..12 + i * 16 + j * 4]
                        .try_into()
                        .unwrap(),
                )
            })
        });
        let publish = |body: &contact_groups::Body| {
            let matrix = |o: usize| {
                std::array::from_fn(|i| {
                    std::array::from_fn(|j| f32::from_bits(body[o / 4 + i * 4 + j]))
                })
            };
            grid_world::physical_pose::calculate(&grid_world::physical_pose::Input {
                basis: matrix(0x190),
                offset: std::array::from_fn(|j| f32::from_bits(body[0x230 / 4 + j])),
                local_matrices: [matrix(0x90), matrix(0x150)],
                shapes: vec![Some(local)],
                group_head: false,
                linear: [0.; 4],
                angular: [0.; 4],
            })
        };
        let center = enabled.pose();
        let bounds = shape
            .mesh()
            .world_bounds(
                publish(&enabled.state.car.owner.physical).unwrap().shapes[0],
                0.,
            )
            .unwrap();
        // Bitwise trajectory projection: complete car/physical/frame buffers,
        // contact pools, solver rows/workspace, both RNGs and runtime metadata.
        let trace = |runtime: &Runtime| {
            let mut words = runtime.state.car.owner.vehicle.clone();
            words.extend(runtime.state.car.owner.physical);
            words.extend(runtime.state.car.frame.body);
            words.extend(runtime.state.car.frame.view);
            words.extend(runtime.state.car.frame.solver);
            for body in &runtime.state.world.groups.bodies {
                words.extend(body);
            }
            for contact in &runtime.state.world.groups.contacts {
                words.extend(contact);
            }
            words.extend(runtime.state.world.groups.world);
            for row in &runtime.state.world.manager.rows {
                words.extend(row);
            }
            for pair in &runtime.state.world.manager.workspace {
                words.extend(pair.iter().flatten().map(|v| v.to_bits()));
            }
            words.push(u32::from(
                runtime.state.car.owner.damage_random_state.is_some(),
            ));
            words.push(runtime.state.car.owner.damage_random_state.unwrap_or(0));
            words.push(u32::from(runtime.cache.plane));
            words.push(runtime.ticks as u32);
            words.push(runtime.last_body_contacts as u32);
            words
        };
        // Authored walls, not recovered Donington barrier placements. The
        // center-plane case previously reached the collision-damage guard.
        for (label, x) in [("shallow", bounds[1][0] - 0.01), ("deep", center[12])] {
            enabled.reset();
            let damage_rng_before = enabled.state.car.owner.damage_random_state;
            let z = center[14];
            let y = center[13];
            let vertices = [
                [x, y - 10., z - 10.],
                [x, y + 10., z - 10.],
                [x, y, z + 10.],
            ];
            let leaves = std::collections::BTreeMap::from([(0, vec![0])]);
            let track = packed_query::Mesh {
                bounds: [x - 20., y - 20., z - 20., x + 20., y + 20., z + 20.],
                vertices: &vertices,
                triangles: &[[0, 1, 2]],
                nodes: &[0x8000],
                leaves: &leaves,
            };
            let mut baseline = Vec::new();
            for pass in 0..if label == "deep" { 3 } else { 1 } {
                enabled.reset();
                let legacy_guard = pass == 2;
                if legacy_guard {
                    enabled.state.car.owner.damage_random_state = None;
                }
                let mut total = 0;
                let mut trajectory = Vec::new();
                let mut rejected = false;
                let mut responses = 0;
                legacy.reset();
                for tick in 0..20 {
                    legacy.tick(Controls::default()).unwrap();
                    let before_tick = trace(&enabled);
                    let result =
                        enabled.tick_with_geometry(Controls::default(), |bodies, remaining| {
                            let published = publish(&bodies[0])?;
                            let generated = packed_contacts::calculate(
                                shape.mesh(),
                                published.shapes[0],
                                &track,
                                &[0],
                                &packed_contacts::Settings {
                                    query: convex_distance::ContactSettings {
                                        margin: 0.,
                                        directions,
                                    },
                                    shape_margin: 0.,
                                    contact_capacity: remaining.min(16),
                                    cache_enabled: true,
                                    candidate_capacity: 4096,
                                    sheet_mask: 0,
                                    correction_bounds: [f32::MAX; 4],
                                },
                            )?;
                            eprintln!(
                                "geometry_probe tick={tick} contacts={} depths={:?}",
                                generated.contacts.len(),
                                generated
                                    .contacts
                                    .iter()
                                    .map(|c| c.penetration)
                                    .collect::<Vec<_>>()
                            );
                            total += generated.contacts.len();
                            Ok(generated
                                .contacts
                                .into_iter()
                                .map(|contact| {
                                    let mut c: contact_groups::Contact = [0; 32];
                                    c[1] = 1;
                                    c[0xc / 4] = 0x01000000;
                                    c[0x1c / 4] = u32::MAX;
                                    c[0x20 / 4] = u32::MAX;
                                    c[0x24 / 4] = 1;
                                    c[0x28 / 4] = 1;
                                    c[0x2c / 4] = 2;
                                    c[0x38 / 4] = contact.penetration.to_bits();
                                    c[0x3c / 4] = f32::MAX.to_bits();
                                    c[0x50 / 4..0x60 / 4]
                                        .copy_from_slice(&contact.point.map(f32::to_bits));
                                    c[0x60 / 4..0x70 / 4]
                                        .copy_from_slice(&contact.normal.map(f32::to_bits));
                                    c[0x70 / 4] = (u32::from(contact.flipped) << 8) | (1 << 24);
                                    c
                                })
                                .collect())
                        });
                    if let Err(error) = result {
                        assert!(legacy_guard, "wall tick {tick}: {error:?}");
                        assert_eq!(error.0, "unsupported handling collision damage");
                        assert_eq!(trace(&enabled), before_tick);
                        eprintln!(
                            "heavy_wall_legacy_guard attempted_tick={} unchanged=true",
                            tick + 1
                        );
                        rejected = true;
                        break;
                    }
                    assert!(!legacy_guard || enabled.state.car.owner.damage_random_state.is_none());
                    trajectory.push(trace(&enabled));
                    responses += usize::from(
                        enabled.state.car.owner.physical != legacy.state.car.owner.physical,
                    );
                }
                if legacy_guard {
                    assert!(
                        rejected,
                        "paired None control must reach the old damage guard"
                    );
                    continue;
                }
                assert!(!rejected);
                if pass == 0 {
                    baseline = trajectory;
                } else {
                    assert_eq!(trajectory, baseline);
                    eprintln!("heavy_wall_reset_replay ticks=20 bitwise_equal=true");
                }
                assert!(total > 0);
                assert!(responses > 0);
                assert_eq!(enabled.ticks, 20);
                assert!(enabled.velocity().iter().all(|x| x.is_finite()));
                eprintln!(
                    "fresh_source_mesh_wall label={label} ticks=20 contacts={total} response_ticks={responses} damage_rng_changed={} velocity={:?}",
                    enabled.state.car.owner.damage_random_state != damage_rng_before,
                    enabled.velocity()
                );
                if label == "deep" {
                    assert_ne!(
                        enabled.state.car.owner.damage_random_state,
                        damage_rng_before
                    );
                }
            }
        }
    }

    // Deliberately opt-in: purchased resources never become test fixtures.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to the purchased installation"]
    fn source_driving_reset_controls_and_rollback() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let mut runtime = Runtime::lacetti(Path::new(&root), "slot_00", 200679).unwrap();
        let initial_damage_rng = runtime.state.car.owner.damage_random_state;
        let expected_seed = (0..24).fold(200679_u32, |seed, _| {
            seed.wrapping_mul(0x41c64e6d).wrapping_add(0x40584c0f)
        });
        assert_eq!(initial_damage_rng, Some(expected_seed));
        let initial = runtime.state.car.owner.vehicle.clone();
        let controls = Controls {
            throttle: 1.,
            steering: 0.03,
            ..Default::default()
        };
        for _ in 0..240 {
            runtime.tick(controls).unwrap();
        }
        let car = runtime.state.car.owner.vehicle.clone();
        let physical = runtime.state.car.owner.physical;
        let cache = runtime.cache.plane;
        let damage_random_state = runtime.state.car.owner.damage_random_state;
        assert!(damage_random_state.is_some());
        assert_ne!(car, initial);
        assert!(
            runtime
                .tick(Controls {
                    throttle: f32::NAN,
                    ..controls
                })
                .is_err()
        );
        assert_eq!(runtime.ticks, 240);
        assert_eq!(runtime.cache.plane, cache);
        assert_eq!(runtime.state.car.owner.vehicle, car);
        assert_eq!(runtime.state.car.owner.physical, physical);
        assert_eq!(
            runtime.state.car.owner.damage_random_state,
            damage_random_state
        );
        runtime.reset();
        assert_eq!(runtime.ticks, 0);
        assert_eq!(runtime.state.car.owner.vehicle, initial);
        assert_eq!(
            runtime.state.car.owner.damage_random_state,
            initial_damage_rng
        );
        for _ in 0..240 {
            runtime.tick(controls).unwrap();
        }
        assert_eq!(runtime.state.car.owner.vehicle, car);
        assert_eq!(runtime.state.car.owner.physical, physical);
        assert_eq!(
            runtime.state.car.owner.damage_random_state,
            damage_random_state
        );
        let before = runtime.velocity();
        for _ in 0..120 {
            runtime
                .tick(Controls {
                    brake: 1.,
                    ..Default::default()
                })
                .unwrap();
        }
        assert!(
            runtime.velocity()[..3].iter().map(|v| v * v).sum::<f32>()
                < before[..3].iter().map(|v| v * v).sum::<f32>()
        );
        for _ in 0..120 {
            runtime
                .tick(Controls {
                    handbrake: true,
                    ..Default::default()
                })
                .unwrap();
        }
        assert_eq!(runtime.state.car.owner.vehicle[0xe38 / 4] & 255, 1);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to the purchased installation"]
    fn source_driving_handbrake_and_reverse() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let mut runtime = Runtime::lacetti(Path::new(&root), "slot_00", 200679).unwrap();
        // About 10 m/s from the idling grid start (formerly 120 ticks from a
        // zero-RPM crank), so the 60-tick handbrake bound keeps its meaning.
        const POWERED: u64 = 72;
        for _ in 0..POWERED {
            runtime
                .tick(Controls {
                    throttle: 1.,
                    ..Default::default()
                })
                .unwrap();
        }
        let powered = runtime.state.clone();
        let cache = runtime.cache;
        let mut speeds = Vec::new();
        for handbrake in [false, true] {
            runtime.state = powered.clone();
            runtime.cache = cache;
            runtime.ticks = POWERED;
            for i in 0..60 {
                runtime
                    .tick(Controls {
                        handbrake,
                        ..Default::default()
                    })
                    .unwrap();
                if i == 0 || i == 59 {
                    let b = &runtime.state.car.owner.vehicle;
                    let f = |o: usize| f32::from_bits(b[o / 4]);
                    println!(
                        "handbrake_probe hand={handbrake} tick={} velocity={:?} derived={} brake={:?} rear_spin={:?} factors={:?}",
                        runtime.ticks,
                        runtime.velocity(),
                        b[0x1640 / 4] & 255,
                        (0..4)
                            .map(|i| f32::from_bits(runtime.state.car.frame.view[0x28 / 4 + i]))
                            .collect::<Vec<_>>(),
                        [f(0x1a94), f(0x1be4)],
                        [
                            f(0x1260),
                            f(0x125c),
                            f(0x1358),
                            f(0x1ef4),
                            f(0x1a98),
                            f(0x1d38)
                        ]
                    );
                    if handbrake && i == 0 {
                        let brakes = &runtime.state.car.frame.view[0x28 / 4..0x38 / 4];
                        assert!(brakes[..2].iter().all(|w| f32::from_bits(*w) > 1000.));
                        assert!(brakes[2..].iter().all(|w| f32::from_bits(*w) == 0.));
                    }
                }
            }
            speeds.push(
                runtime.velocity()[..3]
                    .iter()
                    .map(|v| v * v)
                    .sum::<f32>()
                    .sqrt(),
            );
        }
        assert!(
            speeds[0] > 5. && speeds[1] < 0.1,
            "coast versus handbrake: {speeds:?}"
        );
        runtime.state = powered;
        runtime.cache = cache;
        runtime.ticks = POWERED;
        runtime
            .tick(Controls {
                brake: 1.,
                ..Default::default()
            })
            .unwrap();
        assert_ne!(
            runtime.state.car.owner.vehicle[0x196c / 4],
            9,
            "braking at forward speed must not immediately select reverse"
        );
        runtime.reset();
        for _ in 0..120 {
            runtime
                .tick(Controls {
                    brake: 1.,
                    ..Default::default()
                })
                .unwrap();
        }
        println!(
            "reverse_probe gear={} velocity={:?}",
            runtime.state.car.owner.vehicle[0x196c / 4],
            runtime.velocity()
        );
        let signed_speed = |r: &Runtime| {
            let p = r.pose();
            let v = r.velocity();
            v[0] * p[8] + v[1] * p[9] + v[2] * p[10]
        };
        assert_eq!(runtime.state.car.owner.vehicle[0x196c / 4], 9);
        assert!(signed_speed(&runtime) < -1.);
        for _ in 0..240 {
            runtime
                .tick(Controls {
                    throttle: 1.,
                    ..Default::default()
                })
                .unwrap();
        }
        assert_ne!(runtime.state.car.owner.vehicle[0x196c / 4], 9);
        assert!(signed_speed(&runtime) > 1.);
    }
}
