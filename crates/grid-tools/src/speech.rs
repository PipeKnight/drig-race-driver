//! Race engineer speech, part 1 (docs/SPEECH_AUDIO_RESEARCH.md). This ports:
//! - the original's data-driven rules (`audio/eventSpeech.bin`, parser 0x462FF0) over its line
//!   bank (`audio/audioEventSpeech.bin`);
//! - the evaluation, queue and playback rules of `audio_event_speech_manager` (0x45EC00,
//!   0x44BE90, 0x45E870);
//! - the parameters of `audio_game_info_collector` (0x452810) that the Position and EndOfRace
//!   events read.
//!
//! Only those two events run ([`SLICE`], the AUTHORED scope of this part). Lines are loaded
//! from the owner's installation at run time.
use crate::series::Mt;
use grid_formats::binxml;
use std::{collections::VecDeque, path::Path};

/// The `.raw` lines are PCM16 mono at 32 kHz (stream setup 0x42BD4B: 32000, AL_FORMAT_MONO16).
pub const LINE_RATE: u32 = 32_000;
/// Events evaluated in this part [AUTHORED scope: race start, position calls, finish].
pub const SLICE: [&str; 2] = ["Position", "EndOfRace"];
/// An absent condition value or second parameter (0xC61C3C00).
const UNSET: f32 = -10000.;
/// The silence base after a line starts: now + 2.5 s (0xCB092C, 0x42FFC0).
const AFTER_LINE: f32 = 2.5;
/// A zero race progress divides by this instead (RAY_TOLERANCE 0xCB22E4, 0x438C10).
const TINY_PROGRESS: f64 = 1e-6;
/// m/s to mph, as the record's speed (0x438C10).
const MPH: f32 = 2.236_936_3;

macro_rules! params {
    ($($id:ident = $name:literal),* $(,)?) => {
        /// The collector parameters the Position and EndOfRace rules read (names registered by
        /// 0x45F590). `InRace` comes first: 0x44C510 resolves an unknown name to index 0.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Param { $($id),* }
        pub const PARAM_NAMES: &[&str] = &[$($name),*];
    };
}
params!(
    InRace = "inRace",
    InIntroRace = "inIntroRace",
    EndRace = "endRace",
    OnStartingGrid = "onStartingGrid",
    IsDestructionDerby = "isDestructionDerby",
    PlayerPos = "playerPos",
    PlayerTime = "playerTime",
    PlayerFinished = "playerFinished",
    LeadingCarKeyDriver = "leadingCarKeyDriver",
    LeadingCarTeam = "leadingCarTeam",
    PlayerStartPosPercentage = "playerStartPosPercentage",
    RaceWinnerTeam = "raceWinnerTeam",
    TotalDam = "totalDam",
    TeammateNumber = "teammateNumber",
    WonChampionship = "wonChampionship",
    PlayerLap = "playerLap",
    IsFinalRace = "isFinalRace",
    IsFinalCareerEvent = "isFinalCareerEvent",
    PlayerCollision = "playerCollision",
    CarPosPlus1Time = "carPosPlus1Time",
    CarPosMinus1Time = "carPosMinus1Time",
    CarAheadLap = "carAheadLap",
    CarBehindLap = "carBehindLap",
    RaceTime = "raceTime",
    IsNormalRace = "isNormalRace",
    PlayerHasTeammate = "playerHasTeammate",
    PlTimeInCurrentPos = "plTimeInCurrentPos",
    LeadTimeInCurrentPos = "leadTimeInCurrentPos",
    Plus1TimeInCurrentPos = "plus1TimeInCurrentPos",
    Minus1TimeInCurrentPos = "minus1TimeInCurrentPos",
    CarBehindDistance = "carBehindDistance",
    CarAheadDistance = "carAheadDistance",
    PlayerCarSpeed = "playerCarSpeed",
    NoCollisionTime = "noCollisionTime",
    PlayerMovedTrackPos = "playerMovedTrackPos",
    CarAheadTerminal = "carAheadTerminal",
    CarBehindTerminal = "carBehindTerminal",
    CarAheadSpeed = "carAheadSpeed",
    CarBehindSpeed = "carBehindSpeed",
    PlayerLapDist = "playerLapDist",
    CarPosMinus1KeyDriver = "carPosMinus1KeyDriver",
    NumCarsInRace = "numCarsInRace",
    CoolFinish = "coolFinish",
    NumRaceWins = "numRaceWins",
    NumEventWins = "numEventWins",
    RacesCurTrack = "racesCurTrack",
    StartAfterRestart = "startAfterRestart",
    IsNetRace = "isNetRace",
    IsTestDrive = "isTestDrive",
    IsHeadToHead = "isHeadToHead",
    FirstCarFinished = "firstCarFinished",
    PlayerPrevPosition = "playerPrevPosition",
    TeamToBeatNum = "teamToBeatNum",
    PlBestPosInRace = "plBestPosInRace",
    IsTouge = "isTouge",
    IsLeMans = "isLeMans",
    IsDriftBattle = "isDriftBattle",
);
pub const PARAMS: usize = PARAM_NAMES.len();

fn param_index(name: &str) -> usize {
    PARAM_NAMES.iter().position(|n| *n == name).unwrap_or(0)
}

/// The parameter values one evaluation reads.
#[derive(Clone, Debug, PartialEq)]
pub struct Params(pub [f32; PARAMS]);
impl Default for Params {
    /// Every value 0, except `numRaceWins` = 1. The port keeps no profile statistics, and with 0
    /// `firstWinEver` would play at every win [AUTHORED].
    fn default() -> Self {
        let mut p = Self([0.; PARAMS]);
        p.set(Param::NumRaceWins, 1.);
        p
    }
}
impl Params {
    pub fn get(&self, p: Param) -> f32 {
        self.0[p as usize]
    }
    pub fn set(&mut self, p: Param, v: f32) {
        self.0[p as usize] = v;
    }
    /// The getters' booleans: 1.0 or 0.0.
    pub fn flag(&mut self, p: Param, v: bool) {
        self.set(p, if v { 1. } else { 0. });
    }
}

// ---- the files ---------------------------------------------------------------------------------

