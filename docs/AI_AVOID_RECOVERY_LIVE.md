# Original AI avoid and recovery — live comparison with the port (F302)

15 October 2026, Claude Code. Read-only live trace of the original game's AI during a 12-car
Donington race, compared with the port in `crates/grid-tools/src/ai_driver.rs` (commit dd7d34a;
[AI_DRIVER_PORT.md](AI_DRIVER_PORT.md) "Avoid and recovery") and the static research in
[AI_AVOID_RECOVERY_RESEARCH.md](AI_AVOID_RECOVERY_RESEARCH.md).

Labels: **[V]** verified live or in disassembly; **[I]** inference, not verified.

## Setup

- Process: research copy `grid-compat\grid.exe` (SHA-256 `b1da1b2d…aece34a6`), PID 7208, started
  08:58:42Z. Race Day, Lacetti/Donington GP, 12 cars, 3 laps.
- **Access**: `OpenProcess(PROCESS_VM_READ | PROCESS_QUERY_LIMITED_INFORMATION)` and
  `ReadProcessMemory` only. No writes, injection, suspension or code execution.
- **Input**: the owner unpaused the race twice by hand and left the player car idle. The agent
  sent no input to the game. The game pauses when its window loses focus, so there are two
  captured stretches:
  - about 67 s right after GO;
  - about 270 s covering the rest of the race up to the cool-down.
- New tool: `tools/trace_grid_ai_avoid_recovery.ps1`
  - Hash-guarded process identity and output path under `local/`, at most 1800 s.
  - Per rendered frame and driver, it records:
    - behaviour scores and the active behaviour;
    - the avoid test and avoid behaviour fields;
    - recovery mode, latch, toggle and timers, plus the recover_tbone timer;
    - the spun, on_track, collided_obstruction and contacted_vehicle tests;
    - the request flags;
    - the track-view angles;
    - the own car state;
    - the path obstacle;
    - an emulation of the racing-normally exclusion 0x79A530.
  - Rows are written when anything is active, plus a baseline row every 20th frame.
- Evidence (private, `local/ai-avoid-recovery/live/`):
  - `trace-race-01.tsv` (first stretch, without obstacle columns; sha256 prefix `e6e7710d528ec031`);
  - `trace-race-02.tsv` (first-match link emulation, superseded; `b5634d1e3070dfd8`);
  - `trace-race-03.tsv` (nearest-link emulation, 83,707 rows, 0–270 s; `d6b45d62f01e2711`);
  - discovery scans and dumps.
  - Tool sha256 `47397677b3996e81…c77c15cc7`.

## Live object layout [V]

Driver objects are 12 instances with a stride of 0x4FE0. Driver *i* is at
`0x56A3B10 + i·0x4FE0` in this process. Driver 0 is the player slot.

| Item | Location |
|---|---|
| updater header (0x79F040 `this`) | driver − 0x64: +8 driver, +0x10 dt (double, 0.00606 s), +0x24 → own car state |
| own car state (`own+X`) | `*(driver − 0x40)`: +0x20 centre, +0x30 forward, +0x50 axis, +0xA0 velocity, +0xD0 speed, +0xF0 front, +0x100 rear, +0x110 half width, +0x120/+0x130 obstruction magnitude/normal |
| arbitration / tests runner `this` (0x79CAA0, 0x792ED0) | driver + 0x2AA0 |
| path obstacle (test context +4) | driver + 0x2AC0: +0 relative velocity, +0x20 record, +0x24 distance |
| test vector begin/end | driver + 0x2C64 / + 0x2C68 → inline array driver + 0x3EA0 (21 tests by type) |
| behaviour vector begin/end | driver + 0x2C74 / + 0x2C78 → driver + 0x3F10 (15 behaviours) |
| request (`req+X`) | driver + 0x2CE0 |
| track view (`view+X`) | driver + 0x2DD0; +4 capacity 32, +8 ring offset, +0xC count, +0x14 link array |
| current behaviour | `*(driver + 0x2DC0)` |
| perception records (vtable 0xCF74F4) | inside the driver objects, e.g. the player's at 0x56A8830: +0x10 speed, +0x20 centre, +0x30 forward, +0x90 velocity, +0xF0 half width, +0xF4 length, +0xFC state |

