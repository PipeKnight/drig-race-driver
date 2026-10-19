# Career screen (career entry)

18 October 2026, Claude Code (F424). Code: `crates/grid-app/src/career_screen.rs`. Launcher: `Play GRID Career.cmd` (`grid-app --career`).

A minimal career front end. It runs one own-team career event at a time with the existing race code and pays it with `grid_tools::career` (F416, [CAREER_RESEARCH.md](CAREER_RESEARCH.md)). It sits beside the event menu (F414, [EVENT_MENU.md](EVENT_MENU.md)), with the same process model, and does not change that menu's rows.

Labels: **[D]** read from shipped data; **[V]** verified in the original's code (via CAREER_RESEARCH.md); **AUTHORED** a choice of this port.

## What the screen shows

- Status: money, total reputation and overall rank, the five regional reputations with each region's rank (`Career::rank`), licences out of 7 (`Career::licences`) and the season (`season_number`, `season_state`, counted entries).
- Open events: `Career::event_open` events that the port can run:
  - circuit races: `competition_format_id` 1, not head-to-head, no `use_lemans_selection`;
  - at least one `event_vehicle` model in `series::RACEABLE` whose `cars/<stem>` folder exists. The first one is the player's car, and every one is used for the field;
  - `num_competitions` usable layouts in the event's `event_track_pool`: lap races (no time limit) on a layout the port has raced (the `Series::supported_layouts` rule: `route::CIRCUITS` or a `series::SURVEYED_TRACKS` circuit), with the route folder present.
- During an event: its races, each finished place, and the next race (layout, laps, grid slot), or Abandon event.
- After the last race: the payout lines (Championship position, Prize Money, Teammate Prize Money, Team Win Bonus, Sponsorship Money, total, reputation with its base and bonus, new sponsor offers).
- Text is plain English (AUTHORED). An event's name is `db_<name>` from the language file when that string exists, else the database name.
- Controls: Up/Down, Select, Back/Escape (quits, or continues from the payout), and mouse: click a row, then click it again.

## Running an event

1. **Draw** (F474, [V 0x67BC80], CAREER_RESEARCH.md §12.1): `career::field::draw_races`.
   - The pool is sorted stably: by `order_index` on a first entry, by the races driven per layout (`Career::layout_races`, saved) once the event has been paid [I: paid = completed].
   - Venues not taken yet come first, then the rest fills.
   - An event whose drawn races are not all runnable in the port is not offered (AUTHORED).
   - bmw_si_tour on a first entry: Nürburgring Sprint (3 laps), then GP reverse (2 laps).
2. **Field** (F474, [V 0x673F20, 0x663D40, 0x66D350], §12.2): `career::field::field`, built once when the event is entered.
   - One AI car per `event_driver` row (driver, team, livery); 20 cars in bmw_si_tour.
   - The player and the hired team mate take over the two lowest-index cars of a team drawn among the non-key teams with enough cars. The team mate keeps that car's livery; the player drives the event's first raceable model in livery 0 (AUTHORED).
   - A team mate who is one of the event's drivers gives that seat to an `event_legend` driver.
   - AUTHORED: an AI car whose model the port cannot race drives the player's model in livery 0.
   - Not ported: the Ravenwest guest cars, Le Mans selection fields.
   - The AI is the free race's: `difficulty 0.606` (Normal) and `catchup = on` (F439).
3. **Grid** (F474, [V 0x5C7D40]): `career::field::grid` per race.
   - The player's slot from the race's pool row (`player_grid_position_base/range`).
   - Named drivers from `event_driver_grid_position`.
   - The others ranked by `n − 1 − trunc((n − 1) · clamp(skill + U(−0.2, 0.2), 0, 1))`.
   - AUTHORED: the uniform draw's formula, ties in field order, the clock seed of `series::Mt`.
