//! Race engineer speech (docs/SPEECH_AUDIO_RESEARCH.md). This ports:
//! - the original's data-driven rules (`audio/eventSpeech.bin`, parser 0x462FF0) over its line
//!   bank (`audio/audioEventSpeech.bin`);
//! - the evaluation, queue and playback rules of `audio_event_speech_manager` (0x45EC00,
//!   0x44BE90, 0x45E870);
//! - the parameters of `audio_game_info_collector` (0x452810, §5 and §8–§9) that the events of
//!   [`SLICE`] read.
//!
//! Only the events of [`SLICE`] run (the AUTHORED scope of parts 1 and 2). Lines are loaded
//! from the owner's installation at run time.
use crate::series::Mt;
use grid_formats::binxml;
use std::{collections::VecDeque, path::Path};

/// The `.raw` lines are PCM16 mono at 32 kHz (stream setup 0x42BD4B: 32000, AL_FORMAT_MONO16).
pub const LINE_RATE: u32 = 32_000;
/// Events evaluated [AUTHORED scope: part 1 the race start, position calls and finish; part 2
/// the player's damage, performance, strategy and rules lines, the accidents and AI events, the
/// teammate].
pub const SLICE: [&str; 9] = ["Position", "EndOfRace", "CarDamage", "Performance", "Strategy", "Rules", "CrashesAccidents", "AIEvents", "Teammates"];
/// An absent condition value or second parameter (0xC61C3C00).
const UNSET: f32 = -10000.;
/// The silence base after a line starts: now + 2.5 s (0xCB092C, 0x42FFC0).
const AFTER_LINE: f32 = 2.5;
/// The silence base after a `namedAI` name: now + 2.0 (0x42FFC0).
const AFTER_NAME: f32 = 2.;
/// A zero race progress divides by this instead (RAY_TOLERANCE 0xCB22E4, 0x438C10).
const TINY_PROGRESS: f64 = 1e-6;
/// m/s to mph, as the record's speed (0x438C10).
const MPH: f32 = 2.236_936_3;

macro_rules! params {
    ($($id:ident = $name:literal),* $(,)?) => {
        /// The collector parameters the [`SLICE`] rules read (names registered by 0x45F590).
        /// `InRace` comes first: 0x44C510 resolves an unknown name to index 0.
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
    // Part 2: CarDamage, Performance, Strategy and Rules.
    TerminalDam = "terminalDam",
    BodyDam = "bodyDam",
    FrontDam = "frontDam",
    RearDam = "rearDam",
    RightDam = "rightDam",
    LeftDam = "leftDam",
    EngDam = "engDam",
    RadiatorDam = "radiatorDam",
    GearboxDam = "gearboxDam",
    TyreDamage = "tyreDamage",
    FrontTyreDamage = "frontTyreDamage",
    RearTyreDam = "rearTyreDam",
    FrontRightTyreDam = "frontRightTyreDam",
    FrontLeftTyreDam = "frontLeftTyreDam",
    RearRightTyreDam = "rearRightTyreDam",
    RearLeftTyreDam = "rearLeftTyreDam",
    DamLastAccident = "damLastAccident",
    DamageTakenInPrevLap = "damageTakenInPrevLap",
    LossRightWing = "lossRightWing",
    LossLeftWing = "lossLeftWing",
    LossFrontWing = "lossFrontWing",
    LossRearWing = "lossRearWing",
    LossDoor = "lossDoor",
    LossBumper = "lossBumper",
    LooseBonnet = "looseBonnet",
    IsPlayerCarOpenCockpit = "isPlayerCarOpenCockpit",
    HitAnotherCar = "hitAnotherCar",
    CarToCarCollision = "carToCarCollision",
    AiTermUserImpact = "AITermUserImpact",
    NoRestartOptionEnabled = "noRestartOptionEnabled",
    TerminalDamageCount = "terminalDamageCount",
    HasNoDamageObj = "hasNoDamageObj",
    AnyDamageIndOn = "anyDamageIndOn",
    DroveOffTrack = "droveOffTrack",
    TimeOffTrack = "timeOffTrack",
    WrongWayTime = "wrongWayTime",
    SatOnTrackTime = "satOnTrackTime",
    SlipStreamTime = "slipStreamTime",
    IsDriftFreeStyle = "isDriftFreeStyle",
    IsDownhillDrift = "isDownhillDrift",
    BonusObjWinPos = "bonusObjWinPos",
    DriverChampWinPos = "driverChampWinPos",
    OneTwoFinish = "1_2Finish",
    PlayerCrossedSfLine = "playerCrossedSFLine",
    CurrentTrackLaps = "currentTrackLaps",
    CurrentPaceTime = "currentPaceTime",
    NumRunningCars = "numRunningCars",
    PlayerPrevLapTime = "playerPrevLapTime",
    MaxPlayerCarSpeed = "maxPlayerCarSpeed",
    PlayerCarRevRatio = "playerCarRevRatio",
    PlayerGoodStart = "playerGoodStart",
    CarsPlayerOverTook = "carsPlayerOverTook",
    UsedFlashback = "usedFlashback",
    NewWorldRecord = "newWorldRecord",
    NewPersonalBest = "newPersonalBest",
    TargetTime = "targetTime",
    TargetSpeed = "targetSpeed",
    TargetDamage = "targetDamage",
    AiSpin = "aiSpin",
    AiRoll = "aiRoll",
    // Step 3: CrashesAccidents and AIEvents.
    PlayerVehicleId = "playerVehicleID",
    TeammateVehicleId = "teammateVehicleID",
    TeammateCollision = "teammateCollision",
    PlayerCollWithKeyPlayer = "playerCollWithKeyPlayer",
    KeyDriversOut = "keyDriversOut",
    KeyTeamInRace = "keyTeamInRace",
    IsDrift = "isDrift",
    // Step 4: the teammate (§11, §15).
    TeammatePos = "teammatePos",
    TeammateFinished = "teammateFinished",
    TeammateTermDam = "teammateTermDam",
    TeammateTotDam = "teammateTotDam",
    TeamDriverChampWinPos = "teamDriverChampWinPos",
    PlayerCollideWithTeammate = "playerCollideWithTeammate",
    TmTimeInCurrentPos = "tmTimeInCurrentPos",
    TeamDamFromCollision = "teamDamFromCollision",
    TeammateTyreBlown = "teammateTyreBlown",
    TeammateSpun = "teammateSpun",
    TeammateRoll = "teammateRoll",
    TeammateEngBlown = "teammateEngBlown",
);

/// The numbered getter families of 0x45F590: `AI1…AI20` (race entries 0…19) and `Key1…Key24`
/// (`driver.audio_key_driver_number`).
pub const AI_SLOTS: usize = 20;
pub const KEYS: usize = 24;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiField {
    Collision,
    CollisionDam,
    Distance,
    TermDam,
    TyreBlown,
    EngBlown,
    Spun,
    Roll,
    ForcedRetire,
}
const AI_FIELDS: [&str; 9] = ["Collision", "CollisionDam", "Distance", "TermDam", "TyreBlown", "EngBlown", "Spun", "Roll", "ForcedRetire"];
const KEY_FIELDS: [&str; 3] = ["Collision", "CollisionDam", "TermDam"];
const NAMED: usize = PARAM_NAMES.len();
pub const PARAMS: usize = NAMED + AI_SLOTS * AI_FIELDS.len() + KEYS * KEY_FIELDS.len();

/// The value index of `AI<slot + 1><field>`.
pub fn ai_param(slot: usize, field: AiField) -> usize {
    NAMED + field as usize * AI_SLOTS + slot
}

/// The value index of `Key<n><field>` (`field` 0 Collision, 1 CollisionDam, 2 TermDam; n from 1).
pub fn key_param(n: usize, field: usize) -> usize {
    NAMED + AI_SLOTS * AI_FIELDS.len() + field * KEYS + n - 1
}

/// 0x4244B0: a `nationality_voice.voice` name as `teammateNumber`; 0 for any other name (the
/// database's `japan_1` among them).
pub fn voice_number(voice: &str) -> i32 {
    match voice {
        "us_1" => 1,
        "us_2" => 2,
        "us_3" => 3,
        "british_1" => 4,
        "british_2" => 5,
        "british_3" => 6,
        "french_1" => 7,
        "italian_1" => 8,
        "german_1" => 9,
        "spanish_1" => 10,
        "japanese_1" => 11,
        "japanese_2" => 12,
        "scan_1" => 13,
        "irish_1" => 14,
        _ => 0,
    }
}

/// A race entry's driver as the car record reads it (0x438C10).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Identity {
    /// `driver.audio_key_driver_number` (+0x3C), 0 without a driver.
    pub key: i32,
    /// `team.id` of the entry's team (+0x44), 0 without one [I: the port's entry has its
    /// driver's `driver.team_id`].
    pub team: i32,
    /// `driver.pit_speech_filename`, the name phrase of `namedAI` lines.
    pub name_file: Option<String>,
}

/// The drivers of a race for the collector.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Identities {
    /// By race entry.
    pub entries: Vec<Identity>,
    /// The teammate's entry (vf 0x5C, 0x430AA0).
    pub teammate: Option<usize>,
    /// Race block +0x34 (`teammateNumber`).
    pub teammate_number: i32,
}

impl Identities {
    /// `drivers[k]` is race entry k's `driver.id`. `teammate` is an entry index. In a career race
    /// (`career`) the race block takes the teammate's voice through 0x430D60, 0 without one;
    /// elsewhere it keeps the cleared −1 (0x451A20 computes it only in the career mode) [I: the
    /// two event kinds 0x451A20 also excludes are not identified].
    ///
    /// 0x430D60 skips (count of picks for the nationality) % (voices of the nationality) rows of
    /// `nationality_voice` and counts the pick; the port's count is 0, the first teammate of a
    /// game session [I: the original keeps the counts and the last driver's voice for the whole
    /// session].
    pub fn load(db: &grid_formats::database::Database, drivers: &[Option<i32>], teammate: Option<usize>, career: bool) -> Self {
        let row = |id: i32| db.table("driver")?.rows().find(|r| r.int("id") == Some(id));
        let entries = drivers
            .iter()
            .map(|d| match d.and_then(row) {
                Some(r) => Identity {
                    key: r.int("audio_key_driver_number").unwrap_or(0),
                    team: r.int("team_id").unwrap_or(0),
                    name_file: r.text("pit_speech_filename").map(str::to_owned),
                },
                None => Identity::default(),
            })
            .collect();
        let teammate = teammate.filter(|&t| drivers.get(t).copied().flatten().is_some());
        let teammate_number = if !career {
            -1
        } else {
            let nationality = teammate.and_then(|t| drivers[t]).and_then(row).and_then(|r| r.int("nationality_id"));
            let voice = nationality.and_then(|n| {
                db.table("nationality_voice")?.rows().find(|r| r.int("nationality_id") == Some(n))?.text("voice").map(str::to_owned)
            });
            voice.map_or(0, |v| voice_number(&v))
        };
        Self {
            entries,
            teammate,
            teammate_number,
        }
    }
}

