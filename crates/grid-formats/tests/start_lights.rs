use grid_formats::{
    binxml::{Document, Element},
    start_lights,
};
fn fixture(stages: &[(&'static str, &'static str)], count: &'static str) -> Document<'static> {
    let mut elements = vec![
        Element {
            name: "startLightSequence",
            value: None,
            attributes: vec![],
            children: 1..3,
            parent: None,
        },
        Element {
            name: "lights",
            value: None,
            attributes: vec![("object", "gg_startlights_lights_horiz")],
            children: 3..3,
            parent: Some(0),
        },
        Element {
            name: "sequence",
            value: None,
            attributes: vec![("numStages", count), ("time", "5.0")],
            children: 3..3 + stages.len(),
            parent: Some(0),
        },
    ];
    for (id, texture) in stages {
        elements.push(Element {
            name: "stage",
            value: None,
            attributes: vec![("id", id), ("texture", texture)],
            children: 0..0,
            parent: Some(2),
        });
    }
    Document {
        strings: vec![],
        elements,
        string_padding: &[],
    }
}
#[test]
fn reads_the_stage_textures_in_order() {
    let doc = fixture(&[("1", "a_1_e.tga"), ("2", "a_7_e.tga")], "2");
    let s = start_lights::read(&doc).unwrap();
    assert_eq!(s.object, "gg_startlights_lights_horiz");
    assert_eq!(s.seconds, 5.0);
    assert_eq!(s.stages, ["a_1_e.tga", "a_7_e.tga"]);
}
#[test]
fn rejects_count_and_order_mismatches() {
    assert!(start_lights::read(&fixture(&[("1", "a.tga")], "2")).is_err());
    assert!(start_lights::read(&fixture(&[("2", "a.tga"), ("1", "b.tga")], "2")).is_err());
    assert!(start_lights::read(&fixture(&[], "0")).is_err());
}
