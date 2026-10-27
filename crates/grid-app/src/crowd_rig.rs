//! One authored neutral USA actor. No native animation/clock/LOD policy.
use bevy::prelude::*;
use grid_formats::{mesh, pssg, scene, texture};
use std::{collections::BTreeMap, error::Error, path::Path};
type AnyResult<T> = Result<T, Box<dyn Error>>;
const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];
#[derive(Clone)]
struct Node {
    name: Vec<u8>,
    parent: Option<usize>,
    local: [f32; 16],
}
pub(crate) struct Actor {
    pub source_positions: Vec<[f32; 3]>,
    pub source_normals: Vec<[f32; 3]>,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub indices: Vec<u16>,
    pub diffuse: Vec<u8>,
    pub tint: Vec<u8>,
    pub source_min: Vec3,
    pub source_max: Vec3,
    pub foot_offset: Vec3,
    pub max_position_error: f32,
    pub max_palette_error: f32,
    pub max_inverse_affine_error: f32,
}
fn palettes(
    nodes: &[Node],
    joints: &[Vec<u8>],
    inverse: &[[f32; 16]],
) -> AnyResult<(Vec<[f32; 16]>, f32)> {
    if nodes.is_empty()
        || nodes.len() > 128
        || joints.len() != inverse.len()
        || joints.is_empty()
        || joints.len() > 128
    {
        return Err("neutral rig counts invalid".into());
    }
    let mut ids = BTreeMap::new();
    for (i, n) in nodes.iter().enumerate() {
        if ids.insert(n.name.as_slice(), i).is_some() || n.local.iter().any(|v| !v.is_finite()) {
            return Err("duplicate/nonfinite neutral transform".into());
        }
    }
    let mut worlds = Vec::with_capacity(nodes.len());
    for start in 0..nodes.len() {
        let mut m = IDENTITY;
        let mut at = Some(start);
        let mut seen = Vec::new();
        while let Some(i) = at {
            if i >= nodes.len() || seen.contains(&i) || seen.len() >= 128 {
                return Err("neutral hierarchy cycle/range/depth".into());
            }
            seen.push(i);
            m = scene::compose(&m, &nodes[i].local);
            at = nodes[i].parent;
        }
        if m.iter().any(|v| !v.is_finite()) {
            return Err("nonfinite neutral world".into());
        }
        worlds.push(m);
    }
    let mut result = Vec::new();
    let mut max_error = 0f32;
    let mut names = Vec::new();
    for (name, ib) in joints.iter().zip(inverse) {
        if names.contains(name) {
            return Err("duplicate neutral palette joint".into());
        }
        names.push(name.clone());
        let i = *ids
            .get(name.as_slice())
            .ok_or("neutral joint name missing")?;
        let p = scene::compose(ib, &worlds[i]);
        for (&x, &y) in p.iter().zip(&IDENTITY) {
            if !x.is_finite() {
                return Err("nonfinite palette".into());
            }
            max_error = max_error.max((x - y).abs());
        }
        result.push(p);
    }
    if max_error > 1e-5 {
        return Err(format!("neutral inverse/world cancellation error {max_error}").into());
    }
    Ok((result, max_error))
}
fn inverse(data: &[u8]) -> AnyResult<([f32; 16], f32)> {
    if data.len() != 64 {
        return Err("neutral inverse matrix bytes differ".into());
    }
    let mut m = [0.; 16];
    for (v, b) in m.iter_mut().zip(data.as_chunks::<4>().0) {
        *v = f32::from_be_bytes(*b);
    }
    if m.iter().any(|v| !v.is_finite()) {
        return Err("nonfinite neutral inverse".into());
    }
    let error = [m[3], m[7], m[11], m[15] - 1.]
        .into_iter()
        .map(f32::abs)
        .fold(0., f32::max);
    if error > 1e-4 {
        return Err("unsupported inverse affine column".into());
    }
    // Preserve every saved value: no clearing of tolerated affine noise.
    Ok((m, error))
}
fn rig(tree: &pssg::Tree<'_>) -> AnyResult<(Vec<[f32; 16]>, f32, f32)> {
    let transform_nodes: Vec<_> = (0..tree.nodes.len())
        .filter(|&i| {
            matches!(
                tree.name(i),
                b"ROOTNODE" | b"NODE" | b"JOINTNODE" | b"SKINNODE"
            )
        })
        .collect();
    if transform_nodes.len() != 34 {
        return Err("selected USA transform count differs".into());
    }
    let mut nodes = Vec::new();
    for &i in &transform_nodes {
        let mut at = tree.nodes[i].parent;
        let mut steps = 0;
        let parent = loop {
            let Some(p) = at else { break None };
            steps += 1;
            if steps > tree.nodes.len() {
                return Err("tree ancestry cycle".into());
            }
            if let Some(j) = transform_nodes.iter().position(|&n| n == p) {
                break Some(j);
            }
            at = tree.nodes[p].parent;
        };
        nodes.push(Node {
            name: tree.string(i, b"id")?.to_vec(),
            parent,
            local: scene::matrix(tree, i)?,
        });
    }
    let skins: Vec<_> = (0..tree.nodes.len())
        .filter(|&i| tree.name(i) == b"SKINNODE")
        .collect();
    let [skin] = skins.as_slice() else {
        return Err("expected one USA skin".into());
    };
    if tree.string(*skin, b"id")? != b"f_US_lod0"
        || tree.string(*skin, b"skeleton")? != b"#!A"
        || tree.integer(*skin, b"jointCount")? != 23
    {
        return Err("selected USA skin envelope differs".into());
    }
    let skeletons: Vec<_> = (0..tree.nodes.len())
        .filter(|&i| tree.name(i) == b"SKELETON" && tree.string(i, b"id").ok() == Some(b"!A"))
        .collect();
    let [skeleton] = skeletons.as_slice() else {
        return Err("expected selected skeleton !A".into());
    };
    if tree.integer(*skeleton, b"matrixCount")? != 23 {
        return Err("USA inverse count differs".into());
    }
    let js: Vec<_> = tree.nodes[*skin]
        .children
        .iter()
        .copied()
        .filter(|&i| tree.name(i) == b"SKINJOINT")
        .collect();
    let inv: Vec<_> = tree.nodes[*skeleton]
        .children
        .iter()
        .copied()
        .filter(|&i| tree.name(i) == b"INVERSEBINDMATRIX")
        .collect();
    if js.len() != 23 || inv.len() != 23 {
        return Err("USA ordered palette count differs".into());
    }
    let mut names = Vec::new();
    let mut matrices = Vec::new();
    let mut affine = 0f32;
    for (j, i) in js.into_iter().zip(inv) {
        names.push(
            tree.string(j, b"joint")?
                .strip_prefix(b"#")
                .ok_or("nonlocal joint")?
                .to_vec(),
        );
        let (m, e) = inverse(tree.nodes[i].data.ok_or("inverse raw absent")?)?;
        matrices.push(m);
        affine = affine.max(e);
    }
    let (p, error) = palettes(&nodes, &names, &matrices)?;
    Ok((p, error, affine))
}
pub(crate) fn prepare(root: &Path) -> AnyResult<Actor> {
    let path = root.join("anims/cr_female_usa.pssg");
    if std::fs::metadata(&path)?.len() > 16 * 1024 * 1024 {
        return Err("near actor package exceeds bound".into());
    }
    let bytes = std::fs::read(path)?;
    if bytes.len() > 16 * 1024 * 1024 {
        return Err("near actor package grew beyond bound".into());
    }
    let tree = pssg::read_tree(&bytes)?;
    let weighted = mesh::read_four_weight_mesh(&tree, b"!C")?;
    let source = weighted.mesh;
    let skin = weighted.influences;
    if source.positions.len() != 786
        || source.normals.len() != 786
        || source.texcoords.len() != 786
        || source.indices.len() != 3468
        || source.nonfinite_texcoords != 0
        || source.texcoords.iter().flatten().any(|v| !v.is_finite())
        || source.skin_indices.is_some()
    {
        return Err("female !C geometry contract differs".into());
    }
    let (palette, max_palette_error, max_inverse_affine_error) = rig(&tree)?;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut max_position_error = 0f32;
    for (i, (&p, &n)) in source.positions.iter().zip(&source.normals).enumerate() {
        if skin.weights[i].iter().any(|v| !v.is_finite()) || skin.joints[i].iter().any(|&j| j >= 23)
        {
            return Err("invalid retained influence".into());
        }
        let mut q = Vec3::ZERO;
        let mut normal = Vec3::ZERO;
        for (&w, &j) in skin.weights[i].iter().zip(&skin.joints[i]) {
            let m = &palette[j as usize];
            q += Vec3::from_array(scene::transform_point(m, p)) * w;
            normal += Vec3::new(
                n[0] * m[0] + n[1] * m[4] + n[2] * m[8],
                n[0] * m[1] + n[1] * m[5] + n[2] * m[9],
                n[0] * m[2] + n[1] * m[6] + n[2] * m[10],
            ) * w;
        }
        let normal_length = normal.length_squared();
        if !q.is_finite()
            || !normal.is_finite()
            || !normal_length.is_finite()
            || normal_length < 1e-12
        {
            return Err("unsupported neutral vertex/normal".into());
        }
        max_position_error = max_position_error.max((q - Vec3::from_array(p)).abs().max_element());
        positions.push(q.to_array());
        normals.push(normal.normalize().to_array());
    }
    if max_position_error > 1e-5 {
        return Err(format!("neutral weighted position error {max_position_error}").into());
    }
    let source_min = positions.iter().fold(Vec3::splat(f32::INFINITY), |a, p| {
        a.min(Vec3::from_array(*p))
    });
    let source_max = positions
        .iter()
        .fold(Vec3::splat(f32::NEG_INFINITY), |a, p| {
            a.max(Vec3::from_array(*p))
        });
    let foot_offset = Vec3::new(
        -(source_min.x + source_max.x) * 0.5,
        -source_min.y,
        -(source_min.z + source_max.z) * 0.5,
    );
    if !source_min.is_finite() || !source_max.is_finite() || !foot_offset.is_finite() {
        return Err("unsupported neutral bounds/foot alignment".into());
    }
    let textures = texture::read_named(&tree, &[b"cr_female_usa_d.tga", b"cr_female_usa_m.tga"])?;
    let d = textures
        .iter()
        .find(|t| t.id == b"cr_female_usa_d.tga")
        .ok_or("near diffuse absent")?;
    let m = textures
        .iter()
        .find(|t| t.id == b"cr_female_usa_m.tga")
        .ok_or("near tint absent")?;
    if [d, m]
        .iter()
        .any(|t| t.width != 1024 || t.height != 512 || t.format != texture::Format::Bc1)
    {
        return Err("near texture format differs".into());
    }
    println!(
        "crowd_neutral_cpu source=female-USA/!C vertices=786 indices=3468 joints=23 position_error={max_position_error} palette_error={max_palette_error} inverse_affine_error={max_inverse_affine_error} neutral_bounds={source_min:?}/{source_max:?} foot_offset={foot_offset:?} hierarchy=saved-neutral weights=preserved pose_clock_parity=false"
    );
    Ok(Actor {
        source_positions: source.positions,
        source_normals: source.normals,
        positions,
        normals,
        uv: source.texcoords,
        indices: source.indices,
        diffuse: d.validated_dds().0,
        tint: m.validated_dds().0,
        source_min,
        source_max,
        foot_offset,
        max_position_error,
        max_palette_error,
        max_inverse_affine_error,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    fn translation(x: f32, y: f32, z: f32) -> [f32; 16] {
        let mut m = IDENTITY;
        m[12] = x;
        m[13] = y;
        m[14] = z;
        m
    }
    #[test]
    fn ordinary_helper_ancestor_and_named_palette_order_cancel_only_correct_inverse() {
        let nodes = vec![
            Node {
                name: b"root".to_vec(),
                parent: None,
                local: translation(1., 0., 0.),
            },
            Node {
                name: b"ordinary-neck".to_vec(),
                parent: Some(0),
                // Exact row-vector +90deg Z with local translation Y2.
                local: [
                    0., 1., 0., 0., -1., 0., 0., 0., 0., 0., 1., 0., 0., 2., 0., 1.,
                ],
            },
            Node {
                name: b"head".to_vec(),
                parent: Some(1),
                local: translation(0., 0., 3.),
            },
        ];
        let names = vec![b"head".to_vec(), b"root".to_vec()];
        // Noncommuting rotation/translation makes world-composition order observable.
        let inverse = [
            [
                0., -1., 0., 0., 1., 0., 0., 0., 0., 0., 1., 0., -2., 1., -3., 1.,
            ],
            translation(-1., 0., 0.),
        ];
        let (p, error) = palettes(&nodes, &names, &inverse).unwrap();
        assert_eq!(error, 0.);
        assert_eq!(scene::transform_point(&p[0], [2., 3., 4.]), [2., 3., 4.]);
        assert!(palettes(&nodes, &names, &[inverse[1], inverse[0]]).is_err());
        let mut missing = nodes;
        missing[2].parent = Some(99);
        assert!(palettes(&missing, &names, &inverse).is_err());
    }
}
