//! The season leaderboards (the object at career +0xB0; docs/CAREER_RESEARCH.md
//! §7): the driver list ranked by reputation and the team list ranked by
//! season earnings, shown by the end-of-season pages and by My Team.
//!
//! - Init (0x66E990): empty until the player's team is founded; then the
//!   player (id −2, value 0) and every `driver` row with a `leaderboard_rank`
//!   (value = that rank), and every team that is not `various_team` (value 0).
//! - Update (0x665560 routine A, 0x665890 routine B): the player gains the
//!   event's reputation, the teammate their own; every other driver is
//!   redrawn in `[rank · lower, rank · upper)`, but never past the player
//!   when the player was ahead; the player's team gains the event's money and
//!   every AI team a draw around its target (0x652400). Routine B (Le Mans)
//!   pays only the Le Mans teams.
//! - The random numbers come from the global Mersenne Twister, reseeded with
//!   the leaderboards' own counter at every update, so the save holds the
//!   lists and the counter (0x64A660) [V].
use super::data::Catalogue;
use crate::series::Mt;
use std::collections::BTreeMap;

/// The id of the player's entry in both lists [V].
pub const PLAYER: i32 = -2;
/// The list capacities (+0x24 / +0x1624 arrays) [V].
pub const TEAM_CAP: usize = 350;
pub const DRIVER_CAP: usize = 600;
/// The Le Mans team and driver lists' capacities (+0x3C04, +0x3C84) [V].
pub const LE_MANS_TEAM_CAP: usize = 32;
pub const LE_MANS_DRIVER_CAP: usize = 64;

/// `gameconfig/season_leaderboards_setup.xml` (element
/// `SeasonLeaderboardsSetup`, loader 0x62CD60) [D]. The defaults are the
/// shipped file's values.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Setup {
    /// `teamMaxSeasonEarnings` (+0x3BB0).
    pub team_max: u32,
    /// `teamMinSeasonEarnings` (+0x3BB4).
    pub team_min: u32,
    /// `topCurveFactor` (+0x3BB8).
    pub top_curve: f32,
    /// `botCurveFactor` (+0x3BBC).
    pub bot_curve: f32,
    /// `topRand` (+0x3BC0).
    pub top_rand: f32,
    /// `botRand` (+0x3BC4).
    pub bot_rand: f32,
    /// `bias` (+0x3BC8).
    pub bias: f32,
    /// `driverLowerFactor` (+0x3BCC).
    pub driver_lower: f32,
    /// `driverUpperFactor` (+0x3BD0).
    pub driver_upper: f32,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            team_max: 40_000_000,
            team_min: 120_000,
            top_curve: 12.0,
            bot_curve: 1.0,
            top_rand: 0.05,
            bot_rand: 0.2,
            bias: 0.5,
            driver_lower: 1.0,
            driver_upper: 1.3,
        }
    }
}

impl Setup {
    /// The values of a decoded `season_leaderboards_setup.xml`; absent
    /// attributes keep the shipped values.
    pub fn from_document(doc: &grid_formats::binxml::Document<'_>) -> Result<Self, String> {
        let e = doc
            .elements
            .iter()
            .find(|e| e.name == "SeasonLeaderboardsSetup")
            .ok_or("season_leaderboards_setup.xml has no SeasonLeaderboardsSetup element")?;
        let mut s = Self::default();
        let get = |name: &str| e.attributes.iter().find(|(k, _)| *k == name).map(|(_, v)| v.trim());
        macro_rules! fields {
            ($($attr:literal => $field:ident),* $(,)?) => {$(
                if let Some(v) = get($attr) {
                    s.$field = v
                        .parse()
                        .map_err(|_| format!("season_leaderboards_setup.xml {}={v:?}", $attr))?;
                }
            )*};
        }
        fields!(
            "teamMaxSeasonEarnings" => team_max,
            "teamMinSeasonEarnings" => team_min,
            "topCurveFactor" => top_curve,
            "botCurveFactor" => bot_curve,
            "topRand" => top_rand,
            "botRand" => bot_rand,
            "bias" => bias,
            "driverLowerFactor" => driver_lower,
            "driverUpperFactor" => driver_upper,
        );
        Ok(s)
    }
}

/// One list entry (16 bytes: id, value, rank, change) [V].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    /// Driver or team id; [`PLAYER`] for the player and the player's team.
    pub id: i32,
    /// Reputation (drivers) or season earnings (teams).
    pub value: u32,
    /// 0-based place after the last sort.
    pub rank: u32,
    /// `new rank − old rank` of the last sort (negative = moved up).
    pub change: i16,
}

/// The two leaderboards and the update counter.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Leaderboards {
    pub drivers: Vec<Entry>,
    pub teams: Vec<Entry>,
    /// The seed of the next update (+0x3D98) [V].
    pub counter: u32,
}

