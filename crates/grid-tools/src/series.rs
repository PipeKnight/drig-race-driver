//! Circuit-race series of the free race (Race Day): Pro Tuned, Touring, GT1,
//! GT2 and the Le Mans Series are `single_race` events that differ only in
//! their cars and track pools (docs/DISCIPLINES.md §1.2, docs/SERIES.md).
//!
//! Everything here is read from `database.bin` [D]:
//! - `free_race_discipline` names the series (`pro_tuned`, `touring`, ...);
//! - `free_race_restrictions` lists the series' vehicles, each row pointing
//!   at one `free_race_track_pool` row whose `name` is the series' pool;
//! - the pool's rows are its track layouts (`track_model`) with `max_cars`.
use crate::car::Car;
use grid_formats::database::Database;

/// One layout of a series' track pool.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// `free_race_track_pool.id`.
    pub id: i32,
    /// `track_model.name_string_id`, e.g. `donington_nat`.
    pub name: String,
    /// Route folder, e.g. `tracks/europe/donington/route_1`.
    pub route: String,
    pub max_cars: usize,
    pub sort_order: i32,
}

/// A free-race series: its cars (in `free_race_restrictions` row order, each
/// stem once) and its track pool (in `sort_order`, then id order).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Series {
    pub name: String,
    pub cars: Vec<Car>,
    pub layouts: Vec<Layout>,
}

/// The circuit-race series this port offers. `le_mans` is the free race
/// `Le_Mans` (discipline 18): the 24 Hours on Circuit de la Sarthe, a
/// time-limited race with the `lemans_24` field (docs/LE_MANS.md).
pub const CIRCUIT_SERIES: [&str; 11] = [
    "pro_tuned",
    "touring",
    "gt1",
    "gt2",
    "le_mans_series",
    "le_mans",
    "prototype",
    "pro_muscle",
    "open_wheel",
    "gt_club",
    "prestige",
];

/// The time-limited Le Mans 24 Hours (free race `Le_Mans` [V 0x62A5D0]).
pub fn is_le_mans_24(name: &str) -> bool {
    name.eq_ignore_ascii_case("le_mans")
}

/// Tracks whose every route was raced with the series cars (F401,
/// docs/SERIES.md "Layouts").
/// Further single routes the port has raced solo (two clean laps of a
/// Lacetti, F463 survey in docs/SERIES.md "More layouts"): the other
/// layouts of San Francisco, Detroit, Washington, Milan, Spa, Le Mans
/// (classic), Yokohama and Shibuya. Okutama (entity reader), Washington
/// routes 2/5/7 and the routes where the solo car wrecks (Detroit 0/2,
/// Washington 3/8, San Francisco 2) stay out. The career's head-to-head
/// layouts were raced with their event cars (docs/CAREER_LOCATIONS.md):
/// Washington route 0 (Viper; the Lacetti wrecks 1 km in) and Haruna
/// routes 4 (787B) and 9 (350Z).
pub const SURVEYED_ROUTES: [&str; 38] = [
    "tracks/usa/washington/route_0",
    "tracks/japan/haruna/route_4",
    "tracks/japan/haruna/route_9",
    "tracks/usa/san_fran/route_1",
    "tracks/usa/san_fran/route_4",
    "tracks/usa/san_fran/route_5",
    "tracks/usa/san_fran/route_6",
    "tracks/usa/detroit/route_1",
    "tracks/usa/detroit/route_3",
    "tracks/usa/detroit/route_4",
    "tracks/usa/detroit/route_5",
    "tracks/usa/detroit/route_6",
    "tracks/usa/detroit/route_7",
    "tracks/usa/washington/route_1",
    "tracks/usa/washington/route_4",
    "tracks/usa/washington/route_6",
    "tracks/usa/washington/route_9",
    "tracks/europe/milan/route_1",
    "tracks/europe/milan/route_2",
    "tracks/europe/milan/route_3",
    "tracks/europe/milan/route_4",
    "tracks/europe/milan/route_5",
    "tracks/europe/spa_francorchamps/route_1",
    "tracks/europe/spa_francorchamps/route_2",
    "tracks/europe/spa_francorchamps/route_3",
    "tracks/europe/le_mans/route_1",
    "tracks/japan/yokohama_docks/route_1",
    "tracks/japan/yokohama_docks/route_2",
    "tracks/japan/yokohama_docks/route_7",
    "tracks/japan/yokohama_docks/route_8",
    "tracks/japan/yokohama_docks/route_9",
    "tracks/japan/shibuya/route_1",
    "tracks/japan/shibuya/route_2",
    "tracks/japan/shibuya/route_3",
    "tracks/japan/shibuya/route_4",
    "tracks/japan/shibuya/route_5",
    "tracks/japan/shibuya/route_6",
    "tracks/japan/shibuya/route_7",
];

/// Whether series and drift events run on `route`: a `route::CIRCUITS`
/// route, a route of a [`SURVEYED_TRACKS`] track or a [`SURVEYED_ROUTES`]
/// route.
pub fn route_supported(route: &str) -> bool {
    crate::route::CIRCUITS.contains(&route)
        || SURVEYED_ROUTES.contains(&route)
        || SURVEYED_TRACKS
            .iter()
            .any(|t| route.starts_with(&format!("{t}/")))
}

