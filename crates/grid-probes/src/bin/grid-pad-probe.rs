//! Gamepad probe on the desktop (docs/PAD_PROBE.md): what gilrs, the backend
//! of the game's Bevy gamepad input (Windows.Gaming.Input on Windows), reports
//! for the connected pads, and whether the original XInput pad preset's
//! controls (ActionMap/actionMapPreset4.xml, crates/grid-app/src/input.rs)
//! exist on them. `passive` opens no window.
//!
//!   grid-pad-probe passive [SECONDS]  identity, mapping, values at rest
//!   grid-pad-probe buttons            guided test in a small window: press each
//!                                     control when asked (the owner; about 90 s)
//!
//! The report goes to stdout; pipe it to a file under local/.
use gilrs::{Axis, Button, EventType, Gilrs};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// The controls the XInput pad preset binds (input.rs `xinput_source`), in
/// gilrs terms (bevy_gilrs converts them 1:1 to Bevy's GamepadButton /
/// GamepadAxis), with the Russian names the guided test shows.
const BUTTONS: [(Button, &str, &str); 16] = [
    (Button::South, "buttonA", "кнопку A (нижняя)"),
    (Button::East, "buttonB", "кнопку B (правая)"),
    (Button::West, "buttonX", "кнопку X (левая)"),
    (Button::North, "buttonY", "кнопку Y (верхняя)"),
    (Button::LeftTrigger, "buttonLeftShoulder", "левый бампер LB"),
    (Button::RightTrigger, "buttonRightShoulder", "правый бампер RB"),
    (Button::LeftTrigger2, "buttonLeftTrigger", "левый курок LT до упора"),
    (Button::RightTrigger2, "buttonRightTrigger", "правый курок RT до упора"),
    (Button::Select, "buttonBack", "кнопку Back / View"),
    (Button::Start, "buttonStart", "кнопку Start / Menu"),
    (Button::LeftThumb, "buttonLeftStick", "нажатие левого стика"),
    (Button::RightThumb, "buttonRightStick", "нажатие правого стика"),
    (Button::DPadUp, "buttonDPadUp", "крестовину вверх"),
    (Button::DPadDown, "buttonDPadDown", "крестовину вниз"),
    (Button::DPadLeft, "buttonDPadLeft", "крестовину влево"),
    (Button::DPadRight, "buttonDPadRight", "крестовину вправо"),
];

const AXES: [(Axis, &str, &str); 4] = [
    (Axis::LeftStickX, "analogLeftStickX", "левый стик влево и вправо до упора"),
    (Axis::LeftStickY, "analogLeftStickY", "левый стик вверх и вниз до упора"),
    (Axis::RightStickX, "analogRightStickX", "правый стик влево и вправо до упора"),
    (Axis::RightStickY, "analogRightStickY", "правый стик вверх и вниз до упора"),
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // As bevy_gilrs builds it: no default filters (no gilrs dead zone or
    // jitter filter), so the values are what the game reads.
    let mut gilrs = gilrs::GilrsBuilder::new()
        .with_default_filters(false)
        .build()
        .map_err(|e| format!("gilrs: {e}"))?;
    // Let the backend's thread enumerate the pads.
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(1500) {
        while gilrs.next_event().is_some() {}
        std::thread::sleep(Duration::from_millis(10));
    }
    println!("pad_probe version=2 filters=none backend={}", if cfg!(windows) { "wgi" } else { "platform" });
    let pads: Vec<_> = gilrs.gamepads().map(|(id, _)| id).collect();
    if pads.is_empty() {
        println!("pad_probe pads=0 (none connected, or the backend reports none)");
        return Ok(());
    }
    for &id in &pads {
        let pad = gilrs.gamepad(id);
        let uuid: String = pad.uuid().iter().map(|b| format!("{b:02x}")).collect();
        println!(
            "pad id={id} name={:?} os_name={:?} uuid={uuid} vendor={:04x?} product={:04x?} mapping_source={:?} map_name={:?} power={:?} ff={}",
            pad.name(),
            pad.os_name(),
            pad.vendor_id(),
            pad.product_id(),
            pad.mapping_source(),
            pad.map_name(),
            pad.power_info(),
            pad.is_ff_supported()
        );
        for (button, preset, _) in BUTTONS {
            println!(
                "pad id={id} preset={preset} gilrs={button:?} code={}",
                pad.button_code(button).map_or("absent".into(), |c| c.to_string())
            );
        }
        for (axis, preset, _) in AXES {
            println!(
                "pad id={id} preset={preset} gilrs={axis:?} code={}",
                pad.axis_code(axis).map_or("absent".into(), |c| c.to_string())
            );
        }
    }
    match args.first().map(String::as_str) {
        Some("buttons") => buttons(gilrs, pads[0])?,
        _ => {
            let seconds: f64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(3.);
            passive(&mut gilrs, &pads, seconds);
        }
    }
    Ok(())
}

