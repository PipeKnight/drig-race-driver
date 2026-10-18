//! Phone controls (P11, docs/ANDROID.md "Controls"): on-screen touch zones
//! and tilt steering feed the original action layer like keyboard keys and a
//! pad stick (docs/PLATFORMS_AND_INPUT_RESEARCH.md 6.2). Touch and tilt are
//! not original GRID inputs; the layout, sizes and defaults are AUTHORED.
//!
//! The zone layout and hit test are pure and platform independent. The Bevy
//! glue reads `Touches`, so the HUD also runs on a desktop with
//! `[input] touch = on`; only the tilt sensor is Android-only
//! (`android_sensor.rs`).
use crate::tilt::{self, Tilt};
use bevy::input::{InputSystems, touch::Touches};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Button {
    Throttle,
    Brake,
    Handbrake,
    GearUp,
    GearDown,
    Pause,
    LookBack,
    ChangeView,
    Recentre,
    MenuUp,
    MenuDown,
    MenuLeft,
    MenuRight,
    MenuSelect,
    MenuBack,
}

pub const BUTTONS: usize = 15;

/// Driving zones while the car is driven; menu zones while the pause menu,
/// the post-race screens or a replay own the input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    Drive,
    Menu,
}

/// A screen rectangle in fractions of the window (x right, y down).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x0: f32,
    pub y0: f32,
    pub x1: f32,
    pub y1: f32,
}

