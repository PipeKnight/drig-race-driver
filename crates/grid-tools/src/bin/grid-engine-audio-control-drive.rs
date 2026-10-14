//! Source driving feeds the verified control pipeline, with a paired physics run.
//! Playback lengths here are explicit PCM fixture choices, not native factory proof.
use grid_tools::{
    engine_audio_control::{self, CONTROLLER_BYTES, VehicleEngine},
    source_vehicle::{Controls, DELTA, Runtime},
};
use std::{fs, io::Read, io::Write, path::Path};

fn put(controller: &mut [u8], at: usize, value: u32) {
    controller[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-engine-audio-control-drive GAME_ROOT local/NEW_DIRECTORY".into());
    }
    let root = Path::new(&args[0]);
    let out = Path::new(&args[1]);
    if out.exists()
        || !out
            .parent()
            .ok_or("output parent")?
            .canonicalize()?
            .starts_with(Path::new("local").canonicalize()?)
    {
        return Err("new private output required".into());
    }
    let mut ini = String::new();
    fs::File::open(root.join("audio/cars/lacengine.ini"))?
        .take(1024 * 1024 + 1)
        .read_to_string(&mut ini)?;
    if ini.len() > 1024 * 1024 {
        return Err("engine ini bound".into());
    }
    let curves = engine_audio_control::read_curves(&ini)?;
    let mut car = Runtime::lacetti(root, "slot_00", 200679)?;
    let mut reference = Runtime::lacetti(root, "slot_00", 200679)?;
    let mut controller = [0x6b; CONTROLLER_BYTES];
    controller[0x14c..0x14f].fill(0);
    for (at, value) in [
        (0, 1_f32),
        (0x28, 0.),
        (0x38, 0.),
        (0xb0, 1.),
        (0xb4, 0.),
        (0xc8, 0.),
        (0x30c, 0.),
    ] {
        put(&mut controller, at, value.to_bits());
    }
    for slot in 0..12 {
        put(&mut controller, 0x230 + slot * 4, 0);
        put(&mut controller, 0x260 + slot * 4, 1_f32.to_bits());
        put(&mut controller, 0x290 + slot * 4, 32000);
        put(
            &mut controller,
            0x2c0 + slot * 4,
            if slot % 3 == 0 { 319294 } else { 477038 },
        );
    }
    let mut request = Vec::new();
    request.extend_from_slice(&0x31304345_u32.to_le_bytes());
    request.extend_from_slice(&1000_u32.to_le_bytes());
    for value in curves {
        request.extend_from_slice(&value.to_bits().to_le_bytes());
    }
    let mut output = request[..8].to_vec();
    for tick in 0_u32..1000 {
        let controls = Controls {
            throttle: if (125..625).contains(&tick) { 1. } else { 0. },
            brake: if tick >= 750 { 1. } else { 0. },
            ..Default::default()
        };
        car.tick(controls)?;
        reference.tick(controls)?;
        let before = car.state.car.owner.vehicle.clone();
        let body = car.state.car.owner.physical;
        let engine = VehicleEngine {
            speed: f32::from_bits(before[0x1894 / 4]),
            maximum: f32::from_bits(before[0x17a4 / 4]),
            idle: f32::from_bits(before[0x17a8 / 4]),
            delta: f64::from(DELTA),
        };
        request.extend_from_slice(&tick.to_le_bytes());
        request.extend_from_slice(&controller);
        for value in [engine.speed, engine.maximum, engine.idle] {
            request.extend_from_slice(&value.to_bits().to_le_bytes());
        }
        request.extend_from_slice(&engine.delta.to_le_bytes());
        let queries = engine_audio_control::update(&mut controller, engine, &curves)?;
        output.extend_from_slice(&tick.to_le_bytes());
        output.extend_from_slice(&controller);
        for query in queries {
            output.extend_from_slice(&query.recorded_rpm_bits.to_le_bytes());
            output.extend_from_slice(&query.seek_microseconds.to_le_bytes());
        }
        assert_eq!(car.state.car.owner.vehicle, before);
        assert_eq!(car.state.car.owner.physical, body);
        assert_eq!(
            car.state.car.owner.vehicle,
            reference.state.car.owner.vehicle
        );
        assert_eq!(
            car.state.car.owner.physical,
            reference.state.car.owner.physical
        );
        assert_eq!(car.state.car.frame.body, reference.state.car.frame.body);
        assert_eq!(car.state.car.frame.view, reference.state.car.frame.view);
        assert_eq!(car.state.car.frame.solver, reference.state.car.frame.solver);
        assert_eq!(
            car.state.world.groups.bodies,
            reference.state.world.groups.bodies
        );
        assert_eq!(
            car.state.world.groups.contacts,
            reference.state.world.groups.contacts
        );
        assert_eq!(
            car.state.world.groups.world,
            reference.state.world.groups.world
        );
        assert_eq!(
            car.state.world.manager.rows,
            reference.state.world.manager.rows
        );
    }
    fs::create_dir(out)?;
    fs::File::create_new(out.join("source.request.bin"))?.write_all(&request)?;
    fs::File::create_new(out.join("source.expected.bin"))?.write_all(&output)?;
    println!(
        "engine_control_source_ticks=1000 carried_controller=1 car_physical_frame_contacts_rows_exact=1 physics_feedback=0 playback_lengths=explicit_pcm_fixture no_backend=1"
    );
    Ok(())
}
fn main() {
    let result = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))
        .unwrap()
        .join()
        .unwrap();
    if let Err(error) = result {
        eprintln!("engine_control_source_error={error}");
        std::process::exit(1);
    }
}
