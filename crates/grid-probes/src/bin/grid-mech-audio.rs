//! Private offline render of the player's engine with its mechanical sounds
//! (`mech_audio`: clunk, backfire, turbo) for listening checks. Replays the
//! engine-audio trace of a headless solo race (`GRID_ENGINE_AUDIO_TRACE` of
//! grid-opponent-race) through the continuous mixer, one control per 16 ms
//! block as grid-app publishes them, heard from the car's mounted bumper
//! view (Normal mode, Bonnet audio camera). Writes one 48 kHz mono WAV per
//! variant: `base` without mechanical sounds, then one per sound group and
//! `all`. After the lap, 4 s of authored controls: the limiter, then a lift.
use grid_tools::{
    audio_mix::{self, Camera, Table},
    car::Car,
    engine_audio::{Control, RATE},
    engine_audio_continuous::{BLOCK, Bank as EngineBank, Mixer},
    mech_audio::{self, Levels},
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

/// Mechanical groups heard in a variant (`mech_audio` group indices).
const VARIANTS: [(&str, &[usize]); 5] = [
    ("base", &[]),
    ("clunk", &[mech_audio::GEARBOX]),
    ("backfire", &[mech_audio::EXHAUST]),
    ("turbo", &[mech_audio::TURBO, mech_audio::ENGINE]),
    ("all", &[mech_audio::GEARBOX, mech_audio::EXHAUST, mech_audio::TURBO, mech_audio::ENGINE]),
];

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: grid-mech-audio GAME_ROOT local/ENGINE_TRACE.tsv local/NEW_DIRECTORY".into());
    }
    let root = Path::new(&args[0]);
    let out = Path::new(&args[2]);
    let local = Path::new("local").canonicalize()?;
    if out.exists() || !out.parent().ok_or("output parent")?.canonicalize()?.starts_with(&local) {
        return Err("new private output required".into());
    }
    let mut controls = Vec::new();
    for line in fs::read_to_string(&args[1])?.lines().skip(1) {
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 4 {
            return Err(format!("trace line `{line}`").into());
        }
        controls.push(Control {
            normalized_rpm: f[1].parse()?,
            throttle: f[2].parse()?,
            audible: true,
            generation: 1,
            gear: f[3].parse()?,
        });
    }
    let lap = controls.len();
    // A synthetic tail the AI lap never reaches [authored]: 2 s at the
    // maximum RPM in third at full throttle (the limiter, 414970), then a lift
    // with the RPM falling to 0.4 over 2 s.
    for tick in 0..250 {
        let lift = tick >= 125;
        let t = (tick as f32 - 125.).max(0.) / 125.;
        controls.push(Control {
            normalized_rpm: if lift { 1. - 0.6 * t } else { 1. },
            throttle: if lift { 0. } else { 1. },
            audible: true,
            generation: 1,
            gear: 3,
        });
    }
    let car = Car::default();
    let bank = EngineBank::load(root, &car)?;
    let mech = mech_audio::Bank::load(root, &bank.mechanics_bank)?;
    let table = Table::load(root)?;
    // The ear: the first mounted view whose name says bumper, in the car frame.
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
    let distance = |e: [f32; 3]| ((e[0] - ear[0]).powi(2) + (e[1] - ear[1]).powi(2) + (e[2] - ear[2]).powi(2)).sqrt();
    let [bay, exhaust] = bank.emitters.map(distance);
    let gearbox = distance(bank.gearbox);
    let mode = audio_mix::ModeMix::default();
    let group = |g: usize| mode.group_db(&table, Camera::Bonnet, g, 0.);
    let engine_db = [audio_mix::ENGINE, audio_mix::EXHAUST]
        .into_iter()
        .zip([bay, exhaust])
        .map(|(g, d)| audio_mix::distance_db(d, 10., 175.).map_or(f32::NEG_INFINITY, |d| group(g) + d))
        .collect::<Vec<_>>();
    println!(
        "mech_render car={} mec={} view={view} ear={ear:?} distance_m gearbox={gearbox:.2} bay={bay:.2} exhaust={exhaust:.2} engine_db={engine_db:?} ticks={} lap_ticks={lap}",
        car.stem(),
        bank.mechanics_bank,
        controls.len()
    );
    fs::create_dir(out)?;
    for (name, groups) in VARIANTS {
        let mut mixer = Mixer::new(bank.clone());
        mixer.bus_db = [engine_db[0], engine_db[1]];
        let group_db = [mech_audio::GEARBOX, mech_audio::EXHAUST, mech_audio::TURBO, mech_audio::ENGINE].map(|g| {
            let mix = [audio_mix::GEARBOX, audio_mix::EXHAUST, audio_mix::TURBO, audio_mix::ENGINE][g];
            if groups.contains(&g) { group(mix) } else { f32::NEG_INFINITY }
        });
        mixer.mech = Some(mech_audio::Voices::new(mech.clone(), RATE));
        mixer.mech_levels = Levels { group_db, distance: [gearbox, exhaust, bay] };
        let mut pcm = Vec::with_capacity(controls.len() * BLOCK);
        let mut peak = 0f32;
        for &control in &controls {
            for _ in 0..BLOCK {
                let v = mixer.sample(control);
                peak = peak.max(v.abs());
                pcm.push((v * 32767.).round() as i16);
            }
        }
        if let Some(e) = mixer.failure {
            return Err(e.into());
        }
        let shifts = controls.windows(2).filter(|w| w[1].gear > w[0].gear).count();
        wave(&out.join(format!("{name}.wav")), &pcm)?;
        println!(
            "mech_render variant={name} group_db={group_db:?} peak={peak:.4} clipped={} upshifts={shifts} seconds={:.1}",
            mixer.diagnostics.clipped_samples,
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
        eprintln!("grid-mech-audio: {e}");
        std::process::exit(1);
    }
}
