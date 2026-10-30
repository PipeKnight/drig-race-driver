//! Original GRID AI race driver, ported from the recovered pipeline in
//! docs/AI_DRIVER_RESEARCH.md: racing line from `ai_track.xml`, look-ahead
//! (0x779A20), brake-line corner speed (0x792C80), brake-line braking against the
//! car's speed->distance table (0x7886C0), throttle governor/limiters (0x783180,
//! 0x77A4C0, 0x77A880, 0x77A940), steering (0x77AC50), output slew (0x779F70)
//! and race-behaviour random offsets (0x78A4D0/0x78A610), with the shipped
//! config and the bilinear difficulty mappings.
//!
//! The lateral PID (0x7892E0) shifts the target point by PID(lateral error)
//! with the shipped gains and limits; its internal clamping is authored (the
//! PID vfunc is not decompiled) and its speed-dependent prediction and corner
//! factor terms are not yet recovered.
//!
//! Traffic (docs/AI_OVERTAKING_RESEARCH.md): the brake_for_vehicle,
//! attempt_overtake and alongside tests feed a per-tick request; behaviours
//! arbitrate by priority (alongside 8 > overtake 5 > race 1); the follow
//! throttle (0x77A630) and brake (0x77A1E0) consume the request's follow
//! fields. AUTHORED where not recovered: the path-obstacle scan is a straight
//! corridor along the own heading, the alongside yield and far-side squeeze
//! are omitted and cars on both sides hold the planned offset.
//!
//! The race manager (`crate::ai_pacing`) sets the corner-speed scale A and
//! floor B, the let-pass flag, failures and engine damage, and arms
//! choreographed mistakes; the mistake slot, lost_control, controlled_stop,
//! race_damaged and the retire failure branch are here
//! (docs/AI_MISTAKES_PACING_RESEARCH.md).
//!
//! Selected scope, not recovered or not yet ported: block/draft (data-gated
//! off), visibility_loss (no smoke value), start-line reaction. The random offset generator is an authored LCG (the original RNG
//! stream is not identified). Characteristic values default to 1.0 (the live
//! player-slot set); the difficulty default is the live Normal value 0.606.
use crate::source_vehicle::Controls;
use grid_formats::{ai, binxml};
use std::{collections::BTreeMap, path::Path};

/// Live Normal difficulty observed in the original (all 12 driver objects).
pub const NORMAL_DIFFICULTY: f32 = 0.606;

/// Per-driver characteristic values (aggression, control, mistakes, reaction,
/// variance, speed_in, speed_out, virtual_performance) from a private TSV
/// (header row of names, then one row per driver: index then values), as read
/// from the original's driver objects. Row k drives car k.
pub fn read_characteristics(
    text: &str,
) -> Result<Vec<BTreeMap<String, f32>>, Box<dyn std::error::Error>> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let header: Vec<&str> = lines
        .next()
        .ok_or("empty characteristic table")?
        .split('\t')
        .collect();
    if header.first() != Some(&"driver") || header.len() < 2 {
        return Err("invalid characteristic table header".into());
    }
    let mut rows = Vec::new();
    for line in lines {
        let cells: Vec<&str> = line.split('\t').collect();
        if cells.len() != header.len() {
            return Err("characteristic row width differs from header".into());
        }
        let mut row = BTreeMap::new();
        for (name, value) in header[1..].iter().zip(&cells[1..]) {
            let v: f32 = value.parse()?;
            if !(0. ..=1.).contains(&v) {
                return Err("characteristic value outside 0..1".into());
            }
            row.insert(name.to_string(), v);
        }
        rows.push(row);
    }
    if rows.is_empty() {
        return Err("characteristic table has no drivers".into());
    }
    Ok(rows)
}

fn read_doc(path: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(std::fs::read(path)?)
}

