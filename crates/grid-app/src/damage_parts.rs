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
use bevy::{asset::RenderAssetUsages, camera::primitives::Aabb, image::ImageSampler, prelude::*};
use std::{collections::BTreeMap, path::Path};

use grid_tools::loose_parts::{Config, EXHAUSTS, PARTS, State, WINDOWS, ZONES};
#[cfg(test)]
use grid_tools::loose_parts::Rule;

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
pub(crate) struct Original(pub(crate) Handle<StandardMaterial>);

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
#[derive(Clone, Copy, Debug)]
struct Debris {
    origin: Mat4,
    centre: Vec3,
    position: Vec3,
    velocity: Vec3,
    rotation: Quat,
    spin: Vec3,
    /// Resting height of `position`: the track surface below it plus `lift`
    /// (the launch height until a cast finds the surface).
    ground: f32,
    /// Height of `position` above the surface when the part rests.
    lift: f32,
    /// Landed and stopped: no more steps or casts.
    resting: bool,
    /// The box of a body panel; `None` (a lost wheel's fallback) flies as a
    /// point that rests `lift` above the surface.
    panel: Option<Panel>,
}

/// A detached body panel as a rigid box. The original makes every detached
/// part a body of the world [D]: 9DC270 state 4 -> 9DB0E0 (density 600) ->
/// 9D9F80 -> 9A87B0, a box of the part node's model bounds rounded by
/// min(0.2, half the smallest half extent), angular damping 0.2 (mirrors
/// 0.5), air resistance 0.01 x mass, stationary thresholds 0.1 and 0.1,
/// turnOffTime 1 s (docs/VISUAL_DAMAGE_PARTS_RESEARCH.md, "Detached parts as
/// bodies"). Here the box meets only the track below it (AUTHORED contact
/// solver: corner impulses with friction, no cars or props), and it sleeps
/// only lying on its broadest face: a panel is a thin shell that does not
/// stand on an edge, so off that face it is tipped over its support edge
/// (AUTHORED, owner's report of 23 October 2026: parts froze on an edge).
#[derive(Clone, Copy, Debug)]
struct Panel {
    /// Body axes at the loss (rotation of `Debris::origin`).
    axes: Mat3,
    /// Half extents of the inner box (rounding removed) and the rounding.
    inner: Vec3,
    radius: f32,
    /// Inverse principal inertia per unit mass (box of the full extents).
    inverse_inertia: Vec3,
    /// Body axis normal to the broadest face (the thinnest extent).
    thin: usize,
    angular_damping: f32,
    /// Track normal below the panel; `Debris::ground` is its height there.
    normal: Vec3,
    /// Time lain still and flat; time in touch with the track.
    still: f32,
    touching: f32,
}

/// 9D9F80's stationary thresholds (0.1 m/s, 0.1 rad/s) and turnOffTime.
const STATIONARY: f32 = 0.1;
const TURN_OFF_TIME: f32 = 1.;
/// AUTHORED contact: friction, restitution above 1 m/s closing speed, the
/// tilt of the broadest face from the track that counts as lying flat, the
/// tipping push and its rate cap, and the time after which a panel in touch
/// with the track stops however it lies.
const FRICTION: f32 = 0.7;
const RESTITUTION: f32 = 0.2;
const FLAT_COS: f32 = 0.94; // 20 degrees
const TIP: f32 = 40.;
const TIP_RATE: f32 = 3.;
const GIVE_UP: f32 = 20.;

impl Panel {
    /// The box of model bounds `(min, max)` under the model-to-world `origin`.
    fn new(origin: Mat4, (min, max): (Vec3, Vec3), angular_damping: f32) -> Self {
        let columns = [origin.x_axis, origin.y_axis, origin.z_axis].map(|c| c.truncate());
        let scale = Vec3::from_array(columns.map(|c| c.length().max(1e-6)));
        let axes = Mat3::from_cols(
            columns[0] / scale.x,
            columns[1] / scale.y,
            columns[2] / scale.z,
        );
        let half = ((max - min) * 0.5 * scale).max(Vec3::splat(0.01));
        let radius = (half.min_element() * 0.5).min(0.2);
        let d = half * 2.;
        let inertia = Vec3::new(
            d.y * d.y + d.z * d.z,
            d.x * d.x + d.z * d.z,
            d.x * d.x + d.y * d.y,
        ) / 12.;
        let thin = if half.x <= half.y && half.x <= half.z {
            0
        } else if half.y <= half.z {
            1
        } else {
            2
        };
        Self {
            axes,
            inner: half - Vec3::splat(radius),
            radius,
            inverse_inertia: inertia.recip(),
            thin,
            angular_damping,
            normal: Vec3::Y,
            still: 0.,
            touching: 0.,
        }
    }
}

