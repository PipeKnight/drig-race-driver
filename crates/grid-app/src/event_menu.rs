//! Event selection before the race (docs/EVENT_MENU.md): the original's Race
//! Day screen `f_m_selectrace` (`ScreenGarageFreeRaceSelect`, states.bin [D]:
//! buttons discipline, car, track_select, track_type, laps, team), then the
//! race process with its Race Options front end (`f_m_event_options`).
//!
//! - Disciplines, their cars and track pools come from the user's
//!   `database.bin` (`free_race_discipline`, `free_race_restrictions`,
//!   `free_race_track_pool`, `track_model`, `free_race_times`) and their
//!   names from `language_eng.lng` (`db_<name_string_id>`) [D]. The port
//!   offers the disciplines it can run, the routes it has raced and the
//!   cars it can race; Race and Time Trial (the launchers' events) are
//!   AUTHORED entries. Opponents and Direction rows are AUTHORED; the
//!   panel is the pause menu's AUTHORED look.
//! - The choice is saved in `<data dir>/settings.ini` `[event]` (read only
//!   here, so the launchers' profiles are unaffected) and written as a
//!   settings profile `<data dir>/event-menu.ini` equal to the launchers'
//!   `settings/*.ini` for the same event.
//! - Process model (winit cannot create a second event loop in a process,
//!   and the race is set up before its Bevy app starts): on the desktop
//!   `grid-app --menu` supervises a menu process and a race process in turn
//!   (exit codes [`MENU_GO`], [`BACK_TO_MENU`]); on Android the activity is
//!   restarted (`MainActivity.relaunch`). Leaving a race that was started
//!   from the menu (results Continue/Exit, Retire, Finish Test Drive) returns
//!   here, as the original's `end_race_finish_race` returns to its front end.
use crate::pause_menu::{Labels, Nav, Repeat, View};
use bevy::prelude::*;
use grid_tools::car::Car;
use grid_tools::route;
use std::path::{Path, PathBuf};

/// Exit code of the menu process: the event profile is written, start it.
#[cfg(not(target_os = "android"))]
pub const MENU_GO: i32 = 20;
/// Exit code of a race started from the menu: back to the menu.
pub const BACK_TO_MENU: i32 = 21;
/// The event profile the menu writes into the data dir.
pub const PROFILE: &str = "event-menu.ini";
/// Android: present when the next start is the chosen race, not the menu.
#[cfg(target_os = "android")]
pub const GO_MARKER: &str = "event-menu.go";

/// Laps a lapped event offers (AUTHORED range; the original's free-race
/// lap choices are not decoded).
pub const MAX_LAPS: u32 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The launchers' race: the original's 12 touring cars (AUTHORED entry).
    Race,
    /// Solo laps with personal bests (AUTHORED entry; the original's Time
    /// Trial is a separate main-menu mode).
    TimeTrial,
    /// A circuit series of the free race (`single_race`, docs/SERIES.md).
    Series,
    DriftBattle,
    DriftGp,
    Freestyle,
    /// Midnight Touge: one run, either direction.
    Touge,
    /// Pro Touge: two runs, downhill then uphill (F412, docs/TOUGE.md).
    ProTouge,
    /// Demolition Derby: the stadium laps with disqualification (F421,
    /// docs/DEMOLITION_DERBY.md).
    Demolition,
    /// The 24 Hours of Le Mans (free race `Le_Mans`): a time-limited race of
    /// `free_race_times` minutes on Circuit de la Sarthe (docs/LE_MANS.md).
    LeMans,
}

/// One layout a discipline offers.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub route: String,
    /// Venue (Location) and layout (Route) names.
    pub venue: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EventType {
    pub kind: Kind,
    /// `[event] type`: the `free_race_discipline` name, or `race` /
    /// `time_trial`.
    pub key: String,
    pub label: String,
    pub layouts: Vec<Layout>,
    /// Car stems the player may choose.
    pub cars: Vec<String>,
    /// The original's pool cars are not ported; these are stand-ins
    /// (AUTHORED, shown in the help line).
    pub stand_in_cars: bool,
    pub default_laps: Option<u32>,
    /// Opponents: (most, default).
    pub opponents: Option<(usize, usize)>,
    /// Freestyle: `free_race_times` minutes with their labels.
    pub times: Vec<(u32, String)>,
    /// Touge: downhill (subroute 0) or uphill (1).
    pub directions: bool,
    /// A series whose fields still hold cars that cannot race
    /// (`series::is_experimental`; none since F420): the race runs with
    /// GRID_SERIES_EXPERIMENTAL=1.
    pub experimental: bool,
}

/// What the user picked (`[event]` keys).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Choice {
    pub key: String,
    pub route: String,
    pub car: String,
    pub laps: u32,
    pub opponents: usize,
    pub minutes: u32,
    pub subroute: u8,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Catalog {
    pub types: Vec<EventType>,
}

/// The language text of `id`, or `fallback` when the file lacks it.
fn label(labels: &Labels, id: &str, fallback: &str) -> String {
    let text = labels.get(id);
    if text == id {
        fallback.into()
    } else {
        text.into()
    }
}

/// Names of route folders from `track_model` and `track` [D].
#[derive(Default)]
struct Names {
    /// route folder → (venue id, layout id)
    routes: std::collections::HashMap<String, (String, String)>,
}

impl Names {
    fn layout(&self, labels: &Labels, route: &str) -> Layout {
        let parts: Vec<&str> = route.split('/').collect();
        let track = parts.get(2).copied().unwrap_or(route);
        let leg = parts.get(3).copied().unwrap_or("");
        match self.routes.get(route) {
            Some((venue, name)) => Layout {
                route: route.into(),
                venue: label(labels, &format!("db_{venue}"), venue),
                name: label(labels, &format!("db_{name}"), name),
            },
            None => Layout {
                route: route.into(),
                venue: track.replace('_', " "),
                name: leg.replace('_', " "),
            },
        }
    }
}

fn names(db: Option<&grid_formats::database::Database>) -> Names {
    let mut names = Names::default();
    let Some(db) = db else { return names };
    let (Some(models), Some(tracks)) = (db.table("track_model"), db.table("track")) else {
        return names;
    };
    for m in models.rows() {
        let (Some(folder), Some(file), Some(leg), Some(name)) = (
            m.text("folder_string"),
            m.text("file_string"),
            m.text("route_string"),
            m.text("name_string_id"),
        ) else {
            continue;
        };
        let venue = tracks
            .rows()
            .find(|t| t.int("id") == m.int("track_id"))
            .and_then(|t| t.text("name_string_id"))
            .unwrap_or(file);
        names.routes.insert(
            format!("tracks/{folder}/{file}/{}", leg.to_ascii_lowercase()),
            (venue.into(), name.into()),
        );
    }
    names
}

