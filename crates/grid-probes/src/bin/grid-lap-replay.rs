//! Private sampled-pose replay against purchased gates; no vehicle resimulation.
use grid_world::lap_timing::{Event, State};
use std::{fmt::Write as _, io::Write as _, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !(args.len() == 3 || (args.len() == 4 && args[3] == "--all-progress-gates")) {
        return Err("usage: grid-lap-replay GAME_ROOT local/INPUT.bin local/OUTPUT.tsv".into());
    }
    let input = Path::new(&args[1]);
    let output = Path::new(&args[2]);
    let private = Path::new("local").canonicalize()?;
    if !input.canonicalize()?.starts_with(&private)
        || input.metadata()?.len() > 16 * 1024 * 1024
        || !output
            .parent()
            .ok_or("missing output parent")?
            .canonicalize()?
            .starts_with(&private)
    {
        return Err("bounded private lap replay paths required".into());
    }
    let data = std::fs::read(input)?;
    if data.len() < 28 || &data[..4] != b"LP01" {
        return Err("invalid lap replay framing".into());
    }
    let count = u32::from_le_bytes(data[4..8].try_into().unwrap()) as usize;
    if !(1..=360000).contains(&count) || data.len() != 8 + count * 20 {
        return Err("invalid lap replay extent".into());
    }
    let prepared = grid_tools::lap_progress::load(Path::new(&args[0]))?;
    let mut course = prepared.course;
    if args.len() == 4 {
        let required: Vec<_> = (0..course.gate_count()).collect();
        course = course.with_required(&required)?;
    }
    let position = |r: &[u8]| {
        std::array::from_fn(|i| f32::from_le_bytes(r[8 + i * 4..12 + i * 4].try_into().unwrap()))
    };
    let mut state = State::new(&course, position(&data[8..28]))?;
    let mut text = String::new();
    writeln!(
        text,
        "P\t{}\t{}",
        course.gate_count(),
        course
            .required_gates()
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )?;
    for (i, r) in data[8..].as_chunks::<20>().0.iter().enumerate() {
        let tick = u64::from_le_bytes(r[..8].try_into().unwrap());
        if tick != i as u64 + 1 {
            return Err("lap replay requires consecutive interpolated ticks from one".into());
        }
        let result = state.advance(
            &course,
            position(r),
            tick as f64 * f64::from(grid_tools::source_vehicle::DELTA),
        )?;
        state = result.state;
        for event in result.events {
            match event {
                Event::Started { when } => {
                    writeln!(text, "T\t{tick}\tSTART\t{:016x}", when.to_bits())?
                }
                Event::Checkpoint { gate, when } => {
                    writeln!(text, "T\t{tick}\tGATE\t{gate}\t{:016x}", when.to_bits())?
                }
                Event::Sector { sector, seconds } => writeln!(
                    text,
                    "T\t{tick}\tSECTOR\t{sector}\t{:016x}",
                    seconds.to_bits()
                )?,
                Event::Finished(lap) => writeln!(
                    text,
                    "T\t{tick}\tLAP\t{}\t{}\t{:016x}\t{}\t{}",
                    lap.number,
                    u8::from(lap.valid),
                    lap.seconds.to_bits(),
                    lap.missed_gate
                        .map_or_else(|| "none".into(), |v| v.to_string()),
                    lap.sectors
                        .iter()
                        .map(|v| v
                            .map_or_else(|| "none".into(), |s| format!("{:016x}", s.to_bits())))
                        .collect::<Vec<_>>()
                        .join(",")
                )?,
                Event::JumpInvalidated => writeln!(text, "T\t{tick}\tJUMP")?,
            }
        }
        if text.len() > 16 * 1024 * 1024 {
            return Err("lap replay output exceeds bound".into());
        }
    }
    writeln!(
        text,
        "S\t{}\t{}\t{}\t{}\t{}\t{}",
        state.attempts,
        state.valid_laps,
        state.next_gate,
        state.passed_gates(),
        state
            .best_lap
            .map_or_else(|| "none".into(), |s| format!("{:016x}", s.to_bits())),
        state
            .elapsed()
            .map_or_else(|| "none".into(), |s| format!("{:016x}", s.to_bits()))
    )?;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(text.as_bytes())?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-lap-replay: {error}");
        std::process::exit(1);
    }
}