/// What one update adds (the arguments of 0x665560 / 0x665890).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Update {
    /// The player's reputation from the event.
    pub reputation: u32,
    /// The player's team's money from the event.
    pub money: u32,
    /// The hired teammate's driver id and reputation from the event.
    pub team_mate: Option<(i32, u32)>,
    /// Leave the team list alone (driver offers) [I].
    pub drivers_only: bool,
    /// Routine B: only the Le Mans teams gain earnings.
    pub le_mans: bool,
}

/// `int(lo, hi)` of the global generator (0xA68410), as in ebay.
fn int(rng: &mut Mt, lo: u32, hi: u32) -> u32 {
    super::ebay::int(rng, lo, hi)
}

/// Stable descending sort by value, then ranks and changes (introsort
/// 0x693BF0 + 0x67D540) [V]. [I] The original's order of equal values is
/// that of its introsort; the port keeps the list order.
fn sort_and_rank(list: &mut [Entry]) {
    list.sort_by_key(|e| std::cmp::Reverse(e.value));
    for (i, e) in list.iter_mut().enumerate() {
        e.change = (i as i64 - e.rank as i64) as i16;
        e.rank = i as u32;
    }
}

/// The AI teams' per-update earnings targets (0x665C50) [V]: `(E, R)` by
/// team id, every `team` row. `N` is the team table's row count. Computed in
/// single precision as the original; `R` from the unrounded `E`, both
/// truncated.
pub fn targets(catalogue: &Catalogue) -> BTreeMap<i32, (u32, u32)> {
    let s = &catalogue.leaderboard_setup;
    let n = catalogue.team_leaderboard.len() as u32;
    let max = (s.team_max as f64 * 0.25) as f32;
    let min = (s.team_min as f64 * 0.25) as f32;
    let mut out = BTreeMap::new();
    if n == 0 {
        return out;
    }
    for &(id, rank) in &catalogue.team_leaderboard {
        let t = (n.wrapping_sub(rank) as f64 / n as f32 as f64) as f32;
        let shape = (t as f64).powf(s.bias as f64) as f32;
        let exponent = (s.top_curve - s.bot_curve) * shape + s.bot_curve;
        let mut e = (t as f64).powf(exponent as f64) as f32 * (max - min) + min;
        if catalogue.le_mans_teams.contains(&id) {
            e *= 0.8;
        }
        let spread = ((s.bot_rand + (s.top_rand - s.bot_rand) * shape) as f64 * e as f64) as i64;
        out.insert(id, (e as i64 as u32, spread as u32));
    }
    out
}

impl Leaderboards {
    /// Init (0x66E990): empty while the team is not founded.
    pub fn new(catalogue: &Catalogue, founded: bool) -> Self {
        let mut lb = Self::default();
        if !founded {
            return lb;
        }
        lb.drivers.push(Entry { id: PLAYER, ..Entry::default() });
        for &(id, rank) in &catalogue.driver_leaderboard {
            if lb.drivers.len() >= DRIVER_CAP {
                break;
            }
            lb.drivers.push(Entry { id, value: rank, ..Entry::default() });
        }
        for &(id, _) in &catalogue.team_leaderboard {
            if lb.teams.len() >= TEAM_CAP {
                break;
            }
            if catalogue.team_types.get(&id) != Some(&4) {
                lb.teams.push(Entry { id, ..Entry::default() });
            }
        }
        // [I] The original sorts only the team list here; the driver list
        // keeps table order until the first update. The port ranks both so
        // that a page opened before the next event shows ordered places.
        sort_and_rank(&mut lb.drivers);
        sort_and_rank(&mut lb.teams);
        for e in lb.drivers.iter_mut().chain(lb.teams.iter_mut()) {
            e.change = 0;
        }
        lb
    }

    /// The team is founded (0x674610): init, then the player's team entry
    /// (0x64A6B0, value 0) at the end of the team list [V]. [I] The original
    /// init trigger after founding was not traced.
    pub fn found_team(catalogue: &Catalogue) -> Self {
        let mut lb = Self::new(catalogue, true);
        if lb.teams.len() < TEAM_CAP {
            let rank = lb.teams.len() as u32;
            lb.teams.push(Entry { id: PLAYER, rank, ..Entry::default() });
        }
        lb
    }

    pub fn player(&self) -> Option<&Entry> {
        self.drivers.iter().find(|e| e.id == PLAYER)
    }

    pub fn player_team(&self) -> Option<&Entry> {
        self.teams.iter().find(|e| e.id == PLAYER)
    }