/// Whether the port runs `route` for this kind of event (the routes the
/// launchers and the checks have raced).
fn supported(kind: Kind, route: &str) -> bool {
    match kind {
        Kind::Race | Kind::TimeTrial => route::CIRCUITS.contains(&route),
        Kind::Series | Kind::DriftBattle | Kind::DriftGp => {
            grid_tools::series::route_supported(route)
        }
        Kind::Freestyle => route.starts_with("tracks/japan/yokohama_docks/"),
        Kind::Touge | Kind::ProTouge => route::HARUNA_RACED.contains(&route),
        Kind::Demolition => {
            route == route::DETROIT_STADIUM || route == route::DETROIT_STADIUM_SHORT
        }
        Kind::LeMans => route == route::LE_MANS,
    }
}

impl Catalog {
    /// The events the user's installation can run: database and language
    /// from `root`, a route or car whose folder is missing is left out (the
    /// phone's data subset).
    pub fn load(root: &Path, labels: &Labels) -> Self {
        let db = std::fs::read(root.join("database/schema.bin"))
            .and_then(|s| std::fs::read(root.join("database/database.bin")).map(|d| (s, d)))
            .ok()
            .and_then(|(s, d)| grid_formats::database::read(&s, &d).ok());
        let present = |path: &str| root.join(path).is_dir();
        Self::build(db.as_ref(), labels, &present)
    }

