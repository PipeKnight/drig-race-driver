//! The original's career ("GRID World"): catalogue, state, results and
//! saves (docs/CAREER_RESEARCH.md).
pub mod data;
pub mod ebay;
pub mod field;
pub mod rules;
pub mod save;
pub mod state;

pub use data::{Catalogue, Event, Region};
pub use rules::{Classified, Entrant, Flashbacks, RaceResult, RaceSettings};
pub use state::{
    Career, Payout, SeasonEntry, SeasonState, SponsorPay, apply_driver_offer, apply_event,
};

#[cfg(test)]
mod tests {
    use super::data::*;
    use super::*;
    use std::collections::BTreeMap;

    const P: Entrant = Entrant::Player;
    const T: Entrant = Entrant::TeamMate;
    fn ai(n: i32) -> Entrant {
        Entrant::Ai {
            driver: n,
            team: 100 + n,
        }
    }

    fn race(id: i32) -> EventRace {
        EventRace {
            id,
            track_model_id: 1,
            laps: 3,
            time_limit_seconds: 0,
            order_index: 0,
            player_grid_position_base: 1,
            player_grid_position_range: 0,
            base_results: Vec::new(),
        }
    }

    fn event(id: i32, tier_id: i32, head_to_head: bool) -> Event {
        Event {
            id,
            name: format!("e{id}"),
            tier_id,
            discipline_id: 24,
            competition_format_id: 1,
            head_to_head,
            use_lemans_selection: false,
            order_index: id,
            num_competitions: 2,
            percentage_towards_completion: 2,
            // Event 7 bmw_si_tour [D].
            prize_money_for_first: if head_to_head { 500000 } else { 30000 },
            money_fractions: vec![
                1.0, 0.85, 0.75, 0.65, 0.6, 0.5, 0.45, 0.4, 0.35, 0.3, 0.25, 0.2, 0.15, 0.1, 0.075,
                0.05, 0.025, 0.01,
            ],
            money_string_id: 5,
            reputation_pot: if head_to_head { 0 } else { 40000 },
            reputation_pot_min: 100,
            points_id: 6,
            sponsor_id: 0,
            minor_sponsor_id: 0,
            player_livery_number: None,
            teammate_livery_number: None,
            races: vec![race(1), race(2)],
            vehicles: vec![9],
            sponsor_offers: (1..=3)
                .map(|p| SponsorOffer {
                    id: id * 10 + p,
                    event_id: id,
                    sponsor_id: p,
                    objective: if p == 1 {
                        SponsorObjective::FinishNoDamage
                    } else {
                        SponsorObjective::FinishingPlace
                    },
                    objective_details: 3,
                    position_to_unlock: p,
                    money_reward: [3000, 2150, 1350][p as usize - 1],
                })
                .collect(),
            drivers: (1..=12).map(|d| EventDriver { driver_id: d, livery_id: 100 + d }).collect(),
        }
    }

