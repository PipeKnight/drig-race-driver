//! The player car's damage and suspension sounds (docs/AUDIO_RESEARCH.md
//! §27): the engine damage loop 44DF60, the brake damage loops 453A70/44DB50,
//! the suspension one-shots 4560E0/4441F0 with the damage one-shot 453DA0, and
//! the steering damage loop 4562F0/454430. 457BE0 runs them for the player
//! only, once a frame; here once a vehicle tick. Presentation only.
use crate::collision_audio::{Bank, Curve, DISPLACEMENT, Listener};

/// Mix groups (43B1F0): car-audio +0x10 Engine and +8 Other Mech; +0xC is
/// Displacement.
pub use crate::audio_mix::{ENGINE, OTHER_MECH};

/// Variety, CollisionInfo and ImpactVariety names (43CF10, read through
/// P = M+0x70).
pub const ENGINE_LOOP: &str = "damageEngineLoop";
pub const BRAKE: &str = "DamageBreak";
pub const SUSPENSION_DAMAGE: &str = "DamageSuspension";
pub const SUSPENSION_UP: &str = "SuspensionUp";
pub const SUSPENSION_DOWN: &str = "SuspensionDown";
pub const STEERING: &str = "DamageSteering";

/// Loop slots: the engine loop, the four wheels' brake loops, the steering loop.
pub const ENGINE_SLOT: usize = 0;
pub const BRAKE_SLOT: usize = 1;
pub const STEERING_SLOT: usize = 5;
pub const SLOTS: usize = 6;

/// Where a sound sits on the car.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    /// Car-audio +0x7B0, the carsetup EngineBay offset.
    EngineBay,
    /// Car-audio +0x7E0, the carsetup Cockpit offset.
    Cockpit,
    /// Wheel i (453A70 and 4560E0 transform the wheel record by the car
    /// matrix +0xA3F0).
    Wheel(usize),
}

/// What the damage sounds read of the player's car, one vehicle tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    /// 99A4D0(3): the engine temperature level (vehicle +0xCDFC).
    pub engine: f32,
    /// 9961D0(2): the brake (vehicle +0xE20).
    pub brake: f32,
    /// 9961D0(6): the steering times [CB0948] = −1 (vehicle +0xE30).
    pub steering: f32,
    /// Vehicle +0x1894 / +0x17A4: the engine speed over its maximum.
    pub revs: f32,
    /// 99A540(i): the wheels' damage levels (vehicle +0xCE08).
    pub wheel_damage: [f32; 4],
    /// 99A580(i): the suspension damage levels (vehicle +0xCE18).
    pub suspension_damage: [f32; 4],
    /// 99A5C0(i): the brake damage levels (vehicle +0xCE28).
    pub brake_damage: [f32; 4],
    /// Wheel record +0x40 (vehicle +0x1990 + 0x150·i): the suspension travel.
    pub travel: [f32; 4],
    /// Wheel record +0xC8 is non-zero.
    pub grounded: [bool; 4],
    /// |velocity| of the physical body (A69800, body +0x220; car-audio +0xA480).
    pub speed: f32,
}

impl Input {
    /// From the player's vehicle words and its body velocity (four floats).
    pub fn vehicle(words: &[u32], velocity: [f32; 4]) -> Self {
        let f = |o: usize| words.get(o / 4).copied().map_or(0., f32::from_bits);
        let wheel = |i: usize, o: usize| 0x1990 + 0x150 * i + o;
        let limit = f(0x17a4);
        Self {
            engine: f(0xcdfc),
            brake: f(0xe20),
            steering: -f(0xe30),
            revs: if limit != 0. { f(0x1894) / limit } else { 0. },
            wheel_damage: std::array::from_fn(|i| f(0xce08 + 4 * i)),
            suspension_damage: std::array::from_fn(|i| f(0xce18 + 4 * i)),
            brake_damage: std::array::from_fn(|i| f(0xce28 + 4 * i)),
            travel: std::array::from_fn(|i| f(wheel(i, 0x40))),
            grounded: std::array::from_fn(|i| words.get(wheel(i, 0xc8) / 4).is_some_and(|&w| w != 0)),
            speed: velocity.iter().map(|v| v * v).sum::<f32>().sqrt(),
        }
    }
}

