//! Private chunk-name population report.
use std::{fs::OpenOptions, io::Write, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-population INPUT.txt local/OUTPUT.bin".into());
    }
    let input = Path::new(&args[0]);
    if input.metadata()?.len() > 32768 {
        return Err("input too large".into());
    }
    let mut entries = Vec::new();
    for name in std::fs::read_to_string(input)?.lines() {
        if entries.len() >= 512 || name.len() > 63 {
            return Err("path count or length exceeds bound".into());
        }
        entries.push((
            entries.len() as u32 + 1,
            grid_world::grid_population::Path::from_name(name)?,
        ));
    }
    let grid = grid_world::grid_population::build([0., 0., 1., 1.], &entries)?;
    let mut bytes = b"GBL1".to_vec();
    bytes.extend((entries.len() as u32).to_le_bytes());
    for (_, path) in entries {
        for v in [path.x, path.z, u32::from(path.depth)] {
            bytes.extend(v.to_le_bytes());
        }
    }
    for tile in grid.tiles {
        bytes.extend(u32::from(tile.exponent).to_le_bytes());
        bytes.extend((1u32 << tile.exponent).to_le_bytes());
        for id in tile.cells {
            bytes.extend(id.to_le_bytes());
        }
    }
    let out = Path::new(&args[1]);
    let parent = out.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output only".into());
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(out.file_name().ok_or("missing name")?))?
        .write_all(&bytes)?;
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-population: {e}");
        std::process::exit(1);
    }
}
