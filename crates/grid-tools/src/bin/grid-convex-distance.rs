//! Private source-input diagnostic; never consumes a native output as input.
use std::{fs, io::Write, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !(2..=3).contains(&args.len()) || fs::metadata(&args[0])?.len() > 2 * 1024 * 1024 + 23580 {
        return Err(
            "usage: grid-convex-distance PC03_TO_PC06_INPUT local/OUTPUT [PRIVATE_CD01]".into(),
        );
    }
    let b = fs::read(&args[0])?;
    if b.len() < 44 || !matches!(&b[..4], b"PC03" | b"PC04" | b"PC05" | b"PC06") {
        return Err("invalid distance framing".into());
    }
    let word = |o| u32::from_le_bytes(b[o..o + 4].try_into().unwrap());
    let float = |o| f32::from_bits(word(o));
    let sequence = matches!(&b[..4], b"PC05" | b"PC06");
    let outer = matches!(&b[..4], b"PC03" | b"PC06");
    if args.len() != if outer { 3 } else { 2 } {
        return Err("outer query requires private CD01 sampling data".into());
    }
    let sampling = if outer {
        if fs::metadata(&args[2])?.len() != 712 {
            return Err("invalid sampling extent".into());
        }
        let table = fs::read(&args[2])?;
        if &table[..8] != b"CD01\x2c\0\0\0" {
            return Err("invalid sampling framing".into());
        }
        Some(std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                f32::from_le_bytes(
                    table[8 + i * 16 + j * 4..12 + i * 16 + j * 4]
                        .try_into()
                        .unwrap(),
                )
            })
        }))
    } else {
        None
    };
    let count = if sequence { word(40) as usize } else { 1 };
    if !(1..=64).contains(&count) {
        return Err("invalid distance sequence count".into());
    }
    let s = if sequence { 44 + count * 24 } else { 60 };
    let size = word(4) as usize;
    let chunk_size = word(8) as usize;
    if !(0x1680..=22000).contains(&size)
        || !(52..=2 * 1024 * 1024).contains(&chunk_size)
        || b.len() != s + size + chunk_size
        || word(12) > 16
        || word(16) > 1
        || word(20) > 32768
    {
        return Err("invalid distance extent".into());
    }
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
    if center.iter().any(|v| !v.is_finite() || v.abs() > 10000.) || center[3] != 1. {
        return Err("invalid distance controls".into());
    }
    let mut requests = Vec::with_capacity(count);
    for i in 0..count {
        let o = if sequence { 44 + i * 24 } else { 40 };
        let mode = if sequence { word(o + 4) } else { 1 };
        let d = if sequence { o + 8 } else { o + 4 };
        let direction: [f32; 4] = std::array::from_fn(|j| float(d + j * 4));
        if mode > 1
            || (i == 0 && mode != 1)
            || direction.iter().any(|v| !v.is_finite() || v.abs() > 10000.)
            || direction[3] != 0.
        {
            return Err("invalid distance sequence controls".into());
        }
        requests.push((word(o), mode, direction));
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
    let mut context = grid_world::convex_distance::Context::new();
    let mut direction = [0.; 4];
    let mut records = Vec::with_capacity(count * 448 + 4);
    if sequence {
        records.extend_from_slice(&(count as u32).to_le_bytes());
    }
    for (index, mode, supplied_direction) in requests {
        if mode == 1 {
            direction = supplied_direction;
        }
        let ids = chunk
            .triangles
            .get(index as usize)
            .ok_or("triangle out of range")?
            .indices;
        let triangle = ids.map(|i| {
            let p = chunk.positions[i as usize];
            [p[0], p[1], p[2], 1.]
        });
        let mut extras = None;
        let (returned, flags, witnesses, support_vertices, simplex_bytes) =
            if let Some(directions) = sampling {
                let result = context.contact(
                    &mesh,
                    matrix,
                    triangle,
                    direction,
                    &grid_world::convex_distance::ContactSettings {
                        margin: float(s + 0x220),
                        directions,
                    },
                )?;
                extras = Some(result.extra_directions);
                (
                    result.direction,
                    [false; 2],
                    result.witnesses,
                    result.support_vertices,
                    result.simplex_bytes,
                )
            } else {
                let result = context.query(&mesh, matrix, triangle, float(s + 0x220), direction)?;
                (
                    result.direction,
                    result.flags,
                    result.witnesses,
                    result.support_vertices,
                    result.simplex_bytes,
                )
            };
        direction = returned;
        let mut query = [0u8; 0xa0];
        query[..4].copy_from_slice(&index.to_le_bytes());
        query[8] = flags[0] as u8;
        query[9] = flags[1] as u8;
        if outer {
            query[4] = witnesses.is_some() as u8;
        }
        query[0x20..0x40].fill(0xcd);
        let put = |q: &mut [u8], o: usize, v: u32| q[o..o + 4].copy_from_slice(&v.to_le_bytes());
        for (j, v) in returned.into_iter().enumerate() {
            put(&mut query, 0x10 + j * 4, v.to_bits());
        }
        if let Some(witnesses) = witnesses {
            for (i, p) in witnesses.into_iter().enumerate() {
                for (j, v) in p.into_iter().enumerate() {
                    put(&mut query, 0x20 + i * 16 + j * 4, v.to_bits());
                }
            }
        }
        put(&mut query, 0x40, 0x1000);
        for (j, i) in support_vertices.into_iter().enumerate() {
            put(&mut query, 0x44 + j * 4, 0x30060 + i as u32 * 32);
        }
        for (i, p) in triangle.into_iter().enumerate() {
            for (j, v) in p.into_iter().enumerate() {
                put(&mut query, 0x60 + i * 16 + j * 4, v.to_bits());
            }
        }
        records.extend_from_slice(&simplex_bytes);
        records.extend_from_slice(&query);
        if let Some(extras) = extras {
            for v in extras.into_iter().flatten() {
                records.extend_from_slice(&v.to_le_bytes());
            }
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
    file.write_all(&records)?;
    println!("convex_distance bytes={}", records.len());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-convex-distance: {e}");
        std::process::exit(1);
    }
}