/// A loop this tick: it starts on its first tick, then level and pitch follow.
#[derive(Clone, Debug, PartialEq)]
pub struct Loop {
    pub slot: usize,
    pub variety: String,
    /// Clamped by 444280 to −48…0 dB.
    pub volume_db: f32,
    /// Clamped by 444280 to 0.5…2.
    pub pitch: f32,
    pub place: Place,
    pub group: usize,
}

/// A one-shot this tick.
#[derive(Clone, Debug, PartialEq)]
pub struct Shot {
    pub variety: String,
    pub volume_db: f32,
    pub pitch: f32,
    pub place: Place,
    pub group: usize,
}

/// One tick's sounds: loops playing, loops to stop, one-shots to start.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    pub loops: Vec<Loop>,
    pub stops: Vec<usize>,
    pub shots: Vec<Shot>,
}

/// The car-audio state of these functions.
#[derive(Clone, Debug, Default)]
pub struct Damage {
    /// Voices started: +0xA3A4 engine, +0xA340 + 4i brakes, +0xA370 steering.
    playing: [bool; SLOTS],
    /// +0xA350 + 4i: each wheel's smoothed brake.
    brakes: [f32; 4],
    /// +0xA5C8 + 4i: each wheel's travel of the previous tick.
    travel: [f32; 4],
    /// Manager +0x4E68 + 6·slot + i: each wheel's suspension one-shot latch.
    latched: [bool; 4],
    /// +0xA360 + 4i: seconds each wheel's damage one-shot still plays.
    suspension: [f32; 4],
    /// +0xA4A8: the smoothed steering.
    steering: f32,
    /// +0xA4AC: the grind counter.
    grind: f32,
}

fn lin2db(x: f32) -> f32 {
    if x > 0. { 20. * x.log10() } else { -96. }
}

/// 444280's clamps.
fn clamped(volume_db: f32, pitch: f32) -> (f32, f32) {
    (if volume_db > 0. { 0. } else { volume_db.max(-48.) }, pitch.clamp(0.5, 2.))
}

/// 44ED30/44EEB0/4441F0: of an ImpactVariety's soft, med and hard infos, the
/// first of soft and med whose range holds x, else hard.
fn pick<'b>(bank: &'b Bank, variety: &str, x: f32) -> Option<(&'b str, &'b Curve)> {
    let three = bank.impacts.get(variety)?;
    let curve = |k: usize| bank.infos.get(&three[k]).map(|c| (three[k].as_str(), c));
    [curve(0), curve(1)].into_iter().flatten().find(|(_, c)| c.contains(x)).or_else(|| curve(2))
}

/// The longest sample of `variety`, in seconds.
fn length(bank: &Bank, variety: &str) -> f32 {
    let Some(v) = bank.varieties.get(variety) else { return 0. };
    v.samples
        .iter()
        .filter_map(|&s| bank.sounds.get(s))
        .map(|s| (s.pcm.len() / s.channels.max(1)) as f32 / s.rate.max(1) as f32)
        .fold(0., f32::max)
}

impl Damage {
    /// 457BE0's player calls in order (4560E0, 44DF60, 453A70, 4562F0) for one
    /// tick of `dt` seconds. The suspension one-shots go through `listener`'s
    /// 435EA0 cooldowns.
    pub fn frame(&mut self, bank: &Bank, listener: &mut Listener, input: &Input, dt: f32) -> Frame {
        let mut out = Frame::default();
        self.suspension(bank, listener, input, dt, &mut out);
        self.engine(input, &mut out);
        self.brakes(input, &mut out);
        self.steer(bank, input, &mut out);
        out
    }