/// Min, max and last of every axis and button value over `seconds`, nobody
/// touching the pad; plus every event seen.
fn passive(gilrs: &mut Gilrs, pads: &[gilrs::GamepadId], seconds: f64) {
    let mut range: BTreeMap<(usize, String), (f32, f32, f32)> = BTreeMap::new();
    let mut events = 0;
    let start = Instant::now();
    let mut samples = 0;
    while start.elapsed().as_secs_f64() < seconds {
        while let Some(event) = gilrs.next_event() {
            events += 1;
            if events <= 20 {
                println!("event id={} {:?}", event.id, event.event);
            }
        }
        for &id in pads {
            let pad = gilrs.gamepad(id);
            let mut note = |name: String, v: f32| {
                let e = range.entry((usize::from(id), name)).or_insert((v, v, v));
                e.0 = e.0.min(v);
                e.1 = e.1.max(v);
                e.2 = v;
            };
            for (axis, _, _) in AXES {
                note(format!("{axis:?}"), pad.value(axis));
            }
            for (button, _, _) in BUTTONS {
                note(
                    format!("{button:?}"),
                    pad.button_data(button).map_or(f32::NAN, |b| b.value()),
                );
            }
            // Raw codes, whatever the mapping calls them.
            for (code, data) in pad.state().axes() {
                note(format!("raw_axis_{code}"), data.value());
            }
        }
        samples += 1;
        std::thread::sleep(Duration::from_millis(10));
    }
    println!("rest samples={samples} seconds={seconds} events={events}");
    for ((id, name), (min, max, last)) in range {
        println!("rest id={id} {name} min={min:.4} max={max:.4} last={last:.4}");
    }
}

/// One step of the guided test.
struct Step {
    want: String,
    prompt: String,
}

/// Guided test for the owner (Russian prompts in the console and the window
/// title), in its own small window: Windows.Gaming.Input delivers readings
/// to the focused window's process only [I], so the window must stay in
/// front. First 3 s untouched (rest values), then each control for 4 s.
struct Guided {
    gilrs: Gilrs,
    id: gilrs::GamepadId,
    steps: Vec<Step>,
    index: usize,
    started: Option<Instant>,
    seen: BTreeMap<String, (f32, f32)>,
    window: Option<winit::window::Window>,
}

impl Guided {
    fn seconds(&self) -> u64 {
        if self.index == 0 { 3 } else { 4 }
    }

    fn begin(&mut self) {
        let step = &self.steps[self.index];
        println!("
>>> {}", step.prompt);
        if let Some(w) = &self.window {
            w.set_title(&format!("Тест геймпада: {}", step.prompt));
        }
        self.seen.clear();
        self.started = Some(Instant::now());
    }

    fn finish_step(&mut self) {
        let want = &self.steps[self.index].want;
        if self.seen.is_empty() {
            println!("step want={want} seen=nothing");
        }
        for (name, (min, max)) in &self.seen {
            println!("step want={want} seen={name} min={min:.4} max={max:.4}");
        }
        if self.index == 0 {
            // Values at rest while focused: what the game reads untouched.
            let pad = self.gilrs.gamepad(self.id);
            for (axis, _, _) in AXES {
                println!("rest_focused {axis:?}={:.4}", pad.value(axis));
            }
            for (button, _, _) in BUTTONS {
                println!(
                    "rest_focused {button:?}={}",
                    pad.button_data(button).map_or("no-event".into(), |b| format!("{:.4}", b.value()))
                );
            }
        }
    }
}

impl winit::application::ApplicationHandler for Guided {
    fn resumed(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        if self.window.is_none() {
            let attrs = winit::window::Window::default_attributes()
                .with_title("Тест геймпада: щёлкните здесь")
                .with_inner_size(winit::dpi::LogicalSize::new(900., 200.));
            self.window = event_loop.create_window(attrs).ok();
        }
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    }

    fn window_event(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        _: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            winit::event::WindowEvent::CloseRequested => event_loop.exit(),
            // The countdown starts once the window has the focus.
            winit::event::WindowEvent::Focused(true) if self.started.is_none() => self.begin(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &winit::event_loop::ActiveEventLoop) {
        while let Some(event) = self.gilrs.next_event() {
            if event.id != self.id || self.started.is_none() {
                continue;
            }
            let (name, v) = match event.event {
                EventType::ButtonChanged(b, v, c) => (format!("{b:?} code={c}"), v),
                EventType::AxisChanged(a, v, c) => (format!("{a:?} code={c}"), v),
                _ => continue,
            };
            let e = self.seen.entry(name).or_insert((v, v));
            e.0 = e.0.min(v);
            e.1 = e.1.max(v);
        }
        let Some(started) = self.started else {
            std::thread::sleep(Duration::from_millis(5));
            return;
        };
        if started.elapsed().as_secs() >= self.seconds() {
            self.finish_step();
            self.index += 1;
            if self.index == self.steps.len() {
                println!("
Готово. Спасибо! Окно можно закрыть.");
                event_loop.exit();
                return;
            }
            self.begin();
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn buttons(gilrs: Gilrs, id: gilrs::GamepadId) -> Result<(), Box<dyn std::error::Error>> {
    println!("Тест геймпада, около полутора минут. Щёлкните по окну «Тест геймпада», держите его активным");
    println!("и делайте то, что написано в заголовке окна и здесь.");
    let mut steps = vec![Step {
        want: "rest".into(),
        prompt: "Не трогайте геймпад".into(),
    }];
    steps.extend(BUTTONS.iter().map(|(b, preset, ru)| Step {
        want: format!("{b:?}/{preset}"),
        prompt: format!("Нажмите и держите {ru}"),
    }));
    steps.extend(AXES.iter().map(|(a, preset, ru)| Step {
        want: format!("{a:?}/{preset}"),
        prompt: format!("Отклоните {ru}"),
    }));
    let mut test = Guided {
        gilrs,
        id,
        steps,
        index: 0,
        started: None,
        seen: BTreeMap::new(),
        window: None,
    };
    winit::event_loop::EventLoop::new()?.run_app(&mut test)?;
    Ok(())
}
