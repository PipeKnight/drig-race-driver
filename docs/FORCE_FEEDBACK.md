# Force feedback and rumble (GameFFB)

How the original GRID drives wheel force feedback and XInput pad rumble, and how DRIG ports it
(backlog row 22). Research of 27 October 2026, static only: Ghidra 12.1.4 through the shared master
project (`tools/re/grid_re.py decompile`, cache `local/re-cache/`), capstone disassembly for the
functions Ghidra has no entry for, and the shipped `forcefeedback/*.xml`. Nothing was launched.
Earlier work: F321 (`PLATFORMS_AND_INPUT_RESEARCH.md` §4.3, the private report
`local/platforms-input/ORIGINAL_INPUT_DEVICES.md`), which found the effects and the device output
but not the snapshot builder; this document closes that gap and corrects a few details.

Labels: **[D]** decompiled/disassembled and read; **[I]** interpretation; **[A]** AUTHORED in
the port (no original equivalent).

## 1. Short answer

- One DirectInput effect: an infinite **GUID_ConstantForce** on the force-feedback axes, direction
  (+1, 0) cartesian, autocentre off [D]. No spring, damper, friction or periodic effect is ever
  created. Friction (damping) is computed in software from the wheel's X-axis velocity and added
  to the same constant force [D].
- Per FFB tick: `magnitude = trunc((force + effects + damping) · 10000)` [D].
- **force** is the self-aligning torque: the mean lateral tyre force of the two front wheels,
  normalised by m·g/4, shaped by `pow(x / cornerPoint, exponent)`, clamped to ±scale and faded in
  between 2.5 and 6 m/s, times each wheel's surface grip [D].
- **friction** = slip-angle friction + speed-dependent static friction (25 standing, 5 at speed),
  clamped per device [D].
- **effects** are vibration samples (digit strings in `effectsetup.xml`): surfaces and kerbs,
  collisions, gear changes, skids, suspension hits and a damaged wheel's once-per-revolution
  thump [D]. On a wheel they add to the force; on an XInput pad they are the only output, channel 0
  to the left (heavy) motor and channel 1 to the right (light) motor [D].
- The FFB runs on its own thread: `Sleep(10)`, so about 100 Hz, fed by per-car snapshots that the
  car manager builds every physics step [D].
- Options: `forcefeedback` on/off (default on), and the Advanced Controls sliders
  `feedback_force`, `feedback_friction`, `feedback_effects`, each 0..10, default 5, read as /10 [D].

## 2. Data: `forcefeedback/*.xml` (processor GameFFB)

`forceFeedbackPaths.xml` loads three files [D]. The port reads them from the user's installation.

| File | Element | Fields (struct offset) | Parser |
|---|---|---|---|
| devicesetup | FFBDevice | scaleForce +2C, scaleFriction +30, baseFriction +34, maxFriction +38, scaleEffects +3C, delay +40, hidden nativeFriction +44. Constructor defaults 1, 1, 0, 10, 1, 0.1, 0 | `7EB540` |
| carsetup | FFBCar | name (car folder), template, scaleForce +2C, scaleFriction +30, scaleEffects +34 | |
| effectsetup | FFBCaster | scale +4, toe +8, camber +C, trail +10, thresholdLow +14, frictionLow +18, thresholdHigh +1C, frictionHigh +20, angleThresholdLow +24, angleFrictionLow +28, angleThresholdHigh +2C, angleFrictionHigh +30, cornerPoint +34, exponent +38, deadzoneLow +3C, deadzoneHigh +40, preload +44 | `7EBCC0` |
| | FFBlateral | scale +4, thresholdLow +8, frictionLow +C, thresholdHigh +10, frictionHigh +14 | |
| | FFBSurface | scale +4, surface id +8 (from `name`, default 0xB), thresholdLow +C, thresholdHigh +10, freqLow +14, freqHigh +18, scaleLow +1C, scaleHigh +20, length +24, sample +28, grip +2C | `7EC1B0` |
| | FFBGearChange | scale +4, gear +8 (−1 = any), freq +C, length +10, sample +14 | `7EC380` |
| | FFBWheelDamage | scale +4, thresholdLow +8, thresholdHigh +C, scaleLow +10, scaleHigh +14, sample +18 | `7EC4E0` |
| | FFBSkid | scale +4, thresholdLow +8, thresholdHigh +C, lengthLow +10, lengthHigh +14, freq +18, sample +1C | `7EC650` |
| | FFBSuspension | scale +4, thresholdLow +8, thresholdHigh +C, length +10, freq +14, sample +18 | `7EC7D0` |
| | FFBCollision | scale +4, thresholdLow +8, thresholdHigh +C, damageZone +10 (never read), freq +14, length +18, sample name +1C | `7EBFD0` |
| | FFBSample | wave, envelope, channel (0), wheelScale (1), rubmlerScale (1) | `7EB2A0`, `7E7DA0` |

