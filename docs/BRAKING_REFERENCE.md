# Live straight-line braking: original against the port (F371)

18 October 2026, Claude Code. The first live comparison of Braking Assist in the original
game with the port (docs/DRIVING_ASSISTS_RESEARCH.md §6 asked for it). Evidence: main
checkout `local/driving/braking-20261018/` (captures, port runs, macro logs, screenshots,
user-file snapshot and after-state).

## Authorization and safety

- The owner allowed this session to send input to the original game for a braking test, only
  on a local copy and without changing the Steam install ("Разрешаю сессии самой подавать ввод в
  оригинальную игру для теста торможения, только на локальной копии, без изменения установки
  Steam"), and later freed the desktop for about 10 minutes.
- Game: the research copy `C:\Users\debros\Tools\grid-research\grid-compat\grid.exe`
  (SHA-256 b1da1b2d…aece34a6), PID 42800, started 2026-10-17T22:04:13Z. The Steam install was
  not launched or written.
- User files: the 19 files under `Documents\Codemasters\GRID` were copied with hashes before
  launch (`user-before/`, `user-before.json`). The runs changed 12 savegame files (assist and
  grid-position choices, the retired race); they are kept as `user-after/`, and all 19 were
  restored; the hashes match the snapshot.
- Input: the existing guarded AutoHotkey 2.0.29 macros (`tools/grid_macros.ahk`, exact PID and
  path foreground check, physical-stop keys, key-up on exit). New bounded action
  `brake_test`: unpause, throttle 4 × HOLD_MS (here 6000 ms), full brake 5000 ms, pause.
  Memory access is read-only: `tools/capture_grid_braking.py` opens VM_READ | QUERY_LIMITED
  only. The game was closed with WM_CLOSE afterwards.

## Scenario

Race Day, Touring Cars, Chevrolet Lacetti, Donington Park Grand Prix, Normal, grid position
**Back** (car slot 11), traction control On, Automatic, stability control On; Braking Assist
On (run 1) and Off (run 2). The race starts by itself; the field had left the start straight
(9–14 s race time) before the car was paused and the test sequence ran. Car in memory:
manager index 0, car 0x34527200, physical 0x34D0C8C0/0x34D0E2A0 (found with
`tools/find_reference_vehicle_manager.ps1 -ScanLimitBytes 256MiB`).

The capture writes a row whenever the physical motion or a wheel block changes (≈180–250
rows/s; the original steps with the real frame delta, about 6 ms here,
WORLD_STEPPING_RESEARCH.md): car +0xE00..+0xEFF (controls, +E34, +E74), +0x1140..+0x11BF,
+0x1550..+0x155F (aid bytes), +0x1960..+0x198F (gearbox), the four wheel blocks +0x1990
(4 × 0x150), physical +0x1D0 (pose, angular and linear velocity) and world +0x7C4.

The port side is `grid-braking-replay ROOT slot_11 ASSISTS SCHEDULE OUT` (crate
grid-probes): a solo Lacetti at Donington slot_11 drives 375 ticks of throttle then brake
(6.0 s, the same input) and writes the same vehicle regions per tick. `tools/analyze_braking.py`
decodes both formats.

## Observations [V]

| | Original BA on | Port BA on | Original BA off | Port BA off |
|---|---|---|---|---|
| Speed at brake onset | 48.97 m/s | 49.54 | 48.82 | 49.54 |
| Gear at onset | 4 | 4 | 4 | 4 |
| Distance to < 0.5 m/s | **69.65 m** | **76.23 m** | 53.31 m | 54.77 m |
| Time to < 0.5 m/s | **3.10 s** | **3.38 s** | 2.18 s | 2.22 s |
| Locked-wheel rows | 0 | 0 | 339 of 411 | 114 of 140 |
| Max +E74 | 0 | 0 | 2.057 s | 2.112 s |
| Published brake +E20 | 1 throughout, 0 once gear 9 (reverse) is selected | same | same | same |

Speed and distance from onset, every 0.25 s (BA on):

| t (s) | 0 | 0.5 | 1.0 | 1.5 | 2.0 | 2.5 | 3.0 |
|---|---|---|---|---|---|---|---|
| Original v (m/s) / d (m) | 48.97 / 0 | 42.02 / 16.9 | 33.00 / 35.6 | 24.30 / 50.0 | 16.21 / 60.1 | 8.98 / 66.5 | 2.64 / 69.3 |
| Port | 49.54 / 0 | 42.70 / 17.7 | 34.26 / 36.7 | 26.29 / 51.6 | 18.33 / 62.9 | 12.07 / 70.4 | 5.86 / 74.8 |

- **Braking Assist off matches**: lock-up within 0.5 s of braking in both, the same
  ~23 m/s² while locked, stopping within 1.5 m and 0.05 s; then both select reverse (gear 9)
  at about 2.7 m/s and pull away backwards at the same rate.
- **Braking Assist on does not**: the first 0.25 s match (≈ 9.6 vs 9.2 m/s²), after that the
  original decelerates about 8 % harder (15.9 against 14.7 m/s² between 0.75 and 2.75 s) and
  stops 6.6 m (9 %) and 0.28 s sooner. The original's wheels run closer to the road speed
  (|road − surface speed| ≈ 0.05 m/s against 0.10 in the port).
- At 40 m/s under braking the front wheels' longitudinal force (wheel +0x88/+0xB4) is −6009 in
  the original and −5300 in the port (12 % less); the rear (+0x84/+0xB0) −66.3 against −68.2.
- No ABS-like modulation in either: the published brake stays 1 until reverse, ABS mode
  +11BC is 0 (the Lacetti has none, §2.7 of the assists research), and BA on keeps every wheel
  rolling without a pulse.
- **Drivetrain matches**: up-shifts at 19.6 / 33.4 / 44.5 m/s and down-shifts at 41.9 / 31.3 /
  18.1 m/s in both; the time from the first throttle to onset is 5.85 s in the original (6.0 s
  requested), 6.0 s in the port. The aid parameters +1160..+1178 are equal (60, 0.4, 0.3, 0.4,
  0.05, 0, 1); aid byte +1559 is 0 with BA on and 1 with BA off in both.
- **Wheel constants differ at rest** (all four wheels, before any input):

  | Wheel field (port use) | Original | Port | Port source |
  |---|---|---|---|
  | +0x4C suspension travel / upper clip | 0.153433 | 0.3 | max(0.3, +0x50), standard_car.rs |
  | +0x64 suspension speed threshold | 0.14 | 400 | properties +0x448/+0x44C |
  | +0x140 wheel height offset | −0.0072 rear, −0.0114 front | 0 | not set |
  | +0xBC return clock | −0.021654 | 0 | not set |

- **+E34 (clutch)**: in the original it is 1 only at rest on the paused grid before the first
  throttle, and 0 from the first throttle on, through every shift and the braking (at ≈200
  samples/s, a one-frame value can be missed). The port's runs start with throttle on tick 1
  and show 0 throughout, so they agree while driving; the idle value is not compared.

## Interpretations [I]

- Step size is not the cause: the same port run with DELTA = 0.006 s (a temporary,
  uncommitted build; `port006-*.tsv`) still needs 77.2 m / 3.38 s with BA on and 55.5 m with
  BA off.
- The BA-on gap lies in the unlocked tyre/brake force balance, where the front axle's force
  is 12 % lower. The suspension constants above change travel clipping and load transfer under
  braking, which would act exactly there; with locked wheels (BA off) the friction limit
  dominates and the runs agree. This is a hypothesis: the original's writer of wheel
  +0x4C/+0x64/+0x140/+0xBC after construction is not identified (a set-up/tuning stage at race
  load is the obvious candidate).
- The port is **not changed**: the evidence locates the gap but does not yet say which code
  writes the original's values.

## Follow-up analysis (F377, offline)

**The wheel constants come from the default car setup.** Columns 216–267 of `lac.csv` are the
setup ranges (properties +0x40..+0x130). Their suspension subset is the setup consumer
`9C32F0` that F077 already ported and verified exactly against 24 live cars
(`grid_world::setup::prepare`, `cap_denominators`; docs/VEHICLE_UPDATE.md "Suspension setup
ranges and constructor cap"). The runtime never calls it: the port builds the car with the
verified constructor and stops there. The cap operand, body +0xED68, is the collision hull's
lowest Y (0.103433; live block at body +0xED64 read 0.0042, 0.10343, 2.24166 = max Z, 0.91328
= max X, …, −2.23788 = min Z) [V].

A trial wiring (`Car::configure_default_setup` after `configure_handling`; patch kept as
`local/driving/setup-F372/setup-wiring.patch`) makes all nine setup fields of every wheel equal
to the live original at rest (+0x4, +0x4C 0.153433, +0x54, +0x5C, +0x60, +0x64 0.14, +0x68,
+0x70, +0x114) [V]. But:

- braking is unchanged: BA on 76.231 m / 3.376 s (76.234 without), BA off 54.774 m [V];
- the solo lap moves slightly (88.455 / 86.267 s against 88.420 / 86.282 s; output sha256
  b88b3cd6…), the 12-car race completes [V];
- three release asset tests fail on the changed trajectories: the assists matrix's
  throttle-and-steer run hits a wall and returns "unsupported contact-group impact record"
  (an existing port gap), the autopiloted Drift Battle lap does not finish within 12000 ticks,
  and the deep fresh-mesh wall test no longer changes the damage RNG [V].

So the wiring is **not pushed**; it needs those three cases resolved first (follow-up).
+0x140 (contact floor offset read by the contact-material stage) and +0xBC (a timer) remain
unexplained.

**Where the braking gap is.** Comparing every wheel field that switches on with the brake
(12 rows after onset, BA on) [V]:

| Field | Original | Port |
|---|---|---|
| +0x118 brake torque, rear / front | 1150 / 1850 | 1150 / 1850 |
| +0x88 longitudinal tyre force, rear | −3719 | −3723 |
| +0x88 longitudinal tyre force, front | **−6093** | **−5529** |

Brake torque and the undriven rear axle agree; only the **driven front axle** delivers ~10 %
less force for the same torque. [I] Something on the drivetrain side absorbs front brake torque
in the port (engine inertia or clutch coupling while braking in gear). The shift timer +0x1974
runs alike in both. Car +0x1980 differs (original ≈ 25.6 at rest, falling ≈ 1/s while driving;
port 0); its use is unknown.

The capture tool and the replay probe now also record the engine/drivetrain block car
+0x1840..+0x195F (RPM +0x1894, gear ratios), which the first capture lacked.

## Setup wiring retried; contact-pool slot template (F381, offline)

18 October 2026, Claude Code, while the owner's desktop was busy (no game launched).

**Pool slot template [V, static].** The assists matrix's failure under the setup wiring
("unsupported contact-group impact record") is a wheel-overtravel contact written into a
never-used pool slot: its flag word +0xC was 0, so byte +0x0F (impact-pool flag, which
`contact_groups` requires to be 1) was 0, and +0x1C/+0x20/+0x24 were 0. The overtravel producer
(WHEEL_CONTACT_PRODUCTION.md) writes neither +0x0F nor +0x1C..+0x24; it keeps the slot's
template. In the original the pool allocator `B19E20` allocates N × 0x80 bytes and runs the
contact constructor `B0ECE0` (vtable 0xD3D434) on every slot: +0x1C/+0x20 = −1 (via `B82880`),
byte +0x24 = 1, **byte +0x0F = 1**, +0x28..+0x3C, +0x50..+0x6F and byte +0x70 zero. The port's
slots were all zero. Now `contact_groups::EMPTY_CONTACT` holds that template and every place
that creates pool slots uses it (single-car world, multi-car pool and blank overlay pool,
geometry reservation, prop slots). With the old wheel constants no overtravel ever reached a
fresh slot, so nothing changes in the existing runs; the template only matters once overtravel
occurs, which the shorter original travel causes.

**Setup wiring, retried with the template [V].** Braking replays on 18 October against the live
captures (`local/driving/setup-F372/{setup,nosetup}-ba-{on,off}.tsv`):

| | Original | Port, no setup | Port, with setup |
|---|---|---|---|
| BA on | 69.65 m / 3.10 s | 76.234 m / 3.376 s | 76.231 m / 3.376 s |
| BA off | 53.31 m / 2.18 s | 54.774 m / 2.224 s | 54.774 m / 2.224 s |

The setup keeps the BA-off match but does not shrink the BA-on gap, so it is **not pushed**
(the coordinator's condition). Of the three tests it broke:

- the assists matrix passes with the template;
- the deep authored wall in `source_driving_geometry_provider` no longer consumes the damage
  RNG: the suspension-force impact (wheel +0x138 × delta against the damage threshold) peaks at
  0.67 of the threshold with the original's setup, 1.28 with the old 0.3/400 constants; no wall
  depth between 5 % and 80 % of the car width reaches it (0.08–0.76). That assertion encoded the
  unverified constants; a re-baseline is in the kept patch;
- the autopiloted Drift Battle lap (Long Beach, BMW) fell through the world at the hairpin
  (resolved in F403 below: a test-harness gap).

Patches: `local/driving/setup-F372/setup-wiring-v2-with-template.patch` (setup, template and
wall re-baseline, plus unrelated rustfmt churn) and `setup-wiring-v3.patch` (setup only, on
F381).

**Drivetrain clue [I].** `tools/analyze_braking.py --drivetrain` (new) prints speed, RPM and
per-axle force and wheel speed every 0.25 s from onset. With BA on, the port's front force drops
at each downshift (−5300 N in 3rd, −4830 in 2nd, −3240 in 1st), the original's much less (−5990,
−5680, −4450 in 1st); the rear axle is equal throughout (−3720). This first suggested engine
inertia through the gear ratio; runs 3 and 4 below show the same engine speed in both.

## Live engine-block runs (F381)

18 October 2026, 03:00–03:12 UTC: the owner freed the desktop again and allowed the live test
("Компьютер свободен, ты можешь тестировать оригинальный Grid столько, сколько тебе нужно", via
the coordinator, then "I allow you to run live test" in this session). Same research copy
(SHA-256 b1da1b2d…aece34a6), PID 40200; the same scenario as F371. The 19 user files were
snapshotted first; the runs changed 12, all 19 were restored and match the snapshot hashes; the
game was closed with WM_CLOSE. Evidence: main checkout `local/driving/braking-20261018b/`
(captures with the +0x1840 block, port replays, manager scans, screenshots, macro logs,
user-file snapshot and after-state).

| | Run 3, BA on | Run 4, BA off |
|---|---|---|
| Speed at onset | 48.91 m/s | 48.93 m/s |
| Distance / time to < 0.5 m/s | 69.50 m / 3.09 s (run 1: 69.65 / 3.10) | 53.49 m / 2.18 s (run 2: 53.31 / 2.18) |
| Locked-wheel rows, max +E74 | 0, 0 | 492 of 604, 2.054 s |

The original repeats within 0.2 m. [V] Engine and drivetrain block against the port
(`analyze_braking.py --drivetrain`, `local/driving/braking-20261018b/f403/blockdiff.py`):

- **Engine speed matches.** +0x1894 is 232.7 at idle in both and follows the same course while
  braking (gears 4–2: 690–800 original, 630–790 port). Gear changes happen at nearly the same
  speeds.
- **The front-axle force gap is there from the first braking frame, with and without BA**:
  −5882 against −5057 N at onset, −6087 against −5537 after 0.25 s, before any lock-up in the
  BA-off run. The rear axle agrees (−3719 against −3740).
- Words that differ during braking: car +0x1898, +0x187C, +0x189C..+0x18AC; front wheels +0x110,
  +0x3C, +0x40, +0x138 (suspension force, 3690 / 3850), +0x140 and +0xBC (non-zero only in the
  original), and the setup fields +0x4C/+0x64 (shown by F377). F403 below shows that +0x1898,
  +0x187C, +0x189C..+0x18AC and wheel +0x110 are phases, timers and noise.
- Car +0x1864, +0x1880 and +0x1884 are 1.0 in the original at rest and while driving, 0 in the
  port. Their writer is the engine reset `996C70` (also +0x1888 = 1 and +0x187C from a constant,
  plus zeroing +0x185C..+0x18C4); the port's constructor sets only +0x1888. No instruction in
  the executable reads +0x1864 or +0x1880 by direct offset, and +0x1884 is read only through
  driver preparation `A03703`/`A0381F` (already ported), so these are unlikely to explain the gap.

## Front-axle gap traced offline; Drift Battle fall-through (F403)

18 October 2026, Claude Code, owner away (no game launched). Static analysis of the existing
decompilations and Capstone scans of the research copy, read-only.

**The differing engine words are not torque terms [V, code].** In `vehicle_propulsion::engine`
(verified 1:1 against `9B1B90`, VEHICLE_PROPULSION.md) car +0x1898 is engine e+0xF8, the crank
phase (speed × delta, wrapped at 3141.6); +0x187C is e+0xDC, a below-limit timer; +0x189C..+0x18AC
are the two random engine oscillators. Wheel +0x110 is the wheel angle (spin × delta, wrapped at
±314). They differ because their histories differ, and none feeds a force.

**Torque balance of the front wheels [V].** For each axle, brake torque +0x118 minus ground
force × radius (+0x88 × +0xE0) minus wheel inertia × angular deceleration (+0x100 × dω/dt) leaves
the torque the drivetrain applies (`local/driving/braking-20261018b/f403/balance.py`, BA on):

| Time after onset | 0.1 s (4th) | 0.6 s | 1.2 s (2nd) | 1.6 s (2nd) | 2.0 s (1st) |
|---|---|---|---|---|---|
| Rear, original / port (Nm) | 33 / 29 | 30 / 28 | 30 / 26 | 29 / 26 | 24 / 21 |
| Front, original / port (Nm) | **8 / 181** | 38 / 250 | 129 / 391 | 164 / 387 | 527 / 873 |

The undriven rear agrees (rolling terms). On the driven front axle the port's drivetrain pushes
forward much harder. In the port this term is the engine inertia folded into the driven wheels in
coupling mode 1 (`vehicle_force::run`: view +0x294 = car +0x17A0 / ratio², car +0x17A0 = 0.22):
about 6 kg·m² on the axle in 4th gear, which matches the 181 Nm at −60 rad/s². The original
behaves as if only ~0.24 kg·m² were coupled in 4th, 4.2 in 2nd and 22 in 1st (port: 6.0 / 15 / 42),
while its engine speed still follows the wheels.

**What is ruled out [V, static].**

- Engine inertia: the original's only writer of car +0x17A0 (`A0CFED` in constructor `A0C9F0`)
  copies property +0x1DC, as the port does.
- The rest of the default setup `9C32F0`, which F077 left outside its comparison: gear ratios
  (items 13–20), brake split (21), drive split (22, AWD only), differential (23–26), +0x1F04 (27,
  with +0xE10 = ((+0x1F04 / +0xFD0) × 0.2 + 0.8) × +0xFC4, an identity at the default), camber/toe
  (0–3). For the Lacetti (differential "full", front drive) every default value lies inside its
  CSV range, so the setup reproduces the port's current values. `9AE8F0` (preload) and `9AD6B0`
  (shift thresholds) are already covered.
- The port is in coupling mode 1 (+0x1598 = 1), idle flag +0x1640 = 0 and clutch +E34 = 0
  throughout braking; the original's +E34 is also 0.

[I] The remaining candidates are the original's clutch/coupling state during braking (car
+0x1598, +0x1640, force-view +0x294), which no capture has yet. The sampler and the replay probe
now record car +0x1560..+0x165F and +0x17A0..+0x17AF, so the next live run answers it directly.
No port change is made without that evidence.

**Drift Battle fall-through [V].** With the default setup, the autopiloted BMW slides along the
Long Beach hairpin wall, passes the boundary line at z ≈ −239.4 (sheet triangles 138/143/161,
`boundarylines.cqtc`) with **no body contacts**, then reverses past the end of the track mesh
(collision ends in a straight edge at z ≈ −241.9, ground y = 2.50) and falls. The player's
track had no boundary sheet: `race_app_with` (grid-app race tests) loaded the player runtime
without `prepare_body_geometry`, so in these tests only the player lacked the boundary sheet,
deformation and dynamic props, while the app (main.rs) prepares all three. The shared world
ticks that runtime as car 0, so the test player could pass invisible walls. Fixed in the test
helper. With the fix, the Drift Battle tests pass with the setup applied. The setup itself stays
unwired because it does not shrink the braking gap.

One test depended on the gap: `a_handbrake_turn_in_the_bmw_scores_drift_points` held full
throttle to tick 900, which only worked because the player drove through the boundary wall
(slide 27°, 139 points). With the wall, the BMW meets it at ~59 m/s around tick 800 and is
wrecked; a wrecked car stops ticking and the test's loops waited forever. The test now starts the
turn at tick 600, fails instead of hanging when ticks stop, and asserts the detected slide and
the counted drift. From 108 mph this open-loop turn always ends in a spin (tried start ticks
350–700, handbrake 5–40 ticks, with and without counter-steer), so it banks no points; banked
scores remain covered by the autopiloted Drift Battle tests.

## Next

1. One live run of about 5 minutes (when the owner allows): the same steps as F381, with the
   extended capture (car +0x1560..+0x165F, +0x17A0):
   1. Snapshot `Documents\Codemasters\GRID` (19 files, hashes) as in F371.
   2. Start `C:\Users\debros\Tools\grid-research\grid-compat\grid.exe`; Race Day, Touring Cars,
      Lacetti, Donington GP, Normal, grid Back, TC on, Automatic, SC on, **Braking Assist on**.
   3. When the race starts, pause; `tools/find_reference_vehicle_manager.ps1 -ScanLimitBytes 256MiB`
      gives the car address (manager index 0).
   4. Start `python -I tools/capture_grid_braking.py SESSION.json CAR 16 local/driving/braking-DATE/run05-ba-on.tsv`
      in the background.
   5. `tools/run_grid_macro.ps1 -Action brake_test -HoldMs 1500 -SessionJson ...` (6 s throttle,
      5 s brake).
   6. Close the game with WM_CLOSE, restore the 19 files, verify the hashes.
   7. Offline: compare +0x1598, +0x1640 and +0x17A0 during braking with the port's
      (`local/driving/braking-20261018b/port/f403-ba-on.tsv` columns 10–11: mode 1, flag 0, inertia 0.22).
2. Then port the original's coupling during braking and rerun the replays.
3. Wire the default setup (`setup-wiring-v3.patch`, with the wall-test re-baseline from v2) once
   the braking gap shrinks.
4. Repeat runs at a second onset speed; compare idle +E34 from rest.
