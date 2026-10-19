//! The persistent career state and its updates (docs/CAREER_RESEARCH.md
//! §3, §8). The fields follow the original's career unit (`main_career`
//! save 0x6501B0, docs §9) where that is known; the rest is AUTHORED.
use super::data::{Catalogue, Region};
use super::rules::{self, Classified, Entrant, RaceResult, RaceSettings};
use std::collections::{BTreeMap, BTreeSet};

/// Sponsor slots on the car: one major (pays double) and seven minor
/// (career sponsor manager +0x44, +0x48..+0x60) [V].
pub const MINOR_SLOTS: usize = 7;

/// What a season entry counts (result kind +0x44) [V].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeasonEntry {
    Event(i32),
    DriverOffer(i32),
}

/// The season state (0x62C140) [V].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeasonState {
    /// Fewer than four counted entries.
    Running,
    /// Four entries: Le Mans 24 Hours is offered (or may be refused).
    LeMansPending,
    /// Le Mans driven or refused: the season ends.
    Ended,
}

/// A hired teammate (a `driver` row with `can_be_hired`) [V 0x662850].
#[derive(Clone, Debug, PartialEq)]
pub struct HiredTeamMate {
    pub driver_id: i32,
    pub winnings_percentage: i32,
}

/// A car the team owns. AUTHORED: the original keeps more per car (eBay
/// condition, mileage) in `EBAY_MANAGER_SAVE`, not decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedCar {
    pub vehicle_id: i32,
    pub livery: i32,
}

/// The career's state between events.
#[derive(Clone, Debug, PartialEq)]
pub struct Career {
    /// Bank balance (main_career +0x15EC) [V].
    pub money: i64,
    /// Reputation per region: europe, united_states, japan, international,
    /// le_mans (main_career +0x15F4) [V].
    pub reputation: [i64; 5],
    /// Season number, starting at 1 (main_career +0x1680) [V].
    pub season_number: i32,
    /// The own team exists (founded when the bank reaches
    /// `start_race_team`, 0x674610) [V].
    pub team_founded: bool,
    pub team_name: String,
    pub cars: Vec<OwnedCar>,
    pub team_mate: Option<HiredTeamMate>,
    /// Signing fees paid in total (+0xA10) [V].
    pub team_mate_spending: i64,
    /// The money paid this season (the bank's "Season's Earnings"). [I]
    /// Every payout adds its total; `end_season` clears it.
    pub season_earnings: i64,
    /// Received sponsor offers (`event_sponsor_offer.id`), in arrival order.
    pub sponsor_offers: Vec<i32>,
    pub major_sponsor: Option<i32>,
    pub minor_sponsors: [Option<i32>; MINOR_SLOTS],
    /// Remaining reputation pot per event id (0x65F130 records) [V].
    pub reputation_pots: BTreeMap<i32, i64>,
    /// Completed driver offers (`driver_offer.id`).
    pub completed_offers: BTreeSet<i32>,
    /// Events finished at least once, with the best 0-based result.
    pub event_results: BTreeMap<i32, usize>,
    /// The current season's counted entries (profile +0x1620, at most 5)
    /// [V 0x6427A0]: own events and driver offers finished while the team is
    /// active.
    pub season: Vec<SeasonEntry>,
    /// The previous season (+0x1650) [V].
    pub previous_season: Vec<SeasonEntry>,
    /// Le Mans refused this season (+0x164C) [V].
    pub lemans_refused: bool,
    /// Head-to-head events made available (0x656D40 -> 0x64EA20) [V].
    pub head_to_heads: BTreeSet<i32>,
    /// The final (International) head-to-head was won (profile +0x89) [V].
    pub game_completed: bool,
    pub races_won: i32,
    pub races_lost: i32,
    /// Races driven per `track_model` (the per-layout record 0x4F8B30 +8,
    /// incremented after every race at 0x61017D) [V]: the race draw's key
    /// on a repeated event.
    pub layout_races: BTreeMap<i32, u32>,
}

