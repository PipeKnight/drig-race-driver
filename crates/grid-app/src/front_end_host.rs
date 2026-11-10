//! The one-window front end (`grid-app --menu`, docs/FRONT_END_FLOW.md):
//! the event menu, the loading screen and the race in one process and one
//! window, Menu → Loading → Race → Loading → Menu, as the original's front
//! end and race share one window.
//!
//! - winit allows one event loop per process, so an outer host app owns the
//!   window and the event loop; it has no renderer (no wgpu backend) and no
//!   audio. Each screen is an inner Bevy app ([`hosted`]: no winit or log
//!   plugin) that the host builds on a background thread, attaches to the
//!   window and updates once per host frame. Leaving a screen drops its app:
//!   its world (entities, assets, GPU resources, audio sink) and its render
//!   thread go with it, so nothing carries from one race to the next.
//! - Attach: the inner primary window gets the outer window's raw handle
//!   (its renderer then creates its own surface on the same OS window), size,
//!   scale factor, focus and cursor position. Only one app may have a surface
//!   on the window, so the previous app is dropped before the next one's
//!   first update.
//! - Every host frame the outer window's input and window messages go to the
//!   attached app with the window entity remapped; size, focus and cursor go
//!   outer → inner (the user resizes the OS window), the title inner → outer
//!   (the race shows its status there). Close, create, destroy and file drop
//!   messages are not forwarded; closing the window sends the app an
//!   [`AppExit::Success`] and one last update, so its exit systems run
//!   (camera save, force feedback stop).
//! - Size (AUTHORED): an attached app's requested size (`[graphics]` width
//!   and height for the race, 1280x800 for the menu) is applied to the window
//!   unless the user resized it since the host last set it.
//! - The inner apps end as their own processes did: the menu with
//!   [`event_menu::MENU_GO`] (race) or success (Back: quit), the race with
//!   [`event_menu::BACK_TO_MENU`] or any other code, which ends the process.
//! - States for the screens built on this ([`FrontEnd`]) are kept in the host
//!   world; see docs/FRONT_END_FLOW.md "Port state machine".
//! - `GRID_FRONT_END=processes` keeps the older one-process-per-screen
//!   supervisor (`event_menu::supervise`). The career screen and Android
//!   still use their relaunch paths (docs/FRONT_END_FLOW.md).
//! - Gamepads (AUTHORED): the host has the process's one gamepad backend
//!   (`GilrsPlugin`; each gilrs instance leaves a thread behind when
//!   dropped, gilrs 0.11.2 `ff::server`) and forwards the pads' connection,
//!   button and axis messages to the attached app, each pad as an entity of
//!   that app. An app attached while a pad is connected gets its connection
//!   first, as gilrs reports the pads present at startup.
//! - GPU devices (AUTHORED): screens of one kind ([`Gpu`]) render with one
//!   device for the whole process, made by the first of them and reused
//!   (`RenderCreation::Manual`): each Vulkan device the NVIDIA driver makes
//!   leaves kernel objects behind when it is destroyed, so a device per
//!   screen grew the handle count with every race (docs/FRONT_END_FLOW.md section 5).
//! - Diagnostics `GRID_MENU_LOOPS=N` with `GRID_MENU_AUTO=TICKS`: the races
//!   end at the tick limit and go back to the menu; after N races the host
//!   prints the memory, handle and entity counts at each menu and a verdict
//!   (the smoke check that nothing grows from race to race).

use bevy::app::PluginGroupBuilder;
use bevy::render::settings::{RenderCreation, RenderResources};
use std::sync::Mutex;

/// The GPU device a hosted screen renders with: the menu and the loading
/// screen share one, the races another (the race's device settings and
/// DLAA's Vulkan extensions, `texture_upload::render_creation`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Gpu {
    Ui,
    Race,
}

/// The device of each [`Gpu`] kind, once a screen of that kind made it.
static DEVICES: Mutex<Vec<(Gpu, RenderResources)>> = Mutex::new(Vec::new());

/// The plugin group of an app the front end hosts: the host owns the event
/// loop (winit), the global log subscriber and the gamepad backend (gilrs);
/// the app renders with its kind's device if one was made ([`keep_device`]).
pub(crate) fn hosted(plugins: PluginGroupBuilder, gpu: Gpu) -> PluginGroupBuilder {
    let plugins = plugins
        .disable::<bevy::winit::WinitPlugin>()
        .disable::<bevy::log::LogPlugin>()
        .disable::<bevy::gilrs::GilrsPlugin>();
    let devices = DEVICES.lock().unwrap_or_else(|e| e.into_inner());
    match devices.iter().find(|(kind, _)| *kind == gpu) {
        // The other `RenderPlugin` settings of hosted apps are the defaults.
        Some((_, resources)) => plugins.set(bevy::render::RenderPlugin {
            render_creation: RenderCreation::Manual(resources.clone()),
            ..Default::default()
        }),
        None => plugins,
    }
}

/// Waits on every kept device so that wgpu frees what a dropped app left
/// (buffers, textures, pipelines): a kept device outlives its apps, and wgpu
/// reclaims dropped resources only when the device is polled.
#[cfg(not(target_os = "android"))]
pub(crate) fn reclaim_devices() {
    let devices = DEVICES.lock().unwrap_or_else(|e| e.into_inner());
    for (gpu, resources) in devices.iter() {
        if let Err(error) = resources.0.poll(bevy::render::render_resource::PollType::wait_indefinitely()) {
            eprintln!("grid-app: polling the {gpu:?} device failed: {error}");
        }
    }
}

