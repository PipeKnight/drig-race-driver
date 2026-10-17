# Driving assists: research, port and test matrix

15 October 2026, Claude Code. Static analysis only: Ghidra 12.1.4 (read-only private copy of the
phase3 project, the original was locked by another session), Capstone 5.0.6 sweeps of the
hash-pinned analysis image (SHA-256 `aac46a64…4514`) and the shipped data. No live process was
attached. Private evidence: `local/driving-assists/` in the main checkout (`decomp/d1..d8`,
`human-update.asm`, `input-writer.asm`, `disp-writes.txt`, `scripts/`, `runs/`, `traces/`).

Markers: **[V]** verified (numeric comparison, test or run), **[D]** read directly from
decompiled code or data, **[I]** interpretation.

## 1. What the original exposes

The *Driver Aids* screen (`options_driver_aids`, `feNonPersistentDatabase.pssg`) has four toggles:
`traction`, `steering`, `gearbox`, `brake` [D]. English labels (`language_eng.lng`) [D]:

| Toggle | Profile option | Label / help text | Default |
|---|---|---|---|
| traction | `driveraid_traction` | Traction Control: "reduces wheel-spin and improves cornering" | 1 = On |
| steering | `driveraid_steering` | Stability Control: "reduces understeer" | 1 = On |
| brake | `driveraid_brake` | Braking Assist: "eliminates lock-up" | 1 = On |
| gearbox | `transmission` | Transmission: Automatic / Manual | 0 = Automatic |

- Defaults come from the profile-defaults builder `FUN_005F1100` [D]; the owner's live
  reference race (F277/F279) showed exactly this set on screen.
- A fifth option, `driveraid_surface` (Surface Assist), has a default of 1 and a label and a reward,
  but is not on the screen. No code reads it except the defaults builder [D]: no effect [I].
- **There is no separate ABS or clutch option.** The ABS-like behaviour belongs to Braking
  Assist. The shipped keyboard, pad and wheel action maps have no clutch action, and the human
  controller always writes 0 to the clutch slot [D].
- **Steering assistance is not an option.** It depends on the input device (§2.4).
- The difficulty (`difficulty`, default 2) and the `DifficultyManager` database only feed AI and
  career values. Nothing writes the aid bytes from them [D]. `rewards.xml` gives bonus percentages
  for racing with aids off: tractionControl, steeringAssist, breakingAssist, manualGears and
  surfaceAssist 5 each, realisticDamage 5, fixHeadCamera 10, proMode 20 [D]. That is a reward
  table, not a bundle of assists.
- An event or session flag (`FUN_006572D0`: object +1C0 == 1; network "assists disabled") forces
  all three aid bytes to "off" on the car [D].

## 2. Where each setting goes

### 2.1 Aid options → player car bytes

`FUN_00654C20` applies the options to the local player's car (`FUN_009EE380(0)+24`). It runs
when the options are applied and at race set-up (callers 57AD79, 6566AE, 65B897) [D]:

| Vehicle byte | Value | Meaning |
|---|---|---|
| +1558 | `driveraid_steering != 1` | Stability Control **off** |
| +1559 | `driveraid_brake != 1` | Braking Assist **off** |
| +155A | `driveraid_traction != 1` | Traction Control **off** |

The standard constructor `A0C9F0` clears all three [D]. AI cars never get them, so the AI drive
with every aid on [D].

### 2.2 Transmission → gearbox mode → +1970/+1971/+1972

- The human controller (`FUN_007FBA50`, vtable `CFF25C`, name `"human"`) is built with gearbox mode 4.
- `FUN_00606E80` then sets the mode to **2 when `transmission != 0`** (Manual) and to **4 otherwise**.
  It also uses 4 when game object +21EC is zero [D]. What +21EC means is not known [I].
- Every frame, `FUN_00828670` passes the record's mode to `FUN_0098F110(car+18C8, mode)` [D].

| Mode | +1970 automatic | +1971 clutch assistance | +1972 brake-to-reverse | Reached by |
|---|---|---|---|---|
| 0 | 1 | 0 | 0 | none found |
| 1 | 0 | 0 | 0 | none found (manual clutch) |
| 2 | 0 | 1 | 0 | Manual option |
| 3 | 1 | 1 | 0 | none found |
| 4 | 1 | 1 | 1 | Automatic option, all AI cars, race restart (5C63C0/5F8940/60DB40/A0A370) |

### 2.3 Gear controls

The human update (`FUN_00827D10`) builds a per-frame record [D]:

- Gear Up / Gear Down are actions 6/7, edge-triggered. They go to slots 3/4, which are bytes +E24
  and +E28.
- The H-shifter actions 0x10..0x16 select gears 1..6 and −1 (reverse). They go to slot 5, the
  +E2C request.
