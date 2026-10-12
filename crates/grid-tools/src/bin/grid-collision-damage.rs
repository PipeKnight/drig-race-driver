use grid_world::vehicle_publication::{PHYSICAL_WORDS, State, VEHICLE_WORDS};
use std::{fs, io::Write, path::Path};
const SIZE: usize = 32 + VEHICLE_WORDS * 4 + PHYSICAL_WORDS * 4;
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args_os().skip(1).collect();
    if a.len() != 2 || fs::metadata(&a[0])?.len() > 8 + 64 * SIZE as u64 {
        return Err("usage: grid-collision-damage DI01_INPUT local/OUTPUT".into());
    }
    let raw = fs::read(&a[0])?;
    if raw.len() < 8 || &raw[..4] != b"DI01" {
        return Err("invalid impact framing".into());
    }
    let n = u32::from_le_bytes(raw[4..8].try_into().unwrap()) as usize;
    if !(1..=64).contains(&n) || raw.len() != 8 + n * SIZE {
        return Err("invalid impact extent".into());
    }
    let mut out = raw[..8].to_vec();
    for r in raw[8..].as_chunks::<SIZE>().0 {
        let u = |o| u32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        if u(4) > 1 || u(12) != 0 {
            return Err("unsupported impact reserved words".into());
        }
        let state = State {
            vehicle: (0..VEHICLE_WORDS).map(|i| u(32 + i * 4)).collect(),
            physical: std::array::from_fn(|i| u(32 + VEHICLE_WORDS * 4 + i * 4)),
        };
        let result = grid_world::collision_damage::calculate(
            &state,
            std::array::from_fn(|j| f32::from_bits(u(16 + j * 4))),
            f32::from_bits(u(8)),
            u(4) != 0,
            u(0),
        )?;
        out.extend(u32::from(result.applied).to_le_bytes());
        out.extend(result.random_state.to_le_bytes());
        for value in result
            .state
            .vehicle
            .iter()
            .chain(result.state.physical.iter())
        {
            out.extend(value.to_le_bytes());
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
        .write_all(&out)?;
    println!("collision_damage_cases={n}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
