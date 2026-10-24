use grid_formats::{anim_packet, pssg};

// Authored schema and payloads only; no purchased resource bytes or identifiers.
const NAMES: &[&str] = &[
    "ROOT",
    "UNRELATED",
    "NeAnimPacket_L4",
    "NeAnimPacket_S4",
    "NeAnimPacket_B1",
    "NeAnimPacket_L4_KA",
    "NeAnimPacketData_L4",
    "NeAnimPacketData_S4",
    "NeAnimPacketData_B1",
    "NeAnimPacketData_L4_KA",
    "NeAnimPacketData_L1",
    "NeAnimPacketData_B4",
    "NeAnimPacketData_S4_KA",
    "NeAnimPacketData_UNOBSERVED",
    "DATABLOCKDATA",
    "NEANIMKEYS",
    "PNSTRING",
    "DATA",
];
const ATTRS: &[&str] = &["id", "size", "alignment"];
const PAIRS: &[(&str, &str, u32)] = &[
    ("NeAnimPacket_L4", "NeAnimPacketData_L4", 16),
    ("NeAnimPacket_S4", "NeAnimPacketData_S4", 16),
    ("NeAnimPacket_B1", "NeAnimPacketData_B1", 16),
    ("NeAnimPacket_L4_KA", "NeAnimPacketData_L4_KA", 128),
];
fn u(value: u32) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}
fn text(value: &str) -> Vec<u8> {
    let mut bytes = u(value.len() as u32);
    bytes.extend_from_slice(value.as_bytes());
    bytes
}
fn node(names: &[&str], name: &str, attrs: &[(&str, Vec<u8>)], payload: Vec<u8>) -> Vec<u8> {
    let mut block = Vec::new();
    for (key, value) in attrs {
        block.extend(u(ATTRS.iter().position(|v| v == key).unwrap() as u32 + 1));
        block.extend(u(value.len() as u32));
        block.extend(value);
    }
    let mut bytes = u(names.iter().position(|v| *v == name).unwrap() as u32 + 1);
    bytes.extend(u((4 + block.len() + payload.len()) as u32));
    bytes.extend(u(block.len() as u32));
    bytes.extend(block);
    bytes.extend(payload);
    bytes
}
fn file(names: &[&str], root: Vec<u8>) -> Vec<u8> {
    let mut bytes = b"PSSG".to_vec();
    bytes.extend(u(0));
    bytes.extend(u(ATTRS.len() as u32));
    bytes.extend(u(names.len() as u32));
    for (index, name) in names.iter().enumerate() {
        bytes.extend(u(index as u32 + 1));
        bytes.extend(text(name));
        // Attribute IDs are global, deliberately declared on ROOT only.
        let attrs = if *name == "ROOT" { ATTRS } else { &[] };
        bytes.extend(u(attrs.len() as u32));
        for (ai, attr) in attrs.iter().enumerate() {
            bytes.extend(u(ai as u32 + 1));
            bytes.extend(text(attr));
        }
    }
    bytes.extend(root);
    let size = bytes.len() as u32 - 8;
    bytes[4..8].copy_from_slice(&size.to_be_bytes());
    bytes
}
fn bank(names: &[&str], children: Vec<u8>) -> Vec<u8> {
    file(names, node(names, "ROOT", &[], children))
}
fn packet(
    names: &[&str],
    parent: &str,
    leaf: &str,
    raw: Vec<u8>,
    native: u32,
    alignment: u32,
) -> Vec<u8> {
    node(
        names,
        parent,
        &[
            ("id", text(&format!("synthetic-{parent}"))),
            ("size", u(native)),
            ("alignment", u(alignment)),
        ],
        node(names, leaf, &[], raw),
    )
}
fn ordinary_payload(leaf: &str) -> Vec<u8> {
    let mut bytes = u(1);
    bytes.extend(u(16));
    bytes.extend(text("joint"));
    bytes.extend(text(if leaf == "NeAnimPacketData_L4" {
        "translation"
    } else {
        "rotation_quaternion"
    }));
    for value in [2, 36, 48] {
        bytes.extend(u(value));
    }
    for time in [0.0f32, 1.0] {
        bytes.extend(u(time.to_bits()));
        for value in [1.0f32, -0.0, 3.0, 0.0] {
            bytes.extend(u(value.to_bits()));
        }
    }
    bytes
}
fn find(tree: &pssg::Tree<'_>, name: &[u8]) -> Vec<usize> {
    (0..tree.nodes.len())
        .filter(|index| tree.name(*index) == name)
        .collect()
}

