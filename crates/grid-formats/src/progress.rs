//! Typed circuit and point-to-point progress data. Crossing and lap behavior
//! are not inferred.
use crate::{
    Result,
    binxml::{Document, Element},
    fail,
};

#[derive(Debug)]
pub struct Gate {
    pub distance: f32,
    pub left: [f32; 3],
    pub right: [f32; 3],
}
#[derive(Debug)]
pub struct Point {
    pub distance: f32,
    pub position: [f32; 3],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitKind {
    Finish,
    Time,
    /// The start of a point-to-point run (Haruna, the Yokohama arenas) and
    /// of Yokohama route 3's circuit.
    Start,
    /// Milan route 3's speed trap.
    Speed,
}
/// `track type`: a lap circuit or an open road run from a start to a finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackKind {
    Circuit,
    PointToPoint,
}
#[derive(Debug)]
pub struct Split {
    pub kind: SplitKind,
    pub gate: usize,
}
#[derive(Debug)]
pub struct Route {
    pub splits: Vec<Split>,
    /// `direction="reverse"`: the run goes down the gate order (Haruna's
    /// second route). Circuits are always forwards.
    pub reverse: bool,
}
#[derive(Debug)]
pub struct Progress {
    pub kind: TrackKind,
    pub total_distance: f32,
    pub gates: Vec<Gate>,
    pub points: Vec<Point>,
    pub routes: Vec<Route>,
}

