# Front-end flow: menu, loading, race, results

9 November 2026, Claude Code. How the original moves between the front end, the loading screen
and the race, so the port can rebuild the loading screen and its transitions. Static research
only: shipped data, and a time-boxed look at the executable. Related: [EVENT_MENU.md](EVENT_MENU.md)
(the port's menu and process model), [PAUSE_MENU.md](PAUSE_MENU.md) (pause Restart and Retire),
[RACE_HUD.md](RACE_HUD.md) ("Post-race", the results screens).

Labels: **[D]** read from shipped data; **[V]** verified in the executable (disassembly);
**[I]** interpretation; **AUTHORED** a choice of this port.

## Evidence

| Source | What it gives |
|---|---|
| `system/states.bin` (decoded: main checkout `local/pause-menu/states.xml`) | state and screen classes with their parameters |
| `system/flow.bin` (decoded: `local/pause-menu/flow.xml`) | nested node graph; a node's parents stay active; `link` = transition. Prefixes `f_` free race, `c_` career, `t_` test drive, `m_` multiplayer |
| `frontend/persistentdatacommon.xml` | named FE durations, `loading_stats` timing |
| `frontend/persistentdatarender.xml`, `fe_paramcontrol.xml` | loading camera styles and tweaks, fade styles, lighting of the loading world |
| `frontend/fePersistentDatabase.pssg` (tree dump: worktree `local/front-end-flow/fePersistent-tree.txt`) | the `loading` world, `loading_stats` glyph, spinner |
| `postprocess/effects.xml` | post effect `fe_loading_world` |
| `language/language_eng.lng` (dump: `local/front-end-flow/eng.tsv`) | loading strings and the `fact_*` family |
| `grid-unpacked.exe` via `tools/re/grid_re.py dis/xrefs` | class vtables, selection rule, timing reader |

The master Ghidra project was locked by another session during this work, so no pseudocode was
produced. The [V] statements come from capstone disassembly only.

## 1. State graph (free race)

The original has no separate loading "mode". Loading is a set of states nested under
`state_limbo`, and the FE renderer keeps drawing a 3D loading world while they run.

**Menu → race** [D flow.xml]
1. `f_m_selectrace` → `f_m_event_options` (Race Options) → `go_to_free_race` → `leave_to_free_race`
   (inside `garage_exit_preload`, releases garage audio) → `f_enter` → `start_current_event`.
2. `loading_enable_post_effect`: post effect switches to `fe_loading_world`, both durations 0.0 [D].
3. `race_load_coordinator` (`StateRaceLoadCoordinator`) inside `state_limbo` > `state_loading_race`
   (`StateLoading type=to_race`) > `load_race`. It links to `load_audio` (load=race), `load_cars`,
   `load_track` and `state_start_download`, each of which links back to the coordinator. Then
   `wait_for_load` (`StateWaitForLoadRace`).
4. `select_loading_screen` (`StateLoadingScreenSelect`) has two links: `standings` →
   `driver_event_standings` → team standings → hall of fame → sponsors/teammate → `loading_stat_screen`;
   `stats` → `loading_stat_screen` directly (`ScreenLoadingStats`, glyph `loading_stats`, inside
   `l_zone_loading_stats`, zone 9).
5. `f_enter_finished_loading` → `f_pre_enter_race` (`enter_race` inside `ingame`) →
   `decision_maker` → `prerace` → … race intro (RACE_HUD.md).

**Results → menu** (Continue, or Exit → Yes) [D]
1. `overlay_fade_on_hh` darkens the results → `end_race_prepare_finish_race` (or
   `end_race_prepare_exit_event`) → `end_race` → `post_race_exit`. Its links are `user_exit` and
   `skip_to_garage` → `loading_world_enter`; `next_race` → `nr_loading_world_enter`;
   `end_of_event` → `loading_world_enter`; `next_leg` → `restart_leg`.
2. The unload path: `set_audio_paused` → `hide_overlay_info` → `enable_loading_renderer`
   (`StateChangeActiveRenderer target=loading`, entry fade `fade_to_black_quickest`, waits for
   it) → `enter_loading_from_race` (`ScreenUnloadRace`, glyph `press_start`, zone 8
   `l_zone_unloading_race`) → `loading_enable_post_effect` → overlay fade off → network upload,
   autosave, best lap and ghost save → `start_loading_spinner` (`StateTriggerLoadingSpinner`) →
   `race_audio_release` → `exit_to_garage` → `wait_for_deregister_race` → `skipfe`.
3. `skipfe` → `f_exit_garage_prepare` (`state_limbo` > `state_loading_fe`
   (`StateLoading type=to_garage`) > `garage_loader`) → `garage_load_coordinator` (load audio
   load=fe, cars, track, patch-up) → `select_loading_screen` again → `stats`: rewards screens, or
   `loading_stat_screen` → `f_exit_finished_loading` → `f_exit` → `f_m_enter_menu` → `f_m_selectrace`.

**Restart** (results Restart Race → Yes, or pause Restart) [D]: `overlay_fade_on_hh` →
`f_pre_restart_race` / `closed_shutters_restart` (`restart_race`) → `decision_maker` →
`prerace` (pause path: `prerace_restart`, then the pan to the vehicle). No loading states, no
loading screen: the race restarts in place.

**Next race** (championship) [D]: `go_to_next_race` → `wait_for_deregister_race` →
`race_load_coordinator`, i.e. step 3 of Menu → race without going through the garage.

## 2. Loading screen content

**It is a 3D scene, not an image.** [D pssg] The `loading` ROOTNODE is a small world: a tarmac
floor with a logo plane (shaders `tarmac_d`, `tarmac_logo`), barriers, a building and a gantry
(shader `barrier`), a sky and a test particle emitter. Cameras: `load_cam` (FOV 55.4, animated
position), `zone_03`, `zone_04`, `tree_cam`. Anchors for the FE panels that appear on it:
`loading_stats`, `driver_standings`, `team_standings`, `hall_of_fame`, `press_start`, `sponsors`,
`bank_account`, `race_licence`, `comp_tree`, `profile_select` and others.
- The camera moves to `load_cam` with style `start_to_load_cam` or `profile_to_load_cam`, 1.0 s
  each [D persistentdatarender.xml]. While loading, the camera has slow noise rotation (one cycle
  in about 15 s by the file's own comment) and a spring the player can push with the stick
  (limits 0.35, final scale 0.25) [D].
- The scene is lit by the `loading` parameter group in `fe_paramcontrol.xml` (ambient 5/20/36,
  one directional light). Zone 9 has its own light values [D]. A floor reflection fades from grey
  30 to 5 over 30 units of height (`loading_reflection`) [D].
- Post effect `fe_loading_world` (id 19, chain Type4): fast tone map and a 4x-downsampled bloom
  [D effects.xml].
- **The drawn meshes and their shader (F784).** Below `loading_root` six nodes carry meshes, all
  with effect `fe3d_tracksurface.fx`, `DiffuseColour` (1, 1, 1), `FresnelMax` 0.2, one-sided:
  `building`, `barriers`, `gantry` (material `barrier`, texture `extra_text`, Emmisive 0.5),
  `sky` (`sky`, Emmisive 0.75), and under `floor` the `tarmac_plane` and `logo_plane`
  (`tarmac_d`, `tarmac_logo`, Emmisive 0.5) [D pssg, `grid-fe-anim --under=loading_root`]. The
  `screen/…` nodes have no meshes of their own but are glyph instances (see "Screens placed in the
  world" below). The shaderpack's pixel program
  `fe3d_tracksurface.cg.fp` computes colour = texture × v2 × (ambient + Emmisive × (1 − ambient))
  plus a box-projected cube reflection (`r_min`/`r_max`, raised to `r_gamma`, × `r_scale`) times
  a Fresnel term of at most `FresnelMax` and the texture's alpha; alpha = v2's. Its vertex
  program `.vp_instanced` sets v2 from a per-instance colour and passes no light, so the
  group's directional lights do not reach these surfaces [D, private disassembly in the main
  checkout's `local/front-end-flow/shaders/`].
- **Textures come from the bundle.** In `fePersistentDatabase.pssg` all four textures are
  4 × 4 DXT1 stubs (drawn, they give a black screen: F784's first build). The boot dataset
  `system/render_persistent_data.xml` loads that package as the `FE::DatabaseHandler`
  "database" and `frontend/b_persistent.pssg` as its "bundle"; the bundle holds the same ids at
  512 × 512 (`extra_text`, `tarmac_d`, `tarmac_logo` with 9 further mip levels, `sky` with
  none) [D]. That the bundle's copy replaces the stub is [I] (same ids, the stubs are
  unusable); the other `b_*.pssg` (garage, DLC) presumably pair with their databases the same way.
- **`load_cam`'s path.** `camera=load_cam` looks at `target=load_cam` (the saved matrix's view
  axis points at the target with no roll) [D]. `Anim!0` (18.333 s) moves the camera from
  (−104.96, −80.00, 169.71) at 1.0 s to (−148.68, −85.16, 144.59) and the target from
  (−44.25, −54.22, −17.12) to (−51.83, −63.96, −16.85) [D]: a slow drift and turn.
- **Port (F784, `fe_world.rs`).** The loading screen draws these meshes from the install,
  each texture from the bundle where it has one (`Spec::bundle`), unlit, texture × `DiffuseColour` × (ambient + Emmisive (1 − ambient)) with the `loading`
  group's ambient (an sRGB factor on the texture, so the product is the original's gamma-space
  one [I]; the instance colour taken as `DiffuseColour` [I]), no back-face culling [I], no tone
  map. The camera follows the two `Translation` channels by the screen's own clock (held before
  1.0 s and after 18.333 s), vertical FOV 55.4 [I]. The same `fe_world::Spec` takes any other
  root, camera and lighting group. Open: the floor reflection (`loading_reflection`), the
  particle emitter, the `fe_loading_world` post effect and the stat panel. The noise rotation,
  the stick spring and the cube reflection are ported (next items).
