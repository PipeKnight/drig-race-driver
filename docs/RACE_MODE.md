# Race mode with AI opponents (F293)

15 October 2026, Claude Code. `Play GRID Race.cmd` now runs a **three-lap race** against 3 cars
driven by the ported GRID AI ([AI_DRIVER_PORT.md](AI_DRIVER_PORT.md)). It runs as
`--time-trial-laps 3 --opponents 3` with body contacts and events, plus impact damage when
`local\deformation-parameters.bin` exists.

## Behaviour

- The time-trial event provides the 3-second countdown with every car held on the handbrake,
  the player's lap timing from GO and the frozen finish screen. The heading reads "race" and
  no personal bests are shown or written (`time-trial-records.tsv` is untouched).
- `grid_tools::race::Standings` (F293 AUTHORED classification; the original rules were
  recovered and ported in F308, see "Race rules (F308)" below):
  - every car's race distance is measured along the AI racing line from the start-finish
    line (progress gate 0) and unwrapped tick by tick, so cars behind the line start below
    zero;
  - AI cars finish when they have covered the lap count;
  - the player finishes with the lap-timing finish.
- The HUD shows `Grid p/n` during the countdown and `Position p/n` while racing. The finish
  screen shows `RACE RESULT` with the winner's time and gaps. ~~Cars still running when the
  player finishes get a finish time projected from their remaining distance at their
  current speed, marked `(est.)`.~~ Since F308 the opponents race on after the player's
  finish; estimates only come from F (Finish Race) or terminal damage.
- R restarts the race and the standings.

## Checks

- Headless `grid-opponent-race ROOT CD01 BP01 4 20000 ai` (all AI, 2 laps):
  - classification car0 2:58.064, car1 +0.864, car2 +1.936, car3 +2.816;
  - it agrees with the lap-timing totals (car0 laps 89.335 + 87.149 s plus 1.58 s from the
    grid to the line).
- App, scripted validation with `GRID_AUTOPILOT=1` (the ported AI drives the player's car in
  `--drive-ticks` runs only), 1 lap, 3 opponents:
  - finish tick 5873, event 1:30.945, lap 1:29.375;
  - result: You 1:30.945, AI 1 +0.592 (est.), AI 2 +1.625 (est.), AI 3 +2.346 (est.);
  - render `local/opponents/race-finish-4.png` checked.
- A first autopilot run exposed that the authored stuck-recovery counted the countdown hold
  (full throttle at standstill) as "stuck" and reversed at GO. The autopilot now runs only
  after the hold, as opponents already did.

## Not yet

- ~~No overtaking, side-by-side or avoidance behaviour.~~ Overtake and alongside (F294),
  yield (F297), and avoid, recovery and recover_tbone (F301) are ported; see
  AI_DRIVER_PORT.md. ~~lost_control (choreographed mistakes), drafting and catch-up are not.~~ Mistakes, failures, catch-up and the choreographer are ported (see "AI pacing and mistakes" below); drafting has no effect in the original (test off, no aero bonus).
- ~~No grid order or qualifying rules, no penalties, no lapped-car handling.~~ Recovered in
  F308: the original has no penalties or blue flags in a single race, and lapped cars race to
  the full distance. The random grid placement is ported as `race::grid_slot` but not wired to
  a launcher option.
- ~~Per-driver characteristics are all 1.0.~~ The captured table is used when present (F297).

## Grid position (F295)

`--grid-position N` (with `--opponents`) puts the player on grid slot N (1 = pole); the
opponents fill the other slots in order. `Play GRID Race.cmd` starts the player at the back
(`GRID_PLAY_GRID` overrides). Autopilot check from position 4 of 4: P2 by the last sector of lap 1.

## Opponent engine sound (F295)

Every opponent runs its own continuous engine mixer (same Lacetti bank as the player), fed
with that car's engine speed and AI throttle. Since F422 its level is the original's: the AI
group of the group mix plus the backend distance law per EngineBay/Exhaust emitter (10/175 m),
heard from the rendered camera ([AUDIO_RESEARCH.md](AUDIO_RESEARCH.md)); still mono, no panning.
(F295 used an AUTHORED gain 0.6 within 10 m, then inversely proportional to distance.) A first
windowed run panicked on a Bevy query conflict (two AudioSink queries), fixed with a disjoint
filter. Windowed 4-car race: output started, no decoder failure, simulation real-time.

