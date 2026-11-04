//! Independently authored generic retained rig primitives, extracted from existing crowd_rig.
//! Caller owns selected instance, prior locals, explicit diagnostic pose policy.
use bevy::prelude::*;
use grid_formats::{anim_packet, mesh, pssg, scene};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
};
type AnyResult<T> = Result<T, Box<dyn Error>>;
const IDENTITY: [f32; 16] = [
    1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
];
#[derive(Clone)]
pub(crate) struct Node {
    pub(crate) name: Vec<u8>,
    pub(crate) parent: Option<usize>,
    pub(crate) local: [f32; 16],
}
pub(crate) fn palette_values(
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
pub(crate) fn skin_vertex(
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
    if !q.is_finite()
        || !normal.is_finite()
        || !normal_length.is_finite()
        || (normal_length < 1e-12 && n != [0.; 3])
    {
        return Err("unsupported neutral vertex/normal".into());
    }
    Ok((q.to_array(), normal.normalize_or_zero().to_array()))
}
/// Match the existing renderer's whole-stream smooth-normal fallback.
/// Fully finite saved streams pass through bit-for-bit; raw values remain owned separately.
fn render_normals(
    positions: &[[f32; 3]],
    saved: &[[f32; 3]],
    indices: &[u16],
) -> AnyResult<Vec<[f32; 3]>> {
    let n = positions.len();
    if n == 0
        || (!saved.is_empty() && saved.len() != n)
        || positions.iter().flatten().any(|v| !v.is_finite())
        || indices.iter().any(|&i| usize::from(i) >= n)
    {
        return Err("normal repair geometry/cardinality failure".into());
    }
    if saved.len() == n && saved.iter().flatten().all(|v| v.is_finite()) {
        return Ok(saved.to_vec());
    }
    if !indices.len().is_multiple_of(3) {
        return Err("normal repair requires triangle indices".into());
    }
    let mut geometry = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::default(),
    );
    geometry.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions.to_vec());
    geometry.insert_indices(bevy::mesh::Indices::U16(indices.to_vec()));
    geometry.compute_smooth_normals();
    let Some(bevy::mesh::VertexAttributeValues::Float32x3(values)) =
        geometry.attribute(Mesh::ATTRIBUTE_NORMAL)
    else {
        return Err("smooth normal repair output absent".into());
    };
    if values.len() != n || values.iter().flatten().any(|v| !v.is_finite()) {
        return Err("smooth normal repair output invalid".into());
    }
    // Bevy leaves finite zero normals on unused/degenerate/cancelling geometry.
    println!(
        "crowd_pose_normal_repair vertices={} saved_count={} saved_nonfinite={} repaired_zero={} policy=shared-Bevy-whole-stream-smooth raw_saved=retained",
        n,
        saved.len(),
        saved
            .iter()
            .filter(|v| v.iter().any(|v| !v.is_finite()))
            .count(),
        values.iter().filter(|v| **v == [0.; 3]).count()
    );
    Ok(values.clone())
}
pub(crate) fn inverse(data: &[u8]) -> AnyResult<([f32; 16], f32)> {
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
pub(crate) fn diagnostic_quaternion_row(value: [f32; 4]) -> AnyResult<([f32; 16], f32)> {
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

/// Actual raw source topology/geometry. No track/region/sex/LOD selection here.
pub(crate) struct Source {
    pub(crate) material_shader: Vec<u8>,
    nodes: Vec<Node>,
    names: Vec<Vec<u8>>,
    inverse: Vec<[f32; 16]>,
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>, // Raw saved values; may contain source NaNs.
    render_normals: Vec<[f32; 3]>,
    skin: mesh::SkinInfluences4,
    pub(crate) uv: Vec<[f32; 2]>,
    pub(crate) indices: Vec<u16>,
}
pub(crate) struct VertexBuffers {
    pub(crate) positions: Vec<[f32; 3]>,
    pub(crate) normals: Vec<[f32; 3]>,
}
pub(crate) struct PoseChannel<'a> {
    pub(crate) target: &'a [u8],
    pub(crate) kind: anim_packet::Kind,
    pub(crate) raw: [f32; 4],
}
/// Mandatory explicit diagnostic contract, NOT a native Delta/mirror policy.
pub(crate) enum PosePolicy {
    AbsoluteLocalChannelsIgnoringSourceDelta { mirrored: bool },
}
fn unique(tree: &pssg::Tree<'_>, kind: &[u8], id: &[u8]) -> AnyResult<usize> {
    let mut matches = (0..tree.nodes.len())
        .filter(|&i| tree.name(i) == kind && tree.string(i, b"id").ok() == Some(id));
    let i = matches.next().ok_or("source binding absent")?;
    if matches.next().is_some() {
        return Err("source binding ambiguous".into());
    }
    Ok(i)
}
impl Source {
    /// Caller selects ACTUAL skin and modifier instance IDs from source metadata.
    /// The instance's indices reference determines geometry; no guessed !C/defaultLOD.
    pub(crate) fn read(
        tree: &pssg::Tree<'_>,
        skin_id: &[u8],
        instance_id: &[u8],
    ) -> AnyResult<Self> {
        let skin_node = unique(tree, b"SKINNODE", skin_id)?;
        let mesh_catalogue = mesh::read_object_meshes(tree)?;
        // Reuse existing typed source→skin→ordered joint and geometry binding checks.
        let binding = scene::read_skin(tree, &mesh_catalogue, skin_id)?;
        let matches: Vec<_> = binding
            .instances
            .iter()
            .filter(|(id, _, _)| *id == instance_id)
            .collect();
        let [instance] = matches.as_slice() else {
            return Err("selected skin modifier absent/ambiguous".into());
        };
        let material_shader = instance.1.to_vec();
        let source = instance.2;
        let weighted = mesh::read_four_weight_mesh(tree, mesh_catalogue[source].id)?;
        let skeleton_id = tree
            .string(skin_node, b"skeleton")?
            .strip_prefix(b"#")
            .ok_or("external skeleton unsupported")?;
        let skeleton = unique(tree, b"SKELETON", skeleton_id)?;
        let inverse_nodes: Vec<_> = tree.nodes[skeleton]
            .children
            .iter()
            .copied()
            .filter(|&i| tree.name(i) == b"INVERSEBINDMATRIX")
            .collect();
        if inverse_nodes.len() != binding.joints.len()
            || tree.integer(skeleton, b"matrixCount")? as usize != inverse_nodes.len()
        {
            return Err("actual ordered inverse/joint counts disagree".into());
        }
        // read_skin validates metadata but its bind decoder clears affine noise.
        // Reuse existing crowd inverse() to retain EVERY original matrix word.
        let inverse = inverse_nodes
            .iter()
            .map(|&i| inverse(tree.nodes[i].data.ok_or("inverse payload absent")?).map(|(m, _)| m))
            .collect::<AnyResult<Vec<_>>>()?;
        let names = binding
            .joints
            .iter()
            .map(|&i| tree.string(i, b"id").map(Vec::from))
            .collect::<grid_formats::Result<Vec<_>>>()?;
        let transforms: Vec<_> = (0..tree.nodes.len())
            .filter(|&i| {
                matches!(
                    tree.name(i),
                    b"ROOTNODE" | b"NODE" | b"JOINTNODE" | b"SKINNODE"
                )
            })
            .collect();
        let mut nodes = Vec::new();
        for &i in &transforms {
            let mut at = tree.nodes[i].parent;
            let mut seen = BTreeSet::new();
            let parent = loop {
                let Some(p) = at else { break None };
                if p >= tree.nodes.len() || !seen.insert(p) {
                    return Err("source ancestry cycle/range".into());
                }
                if let Some(j) = transforms.iter().position(|&n| n == p) {
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
        let source = weighted.mesh;
        Self::checked(Self {
            material_shader,
            nodes,
            names,
            inverse,
            positions: source.positions,
            normals: source.normals,
            render_normals: Vec::new(),
            skin: weighted.influences,
            uv: source.texcoords,
            indices: source.indices,
        })
    }
    fn checked(mut result: Self) -> AnyResult<Self> {
        let Self {
            positions,
            normals,
            skin,
            uv,
            indices,
            ..
        } = &result;
        let n = positions.len();
        if n == 0
            || (!normals.is_empty() && normals.len() != n)
            || skin.weights.len() != n
            || skin.joints.len() != n
            || uv.len() != n
            || positions
                .iter()
                .flatten()
                .chain(uv.iter().flatten())
                .any(|v| !v.is_finite())
            || indices.iter().any(|&i| i as usize >= n)
        {
            return Err("geometry/influence/index cardinality or finite-domain failure".into());
        }
        result.render_normals = render_normals(positions, normals, indices)?;
        // Metadata/indices checked with retained source graph, WITHOUT requiring bind
        // cancellation: source saved locals need not be a neutral pose in every bank.
        result.deform_with_locals(&result.saved_locals())?;
        Ok(result)
    }
    pub(crate) fn original_vertices(&self) -> (&[[f32; 3]], &[[f32; 3]]) {
        (&self.positions, &self.normals)
    }
    pub(crate) fn diagnostics(&self) -> AnyResult<(f32, f32, f32)> {
        let (palette, palette_error) =
            palette_values(&self.nodes, &self.names, &self.inverse, false)?;
        let mut position_error = 0f32;
        for i in 0..self.positions.len() {
            let (p, _) = skin_vertex(
                self.positions[i],
                self.render_normals[i],
                &self.skin.weights[i],
                &self.skin.joints[i],
                &palette,
            )?;
            position_error = position_error.max(
                (Vec3::from_array(p) - Vec3::from_array(self.positions[i]))
                    .abs()
                    .max_element(),
            );
        }
        let inverse_error = self
            .inverse
            .iter()
            .flat_map(|m| [m[3], m[7], m[11], m[15] - 1.])
            .map(f32::abs)
            .fold(0., f32::max);
        Ok((position_error, palette_error, inverse_error))
    }
    /// Explicit diagnostic input option. Calling this is NOT a native reset policy.
    pub(crate) fn saved_locals(&self) -> Vec<[f32; 16]> {
        self.nodes.iter().map(|n| n.local).collect()
    }
    pub(crate) fn sample(
        &self,
        prior_locals: &[[f32; 16]],
        channels: &[PoseChannel<'_>],
        policy: PosePolicy,
    ) -> AnyResult<VertexBuffers> {
        let PosePolicy::AbsoluteLocalChannelsIgnoringSourceDelta { mirrored } = policy;
        if mirrored {
            return Err("diagnostic mirror transform is unresolved".into());
        }
        if prior_locals.len() != self.nodes.len()
            || prior_locals.iter().flatten().any(|v| !v.is_finite())
        {
            return Err("caller prior-local cardinality/nonfinite".into());
        }
        let mut locals = prior_locals.to_vec();
        let mut seen = BTreeSet::new();
        for channel in channels {
            let kind = match channel.kind {
                anim_packet::Kind::S4 => 0u8,
                anim_packet::Kind::L4 => 1u8,
            };
            if !seen.insert((kind, channel.target)) {
                return Err("duplicate pose target semantic".into());
            }
            let mut ids = self
                .nodes
                .iter()
                .enumerate()
                .filter(|(_, n)| n.name == channel.target);
            let i = ids.next().ok_or("sampled channel target absent")?.0;
            if ids.next().is_some() {
                return Err("sampled channel target ambiguous".into());
            }
            match channel.kind {
                anim_packet::Kind::S4 => {
                    let (matrix, _) = diagnostic_quaternion_row(channel.raw)?;
                    for k in [0, 1, 2, 4, 5, 6, 8, 9, 10] {
                        locals[i][k] = matrix[k];
                    }
                }
                anim_packet::Kind::L4 => {
                    if channel.raw.iter().any(|v| !v.is_finite()) {
                        return Err("nonfinite diagnostic translation".into());
                    }
                    locals[i][12..15].copy_from_slice(&channel.raw[..3]);
                }
            }
        }
        self.deform_with_locals(&locals)
    }
    fn deform_with_locals(&self, locals: &[[f32; 16]]) -> AnyResult<VertexBuffers> {
        if locals.len() != self.nodes.len() {
            return Err("prior-local count changed".into());
        }
        let mut nodes = self.nodes.clone();
        for (n, local) in nodes.iter_mut().zip(locals) {
            n.local = *local;
        }
        let (palette, _) = palette_values(&nodes, &self.names, &self.inverse, false)?;
        let mut positions = Vec::with_capacity(self.positions.len());
        let mut normals = Vec::with_capacity(self.normals.len());
        for i in 0..self.positions.len() {
            let (p, n) = skin_vertex(
                self.positions[i],
                self.render_normals[i],
                &self.skin.weights[i],
                &self.skin.joints[i],
                &palette,
            )?;
            positions.push(p);
            normals.push(n);
        }
        Ok(VertexBuffers { positions, normals })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn translated(x: f32) -> [f32; 16] {
        let mut m = IDENTITY;
        m[12] = x;
        m
    }
    fn fixture() -> Source {
        // Ordinary helper is retained; palette order deliberately differs from nodes.
        Source::checked(Source {
            material_shader: Vec::new(),
            nodes: vec![
                Node {
                    name: b"root".to_vec(),
                    parent: None,
                    local: translated(10.),
                },
                Node {
                    name: b"helper".to_vec(),
                    parent: Some(0),
                    local: translated(2.),
                },
                Node {
                    name: b"jointA".to_vec(),
                    parent: Some(1),
                    local: translated(3.),
                },
                Node {
                    name: b"jointB".to_vec(),
                    parent: Some(0),
                    local: translated(8.),
                },
            ],
            names: vec![b"jointB".to_vec(), b"jointA".to_vec()],
            inverse: vec![translated(-18.), translated(-15.)],
            positions: vec![[1., 0., 0.]],
            normals: vec![[0., 1., 0.]],
            render_normals: Vec::new(),
            skin: mesh::SkinInfluences4 {
                weights: vec![[0.25, 0.75, 0., 0.]],
                joints: vec![[0, 1, 0, 0]],
            },
            uv: vec![[0., 0.]],
            indices: vec![0],
        })
        .unwrap()
    }
    #[test]
    fn actual_palette_order_helpers_and_all_weight_lanes_are_retained() {
        let source = fixture();
        let mut prior = source.saved_locals();
        prior[2][12] += 4.;
        let result = source
            .sample(
                &prior,
                &[],
                PosePolicy::AbsoluteLocalChannelsIgnoringSourceDelta { mirrored: false },
            )
            .unwrap();
        assert_eq!(result.positions, vec![[4., 0., 0.]]);
        assert_eq!(result.normals, vec![[0., 1., 0.]]);
    }
    #[test]
    fn explicit_translation_changes_named_joint_and_missing_channels_preserve_prior() {
        let source = fixture();
        let prior = source.saved_locals();
        let result = source
            .sample(
                &prior,
                &[PoseChannel {
                    target: b"jointB",
                    kind: anim_packet::Kind::L4,
                    raw: [12., 0., 0., 99.],
                }],
                PosePolicy::AbsoluteLocalChannelsIgnoringSourceDelta { mirrored: false },
            )
            .unwrap();
        assert_eq!(result.positions, vec![[2., 0., 0.]]);
        assert_eq!(source.saved_locals(), prior);
    }
    #[test]
    fn mirror_missing_target_bad_prior_and_palette_indices_refuse() {
        let source = fixture();
        let prior = source.saved_locals();
        assert!(
            source
                .sample(
                    &prior,
                    &[],
                    PosePolicy::AbsoluteLocalChannelsIgnoringSourceDelta { mirrored: true }
                )
                .is_err()
        );
        assert!(
            source
                .sample(
                    &prior[..1],
                    &[],
                    PosePolicy::AbsoluteLocalChannelsIgnoringSourceDelta { mirrored: false }
                )
                .is_err()
        );
        assert!(
            source
                .sample(
                    &prior,
                    &[PoseChannel {
                        target: b"missing",
                        kind: anim_packet::Kind::L4,
                        raw: [0.; 4]
                    }],
                    PosePolicy::AbsoluteLocalChannelsIgnoringSourceDelta { mirrored: false }
                )
                .is_err()
        );
        assert!(
            skin_vertex(
                [0.; 3],
                [0., 1., 0.],
                &[1., 0., 0., 0.],
                &[2, 0, 0, 0],
                &[IDENTITY]
            )
            .is_err()
        );
    }
}

#[cfg(test)]
mod normal_repair_tests {
    use super::*;
    #[test]
    fn finite_saved_stream_is_bit_preserved() {
        let positions = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]];
        let saved = [[1., -0., 0.], [0., 2., 0.], [0., 0., 3.]];
        let fixed = render_normals(&positions, &saved, &[0, 1, 2]).unwrap();
        assert_eq!(
            fixed
                .iter()
                .map(|v| v.map(f32::to_bits))
                .collect::<Vec<_>>(),
            saved.map(|v| v.map(f32::to_bits))
        );
    }
    #[test]
    fn invalid_stream_uses_actual_shared_smooth_normals_without_changing_raw() {
        let positions = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]];
        let raw = [[1., 0., 0.], [f32::NAN, 0., 0.], [0., 2., 0.]];
        let before = raw.map(|v| v.map(f32::to_bits));
        let fixed = render_normals(&positions, &raw, &[0, 1, 2]).unwrap();
        assert_eq!(fixed, vec![[0., 0., 1.]; 3]);
        assert_eq!(raw.map(|v| v.map(f32::to_bits)), before);
        assert_eq!(render_normals(&positions, &[], &[0, 1, 2]).unwrap(), fixed);
    }
    #[test]
    fn degenerate_and_unused_normals_stay_zero_and_skin_safely() {
        let positions = [[0., 0., 0.]; 4];
        let fixed = render_normals(&positions, &[], &[0, 1, 2]).unwrap();
        assert_eq!(fixed, vec![[0.; 3]; 4]);
        let (p, n) = skin_vertex(
            [1., 2., 3.],
            fixed[3],
            &[1., 0., 0., 0.],
            &[0, 0, 0, 0],
            &[IDENTITY],
        )
        .unwrap();
        assert_eq!(p, [1., 2., 3.]);
        assert_eq!(n, [0.; 3]);
        let mut opposed = IDENTITY;
        opposed[0] = -1.;
        opposed[5] = -1.;
        opposed[10] = -1.;
        assert!(
            skin_vertex(
                [0.; 3],
                [1., 0., 0.],
                &[0.5, 0.5, 0., 0.],
                &[0, 1, 0, 0],
                &[IDENTITY, opposed]
            )
            .is_err()
        );
    }
    #[test]
    fn normal_repair_refuses_bad_geometry_and_mixed_cardinality() {
        let p = [[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]];
        assert!(render_normals(&p, &[[0.; 3]], &[0, 1, 2]).is_err());
        assert!(render_normals(&p, &[], &[0, 1, 3]).is_err());
        assert!(render_normals(&p, &[], &[0, 1]).is_err());
        assert!(render_normals(&[[f32::INFINITY, 0., 0.]], &[], &[]).is_err());
    }
}
