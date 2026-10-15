//! Private offline tyre-audio render over a scripted source drive: handbrake
//! rev, launch, straight, hard steer, braking. Writes a mono WAV and per-tick
//! voice telemetry. Authored diagnostic scenario, not an original capture.
use grid_tools::{
    source_vehicle::{Controls, DELTA, Runtime},
    tyre_audio::{Bank, CATEGORIES, Driver, Inputs, Mixer},
};
use std::{fs::OpenOptions, io::Write, path::Path};

const RATE: u32 = 48_000;

fn controls(tick: usize) -> Controls {
    match tick {
        0..120 => Controls {
            throttle: 1.,
            handbrake: true,
            ..Default::default()
        },
        120..520 => Controls {
            throttle: 1.,
            ..Default::default()
        },
        520..580 => Controls {
            throttle: 1.,
            steering: 0.6,
            ..Default::default()
        },
        _ => Controls {
            brake: 1.,
            ..Default::default()
        },
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: grid-tyre-audio GAME_ROOT local/OUTPUT.wav TICKS".into());
    }
    let root = Path::new(&args[0]);
    let path = Path::new(&args[1]);
    let ticks: usize = args[2].to_str().ok_or("invalid ticks")?.parse()?;
    if ticks > 3000 {
        return Err("tick bound exceeded".into());
    }
    let parent = path.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let path = parent.join(path.file_name().ok_or("missing filename")?);
    let open = |p: &Path| OpenOptions::new().write(true).create_new(true).open(p);
    let mut wav = open(&path)?;
    let mut telemetry = open(&path.with_extension("tsv"))?;
    let bank = Bank::load(root)?;
    let mut runtime = Runtime::lacetti(root, "slot_00", 200679)?;
    let mut driver = Driver::new(bank.clone());
    let mut mixer = Mixer::new(bank.clone(), RATE);
    let per_tick = (f64::from(RATE) * f64::from(DELTA)).round() as usize;
    let mut pcm = Vec::with_capacity(ticks * per_tick);
    writeln!(telemetry, "tick\twheel\tcategory\tsample\tvolume_db\tpitch")?;
    let names = ["roll", "peel", "slide", "skid", "scrub"];
    let mut peak = 0f32;
    for tick in 0..ticks {
        if let Err(error) = runtime.tick(controls(tick)) {
            println!("tyre_audio physics_stopped tick={tick} error={error}");
            break;
        }
        driver.tick(&Inputs {
            vehicle: &runtime.state.car.owner.vehicle,
            physical: &runtime.state.car.owner.physical,
            surfaces: runtime.surfaces(),
            delta: DELTA,
        });
        let voices = driver.voices();
        if tick % 8 == 0 {
            for (w, wheel) in voices.iter().enumerate() {
                for (c, v) in wheel.iter().enumerate().take(CATEGORIES) {
                    if let Some(s) = v.sample {
                        writeln!(
                            telemetry,
                            "{tick}\t{w}\t{}\t{}\t{:.2}\t{:.3}",
                            names[c],
                            bank.infos.len().min(s),
                            v.volume_db,
                            v.pitch
                        )?;
                    }
                }
            }
        }
        for _ in 0..per_tick {
            let x = mixer.sample(&voices, true);
            if !x.is_finite() {
                return Err(format!("nonfinite tyre sample at tick {tick}").into());
            }
            peak = peak.max(x.abs());
            pcm.push((x.clamp(-1., 1.) * 32767.) as i16);
        }
    }
    let bytes: Vec<u8> = pcm.iter().flat_map(|s| s.to_le_bytes()).collect();
    wav.write_all(b"RIFF")?;
    wav.write_all(&(36 + bytes.len() as u32).to_le_bytes())?;
    wav.write_all(b"WAVEfmt ")?;
    wav.write_all(&16u32.to_le_bytes())?;
    wav.write_all(&1u16.to_le_bytes())?;
    wav.write_all(&1u16.to_le_bytes())?;
    wav.write_all(&RATE.to_le_bytes())?;
    wav.write_all(&(RATE * 2).to_le_bytes())?;
    wav.write_all(&2u16.to_le_bytes())?;
    wav.write_all(&16u16.to_le_bytes())?;
    wav.write_all(b"data")?;
    wav.write_all(&(bytes.len() as u32).to_le_bytes())?;
    wav.write_all(&bytes)?;
    println!(
        "tyre_audio ticks={ticks} samples={} peak={peak:.4} infos={} source_samples={}",
        pcm.len(),
        bank.infos.len(),
        bank.samples.len()
    );
    Ok(())
}

fn main() {
    let result = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))
        .expect("tyre worker")
        .join()
        .expect("tyre worker panic");
    if let Err(error) = result {
        eprintln!("grid-tyre-audio: {error}");
        std::process::exit(1);
    }
}
