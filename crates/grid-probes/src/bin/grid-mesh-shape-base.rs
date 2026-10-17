//! Private whole mesh shape update diagnostic.
use std::{fs, io::Write, path::Path};
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-mesh-shape-base: {e}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args_os().skip(1).collect();
    if a.len() != 2 || fs::metadata(&a[0])?.len() > 4096 {
        return Err("usage: grid-mesh-shape-base CM04_INPUT local/OUTPUT".into());
    }
    let output = Path::new(&a[1]);
    if !output
        .parent()
        .ok_or("missing parent")?
        .canonicalize()?
        .starts_with(Path::new("local").canonicalize()?)
    {
        return Err("private output required".into());
    }
    let b = fs::read(&a[0])?;
    if b.len() < 192 || &b[..4] != b"CM04" {
        return Err("invalid header".into());
    }
    let word = |o| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let p = word(4) as usize;
    let t = word(8) as usize;
    let fill = word(12);
    if !(4..=80).contains(&p)
        || !(4..=110).contains(&t)
        || ![0, 0xcd].contains(&fill)
        || b.len() != 192 + p * 16 + t * 12
    {
        return Err("invalid framing".into());
    }
    let points: Vec<[f32; 4]> = (0..p)
        .map(|i| std::array::from_fn(|j| f32::from_bits(word(192 + i * 16 + j * 4))))
        .collect();
    let triangles: Vec<[u32; 3]> = (0..t)
        .map(|i| std::array::from_fn(|j| word(192 + p * 16 + i * 12 + j * 4)))
        .collect();
    let mesh = grid_world::collision_mesh::build_from_source(
        &points,
        &triangles,
        std::array::from_fn(|i| f32::from_bits(word(16 + i * 4))),
        std::array::from_fn(|i| f32::from_bits(word(32 + i * 4))),
        fill as u8,
    )?;
    let shape = mesh.construct_shape(
        b[40..172].try_into().unwrap(),
        std::array::from_fn(|i| word(172 + i * 4)),
        fill as u8,
    )?;
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    f.write_all(shape.prefix_bytes())?;
    f.write_all(shape.mesh().bytes())?;
    println!(
        "shape_bytes={} mesh_bytes={}",
        shape.prefix_bytes().len(),
        shape.mesh().bytes().len()
    );
    Ok(())
}
