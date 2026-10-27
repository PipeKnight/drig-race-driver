//! Lists a car package's draws per LOD bundle (`LOD0_`..`LOD7_`): draw id,
//! joint, material and triangles, for docs/CAR_LOD.md. Usage:
//! `grid-car-lod-draws PATH.pssg`.
use grid_formats::{mesh, pssg, scene};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("usage: grid-car-lod-draws PATH.pssg")?;
    let bytes = std::fs::read(&path)?;
    let tree = pssg::read_tree(&bytes)?;
    let sources = mesh::read_meshes(&tree)?;
    for level in 0..8 {
        let Ok(draws) = scene::read_draws(&tree, &sources, format!("LOD{level}_").as_bytes()) else {
            continue;
        };
        let triangles: usize = draws.iter().map(|d| d.indices.len() / 3).sum();
        println!("lod={level} draws={} triangles={triangles}", draws.len());
        for d in &draws {
            println!(
                "  draw lod={level} id={} joint={} material={:?} triangles={}",
                String::from_utf8_lossy(d.id),
                String::from_utf8_lossy(d.joint),
                String::from_utf8_lossy(d.material),
                d.indices.len() / 3
            );
        }
    }
    Ok(())
}
