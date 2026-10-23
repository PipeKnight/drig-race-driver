# In-race camera views (F349)

The playable app now shows the original's in-race views of the player's car, read from the car's shipped `cars/<stem>/cameras.xml`. The cameras are presentation only and do not feed the simulation.

Code:
- `grid_formats::vehicle_views` reads the file.
- `crates/grid-app/src/camera.rs` holds the rigs, the cycle, look-back and persistence.
- `drive_present` in `main.rs` calls it once per frame.

Background and the longer cockpit roadmap: [CAMERA_COCKPIT_PLAN.md](CAMERA_COCKPIT_PLAN.md).

Evidence tags: [D] decompiled (read-only Ghidra 12.1.4 headless `-readOnly -noanalysis` on `local/phase3/ghidra-project`, hash-pinned `grid-unpacked.exe` SHA-256 `aac46a64…4514`); [V] shipped data or a test; [I] interpretation; AUTHORED = port choice. Private evidence lives in the main checkout's `local/cameras/`:
- `lac-cameras.txt` and `bmw-cameras.txt`: decoded trees;
- `chase-vt/`, `chase-2/`, `chase-3/`: Chase class;
- `mounted-head/`: Mounted and Head;
- `mgr-1..7/`: view manager and profile.

Decoder: `grid-binxml` plus the new `tools/binxml_tree.py`.

## 1. Data [V]

`cameras.xml` is sectioned binary XML. It holds `VehicleViewDefinition / ViewManager name=<stem>` with 20 `View` elements. Each view has:
- `type`: Chase, Mounted or Head;
- `ident`;
- an optional `reverseCamera`;
- `hidden`;
- `transitionType`, `transitionSpeed` and `transitionStrength`;
- `Parameter name type value|x y z` children.

The Lacetti and the BMW have the same structure; only positions, FOVs and pitches differ.

Visible views, in file order, with their reverse views:

| ident | type | key data (Lacetti / BMW) | reverse |
|---|---|---|---|
| chase_close | Chase | position (0, 1.7227, −4.7393) / (0, 1.6845, −4.6399); target (0, 1.2552, 1.9141) / (0, 1.2590, 1.9001); fov 52; lag 2.7507..4.5133 | chase_close_reverse (position z +5.116 / +5.162) |
| chase_far | Chase | position (0, 1.8637, −5.1838) / (0, 1.8717, −5.3160); target (0, 1.5535, 2.5941) / (0, 1.5326, 2.5941); fov 55; lag 2..5; roll enabled | chase_far_reverse |
| bumper | Mounted | offset (0, 0.7275, 1.7706), pitch 0.0996, fov 60, near 0.2; all movement 0 (rigid) | bumper_reverse (yaw π, z −1.77) |
| bonnet | Mounted | offset (0, 1.1055, 0.8556) / (0, 1.163, 0.729), pitch 0.0734, fov 60, near 0.1; axisScale (0, 0.08, 0.08), maximumShift 0.0999, movementSpeed 1.7264 | bonnet_reverse (yaw π, pitch 0.2, offset (0, 1.24, −1.437) / (0, 1.309, −1.29), fov 71.84 / 60) |
| head-cam | Head | position (0.3604, 1.1047, −0.1923) / (0.4310, 1.0713, −0.2152), tilt −7.3736 / −10, fov 48, near 0.075, highResInterior true | head-cam_reverse (Mounted, the same as bonnet_reverse) |

The other hidden views are drift_cam_rear and the replay views: wing1/2, rear1/2, roof1/2, driver1/2 and terminal. All in-race views use `transitionType=switch`, an instant cut.

## 2. Original behaviour found

### Parameter binding [D]

Each class has a binder that maps parameter names to object offsets:
- Chase: vtable 0xD00278, binder 0x7F31B0;
- Mounted: vtable 0xD00008, binder 0x7F4BE0;
- Head: vtable 0xD00140, binder 0x7F3A70.

The per-frame update is vtable slot 11:
- Chase: 0x8435D0 → 0x81EC90 → 0x8114B0;
- Mounted: 0x842D10 → 0x822510;
- Head: 0x842DD0.

The output stage stores `fov × 0.0174533` (degrees to radians) in the view record, together with the near plane.

### Profile key and persistence [D]

0x5F1100 registers the profile string `current_camera` with default `chase_close`.
- At the start of a race, 0x582750 looks the name up (0x823360). A hidden or unknown view falls back to the first visible view.
- At the end of a race, 0x584E70 saves the current view's ident. If a hidden view is showing (look-back), it saves the visible view whose `reverseCamera` is that view.
- 0x4CC590 resets the key from a preset: 0/1 chase_close, 2 bumper, 3 head-cam. Its caller was not traced.

### Chase pipeline [D, F461]

Traced statically (2026-10-19) on the hash-pinned image: Ghidra 12.1.4 pseudocode (read-only project copy) checked against Capstone 5.0.6 disassembly wherever the SSE pseudocode mislabels stack slots; constants read from the image. Evidence: worktree camera-dynamics-11a55d `local/cam/` (`decomp/d1..d3`, `readva.py`, `dis.py`, `scan.py`, `vtables.py`).

Chase binder 0x7F31B0 (parameter → object offset): lagMinimum +0x110, lagMaximum +0x114, fov +0x118, rollAngle +0x138, minimum/maximumTiltSpeed +0x120/+0x124, minimum/maximumRollSpeed +0x128/+0x12C, tiltFallback +0x130, tiltAngle +0x134, maxRollPerSecond +0x13C, invertRoll +0x140, cameraTiltEnabled +0x141, cameraRollEnabled +0x142, position +0x150, target +0x160, suspScale +0x19C, suspMixScale +0x194, suspDeadZone +0x198, gearUpDropBack/OverallTime/InitialTime +0x178/+0x17C/+0x180, kCameraCloseFollow* +0x184/+0x188/+0x18C, shake* +0x244..+0x25C, angularLimit(Damping) +0x260/+0x264, fixedRoll +0x268. Constructor 0x81E900 (vtable 0xD00278) also sets +0x170 = 0 and +0x174 = 5.0, which no parameter binds.

The camera target of a car has vtable 0xCFF640 (target +8 = the car): +8 speed = car +0x1648; +0xC world velocity; +0x10 world angular velocity; +0x14 position; +0x18 pose; +0x1C rotation; +0x24 wheel i (0x7F7330): returns wheel +0x40 (suspension displacement) and writes wheel +0x4C (travel clip); +0x28 (0x7F7380): car +0xD234 ≠ 7, i.e. controls disabled.

