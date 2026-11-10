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
- `object/lap_time=lap_time`, the stacker `lap_time_stacker` (rest y in dial2 [V]) of:
  - `main_group` (0): `big_lap_number / big_lap_total`;
  - `race_group` (-48.18): race time;
  - `lap_group` (-73.18): lap time, plus the `pb_wr` lap-record animation;
  - `best_group` (-98.18): best time;
  - `offers_group` (-129.054).
  The groups have no animation of their own; each slot has `{slot}_title` and `{slot}_time`.
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

HUD texts in `language_eng.lng` [D]: `osd_lap` LAP, `osd_race` Race, `osd_time` TIME (the race
and remaining time rows), `osd_laptime` LAP, `osd_best` BEST, `osd_pb` PB, `osd_pos` POS, `osd_GO` GO, `osd_wrong_way` WRONG WAY!, `kph`, `mph`.

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
| Lap group (left) | LAP n/total, RACE time, LAP time, BEST (mm:ss.hh, section 6.4); after each lap the LAP time shows that lap for 5 s, with the PB pill when it is a new best (section 6.4) | The event's completed laps + 1, capped at the total. Free practice: the lap timer's lap number, no total [I]. |
| Position (right top) | POS n/total. Split pills: the leader and the three cars around the player, with name, position and split time | Shown when the player passes a split point (sector end or lap end), for `maxduration` 10 s, at least `minduration` 5 s after a late car's time arrives; the first row with a time shows the race clock, the others the signed difference to it (section 6.4) [D]. |
| Dial (right bottom) | Speed (km/h or mph), gear (N, R, 1…), rpm bar with red zone, GEAR and REV lights, throttle and brake bars, odometer | rpm = engine word +0x1894 × 60/2π against the limit +0x17A4 (rad/s [I]). Scale: the smallest range ≥ the limit [I]; red zone: its last 2000 rpm [I]. GEAR at 0.90 and REV at 0.95 of the limit [D values, I meaning]. The odometer is the distance driven this event [I]. |
| Damage icons | the F333 panel, moved beside the dial | `dial_damage` is part of `dial_main` [D]. |
| Message box (centre) | solo time trial only: countdown number, then GO for 1 s | AUTHORED timing for GO. Races show no box (F590): the original single-player session never starts it (AUDIO_RESEARCH §28). |
| Wrong way (top) | WRONG WAY! | Shown after 4 s moving against the racing line (speed > 3 m/s), hidden after 2 s the right way [D intervals, I reading]. |
| Minimap (left bottom) | the AI racing line (up to 160 points) and a dot per car (player yellow), YELLOW while the player is in a yellow flag's zone (F759) | AUTHORED: the original draws `track_mask` from its own data, with world +Z up [I]. |

Not ported:
- the dial's needle smoothing (the OSD dial has the blur, section 6.4);
- boost (no stock car has it, section 6.4);
- driver name tags (traced, section 6.4);
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
  - `terminal_flash` (1 s) and `terminal_fade` (1 s), then `terminal_damage_delay` (the car's rest,
    at most 2 s, then 1 s; section 6.4), then the crash replay, then the wreck screen.
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
- Terminal damage: the wreck screen appears after the two 1 s transitions [D], then the delay
  state's wait for the car to rest (at most 2 s) and 1 s more [D, section 6.4], so 3 to 5 s after
  the damage. The original plays the crash replay before its menu; the port shows the menu at once
  (AUTHORED).
  - Restart, after its confirmation.
  - Retire / Finish Test Drive, after its confirmation: the player retires and the app closes.
  - Replay plays the terminal-damage replay (TERMINAL cameras, the ordinary image) from 10 s
    before the crash to the replay's start (docs/REPLAY.md §6).
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
  - `retire_from_race_shows_the_results_on_*`, run three times:
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
- Window resize (F737): the image follows the UI camera's size, so a resize (maximising the window,
  a phone rotation) replaces its GPU texture under the same handle. A Bevy 0.19 UI material builds
  its bind group only when the material asset changes (`PreparedUiMaterial` tracks no image
  dependencies), so `set_size` also touches the display material. Before that, the screen kept the
  old texture after a resize: the whole OSD (dial, timers, position, minimap) stood still as it was
  at the resize while the race went on (owner playtest of ee26c2ebc427, window maximised to
  2560 × 1369 mid-race). The render world prints `osd_display bound=B size=WxH` when the binding or
  the texture size changes (`check_display`); `bound=false` is a stale OSD on screen.
- Texture addressing (F739): each texture keeps its own `wrapS` / `wrapT` [D]. 48 of the 53 clamp
  (value 2): the needle, its blur, the dial faces, every sprite. 5 repeat (value 1): `dial_odo` and
  the drift combo strips, which scroll past their ends [I: 1 repeat, 2 clamp, from which ones
  scroll]. The port used to repeat every texture. Bilinear filtering and mips then pulled a
  sprite's opposite edge into its far edges: two thin white lines beside the needle and its blur
  (owner screenshot, prototype ee26c2ebc427, at 0 km/h).
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
  icon panel, countdown box, wrong-way pill, minimap frame with the flag, which plays `in` while the
  player is inside an active yellow flag's zone, as 0x6BA460 (F759; before, it played for the
  chequered flag; docs/RACE_RULES_RESEARCH.md "Flag system").
- Minimap [D] (F777): a chase-camera view of the flat track inside the
  `track_mask` ring, the player arrow fixed below the centre pointing up, one opponent disc per car
  projected from the same camera ("Minimap [D]" below). The port hides the mask quad and draws the
  ribbon itself (`race_osd::minimap`, `race_osd::MapView`).
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
- Drift groups: `drift_osd` drives the meter, the points box and the combo panel in drift events
  (section 6.5); the text panel of `drift_event` then stays hidden.
- Not ported: boost, chat, driver name tags, offers, the cutscene/post-race
  and pause-menu art (front-end packages, not this scene).

### 6.3 Checks

- `grid_formats::osd::tests`: Bezier sampling; the asset test reads dial2 (counts, camera, the
  `1.3` layout value -134.6, the needle and its blur clamp, `dial_odo` repeats, 5 repeat), the cubemaps, `sceneconfig.xml` fonts and all five fonts with metrics
  and kerning.
- `osd_scene::tests`: metric layout (baseline, kerning, alignment); asset: `0:05.600` in
  `swiss_time` sits on its baseline inside `race_time`'s box;
  `a_resize_rebinds_the_display_material_to_the_resized_image` (a new size modifies the display
  material, an unchanged size does not).
- Interactive window (F737): `py -I tools/hud_window_check.py EXE SETTINGS OUT_DIR` starts a silent
  autopilot race in a 1280 × 800 window, resizes the game's own window mid-race (no focus change, no
  keys) and passes when the last `osd_display` line has the new size and `bound=true`;
  `--expect-stale` shows that it catches the defect. It also saves window shots before and after the
  resize through `GRID_WINDOW_SHOTS=S,S,...` (`frame_pacing.rs`, `<data dir>/window-shot-S.png`, game
  real time after loading; interactive window only).
