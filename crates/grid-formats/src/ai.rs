//! Typed AI driving data (docs/AI_DRIVER_RESEARCH.md): `ai_track.xml` gates,
//! links and brake lines, per-class `ai_vehicle_track.xml` brake data,
//! `ai_vehicle_statistics.xml` speed->distance tables, named config sections and
//! difficulty/characteristic mappings. Values only; driving behaviour is external.
use crate::{
    Result,
    binxml::{Document, Element},
    fail,
};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct Gate {
    /// Left track edge.
    pub position: [f32; 3],
    /// Horizontal left->right unit direction.
    pub normal: [f32; 3],
    /// Metres along `normal` from `position`.
    pub racing_line: f32,
    pub left_track_limit: f32,
    pub right_track_limit: f32,
    pub left_racing_limit: f32,
    pub right_racing_limit: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrakeData {
    pub min_speed: f32,
    pub max_speed: f32,
    pub drift_speed: f32,
    pub distance_modifier: f32,
    /// Seconds; -1 when absent (parser default).
    pub hold_time: f32,
}

#[derive(Debug, Clone)]
pub struct BrakeLine {
    pub gate: usize,
    pub data: BrakeData,
}

#[derive(Debug, Clone)]
pub struct Track {
    pub gates: Vec<Gate>,
    /// `(from_gate, to_gate)` in declared order.
    pub links: Vec<(usize, usize)>,
    pub brake_lines: Vec<BrakeLine>,
}

fn element<'a>(doc: &'a Document<'a>, index: usize) -> Result<&'a Element<'a>> {
    doc.elements.get(index).ok_or(crate::Error {
        offset: index,
        message: "AI element absent",
    })
}
fn attr<'a>(e: &Element<'a>, name: &str) -> Result<&'a str> {
    e.attributes
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value)
        .ok_or(crate::Error {
            offset: 0,
            message: "required AI attribute absent",
        })
}
fn number(value: &str) -> Result<f32> {
    // Shipped data contains C-style literals such as "3.0f".
    match value.trim().trim_end_matches('f').parse::<f32>() {
        Ok(v) if v.is_finite() => Ok(v),
        _ => fail(0, "invalid AI float"),
    }
}
fn integer(e: &Element<'_>, name: &str) -> Result<usize> {
    attr(e, name)?.parse().map_err(|_| crate::Error {
        offset: 0,
        message: "invalid AI integer",
    })
}
/// Children of `index` with the given element name.
fn named<'a>(doc: &'a Document<'a>, index: usize, name: &str) -> Result<Vec<usize>> {
    Ok(element(doc, index)?
        .children
        .clone()
        .filter(|&c| doc.elements[c].name == name)
        .collect())
}
fn only<'a>(doc: &'a Document<'a>, index: usize, name: &str) -> Result<usize> {
    match named(doc, index, name)?[..] {
        [one] => Ok(one),
        _ => fail(index, "expected one AI child element"),
    }
}
/// Child of `index` named `name` whose `name` attribute equals `value`.
fn keyed<'a>(
    doc: &'a Document<'a>,
    index: usize,
    name: &str,
    value: &str,
) -> Result<Option<usize>> {
    for c in named(doc, index, name)? {
        if attr(&doc.elements[c], "name")? == value {
            return Ok(Some(c));
        }
    }
    Ok(None)
}
fn vector(doc: &Document<'_>, index: usize) -> Result<[f32; 3]> {
    let e = element(doc, index)?;
    if attr(e, "format")? != "float3" {
        return fail(index, "expected AI float3");
    }
    let values: Vec<f32> = e
        .value
        .unwrap_or("")
        .split_whitespace()
        .map(number)
        .collect::<Result<_>>()?;
    values.try_into().map_err(|_| crate::Error {
        offset: index,
        message: "expected three AI coordinates",
    })
}
fn records(doc: &Document<'_>, index: usize, count: &str, record: &str) -> Result<Vec<usize>> {
    let items = named(doc, index, record)?;
    if items.len() != integer(element(doc, index)?, count)? || items.is_empty() {
        return fail(index, "AI record count mismatch");
    }
    for (ordinal, &item) in items.iter().enumerate() {
        if integer(&doc.elements[item], "id")? != ordinal {
            return fail(item, "AI IDs must follow declared order");
        }
    }
    Ok(items)
}
fn brake_data(e: &Element<'_>) -> Result<BrakeData> {
    let optional = |name| {
        e.attributes
            .iter()
            .find(|(k, _)| *k == name)
            .map(|(_, v)| number(v))
            .transpose()
    };
    let max_speed = number(attr(e, "max_speed")?)?;
    Ok(BrakeData {
        min_speed: number(attr(e, "min_speed")?)?,
        max_speed,
        drift_speed: optional("drift_speed")?.unwrap_or(max_speed - 5.),
        distance_modifier: number(attr(e, "distance_modifier")?)?,
        hold_time: optional("hold_time")?.unwrap_or(-1.),
    })
}

