//! Prints each water.fx draw of a land.pssg: vertex count, vertex colour
//! ranges (alpha is the water's opacity, green damps the bump), and the
//! world cells (CELL metres, default 50) holding the most vertices with
//! alpha above zero, so a camera can be placed on the visible water.
//!
//! Usage: grid-water-mesh LAND.PSSG [CELL]
use grid_formats::{material, mesh, pssg, scene};
use std::collections::{BTreeMap, BTreeSet};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).ok_or("usage: grid-water-mesh LAND.PSSG [CELL]")?;
    let cell: f32 = args.get(2).map_or(Ok(50.0), |s| s.parse())?;
    let bytes = std::fs::read(path)?;
    let tree = pssg::read_tree(&bytes)?;
    let water: BTreeSet<Vec<u8>> = material::read_constants(&tree)?
        .into_iter()
        .filter(|s| s.group == b"water.fx")
        .map(|s| s.shader.to_vec())
        .collect();
    let sources = mesh::read_meshes(&tree)?;
    for draw in scene::read_static_draws(&tree, &sources)? {
        if !water.contains(draw.material) {
            continue;
        }
        let source = &sources[draw.source];
        let range = draw.vertices.clone();
        println!(
            "draw material={} vertices={} triangles={} colours={}",
            String::from_utf8_lossy(draw.material),
            range.len(),
            draw.indices.len() / 3,
            !source.colors_argb.is_empty()
        );
        if source.colors_argb.is_empty() {
            continue;
        }
        let colours = &source.colors_argb[range.clone()];
        for (i, name) in ["a", "r", "g", "b"].iter().enumerate() {
            let values: Vec<u32> = colours.iter().map(|c| u32::from(c[i])).collect();
            let mean = values.iter().sum::<u32>() as f32 / values.len() as f32;
            let zero = values.iter().filter(|&&v| v == 0).count();
            println!(
                "  {name} min={} max={} mean={mean:.1} zero={zero}",
                values.iter().min().unwrap_or(&0),
                values.iter().max().unwrap_or(&0)
            );
        }
        // Cells of vertices with alpha > 0: count, mean alpha, mean world position.
        let mut cells: BTreeMap<(i32, i32), (usize, f32, [f32; 3])> = BTreeMap::new();
        for (p, c) in source.positions[range].iter().zip(colours) {
            if c[0] == 0 {
                continue;
            }
            let w = scene::transform_point(&draw.transform, *p);
            let key = ((w[0] / cell).floor() as i32, (w[2] / cell).floor() as i32);
            let e = cells.entry(key).or_insert((0, 0.0, [0.0; 3]));
            e.0 += 1;
            e.1 += f32::from(c[0]);
            for (sum, v) in e.2.iter_mut().zip(w) {
                *sum += v;
            }
        }
        let mut list: Vec<_> = cells.into_iter().collect();
        list.sort_by_key(|a| std::cmp::Reverse(a.1.0));
        println!("  cells_with_alpha={}", list.len());
        for (_, (n, a, s)) in list.iter().take(12) {
            let m = s.map(|v| v / *n as f32);
            println!(
                "  cell vertices={n} mean_alpha={:.0} centre={:.1},{:.1},{:.1}",
                a / *n as f32,
                m[0],
                m[1],
                m[2]
            );
        }
    }
    Ok(())
}