Effects are kept in one list in parser order. The effectsetup handler `7ECA30` (reached from the
dispatcher `7ED250`) runs the parsers as samples, caster, lateral, collision, surface, gear
change, wheel damage, skid, suspension [D]. The order matters only because the caster multiplies
the friction accumulated before it by its scale; the lateral friction comes after, unscaled.

**Samples** (`7E7DA0`) [D]: `wave` and `envelope` are copied with `strncpy(.., 16)`, so at most 16
digits each (the shipped 17-digit envelopes lose their last digit). The count is the index of the
last non-zero character + 1. A wave digit is stored as `d − '5'` (−4..4), an envelope digit as
`e − '0'`. Several samples with one name form a set; an effect that names a set draws one member
with `rand()` **at load** (`7EB030`; the surface, gear, damage, skid and suspension effects) or
**each time it fires** (the collision effect, which keeps the name) [D].

The shipped values (examples): caster scale 2, trail 1, toe 0, camber 0, cornerPoint 0.075,
exponent 0.35, speed fade 2.5→6 m/s (friction 0→1), angle friction 17.5 at 6.5° → 6.5 at 25°;
lateral friction 25 at ≤2.5 m/s → 5 at ≥6 m/s; device `default` scaleFriction 0.8, maxFriction 10
(the Fanatec and other unlisted wheels use it), G25 maxFriction 45; `XBOX360 Controller` delay 0.1.

## 3. Snapshots: what the game sends (`7E9D60`)

The car manager (`A00AE0`, every physics step, also `98D7C0`) builds a 0x210-byte snapshot for
each car with an FFB device (car +0x2E80, name string +0x2E84) and queues it with `7EB9C0` [D].
Units are the vehicle record's; `q = 1/(m·2.4525)` (m·g/4, the static load of one wheel).

| Snapshot | Source (vehicle record) | Meaning |
|---|---|---|
| +4 | car folder name (`996ED0`) | matched against FFBCar |
| +0x28 | device name | matched against FFBDevice (`7EAA40`, fallback `default`) |
| +0x130..+0x16C | body matrix (`A69630`) | rotates the wheel axes |
| +0x170 | body linear velocity (`A69800`) | speed for all speed thresholds |
| +0x1B4 | body mass (`A69370`; 1000 without a body) | |
| +0x1B8 | +0x196C | gear (0 N, 1.., 9 R) |
| +0x1BC | −`9961D0(6)`·`9959A0` = +0xE30·+0xE54 | front wheel angle, rad (steering input × lock), the same product `9AE5A0` uses |
| +0x1C0 | −`9961D0(6)` = +0xE30 | steering input |
| +0x1C4 | `9AE5A0` front heading | front-axle velocity heading |
| +0x1C8, +0x1CC | front heading / +0xE54, 1.0 | centre term (§4.6) |
| +0x1D4 | device pointer | |
| +0x1D8 | collision event or null | Δv = \|impulse (`A69930`)\| / m; set when Δv > 0.5 or \|v\|² > 4 |
| +0x1DC | gear event or null | set by the queue (§3.1) |

Two wheel records of 0x70 bytes at +0x50 (front-left = vehicle wheel 2 at +0x1C30) and +0xC0
(front-right = wheel 3 at +0x1D80), from wheel record `w` (stride 0x150) [D]:

| Record | Source | Meaning |
|---|---|---|
| +0x00 | (w+20, 0, −w+24) rotated | tyre lateral axis (docs/TYRE_AXES.md) |
| +0x10 | (0, 1, 0) rotated | up |
| +0x20 | (w+24, 0, w+20) rotated | tyre forward axis |
| +0x30 | w+B0 · q | lateral tyre force (vehicle_force.rs writes +B0/+B4) |
| +0x34 | w+138 · q | load |
| +0x38 | w+B4 · q | longitudinal tyre force |
| +0x40 | velocity of the contact point (`A697C0` of w+0..8) − w+E0·w+104 · forward axis | slip velocity |
| +0x50 | w+C0 | surface material (+0x8C = surface type) |
| +0x54 | w+138 · q, 0 below 0.1 | "on the ground" load |
| +0x58 | w+3C | suspension speed |
| +0x60 | 1.0 if car byte +0x22E0+i, else `99A540(i)` | wheel damage |
| +0x64 | w+104 / 2π | wheel revolutions per second |
| +0x6C | car byte +0x22E4+i | wheel detached |

On a wheel device the front heading also gets `(w2+144 + w3+144)·0.5·[E7B204]`; nothing writes
`E7B204` (one read, no other reference), so the term is zero [I].

### 3.1 The queue (`7EB9C0`, under the GameFFB lock) [D]

Only while FFB is enabled (+0xA0) and not paused (+0xA1):
1. The snapshot is copied; its play time is `clock + device.delay` (+0x1D0).
2. Gear event (`7E8340`) against the device's previous snapshot: when the gear changed, count
   which of {neutral 0, first 1, reverse 9} either gear is; with fewer than two, the event is
   `{upshift = new > old && new != 9, gear = new}`. So N↔1, N↔R and 1↔R make no event.
3. Centre term: `x = +0x1C8`, `k = +0x11C` (the wheelLinearity response), `e = 1/(1+k)` for
   k ≥ 0 else `1 − k`; `x = sign(x)·|x|^e`; times `+0x118` (steering saturation × the 828670 ratio)
   when that is > 0 and ≠ 1; `+0x1CC = +0x118`.
4. The copy is appended to the pending list and kept as the device's previous snapshot.

## 4. The FFB task (`7ED380` → `7ECAC0`)

`7ED380` sleeps 10 ms, measures dt (`4683A0`), updates the input devices (which ticks each FFB
device, §5), then under the lock advances the clock by dt and processes every snapshot whose play
time has come (`7EBB80`) [D].

Per snapshot, a zeroed accumulator block (floats): [0] friction, [1]/[9] left/right force,
[2]/[10] left/right camber force, [3]/[11] left/right grip, [0x11..0x13] gains, one-shot list
(10 entries) and loop list (10 entries), [0x7A] force, [0x7C] friction, [0x7D] centre term.
Gains = (feedback_force, feedback_friction, feedback_effects) × the FFBCar scales (exact name
match, `A66720`) [D]. Then every effect's update runs, then `7EA6F0` (§4.7) [D].

### 4.1 Caster (`7E8580`) [D]

```
if !detached L: [1] = −trail·latL + toe·longL;  [2] = scale·(−camber)·loadL
if !detached R: [9] = −trail·latR − toe·longR;  [10] = scale·camber·loadR
c = ([1] + [9]) / 2
a = |c| + preload
g = a > 0 ? pow(a / cornerPoint, exponent)·scale·cornerPoint / a : 1   (pow in double)
c' = c > 0 ? c + preload : c < 0 ? c − preload : c
[1] = [9] = clamp(g·c', −scale, scale)
v = |velocity|            (SSE rsqrt + one Newton step)
s = lerp(frictionLow, frictionHigh) over v in [thresholdLow, thresholdHigh], clamped
t = clamp((|steer − heading|·57.295776 − angleThresholdLow) / (angleThresholdHigh − angleThresholdLow), 0, 1)
L = (detached L ? 0 : clamp(loadL, 0, 4)) + (detached R ? 0 : clamp(loadR, 0, 4))
[0] = ((angleFrictionLow + (angleFrictionHigh − angleFrictionLow)·t)·L·0.5·s + [0]) · scale
[1], [9], [2], [10] *= s
f = deadzoneLow < deadzoneHigh ? clamp((|heading|·57.295776 − deadzoneLow)/(deadzoneHigh − deadzoneLow), 0, 1) : 1
[0x7D] = clamp(snapshot+0x1C8 · f, ±snapshot+0x1CC · 0.95)
```
The hidden defaults deadzoneLow 3 > deadzoneHigh −5 make f = 1.

