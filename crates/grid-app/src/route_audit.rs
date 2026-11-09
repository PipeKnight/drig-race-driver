//! Route graphics audit capture (docs/GRAPHICS_ROUTE_AUDIT.md).
//! `GRID_ROUTE_AUDIT=DIR` with `--drive --drive-ticks N --render OUT.png
//! --render-fps FPS` (the autopilot drives, `GRID_AUTOPILOT=1`) saves the
//! offscreen picture at fixed fractions of the player's first lap, measured
//! as the distance the player's car has driven since the lap started over
//! the route's progress length, from the chase view the run starts in and from the
//! head-cam: `DIR/NN-chase.png` and `DIR/NN-cockpit.png`, each reduced
//! by [`SCALE`]. Each shot prints `route_audit_shot`; the run ends after the
//! last shot is saved, with `route_audit_summary`. The HUD stays hidden so the
//! metrics of tools/route_graphics_audit.py see the 3D picture only. Off
//! unless the variable is set; never part of play.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
};
use std::path::PathBuf;

/// Lap fractions of the shots (AUTHORED: five spread over the lap, the
/// first clear of the start line's grid).
pub const FRACTIONS: [f32; 5] = [0.08, 0.27, 0.46, 0.65, 0.84];
/// Frames the picture settles after a view cut before its shot (temporal
/// anti-aliasing and the shadow caches restart on the cut; AUTHORED).
const SETTLE_FRAMES: u32 = 12;
/// Reduction factor of the saved shots.
pub const SCALE: u32 = 2;
/// The cockpit view's ident in every car's cameras.xml.
const COCKPIT: &str = "head-cam";

#[derive(Clone, Copy, Debug, PartialEq)]
enum Phase {
    /// Waiting for the next fraction.
    Driving,
    /// The chase view settles `n` more frames before its shot.
    Chase(u32),
    /// The head-cam settles `n` more frames before its shot.
    Cockpit(u32),
}

#[derive(Resource)]
pub struct Audit {
    directory: PathBuf,
    next: usize,
    phase: Phase,
    chase: Option<usize>,
    requested: u32,
    saved: u32,
    finished: bool,
    heartbeat: u64,
    /// Route length in metres.
    length: Option<f32>,
    /// Metres the player's car has moved since the lap started.
    driven: f32,
    last: Option<Vec3>,
}

/// Ticks between `route_audit_wait` lines.
const HEARTBEAT: u64 = 1000;

impl Audit {
    /// `length`: the route's progress_track.xml length in metres (None or
    /// zero: no progress line, the time stand-in).
    pub fn from_env(length: Option<f32>) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let Some(directory) = std::env::var_os("GRID_ROUTE_AUDIT").map(PathBuf::from) else {
            return Ok(None);
        };
        std::fs::create_dir_all(&directory)?;
        let length = length.filter(|l| *l > 0.);
        println!(
            "route_audit enabled=true fractions={FRACTIONS:?} scale={SCALE} length={length:?} directory={}",
            directory.display()
        );
        Ok(Some(Self {
            directory,
            next: 0,
            phase: Phase::Driving,
            chase: None,
            requested: 0,
            saved: 0,
            finished: false,
            heartbeat: 0,
            length,
            driven: 0.,
            last: None,
        }))
    }
}

/// Seconds of driving that stand for a lap on a route without a progress
/// line (AUTHORED).
const STAND_IN_LAP_SECONDS: f32 = 60.;

/// A car that has driven less than [`STALL_METRES`] after [`STALL_TICKS`]
/// (24 s, the countdown included) has no driver: the autopilot lives with
/// the opponents, so a freestyle drift event without rivals keeps the car on
/// its mark. The audit then shoots on the clock (AUTHORED).
const STALL_TICKS: u64 = 1500;
const STALL_METRES: f32 = 1.;

fn stalled(ticks: u64, driven: f32) -> bool {
    ticks >= STALL_TICKS && driven < STALL_METRES
}

impl Audit {
    /// The player's lap fraction: metres driven since the lap started (the
    /// first gate; from the start on a route without lap timing) over the
    /// route length. The driven line is near the progress line, so this
    /// tracks the lap within a few percent on every route type, reversed
    /// and point-to-point included. Without a length: the driven time over
    /// [`STAND_IN_LAP_SECONDS`].
    fn fraction(&mut self, driving: &crate::Driving) -> f32 {
        let started = driving.runtime.lap.as_ref().is_none_or(|lap| lap.elapsed().is_some());
        let car = driving.shown_pose(0).map(|pose| pose.w_axis.truncate());
        if started && let (Some(car), Some(last)) = (car, self.last) {
            self.driven += car.distance(last);
        }
        self.last = if started { car } else { None };
        if self.length.is_some() && stalled(driving.runtime.ticks, self.driven) {
            println!("route_audit_stall tick={} driven={:.2}", driving.runtime.ticks, self.driven);
            self.length = None;
        }
        match self.length {
            Some(length) => self.driven / length,
            None => driving.runtime.ticks as f32 * grid_tools::source_vehicle::DELTA / STAND_IN_LAP_SECONDS,
        }
    }
}

