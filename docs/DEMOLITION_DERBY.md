# Demolition Derby (F421)

Status: 2026-10-18, Claude Code. The career event `Western_states_D` (Michigan Demolition Derby, discipline `Stock_Car`) runs on the two Detroit stadium routes, 6 laps [D].

The port now runs a derby on both routes, headless, in the app and in the soak. It has the stock car and the original's two disqualification rules.

The ported AI piles up at the figure-of-eight crossing, and the rules then disqualify the stopped cars. That is an AI gap (section 5), not a rules gap.

Evidence (ignored, this worktree's `local/dd/` and `local/touge/decomp/` (`d1`–`d5`); Ghidra 12.1.4 headless, read-only, and Capstone 5.0.6 on `local/compatibility/grid-unpacked.exe`). Nothing installed; the original game was not started.

Tags: [V] observed, [D] data, [I] interpretation, AUTHORED port choice.

## 1. Data [D]

- **Routes:**
  - `tracks/usa/detroit/route_8`, `detroit_stad_L`, type demolition, 2.6 km;
  - `route_9`, `detroit_stad_s`, 2.4 km;
  - both are figure-of-eight laps through a stadium crossing.
- **Event `Western_states_D`:** route 9 (6 laps, grid 8+19, base result 25) and route 8 (6 laps, grid 7+19, base result 30).
- **Car:** `stk` (discipline `Stock_Car`).
- **`gameconfig/demolition_derby.xml`:** `flippedCountdown 5.0`, `flippedVelocityThreshold 1.0`, `wrongWayThreshold 50.0`, `warningThreshold 0.5`.
- **`ai/ai_driver_config.xml` section `single_demolition_race`:**
  - `laDistMin`/`laDistMax` 15;
  - `thBrakeLineHoldTime` 1.5;
  - `raGenerateRandomOffsets` true;
  - `rtRetirementSpeed` −1;
  - `rtRemoveFromTrack` false;
  - `avOncomingTraffic` true;
  - `avFigureOfEightTrack` true;
  - `avAvoidAlongside` false;
  - `maxObservableVehicles` 8, …
- **Strings:** `osd_demolition_dis` "You will be disqualified!", `net_race_results_disqualified` "Disqualified".

## 2. The original [V unless marked]

- **Derby manager.** 0x668820 (vtable 0xCE9D74) at race +0x23E8 holds its settings at +0x20.
  - Constructor 0x630BC0; reader 0x630C30.
  - Keys and defaults: `flippedCountdown` 5.0, `flippedThreshold` 1.0, `wrongWayThreshold` 1.0, `warningThreshold` 0.5, `notMovingTreshold` 10.0, `notMovingCountdown` 10.0 (constants 0xCB093C, 0xCAF888, 0xCAF88C, 0xCAFA58).
  - The shipped file's `flippedVelocityThreshold` is not a key the reader reads, so `flippedThreshold` stays at its default (also 1.0).
- **Per-car update.** The manager update 0x64DFF0 calls 0x630CF0 for each car record.
  - At or above `notMovingTreshold` mph (0xA7DDB0), the car's countdown (+0x24) restarts at `notMovingCountdown`.
  - Below it, the countdown runs down. When it falls below zero and the car is AI (+0x6C ≠ 0), the car is disqualified with 0x9E64E0(6, 0).
  - The record reset 0x64E030 sets the countdown to 9999, so a car still waiting on the grid is never disqualified.
- **0x9E64E0(status, arg).** Sets the handling status +0x444 (from 7) to one of 3–7 and stores the argument at +0x448, behind the gate +0x214, +0x442 and 0xA14E40. Status ≠ 7 disables the controls at the next synchronisation (WHEEL_DETACHMENT.md).
- **Flipped player.** The local player's auto-reset trigger 0x60D590 (still and tilted for 2 s, RESET_AND_RESPAWN_RESEARCH.md §3.1) calls 0x9E64E0(6, 0) instead of a reset when vehicle +0x2E5C is 2 [I: the derby mode].
- **Not found.** No reader of the derby settings' flipped, wrong-way and warning fields (+0x28…+0x34) was found, and no code reference to `osd_demolition_dis`.

## 3. Port

### Rules (`grid_tools`)

- `demolition::Settings::load`, with the reader's keys and defaults.
- `demolition::NotMoving`: the countdown as in 0x630CF0, f32.
- `Runtime::disqualify` / `disqualified`: handling status 6, argument byte 0. The gate's conditions are not modelled [I].
- The handling validator now accepts status 6 alongside 3 and 4. No existing race reaches it.
- `race::Out::Disqualified`.
- `PlayerReset::demolition`: the trigger disqualifies instead of placing.
- `route::{DETROIT_STADIUM, DETROIT_STADIUM_SHORT}`.

### Headless and soak

- `grid-opponent-race --demolition`: every car is AI, so the not-moving rule applies to all. AI data comes from the `single_demolition_race` section. Each disqualification logs position and AI behaviour.
- `grid-soak quick --seeds 33..36`: 12, 12, 6 and 12 stock cars on routes 8/9/8/9, outside the default set.

### App

- `discipline = demolition_derby` (laps and opponents), via `settings/demolition-derby.ini` and `Play GRID Demolition Derby.cmd`.
- The AI uses the `single_demolition_race` section.
- `demolition_event` applies the not-moving rule to the AI cars. A disqualified car stays on track with its controls off (`rtRemoveFromTrack` false [D]).
- The flipped player is disqualified and goes to the results; disqualified cars read "Disqualified".

### Also in this change

Pro Touge now uses the `pro_touge` AI section [D] (headless `grid-opponent-race` and `grid-pro-touge`, app). Route 1 with that section: car 0 5:12.064 (2:38.672 + 2:33.392), car 1 5:13.952; no contact.

## 4. Results [V]

- **Solo stock car**, route 8 (`--cars stk`, no DF01): 2 laps, 1:33.104. One timed lap (45.809 s), as for the Lacetti: the stadium grid stands past the line.
- **12 stock cars, 6 laps**, DF01 and the characteristics (`local/dd/runs/`):

| Route | AI section | Disqualified | Last disqualification | Car-contact records | Finishers |
|---|---|---|---|---|---|
| 8 | default | 12 | tick 9008 | 49,099 | none |
| 8 | `single_demolition_race` | 12 | tick 7594 | 76,939 | none |
| 9 | default | 11 | — | 138,790 | car 10, 4:02.368 |
| 9 | `single_demolition_race` | 12 | — | 162,083 | none |

  With the derby section on route 8, 11 of the 12 stopped cars stood within 17 m of (790, 174, 570), the stadium crossing; the twelfth stopped at (810, 174, 613). They were mostly in `RecoverTbone`, `Avoid` or `Recovery` behaviour, upright (up y 1.00).
- **Derby soak** seeds 33..36: 4/4 pass (5,000 ticks; up to 16 prop breaks and 232 world bodies).
- **Asset test** `player_reset::tests::a_flipped_derby_car_is_disqualified_not_reset`: the stock car flipped on the route-8 grid is disqualified (not placed), stays on its roof, and its controls are disabled on the next tick.
- **App,** windowed, route 8, autopilot player and 11 AI stock cars (`local/dd/app/dd-r8.log`):
  - the derby settings load (`wrongWayThreshold` 50 from the file);
  - the AI cars are disqualified as they stop (all 11 by tick 8772);
  - the player drives on.

## 5. Open

- **AI at the crossing.** The ported AI has no figure-of-eight or oncoming-traffic avoidance (`avFigureOfEightTrack`, `avOncomingTraffic` are read but not ported). Fields pile up in the crossing and then fall to the not-moving rule.
- **Settings without a reader.** The flipped countdown, wrong way and warning fields and the "You will be disqualified!" warning; their readers are not found.
- **Results.** The DD results screen (`post_race_results_dd` [D]) uses the generic layout here [I]. The DD race end (0x64E130, a field check) is not ported.
- **Stadium laps.** One timed lap fewer than counted laps (the grid stands past the line), as on other routes whose grid is after the finish.
