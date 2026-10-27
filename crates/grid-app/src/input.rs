//! Original input path for every device ([research](../../../docs/PLATFORMS_AND_INPUT_RESEARCH.md)
//! sections 4 to 6): device sources feed the original action layer (per-binding
//! calibration AD9AD0/AD9A80, action value = maximum over bindings, B7C120),
//! and the human record 827D10 turns the actions into the simulation's
//! `Controls` and gear requests. One-shot gear edges go through a queue filled
//! once per frame in `PreUpdate` and drained one event per fixed tick.
use bevy::input::{
    InputSystems,
    gamepad::{
        AxisSettings, ButtonAxisSettings, Gamepad, GamepadAxis, GamepadButton, GamepadSettings,
    },
};
use bevy::prelude::*;
use grid_formats::action_map::{self, CalibrationType};
use grid_tools::source_vehicle::{
    Controls,
    assists::{Assists, Input, Shift},
};
use std::collections::VecDeque;

pub const HELP: &str = "--input SPEC: controller settings, comma-separated: device=auto|keyboard|pad|wheel (keyboard and pad: assisted steering; wheel: unassisted, like the original's input/device_type), steering_deadzone/steering_saturation, accelerate_deadzone/accelerate_saturation, brake_deadzone/brake_saturation 0..1 (original Advanced Controls overrides; default: the preset's values). The same keys go in an [input] section of the --assists-file. A gamepad uses the original XInput pad preset read from ActionMap/actionMapPreset4.xml of the game folder. Phone controls (docs/ANDROID.md): touch=auto|on|off (auto: on with a touch screen), tilt=on|off, tilt_max_angle=5..90 degrees (30), tilt_neutral=auto|-90..90 degrees, tilt_smoothing=0..1 s (0.05).";

/// The 23 in-game actions of table 0xE4DEE8, in id order, then the
/// original's menu actions used by the pause menu (preset `<!-- Menu Actions -->`)
/// and the replay actions (`<!-- Replay Actions -->`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Accelerate,
    Brake,
    Handbrake,
    SteerLeft,
    SteerRight,
    ChangeView,
    GearUp,
    GearDown,
    LookLeft,
    LookRight,
    LookUp,
    LookDown,
    LookBack,
    Pause,
    Boost,
    InstantReplay,
    Gear1,
    Gear2,
    Gear3,
    Gear4,
    Gear5,
    Gear6,
    GearReverse,
    MenuLeft,
    MenuRight,
    MenuUp,
    MenuDown,
    MenuSelect,
    MenuBack,
    MenuButton3,
    ReplayRewind,
    ReplayFastForward,
    ReplayPause,
    ReplayNextCamera,
    ReplayPrevCamera,
    ReplayNextLap,
    ReplayPreviousLap,
    ReplayExit,
    ReplayJumpIn,
}

pub const ACTIONS: usize = 39;

const ALL: [Action; ACTIONS] = {
    use Action::*;
    [
        Accelerate,
        Brake,
        Handbrake,
        SteerLeft,
        SteerRight,
        ChangeView,
        GearUp,
        GearDown,
        LookLeft,
        LookRight,
        LookUp,
        LookDown,
        LookBack,
        Pause,
        Boost,
        InstantReplay,
        Gear1,
        Gear2,
        Gear3,
        Gear4,
        Gear5,
        Gear6,
        GearReverse,
        MenuLeft,
        MenuRight,
        MenuUp,
        MenuDown,
        MenuSelect,
        MenuBack,
        MenuButton3,
        ReplayRewind,
        ReplayFastForward,
        ReplayPause,
        ReplayNextCamera,
        ReplayPrevCamera,
        ReplayNextLap,
        ReplayPreviousLap,
        ReplayExit,
        ReplayJumpIn,
    ]
};

/// `actionName` strings of the shipped presets, in id order.
const NAMES: [&str; ACTIONS] = [
    "Accelerate",
    "Brake",
    "Hand Brake",
    "Steer Left",
    "Steer Right",
    "Change View",
    "Gear Up",
    "Gear Down",
    "Look Left",
    "Look Right",
    "Look Up",
    "Look Down",
    "Look Back",
    "Pause",
    "Boost",
    "Instant Replay",
    "Gear 1",
    "Gear 2",
    "Gear 3",
    "Gear 4",
    "Gear 5",
    "Gear 6",
    "Gear Reverse",
    "Menu Left",
    "Menu Right",
    "Menu Up",
    "Menu Down",
    "Menu Select",
    "Menu Back",
    "Menu Button3",
    "Replay Rewind",
    "Replay Fast Forward",
    "Replay Pause",
    "Replay Next Camera",
    "Replay Prev Camera",
    "Replay Next Lap",
    "Replay Previous Lap",
    "Replay Exit",
    "Replay Jump In",
];

impl Action {
    pub fn from_name(name: &str) -> Option<Self> {
        NAMES.iter().position(|n| *n == name).map(|i| ALL[i])
    }
}

/// Calibration block `{type, deadZone, saturation, threshold, scale, lo, hi,
/// invert}`. All arithmetic is single precision SSE in the original.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Calibration {
    pub kind: CalibrationType,
    pub dead_zone: f32,
    pub saturation: f32,
    pub threshold: f32,
    scale: f32,
    lo: f32,
    hi: f32,
    invert: bool,
}

impl Default for Calibration {
    /// Binding constructor 8400B0: uniDirectionalPositive, dead zone 0.2,
    /// saturation 0.8, threshold 0.5; preset attributes override these.
    fn default() -> Self {
        Self::new(CalibrationType::UniPositive, 0.2, 0.8, 0.5)
    }
}

impl Calibration {
    /// AD9AD0: the active range [lo, hi] and inversion from the type.
    pub fn new(kind: CalibrationType, dead_zone: f32, saturation: f32, threshold: f32) -> Self {
        let (dz, sat) = (dead_zone, saturation);
        let (lo, hi, invert) = match kind {
            CalibrationType::UniPositive => (dz, sat, false),
            CalibrationType::UniNegative => (1. - sat, 1. - dz, true),
            CalibrationType::BiUpper => (dz * 0.5 + 0.5, sat * 0.5 + 0.5, false),
            CalibrationType::BiLower => ((1. - sat) * 0.5, (1. - dz) * 0.5, true),
        };
        Self {
            kind,
            dead_zone,
            saturation,
            threshold,
            scale: 1. / (hi - lo),
            lo,
            hi,
            invert,
        }
    }

    /// AD9A80: linear between the dead zone and the saturation, no curve.
    pub fn value(&self, raw: f32) -> f32 {
        let mut x = raw;
        if self.lo > x {
            x = self.lo;
        }
        if x > self.hi {
            x = self.hi;
        }
        let v = (x - self.lo) * self.scale;
        if self.invert { 1. - v } else { v }
    }

    /// B7C120: a binding without a live source reads raw 0 for the
    /// unidirectional types and 0.5 for the bidirectional ones.
    fn idle(&self) -> f32 {
        match self.kind {
            CalibrationType::UniPositive | CalibrationType::UniNegative => 0.,
            CalibrationType::BiUpper | CalibrationType::BiLower => 0.5,
        }
    }
}

