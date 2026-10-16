//! Player damage option and in-race damage indicators (docs/MECHANICAL_DAMAGE.md).
//!
//! The collision distribution (9E6660), component wear (9F3E80) and effects
//! (9C5B90) live in `collision_damage` and `vehicle_handling`; this module
//! holds the two pieces around them recovered for F333:
//! - `Damage::apply`: the `realistic_damage` profile option as FUN_00654C20
//!   writes it to the player car;
//! - `indicators`: the HUD damage icons of FUN_008EEF60 / FUN_008EEB80.
use crate::vehicle_handling::MUTABLE;

/// Profile option `realistic_damage` (FUN_00654C20). The option value 0 is
/// mode 1, value 1 is mode 2 and any other value is mode 0. The defaults
/// builder FUN_005F1100 stores 1, so a fresh profile races in mode 2.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Damage {
    /// Mode 0: car byte +B14 cleared. 9F3E80 returns at once, the body handler
    /// skips 9E6660, the loose-part callback 9A7670 returns and 9BD7F0 sets
    /// +C7AF, which disables bulk deformation. The single-player toggle never
    /// selects it; the network session override FUN_006572D0 does.
    Off,
    /// Mode 1, option value 0: Realistic Damage off, gain +430 = 1.0.
    Normal,
    /// Mode 2, option value 1 (profile default): gain +430 = 2.0.
    #[default]
    Realistic,
}

impl Damage {
    /// Car +155C as FUN_00654C20 stores it.
    pub fn mode(self) -> u32 {
        match self {
            Damage::Off => 0,
            Damage::Normal => 1,
            Damage::Realistic => 2,
        }
    }

    /// Mutable +430 for an active mode (constants CAF888 = 1.0, CB8DA4 = 2.0).
    pub fn gain(self) -> Option<f32> {
        match self {
            Damage::Off => None,
            Damage::Normal => Some(1.),
            Damage::Realistic => Some(2.),
        }
    }

    /// FUN_00654C20 stores on the player car: +155C mode, +B14 activation and,
    /// for an active mode, the mutable gain +430. Mode 0 keeps the old gain.
    /// `vehicle` may be the short body copy, which has no mutable block.
    pub fn apply(self, vehicle: &mut [u32]) {
        vehicle[0x155c / 4] = self.mode();
        // +B14 is the low byte of its word.
        let word = &mut vehicle[0xb14 / 4];
        *word = (*word & !255) | u32::from(self != Damage::Off);
        if let Some(gain) = self.gain()
            && let Some(slot) = vehicle.get_mut((MUTABLE + 0x430) / 4)
        {
            *slot = gain.to_bits();
        }
    }
}

