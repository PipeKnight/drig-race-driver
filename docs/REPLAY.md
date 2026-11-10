# Race replays

17 October 2026, Claude Code. How the original records and plays back race replays, and the
port in the playable app (`crates/grid-app/src/replay.rs` and `replay/`). Static research only:
no live process, no input sent to the original.

Markers: **[D]** read directly from the executable or shipped data, **[V]** verified by a test or
run, **[I]** interpretation, **AUTHORED** the port's own choice.

## 1. Evidence

Private evidence in the main checkout's `local/replay/` (not published):

- `strings-exe.txt`: replay strings of the hash-pinned unpacked image
  (`local/compatibility/grid-unpacked.exe`, SHA-256 `aac46a64…`), made with `tools/scan_strings.py`.
- `decomp/r1`…`r13`: Ghidra 12.1.4 headless pseudocode, opened `-readOnly` on the existing
  `local/phase3/ghidra-project` (`ghidra/ReplayRefs.java` + `run.ps1`, a new private helper: references
  to an address, or the function containing it, decompiled).
- Instruction-level checks with Capstone 5.0.6 (`local/tools/python`), bundled Python 3.12.14.
- `cam-*.txt`: `route_0/replay_camera_config.xml` of the four circuits decoded with `grid-binxml`
  and `tools/binxml_tree.py`.
- The pause-menu session's decoded `system/states.bin` and `system/flow.bin`
  (`local/pause-menu/states.xml`, `flow.xml`).
- `ActionMap/actionMapPreset0.xml` (keyboard) and `actionMapPreset4.xml` (XInput pad).

## 2. Findings

### 2.1 Snapshots, not inputs

The original stores **state snapshots** and interpolates them; it does not re-simulate inputs:

- Each subsystem implements a `Recordable` with `serialiseState`/`deserialiseState` [D]: the strings
  "serialiseState not supported on TougeMode::Recordable" and "Failed to serialise data for
  component '%s'"; `pfx_recordable` for particle effects. The replay system's frame writer
  (0x93D000) begins a frame at the replay time and calls every recordable's write (vtable +4),
  keeping bytes-per-recordable statistics [D].
- Rigid bodies of the physics world (`neracing_physics_world_recordable.cpp`) are written as
  48-byte records with a rigid-body template index and an entity instance index, both below 1000
  ("Bad entity instance table index '%u' in replay data", "Bad rigid body template table index") [D].
- Playback (0x92F9E0) reads those records, decodes two stored transforms, converts both
  matrices to quaternions, slerps them with a polynomial arc approximation and lerps the
  translation, then sets the body's matrix [D]; the debug overlay prints `[interpolation=%f]` [D].
- The "recordable input class" string (`<!-- This file stores recorded inputs … -->`) belongs to a
  separate developer input recorder, not to the replay [I].

### 2.2 Rate and buffers

- **30 Hz.** All replay statistics are computed "@ 30Hz": "Avg. bytes/frame: %u [%u s per buffer
  @ 30Hz]", "DATA RATE (@30Hz): %u K/sec", "FILE INDEX: %u MAX INDEX: %u (allows for %u minutes
  @ 30Hz)" [D]. A frame is written per game update [D: 0x93DFE0 → 0x93D000]; the game update rate
  itself is 30 Hz [I, from these statistics].
- **Memory pool.** `system/updateMemoryMap.xml`: `<memorypool name="REPLAY" type="HEAP"
  size="7864320" …/>` (7.5 MiB) [D].
