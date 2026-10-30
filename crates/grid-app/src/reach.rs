//! Gear-change and handbrake reaches of the player's driver and the
//! interior's gear lever and handbrake clips (docs/COCKPIT.md "Gear and
//! handbrake"): the per-car sets of database.bin `vehicle_interior`
//! (0x60E310 → 0x860AC0 → 0x861C50), the requests the inputs make
//! (0x85F8F0), the driver's state machine (0x851B20, 0x8573E0) with its
//! crossfades (0xACB430, 0xACB7D0), and the interior's lever, H-pattern
//! knob and handbrake (0x859FC0).
use crate::interior_anim::{Interior, Play};
use bevy::prelude::*;
use grid_formats::{database, pssg, scene};
use std::error::Error;

/// The InCarCharacters manager's constants (0x8613D0): the hand stays on
/// the lever within this long of a shift ...
const STAY_SHIFT_SECONDS: f32 = 1.0;
/// ... at more throttle than this near the next up-shift speed ...
const STAY_THROTTLE: f32 = 0.9;
/// ... or at more brake than this.
const STAY_BRAKE: f32 = 0.95;
/// "Near": within this many m/s of the gear's up-shift speed.
const STAY_SPEED_MARGIN: f32 = 9.0;
/// `<blend name="gear">` and `<blend name="handbrake">` of every
/// `int_*_anim.xml`: the reach is FK for the first 1 − blend of the clip,
/// then IK on the lever (and back).
const BLEND_GEAR: f32 = 0.6;
const BLEND_HANDBRAKE: f32 = 0.5;
/// The state crossfade's durations (`<crossfade_values name="state">`).
const STATE_FADE: f32 = 0.05;
/// The arm crossfades' durations (anim, steering, gear, handbrake), left
/// and right arm, from `int_*_anim.xml` (all 43 identical).
const ARM_FADES: [[f32; 4]; 2] = [[0.1, 0.1, 0.1, 0.1], [0.3, 0.1, 0.1, 0.1]];
/// The H-pattern knob's speed, m/s (0x859FC0).
const KNOB_SPEED: f32 = 0.6;
/// Weights at or below this add nothing (0x8521A0, 0x852FD0).
pub const MIN_WEIGHT: f32 = 0.01;

/// Arm crossfade sources (0xD74..0xD80).
pub const ANIM: usize = 0;
pub const STEER: usize = 1;
pub const GEAR: usize = 2;
pub const HANDBRAKE: usize = 3;

/// The car's `vehicle_interior` row as the driver uses it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Setup {
    /// The gear and handbrake are reached with the right arm (left-hand
    /// drive; driver +0xD18 / +0xD19).
    pub gear_right: bool,
    pub handbrake_right: bool,
    /// `small_cockpit`: the clips come from `small_cockpit_anims.pssg`
    /// (the same indices).
    pub small: bool,
    /// `h_pattern_type`: 0 sequential, 1 four, 2 five, 3 six speeds.
    pub kind: u8,
    /// First of the five gear reach clips (from neutral, left, right,
    /// to left, to right).
    pub reach: usize,
    /// The driver's change-up and change-down clips (only their lengths
    /// matter: their deltas are masked to the wheel hand's fingers).
    pub change: [usize; 2],
    /// First of the five handbrake reach clips, and the hand-on-lever clip.
    pub handbrake_reach: usize,
    pub handbrake_on: usize,
}

/// `set_01`..`set_0n` → 0..n − 1; anything else leaves the record's zero
/// (0x848AF0, 0x848B70, 0x848C00, 0x848C80).
fn set_index(value: Option<&str>, n: usize) -> usize {
    (0..n).find(|k| value == Some(format!("set_0{}", k + 1).as_str())).unwrap_or(0)
}

impl Setup {
    /// The row of the vehicles drawn from folder `stem`, or `None` when
    /// they have none.
    pub fn read(db: &database::Database, stem: &str) -> Result<Option<Self>, Box<dyn Error>> {
        let vehicles = db.table("vehicle").ok_or("database table vehicle absent")?;
        let mut id = None;
        for row in vehicles.rows().filter(|r| r.text("file_string") == Some(stem)) {
            let this = row.int("vehicle_interior_id");
            if id.is_some_and(|id| id != this) {
                return Err(format!("vehicle rows of {stem} disagree on vehicle_interior_id").into());
            }
            id = Some(this);
        }
        let Some(Some(id)) = id else { return Ok(None) };
        let table = db.table("vehicle_interior").ok_or("database table vehicle_interior absent")?;
        Ok(table.rows().find(|r| r.int("id") == Some(id)).map(Self::from_row))
    }