- The record clears these slots after writing them (`828670`: `[esi+8]`, `[esi+9]` and
  `[esi+0xC]` set to 0) [D].
- The keyboard preset 0 binds Gear Up = A and Gear Down = Z. The gear 1..6 and reverse actions
  are unmapped on the keyboard and mapped on the G25 [D].
- In automatic mode, `9B40D0` clears +E24/+E28/+E2C before the automatic selection, so manual
  requests are ignored [D]. The port already contains `9B40D0`
  ([VEHICLE_CONTROLS](VEHICLE_CONTROLS.md)).

### 2.4 Input device → steering assistance (+E6C)

The human update sets record +1F to 1, and `828670` copies it every frame to +E6C [D]:

- If the debug config sets `input/device_type`, `pad` keeps 1 and `wheel` gives 0.
- Otherwise, an attached analog controller turns it off (0) unless its name is
  "Logitech RumblePad 2 USB".
- Keyboard and pads therefore get **assisted steering** (+E6C = 1). Wheels get **unassisted
  steering** (+E6C = 0).
- AI and scripted controllers set +E6C = 1 (`6E2A90`, `91DF40`).
- `9B3110` then selects the assisted or the unassisted steering writer. Both are verified
  ([STEERING](STEERING.md)) [V].

### 2.5 Wheel command shaping (`828670`, human, +1F clear, mode ≠ 1) [D]

The input is s, the raw steering axis difference (right − left). The vehicle receives
+E30 = −1 × the slot-6 value.

1. ratio = 1.5, or 1.0 when properties +50C (vehicle +1384, CSV column 111) is above 50.
   37 Standard cars have 11..15 and 7 have 100.
2. scaled = s × (1/ratio).
3. cur = `9AE5A0`(car) / +E54. `9AE5A0` returns the heading of the front-axle centre velocity:
   atan(lateral/forward), with the axle point from wheel bases +1C30/+1D80 and ω×r + v. It is
   blended with the current wheel angle +E54·+E30 by t = clamp((|speed| − 2)/2, 0, 1), in x87.
4. a = cur + s.
5. k = 1 + 0.5·resp when resp ≥ 0, otherwise 1/((1 − resp)·0.5). resp is record +14: 0.4 from the
   constructor, or (wheelLinearity − 0.5)·2 when that option exists.
6. If a ≥ 0: r = pow(a, k) − cur, clamped to [−1, scaled]. Otherwise:
   r = −pow(−a, k) − cur, clamped to [scaled, 1].
7. The slot receives r. Under k = 1 this reduces to `scaled`.

The wheel path also extrapolates the raw axis by its slope over the last samples (ring at +44,
global history `E4E190..`, ×1/3·dt and ×1/7·0.05). That compensates for real-time sampling and is
**not ported**: the prototype has no analog wheel input [D].

### 2.6 Brake mode record slot (+197C)

`828670` copies record slot 10 (always 0 for a human) to +197C. The brake stage `9B2EC0`
overwrites +197C from +11BC every control step, so the slot has no effect [D].

### 2.7 Per-car systems that look like assists but are not options

- **ABS mode** +11BC is properties +344 (CSV column 276). Lacetti is 0; 42 cars are 0 and 2 are 2.
  `9B2EC0` limits brake force to tyre grip in mode 2, or in mode 1 while the accumulator is below
  1.25 s [D]. It is per car and no option writes it [D].
- **Traction mode** +1988 (`9B1xxx` traction routine) is only ever cleared: by the constructor, by
  reset `996B70`, and by an uncalled setter `98F240` [D]. It is dead in the shipped game [I]. The
  port keeps it at 0.
- **Rollover recovery** +1F2A is properties +4F2 (CSV) [D].

## 3. Runtime effect of each aid

All read sites were found by displacement search, aligned and compared with the port. Every one
is already implemented in `grid-world` with the same constants [D]. The aids act as *penalties
when off*.

| Aid off | Original site | Effect | Port |
|---|---|---|---|
| Traction (+155A) | engine `9B1B90` @9B1F0D | Throttle blend ×1.1111 (0x3F8E38E4), clamped to 0..1 | `vehicle_propulsion.rs` |
| Traction (+155A) | grip `A00E10` @A013B0 | Driven wheel (+10C > 0) with throttle +185C > 0.9: grip × (1 − f). f = (1 − clamp(t)) · +1164 · (thr − 0.9)·10. t = (6 − v)/6 for v ≤ 3, else (v − 6)/+1160 | `vehicle_grip.rs` |
| Stability (+1558) | engine @9B1FC9 | Engine torque × +1178 | `vehicle_propulsion.rs` |
| Stability (+1558) | grip @A0134B | Grip × (1 − +1170) on the front wheels and × (1 − +1174) on the rear | `vehicle_grip.rs` |
| Braking (+1559) | brakes `9B2EC0` @9B2F60 | Brake command ×1.1111, clamped to 0..1 | `vehicle_controls.rs` |
| Braking (+1559) | post-force `9BF230` | Timer +E74 accumulates while brake > 0.9 and throttle < 0.1, else resets | `vehicle_post_force.rs` |
| Braking (+1559) | grip (via +E74) | When +E74 > +1168: grip × (1 − +116C) and wheel spin +104 = 0 (lock-up) | `vehicle_grip.rs` |

