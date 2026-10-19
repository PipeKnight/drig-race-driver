//! Career entry (docs/CAREER_SCREEN.md): one own-team career event at a
//! time, run with the race code and paid with `grid_tools::career` (F416,
//! docs/CAREER_RESEARCH.md).
//!
//! - The screen shows money, the five regional reputations, the licences and
//!   the season, and lists the open events (`Career::event_open`) that the
//!   port can run: circuit races (`competition_format` 1, head-to-heads too,
//!   no Le Mans selection) with a raceable car (`series::RACEABLE`) and
//!   enough surveyed layouts in their `event_track_pool`.
//! - An event's races are the first `num_competitions` pool layouts by
//!   `order_index` (AUTHORED: the original's draw is not traced), each with
//!   the player's slot from `player_grid_position_base/range`
//!   (`race::grid_slot`, docs/RACE_RULES_RESEARCH.md §5) on a 12-car grid of
//!   the event's cars (AUTHORED field; the career grid fill 0x5C7D40 is not
//!   traced).
//! - Process model as the event menu's (event_menu.rs): `grid-app --career`
//!   supervises `--career-screen` (exit [`CAREER_GO`] after writing the race
//!   profile) and the race (`--from-menu --career-result PATH`). The race
//!   writes its classification and driver aids when it leaves to the menu;
//!   the screen adds it to the event and, after the last race, calls
//!   `career::apply_event` and shows the payout.
//! - Files in the data dir: the save `career.save` (`career::save`), the
//!   event in progress `career-event.txt`, the race profile
//!   `career-race.ini` and the race's result `career-race.result`.
//! - Desktop only (AUTHORED): Android has no supervisor for it yet.
use crate::pause_menu::{Labels, Nav, Repeat};
use bevy::prelude::*;
use grid_tools::career::data::EventRace;
use grid_tools::career::{
    Career, Catalogue, Classified, Entrant, Flashbacks, Payout, RaceResult, RaceSettings, Region,
    SeasonState, apply_driver_offer, apply_event, save,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Exit code of the career screen: the race profile is written, start it.
pub const CAREER_GO: i32 = 22;
pub const SAVE: &str = "career.save";
pub const EVENT: &str = "career-event.txt";
pub const PROFILE: &str = "career-race.ini";
pub const RESULT: &str = "career-race.result";
/// AUTHORED career grid: 12 cars, the size of the free-race pools of the
/// rookie circuit layouts (`free_race_track_pool.max_cars`).
pub const FIELD: usize = 12;
/// AUTHORED: a career race runs the free race's AI difficulty (Normal).
pub const DIFFICULTY: f32 = 0.606;

/// The race's difficulty star level for the reputation bonus: the level
/// whose `ai_ratio` is nearest [`DIFFICULTY`] (`flashback::star_level`, as the
/// catch-up config of F439), Normal = 1, `rep_bonus_factor` −0.1 [D].
pub fn star_level() -> i32 {
    grid_tools::flashback::star_level(DIFFICULTY) as i32
}

const REGIONS: [(Region, &str); 5] = [
    (Region::Europe, "Europe"),
    (Region::UnitedStates, "United States"),
    (Region::Japan, "Japan"),
    (Region::International, "International"),
    (Region::LeMans, "Le Mans"),
];

// ---- Plans, results and the event in progress ---------------------------

/// One race of the event: route folder, laps and the player's 0-based slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    pub route: String,
    pub laps: i32,
    pub slot: usize,
    /// `track_model.id` (0 unknown): the race count the draw reads.
    pub layout: i32,
    /// The race's opponents in grid order as field indices (car k of the
    /// race is `field[cars[k - 1]]`); empty for a race without a field.
    pub cars: Vec<usize>,
}

/// One car of the career field (`career::field`), as the race needs it.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldEntry {
    pub entrant: Entrant,
    /// Model stem and livery package index (`livery_NN`).
    pub stem: String,
    pub livery: usize,
    pub name: String,
}

impl FieldEntry {
    fn write(&self) -> String {
        let (kind, id, team) = match self.entrant {
            Entrant::Player => ("p", 0, 0),
            Entrant::TeamMate => ("t", 0, 0),
            Entrant::Ai { driver, team } => ("a", driver, team),
        };
        format!("entrant {kind} {id} {team} {} {} {}\n", self.stem, self.livery, self.name)
    }

    fn read<'a>(mut f: impl Iterator<Item = &'a str>) -> Result<Self, String> {
        let mut next = || f.next().ok_or("short entrant line");
        let kind = next()?;
        let id: i32 = next()?.parse().map_err(|_| "bad entrant id")?;
        let team: i32 = next()?.parse().map_err(|_| "bad entrant team")?;
        let stem = next()?.to_owned();
        let livery = next()?.parse().map_err(|_| "bad entrant livery")?;
        let name: Vec<&str> = std::iter::from_fn(|| f.next()).collect();
        Ok(Self {
            entrant: match kind {
                "p" => Entrant::Player,
                "t" => Entrant::TeamMate,
                _ => Entrant::Ai { driver: id, team },
            },
            stem,
            livery,
            name: name.join(" "),
        })
    }
}

/// The livery package index of a `vehicle_livery.file_string` (`bmw_12` →
/// 12; `bmw_hero` and others → 0).
fn livery_index(file: &str) -> usize {
    file.rsplit('_').next().and_then(|n| n.parse().ok()).unwrap_or(0)
}

/// The career field and each race's grid (docs/CAREER_RESEARCH.md §12):
/// `career::field::field` once per event, then `field::grid` per race with
/// the race's pool row. AUTHORED: an AI car whose model the port cannot
/// race drives the player's model in livery 0; the player wears
/// `player_livery`; the seed is the clock's.
#[allow(clippy::too_many_arguments)]
pub fn career_plan(
    cat: &Catalogue,
    career: &Career,
    entry: grid_tools::career::field::Entry,
    player_stem: &str,
    player_livery: usize,
    races: &[(EventRace, String)],
    exists: impl Fn(&str) -> bool,
    seed: u32,
) -> (Vec<FieldEntry>, Vec<Planned>) {
    use grid_tools::career::field;
    let mut rng = grid_tools::series::Mt::new(seed);
    let cars = field::field(cat, career, entry, 0, &mut rng);
    let entries: Vec<FieldEntry> = cars
        .iter()
        .map(|c| {
            let model = cat.liveries.get(&c.livery_id).and_then(|(vehicle, file)| {
                let stem = &cat.vehicles.get(vehicle)?.stem;
                (grid_tools::series::RACEABLE.contains(&stem.as_str()) && exists(&format!("cars/{stem}")))
                    .then(|| (stem.clone(), livery_index(file)))
            });
            let (stem, livery) = match c.entrant {
                Entrant::Player => (player_stem.to_owned(), player_livery),
                _ => model.unwrap_or((player_stem.to_owned(), 0)),
            };
            let name = match c.driver_id.and_then(|d| cat.drivers.get(&d)) {
                Some(d) => format!("{} {}", d.first_name, d.surname).trim().to_owned(),
                None => "You".into(),
            };
            FieldEntry {
                entrant: c.entrant,
                stem,
                livery,
                name: name.replace(',', " "),
            }
        })
        .collect();
    let offer = matches!(entry, field::Entry::Offer(_));
    let planned = races
        .iter()
        .map(|(r, route)| {
            let order = field::grid(
                cat,
                &cars,
                (!offer).then_some(r.id),
                r.player_grid_position_base,
                r.player_grid_position_range,
                &mut rng,
            );
            Planned {
                route: route.clone(),
                laps: r.laps,
                slot: order.iter().position(|&i| i == 0).unwrap_or(0),
                layout: r.track_model_id,
                cars: order.into_iter().filter(|&i| i != 0).collect(),
            }
        })
        .collect();
    (entries, planned)
}

/// The race profile of a race with the career field: its cars, liveries
/// and names in grid order (the opponents take the free slots in car
/// order), the player in `slot`.
pub fn field_profile(field: &[FieldEntry], race: &Planned) -> String {
    let player = &field[0];
    let cars: Vec<&FieldEntry> = race.cars.iter().filter_map(|&i| field.get(i)).collect();
    let list = |f: &dyn Fn(&FieldEntry) -> String| cars.iter().map(|c| f(c)).collect::<Vec<_>>().join(",");
    format!(
        "# Written by the career screen (docs/CAREER_SCREEN.md) for the next career race.\n\
         [game]\nroute = {}\ncar = {}\nopponent_cars = {}\nopponent_liveries = {}\nopponent_names = {}\n\
         [drive]\nenabled = on\nlaps = {}\nopponents = {}\ngrid_position = {}\n\
         ai_characteristics = ai-driver-characteristics.tsv\n\
         [ai]\ndifficulty = {DIFFICULTY}\ncatchup = on\nchoreographer = on\nmistakes = on\n\
         [private]\nbody_contacts = derive\nbody_events = derive\ndeformation = derive\n",
        race.route,
        player.stem,
        list(&|c| c.stem.clone()),
        list(&|c| c.livery.to_string()),
        list(&|c| c.name.clone()),
        race.laps,
        cars.len(),
        race.slot + 1,
    )
}

/// The race's result in field entrants: car k of the race is
/// `field[race.cars[k - 1]]`.
pub fn to_field(result: &mut RaceResult, field: &[FieldEntry], race: &Planned) {
    for c in &mut result.order {
        if let Entrant::Ai { driver, .. } = c.entrant
            && let Some(e) = race.cars.get((driver - 1).max(0) as usize).and_then(|&i| field.get(i))
            && driver >= 1
        {
            c.entrant = e.entrant;
        }
    }
}

/// An event the port can run, with its drawn layouts (not yet the slots).
#[derive(Clone, Debug, PartialEq)]
pub struct Offer {
    pub event_id: i32,
    pub label: String,
    /// The player's car stem and the field's stems (the event's raceable
    /// `event_vehicle` models, each once).
    pub car: String,
    pub field_cars: Vec<String>,
    pub races: Vec<(EventRace, String)>,
}

/// `track_model.id` → route folder (`tracks/<folder>/<file>/<route>`).
pub fn track_routes(db: &grid_formats::database::Database) -> BTreeMap<i32, String> {
    let mut out = BTreeMap::new();
    if let Some(models) = db.table("track_model") {
        for m in models.rows() {
            if let (Some(id), Some(folder), Some(file), Some(route)) = (
                m.int("id"),
                m.text("folder_string"),
                m.text("file_string"),
                m.text("route_string"),
            ) {
                out.insert(id, format!("tracks/{folder}/{file}/{}", route.to_ascii_lowercase()));
            }
        }
    }
    out
}

/// A layout the port has raced (the rule of `Series::supported_layouts`).
pub fn surveyed(route: &str) -> bool {
    grid_tools::series::route_supported(route)
}

/// The open events this port can run, in tier then event order.
pub fn offers(
    cat: &Catalogue,
    career: &Career,
    routes: &BTreeMap<i32, String>,
    labels: &Labels,
    exists: impl Fn(&str) -> bool,
) -> Vec<Offer> {
    let mut out = Vec::new();
    for event in cat.events.values() {
        if !career.event_open(cat, event.id)
            || event.use_lemans_selection
            || event.competition_format_id != 1
            || event.num_competitions < 1
        {
            continue;
        }
        let mut field_cars: Vec<String> = Vec::new();
        for v in &event.vehicles {
            if let Some(vehicle) = cat.vehicles.get(v)
                && grid_tools::series::RACEABLE.contains(&vehicle.stem.as_str())
                && exists(&format!("cars/{}", vehicle.stem))
                && !field_cars.contains(&vehicle.stem)
            {
                field_cars.push(vehicle.stem.clone());
            }
        }
        // A model the team owns comes first (the garage's car), else the
        // first raceable one (AUTHORED: the port does not require owning it).
        let owned = |stem: &String| {
            career
                .cars
                .iter()
                .any(|c| cat.vehicles.get(&c.vehicle_id).is_some_and(|v| &v.stem == stem))
        };
        let Some(car) = field_cars.iter().find(|s| owned(s)).or(field_cars.first()).cloned() else {
            continue;
        };
        // The original's draw [V 0x67BC80]: by order_index on a first entry,
        // by the races driven per layout afterwards; venues first. [I] An
        // event counts as completed once it has been paid.
        let drawn = grid_tools::career::field::draw_races(
            cat,
            &event.races,
            event.num_competitions as usize,
            !career.event_results.contains_key(&event.id),
            &career.layout_races,
        );
        // AUTHORED: an event whose drawn races the port cannot run all is
        // not offered (no substitute layout).
        let races: Vec<(EventRace, String)> = drawn
            .into_iter()
            .filter(|r| r.laps > 0 && r.time_limit_seconds == 0)
            .filter_map(|r| {
                let route = routes.get(&r.track_model_id)?.clone();
                (surveyed(&route) && exists(&route)).then_some((r, route))
            })
            .collect();
        if races.len() < event.num_competitions as usize {
            continue;
        }
        let key = format!("db_{}", event.name);
        let text = labels.get(&key);
        let mut label = if text == key { event.name.clone() } else { text.to_owned() };
        if event.head_to_head {
            label = format!("Head-to-head: {label}");
        }
        out.push(Offer {
            event_id: event.id,
            label,
            car,
            field_cars,
            races,
        });
    }
    out
}

