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

#[test]
fn transition_balance_reads_vignette_and_leaves_template_parameters_neutral() {
    let input = "<PostProcessEffects><Effect name=\"white\" template=\"base\"><ParameterGroup name=\"ColourBalance\"><Param name=\"contrast\">0.5</Param><Param name=\"brighness\">0.75</Param><Param name=\"maxbrightness\">0.75</Param><Param name=\"gamma\">1.0</Param></ParameterGroup></Effect><Effect name=\"fx\"><ParameterGroup name=\"ColourBalance\"><Param name=\"colour\">0.4156</Param><Param name=\"tintAmount\">0.2987</Param><Param name=\"tintColour\">255, 127, 1, 255</Param><Param name=\"vignetteAmount\">0.6623</Param><Param name=\"vignetteShape\">0.8571</Param><Param name=\"vignetteColour\">0, 0, 0, 255</Param><Param name=\"vignetteScaleX\">0.961</Param><Param name=\"vignetteOffsetY\">0.2338</Param><Param name=\"vignetteRotation\">18.7</Param></ParameterGroup></Effect></PostProcessEffects>";
    let white = graphics_grade::read_balance(input.as_bytes(), "white").unwrap();
    assert_eq!((white.contrast, white.brightness, white.maximum), (0.5, 0.75, 0.75));
    assert_eq!(white.colour, 1.);
    assert_eq!(white.vignette_amount, 0.);
    let fx = graphics_grade::read_balance(input.as_bytes(), "fx").unwrap();
    assert_eq!(fx.tint_rgb, [1., 127. / 255., 1. / 255.]);
    assert_eq!((fx.vignette_amount, fx.vignette_shape), (0.6623, 0.8571));
    assert_eq!(fx.vignette_scale, [0.961, 1.]);
    assert_eq!(fx.vignette_offset, [0., 0.2338]);
    assert_eq!(fx.vignette_rotation, 18.7);
    assert_eq!(fx.contrast, 1.);
    let half = graphics_grade::Balance::NEUTRAL.lerp(&white, 0.5);
    assert_eq!(half.contrast, 0.75);
    assert_eq!(graphics_grade::Balance::NEUTRAL.lerp(&white, 2.), white);
    assert!(graphics_grade::read_balance(input.as_bytes(), "missing").is_err());
    assert!(graphics_grade::read_balance(input.replace("0.6623", "NaN").as_bytes(), "fx").is_err());
}
