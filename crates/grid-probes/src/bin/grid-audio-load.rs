//! Private probe for the low-frame-rate audio report (docs/AUDIO_RESEARCH.md
//! "Low frame rates"): renders the field's audio-thread work, 12 engine
//! mixers and 12 tyre mixers, in output callbacks of `CALLBACK` samples and
//! reports the real-time factor and the slowest callbacks. Controls change at
//! `UPDATE_HZ` (the presentation frame rate).
//!
//! `grid-audio-load ROOT SECONDS UPDATE_HZ`
use grid_tools::{
    car::Car,
    engine_audio::{Control, RATE},
    engine_audio_continuous::{Bank, Mixer},
    tyre_audio::{self, VOICES, Voice},
};
use std::{path::Path, time::Instant};

const CALLBACK: usize = 480;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: grid-audio-load ROOT SECONDS UPDATE_HZ".into());
    }
    let root = Path::new(&args[0]);
    let seconds: f64 = args[1].parse()?;
    let hz: f64 = args[2].parse()?;
    let bank = Bank::load(root, &Car::new("lac")?)?;
    let tyres = tyre_audio::Bank::load(root)?;
    let mut engines: Vec<Mixer> = (0..12).map(|_| Mixer::new(bank.clone())).collect();
    let mut wheels: Vec<tyre_audio::Mixer> = (0..12).map(|_| tyre_audio::Mixer::new(tyres.clone(), RATE)).collect();
    let roll = tyres.material("TARMAC").ok_or("no tarmac")?;
    let voices: [[Voice; VOICES]; 4] = std::array::from_fn(|_| {
        std::array::from_fn(|c| Voice {
            sample: roll[c],
            generation: 0,
            volume_db: -12.,
            pitch: 1.,
        })
    });
    // GRID_AUDIO_ONLY=engines|tyres renders one kind.
    let only = std::env::var("GRID_AUDIO_ONLY").unwrap_or_default();
    if only == "tyres" {
        engines.clear();
    }
    if only == "engines" {
        wheels.clear();
    }
    // Opponents: wheel 0 only, peel/slide/skid (Driver::opponent).
    let mut opponent = voices;
    for (w, wheel) in opponent.iter_mut().enumerate() {
        for (c, v) in wheel.iter_mut().enumerate() {
            if w != 0 || ![tyre_audio::PEEL, tyre_audio::SLIDE, tyre_audio::SKID].contains(&c) {
                v.sample = None;
            }
        }
    }
    let total = (seconds * f64::from(RATE)) as usize;
    let per_update = (f64::from(RATE) / hz) as usize;
    let mut control = Control { normalized_rpm: 0.3, throttle: 0.5, audible: true, generation: 0, gear: 0 };
    let mut worst = Vec::new();
    let start = Instant::now();
    let mut sample = 0;
    let mut sink = 0f32;
    while sample < total {
        let t = Instant::now();
        for _ in 0..CALLBACK {
            if sample % per_update == 0 {
                // A sweep 0.3..0.9 every 4 s, quantised to the update rate.
                let phase = (sample as f64 / f64::from(RATE) / 4.).fract() as f32;
                control.normalized_rpm = 0.3 + 0.6 * phase;
            }
            for (k, e) in engines.iter_mut().enumerate() {
                let mut c = control;
                c.normalized_rpm = (c.normalized_rpm + k as f32 * 0.01).min(1.);
                sink += e.sample(c);
            }
            for (k, w) in wheels.iter_mut().enumerate() {
                sink += w.sample(if k == 0 { &voices } else { &opponent }, true);
            }
            sample += 1;
        }
        worst.push(t.elapsed().as_secs_f64());
    }
    let wall = start.elapsed().as_secs_f64();
    worst.sort_by(|a, b| b.total_cmp(a));
    let period = CALLBACK as f64 / f64::from(RATE);
    let over = worst.iter().filter(|w| **w > period).count();
    println!(
        "audio_load seconds={seconds} update_hz={hz} wall={wall:.3} realtime_factor={:.3} callback_ms={:.2} worst_ms=[{:.3},{:.3},{:.3}] median_ms={:.3} over_period={over} sink={sink:.1}",
        wall / seconds,
        period * 1000.,
        worst[0] * 1000.,
        worst[1] * 1000.,
        worst[2] * 1000.,
        worst[worst.len() / 2] * 1000.,
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("grid-audio-load: {error}");
        std::process::exit(1);
    }
}
