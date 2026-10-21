//! Prints a PSSG tree (element names and attributes) for research.
//!
//! Usage: grid-pssg-tree FILE [FILTER] [MAX]
//! With FILTER, prints only the subtrees whose element name, or any attribute
//! value, contains FILTER; at most MAX of them (default 20).
use grid_formats::pssg;

fn value(v: &[u8]) -> String {
    // A length-prefixed string, else four-byte words.
    if v.len() >= 4 && u32::from_be_bytes(v[..4].try_into().unwrap()) as usize == v.len() - 4 {
        return format!("{:?}", String::from_utf8_lossy(&v[4..]));
    }
    if v.len() == 4 {
        let w = u32::from_be_bytes(v.try_into().unwrap());
        return format!("{w:#x}/{}", f32::from_bits(w));
    }
    format!("<{} bytes>", v.len())
}

fn print(tree: &pssg::Tree<'_>, node: usize, depth: usize) {
    let n = &tree.nodes[node];
    let mut line = format!("{}{}", "  ".repeat(depth), String::from_utf8_lossy(tree.name(node)));
    for a in &n.attributes {
        let name = tree
            .framing
            .schema
            .iter()
            .flat_map(|e| &e.attributes)
            .find(|d| d.id == a.id)
            .map_or("?".into(), |d| String::from_utf8_lossy(d.name).into_owned());
        line += &format!(" {name}={}", value(a.value));
    }
    println!("{line}");
    for &c in &n.children {
        print(tree, c, depth + 1);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let path = args.get(1).ok_or("usage: grid-pssg-tree FILE [FILTER] [MAX]")?;
    let bytes = std::fs::read(path)?;
    let tree = pssg::read_tree(&bytes)?;
    let Some(filter) = args.get(2) else {
        print(&tree, 0, 0);
        return Ok(());
    };
    let max: usize = args.get(3).map_or(Ok(20), |m| m.parse())?;
    let filter = filter.as_bytes();
    let contains = |s: &[u8]| s.windows(filter.len()).any(|w| w == filter);
    let mut shown = 0;
    for i in 0..tree.nodes.len() {
        let hit = contains(tree.name(i))
            || tree.nodes[i].attributes.iter().any(|a| contains(a.value));
        // Skip nodes inside an already printed subtree.
        let mut p = tree.nodes[i].parent;
        let mut inside = false;
        while let Some(q) = p {
            if contains(tree.name(q)) || tree.nodes[q].attributes.iter().any(|a| contains(a.value)) {
                inside = true;
                break;
            }
            p = tree.nodes[q].parent;
        }
        if hit && !inside {
            print(&tree, i, 0);
            shown += 1;
            if shown == max {
                break;
            }
        }
    }
    Ok(())
}