/// HUD dial icons in FUN_008EEF60 order. The first four are the
/// `object/dial_damage` nodes engine, gears, steering and suspension of
/// Osd/sceneconfig.xml; the last four are the per-wheel `damage_wheels`
/// states (RL, RR, FL, FR).
pub const ICONS: [&str; 8] = [
    "engine",
    "gears",
    "steering",
    "suspension",
    "wheel RL",
    "wheel RR",
    "wheel FL",
    "wheel FR",
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Indicators {
    /// Indicator inputs: engine (mutable +C), gearbox (+14), steering, mean
    /// suspension (+28..), then the four wheel levels (+18..).
    pub values: [f32; 8],
    /// Icon stage 0, 0.2, 0.7 or 1.0 from the per-icon thresholds.
    pub grades: [f32; 8],
    /// FUN_008EEF60's result: the summed stages exceed 1e-4 (CBA1F0).
    pub any: bool,
}

fn f(b: &[u32], o: usize) -> f32 {
    f32::from_bits(b[o / 4])
}

/// Component thresholds CAFE60 0.05, CB22A4 0.4, CAFE64 0.75; wheel
/// thresholds are the globals E4FD4C 0.3, E4FD50 0.65, E4FD54 0.8 (their
/// image defaults; no writer was found).
#[allow(clippy::neg_cmp_op_on_partial_ord)] // The original's NaN outcomes.
pub fn grade(index: usize, value: f32) -> f32 {
    let (low, mid, high) = if index < 4 {
        (
            f32::from_bits(0x3d4c_cccd),
            f32::from_bits(0x3ecc_cccd),
            0.75,
        )
    } else {
        (
            f32::from_bits(0x3e99_999a),
            f32::from_bits(0x3f26_6666),
            f32::from_bits(0x3f4c_cccd),
        )
    };
    // Written with `<` only, as the original compares (a NaN gives 0).
    if !(low < value) {
        0.
    } else if !(mid < value) {
        f32::from_bits(0x3e4c_cccd)
    } else if !(high < value) {
        f32::from_bits(0x3f33_3333)
    } else {
        1.
    }
}

/// FUN_008EEF60 over the owner vehicle words (needs the mutable block).
#[allow(clippy::neg_cmp_op_on_partial_ord)] // COMISS/JA keeps the larger, ties take the new.
pub fn indicators(vehicle: &[u32]) -> Option<Indicators> {
    let count = *vehicle.get(0x1620 / 4)? as usize;
    if !(1..=4).contains(&count) || vehicle.len() < (MUTABLE + 0x460) / 4 {
        return None;
    }
    let m = |o: usize| f(vehicle, MUTABLE + o);
    let mut values = [0f32; 8];
    values[0] = m(0xc);
    values[1] = m(0x14);
    // Mean suspension level; the x87 sum is stored to f32 after each wheel,
    // last wheel first.
    let mut sum = 0f32;
    for i in (0..count).rev() {
        sum += m(0x28 + 4 * i);
    }
    values[3] = sum / count as f32;
    // Steering: largest wheel and suspension levels over the wheels whose
    // record +8 (the local anchor Z, forward) is positive, i.e. the fronts.
    let (mut wheel, mut suspension) = (0f32, 0f32);
    for i in (0..count).rev() {
        values[4 + i] = m(0x18 + 4 * i);
        if f(vehicle, 0x1990 + i * 0x150 + 8) > 0. {
            if !(wheel > m(0x18 + 4 * i)) {
                wheel = m(0x18 + 4 * i);
            }
            if !(suspension > m(0x28 + 4 * i)) {
                suspension = m(0x28 + 4 * i);
            }
        }
    }
    // pow(x, 2.0) (CB9B78) of an f32 widened to double is exact as x * x.
    let x =
        f64::from(suspension * f32::from_bits(0x3e4c_cccd) + wheel * f32::from_bits(0x3f4c_cccd));
    values[2] = (x * x) as f32;
    let mut grades = [0f32; 8];
    let mut total = 0f32;
    for i in (0..8).rev() {
        grades[i] = grade(i, values[i]);
        total += grades[i];
    }
    Some(Indicators {
        values,
        grades,
        any: f32::from_bits(0x38d1_b717) < total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn car() -> Vec<u32> {
        let mut v = vec![0; crate::vehicle_publication::VEHICLE_WORDS];
        v[0x1620 / 4] = 4;
        // Lacetti anchors: rear Z -1.365, front Z +1.210.
        for (i, z) in [-1.365f32, -1.365, 1.21, 1.21].into_iter().enumerate() {
            v[(0x1990 + i * 0x150 + 8) / 4] = z.to_bits();
        }
        v
    }
    fn set(v: &mut [u32], o: usize, x: f32) {
        v[(MUTABLE + o) / 4] = x.to_bits();
    }

    #[test]
    fn option_bytes_follow_654c20() {
        let mut v = car();
        v[0xb14 / 4] = 0x1234_ff78;
        set(&mut v, 0x430, 7.);
        Damage::Off.apply(&mut v);
        assert_eq!(v[0x155c / 4], 0);
        assert_eq!(v[0xb14 / 4], 0x1234_ff00);
        assert_eq!(f(&v, MUTABLE + 0x430), 7.);
        Damage::Normal.apply(&mut v);
        assert_eq!((v[0x155c / 4], v[0xb14 / 4]), (1, 0x1234_ff01));
        assert_eq!(f(&v, MUTABLE + 0x430), 1.);
        Damage::Realistic.apply(&mut v);
        assert_eq!(v[0x155c / 4], 2);
        assert_eq!(f(&v, MUTABLE + 0x430), 2.);
        assert_eq!(Damage::default(), Damage::Realistic);
        // The short body copy has no mutable block.
        let mut body = vec![0; crate::vehicle_force::BODY_WORDS];
        Damage::Realistic.apply(&mut body);
        assert_eq!(body[0x155c / 4], 2);
    }

    #[test]
    fn grades_use_the_recovered_thresholds() {
        let stage = [0., 0.2f32, 0.7f32, 1.];
        for (value, expected) in [
            (0.05, 0),
            (0.0501, 1),
            (0.4, 1),
            (0.41, 2),
            (0.75, 2),
            (0.76, 3),
        ] {
            assert_eq!(grade(0, value), stage[expected], "component {value}");
        }
        for (value, expected) in [
            (0.3, 0),
            (0.31, 1),
            (0.65, 1),
            (0.66, 2),
            (0.8, 2),
            (0.81, 3),
        ] {
            assert_eq!(grade(5, value), stage[expected], "wheel {value}");
        }
        assert_eq!(grade(0, f32::NAN), 0.);
    }

    #[test]
    fn indicators_follow_8eef60() {
        let mut v = car();
        let clean = indicators(&v).unwrap();
        assert_eq!(clean.values, [0.; 8]);
        assert!(!clean.any);
        set(&mut v, 0xc, 0.5); // engine
        set(&mut v, 0x14, 0.9); // gearbox
        for (i, (wheel, suspension)) in [(0.9, 0.8), (0.1, 0.2), (0.6, 0.4), (0.2, 0.5)]
            .into_iter()
            .enumerate()
        {
            set(&mut v, 0x18 + 4 * i, wheel);
            set(&mut v, 0x28 + 4 * i, suspension);
        }
        let d = indicators(&v).unwrap();
        // Rear wheels (0.9 / 0.8) are excluded from steering.
        let x =
            f64::from(0.5f32 * f32::from_bits(0x3e4c_cccd) + 0.6f32 * f32::from_bits(0x3f4c_cccd));
        assert_eq!(d.values[2], (x * x) as f32);
        assert_eq!(d.values[3], (((0.5f32 + 0.4) + 0.2) + 0.8) / 4.);
        assert_eq!(&d.values[4..], &[0.9, 0.1, 0.6, 0.2]);
        assert_eq!(d.grades, [0.7, 1., 0.2, 0.7, 1., 0., 0.2, 0.]);
        assert!(d.any);
    }
}
