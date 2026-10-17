//! Print the purchased surface_materials.xml physical solver rows for keys.
use grid_tools::contact_surfaces::Surfaces;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        return Err("usage: grid-surface-rows ROOT KEY [KEY ...]".into());
    }
    let surfaces = Surfaces::read(&std::fs::read(
        Path::new(&args[0]).join("surface_materials.xml"),
    )?)?;
    for key in &args[1..] {
        let mut code = [b' '; 4];
        for (slot, byte) in code.iter_mut().zip(key.bytes()) {
            *slot = byte;
        }
        let (index, row) = surfaces.physical(code)?;
        println!(
            "{key}: index {index} restitution {} friction {} cfm {} correction_vel {} min_bounce {} soft {:?} threshold {} other {:?}",
            row[1],
            row[2],
            row[3],
            row[4],
            row[5],
            &row[6..10],
            row[10],
            [row[0], row[11], row[12]]
        );
    }
    Ok(())
}
