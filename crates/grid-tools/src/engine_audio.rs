//! Independent granular engine presentation. Never writes vehicle state.
//! Source time tables select short windows; no original mixer equivalence.
use grid_formats::audio_dictionary;
use std::{fs, io::Read, path::Path, sync::Arc};

pub const RATE: u32 = 48_000;
const GRAIN: usize = 6144; // 128 ms, 50% overlap at the output rate.
const HALF: usize = GRAIN / 2;

#[derive(Clone)]
struct Layer {
    pcm: Vec<f32>,
    rate: f32,
    time_from_revs: Vec<f32>,
    revs_from_time: Vec<f32>,
}
#[derive(Clone)]
pub struct EngineBank {
    layers: [Layer; 3], // exterior deceleration, acceleration, loaded acceleration
}
fn bounded(path: &Path, limit: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut b = Vec::new();
    fs::File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut b)?;
    if b.len() > limit {
        return Err("engine audio resource exceeds bound".into());
    }
    Ok(b)
}
fn curve(ini: &str, section: &str, key: &str) -> Result<Vec<f32>, &'static str> {
    let mut current = "";
    let mut found = None;
    for line in ini.lines().map(str::trim) {
        if line.starts_with('[') && line.ends_with(']') {
            current = &line[1..line.len() - 1];
        } else if current == section
            && let Some((name, value)) = line.split_once('=')
            && name == key
        {
            if found.is_some() {
                return Err("duplicate engine curve");
            }
            let values = value
                .split(',')
                .map(|v| {
                    v.trim()
                        .parse::<f32>()
                        .map_err(|_| "invalid engine curve number")
                })
                .collect::<Result<Vec<_>, _>>()?;
            if values.len() != 1024
                || values
                    .iter()
                    .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            {
                return Err("unsupported engine curve extent/range");
            }
            found = Some(values);
        }
    }
    found.ok_or("missing engine curve")
}
impl EngineBank {
    /// Only the already validated Lacetti exterior mono layout is selected.
    pub fn lacetti(root: &Path) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
        let dic = bounded(&root.join("audio/CMSamples.dic"), 16 * 1024 * 1024)?;
        let dictionary = audio_dictionary::read(&dic)?;
        let bank = dictionary
            .banks
            .iter()
            .find(|b| b.name == "eng_lac")
            .ok_or("missing Lacetti audio bank")?;
        let bytes = bounded(&root.join("audio/eng_lac.WIP"), 64 * 1024 * 1024)?;
        let ini = bounded(&root.join("audio/cars/lacengine.ini"), 1024 * 1024)?;
        let ini = std::str::from_utf8(&ini)?;
        let mut layers = Vec::new();
        for (name, section) in [
            ("dn_ex", "Idle"),
            ("up_ex", "Throttle"),
            ("up_ex_load", "Throttle"),
        ] {
            let index = bank
                .samples
                .iter()
                .position(|s| s.name == name)
                .ok_or("missing engine layer")?;
            let sample = &bank.samples[index];
            let pcm = bank
                .mono_pcm16(index, &bytes)?
                .as_chunks::<2>()
                .0
                .iter()
                .map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.)
                .collect::<Vec<_>>();
            if sample.rate != 32000 || pcm.len() < 8192 {
                return Err("unsupported engine layer rate/extent".into());
            }
            layers.push(Layer {
                pcm,
                rate: f32::from(sample.rate),
                time_from_revs: curve(ini, section, "TimeFromRevs")?,
                revs_from_time: curve(ini, section, "RevsFromTime")?,
            });
        }
        Ok(Arc::new(Self {
            layers: layers.try_into().map_err(|_| "engine layer count")?,
        }))
    }
}
fn lookup(table: &[f32], t: f32) -> f32 {
    let at = t.clamp(0., 1.) * (table.len() - 1) as f32;
    let i = (at as usize).min(table.len() - 2);
    table[i] + (table[i + 1] - table[i]) * (at - i as f32)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Control {
    pub normalized_rpm: f32,
    pub throttle: f32,
    pub audible: bool,
    pub generation: u32,
}
impl Control {
    /// One coherent atomic mailbox word. Invalid telemetry always silences.
    pub fn pack(self) -> u64 {
        let valid = self.normalized_rpm.is_finite() && self.throttle.is_finite();
        let rpm = (self.normalized_rpm.clamp(0., 1.) * 65535.).round() as u64;
        let throttle = (self.throttle.clamp(0., 1.) * 255.).round() as u64;
        rpm | throttle << 16
            | u64::from(self.audible && valid) << 24
            | u64::from(self.generation) << 32
    }
    pub fn unpack(word: u64) -> Self {
        Self {
            normalized_rpm: (word & 65535) as f32 / 65535.,
            throttle: ((word >> 16) & 255) as f32 / 255.,
            audible: word & (1 << 24) != 0,
            generation: (word >> 32) as u32,
        }
    }
    /// Offsets are established engine angular speed / maximum speed, rad/s.
    pub fn vehicle(words: &[u32], throttle: f32, audible: bool, generation: u32) -> Self {
        let max = words
            .get(0x17a4 / 4)
            .copied()
            .map(f32::from_bits)
            .unwrap_or(f32::NAN);
        let speed = words
            .get(0x1894 / 4)
            .copied()
            .map(f32::from_bits)
            .unwrap_or(f32::NAN);
        Self {
            normalized_rpm: if max.is_finite() && max > 0. && speed.is_finite() {
                speed / max
            } else {
                f32::NAN
            },
            throttle,
            audible: audible && speed > 0.,
            generation,
        }
    }
}
#[derive(Clone, Copy, Default)]
struct Grain {
    position: f32,
    step: f32,
}
pub struct Mixer {
    bank: Arc<EngineBank>,
    grains: [[Grain; 2]; 3],
    phase: usize,
    rpm: f32,
    throttle: f32,
    gain: f32,
    generation: Option<u32>,
}
impl Mixer {
    pub fn new(bank: Arc<EngineBank>) -> Self {
        Self {
            bank,
            grains: [[Grain::default(); 2]; 3],
            phase: 0,
            rpm: 0.3,
            throttle: 0.,
            gain: 0.,
            generation: None,
        }
    }
    fn seed(&mut self, voice: usize, age: usize) {
        for (layer, grains) in self.bank.layers.iter().zip(&mut self.grains) {
            let time = lookup(&layer.time_from_revs, self.rpm);
            let captured = lookup(&layer.revs_from_time, time).max(0.05);
            let step = (self.rpm / captured).clamp(0.25, 2.) * layer.rate / RATE as f32;
            // Keep the complete grain in bounds instead of clamping each sample.
            let span = step * (GRAIN - 1) as f32;
            let start = (time * (layer.pcm.len() - 1) as f32 - span * 0.5)
                .clamp(0., (layer.pcm.len() - 2) as f32 - span);
            grains[voice] = Grain {
                position: start + age as f32 * step,
                step,
            };
        }
    }
    pub fn sample(&mut self, control: Control) -> f32 {
        let control = Control::unpack(control.pack());
        if self.generation != Some(control.generation) {
            self.generation = Some(control.generation);
            self.rpm = control.normalized_rpm.max(0.05);
            self.throttle = control.throttle;
            self.phase = 0;
            self.gain = 0.;
            self.seed(0, 0);
            self.seed(1, HALF);
        }
        // About 25 ms pitch/load smoothing, 10 ms bounded fade on pause/mute.
        self.rpm += (control.normalized_rpm.max(0.05) - self.rpm) / 1200.;
        self.throttle += (control.throttle - self.throttle) / 1200.;
        let target = if control.audible { 0.28 } else { 0. };
        self.gain += (target - self.gain).clamp(-0.28 / 480., 0.28 / 480.);
        if !control.audible && self.gain == 0. {
            return 0.;
        }
        if self.phase == 0 {
            self.seed(0, 0);
        }
        if self.phase == HALF {
            self.seed(1, 0);
        }
        let age = [self.phase, (self.phase + HALF) % GRAIN];
        let mut values = [0.; 3];
        for (i, (layer, grains)) in self.bank.layers.iter().zip(&mut self.grains).enumerate() {
            for (voice, grain) in grains.iter_mut().enumerate() {
                let j = (grain.position as usize).min(layer.pcm.len() - 2);
                let fraction = (grain.position - j as f32).clamp(0., 1.);
                let value = layer.pcm[j] + (layer.pcm[j + 1] - layer.pcm[j]) * fraction;
                let window =
                    0.5 - 0.5 * (std::f32::consts::TAU * age[voice] as f32 / GRAIN as f32).cos();
                values[i] += value * window;
                grain.position += grain.step;
            }
        }
        self.phase = (self.phase + 1) % GRAIN;
        let acceleration = values[1] * (1. - 0.7 * self.throttle) + values[2] * 0.7 * self.throttle;
        let value = (values[0] * (1. - self.throttle) + acceleration * self.throttle) * self.gain;
        value / (1. + value.abs())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn bank() -> Arc<EngineBank> {
        let layer = Layer {
            pcm: (0..16000).map(|i| (i as f32 * 0.1).sin()).collect(),
            rate: 32000.,
            time_from_revs: vec![0., 1.],
            revs_from_time: vec![0.2, 1.],
        };
        Arc::new(EngineBank {
            layers: [layer.clone(), layer.clone(), layer],
        })
    }
    #[test]
    fn coherent_mailbox_and_invalid_silencing() {
        let c = Control {
            normalized_rpm: 0.6,
            throttle: 0.8,
            audible: true,
            generation: u32::MAX,
        };
        let d = Control::unpack(c.pack());
        assert!((c.normalized_rpm - d.normalized_rpm).abs() < 1. / 65535.);
        assert!((c.throttle - d.throttle).abs() < 1. / 255.);
        assert!(d.audible);
        assert_eq!(d.generation, u32::MAX);
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert!(
                !Control::unpack(
                    Control {
                        normalized_rpm: value,
                        ..c
                    }
                    .pack()
                )
                .audible
            );
            assert!(
                !Control::unpack(
                    Control {
                        throttle: value,
                        ..c
                    }
                    .pack()
                )
                .audible
            );
        }
        assert!(!Control::unpack(Control::vehicle(&[], 0., true, 0).pack()).audible);
    }
    #[test]
    fn steady_pitch_pause_reset_bounds_and_repeat() {
        let b = bank();
        let mut a = Mixer::new(b.clone());
        let mut r = Mixer::new(b);
        let c = Control {
            normalized_rpm: 0.5,
            throttle: 1.,
            audible: true,
            generation: 1,
        };
        let wave: Vec<_> = (0..20000).map(|_| a.sample(c)).collect();
        assert!(wave.iter().any(|v| v.abs() > 0.1));
        assert!(wave.iter().all(|v| v.is_finite() && v.abs() < 0.3));
        assert_eq!(wave, (0..20000).map(|_| r.sample(c)).collect::<Vec<_>>());
        let silent = Control {
            audible: false,
            ..c
        };
        for _ in 0..481 {
            a.sample(silent);
        }
        let phase = a.phase;
        assert!((0..10000).all(|_| a.sample(silent) == 0.));
        assert_eq!(a.phase, phase);
        let reset = Control { generation: 2, ..c };
        assert_eq!(
            (0..20000).map(|_| a.sample(reset)).collect::<Vec<_>>(),
            wave
        );
    }
    #[test]
    fn curves_reject_duplicates_extent_and_nonfinite() {
        assert!(curve("[Idle]\nTimeFromRevs=0,1", "Idle", "TimeFromRevs").is_err());
        let row = vec!["0.5"; 1024].join(",");
        let ini = format!("[Idle]\nTimeFromRevs={row}\n[Loop]\nThrottle=1\n[Loop]\nThrottle=0");
        assert_eq!(curve(&ini, "Idle", "TimeFromRevs").unwrap().len(), 1024);
        assert!(
            curve(
                &format!("{ini}\n[Idle]\nTimeFromRevs={row}"),
                "Idle",
                "TimeFromRevs"
            )
            .is_err()
        );
        assert!(curve(&ini.replace("0.5", "NaN"), "Idle", "TimeFromRevs").is_err());
    }
}
