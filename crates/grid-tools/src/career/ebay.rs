//! The eBay Motors used-car market of the career (docs/CAREER_RESEARCH.md
//! §13; static analysis of the eBay manager, ctor 0x675210) [V unless
//! marked]:
//! - buying: for each car model, `maxNumOfCarsOnAuction` (3) used cars at a
//!   fixed price, generated from a per-model seed (0x668BB0, 0x653360); the
//!   player never bids;
//! - selling: the player lists an owned car for 1..4 career events; each
//!   event the bid rises by a fixed step of the car's value (0x64E760).
//!
//! Two MT19937 generators with the original's seeding ([`Mt`]): #1 builds
//! offers, the sale-value noise and the buyers; #2 draws the per-model
//! seeds. Not ported: what the bought car's performance and damageability
//! percentages change in the physics (written at race setup 0x5F8940 to
//! fields whose meaning was not traced).
use super::data::Catalogue;
use crate::series::Mt;
use std::collections::BTreeMap;

/// `gameconfig/ebay.xml` (`EbaySettings`, parser 0x631E60) [D values].
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub max_cars_on_auction: u32,
    pub min_mileage: i32,
    pub max_mileage: i32,
    pub min_buy_it_now_percent: i32,
    pub max_buy_it_now_percent: i32,
    pub min_events: u32,
    pub max_events: u32,
    /// Parsed but never read by the original [V by absence].
    pub winning_chance_percent: i32,
    pub major_crash_chance_percent: u32,
    pub minor_crash_chance_percent: u32,
    pub min_success_wins_fraction: f32,
    pub max_success_wins_fraction: f32,
    pub min_performance_inc_percent: i32,
    pub max_performance_inc_percent: i32,
    pub min_unsuccess_wins_fraction: f32,
    pub max_unsuccess_wins_fraction: f32,
    pub min_performance_dec_percent: i32,
    pub max_performance_dec_percent: i32,
    pub min_major_damage_fraction: f32,
    pub max_major_damage_fraction: f32,
    pub min_damageability_inc_percent: i32,
    pub max_damageability_inc_percent: i32,
    pub max_auction_length: i32,
    pub min_potential_value_percent: i32,
    pub max_potential_value_percent: i32,
    pub min_potential_value_variation_percent: i32,
    pub max_potential_value_variation_percent: i32,
    pub starting_bet_percent: i32,
    pub progress_step_percent: i32,
    pub wins_for_one_percent: f32,
    pub crashes_for_one_percent: f32,
    pub miles_for_one_percent: f32,
}

impl Default for Settings {
    /// The shipped `ebay.xml` [D].
    fn default() -> Self {
        Self {
            max_cars_on_auction: 3,
            min_mileage: 500,
            max_mileage: 50000,
            min_buy_it_now_percent: 88,
            max_buy_it_now_percent: 100,
            min_events: 10,
            max_events: 100,
            winning_chance_percent: 10,
            major_crash_chance_percent: 3,
            minor_crash_chance_percent: 40,
            min_success_wins_fraction: 0.3,
            max_success_wins_fraction: 0.7,
            min_performance_inc_percent: 1,
            max_performance_inc_percent: 5,
            min_unsuccess_wins_fraction: 0.01,
            max_unsuccess_wins_fraction: 0.2,
            min_performance_dec_percent: 1,
            max_performance_dec_percent: 10,
            min_major_damage_fraction: 0.01,
            max_major_damage_fraction: 0.5,
            min_damageability_inc_percent: 1,
            max_damageability_inc_percent: 10,
            max_auction_length: 4,
            min_potential_value_percent: 70,
            max_potential_value_percent: 130,
            min_potential_value_variation_percent: -10,
            max_potential_value_variation_percent: 0,
            starting_bet_percent: 70,
            progress_step_percent: 10,
            wins_for_one_percent: 0.33,
            crashes_for_one_percent: 3.0,
            miles_for_one_percent: 500.0,
        }
    }
}

