//! Private whole steering geometry initializer diagnostic.
use grid_world::steering_geometry;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-steering-geometry INPUT.tsv local/OUTPUT.tsv".into());
    }
    let text = fs::read_to_string(&args[0])?;
    if text.len() > 4 * 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let mut results = Vec::new();
    for line in text.lines() {
        let cells: Vec<_> = line.split('\t').collect();
        if cells.len() != 18 || results.len() >= 4096 {
            return Err("invalid input row".into());
        }
        let v: Vec<_> = cells[1..]
            .iter()
            .map(|v| u32::from_str_radix(v, 16).map(f32::from_bits))
            .collect::<Result<_, _>>()?;
        let points = std::array::from_fn(|i| v[1 + i * 4..5 + i * 4].try_into().unwrap());
        let r = steering_geometry::initialize(points, v[0])?;
        results.push(format!(
            "P\t{}\t{:08x}\t{}\t{}\t{:08x}\t{:08x}",
            cells[0],
            r.scale_e54.to_bits(),
            r.pair[0],
            r.pair[1],
            r.span_21d4.to_bits(),
            r.span_21d8.to_bits()
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
    println!("steering_geometry_cases={}", results.len());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-steering-geometry: {e}");
        std::process::exit(1);
    }
}