    /// Starts slot's voice; true on its first tick.
    fn start(&mut self, slot: usize) -> bool {
        !std::mem::replace(&mut self.playing[slot], true)
    }

    fn stop(&mut self, slot: usize, out: &mut Frame) {
        if std::mem::take(&mut self.playing[slot]) {
            out.stops.push(slot);
        }
    }

    /// 44DF60: the engine damage loop while the engine temperature level is
    /// above 0.1. The first tick only starts it.
    fn engine(&mut self, input: &Input, out: &mut Frame) {
        let level = input.engine;
        if level <= 0.1 {
            self.stop(ENGINE_SLOT, out);
            return;
        }
        let (volume_db, pitch) = if self.start(ENGINE_SLOT) {
            (-48., 1.)
        } else {
            let r = input.revs;
            let v = if r >= 0.3 { (1. - (r - 0.3)).powi(2) } else { 1. };
            // [+0xA494] +0xD8 silences it [I: a flag not traced; never set here].
            clamped(lin2db(v.min(level).max(0.)), (r - 0.3) * 0.5 + 0.5)
        };
        out.loops.push(Loop {
            slot: ENGINE_SLOT,
            variety: ENGINE_LOOP.into(),
            volume_db,
            pitch,
            place: Place::EngineBay,
            group: ENGINE,
        });
    }

    /// 453A70/44DB50: while the brake is above 0.1, a loop at each wheel
    /// whose brake damage is above 0.1. The first tick starts it at −48 dB.
    fn brakes(&mut self, input: &Input, out: &mut Frame) {
        let b = input.brake;
        for i in 0..4 {
            let slot = BRAKE_SLOT + i;
            let damage = input.brake_damage[i];
            if b <= 0.1 {
                self.stop(slot, out);
                continue;
            }
            if damage <= 0.1 {
                continue;
            }
            let (volume_db, pitch) = if self.start(slot) {
                (-48., 1.)
            } else {
                if !input.grounded[i] {
                    self.brakes[i] = 0.;
                }
                self.brakes[i] += (b - self.brakes[i]) * 0.2;
                let s = input.speed;
                // 413130: a clamped lerp; 1.05 down to 0.975 over 0–5 m/s,
                // then 0.975 up to 1.025 over 22.5–40 m/s.
                let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t.clamp(0., 1.);
                let (x, pitch) = if s < 5. {
                    (s * 0.2, lerp(1.05, 0.975, s * 0.2))
                } else {
                    (1., lerp(0.975, 1.025, (s - 22.5) * 0.057_142_857))
                };
                clamped(lin2db(self.brakes[i] * x * damage * 0.75), pitch - 0.05 + b * 0.05)
            };
            out.loops.push(Loop { slot, variety: BRAKE.into(), volume_db, pitch, place: Place::Wheel(i), group: OTHER_MECH });
        }
    }

    /// 4560E0: per wheel, the travel's change ×100 plays SuspensionUp (up) or
    /// SuspensionDown (down) once above the soft info's minVel, latched until
    /// it falls back; then 453DA0. An unchanged wheel ends the tick's loop.
    fn suspension(&mut self, bank: &Bank, listener: &mut Listener, input: &Input, dt: f32, out: &mut Frame) {
        for t in &mut self.suspension {
            *t = (*t - dt).max(0.);
        }
        for i in 0..4 {
            let u = (input.travel[i] - self.travel[i]) * 100.;
            self.travel[i] = input.travel[i];
            if u == 0. {
                return;
            }
            let soft = |variety: &str| {
                bank.impacts.get(variety).and_then(|three| bank.infos.get(&three[0])).map(|c| c.min_vel)
            };
            let bump = if u > 0. && soft(SUSPENSION_UP).is_some_and(|m| u > m) {
                Some((SUSPENSION_UP, u))
            } else if u < 0. && soft(SUSPENSION_DOWN).is_some_and(|m| -u > m) {
                Some((SUSPENSION_DOWN, -u))
            } else {
                self.latched[i] = false;
                None
            };
            // 4441F0: once per latch.
            if let Some((variety, x)) = bump
                && !std::mem::replace(&mut self.latched[i], true)
                && let Some((info, _)) = pick(bank, variety, x)
                && let Some(play) = listener.cooled(info, x, DISPLACEMENT)
            {
                out.shots.push(Shot {
                    variety: play.variety,
                    volume_db: play.volume_db,
                    pitch: play.pitch,
                    place: Place::Wheel(i),
                    group: DISPLACEMENT,
                });
            }
            self.suspension_damage(bank, input, i, u * 0.01, out);
        }
    }