- The view's link window holds 32 links. Window[0] is the link behind the car's own link, and
  the rest run ahead of it.
- Links have +0xC from-gate and +0x10 to-gate.
- Gates have +0x10 position and +0x20 "normal". The normal equals the ai_track XML value and is
  the lateral direction of the gate line.

## Question 1: does avoid fire in normal race traffic?

**Yes, rarely.** Avoid fires on ordinary traffic, but the documented exclusion holds on 99.6%
of the frames where it could fire.

### Evidence (trace-race-03, AI drivers 1–11)

| Path-obstacle frames with ETA = distance / closing < 2 s | Frames | avoid fired |
|---|---|---|
| target slower than 4 m/s: the stopped player car, record state 0 | 116 | 116 (100%) |
| target racing, record state 1; emulation says *excluded* | 19,738 | 71 (0.36%) |

- Every racing target is in record state 1. Only the stationary player car is in state 0.
  State 2 never appeared.
- **The exclusion is as documented** [V]:
  - the live config is avMinRacingSpeed 4 and avMaxRacingAngleFromTrack 0.7853982;
  - every one of the 134 gates gives `t = (−n.z, 0, n.x)` against the driving direction
    (cos −1 … −0.79), from the XML and confirmed for in-memory gates;
  - 0x7A2EA0 is `(−z, 0, x)` (mask 0xCAF470 = sign bits);
  - 0xBF4067 is the SSE2 libm acos;
  - a nonzero return skips firing (0x79A9D8 → 0x79B23B returns 0).
- **Link lookup 0x798C00** searches only the view's 32-link window and keeps the **nearest**
  link (`<=` on the 0x798920 distance).
  - Hairpins put several window links "between gates" for one point.
  - An emulation that takes the first match instead of the nearest misclassified about 1,800
    frames (trace-race-02).
  - With nearest-link selection, the emulation agrees with the game on 99.6% of frames
    (trace-race-03).
- **Leaks**: 71 isolated frames fire although every emulated input says "excluded":
  - target speed 19–58 m/s;
  - heading dot ≥ 0.92;
  - one containing link at about 180°.

  Neighbouring frames with the same, smoothly varying inputs do not fire. The cause is not
  identified [I]. Candidates: record or window data being refreshed between the tests and the
  sample point (0x787620 runs after arbitration), or several AI steps per rendered frame.
- **Branch B turns a leak into an episode.** A Branch A fire stores the target (+0x10).
  Branch B tracks it with no racing-normally check while it stays ahead, inside the 50° cone
  and within `along < 2·closing`.
  - Branch B is identified by ETA = |along|/closing exactly, and Branch A by ETA = obstacle
    distance / |relative velocity|.
  - Example: driver 7 spent 251 frames in avoid (7 Branch A, 244 Branch B) on cars at
    19–45 m/s. The longest episode lasted 3.56 s; target speed 200, side 1.
- **Size**: traffic avoid occupied about 0.4% of AI frames:
  - 1–11 episodes per driver;
  - most episodes last 1–5 frames;
  - a few last 0.7–3.6 s.
- **The stopped player car** (speed 0.005 m/s, 20 m off the track after the start):
  - every follower closing on it fired avoid: 287 frames, side 2, clearance 6.05–6.08 m;
  - target speed 30 (avStoppedTargetSpeed);
  - brake-to-speed appeared on 13 of 287 frames, consistent with the 0.25 s decision ticks.

  This matches the port.

### Port impact

None required. The port implements the exclusion that the original applies on 99.6% of
frames, and it reproduces stopped-car avoid. Reproducing the 0.36% leak would need its unknown
cause, so the port keeps the documented rule.

- **Open**: identify the leak, for example by sampling the record and window together with the
  game's tick counter.