/// A device input normalised to [0, 1], centred axes at 0.5 like the
/// original's DirectInput 0..65535 / 65536 and XInput thumbs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    Key(KeyCode),
    PadButton(GamepadButton),
    PadAxis(GamepadAxis),
    /// `unmapped` in a preset: a binding that never has a live source.
    Unmapped,
    /// AUTHORED phone controls (P11): an on-screen touch zone, 0 or 1.
    Touch(crate::touch_controls::Button),
    /// AUTHORED phone tilt as a bi-axis raw value (0.5 + 0.5 tilt).
    Tilt,
    /// The DirectInput wheel (`win_con_di_*`, force_feedback/windows.rs): an axis in record order
    /// X, Y, Z, Rx, Ry, Rz, Slider0, Slider1 (raw/65536), a button, or a POV 0 direction (up,
    /// right, down, left) read as a button.
    #[cfg_attr(not(windows), allow(dead_code))]
    WheelAxis(u8),
    #[cfg_attr(not(windows), allow(dead_code))]
    WheelButton(u8),
    #[cfg_attr(not(windows), allow(dead_code))]
    WheelPov(u8),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Binding {
    pub source: Source,
    pub calibration: Calibration,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActionMap {
    bindings: [Vec<Binding>; ACTIONS],
}

impl Default for ActionMap {
    fn default() -> Self {
        Self {
            bindings: std::array::from_fn(|_| Vec::new()),
        }
    }
}

impl ActionMap {
    pub fn bind(&mut self, action: Action, source: Source, calibration: Calibration) {
        self.bindings[action as usize].push(Binding {
            source,
            calibration,
        });
    }

    #[cfg(test)]
    pub fn bindings(&self, action: Action) -> &[Binding] {
        &self.bindings[action as usize]
    }

    /// The prototype's keyboard keys with the calibrations of the original
    /// keyboard preset 0 (steering uniPos dz 0.2 sat 1, pedals dz 0 sat 1,
    /// the rest constructor defaults). WASD are AUTHORED aliases; A steers, so
    /// Gear Up moves from A to X. C changes the view and End looks back as in
    /// the original (F349; C looked back before); Delete and PageDown look left
    /// and right (preset 0). Pause is Escape as in the
    /// original, with P kept as before. The menu keys are preset 0's: arrows
    /// and numpad 4/6/8/2, Enter, Escape, Ctrl (Button3).
    pub fn keyboard() -> Self {
        use Action::*;
        let pedal = Calibration::new(CalibrationType::UniPositive, 0., 1., 0.5);
        let steer = Calibration::new(CalibrationType::UniPositive, 0.2, 1., 0.5);
        let button = Calibration::default();
        let mut map = Self::default();
        for (action, keys, calibration) in [
            (Accelerate, &[KeyCode::ArrowUp, KeyCode::KeyW][..], pedal),
            (Brake, &[KeyCode::ArrowDown, KeyCode::KeyS], pedal),
            (Handbrake, &[KeyCode::Space], button),
            (SteerLeft, &[KeyCode::ArrowLeft, KeyCode::KeyA], steer),
            (SteerRight, &[KeyCode::ArrowRight, KeyCode::KeyD], steer),
            (GearUp, &[KeyCode::KeyX], button),
            (GearDown, &[KeyCode::KeyZ], button),
            (ChangeView, &[KeyCode::KeyC], button),
            (LookBack, &[KeyCode::End], button),
            (LookLeft, &[KeyCode::Delete], button),
            (LookRight, &[KeyCode::PageDown], button),
            (Pause, &[KeyCode::Escape, KeyCode::KeyP], button),
            (MenuLeft, &[KeyCode::ArrowLeft, KeyCode::Numpad4], button),
            (MenuRight, &[KeyCode::ArrowRight, KeyCode::Numpad6], button),
            (MenuUp, &[KeyCode::ArrowUp, KeyCode::Numpad8], button),
            (MenuDown, &[KeyCode::ArrowDown, KeyCode::Numpad2], button),
            (MenuSelect, &[KeyCode::Enter, KeyCode::NumpadEnter], button),
            (MenuBack, &[KeyCode::Escape], button),
            // Preset 0's Instant Replay and `<!-- Replay Actions -->` keys
            // (docs/REPLAY.md); Replay Jump In is Flashback
            // (docs/FLASHBACK.md). Replay Save (S) is not ported.
            (InstantReplay, &[KeyCode::Enter], button),
            (ReplayRewind, &[KeyCode::F2], button),
            (ReplayFastForward, &[KeyCode::F3], button),
            (ReplayPause, &[KeyCode::F1], button),
            (ReplayNextCamera, &[KeyCode::F8], button),
            (ReplayPrevCamera, &[KeyCode::F7], button),
            (ReplayNextLap, &[KeyCode::F5], button),
            (ReplayPreviousLap, &[KeyCode::F4], button),
            (ReplayExit, &[KeyCode::Escape], button),
            (ReplayJumpIn, &[KeyCode::F12], button),
            (
                MenuButton3,
                &[KeyCode::ControlLeft, KeyCode::ControlRight],
                button,
            ),
        ] {
            for key in keys {
                map.bind(action, Source::Key(*key), calibration);
            }
        }
        map
    }

    /// The pad bindings of a shipped preset: every `Axis` of the preset's own
    /// device for the 23 in-game actions and the menu actions. Keyboard entries in pad presets
    /// (Escape, B, Return) are left to the prototype's keyboard map. Returns
    /// the unknown axis names, which become unmapped bindings.
    pub fn pad(preset: &action_map::ActionMap) -> (Self, Vec<String>) {
        let mut map = Self::default();
        let mut unknown = Vec::new();
        for action in &preset.actions {
            let Some(id) = Action::from_name(&action.name) else {
                continue;
            };
            for axis in action
                .axes
                .iter()
                .filter(|a| a.device_name == preset.device_type)
            {
                let source = match axis.axis_name.as_str() {
                    "unmapped" => Source::Unmapped,
                    name => xinput_source(name).unwrap_or_else(|| {
                        unknown.push(name.to_owned());
                        Source::Unmapped
                    }),
                };
                let base = Calibration::default();
                map.bind(
                    id,
                    source,
                    Calibration::new(
                        axis.calibration,
                        axis.dead_zone.unwrap_or(base.dead_zone),
                        axis.saturation.unwrap_or(base.saturation),
                        axis.threshold.unwrap_or(base.threshold),
                    ),
                );
            }
        }
        (map, unknown)
    }

    /// The bindings of an original wheel preset (actionMapPreset1/2/3/5: G25, MOMO, DFP, RGT):
    /// every `Axis` of the preset's device. Returns the unknown axis names.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn wheel(preset: &action_map::ActionMap) -> (Self, Vec<String>) {
        let mut map = Self::default();
        let mut unknown = Vec::new();
        for action in &preset.actions {
            let Some(id) = Action::from_name(&action.name) else {
                continue;
            };
            for axis in action.axes.iter().filter(|a| a.device_name == preset.device_type) {
                let source = match axis.axis_name.as_str() {
                    "unmapped" => Source::Unmapped,
                    name => dinput_source(name).unwrap_or_else(|| {
                        unknown.push(name.to_owned());
                        Source::Unmapped
                    }),
                };
                let base = Calibration::default();
                map.bind(
                    id,
                    source,
                    Calibration::new(
                        axis.calibration,
                        axis.dead_zone.unwrap_or(base.dead_zone),
                        axis.saturation.unwrap_or(base.saturation),
                        axis.threshold.unwrap_or(base.threshold),
                    ),
                );
            }
        }
        (map, unknown)
    }

    /// AUTHORED wheel preset for a wheel none of the original presets names (the original has no
    /// fallback, 83A130). Laid out like the G25 preset: X steering biLower/biUpper dz 0 sat 1,
    /// separate pedals uniNegative dz 0 sat 1 (uniPositive with `wheel_pedals=normal`), Gear
    /// Up/Down, Handbrake and Change View on buttons; axes and buttons are settings. Only axes the
    /// device has are bound: an unbound uniNegative pedal would read full (B7C120's idle 0).
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn generic_wheel(w: &WheelKeys, has_axis: impl Fn(u8) -> bool) -> (Self, Vec<String>) {
        use Action::*;
        let mut map = Self::default();
        let mut missing = Vec::new();
        let full = |kind| Calibration::new(kind, 0., 1., 0.5);
        let pedal = full(if w.pedals_inverted() {
            CalibrationType::UniNegative
        } else {
            CalibrationType::UniPositive
        });
        for (action, axis, calibration) in [
            (SteerLeft, 0, full(CalibrationType::BiLower)),
            (SteerRight, 0, full(CalibrationType::BiUpper)),
            (Accelerate, w.accelerate(), pedal),
            (Brake, w.brake(), pedal),
        ] {
            if has_axis(axis) {
                map.bind(action, Source::WheelAxis(axis), calibration);
            } else {
                missing.push(format!("axis{}", WHEEL_AXES[usize::from(axis)]));
            }
        }
        let button = Calibration::default();
        for (action, b) in [
            (GearUp, w.gear_up()),
            (GearDown, w.gear_down()),
            (Handbrake, w.handbrake()),
            (ChangeView, w.view()),
        ] {
            map.bind(action, Source::WheelButton(b), button);
        }
        (map, missing)
    }

    /// AUTHORED phone controls (P11). Touch zones act as keys with the
    /// keyboard preset's calibrations (pedals uniPos dz 0 sat 1, buttons the
    /// constructor defaults), so they give exact 0/1 like the original's
    /// digital pedals. Tilt is a bi-axis source like the pad stick:
    /// biLower/biUpper with dz 0.05 and sat 1, the values 4CC630 sets for
    /// steering on controller types 1-3; the Advanced Controls overrides
    /// apply on top. Menu Back also exits a replay, like Escape.
    pub fn touch(tilt: bool) -> Self {
        use crate::touch_controls::Button as B;
        use Action::*;
        let pedal = Calibration::new(CalibrationType::UniPositive, 0., 1., 0.5);
        let button = Calibration::default();
        let mut map = Self::default();
        for (action, b, calibration) in [
            (Accelerate, B::Throttle, pedal),
            (Brake, B::Brake, pedal),
            (Handbrake, B::Handbrake, button),
            (GearUp, B::GearUp, button),
            (GearDown, B::GearDown, button),
            (Pause, B::Pause, button),
            (LookBack, B::LookBack, button),
            (ChangeView, B::ChangeView, button),
            (MenuUp, B::MenuUp, button),
            (MenuDown, B::MenuDown, button),
            (MenuLeft, B::MenuLeft, button),
            (MenuRight, B::MenuRight, button),
            (MenuSelect, B::MenuSelect, button),
            (MenuBack, B::MenuBack, button),
            (ReplayExit, B::MenuBack, button),
        ] {
            map.bind(action, Source::Touch(b), calibration);
        }
        if tilt {
            map.bind(
                SteerLeft,
                Source::Tilt,
                Calibration::new(CalibrationType::BiLower, 0.05, 1., 0.5),
            );
            map.bind(
                SteerRight,
                Source::Tilt,
                Calibration::new(CalibrationType::BiUpper, 0.05, 1., 0.5),
            );
        }
        map
    }

    pub fn extend(&mut self, other: &ActionMap) {
        for (mine, theirs) in self.bindings.iter_mut().zip(&other.bindings) {
            mine.extend_from_slice(theirs);
        }
    }

    /// 81CD30 with type 4 (keep) and threshold -1 (keep): new dead zone
    /// and/or saturation for every binding of one action, recalibrated.
    pub fn calibrate(&mut self, action: Action, tune: Tune) -> Result<(), String> {
        if tune.dead_zone.is_none() && tune.saturation.is_none() {
            return Ok(());
        }
        for binding in &mut self.bindings[action as usize] {
            let c = binding.calibration;
            let dz = tune.dead_zone.unwrap_or(c.dead_zone);
            let sat = tune.saturation.unwrap_or(c.saturation);
            if dz >= sat {
                return Err(format!(
                    "{} dead zone {dz} must be below its saturation {sat}",
                    NAMES[action as usize]
                ));
            }
            binding.calibration = Calibration::new(c.kind, dz, sat, c.threshold);
        }
        Ok(())
    }
}

