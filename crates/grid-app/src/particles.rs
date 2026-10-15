//! Small CPU billboard particle store for ground effects. Each texture owns
//! one dynamic mesh rebuilt per frame with camera-facing quads. Independent
//! renderer; the emitter parameters come from the shipped particle systems.
use bevy::prelude::*;

/// Per-particle appearance and motion constants shared by one emitter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Look {
    /// Texture slot (one mesh per slot).
    pub slot: usize,
    /// Atlas frames laid out in a `columns` x `rows` grid; one picked per particle.
    pub frames: u32,
    pub columns: u32,
    pub rows: u32,
    /// `agescale` initial/final width, scale type code (0 = none) and
    /// `growthPercentage`, as the original particle update 8A9CC0 uses them.
    pub width: [f32; 2],
    pub scale_type: u8,
    pub growth: f32,
    /// Alpha at birth, middle and death; `in_time`/`out_time` are seconds.
    pub alpha: [f32; 3],
    pub in_time: f32,
    pub out_time: f32,
    /// Colour at birth and death.
    pub colour: [[f32; 3]; 2],
    /// Downward acceleration (m/s^2) and linear drag rate (1/s).
    pub gravity: f32,
    pub drag: f32,
    /// Fraction of vertical speed kept when hitting the ground (0 = no collision).
    pub bounce: Option<f32>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Particle {
    pub look: Look,
    pub position: Vec3,
    pub velocity: Vec3,
    pub ground: f32,
    pub age: f32,
    pub life: f32,
    /// Width factor 1 + v/100 from the scale variance (no scale type only).
    pub scale: f32,
    pub rotation: f32,
    pub spin: f32,
    pub frame: u32,
}

impl Particle {
    /// Original 8A9CC0: start -> middle over `in_time` seconds, middle held,
    /// middle -> end over the last `out_time` seconds of the varied life.
    fn alpha(&self) -> f32 {
        let [start, middle, end] = self.look.alpha;
        let (fade_in, fade_out) = (self.look.in_time, self.look.out_time);
        if self.age < fade_in {
            start + (middle - start) * self.age / fade_in
        } else if self.age <= self.life - fade_out {
            middle
        } else {
            middle + (end - middle) * ((self.age - (self.life - fade_out)) / fade_out).min(1.)
        }
    }
    /// Original 8A9CC0 (0x8AB3ED..0x8AB527). Without a scale type the width is
    /// initial x (1 + v/100) x growth^floor(25 age) and the final width is never
    /// read; with one it is lerp(initial, final, F(age/life)) without variance.
    fn width(&self) -> f32 {
        let [initial, last] = self.look.width;
        let t = (self.age / self.life).clamp(0., 1.);
        let f = match self.look.scale_type {
            0 => {
                let steps = (self.age * 25.).floor() as i32;
                return initial * self.scale * self.look.growth.powi(steps);
            }
            // The names are swapped against their exponents.
            2 => t * t,
            3 => 1. - (1. - t) * (1. - t),
            4 => t * t * t,
            5 => 1. - (1. - t).powi(3),
            _ => t,
        };
        initial + (last - initial) * f
    }
}

pub(crate) struct Store {
    pub particles: Vec<Particle>,
    pub limit: usize,
}

impl Store {
    pub fn new(limit: usize) -> Self {
        Self {
            particles: Vec::with_capacity(limit),
            limit,
        }
    }
    pub fn push(&mut self, particle: Particle) -> bool {
        if self.particles.len() >= self.limit {
            return false;
        }
        self.particles.push(particle);
        true
    }
    pub fn step(&mut self, dt: f32) {
        for p in &mut self.particles {
            p.age += dt;
            p.velocity.y -= p.look.gravity * dt;
            if p.velocity.length() > 0.01 {
                p.velocity -= p.velocity * (p.look.drag * dt).min(1.);
            }
            p.position += p.velocity * dt;
            p.rotation += p.spin * dt;
            if let Some(bounce) = p.look.bounce
                && p.position.y < p.ground
            {
                p.position.y = p.ground;
                p.velocity.y = -p.velocity.y * bounce;
                p.velocity.x *= 0.6;
                p.velocity.z *= 0.6;
            }
        }
        self.particles.retain(|p| p.age < p.life);
    }