Per frame, 0x8435D0 calls 0x81EC90, which does, in order:

1. **Suspension heave** [D]. For each wheel, x = displacement, r = travel, margin = suspDeadZone · r. If margin < |x|, the sum gets x − margin/(1 − suspDeadZone) (x + that for x ≤ 0). Then heave = sum · 0.25 · m + (1 − m) · heave_prev with m = pow(suspMixScale, dt · 40) (0xCB504C = 40, 0xCB01AC = 0.25; pow 0xBF53E5 in double). 0x8005D0 raises the **target** by suspScale · heave (3 × in all chase views). Its drift rotation of the target only runs when 0xE4DD54 = 0; the byte is 1 in the image.
2. **0x8114B0**:
   - Yaw lag as below, then the **drift blend** [D]: when the weight w > 0, the lag used for the frame becomes lag + (−atan2(v·X, v·Z) − lag) · t · w with t = clamp((|v| − 5) · 0.1, 0, 1) (0xCB093C, 0xCAFCD4). w is read at `[[[chase+0x64]+0xC]+0x24]` + 0xE78 + 0x168 [D, F510]: the target setter 0x7F6F80(X) stores target +0xC = X (the race participant) and target +8 = [X+0x24], so +0x24 is **the car** (the same pointer whose +0x1648 is the speed); 0x996ED0 is `lea eax, [ecx+0xE78]`, the car's copy of the vehicle-properties record (0x65C bytes, copied by the car constructor at 0xA0F28A). So w = car +0xFE0 = record +0x168, written only by the CSV decoder 0x9AEC60 (0x9B0EC8: strtod of CSV column 330). Column 330 is empty in all 45 shipped `cars/*/*.csv`, so w = 0 for every car in every race type (it is a per-car property, not a race mode or drift-event flag). The port reads it from car +0xFE0 (`Sample::drift_weight`; the port decodes column 330 into the same record). Evidence: worktree camera-dynamics `local/cam/drift/findings.md`.
   - **Roll** 0x800100 [D]: goal = ω_y · (1/π) · rollAngle · 1.4 (0xCE9608, 0xE4DD98); the state (record +0x9C) moves toward it by at most maxRollPerSecond · 0.5 · dt (0xE4DD90), otherwise takes it. Weight (record +0xA0) = clamp(speed · 1.25, min, max) mapped 0..1 over minimum..maximumRollSpeed (0xE4DD94; 1 when min ≥ max). The yaw-rate lane was checked in the disassembly (lane 1 of target +0x10).
   - **Car frame** 0x7FF3E0 [D]: the car pose with its forward F replaced by e · D + (1 − e) · F, or, when F · D < 0.97 (0xCB0F30), by F turned toward D by e · acos(F·D) (0xE4E164 = 1), and the frame rebuilt about the car's up (X = up × F', Y = F' × X). D is the held forward and e the spin envelope (step 3). A blended forward not longer than 0.1 (0xCF1208, also NaN from an unset D) keeps the car pose.
   - **Camera local** 0x8007B0 [D]: eye = position − (0, 0, fall-back) (0x800320), looking at target + heave. **Fall-back** = tiltFallback · ramp(speed, minimumTiltSpeed, maximumTiltSpeed) − min(gearUpDropBack · tiltFallback · g, tiltFallback · ramp) · s, where g is a per-gear weight (gears 1/R 1.0, 2 1.15, 3 1.452, 4 1.824, 5 2.1, else 2.412; 0xCFF578..0xCFF584, 0xCE8FD4) and s the drop-back envelope over car +0x2E4 (time since the shift): rising 0.5 − cos(t/initial · π)/2 (keeping its larger previous value) until gearUpDropBackInitialTime, falling (cos((t − initial)/(overall − initial) · π) + 1)/2 until gearUpDropBackOverallTime, then 0. In the shipped views tiltFallback is 1 (chase_close, 0..60) and 2 (chase_far, 0..80); 0 in the reverse views. gearUpDropBack is 0 in every in-race view, so the gear term is exactly 0 there. tiltAngle and cameraTiltEnabled are not read in this path.
   - Eye: the local eye turned by the lag about the car's Y and carried by the car frame. Its **height** is replaced by car y + eye_local.y − kCameraCloseFollowYScale · follow, where follow moves by kCameraCloseFollowForwardsRollDamp toward the car frame's forward slope (negated when the eye is in front, z > 0), clamped to ±kCameraCloseFollowYClamp; the first frame after a reset sets it directly (record +0xAC). Until the speed first reaches 5 m/s (0x8002D0 with +0x170/+0x174; latched in record +0xE0) the height is blended with the previous one by speed/5.
   - The orientation 0x8114B0 builds, with a roll gated by cameraRollEnabled, is overwritten by step 4; that roll has no effect.
3. **Spin hold** (impact envelope 1) [D]: ω' = ω with half of its components about the car's X and Z removed. A change of |ω'| above 18 rad/s² per second (0xE4DD58) arms 3 frames; three consecutive frames with |ω'| > 2.5 (0xE4DD5C) within them start a 2 s envelope (0xE4DD60) and store the frame's forward as D. The envelope rises √(1 − (t/2 − 0.9)/0.1) for 0.2 s (exponent 0xE4DD6C), stays 1 until t = 1.2 s (0xE4DD64/0xE4DD68), restarting at 1.8 s while the spin goes on and held while the controls are disabled; when the spin stops (three quiet frames) it jumps to 1.2 s and eases to 0 quadratically (exponent 0xE4DD70). The earlier note that this detector used |v| was wrong: it reads target +0x10.
   The second detector (|v| change above 50, 1 s, 0.4/0.8, exponent 0.5) scales chase +0x1D0 into the eye. +0x1D0 is only zeroed by the constructor (0x81EB1C) and copied between the object and the record (0x820F1D, 0x831E57), so it is always 0: no effect, not ported.
4. **0x810E50** [D]: the view looks from the eye at the target point on the car (car pose · (target + heave), 0x8130D0; not in the lagged frame) and rolls about its direction by roll · weight degrees (positive with invertRoll), independent of cameraRollEnabled. Up = cos a · n(d × h) + sin a · h with h = n((0,1,0) × d) (0x7FE8B0).

