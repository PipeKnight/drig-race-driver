//! Private supplied-response sequence diagnostic (RSC1).
use grid_world::response_consumption as kernel;
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
    let result = data.get(*cursor..end).ok_or("truncated input")?;
    *cursor = end;
    Ok(result)
}
fn word(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(data[offset..offset + 4].try_into().unwrap())
}
fn push(output: &mut Vec<u8>, value: u32) {
    output.extend(value.to_le_bytes());
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-response-consumption INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 32 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let data = fs::read(&args[0])?;
    let mut cursor = 0;
    let header = take(&data, &mut cursor, 8)?;
    let cases = word(header, 4) as usize;
    if word(header, 0) != 0x31435352 || !(1..=1024).contains(&cases) {
        return Err("invalid header".into());
    }
    let mut output = header.to_vec();
    for _ in 0..cases {
        let h = take(&data, &mut cursor, 16)?;
        let (owner_count, contact_count, pending_count, passes) = (
            word(h, 0) as usize,
            word(h, 4) as usize,
            word(h, 8) as usize,
            word(h, 12) as usize,
        );
        if !(1..=64).contains(&owner_count)
            || contact_count > 512
            || pending_count > 240
            || !(1..=8).contains(&passes)
        {
            return Err("row outside bounds".into());
        }
        let mut owners: Vec<kernel::Owner> = Vec::new();
        for _ in 0..owner_count {
            let b = take(&data, &mut cursor, 8)?;
            let identity = word(b, 0);
            if identity == 0 || word(b, 4) > 1 || owners.iter().any(|o| o.identity == identity) {
                return Err("invalid owner".into());
            }
            owners.push(kernel::Owner {
                identity,
                registered: word(b, 4) != 0,
            });
        }
        let owner_for = |identity| -> Result<Option<kernel::Owner>, Box<dyn std::error::Error>> {
            if identity == 0 {
                Ok(None)
            } else {
                owners
                    .iter()
                    .find(|o| o.identity == identity)
                    .copied()
                    .map(Some)
                    .ok_or("unknown owner".into())
            }
        };
        let mut contacts = Vec::new();
        for _ in 0..contact_count {
            let b = take(&data, &mut cursor, 20)?;
            if word(b, 12) > 1 {
                return Err("invalid object kind".into());
            }
            contacts.push(kernel::Contact {
                identity: word(b, 0),
                first: owner_for(word(b, 4))?,
                second: owner_for(word(b, 8))?,
                second_is_body: word(b, 12) == 0,
                accumulated: f32::from_bits(word(b, 16)),
            });
        }
        let mut pending = Vec::new();
        for _ in 0..pending_count {
            let b = take(&data, &mut cursor, 8)?;
            pending.push(kernel::Pending {
                contact: word(b, 0),
                value: f32::from_bits(word(b, 4)),
            });
        }
        push(&mut output, passes as u32);
        for _ in 0..passes {
            let b = take(&data, &mut cursor, 8)?;
            let owner = word(b, 0);
            owner_for(owner)?.ok_or("zero owner")?;
            let count = word(b, 4) as usize;
            if count > 80 {
                return Err("selection exceeds bound".into());
            }
            let mut selected = Vec::new();
            let mut response = Vec::new();
            for _ in 0..count {
                let b = take(&data, &mut cursor, 8)?;
                selected.push(word(b, 0) as usize);
                response.push(f32::from_bits(word(b, 4)));
            }
            let result = kernel::consume(kernel::Input {
                owner,
                contacts: &contacts,
                selected: &selected,
                response: &response,
                pending: &pending,
            })?;
            push(&mut output, contact_count as u32);
            push(&mut output, count as u32);
            push(&mut output, result.pending.len() as u32);
            for contact in &result.contacts {
                push(&mut output, contact.accumulated.to_bits());
            }
            for value in &result.response {
                push(&mut output, value.to_bits());
            }
            for pair in &result.pending {
                push(&mut output, pair.contact);
                push(&mut output, pair.value.to_bits());
            }
            contacts = result.contacts;
            pending = result.pending;
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
    println!("response_consumption_cases={cases}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-response-consumption: {error}");
        std::process::exit(1);
    }
}