- **Cube reflection (F813, static analysis).** `fe3d_tracksurface.cg.fp` in full [D, shaderpack
  disassembly]: V = normalize(eye − pos), N = normalize(normal), f = sat((1 − N·V)^FresnelPower);
  R = N(−2 N·V) + V; ray = normalize(−R / (r_max − r_min)); d = 2·ray (1e-5 on an axis at zero);
  p = sat((pos − r_min)/(r_max − r_min)); q = p + d·min((sat(p + d) − p)/d) (where the ray leaves
  the box); cube direction (2q − 1)·(−1, 1, −1); reflection = cube^r_gamma × r_scale ×
  sat(lerp(FresnelMin, FresnelMax, f)) × texture alpha; colour = sat(texture × v2 × light +
  reflection), all on stored (sRGB) values. The parameters come from the lighting group:
  `r_min`/`r_gamma` fill `_refMinGamma`, `r_max`/`r_scale` `_refMaxScale` (the area of the
  group reader 0x4D7140 that reads r_gamma +0x1A0, r_scale +0x1A4, r_min +0x10, r_max +0x20);
  the vector reader 0x4C9A70 stores valueX, valueY, valueZ as x, y, z (and valueY again as w),
  so the file's own "max Z" comment on the third value does not change the axis [D]. The
  `loading` group: r_gamma 1.5, r_scale 0.8, r_min (−5.139, 0.132, −7.832), r_max (3.737, 4.235,
  9.674); its `zone_09` (`l_zone_loading_stats`, the loading screen's zone) sets r_gamma 1 and
  r_scale 1 and no ambient [D]; that a zone's value replaces the group's and the rest falls back
  to the group is [I]. The cube: `b_persistent.pssg` holds one CUBEMAPTEXTURE,
  `reflection_load.dds` (128², DXT1, six faces, no mips), the default cube that 0x4E2750 loads
  and 0x4DCFA0 selects; that `fe3d_tracksurface`'s cube sampler gets this default is [I]. The
  camera sits far outside the box (x ≈ −105…−149 against −5…4), and so does every loading-world
  surface (the floor is at y −120, the box at y 0.1…4.2), so p is clamped to a face. Where the
  ray points away from the box on a clamped axis, that axis's exit is 0, q = p and the lookup
  collapses to the nearest box corner; where it points back, q moves along the ray. The switch
  happens where an axis of the ray changes sign (a line through the point under the camera) and
  where p stops being clamped (a box plane, for example z = r_min.z). On the floor this leaves a
  hard-edged quad: inside it, the lookup reaches the cube's bright upper half, and outside it, the
  corner's dark lower half. A debug render measured this: texture alpha 1 everywhere, a smooth
  Fresnel term, and the cube's green jumping from 34 to 213 at the edge. The barrier shows a
  smaller rectangle of the same kind. This follows from the [D] formula once pos is world space.
  The space was traced in F817 [D unless marked], and no world-to-box transform was found:
  - The constant builder 0x4DFC00 (0x4E031F–0x4E0393) only inserts w into `_refMinGamma` and
    `_refMaxScale` (r_gamma, r_scale, with andps/orps masks). The box xyz go to the shader as
    the vector reader stored them. Nothing else in the uniform table 0xDAB000 (14 names,
    registered by 0x4D0960) carries a box transform.
  - The pixel program's eye constant is declared `float3 EyePositionWS` (c0). The vertex program
    writes the position as Model × instance rows × pos and the normal through the instance rows
    only. V = eye − pos is consistent only if pos is world space as well.
  - The `loading` group's box, together with its "valueX - max X" note, is a verbatim copy of
    `garage_01`'s. garage_02 (−7.711…6.138, 0.008…4.772, −8.576…8.503) and garage_03 have their
    own boxes, each of car size about the origin.
  - `loading_root` (fePersistentDatabase.pssg) is laid out hundreds of units away: logo_plane
    and tarmac_plane at y −120.34, load_cam's target at (−44.25, −54.22, −17.12), the screens at
    x −160…−105.
  - The only code that names `Model`/`ModelViewProj` (0xB0B600) builds them from the object's
    own matrices. It is a separate draw path; [I] the general mesh path binds the same names
    through the shader's parameter list, not traced.
  So, as far as static analysis reaches, the loading floor's lookup uses a garage box that does
  not enclose it, and the quad is the original's own result [I, strong]. A grid.exe capture
  would settle it. The port keeps the formula literal.
  Port: `fe_world.wgsl` (a `Material` with the formula above, sRGB re-encoded after sampling and
  decoded for the linear target), `fe_world::Surface`; the zone is `Spec::zone`, the cube
  `Spec::reflection`, read through `grid_formats::osd::read_cubemaps`.
- **Camera noise (static analysis of the unpacked exe).** The `<noise_tweaks>` of a
  `<camera_tweaks>` block (`default`, `pause`, `loading`, `in_game_cameras`) are read by
  0x47EBA0 into a noise object (constructor 0x487D00, vtable 0xCBA8D8; defaults x/y 1.2, z 1.01,
  rate 1.066, dampen 1 s). Its update 0x47ECB0 (camera, dt) [D]:
  - Two generators (vtable 0xCB8570, evaluate 0x476720) each sum three octaves at
    frequencies 1, 2, 4 with weights 1, ½, ¼ and divide by 1.75. An octave (constructor
    0x484640, evaluate 0x484780) keeps four values of the integer noise
    1 − hash(n)/2³⁰ (`n = (n<<13)^n`, `(n(n²·15731 + 789221) + 1376312589) & 0x7fffffff`) from a
    start index `rand() % 100`. Each call advances t by its step; past 1 it shifts in the next
    value. It returns the cubic P t³ + Q t² + R t + S with P = (v3 − v2) − (v0 − v1),
    Q = (v0 − v1) − P, R = v2 − v0 and S = v1.
  - The step is `rotation_rate × dt`, so `rotation_rate` is lattice steps per second (0.066:
    about 15 s each, as the file's comment says).
  - Angles: `noise_rotation_x` × first generator and `noise_rotation_y` × second, times
    clamp(2f² − f³), f = 1 − dampen share. The share moves at ±dt/`dampen_to_zero` in a direction
    that is 0 by default; at 1 both generators reset. `noise_rotation_z` is read but unused here.
  - 0x47C7F0 orbits the camera position about its target by these two angles in radians: the
    first about the camera's side axis (row 0 of the frame from 0x47C2E0, side = up × (eye −
    target), rolled by the camera's roll), the second about its up axis (row 1). Another path
    (0x47D100, flag byte +0x190) is off by default. Order and signs [E, F818: 0x47C7F0 run in
    Unicorn 2.1.4 on a camera object with up +Y and roll 0, three mixed angle pairs]: the
    result is the second angle about the camera's up axis, then the first about its original
    side axis; a positive first angle lowers an eye in front of a level target, a positive second
    angle turns +Z towards +X. `camera_noise::orbit` matches within 4e-7 (pitch-then-yaw is off
    by 0.36–0.76).
  - The wiring to the loading camera goes through component tables (0x490410 from 0x4CD3B0,
    0x48FBA0); that the loading world uses the `loading` block is [I] by the name.
  - Port (`camera_noise.rs`, any block by name; `fe_world::Spec::tweaks`): the same octaves, the
    MSVC `rand` from its default seed 1 for the start indices (the original seed is unknown [I]),
    the dampen share kept at 0 (no setter found on this path [I]), and the orbit with world up,
    yaw then pitch (as emulated, above). With the `loading` values (0.1, 0.1, rate 0.066) the
    camera turns up to about 0.1 rad about its target.
- **Stick spring (F818, static analysis; orbit emulated).** The `<spring_tweaks>` of the same
  `<camera_tweaks>` block tune a spring component (constructor 0x47E770, vtable 0xCB9B9C) that
  holds a four-lane spring object (constructor 0x47B250, reader 0x47B570) [D]:
  - Defaults: centering tension 11, damping 12; user tension 15, damping 11; thresholds and
    limits 10.1; final scale 1; `dampen_to_zero` 1 s. The `loading` block: 1, 2, 5, 1;
    thresholds 0.1; limits 0.35; final scale 0.25; dampen 0.5 s.
  - Input: the controller update 0x4A9FF0 calls 0x47F660 unless its target name is `none`. It
    reads the actions 0x2B..0x2E (`Fe View Tweak Left/Right/Up/Down`; pad preset 4: the right
    stick; keyboard preset: unmapped) and sends x = Right − Left, y = Up − Down in a message
    (vtable 0xCB96C0, type 0xDAACC4) to the camera, then once more with a second name (0xCB9C4C,
    purpose [I]).
  - The component's handler (0x47EB80 → 0x47EAE0) clamps each to ±limit and scales it by −0.5;
    0x47B7E0 makes that the user target and sets an axis' weight to 1 while |target| >
    threshold, else 0. The z and w lanes get 0.
  - Step 0x47B970: sub-steps h ≤ 0.1 s; per lane v += ((ku(target − x) − cu v) w + kc(0 − x) −
    cc v) h, x += h v.
  - Update 0x47E890: a dampen share like the noise's (the direction is 0 and no setter is known,
    so the weight stays 1 [I]); the angles are (`final_scale_y` x₁, `final_scale_x` x₀) through
    0x47C7F0 (so the first is the pitch); it also sends weight × v as message 0xCB849C (purpose
    [I], not ported).
  - With the loading values the full stick gives a target of ∓0.175 and settles at 5/6 of it,
    about 0.036 rad; stick right turns the eye towards −X of a +Z view (negative yaw).
  - Port (`camera_spring.rs`, `input::Actions::fe_view_tweak`, `fe_world::WorldCamera`): the two
    live lanes, the spring orbit after the noise orbit about the same target (their order within
    a frame is [I]), and the loading-screen app now installs the player's input
    (`loading_screen::app`). Which controller drives the loading camera is [I] by the block name,
    as for the noise.