- `race_osd::tests`: aspect range names (`2.2` for 20:9), red-zone angle, speed digits,
  `needle_blur_follows_the_needle_speed`.
- `race_hud::tests` (split pills): `split_rows_show_the_leader_and_three_around_the_player`,
  `split_times_read_as_a_race_clock_then_differences`, `split_pills_show_ten_seconds_and_five_after_a_late_car`,
  `a_completed_lap_holds_its_time_and_the_record_pill_for_five_seconds`.
- Interactive split pills (F763): `hud_window_check.py ... --expect-splits` also requires an
  `osd_splits` line (race_osd.rs, printed when the shown pills change) with a race clock and one
  with a signed difference after the resize.
- Interactive penalty pill and brake light (F770): `--expect-penalty` (pro touge) sets
  `GRID_SMOKE_CONTACT` (one player contact counted through the normal penalty path S seconds after
  GO, `--contact-at`, default 30) and requires an `osd_penalty` line; `--expect-brake` (a circuit
  with class tables) requires `speed_advice ... loaded=true` and counts `osd_brake` lines (the
  light starting to flash).
- Offscreen renders (Donington, 11 AI, autopilot, tick 1500): `r3-16x9.png` 1920 × 1080,
  `r4-20x9-touch.png` 2400 × 1080 with touch zones; inspected: texts, dial, red zone, minimap and
  markers in place; with touch zones nothing overlaps a zone.

### 6.4 Original HUD logic from static analysis (F763)

Read from the unpacked executable with `tools/re/grid_re.py` (no live runs). Addresses are in
`docs/re/symbols.tsv`.

Split pills (`playerspositioninterface`) [D]:
- Trigger: 0x6BBB70 starts the display when the player car passes a split point (each sector end
  and the lap end, the same events the lap timer reports).
- Rows: 0x6BB9E0 picks four rows: with four or fewer cars, or the player first or second, the top
  four; otherwise the leader plus three consecutive places around the player (the player and the two
  cars ahead when the player is last, else the car ahead, the player and the car behind).
- Texts: 0x69F010 fills the rows in place order. Each car shows a time only once it has passed
  that split itself. The first row with a time shows that car's split time as a race clock
  (0x69EA10, `mm:ss.hh`), every later row its signed difference to that time (0x69EAB0, `+`/`-`).
- Late cars: 0x6A6A00 fills a car whose time arrives while the pills are up and raises the remaining
  time to at least `minduration`.
- Durations: the constructor 0x6B5240 defaults to 5 / 2 s, the config overrides them with
  `maxduration` 10 and `minduration` 5; 0x6AC650 copies them into the controller. 0x6B94C0 hides the
  pills when the time runs out.
- Pill flip: `playerN` `in` (0 to 0.333 s) turns the pill about X from -90 through 0 to +90
  degrees. The `playersposition` attach (factory vtable 0xCEBF24, object vtable 0xCEBEA0) binds
  `object/position_pill=N` (0x69F460), `splittimeN` (0x69F4E0) and the row texts (0x6A7050), and its
  reset 0x6A7180 selects `in` and seeks to fraction 0.5 (0x8D2AA0, 0x8D5820), so the pill rests face
  on at the range's middle [D]. The port flips a pill in over the first half when it appears and out
  over the second half when it goes [I]; playing the whole range left the pills edge-on (F763,
  found in the window shots).
- Pill colour: `object/position_split=N` (bound at 0x69F560) has no motion of its own; its ranges
  `normal`, `player` and `first` (each with a following `end*` range) key the `drift_combo_pill` and
  `split_pill_text` DiffuseColour and AlphaScaler, like `object/position_pill=N` does for the name
  pill. Each split range fades the pill's alpha in from 0 (to 0.55 normal, 0.9 player, 0.8 leader;
  colours grey, black, orange), so the port plays the row's range once and rests at its end, while
  the name pill is held at its range [I]; held at a range's start, the split pill stayed invisible
  (F763 window shots).
- Port: `race_hud::SplitTimes` records every car's split race times from the lap timers (player)
  and the opponents' `lap_events`, `SplitDisplay` keeps the 10 / 5 s timer, `position` builds the
  rows. Earlier the port showed the car ahead, the player and the car behind for 5 s after each lap
  with distance gaps (AUTHORED).

Needle blur [D]: 0x69D100 binds `needle_blur_up` and `needle_blur_down`; 0x69CFC0 runs every frame:
a smoothed copy of the needle moves 0.4 of the way to it each frame (the port: per 1/30 s, AUTHORED, so the blur does not depend on the frame rate), the lead of the needle over the
copy, scaled by 30, drives the up blur, the negative lead the down blur, each clamped to 0..1 with
dead zones under 0.01 and over 0.99. Port: `race_osd::needle_blur`, seeking each blur animation to
that fraction of its duration.

Excessive-speed brake light (`excessivespeedindicator threshold 10 lookahead 2`), in 0x6BA460 [D
reading, I meaning]:
- severity = (speed - recommended) / threshold, capped at 1, where the recommended speed is read
  `speed × 1 s + lookahead` metres ahead along the lap;
- the light flashes on a 0.3 s cycle and stays steady below severity 0.5.
- The recommended speed comes from 0x99B9F0, a byte-quantised speed table per vehicle class (vehicle
  manager +0x4B0), linearly interpolated between 1 m cells. It is not built at run time: each route
  ships it in `reducedmechanics.jpk` (the "reduced mechanics" simplified car model), loaded by the
  "rm track" resource callback 0x5F68E0 [D]:
  - `track.rm`: 17-byte nodes about 1 m apart (centre as three 24-bit big-endian values, minus
    0x800000, × 0.005 m; signed half-extents in bytes 9-11; byte 12 / 255 a lateral fraction; bytes
    13-14 / 127 a unit vector) [D 0x9C95D0; V on Donington and Washington]. The lap length is the
    node count − 1 plus the closing gap.
  - `<class>.rm`, one per `ai_vehicle_type_track` class (touring, gt1, lmp1 …): a big-endian float
    scale, two 256-entry acceleration tables (512 floats), then two bytes per node, a full-pace and a
    slower profile [D 0x99B6D0; V 13 classes]. Speed = byte × scale [I: m/s].
  - The HUD (0x99B9F0) reads the player's class table, full-pace profile, at the player's distance
    along the node chain plus the lookahead (0x9C78D0 projects each car onto the chain) [D]. The
    same table, blended toward the slower profile, drives the Virtual_Performance path and the
    spawn speed 0x915170 (20 m/s without a table).
  - Without the player's class file the recommendation is FLT_MAX and the severity stays 0 [D].
  - Projection [D 0x9C78D0, 0x9C7550]: each segment's closest point (3D, clamped), distance = (node
    + fraction) × spacing, wrapped into the lap; a local search of the segments within 2|v|dt + 3 m
    of the last fix; farther than 60 m from the chain a coarse scan of every 30th node (every node
    for the first fix). The 3600 m² test also needs a field +0x18 > 60 whose meaning is open; the port
    rescans whenever the local search is farther than 60 m.
  - Light [D 0x6BA460]: the severity goes to the `brake_light` animation (+0x258) through the
    animation's slot 0x2C (command vtable 0xCEC6AC, execute 0x6C01A0); the node (+0x260) is shown
    steadily up to `startflashSeverity` 0.5, above it a timer (+0x2AC) counts down from
    `flashinterval` 0.3 s and the light is off while the timer is at or above `lightoninterval` 0.7 ×
    0.3 s (show/hide command 0x6C0FD0). Haruna's routes ship only `dummy.rm`, so touge and drift
    events show no warning.
  Port: `speed_advice` (grid-app) reads `track.rm` and the player's `<ai_vehicle_type_track>.rm`
  at race start (log line `speed_advice ... loaded=`), locates the player each frame and sets the
  severity; `race_osd::brake_light` seeks that fraction of the `brake_light` animation [I: what slot
  0x2C does with the value is not traced; the needle blur seeks a fraction the same way] and flashes
  the node. Speed is the velocity's magnitude [I: the original reads car+0x1648]. Unit tests on a
  synthetic pack; an ignored test checks every route with a touring table against its AI line.

