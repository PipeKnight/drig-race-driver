# Flashback

17 October 2026, Claude Code. How the original's Flashback works and the port in the playable app
(`crates/grid-tools/src/flashback.rs`, `crates/grid-app/src/flashback.rs`). Static research only:
no live process, no input sent to the original. Builds on the replay system (docs/REPLAY.md).

Markers: **[D]** read directly from the executable or shipped data, **[V]** verified by a test or
run, **[I]** interpretation, **AUTHORED** the port's own choice.

## 1. Evidence

Private evidence in the main checkout's `local/flashback/` (not published):

- `difficulty-table.txt`: the `difficulty` table of `database/database.bin`, from the decoded
  `local/race-rules/data/database.json` (docs/RACE_RULES_RESEARCH.md decoders).
- `decomp/f1`…`f10`: Ghidra 12.1.4 headless pseudocode, `-readOnly -noanalysis` on the existing
  `local/phase3/ghidra-project`, with a copy of the replay session's helper
  (`ghidra/ReplayRefs.java`, `ghidra/run.ps1` writing to `local/flashback/decomp`).
- Small new private scripts, run with the bundled Python 3.12.14 `-I`: `strva.py` (string virtual
  address and its absolute references), `rd.py` (constants), `vt.py` (pointers into a code range,
  for vtables), `dis.py` (Capstone 5.0.6 from `local/tools/python`), `scan_used.py`,
  `scan_award.py`, `scan_call.py` (instruction scans).
- The pause-menu session's decoded `system/states.bin` / `system/flow.bin`
  (`local/pause-menu/states.xml`, `flow.xml`) and `language/eng.tsv`.

All addresses are of the hash-pinned unpacked image `local/compatibility/grid-unpacked.exe`
(SHA-256 `aac46a64…`, docs/REPLAY.md).

## 2. Findings

### 2.1 How many

- `database.bin` `difficulty` [D] has 24 rows, career rank 1..4 × star level 0..5, with
  `available_flashbacks` and `flashback_bonus`:

  | Star level | Name (`difficulty_N`) | Flashbacks |
  |---|---|---|
  | 0 | Basic | 5 |
  | 1 | Normal | 4 |
  | 2 | Serious | 3 |
  | 3 | Savage | 2 |
  | 4 | Extreme | 1 |
  | 5 | Ninja | 0 |

  The count is the same for every rank. `flashback_bonus` is 3000 / 12000 / 30000 / 60000 for
  rank 1 / 2 / 3 / 4 (rookie, pro, expert, international). The port's free-race AI difficulty 0.606
  is the rank-2 Normal row (`ai_ratio` 0.606), so a race has **4** flashbacks, as the owner's
  reference race options showed ("Normal / Random grid / 4 Flashbacks", docs/REFERENCE_RACE.md).
- The flashback manager is set up before the race by 0x6AF7C0 (called from the free-race
  options 0x4D5420 and a career state 0x58ABC0) [D]:
  - maximum = `available_flashbacks`, except **0 in Pro Mode** (`pro_mode` data, 0x4D5420
    passes `pro_mode == 1`) for mode 0, and 0 for a mode 4 (not identified; [I] a network or
    special mode);
  - used = 0; max award = `flashback_bonus`;
  - published to the UI data store as `flashback_manager_maximum` / `_used` / `_max_award`
    (0x6AA200), read back by 0x6AA260.
- Remaining = maximum − used (0x6A2240) [D]. 0x4CAD80 allows a flashback when remaining ≥ 1
  (and not when `pro_mode` is set in a mode-0 race) [D].
- Using one: `used += 1` (0x6A2260) through 0x4CADD0, called only from 0x59AFA0 [D], the enter
  of a state in the replay state family that also sets the game mode and starts a screen effect;
  [I] `StateReplayJumpInPrepare`, the first state of the jump-in.

### 2.2 When it can be used

- Only from a replay that has a `jump_in` link [D flow.bin]: the Instant Replay in the race
  (`c_in_…`, `f_in_…`, `t_in_…`), the Instant Replay from the pause menu (`…_pause_…`) and the
  terminal-damage replay (`…_terminal_damage_loop`, string `terminalDamageFlashback`). The
  post-race View Replay (`c_post_replay_interface`, `full_replay="true"`) has none.