### 4.2 Lateral (`7E80F0`) [D]

`[0] += scale · lerp(frictionLow, frictionHigh)` over v in [thresholdLow, thresholdHigh]
(frictionLow at or below thresholdLow, frictionHigh at or above thresholdHigh).

### 4.3 Surface (`7E9550`) [D]

For each front wheel with a material whose surface type equals the effect's and load > 0:
`grip[i] = grip`; and a loop sample `{sample, amp = v > thLow ? lerp(scaleLow, scaleHigh)·scale : 0,
freq = lerp(freqLow, freqHigh), length}`, where the lerp parameter is (v − thLow)/(thHigh − thLow)
clamped at the top. A wheel on a surface no effect names keeps grip 0, so it adds no force.

### 4.4 One-shot effects [D]

| Effect | Condition (per front wheel unless noted) | Entry {amp, freq, length} |
|---|---|---|
| Gear change (`7E9810`) | gear event and (gear = −1 or gear = event gear); per car | {scale, freq, length} |
| Wheel damage (`7E98D0`) | load > 0, not detached, thLow ≤ damage ≤ thHigh, rev/s ≥ 1 | {lerp(scaleLow, scaleHigh)·scale, rev/s, 1.0} |
| Suspension (`7E9BD0`) | load > 0, not detached, thLow < suspension speed < thHigh | {scale, freq, length} |
| Collision (`7EB190`) | collision event, thLow < Δv < thHigh; per car; sample drawn now | {scale, freq, length} |

One-shots are added to the playing list with start time = clock and play until `length` has
passed. A condition that holds for several snapshots adds a copy each time. The lists hold at
most 10 entries per snapshot.

### 4.5 Skid (`7E99E0`), a loop sample [D]

For each front wheel with load > 0.5, not detached: `u = |lateral axis · slip velocity|`; when
thLow < u < thHigh: `t = clamp(u − thLow/(thHigh − thLow), 0, 1)` (operator precedence as in the
original) and the loop sample `{scale, freq = lerpLength(t), length = freq}` — the update puts the
`lengthLow/High` lerp in the frequency slot and `freq` in the length slot [D].

### 4.6 Loop samples and playback

After the snapshots, each device's effect channels are cleared (`(0, 0, 0)`). A snapshot's loop
samples replace the device's previous loop set. Playback (`7E7E90`) at clock T [D]:
```
t = T − start;  playing while 0 ≤ t < length
e = env[m == 1 ? 0 : floor((m − 1)/length · t)] · 0.111111 · (wheel ? wheelScale : rubmlerScale)
w = wave[n == 1 ? 0 : floor(n · freq · t) mod n] · e · 0.25 · amp
wheel or channel 0: channel 0 += w;  rumble device and channel 1: channel 1 += w
```
`amp` was multiplied by the effects gain when queued. Loop samples restart each period:
`start = floor(T/length)·length`. Finished one-shots are removed.

### 4.7 Device apply (`7EA6F0`) [D]

```
force    = ([1]·[3] + [9]·[11] + [10] + [2]) · gainForce
friction = ([3] + [11]) · gainFriction · [0]
with a device setup: force *= scaleForce
                     friction = clamp(scaleFriction·friction + baseFriction, baseFriction, maxFriction)
                     gainEffects *= scaleEffects
device: SetForce(force), SetFriction(friction), native = nativeFriction != 0, centre = [0x7D]
```

## 5. Device output (vtable 0xD43024)

Slots: +0 tick `B8CD80`, +4 is-wheel `B8CD40` (constant-force effect exists), +8 SetForce
(+0x24), +0xC SetFriction (+0x2C), +0x18 add to channels (+0x30, +0x34, +0x38), +0x24 native flag
(+0x18), +0x28 centre (+0x40) [D]. Nothing in the tick reads the centre term [D].

