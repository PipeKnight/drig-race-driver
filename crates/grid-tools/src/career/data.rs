//! The career catalogue: everything the original's career ("GRID World")
//! reads from `database.bin` [D], as typed rows (docs/CAREER_RESEARCH.md §2).
//!
//! The rows are kept close to the tables; ids are the database ids so that
//! a saved career refers to events, vehicles and drivers the same way the
//! database does.
use grid_formats::database::{Database, Row, Table};
use std::collections::BTreeMap;

/// A data error: a table, row or column the catalogue needs is absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

type Result<T> = std::result::Result<T, Error>;

fn table<'a>(db: &'a Database, name: &str) -> Result<&'a Table> {
    db.table(name)
        .ok_or_else(|| Error(format!("database table {name} absent")))
}

fn int(row: &Row<'_>, column: &str) -> Result<i32> {
    row.int(column)
        .ok_or_else(|| Error(format!("database column {column} absent")))
}

fn float(row: &Row<'_>, column: &str) -> Result<f32> {
    row.float(column)
        .ok_or_else(|| Error(format!("database column {column} absent")))
}

fn text(row: &Row<'_>, column: &str) -> Result<String> {
    row.text(column)
        .map(str::to_owned)
        .ok_or_else(|| Error(format!("database column {column} absent")))
}

/// A comma-separated list such as `event_money_reward.money_string`
/// (`"1.0,0.8, 0.6,0.4,0.2,0.1"`; spaces occur [D]). Empty items are skipped.
pub fn parse_list<T: std::str::FromStr>(list: &str) -> Result<Vec<T>> {
    list.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<T>()
                .map_err(|_| Error(format!("list item {s:?} in {list:?}")))
        })
        .collect()
}

/// `career_region` [D]: 1 europe, 2 united_states, 3 japan, 4 international,
/// 5 le_mans.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Region {
    Europe,
    UnitedStates,
    Japan,
    International,
    LeMans,
}

impl Region {
    pub const ALL: [Region; 5] = [
        Region::Europe,
        Region::UnitedStates,
        Region::Japan,
        Region::International,
        Region::LeMans,
    ];

    pub fn id(self) -> i32 {
        self as i32 + 1
    }

    pub fn from_id(id: i32) -> Result<Self> {
        Self::ALL
            .get((id - 1) as usize)
            .copied()
            .filter(|_| id >= 1)
            .ok_or_else(|| Error(format!("career region id {id}")))
    }

    /// The `career_region.name_string_id`.
    pub fn key(self) -> &'static str {
        ["europe", "united_states", "japan", "international", "le_mans"][self as usize]
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.key() == key)
    }
}

/// `career_rank` [D]: rookie, pro, expert, international, with the
/// reputation each one needs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rank {
    pub id: i32,
    pub name: String,
    pub reputation_to_unlock: i64,
}

/// `tier` rows of the main career (`career_id` 1) [D].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tier {
    pub id: i32,
    pub name: String,
    pub rank_id: i32,
    pub region: Region,
    pub driver_offers: bool,
    pub order_index: i32,
}

/// One race of an event: an `event_track_pool` row [D].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventRace {
    pub id: i32,
    pub track_model_id: i32,
    pub laps: i32,
    pub time_limit_seconds: i32,
    pub order_index: i32,
    pub player_grid_position_base: i32,
    pub player_grid_position_range: i32,
    /// `base_result_list` (drift scores or touge times; empty for races).
    pub base_results: Vec<i64>,
}

/// `sponsor_objective_type` [D].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SponsorObjective {
    /// 1 `sponsor_offer_finishing_place`: finish no lower than N.
    FinishingPlace,
    /// 2 `sponsor_offer_finishing_place_no_damage`.
    FinishingPlaceNoDamage,
    /// 3 `sponsor_offer_finish`.
    Finish,
    /// 4 `sponsor_offer_finish_no_damage`.
    FinishNoDamage,
}

impl SponsorObjective {
    pub fn from_id(id: i32) -> Result<Self> {
        match id {
            1 => Ok(Self::FinishingPlace),
            2 => Ok(Self::FinishingPlaceNoDamage),
            3 => Ok(Self::Finish),
            4 => Ok(Self::FinishNoDamage),
            _ => Err(Error(format!("sponsor objective type {id}"))),
        }
    }