- **Screens placed in the world (F791).** `loading_root`'s 16 `screen/<glyph>=…` nodes are
  instances (F787, `grid_formats::osd_glyph`): grafted, they add 2434 meshes, nearly all of
  the `fe3d_generic` family (generic, G, GD, GMPD, GPD, I), `fe3d_particle` and `fe3d_ramp`, from
  `bank_account`, `comp_tree`, `driver_offer_update`, the three leaderboards, `loading_stats`,
  `profile_select`, `new_sponsors`, the options screens, `press_start`, `race_licence` and the two
  sponsor screens [D, census in the worktree's `local/front-end-flow/loading-world/census.txt`].
  Several lie in `load_cam`'s view (`race_licence` spans x −136 … 1809 at z ≈ 0), so drawing all
  of them would stack every panel at once. A screen owns its instance (`ScreenLoadingStats` looks
  up `loading_stats` by name [D]); that it is drawn only while its screen is active is [I].
  `loading_stats` itself is three 4-vertex quads (`text=1..3` under `anim_top/mid/bot`, materials
  `load_top/middle/bottom`, `fe3d_genericGPD`): the text is built at run time. **Port:**
  `fe_world::drawn` grafts every instance below the root except `screen/` ones, which are left to
  the screen that shows them; on `loading_root` that is still the six meshes above.
- No per-track loading art and no progress bar were found. `raceload.jpk` is race data (physics
  and material parameters, docs/BINARY_XML.md), not loading art [D].

**Loading stat panel** (`loading_stats` glyph) [D pssg]: three extruded 3D text lines on
animated helpers `anim_top`, `anim_mid` and `anim_bot`:

| Line | Font, height, depth | Material | Text |
|---|---|---|---|
| top | microgramma, 5.8, 25, upper case | `load_top` (fe3d generic shader; `title_paint_pattern_13.tga`, `dirt_map_01.tga`, `reflection_sharp.dds`) | fact header |
| middle | microgramma, 11, 25 | `load_middle` | fact body (the value) |
| bottom | swiss black, 3.7, 4 | `load_bottom` | fact details |

- The text is a profile statistic, the `fact_*` family [D language table]: 79 English keys, 35 of
  them `_header`, the rest `_body` and `_details` with `[VALUE]`, `[TOTAL]`, `[REGION]` and `[TYPE]`
  placeholders (distance raced, win streaks, speed records, career results …). There is no tip or
  hint family. `loading` ("Loading, please wait") and `Loading_short` ("Loading...") also exist.
- [V] FUN_004EA770 (one caller, FUN_004FF470) fills a table of 38 fact slots of 0xC4 bytes, three
  0x40-byte string ids each (header, body, details), with 114 calls to the same string setter.
  The executable also names `fact_top_friend_*`, which the English table lacks. [I] the 38 slots
  are the 35 English headers plus online facts; the rule that picks one per screen was not found.
- Animations [D pssg]: `loading_stat_in/out`, `loading_text_in/out`, `loading_1_in` … `loading_5_in`.

**Timing of the stat panel**
- [D persistentdatacommon.xml] `loading_stats definedanimations=5`; `on_time minimum=0.8 desired=1.5`;
  `off_time minimum=0 desired=0 maximum=0.95`. Durations `loading_stat_play_in` 1.3 s,
  `loading_stat_play_out` 0.8 s, `loading_screen_to_stats_transition` 1.5 s,
  `stats_to_loading_screen_transition` 1.0 s, `time_before_end_to_play_in_stat` 1.45 s,
  `loading_stats_to_rewards_transition` 0.45 s, `load_world_fade_up` 0.5 s, `press_start_in` 0.2 s;
  sound `audio_loading_stats_in/out` 1.3/0.8 s. Fade styles `loading_stat_in/out` 0.2 s
  [D persistentdatarender.xml].
- [V] `ScreenLoadingStats` (vtable 0xCCDBE0, GetType 0x55FD80): its parse method 0x504700 looks
  up `loading_stats` through FUN_00477B80 and reads exactly these keys (`definedanimations` to
  +0x228, `on_time` min/desired to +0x1F4/+0x1F8, `off_time` min/desired/max to
  +0x200/+0x204/+0x208) and the durations `loading_stat_play_in/out`. Its per-frame update
  0x52B970 (takes the frame time) calls FUN_005048D0. That function calls `range` (0xA683E0)
  with (1, definedanimations+1) and stores the result at +0x22C.
  [I] that picks one of `loading_1_in` … `loading_5_in` at random (format string `loading_%d_in`).
- [I] `on_time` is how long a stat stays up (at least 0.8 s, 1.5 s wanted) and `off_time` the gap
  between stats (none wanted, at most 0.95 s): stats cycle while the load runs. The `minimum` is
  the nearest thing to a minimum display time found. No minimum time for the whole loading
  screen was found. `minscreentime` 4.0 belongs to save/profile screens, not loading [D].
- [I] Braces in the time values (`{1.5}`, `0.5}`, `{0.8`) look like ease-in/ease-out markers.

**Spinner** [D pssg]: the `load_overlay` root holds `load_circle_anim` (ramp shader with animated
RampMin/RampMax, i.e. a rotating sweep) and `load_circle_anim_disk` (flat colour). It is started
by `start_loading_spinner` on the unload path. Fade styles `savecirclefadein/out` are 0.4 s [D].
Other screens set `show_load_circle="true"` (profile enumeration, garage car load, network) [D].

**Fades** [D persistentdatacommon.xml]: `fade_to_black` and `fade_to_clear` 1.0 s;
`fade_to_black_quickest`, `fade_to_clear_quickest`, `_quick` 0.5 s; `screenfadein/out` 0.2 s,
`_slow` 0.4 s; `screen_transition_in/out` 0.4 s; `pausefadein/out` 0.5 s. The race → loading
switch uses `fade_to_black_quickest` (0.5 s) and waits for it [D states.xml]. The
`overlay_fade_on_hh` / `overlay_fade_off*` states take their time from glyph animations
(`anim_suffix` quick/quickest) [D]; those clip lengths were not read.

## 3. Executable findings

- [V] `StateLoadingScreenSelect` (vtable 0xCD8DE0, GetType 0x5A3FE0). Its enter method 0x57BCC0
  chooses the link. `standings` (string 0xCD8E1C) is chosen only when the game mode at
  game+0xBA0 (read by FUN_004C9930) equals 1 and the predicate FUN_004CA6A0 on game+0xA0 is true.
  Otherwise it is `stats` (0xCD8E14). The link getter is 0x57BD20.
  [I] mode 1 is career; the predicate tests an event that is set up and finished (a count
  at +0x18 has reached +0x14). Free race always shows the stat panel.
- [V] `StateLoading` (vtable 0xCD8D60, GetType 0x5A3FD0):
  - its parse method 0x57BA80 maps `type` to_garage → 0 and to_race → 1 at +0x3C;
  - its enter method 0x582C90 registers with the `limbo` component, passes the type to the
    game (FUN_004C9910 on game+0xBA0), sets a counter at +0x40 to 1 and posts a
    `vehicle_reload_id` job;
  - its update method 0x57BAE0 decrements the counter and finishes once it drops below 0, so the
    state itself only lasts a couple of frames;
  - its exit method 0x57BB00 sends `exit` and sets a busy flag at +0x38;
  - its message handler 0x582D20 clears the flag on a `complete` message.
  [I] loading is event-driven. The FE keeps ticking while the coordinator's load states run.
- [V] The string `FileStreamThreadHighPriorityHack` is registered by FUN_004EFE40, beside
  `raceEntry`, `raceExit`, `raceRestart`, `vehicle_load_request` and `[fe_update]`. The import
  table has `CreateThread` and `SetThreadPriority`. [I] file streaming runs on its own thread
  and gets high priority while loading. The race load does not block the renderer.
- Registrars and class names [V]:

  | Class | Name string |
  |---|---|
  | `ScreenLoadingStats` | 0xCD61F4 |
  | `StateLoadingScreenSelect` | 0xCDDDEC |
  | `StateLoading` | 0xCDDD4C |
  | `StateTriggerLoadingSpinner` | 0xCD71D0 (vtable 0xCC6EA0) |
  | `ScreenUnloadRace` | 0xCD71BC |
  | `StateRaceLoadCoordinator` | 0xCDDD10 |

## 4. Open

- Which fact is shown (random, rotating, eligibility by profile data) and how many per load.
- Whether loading has a minimum overall display time. The only minimum found is per stat
  (`on_time minimum` 0.8 s).
- Lengths of the glyph clips: `overlay_fade_*`, `loading_stat_in`, `loading_N_in`,
  `load_cam` path. They are in the PSSG animation data and were not decoded.
- What the game modes at game+0xBA0 mean, and what the predicate at game+0xA0 tests.
- When the spinner shows on the way into a race. `start_loading_spinner` was only seen on the
  unload path.

## 5. Port state machine (AUTHORED)

`grid-app --menu` on the desktop (`Play GRID.cmd`) runs the event menu, the loading screen and
the race in one process and one window (`crates/grid-app/src/front_end_host.rs`, F-entry
"One-window front end").

```
Starting ─▶ Loading ─▶ Menu ──Continue──▶ Loading ──ready, ≥0.8 s──▶ Race
              ▲                                                       │  ▲
              └──────────── Back to menu (results / pause Exit) ◀─────┘  └─ Restart (in place)
```

**Why a host app.** winit allows one event loop per process, and the race set-up is one long
function (`race_app` in main.rs) that builds a whole Bevy app. Rather than turn the set-up into
staged systems inside one world, every screen stays its own Bevy app, and a small host app owns
the window:

- **Host** (outer app): the window, the event loop and the `FrontEnd` state
  (`Starting, Menu, Loading, Race`), for the main-menu shell to build on. Its plugins are only
  the window, winit, input, the gamepad backend, time, task pools, log, states and the assets the
  cursor needs: no renderer and no audio (a renderer without a GPU backend logs errors).
- **Screens** (inner apps): the event menu (`event_menu::screen_app`), the loading screen
  (`loading_screen::app`) and the race (`race_app`), built with `hosted()` plugins (no winit, no
  log subscriber). Each is built on a background thread, `front-end-build`, up to its plugins'
  `finish`: plugins built, render device made, assets queued.
- **Main thread executor.** Bevy runs the systems that must stay on the main thread (those with
  `NonSendMarker`, such as `tick_global_task_pools` in `Last`) on `MainThreadExecutor`, which is
  the executor of the thread that created it and is ticked only there. Made on the build thread,
  it is never ticked, and the screen's first update waits for ever (observed: the loading screen
  attached and the host hung). So `attach` replaces it with the main thread's executor and only
  then runs the plugins' `cleanup`, which copies it into the render world and starts the render
  thread.