    /// One update (0x665560; routine B 0x665890 when `u.le_mans`).
    pub fn update(&mut self, catalogue: &Catalogue, u: Update) {
        let mut rng = Mt::new(self.counter);
        self.counter = self.counter.wrapping_add(1);
        let Some(p) = self.drivers.iter().position(|e| e.id == PLAYER) else {
            return;
        };
        self.drivers[p].value = self.drivers[p].value.wrapping_add(u.reputation);
        let player_new = self.drivers[p].value;
        let player_old = player_new.wrapping_sub(u.reputation);
        let team_mate = u.team_mate.map(|(id, _)| id);
        if let Some((id, rep)) = u.team_mate
            && let Some(e) = self.drivers.iter_mut().find(|e| e.id == id)
        {
            e.value = e.value.wrapping_add(rep);
        }
        if !u.drivers_only
            && let Some(e) = self.teams.iter_mut().find(|e| e.id == PLAYER)
        {
            e.value = e.value.wrapping_add(u.money);
        }
        let ranks: BTreeMap<i32, u32> = catalogue.driver_leaderboard.iter().copied().collect();
        let (lower, upper) = (
            catalogue.leaderboard_setup.driver_lower as f64,
            catalogue.leaderboard_setup.driver_upper as f64,
        );
        for e in &mut self.drivers {
            if e.id == PLAYER || Some(e.id) == team_mate {
                continue;
            }
            let rank = ranks.get(&e.id).copied().unwrap_or(0) as f64;
            let v = int(&mut rng, (rank * lower) as u32, (rank * upper) as u32);
            if v <= player_new || player_old < e.value {
                e.value = v;
            }
        }
        if !u.drivers_only {
            let targets = targets(catalogue);
            let gain = |rng: &mut Mt, id: i32| {
                targets
                    .get(&id)
                    .map_or(0, |&(e, r)| int(rng, e.wrapping_sub(r), e.wrapping_add(r)))
            };
            if u.le_mans {
                for &id in &catalogue.le_mans_teams {
                    if id == PLAYER {
                        continue;
                    }
                    if let Some(i) = self.teams.iter().position(|e| e.id == id) {
                        let g = gain(&mut rng, id);
                        self.teams[i].value = self.teams[i].value.wrapping_add(g);
                    }
                }
            } else {
                for e in &mut self.teams {
                    if e.id != PLAYER {
                        e.value = e.value.wrapping_add(gain(&mut rng, e.id));
                    }
                }
            }
        }
        sort_and_rank(&mut self.drivers);
        if !u.drivers_only {
            sort_and_rank(&mut self.teams);
        }
    }

