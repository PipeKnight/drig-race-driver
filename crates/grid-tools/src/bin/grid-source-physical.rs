//! Bounded private complete source body-factory comparison.
use grid_tools::source_physical::{self, CAR_BYTES};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-source-physical INPUT local/OUTPUT".into());
    }
    if fs::metadata(&args[0])?.len() != (12 + 0x65c + CAR_BYTES) as u64 {
        return Err("invalid input size".into());
    }
    let b = fs::read(&args[0])?;
    let u = |o| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    if u(0) != 0x31304650 || u(8) > 255 {
        return Err("invalid framing".into());
    }
    let output = Path::new(&args[1]);
    let parent = output.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let result = source_physical::initialize(
        &b[12 + 0x65c..],
        std::array::from_fn(|i| f32::from_bits(u(12 + 0x130 + i * 4))),
        u(4),
        u(8) as u8,
    )?;
    let bytes = result.bytes();
    assert_eq!(bytes.len(), source_physical::OUTPUT_BYTES);
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(output.file_name().ok_or("missing filename")?))?
        .write_all(&bytes)?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-source-physical: {error}");
        std::process::exit(1)
    }
}
