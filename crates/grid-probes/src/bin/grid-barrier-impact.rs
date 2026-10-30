//! Private dynamic barrier check (docs/BARRIER_COLLISION.md): the purchased
//! car laps the route under the ported AI driver with the app's collision
//! world (body contacts, deformation, body events, dynamic props); every
//! SPACING metres the run is cloned and, on each side, the clone is steered
//! into the trackside at the current speed. Each branch records its contacts
//! and penetration by world shape and whether the car ends up beyond the
//! boundary sheet (the segment from the branch start to the car crosses a
//! sheet triangle) or below the road.
//!
//! Usage: grid-barrier-impact ROOT local/OUT.tsv ROUTE CAR SPEED_CAP SPACING_M [BRANCH_TICKS [FROM_M [TO_M]]]
//! Private inputs as grid-soak: GRID_CONTACT_DIRECTIONS (CD01),
//! GRID_BODY_EVENT_PARAMETERS (BP01), GRID_DEFORMATION_PARAMETERS (DF01).
use grid_formats::{binxml, cqtc, progress};
use grid_tools::{
    ai_driver::{AiData, AiDriver, NORMAL_DIFFICULTY, Parameters, View},
    car::Car,
    route::Route,
    source_vehicle::{Controls, DELTA, Runtime},
};
use std::{fmt::Write as _, fs, path::Path};

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Whether the segment `a`..`b` crosses triangle `t` (two-sided).
fn crosses(a: [f32; 3], b: [f32; 3], t: &[[f32; 3]; 3]) -> bool {
    let d = sub(b, a);
    let e1 = sub(t[1], t[0]);
    let e2 = sub(t[2], t[0]);
    let p = cross(d, e2);
    let det = dot(e1, p);
    if det.abs() < 1e-9 {
        return false;
    }
    let inv = 1. / det;
    let s = sub(a, t[0]);
    let u = dot(s, p) * inv;
    if !(0. ..=1.).contains(&u) {
        return false;
    }
    let q = cross(s, e1);
    let v = dot(d, q) * inv;
    if v < 0. || u + v > 1. {
        return false;
    }
    (0. ..=1.).contains(&(dot(e2, q) * inv))
}

fn private(variable: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let path = std::env::var_os(variable).ok_or(format!("set {variable}"))?;
    Ok(fs::read(path)?)
}

