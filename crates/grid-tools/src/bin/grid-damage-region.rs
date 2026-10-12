use std::{fs, io::Write, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 || fs::metadata(&args[0])?.len() > 8 + 16384 * 128 {
        return Err("usage: grid-damage-region DM01_INPUT local/OUTPUT".into());
    }
    let b = fs::read(&args[0])?;
    if b.len() < 8 || &b[..4] != b"DM01" {
        return Err("invalid damage region framing".into());
    }
    let count = u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize;
    if !(1..=16384).contains(&count) || b.len() != 8 + count * 128 {
        return Err("invalid damage region extent".into());
    }
    let mut output = Vec::new();
    for r in b[8..].as_chunks::<128>().0 {
        let u = |o| u32::from_le_bytes(r[o..o + 4].try_into().unwrap());
        let f = |o| f32::from_bits(u(o));
        if u(12) > 1
            || [76, 80, 84, 88, 92, 108, 116, 120, 124]
                .into_iter()
                .any(|o| u(o) != 0)
        {
            return Err("unsupported damage region reserved words".into());
        }
        let result = grid_world::damage_region::calculate(grid_world::damage_region::Input {
            previous: f(0),
            impact: f(4),
            gain: f(8),
            enabled: u(12) != 0,
            point: std::array::from_fn(|j| f(16 + j * 4)),
            center: std::array::from_fn(|j| f(32 + j * 4)),
            half: std::array::from_fn(|j| f(48 + j * 4)),
            coefficients: std::array::from_fn(|j| f(64 + j * 4)),
            radius: f(96),
            factors: [f(100), f(104)],
            random_state: u(112),
        })?;
        output.extend(result.level.to_le_bytes());
        output.extend(result.random_state.to_le_bytes());
    }
    let path = Path::new(&args[1]);
    if !path
        .parent()
        .ok_or("missing output parent")?
        .canonicalize()?
        .starts_with(Path::new("local").canonicalize()?)
    {
        return Err("private output required".into());
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(&output)?;
    println!("damage_regions={count}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