    /// A small catalogue with the shipped values of the tables used.
    fn tiny() -> Catalogue {
        let ranks = [("rookie", 0), ("pro", 80000), ("expert", 250000), ("international", 1000000)]
            .iter()
            .enumerate()
            .map(|(i, (n, r))| Rank {
                id: i as i32 + 1,
                name: (*n).into(),
                reputation_to_unlock: *r,
            })
            .collect();
        let tier = |id, rank_id, region, offers| Tier {
            id,
            name: format!("t{id}"),
            rank_id,
            region,
            driver_offers: offers,
            order_index: 0,
        };
        let mut events = BTreeMap::new();
        for e in [event(7, 1, false), event(34, 1, false), event(69, 1, true), event(78, 14, true)] {
            events.insert(e.id, e);
        }
        let mut difficulties = Vec::new();
        for rank_id in 1..=4 {
            for star in 0..6 {
                difficulties.push(Difficulty {
                    id: (rank_id - 1) * 6 + star + 1,
                    rank_id,
                    star_level: star,
                    ai_ratio: 1.0,
                    available_flashbacks: 5 - star,
                    flashback_bonus: 3000,
                    rep_bonus_factor: [-0.2, -0.1, 0.0, 0.1, 0.2, 0.3][star as usize],
                });
            }
        }
        let offer = DriverOffer {
            id: 12,
            tier_id: 21,
            event_id: 22,
            team_id: 91,
            vehicle_livery_id: 201,
            vehicle_id: Some(10),
            livery: "lac_02".into(),
            order_index: 1,
            per_race_wage: 4500,
            objective: OfferObjective::FinishingPlace,
            objective_finishing_place: 3,
            objective_team_id: 0,
            objective_money_bonus: 1500,
            objective_reputation_bonus: 150,
            target_times: Vec::new(),
            target_speeds_mph: Vec::new(),
            races: vec![race(1), race(2), race(3)],
        };
        Catalogue {
            ranks,
            tiers: vec![
                tier(1, 1, Region::Europe, false),
                tier(2, 2, Region::Europe, false),
                tier(14, 4, Region::International, false),
                tier(21, 1, Region::UnitedStates, true),
            ],
            events,
            vehicles: BTreeMap::from([(
                43,
                Vehicle {
                    id: 43,
                    stem: "bos".into(),
                    region: Some(Region::UnitedStates),
                    class_id: 1,
                    purchasable: true,
                    purchase_price: 55000,
                    sale_price: 30000,
                    can_enter_le_mans: false,
                },
            )]),
            sponsors: BTreeMap::new(),
            driver_offers: BTreeMap::from([(12, offer)]),
            teams: BTreeMap::new(),
            team_types: BTreeMap::from([(5, 2)]),
            drivers: (1..=12)
                .map(|i| {
                    (
                        i,
                        Driver {
                            id: i,
                            first_name: format!("D{i}"),
                            surname: String::new(),
                            team_id: if i <= 2 { 5 } else { 100 + (i + 1) / 2 },
                            skill: i as f32 / 12.0,
                        },
                    )
                })
                .collect(),
            liveries: BTreeMap::new(),
            venues: BTreeMap::from([(1, 6), (2, 6), (3, 7)]),
            named_slots: Vec::new(),
            legends: BTreeMap::from([(7, vec![50])]),
            team_mates: Vec::new(),
            hireable: vec![Hireable {
                driver_id: 8,
                first_name: "Ann".into(),
                surname: "Example".into(),
                rank_id: 1,
                region_id: 1,
                signing_fee: 10000,
                winnings_percentage: 20,
                skill: 0.6,
                team_id: 0,
            }],
            fee_ranges: vec![(0, 10000), (10000, 50000)],
            difficulties,
            reputation_fractions: vec![
                0.5, 0.4, 0.3, 0.2, 0.17, 0.14, 0.12, 0.1, 0.08, 0.06, 0.05, 0.04, 0.03, 0.02, 0.01,
            ],
            points: BTreeMap::from([(6, vec![20, 18, 16, 14, 12, 10, 8, 6, 5, 4, 3, 2])]),
            team_win_rewards: BTreeMap::from([(1, 20000), (2, 150000), (3, 1000000), (4, 3000000)]),
            head_to_head_rewards: BTreeMap::new(),
            initial_vehicle_id: 43,
            start_money: 40000,
            assist_rewards: AssistRewards::default(),
            currency_rates: vec![1.0, 2.0, 1.5, 230.0],
            ebay: super::ebay::Settings::default(),
        }
    }

    /// Star level 2 (factor 0) with every aid on: no reputation bonus.
    fn plain() -> RaceSettings {
        RaceSettings {
            star_level: 2,
            traction_control: true,
            steering_assist: true,
            braking_assist: true,
            ..RaceSettings::default()
        }
    }

    fn founded() -> Career {
        let mut c = Career::new();
        c.team_founded = true;
        c
    }

    fn result(order: &[Entrant]) -> RaceResult {
        state::result(order)
    }

    #[test]
    fn prize_and_reputation_follow_the_worked_examples() {
        let c = tiny();
        let e = c.event(7).unwrap();
        let money: Vec<i64> = [0, 1, 4, 14, 15, 17, 18].map(|p| rules::prize(e, p)).to_vec();
        assert_eq!(money, [30000, 25500, 18000, 2250, 1500, 300, 0]);
        let rep: Vec<i64> = [0, 1, 4, 14, 15]
            .map(|p| rules::reputation_base(&c.reputation_fractions, 40000, p))
            .to_vec();
        assert_eq!(rep, [20000, 16000, 6800, 400, 0]);
        assert_eq!(rules::teammate_share(25500, 20), 20400);
    }

