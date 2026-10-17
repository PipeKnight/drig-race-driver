//! Flashback rules of the original ([research](../../../docs/FLASHBACK.md)):
//! how many flashbacks a race has, when one may be used and what using one
//! costs. The state restore itself lives with the app's simulation.
//!
//! - The count comes from the `difficulty` row of `database.bin`: five at
//!   star level 0 (Basic) down to none at level 5 (Ninja), the same for every
//!   career rank [D]. Pro Mode leaves none [D 0x6AF7C0].
//! - The manager keeps maximum, used and the award (`flashback_bonus` of the
//!   row) and publishes them as `flashback_manager_maximum` / `_used` /
//!   `_max_award` [D 0x6AA200]; remaining = maximum - used [D 0x6A2240].
//! - A jump-in needs one remaining [D 0x4CAD80] and a replay time more than
//!   [`EVENT_GUARD`] before the moment the replay was entered [D 0x937150].
//! - Preparing the jump-in uses one [D 0x59AFA0 -> 0x6A2260 `used += 1`].

/// [D] `difficulty.available_flashbacks` by star level 0..5 (Basic, Normal,
/// Serious, Savage, Extreme, Ninja); identical for the four career ranks.
pub const AVAILABLE: [u32; 6] = [5, 4, 3, 2, 1, 0];

/// [D] `difficulty.flashback_bonus` by career rank 1..4 (rookie, pro,
/// expert, international), the manager's maximum award.
pub const BONUS: [u32; 4] = [3000, 12000, 30000, 60000];

/// [D] `difficulty.ai_ratio` of career rank 2 (pro) by star level, the AI
/// difficulty of the port's free race (0.606 = Normal).
pub const AI_RATIOS: [f32; 6] = [0.506, 0.606, 0.706, 0.8, 0.9, 1.];

/// [D 0xCE8828] A jump-in is refused this close (seconds) before the event
/// time, the live moment the replay was entered.
pub const EVENT_GUARD: f64 = 0.25;

/// [D states.bin] `flashback_flash1..3`: three transitions of
/// `timeDelay="1.0f"` each, then `replay_jump_in` with `frameDelay="2"`.
pub const FLASH_SECONDS: f32 = 1.;
pub const FLASHES: usize = 3;
pub const JUMP_IN_FRAMES: u32 = 2;

/// The star level whose `ai_ratio` is nearest `difficulty` (career rank 2).
pub fn star_level(difficulty: f32) -> usize {
    (0..AI_RATIOS.len())
        .min_by(|&a, &b| {
            (AI_RATIOS[a] - difficulty)
                .abs()
                .total_cmp(&(AI_RATIOS[b] - difficulty).abs())
        })
        .unwrap()
}

/// The race's flashback manager.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Manager {
    pub maximum: u32,
    pub used: u32,
    pub max_award: u32,
}

impl Manager {
    /// 0x6AF7C0: the row's count, or none in Pro Mode.
    pub fn new(star_level: usize, rank: usize, pro_mode: bool) -> Self {
        let available = AVAILABLE[star_level.min(AVAILABLE.len() - 1)];
        Self {
            maximum: if pro_mode { 0 } else { available },
            used: 0,
            max_award: BONUS[rank.clamp(1, BONUS.len()) - 1],
        }
    }

    /// 0x6A2240.
    pub fn remaining(&self) -> u32 {
        self.maximum.saturating_sub(self.used)
    }

    /// 0x4CAD80 (Pro Mode already left no maximum).
    pub fn available(&self) -> bool {
        self.remaining() >= 1
    }

    /// 0x6A2260, from the jump-in's prepare state.
    pub fn use_one(&mut self) -> bool {
        if !self.available() {
            return false;
        }
        self.used += 1;
        true
    }
}

/// 0x937150's time rule: not within [`EVENT_GUARD`] before the event time
/// (only checked once an event time is set).
pub fn time_allowed(replay_time: f64, event_time: f64) -> bool {
    !(event_time > 0. && event_time - EVENT_GUARD < replay_time)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_follow_the_difficulty_rows() {
        // Normal (0.606, the port's default AI) has four, as the owner's
        // reference race options show.
        assert_eq!(star_level(0.606), 1);
        assert_eq!(Manager::new(star_level(0.606), 2, false).remaining(), 4);
        assert_eq!(Manager::new(0, 1, false).remaining(), 5);
        assert_eq!(Manager::new(5, 4, false).remaining(), 0);
        assert_eq!(Manager::new(1, 2, true).remaining(), 0, "Pro Mode");
        assert_eq!(Manager::new(1, 2, false).max_award, 12000);
        assert_eq!(Manager::new(1, 4, false).max_award, 60000);
    }

    #[test]
    fn each_jump_in_uses_one_until_none_remain() {
        let mut m = Manager::new(3, 2, false);
        assert_eq!(m.remaining(), 2);
        assert!(m.use_one());
        assert!(m.use_one());
        assert!(!m.available());
        assert!(!m.use_one());
        assert_eq!(m.used, 2);
    }

    #[test]
    fn no_jump_in_at_the_event_time() {
        assert!(time_allowed(20., 30.));
        assert!(time_allowed(29.75, 30.));
        assert!(!time_allowed(29.76, 30.));
        assert!(!time_allowed(30., 30.));
        // Without an event time the rule does not apply.
        assert!(time_allowed(5., 0.));
    }
}
