use grid_formats::{
    binxml::{Document, Element},
    progress,
};
fn fixture() -> Document<'static> {
    let specs = [
        (
            "progress_track_data",
            None,
            vec![("exporter_version", "2.0.0")],
            1..5,
        ),
        (
            "track",
            None,
            vec![("type", "circuit"), ("total_distance", "100")],
            0..0,
        ),
        ("routes", None, vec![("num_routes", "1")], 5..6),
        ("gates", None, vec![("num_gates", "2")], 7..9),
        ("points", None, vec![("num_points", "2")], 13..15),
        (
            "route",
            None,
            vec![("id", "0"), ("direction", "forwards"), ("num_splits", "1")],
            6..7,
        ),
        (
            "split",
            None,
            vec![("id", "0"), ("type", "finish"), ("gate", "0")],
            0..0,
        ),
        ("gate", None, vec![("id", "0"), ("distance", "0")], 9..11),
        ("gate", None, vec![("id", "1"), ("distance", "50")], 11..13),
        ("left", Some("0 0 -5"), vec![("format", "float3")], 0..0),
        ("right", Some("0 0 5"), vec![("format", "float3")], 0..0),
        ("left", Some("50 0 -5"), vec![("format", "float3")], 0..0),
        ("right", Some("50 0 5"), vec![("format", "float3")], 0..0),
        ("point", None, vec![("id", "0"), ("distance", "0")], 15..16),
        ("point", None, vec![("id", "1"), ("distance", "50")], 16..17),
        ("position", Some("0 0 0"), vec![("format", "float3")], 0..0),
        ("position", Some("50 0 0"), vec![("format", "float3")], 0..0),
    ];
    Document {
        strings: vec![],
        string_padding: &[],
        elements: specs
            .into_iter()
            .map(|(name, value, attributes, children)| Element {
                name,
                value,
                attributes,
                children,
                parent: None,
            })
            .collect(),
    }
}
#[test]
fn resolves_progress_coordinates_distances_and_split_references() {
    let d = fixture();
    let p = progress::read(&d).unwrap();
    assert_eq!(p.total_distance, 100.0);
    assert_eq!(p.gates[1].left, [50.0, 0.0, -5.0]);
    assert_eq!(p.points[1].distance, 50.0);
    assert_eq!(p.routes[0].splits[0].kind, progress::SplitKind::Finish);
    assert_eq!(p.routes[0].splits[0].gate, 0);
}
#[test]
fn rejects_invalid_progress_counts_references_and_geometry() {
    let mut d = fixture();
    d.elements[3].attributes[0].1 = "3";
    assert!(progress::read(&d).is_err());
    let mut d = fixture();
    d.elements[6].attributes[2].1 = "2";
    assert!(progress::read(&d).is_err());
    let mut d = fixture();
    d.elements[8].attributes[1].1 = "0";
    assert!(progress::read(&d).is_err());
    let mut d = fixture();
    d.elements[10].value = d.elements[9].value;
    assert!(progress::read(&d).is_err());
    let mut d = fixture();
    d.elements[16].value = Some("NaN 0 0");
    assert!(progress::read(&d).is_err());
    let mut d = fixture();
    d.elements[5].attributes[1].1 = "backwards";
    assert!(progress::read(&d).is_err());
}
