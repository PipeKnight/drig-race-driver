//! Private whole temporary force-view preparation diagnostic.
use grid_world::force_view::{self, Input, Wheel};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-force-view INPUT.tsv local/OUTPUT.tsv".into());
    }
    let text = fs::read_to_string(&args[0])?;
    if text.len() > 4 * 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let mut results = Vec::new();
    for line in text.lines() {
        let cells: Vec<_> = line.split('\t').collect();
        if cells.len() != 113 || results.len() >= 4096 {
            return Err("invalid input row".into());
        }
        let v: Vec<_> = cells[1..]
            .iter()
            .map(|v| u32::from_str_radix(v, 16).map(f32::from_bits))
            .collect::<Result<_, _>>()?;
        let mut wheels = Vec::new();
        for i in 0..4 {
            let q = &v[16 + i * 24..40 + i * 24];
            wheels.push(Wheel {
                coordinate: q[..4].try_into()?,
                scalar_40: q[4],
                scalar_e0: q[5],
                blend_20: q[6],
                blend_24: q[7],
                matrix: std::array::from_fn(|j| q[8 + j * 4..12 + j * 4].try_into().unwrap()),
            });
        }
        let r = force_view::prepare(Input {
            body_axis_0: v[..4].try_into()?,
            body_axis_2: v[4..8].try_into()?,
            linear: v[8..12].try_into()?,
            angular: v[12..16].try_into()?,
            wheels: wheels.try_into().map_err(|_| "four wheels required")?,
        })?;
        let words: Vec<_> = r
            .iter()
            .flat_map(|w| {
                w.velocity
                    .into_iter()
                    .chain(w.longitudinal)
                    .chain(w.lateral)
                    .chain(w.lever)
            })
            .map(f32::to_bits)
            .collect();
        results.push(format!(
            "P\t{}{}",
            cells[0],
            words
                .iter()
                .map(|w| format!("\t{w:08x}"))
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
    println!("force_view_cases={}", results.len());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-force-view: {e}");
        std::process::exit(1);
    }
}