#[derive(Default)]
struct Branch {
    entry_speed: f32,
    first_contact: Option<(u64, f32)>,
    boundary_contacts: usize,
    boundary_pen: f32,
    /// Mean over boundary contacts of the contact normal along the side
    /// the car is steered to (negative: the normal points back to the track).
    boundary_normal: f32,
    prop_contacts: usize,
    prop_pen: f32,
    road_contacts: usize,
    /// First tick whose step crosses a sheet triangle: (tick, x, z, the
    /// triangle's material, whether the step came from the face-normal side,
    /// boundary contacts in the five ticks before).
    /// The last field: dynamic piece contacts in those five ticks.
    through: Option<(u64, f32, f32, String, bool, usize, usize)>,
    piece_contacts: usize,
    fell: bool,
    ticks: u64,
    error: Option<String>,
    final_offset: f32,
    final_speed: f32,
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !(6..=9).contains(&args.len()) {
        return Err("usage: grid-barrier-impact ROOT local/OUT.tsv ROUTE CAR SPEED_CAP SPACING_M [BRANCH_TICKS [FROM_M [TO_M]]]".into());
    }
    let root = Path::new(&args[0]);
    let output = Path::new(&args[1]);
    let route = Route::new(&args[2])?;
    let car = Car::new(&args[3])?;
    let cap: f32 = args[4].parse()?;
    let spacing: f32 = args[5].parse()?;
    let branch_ticks: u64 = args.get(6).map_or(Ok(220), |s| s.parse())?;
    let from: f32 = args.get(7).map_or(Ok(0.), |s| s.parse())?;
    let to: f32 = args.get(8).map_or(Ok(f32::MAX), |s| s.parse())?;
    if !output
        .parent()
        .ok_or("output parent")?
        .canonicalize()?
        .starts_with(Path::new("local").canonicalize()?)
        || branch_ticks > 2000
        || spacing < 10.
    {
        return Err("invalid probe configuration".into());
    }
    let progress_bytes = fs::read(root.join(route.file("progress_track.xml")))?;
    let document = binxml::read(&progress_bytes)?;
    let progress = progress::read(&document)?;
    let points: Vec<(f32, [f32; 3])> = progress.points.iter().map(|p| (p.distance, p.position)).collect();
    let total = points.last().ok_or("empty centreline")?.0;
    let boundary_bytes = fs::read(root.join(route.file("boundarylines.cqtc")))?;
    let boundary = cqtc::read(&boundary_bytes)?;
    let sheet: Vec<[[f32; 3]; 3]> = boundary
        .triangles
        .iter()
        .map(|t| t.indices.map(|i| boundary.positions[i as usize]))
        .collect();
    let keys: Vec<String> = boundary
        .triangles
        .iter()
        .map(|t| {
            boundary.materials[t.material as usize]
                .iter()
                .map(|&c| if c.is_ascii_graphic() { c as char } else { '_' })
                .collect()
        })
        .collect();
    // The first sheet triangle the step a..b crosses, and whether the step
    // starts on its face-normal side ((t0-t1) x (t0-t2), packed_contacts).
    let through = |a: [f32; 3], b: [f32; 3]| {
        sheet.iter().enumerate().find(|(_, t)| crosses(a, b, t)).map(|(i, t)| {
            let n = cross(sub(t[0], t[1]), sub(t[0], t[2]));
            (i, dot(n, sub(a, t[0])) > 0.)
        })
    };
    // Centreline distance and lateral offset of a position (nearest point).
    let locate = |p: [f32; 3]| -> (f32, f32, usize) {
        let i = points
            .iter()
            .enumerate()
            .min_by(|a, b| {
                let d = |q: &[f32; 3]| (q[0] - p[0]).powi(2) + (q[2] - p[2]).powi(2);
                d(&a.1.1).total_cmp(&d(&b.1.1))
            })
            .map_or(0, |(i, _)| i);
        let q = points[i].1;
        ((points[i].0), (q[0] - p[0]).hypot(q[2] - p[2]), i)
    };