    pub fn id(self) -> i32 {
        self as i32 + 1
    }
}

/// An `event_sponsor_offer` row [D]: the sponsor deal an event can offer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SponsorOffer {
    pub id: i32,
    pub event_id: i32,
    pub sponsor_id: i32,
    pub objective: SponsorObjective,
    /// `objective_details`: the finishing place for the place objectives.
    pub objective_details: i32,
    /// `position_to_unlock`: the event result that unlocks the offer.
    pub position_to_unlock: i32,
    /// `money_reward`: paid per race when the objective is met.
    pub money_reward: i64,
}

/// A career event (`event` row with its races, cars and sponsor offers) [D].
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub id: i32,
    pub name: String,
    pub tier_id: i32,
    pub discipline_id: i32,
    pub competition_format_id: i32,
    pub head_to_head: bool,
    pub use_lemans_selection: bool,
    pub order_index: i32,
    pub num_competitions: i32,
    pub percentage_towards_completion: i32,
    pub prize_money_for_first: i64,
    /// `event_money_reward.money_string` of `money_string_id`: the prize
    /// money fraction per finishing place (index 0 = winner).
    pub money_fractions: Vec<f64>,
    pub money_string_id: i32,
    pub reputation_pot: i64,
    pub reputation_pot_min: i64,
    pub points_id: i32,
    pub sponsor_id: i32,
    pub minor_sponsor_id: i32,
    pub player_livery_number: Option<i32>,
    pub teammate_livery_number: Option<i32>,
    pub races: Vec<EventRace>,
    /// `event_vehicle.vehicle_id` in row order: the cars allowed.
    pub vehicles: Vec<i32>,
    pub sponsor_offers: Vec<SponsorOffer>,
}

/// A `vehicle` row's career fields [D].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vehicle {
    pub id: i32,
    pub stem: String,
    pub region: Option<Region>,
    pub class_id: i32,
    pub purchasable: bool,
    pub purchase_price: i64,
    pub sale_price: i64,
    pub can_enter_le_mans: bool,
}

/// A `sponsor` row [D].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sponsor {
    pub id: i32,
    pub name: String,
    pub file: String,
    pub type_id: i32,
}

/// `driver_offer_objective_type` [D].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OfferObjective {
    /// 1 `race_offer_finishing_place`.
    FinishingPlace,
    /// 2 `race_offer_beat_rival`: finish ahead of a team.
    BeatRival,
    /// 3 `race_offer_no_damage`.
    NoDamage,
    /// 4 `test_offer_time_trial`: lap faster than a target.
    TimeTrial,
    /// 5 `test_offer_speed_test`: exceed a target speed.
    SpeedTest,
    /// 6 `race_offer_finishing_place_le_mans`.
    FinishingPlaceLeMans,
}

impl OfferObjective {
    pub fn from_id(id: i32) -> Result<Self> {
        match id {
            1 => Ok(Self::FinishingPlace),
            2 => Ok(Self::BeatRival),
            3 => Ok(Self::NoDamage),
            4 => Ok(Self::TimeTrial),
            5 => Ok(Self::SpeedTest),
            6 => Ok(Self::FinishingPlaceLeMans),
            _ => Err(Error(format!("driver offer objective type {id}"))),
        }
    }
}

/// A `driver_offer` row with its `driver_offer_track` races [D].
#[derive(Clone, Debug, PartialEq)]
pub struct DriverOffer {
    pub id: i32,
    pub tier_id: i32,
    pub event_id: i32,
    pub team_id: i32,
    pub vehicle_livery_id: i32,
    /// The offered car: `vehicle_livery.vehicle_id` of `vehicle_livery_id` [D].
    pub vehicle_id: Option<i32>,
    /// `vehicle_livery.file_string` (`bmw_02`: car stem and livery) [D].
    pub livery: String,
    pub order_index: i32,
    pub per_race_wage: i64,
    pub objective: OfferObjective,
    pub objective_finishing_place: i32,
    pub objective_team_id: i32,
    pub objective_money_bonus: i64,
    pub objective_reputation_bonus: i64,
    /// `target_time_secs`: the lap time to beat per difficulty star level
    /// 0..5 (six values, e.g. `108, 107, 106, 105, 104, 103`) [D].
    pub target_times: Vec<f32>,
    /// `target_speed_mph`: the speed to exceed per star level [D].
    pub target_speeds_mph: Vec<f32>,
    pub races: Vec<EventRace>,
}