    #[test]
    fn bonus_factor_counts_assists_and_difficulty() {
        let c = tiny();
        // Star 2, TC/steering/braking aids off, manual gears: 5+5+5+5 = 20.
        let s = RaceSettings {
            star_level: 2,
            manual_gears: true,
            ..RaceSettings::default()
        };
        let f = rules::bonus_factor(&c, 1, &s);
        assert!((f - 0.2).abs() < 1e-6, "{f}");
        assert_eq!(rules::reputation_bonus(20000, f), 4000);
        // Star 5, pro mode, head camera, all aids off, manual: 80 points;
        // realistic damage adds nothing.
        let hard = RaceSettings {
            star_level: 5,
            head_camera_only: true,
            pro_mode: true,
            manual_gears: true,
            realistic_damage: true,
            ..RaceSettings::default()
        };
        let f = rules::bonus_factor(&c, 2, &hard);
        assert_eq!(rules::reputation_bonus(40000, f), 32000);
        // Star 0 with every aid on: -20 points.
        let easy = RaceSettings {
            star_level: 0,
            ..plain()
        };
        assert!((rules::bonus_factor(&c, 1, &easy) + 0.2).abs() < 1e-6);
        assert_eq!(rules::bonus_factor(&c, 1, &plain()), 0.0);
    }

    #[test]
    fn standings_share_ranks_on_equal_points() {
        let races = [result(&[P, ai(1), ai(2)]), result(&[ai(1), P, ai(2)])];
        let table = rules::standings(&[20, 18, 16], &races);
        assert_eq!(table[0].points, 38);
        assert_eq!(table[0].rank, 0);
        assert_eq!(table[1].points, 38);
        assert_eq!(table[1].rank, 0);
        assert_eq!(table[2].rank, 1);
        assert_eq!(table[2].races, 2);
    }

    #[test]
    fn each_finished_race_pays_the_flashback_bonus_for_unused_flashbacks() {
        let cat = tiny();
        let fb = |used| Some(Flashbacks { used, maximum: 3 });
        let mut first = result(&[P, ai(1)]);
        first.flashbacks = fb(1);
        let mut second = result(&[ai(1), P]);
        second.flashbacks = fb(0);
        second.order[1].finished = false;
        let mut plain_races = [first.clone(), second.clone()];
        for r in &mut plain_races {
            r.flashbacks = None;
        }
        let mut without = founded();
        let base = apply_event(&cat, &mut without, 7, &plain_races, &plain()).unwrap();
        let mut c = founded();
        let p = apply_event(&cat, &mut c, 7, &[first.clone(), second], &plain()).unwrap();
        // Star 2: 3 flashbacks, award 3000; 1 used -> 2 * 3000 / 3; the
        // unfinished race pays none.
        assert_eq!(p.flashback_bonus, 2000);
        assert_eq!(p.money(), base.money() + 2000);
        assert_eq!(c.money, without.money + 2000);
        assert_eq!(c.season_earnings, without.season_earnings + 2000);
        // A driver offer pays it too (0x678BD0 / 0x678EE0 call 0x652200).
        let mut c = founded();
        first.flashbacks = fb(0);
        let p = apply_driver_offer(&cat, &mut c, 12, false, &[first], &plain()).unwrap();
        assert_eq!(p.flashback_bonus, 3000);
    }

    #[test]
    fn an_event_pays_prize_reputation_sponsors_and_drains_the_pot() {
        let cat = tiny();
        let mut c = founded();
        let races = [result(&[P, ai(1)]), result(&[P, ai(1)])];
        let p = apply_event(&cat, &mut c, 7, &races, &plain()).unwrap();
        assert_eq!(p.position, Some(0));
        assert_eq!(p.prize, 30000);
        assert_eq!(p.reputation(), 20000);
        assert_eq!(p.team_win_bonus, 20000);
        assert_eq!(p.new_sponsor_offers, [71, 72, 73]);
        assert_eq!(c.money, 50000);
        assert_eq!(c.reputation[Region::Europe as usize], 20000);
        assert_eq!(c.reputation_pots[&7], 20000);

        // Sponsors: 71 in the major slot (pays double), 72 minor.
        c.place_sponsor(71, None).unwrap();
        c.place_sponsor(72, Some(0)).unwrap();
        let p = apply_event(&cat, &mut c, 7, &races, &plain()).unwrap();
        assert_eq!(p.sponsor_money, 2 * (3000 * 2 + 2150));
        assert_eq!(p.reputation_base, 10000);
        assert_eq!(c.reputation_pots[&7], 10000);
        assert!(p.new_sponsor_offers.is_empty());

        // The pot never falls below reputation_pot_min.
        for _ in 0..12 {
            apply_event(&cat, &mut c, 7, &races, &plain()).unwrap();
        }
        assert_eq!(c.reputation_pots[&7], 100);
    }

