//! Private, versioned personal bests for the independently authored trial rules.
pub use grid_world::lap_timing::Completed;
pub use grid_world::time_trial::{Phase, Results, State};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const HEADER: &str = "GRID_TRIAL_BESTS\t1\tlac\tdonington-route0\tassisted-auto\trules1\n";
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Mode {
    Wheels,
    Body,
    Events,
    Deformation,
}
impl Mode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Wheels => "wheels",
            Self::Body => "body",
            Self::Events => "events",
            Self::Deformation => "deformation",
        }
    }
    fn parse(s: &str) -> Result<Self> {
        match s {
            "wheels" => Ok(Self::Wheels),
            "body" => Ok(Self::Body),
            "events" => Ok(Self::Events),
            "deformation" => Ok(Self::Deformation),
            _ => Err("invalid trial mode".into()),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    pub laps: u32,
    pub mode: Mode,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Best {
    pub lap_ns: u64,
    pub event_ns: u64,
    pub build: String,
}
pub struct Records {
    path: PathBuf,
    rows: BTreeMap<Key, Best>,
}
fn valid_build(build: &str) -> bool {
    !build.is_empty()
        && build.len() <= 80
        && build
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}
fn ns(seconds: f64) -> Result<u64> {
    if !seconds.is_finite() || seconds <= 0. || seconds > 86400. {
        return Err("invalid trial record duration".into());
    }
    let value = (seconds * 1e9).round() as u64;
    if value == 0 {
        return Err("trial duration rounds to zero".into());
    }
    Ok(value)
}
fn read(path: &Path) -> Result<BTreeMap<Key, Best>> {
    let mut rows = BTreeMap::new();
    let mut bytes = Vec::new();
    match fs::File::open(path) {
        Ok(f) => {
            f.take(65537).read_to_end(&mut bytes)?;
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(rows),
        Err(e) => return Err(e.into()),
    }
    if bytes.len() > 65536 {
        return Err("trial records exceed bound".into());
    }
    let text = std::str::from_utf8(&bytes)?;
    let rest = text.strip_prefix(HEADER).ok_or("trial record header")?;
    if !rest.is_empty() && !rest.ends_with('\n') {
        return Err("truncated trial record".into());
    }
    for line in rest.lines() {
        if rows.len() >= 396 {
            return Err("too many trial records".into());
        }
        let fields: Vec<_> = line.split('\t').collect();
        if fields.len() != 5 {
            return Err("trial record fields".into());
        }
        let key = Key {
            laps: fields[0].parse()?,
            mode: Mode::parse(fields[1])?,
        };
        let best = Best {
            lap_ns: fields[2].parse()?,
            event_ns: fields[3].parse()?,
            build: fields[4].into(),
        };
        if !(1..=99).contains(&key.laps)
            || best.lap_ns == 0
            || best.event_ns < best.lap_ns
            || best.event_ns > 86_400_000_000_000
            || !valid_build(&best.build)
            || rows.insert(key, best).is_some()
        {
            return Err("invalid/duplicate trial record".into());
        }
    }
    Ok(rows)
}
impl Records {
    pub fn load(path: PathBuf) -> Result<Self> {
        let rows = read(&path)?;
        Ok(Self { path, rows })
    }
    pub fn best(&self, key: Key) -> Option<&Best> {
        self.rows.get(&key)
    }
    /// Reload/merge minimum times, sync a new sibling, then replace the old file.
    /// A malformed existing file is retained. I/O failure does not affect driving.
    pub fn record(&mut self, key: Key, result: &Results, build: &str) -> Result<()> {
        if !(1..=99).contains(&key.laps)
            || result.laps.len() != key.laps as usize
            || !valid_build(build)
            || result
                .laps
                .iter()
                .any(|l| !l.valid || l.missed_gate.is_some())
        {
            return Err("invalid completed trial record".into());
        }
        let lap_ns = ns(result.best_lap())?;
        let event_ns = ns(result.seconds)?;
        if event_ns < lap_ns {
            return Err("trial event shorter than its best lap".into());
        }
        let mut sum = 0.;
        for lap in &result.laps {
            ns(lap.seconds)?;
            sum += lap.seconds;
        }
        if !sum.is_finite() || sum > result.seconds + 1e-7 {
            return Err("trial lap sum exceeds event time".into());
        }
        let mut rows = read(&self.path)?;
        rows.entry(key)
            .and_modify(|b| {
                b.lap_ns = b.lap_ns.min(lap_ns);
                if event_ns < b.event_ns {
                    b.event_ns = event_ns;
                    b.build = build.into();
                }
            })
            .or_insert(Best {
                lap_ns,
                event_ns,
                build: build.into(),
            });
        let mut text = String::from(HEADER);
        for (key, b) in &rows {
            text.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\n",
                key.laps,
                key.mode.name(),
                b.lap_ns,
                b.event_ns,
                b.build
            ));
        }
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let file = self
            .path
            .file_name()
            .ok_or("trial record filename")?
            .to_string_lossy();
        let temp = self
            .path
            .with_file_name(format!("{file}.{}.{nonce}.partial", std::process::id()));
        let mut out = fs::File::create_new(&temp)?;
        let write = (|| -> Result<()> {
            out.write_all(text.as_bytes())?;
            out.sync_all()?;
            drop(out);
            fs::rename(&temp, &self.path)?;
            Ok(())
        })();
        if write.is_err() {
            let _ = fs::remove_file(&temp);
        }
        write?;
        self.rows = rows;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn result(seconds: f64) -> Results {
        Results {
            seconds,
            laps: vec![grid_world::lap_timing::Completed {
                number: 1,
                seconds: seconds - 1.,
                sectors: vec![Some(seconds - 1.)],
                valid: true,
                missed_gate: None,
            }],
            invalid_attempts: 0,
        }
    }
    #[test]
    fn persistent_minima_mode_separation_and_malformed_file_preservation() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../local/playtest")
            .join(format!(
                "grid-trial-test-{}-{}.tsv",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
        let key = Key {
            laps: 1,
            mode: Mode::Deformation,
        };
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut r = Records::load(path.clone()).unwrap();
        r.record(key, &result(61.), "test-a").unwrap();
        r.record(key, &result(62.), "test-b").unwrap();
        let mut loaded = Records::load(path.clone()).unwrap();
        assert_eq!(loaded.best(key).unwrap().event_ns, 61_000_000_000);
        let other = Key {
            mode: Mode::Wheels,
            ..key
        };
        loaded.record(other, &result(50.), "test-c").unwrap();
        assert_eq!(
            Records::load(path.clone())
                .unwrap()
                .best(key)
                .unwrap()
                .event_ns,
            61_000_000_000
        );
        loaded.record(key, &result(60.), "test-d").unwrap();
        assert_eq!(
            Records::load(path.clone())
                .unwrap()
                .best(key)
                .unwrap()
                .event_ns,
            60_000_000_000
        );
        fs::write(&path, b"malformed file stays intact").unwrap();
        assert!(loaded.record(key, &result(49.), "test-d").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"malformed file stays intact");
        fs::remove_file(path).unwrap();
    }
}
