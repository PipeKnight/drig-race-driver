//! Writes the private BP01/DF01 (and optionally CD01) files from the user's
//! own grid.exe (the Rust port of tools/extract_body_impact_parameters.py,
//! tools/extract_deformation_parameters.py and
//! tools/extract_contact_directions.py). Refuses to overwrite.
use grid_tools::private_parameters::{body_impact, contact_directions, deformation, hex, sha256};
use std::io::Write;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !(3..=4).contains(&args.len()) {
        return Err(
            "usage: grid-private-parameters GRID_EXE BP01_OUTPUT DF01_OUTPUT [CD01_OUTPUT]".into(),
        );
    }
    let exe = std::fs::read(&args[0])?;
    let mut outputs = vec![
        (&args[1], body_impact(&exe)?),
        (&args[2], deformation(&exe)?),
    ];
    if let Some(path) = args.get(3) {
        outputs.push((path, contact_directions(&exe)?));
    }
    for (path, bytes) in &outputs {
        std::fs::File::create_new(path)?.write_all(bytes)?;
        println!(
            "{} {} bytes sha256={}",
            path.to_string_lossy(),
            bytes.len(),
            hex(&sha256(bytes))
        );
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("grid-private-parameters: {error}");
        std::process::exit(1);
    }
}
