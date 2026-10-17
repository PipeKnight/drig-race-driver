//! List one dictionary bank's samples (name, extent, rate, flags, prefix bytes).
use grid_formats::audio_dictionary;
use std::{fs, path::Path};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-audio-list GAME_ROOT BANK_NAME".into());
    }
    let root = Path::new(&args[0]);
    let source = fs::read(root.join("audio/CMSamples.dic"))?;
    let dictionary = audio_dictionary::read(&source)?;
    let selected = args[1].to_str().ok_or("invalid bank name")?;
    let bank = dictionary
        .banks
        .iter()
        .find(|b| b.name == selected)
        .ok_or("bank not found")?;
    let data = fs::read(root.join(format!("audio/{}.{}", bank.name, bank.extension)))?;
    println!(
        "bank={} extension={} data_bytes={} file_bytes={} samples={}",
        bank.name,
        bank.extension,
        bank.data_bytes,
        data.len(),
        bank.samples.len()
    );
    for (i, s) in bank.samples.iter().enumerate() {
        let prefix = data
            .get(s.offset..(s.offset + 8).min(s.end))
            .map(|b| b.iter().map(|x| format!("{x:02x}")).collect::<String>())
            .unwrap_or_default();
        println!(
            "{i}\t{}\t{}\t{}\t{}\t{:04x}\t{prefix}",
            s.name,
            s.offset,
            s.end - s.offset,
            s.rate,
            s.flags
        );
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("grid-audio-list: {error}");
        std::process::exit(1);
    }
}