0x8435D0 then applies fixedRoll (0 in all views), stores fov · 0.0174533, and calls the shake 0x810490 (shakeScaleX/Y 0 in the chase views; not ported). Because 0xE4DD54 = 1 it finally calls 0x7FEC40, which turns the eye and orientation about Y by atan2(record +0xA4, record +0xA8 + 0.1): the chase look turn (§ Look input, F499).

### Live comparison with the original (F470) [V]

19 October 2026, owner's go ("запускай"). The research copy (SHA-256 b1da1b2d…aece34a6), PID 33556. Race Day, Touring Cars, Lacetti, Donington GP. Three runs (A, B, C) of the bounded macro `camera_run`: from rest, throttle 8 s, full brake 2 s into Redgate, throttle and right 2.5 s over the exit kerb, throttle 1.5 s. Run A slid into the gravel after a touch with an AI car; run B ended against the barrier; run C slid wide.
- Memory was only read, with the new `tools/capture_grid_camera.py`:
  - `scan` finds the 20 view records of the player's camera manager: the camera objects with target vtable 0xCFF640, then the manager entries that hold them, record at entry +0x10. FOV is checked against the object.
  - `capture` writes a row each time the car's physical block changes: world +0x7C4, car +0x1640, +0xD230, the wheels, physical +0x1D0, car +0x10..+0x4F (from CM02, run C), and per view the record +0x00..+0x14F and object +0x190/+0x220.
  - Every view record updates every frame; the one on screen is just the one drawn.
- The user files (all 19) match their snapshot hashes after the session; none changed. The game was closed with WM_CLOSE. Evidence: worktree camera-dynamics-11a55d `local/cam/live/` (captures, decodes, `compare*.py`, `states.py`, `tilt.py`, shots, sheets, macro logs, session).

The port's camera code was then driven with the original's per-frame car state: the ignored test `camera::tests::replay_live_trace`, run with GRID_CAMERA_TRACE, GRID_CAMERA_OUT and GRID_GAME_ROOT. The output was compared with the original's records frame by frame (`compare2.py`). Rows where the record lags the pose by a frame are skipped (sync check on the rigid bumper).

Findings:
1. **Mount frame** [D]. The camera target answers the car's model frame, not the physics pose. 0x9AE3A0 takes the physics pose (body +0x1D0), turns it by the car's 3×3 at +0x10 (rows), and moves the origin by that rotation times car +0x1630. For the Lacetti that is (0, −0.39144, −0.14642): the model origin lies 0.39 m below and 0.15 m behind the body origin. The port mounted its views on the physics pose, 0.42 m off; fixed in `camera::model_frame`, used by `Sample::of`.
    - Car +0x10 is dynamic. It held up to 0.2° of roll mid-run (run C); without it the error is up to ~0.5° of pitch under acceleration and ~1° of roll in the slide (runs A/B). **Correction (F472):** the port does compute it. The writer is the chassis-motion model 0x9BE1E0 → 0x9BDF30 (acceleration lean, wheel roll/pitch, speed-scaled noise tables, bottoming limit), ported byte-exact in F177 as `grid_world::vehicle_chassis` (docs/VEHICLE_CHASSIS.md), so `Sample::of` gets the live matrix. 0x9AE3A0 also adds its translation row car +0x40 (unturned) to the origin; `model_frame` takes it since F472 (0 in all three runs: no bottoming). The app's renderer draws the body at the physics pose without this matrix, so the drawn car lacks the original's body sway (graphics, not changed here).
2. **Camera time.** The original's camera integrates with the rendered-frame time. Here that is the gap between physics changes, about 5.9 ms median, not the world step +0x7C8 (6.06 ms median in run C, about 4 % more in total). With +7C8 the bonnet tilt state reached only 77–82 % of the original's; with the frame time it matches to 4 digits (`tilt.py`). The port feeds its own simulation time, consistent with its own Δv.
3. **Agreement with the frame-time dt and car +0x10** (run C, synced rows, 2-s bins):

   | view | eye | direction | roll |
   |---|---|---|---|
   | chase_close | 0–2.5 cm (4 cm at the end of the slide) | ≤ 0.1° | 0.00° |
   | chase_far | 0–2.3 cm (4 cm at the end of the slide) | ≤ 0.08° | 0.00° |
   | bumper | 0 | 0.00° | 0.00° |
   | bonnet | ≤ 0.6 cm | ≤ 0.07° | ≤ 0.01° |
   | head-cam | 1 cm lower | ≤ 0.12° | ≤ 0.3° in the slide |

   The internal states match too: chase lag (e.g. −0.367 against −0.369 in the slide), bonnet tilt state and shift to 3–4 digits, the eye-height hold below 5 m/s at the start. The head-cam differences were its own dynamics, untraced then and ported in F472 (§ Head).
4. **Not exercised.** The spin hold did not start in either game; the slides were not violent enough (blend 0 in all three runs). The chase roll stayed below 0.05° in both.
5. **Run conditions.** The frame rate was unlocked (world +0x7C8 about 6 ms, ~170 Hz). The chase lag and follow are frame-rate sensitive in the original; the port uses its 1/60-s normalisation for the follow damp (§3).

### Chase yaw lag [D, 0x8114B0]

Per frame:
- The lag angle integrates the car's yaw rate: `θ += dt · ω_y` (target +0x10, lane checked).
- It is limited by `angularLimit` / `angularLimitDamping` (`θ = ±L·d + θ·(1−d)` beyond ±L). Then it is wrapped to ±π.
- It returns toward zero at `|θ| · k`, with `k = lagMinimum + (lagMaximum − lagMinimum) · speed / 55.5554`. The constant 0xE4E0FC is 55.5554 m/s, i.e. 200 km/h. The speed is car +0x1648 (target +8), signed, not clamped.
- If the return speed exceeds |ω| and θ crosses zero, θ becomes 0.

`angularLimit` is 10 and is compared with radians, so after the ±π wrap it never binds. The term under `DAT_00E4DD54 == 0` does not run (the byte is 1).

### Mounted movement [D, 0x822510]

This runs only when `movementSpeed ≠ 0`:
- `a = Rᵀ (v − v_prev) / (dt · 30)`; the constant 0xCAFDB8 is 30.
- The shift goal is `clamp(−a · axisScale, ±maximumShift)`.
- Each frame: `shift += movementSpeed · (goal − shift) · dt`.
- The eye is `offset + shift · finalMovementScale` in the body frame, or offset plus the position when `offsetOnly` is set.