/// A driver the player can hire: a `driver` row with `can_be_hired` (the
/// candidate pool 0x662850, object 0x649320) [V]. The `team_mate` table is
/// not read by the original [V by absence], see [`TeamMate`].
#[derive(Clone, Debug, PartialEq)]
pub struct Hireable {
    pub driver_id: i32,
    pub first_name: String,
    pub surname: String,
    pub rank_id: i32,
    /// `driver.region_id`; 0 (none) for some rows [D].
    pub region_id: i32,
    pub signing_fee: i64,
    pub winnings_percentage: i32,
    /// `driver.skill`: the grid key of the career field (0x5C2E48) [V].
    pub skill: f32,
    pub team_id: i32,
}

/// A `team_mate` row joined to `driver` [D]. Unused by the original (no
/// table-name string in the exe), kept for reference.
#[derive(Clone, Debug, PartialEq)]
pub struct TeamMate {
    pub id: i32,
    pub driver_id: i32,
    pub first_name: String,
    pub surname: String,
    pub rank_id: i32,
    pub region: Region,
    pub signing_fee: i64,
    pub skill: f32,
    /// Share of the teammate's own prize money he keeps, in percent.
    pub winnings_percentage: i32,
}

/// A `difficulty` row [D]: the six star levels per rank.
#[derive(Clone, Debug, PartialEq)]
pub struct Difficulty {
    pub id: i32,
    pub rank_id: i32,
    pub star_level: i32,
    pub ai_ratio: f32,
    pub available_flashbacks: i32,
    pub flashback_bonus: i64,
    pub rep_bonus_factor: f32,
}

/// `gameconfig/rewards.xml` [D]: the reputation bonus, in percent, for each
/// harder setting. The shipped values are the defaults.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssistRewards {
    pub realistic_damage: i32,
    pub fix_head_camera: i32,
    pub pro_mode: i32,
    pub surface_assist: i32,
    pub traction_control: i32,
    pub manual_gears: i32,
    pub steering_assist: i32,
    pub braking_assist: i32,
}

impl Default for AssistRewards {
    fn default() -> Self {
        Self {
            realistic_damage: 5,
            fix_head_camera: 10,
            pro_mode: 20,
            surface_assist: 5,
            traction_control: 5,
            manual_gears: 5,
            steering_assist: 5,
            braking_assist: 5,
        }
    }
}

impl AssistRewards {
    /// The values of a decoded `rewards.xml` (root `rewards`); absent
    /// attributes keep the shipped values.
    pub fn from_document(doc: &grid_formats::binxml::Document<'_>) -> Result<Self> {
        let e = doc
            .elements
            .iter()
            .find(|e| e.name == "rewards")
            .ok_or_else(|| Error("rewards.xml has no rewards element".into()))?;
        let mut r = Self::default();
        let fields: [(&str, &mut i32); 8] = [
            ("realisticDamage", &mut r.realistic_damage),
            ("fixHeadCamera", &mut r.fix_head_camera),
            ("proMode", &mut r.pro_mode),
            ("surfaceAssist", &mut r.surface_assist),
            ("tractionControl", &mut r.traction_control),
            ("manualGears", &mut r.manual_gears),
            ("steeringAssist", &mut r.steering_assist),
            ("breakingAssist", &mut r.braking_assist),
        ];
        for (name, slot) in fields {
            if let Some((_, v)) = e.attributes.iter().find(|(k, _)| *k == name) {
                *slot = v
                    .trim()
                    .parse()
                    .map_err(|_| Error(format!("rewards.xml {name}={v:?}")))?;
            }
        }
        Ok(r)
    }
}