- The Instant Replay's update (0x57B220 / 0x57F630) follows `jump_in` when the Jump In action
  (0x1C) is pressed, 0x937150 allows it and 0x4CAD80 allows it [D].
- 0x937150 [D]: every registered recordable's "can jump in" (vtable +0x18; the TougeMode
  recordable returns true, 0x6BF990) must agree, and the replay time (+0x1B0) must not be
  within **0.25 s** (0xCE8828) before the event time (+0xA08, "EVENT TIME" in the replay debug
  text 0xD10580), when the event time is set. The event time is the live race time when the
  Instant Replay was entered (0x63EBC0 stores it) [D], so a jump-in at the live moment is
  refused. A first condition compares the replay's focus entry with an owner entity; not
  identified.
- The replay interface (0x542A00) shows the Flashback icon (`icon_jumpin`, `flashback_icon`, pad
  X, `buttonType="hold"`) enabled only when 0x937150 and 0x4CAD80 both allow [D].

### 2.3 How far back

- The jump-in works on the in-memory replay buffer (0x93DDA0 acts only when the buffer at
  +0x1A0 is the memory one) [D]; the Instant Replay starts 10 s back (0xDB4FEC) and can be
  rewound to the start of that buffer, 0x380000 bytes (docs/REPLAY.md 2.2) [D]. So a flashback
  reaches as far as the Instant Replay can be rewound.

### 2.4 What is restored

- 0x93DDA0 [D], the jump-in restore of the replay system:
  1. walks the memory stream back to the frame at or before the replay time;
  2. cuts the stream there (the write position is set to that frame, so recording continues
     after it and the abandoned future is overwritten);
  3. calls every recordable's vtable +0x1C until one fails.
