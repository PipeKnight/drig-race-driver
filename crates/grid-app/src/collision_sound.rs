//! Car collision sounds (docs/AUDIO_RESEARCH.md §9): once per vehicle tick
//! (as the engine and tyre parameters, F453) every car's solved contacts go
//! through the original's collision listener (`grid_tools::collision_audio`)
//! and the one-shots play on the player's Panel Coll group or the AI group,
//! with the A23B70 distance law. Presentation only, never feeds physics.
use bevy::{
    audio::{AddAudioSource, ChannelCount, SampleRate, Source},
    prelude::*,
};
use grid_tools::{
    audio_mix::{self, Table},
    collision_audio::{Bank, Car, Contact, Listener, Mixer},
    source_vehicle::{DELTA, Runtime},
};
use std::{
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender, channel},
    },
    time::Duration,
};

const RATE: u32 = grid_tools::engine_audio::RATE;

/// One one-shot for the audio thread: variety, pitch, linear gain.
type Start = (String, f32, f32);

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
            while let Ok((variety, pitch, gain)) = starts.try_recv() {
                self.mixer.start(&variety, pitch, gain);
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
        println!("collision_audio version=1 event=queued rate={RATE} groups=PanelColl,AI impacts=front_back,sides,roof sweeteners=low,high");
    }
}

fn car_of(runtime: &Runtime, heights: Heights, player: bool) -> Car {
    let v = &runtime.state.car.owner.vehicle;
    let f = |o: usize| f32::from_bits(v[o / 4]);
    Car {
        pose: runtime.pose(),
        offset: [f(0x1630), f(0x1634), f(0x1638)],
        mass: f32::from_bits(runtime.state.car.owner.physical[0x3f0 / 4]),
        bottom: heights.bottom,
        roof: heights.roof,
        player,
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
    let mut cars: Vec<(&Runtime, Vec<Contact>)> = Vec::new();
    match &driving.opponents {
        Some(o) => {
            for (k, runtime) in std::iter::once(&driving.runtime).chain(o.multi.cars.iter().skip(1)).enumerate() {
                let contacts = o
                    .multi
                    .contact_impacts(k)
                    .iter()
                    .map(|(x, _)| Contact { point: x.point, impact: x.impact })
                    .collect();
                cars.push((runtime, contacts));
            }
        }
        None => {
            let contacts = driving
                .runtime
                .impacts()
                .iter()
                .map(|x| Contact { point: x.point, impact: x.impact })
                .collect();
            cars.push((&driving.runtime, contacts));
        }
    }
    let (trim, camera) = (listening.trim(), listening.camera());
    let ear = listening.ear(&driving.runtime.pose());
    for (k, (runtime, contacts)) in cars.into_iter().enumerate() {
        if contacts.is_empty() {
            continue;
        }
        let model = sound.car_model.get(k).copied().unwrap_or(0);
        let Some(&heights) = sound.heights.get(model) else { continue };
        let car = car_of(runtime, heights, k == 0);
        let mut plays = Vec::new();
        sound.listener.contacts(&car, &contacts, &mut plays);
        for play in plays {
            let Some(variety) = sound.bank.varieties.get(&play.variety) else { continue };
            // The player's groups use the Car listener (the car itself), the
            // AI group the camera (433AA0) [I: car pose as the Car listener].
            let listener = if k == 0 {
                Vec3::new(car.pose[12], car.pose[13], car.pose[14])
            } else {
                ear
            };
            let distance = Vec3::from_array(play.position).distance(listener);
            let group_db = sound
                .table
                .as_ref()
                .map_or(-9., |t| t.group_db(audio_mix::NORMAL, camera, play.group, trim));
            let gain = audio_mix::backend_gain(
                play.volume_db,
                group_db,
                audio_mix::distance_db(distance, variety.min_distance, variety.max_distance),
            );
            if gain > 0. {
                let _ = sound.starts.send((play.variety, play.pitch, gain));
            }
        }
    }
}