fn attr<'a>(e: &Element<'a>, name: &str) -> Result<&'a str> {
    e.attributes
        .iter()
        .find(|(key, _)| *key == name)
        .map(|(_, value)| *value)
        .ok_or(crate::Error {
            offset: 0,
            message: "required progress attribute absent",
        })
}
fn integer(e: &Element<'_>, name: &str) -> Result<usize> {
    attr(e, name)?.parse().map_err(|_| crate::Error {
        offset: 0,
        message: "invalid progress integer",
    })
}
fn number(value: &str) -> Result<f32> {
    let v: f32 = value.parse().map_err(|_| crate::Error {
        offset: 0,
        message: "invalid progress float",
    })?;
    if !v.is_finite() {
        return fail(0, "nonfinite progress float");
    }
    Ok(v)
}
fn children<'a>(doc: &'a Document<'a>, index: usize, name: &str) -> Result<Vec<usize>> {
    let e = doc.elements.get(index).ok_or(crate::Error {
        offset: index,
        message: "progress element absent",
    })?;
    if e.name != name {
        return fail(index, "unexpected progress element");
    }
    Ok(e.children.clone().collect())
}
fn vector(doc: &Document<'_>, index: usize, name: &str) -> Result<[f32; 3]> {
    if !children(doc, index, name)?.is_empty() || attr(&doc.elements[index], "format")? != "float3"
    {
        return fail(index, "expected progress float3 leaf");
    }
    let text = doc.elements[index].value.ok_or(crate::Error {
        offset: index,
        message: "progress vector absent",
    })?;
    let values: Vec<_> = text.split_whitespace().map(number).collect::<Result<_>>()?;
    values.try_into().map_err(|_| crate::Error {
        offset: index,
        message: "expected three progress coordinates",
    })
}
fn records(
    doc: &Document<'_>,
    index: usize,
    container: &str,
    count: &str,
    record: &str,
) -> Result<Vec<usize>> {
    let items = children(doc, index, container)?;
    if items.len() != integer(&doc.elements[index], count)? || items.is_empty() {
        return fail(index, "progress record count mismatch");
    }
    for (ordinal, &item) in items.iter().enumerate() {
        if doc.elements[item].name != record || integer(&doc.elements[item], "id")? != ordinal {
            return fail(item, "progress IDs must follow declared order");
        }
    }
    Ok(items)
}
pub fn read(doc: &Document<'_>) -> Result<Progress> {
    let root = children(doc, 0, "progress_track_data")?;
    if attr(&doc.elements[0], "exporter_version")? != "2.0.0" {
        return fail(0, "unsupported progress exporter version");
    }
    if root.len() != 4 {
        return fail(0, "unsupported progress root structure");
    }
    let mut selected = [None; 4];
    for index in root {
        let slot = match doc.elements[index].name {
            "track" => 0,
            "routes" => 1,
            "gates" => 2,
            "points" => 3,
            _ => return fail(index, "unknown progress section"),
        };
        if selected[slot].replace(index).is_some() {
            return fail(index, "duplicate progress section");
        }
    }
    let [track, routes, gates, points] = selected.map(Option::unwrap);
    if !children(doc, track, "track")?.is_empty() {
        return fail(track, "progress track must be leaf");
    }
    let kind = match attr(&doc.elements[track], "type")? {
        "circuit" => TrackKind::Circuit,
        "point_to_point" => TrackKind::PointToPoint,
        _ => return fail(track, "unknown progress track type"),
    };
    let total_distance = number(attr(&doc.elements[track], "total_distance")?)?;
    if total_distance <= 0.0 {
        return fail(track, "progress distance must be positive");
    }
    let mut out = Progress {
        kind,
        total_distance,
        gates: Vec::new(),
        points: Vec::new(),
        routes: Vec::new(),
    };
    for index in records(doc, gates, "gates", "num_gates", "gate")? {
        let parts = children(doc, index, "gate")?;
        if parts.len() != 2 {
            return fail(index, "progress gate endpoint count");
        }
        let distance = number(attr(&doc.elements[index], "distance")?)?;
        let gate = Gate {
            distance,
            left: vector(doc, parts[0], "left")?,
            right: vector(doc, parts[1], "right")?,
        };
        if gate.left == gate.right {
            return fail(index, "degenerate progress gate");
        }
        out.gates.push(gate);
    }
    for index in records(doc, points, "points", "num_points", "point")? {
        let parts = children(doc, index, "point")?;
        if parts.len() != 1 {
            return fail(index, "progress point position count");
        }
        out.points.push(Point {
            distance: number(attr(&doc.elements[index], "distance")?)?,
            position: vector(doc, parts[0], "position")?,
        });
    }
    for distances in [
        out.gates.iter().map(|g| g.distance).collect::<Vec<_>>(),
        out.points.iter().map(|p| p.distance).collect(),
    ] {
        // An open road's last point lies at its end (Haruna: 8999.339 of
        // 8999.339 m); a circuit's distances stay below its length.
        let beyond = |d: f32| match kind {
            TrackKind::Circuit => d >= total_distance,
            TrackKind::PointToPoint => d > total_distance,
        };
        if distances.iter().any(|&d| d < 0.0 || beyond(d))
            || distances.windows(2).any(|w| w[1] <= w[0])
        {
            return fail(0, "progress distances must increase within circuit length");
        }
    }
    for index in records(doc, routes, "routes", "num_routes", "route")? {
        let reverse = match attr(&doc.elements[index], "direction")? {
            "forwards" => false,
            "reverse" if kind == TrackKind::PointToPoint => true,
            _ => return fail(index, "unsupported progress route direction"),
        };
        let mut route = Route {
            splits: Vec::new(),
            reverse,
        };
        for split in records(doc, index, "route", "num_splits", "split")? {
            if !children(doc, split, "split")?.is_empty() {
                return fail(split, "progress split must be leaf");
            }
            let kind = match attr(&doc.elements[split], "type")? {
                "finish" => SplitKind::Finish,
                "time" => SplitKind::Time,
                "start" => SplitKind::Start,
                "speed" => SplitKind::Speed,
                _ => return fail(split, "unknown progress split kind"),
            };
            let gate = integer(&doc.elements[split], "gate")?;
            if gate >= out.gates.len() {
                return fail(split, "progress split gate out of bounds");
            }
            route.splits.push(Split { kind, gate });
        }
        out.routes.push(route);
    }
    Ok(out)
}
