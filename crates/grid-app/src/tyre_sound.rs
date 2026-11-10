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
    audio_pacing::Paced,
    engine_audio_continuous::BLOCK,
    source_vehicle::{DELTA, Runtime},
    tyre_audio::{self, Bank, Driver, Inputs, Mixer, VOICES, Voice},
};
use super::stereo::{Pan, Stereo};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

const RATE: u32 = grid_tools::engine_audio::RATE;
type Voices = [[Voice; VOICES]; 4];
type Levels = [[f32; VOICES]; 4];

/// What one car's audio source reads: voice targets, one set per vehicle
/// tick (`audio_pacing`), and backend levels.
#[derive(Clone)]
struct Frame {
    voices: Paced<Voices>,
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
    /// Each car's stereo pan (`stereo`).
    pans: Vec<Pan>,
    audible: Arc<AtomicBool>,
    last: Option<(u64, u64)>,
    /// Each car's driver while a replay is shown, run once per crossed replay
    /// tick on its replayed words (`replay::ReplayedWords`).
    replay_drivers: Vec<Driver>,
}
impl TyreSound {
    fn new(bank: Arc<Bank>, table: Option<Arc<Table>>) -> Self {
        Self {
            bank,
            table,
            drivers: Vec::new(),
            shared: Vec::new(),
            pans: Vec::new(),
            audible: Arc::new(AtomicBool::new(false)),
            last: None,
            replay_drivers: Vec::new(),
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
    pan: Pan,
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
            if let Ok(mut frame) = self.shared.try_lock() {
                // One tick's voice targets per 16 ms block, so a slow
                // frame's ticks are not collapsed into one step.
                if self.count.is_multiple_of(BLOCK) {
                    self.voices = frame.voices.take();
                }
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
    type Decoder = Stereo<Decoder>;
    fn decoder(&self) -> Stereo<Decoder> {
        let frame = self.shared.lock().unwrap().clone();
        let mut mixer = Mixer::new(self.bank.clone(), RATE);
        mixer.bus_db = frame.levels;
        let decoder = Decoder {
            mixer,
            shared: self.shared.clone(),
            audible: self.audible.clone(),
            voices: frame.voices.latest(),
            playing: false,
            count: 0,
        };
        Stereo::new(decoder, self.pan.clone(), RATE)
    }
}

pub(crate) fn install(app: &mut App, bank: Arc<Bank>, table: Option<Arc<Table>>) {
    app.add_audio_source::<TyreSource>();
    install_systems(app, bank, table);
}
/// `install` without the device playback (the race-start mix test).
pub(crate) fn install_systems(app: &mut App, bank: Arc<Bank>, table: Option<Arc<Table>>) {
    app.insert_resource(TyreSound::new(bank, table))
        .add_systems(Startup, start)
        .add_systems(FixedUpdate, tick.after(super::drive_tick))
        .add_systems(Update, (replayed.after(super::replay::control), audibility).chain());
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
            voices: Paced::new(driver.voices()),
            levels: [[f32::NEG_INFINITY; VOICES]; 4],
        }));
        let pan = Pan::centre();
        let handle = assets.add(TyreSource {
            bank: sound.bank.clone(),
            shared: shared.clone(),
            audible: sound.audible.clone(),
            pan: pan.clone(),
        });
        sound.pans.push(pan);
        // ONCE: the decoder never ends; LOOP's rodio `repeat_infinite` would
        // keep every sample played to replay it (F482).
        commands.spawn((AudioPlayer(handle), PlaybackSettings::ONCE));
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
fn tick(mut sound: ResMut<TyreSound>, driving: Res<super::Driving>, replay: Option<Res<super::replay::Replay>>) {
    let now = (driving.segment, driving.runtime.ticks);
    let sound = &mut *sound;
    // A replay's voices come from its frames (`replayed`); the live drivers
    // start again after it.
    if replay.is_some_and(|r| r.session.is_some()) {
        sound.last = None;
        return;
    }
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
            frame.voices.push(driver.voices());
        }
    }
}

