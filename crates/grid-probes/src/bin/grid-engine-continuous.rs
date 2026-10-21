//! Private deterministic mixer audition and actual-vehicle no-feedback check.
use grid_tools::{
    engine_audio::{Control, RATE},
    engine_audio_continuous::{Bank as EngineBank, Mixer},
    source_vehicle::{Controls, Runtime},
};
use std::{fmt::Write as _, fs, io::Write, path::Path};
fn wave(path: &Path, pcm: &[i16]) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = (pcm.len() * 2) as u32;
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    out.write_all(b"RIFF")?;
    out.write_all(&(bytes + 36).to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&16u32.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?;
    out.write_all(&RATE.to_le_bytes())?;
    out.write_all(&(RATE * 2).to_le_bytes())?;
    out.write_all(&2u16.to_le_bytes())?;
    out.write_all(&16u16.to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&bytes.to_le_bytes())?;
    for word in pcm {
        out.write_all(&word.to_le_bytes())?;
    }
    Ok(())
}
fn render(mixer: &mut Mixer, control: Control, frames: usize, pcm: &mut Vec<i16>) {
    for _ in 0..frames {
        let sample = mixer.sample(control);
        assert!(sample.is_finite() && sample.abs() < 1.);
        pcm.push((sample * 32767.).round() as i16);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-engine-continuous GAME_ROOT local/NEW_DIRECTORY".into());
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
    let bank = EngineBank::lacetti(root)?;
    let mut mixer = Mixer::new(bank.clone());
    let mut pcm = Vec::new();
    // Fixed speeds distinguish stable notes from playback of an entire sweep.
    for (rpm, throttle, audible, generation) in [
        (2200., 0., true, 0),
        (4500., 0.5, true, 0),
        (7000., 1., true, 0),
        (4500., 0., true, 0),
        (4500., 0., false, 0),
        (2200., 0., true, 1),
    ] {
        render(
            &mut mixer,
            Control {
                normalized_rpm: rpm / 8500.,
                throttle,
                audible,
                generation,
                gear: 0,
            },
            RATE as usize * 2,
            &mut pcm,
        );
    }
    assert!(mixer.failure.is_none());
    assert_eq!(mixer.diagnostics.failed_blocks, 0);
    assert_eq!(mixer.diagnostics.clipped_samples, 0);
    let audition = pcm;
    let mut pcm = Vec::new();
    let mut mixer = Mixer::new(bank);
    let mut car = Runtime::lacetti(root, "slot_00", 200679)?;
    let mut reference = Runtime::lacetti(root, "slot_00", 200679)?;
    let mut trace = String::from("tick\tangular_speed\tnormalized_rpm\tthrottle\tgear\n");
    for tick in 0..1000 {
        let controls = Controls {
            throttle: if (125..625).contains(&tick) { 1. } else { 0. },
            brake: if (750..1000).contains(&tick) { 1. } else { 0. },
            ..Default::default()
        };
        car.tick(controls)?;
        reference.tick(controls)?;
        let before = car.state.car.owner.vehicle.clone();
        let body = car.state.car.owner.physical;
        let control = Control::vehicle(&before, controls.throttle, true, 0);
        render(&mut mixer, control, 768, &mut pcm); // DELTA=.016; exactly 768 output frames.
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
        writeln!(
            trace,
            "{}\t{}\t{}\t{}\t{}",
            car.ticks,
            f32::from_bits(before[0x1894 / 4]),
            control.normalized_rpm,
            controls.throttle,
            before[0x196c / 4]
        )?;
    }
    assert!(mixer.failure.is_none());
    assert_eq!(mixer.diagnostics.failed_blocks, 0);
    assert_eq!(mixer.diagnostics.clipped_samples, 0);
    fs::create_dir(out)?;
    wave(&out.join("fixed-rpm.wav"), &audition)?;
    wave(&out.join("source-drive.wav"), &pcm)?;
    fs::write(out.join("telemetry.tsv"), trace)?;
    println!(
        "continuous_engine_mixer frames={} source_ticks=1000 car_physical_frame_contacts_rows_exact=1 physics_feedback=0 pause_reset=1 rate={RATE}",
        audition.len() + pcm.len()
    );
    println!("continuous_engine_diagnostics {:?}", mixer.diagnostics);
    Ok(())
}
fn main() {
    let result = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))
        .unwrap()
        .join()
        .unwrap();
    if let Err(e) = result {
        eprintln!("grid-engine-continuous: {e}");
        std::process::exit(1);
    }
}
