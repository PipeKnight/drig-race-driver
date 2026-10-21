//! Purchased car model selection. A car folder `cars/<stem>` holds the
//! per-model files (`<stem>.csv` tuning, `<stem>.ned` description, LODs,
//! liveries, `.ppts` glass points); the engine audio bank and AI statistics are
//! keyed by the same stem (docs/CAR_GENERALIZATION.md).

/// The prototype's original car (Chevrolet Lacetti) and the default everywhere.
pub const LACETTI: &str = "lac";
/// The other Touring_cars model (BMW 320si), raced alongside the Lacetti.
pub const BMW: &str = "bmw";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Car {
    stem: String,
}

impl Car {
    /// A car folder stem of two to eight lower-case ASCII letters or digits.
    pub fn new(stem: &str) -> Result<Self, Box<dyn std::error::Error>> {
        if !(2..=8).contains(&stem.len())
            || !stem
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        {
            return Err("car must be a cars/<stem> folder name, e.g. lac or bmw".into());
        }
        Ok(Self { stem: stem.into() })
    }
    /// A comma-separated list of stems, e.g. `bmw,lac`.
    pub fn list(text: &str) -> Result<Vec<Self>, Box<dyn std::error::Error>> {
        let cars = text
            .split(',')
            .map(Self::new)
            .collect::<Result<Vec<_>, _>>()?;
        if cars.is_empty() || cars.len() > 20 {
            return Err("car list needs 1..20 stems".into());
        }
        Ok(cars)
    }
    pub fn stem(&self) -> &str {
        &self.stem
    }
    /// The car folder, e.g. `cars/bmw`.
    pub fn folder(&self) -> String {
        format!("cars/{}", self.stem)
    }
    /// `cars/<stem>/<stem><suffix>`, e.g. `file(".ned")`.
    pub fn file(&self, suffix: &str) -> String {
        format!("cars/{0}/{0}{suffix}", self.stem)
    }
    /// High-detail livery texture package `livery_NN`.
    pub fn livery(&self, index: usize) -> String {
        format!(
            "cars/{0}/livery_{index:02}/textures_high/{0}_tex_high_{index:02}.pssg",
            self.stem
        )
    }
    /// A per-model texture name, e.g. `texture("main")` is `bmw_main.tga`.
    pub fn texture(&self, name: &str) -> String {
        format!("{}_{name}.tga", self.stem)
    }
    /// HUD name.
    pub fn label(&self) -> &str {
        match self.stem.as_str() {
            LACETTI => "Lacetti",
            BMW => "BMW 320si",
            // The circuit-series cars (docs/SERIES.md): manufacturer and
            // `vehicle_model` names of database.bin.
            "sup" => "Toyota Supra",
            "sky" => "Nissan Skyline",
            "nsx" => "Honda NSX-R",
            "350" => "Nissan 350Z",
            "sal" => "Saleen S7R",
            "ccg" => "Koenigsegg CCGT",
            "c5r" => "Corvette C5-R",
            "c6r" => "Corvette C6-R",
            "dbr" => "Aston Martin DBR9",
            "lam" => "Lamborghini Murcielago",
            "spk" => "Spyker C8",
            "esp" => "Panoz Esperante",
            "911" => "Porsche 911 GT3",
            other => other,
        }
    }
    pub fn is_default(&self) -> bool {
        self.stem == LACETTI
    }
}

/// A car's `database.bin` `vehicle` row [D] (the fields the engine uses).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vehicle {
    /// `ai_vehicle_type_config`: the `ai/ai_vehicle_config.xml` class.
    pub ai_config: String,
    /// `ai_vehicle_type_track`: the class of the routes' `ai_vehicle_track.xml`.
    pub ai_track: String,
    /// `ai_vehicle_type_statistics`: the `ai/ai_vehicle_statistics.xml` entry.
    pub ai_statistics: String,
    /// `vehicle_class.file_string`, e.g. `Touring_cars`, `gt1`.
    pub class: String,
    /// `vehicle_drive_type.type`: `rwd`, `fwd` or `4wd`.
    pub drive: String,
}

