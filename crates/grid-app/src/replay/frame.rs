//! One replay sample: the presented state of every car and of the dynamic
//! props at one simulation tick, its byte encoding and the interpolation
//! between two samples (docs/REPLAY.md).
//!
//! The original serialises each `Recordable`'s state into a frame of its
//! replay stream and, for rigid bodies, interpolates between two stored poses
//! (0x92F9E0: both matrices to quaternions, slerp, `[interpolation=%f]`). The
//! byte layout here is AUTHORED and lossless, so a sample decodes to exactly
//! the recorded state. State that rarely changes (wheel geometry, body
//! deformation, broken props) is written when it changes and otherwise
//! referenced by the frame that holds it. The words a car changes every frame
//! (pose, velocity, wheels, effect inputs) are predicted from the car's last
//! three frames and only the nonzero residuals are written; a key frame
//! writes them raw at least every [`KEY_INTERVAL`] frames and at the start of
//! every stream chunk (docs/REPLAY.md, "Frame encoding").
use bevy::math::{Mat3, Mat4, Quat, Vec3, Vec4};

/// A pose as recorded: the 3x4 part of a rigid transform, column-major.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose(pub [f32; 12]);

impl Pose {
    pub fn from_mat4(m: Mat4) -> Self {
        let c = m.to_cols_array();
        Self([
            c[0], c[1], c[2], c[4], c[5], c[6], c[8], c[9], c[10], c[12], c[13], c[14],
        ])
    }
    pub fn to_mat4(self) -> Mat4 {
        let p = self.0;
        Mat4::from_cols(
            Vec4::new(p[0], p[1], p[2], 0.),
            Vec4::new(p[3], p[4], p[5], 0.),
            Vec4::new(p[6], p[7], p[8], 0.),
            Vec4::new(p[9], p[10], p[11], 1.),
        )
    }
    pub fn translation(self) -> Vec3 {
        Vec3::new(self.0[9], self.0[10], self.0[11])
    }
    fn basis(self) -> Mat3 {
        let p = self.0;
        Mat3::from_cols_array(&[p[0], p[1], p[2], p[3], p[4], p[5], p[6], p[7], p[8]])
    }
    /// Translation lerp and rotation slerp; `t` 0 and 1 give the samples
    /// exactly.
    pub fn lerp(self, other: Self, t: f32) -> Self {
        if t <= 0. {
            return self;
        }
        if t >= 1. {
            return other;
        }
        let a = Quat::from_mat3(&self.basis()).normalize();
        let b = Quat::from_mat3(&other.basis()).normalize();
        let r = Mat3::from_quat(a.slerp(b, t));
        let p = self.translation().lerp(other.translation(), t);
        let mut out = Mat4::from_mat3(r);
        out.w_axis = p.extend(1.);
        Self::from_mat4(out)
    }
}

/// Per-tick wheel state (the live presentation reads it every frame).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wheel {
    pub displacement: f32,
    pub steering: [f32; 2],
    /// Rotation angle (rad) and spin (rad/s, wheel +0x104).
    pub phase: f32,
    pub spin: f32,
}

/// Wheel state that changes only with damage: pivot, axis and the wheel's
/// deformation frame (grid_tools::source_vehicle::WheelPose).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WheelGeometry {
    pub anchor: [f32; 3],
    pub axis: [f32; 3],
    pub deformation_translation: [f32; 3],
    pub deformation_orientation: [[f32; 4]; 4],
}

/// The moved nodes of a car's body deformation lattice
/// (grid_tools::body_deformation::Field::current).
#[derive(Clone, Debug, PartialEq)]
pub struct Deformation {
    pub active: bool,
    pub nodes: Vec<[f32; 4]>,
}

/// The source wheel words the ground effects read (skid marks, wheel debris,
/// tyre smoke; wheel record `body+1990+i*150`, TYRE_AUDIO_RESEARCH.md).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WheelEffects {
    /// Contact material token (`+C0`, 0 = airborne).
    pub token: u32,
    /// Radius (`+E0`) and tyre width (`+F8`).
    pub radius: f32,
    pub width: f32,
    /// Load (`+138`), smoothed excess force (`+AC`), rear term (`+148`).
    pub load: f32,
    pub excess: f32,
    pub rear_term: f32,
    /// Angular speed as stored (`+104`, not sanitised).
    pub omega: f32,
    /// Steering direction words (`+28`, `+2C`).
    pub steer: [f32; 2],
    /// Handbrake applies to this wheel (`+FC` byte).
    pub handbraked: bool,
}

/// What the ground effects read of a car besides [`Car`]'s pose, velocity
/// and wheel geometry, so a replay steps them as the race did
/// (crate::replay_effects).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Effects {
    pub angular_velocity: Vec3,
    /// Speed (`body+1648`), mass (`+15A0`), rear coefficient (`+1008`).
    pub speed: f32,
    pub mass: f32,
    pub rear_coefficient: f32,
    /// Handbrake byte (`+1640`) and open-wheel car (`+EDA8 != 0`).
    pub handbrake: bool,
    pub open_wheel: bool,
    pub wheels: [WheelEffects; 4],
}