- The port maps record state 2 to "moving against its nose" [I]. Live records use 1 for racing
  AI and 0 for the stationary player. State 2 was not observed.

## Question 2: sign convention of the octant angles

**The port's convention matches the original** [V].

- **`b` = view+0x64**: on a paused snapshot of all 12 cars, `b` is positive when the planned
  look-ahead target lies toward the view's line lateral (view+0x180).
  - view+0x180 = −(own+0x50) on all 12 cars (dot −0.998 … −1.000).
  - The port's `clockwise(d) = heading(f) − heading(d)` with `heading = atan2(x, z)` is
    positive toward the same side.
- **Mirror**: the original negates both angles when `dot(front − line point, view+0x180) ≤ 0`.
  - The port's `own_lateral` uses the line lateral with the same orientation: its unit test
    asserts lateral = direction − π/2, and live `heading(t) − heading(view+0x180) ≈ +π/2`.
- **`a`**: from the disassembly, `a` is negated when `f.z·t.x − f.x·t.z ≥ 0`. This gives the
  same sign as `b` and the port.
- **Live check through the recovery mode choice**: the player-slot driver object (driver 0)
  ran recovery repeatedly in the first stretch, about 1,960 frames.
  - Every unlatched mode change matched the octant computed with this convention: mode 2 for
    n ≤ 3, mode 1 with full brake for n ≥ 4.
  - Examples:
    - 264.74 s: a = 90.9°, b = 30.6° → octant 8 → mode 1, full brake 1;
    - 265.39 s: octant 1 → mode 2;
    - 274.93 s: octant 4 → mode 1;
    - 279.49 s: octant 8 → mode 1;
    - 280.16 s: octant 1 → mode 2.
  - With both signs flipped, the 264.74 s case would give octant 0 and mode 2, which
    contradicts the game.
  - Mode 3 and 5 reversing was not reached in the traces, because the car never stopped while
    in octant ≥ 4.
- Naming note: the doc's "own+0x50 = right" and the port's "positive lateral = driver's right"
  name opposite axes. The arithmetic is consistent either way.

## Question 3: do stopped or slow cars count as obstructions?

**Statically yes, with constants confirmed live** [V]. Not observed in a live contact.

- Contact callback 0x4022F0:
  - accepts contacts with static bodies;
  - accepts dynamic bodies only when the 0xA69370 value exceeds **700** (0xCAFCD0) and the
    body's speed is below **2 m/s** (0xCB8DA4);
  - skips floor-like normals with y ≥ **0.1** (0xCAFCD4) for one body class.
- A car (the Lacetti is well over 700 kg [I that 0xA69370 is mass]) below 2 m/s therefore counts.
  This matches `MultiRuntime::obstruction(i)` (static contacts plus cars < 2 m/s, floor
  skipped).
- **Live**: no AI car touched the stopped player car. The player car stood 20 m off the track,
  and AI collided_obstruction never fired in trace-race-03.
- The player-slot object registered obstruction contacts with the barriers in the first
  stretch: 2 frames, which toggled `req+0xAF`. This is consistent with the toggle logic.

## Other observations

- **Recovery in traffic**:
  - AI cars entered recovery only for brief off-track moments: 3–41 frames per driver,
    always mode 2, forward, reverse 0;
  - no recover_tbone;
  - no spun AI car.
- **The player-slot driver object** computes AI outputs while the player drives:
  - recovery, lost_control (26 frames), reverse requests;
  - finally `retire` once the idle car stood off track.
  - These outputs do not control the player car [I].
- **After the leaders finish**, AI cars run the `cruise` behaviour.

## Reproduce

```bash
powershell -NoProfile -File tools/trace_grid_ai_avoid_recovery.ps1 -ProcessId <pid> -Output local/ai-avoid-recovery/live/trace-NN.tsv -Seconds 600
```

- The driver base 0x56A3B10 is specific to this process instance.
- In a new process, find it by scanning for the avoid test vtable 0xCF73EC: driver = test −
  0x45F0.
