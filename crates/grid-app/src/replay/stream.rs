//! The replay byte stream: encoded frames appended to chunks, the newest
//! chunks in memory and older ones spooled to a cache file, like the
//! original's two replay buffers (docs/REPLAY.md):
//!
//! - an in-memory buffer of 0x380000 bytes (3.5 MiB), constructed at
//!   0x64EDDF/0x936670 inside the 7.5 MiB `REPLAY` pool of
//!   `system/updateMemoryMap.xml`; seeks clamp to its oldest time (0x91CC00),
//!   which bounds Instant Replay;
//! - a disk-backed stream (`replay.pbf` in the per-user replay cache, opened
//!   with overlapped unbuffered I/O at 0x871D40) created with the limit
//!   0x7D000000 bytes (0x92E0E0), which keeps the whole race for the full
//!   replay. Without the cache the original keeps memory only ("Cache will
//!   be disabled").
//!
//! The chunk size and the file format are AUTHORED; the file is the port's
//! own private spool, deleted when the stream is cleared or dropped.
use std::{
    collections::VecDeque,
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::PathBuf,
};

/// [D] in-memory replay buffer, bytes.
pub const MEMORY_BYTES: usize = 0x38_0000;
/// [D] disk-backed replay stream limit, bytes.
pub const SPOOL_BYTES: u64 = 0x7D00_0000;
/// AUTHORED chunk size (the REPLAY pool is requested with 0x100000 at
/// 0x64EE25).
pub const CHUNK_BYTES: usize = 0x10_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    pub memory: usize,
    pub chunk: usize,
    pub spool: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            memory: MEMORY_BYTES,
            chunk: CHUNK_BYTES,
            spool: SPOOL_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Entry {
    tick: u64,
    chunk: u32,
    offset: u32,
    len: u32,
}

#[derive(Debug)]
struct Chunk {
    number: u32,
    data: Option<Vec<u8>>,
    /// Offset in the spool file once written there.
    spooled: Option<u64>,
    len: usize,
}

struct Spool {
    path: PathBuf,
    file: File,
    written: u64,
}

