# Replay director and Flashback cameras: audit against the original

19 October 2026, Claude Code. This audits the port's replay director, slow motion and the Flashback
and post-race replay flow (F360, F366) against the original's executable and the shipped track
camera data. It is static research only. No live process was run, and nothing was sent to the
original game.

Markers: **[D]** read directly from the executable or shipped data, **[V]** verified by a test or
run, **[I]** interpretation, **AUTHORED** the port's own choice.

## 1. Evidence

Private evidence, not published:

- `local/replay/decomp/d1`…`d9` in the main checkout: Ghidra 12.1.4 headless pseudocode of the
  hash-pinned unpacked image `local/compatibility/grid-unpacked.exe` (SHA-256 `aac46a64…`). It was
  opened `-readOnly` with the F360 helper `local/replay/ghidra/ReplayRefs.java`.
- This worktree's `local/replay-director/`:
  - `pe.py`, a small private PE reader for strings, floats, tables and references;
  - the vtable dumps `vt_*.txt`;
  - `emu_math.py`, an emulation attempt that was not run because Unicorn is not installed here.
- The decoded `route_0/replay_camera_config.xml` of four circuits (`local/replay/cam-*.txt`).

## 2. The camera classes [D]

The replay camera parameters are bound by two classes. Each is created from a `Camera type=` entry.

| Class | vtable | Constructor (defaults) | Copy | Binder | Activation | Update |
|---|---|---|---|---|---|---|
| zoom | 0xD001A8 | 0x823FD0 | 0x841140 | 0x7F7590 | 0x843700 | 0x824160 |
| static | 0xD000D8 | 0x823130 | 0x840E10 | 0x7F57C0 | 0x8435E0 | 0x8231D0 |

The other camera classes have vtables 0xD00008, 0xD00070, 0xD00140, 0xD00210 and 0xD00278. They
belong to the race and free cameras and are not used by the replay director.

### 2.1 Zoom camera fields (binder 0x7F7590, defaults 0x823FD0)

| Offset | Name in the binder | Default | In the shipped files |
|---|---|---|---|
| +0x110 | nearPlane | base camera | absent |
| +0x114 | radius | 2.5 | yes |
| +0x118 | lookahead | 0.025 | yes |
| +0x11C | lookaheadMinDistance | 5.0 | absent |
| +0x120 | lookaheadMaxAdjust | 0.33 | the files say `lookaheadMaxAdjustment` (0.33) |
| +0x124 | maxTurnAngle | 50 | absent |
| +0x128 / +0x12C / +0x130 | minimumFov / maximumFov / defaultFov | 5 / 175 / 60 | yes |
| +0x134 / +0x138 / +0x13C | turnAcceleration / turnDamping / maxTurnSpeed | 0.5 / 3.0 / 6.0 | yes |
| +0x140 / +0x144 | verticalAdjustment / horizontalAdjustment | 0.5 / 0.5 | yes |
| +0x148…+0x164 | shakeAngle, shakeRadius, shakeDamping, minimumShakeSpeed, maximumShakeSpeed, shakeScaleX/Y, shakeRotationSpeed | 0, 0, 1, 0, 300, 0, 0, 1 | yes |
| +0x168 | zoomPerSecond | 1.0 | yes |
| +0x16C / +0x16D | zoomEnabled / turnEnabled | true / true | yes |
| +0x170 | roll | 0 | yes |
| +0x180 / +0x190 / +0x1A0 | position / targetOffset / sourceOffset | — | position, targetOffset |
| +0xE8 / +0xEC | sourcePath / targetPath | — | yes (0x823A00) |
| +0x60 / +0x64 | target entity / source entity | — | — |

`dynamicAdjustment` has no binding: the original ignores it.

### 2.2 Static camera fields (binder 0x7F57C0)

The static camera binds nearPlane, `fov`, position, orientation and the five shake values. It binds
**no turn parameters**. Its update 0x8231D0 sets the field of view (fov × π/180), the near plane,
the position and the orientation, and nothing else. A static camera therefore never turns. F360
turned the two static cameras whose files say `turnEnabled=true`.

### 2.3 Zoom camera update (0x824160 and its helpers)

- **Target** (0x8199B0):
  - with a target path: its point (§2.4);
  - otherwise: the target car's matrix applied to `targetOffset`, which is a car-local offset.
- **Eye** (0x819D20): a source entity with `sourceOffset`, else the source path, else `position`.
- **Eye on the target**: when the eye is within √0.001 m of the target, it is lifted to
  target.y + 1.