#[test]
fn four_exact_pairs_preserve_raw_slices_and_global_metadata() {
    for reverse in [false, true] {
        let mut names = NAMES.to_vec();
        if reverse {
            names.reverse();
        }
        for &(parent, leaf, alignment) in PAIRS {
            let ordinary = matches!(leaf, "NeAnimPacketData_L4" | "NeAnimPacketData_S4");
            let raw = if ordinary {
                ordinary_payload(leaf)
            } else {
                vec![0xff, 0, 0x80]
            };
            let native = if ordinary { 80 } else { alignment };
            let bytes = bank(
                &names,
                packet(&names, parent, leaf, raw.clone(), native, alignment),
            );
            let tree = pssg::read_tree(&bytes).unwrap();
            let leaves = find(&tree, leaf.as_bytes());
            assert_eq!(leaves.len(), 1);
            let index = leaves[0];
            let n = &tree.nodes[index];
            let parent_index = n.parent.unwrap();
            assert_eq!(tree.name(parent_index), parent.as_bytes());
            assert_eq!(n.data, Some(raw.as_slice()));
            assert!(n.children.is_empty());
            assert!(std::ptr::eq(
                n.data.unwrap().as_ptr(),
                bytes[n.offset + 12..].as_ptr()
            ));
            assert_eq!(tree.integer(parent_index, b"size").unwrap(), native);
            assert_eq!(tree.integer(parent_index, b"alignment").unwrap(), alignment);
            assert_eq!(
                tree.string(parent_index, b"id").unwrap(),
                format!("synthetic-{parent}").as_bytes()
            );
            let envelope = anim_packet::Envelope {
                parent_type: parent,
                leaf_type: leaf,
                parent_alignment: tree.integer(parent_index, b"alignment").unwrap(),
                native_size: tree.integer(parent_index, b"size").unwrap() as usize,
            };
            if ordinary {
                assert_ne!(raw.len(), native as usize);
                let decoded = anim_packet::read(n.data.unwrap(), envelope).unwrap();
                assert_eq!(decoded.channels[0].target, "joint");
                assert_eq!(decoded.channels[0].keys.len(), 2);
                assert_eq!(decoded.channels[0].keys[0].value[1].to_bits(), 0x80000000);
            } else {
                assert_eq!(
                    anim_packet::read(n.data.unwrap(), envelope)
                        .unwrap_err()
                        .reason,
                    "unsupported or mismatched ordinary packet family"
                );
            }
        }
    }
}
#[test]
fn known_data_requires_exact_parent_even_when_payload_is_a_valid_container() {
    let inner = node(NAMES, "UNRELATED", &[], Vec::new());
    for &(parent, leaf, alignment) in PAIRS {
        let wrong = if parent == "NeAnimPacket_L4" {
            "NeAnimPacket_S4"
        } else {
            "NeAnimPacket_L4"
        };
        for actual in ["ROOT", "UNRELATED", wrong] {
            let bytes = bank(
                NAMES,
                packet(NAMES, actual, leaf, inner.clone(), 80, alignment),
            );
            assert_eq!(
                pssg::read_tree(&bytes).unwrap_err().message,
                "animation packet data has wrong parent"
            );
        }
        let bytes = file(NAMES, node(NAMES, leaf, &[], inner.clone()));
        assert_eq!(
            pssg::read_tree(&bytes).unwrap_err().message,
            "animation packet data has wrong parent"
        );
    }
}
#[test]
fn unobserved_animation_data_is_not_reclassified_as_raw_or_a_container() {
    for leaf in [
        "NeAnimPacketData_L1",
        "NeAnimPacketData_B4",
        "NeAnimPacketData_S4_KA",
        "NeAnimPacketData_UNOBSERVED",
    ] {
        for raw in [
            Vec::new(),
            vec![0xff, 0, 0x80],
            node(NAMES, "UNRELATED", &[], Vec::new()),
        ] {
            let bytes = bank(NAMES, packet(NAMES, "NeAnimPacket_L4", leaf, raw, 80, 16));
            assert_eq!(
                pssg::read_tree(&bytes).unwrap_err().message,
                "unsupported animation packet data family"
            );
        }
    }
}
#[test]
fn truncated_or_escaping_packet_frames_still_reject() {
    let bytes = bank(
        NAMES,
        packet(
            NAMES,
            "NeAnimPacket_L4",
            "NeAnimPacketData_L4",
            ordinary_payload("NeAnimPacketData_L4"),
            80,
            16,
        ),
    );
    for end in 0..bytes.len() {
        assert!(pssg::read_tree(&bytes[..end]).is_err(), "prefix {end}");
    }
    let tree = pssg::read_tree(&bytes).unwrap();
    let leaf = tree.nodes[find(&tree, b"NeAnimPacketData_L4")[0]].offset;
    for (offset, value) in [(leaf + 4, u32::MAX), (leaf + 8, u32::MAX), (leaf + 4, 3)] {
        let mut bad = bytes.clone();
        bad[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        assert!(pssg::read_tree(&bad).is_err());
    }
}
#[test]
fn parent_metadata_remains_checked_by_existing_tree_helpers() {
    let raw = ordinary_payload("NeAnimPacketData_L4");
    let duplicate = node(
        NAMES,
        "NeAnimPacket_L4",
        &[("size", u(80)), ("size", u(80))],
        node(NAMES, "NeAnimPacketData_L4", &[], raw.clone()),
    );
    assert!(pssg::read_tree(&bank(NAMES, duplicate)).is_err());
    let bad = node(
        NAMES,
        "NeAnimPacket_L4",
        &[("size", vec![0, 0, 80]), ("alignment", u(16))],
        node(NAMES, "NeAnimPacketData_L4", &[], raw),
    );
    let bytes = bank(NAMES, bad);
    let tree = pssg::read_tree(&bytes).unwrap();
    let parent = find(&tree, b"NeAnimPacket_L4")[0];
    assert!(tree.integer(parent, b"size").is_err());
    assert!(tree.string(parent, b"id").is_err());
}
#[test]
fn mixed_raw_population_is_retained_without_claiming_all_codecs_decode() {
    let mut children = Vec::new();
    for &(parent, leaf, alignment) in PAIRS {
        let ordinary = matches!(leaf, "NeAnimPacketData_L4" | "NeAnimPacketData_S4");
        children.extend(packet(
            NAMES,
            parent,
            leaf,
            if ordinary {
                ordinary_payload(leaf)
            } else {
                vec![0xff, 0, 0x80]
            },
            if ordinary { 80 } else { alignment },
            alignment,
        ));
    }
    let bytes = bank(NAMES, children);
    let tree = pssg::read_tree(&bytes).unwrap();
    let mut decoded = 0;
    let mut opaque = 0;
    for &(parent, leaf, alignment) in PAIRS {
        let leaves = find(&tree, leaf.as_bytes());
        assert_eq!(leaves.len(), 1);
        let n = &tree.nodes[leaves[0]];
        let native = tree.integer(n.parent.unwrap(), b"size").unwrap() as usize;
        match anim_packet::read(
            n.data.unwrap(),
            anim_packet::Envelope {
                parent_type: parent,
                leaf_type: leaf,
                parent_alignment: alignment,
                native_size: native,
            },
        ) {
            Ok(_) => decoded += 1,
            Err(error) => {
                assert_eq!(
                    error.reason,
                    "unsupported or mismatched ordinary packet family"
                );
                opaque += 1;
            }
        }
    }
    assert_eq!((decoded, opaque), (2, 2));
    assert_eq!(tree.nodes[0].children.len(), 4);
}
#[test]
fn unrelated_containers_and_legacy_raw_rules_are_unchanged() {
    for name in ["DATABLOCKDATA", "NEANIMKEYS"] {
        let bytes = bank(NAMES, node(NAMES, name, &[], vec![0xff, 0, 0x80]));
        let tree = pssg::read_tree(&bytes).unwrap();
        assert_eq!(
            tree.nodes[find(&tree, name.as_bytes())[0]].data,
            Some([0xff, 0, 0x80].as_slice())
        );
    }
    let bytes = bank(
        NAMES,
        node(NAMES, "PNSTRING", &[], node(NAMES, "DATA", &[], vec![0xff])),
    );
    let tree = pssg::read_tree(&bytes).unwrap();
    assert_eq!(
        tree.nodes[find(&tree, b"DATA")[0]].data,
        Some([0xff].as_slice())
    );
    for name in ["DATA", "UNRELATED"] {
        let bytes = bank(NAMES, node(NAMES, name, &[], Vec::new()));
        let tree = pssg::read_tree(&bytes).unwrap();
        assert!(tree.nodes[find(&tree, name.as_bytes())[0]].data.is_none());
    }
}
