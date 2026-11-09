//! Crowds (docs/AUDIO_RESEARCH.md §31): the original's crowd object
//! (`grid_tools::crowd_audio`) runs once per vehicle tick on the route's crowd cells, with the
//! race events' excitement from the speech manager's ReactiveCrowd messages. Each voice keeps
//! the level and pitch it started with and is heard from the camera on the Crowd group (11),
//! with the A23B70 distance law. Every car's contacts feed the visual crowd's recoil records,
//! whose hard wall hits make the nearest cells gasp. Presentation only.
use bevy::{
    audio::{AddAudioSource, ChannelCount, SampleRate, Source},
    prelude::*,
};
use grid_tools::{
    ambience_audio::Mixer,
    audio_mix::{self, Table},
    crowd_audio::{Bank, CROWD, Cells, Crowd, Event, Listener, Recoils},
    hud_audio::Sound,
    source_vehicle::{DELTA, multi::Contacted},
};
use std::{
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender, channel},
    },
    time::Duration,
};

const RATE: u32 = grid_tools::engine_audio::RATE;

enum Message {
    Start(usize, usize, bool, f32),
    Level(usize, f32, [f32; 2], f32),
    Stop(usize),
    Hush,
}

#[derive(Resource)]
pub(crate) struct CrowdSound {
    crowd: Crowd,
    cells: Cells,
    table: Option<Arc<Table>>,
    messages: Sender<Message>,
    last: Option<(u64, u64)>,
    hushed: bool,
    events: Vec<Event>,
    recoils: Recoils,
}

