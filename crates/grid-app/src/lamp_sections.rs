//! Exact source semantic-to-rigid-draw join; no activation, emission or native topology claim.
use grid_formats::{mesh, pssg, scene};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Role {
    Front,
    Rear,
    Brake,
    Reverse,
    Combi,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Section {
    pub semantic: u16,
    pub palette: u32,
    pub role: Role,
    pub cluster: u8,
    pub lod: u8,
    pub node: usize,
    pub bundle: String,
    pub draw: String,
    pub material: String,
    pub joint: String,
}
fn catalogue() -> Vec<(u16, Role, u8, String)> {
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
fn decode_lod(bundle: &[u8]) -> Result<u8, String> {
    // Explicit selected-bundle contract, not native grouping/root inference.
    match bundle {
        [b'L', b'O', b'D', n, b'_'] if (b'0'..=b'7').contains(n) => Ok(n - b'0'),
        _ => Err("lamp mapper requires explicit supported LOD0_..LOD7_ bundle".into()),
    }
}
fn inside(tree: &pssg::Tree<'_>, mut node: usize, root: usize) -> bool {
    loop {
        if node == root {
            return true;
        }
        match tree.nodes[node].parent {
            Some(p) => node = p,
            None => return false,
        }
    }
}
pub(crate) fn read(
    tree: &pssg::Tree<'_>,
    meshes: &[mesh::Mesh<'_>],
    draws: &[scene::Draw<'_>],
    bundle: &[u8],
) -> Result<Vec<Section>, String> {
    let lod = decode_lod(bundle)?;
    let bundles: Vec<_> = (0..tree.nodes.len())
        .filter(|&i| {
            tree.name(i) == b"MATRIXPALETTEBUNDLENODE"
                && tree.string(i, b"id").is_ok_and(|id| id == bundle)
        })
        .collect();
    let [root] = bundles.as_slice() else {
        return Err("expected one actual selected lamp bundle".into());
    };
    let mut selected = BTreeMap::new();
    for (semantic, role, cluster, name) in catalogue() {
        let full = format!("x{lod}_{name}");
        let nodes: Vec<_> = (0..tree.nodes.len())
            .filter(|&i| {
                inside(tree, i, *root)
                    && tree.string(i, b"id").is_ok_and(|id| id == full.as_bytes())
            })
            .collect();
        let node = match nodes.as_slice() {
            [] => continue,
            [node] => *node,
            _ => {
                return Err(format!(
                    "duplicate exact lamp node {full:?}; native traversal order not substituted"
                ));
            }
        };
        if tree.name(node) != b"MATRIXPALETTEJOINTNODE" {
            return Err(format!("exact lamp name has unsupported class: {full:?}"));
        }
        let palette = tree.integer(node, b"jointID").map_err(|e| e.to_string())?;
        if palette > 65535 {
            return Err("lamp palette exceeds accepted rigid domain".into());
        }
        selected.insert(full, (semantic, role, cluster, node, palette));
    }
    let mut out = Vec::new();
    for draw in draws {
        let joint = std::str::from_utf8(draw.joint).map_err(|e| e.to_string())?;
        let Some(&(semantic, role, cluster, node, palette)) = selected.get(joint) else {
            continue;
        };
        let mesh = meshes.get(draw.source).ok_or("lamp draw source missing")?;
        let skin = mesh
            .skin_indices
            .as_ref()
            .ok_or("lamp rigid skin indices absent")?
            .get(draw.vertices.clone())
            .ok_or("lamp draw vertex range invalid")?;
        if skin.is_empty() || skin.iter().any(|&v| v != palette as f32) {
            return Err("lamp source palette disagrees with actual rigid draw".into());
        }
        out.push(Section {
            semantic,
            palette,
            role,
            cluster,
            lod,
            node,
            bundle: String::from_utf8(bundle.to_vec()).map_err(|e| e.to_string())?,
            draw: String::from_utf8(draw.id.to_vec()).map_err(|e| e.to_string())?,
            material: String::from_utf8(draw.material.to_vec()).map_err(|e| e.to_string())?,
            joint: joint.into(),
        });
    }
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_tables_have_exact_sixty_distinct_ids() {
        let c = catalogue();
        assert_eq!(c.len(), 60);
        let ids: std::collections::BTreeSet<_> = c.iter().map(|x| x.0).collect();
        assert_eq!(ids, (107..=166).collect());
        for (id, name) in [
            (107, "light_pod_fl"),
            (118, "light_3_glass_fr"),
            (119, "light_pod_bl"),
            (143, "light_pod_br"),
            (147, "light_rev_pod_br"),
            (166, "light_3_combi_glass_br"),
        ] {
            assert_eq!(c.iter().find(|x| x.0 == id).unwrap().3, name);
        }
        assert_eq!(decode_lod(b"LOD0_"), Ok(0));
        assert_eq!(decode_lod(b"LOD5_"), Ok(5));
        assert!(decode_lod(b"LOD8_").is_err());
        assert!(decode_lod(b"lod0_").is_err());
    }
    fn fixture(entries: &[(&str, &str, u32)]) -> Vec<u8> {
        const NAMES: [&str; 4] = [
            "ROOT",
            "MATRIXPALETTEBUNDLENODE",
            "MATRIXPALETTEJOINTNODE",
            "NODE",
        ];
        const ATTRS: [&str; 2] = ["id", "jointID"];
        fn u(n: u32) -> Vec<u8> {
            n.to_be_bytes().to_vec()
        }
        fn string(v: &str) -> Vec<u8> {
            [u(v.len() as u32), v.as_bytes().to_vec()].concat()
        }
        fn node(kind: &str, attrs: &[(&str, Vec<u8>)], payload: Vec<u8>) -> Vec<u8> {
            let mut a = Vec::new();
            for (key, value) in attrs {
                a.extend(u(ATTRS.iter().position(|x| x == key).unwrap() as u32 + 1));
                a.extend(u(value.len() as u32));
                a.extend(value);
            }
            [
                u(NAMES.iter().position(|x| *x == kind).unwrap() as u32 + 1),
                u((4 + a.len() + payload.len()) as u32),
                u(a.len() as u32),
                a,
                payload,
            ]
            .concat()
        }
        let children = entries
            .iter()
            .flat_map(|(kind, name, id)| {
                node(kind, &[("id", string(name)), ("jointID", u(*id))], vec![])
            })
            .collect();
        let payload = node(
            "MATRIXPALETTEBUNDLENODE",
            &[("id", string("LOD0_"))],
            children,
        );
        let mut b = b"PSSG".to_vec();
        b.extend(u(0));
        b.extend(u(2));
        b.extend(u(4));
        for (i, name) in NAMES.iter().enumerate() {
            b.extend(u(i as u32 + 1));
            b.extend(string(name));
            b.extend(u(if i == 0 { 2 } else { 0 }));
            if i == 0 {
                for (j, key) in ATTRS.iter().enumerate() {
                    b.extend(u(j as u32 + 1));
                    b.extend(string(key));
                }
            }
        }
        b.extend(node("ROOT", &[], payload));
        let size = b.len() as u32 - 8;
        b[4..8].copy_from_slice(&size.to_be_bytes());
        b
    }
    #[test]
    fn actual_tree_controls_do_not_trim_or_choose_duplicates() {
        for entries in [
            vec![],
            vec![("MATRIXPALETTEJOINTNODE", "x0_light_pod_fl\t", 9)],
            vec![("MATRIXPALETTEJOINTNODE", "x0_LIGHT_POD_FL", 9)],
        ] {
            let b = fixture(&entries);
            let tree = pssg::read_tree(&b).unwrap();
            assert!(read(&tree, &[], &[], b"LOD0_").unwrap().is_empty());
        }
        for entries in [
            vec![("NODE", "x0_light_pod_fl", 9)],
            vec![
                ("MATRIXPALETTEJOINTNODE", "x0_light_pod_fl", 9),
                ("MATRIXPALETTEJOINTNODE", "x0_light_pod_fl", 10),
            ],
        ] {
            let b = fixture(&entries);
            let tree = pssg::read_tree(&b).unwrap();
            assert!(read(&tree, &[], &[], b"LOD0_").is_err());
        }
        let b = fixture(&[("MATRIXPALETTEJOINTNODE", "x0_light_pod_fl", 9)]);
        let tree = pssg::read_tree(&b).unwrap();
        assert!(
            read(&tree, &[], &[], b"LOD0_").unwrap().is_empty(),
            "no draw does not invent geometry"
        );
    }
    #[test]
    fn low_loader_distinguishes_missing_bundle_from_present_unreadable_bundle() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("grid-lamp-sections-{}-{nonce}", std::process::id()));
        let model = grid_tools::car::Car::default();
        let file = root.join(model.file("_lowLOD.pssg"));
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let mut bytes = fixture(&[]);
        std::fs::write(&file, &bytes).unwrap();
        let missing = crate::low_lod(
            &root,
            &model,
            bevy::prelude::Mat4::IDENTITY,
            "",
            &BTreeMap::new(),
        );
        assert!(missing.parts.is_empty());
        assert!(format!("{:?}", missing.lamp_association.unwrap()).contains("sections: Ok([])"));
        let at = bytes.windows(5).position(|v| v == b"LOD0_").unwrap();
        bytes[at + 3] = b'2'; // Present bundle lacks the required rooted transform, so the real reader refuses.
        std::fs::write(&file, &bytes).unwrap();
        let unavailable = crate::low_lod(
            &root,
            &model,
            bevy::prelude::Mat4::IDENTITY,
            "",
            &BTreeMap::new(),
        );
        assert!(unavailable.parts.is_empty());
        assert!(
            format!("{:?}", unavailable.lamp_association.unwrap())
                .contains("present bundle reader unavailable")
        );
        std::fs::remove_file(&file).unwrap();
        let mut directory = file.parent().unwrap().to_path_buf();
        while directory.starts_with(&root) {
            std::fs::remove_dir(&directory).unwrap();
            if directory == root {
                break;
            }
            directory = directory.parent().unwrap().to_path_buf();
        }
    }
    #[test]
    #[ignore = "requires purchased GRID_GAME_ROOT; actual shared Tree/Mesh/Draw readers"]
    fn all_purchased_eighty_eight_packages_use_actual_draw_palette_join() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let root = std::path::Path::new(&root);
        let (mut packages, mut bundles, mut sections) = (0, 0, 0);
        for entry in std::fs::read_dir(root.join("cars")).unwrap() {
            let entry = entry.unwrap();
            let stem = entry.file_name();
            let Some(stem) = stem.to_str() else { continue };
            let Ok(model) = grid_tools::car::Car::new(stem) else {
                continue;
            };
            for suffix in ["_highLOD.pssg", "_lowLOD.pssg"] {
                let path = model.file(suffix);
                if !root.join(&path).is_file() {
                    continue;
                }
                let bytes = crate::read(&root.join(&path)).unwrap();
                let tree = pssg::read_tree(&bytes).unwrap();
                let meshes = mesh::read_meshes(&tree).unwrap();
                packages += 1;
                for lod in 0..8 {
                    let bundle = format!("LOD{lod}_");
                    if !(0..tree.nodes.len()).any(|i| {
                        tree.name(i) == b"MATRIXPALETTEBUNDLENODE"
                            && tree
                                .string(i, b"id")
                                .is_ok_and(|id| id == bundle.as_bytes())
                    }) {
                        continue;
                    }
                    let draws = scene::read_draws(&tree, &meshes, bundle.as_bytes()).unwrap();
                    let mapped = read(&tree, &meshes, &draws, bundle.as_bytes())
                        .unwrap_or_else(|e| panic!("{path} {bundle}: {e}"));
                    for section in &mapped {
                        assert_ne!(section.joint.as_bytes().last(), Some(&b'\t'));
                        assert_eq!(
                            tree.integer(section.node, b"jointID").unwrap(),
                            section.palette
                        );
                        assert!(draws.iter().any(|d| d.id == section.draw.as_bytes()
                            && d.joint == section.joint.as_bytes()));
                    }
                    sections += mapped.len();
                    bundles += 1;
                }
            }
        }
        assert_eq!(packages, 88, "fixture drift requires review");
        assert!(bundles > 0 && sections > 0);
        println!(
            "lamp_section_census packages={packages} bundles={bundles} rigid_sections={sections} topology=explicit-selected-bundle duplicate=refuse"
        );
    }
}
