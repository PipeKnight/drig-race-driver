//! Audio thread receives coherent, read-only presentation telemetry.
use bevy::{
    audio::{AddAudioSource, ChannelCount, SampleRate, Source},
    prelude::*,
};
use grid_tools::{
    engine_audio::{Control, RATE},
    engine_audio_continuous::{Bank as EngineBank, Mixer},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicU32, AtomicU64, Ordering},
    },
    time::Duration,
};

#[derive(Resource)]
pub(crate) struct EngineSound {
    mailbox: Arc<AtomicU64>,
    failure: Arc<AtomicU32>,
    bank: Arc<EngineBank>,
    pub(crate) muted: bool,
}
impl EngineSound {
    pub(crate) fn new(bank: Arc<EngineBank>) -> Self {
        Self {
            mailbox: Arc::new(AtomicU64::new(0)),
            failure: Arc::new(AtomicU32::new(0)),
            bank,
            muted: false,
        }
    }
    pub(crate) fn control(&self, driving: &super::Driving) -> Control {
        Control::vehicle(
            &driving.runtime.state.car.owner.vehicle,
            if driving.runtime.terminal_wheel().is_some() {
                0.
            } else {
                driving.last_controls.unwrap_or_default().throttle
            },
            !self.muted && !driving.paused && driving.failure.is_none(),
            driving.segment as u32,
        )
    }
}
#[derive(Asset, TypePath)]
pub(crate) struct EngineSource {
    bank: Arc<EngineBank>,
    mailbox: Arc<AtomicU64>,
    failure: Arc<AtomicU32>,
}
pub(crate) struct Decoder {
    mixer: Mixer,
    mailbox: Arc<AtomicU64>,
    control: Control,
    count: usize,
    failure: Arc<AtomicU32>,
    reported_failure: bool,
}
impl Iterator for Decoder {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.count.is_multiple_of(64) {
            let generation = self.control.generation;
            self.control = Control::unpack(self.mailbox.load(Ordering::Relaxed));
            if generation != self.control.generation {
                self.reported_failure = false;
            }
        }
        self.count = self.count.wrapping_add(1);
        let value = self.mixer.sample(self.control);
        if self.mixer.failure.is_some() && !self.reported_failure {
            self.failure.store(1, Ordering::Relaxed);
            self.reported_failure = true;
        }
        Some(value)
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
impl Decodable for EngineSource {
    type Decoder = Decoder;
    fn decoder(&self) -> Decoder {
        Decoder {
            mixer: Mixer::new(self.bank.clone()),
            mailbox: self.mailbox.clone(),
            control: Control::default(),
            count: 0,
            failure: self.failure.clone(),
            reported_failure: false,
        }
    }
}
pub(crate) fn install(app: &mut App, bank: Arc<EngineBank>) {
    app.add_audio_source::<EngineSource>()
        .insert_resource(EngineSound::new(bank))
        .add_systems(Startup, start)
        .add_systems(Update, update.after(super::drive_present));
}
fn start(
    mut commands: Commands,
    sound: Res<EngineSound>,
    mut assets: ResMut<Assets<EngineSource>>,
) {
    let handle = assets.add(EngineSource {
        bank: sound.bank.clone(),
        mailbox: sound.mailbox.clone(),
        failure: sound.failure.clone(),
    });
    commands.spawn(AudioPlayer(handle));
    println!(
        "engine_audio version=2 event=queued rate={RATE} layers=dn_en,up_en,up_en_load,dn_ex,up_ex,up_ex_load voices=12 mixer=continuous_pair_scheduler fidelity=experimental mute_key=M"
    );
}
fn update(
    mut sound: ResMut<EngineSound>,
    driving: Res<super::Driving>,
    input: Res<ButtonInput<KeyCode>>,
    sinks: Query<&AudioSink, With<AudioPlayer<EngineSource>>>,
    mut started: Local<bool>,
    trial: Option<Res<super::time_trial::Trial>>,
) {
    if sound.failure.swap(0, Ordering::Relaxed) != 0 && !sound.muted {
        sound.muted = true;
        eprintln!("engine_audio version=2 event=decoder_failed muted=true reset_key=R");
    }
    if input.just_pressed(KeyCode::KeyM) {
        sound.muted = !sound.muted;
        println!("engine_audio version=2 event=mute muted={}", sound.muted);
    }
    if !*started && !sinks.is_empty() {
        *started = true;
        println!("engine_audio version=2 event=output_started");
    }
    let mut control = sound.control(&driving);
    // The engine idles and may be revved during the countdown; the vehicle no
    // longer ticks after the finish.
    if trial.is_some_and(|t| t.finished()) {
        control.audible = false;
    }
    sound.mailbox.store(control.pack(), Ordering::Relaxed);
}

/// Bounded real-device diagnostic, separate from silent offscreen captures.
pub(crate) fn check(root: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let bank = EngineBank::lacetti(root)?;
    let sound = EngineSound::new(bank);
    let mailbox = sound.mailbox.clone();
    mailbox.store(
        Control {
            normalized_rpm: 0.5,
            throttle: 0.3,
            audible: true,
            generation: 0,
        }
        .pack(),
        Ordering::Relaxed,
    );
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        AssetPlugin::default(),
        bevy::audio::AudioPlugin::default(),
    ))
    .add_audio_source::<EngineSource>()
    .insert_resource(sound)
    .add_systems(Startup, start);
    app.finish();
    app.cleanup();
    let began = std::time::Instant::now();
    let mut played = None;
    while began.elapsed() < Duration::from_secs(4) {
        app.update();
        let world = app.world_mut();
        let mut query = world.query::<&AudioSink>();
        if query.iter(world).any(|s| !s.is_paused() && !s.empty()) {
            played.get_or_insert_with(std::time::Instant::now);
        }
        if played.is_some_and(|p| p.elapsed() > Duration::from_secs(1)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(16));
    }
    mailbox.store(0, Ordering::Relaxed);
    std::thread::sleep(Duration::from_millis(30));
    if played.is_none() {
        return Err("engine audio output did not start within four seconds".into());
    }
    println!(
        "engine_audio version=2 event=device_check_passed playing_seconds=1 decoder_rate={RATE}"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires GRID_GAME_ROOT purchased sources; no audio device used"]
    fn source_engine_sound_pause_mute_reset_and_failure() {
        std::thread::Builder::new().stack_size(16*1024*1024).spawn(|| {
            let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
            let root = std::path::Path::new(&root);
            let bank = EngineBank::lacetti(root).unwrap();
            let mut decoder = EngineSource { bank: bank.clone(), mailbox: Arc::new(AtomicU64::new(0)), failure: Arc::new(AtomicU32::new(0)) }.decoder();
            let sound = EngineSound::new(bank);
            decoder.mailbox = sound.mailbox.clone();
            let mut app = App::new();
            app.insert_resource(sound).init_resource::<ButtonInput<KeyCode>>()
                .add_message::<AppExit>()
                .insert_resource(crate::Driving { runtime: grid_tools::source_vehicle::Runtime::lacetti(root,"slot_00",200679).unwrap(), geometry: None, paused:false, failure:None, scripted_ticks:None, scripted:Default::default(), last_controls:None, segment:0, started:std::time::Instant::now() })
                .add_systems(Update,(crate::drive_actions,crate::drive_tick,update).chain());
            let control = |app: &App| Control::unpack(app.world().resource::<EngineSound>().mailbox.load(Ordering::Relaxed));
            let press = |app: &mut App,key| {let mut input=app.world_mut().resource_mut::<ButtonInput<KeyCode>>();input.reset_all();input.press(key);};
            press(&mut app,KeyCode::ArrowUp);app.update();
            assert!(control(&app).audible);assert_eq!(control(&app).throttle,1.);
            assert!(decoder.by_ref().take(2400).any(|s|s.abs()>0.01));
            press(&mut app,KeyCode::KeyP);app.update();
            assert!(!control(&app).audible);
            assert_eq!(app.world().resource::<crate::Driving>().runtime.ticks,1);
            for _ in 0..600 {decoder.next();}
            assert!(decoder.by_ref().take(1000).all(|s|s==0.));
            press(&mut app,KeyCode::KeyM);app.update();
            assert!(app.world().resource::<EngineSound>().muted);
            press(&mut app,KeyCode::KeyR);app.update();
            assert_eq!(control(&app).generation,1);assert!(!control(&app).audible);
            assert!(!app.world().resource::<crate::Driving>().paused);
            press(&mut app,KeyCode::KeyM);app.update();
            // Reset restores the running idle engine (native stationary reference).
            assert!(control(&app).audible);
            press(&mut app,KeyCode::ArrowUp);app.update();assert!(control(&app).audible);
            app.world_mut().resource_mut::<crate::Driving>().failure=Some("test failure".into());
            let before=app.world().resource::<crate::Driving>().runtime.state.car.owner.vehicle.clone();
            app.world_mut().resource_mut::<ButtonInput<KeyCode>>().reset_all();app.update();
            assert!(!control(&app).audible);
            assert_eq!(app.world().resource::<crate::Driving>().runtime.state.car.owner.vehicle,before);
            press(&mut app,KeyCode::KeyR);app.update();
            assert!(control(&app).audible);assert_eq!(control(&app).generation,2);
            press(&mut app,KeyCode::ArrowUp);app.update();assert!(control(&app).audible);
            eprintln!("source_engine_audio mailbox_pause_mute_reset_failure=1 decoder_silence=1 no_device_opened=1");
        }).unwrap().join().unwrap();
    }
}
