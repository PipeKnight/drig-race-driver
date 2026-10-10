//! Whole four-wheel suspension preparation with an owned world-query callback.
//! Extra tyre axes and preselected dynamic-shape lists are outside this adapter.
use crate::{Error, contact_material, contact_ray, dynamics, pose, wheel_coupling, wheel_frame};

pub const WORDS: usize = 0x2600 / 4;
const CONTEXT: usize = 0x3d0;

/// Bounded diagnostic state. Address-looking words are opaque, never dereferenced.
/// Contact identities are caller-owned tokens; zero denotes an absent contact.
#[derive(Clone, Debug)]
pub struct State {
    pub words: [u32; WORDS],
}

#[derive(Clone, Copy, Debug)]
pub struct Contact {
    pub normal: [f32; 4],
    pub fraction: f32,
    pub material: contact_material::Material,
    /// Material, object and triangle/material-slot identities in that order.
    pub tokens: [u32; 3],
}

impl State {
    pub fn f(&self, offset: usize) -> f32 {
        f32::from_bits(self.words[offset / 4])
    }
    pub fn set(&mut self, offset: usize, value: f32) {
        self.words[offset / 4] = value.to_bits();
    }
    pub fn vector(&self, offset: usize) -> [f32; 4] {
        std::array::from_fn(|i| self.f(offset + i * 4))
    }
    pub fn put(&mut self, offset: usize, vector: [f32; 4]) {
        for (i, value) in vector.into_iter().enumerate() {
            self.set(offset + i * 4, value);
        }
    }
    pub fn basis(&self, offset: usize) -> [[f32; 4]; 4] {
        std::array::from_fn(|i| self.vector(offset + i * 16))
    }
    fn body(&self) -> dynamics::ImpulseBody {
        dynamics::ImpulseBody {
            mass: self.f(0x15a0),
            inertia: std::array::from_fn(|i| self.f(CONTEXT + 0x6f0 + i * 4)),
            basis: self.basis(0x15e0),
        }
    }
    fn physical(&self) -> dynamics::BodyState {
        dynamics::BodyState {
            linear: self.vector(0x15c0),
            angular: self.vector(0x15d0),
            position: self.vector(0x1610),
        }
    }
    fn frames(&mut self) -> Result<(), Error> {
        let frames = wheel_frame::prepare(
            self.basis(0x15e0),
            std::array::from_fn(|i| self.vector(0x1990 + i * 0x150)),
            std::array::from_fn(|i| self.vector(CONTEXT + i * 0x170 + 0x50)),
            std::array::from_fn(|i| self.f(CONTEXT + i * 0x170 + 0x64)),
        )?;
        for (i, f) in frames.into_iter().enumerate() {
            let c = CONTEXT + i * 0x170;
            self.put(c + 0x10, f.point);
            self.put(c + 0x20, f.anchor);
            self.put(c + 0x30, f.axis);
        }
        Ok(())
    }
}

fn transform(basis: [[f32; 4]; 4], v: [f32; 4]) -> [f32; 4] {
    std::array::from_fn(|i| {
        (v[2] * basis[2][i] + v[0] * basis[0][i]) + (v[3] * basis[3][i] + v[1] * basis[1][i])
    })
}

fn dot(a: [f32; 4], b: [f32; 4]) -> f32 {
    (a[1] * b[1] + a[0] * b[0]) + (a[3] * b[3] + a[2] * b[2])
}

