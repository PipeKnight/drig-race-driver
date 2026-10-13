//! Private source-supplied whole initialization comparison; no pointers followed.
use grid_world::{
    deformation_initialization::{self, SETTINGS_WORDS},
    deformation_layout::{Bounds, Parameters, WORDS},
};
use std::{fs::OpenOptions, io::Write, path::Path};
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-deformation-initialization INPUT.bin local/OUTPUT.bin".into());
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
    if input.len() < 8 || &input[..4] != b"GI01" || !(1..=32).contains(&word(&input, 4)) {
        return Err("invalid initialization framing".into());
    }
    let mut cursor = 8;
    let mut results = input[..8].to_vec();
    for _ in 0..word(&input, 4) {
        if input.len() - cursor < 32 + WORDS * 4 + SETTINGS_WORDS * 4 {
            return Err("truncated initialization".into());
        }
        let prefix = &input[cursor..cursor + 32];
        let count = word(prefix, 0) as usize;
        if count > 32 || word(prefix, 4) != 0 || word(prefix, 8) > 255 {
            return Err("invalid initialization prefix".into());
        }
        cursor += 32;
        let descriptor: Vec<_> = (0..WORDS).map(|i| word(&input, cursor + i * 4)).collect();
        cursor += WORDS * 4;
        if input.len() - cursor < count * 32 + SETTINGS_WORDS * 4 || descriptor[0xa4f8 / 4] != 0 {
            return Err("invalid initialization context".into());
        }
        let bounds: Vec<_> = (0..count)
            .map(|i| Bounds {
                minimum: std::array::from_fn(|a| {
                    f32::from_bits(word(&input, cursor + i * 32 + a * 4))
                }),
                maximum: std::array::from_fn(|a| {
                    f32::from_bits(word(&input, cursor + i * 32 + 16 + a * 4))
                }),
            })
            .collect();
        cursor += count * 32;
        let settings: Vec<_> = (0..SETTINGS_WORDS)
            .map(|i| word(&input, cursor + i * 4))
            .collect();
        cursor += SETTINGS_WORDS * 4;
        if (0x1014 / 4..SETTINGS_WORDS).any(|i| {
            !f32::from_bits(settings[i]).is_finite() || f32::from_bits(settings[i]).abs() > 10000.
        }) || f32::from_bits(settings[0x1028 / 4]) < 0.
        {
            return Err("invalid settings scalars".into());
        }
        let f = |at| f32::from_bits(word(prefix, at));
        let answer = deformation_initialization::calculate(
            &descriptor,
            &bounds,
            Parameters {
                margin: f(12),
                spacing_factor: f(16),
                spacing_growth: f(20),
                fallback_xy: f(24),
                fallback_z: f(28),
                flag_a4b0: word(prefix, 8) as u8,
            },
            &settings,
            [0.75, 0.75],
            0x9a0c90,
            0,
        )?;
        for stage in [answer.reset, answer.layout, answer.initialized, settings] {
            for v in stage {
                results.extend(v.to_le_bytes());
            }
        }
    }
    if cursor != input.len() {
        return Err("trailing initialization bytes".into());
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
        eprintln!("{e}");
        std::process::exit(1);
    }
}