    pub fn build(
        db: Option<&grid_formats::database::Database>,
        labels: &Labels,
        present: &dyn Fn(&str) -> bool,
    ) -> Self {
        let names = names(db);
        let cars = |stems: &[&str]| -> Vec<String> {
            stems
                .iter()
                .filter(|s| present(&format!("cars/{s}")))
                .map(|s| s.to_string())
                .collect()
        };
        let layouts = |kind: Kind, routes: &mut dyn Iterator<Item = String>| -> Vec<Layout> {
            let mut out: Vec<Layout> = Vec::new();
            for r in routes {
                if supported(kind, &r) && present(&r) && !out.iter().any(|l| l.route == r) {
                    out.push(names.layout(labels, &r));
                }
            }
            out
        };
        let touring = cars(&[grid_tools::car::LACETTI, grid_tools::car::BMW]);
        let raceable = cars(&grid_tools::series::RACEABLE);
        let mut types = vec![
            EventType {
                kind: Kind::Race,
                key: "race".into(),
                label: "Race".into(),
                layouts: layouts(
                    Kind::Race,
                    &mut route::CIRCUITS.iter().map(|r| r.to_string()),
                ),
                cars: touring.clone(),
                stand_in_cars: false,
                default_laps: Some(3),
                opponents: Some((11, 11)),
                times: Vec::new(),
                directions: false,
                experimental: false,
            },
            EventType {
                kind: Kind::TimeTrial,
                key: "time_trial".into(),
                label: label(labels, "main_menu_time_trial_button_title", "Time Trial"),
                layouts: layouts(
                    Kind::TimeTrial,
                    &mut route::CIRCUITS.iter().map(|r| r.to_string()),
                ),
                cars: raceable.clone(),
                stand_in_cars: false,
                default_laps: Some(3),
                opponents: None,
                times: Vec::new(),
                directions: false,
                experimental: false,
            },
        ];
        if let Some(db) = db {
            let mut disciplines: Vec<(i32, i32, String)> = db
                .table("free_race_discipline")
                .map(|t| {
                    t.rows()
                        .filter_map(|r| {
                            Some((
                                r.int("sort_order").unwrap_or(0),
                                r.int("id")?,
                                r.text("name_string_id")?.to_string(),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();
            disciplines.sort();
            for (_, id, name) in disciplines {
                let kind = match name.as_str() {
                    "pro_tuned" | "touring" | "gt1" | "gt2" | "le_mans_series" | "prototype"
                    | "pro_muscle" | "open_wheel" | "gt_club" | "prestige" => Kind::Series,
                    "drift_battle" => Kind::DriftBattle,
                    "drift_gp" => Kind::DriftGp,
                    "Free_Drift" => Kind::Freestyle,
                    "illegal_touge" => Kind::Touge,
                    "legal_touge" => Kind::ProTouge,
                    "Demolition_D" => Kind::Demolition,
                    "Le_Mans" => Kind::LeMans,
                    // Open wheel, muscle, club and prestige are not ported
                    // (docs/SERIES.md, docs/DISCIPLINES.md).
                    _ => continue,
                };
                let Ok(series) = grid_tools::series::Series::from_database(db, &name) else {
                    continue;
                };
                // Pro Touge's pool holds the legal Haruna layouts; the port
                // also runs it on route 1, the illegal road its launcher uses
                // (AUTHORED), ahead of the pool's raced route 4.
                let pool: Vec<String> = if kind == Kind::ProTouge {
                    std::iter::once(route::HARUNA.to_string())
                        .chain(series.layouts.iter().map(|l| l.route.clone()))
                        .collect()
                } else {
                    series.layouts.iter().map(|l| l.route.clone()).collect()
                };
                let routes = &mut pool.into_iter();
                let (cars, stand_in) = match kind {
                    // The field draws from every one of them, so a series
                    // with one missing from the data subset is left out
                    // (F539: Touring Cars on the Donington bundle needs the BMW).
                    Kind::Series | Kind::LeMans => {
                        let raceable = series.raceable_cars();
                        let stems: Vec<&str> = raceable.iter().map(Car::stem).collect();
                        let found = cars(&stems);
                        (if found.len() == stems.len() { found } else { Vec::new() }, false)
                    }
                    // The drift pool's cars (cor, gto, rx7, ...) are not
                    // ported: the launchers' BMW, and the Lacetti.
                    Kind::DriftBattle | Kind::DriftGp | Kind::Freestyle => (
                        cars(&[grid_tools::car::BMW, grid_tools::car::LACETTI]),
                        true,
                    ),
                    // The event's stock car (`Stock_Car` discipline) [D].
                    Kind::Demolition => (cars(&["stk"]), false),
                    // The pool's raceable cars (350Z, Skyline, Supra) after
                    // the launcher's Lacetti.
                    _ => {
                        let mut list = vec![grid_tools::car::LACETTI.to_string()];
                        list.extend(series.raceable_cars().iter().map(|c| c.stem().to_string()));
                        let stems: Vec<&str> = list.iter().map(String::as_str).collect();
                        (cars(&stems), true)
                    }
                };
                let max_cars = series.layouts.first().map_or(12, |l| l.max_cars);
                let times: Vec<(u32, String)> = if matches!(kind, Kind::Freestyle | Kind::LeMans) {
                    let mut rows: Vec<(u32, String)> = db
                        .table("free_race_times")
                        .map(|t| {
                            t.rows()
                                .filter(|r| r.int("free_race_discipline_id") == Some(id))
                                .filter_map(|r| {
                                    let minutes = u32::try_from(r.int("minutes")?).ok()?;
                                    let text = r.text("time_string_id")?;
                                    Some((
                                        minutes,
                                        label(
                                            labels,
                                            &format!("db_{text}"),
                                            &format!("{minutes} min"),
                                        ),
                                    ))
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    rows.sort();
                    rows.dedup_by_key(|r| r.0);
                    rows
                } else {
                    Vec::new()
                };
                types.push(EventType {
                    kind,
                    key: name.clone(),
                    label: label(labels, &format!("db_{name}"), &name),
                    layouts: layouts(kind, routes),
                    cars,
                    stand_in_cars: stand_in,
                    default_laps: match kind {
                        Kind::Series => Some(3),
                        // The career events' 8 laps cap the lap count [D].
                        Kind::LeMans => Some(8),
                        Kind::DriftBattle | Kind::DriftGp => Some(2),
                        // Western_states_D: 6 laps on both stadium routes [D].
                        Kind::Demolition => Some(6),
                        _ => None,
                    },
                    opponents: match kind {
                        Kind::DriftBattle => {
                            let most = max_cars.saturating_sub(1).clamp(1, 11);
                            Some((most, most))
                        }
                        Kind::Demolition => Some((11, 11)),
                        _ => None,
                    },
                    times,
                    directions: kind == Kind::Touge,
                    experimental: grid_tools::series::is_experimental(&name),
                });
            }
        }
        types.retain(|t| {
            !t.layouts.is_empty()
                && !t.cars.is_empty()
                && (!matches!(t.kind, Kind::Freestyle | Kind::LeMans) || !t.times.is_empty())
        });
        Self { types }
    }

    pub fn find(&self, key: &str) -> Option<&EventType> {
        self.types.iter().find(|t| t.key.eq_ignore_ascii_case(key))
    }
}

impl EventType {
    /// A valid choice of this type, keeping what `old` picked where possible.
    pub fn choice(&self, old: &Choice) -> Choice {
        let route = self
            .layouts
            .iter()
            .find(|l| l.route == old.route)
            .unwrap_or(&self.layouts[0])
            .route
            .clone();
        let car = self
            .cars
            .iter()
            .find(|c| **c == old.car)
            .unwrap_or(&self.cars[0])
            .clone();
        // Laps and opponents carry over within a type; a new type starts
        // from its defaults.
        let same = old.key == self.key;
        let laps = match self.default_laps {
            Some(_) if same && (1..=MAX_LAPS).contains(&old.laps) => old.laps,
            Some(d) => d,
            None => 1,
        };
        let opponents = match self.opponents {
            Some((most, _)) if same && (1..=most).contains(&old.opponents) => old.opponents,
            Some((_, d)) => d,
            None => 0,
        };
        let minutes = if self.times.iter().any(|t| t.0 == old.minutes) {
            old.minutes
        } else {
            // AUTHORED default: 2 minutes, else the shortest.
            self.times
                .iter()
                .find(|t| t.0 == 2)
                .or(self.times.first())
                .map_or(0, |t| t.0)
        };
        Choice {
            key: self.key.clone(),
            route,
            car,
            laps,
            opponents,
            minutes,
            subroute: if self.directions {
                old.subroute.min(1)
            } else {
                0
            },
        }
    }
}

const AI: &str = "[ai]\ndifficulty = 0.606\ncatchup = on\nchoreographer = on\nmistakes = on\n";
/// Touge formats use the `_touge` catch-up configs (0x5D5530, F439).
const AI_TOUGE: &str =
    "[ai]\ndifficulty = 0.606\ncatchup = touge\nchoreographer = on\nmistakes = on\n";
const PRIVATE: &str =
    "[private]\nbody_contacts = derive\nbody_events = derive\ndeformation = derive\n";

/// The settings profile of a choice: the same keys as the launchers'
/// `settings/*.ini` for that event.
pub fn profile(kind: Kind, c: &Choice) -> String {
    let mut game = format!("[game]\nroute = {}\ncar = {}\n", c.route, c.car);
    let mut drive = String::from("[drive]\nenabled = on\n");
    let field = "grid_position = back\nai_characteristics = ai-driver-characteristics.tsv\n";
    let ai = match kind {
        Kind::Race => {
            drive += &format!("laps = {}\nopponents = {}\n{field}", c.laps, c.opponents);
            AI
        }
        Kind::TimeTrial => {
            drive += &format!("laps = {}\n", c.laps);
            ""
        }
        Kind::Series => {
            game = format!(
                "[game]\nseries = {}\nroute = {}\ncar = {}\n",
                c.key, c.route, c.car
            );
            drive += &format!("laps = {}\n{field}", c.laps);
            AI
        }
        Kind::DriftBattle => {
            game += &format!("opponent_cars = {}\n", c.car);
            drive += &format!(
                "discipline = drift_battle\nlaps = {}\nopponents = {}\n{field}",
                c.laps, c.opponents
            );
            AI
        }
        Kind::DriftGp => {
            drive += &format!("discipline = drift_grand_prix\nlaps = {}\n", c.laps);
            "[ai]\ndifficulty = 0.606\n"
        }
        Kind::Freestyle => {
            drive += &format!(
                "discipline = drift_freestyle\ntime_limit = {}\n",
                c.minutes * 60
            );
            "[ai]\ndifficulty = 0.606\n"
        }
        Kind::Touge => {
            // The opponent drives the player's car (the default field).
            game += &format!("subroute = {}\n", c.subroute);
            drive += &format!("laps = 1\nopponents = 1\n{field}");
            AI_TOUGE
        }
        Kind::ProTouge => {
            drive += &format!("discipline = pro_touge\nlaps = 1\nopponents = 1\n{field}");
            AI_TOUGE
        }
        Kind::LeMans => {
            game = format!(
                "[game]\nseries = {}\nroute = {}\ncar = {}\n",
                c.key.to_ascii_lowercase(),
                c.route,
                c.car
            );
            drive += &format!(
                "laps = {}\ntime_limit = {}\n{field}",
                c.laps,
                c.minutes * 60
            );
            AI
        }
        Kind::Demolition => {
            game += &format!("opponent_cars = {}\n", c.car);
            drive += &format!(
                "discipline = demolition_derby\nlaps = {}\nopponents = {}\n{field}",
                c.laps, c.opponents
            );
            AI
        }
    };
    format!(
        "# Written by the event menu (docs/EVENT_MENU.md); chosen again in the menu.\n{game}{drive}{ai}{PRIVATE}"
    )
}

/// `[event]` keys of the choice (settings::with_keys).
pub fn keys(c: &Choice) -> Vec<(&'static str, Option<String>)> {
    vec![
        ("type", Some(c.key.clone())),
        ("route", Some(c.route.clone())),
        ("car", Some(c.car.clone())),
        ("laps", Some(c.laps.to_string())),
        ("opponents", Some(c.opponents.to_string())),
        ("time", Some(c.minutes.to_string())),
        (
            "direction",
            Some(
                if c.subroute == 1 {
                    "uphill"
                } else {
                    "downhill"
                }
                .into(),
            ),
        ),
    ]
}

/// The saved choice from `[event]` (settings::Settings::event).
pub fn saved(event: &[(String, String)]) -> Choice {
    let mut c = Choice::default();
    for (key, value) in event {
        match key.as_str() {
            "type" => c.key.clone_from(value),
            "route" => c.route.clone_from(value),
            "car" => c.car.clone_from(value),
            "laps" => c.laps = value.parse().unwrap_or(0),
            "opponents" => c.opponents = value.parse().unwrap_or(0),
            "time" => c.minutes = value.parse().unwrap_or(0),
            "direction" => c.subroute = u8::from(value == "uphill"),
            _ => {}
        }
    }
    c
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    Discipline,
    Car,
    Location,
    Route,
    Laps,
    Time,
    Opponents,
    Direction,
    Continue,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Command {
    /// Continue: start this event.
    Go(Choice),
    /// Back on the screen: close the menu.
    Quit,
}

/// The menu's state: the catalog, the current choice and the cursor.
#[derive(Resource, Clone, Debug)]
pub struct Menu {
    pub catalog: Catalog,
    pub choice: Choice,
    cursor: usize,
    repeat: Repeat,
}

fn step_in<T: Clone + PartialEq>(list: &[T], current: &T, right: bool) -> T {
    let n = list.len();
    let i = list.iter().position(|x| x == current).unwrap_or(0);
    list[if right { (i + 1) % n } else { (i + n - 1) % n }].clone()
}

impl Menu {
    pub fn new(catalog: Catalog, saved: &Choice) -> Self {
        let kind = catalog
            .find(&saved.key)
            .or(catalog.types.first())
            .expect("a catalog with an event");
        let choice = kind.choice(saved);
        Self {
            catalog,
            choice,
            // Continue, like the original's `active_default` race buttons.
            cursor: 0,
            repeat: Repeat::default(),
        }
        .at_continue()
    }

    fn at_continue(mut self) -> Self {
        self.cursor = self.rows().len() - 1;
        self
    }

    pub fn kind(&self) -> &EventType {
        self.catalog
            .find(&self.choice.key)
            .expect("the choice's type is in the catalog")
    }

    /// The original's button order (discipline, car, location, route,
    /// laps), then the AUTHORED rows and Continue.
    pub fn rows(&self) -> Vec<Row> {
        let t = self.kind();
        let mut rows = vec![Row::Discipline, Row::Car, Row::Location, Row::Route];
        if t.default_laps.is_some() {
            rows.push(Row::Laps);
        }
        if !t.times.is_empty() {
            rows.push(Row::Time);
        }
        if t.opponents.is_some() {
            rows.push(Row::Opponents);
        }
        if t.directions {
            rows.push(Row::Direction);
        }
        rows.push(Row::Continue);
        rows
    }

    fn venues(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for l in &self.kind().layouts {
            if !out.contains(&l.venue) {
                out.push(l.venue.clone());
            }
        }
        out
    }

    fn layout(&self) -> &Layout {
        let t = self.kind();
        t.layouts
            .iter()
            .find(|l| l.route == self.choice.route)
            .unwrap_or(&t.layouts[0])
    }

    /// Left (`right = false`) or right on a row.
    fn change(&mut self, row: Row, right: bool) {
        let t = self.kind().clone();
        let venue = self.layout().venue.clone();
        let venues = self.venues();
        let c = &mut self.choice;
        match row {
            Row::Discipline => {
                let keys: Vec<String> = self.catalog.types.iter().map(|t| t.key.clone()).collect();
                let key = step_in(&keys, &c.key, right);
                let next = self.catalog.find(&key).expect("listed type");
                *c = next.choice(c);
            }
            Row::Car => c.car = step_in(&t.cars, &c.car, right),
            Row::Location => {
                let next = step_in(&venues, &venue, right);
                if let Some(l) = t.layouts.iter().find(|l| l.venue == next) {
                    c.route = l.route.clone();
                }
            }
            Row::Route => {
                let routes: Vec<String> = t
                    .layouts
                    .iter()
                    .filter(|l| l.venue == venue)
                    .map(|l| l.route.clone())
                    .collect();
                c.route = step_in(&routes, &c.route, right);
            }
            Row::Laps => {
                let laps: Vec<u32> = (1..=MAX_LAPS).collect();
                c.laps = step_in(&laps, &c.laps, right);
            }
            Row::Time => {
                let times: Vec<u32> = t.times.iter().map(|t| t.0).collect();
                c.minutes = step_in(&times, &c.minutes, right);
            }
            Row::Opponents => {
                let most = t.opponents.map_or(1, |o| o.0);
                let list: Vec<usize> = (1..=most).collect();
                c.opponents = step_in(&list, &c.opponents, right);
            }
            Row::Direction => c.subroute = 1 - c.subroute.min(1),
            Row::Continue => {}
        }
    }

    pub fn step(&mut self, nav: Nav) -> Option<Command> {
        let rows = self.rows();
        let n = rows.len();
        self.cursor = self.cursor.min(n - 1);
        if nav.up {
            self.cursor = (self.cursor + n - 1) % n;
        }
        if nav.down {
            self.cursor = (self.cursor + 1) % n;
        }
        let row = rows[self.cursor];
        if nav.left || nav.right {
            self.change(row, nav.right);
            // A new discipline may have other rows; stay on Discipline.
            self.cursor = self.rows().iter().position(|r| *r == row).unwrap_or(0);
        }
        if nav.select {
            if row == Row::Continue {
                return Some(Command::Go(self.choice.clone()));
            }
            // Select on a value row moves on, towards Continue.
            self.cursor = (self.cursor + 1).min(n - 1);
        }
        if nav.back {
            return Some(Command::Quit);
        }
        None
    }

    /// Mouse: a click selects a row; a click on the selected row changes it
    /// to the next value, or on Continue starts the event.
    pub fn click(&mut self, index: usize) -> Option<Command> {
        let rows = self.rows();
        let row = *rows.get(index)?;
        if self.cursor != index {
            self.cursor = index;
            return None;
        }
        if row == Row::Continue {
            return Some(Command::Go(self.choice.clone()));
        }
        self.change(row, true);
        None
    }

    pub fn view(&self, labels: &Labels) -> View {
        let t = self.kind();
        let c = &self.choice;
        let l = |id: &str, fallback: &str| label(labels, id, fallback);
        let rows = self
            .rows()
            .into_iter()
            .enumerate()
            .map(|(i, row)| {
                let (name, value) = match row {
                    Row::Discipline => (
                        l("free_race_button_discipline", "Discipline"),
                        t.label.clone(),
                    ),
                    Row::Car => (
                        l("free_race_button_car", "Car"),
                        Car::new(&c.car).map_or(c.car.clone(), |car| car.label().to_string()),
                    ),
                    Row::Location => (
                        l("free_race_button_track_select", "Location"),
                        self.layout().venue.clone(),
                    ),
                    Row::Route => (
                        l("free_race_button_track_type", "Route"),
                        self.layout().name.clone(),
                    ),
                    Row::Laps => (l("free_race_button_laps", "Laps"), c.laps.to_string()),
                    Row::Time => (
                        l("free_race_button_time", "Time"),
                        t.times
                            .iter()
                            .find(|x| x.0 == c.minutes)
                            .map_or(String::new(), |x| x.1.clone()),
                    ),
                    Row::Opponents => ("Opponents".into(), c.opponents.to_string()),
                    Row::Direction => (
                        "Direction".into(),
                        if c.subroute == 1 {
                            "Uphill"
                        } else {
                            "Downhill"
                        }
                        .into(),
                    ),
                    Row::Continue => (l("raceday_buttontext_to_race", "Continue"), String::new()),
                };
                (name, value, true, i == self.cursor)
            })
            .collect();
        let mut help = String::from(
            "Up/Down choose, Left/Right change, Select on Continue starts the event; Race Options follow.",
        );
        if t.experimental {
            help += " Experimental: some cars of this series cannot race yet.";
        }
        if t.stand_in_cars {
            help +=
                " The original's cars of this discipline are not ported yet: ported cars stand in.";
        }
        View {
            title: l("free_race_menu_screen_title", "Race Day"),
            subtitle: format!("{} - {}", t.label, self.layout().venue),
            rows,
            help,
        }
    }
}

// ---- Processes -----------------------------------------------------------

/// Present in a race started from the menu: leaving the event goes back.
#[derive(Resource)]
pub struct FromMenu;

/// Leaves the event: back to the menu when the race came from it, else
/// closes the app (the launchers' behaviour).
pub fn leave(exit: &mut MessageWriter<AppExit>, from_menu: bool) {
    if !from_menu {
        exit.write(AppExit::Success);
        return;
    }
    log_line!("drive_event_menu version=1 event=back_to_menu");
    #[cfg(target_os = "android")]
    crate::android::relaunch();
    #[cfg(not(target_os = "android"))]
    exit.write(AppExit::from_code(BACK_TO_MENU as u8));
}

/// The flags a supervised child gets from the supervisor's own arguments.
#[cfg(not(target_os = "android"))]
pub(crate) fn passed_on(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if matches!(
            args[i].as_str(),
            "--game-root" | "--data-dir" | "--settings"
        ) && i + 1 < args.len()
        {
            out.extend_from_slice(&args[i..i + 2]);
            i += 2;
        } else {
            i += 1;
        }
    }
    out
}

/// `grid-app --menu ...` on the desktop: the menu process, then the chosen
/// race, and again while the race ends with "back to menu". Each is a new
/// process because winit allows one event loop per process. Returns the
/// exit code.
#[cfg(not(target_os = "android"))]
pub fn supervise(args: &[String]) -> Result<i32, Box<dyn std::error::Error>> {
    let exe = std::env::current_exe()?;
    let flags = passed_on(args);
    let dir = flags
        .windows(2)
        .rev()
        .find(|w| w[0] == "--data-dir")
        .map(|w| PathBuf::from(&w[1]))
        .unwrap_or_else(crate::settings::default_data_dir);
    loop {
        let menu = std::process::Command::new(&exe)
            .arg("--menu-screen")
            .args(&flags)
            .status()?;
        if menu.code() != Some(MENU_GO) {
            return Ok(menu.code().unwrap_or(1));
        }
        // An experimental series runs with the gate (none since F420).
        let mut race = std::process::Command::new(&exe);
        if experimental(&dir) {
            race.env("GRID_SERIES_EXPERIMENTAL", "1");
        }
        race.args(&flags)
            .arg("--settings")
            .arg(dir.join(PROFILE))
            .arg("--from-menu");
        if let Some(ticks) = auto_ticks() {
            race.args(["--drive-ticks", &ticks.to_string()]);
        }
        let race = race.status()?;
        if race.code() != Some(BACK_TO_MENU) {
            return Ok(race.code().unwrap_or(1));
        }
    }
}

/// Diagnostics `GRID_MENU_AUTO=TICKS` (unattended checks, desktop; the build
/// gate's event-menu default, docs/TOOLCHAIN.md "Build gate"): the menu
/// presses Continue after 3 s with its current choice, and the supervisor
/// runs that race as a scripted run of TICKS ticks, so `GRID_AUTOPILOT` drives.
#[cfg(not(target_os = "android"))]
fn auto_ticks() -> Option<u64> {
    std::env::var("GRID_MENU_AUTO").ok()?.parse().ok()
}

/// Whether the saved choice is an experimental event (Le Mans Series).
#[cfg(not(target_os = "android"))]
fn experimental(dir: &Path) -> bool {
    let mut s = crate::settings::Settings::new();
    std::fs::read_to_string(dir.join("settings.ini"))
        .is_ok_and(|text| s.apply(text.trim_start_matches('\u{feff}'), dir).is_ok())
        && grid_tools::series::is_experimental(&saved(&s.event).key)
}

/// Android: the arguments of this start. A start after the menu's Continue
/// (the marker file) runs the chosen event; otherwise the menu opens, unless
/// `[front_end] event_menu = off`, which keeps the old direct start.
#[cfg(target_os = "android")]
pub fn android_args() -> Vec<String> {
    let dir = crate::settings::default_data_dir();
    let marker = dir.join(GO_MARKER);
    if marker.is_file() {
        // Consumed first, so a crash in the race opens the menu next time.
        let _ = std::fs::remove_file(&marker);
        return vec![
            "--settings".into(),
            dir.join(PROFILE).display().to_string(),
            "--from-menu".into(),
        ];
    }
    let menu = crate::settings::load(&[])
        .map(|s| s.event_menu != Some(false))
        .unwrap_or(true);
    if menu {
        vec!["--menu-screen".into()]
    } else {
        Vec::new()
    }
}

/// The menu's starting choice (F451): `settings.ini`'s `[event]` (the last
/// Continue), unless the bundled APK's `event.ini` is newer than the last
/// menu profile (`event-menu.ini`). A new bundle (the drift APK) then opens
/// on its own event even when an earlier APK left a saved Race choice, which
/// otherwise always wins because `settings.ini` is read after `event.ini`.
fn starting_event(merged: &[(String, String)], dir: &std::path::Path) -> Vec<(String, String)> {
    let modified = |name: &str| {
        std::fs::metadata(dir.join(name))
            .and_then(|m| m.modified())
            .ok()
    };
    let newer = match (modified("event.ini"), modified(PROFILE)) {
        (Some(_), None) => true,
        (Some(event), Some(profile)) => event > profile,
        _ => false,
    };
    if newer && let Ok(text) = std::fs::read_to_string(dir.join("event.ini")) {
        let mut seed = crate::settings::Settings::default();
        if seed.apply(text.trim_start_matches('\u{feff}'), dir).is_ok() && !seed.event.is_empty() {
            log_line!("drive_event_menu version=1 event=start source=event.ini");
            return seed.event;
        }
    }
    merged.to_vec()
}

// ---- The menu screen -----------------------------------------------------

#[derive(Resource)]
struct Saving {
    settings: PathBuf,
    profile: PathBuf,
}

/// Runs the menu as its own app (`--menu-screen`). Exits with [`MENU_GO`]
/// after Continue (Android: restarts into the race), 0 after Back.
pub fn screen(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let profile = crate::settings::load(args)?;
    let root = crate::game_root(profile.root.clone())?;
    println!("game_root path={}", root.display());
    let labels = Labels::load(&root);
    let catalog = Catalog::load(&root, &labels);
    if catalog.types.is_empty() {
        return Err(format!(
            "no event can run with the game data in {} (docs/EVENT_MENU.md)",
            root.display()
        )
        .into());
    }
    log_line!(
        "drive_event_menu version=1 event=open types={}",
        catalog
            .types
            .iter()
            .map(|t| format!("{}:{}x{}", t.key, t.layouts.len(), t.cars.len()))
            .collect::<Vec<_>>()
            .join(",")
    );
    let start = starting_event(&profile.event, crate::settings::data_dir());
    let menu = Menu::new(catalog, &saved(&start));
    let input_settings = profile.input()?;
    #[cfg(target_os = "android")]
    let touch_screen = crate::android::touch_screen();
    #[cfg(not(target_os = "android"))]
    let touch_screen = false;
    let touch = input_settings.touch_active(touch_screen);
    let (actions, pad) = crate::input::load(&root, &input_settings, touch)?;
    println!("drive_input {input_settings} pad_preset={pad} menu=on");
    let data = crate::settings::data_dir();
    let mut app = App::new();
    app.add_plugins(crate::with_profile(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: "GRID Reassembly".into(),
            resolution: (1280, 800).into(),
            present_mode: profile.present_mode.unwrap_or_default(),
            ..default()
        }),
        ..default()
    })));
    crate::ui_font::install(&mut app);
    #[cfg(target_os = "android")]
    crate::android_display::install(&mut app, profile.refresh_hz.flatten());
    crate::input::install(&mut app, actions);
    #[cfg(target_os = "android")]
    crate::android_pad::install(&mut app);
    if touch {
        // The P11 menu zones (UP/DOWN, < >, OK, BACK); no tilt here.
        crate::touch_controls::install(
            &mut app,
            None,
            input_settings.device == crate::input::Device::Auto,
        );
    }
    let exit = app
        .insert_resource(menu)
        .insert_resource(labels)
        .insert_resource(Saving {
            settings: data.join("settings.ini"),
            profile: data.join(PROFILE),
        })
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
struct Title;
#[derive(Component)]
struct Subtitle;
#[derive(Component)]
struct RowText(usize);
#[derive(Component)]
struct Help;

/// The most rows a type shows (Discipline..Route, Laps, Time, Opponents,
/// Direction, Continue).
const ROWS: usize = 9;

/// The pause menu's panel look (AUTHORED, docs/PAUSE_MENU.md), full screen.
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
                        row_gap: px(8.),
                        min_width: px(520.),
                        max_width: px(760.),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.025, 0.04, 0.065, 0.92)),
                ))
                .with_children(|panel| {
                    panel.spawn((Title, Text::new(""), font(30.), TextColor(Color::WHITE)));
                    panel.spawn((
                        Subtitle,
                        Text::new(""),
                        font(18.),
                        TextColor(Color::srgb(0.75, 0.8, 0.85)),
                    ));
                    for i in 0..ROWS {
                        // Button: the mouse selects and changes rows.
                        panel.spawn((
                            RowText(i),
                            Button,
                            Text::new(""),
                            font(22.),
                            TextColor(Color::WHITE),
                        ));
                    }
                    panel.spawn((
                        Help,
                        Text::new(""),
                        font(15.),
                        TextColor(Color::srgb(0.6, 0.65, 0.7)),
                    ));
                });
        });
}

