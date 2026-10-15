use grid_formats::{mesh, pssg, texture};

// Synthetic schema deliberately declares attributes on ROOT. Other nodes use
// those global IDs, exercising the shared-ID behavior observed in GRID.
const NAMES: &[&str] = &[
    "ROOT",
    "DATABLOCK",
    "DATABLOCKSTREAM",
    "DATABLOCKDATA",
    "RENDERDATASOURCE",
    "RENDERSTREAM",
    "RENDERINDEXSOURCE",
    "INDEXSOURCEDATA",
    "TEXTURE",
    "TEXTUREIMAGEBLOCK",
    "TEXTUREIMAGEBLOCKDATA",
    "TRANSFORM",
    "MATRIXPALETTEBUNDLENODE",
    "MATRIXPALETTEJOINTNODE",
    "MATRIXPALETTERENDERINSTANCE",
    "RENDERINSTANCESOURCE",
    "SHADERINSTANCE",
    "ROOTNODE",
    "NODE",
    "RENDERNODE",
    "RENDERSTREAMINSTANCE",
    "SHADERGROUP",
    "SHADERINPUTDEFINITION",
    "SHADERINPUT",
    "PNSTRING",
    "DATA",
    "TEMPLATETRANSFORM",
    "TEMPLATEENTITYINSTANCE",
    "TEMPLATEENTITYREFERENCE",
    "TEMPLATEENTITY",
    "TEMPLATESUBENTITY",
    "TEMPLATEBEHAVIOUR",
    "TEMPLATEBEHAVIOURTYPE",
    "TEMPLATECOMPONENTCHANGE",
    "TEMPLATETRANSFORMABLE",
    "TEMPLATERENDERABLE",
    "TEMPLATERIGIDBODY",
    "TEMPLATERIGIDBODYSHAPE",
    "TEMPLATESHAPEBOX",
    "TEMPLATECLOTH",
    "TEMPLATEENTITYLINK",
];
const ATTRS: &[&str] = &[
    "id",
    "streamCount",
    "size",
    "elementCount",
    "renderType",
    "dataType",
    "offset",
    "stride",
    "dataBlock",
    "subStream",
    "primitive",
    "format",
    "count",
    "maximumIndex",
    "width",
    "height",
    "texelFormat",
    "imageBlockCount",
    "numberMipMapLevels",
    "typename",
    "sourceCount",
    "indices",
    "streamOffset",
    "elementCountFromOffset",
    "indexOffset",
    "indicesCountFromOffset",
    "jointID",
    "shader",
    "source",
    "name",
    "type",
    "shaderGroup",
    "parameterCount",
    "parameterSavedCount",
    "parameterID",
    "texture",
    "uri",
    "typeuri",
    "componenturi",
    "operation",
    "sid",
    "props",
    "infMass",
    "shell",
    "phantom",
    "length",
];

