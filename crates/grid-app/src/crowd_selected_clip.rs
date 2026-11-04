//! PRIVATE UNAPPLIED shared metadata -> explicit selected clip packet diagnostic.
//! No source clock, pose policy, native clip-ID join or renderer readiness claim.
use super::crowd_population::{self, Fields, ResourceRef, Template};
use grid_formats::{
    anim_packet,
    binxml::{Document, Element},
    pssg,
};
use grid_tools::crowd_selection::{Candidate, Library};
use std::{collections::BTreeSet, error::Error};
type AnyResult<T> = Result<T, Box<dyn Error>>;
const PLAYER_PERFORMANCE: &[&str] = &[
    "Alert",
    "Danger",
    "Base Idle",
    "Action",
    "Normal Idle",
    "Interest Idle",
    "Disapointed",
    "Race Trans to Start Action",
    "Race Start Action",
    "Race Trans from Start Action",
    "Race Trans to Danger Action",
    "Race Danger Action",
    "Race Trans from Danger Action",
    "Race Trans to Finish Action",
    "Race Finish Action",
    "Race Trans from Finish Action",
];
const TYPE: &[&str] = &[
    "Spectator",
    "Leaner",
    "Male Walker",
    "Female Walker",
    "Marshall Walker",
    "Marshall",
    "Cameraman",
    "Grandstand",
    "FE Driver 1",
    "FE Mechanic 1",
    "FE Mechanic 2",
    "FE Mechanic 3",
    "FE Secretary 1",
    "FE Mechanic 1 Level 1",
    "FE Mechanic 1 Level 2",
    "FE Mechanic 2 Level 3",
    "FE Mechanic 2 Level 4",
    "FE Mechanic 2 Level 5",
    "FE Mechanic 3 Level 3",
    "FE Mechanic 4 Level 4",
    "FE Mechanic 4 Level 5",
    "FE Driver 1 Level 3",
    "FE Driver 1 Level 4",
    "FE Driver 1 Level 5",
    "FE Secretary 1 Level 5",
    "FE Driver 1 Walk",
    "FE Mechanic 1 Walk",
];
const SEX: &[&str] = &["M", "F"];
const PROP: &[&str] = &[
    "None",
    "Marshall Flag",
    "Boned Flag",
    "Start Flag",
    "Finish Flag",
    "Foghorn",
    "Camera",
    "Umbrella",
    "Clipboard",
    "Walkie Talkie",
    "Stopwatch",
    "Whistle",
    "Procamera",
    "Board",
    "Flag_sm",
    "Flag_La",
    "Vid_Cam",
    "TV Camera handheld",
    "TV camera mounted",
    "Flare",
    "Binoculars",
    "Phone",
    "Bottle",
    "Balloon",
    "Foam hands",
    "Plastic Tubes",
    "Food",
    "Rag",
    "Magazine",
    "Mug",
    "Spanner",
    "Tyres",
    "Monkey Wrench",
    "Laptop",
    "Headphones",
];
const SET: &[&str] = &["1", "2"];
fn attribute<'a>(e: &Element<'a>, key: &str) -> AnyResult<Option<&'a str>> {
    let mut found = e.attributes.iter().filter(|(k, _)| *k == key);
    let value = found.next().map(|(_, v)| *v);
    if found.next().is_some() {
        return Err("duplicate interpreted selector attribute".into());
    }
    Ok(value)
}
fn field<'a>(fields: &'a Fields, key: &str) -> AnyResult<Option<&'a str>> {
    let mut found = fields.iter().filter(|(k, _)| k == key);
    let value = found.next().map(|(_, v)| v.as_str());
    if found.next().is_some() {
        return Err("duplicate selected map field".into());
    }
    Ok(value)
}
fn root_children(d: &Document<'_>) -> AnyResult<std::ops::Range<usize>> {
    let root = d.elements.first().ok_or("empty source definitions")?;
    if root.name != "crowdDefs"
        || root.children.start > root.children.end
        || root.children.end > d.elements.len()
    {
        return Err("invalid source definitions root/range".into());
    }
    Ok(root.children.clone())
}
fn category(value: Option<&str>, vocabulary: &'static [&'static str]) -> &'static str {
    // Exact source case-insensitive lookup and missing/unmatched index0 policy.
    value
        .and_then(|v| {
            vocabulary
                .iter()
                .copied()
                .find(|x| x.eq_ignore_ascii_case(v))
        })
        .unwrap_or(vocabulary[0])
}
/// Concrete source instanceMap.animSelector consumer, NOT crowdDef fields.
/// Performance is explicitly caller-owned experimental state. None/out-of-range
/// means no positive category, hence source-declared Factor default. Rig Type is
/// deliberately absent: fresh native descriptor is [0,0], not forced3D.
fn inputs(
    defs: &Document<'_>,
    map: &Fields,
    performance_index: Option<u32>,
) -> AnyResult<Vec<(&'static str, &'static str)>> {
    let selected = field(map, "animSelector")?;
    let mut selector = None;
    if let Some(name) = selected {
        for i in root_children(defs)? {
            let e = defs
                .elements
                .get(i)
                .ok_or("selector index outside document")?;
            if e.name == "animSelector"
                && attribute(e, "name")?.is_some_and(|n| n.eq_ignore_ascii_case(name))
            {
                if selector.is_some() {
                    return Err("ambiguous selected animSelector (authored admission)".into());
                }
                selector = Some(e);
            }
        }
    }
    // Unresolved selected reference is retained as explicit failure; a genuinely
    // missing selector follows proven native four-word zero setup.
    if selected.is_some() && selector.is_none() {
        return Err("unresolved selected animSelector".into());
    }
    let mut result = Vec::new();
    for (factor, key, vocabulary) in [
        ("Type", "type", TYPE),
        ("Sex", "gender", SEX),
        ("Set", "animSet", SET),
        ("Prop", "prop", PROP),
    ] {
        let value = selector.map(|e| attribute(e, key)).transpose()?.flatten();
        result.push((factor, category(value, vocabulary)));
    }
    if let Some(value) = performance_index.and_then(|i| PLAYER_PERFORMANCE.get(i as usize)) {
        result.push(("Player Performance", *value));
    }
    Ok(result)
}
pub(crate) struct Selected<'a> {
    pub candidate: Candidate<'a>,
    pub raw_ordinal: usize,
    /// Comes from current dataset's exact alias; never hardcoded crowd_anims path.
    pub animation_bank: ResourceRef,
}
pub(crate) fn select_template<'a>(
    metadata: &crowd_population::Prepared<'_, '_, '_, '_>,
    template: &Template,
    library: &Library<'a>,
    experimental_performance_index: Option<u32>,
    draw: &mut impl FnMut() -> u32,
) -> AnyResult<Option<Selected<'a>>> {
    let values = inputs(
        metadata.definitions,
        &template.map_fields,
        experimental_performance_index,
    )?;
    let Some(candidate) = library
        .select(&values, draw)
        .map_err(|e| format!("source HLA selection: {e:?}"))?
    else {
        return Ok(None);
    };
    let raw_ordinal = candidate
        .unique_ordinal()
        .map_err(|e| format!("raw catalogue admission: {e:?}"))?;
    let animation_bank = metadata
        .dataset_resources
        .get("hla")
        .ok_or("selected dataset has no hla PSSG resource")?
        .clone();
    Ok(Some(Selected {
        candidate,
        raw_ordinal,
        animation_bank,
    }))
}
/// Diagnostic binding MUST be supplied explicitly. The raw name/description has
/// NOT been silently reinterpreted as a source clip ID. Parent must establish
/// this join before any automatic production adapter/default is admitted.
pub(crate) struct ExplicitClipBinding<'a> {
    pub raw_ordinal: usize,
    pub clip_id: &'a [u8],
}
pub(crate) struct ChannelSample {
    pub target: String,
    pub kind: anim_packet::Kind,
    pub raw: [f32; 4],
}
pub(crate) fn sample_bound_clip(
    selected: &Selected<'_>,
    binding: ExplicitClipBinding<'_>,
    bank: &pssg::Tree<'_>,
    bank_coordinate: f32,
) -> AnyResult<Vec<ChannelSample>> {
    if selected.raw_ordinal != binding.raw_ordinal || !bank_coordinate.is_finite() {
        return Err("explicit binding/coordinate mismatch".into());
    }
    if selected.candidate.mirrored {
        return Err("selected mirroring pose operation unresolved".into());
    }
    let mut clips = (0..bank.nodes.len()).filter(|&i| {
        bank.name(i) == b"NeAnimClip" && bank.string(i, b"id").ok() == Some(binding.clip_id)
    });
    let clip = clips.next().ok_or("explicit clip ID absent")?;
    if clips.next().is_some() {
        return Err("explicit clip ID ambiguous".into());
    }
    let refs: Vec<_> = bank.nodes[clip]
        .children
        .iter()
        .copied()
        .filter(|&i| bank.name(i) == b"NeAnimClipPacketRef")
        .collect();
    if refs.is_empty()
        || refs.len() > 64
        || bank.integer(clip, b"packetCount")? as usize != refs.len()
    {
        return Err("clip packet count absent/inconsistent/over authored cap".into());
    }
    let mut samples = Vec::new();
    let mut targets = BTreeSet::new();
    let mut seen_packets = BTreeSet::new();
    for reference in refs {
        let id = bank
            .string(reference, b"packet")?
            .strip_prefix(b"#")
            .ok_or("external packet reference unsupported")?;
        if !seen_packets.insert(id) {
            return Err("duplicate packet reference".into());
        }
        let mut parents = (0..bank.nodes.len()).filter(|&i| {
            matches!(bank.name(i), b"NeAnimPacket_S4" | b"NeAnimPacket_L4")
                && bank.string(i, b"id").ok() == Some(id)
        });
        let parent = parents
            .next()
            .ok_or("ordinary packet reference unresolved")?;
        if parents.next().is_some() {
            return Err("ordinary packet reference ambiguous".into());
        }
        let (parent_type, leaf_type) = if bank.name(parent) == b"NeAnimPacket_S4" {
            ("NeAnimPacket_S4", "NeAnimPacketData_S4")
        } else {
            ("NeAnimPacket_L4", "NeAnimPacketData_L4")
        };
        let [leaf] = bank.nodes[parent].children.as_slice() else {
            return Err("ordinary packet requires exact direct leaf".into());
        };
        let node = &bank.nodes[*leaf];
        if bank.name(*leaf) != leaf_type.as_bytes()
            || node.parent != Some(parent)
            || !node.children.is_empty()
            || !node.attributes.is_empty()
        {
            return Err("ordinary packet leaf shape unsupported".into());
        }
        let packet = anim_packet::read(
            node.data.ok_or("ordinary packet payload absent")?,
            anim_packet::Envelope {
                parent_type,
                leaf_type,
                parent_alignment: bank.integer(parent, b"alignment")?,
                native_size: bank.integer(parent, b"size")? as usize,
            },
        )?;
        for (i, channel) in packet.channels.iter().enumerate() {
            let kind = match packet.kind {
                anim_packet::Kind::S4 => 0u8,
                anim_packet::Kind::L4 => 1u8,
            };
            if !targets.insert((kind, channel.target.to_owned())) {
                return Err("duplicate channel semantic/target across selected packets".into());
            }
            samples.push(ChannelSample {
                target: channel.target.to_owned(),
                kind: packet.kind,
                raw: packet.sample(i, bank_coordinate)?,
            });
        }
    }
    Ok(samples)
}
/// Explicit complete diagnostic frame input, never a selected runtime default.
pub(crate) struct DiagnosticPoseRequest<'a> {
    pub clip: ExplicitClipBinding<'a>,
    pub bank_coordinate: f32,
    pub skin_id: &'a [u8],
    pub modifier_instance_id: &'a [u8],
    pub caller_prior_locals: &'a [[f32; 16]],
    pub policy: super::crowd_pose::PosePolicy,
}
/// Consumes selection+packet samples in the generic retained rig extraction.
/// Geometry tree must be loaded from selected template.mesh ResourceRef by caller.
pub(crate) fn pose_bound_clip(
    selected: &Selected<'_>,
    bank: &pssg::Tree<'_>,
    geometry: &pssg::Tree<'_>,
    request: DiagnosticPoseRequest<'_>,
) -> AnyResult<(super::crowd_pose::Source, super::crowd_pose::VertexBuffers)> {
    let samples = sample_bound_clip(selected, request.clip, bank, request.bank_coordinate)?;
    let rig =
        super::crowd_pose::Source::read(geometry, request.skin_id, request.modifier_instance_id)?;
    let channels: Vec<_> = samples
        .iter()
        .map(|s| super::crowd_pose::PoseChannel {
            target: s.target.as_bytes(),
            kind: s.kind,
            raw: s.raw,
        })
        .collect();
    let vertices = rig.sample(request.caller_prior_locals, &channels, request.policy)?;
    Ok((rig, vertices))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn defs(attrs: Vec<(&'static str, &'static str)>) -> Document<'static> {
        Document {
            strings: vec![],
            string_padding: &[],
            elements: vec![
                Element {
                    name: "crowdDefs",
                    value: None,
                    attributes: vec![],
                    children: 1..2,
                    parent: None,
                },
                Element {
                    name: "animSelector",
                    value: None,
                    attributes: attrs,
                    children: 2..2,
                    parent: Some(0),
                },
            ],
        }
    }
    #[test]
    fn actual_source_fields_feed_categories_without_rig_or_random_word() {
        let d = defs(vec![
            ("name", "femaleSit"),
            ("type", "grandstand"),
            ("gender", "f"),
            ("animSet", "2"),
            ("prop", "camera"),
        ]);
        let map = vec![("animSelector".into(), "FEMALESIT".into())];
        let result = inputs(&d, &map, Some(4)).unwrap();
        assert_eq!(
            result,
            vec![
                ("Type", "Grandstand"),
                ("Sex", "F"),
                ("Set", "2"),
                ("Prop", "Camera"),
                ("Player Performance", "Normal Idle")
            ]
        );
        assert!(!result.iter().any(|(name, _)| *name == "Rig Type"));
    }
    #[test]
    fn missing_selector_is_zero_categories_but_missing_named_reference_refuses() {
        let d = defs(vec![("name", "present")]);
        assert_eq!(
            inputs(&d, &vec![], None).unwrap(),
            vec![
                ("Type", "Spectator"),
                ("Sex", "M"),
                ("Set", "1"),
                ("Prop", "None")
            ]
        );
        assert!(inputs(&d, &vec![("animSelector".into(), "absent".into())], None).is_err());
        assert!(
            inputs(
                &d,
                &vec![
                    ("animSelector".into(), "present".into()),
                    ("animSelector".into(), "present".into())
                ],
                None
            )
            .is_err()
        );
    }
    #[test]
    fn unknown_attributes_follow_source_zero_policy_and_bad_performance_is_absent() {
        let d = defs(vec![
            ("name", "present"),
            ("type", "future"),
            ("gender", "future"),
            ("animSet", "future"),
            ("prop", "future"),
        ]);
        let result = inputs(
            &d,
            &vec![("animSelector".into(), "present".into())],
            Some(u32::MAX),
        )
        .unwrap();
        assert_eq!(
            result,
            vec![
                ("Type", "Spectator"),
                ("Sex", "M"),
                ("Set", "1"),
                ("Prop", "None")
            ]
        );
    }
}
