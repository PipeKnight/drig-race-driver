//! SDL's gamepad layer owns controller input and output on macOS. In particular, its
//! Stadia HID driver decodes the report directly and can address both rumble motors.

use bevy::ecs::system::NonSendMut;
use bevy::input::InputSystems;
use bevy::input::gamepad::{
    GamepadAxis, GamepadButton, GamepadConnection, GamepadConnectionEvent,
    RawGamepadAxisChangedEvent, RawGamepadButtonChangedEvent, RawGamepadEvent,
};
use bevy::prelude::*;
use sdl3::joystick::JoystickId;
use sdl3::{
    EventPump, GamepadSubsystem,
    event::Event,
    gamepad::{Axis, Button, Gamepad},
};
use std::collections::HashMap;

#[derive(Message, Clone, Copy)]
pub(crate) struct Rumble {
    pub(crate) pad: Entity,
    pub(crate) motors: (u8, u8),
}

struct Pad {
    entity: Entity,
    device: Gamepad,
    rumble: bool,
    reported_rumble: bool,
    reported_rumble_error: bool,
}

struct Controllers {
    pads: HashMap<JoystickId, Pad>,
    events: EventPump,
    startup: Vec<JoystickId>,
    subsystem: GamepadSubsystem,
}

const AXES: [Axis; 6] = [
    Axis::LeftX,
    Axis::LeftY,
    Axis::RightX,
    Axis::RightY,
    Axis::TriggerLeft,
    Axis::TriggerRight,
];

const BUTTONS: [(Button, GamepadButton); 15] = [
    (Button::South, GamepadButton::South),
    (Button::East, GamepadButton::East),
    (Button::West, GamepadButton::West),
    (Button::North, GamepadButton::North),
    (Button::Back, GamepadButton::Select),
    (Button::Guide, GamepadButton::Mode),
    (Button::Start, GamepadButton::Start),
    (Button::LeftStick, GamepadButton::LeftThumb),
    (Button::RightStick, GamepadButton::RightThumb),
    (Button::LeftShoulder, GamepadButton::LeftTrigger),
    (Button::RightShoulder, GamepadButton::RightTrigger),
    (Button::DPadUp, GamepadButton::DPadUp),
    (Button::DPadDown, GamepadButton::DPadDown),
    (Button::DPadLeft, GamepadButton::DPadLeft),
    (Button::DPadRight, GamepadButton::DPadRight),
];

fn axis_message(pad: Entity, axis: Axis, value: i16) -> RawGamepadEvent {
    let value = f32::from(value);
    let axis = match axis {
        Axis::TriggerLeft | Axis::TriggerRight => {
            let button = if axis == Axis::TriggerLeft {
                GamepadButton::LeftTrigger2
            } else {
                GamepadButton::RightTrigger2
            };
            return RawGamepadButtonChangedEvent::new(pad, button, value.max(0.) / 32767.).into();
        }
        Axis::LeftX => GamepadAxis::LeftStickX,
        Axis::LeftY => GamepadAxis::LeftStickY,
        Axis::RightX => GamepadAxis::RightStickX,
        Axis::RightY => GamepadAxis::RightStickY,
    };
    let normalized = if value < 0. {
        value / 32768.
    } else {
        value / 32767.
    };
    let normalized = if matches!(axis, GamepadAxis::LeftStickY | GamepadAxis::RightStickY) {
        -normalized
    } else {
        normalized
    };
    RawGamepadAxisChangedEvent::new(pad, axis, normalized).into()
}

impl Pad {
    fn state(&self) -> impl Iterator<Item = RawGamepadEvent> + '_ {
        AXES.into_iter()
            .map(|axis| axis_message(self.entity, axis, self.device.axis(axis)))
            .chain(BUTTONS.into_iter().map(|(from, to)| {
                RawGamepadButtonChangedEvent::new(
                    self.entity,
                    to,
                    f32::from(u8::from(self.device.button(from))),
                )
                .into()
            }))
    }
}

/// A hosted app has its own entities and starts with the current state of each physical pad.
pub(crate) fn snapshot(world: &World, pad: Entity) -> Vec<RawGamepadEvent> {
    world
        .get_non_send::<Controllers>()
        .and_then(|state| state.pads.values().find(|p| p.entity == pad))
        .map(|pad| pad.state().collect())
        .unwrap_or_default()
}

pub(crate) fn install(app: &mut App) {
    // Winit owns the window; SDL would otherwise suppress input without an SDL window.
    sdl3::hint::set("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1");
    let state = (|| {
        let sdl = sdl3::init()?;
        let subsystem = sdl.gamepad()?;
        let events = sdl.event_pump()?;
        let startup = subsystem.gamepads()?;
        Ok::<_, sdl3::Error>(Controllers {
            pads: HashMap::new(),
            events,
            startup,
            subsystem,
        })
    })();
    match state {
        Ok(state) => {
            app.world_mut().insert_non_send(state);
            app.add_message::<Rumble>()
                .add_systems(PreUpdate, poll.before(InputSystems))
                .add_systems(Last, rumble);
            println!("macos_gamepad backend=SDL3");
        }
        Err(error) => {
            warn!("SDL3 gamepads unavailable: {error}");
            app.add_plugins(bevy::gilrs::GilrsPlugin)
                .add_message::<Rumble>();
        }
    }
}

