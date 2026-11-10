//! The drift OSD of dial2.pssg, driven by the player's drift scorer like the
//! original's per-frame bridge 0x6BB130 (docs/RACE_HUD.md "Drift OSD"):
//! the meter (`object/drift_main`, 0x6B8D60), the points box
//! (`object/points_main`, 0x6BB650) and the combo panel
//! (`object/driftcombo_main`, the `drifteffect` attach 0x6B5650/0x6B9640).
//! [D] marks traced behaviour, [I] the port's reading of it.

use crate::drift_event::{DriftEvent, grouped};
use crate::osd_scene::Osd;
use bevy::prelude::*;
use grid_tools::drift::{Scorer, Settings};

pub(crate) const METER: &str = "object/drift_main=drift";
pub(crate) const POINTS: &str = "object/points_main=points";
pub(crate) const COMBO: &str = "object/driftcombo_main=drift_combo";

/// The eight combo timer segments in the order they light, 1..8 [D binding
/// order 0x6B5650]; they go out from the last to the first.
const SEGMENTS: [&str; 8] = [
    "object/driftcombo_timer_a=1",
    "object/driftcombo_timer_b=2",
    "object/driftcombo_timer_b=3",
    "object/driftcombo_timer_a=4",
    "object/driftcombo_timer_a=5",
    "object/driftcombo_timer_b=6",
    "object/driftcombo_timer_b=7",
    "object/driftcombo_timer_a=8",
];
const MULTIPLIER_SLOTS: [&str; 2] = [
    "object/driftcombo_multi_whole=multiplier_1",
    "object/driftcombo_multi_whole=multiplier_2",
];
const TYPE_SLOTS: [&str; 2] = [
    "object/driftcombo_drift_type_whole=drift_type_1",
    "object/driftcombo_drift_type_whole=drift_type_2",
];
/// Text and echo of each drift-type slot.
const TYPE_TEXTS: [[&str; 2]; 2] = [["drift_text", "drift_text_echo"], ["drift_text!0", "drift_text_echo!0"]];
/// Seconds the combo panel stays after a bank or a fail [D 2.0 at 0x6B9850/0x6B9910].
const EXIT_TIME: f32 = 2.;
/// Seconds the close-drift bonus stays [D 1.0, OSD +0x45C].
const BONUS_TIME: f32 = 1.;
/// Meter smoothing per original frame [D 0.3 at 0xDB52C8]; the port applies
/// it at a 60 Hz reference rate so it does not depend on the frame rate [I].
const SMOOTHING: f32 = 0.3;
const REFERENCE_HZ: f32 = 60.;

/// What the drift OSD does to the scene: `Osd` in the game, a recorder in
/// the tests.
pub(crate) trait Sink {
    fn play(&mut self, anim: &str, range: &str);
    /// Seeks an animation to a 0..1 fraction of its length (0x6C1910).
    fn fraction(&mut self, anim: &str, v: f32);
    fn text(&mut self, node: &str, text: &str);
    fn show(&mut self, node: &str, on: bool);
    /// Sets a node's own `scrollY` (a digit glyph, 0x6A7AC0).
    fn scroll(&mut self, node: &str, y: f32);
}

impl Sink for Osd {
    fn play(&mut self, anim: &str, range: &str) {
        Osd::play(self, anim, range);
    }
    fn fraction(&mut self, anim: &str, v: f32) {
        let d = self.duration(anim);
        self.seek(anim, d * v.clamp(0., 1.));
    }
    fn text(&mut self, node: &str, text: &str) {
        self.set_text(node, text);
    }
    fn show(&mut self, node: &str, on: bool) {
        Osd::show(self, node, on);
    }
    fn scroll(&mut self, node: &str, y: f32) {
        self.scroll_y(node, y);
    }
}

/// Localised strings the drift OSD shows.
pub(crate) struct Texts<'a> {
    pub points: &'a str,
    pub perfect: &'a str,
    /// Rating names (drift.xml `DriftType` names, localised).
    pub ratings: Vec<&'a str>,
}

#[derive(Default)]
pub(crate) struct DriftOsd {
    started: bool,
    smoothed: f32,
    previous: f32,
    bars: [f32; 2],
    max: [f32; 2],
    /// The combo is open (OSD +0x468 bit 0) and the ring runs (bit 1).
    open: bool,
    ring: bool,
    lit: usize,
    fraction: f32,
    combo: i32,
    rating: usize,
    multiplier_slot: usize,
    type_slot: usize,
    close_drifts: i32,
    bonus: f32,
    exit: f32,
    total: f32,
}

