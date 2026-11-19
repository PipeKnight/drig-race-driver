//! Stdout lines go to a background writer (F861).
//!
//! The drive writes about five diagnostic lines per simulation tick
//! (`drive_sample` and its siblings). The launchers and the smoke tools
//! redirect stdout to a file, so each line was a synchronous file write on
//! the main thread: 0.3-0.8 ms per frame on average, with single writes of
//! up to 7 ms (F861 probe, docs/PERFORMANCE.md "Main-world peaks (F861)").
//!
//! Now `println!`, `log_line!` and `eprintln!` in this crate (main.rs shadows
//! the first and the last) append to one queue in call order; a thread
//! writes the queue to stdout in the same order, in one write per wake-up.
//! Every line still reaches stdout:
//! - `eprintln!` writes the queue first, so a log that merges stdout and
//!   stderr keeps the order it had;
//! - [`exit`] and the end of `main` write the rest, and a panic writes it
//!   before the panic message;
//! - past [`LIMIT`] queued bytes the caller writes the queue itself, as
//!   before, so a stalled stdout cannot grow memory without bound.
//!
//! Lines from other crates (`println!` in grid-world or grid-tools) and from
//! Bevy's log go out directly and may land a few milliseconds earlier
//! relative to queued lines than before. Unit tests print directly, so the
//! test harness still captures their output.

use std::io::Write;
use std::sync::{Condvar, LazyLock, Mutex, MutexGuard, PoisonError};

/// Queued bytes past which the caller writes the queue itself.
const LIMIT: usize = 4 << 20;

/// The queue and the sink it is written to.
struct Out<W: Write> {
    queue: Mutex<Queue>,
    ready: Condvar,
    /// Held while a chunk is taken from the queue and written, so chunks
    /// reach the sink in queue order whoever writes them.
    sink: Mutex<W>,
}

#[derive(Default)]
struct Queue {
    bytes: Vec<u8>,
    /// The last written chunk, cleared, to take the next one without
    /// allocating.
    spare: Vec<u8>,
    /// The writer thread runs (or could not start: then callers write).
    started: Option<bool>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl<W: Write> Out<W> {
    fn new(sink: W) -> Self {
        Out { queue: Mutex::new(Queue::default()), ready: Condvar::new(), sink: Mutex::new(sink) }
    }

    /// Appends one line; false when the caller must write the queue (no
    /// writer thread, or past `LIMIT`).
    fn push(&self, args: std::fmt::Arguments) -> bool {
        // Formatted before the lock: a panic while formatting must not leave
        // the queue locked for the panic hook's flush.
        let mut text = Vec::with_capacity(256);
        let _ = writeln!(text, "{args}");
        let mut queue = lock(&self.queue);
        queue.bytes.extend_from_slice(&text);
        let queued = queue.started == Some(true) && queue.bytes.len() <= LIMIT;
        drop(queue);
        if queued {
            self.ready.notify_one();
        }
        queued
    }

    /// Writes everything queued so far to the sink and flushes it.
    fn flush(&self) {
        let mut sink = lock(&self.sink);
        let chunk = self.take();
        let _ = sink.write_all(&chunk);
        let _ = sink.flush();
        drop(sink);
        self.give_back(chunk);
    }

    fn take(&self) -> Vec<u8> {
        let mut queue = lock(&self.queue);
        let spare = std::mem::take(&mut queue.spare);
        std::mem::replace(&mut queue.bytes, spare)
    }

    fn give_back(&self, mut chunk: Vec<u8>) {
        chunk.clear();
        let mut queue = lock(&self.queue);
        if queue.spare.capacity() < chunk.capacity() {
            queue.spare = chunk;
        }
    }

    /// The writer thread: waits for lines, then writes them.
    fn run(&self) {
        loop {
            let queue = lock(&self.queue);
            let queue = self.ready.wait_while(queue, |q| q.bytes.is_empty()).unwrap_or_else(PoisonError::into_inner);
            drop(queue);
            self.flush();
        }
    }
}

static STDOUT: LazyLock<Out<std::io::Stdout>> = LazyLock::new(|| Out::new(std::io::stdout()));

/// One line, as `println!` formats it.
pub fn line(args: std::fmt::Arguments) {
    if cfg!(test) {
        std::println!("{args}");
        return;
    }
    let out = &*STDOUT;
    if lock(&out.queue).started.is_none() {
        start(out);
    }
    if !out.push(args) {
        out.flush();
    }
}

fn start(out: &'static Out<std::io::Stdout>) {
    let mut queue = lock(&out.queue);
    if queue.started.is_some() {
        return;
    }
    let spawned = std::thread::Builder::new().name("stdout".into()).spawn(move || out.run());
    queue.started = Some(spawned.is_ok());
    drop(queue);
    // Lines queued before a panic or a lost thread are written first.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        flush();
        previous(info);
    }));
}

/// Writes every queued line now (before stderr output, and at exit).
pub fn flush() {
    if let Some(out) = LazyLock::get(&STDOUT) {
        out.flush();
    }
}

/// `std::process::exit` after the queued lines are written.
pub fn exit(code: i32) -> ! {
    flush();
    std::process::exit(code)
}

/// Writes the queue when `main` returns.
pub struct FlushOnDrop;

impl Drop for FlushOnDrop {
    fn drop(&mut self) {
        flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    /// Lines from several threads all reach the sink, each thread's in its
    /// own order, whether the writer thread or a caller writes them.
    #[test]
    fn every_line_reaches_the_sink_in_order() {
        let out = Arc::new(Out::new(Vec::new()));
        lock(&out.queue).started = Some(true);
        let writer = Arc::clone(&out);
        std::thread::spawn(move || writer.run());
        let threads: Vec<_> = (0..4)
            .map(|t| {
                let out = Arc::clone(&out);
                std::thread::spawn(move || {
                    for i in 0..2000 {
                        if !out.push(format_args!("t{t} {i}")) || i % 500 == 0 {
                            out.flush();
                        }
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        out.flush();
        let text = String::from_utf8(lock(&out.sink).clone()).unwrap();
        let mut next = [0; 4];
        for line in text.lines() {
            let (t, i) = line[1..].split_once(' ').unwrap();
            let (t, i): (usize, usize) = (t.parse().unwrap(), i.parse().unwrap());
            assert_eq!(i, next[t], "thread {t} out of order");
            next[t] += 1;
        }
        assert_eq!(next, [2000; 4]);
    }

    /// Past `LIMIT` the caller is told to write the queue itself.
    #[test]
    fn a_full_queue_is_written_by_the_caller() {
        let out = Out::new(Vec::new());
        lock(&out.queue).started = Some(true);
        let long = "x".repeat(1 << 20);
        assert!((0..3).all(|_| out.push(format_args!("{long}"))));
        assert!(!out.push(format_args!("{long}")));
        out.flush();
        assert_eq!(lock(&out.sink).len(), 4 * ((1 << 20) + 1));
        assert!(lock(&out.queue).bytes.is_empty());
    }
}
