//! Prints a PSSG tree (element names and attributes) for research.
//!
//! Usage: grid-pssg-tree FILE [FILTER] [MAX]
//! Bounded resource diagnostic: grid-pssg-tree --check-l4-packet234 FILE PRIVATE_OUTDIR
//! With FILTER, prints only the subtrees whose element name, or any attribute
//! value, contains FILTER; at most MAX of them (default 20).
use grid_formats::{anim_packet, mesh, pssg};
use std::{fs, io::Write};

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
    if args.get(1).is_some_and(|s| s == "--check-crowd-skin-female-c") {
        if args.len() != 4 { return Err("usage: grid-pssg-tree --check-crowd-skin-female-c FILE PRIVATE_OUTDIR".into()); }
        return check_crowd_skin_female_c(&args[2], &args[3]);
    }
    if args.get(1).is_some_and(|s| s == "--check-l4-packet234") {
        if args.len() != 4 { return Err("usage: grid-pssg-tree --check-l4-packet234 FILE PRIVATE_OUTDIR".into()); }
        return check_l4_packet234(&args[2], &args[3]);
    }
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

// Explicit bank coordinates only; no clock, pose or native x87 parity.
// Strict finite domain is diagnostic policy, not resource admission or native errors.
fn sample_l4(packet: &anim_packet::Packet<'_>, channel: usize, query: f32)
    -> Result<[f32; 4], &'static str>
{
    if packet.kind != anim_packet::Kind::L4 { return Err("not ordinary L4"); }
    let keys = &packet.channels.get(channel).ok_or("missing channel")?.keys;
    if keys.is_empty() || keys.len() > 100_000 || !query.is_finite() {
        return Err("key count or nonfinite query");
    }
    for (index, key) in keys.iter().enumerate() {
        if !key.time.is_finite() || !key.value.iter().all(|v| v.is_finite()) {
            return Err("nonfinite saved key");
        }
        if index > 0 && (key.time <= keys[index-1].time
            || !(key.time-keys[index-1].time).is_finite()) {
            return Err("duplicate/reversed/nonfinite span unsupported");
        }
    }
    if keys.len() == 1 { return Ok(keys[0].value); }
    let (mut left, mut right) = (0, keys.len()-1);
    while right-left > 1 {
        let mid = left+(right-left)/2;
        if query <= keys[mid].time { right=mid; } else { left=mid; }
    }
    let numerator = query-keys[left].time;
    let fraction = numerator/(keys[right].time-keys[left].time);
    if !numerator.is_finite() || !fraction.is_finite() {
        return Err("nonfinite interval arithmetic unsupported");
    }
    let fraction = fraction.clamp(0.0, 1.0);
    let left_weight = 1.0f32-fraction;
    let mut value = [0.0; 4];
    for (lane, output) in value.iter_mut().enumerate() {
        let a = keys[left].value[lane]*left_weight;
        let b = keys[right].value[lane]*fraction;
        *output = a+b;
        if !output.is_finite() { return Err("nonfinite L4 output unsupported"); }
    }
    Ok(value)
}

fn words(packet: &anim_packet::Packet<'_>) -> Vec<u8> {
    let mut out = Vec::new();
    for key in &packet.channels[0].keys {
        for value in [key.time, key.value[0], key.value[1], key.value[2], key.value[3]] {
            out.extend(value.to_bits().to_le_bytes());
        }
    }
    out
}
fn check_l4_packet234(path: &str, output: &str) -> Result<(), Box<dyn std::error::Error>> {
    let bank = fs::read(path)?;
    if bank.len() != 1_389_508 { return Err("bank size differs".into()); }
    // Use the linked production Tree and exact bank-local ID, not a private parser.
    let tree = pssg::read_tree(&bank)?;
    let selected: Vec<_> = (0..tree.nodes.len())
        .filter(|&i| tree.string(i, b"id").is_ok_and(|id| id == b"packet234"))
        .collect();
    if selected.len() != 1 { return Err("packet234 ID missing/ambiguous".into()); }
    let parent = selected[0];
    if tree.name(parent) != b"NeAnimPacket_L4" || tree.nodes[parent].children.len() != 1 {
        return Err("packet234 parent/family differs".into());
    }
    let leaf_index = tree.nodes[parent].children[0];
    let leaf = &tree.nodes[leaf_index];
    if tree.name(leaf_index) != b"NeAnimPacketData_L4" || leaf.parent != Some(parent)
        || !leaf.children.is_empty() || !leaf.attributes.is_empty() {
        return Err("packet234 direct raw leaf differs".into());
    }
    let payload = leaf.data.ok_or("packet234 raw payload absent")?;
    let offset = (payload.as_ptr() as usize).checked_sub(bank.as_ptr() as usize)
        .ok_or("packet234 borrow range")?;
    let native_size = tree.integer(parent, b"size")? as usize;
    let parent_alignment = tree.integer(parent, b"alignment")?;
    if offset != 945_808 || payload.len() != 1025 || leaf.offset != 945_796
        || native_size != 1024 || parent_alignment != 16 {
        return Err("packet234 known provenance differs".into());
    }
    let packet = anim_packet::read(payload, anim_packet::Envelope {
        parent_type: "NeAnimPacket_L4", leaf_type: "NeAnimPacketData_L4",
        parent_alignment, native_size,
    })?;
    if packet.kind != anim_packet::Kind::L4 || packet.channels.len() != 1
        || packet.channels[0].keys.len() != 49 || packet.native_size != 1024 {
        return Err("exact packet234 contract differs".into());
    }
    let before = words(&packet);
    let mut queries = Vec::new();
    let keys = &packet.channels[0].keys;
    for key in keys {
        queries.extend([key.time, key.time.next_down(), key.time.next_up()]);
    }
    for pair in keys.windows(2) {
        let query = pair[0].time + (pair[1].time - pair[0].time) * 0.25;
        if !(query > pair[0].time && query < pair[1].time) {
            return Err("quarter sample not strictly interior; no repair/skip".into());
        }
        queries.push(query);
    }
    if queries.len() != 195 { return Err("sample count differs".into()); }
    let out = std::path::Path::new(output);
    let mut rows = fs::OpenOptions::new().create_new(true).write(true).open(out.join("samples.tsv"))?;
    writeln!(rows, "L4SLOT\t1\tbank-coordinates-only\tno-native-parity")?;
    for (sample, query) in queries.iter().copied().enumerate() {
        let actual = sample_l4(&packet, 0, query)
            .map_err(|e| format!("sample{sample} refusal {e:?}"))?;
        // Independent LINEAR interval oracle versus bracket's binary search.
        let left = keys.windows(2).position(|pair| query <= pair[1].time).unwrap_or(keys.len()-2);
        let raw = (query - keys[left].time) / (keys[left+1].time - keys[left].time);
        let fraction = raw.clamp(0.0, 1.0);
        let left_weight = 1.0f32 - fraction;
        let expected: [f32;4] = std::array::from_fn(|lane| {
            let a = keys[left].value[lane] * left_weight;
            let b = keys[left+1].value[lane] * fraction;
            a+b
        });
        if actual.map(f32::to_bits) != expected.map(f32::to_bits) {
            return Err(format!("sample{sample} L4 raw arithmetic differs").into());
        }
        writeln!(rows, "SAMPLE\t{sample}\t{:08x}\t{left}\t{:08x}\t{:08x}\t{:08x}\t{:08x}\t{:08x}",
            query.to_bits(), fraction.to_bits(), actual[0].to_bits(), actual[1].to_bits(), actual[2].to_bits(), actual[3].to_bits())?;
    }
    if before != words(&packet) { return Err("input float words changed".into()); }
    let mut raw = fs::OpenOptions::new().create_new(true).write(true).open(out.join("input-words.bin"))?;
    raw.write_all(&before)?;
    writeln!(rows, "SUMMARY\tL4\t1\t49\t195\tunchanged")?;
    rows.flush()?;
    println!("ONE_PACKET234_L4 channels=1 keys=49 queries=195 input_words_unchanged=true native_clock_pose_parity=false");
    Ok(())
}

// One purchased CPU fixture only; no pose, LOD selection or skinning.
fn check_crowd_skin_female_c(path: &str, output: &str) -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::Path::new(output).canonicalize()?;
    if !out.starts_with(std::path::Path::new("local").canonicalize()?) {
        return Err("private local output directory required".into());
    }
    if fs::metadata(path)?.len() != 848_496 { return Err("pinned female rig size differs".into()); }
    let bytes = fs::read(path)?;
    let tree = pssg::read_tree(&bytes)?;
    let one_id = |id: &[u8]| -> Result<usize, &'static str> {
        let mut matches = (0..tree.nodes.len()).filter(|&i| tree.string(i, b"id").is_ok_and(|v| v == id));
        let found = matches.next().ok_or("selected ID absent")?;
        if matches.next().is_some() { return Err("selected ID ambiguous"); }
        Ok(found)
    };
    let child = |parent: usize, name: &[u8]| -> Result<usize, &'static str> {
        let mut matches = tree.nodes[parent].children.iter().copied().filter(|&i| tree.name(i) == name);
        let found = matches.next().ok_or("selected child absent")?;
        if matches.next().is_some() { return Err("selected child ambiguous"); }
        Ok(found)
    };
    let selected = mesh::read_four_weight_mesh(&tree, b"!C")?;
    let geometry = &selected.mesh;
    let influences = &selected.influences;
    if geometry.id != b"!C" || geometry.positions.len() != 786
        || influences.weights.len() != 786 || influences.joints.len() != 786 || geometry.skin_indices.is_some() {
        return Err("selected four-weight source/count/representation differs".into());
    }
    let skin = one_id(b"f_US_lod0")?;
    let skeleton = one_id(b"!A")?;
    if tree.name(skin) != b"SKINNODE" || tree.string(skin, b"skeleton")? != b"#!A"
        || tree.name(skeleton) != b"SKELETON" || tree.integer(skin, b"jointCount")? != 23
        || tree.integer(skeleton, b"matrixCount")? != 23 {
        return Err("selected skin/skeleton differs".into());
    }
    let joints: Vec<_> = tree.nodes[skin].children.iter().copied()
        .filter(|&i| tree.name(i) == b"SKINJOINT").collect();
    let inverse: Vec<_> = tree.nodes[skeleton].children.iter().copied()
        .filter(|&i| tree.name(i) == b"INVERSEBINDMATRIX").collect();
    if joints.len() != 23 || inverse.len() != 23
        || inverse.iter().any(|&i| tree.nodes[i].data.is_none_or(|b| b.len() != 64))
        || influences.joints.iter().flatten().any(|&j| j >= 23) {
        return Err("palette/reference/count/range differs".into());
    }
    let index_source = child(one_id(b"!C")?, b"RENDERINDEXSOURCE")?;
    let declared_count = tree.integer(index_source, b"count")? as usize;
    let declared_max = tree.integer(index_source, b"maximumIndex")?;
    if geometry.indices.len() != declared_count { return Err("geometry index count differs".into()); }
    let mut rows = fs::OpenOptions::new().create_new(true).write(true).open(out.join("actual.tsv"))?;
    writeln!(rows, "SKIN4\t1\t!C\t786\t23\t23")?;
    writeln!(rows, "GEOMETRY\t{}\t{}\t{}", geometry.indices.len(),
        geometry.indices.iter().copied().max().ok_or("empty geometry")?, declared_max)?;
    for (index, &entry) in joints.iter().enumerate() {
        let target = tree.string(entry, b"joint")?.strip_prefix(b"#").ok_or("nonlocal palette reference")?;
        if tree.name(one_id(target)?) != b"JOINTNODE" || target.contains(&b'\t') || target.contains(&b'\n') {
            return Err("palette target type/text differs".into());
        }
        writeln!(rows, "PALETTE\t{index}\t{}", std::str::from_utf8(target)?)?;
    }
    for index in 0..786 {
        let p = geometry.positions[index].map(f32::to_bits);
        let w = influences.weights[index].map(f32::to_bits);
        let j = influences.joints[index];
        writeln!(rows, "VERTEX\t{index}\t{:08x}\t{:08x}\t{:08x}\t{:08x}\t{:08x}\t{:08x}\t{:08x}\t{:02x}\t{:02x}\t{:02x}\t{:02x}",
            p[0],p[1],p[2],w[0],w[1],w[2],w[3],j[0],j[1],j[2],j[3])?;
    }
    for (index, value) in geometry.indices.iter().enumerate() { writeln!(rows, "INDEX\t{index}\t{value}")?; }
    writeln!(rows, "SUMMARY\t786\t3144\t3144\t23\t{}", geometry.indices.len())?;
    rows.flush()?;
    println!("ONE_FEMALE_C_SKIN4 vertices=786 weight_words=3144 joint_bytes=3144 palette=23 indices={} no_pose_clock_lod_claim=true", geometry.indices.len());
    Ok(())
}