pub const SURVEYED_TRACKS: [&str; 5] = [
    "tracks/europe/donington",
    "tracks/europe/nurburgring",
    "tracks/europe/istanbul_park",
    "tracks/usa/long_beach",
    "tracks/europe/jarama",
];

/// Cars of the circuit series that race fields on our circuits (AUTHORED
/// port list, docs/SERIES.md "Car survey"): Touring, Pro Tuned, GT1, GT2 and,
/// since the default setup's downforce limit (F414), the Le Mans Series
/// prototypes and the 911; since F457 the other prototypes (787, 390, je4),
/// the muscle cars except the Mustang Boss, the open-wheel cars, gt_club and
/// prestige. Another car needs GRID_SERIES_EXPERIMENTAL=1.
pub const RACEABLE: [&str; 35] = [
    "lac", "bmw", "sup", "sky", "nsx", "350", "dbr", "c6r", "sal", "ccg", "c5r", "lam", "spk", "esp",
    "911", "c65", "b05", "cre", "lc7", "b06", "r10", "787", "390", "je4", "ply", "cha", "gtr", "vip",
    "cam", "df3", "fmj", "tch", "ccx", "zon", "bos",
];

/// Whether a series' events still need GRID_SERIES_EXPERIMENTAL=1 (a car of
/// its fields outside [`RACEABLE`]). None since F463: the Mustang Boss's
/// zero-area collision faces keep the original's NaN planes.
pub fn is_experimental(series: &str) -> bool {
    let _ = series;
    false
}

fn table<'a>(
    db: &'a Database,
    name: &str,
) -> Result<&'a grid_formats::database::Table, Box<dyn std::error::Error>> {
    Ok(db
        .table(name)
        .ok_or(format!("database table {name} absent"))?)
}

impl Series {
    pub fn load(root: &std::path::Path, name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Self::from_database(&crate::read_database(root)?, name)
    }

    pub fn from_database(db: &Database, name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let discipline = table(db, "free_race_discipline")?
            .rows()
            .find(|r| {
                r.text("name_string_id")
                    .is_some_and(|n| n.eq_ignore_ascii_case(name))
            })
            .and_then(|r| r.int("id"))
            .ok_or(format!("no free-race discipline `{name}`"))?;
        let pools = table(db, "free_race_track_pool")?;
        let vehicles = table(db, "vehicle")?;
        let mut cars: Vec<Car> = Vec::new();
        let mut pool_name: Option<String> = None;
        for row in table(db, "free_race_restrictions")?
            .rows()
            .filter(|r| r.int("free_race_discipline_id") == Some(discipline))
        {
            let stem = vehicles
                .rows()
                .find(|v| v.int("id") == row.int("vehicle_id"))
                .and_then(|v| v.text("file_string"))
                .ok_or("restriction vehicle absent")?;
            let car = Car::new(stem)?;
            if !cars.contains(&car) {
                cars.push(car);
            }
            let pool = pools
                .rows()
                .find(|p| p.int("id") == row.int("free_race_track_pool_id"))
                .and_then(|p| p.text("name"))
                .ok_or("restriction track pool absent")?;
            match &pool_name {
                Some(p) if p != pool => return Err(format!("series {name} names two pools").into()),
                Some(_) => {}
                None => pool_name = Some(pool.into()),
            }
        }
        let pool_name = pool_name.ok_or(format!("series {name} has no restrictions"))?;
        let models = table(db, "track_model")?;
        let mut layouts = Vec::new();
        for p in pools
            .rows()
            .filter(|p| p.text("name") == Some(pool_name.as_str()))
        {
            let model = models
                .rows()
                .find(|m| m.int("id") == p.int("track_model_id"))
                .ok_or("pool track model absent")?;
            let text = |c: &str| model.text(c).ok_or(format!("track_model {c} absent"));
            layouts.push(Layout {
                id: p.int("id").ok_or("pool id absent")?,
                name: text("name_string_id")?.into(),
                route: format!(
                    "tracks/{}/{}/{}",
                    text("folder_string")?,
                    text("file_string")?,
                    text("route_string")?.to_ascii_lowercase()
                ),
                max_cars: p.int("max_cars").ok_or("max_cars absent")?.max(1) as usize,
                sort_order: p.int("sort_order").unwrap_or(0),
            });
        }
        layouts.sort_by_key(|l| (l.sort_order, l.id));
        Ok(Self {
            name: name.into(),
            cars,
            layouts,
        })
    }

    /// The pool's layouts this port has raced: every route of the five
    /// surveyed circuits ([`SURVEYED_TRACKS`], F401) and the exact
    /// `route::CIRCUITS` routes of later circuits (Yokohama and Le Mans
    /// route_0).
    pub fn supported_layouts(&self) -> impl Iterator<Item = &Layout> {
        self.layouts.iter().filter(|l| route_supported(&l.route))
    }

    /// The layout named `name` (`track_model.name_string_id`, e.g.
    /// `donington_nat`) or with route folder `name`.
    pub fn layout(&self, name: &str) -> Option<&Layout> {
        self.layouts
            .iter()
            .find(|l| l.name == name || l.route == name)
    }

    /// The series' cars this port can race ([`RACEABLE`]).
    pub fn raceable_cars(&self) -> Vec<Car> {
        self.cars
            .iter()
            .filter(|c| RACEABLE.contains(&c.stem()))
            .cloned()
            .collect()
    }
}