/// `tracks/<track>/route_0/ai_track.xml`, track `default`.
pub fn track(doc: &Document<'_>) -> Result<Track> {
    let root = element(doc, 0)?;
    if root.name != "ai_track_data" || attr(root, "version_major")? != "3" {
        return fail(0, "unsupported AI track data");
    }
    let track = keyed(doc, 0, "track", "default")?.ok_or(crate::Error {
        offset: 0,
        message: "default AI track absent",
    })?;
    let mut out = Track {
        gates: Vec::new(),
        links: Vec::new(),
        brake_lines: Vec::new(),
    };
    for gate in records(doc, only(doc, track, "gates")?, "num_gates", "gate")? {
        let mut lengths = BTreeMap::new();
        for w in named(doc, only(doc, gate, "waypoints")?, "waypoint")? {
            let e = &doc.elements[w];
            let kind = attr(e, "type")?;
            if kind == "racing_line"
                && named(doc, w, "racing_line")?
                    .iter()
                    .any(|&r| attr(&doc.elements[r], "type").ok() != Some("optimal"))
            {
                return fail(w, "only optimal AI racing lines supported");
            }
            if lengths.insert(kind, number(attr(e, "length")?)?).is_some() {
                return fail(w, "duplicate AI waypoint type");
            }
        }
        let get = |k: &str| {
            lengths.get(k).copied().ok_or(crate::Error {
                offset: gate,
                message: "AI waypoint absent",
            })
        };
        out.gates.push(Gate {
            position: vector(doc, only(doc, gate, "position")?)?,
            normal: vector(doc, only(doc, gate, "normal")?)?,
            racing_line: get("racing_line")?,
            left_track_limit: get("left_track_limit")?,
            right_track_limit: get("right_track_limit")?,
            left_racing_limit: get("left_racing_limit")?,
            right_racing_limit: get("right_racing_limit")?,
        });
    }
    for link in records(doc, only(doc, track, "links")?, "num_links", "link")? {
        let e = &doc.elements[link];
        let (from, to) = (integer(e, "from_gate")?, integer(e, "to_gate")?);
        if from >= out.gates.len() || to >= out.gates.len() {
            return fail(link, "AI link gate out of bounds");
        }
        out.links.push((from, to));
    }
    for line in records(
        doc,
        only(doc, track, "brake_lines")?,
        "num_brake_lines",
        "brake_line",
    )? {
        let gate = integer(&doc.elements[line], "gate_id")?;
        if gate >= out.gates.len() {
            return fail(line, "AI brake line gate out of bounds");
        }
        out.brake_lines.push(BrakeLine {
            gate,
            data: brake_data(&doc.elements[only(doc, line, "brake_data")?])?,
        });
    }
    Ok(out)
}

/// Per-class brake data from `route_0/ai_vehicle_track.xml` (track `default`),
/// indexed by brake-line id; `None` when the class has no entry.
pub fn class_brake_lines(doc: &Document<'_>, class: &str) -> Result<Option<Vec<BrakeData>>> {
    if element(doc, 0)?.name != "vehicle_type_track_data" {
        return fail(0, "unsupported AI vehicle track data");
    }
    let Some(track) = keyed(doc, 0, "vehicle_track", "default")? else {
        return Ok(None);
    };
    let Some(class) = keyed(
        doc,
        only(doc, track, "vehicle_types")?,
        "vehicle_type",
        class,
    )?
    else {
        return Ok(None);
    };
    records(
        doc,
        only(doc, class, "brake_lines")?,
        "num_brake_lines",
        "brake_line",
    )?
    .into_iter()
    .map(|line| brake_data(&doc.elements[only(doc, line, "brake_data")?]))
    .collect::<Result<_>>()
    .map(Some)
}

/// One grip's speed->distance table (m/s -> m), speeds increasing.
#[derive(Debug, Clone)]
pub struct GripTable {
    pub grip: f32,
    pub points: Vec<(f32, f32)>,
}

#[derive(Debug, Clone)]
pub struct Statistics {
    pub brake: Vec<GripTable>,
    pub cruise: Vec<GripTable>,
    pub accelerate: Vec<GripTable>,
}

fn grip_tables(doc: &Document<'_>, index: usize) -> Result<Vec<GripTable>> {
    let set = only(doc, index, "grip_performances")?;
    let mut out = Vec::new();
    for g in named(doc, set, "grip_performance")? {
        let mut points = Vec::new();
        for p in named(doc, g, "speed_distance")? {
            let e = &doc.elements[p];
            points.push((number(attr(e, "speed")?)?, number(attr(e, "distance")?)?));
        }
        if points.is_empty() || points.windows(2).any(|w| w[1].0 <= w[0].0) {
            return fail(g, "AI speed table must increase");
        }
        out.push(GripTable {
            grip: number(attr(&doc.elements[g], "grip")?)?,
            points,
        });
    }
    if out.is_empty() || out.windows(2).any(|w| w[1].grip <= w[0].grip) {
        return fail(set, "AI grip tables must increase");
    }
    Ok(out)
}