/// After the game's camera: cuts between the chase view and the head-cam,
/// saves the shots and ends the run after the last.
#[allow(clippy::too_many_arguments)]
pub fn shoot(
    mut commands: Commands,
    mut audit: ResMut<Audit>,
    driving: Res<crate::Driving>,
    capture: Res<crate::Capture>,
    cameras: Option<ResMut<crate::camera::Cameras>>,
    camera: Query<&Transform, With<crate::ViewerCamera>>,
    mut hud: Query<&mut Visibility, (With<Node>, Without<ChildOf>)>,
    mut exit: MessageWriter<AppExit>,
    frame: Res<bevy::diagnostic::FrameCount>,
) {
    for mut visibility in &mut hud {
        *visibility = Visibility::Hidden;
    }
    if audit.finished {
        if audit.saved == audit.requested {
            println!(
                "route_audit_summary shots={} saved={} tick={} directory={}",
                audit.requested,
                audit.saved,
                driving.runtime.ticks,
                audit.directory.display()
            );
            exit.write(AppExit::Success);
        }
        return;
    }
    let f = audit.fraction(&driving);
    let ticks = driving.runtime.ticks;
    if ticks / HEARTBEAT != audit.heartbeat {
        audit.heartbeat = ticks / HEARTBEAT;
        println!(
            "route_audit_wait tick={ticks} fraction={f:.3} next={} phase={:?} cameras={} target={} viewers={}",
            audit.next,
            audit.phase,
            cameras.is_some(),
            capture.target.is_some(),
            camera.iter().count()
        );
    }
    let (Some(mut cameras), Some(target), Ok(camera)) = (cameras, capture.target.clone(), camera.single()) else {
        return;
    };
    let cockpit = cameras.views.0.iter().position(|v| v.ident == COCKPIT);
    let index = audit.next;
    let mut take = |audit: &mut Audit, view: &str| {
        let path = audit.directory.join(format!("{index:02}-{view}.png"));
        let eye = camera.translation;
        println!(
            "route_audit_shot index={index} view={view} target={:.2} fraction={f:.3} tick={} eye={:.1},{:.1},{:.1} path={}",
            FRACTIONS[index],
            driving.runtime.ticks,
            eye.x,
            eye.y,
            eye.z,
            path.display()
        );
        audit.requested += 1;
        commands.spawn(Screenshot::image(target.clone())).observe(
            move |event: On<ScreenshotCaptured>, mut audit: ResMut<Audit>| match save(&event.image, &path) {
                Ok(()) => audit.saved += 1,
                Err(error) => error!("route audit shot {}: {error}", path.display()),
            },
        );
    };
    match audit.phase {
        Phase::Driving => {
            if f >= FRACTIONS[index] {
                audit.chase = Some(cameras.current);
                take(&mut audit, "chase");
                match cockpit {
                    Some(c) => {
                        cameras.current = c;
                        cut(&frame, "cockpit");
                        audit.phase = Phase::Cockpit(SETTLE_FRAMES);
                    }
                    None => {
                        println!("route_audit_shot index={index} view=cockpit missing=true");
                        advance(&mut audit);
                    }
                }
            }
        }
        Phase::Cockpit(0) => {
            take(&mut audit, "cockpit");
            if let Some(chase) = audit.chase {
                cameras.current = chase;
                cut(&frame, "chase");
            }
            audit.phase = Phase::Chase(SETTLE_FRAMES);
        }
        Phase::Cockpit(n) => audit.phase = Phase::Cockpit(n - 1),
        // Back in the chase view before the next fraction can trigger.
        Phase::Chase(0) => advance(&mut audit),
        Phase::Chase(n) => audit.phase = Phase::Chase(n - 1),
    }
}

/// A view cut: the camera system places the new view in the next frame, so
/// a one-frame `shadow_dropout` (shadow_cull.rs) at about this frame compares
/// two views and is no blink in either (tools/route_graphics_audit.py).
fn cut(frame: &bevy::diagnostic::FrameCount, view: &str) {
    println!("route_audit_cut frame={} view={view}", frame.0);
}

fn advance(audit: &mut Audit) {
    audit.next += 1;
    audit.phase = Phase::Driving;
    audit.finished = audit.next == FRACTIONS.len();
}

fn save(image: &Image, path: &std::path::Path) -> Result<(), String> {
    let dynamic = image.clone().try_into_dynamic().map_err(|e| format!("{e:?}"))?;
    let (w, h) = (dynamic.width() / SCALE, dynamic.height() / SCALE);
    // The image crate's thumbnail averages the source pixels of each target
    // pixel (a box filter).
    let small = dynamic.thumbnail_exact(w, h).to_rgb8();
    let m = Metrics::of(w as usize, h as usize, small.as_raw());
    println!(
        "route_audit_metrics path={} black={:.4} dark={:.4} black_blob={:.4} magenta={:.4} mean={:.1} top={:.1} blob_box={},{},{},{}",
        path.display(),
        m.black,
        m.dark,
        m.black_blob,
        m.magenta,
        m.mean,
        m.top,
        m.blob_box[0],
        m.blob_box[1],
        m.blob_box[2],
        m.blob_box[3],
    );
    small.save(path).map_err(|e| e.to_string())
}

