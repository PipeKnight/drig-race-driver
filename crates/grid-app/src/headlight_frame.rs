//! Shared source lamp points, actual model anchors and presented world frames.
//! No activation clock, optical range, intensity or renderer policy.
use bevy::prelude::{Mat4, Vec3};
use grid_formats::{binxml, pssg, scene};
use grid_tools::{car::Car as CarModel, time_of_day::Mode};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Debug)]
struct Point {
    value: Vec3,
    provenance: &'static str,
}
#[derive(Clone, Default)]
struct Candidate {
    bounds: Vec<Vec<u8>>,
    transforms: Vec<Vec<u8>>,
}
fn selected_point(n: &Candidate) -> Result<Vec3, String> {
    let ([bounds], [matrix]) = (&n.bounds[..], &n.transforms[..]) else {
        return Err("selected lamp requires one bounds and transform".into());
    };
    if bounds.len() != 24 {
        return Err("selected lamp bounds must be six floats".into());
    }
    let b: [f32; 6] =
        std::array::from_fn(|i| f32::from_be_bytes(bounds[i * 4..i * 4 + 4].try_into().unwrap()));
    if b.iter().any(|x| !x.is_finite()) || (0..3).any(|i| b[i] > b[i + 3]) {
        return Err("invalid selected lamp bounds".into());
    }
    let m = scene::decode_matrix(matrix, 0).map_err(|e| e.to_string())?;
    let value = Vec3::new(
        (b[0] + b[3]) * 0.5 + m[12],
        (b[1] + b[4]) * 0.5 + m[13],
        (b[2] + b[5]) * 0.5 + m[14],
    );
    if !value.is_finite() {
        return Err("selected lamp point overflow".into());
    }
    Ok(value)
}
fn point(nodes: &BTreeMap<String, Vec<Candidate>>, offset: Vec3) -> Result<Point, String> {
    for (kind, left, right) in [
        (
            "canonical-LOD0-glass-pair",
            "x0_light_glass_fl",
            "x0_light_glass_fr",
        ),
        (
            "canonical-LOD0-pod-pair",
            "x0_light_pod_fl",
            "x0_light_pod_fr",
        ),
    ] {
        if let (Some(l), Some(r)) = (nodes.get(left), nodes.get(right)) {
            let ([l], [r]) = (&l[..], &r[..]) else {
                return Err("ambiguous selected lamp pair".into());
            };
            let value = (selected_point(l)? + selected_point(r)?) * 0.5 + offset;
            if !value.is_finite() {
                return Err("lamp midpoint overflow".into());
            }
            return Ok(Point {
                value,
                provenance: kind,
            });
        }
    }
    Ok(Point {
        value: Vec3::new(0., 0.6, 2.2) + offset,
        provenance: "source-original-default-pair",
    })
}
fn offsets(ned: &binxml::Document<'_>) -> Result<Vec3, String> {
    let parts: Vec<_> = ned.elements.iter().filter(|e| e.name == "PARTS").collect();
    if parts.is_empty() {
        return Ok(Vec3::ZERO);
    }
    let [parts] = &parts[..] else {
        return Err("ambiguous PARTS".into());
    };
    let mut result = Vec3::new(0., 0., 0.2);
    for (key, axis) in [("m_light_ray_y_offset", 1), ("m_light_ray_z_offset", 2)] {
        let values: Vec<_> = parts.attributes.iter().filter(|(k, _)| *k == key).collect();
        match &values[..] {
            [] => {}
            [(_, text)] => {
                let v = text
                    .parse::<f32>()
                    .map_err(|_| "invalid headlight offset")?;
                if !v.is_finite() {
                    return Err("nonfinite headlight offset".into());
                }
                result[axis] = v;
            }
            _ => return Err("duplicate consumed headlight offset".into()),
        }
    }
    Ok(result)
}
fn descriptor(tree: &pssg::Tree<'_>, ned: &binxml::Document<'_>) -> Result<Point, String> {
    let mut bundles = Vec::new();
    for i in 0..tree.nodes.len() {
        if tree.name(i) == b"MATRIXPALETTEBUNDLENODE"
            && tree.string(i, b"id").map_err(|e| e.to_string())? == b"LOD0_"
        {
            bundles.push(i);
        }
    }
    let [bundle] = &bundles[..] else {
        return Err("expected one canonical LOD0 bundle".into());
    };
    let mut nodes: BTreeMap<String, Vec<Candidate>> = BTreeMap::new();
    for &i in &tree.nodes[*bundle].children {
        if tree.name(i) != b"MATRIXPALETTEJOINTNODE" {
            continue;
        }
        let id = tree.string(i, b"id").map_err(|e| e.to_string())?;
        let Ok(id) = std::str::from_utf8(id) else {
            continue;
        };
        if !matches!(
            id,
            "x0_light_glass_fl" | "x0_light_glass_fr" | "x0_light_pod_fl" | "x0_light_pod_fr"
        ) {
            continue;
        }
        let mut n = Candidate::default();
        for &c in &tree.nodes[i].children {
            match tree.name(c) {
                b"BOUNDINGBOX" => n
                    .bounds
                    .push(tree.nodes[c].data.unwrap_or_default().to_vec()),
                b"TRANSFORM" => n
                    .transforms
                    .push(tree.nodes[c].data.unwrap_or_default().to_vec()),
                _ => {}
            }
        }
        nodes.entry(id.to_owned()).or_default().push(n);
    }
    point(&nodes, offsets(ned)?)
}
pub struct Preparation {
    models: Vec<Point>,
    anchors: Vec<Option<Mat4>>,
    slots: Vec<usize>,
}
impl Preparation {
    pub fn load(root: &Path, models: &[CarModel], slots: &[usize]) -> Result<Self, String> {
        let models = models
            .iter()
            .map(|model| {
                let high = crate::read(&root.join(model.file("_highLOD.pssg")))
                    .map_err(|e| e.to_string())?;
                let ned = crate::read(&root.join(model.file(".ned"))).map_err(|e| e.to_string())?;
                descriptor(
                    &pssg::read_tree(&high).map_err(|e| e.to_string())?,
                    &binxml::read(&ned).map_err(|e| e.to_string())?,
                )
            })
            .collect::<Result<Vec<_>, String>>()?;
        if slots.is_empty() || slots.iter().any(|&m| m >= models.len()) {
            return Err("invalid actual headlight Field mapping".into());
        }
        Ok(Self {
            anchors: vec![None; models.len()],
            models,
            slots: slots.to_vec(),
        })
    }
    pub fn retain_anchor(&mut self, model: usize, anchor: Mat4) -> Result<(), String> {
        let a = self
            .anchors
            .get_mut(model)
            .ok_or("headlight model missing")?;
        if a.is_some() || !anchor.is_finite() {
            return Err("ambiguous/invalid append anchor".into());
        }
        *a = Some(anchor);
        Ok(())
    }
    pub fn finish(self) -> Result<Prepared, String> {
        let anchors = self
            .anchors
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or("missing actual append anchor")?;
        let slots = self
            .slots
            .iter()
            .map(|&model| Slot {
                model,
                model_from_source: anchors[model],
            })
            .collect();
        Ok(Prepared {
            slots,
            models: self.models,
        })
    }
    #[cfg(test)]
    pub(crate) fn fixture(
        models: Vec<(Vec3, &'static str)>,
        anchors: Vec<Option<Mat4>>,
        slots: Vec<usize>,
    ) -> Self {
        Self {
            models: models
                .into_iter()
                .map(|(value, provenance)| Point { value, provenance })
                .collect(),
            anchors,
            slots,
        }
    }
}
struct Slot {
    model: usize,
    model_from_source: Mat4,
}
pub struct Prepared {
    slots: Vec<Slot>,
    models: Vec<Point>,
}
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub origin: Vec3,
    pub direction: Vec3,
    pub up: Vec3,
}
impl Prepared {
    pub fn len(&self) -> usize {
        self.slots.len()
    }
    pub fn selections(&self) -> impl Iterator<Item = (usize, Vec3, &'static str)> + '_ {
        self.models
            .iter()
            .enumerate()
            .map(|(model, p)| (model, p.value, p.provenance))
    }
    pub fn frame(&self, slot: usize, pose: Mat4, mode: Mode) -> Result<Option<Frame>, String> {
        let binding = self.slots.get(slot).ok_or("headlight slot absent")?;
        let y = match mode {
            Mode::Timeline => -0.45,
            Mode::ForcedNight => -0.8,
            Mode::None => return Ok(None),
        };
        if !pose.is_finite() {
            return Err("invalid headlight presentation domain".into());
        }
        let local = Vec3::new(0., y, 0.9962).normalize();
        let direction = pose
            .transform_vector3(local)
            .try_normalize()
            .ok_or("invalid headlight direction")?;
        let up = pose
            .transform_vector3(Vec3::new(0., local.z, -local.y))
            .try_normalize()
            .ok_or("invalid headlight up")?;
        let position =
            (pose * binding.model_from_source).transform_point3(self.models[binding.model].value);
        if !position.is_finite() {
            return Err("invalid world headlight origin".into());
        }
        Ok(Some(Frame {
            origin: position,
            direction,
            up,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn candidate(x: f32) -> Candidate {
        Candidate {
            bounds: vec![
                [x, 0., 0., x + 2., 2., 2.]
                    .into_iter()
                    .flat_map(f32::to_be_bytes)
                    .collect(),
            ],
            transforms: vec![
                Mat4::from_rotation_y(1.)
                    .to_cols_array()
                    .into_iter()
                    .flat_map(f32::to_be_bytes)
                    .collect(),
            ],
        }
    }
    fn ned(
        attributes: Vec<(&'static str, &'static str)>,
        present: bool,
    ) -> binxml::Document<'static> {
        binxml::Document {
            strings: vec![],
            string_padding: &[],
            elements: if present {
                vec![binxml::Element {
                    name: "PARTS",
                    value: None,
                    attributes,
                    children: 0..0,
                    parent: None,
                }]
            } else {
                vec![]
            },
        }
    }
    #[test]
    fn real_point_preparation_preserves_atomic_selection_and_offset_domains() {
        let mut n = BTreeMap::from([
            ("x0_light_glass_fl".into(), vec![candidate(0.)]),
            ("x0_light_glass_fr".into(), vec![candidate(2.)]),
            ("x0_light_pod_fl".into(), vec![candidate(8.)]),
            ("x0_light_pod_fr".into(), vec![candidate(10.)]),
        ]);
        let offset = offsets(&ned(vec![], true)).unwrap();
        assert_eq!(point(&n, offset).unwrap().value, Vec3::new(2., 1., 1.2));
        assert_eq!(offsets(&ned(vec![], false)).unwrap(), Vec3::ZERO);
        n.get_mut("x0_light_pod_fl").unwrap()[0].bounds.clear();
        assert!(
            point(&n, offset).is_ok(),
            "unselected malformed pod ignored"
        );
        n.remove("x0_light_glass_fr");
        assert!(
            point(&n, offset).is_err(),
            "selected malformed pod rejected"
        );
        n.insert("x0_light_pod_fl".into(), vec![candidate(8.)]);
        assert_eq!(
            point(&n, offset).unwrap().provenance,
            "canonical-LOD0-pod-pair"
        );
        n.remove("x0_light_pod_fr");
        assert_eq!(
            point(&n, offset).unwrap().provenance,
            "source-original-default-pair"
        );
        assert!(
            offsets(&ned(
                vec![("m_light_ray_z_offset", "0"), ("m_light_ray_z_offset", "1")],
                true
            ))
            .is_err()
        );
        assert!(offsets(&ned(vec![("m_light_ray_y_offset", "NaN")], true)).is_err());
    }
    #[test]
    #[ignore = "requires absolute GRID_GAME_ROOT; purchased model readers, CPU only"]
    fn purchased_all_six_canonical_source_descriptors() {
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required; no skip"),
        );
        assert!(root.is_absolute());
        for (name, expected) in [
            ("lac", [0.0000333, 0.6453644, 2.1526121]),
            ("bmw", [-0.0006400, 0.7822034, 2.0537605]),
            ("stk", [0.0000000, 0.6, 2.4]),
            ("sal", [-0.0000003, 0.524_505, 2.1360629]),
            ("r10", [-0.0000003, 0.9857384, 2.019_38]),
            ("787", [0.0000003, 0.954_859, 1.9609809]),
        ] {
            let model = CarModel::new(name).unwrap();
            let high = crate::read(&root.join(model.file("_highLOD.pssg"))).unwrap();
            let ned = crate::read(&root.join(model.file(".ned"))).unwrap();
            let p = descriptor(
                &pssg::read_tree(&high).unwrap(),
                &binxml::read(&ned).unwrap(),
            )
            .unwrap();
            assert!(p.value.is_finite());
            assert!(
                p.value.distance(Vec3::from_array(expected)) < 0.000002,
                "independent diagnostic59 point: {name}"
            );
            eprintln!(
                "headlight pilot source {name} {:?} {}",
                p.value, p.provenance
            );
        }
    }
}
