//! Exact bounded smoke diagnostics, not general interactive admissions.
use super::{motion_capture::Config, skid_marks::OrderMode};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Profile {
    #[default]
    Derby120,
    BmwIstanbul90,
}
impl Profile {
    pub(crate) fn parse(value: &str) -> Result<Self, &'static str> {
        match value {
            "derby-stk-groups350x120" => Ok(Self::Derby120),
            "bmw-istanbul-groups190x90" => Ok(Self::BmwIstanbul90),
            _ => Err(
                "--motion-phase-profile needs derby-stk-groups350x120 or bmw-istanbul-groups190x90",
            ),
        }
    }
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Derby120 => "derby-stk-groups350x120",
            Self::BmwIstanbul90 => "bmw-istanbul-groups190x90",
        }
    }
    pub(crate) fn shots(self) -> usize {
        match self {
            Self::Derby120 => 120,
            Self::BmwIstanbul90 => 90,
        }
    }
    fn window(self) -> (u64, u32, u64) {
        match self {
            Self::Derby120 => (350, 120, 900),
            Self::BmwIstanbul90 => (190, 90, 450),
        }
    }
    pub(crate) fn check_capture(
        self,
        motion: Option<Config>,
        fps: Option<u32>,
        ticks: Option<u64>,
    ) -> bool {
        let (start, frames, stop) = self.window();
        fps == Some(30)
            && ticks == Some(stop)
            && motion.is_some_and(|c| c.exact_phase_window(start, frames, 30, stop))
    }
    pub(crate) fn boundary_order(self, order: OrderMode) -> bool {
        match self {
            Self::Derby120 => order == OrderMode::GroundFirst,
            Self::BmwIstanbul90 => order == OrderMode::Off,
        }
    }
    pub(crate) fn bmw_fixture(self, c: BmwFixture<'_>) -> bool {
        self == Self::BmwIstanbul90
            && c.route == "tracks/europe/istanbul_park/route_0"
            && c.subroute == 0
            && c.car == "bmw"
            && c.solo
            && c.damage_inputs_off
            && c.handbrake_from == Some(190)
            && c.throttle == 1.
            && c.steering == 1.
    }
}
pub(crate) struct BmwFixture<'a> {
    pub route: &'a str,
    pub subroute: u8,
    pub car: &'a str,
    pub solo: bool,
    pub damage_inputs_off: bool,
    pub handbrake_from: Option<u64>,
    pub throttle: f32,
    pub steering: f32,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crossed_windows_counts_and_script_stops_are_refused() {
        for profile in [Profile::Derby120, Profile::BmwIstanbul90] {
            let (start, count, stop) = profile.window();
            let text = format!("{start},{count}");
            let motion = Config::parse(&text).unwrap();
            assert!(profile.check_capture(Some(motion), Some(30), Some(stop)));
            assert!(!profile.check_capture(Some(motion), Some(60), Some(stop)));
            assert!(!profile.check_capture(Some(motion), Some(30), Some(stop - 1)));
            assert!(!profile.check_capture(Some(motion), Some(30), Some(stop + 1)));
            assert!(!profile.check_capture(None, Some(30), Some(stop)));
            for wrong in ["350,90", "190,120"] {
                assert!(!profile.check_capture(
                    Some(Config::parse(wrong).unwrap()),
                    Some(30),
                    Some(stop)
                ));
            }
        }
        let bmw = Config::parse("190,90").unwrap();
        assert!(!Profile::Derby120.check_capture(Some(bmw), Some(30), Some(450)));
        assert_eq!(Profile::Derby120.shots(), 120);
        assert_eq!(Profile::BmwIstanbul90.shots(), 90);
    }
    #[test]
    fn explicit_profile_identity_controls_and_boundary_order_do_not_cross() {
        let valid = || BmwFixture {
            route: "tracks/europe/istanbul_park/route_0",
            subroute: 0,
            car: "bmw",
            solo: true,
            damage_inputs_off: true,
            handbrake_from: Some(190),
            throttle: 1.,
            steering: 1.,
        };
        assert!(Profile::BmwIstanbul90.bmw_fixture(valid()));
        assert!(!Profile::Derby120.bmw_fixture(valid()));
        for c in [
            BmwFixture {
                route: "tracks/usa/detroit/route_8",
                ..valid()
            },
            BmwFixture {
                car: "stk",
                ..valid()
            },
            BmwFixture {
                subroute: 1,
                ..valid()
            },
            BmwFixture {
                solo: false,
                ..valid()
            },
            BmwFixture {
                damage_inputs_off: false,
                ..valid()
            },
            BmwFixture {
                handbrake_from: Some(500),
                ..valid()
            },
            BmwFixture {
                throttle: f32::NAN,
                ..valid()
            },
            BmwFixture {
                steering: f32::INFINITY,
                ..valid()
            },
        ] {
            assert!(!Profile::BmwIstanbul90.bmw_fixture(c));
        }
        for order in [
            OrderMode::Off,
            OrderMode::Control,
            OrderMode::Isolated,
            OrderMode::GroundFirst,
        ] {
            assert_eq!(
                Profile::Derby120.boundary_order(order),
                order == OrderMode::GroundFirst
            );
            assert_eq!(
                Profile::BmwIstanbul90.boundary_order(order),
                order == OrderMode::Off
            );
        }
        assert!(Profile::parse("bmw").is_err());
        assert_eq!(Profile::default(), Profile::Derby120);
    }
}
