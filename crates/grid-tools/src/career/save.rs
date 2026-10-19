//! The port's own career save (AUTHORED, docs/CAREER_RESEARCH.md §9.4): a
//! versioned UTF-8 text document, one `key value…` line per field, ended by
//! an FNV-1a 64 checksum of the lines before it. The original's encrypted
//! container (docs §9) is not reused.
//!
//! Database rows are referred to by their database ids, as the original
//! does.
use super::state::{Career, HiredTeamMate, MINOR_SLOTS, OwnedCar, SeasonEntry};

pub const MAGIC: &str = "grid-career";
pub const VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "career save: {}", self.0)
    }
}

impl std::error::Error for Error {}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn entry_text(e: &SeasonEntry) -> String {
    match e {
        SeasonEntry::Event(id) => format!("event {id}"),
        SeasonEntry::DriverOffer(id) => format!("offer {id}"),
    }
}

/// The save document of a career.
pub fn write(c: &Career) -> Result<String, Error> {
    if c.team_name.contains(['\n', '\r']) {
        return Err(Error("team name has a line break".into()));
    }
    let mut lines = vec![format!("{MAGIC} {VERSION}")];
    let mut put = |s: String| lines.push(s);
    put(format!("money {}", c.money));
    put(format!(
        "reputation {}",
        c.reputation.map(|r| r.to_string()).join(" ")
    ));
    put(format!("season_number {}", c.season_number));
    put(format!("team_founded {}", c.team_founded as u8));
    put(format!("team_name {}", c.team_name));
    for car in &c.cars {
        put(format!("car {} {}", car.vehicle_id, car.livery));
    }
    if let Some(t) = &c.team_mate {
        put(format!("team_mate {} {}", t.driver_id, t.winnings_percentage));
    }
    put(format!("team_mate_spending {}", c.team_mate_spending));
    for o in &c.sponsor_offers {
        put(format!("sponsor_offer {o}"));
    }
    if let Some(o) = c.major_sponsor {
        put(format!("major_sponsor {o}"));
    }
    for (i, o) in c.minor_sponsors.iter().enumerate() {
        if let Some(o) = o {
            put(format!("minor_sponsor {i} {o}"));
        }
    }
    for (e, pot) in &c.reputation_pots {
        put(format!("reputation_pot {e} {pot}"));
    }
    for o in &c.completed_offers {
        put(format!("completed_offer {o}"));
    }
    for (e, pos) in &c.event_results {
        put(format!("event_result {e} {pos}"));
    }
    for e in &c.season {
        put(format!("season {}", entry_text(e)));
    }
    for e in &c.previous_season {
        put(format!("previous_season {}", entry_text(e)));
    }
    put(format!("lemans_refused {}", c.lemans_refused as u8));
    for e in &c.head_to_heads {
        put(format!("head_to_head {e}"));
    }
    put(format!("game_completed {}", c.game_completed as u8));
    for (layout, n) in &c.layout_races {
        put(format!("layout_races {layout} {n}"));
    }
    put(format!("races_won {}", c.races_won));
    put(format!("races_lost {}", c.races_lost));
    let mut body = lines.join("\n");
    body.push('\n');
    let sum = fnv1a(body.as_bytes());
    body.push_str(&format!("checksum {sum:016x}\n"));
    Ok(body)
}

fn num<T: std::str::FromStr>(s: Option<&str>, key: &str) -> Result<T, Error> {
    s.and_then(|v| v.parse().ok())
        .ok_or_else(|| Error(format!("{key}: bad or missing number")))
}

fn flag(s: Option<&str>, key: &str) -> Result<bool, Error> {
    match s {
        Some("0") => Ok(false),
        Some("1") => Ok(true),
        _ => Err(Error(format!("{key}: expected 0 or 1"))),
    }
}

fn season_entry(kind: Option<&str>, id: Option<&str>, key: &str) -> Result<SeasonEntry, Error> {
    let id = num(id, key)?;
    match kind {
        Some("event") => Ok(SeasonEntry::Event(id)),
        Some("offer") => Ok(SeasonEntry::DriverOffer(id)),
        _ => Err(Error(format!("{key}: expected event or offer"))),
    }
}

