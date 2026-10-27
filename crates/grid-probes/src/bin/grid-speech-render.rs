//! Private offline render of the race engineer speech, parts 1 and 2 (`grid_tools::speech`,
//! docs/SPEECH_AUDIO_RESEARCH.md) for listening checks. The purchased rules and lines run
//! through the port's collector and manager on an authored 12-car race, one vehicle tick (16 ms)
//! at a time; nothing is played on a device.
//! - 0–6 s on the grid in the last slot; green at 6 s.
//! - 12 s: Restart Race from the race in progress (the only way to `playerGridPosLast`), 8 s on
//!   the grid again; green at 20 s.
//! - The player gains a place every 14 s from 12th to 1st, loses 2nd once (to 3rd) and regains
//!   it, leads, and finishes first after three laps of 60 s.
//! - 50 s after the green light a car-to-car collision: 30 % component damage, the front bumper
//!   lost. 100–112 s the wrong way at full speed.
//! - 70 s: entries 3 and 4 collide 2 % of a lap ahead of the player; entry 3 takes 6 % damage,
//!   entry 4 a terminal wheel (status 3).
//!
//! `speech.wav`: mono PCM16 at the lines' 32 kHz, the Codriver group level of the Chase camera
//! with the Talking mix while a line plays. `lines.txt`: each line with its time.
use grid_tools::{
    audio_mix::{self, Camera, ModeMix, Table},
    source_vehicle::DELTA,
    speech::{Car, Collector, Contact, Entry, Kind, Manager, RaceView, Rules, part_id},
};
use std::{fs, io::Write, path::Path, sync::Arc};

const CARS: usize = 12;
const LAP: f64 = 60.;
const LAPS: f64 = 3.;
const CODRIVER: usize = 14;
/// As grid-app's speech_sound [AUTHORED].
const SEED: u32 = 5489;

fn wave(path: &Path, pcm: &[i16], rate: u32) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = (pcm.len() * 2) as u32;
    let mut out = fs::OpenOptions::new().write(true).create_new(true).open(path)?;
    out.write_all(b"RIFF")?;
    out.write_all(&(bytes + 36).to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&16u32.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?;
    out.write_all(&rate.to_le_bytes())?;
    out.write_all(&(rate * 2).to_le_bytes())?;
    out.write_all(&2u16.to_le_bytes())?;
    out.write_all(&16u16.to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&bytes.to_le_bytes())?;
    for word in pcm {
        out.write_all(&word.to_le_bytes())?;
    }
    Ok(())
}

/// The player's place `t` seconds after the green light.
fn place(t: f64) -> usize {
    let gained = ((t - 10.) / 14.).floor().clamp(0., 11.) as usize;
    let place = CARS - gained;
    // Passed back once while 2nd, then 2nd again.
    if place == 2 && (t - 10.) % 14. > 7. && t < 10. + 14. * 10. + 14. { 3 } else { place }
}

