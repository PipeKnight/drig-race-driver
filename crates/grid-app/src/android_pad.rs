//! Android gamepads (P11b, docs/ANDROID.md "Gamepads").
//!
//! gilrs has no Android backend, and winit turns a pad's stick motion into
//! touches and drops its other axes. `MainActivity` therefore takes pad key
//! and joystick events before GameActivity sees them (`PadBridge.java`) and
//! calls the JNI functions below. This module turns them into Bevy's raw
//! gamepad events, so the pad reaches the original action layer through the
//! XInput pad preset exactly like a gilrs pad on the desktop (input.rs).
//!
//! The mapping is plain code tested on the desktop; the queue, JNI glue and
//! Bevy system run only on Android.
#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use bevy::input::gamepad::{GamepadAxis, GamepadButton};

/// Android `KeyEvent.KEYCODE_*` values (android/keycodes.h).
mod key {
    pub const BACK: i32 = 4;
    pub const DPAD_UP: i32 = 19;
    pub const DPAD_DOWN: i32 = 20;
    pub const DPAD_LEFT: i32 = 21;
    pub const DPAD_RIGHT: i32 = 22;
    pub const DPAD_CENTER: i32 = 23;
    pub const BUTTON_A: i32 = 96;
    pub const BUTTON_B: i32 = 97;
    pub const BUTTON_C: i32 = 98;
    pub const BUTTON_X: i32 = 99;
    pub const BUTTON_Y: i32 = 100;
    pub const BUTTON_Z: i32 = 101;
    pub const BUTTON_L1: i32 = 102;
    pub const BUTTON_R1: i32 = 103;
    pub const BUTTON_L2: i32 = 104;
    pub const BUTTON_R2: i32 = 105;
    pub const BUTTON_THUMBL: i32 = 106;
    pub const BUTTON_THUMBR: i32 = 107;
    pub const BUTTON_START: i32 = 108;
    pub const BUTTON_SELECT: i32 = 109;
    pub const BUTTON_MODE: i32 = 110;
}

/// Android's pad keys on Bevy's (Xbox) layout, as gilrs maps desktop pads.
/// A pad's BACK key (some send it for View/Share) is Select; DPAD_CENTER is
/// the confirm button, South. AUTHORED for those two.
pub fn button(code: i32) -> Option<GamepadButton> {
    use GamepadButton as B;
    Some(match code {
        key::BUTTON_A | key::DPAD_CENTER => B::South,
        key::BUTTON_B => B::East,
        key::BUTTON_X => B::West,
        key::BUTTON_Y => B::North,
        key::BUTTON_C => B::C,
        key::BUTTON_Z => B::Z,
        key::BUTTON_L1 => B::LeftTrigger,
        key::BUTTON_R1 => B::RightTrigger,
        key::BUTTON_L2 => B::LeftTrigger2,
        key::BUTTON_R2 => B::RightTrigger2,
        key::BUTTON_THUMBL => B::LeftThumb,
        key::BUTTON_THUMBR => B::RightThumb,
        key::BUTTON_START => B::Start,
        key::BUTTON_SELECT | key::BACK => B::Select,
        key::BUTTON_MODE => B::Mode,
        key::DPAD_UP => B::DPadUp,
        key::DPAD_DOWN => B::DPadDown,
        key::DPAD_LEFT => B::DPadLeft,
        key::DPAD_RIGHT => B::DPadRight,
        _ => return None,
    })
}

/// Every button the bridge reports, in a fixed order.
pub const BUTTONS: [GamepadButton; 19] = {
    use GamepadButton as B;
    [
        B::South,
        B::East,
        B::West,
        B::North,
        B::C,
        B::Z,
        B::LeftTrigger,
        B::RightTrigger,
        B::LeftTrigger2,
        B::RightTrigger2,
        B::LeftThumb,
        B::RightThumb,
        B::Start,
        B::Select,
        B::Mode,
        B::DPadUp,
        B::DPadDown,
        B::DPadLeft,
        B::DPadRight,
    ]
};

pub const AXES: [GamepadAxis; 4] = [
    GamepadAxis::LeftStickX,
    GamepadAxis::LeftStickY,
    GamepadAxis::RightStickX,
    GamepadAxis::RightStickY,
];

