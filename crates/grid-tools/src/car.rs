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
        let cars = text.split(',').map(Self::new).collect::<Result<Vec<_>, _>>()?;
        if cars.is_empty() || cars.len() > 12 {
            return Err("car list needs 1..12 stems".into());
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
            other => other,
        }
    }
    pub fn is_default(&self) -> bool {
        self.stem == LACETTI
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
}
