//! Private SCM1 diagnostic for the serial plan with guarded-off callbacks.
use grid_world::{collision_dispatch, contact_collection::Contact, owner_collection};
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
fn push(data: &mut Vec<u8>, value: u32) {
    data.extend(value.to_le_bytes());
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-serial-collision-dispatch INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 32 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let data = fs::read(&args[0])?;
    let mut cursor = 0;
    let header = take(&data, &mut cursor, 8)?;
    let cases = word(header, 4);
    if word(header, 0) != 0x314d4353 || !(1..=1024).contains(&cases) {
        return Err("invalid header".into());
    }
    let mut output = header.to_vec();
    for _ in 0..cases {
        let h = take(&data, &mut cursor, 20)?;
        let (no, na, nb) = (
            word(h, 0) as usize,
            word(h, 4) as usize,
            word(h, 8) as usize,
        );
        let mut reverse = word(h, 12);
        let passes = word(h, 16);
        if no > 32 || na > 512 || nb > 512 || reverse > 255 || !(1..=8).contains(&passes) {
            return Err("row exceeds bounds".into());
        }
        let mut owners = Vec::new();
        for _ in 0..no {
            let b = take(&data, &mut cursor, 8)?;
            if word(b, 4) > 1 {
                return Err("invalid registration".into());
            }
            owners.push(owner_collection::Owner {
                identity: word(b, 0),
                registered: word(b, 4) != 0,
            });
        }
        let mut contacts = Vec::new();
        for _ in 0..na + nb {
            let b = take(&data, &mut cursor, 24)?;
            if [word(b, 12), word(b, 16), word(b, 20)]
                .iter()
                .any(|&v| v > 1)
            {
                return Err("invalid kind/disable byte".into());
            }
            contacts.push(Contact {
                identity: word(b, 0),
                first: word(b, 4),
                second: word(b, 8),
                second_is_body: word(b, 12) == 0,
                disabled: [word(b, 16) != 0, word(b, 20) != 0],
            });
        }
        push(&mut output, passes);
        for _ in 0..passes {
            let plan = collision_dispatch::plan(
                owner_collection::Input {
                    owners: &owners,
                    first_pool: &contacts[..na],
                    second_pool: &contacts[na..],
                },
                reverse as u8,
            )?;
            reverse = u32::from(plan.next_reverse);
            push(&mut output, reverse);
            push(&mut output, plan.calls.len() as u32);
            push(&mut output, 0);
            for call in plan.calls {
                push(&mut output, call.owner);
                push(&mut output, call.contacts.len() as u32);
                for contact in call.contacts {
                    push(&mut output, contact);
                }
            }
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
    println!("serial_collision_dispatch_cases={cases}");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-serial-collision-dispatch: {error}");
        std::process::exit(1);
    }
}