    /// Season end (0x665BA0): every team's earnings, place and change return
    /// to 0 and the list is resorted; the drivers keep their reputation [V].
    pub fn end_season(&mut self) {
        for e in &mut self.teams {
            *e = Entry { id: e.id, ..Entry::default() };
        }
        sort_and_rank(&mut self.teams);
        for e in &mut self.teams {
            e.change = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalogue() -> Catalogue {
        Catalogue {
            driver_leaderboard: vec![(10, 1000), (11, 500), (12, 100)],
            team_leaderboard: vec![(1, 1), (2, 2), (3, 3), (4, 4)],
            team_types: [(1, 1), (2, 1), (3, 4), (4, 2)].into_iter().collect(),
            le_mans_teams: vec![2],
            ..Catalogue::default()
        }
    }

    #[test]
    fn empty_until_founded() {
        let c = catalogue();
        assert_eq!(Leaderboards::new(&c, false), Leaderboards::default());
        let lb = Leaderboards::found_team(&c);
        assert_eq!(lb.drivers.iter().map(|e| e.id).collect::<Vec<_>>(), [10, 11, 12, PLAYER]);
        // The various team 3 is left out; the player's team is last.
        assert_eq!(lb.teams.iter().map(|e| e.id).collect::<Vec<_>>(), [1, 2, 4, PLAYER]);
        assert_eq!(lb.player_team().unwrap().rank, 3);
    }

    #[test]
    fn targets_follow_the_formula() {
        let c = catalogue();
        let t = targets(&c);
        // Team 1: t = 3/4, s = t^0.5, exp = 11 s + 1; E = t^exp · (10e6 − 30e3) + 30e3.
        let tt = 0.75f64;
        let s = tt.sqrt();
        let e = tt.powf(11.0 * s + 1.0) * (10_000_000.0 - 30_000.0) + 30_000.0;
        let (e1, r1) = t[&1];
        assert!((e1 as f64 - e).abs() < 2.0, "{e1} vs {e}");
        let r = (0.2 + (0.05 - 0.2) * s) * e;
        assert!((r1 as f64 - r).abs() < 2.0);
        // The Le Mans team 2 earns 0.8 of its curve.
        let tt = 0.5f64;
        let s = tt.sqrt();
        let e = (tt.powf(11.0 * s + 1.0) * (10_000_000.0 - 30_000.0) + 30_000.0) * 0.8;
        assert!((t[&2].0 as f64 - e).abs() < 2.0);
    }

    #[test]
    fn update_pays_player_and_redraws_rivals() {
        let c = catalogue();
        let mut lb = Leaderboards::found_team(&c);
        lb.update(
            &c,
            Update { reputation: 700, money: 50_000, ..Update::default() },
        );
        assert_eq!(lb.counter, 1);
        assert_eq!(lb.player().unwrap().value, 700);
        for e in &lb.drivers {
            if e.id != PLAYER {
                let rank = c.driver_leaderboard.iter().find(|r| r.0 == e.id).unwrap().1;
                assert!(e.value >= rank && e.value < rank * 13 / 10 + 1, "{e:?}");
            }
        }
        // Sorted with ranks 0..n.
        assert!(lb.drivers.windows(2).all(|w| w[0].value >= w[1].value));
        assert!(lb.drivers.iter().enumerate().all(|(i, e)| e.rank == i as u32));
        assert_eq!(lb.player_team().unwrap().value, 50_000);
        // The same counter gives the same draws.
        let mut again = Leaderboards::found_team(&c);
        again.update(&c, Update { reputation: 700, money: 50_000, ..Update::default() });
        assert_eq!(lb, again);
    }

    #[test]
    fn rivals_do_not_jump_a_player_who_was_ahead() {
        let c = catalogue();
        let mut lb = Leaderboards::found_team(&c);
        lb.drivers.iter_mut().find(|e| e.id == PLAYER).unwrap().value = 1100;
        lb.update(&c, Update { reputation: 1, drivers_only: true, ..Update::default() });
        // Driver 10 draws in [1000, 1300): it keeps 1000 unless the draw ≤ 1101.
        let d10 = lb.drivers.iter().find(|e| e.id == 10).unwrap();
        assert!(d10.value <= 1101, "{d10:?}");
        // Drivers-only updates leave the teams untouched.
        assert!(lb.teams.iter().all(|e| e.value == 0));
    }

    #[test]
    fn le_mans_pays_only_le_mans_teams_and_season_end_resets_teams() {
        let c = catalogue();
        let mut lb = Leaderboards::found_team(&c);
        lb.update(&c, Update { reputation: 10, money: 5, le_mans: true, ..Update::default() });
        let value = |lb: &Leaderboards, id| lb.teams.iter().find(|e| e.id == id).unwrap().value;
        assert!(value(&lb, 2) > 0);
        assert_eq!(value(&lb, 1), 0);
        assert_eq!(value(&lb, PLAYER), 5);
        let rep = lb.player().unwrap().value;
        lb.end_season();
        assert!(lb.teams.iter().all(|e| e.value == 0 && e.change == 0));
        assert_eq!(lb.player().unwrap().value, rep);
    }

    /// The purchased data: the shipped setup file, the table sizes, the Le
    /// Mans teams and one founded season's first update.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn the_shipped_leaderboards() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"));
        let c = Catalogue::load(&root).unwrap();
        assert_eq!(c.leaderboard_setup, Setup::default());
        assert_eq!(c.team_leaderboard.len(), 250);
        assert_eq!(c.driver_leaderboard.len(), 499);
        assert!(!c.le_mans_teams.is_empty() && c.le_mans_teams.len() < LE_MANS_TEAM_CAP, "{:?}", c.le_mans_teams);
        let lb = Leaderboards::found_team(&c);
        assert_eq!(lb.drivers.len(), 500);
        let various = c.team_types.values().filter(|&&t| t == 4).count();
        assert_eq!(lb.teams.len(), 250 - various + 1);
        let t = targets(&c);
        let (top, _) = c.team_leaderboard.iter().min_by_key(|r| r.1).copied().unwrap();
        let (bottom, _) = c.team_leaderboard.iter().max_by_key(|r| r.1).copied().unwrap();
        assert!(t[&top].0 > t[&bottom].0 && t[&bottom].0 >= 30_000, "{:?} {:?}", t[&top], t[&bottom]);
        let mut lb = lb;
        lb.update(&c, Update { reputation: 2000, money: 30_000, ..Update::default() });
        assert!(lb.drivers.windows(2).all(|w| w[0].value >= w[1].value));
        assert!(lb.teams.windows(2).all(|w| w[0].value >= w[1].value));
        eprintln!(
            "le mans teams {:?}; top team {top} E,R {:?}; bottom {bottom} {:?}; player place {} of {}; team place {}",
            c.le_mans_teams,
            t[&top],
            t[&bottom],
            lb.player().unwrap().rank,
            lb.drivers.len(),
            lb.player_team().unwrap().rank
        );
    }
}
