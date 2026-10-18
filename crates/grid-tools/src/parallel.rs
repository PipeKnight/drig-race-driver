//! Independent per-car work on several cores (docs/PERFORMANCE.md).
//!
//! Work runs on Bevy's [`ComputeTaskPool`], the pool grid-app's systems
//! already use, so the app and the headless tools share one set of worker
//! threads. Headless tools create the pool on first use: `GRID_THREADS=N`
//! sets its size, otherwise every available core.
//!
//! [`map`] returns its results in index order whatever thread computed them.
//! Callers keep every order-dependent step (pool offsets, capacities, error
//! handling) in a sequential pass over those results, so a run gives
//! bit-identical results with one thread or many.
use bevy_tasks::{ComputeTaskPool, TaskPool, TaskPoolBuilder};
use std::sync::OnceLock;

/// `GRID_THREADS=N`, if set to a positive number.
fn requested() -> Option<usize> {
    static REQUESTED: OnceLock<Option<usize>> = OnceLock::new();
    *REQUESTED.get_or_init(|| {
        std::env::var("GRID_THREADS")
            .ok()
            .and_then(|v| v.parse::<usize>().ok())
            .filter(|&n| n >= 1)
    })
}

/// The shared compute pool, created with `GRID_THREADS` (default: all cores)
/// threads unless grid-app's task pool plugin already created it.
pub fn pool() -> &'static TaskPool {
    ComputeTaskPool::get_or_init(|| {
        let threads = requested().unwrap_or_else(|| {
            std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get)
        });
        TaskPoolBuilder::new()
            .num_threads(threads)
            .thread_name("grid compute".into())
            // The race tools run the simulation on a 64 MiB thread.
            .stack_size(64 * 1024 * 1024)
            .build()
    })
}

/// Threads used for per-car work: the shared pool's, at most `GRID_THREADS`
/// (so `GRID_THREADS=1` runs everything serially in grid-app too, whose pool
/// Bevy creates).
pub fn threads() -> usize {
    let pool = pool().thread_num().max(1);
    requested().map_or(pool, |n| n.min(pool))
}

/// `f(0)..f(n)` in index order. With `parallel` false, one item or a
/// single-threaded pool, the calls run here one after another.
pub fn map<T, F>(n: usize, parallel: bool, f: F) -> Vec<T>
where
    T: Send + 'static,
    F: Fn(usize) -> T + Sync,
{
    if !parallel || n < 2 || threads() < 2 {
        return (0..n).map(f).collect();
    }
    let f = &f;
    pool().scope(|scope| {
        for i in 0..n {
            scope.spawn(async move { f(i) });
        }
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn map_keeps_index_order() {
        let square = |i: usize| {
            // Uneven work so that tasks finish out of order.
            std::thread::sleep(std::time::Duration::from_micros(((7 - i % 8) * 50) as u64));
            i * i
        };
        let expected: Vec<usize> = (0..24).map(|i| i * i).collect();
        assert_eq!(super::map(24, true, square), expected);
        assert_eq!(super::map(24, false, square), expected);
    }
}
