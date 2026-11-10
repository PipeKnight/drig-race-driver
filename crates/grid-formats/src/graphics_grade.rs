//! Selected saved track color controls and transition ColourBalance effects. GPU preparation and live selection are separate.
use crate::{Error, Result, fail};
use quick_xml::{Reader, events::Event};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug)]
pub struct Grade {
    pub saturation: f32,
    pub contrast: f32,
    pub brightness: f32,
    pub gamma: f32,
    pub maximum: f32,
    pub tint_amount: f32,
    pub tint_rgb: [f32; 3],
}

/// The named effect's own ColourBalance parameters, as text (no template).
fn parameters(bytes: &[u8], effect: &str) -> Result<BTreeMap<String, String>> {
    if bytes.len() > 1024 * 1024 {
        return fail(0, "grade XML exceeds bound");
    }
    let mut reader = Reader::from_reader(bytes);
    let mut depth = 0usize;
    let mut selected = None;
    let mut group = None;
    let mut effects = 0;
    let mut groups = 0;
    let mut parameter: Option<(String, String)> = None;
    let mut values = BTreeMap::new();
    loop {
        let event = reader.read_event().map_err(|_| Error {
            offset: reader.buffer_position() as usize,
            message: "invalid grade XML",
        })?;
        match event {
            Event::Start(node) => {
                depth += 1;
                let mut name = None;
                for attr in node.attributes() {
                    let attr = attr.map_err(|_| Error {
                        offset: 0,
                        message: "invalid grade attribute",
                    })?;
                    if attr.key.as_ref() == b"name" {
                        name = Some(
                            attr.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                                .map_err(|_| Error {
                                    offset: 0,
                                    message: "invalid grade name",
                                })?
                                .into_owned(),
                        );
                    }
                }
                match node.name().as_ref() {
                    b"Effect" if name.as_deref() == Some(effect) => {
                        effects += 1;
                        selected = Some(depth);
                    }
                    b"ParameterGroup"
                        if selected.is_some() && name.as_deref() == Some("ColourBalance") =>
                    {
                        groups += 1;
                        group = Some(depth);
                    }
                    b"Param" if group.is_some() => {
                        if parameter.is_some() {
                            return fail(0, "nested grade parameter");
                        }
                        parameter = Some((
                            name.ok_or(Error {
                                offset: 0,
                                message: "unnamed grade parameter",
                            })?,
                            String::new(),
                        ));
                    }
                    _ => {}
                }
            }
            Event::Text(text) => {
                if let Some((_, value)) = &mut parameter {
                    value.push_str(std::str::from_utf8(text.as_ref()).map_err(|_| Error {
                        offset: 0,
                        message: "invalid grade text",
                    })?);
                }
            }
            Event::End(node) => {
                if node.name().as_ref() == b"Param"
                    && let Some((key, value)) = parameter.take()
                    && values.insert(key, value.trim().to_owned()).is_some()
                {
                    return fail(0, "duplicate grade parameter");
                }
                if group == Some(depth) {
                    group = None;
                }
                if selected == Some(depth) {
                    selected = None;
                }
                depth = depth.checked_sub(1).ok_or(Error {
                    offset: 0,
                    message: "invalid grade nesting",
                })?;
            }
            Event::DocType(_) | Event::GeneralRef(_) | Event::CData(_) => {
                return fail(0, "unsupported grade XML content");
            }
            Event::Eof => break,
            _ => {}
        }
    }
    if depth != 0 || effects != 1 || groups != 1 {
        return fail(0, "expected one selected effect and color group");
    }
    if values.contains_key("brighness") && values.contains_key("brightness") {
        return fail(0, "ambiguous grade brightness");
    }
    Ok(values)
}

fn number(values: &BTreeMap<String, String>, key: &str, minimum: f32, maximum: f32) -> Result<f32> {
    let value = values
        .get(key)
        .ok_or(Error {
            offset: 0,
            message: "missing grade control",
        })?
        .parse::<f32>()
        .map_err(|_| Error {
            offset: 0,
            message: "invalid grade number",
        })?;
    if !value.is_finite() || !(minimum..=maximum).contains(&value) {
        return fail(0, "grade control outside bound");
    }
    Ok(value)
}

fn colour(values: &BTreeMap<String, String>, key: &str) -> Result<[f32; 3]> {
    let rgba: Vec<_> = values
        .get(key)
        .ok_or(Error {
            offset: 0,
            message: "missing grade colour",
        })?
        .split(',')
        .map(|v| v.trim().parse::<u8>())
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| Error {
            offset: 0,
            message: "invalid grade colour",
        })?;
    if rgba.len() != 4 {
        return fail(0, "expected RGBA grade colour");
    }
    Ok(std::array::from_fn(|i| f32::from(rgba[i]) / 255.))
}

pub fn read(bytes: &[u8], effect: &str) -> Result<Grade> {
    let values = parameters(bytes, effect)?;
    let number = |key, minimum, maximum| number(&values, key, minimum, maximum);
    let tint = colour(&values, "tintColour")?;
    Ok(Grade {
        saturation: number("colour", 0., 2.)?,
        contrast: number("contrast", 0., 4.)?,
        brightness: number(
            if values.contains_key("brighness") {
                "brighness"
            } else {
                "brightness"
            },
            -1.,
            1.,
        )?,
        gamma: number("gamma", 0.1, 4.)?,
        maximum: number("maxbrightness", 0.001, 32.)?,
        tint_amount: number("tintAmount", 0., 1.)?,
        tint_rgb: tint,
    })
}