- **Two replay buffers** (0x9366F0 registers two handles):
  - an in-memory buffer constructed with 0x380000 bytes (3.5 MiB) at 0x64EDDF → 0x936670 [D]. Seeks
    clamp to its oldest time (0x91CC00 calls its vtable +0x1C) [D], so the Instant Replay can only go
    back as far as memory holds [I];
  - a disk-backed stream, `replay.pbf` in the per-user replay cache directory ("Failed to create the
    replay cache directory … Cache will be disabled"), opened with overlapped, unbuffered I/O
    (0x871D40, `CreateFileA … 0x60000080`) and created with the limit 0x7D000000 bytes
    (2 097 152 000) at 0x92E0E0 [D]. It holds the whole race for the post-race replay [I]. Old
    `replay_%08u.pbf` cache files are indexed by number [D].
- **Instant Replay length: 10 s.** `StateInstantReplayRewind` (enter 0x596C90) stores the float
  at 0xDB4FEC, 10.0, as the rewind; 0x63EBC0 seeks to `now − 10.0` (and 0x57F5C0 / the flashback
  path to `t − 10.0` with 0xDB4FE4, also 10.0) [D]. PAUSE_MENU.md earlier said 15 s; that was not
  sourced and is corrected here.

### 2.3 What is recorded

- Cars: through their entity components [D: per-component serialisation]. Which fields the
  vehicle component writes was not traced [I: body pose, wheels, damage].
- Props and loose parts: the physics world's rigid bodies [D: `rigid_body`, `loose_part`
  strings next to the recordable code], so detached wheels (F355) are bodies like props [I].
- Particle effects: `pfx_recordable` [D].
- The replay setup (0x64ECD0) registers its recordables by name through 0x942460 [D, static]:
  "skidmarks" (SkidmarksPlugin+0x78), "pfx_recordable", "broadcast: PFX scrapes" and
  "broadcast: PFX collisions", "broadcast: audio collisions", "mud", "scratches", "loose parts",
  "replay cam" and the vehicle and physics channels. So skid marks, particle effects and the
  collision sounds are replay channels of their own [D]; their record formats were not traced.
- Sound: not recorded as such; the replay states switch the audio mode
  (`set_audio_paused_replay`/`PausedReplay`, `StateAudioStopReplayMusic`) and the post-race replay
  plays `audio/music/replay%i.WIM` [D]. Only collision sounds have a channel of their own
  ("broadcast: audio collisions") [D]. The replay's level and shuttle cues are §2.7.
- Camera: not recorded; the replay camera system chooses shots at playback (§2.5) [D].
- Highlight monitors (`OverCliffMonitor`, `LoseWheelMonitor`, `BodyWorkDamageMonitor`,
  `ObjectCollisionMonitor` with `MinDeltaSpeed`/`MaxDamage`… parameters) feed the replay
  director's event timeline [D strings], not the playback [I].

### 2.4 Playback control

The replay controller's mode (+0xA30) and shuttle (+0xA7C) [D]:

| Mode | Time step per frame (0x944430) |
|---|---|
| 0 recording/live | `+dt` |
| 1 play | `shuttle × dt` |
| 2 / 0xC paused | 0 |
| 3 reverse | `−dt` |
| 5 slow motion | `shuttle × dt × 0.1` |
| 6 slow reverse | `−0.1 × dt` |
| 8 / 9 frame step | `±1/60 s`, then pause |
| 0xB scrub (paused, key held) | `+0xA84 × shuttle × dt`, +0xA84 = 1.0 (0x943FF0) |

Input (0x944710, checked at the instruction level) [D]:

- When either the rewind or the fast-forward action is above 0.1:
  `f = (ff − 0.1)·1.111 − (rw − 0.1)·1.111`. The dead zone terms cancel, so `f = (ff − rw)/0.9`.
- Playing: the shuttle is `3f + 1` (f ≥ 0) or `3f − 1`, so a full key press runs at ±4.33×.
- Paused: a press scrubs (mode 0xB) with shuttle `f`; a release returns to pause.
- Released while playing: the shuttle returns to 1.
- Replay Pause toggles play/pause. Another action toggles slow motion (no keyboard binding).
  Frame step (modes 8/9) has no binding in the shipped presets either [D].

Keys [D] (preset 0 / pad preset 4):

| Action | Keyboard | Pad |
|---|---|---|
| Instant Replay (in race) | Return | Back |
| Replay Pause | F1 | A |
| Replay Rewind | F2 | left stick X left, D-pad left |
| Replay Fast Forward | F3 | left stick X right, D-pad right |
| Replay Previous / Next Lap | F4 / F5 | — |
| Replay Prev / Next Camera | F7 / F8 | LB / RB |
| Replay Jump In (Flashback) | F12 | X |
| Replay Save | S | — (no consumer found, §3.2) |
| Replay Exit | Escape | Y, Start |
| Slow motion, frame step back / forward | F6, comma / period (AUTHORED) | — |