impl Career {
    /// A new career: `start_money` is not given; the player starts with no
    /// money and no team and earns it with driver offers (0x674610) [V].
    pub fn new() -> Self {
        Self {
            money: 0,
            reputation: [0; 5],
            season_number: 1,
            team_founded: false,
            team_name: String::new(),
            cars: Vec::new(),
            team_mate: None,
            team_mate_spending: 0,
            season_earnings: 0,
            sponsor_offers: Vec::new(),
            major_sponsor: None,
            minor_sponsors: [None; MINOR_SLOTS],
            reputation_pots: BTreeMap::new(),
            completed_offers: BTreeSet::new(),
            event_results: BTreeMap::new(),
            season: Vec::new(),
            previous_season: Vec::new(),
            lemans_refused: false,
            head_to_heads: BTreeSet::new(),
            game_completed: false,
            races_won: 0,
            races_lost: 0,
            layout_races: BTreeMap::new(),
        }
    }

    /// Counts one race driven on a layout (0x61017D) [V].
    pub fn count_race(&mut self, track_model_id: i32) {
        *self.layout_races.entry(track_model_id).or_default() += 1;
    }

    /// Total reputation: the sum of the five region values (0x62C4A0) [V].
    pub fn total_reputation(&self) -> i64 {
        self.reputation.iter().sum()
    }

    /// The rank index (0 rookie .. 3 international) in a region (0x639150)
    /// [V]: the highest of the first three ranks whose threshold the region's
    /// reputation reaches; international (3) when the total reaches its
    /// threshold.
    pub fn rank(&self, catalogue: &Catalogue, region: Region) -> usize {
        let thresholds: Vec<i64> = catalogue.ranks.iter().map(|r| r.reputation_to_unlock).collect();
        if thresholds.len() > 3 && self.total_reputation() >= thresholds[3] {
            return 3;
        }
        if region == Region::LeMans || region == Region::International {
            return 0;
        }
        let rep = self.reputation[region as usize];
        (0..3.min(thresholds.len()))
            .rev()
            .find(|&i| rep >= thresholds[i])
            .unwrap_or(0)
    }

    /// The overall rank: international by the total, else the best of the
    /// three home regions [V].
    pub fn overall_rank(&self, catalogue: &Catalogue) -> usize {
        [Region::Europe, Region::UnitedStates, Region::Japan]
            .into_iter()
            .map(|r| self.rank(catalogue, r))
            .max()
            .unwrap_or(0)
    }

    /// AddMoney (0x674610) [V]: credits the bank; while the player has no
    /// team, reaching `start_race_team` (40,000) spends it to found the team,
    /// which also brings rookie licences in the three regions and the free
    /// `initial_vehicle_id` car (0x66F640, 0x656100). Returns true when the
    /// team was founded by this credit.
    pub fn add_money(&mut self, catalogue: &Catalogue, amount: i64) -> bool {
        self.money += amount;
        if !self.team_founded && self.money >= catalogue.start_money {
            self.money -= catalogue.start_money;
            self.team_founded = true;
            if !self.cars.iter().any(|c| c.vehicle_id == catalogue.initial_vehicle_id) {
                self.cars.push(OwnedCar {
                    vehicle_id: catalogue.initial_vehicle_id,
                    livery: 0,
                });
            }
            return true;
        }
        false
    }

    /// The licence countdown (+0x1694) [V 0x66B1D0]: 7 at the start, one
    /// less for each regional pro or expert licence, 0 with the Global Race
    /// Permit. Reputation never falls, so it follows from the reputation.
    pub fn licence_countdown(&self, catalogue: &Catalogue) -> i32 {
        if self.overall_rank(catalogue) == 3 {
            return 0;
        }
        let earned: usize = [Region::Europe, Region::UnitedStates, Region::Japan]
            .into_iter()
            .map(|r| self.rank(catalogue, r).min(2))
            .sum();
        7 - earned as i32
    }