    #[test]
    fn the_teammate_shares_prize_and_sponsor_money() {
        let cat = tiny();
        let mut c = founded();
        c.money = 100000;
        c.hire(8, 10000, 20).unwrap();
        assert_eq!(c.money, 90000);
        c.sponsor_offers.push(72);
        c.place_sponsor(72, Some(3)).unwrap();
        // Player 4th, teammate 2nd: only the teammate meets "top 3".
        let races = [result(&[ai(1), T, ai(2), P])];
        let p = apply_event(&cat, &mut c, 7, &races, &plain()).unwrap();
        assert_eq!(p.position, Some(3));
        assert_eq!(p.team_mate_prize, 20400);
        assert_eq!(p.sponsor_money, 2150);
        assert_eq!(p.team_win_bonus, 20000, "18 + 14 points beat 20");
        assert!(p.new_sponsor_offers.is_empty());
    }

    #[test]
    fn driver_offer_money_founds_the_team() {
        let cat = tiny();
        let mut c = Career::new();
        let p = apply_driver_offer(&cat, &mut c, 12, true, &[], &Default::default()).unwrap();
        assert_eq!(p.driver_offer_wages, 13500);
        assert_eq!(p.driver_offer_bonus, 1500);
        assert_eq!(c.reputation[Region::UnitedStates as usize], 150);
        assert!(!c.team_founded);
        assert!(c.season.is_empty(), "seasons start with the team");
        apply_driver_offer(&cat, &mut c, 12, true, &[], &Default::default()).unwrap();
        let p = apply_driver_offer(&cat, &mut c, 12, false, &[], &Default::default()).unwrap();
        assert!(p.team_founded);
        assert!(c.team_founded);
        // 15000 + 15000 + 13500 - 40000.
        assert_eq!(c.money, 3500);
        assert_eq!(c.cars[0].vehicle_id, 43);
        // [I] The offer whose money founds the team is not counted: the
        // season counter runs before the credit in this port.
        assert!(c.season.is_empty());
        apply_driver_offer(&cat, &mut c, 12, false, &[], &Default::default()).unwrap();
        assert_eq!(c.season, [SeasonEntry::DriverOffer(12)]);
    }

    #[test]
    fn sponsors_pay_per_race_for_player_and_teammate() {
        let cat = tiny();
        let mut c = founded();
        c.sponsor_offers = vec![71, 72];
        c.place_sponsor(71, None).unwrap();
        c.place_sponsor(72, Some(2)).unwrap();
        // 71 finish undamaged (major, 3000 x2), 72 top 3 (2150).
        c.team_mate = Some(state::HiredTeamMate {
            driver_id: 8,
            winnings_percentage: 20,
        });
        let pay = c.sponsor_pay(&cat, &result(&[ai(1), ai(2), P, ai(3), T]));
        assert_eq!(pay.len(), 2);
        assert_eq!((pay[0].offer_id, pay[0].major, pay[0].money), (71, true, 12000));
        assert!(pay[0].player_met && pay[0].teammate_met);
        assert_eq!((pay[1].offer_id, pay[1].player_met, pay[1].teammate_met), (72, true, false));
        assert_eq!(pay[1].money, 2150);
    }

    #[test]
    fn hiring_waits_for_the_first_licence() {
        let cat = tiny();
        let mut c = founded();
        c.money = 50000;
        assert!(!c.can_hire(&cat));
        assert!(c.hire_driver(&cat, 8).is_err());
        c.reputation[Region::Europe as usize] = 80000;
        assert!(c.can_hire(&cat));
        assert_eq!(c.search(&cat, Some(Region::Europe), Some(0)).len(), 1);
        assert!(c.search(&cat, Some(Region::Japan), None).is_empty());
        assert!(c.search(&cat, None, Some(1)).len() == 1, "10,000 is in both ranges");
        c.hire_driver(&cat, 8).unwrap();
        assert_eq!(c.money, 40000);
        assert_eq!(c.team_mate.as_ref().unwrap().winnings_percentage, 20);
        assert!(c.search(&cat, None, None).is_empty(), "the current one is left out");
    }