/// DirectInput `win_con_di_*` names onto the wheel record.
#[cfg_attr(not(windows), allow(dead_code))]
fn dinput_source(name: &str) -> Option<Source> {
    let name = name.strip_prefix("win_con_di_")?;
    if let Some(n) = name.strip_prefix("button") {
        return n.parse::<u8>().ok().filter(|n| *n < 64).map(Source::WheelButton);
    }
    match name.strip_prefix("axis")? {
        "DPad0up" => Some(Source::WheelPov(0)),
        "DPad0right" => Some(Source::WheelPov(1)),
        "DPad0down" => Some(Source::WheelPov(2)),
        "DPad0left" => Some(Source::WheelPov(3)),
        a => wheel_axis(a).map(Source::WheelAxis),
    }
}

/// The wheel record's axes, in `win_con_di_axis*` spelling.
pub const WHEEL_AXES: [&str; 8] = ["X", "Y", "Z", "Rx", "Ry", "Rz", "Slider0", "Slider1"];

fn wheel_axis(name: &str) -> Option<u8> {
    WHEEL_AXES.iter().position(|n| n.eq_ignore_ascii_case(name)).map(|i| i as u8)
}

/// A wheel source's raw value from the device state; `None` for an axis the device lacks.
#[cfg(windows)]
fn wheel_raw(state: &crate::force_feedback::windows::WheelState, source: Source) -> Option<f32> {
    match source {
        Source::WheelAxis(i) => state.axes.get(usize::from(i)).copied().flatten(),
        Source::WheelButton(b) => Some(f32::from(u8::from(state.buttons >> b & 1 != 0))),
        Source::WheelPov(d) => {
            // Hundredths of a degree clockwise from up; the low word 0xFFFF is centred.
            let pov = state.pov[0];
            let held = pov & 0xFFFF != 0xFFFF && {
                let centre = u32::from(d) * 9000;
                let off = (pov % 36000 + 36000 - centre) % 36000;
                !(6750..=36000 - 6750).contains(&off)
            };
            Some(f32::from(u8::from(held)))
        }
        _ => None,
    }
}

/// XInput `win_con_xi_*` names onto Bevy's gamepad layout (gilrs maps known
/// pads onto the Xbox layout).
fn xinput_source(name: &str) -> Option<Source> {
    use GamepadAxis as A;
    use GamepadButton as B;
    let button = |b| Some(Source::PadButton(b));
    match name.strip_prefix("win_con_xi_")? {
        "analogLeftStickX" => Some(Source::PadAxis(A::LeftStickX)),
        "analogLeftStickY" => Some(Source::PadAxis(A::LeftStickY)),
        "analogRightStickX" => Some(Source::PadAxis(A::RightStickX)),
        "analogRightStickY" => Some(Source::PadAxis(A::RightStickY)),
        "buttonLeftTrigger" => button(B::LeftTrigger2),
        "buttonRightTrigger" => button(B::RightTrigger2),
        "buttonLeftShoulder" => button(B::LeftTrigger),
        "buttonRightShoulder" => button(B::RightTrigger),
        "buttonA" => button(B::South),
        "buttonB" => button(B::East),
        "buttonX" => button(B::West),
        "buttonY" => button(B::North),
        "buttonStart" => button(B::Start),
        "buttonBack" => button(B::Select),
        "buttonLeftStick" => button(B::LeftThumb),
        "buttonRightStick" => button(B::RightThumb),
        "buttonDPadUp" => button(B::DPadUp),
        "buttonDPadDown" => button(B::DPadDown),
        "buttonDPadLeft" => button(B::DPadLeft),
        "buttonDPadRight" => button(B::DPadRight),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct State {
    value: f32,
    down: bool,
    pressed: bool,
}

/// Action values of the current frame, latched for every fixed tick in it.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Actions {
    map: ActionMap,
    /// Which connected gamepads may drive.
    pads: Pads,
    state: [State; ACTIONS],
    /// A DirectInput wheel is bound (its sources read the device thread's state).
    wheel: bool,
    /// `wheel_trace=on`: log the wheel's axes and buttons when they change.
    #[cfg_attr(not(windows), allow(dead_code))]
    trace: bool,
}

impl Actions {
    pub fn new(map: ActionMap, pads: Pads) -> Self {
        Self {
            map,
            pads,
            state: [State::default(); ACTIONS],
            wheel: false,
            trace: false,
        }
    }

    /// Whether a DirectInput wheel drives (the device class of `device=auto`).
    pub fn wheel(&self) -> bool {
        self.wheel
    }

    /// B7C120 for every action: the value is the maximum over the bindings
    /// (starting at 0), "down" is any binding at or above its threshold, and
    /// the pressed edge is down now but not on the previous update.
    pub fn update(&mut self, raw: impl Fn(Source) -> Option<f32>) {
        for (bindings, state) in self.map.bindings.iter().zip(&mut self.state) {
            let mut value = 0f32;
            let mut down = false;
            for binding in bindings {
                let c = &binding.calibration;
                let v = c.value(raw(binding.source).unwrap_or_else(|| c.idle()));
                down |= c.threshold <= v;
                if value < v {
                    value = v;
                }
            }
            *state = State {
                value,
                down,
                pressed: down && !state.down,
            };
        }
    }

    pub fn value(&self, action: Action) -> f32 {
        self.state[action as usize].value
    }

    pub fn down(&self, action: Action) -> bool {
        self.state[action as usize].down
    }

    pub fn pressed(&self, action: Action) -> bool {
        self.state[action as usize].pressed
    }

    /// 827D10 human record: accel and brake clamped to [0, 1], handbrake
    /// above 0.5, steering s = SteerR - SteerL. The port's command is
    /// positive toward the left, so it receives SteerL - SteerR (-s).
    /// The first binding's calibration of `action` (Advanced Controls).
    pub fn calibration(&self, action: Action) -> Option<Calibration> {
        self.map.bindings[action as usize]
            .first()
            .map(|b| b.calibration)
    }

    /// [`ActionMap::calibrate`] on the live map (the pause menu's Advanced).
    pub fn calibrate(&mut self, action: Action, tune: Tune) -> Result<(), String> {
        self.map.calibrate(action, tune)
    }

    pub fn controls(&self) -> Controls {
        Controls {
            throttle: self.value(Action::Accelerate).clamp(0., 1.),
            brake: self.value(Action::Brake).clamp(0., 1.),
            steering: self.value(Action::SteerLeft) - self.value(Action::SteerRight),
            handbrake: self.value(Action::Handbrake) > 0.5,
        }
    }

    /// 827D10 gear slots of this update: Gear Up/Down pressed edges, then
    /// the last pressed of Gear 1..6 and Reverse.
    pub fn shifts(&self) -> impl Iterator<Item = Shift> + use<> {
        use Action::*;
        let up = self.pressed(GearUp).then_some(Shift::Up);
        let down = self.pressed(GearDown).then_some(Shift::Down);
        let direct = [Gear1, Gear2, Gear3, Gear4, Gear5, Gear6, GearReverse]
            .into_iter()
            .zip([1, 2, 3, 4, 5, 6, -1])
            .rfind(|(a, _)| self.pressed(*a))
            .map(|(_, g)| Shift::Gear(g));
        [up, down, direct].into_iter().flatten()
    }
}

/// One-shot gear requests of the frame, consumed one per fixed tick, so a
/// frame with two ticks shifts once and a frame with none keeps its press.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ShiftQueue(VecDeque<Shift>);

impl ShiftQueue {
    /// AUTHORED bound: presses beyond it while no tick runs are dropped.
    const CAPACITY: usize = 8;

    pub fn push(&mut self, shift: Shift) {
        if self.0.len() < Self::CAPACITY {
            self.0.push_back(shift);
        }
    }

    pub fn pop(&mut self) -> Option<Shift> {
        self.0.pop_front()
    }
}

/// Bevy gamepad value onto the original raw range: buttons and triggers
/// 0..1 directly, sticks -1..1 to 0.5 + 0.5 x. A connected pad's input that
/// has not reported yet is at rest.
///
/// Y thumbs are flipped (1 - v), so stick up reads low like a DirectInput
/// axis. The original flips them when its device flag +0x94 is set [D]; the
/// shipped pad preset binds Menu Up and Look Up to biDirectionalLower [D], so
/// the flip is taken as the default [I] (Bevy's stick Y is positive up).
fn pad_raw(pad: &Gamepad, source: Source) -> Option<f32> {
    match source {
        Source::PadButton(b) => Some(pad.get(b).unwrap_or(0.).clamp(0., 1.)),
        Source::PadAxis(a @ (GamepadAxis::LeftStickY | GamepadAxis::RightStickY)) => {
            Some(0.5 - 0.5 * pad.get(a).unwrap_or(0.))
        }
        Source::PadAxis(a) => Some(0.5 + 0.5 * pad.get(a).unwrap_or(0.)),
        _ => None,
    }
}

/// Gamepads the pad preset may read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pads {
    Off,
    /// Devices that look like gamepads (see [`recognised`]).
    Recognised,
    /// Every connected device (`device=pad`).
    Any,
}

/// Marks a connected gamepad accepted for the pad preset.
#[derive(Component)]
pub struct PlayerPad;