fn attr<'a>(e: &binxml::Element<'a>, name: &str) -> Option<&'a str> {
    e.attributes.iter().find(|(k, _)| *k == name).map(|(_, v)| *v)
}
/// 0xA67B30-style integer: a leading integer, else 0 [I: atoi; `priority=""` is 0].
fn int(e: &binxml::Element, name: &str) -> u32 {
    let v = attr(e, name).unwrap_or("").trim();
    let end = v.find(|c: char| !c.is_ascii_digit()).unwrap_or(v.len());
    v[..end].parse().unwrap_or(0)
}
fn float(e: &binxml::Element, name: &str) -> f32 {
    attr(e, name).and_then(|v| v.trim().parse().ok()).unwrap_or(0.)
}
fn boolean(e: &binxml::Element, name: &str) -> bool {
    attr(e, name).is_some_and(|v| v.trim().eq_ignore_ascii_case("true") || v.trim() == "1")
}
fn children<'d, 'a>(doc: &'d binxml::Document<'a>, i: usize, name: &'d str) -> impl Iterator<Item = usize> + 'd {
    doc.elements[i].children.clone().filter(move |&c| doc.elements[c].name == name)
}
fn root(doc: &binxml::Document, name: &str) -> Result<usize, String> {
    doc.elements
        .iter()
        .position(|e| e.parent.is_none() && e.name == name)
        .ok_or(format!("speech: root `{name}` absent"))
}

/// One speech of the bank: its folder, the long version and the variations.
#[derive(Clone, Debug, PartialEq)]
pub struct Speech {
    pub name: String,
    pub folder: String,
    pub long: Option<String>,
    pub variations: Vec<String>,
    /// `isPA`: played as a public-address line (group 15) without the Talking mix.
    pub pa: bool,
}

/// `audio/audioEventSpeech.bin` (0x45C5C0).
#[derive(Clone, Debug, Default)]
pub struct Bank {
    pub speeches: Vec<Speech>,
}
impl Bank {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let doc = binxml::read(bytes).map_err(|e| format!("audioEventSpeech: {e:?}"))?;
        let top = root(&doc, "Speeches")?;
        let mut speeches = Vec::new();
        for c in children(&doc, top, "Category") {
            let folder = attr(&doc.elements[c], "path").unwrap_or("").to_string();
            for s in children(&doc, c, "Speech") {
                let e = &doc.elements[s];
                let variations = children(&doc, s, "Variation")
                    .filter_map(|v| attr(&doc.elements[v], "filename").map(str::to_string))
                    .collect();
                speeches.push(Speech {
                    name: attr(e, "name").unwrap_or("").to_string(),
                    folder: folder.clone(),
                    long: attr(e, "long").filter(|l| !l.is_empty()).map(str::to_string),
                    variations,
                    pa: boolean(e, "isPA"),
                });
            }
        }
        Ok(Self { speeches })
    }
    fn find(&self, name: &str) -> Option<usize> {
        self.speeches.iter().position(|s| s.name == name)
    }
}

/// Root attributes of `EventSpeech`.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub minimum_silence: f32,
    pub max_in_queue: u32,
    pub reduction: u32,
    pub delay: f32,
    pub previous_queue: u32,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Condition {
    param: usize,
    op: u8,
    second: usize,
    value: f32,
}

/// 0x417090: operator names in table order (0xE5A3B0); an unknown name is 19, which no
/// comparison handles (always false here) [I: the original indexes past the table].
const OPERATORS: [&str; 19] = [
    "moreThan",
    "lessThan",
    "equals",
    "notEquals",
    "lessThanOrEqual",
    "moreThanOrEqual",
    "moreThanParamPlusValue",
    "lessThanParamMinusValue",
    "moreThanParamMinusValue",
    "lessThanParamPlusValue",
    "moreThanParamDivValue",
    "equalsParamPlusValue",
    "equalsParamMinusValue",
    "equalsWith",
    "notEqualsWith",
    "moreThanParam",
    "lessThanParam",
    "moreThanOrEqualParam",
    "lessThanOrEqualParam",
];

/// The comparisons 0x417240–0x4174A0: `a` the parameter, `v` the value, `c` the second
/// parameter. x87/SSE compares: NaN makes an ordered compare false and `notEquals` true.
fn compare(op: u8, a: f32, v: f32, c: f32) -> bool {
    match op {
        0 => a > v,
        1 => a < v,
        2 => a == v,
        3 => a != v,
        4 => a <= v,
        5 => a >= v,
        6 => a >= c + v,
        7 => a <= c - v,
        8 => a >= c - v,
        9 => a <= c + v,
        10 => a >= c / v,
        11 => a == c + v,
        12 => a == c - v,
        13 => a == c,
        14 => a != c,
        15 => a > c,
        16 => a < c,
        17 => a >= c,
        18 => a <= c,
        _ => false,
    }
}

fn conditions(doc: &binxml::Document, parent: usize) -> Vec<Condition> {
    children(doc, parent, "Condition")
        .map(|i| {
            let e = &doc.elements[i];
            let op = OPERATORS
                .iter()
                .position(|o| Some(*o) == attr(e, "operator"))
                .unwrap_or(OPERATORS.len()) as u8;
            Condition {
                param: param_index(attr(e, "param").unwrap_or("")),
                op,
                second: attr(e, "secondParam").map_or(0, param_index),
                value: attr(e, "value").map_or(UNSET, |_| float(e, "value")),
            }
        })
        .collect()
}
fn holds(list: &[Condition], p: &Params) -> bool {
    list.iter().all(|c| {
        let second = if c.op < 6 { UNSET } else { p.0[c.second] };
        compare(c.op, p.0[c.param], c.value, second)
    })
}

#[derive(Clone, Debug)]
pub struct Event {
    pub kind: String,
    pub priority: u32,
    pub trigger: bool,
    pub external: bool,
    pub delay: f32,
    conditions: Vec<Condition>,
    pub groups: Vec<usize>,
    /// In [`SLICE`].
    pub on: bool,
}
#[derive(Clone, Debug)]
pub struct Group {
    pub name: String,
    pub event: usize,
    pub delay: f32,
    conditions: Vec<Condition>,
    pub messages: Vec<usize>,
}
#[derive(Clone, Debug)]
pub struct Message {
    pub name: String,
    pub event: usize,
    pub group: usize,
    pub speech: Option<usize>,
    pub priority: u32,
    pub queue_value: f32,
    pub repetition: f32,
    pub max_total: u16,
    pub max_race: u16,
    pub max_event: u16,
    /// neutral 0, positive 1, negative 2, namedAI 3, none 4.
    pub name_to_say: u32,
    pub linked: u32,
    pub delay: f32,
    pub exclusive: bool,
    pub is_event: bool,
    pub keep: f32,
    pub ignore_queue: bool,
    pub ignore_silence: bool,
    pub is_final: bool,
    conditions: Vec<Condition>,
}