- **Attach:** the host gives the screen's primary window the outer window's raw handle; its
  renderer makes its own surface on the same OS window. Only one surface may exist per window,
  so the shown screen is dropped first. The menu and the race request their size (1280x800 and
  `[graphics]` width/height); the host applies it unless the user resized the window since the
  host last set it. The loading screen keeps the size.
- **Every frame:** the host forwards the window's input and window messages with the window
  entity remapped, copies size, scale factor, focus and cursor to the screen, runs the screen's
  `update()`, and copies its title back.
- **Gamepads:** the host has the process's one gamepad backend (`GilrsPlugin`); screens are built
  without it. Each gilrs instance starts a force-feedback thread that never ends (gilrs 0.11.2
  `ff/server.rs`: its loop ignores the closed channel), so a backend per screen left two threads
  and about 37 handles behind per race (thread census, F767). The host forwards each pad's
  connection, button and axis messages, raw and typed as `bevy_gilrs` writes them, with the pad
  as an entity of the screen's world; a screen attached while a pad is connected first gets its
  connection, as gilrs reports the pads present at start. Held buttons are not replayed, as at a
  gilrs start.
- **GPU devices:** one render device per kind of screen, `Gpu::Ui` (menu, loading screen) and
  `Gpu::Race`, made by the first screen of that kind and handed to the later ones
  (`RenderCreation::Manual`). The NVIDIA Vulkan driver (621.01, RTX 4070) leaves kernel objects
  behind for every device destroyed (2 mutants, 2 sections, a semaphore and a timer each, the
  rest of the ~35 handles per race; handle census, F767), so a device per screen grew without
  bound. Whenever the host drops a screen it waits on the kept devices (`reclaim_devices`): wgpu
  frees the resources of dropped apps only when their device is polled.
