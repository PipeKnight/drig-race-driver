//! Private complete handling settings comparison from bounded HDM source.
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::Path,
};
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-handling-configuration HDM local/OUTPUT.bin".into());
    }
    if fs::metadata(&args[0])?.len() > 65536 {
        return Err("HDM size exceeds limit".into());
    }
    let output = Path::new(&args[1]);
    let parent = output.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let settings = grid_formats::handling_damage::read_configuration(&fs::read(&args[0])?)?;
    let mut writer = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(parent.join(output.file_name().ok_or("missing filename")?))?;
    for word in settings.words() {
        writer.write_all(&word.to_le_bytes())?;
    }
    println!("handling_bytes=360");
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("grid-handling-configuration: {error}");
        std::process::exit(1);
    }
}
