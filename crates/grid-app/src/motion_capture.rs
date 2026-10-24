//! Bounded offscreen motion evidence. Virtual time, never measured display FPS.
use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy)]
pub struct Config {
    start: u64,
    frames: u32,
}
impl Config {
    pub fn parse(text: &str) -> Result<Self, &'static str> {
        let (start, frames) = text
            .split_once(',')
            .ok_or("--motion-capture needs START_TICK,FRAMES")?;
        let start = start.parse().map_err(|_| "invalid motion start tick")?;
        let frames = frames.parse().map_err(|_| "invalid motion frame count")?;
        if start == 0 || !(2..=300).contains(&frames) {
            return Err("motion capture requires start>0 and2..300frames");
        }
        Ok(Self { start, frames })
    }
    pub(crate) fn skid_phase_window(self) -> bool { self.start == 350 && self.frames == 120 }
    /// Called only with the selected typed diagnostic profile's exact constants.
    pub(crate) fn exact_phase_window(self, start: u64, frames: u32, fps: u32, stop: u64) -> bool {
        self.start == start && self.frames == frames && self.check(fps, stop).is_ok()
    }
    fn required_ticks(self, fps: u32) -> u64 {
        (f64::from(self.frames) / f64::from(fps) / f64::from(grid_tools::source_vehicle::DELTA)).ceil() as u64
    }
    /// Narrow fence pilot only; general motion capture admission is unchanged.
    pub(crate) fn check_fence_window(self, fps: Option<u32>, ticks: Option<u64>) -> bool {
        let (Some(fps), Some(ticks)) = (fps, ticks) else { return false; };
        if !(30..=60).contains(&self.frames) || !(30..=60).contains(&fps) || !(1..=600).contains(&ticks) || self.check(fps,ticks).is_err() { return false; }
        let required = self.required_ticks(fps);
        self.start.checked_add(required + 8).is_some_and(|end| end <= ticks)
    }
    fn check(self, fps: u32, ticks: u64) -> Result<(), &'static str> {
        let required = self.required_ticks(fps);
        if !(30..=300).contains(&fps)
            || self
                .start
                .checked_add(required + 4)
                .is_none_or(|end| end > ticks)
        {
            return Err("motion capture must finish before scripted driving stops");
        }
        Ok(())
    }
}
#[derive(Resource)]
pub struct Motion {
    config: Config,
    directory: PathBuf,
    requested: u32,
    completed: u32,
    failed: bool,
    records: Vec<String>,
}
impl Motion {
    pub fn finished(&self) -> bool {
        self.completed == self.config.frames && !self.failed
    }
}

