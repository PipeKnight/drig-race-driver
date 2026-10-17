//! Shipped per-car `cars/<stem>/cameras.xml` (sectioned binary XML):
//! `<VehicleViewDefinition><ViewManager name>` holding `<View type ident
//! [reverseCamera] hidden transitionType transitionSpeed transitionStrength>`
//! elements, each with `<Parameter name type value|x y z>` children. The
//! original binds the parameters by name onto its camera classes (Chase
//! 0x7F31B0, Mounted 0x7F4BE0, Head 0x7F3A70); see docs/CAMERA_VIEWS.md.
use crate::{Result, binxml, fail};

/// The three camera classes a view can name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Chase,
    Mounted,
    Head,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Scalar(f32),
    Bool(bool),
    Vector([f32; 3]),
}

#[derive(Clone, Debug, PartialEq)]
pub struct View {
    pub kind: Kind,
    pub ident: String,
    /// The view the original shows while Look Back is held.
    pub reverse: Option<String>,
    /// Hidden views are skipped by Change View (reverse and replay views).
    pub hidden: bool,
    pub transition: String,
    pub transition_speed: f32,
    pub transition_strength: f32,
    /// Parameters in file order; names keep the file's spelling (one has a
    /// trailing space).
    pub parameters: Vec<(String, Value)>,
}

impl View {
    pub fn get(&self, name: &str) -> Option<Value> {
        self.parameters
            .iter()
            .find(|(n, _)| n.trim() == name)
            .map(|(_, v)| *v)
    }
    pub fn scalar(&self, name: &str) -> Option<f32> {
        match self.get(name)? {
            Value::Scalar(v) => Some(v),
            _ => None,
        }
    }
    pub fn flag(&self, name: &str) -> Option<bool> {
        match self.get(name)? {
            Value::Bool(v) => Some(v),
            _ => None,
        }
    }
    pub fn vector(&self, name: &str) -> Option<[f32; 3]> {
        match self.get(name)? {
            Value::Vector(v) => Some(v),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Views {
    /// `ViewManager name`, the car stem.
    pub name: String,
    /// Views in file order.
    pub views: Vec<View>,
}

fn number(text: &str) -> Result<f32> {
    text.trim()
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite())
        .map_or_else(|| fail(0, "camera parameter is not a finite number"), Ok)
}

pub fn read(bytes: &[u8]) -> Result<Views> {
    let doc = binxml::read(bytes)?;
    let attr = |i: usize, name: &str| {
        doc.elements[i]
            .attributes
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| *v)
    };
    let root = doc
        .elements
        .iter()
        .position(|e| e.parent.is_none())
        .filter(|&i| doc.elements[i].name == "VehicleViewDefinition");
    let Some(root) = root else {
        return fail(0, "camera definition root is not VehicleViewDefinition");
    };
    let manager = doc.elements[root].children.clone().next();
    let Some(manager) = manager.filter(|&i| doc.elements[i].name == "ViewManager") else {
        return fail(0, "camera definition has no ViewManager");
    };
    let mut views = Vec::new();
    for v in doc.elements[manager].children.clone() {
        if doc.elements[v].name != "View" {
            return fail(0, "ViewManager child is not a View");
        }
        let kind = match attr(v, "type") {
            Some("Chase") => Kind::Chase,
            Some("Mounted") => Kind::Mounted,
            Some("Head") => Kind::Head,
            _ => return fail(0, "unknown camera view type"),
        };
        let mut parameters = Vec::new();
        for p in doc.elements[v].children.clone() {
            if doc.elements[p].name != "Parameter" {
                return fail(0, "View child is not a Parameter");
            }
            let name = attr(p, "name").unwrap_or_default();
            let value = match attr(p, "type") {
                Some("scalar") => Value::Scalar(number(attr(p, "value").unwrap_or_default())?),
                Some("bool") => Value::Bool(attr(p, "value") == Some("true")),
                Some("vector3") => Value::Vector([
                    number(attr(p, "x").unwrap_or_default())?,
                    number(attr(p, "y").unwrap_or_default())?,
                    number(attr(p, "z").unwrap_or_default())?,
                ]),
                _ => return fail(0, "unknown camera parameter type"),
            };
            parameters.push((name.to_owned(), value));
        }
        views.push(View {
            kind,
            ident: attr(v, "ident").unwrap_or_default().to_owned(),
            reverse: attr(v, "reverseCamera").map(str::to_owned),
            hidden: attr(v, "hidden") == Some("true"),
            transition: attr(v, "transitionType").unwrap_or_default().to_owned(),
            transition_speed: number(attr(v, "transitionSpeed").unwrap_or("0"))?,
            transition_strength: number(attr(v, "transitionStrength").unwrap_or("0"))?,
            parameters,
        });
    }
    Ok(Views {
        name: attr(manager, "name").unwrap_or_default().to_owned(),
        views,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped(stem: &str) -> Views {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let path = std::path::Path::new(&root).join(format!("cars/{stem}/cameras.xml"));
        read(&std::fs::read(path).unwrap()).unwrap()
    }

    #[test]
    #[ignore = "requires the purchased installation (GRID_GAME_ROOT)"]
    fn shipped_touring_car_views() {
        for (stem, bonnet_y, head) in [
            ("lac", 1.1055, [0.3604, 1.1047, -0.1923]),
            ("bmw", 1.163, [0.4310, 1.0713, -0.2152]),
        ] {
            let views = shipped(stem);
            assert_eq!(views.name, stem);
            assert_eq!(views.views.len(), 20);
            let visible: Vec<&str> = views
                .views
                .iter()
                .filter(|v| !v.hidden)
                .map(|v| v.ident.as_str())
                .collect();
            assert_eq!(
                visible,
                ["chase_close", "chase_far", "bumper", "bonnet", "head-cam"]
            );
            for v in views.views.iter().filter(|v| !v.hidden) {
                let reverse = v.reverse.as_deref().unwrap();
                assert!(
                    views
                        .views
                        .iter()
                        .any(|r| r.ident == reverse && r.hidden)
                );
                assert_eq!(v.transition, "switch");
            }
            let bonnet = views.views.iter().find(|v| v.ident == "bonnet").unwrap();
            assert_eq!(bonnet.kind, Kind::Mounted);
            assert_eq!(bonnet.vector("offset").unwrap()[1], bonnet_y);
            let cam = views.views.iter().find(|v| v.ident == "head-cam").unwrap();
            assert_eq!(cam.kind, Kind::Head);
            assert_eq!(cam.vector("position").unwrap(), head);
            // The parameter with the trailing space in its name.
            assert_eq!(cam.scalar("bump_interval"), Some(0.2673));
        }
    }
}