Minimap [D]: a camera view of the track drawn by the `rd4minimap` attach.
- Values: the HUD (0x6BA460) reads the DB `track` columns `minimap_zoom`, `minimap_dist_behind`,
  `minimap_dist_above` (Donington 200/116/110) and sends them through 0x6C0F80/0x6C01C0; the execute
  0x6C1AB0 calls 0x6A38E0, which stores zoom +0x28, behind +0x5c, above +0x60 and a not-drift flag
  +0x64 in the controller.
- Setup: 0x6B1260 calls 0x6C0680, which fills the defaults: FOV 60 (+0x24), zoom 500, target height
  10, track half-width 0.03·zoom, track colour ARGB 0xEFD2D2D2, highlight 0xFF414141 alpha 202 (only
  with a highlight index), start/end colour 0xFF212121, player size 0.13·zoom, opponent size
  0.1513·zoom, start/end sizes 0.0742/0.0099·zoom, behind 116, above 110. 0x6AB8D0 copies the block
  into the controller and builds `minimap_database`/`minimap_root` (near 0.1, far 10000),
  `minimap_alphacamera` (−1..1) and the render instances (opponent, player, track 0x6A3DF0,
  start/end, final); 0x6A3490 creates the render targets.
- Camera 0x6B28A0: the car's height is ignored. With f the flat unit forward, eye = (x − f.x·behind,
  above, z − f.z·behind), target = (x, 10, z), up Y (lookAt 0x5E3330). Then it draws the track
  (0x6B2330 → 0x6B1610), start/end (0x6A4D10, only with the +0x64 flag), opponents (0x6AB520,
  billboards on the camera axes, size·zoom) and the player (0x6AB1A0).
- Ribbon 0x6B1610: per route node (closed routes add a closing pair) two vertices at
  node ± side·w with y 0, w = 0.03·zoom (Donington 6 m each side), vertex colour from the ARGB.
