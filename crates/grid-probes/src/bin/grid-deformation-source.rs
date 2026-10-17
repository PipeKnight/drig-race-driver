//! Direct source reader/constructor output kept private for original comparison.
use std::{fs::OpenOptions, io::Write, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: grid-deformation-source GAME_ROOT lac|bmw local/OUTPUT.bin".into());
    }
    let output = Path::new(&args[2]);
    if !output
        .parent()
        .ok_or("missing output parent")?
        .canonicalize()?
        .starts_with(Path::new("local").canonicalize()?)
    {
        return Err("private output required".into());
    }
    let prepared = grid_tools::source_deformation::load(
        Path::new(&args[0]),
        args[1].to_str().ok_or("invalid model")?,
    )?;
    let mut bytes = b"DS01".to_vec();
    bytes.extend((prepared.bounds.selected_joints as u32).to_le_bytes());
    bytes.extend((prepared.bounds.excluded_aerials as u32).to_le_bytes());
    for v in prepared
        .bounds
        .minimum
        .into_iter()
        .chain(prepared.bounds.maximum)
    {
        bytes.extend(v.to_bits().to_le_bytes());
    }
    for stage in [
        prepared.construction.reset,
        prepared.construction.layout,
        prepared.construction.initialized,
        prepared.settings,
    ] {
        for v in stage {
            bytes.extend(v.to_le_bytes());
        }
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&bytes)?;
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
