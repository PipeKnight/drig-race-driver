//! Shipped `ActionMap/actionMapPreset*.xml` input presets (plain XML):
//! `<ActionMap deviceType schemeType priority>` with `<Action actionName>`
//! children, each holding one or more `<Axis axisName deviceName
//! baseCalibration [deadZone] [saturation] [threshold]>` bindings. Buttons are
//! axes with values 0/1. See docs/PLATFORMS_AND_INPUT_RESEARCH.md section 4.1.
use crate::{Error, Result, fail};
use quick_xml::{Reader, events::Event};

/// `baseCalibration` names (table 0xE52210, enum order 0..3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalibrationType {
    UniPositive,
    UniNegative,
    BiUpper,
    BiLower,
}

impl CalibrationType {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "uniDirectionalPositive" => Some(Self::UniPositive),
            "uniDirectionalNegative" => Some(Self::UniNegative),
            "biDirectionalUpper" => Some(Self::BiUpper),
            "biDirectionalLower" => Some(Self::BiLower),
            _ => None,
        }
    }
}

/// One binding. Absent numbers keep the binding constructor's values
/// (8400B0: dead zone 0.2, saturation 0.8, threshold 0.5).
#[derive(Clone, Debug, PartialEq)]
pub struct Axis {
    pub axis_name: String,
    pub device_name: String,
    pub calibration: CalibrationType,
    pub dead_zone: Option<f32>,
    pub saturation: Option<f32>,
    pub threshold: Option<f32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Action {
    pub name: String,
    pub axes: Vec<Axis>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActionMap {
    pub device_type: String,
    pub scheme_type: String,
    pub priority: i32,
    pub actions: Vec<Action>,
}

impl ActionMap {
    pub fn action(&self, name: &str) -> Option<&Action> {
        self.actions.iter().find(|a| a.name == name)
    }
}

const MAXIMUM_ACTIONS: usize = 512;
const MAXIMUM_AXES: usize = 32;

fn number(value: &str, at: usize) -> Result<f32> {
    match value.trim().parse::<f32>() {
        Ok(v) if v.is_finite() => Ok(v),
        _ => fail(at, "invalid action map number"),
    }
}

pub fn read(text: &str) -> Result<ActionMap> {
    if text.len() > 1 << 20 {
        return fail(0, "action map exceeds bound");
    }
    let mut reader = Reader::from_str(text);
    let mut map: Option<ActionMap> = None;
    let mut in_action = false;
    loop {
        let at = reader.buffer_position() as usize;
        let event = reader.read_event().map_err(|_| Error {
            offset: at,
            message: "malformed action map XML",
        })?;
        let (element, empty) = match &event {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::End(e) => {
                if e.name().as_ref() == b"Action" {
                    in_action = false;
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let mut attributes = Vec::new();
        for attribute in element.attributes() {
            let attribute = attribute.map_err(|_| Error {
                offset: at,
                message: "malformed action map attribute",
            })?;
            // Shipped values are plain names and numbers; entities are rejected.
            let value = std::str::from_utf8(&attribute.value)
                .ok()
                .filter(|v| !v.contains('&'))
                .ok_or(Error {
                    offset: at,
                    message: "malformed action map attribute value",
                })?;
            attributes.push((attribute.key.as_ref().to_vec(), value.to_owned()));
        }
        let get = |name: &str| {
            attributes
                .iter()
                .find(|(k, _)| k == name.as_bytes())
                .map(|(_, v)| v.as_str())
        };
        let required = |name: &'static str, message: &'static str| {
            get(name).map(str::to_owned).ok_or(Error {
                offset: at,
                message,
            })
        };
        match element.name().as_ref() {
            b"ActionMap" => {
                if map.is_some() {
                    return fail(at, "duplicate ActionMap root");
                }
                map = Some(ActionMap {
                    device_type: required("deviceType", "ActionMap without deviceType")?,
                    scheme_type: required("schemeType", "ActionMap without schemeType")?,
                    priority: get("priority")
                        .map_or(Ok(0), |p| p.trim().parse())
                        .map_err(|_| Error {
                            offset: at,
                            message: "invalid ActionMap priority",
                        })?,
                    actions: Vec::new(),
                });
            }
            b"Action" => {
                let Some(map) = map.as_mut() else {
                    return fail(at, "Action outside ActionMap");
                };
                if in_action {
                    return fail(at, "nested Action");
                }
                if map.actions.len() == MAXIMUM_ACTIONS {
                    return fail(at, "too many actions");
                }
                map.actions.push(Action {
                    name: required("actionName", "Action without actionName")?,
                    axes: Vec::new(),
                });
                in_action = !empty;
            }
            b"Axis" => {
                let action = match map.as_mut().and_then(|m| m.actions.last_mut()) {
                    Some(action) if in_action => action,
                    _ => return fail(at, "Axis outside Action"),
                };
                if action.axes.len() == MAXIMUM_AXES {
                    return fail(at, "too many axes in one action");
                }
                let calibration = get("baseCalibration")
                    .and_then(CalibrationType::from_name)
                    .ok_or(Error {
                        offset: at,
                        message: "unknown baseCalibration",
                    })?;
                let optional = |key: &str| get(key).map(|v| number(v, at)).transpose();
                action.axes.push(Axis {
                    axis_name: required("axisName", "Axis without axisName")?,
                    device_name: required("deviceName", "Axis without deviceName")?,
                    calibration,
                    dead_zone: optional("deadZone")?,
                    saturation: optional("saturation")?,
                    threshold: optional("threshold")?,
                });
            }
            _ => {}
        }
    }
    map.ok_or(Error {
        offset: 0,
        message: "missing ActionMap root",
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_bindings_with_spaced_attributes_and_defaults() {
        let map = read(
            r#"<!-- authored sample -->
<ActionMap deviceType="win_xinput" schemeType="xinputScheme" priority="6" >
    <Action actionName = "Steer Left" >
        <Axis axisName = "stickX" deviceName = "pad" baseCalibration = "biDirectionalLower" deadZone = "0.2" saturation = "1.0" />
        <Axis axisName = "dpadLeft" deviceName = "pad" baseCalibration = "uniDirectionalPositive" />
    </Action>
    <Action actionName="Brake"><Axis axisName="y" deviceName="wheel" baseCalibration="uniDirectionalNegative" threshold="0.25"/></Action>
</ActionMap>"#,
        )
        .unwrap();
        assert_eq!(
            (
                map.device_type.as_str(),
                map.scheme_type.as_str(),
                map.priority
            ),
            ("win_xinput", "xinputScheme", 6)
        );
        let steer = map.action("Steer Left").unwrap();
        assert_eq!(steer.axes.len(), 2);
        assert_eq!(steer.axes[0].calibration, CalibrationType::BiLower);
        assert_eq!(
            (steer.axes[0].dead_zone, steer.axes[0].saturation),
            (Some(0.2), Some(1.0))
        );
        assert_eq!(steer.axes[1].dead_zone, None);
        let brake = map.action("Brake").unwrap();
        assert_eq!(brake.axes[0].calibration, CalibrationType::UniNegative);
        assert_eq!(brake.axes[0].threshold, Some(0.25));
    }

    #[test]
    fn rejects_malformed_maps() {
        assert!(read("").is_err());
        assert!(read(r#"<Action actionName="A"/>"#).is_err());
        assert!(
            read(r#"<ActionMap deviceType="k" schemeType="s"><Axis axisName="a" deviceName="d" baseCalibration="uniDirectionalPositive"/></ActionMap>"#)
                .is_err()
        );
        assert!(
            read(r#"<ActionMap deviceType="k" schemeType="s"><Action actionName="A"><Axis axisName="a" deviceName="d" baseCalibration="linear"/></Action></ActionMap>"#)
                .is_err()
        );
        assert!(
            read(r#"<ActionMap deviceType="k" schemeType="s"><Action actionName="A"><Axis axisName="a" deviceName="d" baseCalibration="uniDirectionalPositive" deadZone="x"/></Action></ActionMap>"#)
                .is_err()
        );
    }
}
