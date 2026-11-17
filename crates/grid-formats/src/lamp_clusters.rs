//! Source lamp semantics and the native light-cluster LOD0 anchors.
use crate::{Result, fail, pssg::Tree};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Front,
    Rear,
    Brake,
    Reverse,
    Combi,
}

/// Semantic members (part ID, role, physical cluster, exact joint suffix).
pub fn catalogue() -> Vec<(u16, Role, u8, String)> {
    let mut out = Vec::with_capacity(60);
    for (side_index, side) in ["fl", "fr"].into_iter().enumerate() {
        for (variant, prefix) in ["", "2_", "3_"].into_iter().enumerate() {
            for (member, surface) in ["pod", "glass"].into_iter().enumerate() {
                out.push((
                    107 + (side_index * 6 + variant * 2 + member) as u16,
                    Role::Front,
                    (side_index * 3 + variant) as u8,
                    format!("light_{prefix}{surface}_{side}"),
                ));
            }
        }
    }
    for (side_index, side) in ["bl", "br"].into_iter().enumerate() {
        for (variant, prefix) in ["", "2_", "3_"].into_iter().enumerate() {
            for (channel, (kind, role)) in [
                ("", Role::Rear),
                ("brake_", Role::Brake),
                ("rev_", Role::Reverse),
                ("combi_", Role::Combi),
            ]
            .into_iter()
            .enumerate()
            {
                for (member, surface) in ["pod", "glass"].into_iter().enumerate() {
                    out.push((
                        119 + (side_index * 24 + variant * 8 + channel * 2 + member) as u16,
                        role,
                        (6 + variant * 2 + side_index) as u8,
                        format!("light_{prefix}{kind}{surface}_{side}"),
                    ));
                }
            }
        }
    }
    out
}

/// Midpoints of saved LOD0 lamp-node bounds for each native cluster.
/// Original 0x9A5DF0 selects semantic LOD zero and reads the bounds directly.
/// A low-LOD member may share an admitted cluster but does not authorize an anchor.
pub fn lod0_anchors(tree: &Tree<'_>) -> Result<[Option<[f32; 3]>; 12]> {
    let bundles: Vec<_> = (0..tree.nodes.len())
        .filter(|&node| {
            tree.name(node) == b"MATRIXPALETTEBUNDLENODE"
                && tree.string(node, b"id").is_ok_and(|id| id == b"LOD0_")
        })
        .collect();
    let [bundle] = bundles.as_slice() else {
        return fail(0, "expected one LOD0 lamp bundle");
    };

    let mut members = catalogue();
    members.sort_by_key(|&(semantic, _, cluster, _)| {
        // The rear source sums all four pod roles before all four glass roles.
        let order = if semantic <= 118 {
            (semantic - 107) % 2
        } else {
            let member = (semantic - 119) % 8;
            (member % 2) * 4 + member / 2
        };
        (cluster, order)
    });
    let full_names: Vec<_> = members
        .iter()
        .map(|(_, _, _, name)| format!("x0_{name}"))
        .collect();
    let ids: BTreeMap<_, _> = full_names
        .iter()
        .enumerate()
        .map(|(i, name)| (name.as_bytes(), i))
        .collect();
    let mut nodes = [None; 60];
    let mut pending = tree.nodes[*bundle].children.clone();
    while let Some(node) = pending.pop() {
        pending.extend_from_slice(&tree.nodes[node].children);
        let Ok(name) = tree.string(node, b"id") else {
            continue;
        };
        if let Some(&i) = ids.get(name)
            && nodes[i].replace(node).is_some()
        {
            return fail(tree.nodes[node].offset, "ambiguous LOD0 lamp joint");
        }
    }

    let mut sums = [[0_f32; 3]; 12];
    let mut counts = [0_u8; 12];
    for ((_, _, cluster, _), node) in members.into_iter().zip(nodes) {
        let Some(node) = node else { continue };
        if tree.name(node) != b"MATRIXPALETTEJOINTNODE" {
            continue;
        }
        let bounds: Vec<_> = tree.nodes[node]
            .children
            .iter()
            .copied()
            .filter(|&child| tree.name(child) == b"BOUNDINGBOX")
            .collect();
        let [bounds] = bounds.as_slice() else {
            return fail(tree.nodes[node].offset, "expected one source lamp bound");
        };
        let Some(raw) = tree.nodes[*bounds].data.filter(|data| data.len() == 24) else {
            return fail(tree.nodes[node].offset, "invalid source lamp bounds size");
        };
        let limits: [f32; 6] = std::array::from_fn(|i| {
            f32::from_be_bytes(raw[i * 4..i * 4 + 4].try_into().unwrap())
        });
        if limits.iter().any(|v| !v.is_finite() || v.abs() > 1000.)
            || (0..3).any(|axis| limits[axis] > limits[axis + 3])
        {
            return fail(tree.nodes[node].offset, "invalid source lamp bounds");
        }
        let index = usize::from(cluster);
        counts[index] += 1;
        for axis in 0..3 {
            sums[index][axis] += (limits[axis] + limits[axis + 3]) * 0.5;
        }
    }
    Ok(std::array::from_fn(|i| {
        (counts[i] != 0).then(|| sums[i].map(|sum| sum / f32::from(counts[i])))
    }))
}
