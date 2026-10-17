//! Private marker coordinates for independent float validation.
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-markers INPUT local/OUTPUT.tsv".into());
    }
    let out = Path::new(&args[1]);
    let parent = out.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("report must be private".into());
    }
    let mut bytes = Vec::new();
    File::open(&args[0])?
        .take(256 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 256 * 1024 * 1024 {
        return Err("input exceeds size limit".into());
    }
    let tree = grid_formats::pssg::read_tree(&bytes)?;
    let mut writer = std::io::BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(out.file_name().ok_or("missing output name")?))?,
    );
    let mut count = 0;
    for i in 0..tree.nodes.len() {
        if tree.name(i) != b"NODE" {
            continue;
        }
        let Ok(id) = tree.string(i, b"id") else {
            continue;
        };
        if !id.starts_with(b"slot_") {
            continue;
        }
        let marker = grid_formats::grid::read(&tree, id)?;
        write!(
            writer,
            "M\t{}",
            id.iter().map(|v| format!("{v:02x}")).collect::<String>()
        )?;
        for value in marker
            .raw_transform
            .into_iter()
            .chain(marker.corners.into_iter().flatten())
            .chain(marker.center)
            .chain(marker.forward)
        {
            write!(writer, "\t{:08x}", value.to_bits())?;
        }
        writeln!(writer)?;
        count += 1;
    }
    writer.flush()?;
    println!("markers={count}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-markers: {e}");
        std::process::exit(1);
    }
}