- Port: `race_osd::MapView` reads the three columns through `track_model` (route key, venue
  fallback; defaults 500/116/110 otherwise) and builds the same view and a 60° projection; the
  ribbon runs along the AI line [I: the original's node list is the route; the AI line follows it],
  clipped to the near plane and to a 64-gon of the ring (AUTHORED), in the 0xEFD2D2D2 colour. The
  arrow keeps its dial2 size and stays upright; opponent discs keep theirs, scaled with the inverse
  depth (AUTHORED: the original draws them at 0.1513·zoom world size).
- `minimap_file_string` [I: a per-track model; its loader is not traced, and the camera draw does
  not use it].

Post-race cutscene (`cutscene/postrace.xml`) [D]: damage off, the AI takes over with a RaceOver
cruise at 40 then 30, the OSD off. Porting it needs an AI cruise behaviour in the simulation.

Lap time hold and PB/WR pill (the lap interface, constructor 0x6B3950, vtable 0xCED0C8) [D]:
- 0x6B85C0 runs every frame: when the player's completed lap count grows, the LAP timer shows the
  lap just completed (0x6B3C90) and a 5 s timer starts; when the count drops (a restart) the timer
  is cleared; while the timer is out it shows the running lap (0x6B3C00).
- The constructor subscribes to `BroadcastMessage_NewPersonalBestMessage` and
  `..._NewWorldRecordMessage`, which only raise a PB or WR flag (0x69E1E0, 0x69E1D0).
- 0x6B3AF0 binds `object/pb_wr=lap_record` and `pb_wr_lap_text` (in dial2.pssg the pill's `multiplier_text!0`); 0x6B7A90 writes `osd_pb` (or
  `osd_wr`, which wins) into the text and plays `on`; once the 5 s timer is out it calls 0x6B2D00,
  which plays `off`. The `best__record` and `race_record` pills are not bound by any code.
- World record [D]: the lap-record object (vtable 0xCE115C, built by the race setup near 0x67A976
  through 0x5D0BE0) checks each completed lap in 0x5C3890: PB through 0x5C36C0 against its PB lap
  (+0x3c), WR through 0x5C3790 against its WR lap (+0x38), which posts `NewWorldRecordMessage`
  (vtable 0xCB154C). It then loads the record to beat: the `world_record` row of the track model
  whose `vehicle_grade_id` equals the player vehicle's. Not traced: where the PB and WR laps come
  from (a saved profile or ghost file, `ghostlaps/%s%d.xml` [I]).
- Port: `race_hud::LapHold`. The PB source is the session best [I: what the original compares
  against is not traced]. A new best under `race_hud::WorldRecord` (that DB row for the player's
  car) or under a lap that beat it earlier in the session shows `osd_wr` (F777);
  a beaten record is not saved [I]. Before, the port showed `osd_pb` on the
  `best__record` pill for 5 s and never held the completed lap time (AUTHORED).

OSD timer format [D]: the LAP and RACE texts are written with `%02u:%02u.%02u` (0x69E050 for
the lap, 0x6B3D30 for the race): whole minutes (hours included), seconds and truncated
hundredths, capped at 1:39:59.999, so `99:59.99`. Port: `race_hud::osd_clock`, used for LAP,
RACE and BEST [I for BEST, whose writer is not traced]; the empty text `--:--.--` is AUTHORED.
Before, the port wrote `0:05.600` (AUTHORED).

Results list formats [D]: the list fills `list.%d.anim.content.time` with 0x4C8200, which writes
the Le Mans results format (0x908860, `grid_tools::race::format_lemans_time`), and
`list.%d.anim.content.difference` with 0x4C8280: `+%02u.%02u` under a minute, else
`+%02u:%02u.%02u`, truncated hundredths, a blank for the first row (callers 0x540373, 0x542037,
0x54225A; the Touge stats' difference 0x51EB72 uses it too). Port: `post_race::time_text` writes
the winner's time and the others' differences in these formats; the original gives every row
both a time and a difference, the port's one Time column keeps its AUTHORED layout. Before, it
wrote `0:05.600` and `+3.072` (AUTHORED).

Penalty pill (touge only) [D unless marked]:
- 0x6B3380 binds `header_text`, `penalty_text` and the anim `object/penalty_pill=penalty` of the
  HUD widget at HUD+0x6CC (ctor 0x6A5ED0, vtable 0xCEC820, no per-frame update) and hides it; the HUD
  setup hands it to the touge state (0x631130, touge+0xEC) for every race.
- The touge tick 0x64E1E0 counts a pending penalty when the contact window (2 s) ends. For the
  player's car only it calls 0x6B3500 (minor, 2 s) or 0x6B3630 (major, 10 s): the header gets
  `osd_minor` / `osd_major`, the text `"+%d"` of the seconds; the pill shows and plays `small`
  (0-1.5 s) or `big` (1.833-3.333 s) from its start, and the range end hides it (anim flags 3).
  Penalties of the AI car count but are not shown. Reset 0x6B3410 writes `<penalty>` and hides it.
- Port: `touge_event::TougeEvent::pill` records the player's newly counted penalty;
  `race_osd::penalty_pill` sets both texts, plays the range and hides the pill when it ends.

Wreck-screen delay [D]: `StateTerminalDamageDelay` (vtable 0xCDA548; 0x5A5990 is only a shared
base-class stub) ends by itself. Enter 0x57F140 zeroes its timers. Update 0x57F1A0 waits for the
player's car to rest (angular speed ≤ 0.1, linear ≤ 0.5 [I units: rad/s, m/s]), giving up 2 s
after entering the state, then counts 1 s. At the rest moment, before the 1 s, it sets the
camera director's TERMINAL flag (director ctx+0x2254 → +0x14, field +0xD20 = 1) and calls
0x92D110(1, 0), 0x92D400(0) and 0x92D450 [D]; the flag stays set through the crash replay (REPLAY.md
§6). Its link check 0x58E2E0 returns "done" then (or at once
when `FinalBoundary` 0x5EBC90 disqualified the car). Its clock is the replay controller's step
(0x944430). Exit 0x57F170 stores the state's total time in 0xE6DFF8, which the terminal replay's
rewind (0x596C90) adds. Afterwards `flow.bin` plays the crash replay
(`instant_race_replay_silent`) and the wreck menu comes on its exit (action 0x20). Port:
`post_race::on_wreck` waits the two transitions, the rest (at most 2 s) and 1 s; `post_race::update`
then plays the terminal replay (`Replay::request_terminal`) and the wreck menu shows on its exit
(round 5; before it the port showed the menu first). During the two transitions the image blends IG_whiteout →
TerminalFX_01 → the ordinary image and the game runs in slow motion (docs/FLASHBACK.md §2.5).

Two fields of the OSD owner select the layouts below: F = owner+0x1FC, the competition-format
enum of DISCIPLINES.md 1.1 [V: the drift handbrake hint shows only for F 2..4, and the stacker
dispatch 0x6BB0C0 jumps on F 0..8 in the same groups], and T = owner+0x1AC (values 1..4, meaning
open).

Driver name tags [D: bind 0x6B0540, update 0x6BF070]:
- The update runs while +0x2A8 is set and the game mode is not 2; its first frame hides every tag.
- T 2, normal path (0x6BD0B0): tag 0 (`object/driver_id=driver_id_01`) shows the rival;
  `driver_id_team` shows the team-mate (0x6AAB90). A rival who is the team-mate is dropped; no
  rival and no team-mate hide the tag. The rival [D]:
  - only in career mode (`[game+0x23D0]+0x50` = 0) with an event object E (mode object 0x640020,
    its virtual 0x80) whose kind E+0x44 is 0, the player's own event (not an offer);
  - E's virtual 0x2C (0x67CF00) returns E+0x34, the race index within the event;
  - when the car count (+0x280) changes, 0x6BD0B0 calls 0x6B0C40, the team-mate lookup 0x6AAB90 and
    the rival lookup 0x6BCE60. 0x6BCE60 clears +0x284; for a race index > 0 it builds the event
    standing (0x66FF70) and takes its entry 0 (0x6C2D30, 20-byte entries), the championship leader.
    Unless that driver is the player (+0x1C = 2) it finds the race car with the same driver (the
    provider's slot 0x2C count and slot 0x30 get, the driver at vtable +0x58) and stores it in
    +0x284;
  - for race index 0 with more than two cars the rival is chosen every frame: provider slot 0x40(0),
    the car in race position 1, or slot 0x40(1) when that car is the player.
- T 2 with a driver offer of objective type 2 (0x6BD480, 0x6BD6B0): up to seven cars of the named
  team, nearest first, on tags 0..6.
- T 3 (0x6BD900; probably online, it also drives the chat box [I]): the two nearest cars on one
  side and the nearest on the other; the `wrongway` tags when the car's byte +0x1550 is set. Skipped
  for F 3 and 9; the distance is Euclidean for F 4, else signed along the track (0x90BDB0).
- Limits (0x6A80A0): hidden beyond 60 m (0xDB4F60); alpha = base − min(|d|/60, 1)^3 × (base −
  [0xE6DFF4]): the fade range 60 (0xDB4F5C), the exponent 3.0 (0xDB4F68), and the minimum
  [0xE6DFF4] is in .bss with no writer found, so 0 [I]. The alpha goes to parameter 2 of seven
  parameter tables at +0x120 (stride 0x1C), one per tag part; the bases at +0x210 are the parts'
  authored material alphas [I]. Text: the position ("%d", 0xCB86FC), the name (32 characters) and
  the nationality flag: `driver.nationality_id` → `nationality.flag_id` (0x69C860), shown as the
  `flag` part's scrollX = (n / 9) / 7 and scrollY = (n mod 9) / 9 (0x6A0820, parameters 0 and 1);
  −1 or a cell outside 0..1 hides the flag. Anchor (0x6B0820): the car position raised by
  1.5 + 0.04 |d| m, projected; behind the camera it goes off screen, with no edge clamp. The tag
  plays the "expand" range (`driver_id_team`'s range also animates the name pill's colour).
- Open: what T and game mode 2 are, the sources of the parameters +0x2B0/+0x2B4.
- Port (name_tag.rs, F782): the team-mate's tag in a race with `[game] teammate` (the career
  field's team-mate, F771). d is the signed distance along the AI racing line (the route's
  `reset_line`, wrapped the short way round a closed circuit), |d| for the fade and the lift;
  Euclidean without a line. The lift is along world up [I: the original's up vector is not
  traced]. The anchor is projected with the 3D camera and placed where the OSD camera's ray
  through the same screen point meets the tag group's plane (its authored depth −20 under the
  layout). Each part gets its own material copy for the faded alpha and the flag cell
  (`Osd::fade`, `Osd::scroll_x`/`scroll_y`).
- Port, rival (name_tag.rs, F785): the race takes `[game] rival = leader | K`
  (SETTINGS.md). `leader` is the first-race rule (the race leader, the second car when the player
  leads, more than two cars); K is opponent K, the career's standings leader (`RivalRule`;
  `RivalRule::for_event_race` is the career-side rule). The rival's flag comes from its driver id
  (one database read at race start). Both tags share the placement, fade and flag code; logs
  `osd_tag ... tag=rival`. The career writer (`career_screen.rs`, career-ui's file) is requested,
  not part of this change; until it lands, a race has a rival only when its settings name one.

Boost [D: bind 0x8EFA60, dial start 0x8EFD50]: the only reader copies the player vehicle's +0xDE4
block into a flag and shows or hides the `boost` animation (0x6C1030, 0x6C0050, 0x8CFE00); no value
is displayed. Vehicle+0xDE4 is a per-car feature loaded from properties byte 0x4D7, tuning CSV
column 153 (`vehicle_properties.rs`), which is 0 in all 45 car CSVs of the install [V], so the
indicator never shows with stock data. The port keeps it hidden (and vehicle preparation rejects a
nonzero value).

Stacker re-flow [D]: the attach type "stacker" (0xD0A974; constructor 0x8D52D0, vtable 0xD0AB20;
factory probably 0x8D5370 [I]). Build 0x8D3730 collects up to 32 children with their rest
translations, sorts them by rest y (top first, 0x8D0620), gives each the authored y gap to the
next child (the last reuses the previous gap, 0x8D0670) and starts the cursor at the top child.
Reset 0x6C1510 (0x8D35D0) puts every child back at rest; Append(name) 0x6C1570 (0x8D3670) sets the
child's y to the cursor and advances the cursor by its gap. Only y moves, at once. Groups not
appended keep their rest position; visibility is separate (0x8CE5F0). The lap-time stacker
(+0xB0C, bound by 0x6B7800) is laid out once (0x6BC3E0 → 0x6BB0C0) by F:
- F 0/1 (0x6B7D10): T 1 race, lap; T 2/3 race (+ lap) when provider slot 0x24 is true, else main,
  race (+ lap, best), the bracketed parts dropped when byte [+0x7C8]+0x655 is set; T 4 lap, race,
  best;
- F 2-4 drift (0x6B7BF0): T 2/3 only; race for F 4 with slot 0x24, else main;
- F 5/6 touge (0x6B8400): T 1 race, lap; T 2 main (not for F 6), race;
- F 7/8 offers (0x6B80F0): main, a group whose name the decompile hides, best and offers.
The position stacker (0x6B4E00, 0x6B4F90) appends `position_group` (T 2/3 with more than one car,
or T 2) and `split_group`. Not ported: which layout a port event gets depends on T and slot 0x24.
T's source [D]: 0x5F4C70 writes T (data+0x1AC) and F (data+0x1FC) from M = [[game+0x234C]+0x80],
a field of the race-setup object (constructor 0x5EA0D0): M 0 or 2 → T 2, M 3 → T 3, M 4 → T 4,
any other → T 1.
M's writer [D] (static trace, F810): the only store to race setup +0x80 is 0x611F30, which runs
once per race-setup object (its flag +8). It finds the setup through the type key 0xE5EBEC (the
setup registers itself there, 0x4CE0A0) and also calls 0x60BEF0 with M. Its one caller 0x582BF0
(a virtual, .rdata 0xCD8D38) picks M = 2, or 4 when the game mode `[game+0x23D0]+0x50` is 4 (test
drive), or 3 when the online session `[[[x+0x1048]+0x1058]+0x1CC]` (0x6D9B20, as in 0x6572D0 and
0x6F9D70) exists and its flag (session+0x10)+0x30 (0x8D9480) is set. The constructor 0x5EA0D0 does
not set +0x80, and no other function that loads `game+0x234C`, uses the type key or is a setup
virtual stores there. So T is 2 for every offline race, 3 online and 4 in a test drive; T 1 is not
reached through this writer. [I] The online flag means an active online session; not traced
further.

Lap-time layouts for T 2 (every offline race) [D, F814]. The layout first hides every group
(0x6B2D80), then appends elements; an element (classes 0x6B3760, vtables 0xCEC82C..0xCEC850)
binds a slot's `{slot}_group`, `{slot}_title` and `{slot}_time` and shows the group through
0x8D4670 (hide 0x8D46A0). Provider slot 0x24 (index 9) is the time limit: 0x80E5E0 is true only
for the time-limited race type 3 (Le Mans) and gives the time left, max(24 h - race time x clock
scale, 0). Elements:
- lap counter (+0x218) in `main_group`;
- race time (+0x2A0, activate 0x6B3840) titled `osd_time` (TIME, not `osd_race`);
- remaining time (+0x570, activate 0x6B40A0, update 0x6B4110) titled `osd_time`, hours:minutes
  rounded up (0x69DF40: hours 0x9085C0, minutes mod 60 0x9085F0, +1 minute when seconds 0x908640
  are left, carrying into the hours), or minutes:seconds (0x69DF90) in mode 4 (drift freestyle);
- lap time (+0x340), with the `lap_record` pill;
- best lap (+0x400, activate 0x6B3E80, update 0x6B89C0) titled `osd_best` (`osd_world` with
  +0xB2). When the record index changes to a time earlier than the cached one (0x9084C0 is the
  unsigned 64-bit microsecond "less than") it writes the time and shows the row; with flag +0xA0
  set the row hides again after 5.0 s (0xCED5C8), else it stays. Activate caches index count - 1
  and the largest time (0xE6E160), so the first best lap counts.
The layouts, top down: F 0/1 with a time limit: remaining time in `race_group`, best (popup) in
`lap_group`; F 0/1 without: lap counter, race time, lap time, best (popup); F 2/3: lap counter;
F 4: remaining time (minutes:seconds) with a time limit, else nothing; F 5: lap counter, race
time; F 6: race time. Byte [+0x7C8]+0x655 drops the lap and best rows; its meaning was not traced
and the port takes it as 0 [I].

Ported (F814): `race_osd::lap_layout` (the table above), `race_osd::reflow` (build and append on
the rest y read from dial2 by `Osd::children`/`Osd::rest`), `hours_minutes`/`minutes_seconds`
and `BestRow` (the popup). `race_hud::LapStack` gives the format (drift event's format, touge
event, else race) and the remaining time (the Le Mans clock scale). The port has no midnight
touge or online and test-drive layouts (T 3/4). Each layout change prints `osd_stack groups=...`.
[I] the race row shows only once the race clock runs (the original's update shows it from
activate; not traced whether the value is blank before the start).

Not traced yet: the post-race art.

### 6.5 Drift OSD

The original drives three groups from the player's drift scorer (`grid_tools::drift::Scorer`,
field offsets as in that file). The race OSD's per-frame bridge 0x6BB130 runs while the scorer
exists and an OSD flag is set [D; who sets the flag is not traced, I: the drift formats F 2..4].
It posts small commands that the combo attach executes later; the port calls them in the same
order (`drift_osd::DriftOsd::update`).

