//! Location ambience (docs/AUDIO_RESEARCH.md §30): the track model's
//! AutoPlay field (`grid_tools::ambience_audio`) runs once per vehicle tick
//! around the player's car and its emitters play on the Ambience group (12),
//! heard from the camera (433AA0) with the A23B70 distance law. Tracks whose
//! location has no AutoPlay install nothing. Presentation only.
use bevy::{
    audio::{AddAudioSource, ChannelCount, SampleRate, Source},
    prelude::*,
};
use grid_tools::{
    ambience_audio::{AMBIENCE, AutoPlay, Event, Field, Listener, Mixer},
    audio_mix::{self, Table},
    source_vehicle::DELTA,
};
use std::{
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender, channel},
    },
    time::Duration,
};

const RATE: u32 = grid_tools::engine_audio::RATE;

/// For the audio thread: a slot's start, level (linear gain, pan) or end,
/// or every voice silent until its next level.
enum Message {
    Start(usize, usize, bool),
    Level(usize, f32, [f32; 2]),
    Stop(usize),
    Hush,
}

#[derive(Resource)]
pub(crate) struct AmbienceSound {
    field: Field,
    table: Option<Arc<Table>>,
    messages: Sender<Message>,
    last: Option<(u64, u64)>,
    hushed: bool,
    events: Vec<Event>,
}

#[derive(Asset, TypePath)]
pub(crate) struct AmbienceSource {
    play: Arc<AutoPlay>,
    messages: Mutex<Option<Receiver<Message>>>,
}
pub(crate) struct Decoder {
    mixer: Mixer,
    messages: Option<Receiver<Message>>,
    count: usize,
    right: Option<f32>,
}
impl Iterator for Decoder {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if let Some(right) = self.right.take() {
            return Some(right);
        }
        if self.count.is_multiple_of(64)
            && let Some(messages) = &self.messages
        {
            while let Ok(message) = messages.try_recv() {
                match message {
                    Message::Start(slot, sound, looped) => self.mixer.start(slot, sound, looped),
                    Message::Level(slot, gain, pan) => self.mixer.level(slot, gain, pan),
                    Message::Stop(slot) => self.mixer.stop(slot),
                    Message::Hush => self.mixer.hush(),
                }
            }
        }
        self.count = self.count.wrapping_add(1);
        let [left, right] = self.mixer.sample().map(|v| if v.is_finite() { v } else { 0. });
        self.right = Some(right);
        Some(left)
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
impl Decodable for AmbienceSource {
    type Decoder = Decoder;
    fn decoder(&self) -> Decoder {
        Decoder {
            mixer: Mixer::new(self.play.clone(), RATE),
            messages: self.messages.lock().ok().and_then(|mut s| s.take()),
            count: 0,
            right: None,
        }
    }
}

#[derive(Resource)]
struct PendingSource(Option<AmbienceSource>);

pub(crate) fn install(app: &mut App, play: Arc<AutoPlay>, table: Option<Arc<Table>>) {
    app.add_audio_source::<AmbienceSource>();
    install_systems(app, play, table);
}
/// `install` without the device playback (tests).
pub(crate) fn install_systems(app: &mut App, play: Arc<AutoPlay>, table: Option<Arc<Table>>) {
    let (messages, receiver) = channel();
    let source = AmbienceSource { play: play.clone(), messages: Mutex::new(Some(receiver)) };
    app.insert_resource(AmbienceSound {
        field: Field::new(play, 0x2468_ace1),
        table,
        messages,
        last: None,
        hushed: false,
        events: Vec::new(),
    })
    .insert_resource(PendingSource(Some(source)))
    .add_systems(Startup, start)
    .add_systems(FixedUpdate, tick.after(super::drive_tick))
    .add_systems(Update, audibility);
}

fn start(mut commands: Commands, mut pending: ResMut<PendingSource>, mut assets: ResMut<Assets<AmbienceSource>>) {
    if let Some(source) = pending.0.take() {
        let name = source.play.name.clone();
        // ONCE: the decoder never ends (F482).
        commands.spawn((AudioPlayer(assets.add(source)), PlaybackSettings::ONCE));
        println!("ambience_audio version=1 event=queued rate={RATE} autoplay={name} group=Ambience");
    }
}

/// Once per successful vehicle tick: 0x44D420 around the player's car.
fn tick(mut sound: ResMut<AmbienceSound>, driving: Res<super::Driving>, listening: super::engine_sound::Listening) {
    let now = (driving.segment, driving.runtime.ticks);
    if sound.last == Some(now) || now.1 == 0 || driving.paused || driving.failure.is_some() {
        sound.last = Some(now);
        return;
    }
    sound.last = Some(now);
    sound.hushed = false;
    let sound = &mut *sound;
    let pose = driving.runtime.pose();
    let car = Listener::of(&pose, driving.runtime.velocity());
    sound.events.clear();
    sound.field.tick(&car, DELTA, &mut sound.events);
    let (trim, camera, modes) = (listening.trim(), listening.camera(), listening.modes());
    let group = sound.table.as_ref().map_or(-22., |t| modes.group_db(t, camera, AMBIENCE, trim));
    // The Ambience group is heard from the camera listener (433AA0).
    let ear = listening.ear(&pose);
    for event in sound.events.drain(..) {
        let message = match event {
            Event::Start { slot, sound, looped } => Message::Start(slot, sound, looped),
            Event::Stop { slot } => Message::Stop(slot),
            Event::Level { slot, db, position, min_distance, max_distance } => {
                let at = Vec3::from_array(position);
                let gain = audio_mix::backend_gain(db, group, audio_mix::distance_db(at.distance(ear), min_distance, max_distance));
                Message::Level(slot, gain, listening.heard(ear, at))
            }
        };
        let _ = sound.messages.send(message);
    }
}

/// As the collision loops (backlog row 95): once the vehicle ticks stop the
/// emitters would hold their last level, so they glide to silence and
/// return with the next tick. The original's Paused and EndOfRace modes
/// drop the Ambience group to −48 dB (AUDIO_RESEARCH §2).
fn audibility(mut sound: ResMut<AmbienceSound>, driving: Res<super::Driving>, listening: super::engine_sound::Listening) {
    let audible = !driving.paused && driving.failure.is_none() && !listening.stopped(&driving);
    if !audible && !sound.hushed {
        let _ = sound.messages.send(Message::Hush);
        sound.hushed = true;
    }
}

/// The installed field's voices (tests).
#[cfg(test)]
pub(crate) fn active(sound: &AmbienceSound) -> usize {
    sound.field.active()
}