// Android restarts the activity instead of exiting.
#[cfg_attr(target_os = "android", allow(unused_variables, unused_mut))]
fn update(
    mut menu: ResMut<Menu>,
    actions: Res<crate::input::Actions>,
    clicks: Query<(&Interaction, &RowText), Changed<Interaction>>,
    saving: Res<Saving>,
    time: Res<Time<Real>>,
    mut exit: MessageWriter<AppExit>,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let mut repeat = menu.repeat;
    let mut nav = crate::pause_menu::nav(&actions, &mut repeat, time.delta_secs());
    menu.repeat = repeat;
    // Escape is both Pause and Menu Back.
    nav.back |= nav.pause;
    // Diagnostics GRID_MENU_AUTO: Continue with the current choice 3 s after
    // start (each menu is its own process).
    #[cfg(not(target_os = "android"))]
    if auto_ticks().is_some() && time.elapsed_secs() >= 3. {
        menu.cursor = menu.rows().len() - 1;
        nav.select = true;
    }
    let mut command = menu.step(nav);
    for (interaction, row) in &clicks {
        if *interaction == Interaction::Pressed {
            command = command.or(menu.click(row.0));
        }
    }
    match command {
        Some(Command::Go(choice)) => {
            *done = true;
            let kind = menu.kind().kind;
            match go(&saving, kind, &choice) {
                Ok(()) => {
                    log_line!(
                        "drive_event_menu version=1 event=go type={} route={} car={} laps={} opponents={} time={} subroute={}",
                        choice.key,
                        choice.route,
                        choice.car,
                        choice.laps,
                        choice.opponents,
                        choice.minutes,
                        choice.subroute
                    );
                    #[cfg(target_os = "android")]
                    crate::android::relaunch();
                    #[cfg(not(target_os = "android"))]
                    exit.write(AppExit::from_code(MENU_GO as u8));
                }
                Err(error) => {
                    *done = false;
                    warn!("event not started: {error}");
                    log_line!("drive_event_menu version=1 event=go_failed error={error:?}");
                }
            }
        }
        Some(Command::Quit) => {
            // Android: Back is the system's (the activity stays).
            #[cfg(not(target_os = "android"))]
            {
                *done = true;
                log_line!("drive_event_menu version=1 event=quit");
                exit.write(AppExit::Success);
            }
        }
        None => {}
    }
}

