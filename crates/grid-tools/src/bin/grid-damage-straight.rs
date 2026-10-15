//! Source-only controlled damage experiment: preset per-wheel handling-damage
//! levels (CDF0+18), then drive straight with constant controls and record
//! drift, heading and speed. Private output only; authored diagnostic.
use grid_tools::source_vehicle::{Controls, DELTA, Runtime};
use std::{fs::OpenOptions, io::Write, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 5 {
        return Err(
            "usage: grid-damage-straight GAME_ROOT local/OUTPUT.tsv TICKS THROTTLE LEVEL_RL,LEVEL_RR,LEVEL_FL,LEVEL_FR"
                .into(),
        );
    }
    let parse = |i: usize| args[i].to_str().ok_or("invalid argument text");
    let ticks: usize = parse(2)?.parse()?;
    let throttle: f32 = parse(3)?.parse()?;
    let levels: Vec<f32> = parse(4)?
        .split(',')
        .map(str::parse)
        .collect::<Result<_, _>>()?;
    if ticks > 36_000 || levels.len() != 4 || levels.iter().any(|l| !(0.0..=1.0).contains(l)) {
        return Err("invalid tick bound or levels".into());
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
    let mut runtime = Runtime::lacetti(Path::new(&args[0]), "slot_00", 200679)?;
    for (i, level) in levels.iter().enumerate() {
        runtime.state.car.owner.vehicle[(0xcdf0 + 0x18) / 4 + i] = level.to_bits();
    }
    let start = runtime.pose();
    let heading0 = start[10].atan2(start[8]);
    writeln!(
        report,
        "tick\tseconds\tforward\tlateral_offset\theading_change_deg\tyaw_rate\tgear\tlevels\teffects_3a0_3bc"
    )?;
    let controls = Controls {
        throttle,
        ..Default::default()
    };
    for i in 0..=ticks {
        let pose = runtime.pose();
        let v = runtime.velocity();
        let forward = v[0] * pose[8] + v[1] * pose[9] + v[2] * pose[10];
        // Offset from the initial straight line, along the initial side axis.
        let d = [pose[12] - start[12], pose[14] - start[14]];
        let lateral = d[0] * start[0] + d[1] * start[2];
        let heading = pose[10].atan2(pose[8]);
        let mut change = (heading - heading0).to_degrees();
        if change > 180. {
            change -= 360.;
        } else if change < -180. {
            change += 360.;
        }
        let b = &runtime.state.car.owner.vehicle;
        let angular = f32::from_bits(runtime.state.car.owner.physical[0x210 / 4 + 1]);
        let lv: Vec<String> = (0..4)
            .map(|j| format!("{:.3}", f32::from_bits(b[(0xcdf0 + 0x18) / 4 + j])))
            .collect();
        let ef: Vec<String> = (0..8)
            .map(|j| format!("{:.3}", f32::from_bits(b[(0xcdf0 + 0x3a0) / 4 + j])))
            .collect();
        writeln!(
            report,
            "{}\t{:.3}\t{forward:.4}\t{lateral:.4}\t{change:.4}\t{angular:.5}\t{}\t{}\t{}",
            runtime.ticks,
            runtime.ticks as f64 * f64::from(DELTA),
            b[0x196c / 4],
            lv.join(","),
            ef.join(",")
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
    Ok(())
}

fn main() {
    let result = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))
        .expect("damage worker")
        .join()
        .expect("damage worker panic");
    if let Err(error) = result {
        eprintln!("grid-damage-straight: {error}");
        std::process::exit(1);
    }
}
