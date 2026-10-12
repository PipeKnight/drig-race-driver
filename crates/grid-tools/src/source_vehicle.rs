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

pub struct Runtime {
    pub state: vehicle_world::State,
    initial: vehicle_world::State,
    initial_cache: rectangle_cull::Cache,
    track: Track,
    surfaces: Surfaces,
    cache: rectangle_cull::Cache,
    pair: [usize; 2],
    pub ticks: u64,
}

#[derive(Clone, Copy, Default)]
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
        byte(&mut owner, 0x1972, 0);
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
        })
    }

    /// Stage controls, queries and the complete world. A failed tick preserves
    /// all authoritative state, including the query cache and tick counter.
    pub fn tick(&mut self, controls: Controls) -> Result<(), Error> {
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
        let output = vehicle_world::calculate(
            &staged,
            self.pair,
            GRAVITY,
            None,
            |_, ray| {
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
            },
            |token| self.surfaces.joined(token),
        )?;
        self.state = output.state;
        self.cache = cache;
        self.ticks += 1;
        Ok(())
    }
    pub fn reset(&mut self) {
        self.state = self.initial.clone();
        self.cache = self.initial_cache;
        self.ticks = 0;
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
}
