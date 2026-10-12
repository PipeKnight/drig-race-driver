//! Source-only Lacetti/Donington serial driving prototype. No saved native state.
//! The manager's single shared physical material and serial scheduling are explicit
//! prototype choices; full body collisions, damage events and racing remain absent.
use crate::{contact_surfaces::Surfaces, source_physical, standard_car, track::Track};
use grid_formats::{
    binxml, car_collision, grid, handling_damage, jpk, pssg, tuning, vehicle, vehicle_properties,
};
use grid_world::{
    Error, grouped_physical_frame, impact_frame, rectangle_cull, spawn, suspension_prepare,
    vehicle_bridge, vehicle_force, vehicle_manager, vehicle_publication, vehicle_world,
};
use std::path::Path;

pub const DELTA: f32 = 0.016;
pub const GRAVITY: f32 = f32::from_bits(0x411c_f5c3);

type GeometryProvider<'a> = dyn FnMut(
        &[grid_world::contact_groups::Body],
        usize,
    ) -> Result<Vec<grid_world::contact_groups::Contact>, Error>
    + 'a;

enum Geometry<'a> {
    Callback(&'a mut GeometryProvider<'a>),
    Source(&'a crate::body_geometry::BodyGeometry),
}

pub struct Runtime {
    pub state: vehicle_world::State,
    initial: vehicle_world::State,
    initial_cache: rectangle_cull::Cache,
    track: Track,
    surfaces: Surfaces,
    cache: rectangle_cull::Cache,
    pair: [usize; 2],
    pub ticks: u64,
    /// Fresh body records from the last successful tick; diagnostics only.
    /// Failure preserves this count and reset clears it.
    pub last_body_contacts: usize,
}

#[derive(Clone, Copy, Default, PartialEq)]
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
}

