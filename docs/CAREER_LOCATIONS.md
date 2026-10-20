# Career locations: Washington GP and the Haruna head-to-head roads (F520)

20 October 2026, Claude Code, coordinator's task after the planning pause. Static data and headless
races only; the original game was not launched. Evidence (ignored): `local/locations/` of the
kind-pike worktree (`base/` before, `fix1/` after, `app/` grid-app runs, `barriers.txt`).

## 1. What the career needs [D]

Three head-to-head events of the career had no layout the port could race:

| Event | Name | Discipline | Car | Track model | Route | Laps |
| --- | --- | --- | --- | --- | --- | --- |
| 73 | `up_h2h` (US pro) | 7 Pro_Muscle | Viper (`vip`, 37) | 4 `washington_gp` (5.3 km) | `tracks/usa/washington/route_0` | 2 |
| 75 | `jr_h2h` (Japan rookie) | 17 Midnight_Touge | 350Z (`350`, 19) | 58 `haruna_ill_all` (8.8 km) | `tracks/japan/haruna/route_9` | 1 |
| 77 | `je_h2h` (Japan expert) | 16 Prototype | 787B (`787`, 48) | 59 `haruna_leg_top_mid` (6.3 km) | `tracks/japan/haruna/route_4` | 1 |

All three have `competition_format_id` 1 (single race), one competition and two cars (the player
and the `event_driver` rival). Sources: `event`, `event_track_pool`, `event_vehicle`,
`track_model` (career table dumps in `local/career/data/tables/`).

## 2. State before [O]

`grid-opponent-race`, one car of the event (`local/locations/base/`):

- Washington route 0: `unsupported entity collision box count` (8662 boxes, bound 8192).
- Haruna route 9: `expected unique grid marker` for the forward (subroute 0) grid. Reverse ran:
  1 run, 229.9 s.
- Haruna route 4: ran, 1 run (Lacetti 180.3 s, 787B 151.6 s, 2 cars 152.4 / 158.1 s).

## 3. Fixes

### 3.1 Prop box bound

The 8192 bound on a route's prop collision boxes was the port's own (RUNTIME_LIMITS_AUDIT C36–C39).
It is now the original's fixed-object pool, 10,000 (AD9220, RUNTIME_LIMITS_AUDIT). Washington
route 0 has 8662 boxes with the group-part stand-ins of TRACK_READERS §3.6.

### 3.2 Group barrier layouts across the road

With the bound raised, the Viper hit something at (833, 7.4, 834), on racing-line point 121
(probe `grid-line-near`). It lost 46 → 17 m/s and then fell through the world. In the 2-car race
both cars wrecked at the same spot.

The obstacle was the stand-in box of `chnfen_e_group3b`:
- [O] One placement of a mesh laid out in world space over 40 × 43 m (63 connected parts).
- [O] Its whole extent passed the 100 m check of the F357 stand-in, so it became one solid
  40 × 43 m box beside and across the line.
- [O] `chnfen_e_group4` (96 × 67 m, 252 parts) is the same case.

The barrier stand-in is AUTHORED; the original has no ornament collision (FIDELITY_BACKLOG row 17).
New rule:
- A single-placement prototype wider than a part (10 m) in **both** horizontal directions is
  also a group layout.
- It gets the per-part rule: one box per connected part of at most 10 m.
- A barrier section is long in one direction only, so concrete blocks, ad boards and fences keep
  their whole-extent box.

The survey of all routes' single-placement barrier prototypes (`local/locations/barriers.txt`)
shows that the rule changes only Washington: group4 on routes 0, 2, 3, 5, 7 and 8, and group3b on
0, 3, 5 and 8. No other track's boxes change, so the identity runs are unaffected.

### 3.3 RENDERNODE grid slots

[O] Haruna route 9's subroute-0 group `grid_start_standing_subroute_0_17` holds `slot_00_17`
and `slot_01_17` as RENDERNODE elements, not NODE (probe `grid-pssg-ids`). The frontend garages'
slots are RENDERNODEs too. TOUGE.md's "route 9's subroute-0 group has no slots" was this reader
gap.

The grid reader and `scene::node_matrix` now accept RENDERNODE as a scene node (it carries its own
TRANSFORM). No race track other than Haruna route 9 has RENDERNODE slots, so other grids read as
before.

## 4. Results after [O]

Headless, `grid-opponent-race` with the event car (`local/locations/fix1/`):

| Layout | Solo | 2 cars | Larger field |
| --- | --- | --- | --- |
| Washington 0, Viper, 2 laps | 2 laps, 108.7 / 113.2 s | car1 finishes; car0 goes wide at turn 1 on lap 2 (72 m/s) and stays stuck in Recovery | 4 cars: 3 finish; 12 cars: 8 finish, 4 retire (terminal damage) |
| Haruna 9 forward, 350Z | 224.0 s | 220.7 / 223.3 s | — |
| Haruna 9 reverse, 350Z | 225.7 s | 227.2 / 233.5 s | — |
| Haruna 4 forward, 787B | 151.6 s | 152.4 / 158.1 s | — |
| Haruna 4 reverse, 787B | — | 148.2 / 152.0 s | — |

- Washington route 0 with the Lacetti (the survey car) still wrecks about 1 km in, near the
  concrete block sets at (−364, −44).
- On a flying lap, AI cars reach Washington's turn 1 at 72 m/s and start braking about 120 m
  before it. They run wide into the tyre stacks there. This is FIDELITY_BACKLOG row 41 (AI
  braking; "Washington routes wreck"). It is left to the AI work.

## 5. App and career

- `series::SURVEYED_ROUTES` lists the three layouts, so `route_supported` accepts them:
  - series and drift pools that hold `washington_gp`;
  - the career's events and driver offers.
- New careers now also get US driver offer 7, a time trial on Washington GP.
- `route::HARUNA_RACED` (routes 1, 4, 9) are the touge layouts of the event menu.
  - Midnight Touge offers its pool's raced roads: routes 1 and 9.
  - Pro Touge keeps the launcher's route 1, followed by its pool's route 4.
- The career offers events 73, 75 and 77 once open, each with its event car and layout
  (`career_screen::tests::head_to_heads_race_the_rival_alone`). The career runs subroute 0, which
  has a grid on all three roads now.
- grid-app runs (muted, autopilot, two cars): §6.

## 6. grid-app runs and checks [O]

The release grid-app ran each event's settings (`local/locations/app/e73|e75|e77.ini`: route,
subroute 0, the event car against the same car, laps from the event, one opponent, `GRID_AUTOPILOT=1`,
muted with `local/mute_grid.ps1`, logs beside them). Each race finished with a result screen:

| Event | Layout | Result |
| --- | --- | --- |
| 73 | Washington route 0, Viper, 2 laps | AI 1 3:47.040, player (autopilot) +5.849 |
| 75 | Haruna route 9, 350Z, 1 run | player 3:38.150, AI 1 +6.468 (est.) |
| 77 | Haruna route 4, 787B, 1 run | AI 1 2:34.144, player +1.143 |

These runs were built from the tracks change on F516, before F519. The Viper's engine bank still
failed to load there (`Engine audio unavailable`); F519 fixes that.

Full pre-push checks: see PROJECT_LOG F520.

## 7. Open

- Washington turn-1 braking on flying laps (row 41); the Lacetti wreck 1 km in.
- The touge rules for the head-to-heads: events 75 and 77 run as single races (format 1, as the
  data says). The Midnight Touge catch-up config is used only by the event menu's touge types.
- Washington routes 2, 3, 5, 7 and 8 get the group rule too. They were not re-surveyed here.
