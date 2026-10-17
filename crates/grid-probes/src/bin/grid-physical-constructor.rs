//! Private complete physical constructor comparison.
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-physical-constructor INPUT local/OUTPUT".into());
    }
    if fs::metadata(&args[0])?.len() != 0x16c {
        return Err("invalid input size".into());
    }
    let b = fs::read(&args[0])?;
    let u = |o| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    if u(0) != 0x3130_4350 || u(8) > 255 {
        return Err("invalid framing".into());
    }
    let output = Path::new(&args[1]);
    let parent = output.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let result = grid_world::physical_constructor::initialize(
        &std::array::from_fn(|i| u(12 + i * 4)),
        u(4),
        u(8) as u8,
    )?;
    let mut writer = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(output.file_name().ok_or("missing filename")?))?;
    for v in result
        .body
        .into_iter()
        .chain(result.settings)
        .chain(result.world)
    {
        writer.write_all(&v.to_le_bytes())?;
    }
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-physical-constructor: {error}");
        std::process::exit(1);
    }
}