    /// Licences held, as the "Licences: [VALUE] / [TOTAL]" fact [I]: 7 −
    /// countdown out of 7.
    pub fn licences(&self, catalogue: &Catalogue) -> i32 {
        7 - self.licence_countdown(catalogue)
    }

    /// Garages: 1, 2 after the first licence, 3 after the fourth (0x4DA640)
    /// [V].
    pub fn garages(&self, catalogue: &Catalogue) -> i32 {
        match self.licence_countdown(catalogue) {
            c if c <= 3 => 3,
            c if c < 7 => 2,
            _ => 1,
        }
    }

    /// Whether a tier's events can be entered by the own team. [I] A
    /// regional tier needs the region's rank, the International tier the
    /// overall rank 3, the Le Mans tiers a pending Le Mans entry (0x58A200).
    /// The tier availability code (0x6766B0) was not fully traced.
    pub fn tier_open(&self, catalogue: &Catalogue, tier_id: i32) -> bool {
        let Some(tier) = catalogue.tier(tier_id) else {
            return false;
        };
        if tier.driver_offers || !self.team_founded {
            return false;
        }
        let need = (tier.rank_id - 1).max(0) as usize;
        match tier.region {
            Region::International => self.overall_rank(catalogue) == 3,
            Region::LeMans => {
                self.season_state() == SeasonState::LeMansPending
                    && self.overall_rank(catalogue) >= need
            }
            region => self.rank(catalogue, region) >= need,
        }
    }

    /// Whether an event can be entered now: its tier is open and, for a
    /// head-to-head, every other event of the tier was won (0x656D40) [V; I
    /// "won" for the event record field +0x64].
    pub fn event_open(&self, catalogue: &Catalogue, event_id: i32) -> bool {
        let Some(event) = catalogue.event(event_id) else {
            return false;
        };
        self.tier_open(catalogue, event.tier_id)
            && (!event.head_to_head || self.head_to_heads.contains(&event_id))
    }

    /// The driver offers on show (0x651480, 0x650FA0) [V partial]: three
    /// region slots by `tier.order_index` (0 US, 1 Europe, 2 Japan), each
    /// with one race offer and one test offer (time trial or speed test,
    /// separate slots +0x80..+0x88): the first offer by
    /// `driver_offer.order_index` not driven yet, in the driver-offer tier of
    /// the player's rank in that region (the International offer tiers at
    /// overall rank 3). [I] The slot's tier choice (0x67E450) and that a
    /// driven offer, met or not, moves the slot on (0x651920 after the race)
    /// follow the research notes; the Le Mans offer tiers are not slots here.
    pub fn driver_offers_shown(&self, catalogue: &Catalogue) -> Vec<i32> {
        let international = self.overall_rank(catalogue) == 3;
        let mut shown = Vec::new();
        for slot in 0..3 {
            let tier = catalogue.tiers.iter().find(|t| {
                t.driver_offers
                    && t.order_index == slot
                    && match t.region {
                        Region::LeMans => false,
                        Region::International => international && t.rank_id == 4,
                        region => !international && t.rank_id as usize == self.rank(catalogue, region) + 1,
                    }
            });
            let Some(tier) = tier else {
                continue;
            };
            let mut offers: Vec<&super::data::DriverOffer> = catalogue
                .driver_offers
                .values()
                .filter(|o| o.tier_id == tier.id && !self.completed_offers.contains(&o.id))
                .collect();
            offers.sort_by_key(|o| (o.order_index, o.id));
            for test in [false, true] {
                if let Some(o) = offers.iter().find(|o| rules::is_test_offer(o) == test) {
                    shown.push(o.id);
                }
            }
        }
        shown
    }

