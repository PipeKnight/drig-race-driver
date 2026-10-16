//! Export one purchased texture privately through the validated DDS reader.
use grid_formats::{pssg, texture};
use std::{error::Error, fs, io::Write, path::PathBuf};
fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("usage: export_texture SOURCE.pssg TEXTURE_ID local/OUTPUT.dds".into());
    }
    let source = PathBuf::from(&args[0]);
    let output = PathBuf::from(&args[2]);
    let private = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../local")
        .canonicalize()?;
    if output.extension().is_none_or(|e| e != "dds")
        || !output
            .parent()
            .ok_or("missing output parent")?
            .canonicalize()?
            .starts_with(private)
    {
        return Err("output must be DDS under this checkout's existing ignored local/".into());
    }
    if source.metadata()?.len() > 256 * 1024 * 1024 {
        return Err("source exceeds resource bound".into());
    }
    let bytes = fs::read(&source)?;
    let tree = pssg::read_tree(&bytes)?;
    let textures = texture::read_named(&tree, &[args[1].as_bytes()])?;
    if textures.len() != 1 {
        return Err("expected one exact texture identifier".into());
    }
    let t = &textures[0];
    let (dds, levels) = t.validated_dds();
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)?;
    file.write_all(&dds)?;
    println!(
        "exported id={:?} width={} height={} format={:?} mip_levels={} bytes={} source={} output={}",
        args[1],
        t.width,
        t.height,
        t.format,
        levels,
        dds.len(),
        source.display(),
        output.display()
    );
    Ok(())
}