- **Leaving a screen** drops its whole app: world, entities, assets, GPU resources (the device
  stays), the audio output, its render thread. Nothing carries over between races. The pipeline
  warm-up (F726) runs inside each race app as before.
- **Exit codes** stay as in the multi-process supervisor: the menu ends with `MENU_GO` (20, build
  the race) or success (quit); the race with `BACK_TO_MENU` (21, build the menu) or any other
  code, which ends the process with that code. An exited screen stays on the window, frozen,
  until the loading screen replaces it. Closing the window sends the shown screen `AppExit` and
  one last update, so its exit systems run (camera save, force feedback stop).

**Loading screen.** On each change of screen the host builds the loading screen and the next
screen at the same time. The loading screen attaches as soon as it is ready; the next screen
attaches when it is ready and the loading screen has been up for 0.8 s (the stat panel's
`on_time minimum`, section 2), so a fast load does not flash. The loading screen fades up from
black over 0.5 s (`load_world_fade_up`), draws the 3D loading world (section 2, F784) and shows the `loading` string with the overlay's
load circle at the bottom right, where the leaving screen's overlay showed them (transition.rs,
F779). Not ported yet: the `fact_*` stat panel (the port keeps no profile statistics). The fade to
black before leaving the menu or the race, and the race's fade in, are F756's overlay fade
(`transition.rs`, docs/SCREEN_FLOW.md): each screen app plays it itself, so the host shows
menu → black → loading screen → race fading in from black.