    #[test]
    fn races_are_drawn_by_order_then_by_layout_count_venues_first() {
        let cat = tiny();
        let pool = |tm: i32, order: i32| EventRace {
            track_model_id: tm,
            order_index: order,
            ..race(tm)
        };
        // Layouts 1 and 2 share venue 6, layout 3 is venue 7.
        let p = [pool(1, 2), pool(2, 1), pool(3, 3)];
        let ids = |v: Vec<EventRace>| v.iter().map(|r| r.track_model_id).collect::<Vec<_>>();
        let none = BTreeMap::new();
        assert_eq!(ids(field::draw_races(&cat, &p, 2, true, &none)), [2, 3]);
        // A repeat: fewest races first, venues first, then fill.
        let counts = BTreeMap::from([(2, 1), (3, 1)]);
        assert_eq!(ids(field::draw_races(&cat, &p, 2, false, &counts)), [1, 3]);
        assert_eq!(ids(field::draw_races(&cat, &p, 3, false, &counts)), [1, 3, 2]);
    }

    #[test]
    fn the_player_team_takes_over_a_non_key_team() {
        let cat = tiny();
        let mut c = founded();
        let mut rng = crate::series::Mt::new(7);
        let f = field::field(&cat, &c, field::Entry::Event(7), 999, &mut rng);
        assert_eq!(f.len(), 12);
        assert_eq!((f[0].entrant, f[0].livery_id), (Entrant::Player, 999));
        assert!((102..=107).contains(&f[0].team_id), "{}", f[0].team_id);
        // The key team's cars (drivers 1 and 2) stay.
        for d in [1, 2] {
            assert!(f.iter().any(|x| x.driver_id == Some(d)));
        }
        // With a team mate: both take one team's two cars; a team mate who
        // is an event driver gives his seat to a legend.
        c.team_mate = Some(state::HiredTeamMate {
            driver_id: 3,
            winnings_percentage: 20,
        });
        let mut rng = crate::series::Mt::new(7);
        let f = field::field(&cat, &c, field::Entry::Event(7), 999, &mut rng);
        assert_eq!(f.len(), 12);
        let mate = f.iter().find(|x| x.entrant == Entrant::TeamMate).unwrap();
        assert_eq!(mate.team_id, f[0].team_id);
        let legend_or_overwritten = f.iter().any(|x| x.driver_id == Some(50)) || f[0].team_id == 102;
        assert!(legend_or_overwritten);
        assert!(f.iter().filter(|x| x.driver_id == Some(3)).count() == 1, "only as the team mate");
    }

    #[test]
    fn a_race_offer_puts_the_player_in_the_offer_team() {
        let mut cat = tiny();
        let o = cat.driver_offers.get_mut(&12).unwrap();
        o.event_id = 7;
        o.team_id = 5;
        let c = Career::new();
        let mut rng = crate::series::Mt::new(1);
        let f = field::field(&cat, &c, field::Entry::Offer(12), 201, &mut rng);
        assert_eq!(f.len(), 12);
        assert_eq!((f[0].entrant, f[0].team_id), (Entrant::Player, 5));
        // Driver 1 (lowest index of team 5) is replaced; driver 2 stays.
        assert!(!f.iter().any(|x| x.driver_id == Some(1)));
        assert!(f.iter().any(|x| x.driver_id == Some(2) && x.team_id == 5));
    }

    #[test]
    fn the_grid_places_the_player_then_ranks_by_skill() {
        let cat = tiny();
        let c = founded();
        let mut rng = crate::series::Mt::new(3);
        let f = field::field(&cat, &c, field::Entry::Event(7), 999, &mut rng);
        let g = field::grid(&cat, &f, None, 12, 0, &mut rng);
        assert_eq!(g.len(), 12);
        assert_eq!(g[11], 0, "player at the back");
        let mut seen = g.clone();
        seen.sort_unstable();
        assert_eq!(seen, (0..12).collect::<Vec<_>>());
        // The most skilled drivers start near the front.
        let pos = |d: i32| g.iter().position(|&i| f[i].driver_id == Some(d));
        if let (Some(fast), Some(slow)) = (pos(12), pos(1)) {
            assert!(fast < slow);
        }
    }

