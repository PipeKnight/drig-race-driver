//! Private whole physical-pose diagnostic, PPC1. Opaque bytes are preserved.
use grid_world::physical_pose::{self, Input, Matrix};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn word(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}
fn vector(d: &[u8], o: usize) -> [f32; 4] {
    std::array::from_fn(|i| f32::from_bits(word(d, o + i * 4)))
}
fn matrix(d: &[u8], o: usize) -> Matrix {
    std::array::from_fn(|i| vector(d, o + i * 16))
}
fn store(d: &mut [u8], o: usize, m: Matrix) {
    for (i, v) in m.iter().flatten().enumerate() {
        d[o + i * 4..o + i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
}
fn take<'a>(
    d: &'a [u8],
    cursor: &mut usize,
    size: usize,
) -> Result<&'a [u8], Box<dyn std::error::Error>> {
    let end = cursor.checked_add(size).ok_or("length overflow")?;
    let result = d.get(*cursor..end).ok_or("truncated input")?;
    *cursor = end;
    Ok(result)
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-physical-pose INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 32 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let data = fs::read(&args[0])?;
    let mut cursor = 0;
    let header = take(&data, &mut cursor, 8)?;
    let count = word(header, 4) as usize;
    if word(header, 0) != 0x31435050 || !(1..=1024).contains(&count) {
        return Err("invalid header".into());
    }
    let mut output = header.to_vec();
    for _ in 0..count {
        let h = take(&data, &mut cursor, 8)?;
        let head = word(h, 0);
        let n = word(h, 4) as usize;
        if head > 1 || n > 64 {
            return Err("invalid case bounds".into());
        }
        let body = take(&data, &mut cursor, 0x424)?;
        let correction = take(&data, &mut cursor, 32)?;
        let raw = take(&data, &mut cursor, n * 0x220)?;
        if [0x328, 0x32c, 0x334, 0x338, 0x33c, 0x340]
            .iter()
            .any(|&o| word(body, o) != 0)
        {
            return Err("pointer-free body required".into());
        }
        let mut shapes = Vec::new();
        for s in raw.as_chunks::<0x220>().0 {
            let local = matrix(s, 0xf0);
            if word(s, 0xe0) != 0 || s[0x214] > 1 || local.iter().flatten().any(|v| !v.is_finite())
            {
                return Err("invalid pointer-free shape".into());
            }
            shapes.push((s[0x214] != 0).then_some(local));
        }
        let result = physical_pose::calculate(&Input {
            basis: matrix(body, 0x190),
            offset: vector(body, 0x230),
            local_matrices: [matrix(body, 0x90), matrix(body, 0x150)],
            group_head: head != 0,
            shapes,
            linear: vector(correction, 0),
            angular: vector(correction, 16),
        })?;
        let mut after = body.to_vec();
        for (offset, m) in [
            (0x190, result.corrected),
            (0x1d0, result.published),
            (0xd0, result.derived[0]),
            (0x110, result.derived[1]),
        ] {
            store(&mut after, offset, m);
        }
        output.extend(h);
        output.extend(after);
        for (before, m) in raw.as_chunks::<0x220>().0.iter().zip(result.shapes) {
            let mut after = *before;
            store(&mut after, 0x130, m);
            output.extend(after);
        }
    }
    if cursor != data.len() {
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
    println!("physical_pose_cases={count}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-physical-pose: {e}");
        std::process::exit(1);
    }
}
