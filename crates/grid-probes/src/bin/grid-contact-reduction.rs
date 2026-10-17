//! Private contact-reduction diagnostic (ICR1).
use grid_world::{contact_reduction, impact_generation::Contact};
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
fn take<'a>(
    data: &'a [u8],
    cursor: &mut usize,
    size: usize,
) -> Result<&'a [u8], Box<dyn std::error::Error>> {
    let end = cursor.checked_add(size).ok_or("length overflow")?;
    let result = data.get(*cursor..end).ok_or("truncated input")?;
    *cursor = end;
    Ok(result)
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-contact-reduction INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 4 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let data = fs::read(&args[0])?;
    let mut cursor = 0;
    let header = take(&data, &mut cursor, 8)?;
    let cases = word(header, 4) as usize;
    if word(header, 0) != 0x31524349 || !(1..=1024).contains(&cases) {
        return Err("invalid header".into());
    }
    let mut output = header.to_vec();
    for _ in 0..cases {
        let count = word(take(&data, &mut cursor, 4)?, 0) as usize;
        if !(1..=80).contains(&count) {
            return Err("contact count outside bound".into());
        }
        let mut contacts = Vec::with_capacity(count);
        for _ in 0..count {
            let c = take(&data, &mut cursor, 40)?;
            if word(c, 0) > 1 {
                return Err("invalid reverse flag".into());
            }
            contacts.push(Contact {
                enabled: false,
                reverse: word(c, 0) != 0,
                point: std::array::from_fn(|i| number(c, 8 + i * 4)),
                direction: std::array::from_fn(|i| number(c, 24 + i * 4)),
                value_3c: number(c, 4),
                value_38: 0.,
                value_40: 0.,
                value_44: 0.,
                counterpart_velocity: None,
            });
        }
        let result = contact_reduction::reduce(&contacts)?;
        for v in result.point.into_iter().chain(result.direction) {
            output.extend(v.to_le_bytes());
        }
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
    println!("contact_reduction_cases={cases}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-contact-reduction: {error}");
        std::process::exit(1);
    }
}
