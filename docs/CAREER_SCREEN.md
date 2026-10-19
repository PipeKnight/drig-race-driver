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

1. **Draw** (AUTHORED, CAREER_RESEARCH.md §11 q7): the first `num_competitions` usable pool rows by `order_index`. The original's draw was not traced.
2. **Grid slot** [V formula, docs/RACE_RULES_RESEARCH.md §5]: `race::grid_slot(player_grid_position_base, player_grid_position_range, 12, draw)`, with the original's generator (`series::Mt`) and a clock seed (AUTHORED seed).
3. **Field** (AUTHORED): 12 cars, the player plus 11 AI drivers in the event's raceable models (`opponent_cars`). The career grid fill (0x5C7D40), named drivers and the teammate are not ported. The AI is the free race's: `difficulty 0.606` (Normal) and `catchup = on`, which picks `rd_2_star` from the difficulty (F439).
4. **Race profile** `<data dir>/career-race.ini`: the event menu's keys for a race (`route`, `car`, `opponent_cars`, `laps`, `opponents 11`, `grid_position`, the AI and `[private]` sections). The race opens with its usual Race Options front end (Driver Aids).
5. **Result**: when the race leaves to the menu (results Continue or Exit, Retire, Retire from Race), `career_screen::write_result` (Bevy `Last`, on the BACK_TO_MENU exit) writes `<data dir>/career-race.result`:
   - the classification in `post_race::table` order;
   - the player's driver aids at the end of the race.

   Closing the window writes nothing; the event stays in progress and resumes on the next start.
6. **Event in progress** `<data dir>/career-event.txt` (AUTHORED text): event, car, field, the planned races and the results so far. The screen adds each result when it starts again.
7. **Payout**: after the last race, `career::apply_event(catalogue, career, event, results, settings)`, then `career::save::store` to `<data dir>/career.save`. The event file is removed only after the save is written.

Mapping to `career` types:
- `RaceResult`: car 0 is `Entrant::Player`. AI car k is `Entrant::Ai { driver: k, team: k }`: each AI car is its own team (AUTHORED; the career field's drivers and teams are not traced), so the Team Win Bonus is paid when the player leads the event standings.
- A car that is out (retired, wrecked, stopped, disqualified) is classified at its place with `finished = false`. `undamaged` is false only for a wrecked car (AUTHORED; the original's no-damage flag is not traced).
- `RaceSettings`: traction control on unless Off, steering assist = Stability Control, braking assist on unless Off, manual gears = manual gearbox (25 % and 50 % count as on). The star level is the race difficulty's: the level whose `ai_ratio` is nearest 0.606 (`flashback::star_level`, as F439's catch-up), which is Normal, level 1. Its `rep_bonus_factor` is −0.1 [D], so F has −10 points (AUTHORED: the career's own difficulty choice is not ported). No head camera, no pro mode: neither exists in the port.

## New career (Arc 1 stub)

AUTHORED: driver offers (Arc 1) have no front end yet. A new save therefore runs the original's founding at once: `Career::new()`, then `add_money(start_money)`. Founding [V 0x674610 → 0x66F640] spends the 40,000, activates the team, grants the rookie licences in all three regions and gives the free `initial_vehicle_id` 43 (`bos`). The bank starts at 0.

Also AUTHORED: the event's car is lent to the player. Buying a car (`Career::buy_car`) has no screen yet, and the `bos` is not an entry car of any raceable rookie event.

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
- Fully unattended career: `GRID_CAREER_AUTO=EVENT_ID` with `GRID_AUTOPILOT=1` and `GRID_RESULTS_CONTINUE=3`, then `grid-app --career`. Each screen page acts by itself after 3 s: it enters the event, starts each next race, and quits after the payout. The supervisor runs the races with `--drive-ticks 36000`, so the autopilot drives. No keyboard or mouse input reaches the desktop. Driving is unchanged; it acts only on the post-race screens of scripted runs.

## Not done

- Driver offers, team naming and livery, car buying and selling, teammates, sponsor placement, head-to-heads, Le Mans entry, season-end screens, and the original's career look.
- The original's race draw, career grid fill, named drivers and the teammate in the field.
- Flashback limits per difficulty and pro mode's restart ban (§8).
