# In-race HUD and post-race results

17 October 2026, Claude Code. The original's race HUD layout and its post-race screens in the playable
app. Code: `crates/grid-app/src/race_hud.rs`, `crates/grid-app/src/post_race.rs` and per-car lap times
in `grid_tools::race::Standings`. Builds on the pause menu (docs/PAUSE_MENU.md). Static research only:
nothing was attached to the original and no input was sent to it.

Markers: **[D]** read directly from shipped data or code, **[V]** verified by a test or run, **[I]**
interpretation, **AUTHORED** the port's own choice.

Private evidence is in the main checkout under `local/pause-menu/` (`states.xml`, `flow.xml`) and
`local/hud-results/` (`scripts/cstr.py`, `runs/`).

## 1. HUD: what the original has

`Osd/sceneconfig.xml` is the HUD's node tree [D]. The groups are below, in tree order.

**`osd_right`**
- `object/dial_main=dial`:
  - `redzone`;
  - `object/dial_damage=damage` (engine, gears, suspension, steering, wheel 1–4: the F333 icons);
  - `object/dial_pressure=accelerate` and `=brake` gauges;
  - `object/dial_speedo=speedo` (three digit glyphs) and `object/dial_odo=odo` (seven digits);
  - `gear`, `outer_face`, `light_gear`, `light_revs`, `mph_kmh`;
  - the rpm `needle` with two blur images.
- `object/boost=boost`.
- `object/position_main=position`:
  - a stacker of four split pills (`player1..4`, each with name, position and split time);
  - `position_group` with `pos_number / pos_total`.

**`osd_left`**
- `object/messagebox=counter`: the central message box and its segmented timer.
- `object/minimap_main=minimap`: `track_mask` (`rd4minimap`), `brake_light`, `object/flag=flag`.
- `object/lap_time=lap_time`, a stacker of:
  - `main_group`: `big_lap_number / big_lap_total`;
  - `race_group`: race time;
  - `lap_group`: lap time, plus the `pb_wr` lap-record animation;
  - `best_group`: best time;
  - `offers_group`.
- `object/points_main` (drift only).

**`osd_bottom`**
- the drift meter.

**`osd_top`**
- `object/wrong_way_pill`;
- `object/penalty_pill`;
- drift combo.

Above the cars there are seven `driver_id` name tags.

`Osd/config.xml` [D]:
- Four selectable groups: `dial`, `minimap`, `raceposition`, `lapinfo`. These are the pause menu's
  Screen Setup toggles.
- Layouts: `race` shows all four; `spectatormode` shows only `lapinfo`.
- `dialinterface`:
  - `revlimit` 0.95, `gearchange` 0.90, `smoothing` 1.0;
  - display ranges 12000, 10000 and 8000 rpm, each with a 2000 rpm `redline`.
- `playerspositioninterface splitdisplay maxduration="10.0" minduration="5.0"`.
- `minimapinterface`:
  - `excessivespeedindicator threshold="10.0" lookahead="2.0" …` (the brake light);
  - `wrongway wrongwayinterval="4.0" rightwayinterval="2.0"`.

HUD texts in `language_eng.lng` [D]: `osd_lap` LAP, `osd_race` Race, `osd_laptime` LAP, `osd_best`
BEST, `osd_pb` PB, `osd_pos` POS, `osd_GO` GO, `osd_wrong_way` WRONG WAY!, `kph`, `mph`.

## 2. HUD: the port

`race_hud` is installed with the pause menu for every drive. Since F380 it draws the original's own
OSD scene (section 6); the plain panels below remain only as the fallback when `Osd/dial2.pssg` or
the OSD fonts cannot be loaded. The rules in the table (which values, when splits show, wrong way)
are the same for both.
- Each group can be switched off in Screen Setup, as in the original:
  - Lap & Times → lap group;
  - Race Position → position group;
  - Mini Map → minimap;
  - Dials → dial.
