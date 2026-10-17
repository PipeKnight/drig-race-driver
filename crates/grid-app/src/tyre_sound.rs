//! Player tyre/surface audio: the original-style driver (TYRE_AUDIO_RESEARCH.md)
//! runs once per successful physics tick; the audio thread mixes looping
//! car_surfaces voices. Presentation only, never feeds physics.
use bevy::{
    audio::{AddAudioSource, ChannelCount, SampleRate, Source},
    prelude::*,
};
use grid_tools::{
    source_vehicle::DELTA,
    tyre_audio::{Bank, CATEGORIES, Driver, Inputs, Mixer, Voice},
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

#[derive(Resource)]
pub(crate) struct TyreSound {
    bank: Arc<Bank>,
    driver: Driver,
    shared: Arc<Mutex<Voices>>,
    audible: Arc<AtomicBool>,
    last: Option<(u64, u64)>,
}
impl TyreSound {
    fn new(bank: Arc<Bank>) -> Self {
        let driver = Driver::new(bank.clone());
        let shared = Arc::new(Mutex::new(driver.voices()));
        Self {
            bank,
            driver,
            shared,
            audible: Arc::new(AtomicBool::new(false)),
            last: None,
        }
    }
}

#[derive(Asset, TypePath)]
pub(crate) struct TyreSource {
    bank: Arc<Bank>,
    shared: Arc<Mutex<Voices>>,
    audible: Arc<AtomicBool>,
}
pub(crate) struct Decoder {
    mixer: Mixer,
    shared: Arc<Mutex<Voices>>,
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
            if let Ok(voices) = self.shared.try_lock() {
                self.voices = *voices;
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
        Decoder {
            mixer: Mixer::new(self.bank.clone(), RATE),
            shared: self.shared.clone(),
            audible: self.audible.clone(),
            voices: *self.shared.lock().unwrap(),
            playing: false,
            count: 0,
        }
    }
}

pub(crate) fn install(app: &mut App, bank: Arc<Bank>) {
    app.add_audio_source::<TyreSource>()
        .insert_resource(TyreSound::new(bank))
        .add_systems(Startup, start)
        .add_systems(FixedUpdate, tick.after(super::drive_tick))
        .add_systems(Update, audibility);
}
fn start(mut commands: Commands, sound: Res<TyreSound>, mut assets: ResMut<Assets<TyreSource>>) {
    let handle = assets.add(TyreSource {
        bank: sound.bank.clone(),
        shared: sound.shared.clone(),
        audible: sound.audible.clone(),
    });
    commands.spawn(AudioPlayer(handle));
    println!(
        "tyre_audio version=1 event=queued rate={RATE} wheels=4 categories=roll,peel,slide,skid,scrub bus_gain_db=-12 fidelity=experimental"
    );
}

/// Advance the driver exactly once per successful vehicle tick; reset with the car.
fn tick(mut sound: ResMut<TyreSound>, driving: Res<super::Driving>) {
    let now = (driving.segment, driving.runtime.ticks);
    let sound = &mut *sound;
    match sound.last {
        Some(last) if last == now => return,
        Some((segment, ticks)) if segment == now.0 && ticks + 1 == now.1 => {}
        _ => sound.driver = Driver::new(sound.bank.clone()),
    }
    sound.last = Some(now);
    if now.1 == 0 {
        return;
    }
    sound.driver.tick(&Inputs {
        vehicle: &driving.runtime.state.car.owner.vehicle,
        physical: &driving.runtime.state.car.owner.physical,
        surfaces: driving.runtime.surfaces(),
        delta: DELTA,
    });
    if let Ok(mut shared) = sound.shared.lock() {
        *shared = sound.driver.voices();
    }
}

fn audibility(
    sound: Res<TyreSound>,
    driving: Res<super::Driving>,
    engine: Option<Res<super::engine_sound::EngineSound>>,
    trial: Option<Res<super::time_trial::Trial>>,
    menu: Option<Res<super::pause_menu::PauseMenu>>,
    mut sinks: Query<&mut AudioSink, With<AudioPlayer<TyreSource>>>,
) {
    // Pause menu Effects Volume.
    let gain = bevy::audio::Volume::Linear(menu.map_or(1., |m| m.values.gain()));
    for mut sink in &mut sinks {
        if sink.volume() != gain {
            sink.set_volume(gain);
        }
    }
    let audible = !engine.is_some_and(|e| e.muted)
        && !driving.paused
        && driving.failure.is_none()
        && !trial.is_some_and(|t| t.finished());
    sound.audible.store(audible, Ordering::Relaxed);
}
