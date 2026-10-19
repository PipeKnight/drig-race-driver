//! Career result rules of the original: championship standings over an
//! event's races, prize money, reputation, sponsor objectives, teammate
//! share, team win bonus and driver-offer pay (docs/CAREER_RESEARCH.md §4–§7).
//!
//! Positions are 0-based (0 = winner) as in the original. Where a rule is an
//! interpretation or a choice of this port the comment says so.
use super::data::{Catalogue, DriverOffer, Event, OfferObjective, SponsorObjective, SponsorOffer};

/// `trunc(prize_money_for_first × money_fractions[pos])`: the product in
/// f64, truncated (0x6395E0 with `_ftol2`) [V]. Places past the list pay 0
/// (0x826700 returns 0.0 out of range) [V].
pub fn prize(event: &Event, pos: usize) -> i64 {
    let fraction = event.money_fractions.get(pos).copied().unwrap_or(0.0);
    (event.prize_money_for_first as f64 * fraction) as i64
}

/// `trunc(pot × reputation_fractions[pos])` [V 0x6779C0, 0x678435]; places
/// 16 and lower get 0.
pub fn reputation_base(fractions: &[f64], pot: i64, pos: usize) -> i64 {
    let fraction = fractions.get(pos).copied().unwrap_or(0.0);
    (pot as f64 * fraction) as i64
}

/// The race settings that earn extra reputation (0x64A3F0) [V].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RaceSettings {
    /// Difficulty star level 0..5 (`difficulty.star_level`).
    pub star_level: i32,
    /// Locked to the head camera (+`fixHeadCamera`).
    pub head_camera_only: bool,
    /// Pro mode: no restarts (+`proMode`).
    pub pro_mode: bool,
    /// Manual gears (+`manualGears`) [I: the parser's `transmission` flag].
    pub manual_gears: bool,
    pub traction_control: bool,
    pub steering_assist: bool,
    pub braking_assist: bool,
    /// Realistic damage. Loaded from `rewards.xml` but never read by the
    /// bonus code, so it earns nothing [V], despite the menu help text.
    pub realistic_damage: bool,
}

/// The reputation bonus factor F (career +0x3E74) [V 0x64A3F0]:
/// `0.01 × (assist points + floor(rep_bonus_factor × 100 + 0.5))`, in f32.
/// `surfaceAssist` and `realisticDamage` are never read [V].
pub fn bonus_factor(catalogue: &Catalogue, rank_id: i32, settings: &RaceSettings) -> f32 {
    let r = &catalogue.assist_rewards;
    let mut points = 0;
    if settings.head_camera_only {
        points += r.fix_head_camera;
    }
    if settings.pro_mode {
        points += r.pro_mode;
    }
    if !settings.traction_control {
        points += r.traction_control;
    }
    if settings.manual_gears {
        points += r.manual_gears;
    }
    if !settings.steering_assist {
        points += r.steering_assist;
    }
    if !settings.braking_assist {
        points += r.braking_assist;
    }
    let difficulty = catalogue
        .difficulty(rank_id, settings.star_level)
        .map(|d| (d.rep_bonus_factor * 100.0 + 0.5).floor() as i32)
        .unwrap_or(0);
    (points + difficulty) as f32 * 0.01
}

/// `floor(base × F + 0.5)` in f32 (0x6786FB–0x67875F) [V].
pub fn reputation_bonus(base: i64, factor: f32) -> i64 {
    (base as f32 * factor + 0.5).floor() as i64
}

/// Who a classified car belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Entrant {
    Player,
    TeamMate,
    /// Another driver, with its `team.id` (0 when unknown).
    Ai { driver: i32, team: i32 },
}

/// One car in one race's classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Classified {
    pub entrant: Entrant,
    /// Took the chequered flag (status finished, docs/RACE_RULES_RESEARCH.md §1).
    pub finished: bool,
    /// The race result's no-damage flag (+0x10) was clear. What sets it was
    /// not traced [open]; the port's choice is in the caller.
    pub undamaged: bool,
}

