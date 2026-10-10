//! Private VVB1 diagnostic for the vehicle/physical transfer boundary.
use grid_world::{
    suspension_prepare,
    vehicle_bridge::{self, Settings, State},
};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn word(d: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(d[o..o + 4].try_into().unwrap())
}
fn take<'a>(d: &'a [u8], c: &mut usize, n: usize) -> Result<&'a [u8], Box<dyn std::error::Error>> {
    let end = c.checked_add(n).ok_or("length overflow")?;
    let out = d.get(*c..end).ok_or("truncated input")?;
    *c = end;
    Ok(out)
}
fn words<const N: usize>(d: &[u8]) -> [u32; N] {
    std::array::from_fn(|i| word(d, i * 4))
}
fn append(out: &mut Vec<u8>, d: &[u32]) {
    for w in d {
        out.extend(w.to_le_bytes());
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-vehicle-bridge INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 32 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let data = fs::read(&args[0])?;
    let mut cursor = 0;
    let h = take(&data, &mut cursor, 8)?;
    let count = word(h, 4) as usize;
    if word(h, 0) != 0x31425656 || !(1..=128).contains(&count) {
        return Err("invalid header".into());
    }
    let mut output = h.to_vec();
    for _ in 0..count {
        let h = take(&data, &mut cursor, 24)?;
        let n = word(h, 0) as usize;
        if n > 64 || [4, 8, 12, 16].iter().any(|&o| word(h, o) > 1) {
            return Err("invalid case bounds".into());
        }
        let physical = words(take(&data, &mut cursor, 0x424)?);
        let vehicle = suspension_prepare::State {
            words: words(take(&data, &mut cursor, 0x2600)?),
        };
        let correction: [u32; 8] = words(take(&data, &mut cursor, 32)?);
        let correction = correction.map(f32::from_bits);
        if correction.iter().any(|v| !v.is_finite()) {
            return Err("nonfinite correction".into());
        }
        let raw = take(&data, &mut cursor, n * 0x220)?;
        let shapes = raw
            .as_chunks::<0x220>()
            .0
            .iter()
            .map(|s| words(s))
            .collect();
        let state = State {
            physical,
            vehicle,
            shapes,
        };
        let out = vehicle_bridge::calculate(
            &state,
            &Settings {
                notify: word(h, 4) != 0,
                advance: (word(h, 8) != 0).then(|| f32::from_bits(word(h, 20))),
                correction: (word(h, 12) != 0).then(|| {
                    (
                        std::array::from_fn(|i| correction[i]),
                        std::array::from_fn(|i| correction[i + 4]),
                    )
                }),
                group_head: word(h, 16) != 0,
            },
        )?;
        output.extend(h);
        for s in [out.written, out.advanced, out.corrected] {
            append(&mut output, &s.physical);
            append(&mut output, &s.vehicle.words);
            for shape in s.shapes {
                append(&mut output, &shape);
            }
        }
    }
    if cursor != data.len() {
        return Err("trailing input".into());
    }
    let path = Path::new(&args[1]);
    let parent = path.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let mut f = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(path.file_name().ok_or("missing name")?))?;
    f.write_all(&output)?;
    f.flush()?;
    println!("vehicle_bridge_cases={count}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-vehicle-bridge: {e}");
        std::process::exit(1);
    }
}
