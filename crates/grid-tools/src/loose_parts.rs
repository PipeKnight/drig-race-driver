//! Loose-part impact zones and the wheel detachment rule of the original
//! parts object (docs/MECHANICAL_DAMAGE.md section 5, VISUAL_DAMAGE_PARTS_RESEARCH.md).
//!
//! - 98B430 (the loose-part user 9A7670 of every solved body contact): adds a
//!   notification's impact to the zone accumulators +7F0..+830 by car-local cell;
//! - 9EC9B0 (once per tick): compares the tick's zones with the NED PARTS
//!   levels. Its wheel rule sets car bytes +22E4..+22E7 when a wheel zone
//!   (+824..+830, wheel records RL, RR, FL, FR) exceeds
//!   `m_wheel_detach_level` × mass × `m_overall_speed_required_scalar`.
//!
//! The wheel flags change driving and are simulation state; the body-part
//! rules are presentation (grid-app `damage_parts`).
use crate::car::Car;
use grid_formats::{binxml, car_collision};
use std::path::Path;

/// Zone accumulators of the original parts object (+7F0..+830).
pub const LEFT_SIDE: usize = 0; // +7F0
pub const RIGHT_SIDE: usize = 1; // +7F4
pub const BONNET: usize = 2; // +7F8
pub const FRONT_RIGHT: usize = 3; // +7FC
pub const FRONT_LEFT: usize = 4; // +800
pub const REAR_LEFT_WING: usize = 5; // +804
pub const REAR_RIGHT_WING: usize = 6; // +808
pub const ROOF: usize = 7; // +80C
pub const REAR: usize = 8; // +810
pub const BOOT: usize = 9; // +814
pub const EXHAUST: usize = 10; // +818
pub const REAR_RIGHT: usize = 11; // +81C
pub const REAR_LEFT: usize = 12; // +820
/// Wheel zones +824..+830 in wheel record order (RL, RR, FL, FR).
pub const WHEELS: usize = 13;
pub const ZONES: usize = 17;
/// Window ids 0x4E..0x53 in the original order, with their NED crack-only flag.
pub const WINDOWS: [(&str, &str); 6] = [
    ("window_b", "rearWindow"),
    ("window_bl", "rearLeftWindow"),
    ("window_br", "rearRightWindow"),
    ("window_f", "frontWindow"),
    ("window_fl", "frontLeftWindow"),
    ("window_fr", "frontRightWindow"),
];

/// Swing (loose) and detach rule of one part group; `None` when its NED flag
/// is off.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rule {
    pub swing: Option<f32>,
    pub detach: Option<f32>,
}

/// A car model's NED `PARTS`, `WINDOW` and `SIDE` values plus collision
/// extents, as stored by the original description loader 9EFB30.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub disabled: bool,
    pub scalar: f32,
    pub bonnet: Option<f32>,
    pub boot: Option<f32>,
    /// Boot hinge limit in degrees after the loader's clamp to 5.
    pub boot_max_angle: f32,
    pub exhaust: Rule,
    pub exhaust_swing_multiple: f32,
    pub front_doors: Option<f32>,
    pub rear_doors: Option<f32>,
    pub front_wings: Rule,
    pub rear_wings: Rule,
    pub mirrors_detach: Option<f32>,
    pub mirror_with_door: bool,
    pub front_bumper_detach: Option<f32>,
    pub rear_bumper_detach: Option<f32>,
    pub front_bumper_side: Rule,
    pub rear_bumper_side: Rule,
    /// `m_wheel_detach_level` (+1C3C); no flag gates the wheel rule.
    pub wheel_detach: f32,
    pub crack_cos: f32,
    pub smash_cos: f32,
    pub smash_light: f32,
    pub crack_only: [bool; 6],
    /// SIDE strong points: zone front/rear limits (car +EE2C/+EE30).
    pub front: f32,
    pub rear: f32,
    /// Collision max X and max Y (description m_cs_left_x / m_cs_roof_y).
    pub half_width: f32,
    pub roof: f32,
}

impl Config {
    pub fn read(root: &Path, model: &Car) -> Result<Self, String> {
        let bytes = std::fs::read(root.join(model.file(".ned"))).map_err(|e| e.to_string())?;
        let doc = binxml::read(&bytes).map_err(|e| format!("{e:?}"))?;
        Self::from_ned(&doc)
    }