### 2.5 Instant Replay and post-race replay flow

From `flow.bin` and `states.bin` [D]:

- **Instant Replay** (pause menu `instant_replay` link, or the Instant Replay action in the race:
  `c_in_instant_replay…`): `hide_pause_show_overlay` → fade out → `instant_replay_rewind`
  (`StateInstantReplayRewind`: back 10 s, clamped) → fade in → `instant_race_replay`
  (`StateInstantReplay terminal=false silent=false`), with the `instant_replay_interface` screen
  (`ScreenReplayInterface timeOut="6.0f" max_replay_markers="10" on_by_default="true"
  full_replay="false"`). Links: `BACK` → `instant_replay_end` → back where it came from (the pause
  menu, or the race); `loop` → fade → rewind again; `jump_in` → the Flashback states
  (`flashback_flash1..3`, `replay_jump_in`).
- **View Replay** (results pill) → `c_post_replay_interface` (`ScreenReplayInterface
  full_replay="true"`): `post_race_replay_stop` → `post_race_replay_begin` → fade in →
  `post_race_replay_play`; `loop` → fade → begin again; exit → back to the results.
- The interface buttons: pause/play toggle, rewind and fast forward (hold), exit (hold), save
  (hold), jump in (Flashback icon), camera left/right [D].
- The **terminal damage** replay (wreck menu link `terminal_damage_replay`): `osd_play_off` →
  `terminal_flash` → `terminal_fade` → `terminal_damage_delay` → `instant_race_replay_silent`
  (`terminal=true silent=true`) under `terminal_replay_interface`, with links BACK, `jump_in` and
  `terminal_damage_menu` [O: flow.bin]. Its cameras are the ones whose name starts with
  `TERMINAL` (0x93F770) [D]. `terminal_replay_interface` is a `ScreenReplayInterface` (timeOut
  6.0, `full_replay` false; states.txt:9430) [O]; `StateTerminalDamageReplay` (vtable 0xCDA57C,
  factory 0x5AB670, enter 0x57F530), the state that applies TerminalFX_02, backs the ids
  `terminal_damage_replay` and `terminal_damage_slowmo_replay`, which no flow node uses [O]. The
  shipped crash replay therefore shows Effect_0000, where `terminal_fade` left it. The terminal
  rewind variants (0x596C90: 0xE6DFF8 + 0.6 s + 2.0 s)
  are used by no flow node [O]; §6.

### 2.6 Replay cameras

The replay camera set is separate from the race cameras (F349):

- Per route, `replay_camera_config.xml` [D] (`grid_formats::replay_cameras`, verified on the four
  circuits):
  - zoom and static trackside cameras with position, orientation, FOV limits, a framing radius,
    `targetOffset`, `lookahead`, roll, shake and turn parameters;
  - spline paths (cubic Bézier chains, four points per segment) for cameras that move along the
    track;
  - activation zones (oriented boxes) whose `entry` handler activates one ordinary camera and one
    `TERMINAL_` camera; `target.replay_wing1` and similar name views of the car.

  | Route 0 | cameras | static | zones | initial camera |
  |---|---|---|---|---|
  | Donington | 76 | 2 | 27 | `Initial_Camera_R01` |
  | Nürburgring | 88 | 13 | 63 | `initial_camera` |
  | Istanbul Park | 86 | 15 | 59 | `Initial_Camera_R01` |
  | Long Beach | 86 | 7 | 36 | `initial_camera_01` |
- The car's `cameras.xml` has hidden replay views: wing1/2, rear1/2, roof1/2, driver1/2 and
  terminal (docs/CAMERA_VIEWS.md) [D]. The results screen uses `replay_roof2` for its "funky"
  camera (`StatePostRaceSetReplayCamera`) [D].
- The camera classes behind the parameters: docs/REPLAY_DIRECTOR_RESEARCH.md (zoom vtable 0xD001A8, static 0xD000D8).

