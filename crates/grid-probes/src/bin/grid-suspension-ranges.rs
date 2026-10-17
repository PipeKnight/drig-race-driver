//! Private initial Standard suspension report, distinct from runtime preparation.
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-suspension-ranges INPUT local/OUTPUT.tsv".into());
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
        .map(grid_formats::vehicle::read_standard_suspension_setup)
        .collect::<Result<_, _>>()?;
    let mut writer = std::io::BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(output.file_name().ok_or("missing filename")?))?,
    );
    for (ordinal, p) in parameters.iter().enumerate() {
        write!(writer, "B\t{ordinal}\t{}", u32::from(p.explicit_ranges))?;
        for value in [
            p.height,
            p.base_damper[0],
            p.base_damper[1],
            p.base_threshold[0],
            p.base_threshold[1],
            p.base_high_ratio[0],
            p.base_high_ratio[1],
        ] {
            write!(writer, "\t{:08x}", value.to_bits())?;
        }
        writeln!(writer)?;
        for index in 0..30 {
            if let (Some([low, high]), Some(slider)) = (p.ranges[index], p.sliders[index]) {
                writeln!(
                    writer,
                    "R\t{ordinal}\t{index}\t{:08x}\t{:08x}\t{:08x}",
                    low.to_bits(),
                    high.to_bits(),
                    slider.to_bits()
                )?;
            }
        }
    }
    writer.flush()?;
    println!("suspension_parameter_rows={}", parameters.len());
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-suspension-ranges: {error}");
        std::process::exit(1);
    }
}