impl Rect {
    const fn new(x0: f32, y0: f32, x1: f32, y1: f32) -> Self {
        Self { x0, y0, x1, y1 }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        (self.x0..=self.x1).contains(&x) && (self.y0..=self.y1).contains(&y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Zone {
    pub button: Button,
    pub layer: Layer,
    pub rect: Rect,
    pub label: &'static str,
}

/// Every zone. AUTHORED landscape layout for thumbs: pedals at the bottom
/// corners (a 6.7-inch phone makes them about 25 x 34 mm), handbrake and
/// gears next to them, small buttons (at least about 15 x 9.5 mm) at the top
/// centre (the driving status panel covers the top left) and bottom centre.
/// The race HUD (F353) draws on top of the zones.
/// Menu zones sit at the side edges, clear of the centred pause menu.
pub const ZONES: [Zone; BUTTONS + 1] = {
    use self::Button::*;
    use self::Layer::*;
    const fn z(button: Button, layer: Layer, rect: Rect, label: &'static str) -> Zone {
        Zone {
            button,
            layer,
            rect,
            label,
        }
    }
    [
        z(Brake, Drive, Rect::new(0.00, 0.50, 0.17, 1.00), "BRAKE"),
        z(Handbrake, Drive, Rect::new(0.18, 0.74, 0.30, 1.00), "HAND"),
        z(Throttle, Drive, Rect::new(0.83, 0.50, 1.00, 1.00), "GAS"),
        z(GearUp, Drive, Rect::new(0.70, 0.50, 0.82, 0.73), "GEAR +"),
        z(GearDown, Drive, Rect::new(0.70, 0.75, 0.82, 1.00), "GEAR -"),
        z(Pause, Drive, Rect::new(0.45, 0.00, 0.55, 0.14), "II"),
        z(Recentre, Drive, Rect::new(0.33, 0.00, 0.43, 0.14), "CENTRE"),
        z(ChangeView, Drive, Rect::new(0.39, 0.86, 0.49, 1.00), "VIEW"),
        z(LookBack, Drive, Rect::new(0.51, 0.86, 0.61, 1.00), "LOOK"),
        z(Pause, Menu, Rect::new(0.45, 0.00, 0.55, 0.14), "II"),
        z(MenuUp, Menu, Rect::new(0.00, 0.28, 0.12, 0.50), "UP"),
        z(MenuDown, Menu, Rect::new(0.00, 0.52, 0.12, 0.74), "DOWN"),
        z(MenuLeft, Menu, Rect::new(0.00, 0.78, 0.12, 1.00), "<"),
        z(MenuRight, Menu, Rect::new(0.13, 0.78, 0.25, 1.00), ">"),
        z(MenuSelect, Menu, Rect::new(0.88, 0.52, 1.00, 0.74), "OK"),
        z(MenuBack, Menu, Rect::new(0.88, 0.78, 1.00, 1.00), "BACK"),
    ]
};

/// Which zones are live: gear buttons only with the manual gearbox, the
/// re-centre button only with tilt steering.
pub fn shown(zone: &Zone, layer: Layer, manual: bool, tilt: bool) -> bool {
    zone.layer == layer
        && match zone.button {
            Button::GearUp | Button::GearDown => manual,
            Button::Recentre => tilt,
            _ => true,
        }
}

/// Bevy issue #16798: more than 8 touches can crash, so later touches are
/// ignored. AUTHORED bound.
pub const MAX_TOUCHES: usize = 8;

/// Buttons held by the touches of this frame (positions in window fractions).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Held([bool; BUTTONS]);

impl Held {
    pub fn map(
        touches: impl IntoIterator<Item = (f32, f32)>,
        layer: Layer,
        manual: bool,
        tilt: bool,
    ) -> Self {
        let mut held = Self::default();
        for (x, y) in touches.into_iter().take(MAX_TOUCHES) {
            if let Some(zone) = ZONES
                .iter()
                .find(|z| shown(z, layer, manual, tilt) && z.rect.contains(x, y))
            {
                held.0[zone.button as usize] = true;
            }
        }
        held
    }

    pub fn down(&self, button: Button) -> bool {
        self.0[button as usize]
    }
}

/// A finger that is down when the layer changes (OK resumes the race, II
/// pauses it) is ignored until it lifts, so it does not press the zone of the
/// other layer under it (OK and BACK lie over GAS). AUTHORED.
pub fn carry_block(blocked: &[u64], active: &[u64], layer_changed: bool) -> Vec<u64> {
    if layer_changed {
        active.to_vec()
    } else {
        blocked
            .iter()
            .copied()
            .filter(|id| active.contains(id))
            .collect()
    }
}

/// Latest sensor up vector, written by the Android sensor thread.
#[derive(Resource, Clone, Default)]
pub struct TiltReading(pub Arc<Mutex<Option<[f32; 3]>>>);

/// The phone controls' state of this frame, read by `input::sample`.
#[derive(Resource, Debug)]
pub struct Mobile {
    pub layer: Layer,
    pub manual: bool,
    pub held: Held,
    previous: Held,
    /// Touches that were down when the layer last changed; ignored until lifted.
    blocked: Vec<u64>,
    pub tilt: Option<Tilt>,
    /// `device=auto`: a connected gamepad takes over (P11b).
    pub pad_takeover: bool,
    /// A gamepad has taken over: zones hidden, touches and tilt ignored.
    pub pad: bool,
}

impl Mobile {
    pub fn new(tilt: Option<tilt::Settings>) -> Self {
        Self {
            layer: Layer::Drive,
            manual: false,
            held: Held::default(),
            previous: Held::default(),
            blocked: Vec::new(),
            tilt: tilt.map(Tilt::new),
            pad_takeover: false,
            pad: false,
        }
    }

    /// Bi-axis raw steering for the action layer; `None` until a reading
    /// and while a gamepad has taken over.
    pub fn tilt_raw(&self) -> Option<f32> {
        self.tilt
            .as_ref()
            .filter(|_| !self.pad)
            .filter(|t| t.neutral().is_some())
            .map(Tilt::raw)
    }
}

/// `PreUpdate`, before `input::sample`: the frame's layer, held zones,
/// re-centre edge and tilt value.
#[allow(clippy::too_many_arguments)]
fn update(
    mut mobile: ResMut<Mobile>,
    touches: Option<Res<Touches>>,
    window: Query<&Window, With<PrimaryWindow>>,
    driving: Option<Res<crate::Driving>>,
    post: Option<Res<crate::post_race::PostRace>>,
    replay: Option<Res<crate::replay::Replay>>,
    reading: Option<Res<TiltReading>>,
    time: Res<Time<Real>>,
    pads: Query<(), (With<crate::input::PlayerPad>, With<Gamepad>)>,
) {
    let pad = mobile.pad_takeover && !pads.is_empty();
    let pad_changed = pad != mobile.pad;
    if pad_changed {
        mobile.pad = pad;
        log_line!(
            "drive_input_pad takeover={}",
            if pad { "on" } else { "off" }
        );
    }
    let menu = driving.as_ref().is_none_or(|d| d.paused)
        || post.as_ref().is_some_and(|p| p.active())
        || replay.as_ref().is_some_and(|r| r.owns_input());
    let layer = if menu { Layer::Menu } else { Layer::Drive };
    let ids: Vec<u64> = touches
        .as_ref()
        .map_or_else(Vec::new, |t| t.iter().map(|t| t.id()).collect());
    // Fingers down when a pad comes or goes wait until lifted, like a layer change.
    mobile.blocked = carry_block(
        &mobile.blocked,
        &ids,
        layer != mobile.layer || pad_changed,
    );
    mobile.layer = layer;
    mobile.manual = driving.as_ref().is_some_and(|d| {
        d.runtime.assists.transmission == grid_tools::source_vehicle::assists::Transmission::Manual
    });
    let size = window
        .iter()
        .next()
        .map(|w| Vec2::new(w.width(), w.height()));
    let points = match (touches.as_ref(), size) {
        (Some(touches), Some(size)) if size.x > 0. && size.y > 0. && !mobile.pad => touches
            .iter()
            .filter(|t| !mobile.blocked.contains(&t.id()))
            .map(|t| (t.position().x / size.x, t.position().y / size.y))
            .collect(),
        _ => Vec::new(),
    };
    mobile.previous = mobile.held;
    let tilt = mobile.tilt.is_some();
    mobile.held = Held::map(points, mobile.layer, mobile.manual, tilt);
    let recentre = mobile.held.down(Button::Recentre) && !mobile.previous.down(Button::Recentre);
    let up = reading.and_then(|r| r.0.lock().ok().and_then(|g| *g));
    let dt = time.delta_secs();
    if let Some(t) = mobile.tilt.as_mut() {
        t.update(up, dt);
        if recentre {
            t.recentre();
            log_line!("drive_tilt recentre neutral_deg={:?}", t.neutral());
        }
    }
}

#[derive(Component)]
struct ZoneNode(usize);

#[derive(Component)]
struct TiltMarker;

#[derive(Component)]
struct TiltBar;

const IDLE: Color = Color::srgba(1., 1., 1., 0.10);
const HELD: Color = Color::srgba(1., 1., 1., 0.32);
const EDGE: Color = Color::srgba(1., 1., 1., 0.35);

fn spawn(
    mut commands: Commands,
    camera: Query<Entity, With<crate::ViewerCamera>>,
    mobile: Res<Mobile>,
    mut done: Local<bool>,
) {
    let Some(camera) = camera.iter().next().filter(|_| !*done) else {
        return;
    };
    *done = true;
    for (i, zone) in ZONES.iter().enumerate() {
        let r = zone.rect;
        commands
            .spawn((
                ZoneNode(i),
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(r.x0 * 100.),
                    top: percent(r.y0 * 100.),
                    width: percent((r.x1 - r.x0) * 100.),
                    height: percent((r.y1 - r.y0) * 100.),
                    border: UiRect::all(px(2.)),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(IDLE),
                BorderColor::all(EDGE),
                // Below the race HUD and the pause menu.
                GlobalZIndex(-1),
                Visibility::Hidden,
                UiTargetCamera(camera),
            ))
            .with_children(|p| {
                p.spawn((
                    Text::new(zone.label),
                    TextFont {
                        font_size: FontSize::Px(22.),
                        ..default()
                    },
                    TextColor(Color::srgba(1., 1., 1., 0.75)),
                ));
            });
    }
    if mobile.tilt.is_some() {
        // Tilt indicator: a thin bar above VIEW / LOOK with a marker.
        commands
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(39.),
                    top: percent(82.),
                    width: percent(22.),
                    height: percent(2.),
                    ..default()
                },
                TiltBar,
                BackgroundColor(IDLE),
                GlobalZIndex(-1),
                UiTargetCamera(camera),
            ))
            .with_children(|p| {
                p.spawn((
                    TiltMarker,
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(49.),
                        width: percent(2.),
                        height: percent(100.),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(1., 0.85, 0.2, 0.9)),
                ));
            });
    }
}

