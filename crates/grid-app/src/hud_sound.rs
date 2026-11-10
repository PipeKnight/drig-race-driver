//! HUD sounds (docs/AUDIO_RESEARCH.md §7.1): the original's 2D HUD
//! one-shots from `FEMappings.bin` on the HUD group: start lights, the
//! player's drift scorer and the flashback transition, and the menu widget
//! sounds of the pause menu, the pre-race screens and the front-end screens
//! (AUDIO_RESEARCH.md "Menu sounds"). Presentation only.
use bevy::{
    audio::{AddAudioSource, ChannelCount, SampleRate, Source},
    prelude::*,
};
use grid_tools::{
    audio_mix::{self, Table},
    hud_audio::{self, Bank, Cue, Player},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    time::Duration,
};

const RATE: u32 = grid_tools::engine_audio::RATE;

/// The HUD voice queue and its group level, shared by every caller.
#[derive(Resource, Clone)]
pub(crate) struct HudSink {
    cues: Sender<Cue>,
    group_db: Arc<AtomicU32>,
}
impl HudSink {
    pub(crate) fn play(&self, cue: Cue) {
        let _ = self.cues.send(cue);
    }
    fn level(&self, db: f32) {
        self.group_db.store(db.to_bits(), Ordering::Relaxed);
    }
}

/// Plays a menu's queued cues, or drops them without a HUD sink.
pub(crate) fn drain(sounds: &mut Vec<Cue>, sink: Option<&HudSink>) {
    for cue in sounds.drain(..) {
        if let Some(sink) = sink {
            sink.play(cue);
        }
    }
}

/// The menu widget sound of one input frame (0x43E770 callers: list widget
/// 0x490F40, itemflow 0x49DA70, slider 0x4A0CC0, screen back 0x479930):
/// `place` the screen changed or the menu opened or closed (activate, or
/// back when Back was pressed), `cursor` the selection moved (list move),
/// `value` a value changed (itemflow step), `side` left/right was pressed
/// on a value row (the itemflow's dead end when nothing changed).
pub(crate) fn menu_cue(nav: &crate::pause_menu::Nav, place: bool, cursor: bool, value: bool, side: bool) -> Option<Cue> {
    if place {
        Some(if nav.back || nav.pause { Cue::Back } else { Cue::Select })
    } else if cursor {
        Some(Cue::UdlrMove)
    } else if value {
        Some(Cue::ItemDiscrete)
    } else if side && (nav.left || nav.right) {
        Some(Cue::DeadEnd)
    } else {
        None
    }
}

#[derive(Resource)]
pub(crate) struct HudSound {
    table: Option<Arc<Table>>,
    /// Countdown seconds already passed (start lights) and the last
    /// flashback flash seen.
    countdown: Option<f64>,
    green: bool,
    flash: Option<usize>,
}

#[derive(Asset, TypePath)]
pub(crate) struct HudSource {
    bank: Arc<Bank>,
    cues: std::sync::Mutex<Option<Receiver<Cue>>>,
    group_db: Arc<AtomicU32>,
}
pub(crate) struct Decoder {
    player: Player,
    cues: Option<Receiver<Cue>>,
    group_db: Arc<AtomicU32>,
    frame: [f32; 2],
    channel: usize,
    count: usize,
}
impl Iterator for Decoder {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.channel == 0 {
            if self.count.is_multiple_of(64) {
                if let Some(cues) = &self.cues {
                    while let Ok(cue) = cues.try_recv() {
                        self.player.play(cue);
                    }
                }
                self.player.group_db = f32::from_bits(self.group_db.load(Ordering::Relaxed));
            }
            self.count = self.count.wrapping_add(1);
            self.frame = self.player.frame();
        }
        let value = self.frame[self.channel];
        self.channel ^= 1;
        Some(if value.is_finite() { value } else { 0. })
    }
}
impl Source for Decoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(RATE).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}
impl Decodable for HudSource {
    type Decoder = Decoder;
    fn decoder(&self) -> Decoder {
        Decoder {
            player: Player::new(self.bank.clone(), RATE),
            // One decoder owns the queue; a second decoder plays nothing.
            cues: self.cues.lock().ok().and_then(|mut c| c.take()),
            group_db: self.group_db.clone(),
            frame: [0.; 2],
            channel: 0,
            count: 0,
        }
    }
}