- Meter (`object/drift_main=drift`, bind 0x6B48D0, update 0x6B8D60) [D]: the signed slip value
  (scorer +0x50, −1..1) is smoothed by s += (x − s)·0.3 per frame; `car` seeks to p = clamp((s+1)/2),
  `meter_left` to clamp(1 − 2p), `meter_right` to clamp(2p − 1). A side swap after a value above
  0.25 resets `meter_max_left/right` (0x6B4AA0), as does a new combo. The port applies the 0.3 at
  a 60 Hz reference, 1 − 0.7^(60·dt), so the meter does not depend on the frame rate [I]. The
  raise of the max markers (0x6B4C80/0x6B4B90) is not decompiled; the port keeps the highest bar.
- Points box (`object/points_main=points`, bind 0x6BB650) [D]: titles get `osd_points`, the
  total starts at "0" and `points_anim` plays `out`. A new combo plays `in`, hides
  `multiplier_bag` and writes "X1"; each multiplier change writes "X%d" into `multiplier_text`
  (0x6B4860). A bank writes the banked total with the locale's digit grouping (0x6B4700,
  0x6A63B0) into `points_total` and its echo and plays `multiplier_bag` `on` (0x6B4570); a fail
  plays `points_anim` `fail` (0x6B4680). The original also counts the shown value up to the combo
  points (+0x2F8..+0x304); that tick is not traced and the port writes the total at the bank.
