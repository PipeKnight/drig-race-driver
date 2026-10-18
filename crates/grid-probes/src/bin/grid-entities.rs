//! Private, read-only startup-state and collision descriptor report.
use grid_formats::{entities, pssg};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::Path,
};
fn read(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(256 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 256 * 1024 * 1024 {
        return Err("resource exceeds limit".into());
    }
    Ok(bytes)
}
fn name(value: &[u8]) -> Result<&str, Box<dyn std::error::Error>> {
    let text = std::str::from_utf8(value)?;
    if text.contains(['\t', '\r', '\n']) {
        return Err("invalid report name".into());
    }
    Ok(text)
}
fn bits(values: &[f32]) -> String {
    values
        .iter()
        .map(|v| format!("{:08x}", v.to_bits()))
        .collect::<Vec<_>>()
        .join(",")
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-entities GAME_ROOT local/OUTPUT.tsv".into());
    }
    let root = Path::new(&args[0]);
    let out = Path::new(&args[1]);
    let parent = out.parent().ok_or("output parent absent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("report must remain under local/".into());
    }
    let placement_bytes = read(&root.join("tracks/europe/donington/route_0/objects.ens"))?;
    let type_bytes = read(&root.join("tracks/europe/donington/objecttypes.pssg"))?;
    let placements = pssg::read_tree(&placement_bytes)?;
    let types = pssg::read_tree(&type_bytes)?;
    let entities = entities::read(&placements, &types)?;
    let mut writer = std::io::BufWriter::new(
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(parent.join(out.file_name().ok_or("output name absent")?))?,
    );
    let mut renderables = 0;
    let mut shapes = 0;
    let mut cloth = 0;
    for e in &entities {
        writeln!(
            writer,
            "E\t{}\t{}\t{}\t{}",
            name(e.id)?,
            name(e.template)?,
            bits(&e.transform),
            e.links
        )?;
        for r in &e.renderables {
            renderables += 1;
            writeln!(
                writer,
                "R\t{}\t{}\t{}#{}\t{}\t{}\t{}\t{}",
                name(e.id)?,
                name(r.component)?,
                name(r.container)?,
                name(r.node)?,
                name(r.body_node)?,
                name(r.body)?,
                name(r.body_properties)?,
                r.infinite_mass
            )?;
            for s in &r.shapes {
                shapes += 1;
                let (kind, dimensions) = match s.primitive {
                    entities::Primitive::Box {
                        width,
                        height,
                        length,
                    } => ("box", bits(&[width, height, length])),
                    entities::Primitive::Cylinder { radius, height } => {
                        ("cylinder", bits(&[radius, height]))
                    }
                    entities::Primitive::Sphere { radius } => ("sphere", bits(&[radius])),
                };
                writeln!(
                    writer,
                    "S\t{}\t{}\t{}\t{}\t{}\t{}\t{:08x}\t{}\t{}",
                    name(e.id)?,
                    name(r.component)?,
                    name(s.id)?,
                    kind,
                    dimensions,
                    bits(&s.transform),
                    s.shell.to_bits(),
                    s.phantom,
                    name(s.properties)?
                )?;
            }
        }
        for c in &e.cloth {
            cloth += 1;
            writeln!(writer, "C\t{}\t{}", name(e.id)?, name(c)?)?;
        }
    }
    writer.flush()?;
    println!(
        "entities={} renderables={renderables} shape_instances={shapes} omitted_cloth={cloth}",
        entities.len()
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-entities: {e}");
        std::process::exit(1);
    }
}
