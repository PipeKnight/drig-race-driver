//! Car collision sounds (docs/AUDIO_RESEARCH.md §9): once per vehicle tick
//! (as the engine and tyre parameters, F453) every car's solved contacts go
//! through the original's collision listener (`grid_tools::collision_audio`)
//! and the one-shots play on the player's Panel Coll group or the AI group,
//! with the A23B70 distance law; each car's scrape loop (453710/458780)
//! follows on the same groups. Presentation only, never feeds physics.
use bevy::{
    audio::{AddAudioSource, ChannelCount, SampleRate, Source},
    prelude::*,
};
use grid_tools::{
    audio_mix::{self, Table},
    collision_audio::{Bank, Car, Contact, Listener, Mixer, classify},
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

/// For the audio thread: a one-shot (variety, pitch, linear gain) or car
/// k's scrape loop level.
enum Start {
    Shot(String, f32, f32),
    Scrape(usize, String, f32, f32),
}

/// A car model's description heights (lowest and highest `collision_main` y).
#[derive(Clone, Copy)]
pub(crate) struct Heights {
    pub bottom: f32,
    pub roof: f32,
}

#[derive(Resource)]
pub(crate) struct CollisionSound {
    bank: Arc<Bank>,
    table: Option<Arc<Table>>,
    listener: Listener,
    /// Heights per field model; `car_model[k]` selects car k's.
    heights: Vec<Heights>,
    car_model: Vec<usize>,
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
}
impl Iterator for Decoder {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.count.is_multiple_of(64)
            && let Some(starts) = &self.starts
        {
            while let Ok(start) = starts.try_recv() {
                match start {
                    Start::Shot(variety, pitch, gain) => {
                        self.mixer.start(&variety, pitch, gain);
                    }
                    Start::Scrape(car, variety, pitch, gain) => self.mixer.scrape(car, &variety, pitch, gain),
                }
            }
        }
        self.count = self.count.wrapping_add(1);
        let value = self.mixer.sample();
        Some(if value.is_finite() { value } else { 0. })
    }
}
impl Source for Decoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(1).unwrap()
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
        }
    }
}

/// The description heights of a car model (`collision_main` points, the
/// values 0x9E2030 stores at description +0x1B28/+0x1B54).
pub(crate) fn heights(root: &std::path::Path, model: &grid_tools::car::Car) -> Result<Heights, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(root.join(model.file(".ned")))?;
    let doc = grid_formats::binxml::read(&bytes)?;
    let mesh = grid_formats::car_collision::read(&doc)?;
    Ok(Heights { bottom: mesh.bounds.min[1], roof: mesh.bounds.max[1] })
}

pub(crate) fn install(
    app: &mut App,
    bank: Arc<Bank>,
    table: Option<Arc<Table>>,
    heights: Vec<Heights>,
    car_model: Vec<usize>,
) {
    let (starts, receiver) = channel();
    let source = CollisionSource { bank: bank.clone(), starts: Mutex::new(Some(receiver)) };
    app.add_audio_source::<CollisionSource>()
        .insert_resource(CollisionSound {
            listener: Listener::new(bank.clone()),
            bank,
            table,
            heights,
            car_model,
            starts,
            last: None,
        })
        .insert_resource(PendingSource(Some(source)))
        .add_systems(Startup, start)
        .add_systems(FixedUpdate, tick.after(super::drive_tick));
}

#[derive(Resource)]
struct PendingSource(Option<CollisionSource>);

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

/// The AudioMaterial name of what a contact touched: the packed track
/// triangle under it, CAR_ for another car; the boundary sheet and route
/// props are taken as WALL [I: their per-triangle/template materials are
/// not in the contact; every such AudioMaterial gives CarScrapeSideRoad].
fn surface(runtime: &Runtime, contacted: Contacted, point: [f32; 3], normal: [f32; 3]) -> Option<String> {
    match contacted {
        Contacted::Track => runtime.track_surface_at(point, normal).map(str::to_owned),
        Contacted::Car(_) => Some("CAR_".into()),
        Contacted::Prop(_) | Contacted::Static => Some("WALL".into()),
    }
}

fn car_of(runtime: &Runtime, heights: Heights, player: bool, impacts: &[grid_tools::body_impacts::Impact]) -> Car {
    let v = &runtime.state.car.owner.vehicle;
    let f = |o: usize| f32::from_bits(v[o / 4]);
    let velocity = runtime.velocity();
    // A69930: the body's impulse accumulator (+0x240, B0FC00 adds each solved
    // contact's +3C times its normal) when a contact flagged it (+0x40C)
    // [I: summed here from the solved records of the car].
    let impulse = (!impacts.is_empty()).then(|| {
        impacts.iter().fold([0.; 3], |a, x| std::array::from_fn(|j| a[j] + x.impact * x.normal[j]))
    });
    Car {
        pose: runtime.pose(),
        offset: [f(0x1630), f(0x1634), f(0x1638)],
        mass: f32::from_bits(runtime.state.car.owner.physical[0x3f0 / 4]),
        bottom: heights.bottom,
        roof: heights.roof,
        player,
        velocity: [velocity[0], velocity[1], velocity[2]],
        impulse,
    }
}

/// Once per successful vehicle tick: every car's contacts.
fn tick(mut sound: ResMut<CollisionSound>, driving: Res<super::Driving>, listening: super::engine_sound::Listening) {
    let now = (driving.segment, driving.runtime.ticks);
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
    let (trim, camera) = (listening.trim(), listening.camera());
    let ear = listening.ear(&driving.runtime.pose());
    let group_db = |sound: &CollisionSound, group: usize| {
        sound.table.as_ref().map_or(-9., |t| t.group_db(audio_mix::NORMAL, camera, group, trim))
    };
    for (k, (runtime, impacts)) in cars.into_iter().enumerate() {
        let model = sound.car_model.get(k).copied().unwrap_or(0);
        let Some(&heights) = sound.heights.get(model) else { continue };
        let solved: Vec<_> = impacts.iter().map(|(x, _)| *x).collect();
        let car = car_of(runtime, heights, k == 0, &solved);
        let position = Vec3::new(car.pose[12], car.pose[13], car.pose[14]);
        // The player's groups use the Car listener (the car itself), the
        // AI group the camera (433AA0) [I: car pose as the Car listener].
        let listener = if k == 0 { position } else { ear };
        let mut plays = Vec::new();
        if !impacts.is_empty() {
            let contacts: Vec<Contact> = impacts
                .iter()
                .map(|(x, contacted)| Contact {
                    point: x.point,
                    impact: x.impact,
                    // Floor contacts make no sound: skip their surface query.
                    surface: classify(&car, x.point)
                        .and_then(|_| surface(runtime, *contacted, x.point, x.normal)),
                })
                .collect();
            sound.listener.contacts(k, &car, &contacts, &mut plays);
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
            let _ = sound.starts.send(Start::Scrape(k, voice.variety, voice.pitch, gain));
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
                let _ = sound.starts.send(Start::Shot(play.variety, play.pitch, gain));
            }
        }
    }
}