4. **Race profile** `<data dir>/career-race.ini`: `route`, `car`, then the opponents in grid order (they take the free slots in car order):
   - `opponent_cars`, plus the new keys `opponent_liveries` (package index per car) and `opponent_names` (the drivers' names on the HUD and results);
   - `laps`, `opponents` (field − 1), `grid_position`, the AI and `[private]` sections.
   - The race opens with its usual Race Options front end (Driver Aids). The two new keys apply only when present, so other profiles are unchanged.
5. **Result**: when the race leaves to the menu (results Continue or Exit, Retire, Retire from Race), `career_screen::write_result` (Bevy `Last`, on the BACK_TO_MENU exit) writes `<data dir>/career-race.result`:
   - the classification in `post_race::table` order;
   - the player's driver aids at the end of the race.

   Closing the window writes nothing; the event stays in progress and resumes on the next start.
6. **Event in progress** `<data dir>/career-event.txt` (AUTHORED text): event, car, the field (`entrant` lines), the planned races (route, laps, slot, layout, field index of each race car) and the results so far. The screen adds each result when it starts again. It maps the race's car k back to its field entrant (AI driver and team, or the team mate) and counts the race for its layout.
7. **Payout**: after the last race, `career::apply_event(catalogue, career, event, results, settings)`, then `career::save::store` to `<data dir>/career.save`. The event file is removed only after the save is written.

Mapping to `career` types:
- `RaceResult`: car 0 is `Entrant::Player`. AI car k is `Entrant::Ai { driver: k, team: k }`: each AI car is its own team (AUTHORED; the career field's drivers and teams are not traced), so the Team Win Bonus is paid when the player leads the event standings.
- A car that is out (retired, wrecked, stopped, disqualified) is classified at its place with `finished = false`. `undamaged` is false only for a wrecked car (AUTHORED; the original's no-damage flag is not traced).
- `RaceSettings`: traction control on unless Off, steering assist = Stability Control, braking assist on unless Off, manual gears = manual gearbox (25 % and 50 % count as on). The star level is the race difficulty's: the level whose `ai_ratio` is nearest 0.606 (`flashback::star_level`, as F439's catch-up), which is Normal, level 1. Its `rep_bonus_factor` is −0.1 [D], so F has −10 points (AUTHORED: the career's own difficulty choice is not ported). No head camera, no pro mode: neither exists in the port.

## New career and driver offers (Arc 1, F464)

A new save starts as the original's does [V 0x674610]: `Career::new()`, no team and no money. Own-team events stay closed until the team exists. The player drives **driver offers** for other teams, and the money they pay founds the team when the bank reaches `start_race_team` (40,000). Founding [V 0x674610 → 0x66F640] spends the 40,000, activates the team, grants the rookie licences in all three regions and gives the free `initial_vehicle_id` 43 (`bos`). The payout page then says so. Saves made with F424 (team already founded) load unchanged.

Driver offers on the home page, after the open events:
- **Which** (`Career::driver_offers_shown`, CAREER_RESEARCH.md §6) [V partial, I]:
  - three region slots (`tier.order_index` 0 US, 1 Europe, 2 Japan);
  - each shows one race offer and one test offer: the first offer by `driver_offer.order_index` not driven yet, in the region's driver-offer tier of the player's rank there (the International offer tiers at overall rank 3);
  - a driven offer moves its slot on, whether its objective was met or not [I];
  - the Le Mans offer tiers are not shown (no Le Mans entry yet).
- **Which the port runs** (`offer_runs`, AUTHORED filter):
  - the offer's car (`vehicle_livery` → `vehicle`) is in `series::RACEABLE`;
  - its one `driver_offer_track` race is a lap race on a surveyed layout.
  - Others are not listed. On F463's main a new career has four: 2 (Lacetti, Washington), 15 (Porsche 911 at Jarama, finish 3rd or better), 16 (BMW 320si time trial at the Nürburgring) and 25 (350Z, Yokohama, beat a rival team).
- **Row text:** "Race/Test for TEAM: EVENT - OBJECTIVE (fee, bonus) [car]". The team and event names are `db_*` language strings when present. The objective is in English (AUTHORED wording after `db_race_offer_*`).
- **Race offers:** since F474 the offer event's `event_driver` field [V 0x674020]. The player takes the lowest-index car of `driver_offer.team_id`, in the offer's livery; that team's other cars are AI team mates; the own team mate does not race. The grid slot comes from `driver_offer_track` `player_grid_position_base/range`. An offer of a Le Mans selection event (e.g. 15, lms_gt2) keeps the F464 field of the event's raceable cars (AUTHORED; `lemans_restrictions` not ported).
- **Test offers** (time trial, speed test): the port's Time Trial (no opponents) on the layout. AUTHORED: the original's test session is a separate mode.
- **Objective** (`rules::offer_objective_met`, [V] objective classes):
  - finishing place, including Le Mans: `place ≤ objective`;
  - no damage: finished and not wrecked (AUTHORED damage rule, as for sponsors);
  - beat rival: every car of `objective_team_id` must finish behind (the field's teams since F474); AUTHORED only in the Le Mans selection fallback, where AI cars 1 and 2 drive for the rival team (`RIVAL_CARS`);
  - time trial: the player's best lap below `target_time_secs[star level]`;
  - speed test: the top speed at least `target_speed_mph[star level]`. AUTHORED: sampled once per frame from the player's velocity (`TopSpeed`).
  - The star level is the race difficulty's (Normal = 1).
- **Pay** (`career::apply_driver_offer`) [V 0x674940, 0x652270]:
  - race offers pay the appearance fee `per_race_wage × races` (each offer has one race [D]);
  - with the objective met, `objective_money_bonus` and `objective_reputation_bonus` to the offer tier's region;
  - [I] no event prize or reputation pot for an offer race.
  - The payout page shows "Objective: achieved/failed", Driver Offer Wages, Driver Offer Rewards, Total and Reputation.
- **Files:** the event file has an `offer ID` line, and `best`/`top` lines after the race. The race result file adds `best SECONDS` and `top MPH`.

AUTHORED: an event's car is lent to the player. Buying a car (`Career::buy_car`) has no screen yet, and the `bos` is not an entry car of any raceable rookie event.

## Team mate and sponsors (F466)

Two home-page rows once the team exists: "Team mate: NAME" and "Sponsors: N received, M placed".

**Team mate page** (`Career::can_hire`, `search`, `hire_driver`):
- **When** [I]: hiring opens with the first licence (two garages). The original forces its team mate screen at that point (flow.xml `force_buy_team_mates`); its access predicate was not found. Before that the page says "We're not ready to hire a team mate yet…" (`team_mate_noccess_subtitle` [D]).
- **Applicants** [V 0x662850, 0x649320]: every `driver` row with `can_be_hired` (345 [D]). The `team_mate` table is not read by the original.
- **Filters** [V filter 0x638A60]: Region (Any, Europe, United States, Japan) and Signing fee (Any or one `team_mate_fee_range` row). AUTHORED: the list is sorted by fee, then driver id, and capped to the rows that fit.
- **Hiring** [V 0x6994E0]: pays the `signing_fee` once (only if affordable) and replaces any current team mate with no refund. The team mate keeps `winnings_percentage` % of his own prize money [V], so the team gets the rest (Teammate Prize Money).
- **In the race:** since F474 the team mate drives the second car of the player's team in the career field, under his own name. The result marks it `Entrant::TeamMate` (token `t`), so his prize share, the team standings and his sponsor objectives count. Event files from F466 keep the AI car 1 rule (`TEAMMATE_CAR`). Driver offers have no own team mate [V 0x674020].

**Sponsors page**:
- Every received offer (`event_sponsor_offer`, unlocked by finishing 1st–3rd in an event [V]) with its sponsor (`db_<sponsor>` when the language has it), objective, money per race and slot.
- Selecting an offer moves it: not on the car → Major (pays double) → Minor 1 … 7 → off the car. A slot's previous occupant leaves it.
- AUTHORED: the original's sponsor designer (carousel, swap, best-sponsor fill) is not ported, and the logos are not drawn on the car.

**Per-race sponsor pay**: after each race of an own-team event, the event page lists each placed sponsor's objective for the player and the team mate, and the money that race earned (`Career::sponsor_pay`, [V 0x66DB80, 0x652010]: `money_reward × 2 in the major slot × (player met + team mate met)`). The money is paid with the event, as Sponsorship Money on the payout page. The no-damage objectives use the port's damage rule: a car still classified counts as undamaged (AUTHORED; the original's no-damage flag is not traced).

## Processes

The same process model as the event menu (winit allows one event loop per process):
- `grid-app --career` supervises `grid-app --career-screen`. The screen's exit code 22 (`CAREER_GO`) means the race profile is written.
- It then runs `grid-app --settings <data dir>/career-race.ini --from-menu --career-result <data dir>/career-race.result`.
- When the race ends with exit code 21 (`event_menu::BACK_TO_MENU`), the career screen opens again. Any other exit ends the supervisor.
- `--game-root`, `--data-dir` and `--settings` are passed on.
- Desktop only. Android has no career supervisor (AUTHORED).
- Log lines: `drive_career version=1 event=new_career|open|entered|go|race_result|race_added|paid|abandoned|quit`.

## Expected first-play figures (bmw_si_tour, CAREER_RESEARCH.md §3)

`bmw_si_tour` (event 7, europe_rookie, Touring_Car) [D]:
- cars: the BMW 320si only (`event_vehicle` 9, 55, 58: `bmw`);
- pool: the four Nürburgring layouts. The draw gives `nurburgring_spr` (route_1, 3 laps, order 1) and `nurburgring_gp_r` (route_2, 2 laps, order 2);
- grid (14, 2) on 12 cars: slot 12, the back of the grid.

| Championship place | Prize Money | Base reputation (§3.2) | Reputation paid, default aids at Normal |
|---|---|---|---|
| 1st | 30,000 | 20,000 | 18,000 |
| 2nd | 25,500 | 16,000 | 14,400 |
| 5th | 18,000 | 6,800 | 6,120 |
| 10th | 9,000 | 2,400 | 2,160 |

A win also pays the rookie Team Win Bonus of 20,000 (§3.1), because each AI car is its own team, so the player's team leads. A won event therefore credits 50,000. With all aids off and the manual gearbox, F = 0.2 − 0.1 = 0.1, so a win gives 22,000 reputation. A repeat of the event pays half the reputation of the previous base (pot depletion, §3.2).

## Checks

- [V] Unit tests in `career_screen::tests`:
  - the result and event files round-trip, and a result without the player is rejected;
  - the race profile parses to a 12-car race of the event's cars;
  - digit grouping;
  - ignored, on the purchased data: bmw_si_tour is offered with the BMW; its plan is route_1 × 3 laps and route_2 × 2 laps from slot 12; wins, 2nd places and aids-off wins pay as above.
- The in-app run of bmw_si_tour is recorded in PROJECT_LOG F424.
- Unattended runs: diagnostics `results_continue = SECONDS` (or `GRID_RESULTS_CONTINUE`) makes a scripted run (`--drive-ticks` with `autopilot`) select Continue on the stats and results screens after that many seconds. A career race then runs and returns its result with no input.
- Fully unattended career: `GRID_CAREER_AUTO=EVENT_ID` (or `o` and a driver offer id, e.g. `o15`) with `GRID_AUTOPILOT=1` and `GRID_RESULTS_CONTINUE=3`, then `grid-app --career`. Each screen page acts by itself after 3 s: it enters the event, starts each next race, and quits after the payout. The supervisor runs the races with `--drive-ticks 36000`, so the autopilot drives. No keyboard or mouse input reaches the desktop. Driving is unchanged; it acts only on the post-race screens of scripted runs.

## Not done

- Test offers cannot run unattended: the scripted autopilot exists only in races with opponents. Team naming and livery, the sponsor designer and logos, car buying and selling, head-to-heads, Le Mans entry, season-end screens, and the original's career look.
- The Ravenwest guest cars, Le Mans selection fields (`lemans_restrictions`) and the original's player car and livery choice in an own-team field.
- Flashback limits per difficulty and pro mode's restart ban (§8).