    pub fn from_ned(doc: &binxml::Document<'_>) -> Result<Self, String> {
        let bounds = car_collision::read(doc)
            .map_err(|e| format!("{e:?}"))?
            .bounds;
        let element = |name: &str| doc.elements.iter().find(|e| e.name == name);
        let value = |e: &str, key: &str| -> Option<f32> {
            element(e)?
                .attributes
                .iter()
                .find(|(k, _)| *k == key)
                .and_then(|(_, v)| v.trim().parse().ok())
        };
        // The loader zero-fills the description, then stores present values;
        // flags are values above 0.5.
        let parts = |key: &str| value("PARTS", key).unwrap_or(0.);
        let flag = |key: &str| parts(key) > 0.5;
        let level = |on: &str, key: &str| flag(on).then(|| parts(key));
        let rule = |swing: &str, swing_level: &str, detach: &str, detach_level: &str| Rule {
            swing: level(swing, swing_level),
            detach: level(detach, detach_level),
        };
        Ok(Self {
            disabled: parts("m_disableLooseParts") > 0.,
            scalar: value("PARTS", "m_overall_speed_required_scalar").unwrap_or(4.),
            bonnet: level("m_bonnet_swings", "m_bonnet_swing_level"),
            boot: level("m_boot_swings", "m_boot_swing_level"),
            boot_max_angle: parts("m_boot_max_angle").min(5.),
            exhaust: rule(
                "m_exhaust_swings",
                "m_exhaust_swing_level",
                "m_exhaust_detaches",
                "m_exhaust_detach_level",
            ),
            exhaust_swing_multiple: value("PARTS", "m_exhaust_swing_multiple").unwrap_or(1.),
            front_doors: level("m_front_doors_swing", "m_front_doors_swing_level"),
            rear_doors: level("m_rear_doors_swing", "m_rear_doors_swing_level"),
            front_wings: rule(
                "m_front_wings_swing",
                "m_front_wings_swing_level",
                "m_front_wings_detach",
                "m_front_wings_detach_level",
            ),
            rear_wings: rule(
                "m_rear_wings_swing",
                "m_rear_wings_swing_level",
                "m_rear_wings_detach",
                "m_rear_wings_detach_level",
            ),
            mirrors_detach: level("m_wing_mirrors_detach", "m_wing_mirrors_detach_level"),
            mirror_with_door: value("PARTS", "m_wing_mirror_move_with_door").unwrap_or(1.) > 0.5,
            front_bumper_detach: level("m_front_bumper_detaches", "m_front_bumper_detach_level"),
            rear_bumper_detach: level("m_rear_bumper_detaches", "m_rear_bumper_detach_level"),
            front_bumper_side: rule(
                "m_front_bumper_side_swings",
                "m_front_bumper_side_swing_level",
                "m_front_bumper_side_detaches",
                "m_front_bumper_side_detach_level",
            ),
            rear_bumper_side: rule(
                "m_rear_bumper_side_swings",
                "m_rear_bumper_side_swing_level",
                "m_rear_bumper_side_detaches",
                "m_rear_bumper_side_detach_level",
            ),
            // The loader seeds +1C3C with 2 x the bonnet detach level, then
            // reads the attribute when present.
            wheel_detach: value("PARTS", "m_wheel_detach_level")
                .unwrap_or(2. * parts("m_bonnet_detach_level")),
            // Loader defaults cos(1 deg) / cos(2 deg) when WINDOW is absent.
            crack_cos: value("WINDOW", "crackCosTheta").unwrap_or(1f32.to_radians().cos()),
            smash_cos: value("WINDOW", "smashCosTheta").unwrap_or(2f32.to_radians().cos()),
            smash_light: value("PARTS", "m_smashLightCluster").unwrap_or(0.1),
            crack_only: WINDOWS.map(|(_, key)| value("WINDOW", key).unwrap_or(0.) != 0.),
            front: value("SIDE", "strong_point_front").unwrap_or(0.),
            rear: value("SIDE", "strong_point_rear").unwrap_or(0.),
            half_width: bounds.max[0],
            roof: bounds.max[1],
        })
    }
}