impl Settings {
    /// The values of a decoded `ebay.xml` (element `EbaySettings`); absent
    /// attributes keep the shipped values.
    pub fn from_document(doc: &grid_formats::binxml::Document<'_>) -> Result<Self, String> {
        let e = doc
            .elements
            .iter()
            .find(|e| e.name == "EbaySettings")
            .ok_or("ebay.xml has no EbaySettings element")?;
        let mut s = Self::default();
        let get = |name: &str| e.attributes.iter().find(|(k, _)| *k == name).map(|(_, v)| v.trim());
        macro_rules! fields {
            ($($attr:literal => $field:ident),* $(,)?) => {$(
                if let Some(v) = get($attr) {
                    s.$field = v.parse().map_err(|_| format!("ebay.xml {}={v:?}", $attr))?;
                }
            )*};
        }
        fields!(
            "maxNumOfCarsOnAuction" => max_cars_on_auction,
            "minMillage" => min_mileage,
            "maxMillage" => max_mileage,
            "minBuyItNowPricePercentage" => min_buy_it_now_percent,
            "maxBuyItNowPricePercentage" => max_buy_it_now_percent,
            "minNumOfEvents" => min_events,
            "maxNumOfEvents" => max_events,
            "winningChancePercentage" => winning_chance_percent,
            "majorCrashChancePercentage" => major_crash_chance_percent,
            "minorCrashChancePercentage" => minor_crash_chance_percent,
            "minSuccesWinsFraction" => min_success_wins_fraction,
            "maxSuccesWinsFraction" => max_success_wins_fraction,
            "minPerformanceIncPercentage" => min_performance_inc_percent,
            "maxPerformanceIncPercentage" => max_performance_inc_percent,
            "minUnSuccesWinsFraction" => min_unsuccess_wins_fraction,
            "maxUnSuccesWinsFraction" => max_unsuccess_wins_fraction,
            "minPerformanceDecPercentage" => min_performance_dec_percent,
            "maxPerformanceDecPercentage" => max_performance_dec_percent,
            "minMajorDamageFraction" => min_major_damage_fraction,
            "maxMajorDamageFraction" => max_major_damage_fraction,
            "minDamagabilityIncPercentage" => min_damageability_inc_percent,
            "maxDamagabilityIncPercentage" => max_damageability_inc_percent,
            "maxAuctionLength" => max_auction_length,
            "minPotentialAuctionValuePercentage" => min_potential_value_percent,
            "maxPotentialAuctionValuePercentage" => max_potential_value_percent,
            "minPotentialAuctionValueVariationPercentage" => min_potential_value_variation_percent,
            "maxPotentialAuctionValueVariationPercentage" => max_potential_value_variation_percent,
            "startingPotentialAuctionValuePercentageBet" => starting_bet_percent,
            "auctionProgressPercentageStep" => progress_step_percent,
            "numOfWinsForOnePercent" => wins_for_one_percent,
            "numOfCrashesForOnePercent" => crashes_for_one_percent,
            "numOfMilesForOnePercent" => miles_for_one_percent,
        );
        Ok(s)
    }
}

/// `int(lo, hi)` (0xA68410): `lo == hi ? hi : lo + genrand % (hi - lo)`,
/// unsigned, `hi` exclusive.
fn int(rng: &mut Mt, lo: u32, hi: u32) -> u32 {
    if lo == hi { hi } else { lo.wrapping_add(rng.next_u32() % hi.wrapping_sub(lo)) }
}

/// `float(lo, hi)` (0xA683B0): `lo + genrand · 2⁻³² · (hi − lo)` (x87;
/// the port computes in f64).
fn float(rng: &mut Mt, lo: f64, hi: f64) -> f64 {
    lo + rng.next_u32() as f64 * (1.0 / 4_294_967_296.0) * (hi - lo)
}

/// The currency rounding 0x639A80: prices floor to a multiple of
/// `50000 / gcd(round(conversion_rate × 100))` over the `currency` rows
/// (1000 with the shipped rates) [V, D].
pub fn currency_round(catalogue: &Catalogue, price: i64) -> i64 {
    fn gcd(a: i64, b: i64) -> i64 {
        if b == 0 { a } else { gcd(b, a % b) }
    }
    let g = catalogue
        .currency_rates
        .iter()
        .map(|r| (r * 100.0).round() as i64)
        .filter(|&v| v > 0)
        .fold(0, gcd);
    if g <= 0 {
        return price;
    }
    let step = (50000 / g).max(1);
    price - price.rem_euclid(step)
}

/// One used car on offer (record of 0x653360).
#[derive(Clone, Debug, PartialEq)]
pub struct UsedCar {
    pub vehicle_id: i32,
    /// The seller, a hireable driver (0x64E4D0); `None` for the price-only
    /// generation of 0x65F740.
    pub seller: Option<i32>,
    pub events: u32,
    pub miles: f32,
    pub event_wins: u32,
    pub race_wins: u32,
    /// Shown as "Wrecked".
    pub major_crashes: u32,
    pub minor_crashes: u32,
    pub performance_percent: f32,
    pub damageability_percent: f32,
    /// Buy It Now, after the currency rounding.
    pub price: i64,
    /// [I] the listing number.
    pub number: u32,
}

/// A bought eBay car's modifiers (manager +0x1488, 12-byte records).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Modifier {
    pub vehicle_id: i32,
    pub performance_percent: f32,
    pub damageability_percent: f32,
}

/// A sale listing's state (+0x28) and its popup [D strings].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListingState {
    /// Listed, no event run yet (AUTHORED: the original's initial value
    /// was not traced).
    Listed = 0,
    /// `ebay_msg_bidinp`: a bid in progress.
    Bidding = 1,
    /// `ebay_msg_fixmet`: sold at the fixed price.
    FixedPriceMet = 2,
    /// `ebay_msg_bidmet`: ended, sold at the bid.
    Sold = 3,
    /// `ebay_msg_endnobid`: ended unsold; the car stays.
    Unsold = 4,
    /// `ebay_msg_nobids`: no bid at or above the start price yet.
    NoBids = 5,
}

