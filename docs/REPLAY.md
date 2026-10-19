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
- Sound: not recorded as such; the replay states switch the audio mode
  (`set_audio_paused_replay`/`PausedReplay`, `StateAudioStopReplayMusic`) and the post-race replay
  plays `audio/music/replay%i.WIM` [D]. Whether engine sound is regenerated from replayed
  vehicle state was not traced.
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
| Replay Save | S | — |
| Replay Exit | Escape | Y, Start |

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
- The **terminal damage** replay (`terminal_damage_replay`, `TERMINAL_` cameras) starts a short
  time before the wreck (0x596C90: a runtime value + 0.6 s + 2.0 s) [D].

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
  otherwise referenced by the number of the frame holding them. A 12-car frame without changes is
  about 1.3 KB.
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
  [D]. The wreck screen's Replay plays the last 10 s (AUTHORED stand-in for the terminal-damage
  replay with its TERMINAL cameras).
- Interpolation between two samples: translation lerp and rotation slerp for car bodies, lost
  wheels and prop pieces [D 0x92F9E0]. Wheel displacement and steering lerp; the wheel angle is the
  sample's angle plus spin × time, as the live presentation does. Deformation, wheel geometry and
  broken flags step with the earlier sample. A jump over 20 m (a reset onto the line) is not swept
  through (AUTHORED).
- Controls: [`player.rs`](../crates/grid-app/src/replay/player.rs) ports §2.4: play/pause, the
  ±4.33× shuttle, scrub while paused, clamping at the window's start. Next/Previous Lap jump to the
  player's lap starts (AUTHORED: next marker after now, previous more than 1 s before now). Not
  ported: slow motion (no keyboard key), frame step, Replay Save. Replay Jump In (F12, pad X) is
  Flashback, docs/FLASHBACK.md.
- The simulation is held for the replay. The live presentation still runs every frame, and the
  replay then overwrites it in `PostUpdate`:
  - car bodies and wheels; lost wheels at their recorded frames;
  - body dents, projected from the recorded lattice;
  - broken props and pieces;
  - the camera.

  Damage parts that swing on the live car (bonnet, doors) ride on the replayed body. Bodywork
  lying on the road more than 2 m from its place stays where it is (AUTHORED).
- On Replay Exit the live view returns unchanged: drive_present redraws every part each frame, its
  deformation keys are cleared so live dents are projected again, and the HUD is shown again.
- HUD and menus are hidden while the replay plays (`osd_play_off`, `hide_pause_show_overlay`) [D].
  The replay panel (AUTHORED plain text) shows the title (the user's language strings), time,
  play state, camera and, for 6 s after the last input [D `timeOut`], the keys.
- Engine and tyre sound stay muted (the simulation is paused); replay music is not played.

### 3.3 Cameras (`replay/director.rs`)

- Default: the **replay director**. It loads the route's `replay_camera_config.xml`; while
  recording, each crossing of a zone by the player's car (segment between two samples against the
  oriented box with `size` as half extents [I: the racing line passes 9–11 m below the zone
  centres on every circuit, so a full-size box would never be entered; see §4]) is logged with its
  tick. At any replay time the active
  camera is that of the last zone crossed at or before it (else the initial camera), so seeking
  and rewinding show the same shots as playing forward. Ordinary entries are used; `TERMINAL_`
  ones are skipped.
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
  - `replay::frame`: encode/decode bit for bit with references, refusal of truncated frames,
    re-writing forgotten references, interpolation at and between samples, teleports;
  - `replay::stream`: the memory-only ring, spool round trip with out-of-order reads, a full
    spool;
  - `replay::player`: the shuttle formula, pause/scrub, clamping, loop, laps;
  - `replay::director`: zone crossing, the ordinary camera choice, zoom framing, path projection.
- Asset (`--include-ignored`):
  - `grid_formats::replay_cameras::shipped_route_0_directors`: the four circuits' camera
    configurations;
  - `replay::race_tests::recorded_frames_equal_the_live_race_on_every_circuit`: Donington,
    Nürburgring, Istanbul Park and Long Beach. Lacetti and BMW share the field, with 3 AI cars, and
    the AI drives the player's car, for 1600 ticks. Every recorded frame decodes to the live pose
    and velocity bit for bit, and the replayed body × wheel matrices equal the live
    presentation's. The frame count is 12/25 of the ticks, memory stays within 0x380000 bytes,
    and the director's zones are crossed;
  - `replay::race_tests::instant_replay_leaves_the_race_bit_identical`: Donington, both cars,
    3 AI. Instant Replay from the pause menu at tick 900: 12 s of replay with fast forward, rewind
    and a camera change, a loop, Replay Exit back to the open menu, Continue. At tick 1200 the
    race digest equals a race that was never recorded or replayed. The in-race Instant Replay key
    returns to the race;
  - `replay::race_tests::view_replay_plays_the_whole_race_from_the_results`: Nürburgring. Retire,
    View Replay from the first frame, laps, Replay Exit back to the results, the race unchanged.
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

- The vehicle component's own replay fields, the director's event timeline
  and highlight monitors, and the terminal-damage replay's start time were not traced.
- Not replayed (shown as they are when the replay starts): glass and light damage, shards,
  damage-part swing angles, tyre smoke and other particles, skid marks, sound.
- Replay Save, slow motion and frame step are not ported.
