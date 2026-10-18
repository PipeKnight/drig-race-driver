//! Private headless braking replay (docs/BRAKING_REFERENCE.md): one Lacetti
//! at a Donington grid slot drives a per-tick control schedule and writes, for
//! every tick, the same vehicle regions the original capture reads
//! (`tools/capture_grid_braking.py`): +0xE00 (0x100), +0x1140 (0x80), +0x1550
//! (0x10), +0x1960 (0x30), the wheel blocks +0x1990 (4 x 0x150), then the
//! pose (16 floats), velocity (4 floats), the engine/drivetrain block
//! +0x1840 (0x120), +0x1560 (0x100: coupling mode +0x1598, flag +0x1640)
//! and +0x17A0 (0x10: engine inertia).
//!
//! `grid-braking-replay ROOT SLOT ASSISTS SCHEDULE.tsv local/OUT.tsv`
//! SCHEDULE rows: `throttle brake steering` per tick (blank lines and `#`
//! comments ignored). ASSISTS is the `--assists` spec, e.g. `braking=off`.
use grid_tools::source_vehicle::{Controls, Runtime, assists::Assists};
use std::{io::Write, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 5 || !Path::new(&args[4]).starts_with("local") {
        return Err("usage: grid-braking-replay ROOT SLOT ASSISTS SCHEDULE.tsv local/OUT.tsv".into());
    }
    let root = Path::new(&args[0]);
    let mut r = Runtime::lacetti(root, &args[1], 200679)?;
    r.set_assists(Assists::parse(&args[2])?)?;
    let schedule: Vec<Controls> = std::fs::read_to_string(&args[3])?
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| -> Result<Controls, Box<dyn std::error::Error>> {
            let v: Vec<f32> = l
                .split_whitespace()
                .map(str::parse)
                .collect::<Result<_, _>>()?;
            Ok(Controls {
                throttle: v[0],
                brake: v[1],
                steering: v[2],
                ..Default::default()
            })
        })
        .collect::<Result<_, _>>()?;
    let mut out = std::io::BufWriter::new(std::fs::File::create(&args[4])?);
    writeln!(out, "#BR01port\tslot={}\tassists={}", args[1], args[2])?;
    let hex = |words: &[u32]| -> String {
        words
            .iter()
            .flat_map(|w| w.to_le_bytes())
            .map(|b| format!("{b:02x}"))
            .collect()
    };
    for c in schedule {
        r.tick(c)?;
        let b = &r.state.car.owner.vehicle;
        let region = |o: usize, n: usize| hex(&b[o / 4..(o + n) / 4]);
        let pose = r.pose();
        let v = r.velocity();
        writeln!(
            out,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            r.ticks,
            region(0xe00, 0x100),
            region(0x1140, 0x80),
            region(0x1550, 0x10),
            region(0x1960, 0x30),
            region(0x1990, 0x540),
            pose.map(|x| x.to_string()).join(","),
            v.map(|x| x.to_string()).join(","),
            region(0x1840, 0x120),
            region(0x1560, 0x100),
            region(0x17a0, 0x10),
        )?;
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("grid-braking-replay: {e}");
        std::process::exit(1);
    }
}
