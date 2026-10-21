//! Private survey for the circuit series (docs/SERIES.md): every free-race
//! discipline of `database.bin` with its cars (raceable or not) and track
//! pool layouts (supported by the port or not).
//!
//! `grid-series-survey ROOT`
use grid_tools::series::{RACEABLE, Series};
use std::path::Path;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::args().nth(1).ok_or("usage: grid-series-survey ROOT")?;
    let root = Path::new(&root);
    let db = grid_formats::database::read(
        &std::fs::read(root.join("database/schema.bin"))?,
        &std::fs::read(root.join("database/database.bin"))?,
    )?;
    let names: Vec<String> = db
        .table("free_race_discipline")
        .ok_or("no disciplines")?
        .rows()
        .filter_map(|r| r.text("name_string_id").map(str::to_owned))
        .collect();
    for name in names {
        let Ok(series) = Series::from_database(&db, &name) else {
            println!("{name}: not a circuit series");
            continue;
        };
        let cars: Vec<String> = series
            .cars
            .iter()
            .map(|c| {
                let s = c.stem();
                if RACEABLE.contains(&s) { s.to_owned() } else { format!("{s}*") }
            })
            .collect();
        let supported: Vec<&str> = series.supported_layouts().map(|l| l.name.as_str()).collect();
        let other: Vec<String> = series
            .layouts
            .iter()
            .filter(|l| !supported.contains(&l.name.as_str()))
            .map(|l| format!("{}({})", l.name, l.route))
            .collect();
        println!(
            "{name}: cars=[{}] supported={} [{}] unsupported={} [{}]",
            cars.join(","),
            supported.len(),
            supported.join(","),
            other.len(),
            other.join(",")
        );
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("grid-series-survey: {error}");
        std::process::exit(1);
    }
}
