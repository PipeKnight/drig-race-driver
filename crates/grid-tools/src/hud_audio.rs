//! HUD sounds (docs/AUDIO_RESEARCH.md "HUD"): the original's HUD manager
//! (`cmr6_audio_h_u_d_manager.cpp`, Play 0x43E770) plays named effects from
//! `audio/FEMappings.bin` (loader 0x45D4A0). In a race the wrapper 0x440E10
//! looks up `"low_" + name`. Every in-race entry has pan 0, so the voice is a
//! 2D one-shot on the HUD group (16) at clamp(volume, -48, 0) dB and
//! clamp(pitch, 0.5, 2). Samples come from the effect's AudioVariety
//! (`audioVarieties.bin`, bank `audiohud`); WIP bit 0x20 of the format
//! halfword is interleaved stereo (A29B0C selects AL_FORMAT_STEREO16).
use grid_formats::{audio_dictionary, binxml};
use std::{collections::HashMap, path::Path, sync::Arc};

/// The HUD group of GroupModeVolumes (41F570 order).
pub const GROUP: usize = 16;
/// Polyphonic HUD slots at +0x768 (0x43E770); a play with none free is dropped.
pub const SLOTS: usize = 20;

/// One decoded bank sample: interleaved PCM in [-1, 1].
pub struct Sound {
    pub pcm: Arc<[f32]>,
    pub channels: usize,
    pub rate: u32,
    pub looped: bool,
}

/// A `FEMappings` effect resolved against its variety.
#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    pub samples: Vec<usize>,
    /// Clamped as Play does: [-48, 0] dB and [0.5, 2].
    pub volume_db: f32,
    pub pitch: f32,
    pub monophonic: bool,
}

pub struct Bank {
    pub sounds: Vec<Sound>,
    pub effects: HashMap<String, Effect>,
}

fn attr<'a>(e: &binxml::Element<'a>, name: &str) -> Option<&'a str> {
    e.attributes
        .iter()
        .find(|(k, _)| *k == name)
        .map(|(_, v)| *v)
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Interleaved PCM16 of a WIP record: the four-byte prefix is skipped
/// (A29C74); flags 0x10 loop, 0x20 stereo.
pub fn wip_sound(bank: &audio_dictionary::Bank, index: usize, data: &[u8]) -> Result<Sound, String> {
    let s = &bank.samples[index];
    if bank.extension != "WIP" || s.flags & !0x30 != 0 || s.rate == 0 {
        return Err(format!("{}: unsupported HUD sample {}", bank.name, s.name));
    }
    let record = data
        .get(s.offset..s.end)
        .filter(|r| r.len() >= 6 && r.len() % 2 == 0)
        .ok_or_else(|| format!("{}: bad extent of {}", bank.name, s.name))?;
    let channels = if s.flags & 0x20 != 0 { 2 } else { 1 };
    let pcm: Arc<[f32]> = record[4..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|w| f32::from(i16::from_le_bytes(*w)) / 32768.)
        .collect();
    if pcm.len() < channels {
        return Err(format!("{}: empty {}", bank.name, s.name));
    }
    Ok(Sound {
        pcm,
        channels,
        rate: u32::from(s.rate),
        looped: s.flags & 0x10 != 0,
    })
}

impl Bank {
    pub fn load(root: &Path) -> Result<Arc<Self>, String> {
        let audio = root.join("audio");
        Ok(Arc::new(Self::parse(
            &read(&audio.join("CMSamples.dic"))?,
            &read(&audio.join("audiohud.WIP"))?,
            &read(&audio.join("audioVarieties.bin"))?,
            &read(&audio.join("FEMappings.bin"))?,
        )?))
    }

