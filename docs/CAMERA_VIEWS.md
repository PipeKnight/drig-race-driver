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
   - Yaw lag as below, then the **drift blend** [D]: when the weight w > 0, the lag used for the frame becomes lag + (−atan2(v·X, v·Z) − lag) · t · w with t = clamp((|v| − 5) · 0.1, 0, 1) (0xCB093C, 0xCAFCD4). w is read at `[[[chase+0x64]+0xC]+0x24]` + 0xE78 + 0x168; that object is not identified [I] (at the same offset the Lacetti's live car dump holds 0, but the object is not the dumped one). The port passes w = 0.
   - **Roll** 0x800100 [D]: goal = ω_y · (1/π) · rollAngle · 1.4 (0xCE9608, 0xE4DD98); the state (record +0x9C) moves toward it by at most maxRollPerSecond · 0.5 · dt (0xE4DD90), otherwise takes it. Weight (record +0xA0) = clamp(speed · 1.25, min, max) mapped 0..1 over minimum..maximumRollSpeed (0xE4DD94; 1 when min ≥ max). The yaw-rate lane was checked in the disassembly (lane 1 of target +0x10).
   - **Car frame** 0x7FF3E0 [D]: the car pose with its forward F replaced by e · D + (1 − e) · F, or, when F · D < 0.97 (0xCB0F30), by F turned toward D by e · acos(F·D) (0xE4E164 = 1), and the frame rebuilt about the car's up (X = up × F', Y = F' × X). D is the held forward and e the spin envelope (step 3). A blended forward not longer than 0.1 (0xCF1208, also NaN from an unset D) keeps the car pose.
   - **Camera local** 0x8007B0 [D]: eye = position − (0, 0, fall-back) (0x800320), looking at target + heave. **Fall-back** = tiltFallback · ramp(speed, minimumTiltSpeed, maximumTiltSpeed) − min(gearUpDropBack · tiltFallback · g, tiltFallback · ramp) · s, where g is a per-gear weight (gears 1/R 1.0, 2 1.15, 3 1.452, 4 1.824, 5 2.1, else 2.412; 0xCFF578..0xCFF584, 0xCE8FD4) and s the drop-back envelope over car +0x2E4 (time since the shift): rising 0.5 − cos(t/initial · π)/2 (keeping its larger previous value) until gearUpDropBackInitialTime, falling (cos((t − initial)/(overall − initial) · π) + 1)/2 until gearUpDropBackOverallTime, then 0. In the shipped views tiltFallback is 1 (chase_close, 0..60) and 2 (chase_far, 0..80); 0 in the reverse views. gearUpDropBack is 0 in every in-race view, so the gear term is exactly 0 there. tiltAngle and cameraTiltEnabled are not read in this path.
   - Eye: the local eye turned by the lag about the car's Y and carried by the car frame. Its **height** is replaced by car y + eye_local.y − kCameraCloseFollowYScale · follow, where follow moves by kCameraCloseFollowForwardsRollDamp toward the car frame's forward slope (negated when the eye is in front, z > 0), clamped to ±kCameraCloseFollowYClamp; the first frame after a reset sets it directly (record +0xAC). Until the speed first reaches 5 m/s (0x8002D0 with +0x170/+0x174; latched in record +0xE0) the height is blended with the previous one by speed/5.
   - The orientation 0x8114B0 builds, with a roll gated by cameraRollEnabled, is overwritten by step 4; that roll has no effect.
3. **Spin hold** (impact envelope 1) [D]: ω' = ω with half of its components about the car's X and Z removed. A change of |ω'| above 18 rad/s² per second (0xE4DD58) arms 3 frames; three consecutive frames with |ω'| > 2.5 (0xE4DD5C) within them start a 2 s envelope (0xE4DD60) and store the frame's forward as D. The envelope rises √(1 − (t/2 − 0.9)/0.1) for 0.2 s (exponent 0xE4DD6C), stays 1 until t = 1.2 s (0xE4DD64/0xE4DD68), restarting at 1.8 s while the spin goes on and held while the controls are disabled; when the spin stops (three quiet frames) it jumps to 1.2 s and eases to 0 quadratically (exponent 0xE4DD70). The earlier note that this detector used |v| was wrong: it reads target +0x10.
   The second detector (|v| change above 50, 1 s, 0.4/0.8, exponent 0.5) scales chase +0x1D0 into the eye. +0x1D0 is only zeroed by the constructor (0x81EB1C) and copied between the object and the record (0x820F1D, 0x831E57), so it is always 0: no effect, not ported.
4. **0x810E50** [D]: the view looks from the eye at the target point on the car (car pose · (target + heave), 0x8130D0; not in the lagged frame) and rolls about its direction by roll · weight degrees (positive with invertRoll), independent of cameraRollEnabled. Up = cos a · n(d × h) + sin a · h with h = n((0,1,0) × d) (0x7FE8B0).

0x8435D0 then applies fixedRoll (0 in all views), stores fov · 0.0174533, and calls the shake 0x810490 (shakeScaleX/Y 0 in the chase views; not ported). Because 0xE4DD54 = 1 it finally calls 0x7FEC40, which turns the eye and orientation about Y by atan2(record +0xA4, record +0xA8 + 0.1). Those two words are zeroed by the record reset 0x831D20; their writers (0x7FE88E, an angle interpolation, and others at 0x819AF2..0x837B85) were found but not identified [I]. Not ported: treated as 0 in races.

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
   - Car +0x10 is dynamic. It held up to 0.2° of roll mid-run (run C); without it the error is up to ~0.5° of pitch under acceleration and ~1° of roll in the slide (runs A/B). The port does not compute it [I]; `Sample::of` reads the port's words and falls back to the identity when they are not a rotation.
2. **Camera time.** The original's camera integrates with the rendered-frame time. Here that is the gap between physics changes, about 5.9 ms median, not the world step +0x7C8 (6.06 ms median in run C, about 4 % more in total). With +7C8 the bonnet tilt state reached only 77–82 % of the original's; with the frame time it matches to 4 digits (`tilt.py`). The port feeds its own simulation time, consistent with its own Δv.
3. **Agreement with the frame-time dt and car +0x10** (run C, synced rows, 2-s bins):

   | view | eye | direction | roll |
   |---|---|---|---|
   | chase_close | 0–2.5 cm (4 cm at the end of the slide) | ≤ 0.1° | 0.00° |
   | chase_far | 0–2.3 cm (4 cm at the end of the slide) | ≤ 0.08° | 0.00° |
   | bumper | 0 | 0.00° | 0.00° |
   | bonnet | ≤ 0.6 cm | ≤ 0.07° | ≤ 0.01° |
   | head-cam | 1 cm lower | ≤ 0.12° | ≤ 0.3° in the slide |

   The internal states match too: chase lag (e.g. −0.367 against −0.369 in the slide), bonnet tilt state and shift to 3–4 digits, the eye-height hold below 5 m/s at the start. The head-cam differences are its own untraced dynamics (0x842DD0, backlog row 67).
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

**Bonnet turn** [D]: record +0xB0 is the look-left/right input. Its only writer, Mounted slot 7 (0x840A30), moves it by (x − B0) · 0.2 per message for idents `bumper` and `bonnet`; the sender 0x8348F0 sends x = Look Left (action 8) − Look Right (9), and the race reset 0x584810 sends 0. For `bonnet` 0x822510 also turns the local eye by |B0| · 0.05 (0xE4DDA0/0xE4DDA4) about two axes. The port has no Look Left/Right actions, so B0 is 0 and neither term has an effect; not ported. Open: the writer of the controller's look-enabled byte (+0x1C).

The bumper has all movement parameters zero, so it is rigid in the original too.

### Head [not traced]

The binder shows a two-mass body/head model: inertia, damping, centering force and maximum offsets, plus bumps, wind, maximum look angle and crash detection by velocity change. The 43 KB update 0x842DD0 was not traced.

### Change View order [D, F461]

The in-race camera controller 0x8348F0 calls 0x833DD0 on a Change View press (action 5, latched, no transition active). 0x833DD0 finds the next view with 0x8325B0: index + 1, wrapping at the view count, skipping views whose byte +0x68 (cameras.xml `hidden`, parser store 0x80FED5) is set, stopping at the start. It then starts a transition to that view (all in-race views are `switch`). So the order is the next visible view, wrapping:

chase_close → chase_far → bumper → bonnet → head-cam → chase_close.

That the manager's entries are in file order is [I] (the parser loop was not traced; the restore fallback 0x832570 takes the first visible entry). Open: 0x833DD0 calls next, then previous (0x8326D0), and commits the new index through the transition manager (+0x123, +0x4D0, +0x510 → +0x128); which value +0x123 has in a normal race was not determined.

### FOV convention [D, F461]

The record holds fov · 0.0174533 at +0x30 and the near plane at +0x34. Both projection builders found use it as the **vertical** field of view: 0x8163F0 computes f = tan(π/2 − fov/2), x-scale f / aspect, y-scale f (aspect = width/height, 0x7F4F30), and the PSSG camera node 0xBAAFD0 has the same form. The path from record +0x30 to the main render camera's node was not traced. The port's vertical `PerspectiveProjection::fov` is therefore the same convention.

### Bindings [V]

Shipped presets:
- keyboard preset 0: Change View `win_key_c`, Look Back `win_key_end`;
- XInput preset 4: Change View `buttonLeftShoulder`, Look Back `buttonY`.

## 3. What the port does

Change View pressed edge (keyboard C, pad LB through the F325 action map) moves to the next visible view. While Look Back is held (End, pad Y, or diagnostics `look_back`), the view's `reverseCamera` is shown.

Previously the prototype's C key was look back. It now changes the view, as in the original.

| view | port |
|---|---|
| Chase (close, far, reverses) | The pipeline of section 2 (F461): suspension heave of the target, speed fall-back and gear drop-back of the eye, yaw lag about the car's Y, the car frame with the spin hold, follow height with the 5 m/s latch, look at the target on the car, roll with the yaw rate. FOV from the file. Free look (AUTHORED, F305) still orbits the eye about the target in the car frame. The drift blend weight is 0 (source unknown) and the gear term gets no shift time (car +0x2E4 is not kept), which is exact while gearUpDropBack is 0. |
| Mounted (bumper, bonnet, reverses) | Offset and acceleration shift as above, on the body pose. Rotation = body · twist · yaw · (face +Z) · pitch down · roll, with the twist frame of 0x803270 (F461; rows normalised, AUTHORED). FOV and near from the file. |
| Head (head-cam) | Body pose · position, tilt in degrees (negative looks down [I]), FOV 48, near 0.075. Rigid. |

Camera time is simulation time: committed ticks × 0.016 s between frames. So pause holds the camera still, and scripted renders stay reproducible. The filter history resets on R (segment change), when ticks go backwards and on every view change.

Per-frame coefficients are AUTHORED: the original applies the close-follow damp once per rendered frame, and the port applies it per 1/60 s, so the result does not depend on the frame rate. The spin hold counts frames as the original does; the port counts only updates that advance the simulation (AUTHORED).

The camera's inputs are read from the car words after the tick (`camera::Sample::of`: the model frame of 0x9AE3A0 from the physics pose, car +0x10 and +0x1630 (F470); velocity, angular velocity, car +0x1648, wheel +0x40/+0x4C, car +0xD234); nothing is written back. At each history reset the app logs `drive_camera event=start view=… eye=… model_origin=…`. Replays have only pose and velocity (`Sample::moving`: speed |v|, no rotation, suspension at rest).

Persistence: `<data dir>/camera.ini` holds `current_camera = IDENT`. It is read at startup, with the same fallback, and written on exit, with the same reverse-to-owner mapping (log lines `drive_camera event=restore|change|save`). This is the port's stand-in for the original's profile. It is not a `settings.ini` key, so the user's settings files are never rewritten.

Both cars: each car's own file gives its mount points (section 1). If the file cannot be read, the earlier authored chase is used, with `drive_camera event=fallback`.

## 4. Interpretations and limits

- FOV convention: vertical [D, section 2]; a check against the owner's 2026-10-09 chase-view recording is still open.
- Axes:
  - body frame +Z forward, +Y up, X as in the car model, the same frame the meshes use [I];
  - Mounted positive pitch looks down [D, emulated];
  - Head negative tilt looks down [I].
- The yaw lag integrates the heading change per sample instead of dt · ω_y. The result is the same up to the frame's rotation axis.
- The chase roll is applied as a world up vector rolled about the view direction, so its on-screen direction follows whatever mirroring the port's projection has relative to the original's left-handed one [I].
- Not ported:
  - shake (0x810490; shakeScaleX/Y are 0 in the in-race chase views);
  - the drift blend weight and the 0x7FEC40 drift rotation (sources not identified; 0 in races [I]);
  - the gear drop-back input car +0x2E4 (exact while gearUpDropBack is 0, as in every in-race view);
  - the second impact envelope (its vector +0x1D0 is always 0);
  - the bonnet turn (needs Look Left/Right, which the port has no action for);
  - the head/body dynamics, wind and bumps (0x842DD0, backlog row 67);
  - transitions (all switch);
  - drift and replay views.
- Cockpit rendering: head-cam needs the high-detail interior (`interiors/lac/int_lac.pssg`, `highResInterior`), which the app does not load yet. From the seat the exterior body is seen from inside.
- The chase, Mounted and head views were compared with the running original in F470 (section 2, "Live comparison"); the FOV values match the records, and the head dynamics differ as expected. Open after F470: the car body rotation at car +0x10 (the port does not compute it), and the spin hold (not triggered live).
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
- head seat position;
- bit-identical repeat runs;
- reset clears the history.