/// AUTHORED guard. The original binds the XInput pad preset only to XInput
/// devices that are not wheels, and has no generic fallback (83A130). gilrs
/// also lists wheels and joysticks, mapping their raw axes by index, so a
/// wheel's pedals could arrive as triggers. Bevy exposes only the name and
/// USB ids: a device is taken for a pad when its name says so and neither
/// the name nor a wheel maker's vendor id says wheel.
pub fn recognised(name: &str, vendor: Option<u16>) -> Result<(), &'static str> {
    not_a_wheel(name, vendor)?;
    let name = name.to_ascii_lowercase();
    if ["xbox", "xinput", "controller", "gamepad", "pad"]
        .iter()
        .any(|w| name.contains(w))
    {
        Ok(())
    } else {
        Err("unrecognised-name")
    }
}

/// Marks a gamepad the platform itself classified as a pad (Android's
/// SOURCE_GAMEPAD, android_pad.rs). Only the wheel half of [`recognised`]
/// applies, since pad names such as "8BitDo Pro 2" need not say "pad".
/// AUTHORED.
#[derive(Component)]
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub struct ClassifiedPad;

/// The wheel half of [`recognised`].
pub fn not_a_wheel(name: &str, vendor: Option<u16>) -> Result<(), &'static str> {
    // Fanatec, Thrustmaster, Moza [I: public USB vendor ids].
    if matches!(vendor, Some(0x0eb7 | 0x044f | 0x346e)) {
        return Err("wheel-vendor");
    }
    let name = name.to_ascii_lowercase();
    if [
        "wheel",
        "racing",
        "pedal",
        "shifter",
        "fanatec",
        "momo",
        "driving force",
        "joystick",
    ]
    .iter()
    .any(|w| name.contains(w))
    {
        return Err("wheel-or-joystick-name");
    }
    Ok(())
}

/// `PreUpdate`, after Bevy's input systems: sample every device once per
/// frame, update the actions and queue the frame's gear edges.
pub fn sample(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    pads: Query<(Entity, &Gamepad), With<PlayerPad>>,
    mobile: Option<Res<crate::touch_controls::Mobile>>,
    mut actions: ResMut<Actions>,
    mut queue: ResMut<ShiftQueue>,
    #[cfg(windows)] mut traced: Local<Option<crate::force_feedback::windows::WheelState>>,
) {
    // One player device: the first accepted connected pad.
    let pad = pads.iter().min_by_key(|(e, _)| *e).map(|(_, p)| p);
    #[cfg(windows)]
    let wheel = if actions.wheel {
        crate::force_feedback::windows::started().and_then(|h| h.shared.lock().ok().map(|s| s.state))
    } else {
        None
    };
    #[cfg(windows)]
    if actions.trace
        && let Some(state) = wheel
        && traced.is_none_or(|t| {
            t.buttons != state.buttons
                || t.pov != state.pov
                || t.axes
                    .iter()
                    .zip(&state.axes)
                    .any(|(a, b)| (a.unwrap_or(0.) - b.unwrap_or(0.)).abs() > 0.05)
        })
    {
        *traced = Some(state);
        let axes: Vec<String> = WHEEL_AXES
            .iter()
            .zip(&state.axes)
            .filter_map(|(n, v)| v.map(|v| format!("{n}={v:.3}")))
            .collect();
        let buttons: Vec<String> = (0..64)
            .filter(|b| state.buttons >> b & 1 != 0)
            .map(|b: u32| b.to_string())
            .collect();
        log_line!(
            "drive_input_wheel_trace {} buttons=[{}] pov={}",
            axes.join(" "),
            buttons.join(","),
            state.pov[0]
        );
    }
    actions.update(|source| match source {
        Source::Key(k) => keys.as_ref().map(|k2| f32::from(k2.pressed(k))),
        Source::Unmapped => None,
        Source::Touch(b) => mobile.as_ref().map(|m| f32::from(m.held.down(b))),
        Source::Tilt => mobile.as_ref().and_then(|m| m.tilt_raw()),
        #[cfg(windows)]
        Source::WheelAxis(_) | Source::WheelButton(_) | Source::WheelPov(_) => {
            wheel.as_ref().and_then(|w| wheel_raw(w, source))
        }
        #[cfg(not(windows))]
        Source::WheelAxis(_) | Source::WheelButton(_) | Source::WheelPov(_) => None,
        _ => pad.and_then(|p| pad_raw(p, source)),
    });
    for shift in actions.shifts() {
        queue.push(shift);
    }
}

/// Bevy's default stick dead zone (+-0.05), change threshold (0.01) and
/// trigger rounding (0.05/0.95) are removed so the original calibration alone
/// applies, like the original's DirectInput range without a driver dead zone.
fn passthrough() -> GamepadSettings {
    GamepadSettings {
        default_axis_settings: AxisSettings::new(-1., 0., 0., 1., 0.)
            .expect("valid pass-through axis settings"),
        default_button_axis_settings: ButtonAxisSettings {
            high: 1.,
            low: 0.,
            threshold: 0.,
        },
        ..default()
    }
}

type NewPad = (
    Entity,
    &'static Gamepad,
    Option<&'static Name>,
    &'static mut GamepadSettings,
    Has<ClassifiedPad>,
);

fn connect_pads(
    mut commands: Commands,
    actions: Res<Actions>,
    mut pads: Query<NewPad, Added<Gamepad>>,
) {
    for (entity, pad, name, mut settings, classified) in &mut pads {
        *settings = passthrough();
        let name = name.map(Name::as_str).unwrap_or("");
        let accepted = match actions.pads {
            Pads::Off => Err("device-keyboard"),
            Pads::Recognised if classified => not_a_wheel(name, pad.vendor_id()),
            Pads::Recognised => recognised(name, pad.vendor_id()),
            Pads::Any => Ok(()),
        };
        if accepted.is_ok() {
            commands.entity(entity).insert(PlayerPad);
        } else {
            commands.entity(entity).remove::<PlayerPad>();
        }
        log_line!(
            "drive_input_pad connected={entity} name={name:?} vendor={:04x?} product={:04x?} preset4={} dead_zones=original",
            pad.vendor_id(),
            pad.product_id(),
            accepted.map_or_else(|why| format!("ignored({why})"), |()| "used".into())
        );
    }
}

pub fn install(app: &mut App, actions: Actions) {
    app.insert_resource(actions)
        .init_resource::<ShiftQueue>()
        .add_systems(
            PreUpdate,
            (connect_pads, sample).chain().after(InputSystems),
        );
}

/// Advanced Controls dead zone / saturation override for one action group.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tune {
    pub dead_zone: Option<f32>,
    pub saturation: Option<f32>,
}

/// Device class: keyboard and pad get assisted steering (+E6C = 1), a wheel
/// unassisted steering and 828670 shaping (827D10 record +0x33).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Device {
    /// Keep the `--assists input=` class (default keyboard); pads are read.
    #[default]
    Auto,
    Keyboard,
    Pad,
    Wheel,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Settings {
    pub device: Device,
    pub steering: Tune,
    pub accelerate: Tune,
    pub brake: Tune,
    /// AUTHORED phone controls (P11). `None` is the default: touch on when
    /// the device has a touch screen (Android), tilt on with touch.
    pub touch: Option<bool>,
    pub tilt: Option<bool>,
    pub tilt_max_angle: Option<f32>,
    /// `auto` (`None`) captures the neutral from the first reading.
    pub tilt_neutral: Option<f32>,
    pub tilt_smoothing: Option<f32>,
    /// Force feedback and rumble (docs/FORCE_FEEDBACK.md §6): the profile's `forcefeedback`
    /// (default on) and the Advanced Controls `feedback_force`, `feedback_friction` and
    /// `feedback_effects` sliders, 0..10 (default 5).
    pub force_feedback: Option<bool>,
    pub feedback: [Option<u8>; 3],
    /// The AUTHORED generic wheel preset's axes and buttons.
    pub wheel: WheelKeys,
}

/// `[input]` keys of the AUTHORED generic wheel preset ([`ActionMap::generic_wheel`]); `None`
/// keeps the default in brackets.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WheelKeys {
    /// `wheel_accelerate` (Y), `wheel_brake` (Rz): axis indices in [`WHEEL_AXES`].
    pub accelerate: Option<u8>,
    pub brake: Option<u8>,
    /// `wheel_pedals=inverted|normal` (inverted: uniNegative, released at the top of the range).
    pub pedals_inverted: Option<bool>,
    /// `wheel_gear_up` (4), `wheel_gear_down` (5), `wheel_handbrake` (6), `wheel_view` (17):
    /// button numbers, the G25 preset's.
    pub gear_up: Option<u8>,
    pub gear_down: Option<u8>,
    pub handbrake: Option<u8>,
    pub view: Option<u8>,
    /// `wheel_trace=on`: log the wheel's axes and buttons when they change.
    pub trace: Option<bool>,
}

impl WheelKeys {
    pub fn accelerate(&self) -> u8 {
        self.accelerate.unwrap_or(1)
    }
    pub fn brake(&self) -> u8 {
        self.brake.unwrap_or(5)
    }
    pub fn pedals_inverted(&self) -> bool {
        self.pedals_inverted.unwrap_or(true)
    }
    pub fn gear_up(&self) -> u8 {
        self.gear_up.unwrap_or(4)
    }
    pub fn gear_down(&self) -> u8 {
        self.gear_down.unwrap_or(5)
    }
    pub fn handbrake(&self) -> u8 {
        self.handbrake.unwrap_or(6)
    }
    pub fn view(&self) -> u8 {
        self.view.unwrap_or(17)
    }
}

