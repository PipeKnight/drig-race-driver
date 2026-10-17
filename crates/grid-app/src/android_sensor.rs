//! Android tilt sensor (P11, docs/ANDROID.md "Controls").
//!
//! A thread with its own looper reads `TYPE_GAME_ROTATION_VECTOR` (gyroscope
//! and accelerometer, no magnetometer: Google's choice for games) at 100 Hz
//! through the NDK's ASensorManager, or the accelerometer when the phone has
//! no rotation vector. It stores the latest world-up vector for
//! `touch_controls`. The sensor is switched off while the app is suspended.
use crate::tilt;
use crate::touch_controls::TiltReading;
use bevy::prelude::*;
use bevy::window::AppLifecycle;
use ndk_sys as ffi;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the app is in the foreground (the sensor should run).
#[derive(Resource, Clone)]
struct Wanted(Arc<AtomicBool>);

/// 100 Hz, within the 200 Hz allowed without the HIGH_SAMPLING_RATE_SENSORS
/// permission. AUTHORED.
const PERIOD_US: i32 = 10_000;

#[derive(Clone, Copy, Debug)]
enum Kind {
    Rotation,
    Acceleration,
}

fn run(reading: TiltReading, wanted: Arc<AtomicBool>) -> Result<(), &'static str> {
    // SAFETY: plain NDK calls on this thread; every pointer is checked for
    // null before use, and the event buffer outlives each getEvents call.
    unsafe {
        let looper = ffi::ALooper_prepare(ffi::ALOOPER_PREPARE_ALLOW_NON_CALLBACKS as i32);
        if looper.is_null() {
            return Err("no looper");
        }
        let manager = ffi::ASensorManager_getInstanceForPackage(c"org.reassembly.grid".as_ptr());
        if manager.is_null() {
            return Err("no sensor manager");
        }
        let mut kind = Kind::Rotation;
        let mut sensor =
            ffi::ASensorManager_getDefaultSensor(manager, ffi::ASENSOR_TYPE_GAME_ROTATION_VECTOR);
        if sensor.is_null() {
            kind = Kind::Acceleration;
            sensor = ffi::ASensorManager_getDefaultSensor(manager, ffi::ASENSOR_TYPE_ACCELEROMETER);
        }
        if sensor.is_null() {
            return Err("no rotation vector or accelerometer");
        }
        let queue =
            ffi::ASensorManager_createEventQueue(manager, looper, 1, None, std::ptr::null_mut());
        if queue.is_null() {
            return Err("no event queue");
        }
        println!("drive_tilt sensor={kind:?} period_us={PERIOD_US}");
        let mut enabled = false;
        let mut events = [std::mem::zeroed::<ffi::ASensorEvent>(); 16];
        loop {
            let want = wanted.load(Ordering::Relaxed);
            if want && !enabled {
                enabled = ffi::ASensorEventQueue_registerSensor(queue, sensor, PERIOD_US, 0) == 0;
            } else if !want && enabled {
                ffi::ASensorEventQueue_disableSensor(queue, sensor);
                enabled = false;
                if let Ok(mut latest) = reading.0.lock() {
                    *latest = None;
                }
            }
            ffi::ALooper_pollOnce(
                100,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );
            let n = ffi::ASensorEventQueue_getEvents(queue, events.as_mut_ptr(), events.len());
            let Ok(n) = usize::try_from(n) else {
                continue;
            };
            let up = events[..n.min(events.len())].iter().rev().find_map(|e| {
                let d = e.__bindgen_anon_1.__bindgen_anon_1.data;
                match kind {
                    Kind::Rotation => {
                        // values[3] is cos(theta/2); derive it if missing.
                        let w = if d[3].is_finite() && d[3] != 0. {
                            d[3]
                        } else {
                            (1. - d[0] * d[0] - d[1] * d[1] - d[2] * d[2])
                                .max(0.)
                                .sqrt()
                        };
                        Some(tilt::up_from_rotation_vector(d[0], d[1], d[2], w))
                    }
                    Kind::Acceleration => tilt::up_from_acceleration([d[0], d[1], d[2]]),
                }
            });
            if let (Some(up), Ok(mut latest)) = (up, reading.0.lock()) {
                *latest = Some(up);
            }
        }
    }
}

fn lifecycle(mut events: MessageReader<AppLifecycle>, wanted: Res<Wanted>) {
    for event in events.read() {
        let run = matches!(event, AppLifecycle::Running | AppLifecycle::WillResume);
        wanted.0.store(run, Ordering::Relaxed);
    }
}

pub fn install(app: &mut App) {
    let reading = TiltReading::default();
    let wanted = Arc::new(AtomicBool::new(true));
    let (r, w) = (reading.clone(), wanted.clone());
    let spawned = std::thread::Builder::new()
        .name("grid-tilt".into())
        .spawn(move || {
            if let Err(error) = run(r, w) {
                crate::android::log(true, &format!("drive_tilt unavailable: {error}"));
            }
        });
    if spawned.is_err() {
        crate::android::log(true, "drive_tilt unavailable: thread failed");
    }
    app.insert_resource(reading)
        .insert_resource(Wanted(wanted))
        .add_systems(PreUpdate, lifecycle);
}