/// A driver offer on show (`Career::driver_offers_shown`) that the port can
/// run: its car is raceable and its one race (`driver_offer_track`) a lap
/// race on a surveyed layout.
#[derive(Clone, Debug, PartialEq)]
pub struct OfferRun {
    pub offer_id: i32,
    pub label: String,
    pub car: String,
    /// The field's stems: the offer event's raceable cars (race offers), the
    /// car alone (test offers run solo).
    pub field_cars: Vec<String>,
    pub race: (EventRace, String),
}

/// A driver offer's objective in words (`db_race_offer_*`,
/// `driver_offer_desc_*` [D], English).
pub fn objective_text(cat: &Catalogue, offer: &grid_tools::career::data::DriverOffer, labels: &Labels) -> String {
    use grid_tools::career::data::OfferObjective as O;
    let star = star_level().clamp(0, 5) as usize;
    match offer.objective {
        O::FinishingPlace | O::FinishingPlaceLeMans => {
            format!("Finish no lower than {}", offer.objective_finishing_place)
        }
        O::BeatRival => format!("Finish ahead of {}", team_name(cat, offer.objective_team_id, labels)),
        O::NoDamage => "Finish the race with no damage".into(),
        O::TimeTrial => format!(
            "Lap faster than {}",
            offer.target_times.get(star).map_or("?".into(), |t| lap_text(f64::from(*t)))
        ),
        O::SpeedTest => format!(
            "Exceed {} mph",
            offer.target_speeds_mph.get(star).map_or("?".into(), |v| format!("{v:.0}"))
        ),
    }
}

/// A team's name: `db_<team.name_string_id>` when the language has it.
fn team_name(cat: &Catalogue, team_id: i32, labels: &Labels) -> String {
    let Some(name) = cat.teams.get(&team_id) else {
        return format!("team {team_id}");
    };
    let key = format!("db_{name}");
    let text = labels.get(&key);
    if text == key { name.clone() } else { text.to_owned() }
}

fn lap_text(seconds: f64) -> String {
    let m = (seconds / 60.).floor();
    format!("{}:{:06.3}", m as i64, seconds - m * 60.)
}

/// The driver offers on show that the port can run.
pub fn offer_runs(
    cat: &Catalogue,
    career: &Career,
    routes: &BTreeMap<i32, String>,
    labels: &Labels,
    exists: impl Fn(&str) -> bool,
) -> Vec<OfferRun> {
    let raceable = |id: &i32| {
        cat.vehicles
            .get(id)
            .map(|v| v.stem.clone())
            .filter(|stem| grid_tools::series::RACEABLE.contains(&stem.as_str()) && exists(&format!("cars/{stem}")))
    };
    let mut out = Vec::new();
    for id in career.driver_offers_shown(cat) {
        let offer = &cat.driver_offers[&id];
        let Some(car) = offer.vehicle_id.as_ref().and_then(raceable) else {
            continue;
        };
        let Some(race) = offer.races.first() else {
            continue;
        };
        let Some(route) = routes.get(&race.track_model_id) else {
            continue;
        };
        if race.laps < 1 || race.time_limit_seconds != 0 || !surveyed(route) || !exists(route) {
            continue;
        }
        let mut field_cars = vec![car.clone()];
        if !grid_tools::career::rules::is_test_offer(offer) {
            for stem in cat
                .event(offer.event_id)
                .map(|e| e.vehicles.as_slice())
                .unwrap_or_default()
                .iter()
                .filter_map(raceable)
            {
                if !field_cars.contains(&stem) {
                    field_cars.push(stem);
                }
            }
        }
        let event = cat.event(offer.event_id).map_or("", |e| e.name.as_str());
        let key = format!("db_{event}");
        let text = labels.get(&key);
        let event = if text == key { event.to_owned() } else { text.to_owned() };
        let kind = if grid_tools::career::rules::is_test_offer(offer) { "Test" } else { "Race" };
        out.push(OfferRun {
            offer_id: id,
            label: format!(
                "{kind} for {}: {event} - {} (fee ${}, bonus ${})",
                team_name(cat, offer.team_id, labels),
                objective_text(cat, offer, labels),
                thousands(if kind == "Race" { grid_tools::career::rules::appearance_fee(offer) } else { 0 }),
                thousands(offer.objective_money_bonus),
            ),
            car,
            field_cars,
            race: (race.clone(), route.clone()),
        });
    }
    out
}

/// AUTHORED: in an own-team event, the hired team mate drives AI car 1 (the
/// career field, where he takes a team's second car, is item 3 of
/// docs/CAREER_SCREEN.md).
pub const TEAMMATE_CAR: i32 = 1;

/// Marks [`TEAMMATE_CAR`] as the team mate in a race result.
pub fn teammate_car(result: &mut RaceResult) {
    for c in &mut result.order {
        if matches!(c.entrant, Entrant::Ai { driver, .. } if driver == TEAMMATE_CAR) {
            c.entrant = Entrant::TeamMate;
        }
    }
}

/// AUTHORED: in a driver offer race, AI cars 1 and 2 drive for the rival
/// team of a beat-rival objective (the career field's teams are not traced).
pub const RIVAL_CARS: [i32; 2] = [1, 2];

/// Whether the offer's objective was met by the race (`rules::
/// offer_objective_met`, [V] objective classes), with the rival team given to
/// [`RIVAL_CARS`].
pub fn offer_met(cat: &Catalogue, offer_id: i32, out: &RaceOut, rival_cars: bool) -> bool {
    let Some(offer) = cat.driver_offers.get(&offer_id) else {
        return false;
    };
    let mut result = out.result.clone();
    for c in result.order.iter_mut().filter(|_| rival_cars) {
        if let Entrant::Ai { driver, team } = &mut c.entrant
            && RIVAL_CARS.contains(driver)
        {
            *team = offer.objective_team_id;
        }
    }
    grid_tools::career::rules::offer_objective_met(
        offer,
        &result,
        out.settings.star_level,
        out.best_lap.map(|v| v as f32),
        out.top_mph,
    )
}

/// The race profile of one planned race: the event menu's keys for a race.
pub fn race_profile(car: &str, field_cars: &[String], race: &Planned, solo: bool) -> String {
    let head = "# Written by the career screen (docs/CAREER_SCREEN.md) for the next career race.\n";
    let tail = "[private]\nbody_contacts = derive\nbody_events = derive\ndeformation = derive\n";
    if solo {
        // A test offer: the port's Time Trial (no opponents) on the layout.
        return format!(
            "{head}[game]\nroute = {}\ncar = {car}\n[drive]\nenabled = on\nlaps = {}\n{tail}",
            race.route, race.laps
        );
    }
    format!(
        "{head}[game]\nroute = {}\ncar = {car}\nopponent_cars = {}\n\
         [drive]\nenabled = on\nlaps = {}\nopponents = {}\ngrid_position = {}\n\
         ai_characteristics = ai-driver-characteristics.tsv\n\
         [ai]\ndifficulty = {DIFFICULTY}\ncatchup = on\nchoreographer = on\nmistakes = on\n{tail}",
        race.route,
        field_cars.join(","),
        race.laps,
        FIELD - 1,
        race.slot + 1,
    )
}

/// A classification token: car index (0 the player), `x` when out.
fn token(c: &Classified) -> String {
    let out = if c.finished { "" } else { "x" };
    match c.entrant {
        Entrant::Player => format!("0{out}"),
        Entrant::TeamMate => format!("t{out}"),
        Entrant::Ai { driver, .. } => format!("{driver}{out}"),
    }
}

/// AUTHORED: each AI car is its own driver and team (car index); the
/// career field's drivers and teams are not traced.
fn classified(token: &str) -> Result<Classified, String> {
    let (num, finished) = match token.strip_suffix('x') {
        Some(n) => (n, false),
        None => (token, true),
    };
    if num == "t" {
        return Ok(Classified {
            entrant: Entrant::TeamMate,
            finished,
            undamaged: finished,
        });
    }
    let car: i32 = num.parse().map_err(|_| format!("bad car {token:?}"))?;
    Ok(Classified {
        entrant: if car == 0 {
            Entrant::Player
        } else {
            Entrant::Ai {
                driver: car,
                team: car,
            }
        },
        finished,
        // AUTHORED: the no-damage flag's source is not traced; a car still
        // classified counts as undamaged.
        undamaged: finished,
    })
}

fn settings_text(s: &RaceSettings) -> String {
    let b = |v: bool| u8::from(v);
    format!(
        "star={} head={} pro={} manual={} tc={} steer={} brake={}",
        s.star_level,
        b(s.head_camera_only),
        b(s.pro_mode),
        b(s.manual_gears),
        b(s.traction_control),
        b(s.steering_assist),
        b(s.braking_assist)
    )
}

fn parse_settings<'a>(items: impl Iterator<Item = &'a str>) -> Result<RaceSettings, String> {
    let mut s = RaceSettings {
        star_level: star_level(),
        traction_control: true,
        steering_assist: true,
        braking_assist: true,
        ..RaceSettings::default()
    };
    for item in items {
        let (k, v) = item.split_once('=').ok_or(format!("bad setting {item:?}"))?;
        let on = v == "1";
        match k {
            "star" => s.star_level = v.parse().map_err(|_| format!("bad star {v:?}"))?,
            "head" => s.head_camera_only = on,
            "pro" => s.pro_mode = on,
            "manual" => s.manual_gears = on,
            "tc" => s.traction_control = on,
            "steer" => s.steering_assist = on,
            "brake" => s.braking_assist = on,
            _ => return Err(format!("unknown setting {k}")),
        }
    }
    Ok(s)
}

/// What a race reports when it leaves to the career screen.
#[derive(Clone, Debug, PartialEq)]
pub struct RaceOut {
    pub result: RaceResult,
    pub settings: RaceSettings,
    /// The player's best lap in seconds (time-trial offers).
    pub best_lap: Option<f64>,
    /// The player's top speed in mph (speed-test offers; AUTHORED: sampled
    /// once per frame, the original's +0x28 is per result [V 0x655C80]).
    pub top_mph: Option<f32>,
}

pub fn write_race_out(out: &RaceOut) -> String {
    let order: Vec<String> = out.result.order.iter().map(token).collect();
    let mut s = format!(
        "grid-career-race 1\nsettings {}\norder {}\n",
        settings_text(&out.settings),
        order.join(" ")
    );
    if let Some(b) = out.best_lap {
        s += &format!("best {b}\n");
    }
    if let Some(t) = out.top_mph {
        s += &format!("top {t}\n");
    }
    if let Some(f) = out.result.flashbacks {
        s += &format!("flashbacks {} {}\n", f.used, f.maximum);
    }
    s
}

fn flashbacks<'a>(mut f: impl Iterator<Item = &'a str>) -> Result<Flashbacks, String> {
    let mut n = || {
        f.next()
            .and_then(|v| v.parse::<u32>().ok())
            .ok_or_else(|| "bad flashbacks".to_string())
    };
    Ok(Flashbacks { used: n()?, maximum: n()? })
}

pub fn read_race_out(text: &str) -> Result<RaceOut, String> {
    let mut lines = text.lines();
    if lines.next() != Some("grid-career-race 1") {
        return Err("not a career race result".into());
    }
    let (mut result, mut settings, mut best_lap, mut top_mph, mut fb) = (None, None, None, None, None);
    for line in lines {
        let mut f = line.split_whitespace();
        match f.next() {
            Some("settings") => settings = Some(parse_settings(f)?),
            Some("best") => best_lap = f.next().and_then(|v| v.parse().ok()),
            Some("top") => top_mph = f.next().and_then(|v| v.parse().ok()),
            Some("order") => {
                result = Some(RaceResult {
                    order: f.map(classified).collect::<Result<_, _>>()?,
                    flashbacks: None,
                })
            }
            Some("flashbacks") => fb = Some(flashbacks(f)?),
            None => {}
            Some(other) => return Err(format!("unknown line {other}")),
        }
    }
    let mut result = result.ok_or("no order")?;
    result.flashbacks = fb;
    if result.position(Entrant::Player).is_none() {
        return Err("the player is not classified".into());
    }
    Ok(RaceOut {
        result,
        settings: settings.ok_or("no settings")?,
        best_lap,
        top_mph,
    })
}