impl Settings {
    /// Same `key=value` syntax as the assists settings.
    pub fn parse(spec: &str) -> Result<Self, String> {
        let mut settings = Self::default();
        for item in spec
            .lines()
            .map(|l| l.split('#').next().unwrap_or(""))
            .flat_map(|l| l.split([',', ' ', '\t', ';']))
            .filter(|s| !s.is_empty())
        {
            let (key, value) = item
                .split_once('=')
                .ok_or_else(|| format!("input setting `{item}` needs key=value"))?;
            let (key, value) = (key.to_ascii_lowercase(), value.to_ascii_lowercase());
            let fraction = || match value.parse::<f32>() {
                Ok(v) if (0. ..=1.).contains(&v) => Ok(Some(v)),
                _ => Err(format!("input setting `{key}` needs a number 0..1")),
            };
            match key.as_str() {
                "device" | "device_type" => {
                    settings.device = match value.as_str() {
                        "auto" => Device::Auto,
                        "keyboard" => Device::Keyboard,
                        "pad" | "gamepad" => Device::Pad,
                        "wheel" => Device::Wheel,
                        _ => return Err("input device needs auto, keyboard, pad or wheel".into()),
                    }
                }
                "steering_deadzone" => settings.steering.dead_zone = fraction()?,
                "steering_saturation" => settings.steering.saturation = fraction()?,
                "accelerate_deadzone" => settings.accelerate.dead_zone = fraction()?,
                "accelerate_saturation" => settings.accelerate.saturation = fraction()?,
                "brake_deadzone" => settings.brake.dead_zone = fraction()?,
                "brake_saturation" => settings.brake.saturation = fraction()?,
                "touch" | "tilt" => {
                    let on = match value.as_str() {
                        "auto" if key == "touch" => None,
                        "on" | "true" | "1" | "yes" => Some(true),
                        "off" | "false" | "0" | "no" => Some(false),
                        _ => return Err(format!("input setting `{key}` needs on or off")),
                    };
                    if key == "touch" {
                        settings.touch = on;
                    } else {
                        settings.tilt = on;
                    }
                }
                "tilt_max_angle" | "tilt_neutral" | "tilt_smoothing" => {
                    let (range, unit) = match key.as_str() {
                        "tilt_max_angle" => (5. ..=90., "degrees 5..90"),
                        "tilt_neutral" => (-90. ..=90., "degrees -90..90 or auto"),
                        _ => (0. ..=1., "seconds 0..1"),
                    };
                    let v = match value.parse::<f32>() {
                        Ok(v) if range.contains(&v) => Some(v),
                        _ if key == "tilt_neutral" && value == "auto" => None,
                        _ => return Err(format!("input setting `{key}` needs {unit}")),
                    };
                    match key.as_str() {
                        "tilt_max_angle" => settings.tilt_max_angle = v,
                        "tilt_neutral" => settings.tilt_neutral = v,
                        _ => settings.tilt_smoothing = v,
                    }
                }
                "force_feedback" | "wheel_trace" => {
                    let on = match value.as_str() {
                        "on" | "true" | "1" | "yes" => true,
                        "off" | "false" | "0" | "no" => false,
                        _ => return Err(format!("input setting `{key}` needs on or off")),
                    };
                    if key == "force_feedback" {
                        settings.force_feedback = Some(on);
                    } else {
                        settings.wheel.trace = Some(on);
                    }
                }
                "feedback_force" | "feedback_friction" | "feedback_effects" => {
                    let v = match value.parse::<u8>() {
                        Ok(v) if v <= 10 => v,
                        _ => return Err(format!("input setting `{key}` needs 0..10")),
                    };
                    let i = ["feedback_force", "feedback_friction", "feedback_effects"]
                        .iter()
                        .position(|k| *k == key)
                        .unwrap_or(0);
                    settings.feedback[i] = Some(v);
                }
                "wheel_accelerate" | "wheel_brake" => {
                    let axis = wheel_axis(&value).ok_or_else(|| {
                        format!("input setting `{key}` needs x, y, z, rx, ry, rz, slider0 or slider1")
                    })?;
                    if key == "wheel_accelerate" {
                        settings.wheel.accelerate = Some(axis);
                    } else {
                        settings.wheel.brake = Some(axis);
                    }
                }
                "wheel_pedals" => {
                    settings.wheel.pedals_inverted = Some(match value.as_str() {
                        "inverted" => true,
                        "normal" => false,
                        _ => return Err("input setting `wheel_pedals` needs inverted or normal".into()),
                    })
                }
                "wheel_gear_up" | "wheel_gear_down" | "wheel_handbrake" | "wheel_view" => {
                    let b = match value.trim_start_matches("button").parse::<u8>() {
                        Ok(b) if b < 64 => b,
                        _ => return Err(format!("input setting `{key}` needs a button number 0..63")),
                    };
                    match key.as_str() {
                        "wheel_gear_up" => settings.wheel.gear_up = Some(b),
                        "wheel_gear_down" => settings.wheel.gear_down = Some(b),
                        "wheel_handbrake" => settings.wheel.handbrake = Some(b),
                        _ => settings.wheel.view = Some(b),
                    }
                }
                _ => return Err(format!("unknown input setting `{key}`")),
            }
        }
        Ok(settings)
    }

    /// `device=auto` with a DirectInput wheel bound: the wheel class (unassisted steering), as the
    /// original gives a wheel device.
    pub fn apply_wheel(&self, assists: &mut Assists, wheel: bool) {
        if self.device == Device::Auto && wheel {
            assists.input = Input::Wheel;
        }
    }

    /// Force feedback on (the profile's `forcefeedback`, default on).
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn force_feedback(&self) -> bool {
        self.force_feedback.unwrap_or(true)
    }

    /// The three FFB gains as the original reads its sliders: value / 10, default 5.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub fn feedback_gains(&self) -> [f32; 3] {
        self.feedback.map(|v| f32::from(v.unwrap_or(5)) / 10.)
    }

    /// The device class sets the assisted-steering choice (`Assists.input`).
    pub fn apply_class(&self, assists: &mut Assists) {
        match self.device {
            Device::Auto => {}
            Device::Keyboard | Device::Pad => assists.input = Input::Keyboard,
            Device::Wheel => assists.input = Input::Wheel,
        }
    }

    /// `device=keyboard` reads no pad, `device=pad` any connected gamepad,
    /// otherwise only recognised gamepads.
    pub fn pads(&self) -> Pads {
        match self.device {
            Device::Keyboard => Pads::Off,
            Device::Pad => Pads::Any,
            Device::Auto | Device::Wheel => Pads::Recognised,
        }
    }

    /// Whether the phone controls run: `touch=auto` follows the device's
    /// touch screen (Android's configuration; false on the desktop).
    pub fn touch_active(&self, touch_screen: bool) -> bool {
        self.touch.unwrap_or(touch_screen)
    }

    /// Tilt steering settings when tilt is on (with the touch controls).
    pub fn tilt_settings(&self) -> Option<crate::tilt::Settings> {
        let base = crate::tilt::Settings::default();
        self.tilt.unwrap_or(true).then(|| crate::tilt::Settings {
            max_angle: self.tilt_max_angle.unwrap_or(base.max_angle),
            neutral: self.tilt_neutral,
            smoothing: self.tilt_smoothing.unwrap_or(base.smoothing),
        })
    }

    /// 654C20: the Advanced Controls overrides go to every binding of
    /// Steer Left/Right, Accelerate and Brake.
    pub fn tune(&self, map: &mut ActionMap) -> Result<(), String> {
        map.calibrate(Action::SteerLeft, self.steering)?;
        map.calibrate(Action::SteerRight, self.steering)?;
        map.calibrate(Action::Accelerate, self.accelerate)?;
        map.calibrate(Action::Brake, self.brake)
    }
}

impl std::fmt::Display for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "device={}",
            match self.device {
                Device::Auto => "auto",
                Device::Keyboard => "keyboard",
                Device::Pad => "pad",
                Device::Wheel => "wheel",
            }
        )?;
        for (name, tune) in [
            ("steering", self.steering),
            ("accelerate", self.accelerate),
            ("brake", self.brake),
        ] {
            if let Some(v) = tune.dead_zone {
                write!(f, ",{name}_deadzone={v}")?;
            }
            if let Some(v) = tune.saturation {
                write!(f, ",{name}_saturation={v}")?;
            }
        }
        // Phone keys only when set, so the desktop log line is unchanged.
        let on = |v: bool| if v { "on" } else { "off" };
        if let Some(v) = self.touch {
            write!(f, ",touch={}", on(v))?;
        }
        if let Some(v) = self.tilt {
            write!(f, ",tilt={}", on(v))?;
        }
        for (name, value) in [
            ("tilt_max_angle", self.tilt_max_angle),
            ("tilt_neutral", self.tilt_neutral),
            ("tilt_smoothing", self.tilt_smoothing),
        ] {
            if let Some(v) = value {
                write!(f, ",{name}={v}")?;
            }
        }
        // Force feedback and wheel keys only when set, like the phone keys.
        if let Some(v) = self.force_feedback {
            write!(f, ",force_feedback={}", on(v))?;
        }
        for (name, value) in ["feedback_force", "feedback_friction", "feedback_effects"]
            .iter()
            .zip(self.feedback)
        {
            if let Some(v) = value {
                write!(f, ",{name}={v}")?;
            }
        }
        let w = &self.wheel;
        for (name, axis) in [("wheel_accelerate", w.accelerate), ("wheel_brake", w.brake)] {
            if let Some(a) = axis {
                write!(f, ",{name}={}", WHEEL_AXES[usize::from(a)].to_ascii_lowercase())?;
            }
        }
        if let Some(v) = w.pedals_inverted {
            write!(f, ",wheel_pedals={}", if v { "inverted" } else { "normal" })?;
        }
        for (name, button) in [
            ("wheel_gear_up", w.gear_up),
            ("wheel_gear_down", w.gear_down),
            ("wheel_handbrake", w.handbrake),
            ("wheel_view", w.view),
        ] {
            if let Some(b) = button {
                write!(f, ",{name}={b}")?;
            }
        }
        if let Some(v) = w.trace {
            write!(f, ",wheel_trace={}", on(v))?;
        }
        Ok(())
    }
}