- **Field of view** (0x819820, with the goal from 0x808C30):
  - fixed `defaultFov` when `zoomEnabled` is false, `minimumFov == maximumFov`, or
    `zoomPerSecond == 0`;
  - otherwise the goal is **atan(radius / distance)**, clamped to the limits in radians. The
    function 0xBF6F00 has the Intel-libm atan argument reduction `(|x| − c)/(1 + |x|·c)` [I: not
    emulated];
  - the current value moves towards the goal by `zoomPerSecond × k × dt` degrees per update. When
    the limits are at least 1 s of zoom apart, k = clamp(distance to the limit being approached /
    (zoomPerSecond × 0.5°), 0, 1); otherwise k is effectively 1.
- **Activation** (0x843700): the current FOV starts at `defaultFov`. It starts at the framing goal
  only when `defaultFov` is 0.
- **Orientation** (0x808E20):
  - without `turnEnabled`: the look at the target, with `roll`;
  - with `turnEnabled`: the angle between the old and new view directions is
    acos(clamp(dot, −1, 1)). Below 1e−4 rad, or with dt ≤ 0, the wanted angular velocity is 0.
    Otherwise it is (axis × angle / dt), where the axis is the normalised cross product of the old
    and new directions;
  - then ω += turnAcceleration·(wanted − ω)·dt, ω −= turnDamping·ω·dt, and |ω| ≤ maxTurnSpeed;
  - the view direction then turns by |ω|·dt.
- **Shake** (0x818610): only when the update's context flag (`param_2[3]`) is non-zero.
- **Near plane**: after the update it gains 0.8 × clamp((15° − fov)/13°, 0, 1). The constants are
  0.2618 rad, 0.2269 rad and 0.8.
- **Lookahead**: with no target path and lookahead > 0, a vector record is smoothed per update. The
  x and z components use rate dt and y uses 0.5·dt. The target is then recomputed. The exact use
  was not decoded.

### 2.4 Camera paths (0x81E740, 0x7F2880, 0x80FF40)

A `Path` element carries `type`, `duration`, `loop`, `local`, `percentageCurve`,
`targetSpeedRelative`, `minimumSpeed`, `maximumSpeed`, `targetSpeedRelativeProportion`,
`matchSourcePathDuration` and `useTargetProjection`. It is driven in one of three ways:

| Mode | Paths in the 4 files | Rule |
|---|---|---|
| `useTargetProjection` | 56 | Project the target onto the path (0x80FF40, a fraction) |
| `targetSpeedRelative` | 53 | A clock taken from the target entity (`+0xC` object `+0x2E70`, 0x9094F0), never decreasing |
| neither | 10 | The time since activation |

- **Projection**:
  - On activation, the target is shifted by path(0) − target, so that it projects onto the path's
    start. The shifts are stored in record +0xD0 (source) and +0xE0 (target path);
  - each update projects the shifted target. A fraction below the stored one keeps the previous
    point: the camera never goes back;
  - otherwise param = (1 − w)·param + w·new, with w = clamp(|speed in mph| × 0.01, 0.1, 0.9). Speed
    is converted from m/s by × 3600 × 0.001 × 0.6213712 (0xA7DE00); the speed itself comes from the
    target's vtable +8 [I].
- **Time** (0x7F2880):
  - looped paths wrap (`fmod`), and a non-looped path stops at `duration`;
  - fraction = t / duration, mapped through the `percentageCurve` when one is named. The curve is
    `linearPercentage`, or `bezierPercentage` with four values (control times 0, ⅓, ⅔, 1).
  - The 10 time paths are the race-start initial cameras, 2.67 s and 6.67 s long.
- **`local`**: a path's `+5` flag. In the time branch, a local path is evaluated in the target car's
  frame. Every shipped path says `local=true`, yet the initial-camera points are world positions
  next to the grid. So the flag's meaning is unresolved [I].

### 2.5 Recorded camera state

The zoom camera is a replay `Recordable`. 0x7F7810 serialises its record:

- the current FOV at +0x70;
- the path parameters at +0xA0 and +0xA4;
- the points at +0xB0, +0xC0, +0xD0 and +0xE0;
- the clocks at +0xF0 and +0xF4.

0x81BCA0 and 0x81A110 interpolate the record between frames. So the original does not re-run the
camera on playback: it replays the recorded 30 Hz state.

### 2.6 Zones and cuts