/// The event in progress (`career-event.txt`, AUTHORED format).
#[derive(Clone, Debug, PartialEq)]
pub struct InProgress {
    /// A driver offer (`driver_offer.id`) instead of an own-team event.
    pub offer: Option<i32>,
    pub event_id: i32,
    pub car: String,
    pub field_cars: Vec<String>,
    pub races: Vec<Planned>,
    /// The career field (empty: the AUTHORED field of `field_cars`, as
    /// F424/F464 and for Le Mans selection events).
    pub field: Vec<FieldEntry>,
    pub results: Vec<RaceResult>,
    /// The last race's settings (the original reads them at the event end).
    pub settings: Option<RaceSettings>,
    /// The last race's best lap and top speed (driver offers).
    pub best_lap: Option<f64>,
    pub top_mph: Option<f32>,
}

impl InProgress {
    /// The last race as a [`RaceOut`] (driver offers).
    pub fn last_out(&self) -> Option<RaceOut> {
        Some(RaceOut {
            result: self.results.last()?.clone(),
            settings: self.settings.unwrap_or_default(),
            best_lap: self.best_lap,
            top_mph: self.top_mph,
        })
    }

    pub fn done(&self) -> bool {
        self.results.len() >= self.races.len()
    }

    pub fn next(&self) -> Option<&Planned> {
        self.races.get(self.results.len())
    }

    pub fn add(&mut self, out: RaceOut) {
        if !self.done() {
            self.results.push(out.result);
            self.settings = Some(out.settings);
            self.best_lap = out.best_lap;
            self.top_mph = out.top_mph;
        }
    }

    pub fn write(&self) -> String {
        let mut s = format!(
            "grid-career-event 1\nevent {}\ncar {}\nfield {}\n",
            self.event_id,
            self.car,
            self.field_cars.join(",")
        );
        for r in &self.races {
            let cars: Vec<String> = r.cars.iter().map(|c| c.to_string()).collect();
            s += &format!("race {} {} {} {} {}\n", r.route, r.laps, r.slot, r.layout, cars.join(","));
        }
        for c in &self.field {
            s += &c.write();
        }
        for r in &self.results {
            let order: Vec<String> = r.order.iter().map(token).collect();
            s += &format!("result {}\n", order.join(" "));
            if let Some(f) = r.flashbacks {
                s += &format!("flashbacks {} {}\n", f.used, f.maximum);
            }
        }
        if let Some(settings) = &self.settings {
            s += &format!("settings {}\n", settings_text(settings));
        }
        if let Some(o) = self.offer {
            s += &format!("offer {o}\n");
        }
        if let Some(b) = self.best_lap {
            s += &format!("best {b}\n");
        }
        if let Some(t) = self.top_mph {
            s += &format!("top {t}\n");
        }
        s
    }

    pub fn read(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        if lines.next() != Some("grid-career-event 1") {
            return Err("not a career event".into());
        }
        let mut e = Self {
            offer: None,
            event_id: 0,
            car: String::new(),
            field_cars: Vec::new(),
            races: Vec::new(),
            field: Vec::new(),
            results: Vec::new(),
            settings: None,
            best_lap: None,
            top_mph: None,
        };
        let num = |v: Option<&str>| -> Result<i64, String> {
            v.and_then(|v| v.parse().ok()).ok_or("bad number".into())
        };
        for line in lines {
            let mut f = line.split_whitespace();
            match f.next() {
                Some("event") => e.event_id = num(f.next())? as i32,
                Some("car") => e.car = f.next().ok_or("no car")?.into(),
                Some("field") => {
                    e.field_cars = f.next().unwrap_or("").split(',').map(String::from).collect()
                }
                Some("race") => e.races.push(Planned {
                    route: f.next().ok_or("no route")?.into(),
                    laps: num(f.next())? as i32,
                    slot: num(f.next())? as usize,
                    layout: f.next().map_or(Ok(0), |v| num(Some(v)))? as i32,
                    cars: f
                        .next()
                        .unwrap_or("")
                        .split(',')
                        .filter(|v| !v.is_empty())
                        .map(|v| num(Some(v)).map(|n| n as usize))
                        .collect::<Result<_, _>>()?,
                }),
                Some("entrant") => e.field.push(FieldEntry::read(f)?),
                Some("result") => e.results.push(RaceResult {
                    order: f.map(classified).collect::<Result<_, _>>()?,
                    flashbacks: None,
                }),
                Some("flashbacks") => {
                    let fb = flashbacks(f)?;
                    e.results.last_mut().ok_or("flashbacks before a result")?.flashbacks = Some(fb);
                }
                Some("settings") => e.settings = Some(parse_settings(f)?),
                Some("offer") => e.offer = Some(num(f.next())? as i32),
                Some("best") => e.best_lap = f.next().and_then(|v| v.parse().ok()),
                Some("top") => e.top_mph = f.next().and_then(|v| v.parse().ok()),
                None => {}
                Some(other) => return Err(format!("unknown line {other}")),
            }
        }
        if e.races.is_empty() || e.car.is_empty() {
            return Err("career event without races".into());
        }
        Ok(e)
    }
}

/// A new career starts in Arc 1 [V 0x674610]: no team and no money; the
/// player drives driver offers until the bank reaches `start_race_team`
/// (40,000), which founds the team.
pub fn new_career(_cat: &Catalogue) -> Career {
    Career::new()
}

/// Adds `[game] points` (the event's points table, the results' Points
/// column) to a race profile.
pub fn with_points(profile: &str, points: &[i32]) -> String {
    let list: Vec<String> = points.iter().map(i32::to_string).collect();
    profile.replacen("[game]\n", &format!("[game]\npoints = {}\n", list.join(",")), 1)
}

/// The event standings so far (`driver_event_standings_*` [D]; the lines
/// are AUTHORED text): rank, name, points, by `rules::standings`.
pub fn standings_lines(points: &[i32], e: &InProgress, team_mate: &str) -> Vec<String> {
    grid_tools::career::rules::standings(points, &e.results)
        .iter()
        .map(|s| {
            let name = e
                .field
                .iter()
                .find(|f| f.entrant == s.entrant)
                .map(|f| f.name.clone())
                .unwrap_or_else(|| match s.entrant {
                    Entrant::Player => "You".into(),
                    Entrant::TeamMate => team_mate.into(),
                    Entrant::Ai { driver, .. } => format!("Car {driver}"),
                });
            format!("    {:>2}. {name} - {} pts", s.rank + 1, s.points)
        })
        .collect()
}

/// The payout lines (the original's bank text names, English): the paid
/// items, then the balance and the season's earnings
/// (`bank_rewards_balance_desc`, `bank_rewards_season_earnings_desc` [D]).
pub fn payout_lines(p: &Payout, c: &Career) -> Vec<String> {
    let money = |v: i64| format!("${}", thousands(v));
    let offer = p.driver_offer_wages != 0 || p.driver_offer_bonus != 0 || p.driver_offer_reputation != 0;
    let mut lines = Vec::new();
    if p.head_to_head {
        // A head-to-head pays its prize for a win and no reputation
        // [V 0x6788D0]; the text is AUTHORED.
        lines.push(format!("Head-to-head: {}", if p.position == Some(0) { "won" } else { "lost" }));
        lines.push(format!("Prize Money: {}", money(p.prize)));
        lines.push(format!("Sponsorship Money: {}", money(p.sponsor_money)));
        lines.push(format!("Flashback Bonus: {}", money(p.flashback_bonus)));
    } else if offer || p.position.is_none() && p.prize == 0 && p.sponsor_money == 0 {
        // A driver offer pays its own lines (bank_rewards_do_*) [D].
        lines.push(format!("Driver Offer Wages: {}", money(p.driver_offer_wages)));
        lines.push(format!("Driver Offer Rewards: {}", money(p.driver_offer_bonus)));
        lines.push(format!("Flashback Bonus: {}", money(p.flashback_bonus)));
    } else {
        lines.push(match p.position {
            Some(pos) => format!("Championship position: {}", pos + 1),
            None => "Not classified".into(),
        });
        // The bank screen's rows (0x555620) [V]: prize, sponsorship, team
        // win, flashback, then the teammate's share with a team mate.
        lines.push(format!("Prize Money: {}", money(p.prize)));
        lines.push(format!("Sponsorship Money: {}", money(p.sponsor_money)));
        lines.push(format!("Team Win Bonus: {}", money(p.team_win_bonus)));
        lines.push(format!("Flashback Bonus: {}", money(p.flashback_bonus)));
        if c.team_mate.is_some() || p.team_mate_prize != 0 {
            lines.push(format!("Teammate Prize Money: {}", money(p.team_mate_prize)));
        }
    }
    lines.push(format!("Total: {}", money(p.money())));
    lines.push(format!("Balance: {}", money(c.money)));
    lines.push(format!("Season's Earnings: {}", money(c.season_earnings)));
    if p.team_founded {
        lines.push("Your race team is founded: the founding sum is paid, and the rookie licences and the free car come with it.".into());
    }
    let region = p
        .region
        .and_then(|r| REGIONS.iter().find(|(x, _)| *x == r))
        .map_or("", |(_, n)| *n);
    lines.push(if offer || p.head_to_head {
        format!("Reputation: {} pts to {region}", thousands(p.reputation()))
    } else {
        format!(
            "Reputation: {} pts ({} + bonus {}) to {region}",
            thousands(p.reputation()),
            thousands(p.reputation_base),
            thousands(p.reputation_bonus)
        )
    });
    if !p.new_sponsor_offers.is_empty() {
        lines.push(format!("New sponsor offers: {}", p.new_sponsor_offers.len()));
    }
    lines
}

/// A sale listing's state in words (the `ebay_msg_*` popups [D]).
fn listing_state(state: grid_tools::career::ebay::ListingState) -> &'static str {
    use grid_tools::career::ebay::ListingState as S;
    match state {
        S::Listed => "listed",
        S::Bidding => "bidding in progress",
        S::FixedPriceMet => "sold at the fixed price",
        S::Sold => "sold at the last bid",
        S::Unsold => "the auction ended without a bid at the start price",
        S::NoBids => "no bids yet",
    }
}

fn thousands(v: i64) -> String {
    let digits = v.unsigned_abs().to_string();
    let mut out = String::new();
    for (i, ch) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    if v < 0 { format!("-{out}") } else { out }
}

/// Money, reputations, licences and season (the screen's status lines).
pub fn status_lines(cat: &Catalogue, c: &Career) -> Vec<String> {
    let rank_name = |r: usize| {
        cat.ranks
            .get(r)
            .map_or("?".to_owned(), |rank| rank.name.clone())
    };
    let mut lines = vec![
        format!("Money: ${}", thousands(c.money)),
        format!(
            "Reputation: {} pts (overall rank {})",
            thousands(c.total_reputation()),
            rank_name(c.overall_rank(cat))
        ),
    ];
    for (region, name) in REGIONS {
        lines.push(format!(
            "    {name}: {} pts, {}",
            thousands(c.reputation[region as usize]),
            rank_name(c.rank(cat, region))
        ));
    }
    // The fact's count (7 − countdown) holds only pro, expert and the
    // Global Race Permit; the rookie licences come with the team.
    lines.push(format!(
        "Licences: {} / 7{}",
        c.licences(cat),
        if c.team_founded { " (rookie licences held)" } else { "" }
    ));
    let state = match c.season_state() {
        SeasonState::Running => "running",
        SeasonState::LeMansPending => "Le Mans pending",
        SeasonState::Ended => "ended",
    };
    lines.push(format!(
        "Season {}: {state}, {} of 5 entries",
        c.season_number,
        c.season.len()
    ));
    lines
}

// ---- The race side -------------------------------------------------------

/// In a career race: where the race writes its result when it leaves.
#[derive(Resource)]
pub struct ResultPath(pub PathBuf);

/// In a career race: the player's top speed so far, m/s (speed-test offers).
#[derive(Resource, Default)]
pub struct TopSpeed(pub f32);

/// Samples the player's speed once per frame (AUTHORED sampling).
pub fn track_top_speed(driving: Res<crate::Driving>, mut top: ResMut<TopSpeed>) {
    let v = driving.runtime.velocity();
    let speed = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if speed.is_finite() && speed > top.0 {
        top.0 = speed;
    }
}

const MPS_TO_MPH: f32 = 3600. / 1609.344;

/// The race's classification as car tokens (`post_race::table` order).
pub fn race_out(
    rows: &[crate::post_race::Row],
    aids: &grid_tools::source_vehicle::assists::Assists,
    top_mps: f32,
    flashbacks: Option<Flashbacks>,
) -> RaceOut {
    use grid_tools::source_vehicle::assists::{Level, Transmission};
    let order = rows
        .iter()
        .map(|row| Classified {
            entrant: if row.car == 0 {
                Entrant::Player
            } else {
                Entrant::Ai {
                    driver: row.car as i32,
                    team: row.car as i32,
                }
            },
            finished: !matches!(row.time, crate::post_race::Cell::Out(..)),
            undamaged: !matches!(
                row.time,
                crate::post_race::Cell::Out(grid_tools::race::Out::Wrecked, _)
            ),
        })
        .collect();
    RaceOut {
        result: RaceResult { order, flashbacks },
        settings: RaceSettings {
            star_level: star_level(),
            manual_gears: aids.transmission == Transmission::Manual,
            traction_control: aids.traction_control != Level::OFF,
            steering_assist: aids.stability_control,
            braking_assist: aids.braking_assist != Level::OFF,
            ..RaceSettings::default()
        },
        best_lap: rows.iter().find(|r| r.car == 0).and_then(|r| r.best).map(|b| b.0),
        top_mph: (top_mps > 0.).then_some(top_mps * MPS_TO_MPH),
    }
}

