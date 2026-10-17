//! Read-only Donington VCQTC source-bit report for independent validation.
use grid_formats::{collision, jpk};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-collision GAME_ROOT local/OUTPUT.tsv".into());
    }
    let out = Path::new(&args[1]);
    let parent = out.parent().ok_or("output parent absent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("report must remain under local/".into());
    }
    let path = Path::new(&args[0]).join("tracks/europe/donington/route_0/track.jpk");
    let mut bytes = Vec::new();
    File::open(path)?
        .take(256 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 256 * 1024 * 1024 {
        return Err("resource exceeds limit".into());
    }
    let jpk = jpk::inspect(&bytes)?;
    let mut writer = std::io::BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(out.file_name().ok_or("output name absent")?))?,
    );
    let mut vertices = 0;
    let mut triangles = 0;
    let mut nodes = 0;
    let mut refs = 0;
    for entry in &jpk.entries {
        if !entry.name.ends_with(b".vcqtc")
            || entry.uncompressed_size as usize != entry.stored_data.len()
        {
            return Err("unsupported collision container entry".into());
        }
        let name = std::str::from_utf8(entry.name)?;
        if name.contains(['\t', '\r', '\n']) {
            return Err("invalid report name".into());
        }
        let chunk = collision::read_vcqtc(&bytes[entry.stored_data.clone()])?;
        writeln!(
            writer,
            "G\t{name}\t{}\t{}\t{}",
            chunk.positions.len(),
            chunk.triangles.len(),
            chunk.nodes.len()
        )?;
        for (i, v) in chunk.positions.iter().enumerate() {
            writeln!(
                writer,
                "V\t{name}\t{i}\t{:08x},{:08x},{:08x}",
                v[0].to_bits(),
                v[1].to_bits(),
                v[2].to_bits()
            )?;
        }
        for (i, t) in chunk.triangles.iter().enumerate() {
            let material = chunk.materials[t.material as usize];
            writeln!(
                writer,
                "T\t{name}\t{i}\t{},{},{}\t{}\t{}\t{:02x}{:02x}{:02x}{:02x}",
                t.indices[0],
                t.indices[1],
                t.indices[2],
                t.material,
                t.flags,
                material[0],
                material[1],
                material[2],
                material[3]
            )?;
        }
        for (i, node) in chunk.nodes.iter().enumerate() {
            writeln!(writer, "N\t{name}\t{i}\t{node:04x}")?;
        }
        for (offset, list) in &chunk.references {
            writeln!(
                writer,
                "P\t{name}\t{offset}\t{}",
                list.iter()
                    .map(u16::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            )?;
        }
        vertices += chunk.positions.len();
        triangles += chunk.triangles.len();
        nodes += chunk.nodes.len();
        refs += chunk.references.len();
    }
    writer.flush()?;
    println!(
        "chunks={} vertices={vertices} triangles={triangles} nodes={nodes} reference_lists={refs}",
        jpk.entries.len()
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-collision: {e}");
        std::process::exit(1);
    }
}
