//! Private source-input diagnostic; never consumes a native output as input.
use std::{fs, io::Write, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 || fs::metadata(&args[0])?.len() > 2 * 1024 * 1024 + 22060 {
        return Err("usage: grid-convex-distance PC04_INPUT local/OUTPUT".into());
    }
    let b = fs::read(&args[0])?;
    if b.len() < 60 || &b[..4] != b"PC04" {
        return Err("invalid distance framing".into());
    }
    let word = |o| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let float = |o| f32::from_bits(word(o));
    let size = word(4) as usize;
    let chunk_size = word(8) as usize;
    if !(0x1680..=22000).contains(&size)
        || !(52..=2 * 1024 * 1024).contains(&chunk_size)
        || b.len() != 60 + size + chunk_size
        || word(12) > 16
        || word(16) > 1
        || word(20) > 32768
    {
        return Err("invalid distance extent".into());
    }
    let s = 60;
    let mesh_size = size - 0x1620;
    let body = s + 0x500 + mesh_size;
    if !(0x60..=16000).contains(&mesh_size)
        || word(s + 0x10) != 7
        || word(s + 0x240) != 0x1500
        || [s + 0x184, s + 0x188, s + 0xdc, body + 0x320]
            .into_iter()
            .any(|o| word(o) != 0)
    {
        return Err("invalid distance mesh ownership".into());
    }
    let center: [f32; 4] = std::array::from_fn(|j| float(24 + j * 4));
    let direction: [f32; 4] = std::array::from_fn(|j| float(44 + j * 4));
    if center
        .iter()
        .chain(direction.iter())
        .any(|v| !v.is_finite() || v.abs() > 10000.)
        || center[3] != 1.
        || direction[3] != 0.
    {
        return Err("invalid distance controls".into());
    }
    let m = s + 0x500;
    let points = word(m) as usize;
    let faces = word(m + 4) as usize;
    let face_start = 0x60 + points * 48;
    if !(4..=80).contains(&points)
        || !(4..=110).contains(&faces)
        || word(m + 8) != 1
        || word(m + 12) != 1
        || face_start + faces * 32 > mesh_size
        || word(m + 0x54) != 0x60
        || word(m + 0x58) as usize != face_start
    {
        return Err("invalid distance topology".into());
    }
    let vertices: Vec<[f32; 4]> = (0..points)
        .map(|i| std::array::from_fn(|j| float(m + 0x60 + i * 32 + j * 4)))
        .collect();
    let triangles: Vec<[u32; 3]> = (0..faces)
        .map(|i| std::array::from_fn(|j| word(m + face_start + i * 32 + 16 + j * 4)))
        .collect();
    let mesh = grid_world::collision_mesh::build(&vertices, &triangles, 0)?;
    if mesh.bytes().len() != mesh_size {
        return Err("unsupported distance mesh span".into());
    }
    // Only the exact owned topology produced by Mesh::build is admitted.
    let mesh_word = |o| u32::from_le_bytes(mesh.bytes()[o..o + 4].try_into().unwrap());
    for i in 0..points {
        let v = 0x60 + i * 32;
        let aux = 0x60 + points * 32 + i * 16;
        for o in [v + 0x10, v + 0x14, v + 0x18, aux, aux + 4] {
            if word(m + o) != mesh_word(o) {
                return Err("invalid distance adjacency".into());
            }
        }
        for (o, n) in [
            (mesh_word(aux + 4) as usize, mesh_word(aux) as usize),
            (mesh_word(v + 0x18) as usize, mesh_word(v + 0x14) as usize),
        ] {
            for j in 0..n {
                if word(m + o + j * 4) != mesh_word(o + j * 4) {
                    return Err("invalid distance adjacency records".into());
                }
            }
        }
    }
    let mut matrix: [[f32; 4]; 4] =
        std::array::from_fn(|i| std::array::from_fn(|j| float(s + 0x130 + i * 16 + j * 4)));
    if matrix
        .iter()
        .flatten()
        .any(|v| !v.is_finite() || v.abs() > 10000.)
        || (0..4).any(|j| !float(body + 0x1c0 + j * 4).is_finite())
    {
        return Err("invalid distance source pose".into());
    }
    for j in 0..3 {
        matrix[3][j] += center[j] - float(body + 0x1c0 + j * 4);
    }
    let chunk = grid_formats::collision::read_vcqtc(&b[s + size..])?;
    let ids = chunk
        .triangles
        .get(word(40) as usize)
        .ok_or("triangle out of range")?
        .indices;
    let triangle = ids.map(|i| {
        let p = chunk.positions[i as usize];
        [p[0], p[1], p[2], 1.]
    });
    let result = grid_world::convex_distance::calculate(
        &mesh,
        matrix,
        triangle,
        float(s + 0x220),
        direction,
    )?;
    let mut query = [0u8; 0xa0];
    query[..4].copy_from_slice(&word(40).to_le_bytes());
    query[8] = result.flags[0] as u8;
    query[9] = result.flags[1] as u8;
    query[0x20..0x40].fill(0xcd);
    let put = |q: &mut [u8], o: usize, v: u32| q[o..o + 4].copy_from_slice(&v.to_le_bytes());
    for (j, v) in result.direction.into_iter().enumerate() {
        put(&mut query, 0x10 + j * 4, v.to_bits());
    }
    if let Some(witnesses) = result.witnesses {
        for (i, p) in witnesses.into_iter().enumerate() {
            for (j, v) in p.into_iter().enumerate() {
                put(&mut query, 0x20 + i * 16 + j * 4, v.to_bits());
            }
        }
    }
    put(&mut query, 0x40, 0x1000);
    for (j, i) in result.support_vertices.into_iter().enumerate() {
        put(&mut query, 0x44 + j * 4, 0x30060 + i as u32 * 32);
    }
    for (i, p) in triangle.into_iter().enumerate() {
        for (j, v) in p.into_iter().enumerate() {
            put(&mut query, 0x60 + i * 16 + j * 4, v.to_bits());
        }
    }
    let output = Path::new(&args[1]);
    let parent = output.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(output.file_name().ok_or("missing output name")?))?;
    file.write_all(&result.simplex_bytes)?;
    file.write_all(&query)?;
    println!("convex_distance bytes=448");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-convex-distance: {e}");
        std::process::exit(1);
    }
}
