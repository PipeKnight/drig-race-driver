//! Selected saved track color controls. GPU preparation and live selection are separate.
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

pub fn read(bytes: &[u8], effect: &str) -> Result<Grade> {
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
    let number = |key, minimum: f32, maximum: f32| -> Result<f32> {
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
    };
    if values.contains_key("brighness") && values.contains_key("brightness") {
        return fail(0, "ambiguous grade brightness");
    }
    let tint: Vec<_> = values
        .get("tintColour")
        .ok_or(Error {
            offset: 0,
            message: "missing grade tint",
        })?
        .split(',')
        .map(|v| v.trim().parse::<u8>())
        .collect::<std::result::Result<_, _>>()
        .map_err(|_| Error {
            offset: 0,
            message: "invalid grade tint",
        })?;
    if tint.len() != 4 {
        return fail(0, "expected RGBA grade tint");
    }
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
        tint_rgb: std::array::from_fn(|i| f32::from(tint[i]) / 255.),
    })
}