/// The original's random number generator: MT19937 with the old Knuth
/// seeding (0xA68440: `mt[0] = seed | 1`, `mt[i] = mt[i-1] * 69069`),
/// standard generation and tempering (0xA682A0, 0xA68380) [V, docs/SERIES.md].
#[derive(Clone)]
pub struct Mt {
    mt: [u32; 624],
    index: usize,
}

impl Mt {
    pub fn new(seed: u32) -> Self {
        let mut mt = [0; 624];
        mt[0] = seed | 1;
        for i in 1..624 {
            mt[i] = mt[i - 1].wrapping_mul(69069);
        }
        Self { mt, index: 624 }
    }

    pub fn next_u32(&mut self) -> u32 {
        if self.index >= 624 {
            for i in 0..624 {
                let y = (self.mt[i] & 0x8000_0000) | (self.mt[(i + 1) % 624] & 0x7fff_ffff);
                self.mt[i] =
                    self.mt[(i + 397) % 624] ^ (y >> 1) ^ if y & 1 != 0 { 0x9908_b0df } else { 0 };
            }
            self.index = 0;
        }
        let mut y = self.mt[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }

    /// 0xA683E0: `lo + genrand() % (hi - lo)`, upper bound exclusive.
    pub fn range(&mut self, lo: u32, hi: u32) -> u32 {
        lo + self.next_u32() % (hi - lo)
    }
}

/// One car of the field: model, `vehicle_livery.file_string` (e.g. `bmw_02`)
/// and that livery's package index (`livery_02`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub car: Car,
    pub livery: String,
    pub livery_index: Option<usize>,
    /// The entry's `driver.id`: a `free_race_drivers` row (0x664630) or the
    /// `lemans_restrictions` row's `driver_id` (0x663860); `None` for the
    /// player's own entry.
    pub driver: Option<i32>,
}

/// The free-race field of a pool (0x65FF70, 0x664630, 0x66D350 [V]):
/// `max_cars` entries in grid order and the index the player takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    pub entries: Vec<Entry>,
    pub player: usize,
}

/// Builds the free-race field of `series` on a pool with `max_cars` cars for
/// the player's `player` car (docs/SERIES.md "Grid fill").
///
/// - Vehicles: the `free_race_restrictions` rows of the pool, kept when the
///   vehicle counts towards achievements, has an AI livery, its statistics
///   name is not "banned" and, with `match_vehicle_class`, its class is the
///   player's. While more than four remain one is removed at random. The
///   player's vehicle replaces the first when its model is missing.
/// - Drivers: the 16 `free_race_drivers` in table order, reduced to
///   `max_cars` by removing index `rng(0, len) % len`; entry i gets driver i.
/// - Entry i drives vehicle `(i / k) % len` (k = max(max_cars / 4, 1)) in
///   that vehicle's AI livery `i % min(liveries, k)`.
/// - The player replaces one entry drawn at random.
///
/// The original seeds each generator from the clock; here the vehicle draws
/// use `Mt::new(seed)` and the field manager's `Mt::new(seed + 1)` (AUTHORED).
pub fn build_field(
    db: &Database,
    series: &str,
    max_cars: usize,
    player: &Car,
    seed: u32,
) -> Result<Field, Box<dyn std::error::Error>> {
    let discipline = table(db, "free_race_discipline")?
        .rows()
        .find(|r| r.text("name_string_id") == Some(series))
        .ok_or(format!("no free-race discipline `{series}`"))?;
    let match_class = discipline.int("match_vehicle_class").unwrap_or(0) != 0;
    let pools = table(db, "free_race_track_pool")?;
    let vehicles = table(db, "vehicle")?;
    let liveries = table(db, "vehicle_livery")?;
    let restrictions = table(db, "free_race_restrictions")?;
    let vehicle = |id: Option<i32>| vehicles.rows().find(|v| v.int("id") == id);
    let pool_name_of = |id: Option<i32>| {
        pools
            .rows()
            .find(|p| p.int("id") == id)
            .and_then(|p| p.text("name"))
    };
    // 0x62BE40: AI liveries in table order.
    let ai_liveries = |id: Option<i32>| -> Vec<String> {
        liveries
            .rows()
            .filter(|l| l.int("vehicle_id") == id && l.int("not_in_free_race") == Some(0))
            .filter_map(|l| l.text("file_string"))
            .filter(|f| !f.contains("_hero") && !f.contains("_user"))
            .map(str::to_string)
            .collect()
    };
    let pool_name = restrictions
        .rows()
        .find(|r| r.int("free_race_discipline_id") == discipline.int("id"))
        .and_then(|r| pool_name_of(r.int("free_race_track_pool_id")))
        .ok_or(format!("series {series} has no restrictions"))?;
    // The player's vehicle: its first row that counts towards achievements.
    let player_row = vehicles
        .rows()
        .filter(|v| v.text("file_string") == Some(player.stem()))
        .find(|v| v.int("count_towards_achievement") == Some(1))
        .ok_or(format!("no counting vehicle row for {player}"))?;
    let player_id = player_row.int("id");
    let mut list: Vec<Option<i32>> = Vec::new();
    for r in restrictions.rows() {
        if pool_name_of(r.int("free_race_track_pool_id")) != Some(pool_name) {
            continue;
        }
        let Some(v) = vehicle(r.int("vehicle_id")) else {
            continue;
        };
        let keep = v.int("count_towards_achievement") == Some(1)
            && !ai_liveries(v.int("id")).is_empty()
            && !v
                .text("ai_vehicle_type_statistics")
                .unwrap_or("")
                .contains("banned")
            && (!match_class || v.int("vehicle_class_id") == player_row.int("vehicle_class_id"));
        if keep {
            list.push(v.int("id"));
        }
    }
    if list.is_empty() {
        return Err(format!("series {series} has no free-race vehicle").into());
    }
    let mut vehicle_rng = Mt::new(seed);
    while list.len() > 4 {
        let n = list.len() as u32;
        list.remove((vehicle_rng.range(0, n) % n) as usize);
    }
    let model = |id: Option<i32>| vehicle(id).and_then(|v| v.int("vehicle_model_id"));
    if !list.iter().any(|&id| model(id) == model(player_id)) {
        list[0] = player_id;
    }
    let mut manager = Mt::new(seed.wrapping_add(1));
    let mut drivers: Vec<Option<i32>> = table(db, "free_race_drivers")?
        .rows()
        .map(|r| r.int("driver_id"))
        .collect();
    if drivers.len() < max_cars {
        return Err("fewer free-race drivers than cars".into());
    }
    while drivers.len() > max_cars {
        let n = drivers.len() as u32;
        drivers.remove((manager.range(0, n) % n) as usize);
    }
    let k = (max_cars >> 2).max(1);
    let mut entries = Vec::with_capacity(max_cars);
    for i in 0..max_cars {
        let id = list[(i / k) % list.len()];
        let names = ai_liveries(id);
        let livery = names[i % names.len().min(k)].clone();
        let stem = vehicle(id)
            .and_then(|v| v.text("file_string"))
            .ok_or("field vehicle absent")?;
        let livery_index = livery
            .strip_prefix(&format!("{stem}_"))
            .and_then(|n| n.parse().ok());
        entries.push(Entry {
            car: Car::new(stem)?,
            livery,
            livery_index,
            driver: drivers[i],
        });
    }
    let player = (manager.range(0, max_cars as u32) % max_cars as u32) as usize;
    Ok(Field { entries, player })
}

