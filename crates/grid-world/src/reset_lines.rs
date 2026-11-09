//! Player auto-reset arithmetic of the original ResetLinesManager
//! (docs/RESET_AND_RESPAWN_RESEARCH.md 3): the nearly-still-and-on-its-side
//! detector 0x60D590, the placement height limit of 0x9145D0 and the speed-0
//! tail of the grid-spawn setter 0x9EF040 (wheel spin and gear). The caller
//! owns the ground placement (`spawn::align_below`) and the pose setter.
use crate::sse_approx::rsqrt;

/// 0xDB29F0: speed plus spin at or below this counts as still.
pub const STILL: f32 = 0.8;
/// 0xDB29EC: an up-axis Y at or below this counts as on the side or roof.
pub const TILTED: f32 = 0.17;
/// 0xDB29E8: seconds the car must stay still and tilted.
pub const DELAY: f32 = 2.0;
/// Upper timer bound of the firing branch (research 3.1).
const LATCH: f32 = 3.0;
/// 0xCAFBE8 / 0xCAFBE4: half span and span of the limit's vertical cast.
pub const LIMIT_REACH: f32 = 10000.;
const LIMIT_SPAN: f32 = 20000.;

/// Vector length as the game's inline SSE helper computes it (seen in
/// 0x5EC0E0): z² + (y² + x²), one RSQRTPS seed refined once, zero kept. The
/// seed comes from the measured table, never the host instruction (F320).
pub fn length(v: [f32; 3]) -> f32 {
    let square = v[2] * v[2] + (v[1] * v[1] + v[0] * v[0]);
    if square == 0. {
        return 0.;
    }
    let seed = rsqrt(square);
    let root = seed * square;
    (1. - root * seed) * root * 0.5 + root
}

/// Auto-reset timer (manager +0x214).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Detector {
    pub timer: f32,
}

impl Detector {
    /// One update with the body's linear and angular velocity, the pose's
    /// up-axis Y (row 1 . y) and the frame time. True once per still-and-
    /// tilted stretch, after [`DELAY`] seconds: the reset request.
    pub fn update(&mut self, linear: [f32; 3], angular: [f32; 3], up_y: f32, delta: f32) -> bool {
        let speed = length(linear) + length(angular);
        // A non-finite state is never still; it restarts the stretch.
        let still = speed <= STILL && up_y <= TILTED;
        if !still {
            self.timer = 0.;
        } else if self.timer < DELAY {
            self.timer += delta;
        } else if self.timer < LATCH {
            self.timer += DELAY;
            return true;
        }
        false
    }
}

/// Placement limit of 0x9145D0 for a line point at height `y`: max(y + 1,
/// ground + 1), where the ground is the hit of a vertical cast from
/// y + 1 - 10000 up to y + 1 + 10000 at `fraction` (None: no hit).
pub fn placement_limit(y: f32, fraction: Option<f32>) -> f32 {
    let top = y + 1.;
    let start = top - LIMIT_REACH;
    match fraction {
        Some(f) if top < f * LIMIT_SPAN + start + 1. => f * LIMIT_SPAN + start + 1.,
        _ => top,
    }
}

/// Start and end Y of the limit cast for a line point at height `y`.
pub fn limit_cast(y: f32) -> [f32; 2] {
    let top = y + 1.;
    [top - LIMIT_REACH, top + LIMIT_REACH]
}

fn get(words: &[u32], offset: usize) -> f32 {
    f32::from_bits(words[offset / 4])
}
fn set(words: &mut [u32], offset: usize, value: f32) {
    words[offset / 4] = value.to_bits();
}

/// 0xD15028 / 0xD1502C: torque-curve index scale and its top speed.
const CURVE_SCALE: f32 = f32::from_bits(0x3c97_9102);
const CURVE_TOP: f32 = f32::from_bits(0x44d1_7084);