/// The race at `t` seconds after the green light (`None`: on the grid).
fn view(green: Option<f64>) -> RaceView {
    let t = green.unwrap_or(0.);
    let mine = if green.is_some() { place(t) } else { CARS };
    let progress = t / LAP;
    let finished = green.is_some() && progress >= LAPS;
    let mut others = (1..=CARS).filter(|&p| p != mine);
    let entries = (0..CARS)
        .map(|k| {
            let position = if k == 0 { mine } else { others.next().unwrap() };
            // Places 1.5 s apart at the player's pace.
            let p = (progress - (position as f64 - mine as f64) * 0.025).max(0.);
            Entry {
                time: t,
                progress: if green.is_some() { p.min(LAPS) } else { 0. },
                laps: if green.is_some() { p.min(LAPS) as u32 } else { 0 },
                speed: if green.is_some() { 45. } else { 0. },
                position,
                finished: green.is_some() && p >= LAPS,
                grid: if k == 0 { CARS - 1 } else { k - 1 },
                last_lap: (p >= 1.).then_some(LAP),
                ai: k != 0,
                terminal: k == 4 && green.is_some() && t >= 70.,
                damage: if (k == 3 || k == 4) && green.is_some() && t >= 70. { 6. } else { 0. },
            }
        })
        .collect();
    // The collision frame (one vehicle tick).
    let hit = green.is_some() && (t - 50.).abs() < f64::from(DELTA) / 2.;
    let ahead = green.is_some() && (t - 70.).abs() < f64::from(DELTA) / 2.;
    let damaged = green.is_some() && t >= 50.;
    RaceView {
        clock: t,
        started: green.is_some(),
        over: finished,
        track_laps: LAPS as u32,
        entries,
        player: 0,
        kind: Kind::default(),
        contacts: match (hit, ahead) {
            (true, _) => vec![Contact { car: 0, other: Some(1) }],
            (_, true) => vec![Contact { car: 3, other: Some(4) }, Contact { car: 4, other: Some(3) }],
            _ => Vec::new(),
        },
        car: Car {
            components: if damaged { [0.3; 6] } else { [0.; 6] },
            engine: 0.8,
            limit: 1.,
            density: 1.,
            ..Car::default()
        },
        off_track: false,
        wrong_way: green.is_some() && (100. ..112.).contains(&t),
        parts_lost: if hit { vec![part_id("bumper_fm").unwrap()] } else { Vec::new() },
        open_cockpit: false,
        status_changes: if ahead { vec![(4, 3)] } else { Vec::new() },
        // The choreographer, between other lines and with the car ahead of the player on the
        // same lap (the rules compare lap fractions without wrapping): entry 2 power_spin from
        // 25 s to 29 s, entry 1 blow_tyre from 160 s to 162 s.
        ai_events: [(25., 4, 2, true), (29., 4, 2, false), (160., 1, 1, true), (162., 1, 1, false)]
            .into_iter()
            .filter(|&(at, ..)| green.is_some() && (t - at).abs() < f64::from(DELTA) / 2.)
            .map(|(_, kind, car, on)| grid_tools::ai_pacing::Message { kind, car, on })
            .collect(),
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-speech-render GAME_ROOT local/NEW_DIRECTORY".into());
    }
    let root = Path::new(&args[0]);
    let out = Path::new(&args[1]);
    let local = Path::new("local").canonicalize()?;
    if out.exists() || !out.parent().ok_or("output parent")?.canonicalize()?.starts_with(&local) {
        return Err("new private output required".into());
    }
    let rules = Arc::new(Rules::load(root)?);
    let table = Table::load(root)?;
    let mut manager = Manager::new(rules, SEED);
    let mut collector = Collector::default();
    let mut mix = ModeMix::default();
    mix.set(audio_mix::START_LINE);
    let rate = grid_tools::speech::LINE_RATE;
    let per_tick = f64::from(DELTA) * f64::from(rate);
    let mut pcm: Vec<f32> = Vec::new();
    let mut voice: Option<(Vec<i16>, usize)> = None;
    let mut log = String::new();
    let exists = |line: &grid_tools::speech::Line| line.path(root, "en").is_file();
    let finish = 20. + LAP * LAPS;
    let ticks = ((finish + 25.) / f64::from(DELTA)) as u64;
    for tick in 0..ticks {
        let time = tick as f64 * f64::from(DELTA);
        if (time - 12.).abs() < f64::from(DELTA) / 2. {
            // Restart Race.
            collector.restart();
            manager.reset_race();
            voice = None;
            mix.set(audio_mix::START_LINE);
            log.push_str(&format!("{time:7.2} restart\n"));
        }
        let green = match time {
            t if t < 6. => None,
            t if t < 12. => Some(t - 6.),
            t if t < 20. => None,
            t => Some(t - 20.),
        };
        if green.is_some() && mix.mode() == audio_mix::START_LINE {
            mix.set(audio_mix::NORMAL);
        }
        let busy = voice.is_some();
        if !busy {
            mix.release();
        }
        let params = collector.update(&view(green), DELTA).clone();
        let frame = manager.frame(DELTA, &params, busy, false, &exists);
        if frame.stop {
            voice = None;
        }
        if let Some(line) = frame.play {
            let bytes = fs::read(line.path(root, "en"))?;
            let samples: Vec<i16> = bytes.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)).collect();
            log.push_str(&format!(
                "{time:7.2} play {} {}/{} {:.2}s pos={} prev={} best={}\n",
                line.message,
                line.folder,
                line.file,
                samples.len() as f64 / f64::from(rate),
                params.get(grid_tools::speech::Param::PlayerPos),
                params.get(grid_tools::speech::Param::PlayerPrevPosition),
                params.get(grid_tools::speech::Param::PlBestPosInRace),
            ));
            voice = Some((samples, 0));
            if !line.pa {
                mix.set_tied(audio_mix::TALKING);
            }
        }
        mix.advance(&table, DELTA);
        let gain = audio_mix::backend_gain(0., mix.group_db(&table, Camera::Chase, CODRIVER, 0.), Some(0.));
        let end = ((tick + 1) as f64 * per_tick) as usize;
        while pcm.len() < end {
            let v = match &mut voice {
                Some((samples, at)) if *at < samples.len() => {
                    *at += 1;
                    f32::from(samples[*at - 1]) / 32768. * gain
                }
                _ => {
                    voice = None;
                    0.
                }
            };
            pcm.push(v);
        }
    }
    fs::create_dir(out)?;
    let peak = pcm.iter().fold(0f32, |m, v| m.max(v.abs()));
    let pcm: Vec<i16> = pcm.iter().map(|v| (v.clamp(-1., 1.) * 32767.).round() as i16).collect();
    wave(&out.join("speech.wav"), &pcm, rate)?;
    fs::write(out.join("lines.txt"), &log)?;
    print!("{log}");
    println!(
        "speech_render lines={} peak={peak:.4} seconds={:.1}",
        log.matches(" play ").count(),
        pcm.len() as f64 / f64::from(rate)
    );
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("grid-speech-render: {e}");
        std::process::exit(1);
    }
}
