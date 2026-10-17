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

## Next

1. Find the original's writer of wheel +0x4C/+0x64/+0x140/+0xBC (watch the fields on a fresh
   race load, or search the set-up path), port it, and rerun this comparison (`brake_test` and
   `grid-braking-replay`, Braking Assist on).
2. Repeat both runs once more for repeatability, and at a second onset speed.
3. Compare idle +E34 with a port run that starts at rest.