/// What tools/route_graphics_audit.py judges in one shot (shares of all
/// pixels; luminance 0..255).
#[derive(Debug, PartialEq)]
struct Metrics {
    /// No channel above [`BLACK`]: unlit, untextured or NaN-poisoned.
    black: f32,
    /// Luminance below [`DARK`].
    dark: f32,
    /// The largest 4-connected black region.
    black_blob: f32,
    /// The bounding box of that region (x0, y0, x1, y1).
    blob_box: [usize; 4],
    /// Magenta, the usual missing-texture stand-in.
    magenta: f32,
    mean: f32,
    /// Mean luminance of the top eighth (the sky in the chase view).
    top: f32,
}

/// The brightest channel of a black pixel (AUTHORED: shadowed asphalt and
/// tyres stay above it in every circuit shot checked).
const BLACK: u8 = 4;
const DARK: f32 = 16.;

impl Metrics {
    fn of(w: usize, h: usize, rgb: &[u8]) -> Self {
        let n = (w * h).max(1) as f32;
        let lum = |i: usize| 0.2126 * f32::from(rgb[i * 3]) + 0.7152 * f32::from(rgb[i * 3 + 1]) + 0.0722 * f32::from(rgb[i * 3 + 2]);
        let black: Vec<bool> = rgb.as_chunks::<3>().0.iter().map(|p| p.iter().all(|&c| c <= BLACK)).collect();
        let (mut dark, mut magenta, mut sum, mut top) = (0usize, 0usize, 0f32, 0f32);
        let top_rows = (h / 8).max(1);
        for i in 0..w * h {
            let l = lum(i);
            sum += l;
            if i / w < top_rows {
                top += l;
            }
            dark += usize::from(l < DARK);
            let p = &rgb[i * 3..i * 3 + 3];
            magenta += usize::from(p[0] > 180 && p[2] > 180 && p[1] < 80);
        }
        // Largest 4-connected black region by flood fill.
        let mut seen = vec![false; w * h];
        let (mut best, mut best_box) = (0usize, [0usize; 4]);
        let mut stack = Vec::new();
        for start in 0..w * h {
            if !black[start] || seen[start] {
                continue;
            }
            seen[start] = true;
            stack.push(start);
            let (mut size, mut bx) = (0usize, [start % w, start / w, start % w, start / w]);
            while let Some(i) = stack.pop() {
                size += 1;
                let (x, y) = (i % w, i / w);
                bx = [bx[0].min(x), bx[1].min(y), bx[2].max(x), bx[3].max(y)];
                let mut visit = |j: usize| {
                    if black[j] && !seen[j] {
                        seen[j] = true;
                        stack.push(j);
                    }
                };
                if x > 0 { visit(i - 1); }
                if x + 1 < w { visit(i + 1); }
                if y > 0 { visit(i - w); }
                if y + 1 < h { visit(i + w); }
            }
            if size > best {
                (best, best_box) = (size, bx);
            }
        }
        Self {
            black: black.iter().filter(|&&b| b).count() as f32 / n,
            dark: dark as f32 / n,
            black_blob: best as f32 / n,
            blob_box: best_box,
            magenta: magenta as f32 / n,
            mean: sum / n,
            top: top / (top_rows * w).max(1) as f32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractions_rise_inside_the_lap() {
        assert!(FRACTIONS.windows(2).all(|w| w[0] < w[1]));
        assert!(FRACTIONS[0] > 0. && FRACTIONS[FRACTIONS.len() - 1] < 1.);
    }

    #[test]
    fn a_parked_car_falls_back_to_the_clock() {
        assert!(!stalled(STALL_TICKS - 1, 0.));
        assert!(stalled(STALL_TICKS, 0.));
        assert!(!stalled(STALL_TICKS * 4, 50.));
        // The fallback's clock must not have passed every shot already.
        let clock = STALL_TICKS as f32 * grid_tools::source_vehicle::DELTA / STAND_IN_LAP_SECONDS;
        assert!(clock < FRACTIONS[FRACTIONS.len() - 1]);
    }

    #[test]
    fn metrics_find_the_largest_black_region() {
        // 4 x 2: a black 2-pixel region, a lone black pixel, magenta, grey.
        let (k, g, m) = ([0u8, 0, 0], [128u8, 128, 128], [255u8, 0, 255]);
        let rgb: Vec<u8> = [k, k, g, k, g, m, g, g].concat();
        let metrics = Metrics::of(4, 2, &rgb);
        assert_eq!(metrics.black, 3. / 8.);
        assert_eq!(metrics.black_blob, 2. / 8.);
        assert_eq!(metrics.blob_box, [0, 0, 1, 0]);
        assert_eq!(metrics.magenta, 1. / 8.);
        assert!((metrics.top - (0. + 0. + 128. + 0.) / 4.).abs() < 0.5);
    }
}
