//! Prints the animations of a front-end PSSG package (docs/SCREEN_FLOW.md):
//! for each `NEANIMATION` whose id or a channel target contains a filter
//! word, its named time ranges and every channel's keys (time, value);
//! then the matching nodes: local translation and scale, bounds, and the
//! drawn mesh's extent and material (when the whole scene reads).
//! Usage: grid-fe-anim PACKAGE.pssg [WORD ...]
use grid_formats::{osd, pssg};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("usage: grid-fe-anim PACKAGE.pssg [WORD ...]")?;
    let words: Vec<String> = args.collect();
    let bytes = std::fs::read(&path)?;
    let tree = pssg::read_tree(&bytes)?;
    let animations = osd::animations(&tree)?;
    let hit = |s: &str| words.is_empty() || words.iter().any(|w| s.contains(w.as_str()));
    let mut shown = 0;
    for a in &animations {
        let channels: Vec<_> = a.channels.iter().filter(|c| hit(&a.id) || hit(&c.target)).collect();
        if channels.is_empty() {
            continue;
        }
        shown += 1;
        println!("animation {} duration={:.3}", a.id, a.duration());
        for r in &a.ranges {
            println!("  range {} {:.3}..{:.3}", r.name, r.start, r.end);
        }
        for c in channels {
            for (i, curve) in c.components.iter().enumerate() {
                let keys: Vec<String> = curve.0.iter().map(|k| format!("{:.3}:{:.4}", k.time, k.value)).collect();
                println!("  channel {} {:?}[{i}] {}", c.target, c.semantic, keys.join(" "));
            }
        }
    }
    let mut nodes = 0;
    match osd::read_scene(&tree) {
        Ok(scene) => {
            for (i, n) in scene.nodes.iter().enumerate() {
                if !hit(&n.id) {
                    continue;
                }
                nodes += 1;
                let m = n.local;
                let parent = n.parent.map(|p| scene.nodes[p].id.as_str()).unwrap_or("-");
                println!(
                    "node {} parent={parent} t=({:.3},{:.3},{:.3}) diag=({:.3},{:.3},{:.3}) bounds={:?}",
                    n.id, m[12], m[13], m[14], m[0], m[5], m[10], n.bounds
                );
                let w = scene.world(i);
                println!("  world t=({:.3},{:.3},{:.3}) diag=({:.3},{:.3},{:.3})", w[12], w[13], w[14], w[0], w[5], w[10]);
                if n.id.starts_with("camera=") {
                    // The whole matrix: a camera's view direction is its -Z column.
                    println!("  world matrix {:?}", w);
                }
                if let Some(r) = &n.render {
                    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                    for p in &r.mesh.positions {
                        for k in 0..3 {
                            lo[k] = lo[k].min(p[k]);
                            hi[k] = hi[k].max(p[k]);
                        }
                    }
                    let mat = &scene.materials[r.material];
                    println!(
                        "  mesh vertices={} triangles={} min={lo:?} max={hi:?} material={} effect={} params={:?}",
                        r.mesh.positions.len(),
                        r.mesh.indices.len() / 3,
                        mat.id,
                        mat.effect,
                        mat.params
                    );
                }
            }
        }
        Err(e) => println!("scene not read: {e}"),
    }
    println!("grid-fe-anim: {shown} of {} animations matched, {nodes} nodes", animations.len());
    Ok(())
}
