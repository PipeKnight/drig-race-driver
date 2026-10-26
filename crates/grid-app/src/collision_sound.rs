//! Car collision sounds (docs/AUDIO_RESEARCH.md §9): once per vehicle tick
//! (as the engine and tyre parameters, F453) every car's solved contacts go
//! through the original's collision listener (`grid_tools::collision_audio`)
//! and the one-shots play on the player's Panel Coll group or the AI group,
//! with the A23B70 distance law; each car's scrape loop (453710/458780)
//! follows on the same groups. The player's damage and suspension sounds
//! (`grid_tools::damage_audio`, §27) use the same bank and mixer.
//! Presentation only, never feeds physics.
use bevy::{
    audio::{AddAudioSource, ChannelCount, SampleRate, Source},
    prelude::*,
};
use grid_tools::{
    audio_mix::{self, Table},
    collision_audio::{Bank, Car, Heights, LOOP_SLOTS, Listener, Mixer, contacts_of},
    damage_audio::{self, Damage, Place},
    source_vehicle::{DELTA, Runtime, multi::Contacted},
};
use std::{
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender, channel},
    },
    time::Duration,
};

const RATE: u32 = grid_tools::engine_audio::RATE;

/// For the audio thread: a one-shot (variety, pitch, linear gain, pan),
/// loop slot k's level and pan (car k's scrape below `LOOP_SLOTS`), or the
/// end of a loop.
enum Start {
    Shot(String, f32, f32, [f32; 2]),
    Scrape(usize, String, f32, f32, [f32; 2]),
    Stop(usize),
}

#[derive(Resource)]
pub(crate) struct CollisionSound {
    bank: Arc<Bank>,
    table: Option<Arc<Table>>,
    listener: Listener,
    /// Heights per field model; `car_model[k]` selects car k's.
    heights: Vec<Heights>,
    car_model: Vec<usize>,
    /// Per field model: the carsetup EngineBay and Cockpit offsets.
    emitters: Vec<[[f32; 3]; 2]>,
    /// The player's damage sounds.
    damage: Damage,
    starts: Sender<Start>,
    last: Option<(u64, u64)>,
}