/// Original 98B430: add one notification's impact `m` to the zone
/// accumulators. `p` and `n` are the contact point minus the car offset
/// (+1630) and the normal, both in the car frame; `inverted` is the car's up
/// axis pointing below the horizon (pose row 1 Y below zero).
pub fn classify(
    config: &Config,
    p: [f32; 3],
    n: [f32; 3],
    m: f32,
    inverted: bool,
    zones: &mut [f32; ZONES],
) {
    // Roof hits count only on an inverted car.
    if config.roof - 0.4 < p[1] && n[1] < -std::f32::consts::FRAC_1_SQRT_2 && inverted {
        zones[ROOF] += -n[1] * m;
    }
    let mut lateral = 1;
    if config.half_width * 0.5 < p[0] {
        lateral = 2; // left (+X)
    }
    if p[0] < config.half_width * -0.5 {
        lateral = 0; // right
    }
    let mut longitudinal = 1;
    if p[2] > config.front {
        longitudinal = 2;
    }
    if p[2] < config.rear {
        longitudinal = 0;
    }
    let [rl, rr, fl, fr] = [WHEELS, WHEELS + 1, WHEELS + 2, WHEELS + 3];
    match (longitudinal, lateral) {
        (0, _) => {
            for z in [REAR, BOOT, EXHAUST] {
                zones[z] += m;
            }
            match lateral {
                0 => {
                    zones[REAR_RIGHT] += m;
                    zones[REAR_RIGHT_WING] += m;
                    zones[rl] += m * 0.2;
                    zones[rr] += m;
                }
                1 => {
                    zones[rl] += m;
                    zones[rr] += m;
                }
                _ => {
                    zones[REAR_LEFT_WING] += m;
                    zones[REAR_LEFT] += m;
                    zones[rl] += m;
                    zones[rr] += m * 0.2;
                }
            }
        }
        (1, 0) => {
            if n[0] > 0. {
                zones[RIGHT_SIDE] += (n[0] * m).abs();
                zones[rr] += m * 0.5;
                zones[fr] += m * 0.5;
            }
        }
        (1, 2) => {
            if n[0] < 0. {
                zones[LEFT_SIDE] += (n[0] * m).abs();
            }
            zones[rl] += m * 0.5;
            zones[fl] += m * 0.5;
        }
        (1, _) => {}
        (_, 0) => {
            zones[FRONT_RIGHT] += m;
            zones[fl] += m * 0.2;
            zones[fr] += m;
            if n[2] < 0. {
                zones[BONNET] += -n[2] * m;
            }
        }
        (_, 1) => {
            if n[2] < 0. {
                zones[BONNET] += -n[2] * m;
            }
            zones[fl] += m;
            zones[fr] += m;
        }
        _ => {
            zones[FRONT_LEFT] += m;
            zones[fl] += m;
            zones[fr] += m * 0.2;
            if n[2] < 0. {
                zones[BONNET] += -n[2] * m;
            }
        }
    }
}

/// Car frame of one tick from the physical pose (rows 0..2 the basis, row 3
/// the position) and the car offset +1630.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub pose: [f32; 16],
    pub offset: [f32; 3],
}

impl Frame {
    fn local(&self, v: [f32; 3], point: bool) -> [f32; 3] {
        let t = if point {
            std::array::from_fn(|j| v[j] - self.pose[12 + j])
        } else {
            v
        };
        std::array::from_fn(|i| {
            let row = &self.pose[i * 4..i * 4 + 3];
            row[0] * t[0] + row[1] * t[1] + row[2] * t[2]
        })
    }
    /// 98B430 for one world-space notification.
    pub fn classify(
        &self,
        config: &Config,
        impact: &crate::body_impacts::Impact,
        zones: &mut [f32; ZONES],
    ) {
        let p = self.local(impact.point, true);
        let p = std::array::from_fn(|j| p[j] - self.offset[j]);
        let n = self.local(impact.normal, false);
        classify(config, p, n, impact.impact, self.pose[5] < 0., zones);
    }
}

