use grid_formats::binxml;
fn section(tag: u32, data: Vec<u8>) -> Vec<u8> {
    let mut b = tag.to_le_bytes().to_vec();
    b.extend((data.len() as u32).to_le_bytes());
    b.extend(data);
    b
}
fn fixture() -> Vec<u8> {
    let values = ["root", "child", "a", "值", "text"];
    let mut strings = Vec::new();
    let mut offsets = Vec::new();
    for s in values {
        offsets.extend((strings.len() as u32).to_le_bytes());
        strings.extend(s.as_bytes());
        strings.push(0);
    }
    strings.extend([0, 0, 0]);
    let group = [section(0x7252221d, strings), section(0x7252221e, offsets)].concat();
    let elements: [[u32; 6]; 2] = [[0, 0, 1, 0, 1, 1], [1, 4, 1, 1, 0, 0]];
    let attributes: [[u32; 2]; 2] = [[2, 3], [2, 0]];
    let mut b = 0x7252221au32.to_le_bytes().to_vec();
    b.extend(0u32.to_le_bytes());
    b.extend(0x72522217u32.to_le_bytes());
    b.extend((group.len() as u32).to_le_bytes());
    b.extend(group);
    b.extend(section(
        0x7252221b,
        elements
            .into_iter()
            .flatten()
            .flat_map(u32::to_le_bytes)
            .collect(),
    ));
    b.extend(section(
        0x7252221c,
        attributes
            .into_iter()
            .flatten()
            .flat_map(u32::to_le_bytes)
            .collect(),
    ));
    let len = b.len() as u32 - 8;
    b[4..8].copy_from_slice(&len.to_le_bytes());
    b
}
fn sections(bytes: &[u8]) -> [usize; 4] {
    let mut p = 16;
    std::array::from_fn(|_| {
        let start = p;
        let size = u32::from_le_bytes(bytes[p + 4..p + 8].try_into().unwrap());
        p += 8 + size as usize;
        start
    })
}
#[test]
fn resolves_binary_xml_utf8_strings_attributes_values_and_tree_ranges() {
    let bytes = fixture();
    let doc = binxml::read(&bytes).unwrap();
    assert_eq!(doc.strings, ["root", "child", "a", "值", "text"]);
    assert_eq!(doc.string_padding, [0, 0, 0]);
    assert_eq!(doc.elements[0].name, "root");
    assert_eq!(doc.elements[0].value, None);
    assert_eq!(doc.elements[0].attributes, [("a", "值")]);
    assert_eq!(doc.elements[1].value, Some("text"));
    assert_eq!(doc.elements[1].attributes, [("a", "root")]);
    assert_eq!(doc.elements[1].parent, Some(0));
}
#[test]
fn rejects_binary_xml_truncations_sections_ids_utf8_and_invalid_ownership() {
    let bytes = fixture();
    let [strings, offsets, elements, attributes] = sections(&bytes);
    for end in 0..bytes.len() {
        assert!(binxml::read(&bytes[..end]).is_err(), "prefix {end}");
    }
    for (offset, value) in [
        (4, 0u32),
        (8, 0),
        (12, 0),
        (strings, 0),
        (offsets + 8, 1),
        (elements + 8, u32::MAX),
        (elements + 8 + 16, 3),
        (elements + 8 + 24 + 20, 3),
        (elements + 8 + 24 + 16, 1),
        (attributes + 8, 999),
    ] {
        let mut bad = bytes.clone();
        bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(binxml::read(&bad).is_err(), "word {offset}");
    }
    for offset in [strings + 8 + 13, offsets - 1] {
        let mut bad = bytes.clone();
        bad[offset] = 0xff;
        assert!(binxml::read(&bad).is_err());
    }
    let mut bad = bytes;
    bad[elements + 8 + 24 + 12..elements + 8 + 24 + 16].copy_from_slice(&0u32.to_le_bytes());
    assert!(binxml::read(&bad).is_err());
}