/// `audio/eventSpeech.bin` (0x462FF0) resolved against the bank.
#[derive(Clone, Debug)]
pub struct Rules {
    pub settings: Settings,
    pub events: Vec<Event>,
    pub groups: Vec<Group>,
    pub messages: Vec<Message>,
    pub bank: Bank,
}
impl Rules {
    /// Both files from the installation.
    pub fn load(root: &Path) -> Result<Self, String> {
        let read = |name: &str| {
            let path = root.join("audio").join(name);
            std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))
        };
        Self::parse(&read("eventSpeech.bin")?, Bank::parse(&read("audioEventSpeech.bin")?)?)
    }
    pub fn parse(bytes: &[u8], bank: Bank) -> Result<Self, String> {
        let doc = binxml::read(bytes).map_err(|e| format!("eventSpeech: {e:?}"))?;
        let top = root(&doc, "Speeches")?;
        let main = children(&doc, top, "EventSpeech").next().ok_or("eventSpeech: EventSpeech absent")?;
        let m = &doc.elements[main];
        let settings = Settings {
            minimum_silence: float(m, "minimumSilence"),
            max_in_queue: int(m, "maxMsgInQueue"),
            reduction: int(m, "reductionPercentage"),
            delay: float(m, "delay"),
            previous_queue: int(m, "previousQueue").max(1),
            // Absent: the constructor's default [I: enabled].
            enabled: attr(m, "enabled").is_none() || boolean(m, "enabled"),
        };
        let (mut events, mut groups, mut messages) = (Vec::new(), Vec::new(), Vec::new());
        let first = |i: usize, name: &str| children(&doc, i, name).next();
        for ei in children(&doc, main, "Event") {
            let e = &doc.elements[ei];
            let event = events.len();
            let kind = attr(e, "type").unwrap_or("").to_string();
            let mut group_list = Vec::new();
            for gi in children(&doc, ei, "Group") {
                let g = &doc.elements[gi];
                let group = groups.len();
                let mut message_list = Vec::new();
                for mi in children(&doc, gi, "Message") {
                    let x = &doc.elements[mi];
                    let speech = attr(x, "speech").and_then(|s| bank.find(s));
                    // nameToSay and its percentage are read together (0x46370x).
                    let (name_to_say, linked) = match attr(x, "nameToSay") {
                        Some(n) => (
                            ["neutral", "positive", "negative", "namedAI"]
                                .iter()
                                .position(|k| *k == n)
                                .unwrap_or(4) as u32,
                            int(x, "linkedPhrasePercentage"),
                        ),
                        None => (4, 0),
                    };
                    let keep = float(x, "keepInQueueTime");
                    let mut queue_value = float(x, "queueValue");
                    if queue_value < keep {
                        queue_value = keep + 1.;
                    }
                    message_list.push(messages.len());
                    messages.push(Message {
                        name: attr(x, "msg").unwrap_or("").to_string(),
                        event,
                        group,
                        speech,
                        priority: int(x, "priority"),
                        queue_value,
                        repetition: float(x, "repetitionThreshold"),
                        max_total: int(x, "maxPlayCount") as u16,
                        max_race: int(x, "maxPlayCountInRace") as u16,
                        max_event: int(x, "maxPlayCountInEvent") as u16,
                        name_to_say,
                        linked,
                        delay: float(x, "delay"),
                        exclusive: boolean(x, "exclusive"),
                        is_event: boolean(x, "isEvent"),
                        keep,
                        ignore_queue: boolean(x, "ignoreQueue"),
                        ignore_silence: boolean(x, "ignoreMinimumSilence"),
                        is_final: boolean(x, "isFinalMessage"),
                        conditions: conditions(&doc, mi),
                    });
                }
                groups.push(Group {
                    name: attr(g, "name").unwrap_or("").to_string(),
                    event,
                    delay: float(g, "delay"),
                    conditions: first(gi, "Conditions").map_or_else(Vec::new, |c| conditions(&doc, c)),
                    messages: message_list,
                });
                group_list.push(group);
            }
            events.push(Event {
                on: SLICE.contains(&kind.as_str()),
                kind,
                priority: int(e, "priority"),
                trigger: boolean(e, "isEventTrigger"),
                external: boolean(e, "isExternalTriggered"),
                delay: float(e, "delay"),
                conditions: first(ei, "Conditions").map_or_else(Vec::new, |c| conditions(&doc, c)),
                groups: group_list,
            });
        }
        Ok(Self {
            settings,
            events,
            groups,
            messages,
            bank,
        })
    }
    /// Index of the message named `msg`.
    pub fn message(&self, msg: &str) -> Option<usize> {
        self.messages.iter().position(|m| m.name == msg)
    }
    /// Names of the parameters the [`SLICE`] events read that this port does not register
    /// (they would resolve to `inRace`, as an unknown name does in the original).
    pub fn unknown_params(&self, bytes: &[u8]) -> Vec<String> {
        let Ok(doc) = binxml::read(bytes) else { return vec!["unreadable".into()] };
        let mut unknown: Vec<String> = Vec::new();
        for (i, e) in doc.elements.iter().enumerate() {
            if e.name != "Condition" {
                continue;
            }
            // The enclosing Event.
            let mut at = Some(i);
            while let Some(k) = at {
                if doc.elements[k].name == "Event" {
                    break;
                }
                at = doc.elements[k].parent;
            }
            let Some(event) = at else { continue };
            if !SLICE.contains(&attr(&doc.elements[event], "type").unwrap_or("")) {
                continue;
            }
            for name in [attr(e, "param"), attr(e, "secondParam")].into_iter().flatten() {
                if !PARAM_NAMES.contains(&name) && !unknown.iter().any(|u| u == name) {
                    unknown.push(name.to_string());
                }
            }
        }
        unknown
    }
}

// ---- the manager -------------------------------------------------------------------------------

/// A message's run-time fields.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct State {
    /// +0x5A, +0x5C, +0x62.
    total: u16,
    race: u16,
    event: u16,
    /// +0x50, +0x4C.
    last_played: f32,
    queued_at: f32,
    /// +0x48: the long version was played (set at load when the speech has none).
    long_played: bool,
}

/// One line to play: `speech/<lang>/<folder>/<file>.raw`.
#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub message: String,
    pub folder: String,
    pub file: String,
    pub pa: bool,
}
impl Line {
    pub fn path(&self, root: &Path, language: &str) -> std::path::PathBuf {
        root.join("audio/speech")
            .join(language)
            .join(&self.folder)
            .join(format!("{}.raw", self.file))
    }
}

/// What one frame asks of the speech voice.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Output {
    /// Stop the playing line (a priority-0 commit, 0x45C510 → 0xA23350; a pause, 0x430350).
    pub stop: bool,
    /// Start this line on the voice (0x42FFC0) and switch the mix to Talking unless `pa`.
    pub play: Option<Line>,
}

/// `audio_event_speech_manager` for one race world.
pub struct Manager {
    rules: std::sync::Arc<Rules>,
    rng: Mt,
    /// +0x600 clock and +0x604 silence base.
    now: f32,
    silence: f32,
    /// +0xC0 queue and the ring of the last `previousQueue` played messages (front = newest).
    queue: Vec<usize>,
    ring: VecDeque<usize>,
    /// +0x62C last played, +0x634 loading, +0x60B loaded, +0x60C pending playback.
    last: Option<usize>,
    current: Option<usize>,
    loaded: bool,
    pending: bool,
    /// The line stream's file (+0x12C) and the message it was loaded for.
    stream: Option<Line>,
    state: Vec<State>,
}

impl Manager {
    /// A new race world (0x466BA0): every count 0. `seed`: the generator is AUTHORED, since the
    /// original's generator state (audio system +0x4F80) is not reproducible.
    pub fn new(rules: std::sync::Arc<Rules>, seed: u32) -> Self {
        let state = rules
            .messages
            .iter()
            .map(|m| State {
                long_played: m.speech.is_none_or(|s| rules.bank.speeches[s].long.is_none()),
                ..State::default()
            })
            .collect();
        Self {
            rules,
            rng: Mt::new(seed),
            now: 0.,
            silence: 0.,
            queue: Vec::new(),
            ring: VecDeque::new(),
            last: None,
            current: None,
            loaded: false,
            pending: false,
            stream: None,
            state,
        }
    }
    pub fn rules(&self) -> &Rules {
        &self.rules
    }
    /// Names of the queued messages, front first (diagnostics).
    pub fn queued(&self) -> Vec<&str> {
        self.queue.iter().map(|&m| self.rules.messages[m].name.as_str()).collect()
    }

    /// The race (re)start reset, 0x444E30(1) and 0x438840:
    /// - queues, ring, last, loading, pending, the silence base and the stream name are cleared;
    /// - each message's race count and last-played time are reset;
    /// - a speech with a long version plays it again.
    pub fn reset_race(&mut self) {
        self.queue.clear();
        self.ring.clear();
        self.last = None;
        self.current = None;
        self.loaded = false;
        self.pending = false;
        self.silence = 0.;
        self.stream = None;
        for (s, m) in self.state.iter_mut().zip(&self.rules.messages) {
            s.race = 0;
            s.last_played = 0.;
            if m.speech.is_some_and(|k| self.rules.bank.speeches[k].long.is_some()) {
                s.long_played = false;
            }
        }
    }

