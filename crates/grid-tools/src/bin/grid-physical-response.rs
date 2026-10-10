//! Private normal-constraint diagnostic, PNR1.
use grid_world::physical_response::{self, Input, Row};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn word(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}
fn number(data: &[u8], offset: usize) -> f32 {
    f32::from_bits(word(data, offset))
}
fn take<'a>(
    data: &'a [u8],
    cursor: &mut usize,
    size: usize,
) -> Result<&'a [u8], Box<dyn std::error::Error>> {
    let end = cursor.checked_add(size).ok_or("length overflow")?;
    let result = data.get(*cursor..end).ok_or("truncated input")?;
    *cursor = end;
    Ok(result)
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-physical-response INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 32 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let data = fs::read(&args[0])?;
    let mut cursor = 0;
    let header = take(&data, &mut cursor, 8)?;
    let count = word(header, 4) as usize;
    if word(header, 0) != 0x31524e50 || !(1..=1024).contains(&count) {
        return Err("invalid header".into());
    }
    let mut output = header.to_vec();
    for _ in 0..count {
        let h = take(&data, &mut cursor, 24)?;
        let (n, two, bodies, iterations) = (
            word(h, 0) as usize,
            word(h, 4) as usize,
            word(h, 8) as usize,
            word(h, 12) as usize,
        );
        if n > 256 || two > n || bodies > 64 || iterations > 32 || word(h, 16) > 1 {
            return Err("invalid case bounds".into());
        }
        let raw = take(&data, &mut cursor, n * 192)?;
        let initial = take(&data, &mut cursor, bodies * 32)?;
        if initial
            .as_chunks::<4>()
            .0
            .iter()
            .any(|v| !number(v, 0).is_finite())
        {
            return Err("nonfinite initial workspace".into());
        }
        let rows = raw
            .as_chunks::<192>()
            .0
            .iter()
            .map(|r| Row {
                jacobian: std::array::from_fn(|v| {
                    std::array::from_fn(|i| number(r, v * 16 + i * 4))
                }),
                response: std::array::from_fn(|v| {
                    std::array::from_fn(|i| number(r, 0x40 + v * 16 + i * 4))
                }),
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
        let result = physical_response::calculate(&Input {
            rows,
            two_body_rows: two,
            body_count: bodies,
            iterations,
            normalize: word(h, 16) != 0,
            relaxation: number(h, 20),
        })?;
        output.extend((n as u32).to_le_bytes());
        output.extend((bodies as u32).to_le_bytes());
        for (before, r) in raw.as_chunks::<192>().0.iter().zip(&result.rows) {
            let mut after = *before;
            for (v, values) in r.jacobian.iter().enumerate() {
                for (i, value) in values.iter().enumerate() {
                    let offset = v * 16 + i * 4;
                    after[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                }
            }
            for (offset, value) in [
                (0x90, r.bias),
                (0x94, r.impulse),
                (0x98, r.cumulative),
                (0x9c, r.softness),
            ] {
                after[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
            output.extend(after);
        }
        for value in result.bodies.iter().flatten().flatten() {
            output.extend(value.to_le_bytes());
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
    println!("physical_response_cases={count}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-physical-response: {e}");
        std::process::exit(1);
    }
}