- Combo panel (`object/driftcombo_main=drift_combo`, the `drifteffect` attach, vtable 0xCEBF3C,
  bind 0x6B5650, update 0x6B9640) [D]:
  - open (0x6B9730) when the combo multiplier (+0x18) becomes non-zero: the slots reset and
    `driftcombo_anim` plays `on`;
  - multiplier 2..99 (0x6B6040): the two slots `multiplier_1/2` alternate, the old one plays
    `off`, the new one gets the digits and plays `on`;
  - digits (0x6A76E0, the `combodigit` attach): below 10 the double stash (an "X" glyph and one
    digit) shows and the treble stash hides, from 10 the reverse (tens and units). A digit is not
    text: each `text_single`/`text_echo` glyph is a strip of ten, and the attach (vtable 0xCEC06C,
    create 0x6A7AF0, parameter names `scrollX`, `scrollY` bound by 0x69FF10) sets the node's own
    `scrollY` to digit·0.1·0.91 (0x6A7AC0 → 0x8CEBC0). The port adds `Osd::scroll_y`, a per-node
    copy of the scene material that keeps following its animated values;
  - drift type (0x6B5C60): when the rating (+0x20) changes to r > 0 the name of rating r−1 goes
    into the other of `drift_type_1/2` (text and echo) with `off`/`on`; while r ≠ 0 the current
    drift points (+0x10) show as "%d" in `points_text` (0x6C1B70);
  - ring (0x6B99D0, 0x6B5EF0): f = combo timer / wait (0x63C580). Full while drifting; below 1 one
    of the eight `timer_a/b` segments goes `off` every wait/8 s, from 8 down to 1, so ceil(8f) stay
    lit. f reaching 0 banks (0x6B9850): the slots play `off`, the panel exits and hides after
    2 s. A fail (+0x4C) plays `drop` (0x6B9910), also hidden after 2 s;
  - close-drift bonus (0x6B5D10): when a close drift ends, `bonus` (under `redflag`) shows the
    bonus "%d" or `osd_perfect` for a zero distance and plays `bonus`; after 1 s `bonus_off`.
- Ring shake [D]: the shaker attach (constructor 0x6ACAB0: amplitude +0xEC = 8.0, threshold
  +0xF0 = 0.5, generator seed 0x5A5AA5A5) offsets the ring by a random step each frame while
  f < 0.5 (0x6A77E0; generator x = x·0x41C64E6D + 0x40584C0F) and puts it back at rest when f
  rises again or the ring banks (stop 0x6A72D0). Port: `drift_osd` `shake`/`stop_shake`, the offset
  through `osd_scene::Osd::offset`.
- Points roll-up [D: vtable 0xCECBF0 group {0x7157D0, update 0x6B8C50, reset 0x6B8CE0}]: the
  update calls the mode functions (0x6B4410, 0x6B44C0, 0x6B4570, 0x6B4680), then the tick
  0x6B4790(dt): shown = from + (to − from)·t, t += dt, and at t ≥ 1 (0xCAF888) shown = to. The
  grouped text (0x6A63B0) goes to `numbers` and `points_total_echo`; a new total (0x6B4700) starts
  a new roll from the shown value; the resets 0x6B8C10 and 0x6B8CE0 roll back to 0. Port:
  `drift_osd` `roll`, one second per roll.
- Not ported: `drift_message` (attach 0x6A8620, its feeder not traced) and the battle position
  multiplier, which the plain text panel showed.

### 6.6 Original post-race art (row 69)

Static research only (F782). Sources: `system/states.bin` and `system/flow.bin` decoded with
`tools/validate_binxml.py`'s decoder (private copies `local/postrace/states.xml`, `flow.xml`),
`frontend/fePersistentDatabase.pssg`, `postprocess/effects.xml`.

Screens and their glyphs [D, states.bin]:

| Screen id | Class | Glyph |
|---|---|---|
| `post_race_stats_free_race` (and `_touge`, `_do_timetrial`, `_do_speedtest`, `_drift_*`) | `ScreenPostRaceStats` | `post_race_stats` |
| `post_race_results_generic` (and `_dd`, `_lemans`, `_drift`, `_touge_*`) | `ScreenPostRaceResults*` | `leaderboards` |
| `post_race_drift_scoreboard` | `ScreenPostRaceDriftScoreboard` | `drift_scoreboard` |
| `post_race_menu` | `ScreenPostRacePlaceholder` | `menu` |

- The glyph roots live in `frontend/fePersistentDatabase.pssg` (raw name counts: `post_race_stats`
  198, `leaderboards` 117, `drift_scoreboard` 90) [D]; `feNonPersistentDatabase.pssg` and
  `fe_DLC_0.pssg` reuse the `leaderboards` name for their own screens.
- The results screen is a `RaceResultsAnimatedList` (axis y, 12 rows, cursor `cursor`), the pills
  `pills.0..3` (Continue, View Replay, Exit, Restart Race, text glyph `pill_header`), and the texts
  `title_1` and `table_title`; `animSpeed` 0.5 [D].
- The stats screen lists `stat` items (free race: `position`, `fastest_lap`) and plays
  `second_replay_cam="replay_roof1"` behind it [D].