/// Computes predicted contacts, restores pose, then prepares coupling and rates.
/// State commits only on success. A callback may update its own query cache;
/// callers requiring rollback of that cache must stage it separately.
pub fn apply(
    state: &mut State,
    delta: f32,
    mut query: impl FnMut(usize, contact_ray::Ray) -> Result<Option<Contact>, Error>,
) -> Result<(), Error> {
    if !delta.is_finite()
        || delta <= 0.
        || delta > 0.1
        || !(1. / delta).is_finite()
        || state.words[(CONTEXT + 0x5d8) / 4] != 4
        || state.words[(CONTEXT + 0x70c) / 4] & 255 != 0
        || state.words[(CONTEXT + 0x604) / 4] != 0
    {
        return Err(Error("unsupported suspension preparation state"));
    }
    for (offset, count) in [
        (0xb20, 32),
        (0x15a0, 1),
        (0x15a8, 3),
        (0x15c0, 24),
        (0x1650, 16),
        (0xac0, 3),
        (0x159c, 1),
        (0x1648, 1),
        (0x12ac, 1),
    ] {
        if (0..count).any(|i| !state.f(offset + i * 4).is_finite()) {
            return Err(Error("nonfinite suspension preparation input"));
        }
    }
    for i in 0..4 {
        let c = CONTEXT + i * 0x170;
        let w = 0x1990 + i * 0x150;
        if [
            0, 4, 8, 12, 0x3c, 0x40, 0x48, 0x4c, 0x5c, 0xe0, 0x110, 0x140,
        ]
        .into_iter()
        .any(|o| !state.f(w + o).is_finite())
            || [0x50, 0x54, 0x58, 0x5c, 0x6c, 0x7c, 0x80]
                .into_iter()
                .any(|o| !state.f(c + o).is_finite())
            || state.f(w + 0xe0) <= 0.
            || state.f(w + 0x48) > state.f(w + 0x4c)
        {
            return Err(Error("invalid suspension preparation wheel"));
        }
    }
    let mut s = state.clone();
    if s.words[(CONTEXT + 0x5d0) / 4] & 255 != 0 {
        let old_basis = s.basis(0xb40);
        let basis = s.basis(0x15e0);
        let old = dynamics::BodyState {
            linear: s.vector(0xb80),
            angular: s.vector(0xb90),
            position: old_basis[3],
        };
        let current = dynamics::BodyState {
            linear: s.vector(0xb20),
            angular: s.vector(0xb30),
            position: basis[3],
        };
        for i in 0..4 {
            let w = 0x1990 + i * 0x150;
            let c = CONTEXT + i * 0x170;
            let mut local = s.vector(w);
            local[3] = 1.;
            let before = transform(old_basis, local);
            let after = transform(basis, local);
            let axis = transform(basis, s.vector(c + 0x50));
            let old_axis = transform(old_basis, s.vector(c + 0x50));
            let prior_speed = dot(old_axis, dynamics::point_velocity(old, before)?);
            let speed = dot(axis, dynamics::point_velocity(current, after)?);
            let shift = dot(std::array::from_fn(|j| after[j] - before[j]), axis);
            s.set(w + 0x3c, s.f(w + 0x3c) - (speed - prior_speed));
            s.set(w + 0x40, s.f(w + 0x40) - shift);
            if s.f(c + 0x6c) != -1000. {
                s.set(c + 0x6c, s.f(c + 0x6c) - shift);
            }
        }
    }
    for i in 0..4 {
        let w = 0x1990 + i * 0x150;
        let c = CONTEXT + i * 0x170;
        s.set(w + 0x138, 0.);
        s.words[(c + 0x84) / 4] &= !255;
        for field in [0x168, 0x16c, 0x170, 0x174] {
            s.set(c + field, 0.);
        }
        let displacement = if s.f(w + 0x4c) < s.f(w + 0x40) {
            s.f(w + 0x4c)
        } else {
            s.f(w + 0x40)
        };
        s.set(c + 0x64, displacement);
        s.set(c + 0x74, s.f(w + 0x3c));
        s.set(c + 0x60, s.f(w + 0x5c));
    }
    s.frames()?;
    let initial = s.physical();
    let mut speeds = [0.; 4];
    for (i, speed) in speeds.iter_mut().enumerate() {
        let c = CONTEXT + i * 0x170;
        *speed = dot(
            s.vector(c + 0x30),
            dynamics::point_velocity(initial, s.vector(c + 0x10))?,
        );
        if !speed.is_finite() {
            return Err(Error("invalid initial wheel speed"));
        }
    }
    let inverse = 1. / delta;
    let previous_linear = s.vector(0xb20);
    let previous_angular = s.vector(0xb30);
    s.put(
        CONTEXT + 0x620,
        std::array::from_fn(|j| inverse * (initial.linear[j] - previous_linear[j])),
    );
    s.put(
        CONTEXT + 0x630,
        std::array::from_fn(|j| inverse * (initial.angular[j] - previous_angular[j])),
    );
    s.put(0x15c0, previous_linear);
    s.put(0x15d0, previous_angular);
    let previous = s.physical();
    for (i, speed) in speeds.into_iter().enumerate() {
        let c = CONTEXT + i * 0x170;
        s.set(c + 0x70, speed);
        let baseline = dot(
            s.vector(c + 0x30),
            dynamics::point_velocity(previous, s.vector(c + 0x10))?,
        );
        s.set(CONTEXT + 0x640 + i * 4, (speed - baseline) * inverse);
    }
    s.set(CONTEXT + 0x610, 0.);
    let saved_basis = s.basis(0x15e0);
    let predicted = pose::predict(saved_basis, previous, delta)?;
    for (i, row) in predicted.into_iter().enumerate() {
        s.put(0x15e0 + i * 16, row);
    }
    s.frames()?;
    let rays = contact_ray::prepare(
        predicted,
        std::array::from_fn(|i| s.vector(CONTEXT + i * 0x170 + 0x20)),
        std::array::from_fn(|i| s.vector(0x1650 + i * 16)),
        std::array::from_fn(|i| s.f(0x1990 + i * 0x150 + 0xe0)),
        std::array::from_fn(|i| s.f(0x1990 + i * 0x150 + 0x48)),
        std::array::from_fn(|i| s.f(0x1990 + i * 0x150 + 0x4c)),
    )?;
    let mut floors = [0.; 4];
    for (i, ray) in rays.into_iter().enumerate() {
        let c = CONTEXT + i * 0x170;
        let w = 0x1990 + i * 0x150;
        let hit = query(i, ray)?;
        let absent = contact_material::Material {
            time_based: false,
            wavelength: 1.,
            magnitude: 0.,
            real_proportion: 0.,
            car_proportion: 0.,
            height_offset: 0.,
        };
        let result = contact_material::resolve(contact_material::Input {
            ray,
            body_y: predicted[1],
            suspension_axis: s.vector(c + 0x30),
            query_normal: hit.map_or([0.; 4], |h| h.normal),
            fraction: hit.map_or(1., |h| h.fraction),
            hit_present: hit.is_some(),
            material: hit.map_or(absent, |h| h.material),
            wheel_ordinal: i,
            speed: s.f(0x1648),
            clock: s.f(0x159c),
            car_factor: s.f(0x12ac),
            calculate_bump: true,
            previous_bump: s.f(c + 0x7c),
            new_bump_before: s.f(c + 0x80),
            wheel_angle: s.f(w + 0x110),
            wheel_radius: s.f(w + 0xe0),
            radius_samples: std::array::from_fn(|j| {
                [s.f(0x30c + i * 40 + j * 8), s.f(0x310 + i * 40 + j * 8)]
            }),
            wheel_height_offset: s.f(w + 0x140),
        })?;
        let tokens = if result.accepted {
            hit.ok_or(Error("accepted absent contact"))?.tokens
        } else {
            [0; 3]
        };
        for (j, token) in tokens.into_iter().enumerate() {
            s.words[(w + 0xc0) / 4 + j] = token;
        }
        s.put(w + 0xd0, result.wheel_normal);
        s.put(c + 0x40, result.solver_normal);
        s.set(c + 0x80, result.new_bump);
        floors[i] = result.floor;
    }
    for (i, row) in saved_basis.into_iter().enumerate() {
        s.put(0x15e0 + i * 16, row);
    }
    // A15580 saves this otherwise opaque word through x87 FLD/FSTP. An
    // authored signaling-NaN padding control exposed quieting on restore.
    let spare = s.words[0x15a4 / 4];
    if spare & 0x7fff_ffff > 0x7f80_0000 {
        s.words[0x15a4 / 4] = spare | 0x0040_0000;
    }
    s.frames()?;
    for (i, floor) in floors.into_iter().enumerate() {
        let c = CONTEXT + i * 0x170;
        s.set(c + 0x68, s.f(c + 0x6c));
        s.set(c + 0x6c, floor);
    }
    let body = s.body();
    for (i, v) in body.inertia.into_iter().enumerate() {
        s.set(0x15a8 + i * 4, v);
    }
    let coupling = wheel_coupling::prepare(
        body,
        previous,
        std::array::from_fn(|i| {
            let c = CONTEXT + i * 0x170;
            wheel_coupling::Wheel {
                point: s.vector(c + 0x10),
                axis: s.vector(c + 0x30),
                projection_axis: s.vector(c + 0x40),
            }
        }),
    )?;
    for (i, floor) in floors.into_iter().enumerate() {
        let c = CONTEXT + i * 0x170;
        s.set(c + 0x70, coupling.speeds[i]);
        s.put(c + 0x88, coupling.response[i]);
        s.put(CONTEXT + 0x650 + i * 16, coupling.secondary[i]);
        let speed = if floor == -1000. || s.f(c + 0x68) == -1000. {
            0.
        } else {
            (s.f(c + 0x70) * delta + (floor - s.f(c + 0x68))) * inverse
        };
        s.set(c + 0x78, speed);
    }
    *state = s;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn failed_preparation_preserves_owned_state() {
        let mut state = State { words: [0; WORDS] };
        state.words[(CONTEXT + 0x5d8) / 4] = 4;
        state.set(0x15a0, 1000.);
        for i in 0..3 {
            state.set(CONTEXT + 0x6f0 + i * 4, 600.);
        }
        let basis = [
            [1., 0., 0., 0.],
            [0., 1., 0., 0.],
            [0., 0., 1., 0.],
            [0., 1., 0., 1.],
        ];
        for (i, row) in basis.into_iter().enumerate() {
            state.put(0x15e0 + i * 16, row);
        }
        for i in 0..4 {
            let c = CONTEXT + i * 0x170;
            let w = 0x1990 + i * 0x150;
            state.put(c + 0x50, [0., 1., 0., 0.]);
            state.set(w + 0x48, -0.1);
            state.set(w + 0x4c, 0.2);
            state.set(w + 0xe0, 0.3);
            state.set(c + 0x6c, -1000.);
        }
        let before = state.words;
        let mut calls = 0;
        assert!(
            apply(&mut state, 0.01, |_, _| {
                calls += 1;
                Err(Error("authored world query failure"))
            })
            .is_err()
        );
        assert_eq!(calls, 1);
        assert_eq!(before, state.words);
        assert!(
            apply(&mut state, f32::NAN, |_, _| panic!(
                "invalid delta queried world"
            ))
            .is_err()
        );
        assert_eq!(before, state.words);
    }
}