impl ListingState {
    fn from_u8(v: u8) -> Option<Self> {
        [
            Self::Listed,
            Self::Bidding,
            Self::FixedPriceMet,
            Self::Sold,
            Self::Unsold,
            Self::NoBids,
        ]
        .into_iter()
        .find(|s| *s as u8 == v)
    }
}

/// One of the player's cars on sale (0x34-byte listing, 0x632240).
#[derive(Clone, Debug, PartialEq)]
pub struct Listing {
    pub vehicle_id: i32,
    /// Events, `min(chosen, maxAuctionLength)`.
    pub duration: i32,
    pub remaining: i32,
    pub start_price: i64,
    pub fixed_price: i64,
    /// The car's value (0x631B60), redrawn each event.
    pub value: i64,
    pub bid: i64,
    pub reserve_met: bool,
    pub fixed_met: bool,
    /// The bidder, a hireable driver id (0 none).
    pub buyer: i32,
    pub state: ListingState,
}

/// The generators and the per-model seeds; not saved (the original
/// reseeds at load and replays the seed refreshes).
#[derive(Clone)]
struct Rngs {
    offers: Mt,
    seeds: Mt,
    map: BTreeMap<i32, u32>,
    /// False after a load until the refreshes are replayed.
    ready: bool,
}

impl std::fmt::Debug for Rngs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Rngs({} seeds)", self.map.len())
    }
}

impl PartialEq for Rngs {
    /// Generator states are not part of the saved state.
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Rngs {
    /// Reset 0x653B10.
    fn reset() -> Self {
        Self {
            offers: Mt::new(0x6529a9),
            seeds: Mt::new(0x221c63),
            map: BTreeMap::new(),
            ready: true,
        }
    }
}

/// The eBay manager's state (`EBAY_MANAGER_SAVE`, 0x64E3F0).
#[derive(Clone, Debug, PartialEq)]
pub struct Ebay {
    /// +0x14AC: cars sold.
    pub sold: u32,
    /// +0x14B0: money spent buying.
    pub spent: i64,
    /// +0x14B4: money earned selling (`fact_ebay_earnings`).
    pub earned: i64,
    /// +0x14B8: seed refreshes, replayed at load.
    pub refreshes: u32,
    pub modifiers: Vec<Modifier>,
    pub listings: Vec<Listing>,
    rng: Rngs,
}

impl Default for Ebay {
    fn default() -> Self {
        Self::new()
    }
}

/// What a career event did to one listing (the popups of 0x4EECE0).
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub listing: Listing,
    /// Money paid for a completed sale (states 2 and 3).
    pub paid: Option<i64>,
}

impl Ebay {
    /// A new manager (Reset 0x653B10).
    pub fn new() -> Self {
        Self {
            sold: 0,
            spent: 0,
            earned: 0,
            refreshes: 0,
            modifiers: Vec::new(),
            listings: Vec::new(),
            rng: Rngs::reset(),
        }
    }

    /// A loaded state (0x668D20): the generators restart from the reset
    /// seeds on first use and the seed refreshes are replayed, `refreshes`
    /// times or once when 0, so the same offers come back.
    pub fn loaded(&mut self) {
        self.rng.ready = false;
    }

    fn ensure(&mut self, catalogue: &Catalogue) {
        if self.rng.ready {
            return;
        }
        self.rng = Rngs::reset();
        let n = self.refreshes.max(1);
        self.refreshes = 0;
        for _ in 0..n {
            self.refresh(catalogue);
        }
    }

    /// 0x65E9E0: one seed `int(0, 0xFFFF)` per `vehicle` row, in table
    /// order (= id order of the shipped table [D]).
    pub fn refresh_seeds(&mut self, catalogue: &Catalogue) {
        self.ensure(catalogue);
        self.refresh(catalogue);
    }

    fn refresh(&mut self, catalogue: &Catalogue) {
        self.refreshes += 1;
        self.rng.map.clear();
        for &id in catalogue.vehicles.keys() {
            let seed = int(&mut self.rng.seeds, 0, 0xffff);
            self.rng.map.insert(id, seed);
        }
    }

    fn seed(&mut self, catalogue: &Catalogue, vehicle_id: i32) -> u32 {
        self.ensure(catalogue);
        if self.rng.map.is_empty() || self.refreshes == 0 {
            self.refresh(catalogue);
        }
        self.rng.map.get(&vehicle_id).copied().unwrap_or(0)
    }

