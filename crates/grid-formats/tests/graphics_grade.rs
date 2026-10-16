use grid_formats::graphics_grade;

fn fixture() -> String {
    "<PostProcessEffects><Effect name=\"outside\"><ParameterGroup name=\"ColourBalance\"><Param name=\"colour\">0.8</Param><Param name=\"contrast\">1.1</Param><Param name=\"brighness\">0.02</Param><Param name=\"gamma\">1.2</Param><Param name=\"maxbrightness\">1</Param><Param name=\"tintAmount\">0.1</Param><Param name=\"tintColour\">250,220,160,255</Param></ParameterGroup></Effect></PostProcessEffects>".into()
}

#[test]
fn selects_saved_color_group_and_preserves_legacy_brightness_spelling() {
    let input = fixture();
    let grade = graphics_grade::read(input.as_bytes(), "outside").unwrap();
    assert_eq!(grade.brightness, 0.02);
    assert_eq!(grade.saturation, 0.8);
    assert_eq!(grade.tint_rgb, [250. / 255., 220. / 255., 160. / 255.]);
    assert!(graphics_grade::read(input.as_bytes(), "inside").is_err());
}

#[test]
fn rejects_ambiguous_nonfinite_incomplete_or_unsafe_grade_inputs() {
    for input in [
        fixture().replace(
            "</ParameterGroup>",
            "<Param name=\"brightness\">0</Param></ParameterGroup>",
        ),
        fixture().replace(
            "<Param name=\"gamma\">1.2</Param>",
            "<Param name=\"gamma\">NaN</Param>",
        ),
        fixture().replace(
            "<Param name=\"gamma\">1.2</Param>",
            "<Param name=\"gamma\">0</Param>",
        ),
        fixture().replace("250,220,160,255", "300,220,160,255"),
        fixture().replace("250,220,160,255", "250,220,160"),
        fixture().replace("<Param name=\"contrast\">1.1</Param>", ""),
        fixture().replace(
            "</ParameterGroup>",
            "<Param name=\"colour\">1</Param></ParameterGroup>",
        ),
        fixture().replace("1.2", "&unknown;"),
        "<!DOCTYPE x>".to_owned() + &fixture(),
        fixture().replace("</Effect>", ""),
    ] {
        assert!(
            graphics_grade::read(input.as_bytes(), "outside").is_err(),
            "{input}"
        );
    }
}