#[derive(Asset, TypePath)]
pub(crate) struct CrowdSource {
    sounds: Arc<[Sound]>,
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
                    Message::Start(slot, sound, looped, pitch) => self.mixer.start_pitched(slot, sound, looped, pitch),
                    Message::Level(slot, gain, pan, doppler) => {
                        self.mixer.level(slot, gain, pan);
                        self.mixer.doppler(slot, doppler);
                    }
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
impl Decodable for CrowdSource {
    type Decoder = Decoder;
    fn decoder(&self) -> Decoder {
        Decoder {
            mixer: Mixer::new(self.sounds.clone(), RATE),
            messages: self.messages.lock().ok().and_then(|mut s| s.take()),
            count: 0,
            right: None,
        }
    }
}

#[derive(Resource)]
struct PendingSource(Option<CrowdSource>);

pub(crate) fn install(app: &mut App, bank: Arc<Bank>, cells: Cells, table: Option<Arc<Table>>) {
    app.add_audio_source::<CrowdSource>();
    install_systems(app, bank, cells, table);
}
/// `install` without the device playback (tests).
pub(crate) fn install_systems(app: &mut App, bank: Arc<Bank>, cells: Cells, table: Option<Arc<Table>>) {
    let (messages, receiver) = channel();
    let source = CrowdSource { sounds: bank.sounds.clone(), messages: Mutex::new(Some(receiver)) };
    app.insert_resource(CrowdSound {
        crowd: Crowd::new(bank, 0x1357_2468),
        cells,
        table,
        messages,
        last: None,
        hushed: false,
        events: Vec::new(),
        recoils: Recoils::default(),
    })
    .insert_resource(PendingSource(Some(source)))
    .add_systems(Startup, start)
    .add_systems(FixedUpdate, tick.after(super::drive_tick))
    .add_systems(Update, audibility);
}

fn start(mut commands: Commands, mut pending: ResMut<PendingSource>, mut assets: ResMut<Assets<CrowdSource>>) {
    if let Some(source) = pending.0.take() {
        // ONCE: the decoder never ends (F482).
        commands.spawn((AudioPlayer(assets.add(source)), PlaybackSettings::ONCE));
        println!("crowd_audio version=1 event=queued rate={RATE} group=Crowd");
    }
}

/// Once per successful vehicle tick: 0x44E220 for the player's car.
fn tick(
    mut sound: ResMut<CrowdSound>,
    driving: Res<super::Driving>,
    listening: super::engine_sound::Listening,
    speech: Option<ResMut<super::speech_sound::SpeechSound>>,
) {
    let sound = &mut *sound;
    // The ReactiveCrowd events since the last tick (0x424B60 → 0x41F2D0).
    if let Some(mut speech) = speech {
        for (level, hold) in speech.crowd.drain(..) {
            sound.crowd.excite(level, hold);
        }
    }
    let now = (driving.segment, driving.runtime.ticks);
    if sound.last == Some(now) || now.1 == 0 || driving.paused || driving.failure.is_some() {
        sound.last = Some(now);
        return;
    }
    sound.last = Some(now);
    sound.hushed = false;
    let pose = driving.runtime.pose();
    let v = driving.runtime.velocity();
    let car = Listener {
        position: [pose[12], pose[13], pose[14]],
        forward: [pose[8], pose[9], pose[10]],
        speed: (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt(),
    };
    let modes = listening.modes();
    let ear = listening.ear(&pose);
    // The visual crowd's frame (0x608940, 0x76A620): last tick's records broadcast, then this
    // tick's contacts of every car are recorded (0x756E40).
    let mut recoils = Vec::new();
    sound.recoils.frame(DELTA, &mut recoils);
    for r in &recoils {
        sound.crowd.recoil(r);
    }
    let mut record = |k: usize, impacts: Vec<(grid_tools::body_impacts::Impact, Contacted)>| {
        for (i, other) in impacts {
            sound.recoils.add(k, i.point, i.normal, i.impact, !matches!(other, Contacted::Car(_)));
        }
    };
    match &driving.opponents {
        Some(o) => (0..o.multi.cars.len()).for_each(|k| record(k, o.multi.contact_impacts(k))),
        None => record(0, driving.runtime.contact_impacts()),
    }
    sound.events.clear();
    sound.crowd.tick(&sound.cells, &car, modes.mode(), ear.to_array(), DELTA, &mut sound.events);
    for event in sound.events.drain(..) {
        match event {
            Event::Start { slot } => {
                if let Some((_, e)) = sound.crowd.emitters().find(|(s, _)| *s == slot) {
                    let _ = sound.messages.send(Message::Start(slot, e.sound, e.looped, e.pitch));
                }
            }
            Event::Stop { slot } => {
                let _ = sound.messages.send(Message::Stop(slot));
            }
        }
    }
    // Every voice's level from the camera (433AA0: Crowd is on the Camera listener).
    let (trim, camera) = (listening.trim(), listening.camera());
    let group = sound.table.as_ref().map_or(-3., |t| modes.group_db(t, camera, CROWD, trim));
    // A still voice heard by the moving camera (0xA233D0: velocity × Doppler).
    let listener = listening.camera_velocity();
    for (slot, e) in sound.crowd.emitters() {
        let at = Vec3::from_array(e.position);
        let gain = audio_mix::backend_gain(e.db, group, audio_mix::distance_db(at.distance(ear), e.min_distance, e.max_distance));
        let doppler = audio_mix::doppler((at - ear).to_array(), (-listener * e.doppler).to_array());
        let _ = sound.messages.send(Message::Level(slot, gain, listening.heard(ear, at), doppler));
    }
}

/// As the ambience: silent until the next tick once the ticks stop (backlog row 95).
fn audibility(mut sound: ResMut<CrowdSound>, driving: Res<super::Driving>, listening: super::engine_sound::Listening) {
    let audible = !driving.paused && driving.failure.is_none() && !listening.stopped(&driving);
    if !audible && !sound.hushed {
        let _ = sound.messages.send(Message::Hush);
        sound.hushed = true;
    }
}

/// The crowd's voices and excitement (tests).
#[cfg(test)]
pub(crate) fn state(sound: &CrowdSound) -> (usize, f32) {
    (sound.crowd.emitters().count(), sound.crowd.excitement())
}
/// Whether a gasp runs (tests).
#[cfg(test)]
pub(crate) fn gasping(sound: &CrowdSound) -> bool {
    sound.crowd.gasping()
}
/// A hard wall hit at the centre of the populated cell nearest `at`, recorded as a contact of a
/// car outside the field (tests): the next tick broadcasts it (0x76A620).
#[cfg(test)]
pub(crate) fn hit_wall_near(sound: &mut CrowdSound, at: [f32; 3]) {
    let distance = |c: [f32; 3]| (0..3).map(|k| (at[k] - c[k]).powi(2)).sum::<f32>();
    let centre = sound
        .cells
        .cells
        .iter()
        .filter(|c| c.people > 5)
        .map(|c| c.centre)
        .min_by(|a, b| distance(*a).total_cmp(&distance(*b)))
        .expect("a populated crowd cell");
    sound.recoils.add(usize::MAX, centre, [1., 0., 0.], 20_000., true);
}
/// The nearest crowd cell to `at`: its distance and people (tests).
#[cfg(test)]
pub(crate) fn nearest(sound: &CrowdSound, at: [f32; 3]) -> Option<(f32, u32)> {
    let cells = &sound.cells.cells;
    cells
        .iter()
        .filter(|c| c.people > 0)
        .map(|c| ((0..3).map(|k| (at[k] - c.centre[k]).powi(2)).sum::<f32>().sqrt(), c.people))
        .min_by(|a, b| a.0.total_cmp(&b.0))
}