    /// The used cars on offer for a model (0x668BB0): generator #1 seeded
    /// from the model's seed, then `maxNumOfCarsOnAuction` cars. The same
    /// cars come back until the next seed refresh.
    pub fn offers(&mut self, catalogue: &Catalogue, vehicle_id: i32, team_mate: Option<i32>) -> Vec<UsedCar> {
        let seed = self.seed(catalogue, vehicle_id);
        self.rng.offers = Mt::new(seed);
        (0..catalogue.ebay.max_cars_on_auction)
            .map(|_| generate(catalogue, &mut self.rng.offers, vehicle_id, true, team_mate, None))
            .collect()
    }

    /// The lowest eBay price of a model (0x65F740), as on the event's car
    /// list.
    pub fn cheapest(&mut self, catalogue: &Catalogue, vehicle_id: i32) -> Option<i64> {
        let seed = self.seed(catalogue, vehicle_id);
        self.rng.offers = Mt::new(seed);
        (0..catalogue.ebay.max_cars_on_auction)
            .map(|_| generate(catalogue, &mut self.rng.offers, vehicle_id, false, None, None).price)
            .min()
    }

    /// The bought car's modifiers, if it came from eBay (0x64E390).
    pub fn modifier(&self, vehicle_id: i32) -> Option<&Modifier> {
        self.modifiers.iter().find(|m| m.vehicle_id == vehicle_id)
    }
}

/// A random hireable driver (0x64E4D0): one `int(0, count)` draw over the
/// `can_be_hired` drivers in table order; the hired team mate moves the
/// pick to the next row, or the previous one at the end [I from the
/// pseudocode].
fn random_driver(catalogue: &Catalogue, rng: &mut Mt, team_mate: Option<i32>) -> i32 {
    let n = catalogue.hireable.len() as u32;
    if n == 0 {
        return 0;
    }
    let mut k = int(rng, 0, n) as usize;
    if Some(catalogue.hireable[k].driver_id) == team_mate {
        k = if k + 1 < n as usize { k + 1 } else { k.saturating_sub(1) };
    }
    catalogue.hireable[k].driver_id
}

/// One used car (0x653360), all draws on generator #1 in the original's
/// order. `base_price` replaces `vehicle.purchase_price` when given.
pub fn generate(
    catalogue: &Catalogue,
    rng: &mut Mt,
    vehicle_id: i32,
    with_seller: bool,
    team_mate: Option<i32>,
    base_price: Option<i64>,
) -> UsedCar {
    let s = &catalogue.ebay;
    // 1. The seller, or one draw to keep the sequence.
    let seller = if with_seller {
        Some(random_driver(catalogue, rng, team_mate))
    } else {
        int(rng, 0, 1);
        None
    };
    // 2.–5. History.
    let events = int(rng, s.min_events, s.max_events + 1);
    let m = int(rng, 250, 500);
    let miles = m as f32 * events as f32;
    let u = float(rng, 0.0, 0.5);
    let event_wins = (u * events as f64) as u32;
    let race_wins = int(rng, 1, 6) * event_wins;
    // 6.–7. Crashes: one draw per event each.
    let mut major = 0;
    for _ in 0..events {
        major += u32::from(int(rng, 0, 101) < s.major_crash_chance_percent);
    }
    let major = major.min(events - event_wins);
    let mut minor = 0;
    for _ in 0..events {
        minor += u32::from(int(rng, 0, 101) < s.minor_crash_chance_percent);
    }
    // 8. Performance from the win fraction.
    let f = event_wins as f32 / events as f32;
    let performance = if f > s.min_success_wins_fraction {
        let slope = (s.max_performance_inc_percent - s.min_performance_inc_percent) as f32
            / (s.max_success_wins_fraction - s.min_success_wins_fraction);
        (s.min_performance_inc_percent as f32 + (f - s.min_success_wins_fraction) * slope)
            .min(s.max_performance_inc_percent as f32)
    } else if f < s.max_unsuccess_wins_fraction {
        let slope = (s.max_performance_dec_percent - s.min_performance_dec_percent) as f32
            / (s.max_unsuccess_wins_fraction - s.min_unsuccess_wins_fraction);
        -(s.min_performance_dec_percent as f32 + (s.max_unsuccess_wins_fraction - f) * slope)
            .min(s.max_performance_dec_percent as f32)
    } else {
        0.0
    };
    // 9. Damageability from the major-crash fraction. The original's slope
    // numerator is maxDamagabilityIncPercentage − minMajorDamageFraction
    // [V 0x6536C2..0x653710], kept.
    let g = major as f32 / events as f32;
    let damageability = if g > s.min_major_damage_fraction {
        let slope = (s.max_damageability_inc_percent as f32 - s.min_major_damage_fraction)
            / (s.max_major_damage_fraction - s.min_major_damage_fraction);
        (s.min_damageability_inc_percent as f32 + (g - s.min_major_damage_fraction) * slope)
            .min(s.max_damageability_inc_percent as f32)
    } else {
        0.0
    };
    // 10. Buy It Now: mileage percent plus wins, less crashes (not clamped).
    let a = s.max_buy_it_now_percent as f64
        - (miles as f64 - s.min_mileage as f64) * (s.max_buy_it_now_percent - s.min_buy_it_now_percent) as f64
            / (s.max_mileage - s.min_mileage) as f64;
    let p = ((a + s.wins_for_one_percent as f64 * event_wins as f64
        - s.crashes_for_one_percent as f64 * major as f64)
        * 0.01f32 as f64) as f32;
    let base = base_price.unwrap_or_else(|| catalogue.vehicles.get(&vehicle_id).map_or(0, |v| v.purchase_price));
    let price = currency_round(catalogue, (base as f64 * p as f64) as i64);
    // 11.
    let number = int(rng, 0, 100_000);
    UsedCar {
        vehicle_id,
        seller,
        events,
        miles,
        event_wins,
        race_wins,
        major_crashes: major,
        minor_crashes: minor,
        performance_percent: performance,
        damageability_percent: damageability,
        price,
        number,
    }
}

