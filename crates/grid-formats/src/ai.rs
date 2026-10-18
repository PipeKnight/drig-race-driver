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

/// `tracks/<track>/route_0/ai_track.xml`, track `default`, on the `optimal`
/// racing line.
pub fn track(doc: &Document<'_>) -> Result<Track> {
    track_on(doc, "optimal")
}

/// [`track`] on the racing line of `kind` (the executable's kinds, .rdata
/// 0xCF7128: retirement 0, optimal 1, overtake 2, left 3, right 4, traffic 5,
/// drift 6, start 7). A gate without that kind keeps its `optimal` line, as
/// the line choice 0x794870 falls back to kind 1 where the wanted one is absent.
pub fn track_on(doc: &Document<'_>, wanted: &str) -> Result<Track> {
    track_named(doc, "default", wanted)
}

/// [`track_on`] on the AI track `name`: `default`, or `reverse` on
/// Haruna's point-to-point routes, which ship both directions in one file.
pub fn track_named(doc: &Document<'_>, name: &str, wanted: &str) -> Result<Track> {
    let root = element(doc, 0)?;
    if root.name != "ai_track_data" || attr(root, "version_major")? != "3" {
        return fail(0, "unsupported AI track data");
    }
    let track = keyed(doc, 0, "track", name)?.ok_or(crate::Error {
        offset: 0,
        message: "AI track absent",
    })?;
    let mut out = Track {
        gates: Vec::new(),
        links: Vec::new(),
        brake_lines: Vec::new(),
    };
    for gate in records(doc, only(doc, track, "gates")?, "num_gates", "gate")? {
        let mut lengths = BTreeMap::new();
        let mut chosen = None;
        for w in named(doc, only(doc, gate, "waypoints")?, "waypoint")? {
            let e = &doc.elements[w];
            let kind = attr(e, "type")?;
            if kind == "racing_line" && wanted != "optimal" {
                let kinds = named(doc, w, "racing_line")?
                    .iter()
                    .map(|&r| attr(&doc.elements[r], "type"))
                    .collect::<Result<Vec<_>>>()?;
                if kinds == [wanted] {
                    if chosen.replace(number(attr(e, "length")?)?).is_some() {
                        return fail(w, "duplicate AI racing line kind");
                    }
                    continue;
                }
            }
            if kind == "racing_line" {
                // Racing-line kinds of the executable's enumeration (.rdata
                // 0xCF7120): the race line is `optimal`; other kinds (the drift
                // routes ship `drift` on every gate) are skipped here and read
                // by `track_on`. Mixed or unknown kinds fail.
                let kinds = named(doc, w, "racing_line")?
                    .iter()
                    .map(|&r| attr(&doc.elements[r], "type"))
                    .collect::<Result<Vec<_>>>()?;
                if !kinds.iter().all(|&k| k == "optimal") {
                    if kinds.iter().all(|k| {
                        [
                            "retirement",
                            "overtake",
                            "left",
                            "right",
                            "traffic",
                            "drift",
                            "start",
                        ]
                        .contains(k)
                    }) && !kinds.is_empty()
                    {
                        continue;
                    }
                    return fail(w, "only optimal AI racing lines supported");
                }
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
            racing_line: match chosen {
                Some(length) => length,
                None => get("racing_line")?,
            },
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
    class_brake_lines_on(doc, "default", class)
}

/// The vehicle classes of vehicle track `name`, in file order.
pub fn class_names_on<'a>(doc: &'a Document<'a>, name: &str) -> Result<Vec<&'a str>> {
    if element(doc, 0)?.name != "vehicle_type_track_data" {
        return fail(0, "unsupported AI vehicle track data");
    }
    let Some(track) = keyed(doc, 0, "vehicle_track", name)? else {
        return Ok(Vec::new());
    };
    named(doc, only(doc, track, "vehicle_types")?, "vehicle_type")?
        .into_iter()
        .map(|t| attr(&doc.elements[t], "name"))
        .collect()
}

/// [`class_brake_lines`] of the vehicle track `name` (`default`/`reverse`).
pub fn class_brake_lines_on(
    doc: &Document<'_>,
    name: &str,
    class: &str,
) -> Result<Option<Vec<BrakeData>>> {
    if element(doc, 0)?.name != "vehicle_type_track_data" {
        return fail(0, "unsupported AI vehicle track data");
    }
    let Some(track) = keyed(doc, 0, "vehicle_track", name)? else {
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

fn optional<'a>(e: &Element<'a>, name: &str) -> Option<&'a str> {
    e.attributes
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value)
}
fn flag(value: &str) -> Result<bool> {
    match value {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => fail(0, "invalid AI bool"),
    }
}
fn whole<T: std::str::FromStr>(value: &str) -> Result<T> {
    value.trim().parse().map_err(|_| crate::Error {
        offset: 0,
        message: "invalid AI integer",
    })
}

/// One `catchup.xml` config (parser 0x90A590; reset defaults where absent,
/// docs/AI_MISTAKES_PACING_RESEARCH.md §4.1).
#[derive(Debug, Clone, PartialEq)]
pub struct CatchUp {
    pub enabled: bool,
    pub min_distance: f32,
    pub max_distance: f32,
    pub power_at_min_distance: f32,
    pub power_at_max_distance: f32,
    pub min_speed: f32,
    pub min_race_distance: f32,
    pub player_catchup: bool,
    pub higher_class: bool,
    pub use_lead_for_max_distance: bool,
    pub reverse_enabled: bool,
    pub reverse_min_distance: f32,
    pub reverse_max_distance: f32,
    pub reverse_power_at_min_distance: f32,
    pub reverse_power_at_max_distance: f32,
}

impl Default for CatchUp {
    fn default() -> Self {
        Self {
            enabled: false,
            min_distance: 0.,
            max_distance: 0.,
            power_at_min_distance: 1.,
            power_at_max_distance: 1.,
            min_speed: 0.,
            min_race_distance: 0.,
            player_catchup: false,
            higher_class: false,
            use_lead_for_max_distance: false,
            reverse_enabled: false,
            reverse_min_distance: 0.,
            reverse_max_distance: 0.,
            reverse_power_at_min_distance: 1.,
            reverse_power_at_max_distance: 1.,
        }
    }
}

/// The first `catch_up` config whose `user name` is `user` (loader 0x90A790);
/// `None` when no config names it (the original `none`).
pub fn catch_up(doc: &Document<'_>, user: &str) -> Result<Option<CatchUp>> {
    if element(doc, 0)?.name != "catch_up" {
        return fail(0, "unexpected catch-up root");
    }
    for c in named(doc, 0, "config")? {
        let users = named(doc, c, "user")?;
        if !users
            .iter()
            .any(|&u| optional(&doc.elements[u], "name") == Some(user))
        {
            continue;
        }
        let mut out = CatchUp::default();
        for p in named(doc, c, "param")? {
            for &(key, value) in &doc.elements[p].attributes {
                match key {
                    "enabled" => out.enabled = flag(value)?,
                    "minDistance" => out.min_distance = number(value)?,
                    "maxDistance" => out.max_distance = number(value)?,
                    "powerAtMinDistance" => out.power_at_min_distance = number(value)?,
                    "powerAtMaxDistance" => out.power_at_max_distance = number(value)?,
                    "minSpeed" => out.min_speed = number(value)?,
                    "minRaceDistance" => out.min_race_distance = number(value)?,
                    "playerCatchup" => out.player_catchup = flag(value)?,
                    "higherClass" => out.higher_class = flag(value)?,
                    "useLeadForMaxDistance" => out.use_lead_for_max_distance = flag(value)?,
                    "reverseEnabled" => out.reverse_enabled = flag(value)?,
                    "reverseMinDistance" => out.reverse_min_distance = number(value)?,
                    "reverseMaxDistance" => out.reverse_max_distance = number(value)?,
                    "reversePowerAtMinDistance" => {
                        out.reverse_power_at_min_distance = number(value)?
                    }
                    "reversePowerAtMaxDistance" => {
                        out.reverse_power_at_max_distance = number(value)?
                    }
                    // render/renderOnCars are debug drawing only.
                    _ => {}
                }
            }
        }
        return Ok(Some(out));
    }
    Ok(None)
}

/// Choreographer manager block (0x401AA0; built-in defaults where absent).
#[derive(Debug, Clone, PartialEq)]
pub struct Manager {
    pub enable: bool,
    pub remove_unsafe_retires: bool,
    pub removal_distance_crow: f32,
    pub removal_distance_track: f32,
    pub lapping_yield_distance: f32,
    pub team_mate_yield_distance: f32,
    pub race_start_distance: [f32; 2],
    pub race_start_factor: [f32; 2],
    pub min_distance_offline: f32,
    pub max_distance_offline: f32,
    pub max_assist_inside: f32,
    pub max_assist_outside: f32,
    pub min_curve_offline: f32,
    pub fast_higher_classes: bool,
}

impl Default for Manager {
    fn default() -> Self {
        Self {
            enable: false,
            remove_unsafe_retires: false,
            removal_distance_crow: 300.,
            removal_distance_track: 300.,
            lapping_yield_distance: 100.,
            team_mate_yield_distance: 20.,
            race_start_distance: [0., 0.],
            race_start_factor: [1., 1.],
            min_distance_offline: 0.,
            max_distance_offline: 0.,
            max_assist_inside: 0.,
            max_assist_outside: 0.,
            min_curve_offline: 0.,
            fast_higher_classes: true,
        }
    }
}

/// Conveyor belt (0x4014B0). `min_sleep_distance` has no parser string and
/// stays 0 [V].
#[derive(Debug, Clone, PartialEq)]
pub struct Conveyor {
    pub min_grid_size: u32,
    pub forwards_multiple: f32,
    pub backwards_multiple: f32,
    pub max_sleep_distance: f32,
    pub forwards_place: [u32; 2],
    pub backwards_place: [u32; 2],
}

/// Event payload: mistake (id in the 0xCF6F64 table, arm timeout),
/// blow_tyre, or blow_engine (radiator, engine damage).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventKind {
    Mistake { id: u8, timeout: f32 },
    BlowTyre,
    BlowEngine { radiator: f32, engine: f32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Event {
    pub name: String,
    pub min_speed: f32,
    pub tyre_smoke: bool,
    pub max_active_time: f32,
    pub kind: EventKind,
}

/// Trigger (0x4067A0) with its event indices and `position` entries.
#[derive(Debug, Clone, PartialEq)]
pub struct Trigger {
    pub name: String,
    pub priority: i32,
    pub min_distance: f32,
    pub max_distance: f32,
    pub min_race_distance: f32,
    pub min_event_separation: f32,
    pub min_time: f32,
    pub min_distance_ahead: f32,
    pub max_distance_ahead: f32,
    pub select_closest: bool,
    pub max_events: u32,
    pub chance: f32,
    pub affect_higher_classes: bool,
    pub use_time_limit: bool,
    pub events: Vec<usize>,
    pub positions: Vec<(i32, f32)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Choreographer {
    pub manager: Manager,
    /// `intro rev_length`, overwritten with 10 after parsing [V quirk].
    pub rev_length: f32,
    pub conveyor: Option<Conveyor>,
    pub events: Vec<Event>,
    /// Sorted by priority (stable).
    pub triggers: Vec<Trigger>,
}

/// Mistake ids (enum 0xCF6F64); the XML names carry a `mistake_` prefix.
pub const MISTAKE_NAMES: [&str; 7] = [
    "brake_heavy",
    "brake_light",
    "brake_late",
    "brake_early",
    "brake_spin",
    "powerout_spin",
    "brake_lock",
];

/// The `ai_choreographer.xml` set naming `discipline`, else the one naming
/// `default` (loader 0x409A60).
pub fn choreographer(doc: &Document<'_>, discipline: &str) -> Result<Choreographer> {
    if element(doc, 0)?.name != "ai_choreographer" {
        return fail(0, "unexpected choreographer root");
    }
    let sets = named(doc, 0, "set")?;
    let names = |s: usize| -> Result<Vec<&str>> {
        named(doc, s, "discipline")?
            .into_iter()
            .map(|d| attr(&doc.elements[d], "name"))
            .collect()
    };
    let mut chosen = None;
    for wanted in [discipline, "default"] {
        for &s in &sets {
            if chosen.is_none() && names(s)?.contains(&wanted) {
                chosen = Some(s);
            }
        }
    }
    let set = chosen.ok_or(crate::Error {
        offset: 0,
        message: "choreographer set absent",
    })?;
    let f = |e: &Element<'_>, name: &str, default: f32| -> Result<f32> {
        optional(e, name).map_or(Ok(default), number)
    };
    let b = |e: &Element<'_>, name: &str, default: bool| -> Result<bool> {
        optional(e, name).map_or(Ok(default), flag)
    };
    let u = |e: &Element<'_>, name: &str, default: u32| -> Result<u32> {
        optional(e, name).map_or(Ok(default), whole)
    };
    let d = Manager::default();
    let manager = match named(doc, set, "manager")?[..] {
        [m] => {
            let e = &doc.elements[m];
            Manager {
                enable: b(e, "enable", d.enable)?,
                remove_unsafe_retires: b(e, "remove_unsafe_retires", d.remove_unsafe_retires)?,
                removal_distance_crow: f(e, "removal_distance_crow", d.removal_distance_crow)?,
                removal_distance_track: f(e, "removal_distance_track", d.removal_distance_track)?,
                lapping_yield_distance: f(e, "lapping_yield_distance", d.lapping_yield_distance)?,
                team_mate_yield_distance: f(
                    e,
                    "team_mate_yield_distance",
                    d.team_mate_yield_distance,
                )?,
                race_start_distance: [
                    f(e, "race_start_distance_1", 0.)?,
                    f(e, "race_start_distance_2", 0.)?,
                ],
                race_start_factor: [
                    f(e, "race_start_factor_1", 1.)?,
                    f(e, "race_start_factor_2", 1.)?,
                ],
                min_distance_offline: f(e, "min_distance_offline", 0.)?,
                max_distance_offline: f(e, "max_distance_offline", 0.)?,
                max_assist_inside: f(e, "max_assist_inside", 0.)?,
                max_assist_outside: f(e, "max_assist_outside", 0.)?,
                min_curve_offline: f(e, "min_curve_offline", 0.)?,
                fast_higher_classes: b(e, "fast_higher_classes", true)?,
            }
        }
        _ => d,
    };
    let conveyor = match named(doc, set, "conveyer_belt")?[..] {
        [c] => {
            let e = &doc.elements[c];
            Some(Conveyor {
                min_grid_size: u(e, "min_grid_size", 20)?,
                forwards_multiple: f(e, "forwards_multiple", 1.)?,
                backwards_multiple: f(e, "backwards_multiple", 1.)?,
                max_sleep_distance: f(e, "max_sleep_distance", 0.)?,
                forwards_place: [
                    u(e, "min_forwards_place", 0)?,
                    u(e, "max_forwards_place", 0)?,
                ],
                backwards_place: [
                    u(e, "min_backwards_place", 0)?,
                    u(e, "max_backwards_place", 0)?,
                ],
            })
        }
        _ => None,
    };
    let mut events = Vec::new();
    for ev in named(doc, set, "event")? {
        let e = &doc.elements[ev];
        let (mut min_speed, mut tyre_smoke) = (0., false);
        if let [c] = named(doc, ev, "common")?[..] {
            let c = &doc.elements[c];
            min_speed = f(c, "min_speed", 0.)?;
            tyre_smoke = b(c, "tyre_smoke", false)?;
        }
        let kind = match attr(e, "type")? {
            "mistake" => {
                let m = &doc.elements[only(doc, ev, "mistake")?];
                let name = attr(m, "name")?;
                let id = MISTAKE_NAMES
                    .iter()
                    .position(|&n| name.strip_prefix("mistake_") == Some(n))
                    .ok_or(crate::Error {
                        offset: ev,
                        message: "unknown choreographer mistake",
                    })?;
                EventKind::Mistake {
                    id: id as u8,
                    timeout: f(m, "timeout", 60.)?,
                }
            }
            "blow_tyre" => EventKind::BlowTyre,
            "blow_engine" => {
                let (mut radiator, mut engine) = (0., 0.);
                if let [c] = named(doc, ev, "blow_engine")?[..] {
                    let c = &doc.elements[c];
                    radiator = f(c, "radiator_damage", 0.)?;
                    engine = f(c, "engine_damage", 0.)?;
                }
                EventKind::BlowEngine { radiator, engine }
            }
            _ => return fail(ev, "unknown choreographer event type"),
        };
        events.push(Event {
            name: attr(e, "name")?.to_string(),
            min_speed,
            tyre_smoke,
            // Not given in shipped data: built-in 30 s (0xCAFDB8).
            max_active_time: 30.,
            kind,
        });
    }
    let mut triggers = Vec::new();
    for t in named(doc, set, "trigger")? {
        let e = &doc.elements[t];
        let mut trigger_events = Vec::new();
        for ev in named(doc, t, "event")? {
            let name = attr(&doc.elements[ev], "name")?;
            trigger_events.push(events.iter().position(|x| x.name == name).ok_or(
                crate::Error {
                    offset: ev,
                    message: "trigger names an unknown event",
                },
            )?);
        }
        let mut positions = Vec::new();
        for p in named(doc, t, "position")? {
            let p = &doc.elements[p];
            positions.push((whole(attr(p, "index")?)?, f(p, "chance", 1.)?));
        }
        triggers.push(Trigger {
            name: attr(e, "name")?.to_string(),
            priority: optional(e, "priority").map_or(Ok(0), whole)?,
            min_distance: f(e, "min_distance", 0.)?,
            max_distance: f(e, "max_distance", 0.)?,
            min_race_distance: f(e, "min_race_distance", 0.)?,
            min_event_separation: f(e, "min_event_separation", 0.)?,
            min_time: f(e, "min_time", 0.)?,
            min_distance_ahead: f(e, "min_distance_ahead", 1.)?,
            max_distance_ahead: f(e, "max_distance_ahead", 100.)?,
            select_closest: b(e, "select_closest", false)?,
            max_events: u(e, "max_events", 1)?,
            chance: f(e, "chance", 1.)?,
            affect_higher_classes: b(e, "affect_higher_classes", false)?,
            use_time_limit: b(e, "use_time_limit", false)?,
            events: trigger_events,
            positions,
        });
    }
    triggers.sort_by_key(|t| t.priority);
    Ok(Choreographer {
        manager,
        rev_length: 10.,
        conveyor,
        events,
        triggers,
    })
}
