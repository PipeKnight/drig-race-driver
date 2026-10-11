//! Bounded private whole mutable handling comparison.
use grid_world::vehicle_publication::{PHYSICAL_WORDS, State, VEHICLE_WORDS};
use std::{fs::OpenOptions, io::Write, path::Path};
const RECORD: usize = 4 + (VEHICLE_WORDS + PHYSICAL_WORDS) * 4;
fn word(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-vehicle-handling INPUT.bin local/OUTPUT.bin".into());
    }
    let input = Path::new(&args[0]);
    let output = Path::new(&args[1]);
    if input.metadata()?.len() > 16 * 1024 * 1024
        || !output
            .parent()
            .ok_or("missing parent")?
            .canonicalize()?
            .starts_with(Path::new("local").canonicalize()?)
    {
        return Err("bounded private output required".into());
    }
    let mut data = std::fs::read(input)?;
    if data.len() < 8
        || &data[..4] != b"VH01"
        || !(1..=64).contains(&word(&data, 4))
        || data.len() != 8 + word(&data, 4) as usize * RECORD
    {
        return Err("invalid publication framing".into());
    }
    for record in data[8..].as_chunks_mut::<RECORD>().0 {
        let delta = f32::from_bits(word(record, 0));
        let state = State {
            vehicle: (0..VEHICLE_WORDS)
                .map(|i| word(record, 4 + i * 4))
                .collect(),
            physical: std::array::from_fn(|i| word(record, 4 + VEHICLE_WORDS * 4 + i * 4)),
        };
        let next = grid_world::vehicle_handling::calculate(&state, delta)?;
        for (i, w) in next.vehicle.iter().chain(next.physical.iter()).enumerate() {
            record[4 + i * 4..8 + i * 4].copy_from_slice(&w.to_le_bytes());
        }
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&data)?;
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-vehicle-handling: {e}");
        std::process::exit(1);
    }
}
