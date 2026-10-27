//! Force feedback in the race (docs/FORCE_FEEDBACK.md): after every player physics tick, the
//! snapshot `7E9D60` of the player's car goes to the GameFFB task, which runs on the device
//! thread of `force_feedback::windows` (DirectInput wheel or XInput pad). The original queues one
//! snapshot per car manager step; the port queues one per fixed 16 ms tick.

use crate::force_feedback::{Body, Ffb, Setup, Snapshot};
use crate::input::{Device, Settings};
use bevy::prelude::*;
use std::sync::{Arc, Mutex};

type Shared = crate::force_feedback::windows::Shared;

#[derive(Resource)]
struct Feedback {
    /// Surface ids for the snapshots (the task has its own copy of the setup).
    setup: Setup,
    car: String,
    last: Option<(u64, u64)>,
    shared: Arc<Mutex<Shared>>,
}

/// The device the player's FFB goes to: the DirectInput wheel when one is bound and has force
/// feedback, else an XInput pad's motors (the original's device `XBOX360 Controller`, delay
/// 0.1 s) unless `device=keyboard`. Returns the log text.
pub fn install(app: &mut App, root: &std::path::Path, settings: &Settings, wheel_bound: bool, car: &str) -> String {
    if !settings.force_feedback() {
        return "off".into();
    }
    let setup = match Setup::load(root) {
        Ok(s) => s,
        Err(error) => return format!("unavailable({error})"),
    };
    let handle = crate::force_feedback::windows::device();
    let wheel = handle.wheel.as_ref().filter(|w| wheel_bound && w.force_feedback);
    let (name, is_wheel) = match wheel {
        Some(w) => (w.name.clone(), true),
        None if settings.device == Device::Keyboard => return "off(keyboard)".into(),
        None => ("XBOX360 Controller".to_owned(), false),
    };
    let mut ffb = Ffb::new(setup.clone(), &name, is_wheel);
    ffb.gains = settings.feedback_gains();
    // 828670's +0x118 (steering saturation x ratio) and +0x11C (wheelLinearity response 0.4).
    ffb.centre_ratio = settings.steering.saturation.unwrap_or(1.) * 1.5;
    if let Ok(mut s) = handle.shared.lock() {
        s.ffb = Some(ffb);
        s.rumble = !is_wheel;
        s.fed = None;
    }
    app.insert_resource(Feedback { setup, car: car.into(), last: None, shared: handle.shared.clone() })
        .add_systems(FixedUpdate, tick.after(super::drive_tick))
        .add_systems(Last, stop_on_exit);
    let [force, friction, effects] = settings.feedback_gains();
    format!(
        "device={name:?} kind={} force={force} friction={friction} effects={effects}",
        if is_wheel { "wheel" } else { "rumble" }
    )
}

/// Once per successful vehicle tick: the player's snapshot.
fn tick(mut feedback: ResMut<Feedback>, driving: Res<super::Driving>) {
    let now = (driving.segment, driving.runtime.ticks);
    let paused = driving.paused || driving.failure.is_some();
    if paused {
        if let Ok(mut s) = feedback.shared.lock()
            && let Some(ffb) = s.ffb.as_mut()
        {
            ffb.set_paused(true);
        }
        return;
    }
    if feedback.last == Some(now) || now.1 == 0 {
        return;
    }
    feedback.last = Some(now);
    let runtime = &driving.runtime;
    let impacts = match &driving.opponents {
        Some(o) => o.multi.contact_impacts(0),
        None => runtime.contact_impacts(),
    };
    // A69930: the body's impulse accumulator, summed from the solved contacts as the collision
    // sounds do.
    let impulse = (!impacts.is_empty()).then(|| {
        impacts
            .iter()
            .fold([0.; 3], |a, (x, _)| std::array::from_fn(|j| a[j] + x.impact * x.normal[j]))
    });
    let body = Body { pose: runtime.pose(), velocity: runtime.velocity(), angular: runtime.angular_velocity() };
    let surfaces = runtime.surfaces();
    let setup = &feedback.setup;
    let snapshot = Snapshot::build(&runtime.state.car.owner.vehicle, &body, &feedback.car, impulse, |token| {
        surfaces.mechanics_name(token).and_then(|n| setup.surface(n))
    });
    if let Ok(mut s) = feedback.shared.lock() {
        if let Some(ffb) = s.ffb.as_mut() {
            ffb.set_paused(false);
            ffb.queue(snapshot);
        }
        s.fed = Some(std::time::Instant::now());
    }
}

/// The wheel goes slack and the motors stop when the app exits.
fn stop_on_exit(feedback: Res<Feedback>, mut exits: MessageReader<AppExit>) {
    if exits.read().next().is_some()
        && let Ok(mut s) = feedback.shared.lock()
    {
        s.ffb = None;
        s.fed = None;
    }
}