    pub fn parse(dictionary: &[u8], data: &[u8], varieties: &[u8], mappings: &[u8]) -> Result<Self, String> {
        let dictionary = audio_dictionary::read(dictionary).map_err(|e| format!("{e:?}"))?;
        let bank = dictionary
            .banks
            .iter()
            .find(|b| b.name == "audiohud")
            .ok_or("audiohud bank absent")?;
        let mut sounds = Vec::new();
        let mut index = HashMap::new();
        for (i, s) in bank.samples.iter().enumerate() {
            index.insert(s.name, sounds.len());
            sounds.push(wip_sound(bank, i, data)?);
        }
        let doc = binxml::read(varieties).map_err(|e| format!("audioVarieties: {e:?}"))?;
        let e = &doc.elements;
        let mut variety: HashMap<&str, Vec<usize>> = HashMap::new();
        for (i, v) in e.iter().enumerate() {
            if v.name != "AudioVariety" || attr(v, "bank") != Some("audiohud") {
                continue;
            }
            let list = e
                .iter()
                .filter(|s| s.name == "Sample" && s.parent.is_some_and(|p| e[p].parent == Some(i)))
                .filter_map(|s| attr(s, "index").and_then(|n| index.get(n).copied()))
                .collect();
            variety.insert(attr(v, "name").ok_or("unnamed variety")?, list);
        }
        let doc = binxml::read(mappings).map_err(|e| format!("FEMappings: {e:?}"))?;
        let mut effects = HashMap::new();
        for m in doc.elements.iter().filter(|m| m.name == "Effect") {
            let (Some(name), Some(samples)) =
                (attr(m, "name"), attr(m, "variety").and_then(|v| variety.get(v)))
            else {
                continue;
            };
            let number = |k: &str, default: f32| {
                attr(m, k)
                    .and_then(|v| v.trim().parse::<f32>().ok())
                    .filter(|v| v.is_finite())
                    .unwrap_or(default)
            };
            // Only pan 0 effects are 2D HUD voices; the data has no others.
            if number("pan", 0.) != 0. || samples.is_empty() {
                continue;
            }
            effects.insert(
                name.to_owned(),
                Effect {
                    samples: samples.clone(),
                    // Constants 0xCB0BC0 (-48), 0xCAF88C (0.5), 0xCB8DA4 (2).
                    volume_db: number("volume", 0.).clamp(-48., 0.),
                    pitch: number("pitch", 1.).clamp(0.5, 2.),
                    monophonic: attr(m, "monophonic").is_some_and(|v| v != "0" && v != "false"),
                },
            );
        }
        Ok(Self { sounds, effects })
    }

    /// The in-race effect for `name` (0x440E10 prefixes `low_`).
    pub fn race_effect(&self, name: &str) -> Option<&Effect> {
        self.effects.get(&format!("low_{name}"))
    }
}

/// In-race HUD events and their FEMappings names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cue {
    /// Start-light message state 0 / 1 (handler 0x63E710).
    LightRed,
    LightGreen,
    /// Drift scorer (0x643E3D, 0x63C552, 0x6304BC, 0x644038).
    DriftScoreUp,
    DriftActivate,
    /// Combo raised to N (2..=10).
    DriftCombo(u8),
    /// Rating index reached before the increment (0..=7).
    DriftRating(u8),
    DriftValidClose,
    DriftComboBank,
    DriftComboFail,
    /// Flashback transition samples (`system/states.bin` playSample).
    FlashbackShot1,
    FlashbackShot2,
    FlashbackZoomUp,
}

impl Cue {
    pub fn name(self) -> String {
        match self {
            Cue::LightRed => "race_light_red".into(),
            Cue::LightGreen => "race_light_green".into(),
            Cue::DriftScoreUp => "drift_score_up".into(),
            Cue::DriftActivate => "drift_activate".into(),
            Cue::DriftCombo(n) => format!("drift_combo_x{}", n.clamp(2, 10)),
            Cue::DriftRating(r) => format!("drift_rating_x{}", r.min(7)),
            Cue::DriftValidClose => "drift_valid_close".into(),
            Cue::DriftComboBank => "drift_combo_bank".into(),
            Cue::DriftComboFail => "drift_combo_fail".into(),
            Cue::FlashbackShot1 => "flashback_shot_1".into(),
            Cue::FlashbackShot2 => "flashback_shot_2".into(),
            Cue::FlashbackZoomUp => "flashback_zoomup".into(),
        }
    }
}