#[derive(Asset, TypePath)]
pub(crate) struct CollisionSource {
    bank: Arc<Bank>,
    starts: Mutex<Option<Receiver<Start>>>,
}
pub(crate) struct Decoder {
    mixer: Mixer,
    starts: Option<Receiver<Start>>,
    count: usize,
    /// The right channel of the current frame, returned next.
    right: Option<f32>,
}
impl Iterator for Decoder {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if let Some(right) = self.right.take() {
            return Some(right);
        }
        if self.count.is_multiple_of(64)
            && let Some(starts) = &self.starts
        {
            while let Ok(start) = starts.try_recv() {
                match start {
                    Start::Shot(variety, pitch, gain, pan) => {
                        self.mixer.start(&variety, pitch, gain, pan);
                    }
                    Start::Scrape(slot, variety, pitch, gain, pan) => self.mixer.looped(slot, &variety, pitch, gain, pan),
                    Start::Stop(slot) => self.mixer.stop(slot),
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
impl Decodable for CollisionSource {
    type Decoder = Decoder;
    fn decoder(&self) -> Decoder {
        Decoder {
            mixer: Mixer::new(self.bank.clone(), RATE),
            starts: self.starts.lock().ok().and_then(|mut s| s.take()),
            count: 0,
            right: None,
        }
    }
}

pub(crate) fn install(
    app: &mut App,
    bank: Arc<Bank>,
    table: Option<Arc<Table>>,
    heights: Vec<Heights>,
    emitters: Vec<[[f32; 3]; 2]>,
    car_model: Vec<usize>,
) {
    app.add_audio_source::<CollisionSource>();
    install_systems(app, bank, table, heights, emitters, car_model);
}
/// `install` without the device playback (the race-start mix test).
pub(crate) fn install_systems(
    app: &mut App,
    bank: Arc<Bank>,
    table: Option<Arc<Table>>,
    heights: Vec<Heights>,
    emitters: Vec<[[f32; 3]; 2]>,
    car_model: Vec<usize>,
) {
    let (starts, receiver) = channel();
    let source = CollisionSource { bank: bank.clone(), starts: Mutex::new(Some(receiver)) };
    app.insert_resource(CollisionSound {
        listener: Listener::new(bank.clone()),
        bank,
        table,
        heights,
        car_model,
        emitters,
        damage: Damage::default(),
        starts,
        last: None,
    })
    .insert_resource(PendingSource(Some(source)))
    .init_resource::<GlassBreaks>()
    .add_systems(Startup, start)
    .add_systems(FixedUpdate, tick.after(super::drive_tick));
}

#[derive(Resource)]
struct PendingSource(Option<CollisionSource>);

/// Windows that cracked or smashed since the last tick, as (car, window in
/// `WINDOWS` order, smashed): the damage parts' glass found from the
/// deformation, the "game broadcaster" messages 43CE90 plays.
#[derive(Resource, Default)]
pub(crate) struct GlassBreaks(pub(crate) Vec<(usize, usize, bool)>);

fn start(mut commands: Commands, mut pending: ResMut<PendingSource>, mut assets: ResMut<Assets<CollisionSource>>) {
    if let Some(source) = pending.0.take() {
        // ONCE: the decoder never ends; LOOP's rodio `repeat_infinite` would
        // keep every sample played to replay it (F482).
        commands.spawn((AudioPlayer(assets.add(source)), PlaybackSettings::ONCE));
        println!(
            "collision_audio version=2 event=queued rate={RATE} groups=PanelColl,AI impacts=front_back,sides,roof sweeteners=low,high scrapes=loop"
        );
    }
}

/// Once per successful vehicle tick: every car's contacts.
fn tick(
    mut sound: ResMut<CollisionSound>,
    mut glass: ResMut<GlassBreaks>,
    driving: Res<super::Driving>,
    listening: super::engine_sound::Listening,
) {
    let now = (driving.segment, driving.runtime.ticks);
    if sound.last.is_some_and(|(segment, _)| segment != now.0) {
        // A new segment: the player's damage voices start again.
        sound.damage = Damage::default();
        for slot in 0..damage_audio::SLOTS {
            let _ = sound.starts.send(Start::Stop(LOOP_SLOTS + slot));
        }
    }
    if sound.last == Some(now) || now.1 == 0 || driving.paused || driving.failure.is_some() {
        sound.last = Some(now);
        return;
    }
    sound.last = Some(now);
    let sound = &mut *sound;
    sound.listener.tick(DELTA);
    let mut cars: Vec<(&Runtime, Vec<(grid_tools::body_impacts::Impact, Contacted)>)> = Vec::new();
    match &driving.opponents {
        Some(o) => {
            for (k, runtime) in std::iter::once(&driving.runtime).chain(o.multi.cars.iter().skip(1)).enumerate() {
                cars.push((runtime, o.multi.contact_impacts(k)));
            }
        }
        None => cars.push((&driving.runtime, driving.runtime.contact_impacts())),
    }
    let (trim, camera, modes) = (listening.trim(), listening.camera(), listening.modes());
    let ear = listening.ear(&driving.runtime.pose());
    let group_db = |sound: &CollisionSound, group: usize| {
        sound.table.as_ref().map_or(-9., |t| modes.group_db(t, camera, group, trim))
    };
    for (k, (runtime, impacts)) in cars.into_iter().enumerate() {
        let model = sound.car_model.get(k).copied().unwrap_or(0);
        let Some(&heights) = sound.heights.get(model) else { continue };
        let solved: Vec<_> = impacts.iter().map(|(x, _)| *x).collect();
        let car = Car::of(runtime, heights, k == 0, &solved);
        let position = Vec3::new(car.pose[12], car.pose[13], car.pose[14]);
        // The player's groups use the Car listener (the car itself), the
        // AI group the camera (433AA0) [I: car pose as the Car listener].
        let listener = if k == 0 { position } else { ear };
        let mut plays = Vec::new();
        if !impacts.is_empty() {
            let contacts = contacts_of(runtime, &car, &impacts);
            let table = sound.table.clone();
            let mut crash = || table.as_deref().is_some_and(|t| listening.crash(t));
            sound.listener.contacts_crashing(k, &car, &contacts, &mut crash, &mut plays);
        }
        // 455FA0: the landing after airtime.
        {
            let table = sound.table.clone();
            let mut crash = || table.as_deref().is_some_and(|t| listening.crash(t));
            let airborne = grid_tools::collision_audio::airborne(runtime);
            sound.listener.landing(k, &car, airborne, DELTA, &mut crash, &mut plays);
        }
        // 4486E0: the player's yaw/pitch displacement.
        if k == 0 {
            let omega = grid_tools::collision_audio::angular_velocity(runtime);
            sound.listener.displacement(&car, omega, &mut plays);
        }
        // 43CE90: cracked and smashed windows.
        for &(_, window, smashed) in glass.0.iter().filter(|(car, ..)| *car == k) {
            sound.listener.glass(&car, window, smashed, &mut plays);
        }
        // 458780 after the step: the scrape loop's level, pitch and place.
        if let Some(voice) = sound.listener.scrape_frame(k, &car)
            && let Some(variety) = sound.bank.varieties.get(&voice.variety)
        {
            let distance = (position + Vec3::from_array(voice.offset)).distance(listener);
            let gain = audio_mix::backend_gain(
                voice.volume_db,
                group_db(sound, voice.group),
                audio_mix::distance_db(distance, variety.min_distance, variety.max_distance),
            );
            let pan = listening.heard(listener, position + Vec3::from_array(voice.offset));
            let _ = sound.starts.send(Start::Scrape(k, voice.variety, voice.pitch, gain, pan));
        }
        // 457BE0's player calls: suspension, engine, brake and steering damage.
        if k == 0 {
            let input = damage_audio::Input::vehicle(&runtime.state.car.owner.vehicle, runtime.velocity());
            let frame = sound.damage.frame(&sound.bank, &mut sound.listener, &input, DELTA);
            let pose = Mat4::from_cols_array(&car.pose);
            let offsets = sound.emitters.get(model).copied().unwrap_or([[0., 0., 2.], [0., 0.5, 0.]]);
            let wheels = runtime.wheels();
            let place = |p: Place| {
                pose.transform_point3(Vec3::from_array(match p {
                    Place::EngineBay => offsets[0],
                    Place::Cockpit => offsets[1],
                    Place::Wheel(i) => wheels[i.min(3)].anchor,
                }))
            };
            // [I] Engine and Other Mech heard from the camera as the engine
            // and mechanical voices are; Displacement from the car as the
            // player's other collision one-shots.
            let heard_from = |group: usize| if group == grid_tools::collision_audio::DISPLACEMENT { position } else { ear };
            for slot in frame.stops {
                let _ = sound.starts.send(Start::Stop(LOOP_SLOTS + slot));
            }
            for l in frame.loops {
                let Some(variety) = sound.bank.varieties.get(&l.variety) else { continue };
                let (at, from) = (place(l.place), heard_from(l.group));
                let gain = audio_mix::backend_gain(
                    l.volume_db,
                    group_db(sound, l.group),
                    audio_mix::distance_db(at.distance(from), variety.min_distance, variety.max_distance),
                );
                let pan = listening.heard(from, at);
                let _ = sound.starts.send(Start::Scrape(LOOP_SLOTS + l.slot, l.variety, l.pitch, gain, pan));
            }
            for shot in frame.shots {
                let Some(variety) = sound.bank.varieties.get(&shot.variety) else { continue };
                let (at, from) = (place(shot.place), heard_from(shot.group));
                let gain = audio_mix::backend_gain(
                    shot.volume_db,
                    group_db(sound, shot.group),
                    audio_mix::distance_db(at.distance(from), variety.min_distance, variety.max_distance),
                );
                if gain > 0. {
                    let pan = listening.heard(from, at);
                    let _ = sound.starts.send(Start::Shot(shot.variety, shot.pitch, gain, pan));
                }
            }
        }
        for play in plays {
            let Some(variety) = sound.bank.varieties.get(&play.variety) else { continue };
            let distance = Vec3::from_array(play.position).distance(listener);
            let gain = audio_mix::backend_gain(
                play.volume_db,
                group_db(sound, play.group),
                audio_mix::distance_db(distance, variety.min_distance, variety.max_distance),
            );
            if gain > 0. {
                // The contact relative to the listener: the player's hits
                // from the car (Car listener), AI cars' from the camera.
                let pan = listening.heard(listener, Vec3::from_array(play.position));
                let _ = sound.starts.send(Start::Shot(play.variety, play.pitch, gain, pan));
            }
        }
    }
    glass.0.clear();
}