/// An owned car's eBay value (0x631B60): wins add, crashes and mileage
/// take off, clamped to 70..130 % of `purchase_price`, then a noise of
/// −10..0 % from generator #1. No currency rounding.
pub fn sale_value(catalogue: &Catalogue, rng: &mut Mt, vehicle_id: i32, race_wins: u32, major_crashes: u32, miles: f64) -> i64 {
    let s = &catalogue.ebay;
    let v = race_wins as f64 * 0.6f32 as f64 / s.wins_for_one_percent as f64
        - major_crashes as f64 * 10.0f32 as f64 / s.crashes_for_one_percent as f64
        - miles * 0.5 / s.miles_for_one_percent as f64;
    let x = (v + 100.0).clamp(s.min_potential_value_percent as f64, s.max_potential_value_percent as f64);
    let noise = float(
        rng,
        s.min_potential_value_variation_percent as f64,
        s.max_potential_value_variation_percent as f64,
    );
    let price = catalogue.vehicles.get(&vehicle_id).map_or(0, |v| v.purchase_price);
    ((noise + x) * price as f64 * 0.01f32 as f64) as i64
}

impl super::Career {
    /// Buys a used car (0x65F540): the model must not be owned and the
    /// price affordable; the car joins the garage with its history, its
    /// modifiers are kept, and every model's offers change.
    pub fn buy_used(&mut self, catalogue: &Catalogue, car: &UsedCar) -> Result<(), String> {
        if self.cars.iter().any(|c| c.vehicle_id == car.vehicle_id) {
            return Err("you already own this model".into());
        }
        if car.price > self.money {
            return Err("insufficient funds".into());
        }
        self.money -= car.price;
        self.cars.push(super::state::OwnedCar {
            vehicle_id: car.vehicle_id,
            livery: 0,
            race_wins: car.race_wins,
            event_wins: car.event_wins,
            major_crashes: car.major_crashes,
            odometer_m: car.miles as f64 * 1.609344 * 1000.0,
        });
        self.ebay.spent += car.price;
        self.ebay.modifiers.push(Modifier {
            vehicle_id: car.vehicle_id,
            performance_percent: car.performance_percent,
            damageability_percent: car.damageability_percent,
        });
        self.ebay.refresh_seeds(catalogue);
        Ok(())
    }

    /// Lists an owned car for sale (0x65F390): `duration` is capped at
    /// `maxAuctionLength`; a car already listed is refused. AUTHORED: the
    /// port has no current-car rule (the original refuses the car in use).
    pub fn list_on_ebay(
        &mut self,
        catalogue: &Catalogue,
        index: usize,
        start_price: i64,
        fixed_price: i64,
        duration: i32,
    ) -> Result<(), String> {
        let car = self.cars.get(index).ok_or("no such car")?.clone();
        self.ebay.ensure(catalogue);
        if self.ebay.listings.iter().any(|l| l.vehicle_id == car.vehicle_id) {
            return Err("this car is already listed".into());
        }
        let duration = duration.clamp(1, catalogue.ebay.max_auction_length.max(1));
        let value = sale_value(
            catalogue,
            &mut self.ebay.rng.offers,
            car.vehicle_id,
            car.race_wins,
            car.major_crashes,
            car.odometer_m / 1609.344,
        );
        self.ebay.listings.push(Listing {
            vehicle_id: car.vehicle_id,
            duration,
            remaining: duration,
            start_price,
            fixed_price: fixed_price.max(start_price),
            value,
            bid: 0,
            reserve_met: false,
            fixed_met: false,
            buyer: 0,
            state: ListingState::Listed,
        });
        Ok(())
    }