    let mut runtime = Runtime::load(root, &route, &car, "slot_00", 200679)?;
    runtime.body_events = Some(grid_tools::body_impact_parameters::read(&private(
        "GRID_BODY_EVENT_PARAMETERS",
    )?)?);
    let mut geometry = runtime.prepare_body_geometry(root, &private("GRID_CONTACT_DIRECTIONS")?)?;
    runtime.enable_deformation(root, &mut geometry)?;
    let (limit, bulk) =
        grid_tools::deformation_parameters::read(&private("GRID_DEFORMATION_PARAMETERS")?)?;
    runtime.register_deformation_callbacks(limit, bulk, 1)?;
    runtime.enable_dynamic_props(root)?;
    let speed_of = |r: &Runtime| {
        let v = r.velocity();
        v[0].hypot(v[2])
    };
    // Routes without AI waypoints (the Yokohama arenas) are followed by
    // pure pursuit along the centreline instead.
    let mut ai = match AiData::load_route(root, &route, &car, None) {
        Ok(ai) => {
            let pilot = AiDriver::new(Parameters::new(&ai, NORMAL_DIFFICULTY, |_| 1.)?, 0x5eed);
            Some((ai, pilot))
        }
        Err(e) => {
            println!("no AI data ({e}): pure pursuit on the centreline");
            None
        }
    };
    let pursue = |r: &Runtime| -> Controls {
        let pose = r.pose();
        let p = [pose[12], pose[13], pose[14]];
        let (d, _, _) = locate(p);
        let at = |x: f32| {
            let x = x.rem_euclid(total);
            points[points.partition_point(|q| q.0 <= x).min(points.len() - 1)].1
        };
        let target = at(d + 12.);
        let to = [target[0] - p[0], target[2] - p[2]];
        let x = to[0] * pose[0] + to[1] * pose[2];
        let z = to[0] * pose[8] + to[1] * pose[10];
        // Corner speed from the turn over the next 40 m (lateral 6 m/s^2).
        let (a, b, c) = (at(d + 10.), at(d + 30.), at(d + 50.));
        let turn = {
            let u = [b[0] - a[0], b[2] - a[2]];
            let v = [c[0] - b[0], c[2] - b[2]];
            (u[0] * v[1] - u[1] * v[0]).atan2(u[0] * v[0] + u[1] * v[1]).abs()
        };
        let limit = if turn > 1e-3 { (6. * 20. / turn).sqrt() } else { f32::MAX };
        let speed = speed_of(r);
        Controls {
            throttle: if speed < limit { 1. } else { 0. },
            brake: if speed > limit + 2. { 0.6 } else { 0. },
            steering: (1.5 * x.atan2(z)).clamp(-1., 1.),
            ..Default::default()
        }
    };
    let props_shape = runtime.track_shape_count() as u32;
    let boundary_shape = props_shape - 1;

