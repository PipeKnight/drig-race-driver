use grid_formats::{jpk, pssg};

fn put_be(b: &mut Vec<u8>, n: u32) {
    b.extend(n.to_be_bytes());
}
fn set_be(b: &mut [u8], p: usize, n: u32) {
    b[p..p + 4].copy_from_slice(&n.to_be_bytes());
}
fn set_le(b: &mut [u8], p: usize, n: u32) {
    b[p..p + 4].copy_from_slice(&n.to_le_bytes());
}

// Entirely synthetic files; no GRID content is embedded in public tests.
fn pssg_fixture() -> Vec<u8> {
    let mut b = b"PSSG".to_vec();
    for n in [0, 1, 1, 1, 4] {
        put_be(&mut b, n);
    }
    b.extend(b"TEST");
    for n in [1, 1, 1] {
        put_be(&mut b, n);
    }
    b.extend(b"x");
    for n in [1, 17, 9, 1, 1] {
        put_be(&mut b, n);
    }
    b.extend([7, 0xaa, 0xbb, 0xcc, 0xdd]);
    let size = b.len() as u32 - 8;
    set_be(&mut b, 4, size);
    b
}
fn jpk_fixture() -> Vec<u8> {
    let mut b = vec![0; 71];
    b[..4].copy_from_slice(b"JPAK");
    set_le(&mut b, 8, 1);
    set_le(&mut b, 12, 16);
    set_le(&mut b, 32, 64);
    set_le(&mut b, 36, 3);
    set_le(&mut b, 40, 68);
    set_le(&mut b, 44, 3);
    b[64..68].copy_from_slice(b"foo\0");
    b[68..].copy_from_slice(&[1, 2, 3]);
    b
}

#[test]
fn preserves_opaque_values_and_ranges() {
    let b = pssg_fixture();
    let p = pssg::inspect(&b).unwrap();
    assert_eq!(p.schema[0].name, b"TEST");
    assert_eq!(p.root_attributes[0].value, &[7]);
    assert_eq!(&b[p.root_payload], &[0xaa, 0xbb, 0xcc, 0xdd]);
    let b = jpk_fixture();
    let p = jpk::inspect(&b).unwrap();
    assert_eq!(p.entries[0].name, b"foo");
    assert_eq!(&b[p.entries[0].stored_data.clone()], &[1, 2, 3]);
}
#[test]
fn every_truncated_prefix_is_rejected_without_panicking() {
    let p = pssg_fixture();
    for end in 0..p.len() {
        assert!(pssg::inspect(&p[..end]).is_err());
    }
    let j = jpk_fixture();
    for end in 0..j.len() {
        assert!(jpk::inspect(&j[..end]).is_err());
    }
}
#[test]
fn pssg_rejects_out_of_bounds_and_unknown_ids() {
    let base = pssg_fixture();
    // Root starts at 41; its attr length is at 49 and value length at 57.
    for (offset, value) in [
        (8, u32::MAX),
        (12, u32::MAX),
        (16, 0),
        (20, u32::MAX),
        (32, 2),
        (41, 2),
        (45, u32::MAX),
        (49, 18),
        (57, 2),
    ] {
        let mut b = base.clone();
        set_be(&mut b, offset, value);
        assert!(pssg::inspect(&b).is_err(), "accepted mutation at {offset}");
    }
}
#[test]
fn jpk_rejects_invalid_counts_offsets_and_names() {
    let base = jpk_fixture();
    for (offset, value) in [
        (4, 1),
        (8, u32::MAX),
        (32, 0),
        (32, u32::MAX),
        (40, 0),
        (40, u32::MAX),
        (44, u32::MAX),
    ] {
        let mut b = base.clone();
        set_le(&mut b, offset, value);
        assert!(jpk::inspect(&b).is_err(), "accepted mutation at {offset}");
    }
    let mut b = base;
    b[67] = 1;
    assert!(jpk::inspect(&b).is_err());
}

#[test]
fn rejects_duplicate_schema_ids() {
    let mut b = pssg_fixture();
    let mut definition = Vec::new();
    for n in [2, 1] {
        put_be(&mut definition, n);
    }
    definition.push(b'q');
    put_be(&mut definition, 0);
    b.splice(41..41, definition);
    let size = b.len() as u32 - 8;
    set_be(&mut b, 4, size);
    set_be(&mut b, 12, 2);
    assert!(pssg::inspect(&b).is_ok());
    set_be(&mut b, 41, 1);
    assert!(pssg::inspect(&b).is_err());

    let mut b = pssg_fixture();
    let mut definition = Vec::new();
    for n in [2, 1] {
        put_be(&mut definition, n);
    }
    definition.push(b'y');
    b.splice(41..41, definition);
    let size = b.len() as u32 - 8;
    set_be(&mut b, 4, size);
    set_be(&mut b, 8, 2);
    set_be(&mut b, 28, 2);
    assert!(pssg::inspect(&b).is_ok());
    set_be(&mut b, 41, 1);
    assert!(pssg::inspect(&b).is_err());
}

#[test]
fn rejects_wrong_magic_and_caps_name_search() {
    let mut p = pssg_fixture();
    p[0] = 0;
    assert!(pssg::inspect(&p).is_err());
    let mut j = jpk_fixture();
    j[0] = 0;
    assert!(jpk::inspect(&j).is_err());
    let mut j = jpk_fixture();
    j.resize(64 + 4098, b'x');
    j[64..].fill(b'x');
    *j.last_mut().unwrap() = 0;
    assert!(jpk::inspect(&j).is_err());
    j[64 + 4096] = 0;
    assert!(jpk::inspect(&j).is_ok());
}
