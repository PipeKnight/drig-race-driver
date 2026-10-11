//! Private supported body/wheel prelude report for original-reference comparisons.
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let tyre = args.get(2).is_some_and(|v| v == "tyre");
    let gears = args.get(2).is_some_and(|v| v == "gears");
    if args.len() != 2 && !(args.len() == 3 && (tyre || gears)) {
        return Err("usage: grid-body-tuning INPUT local/OUTPUT.tsv [tyre|gears]".into());
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
        .map(grid_formats::vehicle::read_standard_body)
        .collect::<Result<_, _>>()?;
    let selection = if tyre {
        table
            .rows
            .iter()
            .map(grid_formats::vehicle::read_standard_coupled_tyres)
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    let ratios = if gears {
        table
            .rows
            .iter()
            .map(grid_formats::vehicle::read_standard_gear_ratios)
            .collect::<Result<Vec<_>, _>>()?
    } else {
        Vec::new()
    };
    let mut writer = std::io::BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(output.file_name().ok_or("missing filename")?))?,
    );
    for (ordinal, p) in parameters.iter().enumerate() {
        if gears {
            write!(writer, "G\t{ordinal}")?;
            for ratio in ratios[ordinal] {
                write!(writer, "\t{:08x}", ratio.to_bits())?;
            }
            writeln!(writer)?;
            continue;
        }
        if tyre {
            writeln!(writer, "T\t{ordinal}\t{}", u8::from(selection[ordinal]))?;
            continue;
        }
        writeln!(
            writer,
            "B\t{ordinal}\t{:08x}\t{:08x}\t{:08x}\t{:08x}\t{:08x}",
            p.mass.to_bits(),
            p.inverse_mass.to_bits(),
            p.inertia_components[0].to_bits(),
            p.inertia_components[1].to_bits(),
            p.inertia_components[2].to_bits()
        )?;
        for (group, values) in p.wheel_groups.iter().enumerate() {
            write!(
                writer,
                "W\t{ordinal}\t{group}\t{:08x}",
                values.clearance_operand.to_bits()
            )?;
            for value in values.direct_scalars {
                write!(writer, "\t{:08x}", value.to_bits())?;
            }
            writeln!(writer)?;
        }
    }
    writer.flush()?;
    println!("body_parameter_rows={}", parameters.len());
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-body-tuning: {error}");
        std::process::exit(1);
    }
}
