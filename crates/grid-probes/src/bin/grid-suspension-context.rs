//! Private complete source suspension-context comparison.
use std::{fs, io::Write, path::Path};
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-suspension-context: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 || fs::metadata(&args[0])?.len() != 4 + 0x2600 {
        return Err("usage: grid-suspension-context CT01_INPUT local/OUTPUT".into());
    }
    let output = Path::new(&args[1]);
    if !output
        .parent()
        .ok_or("missing parent")?
        .canonicalize()?
        .starts_with(Path::new("local").canonicalize()?)
    {
        return Err("private output required".into());
    }
    let input = fs::read(&args[0])?;
    if &input[..4] != b"CT01" {
        return Err("invalid header".into());
    }
    let words = std::array::from_fn(|i| {
        u32::from_le_bytes(input[4 + i * 4..8 + i * 4].try_into().unwrap())
    });
    let words = grid_world::suspension_context::initialize(&words)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(
        &words
            .into_iter()
            .flat_map(u32::to_le_bytes)
            .collect::<Vec<_>>(),
    )?;
    println!("context_bytes=9728");
    Ok(())
}
