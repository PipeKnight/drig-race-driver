//! One replay sample: the presented state of every car and of the dynamic
//! props at one simulation tick, its byte encoding and the interpolation
//! between two samples (docs/REPLAY.md).
//!
//! The original serialises each `Recordable`'s state into a frame of its
//! replay stream and, for rigid bodies, interpolates between two stored poses
//! (0x92F9E0: both matrices to quaternions, slerp, `[interpolation=%f]`). The
//! byte layout here is AUTHORED: little-endian records with full 3x4 pose
//! matrices, so a sample decodes to exactly the recorded pose. State that
//! rarely changes (wheel geometry, body deformation, broken props) is written
//! when it changes and otherwise referenced by the frame that holds it.
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

#[derive(Clone, Debug, PartialEq)]
pub struct Car {
    pub pose: Pose,
    pub velocity: Vec3,
    pub wheels: [Wheel; 4],
    pub geometry: [WheelGeometry; 4],
    pub deformation: Option<Deformation>,
    /// World frame of each wheel that came off (damage_parts lost wheels).
    pub lost: [Option<Pose>; 4],
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

/// One decoded frame before its shared items are resolved.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    pub tick: u64,
    pub cars: Vec<FrameCar>,
    pub props: Option<FrameProps>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FrameCar {
    pub pose: Pose,
    pub velocity: Vec3,
    pub wheels: [Wheel; 4],
    pub geometry: Shared<[WheelGeometry; 4]>,
    pub deformation: Option<Shared<Deformation>>,
    pub lost: [Option<Pose>; 4],
}

#[derive(Clone, Debug, PartialEq)]
pub struct FrameProps {
    pub broken: Shared<Vec<bool>>,
    pub pieces: Vec<Piece>,
}

/// What the encoder last wrote for each shared item, and where.
#[derive(Clone, Debug, Default)]
pub struct Encoder {
    geometry: Vec<Option<([WheelGeometry; 4], u32)>>,
    deformation: Vec<Option<(Deformation, u32)>>,
    broken: Option<(Vec<bool>, u32)>,
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

    /// Encodes `snapshot` as frame number `number`.
    pub fn encode(&mut self, snapshot: &Snapshot, number: u32) -> Vec<u8> {
        let mut w = Writer::default();
        w.u64(snapshot.tick);
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
            for (k, lost) in car.lost.iter().enumerate() {
                if lost.is_some() {
                    flags |= 16 << k;
                }
            }
            w.u8(flags);
            w.pose(car.pose);
            w.vec3(car.velocity);
            for wheel in &car.wheels {
                w.f32(wheel.displacement);
                w.f32(wheel.steering[0]);
                w.f32(wheel.steering[1]);
                w.f32(wheel.phase);
                w.f32(wheel.spin);
            }
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
        w.0
    }
}

/// Decodes one frame; `None` for malformed bytes.
pub fn decode(bytes: &[u8]) -> Option<Frame> {
    let mut r = Reader(bytes);
    let tick = r.u64()?;
    let count = r.u8()?;
    let mut cars = Vec::with_capacity(count.into());
    for _ in 0..count {
        let flags = r.u8()?;
        let pose = r.pose()?;
        let velocity = r.vec3()?;
        let mut wheels = [Wheel::default(); 4];
        for wheel in &mut wheels {
            *wheel = Wheel {
                displacement: r.f32()?,
                steering: [r.f32()?, r.f32()?],
                phase: r.f32()?,
                spin: r.f32()?,
            };
        }
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
            pose,
            velocity,
            wheels,
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
    r.0.is_empty().then_some(Frame { tick, cars, props })
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
    fn vec3(&mut self, v: Vec3) {
        self.f32s(&v.to_array());
    }
    fn pose(&mut self, p: Pose) {
        self.f32s(&p.0);
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
    fn vec3(&mut self) -> Option<Vec3> {
        Some(Vec3::from_array(self.f32s()?))
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

    fn resolve(frame: Frame, frames: &[Frame]) -> Snapshot {
        Snapshot {
            tick: frame.tick,
            cars: frame
                .cars
                .into_iter()
                .enumerate()
                .map(|(i, c)| Car {
                    pose: c.pose,
                    velocity: c.velocity,
                    wheels: c.wheels,
                    geometry: match c.geometry {
                        Shared::Inline(g) => g,
                        Shared::At(at) => match &frames[at as usize].cars[i].geometry {
                            Shared::Inline(g) => *g,
                            Shared::At(_) => panic!("reference to a reference"),
                        },
                    },
                    deformation: c.deformation.map(|d| match d {
                        Shared::Inline(d) => d,
                        Shared::At(at) => match &frames[at as usize].cars[i].deformation {
                            Some(Shared::Inline(d)) => d.clone(),
                            _ => panic!("bad deformation reference"),
                        },
                    }),
                    lost: c.lost,
                })
                .collect(),
            props: frame.props.map(|p| Props {
                broken: match p.broken {
                    Shared::Inline(b) => b,
                    Shared::At(at) => match &frames[at as usize].props.as_ref().unwrap().broken {
                        Shared::Inline(b) => b.clone(),
                        Shared::At(_) => panic!("reference to a reference"),
                    },
                },
                pieces: p.pieces,
            }),
        }
    }

    #[test]
    fn frames_decode_to_the_recorded_state_bit_for_bit() {
        let mut encoder = Encoder::default();
        let mut frames = Vec::new();
        let mut sizes = Vec::new();
        let mut recorded = Vec::new();
        for n in 0..6u64 {
            let mut s = snapshot(n * 2);
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
            let bytes = encoder.encode(&s, n as u32);
            sizes.push(bytes.len());
            frames.push(decode(&bytes).unwrap());
            recorded.push(s);
        }
        for (n, s) in recorded.iter().enumerate() {
            assert_eq!(&resolve(frames[n].clone(), &frames), s, "frame {n}");
        }
        // Unchanged geometry, deformation and broken flags are references.
        assert!(matches!(frames[1].cars[0].geometry, Shared::At(0)));
        assert!(matches!(frames[4].cars[1].deformation, Some(Shared::At(3))));
        assert!(matches!(
            frames[5].props.as_ref().unwrap().broken,
            Shared::At(4)
        ));
        assert!(sizes[1] < sizes[0], "{sizes:?}");
        // Truncated or extended bytes are refused.
        let bytes = encoder.encode(&recorded[5], 6);
        assert!(decode(&bytes[..bytes.len() - 1]).is_none());
        let mut longer = bytes.clone();
        longer.push(0);
        assert!(decode(&longer).is_none());
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