    /// Camera-facing quads for one texture slot.
    pub fn build(&self, slot: usize, right: Vec3, up: Vec3) -> Mesh {
        let mut positions = Vec::new();
        let mut uvs = Vec::new();
        let mut colors = Vec::new();
        let mut indices = Vec::new();
        for p in self.particles.iter().filter(|p| p.look.slot == slot) {
            let half = 0.5 * p.width();
            let (s, c) = p.rotation.sin_cos();
            let x = (right * c + up * s) * half;
            let y = (up * c - right * s) * half;
            let columns = p.look.columns.max(1);
            let rows = p.look.rows.max(1);
            let (u0, v0) = (
                (p.frame % columns) as f32 / columns as f32,
                (p.frame / columns % rows) as f32 / rows as f32,
            );
            let (du, dv) = (1. / columns as f32, 1. / rows as f32);
            let base = positions.len() as u32;
            for (corner, uv) in [
                (-x - y, [u0, v0 + dv]),
                (x - y, [u0 + du, v0 + dv]),
                (x + y, [u0 + du, v0]),
                (-x + y, [u0, v0]),
            ] {
                positions.push((p.position + corner).to_array());
                uvs.push(uv);
                let t = (p.age / p.life).clamp(0., 1.);
                let [a, b] = p.look.colour;
                let c: [f32; 3] = std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t);
                colors.push([c[0], c[1], c[2], p.alpha()]);
            }
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        let normals = vec![[0., 1., 0.]; positions.len()];
        super::ground_effect_assets::effect_mesh(positions, normals, uvs, colors, indices)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn look() -> Look {
        Look {
            slot: 0,
            frames: 4,
            columns: 2,
            rows: 2,
            width: [0.2, 0.4],
            scale_type: 1,
            growth: 1.,
            alpha: [0.0, 1.0, 0.0],
            in_time: 0.25,
            out_time: 0.25,
            colour: [[1.; 3]; 2],
            gravity: 9.81,
            drag: 0.,
            bounce: Some(0.5),
        }
    }

    #[test]
    fn particles_age_fall_bounce_and_expire() {
        let mut store = Store::new(2);
        let p = Particle {
            look: look(),
            position: Vec3::new(0., 0.5, 0.),
            velocity: Vec3::ZERO,
            ground: 0.,
            age: 0.,
            life: 1.,
            scale: 1.,
            rotation: 0.,
            spin: 0.,
            frame: 3,
        };
        assert!(store.push(p) && store.push(p) && !store.push(p));
        assert!((store.particles[0].alpha() - 0.).abs() < 1e-6);
        for _ in 0..50 {
            store.step(0.01);
        }
        let q = store.particles[0];
        assert!((q.alpha() - 1.).abs() < 1e-6);
        assert!(q.position.y >= 0. && q.position.y < 0.5);
        let mesh = store.build(0, Vec3::X, Vec3::Y);
        assert_eq!(mesh.count_vertices(), 8);
        for _ in 0..60 {
            store.step(0.01);
        }
        assert!(store.particles.is_empty());
    }

    #[test]
    fn width_follows_the_original_scale_rules() {
        let mut p = Particle {
            look: Look {
                // K_Gravel Large_Stones: 0.015 -> 4.0 without a scale type.
                width: [0.015, 4.0],
                scale_type: 0,
                ..look()
            },
            position: Vec3::ZERO,
            velocity: Vec3::ZERO,
            ground: 0.,
            age: 0.,
            life: 0.8,
            scale: 1.5,
            rotation: 0.,
            spin: 0.,
            frame: 0,
        };
        for age in [0., 0.4, 0.79] {
            p.age = age;
            assert!((p.width() - 0.0225).abs() < 1e-6);
        }
        // growthPercentage is a per-1/25 s factor.
        p.look.growth = 2.;
        p.age = 0.1;
        assert!((p.width() - 0.0225 * 4.).abs() < 1e-6);
        // A scale type lerps initial -> final and ignores the variance.
        p.look.scale_type = 3;
        p.look.width = [0.25, 0.05];
        p.age = 0.4;
        assert!((p.width() - (0.25 - 0.2 * 0.75)).abs() < 1e-6);
        // Alpha times are seconds: in 0.25 s, out over the last 0.25 s.
        p.look.alpha = [0., 1., 0.];
        p.age = 0.125;
        assert!((p.alpha() - 0.5).abs() < 1e-6);
        p.age = 0.675;
        assert!((p.alpha() - 0.5).abs() < 1e-6);
    }
}
