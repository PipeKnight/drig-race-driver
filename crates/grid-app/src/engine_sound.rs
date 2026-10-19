//! Audio thread receives coherent, read-only presentation telemetry.
use bevy::{
    audio::{AddAudioSource, ChannelCount, SampleRate, Source},
    prelude::*,
};
use grid_tools::{
    audio_mix::{self, Camera, Table},
    audio_pacing::Paced,
    engine_audio::{Control, RATE},
    engine_audio_continuous::{Bank as EngineBank, CHASE_BUS_DB, Mixer},
};
use super::stereo::{Pan, Stereo};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, AtomicU64, Ordering},
    },
    time::Duration,
};

#[derive(Resource)]
pub(crate) struct EngineSound {
    /// Packed controls, one per vehicle tick (`audio_pacing`).
    mailbox: Mailbox,
    /// The last player control pushed, and the tick last published.
    pushed: Option<u64>,
    ticked: Option<(u64, u64)>,
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
            mailbox: mailbox(),
            pushed: None,
            ticked: None,
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
/// Tick-paced packed controls shared with one engine decoder.
type Mailbox = Arc<Mutex<Paced<u64>>>;
fn mailbox() -> Mailbox {
    Arc::new(Mutex::new(Paced::new(0)))
}
fn push(mailbox: &Mailbox, control: Control) {
    if let Ok(mut m) = mailbox.lock() {
        m.push(control.pack());
    }
}
/// One opponent's engine voice set (car index in the shared world).
#[derive(Component)]
pub(crate) struct OpponentEngine {
    car: usize,
    mailbox: Mailbox,
    bus: Arc<AtomicU64>,
    model: usize,
    /// Heard from the camera (the AI group's listener, 433AA0).
    pan: Pan,
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
    modes: &audio_mix::ModeMix,
    camera: Camera,
    groups: [usize; 2],
    trim: f32,
    distances: [f32; 2],
) -> [f32; 2] {
    std::array::from_fn(|k| {
        let mix = table.map_or(CHASE_BUS_DB[k], |t| {
            modes.group_db(t, camera, groups[k], 0.)
        }) + trim;
        audio_mix::distance_db(distances[k], ENGINE_DISTANCE.0, ENGINE_DISTANCE.1)
            .map_or(f32::NEG_INFINITY, |d| mix + d)
    })
}

#[derive(Asset, TypePath)]
pub(crate) struct EngineSource {
    bank: Arc<EngineBank>,
    mailbox: Mailbox,
    bus: Arc<AtomicU64>,
    failure: Arc<AtomicU32>,
    pan: Pan,
}
pub(crate) struct Decoder {
    mixer: Mixer,
    mailbox: Mailbox,
    bus: Arc<AtomicU64>,
    control: Control,
    count: usize,
    failure: Arc<AtomicU32>,
    reported_failure: bool,
}
impl Iterator for Decoder {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        // RPM and throttle advance one vehicle tick per 16 ms block, so a
        // slow frame's ticks are not collapsed into one step; audibility and
        // the reset generation follow the newest control within 64 samples.
        let block = self.mixer.block_due();
        if (block || self.count.is_multiple_of(64))
            && let Ok(mut mailbox) = self.mailbox.try_lock()
        {
            let generation = self.control.generation;
            let latest = Control::unpack(mailbox.latest());
            if block || latest.generation != generation {
                let next = Control::unpack(mailbox.take());
                self.control.normalized_rpm = next.normalized_rpm;
                self.control.throttle = next.throttle;
            }
            self.control.audible = latest.audible;
            self.control.generation = latest.generation;
            if generation != self.control.generation {
                self.reported_failure = false;
            }
        }
        if self.count.is_multiple_of(64) {
            self.mixer.bus_db = unpack_bus(self.bus.load(Ordering::Relaxed));
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
    type Decoder = Stereo<Decoder>;
    fn decoder(&self) -> Stereo<Decoder> {
        Stereo::new(
            Decoder {
                mixer: Mixer::new(self.bank.clone()),
                mailbox: self.mailbox.clone(),
                bus: self.bus.clone(),
                control: Control::default(),
                count: 0,
                failure: self.failure.clone(),
                reported_failure: false,
            },
            self.pan.clone(),
            RATE,
        )
    }
}
/// `banks` holds one bank per field car model, the player's first.
pub(crate) fn install(app: &mut App, banks: Vec<Arc<EngineBank>>, table: Option<Arc<Table>>) {
    let mut sound = EngineSound::with_models(banks);
    sound.table = table;
    app.add_audio_source::<EngineSource>()
        .insert_resource(sound)
        .init_resource::<AudioMode>()
        .add_systems(Startup, start)
        .add_systems(FixedUpdate, publish.after(super::drive_tick))
        .add_systems(Update, (mix_mode, update).chain().after(super::drive_present));
}
/// The mix mode (411C10/427ED0, `audio_mix::ModeMix`) shared by the car,
/// collision and HUD sounds.
#[derive(Resource, Default)]
pub(crate) struct AudioMode(pub(crate) audio_mix::ModeMix);
/// The race flow's `StateAudioSetState` (docs/AUDIO_RESEARCH.md §13):
/// StartLine through the start sequence (also after a restart), Normal from
/// the green light (`set_audio_normal_start_race`). Free practice has no
/// start sequence and stays Normal. Pause keeps its existing silence; the
/// Paused, Crashed and EndOfRace modes are not driven here [I].
fn mix_mode(
    mut mode: ResMut<AudioMode>,
    sound: Res<EngineSound>,
    trial: Option<Res<super::time_trial::Trial>>,
    time: Res<Time<Real>>,
) {
    let start_line = trial
        .as_ref()
        .is_some_and(|t| matches!(t.state.phase(), grid_tools::time_trial::Phase::Countdown(_)));
    let next = if start_line { audio_mix::START_LINE } else { audio_mix::NORMAL };
    if next != mode.0.mode() {
        mode.0.set(next);
        log_line!("audio_mode version=1 event=set mode={}", audio_mix::MODE_NAMES[next]);
    }
    if let Some(table) = sound.table.as_deref() {
        mode.0.advance(table, time.delta_secs());
    }
}
fn start(
    mut commands: Commands,
    sound: Res<EngineSound>,
    mut assets: ResMut<Assets<EngineSource>>,
    driving: Option<Res<super::Driving>>,
    field: Option<Res<super::Field>>,
) {
    // The player's engine and exhaust (EngineBay (0,0,4), Exhaust (0,0,-4)
    // in the car) are heard by the Car listener at the car: in front of and
    // behind it, so centred.
    let handle = assets.add(EngineSource {
        bank: sound.bank.clone(),
        mailbox: sound.mailbox.clone(),
        bus: sound.bus.clone(),
        failure: sound.failure.clone(),
        pan: Pan::centre(),
    });
    commands.spawn(AudioPlayer(handle));
    let cars = driving
        .as_ref()
        .and_then(|d| d.opponents.as_ref())
        .map_or(0, |o| o.multi.cars.len());
    for car in 1..cars {
        let mailbox = mailbox();
        let bus = Arc::new(AtomicU64::new(pack_bus([f32::NEG_INFINITY; 2])));
        // Each opponent sounds like its own car model.
        let model = field
            .as_ref()
            .and_then(|f| f.car_model.get(car).copied())
            .filter(|&m| m < sound.banks.len())
            .unwrap_or(0);
        let pan = Pan::centre();
        let handle = assets.add(EngineSource {
            bank: sound.banks[model].clone(),
            mailbox: mailbox.clone(),
            bus: bus.clone(),
            failure: sound.failure.clone(),
            pan: pan.clone(),
        });
        // Played once, as the player's engine: the decoder never ends. LOOP
        // wraps it in rodio's `repeat_infinite`, which keeps every sample
        // played since the start to replay it (F482 memory growth).
        commands.spawn((
            AudioPlayer(handle),
            PlaybackSettings::ONCE,
            OpponentEngine {
                car,
                mailbox,
                bus,
                model,
                pan,
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
    mode: Option<Res<'w, AudioMode>>,
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
    /// The mix mode's current group terms.
    pub(crate) fn modes(&self) -> audio_mix::ModeMix {
        self.mode.as_ref().map(|m| m.0.clone()).unwrap_or_default()
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
    /// Stereo gains of `emitter` heard at `listener` (A23DC0's listener
    /// space: the position relative to the listener in its orientation).
    /// Both listeners face the camera's way [I: the Car listener's chase
    /// blend 43F7A0 is not reconstructed]; the camera listener sits at the
    /// camera, the Car listener at the player's car.
    pub(crate) fn heard(&self, listener: Vec3, emitter: Vec3) -> [f32; 2] {
        let rotation = self.listener.single().map_or(Quat::IDENTITY, |t| t.rotation());
        super::stereo::pan_from(rotation, listener, emitter)
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
    let (trim, camera, modes) = (listening.trim(), listening.camera(), listening.modes());
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
            &modes,
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
    // Ticks publish their controls (`publish`); a change without a tick
    // (pause, mute, finish, failure) is published here.
    let finished = listening.stopped(&driving);
    if Some(player_control(&sound, &driving, finished).pack()) != sound.pushed {
        push_controls(&mut sound, &driving, finished, &opponent_engines);
    }
    if let Some(opponents) = &driving.opponents {
        for engine in &opponent_engines {
            let car = &opponents.multi.cars[engine.car];
            let pose = car.pose();
            engine.pan.set(listening.heard(ear, Vec3::new(pose[12], pose[13], pose[14])));
            let distances = distances(Mat4::from_cols_array(&pose), engine.model);
            engine.bus.store(
                pack_bus(bus_levels(
                    table,
                    &modes,
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

/// The player's control; silent once the race is frozen.
fn player_control(sound: &EngineSound, driving: &super::Driving, finished: bool) -> Control {
    let mut control = sound.control(driving);
    // The engine idles and may be revved during the countdown; the vehicles no
    // longer tick once the race is frozen.
    if finished {
        control.audible = false;
    }
    control
}
/// Queue the current controls for the player's and the opponents' engines.
fn push_controls(
    sound: &mut EngineSound,
    driving: &super::Driving,
    finished: bool,
    opponent_engines: &Query<&OpponentEngine>,
) {
    let control = player_control(sound, driving, finished);
    push(&sound.mailbox, control);
    sound.pushed = Some(control.pack());
    if let Some(opponents) = &driving.opponents {
        let audible = !sound.muted && !driving.paused && driving.failure.is_none() && !finished;
        for engine in opponent_engines {
            let car = &opponents.multi.cars[engine.car];
            let throttle = opponents
                .throttles
                .get(engine.car - 1)
                .copied()
                .unwrap_or(0.);
            let control = Control::vehicle(
                &car.state.car.owner.vehicle,
                if car.terminal_wheel().is_some() {
                    0.
                } else {
                    throttle
                },
                audible,
                driving.segment as u32,
            );
            push(&engine.mailbox, control);
        }
    }
}
/// Once per vehicle tick (FixedUpdate after the tick): queue its controls.
fn publish(
    mut sound: ResMut<EngineSound>,
    driving: Res<super::Driving>,
    listening: Listening,
    opponent_engines: Query<&OpponentEngine>,
) {
    let now = (driving.segment, driving.runtime.ticks);
    if sound.ticked == Some(now) {
        return;
    }
    sound.ticked = Some(now);
    let finished = listening.stopped(&driving);
    push_controls(&mut sound, &driving, finished, &opponent_engines);
}

/// Bounded real-device diagnostic, separate from silent offscreen captures.
pub(crate) fn check(root: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let bank = EngineBank::lacetti(root)?;
    let sound = EngineSound::new(bank);
    let mailbox = sound.mailbox.clone();
    push(
        &mailbox,
        Control {
            normalized_rpm: 0.5,
            throttle: 0.3,
            audible: true,
            generation: 0,
        },
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
    push(&mailbox, Control::unpack(0));
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
    /// A 50 ms frame (20 fps) runs three vehicle ticks back to back; the
    /// decoder plays them out one per 16 ms block instead of jumping to the
    /// last (docs/AUDIO_RESEARCH.md 8).
    #[test]
    #[ignore = "requires GRID_GAME_ROOT purchased sources; no audio device used"]
    fn a_slow_frame_reaches_the_mixer_one_tick_per_block() {
        std::thread::Builder::new().stack_size(16 * 1024 * 1024).spawn(|| {
            let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
            let bank = EngineBank::lacetti(std::path::Path::new(&root)).unwrap();
            let mailbox = mailbox();
            let mut decoder = EngineSource {
                bank,
                mailbox: mailbox.clone(),
                bus: Arc::new(AtomicU64::new(pack_bus(CHASE_BUS_DB))),
                failure: Arc::new(AtomicU32::new(0)),
                pan: Pan::centre(),
            }
            .decoder()
            .inner;
            let control = |rpm: f32| Control { normalized_rpm: rpm, throttle: 0.5, audible: true, generation: 0 };
            push(&mailbox, control(0.3));
            for _ in 0..grid_tools::engine_audio_continuous::BLOCK {
                decoder.next();
            }
            for rpm in [0.4, 0.5, 0.6] {
                push(&mailbox, control(rpm));
            }
            let mut seen = Vec::new();
            for _ in 0..3 {
                decoder.next();
                seen.push(decoder.control.normalized_rpm);
                for _ in 1..grid_tools::engine_audio_continuous::BLOCK {
                    decoder.next();
                }
            }
            let quantised = |rpm: f32| Control::unpack(control(rpm).pack()).normalized_rpm;
            assert_eq!(seen, [quantised(0.4), quantised(0.5), quantised(0.6)]);
        }).unwrap().join().unwrap();
    }
    #[test]
    #[ignore = "requires GRID_GAME_ROOT purchased sources; no audio device used"]
    fn source_engine_sound_pause_mute_reset_and_failure() {
        std::thread::Builder::new().stack_size(16*1024*1024).spawn(|| {
            let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
            let root = std::path::Path::new(&root);
            let bank = EngineBank::lacetti(root).unwrap();
            let mut decoder = EngineSource { bank: bank.clone(), mailbox: mailbox(), bus: Arc::new(AtomicU64::new(pack_bus(CHASE_BUS_DB))), failure: Arc::new(AtomicU32::new(0)), pan: Pan::centre() }.decoder().inner;
            let sound = EngineSound::new(bank);
            decoder.mailbox = sound.mailbox.clone();
            decoder.bus = sound.bus.clone();
            let mut app = App::new();
            app.insert_resource(sound).init_resource::<ButtonInput<KeyCode>>()
                .add_message::<AppExit>()
                .insert_resource(crate::Driving { runtime: grid_tools::source_vehicle::Runtime::lacetti(root,"slot_00",200679).unwrap(), geometry: None, paused:false, failure:None, notices:Vec::new(), scripted_ticks:None, scripted:Default::default(),handbrake_from:None, last_controls:None, segment:0, started:std::time::Instant::now(), opponents: None, auto_reset: Default::default(), reset_line: None })
                .insert_resource(crate::input::Actions::new(crate::input::ActionMap::keyboard(),crate::input::Pads::Off)).init_resource::<crate::input::ShiftQueue>().insert_resource(crate::pause_menu::PauseMenu::new(crate::pause_menu::Variant::TestDrive,false,false,Default::default())).add_systems(Update,(crate::input::sample,crate::pause_menu::update,crate::drive_actions,crate::drive_tick,update).chain());
            let control = |app: &App| Control::unpack(app.world().resource::<EngineSound>().mailbox.lock().unwrap().latest());
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