    /// 453DA0: at suspension damage 0.2 or more, the DamageSuspension one-shot
    /// with x = damage·|velocity|·|travel change|, unless the wheel's voice
    /// still plays [I: 44F160 then updates the playing voice; not ported].
    fn suspension_damage(&mut self, bank: &Bank, input: &Input, i: usize, change: f32, out: &mut Frame) {
        let damage = input.suspension_damage[i];
        if damage < 0.2 || self.suspension[i] > 0. {
            return;
        }
        let Some(curve) = bank.infos.get(SUSPENSION_DAMAGE) else { return };
        let (volume_db, pitch) = curve.at(damage * input.speed * change.abs());
        self.suspension[i] = length(bank, &curve.variety);
        out.shots.push(Shot { variety: curve.variety.clone(), volume_db, pitch, place: Place::Wheel(i), group: DISPLACEMENT });
    }

    /// 4562F0/454430: while the larger of wheels 2 and 3's damage is above 0.3
    /// and the steering above 0.05, the DamageSteering loop at the cockpit by
    /// the smoothed steering, with a grind above End's minVel + 0.1.
    fn steer(&mut self, bank: &Bank, input: &Input, out: &mut Frame) {
        let damage = input.wheel_damage[2].max(input.wheel_damage[3]);
        let s = input.steering;
        if damage <= 0.3 || s.abs() <= 0.05 {
            self.stop(STEERING_SLOT, out);
            return;
        }
        self.steering += (s - self.steering) * 0.1;
        let mut x = self.steering.abs();
        if !self.start(STEERING_SLOT) {
            let k = if x == 0. { 0. } else { 0.1 };
            let end = bank.impacts.get(STEERING).and_then(|three| bank.infos.get(&three[1]));
            if end.is_some_and(|end| x > end.min_vel + k) {
                // BF4E44 is sin; the counter grows by [CAF888] = 1 a tick.
                self.grind += 1.;
                x += (self.grind * 0.333_333_34).sin() * (self.grind * 0.1).sin() * k;
            }
        }
        let Some((_, curve)) = pick(bank, STEERING, x) else { return };
        let (volume_db, pitch) = curve.at(x);
        out.loops.push(Loop {
            slot: STEERING_SLOT,
            variety: curve.variety.clone(),
            volume_db,
            pitch,
            place: Place::Cockpit,
            group: OTHER_MECH,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collision_audio::Variety;
    use std::{collections::HashMap, path::Path, sync::Arc};

    fn curve(variety: &str, vel: [f32; 2], vol: [f32; 2], pitch: [f32; 2]) -> Curve {
        Curve {
            variety: variety.into(),
            min_vel: vel[0],
            max_vel: vel[1],
            min_vol: vol[0],
            max_vol: vol[1],
            min_pitch: pitch[0],
            max_pitch: pitch[1],
        }
    }

    fn bank() -> Arc<Bank> {
        let three = |a: &str, b: &str, c: &str| [a.to_string(), b.into(), c.into()];
        Arc::new(Bank {
            sounds: Vec::new(),
            varieties: HashMap::from([(
                "SuspensionDamage".to_string(),
                Variety { samples: Vec::new(), min_distance: 4., max_distance: 50. },
            )]),
            infos: HashMap::from([
                ("SusUpSoft".into(), curve("UpSoft", [10., 20.], [-12., -6.], [1., 1.])),
                ("SusUpMed".into(), curve("UpMed", [20., 40.], [-6., -3.], [1., 1.])),
                ("SusUpHard".into(), curve("UpHard", [40., 80.], [-3., 0.], [1., 1.])),
                ("SusDownSoft".into(), curve("DownSoft", [10., 20.], [-12., -6.], [1., 1.])),
                ("SusDownMed".into(), curve("DownMed", [20., 40.], [-6., -3.], [1., 1.])),
                ("SusDownHard".into(), curve("DownHard", [40., 80.], [-3., 0.], [1., 1.])),
                (SUSPENSION_DAMAGE.into(), curve("SuspensionDamage", [0.01, 2.5], [-48., 0.], [0.9, 1.1])),
                ("DamageSteeringStart".into(), curve(STEERING, [0., 0.8], [-50., -2.], [1., 1.])),
                ("DamageSteeringEnd".into(), curve(STEERING, [0.8, 1.], [-30., -2.], [1., 1.])),
                ("DamageSteeringEndDammy".into(), curve(STEERING, [1., 1.], [-40., -2.], [1., 1.])),
            ]),
            impacts: HashMap::from([
                (SUSPENSION_UP.into(), three("SusUpSoft", "SusUpMed", "SusUpHard")),
                (SUSPENSION_DOWN.into(), three("SusDownSoft", "SusDownMed", "SusDownHard")),
                (STEERING.into(), three("DamageSteeringStart", "DamageSteeringEnd", "DamageSteeringEndDammy")),
            ]),
            car_scrape: HashMap::new(),
        })
    }

    #[test]
    fn the_engine_loop_follows_44df60() {
        let bank = bank();
        let mut listener = Listener::new(bank.clone());
        let mut damage = Damage::default();
        let mut input = Input { engine: 0.1, revs: 0.5, ..Input::default() };
        assert_eq!(damage.frame(&bank, &mut listener, &input, 0.016), Frame::default());
        input.engine = 0.5;
        let first = damage.frame(&bank, &mut listener, &input, 0.016);
        assert_eq!((first.loops[0].volume_db, first.loops[0].pitch), (-48., 1.));
        // v = min((1 − 0.2)², 0.5) = 0.5; pitch (0.5 − 0.3)·0.5 + 0.5.
        let next = damage.frame(&bank, &mut listener, &input, 0.016);
        let l = &next.loops[0];
        assert_eq!((l.slot, l.place, l.group), (ENGINE_SLOT, Place::EngineBay, ENGINE));
        assert!((l.volume_db - 20. * 0.5f32.log10()).abs() < 1e-4 && (l.pitch - 0.6).abs() < 1e-6);
        input.engine = 0.05;
        assert_eq!(damage.frame(&bank, &mut listener, &input, 0.016).stops, vec![ENGINE_SLOT]);
    }

    #[test]
    fn a_damaged_brake_grinds_while_braking() {
        let bank = bank();
        let mut listener = Listener::new(bank.clone());
        let mut damage = Damage::default();
        let mut input = Input { brake: 0.5, speed: 10., grounded: [true; 4], ..Input::default() };
        input.brake_damage[1] = 0.5;
        let first = damage.frame(&bank, &mut listener, &input, 0.016);
        assert_eq!(first.loops.len(), 1);
        assert_eq!((first.loops[0].slot, first.loops[0].volume_db), (BRAKE_SLOT + 1, -48.));
        // Smoothed 0.5·0.2 = 0.1: lin2db(0.1·1·0.5·0.75); pitch 0.975 − 0.05 + 0.025.
        let l = damage.frame(&bank, &mut listener, &input, 0.016).loops.remove(0);
        assert!((l.volume_db - 20. * 0.0375f32.log10()).abs() < 1e-3, "{l:?}");
        assert!((l.pitch - 0.95).abs() < 1e-6 && l.place == Place::Wheel(1) && l.group == OTHER_MECH);
        input.brake = 0.05;
        assert_eq!(damage.frame(&bank, &mut listener, &input, 0.016).stops, vec![BRAKE_SLOT + 1]);
    }

    #[test]
    fn suspension_bumps_play_once_per_latch() {
        let bank = bank();
        let mut listener = Listener::new(bank.clone());
        let mut damage = Damage::default();
        // 0.15 m in a tick: u = 15 on wheel 0, soft.
        let mut input = Input { travel: [0.15, 0., 0., 0.], ..Input::default() };
        let shots = damage.frame(&bank, &mut listener, &input, 0.016).shots;
        assert_eq!(shots.len(), 1);
        assert_eq!((shots[0].variety.as_str(), shots[0].place, shots[0].group), ("UpSoft", Place::Wheel(0), DISPLACEMENT));
        // Still rising: latched, no second shot.
        input.travel = [0.3, 0., 0., 0.];
        assert!(damage.frame(&bank, &mut listener, &input, 1.).shots.is_empty());
        // Falling fast: SuspensionDown; wheel 1 unchanged ends the loop.
        input.travel = [0.0, 0., 0., 0.];
        input.suspension_damage = [0.5; 4];
        input.speed = 10.;
        let shots = damage.frame(&bank, &mut listener, &input, 1.).shots;
        let names: Vec<_> = shots.iter().map(|s| s.variety.as_str()).collect();
        // The latch clears only below the thresholds, so the down shot waits.
        assert_eq!(names, ["SuspensionDamage"]);
    }

    #[test]
    fn steering_damage_needs_a_damaged_rear_wheel() {
        let bank = bank();
        let mut listener = Listener::new(bank.clone());
        let mut damage = Damage::default();
        let mut input = Input { steering: 0.5, ..Input::default() };
        assert!(damage.frame(&bank, &mut listener, &input, 0.016).loops.is_empty());
        input.wheel_damage[3] = 0.5;
        let l = damage.frame(&bank, &mut listener, &input, 0.016).loops.remove(0);
        // Smoothed 0.05: the Start info at x = 0.05.
        assert_eq!((l.slot, l.place, l.variety.as_str()), (STEERING_SLOT, Place::Cockpit, STEERING));
        assert!((l.volume_db - (-50. + 48. * 0.05 / 0.8)).abs() < 1e-3, "{l:?}");
        input.steering = 0.;
        assert_eq!(damage.frame(&bank, &mut listener, &input, 0.016).stops, vec![STEERING_SLOT]);
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn shipped_damage_sounds_load() {
        let root = std::env::var_os("GRID_GAME_ROOT").unwrap();
        let bank = Bank::load(Path::new(&root)).unwrap();
        for variety in [ENGINE_LOOP, BRAKE, STEERING, "SuspensionDamage"] {
            let v = &bank.varieties[variety];
            assert!(!v.samples.is_empty(), "{variety}");
        }
        for impact in [SUSPENSION_UP, SUSPENSION_DOWN, STEERING] {
            for info in &bank.impacts[impact] {
                let c = &bank.infos[info];
                println!("{impact} {info} {} vel {}..{} vol {}..{}", c.variety, c.min_vel, c.max_vel, c.min_vol, c.max_vol);
                assert!(!bank.varieties[&c.variety].samples.is_empty(), "{info}");
            }
        }
        assert_eq!(bank.infos[BRAKE].variety, BRAKE);
        assert_eq!(bank.infos[SUSPENSION_DAMAGE].variety, "SuspensionDamage");
    }
}
