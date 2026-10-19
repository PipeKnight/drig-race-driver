//! Private probe for the default-setup comparison (docs/SERIES.md "The default
//! car setup's downforce limit"): writes the port's car words (owner vehicle,
//! little-endian u32, from +0x0) right after `Runtime::load`, before any tick,
//! to diff against a read-only dump of the original's car object.
//!
//! `grid-car-words ROOT CAR SLOT local/OUT.bin` (CAR e.g. `lac`, `lc7`);
//! `GRID_CAR_TICKS=N` ticks the car at rest first.
use grid_tools::{car::Car, route::Route, source_vehicle::Runtime};
use std::path::Path;

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 4 || !Path::new(&args[3]).starts_with("local") {
        return Err("usage: grid-car-words ROOT CAR SLOT local/OUT.bin".into());
    }
    let mut r = Runtime::load(Path::new(&args[0]), &Route::default(), &Car::new(&args[1])?, &args[2], 200679)?;
    // GRID_CAR_TICKS: ticks at rest (no input) before the words are written.
    let ticks: u32 = std::env::var("GRID_CAR_TICKS").ok().and_then(|t| t.parse().ok()).unwrap_or(0);
    for _ in 0..ticks {
        r.tick(Default::default())?;
    }
    let words = &r.state.car.owner.vehicle;
    let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
    std::fs::write(&args[3], &bytes)?;
    println!("car_words car={} slot={} bytes={:#x}", args[1], args[2], bytes.len());
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("grid-car-words: {error}");
        std::process::exit(1);
    }
}
