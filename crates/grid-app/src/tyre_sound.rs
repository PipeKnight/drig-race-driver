//! Tyre/surface audio: the original-style driver (TYRE_AUDIO_RESEARCH.md)
//! runs once per successful physics tick for the player car (four wheels) and
//! every opponent (wheel object 0, peel/slide/skid; 457BE0); the audio thread
//! mixes looping car_surfaces voices at the original group and distance levels
//! (docs/AUDIO_RESEARCH.md). Presentation only, never feeds physics.
use bevy::{
    audio::{AddAudioSource, ChannelCount, SampleRate, Source},
    prelude::*,
};
use grid_tools::{
    audio_mix::Table,
    source_vehicle::{DELTA, Runtime},
    tyre_audio::{self, Bank, CATEGORIES, Driver, Inputs, Mixer, Voice},
};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

const RATE: u32 = grid_tools::engine_audio::RATE;
type Voices = [[Voice; CATEGORIES]; 4];
type Levels = [[f32; CATEGORIES]; 4];

/// What one car's audio source reads: voice targets and backend levels.
#[derive(Clone, Copy)]
struct Frame {
    voices: Voices,
    levels: Levels,
}
type Shared = Arc<Mutex<Frame>>;

#[derive(Resource)]
pub(crate) struct TyreSound {
    bank: Arc<Bank>,
    table: Option<Arc<Table>>,
    /// Car 0 is the player; car k is opponent k of the shared world.
    drivers: Vec<Driver>,
    shared: Vec<Shared>,
    audible: Arc<AtomicBool>,
    last: Option<(u64, u64)>,
}
impl TyreSound {
    fn new(bank: Arc<Bank>, table: Option<Arc<Table>>) -> Self {
        Self {
            bank,
            table,
            drivers: Vec::new(),
            shared: Vec::new(),
            audible: Arc::new(AtomicBool::new(false)),
            last: None,
        }
    }
    fn driver(&self, car: usize) -> Driver {
        if car == 0 {
            Driver::new(self.bank.clone())
        } else {
            Driver::opponent(self.bank.clone())
        }
    }
}

#[derive(Asset, TypePath)]
pub(crate) struct TyreSource {
    bank: Arc<Bank>,
    shared: Shared,
    audible: Arc<AtomicBool>,
}
pub(crate) struct Decoder {
    mixer: Mixer,
    shared: Shared,
    audible: Arc<AtomicBool>,
    voices: Voices,
    playing: bool,
    count: usize,
}
impl Iterator for Decoder {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.count.is_multiple_of(64) {
            // Never block the audio thread; keep the previous targets if busy.
            if let Ok(frame) = self.shared.try_lock() {
                self.voices = frame.voices;
                self.mixer.bus_db = frame.levels;
            }
            self.playing = self.audible.load(Ordering::Relaxed);
        }
        self.count = self.count.wrapping_add(1);
        let value = self.mixer.sample(&self.voices, self.playing);
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
impl Decodable for TyreSource {
    type Decoder = Decoder;
    fn decoder(&self) -> Decoder {
        let frame = *self.shared.lock().unwrap();
        let mut mixer = Mixer::new(self.bank.clone(), RATE);
        mixer.bus_db = frame.levels;
        Decoder {
            mixer,
            shared: self.shared.clone(),
            audible: self.audible.clone(),
            voices: frame.voices,
            playing: false,
            count: 0,
        }
    }
}

pub(crate) fn install(app: &mut App, bank: Arc<Bank>, table: Option<Arc<Table>>) {
    app.add_audio_source::<TyreSource>()
        .insert_resource(TyreSound::new(bank, table))
        .add_systems(Startup, start)
        .add_systems(FixedUpdate, tick.after(super::drive_tick))
        .add_systems(Update, audibility);
}
fn start(
    mut commands: Commands,
    mut sound: ResMut<TyreSound>,
    mut assets: ResMut<Assets<TyreSource>>,
    driving: Option<Res<super::Driving>>,
) {
    let cars = driving
        .as_ref()
        .and_then(|d| d.opponents.as_ref())
        .map_or(1, |o| o.multi.cars.len().max(1));
    for car in 0..cars {
        let driver = sound.driver(car);
        let shared = Arc::new(Mutex::new(Frame {
            voices: driver.voices(),
            levels: [[f32::NEG_INFINITY; CATEGORIES]; 4],
        }));
        let handle = assets.add(TyreSource {
            bank: sound.bank.clone(),
            shared: shared.clone(),
            audible: sound.audible.clone(),
        });
        commands.spawn((AudioPlayer(handle), PlaybackSettings::LOOP));
        sound.drivers.push(driver);
        sound.shared.push(shared);
    }
    println!(
        "tyre_audio version=2 event=queued rate={RATE} cars={cars} player=4_wheels,roll,peel,slide,skid,scrub opponents=wheel0,peel,slide,skid groups=Wheels,Skids,AI distance=presets fidelity=experimental"
    );
}

/// Car `k` of the shared world; car 0 is the player runtime.
fn runtime(driving: &super::Driving, car: usize) -> Option<&Runtime> {
    if car == 0 {
        Some(&driving.runtime)
    } else {
        driving.opponents.as_ref()?.multi.cars.get(car)
    }
}

/// Advance the drivers exactly once per successful vehicle tick; reset with the car.
fn tick(mut sound: ResMut<TyreSound>, driving: Res<super::Driving>) {
    let now = (driving.segment, driving.runtime.ticks);
    let sound = &mut *sound;
    match sound.last {
        Some(last) if last == now => return,
        Some((segment, ticks)) if segment == now.0 && ticks + 1 == now.1 => {}
        _ => {
            for car in 0..sound.drivers.len() {
                sound.drivers[car] = sound.driver(car);
            }
        }
    }
    sound.last = Some(now);
    if now.1 == 0 {
        return;
    }
    for (car, (driver, shared)) in sound.drivers.iter_mut().zip(&sound.shared).enumerate() {
        let Some(runtime) = runtime(&driving, car) else {
            continue;
        };
        driver.tick(&Inputs {
            vehicle: &runtime.state.car.owner.vehicle,
            physical: &runtime.state.car.owner.physical,
            surfaces: runtime.surfaces(),
            delta: DELTA,
        });
        if let Ok(mut frame) = shared.lock() {
            frame.voices = driver.voices();
        }
    }
}

fn audibility(
    sound: Res<TyreSound>,
    driving: Res<super::Driving>,
    engine: Option<Res<super::engine_sound::EngineSound>>,
    listening: super::engine_sound::Listening,
) {
    let (trim, camera) = (listening.trim(), listening.camera());
    let ear = listening.ear(&driving.runtime.pose());
    for (car, shared) in sound.shared.iter().enumerate() {
        let Some(runtime) = runtime(&driving, car) else {
            continue;
        };
        let distances = tyre_audio::wheel_positions(
            &runtime.state.car.owner.vehicle,
            &runtime.state.car.owner.physical,
        )
        .map(|p| Vec3::from_array(p).distance(ear));
        let levels =
            tyre_audio::bus_levels(sound.table.as_deref(), camera, car == 0, trim, distances);
        if let Ok(mut frame) = shared.lock() {
            frame.levels = levels;
        }
    }
    let audible = !engine.is_some_and(|e| e.muted)
        && !driving.paused
        && driving.failure.is_none()
        && !listening.trial.as_ref().is_some_and(|t| t.finished());
    sound.audible.store(audible, Ordering::Relaxed);
}