    /// One audio frame (0x45E870 with the task 0x45F050 before it).
    /// - `voice_busy`: the speech voice is playing (+0x120).
    /// - `paused`: the mix is in the Paused mode; this clears the queue and stops speech.
    /// - `exists`: whether a line file is present; a missing file is a load error (+0x1D5).
    pub fn frame(&mut self, dt: f32, p: &Params, voice_busy: bool, paused: bool, exists: &dyn Fn(&Line) -> bool) -> Output {
        let mut out = Output::default();
        if paused {
            self.queue.clear();
            out.stop = voice_busy;
            self.stop();
            return out;
        }
        self.now += dt;
        self.evaluate(p);
        self.expire();
        if self.rules.settings.enabled {
            self.queue_update(p, voice_busy, &mut out);
        }
        match self.current {
            Some(c) => self.loading(c, p, voice_busy, exists, &mut out),
            None => self.playback(voice_busy, &mut out),
        }
        out
    }

    fn msg(&self, m: usize) -> &Message {
        &self.rules.messages[m]
    }
    fn conditions_hold(&self, m: usize, p: &Params) -> bool {
        let msg = self.msg(m);
        holds(&self.rules.events[msg.event].conditions, p)
            && holds(&self.rules.groups[msg.group].conditions, p)
            && holds(&msg.conditions, p)
    }
    fn remove(&mut self, m: usize) {
        if let Some(i) = self.queue.iter().position(|&q| q == m) {
            self.queue.remove(i);
        }
    }
    /// 0x430350: forget the line being loaded.
    fn stop(&mut self) {
        self.loaded = false;
        self.current = None;
    }

    /// 0x417000.
    fn percent(&self, m: usize) -> i32 {
        let priority = self.msg(m).priority;
        if priority < 2 {
            return 100;
        }
        let v = 100i32.wrapping_sub(
            (u32::from(self.state[m].total))
                .wrapping_mul(self.rules.settings.reduction)
                .wrapping_mul(priority) as i32,
        );
        let floor = (100. / priority as f32) as i32;
        if v <= floor { floor } else { v }
    }

    /// 0x438740: an exclusive message of `group` is loading or among the recently played.
    fn group_exclusive_busy(&self, group: usize) -> bool {
        let exclusive = |m: usize| self.msg(m).exclusive && self.msg(m).group == group;
        self.current.is_some_and(exclusive) || self.ring.iter().any(|&m| exclusive(m))
    }

    /// 0x45EC00.
    fn evaluate(&mut self, p: &Params) {
        let rules = self.rules.clone();
        for event in rules.events.iter().filter(|e| e.on) {
            if event.external || !holds(&event.conditions, p) {
                continue;
            }
            for &g in &event.groups {
                let group = &rules.groups[g];
                if group.messages.is_empty() || !holds(&group.conditions, p) {
                    continue;
                }
                for &m in &group.messages {
                    // isEventTrigger events feed the external queue (+0x110), not part of this slice.
                    if event.trigger || !rules.settings.enabled {
                        continue;
                    }
                    let msg = &rules.messages[m];
                    let s = self.state[m];
                    let fresh = msg.priority == 0
                        || match self.last {
                            None => true,
                            Some(last) => {
                                m != last
                                    && (s.race == 0
                                        || msg.repetition <= 0.
                                        || msg.repetition <= self.now - s.last_played)
                                    && (msg.ignore_queue || !self.ring.contains(&m))
                                    && !(rules.messages[last].exclusive && rules.messages[last].group == msg.group)
                                    && !self.group_exclusive_busy(msg.group)
                            }
                        };
                    let under = |count: u16, max: u16| max == 0 || count < max;
                    if !fresh
                        || !under(s.total, msg.max_total)
                        || !under(s.event, msg.max_event)
                        || !under(s.race, msg.max_race)
                    {
                        continue;
                    }
                    let roll = self.rng.range(0, 100) as i32;
                    if self.percent(m) < roll || self.queue.contains(&m) || !holds(&msg.conditions, p) {
                        continue;
                    }
                    self.insert(m);
                    self.state[m].queued_at = self.now;
                    if self.queue.len() as u32 >= rules.settings.max_in_queue {
                        return;
                    }
                }
            }
        }
    }

    /// 0x45EAE0: priority 0 first; otherwise by event priority, then message priority.
    fn insert(&mut self, m: usize) {
        if self.queue.is_empty() {
            self.queue.push(m);
            return;
        }
        if self.queue.contains(&m) {
            return;
        }
        let msg = self.msg(m);
        if msg.priority == 0 {
            self.queue.insert(0, m);
            return;
        }
        let event = |q: usize| self.rules.events[self.msg(q).event].priority;
        let ours = event(m);
        for i in 0..self.queue.len() {
            let theirs = event(self.queue[i]);
            if ours == theirs {
                // The scan runs to the end of the queue, not only over this event's entries.
                if let Some(j) = (i..self.queue.len()).find(|&j| msg.priority < self.msg(self.queue[j]).priority) {
                    self.queue.insert(j, m);
                    return;
                }
            } else if ours < theirs {
                self.queue.insert(i, m);
                return;
            }
        }
        self.queue.push(m);
    }

    /// 0x444FE0.
    fn expire(&mut self) {
        let now = self.now;
        let rules = self.rules.clone();
        let state = &self.state;
        self.queue.retain(|&m| {
            let msg = &rules.messages[m];
            now - state[m].queued_at <= msg.queue_value.max(msg.keep)
        });
    }

    /// 0x44BE90.
    fn queue_update(&mut self, p: &Params, voice_busy: bool, out: &mut Output) {
        if self.last.is_some_and(|l| self.msg(l).is_final) {
            return;
        }
        let Some(&f) = self.queue.first() else { return };
        let (fp, fe) = (self.msg(f).priority, self.rules.events[self.msg(f).event].priority);
        let mut normal = false;
        if let Some(c) = self.current
            && self.loaded
            && c != f
            && self.msg(c).priority != 0
            && self.last.is_some_and(|l| l != f)
        {
            let ce = self.rules.events[self.msg(c).event].priority;
            if fe > ce && fp != 0 {
                normal = true;
            } else if fp < self.msg(c).priority {
                self.stop();
                self.remove(c);
                return;
            }
        }
        let urgent = !normal
            && fp == 0
            && self.current.is_some_and(|c| c != f)
            && self.last.is_some_and(|l| l != f);
        if !urgent {
            if self.pending || self.current.is_some() {
                return;
            }
            let fm = self.msg(f);
            if !(fm.ignore_silence || fm.is_final) && self.now <= self.silence + self.rules.settings.minimum_silence {
                return;
            }
            if self.now <= fm.keep + self.state[f].queued_at {
                return;
            }
        }
        match self.try_start(f, p, voice_busy, out) {
            Start::Loaded => {
                if self.msg(f).exclusive {
                    // 0x445070: the other queued messages of the group go.
                    let group = self.msg(f).group;
                    let rules = self.rules.clone();
                    self.queue.retain(|&q| q == f || rules.messages[q].group != group);
                }
            }
            Start::Drop => self.remove(f),
            Start::Wait => {}
        }
    }