/// The Le Mans Series free-race field (0x676F50 → 0x663860 for the mode
/// `le_mans_series` [V, docs/SERIES.md]): the player's entry first, then one
/// AI car per `lemans_restrictions` row of the player's vehicle with
/// `lemans_24` = 0, in storage order, in the row's entrant livery; the row in
/// the player's own livery is skipped. No RNG, no `max_cars` cap (15–17 cars
/// for the shipped vehicles). `teammate_replace` only matters with a career
/// team-mate, which a free race never has. The player's livery is its
/// `<stem>_00` row, else the vehicle's first livery (AUTHORED: the original
/// takes the one chosen in the front end).
pub fn build_lemans_field(
    db: &Database,
    player: &Car,
) -> Result<Vec<Entry>, Box<dyn std::error::Error>> {
    build_lemans_field_for(db, player, false)
}

/// [`build_lemans_field`] for the rows with `lemans_24` = `lemans_24`: the
/// free race `Le_Mans` (the 24 Hours) asks for 1, `le_mans_series` for 0
/// [V 0x677009, 0x677040 → 0x663860, compare at 0x663A6F].
pub fn build_lemans_field_for(
    db: &Database,
    player: &Car,
    lemans_24: bool,
) -> Result<Vec<Entry>, Box<dyn std::error::Error>> {
    let vehicles = table(db, "vehicle")?;
    let liveries = table(db, "vehicle_livery")?;
    let player_row = vehicles
        .rows()
        .filter(|v| v.text("file_string") == Some(player.stem()))
        .find(|v| v.int("count_towards_achievement") == Some(1))
        .ok_or(format!("no counting vehicle row for {player}"))?;
    let vehicle_id = player_row.int("id");
    let own = liveries
        .rows()
        .filter(|l| l.int("vehicle_id") == vehicle_id)
        .find(|l| l.text("file_string") == Some(&format!("{}_00", player.stem())))
        .or_else(|| liveries.rows().find(|l| l.int("vehicle_id") == vehicle_id))
        .ok_or(format!("no livery for {player}"))?;
    let entry = |livery: grid_formats::database::Row<'_>, driver: Option<i32>| -> Result<Entry, Box<dyn std::error::Error>> {
        let file = livery.text("file_string").ok_or("livery file absent")?;
        let stem = vehicles
            .rows()
            .find(|v| v.int("id") == livery.int("vehicle_id"))
            .and_then(|v| v.text("file_string"))
            .ok_or("livery vehicle absent")?;
        Ok(Entry {
            car: Car::new(stem)?,
            livery: file.into(),
            livery_index: file
                .strip_prefix(&format!("{stem}_"))
                .and_then(|n| n.parse().ok()),
            driver,
        })
    };
    let mut field = vec![entry(own, None)?];
    for row in table(db, "lemans_restrictions")?.rows() {
        if row.int("player_vehicle_id") != vehicle_id
            || row.int("lemans_24") != Some(i32::from(lemans_24))
        {
            continue;
        }
        let livery_id = row.int("entrant_vehicle_livery_id");
        if livery_id == own.int("id") {
            continue;
        }
        let livery = liveries
            .rows()
            .find(|l| l.int("id") == livery_id)
            .ok_or("entrant livery absent")?;
        field.push(entry(livery, row.int("driver_id"))?);
    }
    if field.len() < 2 {
        return Err(format!("no Le Mans Series field for {player}").into());
    }
    Ok(field)
}

