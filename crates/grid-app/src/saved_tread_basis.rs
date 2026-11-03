//! Default-off rigid-forward saved-basis diagnostic, not original lighting parity.
use bevy::{
    asset::{AssetEventSystems, AssetId, uuid_handle},
    mesh::{MeshVertexAttribute, MeshVertexBufferLayoutRef, VertexAttributeValues},
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{
        AsBindGroup, RenderPipelineDescriptor, SpecializedMeshPipelineError, VertexFormat,
    },
    shader::ShaderRef,
    transform::TransformSystems,
};
use std::collections::HashMap;
pub const RAW_T: MeshVertexAttribute =
    MeshVertexAttribute::new("GridSavedTreadT", 0x4752545401, VertexFormat::Float32x3);
pub const RAW_B: MeshVertexAttribute =
    MeshVertexAttribute::new("GridSavedTreadB", 0x4752545402, VertexFormat::Float32x3);
pub const RAW_NM: MeshVertexAttribute = MeshVertexAttribute::new(
    "GridSavedTreadNormalMask",
    0x4752545403,
    VertexFormat::Float32x4,
);
const VERTEX: Handle<Shader> = uuid_handle!("2fbd6071-d407-4d62-8c1d-d8504f365917");
const FRAGMENT: Handle<Shader> = uuid_handle!("a7752b91-7934-4ccd-96fb-7e918876290f");
pub fn requested() -> Result<bool, &'static str> {
    match std::env::var("GRID_SAVED_TREAD_BASIS").as_deref() {
        Err(std::env::VarError::NotPresent) => Ok(false),
        Ok("1") => Ok(true),
        _ => Err("GRID_SAVED_TREAD_BASIS accepts only absent or1"),
    }
}
pub fn enabled() -> bool {
    std::env::var("GRID_SAVED_TREAD_BASIS").is_ok_and(|v| v == "1")
}
/// Original material identity stays available to existing LOD/overlay ownership.
/// This wrapper is NOT a second drawable StandardMaterial component.
#[derive(Component, Clone)]
pub struct Base(MeshMaterial3d<StandardMaterial>);
pub fn standard<'a>(
    material: Option<&'a MeshMaterial3d<StandardMaterial>>,
    base: Option<&'a Base>,
) -> Option<&'a MeshMaterial3d<StandardMaterial>> {
    assert!(
        material.is_none() || base.is_none(),
        "saved-basis entity cannot have two drawable material families"
    );
    material.or_else(|| base.map(|b| &b.0))
}
#[derive(Resource, Default)]
pub struct Bindings(pub HashMap<AssetId<StandardMaterial>, Handle<Image>>);
type Material = ExtendedMaterial<StandardMaterial, FullBasis>;
struct Derived {
    handle: Handle<Material>,
    source_normal: Option<AssetId<Image>>,
}
#[derive(Resource, Default)]
struct DerivedMaterials(HashMap<AssetId<StandardMaterial>, Derived>);
// Changed source assets, not every frame, are the authority for the projected base.
fn sync_base(
    mut events: MessageReader<AssetEvent<StandardMaterial>>,
    sources: Res<Assets<StandardMaterial>>,
    derived: Res<DerivedMaterials>,
    mut targets: ResMut<Assets<Material>>,
) {
    let mut dirty = std::collections::HashSet::new();
    for event in events.read() {
        match event {
            AssetEvent::Added { id } | AssetEvent::Modified { id }
                if derived.0.contains_key(id) =>
            {
                dirty.insert(*id);
            }
            AssetEvent::Removed { id } if derived.0.contains_key(id) => {
                panic!("saved basis mapped source material removed");
            }
            _ => {}
        }
    }
    for id in dirty {
        let binding = &derived.0[&id];
        let source = sources.get(id).expect("saved basis mapped source absent");
        assert_eq!(
            source.normal_map_texture.as_ref().map(|h| h.id()),
            binding.source_normal,
            "saved basis live normal binding replacement unsupported"
        );
        let target = targets
            .get(&binding.handle)
            .expect("saved basis derived material absent");
        let replacement = material(source, target.extension.normal.clone());
        *targets
            .get_mut(&binding.handle)
            .expect("saved basis derived material absent") = replacement;
    }
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
struct FullBasis {
    #[texture(100)]
    #[sampler(101)]
    normal: Handle<Image>,
}
fn has(defs: &[bevy::shader::ShaderDefVal], name: &str) -> bool {
    defs.iter().any(|d| match d {
        bevy::shader::ShaderDefVal::Bool(n, v) => n == name && *v,
        bevy::shader::ShaderDefVal::Int(n, _) | bevy::shader::ShaderDefVal::UInt(n, _) => n == name,
    })
}
fn unsupported_defs(
    vertex: &[bevy::shader::ShaderDefVal],
    fragment: &[bevy::shader::ShaderDefVal],
) -> Option<&'static str> {
    [
        "DEFERRED_PREPASS",
        "NORMAL_PREPASS",
        "SKINNED",
        "MORPH_TARGETS",
        "MESHLET_MESH_MATERIAL_PASS",
        "BINDLESS",
        "VERTEX_UVS_B",
        "VERTEX_COLORS",
        "OIT_ENABLED",
        "FORWARD_DECAL",
        "PREMULTIPLY_ALPHA",
    ]
    .into_iter()
    .find(|n| has(vertex, n) || has(fragment, n))
}
impl MaterialExtension for FullBasis {
    fn vertex_shader() -> ShaderRef {
        VERTEX.into()
    }
    fn fragment_shader() -> ShaderRef {
        FRAGMENT.into()
    }
    fn specialize(
        _: &MaterialExtensionPipeline,
        d: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let defs = &d.vertex.shader_defs;
        assert!(
            unsupported_defs(
                defs,
                d.fragment
                    .as_ref()
                    .map(|f| f.shader_defs.as_slice())
                    .unwrap_or(&[])
            )
            .is_none(),
            "saved basis unsupported shader/context; no ordinary fallback"
        );
        if has(defs, "PREPASS_PIPELINE") {
            return Ok(());
        } // unchanged stock depth/MV/shadow IO
        assert!(
            has(defs, "VERTEX_POSITIONS")
                && has(defs, "VERTEX_NORMALS")
                && has(defs, "VERTEX_UVS_A")
                && has(defs, "VERTEX_OUTPUT_INSTANCE_INDEX"),
            "saved basis requires position/normal/UV0"
        );
        let mut attrs = vec![
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            Mesh::ATTRIBUTE_NORMAL.at_shader_location(1),
            Mesh::ATTRIBUTE_UV_0.at_shader_location(2),
            RAW_T.at_shader_location(8),
            RAW_B.at_shader_location(9),
            RAW_NM.at_shader_location(10),
        ];
        if has(defs, "VERTEX_TANGENTS") {
            attrs.push(Mesh::ATTRIBUTE_TANGENT.at_shader_location(4));
        }
        d.vertex.buffers = vec![layout.0.get_layout(&attrs)?];
        Ok(())
    }
}
/// Float32 source vectors copied as-is; mask m=1-R/255, never albedo colour.
pub fn attach(
    mesh: &mut Mesh,
    source: &grid_formats::mesh::Mesh<'_>,
    range: std::ops::Range<usize>,
) -> Result<(), &'static str> {
    let mut t = Vec::with_capacity(range.len());
    let mut b = Vec::with_capacity(range.len());
    let mut nm = Vec::with_capacity(range.len());
    for i in range {
        let n = *source.normals.get(i).ok_or("missing rawN")?;
        let tv = *source.tangents.get(i).ok_or("missing rawT")?;
        let bv = *source.binormals.get(i).ok_or("missing rawB")?;
        let mask = source.colors_argb.get(i).ok_or("missing source mask")?;
        if !valid(n, tv, bv) {
            return Err("invalid/singular raw basis");
        }
        t.push(tv);
        b.push(bv);
        nm.push([n[0], n[1], n[2], 1. - f32::from(mask[1]) / 255.]);
    }
    mesh.insert_attribute(RAW_T, t);
    mesh.insert_attribute(RAW_B, b);
    mesh.insert_attribute(RAW_NM, nm);
    Ok(())
}
fn valid(n: [f32; 3], t: [f32; 3], b: [f32; 3]) -> bool {
    let n = Vec3::from_array(n);
    let t = Vec3::from_array(t);
    let b = Vec3::from_array(b);
    let determinant = t.dot(b.cross(n));
    n.is_finite()
        && t.is_finite()
        && b.is_finite()
        && n.length_squared().is_finite()
        && t.length_squared().is_finite()
        && b.length_squared().is_finite()
        && n.length_squared() > 0.
        && t.length_squared() > 0.
        && b.length_squared() > 0.
        && determinant.is_finite()
        && determinant != 0.
}
pub fn valid_streams(t: &[[f32; 3]], b: &[[f32; 3]], nm: &[[f32; 4]], count: usize) -> bool {
    count > 0
        && t.len() == count
        && b.len() == count
        && nm.len() == count
        && t.iter().zip(b).zip(nm).all(|((t, b), n)| {
            valid([n[0], n[1], n[2]], *t, *b) && n[3].is_finite() && (0. ..=1.).contains(&n[3])
        })
}
pub fn ready(mesh: &Mesh) -> bool {
    let (
        Some(VertexAttributeValues::Float32x3(t)),
        Some(VertexAttributeValues::Float32x3(b)),
        Some(VertexAttributeValues::Float32x4(nm)),
        Some(VertexAttributeValues::Float32x3(pos)),
        Some(VertexAttributeValues::Float32x3(normals)),
        Some(VertexAttributeValues::Float32x2(uv)),
    ) = (
        mesh.attribute(RAW_T),
        mesh.attribute(RAW_B),
        mesh.attribute(RAW_NM),
        mesh.attribute(Mesh::ATTRIBUTE_POSITION),
        mesh.attribute(Mesh::ATTRIBUTE_NORMAL),
        mesh.attribute(Mesh::ATTRIBUTE_UV_0),
    )
    else {
        return false;
    };
    valid_streams(t, b, nm, pos.len())
        && normals.len() == pos.len()
        && uv.len() == pos.len()
        && pos
            .iter()
            .flatten()
            .chain(uv.iter().flatten())
            .all(|v| v.is_finite())
        && normals.iter().all(|n| {
            let n = Vec3::from_array(*n);
            n.is_finite() && n.length_squared().is_finite() && n.length_squared() > 0.
        })
}
fn rigid(matrix: Mat4) -> bool {
    let x = matrix.x_axis.truncate();
    let y = matrix.y_axis.truncate();
    let z = matrix.z_axis.truncate();
    matrix.is_finite()
        && [
            x.length_squared() - 1.,
            y.length_squared() - 1.,
            z.length_squared() - 1.,
            x.dot(y),
            x.dot(z),
            y.dot(z),
        ]
        .iter()
        .all(|v| v.abs() < 1e-4)
}
fn material(base: &StandardMaterial, normal: Handle<Image>) -> Material {
    assert!(
        matches!(base.alpha_mode, AlphaMode::Opaque)
            && !base.unlit
            && !base.double_sided
            && base.diffuse_transmission == 0.
            && base.specular_transmission == 0.
            && base.clearcoat == 0.
            && base.anisotropy_strength == 0.
            && base.depth_map.is_none()
            && base.uv_transform == bevy::math::Affine2::IDENTITY
            && base.normal_map_channel == bevy::mesh::UvChannel::Uv0,
        "saved basis requires existing lit opaque dielectric surface"
    );
    let mut base = base.clone();
    base.normal_map_texture = None;
    Material {
        base,
        extension: FullBasis { normal },
    }
}
type ConversionPart = (
    Entity,
    &'static Mesh3d,
    &'static MeshMaterial3d<StandardMaterial>,
    &'static GlobalTransform,
    Has<crate::DeformingPart>,
    Has<crate::OpponentDeforming>,
);
type UnsupportedView = (
    Has<bevy::core_pipeline::prepass::NormalPrepass>,
    Has<bevy::core_pipeline::prepass::DeferredPrepass>,
);
#[allow(clippy::too_many_arguments)]
fn convert(
    mut commands: Commands,
    bindings: Option<Res<Bindings>>,
    parts: Query<ConversionPart>,
    meshes: Res<Assets<Mesh>>,
    standard: Res<Assets<StandardMaterial>>,
    mut materials: ResMut<Assets<Material>>,
    mut derived: ResMut<DerivedMaterials>,
    views: Query<UnsupportedView, With<Camera>>,
) {
    let bindings = bindings.expect("saved basis exact setup bindings absent");
    assert!(
        !views.iter().any(|(n, d)| n || d),
        "saved basis does not own normal/deferred prepass"
    );
    let mut count = 0;
    let mut refused = 0;
    for (entity, mesh, base, transform, deforming, opponent_deforming) in &parts {
        let Some(normal) = bindings.0.get(&base.id()) else {
            continue;
        };
        let mesh = meshes.get(&mesh.0).expect("loaded saved-basis mesh");
        if !ready(mesh) {
            refused += 1;
            if refused <= 16 {
                println!(
                    "saved_tread_basis entity={entity:?} active=false reason=raw-basis-not-ready stock-material-retained=true"
                );
            }
            continue;
        }
        assert!(
            !deforming && !opponent_deforming,
            "saved basis does not own dynamic normal deformation"
        );
        assert!(
            rigid(transform.to_matrix()),
            "saved basis supports rigid transforms only"
        );
        let handle = derived
            .0
            .entry(base.id())
            .or_insert_with(|| {
                let source = standard.get(base.id()).expect("source base remains alive");
                Derived {
                    handle: materials.add(material(source, normal.clone())),
                    source_normal: source.normal_map_texture.as_ref().map(|h| h.id()),
                }
            })
            .handle
            .clone();
        commands
            .entity(entity)
            .remove::<MeshMaterial3d<StandardMaterial>>()
            .insert((Base(base.clone()), MeshMaterial3d(handle)));
        count += 1;
    }
    assert!(count > 0, "saved basis no positive consumer entities");
    println!(
        "saved_tread_basis converted={count} stock_fallback={refused} normal_response=normalized-Bevy-PBR source_shader_parity=false source_sampler_state=unknown"
    );
}
pub fn install(app: &mut App) {
    app.world_mut()
        .resource_mut::<Assets<Shader>>()
        .insert(
            VERTEX.id(),
            Shader::from_wgsl(
                include_str!("saved_tread_basis_vertex.wgsl"),
                "grid_saved_tread_basis_vertex.wgsl",
            ),
        )
        .expect("unique saved-basis VS");
    app.world_mut()
        .resource_mut::<Assets<Shader>>()
        .insert(
            FRAGMENT.id(),
            Shader::from_wgsl(
                include_str!("saved_tread_basis_fragment.wgsl"),
                "grid_saved_tread_basis_fragment.wgsl",
            ),
        )
        .expect("unique saved-basis FS");
    app.add_plugins(MaterialPlugin::<Material>::default())
        .init_resource::<DerivedMaterials>()
        .add_systems(PostStartup, convert.after(TransformSystems::Propagate))
        .add_systems(PostUpdate, sync_base.after(AssetEventSystems));
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::mesh::{Indices, PrimitiveTopology};
    #[test]
    fn skew_is_retained_and_invalid_basis_is_explicit() {
        let t = [[1., 0., 0.]];
        let b = [[0., 1., 0.5]];
        let nm = [[0., 0., 1., 1.]];
        assert!(valid_streams(&t, &b, &nm, 1));
        assert_eq!(b[0][2], 0.5);
        assert!(!valid_streams(&t, &[[0., 0., 1.]], &nm, 1));
        assert!(!valid_streams(&t, &b, &[[0., 0., f32::NAN, 1.]], 1));
        assert!(!valid_streams(&t, &b, &[[0., 0., 1., 1.01]], 1));
        assert!(!valid_streams(&t, &b, &nm, 0));
        assert!(!valid_streams(&t, &b, &nm, 2));
    }
    #[test]
    fn original_standard_identity_survives_the_one_material_bridge() {
        let handle = Handle::<StandardMaterial>::default();
        let original = MeshMaterial3d(handle.clone());
        let base = Base(original.clone());
        assert_eq!(standard(Some(&original), None).unwrap().id(), handle.id());
        assert_eq!(standard(None, Some(&base)).unwrap().id(), handle.id());
        assert!(standard(None, None).is_none());
    }
    #[test]
    #[should_panic(expected = "two drawable material families")]
    fn dual_drawable_material_ownership_is_rejected() {
        let material = MeshMaterial3d(Handle::<StandardMaterial>::default());
        let base = Base(material.clone());
        let _ = standard(Some(&material), Some(&base));
    }
    #[test]
    fn rigid_domain_rejects_nonuniform_and_invalid_transform() {
        assert!(rigid(Mat4::from_rotation_x(0.3)));
        assert!(rigid(Mat4::from_scale(Vec3::new(-1., 1., 1.))));
        assert!(!rigid(Mat4::from_scale(Vec3::new(2., 1., 1.))));
        assert!(!rigid(Mat4::from_scale(Vec3::ZERO)));
    }
    #[test]
    fn factory_retains_existing_response_and_uses_only_extension_normal() {
        let image = Handle::<Image>::default();
        let base = StandardMaterial {
            perceptual_roughness: 0.9,
            reflectance: 0.3,
            normal_map_texture: Some(image.clone()),
            ..default()
        };
        let full = material(&base, image.clone());
        assert_eq!(full.base.perceptual_roughness, base.perceptual_roughness);
        assert_eq!(full.base.reflectance, base.reflectance);
        assert_eq!(full.base.base_color, base.base_color);
        assert!(full.base.normal_map_texture.is_none());
        assert_eq!(full.extension.normal.id(), image.id());
        assert!(base.normal_map_texture.is_some()); // original Std identity/overlay data untouched
    }
    #[test]
    fn actual_refinement_preserves_source_bits_and_extends_all_raw_streams() {
        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        );
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut uvs = Vec::new();
        let mut t = Vec::new();
        let mut b = Vec::new();
        let mut nm = Vec::new();
        let mut indices = Vec::new();
        for side in [0., 0.1] {
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::TAU / 8.;
                let (s, c) = a.sin_cos();
                positions.push([side, c, s]);
                normals.push([0., c, s]);
                uvs.push([i as f32 / 8., side]);
                t.push([1., 0., 0.]);
                b.push([0.2, -s, c]);
                nm.push([0., c, s, 1.]);
            }
        }
        for i in 0..8u32 {
            let next = (i + 1) % 8;
            indices.extend_from_slice(&[i, next, 8 + i, next, 8 + next, 8 + i]);
        }
        mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals.clone());
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs.clone());
        mesh.insert_indices(Indices::U32(indices.clone()));
        let mut legacy = mesh.clone();
        let mut source = grid_formats::mesh::Mesh {
            id: b"authored_saved_basis",
            positions: positions.clone(),
            normals,
            tangents: t.clone(),
            binormals: b.clone(),
            colors_argb: vec![[255, 0, 255, 255]; 16],
            nonfinite_normals: 0,
            nonfinite_texcoords: 0,
            texcoords: uvs,
            additional_texcoords: Vec::new(),
            skin_indices: None,
            skin_weights: None,
            indices: indices.iter().map(|&i| i as u16).collect(),
        };
        attach(&mut mesh, &source, 0..16).unwrap();
        let bits = |mesh: &Mesh, attribute: MeshVertexAttribute| -> Vec<u32> {
            match mesh.attribute(attribute).unwrap() {
                VertexAttributeValues::Float32x3(v) => {
                    v.iter().flatten().map(|v| v.to_bits()).collect()
                }
                VertexAttributeValues::Float32x2(v) => {
                    v.iter().flatten().map(|v| v.to_bits()).collect()
                }
                _ => panic!(),
            }
        };
        for attribute in [
            Mesh::ATTRIBUTE_POSITION,
            Mesh::ATTRIBUTE_NORMAL,
            Mesh::ATTRIBUTE_UV_0,
        ] {
            assert_eq!(bits(&mesh, attribute), bits(&legacy, attribute));
        }
        assert!(
            mesh.indices()
                .unwrap()
                .iter()
                .eq(legacy.indices().unwrap().iter())
        );
        let (before, after) = crate::tyre_visual::refine(&mut mesh).unwrap();
        let control = crate::tyre_visual::refine(&mut legacy).unwrap();
        assert_eq!((before, after), control);
        for attribute in [
            Mesh::ATTRIBUTE_POSITION,
            Mesh::ATTRIBUTE_NORMAL,
            Mesh::ATTRIBUTE_UV_0,
        ] {
            assert_eq!(bits(&mesh, attribute), bits(&legacy, attribute));
        }
        assert!(
            mesh.indices()
                .unwrap()
                .iter()
                .eq(legacy.indices().unwrap().iter())
        );
        // The real source-attach consumer must preserve mask endpoints and fail atomically.
        let mut single = Mesh::new(
            PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        );
        source.colors_argb[0][1] = 255;
        attach(&mut single, &source, 0..1).unwrap();
        let Some(VertexAttributeValues::Float32x4(v)) = single.attribute(RAW_NM) else {
            panic!()
        };
        assert_eq!(v[0][3], 0.);
        let mut refused = Mesh::new(
            PrimitiveTopology::TriangleList,
            bevy::asset::RenderAssetUsages::default(),
        );
        source.binormals[1] = source.normals[1];
        assert!(attach(&mut refused, &source, 0..2).is_err());
        assert!(
            refused.attribute(RAW_T).is_none()
                && refused.attribute(RAW_B).is_none()
                && refused.attribute(RAW_NM).is_none()
        );
        assert_eq!(after, before * 16);
        assert!(ready(&mesh));
        let Some(VertexAttributeValues::Float32x3(actual_t)) = mesh.attribute(RAW_T) else {
            panic!()
        };
        let Some(VertexAttributeValues::Float32x3(actual_b)) = mesh.attribute(RAW_B) else {
            panic!()
        };
        let Some(VertexAttributeValues::Float32x4(actual_nm)) = mesh.attribute(RAW_NM) else {
            panic!()
        };
        assert_eq!(
            actual_t[..16]
                .iter()
                .map(|v| v.map(f32::to_bits))
                .collect::<Vec<_>>(),
            t.iter().map(|v| v.map(f32::to_bits)).collect::<Vec<_>>()
        );
        assert_eq!(
            actual_b[..16]
                .iter()
                .map(|v| v.map(f32::to_bits))
                .collect::<Vec<_>>(),
            b.iter().map(|v| v.map(f32::to_bits)).collect::<Vec<_>>()
        );
        assert_eq!(
            actual_nm[..16]
                .iter()
                .map(|v| v.map(f32::to_bits))
                .collect::<Vec<_>>(),
            nm.iter().map(|v| v.map(f32::to_bits)).collect::<Vec<_>>()
        );
        assert!(actual_b.iter().all(|v| v[0].to_bits() == 0.2f32.to_bits())); // no cross-product substitute
        assert!(
            actual_t.len() > 16
                && actual_t.len() == actual_b.len()
                && actual_t.len() == actual_nm.len()
        );
    }
    fn sync_app() -> (App, Handle<StandardMaterial>, Handle<Material>) {
        let mut app = App::new();
        app.init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<Material>>()
            .init_resource::<DerivedMaterials>()
            .add_message::<AssetEvent<StandardMaterial>>()
            .add_systems(Update, sync_base);
        let source = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        let target = app
            .world_mut()
            .resource_mut::<Assets<Material>>()
            .add(material(&StandardMaterial::default(), Handle::default()));
        app.world_mut().resource_mut::<DerivedMaterials>().0.insert(
            source.id(),
            Derived {
                handle: target.clone(),
                source_normal: None,
            },
        );
        (app, source, target)
    }
    #[test]
    fn mapped_material_event_preserves_handle_and_normal_ownership() {
        let (mut app, source, target) = sync_app();
        let unrelated = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        app.world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .get_mut(&source)
            .unwrap()
            .perceptual_roughness = 0.37;
        // No asset event: do not blanket-copy changed values every frame.
        app.update();
        assert_ne!(
            app.world()
                .resource::<Assets<Material>>()
                .get(&target)
                .unwrap()
                .base
                .perceptual_roughness,
            0.37
        );
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<StandardMaterial>>>()
            .write(AssetEvent::Modified { id: unrelated.id() });
        app.update();
        assert_ne!(
            app.world()
                .resource::<Assets<Material>>()
                .get(&target)
                .unwrap()
                .base
                .perceptual_roughness,
            0.37
        );
        let old_normal = app
            .world()
            .resource::<Assets<Material>>()
            .get(&target)
            .unwrap()
            .extension
            .normal
            .id();
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<StandardMaterial>>>()
            .write(AssetEvent::Modified { id: source.id() });
        app.update();
        let rendered = app
            .world()
            .resource::<Assets<Material>>()
            .get(&target)
            .unwrap();
        assert_eq!(rendered.base.perceptual_roughness, 0.37);
        assert_eq!(rendered.extension.normal.id(), old_normal);
        assert_eq!(
            app.world().resource::<DerivedMaterials>().0[&source.id()]
                .handle
                .id(),
            target.id()
        );
    }
    #[test]
    #[should_panic(expected = "saved basis mapped source material removed")]
    fn mapped_source_removal_is_not_silently_stale() {
        let (mut app, source, _) = sync_app();
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<StandardMaterial>>>()
            .write(AssetEvent::Removed { id: source.id() });
        app.update();
    }
    #[test]
    #[should_panic(expected = "saved basis requires existing lit opaque dielectric surface")]
    fn mapped_unsupported_live_edit_is_rejected() {
        let (mut app, source, _) = sync_app();
        app.world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .get_mut(&source)
            .unwrap()
            .unlit = true;
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<StandardMaterial>>>()
            .write(AssetEvent::Modified { id: source.id() });
        app.update();
    }
    #[test]
    #[should_panic(expected = "saved basis live normal binding replacement unsupported")]
    fn mapped_normal_rebinding_is_not_silently_ignored() {
        let (mut app, source, _) = sync_app();
        app.world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .get_mut(&source)
            .unwrap()
            .normal_map_texture = Some(Handle::default());
        app.world_mut()
            .resource_mut::<Messages<AssetEvent<StandardMaterial>>>()
            .write(AssetEvent::Modified { id: source.id() });
        app.update();
    }
    #[test]
    fn stock_dither_is_admitted_in_both_passes_without_admitting_unsupported_io() {
        use bevy::shader::ShaderDefVal;
        let dither = ShaderDefVal::Bool("VISIBILITY_RANGE_DITHER".into(), true);
        assert!(unsupported_defs(std::slice::from_ref(&dither), &[]).is_none());
        assert!(
            unsupported_defs(
                &[
                    dither.clone(),
                    ShaderDefVal::Bool("PREPASS_PIPELINE".into(), true)
                ],
                &[]
            )
            .is_none()
        );
        assert_eq!(
            unsupported_defs(
                std::slice::from_ref(&dither),
                &[ShaderDefVal::Bool("NORMAL_PREPASS".into(), true)]
            ),
            Some("NORMAL_PREPASS")
        );
        assert_eq!(
            unsupported_defs(&[dither, ShaderDefVal::Bool("SKINNED".into(), true)], &[]),
            Some("SKINNED")
        );
    }
}