/// One joystick sample from `PadBridge.motion`: left stick, right stick
/// (Android: +Y down), triggers 0..1 and the hat (-1, 0, 1).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Motion {
    pub left: [f32; 2],
    pub right: [f32; 2],
    pub triggers: [f32; 2],
    pub hat: [f32; 2],
}

/// A pad's keys and last joystick sample.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PadState {
    keys: [bool; BUTTONS.len()],
    motion: Motion,
}

/// Hat values beyond this count as pressed (Android reports -1, 0 or 1).
const HAT_PRESSED: f32 = 0.5;

impl PadState {
    pub fn key(&mut self, code: i32, down: bool) {
        if let Some(b) = button(code)
            && let Some(i) = BUTTONS.iter().position(|x| *x == b)
        {
            self.keys[i] = down;
        }
    }

    pub fn motion(&mut self, motion: Motion) {
        self.motion = motion;
    }

    /// Button values in [`BUTTONS`] order. A pad may report the D-pad as
    /// keys or as a hat, and L2/R2 as keys or as trigger axes; either counts.
    pub fn buttons(&self) -> [f32; BUTTONS.len()] {
        use GamepadButton as B;
        let m = &self.motion;
        let mut out = self.keys.map(f32::from);
        for (i, b) in BUTTONS.iter().enumerate() {
            let analog = match b {
                B::LeftTrigger2 => m.triggers[0].clamp(0., 1.),
                B::RightTrigger2 => m.triggers[1].clamp(0., 1.),
                B::DPadLeft => f32::from(m.hat[0] < -HAT_PRESSED),
                B::DPadRight => f32::from(m.hat[0] > HAT_PRESSED),
                B::DPadUp => f32::from(m.hat[1] < -HAT_PRESSED),
                B::DPadDown => f32::from(m.hat[1] > HAT_PRESSED),
                _ => 0.,
            };
            out[i] = out[i].max(analog);
        }
        out
    }

    /// Axis values in [`AXES`] order, Bevy's convention (+Y up).
    pub fn axes(&self) -> [f32; 4] {
        let m = &self.motion;
        [m.left[0], -m.left[1], m.right[0], -m.right[1]].map(|v| v.clamp(-1., 1.))
    }
}

/// What `PadBridge.java` reports, keyed by Android's input device id.
#[derive(Clone, Debug, PartialEq)]
pub enum PadEvent {
    Connected {
        id: i32,
        name: String,
        vendor: u16,
        product: u16,
    },
    Disconnected {
        id: i32,
    },
    Key {
        id: i32,
        code: i32,
        down: bool,
    },
    Motion {
        id: i32,
        motion: Motion,
    },
}

#[cfg(target_os = "android")]
pub use bridge::install;

#[cfg(target_os = "android")]
mod bridge {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use bevy::input::InputSystems;
    use bevy::input::gamepad::{
        GamepadConnection, GamepadConnectionEvent, RawGamepadAxisChangedEvent,
        RawGamepadButtonChangedEvent, RawGamepadEvent,
    };
    use bevy::prelude::*;
    use jni::JNIEnv;
    use jni::objects::{JClass, JString};
    use jni::sys::{jboolean, jfloat, jint};

    use super::{AXES, BUTTONS, Motion, PadEvent, PadState};

    /// Filled on the UI thread by the JNI calls, drained once per frame.
    static QUEUE: Mutex<Vec<PadEvent>> = Mutex::new(Vec::new());

    fn push(event: PadEvent) {
        if let Ok(mut queue) = QUEUE.lock() {
            // A stalled game (loading) must not grow the queue without bound;
            // motion samples are the bulk and only the latest matters.
            if queue.len() >= 4096 {
                queue.retain(|e| !matches!(e, PadEvent::Motion { .. }));
            }
            queue.push(event);
        }
    }

    struct Pad {
        entity: Entity,
        state: PadState,
        sent_buttons: [Option<f32>; BUTTONS.len()],
        sent_axes: [Option<f32>; AXES.len()],
    }