    fn from_row(row: database::Row) -> Self {
        // 0x84D490: left 0, right 1, mid_left 2, mid_right 3, else 0.
        let drive = match row.text("drive") {
            Some("right") => 1,
            Some("mid_left") => 2,
            Some("mid_right") => 3,
            _ => 0,
        };
        let (gear_right, handbrake_right) = (matches!(drive, 0 | 3), matches!(drive, 0 | 2));
        let kind = match row.text("h_pattern_type") {
            Some("4speed") => 1,
            Some("5speed") => 2,
            Some("6speed") => 3,
            _ => 0,
        };
        let g = set_index(row.text("gearstick_placement"), 5);
        let c = set_index(row.text("gearstick"), 4);
        let h = set_index(row.text("handbrake_placement"), 5);
        let o = set_index(row.text("handbrake_type"), 4);
        // 0x84AA90, 0x84AD10, 0x84AE20, 0x84B0A0.
        let pick = |right: bool, a: usize, b: usize, k: usize, step: usize| if right { a + k * step } else { b + k * step };
        let up = pick(gear_right, 22, 55, c, 7);
        Self {
            gear_right,
            handbrake_right,
            small: row.int("small_cockpit").unwrap_or(0) != 0,
            kind,
            reach: pick(gear_right, 17, 50, g, 7),
            change: [up, up + 1],
            handbrake_reach: pick(handbrake_right, 83, 112, h, 6),
            handbrake_on: pick(handbrake_right, 88, 117, o, 6),
        }
    }
}

/// A NeAnim clip timer (0xACA930, 0xACAC50, 0xACAB10, 0xACACE0, 0x770CA0).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Timer {
    t: f32,
    /// Its duration D.
    length: f32,
    span: f32,
    offset: f32,
    playing: bool,
}

impl Timer {
    pub fn new(length: f32) -> Self {
        Self { length, span: length, ..Default::default() }
    }

    /// offset + span · t / D (no wrap).
    pub fn clip_time(&self) -> f32 {
        if self.length > 0. { self.offset + self.span / self.length * self.t } else { self.offset }
    }

    pub fn playing(&self) -> bool {
        self.playing
    }

    /// Stops in the frame its time passes the duration.
    fn advance(&mut self, dt: f32) {
        if self.playing {
            self.t += dt;
            if self.length - self.t < 0. {
                self.playing = false;
            }
        }
    }

    fn restart(&mut self) {
        self.t = 0.;
        self.playing = true;
    }

    fn forward(&mut self) {
        if self.span < 0. {
            let offset = self.offset;
            self.offset = self.span + offset;
            self.span = offset;
        }
    }

    /// clip time = L − t.
    fn backward(&mut self) {
        if self.span >= 0. {
            let span = self.span;
            self.span = self.offset - span;
            self.offset = span;
        }
    }

    /// The interior's lever clip from `o` of its length at speed `s`
    /// (0x84CAA0: span = D · s, offset = D · o).
    fn part(&mut self, o: f32, s: f32) {
        self.span = self.length * s;
        self.offset = self.length * o;
    }
}

/// NeAnimWeightControlCrossfade with the linear interpolator: the target's
/// weight rises from where it was to 1 over the target's duration while
/// the others fall in proportion (the sum stays 1).
#[derive(Clone, Debug, PartialEq)]
pub struct Crossfade {
    pub w: Vec<f32>,
    start: Vec<f32>,
    durations: Vec<f32>,
    target: usize,
    p: f32,
}

impl Crossfade {
    /// All on `initial` (0xACB3D0).
    fn new(durations: &[f32], initial: usize) -> Self {
        let mut w = vec![0.; durations.len()];
        w[initial] = 1.;
        Self { start: w.clone(), w, durations: durations.to_vec(), target: initial, p: 1. }
    }

    /// Starts toward `target` unless it is already the target (0xACB430).
    fn go(&mut self, target: usize) {
        if self.target != target {
            self.start = self.w.clone();
            self.p = 0.;
            self.target = target;
        }
    }

