//! Source-only standing-start telemetry: per-wheel surface speed versus
//! forward body speed, to measure launch wheelspin. Private output only.
use grid_tools::source_vehicle::{Controls, DELTA, Runtime};
use std::{fs::OpenOptions, io::Write, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !(4..=6).contains(&args.len()) {
        return Err(
            "usage: grid-launch-slip GAME_ROOT local/OUTPUT.tsv TICKS THROTTLE [HOLD_TICKS] [slot_00]"
                .into(),
        );
    }
    let parse = |i: usize| args[i].to_str().ok_or("invalid argument text");
    let ticks: usize = parse(2)?.parse()?;
    let throttle: f32 = parse(3)?.parse()?;
    // Optional initial interval with throttle and brake both applied.
    let hold: usize = args
        .get(4)
        .map(|_| parse(4))
        .transpose()?
        .unwrap_or("0")
        .parse()?;
    if ticks > 36_000 || hold > ticks {
        return Err("tick bound exceeded".into());
    }
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
    let mut runtime = Runtime::lacetti(
        Path::new(&args[0]),
        args.get(5).and_then(|v| v.to_str()).unwrap_or("slot_00"),
        200679,
    )?;
    writeln!(
        report,
        "tick\tseconds\tforward\tgear\tengine_speed\tstaged_e1c\tstaged_e3c\tsurface_rl\tsurface_rr\tsurface_fl\tsurface_fr\tslip_rl\tslip_rr\tslip_fl\tslip_fr\twheel_hits"
    )?;
    let f = |w: &[u32], o: usize| f32::from_bits(w[o / 4]);
    for i in 0..=ticks {
        let pose = runtime.pose();
        let v = runtime.velocity();
        let forward = v[0] * pose[8] + v[1] * pose[9] + v[2] * pose[10];
        let car = &runtime.state.car.owner.vehicle;
        let surface: [f32; 4] = std::array::from_fn(|j| {
            let w = 0x1990 + j * 0x150;
            f(car, w + 0x104) * f(car, w + 0xe0)
        });
        let slip = surface.map(|s| s - forward);
        writeln!(
            report,
            "{}\t{:.6}\t{:.6}\t{}\t{:.3}\t{:.3}\t{:.3}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{:.4}\t{}",
            runtime.ticks,
            runtime.ticks as f64 * f64::from(DELTA),
            forward,
            car[0x196c / 4],
            f(car, 0x1894),
            f(car, 0xe1c),
            f(car, 0xe3c),
            surface[0],
            surface[1],
            surface[2],
            surface[3],
            slip[0],
            slip[1],
            slip[2],
            slip[3],
            (0..4)
                .filter(|j| car[(0x1990 + j * 0x150 + 0xc0) / 4] != 0)
                .count(),
        )?;
        if i == ticks {
            break;
        }
        let controls = Controls {
            throttle,
            brake: if i < hold && std::env::var_os("GRID_LAUNCH_NO_BRAKE").is_none() {
                1.
            } else {
                0.
            },
            handbrake: i < hold && std::env::var_os("GRID_LAUNCH_HANDBRAKE").is_some(),
            ..Default::default()
        };
        if let Err(error) = runtime.tick(controls) {
            writeln!(report, "failure\t{}\t{error}", runtime.ticks + 1)?;
            report.flush()?;
            return Err(format!("tick {}: {error}", runtime.ticks + 1).into());
        }
    }
    report.flush()?;
    Ok(())
}

fn main() {
    let result = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))
        .expect("launch worker")
        .join()
        .expect("launch worker panic");
    if let Err(error) = result {
        eprintln!("grid-launch-slip: {error}");
        std::process::exit(1);
    }
}
