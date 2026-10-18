//! Headless Pro Touge between two AI cars (docs/TOUGE.md §7): both runs, the
//! penalties and the result.
//!
//! grid-pro-touge ROOT CD01 BP01 [ROUTE] [CARS] [TICKS]
//! GRID_DEFORMATION_PARAMETERS=DF01 and GRID_AI_CHARACTERISTICS=TSV are used when set.
use grid_tools::{car::Car, route::Route, touge};
use std::path::Path;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !(3..=6).contains(&args.len()) {
        return Err("usage: grid-pro-touge ROOT CD01 BP01 [ROUTE] [CARS a,b] [TICKS]".into());
    }
    let root = Path::new(&args[0]);
    let table = std::fs::read(&args[1])?;
    let events = Some(grid_tools::body_impact_parameters::read(&std::fs::read(&args[2])?)?);
    let route = Route::new(args.get(3).map_or(grid_tools::route::HARUNA, String::as_str))?;
    let models = Car::list(args.get(4).map_or("lac", String::as_str))?;
    let cars = [&models[0], &models[1 % models.len()]];
    let ticks = args.get(5).map_or(Ok(30000), |t| t.parse())?;
    let deformation = match std::env::var_os("GRID_DEFORMATION_PARAMETERS") {
        Some(p) => Some(grid_tools::deformation_parameters::read(&std::fs::read(p)?)?),
        None => None,
    };
    let characteristics = match std::env::var_os("GRID_AI_CHARACTERISTICS") {
        Some(p) => grid_tools::ai_driver::read_characteristics(&std::fs::read_to_string(p)?)?,
        None => Vec::new(),
    };
    let event = touge::Event {
        root,
        route: &route,
        cars,
        table: &table,
        events,
        deformation,
        characteristics: &characteristics,
        ticks,
    };
    let (runs, penalties, standings) = touge::run_pro_touge(&event)?;
    for (r, run) in runs.iter().enumerate() {
        println!(
            "run{} ticks={} contact_ticks={} times={:?}",
            r + 1,
            run.ticks,
            run.contacts,
            run.times
        );
    }
    for (i, p) in penalties.iter().enumerate() {
        println!("penalties car{i} run1(minor,major)={:?} run2(minor,major)={:?}", p.counts[0], p.counts[1]);
    }
    for (place, s) in standings.iter().enumerate() {
        let f = |t: Option<f64>| t.map_or_else(|| "DNF".into(), grid_tools::race::format_time);
        println!(
            "result {}. car{} round1={} (+{:.0}) round2={} (+{:.0}) total={}",
            place + 1,
            s.car,
            f(s.run_total(0)),
            s.penalties[0],
            f(s.run_total(1)),
            s.penalties[1],
            f(s.total())
        );
    }
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("grid-pro-touge: {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
