//! Selected normal-car audio setup from GRID's existing binary XML reader.
//! These are source metadata; gain units and backend behavior are separate.
use crate::{Error, Result, binxml, fail};

#[derive(Debug)]
pub struct Setup<'a> {
    pub engine_ini: &'a str,
    pub gear_ini: &'a str,
    pub engine_bank: &'a str,
    pub mechanics_bank: &'a str,
    pub throttle_frames: u32,
    pub idle_frames: u32,
    pub trims: [f32; 3], // gear, engine, exhaust; native units not inferred here.
    pub offsets: [[f32; 3]; 4], // engine bay, exhaust, gearbox, cockpit.
    pub open_cockpit: bool,
    /// The car's `GearPreset` from `GearPresets`: per forward gear the
    /// whine's basePitch, maxPitch and mec sample (4265D0, 410C00).
    pub gears: Vec<Gear<'a>>,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Gear<'a> {
    pub base_pitch: f32,
    pub max_pitch: f32,
    pub sample: &'a str,
}
fn attribute<'a>(element: &binxml::Element<'a>, name: &str) -> Result<&'a str> {
    let mut values = element.attributes.iter().filter(|(key, _)| *key == name);
    let value = values
        .next()
        .ok_or(Error {
            offset: 0,
            message: "missing audio setup attribute",
        })?
        .1;
    if values.next().is_some() {
        return fail(0, "duplicate audio setup attribute");
    }
    Ok(value)
}
fn child<'d, 'a>(
    doc: &'d binxml::Document<'a>,
    parent: &binxml::Element<'a>,
    name: &str,
) -> Result<&'d binxml::Element<'a>> {
    let children = doc.elements.get(parent.children.clone()).ok_or(Error {
        offset: 0,
        message: "audio setup children outside document",
    })?;
    let mut values = children.iter().filter(|element| element.name == name);
    let value = values.next().ok_or(Error {
        offset: 0,
        message: "missing audio setup element",
    })?;
    if values.next().is_some() {
        return fail(0, "duplicate audio setup element");
    }
    Ok(value)
}
fn path(value: &str) -> Result<&str> {
    if !value.starts_with("audio/cars/")
        || !value.ends_with(".ini")
        || value.len() > 128
        || value.contains("..")
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"/_-.".contains(&b))
    {
        return fail(0, "unsupported audio setup resource path");
    }
    Ok(value)
}
fn bank(value: &str) -> Result<&str> {
    if value.is_empty()
        || value.len() > 32
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return fail(0, "unsupported audio setup bank name");
    }
    Ok(value)
}
fn frames(value: &str) -> Result<u32> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return fail(0, "invalid audio setup frame count");
    }
    let value = value.parse().map_err(|_| Error {
        offset: 0,
        message: "audio setup frame count overflow",
    })?;
    if !(1..=10_000_000).contains(&value) {
        return fail(0, "audio setup frame count bound");
    }
    Ok(value)
}
fn number(value: &str) -> Result<f32> {
    let value: f32 = value.parse().map_err(|_| Error {
        offset: 0,
        message: "invalid audio setup scalar",
    })?;
    if !value.is_finite() || value.abs() > 1000. {
        return fail(0, "audio setup scalar bound");
    }
    Ok(value)
}
fn gears<'a>(doc: &binxml::Document<'a>, root: &binxml::Element<'a>, name: &str) -> Result<Vec<Gear<'a>>> {
    let presets = child(doc, root, "GearPresets")?;
    let children = doc.elements.get(presets.children.clone()).ok_or(Error {
        offset: 0,
        message: "audio setup children outside document",
    })?;
    let mut matching = children
        .iter()
        .filter(|element| element.name == "Gears" && element.attributes.contains(&("name", name)));
    let preset = matching.next().ok_or(Error {
        offset: 0,
        message: "selected gear preset absent",
    })?;
    if matching.next().is_some() {
        return fail(0, "duplicate gear preset");
    }
    let entries = doc.elements.get(preset.children.clone()).ok_or(Error {
        offset: 0,
        message: "audio setup children outside document",
    })?;
    if entries.is_empty() || entries.len() > 9 {
        return fail(0, "gear preset count bound");
    }
    let mut out = Vec::new();
    for (k, entry) in entries.iter().enumerate() {
        if entry.name != format!("Gear{}", k + 1) {
            return fail(0, "gear preset entries out of order");
        }
        out.push(Gear {
            base_pitch: number(attribute(entry, "basePitch")?)?,
            max_pitch: number(attribute(entry, "maxPitch")?)?,
            sample: bank(attribute(entry, "sample")?)?,
        });
    }
    Ok(out)
}
fn select<'a>(doc: &binxml::Document<'a>, name: &str) -> Result<Setup<'a>> {
    let root = doc.elements.first().ok_or(Error {
        offset: 0,
        message: "empty audio setup document",
    })?;
    if root.name != "CarSetup" {
        return fail(0, "expected CarSetup document");
    }
    let container = child(doc, root, "Cars")?;
    let children = doc.elements.get(container.children.clone()).ok_or(Error {
        offset: 0,
        message: "audio setup children outside document",
    })?;
    let mut cars = children
        .iter()
        .filter(|element| element.name == "Car" && element.attributes.contains(&("name", name)));
    let car = cars.next().ok_or(Error {
        offset: 0,
        message: "selected audio car absent",
    })?;
    if cars.next().is_some() {
        return fail(0, "duplicate selected audio car");
    }
    if attribute(car, "type")? != "normal" {
        return fail(0, "unsupported audio car type");
    }
    let open_cockpit = match attribute(car, "openCockpit")? {
        "true" => true,
        "false" => false,
        _ => return fail(0, "invalid audio cockpit flag"),
    };
    let engine = child(doc, car, "EngineBank")?;
    let trims = child(doc, car, "VolumeTrims")?;
    let mut trim_values = [0.; 3];
    for (value, key) in trim_values.iter_mut().zip(["gear", "engine", "exhaust"]) {
        *value = number(attribute(trims, key)?)?;
    }
    let offsets = child(doc, car, "Offsets")?;
    let mut offset_values = [[0.; 3]; 4];
    for (value, key) in offset_values
        .iter_mut()
        .zip(["EngineBay", "Exhaust", "GearBox", "Cockpit"])
    {
        let point = child(doc, offsets, key)?;
        for (coordinate, axis) in value.iter_mut().zip(["x", "y", "z"]) {
            *coordinate = number(attribute(point, axis)?)?;
        }
    }
    Ok(Setup {
        engine_ini: path(attribute(child(doc, car, "EngineIniFile")?, "value")?)?,
        gear_ini: path(attribute(child(doc, car, "GearIniFile")?, "value")?)?,
        engine_bank: bank(attribute(engine, "value")?)?,
        mechanics_bank: bank(attribute(child(doc, car, "MechBank")?, "value")?)?,
        throttle_frames: frames(attribute(engine, "throttleLength")?)?,
        idle_frames: frames(attribute(engine, "idleLength")?)?,
        trims: trim_values,
        offsets: offset_values,
        open_cockpit,
        gears: gears(doc, root, attribute(child(doc, car, "GearPreset")?, "value")?)?,
    })
}
pub fn read<'a>(bytes: &'a [u8], name: &str) -> Result<Setup<'a>> {
    if bytes.len() > 4 * 1024 * 1024 {
        return fail(0, "audio car setup exceeds bound");
    }
    select(&binxml::read(bytes)?, name)
}