/// The language key of a car's class name, `db_<vehicle_class.type_string_id>`
/// (e.g. `db_lmp1` "LMP1", `db_gt2` "GT2") [D].
pub fn class_key(db: &Database, car: &Car) -> Result<String, Box<dyn std::error::Error>> {
    let row = table(db, "vehicle")?
        .rows()
        .find(|v| v.text("file_string") == Some(car.stem()))
        .ok_or(format!("no vehicle row for {car}"))?;
    table(db, "vehicle_class")?
        .rows()
        .find(|c| c.int("id") == row.int("vehicle_class_id"))
        .and_then(|c| c.text("type_string_id"))
        .map(|t| format!("db_{t}"))
        .ok_or_else(|| format!("no vehicle class for {car}").into())
}

/// `vehicle_class.lemans_sort_order` of a car (LMP1 1, LMP2 2, GT1 3, GT2 4).
pub fn lemans_sort_order(db: &Database, car: &Car) -> Result<i32, Box<dyn std::error::Error>> {
    let row = table(db, "vehicle")?
        .rows()
        .find(|v| v.text("file_string") == Some(car.stem()))
        .ok_or(format!("no vehicle row for {car}"))?;
    table(db, "vehicle_class")?
        .rows()
        .find(|c| c.int("id") == row.int("vehicle_class_id"))
        .and_then(|c| c.int("lemans_sort_order"))
        .ok_or_else(|| format!("no vehicle class for {car}").into())
}

/// A car's `vehicle_class` row as the Le Mans results use it.
#[derive(Clone, Debug, PartialEq)]
pub struct LeMansClass {
    /// `file_string`, e.g. `lmp1`, `gt2` (the results' class cell
    /// `le_mans_%s` [V 0x53FE10]).
    pub name: String,
    /// `lemans_sort_order` (LMP1 1, LMP2 2, GT1 3, GT2 4, others 0).
    pub sort_order: i32,
    /// `lemans_progress_adjustment` (LMP1 1.08, GT1 0.92, GT2 0.84, else 1).
    pub progress_adjustment: f64,
}

pub fn lemans_class(db: &Database, car: &Car) -> Result<LeMansClass, Box<dyn std::error::Error>> {
    let row = table(db, "vehicle")?
        .rows()
        .find(|v| v.text("file_string") == Some(car.stem()))
        .ok_or(format!("no vehicle row for {car}"))?;
    let class = table(db, "vehicle_class")?
        .rows()
        .find(|c| c.int("id") == row.int("vehicle_class_id"))
        .ok_or(format!("no vehicle class for {car}"))?;
    Ok(LeMansClass {
        name: class.text("file_string").unwrap_or_default().into(),
        sort_order: class.int("lemans_sort_order").unwrap_or(0),
        progress_adjustment: class
            .float("lemans_progress_adjustment")
            .map_or(1., f64::from),
    })
}

/// The Le Mans Series grid (0x5BDF10 [V]): car indices in slot order.
/// `keys[i]` is car i's class sort key; car 0 is the player. Each slot i
/// swaps with slot `genrand() % n` (one draw per car, a naive shuffle), then
/// a stable bubble sort by key (strict >), then the player moves to the
/// front (option 0), back (1) or middle (2) of its class group, or stays (3).
pub fn lemans_grid(keys: &[i32], rng: &mut Mt, option: u8) -> Vec<usize> {
    let n = keys.len();
    let mut order: Vec<usize> = (0..n).collect();
    for i in 0..n {
        let j = (rng.range(0, n as u32) % n as u32) as usize;
        if j != i {
            order.swap(i, j);
        }
    }
    for p in 0..n {
        for k in 0..n.saturating_sub(1 + p) {
            if keys[order[k]] > keys[order[k + 1]] {
                order.swap(k, k + 1);
            }
        }
    }
    let less = keys.iter().filter(|&&k| k < keys[0]).count();
    let equal = keys.iter().filter(|&&k| k == keys[0]).count();
    let target = match option {
        1 => Some(less + equal - 1),
        2 => Some(less + equal / 2),
        3 => None,
        _ => Some(less),
    };
    if let Some(target) = target {
        let at = order.iter().position(|&c| c == 0).unwrap();
        order.swap(at, target);
    }
    order
}

/// A prepared series race: layout, player car and the opponents in grid
/// order (opponent k = the k-th car after the player).
#[derive(Clone, Debug)]
pub struct Event {
    pub series: String,
    pub layout: Layout,
    pub player: Car,
    pub opponent_count: usize,
    /// Empty when the caller keeps its own opponent cars.
    pub opponents: Vec<Entry>,
    /// The player's 0-based grid slot when the series places it (Le Mans
    /// Series, 0x5BDF10); `None` keeps the caller's grid position.
    pub player_slot: Option<usize>,
    /// Le Mans Series: each car's class name key ([`class_key`]), the player
    /// first, then the opponents in grid order; empty otherwise.
    pub classes: Vec<String>,
    pub seed: u32,
}

