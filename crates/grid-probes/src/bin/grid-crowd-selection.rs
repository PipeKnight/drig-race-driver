//! CPU HLA catalogue diagnostic; explicit categories, no native clock/window/pose.
use grid_tools::crowd_selection;
use std::{collections::BTreeMap, error::Error, path::PathBuf};
fn main() -> Result<(), Box<dyn Error>> {
    let path = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("supply purchased HLA file")?,
    );
    use std::io::Read;
    let mut bytes = Vec::new();
    std::fs::File::open(&path)?
        .take(16 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("HLA exceeds16MiB diagnostic bound".into());
    }
    let doc = grid_formats::binxml::read(&bytes)?;
    let library = crowd_selection::Library::parse(&doc).map_err(|e| format!("HLA parse: {e:?}"))?;
    let mut names = BTreeMap::<_, Vec<_>>::new();
    for raw in &library.raw {
        names.entry(raw.name).or_default().push(raw.ordinal);
    }
    println!(
        "raw_records={} distinct_names={} duplicate_names={}",
        library.raw.len(),
        names.len(),
        names.values().filter(|v| v.len() > 1).count()
    );
    for (name, ordinals) in names.iter().filter(|(_, v)| v.len() > 1) {
        println!("ambiguous name={name} source_ordinals={ordinals:?}");
    }
    for kind in ["Spectator", "Grandstand"] {
        for sex in ["M", "F"] {
            let inputs = [
                ("Type", kind),
                ("Rig Type", "3D"),
                ("Sex", sex),
                ("Set", "1"),
                ("Prop", "None"),
            ];
            let selected = library
                .select(&inputs, &mut || 0)
                .map_err(|e| format!("selection: {e:?}"))?;
            println!(
                "EXPLICIT_AUTHORED_INPUTS {inputs:?} -> {selected:?}; native actor binding/RNG UNVERIFIED"
            );
        }
    }
    Ok(())
}