/// The whole career catalogue.
#[derive(Clone, Debug, PartialEq)]
pub struct Catalogue {
    pub ranks: Vec<Rank>,
    pub tiers: Vec<Tier>,
    pub events: BTreeMap<i32, Event>,
    pub vehicles: BTreeMap<i32, Vehicle>,
    pub sponsors: BTreeMap<i32, Sponsor>,
    pub driver_offers: BTreeMap<i32, DriverOffer>,
    /// `team.name_string_id` by team id (driver-offer teams and rivals) [D].
    pub teams: BTreeMap<i32, String>,
    pub team_mates: Vec<TeamMate>,
    /// Hireable drivers in `driver` row order.
    pub hireable: Vec<Hireable>,
    /// `team_mate_fee_range` (min, max) by `index` [D]: the search ranges.
    pub fee_ranges: Vec<(i64, i64)>,
    pub difficulties: Vec<Difficulty>,
    /// `event_reputation_reward.percentage_string`: the reputation share per
    /// finishing place (index 0 = winner).
    pub reputation_fractions: Vec<f64>,
    /// `points` tables by id.
    pub points: BTreeMap<i32, Vec<i32>>,
    /// `team_win_rewards` by rank id.
    pub team_win_rewards: BTreeMap<i32, i64>,
    /// `head_to_head_rewards`: event id -> (vehicle id, alternative money).
    pub head_to_head_rewards: BTreeMap<i32, (i32, i64)>,
    /// `career_global_settings`.
    pub initial_vehicle_id: i32,
    pub start_money: i64,
    pub assist_rewards: AssistRewards,
}

impl Catalogue {
    /// Reads `database/database.bin` and `gameconfig/rewards.xml` of an
    /// installation.
    pub fn load(root: &std::path::Path) -> std::result::Result<Self, Box<dyn std::error::Error>> {
        let schema = std::fs::read(root.join("database/schema.bin"))?;
        let data = std::fs::read(root.join("database/database.bin"))?;
        let mut catalogue = Self::from_database(&grid_formats::database::read(&schema, &data)?)?;
        let rewards = root.join("gameconfig/rewards.xml");
        if rewards.exists() {
            let bytes = std::fs::read(rewards)?;
            catalogue.assist_rewards =
                AssistRewards::from_document(&grid_formats::binxml::read(&bytes)?)?;
        }
        Ok(catalogue)
    }