    fn update(&mut self, dt: f32) {
        let d = self.durations[self.target];
        self.p = if d > 0. { ((self.p * d + dt) / d).clamp(0., 1.) } else { 1. };
        for (i, (w, s)) in self.w.iter_mut().zip(&self.start).enumerate() {
            *w = if i == self.target { s + (1. - s) * self.p } else { s * (1. - self.p) };
        }
    }
}

/// The interior's lever nodes and clips.
#[derive(Clone, Debug)]
pub struct Lever {
    /// sequential_gear_Dummy01 and the change_up / change_down clips, when
    /// the interior has the gear nodes (0x84CD40).
    pub gear: Option<(usize, [usize; 2])>,
    /// handbrake_Dummy01's HandBrake_on clip, when it has the handbrake nodes.
    pub handbrake: Option<usize>,
    /// The H-pattern gate of each gear (0..=9): its Gear_pos node's frame in
    /// the lever's parent and the change clip into it (0 up, 1 down);
    /// `None` arrives at once (0x85CC41).
    gates: [(Option<Mat4>, usize); 10],
    /// sequential_gear_Dummy01's file local transform: HPatternDummy's
    /// start (0x85C380).
    rest: Mat4,
    /// The interior clips' lengths: change up, change down, HandBrake_on.
    lengths: [f32; 3],
}

impl Lever {
    /// The lever nodes of `tree`, for the H-pattern `kind`; the gear and
    /// handbrake parts need both of their nodes (0x84CD40).
    pub fn load(tree: &pssg::Tree, interior: &Interior, kind: u8) -> Result<Self, Box<dyn Error>> {
        let has = |name: &[u8]| crate::cockpit::named(tree, b"NODE", name).is_some();
        let clips = [interior.clip("change_up"), interior.clip("change_down")];
        let gear = match (has(b"LHD_SEQ_GearStick_Dummy"), interior.node("sequential_gear_Dummy01"), clips) {
            (true, Some(n), [Some(up), Some(down)]) => Some((n, [up, down])),
            _ => None,
        };
        let handbrake = match (has(b"LHD_HandBrake_Dummy"), interior.node("handbrake_Dummy01")) {
            (true, Some(_)) => interior.clip("HandBrake_on"),
            _ => None,
        };
        let mut gates = [(None, 0); 10];
        let mut rest = Mat4::IDENTITY;
        if let Some(lever) = crate::cockpit::named(tree, b"NODE", b"sequential_gear_Dummy01") {
            rest = Mat4::from_cols_array(&scene::matrix(tree, lever)?);
            let parent = tree.nodes[lever].parent.ok_or("sequential_gear_Dummy01 without a parent")?;
            let parent = Mat4::from_cols_array(&scene::node_matrix(tree, parent)?).inverse();
            let gate = |n: u8| -> Result<Option<Mat4>, Box<dyn Error>> {
                let Some(node) = crate::cockpit::named(tree, b"NODE", format!("Gear_pos_0{n}").as_bytes()) else {
                    return Ok(None);
                };
                Ok(Some(parent * Mat4::from_cols_array(&scene::node_matrix(tree, node)?)))
            };
            // 1 up, 2 down, 3 up, 4 down in Gear_pos_01/02; then by kind.
            let mut set = |g: usize, n: u8, dir: usize| -> Result<(), Box<dyn Error>> {
                gates[g] = (gate(n)?, dir);
                Ok(())
            };
            if kind > 0 {
                set(1, 1, 0)?;
                set(2, 1, 1)?;
                set(3, 2, 0)?;
                set(4, 2, 1)?;
                set(9, 3, 1)?;
            }
            if kind > 1 {
                set(5, 3, 0)?;
            }
            if kind > 2 {
                set(6, 3, 1)?;
                set(9, 1, 1)?;
            }
        }
        let length = |c: Option<usize>| c.map_or(0., |c| interior.duration(c));
        let lengths = [length(gear.map(|g| g.1[0])), length(gear.map(|g| g.1[1])), length(handbrake)];
        Ok(Self { gear, handbrake, gates, rest, lengths })
    }
}

/// The interior's lever state (interior +0x1DC..+0x250).
#[derive(Clone, Debug, PartialEq)]
struct LeverState {
    change: [Timer; 2],
    /// The gear weight control (0x84CAA0: up → 1, 0; down → 0, 1).
    weights: [f32; 2],
    handbrake: Timer,
    /// The last lever pose written: the binding keeps it while no timer plays.
    gear_play: Vec<Play>,
    cur: i32,
    target: i32,
    moving: bool,
    playing: bool,
    half: bool,
    /// HPatternDummy's local transform.
    knob: Mat4,
}