pub fn install(
    app: &mut App,
    config: Config,
    path: &Path,
    fps: Option<u32>,
    ticks: Option<u64>,
    size: (u32, u32),
) -> Result<(), Box<dyn std::error::Error>> {
    let fps = fps.ok_or("motion capture requires explicit --render-fps")?;
    config.check(fps, ticks.ok_or("motion capture requires --drive-ticks")?)?;
    if size.0 > 1920 || size.1 > 1080 {
        return Err("motion sequences are limited to1920x1080; use final PNG for native4K".into());
    }
    let directory = path.with_file_name(format!(
        "{}-frames",
        path.file_stem()
            .ok_or("motion output stem absent")?
            .to_string_lossy()
    ));
    std::fs::create_dir(&directory)?; // Never overwrite a previous sequence.
    println!(
        "motion_capture enabled=true start_tick={} frames={} virtual_fps={fps} directory={} measured_display_fps=false",
        config.start,
        config.frames,
        directory.display()
    );
    app.insert_resource(Motion {
        config,
        directory,
        requested: 0,
        completed: 0,
        failed: false,
        records: vec![],
    })
    .add_systems(Last, request);
    Ok(())
}
#[allow(clippy::too_many_arguments)] // Existing ECS request plus optional bounded diagnostic.
fn request(
    mut commands: Commands,
    mut motion: ResMut<Motion>,
    capture: Res<super::Capture>,
    driving: Res<super::Driving>,
    clocks: (Res<Time<Fixed>>, Res<Time<Real>>),
    graphics: Res<super::graphics::Settings>,
    phase_audit: Option<Res<super::skid_phase_ledger::Audit>>,
    mut exit: MessageWriter<AppExit>,
) {
    if motion.failed
        || motion.requested == motion.config.frames
        || driving.runtime.ticks < motion.config.start
    {
        return;
    }
    if driving.failure.is_some()
        || driving.paused
        || driving
            .scripted_ticks
            .is_some_and(|n| driving.runtime.ticks >= n)
    {
        motion.failed = true;
        error!("motion capture stopped before its requested frames");
        exit.write(AppExit::error());
        return;
    }
    let Some(target) = &capture.target else {
        return;
    };
    let index = motion.requested;
    let (clock, real) = clocks;
    let wheels = if graphics.smooth_wheel_motion {
        super::wheel_presentation::wheels(&driving.runtime, clock.overstep().as_secs_f32())
    } else {
        driving.runtime.wheels()
    };
    let phases = wheels.map(|w| w.phase);
    let spins: [f32; 4] =
        std::array::from_fn(|i| super::wheel_presentation::angular_speed(&driving.runtime, i));
    motion.records.push(format!("frame={index} tick={} fixed_remainder={} frame_seconds={} phases={phases:?} spins={spins:?}",driving.runtime.ticks,clock.overstep().as_secs_f32(),real.delta_secs()));
    motion.requested += 1;
    let path = motion.directory.join(format!("{index:04}.png"));
    let entity = commands.spawn(Screenshot::image(target.clone())).id();
    let phase_audit = phase_audit.map(|a| (*a).clone());
    if let Some(audit) = &phase_audit {
        let Some(stamp) = audit.stamp(index, entity, driving.runtime.ticks, target) else {
            motion.failed=true; exit.write(AppExit::error()); return;
        };
        commands.entity(entity).insert(stamp);
    }
    commands.entity(entity).observe(move |event: On<ScreenshotCaptured>, mut motion: ResMut<Motion>, mut exit: MessageWriter<AppExit>| {
        if phase_audit.as_ref().is_some_and(|a| !a.readback(index, entity, event.entity)) {
            motion.failed=true; exit.write(AppExit::error()); return;
        }
        let result=event.image.clone().try_into_dynamic().map_err(|e|format!("{e:?}")).and_then(|image|image.to_rgb8().save(&path).map_err(|e|e.to_string()));
        if let Err(error)=result { motion.failed=true; error!("motion capture: {error}"); exit.write(AppExit::error()); return; }
        if let Some(audit)=&phase_audit { audit.png(index,entity); }
        motion.completed+=1;
        if motion.finished() {
            let result=(|| -> std::io::Result<()> { let mut f=std::fs::OpenOptions::new().create_new(true).write(true).open(motion.directory.join("frames.txt"))?; writeln!(f,"# request-frame metadata; asynchronous GPU readback; virtual clock, not monitor FPS")?; for record in &motion.records { writeln!(f,"{record}")?; } Ok(()) })();
            if let Err(error)=result { motion.failed=true; error!("motion manifest: {error}"); exit.write(AppExit::error()); }
            else { println!("motion_capture complete=true frames={} directory={}",motion.completed,motion.directory.display()); }
        }
    });
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_equal_duration_windows_finish_while_physics_runs() {
        for (fps, n) in [(30, 30), (60, 60), (144, 144), (300, 300)] {
            let c = Config::parse(&format!("240,{n}")).unwrap();
            assert!(c.check(fps, 310).is_ok());
            assert!(c.check(fps, 300).is_err());
        }
        for text in [
            "0,30",
            "240,1",
            "240,301",
            "240",
            "240,-2",
            "18446744073709551615,30",
        ] {
            assert!(Config::parse(text).and_then(|c| c.check(30, 400)).is_err());
        }
    }
}

#[cfg(test)]
mod fence_window_tests {
    use super::*;
    #[test]
    fn real_capture_config_requires_requested_frames_fps_bound_and_extra_headroom() {
        let c=Config::parse("80,60").unwrap();
        assert!(c.check_fence_window(Some(30),Some(220)));
        assert!(!c.check_fence_window(None,Some(220)));
        assert!(!c.check_fence_window(Some(30),None));
        assert!(!c.check_fence_window(Some(30),Some(212)));
        assert!(!c.check_fence_window(Some(61),Some(220)));
        assert!(!c.check_fence_window(Some(30),Some(601)));
        for text in ["80,29","80,61","18446744073709551615,60"] { assert!(!Config::parse(text).unwrap().check_fence_window(Some(30),Some(220))); }
    }
}