    #[derive(Default)]
    struct Pads(HashMap<i32, Pad>);

    pub fn install(app: &mut App) {
        app.add_systems(PreUpdate, bridge_system.before(InputSystems));
    }

    #[allow(clippy::too_many_arguments)]
    fn connect(
        commands: &mut Commands,
        pads: &mut Pads,
        raw: &mut MessageWriter<RawGamepadEvent>,
        connections: &mut MessageWriter<GamepadConnectionEvent>,
        id: i32,
        name: String,
        vendor: Option<u16>,
        product: Option<u16>,
    ) {
        if pads.0.contains_key(&id) {
            return;
        }
        let entity = commands.spawn(crate::input::ClassifiedPad).id();
        println!(
            "drive_input_pad android_device={id} entity={entity} name={name:?} vendor={vendor:04x?} product={product:04x?}"
        );
        let event = GamepadConnectionEvent::new(
            entity,
            GamepadConnection::Connected {
                name,
                vendor_id: vendor,
                product_id: product,
            },
        );
        raw.write(event.clone().into());
        connections.write(event);
        pads.0.insert(
            id,
            Pad {
                entity,
                state: PadState::default(),
                sent_buttons: [None; BUTTONS.len()],
                sent_axes: [None; AXES.len()],
            },
        );
    }

    fn bridge_system(
        mut commands: Commands,
        mut pads: Local<Pads>,
        mut raw: MessageWriter<RawGamepadEvent>,
        mut connections: MessageWriter<GamepadConnectionEvent>,
        mut buttons: MessageWriter<RawGamepadButtonChangedEvent>,
        mut axes: MessageWriter<RawGamepadAxisChangedEvent>,
    ) {
        let events = match QUEUE.lock() {
            Ok(mut queue) => std::mem::take(&mut *queue),
            Err(_) => return,
        };
        for event in events {
            match event {
                PadEvent::Connected {
                    id,
                    name,
                    vendor,
                    product,
                } => {
                    let nonzero = |v: u16| (v != 0).then_some(v);
                    connect(
                        &mut commands,
                        &mut pads,
                        &mut raw,
                        &mut connections,
                        id,
                        name,
                        nonzero(vendor),
                        nonzero(product),
                    );
                }
                PadEvent::Disconnected { id } => {
                    if let Some(pad) = pads.0.remove(&id) {
                        println!("drive_input_pad android_device={id} disconnected");
                        let event = GamepadConnectionEvent::new(
                            pad.entity,
                            GamepadConnection::Disconnected,
                        );
                        raw.write(event.clone().into());
                        connections.write(event);
                    }
                }
                PadEvent::Key { id, .. } | PadEvent::Motion { id, .. } => {
                    // Input before the device listener saw the pad.
                    connect(
                        &mut commands,
                        &mut pads,
                        &mut raw,
                        &mut connections,
                        id,
                        format!("Android pad {id}"),
                        None,
                        None,
                    );
                    if let Some(pad) = pads.0.get_mut(&id) {
                        match event {
                            PadEvent::Key { code, down, .. } => pad.state.key(code, down),
                            PadEvent::Motion { motion, .. } => pad.state.motion(motion),
                            _ => {}
                        }
                    }
                }
            }
        }
        // Report changes only, like gilrs.
        for pad in pads.0.values_mut() {
            for ((b, v), sent) in BUTTONS
                .iter()
                .zip(pad.state.buttons())
                .zip(&mut pad.sent_buttons)
            {
                if *sent != Some(v) {
                    *sent = Some(v);
                    let e = RawGamepadButtonChangedEvent::new(pad.entity, *b, v);
                    raw.write(e.into());
                    buttons.write(e);
                }
            }
            for ((a, v), sent) in AXES.iter().zip(pad.state.axes()).zip(&mut pad.sent_axes) {
                if *sent != Some(v) {
                    *sent = Some(v);
                    let e = RawGamepadAxisChangedEvent::new(pad.entity, *a, v);
                    raw.write(e.into());
                    axes.write(e);
                }
            }
        }
    }