/// One race of an event: the classification in finishing order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RaceResult {
    pub order: Vec<Classified>,
    /// The race's flashback manager at the finish (none: not reported, no
    /// bonus).
    pub flashbacks: Option<Flashbacks>,
}

/// A race's flashbacks: the manager's maximum and how many were used.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Flashbacks {
    pub used: u32,
    pub maximum: u32,
}

/// The Flashback Bonus of one race [V]: the per-race recorder 0x66DB80
/// stores 0x6A22A0 of the race's manager in the race record (+0x5C) when the
/// player's result is final (+0x11, [I] finished); no position condition.
/// `max_award` is the `difficulty` row's `flashback_bonus` for the tier's
/// rank and the star level (0x6AF7C0).
pub fn flashback_bonus(catalogue: &Catalogue, rank_id: i32, star_level: i32, race: &RaceResult) -> i64 {
    let (Some(fb), Some((_, player))) = (race.flashbacks, race.entry(Entrant::Player)) else {
        return 0;
    };
    if !player.finished {
        return 0;
    }
    let max_award = catalogue
        .difficulty(rank_id, star_level)
        .map_or(0, |d| d.flashback_bonus);
    crate::flashback::Manager {
        maximum: fb.maximum,
        used: fb.used.min(fb.maximum),
        max_award: max_award.clamp(0, u32::MAX as i64) as u32,
    }
    .award()
}

impl RaceResult {
    pub fn position(&self, who: Entrant) -> Option<usize> {
        self.order.iter().position(|c| c.entrant == who)
    }

    fn entry(&self, who: Entrant) -> Option<(usize, &Classified)> {
        self.order.iter().enumerate().find(|(_, c)| c.entrant == who)
    }
}

/// One line of the event standings (0x66FF70) [V].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Standing {
    pub entrant: Entrant,
    pub points: i32,
    pub wins: i32,
    pub races: i32,
    /// Dense rank: equal points share a rank (0x6478E0) [V].
    pub rank: usize,
}

/// The event standings over the races run so far: per race
/// `points += points_table[race position]` (0 past the table), sorted by
/// points. Ties share a dense rank [V]. AUTHORED: equal points keep the
/// order of first appearance (the original's sort tie order is not traced).
pub fn standings(points_table: &[i32], races: &[RaceResult]) -> Vec<Standing> {
    let mut table: Vec<Standing> = Vec::new();
    for race in races {
        for (pos, c) in race.order.iter().enumerate() {
            let line = match table.iter_mut().find(|s| s.entrant == c.entrant) {
                Some(s) => s,
                None => {
                    table.push(Standing {
                        entrant: c.entrant,
                        points: 0,
                        wins: 0,
                        races: 0,
                        rank: 0,
                    });
                    table.last_mut().unwrap()
                }
            };
            line.points += points_table.get(pos).copied().unwrap_or(0);
            line.races += 1;
            line.wins += (pos == 0) as i32;
        }
    }
    table.sort_by_key(|s| std::cmp::Reverse(s.points));
    let mut rank = 0;
    for i in 0..table.len() {
        if i > 0 && table[i].points != table[i - 1].points {
            rank += 1;
        }
        table[i].rank = rank;
    }
    table
}

/// A sponsor objective met by one result (0x655960/0x655A10/0x655AC0/
/// 0x655B60) [V]: `objective_details` is the lowest 1-based place allowed.
pub fn sponsor_objective_met(offer: &SponsorOffer, pos: usize, result: &Classified) -> bool {
    if !result.finished {
        return false;
    }
    let place_ok = (pos as i32) < offer.objective_details;
    match offer.objective {
        SponsorObjective::FinishingPlace => place_ok,
        SponsorObjective::FinishingPlaceNoDamage => place_ok && result.undamaged,
        SponsorObjective::Finish => true,
        SponsorObjective::FinishNoDamage => result.undamaged,
    }
}