impl LeverState {
    fn new(lever: &Lever) -> Self {
        Self {
            change: [Timer::new(lever.lengths[0]), Timer::new(lever.lengths[1])],
            weights: [0.; 2],
            handbrake: Timer::new(lever.lengths[2]),
            gear_play: Vec::new(),
            cur: 0,
            target: -1,
            moving: false,
            playing: false,
            half: true,
            knob: lever.rest,
        }
    }

    /// 0x84CAA0 / 0x84CBA0: restart both timers and play `which` from `o`
    /// of its length at speed `s`.
    fn play(&mut self, lever: &Lever, which: usize, o: f32, s: f32) {
        if lever.gear.is_none() {
            return;
        }
        for t in &mut self.change {
            t.restart();
        }
        self.change[which].part(o, s);
        self.weights = if which == 0 { [1., 0.] } else { [0., 1.] };
    }

    /// 0x84CB40: aim the knob at gear `g`.
    fn retarget(&mut self, g: i32) {
        if self.cur != g {
            self.target = g;
            if !self.playing && !self.half {
                self.moving = true;
            }
        }
    }

    /// 0x84CD10: the handbrake lever is down.
    fn handbrake_down(&self, lever: &Lever) -> bool {
        lever.handbrake.is_none() || self.handbrake.clip_time() <= 0.01
    }

    /// The interior update's lever part (0x859FC0), once per frame.
    fn update(&mut self, lever: &Lever, kind: u8, dt: f32) {
        let Some((_, clips)) = lever.gear else {
            self.update_handbrake(dt);
            return;
        };
        let gate = |g: i32| lever.gates.get(g as usize).copied().unwrap_or((None, 0));
        if kind > 0 {
            if !self.moving && self.half && self.cur != -1 && self.target != -1 {
                if !self.playing && self.target != self.cur {
                    // Leave the current gear: the second half of its clip.
                    self.play(lever, gate(self.cur).1, 0.5, 1.);
                }
            } else if self.moving && self.target != -1 {
                let mut arrived = true;
                if let Some(to) = gate(self.target).0 {
                    let (_, rot, pos) = self.knob.to_scale_rotation_translation();
                    let (_, to_rot, to_pos) = to.to_scale_rotation_translation();
                    let d = pos.distance(to_pos);
                    if d >= 0.01 {
                        // Constant speed in the parent's space; the rotation
                        // by the same fraction [I: the original's
                        // interpolation is approximated].
                        let f = (KNOB_SPEED * dt / d).clamp(0., 1.);
                        self.knob = Mat4::from_rotation_translation(rot.slerp(to_rot, f), pos.lerp(to_pos, f));
                        arrived = false;
                    }
                }
                if arrived {
                    self.cur = self.target;
                    self.target = -1;
                    // Into the gear: the first half at half speed, held.
                    self.play(lever, gate(self.cur).1, 0., 0.5);
                    self.moving = false;
                }
            }
        }
        if !self.moving {
            let mut play = Vec::new();
            for ((timer, &w), &clip) in self.change.iter_mut().zip(&self.weights).zip(&clips) {
                if w > 0. && timer.playing() {
                    self.playing = true;
                    timer.advance(dt);
                    play.push((clip, timer.clip_time(), w));
                }
            }
            if !play.is_empty() {
                self.gear_play = play;
            }
        }
        self.update_handbrake(dt);
        if !self.moving {
            for i in 0..2 {
                if self.change[i].playing() {
                    continue;
                }
                if !self.half {
                    self.half = true;
                    self.playing = false;
                    self.weights[i] = 1.;
                } else if self.target == -1 {
                    self.playing = false;
                } else {
                    self.half = false;
                    self.playing = false;
                    self.moving = true;
                    self.weights = [0., 0.];
                }
            }
        }
    }

    fn update_handbrake(&mut self, dt: f32) {
        self.handbrake.advance(dt);
    }

    /// The lever clips to sample this frame, and HPatternDummy's matrix
    /// over the rest pose (`Interior::worlds_with`).
    pub fn play_list(&self, lever: &Lever, kind: u8) -> (Vec<Play>, Option<(usize, Mat4)>) {
        let mut play = self.gear_play.clone();
        if let Some(c) = lever.handbrake {
            play.push((c, self.handbrake.clip_time(), 1.));
        }
        let knob = lever.gear.filter(|_| kind > 0).map(|(n, _)| (n, self.knob * lever.rest.inverse()));
        (play, knob)
    }
}

