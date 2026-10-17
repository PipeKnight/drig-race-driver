//! Bounded private first-pool preparation diagnostic.
use grid_world::{
    deformation_contact_limit::{PHYSICAL_WORDS, Parameters},
    deformation_layout::WORDS,
    deformation_preparation,
};
use std::{fs::OpenOptions, io::Write, path::Path};
fn word(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-deformation-preparation INPUT.bin local/OUTPUT.bin".into());
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
    const ROW: usize = 16 + WORDS * 4 + PHYSICAL_WORDS * 4 + 16 + 96 + 64 * 128;
    if b.len() < 8
        || &b[..4] != b"DC02"
        || !(1..=128).contains(&word(&b, 4))
        || b.len() != 8 + word(&b, 4) as usize * ROW
    {
        return Err("invalid preparation framing".into());
    }
    let mut out = b[..8].to_vec();
    for r in b[8..].as_chunks::<ROW>().0 {
        let count = word(r, 0) as usize;
        if count > 64 {
            return Err("invalid preparation count".into());
        }
        let d: Vec<_> = (0..WORDS).map(|i| word(r, 16 + i * 4)).collect();
        let body: [u32; PHYSICAL_WORDS] = std::array::from_fn(|i| word(r, 16 + WORDS * 4 + i * 4));
        let at = 16 + WORDS * 4 + PHYSICAL_WORDS * 4;
        let center = std::array::from_fn(|i| f32::from_bits(word(r, at + i * 4)));
        let p = at + 16;
        let f = |i| f32::from_bits(word(r, p + i * 4));
        if word(r, p + 92) > 1 || [0x334, 0x418, 0x420].iter().any(|at| body[at / 4] != 0) {
            return Err("invalid preparation owned roles".into());
        }
        let mut contacts: Vec<[u32; 32]> = (0..64)
            .map(|i| std::array::from_fn(|j| word(r, p + 96 + i * 128 + j * 4)))
            .collect();
        let answer = deformation_preparation::calculate(
            &d,
            &body,
            center,
            &contacts[..count],
            word(r, 4) as usize,
            word(r, 8) as usize,
            word(r, 12) as usize,
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
        contacts[..count].copy_from_slice(&answer.contacts);
        for v in [answer.cursor as u32, 0, u32::from(answer.complete), 0]
            .into_iter()
            .chain(answer.descriptor)
            .chain(body)
            .chain(contacts.into_iter().flatten())
        {
            out.extend(v.to_le_bytes());
        }
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