/// Keyboard map plus, unless `device=keyboard`, the pad preset from the
/// user's own installation, then the Advanced Controls overrides. A missing
/// or unreadable preset leaves the keyboard alone and is reported.
/// With `touch`, the phone controls' bindings are added too.
pub fn load(
    root: &std::path::Path,
    settings: &Settings,
    touch: bool,
) -> Result<(Actions, String), String> {
    let mut map = ActionMap::keyboard();
    if touch {
        map.extend(&ActionMap::touch(settings.tilt_settings().is_some()));
    }
    let mut pad = String::from("off");
    if settings.pads() != Pads::Off {
        let path = root.join("ActionMap").join("actionMapPreset4.xml");
        pad = match read_preset(&path) {
            Ok((pad_map, unknown)) => {
                map.extend(&pad_map);
                if unknown.is_empty() {
                    "actionMapPreset4".into()
                } else {
                    format!("actionMapPreset4 unknown_axes={}", unknown.join("|"))
                }
            }
            Err(error) => format!("unavailable({error})"),
        };
    }
    let wheel = wheel_bindings(root, settings, &mut map);
    let status = match &wheel {
        Some(w) => format!("{pad} wheel={w}"),
        None => pad,
    };
    settings.tune(&mut map)?;
    let mut actions = Actions::new(map, settings.pads());
    actions.wheel = wheel.is_some();
    actions.trace = settings.wheel.trace.unwrap_or(false);
    Ok((actions, status))
}

/// With `device=auto` or `wheel` on Windows: the DirectInput wheel's bindings, from the original
/// preset whose deviceType is the wheel's product name, else the AUTHORED generic preset.
/// Returns the log text, `None` without a wheel.
#[cfg(windows)]
fn wheel_bindings(root: &std::path::Path, settings: &Settings, map: &mut ActionMap) -> Option<String> {
    if !matches!(settings.device, Device::Auto | Device::Wheel) {
        return None;
    }
    let wheel = crate::force_feedback::windows::device().wheel.as_ref()?;
    let original = [1, 2, 3, 5].into_iter().find_map(|n| {
        let path = root.join("ActionMap").join(format!("actionMapPreset{n}.xml"));
        let text = std::fs::read_to_string(path).ok()?;
        let preset = action_map::read(&text).ok()?;
        (preset.device_type == wheel.name).then_some((n, preset))
    });
    let (wheel_map, unknown, preset) = match original {
        Some((n, preset)) => {
            let (m, unknown) = ActionMap::wheel(&preset);
            (m, unknown, format!("actionMapPreset{n}"))
        }
        None => {
            let (m, missing) = ActionMap::generic_wheel(&settings.wheel, |a| wheel.axes[usize::from(a)]);
            (m, missing, "generic".into())
        }
    };
    map.extend(&wheel_map);
    let axes: Vec<&str> = WHEEL_AXES.iter().zip(wheel.axes).filter(|(_, on)| *on).map(|(n, _)| *n).collect();
    Some(format!(
        "{:?} preset={preset} ffb={} axes={} buttons={}{}",
        wheel.name,
        wheel.force_feedback,
        axes.join("|"),
        wheel.buttons,
        if unknown.is_empty() { String::new() } else { format!(" unbound={}", unknown.join("|")) }
    ))
}

#[cfg(not(windows))]
fn wheel_bindings(_: &std::path::Path, _: &Settings, _: &mut ActionMap) -> Option<String> {
    None
}

fn read_preset(path: &std::path::Path) -> Result<(ActionMap, Vec<String>), String> {
    let size = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if size > 1 << 20 {
        return Err("preset exceeds 1 MiB".into());
    }
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let preset = action_map::read(&text).map_err(|e| e.to_string())?;
    // 83A130 matches a non-wheel XInput device to this scheme type.
    if preset.scheme_type != "xinputScheme" {
        return Err(format!("scheme {} is not xinputScheme", preset.scheme_type));
    }
    Ok(ActionMap::pad(&preset))
}

#[cfg(test)]
mod tests {
    use super::*;
    use CalibrationType::*;

    #[test]
    fn calibration_types_match_ad9ad0_and_ad9a80() {
        // uniPos [dz, sat].
        let c = Calibration::new(UniPositive, 0.2, 0.8, 0.5);
        assert_eq!((c.lo, c.hi, c.invert), (0.2, 0.8, false));
        assert_eq!(c.value(0.), 0.);
        assert_eq!(c.value(0.2), 0.);
        assert_eq!(c.value(0.5), (0.5 - 0.2) * (1. / (0.8f32 - 0.2)));
        assert_eq!(c.value(1.), 1.);
        // uniNeg [1 - sat, 1 - dz], inverted: a pedal axis reading 0 is full.
        let c = Calibration::new(UniNegative, 0.1, 0.9, 0.5);
        assert_eq!((c.lo, c.hi, c.invert), (1. - 0.9f32, 1. - 0.1f32, true));
        assert_eq!(c.value(0.), 1.);
        assert_eq!(c.value(1.), 0.);
        assert_eq!(c.value(0.5), 1. - (0.5 - c.lo) * c.scale);
        assert_eq!(c.idle(), 0.);
        // biUpper [0.5 + 0.5 dz, 0.5 + 0.5 sat].
        let c = Calibration::new(BiUpper, 0.2, 1., 0.5);
        assert_eq!((c.lo, c.hi, c.invert), (0.6, 1., false));
        assert_eq!(c.value(0.5), 0.);
        assert_eq!(c.value(0.6), 0.);
        assert_eq!(c.value(0.8), (0.8 - 0.6) * (1. / (1. - 0.6f32)));
        assert_eq!(c.value(1.), 1.);
        assert_eq!(c.value(0.), 0.);
        // biLower [0.5 (1 - sat), 0.5 (1 - dz)], inverted.
        let c = Calibration::new(BiLower, 0.2, 1., 0.5);
        assert_eq!((c.lo, c.hi, c.invert), (0., 0.4, true));
        assert_eq!(c.value(0.5), 0.);
        assert_eq!(c.value(0.4), 0.);
        assert_eq!(c.value(0.), 1.);
        assert_eq!(c.value(0.2), 1. - 0.2 * (1. / 0.4f32));
        assert_eq!(c.value(1.), 0.);
        assert_eq!(c.idle(), 0.5);
        // Constructor defaults (8400B0).
        let d = Calibration::default();
        assert_eq!(
            (d.kind, d.dead_zone, d.saturation, d.threshold),
            (UniPositive, 0.2, 0.8, 0.5)
        );
    }

    #[test]
    fn digital_keys_give_exact_zero_and_one_for_every_keyboard_calibration() {
        let map = ActionMap::keyboard();
        for action in ALL {
            for b in map.bindings(action) {
                assert_eq!(b.calibration.value(0.).to_bits(), 0f32.to_bits());
                assert_eq!(b.calibration.value(1.).to_bits(), 1f32.to_bits());
            }
        }
    }

    #[test]
    fn action_value_is_the_maximum_over_bindings() {
        let mut map = ActionMap::default();
        let stick = Calibration::new(BiLower, 0.2, 1., 0.5);
        map.bind(
            Action::SteerLeft,
            Source::PadAxis(GamepadAxis::LeftStickX),
            stick,
        );
        map.bind(
            Action::SteerLeft,
            Source::PadButton(GamepadButton::DPadLeft),
            Calibration::default(),
        );
        map.bind(
            Action::SteerLeft,
            Source::Key(KeyCode::ArrowLeft),
            Calibration::default(),
        );
        let mut actions = Actions::new(map, Pads::Any);
        let run = |actions: &mut Actions, stick: f32, dpad: f32, key: Option<f32>| {
            actions.update(|s| match s {
                Source::PadAxis(_) => Some(stick),
                Source::PadButton(_) => Some(dpad),
                Source::Key(_) => key,
                Source::Unmapped => None,
                Source::Touch(_) | Source::Tilt => None,
                Source::WheelAxis(_) | Source::WheelButton(_) | Source::WheelPov(_) => None,
            });
            (
                actions.value(Action::SteerLeft),
                actions.down(Action::SteerLeft),
            )
        };
        // Half left on the stick, nothing else: the stick wins, not a sum.
        let half = stick.value(0.2);
        assert_eq!(run(&mut actions, 0.2, 0., Some(0.)), (half, true));
        // A full button on top: max = 1, not 1 + half.
        assert_eq!(run(&mut actions, 0.2, 1., Some(0.)), (1., true));
        // No source for the key: uni idle 0 does not raise the value.
        assert_eq!(run(&mut actions, 0.5, 0., None), (0., false));
        // Slightly left, below every threshold: value but not down.
        let small = stick.value(0.35);
        assert!(small > 0. && small < 0.5);
        assert_eq!(run(&mut actions, 0.35, 0., Some(0.)), (small, false));
        // An action without bindings is 0 and never down.
        assert_eq!(actions.value(Action::Boost), 0.);
        assert!(!actions.down(Action::Boost));
    }