/// Writes the result when the race leaves to the career screen
/// (`event_menu::leave`, exit code BACK_TO_MENU).
pub fn write_result(
    mut exits: MessageReader<AppExit>,
    path: Res<ResultPath>,
    driving: Res<crate::Driving>,
    trial: Option<Res<crate::time_trial::Trial>>,
    top: Option<Res<TopSpeed>>,
    flashback: Option<Res<crate::flashback::Flashback>>,
) {
    let back = AppExit::from_code(crate::event_menu::BACK_TO_MENU as u8);
    if !exits.read().any(|e| *e == back) {
        return;
    }
    let rows = crate::post_race::table(&driving, trial.as_deref());
    let fb = flashback.map(|f| Flashbacks {
        used: f.manager.used,
        maximum: f.manager.maximum,
    });
    let out = race_out(&rows, &driving.runtime.assists, top.map_or(0., |t| t.0), fb);
    if out.result.position(Entrant::Player).is_none() {
        log_line!("drive_career version=1 event=race_result_missing");
        return;
    }
    match std::fs::write(&path.0, write_race_out(&out)) {
        Ok(()) => log_line!(
            "drive_career version=1 event=race_result position={} cars={} flashbacks_used={} flashbacks_maximum={}",
            out.result.position(Entrant::Player).unwrap_or(0) + 1,
            out.result.order.len(),
            fb.map_or(-1, |f| f.used as i64),
            fb.map_or(-1, |f| f.maximum as i64)
        ),
        Err(e) => log_line!("drive_career version=1 event=race_result_failed error={e:?}"),
    }
}

// ---- Processes -----------------------------------------------------------

/// Diagnostics `GRID_CAREER_AUTO=EVENT_ID` (unattended checks, desktop): the
/// screen enters that event, starts each next race and quits after the
/// payout; the supervisor runs the races as scripted runs (`--drive-ticks`),
/// so `GRID_AUTOPILOT` drives and `GRID_RESULTS_CONTINUE` leaves the results.
fn auto() -> Option<Auto> {
    let v = std::env::var("GRID_CAREER_AUTO").ok()?;
    match v.strip_prefix('o') {
        Some(o) => o.parse().ok().map(Auto::Offer),
        None => v.parse().ok().map(Auto::Event),
    }
}

/// What `GRID_CAREER_AUTO` takes: an event id, or `o` and a driver offer id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Auto {
    Event(i32),
    Offer(i32),
}

/// Scripted ticks of an unattended career race (the most `--drive-ticks`
/// allows).
#[cfg(not(target_os = "android"))]
const AUTO_TICKS: &str = "36000";

/// `grid-app --career ...` on the desktop: the career screen, then the race
/// it chose, again while the race leaves to the menu.
#[cfg(not(target_os = "android"))]
pub fn supervise(args: &[String]) -> Result<i32, Box<dyn std::error::Error>> {
    let exe = std::env::current_exe()?;
    let flags = crate::event_menu::passed_on(args);
    let dir = flags
        .windows(2)
        .rev()
        .find(|w| w[0] == "--data-dir")
        .map(|w| PathBuf::from(&w[1]))
        .unwrap_or_else(crate::settings::default_data_dir);
    loop {
        let screen = std::process::Command::new(&exe)
            .arg("--career-screen")
            .args(&flags)
            .status()?;
        if screen.code() != Some(CAREER_GO) {
            return Ok(screen.code().unwrap_or(1));
        }
        let result = dir.join(RESULT);
        let _ = std::fs::remove_file(&result);
        let mut race = std::process::Command::new(&exe);
        race.args(&flags)
            .arg("--settings")
            .arg(dir.join(PROFILE))
            .arg("--from-menu")
            .arg("--career-result")
            .arg(&result);
        if auto().is_some() {
            race.args(["--drive-ticks", AUTO_TICKS]);
        }
        let race = race.status()?;
        if race.code() != Some(crate::event_menu::BACK_TO_MENU) {
            return Ok(race.code().unwrap_or(1));
        }
    }
}

// ---- The screen ----------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
enum Page {
    Home,
    Event,
    Payout(Vec<String>),
    /// Teammate search and hiring.
    Team,
    /// Received sponsor offers and their slots.
    Sponsors,
    /// eBay Motors: used cars of the open events' models.
    Ebay,
    /// eBay Motors: the team's cars for sale.
    EbaySell,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    Enter(usize),
    TakeOffer(usize),
    OpenTeam,
    OpenSponsors,
    CycleRegion,
    CycleFee,
    Hire(i32),
    /// Moves a received sponsor offer to its next slot.
    CycleSponsor(i32),
    OpenEbay,
    OpenEbaySell,
    /// Buys `used[i]`.
    BuyUsed(usize),
    /// Lists the team's car `i` on eBay.
    ListCar(usize),
    Home,
    NextRace,
    Abandon,
    Continue,
    Quit,
}

#[derive(Resource)]
struct Screen {
    cat: Catalogue,
    career: Career,
    offers: Vec<Offer>,
    driver_offers: Vec<OfferRun>,
    current: Option<InProgress>,
    page: Page,
    cursor: usize,
    repeat: Repeat,
    dir: PathBuf,
    message: String,
    labels: Labels,
    root: PathBuf,
    /// Teammate search filters (None = any).
    team_region: Option<Region>,
    team_fee: Option<usize>,
    /// The eBay used cars on show (`Ebay::offers` of the shown models).
    used: Vec<grid_tools::career::ebay::UsedCar>,
}

impl Screen {
    fn actions(&self) -> Vec<(String, Action)> {
        match &self.page {
            Page::Home => {
                let mut rows: Vec<(String, Action)> = self
                    .offers
                    .iter()
                    .enumerate()
                    .map(|(i, o)| {
                        let e = &self.cat.events[&o.event_id];
                        (
                            format!(
                                "{} ({} races, {}, first ${})",
                                o.label,
                                o.races.len(),
                                o.car,
                                thousands(e.prize_money_for_first)
                            ),
                            Action::Enter(i),
                        )
                    })
                    .collect();
                rows.extend(
                    self.driver_offers
                        .iter()
                        .enumerate()
                        .map(|(i, o)| (format!("{} [{}]", o.label, o.car), Action::TakeOffer(i))),
                );
                if self.career.team_founded {
                    let mate = self
                        .career
                        .team_mate
                        .as_ref()
                        .map_or("none".to_owned(), |t| self.driver_name(t.driver_id));
                    rows.push((format!("Team mate: {mate}"), Action::OpenTeam));
                    rows.push((
                        format!(
                            "Sponsors: {} received, {} placed",
                            self.career.sponsor_offers.len(),
                            self.career.major_sponsor.iter().count()
                                + self.career.minor_sponsors.iter().flatten().count()
                        ),
                        Action::OpenSponsors,
                    ));
                    rows.push(("eBay Motors: used cars".into(), Action::OpenEbay));
                    rows.push((
                        format!("eBay Motors: sell a car ({} listed)", self.career.ebay.listings.len()),
                        Action::OpenEbaySell,
                    ));
                }
                rows.push(("Quit".into(), Action::Quit));
                rows
            }
            Page::Team => {
                let mut rows = vec![
                    (
                        format!(
                            "Region: {}",
                            self.team_region.map_or("Any", |r| REGIONS[r as usize].1)
                        ),
                        Action::CycleRegion,
                    ),
                    (
                        format!(
                            "Signing fee: {}",
                            self.team_fee
                                .and_then(|i| self.cat.fee_ranges.get(i))
                                .map_or("Any".to_owned(), |(lo, hi)| format!(
                                    "${} - ${}",
                                    thousands(*lo),
                                    thousands(*hi)
                                ))
                        ),
                        Action::CycleFee,
                    ),
                ];
                if self.career.can_hire(&self.cat) {
                    for h in self
                        .career
                        .search(&self.cat, self.team_region, self.team_fee)
                        .into_iter()
                        .take(ROWS - 3)
                    {
                        rows.push((
                            format!(
                                "{} {} - fee ${}, keeps {}% of winnings, rank {}",
                                h.first_name,
                                h.surname,
                                thousands(h.signing_fee),
                                h.winnings_percentage,
                                h.rank_id
                            ),
                            Action::Hire(h.driver_id),
                        ));
                    }
                }
                rows.push(("Back".into(), Action::Home));
                rows
            }
            Page::Sponsors => {
                let mut rows: Vec<(String, Action)> = self
                    .career
                    .sponsor_offers
                    .iter()
                    .take(ROWS - 1)
                    .map(|&o| (self.sponsor_row(o), Action::CycleSponsor(o)))
                    .collect();
                rows.push(("Back".into(), Action::Home));
                rows
            }
            Page::Event => {
                let e = self.current.as_ref();
                let mut rows = Vec::new();
                if let Some((n, next)) = e.and_then(|e| e.next().map(|r| (e.results.len() + 1, r))) {
                    rows.push((
                        format!(
                            "Race {n}: {} ({} laps, grid {})",
                            route_name(&next.route),
                            next.laps,
                            next.slot + 1
                        ),
                        Action::NextRace,
                    ));
                }
                rows.push(("Abandon event".into(), Action::Abandon));
                rows.push(("Quit".into(), Action::Quit));
                rows
            }
            Page::Payout(_) => vec![("Continue".into(), Action::Continue)],
            Page::Ebay => {
                let mut rows: Vec<(String, Action)> = self
                    .used
                    .iter()
                    .enumerate()
                    .take(ROWS - 1)
                    .map(|(i, u)| (self.used_row(u), Action::BuyUsed(i)))
                    .collect();
                rows.push(("Back".into(), Action::Home));
                rows
            }
            Page::EbaySell => {
                let mut rows: Vec<(String, Action)> = self
                    .career
                    .cars
                    .iter()
                    .enumerate()
                    .take(ROWS - 1)
                    .map(|(i, c)| (self.sell_row(c), Action::ListCar(i)))
                    .collect();
                rows.push(("Back".into(), Action::Home));
                rows
            }
        }
    }

    fn stem(&self, vehicle_id: i32) -> String {
        self.cat.vehicles.get(&vehicle_id).map_or(format!("car {vehicle_id}"), |v| v.stem.clone())
    }

    /// One eBay offer: the history the buy screen shows (`ebay_buy_screen_*`
    /// [D]) and the Buy It Now price.
    fn used_row(&self, u: &grid_tools::career::ebay::UsedCar) -> String {
        format!(
            "{}: {} miles, {} events, {} event wins, {} race wins, {} wrecked, performance {:+.1}% - ${} (seller {})",
            self.stem(u.vehicle_id),
            thousands(u.miles as i64),
            u.events,
            u.event_wins,
            u.race_wins,
            u.major_crashes,
            u.performance_percent,
            thousands(u.price),
            u.seller.map_or("?".into(), |d| self.driver_name(d))
        )
    }

    /// A team car on the sell page: its listing, or the AUTHORED listing
    /// the row creates.
    fn sell_row(&self, c: &grid_tools::career::state::OwnedCar) -> String {
        let v = self.cat.vehicles.get(&c.vehicle_id);
        match self.career.ebay.listings.iter().find(|l| l.vehicle_id == c.vehicle_id) {
            Some(l) => format!(
                "{}: listed, {} - bid ${}, {} event(s) left (start ${}, fixed ${})",
                self.stem(c.vehicle_id),
                listing_state(l.state),
                thousands(l.bid),
                l.remaining,
                thousands(l.start_price),
                thousands(l.fixed_price)
            ),
            None => format!(
                "Sell {} on eBay: {} events, start ${}, fixed ${}",
                self.stem(c.vehicle_id),
                self.cat.ebay.max_auction_length,
                thousands(v.map_or(0, |v| v.sale_price)),
                thousands(v.map_or(0, |v| v.purchase_price))
            ),
        }
    }

    /// The used cars of the models the open events race and the team does
    /// not own (the original offers "Buy On eBay" for a model the player
    /// does not own on the event's car list [D]; which models its eBay list
    /// shows was not traced).
    fn refresh_used(&mut self) {
        let mut models: Vec<i32> = Vec::new();
        for o in &self.offers {
            for &v in self.cat.event(o.event_id).map(|e| e.vehicles.as_slice()).unwrap_or_default() {
                let raceable = self
                    .cat
                    .vehicles
                    .get(&v)
                    .is_some_and(|x| x.purchasable && o.field_cars.contains(&x.stem));
                if raceable && !models.contains(&v) && !self.career.cars.iter().any(|c| c.vehicle_id == v) {
                    models.push(v);
                }
            }
        }
        let mate = self.career.team_mate.as_ref().map(|t| t.driver_id);
        self.used = models
            .into_iter()
            .flat_map(|v| self.career.ebay.offers(&self.cat, v, mate))
            .collect();
    }

