//! Research probe: every PSSG element with an `id`, as (index, element, id, parent).
//! `grid-pssg-ids FILE`.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("usage: grid-pssg-ids FILE")?;
    let bytes = std::fs::read(path)?;
    let tree = grid_formats::pssg::read_tree(&bytes)?;
    let mut parent = vec![None; tree.nodes.len()];
    for (i, n) in tree.nodes.iter().enumerate() {
        for &c in &n.children {
            parent[c] = Some(i);
        }
    }
    for (i, up) in parent.iter().enumerate() {
        if let Ok(id) = tree.string(i, b"id") {
            let up = up.and_then(|p| tree.string(p, b"id").ok()).unwrap_or(b"-");
            println!(
                "{i} {} {} parent={}",
                String::from_utf8_lossy(tree.name(i)),
                String::from_utf8_lossy(id),
                String::from_utf8_lossy(up)
            );
        }
    }
    Ok(())
}