- The recordables' interface [D] (TougeMode::Recordable vtable at 0xCE8218): +4 writes a frame,
  +8 reads one, +0xC/+0x10 are `serialiseState`/`deserialiseState` ("not supported on
  TougeMode::Recordable"), +0x18 "can jump in", **+0x1C applies the last read frame to the live
  object** (0x6314F0 copies the read values into the live touge state and clears its flags).
- So the original restores the live game from its replay snapshot of that frame [D for the
  mechanism; I that every system — cars with damage, loose parts and props (physics world
  rigid bodies), AI, timing, particles — is a recordable that applies its frame]. Whether the
  RNG state is a recorded value was not traced.

### 2.5 The transition and resume

From `states.bin` / `flow.bin` [D]: `jump_in` → `replay_jump_in_prepare` →
`flashback_flash1` → `flashback_flash2` → `flashback_flash3` → `osd_play_on` → `enable_ffb` →
`replay_jump_in` (`frameDelay="2"`) → back to the race (`c_in_race_transitions`; from the pause
menu through `set_audio_normal`).

| State | Sample | Camera | Effect | Time |
|---|---|---|---|---|
| flashback_flash1 | flashback_shot_1 | replay camera | IG_whiteout → FlashbackFX_01 | 1.0 s |
| flashback_flash2 | flashback_shot_2 | on-car camera | IG_whiteout → FlashbackFX_01 | 1.0 s |
| flashback_flash3 | flashback_zoomup | in-game camera | IG_whiteout → Effect_0000 | 1.0 s |

`affectPPS="true"`, `controller_id="ingame"` on all three. `StateReplayJumpInOSDCountdown` and
`StateReplayJumpInRestore` are registered state classes not used by `states.bin`.

### 2.6 UI, HUD and results

- The count shows on the replay interface's Flashback icon and on the race options screen
  (`race.flashback` with `flashback_number`, "FLASHBACKS AVAILABLE:" `flashbacks_available`) [D].
  No in-race OSD element for it was found in the front-end strings [I: there is none].
- Results: the bank screen has a "Flashback Bonus" line (`bank_rewards_flashback_desc`) and the
  manager carries the award (`flashback_bonus`), so unused flashbacks earn career money [I]. The
  award formula was not found (no code reads the award getter 0x6A2290 or the published
  values except the reload 0x6AA260). A script/condition `usedFlashback` (0x418FE0) reads a
  per-race flag [D], [I] for stats/achievements (`fact_instant_replay_body` "[VALUE]
  Flashbacks"). The port has no career money, so nothing of this is ported.
- `jump_in_confirm_title` / `_subtitle` ("Are you sure you want to resume the race from this
  point?") exist in the language file but no screen of `states.bin` uses them [D].

## 3. The port

### 3.1 Rules (`grid_tools::flashback`)

`AVAILABLE`, `BONUS`, `AI_RATIOS` (the table above), `star_level(difficulty)`,
`Manager { maximum, used, max_award }` with `remaining`, `available`, `use_one`, and
`time_allowed(replay, event)` with `EVENT_GUARD` 0.25 s. The app's manager is the rank-2 row of
the AI difficulty (4 at the default 0.606). Drift Battle's AI level uses the same table.

### 3.2 Exact restore by checkpoints and re-simulation (`grid-app` `flashback.rs`)

The port's replay frames hold presentation only, so they cannot be applied to the simulation.
The same end state is reached exactly another way (AUTHORED):

- **Checkpoints**: after a tick, every `CHECKPOINT_TICKS` = 250 ticks (4 s), a clone of the whole
  simulation: the player's `Runtime` and body geometry, the shared world (`MultiRuntime` with its
  cars, props, loose wheels and RNG states), the AI drivers, race manager (pacing), standings,
  autopilot, the event clock (`Trial`), the damage parts and the Drift Battle scorers. Checkpoints
  older than the in-memory replay window are dropped. Cloning is cheap where data does not
  change: the track geometry, surfaces, start states (also the deformation registration's) and
  car body geometry are shared (`Arc`), the route props' break state is copy-on-write.
  - Measured [V]: a 4-car checkpoint is 2.19 MB (counting allocator, harness, Donington); a
    12-car race with every car deforming is about 13 MB per checkpoint (mostly each car's
    0x21000-byte vehicle words and deformation owner). Peak working set of a 4000-tick (64 s)
    12-car scripted race: 1003 MB against 809 MB for the playable without Flashback (with a 2 s
    interval it was 1235 MB, which is why the interval is 4 s).
- **Player inputs**: `drive_tick` logs the player's controls and gear request of each tick.
- **A car reset from the pause menu** changes the car outside the tick: a checkpoint of the
  changed state is taken at once (detected as a pose change at the same tick), and is used only
  for later frames, so ticks run again never cross the change.
- **Jump-in** at replay time t: the frame at or before t [D] gives tick T; the checkpoint at or
  before T is restored and ticks up to T run again (`drive_tick`, damage parts, drift scorers)
  with the logged inputs; the AI is deterministic given its state. The simulation is
  deterministic, so the state equals the one the race had at T [V, 4]. At most 249 ticks run
  again, at most `RUN_BUDGET` = 8 ms of them per frame, while the first flash holds at full
  white. Measured [V]: 88 ticks of the 12-car deforming race in 552 ms, 86 ticks of the 8-car
  Drift Battle in 296 ms (real app, release).
- Then, as the original: the replay stream is cut after T (`Replay::cut`, `Stream::truncate`),
  later lap markers and director entries are forgotten, and recording goes on from T.
- Restart Race restores the race's flashbacks [I: the data store keeps the values published at
  set-up, which 0x6AA260 reads back].

### 3.3 Flow and UI

- Replay Jump In: F12 (keyboard preset 0) or pad X (preset 4's `Replay Jump In`), in an Instant
  Replay from the race, from the pause menu and from the wreck screen's replay; not in View
  Replay.
- Refused (logged as `drive_flashback … event=refused`): none left, not an Instant Replay, the
  race finished, within 0.25 s of the event time, the frame before the memory window, no
  checkpoint.
- Accepted: one is used (prepare), the state is restored, the replay holds at the jump-in frame
  (`Replay::freeze`, no replay input) for three 1 s flashes (AUTHORED look: a white overlay fading
  out each second; no samples or cameras of their own), 2 more frames, then the replay ends, the
  pause menu closes, the wreck screen is dismissed and the race resumes.
- The replay panel shows `F12 flashback   FLASHBACKS AVAILABLE: n` while a jump-in is possible,
  like the original's icon (AUTHORED plain text).
- Logs: `drive_flashback version=1 event=install|checkpoint|checkpoint_after_change|jump_in|restored|refused|failed|resume`.
- Diagnostics `flashback_capture = S` (`GRID_FLASHBACK_CAPTURE`): in the `replay_capture` Instant
  Replay, Jump In after S seconds; the scripted drive then continues from the jump-in frame.

### 3.4 Not restored or not ported

- Presentation only, kept from the abandoned future: skid marks, tyre smoke, wheel debris
  particles, sounds. Glass and light damage are drawn intact again and found anew from the
  restored deformation; shards are removed.
- The transition's samples (`flashback_shot_1/2`, `flashback_zoomup`), its post effects and
  camera changes; force feedback; the Pro Mode switch (no race options screen); career money
  (Flashback Bonus).
- `drive_input` log lines after a jump-in go back in tick; `grid-soak replay-log` does not read
  `drive_flashback` lines yet.

## 4. Tests

- Unit: `grid_tools::flashback` (counts of the rows, Pro Mode, award, use until none, the
  event-time rule); `replay::stream` (`a_jump_in_cut_keeps_the_frames_up_to_it_and_records_on`).
- Asset (`--include-ignored`), the pause-menu race harness with the recorder and Flashback,
  3 AI cars, the player driven by keys as a function of the tick (throttle, weaving, braking):
  - `flashback::race_tests::a_flashback_resumes_the_race_tick_for_tick_on_every_circuit`:
    Donington, Nürburgring, Istanbul Park, Long Beach, Jarama; Lacetti and BMW in every field,
    the player's car alternating; at tick 900 the Instant Replay key, 1.4 s of rewind, Jump In.
    The jump-in frame is the one at or before the replay time, the stream is cut there, the
    restored state equals the reference race's at that tick, the transition takes 3 s and 2
    frames without a tick, and every tick up to 1300 equals the race that never flashed back
    (all cars' vehicle and physical words, lap, clock, standings, props, contacts, retirements,
    race manager, throttles). On Donington a pause-menu car reset at tick 160 lies between the
    checkpoint and the frame;
  - `…::a_flashback_in_a_drift_battle_restores_the_scores`: Long Beach route 1, BMW field,
    Drift Battle scorers included in the comparison;
  - `…::four_flashbacks_at_normal_then_none`: refused at the event time; four jump-ins (one from
    the pause menu, which closes); then none remain, Jump In does nothing; Restart Race gives
    them back.
- Real app [V], release, offscreen, `settings/race.ini` (11 AI, deforming) and
  `settings/drift-battle.ini` with autopilot, `--drive-ticks 900`, `replay_capture = 4`,
  `flashback_capture = 1`: jump-in from tick 900 to 338 (Drift Battle 336) from the checkpoint at
  250, the race resumes and drives to 900 again; every `drive_sample` line (pose, velocity,
  damage RNG, contacts) from the jump-in tick on is identical in both passes. The replay panel
  shows `F12 flashback   FLASHBACKS AVAILABLE: 3`.
- Results: see the F entry in PROJECT_LOG.md.

## 5. Open items

- Memory: about 13 MB per 12-car checkpoint; a word-level delta against the shared start state
  would cut most of it (the vehicle words barely change).
- The award formula of the Flashback Bonus, the mode-4 exception, the focus condition of
  0x937150 and which replay state calls 0x93DDA0 were not traced.
- The vehicle recordable's own apply was not traced; [I] it restores the full car state like the
  touge one.
- Whether Restart Race keeps used flashbacks (port: it gives them back) needs a live check.
- A flashback from the wreck screen is wired but not covered by a test (a terminal-damage
  wreck needs a scripted crash).