## Start lights (F296)

`tracks/europe/donington/start_lights.xml`: object `startlights_lights_horizontal`, 6 stages over 4.0 s,
textures `start_lights_1_e` ... `5_e` and `7_e`; the material `start_lights` (object_simple_emissive.fx) has
diffuse `start_lights_d` and emissive `start_lights_e` (idle). Race countdowns now last 4.0 s; the emissive
texture shows stages 1..5 across the countdown and stage 6 (texture 7, the green row) for 3 s from GO.
That mapping is [I]; renders at ~1 s / ~2.9 s / after GO show 1 red, 3 red and the green row.
Since 2026-10-17 the sequence is read per track (`grid_tools::start_lights`, [ISTANBUL_PARK.md](ISTANBUL_PARK.md)
section 3): Nürburgring and Istanbul Park ship 7 stages over 5.0 s, so their races count down 5.0 s with stages
1..6 across the countdown and stage 7 from GO; Donington is unchanged.

## Gaps (F299)

While racing, the HUD shows the time to the car directly ahead and directly behind: the race-distance
difference along the AI line divided by the player's speed (floor 10 m/s). Authored presentation.

## Field size (F299)

The live original runs 12 AI driver objects (a 12-car touring field), and the route has 20 grid slots.
The prototype's shared world caps at 8 cars: the 64-slot contact pool and 256-row solver guards are
bounded-domain limits of the port, not original values. `Play GRID Race.cmd` now starts 7 AI opponents
with the player at the back (8th). A windowed 8-car race with damage, eight engine mixers and
characteristics runs in real time (36.6 s simulated at 38.1 s wall clock, including startup).

## 12-car field (F300)

The contact-pool and solver-row guards now use one bound, `grid_world::MAX_WORLD_CONTACTS`: 256
contacts and 1,024 rows. They are still port guards, but no longer cap the field at 8. The shared world
uses 24 contact slots per car, clamped to 64–256, so one- and two-car worlds keep their 64-slot layout
and the bit-identical tests are unchanged. `MultiRuntime` allows 1–12 cars and `--opponents`
1–11. `Play GRID Race.cmd` now matches the original's 12-car touring field: 11 AI opponents with the
player starting 12th.

Checks:
- headless 12-car 2-lap race with characteristics: all finish, about 8.9 ms per tick, 1,124
  car-contact records. Two cars lost a lot of time (+23.8 s and +47.2 s), likely through contact
  and the authored stuck recovery;
- windowed 12-car race with damage: real time (40.7 s simulated at 42.2 s wall clock);
- 282 tests and Clippy pass.

## Race rules (F308)

[RACE_RULES_RESEARCH.md](RACE_RULES_RESEARCH.md) recovers the original race manager. Ported:

- **Finish.** Every car finishes on the line crossing that completes the laps, lapped cars
  included. A single-player race is over only when every car has finished [V].
  - After the player's finish the opponents race on and the ported AI drives the player's car
    ([I]: the original enables AI control of the finished car).
  - The HUD shows `RACE RESULT - racing on, F: finish race`, updating live. The frozen result
    appears once every car is finished or out.
