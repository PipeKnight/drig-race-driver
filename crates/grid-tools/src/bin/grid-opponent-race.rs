//! Private headless race: N source cars in one shared world. Cars are driven by
//! the ported original AI driver (`ai`, default) or the authored pursuit
//! placeholder (`pursuit`). Reports laps, car-to-car contacts, failures, the
//! classification (race rules in docs/RACE_RULES_RESEARCH.md) and an optional
//! per-tick TSV of car 0's AI telemetry. `--route tracks/<region>/<track>/<route>`
//! (anywhere in the arguments) selects the circuit; Donington route_0 by default.
//! `--cars STEM[,STEM...]` (anywhere) sets the car model of each grid slot,
//! cycled over the field (`bmw,lac` alternates); all Lacetti by default.
use grid_tools::{
    ai_driver::{AiData, AiDriver, NORMAL_DIFFICULTY, Other, Parameters, View},
    car::Car,
    pursuit_driver::PursuitDriver,
    race::{Out, Status},
    route::Route,
    source_vehicle::{Controls, Runtime, multi::MultiRuntime},
};
use std::{io::Write, path::Path};

enum Driver {
    Ai(Box<AiDriver>),
    Pursuit(PursuitDriver),
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let route = match args.iter().position(|a| a == "--route") {
        Some(i) if i + 1 < args.len() => {
            let route = Route::new(&args[i + 1])?;
            args.drain(i..i + 2);
            route
        }
        Some(_) => return Err("--route needs tracks/<region>/<track>/<route>".into()),
        None => Route::default(),
    };
    // `--demolition`: the Demolition Derby rules (grid_tools::demolition).
    let demolition = match args.iter().position(|a| a == "--demolition") {
        Some(i) => {
            args.remove(i);
            true
        }
        None => false,
    };
    // `--subroute 1`: the point-to-point route's second (reverse) direction.
    let route = match args.iter().position(|a| a == "--subroute") {
        Some(i) if i + 1 < args.len() => {
            let subroute = args[i + 1].parse()?;
            args.drain(i..i + 2);
            route.with_subroute(subroute)?
        }
        Some(_) => return Err("--subroute needs 0 or 1".into()),
        None => route,
    };
    let models = match args.iter().position(|a| a == "--cars") {
        Some(i) if i + 1 < args.len() => {
            let models = Car::list(&args[i + 1])?;
            args.drain(i..i + 2);
            models
        }
        Some(_) => return Err("--cars needs STEM[,STEM...]".into()),
        None => vec![Car::default()],
    };
    // `--discipline drift_battle`: the drift_battle AI config and choreographer
    // set, every car scored by the drift scorer (docs/DISCIPLINES.md).
    // `--discipline drift_grand_prix`: one car on the track (an AI drift
    // driver stands in for the player, AUTHORED) and the pool's other cars
    // as virtual rivals, estimated at the race end.
    use grid_tools::virtual_race::Format;
    let discipline = match args.iter().position(|a| a == "--discipline") {
        Some(i) => {
            let format = args
                .get(i + 1)
                .and_then(|name| Format::from_name(name))
                .filter(|f| *f != Format::DriftFreestyle)
                .ok_or("--discipline needs drift_battle or drift_grand_prix")?;
            args.drain(i..i + 2);
            Some(format)
        }
        None => None,
    };
    let drift = discipline.is_some();
    // The drift AI config: the stand-in player's drift line and behaviour in
    // a Drift GP come from the drift_battle section (the drift_grand_prix
    // section switches every behaviour off).
    // A derby reads the single_demolition_race section [D].
    let format = if demolition {
        Some("single_demolition_race")
    } else {
        drift.then_some("drift_battle")
    };
    // GRID_AI_CLASS=CLASS: another ai_vehicle_config/ai_vehicle_track class
    // for every car (research); by default each car's own database classes.
    let class_variable = std::env::var("GRID_AI_CLASS").ok();
    if !(5..=7).contains(&args.len()) {
        return Err(
            "usage: grid-opponent-race [--route ROUTE] [--subroute N] [--cars STEM,...] [--discipline drift_battle|drift_grand_prix] [--demolition] ROOT CD01 BP01 CARS TICKS [ai|pursuit] [local/TRACE.tsv]"
                .into(),
        );
    }
    let root = Path::new(&args[0]);
    let table = std::fs::read(&args[1])?;
    let events = Some(grid_tools::body_impact_parameters::read(&std::fs::read(
        &args[2],
    )?)?);
    let cars: usize = args[3].parse()?;
    let ticks: u64 = args[4].parse()?;
    if !(1..=20).contains(&cars) || ticks > 200_000 {
        return Err("1..20 cars (the routes have 20 grid slots) and at most 200000 ticks".into());
    }
    let kind = args.get(5).map_or("ai", String::as_str);
    let mut trace = match args.get(6) {
        Some(path) => {
            if !Path::new(path).starts_with("local") {
                return Err("trace must stay under local/".into());
            }
            Some(std::io::BufWriter::new(std::fs::File::create(path)?))
        }
        None => None,
    };
    let slots: Vec<String> = (0..cars).map(|i| format!("slot_{i:02}")).collect();
    let slot_refs: Vec<&str> = slots.iter().map(String::as_str).collect();
    // GRID_DEFORMATION_PARAMETERS=private DF01: every car deforms, as in the
    // windowed race.
    let deformation = match std::env::var_os("GRID_DEFORMATION_PARAMETERS") {
        Some(path) => Some(grid_tools::deformation_parameters::read(&std::fs::read(
            path,
        )?)?),
        None => None,
    };
    let field: Vec<&Car> = (0..cars).map(|i| &models[i % models.len()]).collect();
    let entries: Vec<(&str, &Car)> = slot_refs
        .iter()
        .copied()
        .zip(field.iter().copied())
        .collect();
    let mut multi =
        MultiRuntime::load_deforming_on(root, &route, &entries, &table, events, deformation)?;
    // GRID_DYNAMIC_PROPS=1: breakable route props (DYNAMIC_PROPS_RESEARCH.md).
    if std::env::var_os("GRID_DYNAMIC_PROPS").is_some() {
        multi.enable_dynamic_props(root)?;
    }
    // Each car's own AI classes and statistics (brake tables).
    let mut by_model = std::collections::BTreeMap::new();
    for model in &models {
        let data = match &class_variable {
            Some(class) => AiData::load_named(
                root,
                route.path(),
                route.ai_track(),
                (class, class),
                model.stem(),
                format,
            )?,
            None => AiData::load_route(root, &route, model, format)?,
        };
        by_model.insert(model.clone(), data);
    }
    let car_data: Vec<&AiData> = field.iter().map(|m| &by_model[*m]).collect();
    let data = car_data[0];
    let progress = grid_tools::lap_progress::load_route(root, &route)?.progress;
    // GRID_AI_CHARACTERISTICS=private TSV: per-driver characteristics (row k = car k).
    let characteristics = match std::env::var_os("GRID_AI_CHARACTERISTICS") {
        Some(path) => grid_tools::ai_driver::read_characteristics(&std::fs::read_to_string(path)?)?,
        None => Vec::new(),
    };
    let characteristic = |car: usize, name: &str| {
        characteristics
            .get(car % characteristics.len().max(1))
            .and_then(|row| row.get(name).copied())
            .unwrap_or(1.)
    };
    // GRID_REFERENCE_CAR=k: car k stands in for the human player (it still
    // drives with the AI and is never paced); the race manager paces the
    // others with GRID_AI_SETTINGS (grid_tools::ai_pacing::Settings).
    // Without it nothing is paced, as before.
    let reference: Option<usize> = std::env::var("GRID_REFERENCE_CAR")
        .ok()
        .map(|v| v.parse())
        .transpose()?
        .filter(|&k| k < cars);
    let settings = match std::env::var("GRID_AI_SETTINGS") {
        Ok(spec) => grid_tools::ai_pacing::Settings::parse(&spec)?,
        Err(_) => grid_tools::ai_pacing::Settings::default(),
    };
    let difficulty = if reference.is_some() {
        settings.difficulty
    } else {
        NORMAL_DIFFICULTY
    };
    let mut drivers: Vec<Driver> = (0..cars)
        .map(|i| -> Result<Driver, Box<dyn std::error::Error>> {
            Ok(match kind {
                "ai" => Driver::Ai(Box::new(AiDriver::new(
                    Parameters::new(car_data[i], difficulty, |name| {
                        characteristics
                            .get(i % characteristics.len().max(1))
                            .and_then(|row| row.get(name).copied())
                            .unwrap_or(1.)
                    })?,
                    0x5eed + i as u32,
                ))),
                "pursuit" => Driver::Pursuit(PursuitDriver::new(&progress, 18. + i as f64 * 0.5)),
                _ => return Err("driver must be ai or pursuit".into()),
            })
        })
        .collect::<Result<_, _>>()?;
    println!(
        "route={}{} driver={kind} maximum_steering_angle={} line_length={}",
        route.path(),
        // Absent for the default field, so its output stays byte-identical.
        if models == [Car::default()] {
            String::new()
        } else {
            let stems: Vec<&str> = field.iter().map(|c| c.stem()).collect();
            format!(" cars={}", stems.join(","))
        },
        multi.cars[0].maximum_steering_angle(),
        data.line.length
    );
    if let Some(t) = &mut trace {
        writeln!(
            t,
            "tick\tspeed\tthrottle\tbrake\tsteering\tarc\tlook_ahead\tcurvature\theading_error\tline_distance\tline_target\toffset\tline_error\tslip\treverse_gear\tbehaviour"
        )?;
    }
    let positions_of = |multi: &MultiRuntime| -> Vec<[f32; 3]> {
        multi
            .cars
            .iter()
            .map(|c| {
                let p = c.pose();
                [p[12], p[13], p[14]]
            })
            .collect()
    };
    let start = {
        let gate = &progress.gates[0];
        std::array::from_fn(|k| (gate.left[k] + gate.right[k]) * 0.5)
    };
    // GRID_RACE_LAPS=n (default 2).
    let race_laps: u32 = std::env::var("GRID_RACE_LAPS")
        .ok()
        .map(|v| v.parse())
        .transpose()?
        .unwrap_or(2);
    // A point-to-point run is one run from the start gate to the finish gate.
    let open = progress.kind == grid_formats::progress::TrackKind::PointToPoint;
    let mut standings = if open {
        let gate = progress.gates.last().unwrap();
        let finish = std::array::from_fn(|k| (gate.left[k] + gate.right[k]) * 0.5);
        println!(
            "point_to_point subroute={} run_length={:.2}",
            route.subroute(),
            progress.total_distance
        );
        grid_tools::race::Standings::new_run(&data.line, start, finish, &positions_of(&multi))
    } else {
        grid_tools::race::Standings::new(&data.line, race_laps, start, &positions_of(&multi))
    };
    let race_laps = if open { 1 } else { race_laps };
    let names: Vec<String> = (0..cars).map(|i| format!("car{i}")).collect();
    let mut field = match reference {
        Some(k) => {
            println!("pacing reference=car{k} settings={settings} laps={race_laps}");
            Some(grid_tools::ai_pacing::Field::new(
                grid_tools::ai_pacing::Data::load_for(
                    root,
                    &settings,
                    format.unwrap_or("circuit"),
                )?,
                settings.clone(),
                &data.line,
                race_laps,
                start,
                &positions_of(&multi),
                k,
                (0..cars).map(|i| characteristic(i, "mistakes")).collect(),
                0x5eed,
            ))
        }
        None => None,
    };
    // GRID_BLOW_TYRE=car:tick[:wheel] (with GRID_REFERENCE_CAR): fail a wheel.
    let blow_tyre: Option<(usize, u64, usize)> = match std::env::var("GRID_BLOW_TYRE") {
        Ok(v) => {
            let parts: Vec<&str> = v.split(':').collect();
            if field.is_none() || !(2..=3).contains(&parts.len()) {
                return Err("GRID_BLOW_TYRE=car:tick[:wheel] needs GRID_REFERENCE_CAR".into());
            }
            Some((
                parts[0].parse()?,
                parts[1].parse()?,
                parts.get(2).map_or(Ok(0), |w| w.parse())?,
            ))
        }
        Err(_) => None,
    };
    // GRID_LOSE_WHEEL=car:tick[:wheel] (default 2, front left): that car loses
    // the wheel after the tick, as the loose-part rule would; the wheel
    // becomes a debris body of the shared world (F353).
    let lose_wheel: Option<(usize, u64, usize)> = match std::env::var("GRID_LOSE_WHEEL") {
        Ok(v) => {
            let parts: Vec<&str> = v.split(':').collect();
            if !(2..=3).contains(&parts.len()) {
                return Err("GRID_LOSE_WHEEL=car:tick[:wheel]".into());
            }
            Some((
                parts[0].parse()?,
                parts[1].parse()?,
                parts.get(2).map_or(Ok(2), |w| w.parse())?,
            ))
        }
        Err(_) => None,
    };
    // GRID_PACING_TRACE=local/X.tsv: per tick and AI car the race-manager state.
    let mut pacing_trace = match std::env::var("GRID_PACING_TRACE") {
        Ok(path) => {
            if !Path::new(&path).starts_with("local") {
                return Err("trace must stay under local/".into());
            }
            let mut t = std::io::BufWriter::new(std::fs::File::create(path)?);
            writeln!(
                t,
                "tick\tcar\tlaps\tplace\tspeed\ta\tb\tbackwards\ttorque\tgrip_rl\tgrip_fl\tarmed\tactive\tbehaviour"
            )?;
            Some(t)
        }
        Err(_) => None,
    };
    // GRID_SLOW_CAR0=f scales car 0's throttle (slowed-leader overtaking check).
    let slow: f32 = std::env::var("GRID_SLOW_CAR0")
        .ok()
        .map(|v| v.parse())
        .transpose()?
        .unwrap_or(1.);
    let tick_variable = |name: &str| -> Result<Option<u64>, Box<dyn std::error::Error>> {
        Ok(std::env::var(name).ok().map(|v| v.parse()).transpose()?)
    };
    let stop_car0 = tick_variable("GRID_STOP_CAR0")?;
    // GRID_PARKED_CAR=k: car k holds the brake from the start, as the idle
    // player at the back of the grid in a live field capture of the original.
    let parked_car: Option<usize> = std::env::var("GRID_PARKED_CAR")
        .ok()
        .map(|v| v.parse())
        .transpose()?
        .filter(|&k| k < cars);
    // GRID_GRID_HOLD=idle,full (seconds): before the AI drives, every car is
    // held on the handbrake, first at throttle 0, then at full throttle (the
    // start states 1 and 0 of a live field capture of the original).
    let grid_hold: Option<(u64, u64)> = match std::env::var("GRID_GRID_HOLD") {
        Ok(v) => {
            let (idle, full) = v.split_once(',').ok_or("GRID_GRID_HOLD=idle,full")?;
            let ticks = |s: &str| -> Result<u64, Box<dyn std::error::Error>> {
                Ok(
                    (s.parse::<f64>()? / f64::from(grid_tools::source_vehicle::DELTA)).round()
                        as u64,
                )
            };
            Some((ticks(idle)?, ticks(full)?))
        }
        Err(_) => None,
    };
    // GRID_FIELD_TRACE=local/X.tsv: per tick every car's position, speed,
    // controls and behaviour, and the car pairs in contact.
    let mut field_trace = match std::env::var("GRID_FIELD_TRACE") {
        Ok(path) => {
            if !Path::new(&path).starts_with("local") {
                return Err("trace must stay under local/".into());
            }
            let mut t = std::io::BufWriter::new(std::fs::File::create(path)?);
            write!(t, "tick")?;
            for i in 0..cars {
                write!(
                    t,
                    "\tc{i}_x\tc{i}_y\tc{i}_z\tc{i}_v\tc{i}_thr\tc{i}_brk\tc{i}_steer\tc{i}_beh"
                )?;
            }
            writeln!(t, "\tpairs")?;
            Some(t)
        }
        Err(_) => None,
    };
    // GRID_ENGINE_AUDIO_TRACE=local/X.tsv: per tick, car 0's engine-audio
    // inputs as grid-app publishes them after the tick (`Control::vehicle`)
    // and the applied throttle (vehicle +0xE1C), for offline renders.
    let mut engine_trace = match std::env::var("GRID_ENGINE_AUDIO_TRACE") {
        Ok(path) => {
            if !Path::new(&path).starts_with("local") {
                return Err("trace must stay under local/".into());
            }
            let mut t = std::io::BufWriter::new(std::fs::File::create(path)?);
            writeln!(t, "tick	normalized_rpm	throttle	gear	applied_throttle")?;
            Some(t)
        }
        Err(_) => None,
    };
    // GRID_COLLISION_AUDIO_TRACE=local/X.tsv: per tick, what the collision
    // sound listener (`collision_audio`) starts for each car and each car's
    // scrape level, as grid-app would play them (presentation only).
    let mut collision_trace = match std::env::var("GRID_COLLISION_AUDIO_TRACE") {
        Ok(path) => {
            if !Path::new(&path).starts_with("local") {
                return Err("trace must stay under local/".into());
            }
            let bank = grid_tools::collision_audio::Bank::load(root)?;
            let heights = entries
                .iter()
                .map(|(_, m)| grid_tools::collision_audio::Heights::of(root, m))
                .collect::<Result<Vec<_>, _>>()?;
            let mut t = std::io::BufWriter::new(std::fs::File::create(path)?);
            writeln!(t, "tick\tcar\tkind\tvariety\tvolume_db\tpitch\tgroup\tcontacts\tspeed")?;
            Some((t, grid_tools::collision_audio::Listener::new(bank), heights))
        }
        Err(_) => None,
    };
    // GRID_RETIRE_CAR=car:tick: that car's controls fail at that tick, so the
    // shared world retires and holds it (F323) in front of the field.
    let retire_car = match std::env::var("GRID_RETIRE_CAR") {
        Ok(v) => {
            let (car, tick) = v.split_once(':').ok_or("GRID_RETIRE_CAR=car:tick")?;
            Some((car.parse::<usize>()?, tick.parse::<u64>()?))
        }
        Err(_) => None,
    };
    // GRID_PROP_TARGET=TEMPLATE:INDEX (with GRID_DYNAMIC_PROPS): car 0 runs the
    // authored validation drive into that breakable while the field races.
    let mut prop_target = match (std::env::var("GRID_PROP_TARGET"), multi.dynamic_props()) {
        (Ok(target), Some(props)) => Some(grid_tools::dynamic_props::PropPursuit::new(
            root,
            &route,
            &props.catalog,
            &target,
            15.,
        )?),
        (Ok(_), None) => return Err("GRID_PROP_TARGET needs GRID_DYNAMIC_PROPS".into()),
        _ => None,
    };
    // GRID_SPIN_CAR0=tick[:ticks] (default 60 ticks).
    let spin_car0 = match std::env::var("GRID_SPIN_CAR0") {
        Ok(v) => {
            let (start, length) = v.split_once(':').unwrap_or((&v, "60"));
            Some((start.parse::<u64>()?, length.parse::<u64>()?))
        }
        Err(_) => None,
    };
    // GRID_DAMAGE_CAR0=realistic|normal|off: the player damage option
    // (FUN_00654C20) on car 0; AI cars keep the constructor's gain 1.0.
    if let Ok(v) = std::env::var("GRID_DAMAGE_CAR0") {
        let damage =
            grid_tools::source_vehicle::assists::Assists::parse(&format!("damage={v}"))?.damage;
        let owner = &mut multi.cars[0].state.car;
        damage.apply(&mut owner.owner.vehicle);
        damage.apply(&mut owner.frame.body);
        println!("damage car0 option={v} mode={}", damage.mode());
    }
    // GRID_CRASH_CAR0=TICK:IMPULSE:COUNT: before that tick, COUNT frontal hits
    // of solved impulse IMPULSE at the bumper centre (local 0, 0.3, 2.2) go
    // through the original 9E6660 dispatcher with the single-player impact
    // scale 1.0 (global E4FC30), as the body handler 9EF360 would pass them.
    let crash_car0 = match std::env::var("GRID_CRASH_CAR0") {
        Ok(v) => {
            let parts: Vec<&str> = v.split(':').collect();
            if parts.len() != 3 {
                return Err("GRID_CRASH_CAR0 needs TICK:IMPULSE:COUNT".into());
            }
            Some((
                parts[0].parse::<u64>()?,
                parts[1].parse::<f32>()?,
                parts[2].parse::<u32>()?,
            ))
        }
        Err(_) => None,
    };
    // Drift events: drift.xml, the route's close-drift flags and one entry
    // per car (all AI cars at the difficulty's DifficultyBias level); the
    // battle multiplier only in a Drift Battle (0x5F4E60). The virtual race
    // of the route's free-race pool row estimates zero scores at the end.
    let mut drift_event = match discipline {
        Some(format) => {
            let settings = grid_tools::drift::Settings::load(root)?;
            let flags = grid_tools::drift::close_drift_flags(root, &route)?;
            let level = grid_tools::drift::ai_level(difficulty);
            let pool = grid_tools::virtual_race::drift_pool(format, route.path());
            if format == Format::DriftGrandPrix && cars != 1 {
                return Err("drift_grand_prix runs one car on the track".into());
            }
            let rivals = match format {
                Format::DriftGrandPrix => std::env::var("GRID_DRIFT_RIVALS")
                    .ok()
                    .map(|v| v.parse::<usize>())
                    .transpose()?
                    .unwrap_or(pool.map_or(1, |p| p.max_cars as usize - 1)),
                _ => 0,
            };
            let race = grid_tools::virtual_race::VirtualRace::new(
                pool,
                grid_tools::virtual_race::difficulty(2, level),
                0x5eed,
            );
            println!(
                "{} close_drift_flags={} ai_level={level} base_result={:?} rivals={rivals}",
                format.name(),
                flags.len(),
                pool.map(|p| p.base_result)
            );
            let battle = format == Format::DriftBattle;
            let entries = (0..cars + rivals)
                .map(|_| grid_tools::drift::Entry::new(battle, Some(level), false))
                .collect::<Vec<_>>();
            Some((settings, flags, entries, multi.close_drift_flag_boxes(), race))
        }
        None => None,
    };
    // Per car: drift-behaviour ticks, drift-mode ticks, ticks the drift test
    // fired, and the largest slip angle (degrees).
    let mut drift_counts = vec![(0u64, 0u64, 0u64, 0f32); cars];
    // Drift quality (research measure, AUTHORED): spins (body slip above 90°
    // until it falls below 45°), barrier hits (a static or prop contact above
    // collisionThreshold with a mostly horizontal normal, new after 1 s
    // without one) and stalled seconds (below 1 m/s while racing).
    let mut quality = vec![Quality::default(); cars];
    let mut behaviours = vec![(grid_tools::ai_driver::Behaviour::Race, 0u8, 0u8, false); cars];
    let mut pair_contacts = vec![vec![0usize; cars]; cars];
    let mut car_contacts = 0;
    let mut laps: Vec<Vec<f64>> = vec![Vec::new(); cars];
    let mut derby = if demolition {
        let s = grid_tools::demolition::Settings::load(root)?;
        println!("demolition {s:?}");
        Some((s, vec![grid_tools::demolition::NotMoving::default(); cars]))
    } else {
        None
    };
    let trace_laps = std::env::var_os("GRID_TRACE_LAPS").is_some();
    // GRID_SERIAL_TWIN=1 (F568): every tick also runs on a serial clone of
    // the world, and every body and car record must match the parallel tick.
    let serial_twin = std::env::var_os("GRID_SERIAL_TWIN").is_some();
    let mut twin_checked = (0u64, 0usize);
    let mut maximum_speed = 0f32;
    for _ in 0..ticks {
        let positions: Vec<Other> = multi
            .cars
            .iter()
            .enumerate()
            .map(|(i, c)| Other::of(i as u32 + 1, c))
            .collect();
        if drift {
            // The drift test's race place; without a race manager the AI
            // request grips of the last update reach the wheels here, as the
            // grip application 0x408710 does.
            for (i, driver) in drivers.iter_mut().enumerate() {
                if let Driver::Ai(ai) = driver {
                    ai.race = grid_tools::ai_driver::RaceView {
                        place: standings.position(i).saturating_sub(1) as u32,
                        cars: cars as u32,
                        progress: standings.laps_covered(i) as f32,
                    };
                    if field.is_none() {
                        multi.cars[i].set_ai_grip(ai.request_grip());
                    }
                }
            }
        }
        // Race manager before the AI update (0x4095C0).
        if let Some(field) = &mut field {
            if let Some((car, _, wheel)) = blow_tyre.filter(|b| b.1 == multi.ticks) {
                field.blow_tyre(car, wheel);
                println!(
                    "pacing tick={} forced blow_tyre car={car} wheel={wheel}",
                    multi.ticks
                );
            }
            let mut field_cars: Vec<grid_tools::ai_pacing::Car> = multi
                .cars
                .iter()
                .zip(&mut drivers)
                .enumerate()
                .map(|(i, (c, d))| {
                    let p = c.pose();
                    grid_tools::ai_pacing::Car {
                        position: [p[12], p[13], p[14]],
                        velocity: c.velocity(),
                        racing: standings.status(i) == Status::Racing,
                        driver: match d {
                            Driver::Ai(ai) => Some(&mut **ai),
                            Driver::Pursuit(_) => None,
                        },
                    }
                })
                .collect();
            let outputs = field.tick(
                &data.line,
                &mut field_cars,
                grid_tools::source_vehicle::DELTA,
            );
            drop(field_cars);
            for line in &field.log {
                println!("pacing tick={} {line}", multi.ticks);
            }
            for (i, output) in outputs.iter().enumerate() {
                if let Some(o) = output {
                    multi.cars[i].set_ai_pacing(o.torque, o.grip);
                }
            }
            if let Some(t) = &mut pacing_trace {
                let order = standings.order();
                for (i, (o, d)) in outputs.iter().zip(&drivers).enumerate() {
                    if let (Some(o), Driver::Ai(ai)) = (o, d) {
                        let (armed, active) = ai.mistake();
                        let v = multi.cars[i].velocity();
                        writeln!(
                            t,
                            "{}\t{i}\t{:.4}\t{}\t{:.2}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:?}",
                            multi.ticks,
                            standings.laps_covered(i),
                            order.iter().position(|&c| c == i).unwrap_or(0),
                            (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(),
                            ai.pace.a,
                            ai.pace.b,
                            u8::from(ai.pace.backwards),
                            o.torque,
                            o.grip[0],
                            o.grip[2],
                            armed.map_or(-1, i16::from),
                            active.map_or(-1, i16::from),
                            ai.telemetry.behaviour
                        )?;
                    }
                }
            }
        }
        let mut controls: Vec<Controls> = Vec::with_capacity(cars);
        for (i, (car, driver)) in multi.cars.iter().zip(&mut drivers).enumerate() {
            controls.push(match driver {
                Driver::Ai(ai) => {
                    let others: Vec<Other> = positions
                        .iter()
                        .enumerate()
                        .filter(|&(j, _)| j != i)
                        .map(|(_, p)| *p)
                        .collect();
                    let view = View::of(car, multi.obstruction(i));
                    ai.controls(
                        car_data[i],
                        &view,
                        &others,
                        grid_tools::source_vehicle::DELTA,
                    )
                }
                Driver::Pursuit(p) => p.controls(car.pose(), car.velocity()),
            });
        }
        for (car, driver) in multi.cars.iter_mut().zip(&drivers) {
            if let Driver::Ai(ai) = driver {
                car.set_ai_packet(ai.reverse_request());
            }
        }
        controls[0].throttle *= slow;
        if let Some(pursuit) = &mut prop_target {
            let broken_at = multi
                .dynamic_props()
                .and_then(|p| p.events.first())
                .map(|e| e.tick);
            controls[0] = pursuit.controls(
                multi.cars[0].pose(),
                multi.cars[0].velocity(),
                broken_at,
                multi.ticks,
            );
        }
        // GRID_STOP_CAR0=tick: car 0 brakes to a stop and stays there (avoid check).
        if stop_car0.is_some_and(|t| multi.ticks >= t) {
            controls[0] = Controls {
                brake: 1.,
                ..Default::default()
            };
        }
        if let Some((idle, full)) = grid_hold
            && multi.ticks < idle + full
        {
            for c in &mut controls {
                *c = Controls {
                    throttle: if multi.ticks < idle { 0. } else { 1. },
                    handbrake: true,
                    ..Default::default()
                };
            }
        }
        if let Some(k) = parked_car {
            controls[k] = Controls {
                brake: 1.,
                ..Default::default()
            };
        }
        // GRID_SPIN_CAR0: car 0 gets full lock with the handbrake (spin and
        // recovery check).
        if spin_car0.is_some_and(|(t, n)| (t..t + n).contains(&multi.ticks)) {
            controls[0] = Controls {
                throttle: 1.,
                steering: 1.,
                handbrake: true,
                ..Default::default()
            };
        }
        if let Some((car, _)) = retire_car.filter(|&(c, t)| c < cars && multi.ticks + 1 == t) {
            controls[car].throttle = f32::NAN;
        }
        for (i, driver) in drivers.iter().enumerate() {
            if let Driver::Ai(ai) = driver {
                let m = ai.telemetry;
                let now = (m.behaviour, m.overtake_state, m.recovery_mode, m.reverse);
                if now != behaviours[i] {
                    if std::env::var_os("GRID_TRACE_BEHAVIOUR").is_some() {
                        let v = multi.cars[i].velocity();
                        println!(
                            "behaviour tick={} car{i} {:?} state={} recovery={} reverse={} octant={} spun={} on_track={} obstruction={}/{} contact={} avoid_side={} eta={:.2} speed={:.2}",
                            multi.ticks,
                            now.0,
                            now.1,
                            now.2,
                            now.3,
                            m.octant,
                            m.spun,
                            m.on_track,
                            m.obstruction,
                            m.frontal,
                            m.contact,
                            m.avoid_side,
                            m.avoid_eta.min(99.),
                            (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
                        );
                    }
                    behaviours[i] = now;
                }
            }
        }
        if let Some((tick, impulse, count)) = crash_car0
            && multi.ticks == tick
        {
            crash(&mut multi.cars[0], impulse, count)?;
        }
        let mut twin = serial_twin.then(|| {
            let mut twin = multi.clone();
            twin.parallel = false;
            twin
        });
        if let Err(error) = multi.tick(&controls) {
            println!("failure tick={} error={error}", multi.ticks + 1);
            break;
        }
        if let Some(twin) = &mut twin {
            twin.tick(&controls)?;
            let words = |m: &MultiRuntime| -> Vec<u32> {
                let mut all: Vec<u32> = m.bodies().iter().flatten().copied().collect();
                for car in &m.cars {
                    all.extend(&car.state.car.owner.vehicle);
                    all.extend(car.state.car.owner.physical);
                    all.extend(&car.state.car.frame.body);
                }

                all.push(m.dynamic_props().map_or(0, |p| p.events.len()) as u32);
                all
            };
            if words(twin) != words(&multi) {
                return Err(format!("serial twin differs at tick {}", multi.ticks).into());
            }
            twin_checked.0 += 1;
            twin_checked.1 = twin_checked.1.max(multi.bodies().len());
        }
        if let Some((car, _, wheel)) = lose_wheel.filter(|l| l.1 == multi.ticks) {
            let lost = multi.force_wheel_loss(car, wheel)?;
            println!(
                "forced lose_wheel tick={} car={car} wheel={wheel} lost={lost}",
                multi.ticks
            );
        }
        // GRID_HANG_CHECK=N: every N ticks, bodies asleep or parked without
        // support and held cars (MultiRuntime::unsupported, 120 ticks ahead).
        if let Some(every) = std::env::var("GRID_HANG_CHECK").ok().and_then(|v| v.parse::<u64>().ok())
            && every > 0
            && multi.ticks.is_multiple_of(every)
        {
            let bodies = multi.bodies().len();
            let asleep = (multi.bodies().iter())
                .filter(|b| b[0x400 / 4] == grid_world::contact_groups::ASLEEP)
                .count();
            let found = multi.unsupported(120)?;
            println!(
                "hang_check tick={} bodies={bodies} asleep={asleep} unsupported={}",
                multi.ticks,
                found.iter().filter(|u| !u.held).count()
            );
            for u in found {
                println!("hang tick={} {u:?}", multi.ticks);
            }
        }
        // GRID_SINK_TRACE=body:from:to: that body's pose, velocity and contacts each tick.
        if let Some((body, from, to)) = std::env::var("GRID_SINK_TRACE").ok().and_then(|v| {
            let p: Vec<u64> = v.split(':').filter_map(|x| x.parse().ok()).collect();
            (p.len() == 3).then(|| (p[0] as usize, p[1], p[2]))
        }) && (from..=to).contains(&multi.ticks)
            && let Some(b) = multi.bodies().get(body)
        {
            let f = |o: usize| f32::from_bits(b[o / 4]);
            let depth = multi
                .dynamic_props()
                .map(|p| p.road_depths(multi.cars[0].track(), multi.bodies()))
                .and_then(|d| d.into_iter().find(|d| d.body == body))
                .map_or(f32::NAN, |d| d.depth);
            let mut line = format!(
                "trace tick={} body={body} y={:.3} depth={depth:.3} v={:.2},{:.2},{:.2} up={:.2},{:.2},{:.2} state={:x}",
                multi.ticks, f(0x1c4), f(0x220), f(0x224), f(0x228), f(0x1e0), f(0x1e4), f(0x1e8), b[0x400 / 4]
            );
            for (a, o, n, p, pen, force, tokens) in multi.contacts_with(body) {
                line += &format!(
                    " | {a}-{} n={:.2},{:.2},{:.2} y={:.2} pen={pen:.3} f={force:.0} t={}",
                    o.map_or("s".into(), |o| o.to_string()),
                    n[0], n[1], n[2], p[1],
                    if o.is_none() {
                        multi.cars[0].track().shape_name(tokens[1].wrapping_sub(1)).unwrap_or("?").to_string()
                    } else {
                        format!("{}/{}", tokens[0], tokens[1])
                    }
                );
            }
            println!("{line}");
        }
        // GRID_SINK_CHECK=D: every tick, pieces whose hull bottom is more than
        // D m below the road (PropWorld::road_depths).
        if let Some(limit) = std::env::var("GRID_SINK_CHECK").ok().and_then(|v| v.parse::<f32>().ok())
            && let Some(props) = multi.dynamic_props()
        {
            let sunk: Vec<String> = (props.road_depths(multi.cars[0].track(), multi.bodies()).iter())
                .filter(|d| d.depth > limit)
                .map(|d| {
                    format!(
                        "{}:{:.3}{}@{:.2},{:.2},{:.2}",
                        d.body,
                        d.depth,
                        if d.asleep { "z" } else { "" },
                        d.centre[0],
                        d.centre[1],
                        d.centre[2]
                    )
                })
                .collect();
            if !sunk.is_empty() {
                println!("sink tick={} n={} {}", multi.ticks, sunk.len(), sunk.join(" "));
            }
        }
        // GRID_PANEL_CHECK=N: every N ticks, how each detached panel lies.
        if let Some(every) = std::env::var("GRID_PANEL_CHECK")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            && every > 0
            && multi.ticks.is_multiple_of(every)
        {
            for p in multi.panels() {
                println!(
                    "panel tick={} car={} part={} body={} asleep={} parked={} speed={:.3} spin={:.3} face={:.4} low={:.3} fourth={:.3} w={:.3?} size={:.3?}",
                    multi.ticks,
                    p.car,
                    p.part,
                    p.body,
                    p.asleep,
                    p.parked,
                    p.speed,
                    p.spin,
                    p.face,
                    p.low,
                    p.fourth,
                    p.angular,
                    p.size
                );
            }
        }
        if crash_car0.is_some() && multi.ticks.is_multiple_of(500) {
            report_damage(&multi.cars[0], multi.ticks);
        }
        car_contacts += multi.last_car_contacts;
        for &i in multi.newly_retired() {
            standings.retire(i, Out::Retired);
            println!(
                "retire tick={} car{i} reason={}",
                multi.ticks,
                multi.retirement(i).map_or("", |r| r.error.0)
            );
        }
        let pairs = multi.contact_pairs();
        if let Some(t) = &mut engine_trace {
            let car = &multi.cars[0];
            let words = &car.state.car.owner.vehicle;
            let throttle = if car.terminal_wheel().is_some() { 0. } else { controls[0].throttle };
            let c = grid_tools::engine_audio::Control::vehicle(words, throttle, true, 0);
            writeln!(
                t,
                "{}	{}	{}	{}	{}",
                multi.ticks,
                c.normalized_rpm,
                c.throttle,
                c.gear,
                f32::from_bits(words[0xe1c / 4])
            )?;
        }
        if let Some(t) = &mut field_trace {
            write!(t, "{}", multi.ticks)?;
            for (i, (car, c)) in multi.cars.iter().zip(&controls).enumerate() {
                let p = car.pose();
                let v = car.velocity();
                let behaviour = match &drivers[i] {
                    Driver::Ai(ai) if Some(i) != parked_car => {
                        format!("{:?}", ai.telemetry.behaviour)
                    }
                    _ => "-".into(),
                };
                write!(
                    t,
                    "\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.3}\t{behaviour}",
                    p[12],
                    p[13],
                    p[14],
                    (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(),
                    c.throttle,
                    c.brake,
                    c.steering
                )?;
            }
            writeln!(t, "\t{pairs:?}")?;
        }
        if let Some((t, listener, heights)) = &mut collision_trace {
            listener.tick(grid_tools::source_vehicle::DELTA);
            for (k, runtime) in multi.cars.iter().enumerate() {
                let impacts = multi.contact_impacts(k);
                let solved: Vec<_> = impacts.iter().map(|(x, _)| *x).collect();
                let car = grid_tools::collision_audio::Car::of(runtime, heights[k], k == 0, &solved);
                let speed = car.velocity.iter().map(|v| v * v).sum::<f32>().sqrt();
                let mut plays = Vec::new();
                if !impacts.is_empty() {
                    let contacts = grid_tools::collision_audio::contacts_of(runtime, &car, &impacts);
                    listener.contacts(k, &car, &contacts, &mut plays);
                }
                let airborne = grid_tools::collision_audio::airborne(runtime);
                listener.landing(k, &car, airborne, grid_tools::source_vehicle::DELTA, &mut || false, &mut plays);
                if k == 0 {
                    listener.displacement(&car, grid_tools::collision_audio::angular_velocity(runtime), &mut plays);
                }
                for p in plays {
                    writeln!(
                        t,
                        "{}\t{k}\tshot\t{}\t{:.2}\t{:.3}\t{}\t{}\t{speed:.2}",
                        multi.ticks,
                        p.variety,
                        p.volume_db,
                        p.pitch,
                        p.group,
                        impacts.len()
                    )?;
                }
                if let Some(v) = listener.scrape_frame(k, &car)
                    && v.volume_db > -48.
                {
                    writeln!(
                        t,
                        "{}\t{k}\tscrape\t{}\t{:.2}\t{:.3}\t{}\t{}\t{speed:.2}",
                        multi.ticks,
                        v.variety,
                        v.volume_db,
                        v.pitch,
                        v.group,
                        impacts.len()
                    )?;
                }
            }
        }
        if !pairs.is_empty() && std::env::var_os("GRID_TRACE_BEHAVIOUR").is_some() {
            println!("contact tick={} pairs={pairs:?}", multi.ticks);
        }
        for c in pairs {
            pair_contacts[c.0][c.1] += 1;
        }
        standings.update(
            &data.line,
            &positions_of(&multi),
            multi.ticks as f64 * f64::from(grid_tools::source_vehicle::DELTA),
            &[],
        );
        if let Some((settings, not_moving)) = &mut derby {
            // Every car here is AI: a stopped car is disqualified (0x630CF0).
            for (i, n) in not_moving.iter_mut().enumerate() {
                if standings.status(i) != Status::Racing {
                    continue;
                }
                let v = multi.cars[i].velocity();
                let speed = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                if n.tick(settings, speed, f64::from(grid_tools::source_vehicle::DELTA))
                    && multi.cars[i].disqualify()
                {
                    standings.retire(i, Out::Disqualified);
                    let p = multi.cars[i].pose();
                    let behaviour = match &drivers[i] {
                        Driver::Ai(ai) => format!("{:?}", ai.telemetry.behaviour),
                        Driver::Pursuit(_) => "pursuit".into(),
                    };
                    println!(
                        "disqualified tick={} car{i} reason=not-moving pos=({:.1},{:.1},{:.1}) behaviour={behaviour} up_y={:.2}",
                        multi.ticks, p[12], p[13], p[14], p[5]
                    );
                }
            }
        }
        if let Some((settings, flags, entries, flag_boxes, race)) = &mut drift_event {
            for (i, car) in multi.cars.iter().enumerate() {
                let contacts: Vec<_> = multi
                    .contact_impacts(i)
                    .iter()
                    .map(|(x, other)| (x.impact, *other))
                    .collect();
                let (impact, touched) = grid_tools::drift::split_contacts(&contacts, flag_boxes);
                let touched = touched || multi.touched_post(i);
                if std::env::var_os("GRID_TRACE_DRIFT_CONTACTS").is_some() {
                    for (x, prop) in multi.contact_impacts(i) {
                        if x.impact > 100. {
                            println!(
                                "drift_contact tick={} car{i} impact={:.0} point={:?} normal={:?} prop={prop:?}",
                                multi.ticks, x.impact, x.point, x.normal
                            );
                        }
                    }
                }
                let sample = grid_tools::drift::Sample::of(
                    car,
                    standings.laps_covered(i) as f32,
                    standings.position(i).saturating_sub(1),
                    cars,
                    impact,
                );
                if touched {
                    entries[i].detector.touch_flag();
                }
                entries[i].update(settings, flags, &sample, grid_tools::source_vehicle::DELTA);
                if standings.status(i) == Status::Racing && standings.laps_covered(i) > 0. {
                    let barrier = multi.contact_impacts(i).iter().any(|(x, other)| {
                        !matches!(
                            other,
                            grid_tools::source_vehicle::multi::Contacted::Car(_)
                                | grid_tools::source_vehicle::multi::Contacted::Track
                        )
                            && x.impact > settings.collision_threshold
                            && x.normal[1].abs() < 0.7
                    });
                    quality[i].tick(
                        entries[i].detector.angle,
                        entries[i].detector.speed,
                        barrier,
                        multi.ticks,
                    );
                }
                if matches!(standings.status(i), Status::Finished | Status::Out(_)) {
                    let v = car.velocity();
                    let speed = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                    entries[i].finish_race(
                        settings,
                        (true, speed, race_laps),
                        race,
                        characteristic(i, "virtual_performance"),
                    );
                }
                if let Driver::Ai(ai) = &drivers[i] {
                    let m = ai.telemetry;
                    let c = &mut drift_counts[i];
                    c.0 += u64::from(m.behaviour == grid_tools::ai_driver::Behaviour::Drift);
                    c.1 += u64::from(m.drift_mode);
                    c.2 += u64::from(m.drift_fired);
                    c.3 = c.3.max(m.slip.to_degrees());
                }
            }
        }
        // A car stopped by the failure/damage retire drive is out (AUTHORED).
        for (i, driver) in drivers.iter().enumerate() {
            if let Driver::Ai(ai) = driver
                && ai.removal_requested()
                && standings.status(i) == Status::Racing
            {
                standings.retire(i, Out::Wrecked);
                println!("retire tick={} car{i} reason=ai-failure", multi.ticks);
            }
        }
        // Terminal damage takes a car out of the race (AUTHORED for AI cars).
        for (i, car) in multi.cars.iter().enumerate() {
            if let Some(wheel) = car.terminal_wheel()
                && standings.status(i) == Status::Racing
            {
                standings.retire(i, Out::Wrecked);
                // A lost wheel (status 4) names the wheel; the earlier wheel
                // damage terminal (status 3) keeps its line unchanged.
                if car.lost_wheel() {
                    println!(
                        "retire tick={} car{i} reason=lost-wheel wheel={wheel}",
                        multi.ticks
                    );
                } else {
                    println!("retire tick={} car{i} reason=terminal-damage", multi.ticks);
                }
            }
        }
        if multi.ticks.is_multiple_of(1500) {
            let order: Vec<usize> = standings.order();
            println!("standings tick={} order={order:?}", multi.ticks);
        }
        let v = multi.cars[0].velocity();
        let speed = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        maximum_speed = maximum_speed.max(speed);
        if let (Some(t), Driver::Ai(ai)) = (&mut trace, &drivers[0]) {
            let c = controls[0];
            let m = ai.telemetry;
            writeln!(
                t,
                "{}\t{speed:.3}\t{:.3}\t{:.3}\t{:.3}\t{:.2}\t{:.2}\t{:.5}\t{:.4}\t{:.2}\t{:.2}\t{:.2}\t{:.2}\t{:.3}\t{}\t{:?}",
                multi.ticks,
                c.throttle,
                c.brake,
                c.steering,
                m.arc,
                m.look_ahead,
                m.curvature,
                m.heading_error,
                m.line_distance,
                m.line_target,
                m.offset,
                m.line_error,
                m.slip,
                u8::from(multi.cars[0].reverse_gear()),
                m.behaviour
            )?;
            if drift {
                // Drift runs append: test fired, state, drift mode, drift
                // line, request grips rear/front, scorer total and display.
                let g = ai.request_grip();
                let s = drift_event.as_ref().map(|e| &e.2[0].scorer);
                writeln!(
                    t,
                    "#drift\t{}\t{}\t{}\t{}\t{}\t{:.2}\t{:.2}\t{:.0}\t{:.0}\t{:.0}\t{}\t{:.2}\t{:.2}",
                    multi.ticks,
                    u8::from(m.drift_fired),
                    m.drift_state,
                    u8::from(m.drift_mode),
                    u8::from(m.drift_line),
                    g[0],
                    g[2],
                    s.map_or(0., |s| s.total),
                    s.map_or(0., |s| s.display),
                    multi.impacts(0).iter().map(|x| x.impact).fold(0., f32::max),
                    drift_event.as_ref().map_or(0, |e| u8::from(e.2[0].detector.drifting)),
                    m.drift_room[0],
                    m.drift_room[1],
                )?;
            }
        }
        for (i, car) in multi.cars.iter().enumerate() {
            use grid_tools::lap_progress::Event;
            for event in &car.lap_events {
                // GRID_TRACE_LAPS=1: every lap-timing event per car.
                if trace_laps {
                    println!("lap_event car{i} tick={} {event:?}", multi.ticks);
                }
                if let Event::Finished(completed) = event {
                    laps[i].push(completed.seconds);
                }
            }
        }
        if standings.over() {
            break;
        }
    }
    if !standings.over() {
        // Tick limit: end the race as the original's finalisation does
        // (no human car here, so stopped cars are retired).
        let speeds: Vec<f64> = multi
            .cars
            .iter()
            .map(|c| {
                let v = c.velocity();
                f64::from((v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt())
            })
            .collect();
        standings.finalize(
            &speeds,
            multi.ticks as f64 * f64::from(grid_tools::source_vehicle::DELTA),
            &[],
        );
        println!("race finalized tick={} reason=tick-limit", multi.ticks);
    }
    if let Some((settings, _, entries, _, race)) = &mut drift_event {
        // The race end banks the rest (0x64D940); virtual rivals never
        // move, so they are estimated.
        for (i, entry) in entries.iter_mut().enumerate() {
            let (finished, speed) = match multi.cars.get(i) {
                Some(car) => {
                    let v = car.velocity();
                    (
                        matches!(standings.status(i), Status::Finished | Status::Out(_)),
                        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(),
                    )
                }
                None => (false, 0.),
            };
            entry.finish_race(
                settings,
                (finished, speed, race_laps),
                race,
                characteristic(i, "virtual_performance"),
            );
            let s = &entry.scorer;
            let (behaviour, mode, fired, slip) = drift_counts.get(i).copied().unwrap_or_default();
            if let Some(q) = quality.get(i) {
                println!(
                    "drift_quality car{i} laps_covered={:.3} spins={} barrier_hits={} stalled_s={:.1}",
                    standings.laps_covered(i),
                    q.spins,
                    q.hits,
                    q.stalled as f32 * grid_tools::source_vehicle::DELTA
                );
            }
            println!(
                "drift car{i} score={} estimated={} drifts={} longest={:.2} largest_angle={:.1} fastest_mph={:.1} best_combo={:.0} ai_drift_ticks={behaviour} drift_mode_ticks={mode} test_fired_ticks={fired} max_slip={slip:.1}",
                entry.score(),
                entry.estimated,
                s.ratings.iter().map(|r| r.count).sum::<i32>(),
                s.longest_drift,
                s.largest_angle,
                s.fastest_drift,
                s.best_combo,
            );
        }
        let mut race_order = standings.order();
        race_order.extend(multi.cars.len()..entries.len());
        let order = grid_tools::drift::classification(entries, &race_order);
        println!("drift_results order={order:?}");
    }
    // On stderr: the identity check hashes standard output.
    if serial_twin {
        eprintln!(
            "serial_twin ticks={} max_bodies={} threads={}",
            twin_checked.0,
            twin_checked.1,
            grid_tools::parallel::threads()
        );
    }
    for (i, car) in multi.cars.iter().enumerate() {
        let lap = car.lap.as_ref().unwrap();
        let p = Runtime::pose(car);
        println!(
            "car{i} valid_laps={} attempts={} laps_covered={:.3} lap_times={:?} pos=({:.1},{:.1}) terminal={:?}",
            lap.valid_laps,
            lap.attempts,
            standings.laps_covered(i),
            laps[i],
            p[12],
            p[14],
            car.terminal_wheel()
        );
    }
    // Final per-car state digest (FNV-1a over vehicle and physical words) for
    // bit-identity checks.
    for (i, car) in multi.cars.iter().enumerate() {
        let owner = &car.state.car.owner;
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for &word in owner.vehicle.iter().chain(&owner.physical) {
            hash = (hash ^ u64::from(word)).wrapping_mul(0x100_0000_01b3);
        }
        let deformation = car
            .deformation_registration
            .as_ref()
            .map(|r| (r.bulk_calls, r.records));
        println!("state car{i} hash={hash:016x} deformation={deformation:?}");
    }
    for (i, row) in pair_contacts.iter().enumerate() {
        if row.iter().any(|&n| n > 0) {
            println!("contacts car{i} with {row:?}");
        }
    }
    for line in standings.classification(&names) {
        println!("classification {line}");
    }
    if let Some(props) = multi.dynamic_props() {
        for (car, wheel, start, now) in props.wheel_poses(multi.bodies()) {
            let moved: f32 = (0..3)
                .map(|j| (now[3][j] - start[3][j]).powi(2))
                .sum::<f32>()
                .sqrt();
            println!(
                "wheel_body car={car} wheel={wheel} at=({:.2},{:.2},{:.2}) moved={moved:.2}",
                now[3][0], now[3][1], now[3][2]
            );
        }
    }
    println!(
        "race ticks={} car_contacts={car_contacts} car0_max_speed={maximum_speed:.2}",
        multi.ticks
    );
    if let Some(props) = multi.dynamic_props() {
        println!(
            "dynamic_props breaks={} pieces={} events={:?}",
            props.events.len(),
            props.pieces.len(),
            props.events
        );
    }
    Ok(())
}

/// Mutable component levels and effects of one car (MECHANICAL_DAMAGE.md).
fn report_damage(car: &Runtime, tick: u64) {
    let b = &car.state.car.owner.vehicle;
    let m = |o: usize| f32::from_bits(b[(0xcdf0 + o) / 4]);
    let v = car.velocity();
    println!(
        "damage tick={tick} speed={:.3} radiator={:.4} engine={:.4} gearbox={:.4} suspension={:?} wheels={:?} power={:.4} cutout={:.4} gearbox_effect={:.4}",
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(),
        m(8),
        m(0xc),
        m(0x14),
        (0..4).map(|i| m(0x28 + 4 * i)).collect::<Vec<_>>(),
        (0..4).map(|i| m(0x18 + 4 * i)).collect::<Vec<_>>(),
        m(0x3a4),
        m(0x3b8),
        m(0x3c0)
    );
}

fn crash(car: &mut Runtime, impulse: f32, count: u32) -> Result<(), Box<dyn std::error::Error>> {
    let p = car.pose();
    let local = [0f32, 0.3, 2.2];
    let point = std::array::from_fn(|j| {
        if j == 3 {
            1.
        } else {
            p[12 + j] + local[0] * p[j] + local[1] * p[4 + j] + local[2] * p[8 + j]
        }
    });
    for _ in 0..count {
        let owner = &mut car.state.car.owner;
        let random = owner
            .damage_random_state
            .ok_or("crash needs the owned damage RNG")?;
        let out = grid_world::collision_damage::calculate(owner, point, impulse, false, random)?;
        *owner = out.state;
        owner.damage_random_state = Some(out.random_state);
        println!(
            "crash car0 applied={} rng={:08x}",
            out.applied, out.random_state
        );
    }
    report_damage(car, car.ticks);
    Ok(())
}

fn main() {
    let result = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| run().map_err(|e| e.to_string()))
        .expect("race worker")
        .join()
        .expect("race worker panic");
    if grid_tools::perf::enabled() {
        eprintln!("{}", grid_tools::perf::report());
    }
    if let Err(error) = result {
        eprintln!("grid-opponent-race: {error}");
        std::process::exit(1);
    }
}

/// Drift quality counters of one car (see `quality` in `main`).
#[derive(Clone, Copy, Default)]
struct Quality {
    spins: u32,
    spinning: bool,
    hits: u32,
    last_hit: Option<u64>,
    stalled: u64,
}

impl Quality {
    fn tick(&mut self, angle: f32, speed: f32, barrier: bool, tick: u64) {
        if !self.spinning && angle > 90. {
            self.spinning = true;
            self.spins += 1;
        } else if self.spinning && angle < 45. {
            self.spinning = false;
        }
        if barrier {
            if self.last_hit.is_none_or(|t| tick > t + 100) {
                self.hits += 1;
            }
            self.last_hit = Some(tick);
        }
        if speed < 1. {
            self.stalled += 1;
        }
    }
}
