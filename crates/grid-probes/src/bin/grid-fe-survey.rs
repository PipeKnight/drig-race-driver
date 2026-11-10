//! One-off survey of a front-end screen (docs/RACE_HUD.md §6.6): the grafted
//! nodes below a root with their materials, the package's cameras and the
//! animations that move the root's ancestors. Usage:
//! `grid-fe-survey GAME_ROOT ROOT` (prints to stdout; private output).
use grid_formats::{osd, osd_glyph, pssg, texture};
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let game = std::path::PathBuf::from(&args[1]);
    let root_id = &args[2];
    let files = ["fePersistentDatabase.pssg", "feNonPersistentDatabase.pssg", "fe_DLC_0.pssg"];
    let bytes: Vec<Vec<u8>> = files.iter().map(|f| std::fs::read(game.join("frontend").join(f)).unwrap()).collect();
    let trees: Vec<pssg::Tree> = bytes.iter().map(|b| pssg::read_tree(b).unwrap()).collect();
    let scenes: Vec<osd::Scene> = trees.iter().map(|t| osd::read_scene(t).unwrap()).collect();
    let refs: Vec<&osd::Scene> = scenes.iter().collect();
    // The first package that defines the root (feNonPersistentDatabase holds the livery editor).
    let find = |s: &osd::Scene| s.node(root_id).or_else(|| s.nodes.iter().position(|n| n.name() == root_id));
    let (si, root) = scenes.iter().enumerate().find_map(|(i, s)| find(s).map(|r| (i, r))).unwrap();
    let s0 = &scenes[si];
    let placed = osd_glyph::expand(&refs, si, root);
    // SURVEY_TEXTURES=a,b,c: where each named texture is, at which size
    // (the packages and the b_persistent bundle; cubes by id).
    if let Some(want) = std::env::var_os("SURVEY_TEXTURES") {
        let want = want.to_string_lossy().into_owned();
        let bundle_bytes = std::fs::read(game.join("frontend/b_persistent.pssg")).unwrap();
        let bundle = pssg::read_tree(&bundle_bytes).unwrap();
        for name in want.split(',') {
            for (scene, file) in scenes.iter().zip(files) {
                for t in scene.textures.iter().filter(|t| t.id == name.as_bytes()) {
                    println!("texture {name} {file} {}x{}", t.width, t.height);
                }
                for c in scene.cubemaps.iter().filter(|c| c.id == name) {
                    println!("cube {name} {file} {}", c.size);
                }
            }
            // The bundle holds bare TEXTURE nodes (no scene around them).
            for t in texture::read_named(&bundle, &[name.as_bytes()]).unwrap_or_default() {
                println!("texture {name} b_persistent.pssg {}x{}", t.width, t.height);
            }
        }
    }
    let mut by_material: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    for (index, p) in placed.iter().enumerate() {
        let n = &scenes[p.scene].nodes[p.node];
        if let Some(r) = &n.render {
            *by_material.entry((p.scene, r.material)).or_default() += 1;
        }
        if std::env::var_os("SURVEY_NODES").is_some() {
            let r = n.render.as_ref().map(|r| {
                let m = &scenes[p.scene].materials[r.material];
                // The mesh's world-space box (row-vector matrices).
                let w = &p.world;
                let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
                for v in &r.mesh.positions {
                    for k in 0..3 {
                        let x = v[0] * w[k] + v[1] * w[4 + k] + v[2] * w[8 + k] + w[12 + k];
                        lo[k] = lo[k].min(x);
                        hi[k] = hi[k].max(x);
                    }
                }
                format!(
                    " {} {} v{} n{} uv{} c{} box=({:.1},{:.1})-({:.1},{:.1})",
                    m.id,
                    m.effect,
                    r.mesh.positions.len(),
                    r.mesh.normals.len(),
                    r.mesh.uvs.len(),
                    r.mesh.colors_argb.len(),
                    lo[0],
                    lo[1],
                    hi[0],
                    hi[1]
                )
            });
            let t = n.text.as_ref().map(|t| {
                format!(
                    " font={} h={:.2} ha={} va={} wrap={} track={:.2} case={} bounds={:?}",
                    t.font, t.height, t.h_align, t.v_align, t.wrapping, t.tracking, t.case, n.bounds
                )
            });
            let r = r.unwrap_or_default() + &t.unwrap_or_default();
            println!(
                "node {index} <{}> {}{} {} ({:.1},{:.1},{:.1}) s{:.2}{}",
                p.parent.map_or(-1, |q| q as i64),
                " ".repeat(p.depth),
                files[p.scene],
                n.id,
                p.world[12],
                p.world[13],
                p.world[14],
                (p.world[0] * p.world[0] + p.world[1] * p.world[1] + p.world[2] * p.world[2]).sqrt(),
                r
            );
        }
    }
    let mut effects: BTreeMap<String, usize> = BTreeMap::new();
    for (&(s, m), &count) in &by_material {
        let m = &scenes[s].materials[m];
        *effects.entry(m.effect.clone()).or_default() += count;
        println!("material {} {} x{} {:?}", m.id, m.effect, count, m.params);
    }
    println!("effects {effects:?}");
    // Ancestors of the root, with their animation channels.
    let mut chain = vec![root];
    while let Some(p) = s0.nodes[*chain.last().unwrap()].parent {
        chain.push(p);
    }
    println!("ancestors {:?}", chain.iter().map(|&i| &s0.nodes[i].id).collect::<Vec<_>>());
    for c in &s0.cameras {
        let n = &s0.nodes[c.node];
        let mut parents = Vec::new();
        let mut i = c.node;
        while let Some(p) = s0.nodes[i].parent {
            parents.push(s0.nodes[p].id.clone());
            i = p;
        }
        println!(
            "camera {} fov{:.2} near{} far{} eye({:.1},{:.1},{:.1}) parents{:?}",
            n.id, c.fov_degrees, c.near, c.far, c.world[12], c.world[13], c.world[14], parents
        );
    }
    let mut ids: Vec<String> = chain.iter().map(|&i| s0.nodes[i].id.clone()).collect();
    // SURVEY_SUBTREE: also the animations of every grafted node of the root's package.
    if std::env::var_os("SURVEY_SUBTREE").is_some() {
        ids.extend(placed.iter().filter(|p| p.scene == si).map(|p| s0.nodes[p.node].id.clone()));
    }
    // SURVEY_TARGET=substring: also the channels whose target contains it (material parameters).
    let extra = std::env::var("SURVEY_TARGET").ok();
    for a in &s0.animations {
        let hit: Vec<String> = a
            .channels
            .iter()
            .filter(|c| {
                ids.contains(&c.target)
                    || c.target.contains("pause")
                    || c.target.contains("camera=")
                    || extra.as_deref().is_some_and(|e| c.target.contains(e))
            })
            .map(|c| format!("{}[{:?}x{}]", c.target, c.semantic, c.components.len()))
            .collect();
        if !hit.is_empty() {
            let ranges: Vec<String> = a.ranges.iter().map(|r| format!("{}:{:.3}-{:.3}", r.name, r.start, r.end)).collect();
            println!("anim {} dur={:.3} ranges={:?} channels={:?}", a.owner(), a.duration(), ranges, hit);
            // SURVEY_KEYS=substring: the keys of the matching channels.
            if let Some(want) = std::env::var_os("SURVEY_KEYS") {
                let want = want.to_string_lossy().into_owned();
                for c in a.channels.iter().filter(|c| c.target.contains(&want)) {
                    for (i, curve) in c.components.iter().enumerate() {
                        let keys: Vec<String> = curve.0.iter().map(|k| format!("{:.3}:{:.4}", k.time, k.value)).collect();
                        println!("keys {} {:?}[{i}] {}", c.target, c.semantic, keys.join(" "));
                    }
                }
            }
        }
    }
}