/// `ai/ai_vehicle_statistics.xml` tables for one car (e.g. `lac`).
pub fn statistics(doc: &Document<'_>, car: &str) -> Result<Statistics> {
    if element(doc, 0)?.name != "ai_vehicle_data" {
        return fail(0, "unsupported AI vehicle statistics");
    }
    let car =
        keyed(doc, only(doc, 0, "vehicle_types")?, "vehicle_type", car)?.ok_or(crate::Error {
            offset: 0,
            message: "AI vehicle statistics absent for car",
        })?;
    let all = only(doc, car, "all_grip_performances")?;
    Ok(Statistics {
        brake: grip_tables(doc, only(doc, all, "brake_grip_performances")?)?,
        cruise: grip_tables(doc, only(doc, all, "cruise_grip_performances")?)?,
        accelerate: grip_tables(doc, only(doc, all, "accelerate_grip_performances")?)?,
    })
}

/// Named scalar parameters of one config section (`<name float=".."/>`,
/// `int`, `bool` as 0/1). `path` selects nested sections; a `(name, key)`
/// step selects the child whose `name` attribute is `key`.
pub fn config(
    doc: &Document<'_>,
    root: &str,
    path: &[(&str, Option<&str>)],
) -> Result<BTreeMap<String, f32>> {
    if element(doc, 0)?.name != root {
        return fail(0, "unexpected AI config root");
    }
    let mut at = 0;
    for &(name, key) in path {
        at = match key {
            Some(key) => keyed(doc, at, name, key)?.ok_or(crate::Error {
                offset: at,
                message: "AI config section absent",
            })?,
            None => only(doc, at, name)?,
        };
    }
    let mut out = BTreeMap::new();
    for c in element(doc, at)?.children.clone() {
        let e = &doc.elements[c];
        let value = if let Ok(v) = attr(e, "float") {
            number(v)?
        } else if let Ok(v) = attr(e, "int") {
            number(v)?
        } else if let Ok(v) = attr(e, "bool") {
            match v {
                "true" => 1.,
                "false" => 0.,
                _ => return fail(c, "invalid AI config bool"),
            }
        } else {
            continue;
        };
        out.insert(e.name.to_string(), value);
    }
    Ok(out)
}

/// One driver parameter mapping over (difficulty, characteristic value).
#[derive(Debug, Clone)]
pub struct Mapping {
    pub characteristic: String,
    pub value: f32,
    /// `(difficulty, characteristic_value, value)`.
    pub limits: Vec<(f32, f32, f32)>,
}

impl Mapping {
    /// Bilinear evaluation (original 0x7A7A00): interpolate on the
    /// characteristic within the difficulty-0 and difficulty-1 limits, then on
    /// difficulty, each clamped; no limits -> `value`.
    pub fn evaluate(&self, difficulty: f32, characteristic: f32) -> f32 {
        let find = |d: f32, c: f32| {
            self.limits
                .iter()
                .find(|l| l.0 == d && l.1 == c)
                .map(|l| l.2)
        };
        let (Some(a0), Some(a1), Some(b0), Some(b1)) =
            (find(0., 0.), find(0., 1.), find(1., 0.), find(1., 1.))
        else {
            return self.value;
        };
        let c = characteristic.clamp(0., 1.);
        let d = difficulty.clamp(0., 1.);
        let low = a0 + (a1 - a0) * c;
        let high = b0 + (b1 - b0) * c;
        low + (high - low) * d
    }
}

/// `ai/ai_driver_parameter_mappings.xml` section (e.g. `default`), by variable.
pub fn mappings(doc: &Document<'_>, section: &str) -> Result<BTreeMap<String, Mapping>> {
    if element(doc, 0)?.name != "driver_parameter_mappings" {
        return fail(0, "unsupported AI parameter mappings");
    }
    let at = only(doc, 0, section)?;
    let mut out = BTreeMap::new();
    for m in named(doc, at, "mapping")? {
        let e = &doc.elements[m];
        let mut limits = Vec::new();
        for l in named(doc, m, "limit")? {
            let le = &doc.elements[l];
            limits.push((
                number(attr(le, "difficulty")?)?,
                number(attr(le, "characteristic_value")?)?,
                number(le.value.unwrap_or(""))?,
            ));
        }
        if limits.len() != integer(e, "num_limits")? {
            return fail(m, "AI mapping limit count mismatch");
        }
        let mapping = Mapping {
            characteristic: attr(e, "characteristic")?.to_string(),
            value: number(attr(e, "value")?)?,
            limits,
        };
        if out
            .insert(attr(e, "ai_variable")?.to_string(), mapping)
            .is_some()
        {
            return fail(m, "duplicate AI mapping");
        }
    }
    Ok(out)
}
