//! One authored neutral USA actor. No native animation/clock/LOD policy.
use bevy::prelude::*;
use grid_formats::{anim_packet, mesh, pssg, scene, texture};
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
#[derive(Clone, Copy, PartialEq, Eq)]
enum PoseInput {
    Neutral,
    Authored20,
    StoredKeyHypothesis,
}
pub(crate) struct PoseStats {
    pub stored_key_hypothesis: bool,
    pub affected_palette: Vec<usize>,
    pub affected_bones: Vec<String>,
    pub affected_nodes: usize,
    pub influenced_vertices: usize,
    pub moved_vertices: usize,
    pub max_displacement: f32,
}
struct Rig {
    nodes: Vec<Node>,
    names: Vec<Vec<u8>>,
    inverse: Vec<[f32; 16]>,
    affine_error: f32,
}
pub(crate) struct Actor {
    pub pose: Option<PoseStats>,
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
    palette_values(nodes, joints, inverse, true)
}
fn palette_values(
    nodes: &[Node],
    joints: &[Vec<u8>],
    inverse: &[[f32; 16]],
    require_neutral: bool,
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
    if require_neutral && max_error > 1e-5 {
        return Err(format!("neutral inverse/world cancellation error {max_error}").into());
    }
    Ok((result, max_error))
}
fn skin_vertex(
    p: [f32; 3],
    n: [f32; 3],
    weights: &[f32; 4],
    joints: &[u8; 4],
    palette: &[[f32; 16]],
) -> AnyResult<([f32; 3], [f32; 3])> {
    if weights.iter().any(|v| !v.is_finite()) || joints.iter().any(|&j| j as usize >= palette.len())
    {
        return Err("invalid retained influence".into());
    }
    let mut q = Vec3::ZERO;
    let mut normal = Vec3::ZERO;
    for (&w, &j) in weights.iter().zip(joints) {
        let m = &palette[j as usize];
        q += Vec3::from_array(scene::transform_point(m, p)) * w;
        normal += Vec3::new(
            n[0] * m[0] + n[1] * m[4] + n[2] * m[8],
            n[0] * m[1] + n[1] * m[5] + n[2] * m[9],
            n[0] * m[2] + n[1] * m[6] + n[2] * m[10],
        ) * w;
    }
    let normal_length = normal.length_squared();
    if !q.is_finite() || !normal.is_finite() || !normal_length.is_finite() || normal_length < 1e-12
    {
        return Err("unsupported neutral vertex/normal".into());
    }
    Ok((q.to_array(), normal.normalize().to_array()))
}
// Caller first validates the unchanged saved neutral rig with palettes().
fn descendants(nodes: &[Node], target: usize) -> AnyResult<Vec<bool>> {
    if target >= nodes.len() || nodes[target].parent.is_none() {
        return Err("authored forearm cannot be a root".into());
    }
    let mut affected = Vec::with_capacity(nodes.len());
    for start in 0..nodes.len() {
        let mut at = Some(start);
        let mut hit = false;
        let mut steps = 0;
        while let Some(i) = at {
            if i >= nodes.len() || steps >= 128 {
                return Err("authored descendant ancestry invalid".into());
            }
            steps += 1;
            if i == target {
                hit = true;
            }
            at = nodes[i].parent;
        }
        affected.push(hit);
    }
    Ok(affected)
}
fn rotated_local(nodes: &[Node], target: usize) -> AnyResult<(Vec<Node>, Vec<bool>)> {
    let affected = descendants(nodes, target)?;
    let (sin, cos) = 20f32.to_radians().sin_cos();
    let rotation = [
        cos, sin, 0., 0., -sin, cos, 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
    ];
    let mut posed = nodes.to_vec();
    let saved = posed[target].local;
    posed[target].local = scene::compose(&rotation, &saved);
    // Preserve saved translation bits, including signed zero.
    posed[target].local[12..15].copy_from_slice(&saved[12..15]);
    Ok((posed, affected))
}
fn pose_vertices(
    source: &[[f32; 3]],
    source_normals: &[[f32; 3]],
    skin: &mesh::SkinInfluences4,
    palette: &[[f32; 16]],
    affected: &[bool],
    positions: &mut [[f32; 3]],
    normals: &mut [[f32; 3]],
) -> AnyResult<(usize, usize, f32)> {
    if [
        source_normals.len(),
        skin.weights.len(),
        skin.joints.len(),
        positions.len(),
        normals.len(),
    ]
    .iter()
    .any(|&n| n != source.len())
        || affected.len() != palette.len()
    {
        return Err("authored pose vertex lengths differ".into());
    }
    let mut influenced = 0;
    let mut moved = 0;
    let mut maximum = 0f32;
    for i in 0..source.len() {
        if skin.joints[i].iter().any(|&j| j as usize >= affected.len()) {
            return Err("authored palette index outside bounds".into());
        }
        if !skin.weights[i]
            .iter()
            .zip(&skin.joints[i])
            .any(|(&w, &j)| w != 0. && affected[j as usize])
        {
            continue;
        }
        influenced += 1;
        let (p, n) = skin_vertex(
            source[i],
            source_normals[i],
            &skin.weights[i],
            &skin.joints[i],
            palette,
        )?;
        let displacement = (Vec3::from_array(p) - Vec3::from_array(positions[i]))
            .abs()
            .max_element();
        if !displacement.is_finite() {
            return Err("nonfinite authored displacement".into());
        }
        if displacement > 0. {
            moved += 1;
        }
        maximum = maximum.max(displacement);
        positions[i] = p;
        normals[i] = n;
    }
    Ok((influenced, moved, maximum))
}
// Explicit diagnostic convention, NOT the unresolved original receiver semantics.
fn quaternion_row(value: [f32; 4]) -> AnyResult<([f32; 16], f32)> {
    if value.iter().any(|v| !v.is_finite()) {
        return Err("nonfinite stored quaternion".into());
    }
    let norm_squared = value.iter().map(|v| v * v).sum::<f32>();
    if !norm_squared.is_finite() || (norm_squared - 1.).abs() > 1e-3 {
        return Err("stored quaternion outside near-unit diagnostic domain".into());
    }
    let q = Quat::from_xyzw(value[0], value[1], value[2], value[3]).normalize();
    let row = Mat4::from_quat(q).to_cols_array();
    if row.iter().any(|v| !v.is_finite()) {
        return Err("nonfinite quaternion row".into());
    }
    Ok((row, norm_squared))
}
fn basis_error(a: &[f32; 16], b: &[f32; 16]) -> f32 {
    [0, 1, 2, 4, 5, 6, 8, 9, 10]
        .into_iter()
        .map(|i| (a[i] - b[i]).abs())
        .fold(0., f32::max)
}
fn replaced_local(
    nodes: &[Node],
    target: usize,
    row: &[f32; 16],
) -> AnyResult<(Vec<Node>, Vec<bool>)> {
    let affected = descendants(nodes, target)?;
    let mut posed = nodes.to_vec();
    // Replace ONLY 3x3. Saved root, translation and affine-column bits stay untouched.
    for i in [0, 1, 2, 4, 5, 6, 8, 9, 10] {
        posed[target].local[i] = row[i];
    }
    Ok((posed, affected))
}
fn named(tree: &pssg::Tree<'_>, kind: &[u8], id: &[u8]) -> AnyResult<usize> {
    let matches: Vec<_> = (0..tree.nodes.len())
        .filter(|&i| tree.name(i) == kind && tree.string(i, b"id").ok() == Some(id))
        .collect();
    match matches.as_slice() {
        [one] => Ok(*one),
        _ => Err("selected bank ID absent/duplicate".into()),
    }
}
fn stored_forearm_row(root: &Path, rig: &Rig) -> AnyResult<[f32; 16]> {
    let path = root.join("anims/crowd_anims.pssg");
    if std::fs::metadata(&path)?.len() != 1_389_508 {
        return Err("selected crowd bank size differs".into());
    }
    let bank = std::fs::read(path)?;
    if bank.len() != 1_389_508 {
        return Err("selected crowd bank changed size".into());
    }
    let tree = pssg::read_tree(&bank)?;
    let clip = named(&tree, b"NeAnimClip", b"f_gs_s1_ex_03")?;
    let refs: Vec<_> = tree.nodes[clip]
        .children
        .iter()
        .copied()
        .filter(|&i| tree.name(i) == b"NeAnimClipPacketRef")
        .collect();
    if refs.len() != 2 || tree.integer(clip, b"packetCount")? != 2 {
        return Err("selected clip references differ".into());
    }
    let refs: Vec<_> = refs
        .iter()
        .map(|&i| tree.string(i, b"packet"))
        .collect::<grid_formats::Result<_>>()?;
    if refs.iter().filter(|&&r| r == b"#packet234").count() != 1
        || refs.iter().filter(|&&r| r == b"#packet235").count() != 1
    {
        return Err("selected ordinary pair differs".into());
    }
    let parent = named(&tree, b"NeAnimPacket_S4", b"packet235")?;
    let [leaf] = tree.nodes[parent].children.as_slice() else {
        return Err("selected S4 leaf count differs".into());
    };
    let node = &tree.nodes[*leaf];
    if tree.name(*leaf) != b"NeAnimPacketData_S4"
        || node.parent != Some(parent)
        || !node.children.is_empty()
        || !node.attributes.is_empty()
    {
        return Err("selected direct S4 leaf differs".into());
    }
    let data = node.data.ok_or("selected S4 payload absent")?;
    if data.len() != 23451
        || tree.integer(parent, b"size")? != 22464
        || tree.integer(parent, b"alignment")? != 16
    {
        return Err("selected S4 envelope differs".into());
    }
    let packet = anim_packet::read(
        data,
        anim_packet::Envelope {
            parent_type: "NeAnimPacket_S4",
            leaf_type: "NeAnimPacketData_S4",
            parent_alignment: 16,
            native_size: 22464,
        },
    )?;
    if packet.kind != anim_packet::Kind::S4 || packet.channels.len() != 30 {
        return Err("selected S4 channel domain differs".into());
    }
    let forearm = packet.channels.get(16).ok_or("selected channel16 absent")?;
    if forearm.target != "Bip_01 L Forearm"
        || forearm.keys.len() != 49
        || packet
            .channels
            .iter()
            .filter(|c| c.target == forearm.target)
            .count()
            != 1
    {
        return Err("selected forearm target/count differs".into());
    }
    let fingers: Vec<_> = packet
        .channels
        .iter()
        .filter(|c| c.target == "Bip_01 L Finger01")
        .collect();
    let [finger] = fingers.as_slice() else {
        return Err("constant finger control absent/duplicate".into());
    };
    if finger.keys.len() != 2 {
        return Err("constant finger count differs".into());
    }
    let key = forearm.keys.first().ok_or("selected first key absent")?;
    let finger_key = finger.keys.first().ok_or("finger first key absent")?;
    if key.time != 0. || finger_key.time != 0. {
        return Err("selected first stored coordinates differ".into());
    }
    let (row, norm_squared) = quaternion_row(key.value)?;
    let (finger_row, finger_norm) = quaternion_row(finger_key.value)?;
    let saved_finger = rig
        .nodes
        .iter()
        .find(|n| n.name == b"Bip_01 L Finger01")
        .ok_or("saved finger control absent")?;
    let finger_error = basis_error(&finger_row, &saved_finger.local);
    let finger_nonidentity = basis_error(&saved_finger.local, &IDENTITY);
    if finger_error > 1e-5 || finger_nonidentity < 0.1 {
        return Err("stored-key lane/convention control differs".into());
    }
    println!(
        "crowd_key_hypothesis clip=f_gs_s1_ex_03 cache_declared_delta=Yes reference_delta=UNRESOLVED packet=235 target_channel=16 target_palette=15 key_index=0 stored_coordinate={} raw_XYZW_bits={:?} raw_norm_squared={} normalization=authored-unit-not-native-policy quaternion_lanes=XYZW interpretation=HYPOTHESIS-absolute-local-3x3-replacement finger_first_basis_error={} finger_saved_nonidentity={} finger_raw_norm_squared={} root_L4_values=not-decoded-or-applied root=saved-neutral clock=NONE source_pose_parity=false",
        key.time,
        key.value.map(f32::to_bits),
        norm_squared,
        finger_error,
        finger_nonidentity,
        finger_norm
    );
    Ok(row)
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
fn rig(tree: &pssg::Tree<'_>) -> AnyResult<Rig> {
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
    Ok(Rig {
        nodes,
        names,
        inverse: matrices,
        affine_error: affine,
    })
}
pub(crate) fn prepare(root: &Path) -> AnyResult<Actor> {
    prepare_actor(root, PoseInput::Neutral)
}
pub(crate) fn prepare_arm_pose(root: &Path) -> AnyResult<Actor> {
    prepare_actor(root, PoseInput::Authored20)
}
pub(crate) fn prepare_key_hypothesis(root: &Path) -> AnyResult<Actor> {
    prepare_actor(root, PoseInput::StoredKeyHypothesis)
}
fn prepare_actor(root: &Path, input: PoseInput) -> AnyResult<Actor> {
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
    let rig = rig(&tree)?;
    let (palette, max_palette_error) = palettes(&rig.nodes, &rig.names, &rig.inverse)?;
    let max_inverse_affine_error = rig.affine_error;
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut max_position_error = 0f32;
    for (i, (&p, &n)) in source.positions.iter().zip(&source.normals).enumerate() {
        let (q, normal) = skin_vertex(p, n, &skin.weights[i], &skin.joints[i], &palette)?;
        max_position_error = max_position_error.max(
            (Vec3::from_array(q) - Vec3::from_array(p))
                .abs()
                .max_element(),
        );
        positions.push(q);
        normals.push(normal);
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
    // Placement/yaw/foot offset and bounds above belong to NEUTRAL baseline.
    // Never recompute them from this authored arm pose.
    let pose = if input != PoseInput::Neutral {
        const TARGET: &[u8] = b"Bip_01 L Forearm";
        if rig.names.get(15).map(Vec::as_slice) != Some(TARGET) {
            return Err("selected forearm palette15 differs".into());
        }
        let target = rig
            .nodes
            .iter()
            .position(|n| n.name == TARGET)
            .ok_or("selected forearm absent")?;
        let (posed_nodes, affected_nodes) = if input == PoseInput::StoredKeyHypothesis {
            let row = stored_forearm_row(root, &rig)?;
            replaced_local(&rig.nodes, target, &row)?
        } else {
            rotated_local(&rig.nodes, target)?
        };
        let (posed_palette, _) = palette_values(&posed_nodes, &rig.names, &rig.inverse, false)?;
        let affected: Vec<_> = rig
            .names
            .iter()
            .map(|name| {
                rig.nodes
                    .iter()
                    .position(|n| n.name == *name)
                    .map(|i| affected_nodes[i])
                    .ok_or("authored joint absent")
            })
            .collect::<Result<_, _>>()?;
        let affected_palette: Vec<_> = affected
            .iter()
            .enumerate()
            .filter_map(|(i, &hit)| hit.then_some(i))
            .collect();
        if affected_palette != [15, 16, 17] {
            return Err("selected forearm descendant palette differs".into());
        }
        let affected_bones = affected_palette
            .iter()
            .map(|&i| String::from_utf8(rig.names[i].clone()))
            .collect::<Result<_, _>>()?;
        let (influenced_vertices, moved_vertices, max_displacement) = pose_vertices(
            &source.positions,
            &source.normals,
            &skin,
            &posed_palette,
            &affected,
            &mut positions,
            &mut normals,
        )?;
        if moved_vertices == 0 || max_displacement <= 1e-6 {
            return Err("authored arm pose produced no observable vertex displacement".into());
        }
        Some(PoseStats {
            stored_key_hypothesis: input == PoseInput::StoredKeyHypothesis,
            affected_palette,
            affected_bones,
            affected_nodes: affected_nodes.iter().filter(|&&v| v).count(),
            influenced_vertices,
            moved_vertices,
            max_displacement,
        })
    } else {
        None
    };
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
    if let Some(pose) = &pose {
        if pose.stored_key_hypothesis {
            println!(
                "crowd_key_hypothesis_skin affected_palette={:?} affected_bones={:?} influenced_vertices={} moved_vertices={} max_displacement={} neutral_position_error={} neutral_palette_error={} baseline_foot_offset={:?} reference_delta=UNRESOLVED root=saved-neutral source_pose_parity=false",
                pose.affected_palette,
                pose.affected_bones,
                pose.influenced_vertices,
                pose.moved_vertices,
                pose.max_displacement,
                max_position_error,
                max_palette_error,
                foot_offset
            );
        } else {
            println!(
                "crowd_authored_pose local_axis=Z degrees=20 target=left-forearm palette15=true affected_palette={:?} affected_bones={:?} affected_nodes={} influenced_vertices={} moved_vertices={} max_displacement={} neutral_position_error={} neutral_palette_error={} baseline_foot_offset={:?} root=saved-neutral clock=none source_pose_parity=false",
                pose.affected_palette,
                pose.affected_bones,
                pose.affected_nodes,
                pose.influenced_vertices,
                pose.moved_vertices,
                pose.max_displacement,
                max_position_error,
                max_palette_error,
                foot_offset
            );
        }
    }
    Ok(Actor {
        pose,
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
    #[test]
    fn fixed_forearm_rotation_preserves_pivot_root_and_uninfluenced_vertex() {
        let nodes = vec![
            Node {
                name: b"root".to_vec(),
                parent: None,
                local: IDENTITY,
            },
            Node {
                name: b"forearm".to_vec(),
                parent: Some(0),
                local: translation(1., 0., 0.),
            },
            Node {
                name: b"hand".to_vec(),
                parent: Some(1),
                local: translation(1., 0., 0.),
            },
        ];
        let names = vec![b"forearm".to_vec(), b"hand".to_vec(), b"root".to_vec()];
        let inverse = [translation(-1., 0., 0.), translation(-2., 0., 0.), IDENTITY];
        palettes(&nodes, &names, &inverse).unwrap();
        let (posed, affected_nodes) = rotated_local(&nodes, 1).unwrap();
        assert_eq!(posed[0].local, nodes[0].local);
        assert_eq!(posed[1].local[12..15], nodes[1].local[12..15]);
        assert_eq!(affected_nodes, vec![false, true, true]);
        let (palette, _) = palette_values(&posed, &names, &inverse, false).unwrap();
        let source = [[2., 0., 0.], [0., 1., 0.]];
        let source_normals = [[0., 0., 1.]; 2];
        let mut positions = source;
        let mut normals = source_normals;
        let skin = mesh::SkinInfluences4 {
            weights: vec![[0.5, 0.5, 0., 0.], [1., 0., 0., 0.]],
            joints: vec![[1, 2, 0, 0], [2, 0, 1, 0]],
        };
        let (influenced, moved, _) = pose_vertices(
            &source,
            &source_normals,
            &skin,
            &palette,
            &[true, true, false],
            &mut positions,
            &mut normals,
        )
        .unwrap();
        assert_eq!((influenced, moved), (1, 1));
        assert_eq!(positions[1].map(f32::to_bits), source[1].map(f32::to_bits));
        assert_eq!(
            normals[1].map(f32::to_bits),
            source_normals[1].map(f32::to_bits)
        );
        let (sin, cos) = 20f32.to_radians().sin_cos();
        assert!(
            (Vec3::from_array(positions[0]) - Vec3::new(1.5 + 0.5 * cos, 0.5 * sin, 0.)).length()
                < 1e-6
        );
    }
    #[test]
    fn authored_quaternion90z_row_replaces_basis_without_rotating_saved_translation() {
        let h = std::f32::consts::FRAC_1_SQRT_2;
        let (row, _) = quaternion_row([0., 0., h, h]).unwrap();
        assert!(
            (Vec3::from_array(scene::transform_point(&row, [1., 0., 0.])) - Vec3::Y).length()
                < 1e-6
        );
        let nodes = vec![
            Node {
                name: b"root".to_vec(),
                parent: None,
                local: IDENTITY,
            },
            Node {
                name: b"forearm".to_vec(),
                parent: Some(0),
                local: translation(-0., 2., 3.),
            },
        ];
        let (posed, _) = replaced_local(&nodes, 1, &row).unwrap();
        assert_eq!(posed[0].local, nodes[0].local);
        assert_eq!(
            posed[1].local[12..15]
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>(),
            nodes[1].local[12..15]
                .iter()
                .map(|v| v.to_bits())
                .collect::<Vec<_>>()
        );
        assert!(
            (Vec3::from_array(scene::transform_point(&posed[1].local, [1., 0., 0.]))
                - Vec3::new(0., 3., 3.))
            .length()
                < 1e-6
        );
    }
}
