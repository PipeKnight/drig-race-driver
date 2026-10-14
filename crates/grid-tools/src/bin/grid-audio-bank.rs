//! Private source-bank inspection and selected mono PCM16 WAV wrapping.
use grid_formats::audio_dictionary;
use std::{
    fmt::Write as _,
    fs,
    io::{Read, Write},
    path::Path,
};
fn read(path: &Path, limit: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err("audio source exceeds bound".into());
    }
    Ok(bytes)
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: grid-audio-bank GAME_ROOT BANK_NAME local/NEW_DIRECTORY".into());
    }
    let root = Path::new(&args[0]);
    let output = Path::new(&args[2]);
    if !output
        .parent()
        .ok_or("missing output parent")?
        .canonicalize()?
        .starts_with(Path::new("local").canonicalize()?)
    {
        return Err("private output required".into());
    }
    let source = read(&root.join("audio/CMSamples.dic"), 16 * 1024 * 1024)?;
    let dictionary = audio_dictionary::read(&source)?;
    let selected = args[1].to_str().ok_or("invalid bank name")?;
    let bank = dictionary
        .banks
        .iter()
        .find(|b| b.name == selected)
        .ok_or("bank not found")?;
    if bank.extension != "WIP" {
        return Err("only selected WIP mono PCM16 is decoded".into());
    }
    let data = read(
        &root
            .join("audio")
            .join(format!("{}.{}", bank.name, bank.extension)),
        64 * 1024 * 1024,
    )?;
    let waves: Vec<_> = (0..bank.samples.len())
        .map(|i| bank.mono_pcm16_wav(i, &data))
        .collect::<Result<_, _>>()?;
    let mut text = format!(
        "P\tDIC1\t2.00\t{:08x}\t{}\n",
        dictionary.opaque_id,
        dictionary.banks.len()
    );
    for bank in &dictionary.banks {
        writeln!(
            text,
            "B\t{}\t{}\t{}\t{}",
            bank.name,
            bank.extension,
            bank.data_bytes,
            bank.samples.len()
        )?;
        for sample in &bank.samples {
            writeln!(
                text,
                "S\t{}\t{}\t{}\t{}\t{}\t{:04x}",
                bank.name, sample.name, sample.offset, sample.end, sample.rate, sample.flags
            )?;
        }
    }
    // Validate every selected sample and construct all output before creating
    // the new private directory. The purchased sources are always read-only.
    fs::create_dir(output)?;
    fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output.join("dictionary.tsv"))?
        .write_all(text.as_bytes())?;
    for (sample, wave) in bank.samples.iter().zip(waves) {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output.join(format!("{}.wav", sample.name)))?
            .write_all(&wave)?;
    }
    println!(
        "audio_dictionary banks={} samples={} selected_samples={} pcm_bytes={}",
        dictionary.banks.len(),
        dictionary
            .banks
            .iter()
            .map(|b| b.samples.len())
            .sum::<usize>(),
        bank.samples.len(),
        data.len()
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-audio-bank: {e}");
        std::process::exit(1);
    }
}