    /// The season state (0x62C140) [V].
    pub fn season_state(&self) -> SeasonState {
        match (self.season.len(), self.lemans_refused) {
            (0..=3, _) => SeasonState::Running,
            (4, false) => SeasonState::LeMansPending,
            _ => SeasonState::Ended,
        }
    }

    /// Counts a finished event or offer into the season, only while the own
    /// team is active and the season has room (0x6427A0, 0x67BF2D) [V].
    pub fn count_in_season(&mut self, entry: SeasonEntry) {
        if self.team_founded && self.season.len() < 5 {
            self.season.push(entry);
        }
    }

    /// Refuses Le Mans: the season ends (0x4DBAD0) [V].
    pub fn refuse_lemans(&mut self) {
        self.lemans_refused = true;
    }

    /// Ends an ended season (0x4DBB20, 0x656760) [V]: archives it and
    /// increments the season number. Money, reputation and event
    /// availability are not reset [V by absence]. Returns false when the
    /// season has not ended.
    pub fn end_season(&mut self) -> bool {
        if self.season_state() != SeasonState::Ended {
            return false;
        }
        self.previous_season = std::mem::take(&mut self.season);
        self.lemans_refused = false;
        self.season_earnings = 0;
        self.season_number += 1;
        true
    }

    /// Makes a tier's head-to-head available when every other event of the
    /// tier has been won (0x656D40) [V; I "won"].
    fn update_head_to_head(&mut self, catalogue: &Catalogue, tier_id: i32) {
        let events = catalogue.tier_events(tier_id);
        let won = events
            .iter()
            .filter(|e| !e.head_to_head)
            .all(|e| self.event_results.get(&e.id) == Some(&0));
        if won {
            for e in events.iter().filter(|e| e.head_to_head) {
                self.head_to_heads.insert(e.id);
            }
        }
    }

    /// Buys a car: `purchase_price` from the bank (0x65F652) [V]. The car
    /// must be `purchasable`.
    pub fn buy_car(&mut self, catalogue: &Catalogue, vehicle_id: i32) -> Result<(), String> {
        let v = catalogue
            .vehicles
            .get(&vehicle_id)
            .ok_or(format!("vehicle {vehicle_id} absent"))?;
        if !v.purchasable {
            return Err(format!("{} is not for sale", v.stem));
        }
        if self.money < v.purchase_price {
            return Err("insufficient funds".into());
        }
        self.money -= v.purchase_price;
        self.cars.push(OwnedCar {
            vehicle_id,
            livery: 0,
        });
        Ok(())
    }

    /// Sells a car for its `sale_price` ("You will receive [VALUE]") [D].
    pub fn sell_car(&mut self, catalogue: &Catalogue, index: usize) -> Result<i64, String> {
        if index >= self.cars.len() {
            return Err("no such car".into());
        }
        let car = self.cars.remove(index);
        let price = catalogue
            .vehicles
            .get(&car.vehicle_id)
            .map(|v| v.sale_price)
            .unwrap_or(0);
        self.money += price;
        Ok(price)
    }

    /// Hires a teammate for his `signing_fee` (0x6994E0) [V]; replaces the
    /// current one without refund.
    pub fn hire(&mut self, driver_id: i32, signing_fee: i64, winnings_percentage: i32) -> Result<(), String> {
        if signing_fee > self.money {
            return Err("insufficient funds".into());
        }
        self.money -= signing_fee;
        self.team_mate_spending += signing_fee;
        self.team_mate = Some(HiredTeamMate {
            driver_id,
            winnings_percentage,
        });
        Ok(())
    }

