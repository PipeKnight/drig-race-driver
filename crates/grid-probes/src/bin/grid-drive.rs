//! Source-only headless driving and private telemetry, without native captures.
use grid_tools::source_vehicle::{Controls, DELTA, Runtime};
use std::{fs::OpenOptions, io::Write, path::Path};

fn snapshot(file: &mut std::fs::File, runtime: &Runtime) -> std::io::Result<()> {
    file.write_all(b"DV01")?;
    for word in runtime
        .state
        .car
        .owner
        .vehicle
        .iter()
        .chain(&runtime.state.car.owner.physical)
        .chain(&runtime.state.car.frame.view)
        .chain(&runtime.state.car.frame.solver)
    {
        file.write_all(&word.to_le_bytes())?;
    }
    file.flush()
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !(5..=6).contains(&args.len()) {
        return Err(
            "usage: grid-drive GAME_ROOT local/OUTPUT.tsv TICKS THROTTLE STEERING [slot_00]".into(),
        );
    }
    let parse = |i: usize| args[i].to_str().ok_or("invalid argument text");
    let ticks: usize = parse(2)?.parse()?;
    if ticks > 36_000 {
        return Err("tick bound exceeded".into());
    }
    let controls = Controls {
        throttle: parse(3)?.parse()?,
        steering: parse(4)?.parse()?,
        ..Default::default()
    };
    let path = Path::new(&args[1]);
    let parent = path.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let path = parent.join(path.file_name().ok_or("missing filename")?);
    let mut report = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)?;
    let mut initial = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path.with_extension("state.bin"))?;
    let mut runtime = Runtime::lacetti(
        Path::new(&args[0]),
        args.get(5).and_then(|v| v.to_str()).unwrap_or("slot_00"),
        200679,
    )?;
    snapshot(&mut initial, &runtime)?;
    writeln!(
        report,
        "tick\tseconds\tx\ty\tz\tvx\tvy\tvz\tspeed\tgear\tengine_speed\twheel_hits\tconstraint_groups\tthrottle\tsteering"
    )?;
    for i in 0..=ticks {
        let pose = runtime.pose();
        let v = runtime.velocity();
        let car = &runtime.state.car.owner.vehicle;
        writeln!(
            report,
            "{}\t{:.6}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{:.9}\t{}\t{:.9}\t{}\t{}\t{}\t{}",
            runtime.ticks,
            runtime.ticks as f64 * f64::from(DELTA),
            pose[12],
            pose[13],
            pose[14],
            v[0],
            v[1],
            v[2],
            (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(),
            car[0x196c / 4],
            f32::from_bits(car[0x1894 / 4]),
            (0..4)
                .filter(|j| car[(0x1990 + j * 0x150 + 0xc0) / 4] != 0)
                .count(),
            runtime.state.world.visits.len(),
            controls.throttle,
            controls.steering
        )?;
        if i == ticks {
            break;
        }
        if let Err(error) = runtime.tick(controls) {
            writeln!(report, "failure\t{}\t{error}", runtime.ticks + 1)?;
            report.flush()?;
            return Err(format!("tick {}: {error}", runtime.ticks + 1).into());
        }
    }
    report.flush()?;
    let mut final_state = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path.with_extension("final.bin"))?;
    snapshot(&mut final_state, &runtime)?;
    println!(
        "source_only=1 runtime_ticks={} seconds={} pose={:?} velocity={:?}",
        runtime.ticks,
        runtime.ticks as f64 * f64::from(DELTA),
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
        eprintln!("grid-drive: {error}");
        std::process::exit(1);
    }
}
