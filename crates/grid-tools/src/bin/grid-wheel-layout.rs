//! Private Standard wheel geometry/offset report for original-reference comparison.
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

fn read(path: &Path, limit: u64) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err("input exceeds limit".into());
    }
    Ok(bytes)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: grid-wheel-layout INPUT.ned INPUT.csv local/OUTPUT.tsv".into());
    }
    let output = Path::new(&args[2]);
    let parent = output
        .parent()
        .ok_or("missing output parent")?
        .canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("report must remain private".into());
    }
    let xml = read(Path::new(&args[0]), 16 * 1024 * 1024)?;
    let csv = read(Path::new(&args[1]), 4 * 1024 * 1024)?;
    let doc = grid_formats::binxml::read(&xml)?;
    let geometry = grid_formats::vehicle::read_wheel_geometry(&doc)?;
    let table = grid_formats::tuning::read(&csv)?;
    let layouts: Vec<_> = table
        .rows
        .iter()
        .map(|row| grid_formats::vehicle::standard_wheel_layout(row, geometry))
        .collect::<Result<_, _>>()?;
    let mut out = std::io::BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(output.file_name().ok_or("missing output name")?))?,
    );
    for (name, axle) in [("rear", geometry.rear), ("front", geometry.front)] {
        write!(out, "G\t{name}")?;
        for value in axle.position.into_iter().chain([axle.radius]) {
            write!(out, "\t{:08x}", value.to_bits())?;
        }
        writeln!(out)?;
    }
    for (row, layout) in layouts.iter().enumerate() {
        write!(out, "O\t{row}")?;
        for value in layout.offset {
            write!(out, "\t{:08x}", value.to_bits())?;
        }
        writeln!(out)?;
        for (wheel, position) in layout.positions.iter().enumerate() {
            write!(out, "P\t{row}\t{wheel}")?;
            for value in position.iter().copied().chain([layout.radii[wheel]]) {
                write!(out, "\t{:08x}", value.to_bits())?;
            }
            writeln!(out)?;
        }
    }
    out.flush()?;
    println!("wheel_layout_rows={}", layouts.len());
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("grid-wheel-layout: {error}");
        std::process::exit(1);
    }
}
