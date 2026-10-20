//! HUD sounds (docs/AUDIO_RESEARCH.md §7.1): the original's 2D HUD
//! one-shots from `FEMappings.bin` on the HUD group: start lights, the
//! player's drift scorer and the flashback transition. Presentation only.
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

#[derive(Resource)]
pub(crate) struct HudSound {
    table: Option<Arc<Table>>,
    cues: Sender<Cue>,
    group_db: Arc<AtomicU32>,
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
    let (cues, receiver) = channel();
    let group_db = Arc::new(AtomicU32::new(audio_mix::SILENT.to_bits()));
    let source = HudSource {
        bank,
        cues: std::sync::Mutex::new(Some(receiver)),
        group_db: group_db.clone(),
    };
    app.insert_resource(HudSound {
        table,
        cues,
        group_db,
        countdown: None,
        green: false,
        flash: None,
    })
    .insert_resource(PendingSource(Some(source)))
    .add_systems(Startup, start)
    .add_systems(Update, update);
}

#[derive(Resource)]
struct PendingSource(Option<HudSource>);

fn start(mut commands: Commands, mut pending: ResMut<PendingSource>, mut assets: ResMut<Assets<HudSource>>) {
    if let Some(source) = pending.0.take() {
        // ONCE: the decoder never ends; LOOP's rodio `repeat_infinite` would
        // keep every sample played to replay it (F482).
        commands.spawn((AudioPlayer(assets.add(source)), PlaybackSettings::ONCE));
        println!("hud_audio version=1 event=queued rate={RATE} group=HUD cues=lights,drift,flashback");
    }
}

fn update(
    mut sound: ResMut<HudSound>,
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
    sound.group_db.store(level.to_bits(), Ordering::Relaxed);
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
        let _ = sound.cues.send(cue);
    }
}