/// The wheel rule of 9EC9B0 for one tick's zones: the wheels whose flag is
/// newly set. S = mass × scalar is stored as f32 before the comparison
/// `level × S < zone`. `m_disableLooseParts` skips every rule (9EC9B0 jumps
/// to the zone reset); parts +7BF1 is set only by game-mode code and stays 0.
pub fn wheels_to_detach(
    config: &Config,
    zones: &[f32; ZONES],
    mass: f32,
    detached: [bool; 4],
) -> [bool; 4] {
    if config.disabled {
        return [false; 4];
    }
    let s = mass * config.scalar;
    std::array::from_fn(|i| !detached[i] && config.wheel_detach * s < zones[WHEELS + i])
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn config() -> Config {
        Config {
            disabled: false,
            scalar: 4.,
            bonnet: Some(3.),
            boot: Some(2.5),
            boot_max_angle: 5.,
            exhaust: Rule::default(),
            exhaust_swing_multiple: 1.,
            front_doors: Some(5.),
            rear_doors: None,
            front_wings: Rule::default(),
            rear_wings: Rule::default(),
            mirrors_detach: None,
            mirror_with_door: true,
            front_bumper_detach: None,
            rear_bumper_detach: None,
            front_bumper_side: Rule::default(),
            rear_bumper_side: Rule::default(),
            wheel_detach: 6.,
            crack_cos: 0.999903,
            smash_cos: 0.999391,
            smash_light: 0.1,
            crack_only: [false; 6],
            front: 0.38,
            rear: -1.2,
            half_width: 0.9,
            roof: 1.4,
        }
    }

    #[test]
    fn a_wheel_detaches_once_above_level_times_scalar_times_mass() {
        let c = config();
        let mass = 1140.;
        let threshold = 6. * (1140. * 4.);
        assert_eq!(threshold, 27360.);
        let mut z = [0.; ZONES];
        // Equal is not above.
        z[WHEELS + 2] = threshold;
        assert_eq!(wheels_to_detach(&c, &z, mass, [false; 4]), [false; 4]);
        z[WHEELS + 2] = threshold + 1.;
        z[WHEELS + 3] = threshold * 0.2;
        assert_eq!(
            wheels_to_detach(&c, &z, mass, [false; 4]),
            [false, false, true, false]
        );
        // An already detached wheel is not set again.
        assert_eq!(
            wheels_to_detach(&c, &z, mass, [false, false, true, false]),
            [false; 4]
        );
        // m_disableLooseParts stops every rule.
        let off = Config {
            disabled: true,
            ..c
        };
        assert_eq!(wheels_to_detach(&off, &z, mass, [false; 4]), [false; 4]);
    }

    #[test]
    fn a_front_left_corner_hit_loads_the_front_left_wheel_zone() {
        let c = config();
        // Car turned 90 degrees about Y and moved; offset +1630 subtracted.
        let pose = [
            0., 0., -1., 0., //
            0., 1., 0., 0., //
            1., 0., 0., 0., //
            10., 2., 5., 1.,
        ];
        let frame = Frame {
            pose,
            offset: [0., 0.1, 0.],
        };
        // World (12, 2.6, 4.2) is local (+0.8, 0.6, +2.0), minus the offset.
        let impact = crate::body_impacts::Impact {
            point: [12., 2.6, 4.2],
            normal: [-1., 0., 0.],
            impact: 30000.,
        };
        let mut z = [0.; ZONES];
        frame.classify(&c, &impact, &mut z);
        assert_eq!(z[FRONT_LEFT], 30000.);
        assert_eq!([z[WHEELS + 2], z[WHEELS + 3]], [30000., 6000.]);
        assert_eq!(z[BONNET], 30000.);
        assert_eq!(
            wheels_to_detach(&c, &z, 1140., [false; 4]),
            [false, false, true, false]
        );
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn lacetti_and_bmw_wheels_detach_at_level_six() {
        let root = std::env::var("GRID_GAME_ROOT").unwrap();
        for stem in ["lac", "bmw"] {
            let c = Config::read(Path::new(&root), &Car::new(stem).unwrap()).unwrap();
            assert!(!c.disabled, "{stem}");
            assert_eq!((c.wheel_detach, c.scalar), (6., 4.), "{stem}");
        }
    }
}