With offsetOnly set the eye is the bare offset plus the car position (no shift); no shipped view sets it.

Mounted binder 0x7F4BE0: yaw +0x118, pitch +0x11C, roll +0x120, finalMovementScale +0x124, tiltDistance +0x128, movementSpeed +0x12C, maximumShift +0x130, shake* +0x134..+0x14C, axisScale +0x150, axisScaleForTilt +0x160, twistScale +0x170, offset +0x180, fov +0x110, nearPlane +0x114, highResInterior +0x190, offsetOnly +0x194.

**Tilt state** [D, F461]: a second state (record +0x70) follows clamp(−a · axisScaleForTilt, ±maximumShift) at movementSpeed · 2 (0xCB8DA4) per second.

**Orientation 0x803270** [D, emulated F461]. Rows (right, up, forward), row vectors: M = R(roll) · P(pitch) · Y(yaw + record +0xB0), with Y(ψ) = [[c,0,−s],[0,1,0],[s,0,c]], P(θ) = [[1,0,0],[0,c,s],[0,−s,c]], R(φ) = [[c,s,0],[−s,c,0],[0,0,1]]. Positive pitch looks down; positive yaw turns toward local +X. Without twist (twistScale all 0) the view is M · Car. With twist (the bonnet: (−0.1423, 0, 0.4107), tiltDistance 1): T = the tilt state, whose x is replaced (in a local copy) by c · (1 − s) + T.x · s with s = clamp(0.05 · |v|, 0, 1) (0xCAFE60) and c = clamp(−ω_y · 0.2 · 0.5, ±1) (0xCB870C, 0xCAF88C; the target's +0x2C is always 1); L = (T.x · tw.x, T.y · tw.y + tiltDistance, T.z · tw.z), U = L/|L| (or Y), W = [U × Z; U; (U × Z) × U] (not normalised), view = M · W · Car. The up axis thus leans with the filtered acceleration (forward pitch under braking, side lean from the lateral term), and at a standstill with the yaw rate. This was confirmed by running 0x803270 alone in Unicorn 2.1.4 with synthetic inputs (max error 2.2e-7; scripts in `local/cam/agent-emu/`); Ghidra's pseudocode of it mislabels stack slots.

**Bonnet turn** [D, E]: record +0xB0 is the look turn of the bumper and bonnet (§ Look input, F499).

The bumper has all movement parameters zero, so it is rigid in the original too.

**Shake 0x815960** [D, E; V on the bonnet, F508]. The last call of 0x822510 (0x822EDF), after orientation, eye and FOV, for every Mounted view each frame (worktree camera-dynamics `local/cam/shake/`: `shake-findings.md`, `shake_emu.py`, `validate.py`):
- Gate: nothing if shakeAngle, shakeScaleX and shakeScaleY are all 0 (the bumper, the reverses).
- s = car +0x1648 (signed, m/s) clamped to shakeMinSpeed..shakeMaxSpeed; record +0x94 = shakeDamping; the amplitude +0x98 = max(+0x98, (s − min)/(max − min)); the shake time +0x90 += dt; +0x98 −= shakeDamping · +0x98 · dt, zeroed below 1e-4 (0xCFDD14).
- With M the record's rows (right, up, forward): if shakeAngle ≠ 0 the rows are pitched by shakeAngle · 0.0174533 · n(0, shakeRotationSpeed) about the view's right axis (positive looks down); if a scale ≠ 0 the eye moves by right · n(0, 1) · shakeScaleX + up · n(0.1, 1) · shakeScaleY.
- Noise n(a, b) (0x7FE510): u = 300 · |(t + a) · b| (0xCAFBE0); the 6000-entry table at 0xE7B230 linearly interpolated at u (wrapping: a 20 s period, with a jump of 0.344 at the wrap); n = clamp(amplitude · (2v − 1), ±1). The table is filled once by the static initializer 0x7F2B40: T[i] = 0.25 · [(½ sin((x + 0.12) · 42.56) + ½) + (½ cos(7.81 x) + ½) + (½ sin(2.53 x) + ½) + (½ cos(x − 1.54) + ½)], x = i · 0.0033333334, with the f32 constants in x87 precision. **No random source**: the shake is a function of the shake time and amplitude.
- The view's reset (slot 6, 0x82C3C0) and activation (slot 10, 0x82C460) zero the shake time and amplitude.
- Shipped: the bonnet of both cars has angle 0, damping 0.1798, min 20.8641, max 90, scaleX 0.0001, scaleY 0.0024, rotationSpeed 0.0022 (cameras.xml stores the values as 4-decimal text). So the bonnet eye bobs vertically by up to 2.4 mm · amplitude (1.2 mm at 55 m/s), sideways by up to 0.1 mm.
- Live [V] (run C, bonnet record): +0x94 = 0.1798 in every row; the amplitude follows the model (median error 3e-8 while rising); above 40 m/s the bonnet's vertical eye residual against the unshaken port drops from 0.351 to 0.051 mm rms once the modelled shake is subtracted (correlation +0.989, 648 rows). The 4.7 mm rms forward residual is not shake: it comes from one stale bonnet record (t = 4.12 s, its record time +0x90 not advanced with the physics row: one frame of travel, 0.184 m) and 29 start-up rows at 1–2.4 m/s within compare2's 3 cm bumper sync; without them the forward rms is 0.257 mm. The original's shift (0x822510) equals the port's: one step with the record's dt and the physics rotation reproduces record +0x80 to 1.9e-6 m [V] (`local/cam/drift/bonnet_shift.py`). Comparisons should sync on the view's record time, not on the bumper eye.
- The chase's 0x810490 is the same algorithm on its own words (object +0x244..+0x25C, record +0x60..+0x68) but without the all-zero gate and always with its angle path: chase_close's shakeAngle 0.1 with shakeRotationSpeed 0 gives a constant pitch of amplitude · (2 · T[0] − 1) · 0.1° ≤ 0.0027°. Not ported (negligible).

### Head [D, emulated, live-checked, F472]

Vtable 0xD00140; slot 11 0x842DD0 jumps to the update 0x820D10, slot 6 0x843E30 to the reset 0x82C0E0. Tags: [D] read in the disassembly, [E] reproduced by running the original function in Unicorn 2.1.4 frame by frame, [V] matches run C of the live comparison. Evidence: worktree camera-dynamics `local/cam/head/` (`head-findings.md`, `head_emu.py`, `head_model.py`, `results.txt`).

**Binder 0x7F3A70** over the constructor defaults (0x820A40) [D]; shipped Lacetti in brackets: fov +0x210 (48), nearPlane +0x214 (0.075), highResInterior +0x29C, bodyLength +0x218 (default 0.01; 0.01), maxBodyOffset +0x21C (0.1; 0.07), maxBodyOffsetScaleUp +0x220 (0; 1.5), maxBodyOffsetScaleSide +0x224 (1; 0.9), bodyInertia +0x228 (0.1; 0.1), bodyDamping +0x22C (0.7; 1.2), bodyCenteringForce +0x230 (0.6; 2.0), maxHeadOffset +0x234 (0.1; 0.179), headInertia +0x238 (0.2; 0.3399), headDamping +0x23C (0.7; 5.4045), headCenteringForce +0x240 (0.6; 3.8697), headBuffeting +0x244 (false), `bump_interval ` (trailing space) +0x248, bump_likelihood +0x24C, bump_nonbumplevel +0x250, position +0x260, tilt +0x270 (−7.3736), maxMovement +0x274, windSpeed/Tilt/Freq +0x278..+0x280, maxHeadLook +0x28C (40), autoLookAngleInterest +0x290 (0), velocityChangeCrashDetection +0x298 (5).

**State.** The reset copies the parameters into two springs: body (object +0x110: scale (ScaleSide, ScaleUp), allow-y byte 1, damping, inertia, centering, max) and head (object +0x190: scale (1, 0) from the constructor, allow-y 1, head parameters). Their states live in the record: body +0x110 last input velocity (world), +0x120 filtered acceleration (world), +0x130 offset (car frame), +0x140 offset velocity (car frame); head the same at +0x170..+0x1A0. The reset sets record +0xA0 = the target velocity v, both springs' last input = v, acceleration and offset = 0; it does not clear the offset velocity [D].

**Update 0x820D10**, per rendered frame with the frame time dt [D, E]. Row vectors; R = the model frame's rows, o its origin (§ Live comparison), ω the world angular velocity:
1. record time +0xF0 += dt; TrackIR poll (identity and zero without a tracker); transition source bookkeeping (object +0xD0 → +0x2A0).
2. Seat velocity `vin = v + ω × (position · R)` [V: equals record +0x110 to 1e-6].
3. Body spring S(body, vin); then head spring S(head, vin + body.vel · R).
4. `eye = (position + (0, bodyLength, 0) + body.off) · R + o` [V: 3e-6 m p50].
5. Head turn: a = (h.z, 0, −h.x) for the head offset h; if |a|² > 1e-9 (0xCFF550) the rows are turned by Rodrigues about â by |h| radians (a forward offset looks down, a right offset rolls the right axis down [E]).
6. Look and tilt (0x814BC0 → 0x801D10): `rows ← Y(look yaw) · rows`, `rows ← P(−look pitch − (tilt + wind · windTilt · 100) · 0.0174533) · rows`; the look angles follow the look input at 180°/s; without input and wind (+0x284 = 0) this is P(−tilt).
7. Output: quaternion of the rows → record +0x00, eye → +0x10, fov · 0.0174533 → +0x30, near → +0x34.

**Spring S (0x83E6D0)** [D, E; V: a one-frame emulation reproduces the live body state +0x110..+0x14F to 1e-6 over 805 frame pairs]:

    if not dt > 1e-4: prev = vin; return                     (0xCFDD14)
    a = (vin − prev) / dt; prev = vin
    q = 0.5 ^ (60 · dt); acc = (1 − q) · a + q · acc         (0xCBC4F8, 0xCAFEC4)
    vel −= (acc · Rᵀ) · inertia · dt                          (car frame)
    vel = (|vel| − damping · dt) · vel/|vel|  if (damping · dt)² < |vel|² and |vel|² ≥ 1e-9, else 0
    vel −= centering · off                                    (no dt)
    off += dt · vel
    off, vel ÷= (sx, sy, 1) where the scale is not 0
    if |off| > max: e = off − max · n̂; off = max · n̂; if vel · e > 0: vel ×= 0.5, vel −= (vel · ê) ê
    off, vel ×= (sx, sy, 1)

So damping is Coulomb (a constant speed off per frame): the offset moves only while the filtered car-frame acceleration exceeds damping/inertia (body 12 m/s², head 15.9 m/s²) and otherwise decays with time constant 1/centering (body 0.5 s, head 0.26 s). The body's limit is the ellipsoid 0.063 × 0.105 × 0.07 m; the head's y scale 0 keeps its offset to (x, z), a turn of at most 0.179 rad.

**Frame-rate dependence** [D, I]. The centering term has no dt, so the spring's stiffness follows the frame rate (about √(centering/dt) rad/s: at 165 fps body 18 and head 25 rad/s, at 60 fps 11 and 15). Run C was at ~165 fps; filling its missed frames as one lumped frame changes the crash result by up to 10° (`head_model.py --gap lump`).

**Inactive in a race** [D, V in run C]: headBuffeting off (0x802680 not called; its "bumps" are deterministic sines of the camera time, no random source); look input 0 in run C (the look path is ported in F499, § Look input); autoLookAngleInterest 0; the crash timer 0x801380 (|v − record +0xA0| > 5 → +0xEC = 3, re-triggered all race since +0xA0 is only written at the reset) changes nothing in the output; transitions (record +0xD0, +0xD4, +0xF4 = 0); TrackIR (no tracker). The +0x254 velocity-override branch is dead (only ever written with 0).

**Live check** (run C, 1480 rows synced with the raw capture's record time, Python reference model fed the 21 % of camera frames the sampler missed from neighbouring rows): eye p50 0.0 mm, p99 3 mm, max 4.9 mm (crash); direction p99 0.06°, max 0.127°; roll max 0.044°. The rigid seat of F470 for comparison: direction p99 2.3°, max 6.3°. 0–14 s (before the crash): eye ≤ 0.1 mm, direction ≤ 0.007°.

### Look input [D, E, F499]

Traced statically and emulated function by function in Unicorn 2.1.4 against reference models (worktree camera-dynamics `local/cam/look/`: `look-findings.md`, `look_emu.py`, `results.txt`). Nothing was run live.

- **Controller 0x8348F0** (vtable 0xCFF8A8 slot 1 → 0x834E60), once per controller update [I: once per rendered frame, before the views]: if the look byte +0x1C is set it sends a type 1 message to **every** view entry (camera slot 7) with `x = v(Look Left) − v(Look Right)`, `y = v(Look Up) − v(Look Down)`, the calibrated action values (analog); then, if +0x1E, a type 2 message (`x` = Look Back value while looking back, else 0) to the shown view. Every call, not only on change. The enable bytes come from 0x7EE8F0(value, mask) (bits 1 → +0x1C look, 2 → +0x1E look back, 4 → +0x1F change view, 8 → +0x1D FE tweak): the constructor sets look on, the race reset 0x584810 turns all off and sends one (0, 0), race-state slot 10 (0x5849F0) and 0x57AA5C turn all on [D; that this is the race start is I].
- Shipped bindings: keyboard preset 0 Look Left `win_key_delete`, Look Right `win_key_pageDown`, Look Up/Down unmapped; XInput preset 4 the right stick X and Y (biDirectionalLower/Upper, dead zone 0.3, saturation 0.8).
- **Chase** (slot 7 0x840A20 → 0x7FE690, every chase view including the reverses) [E, 4000 cases]: the point (A4, A8 + 0.1) at record +0xA4/+0xA8 follows (x, y + 0.1) in polar form: w = clamp(0.2 · speed) (car +0x1648, signed), blend B = 0.1 + 0.4 w, snap threshold A = 3.2 (1 − w) + w; with a1 = atan2(A4, A8 + 0.1), a2 = atan2(x, y + 0.1), d = a2 − a1 wrapped to ±3.141592: if A > |d| the angle becomes a1 + B · d and the radius r1 + B · (r2 − r1), else the point snaps to the input. After the chase pipeline (end of 0x82B6B0) **0x7FEC40** [E, 300 cases; V: run C's records reproduced with A4 = A8 = 0] turns the view and eye by a = atan2(A4, A8 + 0.1) about the world vertical through the car's model origin (record +0x110), toward +X for a > 0. So above 5 m/s a full Look Left snaps to 84.29° and back; at rest it eases in (8.4°, 16.0°, 22.8° … 78.2°) and out.
- **Mounted** (slot 7 0x840A30; idents `bumper` and `bonnet` only, not the reverses) [E]: `B0 += (x − B0) · 0.2` per message, used as Y(yaw + B0) in 0x803270. For `bonnet`, 0x822510 adds 0.05 · |B0| to the local eye's y and z (0xE4DDA0/A4) [E]: a lift and forward move of up to 5 cm, not a turn (correction of the F461 note).
- **Head** (slot 7 0x840EF0 → 0x801230) [E, 3000 cases]: type 1, `D8 = 0.8 D8 + 0.2 x`, `DC = 0.8 DC + 0.2 y`, looking (+0xF4 = 1) while x or y is not 0; type 2 (x = 0 while the head-cam shows) multiplies both by 0.8 unless looking. Held, D8 → x by ×0.8 per frame; released, ×0.64 per frame. Then 0x814BC0 → 0x801D10 (§ Head step 6): yaw toward maxHeadLook · g(D8) degrees (40°), pitch toward maxHeadLook · g(DC) · 0.3 (0xCFFDBC = 0.00523599 rad per degree: 12°), each at most 180°/s.
- **View change** [D]: the newly shown view's slot 10 zeroes its look state (chase A4/A8, mounted B0, head D8..EC); hidden views keep receiving messages but start from zero when shown. While looking back, a chase reverse view turns with the input; the Mounted reverses do not (ident check).
- All the smoothing is per message without dt (only the head's angle rate uses dt), so its speed follows the frame rate.

### Change View order [D, F461]

The in-race camera controller 0x8348F0 calls 0x833DD0 on a Change View press (action 5, latched, no transition active). 0x833DD0 finds the next view with 0x8325B0: index + 1, wrapping at the view count, skipping views whose byte +0x68 (cameras.xml `hidden`, parser store 0x80FED5) is set, stopping at the start. It then starts a transition to that view (all in-race views are `switch`). So the order is the next visible view, wrapping:

chase_close → chase_far → bumper → bonnet → head-cam → chase_close.

That the manager's entries are in file order is [I] (the parser loop was not traced; the restore fallback 0x832570 takes the first visible entry). Open: 0x833DD0 calls next, then previous (0x8326D0), and commits the new index through the transition manager (+0x123, +0x4D0, +0x510 → +0x128); which value +0x123 has in a normal race was not determined.

### FOV convention [D, F461]

The record holds fov · 0.0174533 at +0x30 and the near plane at +0x34. Both projection builders found use it as the **vertical** field of view: 0x8163F0 computes f = tan(π/2 − fov/2), x-scale f / aspect, y-scale f (aspect = width/height, 0x7F4F30), and the PSSG camera node 0xBAAFD0 has the same form. The path from record +0x30 to the main render camera's node was not traced. The port's vertical `PerspectiveProjection::fov` is therefore the same convention.

### Bindings [V]

Shipped presets:
- keyboard preset 0: Change View `win_key_c`, Look Back `win_key_end`, Look Left `win_key_delete`, Look Right `win_key_pageDown`;
- XInput preset 4: Change View `buttonLeftShoulder`, Look Back `buttonY`, Look Left/Right/Up/Down the right stick.

## 3. What the port does

Change View pressed edge (keyboard C, pad LB through the F325 action map) moves to the next visible view. While Look Back is held (End, pad Y, or diagnostics `look_back`), the view's `reverseCamera` is shown.

Previously the prototype's C key was look back. It now changes the view, as in the original.

Look (F499): Look Left/Right (Delete/PageDown, pad right stick X) and Look Up/Down (pad right stick Y) give `camera::Look { x: left − right, y: up − down }` each frame. The chase views move their look point and turn about the car (0x7FE690, 0x7FEC40), the bumper and bonnet turn (and the bonnet's eye lifts), the head-cam smooths the input and turns up to 40° sideways and 12° up, all as in § Look input. The look messages are taken once per physics tick, like the head-cam springs [AUTHORED: the original takes one per rendered frame, so its look speed follows the frame rate]. Replays pass no look input. The mouse free look (AUTHORED, F305) is unchanged.

| view | port |
|---|---|
| Chase (close, far, reverses) | The pipeline of section 2 (F461): suspension heave of the target, speed fall-back and gear drop-back of the eye, yaw lag about the car's Y, the car frame with the spin hold, follow height with the 5 m/s latch, look at the target on the car, roll with the yaw rate. FOV from the file. Free look (AUTHORED, F305) still orbits the eye about the target in the car frame. The drift blend weight is read from the car (car +0xFE0, 0 for the shipped cars; F510) and the gear term gets no shift time (car +0x2E4 is not kept), which is exact while gearUpDropBack is 0. |
| Mounted (bumper, bonnet, reverses) | Offset and acceleration shift as above, on the body pose. Rotation = body · twist · yaw · (face +Z) · pitch down · roll, with the twist frame of 0x803270 (F461; rows normalised, AUTHORED). FOV and near from the file. The shake of 0x815960 last (F508: the bonnet's speed-gated bob), its amplitude and time stepped once per physics tick [AUTHORED, as the look]. |
| Head (head-cam) | The head-cam of section 2 (F472): seat velocity v + ω × r into the body spring (eye offset) and the head spring (view turn), the eye at position + bodyLength + body offset on the model frame, the view turned by the head offset and pitched by the tilt (negative looks down, emulated). FOV and near from the file, the spring parameters from the file over the constructor defaults. Not ported: look, buffeting, crash timer, TrackIR, transitions (all inactive in a race). |

Camera time is simulation time: committed ticks × 0.016 s between frames. So pause holds the camera still, and scripted renders stay reproducible. The filter history resets on R (segment change), when ticks go backwards and on every view change.

Per-frame coefficients are AUTHORED: the original applies the close-follow damp once per rendered frame, and the port applies it per 1/60 s, so the result does not depend on the frame rate. The spin hold counts frames as the original does; the port counts only updates that advance the simulation (AUTHORED). The head-cam springs step once per physics tick (0.016 s), whatever the frame rate, so they behave as the original's at 62.5 fps; a shot covering several ticks spreads its input change evenly over them, a gap over 30 ticks (replay seek) drops the rest, and a history reset clears the offset velocity that the original's reset keeps (all AUTHORED).

The camera's inputs are read from the car words after the tick (`camera::Sample::of`: the model frame of 0x9AE3A0 from the physics pose, car +0x10 and +0x1630 (F470); velocity, angular velocity, car +0x1648, wheel +0x40/+0x4C, car +0xD234); nothing is written back. At each history reset the app logs `drive_camera event=start view=… eye=… model_origin=…`. Replays have only pose and velocity (`Sample::moving`: speed |v|, no rotation, suspension at rest).

Persistence: `<data dir>/camera.ini` holds `current_camera = IDENT`. It is read at startup, with the same fallback, and written on exit, with the same reverse-to-owner mapping (log lines `drive_camera event=restore|change|save`). This is the port's stand-in for the original's profile. It is not a `settings.ini` key, so the user's settings files are never rewritten.

Both cars: each car's own file gives its mount points (section 1). If the file cannot be read, the earlier authored chase is used, with `drive_camera event=fallback`.

## 4. Interpretations and limits

- FOV convention: vertical [D, section 2]; a check against the owner's 2026-10-09 chase-view recording is still open.
- Axes:
  - body frame +Z forward, +Y up, X as in the car model, the same frame the meshes use [I];
  - Mounted positive pitch looks down [D, emulated];
  - Head negative tilt looks down [D, emulated F472].
- The yaw lag integrates the heading change per sample instead of dt · ω_y. The result is the same up to the frame's rotation axis.
- The chase roll is applied as a world up vector rolled about the view direction, so its on-screen direction follows whatever mirroring the port's projection has relative to the original's left-handed one [I].
- Not ported:
  - the chase shake (0x810490; its scales are 0 in the in-race chase views, its angle path a constant ≤ 0.0027° pitch);
  - the gear drop-back input car +0x2E4 (exact while gearUpDropBack is 0, as in every in-race view);
  - the second impact envelope (its vector +0x1D0 is always 0);
  - the head-cam's wind and bumps (buffeting off in the shipped files), crash timer, TrackIR and its internal look-back state (§2 Head; inactive in a race);
  - the FE View Tweak message (front end only);
  - transitions (all switch);
  - drift and replay views.
- Cockpit rendering: head-cam needs the high-detail interior (`interiors/lac/int_lac.pssg`, `highResInterior`), which the app does not load yet. From the seat the exterior body is seen from inside.
- The chase, Mounted and head views were compared with the running original in F470 (section 2, "Live comparison"); the FOV values match the records. The head dynamics were ported in F472 from the image and checked against the same run (§2 Head). Open: the spin hold (not triggered live); the head-cam at a different frame rate and its spring state directly (next capture, below).
- **Next head-cam capture** (needs a live run; not done in F472). Record per view record, every camera frame (sample on each change of record +0xF0, or log +0xF0 with the target state): record +0x000..+0x1BF (adds the head spring +0x160..+0x1BF: last input +0x170, acceleration +0x180, offset +0x190, velocity +0x1A0, besides the body +0x110..+0x14F); object +0x170..+0x188 and +0x1F0..+0x208 (spring parameters as loaded), +0x244 (headBuffeting), +0x284 (wind amplitude), +0xD0 and +0x2A0 (transition), record +0xD8/+0xDC/+0xE4/+0xE8 (look). Run once unlocked and once at a vsync 60 fps to show the frame-rate dependence, with a braking stop and a kerb strike on the head-cam.
- **Look capture** (F499, needs a live run): the controller's +0x1C..+0x1F bytes during a race (controller `[[car+0x90]+0x2250]` per 0x584842), and per frame chase record +0xA4/+0xA8, bumper/bonnet +0xB0, head +0xD8/+0xDC/+0xE4/+0xE8/+0xF4 with the eye and quaternion while holding Delete, then PageDown, then the pad's right stick, once at a standstill and once above 5 m/s, in each view and while looking back.
- Renders (main checkout `local/cameras/renders/`, both cars) show the bonnet view without a visible hood. The eye may sit at or under the hood surface, which is back-face culled. This is not compared with the original.

## 5. Tests

`grid_formats::vehicle_views::shipped_touring_car_views` (ignored, GRID_GAME_ROOT) checks both shipped files:
- 20 views;
- the visible order;
- reverse views exist and are hidden;
- all visible views switch;
- bonnet and head positions;
- the parameter name with a trailing space.

`grid-app` `camera::tests` (deterministic, no data):
- cycle and wrap;
- restore and save mapping;
- chase at rest equals the file's position and target;
- look-back geometry;
- yaw lag size, speed dependence, the explicit-Euler rate and the inactive angular limit;
- the lagged frame;
- the close-follow drop of 5·sin(slope), set directly on the first update;
- F461, against hand-computed values: the eye height latch at 5 m/s (0.4 blend at 2 m/s); suspension heave (0.0127649 for 0.05 m on every wheel, then decay by 1 − 0.533801) and the raised target; fall-back 0.5 m at 30 m/s and the gear drop-back envelope (0.7125, rise keeps the larger value, fall 0.5); roll steps of 0.08 to the goal 1.114085 with weight 0.75 and the view's up tilt; the spin hold (arming, start, √0.08 rise, plateau restart, quiet jump to 1.2 s, 0.125 ease, held while controls are disabled, the half pitch/roll rates); the spin frame (held forward, halfway turn, straight blend near 0.97, unset held forward); the drift blend (−π/4 · 0.914214); the bonnet tilt state step and twist frame (moving and at a standstill);
- F470: the model-frame mount (`cameras_mount_on_the_model_frame`); the ignored diagnostic `replay_live_trace` (live trace in, port views out);
- rigid, pitched-down bumper and its reverse;
- bonnet braking shift within the limit;
- head seat position (raised by bodyLength) and rest while cruising;
- F472 head-cam: one spring step under braking (acceleration −15.18572, velocity 0.0050972, offset 8.15545e-5 m; nothing below damping/inertia; the car frame turned π/2; a too-short step), the ellipsoid limit (0.063 sideways with the outward velocity removed, 0.105 up, the head's y kept at 0), the view turned by the head offset (forward 0.1 rad looks down by 0.1 rad, right rolls the right axis down) and the eye moved by the body offset, braking lean and settling, one or two ticks per shot giving the same springs; `replay_live_trace` also writes the head-cam with the original's per-frame step;
- F510: the chase takes its drift weight from the car (`chase_takes_the_drift_weight_from_the_car`);
- F508 shake: the noise table against the emulated initializer (four entries; the ignored `shake_table_equals_the_emulated_table` compares all 6000 bit for bit with GRID_SHAKE_TABLE), the bonnet amplitude (0.49896473 after one step at 55.46 m/s, decay at 0.1798/s, zero below 1e-4, no state when all-zero), the noise (0.05897939 up, 0.075486794 side) and the eye moved along the view's up and right, the angle pitching down;
- F499 look input: the chase look point against the emulated sequence (eases in at rest, snaps at speed, rests without input), the chase turn about the car, the bonnet turn and lift (B0 0.2, 0.36; lift 0.018) and the bumper's turn without it, the head's smoothing (0.2, 0.36 … 0.9885, release 0.6326, 0.4049), its curve and its 180°/s approach to 40° and 12°;
- bit-identical repeat runs;
- reset clears the history.


## G240: stable bumper model visibility [D, authored adapter]

G239's real C/R sequence exposed persistent own-bumper geometry in the bumper view. Frozen G238 and G239 produce byte-identical matched bumper images; the accepted G222 Motion executable also shows the obstruction. This predates the new window sorting and transition diagnostic. The shipped eye, pitch, FOV and near plane are not moved to conceal it.

The original stable camera policy has an explicit selected-model suppression path:

- `0x607BC0` compares the shown identifiers `bumper` and `bumper_reverse` in its stable manager branch and calls `0x99C2F0` with the enable byte cleared. The older cache file is `local/cameras/mgr-1/608164.c.txt`: its filename denotes an interior address, while the function header is `0x00607BC0`.
- `0x99C2F0` stores that byte at the model manager's `+0x1CD4`. `0x9CB0B0` applies the override only to the selected model returned by `0x5E17C0`, leaving other models on their normal LOD choices.
- Suppression passes `max_lod + 1` to `0x8F7480`. This setter deliberately preserves the out-of-range selection instead of clamping it. `0x8F4F30` then disables every real LOD node: no node's index matches the chosen index. `0xBA0B80` writes the node enable byte at `+0x42`; the complementary skip byte is `+0x40`.

This is independent of `DisableDriver*`: bumper and bonnet share those driver flags, yet only the bumper identifiers select this whole-model suppression branch. The stable policy is established from existing cached pseudocode and aligned original instructions in the hash-pinned image above, without creating another Ghidra project. Alternate-camera blends, detached-body traversal and original shadow/probe pass policy remain unresolved.

The port's `player_view_visibility` adapts that policy only to the main camera's `Mesh3d` class inside `VisibleEntities`, after `VisibilitySystems::CheckVisibility`. It retains the class even when empty, allowing Bevy 0.19.1's retained queues to dequeue removed entities and requeue them when chase/bonnet returns. It changes neither global `Visibility`/`ViewVisibility` nor render layers, source meshes, camera geometry or simulation.

Ownership includes player `car0/` source parts, their transform descendants (including glass render proxies), and the optional world-space wheel exposure hull's explicit source link. Opponents and world effects such as loose glass shards and skid marks remain independent. Detached source parts retain their player ownership; parity with original detached-piece presentation is not yet established. Shadow and probe visibility lists remain under their existing systems; this is an explicit adapter scope, not a claim of original pass parity. Future `NoCpuCulling` use requires an equivalent GPU visibility policy: CPU camera-list filtering alone does not cover that bypass. Current player entities do not use it.

Validation: the crate gate passed 286 tests with default Clippy; feature Clippy and the DLAA/paint feature build also passed. The CPU fixture exercises source/proxy/grandchild ownership, explicit wheel-hull ownership without reparenting, independent view/light lists, reverse bumper, bonnet/head-cam/chase restoration and changed proxy ownership. Seven native1080/virtual30 rendered cases completed, including matched bumper/bonnet/chase controls and the complete five-C/one-R cycle. Bonnet and chase preservation PNGs are byte-identical; all paired physical exits and request-frame manifests match. Selected native bumper frames show unobstructed road, bonnet and head-cam retain their geometry, and the model returns in chase. Two additional captures cover forced reverse bumper and four active wheel hulls; the latter PNG is byte-identical to the zero-hull bumper control. Forced LookBack is not genuine End-input acceptance. No separate GPU depth/motion readback was taken; retained prepass behavior remains API-supported rather than separately measured. Probe snapshots and cross-frame differences are recorded separately in the private review. The accepted Motion launcher remains unchanged.