    /// Places a received offer in a slot (`None` = major). The offer must
    /// have been received and may occupy one slot only.
    pub fn place_sponsor(&mut self, offer_id: i32, slot: Option<usize>) -> Result<(), String> {
        if !self.sponsor_offers.contains(&offer_id) {
            return Err(format!("sponsor offer {offer_id} not received"));
        }
        if slot.is_some_and(|i| i >= MINOR_SLOTS) {
            return Err("no such sponsor slot".into());
        }
        if self.major_sponsor == Some(offer_id) {
            self.major_sponsor = None;
        }
        for s in &mut self.minor_sponsors {
            if *s == Some(offer_id) {
                *s = None;
            }
        }
        match slot {
            None => self.major_sponsor = Some(offer_id),
            Some(i) => self.minor_sponsors[i] = Some(offer_id),
        }
        Ok(())
    }

    /// Each placed sponsor's objective and pay in one race (0x66DB80 per
    /// slot, 0x652010 pay) [V]: the player and the teammate are checked
    /// separately, the major slot pays double.
    pub fn sponsor_pay(&self, catalogue: &Catalogue, race: &RaceResult) -> Vec<SponsorPay> {
        let met = |who: Entrant, offer: &super::data::SponsorOffer| {
            race.order
                .iter()
                .enumerate()
                .find(|(_, c)| c.entrant == who)
                .is_some_and(|(pos, c): (usize, &Classified)| rules::sponsor_objective_met(offer, pos, c))
        };
        self.sponsor_slots()
            .filter_map(|(offer_id, major)| {
                let offer = catalogue.sponsor_offer(offer_id)?;
                let player_met = met(Entrant::Player, offer);
                let teammate_met = self.team_mate.is_some() && met(Entrant::TeamMate, offer);
                Some(SponsorPay {
                    offer_id,
                    sponsor_id: offer.sponsor_id,
                    major,
                    player_met,
                    teammate_met,
                    money: rules::sponsor_race_pay(offer, major, player_met, teammate_met),
                })
            })
            .collect()
    }

    /// Whether the teammate screen is open. [I] The original forces it
    /// right after the first licence (second garage, flow.xml
    /// `force_buy_team_mates`); its own access predicate was not found.
    pub fn can_hire(&self, catalogue: &Catalogue) -> bool {
        self.team_founded && self.garages(catalogue) >= 2
    }

    /// The teammate search (0x638A60) [V filter]: fee within the chosen
    /// `team_mate_fee_range` (all when `None`) and the region (any when
    /// `None`). AUTHORED: sorted by fee, then driver id; the current
    /// teammate is left out.
    pub fn search<'a>(
        &self,
        catalogue: &'a Catalogue,
        region: Option<Region>,
        fee_range: Option<usize>,
    ) -> Vec<&'a super::data::Hireable> {
        let range = fee_range.and_then(|i| catalogue.fee_ranges.get(i)).copied();
        let mut out: Vec<&super::data::Hireable> = catalogue
            .hireable
            .iter()
            .filter(|h| range.is_none_or(|(lo, hi)| h.signing_fee >= lo && h.signing_fee <= hi))
            .filter(|h| region.is_none_or(|r| h.region_id == r.id()))
            .filter(|h| self.team_mate.as_ref().is_none_or(|t| t.driver_id != h.driver_id))
            .collect();
        out.sort_by_key(|h| (h.signing_fee, h.driver_id));
        out
    }

    /// Hires a driver of the pool by id (0x6994E0) [V].
    pub fn hire_driver(&mut self, catalogue: &Catalogue, driver_id: i32) -> Result<(), String> {
        if !self.can_hire(catalogue) {
            return Err("not ready to hire a team mate yet".into());
        }
        let h = catalogue
            .hireable
            .iter()
            .find(|h| h.driver_id == driver_id)
            .ok_or(format!("driver {driver_id} cannot be hired"))?;
        self.hire(h.driver_id, h.signing_fee, h.winnings_percentage)
    }

    /// Takes a received offer off the car.
    pub fn remove_sponsor(&mut self, offer_id: i32) {
        if self.major_sponsor == Some(offer_id) {
            self.major_sponsor = None;
        }
        for s in &mut self.minor_sponsors {
            if *s == Some(offer_id) {
                *s = None;
            }
        }
    }

    fn sponsor_slots(&self) -> impl Iterator<Item = (i32, bool)> + '_ {
        self.major_sponsor
            .map(|o| (o, true))
            .into_iter()
            .chain(self.minor_sponsors.iter().flatten().map(|&o| (o, false)))
    }
}

