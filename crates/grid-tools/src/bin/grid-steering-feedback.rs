//! Private whole steering feedback diagnostic.
use grid_world::steering_feedback::{self, Input};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-steering-feedback INPUT.tsv local/OUTPUT.tsv".into());
    }
    let text = fs::read_to_string(&args[0])?;
    if text.len() > 4 * 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let mut rows = Vec::new();
    for line in text.lines() {
        let cells: Vec<_> = line.split('\t').collect();
        if cells.len() != 36 || rows.len() >= 4096 {
            return Err("invalid input row".into());
        }
        let v: Vec<_> = cells[1..]
            .iter()
            .map(|v| u32::from_str_radix(v, 16).map(f32::from_bits))
            .collect::<Result<_, _>>()?;
        let value = steering_feedback::calculate(Input {
            speed_scalar: v[0],
            scale_e54: v[1],
            command_e30: v[2],
            matrix: std::array::from_fn(|i| v[3 + i * 4..7 + i * 4].try_into().unwrap()),
            linear: v[19..23].try_into()?,
            angular: v[23..27].try_into()?,
            wheel_points_2_3: std::array::from_fn(|i| {
                v[27 + i * 4..31 + i * 4].try_into().unwrap()
            }),
        })?;
        rows.push(format!("P\t{}\t{:08x}", cells[0], value.to_bits()));
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
    for row in &rows {
        writeln!(writer, "{row}")?;
    }
    writer.flush()?;
    println!("steering_feedback_cases={}", rows.len());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-steering-feedback: {e}");
        std::process::exit(1);
    }
}