/// Racing-line polyline in AI link order: closed on circuits, open on
/// point-to-point roads (Haruna: n gates, n - 1 links).
#[derive(Clone)]
pub struct Line {
    /// No segment joins the last point to the first. Arc lengths outside
    /// 0..length continue straight along the end segments (AUTHORED: the
    /// look-ahead past a road's end; the original's end handling is not
    /// traced).
    open: bool,
    points: Vec<[f32; 3]>,
    lateral: Vec<[f32; 3]>,
    /// Racing-limit offsets relative to the racing line (left <= 0 <= right).
    limits: Vec<[f32; 2]>,
    /// Track-limit offsets (ai_track types 3/4) relative to the racing line.
    track_limits: Vec<[f32; 2]>,
    start: Vec<f32>,
    pub length: f32,
    /// Line index of each AI gate.
    gate_index: Vec<usize>,
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot2(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[2] * b[2]
}
fn lerp3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t)
}
fn heading(d: [f32; 3]) -> f32 {
    d[0].atan2(d[2])
}
fn wrap(a: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (a + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
}

impl Line {
    fn new(track: &ai::Track) -> Result<Self, Box<dyn std::error::Error>> {
        let n = track.gates.len();
        let open = n >= 2 && track.links.len() == n - 1;
        if track.links.len() != n && !open {
            return Err("AI links do not form one loop or one chain".into());
        }
        let mut order = Vec::with_capacity(n);
        if open {
            // The chain's head: the one gate no link leads to.
            let mut heads = (0..n).filter(|g| !track.links.iter().any(|l| l.1 == *g));
            let (Some(mut gate), None) = (heads.next(), heads.next()) else {
                return Err("AI link chain has no single head".into());
            };
            for i in 0..n {
                if order.contains(&gate) {
                    return Err("AI links revisit a gate".into());
                }
                order.push(gate);
                if i + 1 < n {
                    gate = track
                        .links
                        .iter()
                        .find(|l| l.0 == gate)
                        .ok_or("AI link chain broken")?
                        .1;
                }
            }
        } else {
            let mut gate = track.links[0].1;
            for _ in 0..n {
                if order.contains(&gate) {
                    return Err("AI links revisit a gate".into());
                }
                order.push(gate);
                gate = track
                    .links
                    .iter()
                    .find(|l| l.0 == gate)
                    .ok_or("AI link chain broken")?
                    .1;
            }
            if gate != order[0] {
                return Err("AI links do not close".into());
            }
        }
        let mut line = Line {
            open,
            points: Vec::with_capacity(n),
            lateral: Vec::with_capacity(n),
            limits: Vec::with_capacity(n),
            track_limits: Vec::with_capacity(n),
            start: Vec::with_capacity(n),
            length: 0.,
            gate_index: vec![0; n],
        };
        for (i, &g) in order.iter().enumerate() {
            let g_data = &track.gates[g];
            line.gate_index[g] = i;
            line.points.push(std::array::from_fn(|k| {
                g_data.position[k] + g_data.normal[k] * g_data.racing_line
            }));
            line.lateral.push(g_data.normal);
            line.limits.push([
                g_data.left_racing_limit - g_data.racing_line,
                g_data.right_racing_limit - g_data.racing_line,
            ]);
            line.track_limits.push([
                g_data.left_track_limit - g_data.racing_line,
                g_data.right_track_limit - g_data.racing_line,
            ]);
        }
        for i in 0..n {
            line.start.push(line.length);
            if open && i + 1 == n {
                break;
            }
            let d = sub(line.points[(i + 1) % n], line.points[i]);
            line.length += dot2(d, d).sqrt();
        }
        Ok(line)
    }

    /// An open line's segment and position `t` at arc length `s` (`t` runs
    /// outside 0..1 before the first and past the last point).
    fn open_place(&self, s: f32) -> (usize, usize, f32) {
        let n = self.points.len();
        let i = self
            .start
            .partition_point(|&x| x <= s)
            .saturating_sub(1)
            .min(n - 2);
        (i, i + 1, (s - self.start[i]) / self.segment(i).max(1e-6))
    }

    /// Whether the line is an open road (point-to-point) rather than a loop.
    pub fn is_open(&self) -> bool {
        self.open
    }

    fn segment(&self, i: usize) -> f32 {
        let next = if i + 1 < self.points.len() {
            self.start[i + 1]
        } else {
            self.length
        };
        next - self.start[i]
    }

    /// Point, unit direction, lateral direction and racing limits at arc length `s`.
    fn at(&self, s: f32) -> ([f32; 3], [f32; 3], [f32; 3], [f32; 2]) {
        if self.open {
            let (i, j, t) = self.open_place(s);
            let d = sub(self.points[j], self.points[i]);
            let len = dot2(d, d).sqrt().max(1e-6);
            let u = t.clamp(0., 1.);
            let lateral = lerp3(self.lateral[i], self.lateral[j], u);
            let ll = dot2(lateral, lateral).sqrt().max(1e-6);
            return (
                lerp3(self.points[i], self.points[j], t),
                [d[0] / len, 0., d[2] / len],
                [lateral[0] / ll, 0., lateral[2] / ll],
                [
                    self.limits[i][0] + (self.limits[j][0] - self.limits[i][0]) * u,
                    self.limits[i][1] + (self.limits[j][1] - self.limits[i][1]) * u,
                ],
            );
        }
        let s = s.rem_euclid(self.length);
        let i = self.start.partition_point(|&x| x <= s).saturating_sub(1);
        let j = (i + 1) % self.points.len();
        let t = ((s - self.start[i]) / self.segment(i).max(1e-6)).clamp(0., 1.);
        let d = sub(self.points[j], self.points[i]);
        let len = dot2(d, d).sqrt().max(1e-6);
        let lateral = lerp3(self.lateral[i], self.lateral[j], t);
        let ll = dot2(lateral, lateral).sqrt().max(1e-6);
        (
            lerp3(self.points[i], self.points[j], t),
            [d[0] / len, 0., d[2] / len],
            [lateral[0] / ll, 0., lateral[2] / ll],
            [
                self.limits[i][0] + (self.limits[j][0] - self.limits[i][0]) * t,
                self.limits[i][1] + (self.limits[j][1] - self.limits[i][1]) * t,
            ],
        )
    }

    /// Racing-line point and lateral at arc length `s` as the track view
    /// evaluates them (line vfunc +0x14 = 0x791DE0, from 0x779C40): a
    /// Catmull-Rom segment (0x7786B0, basis 0xE7A230 set by 0xC79D60) through
    /// the link's gates G1, G2 and outer control points G0', G3'. The outer
    /// gates G0, G3 (the link's start or end where none exists, 0x7804B0)
    /// are moved by 0x780520 to G2 + 2·proj(G1 − G2 on G0 − G2) and
    /// G1 + 2·proj(G2 − G1 on G3 − G1). The lateral is the curve's tangent
    /// (0x7787A0) turned to (−t_z, 0, t_x), normalised. Past an open line's
    /// ends this keeps the straight continuation of [`Self::at`] (AUTHORED).
    fn curve(&self, s: f32) -> ([f32; 3], [f32; 3]) {
        let n = self.points.len();
        let (i, j, t) = if self.open {
            self.open_place(s)
        } else {
            let s = s.rem_euclid(self.length);
            let i = self.start.partition_point(|&x| x <= s).saturating_sub(1);
            (i, (i + 1) % n, ((s - self.start[i]) / self.segment(i).max(1e-6)).clamp(0., 1.))
        };
        if n < 3 || !(0. ..=1.).contains(&t) {
            let (point, _, lateral, _) = self.at(s);
            return (point, lateral);
        }
        let (h, k) = if self.open {
            (i.saturating_sub(1), (j + 1).min(n - 1))
        } else {
            ((i + n - 1) % n, (j + 1) % n)
        };
        // 0x780520: p3 + 2·((p1 − p3)·(p2 − p3))/|p1 − p3|² · (p1 − p3).
        let reflect = |p1: [f32; 3], p2: [f32; 3], p3: [f32; 3]| -> [f32; 3] {
            let v = sub(p1, p3);
            let square = dot3(v, v);
            if square <= 0. {
                return p3;
            }
            let scale = 2. * dot3(v, sub(p2, p3)) / square;
            std::array::from_fn(|c| p3[c] + scale * v[c])
        };
        let (g1, g2) = (self.points[i], self.points[j]);
        let g0 = reflect(self.points[h], g1, g2);
        let g3 = reflect(self.points[k], g2, g1);
        // 0.5·[t³ t² t 1] × [[−1 3 −3 1] [2 −5 4 −1] [−1 0 1 0] [0 2 0 0]].
        let (t2, t3) = (t * t, t * t * t);
        let w = [
            0.5 * (-t3 + 2. * t2 - t),
            0.5 * (3. * t3 - 5. * t2 + 2.),
            0.5 * (-3. * t3 + 4. * t2 + t),
            0.5 * (t3 - t2),
        ];
        let dw = [
            -1.5 * t2 + 2. * t - 0.5,
            4.5 * t2 - 5. * t,
            -4.5 * t2 + 4. * t + 0.5,
            1.5 * t2 - t,
        ];
        let g = [g0, g1, g2, g3];
        let point = std::array::from_fn(|c| (0..4).map(|m| w[m] * g[m][c]).sum());
        let tangent: [f32; 3] = std::array::from_fn(|c| (0..4).map(|m| dw[m] * g[m][c]).sum());
        let length = (tangent[0] * tangent[0] + tangent[2] * tangent[2]).sqrt();
        if length <= 0. {
            let (_, _, lateral, _) = self.at(s);
            return (point, lateral);
        }
        (point, [-tangent[2] / length, 0., tangent[0] / length])
    }

    /// Racing-limit point (waypoint type 1 for `k` 0, type 2 for `k` 1) at gate `i`.
    fn racing_limit_point(&self, i: usize, k: usize) -> [f32; 3] {
        std::array::from_fn(|c| self.points[i][c] + self.lateral[i][c] * self.limits[i][k])
    }

    /// Link `i` of the track view's 32-link window that starts one link
    /// behind `segment`, and the next gate (`None` past an open road's end).
    fn window_link(&self, segment: usize, k: usize) -> Option<(usize, usize)> {
        let n = self.points.len();
        if self.open {
            let i = (segment + k).checked_sub(1)?;
            (i + 1 < n).then_some((i, i + 1))
        } else {
            let i = (segment + n - 1 + k) % n;
            Some((i, (i + 1) % n))
        }
    }

    /// The avoid behaviour's lookup of a point `p` on the track view's link
    /// window (0x798C00 / 0x78DCD0, from 0x799B30 and 0x799DE0): `(arc,
    /// lateral)` on the chosen link, or `None` when no window link holds `p`
    /// (the original's lateral is then 0).
    ///
    /// A link holds `p` when `p` lies between the gate lines of its two gates
    /// (0x778D00, side test 0x778B40). With `figure_of_eight` the first such
    /// link from the back of the window is taken (0x78DCD0); otherwise the
    /// one nearest to `p` (0x798920: 0 inside its track-limit quad, else the
    /// distance to the nearer limit segment; ties go to the later link). The
    /// lateral is `p`'s offset from the racing-line point at the first gate
    /// along the link's own perpendicular (0x798D00 / 0x796CD0), positive on
    /// the gate normal's side.
    fn locate_in_window(
        &self,
        segment: usize,
        p: [f32; 3],
        figure_of_eight: bool,
    ) -> Option<(f32, f32)> {
        const WINDOW: usize = 32;
        let side = |a: [f32; 3], dir: [f32; 3], q: [f32; 3]| {
            let d = sub(q, a);
            dir[0] * d[2] - dir[2] * d[0]
        };
        let holds = |i: usize, j: usize| {
            let (a, b) = (self.points[i], self.points[j]);
            let (na, nb) = (self.lateral[i], self.lateral[j]);
            side(a, na, p) * side(a, na, b) >= 0. && side(b, nb, p) * side(b, nb, a) >= 0.
        };
        let segment_distance = |a: [f32; 3], b: [f32; 3]| {
            let d = sub(b, a);
            let t = (dot2(sub(p, a), d) / dot2(d, d).max(1e-9)).clamp(0., 1.);
            let e = sub(p, lerp3(a, b, t));
            dot2(e, e).sqrt()
        };
        let limit_point = |i: usize, k: usize| -> [f32; 3] {
            std::array::from_fn(|c| self.points[i][c] + self.lateral[i][c] * self.track_limits[i][k])
        };
        let mut best: Option<(f32, usize, usize)> = None;
        for k in 0..WINDOW.min(self.points.len()) {
            let Some((i, j)) = self.window_link(segment, k) else {
                continue;
            };
            if !holds(i, j) {
                continue;
            }
            if figure_of_eight {
                best = Some((0., i, j));
                break;
            }
            let (l0, l1, r0, r1) = (limit_point(i, 0), limit_point(j, 0), limit_point(i, 1), limit_point(j, 1));
            // Inside the quad between the left and right limit lines: 0.
            let inside = side(l0, sub(l1, l0), p) * side(l0, sub(l1, l0), r0) >= 0.
                && side(r0, sub(r1, r0), p) * side(r0, sub(r1, r0), l0) >= 0.;
            let distance = if inside {
                0.
            } else {
                segment_distance(l0, l1).min(segment_distance(r0, r1))
            };
            if best.is_none_or(|b| distance <= b.0) {
                best = Some((distance, i, j));
            }
        }
        let (_, i, j) = best?;
        let (a, b) = (self.points[i], self.points[j]);
        let along = horizontal(sub(b, a));
        let mut perpendicular = [along[2], 0., -along[0]];
        if dot2(perpendicular, self.lateral[i]) < 0. {
            perpendicular = [-perpendicular[0], 0., -perpendicular[2]];
        }
        let length = dot2(sub(b, a), sub(b, a)).sqrt();
        let t = (dot2(sub(p, a), along) / length.max(1e-6)).clamp(0., 1.);
        Some((
            self.start[i] + t * self.segment(i),
            dot2(sub(p, a), perpendicular),
        ))
    }

    /// Track-limit offsets (left <= 0 <= right) at arc length `s`.
    fn track_limits_at(&self, s: f32) -> [f32; 2] {
        if self.open {
            let (i, j, t) = self.open_place(s);
            let t = t.clamp(0., 1.);
            let [a, b] = [self.track_limits[i], self.track_limits[j]];
            return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        }
        let s = s.rem_euclid(self.length);
        let i = self.start.partition_point(|&x| x <= s).saturating_sub(1);
        let j = (i + 1) % self.points.len();
        let t = ((s - self.start[i]) / self.segment(i).max(1e-6)).clamp(0., 1.);
        let [a, b] = [self.track_limits[i], self.track_limits[j]];
        [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
    }

    /// The racing-line points in link order (the HUD's minimap outline).
    pub fn points(&self) -> &[[f32; 3]] {
        &self.points
    }

    /// Arc length of the racing-line point nearest to `p` (global search).
    pub fn arc(&self, p: [f32; 3]) -> f32 {
        self.project(p, None).1
    }

    /// Research view of `p` (global search): its arc, signed lateral from
    /// the line, and the racing and track limits there.
    pub fn limits_near(&self, p: [f32; 3]) -> (f32, f32, [f32; 2], [f32; 2]) {
        let (_, arc, lateral) = self.project(p, None);
        (arc, lateral, self.at(arc).3, self.track_limits_at(arc))
    }

    /// Signed distance along the line from arc `from` to arc `to` (0x909340):
    /// clamped to the length on an open line, wrapped into half a lap
    /// either way on a closed one.
    #[allow(clippy::neg_cmp_op_on_partial_ord)] // The original's NaN outcomes.
    fn along(&self, from: f32, to: f32) -> f32 {
        let mut d = to - from;
        let length = self.length;
        if !(length > 0.) || !d.is_finite() {
            return d;
        }
        if self.open {
            return d.min(length).max(-length);
        }
        while length * 0.5 < d {
            d -= length;
        }
        while d < -length * 0.5 {
            d += length;
        }
        d
    }

    /// The racing line alone, from the route's `ai_track.xml`.
    pub fn load(root: &Path, route: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = read_doc(&root.join(route).join("ai_track.xml"))?;
        Self::new(&ai::track(&binxml::read(&bytes)?)?)
    }

    /// [`Self::load`] on the route's subroute AI track.
    pub fn load_route(
        root: &Path,
        route: &crate::route::Route,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let bytes = read_doc(&root.join(route.path()).join("ai_track.xml"))?;
        Self::new(&ai::track_named(
            &binxml::read(&bytes)?,
            route.ai_track(),
            "optimal",
        )?)
    }

    /// The driver test of type 11 (`on_track`, 0x79B250) for `wheels` (world
    /// positions): more than half of them inside the track limits at their
    /// projection. [I] Projects without a segment hint, as for a car whose
    /// driver has none (the player's, which the speech collector reads).
    pub fn on_track(&self, wheels: &[[f32; 3]]) -> bool {
        let inside = wheels
            .iter()
            .filter(|&&w| {
                let (_, s, lat) = self.project(w, None);
                let [left_limit, right_limit] = self.track_limits_at(s);
                left_limit <= lat && lat <= right_limit
            })
            .count();
        2 * inside > wheels.len()
    }

    /// Player reset target for a car at `p`: the line point at the car's
    /// projection and the line's horizontal direction there.
    ///
    /// [I] The original takes the player's track-tracker point (0x5EC0E0:
    /// tracker +0x74/+0x78, possibly replaced by a saved point behind it or
    /// moved by 0x90C0F0) on the tracker's own line. Neither the saved point,
    /// the offset nor the tracker's line is ported; this uses the AI racing
    /// line, as the race progress does.
    pub fn reset_target(&self, p: [f32; 3]) -> ([f32; 3], [f32; 3]) {
        let (point, direction, _, _) = self.at(self.arc(p));
        (point, direction)
    }

    /// Projection of `p` near segment `hint` (or globally): segment, arc
    /// length and signed lateral distance (positive toward the right limit).
    fn project(&self, p: [f32; 3], hint: Option<usize>) -> (usize, f32, f32) {
        let n = self.points.len();
        let candidates: Vec<usize> = match (hint, self.open) {
            (Some(h), false) => (0..8).map(|k| (h + n - 2 + k) % n).collect(),
            (None, false) => (0..n).collect(),
            // No segment leaves an open line's last point.
            (Some(h), true) => (h.saturating_sub(2)..(h + 6).min(n - 1)).collect(),
            (None, true) => (0..n - 1).collect(),
        };
        let mut best = (f32::INFINITY, 0, 0., 0.);
        for i in candidates {
            let a = self.points[i];
            let d = sub(self.points[(i + 1) % n], a);
            let square = dot2(d, d).max(1e-9);
            let t = (dot2(sub(p, a), d) / square).clamp(0., 1.);
            let q = lerp3(a, self.points[(i + 1) % n], t);
            let e = sub(p, q);
            let error = dot2(e, e);
            if error < best.0 {
                best = (error, i, self.start[i] + t * square.sqrt(), 0.);
            }
        }
        let (_, i, s, _) = best;
        let (point, _, lateral, _) = self.at(s);
        (i, s, dot2(sub(p, point), lateral))
    }
}

/// Speed->distance lookup over grip tables (0x7A64C0): linear in speed within
/// each table, then linear between the bracketing grips (grip clamped).
fn table_distance(table: &ai::GripTable, speed: f32) -> f32 {
    let p = &table.points;
    let speed = speed.max(0.);
    let mut previous = (0f32, 0f32);
    for &point in p {
        if speed <= point.0 {
            let t = (speed - previous.0) / (point.0 - previous.0).max(1e-6);
            return previous.1 + (point.1 - previous.1) * t;
        }
        previous = point;
    }
    // Above the table: continue the last segment.
    let n = p.len();
    if n < 2 {
        return p[n - 1].1;
    }
    let (a, b) = (p[n - 2], p[n - 1]);
    b.1 + (speed - b.0) * (b.1 - a.1) / (b.0 - a.0)
}
fn grip_distance(tables: &[ai::GripTable], grip: f32, speed: f32) -> f32 {
    let i = tables.partition_point(|t| t.grip < grip);
    if i == 0 {
        return table_distance(&tables[0], speed);
    }
    if i >= tables.len() {
        return table_distance(&tables[tables.len() - 1], speed);
    }
    let (a, b) = (&tables[i - 1], &tables[i]);
    let t = (grip - a.grip) / (b.grip - a.grip);
    let da = table_distance(a, speed);
    da + (table_distance(b, speed) - da) * t
}

/// Shipped AI data for one track, vehicle class and car.
#[derive(Clone)]
pub struct AiData {
    pub line: Line,
    /// `(arc length of the brake-line gate, class brake data)` per brake-line id.
    brake_lines: Vec<(f32, ai::BrakeData)>,
    brake_tables: Vec<ai::GripTable>,
    /// The car's cruise (coast-down) tables: the brake-line ratio's distance
    /// (0x7886C0's second 0x7A6650 call, on vehicle AI data +8).
    cruise_tables: Vec<ai::GripTable>,
    driver: BTreeMap<String, f32>,
    vehicle: BTreeMap<String, f32>,
    mappings: BTreeMap<String, ai::Mapping>,
    /// `behaviour_*` and `test_*` switches of the driver config (behaviour
    /// flags cfg+0xCB4+type, test flags cfg+0xCC4+type).
    switches: BTreeMap<String, f32>,
    /// The same data on the route's `drift` racing line, when the event's
    /// config enables the drift behaviour (line choice 0x794870 reads
    /// cfg+0xCC3 = behaviour_drift).
    drift: Option<Box<AiData>>,
    /// The same data on the route's `start` racing line, followed before the
    /// start line (0x794870 kind 7), when the route ships one that differs
    /// from `optimal` (Shibuya; most routes ship `optimal` only).
    start: Option<Box<AiData>>,
}

/// The racing line a car follows (line choice 0x794870).
#[derive(Clone, Copy, Debug, PartialEq)]
enum RacingLine {
    Optimal,
    Start,
    Drift,
}

impl AiData {
    /// The data on `kind`, or the optimal line when the route lacks it (a
    /// link without the kind falls back to `optimal`, 0x793720(..., 1)).
    fn on(&self, kind: RacingLine) -> &AiData {
        match kind {
            RacingLine::Optimal => None,
            RacingLine::Start => self.start.as_deref(),
            RacingLine::Drift => self.drift.as_deref(),
        }
        .unwrap_or(self)
    }
}

/// `(arc length of the brake-line gate, class brake data)` per brake-line id.
type BrakeLines = Vec<(f32, ai::BrakeData)>;

/// Racing line and brake lines of `track` (brake-line arcs on that line).
fn track_lines(
    track: &ai::Track,
    class_lines: &Option<Vec<ai::BrakeData>>,
) -> Result<(Line, BrakeLines), Box<dyn std::error::Error>> {
    let line = Line::new(track)?;
    let mut brake_lines = Vec::new();
    for (id, b) in track.brake_lines.iter().enumerate() {
        let data = match class_lines {
            Some(lines) => *lines.get(id).ok_or("class brake line absent")?,
            None => b.data,
        };
        brake_lines.push((line.start[line.gate_index[b.gate]], data));
    }
    Ok((line, brake_lines))
}

impl AiData {
    /// `route` is the track route folder, e.g. `tracks/europe/donington/route_0`.
    pub fn load(
        root: &Path,
        route: &str,
        class: &str,
        car: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::load_for(root, route, class, car, None)
    }

    /// [`Self::load`] for an event of the competition format `format` (e.g.
    /// `drift_battle`): its `ai_driver_config.xml` section overrides the
    /// `default` values and switches, and its parameter-mapping section the
    /// `default` mappings [I: the section names are the format names; a
    /// single race has an empty `single_race` section and showed the
    /// `default` values live, F290]. With the drift behaviour switched on the
    /// data is also built on the route's `drift` racing line.
    pub fn load_for(
        root: &Path,
        route: &str,
        class: &str,
        car: &str,
        format: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::load_classes(root, route, (class, class), car, format)
    }

    /// [`Self::load_for`] with the car's own `database.bin` `vehicle` row:
    /// `ai_vehicle_type_config` selects the `ai_vehicle_config.xml` class,
    /// `ai_vehicle_type_track` the route's class brake lines and
    /// `ai_vehicle_type_statistics` the brake tables [D]. Lacetti and BMW are
    /// `touring`/`touring` with their own stems, as the fixed class before.
    pub fn load_car(
        root: &Path,
        route: &str,
        car: &crate::car::Car,
        format: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let v = car.vehicle(root)?;
        Self::load_classes(
            root,
            route,
            (&v.ai_config, &v.ai_track),
            &v.ai_statistics,
            format,
        )
    }

    /// [`Self::load_car`] on the route's subroute: its AI track (`default`,
    /// or `reverse` for Haruna's second direction, [`Route::ai_track`]).
    pub fn load_route(
        root: &Path,
        route: &crate::route::Route,
        car: &crate::car::Car,
        format: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let v = car.vehicle(root)?;
        Self::load_named(
            root,
            route.path(),
            route.ai_track(),
            (&v.ai_config, &v.ai_track),
            &v.ai_statistics,
            format,
        )
    }

    /// `classes` = (`ai_vehicle_config.xml` class, `ai_vehicle_track.xml` class).
    pub fn load_classes(
        root: &Path,
        route: &str,
        classes: (&str, &str),
        car: &str,
        format: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Self::load_named(root, route, "default", classes, car, format)
    }

    /// [`Self::load_classes`] on the AI track `ai_track` of the route's files.
    pub fn load_named(
        root: &Path,
        route: &str,
        ai_track: &str,
        classes: (&str, &str),
        car: &str,
        format: Option<&str>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (class, track_class) = classes;
        let bytes = read_doc(&root.join(route).join("ai_track.xml"))?;
        let track_doc = binxml::read(&bytes)?;
        let track = ai::track_named(&track_doc, ai_track, "optimal")?;
        let bytes = read_doc(&root.join(route).join("ai_vehicle_track.xml"))?;
        let vehicle_doc = binxml::read(&bytes)?;
        let mut class_lines = ai::class_brake_lines_on(&vehicle_doc, ai_track, track_class)?;
        // [V] 0x792C80 falls back to the brake line's own data when the class
        // is absent. Haruna lists only streetgt, promuscle, muscle and clubgt,
        // and its own data has lines with all speeds 0, where a touring car
        // would brake to a stop. AUTHORED: then the route's first class.
        if class_lines.is_none()
            && track.brake_lines.iter().any(|b| b.data.max_speed <= 0.)
            && let Some(first) = ai::class_names_on(&vehicle_doc, ai_track)?.first()
        {
            class_lines = ai::class_brake_lines_on(&vehicle_doc, ai_track, first)?;
        }
        let (line, brake_lines) = track_lines(&track, &class_lines)?;
        let bytes = read_doc(&root.join("ai/ai_vehicle_statistics.xml"))?;
        let statistics = ai::statistics(&binxml::read(&bytes)?, car)?;
        let (brake_tables, cruise_tables) = (statistics.brake, statistics.cruise);
        let bytes = read_doc(&root.join("ai/ai_driver_config.xml"))?;
        let config = binxml::read(&bytes)?;
        let mut driver = ai::config(&config, "ai_driver_config", &[("default", None)])?;
        let mut switches = BTreeMap::new();
        for section in std::iter::once("default").chain(format) {
            // A format without its own section keeps the defaults.
            if section != "default"
                && let Ok(values) = ai::config(&config, "ai_driver_config", &[(section, None)])
            {
                driver.extend(values);
            }
            for group in ["behaviour", "behaviourTest"] {
                if let Ok(values) =
                    ai::config(&config, "ai_driver_config", &[(section, None), (group, None)])
                {
                    switches.extend(values);
                }
            }
        }
        let bytes = read_doc(&root.join("ai/ai_vehicle_config.xml"))?;
        let doc = binxml::read(&bytes)?;
        let mut vehicle = ai::config(
            &doc,
            "ai_vehicle_config",
            &[("vehicle_types", None), ("vehicle_type", Some("default"))],
        )?;
        if let Ok(overrides) = ai::config(
            &doc,
            "ai_vehicle_config",
            &[("vehicle_types", None), ("vehicle_type", Some(class))],
        ) {
            vehicle.extend(overrides);
        }
        let bytes = read_doc(&root.join("ai/ai_driver_parameter_mappings.xml"))?;
        let mapping_doc = binxml::read(&bytes)?;
        let mut mappings = ai::mappings(&mapping_doc, "default")?;
        if let Some(format) = format
            && let Ok(overrides) = ai::mappings(&mapping_doc, format)
        {
            mappings.extend(overrides);
        }
        let mut data = Self {
            line,
            brake_lines,
            brake_tables,
            cruise_tables,
            driver,
            vehicle,
            mappings,
            switches,
            drift: None,
            start: None,
        };
        if data.switch("behaviour_drift") {
            let (line, brake_lines) =
                track_lines(&ai::track_named(&track_doc, ai_track, "drift")?, &class_lines)?;
            data.drift = Some(Box::new(Self {
                line,
                brake_lines,
                ..data.clone()
            }));
        }
        let start = ai::track_named(&track_doc, ai_track, "start")?;
        if start
            .gates
            .iter()
            .zip(&track.gates)
            .any(|(s, o)| s.racing_line != o.racing_line)
        {
            let (line, brake_lines) = track_lines(&start, &class_lines)?;
            data.start = Some(Box::new(Self {
                line,
                brake_lines,
                drift: None,
                ..data.clone()
            }));
        }
        Ok(data)
    }

    /// A `behaviour_*` or `test_*` switch of the driver config.
    fn switch(&self, name: &str) -> bool {
        self.switches.get(name).is_some_and(|&v| v != 0.)
    }

    /// Braking distance from `from` down to `to` (0x7A6650).
    pub fn braking_distance(&self, grip: f32, from: f32, to: f32) -> f32 {
        grip_distance(&self.brake_tables, grip, from) - grip_distance(&self.brake_tables, grip, to)
    }

    /// Coasting distance from `from` down to `to` on the cruise tables
    /// (0x7A6650 on vehicle AI data +8).
    pub fn cruise_distance(&self, grip: f32, from: f32, to: f32) -> f32 {
        grip_distance(&self.cruise_tables, grip, from) - grip_distance(&self.cruise_tables, grip, to)
    }
}

/// Evaluated parameters for one driver.
#[derive(Clone, Debug)]
pub struct Parameters {
    la: [f32; 10],
    th_target_small: f32,
    th_target_medium: f32,
    th_target_speed_small: f32,
    th_target_speed_big: f32,
    th_target_move_speed: f32,
    th_corner_lift_min: f32,
    th_brake_line_speed_drop: f32,
    tg_offset_speed_factor: f32,
    tg_offset_max_for_speed: f32,
    ra_generate: bool,
    ra_merge: f32,
    ra_merge_cornering: f32,
    ra_merge_alone: f32,
    ra_alone_on: f32,
    ra_alone_off: f32,
    ra_generate_time: f32,
    cornering_curvature: f32,
    st_ang_vel_factor: f32,
    st_angle_factor: f32,
    /// stProportionRabbitWhenInAir: in the air, 0 steers along the slip angle,
    /// 1 keeps the ground law (0x77AC50).
    st_proportion_in_air: f32,
    slide_lift: [f32; 3],
    brake_cruise_exponent: f32,
    /// tcMinSpeed, tcMaxSpeed, tcMinLevel, tcMaxLevel, tcMaxWheelSpin
    /// (ai_vehicle_config, built-in defaults 1, 20, 0, 0.5, 2).
    traction: [f32; 5],
    /// absMinSpeed, absMinWheelLock, absMaxWheelLock, absMinBrake
    /// (ai_vehicle_config, built-in defaults 5, 0.7, 0.3, 0.2).
    abs: [f32; 4],
    /// Gains P/I/D and limits P/I/D/output (ai_vehicle_config pid*).
    pid: [f32; 7],
    /// pidPredictionMultiplierLowSpeed/HighSpeed, pidPredictionLowSpeed/
    /// HighSpeed (vehicle config +0x2AC/+0x2C0/+0x2D4/+0x2E8).
    pid_prediction: [f32; 4],
    /// pidAgainstCornerMinAngle/MaxAngle (+0x324/+0x338) in degrees. The
    /// "with corner" pair (+0x2FC/+0x310) is never selected (see `controls`).
    pid_corner_angle: [f32; 2],
    pub steering_speed: f32,
    brake_on_speed: f32,
    brake_off_speed: f32,
    throttle_on_speed: f32,
    throttle_off_speed: f32,
    pub corner_speed_in_ratio: f32,
    pub corner_brake_distance_factor: f32,
    pub throttle_confidence: f32,
    corner_lift: [f32; 2],
    /// thFollow small/medium throttle, dist big/medium/small, speed small/medium.
    follow: [f32; 7],
    follow_brake: f32,
    traffic: Traffic,
    rescue: Rescue,
    deviation_small: f32,
    deviation_large: f32,
    faults: Faults,
    /// tgNonOptimalSpeedFactor: corner-speed factor on a line other than
    /// `optimal` (0x792C80).
    tg_non_optimal_speed_factor: f32,
    /// behaviour_race_damaged and behaviour_controlled_stop switches.
    race_damaged: bool,
    controlled_stop: bool,
    /// behaviour_race_start switch and rsEndSpeed1/2, rsOffsetMergeFactor1/2,
    /// rsTimeout (driver config +0xACC/+0xAE0/+0xAF4/+0xB08/+0xB1C; built-in
    /// merge factors 0.25 and 1.0).
    race_start: bool,
    rs: [f32; 5],
    /// The drift behaviour and test, when the event's config switches both on.
    pub drift: Option<Drift>,
}

/// Drift behaviour parameters (docs/DISCIPLINES.md §4.1): ai_vehicle_config
/// `drift*` (vcfg+0x3B0..+0x4F0), thBrakeLineHoldTimeDrift (cfg+0x2E8) and the
/// mappings drift_skill (45) and drift_position (46). Built-in defaults where
/// the shipped XML is silent: driftInitiateGrip 1.2, driftRacingGrip 1.1,
/// driftBrakeLineAdvance 8 m.
#[derive(Clone, Debug)]
pub struct Drift {
    st_ang_vel_factor: f32,
    /// driftAngleRangeMin/Max and driftAngleMax (degrees).
    angle_range: [f32; 2],
    angle_max: f32,
    /// driftMinGrip / driftMaxGrip (rear wheels).
    min_grip: f32,
    max_grip: f32,
    /// driftStartAngle / driftEndAngle (degrees).
    start_angle: f32,
    end_angle: f32,
    merge: f32,
    speed_min: f32,
    grip_front: f32,
    initiate_grip: f32,
    racing_grip: f32,
    brake_line_advance: f32,
    hold_time: f32,
    pub skill: f32,
    pub position: f32,
}

/// Mistake, failure and let-pass parameters (docs/AI_MISTAKES_PACING_RESEARCH.md
/// §2, §4.2, §5; built-in defaults where the shipped XML is silent).
#[derive(Clone, Debug)]
pub struct Faults {
    /// mistake_probability (map 33) and the per-type factors, by mistake id:
    /// heavy, light, late, early, brake_spin, powerout_spin, brake_lock. The
    /// brake_lock slot reads holder 49, past the table: 0 live [V].
    probability: f32,
    factors: [f32; 7],
    /// generateMistakes (cfg+0xCAC, built-in false) and
    /// mkNumBrakeLinesBeforeMistakes (cfg+0xC84, 3).
    generate: bool,
    lines_before: u32,
    /// mistakeEntrySpinRearGrip, mistakePowerSpinRearGrip, mistakeBrakeLockGrip.
    spin_rear_grip: f32,
    power_rear_grip: f32,
    lock_grip: f32,
    racing_grip: f32,
    crash_front_grip: f32,
    crash_rear_grip: f32,
    crash_min_speed: f32,
    retirement_speed: f32,
    /// rtStoppedSpeed (cfg+0x4F0, built-in 0.5).
    stopped_speed: f32,
    damage_retirement_ratio: f32,
    /// coLetPassDistance (built-in 30) and coLetPassThrottleRatio.
    let_pass_distance: f32,
    let_pass_throttle: f32,
}

/// Avoid, recovery and retire parameters (docs/AI_AVOID_RECOVERY_RESEARCH.md
/// §2.4, §3, §4; built-in defaults where the shipped XML is silent).
#[derive(Clone, Debug)]
pub struct Rescue {
    av_eta: f32,
    av_decision: f32,
    av_buffer: f32,
    av_buffer_speed: f32,
    av_merge: f32,
    av_min_racing_speed: f32,
    av_max_racing_angle: f32,
    av_min_speed_normal: f32,
    av_min_speed_slow: f32,
    av_stopped_speed: f32,
    av_stopped_target: f32,
    av_normal_target: f32,
    av_wrong_way_target: f32,
    av_alongside: bool,
    av_figure_of_eight: bool,
    braking_buffer: f32,
    rc_merge: f32,
    rc_collision_timeout: f32,
    rt_stuck_time: f32,
    rt_stuck_distance_sq: f32,
    rt_player_distance_sq: f32,
    tg_merge: f32,
    target_speed_big: f32,
    contact_zone: f32,
}

/// Fire priorities (ai_behaviour_fire.xml, docs/AI_AVOID_RECOVERY_RESEARCH.md §4).
const AVOID_SCORE: u8 = 15;
const RECOVERY_SCORES: [u8; 4] = [11, 16, 17, 19];
const RECOVER_TBONE_SCORE: u8 = 18;
const RETIRE_STUCK_SCORE: u8 = 23;
/// race_start's score while active (its first ai_behaviour_fire.xml value).
const RACE_START_SCORE: u8 = 14;
/// race_start side choice: a car more than 2 m to one side of us (0xCB8DA4),
/// and the random launch lane width (0xCB093C).
const RACE_START_SIDE_DISTANCE: f32 = 2.;
const RACE_START_LANE: f32 = 5.;
/// Spun-test hysteresis (.data 0xE4D0F0 0.1·π, 0xCF6F08 π/3, 0xE4D0EC 3 m/s).
const SPUN_HOLD_SLIP: f32 = 0.1 * std::f32::consts::PI;
const SPUN_HOLD_HEADING: f32 = std::f32::consts::FRAC_PI_3;
const SPUN_HOLD_SPEED: f32 = 3.;
/// recover_tbone hold after the last qualifying contact (0x78ACC0).
const TBONE_TIME: f32 = 1.;
/// Recovery reverse speed and the stop speed before reversing (0x78EE70).
const RECOVERY_REVERSE_SPEED: f32 = 5.;
const RECOVERY_STOPPED_SPEED: f32 = 0.1;
/// avoid follow distance in side mode 0 (0x799DE0).
const AVOID_FOLLOW_DISTANCE: f32 = 3.;

/// Built-in defaults for values absent from the shipped XML
/// (docs/AI_DRIVER_RESEARCH.md §4).
const BRAKE_LINE_CANCEL_FACTOR: f32 = 0.8;
const SLOW_SPEED: f32 = 3.;
const MINIMUM_MOVING_SPEED: f32 = 1.;
/// Alongside gaps above this are "no car on that side" (0xCBD664 = 2000).
const ALONGSIDE_EMPTY_GAP: f32 = 2000.;
const SLIDE_LIFT_MIN_SPEED: f32 = 5.;
/// Grip input for the speed->distance tables. The AI car-view grip (+0x144)
/// is not yet traced; the lowest shipped grip is used.
const TABLE_GRIP: f32 = 2.;

impl Parameters {
    pub fn new(
        data: &AiData,
        difficulty: f32,
        characteristic: impl Fn(&str) -> f32,
    ) -> Result<Self, String> {
        let d = |k: &str| {
            data.driver
                .get(k)
                .copied()
                .ok_or(format!("AI driver config {k} absent"))
        };
        let v = |k: &str| {
            data.vehicle
                .get(k)
                .copied()
                .ok_or(format!("AI vehicle config {k} absent"))
        };
        let d_or = |k: &str, default: f32| data.driver.get(k).copied().unwrap_or(default);
        let v_or = |k: &str, default: f32| data.vehicle.get(k).copied().unwrap_or(default);
        // A behaviour absent from the config keeps the port's default (on).
        let on = |k: &str| data.switches.get(k).is_none_or(|&s| s != 0.);
        let m = |k: &str| {
            data.mappings
                .get(k)
                .map(|m| m.evaluate(difficulty, characteristic(&m.characteristic)))
                .ok_or(format!("AI mapping {k} absent"))
        };
        Ok(Self {
            la: [
                d("laDistMin")?,
                d("laDistMax")?,
                d("laAngleMin")?,
                d("laAngleMax")?,
                d("laSpeedMin")?,
                d("laSpeedMax")?,
                d("laStepMax")?,
                d("laStepProportion")?,
                d("laDistExtraMin")?,
                d("laSpeedExtraMin")?,
            ],
            th_target_small: d("thTargetSmall")?,
            th_target_medium: d("thTargetMedium")?,
            th_target_speed_small: d("thTargetSpeedSmall")?,
            th_target_speed_big: d("thTargetSpeedBig")?,
            th_target_move_speed: d("thTargetMoveSpeed")?,
            th_corner_lift_min: d("thCornerLiftMin")?,
            th_brake_line_speed_drop: d("thBrakeLineSpeedDrop")?,
            tg_offset_speed_factor: d("tgOffsetSpeedFactor")?,
            tg_offset_max_for_speed: d("tgOffsetMaxForSpeed")?,
            ra_generate: d("raGenerateRandomOffsets")? != 0.,
            ra_merge: d("raOffsetMergeFactor")?,
            ra_merge_cornering: d("raOffsetMergeFactorCornering")?,
            ra_merge_alone: d("raOffsetMergeFactorAlone")?,
            ra_alone_on: d("raAloneDistanceOn")?,
            ra_alone_off: d("raAloneDistanceOff")?,
            ra_generate_time: d("raOffsetGenerateTime")?,
            cornering_curvature: d("crMaxTrackCurvature")?,
            st_ang_vel_factor: v("stAngVelFactor")?,
            st_angle_factor: v("stAngleFactorMin")?,
            st_proportion_in_air: d("stProportionRabbitWhenInAir")?,
            slide_lift: [
                v("throttleSlideLiftStartAngle")?,
                v("throttleSlideLiftEndAngle")?,
                v("throttleSlideLiftMin")?,
            ],
            brake_cruise_exponent: v("brakeCruiseExponent")?,
            traction: [
                v_or("tcMinSpeed", 1.),
                v_or("tcMaxSpeed", 20.),
                v_or("tcMinLevel", 0.),
                v_or("tcMaxLevel", 0.5),
                v_or("tcMaxWheelSpin", 2.),
            ],
            abs: [
                v_or("absMinSpeed", 5.),
                v_or("absMinWheelLock", 0.7),
                v_or("absMaxWheelLock", 0.3),
                v_or("absMinBrake", 0.2),
            ],
            pid: [
                v("pidProportionalGain")?,
                v("pidIntegralGain")?,
                v("pidDerivativeGain")?,
                v("pidProportionalLimit")?,
                v("pidIntegralLimit")?,
                v("pidDerivativeLimit")?,
                v("pidOutputLimit")?,
            ],
            pid_prediction: [
                v_or("pidPredictionMultiplierLowSpeed", -1.),
                v_or("pidPredictionMultiplierHighSpeed", 0.8),
                v_or("pidPredictionLowSpeed", 10.),
                v_or("pidPredictionHighSpeed", 20.),
            ],
            pid_corner_angle: [
                v_or("pidAgainstCornerMinAngle", 0.),
                v_or("pidAgainstCornerMaxAngle", 30.),
            ],
            steering_speed: m("steering_speed")?,
            brake_on_speed: m("brake_on_speed")?,
            brake_off_speed: m("brake_off_speed")?,
            throttle_on_speed: m("throttle_on_speed")?,
            throttle_off_speed: m("throttle_off_speed")?,
            corner_speed_in_ratio: m("corner_speed_in_ratio")?,
            corner_brake_distance_factor: m("corner_brake_distance_factor")?,
            throttle_confidence: m("throttle_confidence")?,
            corner_lift: [
                m("throttle_corner_lift_start_angle")?,
                m("throttle_corner_lift_end_angle")?,
            ],
            follow: [
                d("thFollowSmall")?,
                d("thFollowMedium")?,
                d("thFollowDistBig")?,
                d("thFollowDistMedium")?,
                d("thFollowDistSmall")?,
                d("thFollowSpeedSmall")?,
                d("thFollowSpeedMedium")?,
            ],
            follow_brake: d("brFollowSpeedBrake")?,
            traffic: Traffic {
                ov_reject: d_or("ovRejectDistance", 60.),
                ov_side_reject: d_or("ovSideRejectDistance", 12.),
                ov_speed_difference: d_or("ovSpeedDifference", 0.1),
                ov_continue: d_or("ovContinueSpeedDifference", -2.),
                ov_time: d_or("ovTime", 3.),
                ov_auto_accept: d_or("ovAutoAcceptDistance", 3.),
                ov_slow_speed: d_or("ovSlowSpeed", 12.),
                ov_slow_reject: d_or("ovSlowRejectDistance", 12.),
                ov_follow: d_or("ovFollowDistance", 3.),
                ov_decision: d_or("ovDecisionTimeOut", 0.25),
                ov_safe: d_or("ovSafeSeparationDistance", 1.),
                ov_desired: d_or("ovDesiredSeparationDistance", 2.5),
                ov_min_fire: d_or("ovMinFireTime", 3.),
                ov_merge: d_or("ovOffsetMergeFactor", 4.),
                ov_merge_corner: d_or("ovOffsetMergeFactorCorner", 0.5),
                ov_corner: [
                    d_or("ovMinCornerCurvature", 0.001),
                    d_or("ovMaxCornerCurvature", 0.04),
                ],
                as_reject: d_or("asRejectDistance", 10.),
                as_merge: d_or("asOffsetMergeFactor", 4.),
                as_target_buffer: d_or("asTargetBuffer", -2.),
                front_projection: [
                    m("alongside_front_projection_on_distance")?,
                    m("alongside_front_projection_off_distance")?,
                ],
                rear_projection: [
                    m("alongside_rear_projection_on_distance")?,
                    m("alongside_rear_projection_off_distance")?,
                ],
                min_space: m("alongside_min_space_to_leave_distance")?,
                max_space: m("alongside_max_space_to_leave_distance")?,
                yield_: [
                    m("alongside_yield_distance")?,
                    m("alongside_yield_max_brake")?,
                    m("alongside_yield_min_speed_diff")?,
                    m("alongside_yield_max_speed_diff")?,
                    m("alongside_yield_follow_distance")?,
                    m("alongside_yield_extra_respect_distance")?,
                    d_or("coYieldMinSpeed", 16.),
                    d_or("coYieldDistToLimit", 1.),
                ],
                pass_angle: d_or("avPassAngle", 0.15),
                similar_angle: d_or("avSimilarDirectionMaxAngle", 0.5),
                pass_slow_speed: d_or("avPassSlowSpeed", 15.),
                pass_spare_room: d_or("avPassMaxSpareRoom", 4.),
                pass_side_speed: [
                    d_or("avPassSideSpeedNear", 2.),
                    d_or("avPassSideSpeedFar", 10.),
                ],
                pass_relative_speed: d_or("avPassMaxRelativeSpeed", 20.),
                separation_speed: d_or("avSeparationSpeedDiff", 5.),
                slow_speed: d_or("avSlowSpeed", 40.),
                pedal_push: d_or("avPercentagePedalPushDistance", 1.2),
                buffer: v("brakeBufferZone")?,
                extra_prediction: v("brakeExtraPrediction")?,
                separation: [
                    m("safe_separation_distance_small")?,
                    m("safe_separation_distance_large")?,
                ],
                prediction_ratio: m("brake_for_vehicle_prediction_ratio")?,
                cruise_speed: m("brake_for_vehicle_cruise_speed")?,
                view_angle: m("driver_view_angle")?,
            },
            rescue: Rescue {
                av_eta: d_or("avETAReactionTime", 2.),
                av_decision: d_or("avDecisionTime", 0.25),
                av_buffer: d_or("avBufferZone", 2.),
                av_buffer_speed: d_or("avBufferZoneBaseSpeed", 10.),
                av_merge: d_or("avOffsetMergeFactor", 5.),
                av_min_racing_speed: d_or("avMinRacingSpeed", 4.),
                av_max_racing_angle: d_or("avMaxRacingAngleFromTrack", 0.785),
                av_min_speed_normal: d_or("avMinSpeedNormal", 0.),
                av_min_speed_slow: d_or("avMinSpeedSlow", 9.),
                av_stopped_speed: d_or("avStoppedSpeed", 1.),
                av_stopped_target: d_or("avStoppedTargetSpeed", 30.),
                av_normal_target: d_or("avNormalTargetSpeed", 200.),
                av_wrong_way_target: d_or("avWrongWayTargetSpeed", 5.),
                av_alongside: d_or("avAvoidAlongside", 1.) != 0.,
                av_figure_of_eight: d_or("avFigureOfEightTrack", 0.) != 0.,
                braking_buffer: m("braking_buffer_distance")?,
                rc_merge: d_or("rcMergeFactor", 1.),
                rc_collision_timeout: d_or("rcCollisionTimeOut", 2.),
                rt_stuck_time: d_or("rtStuckTime", 20.),
                rt_stuck_distance_sq: d_or("rtStuckDistanceSq", 25.),
                rt_player_distance_sq: d_or("rtPlayerDistanceSq", 10000.),
                tg_merge: d_or("tgOffsetMergeFactor", 2.5),
                target_speed_big: d_or("brTargetSpeedBig", 2.),
                contact_zone: d_or("contactZone", 0.1),
            },
            deviation_small: m("deviation_from_racing_line_small")?,
            deviation_large: m("deviation_from_racing_line_large")?,
            faults: Faults {
                probability: m("mistake_probability")?,
                factors: [
                    m("mistake_brake_heavy_factor")?,
                    m("mistake_brake_light_factor")?,
                    m("mistake_brake_late_factor")?,
                    m("mistake_brake_early_factor")?,
                    m("mistake_brake_spin_factor")?,
                    m("mistake_powerout_spin_factor")?,
                    0.,
                ],
                generate: d_or("generateMistakes", 0.) != 0.,
                lines_before: d_or("mkNumBrakeLinesBeforeMistakes", 3.) as u32,
                spin_rear_grip: v("mistakeEntrySpinRearGrip")?,
                power_rear_grip: v("mistakePowerSpinRearGrip")?,
                lock_grip: v("mistakeBrakeLockGrip")?,
                racing_grip: v("racingGripMultiplier")?,
                crash_front_grip: v("crashFrontGrip")?,
                crash_rear_grip: v("crashRearGrip")?,
                crash_min_speed: v("crashMinSpeed")?,
                retirement_speed: d("rtRetirementSpeed")?,
                stopped_speed: d_or("rtStoppedSpeed", 0.5),
                damage_retirement_ratio: m("damage_retirement_ratio")?,
                let_pass_distance: d_or("coLetPassDistance", 30.),
                let_pass_throttle: d_or("coLetPassThrottleRatio", 0.8),
            },
            tg_non_optimal_speed_factor: d_or("tgNonOptimalSpeedFactor", 0.8),
            race_damaged: on("behaviour_race_damaged"),
            controlled_stop: on("behaviour_controlled_stop"),
            race_start: on("behaviour_race_start"),
            rs: [
                d_or("rsEndSpeed1", 30.),
                d_or("rsEndSpeed2", 50.),
                d_or("rsOffsetMergeFactor1", 0.25),
                d_or("rsOffsetMergeFactor2", 1.),
                d_or("rsTimeout", 10.),
            ],
            drift: if data.switch("behaviour_drift") && data.switch("test_drift") {
                Some(Drift {
                    st_ang_vel_factor: v("driftStAngVelFactor")?,
                    angle_range: [v("driftAngleRangeMin")?, v("driftAngleRangeMax")?],
                    angle_max: v("driftAngleMax")?,
                    min_grip: v("driftMinGrip")?,
                    max_grip: v("driftMaxGrip")?,
                    start_angle: v("driftStartAngle")?,
                    end_angle: v("driftEndAngle")?,
                    merge: v("driftMergeFactor")?,
                    speed_min: v("driftSpeedMin")?,
                    grip_front: v("driftGripFront")?,
                    initiate_grip: v_or("driftInitiateGrip", 1.2),
                    racing_grip: v_or("driftRacingGrip", 1.1),
                    brake_line_advance: v_or("driftBrakeLineAdvance", 8.),
                    hold_time: d_or("thBrakeLineHoldTimeDrift", 0.2),
                    skill: m("drift_skill")?,
                    position: m("drift_position")?,
                })
            } else {
                None
            },
        })
    }
}

/// Traffic behaviour parameters (shipped config, built-in defaults where absent).
#[derive(Clone, Debug)]
pub struct Traffic {
    ov_reject: f32,
    ov_side_reject: f32,
    ov_speed_difference: f32,
    ov_continue: f32,
    ov_time: f32,
    ov_auto_accept: f32,
    ov_slow_speed: f32,
    ov_slow_reject: f32,
    ov_follow: f32,
    ov_decision: f32,
    ov_safe: f32,
    ov_desired: f32,
    ov_min_fire: f32,
    ov_merge: f32,
    ov_merge_corner: f32,
    ov_corner: [f32; 2],
    as_reject: f32,
    as_merge: f32,
    /// asTargetBuffer (driver config +0x464): the alongside squeeze test.
    as_target_buffer: f32,
    front_projection: [f32; 2],
    rear_projection: [f32; 2],
    min_space: f32,
    max_space: f32,
    /// Yield: distance, max brake, min/max speed difference, follow distance,
    /// extra respect, minimum own speed, room to the limit.
    yield_: [f32; 8],
    pass_angle: f32,
    similar_angle: f32,
    pass_slow_speed: f32,
    pass_spare_room: f32,
    pass_side_speed: [f32; 2],
    pass_relative_speed: f32,
    separation_speed: f32,
    slow_speed: f32,
    pedal_push: f32,
    buffer: f32,
    extra_prediction: f32,
    separation: [f32; 2],
    prediction_ratio: f32,
    cruise_speed: f32,
    view_angle: f32,
}

/// A car's AI traffic extents (m) along its own axes from the body origin.
///
/// The AI car-state update (0x402D20) takes front, rear and half width from
/// the body dimension getter 0x9B8750: the `.ned` `collision_main` maximum Z,
/// minimum Z and maximum X (body +0xED6C/+0xED88/+0xED70) plus the body
/// offset +0x1630 on Z. It then grows them to cover the tyres: front-left
/// wheel Z + radius, rear-right wheel Z - radius, front-left X + half tyre
/// width, |rear-right X - half tyre width|. The state keeps the half width
/// (+0x110), the length front - rear (+0x114) and the bumper points
/// centre + forward * front / rear (+0xF0/+0x100); observed records copy
/// them (0x777A80).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Extents {
    pub half_width: f32,
    /// Front bumper distance ahead of the body origin.
    pub front: f32,
    /// Rear bumper position along forward (negative: behind the origin).
    pub rear: f32,
}

impl Extents {
    /// From the car's collision hull bounds and its vehicle words (wheel
    /// positions +0x14E0.., radii +0x1520.., tyre widths +0x1540.., body
    /// offset +0x1630). The optional deformation-lattice transform of the
    /// two bumper points (0x9A39D0 when body +0xC7AD is set) is not ported:
    /// the extents stay those of the undamaged hull.
    pub fn from_vehicle(hull: grid_formats::car_collision::Bounds, vehicle: &[u32]) -> Self {
        let w = |o: usize| f32::from_bits(vehicle[o / 4]);
        let offset_z = w(0x1638);
        let mut front = hull.max[2] + offset_z;
        let mut rear = hull.min[2] + offset_z;
        // Wheels: +0x1500 front-left centre, +0x14F0 rear-right centre.
        let front_tyre = w(0x1508) + w(0x1528);
        let rear_tyre = w(0x14F8) - w(0x1524);
        // Only one of the two length tests runs (0x403409/0x403473).
        if front_tyre > front {
            front = front_tyre;
        } else if rear > rear_tyre {
            rear = rear_tyre;
        }
        let mut half_width = hull.max[0];
        let left_tyre = w(0x1500) + w(0x1548) * 0.5;
        let right_tyre = (w(0x14F0) - w(0x1544) * 0.5).abs();
        if left_tyre > half_width {
            half_width = left_tyre;
        } else if right_tyre > half_width {
            half_width = right_tyre;
        }
        Self {
            half_width,
            front,
            rear,
        }
    }
    /// Half the length (car state +0x114 / 2).
    pub fn half_length(&self) -> f32 {
        0.5 * (self.front - self.rear)
    }
}

/// Another car seen by the AI (observed record: id, centre, axes, velocity,
/// extents).
#[derive(Clone, Copy)]
pub struct Other {
    pub id: u32,
    pub position: [f32; 3],
    pub velocity: [f32; 4],
    /// Unit forward and car-left (+X) axes.
    pub forward: [f32; 3],
    pub left: [f32; 3],
    pub extents: Extents,
}

impl Other {
    /// From a 4x4 row-major pose (rows 0/2 = left/forward, 12..14 translation).
    pub fn from_pose(id: u32, pose: [f32; 16], velocity: [f32; 4], extents: Extents) -> Self {
        Self {
            id,
            position: [pose[12], pose[13], pose[14]],
            velocity,
            forward: [pose[8], pose[9], pose[10]],
            left: [pose[0], pose[1], pose[2]],
            extents,
        }
    }
    /// The observed record of a source car.
    pub fn of(id: u32, car: &crate::source_vehicle::Runtime) -> Self {
        Self::from_pose(id, car.pose(), car.velocity(), car.ai_extents())
    }
    fn half_width(&self) -> f32 {
        self.extents.half_width
    }
    fn half_length(&self) -> f32 {
        self.extents.half_length()
    }
    fn speed(&self) -> f32 {
        let v = self.velocity;
        (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt()
    }
    fn front(&self) -> [f32; 3] {
        std::array::from_fn(|k| self.position[k] + self.forward[k] * self.extents.front)
    }
    fn rear(&self) -> [f32; 3] {
        std::array::from_fn(|k| self.position[k] + self.forward[k] * self.extents.rear)
    }
    /// Record state 2 (not racing) when the car moves against its nose [I].
    fn backwards(&self) -> bool {
        let v = [self.velocity[0], self.velocity[1], self.velocity[2]];
        self.speed() > MINIMUM_MOVING_SPEED && dot3(v, self.forward) < 0.
    }
}

/// maxObservableVehicles, maxObservableVehiclesRear and
/// maxObservableViewDistance (cfg +0xC20/+0xC34/+0xC0C, §5.2).
const OBSERVABLE_MAX: usize = 5;
const OBSERVABLE_REAR: usize = 2;
const OBSERVABLE_VIEW: f32 = 200.;

/// Observable-list selection 0x7ACA60 (§5.2): the cars `own` observes,
/// given each other car's signed distance along the line (`along`,
/// positive ahead). With no more candidates than the maximum (the original
/// counts its own car among them), every car within the view distance is
/// kept in order. Otherwise each car within the view distance is classified
/// (types 1 ahead, 2/3 ahead off-axis, 4/5 behind, 6/7 alongside) and the
/// list is filled by priority: the front car, one rear car, one car on each
/// side, then the nearest of the rest up to the maximum.
fn observable(own: &Other, others: &[Other], along: &[f32]) -> Vec<Other> {
    observable_with(
        own,
        others,
        along,
        OBSERVABLE_MAX,
        OBSERVABLE_REAR.min(OBSERVABLE_MAX),
        OBSERVABLE_VIEW,
    )
}

#[allow(clippy::neg_cmp_op_on_partial_ord)] // The original's NaN outcomes.
fn observable_with(
    own: &Other,
    others: &[Other],
    along: &[f32],
    max: usize,
    rear: usize,
    view: f32,
) -> Vec<Other> {
    if others.len() < max {
        return others
            .iter()
            .zip(along)
            .filter(|(_, s)| s.abs() < view)
            .map(|(o, _)| *o)
            .collect();
    }
    // Records {car, type, distance, value, taken}; the local list holds 19.
    struct Record {
        car: usize,
        kind: u8,
        distance: f32,
        value: f32,
        taken: bool,
    }
    let mut records: Vec<Record> = Vec::new();
    let (mut ahead, mut behind, mut side_6, mut side_7) = (0, 0, 0, 0);
    let right = own.left.map(|x| -x);
    let (own_front, own_rear) = (own.front(), own.rear());
    for (car, (o, &s)) in others.iter().zip(along).enumerate() {
        let distance = s.abs();
        if !(distance < view) {
            continue;
        }
        let record = if distance <= 20. {
            let w = own.half_width() * 2.;
            let d = sub(o.position, own.position);
            let lateral = dot3(right, d);
            let euclid = dot3(d, d).sqrt();
            // Angle between own forward and the direction to the car.
            let angle = (dot3(own.forward, d) / euclid).acos().to_degrees();
            let gap = dot3(own.forward, sub(o.rear(), own_front)) - 1.;
            let kind = if lateral - w >= 0. || -lateral - w >= 0. {
                if gap > 0. {
                    if angle >= 15. {
                        if lateral <= 0. { 2 } else { 3 }
                    } else {
                        ahead += 1;
                        1
                    }
                } else if dot3(own.forward, sub(o.front(), own_rear)) + 1. >= 0. {
                    if lateral <= 0. {
                        side_6 += 1;
                        6
                    } else {
                        side_7 += 1;
                        7
                    }
                } else if angle > 165. {
                    behind += 1;
                    4
                } else {
                    5
                }
            } else if dot3(own.forward, d) > 0. {
                ahead += 1;
                1
            } else {
                behind += 1;
                4
            };
            Record {
                car,
                kind,
                distance: euclid,
                value: gap,
                taken: false,
            }
        } else {
            let kind = if s <= 0. { 5 } else { 1 };
            Record {
                car,
                kind,
                distance,
                value: s,
                taken: false,
            }
        };
        if records.len() < 19 {
            records.push(record);
        }
    }
    let mut out: Vec<Other> = Vec::new();
    // The first record with the smallest key below the view distance.
    let pick = |records: &[Record], key: &dyn Fn(&Record) -> Option<f32>| {
        let mut best = view;
        let mut found = None;
        for (i, r) in records.iter().enumerate() {
            if let Some(k) = key(r)
                && k < best
            {
                best = k;
                found = Some(i);
            }
        }
        found
    };
    let take = |records: &mut [Record], i: Option<usize>, out: &mut Vec<Other>| {
        if let Some(i) = i {
            out.push(others[records[i].car]);
            records[i].taken = true;
        }
    };
    // Front: the nearest type 1 by bumper gap, else the nearest of 1-3.
    let front = if ahead == 0 {
        pick(&records, &|r| (r.kind < 4).then_some(r.distance))
    } else {
        pick(&records, &|r| (r.kind == 1).then_some(r.value))
    };
    take(&mut records, front, &mut out);
    if rear != 0 {
        let back = if behind == 0 {
            pick(&records, &|r| (r.kind == 5).then_some(r.distance))
        } else {
            pick(&records, &|r| (r.kind == 4).then_some(-r.value))
        };
        take(&mut records, back, &mut out);
    }
    if side_6 + side_7 > 0 {
        if side_6 == 0 {
            let i = pick(&records, &|r| (r.kind == 7).then_some(-r.value));
            take(&mut records, i, &mut out);
        } else if side_7 == 0 {
            let i = pick(&records, &|r| (r.kind == 6).then_some(r.value));
            take(&mut records, i, &mut out);
        } else {
            // One pass: type 6 by the smallest value; type 7 is tested
            // against the type-6 running minimum, so the last such type 7
            // wins (as in the original).
            let (mut best, mut six, mut seven) = (view, None, None);
            for (i, r) in records.iter().enumerate() {
                if r.kind == 6 && r.value < best {
                    best = r.value;
                    six = Some(i);
                }
                if r.kind == 7 && -r.value < best {
                    seven = Some(i);
                }
            }
            take(&mut records, six, &mut out);
            take(&mut records, seven, &mut out);
        }
    }
    // The rest: the nearest untaken cars (rear types only with a rear count).
    for _ in out.len()..max {
        let i = pick(&records, &|r| {
            (!r.taken && (rear != 0 || (r.kind != 4 && r.kind != 5))).then_some(r.distance)
        });
        take(&mut records, i, &mut out);
    }
    out
}

/// Box corners of a car state or observed record (0x77B400 / 0x77F9F0):
/// front point + m·forward and rear point − m·forward, ± (half width + m)
/// along car-right, flattened: front-right, front-left, rear-right, rear-left.
/// The forward axis is used as stored (not flattened or normalised).
fn box_corners(o: &Other, m: f32) -> [[f32; 3]; 4] {
    let right = o.left.map(|x| -x);
    let (front, rear) = (o.front(), o.rear());
    let f: [f32; 3] = std::array::from_fn(|k| front[k] + m * o.forward[k]);
    let r: [f32; 3] = std::array::from_fn(|k| rear[k] - m * o.forward[k]);
    let s = right.map(|x| x * (o.half_width() + m));
    let at = |p: [f32; 3], sign: f32| [p[0] + sign * s[0], 0., p[2] + sign * s[2]];
    [at(f, 1.), at(f, -1.), at(r, 1.), at(r, -1.)]
}

/// 0x774B00: one ordering pass over squared distances to `p`. Index 0 is
/// the nearest (first minimum), index 3 the farthest; 1 and 2 are the rest
/// in whatever order the single pass leaves them (not a full sort).
fn nearest_order(p: [f32; 3], points: &[[f32; 3]; 4]) -> [usize; 4] {
    let dist = points.map(|q| {
        let d = sub(q, p);
        dot3(d, d)
    });
    let mut out = [0, 1, 2, 3];
    for i in 0..4 {
        let d = dist[i];
        let last = out[3];
        if d > dist[last] {
            out[i] = last;
            out[3] = i;
        }
        let first = out[0];
        if dist[first] > d {
            out[i] = first;
            out[0] = i;
        }
    }
    out
}

/// 0x7792C0: intersection of segments a0→a1 and b0→b1 in the xz plane:
/// (result, s, point) with result 1 when both parameters lie in 0..1, 2 when
/// the lines cross outside the segments, -1 parallel, -2 collinear;
/// point = a0 + s·(a1 − a0).
fn segment_intersect_xz(
    a0: [f32; 3],
    a1: [f32; 3],
    b0: [f32; 3],
    b1: [f32; 3],
) -> (i32, f32, [f32; 3]) {
    let (d, e, w) = (sub(a1, a0), sub(b1, b0), sub(a0, b0));
    let den = d[0] * e[2] - d[2] * e[0];
    let (result, s) = if -1e-5 < den && den < 1e-5 {
        let ns = w[2] * e[0] - w[0] * e[2];
        (if -1e-5 < ns && ns < 1e-5 { -2 } else { -1 }, 0.)
    } else {
        let inv = 1. / den;
        let s = (w[2] * e[0] - w[0] * e[2]) * inv;
        let t = (d[0] * w[2] - d[2] * w[0]) * inv;
        let inside = (0. ..=1.).contains(&s) && (0. ..=1.).contains(&t);
        (if inside { 1 } else { 2 }, s)
    };
    (result, s, std::array::from_fn(|k| a0[k] + s * d[k]))
}

/// contact classification 0x783B20 (always returns 0 in the original; its
/// effect is the contact-list entry): 1–4 an own corner inside the other
/// box, 5–8 an other corner inside ours (front, rear, right, left face),
/// refined when two corners are inside.
fn contact_classify(own: &Other, rec: &Other, margin: f32) -> Option<u8> {
    let (ob, rb) = (box_corners(own, margin), box_corners(rec, margin));
    let (own_right, rec_right) = (own.left.map(|x| -x), rec.left.map(|x| -x));
    let mut hits: Vec<(u8, [f32; 3])> = Vec::new();
    for (i, &c) in ob.iter().enumerate() {
        let (a, b) = (sub(rb[0], c), sub(rb[3], c));
        if 0. > dot3(rec.forward, a) * dot3(rec.forward, b)
            && 0. > dot3(rec_right, a) * dot3(rec_right, b)
        {
            hits.push((i as u8 + 1, c));
        }
    }
    if hits.len() != 2 {
        for &c in &rb {
            let (a, b) = (sub(ob[0], c), sub(ob[3], c));
            let (p1, q1) = (dot3(own.forward, a), dot3(own.forward, b));
            let (p2, q2) = (dot3(own_right, a), dot3(own_right, b));
            if 0. > p1 * q1 && 0. > p2 * q2 {
                let (lat, side) = if q2.abs() > p2.abs() {
                    (p2.abs(), 7)
                } else {
                    (q2.abs(), 8)
                };
                let (lon, face) = if q1.abs() > p1.abs() {
                    (p1.abs(), 5)
                } else {
                    (q1.abs(), 6)
                };
                hits.push((if lat > lon { face } else { side }, c));
            }
        }
    }
    let length = own.extents.front - own.extents.rear;
    let ratio = |a: [f32; 3], b: [f32; 3]| dot3(own.forward, sub(a, b)) / length;
    Some(match hits[..] {
        [] => return None,
        [(7, p)] => {
            if 0.33 > ratio(p, ob[2]) {
                3
            } else {
                7
            }
        }
        [(8, p)] => {
            if 0.33 > ratio(p, ob[2]) {
                4
            } else {
                8
            }
        }
        [(t, _)] => t,
        [(t1, p1), (t2, p2)] => match t1 {
            1 if t2 == 7 => {
                if ratio(p1, p2) > 0.35 {
                    7
                } else {
                    1
                }
            }
            1 => {
                if t2 == 5 || t2 == 2 {
                    5
                } else {
                    7
                }
            }
            2 if t2 == 8 => {
                if ratio(p1, p2) > 0.35 {
                    8
                } else {
                    2
                }
            }
            2 => {
                if t2 == 5 || t2 == 1 {
                    5
                } else {
                    8
                }
            }
            3 if t2 == 7 => {
                if ratio(p2, p1) > 0.45 {
                    7
                } else {
                    3
                }
            }
            3 => {
                if t2 == 6 || t2 == 4 {
                    6
                } else {
                    7
                }
            }
            4 if t2 == 8 => {
                if ratio(p2, p1) > 0.45 {
                    8
                } else {
                    4
                }
            }
            4 => {
                if t2 == 6 || t2 == 3 {
                    6
                } else {
                    8
                }
            }
            t => t,
        },
        [(t, _), ..] => t,
    })
}

/// One path-obstacle entry (0x30 bytes in the original): relative velocity
/// (y cleared), hit point, record, distance along the sweep, time (≥ 0).
#[derive(Clone, Copy, Debug)]
struct ObstacleEntry {
    relative: [f32; 3],
    #[allow(dead_code)]
    point: [f32; 3],
    other: usize,
    distance: f32,
    #[allow(dead_code)]
    time: f32,
}

/// Path-obstacle scan 0x78B560 result: the kept obstacle (smallest unclamped
/// time, first on ties), the obstacle list and the contact list (6 each).
struct PathObstacle {
    best: Option<ObstacleEntry>,
    #[allow(dead_code)]
    list: Vec<ObstacleEntry>,
    contacts: Vec<(usize, u8)>,
}

/// Inputs of [`path_obstacle_scan`] from the driver config (cfg +0x8D8,
/// +0x928, +0x93C, +0x9A0, +0xBE4).
struct ScanConfig {
    stopped_speed: f32,
    buffer_zone: f32,
    buffer_base_speed: f32,
    similar_angle: f32,
    contact_zone: f32,
}

/// Path-obstacle scan 0x78B560 (docs/AI_AVOID_RECOVERY_RESEARCH.md §2.1):
/// per observed car, the contact classification, then the sweep of our box
/// along the relative velocity against the car's box.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // The original's NaN outcomes.
fn path_obstacle_scan(
    own: &Other,
    own_speed: f32,
    others: &[Other],
    cfg: &ScanConfig,
    reverse: bool,
) -> PathObstacle {
    let mut result = PathObstacle {
        best: None,
        list: Vec::new(),
        contacts: Vec::new(),
    };
    let base = cfg.buffer_base_speed;
    let ratio = if base == 0. { 0. } else { own_speed / base };
    let raw = ratio * cfg.buffer_zone;
    let b = cfg.buffer_zone;
    let buffer = if b > 0. {
        if raw < 0. {
            0.
        } else if raw > b {
            b
        } else {
            raw
        }
    } else if b > raw {
        b
    } else if raw > 0. {
        0.
    } else {
        raw
    };
    let fwd = horizontal(own.forward);
    let (front, rear) = (own.front(), own.rear());
    let own_front: [f32; 3] = std::array::from_fn(|k| front[k] + buffer * fwd[k]);
    let own_rear: [f32; 3] = std::array::from_fn(|k| rear[k] - buffer * fwd[k]);
    let centre = own.position;
    let heading = fwd;
    let cos_similar = (f64::from(cfg.similar_angle).cos() as f32).abs();
    let mut best_time = f32::MAX;
    for (index, rec) in others.iter().enumerate() {
        if let Some(kind) = contact_classify(own, rec, cfg.contact_zone)
            && result.contacts.len() < 6
        {
            result.contacts.push((index, kind));
        }
        if !(own_speed > cfg.stopped_speed) && !(rec.speed() > cfg.stopped_speed) {
            continue;
        }
        let direction = if rec.speed() > cfg.stopped_speed {
            horizontal(rec.forward)
        } else {
            rec.forward
        };
        let hdot = dot3(direction, heading);
        let p = if hdot > cos_similar {
            if rec.backwards() { rec.rear() } else { rec.front() }
        } else {
            rec.position
        };
        if !(dot3(sub(p, centre), heading) > 0.) {
            continue;
        }
        let v = own.velocity;
        let mut relative = [v[0] - rec.velocity[0], 0., v[2] - rec.velocity[2]];
        if !(dot3(sub(rec.position, centre), relative) > 0.) {
            continue;
        }
        let (rf, rr) = (rec.front(), rec.rear());
        let rec_front: [f32; 3] = std::array::from_fn(|k| rf[k] + buffer * rec.forward[k]);
        let rec_rear: [f32; 3] = std::array::from_fn(|k| rr[k] - buffer * rec.forward[k]);
        let gap = if reverse {
            sub(rec_front, own_rear)
        } else if hdot < 0. {
            sub(rec_front, own_front)
        } else {
            sub(rec_rear, own_front)
        };
        if dot3(gap, heading) > 0. && !(dot3(heading, relative) > 0.) {
            continue;
        }
        // Lateral test: an own corner (unbuffered box) inside the record box's
        // interval across the relative velocity.
        let rec_box = box_corners(rec, buffer);
        let ri = nearest_order(centre, &rec_box);
        if dot3(relative, relative) == 0. {
            let g = normalize(gap);
            relative = [g[0] * 1e-4, 0., g[2] * 1e-4];
        }
        let n = normalize(relative);
        let perp = [-n[2], 0., n[0]];
        let a = ri.map(|i| -dot3(rec_box[i], perp));
        let lo = a.iter().copied().fold(f32::INFINITY, f32::min);
        let hi = a.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        let own_box = box_corners(own, 0.);
        if !own_box.iter().any(|&c| {
            let d = dot3(perp, c);
            0. >= (d + hi) * (lo + d)
        }) {
            continue;
        }
        // Distance: four segment intersections over a 500 m sweep.
        let own_box = box_corners(own, buffer);
        let cr0 = rec_box[ri[0]];
        let oi = nearest_order(cr0, &own_box);
        let co0 = own_box[oi[0]];
        const SWEEP: f32 = 500.;
        let a_own: [f32; 3] = std::array::from_fn(|k| co0[k] + SWEEP * n[k]);
        let a_rec: [f32; 3] = std::array::from_fn(|k| cr0[k] - SWEEP * n[k]);
        let (mut distance, mut point, mut kind) = (f32::MAX, cr0, 2);
        let calls = [
            (co0, a_own, cr0, rec_box[ri[1]], true),
            (co0, a_own, cr0, rec_box[ri[2]], false),
            (cr0, a_rec, co0, own_box[oi[1]], false),
            (cr0, a_rec, co0, own_box[oi[2]], true),
        ];
        for (k, &(a0, a1, b0, b1, keep_point)) in calls.iter().enumerate() {
            let (r, s, out) = segment_intersect_xz(a0, a1, b0, b1);
            let accept = if k == 0 {
                r == 1 || r == 2
            } else {
                (r == 1 || (r == 2 && kind == 2)) && (s * SWEEP < distance || (r == 1 && kind == 2))
            };
            if accept {
                distance = s * SWEEP;
                point = if keep_point { out } else { cr0 };
                if k < 3 {
                    kind = r;
                }
            }
        }
        let time = distance / dot3(relative, relative).sqrt();
        let entry = ObstacleEntry {
            relative,
            point,
            other: index,
            distance,
            time: if time > 0. { time } else { 0. },
        };
        if result.list.len() < 6 {
            result.list.push(entry);
        }
        if best_time > time {
            best_time = time;
            result.best = Some(entry);
        }
    }
    result
}

fn normalize(a: [f32; 3]) -> [f32; 3] {
    let l = dot3(a, a).sqrt();
    a.map(|x| x / l)
}

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn horizontal(a: [f32; 3]) -> [f32; 3] {
    let l = (a[0] * a[0] + a[2] * a[2]).sqrt().max(1e-6);
    [a[0] / l, 0., a[2] / l]
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Behaviour {
    #[default]
    Race,
    Overtake,
    Alongside,
    Avoid,
    Recovery,
    RecoverTbone,
    Retire,
    /// Choreographed failures and stops (docs/AI_MISTAKES_PACING_RESEARCH.md §5):
    /// lost_control (21), controlled_stop (20), race_damaged (9) and the
    /// retire behaviour's failure/damage branch (score 10).
    LostControl,
    ControlledStop,
    RaceDamaged,
    RetireFailure,
    /// Drift events (score 2/12/13, docs/DISCIPLINES.md §4.1).
    Drift,
    /// The standing start (0x78A740, score 14 while active).
    RaceStart,
}

/// The car's place in the race, read by the drift test (test context +0x14
/// count, +0x18: +8 place, +0x10 race progress).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RaceView {
    /// 0-based race position (0 = leader).
    pub place: u32,
    pub cars: u32,
    /// Race progress in laps (not positive before the start line).
    pub progress: f32,
}

impl Default for RaceView {
    fn default() -> Self {
        Self {
            place: 0,
            cars: 1,
            progress: 0.,
        }
    }
}

/// Drift test object (0x78FE90, created by 0x7A7950: +8 fired, +0x10 place
/// when it fired, +0x14 timer = -1, +0x18 before-start flag) and the drift
/// behaviour state (+0x30, 0 at construction 0x795AC0).
#[derive(Clone, Copy, Debug)]
struct DriftState {
    fired: bool,
    place: u32,
    timer: f32,
    before_start: bool,
    state: u8,
    /// The previous update's next brake line: (drift speed, distance), the
    /// request fields +0x1C/+0x28 the test reads (0x78CB30).
    line: Option<(f32, f32)>,
}

impl Default for DriftState {
    fn default() -> Self {
        Self {
            fired: false,
            place: 0,
            timer: -1.,
            before_start: false,
            state: 0,
            line: None,
        }
    }
}

impl DriftState {
    /// One update of the drift test 0x78FE90 with the race place `race` and
    /// `draw(a, b)` uniform between a and b. Returns true when the test is
    /// fired and the refusal 0x78CB30 must run.
    /// - Before the start (progress not positive) a first window is drawn.
    /// - Racing, the test fires when its timer runs out, or at once on an
    ///   even place after its first drift; a fired test lasts a random
    ///   window and stops when that ends unless the car gained places or
    ///   leads, then pauses for a random time.
    fn test(&mut self, race: RaceView, dt: f32, mut draw: impl FnMut(f32, f32) -> f32) -> bool {
        let RaceView {
            place,
            cars,
            progress,
        } = race;
        let f = if cars < 2 {
            0.
        } else {
            place as f32 / (cars - 1) as f32
        };
        let window = |ranges: [[f32; 2]; 2]| {
            (
                remap(f, 0., 1., ranges[0][0], ranges[0][1]),
                remap(f, 0., 1., ranges[1][0], ranges[1][1]),
            )
        };
        if progress <= 0. {
            self.fired = false;
            self.before_start = true;
            if self.timer < 0. {
                let a = (f * -8. + 10.).clamp(2., 10.);
                let b = (f * -12. + 20.).clamp(8., 20.);
                self.timer = draw(a, b);
            }
            return false;
        }
        self.timer -= dt;
        if self.fired {
            if self.timer <= 0. && self.place <= place && place != 0 {
                self.fired = false;
                let (a, b) = window(DRIFT_PAUSE);
                self.timer = draw(a, b);
                return false;
            }
        } else {
            if self.timer < 0. || (place % 2 == 0 && !self.before_start) {
                self.fired = true;
            }
            if !self.fired {
                return false;
            }
            let (a, b) = window(DRIFT_FIRE);
            self.timer = draw(a, b);
            self.place = place;
            self.before_start = false;
        }
        true
    }
}

impl Drift {
    /// Drift behaviour state update 0x78B3C0: 1 approach, 2 initiate (a
    /// cornering test below 50 m/s), 3 drifting (slip above driftStartAngle),
    /// 4 end (slip below driftEndAngle or the corner over), then 1 again;
    /// below driftSpeedMin it is 1.
    fn next_state(&self, state: u8, speed: f32, slip: f32, cornering: bool) -> u8 {
        let state = state.max(1);
        if speed < self.speed_min {
            return 1;
        }
        match state {
            1 if cornering && speed < DRIFT_INITIATE_SPEED => 2,
            2 if slip > self.start_angle.to_radians() => 3,
            3 if slip < self.end_angle.to_radians() || !cornering => 4,
            4 => 1,
            s => s,
        }
    }

    /// Request grips while drifting (0x783A10 state 3), rear L/R then front
    /// L/R: the front gets driftGripFront; the rear driftMinGrip below the
    /// skill-scaled angle range, 1 inside it and up to driftMaxGrip toward
    /// driftAngleMax above it. The range's top is lowered to the angle at
    /// which the car (half width, length) still fits the `outside` room to
    /// the track limit: asin((room - w) / (length + w)) (0xBF8C93 = asin).
    fn drifting_grip(&self, angle: f32, outside: f32, [half_width, length]: [f32; 2]) -> [f32; 4] {
        let (fit, reach) = (outside - half_width, length + half_width);
        let limit = if fit < reach {
            (fit / reach).asin().to_degrees()
        } else {
            90.
        };
        let mut high = self.angle_range[1];
        if limit < high {
            high = limit;
        }
        let low = self.skill * self.angle_range[0];
        let rear = if angle > high {
            remap(angle, high, self.angle_max, 1., self.max_grip)
        } else if low <= angle {
            1.
        } else {
            remap(angle, low, high, self.min_grip, 1.)
        };
        [rear, rear, self.grip_front, self.grip_front]
    }
}

/// Drift test ranges (0x78FE90, .rdata): first window before the start
/// clamp(10 - 8f, 2, 10)..clamp(20 - 12f, 8, 20) s, fired window 48..16 and
/// 24..8 s, pause 15..30 and 5..15 s over the place fraction f (0 leader,
/// 1 last), retry after a refused drift 3 s.
const DRIFT_FIRE: [[f32; 2]; 2] = [[48., 16.], [24., 8.]];
const DRIFT_PAUSE: [[f32; 2]; 2] = [[15., 30.], [5., 15.]];
const DRIFT_RETRY: f32 = 3.;
/// Drift behaviour: initiation needs speed below 50 m/s (0xCB1104); the wall
/// limit starts at 90 degrees (0xCB3D64); fire scores (ai_behaviour_fire.xml
/// drift entries 2, 12, 13).
const DRIFT_INITIATE_SPEED: f32 = 50.;
const DRIFT_SCORES: [u8; 3] = [2, 12, 13];
/// The next corner's direction is measured 20 m past the brake line (0x792580).
const NEXT_CORNER_DISTANCE: f32 = 20.;

/// Per-car inputs from the race manager (choreographer, ctx+0x38 =
/// drv+0x3150): corner-speed scale A (+0x3154) and floor B (+0x3158), the
/// backwards/yield flag (+0x316C), the engine-failure damage gate (+0x316D)
/// with the engine damage value (own+0x11C), the per-wheel failure flags
/// (car adapter +0x254) and a controlled-stop request.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pace {
    pub a: f32,
    pub b: f32,
    pub backwards: bool,
    pub damage: f32,
    pub damage_gate: bool,
    pub failures: [bool; 4],
    pub stop: bool,
}

impl Default for Pace {
    fn default() -> Self {
        Self {
            a: 1.,
            b: 0.,
            backwards: false,
            damage: 0.,
            damage_gate: false,
            failures: [false; 4],
            stop: false,
        }
    }
}

/// Mistake ids (enum 0xCF6F64).
pub const BRAKE_HEAVY: u8 = 0;
pub const BRAKE_LIGHT: u8 = 1;
pub const BRAKE_LATE: u8 = 2;
pub const BRAKE_EARLY: u8 = 3;
pub const BRAKE_SPIN: u8 = 4;
pub const POWEROUT_SPIN: u8 = 5;
pub const BRAKE_LOCK: u8 = 6;

/// Mistake slot drv+0x3100 (§2.1): armed (pending) type and timeout, active
/// type and timer, locked wheel, last brake-line id and the brake-line counter.
#[derive(Clone, Copy, Debug, Default)]
struct Mistake {
    pending: Option<(u8, f32)>,
    active: Option<(u8, f32)>,
    wheel: usize,
    line: Option<usize>,
    counter: u32,
}

/// Spin and other mistake durations (.rdata 0xCB1790 3 s, 0xCB093C 5 s), the
/// random arm timeout (0xCAFEC4 60 s), the cancel speed (0xCAFA58 10 m/s), the
/// brake_spin excess window (0xE4D0C0/0xE4D0BC) and the powerout throttle and
/// speed thresholds (0xE4D0C8/0xE4D0C4).
const MISTAKE_SPIN_TIME: f32 = 3.;
const MISTAKE_TIME: f32 = 5.;
const MISTAKE_ARM_TIMEOUT: f32 = 60.;
const MISTAKE_CANCEL_SPEED: f32 = 10.;
const BRAKE_SPIN_EXCESS: [f32; 2] = [8., 12.];
const POWEROUT_THROTTLE: f32 = 0.6;
const POWEROUT_SPEED: f32 = 10.;
/// lost_control: failed-wheel grip (0x776AB0), wobble rear grip, brake cap
/// and wobble period (0x77CF10).
const FAILED_WHEEL_GRIP: f32 = 0.1;
const WOBBLE_REAR_GRIP: f32 = 0.3;
const WOBBLE_BRAKE_CAP: f32 = 0.15;
const WOBBLE_PERIOD: f32 = 1.;
/// race_damaged: desired speed, throttle cap and side re-choice period (0x799FB0).
const DAMAGED_SPEED: f32 = 50.;
const DAMAGED_THROTTLE: f32 = 0.6;
const DAMAGED_PERIOD: f32 = 1.;

/// lost_control object (0x78B1F0): crash mode or the failed-wheel grip cut,
/// wobble timer and offset; the side/timer of race_damaged and retire.
#[derive(Clone, Copy, Default)]
struct Failure {
    crash: bool,
    wobble_timer: f32,
    wobble_offset: f32,
    side_timer: f32,
    side_offset: f32,
    removal: bool,
}

/// avoid test (0x79A960) memory and behaviour object (0x79C600): remembered
/// record, target id, side mode (0 follow, 1 negative, 2 positive), decision
/// timer, target speed and desired offset.
#[derive(Clone, Copy, Default)]
struct Avoid {
    remembered: Option<u32>,
    target: Option<u32>,
    side: u8,
    decision: f32,
    target_speed: f32,
    offset: f32,
}

/// recovery behaviour object (0x78EE70): mode, latched-after-stop, the
/// obstruction reverse toggle (kept across activations), elapsed time and
/// obstruction timer.
#[derive(Clone, Copy, Default)]
struct Recovery {
    mode: u8,
    latched: bool,
    toggle: bool,
    elapsed: f32,
    timer: f32,
}

/// race_start object (+0x35 active, +0x38 end speed, +0x3C merge factor,
/// +0x40 desired offset, +0x44 initial phase, +0x48 timeout, +0x4C side).
#[derive(Clone, Copy, Debug, Default)]
struct RaceStart {
    active: bool,
    end_speed: f32,
    merge: f32,
    offset: f32,
    initial: bool,
    timer: f32,
    /// Launch toward positive offsets (the right).
    right: bool,
}

/// Overtake behaviour object (+0x30 target, +0x34 state, +0x38 decision timer,
/// +0x3C minimum-fire timer).
#[derive(Clone, Copy, Default)]
struct Overtake {
    target: Option<u32>,
    state: u8,
    decision: f32,
    min_fire: f32,
}

/// Per-tick request fields edited by the behaviours.
struct Request {
    desired_offset: f32,
    merge: f32,
    desired_speed: f32,
    throttle_cap: f32,
    /// Followed car and follow distance (+0x70/+0x74); follow-brake cap +0x78.
    follow: Option<(Other, f32)>,
    follow_brake_cap: f32,
    /// Request flags +0xA4 brake to desired speed, +0xA5 force brake, +0xA6
    /// no steer, +0xB2 full brake, +0xAF reverse; limit types 3/4.
    brake_to_speed: bool,
    force_brake: bool,
    no_steer: bool,
    full_brake: bool,
    reverse: bool,
    track_limits: bool,
    /// Request grip multipliers +0xB8..+0xC4 in wheel order rear L/R, front
    /// L/R (0x776680), and the brake cap +0xA0.
    grip: [f32; 4],
    brake_cap: f32,
}

/// Car state seen by the AI.
pub struct View {
    /// 4x4 row-major pose: rows 0/1/2 = right-hand X / up / forward, 12..14 translation.
    pub pose: [f32; 16],
    pub velocity: [f32; 4],
    pub angular: [f32; 4],
    pub maximum_steering_angle: f32,
    /// World wheel positions (on_track test).
    pub wheels: [[f32; 3]; 4],
    /// Last tick's obstruction contact sum (collided_obstruction test);
    /// zero when no contact feed exists.
    pub obstruction: [f32; 3],
    /// The car's AI traffic extents (own car state +0x110/+0x114).
    pub extents: Extents,
    /// Per wheel: rolling speed (wheel +0xE0 × +0x104) and the driven flag
    /// (drive share +0x10C ≠ 0), as the AI car-state update 0x402D20 stores
    /// them (car state +0x240 and +0x250, read by vfuncs +0x10 and +0x14).
    pub wheel_speeds: [f32; 4],
    pub driven: [bool; 4],
    /// No wheel touches the ground: car state +0xE2 is 0 (0x402D20 sets it
    /// to "fewer than four wheels without contact", 0x990FC0).
    pub airborne: bool,
}

impl View {
    /// View of a source car with its obstruction contact sum.
    pub fn of(car: &crate::source_vehicle::Runtime, obstruction: [f32; 3]) -> Self {
        let pose = car.pose();
        let wheels = car.wheels().map(|w| {
            std::array::from_fn(|k| {
                pose[12 + k]
                    + w.anchor[0] * pose[k]
                    + w.anchor[1] * pose[4 + k]
                    + w.anchor[2] * pose[8 + k]
            })
        });
        Self {
            pose,
            velocity: car.velocity(),
            angular: car.angular_velocity(),
            maximum_steering_angle: car.maximum_steering_angle(),
            wheels,
            obstruction,
            extents: car.ai_extents(),
            wheel_speeds: car.wheel_rolling_speeds(),
            driven: car.driven_wheels(),
            airborne: car.wheels_in_contact() == [false; 4],
        }
    }
}

/// Recovery octant 0x77C9B0 from the side-mirrored angles in degrees (§4.1).
fn octant(a: f32, b: f32) -> u8 {
    let within = |x: f32, lo: f32, hi: f32| lo < x && x < hi;
    const TABLE: [([f32; 2], [f32; 2]); 8] = [
        ([0., 90.], [0., 90.]),
        ([-90., 0.], [0., 90.]),
        ([-90., 0.], [-90., 0.]),
        ([-180., -90.], [-90., 0.]),
        ([-180., -90.], [-180., -90.]),
        ([90., 180.], [-180., -90.]),
        ([90., 180.], [90., 180.]),
        ([0., 90.], [90., 180.]),
    ];
    TABLE
        .iter()
        .position(|&([b0, b1], [a0, a1])| within(b, b0, b1) && within(a, a0, a1))
        .map_or(0, |i| i as u8 + 1)
}

#[derive(Clone, Copy)]
struct BrakeLineState {
    /// Brake line the car is currently braking for.
    active: Option<usize>,
    target: f32,
    modifier: f32,
    hold: f32,
    /// Last brake line passed while active: (id, target, hold, elapsed).
    passed: Option<(usize, f32, f32, f32)>,
    /// Request's brake line on the previous update.
    request: Option<usize>,
}

/// Diagnostics of the last update.
#[derive(Clone, Copy, Debug, Default)]
pub struct Telemetry {
    pub arc: f32,
    pub look_ahead: f32,
    pub curvature: f32,
    pub heading_error: f32,
    pub line_distance: f32,
    pub line_target: f32,
    pub governor_target: f32,
    pub offset: f32,
    pub behaviour: Behaviour,
    pub overtake_state: u8,
    pub following: bool,
    /// Signed distance of the car from the racing line (positive right).
    pub line_error: f32,
    pub slip: f32,
    /// Tests: spun, on_track, collided_obstruction (frontal), contacted
    /// vehicle type, avoid ETA (f32::MAX when not fired).
    pub spun: bool,
    pub on_track: bool,
    pub obstruction: bool,
    pub frontal: bool,
    pub contact: u8,
    pub avoid_eta: f32,
    /// avoid side mode, recovery mode and octant, reverse request.
    pub avoid_side: u8,
    pub recovery_mode: u8,
    pub octant: u8,
    pub reverse: bool,
    /// Signed lateral offset of our front from the racing line (0x40AE90).
    pub front_lateral: f32,
    /// Drift test fired, drift behaviour state, drift mode, drift line.
    pub drift_fired: bool,
    pub drift_state: u8,
    pub drift_mode: bool,
    pub drift_line: bool,
    /// Room to the left/right track limits at the look-ahead (drift drive).
    pub drift_room: [f32; 2],
}

#[derive(Clone)]
pub struct AiDriver {
    pub parameters: Parameters,
    seed: u32,
    random: u32,
    segment: Option<usize>,
    /// Racing-line segment of each other car's last projection (by id).
    other_segments: Vec<(u32, usize)>,
    look_ahead: f32,
    curvature: f32,
    offset: f32,
    /// Planned offset at the car's own link (view +0x1A0, 0x788030): it moves
    /// toward the same request at the same rate as `offset` (+0xE0) but stops
    /// at the limits at the car. The PID error is measured from it.
    offset_here: f32,
    desired_offset: f32,
    offset_timer: f32,
    alone: bool,
    /// Lateral PID state (0x7790B0 object +0x20..+0x2C): error, previous
    /// error, integral. Zeroed while the PID is off.
    pid_state: [f32; 3],
    behaviour: Behaviour,
    overtake: Overtake,
    race_start: RaceStart,
    /// The first update has run (race_start activates on it).
    started: bool,
    /// Grid revving (0x775BF0): (state, on phase, timer).
    rev: (u8, bool, f32),
    attempt_target: Option<u32>,
    alongside_fired: bool,
    avoid: Avoid,
    spun: bool,
    recovery: Recovery,
    /// recover_tbone timer (+0x34).
    tbone: f32,
    /// Retire stuck timer: reference position, time and latched flag (0x77E3B0).
    stuck: Option<([f32; 3], f32, bool)>,
    lines: BrakeLineState,
    /// Published (slewed) packet: throttle, brake, steering.
    throttle: f32,
    brake: f32,
    steering: f32,
    pub telemetry: Telemetry,
    /// Race-manager inputs, set before each update.
    pub pace: Pace,
    mistake: Mistake,
    failure: Failure,
    /// Request grip multipliers of the last update (read by the grip
    /// application 0x408710).
    grip: [f32; 4],
    /// Packet reverse byte +0x1E of the last update.
    reverse: bool,
    /// Race place, set before each update (drift events).
    pub race: RaceView,
    drift: DriftState,
    /// The racing line the car follows.
    racing_line: RacingLine,
}

fn clamp01(x: f32) -> f32 {
    x.clamp(0., 1.)
}
/// 0x782D20 as 0x7878F0 and 0x788030 use it: a positive offset stops at the
/// right limit and a negative one at the left, each moved inward by our half
/// width (view +0x60 = car state +0x110). When that edge lies past the racing
/// line, on the other side, the offset is set to the line (0x782D20 clears
/// its signed limit distance before 0x779CB0 writes the offset).
fn stop_at_limits(offset: f32, limits: [f32; 2], half_width: f32) -> f32 {
    if offset > 0. {
        let edge = limits[1] - half_width;
        if 0. < edge && edge < offset {
            return edge;
        }
        if edge < 0. {
            return 0.;
        }
    } else if offset < 0. {
        let edge = limits[0] + half_width;
        if offset < edge && edge < 0. {
            return edge;
        }
        if edge > 0. {
            return 0.;
        }
    }
    offset
}

/// The airborne steering angle of 0x77AC50 in the +E30 convention: the angle
/// between the horizontal velocity and the horizontal forward axis, positive
/// when the velocity has a component along the car's left axis `left` (car
/// state +0x50; the original's internal sign is the opposite and its publisher
/// negates it). A zero horizontal velocity gives 0 [I].
fn air_slip(velocity: [f32; 4], forward: [f32; 3], left: [f32; 3]) -> f32 {
    let v = [velocity[0], 0., velocity[2]];
    let n = [forward[0], 0., forward[2]];
    let (lv, ln) = (dot3(v, v).sqrt(), dot3(n, n).sqrt());
    if !(lv > 0. && ln > 0.) {
        return 0.;
    }
    let a = (dot3(v, n) / (lv * ln)).clamp(-1., 1.).acos();
    if dot3(v, left) > 0. { a } else { -a }
}

fn approach(current: f32, target: f32, up: f32, down: f32, dt: f32) -> f32 {
    if current <= target {
        (current + dt * up).min(target)
    } else {
        (current - dt * down).max(target)
    }
}
/// AI traction control (0x77AA10), a throttle limit: between tcMinSpeed and
/// tcMaxSpeed, the fastest driven wheel's rolling speed against the car's
/// speed pulls the throttle from 1 towards a level that runs from tcMinLevel
/// to tcMaxLevel with speed, reached at tcMaxWheelSpin.
fn traction_control(tc: &[f32; 5], speed: f32, wheel_speeds: &[f32; 4], driven: &[bool; 4]) -> f32 {
    let [min_speed, max_speed, min_level, max_level, max_spin] = *tc;
    if speed <= min_speed || max_speed <= speed {
        return 1.;
    }
    let fastest = wheel_speeds
        .iter()
        .zip(driven)
        .filter(|&(_, &driven)| driven)
        .fold(0f32, |m, (&w, _)| if m < w { w } else { m });
    if fastest <= speed {
        return 1.;
    }
    let t = if min_speed == max_speed {
        0.
    } else {
        (speed - min_speed) / (max_speed - min_speed)
    };
    let level = ((max_level - min_level) * t + min_level)
        .clamp(min_level.min(max_level), min_level.max(max_level));
    let spin = if max_spin == 1. {
        0.
    } else {
        (fastest / speed - 1.) / (max_spin - 1.)
    };
    let limit = (level - 1.) * spin + 1.;
    if level <= 1. {
        limit.clamp(level, 1.)
    } else {
        limit.clamp(1., level)
    }
}

/// AI ABS cap (0x77A2E0), a brake limit: above absMinSpeed, the slowest
/// wheel's rolling speed against the car's speed pulls the brake from 1 at
/// absMinWheelLock towards absMinBrake at absMaxWheelLock.
fn abs_cap(abs: &[f32; 4], speed: f32, wheel_speeds: &[f32; 4]) -> f32 {
    let [min_speed, min_lock, max_lock, min_brake] = *abs;
    if speed <= min_speed {
        return 1.;
    }
    let slowest = wheel_speeds
        .iter()
        .fold(speed, |m, &w| if w < m { w } else { m });
    if slowest >= speed {
        return 1.;
    }
    let lock = if min_lock == max_lock {
        0.
    } else {
        (slowest / speed - min_lock) / (max_lock - min_lock)
    };
    let cap = (min_brake - 1.) * lock + 1.;
    if min_brake <= 1. {
        cap.clamp(min_brake, 1.)
    } else {
        cap.clamp(1., min_brake)
    }
}

/// Linear remap of `x` over `[x0, x1]` onto `[y0, y1]`, clamped to the output range.
fn remap(x: f32, x0: f32, x1: f32, y0: f32, y1: f32) -> f32 {
    let t = if x1 == x0 { 0. } else { (x - x0) / (x1 - x0) };
    let y = y0 + (y1 - y0) * t;
    y.clamp(y0.min(y1), y0.max(y1))
}

impl AiDriver {
    pub fn new(parameters: Parameters, seed: u32) -> Self {
        let grip = [parameters.faults.racing_grip; 4];
        Self {
            look_ahead: parameters.la[0],
            parameters,
            seed,
            random: seed,
            segment: None,
            other_segments: Vec::new(),
            curvature: 0.,
            offset: 0.,
            offset_here: 0.,
            desired_offset: 0.,
            offset_timer: 0.,
            alone: false,
            pid_state: [0.; 3],
            behaviour: Behaviour::Race,
            overtake: Overtake::default(),
            race_start: RaceStart::default(),
            started: false,
            rev: (0, false, 0.),
            attempt_target: None,
            alongside_fired: false,
            avoid: Avoid::default(),
            spun: false,
            recovery: Recovery::default(),
            tbone: -1.,
            stuck: None,
            lines: BrakeLineState {
                active: None,
                target: 0.,
                modifier: 1.,
                hold: -1.,
                passed: None,
                request: None,
            },
            throttle: 0.,
            brake: 0.,
            steering: 0.,
            telemetry: Telemetry::default(),
            pace: Pace::default(),
            mistake: Mistake::default(),
            failure: Failure::default(),
            grip,
            reverse: false,
            race: RaceView::default(),
            drift: DriftState::default(),
            racing_line: RacingLine::Optimal,
        }
    }

    pub fn reset(&mut self) {
        *self = Self::new(self.parameters.clone(), self.seed);
    }

    /// Authored uniform random in [-1, 1] (original RNG stream not identified).
    fn uniform(&mut self) -> f32 {
        self.random = self
            .random
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        (self.random >> 8) as f32 / (1u32 << 24) as f32 * 2. - 1.
    }

    /// Throttle while held on the grid (0x775BF0): random on/off pulses.
    /// State 3: on 0.20..0.48 s, off 0.01..0.70 s; state 2: on 0.08..0.18 s,
    /// off 0.60..1.15 s (.rdata constants); other states give 0. Which start
    /// state applies when is not recovered; the random stream is authored.
    pub fn grid_rev(&mut self, state: u8, dt: f32) -> f32 {
        let ranges = match state {
            3 => [[0.2, 0.48], [0.01, 0.7]],
            2 => [[0.08, 0.18], [0.6, 1.15]],
            _ => {
                self.rev = (state, false, 0.);
                return 0.;
            }
        };
        let between =
            |driver: &mut Self, [a, b]: [f32; 2]| a + (b - a) * (driver.uniform() * 0.5 + 0.5);
        if self.rev.0 != state {
            let timer = between(self, ranges[0]);
            self.rev = (state, true, timer);
        }
        self.rev.2 -= dt;
        if self.rev.2 > 0. {
            return if self.rev.1 { 1. } else { 0. };
        }
        let (next_on, range) = if self.rev.1 {
            (false, ranges[1])
        } else {
            (true, ranges[0])
        };
        let timer = between(self, range);
        self.rev = (state, next_on, timer);
        0.
    }

    /// Look-ahead distance update (0x779A20) from speed and the previous curvature.
    fn update_look_ahead(&mut self, speed: f32) {
        let [
            dist_min,
            dist_max,
            angle_min,
            angle_max,
            speed_min,
            speed_max,
            step_max,
            proportion,
            extra_dist,
            extra_speed,
        ] = self.parameters.la;
        let target = if speed_min <= speed {
            let a = clamp01(if angle_max == angle_min {
                0.
            } else {
                (self.curvature.abs() - angle_max) / (angle_min - angle_max)
            });
            let s = clamp01(if speed_max == speed_min {
                0.
            } else {
                (speed - speed_min) / (speed_max - speed_min)
            });
            (dist_min + s * a * a * (dist_max - dist_min))
                .clamp(dist_min.min(dist_max), dist_min.max(dist_max))
        } else {
            let t = if extra_speed == speed_min {
                0.
            } else {
                (speed - extra_speed) / (speed_min - extra_speed)
            };
            (extra_dist + (dist_min - extra_dist) * t)
                .clamp(dist_min.min(extra_dist), dist_min.max(extra_dist))
        };
        let step = ((target - self.look_ahead).abs() * proportion).clamp(0., step_max);
        self.look_ahead = if target > self.look_ahead {
            (self.look_ahead + step).min(target)
        } else {
            (self.look_ahead - step).max(target)
        };
        self.look_ahead = self.look_ahead.clamp(dist_min.min(extra_dist), dist_max);
    }

    /// One AI update; `others` are the positions of the other cars.
    pub fn controls(&mut self, data: &AiData, view: &View, others: &[Other], dt: f32) -> Controls {
        let p = self.parameters.clone();
        let position = [view.pose[12], view.pose[13], view.pose[14]];
        let right = [view.pose[0], view.pose[1], view.pose[2]];
        let up = [view.pose[4], view.pose[5], view.pose[6]];
        let forward = [view.pose[8], view.pose[9], view.pose[10]];
        let v = view.velocity;
        let speed = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        let slip = if speed > 0.5 {
            wrap(heading([v[0], 0., v[2]]) - heading(forward)).abs()
        } else {
            0.
        };

        // Line choice (0x794870): before the start line (race progress
        // negative) the `start` kind, after it the `drift` kind when the drift
        // behaviour is switched on, else `optimal`. A kind the route lacks
        // falls back to `optimal`. The context flag param_4+0x1C that also
        // selects the after-start kinds is unidentified [I] and taken as clear.
        let kind = if self.race.progress < 0. {
            RacingLine::Start
        } else if p.drift.is_some() {
            RacingLine::Drift
        } else {
            RacingLine::Optimal
        };
        let kind = if std::ptr::eq(data.on(kind), data) {
            RacingLine::Optimal
        } else {
            kind
        };
        if kind != self.racing_line {
            // Keep the planned path in place across the change [I]: shift
            // the offsets by the two lines' lateral distance at the car.
            let (_, _, from) = data.on(self.racing_line).line.project(position, self.segment);
            let (_, _, to) = data.on(kind).line.project(position, self.segment);
            self.offset += to - from;
            self.offset_here += to - from;
            self.desired_offset += to - from;
            self.racing_line = kind;
        }
        let data = data.on(kind);

        // Track view: progress along the racing line (0x79C360).
        let (segment, arc, line_distance) = data.line.project(position, self.segment);
        self.segment = Some(segment);
        // Racing limits at the car and at the look-ahead sample (the +0x150
        // and +0x90 blocks of 0x798E90, before this tick's look-ahead update).
        // The rooms to them are view +0x220/+0x224 and +0x230/+0x234.
        let (_, _, _, limits_here) = data.line.at(arc);
        let (_, _, _, limits_ahead) = data.line.at(arc + self.look_ahead);
        let tr = p.traffic.clone();
        let own = Other::from_pose(0, view.pose, view.velocity, view.extents);
        // Observable list (0x7ACA60): the tests below see only the selected cars.
        let along: Vec<f32> = others
            .iter()
            .map(|o| {
                let hint = self
                    .other_segments
                    .iter()
                    .find(|x| x.0 == o.id)
                    .map(|x| x.1);
                let (i, s, _) = data.line.project(o.position, hint);
                match self.other_segments.iter_mut().find(|x| x.0 == o.id) {
                    Some(x) => x.1 = i,
                    None => self.other_segments.push((o.id, i)),
                }
                data.line.along(arc, s)
            })
            .collect();
        let observed = observable(&own, others, &along);
        let others = &observed[..];
        let f = horizontal(forward);
        // Car-right in the original's sense is our -X (car left is +X).
        let right_vec = [-right[0], -right[1], -right[2]];
        let (own_front, own_rear) = (own.front(), own.rear());

        // brake_for_vehicle (0x78F6F0) on the path obstacle of 0x78B560.
        let mut pass_vehicle = None;
        let mut brake_follow: Option<(Other, f32)> = None;
        let cone = (tr.view_angle.to_radians())
            .clamp(0., std::f32::consts::PI)
            .cos();
        // Path-obstacle scan 0x78B560, before the tests.
        let scan = path_obstacle_scan(
                &own,
                speed,
                others,
                &ScanConfig {
                    stopped_speed: p.rescue.av_stopped_speed,
                    buffer_zone: p.rescue.av_buffer,
                    buffer_base_speed: p.rescue.av_buffer_speed,
                    similar_angle: tr.similar_angle,
                    contact_zone: p.rescue.contact_zone,
                },
            self.reverse,
        );
        // brake_for_vehicle takes the path obstacle when one of its corners
        // is inside the view cone seen from our rear point (0x7A2F30).
        let scan_target = scan.best.and_then(|e| {
            let r = &others[e.other];
            box_corners(r, 0.)
                .iter()
                .any(|&c| dot3(horizontal(sub(c, own_rear)), f) > cone)
                .then_some((*r, e.distance))
        });
        let candidate = scan_target.map(|(r, _)| r);
        if let Some(r) = candidate {
            let r = &r;
            let w = own.half_width() + r.half_width();
            let lat = dot3(sub(r.position, position), right_vec);
            let (gap_r, gap_l) = (lat - w, -lat - w);
            let same = dot3(f, horizontal(r.forward));
            let closing = speed - r.speed();
            if same > tr.pass_angle.cos().abs() && (gap_r > tr.buffer || gap_l > tr.buffer) {
                let allowed = remap(
                    gap_r.max(gap_l) - tr.buffer,
                    0.,
                    tr.pass_spare_room,
                    tr.pass_side_speed[0],
                    tr.pass_side_speed[1],
                );
                let side_speed = dot3(
                    std::array::from_fn(|k| r.velocity[k] - view.velocity[k]),
                    right_vec,
                )
                .abs();
                if r.speed() < tr.pass_slow_speed
                    || side_speed < remap(closing, 0., tr.pass_relative_speed, allowed, 0.)
                {
                    pass_vehicle = Some(*r);
                }
            }
            if pass_vehicle.is_none() {
                let similar = same > tr.similar_angle.cos().abs();
                let dist = if similar {
                    dot3(sub(r.rear(), own_front), f)
                } else {
                    // [I] The path-relative distance from the obstacle's
                    // nearest corner is the scan's sweep distance.
                    scan_target.map_or(0., |(_, distance)| distance)
                };
                let sep = remap(
                    closing,
                    0.,
                    tr.separation_speed,
                    tr.separation[0],
                    tr.separation[1],
                );
                let sep2 = remap(speed, 0., tr.slow_speed, 0., sep);
                let available = (dist.abs() - sep2).max(0.);
                let multiplier = remap(
                    speed,
                    0.,
                    tr.slow_speed,
                    1.,
                    tr.extra_prediction + tr.prediction_ratio,
                );
                let safe = closing <= 0.
                    || available / closing * speed
                        > (speed / p.brake_on_speed.max(1e-3) * tr.pedal_push
                            + data.braking_distance(
                                TABLE_GRIP,
                                speed,
                                (speed - closing).clamp(0., speed),
                            ))
                            * multiplier;
                if !safe {
                    let distance = if similar && closing < tr.cruise_speed {
                        sep2
                    } else {
                        100.
                    };
                    brake_follow = Some((*r, distance));
                }
            }
        }

        // attempt_overtake (0x77DB90): nearest qualifying car ahead.
        let mut best = tr.ov_reject;
        let mut attempt = None;
        for r in others {
            let d = sub(r.rear(), own_front);
            let along = dot3(d, f);
            let lat = dot3(d, horizontal(r.left)).abs();
            let closing = speed - r.speed();
            if !(0. < along && along < best && lat < tr.ov_side_reject) {
                continue;
            }
            let ok = if Some(r.id) == self.attempt_target {
                closing > tr.ov_continue
            } else {
                along < tr.ov_auto_accept
                    || (closing > tr.ov_speed_difference && along / closing < tr.ov_time)
                    || (closing <= tr.ov_speed_difference
                        && along < tr.ov_slow_reject
                        && r.speed() < tr.ov_slow_speed)
            };
            if ok {
                best = along;
                attempt = Some(*r);
            }
        }
        self.attempt_target = attempt.map(|r| r.id);

        // alongside (0x795C00): overlapping cars within asRejectDistance.
        let margin = usize::from(self.alongside_fired);
        let (front_m, rear_m) = (tr.front_projection[margin], tr.rear_projection[margin]);
        let mut slots: [Option<(f32, bool, Other)>; 2] = [None, None];
        let mut fired = false;
        for r in others {
            if dot3(sub(r.position, position), sub(r.position, position)).sqrt() > tr.as_reject {
                continue;
            }
            let front_point: [f32; 3] = std::array::from_fn(|k| own_front[k] + f[k] * front_m);
            let rear_point: [f32; 3] = std::array::from_fn(|k| own_rear[k] - f[k] * rear_m);
            let beside = dot3(sub(r.rear(), front_point), f) < 0.
                && dot3(sub(r.front(), rear_point), f) > 0.;
            if !beside {
                continue;
            }
            fired = true;
            let strict =
                dot3(sub(r.rear(), own_front), f) < 0. && dot3(sub(r.front(), own_rear), f) > 0.;
            let lat = dot3(sub(r.rear(), own_front), horizontal(right_vec));
            let gap = (lat.abs() - own.half_width() - r.half_width()).max(0.);
            let slot = usize::from(lat > 0.);
            let replace = match slots[slot] {
                None => true,
                Some((g, s, _)) => (strict || !s) && gap < g,
            };
            if replace {
                slots[slot] = Some((gap, strict, *r));
            }
        }
        self.alongside_fired = fired;
        // 0x795C00 tail: an empty side (gap FLT_MAX) gets gap 0 when our side
        // edge (centre ± half width along the car's left axis +0x50) lies
        // beyond that side's racing-limit line through the current link's
        // gates (0x793790 types 1/2 at view links +0x24 and +0x20). The order
        // of the two gates is [I]: start then end makes the test an outside test.
        let mut gaps = slots.map(|s| s.map_or(f32::MAX, |s| s.0));
        if gaps.iter().any(|&g| g > ALONGSIDE_EMPTY_GAP)
            && let Some((i, j)) = data.line.window_link(segment, 1)
        {
            for (k, gap) in gaps.iter_mut().enumerate() {
                if *gap <= ALONGSIDE_EMPTY_GAP {
                    continue;
                }
                let (a, b) = (data.line.racing_limit_point(i, k), data.line.racing_limit_point(j, k));
                let side = if k == 0 { -own.half_width() } else { own.half_width() };
                let edge: [f32; 3] = std::array::from_fn(|c| position[c] + right_vec[c] * side);
                let cross = (b[0] - a[0]) * (edge[2] - b[2]) - (b[2] - a[2]) * (edge[0] - b[0]);
                if (k == 0 && cross < 0.) || (k == 1 && cross > 0.) {
                    *gap = 0.;
                }
            }
        }

        // Track view at the start of the tick (0x798E90, previous planned
        // offset and look-ahead): planned look-ahead point (+0xD0), racing-line
        // point and lateral at the car (+0x170/+0x180) and the lateral position
        // of our front (0x40AE90).
        let rs = p.rescue.clone();
        let (here_point, here_lateral) = data.line.curve(arc);
        let (la_point, la_lateral) = data.line.curve(arc + self.look_ahead);
        let planned: [f32; 3] = std::array::from_fn(|k| la_point[k] + la_lateral[k] * self.offset);
        // Angles from our forward, positive clockwise seen from above (toward
        // the car's right). The original's sign convention is not confirmed;
        // this one makes the recovery octants reverse away from the near edge [I].
        let clockwise = |d: [f32; 3]| wrap(heading(forward) - heading(d));
        let view_heading = clockwise(sub(planned, own_front));
        let own_lateral = dot2(sub(own_front, here_point), here_lateral);

        // spun (0x77E5B0) with its hysteresis.
        let moving_back = dot3([v[0], v[1], v[2]], forward) < 0.;
        let held_slip = if moving_back {
            std::f32::consts::PI - slip
        } else {
            slip
        };
        self.spun = view_heading.abs() > std::f32::consts::FRAC_PI_2
            || (self.spun
                && (held_slip.abs() > SPUN_HOLD_SLIP
                    || view_heading.abs() > SPUN_HOLD_HEADING
                    || speed < SPUN_HOLD_SPEED));
        // on_track (0x79B250): more than half the wheels inside the track limits.
        let inside = view
            .wheels
            .iter()
            .filter(|&&w| {
                let (_, s, lat) = data.line.project(w, Some(segment));
                let [left_limit, right_limit] = data.line.track_limits_at(s);
                left_limit <= lat && lat <= right_limit
            })
            .count();
        let on_track = 2 * inside > view.wheels.len();
        // collided_obstruction (0x776FE0): frontal when the summed normal is
        // more than 120 degrees from our nose.
        let o = view.obstruction;
        let o_length = dot3(o, o).sqrt();
        let obstruction = o_length > 0.;
        let o_direction = if o_length < 1e-5 {
            o
        } else {
            o.map(|x| x / o_length)
        };
        let frontal = obstruction && dot3(forward, o_direction) < -0.5;
        // contacted_vehicle (0x784650): first car whose box touches ours.
        let contact = scan.contacts.first().map(|&(i, t)| (others[i], t));
        // recover_tbone score (0x78ACC0): front contact at a T-bone, head-on or
        // slow nose-to-tail angle holds the score for 1 s.
        if let Some((r, kind)) = contact
            && matches!(kind, 1 | 2 | 5)
        {
            let theta = dot3(f, horizontal(r.forward))
                .clamp(-1., 1.)
                .acos()
                .to_degrees();
            let relative: [f32; 3] = std::array::from_fn(|k| v[k] - r.velocity[k]);
            let relative = dot3(relative, relative).sqrt();
            if (70. < theta && theta < 110. && relative < 5.)
                || (theta < 30. && relative < 5. && r.speed() < 5.)
                || theta > 160.
            {
                self.tbone = TBONE_TIME;
            }
        }
        // recovery score (0x78AB20).
        let n = usize::from(self.spun)
            + usize::from(!on_track)
            + usize::from(obstruction && !on_track)
            + usize::from(frontal);
        let recovery_score = if n == 0 { 0 } else { RECOVERY_SCORES[n - 1] };
        // retire stuck timer (0x77E3B0): 20 s within 5 m and no racing car
        // within 100 m.
        let moved = self.stuck.is_none_or(|(reference, _, _)| {
            let d = sub(position, reference);
            dot3(d, d) >= rs.rt_stuck_distance_sq
        });
        if moved {
            self.stuck = Some((position, 0., false));
        } else if let Some((_, time, latched)) = &mut self.stuck {
            *time += dt;
            *latched = *time > rs.rt_stuck_time
                && !others.iter().any(|o| {
                    let d = sub(o.position, position);
                    !o.backwards() && dot3(d, d) < rs.rt_player_distance_sq
                });
        }
        let stuck = self.stuck.is_some_and(|s| s.2);
        let faults = self.fault_scores(speed);

        // Path obstacle (0x78B560): the kept obstacle with its sweep distance
        // and closing speed.
        let obstacle: Option<(Other, f32, f32)> = scan
            .best
            .map(|e| (others[e.other], e.distance, dot3(e.relative, e.relative).sqrt()));
        // avoid test (0x79A960): (target, ETA, closing speed).
        let mut avoid_fire: Option<(Other, f32, f32)> = None;
        if let Some((r, distance, closing)) = obstacle {
            // Racing-normally exclusion 0x79A530; the original's angle test
            // compares with the reversed track direction (§2.2).
            let (_, r_arc, _) = data.line.project(r.position, None);
            let (_, r_direction, _, _) = data.line.at(r_arc);
            let wrong_way = r_direction.map(|x| -x);
            let racing = r.speed() >= rs.av_min_racing_speed
                && !r.backwards()
                && dot2(f, horizontal(r.forward)) >= 0.
                && dot2(horizontal(r.forward), wrong_way).clamp(-1., 1.).acos()
                    > rs.av_max_racing_angle;
            let inert = closing <= rs.av_min_speed_normal
                && (rs.braking_buffer <= distance || closing >= rs.av_min_speed_slow);
            if !racing && !inert {
                if distance / closing < rs.av_eta {
                    avoid_fire = Some((r, distance / closing, closing));
                    self.avoid.remembered = Some(r.id);
                } else {
                    self.avoid.remembered = None;
                }
            }
        } else if let Some(r) = self
            .avoid
            .remembered
            .and_then(|id| others.iter().find(|o| o.id == id))
        {
            let d = [r.position[0] - position[0], 0., r.position[2] - position[2]];
            let along = dot3(d, f);
            let relative = [v[0] - r.velocity[0], 0., v[2] - r.velocity[2]];
            let closing = dot3(relative, relative).sqrt();
            if along > own.half_length() + r.half_length()
                && cone < dot3(f, horizontal(d))
                && along < rs.av_eta * closing
            {
                avoid_fire = Some((*r, along.abs() / closing, closing));
            } else {
                self.avoid.remembered = None;
            }
        } else {
            self.avoid.remembered = None;
        }

        // cornering (0x77ED00) and drift (0x78FE90) tests; drift score 0x78B360.
        let cornering = self.curvature.abs() > p.cornering_curvature;
        let drift_score = match &p.drift {
            Some(_) => {
                self.drift_test(data, speed, dt);
                let mut score = 0;
                if self.drift.fired {
                    score = DRIFT_SCORES[if cornering { 1 } else { 0 }];
                }
                if self.drift.state != 1 {
                    score = DRIFT_SCORES[2];
                }
                score
            }
            None => 0,
        };

        // race_start activation (0x78A8F0). The original scores 22 while the
        // wait_for_start test fires on the grid and activates there; the
        // port's runners hold the cars without updating the AI, so it
        // activates on the first update instead [I]. The launch delay
        // (+0x30: rsMinimumWaitAtLights + U·max_lights_reaction_time, or a
        // stall) is 0 with the shipped data and is not ported.
        if !self.started {
            self.started = true;
            if p.race_start {
                // Side choice 0x776740: away from the nearest car if it is more
                // than 2 m to one side, else toward the larger racing-limit room.
                let left_axis = [view.pose[0], view.pose[1], view.pose[2]];
                let nearest = others.iter().min_by(|a, b| {
                    let da = dot3(sub(a.position, position), sub(a.position, position));
                    let db = dot3(sub(b.position, position), sub(b.position, position));
                    da.total_cmp(&db)
                });
                let right = match nearest
                    .map(|o| dot3(sub(o.position, position), left_axis))
                    .filter(|side| side.abs() > RACE_START_SIDE_DISTANCE)
                {
                    Some(side) => side > 0.,
                    None => self.offset - limits_ahead[0] > limits_ahead[1] - self.offset,
                };
                let [end_1, end_2, merge_1, merge_2, timeout] = p.rs;
                let mut between = |a: f32, b: f32| a + (b - a) * (self.uniform() * 0.5 + 0.5);
                let end_speed = between(end_1, end_2);
                let merge = between(merge_1, merge_2);
                self.race_start = RaceStart {
                    active: true,
                    end_speed,
                    merge,
                    offset: 0.,
                    initial: true,
                    timer: timeout,
                    right,
                };
            }
        }
        // Arbitration (0x79A400): the first strictly greatest score wins.
        let overtake_target = pass_vehicle.or(attempt);
        let scores = [
            (
                Behaviour::Retire,
                if stuck { RETIRE_STUCK_SCORE } else { 0 },
            ),
            (
                Behaviour::RecoverTbone,
                if self.tbone > 0. {
                    RECOVER_TBONE_SCORE
                } else {
                    0
                },
            ),
            (Behaviour::LostControl, faults[0]),
            (
                Behaviour::ControlledStop,
                if p.controlled_stop { faults[1] } else { 0 },
            ),
            (Behaviour::Recovery, recovery_score),
            (
                Behaviour::Avoid,
                if avoid_fire.is_some() { AVOID_SCORE } else { 0 },
            ),
            (Behaviour::Drift, drift_score),
            (Behaviour::RetireFailure, faults[2]),
            (
                Behaviour::RaceDamaged,
                if p.race_damaged { faults[3] } else { 0 },
            ),
            (Behaviour::Alongside, if fired { 8 } else { 0 }),
            (
                Behaviour::Overtake,
                if overtake_target.is_some() || self.overtake.min_fire > 0. {
                    5
                } else {
                    0
                },
            ),
            (
                Behaviour::RaceStart,
                if self.race_start.active {
                    RACE_START_SCORE
                } else {
                    0
                },
            ),
            (Behaviour::Race, 1),
        ];
        let next = scores
            .iter()
            .fold(
                (Behaviour::Race, 0),
                |best, &s| if s.1 > best.1 { s } else { best },
            )
            .0;
        if next != self.behaviour {
            match self.behaviour {
                Behaviour::Overtake => self.overtake.min_fire = 0.,
                Behaviour::Recovery => self.recovery.elapsed = 0.,
                Behaviour::RaceStart => self.race_start.active = false,
                _ => {}
            }
            match next {
                Behaviour::Overtake => {
                    self.overtake = Overtake {
                        min_fire: tr.ov_min_fire,
                        ..Overtake::default()
                    }
                }
                // Race reactivation: a fresh random offset at the next update [I].
                Behaviour::Race => self.offset_timer = p.ra_generate_time,
                Behaviour::Avoid => {
                    self.avoid = Avoid {
                        remembered: self.avoid.remembered,
                        ..Avoid::default()
                    }
                }
                Behaviour::Recovery => {
                    self.recovery = Recovery {
                        toggle: self.recovery.toggle,
                        timer: -1.,
                        ..Recovery::default()
                    }
                }
                // Activation runs after scoring: the first tick does nothing.
                Behaviour::RecoverTbone => self.tbone = -1.,
                Behaviour::LostControl | Behaviour::RaceDamaged | Behaviour::RetireFailure => {
                    self.activate_failure(next, speed)
                }
                _ => {}
            }
            self.behaviour = next;
        }
        let mut octant_index = 0;

        // Common setup (0x789700) and the race behaviour's offsets.
        let nearest = others
            .iter()
            .map(|o| dot3(sub(o.position, position), sub(o.position, position)).sqrt())
            .fold(f32::INFINITY, f32::min);
        if self.alone && nearest < p.ra_alone_off {
            self.alone = false;
        } else if !self.alone && nearest > p.ra_alone_on {
            self.alone = true;
        }
        // Common setup 0x7978D0: request reset, merge tgOffsetMergeFactor.
        let mut request = Request {
            desired_offset: self.offset,
            merge: rs.tg_merge,
            desired_speed: f32::MAX,
            throttle_cap: 1.,
            follow: brake_follow,
            follow_brake_cap: 1.,
            brake_to_speed: false,
            force_brake: false,
            no_steer: false,
            full_brake: false,
            reverse: false,
            track_limits: false,
            grip: [p.faults.racing_grip; 4],
            brake_cap: 1.,
        };
        let corner =
            clamp01((self.curvature.abs() - tr.ov_corner[0]) / (tr.ov_corner[1] - tr.ov_corner[0]));
        // Request +0xC8: drift mode (set by the drift drive).
        let mut drift_mode = false;
        let mut drift_room = [0f32; 2];
        match self.behaviour {
            Behaviour::Drift => {
                let ([left, right], length) = (
                    data.line.track_limits_at(arc + self.look_ahead),
                    view.extents.front - view.extents.rear,
                );
                drift_room = [self.offset - left, right - self.offset];
                drift_mode = self.drive_drift(
                    data,
                    &mut request,
                    arc,
                    speed,
                    slip,
                    cornering,
                    // Room from the planned path at the look-ahead sample to
                    // the left/right track limits (view +0x218/+0x21C).
                    [self.offset - left, right - self.offset],
                    [view.extents.half_width, length],
                );
            }
            Behaviour::Race => {
                self.offset_timer += dt;
                if p.ra_generate && self.offset_timer >= p.ra_generate_time {
                    self.offset_timer = 0.;
                    let deviation = if self.alone {
                        p.deviation_small
                    } else {
                        p.deviation_large
                    };
                    self.desired_offset = self.uniform() * deviation;
                }
                request.desired_offset = self.desired_offset;
                request.merge = if self.alone {
                    p.ra_merge_alone
                } else if self.curvature.abs() > p.cornering_curvature {
                    p.ra_merge_cornering
                } else {
                    p.ra_merge
                };
                request.throttle_cap = p.throttle_confidence;
                if self.let_pass(others, own_rear, f) {
                    request.throttle_cap = p.throttle_confidence * p.faults.let_pass_throttle;
                }
            }
            Behaviour::RaceStart => {
                // Drive 0x78A740. Below slowSpeed the request offset is our
                // front's own (0x7765B0) with an instant merge (applied below).
                let rs = &mut self.race_start;
                if speed >= SLOW_SPEED {
                    request.merge = rs.merge;
                    if rs.initial {
                        // Room from the planned path at the look-ahead sample
                        // to the chosen side's racing limit (view +0x230/+0x234,
                        // the +0x90 block; 0x798E90, 0x797190).
                        let room = if rs.right {
                            limits_ahead[1] - self.offset
                        } else {
                            self.offset - limits_ahead[0]
                        };
                        if room <= own.half_width() {
                            rs.offset = 0.;
                        } else {
                            rs.initial = false;
                            let (a, b) = if rs.right {
                                (self.offset, self.offset + RACE_START_LANE)
                            } else {
                                (self.offset - RACE_START_LANE, self.offset)
                            };
                            let u = self.uniform() * 0.5 + 0.5;
                            self.race_start.offset = a + (b - a) * u;
                        }
                    }
                    let rs = &mut self.race_start;
                    request.desired_offset = rs.offset;
                    rs.timer -= dt;
                    if rs.end_speed < speed || rs.timer < 0. {
                        rs.active = false;
                    }
                }
                // The initial phase plans within the track limits (types 3/4).
                if self.race_start.initial {
                    request.track_limits = true;
                }
            }
            Behaviour::Overtake => {
                // Drive 0x78A330: decide a side every ovDecisionTimeOut or on a new target.
                match overtake_target {
                    None => self.overtake.state = 5,
                    Some(target) => {
                        self.overtake.decision += dt;
                        if self.overtake.decision > tr.ov_decision
                            || self.overtake.target != Some(target.id)
                        {
                            self.overtake.target = Some(target.id);
                            self.overtake.decision = 0.;
                            // Side choice 0x77BD20 relative to our planned line.
                            let (_, target_arc, target_lat) =
                                data.line.project(target.position, None);
                            let _ = target_arc;
                            let s = target_lat - self.offset;
                            let dist = s.abs();
                            let w = own.half_width() + target.half_width();
                            let room_l = self.offset - limits_ahead[0] + s;
                            let room_r = limits_ahead[1] - self.offset - s;
                            let need = w + tr.ov_safe;
                            self.overtake.state = if dist > w + tr.ov_desired {
                                3
                            } else if dist > w + tr.ov_safe {
                                if s > 0. { 1 } else { 2 }
                            } else if self.overtake.state == 1 && room_l > need {
                                1
                            } else if self.overtake.state == 2 && room_r > need {
                                2
                            } else {
                                match (room_l > need, room_r > need) {
                                    (true, true) if s > own.half_width() => 1,
                                    (true, true) if s < -own.half_width() => 2,
                                    (true, true) => {
                                        if room_l > room_r {
                                            1
                                        } else {
                                            2
                                        }
                                    }
                                    (true, false) => 1,
                                    (false, true) => 2,
                                    (false, false) => 4,
                                }
                            };
                        }
                        // Execute 0x77C180.
                        match self.overtake.state {
                            1 | 2 => {
                                let toward_negative = self.overtake.state == 1;
                                request.desired_offset = self.offset
                                    + if toward_negative { -dt } else { dt } * tr.ov_merge;
                                // State 1 with bend flag +0x45 (κ > min), state 2
                                // with +0x44 (κ < −min). κ is positive for a bend
                                // toward positive offsets, so either move widens it.
                                let outside = if toward_negative {
                                    self.curvature > tr.ov_corner[0]
                                } else {
                                    self.curvature < -tr.ov_corner[0]
                                };
                                let c = if speed > tr.ov_slow_speed && outside {
                                    corner
                                } else {
                                    0.
                                };
                                request.merge = (tr.ov_merge
                                    + (tr.ov_merge_corner - tr.ov_merge) * c)
                                    .clamp(tr.ov_merge_corner, tr.ov_merge);
                            }
                            4 => {
                                request.desired_speed = target.speed();
                                if request.follow.is_none() {
                                    request.follow = Some((target, tr.ov_follow));
                                }
                            }
                            _ => {}
                        }
                    }
                }
                self.overtake.min_fire -= dt;
            }
            Behaviour::Alongside => {
                // Drive 0x799630. The sides come from the strict-overlap flags
                // (+0x24/+0x25), or from the slot cars when neither is strict.
                request.merge = tr.as_merge;
                let strict = slots.map(|s| s.is_some_and(|s| s.1));
                let [left, right] = if strict[0] || strict[1] {
                    strict
                } else {
                    slots.map(|s| s.is_some())
                };
                let cur = self.offset;
                request.desired_offset = if left == right {
                    // Cars on both sides: the mean of their laterals from the
                    // racing line on the view's link window (0x798C00 /
                    // 0x798D00); if either is not located, our centre's
                    // lateral at the car's line point (+0x170/+0x180) less the
                    // left gap and plus the right gap. 0 without both records
                    // or when the look-ahead point is the car's line point.
                    match slots {
                        [Some((_, _, a)), Some((_, _, b))] => {
                            let lateral = |r: Other| {
                                data.line
                                    .locate_in_window(segment, r.position, false)
                                    .map(|(_, l)| l)
                            };
                            match (lateral(a), lateral(b)) {
                                (Some(a), Some(b)) => (a + b) * 0.5,
                                _ if dot2(sub(la_point, here_point), sub(la_point, here_point))
                                    > 0. =>
                                {
                                    let own = dot2(sub(position, here_point), here_lateral);
                                    ((own - gaps[0]) + (gaps[1] + own)) * 0.5
                                }
                                _ => 0.,
                            }
                        }
                        _ => 0.,
                    }
                } else {
                    // One side, with g that car's gap: closer than minSpace,
                    // move away by the shortfall; else when the room on that
                    // car's side at the look-ahead (view +0x230/+0x234, from
                    // the +0x90 block), less the room there at the car
                    // (+0x220/+0x224, the +0x150 block; 0x798E90, 0x797190)
                    // plus asTargetBuffer minus g, is below minSpace, move
                    // away by minSpace ("squeezed", +0x30). Displaced toward
                    // that car with g above maxSpace: back to the racing line.
                    // The room at the car uses the car link's planned
                    // offset +0x1A0.
                    let here = self.offset_here;
                    let (g, room_here, room_ahead, away) = if left {
                        (gaps[0], here - limits_here[0], cur - limits_ahead[0], 1.)
                    } else {
                        (gaps[1], limits_here[1] - here, limits_ahead[1] - cur, -1.)
                    };
                    let mut desired = cur;
                    if g < tr.min_space {
                        desired += away * (tr.min_space - g);
                    } else if room_ahead - ((room_here + tr.as_target_buffer) - g) < tr.min_space {
                        desired += away * tr.min_space;
                    }
                    if desired * -away > 0. && g > tr.max_space {
                        0.
                    } else {
                        desired
                    }
                };
                // Yield 0x789780 / 0x77B7D0: follow a car beside us that is mostly
                // ahead when we cannot move away from it.
                let [
                    distance,
                    max_brake,
                    min_diff,
                    max_diff,
                    follow,
                    respect,
                    min_speed,
                    to_limit,
                ] = tr.yield_;
                if distance > 0. && speed > min_speed {
                    let yields = |slot: &Option<(f32, bool, Other)>| {
                        slot.filter(|&(g, _, r)| {
                            let diff = r.speed() - speed;
                            let respect_point: [f32; 3] = std::array::from_fn(|k| {
                                r.rear()[k] + (r.front()[k] - r.rear()[k]) * respect
                            });
                            g < distance
                                && min_diff < diff
                                && diff < max_diff
                                && dot3(sub(respect_point, own_front), f) > 0.
                        })
                    };
                    let (left, right) = (yields(&slots[0]), yields(&slots[1]));
                    let room_l = self.offset - limits_ahead[0] - own.half_width();
                    let room_r = limits_ahead[1] - self.offset - own.half_width();
                    let chosen = match (left, right) {
                        (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
                        (Some(a), None) if distance > gaps[1] || room_r < to_limit => {
                            Some(a)
                        }
                        (None, Some(b)) if distance > gaps[0] || room_l < to_limit => {
                            Some(b)
                        }
                        _ => None,
                    };
                    if let Some((_, _, r)) = chosen {
                        request.follow = Some((r, follow));
                        request.follow_brake_cap = max_brake;
                    }
                }
            }
            Behaviour::Avoid => {
                // Drive 0x79C600: decide every avDecisionTime or on a new target.
                self.avoid.decision -= dt;
                if let Some((r, eta, closing)) = avoid_fire
                    && (self.avoid.decision < 0. || self.avoid.target != Some(r.id))
                {
                    self.avoid.target = Some(r.id);
                    // Side choice 0x799B30 and offset 0x799DE0 around the
                    // target's predicted position.
                    let clearance =
                        r.half_length() + rs.av_buffer + own.half_width() + r.half_width();
                    let predicted: [f32; 3] =
                        std::array::from_fn(|k| r.position[k] + eta * r.velocity[k]);
                    // P on the track view's link window (0x798C00, or
                    // 0x78DCD0 with avFigureOfEightTrack); lateral 0 off it.
                    let (p_arc, lat) = data
                        .line
                        .locate_in_window(segment, predicted, rs.av_figure_of_eight)
                        .unwrap_or((arc, 0.));
                    let x = own_lateral;
                    self.avoid.side = if closing > speed {
                        1
                    } else {
                        let mut side = if (lat + clearance - x).abs() <= (lat - clearance - x).abs()
                        {
                            2
                        } else {
                            1
                        };
                        // Room check 0x797720 against the chosen track limit.
                        let w = 2. * own.half_width() + r.half_width() + 0.5;
                        let [left_limit, right_limit] = data.line.track_limits_at(p_arc);
                        let limit = if side == 1 { left_limit } else { right_limit };
                        if (lat - limit).abs() < w {
                            if rs.av_alongside && !fired {
                                side = 3 - side;
                            } else {
                                side = match side {
                                    1 if w < gaps[1] => 2,
                                    2 if w < gaps[0] => 1,
                                    _ => 0,
                                };
                            }
                        }
                        side
                    };
                    match self.avoid.side {
                        0 => request.follow = Some((r, AVOID_FOLLOW_DISTANCE)),
                        // Never move toward the target from where we are.
                        1 => self.avoid.offset = (lat - clearance).min(x),
                        _ => self.avoid.offset = (lat + clearance).max(x),
                    }
                    // Target speed 0x77B5A0, only on decision ticks.
                    self.avoid.target_speed = if r.speed() < rs.av_stopped_speed {
                        rs.av_stopped_target
                    } else if closing <= speed {
                        rs.av_normal_target
                    } else if !rs.av_figure_of_eight {
                        rs.av_wrong_way_target
                    } else {
                        0.
                    };
                    if self.avoid.target_speed > 0. {
                        request.desired_speed = self.avoid.target_speed;
                        request.brake_to_speed = true;
                    }
                    self.avoid.decision = rs.av_decision;
                }
                request.track_limits = true;
                request.desired_offset = self.avoid.offset;
                request.merge = rs.av_merge;
            }
            Behaviour::Recovery => {
                // Drive 0x78EE70. Offset 0x78ABB0: off track the planned line
                // drifts toward our side at rcMergeFactor; on track it moves to us.
                if on_track {
                    request.desired_offset = own_lateral;
                } else {
                    let step = if own_lateral <= 0. { -dt } else { dt };
                    request.desired_offset = self.offset + step * rs.rc_merge;
                    request.merge = rs.av_merge;
                }
                // Octant 0x77C9B0, mirrored when we are left of the line.
                let mirror = if own_lateral <= 0. { -1. } else { 1. };
                let b = (view_heading * mirror).to_degrees();
                let a = (clockwise(sub(planned, here_point)) * mirror).to_degrees();
                octant_index = octant(a, b);
                let rc = &mut self.recovery;
                if !rc.latched {
                    rc.mode = if octant_index <= 3 { 2 } else { 1 };
                }
                if rc.mode == 1 {
                    request.full_brake = true;
                    if speed < RECOVERY_STOPPED_SPEED {
                        rc.latched = true;
                        rc.mode = match octant_index {
                            0..=2 => 2,
                            3..=5 => 3,
                            _ => 5,
                        };
                    }
                }
                match rc.mode {
                    3 | 5 => {
                        let done = if rc.mode == 3 {
                            a.abs() < 5. || b.abs() < 5.
                        } else {
                            (80. < a.abs() && a.abs() < 90.) || b.abs() < 5.
                        };
                        request.throttle_cap = 1.;
                        if done {
                            request.desired_speed = if rc.mode == 3 { f32::MAX } else { 0. };
                            rc.mode = 2;
                        } else {
                            request.reverse = true;
                            request.desired_speed = RECOVERY_REVERSE_SPEED;
                        }
                    }
                    2 => request.reverse = false,
                    _ => {}
                }
                // Obstruction contacts toggle reverse/forward for rcCollisionTimeOut.
                if obstruction && rc.timer < 0. {
                    rc.timer = rs.rc_collision_timeout;
                    rc.toggle = !rc.toggle;
                    request.reverse = rc.toggle;
                } else if rc.timer > 0. {
                    request.reverse = rc.toggle;
                    rc.timer -= dt;
                }
                // Reset after rcRecoveryTimeOut is disabled by default (rcResetAllowed).
                rc.elapsed += dt;
            }
            Behaviour::RecoverTbone => {
                // Drive 0x776A00: back straight out.
                if self.tbone > 0. {
                    request.reverse = true;
                    request.no_steer = true;
                    self.tbone -= dt;
                }
            }
            Behaviour::Retire => {
                // Drive 0x79C6E0 for a stuck car: stop and hold; once below
                // rtStoppedSpeed it requests removal (req+0x38), as the
                // failure retire does at its retire point.
                request.force_brake = true;
                request.no_steer = true;
                if speed < self.parameters.faults.stopped_speed {
                    self.failure.removal = true;
                }
            }
            Behaviour::LostControl
            | Behaviour::ControlledStop
            | Behaviour::RaceDamaged
            | Behaviour::RetireFailure => {
                self.drive_failure(data, &mut request, arc, speed, own.half_width(), dt);
            }
        }
        // Both planned offsets move toward the request at its merge rate
        // (0x7878F0 at the look-ahead, 0x788030 at the car).
        if speed < SLOW_SPEED {
            self.offset = if self.behaviour == Behaviour::RaceStart {
                own_lateral
            } else {
                line_distance
            };
            self.offset_here = self.offset;
        } else {
            self.offset = approach(
                self.offset,
                request.desired_offset,
                request.merge,
                request.merge,
                dt,
            );
            self.offset_here = approach(
                self.offset_here,
                request.desired_offset,
                request.merge,
                request.merge,
                dt,
            );
        }
        let followed = request.follow.map(|(r, distance)| {
            let gap = dot3(sub(r.rear(), own_front), sub(r.rear(), own_front)).sqrt() - distance;
            let along = dot3(sub(r.rear(), own_front), f) - distance;
            (gap, along, speed - r.speed())
        });

        // Look-ahead, target point and curvature (0x779A20, 0x798E90).
        self.update_look_ahead(speed);
        let (_, _, _, limits) = data.line.at(arc + self.look_ahead);
        let (point, lateral) = data.line.curve(arc + self.look_ahead);
        // Limit types 1/2 (racing limits), or 3/4 (track limits) for avoid
        // and the race start.
        let limits = if request.track_limits {
            data.line.track_limits_at(arc + self.look_ahead)
        } else {
            limits
        };
        // 0x7878F0 / 0x782D20 at the look-ahead limits.
        let half_width = own.half_width();
        self.offset = stop_at_limits(self.offset, limits, half_width);
        // 0x788030 stops the planned offset at the car the same way at the
        // limits there (the +0x150 block's limit links).
        let limits_car = if request.track_limits {
            data.line.track_limits_at(arc)
        } else {
            limits_here
        };
        self.offset_here = stop_at_limits(self.offset_here, limits_car, half_width);
        // 0x798E90: the angle between the line laterals at the car (+0x180)
        // and at the look-ahead (+0xC0), acos of their clamped dot product,
        // negative when the look-ahead point lies on the car lateral's
        // negative side; kept when the look-ahead is not positive.
        let cos = dot2(here_lateral, lateral).clamp(-1., 1.);
        let mut angle = f64::from(cos).acos() as f32;
        if dot2(sub(point, here_point), here_lateral) < 0. {
            angle = -angle;
        }
        if self.look_ahead > 0. {
            self.curvature = angle / self.look_ahead;
        }
        // Lateral PID (0x7892E0), active while on_track (test type 11):
        // target = planned look-ahead point (+0xD0) + shift along the car's
        // line lateral (+0x180), shift = (prediction·m(v) + PID(error)) ·
        // corner factor. Measured from the front (car state +0xF0).
        let planned: [f32; 3] = std::array::from_fn(|k| point[k] + lateral[k] * self.offset);
        let target = if on_track {
            let [kp, ki, kd, lp, li, ld, lo] = p.pid;
            let error = own_lateral - self.offset_here;
            let [previous_error, _, integral] = self.pid_state;
            // 0x7790B0: the limits clamp the raw terms; the derivative starts
            // from a zero previous error.
            let proportional = error.clamp(-lp, lp);
            let integral = (integral + dt * error).clamp(-li, li);
            let derivative = if dt == 0. {
                0.
            } else {
                ((error - previous_error) / dt).clamp(-ld, ld)
            };
            self.pid_state = [error, previous_error, integral];
            let output = (kp * proportional + ki * integral + kd * derivative).clamp(-lo, lo);
            let [m_low, m_high, v_low, v_high] = p.pid_prediction;
            let multiplier = remap(speed, v_low, v_high, m_low, m_high);
            let prediction = dot2(sub(planned, own_front), here_lateral);
            // The angle between our nose and the line direction at the car
            // (its lateral turned by 90 degrees). The original picks the
            // "with corner" angles when κ·(direction·lateral) > 0, which is
            // never true, so the "against corner" pair always applies.
            let direction = [here_lateral[2], 0., -here_lateral[0]];
            let cos = dot2(direction, horizontal(forward)).clamp(-1., 1.);
            let angle = (f64::from(cos).acos() as f32).abs().to_degrees();
            let [angle_min, angle_max] = p.pid_corner_angle;
            let factor = if angle > angle_min {
                remap(angle, angle_min, angle_max, 1., 0.)
            } else {
                1.
            };
            let shift = (prediction * multiplier + output) * factor;
            std::array::from_fn(|k| planned[k] + here_lateral[k] * shift)
        } else {
            self.pid_state = [0.; 3];
            planned
        };
        let to_target = sub(target, own_front);
        let heading_error = dot2(to_target, right).atan2(dot2(to_target, forward));

        // Next brake line ahead and its corner speed (0x792C80) with the
        // race manager's scale A and floor B.
        let (id, distance) = next_brake_line(data, arc);
        let line = data.brake_lines[id].1;
        // Off the optimal line the factor is tgNonOptimalSpeedFactor.
        let factor = if self.racing_line != RacingLine::Optimal {
            p.tg_non_optimal_speed_factor
        } else {
            remap(
                self.offset.abs().max(line_distance.abs()),
                0.,
                p.tg_offset_max_for_speed,
                1.,
                p.tg_offset_speed_factor,
            )
        };
        let s = self.corner_speed_ratio(p.corner_speed_in_ratio);
        let line_target = (line.min_speed + (line.max_speed - line.min_speed) * factor * s).clamp(
            line.min_speed.min(line.max_speed),
            line.min_speed.max(line.max_speed),
        );
        // Drift mode: the line counts driftBrakeLineAdvance earlier and its
        // target is the drift speed (the maximum when that is 0); within the
        // advance of the line the request has no brake line.
        let (distance, line_target, line_valid) = match (&p.drift, drift_mode) {
            (Some(d), true) => (
                distance - d.brake_line_advance,
                if line.drift_speed != 0. {
                    line.drift_speed
                } else {
                    line.max_speed
                },
                distance - d.brake_line_advance > 0.,
            ),
            _ => (distance, line_target, true),
        };
        // Request +0x1C/+0x28 as the next drift test reads them; an early
        // return leaves the reset values 0 [I: provider kept valid].
        self.drift.line = Some(if line_valid {
            (line.drift_speed, distance)
        } else {
            (0., 0.)
        });

        // Mistake slot update (0x787620), after the behaviour drive.
        self.mistake_update(id, dt, &mut request.grip);

        // Brake-line state (0x7929D0 timer, 0x7886C0).
        if let Some(passed) = &mut self.lines.passed {
            passed.3 += dt;
        }
        let request_line = line_valid.then_some(id);
        if self.lines.request != request_line {
            if let Some(active) = self.lines.active {
                self.lines.passed = Some((active, self.lines.target, self.lines.hold, 0.));
            }
            self.lines.active = None;
        }
        self.lines.request = request_line;
        let mut line_brake = 0.;
        let previous_brake = self.brake;
        // 0x7886C0 goes on only when the class's brake data is valid
        // (0x7A3160); otherwise the line neither activates nor brakes.
        let line_valid = line_valid && brake_data_valid(&line);
        if line_valid {
            self.lines.target = line_target;
            self.lines.modifier = line.distance_modifier;
            self.lines.hold = line.hold_time;
        }
        if line_valid && speed <= line_target {
            if speed < line_target * BRAKE_LINE_CANCEL_FACTOR {
                self.lines.active = None;
            }
        } else if line_valid {
            let b = data.braking_distance(TABLE_GRIP, speed, line_target);
            let pedal = if p.brake_on_speed > 0. {
                speed / p.brake_on_speed
            } else {
                0.
            };
            let reaction = pedal * (1. - previous_brake);
            let mut d = if b <= reaction { b * 2. } else { reaction + b };
            d *= line.distance_modifier;
            // An armed late/early mistake scales the trigger distance and
            // activates when it is reached.
            let trigger = p.corner_brake_distance_factor * d;
            let trigger = match self.mistake.pending {
                Some((k @ (BRAKE_LATE | BRAKE_EARLY), _)) => trigger * p.faults.factors[k as usize],
                _ => trigger,
            };
            if distance <= trigger {
                if self.lines.active != Some(id) {
                    self.activate_mistake(&[BRAKE_LATE, BRAKE_EARLY]);
                }
                self.lines.active = Some(id);
            }
            if self.lines.active == Some(id) {
                // 0x7886C0 measures the ratio against the coasting distance
                // (the cruise tables), not the braking distance b: with the
                // coasting distance far beyond d the ratio sits just under 1
                // and `ratio^24` gives a part pedal that rises as the car
                // closes on the line (live: 0.012 RMS over 6 699 raw samples,
                // docs/AI_CHOREOGRAPHER_PORT.md "Redgate", F476).
                let coast = data.cruise_distance(TABLE_GRIP, speed, line_target);
                let ratio = if coast == d {
                    0.
                } else {
                    clamp01((distance - coast) / (d - coast))
                };
                line_brake = ratio.powf(p.brake_cruise_exponent);
                if line_brake > 0. {
                    self.activate_mistake(&[BRAKE_HEAVY, BRAKE_LIGHT, BRAKE_LOCK]);
                }
            }
        }

        // Brake (0x78E8A0): line brake, spin; cap 1. An active heavy/light
        // mistake is a floor/cap on the brake.
        let mut brake = match self.mistake.active {
            Some((BRAKE_HEAVY, _)) => line_brake.max(p.faults.factors[0]),
            Some((BRAKE_LIGHT, _)) => line_brake.min(p.faults.factors[1]),
            _ => line_brake,
        };
        // brake_spin activates while braking for a line with 8..12 m/s excess (0x77A110).
        if self.lines.active == Some(id) {
            let excess = speed - line_target;
            if BRAKE_SPIN_EXCESS[0] < excess && excess < BRAKE_SPIN_EXCESS[1] {
                self.activate_mistake(&[BRAKE_SPIN]);
            }
        }
        // Follow brake (0x77A1E0): inside the follow distance along our heading.
        if let Some((_, along, _)) = followed
            && along < 0.
            && speed > MINIMUM_MOVING_SPEED
        {
            brake = brake.max(request.follow_brake_cap.min(p.follow_brake));
        }
        if slip > std::f32::consts::FRAC_PI_2 && !request.reverse {
            brake = 1.;
        }
        if request.force_brake
            || (request.brake_to_speed && speed - request.desired_speed > rs.target_speed_big)
        {
            brake = 1.;
        }
        // The AI ABS cap (0x77A2E0) limits every source but the full-brake
        // flag +0xB2, which 0x78E8A0 returns before it.
        brake = brake.min(abs_cap(&p.abs, speed, &view.wheel_speeds));
        if request.full_brake {
            brake = 1.;
        }
        brake = brake.min(1.).min(request.brake_cap);

        // Throttle (0x783180): governor and limiters; any previous brake -> 0.
        let mut governor_target = f32::MAX;
        let throttle = if previous_brake > 0. {
            0.
        } else {
            // Drift mode holds a passed line for thBrakeLineHoldTimeDrift.
            let hold_time = |hold: f32| match &p.drift {
                Some(d) if drift_mode => d.hold_time,
                _ => hold,
            };
            let within_hold = self
                .lines
                .passed
                .is_some_and(|(_, _, hold, t)| t < hold_time(hold));
            governor_target = governor_target.min(request.desired_speed);
            if within_hold {
                governor_target = governor_target.min(self.lines.passed.unwrap().1);
            } else if self.lines.active.is_some() {
                governor_target =
                    governor_target.min(self.lines.target - p.th_brake_line_speed_drop);
            }
            let e = speed - governor_target;
            let current = self.throttle;
            let governor = if e < -p.th_target_speed_big {
                1.
            } else {
                let goal = if e < -p.th_target_speed_small {
                    p.th_target_medium
                } else if e <= p.th_target_speed_small {
                    current
                } else if e <= p.th_target_speed_big {
                    p.th_target_small
                } else {
                    0.
                };
                let gap = (goal - current).abs();
                current + (dt * p.th_target_move_speed * (goal - current)).clamp(-gap, gap)
            };
            let slide = if speed > SLIDE_LIFT_MIN_SPEED {
                remap(slip, p.slide_lift[0], p.slide_lift[1], 1., p.slide_lift[2])
            } else {
                1.
            };
            // The corner lift (0x77A940) works only above the brake line's
            // target (controller +0x8, written by 0x7886C0): at or below it
            // the bend does not lift the throttle.
            let corner = if speed > self.lines.target {
                remap(
                    self.curvature.abs(),
                    p.corner_lift[0],
                    p.corner_lift[1],
                    1.,
                    p.th_corner_lift_min,
                )
            } else {
                1.
            };
            // Follow throttle (0x77A630).
            let [
                small,
                medium,
                dist_big,
                dist_medium,
                dist_small,
                speed_small,
                speed_medium,
            ] = p.follow;
            let follow = match followed {
                Some((gap, _, closing))
                    if gap <= dist_big
                        && closing >= -speed_medium
                        && speed >= MINIMUM_MOVING_SPEED =>
                {
                    if closing <= speed_small && gap > dist_medium {
                        medium
                    } else if closing <= speed_small && gap >= -dist_small {
                        if gap < 0. { small } else { current }
                    } else {
                        0.
                    }
                }
                _ => 1.,
            };
            let limited = governor
                .min(follow)
                .min(slide)
                .min(corner)
                .min(request.throttle_cap)
                .min(traction_control(&p.traction, speed, &view.wheel_speeds, &view.driven));
            // powerout_spin (0x77A450): armed, past the brake line, previous
            // throttle above 0.6 and moving; while active the throttle is full.
            if within_hold && self.throttle > POWEROUT_THROTTLE && speed > POWEROUT_SPEED {
                self.activate_mistake(&[POWEROUT_SPIN]);
            }
            match self.mistake.active {
                Some((POWEROUT_SPIN, _)) => limited.max(1.),
                _ => limited,
            }
        };

        // Steering (0x77AC50): heading gain plus yaw-rate damping, normalised by
        // the car's maximum steering angle. Written in the +E30 convention
        // (positive = left): the original negates both the heading error and
        // ω·up, and its publisher (0x996240) negates the command again, so
        // +ω·up here is the original's sign (docs/AI_DRIVER_PORT.md, F391).
        // Zero with no-steer; negated while reversing.
        let yaw = view.angular[0] * up[0] + view.angular[1] * up[1] + view.angular[2] * up[2];
        let steer = if request.no_steer {
            0.
        } else {
            // Drift mode damps the yaw rate with driftStAngVelFactor.
            let ang_vel_factor = match &p.drift {
                Some(d) if drift_mode => d.st_ang_vel_factor,
                _ => p.st_ang_vel_factor,
            };
            let ground = p.st_angle_factor * heading_error + ang_vel_factor * yaw;
            // In the air (state +0xE2 == 0) the angle blends from the slip
            // angle (horizontal velocity against the horizontal forward axis)
            // to the ground law by stProportionRabbitWhenInAir (0 shipped:
            // the wheels follow the travel direction). Here in the +E30
            // convention: positive when the velocity points left of the nose.
            let angle = if view.airborne {
                let slip = air_slip(view.velocity, f, right);
                slip + (ground - slip) * p.st_proportion_in_air
            } else {
                ground
            };
            (angle / view.maximum_steering_angle.max(1e-3)).clamp(-1., 1.)
        };
        let steer = if request.reverse { -steer } else { steer };
        // Reverse negates the throttle (0x783180); 0x7929D0 publishes
        // |throttle| with the reverse byte +0x1E.
        let throttle = if request.reverse { -throttle } else { throttle };
        let reverse = throttle < 0.;
        let throttle = throttle.abs();

        // Output slew (0x779F70).
        self.steering = approach(self.steering, steer, p.steering_speed, p.steering_speed, dt);
        self.throttle = approach(
            self.throttle,
            throttle,
            p.throttle_on_speed,
            p.throttle_off_speed,
            dt,
        );
        self.brake = approach(self.brake, brake, p.brake_on_speed, p.brake_off_speed, dt);
        // 0x794310 ends any active mistake at low speed.
        if speed <= MISTAKE_CANCEL_SPEED {
            self.mistake.active = None;
        }
        self.grip = request.grip;
        self.telemetry = Telemetry {
            arc,
            look_ahead: self.look_ahead,
            curvature: self.curvature,
            heading_error,
            line_distance: distance,
            line_target,
            governor_target,
            offset: self.offset,
            behaviour: self.behaviour,
            overtake_state: self.overtake.state,
            following: followed.is_some(),
            line_error: line_distance,
            slip,
            spun: self.spun,
            on_track,
            obstruction,
            frontal,
            contact: contact.map_or(0, |c| c.1),
            avoid_eta: avoid_fire.map_or(f32::MAX, |a| a.1),
            avoid_side: self.avoid.side,
            recovery_mode: self.recovery.mode,
            octant: octant_index,
            reverse,
            front_lateral: own_lateral,
            drift_fired: self.drift.fired,
            drift_state: self.drift.state,
            drift_mode,
            drift_line: self.racing_line == RacingLine::Drift,
            drift_room,
        };
        // Packet handbrake byte +0x1C (0x77A110): request +0xB3, or a forced
        // brake below minimumMovingSpeed. +0xB3 is set only by race_start's
        // launch delay, which is not ported (0 with the shipped data).
        let handbrake = request.force_brake && speed < MINIMUM_MOVING_SPEED;
        // The car's input stage FUN_00828670 takes the pedals as they are and
        // turns the reverse byte into the gear request (set_ai_packet).
        self.reverse = reverse;
        Controls {
            throttle: self.throttle,
            brake: self.brake,
            steering: self.steering,
            handbrake,
        }
    }
}

/// The next brake line ahead of arc `arc`: (id, distance along the line).
/// The track view's next line (0x7923F0) is the first link ahead whose line
/// has valid data for the class (0x78E820), so lines a class does not brake
/// for are passed over. A class with no valid line keeps the nearest one.
fn next_brake_line(data: &AiData, arc: f32) -> (usize, f32) {
    let ahead = |valid: bool| {
        data.brake_lines
            .iter()
            .enumerate()
            .filter(|(_, (_, b))| !valid || brake_data_valid(b))
            .map(|(i, &(s, _))| (i, (s - arc).rem_euclid(data.line.length)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
    };
    ahead(true).or_else(|| ahead(false)).expect("brake lines")
}

/// 0x7A3160: brake data is valid when its maximum and minimum speeds and its
/// distance modifier are all positive. The shipped data marks the lines a
/// class does not brake for with 0/0 speeds (Milan route 0: touring lines 1
/// and 3, formula3 also 7).
fn brake_data_valid(b: &ai::BrakeData) -> bool {
    b.max_speed > 0. && b.min_speed > 0. && b.distance_modifier > 0.
}

/// brake_lock slot order: drawn index 0..3 -> request +0xC0, +0xC4, +0xB8,
/// +0xBC, i.e. wheels FL, FR, RL, RR in the rear-first wheel order (§2.4).
const LOCK_WHEEL: [usize; 4] = [2, 3, 0, 1];

/// Random mistake type (0x787620): the type whose factor is the smallest one
/// strictly greater than `r`; ties keep the lower id.
fn pick_mistake(factors: &[f32; 7], r: f32) -> Option<u8> {
    let mut pick: Option<(usize, f32)> = None;
    for (k, &w) in factors.iter().enumerate() {
        if w > r && pick.is_none_or(|(_, best)| w < best) {
            pick = Some((k, w));
        }
    }
    pick.map(|(k, _)| k as u8)
}

/// Mistakes, choreographed failures and race-manager inputs
/// (docs/AI_MISTAKES_PACING_RESEARCH.md §2, §4.3, §5).
impl AiDriver {
    /// Arm a mistake (0x779990 SetPending), as the choreographer's mistake
    /// event does; it activates at its own driving condition.
    pub fn arm_mistake(&mut self, kind: u8, timeout: f32) {
        self.mistake.pending = Some((kind, timeout));
    }
    /// Disarm (mistake event abort 0x4018D0).
    pub fn disarm_mistake(&mut self) {
        self.mistake.pending = None;
    }
    /// Armed and active mistake ids.
    pub fn mistake(&self) -> (Option<u8>, Option<u8>) {
        (
            self.mistake.pending.map(|m| m.0),
            self.mistake.active.map(|m| m.0),
        )
    }
    /// Request grip multipliers of the last update, rear L/R, front L/R.
    pub fn request_grip(&self) -> [f32; 4] {
        self.grip
    }
    /// The packet's reverse byte +0x1E of the last update, for
    /// [`crate::source_vehicle::Runtime::set_ai_packet`].
    pub fn reverse_request(&self) -> bool {
        self.reverse
    }
    /// The failure/damage retire drive has stopped the car (retire drive
    /// 0x79C6E0 below rtStoppedSpeed: removal request req+0x38 [I]).
    pub fn removal_requested(&self) -> bool {
        self.failure.removal
    }

    /// Uniform in [0, 1) from the authored driver stream.
    fn unit(&mut self) -> f32 {
        self.uniform() * 0.5 + 0.5
    }

    /// `S = max(A · corner_speed_in_ratio, B)` (0x792C80).
    fn corner_speed_ratio(&self, ratio: f32) -> f32 {
        let scaled = self.pace.a * ratio;
        if scaled > self.pace.b {
            scaled
        } else {
            self.pace.b
        }
    }

    /// Scores of lost_control (21), controlled_stop (20), the retire
    /// failure/damage branch (10) and race_damaged (9). The damage tests run
    /// only with the engine-failure gate drv+0x316D (§2.8). The spun-and-fast
    /// lost_control branch is not ported (coSpinSpeed's default is not recovered).
    fn fault_scores(&self, speed: f32) -> [u8; 4] {
        let f = &self.parameters.faults;
        let pace = &self.pace;
        let failed = pace.failures.iter().any(|&x| x);
        let lost = failed && speed > f.retirement_speed + 5.;
        let retire = failed || (pace.damage_gate && pace.damage > f.damage_retirement_ratio);
        let damaged = pace.damage_gate && pace.damage >= 0.5;
        [
            if lost { 21 } else { 0 },
            if pace.stop { 20 } else { 0 },
            if retire { 10 } else { 0 },
            if damaged { 9 } else { 0 },
        ]
    }

    /// Drift test 0x78FE90 ([`DriftState::test`]) and the refusal 0x78CB30
    /// on the previous update's request: no drift the car could not brake
    /// from to the next line's drift speed in time.
    /// AUTHORED: the draws use the driver's random stream (the original's
    /// RNG instance at test context +0x38 is not identified).
    fn drift_test(&mut self, data: &AiData, speed: f32, dt: f32) {
        let mut state = self.drift;
        let check = state.test(self.race, dt, |a, b| a + (b - a) * self.unit());
        self.drift = state;
        if check
            && let Some((drift_speed, distance)) = self.drift.line
            && drift_speed < speed
            && data.braking_distance(TABLE_GRIP, speed, drift_speed) > distance
        {
            self.drift.fired = false;
            self.drift.timer = DRIFT_RETRY;
        }
    }

    /// Drift behaviour: state update 0x78B3C0 and drive 0x783A10 (request
    /// edits; the shared race drive follows). Returns the drift-mode flag
    /// (+0xC8). `room` is the free room to the left/right track limits at
    /// the look-ahead sample, `size` the car's half width and length.
    #[allow(clippy::too_many_arguments)]
    fn drive_drift(
        &mut self,
        data: &AiData,
        request: &mut Request,
        arc: f32,
        speed: f32,
        slip: f32,
        cornering: bool,
        room: [f32; 2],
        size: [f32; 2],
    ) -> bool {
        let Some(d) = self.parameters.drift.clone() else {
            return false;
        };
        self.drift.state = d.next_state(self.drift.state, speed, slip, cornering);
        request.merge = 0.;
        match self.drift.state {
            1 => {
                // Toward the side of the next corner (view +0x84, 0x792580).
                let (id, _) = next_brake_line(data, arc);
                let at = data.brake_lines[id].0;
                let (_, here, _, _) = data.line.at(at);
                let (_, ahead, _, _) = data.line.at(at + NEXT_CORNER_DISTANCE);
                let next_corner = wrap(heading(ahead) - heading(here)) / NEXT_CORNER_DISTANCE;
                request.desired_offset = if next_corner < 0. {
                    -d.position
                } else {
                    d.position
                };
                request.merge = d.merge;
                request.grip = [d.racing_grip; 4];
                true
            }
            2 => {
                // Kick only once off the brakes [I: context +0x3C +4 = the
                // published brake].
                if self.brake <= 0. {
                    request.grip = [d.min_grip, d.min_grip, d.initiate_grip, d.initiate_grip];
                }
                true
            }
            3 => {
                // The outside of the bend: left room for a right-hand bend.
                let outside = if self.curvature >= 0. {
                    room[0]
                } else {
                    room[1]
                };
                request.grip = d.drifting_grip(slip.to_degrees(), outside, size);
                true
            }
            _ => false,
        }
    }

    /// Activation of lost_control (0x78B1F0: crash mode above crashMinSpeed,
    /// else the failed-wheel grip cut) and of the side choice of
    /// race_damaged/retire.
    fn activate_failure(&mut self, behaviour: Behaviour, speed: f32) {
        match behaviour {
            Behaviour::LostControl => {
                self.failure.crash = speed > self.parameters.faults.crash_min_speed;
                self.failure.wobble_timer = 0.;
            }
            _ => self.failure.side_timer = 0.,
        }
    }

    /// The nearer track limit, re-chosen every second (0x797F00 [I]): the
    /// planned offset keeps the car's half width plus 0.5 m inside it.
    fn limit_side(&mut self, data: &AiData, arc: f32, half_width: f32, dt: f32) -> f32 {
        self.failure.side_timer -= dt;
        if self.failure.side_timer <= 0. {
            self.failure.side_timer = DAMAGED_PERIOD;
            let [left, right] = data.line.track_limits_at(arc);
            let margin = half_width + 0.5;
            self.failure.side_offset = if (self.offset - left).abs() <= (right - self.offset).abs()
            {
                left + margin
            } else {
                right - margin
            };
        }
        self.failure.side_offset
    }

    /// Drives of lost_control (0x776B80), controlled_stop (0x78AA90),
    /// race_damaged (0x799FB0) and the retire failure branch (0x79A090).
    /// AUTHORED where not recovered: the crash PID, spin/flip angles and
    /// inertia factor are not ported (normal steering, no flip); the wobble
    /// amplitude is deviation_from_racing_line_large; the retire point is the
    /// nearer track limit, where the car stops.
    fn drive_failure(
        &mut self,
        data: &AiData,
        request: &mut Request,
        arc: f32,
        speed: f32,
        half_width: f32,
        dt: f32,
    ) {
        let f = self.parameters.faults.clone();
        match self.behaviour {
            Behaviour::LostControl if self.failure.crash => {
                request.desired_speed = 0.;
                request.full_brake = true;
                request.grip = [
                    f.crash_rear_grip,
                    f.crash_rear_grip,
                    f.crash_front_grip,
                    f.crash_front_grip,
                ];
            }
            Behaviour::LostControl => {
                request.desired_speed = f.retirement_speed;
                request.brake_to_speed = true;
                request.brake_cap = WOBBLE_BRAKE_CAP;
                self.failure.wobble_timer -= dt;
                if self.failure.wobble_timer <= 0. {
                    self.failure.wobble_timer = WOBBLE_PERIOD;
                    self.failure.wobble_offset = self.uniform() * self.parameters.deviation_large;
                }
                request.desired_offset = self.failure.wobble_offset;
                request.grip[0] = WOBBLE_REAR_GRIP;
                request.grip[1] = WOBBLE_REAR_GRIP;
            }
            Behaviour::ControlledStop => {
                request.force_brake = true;
                request.desired_offset = self.offset;
                request.no_steer = speed < MINIMUM_MOVING_SPEED;
            }
            Behaviour::RaceDamaged => {
                request.desired_speed = DAMAGED_SPEED;
                request.throttle_cap = DAMAGED_THROTTLE;
                request.merge = self.parameters.ra_merge;
                request.desired_offset = self.limit_side(data, arc, half_width, dt);
                request.track_limits = true;
            }
            Behaviour::RetireFailure => {
                let side = self.limit_side(data, arc, half_width, dt);
                request.desired_speed = f.retirement_speed;
                request.brake_to_speed = true;
                request.merge = self.parameters.ra_merge;
                request.desired_offset = side;
                request.track_limits = true;
                if (self.offset - side).abs() < 0.5 {
                    request.force_brake = true;
                    request.no_steer = speed < MINIMUM_MOVING_SPEED;
                    if speed < f.stopped_speed {
                        self.failure.removal = true;
                    }
                }
            }
            _ => {}
        }
    }

    /// Let-pass (race drive 0x77C2C0): while the car is dropping back or being
    /// lapped, a car whose front is behind our rear within coLetPassDistance.
    fn let_pass(&self, others: &[Other], own_rear: [f32; 3], forward: [f32; 3]) -> bool {
        let distance = self.parameters.faults.let_pass_distance;
        self.pace.backwards
            && others.iter().any(|r| {
                let d = sub(r.front(), own_rear);
                dot3(d, forward) < 0. && dot3(d, d) < distance * distance
            })
    }

    /// Activation 0x7799B0 of an armed mistake of one of `kinds`.
    fn activate_mistake(&mut self, kinds: &[u8]) {
        if self.mistake.active.is_some() {
            return;
        }
        let Some((kind, _)) = self.mistake.pending else {
            return;
        };
        if !kinds.contains(&kind) {
            return;
        }
        self.mistake.pending = None;
        let time = if kind == BRAKE_SPIN || kind == POWEROUT_SPIN {
            MISTAKE_SPIN_TIME
        } else {
            MISTAKE_TIME
        };
        self.mistake.active = Some((kind, time));
        if kind == BRAKE_LOCK {
            self.mistake.wheel = LOCK_WHEEL[((self.unit() * 4.) as usize).min(3)];
        }
    }

    /// Mistake slot update 0x787620: a new brake line clears active types
    /// 0-3/6 and may draw a random mistake (only with generateMistakes, false
    /// in shipped data); timers; then the grip overrides of the active type.
    /// AUTHORED: a failed wheel (blow_tyre) keeps grip 0.1, standing in for
    /// the vehicle's failed-wheel parameter, which the port does not model.
    fn mistake_update(&mut self, line: usize, dt: f32, grip: &mut [f32; 4]) {
        let f = self.parameters.faults.clone();
        if self.mistake.line != Some(line) {
            self.mistake.line = Some(line);
            if matches!(self.mistake.active, Some((k, _)) if k != BRAKE_SPIN && k != POWEROUT_SPIN)
            {
                self.mistake.active = None;
            }
            if f.generate && self.mistake.pending.is_none() {
                self.mistake.counter += 1;
                if self.mistake.counter > f.lines_before && self.unit() <= f.probability {
                    let r = self.unit();
                    if let Some(k) = pick_mistake(&f.factors, r) {
                        self.mistake.pending = Some((k, MISTAKE_ARM_TIMEOUT));
                    }
                }
            }
        }
        if let Some((_, t)) = &mut self.mistake.pending {
            *t -= dt;
            if *t <= 0. {
                self.mistake.pending = None;
            }
        }
        if let Some((_, t)) = &mut self.mistake.active {
            *t -= dt;
            if *t <= 0. {
                self.mistake.active = None;
            }
        }
        match self.mistake.active {
            Some((BRAKE_SPIN, _)) => *grip = [f.spin_rear_grip, f.spin_rear_grip, 1., 1.],
            Some((POWEROUT_SPIN, _)) => *grip = [f.power_rear_grip, f.power_rear_grip, 1., 1.],
            Some((BRAKE_LOCK, _)) => {
                *grip = [1.; 4];
                grip[self.mistake.wheel] = f.lock_grip;
            }
            _ => {}
        }
        for (g, &failed) in grip.iter_mut().zip(&self.pace.failures) {
            if failed {
                *g = g.min(FAILED_WHEEL_GRIP);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offsets_stop_at_the_limits_or_on_the_line() {
        // Half width 1: the edges are the limits moved 1 m inward.
        let limits = [-4., 3.];
        assert_eq!(stop_at_limits(1.5, limits, 1.), 1.5);
        assert_eq!(stop_at_limits(2.5, limits, 1.), 2.);
        assert_eq!(stop_at_limits(-3.5, limits, 1.), -3.);
        assert_eq!(stop_at_limits(0., limits, 1.), 0.);
        // An edge past the line (Donington's left racing limit 0.5 m from
        // the line) puts the offset on the line (0x782D20).
        assert_eq!(stop_at_limits(-3., [-0.5, 3.], 1.), 0.);
        assert_eq!(stop_at_limits(2., [-4., 0.8], 1.), 0.);
        // An edge exactly on the line leaves it.
        assert_eq!(stop_at_limits(-3., [-1., 3.], 1.), -3.);
    }

    #[test]
    fn traction_control_limits_the_throttle_by_driven_wheel_spin() {
        let tc = [1., 20., 0., 0.5, 2.];
        let front = [false, false, true, true];
        // Outside tcMinSpeed..tcMaxSpeed, or no driven wheel faster than the car: 1.
        assert_eq!(traction_control(&tc, 1., &[0., 0., 9., 9.], &front), 1.);
        assert_eq!(traction_control(&tc, 20., &[0., 0., 40., 40.], &front), 1.);
        assert_eq!(traction_control(&tc, 10., &[30., 30., 9., 9.], &front), 1.);
        // At 10.5 m/s the level is 0.25; 1.5x spin is halfway to tcMaxWheelSpin.
        let half = traction_control(&tc, 10.5, &[0., 0., 15.75, 1.], &front);
        assert!((half - 0.625).abs() < 1e-6, "{half}");
        // Spin beyond tcMaxWheelSpin holds the level.
        let full = traction_control(&tc, 10.5, &[0., 0., 40., 1.], &front);
        assert!((full - 0.25).abs() < 1e-6, "{full}");
    }

    #[test]
    fn the_track_view_curve_passes_the_gates_and_bows_out_of_the_chords() {
        // A 20 m square loop, clockwise seen from above (+y up).
        let points = vec![[0., 0., 0.], [20., 0., 0.], [20., 0., 20.], [0., 0., 20.]];
        let line = Line {
            open: false,
            lateral: vec![[0., 0., 1.]; 4],
            limits: vec![[-5., 5.]; 4],
            track_limits: vec![[-8., 8.]; 4],
            start: vec![0., 20., 40., 60.],
            length: 80.,
            gate_index: (0..4).collect(),
            points,
        };
        // At a gate the curve is the gate itself (t = 0).
        let (p, lateral) = line.curve(20.);
        assert!(p.iter().zip([20., 0., 0.]).all(|(a, b)| (a - b).abs() < 1e-5), "{p:?}");
        // Phantom points (0x780520) on the link (20,0,0)→(20,0,20): G0' = G2 +
        // 2·proj(G1 − G2 on G0 − G2) = (0, 0, 0) and G3' = (0, 0, 20), here the
        // neighbours themselves; the tangent at G1 is ½(G2 − G0') = (10, 0, 10),
        // so the lateral (−t_z, 0, t_x) is (−0.707, 0, 0.707).
        assert!((lateral[0] + 0.70710677).abs() < 1e-5 && (lateral[2] - 0.70710677).abs() < 1e-5, "{lateral:?}");
        // An unequal neighbour moves its phantom: with G0 at (0, 0, −20) the
        // projection is shorter and the tangent turns toward the link.
        let mut skew = line.clone();
        skew.points[0] = [0., 0., -20.];
        let (_, l2) = skew.curve(20.);
        assert!(l2[0] < lateral[0] && l2[2] < lateral[2], "{l2:?}");
        // Mid-link the curve bows outward of the chord x = 20 by 2.5 m.
        let (mid, _) = line.curve(30.);
        assert!((mid[0] - 22.5).abs() < 1e-4 && (mid[2] - 10.).abs() < 1e-4, "{mid:?}");
    }

    #[test]
    fn abs_cap_limits_the_brake_by_the_slowest_wheel() {
        let abs = [5., 0.7, 0.3, 0.2];
        // At or below absMinSpeed, or no wheel slower than the car: 1.
        assert_eq!(abs_cap(&abs, 5., &[0.; 4]), 1.);
        assert_eq!(abs_cap(&abs, 20., &[20., 21., 20., 22.]), 1.);
        // Ratio 0.7 or more: 1; 0.5 is halfway: 0.6; 0.3 or less: absMinBrake.
        assert_eq!(abs_cap(&abs, 20., &[15., 20., 20., 20.]), 1.);
        let half = abs_cap(&abs, 20., &[20., 10., 20., 20.]);
        assert!((half - 0.6).abs() < 1e-6, "{half}");
        assert_eq!(abs_cap(&abs, 20., &[20., 20., 0., 20.]), 0.2);
    }

    #[test]
    fn grid_rev_pulses_stay_within_shipped_ranges() {
        let Some(root) = std::env::var_os("GRID_GAME_ROOT") else {
            return;
        };
        let data = AiData::load(
            Path::new(&root),
            "tracks/europe/donington/route_0",
            "touring",
            "lac",
        )
        .unwrap();
        let mut driver = AiDriver::new(
            Parameters::new(&data, NORMAL_DIFFICULTY, |_| 1.).unwrap(),
            7,
        );
        let dt = 0.016;
        for (state, on, off) in [
            (3, [0.2, 0.48], [0.01, 0.7]),
            (2, [0.08, 0.18], [0.6, 1.15]),
        ] {
            let mut runs: Vec<(bool, f32)> = Vec::new();
            for _ in 0..4000 {
                let level = driver.grid_rev(state, dt) > 0.;
                match runs.last_mut() {
                    Some((l, n)) if *l == level => *n += dt,
                    _ => runs.push((level, dt)),
                }
            }
            // Skip the first and last (partial) runs.
            for &(level, length) in &runs[1..runs.len() - 1] {
                let [a, b] = if level { on } else { off };
                assert!(
                    length >= a - 2. * dt && length <= b + 2. * dt,
                    "{state} {level} {length}"
                );
            }
        }
        assert_eq!(driver.grid_rev(1, dt), 0.);
    }

    #[test]
    fn random_mistake_pick_follows_the_live_weights() {
        // Live Normal driver 0 (§2.2): heavy, light, late, early, spins, lock.
        let w = [0.600, 0.956, 0.441, 0.120, 1.0, 1.0, 0.0];
        let pick = |r| pick_mistake(&w, r);
        assert_eq!(pick(0.05), Some(BRAKE_EARLY));
        assert_eq!(pick(0.3), Some(BRAKE_LATE));
        assert_eq!(pick(0.5), Some(BRAKE_HEAVY));
        assert_eq!(pick(0.9), Some(BRAKE_LIGHT));
        // Ties keep the lower id: brake_spin before powerout_spin.
        assert_eq!(pick(0.97), Some(BRAKE_SPIN));
        assert_eq!(pick(1.0), None);
    }

    #[test]
    fn mistake_slot_windows_and_effects() {
        let Some(root) = std::env::var_os("GRID_GAME_ROOT") else {
            return;
        };
        let data = AiData::load(
            Path::new(&root),
            "tracks/europe/donington/route_0",
            "touring",
            "lac",
        )
        .unwrap();
        let mut d = AiDriver::new(
            Parameters::new(&data, NORMAL_DIFFICULTY, |_| 1.).unwrap(),
            7,
        );
        let f = d.parameters.faults.clone();
        assert!(!f.generate);
        assert_eq!(
            (f.spin_rear_grip, f.power_rear_grip, f.lock_grip),
            (0.2, 0.4, 0.2)
        );
        let mut grip = [1.; 4];
        d.mistake_update(0, 0.016, &mut grip);
        // Only the armed type activates, and only at its own condition.
        d.arm_mistake(BRAKE_SPIN, 60.);
        d.activate_mistake(&[BRAKE_LOCK]);
        assert_eq!(d.mistake(), (Some(BRAKE_SPIN), None));
        d.activate_mistake(&[BRAKE_SPIN]);
        assert_eq!(d.mistake(), (None, Some(BRAKE_SPIN)));
        // Spins: 3 s, survive a new brake line, rear grip 0.2.
        let mut grip = [1.; 4];
        d.mistake_update(1, 2.9, &mut grip);
        assert_eq!(grip, [0.2, 0.2, 1., 1.]);
        assert_eq!(d.mistake().1, Some(BRAKE_SPIN));
        d.mistake_update(1, 0.2, &mut grip);
        assert_eq!(d.mistake().1, None);
        // powerout_spin: rear grip 0.4.
        d.arm_mistake(POWEROUT_SPIN, 60.);
        d.activate_mistake(&[POWEROUT_SPIN]);
        let mut grip = [1.; 4];
        d.mistake_update(1, 0.016, &mut grip);
        assert_eq!(grip, [0.4, 0.4, 1., 1.]);
        d.mistake.active = None;
        // brake_lock: one wheel at 0.2 for 5 s, cleared by the next brake line.
        let mut line = 1;
        let mut wheels = [0; 4];
        for _ in 0..64 {
            d.arm_mistake(BRAKE_LOCK, 60.);
            d.activate_mistake(&[BRAKE_HEAVY, BRAKE_LIGHT, BRAKE_LOCK]);
            let mut grip = [1.; 4];
            d.mistake_update(line, 0.016, &mut grip);
            assert_eq!(grip.iter().filter(|&&g| g == 0.2).count(), 1);
            assert_eq!(grip[d.mistake.wheel], 0.2);
            wheels[d.mistake.wheel] += 1;
            line += 1;
            d.mistake_update(line, 0.016, &mut grip);
            assert_eq!(d.mistake().1, None);
        }
        assert!(wheels.iter().all(|&n| n > 0), "{wheels:?}");
        d.arm_mistake(BRAKE_LOCK, 60.);
        d.activate_mistake(&[BRAKE_LOCK]);
        d.mistake_update(line, 4.9, &mut [1.; 4]);
        assert_eq!(d.mistake().1, Some(BRAKE_LOCK));
        d.mistake_update(line, 0.2, &mut [1.; 4]);
        assert_eq!(d.mistake().1, None);
        // The arm timeout (60 s) disarms.
        d.arm_mistake(BRAKE_SPIN, 60.);
        d.mistake_update(line, 59.9, &mut [1.; 4]);
        assert_eq!(d.mistake().0, Some(BRAKE_SPIN));
        d.mistake_update(line, 0.2, &mut [1.; 4]);
        assert_eq!(d.mistake().0, None);
        // A failed wheel keeps grip 0.1; corner speed scale and floor.
        d.pace.failures[3] = true;
        let mut grip = [1.; 4];
        d.mistake_update(line, 0.016, &mut grip);
        assert_eq!(grip, [1., 1., 1., 0.1]);
        assert_eq!(d.corner_speed_ratio(0.6454), 0.6454);
        d.pace.a = 1.1;
        assert_eq!(d.corner_speed_ratio(0.5), 1.1 * 0.5);
        d.pace.b = 1.;
        assert_eq!(d.corner_speed_ratio(0.5), 1.);
        // Failure scores: lost_control above rtRetirementSpeed + 5, retire;
        // damage only behind the engine-failure gate.
        assert_eq!(d.fault_scores(25.), [21, 0, 10, 0]);
        assert_eq!(d.fault_scores(15.), [0, 0, 10, 0]);
        d.pace.failures = [false; 4];
        d.pace.damage = 1.;
        assert_eq!(d.fault_scores(25.), [0, 0, 0, 0]);
        d.pace.damage_gate = true;
        assert_eq!(d.fault_scores(25.), [0, 0, 10, 9]);
        d.pace.damage = 0.5;
        assert_eq!(d.fault_scores(25.), [0, 0, 0, 9]);
    }

    #[test]
    fn characteristic_table_rows_and_bounds() {
        let rows =
            read_characteristics("driver\taggression\tspeed_in\n00\t1\t1\n01\t0.7\t0.9\n").unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1]["aggression"], 0.7);
        assert_eq!(rows[1]["speed_in"], 0.9);
        assert!(read_characteristics("driver\taggression\n00\t1.5\n").is_err());
        assert!(read_characteristics("name\taggression\n00\t1\n").is_err());
        assert!(read_characteristics("driver\taggression\n00\t1\t2\n").is_err());
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn shipped_ai_data_reproduces_live_normal_values() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let data = AiData::load(
            Path::new(&root),
            "tracks/europe/donington/route_0",
            "touring",
            "lac",
        )
        .unwrap();
        let p = Parameters::new(&data, NORMAL_DIFFICULTY, |_| 1.).unwrap();
        // Live Normal values of driver 0 (docs/AI_DRIVER_RESEARCH.md §7).
        assert!((p.throttle_confidence - 0.9606).abs() < 1e-4);
        assert!(
            (p.corner_speed_in_ratio - 0.6454).abs() < 1e-3,
            "{}",
            p.corner_speed_in_ratio
        );
        assert!(
            (p.corner_brake_distance_factor - 0.1182 - 1.).abs() < 1e-3,
            "{}",
            p.corner_brake_distance_factor
        );
        assert_eq!(p.steering_speed, 15.);
        assert_eq!(p.st_angle_factor, 2.7);
        assert!(
            (data.line.length - 3944.).abs() < 1.,
            "{}",
            data.line.length
        );
        assert_eq!(data.brake_lines.len(), 11);
        // Touring Redgate line: 28.8/46.0 m/s.
        assert_eq!(
            (
                data.brake_lines[0].1.min_speed,
                data.brake_lines[0].1.max_speed
            ),
            (28.8, 46.)
        );
        // Lacetti table: 70.28 m/s -> 132.48 m to stop at grip 2.0.
        assert!((data.braking_distance(2., 70.27765, 0.) - 132.48337).abs() < 1e-3);
        // Positive lateral offsets point to the driver's right (heading
        // decreases clockwise) at every gate; the octant mirror relies on it.
        // Gate normals are not exactly square to the line (up to ~0.25 rad).
        for i in 0..data.line.points.len() {
            let s = data.line.start[i] + 0.5 * data.line.segment(i);
            let (_, direction, lateral, _) = data.line.at(s);
            let turn = wrap(heading(lateral) - heading(direction));
            assert!(
                (turn + std::f32::consts::FRAC_PI_2).abs() < 0.5,
                "segment {i}: {turn}"
            );
            let [left, right] = data.line.track_limits_at(s);
            assert!(left < 0. && right > 0., "segment {i}: {left} {right}");
        }
    }

    #[test]
    fn recovery_octants_follow_the_table() {
        assert_eq!(octant(45., 45.), 1);
        assert_eq!(octant(45., -45.), 2);
        assert_eq!(octant(-45., -45.), 3);
        assert_eq!(octant(-45., -135.), 4);
        assert_eq!(octant(-135., -135.), 5);
        assert_eq!(octant(-135., 135.), 6);
        assert_eq!(octant(135., 135.), 7);
        assert_eq!(octant(135., 45.), 8);
        // Boundaries and the unlisted combinations give 0.
        assert_eq!(octant(90., 45.), 0);
        assert_eq!(octant(-45., 45.), 0);
    }

    #[test]
    fn extents_take_the_hull_offset_and_cover_the_tyres() {
        use grid_formats::car_collision::Bounds;
        let hull = Bounds {
            min: [-0.9, 0.1, -2.2],
            max: [0.92, 1.4, 2.25],
        };
        let mut vehicle = vec![0u32; 0x1640 / 4];
        let set = |vehicle: &mut [u32], o: usize, v: f32| vehicle[o / 4] = v.to_bits();
        set(&mut vehicle, 0x1638, -0.15);
        // Rear-right and front-left wheel centres, radii and tyre widths.
        for (o, v) in [(0x14F0, -0.76), (0x14F8, -1.2), (0x1500, 0.74), (0x1508, 1.35)] {
            set(&mut vehicle, o, v);
        }
        for o in [0x1524, 0x1528] {
            set(&mut vehicle, o, 0.3);
        }
        for o in [0x1544, 0x1548] {
            set(&mut vehicle, o, 0.27);
        }
        let e = Extents::from_vehicle(hull, &vehicle);
        assert_eq!(e.half_width, 0.92);
        assert_eq!(e.front, 2.25 + -0.15);
        assert_eq!(e.rear, -2.2 + -0.15);
        assert_eq!(e.half_length(), 0.5 * (e.front - e.rear));
        // A front tyre beyond the nose moves the front only: the rear test is
        // the else branch of the front test (0x403469).
        set(&mut vehicle, 0x1508, 2.0);
        set(&mut vehicle, 0x14F8, -2.3);
        let e = Extents::from_vehicle(hull, &vehicle);
        assert_eq!((e.front, e.rear), (2.0 + 0.3, -2.2 + -0.15));
        set(&mut vehicle, 0x1508, 1.35);
        let e = Extents::from_vehicle(hull, &vehicle);
        assert_eq!(e.rear, -2.3 - 0.3);
        // Tyre widths: the left (front-left) test first, then the right one.
        set(&mut vehicle, 0x14F0, -0.9);
        let e = Extents::from_vehicle(hull, &vehicle);
        assert_eq!(e.half_width, (-0.9f32 - 0.27 * 0.5).abs());
        set(&mut vehicle, 0x1500, 0.85);
        let e = Extents::from_vehicle(hull, &vehicle);
        assert_eq!(e.half_width, 0.85 + 0.27 * 0.5);
    }

    #[test]
    fn boxes_use_each_cars_own_asymmetric_extents() {
        let long = Extents {
            half_width: 0.95,
            front: 2.0,
            rear: -2.5,
        };
        let car = |position: [f32; 3], extents| Other {
            id: 1,
            position,
            velocity: [0.; 4],
            forward: [0., 0., 1.],
            left: [1., 0., 0.],
            extents,
        };
        let own = car([0.; 3], long);
        assert_eq!(own.front(), [0., 0., 2.]);
        assert_eq!(own.rear(), [0., 0., -2.5]);
    }

    #[test]
    fn nearest_order_is_the_single_pass_of_0x774b00() {
        let p = [0.; 3];
        let at = |d: f32| [d, 0., 0.];
        // Nearest first and farthest last; the middle two as the pass leaves them.
        assert_eq!(nearest_order(p, &[at(3.), at(1.), at(4.), at(2.)]), [1, 3, 0, 2]);
        assert_eq!(nearest_order(p, &[at(1.), at(2.), at(3.), at(4.)]), [0, 1, 2, 3]);
        assert_eq!(nearest_order(p, &[at(4.), at(3.), at(2.), at(1.)]), [3, 1, 2, 0]);
    }

    #[test]
    fn segment_intersection_codes() {
        let o = [0.; 3];
        let (r, s, point) = segment_intersect_xz(o, [0., 0., 10.], [-1., 0., 5.], [1., 0., 5.]);
        assert_eq!((r, s, point), (1, 0.5, [0., 0., 5.]));
        let (r, s, _) = segment_intersect_xz(o, [0., 0., 10.], [-1., 0., 15.], [1., 0., 15.]);
        assert_eq!((r, s), (2, 1.5));
        assert_eq!(segment_intersect_xz(o, [0., 0., 1.], [1., 0., 0.], [1., 0., 1.]).0, -1);
        assert_eq!(segment_intersect_xz(o, [0., 0., 1.], [0., 0., 2.], [0., 0., 3.]).0, -2);
    }

    #[test]
    fn path_obstacle_scan_sweeps_to_the_slower_car_ahead() {
        let extents = Extents {
            half_width: 0.9,
            front: 2.0,
            rear: -2.0,
        };
        let car = |id, x: f32, z: f32, v: f32| Other {
            id,
            position: [x, 0., z],
            velocity: [0., 0., v, 0.],
            forward: [0., 0., 1.],
            left: [1., 0., 0.],
            extents,
        };
        let own = car(0, 0., 0., 20.);
        let cfg = ScanConfig {
            stopped_speed: 1.,
            buffer_zone: 2.,
            buffer_base_speed: 10.,
            similar_angle: 0.5,
            contact_zone: 0.1,
        };
        // Ahead in lane at 10 m/s; beside us; behind; ahead but 6 m to the side.
        let others = [
            car(1, 0., 20., 10.),
            car(2, 1.8, 0.5, 20.),
            car(3, 0., -20., 10.),
            car(4, 6., 20., 10.),
        ];
        let scan = path_obstacle_scan(&own, 20., &others, &cfg, false);
        let best = scan.best.unwrap();
        assert_eq!(best.other, 0);
        // Bumpers 16 m apart, each box widened by the 2 m buffer.
        assert!((best.distance - 12.).abs() < 1e-4, "{}", best.distance);
        assert!((best.time - 1.2).abs() < 1e-4);
        assert_eq!(scan.list.len(), 1);
        // The car beside us: our front-left corner in its box and its rear
        // corner in ours, refined by the length ratio to type 8.
        assert_eq!(scan.contacts, vec![(1, 8)]);
    }

    #[test]
    fn observable_list_picks_by_priority() {
        let extents = Extents {
            half_width: 0.9,
            front: 2.0,
            rear: -2.0,
        };
        let car = |id, x: f32, z: f32| Other {
            id,
            position: [x, 0., z],
            velocity: [0.; 4],
            forward: [0., 0., 1.],
            left: [1., 0., 0.],
            extents,
        };
        let own = car(0, 0., 0.);
        // 1 ahead (gap 5), 2 far ahead, 3 behind in lane, 4 far behind,
        // 5/6 alongside on either side, 7 ahead (gap 10).
        let others = [
            car(1, 0., 10.),
            car(2, 0., 30.),
            car(3, 0., -8.),
            car(4, 0., -40.),
            car(5, 3., 0.),
            car(6, -3., 0.),
            car(7, 0., 15.),
        ];
        let along = [10., 30., -8., -40., 0., 0., 15.];
        let ids = |list: Vec<Other>| list.iter().map(|o| o.id).collect::<Vec<_>>();
        // Front 1, rear 3, side type 6 = car 5; the type-7 pick tests
        // against the type-6 minimum and finds nothing; then the nearest.
        assert_eq!(ids(observable(&own, &others, &along)), vec![1, 3, 5, 6, 7]);
        // Without a rear count, no behind car is picked or filled.
        assert_eq!(
            ids(observable_with(&own, &others, &along, 5, 0, 200.)),
            vec![1, 5, 6, 7, 2]
        );
        // Up to the maximum every car within the view distance stays.
        assert_eq!(
            ids(observable_with(&own, &others[..4], &along[..4], 5, 2, 35.)),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn contact_classify_corners_and_faces() {
        let car = |position: [f32; 3], forward: [f32; 3]| Other {
            id: 1,
            position,
            velocity: [0.; 4],
            forward,
            // Car-left (+X for a car facing +Z): forward rotated by +90 degrees.
            left: [forward[2], 0., -forward[0]],
            extents: Extents {
                half_width: 0.9,
                front: 2.24,
                rear: -2.24,
            },
        };
        let own = car([0.; 3], [0., 0., 1.]);
        // Other car's rear overlapping our nose, offset 0.3 m to our left (the
        // tests are strict, so equal widths exactly in line touch no edge).
        assert!(matches!(
            contact_classify(&own, &car([0.3, 0., 4.5], [0., 0., 1.]), 0.1),
            Some(1 | 2 | 5)
        ));
        // Nose of a car from our right (-X) into our side: right face.
        assert_eq!(
            contact_classify(&own, &car([-2.5, 0., 0.], [1., 0., 0.]), 0.1),
            Some(7)
        );
        // Same from our left: left face.
        assert_eq!(
            contact_classify(&own, &car([2.5, 0., 0.], [-1., 0., 0.]), 0.1),
            Some(8)
        );
        assert_eq!(
            contact_classify(&own, &car([10., 0., 0.], [0., 0., 1.]), 0.1),
            None
        );
    }

    /// ai_vehicle_config `default` drift values (the touring class has no
    /// overrides) and built-in defaults; drift_skill 1, drift_position 0.5.
    fn default_drift() -> Drift {
        Drift {
            st_ang_vel_factor: -0.3,
            angle_range: [15., 30.],
            angle_max: 60.,
            min_grip: 0.5,
            max_grip: 3.,
            start_angle: 15.,
            end_angle: 10.,
            merge: 1.,
            speed_min: 15.,
            grip_front: 10.,
            initiate_grip: 1.2,
            racing_grip: 1.1,
            brake_line_advance: 8.,
            hold_time: 0.2,
            skill: 1.,
            position: 0.5,
        }
    }

    fn race(place: u32, progress: f32) -> RaceView {
        RaceView {
            place,
            cars: 8,
            progress,
        }
    }

    #[test]
    fn drift_test_windows_follow_the_race_place() {
        // Draws are recorded and return the window's first bound.
        let draws = std::cell::RefCell::new(Vec::new());
        let draw = |a: f32, b: f32| {
            draws.borrow_mut().push((a, b));
            a
        };
        // Before the start: one first window, leader 10..20 s, last 2..8 s.
        let mut leader = DriftState::default();
        assert!(!leader.test(race(0, 0.), 0.1, &draw));
        assert!(!leader.test(race(0, -0.01), 0.1, &draw));
        let mut last = DriftState::default();
        assert!(!last.test(race(7, 0.), 0.1, &draw));
        assert_eq!(*draws.borrow(), [(10., 20.), (2., 8.)]);
        assert!(leader.before_start && !leader.fired);
        // Racing: the test fires once the first window runs out.
        draws.borrow_mut().clear();
        let mut t = 0.;
        while !leader.test(race(0, 0.5), 0.5, &draw) {
            t += 0.5;
        }
        assert_eq!(t, 10.);
        // Fired window for the leader 48..24 s; the leader never stops.
        assert_eq!(*draws.borrow(), [(48., 24.)]);
        for _ in 0..200 {
            assert!(leader.test(race(0, 0.6), 0.5, &draw));
        }
        assert!(leader.fired);
        // The last car: 16..8 s, then a 30..15 s pause when it gained nothing.
        draws.borrow_mut().clear();
        while !last.test(race(7, 0.5), 0.5, &draw) {}
        assert_eq!(*draws.borrow(), [(16., 8.)]);
        let mut fired = 0;
        while last.test(race(7, 0.5), 0.5, &draw) {
            fired += 1;
        }
        assert_eq!(fired, 31);
        assert_eq!(*draws.borrow(), [(16., 8.), (30., 15.)]);
        // An odd place waits for the pause; an even place fires again at once
        // after its first drift.
        assert!(!last.test(race(7, 0.5), 0.5, &draw));
        assert!(last.test(race(6, 0.5), 0.5, &draw));
        // A car that gained places keeps the test fired past its window.
        let mut gaining = DriftState {
            fired: true,
            place: 5,
            timer: 0.1,
            ..DriftState::default()
        };
        assert!(gaining.test(race(3, 1.), 0.5, &draw));
        assert!(!gaining.test(race(5, 1.), 0.5, &draw));
    }

    #[test]
    fn drift_states_follow_angle_speed_and_corner() {
        let d = default_drift();
        let deg = |a: f32| a.to_radians();
        // State 0 (construction) acts as 1; below driftSpeedMin it is 1.
        assert_eq!(d.next_state(0, 20., 0., false), 1);
        assert_eq!(d.next_state(3, 14.9, deg(40.), true), 1);
        // Initiation needs a corner below 50 m/s.
        assert_eq!(d.next_state(1, 30., 0., true), 2);
        assert_eq!(d.next_state(1, 50., 0., true), 1);
        assert_eq!(d.next_state(1, 30., 0., false), 1);
        // Drifting past driftStartAngle (15 degrees), ending below
        // driftEndAngle (10 degrees) or out of the corner.
        assert_eq!(d.next_state(2, 30., deg(14.), true), 2);
        assert_eq!(d.next_state(2, 30., deg(16.), true), 3);
        assert_eq!(d.next_state(3, 30., deg(12.), true), 3);
        assert_eq!(d.next_state(3, 30., deg(9.), true), 4);
        assert_eq!(d.next_state(3, 30., deg(30.), false), 4);
        assert_eq!(d.next_state(4, 30., deg(30.), true), 1);
    }

    #[test]
    fn drifting_grip_holds_the_angle_range_and_the_wall() {
        let d = default_drift();
        let car = [0.9, 4.5];
        let open = 20.;
        // Rear L/R then front L/R; the front gets driftGripFront 10.
        assert_eq!(d.drifting_grip(10., open, car), [0.5, 0.5, 10., 10.]);
        assert_eq!(d.drifting_grip(20., open, car), [1., 1., 10., 10.]);
        assert_eq!(d.drifting_grip(30., open, car)[0], 1.);
        // Above the range toward driftAngleMax: 1 -> driftMaxGrip 3.
        assert!((d.drifting_grip(45., open, car)[0] - 2.).abs() < 1e-5);
        assert_eq!(d.drifting_grip(70., open, car)[0], 3.);
        // A wall 2.4 m away caps the range at asin(1.5 / 5.4) = 16.1 degrees.
        let near = d.drifting_grip(20., 2.4, car)[0];
        let expected = 1. + 2. * (20. - 16.128) / (60. - 16.128);
        assert!((near - expected).abs() < 1e-3, "{near} {expected}");
        // Drift skill scales the bottom of the range.
        let unskilled = Drift {
            skill: 0.5,
            ..default_drift()
        };
        assert_eq!(unskilled.drifting_grip(10., open, car)[0], 1.);
    }

    #[test]
    fn airborne_steering_follows_the_slip_angle() {
        // Nose along +Z, car left along +X (+E30 positive = left).
        let (forward, left) = ([0., 0., 1.], [1., 0., 0.]);
        let quarter = std::f32::consts::FRAC_PI_4;
        assert!((air_slip([1., -3., 1., 0.], forward, left) - quarter).abs() < 1e-6);
        assert!((air_slip([-1., 5., 1., 0.], forward, left) + quarter).abs() < 1e-6);
        // Straight backwards: velocity not to the left, so −π as in 0x77AC50.
        assert!((air_slip([0., 0., -2., 0.], forward, left) + std::f32::consts::PI).abs() < 1e-6);
        // Vertical only or at rest: no direction.
        assert_eq!(air_slip([0., -9., 0., 0.], forward, left), 0.);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn the_start_line_is_read_where_a_route_ships_one() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        // Shibuya ships a `start` line on every gate; before the start line
        // the cars follow it (0x794870 kind 7) instead of the optimal line,
        // whose last-hairpin exit cuts across the grid (F635).
        let shibuya = AiData::load(root, "tracks/japan/shibuya/route_0", "touring", "bmw").unwrap();
        let start = shibuya.start.as_deref().unwrap();
        assert_eq!(start.line.points.len(), shibuya.line.points.len());
        assert!(std::ptr::eq(shibuya.on(RacingLine::Start), start));
        // Routes with the optimal kind only keep it before the start line.
        for route in ["tracks/europe/donington/route_0", "tracks/europe/milan/route_0"] {
            let data = AiData::load(root, route, "touring", "bmw").unwrap();
            assert!(data.start.is_none(), "{route}");
            assert!(std::ptr::eq(data.on(RacingLine::Start), &data));
        }
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT"]
    fn drift_battle_config_switches_on_the_drift_behaviour() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let root = Path::new(&root);
        for route in [
            "tracks/usa/long_beach/route_1",
            "tracks/europe/nurburgring/route_1",
            "tracks/europe/istanbul_park/route_1",
            "tracks/europe/jarama/route_0",
            "tracks/europe/jarama/route_1",
        ] {
            let race = AiData::load(root, route, "touring", "bmw").unwrap();
            let drift = AiData::load_for(root, route, "touring", "bmw", Some("drift_battle")).unwrap();
            let p = Parameters::new(&race, NORMAL_DIFFICULTY, |_| 1.).unwrap();
            let q = Parameters::new(&drift, NORMAL_DIFFICULTY, |_| 1.).unwrap();
            // A race keeps the defaults: no drift behaviour or line.
            assert!(race.drift.is_none() && p.drift.is_none());
            assert!(p.race_damaged && p.controlled_stop);
            assert_eq!(p.la[0], 10.);
            // drift_battle: drift behaviour and test on, race_damaged and
            // controlled_stop off, laDistMin 15, the drift line [D].
            let d = q.drift.clone().unwrap();
            assert!(!q.race_damaged && !q.controlled_stop);
            assert_eq!(q.la[0], 15.);
            assert_eq!(d.hold_time, 0.2);
            assert_eq!(
                (d.angle_range, d.grip_front, d.start_angle, d.end_angle),
                ([15., 30.], 10., 15., 10.)
            );
            assert_eq!((d.initiate_grip, d.racing_grip, d.brake_line_advance), (1.2, 1.1, 8.));
            assert_eq!((d.skill, d.position), (1., 0.5));
            assert_eq!(q.tg_non_optimal_speed_factor, 0.8);
            let line = &drift.drift.as_ref().unwrap().line;
            assert_eq!(line.points.len(), drift.line.points.len());
            assert_ne!(line.points, drift.line.points, "{route}");
            assert!((line.length - drift.line.length).abs() < 0.05 * drift.line.length);
            // Mapping overrides of the section (deviation large at
            // difficulty 0.606 and characteristic 1: 3 + (1 - 3) * 0.606).
            assert!((q.deviation_large - (3. - 2. * NORMAL_DIFFICULTY)).abs() < 1e-5);
        }
    }
}
