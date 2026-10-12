//! Private complete collision-mesh construction diagnostic.
use std::{fs, io::Write, path::Path};
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-collision-mesh: {error}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 || fs::metadata(&args[0])?.len() > 4096 {
        return Err("usage: grid-collision-mesh CM01_INPUT local/OUTPUT".into());
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
    let b = fs::read(&args[0])?;
    if b.len() < 16 || &b[..4] != b"CM01" {
        return Err("invalid header".into());
    }
    let word = |o| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let p = word(4) as usize;
    let t = word(8) as usize;
    let fill = word(12);
    if !(4..=80).contains(&p)
        || !(4..=110).contains(&t)
        || ![0, 0xcd].contains(&fill)
        || b.len() != 16 + p * 16 + t * 12
    {
        return Err("invalid mesh framing".into());
    }
    let points: Vec<[f32; 4]> = (0..p)
        .map(|i| std::array::from_fn(|j| f32::from_bits(word(16 + i * 16 + j * 4))))
        .collect();
    let triangles: Vec<[u32; 3]> = (0..t)
        .map(|i| std::array::from_fn(|j| word(16 + p * 16 + i * 12 + j * 4)))
        .collect();
    let mesh = grid_world::collision_mesh::build(&points, &triangles, fill as u8)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(mesh.bytes())?;
    println!("mesh_bytes={}", mesh.bytes().len());
    Ok(())
}
