//! Private, read-only report of route entity renderables that `grid-app`'s
//! `append_entities` finds without draws (startup renderables holding only
//! anchors): per route, each such renderable's instance count, whether the
//! `<stem> Root` preview exists, and its node tree (element names and ids,
//! a few levels). Output under ignored `local/`.
use grid_formats::{entities, mesh, pssg, scene};
use grid_tools::route::Route;
use std::{collections::BTreeMap, io::Write, path::Path};

/// Instances of one renderable, and how many of them each template places.
type Uses<'a> = (usize, BTreeMap<&'a [u8], usize>);

fn read(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(std::fs::read(path)?)
}

fn dump(tree: &pssg::Tree<'_>, node: usize, depth: usize, out: &mut String) {
    if depth > 4 {
        return;
    }
    let id = tree.string(node, b"id").map(String::from_utf8_lossy).unwrap_or_default();
    out.push_str(&format!(
        "{}{} {}\n",
        "  ".repeat(depth + 1),
        String::from_utf8_lossy(tree.name(node)),
        id
    ));
    for &child in tree.nodes[node].children.iter().take(64) {
        dump(tree, child, depth + 1, out);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 3 {
        return Err("usage: grid-empty-renderables GAME_ROOT local/OUT.txt ROUTE...".into());
    }
    let root = Path::new(&args[0]);
    let out_path = Path::new(&args[1]);
    if !out_path.starts_with("local") {
        return Err("output must be under local/".into());
    }
    let mut out = String::new();
    for route_name in &args[2..] {
        let route = Route::new(route_name)?;
        let bytes = read(&root.join(route.track_file("objects.pssg")))?;
        let tree = pssg::read_tree(&bytes)?;
        let sources = mesh::read_object_meshes(&tree)?;
        let placement_bytes = read(&root.join(route.file("objects.ens")))?;
        let type_bytes = read(&root.join(route.track_file("objecttypes.pssg")))?;
        let placement_tree = pssg::read_tree(&placement_bytes)?;
        let type_tree = pssg::read_tree(&type_bytes)?;
        let entities = entities::read(&placement_tree, &type_tree)?;
        let mut placements: BTreeMap<&[u8], Uses> = BTreeMap::new();
        for entity in &entities {
            for renderable in &entity.renderables {
                let e = placements.entry(renderable.node).or_default();
                e.0 += 1;
                *e.1.entry(entity.template).or_default() += 1;
            }
        }
        if let Ok(near) = std::env::var("GRID_NEAR") {
            let v: Vec<f32> = near.split(',').filter_map(|s| s.parse().ok()).collect();
            let [x, z, r] = v[..] else {
                return Err("GRID_NEAR=X,Z,RADIUS".into());
            };
            for entity in &entities {
                let t = &entity.transform;
                if (t[12] - x).hypot(t[14] - z) > r {
                    continue;
                }
                // Each renderable: its node, body node and shape centres.
                let nodes: Vec<_> = (entity.renderables.iter())
                    .map(|n| {
                        let shapes: Vec<_> = (n.shapes.iter())
                            .map(|s| format!("({:.3}, {:.3}, {:.3})", s.transform[12], s.transform[13], s.transform[14]))
                            .collect();
                        format!(
                            "{} body_node={} shapes={}",
                            String::from_utf8_lossy(n.node),
                            String::from_utf8_lossy(n.body_node),
                            shapes.join(" ")
                        )
                    })
                    .collect();
                out.push_str(&format!(
                    "near {} {} at ({:.1}, {:.1}, {:.1}) renderables={nodes:?}\n",
                    String::from_utf8_lossy(entity.template),
                    String::from_utf8_lossy(entity.id),
                    t[12],
                    t[13],
                    t[14]
                ));
            }
        }
        if let Ok(find) = std::env::var("GRID_FIND") {
            for i in 0..tree.nodes.len() {
                let Ok(id) = tree.string(i, b"id") else { continue };
                if !String::from_utf8_lossy(id).contains(&find) {
                    continue;
                }
                let mut chain = Vec::new();
                let mut p = tree.nodes[i].parent;
                while let Some(q) = p {
                    chain.push(format!(
                        "{}:{}",
                        String::from_utf8_lossy(tree.name(q)),
                        tree.string(q, b"id").map(String::from_utf8_lossy).unwrap_or_default()
                    ));
                    p = tree.nodes[q].parent;
                }
                out.push_str(&format!(
                    "find {} {} <- {}\n",
                    String::from_utf8_lossy(tree.name(i)),
                    String::from_utf8_lossy(id),
                    chain.join(" <- ")
                ));
                dump(&tree, i, 0, &mut out);
            }
        }
        let closest = scene::read_object_draws(&tree, &sources)?;
        let renderables = scene::Renderables::new(&tree, &sources)?;
        let (mut empty, mut previewed, mut omitted, mut rooted) = (0, 0, 0, 0);
        let mut detail = String::new();
        for (node, (count, templates)) in &placements {
            if !renderables.draws(node)?.is_empty() {
                continue;
            }
            empty += count;
            let parts: Vec<&[u8]> = node.rsplitn(3, |&c| c == b'_').collect();
            let stem = (parts.len() == 3 && parts[1] == b"default")
                .then(|| [parts[2], b" Root".as_slice()].concat());
            let preview = stem
                .as_ref()
                .is_some_and(|s| closest.iter().any(|d| d.prototype == Some(s.as_slice())));
            if preview {
                previewed += count;
            } else {
                omitted += count;
            }
            detail.push_str(&format!(
                "  {} instances={count} preview={preview} templates={:?}\n",
                String::from_utf8_lossy(node),
                templates
                    .iter()
                    .map(|(t, n)| format!("{}:{n}", String::from_utf8_lossy(t)))
                    .collect::<Vec<_>>()
            ));
            for i in 0..tree.nodes.len() {
                if !tree.string(i, b"id").is_ok_and(|id| id == *node) {
                    continue;
                }
                // The enclosing ROOTNODE and the transforms on the way.
                let mut chain = Vec::new();
                let mut root = None;
                let mut a = Some(i);
                while let Some(k) = a {
                    if let Ok(m) = scene::matrix(&tree, k) {
                        chain.push(format!(
                            "{}:{:?}",
                            String::from_utf8_lossy(tree.name(k)),
                            [m[12], m[13], m[14], m[0], m[5], m[10]]
                        ));
                    }
                    if tree.name(k) == b"ROOTNODE" {
                        root = Some(k);
                        break;
                    }
                    a = tree.nodes[k].parent;
                }
                let root_id = root.and_then(|r| tree.string(r, b"id").ok());
                let root_draws = root_id
                    .map(|r| closest.iter().filter(|d| d.prototype == Some(r)).count())
                    .unwrap_or(0);
                if root_draws > 0 {
                    rooted += count;
                }
                detail.push_str(&format!(
                    "    root={:?} root_draws={root_draws} chain={}\n",
                    root_id.map(String::from_utf8_lossy),
                    chain.join(" / ")
                ));
                if let Some(r) = root {
                    for &c in &tree.nodes[r].children {
                        if (tree.name(c) == b"LODVISIBLERENDERNODE" || tree.name(c) == b"RENDERNODE")
                            && let Ok(m) = scene::matrix(&tree, c)
                        {
                            detail.push_str(&format!(
                                "    lod {} {:?}\n",
                                String::from_utf8_lossy(tree.name(c)),
                                [m[12], m[13], m[14], m[0], m[5], m[10]]
                            ));
                        }
                    }
                }
            }
        }
        out.push_str(&format!(
            "{route_name} entities={} empty_instances={empty} previewed={previewed} omitted={omitted} rooted={rooted}\n{detail}",
            entities.len()
        ));
    }
    std::fs::File::create(out_path)?.write_all(out.as_bytes())?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("grid-empty-renderables: {error}");
        std::process::exit(1);
    }
}
