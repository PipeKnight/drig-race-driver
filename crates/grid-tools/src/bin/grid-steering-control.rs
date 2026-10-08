//! Private mutable-state steering producer diagnostic.
use grid_world::steering_control::{self, Input};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-steering-control INPUT.tsv local/OUTPUT.tsv".into());
    }
    let text = fs::read_to_string(&args[0])?;
    if text.len() > 4 * 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let mut results = Vec::new();
    for line in text.lines() {
        let cells: Vec<_> = line.split('\t').collect();
        if cells.len() != 19 || results.len() >= 4096 {
            return Err("invalid input row".into());
        }
        let v: Vec<_> = cells[1..]
            .iter()
            .map(|v| u32::from_str_radix(v, 16).map(f32::from_bits))
            .collect::<Result<_, _>>()?;
        let active = match v[17] {
            0. => false,
            1. => true,
            _ => return Err("invalid active flag".into()),
        };
        let output = steering_control::calculate(Input {
            wheel_levels_18_24: v[..4].try_into()?,
            cap_levels_30_34: v[4..6].try_into()?,
            cap_seeds_48_4c: v[6..8].try_into()?,
            rotation_seeds_50_5c: v[8..12].try_into()?,
            perturbation_gain_270: v[12],
            ramp_start_274: v[13],
            ramp_end_278: v[14],
            low_gain_27c: v[15],
            cap_threshold_254: v[16],
            wheel_views_active: active,
        })?;
        let mut words = output
            .scalars()
            .iter()
            .map(|v| format!("\t{:08x}", v.to_bits()))
            .collect::<String>();
        for flag in output.frozen_3dc_424 {
            words.push_str(if flag { "\t1" } else { "\t0" });
        }
        results.push(format!("P\t{}{words}", cells[0]));
    }
    let out = Path::new(&args[1]);
    let parent = out.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let mut writer = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(out.file_name().ok_or("missing name")?))?;
    for row in &results {
        writeln!(writer, "{row}")?;
    }
    writer.flush()?;
    println!("steering_control_cases={}", results.len());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-steering-control: {e}");
        std::process::exit(1);
    }
}