    /// The eBay lines of a career event's end (the popups, `ebay_msg_*`).
    fn ebay_lines(&mut self) -> Vec<String> {
        let outcomes = self.career.ebay_after_event(&self.cat);
        outcomes
            .iter()
            .map(|o| {
                let l = &o.listing;
                log_line!(
                    "drive_career version=1 event=ebay vehicle={} state={:?} bid={} paid={:?} money={}",
                    l.vehicle_id,
                    l.state,
                    l.bid,
                    o.paid,
                    self.career.money
                );
                match o.paid {
                    Some(amount) => format!(
                        "eBay: your {} sold for ${}; it has been removed from your garage.",
                        self.stem(l.vehicle_id),
                        thousands(amount)
                    ),
                    None => format!(
                        "eBay: {} - {}{}",
                        self.stem(l.vehicle_id),
                        listing_state(l.state),
                        if l.remaining > 0 { format!(", {} event(s) left", l.remaining) } else { String::new() }
                    ),
                }
            })
            .collect()
    }

    fn driver_name(&self, driver_id: i32) -> String {
        self.cat
            .hireable
            .iter()
            .find(|h| h.driver_id == driver_id)
            .map_or(format!("driver {driver_id}"), |h| format!("{} {}", h.first_name, h.surname))
    }

    fn sponsor_name(&self, sponsor_id: i32) -> String {
        let Some(s) = self.cat.sponsors.get(&sponsor_id) else {
            return format!("sponsor {sponsor_id}");
        };
        let key = format!("db_{}", s.name);
        let text = self.labels.get(&key);
        if text == key { s.name.clone() } else { text.to_owned() }
    }

    /// "Sponsor: objective, $ per race - slot".
    fn sponsor_row(&self, offer_id: i32) -> String {
        use grid_tools::career::data::SponsorObjective as O;
        let Some(o) = self.cat.sponsor_offer(offer_id) else {
            return format!("offer {offer_id}");
        };
        let objective = match o.objective {
            O::FinishingPlace => format!("finish no lower than {}", o.objective_details),
            O::FinishingPlaceNoDamage => {
                format!("finish no lower than {} with no damage", o.objective_details)
            }
            O::Finish => "finish the race".into(),
            O::FinishNoDamage => "finish the race with no damage".into(),
        };
        let slot = if self.career.major_sponsor == Some(offer_id) {
            "Major (pays double)".to_owned()
        } else if let Some(i) = self.career.minor_sponsors.iter().position(|s| *s == Some(offer_id)) {
            format!("Minor {}", i + 1)
        } else {
            "not on the car".into()
        };
        format!(
            "{}: {objective}, ${} per race - {slot}",
            self.sponsor_name(o.sponsor_id),
            thousands(o.money_reward)
        )
    }

    fn info(&self) -> Vec<String> {
        let mut lines = status_lines(&self.cat, &self.career);
        match &self.page {
            Page::Home => {
                lines.push(String::new());
                if !self.career.team_founded {
                    lines.push(format!(
                        "No race team yet: drive for other teams until the bank reaches ${} to found your own.",
                        thousands(self.cat.start_money)
                    ));
                }
                lines.push(match (self.offers.is_empty(), self.driver_offers.is_empty()) {
                    (true, true) => "No open event or driver offer can run in the port yet.".into(),
                    (false, true) => "Open events:".into(),
                    (true, false) => "Driver offers:".into(),
                    (false, false) => "Open events, then driver offers:".into(),
                });
            }
            Page::Event => {
                if let Some(e) = &self.current {
                    lines.push(String::new());
                    let name = self.cat.event(e.event_id).map_or("?", |ev| ev.name.as_str());
                    match e.offer.and_then(|o| self.cat.driver_offers.get(&o)) {
                        Some(o) => lines.push(format!(
                            "Driver offer {} ({name}), car {}: {}",
                            o.id,
                            e.car,
                            objective_text(&self.cat, o, &self.labels)
                        )),
                        None => lines.push(format!("Event {name}, car {}", e.car)),
                    }
                    for (i, r) in e.races.iter().enumerate() {
                        let place = e.results.get(i).and_then(|res| res.position(Entrant::Player));
                        lines.push(format!(
                            "    Race {}: {}{}",
                            i + 1,
                            route_name(&r.route),
                            place.map_or(String::new(), |p| format!(" - finished {}", p + 1))
                        ));
                    }
                    if let Some(points) = self.event_points(e)
                        && !e.results.is_empty()
                    {
                        lines.push("Event Standings - Drivers:".into());
                        let mate = self
                            .career
                            .team_mate
                            .as_ref()
                            .map_or_else(|| "Team mate".into(), |t| self.driver_name(t.driver_id));
                        lines.extend(standings_lines(&points, e, &mate));
                    }
                    // The last race's sponsor objectives (paid with the event).
                    if e.offer.is_none()
                        && let Some(last) = e.results.last()
                    {
                        for p in self.career.sponsor_pay(&self.cat, last) {
                            let met = |b: bool| if b { "met" } else { "failed" };
                            let mate = if self.career.team_mate.is_some() {
                                format!(", team mate {}", met(p.teammate_met))
                            } else {
                                String::new()
                            };
                            lines.push(format!(
                                "    Sponsor {}{}: you {}{mate} - ${}",
                                self.sponsor_name(p.sponsor_id),
                                if p.major { " (major)" } else { "" },
                                met(p.player_met),
                                thousands(p.money)
                            ));
                        }
                    }
                }
            }
            Page::Payout(p) => {
                lines.push(String::new());
                lines.extend(p.iter().cloned());
            }
            Page::Team => {
                lines.push(String::new());
                match &self.career.team_mate {
                    Some(t) => lines.push(format!(
                        "Team mate: {}, keeps {}% of his prize money. Hiring another replaces him (no refund).",
                        self.driver_name(t.driver_id),
                        t.winnings_percentage
                    )),
                    None => lines.push("You currently do not have a team mate.".into()),
                }
                if !self.career.can_hire(&self.cat) {
                    lines.push(
                        "We're not ready to hire a team mate yet. Concentrate on increasing your reputation for now (first licence) [I]."
                            .into(),
                    );
                } else {
                    lines.push("Applicants, cheapest first (choose Region / Signing fee to filter):".into());
                }
            }
            Page::Sponsors => {
                lines.push(String::new());
                lines.push(if self.career.sponsor_offers.is_empty() {
                    "Finish in the top three and sponsorship offers will start rolling in.".into()
                } else {
                    "Select an offer to move it: not on the car, Major (pays double), Minor 1-7.".into()
                });
            }
            Page::Ebay => {
                lines.push(String::new());
                lines.push(if self.used.is_empty() {
                    "No used cars of the open events' models on offer (or you own them all).".into()
                } else {
                    "Used cars at their Buy It Now price; select one to buy it. The offers change after every event.".into()
                });
            }
            Page::EbaySell => {
                lines.push(String::new());
                lines.push(format!(
                    "Select a car to list it. Each event a bid of 70, 80, 90, 100 % of its value comes in; at the fixed price it sells at once. Sold so far: {} for ${}.",
                    self.career.ebay.sold,
                    thousands(self.career.ebay.earned)
                ));
            }
        }
        if !self.message.is_empty() {
            lines.push(self.message.clone());
        }
        lines
    }

    fn save(&self) -> Result<(), String> {
        save::store(&self.dir.join(SAVE), &self.career).map_err(|e| e.to_string())
    }

    /// Adds a race result waiting from the race, and pays a finished event.
    fn collect(&mut self) -> Result<(), String> {
        let result_path = self.dir.join(RESULT);
        let event_path = self.dir.join(EVENT);
        if let (Some(e), Ok(text)) = (&mut self.current, std::fs::read_to_string(&result_path)) {
            let mut out = read_race_out(&text)?;
            let race = e.next().cloned().unwrap_or_else(|| Planned {
                route: String::new(),
                laps: 0,
                slot: 0,
                layout: 0,
                cars: Vec::new(),
            });
            if !e.field.is_empty() {
                to_field(&mut out.result, &e.field, &race);
            } else if e.offer.is_none() && self.career.team_mate.is_some() {
                teammate_car(&mut out.result);
            }
            if race.layout > 0 {
                self.career.count_race(race.layout);
            }
            log_line!(
                "drive_career version=1 event=race_added event_id={} race={} position={}",
                e.event_id,
                e.results.len() + 1,
                out.result.position(Entrant::Player).unwrap_or(0) + 1
            );
            e.add(out);
            std::fs::write(&event_path, e.write()).map_err(|err| err.to_string())?;
            let _ = std::fs::remove_file(&result_path);
        }
        let Some(e) = self.current.clone() else {
            return Ok(());
        };
        if !e.done() {
            self.page = Page::Event;
            return Ok(());
        }
        if let Some(offer_id) = e.offer {
            let out = e.last_out().ok_or("driver offer without a result")?;
            let met = offer_met(&self.cat, offer_id, &out, e.field.is_empty());
            let payout = apply_driver_offer(
                &self.cat,
                &mut self.career,
                offer_id,
                met,
                &e.results,
                &e.settings.unwrap_or_default(),
            )?;
            let ebay = self.ebay_lines();
            self.save()?;
            let _ = std::fs::remove_file(&event_path);
            log_line!(
                "drive_career version=1 event=offer_paid offer={offer_id} met={met} position={} best_lap={:?} top_mph={:?} wages={} bonus={} flashback={} reputation={} money={} team_founded={}",
                out.result.position(Entrant::Player).map_or(0, |p| p + 1),
                out.best_lap,
                out.top_mph,
                payout.driver_offer_wages,
                payout.driver_offer_bonus,
                payout.flashback_bonus,
                payout.reputation(),
                self.career.money,
                self.career.team_founded
            );
            let mut lines = vec![format!(
                "Objective: {}",
                if met { "achieved" } else { "failed" }
            )];
            lines.extend(payout_lines(&payout, &self.career));
            lines.extend(ebay);
            self.current = None;
            self.page = Page::Payout(lines);
            return Ok(());
        }
        let settings = e.settings.unwrap_or_default();
        let payout = apply_event(&self.cat, &mut self.career, e.event_id, &e.results, &settings)?;
        let ebay = self.ebay_lines();
        self.save()?;
        let _ = std::fs::remove_file(&event_path);
        log_line!(
            "drive_career version=1 event=paid event_id={} position={} prize={} team_mate={} team_win={} flashback={} sponsors={} reputation={} money={} total_reputation={}",
            e.event_id,
            payout.position.map_or(0, |p| p + 1),
            payout.prize,
            payout.team_mate_prize,
            payout.team_win_bonus,
            payout.flashback_bonus,
            payout.sponsor_money,
            payout.reputation(),
            self.career.money,
            self.career.total_reputation()
        );
        self.current = None;
        let mut lines = payout_lines(&payout, &self.career);
        lines.extend(ebay);
        self.page = Page::Payout(lines);
        Ok(())
    }

    fn refresh_offers(&mut self, routes: &BTreeMap<i32, String>, labels: &Labels, root: &Path) {
        self.offers = offers(&self.cat, &self.career, routes, labels, |p| root.join(p).exists());
        self.driver_offers = offer_runs(&self.cat, &self.career, routes, labels, |p| root.join(p).exists());
    }

