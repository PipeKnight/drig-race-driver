//! Private whole-body collision adjustment diagnostic (IBR1).
use grid_world::impact_body as kernel;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn word(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}
fn number(data: &[u8], offset: usize) -> f32 {
    f32::from_bits(word(data, offset))
}
fn vector(data: &[u8], offset: usize) -> [f32; 4] {
    std::array::from_fn(|i| number(data, offset + i * 4))
}
fn take<'a>(
    data: &'a [u8],
    cursor: &mut usize,
    length: usize,
) -> Result<&'a [u8], Box<dyn std::error::Error>> {
    let end = cursor.checked_add(length).ok_or("length overflow")?;
    let result = data.get(*cursor..end).ok_or("truncated input")?;
    *cursor = end;
    Ok(result)
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-impact-body INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 64 * 1024 * 1024 {
        return Err("input exceeds limit".into());
    }
    let data = fs::read(&args[0])?;
    let mut cursor = 0;
    let header = take(&data, &mut cursor, 8)?;
    let cases = word(header, 4) as usize;
    if word(header, 0) != 0x31524249 || !(1..=1024).contains(&cases) {
        return Err("invalid input header".into());
    }
    let mut output = header.to_vec();
    for _ in 0..cases {
        let linked = take(&data, &mut cursor, 0x424)?;
        let frame = take(&data, &mut cursor, 64)?;
        let point = take(&data, &mut cursor, 16)?;
        let direction = take(&data, &mut cursor, 16)?;
        let center = take(&data, &mut cursor, 16)?;
        let history = take(&data, &mut cursor, 40)?;
        let extra = take(&data, &mut cursor, 4)?;
        let mass = take(&data, &mut cursor, 4)?;
        let globals = take(&data, &mut cursor, 20)?;
        let seed = take(&data, &mut cursor, 4)?;
        let result = kernel::calculate(kernel::Input {
            angular: vector(linked, 0x210),
            linear: vector(linked, 0x220),
            frame: std::array::from_fn(|i| vector(frame, i * 16)),
            point: vector(point, 0),
            direction: vector(direction, 0),
            center: vector(center, 0),
            history: std::array::from_fn(|i| number(history, i * 4)),
            history_extra: number(extra, 0),
            mass: number(mass, 0),
            gain: number(globals, 0),
            random_gain: number(globals, 4),
            history_range: number(globals, 8),
            history_threshold: number(globals, 12),
            speed_threshold: number(globals, 16),
            random_seed: word(seed, 0),
        })?;
        for value in result.angular.into_iter().chain(result.linear) {
            output.extend(value.to_le_bytes());
        }
        output.extend(point);
        for value in result.history {
            output.extend(value.to_le_bytes());
        }
        output.extend(result.history_extra.to_le_bytes());
        output.extend(result.random_seed.to_le_bytes());
    }
    if cursor != data.len() {
        return Err("trailing input".into());
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
    println!("impact_body_cases={cases}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-impact-body: {error}");
        std::process::exit(1);
    }
}