    /// 0x445170 and the load 0x437F70.
    fn try_start(&mut self, f: usize, p: &Params, voice_busy: bool, out: &mut Output) -> Start {
        let msg = self.msg(f).clone();
        if voice_busy && msg.priority != 0 {
            return Start::Wait;
        }
        if !msg.is_event && !self.conditions_hold(f, p) {
            self.state[f].queued_at = 0.;
            return Start::Drop;
        }
        self.loaded = false;
        let Some(s) = msg.speech else {
            self.current = Some(f);
            self.loaded = true;
            return Start::Loaded;
        };
        let speech = &self.rules.bank.speeches[s];
        let file = if !self.state[f].long_played {
            speech.long.clone()
        } else if speech.variations.is_empty() {
            // "Played the long version, trying to play again without having extra variations".
            None
        } else {
            let n = speech.variations.len() as u32;
            Some(speech.variations[int_between(&mut self.rng, 0, n) as usize].clone())
        };
        let Some(file) = file else { return Start::Drop };
        let line = Line {
            message: msg.name.clone(),
            folder: speech.folder.clone(),
            file,
            pa: speech.pa,
        };
        // The same file as the line stream's: not played twice in a row.
        if self.stream.as_ref().is_some_and(|l| l.folder == line.folder && l.file == line.file) {
            return Start::Drop;
        }
        // Loading replaces the stream: a playing line (priority 0 only) is cut.
        out.stop |= voice_busy;
        self.stream = Some(line);
        self.current = Some(f);
        self.loaded = true;
        // The player-name phrase roll; the port has no profile names, so nothing is armed.
        let _ = int_between(&mut self.rng, 0, 100) <= msg.linked;
        Start::Loaded
    }

    /// 0x45E870 while a message is loading: commit it once ready, or drop it.
    fn loading(&mut self, c: usize, p: &Params, voice_busy: bool, exists: &dyn Fn(&Line) -> bool, out: &mut Output) {
        let msg = self.msg(c).clone();
        let valid = msg.is_event || self.conditions_hold(c, p);
        if msg.speech.is_none() {
            if valid {
                self.commit(c, voice_busy, out);
            }
            self.current = None;
            self.loaded = false;
            self.pending = false;
            return;
        }
        // The stream is ready at once [I: the original loads asynchronously]; a missing file is
        // a load error.
        let ready = self.stream.as_ref().is_some_and(exists);
        if ready && valid {
            self.commit(c, voice_busy, out);
            self.current = None;
            self.loaded = false;
        } else {
            self.stop();
            self.stream = None;
            self.remove(c);
        }
    }

    /// 0x45C510.
    fn commit(&mut self, c: usize, voice_busy: bool, out: &mut Output) {
        if self.msg(c).priority == 0 && voice_busy {
            out.stop = true;
        }
        if self.ring.len() as u32 >= self.rules.settings.previous_queue {
            self.ring.pop_back();
        }
        self.ring.push_front(c);
        self.last = Some(c);
        self.pending = true;
        let now = self.now;
        let s = &mut self.state[c];
        s.long_played = true;
        s.total = s.total.wrapping_add(1);
        s.race = s.race.wrapping_add(1);
        s.event = s.event.wrapping_add(1);
        s.last_played = now;
        self.remove(c);
    }

    /// 0x45E870 → 0x42FFC0: the committed line plays once its delays have passed.
    fn playback(&mut self, voice_busy: bool, out: &mut Output) {
        let Some(l) = self.last else { return };
        if !self.pending || voice_busy {
            return;
        }
        let msg = self.msg(l);
        let wait = self.rules.events[msg.event].delay
            + self.rules.groups[msg.group].delay
            + msg.delay
            + self.state[l].queued_at
            + self.rules.settings.delay;
        if wait >= self.now {
            return;
        }
        out.play = self.stream.clone();
        self.pending = false;
        self.silence = self.now + AFTER_LINE;
    }
}

/// 0xA68410 int(lo, hi): `hi` when the bounds meet, else `lo + genrand() % (hi - lo)`.
fn int_between(rng: &mut Mt, lo: u32, hi: u32) -> u32 {
    if lo == hi { hi } else { rng.range(lo, hi) }
}

enum Start {
    Wait,
    Loaded,
    Drop,
}

// ---- the parameters ----------------------------------------------------------------------------

/// One race entry as the collector sees it (race entry getters, docs/RACE_RULES_RESEARCH.md §1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Entry {
    /// Race time (s): the race clock while racing, the finish time once finished (vf 12).
    pub time: f64,
    /// Race progress in laps (vf 18).
    pub progress: f64,
    /// Completed laps (vf 5).
    pub laps: u32,
    /// Speed (m/s).
    pub speed: f32,
    /// One-based position (vf 2 + 1).
    pub position: usize,
    pub finished: bool,
    /// Zero-based grid slot (car +0x2E4C).
    pub grid: usize,
}

/// The race kind flags of the race block (0x451A20).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Kind {
    pub drift: bool,
    pub drift_battle: bool,
    pub le_mans: bool,
    pub touge: bool,
    pub derby: bool,
}

/// What the app knows about the race this frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RaceView {
    /// Race clock (s, from the green light).
    pub clock: f64,
    /// The race has started (manager +0x10, set at the green light).
    pub started: bool,
    /// Race over (manager +0x11).
    pub over: bool,
    pub entries: Vec<Entry>,
    pub player: usize,
    pub kind: Kind,
    /// The player collided this frame (impulse at least half the mass, 0x430F50).
    pub collision: bool,
    /// The player's total damage percentage now ([`total_damage_percent`]).
    pub damage: f32,
}

/// A car record (0x438C10) with the collector's time in position.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Record {
    time: f32,
    progress: f32,
    speed: f32,
    fraction: f32,
    lap: f32,
    position: f32,
    previous: f32,
    finished: bool,
    grid: u32,
    in_position: f32,
}
impl Record {
    /// 0x438C10. The previous position changes only when the position does.
    fn refresh(&mut self, e: &Entry) {
        let progress = if e.progress == 0. { TINY_PROGRESS } else { e.progress };
        self.time = (e.time / progress) as f32;
        self.progress = e.progress as f32;
        self.speed = e.speed * MPH;
        self.fraction = e.progress.rem_euclid(1.) as f32;
        self.lap = (e.laps + 1) as f32;
        let position = e.position as f32;
        if self.position != position {
            self.previous = self.position;
        }
        self.position = position;
        self.finished = e.finished;
        self.grid = e.grid as u32;
    }
}

/// `audio_game_info_collector` (0x452810) for the parameters of [`Param`].
#[derive(Clone, Debug)]
pub struct Collector {
    /// +0xB410 frame counter.
    counter: u32,
    in_race: bool,
    end_race: bool,
    on_grid: bool,
    restarted: bool,
    clock: f32,
    cars: u32,
    kind: Kind,
    player: Record,
    /// Plus 1, minus 1 and the leader: (car + 1 or 0, previous frame's car + 1, record).
    plus1: (usize, usize, Record),
    minus1: (usize, usize, Record),
    leader: (usize, usize, Record),
    best: u32,
    start_percent: f32,
    collision: bool,
    no_collision: f32,
    total_damage: f32,
    params: Params,
}
impl Default for Collector {
    fn default() -> Self {
        Self {
            counter: 0,
            in_race: false,
            end_race: false,
            on_grid: false,
            restarted: false,
            clock: 0.,
            cars: 0,
            kind: Kind::default(),
            player: Record::default(),
            plus1: Default::default(),
            minus1: Default::default(),
            leader: Default::default(),
            best: 20,
            start_percent: 0.,
            collision: false,
            no_collision: 0.,
            total_damage: 0.,
            params: Params::default(),
        }
    }
}
impl Collector {
    /// The race (re)start reset 0x452520(1).
    /// - `onStartingGrid` becomes 1 only if the race was running.
    /// - `startAfterRestart` becomes 1 and is never cleared again.
    /// - The best position and the damage terms start over.
    pub fn restart(&mut self) {
        if self.in_race {
            self.on_grid = true;
        }
        self.restarted = true;
        self.best = 20;
        self.total_damage = 0.;
        self.collision = false;
        self.no_collision = 0.;
        self.plus1 = Default::default();
        self.minus1 = Default::default();
        self.leader = Default::default();
    }