/// The value index of a registered name, or `None`.
fn known(name: &str) -> Option<usize> {
    if let Some(i) = PARAM_NAMES.iter().position(|n| *n == name) {
        return Some(i);
    }
    let family = |prefix: &str, fields: &[&str], count: usize, base: usize| {
        let rest = name.strip_prefix(prefix)?;
        let digits = rest.chars().take_while(char::is_ascii_digit).count();
        let n: usize = rest[..digits].parse().ok()?;
        let f = fields.iter().position(|f| *f == &rest[digits..])?;
        (1..=count).contains(&n).then_some(base + f * count + n - 1)
    };
    family("AI", &AI_FIELDS, AI_SLOTS, NAMED).or_else(|| family("Key", &KEY_FIELDS, KEYS, NAMED + AI_SLOTS * AI_FIELDS.len()))
}

fn param_index(name: &str) -> usize {
    known(name).unwrap_or(0)
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
    /// A value by index ([`ai_param`]).
    pub fn at(&self, i: usize) -> f32 {
        self.0[i]
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
    /// `userData` (+0x78): for `namedAI`, the race entry + 1 whose driver is named.
    pub user_data: f32,
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
    /// Every line the [`SLICE`] events can play: each message's long version
    /// and variations (`try_start`). The bundled APKs carry these files, as a
    /// run plays only some of them (F608).
    pub fn playable_lines(&self) -> Vec<Line> {
        let mut lines: Vec<Line> = Vec::new();
        for msg in self.messages.iter().filter(|m| self.events[m.event].on) {
            let Some(s) = msg.speech else { continue };
            let speech = &self.bank.speeches[s];
            for file in speech.long.iter().chain(&speech.variations) {
                if !lines.iter().any(|l| l.folder == speech.folder && &l.file == file) {
                    lines.push(Line {
                        message: msg.name.clone(),
                        folder: speech.folder.clone(),
                        file: file.clone(),
                        pa: speech.pa,
                    });
                }
            }
        }
        lines
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
                        user_data: float(x, "userData"),
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
                if known(name).is_none() && !unknown.iter().any(|u| u == name) {
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
    /// +0x1D4: the line stream holds a line not yet played; +0x60A: the line was played.
    line_ready: bool,
    line_played: bool,
    /// The named-AI stream (+0x3E8) and +0x60E: a `namedAI` name is armed for the line.
    name: Option<Line>,
    named: bool,
    /// Each race entry's `driver.pit_speech_filename` (game interface vf 0xC; vf 0x10 the count).
    ai_names: Vec<Option<String>>,
    /// The player-name phrases by nameToSay 0–2: the streams +0x338 (`OK_`), +0x1D8 (`WD_`) and
    /// +0x288 (`BL_`), loaded once the profile has an audio name (+0x608/+0x609).
    player_names: Option<[Line; 3]>,
    /// The armed player-name phrase (the stream flags +0x3E4, +0x284, +0x334): its nameToSay.
    player_name: Option<usize>,
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
            line_ready: false,
            line_played: false,
            name: None,
            named: false,
            ai_names: Vec::new(),
            player_names: None,
            player_name: None,
            state,
        }
    }

    /// The race entries' name files for `namedAI` lines (entry k at k; `None` without a driver).
    pub fn set_ai_names(&mut self, names: Vec<Option<String>>) {
        self.ai_names = names;
    }

    /// The profile's audio name (`M_01`…, `F_nn`, `N_nn`; `None`: "Disable Audio Name"). In a
    /// race the engineer's phrases are `Names_Player/pit/Chief_{OK,WD,BL}_<name>` (0x465690
    /// with 2; the `man/Man_{Hello,WD,Hi}_` set is its other mode).
    pub fn set_player_name(&mut self, name: Option<&str>) {
        self.player_names = name.map(player_name_lines);
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
        self.line_ready = false;
        self.line_played = false;
        self.name = None;
        self.named = false;
        self.player_name = None;
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
            self.stop(true);
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
            None => self.playback(voice_busy, exists, &mut out),
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
    /// 0x430350(`clear`): the player-name flags clear; an armed `namedAI` name is dropped with
    /// the pending playback; `clear` also empties a line stream that is not loading.
    fn stop(&mut self, clear: bool) {
        self.player_name = None;
        if self.named {
            self.named = false;
            self.pending = false;
            self.name = None;
        }
        if clear && !self.loaded {
            self.line_ready = false;
        }
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
                self.stop(false);
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
        // The name phrase roll (0x437F70). With the profile's names loaded (+0x609), nameToSay
        // 0–2 arm that player-name phrase. `namedAI` (3) arms the driver of entry `userData` − 1,
        // which must be above 0 and below the entry count [I: an entry without a driver arms
        // nothing].
        let roll = int_between(&mut self.rng, 0, 100) <= msg.linked;
        if roll && self.player_names.is_some() && msg.name_to_say < 3 {
            self.player_name = Some(msg.name_to_say as usize);
        } else if roll && msg.name_to_say == 3 {
            let entry = msg.user_data as i32 - 1;
            if msg.user_data as i32 > 0
                && entry > 0
                && let Some(Some(file)) = self.ai_names.get(entry as usize)
            {
                self.name = Some(Line {
                    message: msg.name.clone(),
                    folder: "Names_AI".into(),
                    file: file.clone(),
                    pa: false,
                });
                self.named = true;
            }
        }
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
            self.line_ready = true;
            self.current = None;
            self.loaded = false;
        } else {
            self.stop(true);
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

    /// 0x45E870 → 0x42FFC0: once the delays have passed, an armed player-name phrase plays
    /// first, then the committed line on a later call, then an armed `namedAI` name once the
    /// voice is free again.
    fn playback(&mut self, voice_busy: bool, exists: &dyn Fn(&Line) -> bool, out: &mut Output) {
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
        // The player's name: it plays on its own call and moves neither pending nor the
        // silence base. [I] A missing file plays nothing and the line follows at once.
        if let Some(k) = self.player_name.take()
            && let Some(name) = self.player_names.as_ref().map(|n| n[k].clone())
            && exists(&name)
        {
            out.play = Some(Line { message: self.msg(l).name.clone(), ..name });
            return;
        }
        if self.line_ready {
            out.play = self.stream.clone();
            self.line_ready = false;
            self.line_played = true;
            // A `namedAI` line with its name armed stays pending for the name.
            if !(self.msg(l).name_to_say == 3 && self.named) {
                self.pending = false;
                self.silence = self.now + AFTER_LINE;
            }
        } else if let Some(name) = self.name.clone() {
            if !self.line_played {
                return;
            }
            self.pending = false;
            self.name = None;
            // The name loaded: it plays, and the silence base is now + 2.0. A load error
            // ("Error loading NAMED_AI") only ends the playback.
            if exists(&name) {
                out.play = Some(name);
                self.silence = self.now + AFTER_NAME;
                self.line_played = false;
            }
            self.named = false;
        }
    }
}

/// The engineer's player-name phrases for an audio name, by nameToSay 0–2 (0x465690 with 2):
/// `Names_Player/pit/Chief_OK_`, `Chief_WD_` and `Chief_BL_` + the name.
pub fn player_name_lines(name: &str) -> [Line; 3] {
    ["OK_", "WD_", "BL_"].map(|kind| Line {
        message: String::new(),
        folder: "Names_Player/pit".into(),
        file: format!("Chief_{kind}{name}"),
        pa: false,
    })
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
    /// The last completed lap's time (s, vf 0x5C).
    pub last_lap: Option<f64>,
    /// Driven by the AI (controller type, entry +0x6C, other than 0).
    pub ai: bool,
    /// Terminally damaged: the damage status (+0x444, vehicle +0xD234) is not 7 (0x99A720).
    pub terminal: bool,
    /// The car's total damage percentage of 0x424650: [`Car::total`] for the player's car,
    /// [`ai_total_percent`] for the others.
    pub damage: f32,
    /// [`Identity::key`] and [`Identity::team`].
    pub key: i32,
    pub team: i32,
    /// Arc length along the racing line (race car +0x78), the key of the track order.
    pub arc: f32,
    /// Racing: the entry's race state (vf 0x50) is 0.
    pub racing: bool,
}

/// The race kind flags of the race block (0x451A20).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Kind {
    pub drift: bool,
    pub drift_battle: bool,
    pub drift_freestyle: bool,
    pub le_mans: bool,
    pub touge: bool,
    pub derby: bool,
}

/// A contact the collision callback 0x430F50 accepts: its impulse is at least half the physical
/// mass of `car`, the car hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Contact {
    /// The entry hit.
    pub car: usize,
    /// The other object's entry when it is a car.
    pub other: Option<usize>,
}

/// The player's vehicle words the collector reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Car {
    /// The damage object (vehicle +0xCDF0): component levels (+0) and wheel levels (+0x18,
    /// records RL, RR, FL, FR), 0 to 1.
    pub components: [f32; 6],
    pub wheels: [f32; 4],
    /// The body-side terms of 0x424650 in the order 0x99EEB0, 0x99ED10, 0x99F1F0, 0x99F050
    /// (front, rear, right and left by the getters' names), 0 to 1.
    pub sides: [f32; 4],
    /// Engine speed (+0x1894) and its limit (+0x17A4).
    pub engine: f32,
    pub limit: f32,
    /// The low byte of +0x2204 (the launch flag of the grip pipeline).
    pub launch: u8,
    /// Air density (+0x21C4, 0x98DFE0).
    pub density: f32,
}
impl Car {
    pub fn read(vehicle: &[u32]) -> Self {
        let f = |o: usize| vehicle.get(o / 4).copied().map_or(0., f32::from_bits);
        Self {
            components: std::array::from_fn(|i| f(0xcdf0 + 4 * i)),
            wheels: std::array::from_fn(|i| f(0xce08 + 4 * i)),
            sides: body_sides(vehicle),
            engine: f(0x1894),
            limit: f(0x17a4),
            launch: vehicle.get(0x2204 / 4).map_or(0, |w| (w & 255) as u8),
            density: f(0x21c4),
        }
    }
    /// 0x424650 for the player: the largest side term plus the mean of the six components, as
    /// percentages.
    pub fn total(&self) -> f32 {
        let largest = self.sides.iter().fold(self.sides[3], |m, &s| if s > m { s } else { m }) * 100.;
        largest + self.components.iter().sum::<f32>() * (1. / 6.) * 100.
    }
}