    /// Takes a driver offer: one race in the offer's car, solo for a test
    /// offer (AUTHORED: the original's test session is a separate mode).
    fn take_offer(&mut self, index: usize) -> Result<(), String> {
        let run = self.driver_offers.get(index).ok_or("no such offer")?.clone();
        let offer = &self.cat.driver_offers[&run.offer_id];
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_secs() as u32);
        let mut rng = grid_tools::series::Mt::new(seed);
        let (race, route) = &run.race;
        // AUTHORED: an offer of a Le Mans selection event keeps the event's
        // cars as its field (`lemans_restrictions`, 0x663860, not ported).
        let lemans = self.cat.event(offer.event_id).is_some_and(|e| e.use_lemans_selection);
        let test = grid_tools::career::rules::is_test_offer(offer);
        let (field, races) = if lemans || test {
            let planned = Planned {
                route: route.clone(),
                laps: race.laps,
                slot: grid_tools::race::grid_slot(
                    race.player_grid_position_base.max(0) as u32,
                    race.player_grid_position_range.max(0) as u32,
                    FIELD,
                    |n| rng.range(0, n),
                ),
                layout: race.track_model_id,
                cars: Vec::new(),
            };
            (Vec::new(), vec![planned])
        } else {
            let root = self.root.clone();
            career_plan(
                &self.cat,
                &self.career,
                grid_tools::career::field::Entry::Offer(run.offer_id),
                &run.car,
                livery_index(&offer.livery),
                std::slice::from_ref(&run.race),
                |p| root.join(p).exists(),
                seed,
            )
        };
        let e = InProgress {
            offer: Some(run.offer_id),
            event_id: offer.event_id,
            car: run.car.clone(),
            field_cars: run.field_cars.clone(),
            races,
            field,
            results: Vec::new(),
            settings: None,
            best_lap: None,
            top_mph: None,
        };
        std::fs::write(self.dir.join(EVENT), e.write()).map_err(|err| err.to_string())?;
        log_line!(
            "drive_career version=1 event=offer_taken offer={} car={} route={} laps={} grid={} seed={seed}",
            run.offer_id,
            run.car,
            route,
            race.laps,
            e.races[0].slot + 1
        );
        self.current = Some(e);
        self.page = Page::Event;
        self.cursor = 0;
        Ok(())
    }

    fn start(&mut self, index: usize) -> Result<(), String> {
        let offer = self.offers.get(index).ok_or("no such event")?.clone();
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_secs() as u32);
        let root = self.root.clone();
        let (field, races) = career_plan(
            &self.cat,
            &self.career,
            grid_tools::career::field::Entry::Event(offer.event_id),
            &offer.car,
            0,
            &offer.races,
            |p| root.join(p).exists(),
            seed,
        );
        let e = InProgress {
            offer: None,
            event_id: offer.event_id,
            car: offer.car.clone(),
            field_cars: offer.field_cars.clone(),
            races,
            field,
            results: Vec::new(),
            settings: None,
            best_lap: None,
            top_mph: None,
        };
        std::fs::write(self.dir.join(EVENT), e.write()).map_err(|err| err.to_string())?;
        log_line!(
            "drive_career version=1 event=entered event_id={} car={} races={} seed={seed}",
            e.event_id,
            e.car,
            e.races
                .iter()
                .map(|r| format!("{}:{}:{}", r.route, r.laps, r.slot + 1))
                .collect::<Vec<_>>()
                .join(",")
        );
        self.current = Some(e);
        self.page = Page::Event;
        self.cursor = 0;
        Ok(())
    }

    /// A test offer runs solo (AUTHORED).
    fn solo(&self, e: &InProgress) -> bool {
        e.offer
            .and_then(|o| self.cat.driver_offers.get(&o))
            .is_some_and(grid_tools::career::rules::is_test_offer)
    }

    /// An own-team event's points table (none for driver offers and
    /// head-to-heads, which pay without standings).
    fn event_points(&self, e: &InProgress) -> Option<Vec<i32>> {
        let event = self.cat.event(e.event_id).filter(|ev| e.offer.is_none() && !ev.head_to_head)?;
        self.cat.points.get(&event.points_id).cloned()
    }

    /// Writes the next race's profile.
    fn go(&self) -> Result<(), String> {
        let e = self.current.as_ref().ok_or("no event")?;
        let next = e.next().ok_or("event finished")?;
        let mut profile = if e.field.is_empty() {
            race_profile(&e.car, &e.field_cars, next, self.solo(e))
        } else {
            field_profile(&e.field, next)
        };
        if let Some(points) = self.event_points(e) {
            profile = with_points(&profile, &points);
        }
        std::fs::write(self.dir.join(PROFILE), profile).map_err(|err| err.to_string())?;
        log_line!(
            "drive_career version=1 event=go event_id={} race={} route={} laps={} grid={}",
            e.event_id,
            e.results.len() + 1,
            next.route,
            next.laps,
            next.slot + 1
        );
        Ok(())
    }
}

fn route_name(route: &str) -> String {
    let mut parts = route.rsplit('/');
    let r = parts.next().unwrap_or("");
    let t = parts.next().unwrap_or("");
    format!("{t} {r}")
}

#[derive(Resource)]
struct Lookup {
    routes: BTreeMap<i32, String>,
    root: PathBuf,
}

/// Runs the career screen as its own app (`--career-screen`).
pub fn screen(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let profile = crate::settings::load(args)?;
    let root = crate::game_root(profile.root.clone())?;
    println!("game_root path={}", root.display());
    let labels = Labels::load(&root);
    let schema = std::fs::read(root.join("database/schema.bin"))?;
    let data = std::fs::read(root.join("database/database.bin"))?;
    let db = grid_formats::database::read(&schema, &data)?;
    let routes = track_routes(&db);
    let cat = Catalogue::load(&root)?;
    let dir = crate::settings::data_dir().to_path_buf();
    std::fs::create_dir_all(&dir)?;
    let save_path = dir.join(SAVE);
    let career = if save_path.is_file() {
        save::load(&save_path)?
    } else {
        let c = new_career(&cat);
        save::store(&save_path, &c)?;
        log_line!("drive_career version=1 event=new_career team_founded={} money={}", c.team_founded, c.money);
        c
    };
    let current = match std::fs::read_to_string(dir.join(EVENT)) {
        Ok(text) => Some(InProgress::read(&text)?),
        Err(_) => None,
    };
    let mut screen = Screen {
        cat,
        career,
        offers: Vec::new(),
        driver_offers: Vec::new(),
        current,
        page: Page::Home,
        cursor: 0,
        repeat: Repeat::default(),
        dir,
        message: String::new(),
        labels: Labels::load(&root),
        root: root.clone(),
        team_region: None,
        team_fee: None,
        used: Vec::new(),
    };
    screen.collect()?;
    screen.refresh_offers(&routes, &labels, &root);
    log_line!(
        "drive_career version=1 event=open money={} reputation={:?} licences={} season={} offers={}",
        screen.career.money,
        screen.career.reputation,
        screen.career.licences(&screen.cat),
        screen.career.season_number,
        screen
            .offers
            .iter()
            .map(|o| format!("{}:{}", o.event_id, o.car))
            .collect::<Vec<_>>()
            .join(",")
    );
    let input_settings = profile.input()?;
    let (actions, pad) = crate::input::load(&root, &input_settings, false)?;
    println!("drive_input {input_settings} pad_preset={pad} menu=on");
    let mut app = App::new();
    app.add_plugins(crate::with_profile(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "GRID Reassembly - Career".into(),
            resolution: (1280, 800).into(),
            present_mode: profile.present_mode.unwrap_or_default(),
            ..default()
        }),
        ..default()
    })));
    crate::input::install(&mut app, actions);
    let exit = app
        .insert_resource(screen)
        .insert_resource(Lookup { routes, root })
        .insert_resource(labels)
        .insert_resource(ClearColor(Color::srgb(0.055, 0.07, 0.095)))
        .add_systems(Startup, spawn)
        .add_systems(Update, (update, present).chain())
        .run();
    match exit {
        AppExit::Error(code) => std::process::exit(i32::from(code.get())),
        AppExit::Success => Ok(()),
    }
}

#[derive(Component)]
struct Info;
#[derive(Component)]
struct RowText(usize);

const ROWS: usize = 18;

fn spawn(mut commands: Commands) {
    let camera = commands.spawn((Camera2d, crate::ViewerCamera)).id();
    let font = |size: f32| TextFont {
        font_size: FontSize::Px(size),
        ..default()
    };
    commands
        .spawn((
            Node {
                width: percent(100.),
                height: percent(100.),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            UiTargetCamera(camera),
        ))
        .with_children(|screen| {
            screen
                .spawn((
                    Node {
                        flex_direction: FlexDirection::Column,
                        padding: UiRect::all(px(24.)),
                        row_gap: px(6.),
                        min_width: px(620.),
                        max_width: px(900.),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.025, 0.04, 0.065, 0.92)),
                ))
                .with_children(|panel| {
                    panel.spawn((Text::new("Career"), font(30.), TextColor(Color::WHITE)));
                    panel.spawn((Info, Text::new(""), font(17.), TextColor(Color::srgb(0.75, 0.8, 0.85))));
                    for i in 0..ROWS {
                        panel.spawn((RowText(i), Button, Text::new(""), font(21.), TextColor(Color::WHITE)));
                    }
                    panel.spawn((
                        Text::new("Up/Down choose, Enter select, Escape quit. Career races are desktop-only and AUTHORED where docs/CAREER_SCREEN.md says so."),
                        font(14.),
                        TextColor(Color::srgb(0.6, 0.65, 0.7)),
                    ));
                });
        });
}

#[allow(clippy::too_many_arguments)]
fn update(
    mut screen: ResMut<Screen>,
    lookup: Res<Lookup>,
    labels: Res<Labels>,
    actions: Res<crate::input::Actions>,
    clicks: Query<(&Interaction, &RowText), Changed<Interaction>>,
    time: Res<Time<Real>>,
    mut exit: MessageWriter<AppExit>,
    mut done: Local<bool>,
    mut auto_wait: Local<f32>,
) {
    if *done {
        return;
    }
    let mut repeat = screen.repeat;
    let nav: Nav = crate::pause_menu::nav(&actions, &mut repeat, time.delta_secs());
    screen.repeat = repeat;
    let rows = screen.actions();
    let n = rows.len().max(1);
    let mut chosen = None;
    if nav.up {
        screen.cursor = (screen.cursor + n - 1) % n;
    }
    if nav.down {
        screen.cursor = (screen.cursor + 1) % n;
    }
    screen.cursor = screen.cursor.min(n - 1);
    if nav.select {
        chosen = rows.get(screen.cursor).map(|r| r.1);
    }
    if nav.back || nav.pause {
        chosen = Some(if matches!(screen.page, Page::Payout(_) | Page::Team | Page::Sponsors) {
            Action::Continue
        } else {
            Action::Quit
        });
    }
    for (interaction, row) in &clicks {
        if *interaction == Interaction::Pressed && row.0 < rows.len() {
            if screen.cursor == row.0 {
                chosen = Some(rows[row.0].1);
            }
            screen.cursor = row.0;
        }
    }
    // Diagnostics GRID_CAREER_AUTO: each page acts by itself after 3 s (time
    // for a screenshot).
    if chosen.is_none()
        && let Some(target) = auto()
    {
        *auto_wait += time.delta_secs();
        if *auto_wait >= 3. {
            *auto_wait = 0.;
            chosen = Some(match &screen.page {
                Page::Home => match target {
                    Auto::Event(id) => screen
                        .offers
                        .iter()
                        .position(|o| o.event_id == id)
                        .map_or(Action::Quit, Action::Enter),
                    Auto::Offer(id) => screen
                        .driver_offers
                        .iter()
                        .position(|o| o.offer_id == id)
                        .map_or(Action::Quit, Action::TakeOffer),
                },
                Page::Event if screen.current.as_ref().is_some_and(|e| e.next().is_some()) => {
                    Action::NextRace
                }
                Page::Event | Page::Payout(_) => Action::Quit,
                Page::Team | Page::Sponsors | Page::Ebay | Page::EbaySell => Action::Home,
            });
        }
    }
    let Some(action) = chosen else {
        return;
    };
    screen.message.clear();
    let result = match action {
        Action::Enter(i) => screen.start(i).and_then(|()| screen.go()).map(|()| true),
        Action::TakeOffer(i) => screen.take_offer(i).and_then(|()| screen.go()).map(|()| true),
        Action::NextRace => screen.go().map(|()| true),
        Action::Abandon => {
            let _ = std::fs::remove_file(screen.dir.join(EVENT));
            log_line!("drive_career version=1 event=abandoned");
            screen.current = None;
            screen.page = Page::Home;
            screen.cursor = 0;
            Ok(false)
        }
        Action::Continue | Action::Home => {
            screen.page = Page::Home;
            screen.cursor = 0;
            Ok(false)
        }
        Action::OpenTeam => {
            screen.page = Page::Team;
            screen.cursor = 0;
            Ok(false)
        }
        Action::OpenSponsors => {
            screen.page = Page::Sponsors;
            screen.cursor = 0;
            Ok(false)
        }
        Action::OpenEbay => {
            screen.refresh_used();
            screen.page = Page::Ebay;
            screen.cursor = 0;
            Ok(false)
        }
        Action::OpenEbaySell => {
            screen.page = Page::EbaySell;
            screen.cursor = 0;
            Ok(false)
        }
        Action::BuyUsed(i) => {
            let s = &mut *screen;
            match s.used.get(i).cloned() {
                Some(u) => s.career.buy_used(&s.cat, &u).and_then(|()| s.save()).map(|()| {
                    log_line!(
                        "drive_career version=1 event=ebay_bought vehicle={} price={} money={}",
                        u.vehicle_id,
                        u.price,
                        s.career.money
                    );
                    s.message = format!("Bought a used {} for ${}.", s.stem(u.vehicle_id), thousands(u.price));
                    s.refresh_used();
                    false
                }),
                None => Ok(false),
            }
        }
        Action::ListCar(i) => {
            let s = &mut *screen;
            match s.career.cars.get(i).cloned() {
                Some(car) => {
                    // AUTHORED: the port has no price entry; start at the
                    // dealer's sale price, fixed at the new price, the
                    // longest duration.
                    let (start, fixed) = s
                        .cat
                        .vehicles
                        .get(&car.vehicle_id)
                        .map_or((0, 0), |v| (v.sale_price, v.purchase_price));
                    let duration = s.cat.ebay.max_auction_length;
                    s.career.list_on_ebay(&s.cat, i, start, fixed, duration).and_then(|()| s.save()).map(|()| {
                        log_line!(
                            "drive_career version=1 event=ebay_listed vehicle={} start={start} fixed={fixed} duration={duration}",
                            car.vehicle_id
                        );
                        s.message = format!("{} listed on eBay.", s.stem(car.vehicle_id));
                        false
                    })
                }
                None => Ok(false),
            }
        }
        Action::CycleRegion => {
            screen.team_region = match screen.team_region {
                None => Some(Region::Europe),
                Some(Region::Europe) => Some(Region::UnitedStates),
                Some(Region::UnitedStates) => Some(Region::Japan),
                Some(_) => None,
            };
            Ok(false)
        }
        Action::CycleFee => {
            let n = screen.cat.fee_ranges.len();
            screen.team_fee = match screen.team_fee {
                None if n > 0 => Some(0),
                Some(i) if i + 1 < n => Some(i + 1),
                _ => None,
            };
            Ok(false)
        }
        Action::Hire(driver) => {
            let s = &mut *screen;
            s.career
                .hire_driver(&s.cat, driver)
                .and_then(|()| s.save())
                .map(|()| {
                    log_line!("drive_career version=1 event=hired driver={driver} money={}", s.career.money);
                    s.message = format!("Hired {}.", s.driver_name(driver));
                    false
                })
        }
        Action::CycleSponsor(offer) => {
            let s = &mut *screen;
            let next = if s.career.major_sponsor == Some(offer) {
                Some(Some(0))
            } else {
                match s.career.minor_sponsors.iter().position(|o| *o == Some(offer)) {
                    Some(i) if i + 1 < s.career.minor_sponsors.len() => Some(Some(i + 1)),
                    Some(_) => None,
                    None => Some(None),
                }
            };
            let placed = match next {
                Some(slot) => s.career.place_sponsor(offer, slot),
                None => {
                    s.career.remove_sponsor(offer);
                    Ok(())
                }
            };
            placed.and_then(|()| s.save()).map(|()| false)
        }
        Action::Quit => {
            *done = true;
            log_line!("drive_career version=1 event=quit");
            exit.write(AppExit::Success);
            return;
        }
    };
    match result {
        Ok(true) => {
            *done = true;
            exit.write(AppExit::from_code(CAREER_GO as u8));
        }
        Ok(false) => {
            screen.refresh_offers(&lookup.routes, &labels, &lookup.root);
        }
        Err(e) => {
            warn!("career: {e}");
            screen.message = format!("Error: {e}");
        }
    }
}

