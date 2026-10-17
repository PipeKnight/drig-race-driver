//! Private ordered-contact collector diagnostic (COC1).
use grid_world::contact_collection as kernel;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn word(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
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
        return Err("usage: grid-contact-collection INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 32 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let data = fs::read(&args[0])?;
    let mut cursor = 0;
    let header = take(&data, &mut cursor, 8)?;
    let cases = word(header, 4) as usize;
    if word(header, 0) != 0x31434f43 || !(1..=1024).contains(&cases) {
        return Err("invalid header".into());
    }
    let mut output = header.to_vec();
    for _ in 0..cases {
        let h = take(&data, &mut cursor, 20)?;
        let owner = word(h, 0);
        let capacity = word(h, 4) as usize;
        let initial_count = word(h, 8) as usize;
        let first_count = word(h, 12) as usize;
        let second_count = word(h, 16) as usize;
        if capacity > 80 || initial_count > capacity || first_count > 512 || second_count > 512 {
            return Err("row outside bounds".into());
        }
        let initial: Vec<_> = (0..initial_count)
            .map(|_| take(&data, &mut cursor, 4).map(|b| word(b, 0)))
            .collect::<Result<_, _>>()?;
        let mut contacts = Vec::with_capacity(first_count + second_count);
        for _ in 0..first_count + second_count {
            let c = take(&data, &mut cursor, 24)?;
            if [12, 16, 20].into_iter().any(|i| word(c, i) > 1) {
                return Err("invalid boolean".into());
            }
            contacts.push(kernel::Contact {
                identity: word(c, 0),
                first: word(c, 4),
                second: word(c, 8),
                second_is_body: word(c, 12) == 0,
                disabled: [word(c, 16) != 0, word(c, 20) != 0],
            });
        }
        let result = kernel::collect(kernel::Input {
            owner,
            capacity,
            initial: &initial,
            first_pool: &contacts[..first_count],
            second_pool: &contacts[first_count..],
        })?;
        output.extend((result.len() as u32).to_le_bytes());
        for identity in result {
            output.extend(identity.to_le_bytes());
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
    println!("contact_collection_cases={cases}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-contact-collection: {error}");
        std::process::exit(1);
    }
}
