//! Mechanical sounds of the car-audio object (docs/AUDIO_RESEARCH.md §21,
//! §22): the gear clunk (GearClucks), the backfire (Detonations) and the
//! turbo (TurboHiss, TurboSpindle, TurboDump, TurboPip). Each is a
//! CollisionInfo (CollisionInfos.bin, curve over x) whose AudioVariety
//! (audioVarieties.bin, template bank `mec_dbr`) names its samples and
//! distance preset; the samples come from a `mec_*` bank.
use crate::{
    collision_audio::{Curve, read_infos},
    hud_audio::{Sound, wip_sound},
};
use grid_formats::{audio_dictionary, binxml};
use std::{collections::HashMap, path::Path, sync::Arc};

/// The car-audio table entries (43D59D…43D6A3) with a CollisionInfo.
pub const INFOS: [&str; 6] = ["GearClucks", "Detonations", "TurboHiss", "TurboSpindle", "TurboDump", "TurboPip"];

/// One CollisionInfo with its variety's samples (indices into `Bank::sounds`)
/// and preset distances (A23B70 law).
#[derive(Clone, Debug, PartialEq)]
pub struct Info {
    pub curve: Curve,
    pub samples: Vec<usize>,
    pub min_distance: f32,
    pub max_distance: f32,
}

pub struct Bank {
    /// The `mec_*` bank the samples come from.
    pub name: String,
    pub sounds: Vec<Sound>,
    pub infos: HashMap<String, Info>,
}

fn attr<'a>(e: &binxml::Element<'a>, name: &str) -> Option<&'a str> {
    e.attributes.iter().find(|(k, _)| *k == name).map(|(_, v)| *v)
}
fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

impl Bank {
    /// The mechanical sounds from the `mec_*` bank `bank` (e.g. "mec_dbr").
    pub fn load(root: &Path, bank: &str) -> Result<Arc<Self>, String> {
        let audio = root.join("audio");
        Ok(Arc::new(Self::parse(
            bank,
            &read(&audio.join("CMSamples.dic"))?,
            &read(&audio.join(format!("{bank}.WIP")))?,
            &read(&audio.join("audioVarieties.bin"))?,
            &read(&audio.join("CollisionInfos.bin"))?,
        )?))
    }

    pub fn parse(name: &str, dictionary: &[u8], data: &[u8], varieties: &[u8], infos: &[u8]) -> Result<Self, String> {
        let dictionary = audio_dictionary::read(dictionary).map_err(|e| format!("{e:?}"))?;
        let bank = dictionary
            .banks
            .iter()
            .find(|b| b.name == name)
            .ok_or_else(|| format!("{name} bank absent"))?;
        let curves = read_infos(infos)?;
        let doc = binxml::read(varieties).map_err(|e| format!("audioVarieties: {e:?}"))?;
        let presets: HashMap<&str, (f32, f32)> = doc
            .elements
            .iter()
            .filter(|e| e.name == "Preset")
            .filter_map(|e| {
                let number = |k| attr(e, k).and_then(|v| v.trim().parse::<f32>().ok());
                Some((attr(e, "name")?, (number("minDist")?, number("maxDist")?)))
            })
            .collect();
        let mut sounds = Vec::new();
        let mut index: HashMap<&str, usize> = HashMap::new();
        let mut out = HashMap::new();
        for info in INFOS {
            let curve = curves.get(info).ok_or_else(|| format!("CollisionInfo {info} absent"))?.clone();
            let (v, variety) = doc
                .elements
                .iter()
                .enumerate()
                .find(|(_, e)| e.name == "AudioVariety" && attr(e, "name") == Some(curve.variety.as_str()))
                .ok_or_else(|| format!("AudioVariety {} absent", curve.variety))?;
            let mut samples = Vec::new();
            for s in doc.elements.iter().filter(|s| {
                s.name == "Sample" && s.parent.is_some_and(|p| doc.elements[p].parent == Some(v))
            }) {
                let Some(sample) = attr(s, "index") else { continue };
                let k = match index.get(sample) {
                    Some(&k) => k,
                    None => {
                        let i = bank
                            .samples
                            .iter()
                            .position(|b| b.name == sample)
                            .ok_or_else(|| format!("{name}: sample {sample} absent"))?;
                        sounds.push(wip_sound(bank, i, data)?);
                        index.insert(sample, sounds.len() - 1);
                        sounds.len() - 1
                    }
                };
                samples.push(k);
            }
            let (min_distance, max_distance) =
                attr(variety, "preset").and_then(|p| presets.get(p).copied()).unwrap_or((1., 2.));
            out.insert(info.to_owned(), Info { curve, samples, min_distance, max_distance });
        }
        Ok(Self { name: name.to_owned(), sounds, infos: out })
    }
}

/// Group (`group_db`) and emitter distance (`distance`, metres from the
/// listener) indices of [`Levels`].
pub const GEARBOX: usize = 0;
pub const EXHAUST: usize = 1;
/// Group Turbo, emitter EngineBay.
pub const TURBO: usize = 2;

/// What the presentation supplies each frame for one car's mechanical
/// sounds: its group levels in dB (group mix plus trim, 4223D0) and the
/// distances of its GearBox, Exhaust and EngineBay emitters from the
/// listener (A23B70 applies each variety's own preset to them).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Levels {
    pub group_db: [f32; 3],
    pub distance: [f32; 3],
}
impl Levels {
    /// Silent: every emitter out of range.
    pub const SILENT: Self = Self { group_db: [0.; 3], distance: [f32::INFINITY; 3] };
    pub fn to_bits(self) -> [u32; 6] {
        let [a, b, c] = self.group_db;
        let [d, e, f] = self.distance;
        [a, b, c, d, e, f].map(f32::to_bits)
    }
    pub fn from_bits(bits: [u32; 6]) -> Self {
        let v = bits.map(f32::from_bits);
        Self { group_db: [v[0], v[1], v[2]], distance: [v[3], v[4], v[5]] }
    }
}