/// The driver's reach state (driver +0x218..+0x32C, +0xD74..+0xE58).
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    /// +0x30C: 0 steering, 1 reach for the lever, 2 back to the wheel,
    /// 3 on the lever (sequential), 4 on the lever (H-pattern), 5 reach for
    /// the handbrake, 6 back from it, 7 holding it.
    pub state: u8,
    request: u8,
    previous_gear: i32,
    up: bool,
    on_stick: bool,
    stay: bool,
    release: bool,
    releasing: bool,
    started: bool,
    pub reach: Timer,
    change: [Timer; 2],
    pub handbrake_reach: Timer,
    pub handbrake_on: Timer,
    /// Sources 0 steering, 1 gear, 2 handbrake.
    pub states: Crossfade,
    /// Left and right arm: anim, steering, gear, handbrake.
    pub arms: [Crossfade; 2],
    /// The change clips' and the hand-on-handbrake clip's weights (+0xDF0,
    /// +0xDF4, +0xDF8).
    pub blend: [f32; 3],
    lever: LeverState,
    /// The simulation tick last seen.
    tick: u64,
}

/// Everything the reaches need from the cockpit.
#[derive(Clone, Debug)]
pub struct Reach {
    pub setup: Setup,
    pub lever: Lever,
    /// The driver's reach clip lengths: gear reach, change up and down,
    /// handbrake reach, hand on handbrake.
    pub lengths: [f32; 5],
    /// The interior has the wheel and its hand dummies (+0x31).
    pub steering: bool,
}

impl Reach {
    pub fn gear(&self) -> bool {
        self.lever.gear.is_some()
    }

    pub fn handbrake(&self) -> bool {
        self.lever.handbrake.is_some()
    }

    pub fn start(&self) -> State {
        State {
            state: 0,
            request: 0,
            previous_gear: -1,
            up: false,
            on_stick: false,
            stay: false,
            release: false,
            releasing: false,
            started: false,
            reach: Timer::new(self.lengths[0]),
            change: [Timer::new(self.lengths[1]), Timer::new(self.lengths[2])],
            handbrake_reach: Timer::new(self.lengths[3]),
            handbrake_on: Timer::new(self.lengths[4]),
            states: Crossfade::new(&[STATE_FADE; 3], 0),
            arms: ARM_FADES.map(|d| Crossfade::new(&d, STEER)),
            blend: [0.; 3],
            lever: LeverState::new(&self.lever),
            tick: 0,
        }
    }
}

impl State {
    /// One frame: `dt` seconds of simulation since the last.
    pub fn update(&mut self, reach: &Reach, vehicle: &[u32], dt: f32) {
        self.feed(reach, vehicle);
        self.lever.update(&reach.lever, reach.setup.kind, dt);
        self.accept(reach);
        self.transitions(reach);
        self.aim_arms(reach);
        let [up, down] = &mut self.change;
        for t in [&mut self.reach, up, down, &mut self.handbrake_reach, &mut self.handbrake_on] {
            t.advance(dt);
        }
        self.states.update(dt);
        for arm in &mut self.arms {
            arm.update(dt);
        }
    }

    /// The input feed's requests (0x85F8F0): gear changes from the gear
    /// word, the hand staying on the lever, the handbrake byte.
    fn feed(&mut self, reach: &Reach, vehicle: &[u32]) {
        let word = |o: usize| vehicle.get(o / 4).copied().unwrap_or(0);
        let float = |o: usize| f32::from_bits(word(o));
        let byte = |o: usize| (word(o & !3) >> (8 * (o & 3))) as u8;
        let (throttle, brake) = (float(0xe1c), float(0xe20));
        if byte(0x198d) == 0 {
            let g = word(0x196c) as i32;
            let previous = self.previous_gear;
            if previous == -1 {
                self.previous_gear = g;
            } else if g != previous {
                let request = if g == 9 || (previous != 9 && previous > g) { 2 } else { 1 };
                self.previous_gear = g;
                if reach.gear() && self.request != 4 {
                    self.request = request;
                }
            }
            // 0x98F1D0: top gear when g is 8 or the next gear has no ratio.
            let top = g != 9 && (g == 8 || float(0x18f8 + 4 * g.clamp(0, 9) as usize) == 0.);
            let up_shift = float(0x191c + 4 * g.clamp(0, 9) as usize);
            self.stay = float(0x1974) < STAY_SHIFT_SECONDS
                && ((throttle > STAY_THROTTLE && !top && up_shift - float(0x1648) < STAY_SPEED_MARGIN)
                    || (brake > STAY_BRAKE && g != 9));
        }
        let held = byte(0xe38) != 0;
        if reach.handbrake() && self.request != 4 {
            if held && !matches!(self.state, 5 | 7) {
                self.request = 3;
            } else if !held && matches!(self.state, 5 | 7) {
                self.request = 4;
            }
        }
    }

