//! Saved car-atlas decal placements. Runtime selection/composition is separate.
use crate::{Error, Result, fail};
use quick_xml::{Reader, events::Event};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    pub centre: [f32; 2],
    pub size: [f32; 2],
    pub rotation_degrees: f32,
    pub flip: bool,
}

/// Decimal decal text, preserving leading zeroes. This prototype accepts up to3 digits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Number {
    digits: [u8; 3],
    len: u8,
}
impl Number {
    pub fn parse(text: &str) -> Result<Self> {
        if text.is_empty() || text.len() > 3 || !text.bytes().all(|v| v.is_ascii_digit()) {
            return fail(0, "race number needs 1..3 decimal digits");
        }
        let mut digits = [0; 3];
        digits[..text.len()].copy_from_slice(text.as_bytes());
        Ok(Self {
            digits,
            len: text.len() as u8,
        })
    }
    pub fn digits(&self) -> &[u8] {
        &self.digits[..self.len as usize]
    }
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(self.digits()).expect("validated decimal digits")
    }
}
impl std::fmt::Display for Number {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Plain XML only. Sponsor slots are intentionally ignored; number slots retained.
pub fn read_numbers(text: &str) -> Result<Vec<Placement>> {
    if text.len() > 65536 {
        return fail(0, "car decal XML exceeds bound");
    }
    let mut reader = Reader::from_str(text);
    let mut stack: Vec<Vec<u8>> = Vec::new();
    let mut seen_root = false;
    let mut number = false;
    let mut result = Vec::new();
    let mut instances = 0;
    loop {
        let at = reader.buffer_position() as usize;
        let event = reader.read_event().map_err(|_| Error {
            offset: at,
            message: "malformed car decal XML",
        })?;
        let (e, empty) = match &event {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::End(e) => {
                if stack.last().map(Vec::as_slice) != Some(e.name().as_ref()) {
                    return fail(at, "unbalanced car decal XML");
                }
                if e.name().as_ref() == b"Decal" {
                    number = false;
                }
                stack.pop();
                continue;
            }
            Event::Decl(_) | Event::Comment(_) => continue,
            Event::Text(t) if t.as_ref().iter().all(u8::is_ascii_whitespace) => continue,
            Event::Eof => break,
            _ => return fail(at, "unsupported car decal XML content"),
        };
        let name = e.name();
        let mut attrs = std::collections::BTreeMap::new();
        for a in e.attributes() {
            let a = a.map_err(|_| Error {
                offset: at,
                message: "invalid car decal attribute",
            })?;
            let value = std::str::from_utf8(&a.value)
                .ok()
                .filter(|v| !v.contains('&'))
                .ok_or(Error {
                    offset: at,
                    message: "invalid car decal value",
                })?;
            if attrs
                .insert(a.key.as_ref().to_vec(), value.to_owned())
                .is_some()
            {
                return fail(at, "duplicate car decal attribute");
            }
        }
        match name.as_ref() {
            b"CarDecalSet" if stack.is_empty() && !seen_root && !empty => {
                seen_root = true;
            }
            b"Decal" if stack.len() == 1 => {
                number = attrs.get(b"type".as_slice()).ok_or(Error {
                    offset: at,
                    message: "decal type absent",
                })? == "number";
            }
            b"Instance" if stack.len() == 2 && stack[1] == b"Decal" => {
                instances += 1;
                if instances > 70 {
                    return fail(at, "too many car decal instances");
                }
                if number {
                    let get = |key: &[u8]| {
                        attrs.get(key).ok_or(Error {
                            offset: at,
                            message: "number decal field absent",
                        })
                    };
                    let scalar = |key: &[u8]| -> Result<f32> {
                        let v = get(key)?.parse::<f32>().map_err(|_| Error {
                            offset: at,
                            message: "invalid number decal scalar",
                        })?;
                        if !v.is_finite() {
                            return fail(at, "nonfinite number decal scalar");
                        }
                        Ok(v)
                    };
                    let centre = [scalar(b"posU")?, scalar(b"posV")?];
                    let size = [scalar(b"width")?, scalar(b"height")?];
                    let rotation_degrees = scalar(b"rotation")?;
                    let flip = match get(b"flip")?.as_str() {
                        "true" => true,
                        "false" => false,
                        _ => return fail(at, "invalid decal flip"),
                    };
                    if centre.iter().any(|v| !(0.0..=1.0).contains(v))
                        || size.iter().any(|v| *v <= 0. || *v > 1.)
                        || rotation_degrees.abs() > 3600.
                    {
                        return fail(at, "number decal outside supported bounds");
                    }
                    result.push(Placement {
                        centre,
                        size,
                        rotation_degrees,
                        flip,
                    });
                }
            }
            _ => return fail(at, "unexpected car decal structure"),
        }
        if !empty {
            stack.push(name.as_ref().to_vec());
        }
        if empty && name.as_ref() == b"Decal" {
            number = false;
        }
    }
    if !seen_root || !stack.is_empty() || result.is_empty() {
        return fail(0, "number decal set absent or incomplete");
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    const INSTANCE: &str =
        r#"<Instance posU="0.5" posV="0.4" width="0.2" height="0.3" rotation="90" flip="false"/>"#;
    fn xml(instance: &str) -> String {
        format!(
            "<CarDecalSet><Decal type=\"slot0\"><Instance/></Decal><Decal type=\"number\">{instance}</Decal></CarDecalSet>"
        )
    }
    #[test]
    fn number_text_preserves_leading_zeroes_and_rejects_non_decimal_input() {
        assert_eq!(Number::parse("01").unwrap().as_str(), "01");
        assert_eq!(Number::parse("00").unwrap().digits(), b"00");
        for text in ["", "-1", "+1", "1000", " 1", "1.0"] {
            assert!(Number::parse(text).is_err());
        }
    }
    #[test]
    fn reads_only_number_placements_and_preserves_source_uv() {
        let p = read_numbers(&xml(INSTANCE)).unwrap();
        assert_eq!(p.len(), 1);
        assert_eq!(
            p[0],
            Placement {
                centre: [0.5, 0.4],
                size: [0.2, 0.3],
                rotation_degrees: 90.,
                flip: false
            }
        );
    }
    #[test]
    fn rejects_nonfinite_missing_duplicate_and_out_of_bounds_fields() {
        for bad in [
            INSTANCE.replace("0.5", "NaN"),
            INSTANCE.replace("0.3", "0"),
            INSTANCE.replace("flip=\"false\"", ""),
            INSTANCE.replace("posU=\"0.5\"", "posU=\"0.5\" posU=\"0.5\""),
            INSTANCE.replace("rotation=\"90\"", "rotation=\"inf\""),
        ] {
            assert!(read_numbers(&xml(&bad)).is_err());
        }
    }
    #[test]
    fn rejects_entities_truncation_nested_roots_and_unbounded_instances() {
        for bad in [
            xml(INSTANCE).replace("0.5", "&x;"),
            xml(INSTANCE) + "<CarDecalSet/>",
            xml(INSTANCE).replace("</CarDecalSet>", ""),
            xml(&INSTANCE.repeat(71)),
            "<!DOCTYPE x><CarDecalSet/>".to_owned(),
        ] {
            assert!(read_numbers(&bad).is_err());
        }
    }
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn purchased_lacetti_has_two_door_slots_and_quarter_turn_roof_slot() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let text = std::fs::read_to_string(std::path::Path::new(&root).join("cars/lac/decals.xml"))
            .unwrap();
        let p = read_numbers(&text).unwrap();
        assert_eq!(p.len(), 3);
        assert_eq!(p.iter().filter(|p| p.rotation_degrees == 0.).count(), 2);
        assert_eq!(p.iter().filter(|p| p.rotation_degrees == 90.).count(), 1);
        assert!(p.iter().all(|p| !p.flip));
    }
}
