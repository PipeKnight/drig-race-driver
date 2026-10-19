//! Continuous purchased PCM presentation using the reconstructed pair scheduler.
//! Backend resampling/bus policy and supplied outer-frame inputs are experimental.
use crate::{
    engine_audio::{Control, RATE},
    engine_audio_control::{self, CONTROLLER_BYTES, CURVE_WORDS, VehicleEngine},
    engine_audio_publication::{Environment, VOICE_BYTES},
    engine_audio_restart::PlaybackService,
    engine_audio_schedule,
};
use grid_formats::{audio_car_setup, audio_dictionary, tuning, vehicle_properties};
use std::{fs, io::Read, path::Path, sync::Arc};

pub const LAYERS: [&str; 6] = [
    "dn_en",
    "up_en",
    "up_en_load",
    "dn_ex",
    "up_ex",
    "up_ex_load",
];
pub const BLOCK: usize = 768; // Explicit presentation cadence: 16ms at48kHz.
#[derive(Clone)]
struct Layer {
    pcm: Vec<f32>,
    rate: u32,
}
pub struct Bank {
    layers: [Layer; 6],
    curves: [f32; CURVE_WORDS],
    maximum: f32,
    idle: f32,
    trims: [f32; 2],
    /// carsetup EngineBay and Exhaust emitter offsets in the car frame.
    pub emitters: [[f32; 3]; 2],
}
fn bounded(path: &Path, limit: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err("continuous engine resource bound".into());
    }
    Ok(bytes)
}
fn f(b: &[u8], at: usize) -> f32 {
    f32::from_le_bytes(b[at..at + 4].try_into().unwrap())
}
fn put(b: &mut [u8], at: usize, value: f32) {
    b[at..at + 4].copy_from_slice(&value.to_le_bytes());
}
impl Bank {
    pub fn lacetti(root: &Path) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
        Self::load(root, &crate::car::Car::default())
    }
    /// The car's own bank, selected by its `audio/carsetup.bin` row.
    pub fn load(
        root: &Path,
        car: &crate::car::Car,
    ) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
        let setup_bytes = bounded(&root.join("audio/carsetup.bin"), 4 * 1024 * 1024)?;
        let setup = audio_car_setup::read(&setup_bytes, car.stem())?;
        if setup.engine_bank != format!("eng_{}", car.stem()) {
            return Err("continuous engine bank selection".into());
        }
        let dic = bounded(&root.join("audio/CMSamples.dic"), 16 * 1024 * 1024)?;
        let dictionary = audio_dictionary::read(&dic)?;
        let source = dictionary
            .banks
            .iter()
            .find(|b| b.name == setup.engine_bank)
            .ok_or("missing continuous engine bank")?;
        if source.extension != "WIP" || source.samples.len() != 6 {
            return Err("continuous bank layout".into());
        }
        let wip = bounded(
            &root.join(format!("audio/{}.WIP", setup.engine_bank)),
            64 * 1024 * 1024,
        )?;
        let ini = bounded(&root.join(setup.engine_ini), 1024 * 1024)?;
        let curves = engine_audio_control::read_curves(std::str::from_utf8(&ini)?)?;
        let mut layers = Vec::with_capacity(6);
        for (slot, name) in LAYERS.iter().enumerate() {
            let index = source
                .samples
                .iter()
                .position(|s| &s.name == name)
                .ok_or("missing continuous engine recording")?;
            let sample = &source.samples[index];
            let bytes = source.mono_pcm16(index, &wip)?;
            let frames = if slot % 3 == 0 {
                setup.idle_frames
            } else {
                setup.throttle_frames
            };
            if sample.rate != 32000 || bytes.len() / 2 != frames as usize {
                return Err("continuous setup/PCM rate or extent mismatch".into());
            }
            let pcm = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| f32::from(i16::from_le_bytes(*b)) / 32768.)
                .collect();
            layers.push(Layer {
                pcm,
                rate: u32::from(sample.rate),
            });
        }
        let csv_bytes = bounded(&root.join(car.file(".csv")), 4 * 1024 * 1024)?;
        let csv = tuning::read(&csv_bytes)?;
        if csv.rows.len() != 1 {
            return Err("continuous source tuning row".into());
        }
        let properties = vehicle_properties::read(&csv.rows[0], 0x2008)?;
        // Standard constructor copies these properties to17A4/17A8 respectively.
        let maximum = f(properties.bytes(), 0x1e8);
        let idle = f(properties.bytes(), 0x1e0);
        if !maximum.is_finite() || !idle.is_finite() || idle <= 0. || idle >= maximum {
            return Err("continuous source RPM bounds".into());
        }
        Ok(Arc::new(Self {
            layers: layers.try_into().map_err(|_| "continuous layer count")?,
            curves,
            maximum,
            idle,
            trims: [setup.trims[1], setup.trims[2]],
            emitters: [setup.offsets[0], setup.offsets[1]],
        }))
    }
}
#[derive(Clone, Copy, Default)]
struct Cursor {
    position: f64,
    step: f64,
    gain: f32,
    active: bool,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Diagnostics {
    pub frames: u64,
    pub pair_restarts: u64,
    pub clipped_samples: u64,
    pub failed_blocks: u64,
}
pub struct Mixer {
    bank: Arc<Bank>,
    controller: [u8; CONTROLLER_BYTES],
    voices: [[u8; VOICE_BYTES]; 12],
    cursors: [Cursor; 12],
    remaining: usize,
    generation: Option<u32>,
    gain: f32,
    /// Group levels in dB (engine, exhaust) added at the backend (A23B70):
    /// group mix plus distance; `-inf` silences. Default: Normal mode, Chase
    /// camera, PC platform (`audio_mix::Table::group_db`).
    pub bus_db: [f32; 2],
    pub diagnostics: Diagnostics,
    pub failure: Option<&'static str>,
}
/// Normal/Chase/PC levels of the Engine and Exhaust groups.
pub const CHASE_BUS_DB: [f32; 2] = [-14., -7.];
impl Mixer {
    pub fn new(bank: Arc<Bank>) -> Self {
        let mut result = Self {
            bank,
            controller: [0; CONTROLLER_BYTES],
            voices: [[0; VOICE_BYTES]; 12],
            cursors: [Cursor::default(); 12],
            remaining: 0,
            generation: None,
            gain: 0.,
            bus_db: CHASE_BUS_DB,
            diagnostics: Diagnostics::default(),
            failure: None,
        };
        result.reset();
        result
    }
    fn reset(&mut self) {
        self.controller = [0; CONTROLLER_BYTES];
        self.voices = [[0; VOICE_BYTES]; 12];
        self.cursors = [Cursor::default(); 12];
        self.remaining = 0;
        self.gain = 0.;
        self.failure = None;
        for (at, value) in [
            (0, 1.),
            (0xb0, 1.),
            (0x2f8, 1.),
            (0x140, 1.),
            (0x144, 1.),
            (0x148, 1.),
        ] {
            put(&mut self.controller, at, value);
        }
        self.controller[0x14c..0x14f].fill(1);
        for slot in 0..12 {
            let layer = &self.bank.layers[slot % 6];
            put(&mut self.controller, 0x260 + slot * 4, 1.);
            self.controller[0x290 + slot * 4..0x294 + slot * 4]
                .copy_from_slice(&layer.rate.to_le_bytes());
            self.controller[0x2c0 + slot * 4..0x2c4 + slot * 4]
                .copy_from_slice(&(layer.pcm.len() as u32).to_le_bytes());
        }
        for slot in 0..6 {
            self.controller[0x154 + slot * 4..0x158 + slot * 4]
                .copy_from_slice(&(slot as u32).to_le_bytes());
        }
    }
    fn frame(&mut self, control: Control) -> Result<(), &'static str> {
        let engine = VehicleEngine {
            speed: control.normalized_rpm * self.bank.maximum,
            maximum: self.bank.maximum,
            idle: self.bank.idle,
            delta: 0.016,
        };
        engine_audio_control::update(&mut self.controller, engine, &self.bank.curves)?;
        // Outer451770 load/wobble/gear effects are not yet reconstructed.
        put(&mut self.controller, 0xcc, control.throttle);
        let environment = Environment {
            motion_210: [0.; 4],
            motion_220: [0.; 4],
            ranges: [[10., 175.]; 2],
            trims: self.bank.trims,
            treble: 0.,
            player: true,
        };
        let service = PlaybackService {
            accepted_microseconds: [u32::MAX; 2],
            cooldown: f32::from_bits(0x3e21cac0),
            resource: 1,
            groups: [1, 0],
        };
        let commands = engine_audio_schedule::publish(
            &mut self.controller,
            &mut self.voices,
            environment,
            &self.bank.curves,
            engine.delta,
            [service; 3],
        )?;
        for (slot, cursor) in self.cursors.iter_mut().enumerate() {
            let layer = &self.bank.layers[slot % 6];
            cursor.step = f64::from((layer.rate as f32 * f(&self.voices[slot], 0x18) + 0.5) as u32)
                / f64::from(RATE);
            let db = f(&self.voices[slot], 0x14);
            // Backend level (A23B70): voice + group, silent at or below the floor.
            cursor.gain = crate::audio_mix::backend_gain(db, self.bus_db[slot / 3 % 2], Some(0.));
        }
        self.diagnostics.pair_restarts += commands.len() as u64 / 2;
        for command in commands {
            let slot = command.slot as usize;
            let cursor = &mut self.cursors[slot];
            cursor.position = f64::from(command.accepted_microseconds)
                * f64::from(self.bank.layers[slot % 6].rate)
                / 1_000_000.;
            cursor.active = true;
        }
        self.diagnostics.frames += 1;
        Ok(())
    }
    /// The next sample starts a 16 ms block, which reads its control
    /// (`audio_pacing`).
    pub fn block_due(&self) -> bool {
        self.remaining == 0
    }
    pub fn sample(&mut self, control: Control) -> f32 {
        let control = Control::unpack(control.pack());
        if self.generation != Some(control.generation) {
            self.reset();
            self.generation = Some(control.generation);
        }
        let target = if control.audible && self.failure.is_none() {
            1.
        } else {
            0.
        };
        self.gain += (target - self.gain).clamp(-1. / 480., 1. / 480.);
        if self.gain == 0. {
            return 0.;
        }
        if self.remaining == 0 {
            if let Err(error) = self.frame(control) {
                self.failure = Some(error);
                self.diagnostics.failed_blocks += 1;
                self.gain = 0.;
                return 0.;
            }
            self.remaining = BLOCK;
        }
        self.remaining -= 1;
        let mut value = 0.;
        for (slot, cursor) in self.cursors.iter_mut().enumerate() {
            if !cursor.active {
                continue;
            }
            let pcm = &self.bank.layers[slot % 6].pcm;
            if cursor.position >= 0. && cursor.position < (pcm.len() - 1) as f64 {
                let index = cursor.position as usize;
                let fraction = (cursor.position - index as f64) as f32;
                value += (pcm[index] + (pcm[index + 1] - pcm[index]) * fraction) * cursor.gain;
            }
            cursor.position += cursor.step;
        }
        let value = value * self.gain;
        if value.abs() > 0.98 {
            self.diagnostics.clipped_samples += 1;
        }
        value.clamp(-0.98, 0.98)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bank() -> Arc<Bank> {
        Arc::new(Bank {
            layers: std::array::from_fn(|i| Layer {
                pcm: (0..32000)
                    .map(|n| (n as f32 * (0.015 + i as f32 * 0.001)).sin() * 0.2)
                    .collect(),
                rate: 32000,
            }),
            curves: [0.5; CURVE_WORDS],
            maximum: 800.,
            idle: 240.,
            trims: [0.; 2],
            emitters: [[0., 0., 4.], [0., 0., -4.]],
        })
    }
    #[test]
    fn continuous_state_survives_boundaries_and_reset_repeats_pcm() {
        let mut a = Mixer::new(bank());
        let mut b = Mixer::new(bank());
        let c = Control {
            normalized_rpm: 0.5,
            throttle: 0.5,
            audible: true,
            generation: 1,
        };
        let wave: Vec<_> = (0..96000).map(|_| a.sample(c)).collect();
        assert_eq!(wave, (0..96000).map(|_| b.sample(c)).collect::<Vec<_>>());
        assert!(wave.iter().any(|v| v.abs() > 0.01));
        assert_eq!(a.diagnostics.failed_blocks, 0);
        assert!(a.diagnostics.pair_restarts > 3);
        let silent = Control {
            audible: false,
            ..c
        };
        for _ in 0..600 {
            a.sample(silent);
        }
        let positions = a.cursors.map(|c| c.position);
        assert!((0..1000).all(|_| a.sample(silent) == 0.));
        assert_eq!(a.cursors.map(|c| c.position), positions);
        assert_eq!(
            wave,
            (0..96000)
                .map(|_| a.sample(Control { generation: 2, ..c }))
                .collect::<Vec<_>>()
        );
        assert!(a.failure.is_none());
    }
}
