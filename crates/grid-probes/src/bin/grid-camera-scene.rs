//! Deterministic camera scene: drive the source Lacetti with scripted controls
//! and write the car state per tick in the decoded live-capture layout of
//! local/cam/live/decode.py, so the port's views (grid-app
//! `camera::tests::replay_live_trace`) and the emulated original's camera
//! updates read the same rows. Columns: t, step, speed (+0x1648), state
//! (+0xD234), wheel displacement/travel x4, physics pose (16, column-major),
//! angular velocity, velocity, 39 zeros where a capture has the view record,
//! car +0x10..+0x4F (16), car +0x1630 (3). Authored diagnostic; private
//! output only.
use grid_tools::source_vehicle::{Controls, DELTA, Runtime};
use std::{fs::OpenOptions, io::Write, path::Path};

/// SCRIPT: `TICKS:THROTTLE,BRAKE,STEER,HANDBRAKE;...`, steering positive
/// toward the car's left.
fn parse_script(text: &str) -> Result<Vec<(u32, Controls)>, Box<dyn std::error::Error>> {
    text.split(';')
        .map(|phase| {
            let (ticks, c) = phase.split_once(':').ok_or("phase is TICKS:T,B,S,H")?;
            let c: Vec<f32> = c.split(',').map(str::parse).collect::<Result<_, _>>()?;
            if c.len() != 4 {
                return Err("controls are THROTTLE,BRAKE,STEER,HANDBRAKE".into());
            }
            Ok((
                ticks.parse()?,
                Controls { throttle: c[0], brake: c[1], steering: c[2], handbrake: c[3] != 0. },
            ))
        })
        .collect()
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if !(3..=4).contains(&args.len()) {
        return Err("usage: grid-camera-scene GAME_ROOT local/OUTPUT.tsv SCRIPT [CD01]".into());
    }
    let script = parse_script(args[2].to_str().ok_or("invalid script text")?)?;
    if script.iter().map(|(n, _)| u64::from(*n)).sum::<u64>() > 36_000 {
        return Err("script longer than 36000 ticks".into());
    }
    let path = Path::new(&args[1]);
    let parent = path.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let path = parent.join(path.file_name().ok_or("missing filename")?);
    let mut out = OpenOptions::new().write(true).create_new(true).open(&path)?;
    let mut runtime = Runtime::lacetti(Path::new(&args[0]), "slot_00", 200679)?;
    // With the contact directions (CD01) the body collides with the track's
    // walls and props, as `grid-app --body-contacts`.
    let geometry = match args.get(3) {
        Some(table) => Some(runtime.prepare_body_geometry(Path::new(&args[0]), &std::fs::read(table)?)?),
        None => None,
    };
    let row = |runtime: &Runtime, out: &mut std::fs::File| -> std::io::Result<f32> {
        let b = &runtime.state.car.owner.vehicle;
        let f = |o: usize| f32::from_bits(b[o / 4]);
        let mut v: Vec<String> = vec![
            format!("{:?}", runtime.ticks as f64 * f64::from(DELTA)),
            format!("{:?}", DELTA),
            format!("{:?}", f(0x1648)),
            b[0xd234 / 4].to_string(),
        ];
        for i in 0..4 {
            let w = 0x1990 + i * 0x150;
            v.extend([f(w + 0x40), f(w + 0x4c)].map(|x| format!("{x:?}")));
        }
        v.extend(runtime.pose().map(|x| format!("{x:?}")));
        let w = runtime.angular_velocity();
        v.extend(w[..3].iter().map(|x| format!("{x:?}")));
        v.extend(runtime.velocity()[..3].iter().map(|x| format!("{x:?}")));
        v.extend(std::iter::repeat_n("0.0".to_string(), 39));
        v.extend((0..16).map(|i| format!("{:?}", f(0x10 + 4 * i))));
        v.extend((0..3).map(|i| format!("{:?}", f(0x1630 + 4 * i))));
        writeln!(out, "{}", v.join("\t"))?;
        Ok((w[0] * w[0] + w[1] * w[1] + w[2] * w[2]).sqrt())
    };
    let mut peak = row(&runtime, &mut out)?;
    for (ticks, controls) in script {
        for _ in 0..ticks {
            match &geometry {
                Some(g) => runtime.tick_with_body_geometry(controls, g),
                None => runtime.tick(controls),
            }
            .map_err(|e| format!("tick {}: {e}", runtime.ticks + 1))?;
            peak = peak.max(row(&runtime, &mut out)?);
        }
    }
    out.flush()?;
    println!("camera_scene rows={} peak_angular={peak}", runtime.ticks + 1);
    Ok(())
}

fn main() {
    let result = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))
        .expect("scene worker")
        .join()
        .expect("scene worker panic");
    if let Err(error) = result {
        eprintln!("grid-camera-scene: {error}");
        std::process::exit(1);
    }
}