impl Default for Career {
    fn default() -> Self {
        Self::new()
    }
}

/// One placed sponsor's result in one race.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SponsorPay {
    pub offer_id: i32,
    pub sponsor_id: i32,
    pub major: bool,
    pub player_met: bool,
    pub teammate_met: bool,
    pub money: i64,
}

/// The money and reputation an event paid, item by item (the bank screen's
/// lines, career +0x5C..+0x70) [V].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Payout {
    /// The player's 0-based reward position (championship rank).
    pub position: Option<usize>,
    pub prize: i64,
    pub team_mate_prize: i64,
    pub team_win_bonus: i64,
    pub sponsor_money: i64,
    pub driver_offer_wages: i64,
    pub driver_offer_bonus: i64,
    pub reputation_base: i64,
    pub reputation_bonus: i64,
    pub driver_offer_reputation: i64,
    pub region: Option<Region>,
    pub new_sponsor_offers: Vec<i32>,
    pub team_founded: bool,
    /// A head-to-head event (paid by 0x6788D0): won when `position` is 0.
    pub head_to_head: bool,
}

impl Payout {
    pub fn money(&self) -> i64 {
        self.prize
            + self.team_mate_prize
            + self.team_win_bonus
            + self.sponsor_money
            + self.driver_offer_wages
            + self.driver_offer_bonus
    }

    pub fn reputation(&self) -> i64 {
        self.reputation_base + self.reputation_bonus + self.driver_offer_reputation
    }
}

