//! Read-only static texture/draw audit for private matched-camera captures.
use grid_formats::{material, mesh, pssg, scene};
use std::{error::Error, path::Path};
fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: grid-surface-texture GAME_ROOT TEXTURE_ID".into());
    }
    let path = Path::new(&args[0]).join("tracks/europe/donington/tracksplit.pssg");
    if path.metadata()?.len() > 256 * 1024 * 1024 {
        return Err("resource exceeds256MiB".into());
    }
    let bytes = std::fs::read(&path)?;
    let tree = pssg::read_tree(&bytes)?;
    let bindings = material::read_texture_bindings(&tree)?;
    let meshes = mesh::read_meshes(&tree)?;
    let draws = scene::read_static_draws(&tree, &meshes)?;
    let mut count = 0;
    for b in bindings.iter().filter(|b| {
        b.texture == args[1].as_bytes()
            && [
                b"TDiffuseAlphaMap".as_slice(),
                b"TDiffuseAlphaMap2".as_slice(),
            ]
            .contains(&b.sampler)
    }) {
        for draw in draws.iter().filter(|d| d.material == b.shader) {
            let source = &meshes[draw.source];
            if source.colors_argb.len() != source.positions.len() {
                return Err("source color controls absent".into());
            }
            let mut best = None;
            for tri in draw.indices.as_chunks::<3>().0 {
                let vertices = tri.map(|i| {
                    scene::transform_point(&draw.transform, source.positions[usize::from(i)])
                });
                let red = tri
                    .iter()
                    .map(|&i| f32::from(source.colors_argb[usize::from(i)][1]) / 255.)
                    .sum::<f32>()
                    / 3.;
                let weight = if b.sampler == b"TDiffuseAlphaMap" {
                    1. - red
                } else {
                    red
                };
                let u: [f32; 3] = std::array::from_fn(|i| vertices[1][i] - vertices[0][i]);
                let v: [f32; 3] = std::array::from_fn(|i| vertices[2][i] - vertices[0][i]);
                let cross = [
                    u[1] * v[2] - u[2] * v[1],
                    u[2] * v[0] - u[0] * v[2],
                    u[0] * v[1] - u[1] * v[0],
                ];
                let area = cross.iter().map(|x| x * x).sum::<f32>().sqrt();
                if !area.is_finite() || area < 1e-6 || cross[1].abs() / area < 0.7 || weight < 0.2 {
                    continue;
                }
                let score = area * weight;
                if best.as_ref().is_none_or(|(s, _, _)| score > *s) {
                    let centre: [f32; 3] = std::array::from_fn(|i| {
                        (vertices[0][i] + vertices[1][i] + vertices[2][i]) / 3.
                    });
                    best = Some((score, centre, weight));
                }
            }
            if let Some((score, point, weight)) = best {
                println!(
                    "surface_texture draw={:?} material={:?} sampler={:?} point={point:?} layer_weight={weight:.6} score={score:.6}",
                    String::from_utf8_lossy(draw.id),
                    String::from_utf8_lossy(draw.material),
                    String::from_utf8_lossy(b.sampler)
                );
                count += 1;
            }
        }
    }
    if count == 0 {
        return Err("no selected ground triangle uses this diffuse texture".into());
    }
    println!(
        "surface_texture selected_draw_layers={count} scope=static-resource-not-live-original-binding"
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("grid-surface-texture: {e}");
        std::process::exit(1);
    }
}