    #[test]
    fn human_record_follows_827d10() {
        let mut map = ActionMap::default();
        for action in [
            Action::Accelerate,
            Action::Brake,
            Action::Handbrake,
            Action::SteerLeft,
            Action::SteerRight,
        ] {
            map.bind(
                action,
                Source::PadAxis(GamepadAxis::LeftZ),
                Calibration::new(UniPositive, 0., 1., 0.5),
            );
        }
        let mut actions = Actions::new(map, Pads::Any);
        actions.update(|_| Some(0.5));
        let c = actions.controls();
        assert_eq!(
            (c.throttle, c.brake, c.steering, c.handbrake),
            (0.5, 0.5, 0., false)
        );
        actions.update(|_| Some(0.75));
        assert!(actions.controls().handbrake);
        // Steering is SteerL - SteerR: positive toward the left.
        let mut map = ActionMap::keyboard();
        map.bind(
            Action::SteerRight,
            Source::PadAxis(GamepadAxis::LeftStickX),
            Calibration::new(BiUpper, 0.2, 1., 0.5),
        );
        let mut actions = Actions::new(map, Pads::Any);
        actions.update(|s| match s {
            Source::Key(k) => Some(f32::from(k == KeyCode::ArrowLeft)),
            _ => Some(0.8),
        });
        assert_eq!(actions.controls().steering, 1. - 0.5);
    }

    /// The keyboard path through the action layer equals the earlier direct
    /// key formula bit for bit for every combination of the driving keys.
    #[test]
    fn keyboard_controls_are_bit_identical_to_the_direct_key_formula() {
        let keys = [
            KeyCode::ArrowUp,
            KeyCode::KeyW,
            KeyCode::ArrowDown,
            KeyCode::KeyS,
            KeyCode::ArrowLeft,
            KeyCode::KeyA,
            KeyCode::ArrowRight,
            KeyCode::KeyD,
            KeyCode::Space,
        ];
        let mut actions = Actions::new(ActionMap::keyboard(), Pads::Any);
        for mask in 0u32..1 << keys.len() {
            let held = |k: KeyCode| {
                keys.iter()
                    .position(|x| *x == k)
                    .is_some_and(|i| mask & 1 << i != 0)
            };
            actions.update(|s| match s {
                Source::Key(k) => Some(f32::from(held(k))),
                _ => None,
            });
            let pressed = |a, b| held(a) || held(b);
            let old = Controls {
                throttle: f32::from(pressed(KeyCode::ArrowUp, KeyCode::KeyW)),
                brake: f32::from(pressed(KeyCode::ArrowDown, KeyCode::KeyS)),
                steering: f32::from(pressed(KeyCode::ArrowLeft, KeyCode::KeyA))
                    - f32::from(pressed(KeyCode::ArrowRight, KeyCode::KeyD)),
                handbrake: held(KeyCode::Space),
            };
            let new = actions.controls();
            assert_eq!(new.throttle.to_bits(), old.throttle.to_bits(), "{mask:b}");
            assert_eq!(new.brake.to_bits(), old.brake.to_bits(), "{mask:b}");
            assert_eq!(new.steering.to_bits(), old.steering.to_bits(), "{mask:b}");
            assert_eq!(new.handbrake, old.handbrake, "{mask:b}");
        }
    }

    #[test]
    fn gear_edges_fire_once_per_press() {
        let mut map = ActionMap::keyboard();
        map.bind(
            Action::Gear3,
            Source::Key(KeyCode::Digit3),
            Calibration::default(),
        );
        map.bind(
            Action::GearReverse,
            Source::Key(KeyCode::KeyR),
            Calibration::default(),
        );
        let mut actions = Actions::new(map, Pads::Off);
        let press = |actions: &mut Actions, keys: &[KeyCode]| {
            actions.update(|s| match s {
                Source::Key(k) => Some(f32::from(keys.contains(&k))),
                _ => None,
            });
            actions.shifts().collect::<Vec<_>>()
        };
        assert_eq!(press(&mut actions, &[KeyCode::KeyX]), [Shift::Up]);
        // Held: no new edge.
        assert_eq!(press(&mut actions, &[KeyCode::KeyX]), []);
        assert_eq!(press(&mut actions, &[]), []);
        assert_eq!(
            press(
                &mut actions,
                &[KeyCode::KeyZ, KeyCode::Digit3, KeyCode::KeyR]
            ),
            [Shift::Down, Shift::Gear(-1)]
        );
        assert_eq!(press(&mut actions, &[KeyCode::Digit3]), []);
    }

    /// Frames are PreUpdate samples; ticks are FixedUpdate runs that pop one
    /// request each. The earlier `just_pressed` inside FixedUpdate shifted
    /// twice in a two-tick frame and lost a press in a zero-tick frame.
    #[test]
    fn edge_queue_shifts_once_per_press_whatever_the_ticks_per_frame() {
        let mut actions = Actions::new(ActionMap::keyboard(), Pads::Off);
        let mut queue = ShiftQueue::default();
        let frame = |actions: &mut Actions, queue: &mut ShiftQueue, x: bool, ticks: usize| {
            actions.update(|s| match s {
                Source::Key(k) => Some(f32::from(x && k == KeyCode::KeyX)),
                _ => None,
            });
            for shift in actions.shifts() {
                queue.push(shift);
            }
            (0..ticks).filter_map(|_| queue.pop()).collect::<Vec<_>>()
        };
        // Two ticks in the press frame: one shift.
        assert_eq!(frame(&mut actions, &mut queue, true, 2), [Shift::Up]);
        assert_eq!(frame(&mut actions, &mut queue, false, 1), []);
        // Zero ticks in the press frame: the shift waits for the next tick.
        assert_eq!(frame(&mut actions, &mut queue, true, 0), []);
        assert_eq!(frame(&mut actions, &mut queue, true, 0), []);
        assert_eq!(frame(&mut actions, &mut queue, false, 1), [Shift::Up]);
        assert_eq!(frame(&mut actions, &mut queue, false, 3), []);
        // Two presses before a tick: one per tick, in order.
        frame(&mut actions, &mut queue, true, 0);
        frame(&mut actions, &mut queue, false, 0);
        frame(&mut actions, &mut queue, true, 0);
        assert_eq!(frame(&mut actions, &mut queue, false, 1), [Shift::Up]);
        assert_eq!(frame(&mut actions, &mut queue, false, 1), [Shift::Up]);
        // Bounded while no tick runs.
        for _ in 0..20 {
            frame(&mut actions, &mut queue, true, 0);
            frame(&mut actions, &mut queue, false, 0);
        }
        assert_eq!(queue.0.len(), ShiftQueue::CAPACITY);
    }

    #[test]
    fn pad_preset_bindings_and_overrides() {
        let preset = action_map::read(
            r#"<ActionMap deviceType="win_xinput" schemeType="xinputScheme" priority="6">
<Action actionName="Accelerate"><Axis axisName="win_con_xi_buttonRightTrigger" deviceName="win_xinput" baseCalibration="uniDirectionalPositive" deadZone="0.0" saturation="1.0"/></Action>
<Action actionName="Steer Left"><Axis axisName="win_con_xi_analogLeftStickX" deviceName="win_xinput" baseCalibration="biDirectionalLower" deadZone="0.2" saturation="1.0"/></Action>
<Action actionName="Steer Right"><Axis axisName="win_con_xi_analogLeftStickX" deviceName="win_xinput" baseCalibration="biDirectionalUpper" deadZone="0.2" saturation="1.0"/></Action>
<Action actionName="Gear 1"><Axis axisName="unmapped" deviceName="win_xinput" baseCalibration="uniDirectionalPositive"/></Action>
<Action actionName="Pause"><Axis axisName="win_key_escape" deviceName="Keyboard" baseCalibration="uniDirectionalPositive"/></Action>
<Action actionName="Menu Left"><Axis axisName="win_con_xi_buttonDPadLeft" deviceName="win_xinput" baseCalibration="uniDirectionalPositive"/></Action>
<Action actionName="Boost"><Axis axisName="win_con_xi_buttonTurbo" deviceName="win_xinput" baseCalibration="uniDirectionalPositive"/></Action>
</ActionMap>"#,
        )
        .unwrap();
        let (map, unknown) = ActionMap::pad(&preset);
        assert_eq!(unknown, ["win_con_xi_buttonTurbo"]);
        assert_eq!(
            map.bindings(Action::Accelerate)[0].source,
            Source::PadButton(GamepadButton::RightTrigger2)
        );
        assert_eq!(map.bindings(Action::Gear1)[0].source, Source::Unmapped);
        assert!(map.bindings(Action::Pause).is_empty());
        let steer = map.bindings(Action::SteerRight)[0].calibration;
        assert_eq!(
            (steer.kind, steer.dead_zone, steer.saturation),
            (BiUpper, 0.2, 1.)
        );
        // No pad connected: idle 0.5 on the stick, nothing steers.
        let mut actions = Actions::new(map.clone(), Pads::Any);
        actions.update(|_| None);
        assert_eq!(actions.controls().steering, 0.);
        // A stick at 0.5 + 0.5 * 0.6 = 0.8 with dz 0.2: half lock right.
        let pad = |x: f32| {
            move |s: Source| match s {
                Source::PadAxis(_) => Some(0.5 + 0.5 * x),
                _ => Some(0.),
            }
        };
        actions.update(pad(0.6));
        assert_eq!(
            actions.controls().steering,
            -((0.8 - 0.6) * (1. / (1. - 0.6f32)))
        );
        // Advanced Controls steering dead zone 0: the same stick steers more.
        let settings = Settings::parse("steering_deadzone=0").unwrap();
        let mut tuned = map.clone();
        settings.tune(&mut tuned).unwrap();
        let steer = tuned.bindings(Action::SteerRight)[0].calibration;
        assert_eq!((steer.dead_zone, steer.saturation), (0., 1.));
        let mut actions = Actions::new(tuned, Pads::Any);
        actions.update(pad(0.6));
        assert!(-actions.controls().steering > 0.5);
        // A dead zone at or above the saturation is refused.
        let mut bad = map;
        assert!(
            Settings::parse("steering_deadzone=1")
                .unwrap()
                .tune(&mut bad)
                .is_err()
        );
    }

