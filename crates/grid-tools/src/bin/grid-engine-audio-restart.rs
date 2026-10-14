//! Private supplied-backend restart comparison; no device or live game access.
use grid_tools::engine_audio_restart::{PlaybackService, restart};
use std::{fs, io::Write, path::Path};
fn u(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-engine-audio-restart local/REQUEST local/NEW_OUTPUT".into());
    }
    let input = Path::new(&args[0]);
    let out = Path::new(&args[1]);
    let local = Path::new("local").canonicalize()?;
    if !input.canonicalize()?.starts_with(&local)
        || input.metadata()?.len() > 16 * 1024 * 1024
        || out.exists()
        || !out
            .parent()
            .ok_or("output parent")?
            .canonicalize()?
            .starts_with(&local)
    {
        return Err("bounded private input/new output required".into());
    }
    let b = fs::read(input)?;
    if b.len() < 16392 || &b[..4] != b"ER01" {
        return Err("restart header".into());
    }
    let count = u(&b, 4) as usize;
    if !(1..=4096).contains(&count) || b.len() != 16392 + count * 856 {
        return Err("restart extent".into());
    }
    let curves = std::array::from_fn(|i| f32::from_bits(u(&b, 8 + i * 4)));
    let mut output = b[..8].to_vec();
    for row in b[16392..].as_chunks::<856>().0 {
        let mut controller: [u8; 832] = row[4..836].try_into()?;
        let service = PlaybackService {
            accepted_microseconds: [u(row, 840), u(row, 844)],
            cooldown: f32::from_bits(u(row, 848)),
            resource: u(row, 852),
            groups: [u(&controller, 0x11c) ^ 0x100, u(&controller, 0x124) ^ 0x100],
        };
        let commands = restart(&mut controller, &curves, u(row, 836) as usize, service)?;
        output.extend_from_slice(&row[..4]);
        output.extend_from_slice(&controller);
        output.extend_from_slice(&(commands.len() as u32).to_le_bytes());
        for i in 0..2 {
            let c = commands.get(i).copied().unwrap_or_default();
            for v in [
                c.slot,
                c.resource,
                c.sample,
                c.flags,
                c.group,
                c.requested_microseconds,
                c.accepted_microseconds,
            ] {
                output.extend_from_slice(&v.to_le_bytes());
            }
        }
    }
    fs::File::create_new(out)?.write_all(&output)?;
    println!(
        "engine_restart_cases={count} completed={count} bytes={}",
        output.len()
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("engine_restart_error={e}");
        std::process::exit(1);
    }
}
