//! Loose and detached body parts, broken glass and smashed light clusters.
//!
//! Presentation only: reads each car's solved contact notifications, pose,
//! mass and deformation field, and never writes simulation state. Recovered
//! rules (docs/VISUAL_DAMAGE_PARTS_RESEARCH.md):
//! - impact zones of the loose-part user 98B430 and the swing/detach decisions
//!   of 9EC9B0 with the car's NED PARTS levels; a part reacts when one tick's
//!   zone impact exceeds level x m_overall_speed_required_scalar x mass;
//! - window crack/smash of 9D12B0: anchor triangles turned by the deformation
//!   field beyond crackCosTheta / smashCosTheta;
//! - light-cluster smash of 9D1A90: anchor moved more than m_smashLightCluster.
//!
//! AUTHORED (original behaviour not ported): hinge pivots and swing dynamics
//! (the original builds physical hinges in 9DB0E0 -> 9AA960), detached-part
//! flight, glass shards from the lac.ppts points, window anchor layout and the
//! crack-map materials standing in for the original `_Cracked` shader input.
use bevy::{
    asset::RenderAssetUsages,
    camera::primitives::Aabb,
    image::{CompressedImageFormats, ImageSampler, ImageType},
    prelude::*,
};
use grid_tools::body_impacts::Impact;
use std::{collections::BTreeMap, path::Path};

/// Zone accumulators of the original parts object (+7F0..+830).
const LEFT_SIDE: usize = 0; // +7F0
const RIGHT_SIDE: usize = 1; // +7F4
const BONNET: usize = 2; // +7F8
const FRONT_RIGHT: usize = 3; // +7FC
const FRONT_LEFT: usize = 4; // +800
const REAR_LEFT_WING: usize = 5; // +804
const REAR_RIGHT_WING: usize = 6; // +808
const ROOF: usize = 7; // +80C
const REAR: usize = 8; // +810
const BOOT: usize = 9; // +814
const EXHAUST: usize = 10; // +818
const REAR_RIGHT: usize = 11; // +81C
const REAR_LEFT: usize = 12; // +820
/// Wheels RL, RR, FL, FR (+824..+830); wheel detachment is physics and is
/// left to the simulation's own wheel damage.
const WHEELS: usize = 13;
const ZONES: usize = 17;
/// Exhaust part ids 31, 34, 101, 102 of the original exhaust list.
const EXHAUSTS: [&str; 4] = ["exhaust", "exhaust_b", "exhaust_bl", "exhaust_br"];
/// Window ids 0x4E..0x53 in the original order, with their NED crack-only flag.
const WINDOWS: [(&str, &str); 6] = [
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
struct Rule {
    swing: Option<f32>,
    detach: Option<f32>,
}

/// Lacetti NED `PARTS`, `WINDOW` and `SIDE` values plus collision extents, as
/// stored by the original description loader 9EFB30.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Config {
    disabled: bool,
    scalar: f32,
    bonnet: Option<f32>,
    boot: Option<f32>,
    /// Boot hinge limit in degrees after the loader's clamp to 5.
    boot_max_angle: f32,
    exhaust: Rule,
    exhaust_swing_multiple: f32,
    front_doors: Option<f32>,
    rear_doors: Option<f32>,
    front_wings: Rule,
    rear_wings: Rule,
    mirrors_detach: Option<f32>,
    mirror_with_door: bool,
    front_bumper_detach: Option<f32>,
    rear_bumper_detach: Option<f32>,
    front_bumper_side: Rule,
    rear_bumper_side: Rule,
    crack_cos: f32,
    smash_cos: f32,
    smash_light: f32,
    crack_only: [bool; 6],
    /// SIDE strong points: zone front/rear limits (car +EE2C/+EE30).
    front: f32,
    rear: f32,
    /// Collision max X and max Y (description m_cs_left_x / m_cs_roof_y).
    half_width: f32,
    roof: f32,
}