impl Debris {
    fn matrix(&self) -> Mat4 {
        Mat4::from_translation(self.position)
            * Mat4::from_quat(self.rotation)
            * Mat4::from_translation(-self.centre)
            * self.origin
    }
    /// One step of the flight. `surface` is the track height below a point
    /// (`Runtime::ground_below`): the part lands on the track where it is,
    /// not on the plane it left the car at, which on a slope or after an
    /// airborne crash hung it in the air (owner's report of 23 October 2026).
    fn step(&mut self, dt: f32, surface: &mut dyn FnMut(Vec3) -> Option<f32>) {
        if self.resting {
            return;
        }
        if self.panel.is_some() {
            return self.step_panel(dt, surface);
        }
        self.velocity.y -= 9.81 * dt;
        self.position += self.velocity * dt;
        let w = self.spin.length();
        if w > 1e-4 {
            self.rotation =
                (Quat::from_axis_angle(self.spin / w, w * dt) * self.rotation).normalize();
        }
        if let Some(y) = surface(self.position) {
            self.ground = y + self.lift;
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
                self.resting = true;
            }
        }
    }

    /// World axes of a panel's box now.
    fn axes(&self, panel: &Panel) -> Mat3 {
        Mat3::from_quat(self.rotation) * panel.axes
    }

    /// The support points of a panel's rounded box against a surface of
    /// normal `n`: each inner corner moved by the rounding against `n`,
    /// relative to the box centre.
    fn supports(&self, panel: &Panel, n: Vec3) -> [Vec3; 8] {
        let axes = self.axes(panel);
        std::array::from_fn(|i| {
            let sign = Vec3::new(
                if i & 1 == 0 { -1. } else { 1. },
                if i & 2 == 0 { -1. } else { 1. },
                if i & 4 == 0 { -1. } else { 1. },
            );
            axes * (panel.inner * sign) - n * panel.radius
        })
    }

    /// How a panel lies: the cosine between its broadest face's normal and
    /// the track normal (1 = flat) and the heights of the lowest and the
    /// fourth-lowest support point above the track below each (a face lies
    /// on the track when both are near 0; `None` without a panel or a cast).
    fn lying(&self, surface: &mut dyn FnMut(Vec3) -> Option<f32>) -> Option<(f32, f32, f32)> {
        let panel = self.panel?;
        let n = panel.normal;
        let face = self.axes(&panel).col(panel.thin).dot(n).abs();
        let mut gaps = [0f32; 8];
        for (gap, r) in gaps.iter_mut().zip(self.supports(&panel, n)) {
            let point = self.position + r;
            *gap = point.y - surface(point)?;
        }
        gaps.sort_by(f32::total_cmp);
        Some((face, gaps[0], gaps[3]))
    }

    /// The track plane below the panel from three casts (height at the
    /// centre and slopes along X and Z); a miss keeps what is known.
    fn track_plane(&mut self, surface: &mut dyn FnMut(Vec3) -> Option<f32>) {
        let Some(panel) = &mut self.panel else { return };
        let above = Vec3::Y * ((panel.inner.length() + panel.radius) + 0.1);
        let step = 0.5;
        let p = self.position + above;
        let Some(centre) = surface(p) else { return };
        self.ground = centre;
        let x = surface(p + Vec3::X * step).unwrap_or(centre);
        let z = surface(p + Vec3::Z * step).unwrap_or(centre);
        let normal = Vec3::new(0., z - centre, step).cross(Vec3::new(step, x - centre, 0.));
        panel.normal = normal.normalize_or(Vec3::Y);
    }

    /// One step of a panel's rigid box (see `Panel`).
    fn step_panel(&mut self, dt: f32, surface: &mut dyn FnMut(Vec3) -> Option<f32>) {
        let Some(damping) = self.panel.map(|p| p.angular_damping) else {
            return;
        };
        self.velocity.y -= 9.81 * dt;
        self.velocity *= 1. - 0.01 * dt;
        self.spin /= 1. + damping * dt;
        self.position += self.velocity * dt;
        let w = self.spin.length();
        if w > 1e-4 {
            self.rotation =
                (Quat::from_axis_angle(self.spin / w, w * dt) * self.rotation).normalize();
        }
        self.track_plane(surface);
        let Some(mut panel) = self.panel else { return };
        let n = panel.normal;
        let base = Vec3::new(self.position.x, self.ground, self.position.z);
        // Contacts: support points at or below the plane.
        let mut contacts = [(Vec3::ZERO, 0f32); 8];
        let mut count = 0;
        let mut deepest = 0f32;
        for r in self.supports(&panel, n) {
            let depth = (base - (self.position + r)).dot(n);
            if depth > -0.002 {
                contacts[count] = (r, depth);
                count += 1;
                deepest = deepest.max(depth);
            }
        }
        if count == 0 {
            panel.still = 0.;
            self.panel = Some(panel);
            return;
        }
        panel.touching += dt;
        self.position += n * deepest;
        let axes = self.axes(&panel);
        let inverse = axes * Mat3::from_diagonal(panel.inverse_inertia) * axes.transpose();
        let contacts = &contacts[..count];
        // Off its broadest face the panel tips over its support edge: a turn
        // about the support point that every other support rises from.
        let face = axes.col(panel.thin);
        let face = if face.dot(n) < 0. { -face } else { face };
        let flat = face.dot(n) >= FLAT_COS;
        let axis = face.cross(n).normalize_or_zero();
        if !flat && axis != Vec3::ZERO && self.spin.dot(axis) < TIP_RATE {
            let side = n.cross(axis);
            let pivot = contacts
                .iter()
                .map(|(r, _)| *r)
                .min_by(|a, b| a.dot(side).total_cmp(&b.dot(side)))
                .unwrap_or(Vec3::ZERO);
            let turn = axis * TIP * dt;
            self.spin += turn;
            self.velocity -= turn.cross(pivot);
        }
        // Sequential impulses per unit mass: normal with restitution, then
        // Coulomb friction on the accumulated normal impulse.
        let mut targets = [0f32; 8];
        for (target, (r, _)) in targets.iter_mut().zip(contacts) {
            let closing = (self.velocity + self.spin.cross(*r)).dot(n);
            *target = if closing < -1. {
                -RESTITUTION * closing
            } else {
                0.
            };
        }
        let mut normal_impulse = [0f32; 8];
        let mut friction_impulse = [Vec3::ZERO; 8];
        for _ in 0..8 {
            for (i, (r, _)) in contacts.iter().enumerate() {
                let r = *r;
                let along = |d: Vec3| 1. + d.dot((inverse * r.cross(d)).cross(r));
                let vn = (self.velocity + self.spin.cross(r)).dot(n);
                let total = (normal_impulse[i] + (targets[i] - vn) / along(n)).max(0.);
                let p = n * (total - normal_impulse[i]);
                normal_impulse[i] = total;
                self.velocity += p;
                self.spin += inverse * r.cross(p);
                let v = self.velocity + self.spin.cross(r);
                let slip = v - n * v.dot(n);
                let speed = slip.length();
                if speed > 1e-6 {
                    let t = slip / speed;
                    let mut total = friction_impulse[i] - t * (speed / along(t));
                    let limit = FRICTION * normal_impulse[i];
                    if total.length() > limit {
                        total = total.normalize_or_zero() * limit;
                    }
                    let p = total - friction_impulse[i];
                    friction_impulse[i] = total;
                    self.velocity += p;
                    self.spin += inverse * r.cross(p);
                }
            }
        }
        let still = self.velocity.length() < STATIONARY && self.spin.length() < STATIONARY;
        panel.still = if still && flat { panel.still + dt } else { 0. };
        self.panel = Some(panel);
        if panel.still >= TURN_OFF_TIME || panel.touching >= GIVE_UP {
            self.velocity = Vec3::ZERO;
            self.spin = Vec3::ZERO;
            self.resting = true;
            // Never below the track where the plane fit misses a kerb or dip.
            let mut lift = 0f32;
            for r in self.supports(&panel, n) {
                let point = self.position + r;
                if let Some(y) = surface(point) {
                    lift = lift.max(y - point.y);
                }
            }
            self.position.y += lift;
        }
    }
}

