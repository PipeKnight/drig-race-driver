//! Source-input packed candidate diagnostic, with no native output inputs.
use std::{fs, io::Write, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 || fs::metadata(&args[0])?.len() > 2 * 1024 * 1024 + 22044 {
        return Err("usage: grid-packed-candidates PC08_INPUT local/OUTPUT".into());
    }
    let b = fs::read(&args[0])?;
    if b.len() < 44 || &b[..4] != b"PC08" {
        return Err("invalid packed candidate framing".into());
    }
    let u = |o| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let f = |o| f32::from_bits(u(o));
    let size = u(4) as usize;
    let chunk_size = u(8) as usize;
    let capacity = u(20) as usize;
    let s = 44;
    if !(0x1680..=22000).contains(&size)
        || !(52..=2 * 1024 * 1024).contains(&chunk_size)
        || b.len() != 44 + size + chunk_size
        || u(12) > 16
        || u(16) > 1
        || capacity > 32768
        || u(40) > 255
    {
        return Err("invalid packed candidate extent".into());
    }
    let mesh_size = size - 0x1620;
    let m = s + 0x500;
    let body = s + size - 0x1120;
    if mesh_size > 16000
        || u(s + 0x10) != 7
        || u(s + 0x240) != 0x1500
        || [s + 0x184, s + 0x188, s + 0xdc, body + 0x320]
            .into_iter()
            .any(|o| u(o) != 0)
    {
        return Err("invalid packed candidate ownership".into());
    }
    let np = u(m) as usize;
    let nf = u(m + 4) as usize;
    let planes = 0x60 + np * 48;
    if !(4..=80).contains(&np)
        || !(4..=110).contains(&nf)
        || planes + nf * 32 > mesh_size
        || u(m + 8) != 1
        || u(m + 12) != 1
        || u(m + 0x54) != 0x60
        || u(m + 0x58) as usize != planes
    {
        return Err("invalid packed candidate topology".into());
    }
    let points: Vec<[f32; 4]> = (0..np)
        .map(|i| std::array::from_fn(|j| f(m + 0x60 + i * 32 + j * 4)))
        .collect();
    let faces: Vec<[u32; 3]> = (0..nf)
        .map(|i| std::array::from_fn(|j| u(m + planes + i * 32 + 16 + j * 4)))
        .collect();
    let mesh = grid_world::collision_mesh::build(&points, &faces, 0)?;
    if mesh.bytes().len() != mesh_size {
        return Err("unsupported packed candidate mesh extent".into());
    }
    let mut matrix: [[f32; 4]; 4] =
        std::array::from_fn(|i| std::array::from_fn(|j| f(s + 0x130 + i * 16 + j * 4)));
    let center: [f32; 4] = std::array::from_fn(|j| f(24 + j * 4));
    if center[3] != 1.
        || center.iter().any(|x| !x.is_finite() || x.abs() > 10000.)
        || (0..4).any(|j| !f(body + 0x1c0 + j * 4).is_finite())
    {
        return Err("invalid packed candidate center".into());
    }
    for j in 0..3 {
        matrix[3][j] += center[j] - f(body + 0x1c0 + j * 4);
    }
    let bounds = mesh.world_bounds(matrix, f(s + 0x220))?;
    let chunk = grid_formats::collision::read_vcqtc(&b[s + size..])?;
    if u(40) >> chunk.materials.len() != 0 {
        return Err("invalid packed candidate sheet mask".into());
    }
    let triangles: Vec<_> = chunk.triangles.iter().map(|t| t.indices).collect();
    let packed = grid_world::packed_query::Mesh {
        bounds: chunk.bounds,
        vertices: &chunk.positions,
        triangles: &triangles,
        nodes: &chunk.nodes,
        leaves: &chunk.references,
    };
    let mut bytes = Vec::new();
    let mut accepted = vec![0u32; capacity];
    let mut visited = vec![0u32; capacity];
    if u(12) != 0 && u(16) != 0 {
        let result = grid_world::packed_candidates::collect(&packed, bounds, capacity)?;
        for x in bounds.into_iter().flatten() {
            bytes.extend_from_slice(&x.to_le_bytes());
        }
        for n in [result.accepted.len(), result.visited.len()] {
            bytes.extend_from_slice(&(n as u32).to_le_bytes());
        }
        for x in result.stamps {
            bytes.extend_from_slice(&x.to_le_bytes());
        }
        bytes.extend_from_slice(&1u32.to_le_bytes());
        let to = u(s + size + 44);
        for (target, source) in [
            (&mut accepted, result.accepted),
            (&mut visited, result.visited),
        ] {
            for (slot, id) in target.iter_mut().zip(source) {
                *slot = 0x100000 + to + id as u32 * 4;
            }
        }
        bytes.extend_from_slice(&1u32.to_le_bytes());
    } else {
        bytes.resize(32 + 8 + 8192 + 4 + 4, 0);
    }
    for x in accepted.into_iter().chain(visited) {
        bytes.extend_from_slice(&x.to_le_bytes());
    }
    let target = Path::new(&args[1]);
    let parent = target.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(target.file_name().ok_or("missing name")?))?;
    out.write_all(&bytes)?;
    println!("packed_candidates bytes={}", bytes.len());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-packed-candidates: {e}");
        std::process::exit(1);
    }
}