impl Event {
    /// `route` must be a layout of the series' pool; `player` must be one of
    /// its cars (default: its first car). `opponents` is capped at the
    /// layout's `max_cars` − 1 (default: a full grid, as the original). With
    /// `fill` the opponents are the original's free-race field without the
    /// entry the player replaced; with fewer opponents the first ones are
    /// kept (AUTHORED).
    pub fn prepare(
        root: &std::path::Path,
        name: &str,
        route: &crate::route::Route,
        player: Option<&Car>,
        opponents: Option<usize>,
        fill: bool,
        seed: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::prepare_on_grid(root, name, route, player, opponents, fill, seed, 0)
    }

    /// [`Self::prepare`] with the player's grid option of a Le Mans Series
    /// event (0 front, 1 back, 2 middle of its class group, 3 where the
    /// shuffle put it).
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_on_grid(
        root: &std::path::Path,
        name: &str,
        route: &crate::route::Route,
        player: Option<&Car>,
        opponents: Option<usize>,
        fill: bool,
        seed: u32,
        grid_option: u8,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let db = crate::read_database(root)?;
        let series = Series::from_database(&db, name)?;
        let layout = series
            .layout(route.path())
            .ok_or(format!("{} is not in the {name} track pool", route.path()))?
            .clone();
        let player = match player {
            Some(car) if series.cars.contains(car) => car.clone(),
            Some(car) => return Err(format!("{car} is not a {name} car").into()),
            None => series.cars[0].clone(),
        };
        if name == "le_mans_series" || is_le_mans_24(name) {
            // The whole recovered field and grid (0x663860, 0x5BDF10); the
            // pool's max_cars and the caller's opponent count do not apply.
            let field = build_lemans_field_for(&db, &player, is_le_mans_24(name))?;
            let keys = field
                .iter()
                .map(|e| lemans_sort_order(&db, &e.car))
                .collect::<Result<Vec<_>, _>>()?;
            // AUTHORED seeding: the original seeds the grid generator from
            // the clock (session +0x241C).
            let order = lemans_grid(&keys, &mut Mt::new(seed.wrapping_add(2)), grid_option);
            let player_slot = order.iter().position(|&c| c == 0);
            let opponents: Vec<Entry> = order
                .iter()
                .filter(|&&c| c != 0)
                .map(|&c| field[c].clone())
                .collect();
            Self::check_raceable(&player, &opponents)?;
            let classes = std::iter::once(&player)
                .chain(opponents.iter().map(|e| &e.car))
                .map(|c| class_key(&db, c))
                .collect::<Result<Vec<_>, _>>()?;
            return Ok(Self {
                series: name.into(),
                layout,
                player,
                opponent_count: opponents.len(),
                opponents,
                player_slot,
                classes,
                seed,
            });
        }
        let opponent_count = opponents
            .unwrap_or(layout.max_cars - 1)
            .min(layout.max_cars - 1);
        let opponents = if fill {
            let field = build_field(&db, name, layout.max_cars, &player, seed)?;
            let mut rest: Vec<Entry> = field
                .entries
                .into_iter()
                .enumerate()
                .filter(|&(i, _)| i != field.player)
                .map(|(_, e)| e)
                .collect();
            rest.truncate(opponent_count);
            rest
        } else {
            Vec::new()
        };
        Self::check_raceable(&player, &opponents)?;
        Ok(Self {
            series: name.into(),
            layout,
            player,
            opponent_count,
            opponents,
            player_slot: None,
            classes: Vec::new(),
            seed,
        })
    }

    fn check_raceable(player: &Car, opponents: &[Entry]) -> Result<(), Box<dyn std::error::Error>> {
        let experimental = std::env::var_os("GRID_SERIES_EXPERIMENTAL").is_some();
        for car in std::iter::once(player).chain(opponents.iter().map(|e| &e.car)) {
            if !experimental && !RACEABLE.contains(&car.stem()) {
                return Err(format!(
                    "{car} cannot race yet (docs/SERIES.md); set GRID_SERIES_EXPERIMENTAL=1 to try"
                )
                .into());
            }
        }
        Ok(())
    }

    pub fn cars(&self) -> Vec<Car> {
        self.opponents.iter().map(|e| e.car.clone()).collect()
    }

