//! Private physical-material source descriptor report.
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-physical-material INPUT local/OUTPUT.tsv".into());
    }
    let out = Path::new(&args[1]);
    let parent = out.parent().ok_or("output parent absent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("output must remain private".into());
    }
    if fs::metadata(&args[0])?.len() > 2 * 1024 * 1024 {
        return Err("resource exceeds limit".into());
    }
    let bytes = fs::read(&args[0])?;
    let doc = grid_formats::binxml::read(&bytes)?;
    let materials = grid_formats::surface::read(&doc)?;
    let surfaces = grid_tools::contact_surfaces::Surfaces::read(&bytes)?;
    let mut writer = std::io::BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(out.file_name().ok_or("output name absent")?))?,
    );
    for m in &materials {
        if surfaces.physical(m.code)?.1.map(f32::to_bits) != m.physical.row_words(m.code) {
            return Err("physical surface adapter disagrees with source".into());
        }
        write!(writer, "P")?;
        for word in m.physical.row_words(m.code) {
            write!(writer, "\t{word:08x}")?;
        }
        writeln!(
            writer,
            "\t{:08x}\t{:08x}",
            u32::from_le_bytes(m.physical.color),
            u32::from(m.physical.sheet)
        )?;
    }
    writer.flush()?;
    println!("materials={}", materials.len());
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-physical-material: {e}");
        std::process::exit(1);
    }
}
