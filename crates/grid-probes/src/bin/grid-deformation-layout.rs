//! Private whole-layout comparison; no original or captured addresses followed.
use grid_world::deformation_layout::{self, Bounds, Parameters, WORDS};
use std::{fs::OpenOptions, io::Write, path::Path};
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-deformation-layout INPUT.bin local/OUTPUT.bin".into());
    }
    let output = Path::new(&args[1]);
    if !output
        .parent()
        .ok_or("missing parent")?
        .canonicalize()?
        .starts_with(Path::new("local").canonicalize()?)
        || std::fs::metadata(&args[0])?.len() > 8 * 1024 * 1024
    {
        return Err("bounded private output required".into());
    }
    let input = std::fs::read(&args[0])?;
    if input.len() < 8 || &input[..4] != b"GL01" || !(1..=128).contains(&word(&input, 4)) {
        return Err("invalid deformation layout framing".into());
    }
    let mut results = input[..8].to_vec();
    let mut cursor = 8;
    for _ in 0..word(&input, 4) {
        if input.len() - cursor < 32 + WORDS * 4 {
            return Err("truncated layout case".into());
        }
        let prefix = &input[cursor..cursor + 32];
        let count = word(prefix, 0) as usize;
        if count > 32 || word(prefix, 8) > 255 {
            return Err("invalid layout case fields".into());
        }
        cursor += 32;
        let descriptor: Vec<_> = (0..WORDS).map(|i| word(&input, cursor + i * 4)).collect();
        cursor += WORDS * 4;
        if input.len() - cursor < count * 32 {
            return Err("truncated bounds".into());
        }
        let bounds: Vec<_> = (0..count)
            .map(|i| Bounds {
                minimum: std::array::from_fn(|j| {
                    f32::from_bits(word(&input, cursor + i * 32 + j * 4))
                }),
                maximum: std::array::from_fn(|j| {
                    f32::from_bits(word(&input, cursor + i * 32 + 16 + j * 4))
                }),
            })
            .collect();
        cursor += count * 32;
        let f = |at| f32::from_bits(word(prefix, at));
        let answer = deformation_layout::calculate(
            &descriptor,
            &bounds,
            word(prefix, 4),
            Parameters {
                margin: f(12),
                spacing_factor: f(16),
                spacing_growth: f(20),
                fallback_xy: f(24),
                fallback_z: f(28),
                flag_a4b0: word(prefix, 8) as u8,
            },
        )?;
        for w in answer.descriptor {
            results.extend(w.to_le_bytes());
        }
    }
    if cursor != input.len() {
        return Err("trailing deformation layout input".into());
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&results)?;
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-deformation-layout: {e}");
        std::process::exit(1);
    }
}
