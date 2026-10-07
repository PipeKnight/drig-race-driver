use grid_formats::collision;

fn fixture(triangles: usize) -> Vec<u8> {
    let vertices = 20;
    let vertex_offset = 60;
    let node_offset = vertex_offset + vertices * 8;
    let triangle_offset = node_offset + 2;
    let reference_offset = triangle_offset + triangles * 4;
    let mut b = vec![0u8; reference_offset];
    for (i, v) in [0.0f32, 0., 0., 16., 16., 16.].iter().enumerate() {
        b[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    for (offset, value) in [
        (24, (-(triangles as i32)) as u32),
        (28, (-(vertices as i32)) as u32),
        (32, (-2i32) as u32),
        (36, vertex_offset as u32),
        (40, node_offset as u32),
        (44, triangle_offset as u32),
        (48, reference_offset as u32),
    ] {
        b[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    b[52..60].copy_from_slice(b"STDRGRAS");
    for i in 0..vertices {
        let x = ((i % 16) as u32) << 20;
        let y = (i * 1000) as u16;
        let z = ((i / 16) as u32) << 20;
        let p = vertex_offset + i * 8;
        b[p..p + 3].copy_from_slice(&x.to_be_bytes()[1..]);
        b[p + 3..p + 5].copy_from_slice(&y.to_be_bytes());
        b[p + 5..p + 8].copy_from_slice(&z.to_be_bytes()[1..]);
    }
    b[node_offset..triangle_offset].copy_from_slice(&0x8000u16.to_be_bytes());
    for p in (triangle_offset..reference_offset).step_by(4) {
        b[p..p + 4].copy_from_slice(&[0x01, 0x11, 1, 2]);
    }
    b.extend([0, 0, 255]);
    b
}
#[test]
fn decodes_grid_packed_vertices_indices_materials_and_leaf_references() {
    let bytes = fixture(1);
    let c = collision::read_vcqtc(&bytes).unwrap();
    assert_eq!(c.positions.len(), 20);
    assert_eq!(c.materials, [*b"STDR", *b"GRAS"]);
    assert_eq!(c.positions[17], [1., 17000.0 / 4096., 1.]);
    assert_eq!(c.triangles[0].indices, [17, 18, 19]);
    assert_eq!(c.triangles[0].material, 1);
    assert_eq!(c.triangles[0].flags, 0);
    assert_eq!(c.nodes, [0x8000]);
    assert_eq!(c.references[&0], [0]);
}
#[test]
fn rejects_upper_index_bits_instead_of_treating_them_as_flags() {
    for upper in [0x40, 0x80, 0xc0] {
        let mut bytes = fixture(1);
        bytes[222] |= upper;
        assert!(collision::read_vcqtc(&bytes).is_err());
    }
}
#[test]
fn preserves_large_leaf_delta_continuations_without_emitting_them() {
    let mut bytes = fixture(300);
    bytes.truncate(bytes.len() - 3);
    bytes.extend([0, 0, 254, 1, 255]);
    let c = collision::read_vcqtc(&bytes).unwrap();
    assert_eq!(c.references[&0], [0, 255]);
}
#[test]
fn rejects_truncated_bad_variant_sections_indices_dimensions_and_references() {
    let bytes = fixture(1);
    for end in 0..bytes.len() {
        assert!(
            collision::read_vcqtc(&bytes[..end]).is_err(),
            "prefix {end}"
        );
    }
    for (offset, value) in [
        (0, f32::NAN.to_bits()),
        (12, 0),
        (24, 1),
        (28, i32::MIN as u32),
        (32, (-9i32) as u32),
        (36, 64),
        (40, u32::MAX),
        (44, 0),
        (48, u32::MAX),
    ] {
        let mut bad = bytes.clone();
        bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(collision::read_vcqtc(&bad).is_err(), "word {offset}");
    }
    for (offset, value) in [
        (220, 0x81),
        (221, 1),
        (222, 0xff),
        (223, 0xff),
        (224, 255),
        (226, 1),
        (227, 1),
        (228, 1),
    ] {
        let mut bad = bytes.clone();
        bad[offset] = value;
        assert!(collision::read_vcqtc(&bad).is_err(), "byte {offset}");
    }
}
