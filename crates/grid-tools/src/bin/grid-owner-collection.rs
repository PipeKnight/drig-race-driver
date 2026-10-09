//! Private ONC1 ordered owner-collection diagnostic.
use grid_world::{contact_collection::Contact, owner_collection as kernel};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};

fn take<'a>(
    data: &'a [u8],
    cursor: &mut usize,
    size: usize,
) -> Result<&'a [u8], Box<dyn std::error::Error>> {
    let end = cursor.checked_add(size).ok_or("length overflow")?;
    let value = data.get(*cursor..end).ok_or("truncated input")?;
    *cursor = end;
    Ok(value)
}
fn word(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-owner-collection INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 32 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let data = fs::read(&args[0])?;
    let mut cursor = 0;
    let header = take(&data, &mut cursor, 8)?;
    let count = word(header, 4);
    if word(header, 0) != 0x31434e4f || !(1..=1024).contains(&count) {
        return Err("invalid header".into());
    }
    let mut output = header.to_vec();
    for _ in 0..count {
        let h = take(&data, &mut cursor, 16)?;
        let (no, na, nb) = (
            word(h, 0) as usize,
            word(h, 4) as usize,
            word(h, 8) as usize,
        );
        // The fourth wire word is the original unused capacity argument.
        if no > 64 || na > 512 || nb > 512 {
            return Err("row exceeds bounds".into());
        }
        let mut owners = Vec::new();
        for _ in 0..no {
            let b = take(&data, &mut cursor, 8)?;
            if word(b, 4) > 1 {
                return Err("invalid registration flag".into());
            }
            owners.push(kernel::Owner {
                identity: word(b, 0),
                registered: word(b, 4) != 0,
            });
        }
        let mut contacts = Vec::new();
        for index in 0..na + nb {
            let b = take(&data, &mut cursor, 12)?;
            if word(b, 8) > 1 {
                return Err("invalid kind".into());
            }
            contacts.push(Contact {
                identity: index as u32 + 1,
                first: word(b, 0),
                second: word(b, 4),
                second_is_body: word(b, 8) == 0,
                disabled: [false; 2],
            });
        }
        let result = kernel::collect(kernel::Input {
            owners: &owners,
            first_pool: &contacts[..na],
            second_pool: &contacts[na..],
        })?;
        output.extend((result.len() as u32).to_le_bytes());
        for id in result {
            output.extend(id.to_le_bytes());
        }
    }
    if cursor != data.len() {
        return Err("trailing input".into());
    }
    let target = Path::new(&args[1]);
    let parent = target.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(target.file_name().ok_or("missing filename")?))?;
    file.write_all(&output)?;
    file.flush()?;
    println!("owner_collection_cases={count}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-owner-collection: {error}");
        std::process::exit(1);
    }
}
