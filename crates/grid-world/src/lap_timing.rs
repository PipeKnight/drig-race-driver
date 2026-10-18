//! Independent practice-lap policy over supplied ordered source gates.
//! This does not reproduce original race rules, penalties or the original clock.
use crate::Error;

#[derive(Clone, Copy, Debug)]
pub struct Policy {
    pub maximum_step: f64,
    pub height_tolerance: f64,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            maximum_step: 50.,
            height_tolerance: 3.,
        }
    }
}
#[derive(Clone, Debug)]
struct Gate {
    left: [f64; 3],
    edge: [f64; 3],
    center: [f64; 3],
    normal: [f64; 2],
    width_squared: f64,
}
#[derive(Clone, Debug)]
pub struct Course {
    gates: Vec<Gate>,
    sector_gates: Vec<usize>,
    required_gates: Vec<usize>,
    policy: Policy,
    /// A point-to-point run: gate 0 is the start, the last gate the finish.
    open: bool,
}
impl Course {
    /// Gate zero is the finish. Direction follows the ordered circuit, while
    /// actual crossing planes use the source left/right endpoint geometry.
    pub fn new(
        endpoints: &[[[f32; 3]; 2]],
        sector_gates: &[usize],
        policy: Policy,
    ) -> Result<Self, Error> {
        let n = endpoints.len();
        if !(3..=512).contains(&n)
            || sector_gates.len() > 16
            || sector_gates.iter().any(|&i| i == 0 || i >= n)
            || sector_gates.windows(2).any(|s| s[0] >= s[1])
            || endpoints
                .iter()
                .flatten()
                .flatten()
                .any(|v| !v.is_finite() || v.abs() > 100000.)
            || !policy.maximum_step.is_finite()
            || !(1. ..=1000.).contains(&policy.maximum_step)
            || !policy.height_tolerance.is_finite()
            || !(0.1..=100.).contains(&policy.height_tolerance)
        {
            return Err(Error("invalid prototype lap course"));
        }
        let centers: Vec<[f64; 3]> = endpoints
            .iter()
            .map(|e| std::array::from_fn(|i| (f64::from(e[0][i]) + f64::from(e[1][i])) * 0.5))
            .collect();
        let mut gates = Vec::with_capacity(n);
        for (i, e) in endpoints.iter().enumerate() {
            let left = e[0].map(f64::from);
            let edge = std::array::from_fn(|a| f64::from(e[1][a]) - left[a]);
            let square = edge[0] * edge[0] + edge[2] * edge[2];
            if !(0.01..=1000000.).contains(&square) {
                return Err(Error("invalid prototype lap gate width"));
            }
            let width = square.sqrt();
            let mut normal = [edge[2] / width, -edge[0] / width];
            let next = centers[(i + 1) % n];
            let forward =
                normal[0] * (next[0] - centers[i][0]) + normal[1] * (next[2] - centers[i][2]);
            if forward.abs() < 1e-6 {
                return Err(Error("ambiguous prototype lap gate direction"));
            }
            if forward < 0. {
                normal = normal.map(|v| -v);
            }
            gates.push(Gate {
                left,
                edge,
                center: centers[i],
                normal,
                width_squared: square,
            });
        }
        Ok(Self {
            gates,
            sector_gates: sector_gates.to_vec(),
            required_gates: (0..n).collect(),
            policy,
            open: false,
        })
    }
    /// A point-to-point run over `endpoints` in travel order: crossing gate 0
    /// starts the run, crossing the last gate after the required gates ends
    /// it, and nothing wraps. Each gate faces the next one; the finish faces
    /// away from the gate before it. Sector gates lie strictly between.
    pub fn open(
        endpoints: &[[[f32; 3]; 2]],
        sector_gates: &[usize],
        policy: Policy,
    ) -> Result<Self, Error> {
        let n = endpoints.len();
        if !(2..=4096).contains(&n) || sector_gates.iter().any(|&i| i + 1 >= n) {
            return Err(Error("invalid prototype run course"));
        }
        // The circuit constructor orients gate i towards gate i + 1 (mod n);
        // only the finish differs, so build with a mirrored extra gate after
        // it and drop that gate again.
        let last = endpoints[n - 1];
        let before = endpoints[n - 2];
        let beyond = std::array::from_fn(|side| {
            std::array::from_fn(|a| 2. * last[side][a] - before[side][a])
        });
        let mut extended = endpoints.to_vec();
        extended.push(beyond);
        let mut course = Self::new(&extended, sector_gates, policy)?;
        course.gates.pop();
        course.required_gates = (0..n).collect();
        course.open = true;
        Ok(course)
    }
    pub fn is_open(&self) -> bool {
        self.open
    }
    /// Select explicit route checkpoints while retaining the source geometry
    /// and direction. Every timing split remains required in circuit order.
    pub fn with_required(mut self, required: &[usize]) -> Result<Self, Error> {
        if required.len() < 2
            || required.first() != Some(&0)
            || (self.open && required.last() != Some(&(self.gates.len() - 1)))
            || required.iter().any(|&g| g >= self.gates.len())
            || required.windows(2).any(|g| g[0] >= g[1])
            || self.sector_gates.iter().any(|g| !required.contains(g))
        {
            return Err(Error("invalid required prototype lap gates"));
        }
        self.required_gates = required.to_vec();
        Ok(self)
    }
    pub fn required_gates(&self) -> &[usize] {
        &self.required_gates
    }
    pub fn gate_count(&self) -> usize {
        self.gates.len()
    }
    pub fn gate_frame(&self, index: usize) -> Option<([f64; 3], [f64; 2])> {
        self.gates.get(index).map(|g| (g.center, g.normal))
    }
}
impl Gate {
    /// Signed horizontal distance of `p` ahead of the gate plane.
    fn side(&self, p: [f64; 3]) -> f64 {
        self.normal[0] * (p[0] - self.center[0]) + self.normal[1] * (p[2] - self.center[2])
    }
    fn crossing(&self, from: [f64; 3], to: [f64; 3], height: f64) -> Option<(f64, bool)> {
        let side = |p: [f64; 3]| {
            self.normal[0] * (p[0] - self.center[0]) + self.normal[1] * (p[2] - self.center[2])
        };
        let a = side(from);
        let b = side(to);
        let forward = if a < 0. && b >= 0. {
            true
        } else if a > 0. && b <= 0. {
            false
        } else {
            return None;
        };
        let t = -a / (b - a);
        let p: [f64; 3] = std::array::from_fn(|i| from[i] + t * (to[i] - from[i]));
        let along = ((p[0] - self.left[0]) * self.edge[0] + (p[2] - self.left[2]) * self.edge[2])
            / self.width_squared;
        if !(-1e-9..=1. + 1e-9).contains(&along)
            || (p[1] - (self.left[1] + along * self.edge[1])).abs() > height
        {
            return None;
        }
        Some((t, forward))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Completed {
    pub number: u32,
    pub seconds: f64,
    pub sectors: Vec<Option<f64>>,
    pub valid: bool,
    pub missed_gate: Option<usize>,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Started { when: f64 },
    Checkpoint { gate: usize, when: f64 },
    Sector { sector: usize, seconds: f64 },
    Finished(Completed),
    JumpInvalidated,
}
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    previous: [f64; 3],
    time: f64,
    start: Option<f64>,
    visited: Vec<bool>,
    splits: Vec<Option<f64>>,
    reverse_finish: bool,
    /// Open course only: the car started past gate 0 (Haruna's forward
    /// grids stand 3.6 m beyond their start gate), so the run begins on its
    /// first forward movement instead of a crossing (AUTHORED; the
    /// original's standing-start timer is not traced).
    armed: bool,
    pub next_gate: usize,
    pub attempts: u32,
    pub valid_laps: u32,
    pub current_valid: bool,
    pub last_lap: Option<Completed>,
    pub best_lap: Option<f64>,
}
pub struct Output {
    pub state: State,
    pub events: Vec<Event>,
}
impl State {
    pub fn new(course: &Course, position: [f32; 3]) -> Result<Self, Error> {
        if position.iter().any(|v| !v.is_finite() || v.abs() > 100000.) {
            return Err(Error("invalid initial lap position"));
        }
        Ok(Self {
            previous: position.map(f64::from),
            time: 0.,
            start: None,
            visited: vec![false; course.gates.len()],
            splits: vec![None; course.sector_gates.len()],
            reverse_finish: false,
            armed: course.open && course.gates[0].side(position.map(f64::from)) > 0.,
            next_gate: 0,
            attempts: 0,
            valid_laps: 0,
            current_valid: true,
            last_lap: None,
            best_lap: None,
        })
    }
    pub fn elapsed(&self) -> Option<f64> {
        self.start.map(|t| self.time - t)
    }
    pub fn passed_gates(&self) -> usize {
        self.visited.iter().filter(|&&v| v).count()
    }
    pub fn lap_number(&self) -> u32 {
        self.attempts.saturating_add(1)
    }
    fn begin(&mut self, course: &Course, at: f64, events: &mut Vec<Event>) {
        self.start = Some(at);
        self.current_valid = true;
        self.visited.fill(false);
        self.visited[0] = true;
        self.splits.fill(None);
        self.next_gate = course.required_gates[1];
        self.reverse_finish = false;
        events.push(Event::Started { when: at });
    }
    /// Time is supplied from successful simulation ticks. Pause does not call
    /// this function. All state/events are staged and returned on success only.
    pub fn advance(
        &self,
        course: &Course,
        position: [f32; 3],
        end_time: f64,
    ) -> Result<Output, Error> {
        if course.open {
            return self.advance_open(course, position, end_time);
        }
        let n = course.gates.len();
        let duration = end_time - self.time;
        if !end_time.is_finite()
            || duration <= 0.
            || duration > 0.25
            || self.next_gate >= n
            || !course.required_gates.contains(&self.next_gate)
            || self.visited.len() != n
            || self.splits.len() != course.sector_gates.len()
            || position.iter().any(|v| !v.is_finite() || v.abs() > 100000.)
        {
            return Err(Error("invalid prototype lap interval"));
        }
        let to = position.map(f64::from);
        let mut state = self.clone();
        state.previous = to;
        state.time = end_time;
        let mut events = Vec::new();
        let travel = (0..3)
            .map(|i| (to[i] - self.previous[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        if travel > course.policy.maximum_step {
            if state.start.is_some() {
                state.current_valid = false;
                events.push(Event::JumpInvalidated);
            }
            return Ok(Output { state, events });
        }
        let finish = course.gates[0].crossing(self.previous, to, course.policy.height_tolerance);
        let mut processed = Vec::new();
        let mut cursor = 0.;
        for _ in 0..=n {
            let mut crossings = Vec::with_capacity(2);
            if !processed.contains(&0)
                && let Some((t, forward)) = finish
                && t >= cursor
            {
                crossings.push((t, 0, forward));
            }
            if state.start.is_some()
                && state.next_gate != 0
                && !processed.contains(&state.next_gate)
                && let Some((t, true)) = course.gates[state.next_gate].crossing(
                    self.previous,
                    to,
                    course.policy.height_tolerance,
                )
                && t >= cursor
            {
                crossings.push((t, state.next_gate, true));
            }
            crossings.sort_by(|a, b| a.0.total_cmp(&b.0));
            let Some((fraction, gate, forward)) = crossings.first().copied() else {
                break;
            };
            processed.push(gate);
            cursor = fraction;
            let at = self.time + fraction * duration;
            if gate == 0 {
                if !forward {
                    state.reverse_finish = true;
                    continue;
                }
                if state.start.is_none() {
                    state.begin(course, at, &mut events);
                    continue;
                }
                if state.reverse_finish && state.next_gate != 0 {
                    state.reverse_finish = false;
                    continue;
                }
                let seconds = at - state.start.unwrap();
                if !seconds.is_finite() || seconds <= 0. {
                    return Err(Error("invalid completed prototype lap duration"));
                }
                let valid = state.current_valid
                    && state.next_gate == 0
                    && course.required_gates.iter().all(|&g| state.visited[g]);
                state.attempts = state
                    .attempts
                    .checked_add(1)
                    .ok_or(Error("prototype lap counter overflow"))?;
                let mut cumulative = state.splits.clone();
                cumulative.push(Some(seconds));
                let mut previous = Some(0.);
                let sectors = cumulative
                    .into_iter()
                    .map(|value| {
                        let result = previous.zip(value).map(|(a, b)| b - a);
                        previous = value;
                        result
                    })
                    .collect();
                let completed = Completed {
                    number: state.attempts,
                    seconds,
                    sectors,
                    valid,
                    missed_gate: course
                        .required_gates
                        .iter()
                        .copied()
                        .find(|&g| !state.visited[g]),
                };
                if valid {
                    state.valid_laps = state
                        .valid_laps
                        .checked_add(1)
                        .ok_or(Error("valid prototype lap counter overflow"))?;
                    state.best_lap = Some(state.best_lap.map_or(seconds, |best| best.min(seconds)));
                }
                state.last_lap = Some(completed.clone());
                events.push(Event::Finished(completed));
                state.begin(course, at, &mut events);
            } else {
                state.visited[gate] = true;
                let ordinal = course
                    .required_gates
                    .iter()
                    .position(|&g| g == gate)
                    .ok_or(Error("lap gate outside required route"))?;
                state.next_gate =
                    course.required_gates[(ordinal + 1) % course.required_gates.len()];
                events.push(Event::Checkpoint { gate, when: at });
                if let Some(sector) = course.sector_gates.iter().position(|&g| g == gate) {
                    let elapsed = at - state.start.unwrap();
                    let previous = if sector == 0 {
                        0.
                    } else {
                        state.splits[sector - 1].unwrap_or(0.)
                    };
                    state.splits[sector] = Some(elapsed);
                    events.push(Event::Sector {
                        sector,
                        seconds: elapsed - previous,
                    });
                }
            }
        }
        Ok(Output { state, events })
    }

    /// [`Self::advance`] on an open course: the run starts on the first
    /// forward crossing of gate 0 and ends on the forward crossing of the
    /// finish (the last gate) as the next required gate. A finished run does
    /// not restart; crossing gate 0 again begins a new attempt.
    fn advance_open(
        &self,
        course: &Course,
        position: [f32; 3],
        end_time: f64,
    ) -> Result<Output, Error> {
        let n = course.gates.len();
        let finish_gate = n - 1;
        let duration = end_time - self.time;
        if !end_time.is_finite()
            || duration <= 0.
            || duration > 0.25
            || self.next_gate >= n
            || !course.required_gates.contains(&self.next_gate)
            || self.visited.len() != n
            || self.splits.len() != course.sector_gates.len()
            || position.iter().any(|v| !v.is_finite() || v.abs() > 100000.)
        {
            return Err(Error("invalid prototype run interval"));
        }
        let to = position.map(f64::from);
        let mut state = self.clone();
        state.previous = to;
        state.time = end_time;
        let mut events = Vec::new();
        let travel = (0..3)
            .map(|i| (to[i] - self.previous[i]).powi(2))
            .sum::<f64>()
            .sqrt();
        if travel > course.policy.maximum_step {
            if state.start.is_some() {
                state.current_valid = false;
                events.push(Event::JumpInvalidated);
            }
            return Ok(Output { state, events });
        }
        let height = course.policy.height_tolerance;
        let mut cursor = 0.;
        let mut processed = Vec::new();
        if state.armed && state.start.is_none() {
            let gate = &course.gates[0];
            if gate.side(to) - gate.side(self.previous) > 0.01 {
                state.armed = false;
                state.begin(course, self.time, &mut events);
                processed.push(0);
            }
        }
        for _ in 0..=n {
            let gate = if state.start.is_none() { 0 } else { state.next_gate };
            if processed.contains(&gate) {
                break;
            }
            let Some((fraction, true)) = course.gates[gate].crossing(self.previous, to, height)
            else {
                break;
            };
            if fraction < cursor {
                break;
            }
            processed.push(gate);
            cursor = fraction;
            let at = self.time + fraction * duration;
            if state.start.is_none() {
                state.begin(course, at, &mut events);
                continue;
            }
            state.visited[gate] = true;
            if gate == finish_gate {
                let seconds = at - state.start.unwrap();
                if !seconds.is_finite() || seconds <= 0. {
                    return Err(Error("invalid completed prototype run duration"));
                }
                let valid =
                    state.current_valid && course.required_gates.iter().all(|&g| state.visited[g]);
                state.attempts = state
                    .attempts
                    .checked_add(1)
                    .ok_or(Error("prototype lap counter overflow"))?;
                let mut cumulative = state.splits.clone();
                cumulative.push(Some(seconds));
                let mut previous = Some(0.);
                let sectors = cumulative
                    .into_iter()
                    .map(|value| {
                        let result = previous.zip(value).map(|(a, b)| b - a);
                        previous = value;
                        result
                    })
                    .collect();
                let completed = Completed {
                    number: state.attempts,
                    seconds,
                    sectors,
                    valid,
                    missed_gate: course
                        .required_gates
                        .iter()
                        .copied()
                        .find(|&g| !state.visited[g]),
                };
                if valid {
                    state.valid_laps = state
                        .valid_laps
                        .checked_add(1)
                        .ok_or(Error("valid prototype lap counter overflow"))?;
                    state.best_lap = Some(state.best_lap.map_or(seconds, |best| best.min(seconds)));
                }
                state.last_lap = Some(completed.clone());
                events.push(Event::Finished(completed));
                state.start = None;
                state.next_gate = 0;
                state.visited.fill(false);
                break;
            }
            let ordinal = course
                .required_gates
                .iter()
                .position(|&g| g == gate)
                .ok_or(Error("run gate outside required route"))?;
            state.next_gate = course.required_gates[ordinal + 1];
            events.push(Event::Checkpoint { gate, when: at });
            if let Some(sector) = course.sector_gates.iter().position(|&g| g == gate) {
                let elapsed = at - state.start.unwrap();
                let previous = if sector == 0 {
                    0.
                } else {
                    state.splits[sector - 1].unwrap_or(0.)
                };
                state.splits[sector] = Some(elapsed);
                events.push(Event::Sector {
                    sector,
                    seconds: elapsed - previous,
                });
            }
        }
        Ok(Output { state, events })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn course() -> Course {
        Course::new(
            &[
                [[0., 0., -8.], [0., 0., -2.]],
                [[2., 0., 0.], [8., 0., 0.]],
                [[0., 0., 2.], [0., 0., 8.]],
                [[-8., 0., 0.], [-2., 0., 0.]],
            ],
            &[1, 2],
            Policy::default(),
        )
        .unwrap()
    }
    fn step(state: &mut State, course: &Course, p: [f32; 3]) -> Vec<Event> {
        let out = state.advance(course, p, state.time + 0.1).unwrap();
        *state = out.state;
        out.events
    }
    fn finish(state: &mut State, c: &Course) {
        for p in [
            [5., 0., -5.],
            [5., 0., 1.],
            [5., 0., 5.],
            [-1., 0., 5.],
            [-5., 0., 5.],
            [-5., 0., -1.],
            [-5., 0., -5.],
            [1., 0., -5.],
        ] {
            step(state, c, p);
        }
    }
    /// Five gates along +x: start at 0, a sector at 20, finish at 40.
    fn run() -> Course {
        let gate = |x: f32| [[x, 0., -5.], [x, 0., 5.]];
        Course::open(&[gate(0.), gate(10.), gate(20.), gate(30.), gate(40.)], &[2], Policy::default())
            .unwrap()
            .with_required(&[0, 2, 4])
            .unwrap()
    }

    #[test]
    fn an_open_run_starts_at_gate_zero_and_ends_at_the_last_gate() {
        let c = run();
        assert!(c.is_open());
        let mut s = State::new(&c, [-5., 0., 0.]).unwrap();
        let mut events = Vec::new();
        for x in (-4..=50).map(|x| x as f32) {
            events.extend(step(&mut s, &c, [x, 0., 0.]));
        }
        assert_eq!(events.first(), Some(&Event::Started { when: (6. - 1.) * 0.1 }));
        let lap = s.last_lap.clone().unwrap();
        assert!(lap.valid && lap.missed_gate.is_none(), "{lap:?}");
        assert_eq!((s.attempts, s.valid_laps), (1, 1));
        assert!((lap.seconds - 4.).abs() < 1e-9, "{}", lap.seconds);
        assert!((lap.sectors[0].unwrap() - 2.).abs() < 1e-9);
        assert!(events.iter().any(|e| matches!(e, Event::Sector { sector: 0, .. })));
        // Over: driving on past the finish starts nothing, the start is behind.
        assert_eq!(s.elapsed(), None);
        assert_eq!(s.next_gate, 0);
    }

    #[test]
    fn a_car_starting_past_the_start_gate_starts_on_its_first_move() {
        let c = run();
        let mut s = State::new(&c, [3., 0., 0.]).unwrap();
        // Standing still (a held grid) starts nothing.
        assert!(step(&mut s, &c, [3., 0., 0.]).is_empty());
        let events = step(&mut s, &c, [4., 0., 0.]);
        assert_eq!(events, [Event::Started { when: 0.1 }]);
        for x in (5..=45).map(|x| x as f32) {
            step(&mut s, &c, [x, 0., 0.]);
        }
        let lap = s.last_lap.unwrap();
        assert!(lap.valid);
        assert!((lap.seconds - 3.7).abs() < 1e-9, "{}", lap.seconds);
        // A circuit never arms.
        let circuit = course();
        assert!(!State::new(&circuit, [5., 0., -5.]).unwrap().armed);
    }

    #[test]
    fn an_open_run_missing_its_sector_gate_is_not_finished() {
        let c = run();
        let mut s = State::new(&c, [-5., 0., 0.]).unwrap();
        // Across the start, round the sector gate's end (z = 8), back on line.
        for p in [[1., 0., 0.], [10., 0., 8.], [25., 0., 8.], [35., 0., 0.], [45., 0., 0.]] {
            step(&mut s, &c, p);
        }
        // The finish is not the next required gate, so the run goes on.
        assert_eq!(s.attempts, 0);
        assert_eq!(s.next_gate, 2);
    }

    #[test]
    fn ordered_circuit_times_fractional_crossings_and_sectors() {
        let c = course();
        let mut s = State::new(&c, [-1., 0., -5.]).unwrap();
        step(&mut s, &c, [1., 0., -5.]);
        assert_eq!(s.start, Some(0.05));
        finish(&mut s, &c);
        assert_eq!((s.attempts, s.valid_laps), (1, 1));
        let lap = s.last_lap.as_ref().unwrap();
        assert!(lap.valid && lap.missed_gate.is_none());
        assert!(lap.sectors.iter().all(Option::is_some));
        assert!((lap.sectors.iter().flatten().sum::<f64>() - lap.seconds).abs() < 1e-12);
        assert_eq!(s.best_lap, Some(lap.seconds));
        assert!(s.elapsed().unwrap() > 0.);
    }
    #[test]
    fn endpoint_and_reverse_finish_do_not_duplicate_or_erase_progress() {
        let c = course();
        let mut s = State::new(&c, [-1., 0., -5.]).unwrap();
        step(&mut s, &c, [0., 0., -5.]);
        step(&mut s, &c, [1., 0., -5.]);
        assert_eq!((s.attempts, s.next_gate), (0, 1));
        step(&mut s, &c, [-1., 0., -5.]);
        step(&mut s, &c, [1., 0., -5.]);
        assert_eq!((s.attempts, s.next_gate), (0, 1));
        finish(&mut s, &c);
        assert_eq!(s.valid_laps, 1);
    }
    #[test]
    fn missed_width_height_and_position_jump_cannot_earn_valid_lap() {
        let c = course();
        for bad in [[5., 10., 1.], [20., 0., 1.], [-1., 0., 5.]] {
            let mut s = State::new(&c, [-1., 0., -5.]).unwrap();
            step(&mut s, &c, [1., 0., -5.]);
            step(&mut s, &c, bad);
            for p in [[-5., 0., 5.], [-5., 0., -5.], [1., 0., -5.]] {
                step(&mut s, &c, p);
            }
            assert_eq!(s.valid_laps, 0);
            assert!(s.best_lap.is_none());
            assert!(!s.last_lap.as_ref().unwrap().valid);
            assert_eq!(s.last_lap.as_ref().unwrap().missed_gate, Some(1));
        }
        let mut s = State::new(&c, [-1., 0., -5.]).unwrap();
        step(&mut s, &c, [1., 0., -5.]);
        let events = step(&mut s, &c, [100., 0., 100.]);
        assert!(!s.current_valid && events.contains(&Event::JumpInvalidated));
    }
    #[test]
    fn invalid_clock_and_late_counter_failure_preserve_complete_input() {
        let c = course();
        let mut s = State::new(&c, [-1., 0., -5.]).unwrap();
        step(&mut s, &c, [1., 0., -5.]);
        let before = s.clone();
        assert!(s.advance(&c, [5., 0., 1.], f64::NAN).is_err());
        assert!(s.advance(&c, [5., 0., 1.], s.time).is_err());
        assert_eq!(s, before);
        for p in [
            [5., 0., -5.],
            [5., 0., 1.],
            [5., 0., 5.],
            [-1., 0., 5.],
            [-5., 0., 5.],
            [-5., 0., -1.],
            [-5., 0., -5.],
        ] {
            step(&mut s, &c, p);
        }
        s.attempts = u32::MAX;
        let before = s.clone();
        assert!(s.advance(&c, [1., 0., -5.], s.time + 0.1).is_err());
        assert_eq!(s, before);
    }
    #[test]
    fn explicit_route_splits_keep_optional_progress_separate() {
        let original = course();
        let endpoints: Vec<_> = original
            .gates
            .iter()
            .map(|g| {
                [
                    g.left.map(|v| v as f32),
                    std::array::from_fn(|i| (g.left[i] + g.edge[i]) as f32),
                ]
            })
            .collect();
        let base = Course::new(&endpoints, &[2], Policy::default()).unwrap();
        assert!(base.clone().with_required(&[0, 1]).is_err());
        assert!(base.clone().with_required(&[0, 2, 2]).is_err());
        let c = base.with_required(&[0, 2]).unwrap();
        let mut s = State::new(&c, [-1., 0., -5.]).unwrap();
        step(&mut s, &c, [1., 0., -5.]);
        for p in [
            [5., 10., 1.],
            [5., 0., 5.],
            [-1., 0., 5.],
            [-5., 0., 5.],
            [-5., 0., -5.],
            [1., 0., -5.],
        ] {
            step(&mut s, &c, p);
        }
        assert_eq!(s.valid_laps, 1);
        assert_eq!(s.last_lap.as_ref().unwrap().sectors.len(), 2);
    }
}
