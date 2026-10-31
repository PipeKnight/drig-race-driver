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
    engine_audio_whine::{self, Whine},
    mech_audio,
};
use super::stereo::{Pan, Rate, Stereo};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicU32, AtomicU64, Ordering},
    },
    time::Duration,
};

#[derive(Resource)]
pub(crate) struct EngineSound {
    /// Packed controls and the whine, one per vehicle tick (`audio_pacing`).
    mailbox: Mailbox,
    /// The last player control pushed, and the tick last published.
    pushed: Option<u64>,
    ticked: Option<(u64, u64)>,
    /// Engine/exhaust group levels in dB for the audio thread (`pack_bus`).
    bus: Arc<AtomicU64>,
    /// The player's mechanical-sound levels (clunk, backfire, turbo, whine).
    mech: MechBus,
    /// Each car's gear whine (4518E0), run once per vehicle tick; index 0 is
    /// the player, then the opponents by car index.
    whines: Vec<CarWhine>,
    /// The original group mix (`audio/GroupModeVolumes.bin`).
    pub(crate) table: Option<Arc<Table>>,
    failure: Arc<AtomicU32>,
    bank: Option<Arc<EngineBank>>,
    /// One bank per field car model (`Field::models`); the player's is 0.
    /// `None` where the model's bank failed to load: that car is silent.
    banks: Vec<Option<Arc<EngineBank>>>,
    pub(crate) muted: bool,
}
impl EngineSound {
    pub(crate) fn new(bank: Arc<EngineBank>) -> Self {
        Self::with_models(vec![Some(bank)])
    }
    pub(crate) fn with_models(banks: Vec<Option<Arc<EngineBank>>>) -> Self {
        Self {
            mailbox: mailbox(),
            pushed: None,
            ticked: None,
            bus: Arc::new(AtomicU64::new(pack_bus(CHASE_BUS_DB))),
            mech: mech_bus(),
            whines: Vec::new(),
            table: None,
            failure: Arc::new(AtomicU32::new(0)),
            bank: banks.first().cloned().flatten(),
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
                Control::applied_throttle(&driving.runtime.state.car.owner.vehicle)
            },
            !self.muted && !driving.paused && driving.failure.is_none(),
            driving.segment as u32,
        )
    }
}
/// The player's mechanical-sound levels for the audio thread
/// (`mech_audio::Levels::to_bits`).
type MechBus = Arc<[AtomicU32; 9]>;
fn mech_bus() -> MechBus {
    Arc::new(mech_audio::Levels::SILENT.to_bits().map(AtomicU32::new))
}
/// One car's whine model, its segment, and its last voice
/// (`engine_audio_whine::Voice::pack`, 0 for none).
#[derive(Default)]
struct CarWhine {
    model: Option<(u64, Whine)>,
    word: u64,
}
impl CarWhine {
    /// 4518E0 for one tick of the car's `words`; a new segment starts over.
    fn tick(&mut self, settings: Option<&engine_audio_whine::Settings>, segment: u64, seed: u32, words: &[u32]) {
        let Some(settings) = settings else {
            self.word = 0;
            return;
        };
        if self.model.as_ref().is_none_or(|(s, _)| *s != segment) {
            self.model = Some((segment, Whine::new(settings.clone(), seed)));
        }
        if let Some((_, whine)) = &mut self.model {
            self.word = engine_audio_whine::Voice::pack(whine.frame(0.016, engine_audio_whine::Input::vehicle(words)));
        }
    }
}
/// Tick-paced packed controls shared with one engine decoder, each with
/// the whine's voice (`engine_audio_whine::Voice::pack`, 0 for none).
type Mailbox = Arc<Mutex<Paced<[u64; 2]>>>;
fn mailbox() -> Mailbox {
    Arc::new(Mutex::new(Paced::new([0; 2])))
}
fn push(mailbox: &Mailbox, control: Control) {
    send(mailbox, control, 0);
}
/// The mechanical word: the whine's voice and, in bit 60, the handbrake
/// button (vehicle +0xE38, 43C320's input).
const HANDBRAKE_BIT: u64 = 1 << 60;
fn handbrake(words: &[u32]) -> u64 {
    if words.get(0xe38 / 4).is_some_and(|w| w & 255 != 0) { HANDBRAKE_BIT } else { 0 }
}
fn send(mailbox: &Mailbox, control: Control, whine: u64) {
    if let Ok(mut m) = mailbox.lock() {
        m.push([control.pack(), whine]);
    }
}
/// One opponent's engine voice set (car index in the shared world).
#[derive(Component)]
pub(crate) struct OpponentEngine {
    car: usize,
    mailbox: Mailbox,
    bus: Arc<AtomicU64>,
    /// Its mechanical-sound levels: the AI group at its own emitters.
    mech: MechBus,
    model: usize,
    /// Heard from the camera (the AI group's listener, 433AA0).
    pan: Pan,
    /// Its Doppler factor from the camera (Doppler property 1 for AI cars, 0x42EEDB).
    doppler: Rate,
    /// This car's own decoder failure: its mixer stays silent until the
    /// next reset, the other engines play on.
    failure: Arc<AtomicU32>,
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
    /// The car's Doppler factor; `None` for the player's car, whose engine voices have
    /// Doppler 0 outside the detached cameras (0x42EEDB), which the port does not have.
    doppler: Option<Rate>,
    /// The car's mechanical-sound levels (`mech_audio::Levels::to_bits`);
    /// `None` when its model has no mec bank.
    mech: Option<MechBus>,
    /// The player's car (`Mixer::player`): clunk, turbo, focused backfire.
    player: bool,
}
pub(crate) struct Decoder {
    mixer: Mixer,
    mailbox: Mailbox,
    bus: Arc<AtomicU64>,
    mech: Option<MechBus>,
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
            let latest = Control::unpack(mailbox.latest()[0]);
            if block || latest.generation != generation {
                let [next, whine] = mailbox.take();
                let next = Control::unpack(next);
                self.mixer.whine = engine_audio_whine::Voice::unpack(whine);
                self.mixer.handbrake = whine & HANDBRAKE_BIT != 0;
                self.control.normalized_rpm = next.normalized_rpm;
                self.control.throttle = next.throttle;
                self.control.gear = next.gear;
            }
            self.control.audible = latest.audible;
            self.control.generation = latest.generation;
            if generation != self.control.generation {
                self.reported_failure = false;
            }
        }
        if self.count.is_multiple_of(64) {
            self.mixer.bus_db = unpack_bus(self.bus.load(Ordering::Relaxed));
            if let Some(mech) = &self.mech {
                self.mixer.mech_levels = mech_audio::Levels::from_bits(mech.each_ref().map(|a| a.load(Ordering::Relaxed)));
            }
        }
        self.count = self.count.wrapping_add(1);
        let value = self.mixer.sample(self.control);
        if let Some(error) = self.mixer.failure
            && !self.reported_failure
        {
            // The reason, once per failure (the audio thread; the main
            // thread's decoder_failed line follows).
            eprintln!(
                "engine_audio version=2 event=decoder_error error=\"{error}\" normalized_rpm={} throttle={} generation={}",
                self.control.normalized_rpm, self.control.throttle, self.control.generation
            );
            self.failure.store(1, Ordering::Relaxed);
            self.reported_failure = true;
        }
        Some(value)
    }
}
#[cfg(test)]
impl Decoder {
    /// The mixer's failure (the race-start mix test, main.rs).
    pub(crate) fn failure(&self) -> Option<&'static str> {
        self.mixer.failure
    }
}
/// An extra engine voice outside the ECS (the race-start mix test, main.rs):
/// its decoder, a control push per vehicle tick, and its group levels.
#[cfg(test)]
pub(crate) struct TestVoice {
    pub(crate) decoder: Stereo<Decoder>,
    mailbox: Mailbox,
    bus: Arc<AtomicU64>,
}
#[cfg(test)]
impl TestVoice {
    pub(crate) fn new(bank: Arc<EngineBank>, pan: Pan) -> Self {
        let source = EngineSource {
            bank,
            mailbox: mailbox(),
            bus: Arc::new(AtomicU64::new(pack_bus([f32::NEG_INFINITY; 2]))),
            failure: Arc::new(AtomicU32::new(0)),
            pan,
            doppler: None,
            mech: None,
            player: false,
        };
        Self { decoder: source.decoder(), mailbox: source.mailbox, bus: source.bus }
    }
    pub(crate) fn push(&self, control: Control) {
        push(&self.mailbox, control);
    }
    pub(crate) fn set_bus(&self, db: [f32; 2]) {
        self.bus.store(pack_bus(db), Ordering::Relaxed);
    }
}
#[cfg(test)]
impl EngineSound {
    /// The player's published engine and exhaust group levels in dB.
    pub(crate) fn bus_db(&self) -> [f32; 2] {
        unpack_bus(self.bus.load(Ordering::Relaxed))
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
        let decoder = Decoder {
                mixer: {
                    let mut mixer = Mixer::new(self.bank.clone());
                    mixer.player = self.player;
                    if self.mech.is_some() {
                        mixer.mech = self.bank.mech.clone().map(|m| mech_audio::Voices::new(m, RATE));
                    }
                    mixer
                },
                mailbox: self.mailbox.clone(),
                bus: self.bus.clone(),
                mech: self.mech.clone(),
                control: Control::default(),
                count: 0,
                failure: self.failure.clone(),
                reported_failure: false,
            };
        match &self.doppler {
            Some(doppler) => Stereo::with_doppler(decoder, self.pan.clone(), RATE, doppler.clone()),
            None => Stereo::new(decoder, self.pan.clone(), RATE),
        }
    }
}
/// One bank per field car model, the player's first. A model whose bank
/// fails to load is `None` and logged: that car is silent, the others play.
pub(crate) fn load_banks(root: &std::path::Path, models: &[grid_tools::car::Car]) -> Vec<Option<Arc<EngineBank>>> {
    models
        .iter()
        .map(|m| match EngineBank::load(root, m) {
            Ok(mut bank) => {
                // Each car's mechanical sounds come from its carsetup MechBank
                // (the clunk and turbo of the player only, `Mixer::player`).
                match engine_audio_whine::Settings::load(root, m) {
                    Ok(whine) => Arc::get_mut(&mut bank).unwrap().whine = Some(whine),
                    Err(error) => {
                        warn!("Gear whine of {} unavailable: {error}", m.stem());
                        eprintln!("engine_audio version=2 event=whine_failed car={} error=\"{error}\"", m.stem());
                    }
                }
                match mech_audio::Bank::load(root, &bank.mechanics_bank) {
                    Ok(mech) => Arc::get_mut(&mut bank).unwrap().mech = Some(mech),
                    Err(error) => {
                        warn!("Mechanical sounds of {} unavailable: {error}", m.stem());
                        eprintln!("engine_audio version=2 event=mech_bank_failed car={} error=\"{error}\"", m.stem());
                    }
                }
                Some(bank)
            }
            Err(error) => {
                warn!("Engine audio of {} unavailable; that car is silent: {error}", m.stem());
                eprintln!("engine_audio version=2 event=bank_failed car={} error=\"{error}\"", m.stem());
                None
            }
        })
        .collect()
}
/// `banks` holds one bank per field car model, the player's first; a model
/// whose bank failed to load (`None`) is silent, the others play.
pub(crate) fn install(app: &mut App, banks: Vec<Option<Arc<EngineBank>>>, table: Option<Arc<Table>>) {
    app.add_audio_source::<EngineSource>();
    install_systems(app, banks, table);
}
/// `install` without the device playback: the race-start mix test
/// (main.rs) registers the asset and pulls the decoders itself.
pub(crate) fn install_systems(app: &mut App, banks: Vec<Option<Arc<EngineBank>>>, table: Option<Arc<Table>>) {
    let mut sound = EngineSound::with_models(banks);
    sound.table = table;
    app.insert_resource(sound)
        .init_resource::<AudioMode>()
        .init_resource::<ListenerMotion>()
        .add_systems(Startup, start)
        .add_systems(FixedUpdate, publish.after(super::drive_tick))
        .add_systems(Update, (listener_motion, mix_mode, update).chain().after(super::drive_present));
}
/// The camera listener's motion (0x43F570): the camera's displacement over the frame divided
/// by the frame time, kept only below `audio_mix::LISTENER_JUMP` (8 m).
#[derive(Resource, Default)]
pub(crate) struct ListenerMotion {
    last: Option<Vec3>,
    pub(crate) velocity: Vec3,
}
fn listener_motion(
    time: Res<Time>,
    mut motion: ResMut<ListenerMotion>,
    listener: Query<&GlobalTransform, With<super::ViewerCamera>>,
) {
    let Ok(at) = listener.single().map(|t| t.translation()) else { return };
    let dt = time.delta_secs();
    let moved = motion.last.map_or(Vec3::ZERO, |last| at - last);
    motion.velocity = if dt > 0. && moved.length() < audio_mix::LISTENER_JUMP { moved / dt } else { Vec3::ZERO };
    motion.last = Some(at);
}
/// The mix mode (411C10/427ED0, `audio_mix::ModeMix`) shared by the car,
/// collision and HUD sounds; behind a lock so the collision tick can switch
/// to Crashed while it reads the levels.
#[derive(Resource, Default)]
pub(crate) struct AudioMode(pub(crate) Mutex<audio_mix::ModeMix>);
/// The race flow's `StateAudioSetState` (docs/AUDIO_RESEARCH.md §13):
/// StartLine through the start sequence (also after a restart), Normal from
/// the green light (`set_audio_normal_start_race`), EndOfRace on the results
/// screen (`c_post_set_audio_endofrace`, the parent of
/// `results_show_screen`). Free practice has no start sequence and stays
/// Normal. Like the flow's states, a mode is set when the flow changes, so a
/// Crashed in between (collision_sound) returns by itself. Pause keeps its
/// existing silence; Paused, PausedReplay and Transition (replays) are not
/// driven here [I].
/// The state flow's `StateAudioSetState` modes (docs/AUDIO_RESEARCH.md §13, §33;
/// `flow.bin`): the pause menu and a replay from it are Paused (`c_pause_set_audio_paused`),
/// and leaving the pause sets Normal (`c_pause_set_audio_normal`), even during the start
/// lights; a restart returns to StartLine. The post-race stats are Intro
/// (`c_post_set_audio_intro`), the results EndOfRace, a replay from the results Normal
/// (`c_post_set_audio_normal`). The in-race terminal-damage replay changes nothing.
#[allow(clippy::too_many_arguments)]
fn mix_mode(
    mode: Res<AudioMode>,
    sound: Res<EngineSound>,
    trial: Option<Res<super::time_trial::Trial>>,
    post: Option<Res<super::post_race::PostRace>>,
    pause: Option<Res<super::pause_menu::PauseMenu>>,
    replay: Option<Res<super::replay::Replay>>,
    driving: Option<Res<super::Driving>>,
    time: Res<Time<Real>>,
    mut flow: Local<Option<usize>>,
    mut resumed: Local<Option<u64>>,
) {
    use super::post_race::{Return, Stage};
    use super::replay::Origin;
    let start_line = trial
        .as_ref()
        .is_some_and(|t| matches!(t.state.phase(), grid_tools::time_trial::Phase::Countdown(_)));
    let stage = post.as_ref().map(|p| p.stage);
    let results = matches!(stage, Some(Stage::Results | Stage::Confirm(_, Return::Results)));
    let origin = replay.as_ref().and_then(|r| r.session.as_ref()).map(|s| s.origin);
    let paused = pause.as_ref().is_some_and(|p| p.open) || origin == Some(Origin::PauseMenu);
    let segment = driving.as_ref().map_or(0, |d| d.segment);
    // A pause left during the start lights keeps Normal until the lights end or a restart
    // (a new segment) runs the pre-race again.
    if paused && start_line {
        *resumed = Some(segment);
    } else if !start_line || *resumed != Some(segment) {
        *resumed = None;
    }
    let next = if paused {
        audio_mix::PAUSED
    } else if origin == Some(Origin::Results) {
        audio_mix::NORMAL
    } else if stage == Some(Stage::Stats) {
        audio_mix::INTRO
    } else if results {
        audio_mix::END_OF_RACE
    } else if start_line && resumed.is_none() {
        audio_mix::START_LINE
    } else {
        audio_mix::NORMAL
    };
    let Ok(mut mix) = mode.0.lock() else { return };
    if *flow != Some(next) {
        *flow = Some(next);
        if next != mix.mode() {
            mix.set(next);
            log_line!("audio_mode version=1 event=set mode={}", audio_mix::MODE_NAMES[next]);
        }
    }
    if let Some(table) = sound.table.as_deref() {
        mix.advance(table, time.delta_secs());
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
    if let Some(bank) = sound.bank.clone() {
        let handle = assets.add(EngineSource {
            bank,
            mailbox: sound.mailbox.clone(),
            bus: sound.bus.clone(),
            failure: sound.failure.clone(),
            pan: Pan::centre(),
            doppler: None,
            mech: Some(sound.mech.clone()),
            player: true,
        });
        commands.spawn(AudioPlayer(handle));
    }
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
        let Some(bank) = sound.banks[model].clone() else {
            continue;
        };
        let pan = Pan::centre();
        let doppler = Rate::unit();
        let failure = Arc::new(AtomicU32::new(0));
        // Its backfire and gear whine (451260, 4518E0) from its own mec bank.
        let mech = mech_bus();
        let handle = assets.add(EngineSource {
            mech: bank.mech.is_some().then(|| mech.clone()),
            bank,
            mailbox: mailbox.clone(),
            bus: bus.clone(),
            failure: failure.clone(),
            pan: pan.clone(),
            doppler: Some(doppler.clone()),
            player: false,
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
                mech,
                model,
                pan,
                doppler,
                failure,
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
    motion: Option<Res<'w, ListenerMotion>>,
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
    /// Speech Volume as the Codriver/3DSpeech group trim in dB (4124A0, slider kind 2).
    pub(crate) fn speech_trim(&self) -> f32 {
        audio_mix::slider_db(self.menu.as_ref().map_or(1., |m| f32::from(m.values.speech) / 100.))
    }
    /// The mix mode's current group terms.
    pub(crate) fn modes(&self) -> audio_mix::ModeMix {
        self.mode.as_ref().and_then(|m| m.0.lock().ok().map(|m| m.clone())).unwrap_or_default()
    }
    /// 4535A0's request for the Crashed mix (0.2 s); whether it was set.
    pub(crate) fn crash(&self, table: &Table) -> bool {
        let Some(Ok(mut mix)) = self.mode.as_ref().map(|m| m.0.lock()) else { return false };
        let set = mix.set_for(table, audio_mix::CRASHED, audio_mix::CRASH_SECONDS);
        if set {
            log_line!("audio_mode version=1 event=set mode=Crashed seconds={}", audio_mix::CRASH_SECONDS);
        }
        set
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
    /// The camera listener's velocity (0x43F570), for the Doppler term.
    pub(crate) fn camera_velocity(&self) -> Vec3 {
        self.motion.as_ref().map_or(Vec3::ZERO, |m| m.velocity)
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
    // EngineBay (0, 0, 4) and Exhaust (0, 0, -4) for a silent model.
    let emitters: Vec<_> = sound
        .banks
        .iter()
        .map(|b| b.as_ref().map_or([[0., 0., 4.], [0., 0., -4.]], |b| b.emitters))
        .collect();
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
    // The player's clunk (Gearbox), backfire (Exhaust), turbo loops (Turbo),
    // turbo dump and pip (Engine) and handbrake (Other Mech) at the GearBox,
    // Exhaust, EngineBay and Cockpit emitters.
    let (gearbox, cockpit) = sound.bank.as_ref().map_or(([0., 0., 4.], [0.; 3]), |b| (b.gearbox, b.cockpit));
    let group = |g: usize| table.map_or(f32::NEG_INFINITY, |t| modes.group_db(t, camera, g, trim));
    let levels = mech_audio::Levels {
        group_db: [audio_mix::GEARBOX, audio_mix::EXHAUST, audio_mix::TURBO, audio_mix::ENGINE, audio_mix::OTHER_MECH]
            .map(group),
        distance: [
            player.transform_point3(Vec3::from_array(gearbox)).distance(ear),
            player.transform_point3(Vec3::from_array(emitters[0][1])).distance(ear),
            player.transform_point3(Vec3::from_array(emitters[0][0])).distance(ear),
            player.transform_point3(Vec3::from_array(cockpit)).distance(ear),
        ],
    };
    for (a, bits) in sound.mech.iter().zip(levels.to_bits()) {
        a.store(bits, Ordering::Relaxed);
    }
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
            if engine.failure.swap(0, Ordering::Relaxed) != 0 {
                eprintln!("engine_audio version=2 event=decoder_failed car={} muted=car_only", engine.car);
            }
            let car = &opponents.multi.cars[engine.car];
            let pose = car.pose();
            let at = Vec3::new(pose[12], pose[13], pose[14]);
            engine.pan.set(listening.heard(ear, at));
            // Its engine voices carry the car's rigid-body velocity (0x42EF1B); the camera
            // listener moves with the camera (0x43F570).
            let v = car.velocity();
            let relative = Vec3::new(v[0], v[1], v[2]) - listening.camera_velocity();
            engine.doppler.set(audio_mix::doppler((at - ear).to_array(), relative.to_array()));
            let at = Mat4::from_cols_array(&pose);
            let distances = distances(at, engine.model);
            // Its backfire and whine: the AI group (4223D0: 0x11 for all of an
            // opponent's groups) at its GearBox, Exhaust and EngineBay.
            let (gearbox, cockpit) =
                sound.banks[engine.model].as_ref().map_or(([0., 0., 4.], [0.; 3]), |b| (b.gearbox, b.cockpit));
            let ai = table.map_or(f32::NEG_INFINITY, |t| modes.group_db(t, camera, audio_mix::AI, trim));
            let levels = mech_audio::Levels {
                group_db: [ai; 5],
                distance: [
                    at.transform_point3(Vec3::from_array(gearbox)).distance(ear),
                    distances[1],
                    distances[0],
                    at.transform_point3(Vec3::from_array(cockpit)).distance(ear),
                ],
            };
            for (a, bits) in engine.mech.iter().zip(levels.to_bits()) {
                a.store(bits, Ordering::Relaxed);
            }
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
    let word = sound.whines.first().map_or(0, |w| w.word) | handbrake(&driving.runtime.state.car.owner.vehicle);
    send(&sound.mailbox, control, word);
    sound.pushed = Some(control.pack());
    if let Some(opponents) = &driving.opponents {
        let audible = !sound.muted && !driving.paused && driving.failure.is_none() && !finished;
        for engine in opponent_engines {
            let car = &opponents.multi.cars[engine.car];
            let throttle = Control::applied_throttle(&car.state.car.owner.vehicle);
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
            send(&engine.mailbox, control, sound.whines.get(engine.car).map_or(0, |w| w.word));
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
    // 4518E0 for every car, per tick on its vehicle words.
    let segment = driving.segment;
    let EngineSound { banks, whines, .. } = &mut *sound;
    let cars = driving.opponents.as_ref().map_or(1, |o| o.multi.cars.len().max(1));
    whines.resize_with(cars, CarWhine::default);
    let settings = |model: usize| banks.get(model).and_then(|b| b.as_ref()).and_then(|b| b.whine.as_ref());
    whines[0].tick(settings(0), segment, 0x5eed, &driving.runtime.state.car.owner.vehicle);
    if let Some(opponents) = &driving.opponents {
        for engine in &opponent_engines {
            if let (Some(whine), Some(car)) = (whines.get_mut(engine.car), opponents.multi.cars.get(engine.car)) {
                whine.tick(settings(engine.model), segment, 0x5eed ^ engine.car as u32, &car.state.car.owner.vehicle);
            }
        }
    }
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
            gear: 0,
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
                doppler: None,
                mech: None,
                player: true,
            }
            .decoder()
            .inner;
            // Each tick in its own gear: the gear reaches the mixer with the
            // RPM (before the gear clunk port it stayed 0, so no shift effect played).
            let control = |rpm: f32| Control { normalized_rpm: rpm, throttle: 0.5, audible: true, generation: 0, gear: (rpm * 10.).round() as u32 };
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
                seen.push((decoder.control.normalized_rpm, decoder.control.gear));
                for _ in 1..grid_tools::engine_audio_continuous::BLOCK {
                    decoder.next();
                }
            }
            let quantised = |rpm: f32| Control::unpack(control(rpm).pack()).normalized_rpm;
            assert_eq!(seen, [(quantised(0.4), 4), (quantised(0.5), 5), (quantised(0.6), 6)]);
        }).unwrap().join().unwrap();
    }
    /// Row 71: an opponent's engine heard through its Doppler factor. The Lacetti's mono mix at
    /// a steady 50 % RPM, played through `Stereo::with_doppler` once still and once approaching
    /// the camera at 30 m/s from 50 m ahead: the fundamental period (autocorrelation peak) of
    /// the output shortens by f = 343/(343 − 30·50/50.01).
    #[test]
    #[ignore = "requires GRID_GAME_ROOT purchased sources; no audio device used"]
    fn an_approaching_opponent_engine_rises_by_the_doppler_factor() {
        std::thread::Builder::new().stack_size(16 * 1024 * 1024).spawn(|| {
            let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
            let bank = EngineBank::lacetti(std::path::Path::new(&root)).unwrap();
            let source = EngineSource {
                bank,
                mailbox: mailbox(),
                bus: Arc::new(AtomicU64::new(pack_bus([0.; 2]))),
                failure: Arc::new(AtomicU32::new(0)),
                pan: Pan::centre(),
                doppler: None,
                mech: None,
                player: false,
            };
            let mut engine = source.decoder().inner;
            let control = Control { normalized_rpm: 0.5, throttle: 1., audible: true, generation: 0, gear: 3 };
            let mono: Vec<f32> = (0..RATE as usize * 4)
                .map(|frame| {
                    if frame % grid_tools::engine_audio_continuous::BLOCK == 0 {
                        push(&source.mailbox, control);
                    }
                    engine.next().unwrap()
                })
                .collect();
            // The fundamental period in samples of the second after the start-up second: the
            // first autocorrelation peak above 90 % of the largest one past the first dip.
            let period = |x: &[f32]| {
                let w = &x[RATE as usize..RATE as usize * 2];
                let ac: Vec<f64> = (0..2000).map(|l| w[..w.len() - l].iter().zip(&w[l..]).map(|(a, b)| f64::from(a * b)).sum()).collect();
                let dip = ac.iter().position(|&v| v < 0.).unwrap_or(1);
                let best = ac[dip..].iter().cloned().fold(f64::MIN, f64::max);
                let mut l = (dip..ac.len()).find(|&l| ac[l] > 0.9 * best).unwrap();
                while l + 1 < ac.len() && ac[l + 1] > ac[l] {
                    l += 1;
                }
                // Parabolic refinement of the peak.
                let (a, b, c) = (ac[l - 1], ac[l], ac[l + 1]);
                l as f64 + 0.5 * (a - c) / (a - 2. * b + c)
            };
            let heard = |factor: f32| -> Vec<f32> {
                let rate = Rate::unit();
                rate.set(factor);
                let mut s = Stereo::with_doppler(mono.clone().into_iter(), Pan::centre(), RATE, rate);
                std::iter::from_fn(|| {
                    let left = s.next()?;
                    s.next();
                    Some(left)
                })
                .collect()
            };
            let f = audio_mix::doppler([0., 0., 50.], [0., 0., -30.]);
            let (still, approaching) = (period(&heard(1.)), period(&heard(f)));
            let ratio = still / approaching;
            let rms = (mono[RATE as usize..].iter().map(|v| f64::from(v * v)).sum::<f64>() / (mono.len() - RATE as usize) as f64).sqrt();
            println!(
                "engine_audio version=3 doppler factor={f:.4} period_still={still:.2} period_approaching={approaching:.2} ratio={ratio:.4} rms={rms:.5}"
            );
            assert!(rms > 1e-3, "silent engine");
            assert!((ratio / f64::from(f) - 1.).abs() < 0.01, "ratio {ratio} vs factor {f}");
        }).unwrap().join().unwrap();
    }
    /// Row 71: the flow's audio states (§33). Pause → Paused, leaving it → Normal even during
    /// the start lights (until a restart), post-race stats → Intro, results → EndOfRace.
    #[test]
    fn mix_mode_follows_the_state_flow() {
        use crate::{pause_menu::{PauseMenu, Variant}, post_race::{PostRace, Stage}, time_trial::Trial};
        use grid_tools::time_trial::State;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(EngineSound::with_models(Vec::new()))
            .init_resource::<AudioMode>()
            .insert_resource(PauseMenu::new(Variant::TestDrive, false, false, Default::default()))
            .add_systems(Update, mix_mode);
        let countdown = |app: &mut App, seconds: f64| {
            let mut trial = Trial::new(1, grid_tools::time_trial::Mode::Wheels, false).unwrap();
            trial.state = State::new(1, f64::from(grid_tools::source_vehicle::DELTA), seconds).unwrap();
            app.insert_resource(trial);
        };
        let mode = |app: &mut App| {
            app.update();
            app.world().resource::<AudioMode>().0.lock().unwrap().mode()
        };
        let pause = |app: &mut App, open: bool| app.world_mut().resource_mut::<PauseMenu>().open = open;
        countdown(&mut app, 3.);
        assert_eq!(mode(&mut app), audio_mix::START_LINE);
        pause(&mut app, true);
        assert_eq!(mode(&mut app), audio_mix::PAUSED);
        pause(&mut app, false);
        // `c_pause_set_audio_normal`, though the lights still run.
        assert_eq!(mode(&mut app), audio_mix::NORMAL);
        assert_eq!(mode(&mut app), audio_mix::NORMAL);
        // The lights end, then a new countdown (a restart) is StartLine again.
        countdown(&mut app, 0.);
        assert_eq!(mode(&mut app), audio_mix::NORMAL);
        countdown(&mut app, 3.);
        assert_eq!(mode(&mut app), audio_mix::START_LINE);
        countdown(&mut app, 0.);
        let mut post = PostRace::new(Variant::TestDrive);
        post.stage = Stage::Stats;
        app.insert_resource(post);
        assert_eq!(mode(&mut app), audio_mix::INTRO);
        app.world_mut().resource_mut::<PostRace>().stage = Stage::Results;
        assert_eq!(mode(&mut app), audio_mix::END_OF_RACE);
        pause(&mut app, true);
        assert_eq!(mode(&mut app), audio_mix::PAUSED);
    }
    /// A model whose bank fails to load leaves only that car silent: the
    /// others load, and the player's source is spawned only with a bank.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT purchased sources; no audio device used"]
    fn a_failed_bank_silences_only_its_car() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let models = grid_tools::car::Car::list("lac,zzz,bmw").unwrap();
        let banks = load_banks(std::path::Path::new(&root), &models);
        assert_eq!(banks.iter().map(Option::is_some).collect::<Vec<_>>(), [true, false, true]);
        let spawned = |banks: Vec<Option<Arc<EngineBank>>>| {
            crate::mobile::init_test_pool();
            let mut app = App::new();
            app.add_plugins((bevy::asset::AssetPlugin::default(), bevy::app::TaskPoolPlugin::default()))
                .init_asset::<EngineSource>()
                .insert_resource(EngineSound::with_models(banks))
                .add_systems(Startup, start);
            app.update();
            app.world_mut().query::<&AudioPlayer<EngineSource>>().iter(app.world()).count()
        };
        assert_eq!(spawned(banks.clone()), 1);
        assert_eq!(spawned(banks[1..].to_vec()), 0);
    }
    #[test]
    #[ignore = "requires GRID_GAME_ROOT purchased sources; no audio device used"]
    fn source_engine_sound_pause_mute_reset_and_failure() {
        std::thread::Builder::new().stack_size(16*1024*1024).spawn(|| {
            let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
            let root = std::path::Path::new(&root);
            let bank = EngineBank::lacetti(root).unwrap();
            let mut decoder = EngineSource { bank: bank.clone(), mailbox: mailbox(), bus: Arc::new(AtomicU64::new(pack_bus(CHASE_BUS_DB))), failure: Arc::new(AtomicU32::new(0)), pan: Pan::centre(), doppler: None, mech: None, player: true }.decoder().inner;
            let sound = EngineSound::new(bank);
            decoder.mailbox = sound.mailbox.clone();
            decoder.bus = sound.bus.clone();
            crate::mobile::init_test_pool();
            let mut app = App::new();
            app.insert_resource(sound).init_resource::<ButtonInput<KeyCode>>()
                .add_message::<AppExit>()
                .insert_resource(crate::Driving { runtime: grid_tools::source_vehicle::Runtime::lacetti(root,"slot_00",200679).unwrap(), geometry: None, paused:false, failure:None, notices:Vec::new(), scripted_ticks:None, scripted:Default::default(),handbrake_from:None, last_controls:None, segment:0, started:std::time::Instant::now(), opponents: None, auto_reset: Default::default(), reset_line: None })
                .insert_resource(crate::input::Actions::new(crate::input::ActionMap::keyboard(),crate::input::Pads::Off)).init_resource::<crate::input::ShiftQueue>().insert_resource(crate::pause_menu::PauseMenu::new(crate::pause_menu::Variant::TestDrive,false,false,Default::default())).add_systems(Update,(crate::input::sample,crate::pause_menu::update,crate::drive_actions,crate::drive_tick,update).chain());
            let control = |app: &App| Control::unpack(app.world().resource::<EngineSound>().mailbox.lock().unwrap().latest()[0]);
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
