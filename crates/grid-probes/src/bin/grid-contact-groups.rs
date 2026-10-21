//! Private owned contact-group reconstruction diagnostic, CG01.
use grid_world::contact_groups::{self, Input};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
type Failure = Box<dyn std::error::Error>;
fn take<'a>(data: &'a [u8], at: &mut usize, n: usize) -> Result<&'a [u8], Failure> {
    let end = at.checked_add(n).ok_or("length overflow")?;
    let slice = data.get(*at..end).ok_or("truncated input")?;
    *at = end;
    Ok(slice)
}
fn word(data: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(data[at..at + 4].try_into().unwrap())
}
fn run() -> Result<(), Failure> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-contact-groups INPUT.bin local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 32 * 1024 * 1024 {
        return Err("input exceeds bound".into());
    }
    let data = fs::read(&args[0])?;
    let mut at = 0;
    let header = take(&data, &mut at, 8)?;
    let count = word(header, 4);
    if word(header, 0) != 0x31304743 || !(1..=1024).contains(&count) {
        return Err("invalid header".into());
    }
    let mut result = header.to_vec();
    for _ in 0..count {
        let h = take(&data, &mut at, 12)?;
        let nb = word(h, 0) as usize;
        let nc = word(h, 4) as usize;
        if !(1..=16).contains(&nb) || nc > 64 {
            return Err("invalid counts".into());
        }
        let selector = word(h, 8);
        let order = take(&data, &mut at, nb * 4)?;
        let active_order = (0..nb).map(|i| word(order, i * 4) as usize).collect();
        let mut bodies = Vec::new();
        let mut contacts = Vec::new();
        for _ in 0..nb {
            let raw = take(&data, &mut at, 0x424)?;
            bodies.push(std::array::from_fn(|i| word(raw, i * 4)));
        }
        for _ in 0..nc {
            let raw = take(&data, &mut at, 0x80)?;
            contacts.push(std::array::from_fn(|i| word(raw, i * 4)));
        }
        let out = contact_groups::calculate(&Input {
            bodies,
            contacts,
            active_order,
            selector,
            joints: vec![],
        })?;
        for v in out
            .world
            .iter()
            .chain(out.bodies.iter().flatten())
            .chain(out.contacts.iter().flatten())
        {
            result.extend_from_slice(&v.to_le_bytes());
        }
    }
    if at != data.len() {
        return Err("trailing input".into());
    }
    let target = Path::new(&args[1]);
    let parent = fs::canonicalize(target.parent().ok_or("missing output parent")?)?;
    let private = fs::canonicalize("local")?;
    if !parent.starts_with(private) {
        return Err("private output required".into());
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?
        .write_all(&result)?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
