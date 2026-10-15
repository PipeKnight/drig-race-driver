# Race mode with AI opponents (F293)

15 October 2026, Claude Code. `Play GRID Race.cmd` now runs a **three-lap race** against 3 cars
driven by the ported GRID AI ([AI_DRIVER_PORT.md](AI_DRIVER_PORT.md)). It runs as
`--time-trial-laps 3 --opponents 3` with body contacts and events, plus impact damage when
`local\deformation-parameters.bin` exists.

## Behaviour

- The time-trial event provides the 3-second countdown with every car held on the handbrake,
  the player's lap timing from GO and the frozen finish screen. The heading reads "race" and
  no personal bests are shown or written (`time-trial-records.tsv` is untouched).
- `grid_tools::race::Standings` (AUTHORED classification; the original race rules and
  position code are not recovered):
  - every car's race distance is measured along the AI racing line from the start-finish
    line (progress gate 0) and unwrapped tick by tick, so cars behind the line start below
    zero;
  - AI cars finish when they have covered the lap count;
  - the player finishes with the lap-timing finish.
- The HUD shows `Grid p/n` during the countdown and `Position p/n` while racing. The finish
  screen shows `RACE RESULT` with the winner's time and gaps. Cars still running when the
  player finishes get a finish time projected from their remaining distance at their
  current speed, marked `(est.)`.
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

- No overtaking, side-by-side or avoidance behaviour (research running). Opponents queue
  behind slower cars.
- No grid order or qualifying rules, no penalties, no lapped-car handling.
- Per-driver characteristics are all 1.0.

## Grid position (F295)

`--grid-position N` (with `--opponents`) puts the player on grid slot N (1 = pole); the
opponents fill the other slots in order. `Play GRID Race.cmd` starts the player at the back
(`GRID_PLAY_GRID` overrides). Autopilot check from position 4 of 4: P2 by the last sector of lap 1.

## Opponent engine sound (F295)

Every opponent runs its own continuous engine mixer (same Lacetti bank as the player), fed
with that car's engine speed and AI throttle. AUTHORED attenuation (the original 3D audio is not
recovered): gain 0.6 within 10 m, then inversely proportional to distance, no panning. A first
windowed run panicked on a Bevy query conflict (two AudioSink queries), fixed with a disjoint
filter. Windowed 4-car race: output started, no decoder failure, simulation real-time.

## Start lights (F296)

`tracks/europe/donington/start_lights.xml`: object `startlights_lights_horizontal`, 6 stages over 4.0 s,
textures `start_lights_1_e` ... `5_e` and `7_e`; the material `start_lights` (object_simple_emissive.fx) has
diffuse `start_lights_d` and emissive `start_lights_e` (idle). Race countdowns now last 4.0 s; the emissive
texture shows stages 1..5 across the countdown and stage 6 (texture 7, the green row) for 3 s from GO.
That mapping is [I]; renders at ~1 s / ~2.9 s / after GO show 1 red, 3 red and the green row.

## Gaps (F299)

While racing, the HUD shows the time to the car directly ahead and directly behind: the race-distance
difference along the AI line divided by the player's speed (floor 10 m/s). Authored presentation.

## Field size (F299)

The live original runs 12 AI driver objects (a 12-car touring field), and the route has 20 grid slots.
The prototype's shared world caps at 8 cars: the 64-slot contact pool and 256-row solver guards are
bounded-domain limits of the port, not original values. `Play GRID Race.cmd` now starts 7 AI opponents
with the player at the back (8th). A windowed 8-car race with damage, eight engine mixers and
characteristics runs in real time (36.6 s simulated at 38.1 s wall clock, including startup).