**DirectInput wheel** (`B8CB10` create, `B8CD80` tick) [D]:
- `SetProperty(DIPROP_AUTOCENTER, DIPROPAUTOCENTER_OFF)`; `CreateEffect(GUID_ConstantForce,
  {dwSize 0x38, DIEFF_CARTESIAN|DIEFF_OBJECTOFFSETS, INFINITE, gain 10000, no trigger,
  cAxes = actuator count, direction (1, 0), DICONSTANTFORCE})`; the damping axis is
  `win_con_di_axisX`.
- Tick: `F = force + ch0`, `fric = friction + effectFriction(+0x3C, never set)`. With no native
  friction effect (always, nothing creates one):
  ```
  pos  = 2·axisX − 1                (axisX = raw/65536)
  vel  = dt > 0 ? ((vel + pos − pos_prev) / dt) · 0.005 : 0
  damp = vel · fric;  if damp·damp_prev < 0: damp = (damp + damp_prev) · 0.5
  F   += damp
  SetParameters({lMagnitude = trunc(F·10000)}, DIEP_START|DIEP_TYPESPECIFICPARAMS|DIEP_DIRECTION)
  ```
  Note `vel` keeps a fraction of its previous value (vel + Δpos)/dt·0.005 — a leaky velocity [D].
  DirectInput forces push *away* from their direction [I], so a positive magnitude pushes the
  wheel towards negative X; the damping term only opposes motion with that reading.
- **XInput pad** (`+0xC` set): `XInputSetState(user, {|clamp(ch0, −1, 1)|·65535,
  |clamp(ch1, −1, 1)|·65535})`. The steady force and friction never reach the motors [D].

## 6. Settings [D]

- Profile `forcefeedback` (default 1, `5F1100`); race setup `654C20` passes it to GameFFB
  `7EB110` (enable; disabling zeroes every output). The front end has
  `options_controls_forcefeedback` and `options_controls_vibration` screens [D]; that both edit
  this one flag is [I].
- `feedback_force`, `feedback_friction`, `feedback_effects`: Advanced Controls sliders, default
  5 of 10, written to GameFFB +0x94/+0x98/+0x9C by `654C20` [D].

## 7. The port (DRIG)

| Part | Where | Label |
|---|---|---|
| XML setup (a small attribute reader; atof-style numbers, so `"0.5f"` reads 0.5), snapshot builder, GameFFB task with every effect, sample playback, device apply, DirectInput damping and magnitude, XInput motor scaling | `crates/grid-app/src/force_feedback.rs`, pure, unit tests against §2–5 | ORIGINAL |
| DirectInput 8 wheel: enumerate, exclusive background acquire, range 0..65535, dead zone 0, autocentre off, one infinite constant force on X with direction (1, 0), `SetParameters` every 10 ms; the wheel's axes, buttons and POV for input | `crates/grid-app/src/force_feedback/windows.rs`: a thin hand-written COM binding through the vtables, `raw-dylib` (dinput8, xinput1_4, user32, kernel32), no new crate | ORIGINAL output; device choice [A] |
| The FFB task thread: `Sleep(10)`, device tick, then the task | same file, thread "FFB updateTask" | ORIGINAL |
| XInput pad motors | same file, `XInputSetState` to the first connected pad | ORIGINAL scaling; pad choice [A] |
| One snapshot of the player's car per physics tick; pause | `crates/grid-app/src/ffb.rs` | ORIGINAL data, per tick [A] |
| Wheel input: `win_con_di_*` sources, the original preset whose deviceType equals the wheel's product name (G25, MOMO, DFP, RGT), else a generic preset; `device=auto` with a wheel gives the wheel class (unassisted steering) | `crates/grid-app/src/input.rs` | ORIGINAL presets; generic preset [A] |
| Android pad rumble and phone vibration (§7.2) | `crates/grid-app/src/force_feedback/android.rs`, `android.rs` `rumble`, `android/.../Rumble.java` | [A] |