/// Reads a save document. The checksum and version must match; unknown keys
/// are an error so that a newer save is not silently truncated.
pub fn read(text: &str) -> Result<Career, Error> {
    let body_end = text
        .rfind("checksum ")
        .ok_or_else(|| Error("no checksum".into()))?;
    let (body, tail) = text.split_at(body_end);
    let stored = u64::from_str_radix(tail["checksum ".len()..].trim(), 16)
        .map_err(|_| Error("bad checksum".into()))?;
    if fnv1a(body.as_bytes()) != stored {
        return Err(Error("checksum mismatch".into()));
    }
    let mut lines = body.lines();
    let mut head = lines.next().unwrap_or("").split(' ');
    if head.next() != Some(MAGIC) {
        return Err(Error("not a career save".into()));
    }
    let version: u32 = num(head.next(), "version")?;
    if version != VERSION {
        return Err(Error(format!("unsupported version {version}")));
    }
    let mut c = Career::new();
    for line in lines {
        let (key, rest) = line.split_once(' ').unwrap_or((line, ""));
        let mut f = rest.split(' ');
        match key {
            "money" => c.money = num(f.next(), key)?,
            "reputation" => {
                for r in &mut c.reputation {
                    *r = num(f.next(), key)?;
                }
            }
            "season_number" => c.season_number = num(f.next(), key)?,
            "team_founded" => c.team_founded = flag(f.next(), key)?,
            "team_name" => c.team_name = rest.to_owned(),
            "car" => c.cars.push(OwnedCar {
                vehicle_id: num(f.next(), key)?,
                livery: num(f.next(), key)?,
            }),
            "team_mate" => {
                c.team_mate = Some(HiredTeamMate {
                    driver_id: num(f.next(), key)?,
                    winnings_percentage: num(f.next(), key)?,
                })
            }
            "team_mate_spending" => c.team_mate_spending = num(f.next(), key)?,
            "sponsor_offer" => c.sponsor_offers.push(num(f.next(), key)?),
            "major_sponsor" => c.major_sponsor = Some(num(f.next(), key)?),
            "minor_sponsor" => {
                let slot: usize = num(f.next(), key)?;
                if slot >= MINOR_SLOTS {
                    return Err(Error(format!("minor sponsor slot {slot}")));
                }
                c.minor_sponsors[slot] = Some(num(f.next(), key)?);
            }
            "reputation_pot" => {
                let e = num(f.next(), key)?;
                c.reputation_pots.insert(e, num(f.next(), key)?);
            }
            "completed_offer" => {
                c.completed_offers.insert(num(f.next(), key)?);
            }
            "event_result" => {
                let e = num(f.next(), key)?;
                c.event_results.insert(e, num(f.next(), key)?);
            }
            "season" => c.season.push(season_entry(f.next(), f.next(), key)?),
            "previous_season" => c
                .previous_season
                .push(season_entry(f.next(), f.next(), key)?),
            "lemans_refused" => c.lemans_refused = flag(f.next(), key)?,
            "head_to_head" => {
                c.head_to_heads.insert(num(f.next(), key)?);
            }
            "game_completed" => c.game_completed = flag(f.next(), key)?,
            "layout_races" => {
                let layout = num(f.next(), key)?;
                c.layout_races.insert(layout, num(f.next(), key)?);
            }
            "races_won" => c.races_won = num(f.next(), key)?,
            "races_lost" => c.races_lost = num(f.next(), key)?,
            "" => {}
            other => return Err(Error(format!("unknown key {other}"))),
        }
    }
    Ok(c)
}

/// Writes a save atomically: a temporary file next to `path`, then rename.
pub fn store(path: &std::path::Path, c: &Career) -> Result<(), Box<dyn std::error::Error>> {
    let text = write(c)?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

pub fn load(path: &std::path::Path) -> Result<Career, Box<dyn std::error::Error>> {
    Ok(read(&std::fs::read_to_string(path)?)?)
}
