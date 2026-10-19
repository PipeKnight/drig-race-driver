//! Career entry (docs/CAREER_SCREEN.md): one own-team career event at a
//! time, run with the race code and paid with `grid_tools::career` (F416,
//! docs/CAREER_RESEARCH.md).
//!
//! - The screen shows money, the five regional reputations, the licences and
//!   the season, and lists the open events (`Career::event_open`) that the
//!   port can run: circuit races (`competition_format` 1, no head-to-head,
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
    Career, Catalogue, Classified, Entrant, Payout, RaceResult, RaceSettings, Region,
    SeasonState, apply_event, save,
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
    grid_tools::route::CIRCUITS.contains(&route)
        || grid_tools::series::SURVEYED_TRACKS
            .iter()
            .any(|t| route.starts_with(&format!("{t}/")))
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
            || event.head_to_head
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
        let Some(car) = field_cars.first().cloned() else {
            continue;
        };
        let mut pool: Vec<&EventRace> = event.races.iter().collect();
        pool.sort_by_key(|r| (r.order_index, r.id));
        let races: Vec<(EventRace, String)> = pool
            .into_iter()
            .filter(|r| r.laps > 0 && r.time_limit_seconds == 0)
            .filter_map(|r| {
                let route = routes.get(&r.track_model_id)?;
                (surveyed(route) && exists(route)).then(|| (r.clone(), route.clone()))
            })
            .take(event.num_competitions as usize)
            .collect();
        if races.len() < event.num_competitions as usize {
            continue;
        }
        let key = format!("db_{}", event.name);
        let text = labels.get(&key);
        let label = if text == key { event.name.clone() } else { text.to_owned() };
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

/// The event's plan: each drawn layout with the player's slot
/// (`race::grid_slot` on [`FIELD`] cars, drawn with the original's generator,
/// AUTHORED seed).
pub fn plan(offer: &Offer, seed: u32) -> Vec<Planned> {
    let mut rng = grid_tools::series::Mt::new(seed);
    offer
        .races
        .iter()
        .map(|(r, route)| Planned {
            route: route.clone(),
            laps: r.laps,
            slot: grid_tools::race::grid_slot(
                r.player_grid_position_base.max(0) as u32,
                r.player_grid_position_range.max(0) as u32,
                FIELD,
                |n| rng.range(0, n),
            ),
        })
        .collect()
}

/// The race profile of one planned race: the event menu's keys for a race.
pub fn race_profile(car: &str, field_cars: &[String], race: &Planned) -> String {
    format!(
        "# Written by the career screen (docs/CAREER_SCREEN.md) for the next career race.\n\
         [game]\nroute = {}\ncar = {car}\nopponent_cars = {}\n\
         [drive]\nenabled = on\nlaps = {}\nopponents = {}\ngrid_position = {}\n\
         ai_characteristics = ai-driver-characteristics.tsv\n\
         [ai]\ndifficulty = {DIFFICULTY}\ncatchup = on\nchoreographer = on\nmistakes = on\n\
         [private]\nbody_contacts = derive\nbody_events = derive\ndeformation = derive\n",
        race.route,
        field_cars.join(","),
        race.laps,
        FIELD - 1,
        race.slot + 1,
    )
}

/// A classification token: car index (0 the player), `x` when out.
fn token(c: &Classified) -> String {
    let car = match c.entrant {
        Entrant::Player => 0,
        Entrant::TeamMate => return "t".into(),
        Entrant::Ai { driver, .. } => driver,
    };
    format!("{car}{}", if c.finished { "" } else { "x" })
}

/// AUTHORED: each AI car is its own driver and team (car index); the
/// career field's drivers and teams are not traced.
fn classified(token: &str) -> Result<Classified, String> {
    let (num, finished) = match token.strip_suffix('x') {
        Some(n) => (n, false),
        None => (token, true),
    };
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
}

pub fn write_race_out(out: &RaceOut) -> String {
    let order: Vec<String> = out.result.order.iter().map(token).collect();
    format!(
        "grid-career-race 1\nsettings {}\norder {}\n",
        settings_text(&out.settings),
        order.join(" ")
    )
}

