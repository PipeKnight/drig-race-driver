//! Track time-of-day keyframes (`tracks/<region>/<track>/daynight.xml`, shipped
//! only for Le Mans): `daynight` > `timepoint clock1="HHMM" name="value" ...`.
//! Each timepoint sets only some parameters, so every parameter is its own
//! sparse track of (minute of the day, value) keys in file order. How the game
//! interpolates and applies them is separate (docs/LE_MANS.md §3).
use crate::{Result, binxml::Document, fail};
use std::collections::BTreeMap;

/// One parameter's keys: (minute of the day 0..1440, value), strictly
/// increasing in minute.
pub type Track = Vec<(u16, f32)>;

#[derive(Clone, Debug, PartialEq)]
pub struct DayNight {
    /// Number of `timepoint` elements.
    pub timepoints: usize,
    /// Parameter name → its keys.
    pub tracks: BTreeMap<String, Track>,
}

/// `HHMM` (00..23, 00..59) → minute of the day.
pub fn clock_minutes(clock: &str) -> Option<u16> {
    let b = clock.as_bytes();
    if b.len() != 4 || !b.iter().all(u8::is_ascii_digit) {
        return None;
    }
    let hours = u16::from(b[0] - b'0') * 10 + u16::from(b[1] - b'0');
    let minutes = u16::from(b[2] - b'0') * 10 + u16::from(b[3] - b'0');
    (hours < 24 && minutes < 60).then_some(hours * 60 + minutes)
}

pub fn read(doc: &Document<'_>) -> Result<DayNight> {
    match doc.elements.first() {
        Some(e) if e.name == "daynight" => {}
        _ => return fail(0, "expected daynight root"),
    }
    let mut tracks: BTreeMap<String, Track> = BTreeMap::new();
    let mut timepoints = 0;
    let mut last = None;
    for i in doc.elements[0].children.clone() {
        let Some(e) = doc.elements.get(i) else {
            return fail(i, "daynight child out of range");
        };
        if e.name != "timepoint" || !e.children.is_empty() {
            return fail(i, "expected flat timepoint elements");
        }
        let mut minute = None;
        let mut values = Vec::new();
        for &(key, value) in &e.attributes {
            if key == "clock1" {
                minute = clock_minutes(value);
                if minute.is_none() {
                    return fail(i, "invalid timepoint clock");
                }
            } else {
                match value.parse::<f32>() {
                    Ok(v) if v.is_finite() => values.push((key, v)),
                    _ => return fail(i, "invalid timepoint value"),
                }
            }
        }
        let Some(minute) = minute else {
            return fail(i, "timepoint without clock1");
        };
        if last.is_some_and(|l| minute <= l) {
            return fail(i, "timepoints out of order");
        }
        last = Some(minute);
        if values.is_empty() {
            return fail(i, "timepoint without parameters");
        }
        for (key, v) in values {
            tracks.entry(key.into()).or_default().push((minute, v));
        }
        timepoints += 1;
    }
    if timepoints == 0 || timepoints > 4096 {
        return fail(0, "daynight timepoint count out of range");
    }
    Ok(DayNight { timepoints, tracks })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clocks_are_hours_and_minutes() {
        assert_eq!(clock_minutes("0000"), Some(0));
        assert_eq!(clock_minutes("0035"), Some(35));
        assert_eq!(clock_minutes("2336"), Some(23 * 60 + 36));
        assert_eq!(clock_minutes("2400"), None);
        assert_eq!(clock_minutes("0960"), None);
        assert_eq!(clock_minutes("935"), None);
        assert_eq!(clock_minutes("09:3"), None);
    }
}