**Kept outside the host:**

- `--settings FILE` (the `Play GRID *.cmd` race launchers) runs the race straight in its own
  app, as before: winit allows one event loop per process, so the host cannot start after the
  race's own loop. When such a race leaves to the menu, it starts `grid-app --menu` as a new
  process (docs/SCREEN_FLOW.md section 2.3), which runs the host. The host keeps the hand-off
  contract: with `GRID_HANDOFF_OUTER_READY` set, its window opens hidden at `GRID_WINDOW_AT`,
  shows after the shown screen's third update and then writes the race's ready marker
  (`front_end version=1 event=handoff_ready`), so the race exits with no desktop between them.
  Starting launcher races inside the host itself is open work.
- `GRID_FRONT_END=processes` keeps the older supervisor: one process per screen
  (`event_menu::supervise`, EVENT_MENU.md "Processes").
- The career screen keeps its supervisor (`career_screen.rs`).
- Android keeps relaunching the activity per screen: a NativeActivity owns its own event loop
  and surface, and the host path was not verified there.
- Restart stays in place inside the race (PAUSE_MENU.md). Championship "next race" does not
  exist in the port yet; it would go Race → Loading → Race.

**Diagnostics.** `GRID_MENU_LOOPS=N` with `GRID_MENU_AUTO=TICKS` (`tools/front_end_loops.ps1`):
each race ends at the tick limit and goes back to the menu. At each menu the host prints
`front_end_loops version=1 event=menu` with private MB, handles, threads and its entity count;
after N races (8 by default in the tool; the build gate runs it as `front-end-loops`) it prints
the verdict. The limits are AUTHORED: 48 MB and 40 handles of growth per race, measured from the
middle menu to the last, because the first races fill process-wide caches and the kept devices'
allocators (F767: 1.47 to 1.79 GB over races 1-6, then 1.74 to 1.91 GB up to race 12); the thread
count may not exceed the middle menu's by more than two (a pool or driver thread may still be
ending). The host's entity count must not change, and at least five menus must be sampled. The sample line also names the threads by count (`names=`), which is how the gilrs
threads were found, and counts the process's handles by kernel object type (`handle_types=`,
Windows only), which is how the per-device driver residue was found.