fn present(
    mobile: Res<Mobile>,
    mut zones: Query<(&ZoneNode, &mut Visibility, &mut BackgroundColor)>,
    mut marker: Query<&mut Node, With<TiltMarker>>,
    mut bar: Query<&mut Visibility, (With<TiltBar>, Without<ZoneNode>)>,
) {
    let tilt = mobile.tilt.is_some();
    let hidden = |show: bool| {
        if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        }
    };
    for mut visibility in &mut bar {
        visibility.set_if_neq(hidden(!mobile.pad));
    }
    for (node, mut visibility, mut colour) in &mut zones {
        let zone = &ZONES[node.0];
        let show = !mobile.pad && shown(zone, mobile.layer, mobile.manual, tilt);
        visibility.set_if_neq(hidden(show));
        let held = show && mobile.held.down(zone.button);
        colour.set_if_neq(BackgroundColor(if held { HELD } else { IDLE }));
    }
    if let (Some(t), Ok(mut node)) = (mobile.tilt.as_ref(), marker.single_mut()) {
        node.left = percent(49. + 49. * t.value());
    }
}

/// AUTHORED: phones report small logical windows (about 914 x 411 on the
/// owner's phone at its default density, [I]), while the race HUD and the
/// pause menu are laid out in desktop pixels. In touch mode the UI is scaled
/// so the window is at least 720 UI pixels high.
fn ui_scale(
    camera: Query<&Camera, With<crate::ViewerCamera>>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut scale: ResMut<UiScale>,
) {
    // The HUD camera's target: the window, the render_height image shown
    // over the window (render_scale.rs), or the image of a --render capture.
    // The cap keeps the HUD at the size it has on the window itself.
    let cap = |h: f32| {
        window
            .iter()
            .next()
            .filter(|w| w.height() > 0.)
            .map_or(1., |w| h / w.height())
    };
    let height = camera
        .iter()
        .next()
        .and_then(Camera::logical_target_size)
        .map(|s| s.y);
    if let Some(h) = height.filter(|h| *h > 0.) {
        let s = (h / 720.).min(cap(h).max(1.));
        if (scale.0 - s).abs() > 1e-3 {
            scale.0 = s;
        }
    }
}