impl DriftOsd {
    /// One frame. `dt` is seconds since the last frame.
    pub(crate) fn update(&mut self, s: &Scorer, settings: &Settings, texts: &Texts, dt: f32, sink: &mut impl Sink) {
        // Restart Race: the scorer starts again from zero.
        if self.started && s.total < self.total {
            *self = Self::default();
        }
        if !self.started {
            self.start(texts, sink);
        }
        self.total = s.total;
        // The 2 s exit counts from the frame after the bank or fail.
        if self.exit > 0. {
            self.exit -= dt;
            if self.exit <= 0. {
                sink.show(COMBO, false);
            }
        }
        self.meter(s.wrong_way, dt, sink);
        if s.combo_failed && self.open {
            self.end(false, s, sink);
        }
        if s.combo != 0 && !self.open {
            self.open_combo(sink);
        }
        if s.combo != self.combo {
            if s.combo > 0 {
                self.multiplier(s.combo, sink);
            }
            self.combo = s.combo;
        }
        if s.rating != self.rating {
            if let Some(name) = s.rating.checked_sub(1).and_then(|r| texts.ratings.get(r)) {
                self.drift_type(name, sink);
            }
            self.rating = s.rating;
        }
        if s.rating != 0 {
            sink.text("points_text", &format!("{}", s.current as i32));
        }
        self.timer_ring(s, settings, sink);
        self.close_bonus(s, texts, dt, sink);
    }

    /// Bind time [D 0x6BB650, 0x6B5650]: titles, a zero total, the points box
    /// out and the combo hidden.
    fn start(&mut self, texts: &Texts, sink: &mut impl Sink) {
        for node in ["points_title", "points_title_echo"] {
            sink.text(node, texts.points);
        }
        for node in ["points_total", "points_total_echo"] {
            sink.text(node, "0");
        }
        sink.play("points_anim", "out");
        sink.show(COMBO, false);
        self.bars = [-1., -1.];
        self.started = true;
    }

    /// The meter [D 0x6B8D60]: the smoothed signed slip angle moves the car
    /// needle; the bars split at the middle.
    fn meter(&mut self, angle: f32, dt: f32, sink: &mut impl Sink) {
        let k = 1. - (1. - SMOOTHING).powf(dt * REFERENCE_HZ);
        let s = self.smoothed + (angle.clamp(-1., 1.) - self.smoothed) * k;
        // The max markers reset when the drift swaps sides after a large
        // angle [D 0x6B4AA0]; otherwise they follow the largest bar [I].
        if s * self.previous < 0. && self.previous.abs() > 0.25 {
            self.reset_max(sink);
        }
        self.previous = self.smoothed;
        self.smoothed = s;
        let p = ((s + 1.) / 2.).clamp(0., 1.);
        sink.fraction("car", p);
        let bars = [(1. - 2. * p).clamp(0., 1.), (2. * p - 1.).clamp(0., 1.)];
        for (i, (anim, max_anim)) in [("meter_left", "meter_max_left"), ("meter_right", "meter_max_right")]
            .into_iter()
            .enumerate()
        {
            if bars[i] != self.bars[i] {
                sink.fraction(anim, bars[i]);
                self.bars[i] = bars[i];
            }
            if bars[i] > self.max[i] {
                self.max[i] = bars[i];
                sink.fraction(max_anim, bars[i]);
            }
        }
    }

    fn reset_max(&mut self, sink: &mut impl Sink) {
        self.max = [0., 0.];
        sink.fraction("meter_max_left", 0.);
        sink.fraction("meter_max_right", 0.);
    }

    /// A combo starts [D 0x6BB130 -> 0x6B9730, 0x6B4410]: the panel resets
    /// and plays `on`, the points box slides in with "X1".
    fn open_combo(&mut self, sink: &mut impl Sink) {
        sink.show(COMBO, true);
        self.exit = 0.;
        for slot in MULTIPLIER_SLOTS.iter().chain(&TYPE_SLOTS) {
            sink.play(slot, "off");
        }
        sink.play("drift_bg", "on");
        sink.play("object/driftcombo_combo_text=combo_text", "on");
        sink.text("multiplier_text!2", "X1");
        sink.play("points_anim", "in");
        sink.show("multiplier_bag", false);
        self.reset_max(sink);
        self.combo = 1;
        self.rating = 0;
        self.lit = 8;
        self.open = true;
    }