pub(crate) fn install(app: &mut App, bank: Arc<Bank>, table: Option<Arc<Table>>) {
    app.add_audio_source::<HudSource>();
    install_systems(app, bank, table);
}
/// `install` without the device playback (the race-start mix test).
pub(crate) fn install_systems(app: &mut App, bank: Arc<Bank>, table: Option<Arc<Table>>) {
    sink(app, bank, "lights,drift,flashback,menu");
    app.insert_resource(HudSound {
        table,
        countdown: None,
        green: false,
        flash: None,
    })
    .add_systems(Update, update);
}

/// The front-end screens' HUD voices (the unprefixed `fe_fx` set) at the
/// HUD group of the front-end camera, trimmed by Effects Volume, with the
/// base screen's whoosh in on entry and out when the screen fades out
/// (`transition::Fade::leaving`). Nothing without an audio device.
pub(crate) fn install_front_end(app: &mut App, root: &std::path::Path, effects: u8) {
    if crate::frame_pacing::audio_off() || !app.is_plugin_added::<bevy::audio::AudioPlugin>() {
        return;
    }
    let bank = match Bank::load_front_end(root) {
        Ok(bank) => bank,
        Err(error) => {
            warn!("Front-end sounds unavailable: {error}");
            return;
        }
    };
    let level = match Table::load(root) {
        Ok(t) => front_end_db(&t, effects),
        Err(error) => {
            warn!("Audio group mix unavailable; front-end sounds at -9 dB: {error}");
            -9.
        }
    };
    app.add_audio_source::<HudSource>();
    sink(app, bank, "menu,screen");
    app.world().resource::<HudSink>().level(level);
    app.add_systems(Update, screen_sounds);
}

/// The HUD group level on a front-end screen (Normal mode, FrontEndCamera).
pub(crate) fn front_end_db(table: &Table, effects: u8) -> f32 {
    table.group_db(
        audio_mix::NORMAL,
        audio_mix::Camera::FrontEnd,
        hud_audio::GROUP,
        audio_mix::slider_db(f32::from(effects) / 100.),
    )
}

/// Whoosh in on the first frame, out when the screen starts to leave.
pub(crate) fn screen_cue(opened: &mut bool, left: &mut bool, leaving: bool) -> Option<Cue> {
    if !*opened {
        *opened = true;
        return Some(Cue::ScreenWhooshIn);
    }
    if leaving && !*left {
        *left = true;
        return Some(Cue::ScreenWhooshOut);
    }
    None
}

fn screen_sounds(
    sink: Res<HudSink>,
    fade: Option<Res<crate::transition::Fade>>,
    mut state: Local<(bool, bool)>,
) {
    let (opened, left) = &mut *state;
    if let Some(cue) = screen_cue(opened, left, fade.is_some_and(|f| f.leaving())) {
        sink.play(cue);
    }
}

fn sink(app: &mut App, bank: Arc<Bank>, cues: &'static str) {
    let (sender, receiver) = channel();
    let group_db = Arc::new(AtomicU32::new(audio_mix::SILENT.to_bits()));
    let source = HudSource {
        bank,
        cues: std::sync::Mutex::new(Some(receiver)),
        group_db: group_db.clone(),
    };
    app.insert_resource(HudSink { cues: sender, group_db })
        .insert_resource(PendingSource(Some(source), cues))
        .add_systems(Startup, start);
}