    pub fn log_line(&self) -> String {
        let field: Vec<&str> = self.opponents.iter().map(|e| e.livery.as_str()).collect();
        format!(
            "drive_series series={} layout={} pool_id={} max_cars={} player={} opponents={} field={} player_slot={} seed={} fill={} seeding=authored",
            self.series,
            self.layout.name,
            self.layout.id,
            self.layout.max_cars,
            self.player,
            self.opponent_count,
            field.join(","),
            self.player_slot.map_or("caller".to_string(), |s| s.to_string()),
            self.seed,
            if self.player_slot.is_some() {
                "original-le-mans-series"
            } else {
                "original-free-race"
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn series_cars_and_pools_come_from_the_database() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let stems = |s: &Series| {
            s.cars
                .iter()
                .map(|c| c.stem().to_string())
                .collect::<Vec<_>>()
        };
        let pro = Series::load(&root, "pro_tuned").unwrap();
        assert_eq!(stems(&pro), ["sup", "sky", "nsx", "350"]);
        assert_eq!(pro.layouts.len(), 76);
        let supported: Vec<&str> = pro.supported_layouts().map(|l| l.name.as_str()).collect();
        // 20 layouts of the surveyed circuits and the route_0 circuits of
        // Yokohama (yoko_v1), Spa (spa_gp), Milan (milan_gp), Shibuya
        // (shib_drift_cir), San Francisco (san_francisco_gp) and Circuit de la
        // Sarthe (le_mans_circuit), plus the surveyed routes (F463) except
        // the two Haruna roads, which are in no circuit pool.
        assert_eq!(supported.len(), 26 + SURVEYED_ROUTES.len() - 2);
        for name in [
            "yoko_v1",
            "spa_gp",
            "milan_gp",
            "shib_drift_cir",
            "san_francisco_gp",
            "le_mans_circuit",
        ] {
            assert!(supported.contains(&name), "{name}");
        }
        let nat = pro.layout("donington_nat").unwrap();
        assert_eq!(
            (nat.route.as_str(), nat.max_cars),
            ("tracks/europe/donington/route_1", 12)
        );
        assert_eq!(
            pro.layout("tracks/usa/long_beach/route_5").unwrap().name,
            "longbeach_sp_r"
        );
        let touring = Series::load(&root, "touring").unwrap();
        assert_eq!(stems(&touring), ["bmw", "lac"]);
        let gt1 = Series::load(&root, "gt1").unwrap();
        assert_eq!(stems(&gt1), ["dbr", "c6r", "sal", "ccg", "c5r", "lam"]);
        assert_eq!(gt1.raceable_cars().len(), 6);
        let gt2 = Series::load(&root, "gt2").unwrap();
        assert_eq!(stems(&gt2), ["spk", "esp"]);
        assert_eq!(gt2.raceable_cars().len(), 2);
        let lms = Series::load(&root, "le_mans_series").unwrap();
        assert_eq!(lms.cars.len(), 13);
        for name in CIRCUIT_SERIES {
            let s = Series::load(&root, name).unwrap();
            for l in s.supported_layouts() {
                assert!(
                    root.join(&l.route).join("ai_track.xml").is_file(),
                    "{}",
                    l.route
                );
            }
        }
        assert!(Series::load(&root, "nonexistent").is_err());
    }

    #[test]
    fn mt19937_matches_the_reference_generator() {
        // Knuth seeding: the 1998 reference's sgenrand(4357) state.
        let mut mt = Mt::new(4357);
        let first: Vec<u32> = (0..3).map(|_| mt.next_u32()).collect();
        assert_eq!(first, [3510405877, 4290933890, 2191955339]);
        let mut a = Mt::new(4356);
        let mut b = Mt::new(4357);
        assert_eq!(a.next_u32(), b.next_u32(), "seed | 1");
        for _ in 0..2000 {
            assert!((3..7).contains(&mt.range(3, 7)));
        }
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn free_race_fields_follow_the_original_blocks() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let schema = std::fs::read(root.join("database/schema.bin")).unwrap();
        let data = std::fs::read(root.join("database/database.bin")).unwrap();
        let db = grid_formats::database::read(&schema, &data).unwrap();
        let liveries = |f: &Field| {
            f.entries
                .iter()
                .map(|e| e.livery.clone())
                .collect::<Vec<_>>()
        };
        // Touring: BMW and Lacetti in blocks of three, liveries 00/02/03.
        let touring = build_field(&db, "touring", 12, &Car::default(), 7).unwrap();
        assert_eq!(
            liveries(&touring),
            [
                "bmw_00", "bmw_02", "bmw_03", "lac_00", "lac_01", "lac_02", "bmw_00", "bmw_02",
                "bmw_03", "lac_00", "lac_01", "lac_02"
            ]
        );
        assert!(touring.player < 12);
        assert_eq!(touring.entries[4].livery_index, Some(1));
        // Twelve different free-race drivers, in table order after the removals.
        let rows: Vec<i32> = db
            .tables
            .iter()
            .find(|t| t.name == "free_race_drivers")
            .unwrap()
            .rows()
            .filter_map(|r| r.int("driver_id"))
            .collect();
        let drivers: Vec<i32> = touring.entries.iter().map(|e| e.driver.unwrap()).collect();
        let positions: Vec<usize> = drivers
            .iter()
            .map(|d| rows.iter().position(|r| r == d).unwrap())
            .collect();
        assert!(positions.windows(2).all(|w| w[0] < w[1]), "{positions:?}");
        assert_eq!((rows.len(), drivers.len()), (16, 12));
        // Pro Tuned: four models, so no vehicle draw; three cars each.
        let pro = build_field(&db, "pro_tuned", 12, &Car::new("sky").unwrap(), 1).unwrap();
        let stems: Vec<&str> = pro.entries.iter().map(|e| e.car.stem()).collect();
        assert_eq!(
            stems,
            [
                "sup", "sup", "sup", "sky", "sky", "sky", "nsx", "nsx", "nsx", "350", "350", "350"
            ]
        );
        // GT1: six eligible vehicles reduced to four by the seed.
        for seed in 0..20 {
            let gt1 = build_field(&db, "gt1", 12, &Car::new("c5r").unwrap(), seed).unwrap();
            let mut models: Vec<&str> = gt1.entries.iter().map(|e| e.car.stem()).collect();
            assert!(models.contains(&"c5r"), "seed {seed}");
            models.dedup();
            assert_eq!(models.len(), 4, "seed {seed}");
        }
        // The player index is drawn from the seed.
        let players: std::collections::BTreeSet<usize> = (0..40)
            .map(|s| {
                build_field(&db, "touring", 12, &Car::default(), s)
                    .unwrap()
                    .player
            })
            .collect();
        assert!(players.len() > 5);
    }

    #[test]
    fn lemans_grid_sorts_classes_and_places_the_player_in_its_group() {
        // Player (car 0) is GT1 (3) among LMP1 (1), LMP2 (2) and GT2 (4).
        let keys = [3, 1, 2, 3, 4, 1, 3, 2];
        for option in 0..4u8 {
            let order = lemans_grid(&keys, &mut Mt::new(7), option);
            let sorted: Vec<i32> = order.iter().map(|&c| keys[c]).collect();
            assert!(sorted.windows(2).all(|w| w[0] <= w[1]), "{sorted:?}");
            let slot = order.iter().position(|&c| c == 0).unwrap();
            match option {
                0 => assert_eq!(slot, 4),
                1 => assert_eq!(slot, 6),
                2 => assert_eq!(slot, 5),
                _ => assert!((4..=6).contains(&slot)),
            }
        }
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn lemans_series_fields_come_from_lemans_restrictions() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let schema = std::fs::read(root.join("database/schema.bin")).unwrap();
        let data = std::fs::read(root.join("database/database.bin")).unwrap();
        let db = grid_formats::database::read(&schema, &data).unwrap();
        // DBR9: 15 rows for vehicle 29 with lemans_24 = 0, one in dbr_00 (the
        // player takes it).
        let dbr = build_lemans_field(&db, &Car::new("dbr").unwrap()).unwrap();
        assert_eq!(dbr[0].livery, "dbr_00");
        assert_eq!(dbr.len(), 15);
        let count = |f: &[Entry], s: &str| f.iter().filter(|e| e.car.stem() == s).count();
        assert_eq!(count(&dbr, "lc7"), 3);
        assert_eq!(count(&dbr, "c65"), 4);
        // Every AI entry names its row's driver; the player's has none.
        assert_eq!(dbr[0].driver, None);
        assert!(dbr[1..].iter().all(|e| e.driver.is_some_and(|d| d > 0)));
        // Spyker: 16 rows, one in spk_00.
        let spk = build_lemans_field(&db, &Car::new("spk").unwrap()).unwrap();
        assert_eq!(spk.len(), 16);
        assert_eq!(lemans_sort_order(&db, &Car::new("b06").unwrap()).unwrap(), 1);
        assert_eq!(lemans_sort_order(&db, &Car::new("c65").unwrap()).unwrap(), 2);
        assert_eq!(lemans_sort_order(&db, &Car::new("dbr").unwrap()).unwrap(), 3);
        assert_eq!(lemans_sort_order(&db, &Car::new("esp").unwrap()).unwrap(), 4);
        assert_eq!(class_key(&db, &Car::new("lc7").unwrap()).unwrap(), "db_lmp1");
        assert_eq!(class_key(&db, &Car::new("911").unwrap()).unwrap(), "db_gt2");
    }

    /// The free race `Le_Mans` (the 24 Hours): its cars, its one layout and
    /// the `lemans_24` = 1 fields.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn le_mans_24_fields_come_from_the_24_hour_rows() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let series = Series::load(&root, "le_mans").unwrap();
        let routes: Vec<&str> = series.layouts.iter().map(|l| l.route.as_str()).collect();
        assert_eq!(routes, [crate::route::LE_MANS]);
        assert_eq!(series.cars.len(), 14);
        let schema = std::fs::read(root.join("database/schema.bin")).unwrap();
        let data = std::fs::read(root.join("database/database.bin")).unwrap();
        let db = grid_formats::database::read(&schema, &data).unwrap();
        // DBR9: 16 cars, the 24 Hours entrants (two Audi R10 LMP1s, four
        // Courage C65 LMP2s) instead of the Le Mans Series field.
        let dbr = build_lemans_field_for(&db, &Car::new("dbr").unwrap(), true).unwrap();
        assert_eq!(dbr.len(), 16);
        assert_eq!(dbr[0].livery, "dbr_00");
        let count = |f: &[Entry], s: &str| f.iter().filter(|e| e.car.stem() == s).count();
        assert_eq!(count(&dbr, "r10"), 2);
        assert_eq!(count(&dbr, "c65"), 4);
        assert_eq!(count(&dbr, "lc7"), 0);
        let spk = build_lemans_field_for(&db, &Car::new("spk").unwrap(), true).unwrap();
        assert_eq!(spk.len(), 17);
        assert!(is_le_mans_24("Le_Mans") && !is_le_mans_24("le_mans_series"));
        let class = |stem: &str| lemans_class(&db, &Car::new(stem).unwrap()).unwrap();
        assert_eq!(class("r10").name, "lmp1");
        assert!((class("r10").progress_adjustment - 1.08).abs() < 1e-6);
        assert_eq!((class("c65").name.as_str(), class("c65").sort_order), ("lmp2", 2));
        assert!((class("esp").progress_adjustment - 0.84).abs() < 1e-6);
        assert_eq!(class("lac").sort_order, 0);
    }
}