/// The four body-side terms of 0x424650: 0x99EEB0, 0x99ED10, 0x99F1F0 and 0x99F050, the RMS
/// lattice displacement over the back two thirds and the front third of the long axis and the
/// two halves of the lateral axis, × 5/3, clamped to 1.
///
/// The deformation lattice is the one published at vehicle +0x2300 (records of 0x50 bytes at
/// +0x2580 current and +0x2590 rest position; dimensions at +0xA414).
fn body_sides(vehicle: &[u32]) -> [f32; 4] {
    let f = |o: usize| vehicle.get(o / 4).copied().map_or(0., f32::from_bits);
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
    [side(third..nz, 0..nx), side(0..third.max(1), 0..nx), side(0..nz, 0..nx / 2), side(0..nz, nx / 2..nx)]
}

/// 0x424650 for a car other than the player's: 0x99EBB0 (the RMS lattice displacement over the
/// whole lattice × 5/3, clamped to 1; 0 unless lattice byte +0xA4AD is set) plus the mean of the
/// six components, as percentages.
pub fn ai_total_percent(vehicle: &[u32]) -> f32 {
    let f = |o: usize| vehicle.get(o / 4).copied().map_or(0., f32::from_bits);
    let base = 0x2300;
    let components: f32 = (0..6).map(|i| f(0xcdf0 + 4 * i)).sum();
    let dim = |i: usize| vehicle.get((base + 0xa414) / 4 + i).copied().unwrap_or(0) as usize;
    let count = dim(0) * dim(1) * dim(2);
    let flag = vehicle.get((base + 0xa4ad) / 4).is_some_and(|w| (w >> (8 * ((base + 0xa4ad) % 4))) & 255 != 0);
    let lattice = if flag && count > 0 {
        let sum: f32 = (0..count)
            .map(|k| {
                let record = base + 0x2590 + k * 0x50;
                (0..3).map(|a| (f(record - 0x10 + 4 * a) - f(record + 4 * a)).powi(2)).sum::<f32>()
            })
            .sum();
        ((sum / count as f32).sqrt() * 1.666_666_6).clamp(0., 1.)
    } else {
        0.
    };
    lattice * 100. + components * (1. / 6.) * 100.
}

/// 0x424650 for the player:
/// - 100 × the mean of the six component levels (vehicle +0xCDF0);
/// - plus the largest of the four body-side terms ([`Car::sides`]) as a percentage.
pub fn total_damage_percent(vehicle: &[u32]) -> f32 {
    Car::read(vehicle).total()
}

/// The original part ids (name table 0xE4EF20) whose loss 0x417D60 reports, with the flag each
/// sets: 10 bonnet; 12–17 the bumpers; 22–25 the doors; 84 `wing_bl`, 85 `wing_br`, 86 `wing_fl`
/// and 87 `wing_fr`, whose flags are named rear, right, left and front.
pub fn part_id(name: &str) -> Option<u32> {
    Some(match name {
        "bonnet" => 10,
        "boot" => 11,
        "bumper_bl" => 12,
        "bumper_br" => 13,
        "bumper_bm" => 14,
        "bumper_fl" => 15,
        "bumper_fr" => 16,
        "bumper_fm" => 17,
        "door_bl" => 22,
        "door_br" => 23,
        "door_fl" => 24,
        "door_fr" => 25,
        "exhaust" => 31,
        "exhaust_b" => 34,
        "wing_bl" => 84,
        "wing_br" => 85,
        "wing_fl" => 86,
        "wing_fr" => 87,
        "exhaust_bl" => 101,
        "exhaust_br" => 102,
        _ => return None,
    })
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
    /// The race's lap count (race block +0x20).
    pub track_laps: u32,
    pub entries: Vec<Entry>,
    pub player: usize,
    pub kind: Kind,
    /// The contacts of this frame that pass 0x430F50's impulse test.
    pub contacts: Vec<Contact>,
    /// The player's vehicle.
    pub car: Car,
    /// The player's driver test of type 11 (`on_track`) fails (record +0x71).
    pub off_track: bool,
    /// The race interface's wrong-way flag (+0x18).
    pub wrong_way: bool,
    /// Part ids ([`part_id`]) the player's car lost this frame (0x9FF4F0 → 0x417D60).
    pub parts_lost: Vec<u32>,
    /// Car description byte +0xD4 (`isPlayerCarOpenCockpit`), read before the race.
    pub open_cockpit: bool,
    /// `TerminalDamageMessage`s of this frame (0x9E64E0 → 0x417C10): the entry and its new
    /// damage status.
    pub status_changes: Vec<(usize, u32)>,
    /// The choreographer's `AIChoreographerEvent`s of this frame (0x408E50 → 0x417CB0).
    pub ai_events: Vec<crate::ai_pacing::Message>,
    /// [`Identities::teammate`] and [`Identities::teammate_number`].
    pub teammate: Option<usize>,
    pub teammate_number: i32,
}

/// A car record (0x438C10) with the collector's time in position.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Record {
    time: f32,
    progress: f32,
    /// +8: the last completed lap's time, or `time` before the second lap.
    last_lap: f32,
    speed: f32,
    /// +0x10: the highest speed since the record was cleared.
    max_speed: f32,
    fraction: f32,
    lap: f32,
    position: f32,
    previous: f32,
    finished: bool,
    off_track: bool,
    terminal: bool,
    grid: u32,
    in_position: f32,
    /// +0x3C the key driver number and +0x44 the team: −1 in a cleared record (0x4689C0), then
    /// taken at the first refresh.
    key: Option<i32>,
    team: Option<i32>,
    /// +0x75: racing (vf 0x50 is 0).
    racing: bool,
    /// +0x34: the entry's place in the track order (game interface vf 0x80, 0x445770).
    track: i32,
}
impl Record {
    /// 0x438C10. The previous position changes only when the position does.
    fn refresh(&mut self, e: &Entry) {
        let progress = if e.progress == 0. { TINY_PROGRESS } else { e.progress };
        self.time = (e.time / progress) as f32;
        self.progress = e.progress as f32;
        self.lap = (e.laps + 1) as f32;
        self.last_lap = match e.last_lap {
            Some(t) if self.lap > 1. => t as f32,
            _ => self.time,
        };
        self.speed = e.speed * MPH;
        if self.speed > self.max_speed {
            self.max_speed = self.speed;
        }
        self.fraction = e.progress.rem_euclid(1.) as f32;
        let position = e.position as f32;
        if self.position != position {
            self.previous = self.position;
        }
        self.position = position;
        self.finished = e.finished;
        self.terminal = e.terminal;
        self.grid = e.grid as u32;
        self.key.get_or_insert(e.key);
        self.team.get_or_insert(e.team);
        self.racing = e.racing;
    }
}

/// The player's part-loss flags of 0x417D60 (+0xB45C…+0xB462).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Lost {
    door: bool,
    bumper: bool,
    front_wing: bool,
    rear_wing: bool,
    left_wing: bool,
    right_wing: bool,
    bonnet: bool,
}

/// One race entry's AI slot of the collector.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct Slot {
    /// +0xB5AC + i, +0xB5C0 + 4i (the last total), +0xB610 + 4i, +0xB55C + 4i, +0xB660 + i.
    collision: bool,
    total: f32,
    damage: f32,
    distance: f32,
    terminal: bool,
    /// +0xB674 + 0x14·k + i, in 0x417CB0's order: engine blown, tyre blown, spun, roll, forced
    /// retire.
    events: [bool; 5],
}

/// One key driver's values: +0xB465 + n `KeynCollision`, +0xB47C + 4n the last total,
/// +0xB4DC + 4n `KeynCollisionDam`, +0xB53F + n `KeynTermDam`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct KeySlot {
    collision: bool,
    total: f32,
    damage: f32,
    terminal: bool,
}