    pub fn from_database(db: &Database) -> Result<Self> {
        let ranks = table(db, "career_rank")?
            .rows()
            .map(|r| {
                Ok(Rank {
                    id: int(&r, "id")?,
                    name: text(&r, "name_string_id")?,
                    reputation_to_unlock: int(&r, "reputation_to_unlock")? as i64,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let mut tiers = Vec::new();
        for r in table(db, "tier")?.rows() {
            if int(&r, "career_id")? != 1 {
                continue;
            }
            tiers.push(Tier {
                id: int(&r, "id")?,
                name: text(&r, "name_string_id")?,
                rank_id: int(&r, "career_rank_id")?,
                region: Region::from_id(int(&r, "career_region_id")?)?,
                driver_offers: int(&r, "is_driver_offer_tier")? != 0,
                order_index: int(&r, "order_index")?,
            });
        }
        tiers.sort_by_key(|t| t.id);

        let money: BTreeMap<i32, Vec<f64>> = table(db, "event_money_reward")?
            .rows()
            .map(|r| Ok((int(&r, "id")?, parse_list(&text(&r, "money_string")?)?)))
            .collect::<Result<_>>()?;
        let reputation_fractions = table(db, "event_reputation_reward")?
            .rows()
            .next()
            .map(|r| text(&r, "percentage_string"))
            .transpose()?
            .map(|s| parse_list(&s))
            .transpose()?
            .ok_or_else(|| Error("event_reputation_reward has no row".into()))?;

        let race = |r: &Row<'_>| -> Result<EventRace> {
            let base = r.text("base_result_list").unwrap_or("");
            Ok(EventRace {
                id: int(r, "id")?,
                track_model_id: int(r, "track_model_id")?,
                laps: int(r, "num_laps")?,
                time_limit_seconds: int(r, "time_limit_seconds")?,
                order_index: int(r, "order_index")?,
                player_grid_position_base: int(r, "player_grid_position_base")?,
                player_grid_position_range: int(r, "player_grid_position_range")?,
                base_results: parse_list(base)?,
            })
        };
        let mut event_races: BTreeMap<i32, Vec<EventRace>> = BTreeMap::new();
        for r in table(db, "event_track_pool")?.rows() {
            event_races
                .entry(int(&r, "event_id")?)
                .or_default()
                .push(race(&r)?);
        }
        let mut event_vehicles: BTreeMap<i32, Vec<i32>> = BTreeMap::new();
        for r in table(db, "event_vehicle")?.rows() {
            event_vehicles
                .entry(int(&r, "event_id")?)
                .or_default()
                .push(int(&r, "vehicle_id")?);
        }
        let mut event_offers: BTreeMap<i32, Vec<SponsorOffer>> = BTreeMap::new();
        for r in table(db, "event_sponsor_offer")?.rows() {
            let offer = SponsorOffer {
                id: int(&r, "id")?,
                event_id: int(&r, "event_id")?,
                sponsor_id: int(&r, "sponsor_id")?,
                objective: SponsorObjective::from_id(int(&r, "sponsor_objective_type_id")?)?,
                objective_details: int(&r, "objective_details")?,
                position_to_unlock: int(&r, "position_to_unlock")?,
                money_reward: int(&r, "money_reward")? as i64,
            };
            event_offers.entry(offer.event_id).or_default().push(offer);
        }
        let livery = |r: &Row<'_>, c: &str| r.int(c).filter(|n| *n > 0);
        let mut events = BTreeMap::new();
        for r in table(db, "event")?.rows() {
            let id = int(&r, "id")?;
            let money_string_id = int(&r, "money_string_id")?;
            let mut races = event_races.remove(&id).unwrap_or_default();
            races.sort_by_key(|x| (x.order_index, x.id));
            let mut sponsor_offers = event_offers.remove(&id).unwrap_or_default();
            sponsor_offers.sort_by_key(|o| o.id);
            events.insert(
                id,
                Event {
                    id,
                    name: text(&r, "name_string_id")?,
                    tier_id: int(&r, "tier_id")?,
                    discipline_id: int(&r, "discipline")?,
                    competition_format_id: int(&r, "competition_format_id")?,
                    head_to_head: int(&r, "head_to_head")? != 0,
                    use_lemans_selection: int(&r, "use_lemans_selection")? != 0,
                    order_index: int(&r, "order_index")?,
                    num_competitions: int(&r, "num_competitions")?,
                    percentage_towards_completion: int(&r, "percentage_towards_completion")?,
                    prize_money_for_first: int(&r, "prize_money_for_first")? as i64,
                    money_fractions: money.get(&money_string_id).cloned().unwrap_or_default(),
                    money_string_id,
                    reputation_pot: int(&r, "reputation_pot")? as i64,
                    reputation_pot_min: int(&r, "reputation_pot_min")? as i64,
                    points_id: int(&r, "points_id")?,
                    sponsor_id: int(&r, "sponsor_id")?,
                    minor_sponsor_id: int(&r, "minor_sponsor_id")?,
                    player_livery_number: livery(&r, "player_livery_number"),
                    teammate_livery_number: livery(&r, "teammate_livery_number"),
                    races,
                    vehicles: event_vehicles.remove(&id).unwrap_or_default(),
                    sponsor_offers,
                },
            );
        }

        let vehicles = table(db, "vehicle")?
            .rows()
            .map(|r| {
                let region = int(&r, "career_region_id")?;
                Ok((
                    int(&r, "id")?,
                    Vehicle {
                        id: int(&r, "id")?,
                        stem: text(&r, "file_string")?,
                        region: Region::from_id(region).ok(),
                        class_id: int(&r, "vehicle_class_id")?,
                        purchasable: int(&r, "purchasable")? != 0,
                        purchase_price: int(&r, "purchase_price")? as i64,
                        sale_price: int(&r, "sale_price")? as i64,
                        can_enter_le_mans: int(&r, "can_enter_le_mans")? != 0,
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;

        let sponsors = table(db, "sponsor")?
            .rows()
            .map(|r| {
                Ok((
                    int(&r, "id")?,
                    Sponsor {
                        id: int(&r, "id")?,
                        name: text(&r, "name_string_id")?,
                        file: text(&r, "file_string")?,
                        type_id: int(&r, "sponsor_type_id")?,
                    },
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;

        let mut offer_races: BTreeMap<i32, Vec<EventRace>> = BTreeMap::new();
        for r in table(db, "driver_offer_track")?.rows() {
            let x = EventRace {
                id: int(&r, "id")?,
                track_model_id: int(&r, "track_model_id")?,
                laps: int(&r, "num_laps")?,
                time_limit_seconds: int(&r, "time_limit_seconds")?,
                order_index: int(&r, "order_index")?,
                player_grid_position_base: int(&r, "player_grid_position_base")?,
                player_grid_position_range: int(&r, "player_grid_position_range")?,
                base_results: Vec::new(),
            };
            offer_races
                .entry(int(&r, "driver_offer_id")?)
                .or_default()
                .push(x);
        }
        let liveries: BTreeMap<i32, (i32, String)> = table(db, "vehicle_livery")?
            .rows()
            .map(|r| Ok((int(&r, "id")?, (int(&r, "vehicle_id")?, text(&r, "file_string")?))))
            .collect::<Result<_>>()?;
        let mut driver_offers = BTreeMap::new();
        for r in table(db, "driver_offer")?.rows() {
            let id = int(&r, "id")?;
            let mut races = offer_races.remove(&id).unwrap_or_default();
            races.sort_by_key(|x| (x.order_index, x.id));
            // The target columns are string lists; empty when unused [D].
            let targets = |c: &str| parse_list::<f32>(r.text(c).unwrap_or(""));
            let livery_id = int(&r, "vehicle_livery_id")?;
            let livery = liveries.get(&livery_id);
            driver_offers.insert(
                id,
                DriverOffer {
                    id,
                    tier_id: int(&r, "tier_id")?,
                    event_id: int(&r, "event_id")?,
                    team_id: int(&r, "team_id")?,
                    vehicle_livery_id: livery_id,
                    vehicle_id: livery.map(|l| l.0),
                    livery: livery.map(|l| l.1.clone()).unwrap_or_default(),
                    order_index: int(&r, "order_index")?,
                    per_race_wage: int(&r, "per_race_wage")? as i64,
                    objective: OfferObjective::from_id(int(&r, "objective_type_id")?)?,
                    objective_finishing_place: int(&r, "objective_finishing_place")?,
                    objective_team_id: int(&r, "objective_team_id")?,
                    objective_money_bonus: int(&r, "objective_money_bonus")? as i64,
                    objective_reputation_bonus: int(&r, "objective_reputation_bonus")? as i64,
                    target_times: targets("target_time_secs")?,
                    target_speeds_mph: targets("target_speed_mph")?,
                    races,
                },
            );
        }

        let teams = table(db, "team")?
            .rows()
            .map(|r| Ok((int(&r, "id")?, text(&r, "name_string_id")?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        let drivers = table(db, "driver")?;
        let mut team_mates = Vec::new();
        for r in table(db, "team_mate")?.rows() {
            let driver_id = int(&r, "driver_id")?;
            let d = drivers
                .rows()
                .find(|d| d.int("id") == Some(driver_id))
                .ok_or_else(|| Error(format!("team_mate driver {driver_id} absent")))?;
            team_mates.push(TeamMate {
                id: int(&r, "id")?,
                driver_id,
                first_name: text(&d, "first_name_string_id")?,
                surname: text(&d, "surname_string_id")?,
                rank_id: int(&r, "rank_id")?,
                region: Region::from_id(int(&r, "region_id")?)?,
                signing_fee: int(&r, "signing_fee")? as i64,
                skill: float(&r, "skill")?,
                winnings_percentage: int(&r, "winnings_percentage")?,
            });
        }
        team_mates.sort_by_key(|t| t.id);
        let mut hireable = Vec::new();
        for d in drivers.rows() {
            if d.int("can_be_hired") != Some(1) {
                continue;
            }
            hireable.push(Hireable {
                driver_id: int(&d, "id")?,
                first_name: text(&d, "first_name_string_id")?,
                surname: text(&d, "surname_string_id")?,
                rank_id: int(&d, "rank_id")?,
                region_id: int(&d, "region_id")?,
                signing_fee: int(&d, "signing_fee")? as i64,
                winnings_percentage: int(&d, "winnings_percentage")?,
                skill: float(&d, "skill")?,
                team_id: int(&d, "team_id")?,
            });
        }
        let mut ranges: Vec<(i32, (i64, i64))> = table(db, "team_mate_fee_range")?
            .rows()
            .map(|r| {
                Ok((
                    int(&r, "index")?,
                    (int(&r, "teammate_fee_min")? as i64, int(&r, "teammate_fee_max")? as i64),
                ))
            })
            .collect::<Result<_>>()?;
        ranges.sort_by_key(|r| r.0);
        let fee_ranges = ranges.into_iter().map(|r| r.1).collect();

        let mut difficulties = table(db, "difficulty")?
            .rows()
            .map(|r| {
                Ok(Difficulty {
                    id: int(&r, "id")?,
                    rank_id: int(&r, "career_rank_id")?,
                    star_level: int(&r, "star_level")?,
                    ai_ratio: float(&r, "ai_ratio")?,
                    available_flashbacks: int(&r, "available_flashbacks")?,
                    flashback_bonus: int(&r, "flashback_bonus")? as i64,
                    rep_bonus_factor: float(&r, "rep_bonus_factor")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        difficulties.sort_by_key(|d| d.id);

        let points = table(db, "points")?
            .rows()
            .map(|r| Ok((int(&r, "id")?, parse_list(&text(&r, "points")?)?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        let team_win_rewards = table(db, "team_win_rewards")?
            .rows()
            .map(|r| Ok((int(&r, "rank_id")?, int(&r, "reward")? as i64)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        let head_to_head_rewards = table(db, "head_to_head_rewards")?
            .rows()
            .map(|r| {
                Ok((
                    int(&r, "event_id")?,
                    (int(&r, "vehicle_id")?, int(&r, "alt_reward_amount")? as i64),
                ))
            })
            .collect::<Result<BTreeMap<_, _>>>()?;
        let global = table(db, "career_global_settings")?
            .rows()
            .next()
            .ok_or_else(|| Error("career_global_settings has no row".into()))?;

        Ok(Self {
            ranks,
            tiers,
            events,
            vehicles,
            sponsors,
            driver_offers,
            teams,
            team_mates,
            hireable,
            fee_ranges,
            difficulties,
            reputation_fractions,
            points,
            team_win_rewards,
            head_to_head_rewards,
            initial_vehicle_id: int(&global, "initial_vehicle_id")?,
            start_money: int(&global, "start_race_team")? as i64,
            assist_rewards: AssistRewards::default(),
        })
    }

    pub fn event(&self, id: i32) -> Option<&Event> {
        self.events.get(&id)
    }

    /// An `event_sponsor_offer` row by id.
    pub fn sponsor_offer(&self, id: i32) -> Option<&SponsorOffer> {
        self.events
            .values()
            .flat_map(|e| e.sponsor_offers.iter())
            .find(|o| o.id == id)
    }

    pub fn tier(&self, id: i32) -> Option<&Tier> {
        self.tiers.iter().find(|t| t.id == id)
    }

    /// The events of a tier in `order_index` order.
    pub fn tier_events(&self, tier_id: i32) -> Vec<&Event> {
        let mut v: Vec<&Event> = self
            .events
            .values()
            .filter(|e| e.tier_id == tier_id)
            .collect();
        v.sort_by_key(|e| (e.order_index, e.id));
        v
    }

    pub fn difficulty(&self, rank_id: i32, star_level: i32) -> Option<&Difficulty> {
        self.difficulties
            .iter()
            .find(|d| d.rank_id == rank_id && d.star_level == star_level)
    }
}
