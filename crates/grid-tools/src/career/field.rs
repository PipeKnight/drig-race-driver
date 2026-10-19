//! A career event's races and field: the race draw from the event's track
//! pool, the AI field of its `event_driver` rows with the player's team
//! placed in it, and the grid (docs/CAREER_RESEARCH.md §12).
use super::data::{Catalogue, EventRace};
use super::rules::Entrant;
use super::state::Career;
use crate::series::Mt;
use std::collections::BTreeMap;

/// The race draw (0x67BC80 → 0x664E30) [V]: the pool is sorted stably by
/// `order_index` on an event's first entry, else (a completed event, and
/// every driver offer) by the profile's race count on each layout; then the
/// entries of a venue (`track_id`) not taken yet come first, and the rest
/// of the sorted list fills up to `n`. No randomness.
pub fn draw_races(
    catalogue: &Catalogue,
    pool: &[EventRace],
    n: usize,
    by_order: bool,
    layout_races: &BTreeMap<i32, u32>,
) -> Vec<EventRace> {
    let mut sorted: Vec<&EventRace> = pool.iter().collect();
    if by_order {
        sorted.sort_by_key(|r| r.order_index);
    } else {
        sorted.sort_by_key(|r| layout_races.get(&r.track_model_id).copied().unwrap_or(0));
    }
    let venue = |r: &EventRace| catalogue.venues.get(&r.track_model_id).copied();
    let mut out: Vec<&EventRace> = Vec::new();
    let mut venues = Vec::new();
    for r in &sorted {
        if out.len() >= n {
            break;
        }
        let v = venue(r);
        if !venues.contains(&v) {
            venues.push(v);
            out.push(r);
        }
    }
    for r in &sorted {
        if out.len() >= n {
            break;
        }
        if !out.iter().any(|o| std::ptr::eq(*o, *r)) {
            out.push(r);
        }
    }
    out.into_iter().cloned().collect()
}

/// One car of a career field.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldCar {
    pub entrant: Entrant,
    /// The car's driver (`driver.id`); `None` for the player.
    pub driver_id: Option<i32>,
    /// The car's team (0x62AC70): the original driver's team for a legend
    /// substitute, else `driver.team_id`; the player's and team mate's cars
    /// keep the team they took over.
    pub team_id: i32,
    /// `vehicle_livery.id`: the car and its livery.
    pub livery_id: i32,
    /// `driver.skill` (the grid key); 0 for the player.
    pub skill: f32,
}

/// What the player enters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    /// An own-team event (`event.id`).
    Event(i32),
    /// A driver offer (`driver_offer.id`).
    Offer(i32),
}

fn ai_cars(catalogue: &Catalogue, career: &Career, event_id: i32, rng: &mut Mt) -> Vec<FieldCar> {
    let Some(event) = catalogue.event(event_id) else {
        return Vec::new();
    };
    let mate = career.team_mate.as_ref().map(|t| t.driver_id);
    let legends = catalogue.legends.get(&event_id).cloned().unwrap_or_default();
    event
        .drivers
        .iter()
        .map(|d| {
            let team_id = catalogue.drivers.get(&d.driver_id).map_or(0, |x| x.team_id);
            // Legend substitution (0x663D40, 0x62BEE0) [V]: the hired team
            // mate's seat goes to a random legend of the event, who keeps
            // the original driver's team.
            let driver_id = if Some(d.driver_id) == mate && !legends.is_empty() {
                legends[rng.range(0, legends.len() as u32) as usize % legends.len()]
            } else {
                d.driver_id
            };
            FieldCar {
                entrant: Entrant::Ai {
                    driver: driver_id,
                    team: team_id,
                },
                driver_id: Some(driver_id),
                team_id,
                livery_id: d.livery_id,
                skill: catalogue.drivers.get(&driver_id).map_or(0.5, |x| x.skill),
            }
        })
        .collect()
}