#[derive(Clone, Default)]
struct Part {
    state: State,
    angle: f32,
    rate: f32,
    debris: Option<Debris>,
    /// A detached part's world body: pose at the loss and now (loose_panel).
    body: Option<(Mat4, Mat4)>,
}

#[derive(Clone, Default)]
struct CarDamage {
    totals: [f32; ZONES],
    parts: BTreeMap<&'static str, Part>,
    windows: [Glass; 6],
    lights: Vec<bool>,
    field_key: Option<Vec<u32>>,
    velocity: Option<Vec3>,
    acceleration: Vec3,
    shards: Vec<&'static str>,
    /// Wheels the simulation has detached (car +22E4..+22E7) and their flight.
    lost: [bool; 4],
    wheels: [Option<LostWheel>; 4],
}

/// A lost wheel: its world frame at the loss and where it is drawn now.
#[derive(Clone, Copy)]
struct LostWheel {
    origin: Mat4,
    /// The debris body's pose at the loss and now (grid_tools::loose_wheel),
    /// when the world simulates it.
    body: Option<(Mat4, Mat4)>,
    /// AUTHORED fallback flight without body geometry.
    flight: Debris,
}
impl LostWheel {
    fn matrix(&self) -> Mat4 {
        match self.body {
            Some((start, now)) => now * start.inverse() * self.origin,
            None => self.flight.matrix(),
        }
    }
}

/// The damage parts of a Flashback checkpoint (docs/FLASHBACK.md).
#[derive(Clone)]
pub(crate) struct Saved {
    cars: Vec<CarDamage>,
    last_tick: Option<u64>,
}

impl DamageParts {
    /// Read-only lamp cluster state for the optional signal presentation.
    pub(crate) fn light_smashed(&self, index: usize, joint: &str) -> bool {
        let Some(car) = self.cars.get(index) else { return false; };
        let Some(model) = self.car_model.get(index).and_then(|i|self.models.get(*i)) else {return false;};
        model.layout.as_ref().is_some_and(|layout|layout.lights.iter().enumerate().any(|(i,(joints,_))|car.lights.get(i)==Some(&true)&&joints.iter().any(|name|name==joint)))
    }
    pub(crate) fn checkpoint(&self) -> Saved {
        Saved {
            cars: self.cars.clone(),
            last_tick: self.last_tick,
        }
    }
    /// Back to a checkpoint: parts, swing and lost wheels as saved; glass and
    /// lights are drawn intact again and found anew from the restored
    /// deformation (`present`), shards are removed.
    pub(crate) fn restore(&mut self, saved: &Saved) {
        self.cars = saved.cars.clone();
        for car in &mut self.cars {
            car.windows = Default::default();
            car.lights.clear();
            car.field_key = None;
        }
        self.last_tick = saved.last_tick;
        self.reset = true;
    }

    /// Where car `car`'s wheel `wheel` is drawn once it has come off (world).
    pub(crate) fn lost_wheel(&self, car: usize, wheel: usize) -> Option<Mat4> {
        Some(self.cars.get(car)?.wheels.get(wheel)?.as_ref()?.matrix())
    }
}

/// Model-space joint bounds and grid-frame glass/light anchors.
struct Layout {
    bounds: BTreeMap<String, (Vec3, Vec3)>,
    windows: [Option<[Vec3; 5]>; 6],
    /// Original 12 light records: member joints and grid-frame anchor.
    lights: Vec<(Vec<String>, Vec3)>,
}

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