- **F ("Finish Race")** ends the race at once with the original's finalisation:
  - cars still racing above 5 mph (2.2352 m/s) get an estimated finish time, marked `(est.)` [V];
  - slower cars are classified `DNF` [V].
  - The estimate (remaining distance at the car's mean race speed so far) is AUTHORED; the
    original's per-vehicle lap estimator is not ported.
- **Classification** [V]: finishers by time, then estimated finishers, then every other car by
  race distance. Cars out of the race show their completed laps (`DNF, 1 lap`, `Wrecked, 2 laps`).
- **Terminal damage:**
  - for the player (the terminal-wheel status that already disables controls) it retires the car
    as `Wrecked` and ends the race at once; every opponent still racing is estimated, as when the
    original's wrecked screen retires a human [V];
  - opponents with terminal damage are retired as `Wrecked` at once (AUTHORED shortcut for the AI
    retire behaviour).
- **Chequered flag** appears under the position once the leader is on the last lap within 250 m
  of the line (rules.xml `end_race` [V]). The original shows it as a marshal flag.
- **Not in the original, so not added:** corner-cutting or track-limit penalties (the
  `corner_cutting` rule is an empty stub), false-start rules and blue flags.
- `grid-opponent-race` retires terminally damaged cars and stops when the race is over. At the
  tick limit it ends the race by finalisation (`race finalized tick=… reason=tick-limit`).
- A render capture (`--render` with `--drive-ticks`) also completes once the race is over, since
  the frozen result stops the tick count.

Checks:
- 9 race unit tests:
  - lapped car, finish order, finalisation with the 5 mph rule, unfinished human;
  - cars out ranked by distance, chequered flag, grid-slot clamping.
- Headless `grid-opponent-race ROOT CD01 BP01 4 20000 ai` (2 laps):
  - all finish: car0 2:56.896, car1 +2.608, car2 +4.624, car3 +5.808, tick 11419;
  - car0's laps 89.064 + 86.242 s plus about 1.59 s from the grid to the line agree with its time.
- Headless with `GRID_STOP_CAR0=3000` and an 8,000-tick limit, finalised at tick 8000:
  - car1 2:55.911 (est.), car2 +1.348 (est.), car3 +2.969 (est.);
  - car0 `DNF, 0 laps` (stopped before completing a lap).
- App, `GRID_AUTOPILOT=1`, 1 lap, 3 opponents, `--drive-ticks 6600`:
  - the player finished at tick 6008 (event 1:32.118, lap 1:30.548);
  - the AI kept driving the player's car, and the race was over at tick 6264;
  - result: AI 1 1:30.560, You +1.558, AI 2 +3.072, AI 3 +5.664;
  - render `local/race-rules/app/race-over-4.png` checked (frozen RACE RESULT, player's car at
    228 km/h under AI control).
- 304 tests including the asset tests (after the rebase onto F307), and Clippy, pass.

## AI pacing and mistakes (F336)

The original race manager now paces the opponents around the player. Details are in
[AI_DRIVER_PORT.md](AI_DRIVER_PORT.md), "Mistakes, failures and race pacing".

- **Catch-up** (`rd_3_star` by default, AUTHORED until the Normal star level is traced):
  - opponents more than 50 m ahead of you lose engine torque, down to 50 % at 200 m (or at
    the leader's gap);
  - opponents more than 50 m behind gain up to +20 % at 400 m;
  - not within the first 500 m or below 12 m/s.
- **Choreographer**:
  - race start: +5 % torque and full corner speed for cars ahead of you in the first 100 m;
    +25 % grip for everyone in the first 750 m;
  - grip assist for cars pushed off the racing line in corners;
  - conveyor belt: one car from the front third drops to the back two places, and one car
    from the back third pushes up to 3rd–4th place;
  - lapped cars ease off for the car behind.
- **Scripted events** on an AI car 0–250 m ahead of you, weighted by its `mistakes`
  characteristic, so they need the private characteristic table:
  - brake locks;
  - brake and power-out spins;
  - from 5000 m and a quarter of the race, tyre blow-outs (the car loses control and
    retires) and engine failures.
- Settings: `--ai difficulty=…,catchup=rd_N_star|on|off,choreographer=on|off,mistakes=on|off`,
  or the `[ai]` section of a settings file (`settings/race.ini`). This needs a playable built
  after this change.

12-car Donington check, 3 laps, characteristics, reference car 11 on the back of the grid
(headless):
- without pacing: winner 4:23.552, spread 14.9 s, 874 contact records;
- with the defaults: winner 4:43.200 (the leaders are held back), spread 3.6 s among the 8
  finishers when the 18,000-tick limit ended the race, 4120 contact records;
- 36 choreographer events started (21 brake locks, 8 brake spins, 7 power-out spins): 17 executed
  and completed, 19 were aborted while still armed (another selection replaced them), 2 timed
  out; no mechanical failure was drawn in 3 laps;
- each spin went through recovery and the car rejoined;
- no failure.
- The tighter field gives far more contact records; this has not been compared with the
  original.
