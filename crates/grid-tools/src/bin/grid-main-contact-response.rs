//! Private whole main contact response diagnostic, MCR1.
use grid_world::main_contact_response::{self, Body, Input};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn word(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}
fn f(data: &[u8], offset: usize) -> f32 {
    f32::from_bits(word(data, offset))
}
fn take<'a>(
    data: &'a [u8],
    at: &mut usize,
    size: usize,
) -> Result<&'a [u8], Box<dyn std::error::Error>> {
    let end = at.checked_add(size).ok_or("length overflow")?;
    let value = data.get(*at..end).ok_or("truncated input")?;
    *at = end;
    Ok(value)
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-main-contact-response INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 32 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let data = fs::read(&args[0])?;
    let mut at = 0;
    let header = take(&data, &mut at, 8)?;
    let count = word(header, 4);
    if word(header, 0) != 0x3152434d || !(1..=1024).contains(&count) {
        return Err("invalid header".into());
    }
    let mut output = header.to_vec();
    for _ in 0..count {
        let h = take(&data, &mut at, 40)?;
        let n = word(h, 0) as usize;
        let nb = word(h, 4) as usize;
        let ends = std::array::from_fn(|i| word(h, 8 + i * 4) as usize);
        let iterations = word(h, 24) as usize;
        if n > 256
            || nb > 64
            || iterations > 32
            || ends[3] != n
            || ends.windows(2).any(|w| w[0] > w[1])
        {
            return Err("invalid counts/spans".into());
        }
        let raw = take(&data, &mut at, n * 192)?;
        let rows: Vec<[u32; 48]> = raw
            .as_chunks::<192>()
            .0
            .iter()
            .map(|r| std::array::from_fn(|i| word(r, i * 4)))
            .collect();
        let references = rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                if i >= ends[1] {
                    Some(r[42] as usize)
                } else {
                    None
                }
            })
            .collect();
        let mut bodies = Vec::with_capacity(nb);
        for _ in 0..nb {
            let b = take(&data, &mut at, 100)?;
            bodies.push(Body {
                workspace: std::array::from_fn(|v| std::array::from_fn(|i| f(b, v * 16 + i * 4))),
                inverse_mass: f(b, 32),
                inverse_inertia: std::array::from_fn(|v| {
                    std::array::from_fn(|i| f(b, 36 + v * 16 + i * 4))
                }),
            });
        }
        let o = main_contact_response::calculate(&Input {
            rows,
            ends,
            references,
            bodies,
            iterations,
            softness_scale: f(h, 28),
            bias_scale: f(h, 32),
            relaxation: f(h, 36),
        })?;
        output.extend((n as u32).to_le_bytes());
        output.extend((nb as u32).to_le_bytes());
        for row in o.biased.into_iter().chain(o.prepared).chain(o.rows) {
            for w in row {
                output.extend(w.to_le_bytes());
            }
        }
        for value in o.bodies.into_iter().flatten().flatten() {
            output.extend(value.to_le_bytes());
        }
    }
    if at != data.len() {
        return Err("trailing input".into());
    }
    let path = Path::new(&args[1]);
    let parent = path.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(path.file_name().ok_or("missing name")?))?;
    file.write_all(&output)?;
    file.flush()?;
    println!("main_contact_response_cases={count}");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-main-contact-response: {e}");
        std::process::exit(1);
    }
}