    /// 0x851B20: requests not accepted stay pending, except a release.
    fn accept(&mut self, reach: &Reach) {
        match self.request {
            1 | 2 if matches!(self.state, 0 | 3) => {
                if !self.on_stick {
                    self.reach.forward();
                    self.reach.restart();
                    self.states.go(1);
                }
                self.state = 1;
                self.up = self.request == 1;
                self.started = true;
                self.request = 0;
            }
            3 if self.state == 0 && reach.handbrake() => {
                self.handbrake_reach.forward();
                self.handbrake_reach.restart();
                self.states.go(2);
                self.state = 5;
                self.release = false;
                self.releasing = false;
                self.request = 0;
            }
            4 => {
                if matches!(self.state, 5 | 7) {
                    self.release = true;
                }
                self.request = 0;
            }
            _ => {}
        }
    }

    /// 0x8573E0: one transition a frame.
    fn transitions(&mut self, reach: &Reach) {
        let (lever, kind) = (&reach.lever, reach.setup.kind);
        self.on_stick = false;
        let which = usize::from(!self.up);
        if self.state == 1 && !self.reach.playing() {
            self.state = 3;
            self.on_stick = true;
            if kind == 0 {
                self.change[which].restart();
                self.blend[..2].copy_from_slice(if self.up { &[1., 0.] } else { &[0., 1.] });
                self.lever.play(lever, which, 0., 1.);
            } else {
                self.lever.retarget(self.previous_gear);
                self.state = 4;
            }
            self.started = false;
        } else if self.state == 3 && !self.change[0].playing() && self.stay {
            self.on_stick = true;
        } else if self.state == 4 {
            let l = &self.lever;
            if !l.moving && !l.playing && l.target == -1 && !self.stay {
                for t in &mut self.change {
                    t.restart();
                }
                self.state = 2;
                self.reach.backward();
                self.reach.restart();
            } else if !l.playing {
                self.lever.retarget(self.previous_gear);
            }
        } else if self.state == 3 && !self.stay && !self.change[which].playing() {
            self.state = 2;
            self.blend[which] = 0.;
            self.reach.backward();
            self.reach.restart();
        } else if self.state == 2 && !self.reach.playing() {
            self.state = 0;
            self.states.go(0);
        } else if self.state == 5 && !self.handbrake_reach.playing() {
            self.state = 7;
            self.handbrake_on.forward();
            self.handbrake_on.restart();
            self.blend[2] = 1.;
            self.lever.handbrake.forward();
            self.lever.handbrake.restart();
        } else if self.state == 7 {
            if self.release && !self.releasing {
                self.handbrake_on.backward();
                self.handbrake_on.restart();
                self.lever.handbrake.backward();
                self.lever.handbrake.restart();
                self.release = false;
                self.releasing = true;
            } else if self.releasing && self.lever.handbrake_down(lever) {
                // The original also makes this test while the handbrake is
                // held, so at 100 fps or more the hand left the raised
                // lever at once; the port waits for the release (frame-rate
                // dependent, not ported).
                self.releasing = false;
                self.state = 6;
                self.blend[2] = 0.;
                self.handbrake_reach.backward();
                self.handbrake_reach.restart();
            }
        } else if self.state == 6 && !self.handbrake_reach.playing() {
            self.state = 0;
            self.states.go(0);
        }
    }