- Mini Map and Dials are now real options: `[osd] minimap`, `[osd] dials` in docs/SETTINGS.md.
- The old text HUD keeps only the heading (car / circuit / event) and the status lines.

| Group | Port | Rule |
|---|---|---|
| Lap group (left) | LAP n/total, RACE time, LAP time, BEST, with PB for 5 s after a new best | The event's completed laps + 1, capped at the total. Free practice: the lap timer's lap number, no total [I]. |
| Position (right top) | POS n/total. Split pills: the car ahead, the player and the car behind, with name, position and the gap in seconds | Pills show for 5 s after each completed lap (the `minduration`) [I]. The rows and trigger were not traced. Gaps are race distance at the player's speed (floor 10 m/s), as before. |
| Dial (right bottom) | Speed (km/h or mph), gear (N, R, 1…), rpm bar with red zone, GEAR and REV lights, throttle and brake bars, odometer | rpm = engine word +0x1894 × 60/2π against the limit +0x17A4 (rad/s [I]). Scale: the smallest range ≥ the limit [I]; red zone: its last 2000 rpm [I]. GEAR at 0.90 and REV at 0.95 of the limit [D values, I meaning]. The odometer is the distance driven this event [I]. |
| Damage icons | the F333 panel, moved beside the dial | `dial_damage` is part of `dial_main` [D]. |
| Message box (centre) | countdown number, then GO for 1 s | AUTHORED timing for GO. |
| Wrong way (top) | WRONG WAY! | Shown after 4 s moving against the racing line (speed > 3 m/s), hidden after 2 s the right way [D intervals, I reading]. |
| Minimap (left bottom) | the AI racing line (up to 160 points) and a dot per car (player yellow), CHEQUERED when the leader has the flag | AUTHORED: the original draws `track_mask` from its own data, with world +Z up [I]. |

Not ported:
- the dial's needle, blur and smoothing;
- boost;
- the excessive-speed brake light;
- penalty pills;
- driver name tags;
- the PB/WR animation;
- offers;
- spectator layout.

Pure functions are unit tested: `lap_info`, `position`, `dial`, `WrongWay`, `map_bounds`/`map_point`.

## 3. Post-race: what the original does

Free race flow `f_post` in `system/flow.bin`; screens in `system/states.bin` [D]:

- **Finish:** `post_race_entry` → OSD off → force feedback off → `post_race_cutscene`
  (`postrace`, skippable) → `post_race_stats_free_race`.
  - The stats screen has `stat type="position"` and `stat type="fastest_lap"`, with select =
    "Continue".
  - Its texts: Race Winner / Podium Finish / Finished; Fastest Lap, "Produced on Lap [LAPNO]".
  - Then `stats_flow_looper` → replay → `results_show_screen` → `post_race_results_generic`.
- **Retire from Race** (pause menu): `retire_race` → replay → `retire_discipline_decision` →
  `results_show_screen` → `post_race_results_generic`. There is no stats screen.
- **`post_race_results_generic`**:
  - title "Race Results", table "Positions", `RaceResultsAnimatedList` of 12 rows;
  - pills in itemflow order:
    - Continue → `end_race_finish_race`, back to the front end;
    - View Replay;
    - Exit → `exit_race_confirm` → `end_race_exit_event`;
    - Restart Race → `restart_race_confirm` → `restart_race`.
- **Columns** [D], from the header set-up at 0x52E4F0..0x52E6A8 (`race_results` branch):
  - `titles.content.position` = Pos;
  - `.points` = Points, only when 0x4C7F50 holds (career [I]);
  - `.driver` = Name;
  - `.time` = Time;
  - `.car` = Vehicle;
  - `.team` = Team.
- **Terminal damage:**
  - `terminal_flash` (1 s) and `terminal_fade` (1 s), then `terminal_damage_delay`, then the wreck
    screen.
  - Free race, `fr_terminal_damage`: Restart Race, Retire (link `exit` → `retire_race_confirm` →
    `end_race_exit_event`, leaving the event without the results) and Instant Replay.
  - Test drive, `td_terminal_damage`: Restart Test Drive, Finish Test Drive and Instant Replay.
