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
    if args.len() != 2 || fs::metadata(&args[0])?.len() > 64 * 1024 {
        return Err("usage: grid-collision-mesh CM01_OR_CM02_INPUT local/OUTPUT".into());
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
    if b.len() < 16 || !matches!(&b[..4], b"CM01" | b"CM02" | b"CM06") {
        return Err("invalid header".into());
    }
    let word = |o| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let p = word(4) as usize;
    let t = word(8) as usize;
    let fill = word(12);
    let deformed = &b[..4] == b"CM06";
    let adjusted = &b[..4] == b"CM02" || deformed;
    let header = if deformed {
        40 + 0xa500
    } else if adjusted {
        40
    } else {
        16
    };
    if !(4..=80).contains(&p)
        || !(4..=110).contains(&t)
        || ![0, 0xcd].contains(&fill)
        || b.len() != header + p * 16 + t * 12
    {
        return Err("invalid mesh framing".into());
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
    } else if adjusted {
        grid_world::collision_mesh::build_from_source(
            &points,
            &triangles,
            std::array::from_fn(|i| f32::from_bits(word(16 + i * 4))),
            std::array::from_fn(|i| f32::from_bits(word(32 + i * 4))),
            fill as u8,
        )?
    } else {
        grid_world::collision_mesh::build(&points, &triangles, fill as u8)?
    };
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(mesh.bytes())?;
    println!("mesh_bytes={}", mesh.bytes().len());
    Ok(())
}
