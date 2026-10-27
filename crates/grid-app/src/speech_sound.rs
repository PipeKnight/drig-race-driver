//! Race engineer speech (docs/SPEECH_AUDIO_RESEARCH.md): the original's event speech for the
//! events of `grid_tools::speech::SLICE` (race start, positions, finish, the player's damage,
//! performance, strategy and rules lines). One voice on the
//! Codriver group plays the lines from the installation; while it plays the mix is in Talking
//! (0x411D40). Presentation only.
use bevy::{
    audio::{AddAudioSource, ChannelCount, SampleRate, Source},
    prelude::*,
};
use grid_tools::{
    audio_mix::{self, Table},
    source_vehicle::DELTA,
    speech::{self, Collector, Entry, Identities, Kind, Line, Manager, RaceView, Rules},
};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    time::Duration,
};

const RATE: u32 = grid_tools::engine_audio::RATE;
/// 0x42FFC0 plays a line with group 14, a PA line with 15 [I: Codriver and 3DSpeech].
const CODRIVER: usize = 14;
const PA: usize = 15;
/// The installation's speech language [AUTHORED: the port has no language option].
const LANGUAGE: &str = "en";
/// The manager's generator seed [AUTHORED: the original's state is not reproducible].
const SEED: u32 = 5489;

enum Command {
    Play(Arc<[i16]>, bool),
    Stop,
}

#[derive(Resource)]
pub(crate) struct SpeechSound {
    root: PathBuf,
    table: Option<Arc<Table>>,
    manager: Manager,
    collector: Collector,
    player_slot: usize,
    /// The race's drivers (docs/SPEECH_AUDIO_RESEARCH.md §15).
    identities: Identities,
    commands: Sender<Command>,
    /// Codriver and 3DSpeech levels in dB.
    group_db: Arc<[AtomicU32; 2]>,
    /// The line playing ends at this vehicle time (s) [I: the voice's state is taken from the
    /// line's length at 32 kHz rather than from the device, so a run without one still advances].
    ends: f64,
    /// Talking was set for the line playing.
    talking: bool,
    last: Option<(u64, u64)>,
    /// Flashbacks used so far in this race.
    flashbacks: u32,
    /// Each car's damage status (vehicle +0xD234) last frame.
    statuses: Vec<u32>,
}

