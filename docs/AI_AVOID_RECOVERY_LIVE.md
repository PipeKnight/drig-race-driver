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

  Neighbouring frames with the same, smoothly varying inputs do not fire. **Cause (F306,
  below):** the original's f32 cosine uses rsqrtps-based lengths and can fall below −1. The
  libm acos then returns NaN, and the `ja` compare treats NaN as "not racing normally". This
  happens only when the target heads within about 5·10⁻⁴ rad of the gate direction. See
  [The 0.36% leak](#the-036-leak-float-cosine-below-1-gives-acos-nan-f303).
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
frames, and it reproduces stopped-car avoid. The leak is now identified (F306); reproducing it
is a separate, unimplemented proposal (see [Port proposal](#port-proposal-not-implemented)).

- **Closed (F306)**: the leak cause. Still open: a live confirmation with raw record and gate
  bits (see the F306 "Open" list).
- The port maps record state 2 to "moving against its nose" [I]. Live records use 1 for racing
  AI and 0 for the stationary player. State 2 was not observed.

## The 0.36% leak: float cosine below −1 gives acos NaN (F306)

15 October 2026, Claude Code. Static work on the hash-guarded analysis image
`local/compatibility/grid-unpacked.exe` (sha256 `aac46a64…4f34514`). It used
`local/ai-overtaking/scripts/pe.py` (capstone) and the existing private disassembly
`local/ai-avoid-recovery/dis/79a530.txt`, plus a reanalysis of `trace-race-03.tsv`.
There was no new live run and no input to the game. Ghidra was not needed; the earlier
decompile (`decomp/a1/79a530.c.txt`) already hides the cause, because it drops the acos
argument and the NaN path.

### Mechanism [V code]

The angle test of the racing-normally exclusion 0x79A530 (0x79A64E–0x79A958), instruction by
instruction:

1. **xz only.** Both the target's forward vector (record +0x30) and `t` are masked with
   `0xCAF460 ^ 0xCAF410` = (FFFFFFFF, 0, FFFFFFFF, FFFFFFFF). Lane y is then set to 0.
   `t = (−n.z, 0, n.x)` from 0x7A2EA0 has w = 0. The heading pre-check at 0x79A55B uses the
   same masking.
2. **Dot product.** `dot = t.x·f.x + t.z·f.z` in f32, as a shuffle-based horizontal sum.
3. **Lengths are estimates, not `sqrtps`.**
   - `r = rsqrtps(x)` and `s = r·x`.
   - `len = s + (1 − s·r)·(s·0.5)`, with constants 0xCAF888 = 1.0 and 0xCAF88C = 0.5.
     Zero inputs are masked to 0.
   - This is a single Newton step from the 12-bit hardware estimate. The step converges from
     below, so both lengths come out slightly short (about 1 ulp near 1.0), and |cos| comes out
     slightly too large.
4. **Cosine.** `cos = dot / (len_t · len_f)` in f32 (`mulss`, `divss`), then `cvtps2pd` and a
   call to 0xBF4067.
5. **0xBF4067 (SSE2 libm acos)** does no clamping and calls no error handler:
   - exactly ±1 gives 0 or π (0xBF44B3);
   - a finite |x| > 1 gives `0 · +inf` = NaN (0xBF4499);
   - a NaN input is returned as NaN (0xBF44FE).
6. **Compare.** `cvtsd2ss`, `comiss acos, max`, `mov al, 1`, `ja` → return 1.
   - NaN is unordered: ZF = PF = CF = 1, so `ja` is not taken.
   - The function returns `bl = 0`, which means **not excluded**.
   - 0x79A960 Branch A then proceeds to its ETA test and fires.

On Donington, every gate's `t` points against the traffic, so a racing car has cos ≈ −1.
When the car's horizontal heading lies within about 5·10⁻⁴ rad (0.03°) of a from-gate's
direction, the f32 cosine can drop below −1. That car then counts as "not racing normally".
The F302 trace emulation computed the cosine in double precision and clamped it to ±1, so it
said "excluded" on exactly these frames.

### Evidence

**Trace [V].** `python -I tools/analyze_avoid_leak.py local/ai-avoid-recovery/live/trace-race-03.tsv`
covers AI drivers with an obstacle ETA < 2 s that the emulation marks excluded: 19,589 rows,
70 fired. The deviation is π − `ob_angle`, which is quantized to 1·10⁻⁵ rad by the trace's
6-digit text.

| deviation from antiparallel (rad) | frames | fired | rate | host-rsqrtps MC |
|---|---|---|---|---|
| 0 – 1e-4 | 17 | 17 | 1.00 | 0.99 |
| 1e-4 – 2e-4 | 17 | 17 | 1.00 | 0.98 |
| 2e-4 – 3e-4 | 19 | 17 | 0.89 | 0.96 |
| 3e-4 – 4e-4 | 20 | 11 | 0.55 | 0.87 |
| 4e-4 – 5e-4 | 19 | 7 | 0.37 | 0.62 |
| 5e-4 – 6e-4 | 26 | 1 | 0.04 | 0.25 |
| 6e-4 – 7e-4 | 24 | 0 | 0 | 0.03 |
| ≥ 7e-4 | 19,447 | 0 | 0 | 0 |

- All 70 leak rows lie within 5.1·10⁻⁴ rad of antiparallel (69 within 5·10⁻⁴).
- None of the 19,447 frames beyond 7·10⁻⁴ rad leaks.
- That rules out chance.
- The doc's earlier count of 71 came from the tool's in-run counter. The TSV holds 70 leak
  rows. The extra counter frame was not investigated.

**Monte Carlo [V on this CPU, I for the input model].** `tools/avoid_leak_rsqrt_mc.rs` runs
the same f32 pipeline with the host's `_mm_rsqrt_ps`. The host is an i7-12700, the same PC that
ran PID 7208. Inputs are random gate yaw, unit 3D forward vectors with ±5.7° pitch, and
deviations per bin. Output: `local/ai-avoid-recovery/leak/mc-host-i7-12700.txt`.

| rsqrt model | P(NaN) per 1e-4 rad bin, 0 … 8e-4 |
|---|---|
| host rsqrtps | 0.99 0.98 0.96 0.87 0.62 0.25 0.03 0.00 |
| exact 1/√x rounded to f32 | 0.36 0.26 0.13 0.03 0.00 0.00 0.00 0.00 |
| 1/√x truncated to 12 significant bits | 0.73 0.67 0.57 0.46 0.35 0.25 0.16 0.07 |
| 1/√x truncated to 13 significant bits | 0.55 0.46 0.29 0.12 0.02 0.00 0.00 0.00 |

- Only the hardware estimate gives the observed shape: nearly certain below 2·10⁻⁴ rad and a
  sharp edge near 5–6·10⁻⁴ rad.
- The model's edge sits about 1·10⁻⁴ rad further out than the trace. Possible reasons [I]:
  - the real distribution of |f_xz|² (pitch);
  - the record's w lane;
  - the record moving between the test and the sample;
  - the 1·10⁻⁵ rad quantization of the trace.

**Why single frames.** The heading sweeps through a window only about 10⁻³ rad wide. The
updater's dt is 0.00606 s (header +0x10), and the trace's median sample spacing is 17.5 ms.
That is about 2.9 AI steps per sample [I], so a leak is usually seen as one sample. Branch B,
which has no exclusion, then stretches it into an episode.

### Order of 0x79EE30 [V code]

The per-driver update 0x79EE30 is called from 0x79F06F. It runs:

1. 0xAB87D0 on driver + 0xA00;
2. 0x79C360 on the view (driver + 0x2DD0; it calls 0x794160, 0x793F60, 0x779A20, 0x798E90,
   0x7923F0 and 0x792580), so the view is refreshed **before** the tests;
3. 0x79CAA0, the arbitration and tests (driver + 0x2AA0);
4. 0x787620 on driver + 0x3100 with the request (+0x2CE0). This is a controller: its only
   calls are to 0xA683B0, and it does not touch the view or the perception records;
5. vtable +8 of `*(driver + 0x9F8)`;
6. 0x79E050 when req+0xB5 is set.

The "stale data" candidate is therefore not needed. Several AI steps per rendered frame do
happen [I], but they only explain why leaks appear as isolated samples.

### Port proposal (not implemented)

This would go in Branch A's `racing` expression in `crates/grid-tools/src/ai_driver.rs`, which
currently reads `dot2(horizontal(r.forward), wrong_way).clamp(-1., 1.).acos() > rs.av_max_racing_angle`.

```rust
/// 0x79A530's angle test as the original evaluates it: xz-only f32 cosine with
/// rsqrtps + one Newton step lengths; acos is NaN for |cos| > 1 and NaN compares false.
fn beyond_track_angle(forward: [f32; 3], t: [f32; 3], max: f32) -> bool {
    let len = |x: f32| {
        if x == 0. {
            return 0.;
        }
        let r = rsqrt_estimate(x); // _mm_rsqrt_ps on x86/x86_64
        let s = r * x;
        (1. - s * r) * (s * 0.5) + s
    };
    let c = (t[0] * forward[0] + t[2] * forward[2])
        / (len(t[0] * t[0] + t[2] * t[2]) * len(forward[0] * forward[0] + forward[2] * forward[2]));
    ((c as f64).acos() as f32) > max
}
```

1. **Pass the raw `r.forward`.** Do not normalize it with `horizontal()`; the original uses the
   unnormalized xz part of the unit 3D forward vector.
2. **Use the original's `t`.** That is `(−n.z, 0, n.x)` of the from-gate normal `n` of the
   target's link (`Line::lateral[i]` for the segment holding `r_arc`, not interpolated).
   - The port currently uses the negated interpolated line direction. Gate normals deviate from
     the line by up to 0.25 rad (unit test note), so the 5·10⁻⁴ rad alignment would happen at
     different places.
   - The original looks up the nearest containing link in its own 32-link view window.
     Projecting onto the line is close but not identical [I].
3. **Choose `rsqrt_estimate`.**
   - `std::arch` `_mm_rsqrt_ps` on x86/x86_64. On the same CPU, this reproduces the original.
   - The original itself depends on the CPU here: the SDM only bounds the error at
     1.5·2⁻¹²; vendors and generations may differ [I].
   - Fallback for other architectures: exact `1/√x` rounded to f32. With it, leaks happen at
     about a third of the rate (MC table).
   - No portable bit-exact Intel table is available.
4. **Consequences.**
   - Multi-car headless results change: rare traffic avoid episodes appear, about 0.4% of AI
     frames in the original.
   - On x86 those results become CPU-dependent, like the original.
   - **Solo laps 88.420 / 86.282 s stay bit-identical by construction.** The changed
     expression runs only inside `if let Some((r, …)) = obstacle`, and `obstacle` is filled only
     from `for r in others`. Solo runs have no other cars.
   - After implementing, rerun the solo headless check to confirm.

**Owner decision (16 October 2026): not adopted.** Race results must not depend on the CPU.
The port keeps the documented rule: a clamped, exact cosine, so the acos-NaN leak never fires.
Behaviour should stay close to the original but without such bugs and leaks. A deterministic
fidelity step remains open: item 2 (the from-gate normal `t` instead of the interpolated line
direction) can be adopted with exact arithmetic, without the rsqrt estimate.

### Open

- **Live confirmation.** Extend `tools/trace_grid_ai_avoid_recovery.ps1` to log the raw bits
  of the target record's +0x30 and the from-gate normal, then replay the f32 pipeline offline
  with `avoid_leak_rsqrt_mc.rs`-style code on the same CPU.
  - Windows PowerShell 5.1 has no SSE intrinsics, so the replay has to happen offline.
  - This needs the owner to unpause a race. It was not done in F306.
- Whether the i7-12700's P-cores and E-cores return identical `rsqrtps` results [I].

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