    let mut text = String::from(
        "distance\tside\tentry_speed\tfirst_contact_tick\tfirst_contact_speed\tboundary_contacts\tboundary_pen\tboundary_normal\tprop_contacts\tprop_pen\troad_contacts\tpiece_contacts\tthrough_tick\tthrough_x\tthrough_z\tthrough_key\tfrom_front\tcontacts_before\tpieces_before\tfell\tfinal_offset\tfinal_speed\tticks\terror\n",
    );
    // GRID_BARRIER_TRACE=D1,D2: per-tick trace of the branches at these distances.
    let trace: Vec<i64> = std::env::var("GRID_BARRIER_TRACE")
        .map(|v| v.split(',').filter_map(|x| x.parse().ok()).collect())
        .unwrap_or_default();
    let mut next = from.max(50.);
    let mut base_ticks = 0u64;
    let mut last_distance = f32::MIN;
    let mut previous = None::<(f32, f32)>;
    let mut stuck = 0u32;
    let mut summary = (0usize, 0usize, 0usize, 0usize);
    let capped = |mut c: Controls, speed: f32| {
        if speed > cap {
            c.throttle = 0.;
            c.brake = c.brake.max(((speed - cap) * 0.2).min(1.));
        }
        c
    };
    while base_ticks < 400_000 {
        let pose = runtime.pose();
        let position = [pose[12], pose[13], pose[14]];
        let (wrapped, _, _) = locate(position);
        // Distance unwrapped from the grid, which may stand behind the line.
        let distance = match previous {
            None if wrapped > total * 0.5 => wrapped - total,
            None => wrapped,
            Some((w, u)) => {
                let mut delta = wrapped - w;
                if delta < -total * 0.5 {
                    delta += total;
                } else if delta > total * 0.5 {
                    delta -= total;
                }
                u + delta
            }
        };
        previous = Some((wrapped, distance));
        let speed = speed_of(&runtime);
        last_distance = last_distance.max(distance);
        if distance >= to.min(total - 30.) {
            break;
        }
        stuck = if speed < 1. && base_ticks > 600 { stuck + 1 } else { 0 };
        if stuck > 300 {
            println!("base run stuck at distance {distance:.0}");
            break;
        }
        if distance >= next && speed > 5. {
            next = distance + spacing;
            for side in [1f32, -1.] {
                let mut r = runtime.clone();
                let mut g = geometry.clone();
                let mut b = Branch {
                    entry_speed: speed,
                    ..Default::default()
                };
                let start = position;
                let start_offset = locate(start).1;
                let mut slow = 0;
                let mut previous = start;
                let mut recent = [0usize; 5];
                let mut recent_pieces = [0usize; 5];
                let mut no_ground = 0;
                for _ in 0..branch_ticks {
                    let pose = r.pose();
                    let p = [pose[12], pose[13], pose[14]];
                    let (d, _, i) = locate(p);
                    // Aim 25 m ahead, 60 m to the side of the centreline.
                    let j = points.partition_point(|q| q.0 <= d + 25.).min(points.len() - 1);
                    let t = sub(points[(i + 1).min(points.len() - 1)].1, points[i.saturating_sub(1)].1);
                    let l = t[0].hypot(t[2]).max(1e-6);
                    let left = [t[2] / l, 0., -t[0] / l];
                    let q = points[j].1;
                    let aim = [q[0] + left[0] * 60. * side, q[1], q[2] + left[2] * 60. * side];
                    let to = [aim[0] - p[0], aim[2] - p[2]];
                    let x = to[0] * pose[0] + to[1] * pose[2];
                    let z = to[0] * pose[8] + to[1] * pose[10];
                    let s = speed_of(&r);
                    let wall = left.map(|x| x * side);
                    let controls = Controls {
                        throttle: if s < b.entry_speed { 1. } else { 0. },
                        steering: (1.5 * x.atan2(z)).clamp(-1., 1.),
                        ..Default::default()
                    };
                    if let Err(e) = r.tick_with_deforming_body_geometry(controls, &mut g) {
                        b.error = Some(e.to_string());
                        break;
                    }
                    b.ticks += 1;
                    recent.rotate_right(1);
                    recent_pieces.rotate_right(1);
                    recent[0] = 0;
                    recent_pieces[0] = 0;
                    let active = r.last_active_contacts.min(r.state.world.groups.contacts.len());
                    for c in &r.state.world.groups.contacts[4.min(active)..active] {
                        // The car is token 1; a token past the track shapes
                        // is a dynamic prop piece or a lost part.
                        let [a, o] = [c[0x28 / 4], c[0x2c / 4]];
                        if (a == 1) != (o == 1) && a.max(o) > props_shape + 1 {
                            b.piece_contacts += 1;
                            recent_pieces[0] += 1;
                            continue;
                        }
                        if c[1] != 1 || c[0x2c / 4] == 0 {
                            continue;
                        }
                        let shape = c[0x2c / 4] - 1;
                        let pen = f32::from_bits(c[0x38 / 4]).abs();
                        if shape == props_shape {
                            b.prop_contacts += 1;
                            b.prop_pen = b.prop_pen.max(pen);
                        } else if shape == boundary_shape {
                            b.boundary_contacts += 1;
                            let n: [f32; 3] =
                                std::array::from_fn(|k| f32::from_bits(c[0x60 / 4 + k]));
                            b.boundary_normal += dot(n, wall);
                            recent[0] += 1;
                            b.boundary_pen = b.boundary_pen.max(pen);
                        } else {
                            b.road_contacts += 1;
                            continue;
                        }
                        b.first_contact.get_or_insert((b.ticks, s));
                    }
                    let pose = r.pose();
                    let p = [pose[12], pose[13], pose[14]];
                    let lift = |q: [f32; 3]| [q[0], q[1] + 0.5, q[2]];
                    if b.through.is_none()
                        && let Some((i, front)) = through(lift(previous), lift(p))
                    {
                        eprintln!(
                            "crossing d={distance:.0} side={side} tick={} step {:?} -> {:?} triangle {i} {:?}",
                            b.ticks, previous, p, sheet[i]
                        );
                        b.through = Some((
                            b.ticks,
                            p[0],
                            p[2],
                            keys[i].clone(),
                            front,
                            recent.iter().sum(),
                            recent_pieces.iter().sum(),
                        ));
                    }
                    previous = p;
                    if trace.contains(&(distance.round() as i64)) {
                        let v = r.velocity();
                        eprintln!(
                            "trace d={distance:.0} side={side} tick={} p={:.2},{:.2},{:.2} v={:.2},{:.2},{:.2} ground={:?} boundary={} props={} road={}",
                            b.ticks, p[0], p[1], p[2], v[0], v[1], v[2],
                            r.ground_below([p[0], p[1] + 2., p[2]]),
                            recent[0], b.prop_contacts, b.road_contacts
                        );
                        eprintln!("trace pieces={}", recent_pieces[0]);
                    }
                    // Below the road, or over no road for 15 ticks: a single
                    // vertical query can miss a sliver between chunks.
                    let ground = r.ground_below([p[0], p[1] + 2., p[2]]);
                    no_ground = if ground.is_none() { no_ground + 1 } else { 0 };
                    if ground.is_some_and(|y| p[1] < y - 3.) || no_ground >= 15 {
                        b.fell = true;
                    }
                    slow = if speed_of(&r) < 1. { slow + 1 } else { 0 };
                    if slow > 40 || b.fell {
                        break;
                    }
                }
                let pose = r.pose();
                b.final_offset = locate([pose[12], pose[13], pose[14]]).1 - start_offset;
                b.final_speed = speed_of(&r);
                summary.0 += 1;
                summary.1 += usize::from(b.through.is_some());
                summary.2 += usize::from(b.fell);
                summary.3 += usize::from(b.error.is_some());
                let opt = |o: Option<u64>| o.map_or("-".to_owned(), |v| v.to_string());
                writeln!(
                    text,
                    "{distance:.0}\t{}\t{:.2}\t{}\t{}\t{}\t{:.4}\t{:.3}\t{}\t{:.4}\t{}\t{}\t{}\t{}\t{:.2}\t{:.2}\t{}\t{}",
                    if side > 0. { "left" } else { "right" },
                    b.entry_speed,
                    opt(b.first_contact.map(|f| f.0)),
                    b.first_contact.map_or("-".into(), |f| format!("{:.2}", f.1)),
                    b.boundary_contacts,
                    b.boundary_pen,
                    b.boundary_normal / b.boundary_contacts.max(1) as f32,
                    b.prop_contacts,
                    b.prop_pen,
                    b.road_contacts,
                    b.piece_contacts,
                    b.through.as_ref().map_or("-\t-\t-\t-\t-\t-\t-".to_owned(), |t| format!(
                        "{}\t{:.2}\t{:.2}\t{}\t{}\t{}\t{}",
                        t.0,
                        t.1,
                        t.2,
                        t.3,
                        u8::from(t.4),
                        t.5,
                        t.6
                    )),
                    u8::from(b.fell),
                    b.final_offset,
                    b.final_speed,
                    b.ticks,
                    b.error.as_deref().unwrap_or("-"),
                )?;
            }
            fs::write(output, &text)?;
        }
        let controls = match &mut ai {
            Some((ai, pilot)) => pilot.controls(ai, &View::of(&runtime, [0.; 3]), &[], DELTA),
            None => pursue(&runtime),
        };
        let controls = capped(controls, speed);
        if let Err(e) = runtime.tick_with_deforming_body_geometry(controls, &mut geometry) {
            println!("base run failed at distance {distance:.0}: {e}");
            break;
        }
        base_ticks += 1;
    }
    fs::write(output, &text)?;
    println!(
        "barrier_impact route={} car={} cap={cap} spacing={spacing} base_ticks={base_ticks} reached={last_distance:.0}/{total:.0} branches={} through={} fell={} errors={}",
        route.path(),
        car.stem(),
        summary.0,
        summary.1,
        summary.2,
        summary.3
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
        eprintln!("grid-barrier-impact: {error}");
        std::process::exit(1);
    }
}
