//! Private owned Standard constructor comparison.
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 4 {
        return Err("usage: grid-standard-car CAR.csv CAR.ned RNG_HEX local/OUTPUT.bin".into());
    }
    let random_state = u32::from_str_radix(args[2].to_str().ok_or("invalid RNG text")?, 16)?;
    let output = Path::new(&args[3]);
    let parent = output.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?)
        || fs::metadata(&args[0])?.len() > 4 * 1024 * 1024
        || fs::metadata(&args[1])?.len() > 16 * 1024 * 1024
    {
        return Err("bounded input/private output required".into());
    }
    let csv = fs::read(&args[0])?;
    let rows = grid_formats::tuning::read(&csv)?;
    if rows.rows.len() != 1 {
        return Err("one source row required".into());
    }
    let properties = grid_formats::vehicle_properties::read(&rows.rows[0], 0x2008)?;
    let ned = fs::read(&args[1])?;
    let document = grid_formats::binxml::read(&ned)?;
    let geometry = grid_formats::vehicle::read_wheel_geometry(&document)?;
    let wheels = document
        .elements
        .iter()
        .find(|e| e.name == "WHEELS")
        .ok_or("missing wheels")?;
    let mut widths = [0.; 4];
    for (i, name) in [
        "m_rear_l_width",
        "m_rear_r_width",
        "m_front_l_width",
        "m_front_r_width",
    ]
    .iter()
    .enumerate()
    {
        widths[i] = wheels
            .attributes
            .iter()
            .find(|(key, _)| key == name)
            .ok_or("missing wheel width")?
            .1
            .parse::<f64>()? as f32;
    }
    let car = grid_tools::standard_car::initialize(
        &properties,
        geometry,
        widths,
        f32::from_bits(0x411c_f5c3),
        random_state,
    )?;
    let mut writer = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(output.file_name().ok_or("missing filename")?))?;
    writer.write_all(car.bytes())?;
    println!(
        "constructor_bytes={} driven={:?} random_state={:08x}",
        car.bytes().len(),
        car.driven,
        car.random_state
    );
    Ok(())
}
fn main() {
    let result = std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(run)
        .expect("constructor worker")
        .join()
        .expect("constructor worker panic");
    if let Err(error) = result {
        eprintln!("grid-standard-car: {error}");
        std::process::exit(1);
    }
}