#[allow(clippy::type_complexity)]
fn present(
    screen: Res<Screen>,
    mut info: Query<&mut Text, (With<Info>, Without<RowText>)>,
    mut rows: Query<(&RowText, &mut Text, &mut TextColor, &mut Node), Without<Info>>,
    mut shown: Local<Option<(Vec<String>, Vec<String>, usize)>>,
) {
    let lines = screen.info();
    let actions: Vec<String> = screen.actions().into_iter().map(|a| a.0).collect();
    let view = (lines, actions, screen.cursor);
    if shown.as_ref() == Some(&view) {
        return;
    }
    for mut text in &mut info {
        text.0 = view.0.join("\n");
    }
    for (row, mut text, mut colour, mut node) in &mut rows {
        let Some(label) = view.1.get(row.0) else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        let selected = row.0 == view.2;
        text.0 = format!("{}{label}", if selected { "> " } else { "   " });
        colour.0 = if selected {
            Color::srgb(1., 0.78, 0.2)
        } else {
            Color::WHITE
        };
    }
    *shown = Some(view);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(order: &[i32], aids_off: bool) -> RaceOut {
        RaceOut {
            result: RaceResult {
                order: order.iter().map(|&c| classified(&c.to_string()).unwrap()).collect(),
                flashbacks: None,
            },
            settings: RaceSettings {
                star_level: star_level(),
                traction_control: !aids_off,
                steering_assist: !aids_off,
                braking_assist: !aids_off,
                manual_gears: aids_off,
                ..RaceSettings::default()
            },
            best_lap: None,
            top_mph: None,
        }
    }

    #[test]
    fn race_results_and_events_round_trip() {
        let mut o = out(&[3, 0, 1, 2], true);
        o.result.order[3].finished = false;
        o.result.flashbacks = Some(Flashbacks { used: 1, maximum: 4 });
        let back = read_race_out(&write_race_out(&o)).unwrap();
        assert_eq!(back.result.flashbacks, o.result.flashbacks);
        assert!(!back.result.order[3].finished);
        assert_eq!(back.result.order[3].entrant, Entrant::Ai { driver: 2, team: 2 });
        assert_eq!(back.settings, o.settings);
        assert_eq!(back.result.position(Entrant::Player), Some(1));
        assert!(read_race_out("grid-career-race 1\nsettings star=2\norder 1 2\n").is_err());

        let mut e = InProgress {
            offer: None,
            event_id: 7,
            car: "bmw".into(),
            field_cars: vec!["bmw".into()],
            races: vec![
                Planned {
                    route: "tracks/europe/nurburgring/route_1".into(),
                    laps: 3,
                    slot: 11,
                    layout: 24,
                    cars: vec![2, 1],
                },
                Planned { route: "tracks/europe/nurburgring/route_2".into(), laps: 2, slot: 11, layout: 99, cars: vec![] },
            ],
            field: vec![
                FieldEntry { entrant: Entrant::Player, stem: "bmw".into(), livery: 0, name: "You".into() },
                FieldEntry {
                    entrant: Entrant::Ai { driver: 238, team: 116 },
                    stem: "bmw".into(),
                    livery: 3,
                    name: "Lee Niven".into(),
                },
                FieldEntry { entrant: Entrant::TeamMate, stem: "bmw".into(), livery: 5, name: "Ann Example".into() },
            ],
            results: Vec::new(),
            settings: None,
            best_lap: None,
            top_mph: None,
        };
        assert_eq!(InProgress::read(&e.write()).unwrap(), e);
        e.add(back.clone());
        assert!(!e.done());
        assert_eq!(e.next().unwrap().laps, 2);
        e.add(back.clone());
        e.add(back);
        assert!(e.done());
        assert_eq!(e.results.len(), 2);
        assert_eq!(InProgress::read(&e.write()).unwrap(), e);

        // A driver offer keeps its id, best lap and top speed.
        let mut o = out(&[0], false);
        o.best_lap = Some(106.25);
        o.top_mph = Some(151.5);
        let back = read_race_out(&write_race_out(&o)).unwrap();
        assert_eq!((back.best_lap, back.top_mph), (Some(106.25), Some(151.5)));
        let mut e = InProgress {
            offer: Some(16),
            races: vec![Planned {
                route: "tracks/europe/nurburgring/route_0".into(),
                laps: 3,
                slot: 0,
                layout: 0,
                cars: vec![],
            }],
            field: Vec::new(),
            ..e
        };
        e.results.clear();
        e.add(back);
        assert!(e.done());
        let again = InProgress::read(&e.write()).unwrap();
        assert_eq!(again, e);
        assert_eq!(again.last_out().unwrap().best_lap, Some(106.25));
    }

    #[test]
    fn the_race_profile_is_a_twelve_car_race_of_the_event_cars() {
        let p = race_profile(
            "bmw",
            &["bmw".into()],
            &Planned { route: "tracks/europe/nurburgring/route_1".into(), laps: 3, slot: 11, layout: 0, cars: vec![] },
            false,
        );
        let mut s = crate::settings::Settings::new();
        s.apply(&p, Path::new(".")).unwrap();
        assert_eq!((s.laps, s.opponents, s.grid_position), (Some(3), Some(11), Some(12)));
        assert_eq!(s.opponent_cars.as_deref(), Some("bmw"));
        // The catch-up follows the difficulty, as the launchers (F439).
        assert!(p.contains("difficulty = 0.606\ncatchup = on\n"));
        // A test offer runs alone, from the front.
        let solo = race_profile(
            "bmw",
            &["bmw".into()],
            &Planned { route: "tracks/europe/nurburgring/route_0".into(), laps: 3, slot: 7, layout: 0, cars: vec![] },
            true,
        );
        let mut s = crate::settings::Settings::new();
        s.apply(&solo, Path::new(".")).unwrap();
        assert_eq!((s.opponents, s.laps), (None, Some(3)));
    }

    #[test]
    fn points_standings_and_bank_lines() {
        // The profile carries the event's points table.
        let race = Planned { route: "tracks/europe/nurburgring/route_1".into(), laps: 3, slot: 11, layout: 0, cars: vec![] };
        let p = with_points(&race_profile("bmw", &["bmw".into()], &race, false), &[20, 18, 16]);
        let mut s = crate::settings::Settings::new();
        s.apply(&p, Path::new(".")).unwrap();
        assert_eq!(s.points, Some(vec![20, 18, 16]));
        assert_eq!(s.laps, Some(3));
        // Standings over two races: names from the field, dense ranks.
        let mate = Entrant::TeamMate;
        let rival = Entrant::Ai { driver: 238, team: 116 };
        let e = InProgress {
            offer: None,
            event_id: 7,
            car: "bmw".into(),
            field_cars: vec!["bmw".into()],
            races: vec![race.clone(), race],
            field: vec![
                FieldEntry { entrant: Entrant::Player, stem: "bmw".into(), livery: 0, name: "You".into() },
                FieldEntry { entrant: rival, stem: "bmw".into(), livery: 3, name: "Lee Niven".into() },
            ],
            results: vec![
                grid_tools::career::state::result(&[Entrant::Player, rival, mate]),
                grid_tools::career::state::result(&[rival, Entrant::Player, mate]),
            ],
            settings: None,
            best_lap: None,
            top_mph: None,
        };
        assert_eq!(
            standings_lines(&[20, 18, 16], &e, "Ann Example"),
            ["     1. You - 38 pts", "     1. Lee Niven - 38 pts", "     2. Ann Example - 32 pts"]
        );
        // The bank shows the balance and the season's earnings.
        let mut c = Career::new();
        c.money = 52000;
        c.season_earnings = 61500;
        let lines = payout_lines(&Payout { position: Some(0), prize: 30000, ..Payout::default() }, &c);
        let at = lines.iter().position(|l| l == "Total: $30,000").unwrap();
        assert_eq!(lines[at + 1..at + 3], ["Balance: $52,000", "Season's Earnings: $61,500"]);
    }

    #[test]
    fn the_team_mate_is_car_one_and_round_trips() {
        let mut o = out(&[2, 1, 0, 3], false);
        teammate_car(&mut o.result);
        assert_eq!(o.result.position(Entrant::TeamMate), Some(1));
        o.result.order[3].finished = false;
        o.result.order[3].undamaged = false;
        let back = read_race_out(&write_race_out(&o)).unwrap();
        assert_eq!(back.result, o.result);
        assert!(write_race_out(&o).contains("order 2 t 0 3x"));
    }

    /// bmw_si_tour with a hired team mate and placed sponsors on the
    /// purchased data: the team mate's share, the team win and the per-race
    /// sponsor money (docs/CAREER_RESEARCH.md §3.1, §5).
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn a_team_mate_and_sponsors_pay_in_bmw_si_tour() {
        let (cat, _, _) = catalogue();
        let mut c = new_career(&cat);
        c.add_money(&cat, cat.start_money);
        c.reputation[Region::Europe as usize] = 80000;
        c.money = 100000;
        assert!(c.can_hire(&cat));
        let cheapest = c.search(&cat, Some(Region::Europe), None)[0].clone();
        c.hire_driver(&cat, cheapest.driver_id).unwrap();
        assert_eq!(c.money, 100000 - cheapest.signing_fee);
        // bmw_si_tour's offers: 5 (finish, no damage, 3,000), 64 (finish,
        // 2,150), 65 (finish, 1,350) [D].
        c.sponsor_offers = vec![5, 64];
        c.place_sponsor(5, None).unwrap();
        c.place_sponsor(64, Some(0)).unwrap();
        let mut race = out(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], false).result;
        teammate_car(&mut race);
        let before = c.money;
        let p = apply_event(&cat, &mut c, 7, &[race.clone(), race], &out(&[0], false).settings).unwrap();
        // Team mate 2nd: 25,500 less his share.
        let share = (25500.0 * (1.0 - f64::from(cheapest.winnings_percentage) * 0.01)) as i64;
        assert_eq!(p.team_mate_prize, share);
        // Per race: major 3,000 x2 x(player + team mate) + minor 2,150 x(player + team mate).
        assert_eq!(p.sponsor_money, 2 * (3000 * 2 * 2 + 2150 * 2));
        assert_eq!(p.team_win_bonus, 20000);
        assert_eq!(c.money - before, 30000 + share + 20000 + p.sponsor_money);
    }

    #[test]
    fn thousands_groups_digits() {
        assert_eq!(thousands(0), "0");
        assert_eq!(thousands(30000), "30,000");
        assert_eq!(thousands(-1234567), "-1,234,567");
    }

    fn catalogue() -> (Catalogue, BTreeMap<i32, String>, PathBuf) {
        let root = PathBuf::from(std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"));
        let schema = std::fs::read(root.join("database/schema.bin")).unwrap();
        let data = std::fs::read(root.join("database/database.bin")).unwrap();
        let db = grid_formats::database::read(&schema, &data).unwrap();
        (Catalogue::load(&root).unwrap(), track_routes(&db), root)
    }

    /// bmw_si_tour on the purchased data: offered with the BMW, its two races
    /// are the pool's first two Nurburgring layouts with the player in slot
    /// 12 of 12, and two wins pay as docs/CAREER_RESEARCH.md §3 predicts.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn bmw_si_tour_is_offered_planned_and_paid() {
        let (cat, routes, root) = catalogue();
        // A founded team (Arc 1 done): the bank is back at 0.
        let mut career = new_career(&cat);
        assert!(!career.team_founded);
        career.add_money(&cat, cat.start_money);
        assert!(career.team_founded);
        assert_eq!(career.money, 0);
        let offers = offers(&cat, &career, &routes, &Labels::default(), |p| root.join(p).exists());
        let bmw = offers.iter().find(|o| o.event_id == 7).expect("bmw_si_tour offered");
        assert_eq!(bmw.car, "bmw");
        assert_eq!(bmw.field_cars, ["bmw"]);
        // The original's draw: Nurburgring Sprint (layout 24) then GP
        // reverse (99); the field is the event's 20 cars with the player in
        // one team's car, from slot 14 or 15 [V rules, CAREER_RESEARCH §12].
        let layouts: Vec<i32> = bmw.races.iter().map(|(r, _)| r.track_model_id).collect();
        assert_eq!(layouts, [24, 99]);
        let (field, planned) = career_plan(
            &cat,
            &career,
            grid_tools::career::field::Entry::Event(7),
            "bmw",
            0,
            &bmw.races,
            |p| root.join(p).exists(),
            1,
        );
        assert_eq!(field.len(), 20);
        assert_eq!((field[0].entrant, field[0].stem.as_str()), (Entrant::Player, "bmw"));
        assert!(field.iter().all(|c| c.stem == "bmw"));
        assert!(field[1..].iter().all(|c| !c.name.is_empty() && c.livery >= 2));
        assert_eq!(planned.iter().map(|r| r.laps).collect::<Vec<_>>(), [3, 2]);
        for r in &planned {
            assert!(r.slot == 13 || r.slot == 14, "slot {}", r.slot);
            assert_eq!(r.cars.len(), 19);
        }
        // The profile lists 19 opponents with liveries and names in grid order.
        // bmw_si_tour scores with points table 6 (US_Rookie) [D].
        let points = &cat.points[&cat.event(7).unwrap().points_id];
        assert_eq!(points[..3], [20, 18, 16]);
        let profile = with_points(&field_profile(&field, &planned[0]), points);
        let mut s = crate::settings::Settings::new();
        s.apply(&profile, Path::new(".")).unwrap();
        assert_eq!(s.points.as_ref(), Some(points));
        assert_eq!((s.opponents, s.grid_position), (Some(19), Some(planned[0].slot + 1)));
        assert_eq!(s.opponent_liveries.as_ref().map(Vec::len), Some(19));
        assert_eq!(s.opponent_names.as_ref().map(Vec::len), Some(19));
        // A race result maps back to the field's drivers and teams.
        let mut result = out(&(0..20).collect::<Vec<_>>(), false).result;
        to_field(&mut result, &field, &planned[0]);
        let first_ai = &field[planned[0].cars[0]];
        assert_eq!(result.order[1].entrant, first_ai.entrant);
        let mut c = career.clone();
        let win = out(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], false);
        let p = apply_event(&cat, &mut c, 7, &[win.result.clone(), win.result], &win.settings).unwrap();
        // The win also leads the team standings: the rookie Team Win Bonus.
        // Normal (star level 1) takes 10 % off the base reputation.
        assert_eq!(star_level(), 1);
        assert_eq!((p.prize, p.team_win_bonus), (30000, 20000));
        assert_eq!((p.reputation_base, p.reputation_bonus), (20000, -2000));
        assert_eq!(c.money, 50000);
        assert_eq!(c.reputation[Region::Europe as usize], 18000);
        // 2nd and 5th place: 25,500 / 16,000 (§3.2 table) in a fresh career.
        let mut c = career.clone();
        let second = out(&[1, 0, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11], false).result;
        let p = apply_event(&cat, &mut c, 7, &[second.clone(), second], &win.settings).unwrap();
        assert_eq!((p.prize, p.reputation_base, p.team_win_bonus), (25500, 16000, 0));
        // Aids off and manual gears at Normal: F = 0.2 − 0.1 (§3.2).
        let mut c = career;
        let off = out(&[0, 1, 2], true);
        let p = apply_event(&cat, &mut c, 7, &[off.result.clone(), off.result], &off.settings).unwrap();
        assert_eq!(p.reputation(), 22000);
    }

    /// Head-to-heads on the purchased data: offered once open, raced
    /// against the event's one rival with no team mate, and paid by the
    /// prize for a win only (docs/CAREER_SCREEN.md, F478).
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn head_to_heads_race_the_rival_alone() {
        let (cat, routes, root) = catalogue();
        let exists = |p: &str| root.join(p).exists();
        let mut c = new_career(&cat);
        c.add_money(&cat, cat.start_money);
        c.reputation = [2_000_000; 5];
        c.team_mate = Some(grid_tools::career::state::HiredTeamMate { driver_id: -1, winnings_percentage: 20 });
        let all = |c: &Career| -> Vec<Offer> {
            offers(&cat, c, &routes, &Labels::default(), exists)
                .into_iter()
                .filter(|o| cat.event(o.event_id).unwrap().head_to_head)
                .collect()
        };
        assert!(all(&c).is_empty(), "closed until the tier's other events are won");
        c.head_to_heads = cat.events.values().filter(|e| e.head_to_head).map(|e| e.id).collect();
        let h2h = all(&c);
        let ids: Vec<i32> = h2h.iter().map(|o| o.event_id).collect();
        eprintln!("runnable head-to-heads: {ids:?}");
        assert!(!h2h.is_empty());
        let o = &h2h[0];
        assert!(o.label.starts_with("Head-to-head: "));
        let event = cat.event(o.event_id).unwrap();
        let (field, planned) = career_plan(
            &cat,
            &c,
            grid_tools::career::field::Entry::Event(o.event_id),
            &o.car,
            0,
            &o.races,
            exists,
            1,
        );
        // The rows plus the player [V 0x66D1C0]: one rival, no team mate.
        assert_eq!(field.len(), event.drivers.len() + 1);
        assert_eq!(field.len(), 2);
        assert_eq!(field[0].entrant, Entrant::Player);
        assert!(matches!(field[1].entrant, Entrant::Ai { .. }) && !field[1].name.is_empty());
        assert_eq!(planned.len(), 1);
        assert_eq!(planned[0].cars, [1]);
        let mut s = crate::settings::Settings::new();
        s.apply(&field_profile(&field, &planned[0]), Path::new(".")).unwrap();
        assert_eq!((s.opponents, s.points.as_ref()), (Some(1), None));
        // Lost: nothing but sponsor money; won: the prize, no reputation.
        let rival = field[1].entrant;
        let lost = grid_tools::career::state::result(&[rival, Entrant::Player]);
        let mut lose = c.clone();
        let p = apply_event(&cat, &mut lose, o.event_id, &[lost], &RaceSettings::default()).unwrap();
        assert_eq!((p.prize, p.reputation()), (0, 0));
        assert_eq!(payout_lines(&p, &lose)[0], "Head-to-head: lost");
        let won = grid_tools::career::state::result(&[Entrant::Player, rival]);
        let mut win = c.clone();
        let p = apply_event(&cat, &mut win, o.event_id, &[won], &RaceSettings::default()).unwrap();
        assert_eq!((p.prize, p.reputation()), (event.prize_money_for_first, 0));
        let lines = payout_lines(&p, &win);
        assert_eq!(lines[0], "Head-to-head: won");
        assert!(lines.contains(&format!("Prize Money: ${}", thousands(event.prize_money_for_first))));
    }

    /// eBay on the purchased data: a used Lacetti bought for the team
    /// becomes euro_touring's car (event 36 races the BMW and the Lacetti).
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn an_ebay_car_becomes_the_events_car() {
        let (cat, routes, root) = catalogue();
        let exists = |p: &str| root.join(p).exists();
        let mut c = new_career(&cat);
        c.add_money(&cat, cat.start_money);
        c.reputation[Region::Europe as usize] = 80000;
        c.money = 500_000;
        let event = |c: &Career| {
            offers(&cat, c, &routes, &Labels::default(), exists)
                .into_iter()
                .find(|o| o.event_id == 36)
                .unwrap()
        };
        let before = event(&c);
        assert_eq!(before.car, "bmw");
        assert!(before.field_cars.contains(&"lac".to_owned()));
        let lac = cat.event(36).unwrap().vehicles.iter().copied().find(|v| cat.vehicles[v].stem == "lac").unwrap();
        let used = c.ebay.offers(&cat, lac, None);
        assert_eq!(used.len(), 3);
        let cheapest = used.iter().min_by_key(|u| u.price).unwrap().clone();
        assert!(cheapest.price < cat.vehicles[&lac].purchase_price);
        c.buy_used(&cat, &cheapest).unwrap();
        assert_eq!(event(&c).car, "lac");
        // The offers changed with the purchase (a seed refresh).
        assert_ne!(c.ebay.offers(&cat, lac, None), used);
    }

    /// A new career on the purchased data: no team, no own events, and the
    /// driver offers the port can run (15: Porsche 911 at Jarama,
    /// finish 3rd or better; 16: BMW time trial at the Nurburgring).
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn a_new_career_drives_offers_until_the_team_is_founded() {
        let (cat, routes, root) = catalogue();
        let mut career = new_career(&cat);
        let exists = |p: &str| root.join(p).exists();
        assert!(offers(&cat, &career, &routes, &Labels::default(), exists).is_empty());
        let runs = offer_runs(&cat, &career, &routes, &Labels::default(), exists);
        let ids: Vec<i32> = runs.iter().map(|r| r.offer_id).collect();
        // US 2 (Lacetti, Washington), Europe 15 and 16, Japan 25 (350Z,
        // Yokohama): the offers on show whose car and layout the port races.
        assert_eq!(ids, [2, 15, 16, 25]);
        let r15 = &runs[1];
        assert_eq!(r15.car, "911");
        assert_eq!(r15.race.1, "tracks/europe/jarama/route_0");
        assert!(r15.field_cars.len() > 1, "race offers have a field");
        assert_eq!(runs[2].field_cars, ["bmw"], "test offers run solo");
        // Offer 15 (finish no lower than 3rd): 2nd meets it, 4th does not.
        assert!(offer_met(&cat, 15, &out(&[1, 0, 2, 3], false), true));
        assert!(!offer_met(&cat, 15, &out(&[1, 2, 3, 0], false), true));
        // Offer 16 (time trial): 107 s at Normal (star level 1) [D].
        let mut lap = out(&[0], false);
        lap.best_lap = Some(106.9);
        assert!(offer_met(&cat, 16, &lap, true));
        lap.best_lap = Some(107.2);
        assert!(!offer_met(&cat, 16, &lap, true));
        // Paying 15 met: 3,000 fee + 2,000 bonus, 180 reputation to Europe.
        let p = apply_driver_offer(&cat, &mut career, 15, true, &[], &Default::default()).unwrap();
        assert_eq!((p.driver_offer_wages, p.driver_offer_bonus, p.reputation()), (3000, 2000, 180));
        assert_eq!(career.reputation[Region::Europe as usize], 180);
        // The slot moves on to offer 17, the next Europe race offer by order_index.
        let ids: Vec<i32> = offer_runs(&cat, &career, &routes, &Labels::default(), exists)
            .iter()
            .map(|r| r.offer_id)
            .collect();
        assert!(!ids.contains(&15) && ids.contains(&16));
        assert!(career.driver_offers_shown(&cat).contains(&17));
        // Rival objective: AI cars 1 and 2 drive for the rival team.
        assert!(offer_met(&cat, 19, &out(&[0, 3, 1, 2], false), true));
        assert!(!offer_met(&cat, 19, &out(&[3, 1, 0, 2], false), true));
    }
}