    // JNI entry points of `PadBridge`'s static natives (UI thread).

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_org_reassembly_grid_PadBridge_nativeConnected(
        mut env: JNIEnv,
        _class: JClass,
        id: jint,
        name: JString,
        vendor: jint,
        product: jint,
    ) {
        let name: String = env
            .get_string(&name)
            .map(Into::into)
            .unwrap_or_else(|_| format!("Android pad {id}"));
        push(PadEvent::Connected {
            id,
            name,
            vendor: u16::try_from(vendor).unwrap_or(0),
            product: u16::try_from(product).unwrap_or(0),
        });
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_org_reassembly_grid_PadBridge_nativeDisconnected(
        _env: JNIEnv,
        _class: JClass,
        id: jint,
    ) {
        push(PadEvent::Disconnected { id });
    }

    #[unsafe(no_mangle)]
    pub extern "system" fn Java_org_reassembly_grid_PadBridge_nativeKey(
        _env: JNIEnv,
        _class: JClass,
        id: jint,
        code: jint,
        down: jboolean,
    ) {
        push(PadEvent::Key {
            id,
            code,
            down: down != 0,
        });
    }

    #[unsafe(no_mangle)]
    #[allow(clippy::too_many_arguments)]
    pub extern "system" fn Java_org_reassembly_grid_PadBridge_nativeMotion(
        _env: JNIEnv,
        _class: JClass,
        id: jint,
        lx: jfloat,
        ly: jfloat,
        rx: jfloat,
        ry: jfloat,
        lt: jfloat,
        rt: jfloat,
        hat_x: jfloat,
        hat_y: jfloat,
    ) {
        push(PadEvent::Motion {
            id,
            motion: Motion {
                left: [lx, ly],
                right: [rx, ry],
                triggers: [lt, rt],
                hat: [hat_x, hat_y],
            },
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use GamepadButton as B;

    fn value(state: &PadState, b: GamepadButton) -> f32 {
        state.buttons()[BUTTONS.iter().position(|x| *x == b).unwrap()]
    }

    #[test]
    fn every_mapped_key_lands_on_a_reported_button() {
        for code in 0..300 {
            if let Some(b) = button(code) {
                assert!(BUTTONS.contains(&b), "{code} -> {b:?}");
            }
        }
        assert_eq!(button(key::BUTTON_A), Some(B::South));
        assert_eq!(button(key::BUTTON_B), Some(B::East));
        assert_eq!(button(key::BACK), Some(B::Select));
        // Volume and letter keys are not pad buttons.
        assert_eq!(button(24), None);
        assert_eq!(button(29), None);
    }

    #[test]
    fn keys_press_and_release() {
        let mut s = PadState::default();
        s.key(key::BUTTON_A, true);
        assert_eq!(value(&s, B::South), 1.);
        s.key(key::BUTTON_A, false);
        assert_eq!(value(&s, B::South), 0.);
    }

    #[test]
    fn dpad_from_keys_or_hat_and_triggers_from_keys_or_axes() {
        let mut s = PadState::default();
        s.motion(Motion {
            hat: [-1., 1.],
            triggers: [0.25, 0.],
            ..Motion::default()
        });
        assert_eq!(value(&s, B::DPadLeft), 1.);
        assert_eq!(value(&s, B::DPadDown), 1.);
        assert_eq!(value(&s, B::DPadUp), 0.);
        assert_eq!(value(&s, B::LeftTrigger2), 0.25);
        s.key(key::BUTTON_L2, true);
        s.key(key::DPAD_UP, true);
        assert_eq!(value(&s, B::LeftTrigger2), 1.);
        assert_eq!(value(&s, B::DPadUp), 1.);
        s.motion(Motion::default());
        s.key(key::BUTTON_L2, false);
        assert_eq!(value(&s, B::LeftTrigger2), 0.);
        assert_eq!(value(&s, B::DPadLeft), 0.);
    }

    #[test]
    fn stick_y_is_flipped_to_bevys_up_positive() {
        let mut s = PadState::default();
        s.motion(Motion {
            left: [0.5, -1.],
            right: [-0.25, 0.75],
            ..Motion::default()
        });
        assert_eq!(s.axes(), [0.5, 1., -0.25, -0.75]);
    }
}
