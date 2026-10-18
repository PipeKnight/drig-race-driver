//! Audio thread receives coherent, read-only presentation telemetry.
use bevy::{
    audio::{AddAudioSource, ChannelCount, SampleRate, Source},
    prelude::*,
};
use grid_tools::{
    audio_mix::{self, Camera, Table},
    engine_audio::{Control, RATE},
    engine_audio_continuous::{Bank as EngineBank, CHASE_BUS_DB, Mixer},
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
    /// Engine/exhaust group levels in dB for the audio thread (`pack_bus`).
    bus: Arc<AtomicU64>,
    /// The original group mix (`audio/GroupModeVolumes.bin`).
    pub(crate) table: Option<Arc<Table>>,
    failure: Arc<AtomicU32>,
    bank: Arc<EngineBank>,
    /// One bank per field car model (`Field::models`); the player's is 0.
    banks: Vec<Arc<EngineBank>>,
    pub(crate) muted: bool,
}
impl EngineSound {
    pub(crate) fn new(bank: Arc<EngineBank>) -> Self {
        Self::with_models(vec![bank])
    }
    pub(crate) fn with_models(banks: Vec<Arc<EngineBank>>) -> Self {
        Self {
            mailbox: Arc::new(AtomicU64::new(0)),
            bus: Arc::new(AtomicU64::new(pack_bus(CHASE_BUS_DB))),
            table: None,
            failure: Arc::new(AtomicU32::new(0)),
            bank: banks[0].clone(),
            banks,
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
/// One opponent's engine voice set (car index in the shared world).
#[derive(Component)]
pub(crate) struct OpponentEngine {
    car: usize,
    mailbox: Arc<AtomicU64>,
    bus: Arc<AtomicU64>,
    model: usize,
}
/// audioVarieties presets `Engine`/`Exhaust`: min/max distance in metres.
const ENGINE_DISTANCE: (f32, f32) = (10., 175.);

pub(crate) fn pack_bus(db: [f32; 2]) -> u64 {
    u64::from(db[0].to_bits()) | (u64::from(db[1].to_bits()) << 32)
}
fn unpack_bus(word: u64) -> [f32; 2] {
    [
        f32::from_bits(word as u32),
        f32::from_bits((word >> 32) as u32),
    ]
}
/// Engine and exhaust group levels of one car: the group mix of its groups
/// (Engine/Exhaust for the player, AI for opponents, 4223D0) plus each
/// emitter's distance term (A23B70); `-inf` beyond the maximum distance.
pub(crate) fn bus_levels(
    table: Option<&Table>,
    camera: Camera,
    groups: [usize; 2],
    trim: f32,
    distances: [f32; 2],
) -> [f32; 2] {
    std::array::from_fn(|k| {
        let mix = table.map_or(CHASE_BUS_DB[k], |t| {
            t.group_db(audio_mix::NORMAL, camera, groups[k], 0.)
        }) + trim;
        audio_mix::distance_db(distances[k], ENGINE_DISTANCE.0, ENGINE_DISTANCE.1)
            .map_or(f32::NEG_INFINITY, |d| mix + d)
    })
}

#[derive(Asset, TypePath)]
pub(crate) struct EngineSource {
    bank: Arc<EngineBank>,
    mailbox: Arc<AtomicU64>,
    bus: Arc<AtomicU64>,
    failure: Arc<AtomicU32>,
}
pub(crate) struct Decoder {
    mixer: Mixer,
    mailbox: Arc<AtomicU64>,
    bus: Arc<AtomicU64>,
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
            self.mixer.bus_db = unpack_bus(self.bus.load(Ordering::Relaxed));
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
            bus: self.bus.clone(),
            control: Control::default(),
            count: 0,
            failure: self.failure.clone(),
            reported_failure: false,
        }
    }
}
/// `banks` holds one bank per field car model, the player's first.
pub(crate) fn install(app: &mut App, banks: Vec<Arc<EngineBank>>, table: Option<Arc<Table>>) {
    let mut sound = EngineSound::with_models(banks);
    sound.table = table;
    app.add_audio_source::<EngineSource>()
        .insert_resource(sound)
        .add_systems(Startup, start)
        .add_systems(Update, update.after(super::drive_present));
}
fn start(
    mut commands: Commands,
    sound: Res<EngineSound>,
    mut assets: ResMut<Assets<EngineSource>>,
    driving: Option<Res<super::Driving>>,
    field: Option<Res<super::Field>>,
) {
    let handle = assets.add(EngineSource {
        bank: sound.bank.clone(),
        mailbox: sound.mailbox.clone(),
        bus: sound.bus.clone(),
        failure: sound.failure.clone(),
    });
    commands.spawn(AudioPlayer(handle));
    let cars = driving
        .as_ref()
        .and_then(|d| d.opponents.as_ref())
        .map_or(0, |o| o.multi.cars.len());
    for car in 1..cars {
        let mailbox = Arc::new(AtomicU64::new(0));
        let bus = Arc::new(AtomicU64::new(pack_bus([f32::NEG_INFINITY; 2])));
        // Each opponent sounds like its own car model.
        let model = field
            .as_ref()
            .and_then(|f| f.car_model.get(car).copied())
            .filter(|&m| m < sound.banks.len())
            .unwrap_or(0);
        let handle = assets.add(EngineSource {
            bank: sound.banks[model].clone(),
            mailbox: mailbox.clone(),
            bus: bus.clone(),
            failure: sound.failure.clone(),
        });
        commands.spawn((
            AudioPlayer(handle),
            PlaybackSettings::LOOP,
            OpponentEngine {
                car,
                mailbox,
                bus,
                model,
            },
        ));
    }
    if cars > 1 {
        println!(
            "engine_audio version=3 event=opponents count={} group=AI distance_m={}..{} law=floor*((d-min)/(max-min))^2",
            cars - 1,
            ENGINE_DISTANCE.0,
            ENGINE_DISTANCE.1
        );
    }
    println!(
        "engine_audio version=2 event=queued rate={RATE} layers=dn_en,up_en,up_en_load,dn_ex,up_ex,up_ex_load voices=12 mixer=continuous_pair_scheduler fidelity=experimental mute_key=M"
    );
}
/// What the audio mix reads from the presentation: the event, the pause
/// menu (Effects Volume), the shown view and the listener.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct Listening<'w, 's> {
    pub(crate) trial: Option<Res<'w, super::time_trial::Trial>>,
    menu: Option<Res<'w, super::pause_menu::PauseMenu>>,
    cameras: Option<Res<'w, super::camera::Cameras>>,
    listener: Query<'w, 's, &'static GlobalTransform, With<super::ViewerCamera>>,
}
impl Listening<'_, '_> {
    /// Effects Volume as a group trim in dB (4124A0), not a linear gain.
    pub(crate) fn trim(&self) -> f32 {
        audio_mix::slider_db(self.menu.as_ref().map_or(1., |m| m.values.gain()))
    }
    /// The race is frozen: the player finished a single-car event, or every
    /// car finished or is out. After the player's finish in a field race the
    /// opponents race on and the player car follows the after-finish pilot
    /// (main.rs), so their engines and tyres stay audible; the original has no
    /// mute there (docs/RUNTIME_LIMITS_AUDIT.md R21).
    pub(crate) fn stopped(&self, driving: &super::Driving) -> bool {
        self.trial.as_ref().is_some_and(|t| t.finished())
            && driving.opponents.as_ref().is_none_or(|o| o.standings.over())
    }
    pub(crate) fn camera(&self) -> Camera {
        self.cameras
            .as_ref()
            .map_or(Camera::Chase, |c| c.audio_camera())
    }
    /// The listener is the rendered camera; `player` (its pose) without one.
    pub(crate) fn ear(&self, player: &[f32; 16]) -> Vec3 {
        self.listener
            .single()
            .map_or(Vec3::new(player[12], player[13], player[14]), |t| {
                t.translation()
            })
    }
}
fn update(
    mut sound: ResMut<EngineSound>,
    driving: Res<super::Driving>,
    input: Res<ButtonInput<KeyCode>>,
    sinks: Query<(), (With<AudioSink>, With<AudioPlayer<EngineSource>>)>,
    mut started: Local<bool>,
    listening: Listening,
    opponent_engines: Query<&OpponentEngine>,
) {
    let (trim, camera) = (listening.trim(), listening.camera());
    let table = sound.table.clone();
    let table = table.as_deref();
    let ear = listening.ear(&driving.runtime.pose());
    let player = Mat4::from_cols_array(&driving.runtime.pose());
    let emitters: Vec<_> = sound.banks.iter().map(|b| b.emitters).collect();
    let distances = |pose: Mat4, model: usize| {
        emitters[model].map(|e| pose.transform_point3(Vec3::from_array(e)).distance(ear))
    };
    sound.bus.store(
        pack_bus(bus_levels(
            table,
            camera,
            [audio_mix::ENGINE, audio_mix::EXHAUST],
            trim,
            distances(player, 0),
        )),
        Ordering::Relaxed,
    );
    if sound.failure.swap(0, Ordering::Relaxed) != 0 && !sound.muted {
        sound.muted = true;
        eprintln!("engine_audio version=2 event=decoder_failed muted=true reset_key=R");
    }
    if input.just_pressed(KeyCode::KeyM) {
        sound.muted = !sound.muted;
        log_line!("engine_audio version=2 event=mute muted={}", sound.muted);
    }
    if !*started && !sinks.is_empty() {
        *started = true;
        log_line!("engine_audio version=2 event=output_started");
    }
    let mut control = sound.control(&driving);
    // The engine idles and may be revved during the countdown; the vehicles no
    // longer tick once the race is frozen.
    let finished = listening.stopped(&driving);
    if finished {
        control.audible = false;
    }
    sound.mailbox.store(control.pack(), Ordering::Relaxed);
    if let Some(opponents) = &driving.opponents {
        let audible = !sound.muted && !driving.paused && driving.failure.is_none() && !finished;
        for engine in &opponent_engines {
            let car = &opponents.multi.cars[engine.car];
            let throttle = opponents
                .throttles
                .get(engine.car - 1)
                .copied()
                .unwrap_or(0.);
            let control = grid_tools::engine_audio::Control::vehicle(
                &car.state.car.owner.vehicle,
                if car.terminal_wheel().is_some() {
                    0.
                } else {
                    throttle
                },
                audible,
                driving.segment as u32,
            );
            engine.mailbox.store(control.pack(), Ordering::Relaxed);
            let distances = distances(Mat4::from_cols_array(&car.pose()), engine.model);
            engine.bus.store(
                pack_bus(bus_levels(
                    table,
                    camera,
                    [audio_mix::AI; 2],
                    trim,
                    distances,
                )),
                Ordering::Relaxed,
            );
        }
    }
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
            let mut decoder = EngineSource { bank: bank.clone(), mailbox: Arc::new(AtomicU64::new(0)), bus: Arc::new(AtomicU64::new(pack_bus(CHASE_BUS_DB))), failure: Arc::new(AtomicU32::new(0)) }.decoder();
            let sound = EngineSound::new(bank);
            decoder.mailbox = sound.mailbox.clone();
            decoder.bus = sound.bus.clone();
            let mut app = App::new();
            app.insert_resource(sound).init_resource::<ButtonInput<KeyCode>>()
                .add_message::<AppExit>()
                .insert_resource(crate::Driving { runtime: grid_tools::source_vehicle::Runtime::lacetti(root,"slot_00",200679).unwrap(), geometry: None, paused:false, failure:None, notices:Vec::new(), scripted_ticks:None, scripted:Default::default(),handbrake_from:None, last_controls:None, segment:0, started:std::time::Instant::now(), opponents: None, auto_reset: Default::default(), reset_line: None })
                .insert_resource(crate::input::Actions::new(crate::input::ActionMap::keyboard(),crate::input::Pads::Off)).init_resource::<crate::input::ShiftQueue>().insert_resource(crate::pause_menu::PauseMenu::new(crate::pause_menu::Variant::TestDrive,false,false,Default::default())).add_systems(Update,(crate::input::sample,crate::pause_menu::update,crate::drive_actions,crate::drive_tick,update).chain());
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
