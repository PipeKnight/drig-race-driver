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
    /// The front-end set (unprefixed names) instead of the in-race one.
    pub front_end: bool,
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

/// Whether a FEMappings name belongs to the front-end or the in-race set
/// (0x440E10 prefixes `low_` in a race).
fn in_scope(name: &str, front_end: bool) -> bool {
    name.starts_with("low_") != front_end
}

impl Bank {
    /// The in-race set (`low_` effects, bank `audiohud`).
    pub fn load(root: &Path) -> Result<Arc<Self>, String> {
        Self::load_set(root, false)
    }

    /// The front-end set (unprefixed effects, mostly bank `fe_fx`).
    pub fn load_front_end(root: &Path) -> Result<Arc<Self>, String> {
        Self::load_set(root, true)
    }

    fn load_set(root: &Path, front_end: bool) -> Result<Arc<Self>, String> {
        let audio = root.join("audio");
        Ok(Arc::new(Self::parse(
            &read(&audio.join("CMSamples.dic"))?,
            &read(&audio.join("audioVarieties.bin"))?,
            &read(&audio.join("FEMappings.bin"))?,
            front_end,
            |bank| read(&audio.join(format!("{}.{}", bank.name, bank.extension))),
        )?))
    }

    /// Keeps the pan-0 effects of one set and decodes only the samples
    /// they use; `data` reads a bank's sample file (once per bank).
    pub fn parse(
        dictionary: &[u8],
        varieties: &[u8],
        mappings: &[u8],
        front_end: bool,
        mut data: impl FnMut(&audio_dictionary::Bank) -> Result<Vec<u8>, String>,
    ) -> Result<Self, String> {
        let dictionary = audio_dictionary::read(dictionary).map_err(|e| format!("{e:?}"))?;
        let doc = binxml::read(varieties).map_err(|e| format!("audioVarieties: {e:?}"))?;
        let e = &doc.elements;
        // Variety -> (bank, sample name) list.
        let mut variety: HashMap<&str, Vec<(&str, &str)>> = HashMap::new();
        for (i, v) in e.iter().enumerate() {
            let (Some(name), Some(bank)) = (attr(v, "name"), attr(v, "bank")) else {
                continue;
            };
            if v.name != "AudioVariety" {
                continue;
            }
            let list = e
                .iter()
                .filter(|s| s.name == "Sample" && s.parent.is_some_and(|p| e[p].parent == Some(i)))
                .filter_map(|s| attr(s, "index").map(|n| (bank, n)))
                .collect();
            variety.insert(name, list);
        }
        let doc = binxml::read(mappings).map_err(|e| format!("FEMappings: {e:?}"))?;
        let mut wanted = Vec::new();
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
            if !in_scope(name, front_end) || number("pan", 0.) != 0. || samples.is_empty() {
                continue;
            }
            let effect = Effect {
                samples: Vec::new(),
                // Constants 0xCB0BC0 (-48), 0xCAF88C (0.5), 0xCB8DA4 (2).
                volume_db: number("volume", 0.).clamp(-48., 0.),
                pitch: number("pitch", 1.).clamp(0.5, 2.),
                monophonic: attr(m, "monophonic").is_some_and(|v| v != "0" && v != "false"),
            };
            wanted.push((name, samples, effect));
        }
        // Decode the used samples bank by bank.
        let mut sounds = Vec::new();
        let mut decoded: HashMap<(&str, &str), Option<usize>> = HashMap::new();
        for bank in &dictionary.banks {
            let used: Vec<usize> = bank
                .samples
                .iter()
                .enumerate()
                .filter(|(_, s)| wanted.iter().any(|w| w.1.contains(&(bank.name, s.name))))
                .map(|(i, _)| i)
                .collect();
            if used.is_empty() {
                continue;
            }
            let bytes = data(bank)?;
            for i in used {
                // An undecodable sample leaves its variety one choice short.
                let sound = wip_sound(bank, i, &bytes).ok().map(|s| {
                    sounds.push(s);
                    sounds.len() - 1
                });
                decoded.insert((bank.name, bank.samples[i].name), sound);
            }
        }
        let mut effects = HashMap::new();
        for (name, samples, mut effect) in wanted {
            effect.samples = samples.iter().filter_map(|k| decoded.get(k).copied().flatten()).collect();
            if !effect.samples.is_empty() {
                effects.insert(name.to_owned(), effect);
            }
        }
        Ok(Self {
            sounds,
            effects,
            front_end,
        })
    }

    /// The effect a cue named `name` plays in this set (0x440E10 prefixes
    /// `low_` in a race).
    pub fn effect(&self, name: &str) -> Option<&Effect> {
        if self.front_end {
            self.effects.get(name)
        } else {
            self.effects.get(&format!("low_{name}"))
        }
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
    /// Menu widgets (docs/AUDIO_RESEARCH.md "Menu sounds"): a list cursor
    /// move (0x4910DF), an itemflow value step (0x474100) or a step at its
    /// end (0x474130), activate (0x491151) and the screen's back (0x479930).
    UdlrMove,
    ItemDiscrete,
    DeadEnd,
    Select,
    Back,
    /// The livery designer's Randomise (0x5043F0, 0x504230: `randomize`,
    /// third argument 0; front-end set only).
    Randomize,
    /// Base screen enter (0x4799E0) and leave (0x479A10).
    ScreenWhooshIn,
    ScreenWhooshOut,
    /// An overlay panel shown (0x49CFA0) or hidden (0x4936A0, 0x4942F0).
    OverlayOn,
    OverlayOff,
    /// The loading overlay's "Loading" text shown and hidden, and its
    /// circle starting to turn (FEMappings `loading_text_in`/`_out`,
    /// `spinner`; the race set has the text cues only).
    LoadingTextIn,
    LoadingTextOut,
    Spinner,
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
            Cue::UdlrMove => "UDLR_move".into(),
            Cue::ItemDiscrete => "item_discrete".into(),
            Cue::DeadEnd => "dead_end".into(),
            Cue::Select => "select".into(),
            Cue::Back => "back".into(),
            Cue::Randomize => "randomize".into(),
            Cue::ScreenWhooshIn => "screen_whoosh_in".into(),
            Cue::ScreenWhooshOut => "screen_whoosh_out".into(),
            Cue::OverlayOn => "overlay_on".into(),
            Cue::OverlayOff => "overlay_off".into(),
            Cue::LoadingTextIn => "loading_text_in".into(),
            Cue::LoadingTextOut => "loading_text_out".into(),
            Cue::Spinner => "spinner".into(),
        }
    }

    /// Play's third argument: false for the callers that pass 0 (select,
    /// back, dead end), which may not retrigger an effect still playing or
    /// remembered by a free slot.
    pub fn continuous(self) -> bool {
        !matches!(self, Cue::Select | Cue::Back | Cue::DeadEnd | Cue::Randomize)
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
    /// The effect last started here (+0x76C), kept after the voice ends.
    effect: Option<u32>,
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

    /// Play(name, 0, continuous) of 0x43E770. With no free slot the play is
    /// dropped. A non-continuous play walks the slots in order: a free slot
    /// that remembers this effect, or a busy one playing it, drops the play;
    /// other free slots forget theirs. So select, back and dead end replay
    /// only after another HUD effect took their slot. Returns whether a
    /// voice started.
    pub fn play(&mut self, cue: Cue) -> bool {
        let name = cue.name();
        let Some(effect) = self.bank.effect(&name).cloned() else {
            return false;
        };
        let id = self.effect_id(&name);
        if !effect.monophonic && !cue.continuous() {
            for slot in &mut self.slots {
                if slot.effect == Some(id) {
                    return false;
                }
                if slot.sound.is_none() {
                    slot.effect = None;
                }
            }
        }
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
            effect: Some(id),
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

    fn synthetic(front_end: bool) -> Arc<Bank> {
        let effect = |sample| Effect {
            samples: vec![sample],
            volume_db: 0.,
            pitch: 1.,
            monophonic: false,
        };
        let prefix = if front_end { "" } else { "low_" };
        Arc::new(Bank {
            sounds: (0..2)
                .map(|_| Sound {
                    pcm: vec![0.5; 100].into(),
                    channels: 1,
                    rate: 48_000,
                    looped: false,
                })
                .collect(),
            effects: HashMap::from([
                (format!("{prefix}select"), effect(0)),
                (format!("{prefix}UDLR_move"), effect(1)),
            ]),
            front_end,
        })
    }

    #[test]
    fn a_non_continuous_effect_replays_only_after_another_took_its_slot() {
        let mut player = Player::new(synthetic(true), 48_000);
        let finish = |p: &mut Player| {
            while p.active() > 0 {
                p.frame();
            }
        };
        assert!(player.play(Cue::Select));
        assert!(!player.play(Cue::Select), "still playing");
        finish(&mut player);
        assert!(!player.play(Cue::Select), "its free slot remembers it");
        // A continuous effect retriggers freely and takes the first free slot.
        assert!(player.play(Cue::UdlrMove));
        assert!(player.play(Cue::UdlrMove));
        assert_eq!(player.active(), 2);
        assert!(player.play(Cue::Select));
        finish(&mut player);
        // Select ended in slot 2; a move plays in slot 0, so select is still
        // remembered by slot 2 until another effect takes that slot.
        assert!(!player.play(Cue::Select));
        assert!(player.play(Cue::UdlrMove) && player.play(Cue::UdlrMove) && player.play(Cue::UdlrMove));
        assert!(player.play(Cue::Select));
    }

    #[test]
    fn a_race_set_plays_the_low_variants() {
        let race = synthetic(false);
        assert!(race.effect("select").is_some() && race.effect("low_select").is_none());
        let front = synthetic(true);
        assert!(front.effect("select").is_some());
        assert!(Player::new(race, 48_000).play(Cue::Select));
        assert!(Cue::UdlrMove.continuous() && !Cue::Back.continuous() && !Cue::DeadEnd.continuous());
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn shipped_front_end_menu_effects_load() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let front = Bank::load_front_end(Path::new(&root)).unwrap();
        let race = Bank::load(Path::new(&root)).unwrap();
        let menu = [
            Cue::UdlrMove,
            Cue::ItemDiscrete,
            Cue::DeadEnd,
            Cue::Select,
            Cue::Back,
            Cue::ScreenWhooshIn,
            Cue::ScreenWhooshOut,
            Cue::OverlayOn,
            Cue::OverlayOff,
        ];
        assert!(front.effect(&Cue::Randomize.name()).is_some());
        for cue in menu {
            assert!(front.effect(&cue.name()).is_some(), "front {cue:?}");
            assert!(race.effect(&cue.name()).is_some(), "race {cue:?}");
        }
        assert_eq!(front.effect("UDLR_move").unwrap().volume_db, -13.4);
        assert_eq!(front.effect("back").unwrap().volume_db, -1.);
        assert_eq!(race.effect("select").unwrap().volume_db, -4.4);
        assert_eq!(race.effect("back").unwrap().volume_db, -4.4);
        // The loading overlay: text in/out in both sets, the spinner in the
        // front end only (FEMappings has no `low_spinner`).
        assert_eq!(front.effect(&Cue::LoadingTextIn.name()).unwrap().volume_db, -6.4);
        assert_eq!(front.effect(&Cue::LoadingTextOut.name()).unwrap().volume_db, -9.);
        assert_eq!(front.effect(&Cue::Spinner.name()).unwrap().volume_db, -7.4);
        assert_eq!(race.effect(&Cue::LoadingTextIn.name()).unwrap().volume_db, -3.4);
        assert_eq!(race.effect(&Cue::LoadingTextOut.name()).unwrap().volume_db, -5.8);
        assert!(race.effect(&Cue::Spinner.name()).is_none());
        println!(
            "front effects={} sounds={}; race effects={} sounds={}",
            front.effects.len(),
            front.sounds.len(),
            race.effects.len(),
            race.sounds.len()
        );
        let mut player = Player::new(front, 48_000);
        player.group_db = -9.;
        assert!(player.play(Cue::ScreenWhooshIn));
        let mut peak = 0f32;
        for _ in 0..48_000 {
            peak = peak.max(player.frame()[0].abs());
        }
        assert!(peak > 0.001 && peak < 1., "{peak}");
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn shipped_hud_effects_load_and_play() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let bank = Bank::load(Path::new(&root)).unwrap();
        assert!(bank.sounds.iter().all(|s| s.rate == 40_000));
        assert!(bank.sounds.iter().any(|s| s.channels == 2));
        // Race lights are +6 dB in the file, clamped to 0 by Play.
        let red = bank.effect("race_light_red").unwrap();
        assert_eq!((red.volume_db, red.pitch), (0., 1.));
        assert_eq!(bank.effect("drift_combo_bank").unwrap().volume_db, -1.);
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
            assert!(bank.effect(&cue.name()).is_some(), "{cue:?}");
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