- **Race end and estimates** come from docs/RACE_RULES_RESEARCH.md. Finalisation can come from the
  front end (0x583B00) or from retiring (0x585E00). An unfinished human makes every AI car still
  racing be estimated.

## 4. Post-race: the port

- When the player finishes, the stats screen appears at once; the cutscene is not ported.
  - The race goes on behind it, with the AI driving the finished car.
  - Continue ends the race (`Standings::finalize`, as the front end's 0x583B00 does [I]) and shows
    the results.
- Retire from Race (pause menu) retires and finalises as before, then shows the **results**
  instead of closing the app.
- Results:
  - Rows in classification order, a header and up to 12 cars (the original list's `size="12"`).
  - Pos, Name, Time, Vehicle (each car's model), Team ("--": the prototype has no teams).
  - An extra best-lap column, AUTHORED, because the task asks for best laps; the original shows only
    the player's fastest lap, on the stats screen.
  - Time: the winner's race time, then "+gap" to the winner [I: the original's cell format was
    not traced]. A `*` marks estimated times. Cars out show Retired, Wrecked or DNF with their
    completed laps.
  - The player's row is highlighted.
- Pills:
  - Continue and Exit (after its confirmation) leave the event. The app has no front end, so it
    closes, back to the launcher.
  - Restart Race (after its confirmation) restarts.
  - View Replay plays the whole recorded race (F360, docs/REPLAY.md); Replay Exit returns to the
    results.
- Terminal damage: the wreck screen appears 2 s after the damage (the two 1 s transitions [D]; the
  delay state's own wait was not traced).
  - Restart, after its confirmation.
  - Retire / Finish Test Drive, after its confirmation: the player retires and the app closes.
  - Replay plays the last 10 s (F360; an AUTHORED stand-in for the terminal-damage replay).
- While a post-race screen is up, the pause menu is off and gear presses are dropped. Once the race
  is over the simulation is frozen.
- Best laps:
  - The player's comes from the gate lap timer, or from the event result once finished.
  - AI cars get line-timed laps: `Standings::lap_times` and `best_lap`, measured when the race
    progress passes each lap length. AUTHORED; the original times every car with its own track
    tracker.
- Log: `drive_post_race version=1 … stage=stats|wreck`, `command=…`; `drive_race_result` lines at
  finalisation.

## 5. Tests

- `grid_tools::race::tests::lap_times_and_best_lap_follow_the_line_crossings`.
- `post_race::tests` (unit):
  - stats → results → leave;
  - results pills, Exit/Restart confirmations, View Replay (F360);
  - wreck screen after 2 s, free race and test drive;
  - results rows: order, total and gaps, estimated, out with laps, best laps (the player's
    gate-timed lap replacing the line-timed one), positions;
  - ordinals.
- `post_race::tests::screens_match_the_original_states_file` (asset):
  - the results pills, both wreck screens and the free-race stats types equal `states.bin`;
  - every label and column id is in the English table.
- `race_hud::tests`: dial ratios and ranges, wrong-way intervals, lap group, minimap mapping.
- `pause_menu::race_tests` (asset and private CD01/BP01/DF01, 3 AI opponents):
  - `retire_from_race_shows_the_results_on_every_circuit`, run three times:
    - Donington with a Lacetti field;
    - Nürburgring with a BMW field;
    - Istanbul Park with a mixed field.

    Each run drives 500 ticks, then Retire from Race from the menu. It checks the results stage, a
    finalised race, the player retired and every opponent estimated, the BMW label, a frozen
    simulation, then Restart Race from the results.
  - `a_finished_race_goes_through_stats_to_classified_results`: one lap at Donington with the AI
    driving the player's car. Finish → stats (position, best lap) → Continue → results. Checks a
    finalised race, the winner's total time and rising gaps.

## 6. Original look (F380)

18 October 2026, Claude Code. Code: `grid_formats::osd` (reader), `crates/grid-app/src/osd_scene.rs`
(scene, camera, text, animations), `osd.wgsl` and `osd_display.wgsl`, `race_osd.rs` (what the HUD
values drive). Everything is read at run time from the owner's install; no image is extracted into
the repository. Private evidence: worktree `local/race-hud-look/` (`dial2-tree.txt`, `osd-dump.txt`,
`decomp/`, `tex/` decoded textures, `app/data/*.png` renders, `checks/`).

### 6.1 What the files hold [D]

- `Osd/osd_loader.xml` loads `Osd/sceneconfig.xml`, five fonts and `Osd/dial2.pssg`.
- `Osd/dial2.pssg` is the whole HUD as one scene:
  - 423 nodes (244 rendered quads or meshes), 206 shader instances of 11 effect files
    (`osd3d_flat_image`, `osd3d_special_image`, `osd3d_maskramp`, `osd3d_needleblur`,
    `osd3d_reflect_*`, `osd2d_font`, ...);
  - 53 textures (`dial_base`, `dial_needle`, `rev_dials`, `dial_speedo`, `dial_gears`, `lap_font`,
    `time_font`, `minimap_*`, `timer_*`, `flag`, ...) and two DXT1 cubemaps `env_soft`/`env_sharp`;
  - 101 animations: Bezier keys (value, in and out control points) on node translation, Euler
    rotation and scale and on shader parameters (`DiffuseColour`, `alphaScaler`, `scrollX/Y`,
    `glowScaler`, `Ramp`, `blurScaler`), each with named time ranges (`NETIMERANGE`);
  - the camera `camera_dialcam`: perspective, vertical field of view 12.3068 degrees, at z 3320
    looking down -z (half height 357.9 scene units at z 0).
- The stored node positions are the groups' off-screen positions; each group's `on` range slides it
  in (for example the dial from x 810.5 to 452 in 1/3 s).
- `sceneconfig.xml` gives each text node (`rd4text`) a font `parameter`. It indexes
  `Osd/fontDescriptions.xml`: 0 `microgramma_osd` (extruded 3D glyph meshes), 1 `swiss_bold_osd_2d`
  (flat glyph meshes), 2 `micro_drift`, 3 `micro_lap`, 4 `swiss_time` (glyph quads into
  the scene's `drift_font`, `lap_font` and `time_font` atlases). The fonts hold
  `NEFONTMETRICS` (ascender, descender, 1000 units per em) and per glyph `NEGLYPHMETRICS`
  (advance, bearings, physical size) and `NEGLYPHKERN` pairs.
- Text nodes are one box quad each; the box's place around its origin and its height are all
  the scene says about alignment and size.
- grid.exe [D, decompiled privately]: the `aspectratio` attach (0x8D5CB0) formats the screen aspect
  with `%1.2f`, cuts it to three characters and plays the layout range of that name (`1.7` 16:9,
  `1.6` 16:10, `1.3` 4:3, `1.2` 5:4); a name without a range (0x8D5900) changes nothing, so a 20:9
  screen (`2.2`) keeps the default layout, the 16:9 one.
- Ranges used: `gear` `R N 1..9`, `light_gear`/`light_revs` `on/off`, `mph_kmh` `mph/kmh`,
  `outer_face` `12/10/8`, `object/position_pill=N` `normal/player/first`, `playerN` and
  `splittimeN` `in`, `messagebox_anim` `expand`, `anim_helper` (wrong way) `in/out`,
  `object/flag=flag` `in/out`, `object/pb_wr=*` `on/off`, the pedal gauges and damage icons
  `start`, `redzone` `start`, `needle` 0..3.333 s.

### 6.2 The port

- The scene renders through its camera into a transparent image at the 3D view's size, drawn as the
  lowest UI node (premultiplied). The pause menu, post-race screens, front end and touch zones stay
  above it, and the 3D post-processing does not touch it (AUTHORED composition). Nodes become Bevy
  entities in the saved hierarchy with the saved transforms; animations are sampled every frame.
- Shaders (`osd.wgsl`) [I: the effect files were not decoded]:
  - flat image: texture × colour, alpha × `alphaScaler`;
  - special image: the atlas scrolled by `scrollX/Y` plus its glow map × `glowScaler`;
  - mask ramp: `dial_redzone` holds the arc shape in red and the angle in green (0..1 over 135
    degrees from 12 o'clock, measured); texels whose green is at or above `Ramp` show;
  - reflect shaders: colour lit by the decoded cubemaps along the view reflection;
  - fonts: atlas × colour.
- Text: glyphs set by the original metrics and kerning. The box spans the font's descender to
  ascender [I]; left, right or centre alignment from where the box sits around its origin [I];
  titles upper-cased (the fonts have upper case only).
- Values to times [I]: speed and odometer digits at digit/6 s (the atlas steps), leading speed zeros
  hidden (AUTHORED), seven odometer drums with tenths on the red one; the needle at rpm/scale of its
  240 degree sweep; the red zone from the red line (`RED_LINE` 2000 rpm before the face's top);
  pedals over `start`.
- Shown: lap group (LAP n / total, RACE, LAP, BEST, PB pill after a new best), position number,
  slash and total, the split pills (player1 lowest; `player` and `first` colour states), dial with
  speed, gear, needle, red zone, gear and rev lights, km/h or mph, odometer, pedal gauges, damage
  icon panel, countdown box, wrong-way pill, minimap frame with the chequered flag.
- Minimap (AUTHORED): the original draws its track shape into `track_mask` (`rd4minimap`, not
  traced). The port hides the mask quad, draws the route as a light line inside the frame (north
  up, as the plain HUD) and moves the original player arrow (turned with the heading [I]) and one
  opponent marker per car.
- The OSD is off while the pause menu, a post-race screen, a replay or the front end is up
  (`post_race_entry` turns it off [D]; pause and replay from the original's pause frame [I]).
- Screen Setup still hides the four groups.
- Wider than 16:9 (AUTHORED): the original keeps its 16:9 layout. The port continues the layout's
  own slope (`anim_right` X 0 at 16:9, -52.82 at 16:10: 297 units per aspect unit) so the groups
  stay near the edges.
- Touch zones (P11, AUTHORED): they fill the lower half's sides, so with touch controls the minimap
  and the dial move up to end at 48 % screen height, and the app's help strip moves under the top
  buttons. Without touch the help strip is a small bottom-centre line (it used to cover the lap group).
- Phone: `tools/push_android_data.ps1` (and so `build_android.ps1 -BundleData`) now also copies
  `Osd/` and the five OSD fonts (about 3.6 MB); without them the app falls back to the panels.
- Not ported: boost, drift groups, chat, penalty pills, driver name tags, offers, the brake light,
  the needle blur sweep, the stacker re-flow (hidden groups leave their gap), the cutscene/post-race
  and pause-menu art (front-end packages, not this scene).

### 6.3 Checks

- `grid_formats::osd::tests`: Bezier sampling; the asset test reads dial2 (counts, camera, the
  `1.3` layout value -134.6), the cubemaps, `sceneconfig.xml` fonts and all five fonts with metrics
  and kerning.
- `osd_scene::tests`: metric layout (baseline, kerning, alignment); asset: `0:05.600` in
  `swiss_time` sits on its baseline inside `race_time`'s box.
- `race_osd::tests`: aspect range names (`2.2` for 20:9), red-zone angle, speed digits.
- Offscreen renders (Donington, 11 AI, autopilot, tick 1500): `r3-16x9.png` 1920 × 1080,
  `r4-20x9-touch.png` 2400 × 1080 with touch zones; inspected: texts, dial, red zone, minimap and
  markers in place; with touch zones nothing overlaps a zone.
