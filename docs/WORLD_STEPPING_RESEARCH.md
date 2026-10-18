# Original physics world stepping and solver configuration — F285

15 October 2026. Goal: match the original's wall-collision behaviour exactly, starting
with how the rigid-body/contact world is stepped and configured. Static analysis of the
private analysis image; no game launch.

## World step scheduler `A72930(world, dt)`

Called from physics task `5E7F30` with the frame delta (f32 of the task's double).
With `fps = 1/dt` and world fields `+8` (maxfps), `+C` (minfps), `+10` (maxTimesToRun):

- `dt <= 1e-6`: no step.
- `fps > maxfps`: one step of `1/maxfps`.
- `minfps <= fps <= maxfps`: one step of `dt`.
- `fps < minfps`: `n = trunc(minfps/fps)`, incremented when the fraction exceeds 0,
  clamped to maxTimesToRun; substep `1/clamp(n*fps)` repeated n times. The first n-1
  substeps run `AD8D70(step)`, `A727A0()`, `A71C70()`; the last runs `AD8E30(step)`.
- `AD8D70`/`AD8E30` store the step in world `+7C8`; `+7C4` is a phase word (0..7).

## Configuration source

`A74F00` reads section `PHYSICSENGINE` through the generic config reader; defaults come
from `6195F0` and `A749A0` copies them into the world. No installed file (all extensions,
ASCII and UTF-16, install and user folders) contains the section or its keys, so the
built-in defaults apply unless something overrides them at runtime.

| Key | Default | Prototype today |
| --- | --- | --- |
| maxfps / minfps / maxTimesToRun | 60 / 30 / 1 | fixed 0.016 s, no scheduler |
| numQuickStepIterations | 20 | 4 |
| quickStepW | 1.3 | 0.8 |
| globalCFM / globalERP | 1e-5 / 0.2 | 0.01 / 0.2 |
| minPenetrationDepth | 0.001 | 0.125 |
| firstOrder iterations / W / minConstraintError | 5 / 1.0 / 0.02 | 4 / 0.8 / 0 |
| firstOrderCFM / ERP / minPenetrationDepth | 0 / 1.0 / 0.02 | 0.01 / 1 / – |
| gravity | (0, -9.81, 0) | (0, -9.81 source constant, 0) |
| maxContinuousCollisions, maxContacts, maxContactsPerCollision, ... | 6195F0 words | no continuous collision |

## Contradiction to resolve live

The native paused DT02 captures read world `+7C8` = 0.006061346735805273 s, the 165 Hz
frame delta. With maxfps = 60 the scheduler would step 1/60 s per frame (2.75x real
time). So at runtime world `+8` must be at least the display rate: something overrides
maxfps after `A749A0`. Two candidate store sites (`A70050`/`A70090`) are list iterators,
not setters. A read-only live capture of world `+8/+C/+10/+7C4/+7C8` and the solver
manager settings settles the stepping and confirms whether the solver defaults above are
the live values, before any prototype change. `maxContinuousCollisions` indicates the
original has continuous collision detection, which the prototype lacks entirely. F411 traced it
(world phase 2, B18E20 → B15B70 → B3A700 → AE4700; live world +0x84 = 1) and ported it for prop
pieces and lost wheels against the road: ROBUSTNESS_RESEARCH.md section 7.

## Live capture (resolved)

15 October 2026, owner-started research copy `grid-compat\grid.exe` (SHA b1da1b2d…34a6,
PID 7208), Touring Cars Lacetti / Donington GP race, paused. New read-only tool
`tools/read_grid_world_settings.ps1` (VM_READ + QUERY_LIMITED only) follows car list
`0xE85F28` -> `+DB0` -> `+8` physical -> `+32C` world -> `+98` scheduler (back-link `+4`
verified). Five samples, identical: **maxfps 10000, minfps 0, maxTimesToRun 1**, world
`+7C4` 0, `+7C8` **0.0060602077 s**. The game overrides the config defaults at runtime:
the world takes exactly **one step per rendered frame of the real frame delta**, with no
substeps (6.06 ms at the owner's 165 Hz display).

World `AD8BA0` copies the 15 config words into world `+788`; key order verified against
`A74F00` (defaults 20 iterations / W 1.3 at indices 6/7). Live values:

| Index | Key | Default | Live |
| ---: | --- | --- | --- |
| 0..2 | maxBodies/Constraints/DOFConstraints in island | 1000/1000/6000 | 200/1000/3000 |
| 3 | globalCFM | 1e-5 | **0** |
| 4 | globalERP | 0.2 | **0.2** |
| 5 | minPenetrationDepth | 0.001 | **0.003** |
| 6 | numQuickStepIterations | 20 | **5** |
| 7 | quickStepW | 1.3 | **1.0** |
| 8..9 | firstOrderCFM / ERP | 0 / 1.0 | **0 / 1.0** |
| 10..11 | firstOrderMinPenetrationDepth / MinConstraintError | 0.02 / 0.02 | **0.02 / 0.02** |
| 12..13 | firstOrderQuickStepW / iterations | 1.0 / 5 | **1.0 / 3** |

Prototype today: fixed 16 ms steps, CFM 0.01, min penetration 0.125, W 0.8, 4 iterations,
correction {4 iterations, CFM 0.01, W 0.8, threshold 0}. Evidence private:
`local/world-settings/paused01.json`. The solver manager object (`B7A9E0`) was not reached
from direct world pointers; its copy of these words remains unread.

Private evidence: `local/behavior/20261015T0836028049249Z` (A72930/A02B70) and the later
surveys of A74F00/6195F0/A749A0 and world stores 7C4/7C8 under `local/behavior/`.