/// While a replay is shown: each car's driver runs once per replay tick
/// crossed this frame on the replayed words. The original records no tyre
/// sound; the port regenerates it from the replayed cars [I] (docs/REPLAY.md
/// 3.2).
fn replayed(mut sound: ResMut<TyreSound>, driving: Res<super::Driving>, listening: super::engine_sound::Listening) {
    let sound = &mut *sound;
    let Some(session) = listening.replay_session().filter(|s| !s.words.is_empty()) else {
        sound.replay_drivers.clear();
        return;
    };
    if sound.replay_drivers.is_empty() {
        let drivers: Vec<Driver> = (0..sound.shared.len()).map(|car| sound.driver(car)).collect();
        sound.replay_drivers = drivers;
    }
    for _ in 0..session.advanced {
        for (car, ((driver, shared), words)) in
            sound.replay_drivers.iter_mut().zip(&sound.shared).zip(&session.words).enumerate()
        {
            let Some(runtime) = runtime(&driving, car) else {
                continue;
            };
            driver.tick(&Inputs {
                vehicle: &words.vehicle,
                physical: &words.physical,
                surfaces: runtime.surfaces(),
                delta: DELTA,
            });
            if let Ok(mut frame) = shared.lock() {
                frame.voices.push(driver.voices());
            }
        }
    }
}

/// Car `car`'s pose, vehicle and physical words: the replayed ones while a
/// replay is shown, else the live runtime's.
fn car_words<'a>(
    replay: Option<&'a super::replay::Session>,
    car: usize,
    runtime: &'a Runtime,
) -> ([f32; 16], &'a [u32], &'a [u32]) {
    match replay.and_then(|s| s.words.get(car)) {
        Some(w) => (pose_of(&w.physical), &w.vehicle, &w.physical),
        None => (runtime.pose(), &runtime.state.car.owner.vehicle, &runtime.state.car.owner.physical),
    }
}

/// The pose words (physical `+1D0`).
fn pose_of(physical: &[u32]) -> [f32; 16] {
    std::array::from_fn(|i| physical.get(0x1d0 / 4 + i).map_or(0., |w| f32::from_bits(*w)))
}

fn audibility(
    sound: Res<TyreSound>,
    driving: Res<super::Driving>,
    engine: Option<Res<super::engine_sound::EngineSound>>,
    listening: super::engine_sound::Listening,
) {
    let (trim, camera, modes) = (listening.trim(), listening.camera(), listening.modes());
    // A replay is heard from its replayed cars, at the replay level.
    let replay = listening.replay_session().filter(|s| !s.words.is_empty());
    let trim = match replay {
        Some(_) => trim + engine.as_ref().map_or(0., |e| e.replay_db()),
        None => trim,
    };
    let words = |car: usize, runtime| car_words(replay, car, runtime);
    let ear = listening.ear(&words(0, &driving.runtime).0);
    for (car, shared) in sound.shared.iter().enumerate() {
        let Some(runtime) = runtime(&driving, car) else {
            continue;
        };
        let (pose, vehicle, physical) = words(car, runtime);
        let distances = tyre_audio::wheel_positions(vehicle, physical).map(|p| Vec3::from_array(p).distance(ear));
        let levels =
            tyre_audio::bus_levels(sound.table.as_deref(), &modes, camera, car == 0, trim, distances);
        // Opponents' tyres (AI group) are heard from the camera; the
        // player's four wheels are mixed into one source heard by the Car
        // listener at the car, so centred [I: no per-wheel panning].
        if car > 0
            && let Some(pan) = sound.pans.get(car)
        {
            pan.set(listening.heard(ear, Vec3::new(pose[12], pose[13], pose[14])));
        }
        if let Ok(mut frame) = shared.lock() {
            frame.levels = levels;
        }
    }
    let audible = !engine.is_some_and(|e| e.muted)
        && driving.failure.is_none()
        && (replay.is_some() || !driving.paused && !listening.stopped(&driving));
    sound.audible.store(audible, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::audio::PlaybackMode;

    /// The voices never end; a looped player would keep every sample played
    /// so far (F482).
    #[test]
    #[ignore = "requires GRID_GAME_ROOT purchased sources; no audio device used"]
    fn endless_tyre_voices_play_once() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let bank = Bank::load(std::path::Path::new(&root)).unwrap();
        crate::mobile::init_test_pool();
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<TyreSource>()
            .insert_resource(TyreSound::new(bank, None))
            .add_systems(Startup, start);
        app.update();
        let world = app.world_mut();
        let modes: Vec<PlaybackMode> = world
            .query::<&PlaybackSettings>()
            .iter(world)
            .map(|s| s.mode)
            .collect();
        assert_eq!(modes.len(), 1);
        assert!(modes.iter().all(|m| matches!(m, PlaybackMode::Once)));
    }
}
