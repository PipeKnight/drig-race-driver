//! Private breakable-prop probe: follow the purchased centreline until the
//! chosen breakable route prop is near, then steer straight at it, recording
//! breaks, live pieces and their motion per tick. Authored scenario; the
//! physics are the runtime's (`enable_dynamic_props`). `PROBE_MULTI=1` runs
//! the same car through the shared multi-car world.
use grid_tools::{
    body_geometry::BodyGeometry,
    dynamic_props::PropWorld,
    source_vehicle::{Controls, Runtime, multi::MultiRuntime},
    track::Track,
};
use grid_world::contact_groups::Body;
use std::{fmt::Write as _, fs, path::Path};

enum Sim {
    Single(Box<Runtime>, Box<BodyGeometry>),
    Multi(Box<MultiRuntime>),
}

impl Sim {
    fn car(&self) -> &Runtime {
        match self {
            Sim::Single(r, _) => r,
            Sim::Multi(m) => &m.cars[0],
        }
    }
    fn ticks(&self) -> u64 {
        match self {
            Sim::Single(r, _) => r.ticks,
            Sim::Multi(m) => m.ticks,
        }
    }
    fn tick(&mut self, controls: Controls) -> Result<(), grid_world::Error> {
        match self {
            Sim::Single(r, g) => r.tick_with_deforming_body_geometry(controls, g),
            Sim::Multi(m) => m.tick(&[controls]),
        }
    }
    fn props(&self) -> &PropWorld {
        match self {
            Sim::Single(r, _) => r.dynamic_props().unwrap(),
            Sim::Multi(m) => m.dynamic_props().unwrap(),
        }
    }
    fn bodies(&self) -> &[Body] {
        match self {
            Sim::Single(r, _) => &r.state.world.groups.bodies,
            Sim::Multi(m) => m.bodies(),
        }
    }
    fn track(&self) -> &Track {
        self.car().track()
    }
    fn body_contacts(&self) -> usize {
        match self {
            Sim::Single(r, _) => r.last_body_contacts,
            Sim::Multi(m) => m.last_body_contacts,
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 9 {
        return Err("usage: grid-prop-probe ROOT CD01 BP01 DF01 local/OUT.tsv TEMPLATE INDEX SPEED_M_S TICKS_AFTER".into());
    }
    let root = Path::new(&args[0]);
    let output = Path::new(&args[4]);
    let template = &args[5];
    let index: usize = args[6].parse()?;
    let cap: f64 = args[7].parse()?;
    let after: u64 = args[8].parse()?;
    if !output
        .parent()
        .ok_or("output parent")?
        .canonicalize()?
        .starts_with(Path::new("local").canonicalize()?)
        || after > 3000
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
    let table = fs::read(&args[1])?;
    let events = grid_tools::body_impact_parameters::read(&fs::read(&args[2])?)?;
    let (limit, bulk) = grid_tools::deformation_parameters::read(&fs::read(&args[3])?)?;
    let mut runtime = if std::env::var_os("PROBE_MULTI").is_some() {
        let mut multi = MultiRuntime::lacetti_deforming(
            root,
            &["slot_00"],
            &table,
            Some(events),
            Some((limit, bulk)),
        )?;
        multi.enable_dynamic_props(root)?;
        Sim::Multi(Box::new(multi))
    } else {
        let mut runtime = Runtime::lacetti(root, "slot_00", 200679)?;
        let mut geometry = runtime.prepare_body_geometry(root, &table)?;
        runtime.enable_deformation(root, &mut geometry)?;
        runtime.body_events = Some(events);
        runtime.register_deformation_callbacks(limit, bulk, 1)?;
        runtime.enable_dynamic_props(root)?;
        Sim::Single(Box::new(runtime), Box::new(geometry))
    };
    let catalog = runtime.props().catalog.clone();
    eprintln!(
        "breakables={} unported={:?}",
        catalog.breakables.len(),
        catalog.unported
    );
    if std::env::var_os("PROBE_LIST").is_some() {
        let p = runtime.car().pose();
        for b in &catalog.breakables {
            let d = ((b.placement[12] - p[12]).powi(2) + (b.placement[14] - p[14]).powi(2)).sqrt();
            // Start-frame coordinates: +X left, +Z forward (car axes).
            let rel = [b.placement[12] - p[12], b.placement[14] - p[14]];
            let x = rel[0] * p[0] + rel[1] * p[2];
            let z = rel[0] * p[8] + rel[1] * p[10];
            println!("{} distance={d:.1} left={x:.1} ahead={z:.1}", b.template);
        }
        return Ok(());
    }
    let candidates: Vec<usize> = catalog
        .breakables
        .iter()
        .enumerate()
        .filter(|(_, b)| b.template.starts_with(template.as_str()))
        .map(|(i, _)| i)
        .collect();
    let target_index = *candidates.get(index).ok_or("no such breakable")?;
    let target = &catalog.breakables[target_index];
    let goal = [
        f64::from(target.placement[12]),
        f64::from(target.placement[13]),
        f64::from(target.placement[14]),
    ];
    // Start the approach where the centreline passes nearest the prop.
    let (near, _) = points
        .iter()
        .map(|p| (p.0, (p.1[0] - goal[0]).powi(2) + (p.1[2] - goal[2]).powi(2)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .unwrap();
    let switch = near - 45.;
    eprintln!(
        "target {} #{index} entity={} at ({:.1},{:.1},{:.1}) threshold={} explosion={} pieces={} switch_m={switch:.0}",
        target.template,
        target.entity,
        goal[0],
        goal[1],
        goal[2],
        target.threshold,
        target.explosion,
        target.pieces.len()
    );
    let mut travelled = 0.;
    let mut last = None::<[f64; 3]>;
    let mut switched_at = None;
    let mut text = String::from(
        "tick\tphase\tspeed\tx\tz\tdistance\tbreaks\tpieces\tpiece_heights\tpiece_speeds\tbody_contacts\n",
    );
    let mut first_break = None;
    let mut break_speed = 0.;
    for step in 0..20_000u64 {
        let pose = runtime.car().pose().map(f64::from);
        let position = [pose[12], pose[13], pose[14]];
        if let Some(p) = last {
            travelled += ((position[0] - p[0]).powi(2) + (position[2] - p[2]).powi(2)).sqrt();
        }
        last = Some(position);
        let velocity = runtime.car().velocity().map(f64::from);
        let speed = (velocity[0] * velocity[0] + velocity[2] * velocity[2]).sqrt();
        let aim = if switched_at.is_none() && travelled < switch {
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
            points[i].1
        } else {
            switched_at.get_or_insert(step);
            goal
        };
        let to = [aim[0] - position[0], aim[2] - position[2]];
        let forward = [pose[8], pose[10]];
        let side = [pose[0], pose[2]];
        let x = to[0] * side[0] + to[1] * side[1];
        let z = to[0] * forward[0] + to[1] * forward[1];
        let broken_at = first_break
            .as_ref()
            .map(|b: &grid_tools::dynamic_props::BreakEvent| b.tick);
        // Half a second after the break: brake straight to a stop.
        let stopping = broken_at.is_some_and(|t| runtime.ticks() > t + 30);
        let controls = if stopping {
            Controls {
                brake: 1.,
                ..Default::default()
            }
        } else {
            Controls {
                throttle: if speed < cap { 1. } else { 0. },
                brake: if speed > cap + 3. { 0.5 } else { 0. },
                steering: (1.5 * x.atan2(z)).clamp(-1., 1.) as f32,
                ..Default::default()
            }
        };
        if broken_at.is_some_and(|t| runtime.ticks() > t + after)
            || switched_at.is_some_and(|s| step > s + 1500)
        {
            break;
        }
        runtime.tick(controls)?;
        let props = runtime.props();
        if first_break.is_none() && !props.events.is_empty() {
            first_break = Some(props.events[0].clone());
            break_speed = speed;
        }
        if switched_at.is_some() {
            let bodies = runtime.bodies();
            let heights: Vec<String> = props
                .poses(bodies)
                .iter()
                .map(|(_, _, m)| format!("{:.2}", m[3][1]))
                .collect();
            let speeds: Vec<String> = props
                .pieces
                .iter()
                .map(|p| {
                    let b = &bodies[p.body];
                    let v: Vec<f32> = (0..3).map(|k| f32::from_bits(b[0x220 / 4 + k])).collect();
                    format!("{:.2}", (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
                })
                .collect();
            let distance =
                ((position[0] - goal[0]).powi(2) + (position[2] - goal[2]).powi(2)).sqrt();
            writeln!(
                text,
                "{}\t{}\t{speed:.3}\t{:.2}\t{:.2}\t{distance:.2}\t{}\t{}\t{}\t{}\t{}",
                runtime.ticks(),
                if stopping { "stop" } else { "approach" },
                position[0],
                position[2],
                props.events.len(),
                props.pieces.len(),
                heights.join(","),
                speeds.join(","),
                runtime.body_contacts(),
            )?;
        }
    }
    fs::write(output, text)?;
    let props = runtime.props();
    let mut cache = grid_world::rectangle_cull::Cache::default();
    for (b, p, m) in props.poses(runtime.bodies()) {
        let [x, y, z] = [m[3][0], m[3][1], m[3][2]];
        let ground = runtime
            .track()
            .query([x, y + 5., z, 1.], [x, y - 20., z, 1.], &mut cache)?
            .hit
            .map(|h| y + 5. - 25. * h.contact.fraction);
        let spec = &props.catalog.breakables[b].pieces[p];
        println!(
            "piece {} {:?} mass={:.1} centre=({x:.2},{y:.3},{z:.2}) ground={ground:?} up=({:.2},{:.2},{:.2})",
            spec.anchor, spec.primitive, spec.mass, m[1][0], m[1][1], m[1][2]
        );
    }
    println!(
        "prop_probe target={} first_break={first_break:?} car_speed_at_break={break_speed:.2} breaks={} pieces={} final_speed={:.2}",
        target.template,
        props.events.len(),
        props.pieces.len(),
        {
            let v = runtime.car().velocity();
            (v[0] * v[0] + v[2] * v[2]).sqrt()
        }
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
        eprintln!("grid-prop-probe: {error}");
        std::process::exit(1);
    }
}