pub fn read_race_out(text: &str) -> Result<RaceOut, String> {
    let mut lines = text.lines();
    if lines.next() != Some("grid-career-race 1") {
        return Err("not a career race result".into());
    }
    let (mut result, mut settings) = (None, None);
    for line in lines {
        let mut f = line.split_whitespace();
        match f.next() {
            Some("settings") => settings = Some(parse_settings(f)?),
            Some("order") => {
                result = Some(RaceResult {
                    order: f.map(classified).collect::<Result<_, _>>()?,
                })
            }
            None => {}
            Some(other) => return Err(format!("unknown line {other}")),
        }
    }
    let result = result.ok_or("no order")?;
    if result.position(Entrant::Player).is_none() {
        return Err("the player is not classified".into());
    }
    Ok(RaceOut {
        result,
        settings: settings.ok_or("no settings")?,
    })
}

/// The event in progress (`career-event.txt`, AUTHORED format).
#[derive(Clone, Debug, PartialEq)]
pub struct InProgress {
    pub event_id: i32,
    pub car: String,
    pub field_cars: Vec<String>,
    pub races: Vec<Planned>,
    pub results: Vec<RaceResult>,
    /// The last race's settings (the original reads them at the event end).
    pub settings: Option<RaceSettings>,
}

impl InProgress {
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
            s += &format!("race {} {} {}\n", r.route, r.laps, r.slot);
        }
        for r in &self.results {
            let order: Vec<String> = r.order.iter().map(token).collect();
            s += &format!("result {}\n", order.join(" "));
        }
        if let Some(settings) = &self.settings {
            s += &format!("settings {}\n", settings_text(settings));
        }
        s
    }

    pub fn read(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        if lines.next() != Some("grid-career-event 1") {
            return Err("not a career event".into());
        }
        let mut e = Self {
            event_id: 0,
            car: String::new(),
            field_cars: Vec::new(),
            races: Vec::new(),
            results: Vec::new(),
            settings: None,
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
                }),
                Some("result") => e.results.push(RaceResult {
                    order: f.map(classified).collect::<Result<_, _>>()?,
                }),
                Some("settings") => e.settings = Some(parse_settings(f)?),
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

/// AUTHORED: a new career skips Arc 1 (driver offers, not ported): the
/// original's founding runs at once (`add_money` of the founding sum,
/// 0x674610 → 0x66F640: team active, rookie licences, the free car), so the
/// bank starts at 0 with the team founded.
pub fn new_career(cat: &Catalogue) -> Career {
    let mut c = Career::new();
    c.add_money(cat, cat.start_money);
    c
}

/// The payout lines (the original's bank text names, English).
pub fn payout_lines(p: &Payout) -> Vec<String> {
    let money = |v: i64| format!("${}", thousands(v));
    let mut lines = vec![match p.position {
        Some(pos) => format!("Championship position: {}", pos + 1),
        None => "Not classified".into(),
    }];
    lines.push(format!("Prize Money: {}", money(p.prize)));
    lines.push(format!("Teammate Prize Money: {}", money(p.team_mate_prize)));
    lines.push(format!("Team Win Bonus: {}", money(p.team_win_bonus)));
    lines.push(format!("Sponsorship Money: {}", money(p.sponsor_money)));
    lines.push(format!("Total: {}", money(p.money())));
    let region = p
        .region
        .and_then(|r| REGIONS.iter().find(|(x, _)| *x == r))
        .map_or("", |(_, n)| *n);
    lines.push(format!(
        "Reputation: {} pts ({} + bonus {}) to {region}",
        thousands(p.reputation()),
        thousands(p.reputation_base),
        thousands(p.reputation_bonus)
    ));
    if !p.new_sponsor_offers.is_empty() {
        lines.push(format!("New sponsor offers: {}", p.new_sponsor_offers.len()));
    }
    lines
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

/// The race's classification as car tokens (`post_race::table` order).
pub fn race_out(rows: &[crate::post_race::Row], aids: &grid_tools::source_vehicle::assists::Assists) -> RaceOut {
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
        result: RaceResult { order },
        settings: RaceSettings {
            star_level: star_level(),
            manual_gears: aids.transmission == Transmission::Manual,
            traction_control: aids.traction_control != Level::OFF,
            steering_assist: aids.stability_control,
            braking_assist: aids.braking_assist != Level::OFF,
            ..RaceSettings::default()
        },
    }
}