    /// The combo multiplier [D 0x6B4860, 0x6B6040]: "X%d" in the points box;
    /// from 2 to 99 the two panel slots alternate with the new digits.
    fn multiplier(&mut self, combo: i32, sink: &mut impl Sink) {
        sink.text("multiplier_text!2", &format!("X{combo}"));
        if (2..=99).contains(&combo) {
            sink.play(MULTIPLIER_SLOTS[self.multiplier_slot], "off");
            self.multiplier_slot ^= 1;
            digits(self.multiplier_slot, combo, sink);
            sink.play(MULTIPLIER_SLOTS[self.multiplier_slot], "on");
            sink.play("driftcombo_anim", "on");
        }
    }

    /// A new drift rating [D 0x6B5C60]: the two type slots alternate.
    fn drift_type(&mut self, name: &str, sink: &mut impl Sink) {
        sink.play(TYPE_SLOTS[self.type_slot], "off");
        self.type_slot ^= 1;
        for node in TYPE_TEXTS[self.type_slot] {
            sink.text(node, name);
        }
        sink.play(TYPE_SLOTS[self.type_slot], "on");
    }

    /// The ring of eight timer segments [D 0x6B8BB0, 0x6B99D0, 0x6B5EF0]:
    /// all lit while the combo timer is full, then one goes out every eighth
    /// of the wait. The original counts the segments down with its own clock
    /// and re-syncs when the timer rises; the port reads the lit count from
    /// the timer fraction, which goes out at the same moments [I].
    fn timer_ring(&mut self, s: &Scorer, settings: &Settings, sink: &mut impl Sink) {
        if !self.open {
            return;
        }
        let f = if settings.combo_time > 0. {
            (s.combo_timer / settings.combo_time).clamp(0., 1.)
        } else {
            0.
        };
        if f <= 0. {
            if self.ring || s.combo == 0 {
                self.end(true, s, sink);
            }
            return;
        }
        self.ring = f < 1.;
        let lit = (f * 8.).ceil() as usize;
        if lit > self.lit {
            for seg in &SEGMENTS[self.lit..lit] {
                sink.play(seg, "on");
            }
        }
        if lit < self.lit {
            for seg in SEGMENTS[lit..self.lit].iter().rev() {
                sink.play(seg, "off");
            }
        }
        self.lit = lit;
        self.fraction = f;
    }

    /// The combo ends [D 0x6BBF20]: banked (0x6B9850, 0x6B4700, 0x6B4570) or
    /// failed (0x6B9910, 0x6B4680); the panel leaves and hides after 2 s.
    fn end(&mut self, banked: bool, s: &Scorer, sink: &mut impl Sink) {
        if banked {
            sink.play(MULTIPLIER_SLOTS[self.multiplier_slot], "complete");
            sink.play(TYPE_SLOTS[self.type_slot], "off");
            sink.play("object/driftcombo_combo_text=combo_text", "complete");
            sink.play("drift_bg", "off");
            sink.play("driftcombo_anim", "off");
            let total = grouped(s.total as i64);
            for node in ["points_total", "points_total_echo"] {
                sink.text(node, &total);
            }
            sink.show("multiplier_bag", true);
            sink.play("multiplier_bag", "on");
        } else {
            sink.play(MULTIPLIER_SLOTS[self.multiplier_slot], "drop");
            sink.play(TYPE_SLOTS[self.type_slot], "drop");
            sink.play("driftcombo_timer_anim_notes", "drop");
            sink.play("object/driftcombo_combo_text=combo_text", "drop");
            sink.play("drift_bg", "off");
            sink.play("driftcombo_anim", "drop");
            sink.play("points_anim", "fail");
        }
        self.exit = EXIT_TIME;
        self.open = false;
        self.ring = false;
        self.combo = 0;
        self.rating = 0;
    }

    /// The close-drift bonus [D 0x6B5D10, 0x6AD0A0, 0x6AD000]: "%d" or the
    /// perfect text for 1 s. The original posts it when the close contact
    /// ends; the port's scorer keeps that contact private, so the count's
    /// rise triggers it [I].
    fn close_bonus(&mut self, s: &Scorer, texts: &Texts, dt: f32, sink: &mut impl Sink) {
        if s.close_drifts > self.close_drifts && s.last_close_bonus != 0. {
            let text = if s.last_close_distance == 0. {
                texts.perfect.to_string()
            } else {
                format!("{}", s.last_close_bonus as i32)
            };
            sink.text("bonus", &text);
            sink.play("redflag", "bonus");
            self.bonus = BONUS_TIME;
        }
        self.close_drifts = s.close_drifts;
        if self.bonus > 0. {
            self.bonus -= dt;
            if self.bonus <= 0. {
                sink.play("redflag", "bonus_off");
            }
        }
    }
}