/// The group and emitter of a CollisionInfo's voice (4511E0: group +0x120,
/// emitter +0x100).
fn route(info: &str) -> (usize, usize) {
    match info {
        "GearClucks" => (GEARBOX, GEARBOX),
        "Detonations" => (EXHAUST, EXHAUST),
        _ => (TURBO, TURBO),
    }
}

struct Voice {
    info: &'static str,
    sound: usize,
    position: f64,
    /// Rate ratio without the curve's pitch.
    step: f64,
    x: f32,
    gain: f32,
}

/// One car's mechanical voices. One-shots started by [`Voices::start`]
/// play to their end; each info has one handle (4511E0 keeps the clunk's at
/// +0x228), so a restart replaces the playing sample.
pub struct Voices {
    bank: Arc<Bank>,
    rate: u32,
    voices: Vec<Voice>,
    last: HashMap<&'static str, usize>,
    random: u32,
}
impl Voices {
    pub fn new(bank: Arc<Bank>, rate: u32) -> Self {
        Self { bank, rate, voices: Vec::new(), last: HashMap::new(), random: 0x2468_ace1 }
    }

    pub fn stop(&mut self) {
        self.voices.clear();
    }

    /// 44F010 one-shot of CollisionInfo `info` at curve position `x`.
    pub fn start(&mut self, info: &'static str, x: f32) {
        let Some(entry) = self.bank.infos.get(info) else { return };
        if entry.samples.is_empty() {
            return;
        }
        // 413B80: a uniform pick that avoids repeating the last sample [I: RNG].
        let previous = self.last.get(info).copied();
        let choices: Vec<usize> =
            entry.samples.iter().copied().filter(|s| Some(*s) != previous || entry.samples.len() == 1).collect();
        self.random = self.random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let sound = choices[(self.random >> 8) as usize % choices.len()];
        self.last.insert(info, sound);
        self.voices.retain(|v| v.info != info);
        let step = f64::from(self.bank.sounds[sound].rate) / f64::from(self.rate);
        self.voices.push(Voice { info, sound, position: 0., step, x, gain: 0. });
    }

    /// Whether a voice of `info` is playing.
    pub fn playing(&self, info: &str) -> bool {
        self.voices.iter().any(|v| v.info == info)
    }

    /// Per frame: each voice's level from its curve (44F010), group and
    /// preset distance (A23B70).
    pub fn frame(&mut self, levels: Levels) {
        for v in &mut self.voices {
            let entry = &self.bank.infos[v.info];
            let (db, _) = entry.curve.at(v.x);
            let (group, emitter) = route(v.info);
            let distance = crate::audio_mix::distance_db(levels.distance[emitter], entry.min_distance, entry.max_distance);
            v.gain = crate::audio_mix::backend_gain(db, levels.group_db[group], distance);
        }
    }

    pub fn sample(&mut self) -> f32 {
        let mut value = 0.;
        let sounds = &self.bank.sounds;
        let infos = &self.bank.infos;
        self.voices.retain_mut(|v| {
            let pcm = &sounds[v.sound].pcm;
            if v.position + 1. >= pcm.len() as f64 {
                return false;
            }
            let index = v.position as usize;
            let fraction = (v.position - index as f64) as f32;
            value += (pcm[index] + (pcm[index + 1] - pcm[index]) * fraction) * v.gain;
            v.position += v.step * f64::from(infos[v.info].curve.at(v.x).1);
            true
        });
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every shipped `mec_*` bank holds the mechanical samples; reports how
    /// the banks differ (sample length and a checksum per sample).
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn every_mec_bank_holds_the_mechanical_samples() {
        let root = std::path::PathBuf::from(std::env::var_os("GRID_GAME_ROOT").unwrap());
        let dictionary = std::fs::read(root.join("audio/CMSamples.dic")).unwrap();
        let names: Vec<String> = audio_dictionary::read(&dictionary)
            .unwrap()
            .banks
            .iter()
            .filter(|b| b.name.starts_with("mec_"))
            .map(|b| b.name.to_owned())
            .collect();
        assert!(names.len() >= 40, "{names:?}");
        for name in &names {
            let bank = Bank::load(&root, name).unwrap_or_else(|e| panic!("{name}: {e}"));
            let sums: Vec<String> = INFOS
                .iter()
                .flat_map(|i| bank.infos[*i].samples.iter())
                .map(|&k| {
                    let s = &bank.sounds[k];
                    let sum = s.pcm.iter().fold(0u64, |a, v| a.wrapping_mul(31).wrapping_add(v.to_bits() as u64));
                    format!("{}:{}:{:x}", s.rate, s.pcm.len(), sum & 0xffff)
                })
                .collect();
            println!("mec_bank {name} {}", sums.join(" "));
        }
        let dbr = Bank::load(&root, "mec_dbr").unwrap();
        assert!(dbr.sounds.iter().all(|s| s.channels == 1), "mono samples");
        let clunk = &dbr.infos["GearClucks"];
        assert_eq!((clunk.samples.len(), clunk.min_distance, clunk.max_distance), (3, 3.5, 4.));
        assert_eq!(clunk.curve.at(1.), (0., 1.));
        assert_eq!(dbr.infos["Detonations"].curve.at(1.), (-11., 1.1));
    }
}
