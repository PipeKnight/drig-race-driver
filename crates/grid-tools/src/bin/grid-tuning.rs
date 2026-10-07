//! Private positional tuning field report, preserving all unknown labels.
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-tuning INPUT local/OUTPUT.tsv".into());
    }
    let out = Path::new(&args[1]);
    let parent = out.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("report must remain private".into());
    }
    let mut bytes = Vec::new();
    File::open(&args[0])?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    let table = grid_formats::tuning::read(&bytes)?;
    let mut writer = std::io::BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(out.file_name().ok_or("missing filename")?))?,
    );
    writeln!(writer, "B\t{}", table.blank_lines)?;
    for (i, row) in table.rows.iter().enumerate() {
        writeln!(writer, "R\t{i}\t{}", row.fields.len())?;
        for (j, field) in row.fields.iter().enumerate() {
            let hex = field
                .as_bytes()
                .iter()
                .map(|v| format!("{v:02x}"))
                .collect::<String>();
            let number = match row.number(j) {
                Ok(Some(v)) => format!("{:016x}", v.to_bits()),
                Ok(None) => String::new(),
                Err(_) => String::from("nonnumeric"),
            };
            writeln!(writer, "C\t{i}\t{j}\t{hex}\t{number}")?;
        }
    }
    writer.flush()?;
    println!(
        "rows={} blank_lines={}",
        table.rows.len(),
        table.blank_lines
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-tuning: {e}");
        std::process::exit(1);
    }
}