impl Car {
    /// The car's `vehicle` row. Several rows may share a folder (the BMW has
    /// three); they must agree on every field read here.
    pub fn vehicle(&self, root: &std::path::Path) -> Result<Vehicle, Box<dyn std::error::Error>> {
        let db = crate::read_database(root)?;
        Self::vehicle_in(&db, &self.stem)
    }

    pub fn vehicle_in(
        db: &grid_formats::database::Database,
        stem: &str,
    ) -> Result<Vehicle, Box<dyn std::error::Error>> {
        let table = |name: &str| db.table(name).ok_or(format!("database table {name} absent"));
        let name_of = |table_name: &str, id: Option<i32>, column: &str| -> Result<String, String> {
            table(table_name)?
                .rows()
                .find(|r| r.int("id") == id)
                .and_then(|r| r.text(column))
                .map(str::to_string)
                .ok_or(format!("{table_name} row {id:?} absent"))
        };
        let mut found: Option<Vehicle> = None;
        for row in table("vehicle")?.rows().filter(|r| r.text("file_string") == Some(stem)) {
            let text = |c: &str| row.text(c).map(str::to_string).ok_or(format!("vehicle {c} absent"));
            let v = Vehicle {
                ai_config: text("ai_vehicle_type_config")?,
                ai_track: text("ai_vehicle_type_track")?,
                ai_statistics: text("ai_vehicle_type_statistics")?,
                class: name_of("vehicle_class", row.int("vehicle_class_id"), "file_string")?,
                drive: name_of("vehicle_drive_type", row.int("vehicle_drive_type_id"), "type")?,
            };
            match &found {
                Some(f) if *f != v => return Err(format!("vehicle rows of {stem} disagree").into()),
                Some(_) => {}
                None => found = Some(v),
            }
        }
        found.ok_or_else(|| format!("no database vehicle row for cars/{stem}").into())
    }
}

impl Default for Car {
    fn default() -> Self {
        Self::new(LACETTI).unwrap()
    }
}

impl std::fmt::Display for Car {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.stem)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn car_folders() {
        let c = Car::default();
        assert!(c.is_default());
        assert_eq!(c.file(".ned"), "cars/lac/lac.ned");
        assert_eq!(c.texture("main"), "lac_main.tga");
        let b = Car::new("bmw").unwrap();
        assert_eq!(
            b.livery(3),
            "cars/bmw/livery_03/textures_high/bmw_tex_high_03.pssg"
        );
        assert_eq!(b.label(), "BMW 320si");
        assert_eq!(Car::list("bmw,lac").unwrap(), vec![b, c]);
        for bad in ["", "l", "Lac", "lac/", "../x", "lac,", "toolongstem"] {
            assert!(Car::list(bad).is_err(), "{bad}");
        }
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn vehicle_rows_give_the_ai_classes() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let lac = Car::default().vehicle(&root).unwrap();
        assert_eq!((lac.ai_config.as_str(), lac.ai_track.as_str()), ("touring", "touring"));
        assert_eq!((lac.ai_statistics.as_str(), lac.drive.as_str()), ("lac", "fwd"));
        assert_eq!(lac.class, "Touring_cars");
        let bmw = Car::new("bmw").unwrap().vehicle(&root).unwrap();
        assert_eq!((bmw.ai_config.as_str(), bmw.drive.as_str()), ("touring", "rwd"));
        let sky = Car::new("sky").unwrap().vehicle(&root).unwrap();
        assert_eq!((sky.ai_config.as_str(), sky.drive.as_str()), ("streetgt", "4wd"));
        assert_eq!(sky.class, "protuned_street");
        let c65 = Car::new("c65").unwrap().vehicle(&root).unwrap();
        assert_eq!((c65.ai_config.as_str(), c65.class.as_str()), ("lmp2", "lmp2"));
        assert!(Car::new("zz").unwrap().vehicle(&root).is_err());
    }
}