/// Start-light sounds of the original sequence (0x6CE350 start, 0x6CE260
/// update): the stage advances every 1.0 s (0x6CE383) from stage 1 at the
/// start; each advance posts red, and green when the stage reaches
/// `stages - 1`. So `stages - 2` reds at 0, 1, ... s and green at
/// `stages - 2` s, the shipped sequence time. Returns the cue for the
/// countdown second that `elapsed` (s since the countdown began) entered
/// after `previous`.
pub fn light_cue(previous: Option<f64>, elapsed: f64, stages: usize) -> Option<Cue> {
    let reds = stages.saturating_sub(2) as i64;
    let second = |t: f64| t.floor() as i64;
    let now = second(elapsed);
    if previous.is_some_and(|p| second(p) >= now) || now < 0 {
        return None;
    }
    if now < reds {
        Some(Cue::LightRed)
    } else if now == reds {
        Some(Cue::LightGreen)
    } else {
        None
    }
}

#[derive(Clone, Copy, Default)]
struct Slot {
    sound: Option<usize>,
    position: f64,
    step: f64,
    gain: f32,
}

/// The HUD voices: 20 polyphonic slots and two monophonic ones (+0x864/+0x868),
/// mixed to interleaved stereo at `rate`.
pub struct Player {
    bank: Arc<Bank>,
    rate: u32,
    slots: [Slot; SLOTS],
    mono: [Slot; 2],
    next_mono: usize,
    last: HashMap<u32, usize>,
    names: Vec<String>,
    random: u32,
    /// HUD group level in dB (mix table + slider trim).
    pub group_db: f32,
}

impl Player {
    pub fn new(bank: Arc<Bank>, rate: u32) -> Self {
        Self {
            bank,
            rate,
            slots: [Slot::default(); SLOTS],
            mono: [Slot::default(); 2],
            next_mono: 0,
            last: HashMap::new(),
            names: Vec::new(),
            random: 0x1234_5678,
            group_db: 0.,
        }
    }

    fn effect_id(&mut self, name: &str) -> u32 {
        match self.names.iter().position(|n| n == name) {
            Some(i) => i as u32,
            None => {
                self.names.push(name.to_owned());
                (self.names.len() - 1) as u32
            }
        }
    }

    /// Play(name, 0, 1) as every in-race caller does: retriggering the same
    /// effect is allowed; with no free slot the play is dropped. Returns
    /// whether a voice started.
    pub fn play(&mut self, cue: Cue) -> bool {
        let name = cue.name();
        let Some(effect) = self.bank.race_effect(&name).cloned() else {
            return false;
        };
        let id = self.effect_id(&name);
        // 0x413B80: a uniform pick that avoids repeating the last sample [I: RNG].
        let mut pick = effect.samples[0];
        if effect.samples.len() > 1 {
            self.random = self.random.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let n = effect.samples.len() - 1;
            let i = (self.random >> 8) as usize % n;
            let previous = self.last.get(&id).copied();
            let others: Vec<usize> = effect.samples.iter().copied().filter(|s| Some(*s) != previous).collect();
            pick = others.get(i).copied().unwrap_or(pick);
        }
        self.last.insert(id, pick);
        let sound = &self.bank.sounds[pick];
        let voice = Slot {
            sound: Some(pick),
            position: 0.,
            step: f64::from(effect.pitch) * f64::from(sound.rate) / f64::from(self.rate),
            gain: 10f32.powf(effect.volume_db / 20.),
        };
        let slot = if effect.monophonic {
            // Ping-pong: the new play stops the other slot's voice.
            let k = self.next_mono;
            self.next_mono ^= 1;
            self.mono[k ^ 1].sound = None;
            &mut self.mono[k]
        } else {
            match self.slots.iter_mut().find(|s| s.sound.is_none()) {
                Some(s) => s,
                None => return false,
            }
        };
        *slot = voice;
        true
    }

    /// Voices still playing.
    pub fn active(&self) -> usize {
        self.slots.iter().chain(&self.mono).filter(|s| s.sound.is_some()).count()
    }

