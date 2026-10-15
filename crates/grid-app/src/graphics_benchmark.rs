//! Bounded main-schedule wall intervals, not GPU timestamps or monitor presentation FPS.
use bevy::prelude::*;
use std::time::Instant;

const WARMUP_FRAMES: u32 = 240;

#[derive(Resource)]
pub struct Benchmark {
    requested: u32,
    frames: u32,
    previous: Option<Instant>,
    samples: Vec<f64>,
    pub finished: bool,
}

pub fn install(app: &mut App, requested: u32) {
    app.insert_resource(Benchmark {
        requested,
        frames: 0,
        previous: None,
        samples: Vec::with_capacity(requested as usize),
        finished: false,
    })
    .add_systems(Last, measure);
}

fn measure(mut benchmark: ResMut<Benchmark>) {
    if benchmark.finished {
        return;
    }
    let now = Instant::now();
    let previous = benchmark.previous.replace(now);
    benchmark.frames += 1;
    if benchmark.frames <= WARMUP_FRAMES {
        return;
    }
    if let Some(previous) = previous {
        benchmark
            .samples
            .push(now.duration_since(previous).as_secs_f64() * 1000.0);
    }
    if benchmark.samples.len() < benchmark.requested as usize {
        return;
    }
    let mut sorted = benchmark.samples.clone();
    sorted.sort_by(f64::total_cmp);
    let percentile = |q: f64| sorted[((sorted.len() as f64 * q).ceil() as usize).saturating_sub(1)];
    let mean = sorted.iter().sum::<f64>() / sorted.len() as f64;
    println!(
        "graphics_benchmark version=1 scope=main_schedule_wall_interval warmup_frames={WARMUP_FRAMES} samples={} mean_ms={mean:.6} p50_ms={:.6} p95_ms={:.6} p99_ms={:.6} max_ms={:.6} over_16_667_ms={} over_33_333_ms={} interval_rate_hz={:.6}",
        sorted.len(),
        percentile(0.5),
        percentile(0.95),
        percentile(0.99),
        sorted[sorted.len() - 1],
        sorted.iter().filter(|&&ms| ms > 1000.0 / 60.0).count(),
        sorted.iter().filter(|&&ms| ms > 1000.0 / 30.0).count(),
        1000.0 / mean
    );
    println!("graphics_benchmark_samples_ms {:?}", benchmark.samples);
    benchmark.finished = true;
}
