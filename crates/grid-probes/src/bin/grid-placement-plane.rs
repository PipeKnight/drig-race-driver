//! Controlled analytic-plane reports for conditional original placement comparison.
use grid_world::spawn::{self, Normalization, Wheel};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err(
            "usage: grid-placement-plane INPUT.tsv local/OUTPUT.tsv direct|fraction|reference"
                .into(),
        );
    }
    let (fractional, normalization) = match args[2].to_str() {
        Some("direct") => (false, Normalization::Standard),
        Some("fraction") => (true, Normalization::Standard),
        Some("reference") => (true, Normalization::ReferenceSse),
        _ => return Err("height mode must be direct, fraction or reference".into()),
    };
    let output = Path::new(&args[1]);
    let parent = output
        .parent()
        .ok_or("missing output parent")?
        .canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("report must remain private".into());
    }
    let mut input = String::new();
    File::open(&args[0])?
        .take(1024 * 1024 + 1)
        .read_to_string(&mut input)?;
    if input.len() > 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let mut rows = Vec::new();
    for line in input.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() != 30 || fields[0].len() > 63 {
            return Err("case requires name and 29 numbers".into());
        }
        let values: Vec<f32> = fields[1..]
            .iter()
            .map(|v| v.parse())
            .collect::<Result<_, _>>()?;
        if values.iter().any(|v| !v.is_finite()) {
            return Err("nonfinite plane case input".into());
        }
        let wheels = std::array::from_fn(|i| Wheel {
            position: values[6 + i * 5..9 + i * 5].try_into().unwrap(),
            radius: values[9 + i * 5],
            vertical_displacement: values[10 + i * 5],
        });
        let mut calls = 0;
        let pose = spawn::align_with_normalization(
            values[..3].try_into()?,
            values[3..6].try_into()?,
            wheels,
            normalization,
            |x, z, upper, lower| {
                calls += 1;
                let height = (values[26] * x + values[27] * z) + values[28];
                if height > upper || height < lower {
                    return Ok(None);
                }
                let height = if fractional {
                    let fraction = (height - upper) / (lower - upper);
                    upper - (upper - lower) * fraction
                } else {
                    height
                };
                Ok(Some(height))
            },
        )?;
        rows.push((fields[0], pose, calls));
    }
    let mut writer = std::io::BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(output.file_name().ok_or("missing output name")?))?,
    );
    for (name, pose, calls) in &rows {
        write!(writer, "P\t{name}")?;
        for value in pose.matrix() {
            write!(writer, "\t{:08x}", value.to_bits())?;
        }
        writeln!(writer, "\t{calls}")?;
    }
    writer.flush()?;
    println!("plane_cases={} fractional_height={fractional}", rows.len());
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-placement-plane: {error}");
        std::process::exit(1);
    }
}
