//! Private barrier-impact probe: follow the purchased centreline to a switch
//! distance, then hold constant steering into the trackside, recording body
//! contacts by world shape and their penetration per tick. Authored scenario.
use grid_tools::source_vehicle::{Controls, Runtime};
use std::{fmt::Write as _, fs, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 9 {
        return Err("usage: grid-wall-probe ROOT CD01 BP01 DF01 local/OUT.tsv SWITCH_M SPEED_M_S STEER TICKS_AFTER".into());
    }
    let root = Path::new(&args[0]);
    let output = Path::new(&args[4]);
    let switch: f64 = args[5].parse()?;
    let cap: f64 = args[6].parse()?;
    let steer: f32 = args[7].parse()?;
    let after: u64 = args[8].parse()?;
    if !output
        .parent()
        .ok_or("output parent")?
        .canonicalize()?
        .starts_with(Path::new("local").canonicalize()?)
        || after > 2000
    {
        return Err("invalid probe configuration".into());
    }
    let prepared = grid_tools::lap_progress::load(root)?;
    let points: Vec<_> = prepared
        .progress
        .points
        .iter()
        .map(|p| (f64::from(p.distance), p.position.map(f64::from)))
        .collect();
    let mut runtime = Runtime::lacetti(root, "slot_00", 200679)?;
    let mut geometry = runtime.prepare_body_geometry(root, &fs::read(&args[1])?)?;
    runtime.enable_deformation(root, &mut geometry)?;
    runtime.body_events = Some(grid_tools::body_impact_parameters::read(&fs::read(
        &args[2],
    )?)?);
    let (limit, bulk) = grid_tools::deformation_parameters::read(&fs::read(&args[3])?)?;
    runtime.register_deformation_callbacks(limit, bulk, 1)?;
    // Contacts store shape+1; props are the last registered track shape.
    let props_shape = runtime.track_shape_count() as u32;
    let boundary_shape = props_shape - 1;
    let mut travelled = 0.;
    let mut last = None::<[f64; 3]>;
    let mut switched_at = None;
    let mut text = String::from(
        "tick\tphase\tspeed\tprops_contacts\tprops_max_pen\tboundary_contacts\tboundary_max_pen\tother_contacts\tbody_contacts\tx\tz\tslots\n",
    );
    let mut first = None;
    let mut peak = (0f32, 0f32);
    for step in 0..20_000u64 {
        let pose = runtime.pose().map(f64::from);
        let position = [pose[12], pose[13], pose[14]];
        if let Some(p) = last {
            travelled += ((position[0] - p[0]).powi(2) + (position[2] - p[2]).powi(2)).sqrt();
        }
        last = Some(position);
        let velocity = runtime.velocity().map(f64::from);
        let speed = (velocity[0] * velocity[0] + velocity[2] * velocity[2]).sqrt();
        let controls = if switched_at.is_none() && travelled < switch {
            // Pure pursuit 12 m ahead of the nearest centreline point.
            let nearest = points
                .iter()
                .enumerate()
                .min_by(|a, b| {
                    let d =
                        |p: &[f64; 3]| (p[0] - position[0]).powi(2) + (p[2] - position[2]).powi(2);
                    d(&a.1.1).total_cmp(&d(&b.1.1))
                })
                .map(|(i, _)| i)
                .unwrap();
            let target_distance = points[nearest].0 + 12.;
            let i = points
                .partition_point(|p| p.0 <= target_distance)
                .min(points.len() - 1);
            let target = points[i].1;
            let to = [target[0] - position[0], target[2] - position[2]];
            let forward = [pose[8], pose[10]];
            let side = [pose[0], pose[2]];
            let x = to[0] * side[0] + to[1] * side[1];
            let z = to[0] * forward[0] + to[1] * forward[1];
            let steering = (1.5 * x.atan2(z)).clamp(-1., 1.) as f32;
            Controls {
                throttle: if speed < cap { 1. } else { 0. },
                brake: if speed > cap + 3. { 0.5 } else { 0. },
                steering,
                ..Default::default()
            }
        } else {
            switched_at.get_or_insert(step);
            Controls {
                throttle: if speed < cap { 1. } else { 0. },
                steering: steer,
                ..Default::default()
            }
        };
        if let Some(s) = switched_at
            && step > s + after
        {
            break;
        }
        runtime.tick_with_deforming_body_geometry(controls, &mut geometry)?;
        let mut counts = [0usize; 3];
        let mut pens = [0f32; 2];
        for c in &runtime.state.world.groups.contacts[4..] {
            if c[1] != 1 || c[0x2c / 4] == 0 {
                continue;
            }
            let shape = c[0x2c / 4] - 1;
            let pen = f32::from_bits(c[0x38 / 4]).abs();
            if shape == props_shape {
                counts[0] += 1;
                pens[0] = pens[0].max(pen);
            } else if shape == boundary_shape {
                counts[1] += 1;
                pens[1] = pens[1].max(pen);
            } else {
                counts[2] += 1;
            }
        }
        if runtime.last_body_contacts == 0 {
            counts = [0; 3];
            pens = [0.; 2];
        }
        if switched_at.is_some() {
            if first.is_none() && counts[0] + counts[1] > 0 {
                first = Some((runtime.ticks, counts, pens, speed));
            }
            peak = (peak.0.max(pens[0]), peak.1.max(pens[1]));
            writeln!(
                text,
                "{}\t{}\t{speed:.3}\t{}\t{:.4}\t{}\t{:.4}\t{}\t{}\t{:.2}\t{:.2}\t{}",
                runtime.ticks,
                if switched_at.is_some() {
                    "steer"
                } else {
                    "follow"
                },
                counts[0],
                pens[0],
                counts[1],
                pens[1],
                counts[2],
                runtime.last_body_contacts,
                position[0],
                position[2],
                runtime.state.world.groups.contacts[4..]
                    .iter()
                    .filter(|c| c[0x2c / 4] != 0)
                    .map(|c| format!(
                        "{}:{}:{:.3}",
                        c[1],
                        c[0x2c / 4],
                        f32::from_bits(c[0x38 / 4])
                    ))
                    .collect::<Vec<_>>()
                    .join(",")
            )?;
        }
    }
    fs::write(output, text)?;
    println!(
        "wall_probe switch_m={switch} cap={cap} steer={steer} switched_tick={switched_at:?} first_barrier_contact={first:?} peak_props_pen={:.4} peak_boundary_pen={:.4}",
        peak.0, peak.1
    );
    Ok(())
}

fn main() {
    let result = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))
        .expect("probe worker")
        .join()
        .expect("probe worker panic");
    if let Err(error) = result {
        eprintln!("grid-wall-probe: {error}");
        std::process::exit(1);
    }
}