- Behind the screens [D]: `post_race_increase_blur` blends the post effect to `fepause_final`
  (effects.xml id 17) over 2.0 s, the same effect as the pause menu (0.3 s there);
  `results_screen_camera_instant` moves the camera 1000 back and 215 up (`StatePostRaceMoveCamera`,
  `post_race_stats_cam`); the stats screen runs the `stats_funky_camera` (`replay_roof2`, after 4 s,
  3 s fade).

Blockers for a port [V/D]:
- Fixed in this change: the OSD reader (`grid_formats::osd::read_scene`) used to stop on
  `fePersistentDatabase.pssg` at 0xCEA16, a `uint_color_argb` vertex stream. It now decodes every
  colour stream into `Mesh::colors_argb` (one ARGB word per vertex, the shared `mesh::color_words`
  decoder; a count that does not match the positions is an error). All three front-end packages
  read [V]: fePersistentDatabase 2014 nodes, 920 meshes, 19 coloured; feNonPersistentDatabase 96
  coloured; fe_DLC_0 2 coloured.
- feNonPersistentDatabase's animations also bind 284 channels in another package, for example
  `channel="fePersistentDatabase.pssg#DiffuseColour!2"` on `matinst=pill_basic#DiffuseColour`
  (DiffuseColour 98, RampMin 96, Alpha 47, GlowAmount 42, SpecularLevel 1; all into
  fePersistentDatabase) [V]. The reader keeps them as `Animation::external` instead of failing; the
  other two packages have none. Its cameras also animate a `lookAt` semantic (the channel names the
  camera and its `#target=` node, e.g. `#camera=zone_01-04#target=zone_01-04`); the reader keeps
  it as `Semantic::LookAt`. All three packages read in full [V]: fePersistentDatabase 2014 nodes,
  feNonPersistentDatabase 2478 nodes (775 meshes), fe_DLC_0 1052 nodes (350 meshes).
- The front-end glyphs use another shader family [D, effect names in the files]:
  `fePersistentDatabase.pssg` references `fe3d_generic` (66), `feov_colour` (26),
  `fe3d_genericGMPD` (24), `fe3d_genericGD`, `fe3d_ramp`, `fe3d_genericGPD`, `fe3d_genericI`,
  `fe3d_tracksurface`, `fe3d_particle`, `fe3d_video` …, mapped to instanced `fei3d_*` effects by
  `frontend/fe_shadersetup.xml` (parameters `DiffuseColour`, `Alpha`, `GlowAmount`, `Scale`,
  `RampMin/Max`). `Osd/dial2.pssg` uses only `osd3d_*`/`osd2d_font`, which `osd.wgsl` implements.
  The port has no `fe3d_*` shader yet (the loading world, F773, waits for the same renderer).
- `fepause_final` is shared with the pause menu, which the front-end session owns.
- The post-race cutscene needs an AI RaceOver cruise (above).

