//! Diagnostic cinematic capture (row 117 demo, F736). `GRID_CINE=SCRIPT` with
//! `--drive --drive-ticks N --render OUT.png --render-fps FPS` replays a
//! playtest log's player inputs tick by tick (the simulation is deterministic,
//! as Flashback relies on), draws the cars between ticks, and saves every
//! frame of a window with a keyframed camera and a speed ramp. Off unless the
//! variable is set; never part of play.
//!
//! Script lines (`#` comments):
//! - `inputs PATH`: a grid-app log; its segment 0 `drive_input` records.
//! - `frames DIR`: new directory for `00000.png`...
//! - `window FROM TO`: the ticks whose frames are saved.
//! - `key TICK MODE X Y Z LX LY LZ SPEED`: camera keyframe. MODE `abs` puts
//!   the eye at world X Y Z, `rel` at the player car's position plus X Y Z
//!   (world axes); the camera looks at the car plus LX LY LZ. SPEED is
//!   simulation time per output time (0.25 = quarter speed). Between keys the
//!   eye, look offset and speed blend with a smoothstep.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
    time::TimeUpdateStrategy,
};
use grid_tools::source_vehicle::Controls;
use std::{path::PathBuf, time::Duration};

#[derive(Clone, Copy)]
struct Key {
    tick: f32,
    relative: bool,
    eye: Vec3,
    look: Vec3,
    speed: f32,
}

#[derive(Resource)]
pub struct Cine {
    /// Player controls per tick, index 0 = tick 1.
    controls: Vec<Controls>,
    keys: Vec<Key>,
    window: (u64, u64),
    directory: PathBuf,
    fps: f32,
    saved: u32,
    requested: u32,
    reported: bool,
}

impl Cine {
    pub fn from_env(fps: Option<u32>) -> Result<Option<Self>, Box<dyn std::error::Error>> {
        let Some(script) = std::env::var_os("GRID_CINE") else {
            return Ok(None);
        };
        let fps = fps.ok_or("GRID_CINE needs --render-fps")? as f32;
        let (mut controls, mut keys, mut window, mut directory) = (Vec::new(), Vec::new(), None, None);
        for line in std::fs::read_to_string(&script)?.lines() {
            let w: Vec<&str> = line.split_whitespace().collect();
            match w.first().copied() {
                Some("inputs") => controls = read_inputs(w.get(1).ok_or("inputs PATH")?)?,
                Some("frames") => directory = Some(PathBuf::from(w.get(1).ok_or("frames DIR")?)),
                Some("window") if w.len() == 3 => window = Some((w[1].parse()?, w[2].parse()?)),
                Some("key") if w.len() == 10 => {
                    let n: Vec<f32> = w[3..].iter().map(|v| v.parse()).collect::<Result<_, _>>()?;
                    keys.push(Key {
                        tick: w[1].parse()?,
                        relative: w[2] == "rel",
                        eye: Vec3::new(n[0], n[1], n[2]),
                        look: Vec3::new(n[3], n[4], n[5]),
                        speed: n[6],
                    });
                }
                None => {}
                Some(word) if word.starts_with('#') => {}
                _ => return Err(format!("GRID_CINE: bad line {line:?}").into()),
            }
        }
        let directory = directory.ok_or("GRID_CINE: no frames line")?;
        if controls.is_empty() || keys.is_empty() {
            return Err("GRID_CINE: needs inputs and at least one key".into());
        }
        keys.sort_by(|a, b| a.tick.total_cmp(&b.tick));
        std::fs::create_dir(&directory)?; // Never overwrite a previous sequence.
        println!(
            "cine enabled=true input_ticks={} keys={} directory={}",
            controls.len(),
            keys.len(),
            directory.display()
        );
        Ok(Some(Self {
            controls,
            keys,
            window: window.ok_or("GRID_CINE: no window line")?,
            directory,
            fps,
            saved: 0,
            requested: 0,
            reported: false,
        }))
    }

    /// The logged player controls of `tick` (the last record at or before it).
    pub fn controls(&self, tick: u64) -> Option<Controls> {
        let index = usize::try_from(tick.checked_sub(1)?).ok()?;
        Some(self.controls.get(index).copied().unwrap_or_default())
    }

    /// Camera key blended at fractional tick `t`.
    fn at(&self, t: f32, car: Vec3) -> (Vec3, Vec3, f32) {
        let eye = |k: &Key| if k.relative { car + k.eye } else { k.eye };
        let first = &self.keys[0];
        let Some(i) = self.keys.iter().position(|k| k.tick > t) else {
            let k = self.keys.last().unwrap_or(first);
            return (eye(k), car + k.look, k.speed);
        };
        if i == 0 {
            return (eye(first), car + first.look, first.speed);
        }
        let (a, b) = (&self.keys[i - 1], &self.keys[i]);
        let x = ((t - a.tick) / (b.tick - a.tick)).clamp(0., 1.);
        let s = x * x * (3. - 2. * x);
        (eye(a).lerp(eye(b), s), car + a.look.lerp(b.look, s), a.speed + (b.speed - a.speed) * s)
    }
}

