use std::{fs, io::Write, path::Path};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 || fs::metadata(&args[0])?.len() > 2 * 1024 * 1024 {
        return Err("usage: grid-damage-boxes CAR.ned DB01_REQUEST local/OUTPUT".into());
    }
    let request = fs::read(&args[1])?;
    if request.len() != 24 || &request[..4] != b"DB01" {
        return Err("invalid damage box framing".into());
    }
    let u = |o| u32::from_le_bytes(request[o..o + 4].try_into().unwrap());
    if u(20) > 255 {
        return Err("invalid damage box fill".into());
    }
    let offset = std::array::from_fn(|i| f32::from_bits(u(4 + i * 4)));
    let bytes = fs::read(&args[0])?;
    let regions = grid_formats::damage_boxes::read(&grid_formats::binxml::read(&bytes)?)?;
    let mut output = Vec::new();
    for region in regions {
        let mut record = [u(20) as u8; 48];
        grid_formats::damage_boxes::apply(region, offset, &mut record)?;
        output.extend(record);
    }
    let path = Path::new(&args[2]);
    if !path
        .parent()
        .ok_or("missing parent")?
        .canonicalize()?
        .starts_with(Path::new("local").canonicalize()?)
    {
        return Err("private output required".into());
    }
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?
        .write_all(&output)?;
    println!("damage_boxes=5");
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
