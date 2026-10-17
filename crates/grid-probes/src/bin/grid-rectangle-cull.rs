//! Private horizontal culling comparison.
use std::{fs::OpenOptions, io::Write, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-rectangle-cull INPUT.tsv local/OUTPUT.tsv".into());
    }
    let input = Path::new(&args[0]);
    if input.metadata()?.len() > 1024 * 1024 {
        return Err("input too large".into());
    }
    let out = Path::new(&args[1]);
    let parent = out.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output only".into());
    }
    let mut report = String::new();
    let mut count = 0;
    for line in std::fs::read_to_string(input)?.lines() {
        count += 1;
        let fields: Vec<_> = line.split_whitespace().collect();
        if count > 512
            || fields.len() != 10
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
            .collect::<Result<Vec<_>, _>>()?;
        if words[8] > 3 {
            return Err("invalid culling cache".into());
        }
        let f: Vec<_> = words[..8].iter().map(|&w| f32::from_bits(w)).collect();
        let mut cache = grid_world::rectangle_cull::Cache {
            plane: words[8] as u8,
        };
        let hit = cache.intersects(
            f[..2].try_into()?,
            f[2..4].try_into()?,
            f[4..6].try_into()?,
            f[6..8].try_into()?,
        )?;
        report += &format!(
            "P\t{}\t{:08x}\t{:08x}\n",
            fields[0],
            u32::from(hit),
            cache.plane
        );
    }
    if count == 0 {
        return Err("no cases".into());
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(out.file_name().ok_or("missing name")?))?
        .write_all(report.as_bytes())?;
    println!("rectangle_cull_cases={count}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-rectangle-cull: {e}");
        std::process::exit(1);
    }
}
