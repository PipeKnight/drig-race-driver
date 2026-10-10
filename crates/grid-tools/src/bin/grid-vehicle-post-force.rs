//! Bounded private diagnostic for the post-force driver interval.
use std::{fs::OpenOptions, io::Write, path::Path};
const RECORD: usize = 0x2604;
fn word(data: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(data[o..o + 4].try_into().unwrap())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-vehicle-post-force INPUT local/OUTPUT".into());
    }
    let input = Path::new(&args[0]);
    let output = Path::new(&args[1]);
    if input.metadata()?.len() > 2 * 1024 * 1024
        || !output
            .parent()
            .ok_or("missing parent")?
            .canonicalize()?
            .starts_with(Path::new("local").canonicalize()?)
    {
        return Err("bounded private output required".into());
    }
    let mut data = std::fs::read(input)?;
    if data.len() < 8 || word(&data, 0) != 0x31304154 {
        return Err("invalid header".into());
    }
    let count = word(&data, 4) as usize;
    if !(1..=64).contains(&count) || data.len() != 8 + count * RECORD {
        return Err("invalid size/count".into());
    }
    for record in data[8..].as_chunks_mut::<RECORD>().0 {
        let mut body = std::array::from_fn(|i| word(record, 4 + i * 4));
        grid_world::vehicle_post_force::apply(&mut body, f32::from_bits(word(record, 0)))?;
        for (i, value) in body.into_iter().enumerate() {
            record[4 + i * 4..8 + i * 4].copy_from_slice(&value.to_le_bytes());
        }
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(&data)?;
    println!("post_force_cases={count}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
