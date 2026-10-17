//! Private binary XML field report, with UTF-8 encoded as hex for exact comparison.
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
fn hex(value: &str) -> String {
    value
        .as_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-binxml INPUT local/OUTPUT.tsv".into());
    }
    let out = Path::new(&args[1]);
    let parent = out.parent().ok_or("output parent absent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("output must remain private".into());
    }
    let mut bytes = Vec::new();
    File::open(&args[0])?
        .take(256 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 256 * 1024 * 1024 {
        return Err("resource exceeds limit".into());
    }
    let doc = grid_formats::binxml::read(&bytes)?;
    let mut writer = std::io::BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(out.file_name().ok_or("output name absent")?))?,
    );
    for (i, e) in doc.elements.iter().enumerate() {
        writeln!(
            writer,
            "E\t{i}\t{}\t{}\t{}\t{}\t{}",
            hex(e.name),
            e.value.map(hex).unwrap_or_default(),
            e.parent.map(|v| v.to_string()).unwrap_or_default(),
            e.children.start,
            e.children.end
        )?;
        for (name, value) in &e.attributes {
            writeln!(writer, "A\t{i}\t{}\t{}", hex(name), hex(value))?;
        }
    }
    writer.flush()?;
    println!(
        "strings={} elements={} attributes={} string_padding={}",
        doc.strings.len(),
        doc.elements.len(),
        doc.elements
            .iter()
            .map(|e| e.attributes.len())
            .sum::<usize>(),
        doc.string_padding.len()
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-binxml: {e}");
        std::process::exit(1);
    }
}