impl Config {
    pub(crate) fn read(root: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(root.join("cars/lac/lac.ned")).map_err(|e| e.to_string())?;
        let doc = grid_formats::binxml::read(&bytes).map_err(|e| format!("{e:?}"))?;
        let bounds = grid_formats::car_collision::read(&doc)
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

/// Car frame of one tick: world-to-model transform, the car-local offset at
/// car +1630 and the world Y of the car's up axis.
struct Frame {
    inverse: Mat4,
    offset: Vec3,
    up_y: f32,
}

/// Original 98B430: add one notification's impact to the zone accumulators.
fn classify(config: &Config, frame: &Frame, impact: &Impact, zones: &mut [f32; ZONES]) {
    let p = frame.inverse.transform_point3(Vec3::from(impact.point)) - frame.offset;
    let n = frame.inverse.transform_vector3(Vec3::from(impact.normal));
    let m = impact.impact;
    // Roof hits count only on an inverted car (pose row 1 Y below zero).
    if config.roof - 0.4 < p.y && n.y < -std::f32::consts::FRAC_1_SQRT_2 && frame.up_y < 0. {
        zones[ROOF] += -n.y * m;
    }
    let mut lateral = 1;
    if config.half_width * 0.5 < p.x {
        lateral = 2; // left (+X)
    }
    if p.x < config.half_width * -0.5 {
        lateral = 0; // right
    }
    let mut longitudinal = 1;
    if p.z > config.front {
        longitudinal = 2;
    }
    if p.z < config.rear {
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
            if n.x > 0. {
                zones[RIGHT_SIDE] += (n.x * m).abs();
                zones[rr] += m * 0.5;
                zones[fr] += m * 0.5;
            }
        }
        (1, 2) => {
            if n.x < 0. {
                zones[LEFT_SIDE] += (n.x * m).abs();
            }
            zones[rl] += m * 0.5;
            zones[fl] += m * 0.5;
        }
        (1, _) => {}
        (_, 0) => {
            zones[FRONT_RIGHT] += m;
            zones[fl] += m * 0.2;
            zones[fr] += m;
            if n.z < 0. {
                zones[BONNET] += -n.z * m;
            }
        }
        (_, 1) => {
            if n.z < 0. {
                zones[BONNET] += -n.z * m;
            }
            zones[fl] += m;
            zones[fr] += m;
        }
        _ => {
            zones[FRONT_LEFT] += m;
            zones[fl] += m;
            zones[fr] += m * 0.2;
            if n.z < 0. {
                zones[BONNET] += -n.z * m;
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum State {
    #[default]
    Attached,
    Loose,
    Detached,
}

/// One requested state change with the zone value and threshold that caused it.
#[derive(Debug, PartialEq)]
struct Change {
    part: &'static str,
    state: State,
    zone: f32,
    threshold: f32,
}

/// Decisions of one tick so far, over the car's states before the tick.
struct Decisions<'a> {
    out: Vec<Change>,
    state: &'a dyn Fn(&str) -> Option<State>,
}
impl Decisions<'_> {
    fn current(&self, part: &str) -> Option<State> {
        self.out
            .iter()
            .rev()
            .find(|x| x.part == part)
            .map(|x| x.state)
            .or_else(|| (self.state)(part))
    }
    /// FUN_009a7cb0: absent parts read as state 0.
    fn attached(&self, part: &str) -> bool {
        self.current(part).is_none_or(|s| s == State::Attached)
    }
    fn not_detached(&self, part: &str) -> bool {
        self.current(part).is_some_and(|s| s != State::Detached)
    }
    fn push(&mut self, part: &'static str, state: State, zone: f32, threshold: f32) {
        if self
            .current(part)
            .is_some_and(|s| s != state && s != State::Detached)
        {
            self.out.push(Change {
                part,
                state,
                zone,
                threshold,
            });
        }
    }
    /// Wing-style rule: detach above level x S, else swing above level x mass.
    fn wing(&mut self, rule: Rule, s: f32, scalar: f32, mass: f32, part: &'static str, zone: f32) {
        if let Some(level) = rule.detach
            && level * s < zone
        {
            self.push(part, State::Detached, zone, level * s);
        } else if let Some(level) = rule.swing
            && level * s < scalar * zone
            && self.not_detached(part)
        {
            self.push(part, State::Loose, zone, level * mass);
        }
    }
    /// Bumper corner rule: as `wing`, without the detached-state guard.
    fn corner(
        &mut self,
        rule: Rule,
        s: f32,
        scalar: f32,
        mass: f32,
        part: &'static str,
        zone: f32,
    ) {
        if let Some(level) = rule.detach
            && level * s < zone
        {
            self.push(part, State::Detached, zone, level * s);
        } else if let Some(level) = rule.swing
            && level * s < scalar * zone
        {
            self.push(part, State::Loose, zone, level * mass);
        }
    }
}

/// Original 9EC9B0 decisions for one tick. `state` reports a part's current
/// state, or `None` when the car model has no such part.
fn decide(
    c: &Config,
    zones: &[f32; ZONES],
    totals: &[f32; ZONES],
    mass: f32,
    state: &dyn Fn(&str) -> Option<State>,
) -> Vec<Change> {
    let mut d = Decisions {
        out: Vec::new(),
        state,
    };
    let s = mass * c.scalar;
    let roof_doors = zones[ROOF] > 0. && s * 0.25 < zones[ROOF] && c.front_doors.is_some();
    if let Some(level) = c.bonnet
        && level * s < zones[BONNET]
        && d.not_detached("bonnet")
    {
        d.push("bonnet", State::Loose, zones[BONNET], level * s);
    }
    if let Some(level) = c.boot
        && level * s < zones[BOOT]
        && level * s * 2.5 < totals[BOOT]
        && d.not_detached("boot")
    {
        d.push("boot", State::Loose, zones[BOOT], level * s);
    }
    for part in EXHAUSTS {
        let zone = zones[EXHAUST];
        if let Some(level) = c.exhaust.detach
            && level * s < zone
        {
            d.push(part, State::Detached, zone, level * s);
        } else if let Some(level) = c.exhaust.swing
            && level * s < zone
            && d.not_detached(part)
        {
            // The original also requires car +1554 to be zero [I].
            d.push(part, State::Loose, zone, level * s);
        }
    }
    if let Some(level) = c.front_doors {
        for (part, zone) in [("door_fr", RIGHT_SIDE), ("door_fl", LEFT_SIDE)] {
            if (level * s < c.scalar * zones[zone] || roof_doors) && d.not_detached(part) {
                d.push(part, State::Loose, zones[zone], level * mass);
            }
        }
    }
    d.wing(
        c.rear_wings,
        s,
        c.scalar,
        mass,
        "wing_br",
        zones[REAR_RIGHT_WING],
    );
    d.wing(
        c.rear_wings,
        s,
        c.scalar,
        mass,
        "wing_bl",
        zones[REAR_LEFT_WING],
    );
    d.wing(
        c.front_wings,
        s,
        c.scalar,
        mass,
        "wing_fr",
        zones[FRONT_RIGHT],
    );
    d.wing(
        c.front_wings,
        s,
        c.scalar,
        mass,
        "wing_fl",
        zones[FRONT_LEFT],
    );
    if let Some(level) = c.mirrors_detach {
        for (part, zone) in [
            ("wing_mirror_r", FRONT_RIGHT),
            ("wing_mirror_l", FRONT_LEFT),
        ] {
            if level * s < zones[zone] {
                d.push(part, State::Detached, zones[zone], level * s);
            }
        }
    }
    if let Some(level) = c.rear_doors {
        for (part, zone) in [("door_br", RIGHT_SIDE), ("door_bl", LEFT_SIDE)] {
            if level * s < zones[zone] && d.not_detached(part) {
                d.push(part, State::Loose, zones[zone], level * s);
            }
        }
    }
    let corners = zones[FRONT_LEFT] + zones[FRONT_RIGHT];
    if let Some(level) = c.front_bumper_detach
        && level * s < corners
        && state("bumper_fm").is_some()
    {
        for part in ["bumper_fl", "bumper_fr", "bumper_fm"] {
            d.push(part, State::Detached, corners, level * s);
        }
    }
    if d.attached("bumper_fm") {
        d.corner(
            c.front_bumper_side,
            s,
            c.scalar,
            mass,
            "bumper_fr",
            zones[FRONT_RIGHT],
        );
    }
    if d.attached("bumper_fm") {
        d.corner(
            c.front_bumper_side,
            s,
            c.scalar,
            mass,
            "bumper_fl",
            zones[FRONT_LEFT],
        );
    }
    if let Some(level) = c.rear_bumper_detach {
        let zone = zones[REAR];
        if level * s < zone {
            if d.current("bumper_bm") != Some(State::Detached) {
                for part in ["bumper_bl", "bumper_br", "bumper_bm"] {
                    d.push(part, State::Detached, zone, level * s);
                }
            }
        } else if level * s * 0.6 < zone
            && d.attached("bumper_bm")
            && (d.attached("bumper_bl") || d.attached("bumper_br"))
        {
            d.push("bumper_bm", State::Loose, zone, level * s * 0.6);
        }
    }
    if d.attached("bumper_bm") {
        d.corner(
            c.rear_bumper_side,
            s,
            c.scalar,
            mass,
            "bumper_br",
            zones[REAR_RIGHT],
        );
    }
    if d.attached("bumper_bm") {
        d.corner(
            c.rear_bumper_side,
            s,
            c.scalar,
            mass,
            "bumper_bl",
            zones[REAR_LEFT],
        );
    }
    d.out
}

#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
enum Glass {
    #[default]
    Intact,
    Cracked,
    Smashed,
}

/// Original 9D12B0 per-triangle rule: compare each triangle's rest normal
/// with its deformed normal; anchors are four corners and a centre.
fn glass_state(rest: &[Vec3; 5], moved: &[Vec3; 5], c: &Config, crack_only: bool) -> Glass {
    let normal = |p: &[Vec3; 5], [a, b, k]: [usize; 3]| (p[b] - p[a]).cross(p[k] - p[a]);
    let mut worst = Glass::Intact;
    for t in [[0, 1, 4], [1, 2, 4], [2, 3, 4], [3, 0, 4]] {
        let (a, b) = (normal(rest, t), normal(moved, t));
        if a.length_squared() < 1e-12 || b.length_squared() < 1e-12 {
            continue;
        }
        let cos = a.normalize().dot(b.normalize()).abs();
        let state = if c.crack_cos <= cos {
            Glass::Intact
        } else if c.smash_cos <= cos || crack_only {
            Glass::Cracked
        } else {
            Glass::Smashed
        };
        if state > worst {
            worst = state;
        }
    }
    worst
}

/// Spawned car draw with its source joint (LOD prefix removed).
#[derive(Component)]
pub(crate) struct Joint(String);
impl Joint {
    pub(crate) fn new(joint: &str) -> Self {
        Self(joint.split_once('_').map_or(joint, |(_, j)| j).to_string())
    }
}
#[derive(Component)]
struct Shard {
    velocity: Vec3,
    age: f32,
}
/// Original material of a draw whose material was swapped for damage.
#[derive(Component)]
struct Original(Handle<StandardMaterial>);

/// AUTHORED hinge of a loose part, in model space.
#[derive(Clone, Copy)]
struct Hinge {
    pivot: Vec3,
    axis: Vec3,
    max: f32,
    /// Direction the part centre moves as the angle opens.
    opening: Vec3,
}
impl Hinge {
    fn matrix(&self, angle: f32) -> Mat4 {
        Mat4::from_translation(self.pivot)
            * Mat4::from_axis_angle(self.axis, angle)
            * Mat4::from_translation(-self.pivot)
    }
}

/// AUTHORED flight of a detached part, applied to its model-to-world transform.
#[derive(Clone, Copy)]
struct Debris {
    origin: Mat4,
    centre: Vec3,
    position: Vec3,
    velocity: Vec3,
    rotation: Quat,
    spin: Vec3,
    ground: f32,
}
impl Debris {
    fn matrix(&self) -> Mat4 {
        Mat4::from_translation(self.position)
            * Mat4::from_quat(self.rotation)
            * Mat4::from_translation(-self.centre)
            * self.origin
    }
    fn step(&mut self, dt: f32) {
        self.velocity.y -= 9.81 * dt;
        self.position += self.velocity * dt;
        let w = self.spin.length();
        if w > 1e-4 {
            self.rotation =
                (Quat::from_axis_angle(self.spin / w, w * dt) * self.rotation).normalize();
        }
        if self.position.y < self.ground {
            self.position.y = self.ground;
            self.velocity = Vec3::new(
                self.velocity.x * 0.6,
                -self.velocity.y * 0.25,
                self.velocity.z * 0.6,
            );
            self.spin *= 0.6;
            if self.velocity.length() < 0.3 {
                self.velocity = Vec3::ZERO;
                self.spin = Vec3::ZERO;
            }
        }
    }
}

#[derive(Default)]
struct Part {
    state: State,
    angle: f32,
    rate: f32,
    debris: Option<Debris>,
}

#[derive(Default)]
struct CarDamage {
    totals: [f32; ZONES],
    parts: BTreeMap<&'static str, Part>,
    windows: [Glass; 6],
    lights: Vec<bool>,
    field_key: Option<Vec<u32>>,
    velocity: Option<Vec3>,
    acceleration: Vec3,
    shards: Vec<&'static str>,
}

/// Model-space joint bounds and grid-frame glass/light anchors.
struct Layout {
    bounds: BTreeMap<String, (Vec3, Vec3)>,
    windows: [Option<[Vec3; 5]>; 6],
    /// Original 12 light records: member joints and grid-frame anchor.
    lights: Vec<(Vec<String>, Vec3)>,
}

const PARTS: [&str; 22] = [
    "bonnet",
    "boot",
    "bumper_bl",
    "bumper_br",
    "bumper_bm",
    "bumper_fl",
    "bumper_fr",
    "bumper_fm",
    "door_bl",
    "door_br",
    "door_fl",
    "door_fr",
    "exhaust",
    "exhaust_b",
    "exhaust_bl",
    "exhaust_br",
    "wing_bl",
    "wing_br",
    "wing_fl",
    "wing_fr",
    "wing_mirror_l",
    "wing_mirror_r",
];

/// Light records of 9A5EC0: front records are (pod, glass) of one variant and
/// side; rear records join the plain, brake, reverse and combi lamps.
fn light_records() -> Vec<Vec<String>> {
    let mut out = Vec::new();
    for side in ["fl", "fr"] {
        for variant in ["", "2_", "3_"] {
            out.push(vec![
                format!("light_{variant}pod_{side}"),
                format!("light_{variant}glass_{side}"),
            ]);
        }
    }
    for variant in ["", "2_", "3_"] {
        for side in ["bl", "br"] {
            let mut joints = Vec::new();
            for kind in ["", "brake_", "rev_", "combi_"] {
                joints.push(format!("light_{variant}{kind}pod_{side}"));
                joints.push(format!("light_{variant}{kind}glass_{side}"));
            }
            out.push(joints);
        }
    }
    out
}

/// AUTHORED hinge placement from a part's model-space bounds. Angle limits
/// follow the hinge setup of 9DB0E0 where its numbers read as angles: bonnet
/// 0.5 rad, doors 0.4 rad, boot m_boot_max_angle degrees (clamped to 5), rear
/// centre bumper 0.4 rad, exhaust 0.66 x m_exhaust_swing_multiple. Its wing
/// (0.05) and bumper-corner (0.12/0.14) values are of unknown meaning [I];
/// wings 0.15 rad and corners 0.3 rad are AUTHORED so the hang is visible.
/// Pivot edges and axes are AUTHORED.
fn hinge(part: &str, (min, max): (Vec3, Vec3), config: &Config) -> Option<Hinge> {
    let c = (min + max) * 0.5;
    let (pivot, axis, limit) = match part {
        "bonnet" => (Vec3::new(c.x, max.y, min.z), Vec3::NEG_X, 0.5),
        "boot" => (
            Vec3::new(c.x, max.y, max.z),
            Vec3::X,
            config.boot_max_angle.to_radians(),
        ),
        "door_fl" | "door_bl" => (Vec3::new(max.x, c.y, max.z), Vec3::NEG_Y, 0.4),
        "door_fr" | "door_br" => (Vec3::new(min.x, c.y, max.z), Vec3::Y, 0.4),
        "wing_fl" | "wing_bl" => (Vec3::new(c.x, c.y, min.z), Vec3::Y, 0.15),
        "wing_fr" | "wing_br" => (Vec3::new(c.x, c.y, min.z), Vec3::NEG_Y, 0.15),
        "bumper_fl" | "bumper_bl" => (Vec3::new(min.x, max.y, c.z), Vec3::NEG_Z, 0.3),
        "bumper_fr" | "bumper_br" => (Vec3::new(max.x, max.y, c.z), Vec3::Z, 0.3),
        "bumper_bm" => (Vec3::new(max.x, max.y, c.z), Vec3::Z, 0.4),
        p if EXHAUSTS.contains(&p) => (
            Vec3::new(c.x, max.y, max.z),
            Vec3::X,
            0.66 * config.exhaust_swing_multiple,
        ),
        _ => return None,
    };
    let opening = axis.cross(c - pivot).normalize_or_zero();
    Some(Hinge {
        pivot,
        axis,
        max: limit,
        opening,
    })
}

#[derive(Resource)]
pub(crate) struct DamageParts {
    config: Config,
    /// lac.ppts window points (model space) and outward normals per window.
    shards: BTreeMap<String, Vec<(Vec3, Vec3)>>,
    cars: Vec<CarDamage>,
    last_tick: Option<u64>,
    layout: Option<Layout>,
    reset: bool,
    log_zones: bool,
    images: Option<(Handle<Image>, Handle<Image>)>,
    cracked: BTreeMap<(AssetId<StandardMaterial>, bool), Handle<StandardMaterial>>,
    shard_assets: Option<(Handle<Mesh>, Handle<StandardMaterial>)>,
    camera: Option<(usize, Vec3)>,
}

/// lac.ppts: count, then 40-byte entries (32-byte name, point count, offset)
/// of point/normal pairs (six f32 each).
fn read_ppts(bytes: &[u8]) -> Result<BTreeMap<String, Vec<(Vec3, Vec3)>>, String> {
    let word = |o: usize| -> Result<u32, String> {
        bytes
            .get(o..o + 4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
            .ok_or_else(|| "truncated ppts".to_string())
    };
    let count = word(0)? as usize;
    if count > 64 {
        return Err("too many ppts entries".into());
    }
    let mut out: BTreeMap<String, Vec<(Vec3, Vec3)>> = BTreeMap::new();
    for i in 0..count {
        let o = 4 + i * 40;
        let name = bytes.get(o..o + 32).ok_or("truncated ppts name")?;
        let name = String::from_utf8_lossy(name.split(|b| *b == 0).next().unwrap_or(&[]));
        let (points, offset) = (word(o + 32)? as usize, word(o + 36)? as usize);
        if points > 4096 || offset + points * 24 > bytes.len() {
            return Err("ppts points outside file".into());
        }
        let f = |k: usize| f32::from_bits(word(offset + k * 4).unwrap_or(0));
        // "x0_window_f_0" -> window "window_f".
        let window = name
            .split_once('_')
            .map_or(&*name, |(_, rest)| rest)
            .rsplit_once('_')
            .map_or(String::new(), |(window, _)| window.to_string());
        let entry = out.entry(window).or_default();
        for p in 0..points {
            let v: Vec<f32> = (0..6).map(|k| f(p * 6 + k)).collect();
            if v.iter().all(|x| x.is_finite()) {
                entry.push((Vec3::new(v[0], v[1], v[2]), Vec3::new(v[3], v[4], v[5])));
            }
        }
    }
    Ok(out)
}

pub(crate) fn install(app: &mut App, root: &Path) {
    let config = match Config::read(root) {
        Ok(config) => config,
        Err(error) => {
            warn!("Visual damage parts unavailable; driving continues: {error}");
            return;
        }
    };
    let shards = std::fs::read(root.join("cars/lac/lac.ppts"))
        .map_err(|e| e.to_string())
        .and_then(|b| read_ppts(&b))
        .unwrap_or_else(|error| {
            warn!("Glass shard points unavailable: {error}");
            BTreeMap::new()
        });
    println!(
        "drive_damage_parts enabled=true scalar={} crack_cos={} smash_cos={} smash_light={} front={} rear={} half_width={} roof={} shard_windows={} policy=recovered-zones-authored-motion",
        config.scalar,
        config.crack_cos,
        config.smash_cos,
        config.smash_light,
        config.front,
        config.rear,
        config.half_width,
        config.roof,
        shards.len()
    );
    app.insert_resource(DamageParts {
        config,
        shards,
        cars: Vec::new(),
        last_tick: None,
        layout: None,
        reset: false,
        log_zones: std::env::var_os("GRID_DAMAGE_ZONES").is_some(),
        images: None,
        cracked: BTreeMap::new(),
        shard_assets: None,
        camera: std::env::var("GRID_DAMAGE_CAMERA").ok().and_then(|v| {
            let v: Vec<f32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            (v.len() == 4 && v[0] >= 0.).then(|| (v[0] as usize, Vec3::new(v[1], v[2], v[3])))
        }),
    })
    .add_systems(FixedUpdate, tick.after(crate::drive_tick))
    .add_systems(Update, present.after(crate::drive_present));
}

fn byte(words: &[u32], o: usize) -> u8 {
    (words[o / 4] >> (o % 4 * 8)) as u8
}

/// Per-car simulation readings for one tick.
struct Reading {
    pose: Mat4,
    mass: f32,
    offset: Vec3,
    velocity: Vec3,
    enabled: bool,
    impacts: Vec<Impact>,
}

fn reading(runtime: &grid_tools::source_vehicle::Runtime, impacts: Vec<Impact>) -> Reading {
    let owner = &runtime.state.car.owner;
    let f = |o: usize| f32::from_bits(owner.vehicle[o / 4]);
    let v = runtime.velocity();
    Reading {
        pose: Mat4::from_cols_array(&runtime.pose()),
        mass: f32::from_bits(owner.physical[0x3f0 / 4]),
        offset: Vec3::new(f(0x1630), f(0x1634), f(0x1638)),
        velocity: Vec3::new(v[0], v[1], v[2]),
        // 9A7670 forwards notifications only while both damage flags are set.
        enabled: byte(&owner.vehicle, 0xb13) != 0 && byte(&owner.vehicle, 0xb14) != 0,
        impacts,
    }
}

fn tick(driving: Res<crate::Driving>, mut damage: ResMut<DamageParts>) {
    let ticks = driving.runtime.ticks;
    let damage = &mut *damage;
    if damage.last_tick.is_some_and(|last| ticks < last) {
        damage.cars.clear();
        damage.reset = true;
    }
    if damage.last_tick == Some(ticks) || driving.failure.is_some() {
        damage.last_tick = Some(ticks);
        return;
    }
    damage.last_tick = Some(ticks);
    if ticks == 0 {
        return;
    }
    let readings: Vec<Reading> = if let Some(opponents) = &driving.opponents {
        std::iter::once(reading(&driving.runtime, opponents.multi.impacts(0)))
            .chain(
                opponents
                    .multi
                    .cars
                    .iter()
                    .enumerate()
                    .skip(1)
                    .map(|(i, car)| reading(car, opponents.multi.impacts(i))),
            )
            .collect()
    } else {
        vec![reading(&driving.runtime, driving.runtime.impacts())]
    };
    let Some(layout) = &damage.layout else {
        return;
    };
    damage.cars.resize_with(readings.len(), CarDamage::default);
    let dt = grid_tools::source_vehicle::DELTA;
    for (index, (car, r)) in damage.cars.iter_mut().zip(&readings).enumerate() {
        if !r.mass.is_finite() || r.mass <= 0. {
            continue;
        }
        let inverse = r.pose.inverse();
        if let Some(previous) = car.velocity {
            car.acceleration = inverse.transform_vector3((r.velocity - previous) / dt);
        }
        car.velocity = Some(r.velocity);
        let mut zones = [0f32; ZONES];
        if r.enabled {
            let frame = Frame {
                inverse,
                offset: r.offset,
                up_y: r.pose.y_axis.y,
            };
            for impact in &r.impacts {
                classify(&damage.config, &frame, impact, &mut zones);
            }
        }
        for (total, zone) in car.totals.iter_mut().zip(zones) {
            *total += zone;
        }
        if damage.log_zones && zones.iter().any(|z| *z > 0.) {
            log_line!(
                "drive_damage_zones car={index} tick={ticks} mass={} impacts={} zones={zones:?}",
                r.mass,
                r.impacts.len()
            );
        }
        if !damage.config.disabled {
            let parts = &car.parts;
            let present = |part: &str| {
                layout
                    .bounds
                    .contains_key(part)
                    .then(|| parts.get(part).map_or(State::Attached, |p| p.state))
            };
            let changes = decide(&damage.config, &zones, &car.totals, r.mass, &present);
            for change in changes {
                let part = car.parts.entry(change.part).or_default();
                if part.state == State::Detached || part.state == change.state {
                    continue;
                }
                log_line!(
                    "drive_damage_part car={index} tick={ticks} part={} state={:?} zone={:.1} threshold={:.1}",
                    change.part,
                    change.state,
                    change.zone,
                    change.threshold
                );
                if change.state == State::Loose {
                    part.rate = 3.;
                } else {
                    // AUTHORED flight: carried along, pushed outward and up.
                    let (min, max) = layout.bounds[change.part];
                    let centre = (min + max) * 0.5;
                    let hinged = hinge(change.part, (min, max), &damage.config)
                        .map_or(Mat4::IDENTITY, |h| h.matrix(part.angle));
                    let origin = r.pose * hinged;
                    let world = origin.transform_point3(centre);
                    let outward = r.pose.transform_vector3(Vec3::new(
                        centre.x.signum() * 1.5,
                        2.5,
                        centre.z.signum() * 1.0,
                    ));
                    let seed = change.part.len() as f32;
                    part.debris = Some(Debris {
                        origin,
                        centre: world,
                        position: world,
                        velocity: r.velocity + outward,
                        rotation: Quat::IDENTITY,
                        spin: r.pose.transform_vector3(Vec3::new(
                            seed % 3. - 1.,
                            2.,
                            1.5 - seed % 2.,
                        )) * 2.,
                        ground: r.pose.w_axis.y + (max.y - min.y) * 0.5,
                    });
                }
                part.state = change.state;
            }
        }
        // AUTHORED swing: damped spring around a half-open rest angle, driven
        // by the car's inertial acceleration along the opening direction.
        for (name, part) in &mut car.parts {
            match part.state {
                State::Loose => {
                    let Some(h) = layout
                        .bounds
                        .get(*name)
                        .and_then(|b| hinge(name, *b, &damage.config))
                    else {
                        continue;
                    };
                    let drive = -car.acceleration.dot(h.opening) * 1.5;
                    let target = h.max * 0.5;
                    part.rate += (-30. * (part.angle - target) - 2.5 * part.rate + drive) * dt;
                    part.angle += part.rate * dt;
                    if part.angle < 0. || part.angle > h.max {
                        part.angle = part.angle.clamp(0., h.max);
                        part.rate *= -0.3;
                    }
                }
                State::Detached => {
                    if let Some(debris) = &mut part.debris {
                        debris.step(dt);
                    }
                }
                State::Attached => {}
            }
        }
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)] // Explicit Bevy resource/component ownership.
fn present(
    mut commands: Commands,
    driving: Res<crate::Driving>,
    mut damage: ResMut<DamageParts>,
    source: Res<crate::Car>,
    time: Res<Time>,
    mut parts: PartQuery,
    mut shards: Query<
        (Entity, &mut Shard, &mut Transform),
        (Without<Joint>, Without<crate::ViewerCamera>),
    >,
    mut camera: Single<&mut Transform, (With<crate::ViewerCamera>, Without<Joint>, Without<Shard>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let damage = &mut *damage;
    if damage.layout.is_none() {
        damage.layout = Some(layout(&parts));
    }
    if damage.reset {
        damage.reset = false;
        for (entity, .., mut visibility, mut material, original) in &mut parts {
            *visibility = Visibility::Inherited;
            if let Some(original) = original {
                material.0 = original.0.clone();
                commands.entity(entity).remove::<Original>();
            }
        }
        for (entity, ..) in &shards {
            commands.entity(entity).despawn();
        }
    }
    let Some(layout) = &damage.layout else {
        return;
    };
    let runtimes: Vec<&grid_tools::source_vehicle::Runtime> = std::iter::once(&driving.runtime)
        .chain(
            driving
                .opponents
                .iter()
                .flat_map(|o| o.multi.cars.iter().skip(1)),
        )
        .collect();
    damage.cars.resize_with(runtimes.len(), CarDamage::default);
    // Glass and lights from each car's deformation field, when it changes.
    let mut new_shards = Vec::new();
    for (index, (car, runtime)) in damage.cars.iter_mut().zip(&runtimes).enumerate() {
        if !runtime.deformation_enabled || runtime.ticks == 0 {
            continue;
        }
        let Ok(field) = grid_tools::body_deformation::Field::read(&runtime.state.car.owner.vehicle)
        else {
            continue;
        };
        let key = field.key();
        if car.field_key.as_ref() == Some(&key) {
            continue;
        }
        car.field_key = Some(key);
        let moved = |points: &[Vec3]| -> Option<Vec<Vec3>> {
            let queries: Vec<[f32; 4]> = points.iter().map(|p| [p.x, p.y, p.z, 1.]).collect();
            let deltas = field.displacements(&queries).ok()?;
            Some(
                points
                    .iter()
                    .zip(deltas)
                    .map(|(p, d)| *p + Vec3::from(d))
                    .collect(),
            )
        };
        for (w, anchors) in layout.windows.iter().enumerate() {
            let Some(rest) = anchors else { continue };
            let Some(now) = moved(rest) else { continue };
            let now: [Vec3; 5] = std::array::from_fn(|k| now[k]);
            let state = glass_state(rest, &now, &damage.config, damage.config.crack_only[w]);
            if state > car.windows[w] {
                car.windows[w] = state;
                log_line!(
                    "drive_damage_glass car={index} tick={} window={} state={state:?}",
                    runtime.ticks,
                    WINDOWS[w].0
                );
                if state == Glass::Smashed {
                    new_shards.push((index, WINDOWS[w].0));
                }
            }
        }
        car.lights.resize(layout.lights.len(), false);
        for (l, (joints, anchor)) in layout.lights.iter().enumerate() {
            if car.lights[l] {
                continue;
            }
            if let Some(now) = moved(&[*anchor]) {
                let distance = now[0].distance(*anchor);
                if distance > damage.config.smash_light {
                    car.lights[l] = true;
                    log_line!(
                        "drive_damage_light car={index} tick={} cluster={} displacement={distance:.3}",
                        runtime.ticks,
                        joints.first().map_or("", String::as_str)
                    );
                }
            }
        }
    }
    let images = damage.images.get_or_insert_with(|| {
        let mut load = |name: &str| {
            source
                .textures
                .get(&format!("lacetti/{name}"))
                .and_then(|dds| {
                    Image::from_buffer(
                        dds,
                        ImageType::Extension("dds"),
                        CompressedImageFormats::BC,
                        true,
                        ImageSampler::Default,
                        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
                    )
                    .ok()
                })
                .map_or_else(Handle::default, |image| images.add(image))
        };
        (load("lac_glass_dm.tga"), load("lac_lights_dm.tga"))
    });
    let images = images.clone();
    let poses: Vec<Mat4> = runtimes
        .iter()
        .map(|r| Mat4::from_cols_array(&r.pose()))
        .collect();
    for (
        entity,
        joint,
        moving,
        opponent,
        ..,
        mut transform,
        mut visibility,
        mut material,
        original,
    ) in &mut parts
    {
        let (index, rest) = match (moving, opponent) {
            (Some(m), _) => (0, m.rest),
            (_, Some(o)) => (o.car, o.rest),
            _ => continue,
        };
        let Some(car) = damage.cars.get(index) else {
            continue;
        };
        let name = joint.0.as_str();
        // Windows and mirrors ride on their door while it is loose [I].
        let carrier = match name {
            "window_fl" => Some("door_fl"),
            "window_fr" => Some("door_fr"),
            "wing_mirror_l" if damage.config.mirror_with_door => Some("door_fl"),
            "wing_mirror_r" if damage.config.mirror_with_door => Some("door_fr"),
            _ => None,
        };
        let own = car.parts.get(name).filter(|p| p.state != State::Attached);
        let carried = carrier
            .and_then(|door| car.parts.get(door).map(|p| (door, p)))
            .filter(|(_, p)| p.state == State::Loose && own.is_none());
        let model = match (own, carried) {
            (Some(part), _) if part.state == State::Detached => part.debris.map(|d| d.matrix()),
            (Some(part), _) => layout
                .bounds
                .get(name)
                .and_then(|b| hinge(name, *b, &damage.config))
                .map(|h| poses[index] * h.matrix(part.angle)),
            (None, Some((door, part))) => layout
                .bounds
                .get(door)
                .and_then(|b| hinge(door, *b, &damage.config))
                .map(|h| poses[index] * h.matrix(part.angle)),
            _ => None,
        };
        if let Some(model) = model {
            *transform = Transform::from_matrix(model * rest);
        }
        let window = WINDOWS.iter().position(|(w, _)| *w == name);
        let glass = window.map_or(Glass::Intact, |w| car.windows[w]);
        let hidden = glass == Glass::Smashed;
        let target = if hidden {
            Visibility::Hidden
        } else {
            Visibility::Inherited
        };
        if *visibility != target {
            *visibility = target;
        }
        let light = name.contains("glass")
            && name.starts_with("light_")
            && layout
                .lights
                .iter()
                .zip(&car.lights)
                .any(|((joints, _), smashed)| *smashed && joints.iter().any(|j| j == name));
        if (glass == Glass::Cracked || light) && original.is_none() {
            let base = material.0.clone();
            let texture = if light { &images.1 } else { &images.0 };
            if *texture == Handle::default() {
                continue;
            }
            let cracked = damage
                .cracked
                .entry((base.id(), light))
                .or_insert_with(|| {
                    let mut m = materials.get(&base).cloned().unwrap_or_default();
                    m.base_color_texture = Some(texture.clone());
                    materials.add(m)
                })
                .clone();
            material.0 = cracked;
            commands.entity(entity).insert(Original(base));
        }
    }
    // AUTHORED glass shards from the lac.ppts points of a smashed window.
    if !new_shards.is_empty() {
        let (mesh, material) = damage
            .shard_assets
            .get_or_insert_with(|| {
                (
                    meshes.add(Triangle3d::new(
                        Vec3::new(0., 0.03, 0.),
                        Vec3::new(-0.025, -0.02, 0.),
                        Vec3::new(0.025, -0.015, 0.),
                    )),
                    materials.add(StandardMaterial {
                        base_color: Color::srgba(0.75, 0.85, 0.9, 0.7),
                        alpha_mode: AlphaMode::Blend,
                        double_sided: true,
                        cull_mode: None,
                        ..default()
                    }),
                )
            })
            .clone();
        for (index, window) in new_shards {
            let pose = poses[index];
            let velocity = damage.cars[index].velocity.unwrap_or_default();
            for (k, (point, normal)) in damage.shards.get(window).into_iter().flatten().enumerate()
            {
                let spread = ((k * 37 % 11) as f32 / 11.) - 0.5;
                let v = velocity
                    + pose.transform_vector3(
                        *normal * (1.2 + spread) + Vec3::new(spread, 1., -spread),
                    );
                commands.spawn((
                    Name::new(format!("glass shard car{index} {window}")),
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(pose.transform_point3(*point))
                        .with_rotation(Quat::from_rotation_y(k as f32)),
                    Shard {
                        velocity: v,
                        age: 0.,
                    },
                ));
            }
            damage.cars[index].shards.push(window);
        }
    }
    // Diagnostic validation view: GRID_DAMAGE_CAMERA=car,x,y,z places the
    // camera at a car-local point looking at that car.
    if let Some((car, eye)) = damage.camera
        && let Some(pose) = poses.get(car)
    {
        **camera = Transform::from_translation(pose.transform_point3(eye))
            .looking_at(pose.transform_point3(Vec3::new(0., 0.6, 0.)), Vec3::Y);
    }
    let dt = time.delta_secs().min(0.1);
    for (entity, mut shard, mut transform) in &mut shards {
        shard.age += dt;
        if shard.age > 1.5 {
            commands.entity(entity).despawn();
            continue;
        }
        shard.velocity.y -= 9.81 * dt;
        transform.translation += shard.velocity * dt;
        transform.rotate_local_x(6. * dt);
    }
}

type PartQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Joint,
        Option<&'static crate::MovingPart>,
        Option<&'static crate::OpponentPart>,
        Option<&'static crate::DeformingPart>,
        Option<&'static crate::OpponentDeforming>,
        &'static Aabb,
        &'static mut Transform,
        &'static mut Visibility,
        &'static mut MeshMaterial3d<StandardMaterial>,
        Option<&'static Original>,
    ),
    (Without<Shard>, Without<crate::ViewerCamera>),
>;

/// Joint bounds from the player's draws (mesh bounds through each draw's rest
/// matrix) and grid-frame glass/light anchors from its source meshes.
fn layout(parts: &PartQuery) -> Layout {
    let mut bounds: BTreeMap<String, (Vec3, Vec3)> = BTreeMap::new();
    let mut grid: BTreeMap<String, (Vec3, Vec3)> = BTreeMap::new();
    for (_, joint, moving, _, deforming, _, aabb, ..) in parts {
        let Some(moving) = moving else { continue };
        let (lo, hi) = (Vec3::from(aabb.min()), Vec3::from(aabb.max()));
        let entry = bounds
            .entry(joint.0.clone())
            .or_insert((Vec3::INFINITY, Vec3::NEG_INFINITY));
        for k in 0..8 {
            let corner = Vec3::new(
                if k & 1 == 0 { lo.x } else { hi.x },
                if k & 2 == 0 { lo.y } else { hi.y },
                if k & 4 == 0 { lo.z } else { hi.z },
            );
            let p = moving.rest.transform_point3(corner);
            entry.0 = entry.0.min(p);
            entry.1 = entry.1.max(p);
        }
        if let Some(deforming) = deforming
            && let Some(bevy::mesh::VertexAttributeValues::Float32x3(points)) =
                deforming.source.attribute(Mesh::ATTRIBUTE_POSITION)
        {
            let entry = grid
                .entry(joint.0.clone())
                .or_insert((Vec3::INFINITY, Vec3::NEG_INFINITY));
            for p in points {
                let p = deforming.raw_bind.transform_point3(Vec3::from(*p));
                entry.0 = entry.0.min(p);
                entry.1 = entry.1.max(p);
            }
        }
    }
    // Window anchors [I]: four corners of the glass box across its long
    // horizontal axis, plus the centre (the original builds six anchors from
    // the window node's bounds in 9A4A20).
    let windows = WINDOWS.map(|(name, _)| {
        let (min, max) = *grid.get(name)?;
        let long_x = (max.x - min.x).abs() >= (max.z - min.z).abs();
        let swap = |a: Vec3, b: Vec3| {
            if long_x {
                Vec3::new(b.x, a.y, a.z)
            } else {
                Vec3::new(a.x, a.y, b.z)
            }
        };
        Some([min, swap(min, max), max, swap(max, min), (min + max) * 0.5])
    });
    let lights = light_records()
        .into_iter()
        .filter_map(|joints| {
            let centres: Vec<Vec3> = joints
                .iter()
                .filter_map(|j| grid.get(j))
                .map(|(a, b)| (*a + *b) * 0.5)
                .collect();
            (!centres.is_empty()).then(|| {
                let anchor = centres.iter().sum::<Vec3>() / centres.len() as f32;
                (joints, anchor)
            })
        })
        .collect();
    let bounds = bounds
        .into_iter()
        .filter(|(name, _)| PARTS.contains(&name.as_str()) || name.starts_with("window"))
        .collect();
    Layout {
        bounds,
        windows,
        lights,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> Config {
        Config {
            disabled: false,
            scalar: 4.,
            bonnet: Some(3.),
            boot: Some(2.5),
            boot_max_angle: 5.,
            exhaust: Rule {
                swing: Some(5.),
                detach: Some(6.),
            },
            exhaust_swing_multiple: 1.,
            front_doors: Some(5.),
            rear_doors: None,
            front_wings: Rule {
                swing: Some(4.),
                detach: Some(4.2),
            },
            rear_wings: Rule::default(),
            mirrors_detach: Some(5.),
            mirror_with_door: true,
            front_bumper_detach: Some(2.8),
            rear_bumper_detach: Some(2.5),
            front_bumper_side: Rule {
                swing: Some(2.),
                detach: Some(2.5),
            },
            rear_bumper_side: Rule {
                swing: Some(2.5),
                detach: Some(2.5),
            },
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
    fn frame() -> Frame {
        Frame {
            inverse: Mat4::IDENTITY,
            offset: Vec3::ZERO,
            up_y: 1.,
        }
    }
    fn hit(point: [f32; 3], normal: [f32; 3], impact: f32) -> Impact {
        Impact {
            point,
            normal,
            impact,
        }
    }

    #[test]
    fn zones_follow_the_original_cells() {
        let c = config();
        let mut z = [0.; ZONES];
        // Front centre pushed back: bonnet gets -nz * m, both front wheels m.
        classify(
            &c,
            &frame(),
            &hit([0., 0.5, 2.], [0., 0., -0.5], 100.),
            &mut z,
        );
        assert_eq!(z[BONNET], 50.);
        assert_eq!([z[WHEELS + 2], z[WHEELS + 3]], [100., 100.]);
        // Front left (+X) corner.
        let mut z = [0.; ZONES];
        classify(
            &c,
            &frame(),
            &hit([0.8, 0.5, 2.], [-1., 0., 0.], 10.),
            &mut z,
        );
        assert_eq!(z[FRONT_LEFT], 10.);
        assert_eq!([z[WHEELS + 2], z[WHEELS + 3]], [10., 2.]);
        assert_eq!(z[BONNET], 0.);
        // Middle right side pushed toward +X.
        let mut z = [0.; ZONES];
        classify(
            &c,
            &frame(),
            &hit([-0.8, 0.5, 0.], [0.5, 0., 0.], 10.),
            &mut z,
        );
        assert_eq!(z[RIGHT_SIDE], 5.);
        // Middle right pushed the wrong way adds nothing at all.
        let mut z = [0.; ZONES];
        classify(
            &c,
            &frame(),
            &hit([-0.8, 0.5, 0.], [-0.5, 0., 0.], 10.),
            &mut z,
        );
        assert!(z.iter().all(|v| *v == 0.));
        // Rear left adds rear, boot, exhaust, rear-left wing and corner.
        let mut z = [0.; ZONES];
        classify(
            &c,
            &frame(),
            &hit([0.8, 0.5, -2.], [0., 0., 1.], 10.),
            &mut z,
        );
        for k in [REAR, BOOT, EXHAUST, REAR_LEFT_WING, REAR_LEFT] {
            assert_eq!(z[k], 10.);
        }
        assert_eq!([z[WHEELS], z[WHEELS + 1]], [10., 2.]);
        // Roof only on an inverted car.
        let mut z = [0.; ZONES];
        classify(
            &c,
            &frame(),
            &hit([0., 1.3, 0.], [0., -1., 0.], 10.),
            &mut z,
        );
        assert_eq!(z[ROOF], 0.);
        let inverted = Frame {
            up_y: -1.,
            ..frame()
        };
        classify(
            &c,
            &inverted,
            &hit([0., 1.3, 0.], [0., -1., 0.], 10.),
            &mut z,
        );
        assert_eq!(z[ROOF], 10.);
    }

    #[test]
    fn decisions_use_level_times_scalar_times_mass() {
        let c = config();
        let all = |_: &str| Some(State::Attached);
        let mass = 1000.;
        let mut z = [0.; ZONES];
        z[BONNET] = 3. * 4. * mass; // equal is not above the threshold
        assert!(decide(&c, &z, &z, mass, &all).is_empty());
        z[BONNET] += 1.;
        let out = decide(&c, &z, &z, mass, &all);
        assert_eq!(out.len(), 1);
        assert_eq!((out[0].part, out[0].state), ("bonnet", State::Loose));
        // Doors: level x mass against the side zone (scalar cancels).
        let mut z = [0.; ZONES];
        z[LEFT_SIDE] = 5. * mass + 1.;
        let out = decide(&c, &z, &z, mass, &all);
        assert_eq!(out.iter().map(|x| x.part).collect::<Vec<_>>(), ["door_fl"]);
        // Front corners together detach the whole front bumper, which then
        // stops the side rules.
        let mut z = [0.; ZONES];
        z[FRONT_LEFT] = 2. * 4. * mass;
        z[FRONT_RIGHT] = 2. * 4. * mass;
        let out = decide(&c, &z, &z, mass, &all);
        let parts: Vec<_> = out.iter().map(|x| (x.part, x.state)).collect();
        assert!(parts.contains(&("bumper_fm", State::Detached)));
        assert!(parts.contains(&("bumper_fl", State::Detached)));
        assert!(!parts.contains(&("bumper_fl", State::Loose)));
        // Absent parts never change.
        let none = |_: &str| None;
        assert!(decide(&c, &z, &z, mass, &none).is_empty());
        // Detached parts never swing again.
        let detached = |_: &str| Some(State::Detached);
        assert!(decide(&c, &z, &z, mass, &detached).is_empty());
    }

    #[test]
    fn boot_needs_the_accumulated_total() {
        let c = config();
        let all = |_: &str| Some(State::Attached);
        let mut z = [0.; ZONES];
        z[BOOT] = 2.5 * 4. * 1000. + 1.;
        let total = z;
        assert!(
            !decide(&c, &z, &total, 1000., &all)
                .iter()
                .any(|x| x.part == "boot")
        );
        let mut total = z;
        total[BOOT] *= 3.;
        assert!(
            decide(&c, &z, &total, 1000., &all)
                .iter()
                .any(|x| x.part == "boot")
        );
    }

    #[test]
    fn glass_cracks_then_smashes_with_rotation() {
        let c = config();
        let rest = [
            Vec3::new(-0.5, 1., 0.5),
            Vec3::new(0.5, 1., 0.5),
            Vec3::new(0.5, 1.2, 0.8),
            Vec3::new(-0.5, 1.2, 0.8),
            Vec3::new(0., 1.1, 0.65),
        ];
        assert_eq!(glass_state(&rest, &rest, &c, false), Glass::Intact);
        let turn = |deg: f32| rest.map(|p| Quat::from_rotation_z(deg.to_radians()) * p);
        assert_eq!(glass_state(&rest, &turn(0.5), &c, false), Glass::Intact);
        assert_eq!(glass_state(&rest, &turn(1.5), &c, false), Glass::Cracked);
        assert_eq!(glass_state(&rest, &turn(5.), &c, false), Glass::Smashed);
        assert_eq!(glass_state(&rest, &turn(5.), &c, true), Glass::Cracked);
    }

    #[test]
    fn ppts_points_group_by_window() {
        let mut b = vec![0u8; 4 + 40 * 2];
        b[..4].copy_from_slice(&2u32.to_le_bytes());
        for (i, name) in ["x0_window_bl_0", "x0_window_bl_1"].iter().enumerate() {
            let o = 4 + i * 40;
            b[o..o + name.len()].copy_from_slice(name.as_bytes());
            b[o + 32..o + 36].copy_from_slice(&1u32.to_le_bytes());
            b[o + 36..o + 40].copy_from_slice(&((84 + i * 24) as u32).to_le_bytes());
        }
        for v in [1f32, 2., 3., 0., 1., 0., 4., 5., 6., 0., 0., 1.] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        let out = read_ppts(&b).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out["window_bl"].len(), 2);
        assert_eq!(out["window_bl"][1].0, Vec3::new(4., 5., 6.));
    }

    #[test]
    fn joint_names_drop_the_lod_prefix() {
        assert_eq!(Joint::new("x0_wing_mirror_l").0, "wing_mirror_l");
        assert_eq!(light_records().len(), 12);
        assert_eq!(light_records()[6][2], "light_brake_pod_bl");
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn lacetti_ned_parts_configuration() {
        let root = std::env::var("GRID_GAME_ROOT").unwrap();
        let c = Config::read(Path::new(&root)).unwrap();
        assert!(!c.disabled);
        assert!(c.bonnet.is_some() && c.front_doors.is_some());
        assert!(c.rear_doors.is_none() && c.rear_wings == Rule::default());
        assert!(c.smash_cos < c.crack_cos && c.crack_cos < 1.);
        assert!(c.front > c.rear && c.half_width > 0.5 && c.roof > 1.);
        assert!(c.boot_max_angle <= 5.);
        let shards =
            read_ppts(&std::fs::read(Path::new(&root).join("cars/lac/lac.ppts")).unwrap()).unwrap();
        assert_eq!(
            shards.keys().cloned().collect::<Vec<_>>(),
            [
                "window_b",
                "window_bl",
                "window_br",
                "window_f",
                "window_fl",
                "window_fr"
            ]
        );
    }
}
