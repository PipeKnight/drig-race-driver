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

/// The circuit-race series this port offers.
pub const CIRCUIT_SERIES: [&str; 5] = ["pro_tuned", "touring", "gt1", "gt2", "le_mans_series"];

/// Tracks whose every route was raced with the series cars (F401,
/// docs/SERIES.md "Layouts").
pub const SURVEYED_TRACKS: [&str; 5] = [
    "tracks/europe/donington",
    "tracks/europe/nurburgring",
    "tracks/europe/istanbul_park",
    "tracks/usa/long_beach",
    "tracks/europe/jarama",
];

/// Cars of the circuit series that race 12-car fields on our circuits
/// (AUTHORED port limitation, docs/SERIES.md "Car survey"). The GT1 and GT2
/// cars race since the collision floor lift (F406); the LMP1 prototypes
/// (lc7, b06) still ground at speed and the Le Mans Series field builder is
/// separate, so a series event that draws another car needs
/// GRID_SERIES_EXPERIMENTAL=1.
pub const RACEABLE: [&str; 14] = [
    "lac", "bmw", "sup", "sky", "nsx", "350", "dbr", "c6r", "sal", "ccg", "c5r", "lam", "spk", "esp",
];

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
        let schema = std::fs::read(root.join("database/schema.bin"))?;
        let data = std::fs::read(root.join("database/database.bin"))?;
        Self::from_database(&grid_formats::database::read(&schema, &data)?, name)
    }

    pub fn from_database(db: &Database, name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let discipline = table(db, "free_race_discipline")?
            .rows()
            .find(|r| r.text("name_string_id") == Some(name))
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
    /// `route::CIRCUITS` routes of later circuits (Yokohama route_0).
    pub fn supported_layouts(&self) -> impl Iterator<Item = &Layout> {
        self.layouts.iter().filter(|l| {
            crate::route::CIRCUITS.contains(&l.route.as_str())
                || SURVEYED_TRACKS
                    .iter()
                    .any(|t| l.route.starts_with(&format!("{t}/")))
        })
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
/// - Drivers: the 16 `free_race_drivers`, reduced to `max_cars` at random.
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
    let mut drivers = table(db, "free_race_drivers")?.rows.len();
    if drivers < max_cars {
        return Err("fewer free-race drivers than cars".into());
    }
    while drivers > max_cars {
        let n = drivers as u32;
        let _ = manager.range(0, n) % n;
        drivers -= 1;
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
        });
    }
    let player = (manager.range(0, max_cars as u32) % max_cars as u32) as usize;
    Ok(Field { entries, player })
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
        let schema = std::fs::read(root.join("database/schema.bin"))?;
        let data = std::fs::read(root.join("database/database.bin"))?;
        let db = grid_formats::database::read(&schema, &data)?;
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
        let experimental = std::env::var_os("GRID_SERIES_EXPERIMENTAL").is_some();
        for car in std::iter::once(&player).chain(opponents.iter().map(|e| &e.car)) {
            if !experimental && !RACEABLE.contains(&car.stem()) {
                return Err(format!(
                    "{car} cannot race yet (docs/SERIES.md); set GRID_SERIES_EXPERIMENTAL=1 to try"
                )
                .into());
            }
        }
        Ok(Self {
            series: name.into(),
            layout,
            player,
            opponent_count,
            opponents,
            seed,
        })
    }

    pub fn cars(&self) -> Vec<Car> {
        self.opponents.iter().map(|e| e.car.clone()).collect()
    }

    pub fn log_line(&self) -> String {
        let field: Vec<&str> = self.opponents.iter().map(|e| e.livery.as_str()).collect();
        format!(
            "drive_series series={} layout={} pool_id={} max_cars={} player={} opponents={} field={} seed={} fill=original-free-race seeding=authored",
            self.series,
            self.layout.name,
            self.layout.id,
            self.layout.max_cars,
            self.player,
            self.opponent_count,
            field.join(","),
            self.seed
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
        // 20 layouts of the surveyed circuits and Yokohama route_0 (yoko_v1).
        assert_eq!(supported.len(), 21);
        assert!(supported.contains(&"yoko_v1"));
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
}