/// Saves the choice in `[event]` and writes the event profile (and on
/// Android the marker that makes the next start the race).
fn go(saving: &Saving, kind: Kind, choice: &Choice) -> Result<(), String> {
    let old = match std::fs::read_to_string(&saving.settings) {
        Ok(text) => text.trim_start_matches('\u{feff}').to_owned(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(format!("{}: {e}", saving.settings.display())),
    };
    let new = crate::settings::with_keys(&old, "event", &keys(choice));
    let mut check = crate::settings::Settings::new();
    let dir = saving.settings.parent().unwrap_or(Path::new("."));
    check.apply(&new, dir)?;
    if saved(&check.event) != *choice {
        return Err("saved event choice does not read back".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    std::fs::write(&saving.settings, new)
        .map_err(|e| format!("{}: {e}", saving.settings.display()))?;
    std::fs::write(&saving.profile, profile(kind, choice))
        .map_err(|e| format!("{}: {e}", saving.profile.display()))?;
    #[cfg(target_os = "android")]
    std::fs::write(dir.join(GO_MARKER), b"go\n").map_err(|e| e.to_string())?;
    Ok(())
}

#[allow(clippy::type_complexity)]
fn present(
    menu: Res<Menu>,
    labels: Res<Labels>,
    mut texts: ParamSet<(
        Query<&mut Text, With<Title>>,
        Query<&mut Text, With<Subtitle>>,
        Query<(&RowText, &mut Text, &mut TextColor, &mut Node)>,
        Query<&mut Text, With<Help>>,
    )>,
    mut shown: Local<Option<View>>,
) {
    let view = menu.view(&labels);
    if shown.as_ref() == Some(&view) {
        return;
    }
    for mut text in &mut texts.p0() {
        text.0.clone_from(&view.title);
    }
    for mut text in &mut texts.p1() {
        text.0.clone_from(&view.subtitle);
    }
    for (row, mut text, mut colour, mut node) in &mut texts.p2() {
        let Some((label, value, _, selected)) = view.rows.get(row.0) else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;
        let marker = if *selected { "> " } else { "   " };
        text.0 = if value.is_empty() {
            format!("{marker}{label}")
        } else {
            format!("{marker}{label}:  < {value} >")
        };
        colour.0 = if *selected {
            Color::srgb(1., 0.78, 0.2)
        } else {
            Color::WHITE
        };
    }
    for mut text in &mut texts.p3() {
        text.0.clone_from(&view.help);
    }
    *shown = Some(view);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_newer_bundled_event_ini_sets_the_starting_choice() {
        let dir = std::env::temp_dir().join(format!("grid-menu-start-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let saved = vec![("type".to_string(), "race".to_string())];
        let at = |name: &str, seconds: u64| {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .open(dir.join(name))
                .unwrap();
            file.set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds))
                .unwrap();
        };
        // No event.ini: the saved choice.
        assert_eq!(starting_event(&saved, &dir), saved);
        std::fs::write(
            dir.join("event.ini"),
            "[event]\ntype = drift_battle\nroute = tracks/europe/nurburgring/route_1\n",
        )
        .unwrap();
        // event.ini and no menu profile yet: the bundle's choice.
        let start = starting_event(&saved, &dir);
        assert_eq!(saved_choice_key(&start), "drift_battle");
        // A menu profile written after the install: the owner's choice.
        std::fs::write(dir.join(PROFILE), "").unwrap();
        at("event.ini", 1_000);
        at(PROFILE, 2_000);
        assert_eq!(starting_event(&saved, &dir), saved);
        // A newer install (event.ini replaced): the bundle's choice again.
        at("event.ini", 3_000);
        assert_eq!(
            saved_choice_key(&starting_event(&saved, &dir)),
            "drift_battle"
        );
        // An event.ini without [event] (the Donington bundle) keeps the saved one.
        std::fs::write(dir.join("event.ini"), "# Donington\n").unwrap();
        at("event.ini", 4_000);
        assert_eq!(starting_event(&saved, &dir), saved);
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn saved_choice_key(event: &[(String, String)]) -> String {
        saved(event).key
    }

    fn catalog() -> Catalog {
        Catalog::build(None, &Labels::default(), &|_| true)
    }

    fn press(f: impl Fn(&mut Nav)) -> Nav {
        let mut nav = Nav::default();
        f(&mut nav);
        nav
    }

    #[test]
    fn without_a_database_race_and_time_trial_remain() {
        let c = catalog();
        let keys: Vec<&str> = c.types.iter().map(|t| t.key.as_str()).collect();
        assert_eq!(keys, ["race", "time_trial"]);
        assert_eq!(c.types[0].layouts.len(), route::CIRCUITS.len());
        // The phone's subset: only Donington and the Lacetti.
        let phone = Catalog::build(None, &Labels::default(), &|p| {
            p.starts_with("tracks/europe/donington") || p == "cars/lac"
        });
        assert_eq!(phone.types[0].layouts.len(), 1);
        assert_eq!(phone.types[0].cars, ["lac"]);
    }

    /// The bundles' subsets with the database (F539): the Donington bundle
    /// has no BMW, so Touring Cars (whose field draws BMWs) is not offered;
    /// the drift bundle has both touring cars and offers it.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT purchased source"]
    fn a_series_with_a_missing_car_is_left_out() {
        let root = PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let db = grid_tools::read_database(&root).unwrap();
        let keys = |cars: &[&str]| -> Vec<String> {
            Catalog::build(Some(&db), &Labels::default(), &|p| {
                p.starts_with("tracks/europe/donington")
                    || p.starts_with("tracks/europe/nurburgring")
                    || cars.iter().any(|c| p == format!("cars/{c}"))
            })
            .types
            .into_iter()
            .map(|t| t.key)
            .collect()
        };
        let donington = keys(&["lac"]);
        assert!(donington.contains(&"race".to_string()), "{donington:?}");
        assert!(!donington.contains(&"touring".to_string()), "{donington:?}");
        assert!(keys(&["lac", "bmw"]).contains(&"touring".to_string()));
    }

    #[test]
    fn the_menu_opens_on_continue_and_walks_the_original_button_order() {
        let mut m = Menu::new(catalog(), &Choice::default());
        assert_eq!(
            m.rows(),
            [
                Row::Discipline,
                Row::Car,
                Row::Location,
                Row::Route,
                Row::Laps,
                Row::Opponents,
                Row::Continue
            ]
        );
        let view = m.view(&Labels::default());
        assert_eq!(view.title, "Race Day");
        assert!(view.rows.last().unwrap().3);
        assert_eq!(
            m.step(press(|n| n.select = true)),
            Some(Command::Go(m.choice.clone()))
        );
        // Down wraps to Discipline; Right picks Time Trial, which has no
        // opponents row.
        m.step(press(|n| n.down = true));
        m.step(press(|n| n.right = true));
        assert_eq!(m.choice.key, "time_trial");
        assert!(!m.rows().contains(&Row::Opponents));
        assert_eq!(m.step(press(|n| n.back = true)), Some(Command::Quit));
    }

    #[test]
    fn location_and_route_cycle_and_the_profile_matches_the_launchers() {
        let mut m = Menu::new(catalog(), &Choice::default());
        let start = m.choice.route.clone();
        m.change(Row::Location, true);
        assert_ne!(m.choice.route, start);
        m.change(Row::Location, false);
        assert_eq!(m.choice.route, start);
        let menu = parsed(&profile(Kind::Race, &m.choice));
        assert_eq!(menu, launcher("race").0);
    }

    /// A settings text as the race reads it; the defaults of route, car and
    /// subroute made explicit, comment-only text before the first section
    /// dropped.
    fn parsed(text: &str) -> crate::settings::Settings {
        let mut s = crate::settings::Settings::new();
        s.apply(text, Path::new("d")).unwrap();
        s.route.get_or_insert_with(|| route::DONINGTON.into());
        s.car.get_or_insert_with(|| "lac".into());
        if s.subroute == Some(0) {
            s.subroute = None;
        }
        s.assists = s.assists.trim().into();
        s
    }

    fn launcher(name: &str) -> (crate::settings::Settings, String) {
        let text = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../settings/{name}.ini")),
        )
        .unwrap();
        (parsed(&text), text)
    }

    /// The menu offers every launcher's event and writes the same settings
    /// (the launchers keep working; a menu race is the launcher's race).
    #[test]
    #[ignore = "requires GRID_GAME_ROOT purchased source"]
    fn every_launcher_event_is_offered_and_written_identically() {
        let root = PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let labels = Labels::load(&root);
        let catalog = Catalog::load(&root, &labels);
        let mut names: Vec<String> =
            std::fs::read_dir(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../settings"))
                .unwrap()
                .filter_map(|e| {
                    e.ok()?
                        .file_name()
                        .into_string()
                        .ok()?
                        .strip_suffix(".ini")
                        .map(String::from)
                })
                .collect();
        names.sort();
        let mut checked = 0;
        for name in names {
            // Practice and the body-contact test drive are not events.
            if name == "practice" || name == "body-contacts" {
                continue;
            }
            let (ini, _) = launcher(&name);
            let key = match (&ini.series, ini.discipline.as_deref(), ini.opponents) {
                (Some(series), _, _) => series.clone(),
                (None, Some("drift_battle"), _) => "drift_battle".into(),
                (None, Some("drift_grand_prix"), _) => "drift_gp".into(),
                (None, Some("drift_freestyle"), _) => "Free_Drift".into(),
                (None, Some("pro_touge"), _) => "legal_touge".into(),
                (None, Some("demolition_derby"), _) => "Demolition_D".into(),
                (None, _, _) if ini.route.as_deref() == Some(route::HARUNA) => {
                    "illegal_touge".into()
                }
                (None, _, Some(_)) => "race".into(),
                (None, _, None) => "time_trial".into(),
            };
            let t = catalog
                .find(&key)
                .unwrap_or_else(|| panic!("{name}: no type {key}"));
            let choice = Choice {
                key: key.clone(),
                route: ini.route.clone().unwrap(),
                car: ini.car.clone().unwrap(),
                laps: ini.laps.unwrap_or(1),
                opponents: ini.opponents.unwrap_or(0),
                minutes: ini.time_limit.map_or(0, |s| (s / 60.).round() as u32),
                subroute: ini.subroute.unwrap_or(0),
            };
            assert!(
                t.layouts.iter().any(|l| l.route == choice.route),
                "{name}: route"
            );
            assert!(t.cars.contains(&choice.car), "{name}: car");
            let mut menu = parsed(&profile(t.kind, &choice));
            let mut want = ini.clone();
            if t.kind == Kind::Freestyle {
                // The launcher's 90 s is not one of the original's times
                // (free_race_times: whole minutes) [D].
                assert!(t.times.iter().any(|x| x.0 == 1) && t.times.iter().any(|x| x.0 == 2));
                want.time_limit = None;
                menu.time_limit = None;
            }
            assert_eq!(menu, want, "{name}");
            checked += 1;
        }
        assert!(checked >= 18, "{checked}");
    }

    #[test]
    fn the_saved_choice_round_trips_through_settings_keys() {
        let c = Choice {
            key: "illegal_touge".into(),
            route: route::HARUNA.into(),
            car: "sup".into(),
            laps: 1,
            opponents: 0,
            minutes: 0,
            subroute: 1,
        };
        let text = crate::settings::with_keys("# mine\n[osd]\nfps = on\n", "event", &keys(&c));
        let mut s = crate::settings::Settings::new();
        s.apply(&text, Path::new("d")).unwrap();
        assert_eq!(saved(&s.event), c);
        assert!(text.starts_with("# mine\n[osd]\nfps = on\n"));
    }
}
