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
- the lap-time stacker re-flow (traced, section 6.4);
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
- Not ported: boost, chat, driver name tags, offers,
  the stacker re-flow (hidden groups leave their gap), the cutscene/post-race
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
after entering the state, then counts 1 s; its link check 0x58E2E0 returns "done" then (or at once
when `FinalBoundary` 0x5EBC90 disqualified the car). Its clock is the replay controller's step
(0x944430). Exit 0x57F170 stores the state's total time in 0xE6DFF8, which the terminal replay's
rewind (0x596C90) adds. Afterwards `flow.bin` plays the crash replay
(`instant_race_replay_silent`) and the wreck menu comes on a button press. Port:
`post_race::on_wreck` waits the two transitions, the rest (at most 2 s) and 1 s; it shows the menu
without the automatic replay (AUTHORED).

Two fields of the OSD owner select the layouts below: F = owner+0x1FC, the competition-format
enum of DISCIPLINES.md 1.1 [V: the drift handbrake hint shows only for F 2..4, and the stacker
dispatch 0x6BB0C0 jumps on F 0..8 in the same groups], and T = owner+0x1AC (values 1..4, meaning
open).

Driver name tags [D: bind 0x6B0540, update 0x6BF070]:
- The update runs while +0x2A8 is set and the game mode is not 2; its first frame hides every tag.
- T 2, normal path (0x6BD0B0): tag 0 shows the rival (the career lookup 0x6BCE60, or in the
  player's own career event the leader, the second car when the player leads, with more than two
  cars); `driver_id_team` shows the team-mate (0x6AAB90). A rival who is the team-mate is dropped;
  no rival and no team-mate hide the tag.
- T 2 with a driver offer of objective type 2 (0x6BD480, 0x6BD6B0): up to seven cars of the named
  team, nearest first, on tags 0..6.
- T 3 (0x6BD900; probably online, it also drives the chat box [I]): the two nearest cars on one
  side and the nearest on the other; the `wrongway` tags when the car's byte +0x1550 is set. Skipped
  for F 3 and 9; the distance is Euclidean for F 4, else signed along the track (0x90BDB0).
- Limits (0x6A80A0): hidden beyond 60 m (0xDB4F60); alpha = base − min(|d|/60, 1)³ × (base −
  [0xE6DFF4]). Text: the position ("%d"), the name (32 characters) and the nationality flag
  (0x69C860, 0x6A0820). Anchor (0x6B0820): the car position raised by 1.5 + 0.04 |d| m, projected;
  behind the camera it goes off screen, with no edge clamp. The tag plays the "expand" range.
- Open: what T and game mode 2 are, the sources of the parameters +0x2B0/+0x2B4 and of the base
  alphas (+0x210). Not ported: the port has no career rival or team-mate data in a race yet.

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
or T 2) and `split_group`. Not ported: which layout a port event gets depends on T and slot 0x24,
whose meaning is open.

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
- Not ported: the ring shake near the end (0x6A77E0; its amplitude and threshold come from the
  attach's construction, not traced), the points roll-up, `drift_message` (attach 0x6A8620, its
  feeder not traced) and the battle position multiplier, which the plain text panel showed.
