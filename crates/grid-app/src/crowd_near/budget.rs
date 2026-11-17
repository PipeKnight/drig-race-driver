//! Shared clip sample counting and reserved near-actor pose allocation.

const FRAME_RATE: f32 = 20.0;
const MAX_FRAMES: usize = 48;
// About 10 MiB of mesh position/normal/UV attributes at the limit.
pub(crate) const MAX_FRAME_VERTICES: usize = 320_000;

pub(super) fn frame_count(duration: f32) -> Option<usize> {
    if !duration.is_finite() || duration <= 0.0 || duration > 60.0 {
        return None;
    }
    Some(((duration * FRAME_RATE).ceil() as usize).clamp(2, MAX_FRAMES))
}

pub(super) mod planner {
    use super::MAX_FRAMES;
    use std::collections::BTreeMap;

    /// Caller has already resolved the exact clip in this rig's animation bank.
    #[derive(Clone, Copy)]
    pub(in super::super) struct ClipCost {
        pub(in super::super) ordinal: usize,
        pub(in super::super) samples: usize,
    }

    #[derive(Debug, PartialEq, Eq)]
    pub(in super::super) struct Plan {
        /// Index of the actual validated modifier passed to plan_lod.
        pub(in super::super) lod: usize,
        pub(in super::super) samples: usize,
        pub(in super::super) vertices: usize,
        pub(in super::super) total: usize,
    }

    /// One immutable pose bank per rig. Caller passes the real modifier vertex
    /// counts in authored detail order, highest first. Clip ordinals are from
    /// one loaded bank. Reused materials and placements cost no further poses.
    pub(in super::super) fn plan_lod(
        used: usize,
        limit: usize,
        lod_vertices: &[usize],
        clips: &[ClipCost],
    ) -> Option<Plan> {
        let mut unique = BTreeMap::new();
        for clip in clips {
            if !(2..=MAX_FRAMES).contains(&clip.samples) {
                return None;
            }
            let old = unique.insert(clip.ordinal, clip.samples);
            if old.is_some_and(|old| old != clip.samples) {
                return None;
            }
        }
        let samples = unique.into_values().try_fold(0usize, usize::checked_add)?;
        if samples == 0 {
            return None;
        }
        lod_vertices
            .iter()
            .copied()
            .enumerate()
            .find_map(|(lod, vertices)| {
                let vertices = vertices.checked_mul(samples)?;
                let total = used.checked_add(vertices)?;
                (vertices > 0 && total <= limit).then_some(Plan {
                    lod,
                    samples,
                    vertices,
                    total,
                })
            })
    }

    #[cfg(test)]
    mod tests {
        use super::{super::frame_count, *};

        use super::super::MAX_FRAME_VERTICES as LIMIT;
        // Existing published census from the read-only installed source:
        // mar_LOD0's five real PSSG modifier vertex counts, in detail order.
        const MARSHALL: [usize; 5] = [910, 651, 361, 188, 62];

        fn clip(ordinal: usize, duration: f32) -> ClipCost {
            ClipCost {
                ordinal,
                samples: frame_count(duration).unwrap(),
            }
        }

        fn start_finish() -> Vec<ClipCost> {
            vec![
                clip(183, 1. / 30.),
                clip(107, 50. / 30.),
                clip(110, 112. / 30.),
                clip(106, 53. / 30.),
                // Normal Idle resolves to this same authored bank clip.
                clip(183, 1. / 30.),
                clip(103, 48. / 30.),
                clip(109, 136. / 30.),
                clip(72, 62. / 30.),
            ]
        }

        #[test]
        fn sample_limits_match_the_normal_near_baker() {
            assert_eq!(frame_count(1. / 30.), Some(2));
            assert_eq!(frame_count(1.), Some(20));
            assert_eq!(frame_count(2.4), Some(48));
            assert_eq!(frame_count(60.), Some(48));
            for invalid in [0., -0.5, 60.0001, f32::NAN, f32::INFINITY] {
                assert_eq!(frame_count(invalid), None);
            }
        }

        #[test]
        fn exact_budget_can_retain_the_highest_lod_and_reused_idle() {
            let roles = start_finish();
            let samples = 242;
            let vertices = MARSHALL[0] * samples;
            assert_eq!(
                plan_lod(LIMIT - vertices, LIMIT, &MARSHALL, &roles),
                Some(Plan {
                    lod: 0,
                    samples,
                    vertices,
                    total: LIMIT
                })
            );
            let reserve = LIMIT - vertices + 1;
            let reduced = plan_lod(reserve, LIMIT, &MARSHALL, &roles).unwrap();
            assert_eq!(reduced.lod, 1);
            assert_eq!(reduced.total, reserve + MARSHALL[1] * samples);
        }

        #[test]
        fn all_roles_and_largest_walker_reservation_fit_by_source_lod() {
            let mut roles = start_finish();
            let jarama = {
                roles.extend([
                    clip(183, 1. / 30.),
                    clip(105, 56. / 30.),
                    clip(108, 90. / 30.),
                    clip(104, 73. / 30.),
                ]);
                plan_lod(114_660, LIMIT, &MARSHALL, &roles).unwrap()
            };
            assert_eq!(
                (jarama.lod, jarama.samples, jarama.total),
                (2, 376, 250_396)
            );
            let roles = start_finish();
            let milan = plan_lod(242_692, LIMIT, &MARSHALL, &roles).unwrap();
            assert_eq!((milan.lod, milan.samples, milan.total), (3, 242, 288_188));
            let detroit = plan_lod(171_712, LIMIT, &MARSHALL, &roles).unwrap();
            assert_eq!((detroit.lod, detroit.total), (2, 259_074));
            let donington = plan_lod(114_660, LIMIT, &MARSHALL, &roles).unwrap();
            assert_eq!((donington.lod, donington.total), (1, 272_202));
        }

        #[test]
        fn unfit_or_ambiguous_inputs_leave_the_existing_reserve_untouched() {
            let mut roles = start_finish();
            assert!(plan_lod(LIMIT + 1, LIMIT, &MARSHALL, &roles).is_none());
            assert!(plan_lod(LIMIT, LIMIT, &MARSHALL, &roles).is_none());
            assert!(plan_lod(319_999, LIMIT, &MARSHALL, &roles).is_none());
            assert!(plan_lod(0, LIMIT, &[usize::MAX, 0], &roles).is_none());
            assert!(plan_lod(0, LIMIT, &MARSHALL, &[]).is_none());
            assert!(plan_lod(0, LIMIT, &[], &roles).is_none());
            roles.push(ClipCost {
                ordinal: 183,
                samples: 4,
            });
            assert!(plan_lod(0, LIMIT, &MARSHALL, &roles).is_none());
            assert!(
                plan_lod(
                    0,
                    LIMIT,
                    &MARSHALL,
                    &[ClipCost {
                        ordinal: 183,
                        samples: 49
                    }]
                )
                .is_none()
            );
        }
    }
}
