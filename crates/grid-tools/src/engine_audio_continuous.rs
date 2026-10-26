//! Continuous purchased PCM presentation using the reconstructed pair scheduler.
//! Backend resampling/bus policy and supplied outer-frame inputs are experimental.
use crate::{
    engine_audio::{Control, RATE},
    engine_audio_control::{self, CONTROLLER_BYTES, CURVE_WORDS, VehicleEngine},
    engine_audio_publication::{Environment, VOICE_BYTES},
    engine_audio_restart::PlaybackService,
    engine_audio_schedule,
    engine_audio_load,
    engine_audio_shift::{self, Shift},
    engine_audio_whine,
    mech_audio::{self, Backfire, Exhaust, Turbo},
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
    /// carsetup GearBox emitter offset.
    pub gearbox: [f32; 3],
    /// carsetup MechBank: the `mec_*` bank of the car's mechanical sounds
    /// (41E785 resolves it into the car record +0x14, which 4511E0 passes to
    /// 435940 in place of the variety's template bank).
    pub mechanics_bank: String,
    /// Its samples, when loaded for the player (`mech_audio::Bank::load`).
    pub mech: Option<Arc<mech_audio::Bank>>,
    /// The gear whine's parameters, when loaded for the player
    /// (`engine_audio_whine::Settings::load`).
    pub whine: Option<engine_audio_whine::Settings>,
    /// engine.ini [Physics] gear-shift settings.
    shift: engine_audio_shift::Settings,
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
        let shift = engine_audio_shift::Settings::read(std::str::from_utf8(&ini)?)?;
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
            // Nine banks' idle or throttle recordings are one frame shorter
            // than carsetup says (eng_sal throttle 454681 of 454682). The
            // controller's length (0x2C0) is the PCM's own, below.
            let pcm_frames = bytes.len() / 2;
            if sample.rate != 32000 || !(pcm_frames == frames as usize || pcm_frames + 1 == frames as usize) {
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
            gearbox: setup.offsets[2],
            mechanics_bank: setup.mechanics_bank.to_owned(),
            mech: None,
            whine: None,
            shift,
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
    /// 451770's gear-shift state (`engine_audio_shift`).
    shift: Shift,
    /// The car's mechanical sounds (`mech_audio`), `None` without a mec bank.
    pub mech: Option<mech_audio::Voices>,
    /// The player's car (car +0x6C = 0): only it plays the clunk (414750
    /// sets +0x310 for it alone) and the turbo (+0xA4D8), and its backfire
    /// plays at x = 1 (+0x334 clear, the focused car); other cars' at x = 0.
    pub player: bool,
    /// Their group levels and emitter distances.
    pub mech_levels: mech_audio::Levels,
    /// 451260 and 44D840 state of the player.
    backfire: Backfire,
    turbo: Turbo,
    /// The gear whine this block (`engine_audio_whine`, run per tick by the
    /// caller from the vehicle), and the last start it played.
    pub whine: Option<engine_audio_whine::Voice>,
    whine_start: Option<u8>,
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
            shift: Shift::new(engine_audio_shift::Settings::NONE, 1),
            gain: 0.,
            bus_db: CHASE_BUS_DB,
            diagnostics: Diagnostics::default(),
            failure: None,
            mech: None,
            player: true,
            mech_levels: mech_audio::Levels::SILENT,
            backfire: Backfire::new(0x5eed),
            turbo: Turbo::new(Turbo::SPIDLE, Turbo::HISS),
            whine: None,
            whine_start: None,
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
        // The constructor's shift state [I: the seed of A682A0's sequence].
        self.shift = Shift::new(self.bank.shift, 0x5eed);
        self.backfire = Backfire::new(0x5eed);
        self.turbo = Turbo::new(Turbo::SPIDLE, Turbo::HISS);
        self.whine_start = None;
        if let Some(mech) = &mut self.mech {
            mech.clear();
        }
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
        // 451770 after 422A40: the gear-shift effects (414750, 451580,
        // 4150D0). The rest of its load effects are not reconstructed.
        let dt = engine.delta as f32;
        let phase = f(&self.controller, 0x2c) + dt;
        put(&mut self.controller, 0x2c, phase);
        let shift = self.shift.frame(control.gear, dt);
        put(&mut self.controller, 0, shift.scale);
        put(&mut self.controller, 0x1a0, shift.load);
        put(&mut self.controller, 0x2f8, shift.pitch);
        // 4148A0 and 414970 with the applied throttle (`Control::throttle`).
        engine_audio_load::throttle(&mut self.controller, control.throttle, dt);
        engine_audio_load::limiter(&mut self.controller, control.gear, dt);
        if let Some(mech) = &mut self.mech {
            // 451580 → 4511E0: GearClucks at x = 1 (0 dB, pitch 1).
            if shift.clunk && self.player {
                mech.start("GearClucks", 1.);
            }
            let (up, since_shift) = self.shift.since();
            // 451260; +0x20 is +0x48 after 414C70 [I: its fall limit and
            // 451580's RPM blend are not ported]. x = 1: the focused car.
            let exhaust = Exhaust {
                change: f(&self.controller, 0x304),
                rpm: f(&self.controller, 0x48),
                up,
                since_shift,
                limiter: self.controller[0xd0] != 0,
            };
            self.backfire.frame(exhaust, dt, if self.player { 1. } else { 0. }, mech);
            // 44D840 after 451770: the turbo on +0xC8 and +0xB0.
            if self.player {
                let state = if since_shift == 0. { if up { 1 } else { 2 } } else { 0 };
                let blow = self.turbo.frame(dt, f(&self.controller, 0xc8), f(&self.controller, 0xb0), state);
                self.turbo.play(blow, mech);
            }
            // 4518E0 between 451770 and 44D840: the whine.
            match self.whine {
                Some(w) => {
                    mech.whine(w.entry, self.whine_start != Some(w.start), w.db, w.pitch);
                    self.whine_start = Some(w.start);
                }
                None => {
                    mech.stop(mech_audio::WHINE);
                }
            }
            mech.frame(self.mech_levels);
        }
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
        if let Some(mech) = &mut self.mech {
            value += mech.sample();
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
            gearbox: [0., 0., 4.],
            mechanics_bank: String::new(),
            mech: None,
            whine: None,
            shift: engine_audio_shift::Settings::NONE,
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
            gear: 0,
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

    /// Idle on the grid for a second, then the throttle up through 3 s:
    /// no raceable car's decoder fails. A pair restart stores the raw
    /// pitch, 0.4955 for the BMW 320si at idle; the controller's input check
    /// rejected it below 0.5 and every BMW engine went silent (F516).
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn every_loadable_engine_runs_through_the_start() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        // Every raceable car's bank loads (until F519 eleven failed: curves
        // above 1, recordings one frame short).
        for stem in crate::series::RACEABLE.iter().chain(&["stk"]) {
            let bank = Bank::load(std::path::Path::new(&root), &crate::car::Car::new(stem).unwrap())
                .unwrap_or_else(|e| panic!("{stem}: {e}"));
            let mut m = Mixer::new(bank);
            let mut peak = 0f32;
            for n in 0..48000 * 4 {
                let (rpm, throttle) =
                    if n < 48000 { (0.297, 0.) } else { (0.297 + (n - 48000) as f32 / (48000. * 3.) * 0.6, 1.) };
                peak = peak.max(m.sample(Control { normalized_rpm: rpm, throttle, audible: true, generation: 1, gear: 0 }).abs());
                assert!(m.failure.is_none(), "{stem} at sample {n}: {:?}", m.failure);
            }
            assert!(peak > 0.01, "{stem} silent");
        }
    }

    /// An upshift from second to third at full throttle on the Lacetti: the
    /// same samples as a held gear up to the shift, then the 451770 shift
    /// effects (RPM dip, off-load crossfade for 0.13 s, the wobble from
    /// 0.18 s) change the sound. `GRID_ENGINE_SHIFT_WAV=dir` writes both
    /// renders as 48 kHz mono 16-bit WAVs.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn an_upshift_changes_the_lacetti_only_from_the_shift() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let bank = Bank::lacetti(std::path::Path::new(&root)).unwrap();
        let render = |shift: bool| {
            let mut m = Mixer::new(bank.clone());
            (0..48000 * 2)
                .map(|n| {
                    let after = n >= 48000;
                    let gear = if after && shift { 3 } else { 2 };
                    let rpm = if after && shift { 0.55 } else { 0.7 };
                    let v = m.sample(Control { normalized_rpm: rpm, throttle: 1., audible: true, generation: 1, gear });
                    assert!(m.failure.is_none(), "{:?}", m.failure);
                    v
                })
                .collect::<Vec<f32>>()
        };
        let (held, shifted) = (render(false), render(true));
        assert_eq!(held[..48000], shifted[..48000]);
        let rms = |x: &[f32]| (x.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / x.len() as f64).sqrt();
        for k in 0..20 {
            let w = 48000 + k * 2400..48000 + (k + 1) * 2400;
            println!(
                "engine_shift lacetti ms={}..{} held_rms={:.4} shifted_rms={:.4}",
                k * 50,
                (k + 1) * 50,
                rms(&held[w.clone()]),
                rms(&shifted[w])
            );
        }
        assert_ne!(held[48000..48000 + 4800], shifted[48000..48000 + 4800]);
        if let Some(dir) = std::env::var_os("GRID_ENGINE_SHIFT_WAV") {
            for (name, pcm) in [("held", &held), ("upshift", &shifted)] {
                let mut bytes = Vec::new();
                let data = (pcm.len() * 2) as u32;
                bytes.extend_from_slice(b"RIFF");
                bytes.extend_from_slice(&(data + 36).to_le_bytes());
                bytes.extend_from_slice(b"WAVEfmt ");
                for v in [16u32, 1 | (1 << 16), 48000, 96000, 2 | (16 << 16)] {
                    bytes.extend_from_slice(&v.to_le_bytes());
                }
                bytes.extend_from_slice(b"data");
                bytes.extend_from_slice(&data.to_le_bytes());
                for v in pcm.iter() {
                    bytes.extend_from_slice(&((v.clamp(-1., 1.) * 32767.).round() as i16).to_le_bytes());
                }
                std::fs::write(std::path::Path::new(&dir).join(format!("lacetti-{name}.wav")), bytes).unwrap();
            }
        }
    }
}