/// One effect's ColourBalance as CheapFinalMerge applies it (docs/FLASHBACK.md
/// §2.5): `min(maximum, lerp(vignette, mix(luma, c, colour) * pre_tint) *
/// contrast + brightness)`, no gamma. Parameters the effect leaves to its
/// template take the neutral value: the port's ordinary image stands for
/// the template (Effect_0000), so only the effect's own changes are drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Balance {
    pub contrast: f32,
    pub brightness: f32,
    pub maximum: f32,
    pub colour: f32,
    pub tint_amount: f32,
    pub tint_rgb: [f32; 3],
    pub vignette_amount: f32,
    pub vignette_shape: f32,
    pub vignette_rgb: [f32; 3],
    pub vignette_scale: [f32; 2],
    pub vignette_offset: [f32; 2],
    /// Degrees.
    pub vignette_rotation: f32,
    /// vignetteTransition: the width of the vignette's edge.
    pub vignette_transition: f32,
}

impl Balance {
    /// Leaves the image as it is.
    pub const NEUTRAL: Self = Self {
        contrast: 1.,
        brightness: 0.,
        maximum: 32.,
        colour: 1.,
        tint_amount: 0.,
        tint_rgb: [1.; 3],
        vignette_amount: 0.,
        vignette_shape: 1.,
        vignette_rgb: [0.; 3],
        vignette_scale: [1.; 2],
        vignette_offset: [0.; 2],
        vignette_rotation: 0.,
        vignette_transition: 1.,
    };

    /// Per-parameter linear blend (the original's blend is untraced).
    pub fn lerp(&self, to: &Self, t: f32) -> Self {
        let t = t.clamp(0., 1.);
        // Exact at both ends.
        let f = |a: f32, b: f32| a * (1. - t) + b * t;
        let v3 = |a: [f32; 3], b: [f32; 3]| std::array::from_fn(|i| f(a[i], b[i]));
        let v2 = |a: [f32; 2], b: [f32; 2]| std::array::from_fn(|i| f(a[i], b[i]));
        Self {
            contrast: f(self.contrast, to.contrast),
            brightness: f(self.brightness, to.brightness),
            maximum: f(self.maximum, to.maximum),
            colour: f(self.colour, to.colour),
            tint_amount: f(self.tint_amount, to.tint_amount),
            tint_rgb: v3(self.tint_rgb, to.tint_rgb),
            vignette_amount: f(self.vignette_amount, to.vignette_amount),
            vignette_shape: f(self.vignette_shape, to.vignette_shape),
            vignette_rgb: v3(self.vignette_rgb, to.vignette_rgb),
            vignette_scale: v2(self.vignette_scale, to.vignette_scale),
            vignette_offset: v2(self.vignette_offset, to.vignette_offset),
            vignette_rotation: f(self.vignette_rotation, to.vignette_rotation),
            vignette_transition: f(self.vignette_transition, to.vignette_transition),
        }
    }
}

/// The effect's own ColourBalance; absent parameters stay neutral.
pub fn read_balance(bytes: &[u8], effect: &str) -> Result<Balance> {
    let values = parameters(bytes, effect)?;
    let n = Balance::NEUTRAL;
    let number = |key: &str, minimum, maximum, neutral| {
        if values.contains_key(key) {
            number(&values, key, minimum, maximum)
        } else {
            Ok(neutral)
        }
    };
    let colour = |key: &str, neutral| {
        if values.contains_key(key) {
            colour(&values, key)
        } else {
            Ok(neutral)
        }
    };
    let brightness = if values.contains_key("brighness") { "brighness" } else { "brightness" };
    Ok(Balance {
        contrast: number("contrast", 0., 4., n.contrast)?,
        brightness: number(brightness, -1., 1., n.brightness)?,
        maximum: number("maxbrightness", 0., 32., n.maximum)?,
        colour: number("colour", 0., 2., n.colour)?,
        tint_amount: number("tintAmount", 0., 1., n.tint_amount)?,
        tint_rgb: colour("tintColour", n.tint_rgb)?,
        vignette_amount: number("vignetteAmount", 0., 1., n.vignette_amount)?,
        vignette_shape: number("vignetteShape", 0., 4., n.vignette_shape)?,
        vignette_rgb: colour("vignetteColour", n.vignette_rgb)?,
        vignette_scale: [
            number("vignetteScaleX", 0.01, 16., n.vignette_scale[0])?,
            number("vignetteScaleY", 0.01, 16., n.vignette_scale[1])?,
        ],
        vignette_offset: [
            number("vignetteOffsetX", -4., 4., n.vignette_offset[0])?,
            number("vignetteOffsetY", -4., 4., n.vignette_offset[1])?,
        ],
        vignette_rotation: number("vignetteRotation", -360., 360., n.vignette_rotation)?,
        vignette_transition: number("vignetteTransition", 0.01, 16., n.vignette_transition)?,
    })
}
