//! Private complete owned gain-publication diagnostic.
use grid_tools::{
    engine_audio_control::CONTROLLER_BYTES,
    engine_audio_publication::{Environment, VOICE_BYTES},
    engine_audio_schedule,
};
use std::{fs, io::Write, path::Path};
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn scalar(b: &[u8], at: usize) -> f32 {
    f32::from_bits(word(b, at))
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-engine-audio-playback local/REQUEST local/NEW_OUTPUT".into());
    }
    let input = Path::new(&args[0]);
    let out = Path::new(&args[1]);
    let local = Path::new("local").canonicalize()?;
    if !input.canonicalize()?.starts_with(&local)
        || out.exists()
        || !out
            .parent()
            .ok_or("output parent")?
            .canonicalize()?
            .starts_with(&local)
        || input.metadata()?.len() > 16 * 1024 * 1024
    {
        return Err("bounded private input and new output required".into());
    }
    let bytes = fs::read(input)?;
    if bytes.len() < 8 || &bytes[..4] != b"EP01" {
        return Err("engine gain request header".into());
    }
    let count = word(&bytes, 4) as usize;
    const RECORD: usize = 2448;
    if !(1..=4096).contains(&count) || bytes.len() != 8 + 16384 + count * RECORD {
        return Err("engine gain request extent".into());
    }
    let mut output = bytes[..8].to_vec();
    let curves: [f32; 4096] = std::array::from_fn(|i| scalar(&bytes, 8 + i * 4));
    for record in bytes[16392..].as_chunks::<RECORD>().0 {
        let mut controller: [u8; CONTROLLER_BYTES] = record[4..4 + CONTROLLER_BYTES].try_into()?;
        let mut voices: [[u8; VOICE_BYTES]; 12] = std::array::from_fn(|slot| {
            record[4 + CONTROLLER_BYTES + slot * VOICE_BYTES
                ..4 + CONTROLLER_BYTES + (slot + 1) * VOICE_BYTES]
                .try_into()
                .unwrap()
        });
        let base = 4 + CONTROLLER_BYTES + 12 * VOICE_BYTES;
        let player = word(record, base + 60);
        if player > 1 {
            return Err("engine gain player flag".into());
        }
        let env = Environment {
            motion_210: std::array::from_fn(|i| scalar(record, base + i * 4)),
            motion_220: std::array::from_fn(|i| scalar(record, base + 16 + i * 4)),
            ranges: std::array::from_fn(|i| {
                std::array::from_fn(|j| scalar(record, base + 32 + i * 8 + j * 4))
            }),
            trims: [scalar(record, base + 48), scalar(record, base + 52)],
            treble: scalar(record, base + 56),
            player: player != 0,
        };
        let services = [grid_tools::engine_audio_restart::PlaybackService {
            accepted_microseconds: [u32::MAX; 2],
            cooldown: scalar(record, 2444),
            resource: 1,
            groups: [
                word(&controller, 0x11c) ^ 0x100,
                word(&controller, 0x124) ^ 0x100,
            ],
        }; 3];
        let commands = engine_audio_schedule::publish(
            &mut controller,
            &mut voices,
            env,
            &curves,
            f64::from_le_bytes(record[base + 64..base + 72].try_into()?),
            services,
        )?;
        output.extend_from_slice(&record[..4]);
        output.extend_from_slice(&controller);
        for voice in voices {
            output.extend_from_slice(&voice);
        }
        output.extend_from_slice(&0_u32.to_le_bytes());
        output.extend_from_slice(&(commands.len() as u32).to_le_bytes());
        for i in 0..6 {
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
        "engine_schedule_playback_cases={count} completed={count} bytes={}",
        output.len()
    );
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("engine_schedule_playback_error={error}");
        std::process::exit(1);
    }
}