Why a hand-written binding: the `windows` crate (0.62.2, already in Cargo.lock through Bevy) has
the DirectInput 8 COM wrappers, but enabling its HumanInterfaceDevice feature in grid-app adds a
large crate build and still lacks the `c_dfDIJoystick` data format, which lives in `dinput8.lib`'s
static part. The binding needs about 15 vtable slots and a dozen structures from `dinput.h`.
SDL3 (F321's plan) would add a C library; the original's single constant-force effect needs none of
its effect types.

Differences from the original:
- The task is fed once per fixed 16 ms physics tick (the original once per car-manager step, one
  per frame).
- Wheels other than the original's four named presets get the AUTHORED generic preset: X
  steering, pedals on Y and Rz (uniNegative), Gear Up/Down on buttons 4/5, Handbrake 6, Change
  View 17 (the G25 preset's), all settings (`[input]` `wheel_*`, docs/SETTINGS.md). Only axes
  the device has are bound.
- `DISCL_EXCLUSIVE|DISCL_BACKGROUND` on a hidden window of the FFB thread [A: the original's
  flags are open]; without force feedback, or when exclusive access fails, the wheel is read
  non-exclusively.
- The outputs go to zero 0.25 s after the race stops queueing snapshots (menus, the end of a
  race) and while the game is paused [A].
- `rand()` for the sample sets is the MSVC generator seeded with 1 [A]; the original shares the
  CRT generator with the rest of the game.

### 7.1 Settings (`[input]`)

| Key | Values | Original |
|---|---|---|
| `force_feedback` | `on` (default) / `off` | profile `forcefeedback` |
| `feedback_force`, `feedback_friction`, `feedback_effects` | 0..10, default 5 | Advanced Controls sliders, /10 |
| `wheel_accelerate`, `wheel_brake` | `x`, `y`, `z`, `rx`, `ry`, `rz`, `slider0`, `slider1` (default `y`, `rz`) | [A] generic preset |
| `wheel_pedals` | `inverted` (default: released at the top of the axis) / `normal` | [A] |
| `wheel_gear_up`, `wheel_gear_down`, `wheel_handbrake`, `wheel_view` | button numbers 0..63 (default 4, 5, 6, 17) | [A] |
| `wheel_trace` | `on` / `off` (default): log `drive_input_wheel_trace` lines with the axes and buttons when they change, to find a wheel's pedal axes and buttons | [A] |

### 7.2 Android [A]

The original has no Android version; this part is AUTHORED. The phone runs the same task with
the original's `XBOX360 Controller` device (delay 0.1 s, `scaleEffects` 1). Its two motor
channels carry only the effect samples: surfaces and rumble strips, gear changes, collisions,
landings and bumps, skids and a damaged wheel. The steady self-aligning force and friction never
reach motors (§5), so nothing buzzes in a plain corner.

- The task thread is the same as on Windows ("FFB updateTask", `Sleep(10)`); its motor levels
  become 8-bit amplitudes. `Vibration` (pure, tested) sends a change at most every 30 ms, a stop
  at once and a running level again every 60 ms, so JNI is called about 15–33 times a second
  while something vibrates and not at all otherwise.
- `MainActivity.rumble(left, right)` runs `Rumble.java` on the UI thread. A connected pad with
  vibrators (`InputDevice.getVibratorManager()`, API 31) gets the left level on its first
  vibrator and the right one on its second (a single vibrator takes the larger). A pad without
  vibrators gets nothing, and the phone stays still because it is probably not in the hand. With
  no pad the phone's own vibrator plays the larger level, with amplitude control when it has it,
  else on from level 128. Each call plays a 100 ms one-shot, renewed while the level lasts.
- The manifest asks for `android.permission.VIBRATE`, a normal permission granted at install.
- `[input] force_feedback = off` stops it; `feedback_effects` (0..10, default 5) scales it.

## 8. Open

1. The original's `SetCooperativeLevel` flags and its device enumeration filter (F321 item 4).
2. A live trace of `trunc(F·10000)` against a known manoeuvre would verify the whole chain; it
   needs the owner's go for a capture (docs/CAPTURE_BACKLOG.md).
