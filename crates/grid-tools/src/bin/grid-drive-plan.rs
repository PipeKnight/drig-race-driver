//! Bounded scheduled source driving with private contact/damage telemetry.
use grid_tools::source_vehicle::{Controls, DELTA, Runtime};
use std::{fs, io::Write, path::Path};

fn plan(bytes: &[u8]) -> Result<Vec<(usize, Controls)>, Box<dyn std::error::Error>> {
    if bytes.len() > 65536 {
        return Err("plan exceeds 64KiB".into());
    }
    let mut result = Vec::new();
    let mut total = 0usize;
    for line in std::str::from_utf8(bytes)?.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() != 5 || result.len() == 1024 {
            return Err("plan rows: TICKS THROTTLE BRAKE STEERING HANDBRAKE_0_OR_1".into());
        }
        let ticks: usize = fields[0].parse()?;
        let controls = Controls {
            throttle: fields[1].parse()?,
            brake: fields[2].parse()?,
            steering: fields[3].parse()?,
            handbrake: match fields[4] {
                "0" => false,
                "1" => true,
                _ => return Err("invalid handbrake".into()),
            },
        };
        if ticks == 0
            || ticks > 36000
            || !controls.throttle.is_finite()
            || !controls.brake.is_finite()
            || !controls.steering.is_finite()
            || !(0. ..=1.).contains(&controls.throttle)
            || !(0. ..=1.).contains(&controls.brake)
            || !(-1. ..=1.).contains(&controls.steering)
        {
            return Err("invalid bounded plan control".into());
        }
        total = total.checked_add(ticks).ok_or("tick overflow")?;
        if total > 36000 {
            return Err("plan exceeds 36000 ticks".into());
        }
        result.push((ticks, controls));
    }
    if result.is_empty() {
        return Err("empty plan".into());
    }
    Ok(result)
}

fn snapshot(path: &Path, runtime: &Runtime) -> std::io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    file.write_all(b"DP01")?;
    let car = &runtime.state.car;
    for count in [
        car.owner.vehicle.len(),
        car.owner.physical.len(),
        car.frame.view.len(),
        car.frame.solver.len(),
    ] {
        file.write_all(&(count as u32).to_le_bytes())?;
    }
    file.write_all(&runtime.ticks.to_le_bytes())?;
    file.write_all(&u32::from(car.owner.damage_random_state.is_some()).to_le_bytes())?;
    file.write_all(&car.owner.damage_random_state.unwrap_or(0).to_le_bytes())?;
    for word in car
        .owner
        .vehicle
        .iter()
        .chain(&car.owner.physical)
        .chain(&car.frame.view)
        .chain(&car.frame.solver)
    {
        file.write_all(&word.to_le_bytes())?;
    }
    file.flush()
}

fn sample(
    file: &mut fs::File,
    runtime: &Runtime,
    segment: usize,
    controls: Controls,
) -> std::io::Result<()> {
    let pose = runtime.pose();
    let velocity = runtime.velocity();
    let car = &runtime.state.car.owner;
    let levels = car.vehicle[(0xcdf0 + 0x3a0) / 4..(0xcdf0 + 0x3c4) / 4]
        .iter()
        .map(|word| format!("{word:08x}"))
        .collect::<Vec<_>>()
        .join(",");
    let impacts = (0..4)
        .map(|i| format!("{:08x}", car.vehicle[(0x1ac8 + i * 0x150) / 4]))
        .collect::<Vec<_>>()
        .join(",");
    writeln!(
        file,
        "{}\t{:.6}\t{segment}\t{}\t{}\t{}\t{}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{}\t{:08x}\t{:08x}\t{levels}\t{impacts}\t{:08x},{:08x}",
        runtime.ticks,
        runtime.ticks as f64 * f64::from(DELTA),
        controls.throttle,
        controls.brake,
        controls.steering,
        u8::from(controls.handbrake),
        pose[12],
        pose[13],
        pose[14],
        velocity[0],
        velocity[1],
        velocity[2],
        runtime.last_body_contacts,
        car.damage_random_state.unwrap_or(0),
        car.vehicle[0x1644 / 4],
        car.physical[0x3f0 / 4],
        car.vehicle[(0xcdf0 + 0x368) / 4]
    )
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 4 {
        return Err(
            "usage: grid-drive-plan GAME_ROOT PRIVATE_CD01 PLAN.txt local/NEW_OUTPUT_DIR".into(),
        );
    }
    if fs::metadata(&args[2])?.len() > 65536 || fs::metadata(&args[1])?.len() != 712 {
        return Err("bounded plan and private sampling required".into());
    }
    let bytes = fs::read(&args[2])?;
    let controls = plan(&bytes)?;
    let output = Path::new(&args[3]);
    let parent = output.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output directory required".into());
    }
    // Canonicalize the parent but require a new final component: existing files,
    // directories and links are preserved rather than traversed or overwritten.
    let output = parent.join(output.file_name().ok_or("missing output name")?);
    if fs::symlink_metadata(&output).is_ok() {
        return Err("output already exists".into());
    }
    let mut runtime = Runtime::lacetti(Path::new(&args[0]), "slot_00", 200679)?;
    let geometry = runtime.prepare_body_geometry(Path::new(&args[0]), &fs::read(&args[1])?)?;
    fs::create_dir(&output)?;
    fs::write(output.join("plan.txt"), bytes)?;
    snapshot(&output.join("initial.bin"), &runtime)?;
    let mut report = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("telemetry.tsv"))?;
    writeln!(
        report,
        "tick\tseconds\tsegment\tthrottle\tbrake\tsteering\thandbrake\tx\ty\tz\tvx\tvy\tvz\tbody_contacts\tglobal_rng\twear_rng\tdamage_words\twheel_impact_words\tdamage_threshold_words"
    )?;
    sample(&mut report, &runtime, 0, Controls::default())?;
    let mut contacts = 0usize;
    let initial_rng = runtime.state.car.owner.damage_random_state;
    for (segment, (ticks, controls)) in controls.into_iter().enumerate() {
        for _ in 0..ticks {
            if let Err(error) = runtime.tick_with_body_geometry(controls, &geometry) {
                writeln!(report, "failure\t{}\t{error}", runtime.ticks + 1)?;
                report.flush()?;
                snapshot(&output.join("failed.bin"), &runtime)?;
                return Err(format!("attempted tick {}: {error}", runtime.ticks + 1).into());
            }
            contacts += runtime.last_body_contacts;
            sample(&mut report, &runtime, segment, controls)?;
        }
    }
    report.flush()?;
    snapshot(&output.join("final.bin"), &runtime)?;
    println!(
        "ticks={} body_contacts={contacts} damage_rng_changed={} pose={:?} velocity={:?}",
        runtime.ticks,
        runtime.state.car.owner.damage_random_state != initial_rng,
        runtime.pose(),
        runtime.velocity()
    );
    Ok(())
}

fn main() {
    let result = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))
        .expect("drive worker")
        .join()
        .expect("drive worker panic");
    if let Err(error) = result {
        eprintln!("grid-drive-plan: {error}");
        std::process::exit(1);
    }
}