/// Pays a finished career event of the own team (reward routine A,
/// 0x6779C0) [V unless marked]:
/// - standings over the races by the event's points table; the player's
///   rank is the reward position;
/// - prize money and the remaining reputation pot (then reduced by the base
///   reputation, never below `reputation_pot_min`);
/// - reputation bonus for the race settings;
/// - the teammate's share and the team win bonus;
/// - each race's sponsor objectives for the player and the teammate;
/// - sponsor offers unlocked by the player's position (0x673B20).
///
/// Head-to-head events pay `prize_money_for_first` for a win and no
/// reputation (0x6788D0) [V].
pub fn apply_event(
    catalogue: &Catalogue,
    career: &mut Career,
    event_id: i32,
    races: &[RaceResult],
    settings: &RaceSettings,
) -> Result<Payout, String> {
    let event = catalogue
        .event(event_id)
        .ok_or(format!("event {event_id} absent"))?;
    let tier = catalogue
        .tier(event.tier_id)
        .ok_or(format!("tier {} absent", event.tier_id))?;
    let mut payout = Payout {
        region: Some(tier.region),
        ..Payout::default()
    };

    // Sponsors pay per race (0x66DB80, 0x652010) [V].
    for race in races {
        payout.sponsor_money += career
            .sponsor_pay(catalogue, race)
            .iter()
            .map(|p| p.money)
            .sum::<i64>();
        match race.position(Entrant::Player) {
            Some(0) => career.races_won += 1,
            _ => career.races_lost += 1,
        }
    }

    if event.head_to_head {
        payout.head_to_head = true;
        let won = races
            .last()
            .and_then(|r| r.order.first())
            .is_some_and(|c| c.entrant == Entrant::Player && c.finished);
        if won {
            payout.prize = event.prize_money_for_first;
            payout.position = Some(0);
            career.event_results.insert(event.id, 0);
            // The International head-to-head completes the game (+0x89) [V].
            if tier.rank_id == 4 {
                career.game_completed = true;
            }
        }
        career.count_in_season(SeasonEntry::Event(event.id));
        career.season_earnings += payout.money();
        payout.team_founded = career.add_money(catalogue, payout.money());
        return Ok(payout);
    }

    let points = catalogue
        .points
        .get(&event.points_id)
        .cloned()
        .unwrap_or_default();
    let table = rules::standings(&points, races);
    let rank_of = |who: Entrant| {
        table
            .iter()
            .find(|s| s.entrant == who && s.races > 0)
            .map(|s| s.rank)
    };
    payout.position = rank_of(Entrant::Player);
    if let Some(pos) = payout.position {
        payout.prize = rules::prize(event, pos);
        let pot = *career
            .reputation_pots
            .entry(event.id)
            .or_insert(event.reputation_pot);
        payout.reputation_base = rules::reputation_base(&catalogue.reputation_fractions, pot, pos);
        let factor = rules::bonus_factor(catalogue, tier.rank_id, settings);
        payout.reputation_bonus = rules::reputation_bonus(payout.reputation_base, factor);
        career.reputation_pots.insert(
            event.id,
            (pot - payout.reputation_base).max(event.reputation_pot_min),
        );
        if let (Some(tm), Some(tm_pos)) = (&career.team_mate, rank_of(Entrant::TeamMate)) {
            payout.team_mate_prize =
                rules::teammate_share(rules::prize(event, tm_pos), tm.winnings_percentage);
        }
        if rules::player_team_first(&table) {
            payout.team_win_bonus = catalogue
                .team_win_rewards
                .get(&tier.rank_id)
                .copied()
                .unwrap_or(0);
        }
        for offer in rules::unlocked_offers(event, pos) {
            if !career.sponsor_offers.contains(&offer.id) {
                career.sponsor_offers.push(offer.id);
                payout.new_sponsor_offers.push(offer.id);
            }
        }
        let best = career.event_results.entry(event.id).or_insert(pos);
        *best = (*best).min(pos);
        career.update_head_to_head(catalogue, tier.id);
    }
    career.count_in_season(SeasonEntry::Event(event.id));
    career.reputation[tier.region as usize] += payout.reputation();
    career.season_earnings += payout.money();
    payout.team_founded = career.add_money(catalogue, payout.money());
    Ok(payout)
}

/// Pays a driver offer for another team (0x674940, 0x652270) [V]: the
/// appearance fee (`per_race_wage × races`, race offers only) and, when the
/// objective was met, `objective_money_bonus` and `objective_reputation_bonus`.
/// [I] No event prize or reputation pot is paid for an offer race.
pub fn apply_driver_offer(
    catalogue: &Catalogue,
    career: &mut Career,
    offer_id: i32,
    objective_met: bool,
) -> Result<Payout, String> {
    let offer = catalogue
        .driver_offers
        .get(&offer_id)
        .ok_or(format!("driver offer {offer_id} absent"))?;
    let tier = catalogue
        .tier(offer.tier_id)
        .ok_or(format!("tier {} absent", offer.tier_id))?;
    let test = rules::is_test_offer(offer);
    let mut payout = Payout {
        region: Some(tier.region),
        ..Payout::default()
    };
    if !test {
        payout.driver_offer_wages = rules::appearance_fee(offer);
    }
    if objective_met {
        payout.driver_offer_bonus = offer.objective_money_bonus;
        payout.driver_offer_reputation = offer.objective_reputation_bonus;
    }
    career.reputation[tier.region as usize] += payout.reputation();
    career.completed_offers.insert(offer_id);
    career.count_in_season(SeasonEntry::DriverOffer(offer_id));
    career.season_earnings += payout.money();
    payout.team_founded = career.add_money(catalogue, payout.money());
    Ok(payout)
}

/// A one-car result helper for tests and simple callers.
pub fn result(order: &[Entrant]) -> RaceResult {
    RaceResult {
        order: order
            .iter()
            .map(|&entrant| Classified {
                entrant,
                finished: true,
                undamaged: true,
            })
            .collect(),
    }
}
