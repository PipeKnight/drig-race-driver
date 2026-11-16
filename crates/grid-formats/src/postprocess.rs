//! Ordered saved postprocess descriptions. No effect activation, resource overlay or GPU domain.
//! Semantic reader over the existing binary-XML Document, not a second byte parser.
use crate::{
    Error, Result,
    binxml::{Document, Element},
    fail,
};
#[derive(Clone, Debug)]
pub struct Parameter<'a> {
    pub element: usize,
    pub name: &'a str,
    pub payload: Option<&'a str>,
}
#[derive(Clone, Debug)]
pub struct Group<'a> {
    pub element: usize,
    pub name: &'a str,
    pub parameters: Vec<Parameter<'a>>,
}
#[derive(Clone, Debug)]
pub struct Effect<'a> {
    pub element: usize,
    pub ordinal: usize,
    pub id: i32,
    pub name: &'a str,
    pub chain: Option<&'a str>,
    pub template: Option<&'a str>,
    pub groups: Vec<Group<'a>>,
}
#[derive(Clone, Debug)]
pub struct Effects<'a> {
    pub effects: Vec<Effect<'a>>,
}
fn error(index: usize, message: &'static str) -> Error {
    Error {
        offset: index,
        message,
    }
}
pub(crate) fn attr<'a>(index: usize, e: &Element<'a>, name: &str) -> Result<Option<&'a str>> {
    let mut values = e.attributes.iter().filter(|(n, _)| *n == name);
    let value = values.next().map(|(_, v)| *v);
    if values.next().is_some() {
        return fail(index, "duplicate postprocess attribute");
    };
    Ok(value)
}
fn required<'a>(index: usize, e: &Element<'a>, name: &str) -> Result<&'a str> {
    attr(index, e, name)?.ok_or(error(index, "missing postprocess attribute"))
}
fn children<'a, 'd>(
    d: &'d Document<'a>,
    e: &Element<'a>,
    index: usize,
) -> Result<&'d [Element<'a>]> {
    d.elements
        .get(e.children.clone())
        .ok_or(error(index, "postprocess children outside Document"))
}
/// Stronger authored admission: bounded actual Document indices, direct schema only.
/// Duplicate effect/group/Param names remain ordered raw data, not invented override priority.
pub fn read<'a>(d: &Document<'a>) -> Result<Effects<'a>> {
    if d.elements.len() > 16384 {
        return fail(0, "postprocess element bound");
    }
    let root = d
        .elements
        .first()
        .ok_or(error(0, "empty postprocess Document"))?;
    if root.name != "PostProcessEffects" {
        return fail(0, "expected PostProcessEffects");
    }
    let mut effects = Vec::new();
    let mut budget = 16384usize;
    for (ordinal, e) in children(d, root, 0)?.iter().enumerate() {
        if budget == 0 {
            return fail(0, "postprocess traversal bound");
        };
        budget -= 1;
        let index = root.children.start + ordinal;
        if e.name != "Effect" {
            return fail(index, "expected direct Effect child");
        }
        let id = required(index, e, "id")?
            .parse::<i32>()
            .map_err(|_| error(index, "invalid finite-domain effect id"))?;
        let mut groups = Vec::new();
        for (j, _) in children(d, e, index)?.iter().enumerate() {
            groups.push(group_with_budget(d, e.children.start + j, &mut budget)?);
        }
        effects.push(Effect {
            element: index,
            ordinal,
            id,
            name: required(index, e, "name")?,
            chain: attr(index, e, "chain")?,
            template: attr(index, e, "template")?,
            groups,
        });
    }
    Ok(Effects { effects })
}
/// Read one actual group without requiring effect IDs; shared legacy grade policy stays separate.
pub fn read_group<'a>(d: &Document<'a>, index: usize) -> Result<Group<'a>> {
    let mut budget = 16384;
    group_with_budget(d, index, &mut budget)
}
fn group_with_budget<'a>(d: &Document<'a>, index: usize, budget: &mut usize) -> Result<Group<'a>> {
    if *budget == 0 {
        return fail(index, "postprocess traversal bound");
    };
    *budget -= 1;
    let g = d
        .elements
        .get(index)
        .ok_or(error(index, "group outside Document"))?;
    if g.name != "ParameterGroup" {
        return fail(index, "expected direct ParameterGroup");
    }
    let mut parameters = Vec::new();
    for (k, param) in children(d, g, index)?.iter().enumerate() {
        if *budget == 0 {
            return fail(index, "postprocess traversal bound");
        };
        *budget -= 1;
        let pi = g.children.start + k;
        if param.name != "Param" || !children(d, param, pi)?.is_empty() {
            return fail(pi, "expected leaf Param");
        }
        parameters.push(Parameter {
            element: pi,
            name: required(pi, param, "name")?,
            payload: param.value,
        });
    }
    Ok(Group {
        element: index,
        name: required(index, g, "name")?,
        parameters,
    })
}
/// Text or binary source to one shared owned Document. No activation/default resolution.
pub fn read_source(bytes: &[u8]) -> Result<crate::xml_document::OwnedDocument> {
    let owned = crate::xml_document::read(bytes)?;
    read(&owned.document())?;
    Ok(owned)
}
impl<'a> Effects<'a> {
    /// Conservative unique selection is caller admission, NOT native same-id overlay semantics.
    pub fn unique_id(&self, id: i32) -> Result<&Effect<'a>> {
        let mut found = self.effects.iter().filter(|e| e.id == id);
        let item = found.next().ok_or(error(0, "effect id absent"))?;
        if found.next().is_some() {
            return fail(item.element, "ambiguous effect id");
        };
        Ok(item)
    }
}
impl<'a> Effect<'a> {
    pub fn unique_group(&self, name: &str) -> Result<&Group<'a>> {
        let mut found = self.groups.iter().filter(|g| g.name == name);
        let item = found
            .next()
            .ok_or(error(self.element, "parameter group absent"))?;
        if found.next().is_some() {
            return fail(item.element, "ambiguous parameter group");
        };
        Ok(item)
    }
}
impl Group<'_> {
    fn payload(&self, name: &str) -> Result<&str> {
        let mut found = self.parameters.iter().filter(|p| p.name == name);
        let p = found
            .next()
            .ok_or(error(self.element, "parameter absent"))?;
        if found.next().is_some() {
            return fail(p.element, "ambiguous parameter write");
        };
        p.payload
            .ok_or(error(p.element, "parameter payload absent"))
    }
    /// Ordinary finite input: _atof -> stored binary32. No invented clamping/unit/default.
    pub fn float(&self, name: &str) -> Result<f32> {
        float(self.payload(name)?)
    }
    /// Ordinary four signed32 tokens, low-byte truncation, NOT saturation/sRGB conversion.
    pub fn rgba8(&self, name: &str) -> Result<[u8; 4]> {
        rgba8(self.payload(name)?)
    }
}
pub fn float(text: &str) -> Result<f32> {
    let double = text
        .trim()
        .parse::<f64>()
        .map_err(|_| error(0, "invalid ordinary float payload"))?;
    let value = double as f32;
    if !double.is_finite() || !value.is_finite() {
        return fail(0, "nonfinite stored float payload");
    };
    Ok(value)
}
pub fn rgba8(text: &str) -> Result<[u8; 4]> {
    let tokens: Vec<_> = text
        .trim()
        .split([' ', ','])
        .filter(|t| !t.is_empty())
        .collect();
    if tokens.len() != 4 {
        return fail(0, "expected four comma/space colour tokens");
    }
    let mut bytes = [0; 4];
    for (i, token) in tokens.into_iter().enumerate() {
        bytes[i] = token
            .parse::<i32>()
            .map_err(|_| error(0, "invalid ordinary signed32 colour token"))?
            as u8;
    }
    Ok(bytes)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn doc(value: Option<&'static str>) -> Document<'static> {
        Document {
            strings: vec![],
            string_padding: &[],
            elements: vec![
                Element {
                    name: "PostProcessEffects",
                    value: None,
                    attributes: vec![],
                    parent: None,
                    children: 1..2,
                },
                Element {
                    name: "Effect",
                    value: None,
                    attributes: vec![
                        ("id", "2"),
                        ("name", "named"),
                        ("chain", "Type2"),
                        ("template", "other"),
                    ],
                    parent: Some(0),
                    children: 2..3,
                },
                Element {
                    name: "ParameterGroup",
                    value: None,
                    attributes: vec![("name", "ColourBalance")],
                    parent: Some(1),
                    children: 3..4,
                },
                Element {
                    name: "Param",
                    value,
                    attributes: vec![("name", "tintAmount")],
                    parent: Some(2),
                    children: 4..4,
                },
            ],
        }
    }
    #[test]
    fn actual_document_payload_and_provenance_are_retained() {
        let d = doc(Some(" 0.25 "));
        let m = read(&d).unwrap();
        let e = m.unique_id(2).unwrap();
        assert_eq!(
            (e.ordinal, e.element, e.name, e.chain, e.template),
            (0, 1, "named", Some("Type2"), Some("other"))
        );
        let g = e.unique_group("ColourBalance").unwrap();
        assert_eq!(g.parameters[0].payload, Some(" 0.25 "));
        assert_eq!(g.float("tintAmount").unwrap(), 0.25);
        assert!(g.float("TintAmount").is_err());
    }
    #[test]
    fn packed_colour_truncates_lowbytes_without_saturating() {
        assert_eq!(rgba8("-1, 256 511, 128").unwrap(), [255, 0, 255, 128]);
        assert!(rgba8("1,2,3").is_err());
        assert!(rgba8("2147483648,0,0,0").is_err());
        assert!(float("NaN").is_err());
        assert!(float("1e99").is_err());
    }
    #[test]
    fn public_document_bounds_and_payload_fail_closed() {
        let mut d = doc(None);
        assert!(
            read(&d)
                .unwrap()
                .unique_id(2)
                .unwrap()
                .unique_group("ColourBalance")
                .unwrap()
                .float("tintAmount")
                .is_err()
        );
        d.elements[0].children = 1..99;
        assert!(read(&d).is_err());
        let mut d = doc(Some("1"));
        d.elements[2].attributes.push(("name", "other"));
        assert!(read(&d).is_err());
    }
    #[test]
    fn duplicate_writes_preserved_but_unique_projection_refuses() {
        let mut d = doc(Some("1"));
        let p = Element {
            name: "Param",
            value: Some("2"),
            attributes: vec![("name", "tintAmount")],
            parent: Some(2),
            children: 5..5,
        };
        d.elements.push(p);
        d.elements[2].children = 3..5;
        let m = read(&d).unwrap();
        let g = m
            .unique_id(2)
            .unwrap()
            .unique_group("ColourBalance")
            .unwrap();
        assert_eq!(g.parameters.len(), 2);
        assert!(g.float("tintAmount").is_err());
    }
}