fn read_inputs(path: &str) -> Result<Vec<Controls>, Box<dyn std::error::Error>> {
    let mut records: Vec<(u64, Controls)> = Vec::new();
    for line in std::fs::read_to_string(path)?.lines() {
        let Some(rest) = line.strip_prefix("drive_input version=1 segment=0 ") else {
            continue;
        };
        let field = |name: &str| {
            rest.split_whitespace()
                .find_map(|w| w.strip_prefix(name)?.strip_prefix('='))
                .ok_or(format!("drive_input without {name}"))
        };
        records.push((
            field("tick")?.parse()?,
            Controls {
                throttle: field("throttle")?.parse()?,
                brake: field("brake")?.parse()?,
                steering: field("steering")?.parse()?,
                handbrake: field("handbrake")?.parse()?,
            },
        ));
    }
    let last = records.last().map_or(0, |r| r.0);
    let mut out = Vec::new();
    let mut current = Controls::default();
    let mut next = records.iter().peekable();
    for tick in 1..=last {
        while let Some(&&(t, c)) = next.peek() {
            if t > tick {
                break;
            }
            current = c;
            next.next();
        }
        out.push(current);
    }
    Ok(out)
}

/// Before the frame: the virtual frame step follows the speed ramp.
pub fn pace(
    cine: Res<Cine>,
    driving: Res<crate::Driving>,
    mut strategy: ResMut<TimeUpdateStrategy>,
) {
    let t = driving.runtime.ticks as f32;
    let car = driving.shown_pose(0).map_or(Vec3::ZERO, |m| m.w_axis.truncate());
    let (_, _, speed) = cine.at(t, car);
    *strategy = TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(speed.max(0.01) / cine.fps));
}

/// After the game's camera: the keyed camera, the HUD hidden, and the frame
/// saved inside the window.
pub fn shoot(
    mut commands: Commands,
    mut cine: ResMut<Cine>,
    driving: Res<crate::Driving>,
    fixed: Res<Time<Fixed>>,
    capture: Res<crate::Capture>,
    mut camera: Single<&mut Transform, With<crate::ViewerCamera>>,
    mut hud: Query<&mut Visibility, (With<Node>, Without<ChildOf>)>,
) {
    // A clean picture: the HUD's root nodes stay hidden.
    for mut visibility in &mut hud {
        *visibility = Visibility::Hidden;
    }
    let Some(pose) = driving.shown_pose(0) else {
        return;
    };
    let car = pose.w_axis.truncate();
    // The drawn pose is one tick behind the simulation (F729).
    let t = driving.runtime.ticks as f32 - 1. + fixed.overstep_fraction();
    let (eye, look, _) = cine.at(t, car);
    **camera = Transform::from_translation(eye).looking_at(look, Vec3::Y);
    let (from, to) = cine.window;
    let tick = driving.runtime.ticks;
    let Some(target) = &capture.target else {
        return;
    };
    if tick > to && !cine.reported {
        cine.reported = true;
        println!("cine_summary requested={} saved_so_far={} directory={}", cine.requested, cine.saved, cine.directory.display());
    }
    if tick < from || tick > to {
        return;
    }
    let path = cine.directory.join(format!("{:05}.png", cine.requested));
    cine.requested += 1;
    commands
        .spawn(Screenshot::image(target.clone()))
        .observe(move |event: On<ScreenshotCaptured>, mut cine: ResMut<Cine>| {
            let saved = event
                .image
                .clone()
                .try_into_dynamic()
                .map_err(|e| format!("{e:?}"))
                .and_then(|image| image.to_rgb8().save(&path).map_err(|e| e.to_string()));
            match saved {
                Ok(()) => cine.saved += 1,
                Err(error) => error!("cine frame {}: {error}", path.display()),
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inputs_hold_the_last_record_and_keys_blend() {
        let dir = std::env::temp_dir().join(format!("cine-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let log = dir.join("a.log");
        std::fs::write(
            &log,
            "drive_input version=1 segment=0 tick=1 throttle=0 brake=0 steering=0 handbrake=true shift=- hold=1\n\
             drive_input version=1 segment=0 tick=4 throttle=1 brake=0 steering=-1 handbrake=false shift=- hold=0\n",
        )
        .unwrap();
        let controls = read_inputs(log.to_str().unwrap()).unwrap();
        assert_eq!(controls.len(), 4);
        assert!(controls[2].handbrake && controls[3].throttle == 1. && controls[3].steering == -1.);
        let key = |tick, speed| Key { tick, relative: true, eye: Vec3::X * tick, look: Vec3::ZERO, speed };
        let cine = Cine {
            controls,
            keys: vec![key(0., 1.), key(10., 0.25)],
            window: (0, 1),
            directory: dir.clone(),
            fps: 30.,
            saved: 0,
            requested: 0,
            reported: false,
        };
        assert_eq!(cine.controls(5).unwrap().throttle, 0.); // past the log: released
        let (eye, _, speed) = cine.at(5., Vec3::Y);
        assert!((eye - Vec3::new(5., 1., 0.)).length() < 1e-5 && (speed - 0.625).abs() < 1e-6);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