#[cfg(not(target_os = "android"))]
/// Keeps a built app's device for the next screens of its kind, if none is
/// kept yet. The device is in the app's worlds after its plugins' `finish`.
pub(crate) fn keep_device(gpu: Gpu, app: &bevy::app::App) {
    use bevy::render::renderer::{
        RenderAdapter, RenderAdapterInfo, RenderDevice, RenderInstance, RenderQueue,
    };
    let mut devices = DEVICES.lock().unwrap_or_else(|e| e.into_inner());
    if devices.iter().any(|(kind, _)| *kind == gpu) {
        return;
    }
    let main = app.world();
    let Some(render) = app.get_sub_app(bevy::render::RenderApp).map(|a| a.world()) else {
        return;
    };
    let (Some(device), Some(queue), Some(info), Some(adapter), Some(instance)) = (
        main.get_resource::<RenderDevice>(),
        main.get_resource::<RenderQueue>(),
        main.get_resource::<RenderAdapterInfo>(),
        main.get_resource::<RenderAdapter>(),
        render.get_resource::<RenderInstance>(),
    ) else {
        return;
    };
    #[cfg(feature = "dlaa")]
    let Some(vulkan) =
        render.get_resource::<bevy::render::renderer::raw_vulkan_init::AdditionalVulkanFeatures>()
    else {
        return;
    };
    devices.push((
        gpu,
        RenderResources(
            device.clone(),
            queue.clone(),
            info.clone(),
            adapter.clone(),
            instance.clone(),
            #[cfg(feature = "dlaa")]
            vulkan.clone(),
        ),
    ));
}

#[cfg(not(target_os = "android"))]
pub use host::*;

#[cfg(not(target_os = "android"))]
mod host {
    use crate::event_menu;
    use bevy::app::{AppExit, PluginsState};
    use bevy::ecs::schedule::MainThreadExecutor;
    use bevy::input::gamepad::{
        GamepadConnection, GamepadConnectionEvent, RawGamepadAxisChangedEvent,
        RawGamepadButtonChangedEvent, RawGamepadEvent,
    };
    use std::collections::HashMap;
    use bevy::prelude::*;
    use bevy::window::{
        ExitCondition, PrimaryWindow, RawHandleWrapper, RawHandleWrapperHolder, WindowEvent,
        WindowPosition,
    };
    use std::path::PathBuf;
    use std::sync::mpsc::{self, Receiver, TryRecvError};
    use std::time::{Duration, Instant};

    /// The shortest time the loading screen stays up, the stat panel's
    /// `on_time` minimum [D, docs/FRONT_END_FLOW.md section 2]: a fast load
    /// does not flash it.
    const LOADING_MIN: Duration = Duration::from_millis(800);

    /// The screen the window shows, for the screens built on the host (the
    /// main-menu shell). `Loading` while the next screen is built.
    #[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub enum FrontEnd {
        /// Before the first screen is attached.
        #[default]
        Starting,
        Menu,
        Loading,
        Race,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Screen {
        Menu,
        /// The loading screen (`loading_screen.rs`), shown while the next
        /// screen is built.
        Loading,
        Race,
    }

    impl Screen {
        fn gpu(self) -> super::Gpu {
            match self {
                Screen::Menu | Screen::Loading => super::Gpu::Ui,
                Screen::Race => super::Gpu::Race,
            }
        }

        fn state(self) -> FrontEnd {
            match self {
                Screen::Menu => FrontEnd::Menu,
                Screen::Loading => FrontEnd::Loading,
                Screen::Race => FrontEnd::Race,
            }
        }
    }

    /// A built inner app, sent from the build thread.
    struct Built(App);
    // SAFETY: `App` is not `Send` only because its runner is a boxed
    // `FnOnce` without a `Send` bound; the runner is replaced with a
    // non-capturing function before sending. Its worlds are `Send`, and no
    // plugin of these apps (Bevy 0.19's and this crate's) inserts a
    // non-send resource, which would tie a world to its thread.
    unsafe impl Send for Built {}

    /// Builds a screen's app on a background thread up to its plugins'
    /// `finish`; `Host::attach` does the rest on the main thread.
    fn build(screen: Screen, args: Vec<String>) -> Receiver<Result<Built, String>> {
        let (tx, rx) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("front-end-build".into())
            .spawn(move || {
                let started = std::time::Instant::now();
                let result = (|| -> Result<App, Box<dyn std::error::Error>> {
                    let mut app = match screen {
                        Screen::Menu => event_menu::screen_app(&args, true)?,
                        Screen::Loading => crate::loading_screen::app(&args)?,
                        Screen::Race => crate::race_app(args, true)?
                            .ok_or("the race arguments start no race")?,
                    };
                    app.set_runner(|_| AppExit::Success);
                    while app.plugins_state() == PluginsState::Adding {
                        bevy::tasks::tick_global_task_pools_on_main_thread();
                        std::thread::yield_now();
                    }
                    // `cleanup` (the render thread) runs at attach, on the
                    // main thread (`Host::attach`).
                    app.finish();
                    super::keep_device(screen.gpu(), &app);
                    Ok(app)
                })();
                println!(
                    "front_end version=1 event=built screen={screen:?} ok={} seconds={:.2}",
                    result.is_ok(),
                    started.elapsed().as_secs_f64()
                );
                let _ = tx.send(result.map(Built).map_err(|e| e.to_string()));
            });
        if let Err(error) = spawned {
            let (tx, rx2) = mpsc::channel();
            let _ = tx.send(Err(format!("cannot start the build thread: {error}")));
            return rx2;
        }
        rx
    }