    /// The arm crossfades' targets after the transitions.
    fn aim_arms(&mut self, reach: &Reach) {
        let g = usize::from(reach.setup.gear_right);
        let h = usize::from(reach.setup.handbrake_right);
        let back = |t: &Timer, blend: f32| if t.span.abs() * blend > t.clip_time() && reach.steering { STEER } else { ANIM };
        let out = |t: &Timer, blend: f32, ik: usize| if t.clip_time() <= (1. - blend) * t.span { ANIM } else { ik };
        match self.state {
            3 => self.arms[g].go(GEAR),
            1 => self.arms[g].go(out(&self.reach, BLEND_GEAR, GEAR)),
            2 => self.arms[g].go(back(&self.reach, BLEND_GEAR)),
            _ => {}
        }
        match self.state {
            7 => self.arms[h].go(HANDBRAKE),
            5 => self.arms[h].go(out(&self.handbrake_reach, BLEND_HANDBRAKE, HANDBRAKE)),
            6 => self.arms[h].go(back(&self.handbrake_reach, BLEND_HANDBRAKE)),
            _ => {}
        }
        if self.state == 0 && reach.steering {
            for arm in &mut self.arms {
                arm.go(STEER);
            }
        }
    }

    /// The interior's lever clips and knob for this frame.
    pub fn lever_play(&self, reach: &Reach) -> (Vec<Play>, Option<(usize, Mat4)>) {
        self.lever.play_list(&reach.lever, reach.setup.kind)
    }
}

/// The cockpit's reach state, per frame.
#[derive(Resource, Default)]
pub struct Reaching(pub Option<State>);