fn u(value: u32) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}
fn text(value: &str) -> Vec<u8> {
    let mut b = u(value.len() as u32);
    b.extend(value.as_bytes());
    b
}
fn node(name: &str, attrs: &[(&str, Vec<u8>)], payload: Vec<u8>) -> Vec<u8> {
    let mut block = Vec::new();
    for (name, value) in attrs {
        block.extend(u(ATTRS.iter().position(|n| n == name).unwrap() as u32 + 1));
        block.extend(u(value.len() as u32));
        block.extend(value);
    }
    let mut b = u(NAMES.iter().position(|n| *n == name).unwrap() as u32 + 1);
    b.extend(u((4 + block.len() + payload.len()) as u32));
    b.extend(u(block.len() as u32));
    b.extend(block);
    b.extend(payload);
    b
}
fn file(payload: Vec<u8>) -> Vec<u8> {
    let mut b = b"PSSG".to_vec();
    b.extend(u(0));
    b.extend(u(ATTRS.len() as u32));
    b.extend(u(NAMES.len() as u32));
    for (i, name) in NAMES.iter().enumerate() {
        b.extend(u(i as u32 + 1));
        b.extend(text(name));
        b.extend(u(if i == 0 { ATTRS.len() as u32 } else { 0 }));
        if i == 0 {
            for (j, name) in ATTRS.iter().enumerate() {
                b.extend(u(j as u32 + 1));
                b.extend(text(name));
            }
        }
    }
    b.extend(node("ROOT", &[], payload));
    let size = b.len() as u32 - 8;
    b[4..8].copy_from_slice(&size.to_be_bytes());
    b
}
fn fixture() -> Vec<u8> {
    mesh_fixture(false)
}
fn mesh_fixture(rigid: bool) -> Vec<u8> {
    let mut buffer = Vec::new();
    for position in [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
        for value in position.into_iter().chain([0.0, 0.0, 1.0, 0.25, 0.75]) {
            buffer.extend(value.to_be_bytes());
        }
        if rigid {
            buffer.extend(7.0f32.to_be_bytes());
        }
    }
    let mut block = Vec::new();
    let mut descriptors = vec![
        ("Vertex", "float3", 0),
        ("Normal", "float3", 12),
        ("ST", "float2", 24),
    ];
    if rigid {
        descriptors.push(("SkinIndices", "float", 32));
    }
    for (kind, format, offset) in descriptors {
        block.extend(node(
            "DATABLOCKSTREAM",
            &[
                ("renderType", text(kind)),
                ("dataType", text(format)),
                ("offset", u(offset)),
                ("stride", u(if rigid { 36 } else { 32 })),
            ],
            vec![],
        ));
    }
    block.extend(node("DATABLOCKDATA", &[], buffer));
    let mut payload = node(
        "DATABLOCK",
        &[
            ("id", text("points")),
            ("streamCount", u(if rigid { 4 } else { 3 })),
            ("size", u(if rigid { 108 } else { 96 })),
            ("elementCount", u(3)),
        ],
        block,
    );
    let mut source = node(
        "RENDERINDEXSOURCE",
        &[
            ("primitive", text("triangles")),
            ("format", text("ushort")),
            ("count", u(3)),
            ("maximumIndex", u(2)),
        ],
        node("INDEXSOURCEDATA", &[], vec![0, 0, 0, 1, 0, 2]),
    );
    for i in 0..if rigid { 4 } else { 3 } {
        source.extend(node(
            "RENDERSTREAM",
            &[("dataBlock", text("#points")), ("subStream", u(i))],
            vec![],
        ));
    }
    payload.extend(node(
        "RENDERDATASOURCE",
        &[
            ("id", text("triangle")),
            ("streamCount", u(if rigid { 4 } else { 3 })),
        ],
        source,
    ));
    let data = node(
        "TEXTUREIMAGEBLOCKDATA",
        &[],
        vec![0x00, 0xf8, 0, 0, 0, 0, 0, 0],
    ); // BC1 red block
    let block = node(
        "TEXTUREIMAGEBLOCK",
        &[("typename", text("Raw")), ("size", u(8))],
        data,
    );
    payload.extend(node(
        "TEXTURE",
        &[
            ("id", text("synthetic")),
            ("width", u(4)),
            ("height", u(4)),
            ("texelFormat", text("dxt1")),
            ("imageBlockCount", u(1)),
            ("numberMipMapLevels", u(0)),
        ],
        block,
    ));
    file(payload)
}

fn attribute_offset(bytes: &[u8], node_name: &[u8], name: &[u8]) -> usize {
    let tree = pssg::read_tree(bytes).unwrap();
    let i = (0..tree.nodes.len())
        .find(|&i| tree.name(i) == node_name)
        .unwrap();
    let value = tree.attribute(i, name).unwrap();
    value.as_ptr() as usize - bytes.as_ptr() as usize
}

#[test]
fn decodes_shared_attributes_and_big_endian_mesh_streams() {
    let bytes = fixture();
    let tree = pssg::read_tree(&bytes).unwrap();
    let meshes = mesh::read_meshes(&tree).unwrap();
    assert_eq!(meshes.len(), 1);
    assert_eq!(
        meshes[0].positions,
        vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]
    );
    assert_eq!(meshes[0].normals, vec![[0.0, 0.0, 1.0]; 3]);
    assert_eq!(meshes[0].texcoords, vec![[0.25, 0.75]; 3]);
    assert_eq!(meshes[0].indices, vec![0, 1, 2]);
    assert!(
        tree.nodes
            .iter()
            .enumerate()
            .all(|(i, n)| n.children.iter().all(|&j| tree.nodes[j].parent == Some(i)))
    );
}

#[test]
fn borrows_texture_base_and_builds_standard_dds_header() {
    let bytes = fixture();
    let tree = pssg::read_tree(&bytes).unwrap();
    let textures = texture::read_textures(&tree).unwrap();
    let t = &textures[0];
    assert_eq!((t.width, t.height, t.format), (4, 4, texture::Format::Bc1));
    assert_eq!(t.base_blocks.len(), 8);
    assert!(t.remaining_data.is_empty());
    let dds = t.base_dds();
    assert_eq!(&dds[..4], b"DDS ");
    assert_eq!(&dds[84..88], b"DXT1");
    assert_eq!(dds.len(), 136);
    assert_eq!(u32::from_le_bytes(dds[4..8].try_into().unwrap()), 124);
    assert_eq!(&dds[128..], t.base_blocks);
}

#[test]
fn preserves_nonfinite_normals_without_accepting_nonfinite_positions() {
    let mut bytes = fixture();
    let tree = pssg::read_tree(&bytes).unwrap();
    let node = (0..tree.nodes.len())
        .find(|&i| tree.name(i) == b"DATABLOCKDATA")
        .unwrap();
    let offset = tree.nodes[node].data.unwrap().as_ptr() as usize - bytes.as_ptr() as usize;
    bytes[offset + 12..offset + 16].copy_from_slice(&f32::NAN.to_be_bytes());
    let tree = pssg::read_tree(&bytes).unwrap();
    let meshes = mesh::read_meshes(&tree).unwrap();
    assert_eq!(meshes[0].nonfinite_normals, 1);
    assert!(meshes[0].normals[0][0].is_nan());
}