/// Speed-0 tail of 0x9EF040 on the car words: every wheel's spin
/// (+0x104) = speed / radius (+0xE0), then the gear (+0x196C) with the most
/// torque at the resulting engine speed and the engine speed (+0x1894).
/// [I] The engine speed source *(+0x18D8)+0x104 is a wheel spin, so it is 0
/// once the spins are cleared.
pub fn stop_drivetrain(words: &mut [u32]) {
    const SPEED: f32 = 0.;
    let wheels = (words[0x1620 / 4] as usize).min(4);
    for i in 0..wheels {
        let w = 0x1990 + i * 0x150;
        set(words, w + 0x104, SPEED / get(words, w + 0xe0));
    }
    let spin = 0f32;
    let mut best = 0f32;
    let mut gear = 0;
    for i in 0..10 {
        let ratio = get(words, 0x18f4 + i * 4);
        if ratio == 0. {
            continue;
        }
        let mut speed = spin / ratio;
        let torque = if speed > CURVE_TOP {
            0.
        } else {
            // Negative or NaN reads node 0 (the original's else branch).
            if speed < 0. || speed.is_nan() {
                speed = 0.;
            }
            let index = (speed * CURVE_SCALE) as i32;
            let fraction = speed * CURVE_SCALE - index as f32;
            let index = index.clamp(0, 0x1e) as usize;
            let low = get(words, 0x17ac + index * 4);
            if low == 0. {
                0.
            } else {
                (get(words, 0x17b0 + index * 4) - low) * fraction + low
            }
        };
        if best < torque {
            gear = i;
            best = torque;
        }
    }
    words[0x196c / 4] = gear as u32;
    let ratio = get(words, 0x18f4 + gear * 4);
    let engine = if ratio == 0. {
        get(words, 0x17a8)
    } else {
        spin / ratio
    };
    set(words, 0x1894, engine);
    if gear == 0 {
        words[0x196c / 4] = 1;
    }
}

/// 0x5EBDA0 / 0x612C90 (kind 0, the player): seconds of the fade; the job
/// value v3 is set to this at event 0 (0xCB0948 holds -1 at request time).
pub const FADE: f32 = 1.0;
/// Job v4: the fade value at which the car is placed (event 4), half way.
pub const MIDPOINT: f32 = 0.5;
/// 0xCB093C (job v6): seconds after the fade before collisions with other
/// cars may come back.
pub const TAIL: f32 = 5.0;
/// Manager +0x224 (0x612870): race seconds during which a circuit target is
/// moved ahead (0x5EC0E0).
pub const EARLY: f32 = 15.0;
/// Manager +0x220: metres ahead per metre between the car and its line point.
pub const EARLY_AHEAD: f32 = 1.5;
/// 0x612C90 event 4: placements tried while the car overlaps another car.
pub const OVERLAP_TRIES: usize = 20;
/// Metres stepped back along the line per retry (0x90C0C0(1.0) -> 0x90BEC0).
pub const OVERLAP_STEP: f32 = 1.0;

/// The track tracker's line (the route of `progress_track.xml`, not the AI
/// racing line): progress points in travel order with their distances.
/// 0x912980 places a trackpos by lerping the two neighbouring points'
/// positions and their horizontal directions (0x909C00); the direction is
/// normalised and, on a reversed route, negated.
#[derive(Clone, Debug, PartialEq)]
pub struct TrackerLine {
    points: Vec<[f32; 3]>,
    distances: Vec<f32>,
    directions: Vec<[f32; 3]>,
    closed: bool,
    length: f32,
}

/// Horizontal unit vector from `a` toward `b` (0x909C00), zero when level.
fn horizontal(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    let d = [b[0] - a[0], 0., b[2] - a[2]];
    let l = length(d);
    if l > 0. { [d[0] / l, 0., d[2] / l] } else { [0.; 3] }
}

impl TrackerLine {
    /// `points` as (position, distance) in travel order with rising
    /// distances from the run's start. A closed line (a circuit) wraps from
    /// its last point to its first at `length`; an open one (a point-to-point
    /// run) ends at its last point. `reversed` is a run against the file's
    /// point order (Haruna's downhill routes): the original then takes each
    /// point's direction from the file order and negates it, which in travel
    /// order is the direction from the previous point.
    pub fn new(points: &[([f32; 3], f32)], closed: bool, reversed: bool, length: f32) -> Option<Self> {
        let n = points.len();
        if n < 2
            || !length.is_finite()
            || points.iter().any(|(p, d)| !d.is_finite() || p.iter().any(|v| !v.is_finite()))
            || points.windows(2).any(|w| w[1].1 <= w[0].1)
            || (closed && length <= points[n - 1].1 - points[0].1)
        {
            return None;
        }
        let at = |i: usize| points[i].0;
        let forward = |i: usize| match (i + 1 < n, closed) {
            (true, _) => horizontal(at(i), at(i + 1)),
            (false, true) => horizontal(at(i), at(0)),
            // [I] An open line's last point keeps its segment's direction.
            (false, false) => horizontal(at(i - 1), at(i)),
        };
        let directions = (0..n)
            .map(|i| {
                if !reversed {
                    forward(i)
                } else {
                    match (i > 0, closed) {
                        (true, _) => horizontal(at(i - 1), at(i)),
                        (false, true) => horizontal(at(n - 1), at(0)),
                        // [I] The file's next point is past the run's start.
                        (false, false) => forward(0),
                    }
                }
            })
            .collect();
        Some(Self {
            points: points.iter().map(|p| p.0).collect(),
            distances: points.iter().map(|p| p.1).collect(),
            directions,
            closed,
            length: if closed { length } else { points[n - 1].1 },
        })
    }