/// Runs the input feed, the interior lever and the driver's state machine
/// with the simulation time since the last frame (0 while paused; a reset
/// starts over).
pub(crate) fn update(
    driving: Option<Res<crate::Driving>>,
    cockpit: Option<Res<crate::cockpit::Cockpit>>,
    mut reaching: ResMut<Reaching>,
) {
    let (Some(driving), Some(cockpit)) = (driving, cockpit) else {
        reaching.0 = None;
        return;
    };
    if cockpit.is_changed() {
        reaching.0 = None;
    }
    let Some(reach) = &cockpit.reach else {
        return;
    };
    let tick = driving.runtime.ticks;
    let state = match &mut reaching.0 {
        Some(s) if s.tick <= tick => s,
        slot => slot.insert(reach.start()),
    };
    let dt = (tick - state.tick) as f32 * grid_tools::source_vehicle::DELTA;
    state.tick = tick;
    state.update(reach, &driving.runtime.state.car.owner.vehicle, dt);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The Lacetti's sets: left-hand drive, sequential, set_01 everywhere.
    fn lacetti(kind: u8) -> Reach {
        let mut gates = [(None, 0); 10];
        if kind > 0 {
            gates[2] = (Some(Mat4::from_translation(Vec3::new(0.06, 0., 0.))), 1);
        }
        Reach {
            setup: Setup {
                gear_right: true,
                handbrake_right: true,
                small: false,
                kind,
                reach: 17,
                change: [22, 23],
                handbrake_reach: 83,
                handbrake_on: 88,
            },
            lever: Lever { gear: Some((0, [0, 1])), handbrake: Some(2), gates, rest: Mat4::IDENTITY, lengths: [0.2667, 0.2667, 0.2] },
            lengths: [0.5, 1. / 6., 1. / 6., 0.5, 1. / 3.],
            steering: true,
        }
    }

    fn vehicle(gear: u32, handbrake: bool) -> Vec<u32> {
        let mut v = vec![0; 0x1990 / 4];
        v[0x196c / 4] = gear;
        v[0xe38 / 4] = u32::from(handbrake);
        // Long after the last shift: the hand does not stay on the lever.
        v[0x1974 / 4] = 5f32.to_bits();
        v
    }

    /// Frames of 1/60 s; the states seen, deduplicated.
    fn run(state: &mut State, reach: &Reach, v: &[u32], seconds: f32, seen: &mut Vec<u8>) {
        for _ in 0..(seconds * 60.).round() as usize {
            state.update(reach, v, 1. / 60.);
            if seen.last() != Some(&state.state) {
                seen.push(state.state);
            }
        }
    }

    #[test]
    fn timers_run_forward_and_back() {
        let mut t = Timer::new(0.5);
        t.restart();
        t.advance(0.2);
        assert!((t.clip_time() - 0.2).abs() < 1e-6);
        t.backward();
        t.restart();
        t.advance(0.1);
        assert!((t.clip_time() - 0.4).abs() < 1e-6);
        t.advance(0.41);
        assert!(!t.playing());
        t.forward();
        assert_eq!((t.offset, t.span), (0., 0.5));
    }

    #[test]
    fn crossfades_move_the_weight_over_the_target_duration() {
        let mut c = Crossfade::new(&[0.1, 0.3], 0);
        c.go(1);
        c.update(0.15);
        assert!((c.w[1] - 0.5).abs() < 1e-6 && (c.w[0] - 0.5).abs() < 1e-6);
        c.update(0.2);
        assert_eq!(c.w, vec![0., 1.]);
        c.go(1);
        c.update(0.01);
        assert_eq!(c.w, vec![0., 1.]);
    }

    #[test]
    fn steering_other_is_a_partition_of_unity() {
        for s in [-1., -0.75, -0.5, -0.2, 0., 0.3, 0.5, 0.9, 1.] {
            let w = crate::driver::steering_other(s);
            assert!((w.iter().sum::<f32>() - 1.).abs() < 1e-6, "{s}: {w:?}");
        }
    }

    /// A sequential up-shift: reach (0.5 s), the lever clip with the hand
    /// on it, back to the wheel (0.5 s); the right arm on the lever IK in
    /// between and back on the wheel at the end.
    #[test]
    fn sequential_shift_reaches_the_lever_and_returns() {
        let reach = lacetti(0);
        let mut state = reach.start();
        let mut seen = vec![0];
        run(&mut state, &reach, &vehicle(1, false), 0.1, &mut seen);
        run(&mut state, &reach, &vehicle(2, false), 0.55, &mut seen);
        assert_eq!(state.state, 3, "{seen:?}");
        assert!(state.arms[1].w[GEAR] > 0.99, "{:?}", state.arms[1].w);
        assert!(state.states.w[1] > 0.99);
        assert_eq!(state.arms[0].w[STEER], 1.);
        assert!(state.lever.change[0].playing() && state.lever.gear_play[0].0 == 0);
        run(&mut state, &reach, &vehicle(2, false), 1.0, &mut seen);
        assert_eq!(seen, [0, 1, 3, 2, 0]);
        assert_eq!(state.arms[1].w[STEER], 1.);
        assert_eq!(state.states.w[0], 1.);
        // A down-shift plays change_down.
        run(&mut state, &reach, &vehicle(1, false), 0.55, &mut seen);
        assert_eq!(state.lever.gear_play[0].0, 1);
    }

    /// The handbrake: reach, pull and hold while held, then push down and
    /// return; the hand leaves only after the lever is down.
    #[test]
    fn handbrake_holds_until_released() {
        let reach = lacetti(0);
        let mut state = reach.start();
        let mut seen = vec![0];
        run(&mut state, &reach, &vehicle(1, true), 2., &mut seen);
        assert_eq!((state.state, seen.as_slice()), (7, &[0, 5, 7][..]));
        assert!(state.arms[1].w[HANDBRAKE] > 0.99);
        assert!((state.lever.handbrake.clip_time() - 0.2).abs() < 0.02);
        run(&mut state, &reach, &vehicle(1, false), 0.1, &mut seen);
        assert_eq!(state.state, 7);
        run(&mut state, &reach, &vehicle(1, false), 1., &mut seen);
        assert_eq!(seen, [0, 5, 7, 6, 0]);
        assert!(state.lever.handbrake.clip_time() <= 0.01);
    }

    /// An H-pattern shift: the lever leaves its gate, the knob slides to
    /// the new gate at 0.6 m/s, the lever goes into the gear, then the
    /// hand returns.
    #[test]
    fn h_pattern_knob_slides_to_the_gate() {
        let reach = lacetti(3);
        let mut state = reach.start();
        let mut seen = vec![0];
        run(&mut state, &reach, &vehicle(0, false), 0.1, &mut seen);
        run(&mut state, &reach, &vehicle(2, false), 0.75, &mut seen);
        assert_eq!(state.state, 4, "{seen:?}");
        run(&mut state, &reach, &vehicle(2, false), 1.5, &mut seen);
        assert_eq!(seen, [0, 1, 4, 2, 0]);
        assert_eq!((state.lever.cur, state.lever.target), (2, -1));
        let at = state.lever.knob.w_axis.truncate();
        assert!(at.distance(Vec3::new(0.06, 0., 0.)) < 0.011, "{at}");
        let (_, knob) = state.lever_play(&reach);
        assert!(knob.is_some());
    }
}
