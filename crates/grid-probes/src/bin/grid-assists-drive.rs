//! Private headless driver-aid matrix: every combination of traction control,
//! stability control, braking assist, gearbox and input device drives four
//! scripted manoeuvres from Donington slot_00 (straight launch, hard braking,
//! steady turn, gear shifts). Writes one per-tick TSV per run and a summary.
use grid_tools::source_vehicle::{
    Controls, DELTA, Runtime,
    assists::{Assists, Input, Shift, Transmission},
};
use std::{io::Write, path::Path};

const SCENARIOS: [&str; 4] = ["launch", "brake", "turn", "shifts"];

fn f(b: &[u32], o: usize) -> f32 {
    f32::from_bits(b[o / 4])
}

struct Sample {
    speed: f32,
    gear: u32,
    rpm: f32,
    yaw: f32,
    front_slip: f32,
    locked: usize,
    brake_timer: f32,
}

fn sample(r: &Runtime) -> Sample {
    let b = &r.state.car.owner.vehicle;
    let v = r.velocity();
    let speed = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    let surface = |i: usize| f(b, 0x1990 + i * 0x150 + 0x104) * f(b, 0x1990 + i * 0x150 + 0xe0);
    let front = (surface(2) + surface(3)) * 0.5;
    Sample {
        speed,
        gear: r.gear(),
        rpm: f(b, 0x1894),
        yaw: r.angular_velocity()[1],
        front_slip: if speed > 1. { front / speed - 1. } else { 0. },
        locked: (0..4)
            .filter(|&i| speed > 2. && surface(i).abs() < 0.05)
            .count(),
        brake_timer: f(b, 0xe74),
    }
}

/// Manual policy: up near the limiter, down when slow, at most every 0.5 s.
fn manual_shift(
    r: &mut Runtime,
    last: &mut u64,
    braking: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if r.assists.transmission != Transmission::Manual || r.ticks < *last + 31 {
        return Ok(());
    }
    let b = &r.state.car.owner.vehicle;
    let (rpm, limit, gear) = (f(b, 0x1894), f(b, 0x17a4), r.gear());
    let top = (1..=8)
        .rev()
        .find(|&g| f(b, 0x18f4 + g * 4) > 0.)
        .unwrap_or(1) as u32;
    if !braking && (1..top).contains(&gear) && rpm > 0.92 * limit {
        r.request_shift(Shift::Up)?;
        *last = r.ticks;
    } else if braking && gear > 1 && gear < 9 && rpm < 0.45 * limit {
        r.request_shift(Shift::Down)?;
        *last = r.ticks;
    }
    Ok(())
}

fn controls(scenario: &str, tick: u64) -> (Controls, bool) {
    let full = Controls {
        throttle: 1.,
        ..Default::default()
    };
    match scenario {
        "launch" => (full, false),
        // 6 s of throttle, then full brake to rest.
        "brake" if tick < 375 => (full, false),
        "brake" => (
            Controls {
                brake: 1.,
                ..Default::default()
            },
            true,
        ),
        // 3 s straight, then a constant half-lock turn at 70 % throttle.
        "turn" if tick < 190 => (full, false),
        "turn" => (
            Controls {
                throttle: 0.7,
                steering: 0.5,
                ..Default::default()
            },
            false,
        ),
        // 8 s full throttle (shifts up), then lift and coast/brake gently.
        "shifts" if tick < 500 => (full, false),
        _ => (
            Controls {
                brake: 0.3,
                ..Default::default()
            },
            true,
        ),
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 || !Path::new(&args[1]).starts_with("local") {
        return Err("usage: grid-assists-drive ROOT local/OUTPUT_DIR".into());
    }
    let root = Path::new(&args[0]);
    let out = Path::new(&args[1]);
    std::fs::create_dir_all(out)?;
    let mut summary = std::io::BufWriter::new(std::fs::File::create(out.join("summary.tsv"))?);
    writeln!(
        summary,
        "assists\tscenario\tticks\tend_speed\tmax_speed\tdistance\tmax_front_slip\tlocked_ticks\tmax_brake_timer\tstop_ticks\tyaw_mean_last_2s\tgears\tfailure"
    )?;
    for bits in 0..32u32 {
        let assists = Assists {
            traction_control: (bits & 1 == 0).into(),
            stability_control: bits & 2 == 0,
            braking_assist: (bits & 4 == 0).into(),
            transmission: if bits & 8 == 0 {
                Transmission::Automatic
            } else {
                Transmission::Manual
            },
            input: if bits & 16 == 0 {
                Input::Keyboard
            } else {
                Input::Wheel
            },
            ..Assists::default()
        };
        let name = assists.to_string().replace([',', '='], "_");
        std::fs::create_dir_all(out.join(&name))?;
        for scenario in SCENARIOS {
            let mut r = Runtime::lacetti(root, "slot_00", 200679)?;
            r.set_assists(assists)?;
            let mut trace = std::io::BufWriter::new(std::fs::File::create(
                out.join(&name).join(format!("{scenario}.tsv")),
            )?);
            writeln!(
                trace,
                "tick\tthrottle\tbrake\tsteer_in\tsteer_e30\tspeed\tgear\trpm\tyaw\tfront_slip\tlocked\tbrake_timer"
            )?;
            let (mut max_speed, mut distance, mut max_slip, mut locked, mut max_timer) =
                (0f32, 0f32, 0f32, 0, 0f32);
            let (mut stop, mut yaw, mut yaw_n, mut last_shift, mut failure) =
                (None, 0f32, 0, 0, String::new());
            let mut gears = vec![r.gear()];
            let ticks = 700;
            for tick in 0..ticks {
                let (c, braking) = controls(scenario, tick);
                manual_shift(&mut r, &mut last_shift, braking)?;
                if let Err(e) = r.tick(c) {
                    failure = format!("tick {}: {e}", tick + 1);
                    break;
                }
                let s = sample(&r);
                let e30 = f(&r.state.car.owner.vehicle, 0xe30);
                writeln!(
                    trace,
                    "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                    r.ticks,
                    c.throttle,
                    c.brake,
                    c.steering,
                    e30,
                    s.speed,
                    s.gear,
                    s.rpm,
                    s.yaw,
                    s.front_slip,
                    s.locked,
                    s.brake_timer
                )?;
                max_speed = max_speed.max(s.speed);
                distance += s.speed * DELTA;
                if c.throttle > 0. {
                    max_slip = max_slip.max(s.front_slip);
                }
                locked += usize::from(s.locked > 0);
                max_timer = max_timer.max(s.brake_timer);
                if braking && stop.is_none() && s.speed < 0.5 && c.brake == 1. {
                    stop = Some(tick - 374);
                }
                if tick >= ticks - 125 {
                    yaw += s.yaw;
                    yaw_n += 1;
                }
                if gears.last() != Some(&s.gear) {
                    gears.push(s.gear);
                }
            }
            let end = sample(&r);
            writeln!(
                summary,
                "{assists}\t{scenario}\t{}\t{:.3}\t{:.3}\t{:.2}\t{:.4}\t{locked}\t{max_timer:.3}\t{}\t{:.4}\t{}\t{failure}",
                r.ticks,
                end.speed,
                max_speed,
                distance,
                max_slip,
                stop.map_or("-".into(), |t| t.to_string()),
                if yaw_n > 0 { yaw / yaw_n as f32 } else { 0. },
                gears
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(">")
            )?;
        }
        eprintln!("done {assists}");
    }
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("grid-assists-drive: {e}");
        std::process::exit(1);
    }
}