    pub fn closed(&self) -> bool {
        self.closed
    }

    /// Run length: the lap on a closed line, the last point on an open one.
    pub fn length(&self) -> f32 {
        self.length
    }

    /// `d` on the line: wrapped on a closed line, clamped on an open one
    /// (0x90BFC0 / 0x90BEC0 keep a trackpos on its line this way).
    pub fn normalise(&self, d: f32) -> f32 {
        if self.closed {
            let start = self.distances[0];
            start + (d - start).rem_euclid(self.length)
        } else {
            d.clamp(self.distances[0], self.length)
        }
    }

    /// 0x90BFC0 (forward, `delta` > 0) and 0x90BEC0 (backward): `d` moved
    /// along the line, wrapping or clamping at its ends.
    pub fn step(&self, d: f32, delta: f32) -> f32 {
        self.normalise(d + delta)
    }

    /// The segment from point `i`: its end point, start distance and span.
    fn segment(&self, i: usize) -> (usize, f32, f32) {
        let n = self.points.len();
        if i + 1 < n {
            (i + 1, self.distances[i], self.distances[i + 1] - self.distances[i])
        } else {
            (0, self.distances[i], self.length - self.distances[i] + self.distances[0])
        }
    }

    /// 0x912980: position and normalised horizontal travel direction at `d`.
    pub fn point_at(&self, d: f32) -> ([f32; 3], [f32; 3]) {
        let d = self.normalise(d);
        let n = self.points.len();
        let i = self.distances.partition_point(|&x| x <= d).saturating_sub(1);
        if !self.closed && i + 1 >= n {
            return (self.points[n - 1], self.directions[n - 1]);
        }
        let (j, start, span) = self.segment(i);
        let t = if span > 0. { ((d - start) / span).clamp(0., 1.) } else { 0. };
        let lerp = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] {
            std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t)
        };
        let point = lerp(self.points[i], self.points[j]);
        let direction = lerp(self.directions[i], self.directions[j]);
        let l = length(direction);
        let direction = if l > 0. { direction.map(|v| v / l) } else { self.directions[i] };
        (point, direction)
    }

    /// Nearest point of the segments from `candidates` to `p`: (segment,
    /// distance along the line, squared separation).
    fn nearest(&self, p: [f32; 3], candidates: impl Iterator<Item = usize>) -> Option<(usize, f32, f32)> {
        let mut best: Option<(usize, f32, f32)> = None;
        for i in candidates {
            let (j, start, span) = self.segment(i);
            let (a, b) = (self.points[i], self.points[j]);
            let ab = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let ap = [p[0] - a[0], p[1] - a[1], p[2] - a[2]];
            let ab2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
            let t = if ab2 > 0. {
                ((ap[0] * ab[0] + ap[1] * ab[1] + ap[2] * ab[2]) / ab2).clamp(0., 1.)
            } else {
                0.
            };
            let q: [f32; 3] = std::array::from_fn(|k| ap[k] - ab[k] * t);
            let squared = q[0] * q[0] + q[1] * q[1] + q[2] * q[2];
            if best.is_none_or(|b| squared < b.2) {
                best = Some((i, self.normalise(start + span * t), squared));
            }
        }
        best
    }

    fn segments(&self) -> usize {
        if self.closed { self.points.len() } else { self.points.len() - 1 }
    }
}

/// [`Tracker`] window: segments searched around the last one.
const WINDOW_BACK: usize = 2;
const WINDOW_AHEAD: usize = 6;
/// [`Tracker`]: a windowed match farther than this (squared metres) falls
/// back to the whole line.
const WINDOW_LOST: f32 = 30. * 30.;

/// The car's track tracker: its trackpos on a [`TrackerLine`] (record
/// +0x74/+0x78, updated each frame by 0x915100 from the car's position).
/// AUTHORED: the update is a nearest-point projection searched around the
/// last segment (the whole line at first and when the car is far from that
/// window); the original's own tracker update is not ported (research 7).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tracker {
    pub segment: Option<usize>,
    pub distance: f32,
}

