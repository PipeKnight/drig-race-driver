//! Private offline render of the player's damage and suspension sounds
//! (`damage_audio`, docs/AUDIO_RESEARCH.md §27) for listening checks. An
//! authored 16 s script, one vehicle tick (16 ms) at a time, through the
//! collision mixer, heard from the Lacetti's mounted bumper view (Normal mode,
//! Bonnet audio camera):
//! 0–4 s engine damage loop (temperature level 0.6, revs 0.3 → 1.0);
//! 4–8 s braking from 30 m/s with every brake damaged (0.6);
//! 8–12 s steering swept ±0.6 with rear wheel 3 damaged (0.5);
//! 12–16 s suspension travel bumps (alternately 8 and 20 cm) at 20 m/s with the
//! suspension damaged (0.8).
//! `off.wav` plays the same script with every damage level 0 (only the
//! suspension bumps sound), `on.wav` with the damage. Mono: the left channel
//! with centred pan.
use grid_tools::{
    audio_mix::{self, Camera, Table},
    car::Car,
    collision_audio::{Bank, DISPLACEMENT, LOOP_SLOTS, Listener, Mixer},
    damage_audio::{Damage, Input, Place},
    engine_audio::RATE,
    engine_audio_continuous::{BLOCK, Bank as EngineBank},
};
use std::{fs, io::Write, path::Path};

fn wave(path: &Path, pcm: &[i16]) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = (pcm.len() * 2) as u32;
    let mut out = fs::OpenOptions::new().write(true).create_new(true).open(path)?;
    out.write_all(b"RIFF")?;
    out.write_all(&(bytes + 36).to_le_bytes())?;
    out.write_all(b"WAVEfmt ")?;
    out.write_all(&16u32.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?;
    out.write_all(&1u16.to_le_bytes())?;
    out.write_all(&RATE.to_le_bytes())?;
    out.write_all(&(RATE * 2).to_le_bytes())?;
    out.write_all(&2u16.to_le_bytes())?;
    out.write_all(&16u16.to_le_bytes())?;
    out.write_all(b"data")?;
    out.write_all(&bytes.to_le_bytes())?;
    for word in pcm {
        out.write_all(&word.to_le_bytes())?;
    }
    Ok(())
}

/// Ticks of the script (16 s).
const TICKS: usize = 1000;
/// Wheel anchors in the car frame [authored: Lacetti-like track and base].
const WHEELS: [[f32; 3]; 4] = [[-0.75, 0.3, 1.3], [0.75, 0.3, 1.3], [-0.75, 0.3, -1.3], [0.75, 0.3, -1.3]];

