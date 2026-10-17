//! Private independent f32 triangle-kernel comparison output.
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-triangle INPUT.tsv local/OUTPUT.tsv".into());
    }
    let out = Path::new(&args[1]);
    let parent = out.parent().ok_or("output parent absent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("report must remain under local/".into());
    }
    let mut input = String::new();
    File::open(&args[0])?
        .take(16 * 1024 * 1024 + 1)
        .read_to_string(&mut input)?;
    if input.len() > 16 * 1024 * 1024 {
        return Err("query input exceeds limit".into());
    }
    let mut writer = std::io::BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(out.file_name().ok_or("output name absent")?))?,
    );
    let mut cases = 0;
    for line in input.lines() {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() != 16 {
            return Err("query requires name and 15 coordinates".into());
        }
        let numbers: Vec<f32> = fields[1..]
            .iter()
            .map(|v| v.parse())
            .collect::<Result<_, _>>()?;
        let vertices = std::array::from_fn(|i| numbers[6 + i * 3..9 + i * 3].try_into().unwrap());
        if let Some(hit) = grid_world::triangle::query(
            numbers[..3].try_into()?,
            numbers[3..6].try_into()?,
            vertices,
        ) {
            writeln!(
                writer,
                "H\t{}\t{:08x}\t{:08x}\t{:08x}",
                fields[0],
                hit.fraction.to_bits(),
                hit.u.to_bits(),
                hit.v.to_bits()
            )?;
        } else {
            writeln!(writer, "N\t{}", fields[0])?;
        }
        cases += 1;
    }
    writer.flush()?;
    println!("independent_cases={cases}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-triangle: {e}");
        std::process::exit(1);
    }
}
