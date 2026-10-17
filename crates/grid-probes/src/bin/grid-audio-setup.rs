//! Selected purchased car audio metadata, written only to a new private report.
use std::{fs, io::Read, io::Write, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: grid-audio-setup INPUT.bin CAR local/NEW.tsv".into());
    }
    let out = Path::new(&args[2]);
    if out.exists()
        || !out
            .parent()
            .ok_or("output parent")?
            .canonicalize()?
            .starts_with(Path::new("local").canonicalize()?)
    {
        return Err("new private output required".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(&args[0])?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    let setup =
        grid_formats::audio_car_setup::read(&bytes, args[1].to_str().ok_or("car name UTF-8")?)?;
    let mut report = String::new();
    use std::fmt::Write as _;
    for (name, value) in [
        ("engine_ini", setup.engine_ini),
        ("gear_ini", setup.gear_ini),
        ("engine_bank", setup.engine_bank),
        ("mechanics_bank", setup.mechanics_bank),
    ] {
        writeln!(report, "{name}\t{value}")?;
    }
    writeln!(
        report,
        "throttle_frames\t{}\nidle_frames\t{}\nopen_cockpit\t{}",
        setup.throttle_frames, setup.idle_frames, setup.open_cockpit
    )?;
    for (name, value) in ["gear_trim", "engine_trim", "exhaust_trim"]
        .into_iter()
        .zip(setup.trims)
    {
        writeln!(report, "{name}\t{:08x}", value.to_bits())?;
    }
    for (name, point) in ["engine_bay", "exhaust", "gearbox", "cockpit"]
        .into_iter()
        .zip(setup.offsets)
    {
        writeln!(
            report,
            "{name}\t{:08x}\t{:08x}\t{:08x}",
            point[0].to_bits(),
            point[1].to_bits(),
            point[2].to_bits()
        )?;
    }
    fs::File::create_new(out)?.write_all(report.as_bytes())?;
    println!("audio_car_setup_selected=1 typed_fields=14");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("audio_car_setup_error={error}");
        std::process::exit(1);
    }
}