impl Tracker {
    pub fn update(&mut self, line: &TrackerLine, p: [f32; 3]) {
        let n = line.segments();
        let windowed = self.segment.filter(|&h| h < n).and_then(|h| {
            let candidates: Vec<usize> = if line.closed {
                let back = WINDOW_BACK.min(n);
                (0..(WINDOW_BACK + WINDOW_AHEAD).min(n)).map(|k| (h + n - back + k) % n).collect()
            } else {
                (h.saturating_sub(WINDOW_BACK)..(h + WINDOW_AHEAD).min(n)).collect()
            };
            line.nearest(p, candidates.into_iter()).filter(|b| b.2 <= WINDOW_LOST)
        });
        if let Some((segment, distance, _)) = windowed.or_else(|| line.nearest(p, 0..n)) {
            self.segment = Some(segment);
            self.distance = distance;
        }
    }
}

/// 0x5EC0E0 for a request without a saved trackpos: the tracker's trackpos
/// `distance` for a car at `car`. On a closed route during the first
/// [`EARLY`] seconds of `clock` (trackers +0x120, [I] the race clock) it moves
/// ahead by [`EARLY_AHEAD`] times the car's 3D distance from its line point;
/// on an open route a trackpos past `finish` comes back to it. The hold
/// branch (manager +0xD8, 88 m) is never armed in the executable.
pub fn target(line: &TrackerLine, distance: f32, car: [f32; 3], clock: f32, finish: Option<f32>) -> f32 {
    if line.closed {
        if clock < EARLY {
            let (point, _) = line.point_at(distance);
            let off = length([car[0] - point[0], car[1] - point[1], car[2] - point[2]]);
            return line.step(distance, EARLY_AHEAD * off);
        }
        line.normalise(distance)
    } else {
        let distance = line.normalise(distance);
        match finish {
            Some(finish) if distance > finish => finish,
            _ => distance,
        }
    }
}

/// What a [`Job`] update asks of its caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobEvent {
    /// Event 4: place the car on the target (then collide with no other car).
    Place,
    /// Event 9, each frame after the tail: restore car collisions and end
    /// the job once the car overlaps no other car, else ask again.
    TailOver,
}

/// One player reset job of the ResetLinesManager (0x60D0E0 request, phase
/// machine 0x5EBDA0, update 0x612C90, kind 0). Controls are off from the
/// fade start (event 2, 0x606990 disable) until the placement (event 4,
/// enable); the car is placed when the fade passes [`MIDPOINT`]; after the
/// fade, a [`TAIL`] of seconds runs before collisions with other cars may
/// return (0x5EBF90 layers 0xE and 0x12).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Job {
    /// v3: fade time left.
    pub fade: f32,
    /// v6: tail time left.
    pub tail: f32,
    /// The trackpos chosen at event 0.
    pub target: f32,
    pub placed: bool,
}

impl Job {
    pub fn new(target: f32) -> Self {
        Self { fade: FADE, tail: TAIL, target, placed: false }
    }

    /// One frame of `delta` seconds.
    pub fn update(&mut self, delta: f32) -> Option<JobEvent> {
        if self.fade > 0. {
            self.fade = (self.fade - delta).max(0.);
            if !self.placed && self.fade <= MIDPOINT {
                self.placed = true;
                return Some(JobEvent::Place);
            }
            return None;
        }
        if self.tail > 0. {
            self.tail -= delta;
            return None;
        }
        Some(JobEvent::TailOver)
    }

    /// Player input is ignored (0x606990 disable) until the placement.
    pub fn controls_off(&self) -> bool {
        !self.placed
    }

    /// Other cars pass through the reset car (layer 0xE) from the
    /// placement until the job ends.
    pub fn ghost(&self) -> bool {
        self.placed
    }