### 2.7 Replay sound (F797) [D static unless marked]

Twelve functions read the replay controller's mode (world+0x2200, then +0xA30); the shuttle
speed S is at +0xA7C and a flag at +0x9FC. The two sound readers are called from the swoosh
manager's update 0x44E860 (docs/AUDIO_RESEARCH.md §7.4):

- **0x4430C0, the replay level.** Its target T is 0, or −48 dB while +0x9FC is set. In mode 0
  (playing) with S < 0.97, T is −96: it plays cue (bank 0x33, 0x1D) once (latch +0x374) and the
  loop (0x33, 0x18) kept at +0x394. In another mode with the level below 0, T is −96; mode 2
  (paused) gives −48. With S > 0.97 the loop is stopped. The level +0x38C steps 4 dB per call
  toward T, clamped to [−48, 0], and is written to sound groups 0..7 through 0x46A7D0 on the
  mixer DAT_00EBF758 (group chosen by PTR_00E50264 vf+4). Modes 3, 4, 5, 6 and 7/2 play cues
  0x26, 0x20, 0x29, 0x26 and 0x22 with their own fade (+0x380: −0.8 per call down to −48, +4
  toward −12 in modes 0xB and 2).
- **0x442B30, the shuttle cues.** Cue (0x33, 0x25) when |S| falls to ≤ 0.2 from ≥ 0.9 (the
  previous S is kept at +0x378), cue (0x33, 0x24) when it rises to ≥ 0.9 from ≤ 0.2; not in
  mode 0xB.
- So the car and world groups 0..7 sound at full level while a replay plays at normal speed
  and fade to −48 dB when it is slowed or paused [D]. They must then be produced from the
  replayed vehicles [I: no recorded sound channel other than collisions]. Bank 0x33's file and
  the cues' samples were not identified [I].
- The mix mode around a replay is the flow's (AUDIO_RESEARCH §33): a replay from the pause menu
  is inside Paused (all groups −48 except Global Ambience and HUD), a replay from the results is
  Normal, the terminal-damage replay changes nothing [D].

## 3. The port

### 3.1 Recording (`replay.rs`, `replay/frame.rs`, `replay/stream.rs`)