fn read(root: &Path, relative: &str, bound: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(root.join(relative))?
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

impl Runtime {
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
            }
        })
    }

    /// Load purchased source files directly. The selected assisted automatic
    /// preferences/readiness and serial manager configuration are supplied here,
    /// rather than pretending to execute the original resource/preferences layer.
    pub fn lacetti(root: &Path, slot: &str, seed: u32) -> Result<Self, Box<dyn std::error::Error>> {
        if !slot.starts_with("slot_")
            || slot.len() > 32
            || !slot.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
        {
            return Err("invalid grid slot".into());
        }
        let csv_bytes = read(root, "cars/lac/lac.csv", 4 * 1024 * 1024)?;
        let csv = tuning::read(&csv_bytes)?;
        if csv.rows.len() != 1 {
            return Err("expected one Lacetti tuning row".into());
        }
        let properties = vehicle_properties::read(&csv.rows[0], 0x2008)?;
        let ned_bytes = read(root, "cars/lac/lac.ned", 16 * 1024 * 1024)?;
        let ned = binxml::read(&ned_bytes)?;
        let geometry = vehicle::read_wheel_geometry(&ned)?;
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
        let mut car = standard_car::initialize_with_source_prelude(
            &properties,
            geometry,
            widths,
            GRAVITY,
            seed,
        )?;
        car.configure_suspension(geometry)?;
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
        let track = Track::read(&read(
            root,
            "tracks/europe/donington/route_0/track.jpk",
            256 * 1024 * 1024,
        )?)?;
        let surfaces = Surfaces::read(&read(root, "surface_materials.xml", 16 * 1024 * 1024)?)?;
        let grid_bytes = read(
            root,
            "tracks/europe/donington/route_0/grids.pssg",
            256 * 1024 * 1024,
        )?;
        let markers = pssg::read_tree(&grid_bytes)?;
        let marker = grid::read(&markers, slot.as_bytes())?;
        let shape = car_collision::read(&ned)?;
        let front_extent = shape.bounds.max[2] + f32::from_bits(owner[0x1638 / 4]);
        let center = spawn::reference_front_aligned_center(marker.corners, front_extent, 0.)?;
        let heading = std::array::from_fn(|i| marker.corners[0][i] - marker.corners[2][i]);
        let wheels = std::array::from_fn(|i| {
            let w = (0x1990 + i * 0x150) / 4;
            spawn::Wheel {
                position: std::array::from_fn(|j| f32::from_bits(owner[w + j])),
                radius: f32::from_bits(owner[w + 0xe0 / 4]),
                vertical_displacement: f32::from_bits(owner[w + 0x40 / 4]),
            }
        });
        let mut cache = rectangle_cull::Cache::default();
        let pose = spawn::align_with_normalization(
            center,
            heading,
            wheels,
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
        let manager = grouped_physical_frame::Manager {
            parameters: [DELTA, 1., 1., 1., 1., 0.01, 0.2, 0.125, 0.8, 0.2, 0.2],
            gravity: [0., -GRAVITY, 0., 0.],
            correction: Some(impact_frame::Correction {
                iterations: 4,
                softness: 0.01,
                scale: 1.,
                threshold: 0.,
                relaxation: 0.8,
            }),
            // Same explicit selected material as the whole-world comparison.
            materials: vec![[
                0., 0.3, 0.4, 0.01, 0.5, 0.2, 2., 0.5, 3., 0.75, 20., 0., 0.125,
            ]],
            rows: vec![[0; 48]; 16],
            capacity: 16,
            iterations: 4,
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
                    vehicle: owner,
                    physical,
                },
            },
            manager,
            track.shape_count(),
        )?;
        Ok(Self {
            initial: state.clone(),
            initial_cache: cache,
            state,
            track,
            surfaces,
            cache,
            pair,
            ticks: 0,
            last_body_contacts: 0,
        })
    }

    /// Stage controls, queries and the complete world. A failed tick preserves
    /// all authoritative state, including the query cache and tick counter.
    pub fn tick(&mut self, controls: Controls) -> Result<(), Error> {
        self.tick_owned(controls, None)
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
        self.state.world.groups.contacts.resize(capacity, [0; 32]);
        self.state.world.manager.rows.resize(capacity * 4, [0; 48]);
        self.state.world.manager.capacity = capacity * 4;
        self.initial = self.state.clone();
        Ok(())
    }

    /// Prepare the selected source body shape and reserve four wheel plus sixteen
    /// geometric slots. Sampling remains an explicit locally prepared input.
    pub fn prepare_body_geometry(
        &mut self,
        root: &Path,
        table: &[u8],
    ) -> Result<crate::body_geometry::BodyGeometry, Box<dyn std::error::Error>> {
        let ned = read(root, "cars/lac/lac.ned", 16 * 1024 * 1024)?;
        let offset =
            std::array::from_fn(|j| f32::from_bits(self.state.car.owner.vehicle[0x1630 / 4 + j]));
        let geometry = crate::body_geometry::BodyGeometry::read(&ned, offset, table)?;
        let boundary = crate::cqtc_geometry::Geometry::read(&read(
            root,
            "tracks/europe/donington/route_0/boundarylines.cqtc",
            2 * 1024 * 1024,
        )?)?;
        if self.track.has_boundary() || self.state.shapes.len() != self.track.shape_count() + 1 {
            return Err("body boundary already prepared or shape registration differs".into());
        }
        self.reserve_geometry_pool(20)?;
        self.track.attach_boundary(boundary);
        self.state.shapes.push(grid_world::contact_groups::Shape {
            angular: true,
            ..Default::default()
        });
        self.initial = self.state.clone();
        Ok(geometry)
    }

    pub fn tick_with_geometry(
        &mut self,
        controls: Controls,
        mut geometry: impl FnMut(
            &[grid_world::contact_groups::Body],
            usize,
        ) -> Result<Vec<grid_world::contact_groups::Contact>, Error>,
    ) -> Result<(), Error> {
        self.tick_owned(controls, Some(Geometry::Callback(&mut geometry)))
    }

    /// Experimental actual-track body geometry. Uses the existing selected
    /// shared solver material; heavy-impact damage remains explicitly guarded.
    pub fn tick_with_body_geometry(
        &mut self,
        controls: Controls,
        geometry: &crate::body_geometry::BodyGeometry,
    ) -> Result<(), Error> {
        self.tick_owned(controls, Some(Geometry::Source(geometry)))
    }

    fn tick_owned(
        &mut self,
        controls: Controls,
        geometry: Option<Geometry<'_>>,
    ) -> Result<(), Error> {
        if [controls.throttle, controls.brake]
            .iter()
            .any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
            || !controls.steering.is_finite()
            || !(-1. ..=1.).contains(&controls.steering)
        {
            return Err(Error("invalid driving controls"));
        }
        let mut staged = self.state.clone();
        for (o, v) in [
            (0xe1c, controls.throttle),
            (0xe20, controls.brake),
            (0xe3c, controls.throttle),
            (0xe40, controls.brake),
            (0xe30, controls.steering),
        ] {
            staged.car.owner.vehicle[o / 4] = v.to_bits();
            staged.car.frame.body[o / 4] = v.to_bits();
        }
        byte(
            &mut staged.car.owner.vehicle,
            0xe38,
            u8::from(controls.handbrake),
        );
        byte(
            &mut staged.car.frame.body,
            0xe38,
            u8::from(controls.handbrake),
        );
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
        let output = match geometry {
            Some(Geometry::Callback(geometry)) => vehicle_world::calculate_with_geometry(
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
            )?,
            Some(Geometry::Source(geometry)) => vehicle_world::calculate_with_geometry(
                &staged,
                self.pair,
                GRAVITY,
                None,
                query,
                material,
                |bodies, remaining| {
                    let contacts =
                        geometry.contacts(&self.track, &self.surfaces, bodies, remaining)?;
                    body_contacts = contacts.len();
                    Ok(contacts)
                },
            )?,
            None => vehicle_world::calculate(&staged, self.pair, GRAVITY, None, query, material)?,
        };
        self.state = output.state;
        self.cache = cache;
        self.ticks += 1;
        self.last_body_contacts = body_contacts;
        Ok(())
    }
    pub fn reset(&mut self) {
        self.state = self.initial.clone();
        self.cache = self.initial_cache;
        self.ticks = 0;
        self.last_body_contacts = 0;
    }
    pub fn pose(&self) -> [f32; 16] {
        std::array::from_fn(|i| f32::from_bits(self.state.car.owner.physical[0x1d0 / 4 + i]))
    }
    pub fn velocity(&self) -> [f32; 4] {
        std::array::from_fn(|i| f32::from_bits(self.state.car.owner.physical[0x220 / 4 + i]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires GRID_GAME_ROOT and private GRID_CONTACT_DIRECTIONS"]
    fn source_driving_cqtc_boundary() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        let table = std::fs::read(std::env::var_os("GRID_CONTACT_DIRECTIONS").unwrap()).unwrap();
        let mut enabled = Runtime::lacetti(root, "slot_00", 200679).unwrap();
        let geometry = enabled.prepare_body_geometry(root, &table).unwrap();
        assert!(enabled.track.has_boundary());
        assert_eq!(enabled.track.shape_count(), 246);
        assert_eq!(enabled.state.shapes.len(), 247);
        assert_eq!(enabled.track.shape_name(246), Some("boundarylines.cqtc"));
        assert!(enabled.prepare_body_geometry(root, &table).is_err());
        enabled.reset();
        assert_eq!(enabled.state.shapes.len(), 247);
        assert_eq!(enabled.state.world.groups.contacts.len(), 20);
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
                assert!(
                    enabled
                        .state
                        .world
                        .groups
                        .contacts
                        .iter()
                        .any(|c| c[0x2c / 4] == 247)
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
        assert!(
            geometry
                .contacts(&enabled.track, &enabled.surfaces, &[lowered], 65)
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
        // Authored shallow wall, not a recovered Donington barrier placement.
        let x = bounds[1][0] - 0.01;
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
        let mut total = 0;
        let mut responses = 0;
        legacy.reset();
        for tick in 0..20 {
            legacy.tick(Controls::default()).unwrap();
            enabled
                .tick_with_geometry(Controls::default(), |bodies, remaining| {
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
                            c[0x50 / 4..0x60 / 4].copy_from_slice(&contact.point.map(f32::to_bits));
                            c[0x60 / 4..0x70 / 4]
                                .copy_from_slice(&contact.normal.map(f32::to_bits));
                            c[0x70 / 4] = (u32::from(contact.flipped) << 8) | (1 << 24);
                            c
                        })
                        .collect())
                })
                .unwrap_or_else(|e| panic!("wall tick {tick}: {e:?}"));
            responses +=
                usize::from(enabled.state.car.owner.physical != legacy.state.car.owner.physical);
        }
        assert!(total > 0);
        assert!(responses > 0);
        assert_eq!(enabled.ticks, 20);
        assert!(enabled.velocity().iter().all(|x| x.is_finite()));
        eprintln!(
            "fresh_source_mesh_wall ticks=20 contacts={total} response_ticks={responses} velocity={:?}",
            enabled.velocity()
        );
    }

    // Deliberately opt-in: purchased resources never become test fixtures.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to the purchased installation"]
    fn source_driving_reset_controls_and_rollback() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let mut runtime = Runtime::lacetti(Path::new(&root), "slot_00", 200679).unwrap();
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
        runtime.reset();
        assert_eq!(runtime.ticks, 0);
        assert_eq!(runtime.state.car.owner.vehicle, initial);
        for _ in 0..240 {
            runtime.tick(controls).unwrap();
        }
        assert_eq!(runtime.state.car.owner.vehicle, car);
        assert_eq!(runtime.state.car.owner.physical, physical);
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
        for _ in 0..120 {
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
            runtime.ticks = 120;
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
        runtime.ticks = 120;
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
