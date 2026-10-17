//! Private diagnostic for the downstream supplied history grip multiplier.
use grid_world::grip_history::{self, Input};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 && args.len() != 3 {
        return Err("usage: grid-grip-history INPUT.tsv local/OUTPUT.tsv [CAR.csv]".into());
    }
    let reductions = if args.len() == 3 {
        let bytes = fs::read(&args[2])?;
        let table = grid_formats::tuning::read(&bytes)?;
        if table.rows.len() != 1 {
            return Err("source mode requires one Standard row".into());
        }
        Some(grid_formats::vehicle::read_standard_grip_reductions(
            &table.rows[0],
        )?)
    } else {
        None
    };
    let text = fs::read_to_string(&args[0])?;
    if text.len() > 4 * 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let mut results = Vec::new();
    for line in text.lines() {
        let cells: Vec<_> = line.split('\t').collect();
        if cells.len() != 13 || results.len() >= 4096 {
            return Err("invalid input row".into());
        }
        let v: Vec<_> = cells[1..]
            .iter()
            .map(|v| u32::from_str_radix(v, 16).map(f32::from_bits))
            .collect::<Result<_, _>>()?;
        if v.iter().any(|v| !v.is_finite())
            || ![0., 1., 2., 3.].contains(&v[0])
            || [1, 2].iter().any(|i| v[*i] != 0. && v[*i] != 1.)
        {
            return Err("invalid ordinal/flags/values".into());
        }
        let input = Input {
            flag_b12: v[1] != 0.,
            flag_227c: v[2] != 0.,
            history: v[3..6].try_into()?,
            scalar_1ffc: v[6],
            scalar_1f24: v[7],
            scalar_1fa4: reductions.map_or(v[8], |r| r[0]),
            scalar_1fa8: reductions.map_or(v[9], |r| r[1]),
            wheel_coordinate_z: v[10],
        };
        let factor = grip_history::factor(input)?;
        let grip = grip_history::apply(input, v[11])?;
        results.push(format!(
            "P\t{}\t{:08x}\t{:08x}",
            cells[0],
            factor.to_bits(),
            grip.to_bits()
        ));
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
    println!("grip_history_cases={}", results.len());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-grip-history: {e}");
        std::process::exit(1);
    }
}