/// `audio_game_info_collector` (0x452810) for the parameters of [`Param`].
#[derive(Clone, Debug)]
pub struct Collector {
    /// +0x18: cleared by the player's terminal damage; the update then stops.
    enabled: bool,
    /// +0xB738: the paused path has cleared the records.
    paused: bool,
    /// +0xB410 frame counter.
    counter: u32,
    in_race: bool,
    end_race: bool,
    on_grid: bool,
    restarted: bool,
    clock: f32,
    cars: u32,
    kind: Kind,
    /// Race block +0x08, +0x20 and +0x24.
    pace: f32,
    track_laps: u32,
    running: u32,
    player: Record,
    /// Plus 1, minus 1 and the leader: (car + 1 or 0, previous frame's car + 1, record).
    plus1: (usize, usize, Record),
    minus1: (usize, usize, Record),
    leader: (usize, usize, Record),
    best: u32,
    start_percent: f32,
    /// The live vehicle and the damage object copied every second frame (+0x470, 0x9EF020).
    car: Car,
    copy: Car,
    /// +0xB43A `playerCollision`, +0xB43B `hitAnotherCar`, +0xB439 `carToCarCollision`.
    collision: bool,
    hit_car: bool,
    car_to_car: bool,
    no_collision: f32,
    /// +0xB3E0 `totalDam`, +0xB440 `damLastAccident`.
    total_damage: f32,
    last_accident: f32,
    /// +0xB708 `bodyDam` and +0xB70C…+0xB718 the four sides (0x424650).
    body: f32,
    sides: [f32; 4],
    /// +0xB450 the damage of the lap so far, +0xB44C `damageTakenInPrevLap`, +0xB3F4 the lap
    /// `playerCrossedSFLine` last saw.
    lap_damage: f32,
    prev_lap_damage: f32,
    seen_lap: f32,
    /// +0xB40C the overtake reference place and +0xB418 `carsPlayerOverTook`.
    reference: u32,
    overtook: u32,
    /// +0xB3EC progress last frame; +0xB3F0, +0xB3FC, +0xB400 and +0xB430 the timers.
    last_progress: f32,
    sat: f32,
    slipstream: f32,
    wrong_way: f32,
    off_track_time: f32,
    lost: Lost,
    open_cockpit: bool,
    /// +0xB72D `usedFlashback`.
    flashback: bool,
    /// +0xB71C `1_2Finish` (career).
    one_two: i32,
    /// +0xB720 `playerVehicleID`.
    player_id: u32,
    slots: [Slot; AI_SLOTS],
    /// +0xB6D9 `aiSpin`, +0xB6D8 `aiRoll`.
    ai_spin: bool,
    ai_roll: bool,
    /// +0xE8 the teammate's entry (cleared with the records, 0x417E40), +0x3F4 its record and
    /// +0xB408 its position at the last frame.
    teammate: Option<usize>,
    mate: Record,
    mate_previous: f32,
    /// Race block +0x34 `teammateNumber` (−1 when cleared).
    teammate_number: i32,
    /// +0xB724 `teammateVehicleID`.
    teammate_id: u32,
    /// +0xB43C `playerCollideWithTeammate`; +0xB43D `teammateCollision` and +0xB43E/+0xB43F (the
    /// teammate hit a car, the player; no getter).
    hit_mate: bool,
    mate_collision: bool,
    mate_hit_car: bool,
    mate_hit_player: bool,
    /// +0xB3E4 `teammateTotDam`, +0xB444 `teamDamFromCollision`.
    mate_total: f32,
    mate_damage: f32,
    /// +0xB434 + k: the teammate's flags in 0x417CB0's order (`teammateEngBlown`,
    /// `teammateTyreBlown`, `teammateSpun`, `teammateRoll`, a forced retirement without getter).
    mate_events: [bool; 5],
    /// Each race entry's key driver number once taken (entry +0x50/+0x54).
    keys: Vec<Option<i32>>,
    /// By key driver number; index 0 (+0xB465) is cleared but never set.
    key_slots: [KeySlot; KEYS + 1],
    /// +0xB558 `playerCollWithKeyPlayer` (−1 when cleared), +0xB6E8 `keyDriversOut`, +0xB6EC
    /// `keyTeamInRace`.
    coll_key: i32,
    keys_out: i32,
    key_team: i32,
    /// The race's track order (race +0x21A0 → +0xD0): entries by arc along the racing line,
    /// ascending, kept from frame to frame (insertion sort 0x91B240).
    track: Vec<usize>,
    /// +0xEC/+0x2FC the car ahead on the track and +0xF0/+0x378 the car behind (car + 1 or 0).
    ahead: (usize, Record),
    behind: (usize, Record),
    /// +0xB414 the track place reference and +0xB424 `playerMovedTrackPos`.
    track_reference: i32,
    moved: bool,
    params: Params,
}
impl Default for Collector {
    fn default() -> Self {
        Self {
            enabled: true,
            paused: false,
            counter: 0,
            in_race: false,
            end_race: false,
            on_grid: false,
            restarted: false,
            clock: 0.,
            cars: 0,
            kind: Kind::default(),
            pace: 0.,
            track_laps: 0,
            running: 0,
            player: Record::default(),
            plus1: Default::default(),
            minus1: Default::default(),
            leader: Default::default(),
            best: 20,
            start_percent: 0.,
            car: Car::default(),
            copy: Car::default(),
            collision: false,
            hit_car: false,
            car_to_car: false,
            no_collision: 0.,
            total_damage: 0.,
            last_accident: 0.,
            body: 0.,
            sides: [0.; 4],
            lap_damage: 0.,
            prev_lap_damage: 0.,
            // The race set-up runs 0x452520(1) before the first frame.
            seen_lap: 1.,
            reference: 0,
            overtook: 0,
            last_progress: 0.,
            sat: 0.,
            slipstream: 0.,
            wrong_way: 0.,
            off_track_time: 0.,
            lost: Lost::default(),
            open_cockpit: false,
            flashback: false,
            one_two: 0,
            player_id: 0,
            slots: [Slot::default(); AI_SLOTS],
            ai_spin: false,
            ai_roll: false,
            teammate: None,
            mate: Record::default(),
            mate_previous: 0.,
            teammate_number: -1,
            teammate_id: 0,
            hit_mate: false,
            mate_collision: false,
            mate_hit_car: false,
            mate_hit_player: false,
            mate_total: 0.,
            mate_damage: 0.,
            mate_events: [false; 5],
            keys: Vec::new(),
            key_slots: [KeySlot::default(); KEYS + 1],
            coll_key: -1,
            keys_out: 0,
            key_team: 0,
            track: Vec::new(),
            ahead: Default::default(),
            behind: Default::default(),
            track_reference: 0,
            moved: false,
            params: Params::default(),
        }
    }
}
impl Collector {
    /// The race (re)start reset 0x452520(1): [`Self::reset`] with the grid flag.
    /// - `onStartingGrid` becomes 1 only if the race was running.
    /// - `startAfterRestart` becomes 1 and stays (§5).
    pub fn restart(&mut self) {
        self.reset(true);
        // [I] The load handler 0x4391E0 enables the collector at a race set-up; whether a
        // restart runs it is not established (§5). The port enables it, or a terminal damage
        // would silence the speech for every later restart.
        self.enabled = true;
    }

    /// A flashback (0x453000, 0x454910): 0x452520(0), then `usedFlashback`.
    pub fn flashback(&mut self) {
        self.reset(false);
        self.flashback = true;
    }

    /// 0x452520(`grid`).
    fn reset(&mut self, grid: bool) {
        let was_racing = self.in_race;
        for slot in &mut self.slots {
            slot.collision = false;
            slot.terminal = false;
            slot.damage = 0.;
            slot.events = [false; 5];
        }
        self.mate_damage = 0.;
        self.mate_collision = false;
        for k in &mut self.key_slots {
            k.collision = false;
            k.terminal = false;
            k.damage = 0.;
        }
        self.mate_events = [false; 5];
        self.keys_out = 0;
        self.key_team = 0;
        self.no_collision = 0.;
        self.off_track_time = 0.;
        self.clear_events();
        // 0x468B30 and 0x417E40: the race block and the records.
        self.in_race = false;
        self.end_race = false;
        self.clock = 0.;
        self.cars = 0;
        self.kind = Kind::default();
        self.pace = 0.;
        self.track_laps = 0;
        self.running = 0;
        self.player = Record::default();
        self.plus1 = Default::default();
        self.minus1 = Default::default();
        self.leader = Default::default();
        self.ahead = Default::default();
        self.behind = Default::default();
        self.teammate = None;
        self.mate = Record::default();
        self.teammate_number = -1;
        if was_racing {
            self.on_grid = grid;
        }
        if !grid {
            self.enabled = true;
        }
        self.wrong_way = 0.;
        self.lost = Lost::default();
        if !grid {
            return;
        }
        self.one_two = -1;
        self.body = 0.;
        self.sides = [0.; 4];
        self.best = 20;
        self.total_damage = 0.;
        self.last_accident = 0.;
        self.lap_damage = 0.;
        self.prev_lap_damage = 0.;
        self.seen_lap = 1.;
        self.restarted = true;
        self.flashback = false;
        for slot in &mut self.slots {
            slot.distance = 0.;
            slot.total = 0.;
        }
        self.mate_total = 0.;
        for k in &mut self.key_slots {
            k.total = 0.;
        }
    }

    /// 0x417ED0, after every refresh: the one-shot flags.
    fn clear_events(&mut self) {
        self.car_to_car = false;
        self.hit_car = false;
        self.overtook = 0;
        self.ai_spin = false;
        self.ai_roll = false;
        self.hit_mate = false;
        self.mate_hit_car = false;
        self.mate_hit_player = false;
        self.coll_key = -1;
        self.key_slots[0].collision = false;
        self.moved = false;
    }

    /// Entry `i`'s key driver number, taken once (entry +0x50/+0x54 through a record, vf 0x4C).
    fn key_of(&mut self, i: usize, v: &RaceView) -> i32 {
        if self.keys.len() < v.entries.len() {
            self.keys.resize(v.entries.len(), None);
        }
        match (self.keys.get_mut(i), v.entries.get(i)) {
            (Some(k), Some(e)) => *k.get_or_insert(e.key),
            _ => 0,
        }
    }

    /// `KeynCollision` for a key driver n above 0, and `keyTeamInRace` = n (0x430F50).
    fn key_collision(&mut self, n: i32) {
        if n > 0 {
            self.key_team = n;
            if let Some(k) = self.key_slots.get_mut(n as usize) {
                k.collision = true;
            }
        }
    }

    /// The Paused mix mode, or no player car: 0x417ED0 and 0x417E40 once.
    pub fn pause(&mut self) {
        if self.paused {
            return;
        }
        self.clear_events();
        self.player = Record::default();
        self.plus1 = Default::default();
        self.minus1 = Default::default();
        self.leader = Default::default();
        self.ahead = Default::default();
        self.behind = Default::default();
        self.teammate = None;
        self.mate = Record::default();
        self.paused = true;
    }

    /// One collector frame and the refresh after it (0x44C640); returns the parameters.
    pub fn update(&mut self, v: &RaceView, dt: f32) -> &Params {
        // TerminalDamageMessage (0x41BC70 → 0x417C10): the entry's terminal flag. The first
        // message of an entry whose key is not yet taken sets `keyTeamInRace` to its team.
        for &(car, status) in &v.status_changes {
            if let Some(slot) = self.slots.get_mut(car) {
                slot.terminal = status != 7;
            }
            if self.keys.get(car).copied().flatten().is_none() {
                self.key_team = v.entries.get(car).map_or(0, |e| e.team);
            }
            let n = self.key_of(car, v);
            if n > 0
                && let Some(k) = self.key_slots.get_mut(n as usize)
            {
                k.terminal = status != 7;
                self.keys_out += if status != 7 { 1 } else { -1 };
            }
        }
        // AIChoreographerEvent (0x4457F0 → 0x417CB0): the type picks the slot and the
        // message's flag sets or clears it. `aiSpin` and `aiRoll` are set either way.
        for e in &v.ai_events {
            let k = match e.kind {
                1 => 1,
                2 => 0,
                3 | 4 => {
                    self.ai_spin = true;
                    2
                }
                5 => {
                    self.ai_roll = true;
                    3
                }
                6 => 4,
                _ => continue,
            };
            if self.teammate == Some(e.car) {
                self.mate_events[k] = e.on;
            } else if let Some(slot) = self.slots.get_mut(e.car) {
                slot.events[k] = e.on;
            }
        }
        if self.enabled {
            self.frame(v, dt);
        }
        self.publish();
        self.clear_events();
        &self.params
    }

