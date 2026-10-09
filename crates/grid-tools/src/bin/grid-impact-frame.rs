//! Private supplied-pose contact-frame diagnostic (IFR1).
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn word(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}
fn vector(data: &[u8], offset: usize) -> [f32; 4] {
    std::array::from_fn(|i| f32::from_bits(word(data, offset + i * 4)))
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-impact-frame INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 8 + 1024 * 80 {
        return Err("input exceeds limit".into());
    }
    let data = fs::read(&args[0])?;
    if data.len() < 8 {
        return Err("truncated header".into());
    }
    let cases = word(&data, 4) as usize;
    if word(&data, 0) != 0x31524649 || !(1..=1024).contains(&cases) || data.len() != 8 + cases * 80
    {
        return Err("invalid framing".into());
    }
    let mut output = data[..8].to_vec();
    for row in data[8..].as_chunks::<80>().0 {
        let frame = grid_world::impact_generation::contact_frame(
            std::array::from_fn(|i| vector(row, i * 16)),
            vector(row, 64),
        )?;
        for value in frame.into_iter().flatten() {
            output.extend(value.to_le_bytes());
        }
    }
    let out = Path::new(&args[1]);
    let parent = out.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(out.file_name().ok_or("missing name")?))?;
    file.write_all(&output)?;
    file.flush()?;
    println!("impact_frame_cases={cases}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-impact-frame: {error}");
        std::process::exit(1);
    }
}