/// The authored input of tick `k`, with or without damage.
fn script(k: usize, damaged: bool) -> Input {
    let t = k as f32 * 0.016;
    let d = |v: f32| if damaged { v } else { 0. };
    let mut input = Input { grounded: [true; 4], ..Input::default() };
    match k {
        0..250 => {
            input.engine = d(0.6);
            input.revs = 0.3 + 0.7 * (t / 4.);
            input.speed = 10.;
        }
        250..500 => {
            input.brake = 0.8;
            input.brake_damage = [d(0.6); 4];
            input.speed = (30. - 25. * (t - 4.) / 4.).max(0.);
        }
        500..750 => {
            input.steering = 0.6 * (std::f32::consts::TAU * 0.5 * (t - 8.)).sin();
            input.wheel_damage[3] = d(0.5);
            input.speed = 15.;
        }
        _ => {
            input.speed = 20.;
            input.suspension_damage = [d(0.8); 4];
            // A bump every 0.5 s, alternately small and hard: up over two
            // ticks (4 or 10 cm a tick, above SuspensionUp's soft minVel 2 a
            // frame), back over four.
            let phase = (k - 750) % 31;
            let step = if ((k - 750) / 31).is_multiple_of(2) { 0.04 } else { 0.1 };
            let h = match phase {
                0 => step,
                1 => 2. * step,
                2..6 => 2. * step * (1. - 0.25 * (phase - 1) as f32),
                _ => 0.,
            };
            input.travel = [h, h, h * 0.5, h * 0.5];
        }
    }
    input
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-damage-audio GAME_ROOT local/NEW_DIRECTORY".into());
    }
    let root = Path::new(&args[0]);
    let out = Path::new(&args[1]);
    let local = Path::new("local").canonicalize()?;
    if out.exists() || !out.parent().ok_or("output parent")?.canonicalize()?.starts_with(&local) {
        return Err("new private output required".into());
    }
    let car = Car::default();
    let engine = EngineBank::load(root, &car)?;
    let bank = Bank::load(root)?;
    let table = Table::load(root)?;
    let views = fs::read(root.join(format!("cars/{}/cameras.xml", car.stem())))?;
    let views = grid_formats::vehicle_views::read(&views)?;
    let mounted: Vec<_> = views
        .views
        .iter()
        .filter(|v| v.kind == grid_formats::vehicle_views::Kind::Mounted)
        .filter_map(|v| Some((v.ident.as_str(), v.vector("offset")?)))
        .collect();
    let (view, ear) = *mounted
        .iter()
        .find(|(name, _)| name.to_lowercase().contains("bumper"))
        .or(mounted.first())
        .ok_or("no mounted view")?;
    let mode = audio_mix::ModeMix::default();
    let group_db = |g: usize| mode.group_db(&table, Camera::Bonnet, g, 0.);
    let distance = |a: [f32; 3], b: [f32; 3]| ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
    println!(
        "damage_render car={} view={view} ear={ear:?} bay={:?} cockpit={:?} engine_db={:.1} other_mech_db={:.1} displacement_db={:.1}",
        car.stem(),
        engine.emitters[0],
        engine.cockpit,
        group_db(audio_mix::ENGINE),
        group_db(audio_mix::OTHER_MECH),
        group_db(DISPLACEMENT)
    );
    fs::create_dir(out)?;
    for (name, damaged) in [("off", false), ("on", true)] {
        let mut mixer = Mixer::new(bank.clone(), RATE);
        let mut listener = Listener::new(bank.clone());
        let mut damage = Damage::default();
        let (mut loops, mut shots) = (0, 0);
        let mut pcm = Vec::with_capacity(TICKS * BLOCK);
        let mut peak = 0f32;
        for k in 0..TICKS {
            listener.tick(0.016);
            let frame = damage.frame(&bank, &mut listener, &script(k, damaged), 0.016);
            let place = |p: Place| match p {
                Place::EngineBay => engine.emitters[0],
                Place::Cockpit => engine.cockpit,
                Place::Wheel(i) => WHEELS[i.min(3)],
            };
            // Engine and Other Mech from the camera, Displacement from the car (grid-app).
            let from = |g: usize| if g == DISPLACEMENT { [0.; 3] } else { ear };
            let gain = |variety: &str, volume_db: f32, group: usize, at: [f32; 3]| {
                bank.varieties.get(variety).map_or(0., |v| {
                    audio_mix::backend_gain(
                        volume_db,
                        group_db(group),
                        audio_mix::distance_db(distance(at, from(group)), v.min_distance, v.max_distance),
                    )
                })
            };
            for slot in frame.stops {
                mixer.stop(LOOP_SLOTS + slot);
            }
            for l in frame.loops {
                let g = gain(&l.variety, l.volume_db, l.group, place(l.place));
                mixer.looped(LOOP_SLOTS + l.slot, &l.variety, l.pitch, g, [1.; 2]);
                loops += 1;
            }
            for s in frame.shots {
                let g = gain(&s.variety, s.volume_db, s.group, place(s.place));
                shots += usize::from(mixer.start(&s.variety, s.pitch, g, [1.; 2]));
            }
            for _ in 0..BLOCK {
                let v = mixer.sample()[0];
                peak = peak.max(v.abs());
                pcm.push((v.clamp(-1., 1.) * 32767.).round() as i16);
            }
        }
        wave(&out.join(format!("{name}.wav")), &pcm)?;
        println!(
            "damage_render variant={name} loop_ticks={loops} shots={shots} peak={peak:.4} seconds={:.1}",
            pcm.len() as f32 / RATE as f32
        );
    }
    Ok(())
}

fn main() {
    let result = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))
        .unwrap()
        .join()
        .unwrap();
    if let Err(e) = result {
        eprintln!("grid-damage-audio: {e}");
        std::process::exit(1);
    }
}