#[test]
fn rejects_every_truncation_and_parent_boundary_escape() {
    let bytes = fixture();
    for end in 0..bytes.len() {
        assert!(pssg::read_tree(&bytes[..end]).is_err());
    }
    let tree = pssg::read_tree(&bytes).unwrap();
    let offset = tree.nodes[1].offset;
    let mut bad = bytes.clone();
    bad[offset + 4..offset + 8].copy_from_slice(&u(u32::MAX));
    assert!(pssg::read_tree(&bad).is_err());
    let mut bad = bytes.clone();
    bad[offset + 8..offset + 12].copy_from_slice(&u(u32::MAX));
    assert!(pssg::read_tree(&bad).is_err());
    let mut bad = bytes;
    bad[offset..offset + 4].copy_from_slice(&u(0));
    assert!(pssg::read_tree(&bad).is_err());
}

#[test]
fn rejects_bad_stream_layout_references_counts_and_nonfinite_floats() {
    let base = fixture();
    for (node_name, name, value) in [
        (b"DATABLOCKSTREAM".as_slice(), b"stride".as_slice(), 4),
        (b"DATABLOCK", b"elementCount", u32::MAX),
        (b"RENDERSTREAM", b"subStream", 3),
        (b"RENDERINDEXSOURCE", b"maximumIndex", 0),
        (b"RENDERINDEXSOURCE", b"count", u32::MAX),
    ] {
        let mut b = base.clone();
        let offset = attribute_offset(&b, node_name, name);
        b[offset..offset + 4].copy_from_slice(&u(value));
        assert!(mesh::read_meshes(&pssg::read_tree(&b).unwrap()).is_err());
    }
    let tree = pssg::read_tree(&base).unwrap();
    let index = (0..tree.nodes.len())
        .find(|&i| tree.name(i) == b"DATABLOCKDATA")
        .unwrap();
    let offset = tree.nodes[index].data.unwrap().as_ptr() as usize - base.as_ptr() as usize;
    let mut b = base.clone();
    b[offset..offset + 4].copy_from_slice(&f32::NAN.to_be_bytes());
    assert!(mesh::read_meshes(&pssg::read_tree(&b).unwrap()).is_err());
    let index = (0..tree.nodes.len())
        .find(|&i| tree.name(i) == b"INDEXSOURCEDATA")
        .unwrap();
    let offset = tree.nodes[index].data.unwrap().as_ptr() as usize - base.as_ptr() as usize;
    let mut b = base.clone();
    b[offset..offset + 2].copy_from_slice(&3u16.to_be_bytes());
    assert!(mesh::read_meshes(&pssg::read_tree(&b).unwrap()).is_err());
    let offset = attribute_offset(&base, b"RENDERSTREAM", b"dataBlock");
    let mut b = base;
    b[offset + 5] = b'x';
    assert!(mesh::read_meshes(&pssg::read_tree(&b).unwrap()).is_err());
}

#[test]
fn rejects_texture_size_dimensions_and_unsupported_format() {
    let base = fixture();
    for (node_name, name, value) in [
        (b"TEXTURE".as_slice(), b"width".as_slice(), 0),
        (b"TEXTURE", b"width", 16_388),
        (b"TEXTURE", b"height", 3),
        (b"TEXTURE", b"width", 8),
        (b"TEXTURE", b"imageBlockCount", 2),
        (b"TEXTUREIMAGEBLOCK", b"size", 7),
    ] {
        let mut b = base.clone();
        let offset = attribute_offset(&b, node_name, name);
        b[offset..offset + 4].copy_from_slice(&u(value));
        assert!(texture::read_textures(&pssg::read_tree(&b).unwrap()).is_err());
    }
    let offset = attribute_offset(&base, b"TEXTURE", b"texelFormat");
    let mut b = base;
    b[offset + 4] = b'x';
    assert!(texture::read_textures(&pssg::read_tree(&b).unwrap()).is_err());
}

#[test]
fn rejects_excessive_depth_duplicate_attributes_and_unknown_raw_payloads() {
    let mut nested = node("ROOT", &[], vec![]);
    for _ in 0..130 {
        nested = node("ROOT", &[], nested);
    }
    assert!(pssg::read_tree(&file(nested)).is_err());
    assert!(
        pssg::read_tree(&file(node(
            "ROOT",
            &[("id", text("x")), ("id", text("y"))],
            vec![]
        )))
        .is_err()
    );
    assert!(pssg::read_tree(&file(node("ROOT", &[], vec![0; 12]))).is_err());
}