/// The field of an entry, in `event_driver` order (the player is moved to
/// index 0, as 0x649E90 does) [V]:
/// - own-team event (0x673F20): the player and the hired team mate
///   overwrite the two lowest-index cars of a team drawn among the teams
///   with enough cars, `key` and `primary_key` teams excluded
///   (0x66D350, `cand[rng(0, k)]`); without such a team each overwrites a
///   random car;
/// - race offer (0x674020): the player overwrites the lowest-index car of
///   `driver_offer.team_id` (key teams allowed), else a random team's car;
///   no own team mate;
/// - test offer: the player alone (0x663470).
///
/// The player's car is `player_livery` (an own-team event: the port's
/// AUTHORED choice; an offer: `driver_offer.vehicle_livery_id`). Not
/// ported: the Ravenwest guest cars and Le Mans fields (`lemans_restrictions`).
pub fn field(
    catalogue: &Catalogue,
    career: &Career,
    entry: Entry,
    player_livery: i32,
    rng: &mut Mt,
) -> Vec<FieldCar> {
    let player = player_car(player_livery);
    let (event_id, offer) = match entry {
        Entry::Event(id) => (id, None),
        Entry::Offer(id) => match catalogue.driver_offers.get(&id) {
            Some(o) if super::rules::is_test_offer(o) => return vec![player],
            Some(o) => (o.event_id, Some(o)),
            None => return vec![player],
        },
    };
    let mut cars = ai_cars(catalogue, career, event_id, rng);
    if cars.is_empty() {
        return vec![player];
    }
    let mut group = vec![player];
    if offer.is_none()
        && let Some(t) = &career.team_mate
    {
        group.push(FieldCar {
            entrant: Entrant::TeamMate,
            driver_id: Some(t.driver_id),
            team_id: 0,
            livery_id: player_livery,
            skill: catalogue.drivers.get(&t.driver_id).map_or(0.5, |d| d.skill),
        });
    }
    // Team → field indices of its eligible AI cars, by team id (0x65EF50).
    let include_key = offer.is_some();
    let mut teams: BTreeMap<i32, Vec<usize>> = BTreeMap::new();
    for (i, c) in cars.iter().enumerate() {
        let kind = catalogue.team_types.get(&c.team_id).copied().unwrap_or(1);
        if include_key || !matches!(kind, 2 | 3) {
            teams.entry(c.team_id).or_default().push(i);
        }
    }
    let targets: Vec<usize> = match offer.and_then(|o| teams.get(&o.team_id)) {
        Some(own) => own.iter().take(1).copied().collect(),
        None => {
            let cand: Vec<&Vec<usize>> = teams.values().filter(|v| v.len() >= group.len()).collect();
            if cand.is_empty() {
                // Each member takes a random eligible car.
                let mut pool: Vec<usize> = teams.values().flatten().copied().collect();
                pool.sort_unstable();
                let mut out = Vec::new();
                for _ in 0..group.len() {
                    if pool.is_empty() {
                        break;
                    }
                    let k = rng.range(0, pool.len() as u32) as usize % pool.len();
                    out.push(pool.remove(k));
                }
                out
            } else {
                let k = rng.range(0, cand.len() as u32) as usize % cand.len();
                cand[k].iter().take(group.len()).copied().collect()
            }
        }
    };
    for (member, &i) in group.into_iter().zip(&targets) {
        let team_id = cars[i].team_id;
        let livery_id = if member.entrant == Entrant::Player { member.livery_id } else { cars[i].livery_id };
        cars[i] = FieldCar {
            team_id,
            livery_id,
            ..member
        };
    }
    if let Some(p) = cars.iter().position(|c| c.entrant == Entrant::Player) {
        let player = cars.remove(p);
        cars.insert(0, player);
    } else {
        cars.insert(0, player_car(player_livery));
    }
    cars
}

fn player_car(livery_id: i32) -> FieldCar {
    FieldCar {
        entrant: Entrant::Player,
        driver_id: None,
        team_id: 0,
        livery_id,
        skill: 0.0,
    }
}

/// AUTHORED stand-in for the original's float draw 0xA683B0 (not traced):
/// `lo + (genrand / 2^32) · (hi − lo)`.
fn uniform(rng: &mut Mt, lo: f32, hi: f32) -> f32 {
    lo + (rng.next_u32() as f64 / 4_294_967_296.0) as f32 * (hi - lo)
}

/// The grid (0x5C7D40) [V]: field indices in slot order (slot 0 = pole).
/// - the player's slot from `(base, range)` of the race's pool row
///   (`race::grid_slot`);
/// - named drivers' slots from `event_driver_grid_position` rows of
///   `pool_id` (same formula; AUTHORED: a taken slot leaves the driver to
///   the ranking);
/// - every other car ranked into the free slots by
///   `n − 1 − trunc((n − 1) · clamp(skill + U(−0.2, 0.2), 0, 1))`, ties in
///   field order (AUTHORED: the ranking 0x5B6340's tie order is not traced).
pub fn grid(
    catalogue: &Catalogue,
    cars: &[FieldCar],
    pool_id: Option<i32>,
    base: i32,
    range: i32,
    rng: &mut Mt,
) -> Vec<usize> {
    let n = cars.len();
    let mut slots: Vec<Option<usize>> = vec![None; n];
    let mut placed = vec![false; n];
    if let Some(p) = cars.iter().position(|c| c.entrant == Entrant::Player) {
        let s = crate::race::grid_slot(base.max(0) as u32, range.max(0) as u32, n, |r| rng.range(0, r));
        slots[s] = Some(p);
        placed[p] = true;
    }
    if let Some(pool) = pool_id {
        for slot in catalogue.named_slots.iter().filter(|s| s.pool_id == pool) {
            let Some(i) = cars.iter().position(|c| c.driver_id == Some(slot.driver_id) && c.entrant != Entrant::Player) else {
                continue;
            };
            let s = crate::race::grid_slot(slot.base.max(0) as u32, slot.range.max(0) as u32, n, |r| rng.range(0, r));
            if slots[s].is_none() && !placed[i] {
                slots[s] = Some(i);
                placed[i] = true;
            }
        }
    }
    let top = n.saturating_sub(1) as f32;
    let mut keyed: Vec<(usize, usize)> = (0..n)
        .filter(|&i| !placed[i])
        .map(|i| {
            let x = (cars[i].skill + uniform(rng, -0.2, 0.2)).clamp(0.0, 1.0);
            (n.saturating_sub(1) - (top * x) as usize, i)
        })
        .collect();
    keyed.sort_by_key(|k| k.0);
    let mut rest = keyed.into_iter().map(|k| k.1);
    for s in &mut slots {
        if s.is_none() {
            *s = rest.next();
        }
    }
    slots.into_iter().flatten().collect()
}
