//! Private full handling update with explicit global RNG and owned queries.
use grid_world::{
    vehicle_handling,
    vehicle_publication::{PHYSICAL_WORDS, State, VEHICLE_WORDS},
};
use std::{fs, io::Write, path::Path};
const PREFIX: usize = 16 + 64 * 8;
const SIZE: usize = PREFIX + (VEHICLE_WORDS + PHYSICAL_WORDS) * 4;
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args_os().skip(1).collect();
    if a.len() != 2 || fs::metadata(&a[0])?.len() > 8 + 64 * SIZE as u64 {
        return Err("usage: grid-handling-damage VH02_INPUT local/OUTPUT".into());
    }
    let mut raw = fs::read(&a[0])?;
    if raw.len() < 8 || &raw[..4] != b"VH02" {
        return Err("invalid handling damage framing".into());
    }
    let n = u32::from_le_bytes(raw[4..8].try_into().unwrap()) as usize;
    if !(1..=64).contains(&n) || raw.len() != 8 + n * SIZE {
        return Err("invalid handling damage extent".into());
    }
    for r in raw[8..].as_chunks_mut::<SIZE>().0 {
        let u = |o| u32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        let count = u(12) as usize;
        if u(8) > 1
            || count > 64
            || (u(8) == 0 && count != 0)
            || r[16 + count * 8..PREFIX].iter().any(|v| *v != 0)
        {
            return Err("invalid handling query list".into());
        }
        let queries: Vec<_> = (0..count)
            .map(|i| vehicle_handling::QueryRecord {
                id: u(16 + i * 8),
                status: u(20 + i * 8),
            })
            .collect();
        let state = State {
            damage_random_state: Some(u(4)),
            vehicle: (0..VEHICLE_WORDS).map(|i| u(PREFIX + i * 4)).collect(),
            physical: std::array::from_fn(|i| u(PREFIX + VEHICLE_WORDS * 4 + i * 4)),
        };
        let next = vehicle_handling::calculate_with_queries(
            &state,
            f32::from_bits(u(0)),
            if u(8) != 0 { Some(&queries) } else { None },
        )?;
        r[4..8].copy_from_slice(
            &next
                .damage_random_state
                .ok_or("lost damage RNG")?
                .to_le_bytes(),
        );
        for (i, w) in next.vehicle.iter().chain(next.physical.iter()).enumerate() {
            r[PREFIX + i * 4..PREFIX + i * 4 + 4].copy_from_slice(&w.to_le_bytes());
        }
    }
    let path = Path::new(&a[1]);
    if !path
        .parent()
        .ok_or("missing parent")?
        .canonicalize()?
        .starts_with(Path::new("local").canonicalize()?)
    {
        return Err("private output required".into());
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(&raw)?;
    println!("handling_damage_cases={n}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