The per-car parameters are CSV columns 323..329 → properties +2E8..+300 → vehicle +1160..+1178.
Their defaults apply when the cell is empty: 60, 0.6, 0.3, 0.4, 0.2, 0 and 1. The Lacetti has
60, 0.4, 0.3, 0.4, 0.05, 0 and 1 [D, read by the port's property reader].

Gearbox modes 4 and 2 differ as follows [D, port code verified in F172/F179/F185]:

- In mode 4, `9B29A0` selects gears from speed, slip and RPM. +1972 swaps throttle and brake in
  reverse and selects reverse with the brake near rest.
- In mode 2, Gear Up/Down or the direct request moves through 1..top, N(0) and R(9). Down from N
  gives R and up from R gives N.
- In mode 2 the post-force stage (`1974 < .25`, +1970 = 0) raises low RPM to idle·1.01 just after
  a shift.
- Clutch assistance (+1971) is set in both modes. It engages and disengages automatically
  (`98E1B0`, idle byte +1640).

### Interactions

- The aids are independent bytes, but four interactions follow from the code above [D/I]:
  - Traction off and braking off both scale pedals by 1/0.9. A keyboard pedal is 0 or 1, so the
    scale only matters through the clamp at 1.
  - Stability off reduces front grip. With Traction off, a front-driven car (Lacetti) also loses
    grip at full throttle on the driven front wheels, so the two compound in a powered turn.
  - Braking off only locks wheels under a sustained brake > 0.9 with throttle < 0.1, for longer
    than +1168 (0.3 s).
  - Brake-to-reverse exists only in Automatic. In Manual, reverse needs Gear Down from N.
- Wheel input changes only steering: it selects unassisted steering and the shaped command.
  Pedals and gears are unchanged.

### Open discrepancy (not changed)

The original writes record slot 7 (clutch, always 0 for a human) to +E34 every frame. The port
keeps +E34 from the previous control step. The preparation stage reads +E34 before the controls
run, so the two can differ after a rev-match or a timer set it to 1 [D]. Whether the original runs
several physics substeps per input write is not established [I]. Changing it would alter the
default configuration, so it is recorded here and left as a follow-up.

## 4. Port

- `grid_tools::source_vehicle::assists` holds:
  - `Assists` (traction_control, stability_control, braking_assist, transmission, input,
    wheel_response) with `Default` = every aid on, Automatic and Keyboard;
  - `apply` (the bytes of §2.1–2.4), `parse`/`Display` (`tcs=off,stability=off,braking=on,gearbox=manual,input=wheel`,
    `all=`, `preset=default|none`, `linearity=`/`response=`), the `gearbox_bytes` table, and
    `front_heading`/`wheel_command` (§2.5);
  - `Runtime::set_assists` (before the first tick; reset keeps it), `Runtime::request_shift(Shift)`
    (one tick), `Runtime::gear()` and `MultiRuntime::set_assists(car, …)`.
- `Runtime` now writes +E24/+E28/+E2C every tick, as the original record does: 0 unless a shift
  is pending. The wheel command is applied before staging.
- `vehicle_driver_prepare` accepted only the automatic/in-gear path. It now accepts +1970 = 0 and
  gear 0, which `A03670` does not read differently, and follows the original neutral/idle
  mode-two transition (`A03670` lines 134–155). The manual-clutch branches (+1971 = 0) stay
  rejected.
- The app takes `--assists SPEC` / `--assists-file PATH` and prints `drive_assists …`. In Manual,
  X shifts up and Z shifts down. The original binds A/Z, but A is the prototype's steer-left
  alias. The launchers pass `GRID_PLAY_ASSISTS` or `local\assists.txt`.

The front-end Driver Aids screen, the settings it saves and the port's graded (25/50 %) Traction
Control and Braking Assist: [DRIVER_AIDS.md](DRIVER_AIDS.md).

## 5. Test matrix and verification

The matrix is every combination of the five settings: TC × SC × BA × {Automatic, Manual} ×
{Keyboard, Wheel} = 32 configurations. Each configuration runs four scripted manoeuvres from
Donington slot_00 with `grid-assists-drive ROOT local/driving-assists/traces` (700 ticks each,
per-tick TSV, `summary.tsv`):

| Manoeuvre | Script | Verifies |
|---|---|---|
| launch | Full throttle 11.2 s | TC: front-wheel slip and grip penalty; gear changes (auto) or the up-shift policy (manual) |
| brake | Throttle 6 s, then full brake | BA: +E74 timer and locked wheels, stopping distance; brake-to-reverse only in Automatic |
| turn | Throttle 3 s, then 70 % throttle with half lock | SC: lower front grip means lower yaw rate. Wheel: shaped +E30 differs from the input |
| shifts | Full throttle 8 s, then 0.3 brake | Gear sequence: automatic up-shifts, or manual X/Z requests incl. down-shifts |

Manual policy (authored test driver, not original): shift up above 0.92 of the limiter (+17A4),
shift down while braking below 0.45, at most every 0.5 s.

Checks:
- [V] Default bit-identity: solo AI Donington 2 laps 88.42029812 / 86.28228502 s, 0 contacts
  (`runs/solo-default.txt`), equal to F301/F304. The asset test
  `source_driving_assists_matrix` asserts that `set_assists(default)` leaves 300 ticks of vehicle,
  physical and world state identical.
- [V] Unit tests: parse/display/bytes, the `98F110` table, wheel shaping bounds/symmetry and the
  1/1.5 ratio.
- [V] Asset test `source_driving_assists_matrix`:
  - each aid off changes the drive in the expected direction (TC: higher launch slip; BA: +E74
    exceeds +1168 and wheels lock; SC: lower yaw rate in the turn);
  - Manual: X/Z requests change gears, down from 1 gives N then R, and an automatic up-shift never
    happens;
  - Wheel: unassisted steering and a shaped command.
- Summary results: §6.

## 6. Results

The 128 runs complete with no tick failure. Evidence is in `traces/summary.tsv` (SHA-256
`1ed12109…c7f3`), one TSV per run, and `runs/matrix.log` (22 s wall).

Observations [V for the port; not compared with the original]:

- **Independence.** Braking Assist changes nothing in launch or turn: those rows are
  bit-identical across B. Traction and Stability Control change nothing before a throttle or
  steering input reaches them.
- **Traction Control (launch, 700 ticks of full throttle).**
  - Peak front-wheel slip ratio: TC on 1.418, TC off 2.837.
  - With SC also off: 1.631 and 3.044.
  - Distance in Automatic/Keyboard: 453.7 m with TC on, 425.6 m with TC off.
  - The run leaves the start straight before 11 s, so its end values include the first corner.
- **Braking Assist (brake from about 49 m/s).**
  - BA on: +E74 stays 0 and no wheel locks.
  - BA off: +E74 peaks at 1.98..2.16 s in Automatic and 5.2 s in Manual, with 10..20 ticks of
    locked wheels.
  - Time to rest is 131..143 ticks with BA off and 187..210 with BA on. Locked wheels stop this
    port's Lacetti sooner. The live comparison (F371, BRAKING_REFERENCE.md) confirms this for the
    original: BA off 53.3 m, BA on 69.7 m from about 49 m/s; the port is 9 % longer with BA on.
  - Automatic holds the brake past rest, selects reverse (gear 9) and backs off to about 16 m/s.
    That is the brake-to-reverse of mode 4. Manual stays in gear 1 at rest.
- **Stability Control (turn, 70 % throttle, half lock, Automatic/Keyboard).**
  - Mean yaw rate over the last 2 s: SC on 0.464 rad/s, SC off 0.445.
  - With TC off and the other configurations, the ordering changes with gear choice: TC off/SC on
    0.356 against TC off/SC off 0.464. The turn is not a steady state.
- **Gearbox.**
  - Automatic launch: 1>2>3>4>5>4.
  - Manual with the authored policy: 1>2>3>4>5>6, down-shifts to 1 when braking, no automatic
    shift.
  - The asset test checks X/Z, a direct gear 4, Down from 1 to N, from N to R, and backwards drive
    in R.
- **Wheel input.** The command is shaped: straight at speed, an input of 0.5 gives +E30 0.3333,
  the 1/1.5 ratio. Turn yaw is lower than with keyboard (0.323 against 0.464 in
  Automatic/all-on), because unassisted steering also applies.

The asset test `source_driving_assists_matrix` also confirms that each aid acts in the expected
direction (TC off: launch slip 2.84 vs 1.42; BA off: lock-up timer 2.11 s, 17 locked ticks; SC off: yaw 0.374 vs 0.377 rad/s; wheel: 0.5 -> 0.3333).