pub fn install(app: &mut App, tilt: Option<tilt::Settings>, pad_takeover: bool) {
    let mut mobile = Mobile::new(tilt);
    mobile.pad_takeover = pad_takeover;
    app.insert_resource(mobile)
        .add_systems(
            PreUpdate,
            update.after(InputSystems).before(crate::input::sample),
        )
        .add_systems(Update, (ui_scale, spawn, present).chain());
    #[cfg(target_os = "android")]
    if tilt.is_some() {
        crate::android_sensor::install(app);
    }
}

#[cfg(test)]
mod tests {
    use super::Button;
    use super::*;

    #[test]
    fn zones_lie_on_screen_and_do_not_overlap_within_a_layer() {
        for (i, a) in ZONES.iter().enumerate() {
            let r = a.rect;
            assert!(0. <= r.x0 && r.x0 < r.x1 && r.x1 <= 1., "{a:?}");
            assert!(0. <= r.y0 && r.y0 < r.y1 && r.y1 <= 1., "{a:?}");
            for b in &ZONES[i + 1..] {
                if a.layer != b.layer {
                    continue;
                }
                let s = b.rect;
                let apart = r.x1 <= s.x0 || s.x1 <= r.x0 || r.y1 <= s.y0 || s.y1 <= r.y0;
                assert!(apart, "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn zones_are_thumb_sized_on_a_landscape_phone() {
        // 6.73-inch 3200 x 1440 screen (Xiaomi 13 Ultra [S]): about 150 x 68 mm.
        let (w, h) = (150., 68.);
        for z in &ZONES {
            let (zw, zh) = ((z.rect.x1 - z.rect.x0) * w, (z.rect.y1 - z.rect.y0) * h);
            assert!(zw >= 9. && zh >= 9., "{z:?}: {zw} x {zh} mm");
        }
    }

    #[test]
    fn touches_map_to_buttons_by_layer_and_mode() {
        use super::Button::*;
        let both = [(0.95, 0.9), (0.05, 0.6)];
        let h = Held::map(both, Layer::Drive, false, true);
        assert!(h.down(Throttle) && h.down(Brake) && !h.down(Handbrake));
        // Gear zones only with the manual gearbox.
        let gear = [(0.75, 0.6), (0.75, 0.9)];
        assert_eq!(Held::map(gear, Layer::Drive, false, true), Held::default());
        let h = Held::map(gear, Layer::Drive, true, true);
        assert!(h.down(GearUp) && h.down(GearDown));
        // Re-centre only with tilt.
        assert!(Held::map([(0.38, 0.05)], Layer::Drive, false, true).down(Recentre));
        assert!(!Held::map([(0.38, 0.05)], Layer::Drive, false, false).down(Recentre));
        // The menu layer reuses the corners for navigation, and Pause stays.
        let h = Held::map(
            [(0.05, 0.6), (0.95, 0.9), (0.5, 0.02)],
            Layer::Menu,
            true,
            true,
        );
        assert!(h.down(MenuDown) && h.down(MenuBack) && h.down(Pause));
        assert!(!h.down(Brake) && !h.down(Throttle));
        // Empty screen areas and the window edge.
        assert_eq!(
            Held::map([(0.5, 0.4)], Layer::Drive, true, true),
            Held::default()
        );
        assert!(Held::map([(1.0, 1.0)], Layer::Drive, false, true).down(Throttle));
    }

    #[test]
    fn only_the_first_eight_touches_count() {
        let mut touches = vec![(0.5, 0.4); MAX_TOUCHES];
        touches.push((0.95, 0.9));
        assert!(!Held::map(touches, Layer::Drive, false, false).down(Button::Throttle));
    }

    #[test]
    fn fingers_down_at_a_layer_change_wait_until_lifted() {
        // OK held while the race resumes: blocked.
        assert_eq!(carry_block(&[], &[7, 9], true), vec![7, 9]);
        // Still down: stays blocked; lifted: released; new touches count.
        assert_eq!(carry_block(&[7, 9], &[7, 11], false), vec![7]);
        assert_eq!(carry_block(&[7], &[11], false), Vec::<u64>::new());
    }

    #[test]
    fn tilt_raw_waits_for_the_first_reading() {
        let mut m = Mobile::new(Some(tilt::Settings::default()));
        assert_eq!(m.tilt_raw(), None);
        m.tilt.as_mut().unwrap().update(Some([1., 0., 0.]), 0.016);
        assert_eq!(m.tilt_raw(), Some(0.5));
        assert_eq!(Mobile::new(None).tilt_raw(), None);
        // A gamepad that took over silences tilt until it disconnects.
        m.pad = true;
        assert_eq!(m.tilt_raw(), None);
        m.pad = false;
        assert_eq!(m.tilt_raw(), Some(0.5));
    }
}