#[derive(Resource)]
struct PendingSource(Option<HudSource>, &'static str);

fn start(mut commands: Commands, mut pending: ResMut<PendingSource>, mut assets: ResMut<Assets<HudSource>>) {
    if let Some(source) = pending.0.take() {
        // ONCE: the decoder never ends; LOOP's rodio `repeat_infinite` would
        // keep every sample played to replay it (F482).
        commands.spawn((AudioPlayer(assets.add(source)), PlaybackSettings::ONCE));
        println!("hud_audio version=1 event=queued rate={RATE} group=HUD cues={}", pending.1);
    }
}

fn update(
    mut sound: ResMut<HudSound>,
    sink: Res<HudSink>,
    listening: super::engine_sound::Listening,
    start: Option<Res<super::StartSequence>>,
    drift: Option<ResMut<super::drift_event::DriftEvent>>,
    flashback: Option<Res<super::flashback::Flashback>>,
) {
    use grid_tools::time_trial::Phase;
    // HUD group: the mode term (Paused keeps 0 for HUD), camera and Effects trim.
    let modes = listening.modes();
    let level = sound.table.as_ref().map_or(-9., |t| {
        modes.group_db(t, listening.camera(), hud_audio::GROUP, listening.trim())
    });
    sink.level(level);
    let mut cues = Vec::new();
    // Start lights: the original's 1 s stage timer from the countdown start.
    let (seconds, stages) = start.map_or((super::START_SEQUENCE_SECONDS, 6), |s| (s.seconds, s.textures.len()));
    match listening.trial.as_ref().map(|t| t.state.phase()) {
        Some(Phase::Countdown(left)) => {
            let remaining = f64::from(*left) * f64::from(grid_tools::source_vehicle::DELTA);
            let elapsed = (seconds - remaining).max(0.);
            if let Some(cue) = hud_audio::light_cue(sound.countdown, elapsed, stages)
                && cue == Cue::LightRed
            {
                cues.push(cue);
            }
            sound.countdown = Some(elapsed);
            sound.green = false;
        }
        Some(Phase::Running) if sound.countdown.is_some() && !sound.green => {
            sound.green = true;
            sound.countdown = None;
            cues.push(Cue::LightGreen);
        }
        _ => {}
    }
    // The player's drift scorer.
    if let Some(mut drift) = drift
        && let Some(player) = drift.entries.first_mut()
    {
        cues.append(&mut player.scorer.cues);
    }
    // Flashback flashes: shot 1, shot 2, zoom up (states.bin playSample).
    let flash = flashback.and_then(|f| match f.stage {
        super::flashback::Stage::Flash { index, .. } => Some(index),
        _ => None,
    });
    if flash != sound.flash
        && let Some(index) = flash
    {
        cues.push(match index {
            0 => Cue::FlashbackShot1,
            1 => Cue::FlashbackShot2,
            _ => Cue::FlashbackZoomUp,
        });
    }
    sound.flash = flash;
    for cue in cues {
        sink.play(cue);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pause_menu::Nav;

    #[test]
    fn a_menu_frame_plays_one_widget_sound() {
        let nav = |f: fn(&mut Nav)| {
            let mut n = Nav::default();
            f(&mut n);
            n
        };
        let back = nav(|n| n.back = true);
        let right = nav(|n| n.right = true);
        assert_eq!(menu_cue(&back, true, true, true, true), Some(Cue::Back));
        assert_eq!(menu_cue(&nav(|n| n.pause = true), true, false, false, false), Some(Cue::Back));
        assert_eq!(menu_cue(&nav(|n| n.select = true), true, true, false, false), Some(Cue::Select));
        assert_eq!(menu_cue(&right, false, true, true, true), Some(Cue::UdlrMove));
        assert_eq!(menu_cue(&right, false, false, true, true), Some(Cue::ItemDiscrete));
        assert_eq!(menu_cue(&right, false, false, false, true), Some(Cue::DeadEnd));
        assert_eq!(menu_cue(&right, false, false, false, false), None);
        assert_eq!(menu_cue(&back, false, false, false, true), None);
    }

    #[test]
    fn a_screen_whooshes_in_once_and_out_once() {
        let (mut opened, mut left) = (false, false);
        let mut cues = Vec::new();
        for leaving in [false, false, true, true] {
            cues.extend(screen_cue(&mut opened, &mut left, leaving));
        }
        assert_eq!(cues, [Cue::ScreenWhooshIn, Cue::ScreenWhooshOut]);
    }
}
