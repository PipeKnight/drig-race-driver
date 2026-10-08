//! Bounded steering and visual damage configuration from legacy HDM text.
use crate::{Error, Result, fail};
use quick_xml::{Reader, events::Event};

#[derive(Clone, Copy, Debug)]
pub struct SteeringParameters {
    pub visual_damage: VisualDamageParameters,
    pub maximum_rotation_26c: f32,
    pub maximum_cap_250: f32,
    pub perturbation_gain_270: f32,
    pub ramp_start_274: f32,
    pub ramp_end_278: f32,
    pub low_gain_27c: f32,
    pub cap_threshold_254: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct VisualDamageParameters {
    pub angular_limit_280: f32,
    pub displacement_limit_284: f32,
    pub terminal_percentage_288: f32,
}
impl VisualDamageParameters {
    pub fn scalars(self) -> [f32; 3] {
        [
            self.angular_limit_280,
            self.displacement_limit_284,
            self.terminal_percentage_288,
        ]
    }
}
impl SteeringParameters {
    pub fn scalars(self) -> [f32; 7] {
        [
            self.maximum_rotation_26c,
            self.maximum_cap_250,
            self.perturbation_gain_270,
            self.ramp_start_274,
            self.ramp_end_278,
            self.low_gain_27c,
            self.cap_threshold_254,
        ]
    }
}

fn normalize(text: &str) -> Result<String> {
    let mut output = String::with_capacity(text.len());
    let mut cursor = 0;
    let mut in_tag = false;
    let mut quote = None;
    while cursor < text.len() {
        let remaining = &text[cursor..];
        let byte = text.as_bytes()[cursor];
        if !remaining.starts_with("<!") {
            if let Some(q) = quote {
                if byte == q {
                    quote = None;
                } else if byte == b'<' {
                    return fail(cursor, "markup inside HDM attribute");
                }
            } else if byte == b'<' {
                if in_tag {
                    return fail(cursor, "nested HDM markup");
                }
                in_tag = true;
            } else if in_tag && matches!(byte, b'\'' | b'"') {
                quote = Some(byte);
            } else if byte == b'>' {
                in_tag = false;
            }
            output.push(byte as char);
            cursor += 1;
            continue;
        }
        if in_tag {
            return fail(cursor, "description inside HDM tag");
        }
        let declaration = remaining;
        let end = if declaration.starts_with("<!--") {
            declaration.find("-->").map(|p| p + 3)
        } else if declaration.starts_with("<!DOCTYPE")
            || declaration.starts_with("<!ENTITY")
            || declaration.starts_with("<![")
        {
            return fail(cursor, "unsupported HDM declaration");
        } else {
            declaration.find('>').map(|p| p + 1)
        }
        .ok_or(Error {
            offset: cursor,
            message: "unterminated HDM description",
        })?;
        cursor += end;
    }
    Ok(output)
}

pub fn read(bytes: &[u8]) -> Result<SteeringParameters> {
    if bytes.len() > 65536 || !bytes.is_ascii() || bytes.contains(&0) {
        return fail(0, "unsupported HDM text size or encoding");
    }
    let text = normalize(std::str::from_utf8(bytes).map_err(|_| Error {
        offset: 0,
        message: "invalid HDM text",
    })?)?;
    let mut reader = Reader::from_str(&text);
    reader.config_mut().check_comments = true;
    let mut depth = 0usize;
    let mut roots = 0;
    let mut count = 0;
    let mut suspension = false;
    let mut wheel = false;
    let mut fields = [
        0.,
        0.,
        0.,
        0.,
        f32::from_bits(0x3f733333),
        f32::from_bits(0x3dcccccd),
        0.,
        27.,
        f32::from_bits(0x3ee66666),
        150.,
    ];
    loop {
        let event = reader.read_event().map_err(|_| Error {
            offset: reader.error_position() as usize,
            message: "malformed HDM XML",
        })?;
        let empty = matches!(event, Event::Empty(_));
        match event {
            Event::Start(e) | Event::Empty(e) => {
                count += 1;
                if count > 128 || depth >= 16 {
                    return fail(0, "HDM tree exceeds limits");
                }
                let name = e.name();
                let name = std::str::from_utf8(name.as_ref()).map_err(|_| Error {
                    offset: 0,
                    message: "invalid HDM name",
                })?;
                if name.len() > 64 {
                    return fail(0, "HDM name exceeds limit");
                }
                if depth == 0 {
                    roots += 1;
                    if roots != 1 || name != "VEHICLEHANDLINGDAMAGE" {
                        return fail(0, "unsupported HDM root");
                    }
                }
                let selected = depth == 1 && matches!(name, "SUSPENSION" | "WHEEL");
                if selected {
                    let seen = if name == "SUSPENSION" {
                        &mut suspension
                    } else {
                        &mut wheel
                    };
                    if *seen {
                        return fail(0, "duplicate HDM steering node");
                    }
                    *seen = true;
                }
                for (i, attr) in e.attributes().enumerate() {
                    if i >= 64 {
                        return fail(0, "HDM attributes exceed limit");
                    }
                    let attr = attr.map_err(|_| Error {
                        offset: 0,
                        message: "invalid HDM attribute",
                    })?;
                    let key = std::str::from_utf8(attr.key.as_ref()).map_err(|_| Error {
                        offset: 0,
                        message: "invalid HDM attribute name",
                    })?;
                    if key.len() > 64 || attr.value.len() > 256 || attr.value.contains(&b'&') {
                        return fail(0, "unsupported HDM attribute");
                    }
                    let slot = if !selected {
                        None
                    } else {
                        match (name, key) {
                            ("SUSPENSION", "maxWheelLockScale") => Some(1),
                            ("SUSPENSION", "damageToStartEffect") => Some(6),
                            ("WHEEL", "maxMisalignAngle") => Some(0),
                            ("WHEEL", "maxSteerWobbleScale") => Some(2),
                            ("WHEEL", "damageToStartBaseEffect") => Some(3),
                            ("WHEEL", "damageToHaveFullEffect") => Some(4),
                            ("WHEEL", "damageMinEffect") => Some(5),
                            ("WHEEL", "damageScaleFromVisual") => Some(7),
                            ("WHEEL", "damageScaleFromVisualDisplacement") => Some(8),
                            ("WHEEL", "terminalDamagePercentage") => Some(9),
                            _ => None,
                        }
                    };
                    if let Some(slot) = slot {
                        let value = std::str::from_utf8(&attr.value).map_err(|_| Error {
                            offset: 0,
                            message: "invalid HDM number encoding",
                        })?;
                        let parsed: f64 = value.trim().parse().map_err(|_| Error {
                            offset: 0,
                            message: "invalid HDM steering number",
                        })?;
                        let value = parsed as f32;
                        if !value.is_finite() {
                            return fail(0, "nonfinite HDM steering number");
                        }
                        fields[slot] = value;
                    }
                }
                if !empty {
                    depth += 1;
                }
            }
            Event::End(_) => {
                depth = depth.checked_sub(1).ok_or(Error {
                    offset: 0,
                    message: "unmatched HDM end",
                })?;
            }
            Event::Text(e) if e.as_ref().iter().all(u8::is_ascii_whitespace) => {}
            Event::Decl(e)
                if depth == 0 && roots == 0 && e.version().is_ok_and(|v| v.as_ref() == b"1.0") => {}
            Event::Eof => break,
            _ => return fail(0, "unsupported HDM XML content"),
        }
    }
    if depth != 0 || roots != 1 || !suspension || !wheel {
        return fail(0, "incomplete HDM steering configuration");
    }
    Ok(SteeringParameters {
        visual_damage: VisualDamageParameters {
            angular_limit_280: fields[7],
            displacement_limit_284: fields[8],
            terminal_percentage_288: fields[9],
        },
        maximum_rotation_26c: fields[0],
        maximum_cap_250: fields[1],
        perturbation_gain_270: fields[2],
        ramp_start_274: fields[3],
        ramp_end_278: fields[4],
        low_gain_27c: fields[5],
        cap_threshold_254: fields[6],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &str = "<?xml version=\"1.0\"?><VEHICLEHANDLINGDAMAGE><!legacy note, level<1><SUSPENSION maxWheelLockScale=\".8\"/><WHEEL maxMisalignAngle=\"-0\" damageToStartBaseEffect=\".100000004\" damageToStartSteeringWobble=\"999\"/></VEHICLEHANDLINGDAMAGE>";
    #[test]
    fn descriptions_defaults_and_distinct_attribute_mapping() {
        let values = read(FIXTURE.as_bytes()).unwrap();
        assert_eq!(values.maximum_rotation_26c.to_bits(), (-0.0f32).to_bits());
        assert_eq!(values.ramp_start_274, 0.100000004f64 as f32);
        assert_eq!(values.ramp_end_278.to_bits(), 0x3f733333);
        assert_eq!(values.low_gain_27c.to_bits(), 0x3dcccccd);
        assert_eq!(values.perturbation_gain_270, 0.);
        assert_eq!(values.maximum_cap_250, 0.8);
        assert_eq!(
            values.visual_damage.scalars().map(f32::to_bits),
            [27.0f32.to_bits(), 0x3ee66666, 150.0f32.to_bits()]
        );
    }
    #[test]
    fn malformed_truncated_and_ambiguous_configuration_is_rejected() {
        for prefix in 0..FIXTURE.len() {
            assert!(
                read(&FIXTURE.as_bytes()[..prefix]).is_err(),
                "prefix={prefix}"
            );
        }
        for replacement in [
            "<WHEEL/><WHEEL/>",
            "<WHEEL maxMisalignAngle=\"NaN\"/>",
            "<WHEEL maxMisalignAngle=\"1e300\"/>",
            "<WHEEL x=\"<!note>\"/>",
            "<WHEEL x=\"&foo;\"/>",
            "<WHEEL x=\"1\" x=\"2\"/>",
            "<!DOCTYPE x><WHEEL/>",
            "<![CDATA[x]]><WHEEL/>",
            "<WHEEL>text</WHEEL>",
        ] {
            let text = format!(
                "<VEHICLEHANDLINGDAMAGE><SUSPENSION/>{replacement}</VEHICLEHANDLINGDAMAGE>"
            );
            assert!(read(text.as_bytes()).is_err(), "{replacement}");
        }
    }
}
