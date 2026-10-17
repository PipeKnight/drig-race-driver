//! Source driving feeds the verified control pipeline, with a paired physics run.
//! Uses purchased carsetup lengths; no whole native factory/backend claim.
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
    let mut setup_bytes = Vec::new();
    fs::File::open(root.join("audio/carsetup.bin"))?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut setup_bytes)?;
    let setup = grid_formats::audio_car_setup::read(&setup_bytes, "lac")?;
    if setup.engine_bank != "eng_lac" {
        return Err("selected engine bank unsupported".into());
    }
    let mut dictionary_bytes = Vec::new();
    fs::File::open(root.join("audio/CMSamples.dic"))?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut dictionary_bytes)?;
    let dictionary = grid_formats::audio_dictionary::read(&dictionary_bytes)?;
    let bank = dictionary
        .banks
        .iter()
        .find(|bank| bank.name == setup.engine_bank)
        .ok_or("selected audio bank absent")?;
    if bank.extension != "WIP" || bank.samples.len() != 6 {
        return Err("selected audio bank layout".into());
    }
    let mut bank_bytes = Vec::new();
    fs::File::open(
        root.join("audio")
            .join(format!("{}.WIP", setup.engine_bank)),
    )?
    .take(64 * 1024 * 1024 + 1)
    .read_to_end(&mut bank_bytes)?;
    let mut rates = [0_u32; 6];
    for (slot, name) in [
        "dn_en",
        "up_en",
        "up_en_load",
        "dn_ex",
        "up_ex",
        "up_ex_load",
    ]
    .into_iter()
    .enumerate()
    {
        let sample_index = bank
            .samples
            .iter()
            .position(|sample| sample.name == name)
            .ok_or("selected audio sample absent")?;
        let sample = &bank.samples[sample_index];
        let frames = if slot % 3 == 0 {
            setup.idle_frames
        } else {
            setup.throttle_frames
        };
        if bank.mono_pcm16(sample_index, &bank_bytes)?.len() / 2 != frames as usize {
            return Err("selected Lacetti setup/sample extent mismatch".into());
        }
        rates[slot] = u32::from(sample.rate);
    }
    let mut ini = String::new();
    fs::File::open(root.join(setup.engine_ini))?
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
        put(&mut controller, 0x290 + slot * 4, rates[slot % 6]);
        put(
            &mut controller,
            0x2c0 + slot * 4,
            if slot % 3 == 0 {
                setup.idle_frames
            } else {
                setup.throttle_frames
            },
        );
    }
    let mut request = Vec::new();
    request.extend_from_slice(&0x31304345_u32.to_le_bytes());
    request.extend_from_slice(&1000_u32.to_le_bytes());
    for value in curves {
        request.extend_from_slice(&value.to_bits().to_le_bytes());
    }
    let mut output = request[..8].to_vec();
    let mut gain_request = b"EG01".to_vec();
    gain_request.extend_from_slice(&1000_u32.to_le_bytes());
    let mut gain_output = gain_request.clone();
    let mut gain_voices = [[0x6b; grid_tools::engine_audio_publication::VOICE_BYTES]; 12];
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
        // This fixture supplies ordinary throttle and triangular fades, not 451770.
        let mut gain_controller = controller;
        gain_controller[0xd8] = 0;
        for (at, value) in [
            (0xcc, controls.throttle),
            (0x1a0, 0.),
            (0x2f8, 1.),
            (0x330, 0.),
        ] {
            put(&mut gain_controller, at, value.to_bits());
        }
        let phase = ((tick % 250) as f32 / 249.) * 2. - 1.;
        for category in 0..3 {
            put(&mut gain_controller, 0x140 + category * 4, phase.to_bits());
        }
        let pose = car.pose();
        for emitter in 0..2 {
            let point = setup.offsets[emitter];
            for axis in 0..3 {
                let position = (pose[12 + axis] + pose[4 + axis] * point[1])
                    + (pose[axis] * point[0] + pose[8 + axis] * point[2]);
                put(
                    &mut gain_controller,
                    0xe0 + emitter * 16 + axis * 4,
                    position.to_bits(),
                );
            }
        }
        let gain_env = grid_tools::engine_audio_publication::Environment {
            motion_210: std::array::from_fn(|i| f32::from_bits(body[0x210 / 4 + i])),
            motion_220: std::array::from_fn(|i| f32::from_bits(body[0x220 / 4 + i])),
            ranges: [[1., 50.], [1., 50.]],
            trims: [setup.trims[1], setup.trims[2]],
            treble: 0.,
            player: true,
        };
        gain_request.extend_from_slice(&tick.to_le_bytes());
        gain_request.extend_from_slice(&gain_controller);
        for voice in gain_voices {
            gain_request.extend_from_slice(&voice);
        }
        for value in gain_env
            .motion_210
            .into_iter()
            .chain(gain_env.motion_220)
            .chain(gain_env.ranges.into_iter().flatten())
            .chain(gain_env.trims)
            .chain([gain_env.treble])
        {
            gain_request.extend_from_slice(&value.to_bits().to_le_bytes());
        }
        gain_request.extend_from_slice(&1_u32.to_le_bytes());
        grid_tools::engine_audio_publication::publish(
            &mut gain_controller,
            &mut gain_voices,
            gain_env,
        )?;
        gain_output.extend_from_slice(&tick.to_le_bytes());
        gain_output.extend_from_slice(&gain_controller);
        for voice in gain_voices {
            gain_output.extend_from_slice(&voice);
        }
        gain_output.extend_from_slice(&0_u32.to_le_bytes());
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
    fs::File::create_new(out.join("source.gain.request.bin"))?.write_all(&gain_request)?;
    fs::File::create_new(out.join("source.gain.expected.bin"))?.write_all(&gain_output)?;
    println!(
        "engine_control_source_ticks=1000 carried_controller=1 gain_publication=1 carried_voices=1 car_physical_frame_contacts_rows_exact=1 physics_feedback=0 playback_lengths=purchased_carsetup no_backend=1"
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