impl Drop for Spool {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

pub struct Stream {
    limits: Limits,
    chunks: VecDeque<Chunk>,
    index: Vec<Entry>,
    /// Frames before this one are gone (dropped without a spool).
    first: usize,
    spool: Option<Spool>,
    /// Recording stopped: the spool limit was reached.
    full: bool,
    cache: Option<(u32, Vec<u8>)>,
}

impl Stream {
    /// `spool`: the cache file; `None` keeps memory only.
    pub fn new(limits: Limits, spool: Option<PathBuf>) -> Self {
        let spool = spool.and_then(|path| {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).ok()?;
            }
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path)
                .ok()?;
            Some(Spool {
                path,
                file,
                written: 0,
            })
        });
        Self {
            limits,
            chunks: VecDeque::new(),
            index: Vec::new(),
            first: 0,
            spool,
            full: false,
            cache: None,
        }
    }

    pub fn spooling(&self) -> bool {
        self.spool.is_some()
    }

    pub fn full(&self) -> bool {
        self.full
    }

    /// Forget every frame (a new race); the spool file is reused.
    pub fn clear(&mut self) {
        self.chunks.clear();
        self.index.clear();
        self.first = 0;
        self.full = false;
        self.cache = None;
        if let Some(spool) = &mut self.spool {
            spool.written = 0;
            let _ = spool.file.set_len(0);
        }
    }

    /// Keeps frames up to and including `last` and forgets the later ones,
    /// so recording continues after `last` (a Flashback jump-in, the
    /// original's 0x93DDA0 cuts its stream there). `last` must be in memory.
    pub fn truncate(&mut self, last: usize) -> bool {
        if last < self.memory_start() || last >= self.index.len() {
            return false;
        }
        let keep = self.index[last];
        self.index.truncate(last + 1);
        while self.chunks.back().is_some_and(|c| c.number > keep.chunk) {
            self.chunks.pop_back();
        }
        if let Some(chunk) = self.chunks.back_mut()
            && let Some(data) = chunk.data.as_mut()
        {
            data.truncate((keep.offset + keep.len) as usize);
            chunk.len = data.len();
        }
        self.full = false;
        self.cache = None;
        true
    }

    /// Number of the next frame.
    pub fn next(&self) -> u32 {
        self.index.len() as u32
    }

    /// Readable frames, oldest first.
    pub fn frames(&self) -> std::ops::Range<usize> {
        self.first..self.index.len()
    }

    /// The first frame still in memory (the Instant Replay window).
    pub fn memory_start(&self) -> usize {
        self.chunks
            .iter()
            .find(|c| c.data.is_some())
            .map_or(self.index.len(), |c| {
                self.index
                    .partition_point(|e| e.chunk < c.number)
                    .max(self.first)
            })
    }

    pub fn tick(&self, frame: usize) -> u64 {
        self.index[frame].tick
    }

    /// The last readable frame at or before `tick`.
    pub fn at_or_before(&self, tick: u64) -> Option<usize> {
        let n = self.index.partition_point(|e| e.tick <= tick);
        (n > self.first).then_some(n - 1)
    }

    pub fn memory_bytes(&self) -> usize {
        self.chunks
            .iter()
            .filter_map(|c| c.data.as_ref())
            .map(Vec::len)
            .sum()
    }

    pub fn spooled_bytes(&self) -> u64 {
        self.spool.as_ref().map_or(0, |s| s.written)
    }

    /// Appends a frame; false once recording has stopped. Frames before
    /// [`Self::frames`]`.start` are no longer readable afterwards, so the
    /// encoder must not reference them.
    pub fn push(&mut self, tick: u64, bytes: &[u8]) -> bool {
        if self.full || bytes.len() > self.limits.chunk {
            self.full = true;
            return false;
        }
        let open = self
            .chunks
            .back()
            .is_some_and(|c| c.data.as_ref().is_some_and(|d| d.len() + bytes.len() <= self.limits.chunk));
        if !open {
            let number = self.chunks.back().map_or(0, |c| c.number + 1);
            self.chunks.push_back(Chunk {
                number,
                data: Some(Vec::with_capacity(self.limits.chunk.min(1 << 16))),
                spooled: None,
                len: 0,
            });
        }
        let chunk = self.chunks.back_mut().unwrap();
        let data = chunk.data.as_mut().unwrap();
        let offset = data.len() as u32;
        data.extend_from_slice(bytes);
        chunk.len = data.len();
        self.index.push(Entry {
            tick,
            chunk: chunk.number,
            offset,
            len: bytes.len() as u32,
        });
        self.trim();
        true
    }

    /// Moves the oldest sealed chunks out of memory while it is over budget.
    fn trim(&mut self) {
        while self.memory_bytes() > self.limits.memory {
            let last = self.chunks.len() - 1;
            let Some(i) = self.chunks.iter().position(|c| c.data.is_some()) else {
                return;
            };
            if i == last {
                return;
            }
            let chunk = &mut self.chunks[i];
            let data = chunk.data.take().unwrap();
            let written = self.spool.as_mut().and_then(|spool| {
                if spool.written + data.len() as u64 > self.limits.spool {
                    return None;
                }
                spool.file.seek(SeekFrom::Start(spool.written)).ok()?;
                spool.file.write_all(&data).ok()?;
                let at = spool.written;
                spool.written += data.len() as u64;
                Some(at)
            });
            match written {
                Some(at) => chunk.spooled = Some(at),
                None if self.spool.is_some() => {
                    // Spool full or failing: keep what is readable and stop.
                    chunk.data = Some(data);
                    self.full = true;
                    return;
                }
                None => {
                    // Memory only: the oldest frames are gone.
                    let number = chunk.number;
                    self.chunks.remove(i);
                    self.first = self
                        .first
                        .max(self.index.partition_point(|e| e.chunk <= number));
                }
            }
        }
    }

    /// The bytes of frame `frame`; `None` once gone or on a read error.
    pub fn read(&mut self, frame: usize) -> Option<Vec<u8>> {
        if !self.frames().contains(&frame) {
            return None;
        }
        let e = self.index[frame];
        let range = e.offset as usize..(e.offset + e.len) as usize;
        let chunk = self.chunks.iter().find(|c| c.number == e.chunk)?;
        if let Some(data) = &chunk.data {
            return data.get(range).map(<[u8]>::to_vec);
        }
        if self.cache.as_ref().is_none_or(|(n, _)| *n != e.chunk) {
            let at = chunk.spooled?;
            let mut buffer = vec![0; chunk.len];
            let spool = self.spool.as_mut()?;
            spool.file.seek(SeekFrom::Start(at)).ok()?;
            spool.file.read_exact(&mut buffer).ok()?;
            self.cache = Some((e.chunk, buffer));
        }
        self.cache.as_ref()?.1.get(range).map(<[u8]>::to_vec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(n: u64, len: usize) -> Vec<u8> {
        (0..len).map(|i| (n as usize * 31 + i) as u8).collect()
    }

    #[test]
    fn memory_only_keeps_a_bounded_window() {
        let limits = Limits {
            memory: 1000,
            chunk: 300,
            spool: 0,
        };
        let mut s = Stream::new(limits, None);
        for n in 0..100u64 {
            assert!(s.push(n * 2, &bytes(n, 50)));
            assert!(s.memory_bytes() <= 1000 + 300, "{}", s.memory_bytes());
        }
        let frames = s.frames();
        assert!(frames.start > 0 && frames.end == 100);
        assert_eq!(s.memory_start(), frames.start);
        for f in frames.clone() {
            assert_eq!(s.read(f).unwrap(), bytes(f as u64, 50));
        }
        assert!(s.read(frames.start - 1).is_none());
        assert_eq!(s.at_or_before(0), None);
        assert_eq!(s.at_or_before(199), Some(99));
        assert_eq!(s.at_or_before(frames.start as u64 * 2 + 1), Some(frames.start));
    }

    #[test]
    fn a_jump_in_cut_keeps_the_frames_up_to_it_and_records_on() {
        let limits = Limits {
            memory: 1000,
            chunk: 300,
            spool: 0,
        };
        let mut s = Stream::new(limits, None);
        for n in 0..100u64 {
            s.push(n * 2, &bytes(n, 50));
        }
        let start = s.frames().start;
        assert!(!s.truncate(start - 1), "before the memory window");
        assert!(!s.truncate(100));
        assert!(s.truncate(95));
        assert_eq!(s.frames(), start..96);
        assert_eq!(s.at_or_before(1000), Some(95));
        assert_eq!(s.read(95).unwrap(), bytes(95, 50));
        // Recording continues from the cut with new frames.
        for n in 96..100u64 {
            assert!(s.push(n * 2 + 1, &bytes(n + 7, 50)));
        }
        for f in s.frames() {
            let n = f as u64;
            assert_eq!(s.read(f).unwrap(), bytes(if n > 95 { n + 7 } else { n }, 50));
        }
        assert_eq!(s.tick(96), 193);
    }

    #[test]
    fn the_spool_keeps_everything_and_reads_it_back() {
        let dir = std::env::temp_dir().join(format!("grid-replay-test-{}", std::process::id()));
        let path = dir.join("spool.bin");
        let limits = Limits {
            memory: 1000,
            chunk: 300,
            spool: 1 << 20,
        };
        let mut s = Stream::new(limits, Some(path.clone()));
        assert!(s.spooling());
        for n in 0..200u64 {
            assert!(s.push(n, &bytes(n, 70)));
        }
        assert_eq!(s.frames(), 0..200);
        assert!(s.memory_bytes() <= 1000);
        assert!(s.spooled_bytes() > 0);
        assert!(s.memory_start() > 0);
        // Out of order: spooled and in-memory frames alike.
        for f in [0, 150, 3, 199, 77, 78, 1] {
            assert_eq!(s.read(f).unwrap(), bytes(f as u64, 70), "frame {f}");
        }
        s.clear();
        assert_eq!(s.frames(), 0..0);
        assert!(s.push(5, &bytes(5, 70)));
        assert_eq!(s.read(0).unwrap(), bytes(5, 70));
        drop(s);
        assert!(!path.exists());
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn a_full_spool_stops_recording() {
        let dir = std::env::temp_dir().join(format!("grid-replay-full-{}", std::process::id()));
        let limits = Limits {
            memory: 600,
            chunk: 300,
            spool: 600,
        };
        let mut s = Stream::new(limits, Some(dir.join("spool.bin")));
        let mut stored = 0;
        for n in 0..100u64 {
            if !s.push(n, &bytes(n, 100)) {
                break;
            }
            stored += 1;
        }
        assert!(s.full());
        assert!(stored < 100);
        assert_eq!(s.frames(), 0..stored);
        for f in 0..stored {
            assert_eq!(s.read(f).unwrap(), bytes(f as u64, 100));
        }
        drop(s);
        let _ = std::fs::remove_dir_all(dir);
    }
}
