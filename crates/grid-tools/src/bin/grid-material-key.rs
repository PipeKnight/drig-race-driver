//! Private raw packed material lookup comparison.
use std::{fs::OpenOptions, io::Write, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: grid-material-key PRIVATE_KEYS INPUT.tsv local/OUTPUT.tsv".into());
    }
    let keys_path = Path::new(&args[0]);
    let input = Path::new(&args[1]);
    if keys_path.metadata()?.len() > 16392 || input.metadata()?.len() > 1024 * 1024 {
        return Err("input bound exceeded".into());
    }
    let bytes = std::fs::read(keys_path)?;
    if bytes.get(..4) != Some(b"KEY1") {
        return Err("invalid key tag".into());
    }
    let count = u32::from_le_bytes(bytes.get(4..8).ok_or("missing count")?.try_into()?) as usize;
    if count > 4096 || bytes.len() != 8 + count * 4 {
        return Err("invalid key framing".into());
    }
    let keys: Vec<[u8; 4]> = bytes[8..].as_chunks::<4>().0.to_vec();
    let mut report = String::new();
    let mut cases = 0;
    for line in std::fs::read_to_string(input)?.lines() {
        cases += 1;
        let f: Vec<_> = line.split_whitespace().collect();
        if cases > 4096
            || f.len() != 3
            || f[0].len() > 63
            || !f[0].bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err("invalid case framing".into());
        }
        let words: Vec<u32> = f[1..]
            .iter()
            .map(|s| {
                if s.len() != 8 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err("invalid word");
                }
                u32::from_str_radix(s, 16).map_err(|_| "invalid word")
            })
            .collect::<Result<_, _>>()?;
        if words[1] > 1 {
            return Err("invalid active flag".into());
        }
        let found =
            grid_formats::surface::packed_index(&keys, words[0].to_le_bytes(), words[1] != 0);
        report += &format!(
            "P\t{}\t{:08x}\t{:08x}\n",
            f[0],
            u32::from(found.is_some()),
            found.unwrap_or(0)
        );
    }
    if cases == 0 {
        return Err("no cases".into());
    }
    let out = Path::new(&args[2]);
    let parent = out.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output only".into());
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(out.file_name().ok_or("missing name")?))?
        .write_all(report.as_bytes())?;
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-material-key: {e}");
        std::process::exit(1);
    }
}