/// The offers an event result unlocks (0x673B20 / predicate 0x67F000) [V]:
/// every offer of the event with `pos < position_to_unlock`, so a win
/// unlocks all three, second place two, third place one.
pub fn unlocked_offers(event: &Event, pos: usize) -> impl Iterator<Item = &SponsorOffer> {
    event
        .sponsor_offers
        .iter()
        .filter(move |o| (pos as i32) < o.position_to_unlock)
}

/// A sponsor's pay for one race (0x652010, 0x642570) [V]: `money_reward ×
/// (2 in the major slot) × (player met + teammate met)`.
pub fn sponsor_race_pay(offer: &SponsorOffer, major: bool, player_met: bool, teammate_met: bool) -> i64 {
    offer.money_reward * if major { 2 } else { 1 } * (player_met as i64 + teammate_met as i64)
}

/// The team's share of the teammate's prize [V 0x677D87–0x677E85]:
/// `trunc(prize × (1 − winnings_percentage × 0.01))` in f64.
pub fn teammate_share(prize: i64, winnings_percentage: i32) -> i64 {
    (prize as f64 * (1.0 - winnings_percentage as f64 * 0.01)) as i64
}

/// Whether the player's team leads the team standings of the event
/// (0x660E90 returns 0). [I] The summed value is the event points of each
/// team's cars; the player and the teammate are one team, AI cars count by
/// `team.id` (an AI with team 0 is its own team).
pub fn player_team_first(standings: &[Standing]) -> bool {
    use std::collections::BTreeMap;
    let mut ours = 0;
    let mut teams: BTreeMap<(i32, i32), i32> = BTreeMap::new();
    for s in standings {
        match s.entrant {
            Entrant::Player | Entrant::TeamMate => ours += s.points,
            Entrant::Ai { driver, team } => {
                let key = if team == 0 { (0, driver) } else { (team, 0) };
                *teams.entry(key).or_default() += s.points;
            }
        }
    }
    teams.values().all(|&p| p < ours)
}

/// A driver-offer objective met in one race (0x62CB50, 0x62CBD0, 0x6556B0,
/// 0x655C80, 0x655D20) [V].
pub fn offer_objective_met(
    offer: &DriverOffer,
    race: &RaceResult,
    star_level: i32,
    best_lap_secs: Option<f32>,
    top_speed_mph: Option<f32>,
) -> bool {
    let target = |list: &[f32]| list.get(star_level.clamp(0, 5) as usize).copied();
    match offer.objective {
        OfferObjective::TimeTrial => matches!(
            (best_lap_secs, target(&offer.target_times)),
            (Some(lap), Some(t)) if lap < t
        ),
        OfferObjective::SpeedTest => matches!(
            (top_speed_mph, target(&offer.target_speeds_mph)),
            (Some(v), Some(t)) if v >= t
        ),
        _ => {
            let Some((pos, me)) = race.entry(Entrant::Player) else {
                return false;
            };
            if !me.finished {
                return false;
            }
            match offer.objective {
                OfferObjective::FinishingPlace | OfferObjective::FinishingPlaceLeMans => {
                    (pos as i32) < offer.objective_finishing_place
                }
                OfferObjective::NoDamage => me.undamaged,
                // Every car of the rival team must be classified behind.
                OfferObjective::BeatRival => race.order[..pos].iter().all(|c| {
                    !matches!(c.entrant, Entrant::Ai { team, .. } if team == offer.objective_team_id)
                }),
                OfferObjective::TimeTrial | OfferObjective::SpeedTest => unreachable!(),
            }
        }
    }
}

/// A test offer (time trial or speed test) has its own slots and pays no
/// appearance fee (0x4F10B0, 0x674940) [V].
pub fn is_test_offer(offer: &DriverOffer) -> bool {
    matches!(offer.objective, OfferObjective::TimeTrial | OfferObjective::SpeedTest)
}

/// The appearance fee of a driver offer: `per_race_wage × races`
/// (0x637980, 0x674CC2) [V].
pub fn appearance_fee(offer: &DriverOffer) -> i64 {
    offer.per_race_wage * offer.races.len().max(1) as i64
}
