//! Bounded private coupled playback-control diagnostic; no PCM/backend output.
use grid_tools::engine_audio_control::{self, CONTROLLER_BYTES, CURVE_WORDS, VehicleEngine};
use std::{fs, io::Write, path::Path};

fn word(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-engine-audio-control local/REQUEST local/NEW_OUTPUT".into());
    }
    let input = Path::new(&args[0]);
    let output = Path::new(&args[1]);
    let local = Path::new("local").canonicalize()?;
    if !input.canonicalize()?.starts_with(&local)
        || output.exists()
        || !output
            .parent()
            .ok_or("output parent")?
            .canonicalize()?
            .starts_with(&local)
        || input.metadata()?.len() > 4 * 1024 * 1024
    {
        return Err("bounded private input and new private output required".into());
    }
    let bytes = fs::read(input)?;
    if bytes.len() < 8 || word(&bytes, 0) != 0x31304345 {
        return Err("engine audio request header".into());
    }
    let count = word(&bytes, 4) as usize;
    const RECORD: usize = 4 + CONTROLLER_BYTES + 12 + 8;
    if !(1..=4096).contains(&count) || bytes.len() != 8 + CURVE_WORDS * 4 + count * RECORD {
        return Err("engine audio request extent".into());
    }
    let mut curves = [0.; CURVE_WORDS];
    for (i, v) in curves.iter_mut().enumerate() {
        *v = f32::from_bits(word(&bytes, 8 + i * 4));
    }
    let mut result = bytes[..8].to_vec();
    for record in bytes[8 + CURVE_WORDS * 4..].as_chunks::<RECORD>().0 {
        let mut controller: [u8; CONTROLLER_BYTES] = record[4..4 + CONTROLLER_BYTES].try_into()?;
        let base = 4 + CONTROLLER_BYTES;
        let engine = VehicleEngine {
            speed: f32::from_bits(word(record, base)),
            maximum: f32::from_bits(word(record, base + 4)),
            idle: f32::from_bits(word(record, base + 8)),
            delta: f64::from_le_bytes(record[base + 12..base + 20].try_into()?),
        };
        let queries = engine_audio_control::update(&mut controller, engine, &curves)?;
        result.extend_from_slice(&record[..4]);
        result.extend_from_slice(&controller);
        for query in queries {
            result.extend_from_slice(&query.recorded_rpm_bits.to_le_bytes());
            result.extend_from_slice(&query.seek_microseconds.to_le_bytes());
        }
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&result)?;
    println!(
        "engine_audio_control_cases={count} completed={count} bytes={}",
        result.len()
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("engine_audio_control_error={error}");
        std::process::exit(1);
    }
}
