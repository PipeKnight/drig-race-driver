//! Source CrowdState messages for in-zone race flags.
use crate::race_flags::{CHEQUERED_FLAG, GREEN_FLAG, RaceFlags, YELLOW_FLAG};
use grid_formats::{
    Error, Result,
    binxml::{Document, Element},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Role {
    Start = 1,
    Danger = 2,
    Finish = 3,
}

impl Role {
    /// Broadcast iteration order at 0x64BA60, independent of code order.
    const SOURCE_ORDER: [Self; 3] = [Self::Start, Self::Finish, Self::Danger];

    pub const fn action_name(self) -> &'static str {
        match self {
            Self::Start => "startFlag",
            Self::Danger => "dangerFlag",
            Self::Finish => "finishFlag",
        }
    }

    const fn flag_name(self) -> &'static str {
        match self {
            Self::Start => GREEN_FLAG,
            Self::Danger => YELLOW_FLAG,
            Self::Finish => CHEQUERED_FLAG,
        }
    }

    const fn bit(self) -> u8 {
        1 << (self as u8 - 1)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cue {
    Reset,
    Begin(Role),
    End(Role),
}

impl Cue {
    pub const fn native_code(self) -> u32 {
        match self {
            Self::Reset => 0,
            Self::Begin(role) => role as u32,
            Self::End(role) => role as u32 + 3,
        }
    }
}

/// The three flag predicates for one simulation sample at the viewed car's arc.
/// Sample at the simulation tick; replay needs the historical cues.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InZone(u8);

impl InZone {
    pub fn sample(flags: &RaceFlags, position: f64) -> Self {
        if !position.is_finite() {
            return Self::default();
        }
        let mut active = 0;
        for role in Role::SOURCE_ORDER {
            if flags.showing(role.flag_name(), position) {
                active |= role.bit();
            }
        }
        Self(active)
    }

    pub fn edges(self, next: Self) -> impl Iterator<Item = Cue> {
        Role::SOURCE_ORDER.into_iter().filter_map(move |role| {
            let mask = role.bit();
            match (self.0 & mask != 0, next.0 & mask != 0) {
                (false, true) => Some(Cue::Begin(role)),
                (true, false) => Some(Cue::End(role)),
                _ => None,
            }
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Action<'a> {
    pub role: Role,
    pub start_state: &'a str,
    pub mid_state: &'a str,
    pub end_state: &'a str,
}

fn attr<'a>(node: &Element<'a>, key: &str) -> Option<&'a str> {
    let mut matches = node.attributes.iter().filter(|(name, _)| *name == key);
    let (_, value) = matches.next()?;
    (matches.next().is_none() && !value.is_empty() && value.len() <= 96 && value.is_ascii())
        .then_some(*value)
}

fn action<'a>(children: &[Element<'a>], role: Role) -> Result<Action<'a>> {
    let mut matching = children.iter().filter(|node| {
        node.name == "animAction"
            && attr(node, "name").is_some_and(|name| name.eq_ignore_ascii_case(role.action_name()))
    });
    let error = Error {
        offset: 0,
        message: "missing or ambiguous crowd flag action",
    };
    let node = matching.next().ok_or_else(|| error.clone())?;
    if matching.next().is_some() || !node.children.is_empty() {
        return Err(error);
    }
    Ok(Action {
        role,
        start_state: attr(node, "startState").ok_or_else(|| error.clone())?,
        mid_state: attr(node, "midState").ok_or_else(|| error.clone())?,
        end_state: attr(node, "endState").ok_or(error)?,
    })
}

/// Resolve all three original role actions in broadcaster order.
/// Performance labels are retained for the existing HLA selector.
pub fn actions<'a>(defs: &Document<'a>) -> Result<[Action<'a>; 3]> {
    let children = defs
        .elements
        .first()
        .filter(|root| root.name == "crowdDefs")
        .and_then(|root| defs.elements.get(root.children.clone()))
        .ok_or(Error {
            offset: 0,
            message: "invalid crowdDefs root",
        })?;
    Ok([
        action(children, Role::Start)?,
        action(children, Role::Finish)?,
        action(children, Role::Danger)?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::race_flags::{CarSample, RaceState, RuleSet};

    #[test]
    fn simultaneous_rules_emit_zone_edges_in_original_order() {
        let mut flags = RaceFlags::on_line(RuleSet::single_race(), 3000., false, 1);
        let start = InZone::sample(&flags, 0.);
        let car = CarSample {
            position: 10.,
            angle: 0.,
            speed: 0.,
            racing: true,
        };
        let race = RaceState {
            started: true,
            ended: false,
            cars: &[car],
            leader_to_line: Some(10.),
            line: 0.,
            chequered: true,
        };
        assert_eq!(flags.update(11., &race).len(), 3);
        let all = InZone::sample(&flags, 0.);
        let without_yellow = InZone::sample(&flags, 50.);
        let outside = InZone::sample(&flags, 101.);
        let codes = |from: InZone, to| from.edges(to).map(Cue::native_code).collect::<Vec<_>>();
        assert_eq!(codes(start, all), [1, 3, 2]);
        assert_eq!(codes(all, without_yellow), [5]);
        assert_eq!(codes(all, outside), [4, 6, 5]);
        assert_eq!(codes(outside, all), [1, 3, 2]);
        assert!(codes(all, all).is_empty());
        assert_eq!(Cue::Reset.native_code(), 0);
    }
}
