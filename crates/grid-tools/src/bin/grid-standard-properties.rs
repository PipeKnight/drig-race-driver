//! Private complete Standard properties comparison report.
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-standard-properties INPUT.csv local/OUTPUT.bin".into());
    }
    let output = Path::new(&args[1]);
    let parent = output
        .parent()
        .ok_or("output parent absent")?
        .canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?)
        || fs::metadata(&args[0])?.len() > 4 * 1024 * 1024
    {
        return Err("bounded source and private output required".into());
    }
    let bytes = fs::read(&args[0])?;
    let table = grid_formats::tuning::read(&bytes)?;
    if table.rows.len() != 1 {
        return Err("exactly one Standard source row required".into());
    }
    let properties = grid_formats::vehicle_properties::read(&table.rows[0], 0x2008)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(output.file_name().ok_or("output filename absent")?))?;
    file.write_all(properties.bytes())?;
    println!("properties_bytes={}", properties.bytes().len());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-standard-properties: {e}");
        std::process::exit(1);
    }
}