    /// [I] Screen darkness of the fade: 0 to 1 over the fade-out (vehicle
    /// +0x88 with +0x8C = 15 at event 2), 1 at the placement, back to 0 over
    /// the fade-in (+0x89, event 4). The reader of those fields is not
    /// identified (research 7).
    pub fn darkness(&self) -> f32 {
        if self.fade <= 0. {
            return 0.;
        }
        (1. - (self.fade - MIDPOINT).abs() / (FADE - MIDPOINT)).clamp(0., 1.)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_match_the_executable() {
        assert_eq!(STILL.to_bits(), 0x3f4c_cccd);
        assert_eq!(TILTED.to_bits(), 0x3e2e_147b);
        assert_eq!(DELAY.to_bits(), 0x4000_0000);
        assert_eq!(CURVE_SCALE, 0.018_501_762);
        assert_eq!(CURVE_TOP, 1_675.516_1);
    }

    #[test]
    fn length_follows_the_refined_seed() {
        assert_eq!(length([0.; 3]), 0.);
        for v in [[3f32, 4., 0.], [0.1, -0.2, 0.3], [40., 1., -7.]] {
            let exact = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            assert!((length(v) - exact).abs() <= exact * 1e-5, "{v:?}");
        }
    }

    #[test]
    fn detector_fires_once_after_two_still_tilted_seconds() {
        let mut d = Detector::default();
        let mut fired = Vec::new();
        for tick in 0..400 {
            if d.update([0.1, 0., 0.], [0., 0.2, 0.], -1., 0.016) {
                fired.push(tick);
            }
        }
        // 2.0 / 0.016 = 125 accumulating updates (f32 sum reaches 2.0 on the
        // 126th), the next one fires; then the timer stays latched above 3.
        assert_eq!(fired.len(), 1);
        assert!((125..=127).contains(&fired[0]), "{fired:?}");
        // Upright or moving restarts the stretch.
        assert!(!d.update([0.; 3], [0.; 3], 0.9, 0.016));
        assert_eq!(d.timer, 0.);
        d.timer = 1.9;
        assert!(!d.update([0.9, 0., 0.], [0.; 3], -1., 0.016));
        assert_eq!(d.timer, 0.);
        assert!(!d.update([f32::NAN; 3], [0.; 3], -1., 0.016));
        assert_eq!(d.timer, 0.);
        // Exactly at the thresholds still counts.
        d.timer = 0.;
        assert!(!d.update([0.8, 0., 0.], [0.; 3], 0.17, 0.016));
        assert_eq!(d.timer, 0.016);
    }

    #[test]
    fn limit_is_the_higher_of_point_and_ground() {
        assert_eq!(placement_limit(5., None), 6.);
        assert_eq!(limit_cast(5.), [-9994., 10006.]);
        // Ground at 4 (below the point): the point wins.
        assert_eq!(placement_limit(5., Some(0.4999)), 6.);
        // Ground at 10: limit 11.
        let f = (10. - limit_cast(5.)[0]) / LIMIT_SPAN;
        assert!((placement_limit(5., Some(f)) - 11.).abs() < 1e-3);
    }

    #[test]
    fn drivetrain_stops_in_the_first_gear_with_torque() {
        let mut words = vec![0u32; 0x2000 / 4];
        words[0x1620 / 4] = 4;
        for i in 0..4 {
            let w = 0x1990 + i * 0x150;
            set(&mut words, w + 0xe0, 0.3);
            set(&mut words, w + 0x104, 55.);
        }
        set(&mut words, 0x17a8, 230.);
        set(&mut words, 0x17ac, 100.);
        for (i, r) in [0., 3.5, 2.1, 1.4, 1.0, 0.8, 0., 0., 0., -3.2]
            .into_iter()
            .enumerate()
        {
            set(&mut words, 0x18f4 + i * 4, r);
        }
        words[0x196c / 4] = 4;
        set(&mut words, 0x1894, 600.);
        stop_drivetrain(&mut words);
        for i in 0..4 {
            assert_eq!(get(&words, 0x1990 + i * 0x150 + 0x104), 0.);
        }
        assert_eq!(words[0x196c / 4], 1);
        assert_eq!(get(&words, 0x1894), 0.);
        // No torque at rest: gear 0 is chosen, then gear 1 forced, and the
        // neutral ratio 0 gives the idle speed.
        set(&mut words, 0x17ac, 0.);
        stop_drivetrain(&mut words);
        assert_eq!(words[0x196c / 4], 1);
        assert_eq!(get(&words, 0x1894), 230.);
    }

    fn square(closed: bool, reversed: bool) -> TrackerLine {
        // A 100 m square, travelled anticlockwise seen from above (+Y up).
        let points = [([0., 0., 0.], 0.), ([100., 0., 0.], 100.), ([100., 0., 100.], 200.), ([0., 0., 100.], 300.)];
        TrackerLine::new(&points, closed, reversed, 400.).unwrap()
    }

    #[test]
    fn reset_job_constants_match_the_executable() {
        // 0xCB093C tail, manager +0x220 / +0x224 (0x612870), job v4.
        assert_eq!(TAIL.to_bits(), 0x40a0_0000);
        assert_eq!(EARLY_AHEAD.to_bits(), 0x3fc0_0000);
        assert_eq!(EARLY.to_bits(), 0x4170_0000);
        assert_eq!((FADE, MIDPOINT, OVERLAP_TRIES, OVERLAP_STEP), (1., 0.5, 20, 1.));
    }

    #[test]
    fn tracker_line_lerps_points_and_directions() {
        let line = square(true, false);
        let (p, d) = line.point_at(50.);
        assert_eq!(p, [50., 0., 0.]);
        // Half way between the +X and +Z segment directions at the corner.
        let (_, d2) = line.point_at(150.);
        assert!((d[0] - 0.5f32.sqrt()).abs() < 1e-4 && (d[2] - 0.5f32.sqrt()).abs() < 1e-4, "{d:?}");
        assert!((d2[0] + 0.5f32.sqrt()).abs() < 1e-4 && (d2[2] - 0.5f32.sqrt()).abs() < 1e-4, "{d2:?}");
        // The closing segment wraps from the last point to the first.
        assert_eq!(line.point_at(350.).0, [0., 0., 50.]);
        assert_eq!(line.normalise(-10.), 390.);
        assert_eq!(line.step(395., 10.), 5.);
        assert_eq!(line.step(5., -OVERLAP_STEP * 10.), 395.);
        // An open line clamps at both ends and ends at its last point.
        let open = square(false, false);
        assert_eq!(open.length(), 300.);
        assert_eq!(open.step(295., 10.), 300.);
        assert_eq!(open.step(5., -10.), 0.);
        assert_eq!(open.point_at(400.).0, [0., 0., 100.]);
    }

    #[test]
    fn reversed_line_takes_directions_from_the_previous_point() {
        let line = square(false, true);
        // Point 1 faces along the segment that arrives at it.
        assert_eq!(line.directions[1], [1., 0., 0.]);
        assert_eq!(line.directions[2], [0., 0., 1.]);
        let line = square(false, false);
        assert_eq!(line.directions[1], [0., 0., 1.]);
    }

    #[test]
    fn tracker_follows_the_car_along_the_line() {
        let line = square(true, false);
        let mut t = Tracker::default();
        for (car, segment, distance) in [([30., 0., 5.], 0, 30.), ([100., 0., 60.], 1, 160.), ([-3., 0., 70.], 3, 330.)] {
            t.update(&line, car);
            assert_eq!(t.segment, Some(segment));
            assert!((t.distance - distance).abs() < 1e-3, "{} != {distance}", t.distance);
        }
    }

    #[test]
    fn target_moves_ahead_early_and_stops_at_the_finish() {
        let line = square(true, false);
        // 4 m off the line in the first 15 s: 6 m ahead.
        assert!((target(&line, 30., [30., 0., 4.], 3., None) - 36.).abs() < 1e-3);
        assert!((target(&line, 398., [-4., 0., 2.], 3., None) - 4.).abs() < 1e-3);
        assert_eq!(target(&line, 30., [30., 0., 4.], 15., None), 30.);
        let open = square(false, false);
        assert_eq!(target(&open, 290., [0., 0., 0.], 3., Some(250.)), 250.);
        assert_eq!(target(&open, 200., [0., 0., 0.], 3., Some(250.)), 200.);
    }

    #[test]
    fn job_places_at_half_fade_and_ends_after_the_tail() {
        let mut job = Job::new(12.);
        let mut events = Vec::new();
        let mut off = 0;
        let mut darkest = 0f32;
        for tick in 1..=500 {
            if job.controls_off() {
                off += 1;
            }
            darkest = darkest.max(job.darkness());
            if let Some(e) = job.update(0.016) {
                events.push((tick, e));
            }
        }
        // 0.5 s of 16 ms frames: placed on the 32nd, controls off before it.
        assert_eq!(events[0], (32, JobEvent::Place));
        assert_eq!(off, 32);
        assert!(darkest > 0.95);
        // Fade ends at 1 s (63 frames), then 5 s of tail.
        let tail_over = events.iter().find(|e| e.1 == JobEvent::TailOver).unwrap().0;
        assert!((375..=378).contains(&tail_over), "{tail_over}");
        assert!(events.iter().skip_while(|e| e.1 != JobEvent::TailOver).all(|e| e.1 == JobEvent::TailOver));
        assert_eq!(job.darkness(), 0.);
        assert!(job.ghost() && !job.controls_off());
    }
}
