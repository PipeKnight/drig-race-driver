//! Private whole optional-pass diagnostic with impact pools only, IPC1.
use grid_world::{
    impact_correction::{self, Contact, Input},
    physical_pose::{self, Matrix},
    physical_response::Row,
};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn word(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}
fn number(d: &[u8], o: usize) -> f32 {
    f32::from_bits(word(d, o))
}
fn vector(d: &[u8], o: usize) -> [f32; 4] {
    std::array::from_fn(|i| number(d, o + i * 4))
}
fn matrix(d: &[u8], o: usize) -> Matrix {
    std::array::from_fn(|i| vector(d, o + i * 16))
}
fn store(d: &mut [u8], o: usize, m: Matrix) {
    for (i, v) in m.iter().flatten().enumerate() {
        d[o + i * 4..o + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
}
fn take<'a>(d: &'a [u8], p: &mut usize, n: usize) -> Result<&'a [u8], Box<dyn std::error::Error>> {
    let end = p.checked_add(n).ok_or("length overflow")?;
    let v = d.get(*p..end).ok_or("truncated input")?;
    *p = end;
    Ok(v)
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-impact-correction INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 32 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let data = fs::read(&args[0])?;
    let mut p = 0;
    let h = take(&data, &mut p, 8)?;
    let count = word(h, 4) as usize;
    if word(h, 0) != 0x31435049 || !(1..=1024).contains(&count) {
        return Err("invalid header".into());
    }
    let mut output = h.to_vec();
    for _ in 0..count {
        let h = take(&data, &mut p, 40)?;
        let n = word(h, 0) as usize;
        let two = word(h, 4) as usize;
        let bodies = word(h, 8) as usize;
        let iterations = word(h, 12) as usize;
        if n > 256 || two > n || bodies > 16 || iterations > 32 {
            return Err("invalid bounds".into());
        }
        let raw = take(&data, &mut p, n * 192)?;
        let contacts = take(&data, &mut p, n * 8)?;
        let workspace = take(&data, &mut p, bodies * 32)?;
        let mut contact_values = Vec::new();
        for c in contacts.as_chunks::<8>().0 {
            if word(c, 0) > 1 {
                return Err("invalid contact flag".into());
            }
            contact_values.push(Contact {
                disabled: word(c, 0) != 0,
                accumulated: number(c, 4),
            });
        }
        let rows = raw
            .as_chunks::<192>()
            .0
            .iter()
            .map(|r| Row {
                jacobian: matrix(r, 0),
                response: matrix(r, 0x40),
                softness_input: number(r, 0x84),
                lower: number(r, 0x88),
                upper: number(r, 0x8c),
                bias: number(r, 0x90),
                impulse: number(r, 0x94),
                cumulative: number(r, 0x98),
                softness: number(r, 0x9c),
                bodies: [word(r, 0xa0) as usize, word(r, 0xa4) as usize],
                normalization: number(r, 0xac),
                denominator: number(r, 0xb0),
                batch_len: word(r, 0xb4) as usize,
            })
            .collect();
        let mut poses = Vec::new();
        let mut original = Vec::new();
        for _ in 0..bodies {
            let ph = take(&data, &mut p, 8)?;
            let shape_count = word(ph, 4) as usize;
            if word(ph, 0) > 1 || shape_count > 8 {
                return Err("invalid pose bounds".into());
            }
            let body = take(&data, &mut p, 0x424)?;
            let shapes = take(&data, &mut p, shape_count * 0x220)?;
            if [0x328, 0x32c, 0x334, 0x338, 0x33c, 0x340]
                .iter()
                .any(|&o| word(body, o) != 0)
            {
                return Err("pointer-free body required".into());
            }
            let mut local_shapes = Vec::new();
            for s in shapes.as_chunks::<0x220>().0 {
                let m = matrix(s, 0xf0);
                if word(s, 0xe0) != 0 || s[0x214] > 1 || m.iter().flatten().any(|v| !v.is_finite())
                {
                    return Err("invalid pointer-free shape".into());
                }
                local_shapes.push((s[0x214] != 0).then_some(m));
            }
            poses.push(physical_pose::Input {
                basis: matrix(body, 0x190),
                offset: vector(body, 0x230),
                local_matrices: [matrix(body, 0x90), matrix(body, 0x150)],
                group_head: word(ph, 0) != 0,
                shapes: local_shapes,
                linear: [0.; 4],
                angular: [0.; 4],
            });
            original.push((ph, body, shapes));
        }
        let result = impact_correction::calculate(&Input {
            rows,
            two_body_contacts: contact_values[..two].to_vec(),
            single_body_contacts: contact_values[two..].to_vec(),
            poses,
            workspace: workspace
                .as_chunks::<32>()
                .0
                .iter()
                .map(|w| [vector(w, 0), vector(w, 16)])
                .collect(),
            span_ends: [word(h, 16) as usize, word(h, 20) as usize],
            iterations,
            softness: number(h, 24),
            scale: number(h, 28),
            threshold: number(h, 32),
            relaxation: number(h, 36),
        })?;
        for value in [n, bodies, result.span_ends[0], result.span_ends[1]] {
            output.extend((value as u32).to_le_bytes());
        }
        for (before, r) in raw.as_chunks::<192>().0.iter().zip(result.rows) {
            let mut after = *before;
            store(&mut after, 0, r.jacobian);
            store(&mut after, 0x40, r.response);
            if result.rows_reset {
                after[0x80..0x84].fill(0);
            }
            for (o, v) in [
                (0x84, r.softness_input),
                (0x88, r.lower),
                (0x8c, r.upper),
                (0x90, r.bias),
                (0x94, r.impulse),
                (0x98, r.cumulative),
                (0x9c, r.softness),
            ] {
                after[o..o + 4].copy_from_slice(&v.to_le_bytes());
            }
            output.extend(after);
        }
        for v in result.workspace.iter().flatten().flatten() {
            output.extend(v.to_le_bytes());
        }
        for ((ph, before, shapes), pose) in original.into_iter().zip(result.poses) {
            let mut after = before.to_vec();
            output.extend(ph);
            if let Some(r) = pose {
                for (o, m) in [
                    (0x190, r.corrected),
                    (0x1d0, r.published),
                    (0xd0, r.derived[0]),
                    (0x110, r.derived[1]),
                ] {
                    store(&mut after, o, m);
                }
                output.extend(after);
                for (s, m) in shapes.as_chunks::<0x220>().0.iter().zip(r.shapes) {
                    let mut changed = *s;
                    store(&mut changed, 0x130, m);
                    output.extend(changed);
                }
            } else {
                output.extend(after);
                output.extend(shapes);
            }
        }
    }
    if p != data.len() {
        return Err("trailing input".into());
    }
    let path = Path::new(&args[1]);
    let parent = path.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(path.file_name().ok_or("missing name")?))?;
    f.write_all(&output)?;
    f.flush()?;
    println!("impact_correction_cases={count}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-impact-correction: {e}");
        std::process::exit(1);
    }
}
