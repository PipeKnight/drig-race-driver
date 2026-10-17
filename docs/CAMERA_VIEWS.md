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

### Chase yaw lag [D, 0x8114B0]

Per frame:
- The lag angle integrates the car's yaw rate: `θ += dt · ω`.
- It is limited by `angularLimit` / `angularLimitDamping` (`θ = ±L·d + θ·(1−d)` beyond ±L). Then it is wrapped to ±π.
- It returns toward zero at `|θ| · k`, with `k = lagMinimum + (lagMaximum − lagMinimum) · speed / 55.5554`. The constant 0xE4E0FC is 55.5554 m/s, i.e. 200 km/h, and the speed is not clamped.
- If the return speed exceeds |ω| and θ crosses zero, θ becomes 0.

`angularLimit` is 10 and is compared with radians, so after the ±π wrap it never binds.

A drift blend (`+0x168` of a global, through atan2 of the velocity) and an extra term (`DAT_00E4DD54 == 0`, statically 1) are not ported.

### Chase close follow [D partly, 0x8114B0]

A state follows a component of the car's direction, clamped to ±`kCameraCloseFollowYClamp` (0.58). Each frame the state moves by `kCameraCloseFollowForwardsRollDamp` (0.08) toward it. The camera height then subtracts `kCameraCloseFollowYScale` (5) × state.

[I] The component is the forward vector's vertical part (the slope). It is negated for the reverse view.

### Chase suspension [D partly, 0x81EC90]

The four wheels' suspension values are taken beyond `suspDeadZone`. Their sum × 0.25 is mixed per frame with `suspMixScale`. Where the result is applied (with `suspScale`) was not found. Not ported.

### Chase impact response [D partly, 0x81EC90]

- When the change of |v| divided by dt exceeds 18, a 2 s envelope starts, smooth-stepped between 0.6 and 0.9 of its time. Its value is stored at `+0x220`.
- A second detector (threshold 50, 1 s, steps 0.4/0.8) scales the vector `+0x1D0` into the eye position.

Neither target value comes from cameras.xml, and `+0x220`'s consumer was not found. Not ported.

### Mounted movement [D, 0x822510]

This runs only when `movementSpeed ≠ 0`:
- `a = Rᵀ (v − v_prev) / (dt · 30)`; the constant 0xCAFDB8 is 30.
- The shift goal is `clamp(−a · axisScale, ±maximumShift)`.
- Each frame: `shift += movementSpeed · (goal − shift) · dt`.
- The eye is `offset + shift · finalMovementScale` in the body frame, or offset plus the position when `offsetOnly` is set.

A second state does the same with `axisScaleForTilt` at twice the rate and feeds the orientation (0x803270, with `twistScale`). Not ported. For ident `bonnet` the offset is also turned by `|state| · 0.05` about two axes. Not ported.

The bumper has all movement parameters zero, so it is rigid in the original too.

### Head [not traced]

The binder shows a two-mass body/head model: inertia, damping, centering force and maximum offsets, plus bumps, wind, maximum look angle and crash detection by velocity change. The 43 KB update 0x842DD0 was not traced.

### Change View order [I]

The handler of the Change View action was not located: the candidates checked were replay and message paths. The port cycles visible views in file order, as the restore fallback (first visible view) implies:

chase_close → chase_far → bumper → bonnet → head-cam → chase_close.

### Bindings [V]

Shipped presets:
- keyboard preset 0: Change View `win_key_c`, Look Back `win_key_end`;
- XInput preset 4: Change View `buttonLeftShoulder`, Look Back `buttonY`.

## 3. What the port does

Change View pressed edge (keyboard C, pad LB through the F325 action map) moves to the next visible view. While Look Back is held (End, pad Y, or diagnostics `look_back`), the view's `reverseCamera` is shown.

Previously the prototype's C key was look back. It now changes the view, as in the original.

| view | port |
|---|---|
| Chase (close, far, reverses) | Heading frame (car forward projected to horizontal) turned back by the yaw lag above. Eye = frame · (position − Y · yScale · follow), looking at frame · target, world up. FOV from the file. Free look (AUTHORED, F305) still orbits the eye about the target. |
| Mounted (bumper, bonnet, reverses) | Offset and acceleration shift as above, on the body pose. Rotation = body · yaw · (face +Z) · pitch down · roll. FOV and near from the file. |
| Head (head-cam) | Body pose · position, tilt in degrees (negative looks down [I]), FOV 48, near 0.075. Rigid. |

Camera time is simulation time: committed ticks × 0.016 s between frames. So pause holds the camera still, and scripted renders stay reproducible. The filter history resets on R (segment change), when ticks go backwards and on every view change.

Per-frame coefficients are AUTHORED: the original applies the close-follow damp once per rendered frame, and the port applies it per 1/60 s, so the result does not depend on the frame rate.

Persistence: `<data dir>/camera.ini` holds `current_camera = IDENT`. It is read at startup, with the same fallback, and written on exit, with the same reverse-to-owner mapping (log lines `drive_camera event=restore|change|save`). This is the port's stand-in for the original's profile. It is not a `settings.ini` key, so the user's settings files are never rewritten.

Both cars: each car's own file gives its mount points (section 1). If the file cannot be read, the earlier authored chase is used, with `drive_camera event=fallback`.

## 4. Interpretations and limits

- FOV convention [I]. The port treats `fov` as vertical (Bevy and D3DX `PerspectiveFovLH`). The projection consumer was not traced. A check against the owner's 2026-10-09 chase-view recording is open.
- Axes [I]:
  - body frame +Z forward, +Y up, X as in the car model, the same frame the meshes use;
  - Mounted positive pitch looks down. The reverse bonnet camera on the rear window, with pitch 0.2, would otherwise look at the sky;
  - Head negative tilt looks down.
- Lag speed [I]: |v|. The original asks the car object (vtable +8) and may use the signed forward speed.
- The yaw lag integrates the heading change per sample instead of the physics angular velocity. The result is the same up to the frame's rotation axis.
- Not ported:
  - chase tilt and roll (tilt/roll speeds, `rollAngle`, `maxRollPerSecond`);
  - shake (zero for the in-race chase views);
  - suspension heave;
  - gear drop-back (zero except drift_cam_rear);
  - impact envelopes;
  - Mounted tilt/twist and the bonnet turn;
  - the head/body dynamics, wind and bumps;
  - transitions (all switch);
  - drift and replay views.
- Cockpit rendering: head-cam needs the high-detail interior (`interiors/lac/int_lac.pssg`, `highResInterior`), which the app does not load yet. From the seat the exterior body is seen from inside.
- The FOV and head behaviour have not been compared with the running original.
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
- the close-follow drop of 5·sin(slope);
- rigid, pitched-down bumper and its reverse;
- bonnet braking shift within the limit;
- head seat position;
- bit-identical repeat runs;
- reset clears the history.