- After every fixed tick (and after the damage parts' tick), `record` samples when the tick starts
  a new 1/30 s of simulation time: `floor(tick·12/25)` grows (a tick is 16 ms), i.e. 12 samples
  per 25 ticks [D 30 Hz; sampling on simulation time is AUTHORED, so recording does not depend on
  the frame rate].
- A snapshot holds, for every car (player first, then the opponents):
  - the pose as a full 3×4 matrix, so a sample decodes to exactly the recorded pose;
  - velocity;
  - each wheel's displacement, steering, angle and spin;
  - the wheel geometry (pivot, axis, wheel deformation frame);
  - the body deformation lattice (`body_deformation::Field::current`);
  - lost wheels' world frames.

  For the dynamic props it holds the broken flags and every live piece's body pose.
- Encoding (AUTHORED layout, in the spirit of the original's tagged records): little-endian, one
  frame per sample. Wheel geometry, deformation and broken flags are written when they change,
  otherwise referenced by the number of the frame holding them. Each car's 78 per-frame words
  (pose, velocities, wheel state, effect inputs) are predicted from the car's last three frames:
  linear (`2a − b`) when that predicted the previous frame's word better than repeating it, else
  a repeat; floats are compared in an order-preserving integer map. Only the residuals are
  written (zigzag, then varint), behind a 10-byte mask of the nonzero words; a car whose mask
  equals its last one writes a flag instead. A key frame (every word written as a residual from
  zero) starts every chunk and comes every `KEY_INTERVAL` = 64 frames, so a reader can start at
  any key frame; the decoder keeps the history and is reset on a cut. Measured on race.ini (12
  cars, tick 900): about 1.7 kB a frame (735466 bytes for 433 frames), 1.9 kB after a jump-in cut,
  against 4.09 kB raw.
- Buffers [D sizes]:
  - the newest frames stay in memory up to 0x380000 bytes;
  - older 1 MiB chunks (AUTHORED size) go to a private spool, `<data dir>/replay-cache/replay.spool`,
    up to 0x7D000000 bytes, so the whole race stays available;
  - when the spool cannot be created, memory only: the oldest chunks are dropped (a ring), like
    the original without its cache;
  - a full spool stops recording.

  Restart Race clears the stream; the spool file is deleted when the app ends.
- Recording only reads the simulation. Holding the race for a replay is the pause menu's
  `Driving::paused`, so the race resumes bit for bit [V, §4].

### 3.2 Playback

- Instant Replay:
  - from the pause menu (it stays open underneath and shows again on Replay Exit), or with the
    Instant Replay key (Return, pad Back) in the race (Replay Exit returns to the race);
  - starts 10 s before the newest frame, clamped to the in-memory window [D];
  - loops back to that point at the end [D `loop`].
- View Replay (results) plays the whole recorded race from its first frame and loops to the start
  [D]. The terminal-damage replay plays by itself once the wrecked car has rested (round 5) and
  again from the wreck screen's Replay: from 10 s before the crash (not before the oldest frame)
  to the replay's start (§6), with the zones' TERMINAL cameras and the ordinary image; the wreck
  menu shows on its exit.
- Interpolation between two samples: translation lerp and rotation slerp for car bodies, lost
  wheels and prop pieces [D 0x92F9E0]. Wheel displacement and steering lerp; the wheel angle is the
  sample's angle plus spin × time, as the live presentation does. Deformation, wheel geometry and
  broken flags step with the earlier sample. A jump over 20 m (a reset onto the line) is not swept
  through (AUTHORED).
- Controls: [`player.rs`](../crates/grid-app/src/replay/player.rs) ports §2.4: play/pause, the
  ±4.33× shuttle, scrub while paused, clamping at the window's start. Next/Previous Lap jump to the
  player's lap starts (AUTHORED: next marker after now, previous more than 1 s before now). Slow
  motion (modes 5/6: play and its shuttle at 0.1×) and frame step (modes 8/9: ±1/60 s, then
  paused) are ported with AUTHORED keys F6 and comma/period (actions Replay Slow Motion, Replay
  Step Back, Replay Step Forward), since the shipped presets bind none. Replay Save (action id
  0x1F in the action table 0xE4DFA0, string 0xD01C40) is bound to S, but no code queries it: the
  input queries `push ID; call [vtable+0x2C]` exist for Jump In 0x1C (0x57B220, 0x57F630, 0x6DE370)
  and Exit 0x20 (0x57B220, 0x57F630), and a scan of the whole `.text` for `push 0x1F` before a call
  and of the replay code (0x570000–0x5A0000, 0x930000–0x950000, 0x6D0000–0x6F0000) for an
  immediate 0x1F found only unrelated sites (0x5BFF24, 0x942D7E). The interface draws a save (hold)
  button (§2.5), but its action is never read [I: unused on PC]. Nothing to port.
  Replay Jump In (F12, pad X) is Flashback, docs/FLASHBACK.md.
- The simulation is held for the replay. The live presentation still runs every frame, and the
  replay then overwrites it in `PostUpdate`:
  - car bodies and wheels; lost wheels at their recorded frames;
  - body dents, projected from the recorded lattice;
  - broken props and pieces;
  - the camera.

  Damage parts (F796): each frame records every car's parts off their rest place (a hinged
  part's swing angle, a detached part's world frame). The replay draws them as recorded at the
  replay time: a part still attached then is drawn at rest, a swinging one at its replayed
  angle on the replayed body, a detached one at its replayed world frame. Glass and lights are
  found again from the replayed deformation (not latched: a rewind restores them); shards fall
  each time the replayed glass breaks (no glass sound). When the replay ends the live
  parts, glass and lights return.
- Replayed effects ([`replay_effects.rs`](../crates/grid-app/src/replay_effects.rs)): skid marks,
  wheel debris and tyre smoke show the state they had in the race at the replay time, forwards,
  backwards, slowed and after a seek. The frame records each car's effect inputs (`Effects`:
  angular velocity, speed, mass, handbrake, rear coefficient, open-wheel flag, and per wheel the
  surface token, radius, width, load, excess force, rear term, spin, steer pair and handbrake bit;
  169 bytes a car, lerped between samples). Written raw they doubled a 12-car frame to 4.09 kB
  (a 30 s memory window); with the predicted residuals of 3.1 a frame is 1.7–1.9 kB, so the
  0x380000-byte memory window, and with it the Instant Replay rewind and the Flashback reach,
  holds about 65–72 s of a 12-car race, more than the 59 s estimated for the original's frame
  without effect channels; the original's window also holds its effect channels, its reach in
  seconds is not known [I]. The replay re-runs the race's own effect systems
  tick by tick from those inputs, starting at a checkpoint of their state: the race keeps one
  every 2 s (all of the last 20 s, older ones every 30 s), the replay adds one every 0.4 s it
  steps (48 at most), and the first recorded frame starts empty. Re-running is bounded to 6 ms a
  frame; a long seek catches up over a few frames. The live effect systems skip while a session
  exists (they are frozen anyway), and Replay Exit restores the race's effects exactly. This is
  equivalent to the original's channels (§2.3) for what is seen, without a stream per effect
  (AUTHORED mechanism). Reconstruction check: the replayed inputs equal the live ones bit for bit
  at every sample (§4).