    /// One collector frame; returns the parameters.
    pub fn update(&mut self, v: &RaceView, dt: f32) -> &Params {
        self.counter = self.counter.wrapping_add(1);
        // Race block: refreshed every frame, but only on frames whose counter is not a multiple
        // of 5 once racing.
        if !self.in_race || !self.counter.is_multiple_of(5) {
            self.in_race = v.started && !v.over;
            self.end_race = v.over;
            if self.in_race {
                self.on_grid = false;
            }
            self.clock = v.clock as f32;
            self.cars = v.entries.len() as u32;
            self.kind = v.kind;
        }
        let previous_position = self.player.position;
        let Some(&me) = v.entries.get(v.player) else { return &self.params };
        self.player.refresh(&me);
        self.best = self.best.min(me.position as u32);
        if !self.in_race && self.cars > 0 {
            // 0x452A42: integer division before × 100.
            self.start_percent = ((self.player.grid + 1) / self.cars) as f32 * 100.;
        }
        let at = |position: usize| v.entries.iter().position(|e| e.position == position);
        let follow = |slot: &mut (usize, usize, Record), car: Option<usize>| match car {
            Some(k) => {
                if slot.0 != k + 1 {
                    slot.2 = Record::default();
                }
                slot.0 = k + 1;
                slot.2.refresh(&v.entries[k]);
            }
            None => {
                slot.2 = Record::default();
                slot.0 = 0;
            }
        };
        if self.in_race {
            let position = me.position;
            match self.counter % 4 {
                0 => follow(&mut self.plus1, (position < self.cars as usize).then(|| at(position + 1)).flatten()),
                1 => follow(&mut self.minus1, (position > 1).then(|| at(position - 1)).flatten()),
                2 => follow(&mut self.leader, at(1)),
                // 3: the cars ahead and behind on the track (lapping lines), not ported.
                _ => {}
            }
        }
        // 0x431200 / 0x424780 on this frame's car: totalDam follows the damage only after a
        // player collision, until an update changes it by less than 1.
        if v.collision {
            self.collision = true;
        }
        if self.in_race && !self.end_race && self.cars > 0 && self.counter % self.cars == v.player as u32 && self.collision {
            let change = (v.damage - self.total_damage).abs();
            self.total_damage = v.damage;
            if change < 1. {
                self.collision = false;
            }
        }
        // Times in position (0x452D7D…): + dt while unchanged, else 0.
        self.player.in_position = if self.player.position == previous_position { self.player.in_position + dt } else { 0. };
        for slot in [&mut self.plus1, &mut self.minus1] {
            slot.2.in_position = if slot.0 != 0 && slot.0 == slot.1 { slot.2.in_position + dt } else { 0. };
            slot.1 = slot.0;
        }
        self.leader.2.in_position = if self.leader.0 == self.leader.1 { self.leader.2.in_position + dt } else { 0. };
        self.leader.1 = self.leader.0;
        self.no_collision = if self.collision { 0. } else { self.no_collision + dt };
        self.publish();
        &self.params
    }

    fn publish(&mut self) {
        use Param::*;
        let p = &mut self.params;
        let k = self.kind;
        p.flag(InRace, self.in_race && !self.player.finished);
        p.flag(EndRace, self.end_race);
        p.flag(OnStartingGrid, self.on_grid);
        p.flag(StartAfterRestart, self.restarted);
        p.set(RaceTime, self.clock);
        p.set(NumCarsInRace, self.cars as f32);
        p.set(PlayerPos, self.player.position);
        p.set(PlayerPrevPosition, self.player.previous);
        p.set(PlBestPosInRace, self.best as f32);
        p.set(PlayerTime, self.player.time);
        p.set(PlayerCarSpeed, self.player.speed);
        p.set(PlayerLapDist, self.player.fraction);
        p.set(PlayerLap, self.player.lap);
        p.flag(PlayerFinished, self.player.finished);
        p.set(PlayerStartPosPercentage, self.start_percent);
        p.set(PlTimeInCurrentPos, self.player.in_position);
        p.set(CarPosPlus1Time, self.plus1.2.time);
        p.set(Plus1TimeInCurrentPos, self.plus1.2.in_position);
        p.set(CarPosMinus1Time, self.minus1.2.time);
        p.set(Minus1TimeInCurrentPos, self.minus1.2.in_position);
        p.set(LeadTimeInCurrentPos, self.leader.2.in_position);
        p.flag(FirstCarFinished, self.leader.2.finished);
        p.flag(PlayerCollision, self.collision);
        p.set(NoCollisionTime, self.no_collision);
        p.set(TotalDam, self.total_damage);
        p.flag(IsNormalRace, !(k.drift || k.le_mans || k.touge || k.derby));
        p.flag(IsLeMans, k.le_mans);
        p.flag(IsTouge, k.touge);
        p.flag(IsDriftBattle, k.drift_battle);
        p.flag(IsDestructionDerby, k.derby);
    }
}

