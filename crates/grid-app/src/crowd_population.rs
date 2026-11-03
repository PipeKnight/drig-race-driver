//! Shared crowd resource and definition metadata preparation.
//! GENERAL typed metadata only; source XML graph + explicit caller context.
//! No geometry, file I/O, animation, clock, RNG, LOD selection or implicit region policy.
use grid_formats::{
    binxml::{Document, Element},
    crowd_placements::{Crowd, Group, Placements},
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default)]
#[allow(
    dead_code,
    reason = "Retained source metadata; bounded preview consumes only selected fields, without discarding unresolved/unknown data."
)]
pub enum DefinitionSelection<'a> {
    #[default]
    None,
    /// All three names are caller-supplied source context, not derived from a route.
    Exact {
        primary_definition_set: Option<&'a str>,
        fallback_definition_set: Option<&'a str>,
        map_group: &'a str,
        billboard_userdata: &'a str,
    },
}
/// ONLY parsed PSSG dataset entries. Binary/xml entries may legitimately reuse userdata.
/// This type is a semantic input seam, NOT a replacement text-XML parser.
#[derive(Clone, Copy, Debug)]
pub struct DatasetPssg<'a> {
    pub userdata: &'a str,
    pub filename: &'a str,
}
/// Borrow the original graph: unhandled records and attributes remain inspectable.
/// Only direct OrganismPlugin PSSG records enter the typed resource map.
pub struct Dataset<'a> {
    pub source: &'a grid_tools::tyre_smoke::Node,
    pub pssg: Vec<DatasetPssg<'a>>,
}
fn dataset_attribute<'a>(
    node: &'a grid_tools::tyre_smoke::Node,
    key: &'static str,
) -> Result<&'a str, Error> {
    let mut values = node.attributes.iter().filter(|(name, _)| name == key);
    let value = values.next().ok_or_else(|| Error::MissingAttribute {
        element: node.name.clone(),
        key,
    })?;
    if values.next().is_some() {
        return Err(Error::Duplicate {
            kind: "dataset attribute",
            key: key.to_owned(),
        });
    }
    Ok(value.1.as_str())
}
fn literal_dataset_value<'a>(value: &'a str, key: &'static str) -> Result<&'a str, Error> {
    // Only interpreted keys are constrained. Existing parse_plain does not
    // decode entities; accepting them could bind a different alias or processor.
    if value.is_empty() || value.contains('&') || value.chars().any(char::is_control) {
        return Err(Error::InvalidDatasetAttribute(key.to_owned()));
    }
    Ok(value)
}
/// Input comes from existing tyre_smoke::read_xml, not substring matching or
/// another parser. No recursive resource search, processor fallback or region inference.
pub fn dataset(root: &grid_tools::tyre_smoke::Node) -> Result<Dataset<'_>, Error> {
    if root.name != "dataset" {
        return Err(Error::InvalidDatasetRoot(root.name.clone()));
    }
    let mut pssg = Vec::new();
    for entry in &root.children {
        if entry.name != "pssg" {
            continue;
        }
        let processor = literal_dataset_value(dataset_attribute(entry, "processor")?, "processor")?;
        if processor != "OrganismPlugin" {
            continue;
        }
        let userdata = literal_dataset_value(dataset_attribute(entry, "userdata")?, "userdata")?;
        let filename = literal_dataset_value(dataset_attribute(entry, "filename")?, "filename")?;
        pssg.push(DatasetPssg { userdata, filename });
    }
    // Whole selected dataset admission, including currently unused aliases.
    let _ = resources(&pssg)?;
    Ok(Dataset { source: root, pssg })
}
#[derive(Clone, Debug)]
#[allow(
    dead_code,
    reason = "Retained source metadata; bounded preview consumes only selected fields, without discarding unresolved/unknown data."
)]
pub struct SelectedContext {
    pub primary_definition_set: Option<String>,
    pub fallback_definition_set: Option<String>,
    pub map_group: String,
    pub billboard_userdata: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResourceRef {
    pub userdata: String,
    pub relative_path: String,
}
pub type Fields = Vec<(String, String)>;
#[derive(Clone, Debug, PartialEq, Eq)]
#[allow(
    dead_code,
    reason = "Structured metadata failure payloads retain unresolved source keys for diagnostics."
)]
pub enum Error {
    UnresolvedDefinitions,
    UnsupportedSourceName(String),
    InvalidDatasetRoot(String),
    InvalidDatasetAttribute(String),
    Missing { kind: &'static str, key: String },
    Duplicate { kind: &'static str, key: String },
    MissingAttribute { element: String, key: &'static str },
    InvalidResourcePath(String),
    InvalidInstanceSetName,
    NoMaps(String),
    NoVariants(String),
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "crowd metadata: {self:?}")
    }
}
impl std::error::Error for Error {}
#[derive(Clone, Debug)]
#[allow(
    dead_code,
    reason = "Retained source metadata; bounded preview consumes only selected fields, without discarding unresolved/unknown data."
)]
pub struct ColourSet {
    pub fields: Fields,
    pub colours: Vec<Fields>,
}
#[derive(Clone, Debug)]
#[allow(
    dead_code,
    reason = "Retained source metadata; bounded preview consumes only selected fields, without discarding unresolved/unknown data."
)]
pub struct Variant {
    pub fields: Fields,
    /// Selected stance's attributes only; absence is explicit, not a guessed card.
    pub billboard_stance: Option<Fields>,
    pub skin_fields: Option<Fields>,
    pub colour_sets: Vec<ColourSet>,
}
#[derive(Clone, Debug)]
#[allow(
    dead_code,
    reason = "Retained source metadata; bounded preview consumes only selected fields, without discarding unresolved/unknown data."
)]
pub struct Template {
    /// Document indices, NOT native runtime global-table indices.
    pub definition_set_element: usize,
    pub definition_element: usize,
    pub definition_origin: DefinitionOrigin,
    pub requested_definition_name: String,
    pub map_name: String,
    pub map_fields: Fields,
    pub instance_set_fields: Fields,
    pub map_reference_fields: Fields,
    pub definition_set: String,
    pub definition_name: String,
    pub definition_fields: Fields,
    pub stance_name: String,
    pub stance_fields: Fields,
    pub mesh: Option<ResourceRef>,
    pub billboard: ResourceRef,
    /// Preserve all alternatives in source order; this resolver does not select variety.
    pub variants: Vec<Variant>,
}
#[derive(Clone, Debug)]
#[allow(
    dead_code,
    reason = "Retained source metadata; bounded preview consumes only selected fields, without discarding unresolved/unknown data."
)]
pub enum Resolution {
    /// Source instanceSet can refer to multiple maps. No first/random choice here.
    StaticTemplates(Vec<Template>),
    UnsupportedMoving {
        records: u32,
        payload_bytes: usize,
    },
    Unresolved(Error),
}
#[derive(Debug)]
#[allow(
    dead_code,
    reason = "Retained source metadata; bounded preview consumes only selected fields, without discarding unresolved/unknown data."
)]
pub struct PreparedGroup<'p, 'asset> {
    /// Borrow the original group, retaining names, every float bit and opaque payload.
    pub source: &'p Group<'asset>,
    pub resolution: Resolution,
}
#[derive(Debug)]
pub struct Prepared<'p, 'asset, 'd, 'def> {
    /// Entire source definitions retain unknown fields/children without copying.
    pub definitions: &'d Document<'def>,
    pub context: SelectedContext,
    pub dataset_resources: BTreeMap<String, ResourceRef>,
    /// Includes original cells/bounds/origin/grid; not converted to engine transforms.
    pub source: &'p Crowd<'asset>,
    pub groups: Vec<PreparedGroup<'p, 'asset>>,
    pub resolved_static_records: u32,
    pub unresolved_static_records: u32,
    pub unsupported_moving_records: u32,
}
fn fields(e: &Element<'_>) -> Fields {
    e.attributes
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}
fn attr<'a>(e: &Element<'a>, key: &'static str) -> Result<&'a str, Error> {
    let mut found = e.attributes.iter().filter(|(k, _)| *k == key);
    let value = found
        .next()
        .map(|(_, v)| *v)
        .ok_or_else(|| Error::MissingAttribute {
            element: e.name.to_owned(),
            key,
        })?;
    if found.next().is_some() {
        return Err(Error::Duplicate {
            kind: "attribute",
            key: key.to_owned(),
        });
    }
    Ok(value)
}
fn children<'d, 'a>(
    d: &'d Document<'a>,
    parent: usize,
    tag: &'static str,
) -> impl Iterator<Item = (usize, &'d Element<'a>)> + 'd
where
    'a: 'd,
{
    d.elements[parent].children.clone().filter_map(move |i| {
        let e = &d.elements[i];
        (e.name == tag).then_some((i, e))
    })
}
fn optional<'d, 'a>(
    d: &'d Document<'a>,
    parent: usize,
    tag: &'static str,
    name: Option<&str>,
) -> Result<Option<(usize, &'d Element<'a>)>, Error>
where
    'a: 'd,
{
    let matches: Vec<_> = children(d, parent, tag)
        .filter(|(_, e)| {
            name.is_none_or(|n| e.attributes.iter().any(|(k, v)| *k == "name" && *v == n))
        })
        .collect();
    match matches.len() {
        0 => Ok(None),
        1 => Ok(Some(matches[0])),
        _ => Err(Error::Duplicate {
            kind: "element",
            key: format!("{tag}/{name:?}"),
        }),
    }
}
fn one<'d, 'a>(
    d: &'d Document<'a>,
    parent: usize,
    tag: &'static str,
    name: Option<&str>,
) -> Result<(usize, &'d Element<'a>), Error>
where
    'a: 'd,
{
    optional(d, parent, tag, name)?.ok_or_else(|| Error::Missing {
        kind: "element",
        key: format!("{tag}/{name:?}"),
    })
}
pub(crate) fn resource_path(filename: &str) -> Result<String, Error> {
    let relative = filename
        .strip_prefix("/data/")
        .ok_or_else(|| Error::InvalidResourcePath(filename.to_owned()))?;
    if relative.is_empty()
        || relative
            .chars()
            .any(|c| c == '\\' || c == ':' || c.is_control())
        || relative
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == ".." || p.contains('%'))
    {
        return Err(Error::InvalidResourcePath(filename.to_owned()));
    }
    Ok(relative.to_owned())
}
fn resources(entries: &[DatasetPssg<'_>]) -> Result<BTreeMap<String, ResourceRef>, Error> {
    let mut result = BTreeMap::new();
    for e in entries {
        let r = ResourceRef {
            userdata: e.userdata.to_owned(),
            relative_path: resource_path(e.filename)?,
        };
        if e.userdata.is_empty() || result.insert(r.userdata.clone(), r).is_some() {
            return Err(Error::Duplicate {
                kind: "dataset PSSG userdata",
                key: e.userdata.to_owned(),
            });
        }
    }
    Ok(result)
}
fn resource(r: &BTreeMap<String, ResourceRef>, name: &str) -> Result<ResourceRef, Error> {
    r.get(name).cloned().ok_or_else(|| Error::Missing {
        kind: "dataset PSSG userdata",
        key: name.to_owned(),
    })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefinitionOrigin {
    Primary,
    Fallback,
}
/// Source-proven __stricmp boundary for crowdDefSet and crowdDef names only.
/// Supported ASCII domain explicitly excludes unknown CRT locale/Unicode rules.
fn source_named<'d, 'a>(
    d: &'d Document<'a>,
    parent: usize,
    tag: &'static str,
    requested: &str,
) -> Result<Option<(usize, &'d Element<'a>)>, Error>
where
    'a: 'd,
{
    if !requested.is_ascii() || requested.chars().any(char::is_control) {
        return Err(Error::UnsupportedSourceName(requested.to_owned()));
    }
    for (index, entry) in children(d, parent, tag) {
        let name = attr(entry, "name")?;
        if !name.is_ascii() || name.chars().any(char::is_control) {
            return Err(Error::UnsupportedSourceName(name.to_owned()));
        }
        if name.eq_ignore_ascii_case(requested) {
            return Ok(Some((index, entry)));
        }
    }
    Ok(None)
}
fn selected_set(d: &Document<'_>, name: Option<&str>) -> Result<Option<usize>, Error> {
    name.map(|name| {
        source_named(d, 0, "crowdDefSet", name).map(|found| found.map(|(index, _)| index))
    })
    .transpose()
    .map(Option::flatten)
}
struct Context<'d, 'a> {
    doc: &'d Document<'a>,
    primary: Option<usize>,
    fallback: Option<usize>,
    maps: usize,
    billboard: ResourceRef,
    resources: BTreeMap<String, ResourceRef>,
}
fn template(c: &Context<'_, '_>, map_name: &str) -> Result<Template, Error> {
    let d = c.doc;
    let (_, map) = one(d, c.maps, "instanceMap", Some(map_name))?;
    let def_name = attr(map, "crowdDef")?;
    let stance_name = attr(map, "stance")?;
    let mut chosen = None;
    for (set, origin) in [
        (c.primary, DefinitionOrigin::Primary),
        (c.fallback, DefinitionOrigin::Fallback),
    ] {
        if let Some(set) = set
            && let Some((index, definition)) = source_named(d, set, "crowdDef", def_name)?
        {
            chosen = Some((set, index, definition, origin));
            break;
        }
    }
    let (set_i, def_i, def, origin) = chosen.ok_or_else(|| Error::Missing {
        kind: "crowdDef in selected primary/fallback sets",
        key: def_name.to_owned(),
    })?;
    let (_, stance) = one(d, def_i, "stance", Some(stance_name))?;
    let mesh = optional(d, def_i, "skinData", None)?
        .map(|(_, skin)| resource(&c.resources, attr(skin, "meshTag")?))
        .transpose()?;
    let mut variants = Vec::new();
    for (vi, v) in children(d, def_i, "textureVariant") {
        let billboard_stance = if let Some((bi, _)) = optional(d, vi, "billboardData", None)? {
            optional(d, bi, "stance", Some(stance_name))?.map(|(_, s)| fields(s))
        } else {
            None
        };
        let colour_sets = children(d, vi, "colourSet")
            .map(|(ci, s)| ColourSet {
                fields: fields(s),
                colours: children(d, ci, "colour").map(|(_, e)| fields(e)).collect(),
            })
            .collect();
        variants.push(Variant {
            fields: fields(v),
            billboard_stance,
            skin_fields: optional(d, vi, "skinData", None)?.map(|(_, s)| fields(s)),
            colour_sets,
        });
    }
    if variants.is_empty() {
        return Err(Error::NoVariants(def_name.to_owned()));
    }
    Ok(Template {
        definition_set_element: set_i,
        definition_element: def_i,
        definition_origin: origin,
        requested_definition_name: def_name.to_owned(),
        map_name: map_name.to_owned(),
        map_fields: fields(map),
        instance_set_fields: Vec::new(),
        map_reference_fields: Vec::new(),
        definition_set: attr(&d.elements[set_i], "name")?.to_owned(),
        definition_name: attr(def, "name")?.to_owned(),
        definition_fields: fields(def),
        stance_name: stance_name.to_owned(),
        stance_fields: fields(stance),
        mesh,
        billboard: c.billboard.clone(),
        variants,
    })
}
fn group_templates(c: &Context<'_, '_>, group: &Group<'_>) -> Result<Vec<Template>, Error> {
    let name =
        std::str::from_utf8(group.instance_set).map_err(|_| Error::InvalidInstanceSetName)?;
    let (si, set) = one(c.doc, 0, "instanceSet", Some(name))?;
    let maps: Vec<_> = children(c.doc, si, "map").collect();
    if maps.is_empty() {
        return Err(Error::NoMaps(name.to_owned()));
    }
    maps.into_iter()
        .map(|(_, m)| {
            let mut t = template(c, attr(m, "name")?)?;
            t.instance_set_fields = fields(set);
            t.map_reference_fields = fields(m);
            Ok(t)
        })
        .collect()
}
/// Call with documents produced by existing binxml::read and crowd_placements::read.
/// Reader admission/range/count checks stay there. This is graphics resource semantics.
/// NONE is explicit unresolved state, before any resource guess or first-set fallback.
pub fn prepare<'p, 'asset, 'd, 'def>(
    source: &'p Crowd<'asset>,
    defs: &'d Document<'def>,
    dataset: &[DatasetPssg<'_>],
    selection: DefinitionSelection<'_>,
) -> Result<Prepared<'p, 'asset, 'd, 'def>, Error> {
    let DefinitionSelection::Exact {
        primary_definition_set,
        fallback_definition_set,
        map_group,
        billboard_userdata,
    } = selection
    else {
        return Err(Error::UnresolvedDefinitions);
    };
    let primary = selected_set(defs, primary_definition_set)?;
    let fallback = selected_set(defs, fallback_definition_set)?;
    let (maps, _) = one(defs, 0, "instanceMapGroup", Some(map_group))?;
    let refs = resources(dataset)?;
    let billboard = resource(&refs, billboard_userdata)?;
    let c = Context {
        doc: defs,
        primary,
        fallback,
        maps,
        billboard,
        resources: refs,
    };
    let mut result = Prepared {
        definitions: defs,
        context: SelectedContext {
            primary_definition_set: primary_definition_set.map(str::to_owned),
            fallback_definition_set: fallback_definition_set.map(str::to_owned),
            map_group: map_group.to_owned(),
            billboard_userdata: billboard_userdata.to_owned(),
        },
        dataset_resources: c.resources.clone(),
        source,
        groups: Vec::with_capacity(source.groups.len()),
        resolved_static_records: 0,
        unresolved_static_records: 0,
        unsupported_moving_records: 0,
    };
    for group in &source.groups {
        let resolution = match &group.placements {
            Placements::Moving(bytes) => {
                result.unsupported_moving_records += group.count;
                Resolution::UnsupportedMoving {
                    records: group.count,
                    payload_bytes: bytes.len(),
                }
            }
            Placements::Standing(_) => match group_templates(&c, group) {
                Ok(t) => {
                    result.resolved_static_records += group.count;
                    Resolution::StaticTemplates(t)
                }
                Err(e) => {
                    result.unresolved_static_records += group.count;
                    Resolution::Unresolved(e)
                }
            },
        };
        result.groups.push(PreparedGroup {
            source: group,
            resolution,
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use grid_formats::crowd_placements::Standing;
    fn node(
        name: &'static str,
        attrs: &[(&'static str, &'static str)],
        children: std::ops::Range<usize>,
    ) -> Element<'static> {
        Element {
            name,
            value: None,
            attributes: attrs.to_vec(),
            children,
            parent: None,
        }
    }
    // Authored metadata graph, not purchased payload or replacement XML parser.
    fn defs() -> Document<'static> {
        Document {
            strings: vec![],
            string_padding: &[],
            elements: vec![
                node("crowdDefs", &[], 1..5),
                node("crowdDefSet", &[("name", "blue")], 5..6),
                node("instanceMapGroup", &[("name", "pc")], 6..7),
                node("instanceSet", &[("name", "spectators-arbitrary")], 7..8),
                node("crowdDefSet", &[("name", "other")], 8..9),
                node("crowdDef", &[("name", "body"), ("unknown", "keep")], 9..12),
                node(
                    "instanceMap",
                    &[
                        ("name", "map-A"),
                        ("crowdDef", "body"),
                        ("stance", "sitting"),
                    ],
                    0..0,
                ),
                node("map", &[("name", "map-A")], 0..0),
                node("crowdDef", &[("name", "body")], 12..13),
                node(
                    "stance",
                    &[("name", "sitting"), ("randomWidthScale", "0.125")],
                    0..0,
                ),
                node("skinData", &[("meshTag", "bodyAlias")], 0..0),
                node("textureVariant", &[("name", "different-variant")], 13..15),
                node("stance", &[("name", "standing")], 0..0),
                node("billboardData", &[], 15..16),
                node("colourSet", &[("name", "different-colour")], 16..17),
                node(
                    "stance",
                    &[
                        ("name", "sitting"),
                        ("width", "0.7"),
                        ("opaque", "retained"),
                    ],
                    0..0,
                ),
                node(
                    "colour",
                    &[("red", "0.2"), ("green", "0.3"), ("blue", "0.4")],
                    0..0,
                ),
            ],
        }
    }
    fn source() -> Crowd<'static> {
        Crowd {
            origin_xz: [0.; 2],
            cell_size_xz: [1.; 2],
            dimensions: [1; 2],
            cells: vec![],
            total_instances: 3,
            groups: vec![
                Group {
                    instance_set: b"spectators-arbitrary",
                    count: 1,
                    placements: Placements::Standing(vec![Standing {
                        position: [1., 2., 3.],
                        orientation_pair: [-0., 1.],
                        opaque_scalar: f32::from_bits(0x3f000001),
                        cell: 7,
                    }]),
                },
                Group {
                    instance_set: b"not-decoded",
                    count: 2,
                    placements: Placements::Moving(&[9, 8, 7]),
                },
            ],
        }
    }
    fn dataset() -> [DatasetPssg<'static>; 2] {
        [
            DatasetPssg {
                userdata: "cards",
                filename: "/data/anims/unusual_cards.pssg",
            },
            DatasetPssg {
                userdata: "bodyAlias",
                filename: "/data/anims/unusual_body.pssg",
            },
        ]
    }
    fn selected() -> DefinitionSelection<'static> {
        DefinitionSelection::Exact {
            primary_definition_set: Some("blue"),
            fallback_definition_set: None,
            map_group: "pc",
            billboard_userdata: "cards",
        }
    }
    fn first<'a>(p: &'a Prepared<'_, '_, '_, '_>) -> &'a Template {
        match &p.groups[0].resolution {
            Resolution::StaticTemplates(t) => &t[0],
            _ => panic!("resolved expected"),
        }
    }
    #[test]
    fn none_never_guesses_definition_or_resource() {
        assert!(matches!(
            prepare(&source(), &defs(), &[], DefinitionSelection::None),
            Err(Error::UnresolvedDefinitions)
        ));
    }
    #[test]
    fn exact_names_own_stance_resources_and_unknowns() {
        let s = source();
        let d = defs();
        let p = prepare(&s, &d, &dataset(), selected()).unwrap();
        let t = first(&p);
        assert_eq!(t.stance_name, "sitting");
        assert_eq!(t.definition_set, "blue");
        assert_eq!(
            t.mesh.as_ref().unwrap().relative_path,
            "anims/unusual_body.pssg"
        );
        assert_eq!(t.billboard.relative_path, "anims/unusual_cards.pssg");
        assert!(
            t.definition_fields
                .contains(&("unknown".into(), "keep".into()))
        );
        assert!(
            t.variants[0]
                .billboard_stance
                .as_ref()
                .unwrap()
                .contains(&("opaque".into(), "retained".into()))
        );
        assert_eq!(t.variants[0].colour_sets[0].fields[0].1, "different-colour");
    }
    #[test]
    fn preserves_original_rows_bits_and_moving_accounting() {
        let s = source();
        let d = defs();
        let p = prepare(&s, &d, &dataset(), selected()).unwrap();
        assert!(std::ptr::eq(p.source, &s));
        assert!(std::ptr::eq(p.groups[0].source, &s.groups[0]));
        let Placements::Standing(rows) = &p.groups[0].source.placements else {
            panic!()
        };
        assert_eq!(rows[0].orientation_pair[0].to_bits(), (-0f32).to_bits());
        assert_eq!(rows[0].opaque_scalar.to_bits(), 0x3f000001);
        assert_eq!(rows[0].cell, 7);
        assert_eq!(
            (
                p.resolved_static_records,
                p.unresolved_static_records,
                p.unsupported_moving_records
            ),
            (1, 0, 2)
        );
        assert!(matches!(
            p.groups[1].resolution,
            Resolution::UnsupportedMoving {
                records: 2,
                payload_bytes: 3
            }
        ));
    }
    #[test]
    fn exact_set_has_no_cross_set_fallback() {
        let s = source();
        let d = defs();
        let p = prepare(
            &s,
            &d,
            &dataset(),
            DefinitionSelection::Exact {
                primary_definition_set: Some("other"),
                fallback_definition_set: None,
                map_group: "pc",
                billboard_userdata: "cards",
            },
        )
        .unwrap();
        assert!(matches!(
            &p.groups[0].resolution,
            Resolution::Unresolved(Error::Missing { .. })
        ));
    }
    #[test]
    fn unresolved_group_retains_count_and_identity() {
        let mut s = source();
        s.groups[0].instance_set = b"missing";
        let d = defs();
        let p = prepare(&s, &d, &dataset(), selected()).unwrap();
        assert_eq!(p.unresolved_static_records, 1);
        assert_eq!(p.groups[0].source.instance_set, b"missing");
        assert!(matches!(
            &p.groups[0].resolution,
            Resolution::Unresolved(Error::Missing { .. })
        ));
    }
    #[test]
    fn first_matching_set_is_selected_but_duplicate_dataset_alias_still_rejects() {
        let mut d = defs();
        d.elements[4].attributes[0].1 = "BLUE";
        let source = source();
        let prepared = prepare(&source, &d, &dataset(), selected()).unwrap();
        assert_eq!(first(&prepared).definition_set_element, 1);
        let mut entries = dataset();
        entries[1].userdata = "cards";
        assert!(matches!(
            prepare(&source, &d, &entries, selected()),
            Err(Error::Duplicate { .. })
        ));
    }
    #[test]
    fn unsafe_paths_and_missing_alias_reject() {
        for p in [
            "C:/escape.pssg",
            "/data/../escape.pssg",
            "/data/anims\\escape.pssg",
            "/data/%1%/a.pssg",
            "/data//a.pssg",
        ] {
            assert!(matches!(
                resource_path(p),
                Err(Error::InvalidResourcePath(_))
            ));
        }
        let s = source();
        let d = defs();
        let p = prepare(&s, &d, &dataset()[..1], selected()).unwrap();
        assert!(matches!(
            &p.groups[0].resolution,
            Resolution::Unresolved(Error::Missing {
                kind: "dataset PSSG userdata",
                ..
            })
        ));
    }
    #[test]
    fn map_alternatives_stay_in_source_order() {
        let mut d = defs();
        let mi = d.elements.len();
        d.elements.push(node(
            "instanceMap",
            &[
                ("name", "map-A"),
                ("crowdDef", "body"),
                ("stance", "sitting"),
            ],
            0..0,
        ));
        d.elements.push(node(
            "instanceMap",
            &[
                ("name", "map-B"),
                ("crowdDef", "body"),
                ("stance", "sitting"),
            ],
            0..0,
        ));
        d.elements[2].children = mi..mi + 2;
        let si = d.elements.len();
        d.elements.push(node("map", &[("name", "map-B")], 0..0));
        d.elements.push(node("map", &[("name", "map-A")], 0..0));
        d.elements[3].children = si..si + 2;
        let s = source();
        let p = prepare(&s, &d, &dataset(), selected()).unwrap();
        let Resolution::StaticTemplates(t) = &p.groups[0].resolution else {
            panic!()
        };
        assert_eq!(
            t.iter().map(|t| t.map_name.as_str()).collect::<Vec<_>>(),
            ["map-B", "map-A"]
        );
    }
    fn xml(text: &str) -> grid_tools::tyre_smoke::Node {
        grid_tools::tyre_smoke::parse_plain(text).unwrap()
    }
    #[test]
    fn arbitrary_dataset_aliases_feed_exact_context_and_keep_unknown_records() {
        let graph = xml(
            r#"<?xml version="1.0"?><dataset custom="retained"><binary userdata="cards" filename="not-a-pssg"/><pssg processor="Other" userdata="cards" filename="ignored"/><pssg processor="OrganismPlugin" userdata="cards" filename="/data/elsewhere/nonregional-card.pssg"/><pssg processor="OrganismPlugin" userdata="bodyAlias" filename="/data/elsewhere/unusual-body.pssg"/></dataset>"#,
        );
        let binding = super::dataset(&graph).unwrap();
        let source = source();
        let defs = defs();
        let prepared = prepare(&source, &defs, &binding.pssg, selected()).unwrap();
        assert!(std::ptr::eq(binding.source, &graph));
        assert_eq!(binding.source.children.len(), 4);
        assert_eq!(prepared.dataset_resources.len(), 2);
        assert_eq!(
            first(&prepared).billboard.relative_path,
            "elsewhere/nonregional-card.pssg"
        );
        assert_eq!(
            prepared.context.primary_definition_set.as_deref(),
            Some("blue")
        );
        assert_eq!(prepared.context.map_group, "pc");
        assert!(std::ptr::eq(prepared.definitions, &defs));
    }
    #[test]
    fn dataset_alias_collision_rejects_even_when_filenames_equal() {
        let graph = xml(
            r#"<dataset><pssg processor="OrganismPlugin" userdata="x" filename="/data/a.pssg"/><pssg processor="OrganismPlugin" userdata="x" filename="/data/a.pssg"/></dataset>"#,
        );
        assert!(matches!(
            super::dataset(&graph),
            Err(Error::Duplicate {
                kind: "dataset PSSG userdata",
                ..
            })
        ));
    }
    #[test]
    fn duplicate_or_missing_selected_attributes_cannot_silently_bind() {
        let duplicate = xml(
            r#"<dataset><pssg processor="OrganismPlugin" processor="Other" userdata="x" filename="/data/a.pssg"/></dataset>"#,
        );
        assert!(matches!(
            super::dataset(&duplicate),
            Err(Error::Duplicate {
                kind: "dataset attribute",
                ..
            })
        ));
        let missing =
            xml(r#"<dataset><pssg processor="OrganismPlugin" filename="/data/a.pssg"/></dataset>"#);
        assert!(matches!(
            super::dataset(&missing),
            Err(Error::MissingAttribute {
                key: "userdata",
                ..
            })
        ));
        let root = xml("<unexpected/>");
        assert!(matches!(
            super::dataset(&root),
            Err(Error::InvalidDatasetRoot(_))
        ));
    }
    #[test]
    fn encoded_or_traversing_paths_never_become_resource_bindings() {
        for path in [
            "/data/../escape.pssg",
            "/data/anims/a&amp;b.pssg",
            "/data/%REGION%/body.pssg",
        ] {
            let graph = xml(&format!(
                r#"<dataset><pssg processor="OrganismPlugin" userdata="x" filename="{path}"/></dataset>"#
            ));
            assert!(super::dataset(&graph).is_err());
        }
    }
    #[test]
    fn empty_selected_dataset_never_borrows_other_processor_alias() {
        let graph = xml(
            r#"<dataset><pssg processor="Different" userdata="cards" filename="/data/a.pssg"/></dataset>"#,
        );
        let bindings = super::dataset(&graph).unwrap();
        assert!(bindings.pssg.is_empty());
        assert!(matches!(
            prepare(&source(), &defs(), &bindings.pssg, selected()),
            Err(Error::Missing {
                kind: "dataset PSSG userdata",
                ..
            })
        ));
    }
    #[test]
    fn multiple_variants_and_colour_sets_remain_ordered_without_selection() {
        let mut defs = defs();
        defs.elements[5].children = 9..13;
        defs.elements[12] = node(
            "textureVariant",
            &[("name", "second-variant"), ("future", "untouched")],
            17..19,
        );
        defs.elements.push(node("billboardData", &[], 19..20));
        defs.elements
            .push(node("colourSet", &[("name", "second-colour")], 20..21));
        defs.elements.push(node(
            "stance",
            &[("name", "sitting"), ("width", "0.9")],
            0..0,
        ));
        defs.elements.push(node("colour", &[("red", "0.9")], 0..0));
        let source = source();
        let prepared = prepare(&source, &defs, &dataset(), selected()).unwrap();
        let variants = &first(&prepared).variants;
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0].fields[0].1, "different-variant");
        assert_eq!(variants[1].fields[0].1, "second-variant");
        assert!(
            variants[1]
                .fields
                .contains(&("future".into(), "untouched".into()))
        );
        assert_eq!(variants[0].colour_sets[0].fields[0].1, "different-colour");
        assert_eq!(variants[1].colour_sets[0].fields[0].1, "second-colour");
        assert_eq!(prepared.resolved_static_records, 1); // alternatives are not duplicated actors
    }
    #[test]
    fn root_and_unselected_unknown_values_are_preserved_without_admission_policy() {
        let graph = xml(
            r#"<dataset unknown="a&amp;b"><pssg processor="Other" userdata="a&amp;b" filename="../not-our-path" future="x&amp;y"/><pssg processor="OrganismPlugin" userdata="chosen" filename="/data/anywhere/valid.pssg" future="x&amp;y"/></dataset>"#,
        );
        let bindings = super::dataset(&graph).unwrap();
        assert_eq!(bindings.pssg.len(), 1);
        assert_eq!(bindings.pssg[0].userdata, "chosen");
        assert_eq!(bindings.source.attr("unknown"), Some("a&amp;b"));
        assert_eq!(
            bindings.source.children[0].attr("userdata"),
            Some("a&amp;b")
        );
        assert_eq!(bindings.source.children[0].attr("future"), Some("x&amp;y"));
        assert_eq!(bindings.source.children[1].attr("future"), Some("x&amp;y"));
        assert_eq!(
            bindings.source.children[0].attr("filename"),
            Some("../not-our-path")
        );
    }
    #[test]
    fn selected_refs_and_ambiguous_processor_still_fail_closed() {
        for text in [
            r#"<dataset><pssg processor="OrganismPlugin" userdata="a&amp;b" filename="/data/a.pssg"/></dataset>"#,
            r#"<dataset><pssg processor="OrganismPlugin" userdata="x" filename="/data/a&amp;b.pssg"/></dataset>"#,
            r#"<dataset><pssg processor="Other" processor="OrganismPlugin" userdata="x" filename="/data/a.pssg"/></dataset>"#,
            r#"<dataset><pssg processor="Organism&#80;lugin" userdata="x" filename="/data/a.pssg"/></dataset>"#,
            r#"<dataset><pssg processor="OrganismPlugin" userdata="x" userdata="y" filename="/data/a.pssg"/></dataset>"#,
            r#"<dataset><pssg processor="OrganismPlugin" userdata="x" filename="/data/a.pssg" filename="/data/b.pssg"/></dataset>"#,
        ] {
            assert!(super::dataset(&xml(text)).is_err());
        }
        let mut graph = xml(
            r#"<dataset><pssg processor="Other" future="kept"/><pssg processor="OrganismPlugin" userdata="x" filename="/data/a.pssg"/></dataset>"#,
        );
        graph
            .attributes
            .push(("opaque".into(), "control\nretained".into()));
        graph.children[0]
            .attributes
            .push(("opaque".into(), "control\nretained".into()));
        assert!(super::dataset(&graph).is_ok());
        graph.children[1].attributes[1].1.push('\n');
        assert!(matches!(
            super::dataset(&graph),
            Err(Error::InvalidDatasetAttribute(_))
        ));
    }
    fn dual_defs() -> Document<'static> {
        Document {
            strings: vec![],
            string_padding: &[],
            elements: vec![
                node("crowdDefs", &[], 1..5),
                node("crowdDefSet", &[("name", "regional")], 5..6),
                node("crowdDefSet", &[("name", "generic")], 6..8),
                node("instanceMapGroup", &[("name", "pc")], 8..10),
                node("instanceSet", &[("name", "spectators-arbitrary")], 10..12),
                node(
                    "crowdDef",
                    &[("name", "body"), ("regionalOnly", "retained")],
                    12..14,
                ),
                node(
                    "crowdDef",
                    &[("name", "body"), ("genericOnly", "not-inherited")],
                    14..16,
                ),
                node("crowdDef", &[("name", "marshall")], 16..18),
                node(
                    "instanceMap",
                    &[
                        ("name", "body-map"),
                        ("crowdDef", "body"),
                        ("stance", "sitting"),
                    ],
                    0..0,
                ),
                node(
                    "instanceMap",
                    &[
                        ("name", "official-map"),
                        ("crowdDef", "marshall"),
                        ("stance", "standing"),
                    ],
                    0..0,
                ),
                node("map", &[("name", "body-map")], 0..0),
                node("map", &[("name", "official-map")], 0..0),
                node("stance", &[("name", "sitting")], 0..0),
                node("textureVariant", &[("name", "regional-variant")], 0..0),
                node("stance", &[("name", "sitting")], 0..0),
                node("textureVariant", &[("name", "generic-variant")], 0..0),
                node("stance", &[("name", "standing")], 0..0),
                node("textureVariant", &[("name", "official-variant")], 0..0),
            ],
        }
    }
    fn dual_context() -> DefinitionSelection<'static> {
        DefinitionSelection::Exact {
            primary_definition_set: Some("regional"),
            fallback_definition_set: Some("generic"),
            map_group: "pc",
            billboard_userdata: "cards",
        }
    }
    #[test]
    fn regional_override_is_one_whole_definition_not_field_inheritance() {
        let source = source();
        let mut defs = dual_defs();
        let prepared = prepare(&source, &defs, &dataset(), dual_context()).unwrap();
        let body = first(&prepared);
        assert_eq!(body.definition_origin, DefinitionOrigin::Primary);
        assert_eq!(body.definition_set, "regional");
        assert!(
            body.definition_fields
                .contains(&("regionalOnly".into(), "retained".into()))
        );
        assert!(
            !body
                .definition_fields
                .iter()
                .any(|(key, _)| key == "genericOnly")
        );
        assert_eq!(body.variants[0].fields[0].1, "regional-variant");
        // Missing required data in a FOUND primary definition must not borrow
        // the generic definition's sitting stance or variant.
        defs.elements[12].attributes[0].1 = "not-sitting";
        let failed = prepare(&source, &defs, &dataset(), dual_context()).unwrap();
        assert_eq!(failed.unresolved_static_records, 1);
        assert!(matches!(
            failed.groups[0].resolution,
            Resolution::Unresolved(Error::Missing { .. })
        ));
    }
    #[test]
    fn generic_only_official_and_map_alternatives_do_not_double_count() {
        let source = source();
        let mut defs = dual_defs();
        // A flag-role map names the same generic official definition; it is
        // another map alternative, not another actor or an unrelated region.
        defs.elements[4].children = 18..21;
        defs.elements[3].children = 21..24;
        defs.elements
            .push(node("map", &[("name", "body-map")], 0..0));
        defs.elements
            .push(node("map", &[("name", "official-map")], 0..0));
        defs.elements
            .push(node("map", &[("name", "flag-map")], 0..0));
        defs.elements.push(node(
            "instanceMap",
            &[
                ("name", "body-map"),
                ("crowdDef", "body"),
                ("stance", "sitting"),
            ],
            0..0,
        ));
        defs.elements.push(node(
            "instanceMap",
            &[
                ("name", "official-map"),
                ("crowdDef", "marshall"),
                ("stance", "standing"),
            ],
            0..0,
        ));
        defs.elements.push(node(
            "instanceMap",
            &[
                ("name", "flag-map"),
                ("crowdDef", "marshall"),
                ("stance", "standing"),
                ("propRole", "flag"),
            ],
            0..0,
        ));
        let prepared = prepare(&source, &defs, &dataset(), dual_context()).unwrap();
        let Resolution::StaticTemplates(templates) = &prepared.groups[0].resolution else {
            panic!()
        };
        assert_eq!(templates.len(), 3);
        assert_eq!(templates[0].definition_origin, DefinitionOrigin::Primary);
        assert_eq!(templates[1].definition_origin, DefinitionOrigin::Fallback);
        assert_eq!(templates[1].definition_name, "marshall");
        assert_eq!(templates[2].definition_name, "marshall");
        assert_eq!(templates[2].definition_origin, DefinitionOrigin::Fallback);
        assert!(
            templates[2]
                .map_fields
                .contains(&("propRole".into(), "flag".into()))
        );
        assert_eq!(templates[1].variants[0].fields[0].1, "official-variant");
        assert_eq!(
            (
                prepared.resolved_static_records,
                prepared.unresolved_static_records,
                prepared.unsupported_moving_records
            ),
            (1, 0, 2)
        );
    }
    #[test]
    fn absent_regional_set_uses_only_explicit_generic() {
        let source = source();
        let defs = dual_defs();
        let context = DefinitionSelection::Exact {
            primary_definition_set: Some("missing-region"),
            fallback_definition_set: Some("generic"),
            map_group: "pc",
            billboard_userdata: "cards",
        };
        let prepared = prepare(&source, &defs, &dataset(), context).unwrap();
        let Resolution::StaticTemplates(templates) = &prepared.groups[0].resolution else {
            panic!()
        };
        assert!(
            templates
                .iter()
                .all(|t| t.definition_origin == DefinitionOrigin::Fallback)
        );
        assert_eq!(templates[0].variants[0].fields[0].1, "generic-variant");
    }
    #[test]
    fn both_missing_names_never_search_an_unselected_region() {
        let source = source();
        let mut named = dual_defs();
        named.elements[8].attributes[1].1 = "absent-in-both";
        let both_miss = prepare(&source, &named, &dataset(), dual_context()).unwrap();
        assert_eq!(both_miss.unresolved_static_records, 1);
        assert!(matches!(
            both_miss.groups[0].resolution,
            Resolution::Unresolved(Error::Missing {
                kind: "crowdDef in selected primary/fallback sets",
                ..
            })
        ));
        let mut defs = dual_defs();
        // Selected sets do not have either name; the unselected regional set
        // still contains body but may not rescue an unresolved group.
        defs.elements[2].attributes[0].1 = "unselected-generic";
        let context = DefinitionSelection::Exact {
            primary_definition_set: None,
            fallback_definition_set: Some("not-present"),
            map_group: "pc",
            billboard_userdata: "cards",
        };
        let prepared = prepare(&source, &defs, &dataset(), context).unwrap();
        assert_eq!(
            (
                prepared.resolved_static_records,
                prepared.unresolved_static_records,
                prepared.unsupported_moving_records
            ),
            (0, 1, 2)
        );
        assert!(matches!(
            prepared.groups[0].resolution,
            Resolution::Unresolved(Error::Missing {
                kind: "crowdDef in selected primary/fallback sets",
                ..
            })
        ));
    }
    #[test]
    fn ascii_case_insensitive_set_and_definition_names_keep_source_spelling() {
        let source = source();
        let mut defs = dual_defs();
        defs.elements[8].attributes[1].1 = "BoDy";
        let context = DefinitionSelection::Exact {
            primary_definition_set: Some("REGIONAL"),
            fallback_definition_set: Some("GENERIC"),
            map_group: "pc",
            billboard_userdata: "cards",
        };
        let prepared = prepare(&source, &defs, &dataset(), context).unwrap();
        assert_eq!(first(&prepared).definition_set, "regional");
        assert_eq!(first(&prepared).definition_name, "body");
        assert_eq!(first(&prepared).requested_definition_name, "BoDy");
    }
    #[test]
    fn first_equal_definition_wins_in_source_order() {
        let defs = Document {
            strings: vec![],
            string_padding: &[],
            elements: vec![
                node("crowdDefSet", &[("name", "regional")], 1..3),
                node("crowdDef", &[("name", "BODY"), ("identity", "first")], 0..0),
                node(
                    "crowdDef",
                    &[("name", "body"), ("identity", "second")],
                    0..0,
                ),
            ],
        };
        let (index, entry) = source_named(&defs, 0, "crowdDef", "body").unwrap().unwrap();
        assert_eq!(index, 1);
        assert_eq!(attr(entry, "identity").unwrap(), "first");
    }
    #[test]
    fn unsupported_nonascii_names_are_explicit_not_unicode_case_parity() {
        let defs = dual_defs();
        assert!(matches!(
            selected_set(&defs, Some("régional")),
            Err(Error::UnsupportedSourceName(_))
        ));
        let source = source();
        let mut defs = defs;
        defs.elements[8].attributes[1].1 = "bödy";
        let prepared = prepare(&source, &defs, &dataset(), dual_context()).unwrap();
        assert!(matches!(
            prepared.groups[0].resolution,
            Resolution::Unresolved(Error::UnsupportedSourceName(_))
        ));
    }
    #[test]
    fn missing_crowddef_attribute_stays_unresolved_without_a_default() {
        let source = source();
        let mut defs = dual_defs();
        defs.elements[8]
            .attributes
            .retain(|(key, _)| *key != "crowdDef");
        let prepared = prepare(&source, &defs, &dataset(), dual_context()).unwrap();
        assert!(matches!(
            prepared.groups[0].resolution,
            Resolution::Unresolved(Error::MissingAttribute {
                key: "crowdDef",
                ..
            })
        ));
    }
    fn purchased_metadata(route_path: &str, region: &str, refs: [&str; 4], counts: [u32; 3]) {
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required; never skip"),
        );
        assert!(root.is_absolute() && root.is_dir());
        let route = grid_tools::route::Route::new(route_path).unwrap();
        let graph = grid_tools::tyre_smoke::read_xml(
            &root.join(route.track_file("organism_track_dataset.xml")),
        )
        .unwrap();
        let bindings = super::dataset(&graph).unwrap();
        let definition_raw = std::fs::read(root.join("anims/crowdDefs.xml")).unwrap();
        let definitions = grid_formats::binxml::read(&definition_raw).unwrap();
        let placement_raw = std::fs::read(root.join(route.file("crowd_standing.bin"))).unwrap();
        let placements = grid_formats::crowd_placements::read(&placement_raw).unwrap();
        let prepared = prepare(
            &placements,
            &definitions,
            &bindings.pssg,
            DefinitionSelection::Exact {
                primary_definition_set: Some(region),
                fallback_definition_set: Some("generic"),
                map_group: "generic",
                billboard_userdata: refs[0],
            },
        )
        .unwrap();
        assert_eq!(placements.total_instances, counts[0]);
        assert_eq!(prepared.groups.len(), placements.groups.len());
        assert!(std::ptr::eq(prepared.source, &placements));
        let mut standing = 0;
        let mut moving = 0;
        for (result, original) in prepared.groups.iter().zip(&placements.groups) {
            assert!(std::ptr::eq(result.source, original));
            match &original.placements {
                Placements::Standing(rows) => {
                    standing += original.count;
                    assert_eq!(rows.len(), original.count as usize);
                    assert!(!matches!(
                        result.resolution,
                        Resolution::UnsupportedMoving { .. }
                    ));
                }
                Placements::Moving(bytes) => {
                    moving += original.count;
                    let Resolution::UnsupportedMoving {
                        records,
                        payload_bytes,
                    } = &result.resolution
                    else {
                        panic!("moving source reclassified")
                    };
                    assert_eq!((*records, *payload_bytes), (original.count, bytes.len()));
                }
            }
        }
        assert_eq!([placements.total_instances, standing, moving], counts);
        assert_eq!(prepared.unsupported_moving_records, counts[2]);
        assert_eq!(
            prepared.resolved_static_records + prepared.unresolved_static_records,
            counts[1]
        );
        assert_eq!(
            prepared.resolved_static_records
                + prepared.unresolved_static_records
                + prepared.unsupported_moving_records,
            counts[0]
        );
        for (alias, path) in [
            (refs[0], refs[1]),
            ("maleVariant0", refs[2]),
            ("femaleVariant0", refs[3]),
            ("hla", "anims/crowd_anims.pssg"),
            ("camFlash", "anims/cr_camera_flash.pssg"),
            ("marshall", "anims/marshall.pssg"),
        ] {
            assert_eq!(
                prepared.dataset_resources.get(alias).unwrap().relative_path,
                path
            );
        }
        for (name, definition) in [
            (b"m_gr".as_slice(), "crowdMale"),
            (b"f_gr".as_slice(), "crowdFemale"),
        ] {
            let group = prepared
                .groups
                .iter()
                .find(|g| g.source.instance_set == name)
                .unwrap();
            let Resolution::StaticTemplates(templates) = &group.resolution else {
                panic!("grandstand metadata unresolved")
            };
            assert_eq!(templates.len(), 1);
            assert_eq!(templates[0].definition_origin, DefinitionOrigin::Primary);
            assert_eq!(templates[0].definition_set, region);
            assert_eq!(templates[0].definition_name, definition);
            assert_eq!(templates[0].billboard.relative_path, refs[1]);
        }
        // No claim every metadata-resolved record is drawable. All other groups,
        // including unresolved officials and opaque moving data, remain retained.
    }
    #[test]
    #[ignore = "requires purchased absolute GRID_GAME_ROOT; metadata CPU only, no GPU"]
    fn purchased_donington0_metadata_retains_all_groups_and_euro_bindings() {
        purchased_metadata(
            "tracks/europe/donington/route_0",
            "europe",
            [
                "billboard",
                "anims/cr_euro_billboard.pssg",
                "anims/cr_male_2.pssg",
                "anims/cr_female.pssg",
            ],
            [16189, 16187, 2],
        );
    }
    #[test]
    #[ignore = "requires purchased absolute GRID_GAME_ROOT; metadata CPU only, no GPU"]
    fn purchased_shibuya0_metadata_retains_all_groups_and_japan_bindings() {
        purchased_metadata(
            "tracks/japan/shibuya/route_0",
            "japan",
            [
                "billboardjapan",
                "anims/cr_jpn_billboard.pssg",
                "anims/cr_male_jpn.pssg",
                "anims/cr_female_jpn.pssg",
            ],
            [15422, 15421, 1],
        );
    }
    #[test]
    #[ignore = "requires purchased absolute GRID_GAME_ROOT; metadata CPU only, no GPU"]
    fn purchased_detroit8_metadata_retains_all_groups_and_usa_bindings() {
        purchased_metadata(
            "tracks/usa/detroit/route_8",
            "usa",
            [
                "billboardusa",
                "anims/cr_us_billboard.pssg",
                "anims/cr_male_usa.pssg",
                "anims/cr_female_usa.pssg",
            ],
            [6854, 6854, 0],
        );
    }
}