fn scene_fixture() -> Vec<u8> {
    let identity: [f32; 16] = [
        1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
    ];
    let transform: [f32; 16] = [
        0., 1., 0., 0., -1., 0., 0., 0., 0., 0., 1., 0., 10., 20., 30., 1.,
    ];
    let draw = node(
        "MATRIXPALETTERENDERINSTANCE",
        &[
            ("id", text("draw")),
            ("sourceCount", u(1)),
            ("streamCount", u(0)),
            ("indices", text("#triangle")),
            ("streamOffset", u(2)),
            ("elementCountFromOffset", u(3)),
            ("indexOffset", u(0)),
            ("indicesCountFromOffset", u(3)),
            ("shader", text("#paint")),
        ],
        node(
            "RENDERINSTANCESOURCE",
            &[("source", text("#triangle"))],
            vec![],
        ),
    );
    let joint = node(
        "MATRIXPALETTEJOINTNODE",
        &[("id", text("door")), ("jointID", u(7))],
        [
            node(
                "TRANSFORM",
                &[],
                transform.into_iter().flat_map(f32::to_be_bytes).collect(),
            ),
            draw,
        ]
        .concat(),
    );
    let bundle = node(
        "MATRIXPALETTEBUNDLENODE",
        &[("id", text("LOD0_"))],
        [
            node(
                "TRANSFORM",
                &[],
                identity.into_iter().flat_map(f32::to_be_bytes).collect(),
            ),
            joint,
        ]
        .concat(),
    );
    file(
        [
            bundle,
            node("SHADERINSTANCE", &[("id", text("paint"))], vec![]),
        ]
        .concat(),
    )
}
fn scene_mesh() -> mesh::Mesh<'static> {
    mesh::Mesh {
        id: b"triangle",
        positions: vec![[0.; 3]; 6],
        normals: vec![[0., 0., 1.]; 6],
        nonfinite_normals: 0,
        texcoords: vec![[0.; 2]; 6],
        additional_texcoords: vec![],
        skin_indices: Some(vec![7.; 6]),
        indices: vec![2, 3, 4],
    }
}
#[test]
fn assembles_selected_ranges_and_row_vector_rotation_translation() {
    let bytes = scene_fixture();
    let tree = pssg::read_tree(&bytes).unwrap();
    let meshes = [scene_mesh()];
    let draws = grid_formats::scene::read_draws(&tree, &meshes, b"LOD0_").unwrap();
    assert_eq!(draws.len(), 1);
    assert_eq!(draws[0].vertices, 2..5);
    assert_eq!(draws[0].indices, [0, 1, 2]);
    assert_eq!(
        grid_formats::scene::transform_point(&draws[0].transform, [2., 3., 4.]),
        [7., 22., 34.]
    );
    assert!(grid_formats::scene::read_draws(&tree, &meshes, b"missing").is_err());
    for (node_name, name, value) in [
        (
            b"MATRIXPALETTERENDERINSTANCE".as_slice(),
            b"streamOffset".as_slice(),
            5,
        ),
        (b"MATRIXPALETTERENDERINSTANCE", b"indicesCountFromOffset", 4),
        (b"MATRIXPALETTERENDERINSTANCE", b"sourceCount", 2),
        (b"MATRIXPALETTEJOINTNODE", b"jointID", 8),
    ] {
        let mut bad = bytes.clone();
        let offset = attribute_offset(&bad, node_name, name);
        bad[offset..offset + 4].copy_from_slice(&u(value));
        assert!(
            grid_formats::scene::read_draws(&pssg::read_tree(&bad).unwrap(), &meshes, b"LOD0_")
                .is_err()
        );
    }
    let mut bad_mesh = scene_mesh();
    bad_mesh.indices[2] = 5;
    assert!(grid_formats::scene::read_draws(&tree, &[bad_mesh], b"LOD0_").is_err());
    let mut bad = bytes.clone();
    let offset = attribute_offset(&bad, b"RENDERINSTANCESOURCE", b"source");
    bad[offset + 5] = b'x';
    assert!(
        grid_formats::scene::read_draws(&pssg::read_tree(&bad).unwrap(), &meshes, b"LOD0_")
            .is_err()
    );
    let offset = tree.nodes[2].data.unwrap().as_ptr() as usize - bytes.as_ptr() as usize;
    let mut bad = bytes;
    bad[offset + 48..offset + 52].copy_from_slice(&1.0f32.to_be_bytes());
    assert!(
        grid_formats::scene::read_draws(&pssg::read_tree(&bad).unwrap(), &meshes, b"LOD0_")
            .is_err()
    );
}
#[test]
fn reads_rigid_skin_indices_and_rejects_fractional_or_nonfinite_values() {
    let bytes = mesh_fixture(true);
    let tree = pssg::read_tree(&bytes).unwrap();
    assert_eq!(
        mesh::read_meshes(&tree).unwrap()[0]
            .skin_indices
            .as_ref()
            .unwrap(),
        &vec![7.; 3]
    );
    let index = (0..tree.nodes.len())
        .find(|&i| tree.name(i) == b"DATABLOCKDATA")
        .unwrap();
    let offset = tree.nodes[index].data.unwrap().as_ptr() as usize - bytes.as_ptr() as usize + 32;
    for value in [f32::NAN, f32::INFINITY, -1., 1.5, 65536.] {
        let mut bad = bytes.clone();
        bad[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
        assert!(mesh::read_meshes(&pssg::read_tree(&bad).unwrap()).is_err());
    }
}

#[test]
fn exhaustively_decodes_finite_binary16_and_signed_zero() {
    for bits in 0..=u16::MAX {
        let sign = if bits & 0x8000 == 0 { 1.0f32 } else { -1.0 };
        let exponent = (bits >> 10) & 31;
        let fraction = bits & 1023;
        let actual = mesh::half_to_f32(bits);
        if exponent == 31 {
            if fraction == 0 {
                assert!(actual.is_infinite());
            } else {
                assert!(actual.is_nan());
            }
            continue;
        }
        let expected = sign
            * if exponent == 0 {
                (fraction as f32) * 2.0f32.powi(-24)
            } else {
                (1024 + fraction) as f32 * 2.0f32.powi(exponent as i32 - 25)
            };
        assert_eq!(actual.to_bits(), expected.to_bits(), "half {bits:04x}");
    }
}
#[test]
fn accepts_only_count_and_size_validated_complete_bc_chains() {
    let base = [0u8; 32];
    let tail = [0u8; 24];
    let mut texture = texture::Texture {
        id: b"synthetic",
        width: 8,
        height: 8,
        format: texture::Format::Bc1,
        declared_mip_levels: 3,
        base_blocks: &base,
        remaining_data: &tail,
    };
    let (dds, levels) = texture.validated_dds();
    assert_eq!(levels, 4);
    assert_eq!(dds.len(), 184);
    assert_eq!(u32::from_le_bytes(dds[28..32].try_into().unwrap()), 4);
    assert_eq!(&dds[160..], &tail);
    texture.remaining_data = &tail[..16];
    assert_eq!(texture.validated_dds().1, 1);
    texture.remaining_data = &tail;
    texture.declared_mip_levels = 2;
    assert_eq!(texture.validated_dds().1, 1);
}
#[test]
fn preserves_declared_partial_bc_chains_and_rejects_padding_or_impossible_counts() {
    let base = [0u8; 32];
    let tail = [17u8; 16];
    let mut texture = texture::Texture {
        id: b"partial",
        width: 8,
        height: 8,
        format: texture::Format::Bc1,
        declared_mip_levels: 2,
        base_blocks: &base,
        remaining_data: &tail,
    };
    let (dds, levels) = texture.validated_dds();
    assert_eq!(levels, 3);
    assert_eq!(u32::from_le_bytes(dds[28..32].try_into().unwrap()), 3);
    assert_eq!(&dds[128..160], &base);
    assert_eq!(&dds[160..], &tail);
    texture.remaining_data = &tail[..15];
    assert_eq!(texture.validated_dds().1, 1);
    texture.remaining_data = &tail;
    texture.declared_mip_levels = u32::MAX;
    assert_eq!(texture.validated_dds().1, 1);
    texture.declared_mip_levels = 4; // More levels than 8x8 can contain.
    assert_eq!(texture.validated_dds().1, 1);

    // Observed Donington edge-map layout: 256x512 BC3 plus three lower levels.
    let base = vec![0u8; 131_072];
    let tail = vec![29u8; 43_008];
    let texture = texture::Texture {
        id: b"rectangular-partial",
        width: 256,
        height: 512,
        format: texture::Format::Bc3,
        declared_mip_levels: 3,
        base_blocks: &base,
        remaining_data: &tail,
    };
    let (dds, levels) = texture.validated_dds();
    assert_eq!(levels, 4);
    assert_eq!(dds.len(), 128 + 174_080);
    assert_eq!(&dds[128 + base.len()..], &tail);
}

fn material_fixture() -> Vec<u8> {
    let definitions = [
        node(
            "SHADERINPUTDEFINITION",
            &[("name", text("constant")), ("type", text("constant"))],
            vec![],
        ),
        node(
            "SHADERINPUTDEFINITION",
            &[
                ("name", text("TDiffuseAlphaMap")),
                ("type", text("texture")),
            ],
            vec![],
        ),
    ]
    .concat();
    file(
        [
            node(
                "SHADERGROUP",
                &[("id", text("surface.fx")), ("parameterCount", u(2))],
                definitions,
            ),
            node(
                "SHADERINSTANCE",
                &[
                    ("id", text("paint")),
                    ("shaderGroup", text("#surface.fx")),
                    ("parameterSavedCount", u(1)),
                ],
                node(
                    "SHADERINPUT",
                    &[
                        ("parameterID", u(1)),
                        ("type", text("texture")),
                        ("texture", text("#surface.tga")),
                    ],
                    vec![],
                ),
            ),
        ]
        .concat(),
    )
}
#[test]
fn resolves_texture_parameters_by_definition_ordinal_and_checks_types_counts() {
    let bytes = material_fixture();
    let tree = pssg::read_tree(&bytes).unwrap();
    let bindings = grid_formats::material::read_texture_bindings(&tree).unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].sampler, b"TDiffuseAlphaMap");
    assert_eq!(bindings[0].texture, b"surface.tga");
    for (name, attribute, value) in [
        (b"SHADERINPUT".as_slice(), b"parameterID".as_slice(), 0),
        (b"SHADERINPUT", b"parameterID", 2),
        (b"SHADERGROUP", b"parameterCount", 3),
        (b"SHADERINSTANCE", b"parameterSavedCount", 0),
    ] {
        let mut bad = bytes.clone();
        let offset = attribute_offset(&bad, name, attribute);
        bad[offset..offset + 4].copy_from_slice(&u(value));
        assert!(
            grid_formats::material::read_texture_bindings(&pssg::read_tree(&bad).unwrap()).is_err()
        );
    }
}
#[test]
fn composes_static_parent_and_child_transforms_in_row_vector_order() {
    let identity: [f32; 16] = [
        1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
    ];
    let mut parent = identity;
    parent[12] = 5.;
    let child: [f32; 16] = [
        0., 1., 0., 0., -1., 0., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
    ];
    let transform = |m: [f32; 16]| {
        node(
            "TRANSFORM",
            &[],
            m.into_iter().flat_map(f32::to_be_bytes).collect(),
        )
    };
    let draw = node(
        "RENDERSTREAMINSTANCE",
        &[
            ("id", text("draw")),
            ("sourceCount", u(1)),
            ("streamCount", u(0)),
            ("indices", text("#triangle")),
            ("shader", text("#paint")),
        ],
        node(
            "RENDERINSTANCESOURCE",
            &[("source", text("#triangle"))],
            vec![],
        ),
    );
    let render = node(
        "RENDERNODE",
        &[("id", text("render"))],
        [transform(child), draw].concat(),
    );
    let parent = node(
        "NODE",
        &[("id", text("parent"))],
        [transform(parent), render].concat(),
    );
    let bytes = file(
        [
            node(
                "ROOTNODE",
                &[("id", text("root"))],
                [transform(identity), parent].concat(),
            ),
            node("SHADERINSTANCE", &[("id", text("paint"))], vec![]),
        ]
        .concat(),
    );
    let tree = pssg::read_tree(&bytes).unwrap();
    let meshes = [scene_mesh()];
    let draws = grid_formats::scene::read_static_draws(&tree, &meshes).unwrap();
    assert_eq!(draws[0].indices, [2, 3, 4]);
    assert_eq!(
        grid_formats::scene::transform_point(&draws[0].transform, [1., 0., 0.]),
        [5., 1., 0.]
    );
}

