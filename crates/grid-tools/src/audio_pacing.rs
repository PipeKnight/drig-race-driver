//! Tick-paced audio parameters (docs/AUDIO_RESEARCH.md §8). The vehicle
//! ticks at 62.5 Hz (16 ms) whatever the frame rate, and the engine and tyre
//! mixers run one 16 ms block (768 samples at 48 kHz) at a time. The game
//! pushes one value per physics tick and the audio thread takes one per
//! block, so a slow frame that runs three ticks back to back still feeds the
//! mixers three 16 ms steps instead of one 50 ms jump. At 60 fps one tick
//! runs per frame and the mixers see the same values as before.
use std::collections::VecDeque;

/// Values queued at most; older ones are dropped by the producer.
const CAPACITY: usize = 8;
/// Above this many queued values the consumer skips ahead to `RESUME`, so
/// the audio clock and the tick clock cannot drift apart.
const BACKLOG: usize = 4;
const RESUME: usize = 2;

#[derive(Clone, Debug)]
pub struct Paced<T: Copy> {
    queue: VecDeque<T>,
    last: T,
}

impl<T: Copy> Paced<T> {
    pub fn new(value: T) -> Self {
        Self {
            queue: VecDeque::with_capacity(CAPACITY),
            last: value,
        }
    }

    /// One value per physics tick (or per frame while nothing ticks).
    pub fn push(&mut self, value: T) {
        if self.queue.len() >= CAPACITY {
            self.queue.pop_front();
        }
        self.queue.push_back(value);
    }

    /// The value for the next 16 ms audio block: the oldest queued one, or
    /// the last one again when nothing new arrived.
    pub fn take(&mut self) -> T {
        if self.queue.len() > BACKLOG {
            let skip = self.queue.len() - RESUME;
            self.queue.drain(..skip);
        }
        if let Some(value) = self.queue.pop_front() {
            self.last = value;
        }
        self.last
    }

    /// The newest value, queued or taken.
    pub fn latest(&self) -> T {
        self.queue.back().copied().unwrap_or(self.last)
    }

    pub fn queued(&self) -> usize {
        self.queue.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slow_frame_is_played_out_one_tick_per_block() {
        let mut p = Paced::new(0);
        // A 50 ms frame runs three ticks back to back.
        for tick in 1..=3 {
            p.push(tick);
        }
        assert_eq!([p.take(), p.take(), p.take()], [1, 2, 3]);
        // Nothing new: the last value holds.
        assert_eq!(p.take(), 3);
    }

    #[test]
    fn a_backlog_skips_ahead_and_the_queue_is_bounded() {
        let mut p = Paced::new(0);
        for v in 1..=20 {
            p.push(v);
        }
        assert_eq!(p.queued(), CAPACITY);
        assert_eq!(p.latest(), 20);
        // 13..20 queued; the consumer resumes two values from the end.
        assert_eq!(p.take(), 19);
        assert_eq!(p.take(), 20);
        assert_eq!(p.take(), 20);
    }
}
