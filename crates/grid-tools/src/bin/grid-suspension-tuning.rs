//! Private initial Standard suspension report, distinct from runtime preparation.
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-suspension-tuning INPUT local/OUTPUT.tsv".into());
    }
    let output = Path::new(&args[1]);
    let parent = output.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("report must remain private".into());
    }
    let mut bytes = Vec::new();
    File::open(&args[0])?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    let table = grid_formats::tuning::read(&bytes)?;
    let parameters: Vec<_> = table
        .rows
        .iter()
        .map(grid_formats::vehicle::read_standard_suspension)
        .collect::<Result<_, _>>()?;
    let mut writer = std::io::BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(output.file_name().ok_or("missing filename")?))?,
    );
    for (ordinal, groups) in parameters.iter().enumerate() {
        for (group, p) in groups.iter().enumerate() {
            write!(writer, "S\t{ordinal}\t{group}")?;
            for value in [
                p.spring,
                p.damper,
                p.axle_coupling,
                p.bump,
                p.speed_threshold,
                p.high_speed_ratio,
                p.negative_speed_scale,
                p.travel_denominator,
                p.preload,
                p.lower_stop,
                p.upper_stop,
            ] {
                write!(writer, "\t{:08x}", value.to_bits())?;
            }
            writeln!(writer)?;
        }
    }
    writer.flush()?;
    println!("suspension_parameter_rows={}", parameters.len());
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-suspension-tuning: {error}");
        std::process::exit(1);
    }
}
