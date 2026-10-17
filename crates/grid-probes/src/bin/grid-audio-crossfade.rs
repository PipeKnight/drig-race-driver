//! Private crossfade diagnostic, not audio output or a native backend substitute.
use std::{fs, io::Write, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err("usage: grid-audio-crossfade local/NEW.bin".into());
    }
    let out = Path::new(&args[0]);
    if out.exists()
        || !out
            .parent()
            .ok_or("output parent")?
            .canonicalize()?
            .starts_with(Path::new("local").canonicalize()?)
    {
        return Err("new private output required".into());
    }
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"GC01");
    bytes.extend_from_slice(&4097_u32.to_le_bytes());
    for i in 0..4097 {
        let phase = 0.001_f32 + (0.998_f32 * i as f32) / 4096.;
        let gains = grid_tools::engine_audio_gain::crossfade_db(phase)?;
        for value in [phase, gains[0], gains[1]] {
            bytes.extend_from_slice(&value.to_bits().to_le_bytes());
        }
    }
    fs::File::create_new(out)?.write_all(&bytes)?;
    println!("audio_crossfade_cases=4097 no_backend=1");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("audio_crossfade_error={error}");
        std::process::exit(1);
    }
}
