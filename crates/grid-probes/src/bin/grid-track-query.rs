//! Private supplied-world comparison fixture; no proprietary outputs outside local/.
use grid_world::rectangle_cull::Cache;
use std::{fs::OpenOptions, io::Write, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: grid-track-query TRACK.jpk INPUT.tsv local/OUTPUT.tsv".into());
    }
    let path = Path::new(&args[0]);
    let input = Path::new(&args[1]);
    if path.metadata()?.len() > 256 * 1024 * 1024 || input.metadata()?.len() > 1024 * 1024 {
        return Err("input bound exceeded".into());
    }
    let mut track = grid_tools::track::Track::read(&std::fs::read(path)?)?;
    let out = Path::new(&args[2]);
    let parent = out.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output only".into());
    }
    let mut report = String::new();
    let mut cases = 0;
    for line in std::fs::read_to_string(input)?.lines() {
        cases += 1;
        let fields: Vec<_> = line.split_whitespace().collect();
        if cases > 4096
            || fields.len() != 11
            || fields[0].len() > 63
            || !fields[0]
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err("invalid case framing".into());
        }
        let words: Vec<_> = fields[1..]
            .iter()
            .map(|s| {
                if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("invalid word");
                }
                u32::from_str_radix(s, 16).map_err(|_| "invalid word")
            })
            .collect::<Result<_, _>>()?;
        if words[8] > 3 || words[9] > 1 {
            return Err("invalid cache or active flag".into());
        }
        track.grid.active = words[9] != 0;
        let mut cache = Cache {
            plane: words[8] as u8,
        };
        let result = track.query(
            std::array::from_fn(|i| f32::from_bits(words[i])),
            std::array::from_fn(|i| f32::from_bits(words[i + 4])),
            &mut cache,
        )?;
        if let Some(hit) = result.hit {
            report += &format!(
                "H\t{}\t{:08x}\t{:08x}",
                fields[0],
                hit.shape,
                hit.contact.fraction.to_bits()
            );
            for v in hit.contact.normal {
                report += &format!("\t{:08x}", v.to_bits());
            }
            report += &format!("\t{:08x}", u32::from_le_bytes(hit.contact.material));
        } else {
            report += &format!("N\t{}", fields[0]);
        }
        report += &format!("\t{:08x}\t{:08x}", cache.plane, result.candidates.len());
        for id in result.candidates {
            report += &format!("\t{id:08x}");
        }
        report += "\n";
    }
    if cases == 0 {
        return Err("no cases".into());
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(out.file_name().ok_or("missing name")?))?
        .write_all(report.as_bytes())?;
    println!("source_track_cases={cases}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-track-query: {e}");
        std::process::exit(1);
    }
}