fn ornament_fixture() -> Vec<u8> {
    let mut bytes = vec![0u8; 155];
    for (offset, value) in [
        (28, 2u32),
        (32, 40),
        (36, 1),
        (40, 148),
        (68, 2),
        (72, 0),
        (76, 92),
        (80, 1),
        (84, u32::MAX),
        (88, 123),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    for (i, value) in [1.0f32, 0., 0., 0., 1., 0., 0., 0., 1., 10., 20., 30.]
        .into_iter()
        .enumerate()
    {
        bytes[92 + i * 4..96 + i * 4].copy_from_slice(&value.to_le_bytes());
    }
    bytes[140..144].copy_from_slice(&456u32.to_le_bytes());
    bytes[148..].copy_from_slice(b"object\0");
    bytes
}
#[test]
fn checks_ornament_boundaries_names_contiguity_and_preserves_unknown_metadata() {
    let bytes = ornament_fixture();
    let ornaments = grid_formats::ornaments::read(&bytes).unwrap();
    assert_eq!(ornaments.total_instances, 2);
    assert_eq!(ornaments.prototypes[0].name, b"object");
    assert_eq!(ornaments.prototypes[0].transform_metadata, [[456, 0]]);
    assert_eq!(
        grid_formats::scene::transform_point(&ornaments.prototypes[0].transforms[0], [1., 2., 3.]),
        [11., 22., 33.]
    );
    for end in 0..bytes.len() {
        assert!(grid_formats::ornaments::read(&bytes[..end]).is_err());
    }
    for (offset, value) in [
        (28, 3u32),
        (32, 41),
        (36, u32::MAX),
        (40, 9999),
        (72, 1),
        (76, 93),
        (80, 3),
    ] {
        let mut bad = bytes.clone();
        bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(grid_formats::ornaments::read(&bad).is_err());
    }
    let mut bad = bytes;
    bad[92..96].copy_from_slice(&f32::NAN.to_le_bytes());
    assert!(grid_formats::ornaments::read(&bad).is_err());
}
#[test]
fn restricts_opaque_data_leaf_to_pnstring_context() {
    let bytes = file(node(
        "PNSTRING",
        &[],
        node("DATA", &[], b"synthetic\0".to_vec()),
    ));
    let tree = pssg::read_tree(&bytes).unwrap();
    assert_eq!(tree.nodes[2].data, Some(b"synthetic\0".as_slice()));
    assert!(pssg::read_tree(&file(node("DATA", &[], b"synthetic\0".to_vec()))).is_err());
    let bytes = file(node("TEMPLATETRANSFORM", &[], [0u8; 64].to_vec()));
    let tree = pssg::read_tree(&bytes).unwrap();
    assert_eq!(tree.nodes[1].data.unwrap().len(), 64);
    assert!(grid_formats::scene::decode_matrix(tree.nodes[1].data.unwrap(), 0).is_err());
}
#[test]
fn excludes_skinnable_sources_only_in_explicit_static_subset() {
    let bytes = fixture();
    let value = text("SkinnableVertex");
    let mut tree = pssg::read_tree(&bytes).unwrap();
    let definition = tree
        .framing
        .schema
        .iter()
        .flat_map(|e| &e.attributes)
        .find(|a| a.name == b"renderType")
        .unwrap()
        .id;
    let descriptor = (0..tree.nodes.len())
        .find(|&i| tree.name(i) == b"DATABLOCKSTREAM")
        .unwrap();
    tree.nodes[descriptor]
        .attributes
        .iter_mut()
        .find(|a| a.id == definition)
        .unwrap()
        .value = &value;
    assert!(mesh::read_meshes(&tree).is_err());
    assert!(mesh::read_static_meshes(&tree).unwrap().is_empty());
}

fn entity_fixture() -> (Vec<u8>, Vec<u8>) {
    let matrix = [
        1.0f32, 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 10., 20., 30., 1.,
    ];
    let transform = || {
        node(
            "TEMPLATETRANSFORM",
            &[],
            matrix.iter().flat_map(|v| v.to_be_bytes()).collect(),
        )
    };
    let placements = file(
        [
            node(
                "TEMPLATEENTITYREFERENCE",
                &[
                    ("id", text("reference")),
                    ("uri", text("objecttypes.pssg#template")),
                ],
                vec![],
            ),
            node(
                "TEMPLATEENTITYINSTANCE",
                &[("id", text("placement")), ("uri", text("#reference"))],
                [transform(), node("TEMPLATEENTITYLINK", &[], vec![])].concat(),
            ),
        ]
        .concat(),
    );
    let body = node(
        "TEMPLATERIGIDBODY",
        &[
            ("id", text("body")),
            ("props", text("physparams.xml#rbDefault")),
            ("infMass", u(1)),
        ],
        node(
            "TEMPLATERIGIDBODYSHAPE",
            &[("id", text("shape_instance")), ("uri", text("#box"))],
            transform(),
        ),
    );
    let subentity = node(
        "TEMPLATESUBENTITY",
        &[("id", text("intact"))],
        [
            body,
            node(
                "TEMPLATETRANSFORMABLE",
                &[("uri", text("#renderable")), ("sid", text("body_node"))],
                vec![],
            ),
        ]
        .concat(),
    );
    let template = node(
        "TEMPLATEENTITY",
        &[("id", text("template"))],
        [
            subentity,
            node("TEMPLATESUBENTITY", &[("id", text("broken"))], vec![]),
            node("TEMPLATECLOTH", &[("id", text("cloth"))], vec![]),
            node(
                "TEMPLATEBEHAVIOUR",
                &[("typeuri", text("#initial"))],
                vec![],
            ),
        ]
        .concat(),
    );
    let changes = [
        ("add", "broken"),
        ("add", "intact"),
        ("delete", "broken"),
        ("add", "cloth"),
    ]
    .iter()
    .flat_map(|(op, id)| {
        node(
            "TEMPLATECOMPONENTCHANGE",
            &[
                ("operation", text(op)),
                ("componenturi", text(&format!("#{id}"))),
            ],
            vec![],
        )
    })
    .collect();
    let types = file(
        [
            template,
            node("TEMPLATEBEHAVIOURTYPE", &[("id", text("initial"))], changes),
            node(
                "TEMPLATERENDERABLE",
                &[
                    ("id", text("renderable")),
                    ("uri", text("objects.pssg#intact_node")),
                ],
                vec![],
            ),
            node(
                "TEMPLATESHAPEBOX",
                &[
                    ("id", text("box")),
                    ("width", u(2.0f32.to_bits())),
                    ("height", u(3.0f32.to_bits())),
                    ("length", u(4.0f32.to_bits())),
                    ("shell", u(0.02f32.to_bits())),
                    ("phantom", u(0)),
                    ("props", text("physparams.xml#sDefault")),
                ],
                vec![],
            ),
        ]
        .concat(),
    );
    (placements, types)
}
#[test]
fn resolves_startup_state_without_rendering_deleted_components() {
    let (p, t) = entity_fixture();
    let p = pssg::read_tree(&p).unwrap();
    let t = pssg::read_tree(&t).unwrap();
    let e = grid_formats::entities::read(&p, &t).unwrap();
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].links, 1);
    assert_eq!(e[0].cloth, [b"cloth".as_slice()]);
    assert_eq!(e[0].transform[12..15], [10., 20., 30.]);
    assert_eq!(e[0].renderables.len(), 1);
    let r = &e[0].renderables[0];
    assert_eq!(r.component, b"intact");
    assert_eq!(r.node, b"intact_node");
    assert!(r.infinite_mass);
    assert_eq!(r.shapes.len(), 1);
    assert!(matches!(
        r.shapes[0].primitive,
        grid_formats::entities::Primitive::Box {
            width: 2.,
            height: 3.,
            length: 4.
        }
    ));
    assert_eq!(r.shapes[0].transform[12..15], [10., 20., 30.]);
}
#[test]
fn rejects_bad_entity_references_states_and_collision_dimensions() {
    let (p, t) = entity_fixture();
    let p = pssg::read_tree(&p).unwrap();
    for (kind, field, value) in [
        ("TEMPLATERENDERABLE", "uri", text("objects.pssg#")),
        ("TEMPLATECOMPONENTCHANGE", "operation", text("toggle")),
        ("TEMPLATECOMPONENTCHANGE", "componenturi", text("#box")),
        ("TEMPLATETRANSFORMABLE", "uri", text("#absent")),
        ("TEMPLATESHAPEBOX", "width", u(f32::NAN.to_bits())),
        ("TEMPLATESHAPEBOX", "height", u(0)),
        ("TEMPLATERIGIDBODY", "infMass", u(2)),
    ] {
        let mut tree = pssg::read_tree(&t).unwrap();
        let aid = tree
            .framing
            .schema
            .iter()
            .flat_map(|e| &e.attributes)
            .find(|a| a.name == field.as_bytes())
            .unwrap()
            .id;
        let i = (0..tree.nodes.len())
            .find(|&i| tree.name(i) == kind.as_bytes())
            .unwrap();
        tree.nodes[i]
            .attributes
            .iter_mut()
            .find(|a| a.id == aid)
            .unwrap()
            .value = &value;
        assert!(
            grid_formats::entities::read(&p, &tree).is_err(),
            "{kind}/{field}"
        );
    }
}