    struct Shown {
        app: App,
        screen: Screen,
        /// The inner app's primary window.
        window: Entity,
        /// Exited: no more updates; kept on screen until the next one.
        frozen: bool,
        /// Host pad entity -> this app's entity for it.
        pads: HashMap<Entity, Entity>,
    }

    struct Host {
        /// Flags every screen gets (`event_menu::passed_on`).
        flags: Vec<String>,
        dir: PathBuf,
        shown: Option<Shown>,
        building: Vec<(Screen, Receiver<Result<Built, String>>)>,
        /// Built, waiting to be attached.
        ready: Vec<(Screen, App)>,
        /// When the loading screen was attached.
        loading_since: Option<Instant>,
        /// The logical size the host last gave the window.
        adopted: Vec2,
        /// `GRID_SERIES_EXPERIMENTAL` was set by the user, not by the host.
        user_experimental: bool,
        loops: Option<Loops>,
        /// The launcher race that started this process, black until the
        /// window shows (`event_menu::spawn_menu`).
        outer: crate::transition::OuterReady,
        /// Inner updates drawn while the window is still hidden.
        hidden_frames: Option<u32>,
    }

    /// `GRID_MENU_LOOPS`: races to run and the samples at each menu.
    struct Loops {
        races: u32,
        done: u32,
        samples: Vec<Sample>,
    }

    #[derive(Clone, Copy)]
    struct Sample {
        private_mb: f64,
        handles: u32,
        host_entities: usize,
        threads: u32,
    }

    /// Outer window and gamepad messages of this frame, for the attached
    /// app, and the pads connected now (host entity, connection).
    #[derive(Resource, Default)]
    struct Forward {
        window: Vec<WindowEvent>,
        pads: Vec<RawGamepadEvent>,
        connected: Vec<(Entity, GamepadConnection)>,
    }

    fn collect(
        mut events: MessageReader<WindowEvent>,
        mut pads: MessageReader<RawGamepadEvent>,
        mut forward: ResMut<Forward>,
    ) {
        forward.window.extend(events.read().cloned());
        for event in pads.read() {
            if let RawGamepadEvent::Connection(c) = event {
                forward.connected.retain(|(pad, _)| *pad != c.gamepad);
                if matches!(c.connection, GamepadConnection::Connected { .. }) {
                    forward.connected.push((c.gamepad, c.connection.clone()));
                }
            }
            forward.pads.push(event.clone());
        }
    }

