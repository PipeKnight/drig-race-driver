//! Produce private solved-source wrapper requests and production-event answers.
use grid_tools::source_vehicle::{Controls, DELTA, Runtime};
use std::{fs, io::Write, path::Path};
fn words(out: &mut Vec<u8>, input: impl IntoIterator<Item = u32>) {
    for word in input {
        out.extend(word.to_le_bytes());
    }
}
fn flush(
    directory: &Path,
    index: usize,
    count: u32,
    input: &[u8],
    expected: &[u8],
) -> std::io::Result<()> {
    for (suffix, bytes) in [("input", input), ("expected", expected)] {
        let mut f = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(directory.join(format!("{index:03}.{suffix}")))?;
        f.write_all(b"BW01")?;
        f.write_all(&count.to_le_bytes())?;
        f.write_all(bytes)?;
    }
    Ok(())
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<_> = std::env::args_os().skip(1).collect();
    if a.len() != 6 {
        return Err("usage: grid-body-pool-trace GAME_ROOT CD01 BP01 STRAIGHT_TICKS STEERING local/NEW_DIRECTORY".into());
    }
    let straight: usize = a[3].to_str().ok_or("invalid straight extent")?.parse()?;
    let steering: f32 = a[4].to_str().ok_or("invalid steering")?.parse()?;
    if straight > 600
        || !(-1. ..=1.).contains(&steering)
        || fs::metadata(&a[1])?.len() != 712
        || fs::metadata(&a[2])?.len() != 28
    {
        return Err("invalid bounded source trace configuration".into());
    }
    let out = Path::new(&a[5]);
    let parent = out.parent().ok_or("missing parent")?.canonicalize()?;
    if !parent.starts_with(Path::new("local").canonicalize()?) {
        return Err("private output required".into());
    }
    let root = Path::new(&a[0]);
    let directions = fs::read(&a[1])?;
    let parameters = grid_tools::body_impact_parameters::read(&fs::read(&a[2])?)?;
    let mut quiet = Runtime::lacetti(root, "slot_00", 200679)?;
    let quiet_geometry = quiet.prepare_body_geometry(root, &directions)?;
    let mut active = Runtime::lacetti(root, "slot_00", 200679)?;
    let active_geometry = active.prepare_body_geometry(root, &directions)?;
    active.body_events = Some(parameters);
    fs::create_dir(out)?;
    let mut input = Vec::new();
    let mut expected = Vec::new();
    let mut count = 0;
    let mut batch = 0;
    let mut contacts = 0;
    let mut external_total = 0;
    let mut changes = 0;
    for tick in 0..600 {
        let controls = Controls {
            throttle: 1.,
            steering: if tick < straight { 0. } else { steering },
            ..Default::default()
        };
        quiet.tick_with_body_geometry(controls, &quiet_geometry)?;
        active.tick_with_body_geometry(controls, &active_geometry)?;
        if quiet.state.world.groups.contacts != active.state.world.groups.contacts
            || quiet.state.world.groups.bodies != active.state.world.groups.bodies
            || quiet.state.car.owner.physical != active.state.car.owner.physical
        {
            return Err("event stage unexpectedly changed solved world/physical storage".into());
        }
        let used: usize = quiet.state.world.visits.iter().map(|v| v.2).sum();
        let external = used
            .checked_sub(quiet.last_body_contacts)
            .ok_or("invalid occupied contact split")?;
        if used > 20
            || external > 4
            || quiet.state.world.groups.contacts.len() != 20
            || active.state.world.pool_used != 0
        {
            return Err("unsupported source pool framing".into());
        }
        let before = &quiet.state.car.owner;
        let after = &active.state.car.owner;
        let seed = before.damage_random_state.ok_or("missing damage RNG")?;
        words(
            &mut input,
            [seed, DELTA.to_bits(), used as u32, external as u32],
        );
        words(&mut input, before.vehicle.iter().copied());
        words(&mut input, before.physical);
        for c in &quiet.state.world.groups.contacts {
            words(&mut input, *c);
        }
        words(
            &mut expected,
            [
                after.damage_random_state.ok_or("missing result RNG")?,
                0,
                (used - external) as u32,
            ],
        );
        words(&mut expected, after.vehicle.iter().copied());
        words(&mut expected, after.physical);
        for c in &active.state.world.groups.contacts {
            words(&mut expected, *c);
        }
        if active.state.car.frame.body != after.vehicle[..active.state.car.frame.body.len()] {
            return Err("event frame prefix publication mismatch".into());
        }
        changes += usize::from(before.damage_random_state != after.damage_random_state);
        contacts += used;
        external_total += external;
        // Carry the production result into the next callback-free physical
        // stage. Both private query caches already saw the same current inputs.
        quiet.state = active.state.clone();
        count += 1;
        if count == 32 || tick == 599 {
            flush(out, batch, count, &input, &expected)?;
            batch += 1;
            count = 0;
            input.clear();
            expected.clear();
        }
    }
    println!(
        "body_pool_trace ticks=600 batches={batch} contacts={contacts} external={external_total} event_rng_changed_frames={changes}"
    );
    Ok(())
}
fn main() {
    // Two complete source runtimes plus debug-only temporaries exceed the
    // default Windows stack. Keep this diagnostic's storage bound explicit.
    let worker = std::thread::Builder::new()
        .name("body-pool-trace".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(|| run().map_err(|error| error.to_string()))
        .expect("cannot start bounded source trace worker");
    if let Err(error) = worker.join().expect("source trace worker panicked") {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
