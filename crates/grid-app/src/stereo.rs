//! Stereo output of the mono car sources (FIDELITY_BACKLOG row 71): the game
//! hands OpenAL listener-space emitter positions and OpenAL pans them
//! (docs/AUDIO_RESEARCH.md §7.2, §12). Each source's pan gains come from
//! `grid_tools::audio_mix::pan_gains`, set by the game thread and read by
//! the audio thread every 64 frames; the gains glide over about 10 ms.
use bevy::audio::{ChannelCount, SampleRate, Source};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

/// Left/right gains shared between the game and the audio thread.
#[derive(Clone)]
pub(crate) struct Pan(Arc<AtomicU64>);

impl Pan {
    pub(crate) fn centre() -> Self {
        let pan = Self(Arc::new(AtomicU64::new(0)));
        pan.set([1., 1.]);
        pan
    }
    pub(crate) fn set(&self, gains: [f32; 2]) {
        let word = u64::from(gains[0].to_bits()) | (u64::from(gains[1].to_bits()) << 32);
        self.0.store(word, Ordering::Relaxed);
    }
    pub(crate) fn get(&self) -> [f32; 2] {
        let word = self.0.load(Ordering::Relaxed);
        [f32::from_bits(word as u32), f32::from_bits((word >> 32) as u32)]
    }
}

/// Pan gains of `emitter` for a listener at `listener` facing like a Bevy
/// camera with `rotation` (looking along -Z, +X to the right).
pub(crate) fn pan_from(rotation: bevy::math::Quat, listener: bevy::math::Vec3, emitter: bevy::math::Vec3) -> [f32; 2] {
    let rel = rotation.inverse() * (emitter - listener);
    grid_tools::audio_mix::pan_gains([rel.x, rel.y, -rel.z])
}

/// Per-frame approach of the gains to the latest pan [I: smoothing only].
pub(crate) const GLIDE: f32 = 0.002;

/// A mono decoder played as interleaved stereo with its source's pan.
pub(crate) struct Stereo<D> {
    /// The mono decoder (tests drive it directly).
    pub(crate) inner: D,
    pan: Pan,
    gains: [f32; 2],
    target: [f32; 2],
    right: Option<f32>,
    frames: usize,
    rate: u32,
}

impl<D> Stereo<D> {
    pub(crate) fn new(inner: D, pan: Pan, rate: u32) -> Self {
        let gains = pan.get();
        Self { inner, pan, gains, target: gains, right: None, frames: 0, rate }
    }
}

impl<D: Iterator<Item = f32>> Iterator for Stereo<D> {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if let Some(right) = self.right.take() {
            return Some(right);
        }
        let value = self.inner.next()?;
        if self.frames.is_multiple_of(64) {
            self.target = self.pan.get();
        }
        self.frames = self.frames.wrapping_add(1);
        for (g, t) in self.gains.iter_mut().zip(self.target) {
            *g += (t - *g) * GLIDE;
        }
        self.right = Some(value * self.gains[1]);
        Some(value * self.gains[0])
    }
}

impl<D: Iterator<Item = f32>> Source for Stereo<D> {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(self.rate).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_frames_become_panned_stereo_pairs() {
        let pan = Pan::centre();
        let mut s = Stereo::new(std::iter::repeat(0.5f32), pan.clone(), 48_000);
        assert_eq!((s.next(), s.next()), (Some(0.5), Some(0.5)));
        pan.set([0., 2f32.sqrt()]);
        let mut last = [0.; 2];
        for _ in 0..20_000 {
            last = [s.next().unwrap(), s.next().unwrap()];
        }
        assert!(last[0].abs() < 1e-3 && (last[1] - 0.5 * 2f32.sqrt()).abs() < 1e-3, "{last:?}");
        assert_eq!(s.channels().get(), 2);
    }

    #[test]
    fn pan_follows_the_camera_orientation() {
        use bevy::math::{Quat, Vec3};
        let hard_right = [0., 2f32.sqrt()];
        let at = Vec3::new(10., 0., 10.);
        // Unrotated camera: +X is to the right.
        assert_eq!(pan_from(Quat::IDENTITY, at, at + Vec3::X * 5.), hard_right);
        // Turned 90 degrees left (looking along -X): -Z is to the right.
        let left_turn = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
        let g = pan_from(left_turn, at, at - Vec3::Z * 5.);
        assert!((g[0] - hard_right[0]).abs() < 1e-3 && (g[1] - hard_right[1]).abs() < 1e-3, "{g:?}");
        // Straight ahead and the listener's own position are centred.
        assert_eq!(pan_from(left_turn, at, at - Vec3::X * 5.).map(|v| (v * 1e3).round()), [1e3, 1e3]);
        assert_eq!(pan_from(Quat::IDENTITY, at, at), [1., 1.]);
    }
}
