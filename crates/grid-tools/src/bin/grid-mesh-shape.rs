//! Private whole mesh shape update diagnostic.
use std::{fs, io::Write, path::Path};
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-mesh-shape: {e}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args_os().skip(1).collect();
    if a.len() != 2 || fs::metadata(&a[0])?.len() > 64 * 1024 {
        return Err("usage: grid-mesh-shape CM03_INPUT local/OUTPUT".into());
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
    if b.len() < 56 || !matches!(&b[..4], b"CM03" | b"CM07") {
        return Err("invalid header".into());
    }
    let word = |o| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let p = word(4) as usize;
    let t = word(8) as usize;
    let fill = word(12);
    let deformed = &b[..4] == b"CM07";
    let settings = if deformed { 40 + 0xa500 } else { 40 };
    let header = settings + 16;
    if !(4..=80).contains(&p)
        || !(4..=110).contains(&t)
        || ![0, 0xcd].contains(&fill)
        || b.len() != header + p * 16 + t * 12
        || word(settings + 8) > 255
        || word(settings + 12) > 1
    {
        return Err("invalid framing".into());
    }
    let points: Vec<[f32; 4]> = (0..p)
        .map(|i| std::array::from_fn(|j| f32::from_bits(word(header + i * 16 + j * 4))))
        .collect();
    let triangles: Vec<[u32; 3]> = (0..t)
        .map(|i| std::array::from_fn(|j| word(header + p * 16 + i * 12 + j * 4)))
        .collect();
    let mesh = if deformed {
        let dimensions: [usize; 3] = std::array::from_fn(|i| word(40 + 0xa414 + i * 4) as usize);
        if dimensions.iter().any(|d| !(2..=16).contains(d))
            || dimensions.iter().product::<usize>() > 400
        {
            return Err("invalid deformed mesh dimensions".into());
        }
        let positions: Vec<[f32; 4]> = (0..dimensions.iter().product::<usize>())
            .map(|i| std::array::from_fn(|a| f32::from_bits(word(40 + 0x2580 + i * 80 + a * 4))))
            .collect();
        grid_world::collision_mesh::build_from_deformed_source(
            &points,
            &triangles,
            std::array::from_fn(|i| f32::from_bits(word(16 + i * 4))),
            std::array::from_fn(|i| f32::from_bits(word(32 + i * 4))),
            &grid_world::deformation_grid::Grid {
                dimensions,
                spacing: f32::from_bits(word(40 + 0xa420)),
                minimum: std::array::from_fn(|a| f32::from_bits(word(40 + 0xa430 + a * 4))),
                positions: &positions,
            },
            fill as u8,
        )?
    } else {
        grid_world::collision_mesh::build_from_source(
            &points,
            &triangles,
            std::array::from_fn(|i| f32::from_bits(word(16 + i * 4))),
            std::array::from_fn(|i| f32::from_bits(word(32 + i * 4))),
            fill as u8,
        )?
    };
    let shape = mesh.initialize_shape(
        grid_world::collision_mesh::ShapeSettings {
            span: word(settings),
            scalar220: f32::from_bits(word(settings + 4)),
            layer: word(settings + 8) as u8,
            flag_c0: word(settings + 12) != 0,
        },
        fill as u8,
    )?;
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    f.write_all(shape.shape_bytes())?;
    f.write_all(shape.mesh().bytes())?;
    println!(
        "shape_bytes={} mesh_bytes={}",
        shape.shape_bytes().len(),
        shape.mesh().bytes().len()
    );
    Ok(())
}