- On Replay Exit the live view returns unchanged: drive_present redraws every part each frame, its
  deformation keys are cleared so live dents are projected again, and the HUD is shown again.
- HUD and menus are hidden while the replay plays (`osd_play_off`, `hide_pause_show_overlay`) [D].
  The replay panel (AUTHORED plain text) shows the title (the user's language strings), time,
  play state, camera and, for 6 s after the last input [D `timeOut`], the keys.
- Engine sound (F797): every frame records each car's engine control (normalised engine
  speed, applied throttle, gear; `frame::Effects`). While a replay is shown, `engine_sound`
  sends the replayed controls to every car's engine voices once per frame and places them at
  the replayed poses and velocities (levels, pan, Doppler); the live controls return when the
  replay ends. The car groups get 0x4430C0's level (§2.7): full while playing at shuttle
  ≥ 0.97 without slow motion, otherwise 4 dB per frame down to −48 dB. The mix mode stays the
  flow's (a pause-menu replay is inside Paused). Tyre sound, the gear whine, backfires from the
  tick model and collision sounds stay silent in a replay; replay music and the shuttle cues
  (bank 0x33) are not played (§6).

### 3.3 Cameras (`replay/director.rs`)

- Default: the **replay director**. It loads the route's `replay_camera_config.xml`; while
  recording, each crossing of a zone by the player's car (segment between two samples against the
  oriented box with `size` as half extents [I: the racing line passes 9–11 m below the zone
  centres on every circuit, so a full-size box would never be entered; see §4]) is logged with its
  tick. At any replay time the active
  camera is that of the last zone crossed at or before it (else the initial camera), so seeking
  and rewinding show the same shots as playing forward. Ordinary entries are used, and in a
  terminal-damage replay each zone's first `TERMINAL` entry (a zone without one keeps its ordinary
  camera [I]).
- Track cameras: the original's zoom and static camera classes, decoded in
  docs/REPLAY_DIRECTOR_RESEARCH.md (F441 audit). In short:
  - target = car matrix x `targetOffset`, or a point on `targetPath`;
  - eye = `Position`, or a point on `sourcePath`. Projection paths never go back and smooth by
    speed; time paths follow their percentage curve;
  - FOV = atan(radius/distance) within the limits, from `defaultFov` at `zoomPerSecond`, or fixed;
  - `turnEnabled` turns with angular dynamics;
  - static cameras never turn.

  The camera state is stepped at each replay sample from the activation and interpolated, as the
  original replays its recorded camera state. No shake.
- `target.NAME` entries use the car's own view NAME through the race camera code (F349).
- Next/Previous Camera cycle: director, then the car's `replay_*` views, then its race views
  (AUTHORED order).

## 4. Tests