/// The digit nodes of one multiplier slot: each a main glyph and its echo.
/// The double stash shows "X" and one digit, the treble stash "X" and two;
/// the "X" glyphs (`main_1`/`echo_1`) have no digit attach [D sceneconfig].
struct Digits {
    double: [&'static str; 2],
    treble: [&'static str; 2],
    units_double: [&'static str; 2],
    units_treble: [&'static str; 2],
    tens_treble: [&'static str; 2],
}
const DIGITS: [Digits; 2] = [
    Digits {
        double: ["main_stash_double", "echo_stash_double"],
        treble: ["main_stash_treble", "echo_stash_treble"],
        units_double: ["text_single", "text_echo"],
        units_treble: ["text_single!1", "text_echo!1"],
        tens_treble: ["text_single!3", "text_echo!3"],
    },
    Digits {
        double: ["main_stash_double!0", "echo_stash_double!0"],
        treble: ["main_stash_treble!0", "echo_stash_treble!0"],
        units_double: ["text_single!4", "text_echo!4"],
        units_treble: ["text_single!6", "text_echo!6"],
        tens_treble: ["text_single!8", "text_echo!8"],
    },
];
/// `scrollY` per digit of the glyph strip [D 0.1 × 0.91 at 0x6A7785].
const DIGIT_STEP: f32 = 0.091;

/// The multiplier digits of one panel slot [D 0x6A76E0]: below 10 the double
/// stash with the units, else the treble stash with tens and units; each
/// glyph and its echo scroll their strip to digit × 0.091.
fn digits(slot: usize, value: i32, sink: &mut impl Sink) {
    let d = &DIGITS[slot];
    let value = value.clamp(0, 99);
    let two = value >= 10;
    for (node, on) in d.double.iter().map(|n| (n, !two)).chain(d.treble.iter().map(|n| (n, two))) {
        sink.show(node, on);
    }
    let mut put = |nodes: &[&str; 2], digit: i32| {
        for n in nodes {
            sink.scroll(n, digit as f32 * DIGIT_STEP);
        }
    };
    if two {
        put(&d.units_treble, value % 10);
        put(&d.tens_treble, value / 10);
    } else {
        put(&d.units_double, value);
    }
}

/// Feeds the drift OSD from the player's scorer; the drift groups stay off
/// outside drift events.
pub(crate) fn drive(
    event: Option<Res<DriftEvent>>,
    osd: Option<ResMut<Osd>>,
    labels: Option<Res<crate::pause_menu::Labels>>,
    time: Option<Res<Time<Real>>>,
    mut state: Local<DriftOsd>,
) {
    let Some(mut osd) = osd else {
        return;
    };
    let Some(entry) = event.as_ref().and_then(|e| e.entries.first()) else {
        for node in [METER, POINTS, COMBO] {
            Sink::show(osd.as_mut(), node, false);
        }
        return;
    };
    let event = event.as_ref().expect("checked above");
    osd.show(METER, true);
    osd.show(POINTS, true);
    let fallback = crate::pause_menu::Labels(None);
    let labels = labels.as_deref().unwrap_or(&fallback);
    let texts = Texts {
        points: labels.get("osd_points"),
        perfect: labels.get("osd_perfect"),
        ratings: event.settings.types.iter().map(|t| labels.get(&t.name)).collect(),
    };
    let dt = time.map_or(0., |t| t.delta_secs());
    let before = (state.open, state.combo);
    state.update(&entry.scorer, &event.settings, &texts, dt, osd.as_mut());
    // One line per combo change for the window smoke (hud_window_check.py --expect-drift).
    if (state.open, state.combo) != before {
        let digits = &DIGITS[state.multiplier_slot];
        let shown = [digits.units_double, digits.units_treble, digits.tens_treble]
            .map(|n| osd.scroll_of(n[0]).map_or("-".to_string(), |y| format!("{y:.3}")));
        println!(
            "osd_drift open={} combo={} total={} scroll={}",
            state.open,
            state.combo,
            entry.scorer.total as i64,
            shown.join(",")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Recorder {
        played: Vec<(String, String)>,
        texts: HashMap<String, String>,
        shown: HashMap<String, bool>,
        fractions: HashMap<String, f32>,
        scrolls: HashMap<String, f32>,
    }

    impl Sink for Recorder {
        fn play(&mut self, anim: &str, range: &str) {
            self.played.push((anim.into(), range.into()));
        }
        fn fraction(&mut self, anim: &str, v: f32) {
            self.fractions.insert(anim.into(), v);
        }
        fn text(&mut self, node: &str, text: &str) {
            self.texts.insert(node.into(), text.into());
        }
        fn show(&mut self, node: &str, on: bool) {
            self.shown.insert(node.into(), on);
        }
        fn scroll(&mut self, node: &str, y: f32) {
            self.scrolls.insert(node.into(), y);
        }
    }

    impl Recorder {
        fn take(&mut self) -> Vec<(String, String)> {
            std::mem::take(&mut self.played)
        }
        fn played(&self, anim: &str, range: &str) -> bool {
            self.played.iter().any(|(a, r)| a == anim && r == range)
        }
    }

    fn texts() -> Texts<'static> {
        Texts { points: "POINTS", perfect: "PERFECT", ratings: vec!["DRIFT", "ACE DRIFT"] }
    }

    fn settings() -> Settings {
        Settings { combo_time: 4., ..Settings::default() }
    }

    #[test]
    fn a_combo_opens_counts_its_ring_down_and_banks() {
        let (settings, texts) = (settings(), texts());
        let (mut osd, mut rec) = (DriftOsd::default(), Recorder::default());
        let mut s = Scorer::new(false, None, true);
        osd.update(&s, &settings, &texts, 0.016, &mut rec);
        assert_eq!(rec.texts["points_title"], "POINTS");
        assert_eq!(rec.texts["points_total"], "0");
        assert!(!rec.shown[COMBO]);
        rec.take();
        // The first scored step opens the combo with the timer full.
        (s.combo, s.combo_timer, s.rating, s.current) = (1, 4., 1, 120.);
        osd.update(&s, &settings, &texts, 0.016, &mut rec);
        assert!(rec.shown[COMBO]);
        assert_eq!(rec.texts["multiplier_text!2"], "X1");
        assert!(rec.played("points_anim", "in"));
        assert_eq!(rec.texts["drift_text!0"], "DRIFT");
        assert!(rec.played(TYPE_SLOTS[1], "on"));
        assert_eq!(rec.texts["points_text"], "120");
        rec.take();
        // A second drift: x2 swaps the multiplier slot; a higher rating swaps the type slot.
        (s.combo, s.rating) = (2, 2);
        osd.update(&s, &settings, &texts, 0.016, &mut rec);
        assert_eq!(rec.texts["multiplier_text!2"], "X2");
        assert!(rec.played(MULTIPLIER_SLOTS[0], "off") && rec.played(MULTIPLIER_SLOTS[1], "on"));
        assert_eq!(rec.texts["drift_text"], "ACE DRIFT");
        assert!(rec.played("driftcombo_anim", "on"));
        rec.take();
        // The drift ends: segments go out from the eighth, one per eighth of
        // the wait; the first goes with the bank.
        s.rating = 0;
        let mut out = Vec::new();
        for i in 1..40 {
            s.combo_timer = 4. - i as f32 * 0.1;
            osd.update(&s, &settings, &texts, 0.1, &mut rec);
            out.extend(rec.take().into_iter().filter(|(a, r)| SEGMENTS.contains(&a.as_str()) && r == "off").map(|(a, _)| a));
        }
        let expected: Vec<String> = SEGMENTS[1..].iter().rev().map(|s| s.to_string()).collect();
        assert_eq!(out, expected);
        // f reached 0: banked. The total shows grouped; the panel hides 2 s later.
        s.total = 12345.;
        (s.combo, s.combo_timer) = (0, 0.);
        osd.update(&s, &settings, &texts, 0.1, &mut rec);
        assert_eq!(rec.texts["points_total"], "12,345");
        assert!(rec.played("multiplier_bag", "on"));
        assert!(rec.played(MULTIPLIER_SLOTS[1], "complete"));
        for _ in 0..19 {
            osd.update(&s, &settings, &texts, 0.1, &mut rec);
        }
        assert!(rec.shown[COMBO]);
        osd.update(&s, &settings, &texts, 0.11, &mut rec);
        assert!(!rec.shown[COMBO]);
    }

    #[test]
    fn a_failed_combo_drops() {
        let (settings, texts) = (settings(), texts());
        let (mut osd, mut rec) = (DriftOsd::default(), Recorder::default());
        let mut s = Scorer::new(false, None, true);
        (s.combo, s.combo_timer) = (3, 2.);
        osd.update(&s, &settings, &texts, 0.016, &mut rec);
        rec.take();
        (s.combo, s.combo_timer, s.combo_failed) = (0, 0., true);
        osd.update(&s, &settings, &texts, 0.016, &mut rec);
        assert!(rec.played("points_anim", "fail"));
        assert!(rec.played("driftcombo_anim", "drop"));
        assert!(!rec.played("multiplier_bag", "on"), "a fail banks nothing");
    }

    #[test]
    fn the_meter_follows_the_slip_angle_at_any_frame_rate() {
        let (settings, texts) = (settings(), texts());
        let mut s = Scorer::new(false, None, true);
        s.wrong_way = 1.;
        let run = |dt: f32, frames: usize| {
            let (mut osd, mut rec) = (DriftOsd::default(), Recorder::default());
            for _ in 0..frames {
                osd.update(&s, &settings, &texts, dt, &mut rec);
            }
            (rec.fractions["car"], rec.fractions["meter_right"], rec.fractions["meter_left"])
        };
        let (car60, right60, left60) = run(1. / 60., 6);
        let (car120, ..) = run(1. / 120., 12);
        assert!((car60 - car120).abs() < 1e-4, "{car60} vs {car120}");
        assert!(car60 > 0.8 && right60 > 0.6 && left60 == 0.);
        // 0.3 per 60 Hz frame: after one frame the needle is 30 % of the way.
        let (car, ..) = run(1. / 60., 1);
        assert!((car - 0.65).abs() < 1e-4);
    }

    #[test]
    fn a_restart_resets_the_points_box() {
        let (settings, texts) = (settings(), texts());
        let (mut osd, mut rec) = (DriftOsd::default(), Recorder::default());
        let mut s = Scorer::new(false, None, true);
        s.total = 500.;
        osd.update(&s, &settings, &texts, 0.016, &mut rec);
        rec.texts.insert("points_total".into(), "500".into());
        s.total = 0.;
        osd.update(&s, &settings, &texts, 0.016, &mut rec);
        assert_eq!(rec.texts["points_total"], "0");
        assert!(rec.played("points_anim", "out"));
    }

    #[test]
    fn a_close_drift_shows_its_bonus_for_a_second() {
        let (settings, texts) = (settings(), texts());
        let (mut osd, mut rec) = (DriftOsd::default(), Recorder::default());
        let mut s = Scorer::new(false, None, true);
        osd.update(&s, &settings, &texts, 0.1, &mut rec);
        (s.close_drifts, s.last_close_bonus, s.last_close_distance) = (1, 250., 1.5);
        osd.update(&s, &settings, &texts, 0.1, &mut rec);
        assert_eq!(rec.texts["bonus"], "250");
        assert!(rec.played("redflag", "bonus"));
        rec.take();
        for _ in 0..10 {
            osd.update(&s, &settings, &texts, 0.1, &mut rec);
        }
        assert!(rec.played("redflag", "bonus_off"));
        (s.close_drifts, s.last_close_distance) = (2, 0.);
        osd.update(&s, &settings, &texts, 0.1, &mut rec);
        assert_eq!(rec.texts["bonus"], "PERFECT");
    }

    #[test]
    fn the_multiplier_shows_one_or_two_digits() {
        let mut rec = Recorder::default();
        digits(0, 7, &mut rec);
        assert!(rec.shown["main_stash_double"] && rec.shown["echo_stash_double"]);
        assert!(!rec.shown["main_stash_treble"] && !rec.shown["echo_stash_treble"]);
        assert!((rec.scrolls["text_single"] - 7. * DIGIT_STEP).abs() < 1e-6);
        assert_eq!(rec.scrolls["text_single"], rec.scrolls["text_echo"]);
        let mut rec = Recorder::default();
        digits(1, 42, &mut rec);
        assert!(rec.shown["main_stash_treble!0"] && !rec.shown["main_stash_double!0"]);
        assert!((rec.scrolls["text_single!6"] - 2. * DIGIT_STEP).abs() < 1e-6);
        assert!((rec.scrolls["text_single!8"] - 4. * DIGIT_STEP).abs() < 1e-6);
        assert_eq!(rec.scrolls["text_echo!8"], rec.scrolls["text_single!8"]);
        assert!(!rec.scrolls.contains_key("text_single!7"), "the X glyph keeps its own scroll");
    }
}