The zone and activation flow is as F360 found it [D]: `ActivationZone` → `EventHandler entry` →
`ActivateCamera`, with one ordinary and one `TERMINAL_` camera per zone. No minimum shot length and
no cut-timing rule was found in the camera code. A cut happens at the zone crossing.

## 3. Gaps and what was ported (F441)

| Item | F360 port | Original | Now |
|---|---|---|---|
| Target point | car + offset in world axes | car matrix × offset (car-local) [D] | ported |
| Zoom goal | 2·atan(3·radius/d) (AUTHORED) | atan(radius/d), clamped [D] | ported |
| Fixed FOV cases | only `Fov` cameras | `defaultFov` when zoom is off, min = max, or rate 0 [D] | ported |
| Shot start | at the framing goal (AUTHORED) | at `defaultFov` [D] | ported |
| Zoom easing near the limits | none | k factor [D] | ported |
| Negative `zoomPerSecond` (one Long Beach camera) | magnitude | zooms away from the goal [D quirk] | magnitude kept (intended behaviour) |
| Path mode | nearest point every frame | projection that never goes back, with speed smoothing / time / speed-relative [D] | projection and time ported; speed-relative driven like time [I] |
| Activation shift of projected paths | none | path(0) − target [D] | ported |
| `targetPath` | ignored | target on its own path [D] | ported |
| Percentage curves | ignored | linear / Bézier [D, Bézier timing I] | ported |
| Zoom turn | always snaps | `turnEnabled` → angular dynamics [D] | ported |
| Static turn | turns when `turnEnabled` | never turns [D] | ported |
| Camera state over seeks | per-frame state | recorded 30 Hz state, interpolated [D] | stepped at each replay sample from the activation and interpolated; deterministic on seek |
| Shake | none | only with the context flag [D] | not ported (when the flag is set was not found) |
| Lookahead | target + lookahead × velocity (AUTHORED) | smoothed vector record [D, use not decoded] | AUTHORED kept |
| Near plane | default | +0.8 × clamp((15° − fov)/13°) [D] | not ported (base near plane unknown) |
| vertical/horizontalAdjustment, lookaheadMinDistance, maxTurnAngle | ignored | bound, use not found | open |

## 4. Slow motion, frame step and Flashback UI

These come from F360 and F366 and are confirmed by this audit's strings [D]:

- Replay modes 5 and 6 are slow motion and slow reverse (× 0.1). Modes 8 and 9 are frame steps of
  ±1/60 s (docs/REPLAY.md §2.4). The slow-motion toggle has **no keyboard or pad binding** in the
  shipped action maps, so the original's player cannot reach it through the interface.
- `slowMo` / `rampSlowMo` / `selectReplayCamera` / `selectOnCarCamera` / `restoreInGameCamera`
  (0x579FE0) are commands of the scripted race-intro and cutscene system, not the replay player.
- `postrace_slowmo_tick` / `postrace_slowmo_bullet` (0x508A36) belong to the post-race front-end
  animation.
- The Flashback transition (`flashback_flash1..3`, `replay_jump_in`) and the replay interface
  buttons are as in docs/FLASHBACK.md. The transition's screen effects are still not ported.
- Post-race flow: View Replay → `post_race_replay_stop/begin/play`, looping (REPLAY.md §2.5). The
  results screen's "funky" camera is `replay_roof2` (`StatePostRaceSetReplayCamera`).

Nothing new to port here without a binding. Frame step and slow motion stay unbound, as in the
original.

## 5. Checks

- Unit tests (`replay::director`):
  - zone crossing;
  - start at `defaultFov`, then zoom to the atan goal clamped at the minimum; equal limits hold
    `defaultFov`; a negative rate still zooms towards the goal; static cameras never turn;
  - a projection path follows the car forwards only, with weights 0.1 and 0.9;
  - time paths follow linear and Bézier curves;
  - a turning camera lags, then settles.
- Asset tests: `replay_cameras::shipped_route_0_directors` (now with target paths) and
  `replay::race_tests` (frames bit-for-bit, Instant Replay leaves the race identical, View Replay)
  pass [V].

## 6. Open

- The speed-relative path clock and its `minimumSpeed`, `maximumSpeed` and proportion.
- Shake, and when the context flag is set.
- The lookahead record.
- The adjustment parameters.
- The `local` flag.
- The near plane's base value.
- The spline's parametrisation: the port uses a uniform per-segment parameter [I].
- A windowed comparison with the original's replays by a person.