fn poll(
    mut state: NonSendMut<Controllers>,
    mut commands: Commands,
    mut raw: MessageWriter<RawGamepadEvent>,
    mut connections: MessageWriter<GamepadConnectionEvent>,
) {
    let Controllers {
        pads,
        events,
        startup,
        subsystem,
    } = &mut *state;
    let attached = std::mem::take(startup)
        .into_iter()
        .map(|which| Event::GamepadAdded {
            which,
            timestamp: 0,
        });
    for event in attached.chain(events.poll_iter()) {
        match event {
            Event::GamepadAdded { which, .. } => {
                if pads.contains_key(&which) {
                    continue;
                }
                let Ok(device) = subsystem.open(which) else {
                    continue;
                };
                let name = device.name().unwrap_or_else(|| "Gamepad".into());
                let vendor_id = device.vendor_id();
                let product_id = device.product_id();
                let entity = commands.spawn(crate::input::ClassifiedPad).id();
                let connection = GamepadConnectionEvent::new(
                    entity,
                    GamepadConnection::Connected {
                        name,
                        vendor_id,
                        product_id,
                    },
                );
                // SAFETY: the handle came from a successful open and is owned by this state.
                let rumble = unsafe { device.has_rumble() };
                let pad = Pad {
                    entity,
                    device,
                    rumble,
                    reported_rumble: false,
                    reported_rumble_error: false,
                };
                raw.write(connection.clone().into());
                connections.write(connection);
                raw.write_batch(pad.state());
                println!("macos_gamepad connected={entity} rumble={rumble}");
                pads.insert(which, pad);
            }
            Event::GamepadRemoved { which, .. } => {
                if let Some(pad) = pads.remove(&which) {
                    let event =
                        GamepadConnectionEvent::new(pad.entity, GamepadConnection::Disconnected);
                    raw.write(event.clone().into());
                    connections.write(event);
                }
            }
            Event::GamepadRemapped { which, .. } => {
                if let Some(pad) = pads.get(&which) {
                    raw.write_batch(pad.state());
                }
            }
            Event::GamepadAxisMotion {
                which, axis, value, ..
            } => {
                if let Some(pad) = pads.get(&which) {
                    raw.write(axis_message(pad.entity, axis, value));
                }
            }
            Event::GamepadButtonDown { which, button, .. }
            | Event::GamepadButtonUp { which, button, .. } => {
                if let Some(pad) = pads.get(&which)
                    && let Some((_, button)) = BUTTONS.iter().find(|(from, _)| *from == button)
                {
                    let down = matches!(event, Event::GamepadButtonDown { .. });
                    raw.write(
                        RawGamepadButtonChangedEvent::new(
                            pad.entity,
                            *button,
                            f32::from(u8::from(down)),
                        )
                        .into(),
                    );
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn classified(world: &World, pad: Entity) -> bool {
    world.get::<crate::input::ClassifiedPad>(pad).is_some()
}

fn rumble(mut state: NonSendMut<Controllers>, mut requests: MessageReader<Rumble>) {
    for request in requests.read() {
        if let Some(pad) = state
            .pads
            .values_mut()
            .find(|p| p.entity == request.pad && p.rumble)
        {
            let (left, right) = request.motors;
            let duration = if (left, right) == (0, 0) { 0 } else { 120 };
            if let Err(error) =
                pad.device
                    .set_rumble(u16::from(left) * 257, u16::from(right) * 257, duration)
            {
                if !pad.reported_rumble_error {
                    warn!("macOS gamepad rumble failed for {}: {error}", request.pad);
                    pad.reported_rumble_error = true;
                }
            } else if duration != 0 && !pad.reported_rumble {
                println!("macos_gamepad rumble_started={}", request.pad);
                pad.reported_rumble = true;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sdl_trigger_uses_only_the_analog_trigger_button() {
        let pad = Entity::PLACEHOLDER;
        for (value, expected) in [(i16::MIN, 0.), (0, 0.), (i16::MAX, 1.)] {
            let RawGamepadEvent::Button(event) = axis_message(pad, Axis::TriggerRight, value)
            else {
                panic!("SDL trigger must become a Bevy trigger button");
            };
            assert_eq!(event.button, GamepadButton::RightTrigger2);
            assert_eq!(event.value, expected);
        }
    }

    #[test]
    fn sdl_stick_ranges_match_bevy() {
        let pad = Entity::PLACEHOLDER;
        for (axis, value, expected_axis, expected_value) in [
            (Axis::LeftX, i16::MIN, GamepadAxis::LeftStickX, -1.),
            (Axis::LeftY, i16::MIN, GamepadAxis::LeftStickY, 1.),
            (Axis::RightX, i16::MAX, GamepadAxis::RightStickX, 1.),
            (Axis::RightY, i16::MAX, GamepadAxis::RightStickY, -1.),
        ] {
            let RawGamepadEvent::Axis(event) = axis_message(pad, axis, value) else {
                panic!("SDL stick must become a Bevy stick axis");
            };
            assert_eq!(event.axis, expected_axis);
            assert_eq!(event.value, expected_value);
        }
    }
}