- Unit:
  - `replay::frame`: encode/decode bit for bit with references and predicted residuals across
    key frames, the same-mask flag of a parked car, a reader starting only at a key frame (frames
    dropped before it), refusal of truncated frames, re-writing forgotten references,
    interpolation at and between samples, teleports;
  - `replay::stream`: the memory-only ring, spool round trip with out-of-order reads, a full
    spool;
  - `replay::player`: the shuttle formula, pause/scrub, clamping, loop, laps, slow motion and
    slow reverse, frame step and its pause;
  - `replay::director`: zone crossing, the ordinary camera choice, zoom framing, path projection.
- Asset (`--include-ignored`):
  - `grid_formats::replay_cameras::shipped_route_0_directors`: the four circuits' camera
    configurations;
  - `replay::race_tests::recorded_frames_equal_the_live_race_on_*`: Donington,
    Nürburgring, Istanbul Park and Long Beach. Lacetti and BMW share the field, with 3 AI cars, and
    the AI drives the player's car, for 1600 ticks. Every recorded frame decodes to the live pose
    and velocity bit for bit, and the replayed body × wheel matrices equal the live
    presentation's. The effect inputs rebuilt from each frame (`wheel_ground::replayed`,
    `tyre_smoke::replayed_input`) equal the live `wheel_ground::sample` and `CarInput::read` bit
    for bit. The frame count is 12/25 of the ticks, memory stays within 0x380000 bytes,
    and the director's zones are crossed;
  - `replay::race_tests::instant_replay_leaves_the_race_bit_identical`: Donington, both cars,
    3 AI. Instant Replay from the pause menu at tick 900: 12 s of replay with fast forward, rewind
    and a camera change, a loop, Replay Exit back to the open menu, Continue. At tick 1200 the
    race digest equals a race that was never recorded or replayed. The in-race Instant Replay key
    returns to the race;
  - `replay::race_tests::view_replay_plays_the_whole_race_from_the_results`: Nürburgring. Retire,
    View Replay from the first frame, laps, Replay Exit back to the results, the race unchanged.
- Interactive window: [`tools/replay_window_check.py`](../tools/replay_window_check.py) records a
  race with the autopilot in a 1280×800 window (tick interpolation on, no audio), plays the
  Instant Replay (optionally in slow motion, `replay_slow`), jumps in and races on to a second
  replay; PASS from the `drive_race_replay`, `drive_flashback` and `replay_effects` lines.
- Results: see the F entry in PROJECT_LOG.md.

## 5. Flashback

Ported on top of this replay system: docs/FLASHBACK.md. In short, the original restores every
recordable from the replay frame at or before the replay time and cuts the stream there
(0x93DDA0) [D]; the port reaches the same state exactly with full-simulation checkpoints, the
player's logged inputs and a re-run of the ticks up to that frame, then cuts the stream
(`Replay::cut`, `Stream::truncate`) and holds the session through the transition
(`Replay::freeze`, `Replay::finish`). The original's jump-in rules and the 10 s rewind constants
used by the flashback path (0xDB4FE4) are in FLASHBACK.md 2.

## 6. Open items

- The vehicle component's own replay fields were not traced; the highlight monitors'
  structure is traced (REPLAY_DIRECTOR_RESEARCH.md §2.7) but not their parameter data; the
  terminal-damage replay's start position needs a capture (below).
- Replay sound (§2.7): engine sound is replayed (F797); tyre, whine and collision sound in a
  replay, the shuttle cues and replay music are not; bank 0x33 is not identified. Whether the
  original regenerates engine sound from replayed cars (rather than a recorded channel) is [I]:
  only collisions have a recorded channel.
- Not replayed: the port has no collision sparks; tyre smoke, skid marks and wheel debris are
  replayed (`replay_effects.rs`), and since F796 damage parts, glass, lights and shards too.