/// One field car model's description, glass points, part layout and
/// damage textures.
struct ModelDamage {
    config: Config,
    /// `<stem>.ppts` window points (model space) and outward normals per window.
    shards: BTreeMap<String, Vec<(Vec3, Vec3)>>,
    layout: Option<Layout>,
    images: Option<(Handle<Image>, Handle<Image>)>,
    /// Glass and light damage texture names, e.g. `car0/lac_glass_dm.tga`.
    textures: [String; 2],
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Presented;

#[derive(Resource)]
pub(crate) struct DamageParts {
    /// Per field model (`crate::Field::models`).
    models: Vec<ModelDamage>,
    /// Each car's model index (car 0 is the player).
    car_model: Vec<usize>,
    cars: Vec<CarDamage>,
    last_tick: Option<u64>,
    reset: bool,
    log_zones: bool,
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

/// `models` are the field's car models (the player's first) and `car_model`
/// each car's index into them.
pub(crate) fn install(
    app: &mut App,
    root: &Path,
    models: &[grid_tools::car::Car],
    car_model: &[usize],
) {
    let mut out = Vec::new();
    for (m, model) in models.iter().enumerate() {
        let config = match Config::read(root, model) {
            Ok(config) => config,
            Err(error) => {
                warn!("Visual damage parts unavailable; driving continues: {error}");
                return;
            }
        };
        let shards = std::fs::read(root.join(model.file(".ppts")))
            .map_err(|e| e.to_string())
            .and_then(|b| read_ppts(&b))
            .unwrap_or_else(|error| {
                warn!("Glass shard points unavailable: {error}");
                BTreeMap::new()
            });
        println!(
            "drive_damage_parts enabled=true{} scalar={} crack_cos={} smash_cos={} smash_light={} front={} rear={} half_width={} roof={} shard_windows={} policy=recovered-zones-authored-motion",
            // Only a non-default model is named, so default logs keep their form.
            if model.is_default() {
                String::new()
            } else {
                format!(" car={model}")
            },
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
        let prefix = crate::model_prefix(m);
        out.push(ModelDamage {
            config,
            shards,
            layout: None,
            images: None,
            textures: [
                format!("{prefix}{}", model.texture("glass_dm")),
                format!("{prefix}{}", model.texture("lights_dm")),
            ],
        });
    }
    app.insert_resource(DamageParts {
        models: out,
        car_model: car_model.to_vec(),
        cars: Vec::new(),
        last_tick: None,
        reset: false,
        log_zones: crate::settings::diagnostics().damage_zones,
        cracked: BTreeMap::new(),
        shard_assets: None,
        camera: crate::settings::diagnostics()
            .damage_camera
            .as_deref()
            .and_then(|v| {
                let v: Vec<f32> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                (v.len() == 4 && v[0] >= 0.).then(|| (v[0] as usize, Vec3::new(v[1], v[2], v[3])))
            }),
    })
    .add_systems(FixedUpdate, tick.after(crate::drive_tick))
    .add_systems(Update, present.after(crate::drive_present).in_set(Presented))
    .add_systems(Update, present_lost_wheels.after(crate::drive_present));
}

/// Per-car simulation readings for one tick.
struct Reading {
    pose: Mat4,
    mass: f32,
    velocity: Vec3,
    /// The car's parts object after the tick (9EC9B0 in the simulation).
    parts: grid_tools::loose_parts::Parts,
    /// Car +22E4..+22E7 after the tick (grid_tools::loose_parts wheel rule).
    detached: [bool; 4],
    /// Each wheel's presented world frame (pose x wheel motion) and hub.
    wheel_frames: [Mat4; 4],
    hubs: [Vec3; 4],
    /// Wheel spins +104 (rad/s).
    spins: [f32; 4],
    /// Car +C754 while a deformation impact is active (+C7A0 = 1).
    strength: Option<f32>,
}

fn reading(runtime: &grid_tools::source_vehicle::Runtime) -> Reading {
    let owner = &runtime.state.car.owner;
    let f = |o: usize| f32::from_bits(owner.vehicle[o / 4]);
    let v = runtime.velocity();
    let pose = Mat4::from_cols_array(&runtime.pose());
    let anchors = runtime.wheels().map(|w| Vec3::from_array(w.anchor));
    let motion = crate::wheel_presentation::wheels(runtime, 0.).map(crate::wheel_motion);
    let wheel_frames = motion.map(|m| pose * m);
    Reading {
        pose,
        mass: f32::from_bits(owner.physical[0x3f0 / 4]),
        velocity: Vec3::new(v[0], v[1], v[2]),
        parts: runtime.parts().clone(),
        detached: runtime.detached_wheels(),
        hubs: std::array::from_fn(|i| wheel_frames[i].transform_point3(anchors[i])),
        wheel_frames,
        spins: std::array::from_fn(|i| crate::wheel_presentation::angular_speed(runtime, i)),
        strength: (owner.vehicle[0xc7a0 / 4] == 1).then(|| f(0xc754)),
    }
}

/// 98C5C0 maps the wheel parts 0x2E..0x31 (wheel_bl, _br, _fl, _fr; wheel
/// records 0..3 in 9FA6E0) to the records 1, 0, 3, 2, so a lost wheel takes
/// the spin of the other wheel on its axle [D].
const SPIN_RECORD: [usize; 4] = [1, 0, 3, 2];

/// Launch of a lost wheel by 9DC270's state-4 path for a wheel part [D]:
/// while a deformation impact is active (car +C7A0 = 1), a kick of
/// (2.5 + 5 r/255) x clamp(+C754 - 0.25, 0.75, 4) along the normalised
/// (|r - 127.5|, r - 127.5, r - 127.5) / 637.5, and the spin about the axle
/// (part X). [I] the kick is a car-frame velocity added to the car's; the
/// original hands both to its debris-body builder 9DB0E0/9D9F80.
/// AUTHORED: the random bytes (a seeded LCG for the CRT rand). The world
/// simulates the wheel as a body (grid_tools::loose_wheel); this flight
/// (`Debris::step`) is only the fallback without body geometry.
fn launch(r: &Reading, i: usize, seed: u32) -> Debris {
    let mut state = seed;
    let mut random = || {
        state = state.wrapping_mul(0x41c6_4e6d).wrapping_add(12345);
        ((state >> 16) & 255) as f32
    };
    let mut velocity = r.velocity;
    if let Some(strength) = r.strength {
        let k = f32::from_bits(0x3acd_9a67); // 0.0015686 = 0.2 / 127.5
        let x = ((random() - 127.5) * k).abs();
        let y = (random() - 127.5) * k;
        let z = (random() - 127.5) * k;
        let magnitude =
            (random() * f32::from_bits(0x3b80_8081) * 5. + 2.5) * (strength - 0.25).clamp(0.75, 4.);
        let direction = Vec3::new(x, y, z);
        if direction.length_squared() > 0. {
            velocity += r.pose.transform_vector3(direction.normalize() * magnitude);
        }
    }
    Debris {
        origin: r.wheel_frames[i],
        centre: r.hubs[i],
        position: r.hubs[i],
        velocity,
        rotation: Quat::IDENTITY,
        spin: r.pose.x_axis.truncate() * r.spins[SPIN_RECORD[i]],
        ground: r.hubs[i].y,
        lift: 0.,
        resting: false,
        panel: None,
    }
}

/// A row-vector body pose as a column-vector transform.
fn row_pose(m: &[[f32; 4]; 4]) -> Mat4 {
    Mat4::from_cols_array(&std::array::from_fn(|i| m[i / 4][i % 4]))
}

pub(crate) fn tick(driving: Res<crate::Driving>, mut damage: ResMut<DamageParts>) {
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
        std::iter::once(reading(&driving.runtime))
            .chain(
                opponents
                    .multi
                    .cars
                    .iter()
                    .enumerate()
                    .skip(1)
                    .map(|(_, car)| reading(car)),
            )
            .collect()
    } else {
        vec![reading(&driving.runtime)]
    };
    damage.cars.resize_with(readings.len(), CarDamage::default);
    let dt = grid_tools::source_vehicle::DELTA;
    let runtime = &driving.runtime;
    let mut surface = |p: Vec3| runtime.ground_below([p.x, p.y, p.z]);
    // Simulated debris bodies of lost wheels, by (car, wheel).
    let (props, bodies) = match &driving.opponents {
        Some(o) => (o.multi.dynamic_props(), o.multi.bodies()),
        None => (driving.runtime.dynamic_props(), driving.runtime.bodies()),
    };
    let panel_bodies: BTreeMap<(usize, &str), (Mat4, Mat4)> = props
        .map(|p| p.panel_poses(bodies))
        .unwrap_or_default()
        .into_iter()
        .map(|(car, part, start, now)| ((car, part), (row_pose(&start), row_pose(&now))))
        .collect();
    let bodies: BTreeMap<(usize, usize), (Mat4, Mat4)> = props
        .map(|p| p.wheel_poses(bodies))
        .unwrap_or_default()
        .into_iter()
        .map(|(car, wheel, start, now)| ((car, wheel), (row_pose(&start), row_pose(&now))))
        .collect();
    for (index, (car, r)) in damage.cars.iter_mut().zip(&readings).enumerate() {
        for i in 0..4 {
            if r.detached[i] && !car.lost[i] {
                car.lost[i] = true;
                let seed = (ticks as u32) ^ (((index * 4 + i) as u32) << 24);
                let mut flight = launch(r, i, seed);
                // The hub rests as high above the track as it left the car.
                if let Some(y) = surface(flight.position) {
                    flight.lift = (flight.position.y - y).max(0.);
                }
                car.wheels[i] = Some(LostWheel {
                    origin: r.wheel_frames[i],
                    body: None,
                    flight,
                });
                log_line!(
                    "drive_wheel_detached car={index} tick={ticks} wheel={i} impact_strength={:?}",
                    r.strength
                );
            }
            if let Some(wheel) = &mut car.wheels[i] {
                wheel.body = bodies.get(&(index, i)).copied();
                if wheel.body.is_none() {
                    wheel.flight.step(dt, &mut surface);
                }
            }
        }
    }
    if damage.models.iter().any(|m| m.layout.is_none()) {
        return;
    }
    for (index, (car, r)) in damage.cars.iter_mut().zip(&readings).enumerate() {
        if !r.mass.is_finite() || r.mass <= 0. {
            continue;
        }
        let model = &damage.models[damage.car_model.get(index).copied().unwrap_or(0)];
        let (config, layout) = (&model.config, model.layout.as_ref().unwrap());
        let inverse = r.pose.inverse();
        if let Some(previous) = car.velocity {
            car.acceleration = inverse.transform_vector3((r.velocity - previous) / dt);
        }
        car.velocity = Some(r.velocity);
        // The simulation's part totals since the last tick (the tick's zones).
        let zones: [f32; ZONES] = std::array::from_fn(|z| r.parts.totals[z] - car.totals[z]);
        car.totals = r.parts.totals;
        if damage.log_zones && zones.iter().any(|z| *z > 0.) {
            log_line!(
                "drive_damage_zones car={index} tick={ticks} mass={} zones={zones:?}",
                r.mass
            );
        }
        for change in &r.parts.changes {
            log_line!(
                "drive_damage_part car={index} tick={ticks} part={} state={:?} zone={:.1} threshold={:.1}",
                change.part,
                change.state,
                change.zone,
                change.threshold
            );
        }
        // Follow the simulation's part states (9EC9B0 in loose_parts; a
        // Flashback can also take a part back).
        for (&name, &state) in &r.parts.states {
            let part = car.parts.entry(name).or_default();
            if part.state == state {
                continue;
            }
            match state {
                State::Attached => *part = Part::default(),
                State::Loose => part.rate = 3.,
                State::Detached => {
                    // Drawn at its world body (loose_panel); the AUTHORED
                    // flight only without one: carried along, pushed outward
                    // and up.
                    let Some(&(min, max)) = layout.bounds.get(name) else {
                        continue;
                    };
                    let centre = (min + max) * 0.5;
                    let hinged = hinge(name, (min, max), config)
                        .map_or(Mat4::IDENTITY, |h| h.matrix(part.angle));
                    let origin = r.pose * hinged;
                    let world = origin.transform_point3(centre);
                    let outward = r.pose.transform_vector3(Vec3::new(
                        centre.x.signum() * 1.5,
                        2.5,
                        centre.z.signum() * 1.0,
                    ));
                    let seed = name.len() as f32;
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
                        ground: r.pose.w_axis.y,
                        lift: 0.,
                        resting: false,
                        // 9D9F80: angular damping 0.5 for the mirrors (0x58, 0x59).
                        panel: Some(Panel::new(
                            origin,
                            (min, max),
                            if name.starts_with("wing_mirror") {
                                0.5
                            } else {
                                0.2
                            },
                        )),
                    });
                }
            }
            part.state = state;
        }
        // AUTHORED swing: damped spring around a half-open rest angle, driven
        // by the car's inertial acceleration along the opening direction.
        for (name, part) in &mut car.parts {
            match part.state {
                State::Loose => {
                    let Some(h) = layout
                        .bounds
                        .get(*name)
                        .and_then(|b| hinge(name, *b, config))
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
                    part.body = panel_bodies.get(&(index, *name)).copied();
                    if part.body.is_some() {
                        continue;
                    }
                    if let Some(debris) = &mut part.debris {
                        let flying = !debris.resting;
                        debris.step(dt, &mut surface);
                        if flying && debris.resting {
                            let (face, low, high) = debris.lying(&mut surface).unwrap_or((
                                f32::NAN,
                                f32::NAN,
                                f32::NAN,
                            ));
                            log_line!(
                                "drive_damage_part_rest car={index} tick={ticks} part={name} face={face:.4} low={low:.3} high={high:.3} touching={:.2}",
                                debris.panel.map_or(f32::NAN, |p| p.touching)
                            );
                        }
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
    mut glass: Option<ResMut<crate::collision_sound::GlassBreaks>>,
    own: OwnCarView,
) {
    let _timer = grid_tools::perf::Timer::start(grid_tools::perf::Stage::DamageParts);
    // What the view draws of the player car (docs/CAMERA_VIEWS.md "Own car per view").
    let (cameras, replay, cockpit, interiors) = own;
    let view = crate::camera::own_car(cameras.as_deref(), replay.as_deref(), cockpit.is_some());
    let active = cockpit.as_ref().map_or(&[][..], |c| &c.active[..]);
    let damage = &mut *damage;
    for m in 0..damage.models.len() {
        if damage.models[m].layout.is_none() {
            damage.models[m].layout = Some(layout(&parts, m, &damage.car_model));
        }
    }
    if damage.reset {
        damage.reset = false;
        for (entity, .., mut visibility, material, original) in &mut parts {
            *visibility = Visibility::Inherited;
            if let (Some(original), Some(mut material)) = (original, material) {
                material.0 = original.0.clone();
                commands.entity(entity).remove::<Original>();
            }
        }
        for (entity, ..) in &shards {
            commands.entity(entity).despawn();
        }
    }
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
        let model = &damage.models[damage.car_model.get(index).copied().unwrap_or(0)];
        let (config, layout) = (&model.config, model.layout.as_ref().unwrap());
        let key = field.key();
        if car.field_key.as_ref() == Some(&key) {
            continue;
        }
        // The first look after a start or a checkpoint finds glass anew
        // without sound.
        let heard = car.field_key.is_some();
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
            let state = glass_state(rest, &now, config, config.crack_only[w]);
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
                if heard && let Some(glass) = glass.as_mut() {
                    glass.0.push((index, w, state == Glass::Smashed));
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
                if distance > config.smash_light {
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
    let images: Vec<(Handle<Image>, Handle<Image>)> = damage
        .models
        .iter_mut()
        .map(|model| {
            let textures = &model.textures;
            model
                .images
                .get_or_insert_with(|| {
                    let mut load = |name: &str| {
                        source
                            .textures
                            .get(name)
                            .and_then(|dds| {
                                crate::texture_upload::image(
                                    dds,
                                    ImageSampler::Default,
                                    RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
                                )
                                .ok()
                            })
                            .map_or_else(Handle::default, |image| images.add(image))
                    };
                    (load(&textures[0]), load(&textures[1]))
                })
                .clone()
        })
        .collect();
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
        material,
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
        let m = damage.car_model.get(index).copied().unwrap_or(0);
        let (config, layout) = (
            &damage.models[m].config,
            damage.models[m].layout.as_ref().unwrap(),
        );
        let name = joint.0.as_str();
        // Windows and mirrors ride on their door while it is loose [I].
        let carrier = match name {
            "window_fl" => Some("door_fl"),
            "window_fr" => Some("door_fr"),
            "wing_mirror_l" if config.mirror_with_door => Some("door_fl"),
            "wing_mirror_r" if config.mirror_with_door => Some("door_fr"),
            _ => None,
        };
        let own = car.parts.get(name).filter(|p| p.state != State::Attached);
        let carried = carrier
            .and_then(|door| car.parts.get(door).map(|p| (door, p)))
            .filter(|(_, p)| p.state == State::Loose && own.is_none());
        let model = match (own, carried) {
            (Some(part), _) if part.state == State::Detached => {
                part.debris.map(|d| match part.body {
                    Some((start, now)) => now * start.inverse() * d.origin,
                    None => d.matrix(),
                })
            }
            (Some(part), _) => layout
                .bounds
                .get(name)
                .and_then(|b| hinge(name, *b, config))
                .map(|h| poses[index] * h.matrix(part.angle)),
            (None, Some((door, part))) => layout
                .bounds
                .get(door)
                .and_then(|b| hinge(door, *b, config))
                .map(|h| poses[index] * h.matrix(part.angle)),
            _ => None,
        };
        if let Some(model) = model {
            *transform = Transform::from_matrix(model * rest);
        }
        let window = WINDOWS.iter().position(|(w, _)| *w == name);
        let glass = window.map_or(Glass::Intact, |w| car.windows[w]);
        let hidden = glass == Glass::Smashed
            || (index == 0 && !view.shows(interiors.contains(entity), name, active));
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
        if (glass == Glass::Cracked || light) && original.is_none()
            && let Some(mut material) = material
        {
            let base = material.0.clone();
            let texture = if light { &images[m].1 } else { &images[m].0 };
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
            let m = damage.car_model.get(index).copied().unwrap_or(0);
            for (k, (point, normal)) in damage.models[m]
                .shards
                .get(window)
                .into_iter()
                .flatten()
                .enumerate()
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

type OwnCarView<'w, 's> = (
    Option<Res<'w, crate::camera::Cameras>>,
    Option<Res<'w, crate::replay::Replay>>,
    Option<Res<'w, crate::cockpit::Cockpit>>,
    Query<'w, 's, (), With<crate::cockpit::InteriorPart>>,
);
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
        Option<&'static mut MeshMaterial3d<StandardMaterial>>,
        Option<&'static Original>,
    ),
    (With<Mesh3d>, Without<Shard>, Without<crate::ViewerCamera>),
>;

/// Joint bounds from the player's draws (mesh bounds through each draw's rest
/// matrix) and grid-frame glass/light anchors from its source meshes.
/// Field model `m`'s layout, from the player's parts (model 0) or from the
/// first opponent that drives the model.
fn layout(parts: &PartQuery, m: usize, car_model: &[usize]) -> Layout {
    let mut bounds: BTreeMap<String, (Vec3, Vec3)> = BTreeMap::new();
    let mut grid: BTreeMap<String, (Vec3, Vec3)> = BTreeMap::new();
    let first = car_model.iter().position(|&c| c == m).unwrap_or(0);
    for (_, joint, moving, opponent, deforming, opponent_deforming, aabb, ..) in parts {
        let (rest, deforming) = match (moving, opponent) {
            (Some(moving), _) if first == 0 => (moving.rest, deforming),
            (_, Some(o)) if first != 0 && o.car == first => {
                (o.rest, opponent_deforming.map(|d| &d.0))
            }
            _ => continue,
        };
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
            let p = rest.transform_point3(corner);
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

/// Draw a lost wheel (and its tyre) at its debris body instead of on the car;
/// the disc and the caliper stay (`m_calipers_come_off_with_wheels` is 0 on
/// both cars).
fn present_lost_wheels(
    damage: Res<DamageParts>,
    mut parts: Query<(&crate::MovingPart, &mut Transform), Without<crate::OpponentPart>>,
    mut opponents: Query<(&crate::OpponentPart, &mut Transform), Without<crate::MovingPart>>,
) {
    let _timer = grid_tools::perf::Timer::start(grid_tools::perf::Stage::DamageParts);
    let flight = |car: usize, wheel: Option<usize>, flies: bool| {
        let lost = damage.cars.get(car)?.wheels[wheel?].as_ref()?;
        flies.then(|| lost.matrix())
    };
    for (part, mut transform) in &mut parts {
        if let Some(matrix) = flight(0, part.wheel, part.lost_with_wheel) {
            *transform = Transform::from_matrix(matrix * part.rest);
        }
    }
    for (part, mut transform) in &mut opponents {
        if let Some(matrix) = flight(part.car, part.wheel, part.lost_with_wheel) {
            *transform = Transform::from_matrix(matrix * part.rest);
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn lamp_signal_gate_uses_source_cluster_membership_and_reset() {
        let model=ModelDamage {config:config(),shards:BTreeMap::new(),images:None,textures:Default::default(),layout:Some(Layout {
            bounds:BTreeMap::new(),windows:[None;6],lights:vec![(vec!["light_brake_pod_bl".into(),"light_brake_glass_bl".into()],Vec3::ZERO),(vec!["light_pod_fl".into()],Vec3::ZERO)]
        })};
        let mut damage=DamageParts {models:vec![model],car_model:vec![0,0],cars:vec![CarDamage {lights:vec![true,false],..Default::default()},CarDamage::default()],last_tick:None,reset:false,log_zones:false,cracked:BTreeMap::new(),shard_assets:None,camera:None};
        assert!(damage.light_smashed(0,"light_brake_pod_bl"));assert!(damage.light_smashed(0,"light_brake_glass_bl"));
        assert!(!damage.light_smashed(0,"light_pod_fl"));assert!(!damage.light_smashed(1,"light_brake_pod_bl"));assert!(!damage.light_smashed(9,"light_brake_pod_bl"));
        let saved=damage.checkpoint();damage.restore(&saved);assert!(!damage.light_smashed(0,"light_brake_pod_bl"));
    }
    use super::*;

    #[test]
    fn layout_includes_meshes_without_standard_material() {
        #[derive(Resource, Default)]
        struct Observed(BTreeMap<String, (Vec3, Vec3)>);
        fn observe(parts: PartQuery, mut output: ResMut<Observed>) {
            output.0 = layout(&parts, 0, &[0]).bounds;
        }
        let mut app = App::new();
        app.init_resource::<Observed>().add_systems(Update, observe);
        for (name, x, standard) in [("x0_door_fl", 2., false), ("x0_boot", 0., true)] {
            let mut entity = app.world_mut().spawn((
                Joint::new(name),
                crate::MovingPart {
                    rest: Mat4::from_translation(Vec3::new(x, 0., 0.)),
                    wheel: None,
                    lost_with_wheel: false,
                },
                Aabb::from_min_max(Vec3::splat(-1.), Vec3::ONE),
                Transform::default(),
                Visibility::Inherited,
                Mesh3d(Handle::default()),
            ));
            if standard {
                entity.insert(MeshMaterial3d(Handle::<StandardMaterial>::default()));
            }
        }
        app.update();
        let observed = app.world().resource::<Observed>();
        assert_eq!(observed.0["door_fl"], (Vec3::new(1., -1., -1.), Vec3::new(3., 1., 1.)));
        assert_eq!(observed.0["boot"], (Vec3::splat(-1.), Vec3::ONE));
    }


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
            wheel_detach: 6.,
        }
    }
    #[test]
    fn a_lost_wheel_launches_with_the_axle_partner_spin_and_the_impact_kick() {
        let pose = Mat4::from_rotation_y(0.5) * Mat4::IDENTITY;
        let pose = Mat4::from_translation(Vec3::new(5., 1., -3.)) * pose;
        let mut r = Reading {
            pose,
            mass: 1140.,
            velocity: Vec3::new(0., 0., 20.),
            parts: Default::default(),
            detached: [false, false, true, false],
            wheel_frames: [pose; 4],
            hubs: [Vec3::new(1., 0.3, 2.); 4],
            spins: [10., 20., 30., 40.],
            strength: None,
        };
        // No active impact: the car's velocity and the front-right record's spin.
        let quiet = launch(&r, 2, 7);
        assert_eq!(quiet.velocity, r.velocity);
        assert_eq!(quiet.spin, pose.x_axis.truncate() * 40.);
        assert_eq!((quiet.position, quiet.ground), (r.hubs[2], 0.3));
        // An impact of strength 2: kick 2.5..7.5 x 1.75, outward along local +X.
        r.strength = Some(2.);
        let kicked = launch(&r, 2, 7);
        let local = pose
            .inverse()
            .transform_vector3(kicked.velocity - r.velocity);
        assert!(
            (2.5 * 1.75..=7.5 * 1.75).contains(&local.length()),
            "{local}"
        );
        assert!(local.x >= 0.);
        // The impact factor is clamped to 0.75..4.
        r.strength = Some(100.);
        let hard = launch(&r, 2, 7);
        let ratio = (hard.velocity - r.velocity).length() / (kicked.velocity - r.velocity).length();
        assert!((ratio - 4. / 1.75).abs() < 1e-4, "{ratio}");
    }

    /// A detached part lands on the track below it (owner's report of 23
    /// October 2026: parts asleep in the air). Launched 2 m above a road
    /// that falls 1 m per 2 m ahead, it comes to rest on the road (plus its
    /// lift), metres below the plane it left the car at, and stays there.
    #[test]
    fn a_detached_part_lands_on_the_road_below_it_not_in_the_air() {
        let road = |p: Vec3| Some(-0.5 * p.x);
        let mut part = Debris {
            origin: Mat4::IDENTITY,
            centre: Vec3::new(0., 2.3, 0.),
            position: Vec3::new(0., 2.3, 0.),
            velocity: Vec3::new(6., 2., 0.),
            rotation: Quat::IDENTITY,
            spin: Vec3::ZERO,
            ground: 2.3,
            lift: 0.3,
            resting: false,
            panel: None,
        };
        let mut surface = |p: Vec3| road(p);
        for _ in 0..1000 {
            part.step(1. / 60., &mut surface);
        }
        assert!(part.resting);
        let below = road(part.position).unwrap() + 0.3;
        assert!(
            (part.position.y - below).abs() < 0.05,
            "{part:?} rests at {below}"
        );
        assert!(part.position.y < 1., "{:?}", part.position);
        let rest = part.position;
        part.step(1. / 60., &mut surface);
        assert_eq!(part.position, rest);
        // Without a surface (cast miss) it keeps the plane it left the car at.
        let mut blind = Debris {
            resting: false,
            velocity: Vec3::ZERO,
            ..part
        };
        blind.position.y = 5.;
        blind.ground = 2.;
        for _ in 0..600 {
            blind.step(1. / 60., &mut |_| None);
        }
        assert!((blind.position.y - 2.).abs() < 1e-3, "{:?}", blind.position);
    }

    /// A panel of model bounds `half` around the origin, posed by `pose`,
    /// dropped from `height` above `road` and stepped for 20 s at 60 Hz.
    /// Returns the debris and how far its broadest face is from flat on the
    /// road (1 = flat) and its lowest support above the road (min, max).
    fn drop_panel(
        half: Vec3,
        pose: Quat,
        height: f32,
        velocity: Vec3,
        road: impl Fn(Vec3) -> f32,
    ) -> (Debris, f32, (f32, f32)) {
        let origin =
            Mat4::from_rotation_translation(pose, Vec3::new(0., road(Vec3::ZERO) + height, 0.));
        let centre = origin.transform_point3(Vec3::ZERO);
        let mut part = Debris {
            origin,
            centre,
            position: centre,
            velocity,
            rotation: Quat::IDENTITY,
            spin: Vec3::ZERO,
            ground: 0.,
            lift: 0.,
            resting: false,
            panel: Some(Panel::new(origin, (-half, half), 0.2)),
        };
        let mut surface = |p: Vec3| Some(road(p));
        for _ in 0..1200 {
            part.step(1. / 60., &mut surface);
        }
        let (face, low, high) = part.lying(&mut surface).unwrap();
        (part, face, (low, high))
    }

    /// 200 drops of panel-sized boxes in random poses, spins and speeds on a
    /// flat road and on slopes up to 12 degrees: every one comes to rest by
    /// the stationary rule (not the give-up time), flat on its broadest face,
    /// on the road and never below it.
    #[test]
    fn random_panel_drops_all_end_lying_flat_on_the_road() {
        let mut state = 1u32;
        let mut random = || {
            state = state.wrapping_mul(0x41c6_4e6d).wrapping_add(12345);
            ((state >> 8) & 0xffff) as f32 / 65535.
        };
        let shapes = [
            Vec3::new(0.7, 0.04, 0.6),  // bonnet
            Vec3::new(0.9, 0.22, 0.17), // bumper
            Vec3::new(0.05, 0.3, 0.5),  // wing
            Vec3::new(0.06, 0.45, 0.6), // door
            Vec3::new(0.1, 0.06, 0.12), // mirror
        ];
        let mut settle = Vec::new();
        for i in 0..200 {
            let half = shapes[i % shapes.len()];
            let pose = Quat::from_euler(
                EulerRot::XYZ,
                random() * 6.3,
                random() * 6.3,
                random() * 6.3,
            );
            let slope = (random() - 0.5) * 0.42;
            let velocity = Vec3::new(random() * 8. - 4., random() * 3., random() * 8. - 4.);
            let road = move |p: Vec3| slope * p.x + 0.3 * slope * p.z;
            let origin = Mat4::from_rotation_translation(pose, Vec3::new(0., 1. + random(), 0.));
            let centre = origin.transform_point3(Vec3::ZERO);
            let mut part = Debris {
                origin,
                centre,
                position: centre,
                velocity,
                rotation: Quat::IDENTITY,
                spin: Vec3::new(random() - 0.5, random() - 0.5, random() - 0.5) * 16.,
                ground: 0.,
                lift: 0.,
                resting: false,
                panel: Some(Panel::new(origin, (-half, half), 0.2)),
            };
            let mut surface = |p: Vec3| Some(road(p));
            for _ in 0..1200 {
                part.step(1. / 60., &mut surface);
            }
            let (face, low, high) = part.lying(&mut surface).unwrap();
            let touching = part.panel.unwrap().touching;
            assert!(part.resting && touching < 8., "drop {i}: {part:?}");
            assert!(face > 0.995, "drop {i}: face {face}");
            assert!(
                low > -0.01 && high < 0.03,
                "drop {i}: supports {low}..{high}"
            );
            settle.push(touching);
        }
        settle.sort_by(f32::total_cmp);
        println!(
            "settle after touchdown: median {} s, max {} s",
            settle[100], settle[199]
        );
    }

    /// Owner's report of 23 October 2026: parts froze in the pose they
    /// touched the road in. A bonnet-sized panel dropped on its edge tips over
    /// and lies on its broadest face on the road, not below it.
    #[test]
    fn a_panel_dropped_on_its_edge_lies_flat_on_the_road() {
        let bonnet = Vec3::new(0.7, 0.04, 0.6);
        let on_edge = Quat::from_rotation_z(1.4) * Quat::from_rotation_x(0.2);
        let (part, face, (low, high)) = drop_panel(bonnet, on_edge, 1.5, Vec3::ZERO, |_| 0.);
        assert!(part.resting, "{part:?}");
        assert!(face > 0.999, "face {face} {part:?}");
        assert!(low > -0.01 && low < 0.01, "lowest support {low}");
        assert!(high < 0.03, "highest support {high}");
        // A bumper box standing on its long narrow side tips onto its broadest face.
        let bumper = Vec3::new(0.9, 0.22, 0.17);
        let standing = Quat::IDENTITY;
        let (part, face, (low, high)) =
            drop_panel(bumper, standing, 0.4, Vec3::new(1., 0., 0.), |_| 0.);
        assert!(part.resting, "{part:?}");
        assert!(face > 0.999, "face {face}");
        assert!(low > -0.01 && high < 0.03, "supports {low}..{high}");
        assert!((part.position.y - 0.17).abs() < 0.02, "{:?}", part.position);
    }

    /// On a 10 degree slope a panel dropped edge-first ends flat on the slope
    /// (friction holds it), every support at or just above the road.
    #[test]
    fn a_panel_dropped_on_a_slope_lies_flat_on_the_slope() {
        let road = |p: Vec3| -0.176 * p.x + 3.;
        let wing = Vec3::new(0.05, 0.3, 0.5);
        let edge = Quat::from_rotation_y(0.7) * Quat::from_rotation_x(0.3);
        let (part, face, (low, high)) = drop_panel(wing, edge, 1.2, Vec3::new(0., 0., 2.), road);
        assert!(part.resting, "{part:?}");
        assert!(face > 0.999, "face {face}");
        assert!(low > -0.01 && high < 0.03, "supports {low}..{high}");
        assert!((part.panel.unwrap().normal.y - 0.985).abs() < 0.002);
        let rest = part.position;
        let mut part = part;
        part.step(1. / 60., &mut |p| Some(road(p)));
        assert_eq!(part.position, rest);
    }

    #[test]
    fn a_lost_wheel_is_drawn_at_its_body_and_falls_back_to_the_flight() {
        let origin = Mat4::from_translation(Vec3::new(1., 0.3, 2.)) * Mat4::from_rotation_y(0.5);
        let flight = Debris {
            origin,
            centre: Vec3::ZERO,
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            spin: Vec3::ZERO,
            ground: 0.,
            lift: 0.,
            resting: false,
            panel: None,
        };
        let start = Mat4::from_translation(Vec3::new(1., 0.3, 2.)) * Mat4::from_rotation_z(1.);
        let mut wheel = LostWheel {
            origin,
            body: Some((start, start)),
            flight,
        };
        // At the loss the body leaves the wheel where it was drawn.
        assert!(wheel.matrix().abs_diff_eq(origin, 1e-5));
        // The body's motion since the loss carries the wheel with it.
        let moved = Mat4::from_translation(Vec3::new(0., 0., 5.)) * Mat4::from_rotation_x(0.7);
        wheel.body = Some((start, moved * start));
        assert!(wheel.matrix().abs_diff_eq(moved * origin, 1e-5));
        wheel.body = None;
        assert_eq!(wheel.matrix(), flight.matrix());
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
        let c = Config::read(Path::new(&root), &grid_tools::car::Car::default()).unwrap();
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