    #[test]
    fn settings_parse_and_device_class() {
        let s =
            Settings::parse("device=pad, brake_saturation=0.9 # comment\naccelerate_deadzone=0.1")
                .unwrap();
        assert_eq!(s.device, Device::Pad);
        assert_eq!(s.brake.saturation, Some(0.9));
        assert_eq!(s.accelerate.dead_zone, Some(0.1));
        assert_eq!(Settings::parse(&s.to_string()).unwrap(), s);
        assert!(Settings::parse("device=joystick").is_err());
        assert!(Settings::parse("steering_deadzone=2").is_err());
        assert!(Settings::parse("speed=1").is_err());
        let mut a = Assists::default();
        Settings::parse("device=wheel").unwrap().apply_class(&mut a);
        assert_eq!(a.input, Input::Wheel);
        Settings::parse("device=pad").unwrap().apply_class(&mut a);
        assert_eq!(a.input, Input::Keyboard);
        let mut w = Assists {
            input: Input::Wheel,
            ..Assists::default()
        };
        Settings::default().apply_class(&mut w);
        assert_eq!(w.input, Input::Wheel);
        assert_eq!(
            Settings::parse("device=keyboard").unwrap().pads(),
            Pads::Off
        );
        assert_eq!(Settings::parse("device=pad").unwrap().pads(), Pads::Any);
        assert_eq!(Settings::default().pads(), Pads::Recognised);
    }

    #[test]
    fn phone_settings_parse_round_trip_and_stay_out_of_the_desktop_line() {
        // Defaults print exactly as before P11.
        assert_eq!(Settings::default().to_string(), "device=auto");
        assert!(!Settings::default().touch_active(false));
        assert!(Settings::default().touch_active(true));
        let s = Settings::parse(
            "touch=on tilt=on tilt_max_angle=25 tilt_neutral=-4.5 tilt_smoothing=0",
        )
        .unwrap();
        assert!(s.touch_active(false));
        let t = s.tilt_settings().unwrap();
        assert_eq!((t.max_angle, t.neutral, t.smoothing), (25., Some(-4.5), 0.));
        assert_eq!(Settings::parse(&s.to_string()).unwrap(), s);
        let d = Settings::default().tilt_settings().unwrap();
        assert_eq!(d, crate::tilt::Settings::default());
        assert_eq!(Settings::parse("tilt=off").unwrap().tilt_settings(), None);
        assert!(!Settings::parse("touch=off").unwrap().touch_active(true));
        assert_eq!(Settings::parse("touch=auto").unwrap().touch, None);
        assert_eq!(
            Settings::parse("tilt_neutral=auto").unwrap().tilt_neutral,
            None
        );
        for bad in [
            "tilt=auto",
            "touch=maybe",
            "tilt_max_angle=2",
            "tilt_max_angle=120",
            "tilt_neutral=100",
            "tilt_smoothing=-1",
            "tilt_max_angle=auto",
        ] {
            assert!(Settings::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn touch_zones_drive_like_keys_and_tilt_like_a_pad_stick() {
        use crate::touch_controls::Button as B;
        let mut actions = Actions::new(ActionMap::touch(true), Pads::Off);
        let run = |actions: &mut Actions, held: &[B], tilt: Option<f32>| {
            actions.update(|s| match s {
                Source::Touch(b) => Some(f32::from(held.contains(&b))),
                Source::Tilt => tilt,
                _ => None,
            });
            actions.controls()
        };
        // Idle, no tilt reading yet: centred, +0.0 like the keyboard.
        let c = run(&mut actions, &[], None);
        assert_eq!((c.throttle, c.brake, c.handbrake), (0., 0., false));
        assert_eq!(c.steering.to_bits(), 0f32.to_bits());
        // Pedals and handbrake are exact 0/1.
        let c = run(
            &mut actions,
            &[B::Throttle, B::Brake, B::Handbrake],
            Some(0.5),
        );
        assert_eq!((c.throttle, c.brake, c.handbrake), (1., 1., true));
        assert_eq!(c.steering.to_bits(), 0f32.to_bits());
        // Tilt: raw 0.5 + 0.5 t, dz 0.05, sat 1, positive right -> negative
        // (right) port steering.
        let steer = |actions: &mut Actions, t: f32| run(actions, &[], Some(0.5 + 0.5 * t)).steering;
        assert_eq!(steer(&mut actions, 0.04), 0.);
        assert_eq!(steer(&mut actions, 1.), -1.);
        assert_eq!(steer(&mut actions, -1.), 1.);
        let half = steer(&mut actions, 0.525);
        assert!((half + 0.5).abs() < 1e-5, "{half}");
        // The Advanced Controls override reaches the tilt bindings.
        actions
            .calibrate(
                Action::SteerRight,
                Tune {
                    dead_zone: Some(0.5),
                    saturation: None,
                },
            )
            .unwrap();
        assert_eq!(steer(&mut actions, 0.45), 0.);
        // Gear buttons edge once per press; Menu Back also exits a replay.
        run(&mut actions, &[B::GearUp, B::MenuBack], None);
        assert_eq!(actions.shifts().collect::<Vec<_>>(), vec![Shift::Up]);
        assert!(actions.pressed(Action::ReplayExit) && actions.pressed(Action::MenuBack));
        run(&mut actions, &[B::GearUp], None);
        assert_eq!(actions.shifts().count(), 0);
        // Without tilt there are no steering bindings.
        assert!(
            ActionMap::touch(false)
                .bindings(Action::SteerLeft)
                .is_empty()
        );
    }

    #[test]
    fn only_gamepads_take_the_pad_preset_by_default() {
        assert!(recognised("Xbox Controller", Some(0x045e)).is_ok());
        assert!(recognised("Controller (XBOX 360 For Windows)", None).is_ok());
        assert!(recognised("Wireless Gamepad F710", Some(0x046d)).is_ok());
        assert!(recognised("FANATEC CSL Elite Wheel Base", Some(0x0eb7)).is_err());
        assert!(recognised("Pad", Some(0x0eb7)).is_err());
        assert!(recognised("Logitech G29 Driving Force Racing Wheel", Some(0x046d)).is_err());
        assert!(recognised("T-Rudder pedals", None).is_err());
        assert!(recognised("HID-compliant game controller joystick", None).is_err());
        assert!(recognised("Unknown device", None).is_err());
        // A pad Android classified keeps only the wheel guard.
        assert!(recognised("8BitDo Pro 2", Some(0x2dc8)).is_err());
        assert!(not_a_wheel("8BitDo Pro 2", Some(0x2dc8)).is_ok());
        assert!(not_a_wheel("DualSense Wireless Controller", Some(0x054c)).is_ok());
        assert!(not_a_wheel("Thrustmaster T300RS Racing wheel", Some(0x044f)).is_err());
        assert!(not_a_wheel("Logitech G29 Driving Force Racing Wheel", Some(0x046d)).is_err());
    }

    #[test]
    fn pass_through_pad_settings_keep_raw_values() {
        let s = passthrough();
        assert_eq!(s.default_axis_settings.clamp(0.01), 0.01);
        assert_eq!(s.default_axis_settings.clamp(-0.04), -0.04);
        assert_eq!(s.default_axis_settings.threshold(), 0.);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT purchased source"]
    fn shipped_pad_preset_reads_without_unknown_axes() {
        let root = std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required");
        let (actions, pad) =
            load(std::path::Path::new(&root), &Settings::default(), false).unwrap();
        assert_eq!(actions.pads, Pads::Recognised);
        assert_eq!(pad, "actionMapPreset4");
        let left = actions.map.bindings(Action::SteerLeft);
        // Keyboard Left/A, then the pad's left stick and D-pad.
        assert_eq!(left.len(), 4);
        assert_eq!(left[2].source, Source::PadAxis(GamepadAxis::LeftStickX));
        assert_eq!(
            (left[2].calibration.kind, left[2].calibration.dead_zone),
            (BiLower, 0.2)
        );
        assert_eq!(
            actions.map.bindings(Action::Accelerate)[2].source,
            Source::PadButton(GamepadButton::RightTrigger2)
        );
        assert_eq!(
            actions.map.bindings(Action::Handbrake)[1].source,
            Source::PadButton(GamepadButton::East)
        );
        assert_eq!(
            actions.map.bindings(Action::GearUp)[1].source,
            Source::PadButton(GamepadButton::South)
        );
        let (keyboard, off) = load(
            std::path::Path::new(&root),
            &Settings::parse("device=keyboard").unwrap(),
            false,
        )
        .unwrap();
        assert_eq!(off, "off");
        assert_eq!(keyboard.map, ActionMap::keyboard());
    }
}
