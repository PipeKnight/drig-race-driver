//! Private correction-row diagnostic, CBI1; preserves opaque row words.
use grid_world::contact_bias::{self, Constraint, Input};
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
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-contact-bias INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 16 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let data = fs::read(&args[0])?;
    if data.len() < 8 || word(&data, 0) != 0x31494243 {
        return Err("invalid header".into());
    }
    let count = word(&data, 4) as usize;
    if !(1..=16384).contains(&count) || data.len() != 8 + count * 220 {
        return Err("invalid case framing".into());
    }
    let mut output = data[..8].to_vec();
    for c in data[8..].as_chunks::<220>().0 {
        if [0, 4, 8].into_iter().any(|i| word(c, i) > 1) {
            return Err("invalid boolean".into());
        }
        let mut row = c[28..].to_vec();
        let r = contact_bias::calculate(&Input {
            two_bodies: word(c, 0) != 0,
            disabled: word(c, 4) != 0,
            active: word(c, 8) != 0,
            accumulated: number(c, 12),
            threshold: number(c, 16),
            scale: number(c, 20),
            upper_limit: number(c, 24),
            constraint: Constraint {
                vectors: std::array::from_fn(|v| {
                    std::array::from_fn(|i| number(&row, 0x40 + v * 16 + i * 4))
                }),
                lower: number(&row, 0x88),
                upper: number(&row, 0x8c),
                bias: number(&row, 0x90),
            },
        })?;
        for (v, values) in r.constraint.vectors.iter().enumerate() {
            for (i, value) in values.iter().enumerate() {
                let offset = 0x40 + v * 16 + i * 4;
                row[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        for (offset, value) in [
            (0x88, r.constraint.lower),
            (0x8c, r.constraint.upper),
            (0x90, r.constraint.bias),
        ] {
            row[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        output.extend((r.cursor_advance as u32).to_le_bytes());
        output.extend(u32::from(r.active).to_le_bytes());
        output.extend(row);
    }
    let path = Path::new(&args[1]);
    let parent = path.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(path.file_name().ok_or("missing name")?))?;
    file.write_all(&output)?;
    file.flush()?;
    println!("contact_bias_cases={count}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-contact-bias: {error}");
        std::process::exit(1);
    }
}