    /// The update 0x452810.
    fn frame(&mut self, v: &RaceView, dt: f32) {
        self.paused = false;
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
            self.track_laps = v.track_laps;
            self.teammate_number = v.teammate_number;
            if v.started {
                self.block(v);
            }
        }
        let previous_position = self.player.position;
        let Some(&me) = v.entries.get(v.player) else { return };
        // 0x430F50 for this frame's contacts (they arrive before the update).
        for c in &v.contacts {
            // The other car's key driver.
            let other_key = c.other.map(|o| self.key_of(o, v));
            self.key_collision(other_key.unwrap_or(0));
            self.car_to_car = c.other.is_some();
            if c.car == v.player {
                self.collision = true;
                if let Some(o) = c.other {
                    self.hit_car = true;
                    // Against another car the original clears `AITermUserImpact`, always 0.
                    if self.teammate == Some(o) && !self.mate.terminal {
                        self.hit_mate = true;
                    }
                    if let Some(n) = other_key.filter(|&n| n > 0) {
                        self.coll_key = n;
                    }
                }
            } else if self.teammate == Some(c.car) {
                self.mate_collision = true;
                if let Some(o) = c.other {
                    self.mate_hit_car = true;
                    self.mate_hit_player |= o == v.player;
                }
            } else {
                // The car hit's own key driver (+0xB730, the other car's driver, has no getter).
                let n = self.key_of(c.car, v);
                self.key_collision(n);
            }
            // `AI(i+1)Collision` for an entry index above 0 (0x4311E7: entry 0 never).
            if c.car > 0
                && let Some(slot) = self.slots.get_mut(c.car)
            {
                slot.collision = true;
            }
        }
        // 0x417D60: the part callback.
        for &id in &v.parts_lost {
            match id {
                10 => self.lost.bonnet = true,
                12..=17 => self.lost.bumper = true,
                22..=25 => self.lost.door = true,
                84 => self.lost.rear_wing = true,
                85 => self.lost.right_wing = true,
                86 => self.lost.left_wing = true,
                87 => self.lost.front_wing = true,
                _ => {}
            }
        }
        // 0x917190: the track order follows the arcs every frame.
        if self.track.len() != v.entries.len() {
            self.track = (0..v.entries.len()).collect();
        }
        for i in 1..self.track.len() {
            let mut j = i;
            while j > 0 && v.entries[self.track[j - 1]].arc > v.entries[self.track[j]].arc {
                self.track.swap(j - 1, j);
                j -= 1;
            }
        }
        let place = self.track.iter().position(|&k| k == v.player);
        self.player.refresh(&me);
        self.player.track = place.map_or(0, |p| p as i32);
        self.player.off_track = v.off_track;
        self.car = v.car;
        self.best = self.best.min(me.position as u32);
        // 0x452997: the teammate's entry when unset, then its record every frame.
        if self.teammate.is_none() {
            self.teammate = v.teammate;
        }
        if let Some(t) = self.teammate
            && let Some(e) = v.entries.get(t)
        {
            self.mate.refresh(e);
            self.key_of(t, v);
        }
        if !self.in_race && self.cars > 0 {
            self.open_cockpit = v.open_cockpit;
            self.player_id = v.player as u32 + 1;
            if let Some(t) = self.teammate {
                self.teammate_id = t as u32 + 1;
            }
            // 0x452A42: integer division before × 100.
            self.start_percent = ((self.player.grid + 1) / self.cars) as f32 * 100.;
        }
        let at = |position: usize| v.entries.iter().position(|e| e.position == position);
        // A record's first refresh takes its entry's key (0x438C10).
        let mut keys = vec![v.player];
        let mut follow = |slot: &mut (usize, usize, Record), car: Option<usize>| match car {
            Some(k) => {
                if slot.0 != k + 1 {
                    slot.2 = Record::default();
                }
                slot.0 = k + 1;
                slot.2.refresh(&v.entries[k]);
                keys.push(k);
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
                // 3: the next and previous entries of the track order (vf 0x48, 0x4456F0:
                // 0x46D260 and 0x46D320 without wrapping).
                _ => {
                    let ahead = place.and_then(|p| self.track.get(p + 1)).copied();
                    let behind = place.and_then(|p| p.checked_sub(1)).map(|p| self.track[p]);
                    for (slot, car) in [(&mut self.ahead, ahead), (&mut self.behind, behind)] {
                        if slot.0 != car.map_or(0, |k| k + 1) {
                            slot.1 = Record::default();
                        }
                        slot.0 = car.map_or(0, |k| k + 1);
                        if let Some(k) = car {
                            slot.1.refresh(&v.entries[k]);
                            keys.push(k);
                        }
                    }
                }
            }
        }
        for k in keys {
            self.key_of(k, v);
        }
        // 0x452C1E: the damage object is copied on even frames.
        if self.counter.is_multiple_of(2) {
            self.copy = v.car;
        }
        // 0x452C76: overtakes. The reference follows the place every 60th frame (0x4692B0).
        let place = me.position as u32;
        if !self.in_race {
            self.reference = place;
        } else {
            if place < self.reference {
                self.overtook = self.reference - place;
                if self.overtook >= 2 {
                    self.reference = place;
                }
            }
            if self.counter.is_multiple_of(60) {
                self.reference = place;
            }
        }
        // 0x452CBD: `playerMovedTrackPos` when the track place passes the reference, which
        // follows every 60th frame (in and out of the race).
        if self.player.track > self.track_reference {
            self.moved = true;
        }
        if self.counter.is_multiple_of(60) {
            self.track_reference = self.player.track;
        }
        // 0x431200 and 0x424780 on this frame's car.
        if self.in_race && !self.end_race && self.cars > 0 {
            self.car_frame((self.counter % self.cars) as usize, v);
        }
        // 0x452D11: the progress change; `satOnTrackTime` while racing without progress.
        let progress_change = self.player.progress - self.last_progress;
        self.last_progress = self.player.progress;
        self.sat = if self.in_race && progress_change < 1e-5 { self.sat + dt } else { 0. };
        // Times in position (0x452D7D…): + dt while unchanged, else 0.
        self.player.in_position = if self.player.position == previous_position { self.player.in_position + dt } else { 0. };
        // 0x452DAD: `tmTimeInCurrentPos` against the teammate's position at the last frame.
        self.mate.in_position = if self.mate.position == self.mate_previous { self.mate.in_position + dt } else { 0. };
        self.mate_previous = self.mate.position;
        for slot in [&mut self.plus1, &mut self.minus1] {
            slot.2.in_position = if slot.0 != 0 && slot.0 == slot.1 { slot.2.in_position + dt } else { 0. };
            slot.1 = slot.0;
        }
        self.leader.2.in_position = if self.leader.0 == self.leader.1 { self.leader.2.in_position + dt } else { 0. };
        self.leader.1 = self.leader.0;
        // 0x452ED9…: slipstream (air density at most 0.6), wrong way, no collision, off track.
        self.slipstream = if v.car.density <= 0.6 { self.slipstream + dt } else { 0. };
        self.wrong_way = if v.wrong_way { self.wrong_way + dt } else { 0. };
        self.no_collision = if self.collision { 0. } else { self.no_collision + dt };
        self.off_track_time = if self.player.off_track { self.off_track_time + dt } else { 0. };
        // Terminal damage disables the collector.
        if self.player.terminal {
            self.enabled = false;
        }
    }

    /// 0x431200 then 0x424780 for entry `i`.
    fn car_frame(&mut self, i: usize, v: &RaceView) {
        let Some(e) = v.entries.get(i) else { return };
        if let Some(slot) = self.slots.get_mut(i) {
            slot.distance = e.progress.rem_euclid(1.) as f32;
            slot.terminal = e.terminal;
            // [I] `AInForcedRetire` from the race state +0x2E64 == 2: its writer is not traced;
            // the disqualification message sets it (0x417CB0).
        }
        // 0x424650: the player's formula (with the four sides) for the player's car.
        let total = if i == v.player { v.car.total() } else { e.damage };
        if i == v.player {
            self.sides = v.car.sides.map(|s| s * 100.);
            self.body = self.sides.iter().fold(self.sides[3], |m, &s| if s > m { s } else { m });
            if self.collision {
                let change = (total - self.total_damage).abs();
                self.total_damage = total;
                self.lap_damage += change;
                self.last_accident = change;
                if self.last_accident < 1. {
                    self.collision = false;
                }
                return;
            }
        }
        // The teammate while `teammateCollision` is set.
        if self.teammate == Some(i) && self.mate_collision {
            self.mate_damage = (total - self.mate_total).abs();
            self.mate_total = total;
            if self.mate_damage < 1. {
                self.mate_collision = false;
            }
            return;
        }
        // The slot's collision damage, and a key driver's whose number the entry has taken.
        if let Some(slot) = self.slots.get_mut(i)
            && slot.collision
        {
            if let Some(Some(n)) = self.keys.get(i).copied()
                && n > 0
                && let Some(k) = self.key_slots.get_mut(n as usize)
            {
                k.damage = (total - k.total).abs();
                k.total = total;
                k.collision = false;
            }
            slot.damage = (total - slot.total).abs();
            slot.total = total;
            if slot.damage < 1. {
                slot.collision = false;
            }
        }
    }

    /// The race block's pace time and running cars (0x4521C4…), while the race runs.
    fn block(&mut self, v: &RaceView) {
        let (mut sum, mut count, mut running) = (0f32, 0u32, 0u32);
        for e in v.entries.iter().filter(|e| e.ai && !e.terminal) {
            if e.progress > 0. && e.speed * MPH > 10. {
                sum += (e.time / e.progress) as f32;
                count += 1;
            }
            running += 1;
        }
        self.pace = if count > 0 { sum / count as f32 } else { 0. };
        self.running = running + 1;
    }

    /// The refresh 0x44C3C0: every getter into the values, in the map's order (ascending name
    /// address), so `damageTakenInPrevLap` (0xCB6FCC) is read before `playerCrossedSFLine`
    /// (0xCB7004) moves the lap's damage.
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
        p.flag(IsDriftFreeStyle, k.drift_freestyle);
        p.flag(IsDestructionDerby, k.derby);
        // Part 2.
        let c = &self.copy;
        p.flag(TerminalDam, self.player.terminal);
        p.set(BodyDam, self.body);
        p.set(FrontDam, self.sides[0]);
        p.set(RearDam, self.sides[1]);
        p.set(RightDam, self.sides[2]);
        p.set(LeftDam, self.sides[3]);
        p.set(EngDam, c.components[3] * 100.);
        p.set(RadiatorDam, c.components[2] * 100.);
        p.set(GearboxDam, c.components[5] * 100.);
        p.set(TyreDamage, (c.wheels[3] + c.wheels[2] + c.wheels[1] + c.wheels[0]) * 25.);
        p.set(FrontTyreDamage, (c.wheels[3] + c.wheels[2]) * 50.);
        p.set(RearTyreDam, (c.wheels[1] + c.wheels[0]) * 50.);
        p.set(FrontRightTyreDam, c.wheels[3] * 100.);
        p.set(FrontLeftTyreDam, c.wheels[2] * 100.);
        p.set(RearRightTyreDam, c.wheels[1] * 100.);
        p.set(RearLeftTyreDam, c.wheels[0] * 100.);
        p.set(DamLastAccident, self.last_accident);
        p.flag(LossDoor, self.lost.door);
        p.flag(LossBumper, self.lost.bumper);
        p.flag(LossFrontWing, self.lost.front_wing);
        p.flag(LossRearWing, self.lost.rear_wing);
        p.flag(LossLeftWing, self.lost.left_wing);
        p.flag(LossRightWing, self.lost.right_wing);
        p.flag(LooseBonnet, self.lost.bonnet);
        p.flag(IsPlayerCarOpenCockpit, self.open_cockpit);
        p.flag(HitAnotherCar, self.hit_car);
        p.flag(CarToCarCollision, self.car_to_car);
        p.flag(DroveOffTrack, self.player.off_track);
        p.set(TimeOffTrack, self.off_track_time);
        p.set(WrongWayTime, self.wrong_way);
        p.set(SatOnTrackTime, self.sat);
        p.set(SlipStreamTime, self.slipstream);
        p.set(OneTwoFinish, self.one_two as f32);
        p.set(CurrentTrackLaps, self.track_laps as f32);
        p.set(CurrentPaceTime, self.pace);
        p.set(NumRunningCars, self.running as f32);
        p.set(PlayerPrevLapTime, self.player.last_lap);
        p.set(MaxPlayerCarSpeed, self.player.max_speed);
        p.set(PlayerCarRevRatio, self.car.engine / self.car.limit);
        p.flag(PlayerGoodStart, self.car.launch == 0);
        p.set(CarsPlayerOverTook, self.overtook as f32);
        p.flag(UsedFlashback, self.flashback);
        // Race block +0x14/+0x18 (−1 from 0x468B30) and +0x1C: the test-drive targets, set only
        // by an objective (career).
        p.set(TargetTime, -1.);
        p.set(TargetSpeed, -1.);
        // Step 3.
        p.flag(AiSpin, self.ai_spin);
        p.flag(AiRoll, self.ai_roll);
        p.flag(IsDrift, k.drift);
        p.set(PlayerVehicleId, self.player_id as f32);
        // Part 3: the cars ahead and behind on the track. `carAheadTerminal` is 0 only for a
        // racing car that is not terminally damaged, so a cleared record reads 1 (0x418A60).
        let out = |r: &Record| if !r.terminal && r.racing { 0. } else { 1. };
        p.set(CarAheadLap, self.ahead.1.lap);
        p.set(CarBehindLap, self.behind.1.lap);
        p.set(CarAheadDistance, self.ahead.1.fraction);
        p.set(CarBehindDistance, self.behind.1.fraction);
        p.set(CarAheadTerminal, out(&self.ahead.1));
        p.set(CarBehindTerminal, out(&self.behind.1));
        p.set(CarAheadSpeed, self.ahead.1.speed);
        p.set(CarBehindSpeed, self.behind.1.speed);
        p.flag(PlayerMovedTrackPos, self.moved);
        // Step 4: key drivers, teams and the teammate. Records read −1 while cleared.
        let key = |r: &Record| r.key.unwrap_or(-1) as f32;
        let team = |r: &Record| r.team.unwrap_or(-1) as f32;
        p.set(LeadingCarKeyDriver, key(&self.leader.2));
        p.set(LeadingCarTeam, team(&self.leader.2));
        p.set(RaceWinnerTeam, team(&self.leader.2));
        p.set(CarPosMinus1KeyDriver, key(&self.minus1.2));
        p.set(TeammateVehicleId, self.teammate_id as f32);
        p.flag(TeammateCollision, self.mate_collision);
        p.set(PlayerCollWithKeyPlayer, self.coll_key as f32);
        p.set(KeyDriversOut, self.keys_out as f32);
        p.set(KeyTeamInRace, self.key_team as f32);
        p.flag(PlayerHasTeammate, self.teammate.is_some());
        p.set(TeammateNumber, self.teammate_number as f32);
        p.set(TeammatePos, self.mate.position);
        p.flag(TeammateFinished, self.mate.finished);
        p.flag(TeammateTermDam, self.mate.terminal);
        p.set(TmTimeInCurrentPos, self.mate.in_position);
        // Record +0x40, a championship value (0x670470) [I: not ported, as `driverChampWinPos`].
        p.set(TeamDriverChampWinPos, 0.);
        p.flag(PlayerCollideWithTeammate, self.hit_mate);
        p.set(TeammateTotDam, self.mate_total);
        p.set(TeamDamFromCollision, self.mate_damage);
        p.flag(TeammateEngBlown, self.mate_events[0]);
        p.flag(TeammateTyreBlown, self.mate_events[1]);
        p.flag(TeammateSpun, self.mate_events[2]);
        p.flag(TeammateRoll, self.mate_events[3]);
        for n in 1..=KEYS {
            let b = |v: bool| if v { 1. } else { 0. };
            // `Key19Collision` reads +0xB479, key 20's flag (getter 0x41B500).
            let collision = self.key_slots[if n == 19 { 20 } else { n }].collision;
            let k = &self.key_slots[n];
            p.0[key_param(n, 0)] = b(collision);
            p.0[key_param(n, 1)] = k.damage;
            p.0[key_param(n, 2)] = b(k.terminal);
        }
        for (i, slot) in self.slots.iter().enumerate() {
            let b = |v: bool| if v { 1. } else { 0. };
            let values = [
                (AiField::Collision, b(slot.collision)),
                (AiField::CollisionDam, slot.damage),
                (AiField::Distance, slot.distance),
                (AiField::TermDam, b(slot.terminal)),
                (AiField::EngBlown, b(slot.events[0])),
                (AiField::TyreBlown, b(slot.events[1])),
                (AiField::Spun, b(slot.events[2])),
                (AiField::Roll, b(slot.events[3])),
                (AiField::ForcedRetire, b(slot.events[4])),
            ];
            for (field, value) in values {
                p.0[ai_param(i, field)] = value;
            }
        }
        // 0x418500 before 0x418460.
        p.set(DamageTakenInPrevLap, self.prev_lap_damage);
        let crossed = self.seen_lap < self.player.lap;
        if crossed {
            self.prev_lap_damage = self.lap_damage;
            self.seen_lap = self.player.lap;
            self.lap_damage = 0.;
        }
        p.flag(PlayerCrossedSfLine, crossed);
    }
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
                user_data: 0.,
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

    /// Frames at 60 Hz with a voice busy for 1 s per play: (manager clock, folder, file).
    fn plays(m: &mut Manager, p: &Params, seconds: f32, exists: &dyn Fn(&Line) -> bool) -> Vec<(f32, String, String)> {
        let (mut busy_until, mut t, mut out) = (-1f32, 0f32, Vec::new());
        while t < seconds {
            if let Some(l) = m.frame(1. / 60., p, t < busy_until, false, exists).play {
                out.push((m.now, l.folder, l.file));
                busy_until = t + 1.;
            }
            t += 1. / 60.;
        }
        out
    }

    fn named(user_data: f32) -> Manager {
        let mut r = rules(2, &[("aiTyreBlow", 2, false, false)]);
        r.messages[0].name_to_say = 3;
        r.messages[0].linked = 100;
        r.messages[0].user_data = user_data;
        r.messages[0].ignore_silence = true;
        let mut m = Manager::new(std::sync::Arc::new(r), 1);
        m.set_ai_names(vec![None, Some("Chief_AI_467".into()), None]);
        m
    }

    #[test]
    fn a_player_name_plays_before_the_line() {
        // nameToSay 1 (positive) arms `WD_`: the name first, then the line once the voice is free.
        let mut r = rules(2, &[("pos1st", 2, false, false)]);
        r.messages[0].name_to_say = 1;
        r.messages[0].linked = 100;
        r.messages[0].ignore_silence = true;
        let r = std::sync::Arc::new(r);
        let mut m = Manager::new(r.clone(), 1);
        m.set_player_name(Some("M_01"));
        let out = plays(&mut m, &at(1.), 5., &|_| true);
        assert_eq!(out.len(), 2, "{out:?}");
        assert_eq!((out[0].1.as_str(), out[0].2.as_str()), ("Names_Player/pit", "Chief_WD_M_01"));
        assert_eq!(out[1].1, "04_position");
        assert!(out[1].0 - out[0].0 >= 1., "{out:?}");
        // The silence base follows the line only.
        assert!((m.silence - (out[1].0 + 2.5)).abs() < 1e-4);
        // Without an audio name, or with its file missing, only the line plays.
        let mut m = Manager::new(r.clone(), 1);
        assert_eq!(plays(&mut m, &at(1.), 5., &|_| true).len(), 1);
        let mut m = Manager::new(r, 1);
        m.set_player_name(Some("M_01"));
        assert_eq!(plays(&mut m, &at(1.), 5., &|l| l.folder != "Names_Player/pit").len(), 1);
    }

    #[test]
    fn a_named_ai_line_plays_the_name_after_it() {
        // `userData` 2 names entry 1 once the line has played and the voice is free.
        let mut m = named(2.);
        let out = plays(&mut m, &at(1.), 5., &|_| true);
        assert_eq!(out.len(), 2, "{out:?}");
        assert_eq!((out[0].1.as_str(), out[1].1.as_str(), out[1].2.as_str()), ("04_position", "Names_AI", "Chief_AI_467"));
        assert!(out[1].0 - out[0].0 >= 1., "{out:?}");
        assert!((m.silence - (out[1].0 + 2.)).abs() < 1e-4);
        // Entry 0 (`userData` 1) and an entry without a driver are never named.
        assert_eq!(plays(&mut named(1.), &at(1.), 5., &|_| true).len(), 1);
        assert_eq!(plays(&mut named(3.), &at(1.), 5., &|_| true).len(), 1);
        // A missing name file ends the playback without a name and without a new silence base.
        let mut m = named(2.);
        let out = plays(&mut m, &at(1.), 5., &|l| l.folder != "Names_AI");
        assert_eq!(out.len(), 1);
        assert!((m.silence - (out[0].0 + 2.5)).abs() > 1e-4 && !m.pending);
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
            ..Entry::default()
        }
    }

    /// A race in progress: the player 4th of 4 at `progress` laps.
    fn racing(progress: f64) -> RaceView {
        RaceView {
            started: true,
            clock: 60.,
            track_laps: 3,
            entries: vec![entry(4, 3, progress), entry(1, 0, progress + 0.1), entry(2, 1, progress + 0.05), entry(3, 2, progress + 0.02)],
            car: Car { density: 1., limit: 1., ..Car::default() },
            ..RaceView::default()
        }
    }

    #[test]
    fn overtakes_count_until_the_reference_follows() {
        let mut c = Collector::default();
        let mut v = racing(0.5);
        // On the grid the reference is the place.
        v.started = false;
        c.update(&v, 0.1);
        assert_eq!(c.reference, 4);
        v.started = true;
        // One place: counted on every refresh until the 60th-frame reference update.
        v.entries[0].position = 3;
        v.entries[3].position = 4;
        let mut counts = Vec::new();
        while c.counter < 60 {
            counts.push(c.update(&v, 0.1).get(Param::CarsPlayerOverTook));
        }
        assert!(counts.iter().all(|&n| n == 1.), "{counts:?}");
        assert_eq!(c.update(&v, 0.1).get(Param::CarsPlayerOverTook), 0.);
        // Two places at once: the reference follows at once.
        v.entries[0].position = 1;
        v.entries[1].position = 3;
        assert_eq!(c.update(&v, 0.1).get(Param::CarsPlayerOverTook), 2.);
        assert_eq!(c.update(&v, 0.1).get(Param::CarsPlayerOverTook), 0.);
    }

    #[test]
    fn lap_damage_reaches_the_getter_one_crossing_late() {
        let mut c = Collector::default();
        let mut v = racing(0.5);
        v.contacts = vec![Contact { car: 0, other: Some(1) }];
        v.car.components = [0.6; 6];
        // Up to the player's slot (counter % 4 == 0): a collision of 60 %.
        let mut p = Params::default();
        for _ in 0..4 {
            p = c.update(&v, 0.1).clone();
            v.contacts.clear();
        }
        assert!((p.get(Param::TotalDam) - 60.).abs() < 1e-3 && p.get(Param::DamLastAccident) == p.get(Param::TotalDam));
        assert_eq!(p.get(Param::NoCollisionTime), 0.);
        // The first crossing: the getter still reads the earlier lap (0); the 60 move over.
        v.entries[0].laps = 1;
        v.entries[0].last_lap = Some(70.);
        let p = c.update(&v, 0.1).clone();
        assert_eq!((p.get(Param::PlayerCrossedSfLine), p.get(Param::DamageTakenInPrevLap)), (1., 0.));
        assert_eq!(p.get(Param::PlayerPrevLapTime), 70.);
        let p = c.update(&v, 0.1).clone();
        assert_eq!(p.get(Param::PlayerCrossedSfLine), 0.);
        assert!((p.get(Param::DamageTakenInPrevLap) - 60.).abs() < 1e-3);
        // The second crossing: a clean lap.
        v.entries[0].laps = 2;
        c.update(&v, 0.1);
        assert_eq!(c.update(&v, 0.1).get(Param::DamageTakenInPrevLap), 0.);
    }

    #[test]
    fn damage_getters_read_the_copy_of_even_frames() {
        let mut c = Collector::default();
        let mut v = racing(0.5);
        v.car.components[3] = 0.25;
        v.car.wheels = [0.1, 0.2, 0.3, 0.4];
        // Frame 1 (odd): no copy yet.
        assert_eq!(c.update(&v, 0.1).get(Param::EngDam), 0.);
        let p = c.update(&v, 0.1).clone();
        assert_eq!(p.get(Param::EngDam), 25.);
        assert_eq!((p.get(Param::FrontRightTyreDam), p.get(Param::RearLeftTyreDam)), (40., 10.));
        assert!((p.get(Param::TyreDamage) - 25.).abs() < 1e-4);
        assert!((p.get(Param::FrontTyreDamage) - 35.).abs() < 1e-4);
    }

    #[test]
    fn lost_parts_set_their_flags_until_a_restart() {
        assert_eq!(["wing_bl", "wing_br", "wing_fl", "wing_fr"].map(|n| part_id(n).unwrap()), [84, 85, 86, 87]);
        let mut c = Collector::default();
        let mut v = racing(0.5);
        v.parts_lost = vec![part_id("bumper_fm").unwrap(), part_id("wing_fl").unwrap()];
        let p = c.update(&v, 0.1).clone();
        // `wing_fl` is the flag named left.
        assert_eq!((p.get(Param::LossBumper), p.get(Param::LossLeftWing), p.get(Param::LossFrontWing)), (1., 1., 0.));
        v.parts_lost.clear();
        assert_eq!(c.update(&v, 0.1).get(Param::LossBumper), 1.);
        c.restart();
        assert_eq!(c.update(&v, 0.1).get(Param::LossBumper), 0.);
    }

    #[test]
    fn terminal_damage_freezes_the_collector_until_a_restart() {
        let mut c = Collector::default();
        let mut v = racing(0.5);
        v.entries[0].terminal = true;
        let p = c.update(&v, 0.1).clone();
        assert_eq!(p.get(Param::TerminalDam), 1.);
        // Frozen: the clock no longer follows, the values still refresh.
        v.clock = 90.;
        let p = c.update(&v, 0.1).clone();
        assert_eq!((p.get(Param::TerminalDam), p.get(Param::RaceTime)), (1., 60.));
        c.restart();
        v.entries[0].terminal = false;
        assert_eq!(c.update(&v, 0.1).get(Param::TerminalDam), 0.);
    }

    #[test]
    fn pace_counts_moving_ai_cars() {
        let mut c = Collector::default();
        let mut v = racing(0.5);
        for e in &mut v.entries[1..] {
            e.ai = true;
        }
        v.entries[3].terminal = true;
        // 4 m/s is under 10 mph: running, not in the pace.
        v.entries[2].speed = 4.;
        let p = c.update(&v, 0.1).clone();
        assert_eq!(p.get(Param::CurrentPaceTime), 100.);
        assert_eq!(p.get(Param::NumRunningCars), 3.);
        assert_eq!(p.get(Param::CurrentTrackLaps), 3.);
    }

    #[test]
    fn timers_follow_their_conditions() {
        let mut c = Collector::default();
        let mut v = racing(0.5);
        v.wrong_way = true;
        v.off_track = true;
        for _ in 0..4 {
            c.update(&v, 0.25);
        }
        let p = c.params.clone();
        assert_eq!((p.get(Param::WrongWayTime), p.get(Param::TimeOffTrack)), (1., 1.));
        // No progress after the first frame.
        assert_eq!(p.get(Param::SatOnTrackTime), 0.75);
        // Density 1.0: no slipstream.
        assert_eq!(p.get(Param::SlipStreamTime), 0.);
        v.wrong_way = false;
        v.entries[0].progress = 0.6;
        let p = c.update(&v, 0.25).clone();
        assert_eq!((p.get(Param::WrongWayTime), p.get(Param::SatOnTrackTime)), (0., 0.));
    }

    #[test]
    fn a_flashback_resets_without_the_grid_flags() {
        let mut c = Collector::default();
        let v = racing(0.5);
        c.update(&v, 0.1);
        c.flashback();
        let p = c.update(&v, 0.1).clone();
        assert_eq!((p.get(Param::UsedFlashback), p.get(Param::StartAfterRestart), p.get(Param::OnStartingGrid)), (1., 0., 0.));
        c.restart();
        assert_eq!(c.update(&v, 0.1).get(Param::UsedFlashback), 0.);
    }

    #[test]
    fn numbered_families_resolve() {
        assert_eq!(param_index("AI3Spun"), ai_param(2, AiField::Spun));
        assert_eq!(param_index("AI20ForcedRetire"), ai_param(19, AiField::ForcedRetire));
        assert_eq!(param_index("AI21Spun"), 0);
        assert!(known("Key24TermDam").is_some() && known("Key25TermDam").is_none());
        assert!(known("AI1").is_none());
    }

    #[test]
    fn ai_slots_follow_collisions_and_distance() {
        let mut c = Collector::default();
        let mut v = racing(0.5);
        // Entry 2 hit by entry 1 at 20 %; entry 0 (the player) never gets an AI flag. Each
        // contact sets `carToCarCollision` to whether the other is a car, so the last one stays.
        v.contacts = vec![Contact { car: 0, other: None }, Contact { car: 2, other: Some(1) }];
        v.entries[2].damage = 20.;
        let p = c.update(&v, 0.1).clone();
        assert_eq!(p.at(ai_param(2, AiField::Collision)), 1.);
        assert_eq!(p.at(ai_param(0, AiField::Collision)), 0.);
        assert_eq!(p.get(Param::CarToCarCollision), 1.);
        v.contacts.clear();
        // Counter 2: entry 2's frame.
        let p = c.update(&v, 0.1).clone();
        assert_eq!(p.at(ai_param(2, AiField::CollisionDam)), 20.);
        assert!((p.at(ai_param(2, AiField::Distance)) - 0.55).abs() < 1e-6);
        assert_eq!(p.get(Param::CarToCarCollision), 0.);
        // Its next frame: no change, the flag clears.
        for _ in 0..4 {
            c.update(&v, 0.1);
        }
        let p = c.params.clone();
        assert_eq!(p.at(ai_param(2, AiField::Collision)), 0.);
        assert_eq!(p.at(ai_param(2, AiField::CollisionDam)), 0.);
        assert_eq!(p.get(Param::PlayerVehicleId), 0.);
    }

    #[test]
    fn choreographer_messages_set_and_clear_the_slots() {
        use crate::ai_pacing::Message;
        let mut c = Collector::default();
        let mut v = racing(0.5);
        let m = |kind, car, on| Message { kind, car, on };
        // power_spin, blow_engine, blow_tyre, retired on; brake_lock (7) has no slot.
        v.ai_events = vec![m(4, 3, true), m(2, 2, true), m(1, 1, true), m(6, 4, true), m(7, 5, true)];
        let p = c.update(&v, 0.1).clone();
        assert_eq!(p.at(ai_param(3, AiField::Spun)), 1.);
        assert_eq!(p.at(ai_param(2, AiField::EngBlown)), 1.);
        assert_eq!(p.at(ai_param(1, AiField::TyreBlown)), 1.);
        assert_eq!(p.at(ai_param(4, AiField::ForcedRetire)), 1.);
        assert_eq!((p.get(Param::AiSpin), p.get(Param::AiRoll)), (1., 0.));
        v.ai_events.clear();
        let p = c.update(&v, 0.1).clone();
        assert_eq!((p.get(Param::AiSpin), p.get(Param::AiRoll)), (0., 0.));
        assert_eq!(p.at(ai_param(3, AiField::Spun)), 1.);
        // The end of the event clears its slot and sets `aiSpin` again.
        v.ai_events = vec![m(3, 3, false)];
        let p = c.update(&v, 0.1).clone();
        assert_eq!(p.at(ai_param(3, AiField::Spun)), 0.);
        assert_eq!(p.get(Param::AiSpin), 1.);
        // A status change only sets the terminal flag (0x417C10).
        v.ai_events.clear();
        v.status_changes = vec![(2, 3)];
        let p = c.update(&v, 0.1).clone();
        assert_eq!(p.at(ai_param(2, AiField::TermDam)), 1.);
        assert_eq!(p.at(ai_param(2, AiField::Spun)), 0.);
        c.restart();
        assert_eq!(c.update(&v, 0.1).at(ai_param(1, AiField::TyreBlown)), 0.);
    }

    #[test]
    fn the_teammate_has_its_record_collisions_and_events() {
        use crate::ai_pacing::Message;
        let mut c = Collector::default();
        let mut v = racing(0.5);
        v.teammate = Some(3);
        v.teammate_number = 4;
        let p = c.update(&v, 0.1).clone();
        assert_eq!(p.get(Param::PlayerHasTeammate), 1.);
        assert_eq!((p.get(Param::TeammatePos), p.get(Param::TeammateNumber)), (3., 4.));
        assert_eq!((p.get(Param::TmTimeInCurrentPos), p.get(Param::TeammateVehicleId)), (0., 0.));
        let p = c.update(&v, 0.1).clone();
        assert!((p.get(Param::TmTimeInCurrentPos) - 0.1).abs() < 1e-6);
        // The player hits the teammate: one frame of `playerCollideWithTeammate`.
        v.contacts = vec![Contact { car: 0, other: Some(3) }];
        assert_eq!(c.update(&v, 0.1).get(Param::PlayerCollideWithTeammate), 1.);
        v.contacts.clear();
        assert_eq!(c.update(&v, 0.1).get(Param::PlayerCollideWithTeammate), 0.);
        // A car hits the teammate: `teammateCollision` until a damage update finds less than 1.
        v.contacts = vec![Contact { car: 3, other: Some(1) }];
        v.entries[3].damage = 12.;
        assert_eq!(c.update(&v, 0.1).get(Param::TeammateCollision), 1.);
        v.contacts.clear();
        let mut p = Params::default();
        for _ in 0..4 {
            p = c.update(&v, 0.1).clone();
        }
        assert_eq!((p.get(Param::TeammateTotDam), p.get(Param::TeamDamFromCollision)), (12., 12.));
        assert_eq!(p.get(Param::TeammateCollision), 1.);
        for _ in 0..4 {
            p = c.update(&v, 0.1).clone();
        }
        assert_eq!((p.get(Param::TeammateCollision), p.get(Param::TeamDamFromCollision)), (0., 0.));
        // Its choreographer events set the teammate's flags, not its AI slot.
        v.ai_events = vec![Message { kind: 4, car: 3, on: true }];
        let p = c.update(&v, 0.1).clone();
        assert_eq!((p.get(Param::TeammateSpun), p.at(ai_param(3, AiField::Spun))), (1., 0.));
        // A restart clears the teammate and its flags; the next frame takes it again.
        v.ai_events.clear();
        c.restart();
        v.started = false;
        let p = c.update(&v, 0.1).clone();
        assert_eq!((p.get(Param::PlayerHasTeammate), p.get(Param::TeammateSpun)), (1., 0.));
        assert_eq!(p.get(Param::TeammateVehicleId), 4.);
    }

    #[test]
    fn key_drivers_follow_records_collisions_and_terminal_messages() {
        let mut c = Collector::default();
        let mut v = racing(0.5);
        v.entries.push(entry(5, 4, 0.3));
        for (e, (key, team)) in v.entries.iter_mut().zip([(0, 0), (19, 7), (20, 8), (5, 9), (3, 11)]) {
            (e.key, e.team) = (key, team);
        }
        let key = |p: &Params, n, f| p.at(key_param(n, f));
        // A terminal message of an entry not yet seen sets `keyTeamInRace` to its team.
        v.status_changes = vec![(4, 5)];
        let p = c.update(&v, 0.1).clone();
        assert_eq!((p.get(Param::KeyTeamInRace), key(&p, 3, 2), p.get(Param::KeyDriversOut)), (11., 1., 1.));
        // The car ahead's record on the first frame, the leader's on the second.
        assert_eq!((p.get(Param::CarPosMinus1KeyDriver), p.get(Param::LeadingCarKeyDriver)), (5., -1.));
        v.status_changes.clear();
        let p = c.update(&v, 0.1).clone();
        assert_eq!(p.get(Param::LeadingCarKeyDriver), 19.);
        assert_eq!((p.get(Param::LeadingCarTeam), p.get(Param::RaceWinnerTeam)), (7., 7.));
        // The player hits key driver 20; `Key19Collision` reads key 20's flag (0x41B500).
        v.contacts = vec![Contact { car: 0, other: Some(2) }, Contact { car: 2, other: Some(0) }];
        v.entries[2].damage = 4.;
        let p = c.update(&v, 0.1).clone();
        assert_eq!((p.get(Param::PlayerCollWithKeyPlayer), p.get(Param::KeyTeamInRace)), (20., 20.));
        assert_eq!((key(&p, 20, 0), key(&p, 19, 0)), (1., 1.));
        v.contacts.clear();
        let mut p = c.update(&v, 0.1).clone();
        assert_eq!((p.get(Param::PlayerCollWithKeyPlayer), key(&p, 20, 0)), (-1., 1.));
        // The damage update of entry 2 measures key 20's damage and clears its flag.
        for _ in 0..5 {
            p = c.update(&v, 0.1).clone();
        }
        assert_eq!((key(&p, 20, 0), key(&p, 20, 1)), (0., 4.));
        v.status_changes = vec![(4, 7)];
        let p = c.update(&v, 0.1).clone();
        assert_eq!((key(&p, 3, 2), p.get(Param::KeyDriversOut), p.get(Param::KeyTeamInRace)), (0., 0., 20.));
        v.status_changes.clear();
        c.restart();
        let p = c.update(&v, 0.1).clone();
        assert_eq!((p.get(Param::KeyTeamInRace), key(&p, 20, 1)), (0., 0.));
    }

    #[test]
    fn the_track_order_names_the_cars_ahead_and_behind() {
        let mut c = Collector::default();
        let mut v = racing(1.5);
        // Arcs along a 1000 m line: entry 3 is just ahead of the player on the track but a lap
        // down; entry 1 leads the race but is behind on the track.
        for (e, (arc, laps)) in v.entries.iter_mut().zip([(500., 1), (100., 2), (900., 1), (560., 0)]) {
            (e.arc, e.laps, e.racing) = (arc, laps, true);
            e.progress = f64::from(laps) + f64::from(arc) / 1000.;
            e.speed = 30.;
        }
        let mut p = Params::default();
        for _ in 0..3 {
            p = c.update(&v, 0.1).clone();
        }
        // Counter 3: the ahead and behind records.
        assert_eq!((p.get(Param::CarAheadLap), p.get(Param::CarBehindLap)), (1., 3.));
        assert!((p.get(Param::CarAheadDistance) - 0.56).abs() < 1e-6);
        assert!((p.get(Param::CarBehindDistance) - 0.1).abs() < 1e-6);
        assert_eq!((p.get(Param::CarAheadTerminal), p.get(Param::CarAheadSpeed)), (0., 30. * MPH));
        // The reference starts at place 0, so place 1 counts as moved until frame 60 takes it.
        assert_eq!(p.get(Param::PlayerMovedTrackPos), 1.);
        for _ in 0..58 {
            p = c.update(&v, 0.1).clone();
        }
        assert_eq!(p.get(Param::PlayerMovedTrackPos), 0.);
        // The player passes entry 3 on the track: its place rises above the reference.
        v.entries[0].arc = 600.;
        let p = c.update(&v, 0.1).clone();
        assert_eq!(p.get(Param::PlayerMovedTrackPos), 1.);
        // Out of the race, a cleared record reads 1 for `carAheadTerminal`.
        c.restart();
        v.started = false;
        let p = c.update(&v, 0.1).clone();
        assert_eq!(p.get(Param::CarAheadTerminal), 1.);
    }

    #[test]
    fn voices_number_the_teammate() {
        assert_eq!((voice_number("us_1"), voice_number("british_1"), voice_number("irish_1")), (1, 4, 14));
        assert_eq!((voice_number("french_1"), voice_number("italian_1")), (7, 8));
        assert_eq!(voice_number("japan_1"), 0);
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

    /// The purchased rules through the collector: the leading player on lap 3 with entry 3 a lap
    /// down moving past it on the track at `lapped(t)` laps from the player, over `seconds`.
    fn lapping_lines(lapped: impl Fn(f32) -> f64, seconds: f32) -> Vec<String> {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT");
        let mut m = Manager::new(std::sync::Arc::new(Rules::load(Path::new(&root)).unwrap()), 1);
        let mut c = Collector::default();
        let mut v = racing(2.3);
        v.entries = vec![entry(1, 0, 2.3), entry(2, 1, 2.2), entry(3, 2, 2.15), entry(4, 3, 1.3)];
        let (mut t, mut busy_until, mut played) = (0f32, -1f32, Vec::new());
        while t < seconds {
            v.clock = 60. + f64::from(t);
            v.entries[3].progress = 1.3 + lapped(t);
            for e in &mut v.entries {
                (e.arc, e.racing) = ((e.progress.fract() * 1000.) as f32, true);
            }
            let p = c.update(&v, 1. / 60.).clone();
            if let Some(l) = m.frame(1. / 60., &p, t < busy_until, false, &|_| true).play {
                played.push(l.message);
                busy_until = t + 2.;
            }
            t += 1. / 60.;
        }
        played
    }

    #[test]
    #[ignore = "requires the purchased GRID installation (GRID_GAME_ROOT)"]
    fn purchased_rules_play_the_lapping_lines() {
        // 0.07 laps ahead on the track: `dontWorryCarAhead`.
        let played = lapping_lines(|_| 0.07, 60.);
        assert!(played.iter().any(|m| m == "dontWorryCarAhead"), "{played:?}");
        // Passed every 10 s from 0.02 ahead to 0.07 behind: `lappedDriver` while the place is
        // above the 60th-frame reference.
        let played = lapping_lines(|t| if ((t / 10.) as u32).is_multiple_of(2) { 0.02 } else { -0.07 }, 120.);
        assert!(played.iter().any(|m| m == "lappedDriver"), "{played:?}");
        assert!(!played.iter().any(|m| m == "dontWorryCarAhead"), "{played:?}");
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
        assert_eq!(slice.len(), 881);
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