#[derive(Asset, TypePath)]
pub(crate) struct SpeechSource {
    commands: std::sync::Mutex<Option<Receiver<Command>>>,
    group_db: Arc<[AtomicU32; 2]>,
}
pub(crate) struct Decoder {
    commands: Option<Receiver<Command>>,
    group_db: Arc<[AtomicU32; 2]>,
    line: Option<(Arc<[i16]>, bool)>,
    /// Position in the line, in line samples.
    at: f64,
    gain: f32,
    value: f32,
    channel: usize,
    count: usize,
}
impl Decoder {
    fn sample(&mut self) -> f32 {
        let Some((line, _)) = &self.line else { return 0. };
        let i = self.at as usize;
        if i + 1 >= line.len() {
            self.line = None;
            return 0.;
        }
        // 32 kHz to the output rate, linear [I: the backend's resampler is not reconstructed].
        let t = (self.at - i as f64) as f32;
        let value = (f32::from(line[i]) * (1. - t) + f32::from(line[i + 1]) * t) / 32768.;
        self.at += f64::from(speech::LINE_RATE) / f64::from(RATE);
        value * self.gain
    }
}
impl Iterator for Decoder {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if self.channel == 0 {
            if self.count.is_multiple_of(64) {
                if let Some(commands) = &self.commands {
                    while let Ok(command) = commands.try_recv() {
                        match command {
                            Command::Play(line, pa) => {
                                self.line = Some((line, pa));
                                self.at = 0.;
                            }
                            Command::Stop => self.line = None,
                        }
                    }
                }
                let group = self.line.as_ref().map_or(0, |(_, pa)| usize::from(*pa));
                self.gain = audio_mix::backend_gain(0., f32::from_bits(self.group_db[group].load(Ordering::Relaxed)), Some(0.));
            }
            self.count = self.count.wrapping_add(1);
            self.value = self.sample();
        }
        self.channel ^= 1;
        // A 2D voice: centred.
        Some(if self.value.is_finite() { self.value } else { 0. })
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
impl Decodable for SpeechSource {
    type Decoder = Decoder;
    fn decoder(&self) -> Decoder {
        Decoder {
            // One decoder owns the queue; a second decoder plays nothing.
            commands: self.commands.lock().ok().and_then(|mut c| c.take()),
            group_db: self.group_db.clone(),
            line: None,
            at: 0.,
            gain: 0.,
            value: 0.,
            channel: 0,
            count: 0,
        }
    }
}

/// `grid-app --speech-files`: the installation's speech files the ported
/// events can play, relative to the game root, one per line. A run plays
/// only some, so tools/trace_bundle_files.ps1 adds these to the bundle (F608).
#[cfg(not(target_os = "android"))]
pub(crate) fn list_files(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let profile = crate::settings::load(args)?;
    let root = crate::game_root(profile.root.clone())?;
    for line in Rules::load(&root)?.playable_lines() {
        let path = line.path(std::path::Path::new(""), LANGUAGE);
        if root.join(&path).is_file() {
            println!("{}", path.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

/// Loads the rules from `root`; without them the race runs silent of speech.
pub(crate) fn install(
    app: &mut App,
    root: PathBuf,
    table: Option<Arc<Table>>,
    player_slot: usize,
    identities: Identities,
) -> Result<(), String> {
    let rules = Arc::new(Rules::load(&root)?);
    app.add_audio_source::<SpeechSource>();
    let (commands, receiver) = channel();
    let group_db = Arc::new([AtomicU32::new(audio_mix::SILENT.to_bits()), AtomicU32::new(audio_mix::SILENT.to_bits())]);
    let source = SpeechSource {
        commands: std::sync::Mutex::new(Some(receiver)),
        group_db: group_db.clone(),
    };
    let lines = rules.messages.iter().filter(|m| rules.events[m.event].on).count();
    let mut manager = Manager::new(rules, SEED);
    manager.set_ai_names(identities.entries.iter().map(|i| i.name_file.clone()).collect());
    let keys: Vec<String> = identities.entries.iter().map(|i| format!("{}/{}", i.key, i.team)).collect();
    println!(
        "speech_audio version=1 event=drivers keys={} teammate={} teammate_number={}",
        keys.join(","),
        identities.teammate.map_or(-1, |t| t as i64),
        identities.teammate_number
    );
    app.insert_resource(SpeechSound {
        root,
        table,
        manager,
        collector: Collector::default(),
        player_slot,
        identities,
        commands,
        group_db,
        ends: 0.,
        talking: false,
        last: None,
        flashbacks: 0,
        statuses: Vec::new(),
    })
    .insert_resource(PendingSource(Some(source)))
    .add_systems(Startup, start)
    .add_systems(FixedUpdate, tick.after(super::drive_tick));
    println!("speech_audio version=1 event=queued rate={RATE} line_rate={} language={LANGUAGE} events={} messages={lines} group=Codriver", speech::LINE_RATE, speech::SLICE.join(","));
    Ok(())
}

#[derive(Resource)]
struct PendingSource(Option<SpeechSource>);

fn start(mut commands: Commands, mut pending: ResMut<PendingSource>, mut assets: ResMut<Assets<SpeechSource>>) {
    if let Some(source) = pending.0.take() {
        // ONCE: the decoder never ends (F482).
        commands.spawn((AudioPlayer(assets.add(source)), PlaybackSettings::ONCE));
    }
}

/// The race as the collector sees it, from the shared-world standings.
fn view(
    driving: &super::Driving,
    trial: Option<&super::time_trial::Trial>,
    kind: Kind,
    player_slot: usize,
    ids: &Identities,
) -> Option<RaceView> {
    let o = driving.opponents.as_ref()?;
    let standings = &o.standings;
    let clock = trial.and_then(|t| t.race_time(driving.runtime.ticks));
    let speeds = o.speeds(&driving.runtime);
    let order = standings.order();
    let player = &driving.runtime;
    // Car 0 is the player (its own runtime); the opponents are the shared world's cars 1….
    let car = |k: usize| if k == 0 { Some(player) } else { o.multi.cars.get(k) };
    let entries = (0..standings.len())
        .map(|k| {
            let laps = standings.lap_times(k);
            Entry {
                time: standings.finished(k).unwrap_or(clock.unwrap_or(0.)),
                progress: standings.progress(k),
                laps: laps.len() as u32,
                speed: speeds.get(k).copied().unwrap_or(0.) as f32,
                position: order.iter().position(|&c| c == k).map_or(0, |p| p + 1),
                finished: standings.status(k) == grid_tools::race::Status::Finished,
                // The opponents fill the other grid slots in order.
                grid: match k {
                    0 => player_slot,
                    k if k - 1 < player_slot => k - 1,
                    k => k,
                },
                last_lap: laps.last().copied(),
                ai: k != 0,
                terminal: car(k).is_some_and(|c| c.state.car.owner.vehicle[0xd234 / 4] != 7),
                damage: car(k).map_or(0., |c| {
                    let vehicle = &c.state.car.owner.vehicle;
                    if k == 0 { speech::Car::read(vehicle).total() } else { speech::ai_total_percent(vehicle) }
                }),
                key: ids.entries.get(k).map_or(0, |i| i.key),
                team: ids.entries.get(k).map_or(0, |i| i.team),
            }
        })
        .collect();
    // 0x430F50 on every car: a contact whose impulse is at least half the car's physical mass
    // (car +DB0 → +3F0) [I: the port's solved contact impulse].
    let mut contacts = Vec::new();
    for k in 0..o.multi.cars.len() {
        let Some(c) = car(k) else { continue };
        let mass = f32::from_bits(c.state.car.owner.physical[0x3f0 / 4]);
        for (impact, other) in o.multi.contact_impacts(k) {
            if impact.impact >= 0.5 * mass {
                let other = match other {
                    grid_tools::source_vehicle::multi::Contacted::Car(j) => Some(j),
                    _ => None,
                };
                contacts.push(speech::Contact { car: k, other });
            }
        }
    }
    let vehicle = &player.state.car.owner.vehicle;
    let wheels = grid_tools::tyre_audio::wheel_positions(vehicle, &player.state.car.owner.physical);
    Some(RaceView {
        clock: clock.unwrap_or(0.),
        started: clock.is_some(),
        over: standings.over(),
        track_laps: standings.laps(),
        entries,
        player: 0,
        kind,
        contacts,
        car: speech::Car::read(vehicle),
        // The player's driver test of type 11 on the racing line [I: the port's player has no
        // driver object; the AI's test is applied to its wheel anchors].
        off_track: driving.reset_line.as_ref().is_some_and(|line| !line.on_track(&wheels)),
        wrong_way: super::race_hud::wrong_way(driving),
        // 0x9FF4F0 reports a part when its detached body is made: the parts detached this tick.
        parts_lost: player
            .parts()
            .changes
            .iter()
            .filter(|c| c.state == grid_tools::loose_parts::State::Detached)
            .filter_map(|c| speech::part_id(c.part))
            .collect(),
        // [I] Car description byte +0xD4 is not traced to a port field.
        open_cockpit: false,
        status_changes: Vec::new(),
        ai_events: Vec::new(),
        teammate: ids.teammate,
        teammate_number: ids.teammate_number,
    })
}

#[allow(clippy::too_many_arguments)]
fn tick(
    mut sound: ResMut<SpeechSound>,
    driving: Res<super::Driving>,
    listening: super::engine_sound::Listening,
    mode: Option<Res<super::engine_sound::AudioMode>>,
    drift: Option<Res<super::drift_event::DriftEvent>>,
    le_mans: Option<Res<super::le_mans_event::LeMansEvent>>,
    touge: Option<Res<super::touge_event::TougeEvent>>,
    derby: Option<Res<super::demolition_event::DemolitionEvent>>,
    flashback: Option<Res<super::flashback::Flashback>>,
) {
    let sound = &mut *sound;
    let now = (driving.segment, driving.runtime.ticks);
    let time = now.1 as f64 * f64::from(DELTA);
    if sound.last.is_some_and(|(segment, _)| segment != now.0) {
        // Restart Race: 0x452520(1) and 0x444E30(1).
        sound.collector.restart();
        sound.manager.reset_race();
        let _ = sound.commands.send(Command::Stop);
        sound.ends = 0.;
        sound.statuses.clear();
    }
    let busy = time < sound.ends;
    // 427ED0: Talking returns to the remembered mode once the voice has stopped.
    if sound.talking && !busy {
        sound.talking = false;
        if let Some(Ok(mut mix)) = mode.as_ref().map(|m| m.0.lock())
            && mix.release()
        {
            log_line!("audio_mode version=1 event=set mode={} after=speech", audio_mix::MODE_NAMES[mix.mode()]);
        }
    }
    let (camera, modes) = (listening.camera(), listening.modes());
    let trim = listening.speech_trim();
    for (slot, group) in [CODRIVER, PA].into_iter().enumerate() {
        let level = sound.table.as_ref().map_or(-9., |t| modes.group_db(t, camera, group, trim));
        sound.group_db[slot].store(level.to_bits(), Ordering::Relaxed);
    }
    let fresh = sound.last != Some(now) && now.1 != 0 && driving.failure.is_none();
    let since = sound.last.replace(now);
    let root = sound.root.clone();
    let exists = |line: &Line| line.path(&root, LANGUAGE).is_file();
    // A flashback jump-in: 0x452520(0) and `usedFlashback` [I: taken when the flashback manager
    // counts one used].
    let used = flashback.as_ref().map_or(0, |f| f.manager.used);
    if used > sound.flashbacks {
        sound.collector.flashback();
    }
    sound.flashbacks = used;
    let out = if driving.paused {
        // The pause menu: the original's Paused mix clears the queue and stops speech; the
        // collector clears its records once.
        sound.collector.pause();
        sound.manager.frame(0., &speech::Params::default(), busy, true, &exists)
    } else if fresh {
        let format = drift.as_ref().map(|d| d.format);
        let kind = Kind {
            drift: drift.is_some(),
            drift_battle: format == Some(grid_tools::virtual_race::Format::DriftBattle),
            drift_freestyle: format == Some(grid_tools::virtual_race::Format::DriftFreestyle),
            le_mans: le_mans.is_some(),
            touge: touge.is_some(),
            derby: derby.is_some(),
        };
        let Some(mut v) = view(&driving, listening.trial.as_deref(), kind, sound.player_slot, &sound.identities) else { return };
        // The choreographer's messages since the last speech frame (several ticks can pass).
        if let Some(o) = driving.opponents.as_ref() {
            v.ai_events = o
                .ai_messages
                .iter()
                .filter(|&&(segment, t, _)| since.is_none_or(|(s, last)| segment != s || t > last))
                .map(|&(.., m)| m)
                .collect();
        }
        // 0x9E64E0 broadcasts a TerminalDamageMessage when a car's status leaves running (7).
        if let Some(o) = driving.opponents.as_ref() {
            let statuses: Vec<u32> = (0..o.multi.cars.len())
                .map(|k| if k == 0 { &driving.runtime } else { &o.multi.cars[k] })
                .map(|c| c.state.car.owner.vehicle[0xd234 / 4])
                .collect();
            if statuses.len() == sound.statuses.len() {
                v.status_changes = statuses
                    .iter()
                    .zip(&sound.statuses)
                    .enumerate()
                    .filter(|(_, (now, before))| now != before && **now != 7)
                    .map(|(k, (now, _))| (k, *now))
                    .collect();
            }
            sound.statuses = statuses;
        }
        let params = sound.collector.update(&v, DELTA).clone();
        sound.manager.frame(DELTA, &params, busy, false, &exists)
    } else {
        return;
    };
    if out.stop {
        let _ = sound.commands.send(Command::Stop);
        sound.ends = time;
    }
    if let Some(line) = out.play {
        let path = line.path(&root, LANGUAGE);
        match std::fs::read(&path) {
            Ok(bytes) => {
                let samples: Arc<[i16]> = bytes.as_chunks::<2>().0.iter().map(|b| i16::from_le_bytes(*b)).collect();
                sound.ends = time + samples.len() as f64 / f64::from(speech::LINE_RATE);
                let _ = sound.commands.send(Command::Play(samples, line.pa));
                log_line!("speech_audio version=1 event=play message={} file={}/{} tick={}", line.message, line.folder, line.file, now.1);
                // 0x42FFC0 → 0x411D40: Talking while the line plays, not for PA lines.
                if !line.pa
                    && let Some(Ok(mut mix)) = mode.as_ref().map(|m| m.0.lock())
                    && mix.set_tied(audio_mix::TALKING)
                {
                    sound.talking = true;
                    log_line!("audio_mode version=1 event=set mode=Talking");
                }
            }
            Err(error) => warn!("speech line {}: {error}", path.display()),
        }
    }
}