impl Effects {
    /// Continuous values interpolate; tokens, constants and flags come from
    /// `self` (the earlier sample).
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let l = |a: f32, b: f32| a + (b - a) * t;
        Self {
            angular_velocity: self.angular_velocity.lerp(other.angular_velocity, t),
            speed: l(self.speed, other.speed),
            wheels: std::array::from_fn(|k| {
                let (a, b) = (&self.wheels[k], &other.wheels[k]);
                WheelEffects {
                    load: l(a.load, b.load),
                    excess: l(a.excess, b.excess),
                    rear_term: l(a.rear_term, b.rear_term),
                    omega: l(a.omega, b.omega),
                    steer: [l(a.steer[0], b.steer[0]), l(a.steer[1], b.steer[1])],
                    ..*a
                }
            }),
            ..*self
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Car {
    pub pose: Pose,
    pub velocity: Vec3,
    pub wheels: [Wheel; 4],
    pub geometry: [WheelGeometry; 4],
    pub deformation: Option<Deformation>,
    /// World frame of each wheel that came off (damage_parts lost wheels).
    pub lost: [Option<Pose>; 4],
    pub effects: Effects,
}

/// A live piece of a broken prop: (breakable, piece spec, body pose).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub breakable: u16,
    pub spec: u16,
    pub pose: Pose,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Props {
    pub broken: Vec<bool>,
    pub pieces: Vec<Piece>,
}

/// The complete recorded state at one tick.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub tick: u64,
    pub cars: Vec<Car>,
    pub props: Option<Props>,
}

/// Where a shared item of a decoded frame lives.
#[derive(Clone, Debug, PartialEq)]
pub enum Shared<T> {
    Inline(T),
    /// Written by the frame with this number.
    At(u32),
}

/// One frame parsed without the prediction history: its shared items, and
/// the bytes of the per-frame words that [`Decoder`] resolves.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub tick: u64,
    /// Every word is written raw (no prediction from earlier frames).
    pub key: bool,
    pub cars: Vec<FrameCar>,
    pub props: Option<FrameProps>,
    words: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FrameCar {
    /// The residual mask is the car's mask of the previous frame.
    same_mask: bool,
    pub geometry: Shared<[WheelGeometry; 4]>,
    pub deformation: Option<Shared<Deformation>>,
    pub lost: [Option<Pose>; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct FrameProps {
    pub broken: Shared<Vec<bool>>,
    pub pieces: Vec<Piece>,
}

/// The words of a car that are written every frame, in this order: pose
/// (12), velocity (3), per wheel displacement, steering pair, phase and spin
/// (20), the effect inputs' angular velocity (3), speed, mass, rear
/// coefficient, flag bits, and per wheel surface token, radius, width, load,
/// excess, rear term, omega and steer pair (36).
const WORDS: usize = 78;
type Words = [u32; WORDS];
const MASK: usize = WORDS.div_ceil(8);
/// The mask a key frame leaves: no delta mask can equal it.
const KEY_MASK: [u8; MASK] = [0xFF; MASK];
/// A key frame (every word raw) is written at least every this many frames
/// (AUTHORED): reading a frame decodes at most this many.
pub const KEY_INTERVAL: usize = 64;
const KEY_FRAME: u8 = 0;
const DELTA_FRAME: u8 = 1;

const fn spin_word(wheel: usize) -> usize {
    15 + 5 * wheel + 4
}

/// The wheel whose omega word is `word`.
const fn omega_wheel(word: usize) -> Option<usize> {
    if word >= 42 && (word - 42) % 9 == 6 {
        Some((word - 42) / 9)
    } else {
        None
    }
}

fn words(car: &Car) -> Words {
    let mut out = [0; WORDS];
    let e = &car.effects;
    let mut bits = u32::from(e.handbrake) | u32::from(e.open_wheel) << 1;
    for (k, w) in e.wheels.iter().enumerate() {
        bits |= u32::from(w.handbraked) << (2 + k);
    }
    let floats = car
        .pose
        .0
        .into_iter()
        .chain(car.velocity.to_array())
        .chain(car.wheels.iter().flat_map(|w| {
            [w.displacement, w.steering[0], w.steering[1], w.phase, w.spin]
        }))
        .chain(e.angular_velocity.to_array())
        .chain([e.speed, e.mass, e.rear_coefficient])
        .map(f32::to_bits);
    let wheels = e.wheels.iter().flat_map(|w| {
        [
            w.token,
            w.radius.to_bits(),
            w.width.to_bits(),
            w.load.to_bits(),
            w.excess.to_bits(),
            w.rear_term.to_bits(),
            w.omega.to_bits(),
            w.steer[0].to_bits(),
            w.steer[1].to_bits(),
        ]
    });
    let mut n = 0;
    for v in floats.chain([bits]).chain(wheels) {
        out[n] = v;
        n += 1;
    }
    debug_assert_eq!(n, WORDS);
    out
}

/// Pose, velocity, wheels and effect inputs from a car's words.
fn from_words(w: &Words) -> (Pose, Vec3, [Wheel; 4], Effects) {
    let f = |i: usize| f32::from_bits(w[i]);
    let bits = w[41];
    let effects = Effects {
        angular_velocity: Vec3::new(f(35), f(36), f(37)),
        speed: f(38),
        mass: f(39),
        rear_coefficient: f(40),
        handbrake: bits & 1 != 0,
        open_wheel: bits & 2 != 0,
        wheels: std::array::from_fn(|k| {
            let b = 42 + 9 * k;
            WheelEffects {
                token: w[b],
                radius: f(b + 1),
                width: f(b + 2),
                load: f(b + 3),
                excess: f(b + 4),
                rear_term: f(b + 5),
                omega: f(b + 6),
                steer: [f(b + 7), f(b + 8)],
                handbraked: bits & (4 << k) != 0,
            }
        }),
    };
    let wheels = std::array::from_fn(|k| {
        let b = 15 + 5 * k;
        Wheel {
            displacement: f(b),
            steering: [f(b + 1), f(b + 2)],
            phase: f(b + 3),
            spin: f(b + 4),
        }
    });
    (
        Pose(std::array::from_fn(f)),
        Vec3::new(f(12), f(13), f(14)),
        wheels,
        effects,
    )
}

/// Float bits to an unsigned integer of the same order, so close floats are
/// close integers; any word maps one to one.
fn order(u: u32) -> u32 {
    if u & 0x8000_0000 != 0 { !u } else { u | 0x8000_0000 }
}

fn unorder(m: u32) -> u32 {
    if m & 0x8000_0000 != 0 { m & 0x7FFF_FFFF } else { !m }
}

fn zigzag(r: u32) -> u32 {
    let s = r as i32;
    ((s << 1) ^ (s >> 31)) as u32
}

fn unzigzag(z: u32) -> u32 {
    ((z >> 1) as i32 ^ -((z & 1) as i32)) as u32
}

/// The ordered words of each car's last three frames (newest first) and
/// last residual mask, which the encoder and the decoder keep alike.
#[derive(Clone, Debug, Default)]
struct History {
    cars: Vec<[Words; 3]>,
    masks: Vec<[u8; MASK]>,
    /// Frames held, at most 3; 0 before the first key frame.
    depth: u8,
}

impl History {
    /// The prediction of word `j` of car `car`; `current` holds this frame's
    /// words before `j`.
    fn predict(&self, car: usize, j: usize, current: &Words) -> u32 {
        // Omega is the unsanitised spin: equal to the spin word but where
        // the spin was not finite.
        if let Some(k) = omega_wheel(j) {
            return current[spin_word(k)];
        }
        let h = &self.cars[car];
        let (a, b, c) = (h[0][j], h[1][j], h[2][j]);
        if self.depth >= 3 {
            // Linear extrapolation where it predicted the last word better
            // than repeating the one before.
            let linear = (a.wrapping_sub(b.wrapping_mul(2).wrapping_sub(c)) as i32).unsigned_abs();
            let repeat = (a.wrapping_sub(b) as i32).unsigned_abs();
            if linear < repeat {
                return a.wrapping_mul(2).wrapping_sub(b);
            }
        }
        a
    }

    fn push(&mut self, key: bool, words: Vec<Words>, masks: Vec<[u8; MASK]>) {
        if key || self.cars.len() != words.len() {
            self.cars = words.into_iter().map(|w| [w; 3]).collect();
            self.depth = 1;
        } else {
            for (h, w) in self.cars.iter_mut().zip(words) {
                h[2] = h[1];
                h[1] = h[0];
                h[0] = w;
            }
            self.depth = (self.depth + 1).min(3);
        }
        self.masks = masks;
    }

    /// The raw words of each car of `frame`, which follows the frame this
    /// history holds (any frame for a key frame).
    fn apply(&mut self, frame: &Frame) -> Option<Vec<Words>> {
        let n = frame.cars.len();
        let mut r = Reader(&frame.words);
        let mut ordered = Vec::with_capacity(n);
        let mut masks = Vec::with_capacity(n);
        if frame.key {
            for _ in 0..n {
                let mut w = [0; WORDS];
                for x in &mut w {
                    *x = order(r.u32()?);
                }
                ordered.push(w);
                masks.push(KEY_MASK);
            }
        } else {
            if self.depth == 0 || self.cars.len() != n {
                return None;
            }
            for (i, car) in frame.cars.iter().enumerate() {
                let mask = if car.same_mask {
                    self.masks[i]
                } else {
                    r.take::<MASK>()?
                };
                let mut w = [0; WORDS];
                for j in 0..WORDS {
                    let residual = if mask[j / 8] & (1 << (j % 8)) != 0 {
                        unzigzag(r.varint()?)
                    } else {
                        0
                    };
                    w[j] = self.predict(i, j, &w).wrapping_add(residual);
                }
                ordered.push(w);
                masks.push(mask);
            }
        }
        if !r.0.is_empty() {
            return None;
        }
        let raw = ordered.iter().map(|w| w.map(unorder)).collect();
        self.push(frame.key, ordered, masks);
        Some(raw)
    }
}

/// What the encoder last wrote for each shared item, and where, and the
/// prediction history of the per-frame words.
#[derive(Clone, Debug, Default)]
pub struct Encoder {
    geometry: Vec<Option<([WheelGeometry; 4], u32)>>,
    deformation: Vec<Option<(Deformation, u32)>>,
    broken: Option<(Vec<bool>, u32)>,
    history: History,
    /// Frames written since the last key frame.
    since_key: usize,
}

impl Encoder {
    /// Forget everything written before frame `first`, which is no longer
    /// readable; such items are written again.
    pub fn forget_before(&mut self, first: u32) {
        let keep = |at: u32| at >= first;
        for g in &mut self.geometry {
            if g.as_ref().is_some_and(|(_, at)| !keep(*at)) {
                *g = None;
            }
        }
        for d in &mut self.deformation {
            if d.as_ref().is_some_and(|(_, at)| !keep(*at)) {
                *d = None;
            }
        }
        if self.broken.as_ref().is_some_and(|(_, at)| !keep(*at)) {
            self.broken = None;
        }
    }

    /// The per-frame words of every car: raw for a key frame, otherwise each
    /// word's residual from its prediction ([`History::predict`]), zigzag
    /// LEB128, after a mask of the nonzero ones (left out when it is the
    /// car's previous mask). Returns the bytes and which cars reuse a mask.
    fn words(&mut self, cars: &[Car]) -> (bool, Vec<u8>, Vec<bool>) {
        let n = cars.len();
        let ordered: Vec<Words> = cars.iter().map(|c| words(c).map(order)).collect();
        let key = self.history.depth == 0
            || self.history.cars.len() != n
            || self.since_key + 1 >= KEY_INTERVAL;
        let mut out = Writer::default();
        let mut same = vec![false; n];
        let mut masks = Vec::with_capacity(n);
        if key {
            for w in &ordered {
                for &x in w {
                    out.u32(unorder(x));
                }
                masks.push(KEY_MASK);
            }
            self.since_key = 0;
        } else {
            for (i, w) in ordered.iter().enumerate() {
                let mut mask = [0u8; MASK];
                let mut residuals = [0u32; WORDS];
                for j in 0..WORDS {
                    let r = w[j].wrapping_sub(self.history.predict(i, j, w));
                    residuals[j] = r;
                    if r != 0 {
                        mask[j / 8] |= 1 << (j % 8);
                    }
                }
                same[i] = mask == self.history.masks[i];
                if !same[i] {
                    out.0.extend_from_slice(&mask);
                }
                for r in residuals.into_iter().filter(|&r| r != 0) {
                    out.varint(zigzag(r));
                }
                masks.push(mask);
            }
            self.since_key += 1;
        }
        self.history.push(key, ordered, masks);
        (key, out.0, same)
    }

    /// Encodes `snapshot` as frame number `number`.
    pub fn encode(&mut self, snapshot: &Snapshot, number: u32) -> Vec<u8> {
        let (key, tail, same) = self.words(&snapshot.cars);
        let mut w = Writer::default();
        w.u64(snapshot.tick);
        w.u8(if key { KEY_FRAME } else { DELTA_FRAME });
        w.u8(u8::try_from(snapshot.cars.len()).expect("at most 255 cars"));
        self.geometry.resize(snapshot.cars.len(), None);
        self.deformation.resize(snapshot.cars.len(), None);
        for (i, car) in snapshot.cars.iter().enumerate() {
            let geometry_at = match &self.geometry[i] {
                Some((g, at)) if *g == car.geometry => Some(*at),
                _ => None,
            };
            let deformation_at = match (&self.deformation[i], &car.deformation) {
                (Some((d, at)), Some(now)) if d == now => Some(*at),
                _ => None,
            };
            let mut flags = 0u8;
            if geometry_at.is_none() {
                flags |= 1;
            }
            if car.deformation.is_some() {
                flags |= 2;
                if deformation_at.is_none() {
                    flags |= 4;
                }
            }
            if same[i] {
                flags |= 8;
            }
            for (k, lost) in car.lost.iter().enumerate() {
                if lost.is_some() {
                    flags |= 16 << k;
                }
            }
            w.u8(flags);
            match geometry_at {
                Some(at) => w.u32(at),
                None => {
                    for g in &car.geometry {
                        w.f32s(&g.anchor);
                        w.f32s(&g.axis);
                        w.f32s(&g.deformation_translation);
                        for row in &g.deformation_orientation {
                            w.f32s(row);
                        }
                    }
                    self.geometry[i] = Some((car.geometry, number));
                }
            }
            if let Some(d) = &car.deformation {
                match deformation_at {
                    Some(at) => w.u32(at),
                    None => {
                        w.u8(u8::from(d.active));
                        w.u16(u16::try_from(d.nodes.len()).expect("bounded lattice"));
                        for node in &d.nodes {
                            w.f32s(node);
                        }
                        self.deformation[i] = Some((d.clone(), number));
                    }
                }
            }
            for lost in car.lost.iter().flatten() {
                w.pose(*lost);
            }
        }
        match &snapshot.props {
            None => w.u8(0),
            Some(props) => {
                let broken_at = match &self.broken {
                    Some((b, at)) if *b == props.broken => Some(*at),
                    _ => None,
                };
                w.u8(if broken_at.is_some() { 1 } else { 3 });
                match broken_at {
                    Some(at) => w.u32(at),
                    None => {
                        w.u32(u32::try_from(props.broken.len()).expect("bounded props"));
                        for chunk in props.broken.chunks(8) {
                            w.u8(chunk
                                .iter()
                                .enumerate()
                                .fold(0, |b, (k, &v)| b | (u8::from(v) << k)));
                        }
                        self.broken = Some((props.broken.clone(), number));
                    }
                }
                w.u32(u32::try_from(props.pieces.len()).expect("bounded pieces"));
                for piece in &props.pieces {
                    w.u16(piece.breakable);
                    w.u16(piece.spec);
                    w.pose(piece.pose);
                }
            }
        }
        w.0.extend_from_slice(&tail);
        w.0
    }
}

/// The frame's bytes are a key frame.
fn is_key(bytes: &[u8]) -> bool {
    bytes.get(8) == Some(&KEY_FRAME)
}

/// Parses one frame's shared items; `None` for malformed bytes. The
/// per-frame words are checked when [`Decoder`] resolves them.
pub fn decode(bytes: &[u8]) -> Option<Frame> {
    let mut r = Reader(bytes);
    let tick = r.u64()?;
    let key = match r.u8()? {
        KEY_FRAME => true,
        DELTA_FRAME => false,
        _ => return None,
    };
    let count = r.u8()?;
    let mut cars = Vec::with_capacity(count.into());
    for _ in 0..count {
        let flags = r.u8()?;
        let geometry = if flags & 1 == 0 {
            Shared::At(r.u32()?)
        } else {
            let mut out = [WheelGeometry {
                anchor: [0.; 3],
                axis: [0.; 3],
                deformation_translation: [0.; 3],
                deformation_orientation: [[0.; 4]; 4],
            }; 4];
            for g in &mut out {
                g.anchor = r.f32s()?;
                g.axis = r.f32s()?;
                g.deformation_translation = r.f32s()?;
                for row in &mut g.deformation_orientation {
                    *row = r.f32s()?;
                }
            }
            Shared::Inline(out)
        };
        let deformation = if flags & 2 == 0 {
            None
        } else if flags & 4 == 0 {
            Some(Shared::At(r.u32()?))
        } else {
            let active = r.u8()? != 0;
            let n = r.u16()?;
            let mut nodes = Vec::with_capacity(n.into());
            for _ in 0..n {
                nodes.push(r.f32s()?);
            }
            Some(Shared::Inline(Deformation { active, nodes }))
        };
        let mut lost = [None; 4];
        for (k, l) in lost.iter_mut().enumerate() {
            if flags & (16 << k) != 0 {
                *l = Some(r.pose()?);
            }
        }
        cars.push(FrameCar {
            same_mask: flags & 8 != 0,
            geometry,
            deformation,
            lost,
        });
    }
    let props = match r.u8()? {
        0 => None,
        flags => {
            let broken = if flags & 2 == 0 {
                Shared::At(r.u32()?)
            } else {
                let n = r.u32()? as usize;
                if n > 1 << 20 {
                    return None;
                }
                let mut out = Vec::with_capacity(n);
                for _ in 0..n.div_ceil(8) {
                    let b = r.u8()?;
                    for k in 0..8 {
                        if out.len() < n {
                            out.push(b & (1 << k) != 0);
                        }
                    }
                }
                Shared::Inline(out)
            };
            let n = r.u32()?;
            if n > 1 << 20 {
                return None;
            }
            let mut pieces = Vec::with_capacity(n as usize);
            for _ in 0..n {
                pieces.push(Piece {
                    breakable: r.u16()?,
                    spec: r.u16()?,
                    pose: r.pose()?,
                });
            }
            Some(FrameProps { broken, pieces })
        }
    };
    if key && r.0.len() != cars.len() * WORDS * 4 {
        return None;
    }
    Some(Frame {
        tick,
        key,
        cars,
        props,
        words: r.0.to_vec(),
    })
}

/// Resolved snapshots kept by [`Decoder`] (playback reads a frame and the
/// next one, the director's camera one ahead).
const RECENT: usize = 4;

/// Reads the frames of one stream in any order: keeps the prediction history
/// after the last frame it decoded, so playing forwards decodes each frame
/// once, and decodes any other frame from the key frame at or before it.
/// Reset it whenever frames already read are recorded again (a cleared or
/// cut stream).
#[derive(Default)]
pub struct Decoder {
    history: History,
    /// The frame `history` follows.
    at: Option<usize>,
    recent: std::collections::VecDeque<(usize, Snapshot)>,
}

impl Decoder {
    /// Frame `n` with its shared items resolved; `read` gives a frame's
    /// bytes.
    pub fn snapshot(
        &mut self,
        n: usize,
        mut read: impl FnMut(usize) -> Option<Vec<u8>>,
    ) -> Option<Snapshot> {
        if let Some((_, s)) = self.recent.iter().find(|(k, _)| *k == n) {
            return Some(s.clone());
        }
        let (frame, words) = self.words(n, &mut read)?;
        let snapshot = resolve(frame, &words, &mut read)?;
        if self.recent.len() == RECENT {
            self.recent.pop_front();
        }
        self.recent.push_back((n, snapshot.clone()));
        Some(snapshot)
    }

    /// Frame `n` and its cars' words.
    fn words(
        &mut self,
        n: usize,
        read: &mut impl FnMut(usize) -> Option<Vec<u8>>,
    ) -> Option<(Frame, Vec<Words>)> {
        let mut pending = Vec::new();
        match self.at.filter(|&a| a < n && n - a <= KEY_INTERVAL) {
            Some(a) => {
                for k in a + 1..=n {
                    pending.push((k, read(k)?));
                }
            }
            None => {
                let mut k = n;
                loop {
                    let bytes = read(k)?;
                    let key = is_key(&bytes);
                    pending.push((k, bytes));
                    if key {
                        break;
                    }
                    k = k.checked_sub(1)?;
                }
                pending.reverse();
            }
        }
        let mut last = None;
        for (k, bytes) in pending {
            self.at = None;
            let frame = decode(&bytes)?;
            let words = self.history.apply(&frame)?;
            self.at = Some(k);
            last = Some((frame, words));
        }
        last
    }
}

/// Frame `n` parsed, kept for the shared items of one snapshot.
fn held<'h>(
    held: &'h mut Vec<(u32, Frame)>,
    n: u32,
    read: &mut impl FnMut(usize) -> Option<Vec<u8>>,
) -> Option<&'h Frame> {
    let i = match held.iter().position(|(k, _)| *k == n) {
        Some(i) => i,
        None => {
            held.push((n, decode(&read(n as usize)?)?));
            held.len() - 1
        }
    };
    Some(&held[i].1)
}

/// The snapshot of a decoded frame: the shared items it refers to are read
/// from the frames that hold them.
fn resolve(
    frame: Frame,
    words: &[Words],
    read: &mut impl FnMut(usize) -> Option<Vec<u8>>,
) -> Option<Snapshot> {
    let mut frames = Vec::new();
    let mut cars = Vec::with_capacity(frame.cars.len());
    for (i, (c, w)) in frame.cars.into_iter().zip(words).enumerate() {
        let geometry = match c.geometry {
            Shared::Inline(g) => g,
            Shared::At(at) => match held(&mut frames, at, read)?.cars.get(i)?.geometry {
                Shared::Inline(g) => g,
                Shared::At(_) => return None,
            },
        };
        let deformation = match c.deformation {
            None => None,
            Some(Shared::Inline(d)) => Some(d),
            Some(Shared::At(at)) => match &held(&mut frames, at, read)?.cars.get(i)?.deformation {
                Some(Shared::Inline(d)) => Some(d.clone()),
                _ => return None,
            },
        };
        let (pose, velocity, wheels, effects) = from_words(w);
        cars.push(Car {
            pose,
            velocity,
            wheels,
            geometry,
            deformation,
            lost: c.lost,
            effects,
        });
    }
    let props = match frame.props {
        None => None,
        Some(p) => Some(Props {
            broken: match p.broken {
                Shared::Inline(b) => b,
                Shared::At(at) => match &held(&mut frames, at, read)?.props.as_ref()?.broken {
                    Shared::Inline(b) => b.clone(),
                    Shared::At(_) => return None,
                },
            },
            pieces: p.pieces,
        }),
    };
    Some(Snapshot {
        tick: frame.tick,
        cars,
        props,
    })
}

/// Snapshots further apart than this (m) are a teleport (car reset): no
/// interpolation across them (AUTHORED).
pub const TELEPORT: f32 = 20.;

fn wheel_lerp(a: &Wheel, b: &Wheel, t: f32, seconds: f32) -> Wheel {
    Wheel {
        displacement: a.displacement + (b.displacement - a.displacement) * t,
        steering: [
            a.steering[0] + (b.steering[0] - a.steering[0]) * t,
            a.steering[1] + (b.steering[1] - a.steering[1]) * t,
        ],
        // The live presentation's bounded extrapolation (wheel_presentation):
        // angle + spin x time since the sample, not a lerp of wrapped angles.
        phase: if t <= 0. { a.phase } else { a.phase + a.spin * seconds },
        spin: a.spin + (b.spin - a.spin) * t,
    }
}

/// The state between two consecutive snapshots: poses lerp/slerp, wheels
/// interpolate, everything that is stepwise comes from `a`.
pub fn interpolate(a: &Snapshot, b: &Snapshot, t: f32, seconds: f32) -> Snapshot {
    if t <= 0. || a.cars.len() != b.cars.len() {
        return a.clone();
    }
    let cars = a
        .cars
        .iter()
        .zip(&b.cars)
        .map(|(ca, cb)| {
            let jump = ca.pose.translation().distance(cb.pose.translation()) > TELEPORT;
            let s = if jump { 0. } else { t };
            Car {
                pose: ca.pose.lerp(cb.pose, s),
                velocity: ca.velocity.lerp(cb.velocity, s),
                wheels: std::array::from_fn(|k| wheel_lerp(&ca.wheels[k], &cb.wheels[k], s, seconds)),
                geometry: ca.geometry,
                deformation: ca.deformation.clone(),
                lost: std::array::from_fn(|k| match (ca.lost[k], cb.lost[k]) {
                    (Some(x), Some(y)) => Some(x.lerp(y, s)),
                    (x, _) => x,
                }),
                effects: ca.effects.lerp(&cb.effects, s),
            }
        })
        .collect();
    let props = a.props.as_ref().map(|pa| Props {
        broken: pa.broken.clone(),
        pieces: pa
            .pieces
            .iter()
            .map(|p| {
                let next = b.props.as_ref().and_then(|pb| {
                    pb.pieces
                        .iter()
                        .find(|q| (q.breakable, q.spec) == (p.breakable, p.spec))
                });
                Piece {
                    pose: next.map_or(p.pose, |q| p.pose.lerp(q.pose, t)),
                    ..*p
                }
            })
            .collect(),
    });
    Snapshot {
        tick: a.tick,
        cars,
        props,
    }
}

#[derive(Default)]
struct Writer(Vec<u8>);

impl Writer {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    fn f32s(&mut self, v: &[f32]) {
        for &x in v {
            self.f32(x);
        }
    }
    fn pose(&mut self, p: Pose) {
        self.f32s(&p.0);
    }
    /// LEB128.
    fn varint(&mut self, mut v: u32) {
        while v >= 0x80 {
            self.0.push(v as u8 | 0x80);
            v >>= 7;
        }
        self.0.push(v as u8);
    }
}

struct Reader<'a>(&'a [u8]);

impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (head, rest) = self.0.split_first_chunk::<N>()?;
        self.0 = rest;
        Some(*head)
    }
    fn u8(&mut self) -> Option<u8> {
        Some(self.take::<1>()?[0])
    }
    fn u16(&mut self) -> Option<u16> {
        Some(u16::from_le_bytes(self.take()?))
    }
    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.take()?))
    }
    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.take()?))
    }
    fn varint(&mut self) -> Option<u32> {
        let mut v = 0u32;
        for shift in [0, 7, 14, 21, 28] {
            let b = self.u8()?;
            v |= u32::from(b & 0x7F) << shift;
            if b & 0x80 == 0 {
                return Some(v);
            }
        }
        None
    }
    fn f32(&mut self) -> Option<f32> {
        Some(f32::from_bits(self.u32()?))
    }
    fn f32s<const N: usize>(&mut self) -> Option<[f32; N]> {
        let mut out = [0.; N];
        for v in &mut out {
            *v = self.f32()?;
        }
        Some(out)
    }
    fn pose(&mut self) -> Option<Pose> {
        Some(Pose(self.f32s()?))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn car(x: f32, phase: f32) -> Car {
        Car {
            pose: Pose::from_mat4(
                Mat4::from_rotation_y(x * 0.01) * Mat4::from_translation(Vec3::new(x, 1., -x)),
            ),
            velocity: Vec3::new(x, 0., 1.),
            wheels: std::array::from_fn(|k| Wheel {
                displacement: 0.01 * k as f32,
                steering: [1., 0.],
                phase,
                spin: 10.,
            }),
            geometry: [WheelGeometry {
                anchor: [0.7, 0.3, 1.2],
                axis: [0., 1., 0.],
                deformation_translation: [0.; 3],
                deformation_orientation: Mat4::IDENTITY.to_cols_array_2d(),
            }; 4],
            deformation: None,
            lost: [None; 4],
            effects: Effects {
                angular_velocity: Vec3::new(0.1, x, -0.2),
                speed: x.abs(),
                mass: 1200.,
                rear_coefficient: 0.5,
                handbrake: phase > 1.,
                open_wheel: false,
                wheels: std::array::from_fn(|k| WheelEffects {
                    token: 0x7461_6d00 + k as u32,
                    radius: 0.31,
                    width: 0.22,
                    load: 3000. + x,
                    excess: x * 10.,
                    rear_term: 0.25,
                    omega: 10. + x,
                    steer: [1., 0.],
                    handbraked: k < 2,
                }),
            },
        }
    }

    pub(crate) fn snapshot(tick: u64) -> Snapshot {
        let x = tick as f32 * 0.5;
        Snapshot {
            tick,
            cars: vec![car(x, x), car(-x, 0.)],
            props: Some(Props {
                broken: vec![false, true, false],
                pieces: vec![Piece {
                    breakable: 1,
                    spec: 0,
                    pose: Pose::from_mat4(Mat4::from_translation(Vec3::splat(x))),
                }],
            }),
        }
    }

    /// Frame `n` of an uneven drive: smooth motion with noise, a sanitised
    /// spin, a negative zero, a deformation and a lost wheel from frame 3,
    /// a broken prop from frame 4.
    fn uneven(n: u64) -> Snapshot {
        let mut s = snapshot(n * 2);
        let wobble = (n as f32 * 1.7).sin() * 0.37;
        for (i, car) in s.cars.iter_mut().enumerate() {
            car.pose.0[10] += wobble * (i + 1) as f32;
            car.velocity.y = -wobble;
            car.wheels[2].displacement = wobble * 0.01;
            car.effects.wheels[3].load += wobble * 900.;
            car.effects.wheels[0].token = if n.is_multiple_of(5) { 0 } else { 0x7461_6d00 };
        }
        if n % 3 == 1 {
            // vehicle_force stored a non-finite spin; the presented one is 0.
            s.cars[0].wheels[1].spin = 0.;
            s.cars[0].effects.wheels[1].omega = f32::NAN;
            s.cars[1].velocity.x = -0.;
        }
        if n >= 3 {
            s.cars[1].deformation = Some(Deformation {
                active: true,
                nodes: vec![[1., 2., 3., f32::from_bits(0x7f7f_ffff)]; 8],
            });
            s.cars[0].lost[2] = Some(s.cars[0].pose);
        }
        if n >= 4 {
            s.props.as_mut().unwrap().broken[0] = true;
        }
        s
    }

    fn read(frames: &[Vec<u8>]) -> impl FnMut(usize) -> Option<Vec<u8>> + '_ {
        |n| frames.get(n).cloned()
    }

    /// Snapshot bits, so NaN words compare equal.
    fn bits(s: &Snapshot) -> String {
        format!("{s:?}")
    }

    #[test]
    fn frames_decode_to_the_recorded_state_bit_for_bit() {
        let mut encoder = Encoder::default();
        let recorded: Vec<Snapshot> = (0..150).map(uneven).collect();
        let frames: Vec<Vec<u8>> = recorded
            .iter()
            .enumerate()
            .map(|(n, s)| encoder.encode(s, n as u32))
            .collect();
        // Forwards, backwards and jumping about, each from a fresh decoder.
        let forwards: Vec<usize> = (0..frames.len()).collect();
        let backwards: Vec<usize> = forwards.iter().rev().copied().collect();
        let jumps = vec![149, 3, 64, 63, 127, 128, 0, 140, 2, 2, 70];
        for order in [forwards, backwards, jumps] {
            let mut decoder = Decoder::default();
            for &n in &order {
                let s = decoder.snapshot(n, read(&frames)).unwrap();
                assert_eq!(bits(&s), bits(&recorded[n]), "frame {n}");
            }
        }
        // Key frames: the first and then every KEY_INTERVAL frames.
        let keys: Vec<usize> = (0..frames.len()).filter(|&n| is_key(&frames[n])).collect();
        assert_eq!(keys, [0, KEY_INTERVAL, 2 * KEY_INTERVAL]);
        // Unchanged geometry, deformation and broken flags are references.
        let parsed: Vec<Frame> = frames.iter().map(|b| decode(b).unwrap()).collect();
        assert!(matches!(parsed[1].cars[0].geometry, Shared::At(0)));
        assert!(matches!(parsed[4].cars[1].deformation, Some(Shared::At(3))));
        assert!(matches!(
            parsed[5].props.as_ref().unwrap().broken,
            Shared::At(4)
        ));
        // Predicted frames are smaller; a parked car reuses its mask (moving
        // floats are not linear in their bits, so their masks vary).
        assert!(frames[1].len() < frames[0].len());
        assert!(frames[10].len() * 2 < frames[0].len(), "{} {}", frames[10].len(), frames[0].len());
        let mut steady = Encoder::default();
        let masks: Vec<bool> = (0..8)
            .map(|n| decode(&steady.encode(&snapshot(10), n as u32)).unwrap().cars[1].same_mask)
            .collect();
        assert_eq!(masks, [false, false, true, true, true, true, true, true]);
        // Truncated or extended bytes are refused.
        for n in [0, 5] {
            let mut cut = frames.clone();
            cut[n].pop();
            assert!(Decoder::default().snapshot(n, read(&cut)).is_none(), "frame {n}");
            let mut longer = frames.clone();
            longer[n].push(0);
            assert!(Decoder::default().snapshot(n, read(&longer)).is_none(), "frame {n}");
        }
        // A delta frame without the frames before it is not readable; one
        // after the next key frame is (its references are kept, frames 0..4).
        let mut dropped = frames.clone();
        for f in &mut dropped[5..KEY_INTERVAL] {
            f.clear();
        }
        assert!(Decoder::default().snapshot(KEY_INTERVAL - 1, read(&dropped)).is_none());
        assert!(Decoder::default().snapshot(KEY_INTERVAL + 6, read(&dropped)).is_some());
        dropped[0].clear();
        assert!(Decoder::default().snapshot(4, read(&dropped)).is_none());
    }

    #[test]
    fn words_round_trip_and_order_floats() {
        let car = &uneven(4).cars[0];
        let (pose, velocity, wheels, effects) = from_words(&words(car));
        let rebuilt = Car { pose, velocity, wheels, effects, ..car.clone() };
        assert_eq!(words(&rebuilt), words(car));
        assert!(rebuilt.effects.wheels[1].omega.is_nan());
        for k in 0..4 {
            assert_eq!(f32::from_bits(words(car)[spin_word(k)]), car.wheels[k].spin);
            assert_eq!(omega_wheel(42 + 9 * k + 6), Some(k));
        }
        let floats = [f32::NEG_INFINITY, -2., -0.5, -0., 0., 1e-30, 0.5, 2., f32::INFINITY];
        let ordered: Vec<u32> = floats.iter().map(|f| order(f.to_bits())).collect();
        assert!(ordered.windows(2).all(|w| w[0] < w[1]), "{ordered:x?}");
        for u in [0, 1, 0x7FFF_FFFF, 0x8000_0000, 0xFFFF_FFFF, 0x7FC0_0000] {
            assert_eq!(unorder(order(u)), u);
            assert_eq!(unzigzag(zigzag(u)), u);
        }
        assert_eq!(zigzag(1), 2);
        assert_eq!(zigzag(u32::MAX), 1);
    }

    #[test]
    fn forgotten_frames_are_written_again() {
        let mut encoder = Encoder::default();
        let s = snapshot(0);
        let _ = encoder.encode(&s, 0);
        let again = decode(&encoder.encode(&s, 1)).unwrap();
        assert!(matches!(again.cars[0].geometry, Shared::At(0)));
        encoder.forget_before(1);
        let fresh = decode(&encoder.encode(&s, 2)).unwrap();
        assert!(matches!(fresh.cars[0].geometry, Shared::Inline(_)));
        assert!(matches!(
            fresh.props.unwrap().broken,
            Shared::Inline(_)
        ));
    }

    #[test]
    fn interpolation_hits_the_samples_and_slerps_between() {
        let a = snapshot(0);
        let b = snapshot(2);
        assert_eq!(interpolate(&a, &b, 0., 0.), a);
        let end = interpolate(&a, &b, 1., 0.032);
        assert_eq!(end.cars[0].pose, b.cars[0].pose);
        let mid = interpolate(&a, &b, 0.5, 0.016);
        let p = mid.cars[0].pose.translation();
        let expected = (a.cars[0].pose.translation() + b.cars[0].pose.translation()) * 0.5;
        assert!(p.distance(expected) < 1e-5);
        let rotation = Quat::from_mat3(&Mat3::from_mat4(mid.cars[0].pose.to_mat4()));
        assert!(rotation.angle_between(Quat::from_rotation_y(0.005)) < 1e-5);
        // Wheel angle: bounded extrapolation from the earlier sample.
        assert_eq!(mid.cars[0].wheels[0].phase, a.cars[0].wheels[0].phase + 10. * 0.016);
        assert_eq!(
            mid.props.unwrap().pieces[0].pose.translation(),
            Vec3::splat(0.5)
        );
        // A teleport (reset onto the line) is not swept through.
        let mut far = b.clone();
        far.cars[0].pose = Pose::from_mat4(Mat4::from_translation(Vec3::new(500., 0., 0.)));
        assert_eq!(interpolate(&a, &far, 0.5, 0.016).cars[0].pose, a.cars[0].pose);
    }
}
