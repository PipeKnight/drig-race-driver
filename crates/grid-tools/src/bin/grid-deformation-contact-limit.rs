//! Private static-peer pre-solver cap diagnostic, complete owned buffers.
use grid_world::{
    deformation_contact_limit::{self, PHYSICAL_WORDS, Parameters},
    deformation_layout::WORDS,
};
use std::{fs::OpenOptions, io::Write, path::Path};
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-deformation-contact-limit INPUT.bin local/OUTPUT.bin".into());
    }
    let output = Path::new(&args[1]);
    if !output
        .parent()
        .ok_or("missing output parent")?
        .canonicalize()?
        .starts_with(Path::new("local").canonicalize()?)
        || std::fs::metadata(&args[0])?.len() > 8 * 1024 * 1024
    {
        return Err("bounded private output required".into());
    }
    let b = std::fs::read(&args[0])?;
    const ROW: usize = WORDS * 4 + PHYSICAL_WORDS * 4 + 128 + 16 + 96;
    if b.len() < 8
        || &b[..4] != b"DC01"
        || !(1..=128).contains(&word(&b, 4))
        || b.len() != 8 + word(&b, 4) as usize * ROW
    {
        return Err("invalid contact limit framing".into());
    }
    let mut out = b[..8].to_vec();
    for r in b[8..].as_chunks::<ROW>().0 {
        let d: Vec<_> = (0..WORDS).map(|i| word(r, i * 4)).collect();
        let body: [u32; PHYSICAL_WORDS] = std::array::from_fn(|i| word(r, WORDS * 4 + i * 4));
        let at = WORDS * 4 + PHYSICAL_WORDS * 4;
        let contact = std::array::from_fn(|i| word(r, at + i * 4));
        let center = std::array::from_fn(|i| f32::from_bits(word(r, at + 128 + i * 4)));
        let p = at + 144;
        let f = |i| f32::from_bits(word(r, p + i * 4));
        if word(r, p + 92) > 1 || body[0x334 / 4] != 0 {
            return Err("invalid contact limit owned fields".into());
        }
        let answer = deformation_contact_limit::calculate(
            &d,
            &body,
            center,
            &contact,
            Parameters {
                low_mass_threshold: f(0),
                low_mass_speed_cap: f(1),
                corner_scale: f(2),
                velocity_projection: f(3),
                stiffness_scale: f(4),
                unlimited: f(5),
                damage_scale: f(6),
                table: std::array::from_fn(|i| f(7 + i)),
                global_disabled: word(r, p + 92) != 0,
            },
        )?;
        for v in answer
            .descriptor
            .into_iter()
            .chain(body)
            .chain(answer.contact)
        {
            out.extend(v.to_le_bytes());
        }
        out.extend(answer.cap.to_le_bytes());
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&out)?;
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