    /// One stereo frame. The group gain follows the A23B70 floor gate.
    pub fn frame(&mut self) -> [f32; 2] {
        let group = crate::audio_mix::backend_gain(0., self.group_db, Some(0.));
        let mut out = [0f32; 2];
        for slot in self.slots.iter_mut().chain(self.mono.iter_mut()) {
            let Some(index) = slot.sound else { continue };
            let sound = &self.bank.sounds[index];
            let frames = sound.pcm.len() / sound.channels;
            let i = slot.position as usize;
            if i + 1 >= frames && !sound.looped {
                slot.sound = None;
                continue;
            }
            let frac = (slot.position - slot.position.floor()) as f32;
            let at = |k: usize, c: usize| sound.pcm[(k % frames) * sound.channels + c.min(sound.channels - 1)];
            for (c, o) in out.iter_mut().enumerate() {
                let (a, b) = (at(i, c), at(i + 1, c));
                *o += (a + (b - a) * frac) * slot.gain * group;
            }
            slot.position += slot.step;
            if sound.looped {
                slot.position %= frames as f64;
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lights_follow_the_one_second_stage_timer() {
        // Donington, 6 stages: reds at 0..3 s, green at 4 s (the 4.0 s sequence).
        let mut cues = Vec::new();
        let mut previous = None;
        for k in 0..=300 {
            let t = f64::from(k) * 0.016;
            if let Some(c) = light_cue(previous, t, 6) {
                cues.push((t.floor() as i64, c));
            }
            previous = Some(t);
        }
        assert_eq!(
            cues,
            [
                (0, Cue::LightRed),
                (1, Cue::LightRed),
                (2, Cue::LightRed),
                (3, Cue::LightRed),
                (4, Cue::LightGreen)
            ]
        );
        // Yokohama, 4 stages over 2.0 s: two reds, green at 2 s.
        assert_eq!(light_cue(Some(1.9), 2.0, 4), Some(Cue::LightGreen));
        assert_eq!(light_cue(Some(1.0), 1.5, 4), None);
    }

    #[test]
    fn cue_names_match_the_mappings() {
        assert_eq!(Cue::DriftCombo(12).name(), "drift_combo_x10");
        assert_eq!(Cue::DriftRating(0).name(), "drift_rating_x0");
        assert_eq!(Cue::LightGreen.name(), "race_light_green");
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn shipped_hud_effects_load_and_play() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let bank = Bank::load(Path::new(&root)).unwrap();
        assert_eq!(bank.sounds.len(), 87);
        assert!(bank.sounds.iter().all(|s| s.rate == 40_000));
        assert!(bank.sounds.iter().any(|s| s.channels == 2));
        // Race lights are +6 dB in the file, clamped to 0 by Play.
        let red = bank.race_effect("race_light_red").unwrap();
        assert_eq!((red.volume_db, red.pitch), (0., 1.));
        assert_eq!(bank.race_effect("drift_combo_bank").unwrap().volume_db, -1.);
        for cue in [
            Cue::LightRed,
            Cue::LightGreen,
            Cue::DriftScoreUp,
            Cue::DriftActivate,
            Cue::DriftCombo(2),
            Cue::DriftCombo(10),
            Cue::DriftRating(0),
            Cue::DriftRating(7),
            Cue::DriftValidClose,
            Cue::DriftComboBank,
            Cue::DriftComboFail,
            Cue::FlashbackShot1,
            Cue::FlashbackShot2,
            Cue::FlashbackZoomUp,
        ] {
            assert!(bank.race_effect(&cue.name()).is_some(), "{cue:?}");
        }
        let mut player = Player::new(bank.clone(), 48_000);
        player.group_db = -9.;
        assert!(player.play(Cue::LightRed));
        let mut frames = 0;
        let mut peak = 0f32;
        while player.active() > 0 && frames < 48_000 * 10 {
            peak = peak.max(player.frame()[0].abs());
            frames += 1;
        }
        assert!(peak > 0.01 && peak < 1., "{peak}");
        assert_eq!(player.active(), 0, "the one-shot ends ({frames} frames)");
        // 20 voices at most.
        let started = (0..25).filter(|_| player.play(Cue::DriftScoreUp)).count();
        assert_eq!(started, SLOTS);
    }
}