    /// After a career event (0x587580 → 0x65ABA0, 0x65E9E0) [I: once per
    /// event]: each listing's value is redrawn and its bid steps up; ended
    /// sales complete (0x4EECE0: the original completes them when the popup
    /// is acknowledged, the port at once); then every model's offers change.
    pub fn ebay_after_event(&mut self, catalogue: &Catalogue) -> Vec<Outcome> {
        self.ebay.ensure(catalogue);
        let team_mate = self.team_mate.as_ref().map(|t| t.driver_id);
        let s = catalogue.ebay.clone();
        let mut outcomes = Vec::new();
        let mut listings = std::mem::take(&mut self.ebay.listings);
        for l in &mut listings {
            // 0x656EF0: history from the garage car, then the value.
            if let Some(car) = self.cars.iter().find(|c| c.vehicle_id == l.vehicle_id) {
                l.value = sale_value(
                    catalogue,
                    &mut self.ebay.rng.offers,
                    car.vehicle_id,
                    car.race_wins,
                    car.major_crashes,
                    car.odometer_m / 1609.344,
                );
            }
            step(catalogue, &s, &mut self.ebay.rng.offers, l, team_mate);
        }
        for l in listings {
            let paid = match l.state {
                ListingState::FixedPriceMet => Some(l.fixed_price),
                ListingState::Sold => Some(l.bid),
                _ => None,
            };
            if let Some(amount) = paid {
                // 0x655F60.
                self.ebay.modifiers.retain(|m| m.vehicle_id != l.vehicle_id);
                self.cars.retain(|c| c.vehicle_id != l.vehicle_id);
                self.money += amount;
                self.ebay.earned += amount;
                self.ebay.sold += 1;
            } else if l.state != ListingState::Unsold {
                self.ebay.listings.push(l.clone());
            }
            outcomes.push(Outcome { listing: l, paid });
        }
        self.ebay.refresh_seeds(catalogue);
        outcomes
    }
}

/// One event of a sale (0x64E760): `bid = trunc((k·step + startingBet) ·
/// value · 0.01f)` for k = 0.. (70, 80, 90, 100 %), floored to the currency
/// step; below the start price there is no bid; at or above the fixed price
/// the car sells at the fixed price.
fn step(catalogue: &Catalogue, s: &Settings, rng: &mut Mt, l: &mut Listing, team_mate: Option<i32>) {
    let mut has_bid = true;
    if l.remaining < 1 {
        l.remaining = 0;
    } else {
        l.remaining -= 1;
        let k = (l.duration - l.remaining - 1) as f64;
        let bid = (l.value as f64
            * (k * s.progress_step_percent as f64 + s.starting_bet_percent as f64)
            * 0.01f32 as f64) as i64;
        l.bid = currency_round(catalogue, bid);
        if l.bid < l.start_price {
            l.bid = 0;
            has_bid = false;
        }
        l.buyer = random_driver(catalogue, rng, team_mate);
        if l.bid >= l.start_price {
            l.reserve_met = true;
        }
        if l.bid >= l.fixed_price {
            l.fixed_met = true;
            l.bid = l.fixed_price;
        }
    }
    l.state = if l.fixed_met {
        ListingState::FixedPriceMet
    } else if l.remaining == 0 {
        if l.reserve_met { ListingState::Sold } else { ListingState::Unsold }
    } else if !has_bid {
        ListingState::NoBids
    } else {
        ListingState::Bidding
    };
}

/// Save lines of the eBay state (the port's save, docs/CAREER_RESEARCH.md
/// §9.4).
pub fn write(e: &Ebay, put: &mut impl FnMut(String)) {
    put(format!("ebay {} {} {} {}", e.sold, e.spent, e.earned, e.refreshes));
    for m in &e.modifiers {
        put(format!("ebay_car {} {:?} {:?}", m.vehicle_id, m.performance_percent, m.damageability_percent));
    }
    for l in &e.listings {
        put(format!(
            "ebay_listing {} {} {} {} {} {} {} {} {} {} {}",
            l.vehicle_id,
            l.duration,
            l.remaining,
            l.start_price,
            l.fixed_price,
            l.value,
            l.bid,
            u8::from(l.reserve_met),
            u8::from(l.fixed_met),
            l.buyer,
            l.state as u8
        ));
    }
}