#[test]
fn selects_renderable_subtree_and_excludes_its_external_ancestry() {
    let mut root = [
        1.0f32, 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 100., 0., 0., 1.,
    ];
    let transform = |m: [f32; 16]| {
        node(
            "TRANSFORM",
            &[],
            m.iter().flat_map(|v| v.to_be_bytes()).collect(),
        )
    };
    let draw = node(
        "RENDERSTREAMINSTANCE",
        &[
            ("id", text("draw")),
            ("sourceCount", u(1)),
            ("streamCount", u(0)),
            ("indices", text("#triangle")),
            ("shader", text("#paint")),
        ],
        node(
            "RENDERINSTANCESOURCE",
            &[("source", text("#triangle"))],
            vec![],
        ),
    );
    let identity = [
        1.0f32, 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
    ];
    let mut selected = identity;
    selected[12] = 2.;
    let render = node(
        "RENDERNODE",
        &[("id", text("render"))],
        [transform(identity), draw].concat(),
    );
    let subtree = node(
        "NODE",
        &[("id", text("selected"))],
        [transform(selected), render].concat(),
    );
    let empty = node("NODE", &[("id", text("empty"))], transform(identity));
    let bytes = file(
        [
            node(
                "ROOTNODE",
                &[("id", text("root"))],
                [transform(root), subtree, empty].concat(),
            ),
            node("SHADERINSTANCE", &[("id", text("paint"))], vec![]),
        ]
        .concat(),
    );
    let tree = pssg::read_tree(&bytes).unwrap();
    let meshes = [scene_mesh()];
    let draws = grid_formats::scene::read_renderable_draws(&tree, &meshes, b"selected").unwrap();
    assert_eq!(draws.len(), 1);
    assert_eq!(draws[0].transform[12], 2.);
    assert_eq!(
        grid_formats::scene::read_static_draws(&tree, &meshes).unwrap()[0].transform[12],
        102.
    );
    assert!(
        grid_formats::scene::read_renderable_draws(&tree, &meshes, b"empty")
            .unwrap()
            .is_empty()
    );
    assert!(grid_formats::scene::read_renderable_draws(&tree, &meshes, b"missing").is_err());
    root[12] = f32::NAN;
    assert!(
        grid_formats::scene::decode_matrix(
            &root
                .iter()
                .flat_map(|v| v.to_be_bytes())
                .collect::<Vec<_>>(),
            0
        )
        .is_err()
    );
}