- Replay Save has no consumer in the PC executable (§3.2).
- Terminal-damage replay (round 3, corrected in round 4): the wreck screen's Replay runs a
  terminal session (`Replay::request_terminal`): the director takes each zone's first `TERMINAL`
  camera (0x93F770 compares 8 characters; 0x93F840 passes both to 0x93C9D0, which shows it while
  the director's +0xD20 flag is set [I]) and keeps the ordinary one where a zone has none [I].
  Round 3 also graded it with TerminalFX_02, read as the enter 0x57F530 of
  `terminal_replay_interface`; 0x57F530 belongs to `StateTerminalDamageReplay`, which no flow node
  uses (§2.5), so round 4 removed the grade.
- TERMINAL flag lifetime (round 5) [D]: +0xD20 (director ctx+0x2254 → +0x14) is set to 1 by
  `StateTerminalDamageDelay`'s update 0x57F1A0 when the wrecked car rests (before its 1 s count),
  by the delay's link 0x58E2E0 for a `FinalBoundary` disqualification, by 0x57F530 (unused state)
  and by the terminal rewind; it is cleared only by `StateInstantReplayRewind`'s enter (non-terminal)
  and its exit (terminal). The silent replay's enter 0x596A40 (its arguments read by 0x57B1B0:
  +0x2C terminal, +0x2D silent) and exit 0x57B180 (which sets +0x28 |= 0x100 and restores force
  feedback through 0x7EB110(+0x2E)) leave it, so the crash
  replay that follows the delay shows the TERMINAL cameras. 0x607BC0 also reads +0xD20 with the
  camera's +0x42, for the shadow cascade set-up only. The port turns the TERMINAL choice on with
  the terminal session; between the rest moment and the replay no replay camera is shown, so the
  difference is not visible.
- Terminal window (round 4) [D]: the silent StateInstantReplay enter 0x596A40 zeroes the rewind
  and posts UpdateMessage_ReplayState; its handler 0x63EBC0 sets the window END to the replay's
  start Te (0x91CA80, amount 0) and START to max(W − 10 s [0xDB4FE4], oldest frame) (0x91CC00),
  where W is the crash time stored at P+0xA18 by 0x59D870, and saves W − 10, Te and W in
  0xE6E008/0xE6E010/0xE6E018. It plays (0x9440F0(1)) then pauses (0x9440F0(2)) the player, which
  sits at END; forward stops at END and rewind at START. The terminal link 0x57B220 sets the
  replay controller's mode 3 (0x6A14D0); 0x1C leads to `jump_in`, 0x20 to `terminal_damage_menu`.
  Round 5: nothing automatic moves the time from END back to START [I: static]. The replay
  controller (ctx+0x225C; constructor 0x6A1430, vtable 0xCEC3E0, update 0x6BA130) in modes 3/4
  runs 0x6B6250, the terminal-intro tutorial, then the ordinary input 0x6AEF30, which moves time
  only on the player's rewind/play/shuttle; the player step 0x944430 → 0x944220 and the reverse
  step 0x9407A0 stop at the window's ends. The menu's Replay re-enters the replay through
  `loop_entry_special` (0xCCBF40, set by 0x524210), which keeps the saved window and crash time
  (0xE6E008, 0xE6E010, 0xE6E018). Whether the ScreenReplayInterface itself issues a rewind or play
  on entry needs a capture; its `timeOut` 6.0 is the key-hint fade (§3.2). The port anchors the
  window at the tick the player's car took terminal damage (`PostRace::wreck_tick`) and plays
  from START (AUTHORED start position); a terminal request without a crash time (the `replay_terminal` diagnostic) keeps the last 10 s.
  The terminal rewind variants of `StateInstantReplayRewind` (`terminal_replay_rewind_forslow`,
  `_visual`, `_special`; enter 0x596C90 rewinds by 0xE6DFF8 + 0.6 s (0xDB4FE8) or 10 s + 2.0 s, or
  by the whole elapsed time) are used by no `flow.bin` node [O]; 0xE6DFF8 is read only there.
- The shipped flow plays the crash replay by itself after `terminal_damage_delay` and shows the
  wreck menu on its exit; the port does the same since round 5 (`PostRace::crash_replay`; an
  unattended scripted run with diagnostics `results_continue` skips it). Where the original's
  crash replay starts (paused at END, or played from START) needs a capture; the port plays from
  START (AUTHORED).