/// Reads one eBay save line into `e`; `false` when the key is not eBay's.
pub fn read(e: &mut Ebay, key: &str, rest: &str) -> Result<bool, String> {
    let f: Vec<&str> = rest.split(' ').collect();
    fn n<T: std::str::FromStr>(f: &[&str], i: usize, key: &str) -> Result<T, String> {
        f.get(i).and_then(|v| v.parse().ok()).ok_or(format!("bad {key} line"))
    }
    match key {
        "ebay" => {
            e.sold = n(&f, 0, key)?;
            e.spent = n(&f, 1, key)?;
            e.earned = n(&f, 2, key)?;
            e.refreshes = n(&f, 3, key)?;
        }
        "ebay_car" => e.modifiers.push(Modifier {
            vehicle_id: n(&f, 0, key)?,
            performance_percent: n(&f, 1, key)?,
            damageability_percent: n(&f, 2, key)?,
        }),
        "ebay_listing" => e.listings.push(Listing {
            vehicle_id: n(&f, 0, key)?,
            duration: n(&f, 1, key)?,
            remaining: n(&f, 2, key)?,
            start_price: n(&f, 3, key)?,
            fixed_price: n(&f, 4, key)?,
            value: n(&f, 5, key)?,
            bid: n(&f, 6, key)?,
            reserve_met: n::<u8>(&f, 7, key)? != 0,
            fixed_met: n::<u8>(&f, 8, key)? != 0,
            buyer: n(&f, 9, key)?,
            state: ListingState::from_u8(n(&f, 10, key)?).ok_or("bad listing state")?,
        }),
        _ => return Ok(false),
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::career::Career;
    use crate::career::data::{Hireable, Vehicle};

    fn catalogue() -> Catalogue {
        let mut c = Catalogue {
            currency_rates: vec![1.0, 2.0, 1.5, 230.0],
            ..Catalogue::default()
        };
        for (id, price) in [(3, 60000), (7, 120000), (43, 30000)] {
            c.vehicles.insert(
                id,
                Vehicle {
                    id,
                    stem: format!("car{id}"),
                    region: None,
                    class_id: 1,
                    purchasable: true,
                    purchase_price: price,
                    sale_price: price / 2,
                    can_enter_le_mans: false,
                },
            );
        }
        for driver_id in [200, 201, 202] {
            c.hireable.push(Hireable {
                driver_id,
                first_name: "A".into(),
                surname: "B".into(),
                rank_id: 1,
                region_id: 1,
                signing_fee: 0,
                winnings_percentage: 0,
                skill: 0.5,
                team_id: 1,
            });
        }
        c
    }

    fn listing(duration: i32, start: i64, fixed: i64) -> Listing {
        Listing {
            vehicle_id: 7,
            duration,
            remaining: duration,
            start_price: start,
            fixed_price: fixed,
            value: 100_000,
            bid: 0,
            reserve_met: false,
            fixed_met: false,
            buyer: 0,
            state: ListingState::Listed,
        }
    }

    #[test]
    fn prices_floor_to_the_currency_step() {
        // gcd(100, 200, 150, 23000) = 50: 50000 / 50 = 1000 [D].
        assert_eq!(currency_round(&catalogue(), 123_456), 123_000);
        assert_eq!(currency_round(&catalogue(), 999), 0);
    }

    #[test]
    fn used_cars_follow_the_history_formulas() {
        let cat = catalogue();
        let mut rng = Mt::new(1234);
        for _ in 0..200 {
            let car = generate(&cat, &mut rng, 7, true, None, None);
            assert!((10..=100).contains(&car.events));
            assert!(car.miles >= 2500. && car.miles <= 49900.);
            assert!(car.event_wins * 2 <= car.events);
            assert!(car.race_wins.is_multiple_of(car.event_wins.max(1)) && car.race_wins <= 5 * car.event_wins);
            assert!(car.major_crashes <= car.events - car.event_wins);
            // Performance from the win fraction.
            let f = car.event_wins as f32 / car.events as f32;
            let perf = if f > 0.3 {
                (1. + (f - 0.3) * (4. / (0.7f32 - 0.3))).min(5.)
            } else if f < 0.2 {
                -(1. + (0.2 - f) * (9. / (0.2f32 - 0.01))).min(10.)
            } else {
                0.
            };
            assert_eq!(car.performance_percent, perf);
            // The price from the history: mileage, wins and crashes.
            let a = 100. - (car.miles as f64 - 500.) * 12. / 49500.;
            let p = ((a + 0.33f32 as f64 * car.event_wins as f64 - 3. * car.major_crashes as f64)
                * 0.01f32 as f64) as f32;
            let price = (120000. * p as f64) as i64;
            assert_eq!(car.price, price - price % 1000);
            assert!(car.seller.is_some_and(|d| (200..=202).contains(&d)));
        }
    }

    #[test]
    fn the_damageability_slope_keeps_the_original_quirk() {
        // g = 0.5: 1 + 0.49 × (10 − 0.01)/0.49 = 10.99, capped at 10;
        // g = 0.05: 1 + 0.04 × 9.99/0.49 = 1.8155.
        let s = Settings::default();
        let slope = (s.max_damageability_inc_percent as f32 - s.min_major_damage_fraction)
            / (s.max_major_damage_fraction - s.min_major_damage_fraction);
        let at = |g: f32| (1. + (g - 0.01) * slope).min(10.);
        assert_eq!(at(0.5), 10.);
        assert!((at(0.05) - 1.815_510_2).abs() < 1e-5);
    }

    #[test]
    fn offers_stay_until_the_seeds_refresh_and_survive_a_load() {
        let cat = catalogue();
        let mut c = Career::new();
        let first = c.ebay.offers(&cat, 7, None);
        assert_eq!(first.len(), 3);
        assert_eq!(c.ebay.offers(&cat, 7, None), first);
        assert_eq!(c.ebay.cheapest(&cat, 7), first.iter().map(|o| o.price).min());
        // A save and load replays the refreshes: the same offers.
        let mut back = crate::career::save::read(&crate::career::save::write(&c).unwrap()).unwrap();
        assert_eq!(back.ebay.offers(&cat, 7, None), first);
        c.ebay.refresh_seeds(&cat);
        assert_ne!(c.ebay.offers(&cat, 7, None), first);
        let mut back = crate::career::save::read(&crate::career::save::write(&c).unwrap()).unwrap();
        assert_eq!(back.ebay.offers(&cat, 7, None), c.ebay.offers(&cat, 7, None));
    }

    #[test]
    fn buying_takes_the_money_and_keeps_the_history() {
        let cat = catalogue();
        let mut c = Career::new();
        c.money = 200_000;
        let car = c.ebay.offers(&cat, 7, None)[0].clone();
        c.buy_used(&cat, &car).unwrap();
        assert_eq!(c.money, 200_000 - car.price);
        assert_eq!((c.ebay.spent, c.ebay.refreshes), (car.price, 2));
        let owned = &c.cars[0];
        assert_eq!((owned.race_wins, owned.major_crashes), (car.race_wins, car.major_crashes));
        assert!((owned.odometer_m - car.miles as f64 * 1609.344).abs() < 1e-6);
        assert_eq!(c.ebay.modifier(7).unwrap().performance_percent, car.performance_percent);
        // One car per model.
        let again = c.ebay.offers(&cat, 7, None)[0].clone();
        assert!(c.buy_used(&cat, &again).is_err());
        let back = crate::career::save::read(&crate::career::save::write(&c).unwrap()).unwrap();
        assert_eq!(back.cars, c.cars);
        assert_eq!(back.ebay.modifiers, c.ebay.modifiers);
    }

    #[test]
    fn a_bid_rises_by_ten_percent_of_the_value_each_event() {
        let cat = catalogue();
        let s = Settings::default();
        let mut rng = Mt::new(1);
        // 70 % · 100,000 · 0.01f = 69,999.998 → 69,999 → 69,000: below the
        // start price, no bid; 80 %, 90 %; 100 % passes the fixed price.
        let mut l = listing(4, 75_000, 95_000);
        let mut seen = Vec::new();
        for _ in 0..4 {
            step(&cat, &s, &mut rng, &mut l, None);
            seen.push((l.bid, l.state));
        }
        assert_eq!(
            seen,
            [
                (0, ListingState::NoBids),
                (79_000, ListingState::Bidding),
                (89_000, ListingState::Bidding),
                (95_000, ListingState::FixedPriceMet),
            ]
        );
        // Without a fixed-price hit the sale ends at the last bid, or unsold.
        let mut l = listing(2, 75_000, 1_000_000);
        step(&cat, &s, &mut rng, &mut l, None);
        step(&cat, &s, &mut rng, &mut l, None);
        assert_eq!((l.bid, l.state), (79_000, ListingState::Sold));
        let mut l = listing(1, 90_000, 1_000_000);
        step(&cat, &s, &mut rng, &mut l, None);
        assert_eq!((l.bid, l.state), (0, ListingState::Unsold));
    }

    /// The purchased data: `ebay.xml` holds the documented values, and a
    /// purchasable model has three offers near or below its new price.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn the_shipped_ebay_settings_and_offers() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"));
        let cat = Catalogue::load(&root).unwrap();
        assert_eq!(cat.ebay, Settings::default());
        assert_eq!(currency_round(&cat, 123_456), 123_000);
        let (&id, v) = cat.vehicles.iter().find(|(_, v)| v.purchasable && v.purchase_price > 0).unwrap();
        let mut c = Career::new();
        let offers = c.ebay.offers(&cat, id, None);
        assert_eq!(offers.len(), 3);
        for o in &offers {
            assert!(o.price > 0 && o.price <= v.purchase_price * 103 / 100, "{o:?}");
            assert!(o.seller.is_some());
        }
    }

    #[test]
    fn a_listed_car_is_sold_after_its_events() {
        let cat = catalogue();
        let mut c = Career::new();
        c.money = 100_000;
        c.buy_car(&cat, 43).unwrap();
        // A new car: value 90..100 % of 30,000; a start of 1,000 sells at
        // the last bid after 4 events (the duration is capped at 4).
        c.list_on_ebay(&cat, 0, 1_000, 1_000_000, 9).unwrap();
        assert_eq!(c.ebay.listings[0].duration, 4);
        assert!(c.list_on_ebay(&cat, 0, 1_000, 1_000_000, 4).is_err());
        let before = c.money;
        let mut paid = None;
        for _ in 0..4 {
            for o in c.ebay_after_event(&cat) {
                paid = paid.or(o.paid);
            }
        }
        let paid = paid.unwrap();
        assert!((27_000..=30_000).contains(&paid), "{paid}");
        assert_eq!(c.money, before + paid);
        assert!(c.cars.is_empty() && c.ebay.listings.is_empty());
        assert_eq!((c.ebay.sold, c.ebay.earned), (1, paid));
    }
}
