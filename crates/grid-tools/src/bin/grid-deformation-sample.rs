//! Private supplied deformation descriptor diagnostic.
use grid_world::deformation_grid::Grid;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-deformation-sample INPUT.tsv local/OUTPUT.tsv".into());
    }
    let text = fs::read_to_string(&args[0])?;
    if text.len() > 16 * 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let mut results = Vec::new();
    for line in text.lines() {
        let cells: Vec<_> = line.split('\t').collect();
        if cells.len() < 13 || results.len() >= 1024 || cells[0].is_empty() || cells[0].len() > 63 {
            return Err("invalid input row".into());
        }
        let v: Vec<_> = cells[1..]
            .iter()
            .map(|s| u32::from_str_radix(s, 16).map(f32::from_bits))
            .collect::<Result<_, _>>()?;
        if v[..3]
            .iter()
            .any(|d| !d.is_finite() || !(2. ..=16.).contains(d) || d.fract() != 0.)
        {
            return Err("invalid grid dimensions".into());
        }
        let dimensions: [usize; 3] = std::array::from_fn(|i| v[i] as usize);
        let count = dimensions.iter().product::<usize>();
        if count > 400 || v.len() != 12 + count * 4 {
            return Err("invalid node count".into());
        }
        let positions = v[12..].as_chunks::<4>().0;
        let result = Grid {
            dimensions,
            spacing: v[3],
            minimum: v[4..8].try_into()?,
            positions,
        }
        .sample(v[8..12].try_into()?)?;
        results.push(format!(
            "P\t{}{}",
            cells[0],
            result
                .iter()
                .map(|v| format!("\t{:08x}", v.to_bits()))
                .collect::<String>()
        ));
    }
    let out = Path::new(&args[1]);
    let parent = out.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let mut writer = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(out.file_name().ok_or("missing name")?))?;
    for row in &results {
        writeln!(writer, "{row}")?;
    }
    writer.flush()?;
    println!("deformation_sample_cases={}", results.len());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-deformation-sample: {e}");
        std::process::exit(1);
    }
}