Glyph trees [V, dumped with the reader: `GRID_OSD_DUMP=1` on the ignored test
`front_end_scenes_read_their_vertex_colours`; private copy `local/postrace/glyph-trees.txt`]:
- A glyph is defined once under a root node `<glyph>_root`; a node `kind/glyph=instance` (kinds
  `screen`, `gadget`, `object`, `component`, `kitpart`, …; e.g. `gadget/list_items=0`) instances it
  and has no children in the file. Drawing a screen means grafting the glyph's `_root` subtree under
  each instance, recursively [I: the grafting is inferred from the names and the empty instance
  nodes; the original's instancing code is not traced].
- `leaderboards_root` (the results screen; the same tree in fe_DLC_0): 19 nodes, 7 meshes, effects
  `fe3d_generic` 3, `fe3d_genericGD` 3, `fe3d_genericGPD` 1; texts `title_0`, `title_1`,
  `table_title`, `timer`; instances `gadget/list_items` ×2, `gadget/list_titles`,
  `object/leaderboard_bg`, `object/h_pill_group`.
- `post_race_stats_root`: 4 nodes, only the instances `component/post_race_stats_chunky` and
  `component/post_race_stats_normal` ×2.
- `drift_scoreboard_root`: 21 nodes, 7 meshes (`fe3d_genericGD` 5, `fe3d_genericGPD` 2), instances
  `gadget/drift_list_items` ×2, `gadget/drift_list_titles`, `component/drift_cursor_half` ×6,
  `component/drift_cursor_total`, `object/h_pill_group`.
- The instances `screen/post_race_stats` and `screen/pause_leaderboards` sit in `pause_root`
  (`helper=anim!3`), the pause menu's tree.

Grafting, ported (`grid_formats::osd_glyph`, F787): `expand(scenes, scene, root)` walks the tree
below `root` and, at every childless instance, walks its glyph's definition under the instance's
world matrix. The definition is looked up in the instancing package first, then in the other loaded
packages; the definition root's own transform is not applied (identity in every traced glyph [I]);
an instance whose glyph is undefined, or that would graft a glyph into itself, stays a leaf; at
most `MAX_DEPTH` = 16 nested instances. Each placed node carries its package, index, world matrix
and instance depth. With all three front-end packages loaded [V, ignored test
`post_race_glyphs_expand_through_the_front_end_packages`], every instance below the post-race
roots resolves:

| root | placed nodes | own meshes | grafted meshes | deepest |
|---|---|---|---|---|
| `leaderboards_root` | 471 | 7 | 319 | 3 |
| `post_race_stats_root` | 22 | 0 | 9 | 1 |
| `drift_scoreboard_root` | 77 | 7 | 40 | 2 |
| `loading_root` (front-end's loading world) | 3779 | not counted | 2434 | not counted |

`loading_root` holds 743 instances, so the loading world, which draws only the nodes stored below
its root (grid-app `fe_world`), lacks the grafted meshes; switching `fe_world` to `expand` is the
front-end session's call (told 2026-11-10).

Next steps: the results screen needs the `fe3d_generic`/`GD`/`GPD` materials (`fe_world` has only the
unlit `fe3d_tracksurface` term), the camera that views `pause_root`'s screens, and its own
`osd_scene`-style driver for the rows, pills and texts. The front-end session ports the pause art
but builds neither the grafting nor these materials (their reply, 2026-11-10).

### 6.7 Front-end materials and the results art (row 69)

Static research and port (F810). The results screen now draws the original's art: the
`screen/pause_leaderboards` glyph of `fePersistentDatabase.pssg`, its glyphs grafted in (§6.6), seen
through the `pause_root` camera, lit by the shared front-end materials and filled from the post-race
results view. Ported in grid-app `fe3d` (materials, shared) and `results_art` (the screen).

Front-end materials (`fe3d`, shared by every screen that draws front-end package nodes) [D]:
- Families by effect name: `fe3d_generic`, `fe3d_genericGD`, `fe3d_genericGPD`, `fe3d_genericI`,
  `fe3d_genericGMPD`; the arithmetic follows the in-game `feig3d_*` programs of `shaderpack.pssg`,
  with the instance streams of `frontend/fe_shadersetup.xml` (`DiffuseColour`, `Alpha`,
  `GlowAmount`). Textures hold raw (gamma-space) values (`Rgba8Unorm`); the shader converts only its
  result for the target.
- Lighting comes from `frontend/fe_paramcontrol.xml`, read as `FE::MaterialLibrary` does: zones
  0x4D04B0, values 0x4D6EE0, defaults 0x4CFA70 (used for every value a zone leaves out), zone change
  ease 0x4D0100, constant registration 0x4D0960. The global constants are built as 0x4DFC00 does,
  with the light directions of 0x4CFE40 and the glow pulse. The results screen uses group `ingame`,
  zone `leaderboards`.
- [I] A zero-extent bounds axis adds nothing to the glow falloff (the original divides by zero);
  no screen-space shadow mask (the zone's third light is black).
- Reflection cubes [I]: the `TR1` slot takes a cube; the writer of the original's front-end
  reflection cube was not found (0x4E2750, the front-end render setup, registers the reflection
  slots; 0x497E70 finds or creates a texture-name slot). The port uses `env_soft`/`env_sharp` of
  `Osd/dial2.pssg` as a stand-in.

Text layouts [D]: `FETEXTLAYOUT` nodes give each text node its font, height, alignment, wrapping,
tracking and case; the enums are numbered as the original's style parsers do (0x47A4F0 horizontal
alignment, 0x47A5B0 vertical alignment, 0x47A660 wrapping, 0x47A800 case). Read into
`grid_formats::osd::Node::text` (`TextLayout`); `Mesh::texcoord1` keeps a second UV stream.

The camera [D/I]: `pause_root` holds the camera `camera=camera1` and its `target=camera1`; the
package's `Anim!96` flies both between the pause screens (the loader 0x58B380 and update 0x583E10
of the pause world; 0x5A5630 builds `MovePauseWorldToCamera`, dispatched by 0x4ED780; 0x4DCDF0
applies the race camera matrix to the helper). [I] The results pose is the first key time from 0
of that animation's `screen/pause_leaderboards` translation at which the screen lies at the
target's distance along the view, within 1 % of it: t ≈ 1.667 s (relative error 0.6 %). Its later
hold (3.333 to 4.667 s, error 0.5 to 1.4 %) brings the screen about 30 % closer, and the art then
reaches NDC y 1.36 on a 16:10 view with the title clipped at the top edge; the first hold keeps it
inside (NDC x −0.81 to 0.87, y −0.68 to 0.94). The least-error key (4.667 s) is the fallback when
no key arrives. Which hold the original shows for the results was not traced. Other three-key
animations that start at a node's saved translation are read as slides; their middle key is the
on-screen place.

Rows [D]: the row filler 0x54D270 instances one row glyph per entry (`list.%d`, state `player`,
`friends` or `deactivate`). The port [I]:
- Twelve visible rows (`RaceResultsAnimatedList`, §6.6), the window centred on the player; row
  `gadget/list_items=0` repeated at the pitch between `list_items=0` and `=1`.
- Text node names pick the column: `position`, `driver`, `time`/`score`, `car`, then the fourth
  column `team`, `class` or `points` by race kind, and `difference` only for the touge's second run.
- The player's row shows the row cursor pill; never drawn: the race timer, the row arrows and
  flags, the pill helper, the frame of a list that does not scroll.
- Titles: `title_0`/`title_1` the view's title, `table_title` its body line.

Drawing [I]: the art is rendered on its own layer (24) by a camera into an image, shown under a 50 %
dim over the race; the post-race panel keeps only its pills (Continue, View Replay, Exit, Restart)
at the screen's foot while the art is shown, and falls back to the full text panel while the
packages load or when they fail to read.

Textures [O]: the screen's pattern, dirt and icon textures live in `frontend/b_persistent.pssg` as
`dxt3` (BC2: 8 bytes of explicit 4-bit alpha, then a four-colour BC1 block). The shared texture
reader (grid-formats `texture`) reads them as `Format::Bc2` (DDS FourCC `DXT3`) and every RGBA8
decode path (`texture_upload::decode_format_levels`, the BC fallback) decodes them, so any package
that ships `dxt3` loads through the same path; before, the bundle failed to read.

Checks: `tools/results_window_check.py` runs a 1-lap autopilot race in a real window and saves the
results screen (`[diagnostics] results_shot = S`, GRID_RESULTS_SHOT). `results_art_off` (GRID_RESULTS_ART_OFF=1)
is experimental: it keeps the plain panel, for the "before" side of a matched pair only.

Not ported: the screen's in/out animations (it appears in its held pose), `fepause_final` behind it
(the front-end session owns the blur), the stats and drift scoreboard screens, the pill art.

Row state colour [I]. The text materials carry DiffuseColour 0.1 (`fe_shadersetup.xml` gives the
front-end instances a DiffuseColour/Alpha and a GlowAmount stream), which reads dark on the grey
panel. Every animation of `text_pill_optiontitles#DiffuseColour` and
`text_pill_subtitles#DiffuseColour` in the three packages has the same keys, with no time ranges:
0.10 at 0 s, 0.83 from 0.33 to 0.67 s, 0.10 from 1.00 to 1.33 s, 0.83 at 1.67 s. The authored 0.1 is
the first key. 0x54D270 sends each row (`list.%d` and `list.%d.cursor`) the state `player` (when
FUN_004d99b0 holds), `friends` (FUN_004d9a00) or `deactivate`; FUN_00475f70 builds the state
message, and `persistentdatacommon.xml` gives `activate`, `reactivate` and `deactivate` 0.21 s each.
The cursor pill (`pill_glow_txtrd_3`, in the same 1.67 s animations, beside `text_pill_titles`) is
0.10 at 0.67 s, orange (1.0, 0.79, 0) at 1.00 s and 0.60 at 1.33 s. So each animation has two held
windows on its DiffuseColour curves, ending at 0.67 s and 1.33 s. The port reads the first as
`deactivate` (light text on the grey panel) and the second as `player` (dark text on the lit pill).
Each animated material gets one instance per state, sampled at the window end
(`results_art::hold_ends`, `animation_states`, `state_colours`). The player's row uses the
`player` instances; the other rows and the fixed items use `deactivate`. Each material takes its states from
the shortest animation that gives both: the 8.63 s animation of fePersistentDatabase keys dozens
of components at once, and its merged windows end elsewhere (the pill would read 0.2 in both). This
works for any material whose DiffuseColour an animation keys. The first window shot (F810), with the authored
0.15 pill, showed the player's row dark on dark. Which key range each state plays was not traced. The pause post-process of
`persistentdatarender.xml` (`fepause_final`: ToneMapFast with lumMin 0.2, lumMax 0.6, adaptMax 0.5,
bloomAmount 0.4) also
brightens the screen in the original; the port draws without a tone map (`Tonemapping::None`).
