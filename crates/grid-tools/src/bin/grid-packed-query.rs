//! Private supplied VCQTC traversal comparison.
use std::{fs::OpenOptions, io::Write, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: grid-packed-query PRIVATE_CHUNK INPUT.tsv local/OUTPUT.tsv".into());
    }
    let chunk_path = Path::new(&args[0]);
    let input = Path::new(&args[1]);
    if chunk_path.metadata()?.len() > 2 * 1024 * 1024 || input.metadata()?.len() > 1024 * 1024 {
        return Err("input bound exceeded".into());
    }
    let bytes = std::fs::read(chunk_path)?;
    let chunk = grid_formats::collision::read_vcqtc(&bytes)?;
    let triangles: Vec<_> = chunk.triangles.iter().map(|t| t.indices).collect();
    let mesh = grid_world::packed_query::Mesh {
        bounds: chunk.bounds,
        vertices: &chunk.positions,
        triangles: &triangles,
        nodes: &chunk.nodes,
        leaves: &chunk.references,
    };
    let out = Path::new(&args[2]);
    let parent = out.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output only".into());
    }
    let mut report = String::new();
    let mut count = 0;
    for line in std::fs::read_to_string(input)?.lines() {
        count += 1;
        let fields: Vec<_> = line.split_whitespace().collect();
        if count > 1024
            || fields.len() != 10
            || fields[0].len() > 63
            || !fields[0]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err("invalid leaf case framing".into());
        }
        let words: Vec<_> = fields[1..]
            .iter()
            .map(|s| {
                if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("invalid word");
                }
                u32::from_str_radix(s, 16).map_err(|_| "invalid word")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let start = std::array::from_fn(|i| f32::from_bits(words[i]));
        let end = std::array::from_fn(|i| f32::from_bits(words[i + 4]));
        if words[8] > 3 {
            return Err("invalid cached plane".into());
        }
        let mut cache = grid_world::rectangle_cull::Cache {
            plane: words[8] as u8,
        };
        if let Some(hit) = mesh.query(start, end, &mut cache)? {
            report += &format!(
                "H\t{}\t{:08x}\t{:08x}\t{:08x}",
                fields[0],
                hit.triangle,
                hit.fraction.to_bits(),
                u32::from(hit.zero_fraction)
            );
            for v in hit.displacement {
                report += &format!("\t{:08x}", v.to_bits());
            }
            report += &format!("\t{:08x}\n", cache.plane);
        } else {
            report += &format!("N\t{}\t{:08x}\n", fields[0], cache.plane);
        }
    }
    if count == 0 {
        return Err("no cases".into());
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(out.file_name().ok_or("missing name")?))?
        .write_all(report.as_bytes())?;
    println!("packed_query_cases={count}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-packed-query: {e}");
        std::process::exit(1);
    }
}
