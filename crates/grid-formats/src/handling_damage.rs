//! Bounded complete cold handling configuration from legacy HDM text.
use crate::handling_damage_fields::{DEFAULTS, FIELDS, STRENGTHS};
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

/// Complete cold 0x168-byte settings block, relative to mutable owner218.
#[derive(Clone, Debug)]
pub struct Configuration {
    words: [u32; 90],
}
impl Configuration {
    pub fn words(&self) -> &[u32; 90] {
        &self.words
    }
    pub fn steering(&self) -> SteeringParameters {
        let f = |o: usize| f32::from_bits(self.words[o / 4]);
        SteeringParameters {
            visual_damage: VisualDamageParameters {
                angular_limit_280: f(0x68),
                displacement_limit_284: f(0x6c),
                terminal_percentage_288: f(0x70),
            },
            maximum_rotation_26c: f(0x54),
            maximum_cap_250: f(0x38),
            perturbation_gain_270: f(0x58),
            ramp_start_274: f(0x5c),
            ramp_end_278: f(0x60),
            low_gain_27c: f(0x64),
            cap_threshold_254: f(0x3c),
        }
    }
}
pub fn read(bytes: &[u8]) -> Result<SteeringParameters> {
    Ok(parse(bytes, true)?.steering())
}
pub fn read_configuration(bytes: &[u8]) -> Result<Configuration> {
    parse(bytes, false)
}
fn parse(bytes: &[u8], require_steering: bool) -> Result<Configuration> {
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
    let mut stack: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut strengths_seen = std::collections::HashSet::new();
    let mut fields = [0u32; 90];
    for &(offset, value) in DEFAULTS {
        fields[offset / 4] = value;
    }
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
                let selected = depth == 1 && FIELDS.iter().any(|&(node, _, _)| node == name);
                let strength = STRENGTHS.iter().find(|&&(parent, node, _)| {
                    depth == 2 && node == name && stack.last().is_some_and(|s| s == parent)
                });
                if selected && !seen.insert(name.to_string()) {
                    return fail(0, "duplicate HDM configuration node");
                }
                if let Some(&(_, _, offset)) = strength
                    && !strengths_seen.insert(offset)
                {
                    return fail(0, "duplicate HDM strength node");
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
                    let slot = if selected {
                        FIELDS
                            .iter()
                            .find(|&&(node, attribute, _)| node == name && attribute == key)
                            .map(|&(_, _, offset)| offset / 4)
                    } else if let Some(&(_, _, offset)) = strength {
                        ["damageScale", "chanceOfMinorDamage", "chanceOfMajorDamage"]
                            .iter()
                            .position(|&k| k == key)
                            .map(|i| offset / 4 + i)
                    } else {
                        None
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
                        fields[slot] = value.to_bits();
                    }
                }
                if !empty {
                    stack.push(name.to_string());
                    depth += 1;
                }
            }
            Event::End(_) => {
                stack.pop();
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
    if depth != 0
        || roots != 1
        || (require_steering && (!seen.contains("SUSPENSION") || !seen.contains("WHEEL")))
    {
        return fail(0, "incomplete HDM configuration");
    }
    for offset in strengths_seen {
        let i = offset / 4;
        let minor = f32::from_bits(fields[i + 1]);
        let major = f32::from_bits(fields[i + 2]);
        let total = minor + major;
        if total > 1. {
            let scale = 1. / total;
            fields[i + 2] = (major * scale).to_bits();
            fields[i + 1] = (minor * scale).to_bits();
        }
    }
    Ok(Configuration { words: fields })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strength_probabilities_are_normalized_only_in_the_matching_parent() {
        let source = b"<VEHICLEHANDLINGDAMAGE><WHEEL><WHEELSTRENGTH damageScale='7' chanceOfMinorDamage='2' chanceOfMajorDamage='3'/></WHEEL><SUSPENSION><WHEELSTRENGTH damageScale='999'/></SUSPENSION></VEHICLEHANDLINGDAMAGE>";
        let settings = read_configuration(source).unwrap();
        assert_eq!(f32::from_bits(settings.words()[0x40 / 4]), 7.);
        assert_eq!(f32::from_bits(settings.words()[0x44 / 4]), 2. * (1. / 5.));
        assert_eq!(f32::from_bits(settings.words()[0x48 / 4]), 3. * (1. / 5.));
        assert!(read_configuration(b"<VEHICLEHANDLINGDAMAGE><WHEEL><WHEELSTRENGTH/><WHEELSTRENGTH/></WHEEL></VEHICLEHANDLINGDAMAGE>").is_err());
    }
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