/// Writes the result when the race leaves to the career screen
/// (`event_menu::leave`, exit code BACK_TO_MENU).
pub fn write_result(
    mut exits: MessageReader<AppExit>,
    path: Res<ResultPath>,
    driving: Res<crate::Driving>,
    trial: Option<Res<crate::time_trial::Trial>>,
) {
    let back = AppExit::from_code(crate::event_menu::BACK_TO_MENU as u8);
    if !exits.read().any(|e| *e == back) {
        return;
    }
    let rows = crate::post_race::table(&driving, trial.as_deref());
    let out = race_out(&rows, &driving.runtime.assists);
    if out.result.position(Entrant::Player).is_none() {
        log_line!("drive_career version=1 event=race_result_missing");
        return;
    }
    match std::fs::write(&path.0, write_race_out(&out)) {
        Ok(()) => log_line!(
            "drive_career version=1 event=race_result position={} cars={}",
            out.result.position(Entrant::Player).unwrap_or(0) + 1,
            out.result.order.len()
        ),
        Err(e) => log_line!("drive_career version=1 event=race_result_failed error={e:?}"),
    }
}

// ---- Processes -----------------------------------------------------------

/// Diagnostics `GRID_CAREER_AUTO=EVENT_ID` (unattended checks, desktop): the
/// screen enters that event, starts each next race and quits after the
/// payout; the supervisor runs the races as scripted runs (`--drive-ticks`),
/// so `GRID_AUTOPILOT` drives and `GRID_RESULTS_CONTINUE` leaves the results.
fn auto() -> Option<i32> {
    std::env::var("GRID_CAREER_AUTO").ok()?.parse().ok()
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
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    Enter(usize),
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
    current: Option<InProgress>,
    page: Page,
    cursor: usize,
    repeat: Repeat,
    dir: PathBuf,
    message: String,
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
                rows.push(("Quit".into(), Action::Quit));
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
        }
    }

    fn info(&self) -> Vec<String> {
        let mut lines = status_lines(&self.cat, &self.career);
        match &self.page {
            Page::Home => {
                lines.push(String::new());
                lines.push(if self.offers.is_empty() {
                    "No open event can run in the port yet.".into()
                } else {
                    "Open events:".into()
                });
            }
            Page::Event => {
                if let Some(e) = &self.current {
                    lines.push(String::new());
                    let name = self.cat.event(e.event_id).map_or("?", |ev| ev.name.as_str());
                    lines.push(format!("Event {name}, car {}", e.car));
                    for (i, r) in e.races.iter().enumerate() {
                        let place = e.results.get(i).and_then(|res| res.position(Entrant::Player));
                        lines.push(format!(
                            "    Race {}: {}{}",
                            i + 1,
                            route_name(&r.route),
                            place.map_or(String::new(), |p| format!(" - finished {}", p + 1))
                        ));
                    }
                }
            }
            Page::Payout(p) => {
                lines.push(String::new());
                lines.extend(p.iter().cloned());
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
            let out = read_race_out(&text)?;
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
        let settings = e.settings.unwrap_or_default();
        let payout = apply_event(&self.cat, &mut self.career, e.event_id, &e.results, &settings)?;
        self.save()?;
        let _ = std::fs::remove_file(&event_path);
        log_line!(
            "drive_career version=1 event=paid event_id={} position={} prize={} team_mate={} team_win={} sponsors={} reputation={} money={} total_reputation={}",
            e.event_id,
            payout.position.map_or(0, |p| p + 1),
            payout.prize,
            payout.team_mate_prize,
            payout.team_win_bonus,
            payout.sponsor_money,
            payout.reputation(),
            self.career.money,
            self.career.total_reputation()
        );
        self.current = None;
        self.page = Page::Payout(payout_lines(&payout));
        Ok(())
    }

    fn refresh_offers(&mut self, routes: &BTreeMap<i32, String>, labels: &Labels, root: &Path) {
        self.offers = offers(&self.cat, &self.career, routes, labels, |p| root.join(p).exists());
    }

    fn start(&mut self, index: usize) -> Result<(), String> {
        let offer = self.offers.get(index).ok_or("no such event")?.clone();
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(1, |d| d.as_secs() as u32);
        let e = InProgress {
            event_id: offer.event_id,
            car: offer.car.clone(),
            field_cars: offer.field_cars.clone(),
            races: plan(&offer, seed),
            results: Vec::new(),
            settings: None,
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

    /// Writes the next race's profile.
    fn go(&self) -> Result<(), String> {
        let e = self.current.as_ref().ok_or("no event")?;
        let next = e.next().ok_or("event finished")?;
        std::fs::write(
            self.dir.join(PROFILE),
            race_profile(&e.car, &e.field_cars, next),
        )
        .map_err(|err| err.to_string())?;
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
        current,
        page: Page::Home,
        cursor: 0,
        repeat: Repeat::default(),
        dir,
        message: String::new(),
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

const ROWS: usize = 14;

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
        chosen = Some(if matches!(screen.page, Page::Payout(_)) {
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
        && let Some(event_id) = auto()
    {
        *auto_wait += time.delta_secs();
        if *auto_wait >= 3. {
            *auto_wait = 0.;
            chosen = Some(match &screen.page {
                Page::Home => screen
                    .offers
                    .iter()
                    .position(|o| o.event_id == event_id)
                    .map_or(Action::Quit, Action::Enter),
                Page::Event if screen.current.as_ref().is_some_and(|e| e.next().is_some()) => {
                    Action::NextRace
                }
                Page::Event | Page::Payout(_) => Action::Quit,
            });
        }
    }
    let Some(action) = chosen else {
        return;
    };
    screen.message.clear();
    let result = match action {
        Action::Enter(i) => screen.start(i).and_then(|()| screen.go()).map(|()| true),
        Action::NextRace => screen.go().map(|()| true),
        Action::Abandon => {
            let _ = std::fs::remove_file(screen.dir.join(EVENT));
            log_line!("drive_career version=1 event=abandoned");
            screen.current = None;
            screen.page = Page::Home;
            screen.cursor = 0;
            Ok(false)
        }
        Action::Continue => {
            screen.page = Page::Home;
            screen.cursor = 0;
            Ok(false)
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
            },
            settings: RaceSettings {
                star_level: star_level(),
                traction_control: !aids_off,
                steering_assist: !aids_off,
                braking_assist: !aids_off,
                manual_gears: aids_off,
                ..RaceSettings::default()
            },
        }
    }

    #[test]
    fn race_results_and_events_round_trip() {
        let mut o = out(&[3, 0, 1, 2], true);
        o.result.order[3].finished = false;
        let back = read_race_out(&write_race_out(&o)).unwrap();
        assert!(!back.result.order[3].finished);
        assert_eq!(back.result.order[3].entrant, Entrant::Ai { driver: 2, team: 2 });
        assert_eq!(back.settings, o.settings);
        assert_eq!(back.result.position(Entrant::Player), Some(1));
        assert!(read_race_out("grid-career-race 1\nsettings star=2\norder 1 2\n").is_err());

        let mut e = InProgress {
            event_id: 7,
            car: "bmw".into(),
            field_cars: vec!["bmw".into()],
            races: vec![
                Planned { route: "tracks/europe/nurburgring/route_1".into(), laps: 3, slot: 11 },
                Planned { route: "tracks/europe/nurburgring/route_2".into(), laps: 2, slot: 11 },
            ],
            results: Vec::new(),
            settings: None,
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
    }

    #[test]
    fn the_race_profile_is_a_twelve_car_race_of_the_event_cars() {
        let p = race_profile(
            "bmw",
            &["bmw".into()],
            &Planned { route: "tracks/europe/nurburgring/route_1".into(), laps: 3, slot: 11 },
        );
        let mut s = crate::settings::Settings::new();
        s.apply(&p, Path::new(".")).unwrap();
        assert_eq!((s.laps, s.opponents, s.grid_position), (Some(3), Some(11), Some(12)));
        assert_eq!(s.opponent_cars.as_deref(), Some("bmw"));
        // The catch-up follows the difficulty, as the launchers (F439).
        assert!(p.contains("difficulty = 0.606\ncatchup = on\n"));
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
        let career = new_career(&cat);
        assert!(career.team_founded);
        assert_eq!(career.money, 0);
        let offers = offers(&cat, &career, &routes, &Labels::default(), |p| root.join(p).exists());
        let bmw = offers.iter().find(|o| o.event_id == 7).expect("bmw_si_tour offered");
        assert_eq!(bmw.car, "bmw");
        assert_eq!(bmw.field_cars, ["bmw"]);
        let planned = plan(bmw, 1);
        assert_eq!(
            planned,
            [
                Planned { route: "tracks/europe/nurburgring/route_1".into(), laps: 3, slot: 11 },
                Planned { route: "tracks/europe/nurburgring/route_2".into(), laps: 2, slot: 11 },
            ]
        );
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
}