    /// `grid-app --menu ...` on the desktop. Returns the exit code.
    pub fn run(args: &[String]) -> Result<i32, Box<dyn std::error::Error>> {
        let flags = event_menu::passed_on(args);
        let dir = flags
            .windows(2)
            .rev()
            .find(|w| w[0] == "--data-dir")
            .map(|w| PathBuf::from(&w[1]))
            .unwrap_or_else(crate::settings::default_data_dir);
        let loops = std::env::var("GRID_MENU_LOOPS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|&n| n > 0);
        if loops.is_some() && event_menu::auto_ticks().is_none() {
            return Err("GRID_MENU_LOOPS needs GRID_MENU_AUTO=TICKS".into());
        }
        let size = Vec2::new(1280., 800.);
        // Started by a launcher race (the hand-off, docs/SCREEN_FLOW.md
        // section 2.3): the window opens hidden where the race's was and
        // shows once the first screen has drawn ([`Host::reveal`]).
        let outer = crate::transition::OuterReady::from_env();
        let hidden = outer.waiting();
        let position = match crate::transition::window_at() {
            Some(at) if hidden => WindowPosition::At(at),
            _ => WindowPosition::default(),
        };
        let mut app = App::new();
        // The host only owns the window, the event loop and the input
        // (gamepads included); the attached app renders. No renderer, assets
        // beyond the cursor's images or audio here (each logs or opens
        // devices).
        #[cfg(feature = "profile")]
        let log = bevy::log::LogPlugin {
            custom_layer: |_| Some(Box::new(crate::span_profile::SpanProfile)),
            ..default()
        };
        #[cfg(not(feature = "profile"))]
        let log = bevy::log::LogPlugin::default();
        app.add_plugins((
            bevy::app::PanicHandlerPlugin,
            log,
            bevy::app::TaskPoolPlugin::default(),
            bevy::diagnostic::FrameCountPlugin,
            bevy::time::TimePlugin,
            bevy::input::InputPlugin,
            WindowPlugin {
                primary_window: Some(Window {
                    title: "GRID Reassembly".into(),
                    resolution: (size.x as u32, size.y as u32).into(),
                    position,
                    visible: !hidden,
                    ..default()
                }),
                exit_condition: ExitCondition::OnAllClosed,
                ..default()
            },
            bevy::a11y::AccessibilityPlugin,
            bevy::app::TerminalCtrlCHandlerPlugin,
            bevy::asset::AssetPlugin::default(),
            bevy::winit::WinitPlugin::default(),
            bevy::gilrs::GilrsPlugin,
            bevy::state::app::StatesPlugin,
        ))
        // The winit cursor system reads the image assets (custom cursors).
        .init_asset::<Image>()
        .init_state::<FrontEnd>()
        .init_resource::<Forward>()
        .add_systems(Update, (collect, drive).chain());
        let mut host = Host {
            flags,
            dir,
            shown: None,
            building: Vec::new(),
            ready: Vec::new(),
            loading_since: None,
            adopted: size,
            user_experimental: std::env::var_os("GRID_SERIES_EXPERIMENTAL").is_some(),
            loops: loops.map(|races| Loops {
                races,
                done: 0,
                samples: Vec::new(),
            }),
            outer,
            hidden_frames: hidden.then_some(0),
        };
        host.start(Screen::Menu);
        app.world_mut().insert_non_send(host);
        println!("front_end version=1 event=open window=one");
        Ok(match app.run() {
            AppExit::Success => 0,
            AppExit::Error(code) => i32::from(code.get()),
        })
    }

    /// The host's frame: finished builds, the attached app's update and the
    /// screen changes its exit asks for.
    fn drive(world: &mut World) {
        let (events, pads, connected) = {
            let mut forward = world.resource_mut::<Forward>();
            let window = std::mem::take(&mut forward.window);
            let pads = std::mem::take(&mut forward.pads);
            (window, pads, forward.connected.clone())
        };
        let Ok(outer) = world
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(world)
        else {
            return;
        };
        let Some(mut host) = world.remove_non_send::<Host>() else {
            return;
        };
        let exit = host.frame(world, outer, &events, &pads, &connected);
        world.insert_non_send(host);
        if let Some(code) = exit {
            world.write_message(if code == 0 {
                AppExit::Success
            } else {
                AppExit::from_code(code.clamp(1, 255) as u8)
            });
        }
    }

    impl Host {
        fn start(&mut self, screen: Screen) {
            let mut args = self.flags.clone();
            if screen == Screen::Race {
                // An experimental series runs with the gate (none since F420).
                if !self.user_experimental {
                    if event_menu::experimental(&self.dir) {
                        // SAFETY: set on the main thread before the build
                        // thread starts; no thread reads the environment
                        // through C at this time (std's own lock covers Rust
                        // readers).
                        unsafe { std::env::set_var("GRID_SERIES_EXPERIMENTAL", "1") };
                    } else {
                        // SAFETY: as above.
                        unsafe { std::env::remove_var("GRID_SERIES_EXPERIMENTAL") };
                    }
                }
                args.push("--settings".into());
                args.push(self.dir.join(event_menu::PROFILE).display().to_string());
                args.push("--from-menu".into());
                if let Some(ticks) = event_menu::auto_ticks() {
                    args.push("--drive-ticks".into());
                    args.push(ticks.to_string());
                }
            }
            println!("front_end version=1 event=build screen={screen:?}");
            // The loading screen and the screen behind it build at the same
            // time; the loading screen is small and attaches first.
            self.building
                .push((Screen::Loading, build(Screen::Loading, self.flags.clone())));
            self.building.push((screen, build(screen, args)));
        }

        /// Finished builds into `ready`. A failed loading screen is skipped
        /// (the next screen attaches without it); any other failure ends the
        /// process.
        fn poll_builds(&mut self) -> Result<(), String> {
            let mut index = 0;
            while index < self.building.len() {
                let screen = self.building[index].0;
                let result = match self.building[index].1.try_recv() {
                    Err(TryRecvError::Empty) => {
                        index += 1;
                        continue;
                    }
                    Ok(result) => result,
                    Err(TryRecvError::Disconnected) => Err("the build thread ended".into()),
                };
                self.building.remove(index);
                match result {
                    Ok(Built(app)) => self.ready.push((screen, app)),
                    Err(error) if screen == Screen::Loading => {
                        eprintln!("grid-app: no loading screen: {error}");
                    }
                    Err(error) => return Err(format!("the {screen:?} build failed: {error}")),
                }
            }
            Ok(())
        }

        /// The ready app to attach now, if any: the loading screen as soon as
        /// it is built; the next screen once no loading screen is still
        /// building and the shown one has been up for [`LOADING_MIN`].
        fn next_ready(&mut self) -> Option<(Screen, App)> {
            let take = |ready: &mut Vec<(Screen, App)>, i: usize| Some(ready.remove(i));
            if let Some(i) = self.ready.iter().position(|(s, _)| *s == Screen::Loading) {
                return take(&mut self.ready, i);
            }
            if self.building.iter().any(|(s, _)| *s == Screen::Loading) {
                return None;
            }
            let loading_up = self.shown.as_ref().is_some_and(|s| s.screen == Screen::Loading);
            if loading_up && self.loading_since.is_some_and(|t| t.elapsed() < LOADING_MIN) {
                return None;
            }
            let i = self.ready.iter().position(|(s, _)| *s != Screen::Loading)?;
            take(&mut self.ready, i)
        }

        /// One host frame; `Some(code)` ends the process with that code.
        fn frame(
            &mut self,
            world: &mut World,
            outer: Entity,
            events: &[WindowEvent],
            pads: &[RawGamepadEvent],
            connected: &[(Entity, GamepadConnection)],
        ) -> Option<i32> {
            let closing = events
                .iter()
                .any(|e| matches!(e, WindowEvent::WindowCloseRequested(_)));
            if closing {
                if let Some(mut shown) = self.shown.take()
                    && !shown.frozen
                {
                    shown.app.world_mut().write_message(AppExit::Success);
                    shown.app.update();
                }
                println!("front_end version=1 event=close");
                return None;
            }
            if let Err(error) = self.poll_builds() {
                eprintln!("grid-app: {error}");
                return Some(1);
            }
            if let Some(handle) = world.get::<RawHandleWrapper>(outer).cloned()
                && let Some((screen, app)) = self.next_ready()
            {
                self.loading_since = (screen == Screen::Loading).then(Instant::now);
                // The old app's surface goes before the new one's is made.
                self.shown = None;
                super::reclaim_devices();
                match self.attach(world, outer, handle, screen, app, connected) {
                    Ok(shown) => self.shown = Some(shown),
                    Err(error) => {
                        eprintln!("grid-app: {error}");
                        return Some(1);
                    }
                }
                if screen == Screen::Menu
                    && let Some(code) = self.sample(world)
                {
                    return Some(code);
                }
            }
            let shown = self.shown.as_mut()?;
            if shown.frozen {
                return None;
            }
            forward(shown.app.world_mut(), shown.window, events);
            forward_pads(shown, pads);
            sync_in(world, outer, shown);
            shown.app.update();
            sync_out(world, outer, shown);
            self.reveal(world, outer);
            let shown = self.shown.as_mut()?;
            let code = match shown.app.should_exit()? {
                AppExit::Success => 0,
                AppExit::Error(code) => i32::from(code.get()),
            };
            shown.frozen = true;
            let screen = shown.screen;
            println!("front_end version=1 event=exit screen={screen:?} code={code}");
            match (screen, code) {
                (Screen::Menu, event_menu::MENU_GO) => self.start(Screen::Race),
                (Screen::Race, event_menu::BACK_TO_MENU) => {
                    // Free the race before the menu is built.
                    self.shown = None;
                    super::reclaim_devices();
                    match &mut self.loops {
                        Some(loops) => loops.done += 1,
                        // GRID_MENU_AUTO_RACES; the loops smoke ends by
                        // its own count instead.
                        None => {
                            event_menu::HOST_ROUND.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                    self.start(Screen::Menu);
                }
                _ => return Some(code),
            }
            world
                .resource_mut::<NextState<FrontEnd>>()
                .set(FrontEnd::Loading);
            None
        }

        /// A window opened hidden shows on the first screen's
        /// `transition::SHOW_FRAME`th update, set in Update so bevy_winit
        /// applies it in this frame's Last (the F773 window-show hang); the
        /// launcher race's marker follows a frame later, once it is shown.
        fn reveal(&mut self, world: &mut World, outer: Entity) {
            let Some(frames) = self.hidden_frames.as_mut() else {
                return;
            };
            *frames += 1;
            if *frames == crate::transition::SHOW_FRAME
                && let Some(mut window) = world.get_mut::<Window>(outer)
            {
                window.visible = true;
                window.focused = true;
            } else if *frames > crate::transition::SHOW_FRAME {
                self.hidden_frames = None;
                self.outer.write();
                println!("front_end version=1 event=handoff_ready");
            }
        }

        fn attach(
            &mut self,
            world: &mut World,
            outer: Entity,
            handle: RawHandleWrapper,
            screen: Screen,
            mut app: App,
            connected: &[(Entity, GamepadConnection)],
        ) -> Result<Shown, String> {
            let inner = app
                .world_mut()
                .query_filtered::<Entity, With<PrimaryWindow>>()
                .single(app.world())
                .map_err(|e| format!("the {screen:?} app has no window: {e}"))?;
            let requested = app
                .world()
                .get::<Window>(inner)
                .map(|w| w.resolution.size())
                .unwrap_or(self.adopted);
            // The loading screen keeps the size of the screen before it.
            if screen != Screen::Loading
                && let Some(mut window) = world.get_mut::<Window>(outer)
            {
                let now = window.resolution.size();
                let untouched = (now - self.adopted).abs().max_element() < 1.;
                if untouched && (requested - now).abs().max_element() >= 1. {
                    window.resolution.set(requested.x, requested.y);
                    self.adopted = requested;
                }
            }
            // The executor for the systems that must run on the main thread
            // (Bevy's `NonSendMarker`, e.g. `tick_global_task_pools` in
            // `Last`) is the creating thread's: made on the build thread, it
            // would never be ticked and the first update would wait for ever.
            // So it is made here, and `cleanup` (which gives the render world
            // a copy and starts the render thread) runs after it.
            if app.world().contains_resource::<MainThreadExecutor>() {
                app.insert_resource(MainThreadExecutor::new());
            }
            app.cleanup();
            let inner_world = app.world_mut();
            if let Some(holder) = inner_world.get::<RawHandleWrapperHolder>(inner) {
                *holder.0.lock().unwrap_or_else(|e| e.into_inner()) = Some(handle.clone());
            }
            inner_world.entity_mut(inner).insert(handle);
            let mut shown = Shown {
                app,
                screen,
                window: inner,
                frozen: false,
                pads: HashMap::new(),
            };
            // The pads connected now, as gilrs reports them at startup.
            let connections: Vec<_> = connected
                .iter()
                .map(|(pad, c)| {
                    RawGamepadEvent::Connection(GamepadConnectionEvent::new(*pad, c.clone()))
                })
                .collect();
            forward_pads(&mut shown, &connections);
            sync_in(world, outer, &mut shown);
            world
                .resource_mut::<NextState<FrontEnd>>()
                .set(screen.state());
            println!("front_end version=1 event=attach screen={screen:?}");
            Ok(shown)
        }

        /// `GRID_MENU_LOOPS`: a sample at each menu; the verdict after the
        /// last race.
        fn sample(&mut self, world: &mut World) -> Option<i32> {
            let loops = self.loops.as_mut()?;
            let (private_mb, handles, threads) = usage();
            let host_entities = world.query::<Entity>().iter(world).count();
            loops.samples.push(Sample {
                private_mb,
                handles,
                host_entities,
                threads,
            });
            println!(
                "front_end_loops version=1 event=menu races={} private_mb={private_mb:.0} handles={handles} threads={threads} host_entities={host_entities} names={} handle_types={}",
                loops.done,
                self::threads().1,
                handle_types()
            );
            if loops.done < loops.races {
                // The menu's GRID_MENU_AUTO presses Continue after 3 s.
                return None;
            }
            Some(verdict(&loops.samples))
        }
    }

    /// The verdict over the samples at each menu (AUTHORED limits): the first
    /// races fill process-wide caches (shader and texture caches, allocator
    /// pools of the kept devices), so growth is measured from the middle menu
    /// to the last one; a leak per race still shows there.
    fn verdict(samples: &[Sample]) -> i32 {
        const MB_PER_RACE: f64 = 48.;
        const HANDLES_PER_RACE: f64 = 40.;
        // Threads: none may outlive its screen. A pool or driver thread may
        // still be ending at the menu, so the last count may exceed the
        // base's by this much.
        const THREADS_SLACK: u32 = 2;
        let list = |f: &dyn Fn(&Sample) -> String| {
            samples.iter().map(f).collect::<Vec<_>>().join(",")
        };
        // Growth is measured over the second half of the races: the first
        // races fill process-wide caches and the kept devices' allocators
        // (docs/FRONT_END_FLOW.md section 5: 1.47 to 1.79 GB over races 1-6, then 1.74-1.91 GB to race 12).
        let mid = (samples.len() / 2).max(1);
        let base = samples.get(mid).or(samples.first()).copied();
        let last = samples.last().copied();
        let (growth_mb, growth_handles, races) = match (base, last) {
            (Some(b), Some(l)) => (
                l.private_mb - b.private_mb,
                f64::from(l.handles) - f64::from(b.handles),
                (samples.len().saturating_sub(mid + 1)).max(1) as f64,
            ),
            _ => (0., 0., 1.),
        };
        let entities_same = samples.windows(2).all(|w| w[0].host_entities == w[1].host_entities);
        let threads_flat = match (base, last) {
            (Some(b), Some(l)) => l.threads <= b.threads + THREADS_SLACK,
            _ => true,
        };
        let pass = samples.len() >= 5
            && growth_mb <= MB_PER_RACE * races
            && growth_handles <= HANDLES_PER_RACE * races
            && entities_same
            && threads_flat;
        println!(
            "front_end_loops version=1 races={} private_mb=[{}] handles=[{}] threads=[{}] host_entities=[{}] from_menu={mid} growth_mb={growth_mb:.0} growth_handles={growth_handles:.0} limit_mb_per_race={MB_PER_RACE} verdict={}",
            samples.len().saturating_sub(1),
            list(&|s| format!("{:.0}", s.private_mb)),
            list(&|s| s.handles.to_string()),
            list(&|s| s.threads.to_string()),
            list(&|s| s.host_entities.to_string()),
            if pass { "pass" } else { "fail" }
        );
        if pass { 0 } else { 1 }
    }

    /// The outer window's messages for the attached app, with its window.
    fn forward(world: &mut World, inner: Entity, events: &[WindowEvent]) {
        use WindowEvent as E;
        for event in events {
            let mut event = event.clone();
            macro_rules! with_window {
                ($m:expr) => {{
                    $m.window = inner;
                    world.write_message($m.clone());
                }};
            }
            match &mut event {
                E::CursorEntered(m) => with_window!(m),
                E::CursorLeft(m) => with_window!(m),
                E::CursorMoved(m) => with_window!(m),
                E::MouseButtonInput(m) => with_window!(m),
                E::MouseWheel(m) => with_window!(m),
                E::TouchInput(m) => with_window!(m),
                E::KeyboardInput(m) => with_window!(m),
                E::WindowFocused(m) => with_window!(m),
                E::WindowOccluded(m) => with_window!(m),
                E::WindowResized(m) => with_window!(m),
                E::WindowScaleFactorChanged(m) => with_window!(m),
                E::WindowBackendScaleFactorChanged(m) => with_window!(m),
                E::WindowMoved(m) => with_window!(m),
                E::WindowThemeChanged(m) => with_window!(m),
                E::Ime(m) => {
                    match m {
                        bevy::window::Ime::Preedit { window, .. }
                        | bevy::window::Ime::Commit { window, .. }
                        | bevy::window::Ime::Enabled { window }
                        | bevy::window::Ime::Disabled { window } => *window = inner,
                    }
                    world.write_message(m.clone());
                }
                E::MouseMotion(m) => {
                    world.write_message(*m);
                }
                E::PinchGesture(m) => {
                    world.write_message(*m);
                }
                E::RotationGesture(m) => {
                    world.write_message(*m);
                }
                E::DoubleTapGesture(m) => {
                    world.write_message(*m);
                }
                E::PanGesture(m) => {
                    world.write_message(*m);
                }
                E::KeyboardFocusLost(m) => {
                    world.write_message(m.clone());
                }
                E::AppLifecycle(m) => {
                    world.write_message(*m);
                }
                E::RequestRedraw(m) => {
                    world.write_message(m.clone());
                }
                E::FileDragAndDrop(_)
                | E::WindowCloseRequested(_)
                | E::WindowCreated(_)
                | E::WindowDestroyed(_) => continue,
            }
            world.write_message(event);
        }
    }

    /// Host gamepad messages for the attached app, each pad as an entity of
    /// that app (spawned at its connection), written raw and typed as
    /// `bevy_gilrs` writes them.
    fn forward_pads(shown: &mut Shown, events: &[RawGamepadEvent]) {
        let world = shown.app.world_mut();
        for event in events {
            match event {
                RawGamepadEvent::Connection(c) => {
                    let connected = matches!(c.connection, GamepadConnection::Connected { .. });
                    let pad = match shown.pads.get(&c.gamepad) {
                        Some(&pad) => pad,
                        None if connected => {
                            let pad = world.spawn_empty().id();
                            shown.pads.insert(c.gamepad, pad);
                            pad
                        }
                        None => continue,
                    };
                    let event = GamepadConnectionEvent::new(pad, c.connection.clone());
                    world.write_message(RawGamepadEvent::from(event.clone()));
                    world.write_message(event);
                }
                RawGamepadEvent::Button(b) => {
                    let Some(&pad) = shown.pads.get(&b.gamepad) else {
                        continue;
                    };
                    let event = RawGamepadButtonChangedEvent::new(pad, b.button, b.value);
                    world.write_message(RawGamepadEvent::from(event));
                    world.write_message(event);
                }
                RawGamepadEvent::Axis(a) => {
                    let Some(&pad) = shown.pads.get(&a.gamepad) else {
                        continue;
                    };
                    let event = RawGamepadAxisChangedEvent::new(pad, a.axis, a.value);
                    world.write_message(RawGamepadEvent::from(event));
                    world.write_message(event);
                }
            }
        }
    }

    /// Outer → inner: size, scale factor, focus and cursor.
    fn sync_in(world: &World, outer: Entity, shown: &mut Shown) {
        let Some(from) = world.get::<Window>(outer) else {
            return;
        };
        let Some(mut to) = shown.app.world_mut().get_mut::<Window>(shown.window) else {
            return;
        };
        if to.resolution.physical_size() != from.resolution.physical_size()
            || to.resolution.scale_factor() != from.resolution.scale_factor()
        {
            to.resolution = from.resolution.clone();
        }
        if to.focused != from.focused {
            to.focused = from.focused;
        }
        let cursor = from.physical_cursor_position();
        if to.physical_cursor_position() != cursor {
            to.set_physical_cursor_position(cursor.map(|p| p.as_dvec2()));
        }
    }

    /// Inner → outer: the title.
    fn sync_out(world: &mut World, outer: Entity, shown: &Shown) {
        let Some(title) = shown
            .app
            .world()
            .get::<Window>(shown.window)
            .map(|w| w.title.clone())
        else {
            return;
        };
        if let Some(mut window) = world.get_mut::<Window>(outer)
            && window.title != title
        {
            window.title = title;
        }
    }

    /// Private bytes (MB), handle and thread counts of this process.
    #[cfg(windows)]
    fn usage() -> (f64, u32, u32) {
        #[repr(C)]
        #[derive(Default)]
        struct Counters {
            cb: u32,
            page_fault_count: u32,
            peak_working_set_size: usize,
            working_set_size: usize,
            quota_peak_paged_pool_usage: usize,
            quota_paged_pool_usage: usize,
            quota_peak_non_paged_pool_usage: usize,
            quota_non_paged_pool_usage: usize,
            pagefile_usage: usize,
            peak_pagefile_usage: usize,
            private_usage: usize,
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetCurrentProcess() -> isize;
            fn K32GetProcessMemoryInfo(process: isize, counters: *mut Counters, cb: u32) -> i32;
            fn GetProcessHandleCount(process: isize, count: *mut u32) -> i32;
        }
        let mut counters = Counters {
            cb: std::mem::size_of::<Counters>() as u32,
            ..Default::default()
        };
        let mut handles = 0u32;
        // SAFETY: the pseudo handle of this process and out-pointers to
        // locals of the sizes the calls are given.
        unsafe {
            let process = GetCurrentProcess();
            K32GetProcessMemoryInfo(process, &mut counters, counters.cb);
            GetProcessHandleCount(process, &mut handles);
        }
        (counters.private_usage as f64 / 1048576., handles, threads().0)
    }

    #[cfg(not(windows))]
    fn usage() -> (f64, u32, u32) {
        (0., 0, 0)
    }

    #[cfg(not(windows))]
    fn threads() -> (u32, String) {
        (0, String::new())
    }

    #[cfg(not(windows))]
    fn handle_types() -> String {
        String::new()
    }

    /// This process's handles by object type (`Event:120,File:40,...`), for
    /// finding a handle that outlives its screen.
    #[cfg(windows)]
    fn handle_types() -> String {
        #[repr(C)]
        struct Entry {
            object: usize,
            process: usize,
            handle: usize,
            access: u32,
            back_trace: u16,
            type_index: u16,
            attributes: u32,
            reserved: u32,
        }
        #[link(name = "ntdll")]
        unsafe extern "system" {
            fn NtQuerySystemInformation(class: u32, info: *mut u8, len: u32, ret: *mut u32) -> i32;
            fn NtQueryObject(handle: usize, class: u32, info: *mut u8, len: u32, ret: *mut u32) -> i32;
        }
        const SYSTEM_EXTENDED_HANDLE_INFORMATION: u32 = 64;
        const OBJECT_TYPE_INFORMATION: u32 = 2;
        let me = std::process::id() as usize;
        let mut buffer = vec![0u64; 1 << 17];
        // SAFETY: the buffer is 8-aligned and its byte length is passed; it
        // grows until the call fits. The entries read are within the count
        // the call wrote. Type names are read from this process's own
        // handles into a buffer of the length passed.
        unsafe {
            loop {
                let bytes = (buffer.len() * 8) as u32;
                let mut needed = 0;
                let status = NtQuerySystemInformation(
                    SYSTEM_EXTENDED_HANDLE_INFORMATION,
                    buffer.as_mut_ptr().cast(),
                    bytes,
                    &mut needed,
                );
                if status == 0 {
                    break;
                }
                // STATUS_INFO_LENGTH_MISMATCH
                if status as u32 != 0xC000_0004 || buffer.len() > 1 << 25 {
                    return String::new();
                }
                buffer.resize(buffer.len().max(needed as usize / 8 + 1024) * 2, 0);
            }
            let count = buffer[0] as usize;
            let entries = buffer.as_ptr().add(2).cast::<Entry>();
            let mut by_type = std::collections::BTreeMap::<u16, (u32, usize)>::new();
            for i in 0..count {
                let entry = &*entries.add(i);
                if entry.process == me {
                    by_type.entry(entry.type_index).or_insert((0, entry.handle)).0 += 1;
                }
            }
            let mut named = std::collections::BTreeMap::<String, u32>::new();
            let mut info = vec![0u64; 128];
            for (index, (n, handle)) in by_type {
                let mut ret = 0;
                let mut name = format!("type{index}");
                if NtQueryObject(handle, OBJECT_TYPE_INFORMATION, info.as_mut_ptr().cast(), 1024, &mut ret) == 0 {
                    // UNICODE_STRING: length in bytes, then the buffer pointer.
                    let len = (info[0] & 0xffff) as usize / 2;
                    let text = info[1] as *const u16;
                    if !text.is_null() && len > 0 {
                        name = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
                    }
                }
                *named.entry(name).or_default() += n;
            }
            named
                .iter()
                .map(|(name, n)| format!("{name}:{n}"))
                .collect::<Vec<_>>()
                .join(",")
        }
    }

    /// Threads of this process (Toolhelp snapshot) and their count by name
    /// (`name:count,...`, unnamed as `?`), for finding a thread that
    /// outlives its screen.
    #[cfg(windows)]
    fn threads() -> (u32, String) {
        #[repr(C)]
        struct ThreadEntry {
            size: u32,
            usage: u32,
            thread_id: u32,
            owner_process_id: u32,
            base_priority: i32,
            delta_priority: i32,
            flags: u32,
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn CreateToolhelp32Snapshot(flags: u32, process: u32) -> isize;
            fn Thread32First(snapshot: isize, entry: *mut ThreadEntry) -> i32;
            fn Thread32Next(snapshot: isize, entry: *mut ThreadEntry) -> i32;
            fn CloseHandle(handle: isize) -> i32;
            fn OpenThread(access: u32, inherit: i32, thread_id: u32) -> isize;
            fn GetThreadDescription(thread: isize, description: *mut *mut u16) -> i32;
            fn LocalFree(memory: *mut u16) -> *mut u16;
        }
        const TH32CS_SNAPTHREAD: u32 = 4;
        const THREAD_QUERY_LIMITED_INFORMATION: u32 = 0x0800;
        let mut names = std::collections::BTreeMap::<String, u32>::new();
        let me = std::process::id();
        let mut entry = ThreadEntry {
            size: std::mem::size_of::<ThreadEntry>() as u32,
            usage: 0,
            thread_id: 0,
            owner_process_id: 0,
            base_priority: 0,
            delta_priority: 0,
            flags: 0,
        };
        let mut count = 0;
        // SAFETY: a snapshot handle checked for INVALID_HANDLE_VALUE, walked
        // with an entry of the size it declares, then closed.
        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
            if snapshot == -1 {
                return (0, String::new());
            }
            let mut more = Thread32First(snapshot, &mut entry) != 0;
            while more {
                if entry.owner_process_id == me {
                    count += 1;
                    let mut name = String::from("?");
                    let thread = OpenThread(THREAD_QUERY_LIMITED_INFORMATION, 0, entry.thread_id);
                    if thread != 0 {
                        let mut text: *mut u16 = std::ptr::null_mut();
                        // HRESULT >= 0 is success; the text is NUL-terminated
                        // and freed with LocalFree.
                        if GetThreadDescription(thread, &mut text) >= 0 && !text.is_null() {
                            let mut len = 0;
                            while *text.add(len) != 0 {
                                len += 1;
                            }
                            let read = String::from_utf16_lossy(std::slice::from_raw_parts(text, len));
                            if !read.is_empty() {
                                name = read;
                            }
                            LocalFree(text);
                        }
                        CloseHandle(thread);
                    }
                    *names.entry(name).or_default() += 1;
                }
                more = Thread32Next(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
        }
        let names = names
            .iter()
            .map(|(name, n)| format!("{}:{n}", name.replace([' ', ','], "_")))
            .collect::<Vec<_>>()
            .join(",");
        (count, names)
    }
}
