//! Default-off bounded C/R input diagnostic using the ordinary action handlers.
use bevy::{diagnostic::{FrameCount, update_frame_count}, input::InputSystems, prelude::*};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Key { ChangeView, Reset }
impl Key {
    fn code(self) -> KeyCode {
        match self { Self::ChangeView => KeyCode::KeyC, Self::Reset => KeyCode::KeyR }
    }
}
#[derive(Clone, Copy, Debug)]
struct Event { frame: u32, key: Key }
#[derive(Resource)]
pub(crate) struct Script { events: Vec<Event>, next: usize }
impl Script {
    pub(crate) fn parse(text: &str) -> Result<Self, &'static str> {
        let mut events = Vec::new();
        let mut resets = 0;
        for token in text.split(',') {
            if events.len() == 8 { return Err("window transitions allow at most8events"); }
            let (frame, key) = token.split_once(':').ok_or("window transitions need FRAME:C or FRAME:R")?;
            let frame: u32 = frame.parse().map_err(|_| "invalid transition frame")?;
            if !(2..=5000).contains(&frame) { return Err("transition frame must be2..5000"); }
            if events.last().is_some_and(|e: &Event| frame < e.frame + 2) {
                return Err("transition frames must increase with at least one release frame");
            }
            let key = match key {
                "C" => Key::ChangeView,
                "R" => { resets += 1; Key::Reset },
                _ => return Err("window transitions accept only C or R"),
            };
            if resets > 1 { return Err("window transitions allow at most one reset"); }
            events.push(Event { frame, key });
        }
        if events.is_empty() { return Err("empty window transition script"); }
        Ok(Self { events, next: 0 })
    }
    pub(crate) fn fits(&self, ticks: u64, fps: u32) -> bool {
        if !(30..=300).contains(&fps) { return false; }
        let frame = self.events.last().expect("nonempty parser").frame;
        let required = (f64::from(frame) / f64::from(fps)
            / f64::from(grid_tools::source_vehicle::DELTA)).ceil() as u64 + 8;
        required < ticks
    }
    fn poll(&mut self, frame: u32) -> Result<Option<Key>, &'static str> {
        let Some(event) = self.events.get(self.next) else { return Ok(None); };
        if event.frame < frame { return Err("window transition frame was missed"); }
        if event.frame != frame { return Ok(None); }
        self.next += 1;
        Ok(Some(event.key))
    }
}
fn pulse(frame: u32, script: &mut Script, keys: &mut ButtonInput<KeyCode>) -> Result<Option<Key>, &'static str> {
    keys.release(KeyCode::KeyC);
    keys.release(KeyCode::KeyR);
    let event = script.poll(frame)?;
    if let Some(key) = event { keys.press(key.code()); }
    Ok(event)
}
pub(crate) fn install(app: &mut App, script: Script) {
    println!("window_transitions enabled=true events={:?} clock=expected-extract-frame raw_input=CR bounded=true", script.events);
    app.insert_resource(script)
        .add_systems(PreUpdate, inject.after(InputSystems).before(super::input::sample))
        .add_systems(Last, observe.before(update_frame_count));
}
fn inject(
    frame: Res<FrameCount>, mut script: ResMut<Script>,
    mut keys: ResMut<ButtonInput<KeyCode>>, driving: Res<super::Driving>,
    mut exit: MessageWriter<AppExit>,
) {
    // These two keys belong to this opt-in offscreen script only.
    // Release after one update; normal InputSystems clears transient edges.
    // FrameCount increments once in Last; Extract sees the next value.
    let extract_frame = frame.0.saturating_add(1);
    match pulse(extract_frame, &mut script, &mut keys) {
        Ok(Some(key)) => {
            println!("window_transition_inject frame={extract_frame} update_frame={} key={key:?} before_segment={} before_tick={}", frame.0, driving.segment, driving.runtime.ticks);
        }
        Ok(None) => {}
        Err(error) => { error!("{error}"); exit.write(AppExit::error()); }
    }
}
fn observe(
    frame: Res<FrameCount>, script: Res<Script>, driving: Res<super::Driving>,
    cameras: Option<Res<super::camera::Cameras>>,
) {
    let extract_frame = frame.0.saturating_add(1);
    if script.events.iter().any(|e| e.frame.abs_diff(extract_frame) <= 1) {
        let pose = driving.runtime.pose();
        println!("window_transition_observe frame={extract_frame} segment={} tick={} view={} heading_rad={} consumed_events={}/{}",
            driving.segment, driving.runtime.ticks,
            cameras.as_ref().map_or("none", |c| c.ident()), pose[8].atan2(pose[10]), script.next, script.events.len());
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // Shared pulse helper, real keyboard edges and Actions; NOT a physical Runtime reset.
    #[derive(Resource, Default)]
    struct Fixture { tick: u32, resets: Vec<(u32,u32)>, cameras: Vec<u32>, revisited: bool }
    fn fixture_inject(frame: Res<FrameCount>, mut script: ResMut<Script>, mut keys: ResMut<ButtonInput<KeyCode>>) {
        pulse(frame.0 + 1, &mut script, &mut keys).unwrap();
    }
    fn consume(frame: Res<FrameCount>, keys: Res<ButtonInput<KeyCode>>, actions: Res<super::super::input::Actions>, mut state: ResMut<Fixture>) {
        state.tick += 1;
        if frame.0 + 1 > 180 && state.tick == 180 { state.revisited = true; }
        if keys.just_pressed(KeyCode::KeyR) {
            let before = state.tick;
            state.resets.push((frame.0 + 1, before));
            state.tick = 0;
        }
        if actions.pressed(super::super::input::Action::ChangeView) { state.cameras.push(frame.0 + 1); }
    }
    #[test]
    fn keyboard_edges_do_not_repeat_reset_when_synthetic_ticks_revisit() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::input::InputPlugin))
            .insert_resource(Script::parse("90:C,180:R,240:C").unwrap())
            .init_resource::<Fixture>();
        super::super::input::install(&mut app, super::super::input::Actions::new(
            super::super::input::ActionMap::keyboard(), super::super::input::Pads::Off));
        app.add_systems(PreUpdate, fixture_inject.after(InputSystems).before(super::super::input::sample))
            .add_systems(Update, consume);
        for _ in 0..400 { app.update(); }
        let state = app.world().resource::<Fixture>();
        assert_eq!(state.resets, [(180,180)]);
        assert_eq!(state.cameras, [90,240]);
        assert!(state.revisited, "fixture must actually revisit the reset tick");
        assert_eq!(state.tick, 220);
        assert_eq!(app.world().resource::<Script>().next, 3);
        assert!(!app.world().resource::<ButtonInput<KeyCode>>().pressed(KeyCode::KeyR));
    }

    #[test]
    fn bounded_parser_rejects_invalid_or_missed_frames() {
        let mut script = Script::parse("90:C,180:R,240:C").unwrap();
        assert!(script.fits(1500, 30));
        assert!(!script.fits(100, 30));
        for (frame, expected) in [(89,None),(90,Some(Key::ChangeView)),(91,None),
            (179,None),(180,Some(Key::Reset)),(181,None),(239,None),(240,Some(Key::ChangeView)),(241,None)] {
            assert_eq!(script.poll(frame).unwrap(), expected);
        }
        assert_eq!(script.next, 3);
        assert!(Script::parse("90:C").unwrap().poll(91).is_err());
        for text in ["", "0:C", "90:X", "90:C,91:C", "90:R,180:R", "180:C,90:R"] {
            assert!(Script::parse(text).is_err());
        }
    }
}
