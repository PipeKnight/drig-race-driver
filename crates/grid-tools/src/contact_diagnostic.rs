//! Fresh source-input packed contact diagnostic, with no native output inputs.
use std::{fs, io::Write, path::Path};
pub fn run(cqtc: bool) -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 || fs::metadata(&args[0])?.len() > 2 * 1024 * 1024 + 22044 {
        return Err("usage: grid-packed-contacts PC08_INPUT local/OUTPUT PRIVATE_CD01".into());
    }
    if fs::metadata(&args[2])?.len() != 712 {
        return Err("invalid private sampling extent".into());
    }
    let table = fs::read(&args[2])?;
    if &table[..8] != b"CD01\x2c\0\0\0" {
        return Err("invalid private sampling header".into());
    }
    let directions = std::array::from_fn(|i| {
        std::array::from_fn(|j| {
            f32::from_le_bytes(
                table[8 + i * 16 + j * 4..12 + i * 16 + j * 4]
                    .try_into()
                    .unwrap(),
            )
        })
    });
    let b = fs::read(&args[0])?;
    if b.len() < 44 || &b[..4] != if cqtc { b"CC01" } else { b"PC08" } {
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
    let built = grid_world::collision_mesh::build(&points, &faces, 0)?;
    if built.bytes().len() != mesh_size {
        return Err("unsupported packed candidate mesh extent".into());
    }
    let initialized = built.initialize_shape(
        grid_world::collision_mesh::ShapeSettings {
            span: mesh_size as u32,
            scalar220: f(s + 0x220),
            layer: 0,
            flag_c0: false,
        },
        0,
    )?;
    let mesh = initialized.mesh();
    for i in 0..nf {
        for (j, value) in mesh.face_normal(i).unwrap().into_iter().enumerate() {
            if value.to_bits() != u(m + planes + i * 32 + j * 4) {
                return Err("invalid initialized face normal".into());
            }
        }
    }
    // Admit exactly the owned source topology rebuilt here, including adjacency.
    let mw = |o| u32::from_le_bytes(mesh.bytes()[o..o + 4].try_into().unwrap());
    for i in 0..np {
        let v = 0x60 + i * 32;
        let aux = 0x60 + np * 32 + i * 16;
        for o in [v + 0x10, v + 0x14, v + 0x18, aux, aux + 4] {
            if u(m + o) != mw(o) {
                return Err("invalid source adjacency".into());
            }
        }
        for (o, n) in [
            (mw(aux + 4) as usize, mw(aux) as usize),
            (mw(v + 0x18) as usize, mw(v + 0x14) as usize),
        ] {
            for j in 0..n {
                if u(m + o + j * 4) != mw(o + j * 4) {
                    return Err("invalid source adjacency record".into());
                }
            }
        }
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
    let settings = grid_world::packed_contacts::Settings {
        query: grid_world::convex_distance::ContactSettings {
            margin: f(s + 0x220),
            directions,
        },
        shape_margin: 0.,
        contact_capacity: u(12) as usize,
        cache_enabled: u(16) != 0,
        candidate_capacity: capacity,
        sheet_mask: u(40) as u8,
        correction_bounds: [f32::MAX; 4],
    };
    let generated = if cqtc {
        let boundary = crate::cqtc_geometry::Geometry::read(&b[s + size..])?;
        if u(40) >> boundary.material_keys.len() != 0 {
            return Err("invalid CQTC sheet mask".into());
        }
        boundary.contacts(mesh, matrix, &settings)?
    } else {
        let chunk = grid_formats::collision::read_vcqtc(&b[s + size..])?;
        if u(40) >> chunk.materials.len() != 0 {
            return Err("invalid packed candidate sheet mask".into());
        }
        let triangles: Vec<_> = chunk.triangles.iter().map(|t| t.indices).collect();
        let materials: Vec<_> = chunk.triangles.iter().map(|t| t.material).collect();
        grid_world::packed_contacts::calculate(
            mesh,
            matrix,
            &grid_world::packed_query::Mesh {
                bounds: chunk.bounds,
                vertices: &chunk.positions,
                triangles: &triangles,
                nodes: &chunk.nodes,
                leaves: &chunk.references,
            },
            &materials,
            &settings,
        )?
    };
    let mut bytes = Vec::new();
    let mut accepted = vec![0u32; capacity];
    let mut visited = vec![0u32; capacity];
    if let Some(result) = generated.candidates {
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
                *slot = 0x100000 + to + id as u32 * if cqtc { 7 } else { 4 };
            }
        }
        bytes.extend_from_slice(&1u32.to_le_bytes());
    } else {
        bytes.resize(32 + 8 + 8192 + 4 + 4, 0);
    }
    for x in accepted.into_iter().chain(visited) {
        bytes.extend_from_slice(&x.to_le_bytes());
    }
    bytes.extend_from_slice(&(generated.contacts.len() as u32).to_le_bytes());
    for x in generated.direction {
        bytes.extend_from_slice(&x.to_le_bytes());
    }
    let mut arena = [0u8; 0x800];
    let put = |b: &mut [u8], o: usize, x: u32| b[o..o + 4].copy_from_slice(&x.to_le_bytes());
    for i in 0..16 {
        let c = &mut arena[i * 0x80..(i + 1) * 0x80];
        put(c, 0, 0x7200);
        c[0xf] = 1;
        put(c, 0x1c, u32::MAX);
        put(c, 0x20, u32::MAX);
        put(c, 0x24, 1);
        if let Some(contact) = generated.contacts.get(i) {
            for (o, v) in [
                (4, 0xa000),
                (0x28, 0x1000),
                (0x2c, 0xc000),
                (0x30, 0x1420),
                (0x34, 0x1420),
                (0x38, contact.penetration.to_bits()),
                (0x3c, f32::MAX.to_bits()),
            ] {
                put(c, o, v);
            }
            for (o, p) in [(0x50, contact.point), (0x60, contact.normal)] {
                for (j, x) in p.into_iter().enumerate() {
                    put(c, o + j * 4, x.to_bits());
                }
            }
            c[0x71] = contact.flipped as u8;
            c[0x73] = 1;
        }
    }
    bytes.extend_from_slice(&arena);
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
    println!("packed_contacts bytes={}", bytes.len());
    Ok(())
}