/// 0x424650 for the player:
/// - 100 × the mean of the six component levels (vehicle +0xCDF0);
/// - plus the largest of the four body-side terms 0x99EEB0, 0x99ED10, 0x99F1F0 and 0x99F050:
///   the RMS lattice displacement over the back two thirds and the front third of the long axis
///   and the two halves of the lateral axis, × 5/3, clamped to 1, as percentages.
///
/// The deformation lattice is the one published at vehicle +0x2300 (records of 0x50 bytes at
/// +0x2580 current and +0x2590 rest position; dimensions at +0xA414).
pub fn total_damage_percent(vehicle: &[u32]) -> f32 {
    let f = |o: usize| vehicle.get(o / 4).copied().map_or(0., f32::from_bits);
    let components: f32 = (0..6).map(|i| f(0xcdf0 + 4 * i)).sum();
    let base = 0x2300;
    let dim = |i: usize| vehicle.get((base + 0xa414) / 4 + i).copied().unwrap_or(0) as usize;
    let (nx, ny, nz) = (dim(0), dim(1), dim(2));
    let side = |z: std::ops::Range<usize>, x: std::ops::Range<usize>| -> f32 {
        let (mut sum, mut count) = (0f32, 0f32);
        for zi in z {
            for yi in 0..ny {
                for xi in x.clone() {
                    let record = base + 0x2590 + ((zi * ny + yi) * nx + xi) * 0x50;
                    count += 1.;
                    sum += (0..3).map(|a| (f(record - 0x10 + 4 * a) - f(record + 4 * a)).powi(2)).sum::<f32>();
                }
            }
        }
        let v = (sum / count).sqrt() * 1.666_666_6;
        v.clamp(0., 1.)
    };
    let third = nz / 3;
    let sides = [
        side(third..nz, 0..nx),
        side(0..third.max(1), 0..nx),
        side(0..nz, 0..nx / 2),
        side(0..nz, nx / 2..nx),
    ];
    let largest = sides.iter().fold(sides[3], |m, &s| if s > m { s } else { m }) * 100.;
    largest + components * (1. / 6.) * 100.
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element<'a>(name: &'a str, attributes: Vec<(&'a str, &'a str)>) -> binxml::Element<'a> {
        binxml::Element {
            name,
            value: None,
            attributes,
            children: 0..0,
            parent: None,
        }
    }

    #[test]
    fn operators_match_the_comparison_table() {
        let op = |name: &str| OPERATORS.iter().position(|o| *o == name).unwrap() as u8;
        // Inclusive "Plus/Minus" comparisons (comiss + jb).
        assert!(compare(op("moreThanParamPlusValue"), 7., 2., 5.));
        assert!(!compare(op("moreThanParamPlusValue"), 6.9, 2., 5.));
        assert!(compare(op("lessThanParamPlusValue"), 7.5, 2.5, 5.));
        assert!(compare(op("lessThanParamMinusValue"), 3., 2., 5.));
        assert!(compare(op("moreThanParamMinusValue"), 3., 2., 5.));
        assert!(compare(op("equalsParamMinusValue"), 3., 1., 4.));
        assert!(compare(op("moreThanParamDivValue"), 2., 2., 4.));
        assert!(compare(op("lessThanParam"), 3., UNSET, 4.));
        assert!(compare(op("equalsWith"), 4., UNSET, 4.));
        assert!(!compare(op("moreThan"), 1., 1., UNSET));
        assert!(compare(op("moreThanOrEqual"), 1., 1., UNSET));
        // NaN: ordered compares false, notEquals true.
        assert!(!compare(op("equals"), f32::NAN, 1., UNSET));
        assert!(compare(op("notEquals"), f32::NAN, 1., UNSET));
        assert!(!compare(OPERATORS.len() as u8, 1., 1., UNSET));
    }

    #[test]
    fn attribute_parsing_follows_the_reader() {
        let e = element("Message", vec![("priority", ""), ("queueValue", "3"), ("exclusive", "true")]);
        assert_eq!(int(&e, "priority"), 0);
        assert_eq!(float(&e, "queueValue"), 3.);
        assert!(boolean(&e, "exclusive"));
        assert!(!boolean(&e, "isEvent"));
        assert_eq!(param_index("noSuchParam"), Param::InRace as usize);
        assert_eq!(param_index("playerPos"), Param::PlayerPos as usize);
    }

    /// A hand-built rule set: one event, one group, `messages` as (name, priority, flags).
    fn rules(event_priority: u32, messages: &[(&str, u32, bool, bool)]) -> Rules {
        let bank = Bank {
            speeches: messages
                .iter()
                .map(|(n, ..)| Speech {
                    name: n.to_string(),
                    folder: "04_position".into(),
                    long: None,
                    variations: vec![format!("{n}_1"), format!("{n}_2")],
                    pa: false,
                })
                .collect(),
        };
        let messages: Vec<Message> = messages
            .iter()
            .enumerate()
            .map(|(i, (n, priority, exclusive, is_final))| Message {
                name: n.to_string(),
                event: 0,
                group: 0,
                speech: Some(i),
                priority: *priority,
                queue_value: 3.,
                repetition: 20.,
                max_total: 0,
                max_race: 1,
                max_event: 0,
                name_to_say: 4,
                linked: 0,
                delay: 0.,
                exclusive: *exclusive,
                is_event: true,
                keep: 0.,
                ignore_queue: false,
                ignore_silence: false,
                is_final: *is_final,
                conditions: vec![Condition { param: Param::PlayerPos as usize, op: 2, second: 0, value: (i + 1) as f32 }],
            })
            .collect();
        Rules {
            settings: Settings {
                minimum_silence: 7.,
                max_in_queue: 5,
                reduction: 5,
                delay: 0.,
                previous_queue: 5,
                enabled: true,
            },
            events: vec![Event {
                kind: "Position".into(),
                priority: event_priority,
                trigger: false,
                external: false,
                delay: 0.,
                conditions: vec![],
                groups: vec![0],
                on: true,
            }],
            groups: vec![Group {
                name: "g".into(),
                event: 0,
                delay: 0.,
                conditions: vec![],
                messages: (0..messages.len()).collect(),
            }],
            messages,
            bank,
        }
    }

    fn at(position: f32) -> Params {
        let mut p = Params::default();
        p.set(Param::PlayerPos, position);
        p
    }

    /// Run `seconds` of frames at 60 Hz with a voice that plays each line for `line` seconds.
    fn run(m: &mut Manager, p: &Params, seconds: f32, line: f32, played: &mut Vec<(f32, String)>) {
        let mut busy_until = -1f32;
        let mut t = 0.;
        while t < seconds {
            let out = m.frame(1. / 60., p, t < busy_until, false, &|_| true);
            if let Some(l) = out.play {
                played.push((m.now, l.message));
                busy_until = t + line;
            }
            t += 1. / 60.;
        }
    }

    #[test]
    fn a_line_plays_once_per_race_and_the_minimum_silence_holds() {
        let mut m = Manager::new(std::sync::Arc::new(rules(2, &[("pos1st", 2, false, false), ("pos2nd", 2, false, false)])), 1);
        let mut played = Vec::new();
        // The manager's clock starts at 0 with a silence base of 0: nothing that respects the
        // minimum silence plays in its first 7 s.
        run(&mut m, &at(1.), 6.5, 1., &mut played);
        assert_eq!(played.len(), 0, "{played:?}");
        run(&mut m, &at(9.), 2., 1., &mut played);
        run(&mut m, &at(1.), 3., 1., &mut played);
        assert_eq!(played.len(), 1, "{played:?}");
        assert_eq!(played[0].1, "pos1st");
        // The line plays the frame after its commit.
        assert!(played[0].0 < 8.55, "{played:?}");
        // maxPlayCountInRace 1: no second play.
        run(&mut m, &at(1.), 30., 1., &mut played);
        assert_eq!(played.len(), 1);
        // Another message waits for 2.5 s after the first line's start + 7 s of silence.
        let start = m.now;
        run(&mut m, &at(2.), 15., 1., &mut played);
        assert_eq!(played.len(), 2);
        assert_eq!(played[1].1, "pos2nd");
        assert!(played[1].0 <= start + 0.05, "silence already passed: {played:?}");
    }

    #[test]
    fn silence_delays_the_next_line() {
        let mut m = Manager::new(std::sync::Arc::new(rules(2, &[("pos1st", 2, false, false), ("pos2nd", 2, false, false)])), 1);
        let mut played = Vec::new();
        run(&mut m, &at(9.), 8., 1., &mut played);
        run(&mut m, &at(1.), 1., 1., &mut played);
        run(&mut m, &at(2.), 15., 1., &mut played);
        let gap = played[1].0 - played[0].0;
        assert!((9.5..9.6).contains(&gap), "{played:?}");
    }

    #[test]
    fn a_final_message_ends_the_speech() {
        let mut m = Manager::new(std::sync::Arc::new(rules(1, &[("win", 3, true, true), ("pos2nd", 1, false, false)])), 1);
        let mut played = Vec::new();
        run(&mut m, &at(1.), 1., 1., &mut played);
        run(&mut m, &at(2.), 30., 1., &mut played);
        assert_eq!(played.iter().map(|p| p.1.as_str()).collect::<Vec<_>>(), ["win"]);
        m.reset_race();
        run(&mut m, &at(2.), 1., 1., &mut played);
        assert_eq!(played.len(), 2);
    }

    #[test]
    fn queue_order_follows_event_then_message_priority() {
        let mut r = rules(2, &[("a", 3, false, false), ("b", 2, false, false), ("c", 0, false, false)]);
        r.events.push(Event { priority: 1, groups: vec![], ..r.events[0].clone() });
        let mut m = Manager::new(std::sync::Arc::new(r), 1);
        m.insert(0);
        m.insert(1);
        assert_eq!(m.queued(), ["b", "a"]);
        m.insert(2);
        assert_eq!(m.queued(), ["c", "b", "a"]);
        m.insert(1);
        assert_eq!(m.queued(), ["c", "b", "a"]);
    }

    #[test]
    fn play_percentage_falls_with_plays() {
        let mut m = Manager::new(std::sync::Arc::new(rules(2, &[("a", 3, false, false), ("b", 1, false, false)])), 1);
        assert_eq!(m.percent(0), 100);
        m.state[0].total = 4;
        assert_eq!(m.percent(0), 40);
        m.state[0].total = 10;
        assert_eq!(m.percent(0), 33);
        m.state[1].total = 50;
        assert_eq!(m.percent(1), 100);
    }

    #[test]
    fn variations_never_repeat_the_previous_file() {
        // Priority 0 bypasses the last-message and ring checks.
        let mut r = rules(2, &[("a", 0, false, false)]);
        r.messages[0].max_race = 0;
        r.messages[0].repetition = 0.;
        r.settings.previous_queue = 1;
        r.settings.minimum_silence = 0.;
        let mut m = Manager::new(std::sync::Arc::new(r), 7);
        let mut files = Vec::new();
        for _ in 0..4000 {
            let out = m.frame(1. / 60., &at(1.), false, false, &|_| true);
            if let Some(l) = out.play {
                files.push(l.file);
            }
        }
        assert!(files.len() > 10, "{files:?}");
        assert!(files.windows(2).all(|w| w[0] != w[1]), "{files:?}");
    }

    #[test]
    fn a_missing_file_drops_the_message() {
        let mut m = Manager::new(std::sync::Arc::new(rules(2, &[("a", 1, false, false)])), 1);
        let out = (0..10).map(|_| m.frame(1. / 60., &at(1.), false, false, &|_| false)).find(|o| o.play.is_some());
        assert!(out.is_none());
        assert_eq!(m.state[0].total, 0);
    }

    #[test]
    fn pause_clears_the_queue() {
        let mut m = Manager::new(std::sync::Arc::new(rules(2, &[("a", 2, false, false)])), 1);
        m.insert(0);
        let out = m.frame(1. / 60., &at(9.), true, true, &|_| true);
        assert!(out.stop);
        assert!(m.queue.is_empty());
    }

    fn entry(position: usize, grid: usize, progress: f64) -> Entry {
        Entry {
            time: 60.,
            progress,
            laps: progress as u32,
            speed: 30.,
            position,
            finished: false,
            grid,
        }
    }

    #[test]
    fn start_percentage_uses_integer_division() {
        let view = |grid: usize| RaceView {
            entries: vec![entry(grid + 1, grid, 0.), entry(1, 0, 0.), entry(2, 1, 0.), entry(3, 2, 0.)],
            ..RaceView::default()
        };
        for grid in 0..3 {
            let mut c = Collector::default();
            assert_eq!(c.update(&view(grid), 0.1).get(Param::PlayerStartPosPercentage), 0.);
        }
        let mut c = Collector::default();
        assert_eq!(c.update(&view(3), 0.1).get(Param::PlayerStartPosPercentage), 100.);
    }

    #[test]
    fn grid_flags_follow_a_restart_while_racing() {
        let mut c = Collector::default();
        let mut v = RaceView {
            entries: vec![entry(4, 3, 0.01), entry(1, 0, 0.)],
            ..RaceView::default()
        };
        c.update(&v, 0.1);
        c.restart();
        let p = c.update(&v, 0.1).clone();
        assert_eq!((p.get(Param::OnStartingGrid), p.get(Param::StartAfterRestart)), (0., 1.));
        v.started = true;
        c.update(&v, 0.1);
        c.restart();
        v.started = false;
        let p = c.update(&v, 0.1).clone();
        assert_eq!((p.get(Param::OnStartingGrid), p.get(Param::StartAfterRestart)), (1., 1.));
        v.started = true;
        assert_eq!(c.update(&v, 0.1).get(Param::OnStartingGrid), 0.);
    }

    #[test]
    fn records_hold_average_lap_times_and_position_timers() {
        let mut c = Collector::default();
        let mut v = RaceView {
            started: true,
            clock: 120.,
            entries: vec![entry(2, 1, 1.5), entry(1, 0, 1.6), entry(3, 2, 1.4)],
            ..RaceView::default()
        };
        v.entries[0].time = 120.;
        for _ in 0..8 {
            c.update(&v, 0.25);
        }
        let p = c.params.clone();
        assert_eq!(p.get(Param::PlayerTime), 80.);
        assert_eq!(p.get(Param::CarPosMinus1Time), 37.5);
        assert_eq!(p.get(Param::CarPosPlus1Time), 60. / 1.4);
        // The first frame moves from position 0 and restarts the timer.
        assert_eq!(p.get(Param::PlTimeInCurrentPos), 1.75);
        assert!(p.get(Param::Minus1TimeInCurrentPos) > 0.);
        assert_eq!(p.get(Param::PlayerCarSpeed), 30. * MPH);
        // A position gain: the previous position follows, the timer restarts.
        v.entries[0].position = 1;
        v.entries[1].position = 2;
        let p = c.update(&v, 0.25).clone();
        assert_eq!((p.get(Param::PlayerPos), p.get(Param::PlayerPrevPosition)), (1., 2.));
        assert_eq!(p.get(Param::PlTimeInCurrentPos), 0.);
        assert_eq!(p.get(Param::PlBestPosInRace), 1.);
    }

    #[test]
    fn total_damage_adds_the_largest_side() {
        let mut v = vec![0u32; 0xd000 / 4];
        // A 3×1×3 lattice at rest.
        for (i, d) in [3u32, 1, 3].into_iter().enumerate() {
            v[(0x2300 + 0xa414) / 4 + i] = d;
        }
        for i in 0..6 {
            v[(0xcdf0 + 4 * i) / 4] = 0.3f32.to_bits();
        }
        assert!((total_damage_percent(&v) - 30.).abs() < 1e-3);
        // One front-third record displaced by 0.3 on x.
        v[(0x2300 + 0x2580) / 4] = 0.3f32.to_bits();
        // Front third (z = 0): 3 records, mean square 0.03 → RMS 0.1732 × 5/3 = 0.2887.
        let expected = 30. + (0.09f32 / 3.).sqrt() * 1.666_666_6 * 100.;
        assert!((total_damage_percent(&v) - expected).abs() < 1e-2, "{}", total_damage_percent(&v));
    }

    /// The purchased files: every parameter of the slice resolves and the documented quirks hold.
    #[test]
    #[ignore = "requires the purchased GRID installation (GRID_GAME_ROOT)"]
    fn purchased_rules_parse() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT");
        let root = Path::new(&root);
        let rules = Rules::load(root).unwrap();
        assert_eq!(rules.settings.minimum_silence, 7.);
        assert_eq!((rules.settings.max_in_queue, rules.settings.reduction, rules.settings.previous_queue), (5, 5, 5));
        assert_eq!(rules.bank.speeches.len(), 907);
        let bytes = std::fs::read(root.join("audio/eventSpeech.bin")).unwrap();
        assert_eq!(rules.unknown_params(&bytes), Vec::<String>::new());
        let slice: Vec<_> = rules.messages.iter().filter(|m| rules.events[m.event].on).collect();
        assert_eq!(slice.len(), 158);
        assert!(slice.iter().all(|m| m.speech.is_some()));
        let m = |n: &str| &rules.messages[rules.message(n).unwrap()];
        assert_eq!(m("keyDriver22Ahead3s").priority, 0);
        assert!(m("win").is_final && m("win").exclusive && m("win").is_event);
        assert_eq!(rules.bank.speeches[m("firstWinEver").speech.unwrap()].long.as_deref(), Some("firstWinEver"));
        assert!(m("playerGridPosLast").ignore_silence);
        assert_eq!(rules.events[m("pos1st").event].priority, 2);
        assert_eq!(rules.events[m("win").event].priority, 1);
    }
}
