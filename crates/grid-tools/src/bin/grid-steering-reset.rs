//! Private deterministic steering reset diagnostic.
use grid_world::steering_control::{self, ResetParameters};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let source_mode = args.len() == 4 && args[2] == "--source";
    if args.len() != 2 && !source_mode {
        return Err("usage: grid-steering-reset INPUT.tsv local/OUTPUT.tsv [--source HDM]".into());
    }
    let source = if source_mode {
        Some(grid_formats::handling_damage::read(&fs::read(&args[3])?)?)
    } else {
        None
    };
    let text = fs::read_to_string(&args[0])?;
    if text.len() > 4 * 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let mut rows = Vec::new();
    for line in text.lines() {
        let cells: Vec<_> = line.split('\t').collect();
        if cells.len() != 10 || rows.len() >= 4096 {
            return Err("invalid reset row".into());
        }
        let seed = u32::from_str_radix(cells[1], 16)?;
        let mut v: Vec<_> = cells[2..]
            .iter()
            .map(|v| u32::from_str_radix(v, 16).map(f32::from_bits))
            .collect::<Result<_, _>>()?;
        if let Some(source) = source {
            v[..7].copy_from_slice(&source.scalars());
        }
        let active = match v[7] {
            0. => false,
            1. => true,
            _ => return Err("invalid active flag".into()),
        };
        let out = steering_control::reset(
            seed,
            ResetParameters {
                maximum_rotation_26c: v[0],
                maximum_cap_250: v[1],
                perturbation_gain_270: v[2],
                ramp_start_274: v[3],
                ramp_end_278: v[4],
                low_gain_27c: v[5],
                cap_threshold_254: v[6],
                wheel_views_active: active,
            },
        )?;
        let mut result = format!("P\t{}", cells[0]);
        if source_mode {
            for value in &v[..7] {
                result.push_str(&format!("\t{:08x}", value.to_bits()));
            }
        }
        result.push_str(&format!("\t{:08x}", out.random_state));
        for value in out
            .cap_seeds_48_4c
            .into_iter()
            .chain(out.rotation_seeds_50_5c)
            .chain(out.steering.scalars())
        {
            result.push_str(&format!("\t{:08x}", value.to_bits()));
        }
        for active in out.steering.frozen_3dc_424 {
            result.push_str(if active { "\t1" } else { "\t0" });
        }
        rows.push(result);
    }
    let output = Path::new(&args[1]);
    let parent = output.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let mut writer = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(output.file_name().ok_or("missing name")?))?;
    for row in &rows {
        writeln!(writer, "{row}")?;
    }
    writer.flush()?;
    println!("steering_reset_cases={}", rows.len());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-steering-reset: {e}");
        std::process::exit(1);
    }
}
