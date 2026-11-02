//! Prints every shader instance's saved constants (group, id, name = values)
//! of a PSSG file, optionally only groups containing FILTER.
//!
//! Usage: grid-shader-constants FILE [FILTER]
use grid_formats::{material, pssg};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).ok_or("usage: grid-shader-constants FILE [FILTER]")?;
    let filter = args.get(2).map(String::as_bytes).unwrap_or_default();
    let bytes = std::fs::read(path)?;
    let tree = pssg::read_tree(&bytes)?;
    for shader in material::read_constants(&tree)? {
        if !shader.group.windows(filter.len().max(1)).any(|w| filter.is_empty() || w == filter) {
            continue;
        }
        println!("{} {}", String::from_utf8_lossy(shader.group), String::from_utf8_lossy(shader.shader));
        for (name, values) in &shader.values {
            println!("  {} = {values:?}", String::from_utf8_lossy(name));
        }
    }
    Ok(())
}
