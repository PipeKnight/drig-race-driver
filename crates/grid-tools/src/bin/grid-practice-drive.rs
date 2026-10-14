//! Independent diagnostic path follower; this is not original GRID AI.
//! Uses ordinary source physics/contacts/damage/timing without teleporting the car.
use grid_tools::source_vehicle::{Controls, Runtime};
use std::{fmt::Write as _, fs, io::Write as _, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 8 && args.len() != 9 {
        return Err("usage: grid-practice-drive ROOT CD01 BP01 DF01 local/OUTPUT TICKS SPEED_M_S STEERING_GAIN [TRIAL_LAPS]".into());
    }
    let root = Path::new(&args[0]);
    let output = Path::new(&args[4]);
    let ticks: u64 = args[5].to_str().ok_or("invalid ticks")?.parse()?;
    let maximum_speed: f64 = args[6].to_str().ok_or("invalid speed")?.parse()?;
    let gain: f64 = args[7].to_str().ok_or("invalid gain")?.parse()?;
    let trial_laps = args
        .get(8)
        .map(|s| {
            s.to_str()
                .ok_or("invalid trial laps")?
                .parse::<u32>()
                .map_err(|_| "invalid trial laps")
        })
        .transpose()?;
    if trial_laps.is_some_and(|n| !(1..=9).contains(&n)) {
        return Err("trial laps must be1..9".into());
    }
    if !(1..=100000).contains(&ticks)
        || !(5. ..=40.).contains(&maximum_speed)
        || !(0.1..=4.).contains(&gain)
        || !output
            .parent()
            .ok_or("output parent")?
            .canonicalize()?
            .starts_with(Path::new("local").canonicalize()?)
    {
        return Err("invalid bounded private practice-driver configuration".into());
    }
    let prepared = grid_tools::lap_progress::load(root)?;
    let points: Vec<_> = prepared
        .progress
        .points
        .iter()
        .map(|p| (f64::from(p.distance), p.position.map(f64::from)))
        .collect();
    let length = f64::from(prepared.progress.total_distance);
    let target_at = |distance: f64| -> [f64; 3] {
        let distance = distance.rem_euclid(length);
        let i = points
            .partition_point(|p| p.0 <= distance)
            .saturating_sub(1);
        let (a, from) = points[i];
        let (b, to) = if i + 1 < points.len() {
            points[i + 1]
        } else {
            (length, points[0].1)
        };
        let fraction = ((distance - a) / (b - a)).clamp(0., 1.);
        std::array::from_fn(|j| from[j] + fraction * (to[j] - from[j]))
    };
    let mut runtime = Runtime::lacetti(root, "slot_00", 200679)?;
    let mut geometry = runtime.prepare_body_geometry(root, &fs::read(&args[1])?)?;
    runtime.enable_deformation(root, &mut geometry)?;
    runtime.body_events = Some(grid_tools::body_impact_parameters::read(&fs::read(
        &args[2],
    )?)?);
    let (limit, bulk) = grid_tools::deformation_parameters::read(&fs::read(&args[3])?)?;
    runtime.register_deformation_callbacks(limit, bulk, 1)?;
    runtime.enable_lap_timing(root)?;
    let mut trial = trial_laps
        .map(|laps| {
            grid_tools::time_trial::State::new(
                laps,
                f64::from(grid_tools::source_vehicle::DELTA),
                3.,
            )
        })
        .transpose()?;
    if let Some(trial) = &mut trial {
        while matches!(trial.phase(), grid_tools::time_trial::Phase::Countdown(_)) {
            trial.countdown_tick();
        }
        assert_eq!(runtime.ticks, 0);
    }
    let mut nearest = 0usize;
    let mut text = format!("P\tindependent-pure-pursuit\t{ticks}\t{maximum_speed}\t{gain}\n");
    let mut failure = None;
    let mut body_contacts = 0usize;
    let mut maximum_lateral = 0_f64;
    let mut minimum_wheel_hits = 4;
    for step in 0..ticks {
        let pose = runtime.pose().map(f64::from);
        let position = [pose[12], pose[13], pose[14]];
        let velocity = runtime.velocity().map(f64::from);
        let speed = (velocity[0] * velocity[0] + velocity[2] * velocity[2]).sqrt();
        let mut best = (f64::INFINITY, 0., 0usize);
        for i in 0..points.len() {
            if step != 0 {
                let ahead = (i + points.len() - nearest) % points.len();
                let behind = (nearest + points.len() - i) % points.len();
                if ahead > 10 && behind > 5 {
                    continue;
                }
            }
            let (distance, from) = points[i];
            let (next_distance, to) = if i + 1 < points.len() {
                points[i + 1]
            } else {
                (length, points[0].1)
            };
            let dx = to[0] - from[0];
            let dz = to[2] - from[2];
            let square = dx * dx + dz * dz;
            if square <= 1e-9 {
                continue;
            }
            let t = (((position[0] - from[0]) * dx + (position[2] - from[2]) * dz) / square)
                .clamp(0., 1.);
            let error =
                (position[0] - from[0] - t * dx).powi(2) + (position[2] - from[2] - t * dz).powi(2);
            if error < best.0 {
                best = (error, distance + t * (next_distance - distance), i);
            }
        }
        nearest = best.2;
        let lateral = best.0.sqrt();
        maximum_lateral = maximum_lateral.max(lateral);
        let lookahead = (8. + speed * 0.7).clamp(8., 30.);
        let target = target_at(best.1 + lookahead);
        let dx = target[0] - position[0];
        let dz = target[2] - position[2];
        let local_x = dx * pose[0] + dz * pose[2];
        let curvature = 2. * local_x / (dx * dx + dz * dz).max(1.);
        let steering = (gain * (curvature * 2.65).atan() / 0.45).clamp(-1., 1.) as f32;
        let desired_speed = maximum_speed
            .min((3.5 / curvature.abs().max(0.002)).sqrt())
            .max(5.);
        let error = desired_speed - speed;
        let throttle = if error < -0.5 {
            0.
        } else {
            (0.22 + error * 0.3).clamp(0., 1.) as f32
        };
        let brake = if error < -0.5 {
            (-error * 0.25).clamp(0., 0.8) as f32
        } else {
            0.
        };
        if let Err(e) = runtime.tick_with_deforming_body_geometry(
            Controls {
                throttle,
                brake,
                steering,
                handbrake: false,
            },
            &mut geometry,
        ) {
            failure = Some(format!("tick {}: {e}", runtime.ticks + 1));
            break;
        }
        if let Some(trial) = &mut trial {
            trial.finish_tick(runtime.ticks, &runtime.lap_events)?;
        }
        body_contacts += runtime.last_body_contacts;
        minimum_wheel_hits = minimum_wheel_hits.min(
            (0..4)
                .filter(|i| runtime.state.car.owner.vehicle[(0x1990 + i * 0x150 + 0xc0) / 4] != 0)
                .count(),
        );
        let lap = runtime.lap.as_ref().unwrap();
        let position = runtime.pose();
        if runtime.ticks.is_multiple_of(16)
            || !runtime.lap_events.is_empty()
            || runtime.terminal_wheel().is_some()
        {
            writeln!(
                text,
                "T\t{}\t{:.9}\t{lateral:.9}\t{speed:.9}\t{throttle:.9}\t{brake:.9}\t{steering:.9}\t{}\t{}\t{}\t{}\t{}\t{:.9}\t{:.9}\t{:.9}",
                runtime.ticks,
                best.1,
                runtime.last_body_contacts,
                lap.valid_laps,
                lap.next_gate,
                lap.elapsed()
                    .map_or_else(|| "none".into(), |v| format!("{v:.9}")),
                runtime
                    .terminal_wheel()
                    .map_or_else(|| "none".into(), |v| v.to_string()),
                position[12],
                position[13],
                position[14]
            )?;
        }
        for event in &runtime.lap_events {
            writeln!(text, "E\t{}\t{event:?}", runtime.ticks)?;
        }
        if trial.as_ref().map_or(lap.valid_laps > 0, |t| {
            matches!(t.phase(), grid_tools::time_trial::Phase::Finished(_))
        }) || runtime.terminal_wheel().is_some()
        {
            break;
        }
        if text.len() > 16 * 1024 * 1024 {
            return Err("practice trace exceeds bound".into());
        }
    }
    let lap = runtime.lap.as_ref().unwrap();
    writeln!(
        text,
        "S\t{}\t{}\t{}\t{}\t{body_contacts}\t{maximum_lateral:.9}\t{}",
        runtime.ticks,
        lap.valid_laps,
        lap.best_lap
            .map_or_else(|| "none".into(), |v| format!("{v:.9}")),
        runtime
            .terminal_wheel()
            .map_or_else(|| "none".into(), |v| v.to_string()),
        failure.as_deref().unwrap_or("none")
    )?;
    let trial_incomplete = trial
        .as_ref()
        .is_some_and(|t| !matches!(t.phase(), grid_tools::time_trial::Phase::Finished(_)));
    if let Some(trial) = &trial
        && let grid_tools::time_trial::Phase::Finished(result) = trial.phase()
    {
        writeln!(
            text,
            "R\t{}\t{:.9}\t{:.9}\t{}",
            result.laps.len(),
            result.seconds,
            result.best_lap(),
            result.invalid_attempts
        )?;
        for lap in &result.laps {
            writeln!(text, "L\t{}\t{:.9}\t{}", lap.number, lap.seconds, lap.valid)?;
        }
    }
    let registration = runtime.deformation_registration.as_ref().unwrap();
    writeln!(
        text,
        "D\t{}\t{}\t{:08x}\t{:08x}\t{minimum_wheel_hits}",
        registration.bulk_calls,
        registration.records,
        registration.random_seed,
        runtime.state.car.owner.damage_random_state.unwrap()
    )?;
    // Complete final car and physical words, including untouched source-derived
    // configuration, remain private. Equality is stronger than sampled poses.
    text.push('V');
    for word in &runtime.state.car.owner.vehicle {
        write!(text, "\t{word:08x}")?;
    }
    text.push('\n');
    text.push('B');
    for word in &runtime.state.car.owner.physical {
        write!(text, "\t{word:08x}")?;
    }
    text.push('\n');
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(text.as_bytes())?;
    if trial_incomplete {
        return Err(
            "time trial did not complete within tick budget; private trace retained".into(),
        );
    }
    println!(
        "practice_drive ticks={} valid_laps={} contacts={body_contacts} max_lateral={maximum_lateral:.3} failure={}",
        runtime.ticks,
        lap.valid_laps,
        failure.as_deref().unwrap_or("none")
    );
    if let Some(failure) = failure {
        return Err(failure.into());
    }
    Ok(())
}
fn main() {
    let result = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))
        .expect("practice thread")
        .join()
        .expect("practice panic");
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