    #[test]
    fn a_driven_offer_moves_its_slot_on() {
        let cat = tiny();
        let mut c = Career::new();
        assert_eq!(c.driver_offers_shown(&cat), [12]);
        apply_driver_offer(&cat, &mut c, 12, false, &[], &Default::default()).unwrap();
        assert!(c.driver_offers_shown(&cat).is_empty());
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn a_new_career_shows_one_race_and_one_test_offer_per_region() {
        let cat = catalogue();
        let c = Career::new();
        // US rookie: 2 (race), 7 (time trial); Europe rookie: 15, 16;
        // Japan rookie: 25, 27 (speed test) - first by order_index [D].
        assert_eq!(c.driver_offers_shown(&cat), [2, 7, 15, 16, 25, 27]);
        let bmw = &cat.driver_offers[&23];
        assert_eq!(cat.vehicles[&bmw.vehicle_id.unwrap()].stem, "bmw");
        assert_eq!(bmw.races.len(), 1);
    }

    #[test]
    fn ranks_licences_and_garages_follow_reputation() {
        let cat = tiny();
        let mut c = founded();
        assert_eq!((c.licences(&cat), c.garages(&cat)), (0, 1));
        c.reputation[Region::Europe as usize] = 80000;
        assert_eq!(c.rank(&cat, Region::Europe), 1);
        assert_eq!((c.licences(&cat), c.garages(&cat)), (1, 2));
        assert!(c.tier_open(&cat, 2));
        c.reputation[Region::Japan as usize] = 250000;
        c.reputation[Region::UnitedStates as usize] = 80000;
        assert_eq!((c.licences(&cat), c.garages(&cat)), (4, 3));
        assert_eq!(c.overall_rank(&cat), 2);
        assert!(!c.tier_open(&cat, 14));
        // International by the total of all regions, Le Mans included.
        c.reputation[Region::LeMans as usize] = 590000;
        assert_eq!(c.total_reputation(), 1000000);
        assert_eq!(c.overall_rank(&cat), 3);
        assert_eq!(c.rank(&cat, Region::Europe), 3);
        assert_eq!(c.licences(&cat), 7);
        assert!(c.tier_open(&cat, 14));
    }

    #[test]
    fn a_season_is_four_events_and_le_mans() {
        let cat = tiny();
        let mut c = founded();
        let races = [result(&[ai(1), P])];
        let before = c.money;
        for _ in 0..3 {
            apply_event(&cat, &mut c, 34, &races, &plain()).unwrap();
        }
        assert_eq!(c.season_state(), SeasonState::Running);
        apply_event(&cat, &mut c, 34, &races, &plain()).unwrap();
        assert_eq!(c.season_state(), SeasonState::LeMansPending);
        assert!(!c.end_season());
        // Season's Earnings: every payout of the season, cleared at its end.
        assert!(c.money > before);
        assert_eq!(c.season_earnings, c.money - before);
        c.refuse_lemans();
        assert_eq!(c.season_state(), SeasonState::Ended);
        assert!(c.end_season());
        assert_eq!(c.season_earnings, 0);
        assert_eq!(c.season_number, 2);
        assert_eq!(c.previous_season.len(), 4);
        assert!(c.season.is_empty());
    }

    #[test]
    fn head_to_head_opens_after_winning_the_tier() {
        let cat = tiny();
        let mut c = founded();
        let win = [result(&[P, ai(1)])];
        assert!(!c.event_open(&cat, 69));
        apply_event(&cat, &mut c, 7, &win, &plain()).unwrap();
        assert!(!c.event_open(&cat, 69));
        apply_event(&cat, &mut c, 34, &win, &plain()).unwrap();
        assert!(c.event_open(&cat, 69));
        let before = c.total_reputation();
        let p = apply_event(&cat, &mut c, 69, &win, &plain()).unwrap();
        assert_eq!(p.prize, 500000);
        assert!(p.head_to_head && p.position == Some(0));
        assert_eq!(c.total_reputation(), before);
        assert!(!c.game_completed);
        c.head_to_heads.insert(78);
        apply_event(&cat, &mut c, 78, &win, &plain()).unwrap();
        assert!(c.game_completed);
    }

    #[test]
    fn cars_are_bought_and_sold_at_list_prices() {
        let cat = tiny();
        let mut c = founded();
        c.money = 60000;
        c.buy_car(&cat, 43).unwrap();
        assert_eq!(c.money, 5000);
        assert!(c.buy_car(&cat, 43).is_err());
        assert_eq!(c.sell_car(&cat, 0).unwrap(), 30000);
        assert_eq!(c.money, 35000);
    }

    #[test]
    fn saves_round_trip_and_reject_tampering() {
        let cat = tiny();
        let mut c = founded();
        c.team_name = "Ravenwest Motorsport".into();
        c.money = 123456;
        c.reputation = [1, 2, 3, 4, 5];
        c.hire(8, 0, 25).unwrap();
        apply_event(&cat, &mut c, 7, &[result(&[P, T])], &plain()).unwrap();
        c.place_sponsor(71, None).unwrap();
        c.place_sponsor(73, Some(6)).unwrap();
        c.completed_offers.insert(12);
        c.previous_season.push(SeasonEntry::DriverOffer(12));
        c.head_to_heads.insert(69);
        c.count_race(24);
        c.count_race(24);
        let text = save::write(&c).unwrap();
        assert!(text.contains("layout_races 24 2\n"));
        assert_eq!(save::read(&text).unwrap(), c);
        let forged = text.replace("money 1", "money 9");
        assert!(save::read(&forged).is_err());
        assert!(save::read("grid-career 2\nchecksum 0\n").is_err());
        c.team_name = "a\nb".into();
        assert!(save::write(&c).is_err());
    }

    fn catalogue() -> Catalogue {
        let root = std::path::PathBuf::from(
            std::env::var_os("GRID_GAME_ROOT").expect("GRID_GAME_ROOT required"),
        );
        Catalogue::load(&root).unwrap()
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn catalogue_reads_the_career_tables() {
        let c = catalogue();
        let reps: Vec<i64> = c.ranks.iter().map(|r| r.reputation_to_unlock).collect();
        assert_eq!(reps, [0, 80000, 250000, 1000000]);
        assert_eq!(c.start_money, 40000);
        assert_eq!(c.initial_vehicle_id, 43);
        assert_eq!(c.vehicles[&43].stem, "bos");
        assert_eq!(c.reputation_fractions.len(), 15);
        assert_eq!(c.reputation_fractions[0], 0.5);
        let bmw = c.event(7).unwrap();
        assert_eq!(bmw.name, "bmw_si_tour");
        assert_eq!(bmw.tier_id, 1);
        assert_eq!(bmw.num_competitions, 2);
        assert_eq!(bmw.prize_money_for_first, 30000);
        assert_eq!(bmw.money_fractions.len(), 18);
        // Two races drawn from a pool of four layouts [D].
        assert_eq!(bmw.races.len(), 4);
        assert!(!bmw.vehicles.is_empty());
        assert_eq!(bmw.money_fractions[1], 0.85);
        assert_eq!(bmw.sponsor_offers.len(), 3);
        assert_eq!(c.events.len(), 71);
        assert_eq!(c.team_mates.len(), 35);
        assert_eq!(c.driver_offers.len(), 76);
        assert_eq!(c.difficulties.len(), 24);
        let offer = &c.driver_offers[&16];
        assert_eq!(offer.target_times, [108.0, 107.0, 106.0, 105.0, 104.0, 103.0]);
        // The hire pool is `driver.can_be_hired` (345 rows) [D, V 0x662850].
        assert_eq!(c.hireable.len(), 345);
        assert_eq!(c.fee_ranges.len(), 7);
        assert_eq!(c.fee_ranges[6], (5000000, 10000000));
        assert_eq!(c.assist_rewards, AssistRewards::default());
        let offers: usize = c.events.values().map(|e| e.sponsor_offers.len()).sum();
        assert_eq!(offers, 156);
        // The shipped events reproduce the worked examples.
        assert_eq!(rules::prize(bmw, 1), 25500);
        let gt2 = c.event(31).unwrap();
        assert_eq!(rules::prize(gt2, 2), 112500);
        assert_eq!(
            rules::reputation_base(&c.reputation_fractions, gt2.reputation_pot, 9),
            4800
        );
        // One head-to-head per regional and international tier.
        for t in c
            .tiers
            .iter()
            .filter(|t| !t.driver_offers && [1, 2, 3, 4].contains(&t.region.id()))
            .filter(|t| t.name != "garage_tier")
        {
            let h2h = c.tier_events(t.id).iter().filter(|e| e.head_to_head).count();
            assert_eq!(h2h, 1, "tier {}", t.name);
        }
    }

    /// bmw_si_tour (DRAW_FIELD worked example): the first entry draws the
    /// Nurburgring Sprint then the GP reverse; the field is its 20
    /// `event_driver` cars, Victor Rossi's (key team) untouched, the player
    /// in one of the 9 other teams.
    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn bmw_si_tour_draw_and_field_follow_the_original() {
        let cat = catalogue();
        let e = cat.event(7).unwrap();
        let races = field::draw_races(&cat, &e.races, 2, true, &BTreeMap::new());
        let layouts: Vec<i32> = races.iter().map(|r| r.track_model_id).collect();
        assert_eq!(layouts, [24, 99]);
        assert_eq!(races.iter().map(|r| r.laps).collect::<Vec<_>>(), [3, 2]);
        let mut c = Career::new();
        c.team_founded = true;
        let teams = [116, 117, 119, 120, 121, 123, 124, 125, 368];
        for seed in 0..20 {
            let mut rng = crate::series::Mt::new(seed);
            let f = field::field(&cat, &c, field::Entry::Event(7), 140, &mut rng);
            assert_eq!(f.len(), 20);
            assert!(teams.contains(&f[0].team_id), "{}", f[0].team_id);
            assert_eq!(f.iter().filter(|x| x.team_id == 5).count(), 2);
            let g = field::grid(&cat, &f, Some(150), 14, 2, &mut rng);
            let slot = g.iter().position(|&i| i == 0).unwrap();
            assert!(slot == 13 || slot == 14, "player slot {slot}");
        }
        // Race 2 (pool row 39): Fraga (driver 11) is named to slots 1-3.
        let mut rng = crate::series::Mt::new(5);
        let f = field::field(&cat, &c, field::Entry::Event(7), 140, &mut rng);
        if let Some(i) = f.iter().position(|x| x.driver_id == Some(11)) {
            let g = field::grid(&cat, &f, Some(39), 14, 2, &mut rng);
            assert!(g.iter().position(|&k| k == i).unwrap() < 3);
        }
    }

    #[test]
    #[ignore = "requires GRID_GAME_ROOT pointing to purchased resources"]
    fn a_rookie_career_runs_on_the_shipped_data() {
        let cat = catalogue();
        let mut c = Career::new();
        // Arc 1: US rookie driver offers until the team is founded.
        let offers: Vec<i32> = cat
            .driver_offers
            .values()
            .filter(|o| o.tier_id == 21)
            .map(|o| o.id)
            .collect();
        for id in offers.iter().cycle().take(40) {
            apply_driver_offer(&cat, &mut c, *id, true, &[], &Default::default()).unwrap();
            if c.team_founded {
                break;
            }
        }
        assert!(c.team_founded);
        assert_eq!(c.cars[0].vehicle_id, 43);
        // The open own-team events are the three rookie tiers'.
        let open: Vec<i32> = cat
            .events
            .keys()
            .copied()
            .filter(|&e| c.event_open(&cat, e))
            .collect();
        assert!(open.contains(&7) && open.contains(&18) && open.contains(&38));
        assert!(!open.contains(&69), "head-to-head needs the tier won");
        assert!(!open.contains(&8), "Europe pro needs 80,000");
        let field: Vec<Entrant> = std::iter::once(Entrant::Player).chain((1..12).map(ai)).collect();
        let win = vec![result(&field); 2];
        let p = apply_event(&cat, &mut c, 7, &win, &plain()).unwrap();
        assert_eq!(p.prize, 30000);
        assert_eq!(p.reputation(), 20000);
        assert_eq!(p.new_sponsor_offers.len(), 3);
        let text = save::write(&c).unwrap();
        assert_eq!(save::read(&text).unwrap(), c);
    }
}
