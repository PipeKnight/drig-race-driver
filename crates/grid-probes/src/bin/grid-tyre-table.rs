//! Private source tyre-table report, including absent original row slots.
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-tyre-table INPUT.csv local/OUTPUT.tsv".into());
    }
    let table = grid_formats::tyre_table::read(&fs::read(&args[0])?)?;
    let out = Path::new(&args[1]);
    let parent = out.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let mut writer = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(out.file_name().ok_or("missing name")?))?;
    writeln!(
        writer,
        "T\t{:08x}\t{:08x}",
        u32::from(table.active()),
        table.columns.len()
    )?;
    for (i, code) in table.columns.iter().enumerate() {
        writeln!(writer, "K\t{i:08x}\t{:08x}", u32::from_le_bytes(*code))?;
    }
    for (i, row) in table.rows.iter().enumerate() {
        write!(writer, "R\t{i:08x}\t{:08x}", u32::from(row.is_some()))?;
        if let Some(values) = row {
            for v in values {
                write!(writer, "\t{:08x}", v.to_bits())?;
            }
        }
        writeln!(writer)?;
    }
    writer.flush()?;
    println!(
        "tyre_table_columns={} rows={}",
        table.columns.len(),
        table.rows.iter().filter(|r| r.is_some()).count()
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-tyre-table: {e}");
        std::process::exit(1);
    }
}
