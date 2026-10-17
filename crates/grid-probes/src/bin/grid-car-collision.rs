//! Private report of supported named car collision geometry and description bounds.
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-car-collision INPUT.ned local/OUTPUT.tsv".into());
    }
    let output = Path::new(&args[1]);
    let parent = output
        .parent()
        .ok_or("missing output parent")?
        .canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("report must remain private".into());
    }
    let mut bytes = Vec::new();
    File::open(&args[0])?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let doc = grid_formats::binxml::read(&bytes)?;
    let mesh = grid_formats::car_collision::read(&doc)?;
    let mut out = std::io::BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(output.file_name().ok_or("missing output name")?))?,
    );
    for (ordinal, point) in mesh.points.iter().enumerate() {
        writeln!(
            out,
            "P\t{ordinal}\t{:08x}\t{:08x}\t{:08x}",
            point[0].to_bits(),
            point[1].to_bits(),
            point[2].to_bits()
        )?;
    }
    for (ordinal, triangle) in mesh.triangles.iter().enumerate() {
        writeln!(
            out,
            "T\t{ordinal}\t{}\t{}\t{}",
            triangle[0], triangle[1], triangle[2]
        )?;
    }
    write!(out, "B")?;
    for value in mesh.bounds.description_extents()? {
        write!(out, "\t{:08x}", value.to_bits())?;
    }
    writeln!(out)?;
    out.flush()?;
    println!(
        "car_collision_points={} triangles={}",
        mesh.points.len(),
        mesh.triangles.len()
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("grid-car-collision: {error}");
        std::process::exit(1);
    }
}
