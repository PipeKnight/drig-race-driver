# Cockpit: mirror, steering wheel, gauges and driver (camera session, 27 October 2026)

The head-cam draws the car's interior file `int_<stem>.pssg` (F618, [CAMERA_VIEWS.md](CAMERA_VIEWS.md) "Own car per view"). This document covers the parts that move or render on their own: the rear-view mirror, the steering wheel, the dash gauges and displays, and the driver with the hands. All of it is presentation only and never feeds the simulation.

Evidence tags: [D] decompiled through the master Ghidra project (`tools/re/grid_re.py`, `grid-unpacked.exe` SHA-256 `aac46a64…4514`); [V] shipped data or a test; [I] interpretation; AUTHORED = port choice. The original was not launched for this work. Private notes, scripts and shader disassembly are in worktree camera-dynamics-11a55d `local/cam/cockpit/{mirror,gauges,driver}/`.

Code: `crates/grid-app/src/cockpit.rs`.

## Mirror

### What the original does [D]

Setup, `renderer_create_race_passes` (0x602400):
- The pass "rear view mirror" (`rear_view_mirror_pass_ctor` (0x5F73F0), pass code 0x403, vtable `rear_view_mirror_pass_vtable` (0xCE3788)) renders into its own render texture "rearmirror".
- The size is the hardware settings' `<mirrors width height>`, 512 × 128 by default and in every shipped preset [V]. It does not depend on the screen resolution.
- MSAA is `<multisampling><rear_view_mirror level>`, read by `hardware_settings_read_graphics` (0x601B30): 0 in every option [V].
- `<mirrors enabled>` is never read: only the options menu (0x553D40) refers to the category. So the mirror is built and rendered at every preset, "off" included [D, static; the live check is in [CAMERA_VIEWS.md](CAMERA_VIEWS.md) §4].
- `<mirrors forceBilinear>` is stored at pass +0x2F0, but nothing reads it.

Mirror slots: `rear_view_mirror_attach_interior` (0x5FB990) walks the interior, and `rear_view_mirror_register_slot` (0x5F9F80) binds `int_reflection_l`, `int_reflection_r` and `int_reflection_m` (table `rear_view_mirror_slot_nodes` (0xCE51E0); nodes found by `interior_find_nodes` (0x99D490)). Each slot keeps:
- the node's matrix up to the interior root;
- the AABB of its vertices;
- its own shader instance, with `TReflectionMap` bound to the target.

The camera, every rendered frame (`renderer_frame_cameras` (0x5FBC10) → `rear_view_mirror_camera_update` (0x5FA440)):
- **When it renders.** The pass becomes active (+0x6B0) only while the interior is shown (head-cam, replay_driver1/2) and at least one mirror's oriented box is inside the main camera's frustum. Otherwise the target keeps its last picture.
- **Where it looks from.** One camera serves all mirrors. The eye is the mean of the mirrors' world AABB centres, moved 0.5 m forward along the car's +Z, so the near plane (0.5 m) passes through that mean.
- **Where it looks.** It looks back along the car (interior root frame), with the car's up. It does not follow the head-cam eye, the head look or the driver.
- **Projection.** Vertical field of view 15° (`REAR_VIEW_MIRROR_FOV` (0xCE5204)), aspect of the target (4), near 0.5 m. Far is 1000 m for drawing; visibility is culled at 500 m.
- **Not a mirrored camera.** Its picture is a plain rear view: the car's left is on the image's right.
- **Update rate.** Once per rendered frame, with no frame skipping (0x608940 → 0x5FBC10, and `renderer_frame_render` (0x5FAD10) → `rear_view_mirror_render` (0x5F33A0)).

What it draws, per `rear_view_mirror_collect` (0x5EA6A0) and `rear_view_mirror_draw` (0x5EA9C0):
- the world, with an object-size cull threshold 15 times the main view's (0x5FAD10);
- cars, through `vehicle_pass_submit` (0x9CB5B0) with code 0x403: only cars within 200 m of the mirror camera, at their coarsest LOD only, the player's own included (seen from inside, so its faces are culled [I]);
- in-car characters (0x85CC80), trees and ornaments.

It does not draw particles, cascade shadows, the cockpit interior or post-processing. The clear colour is the base pass default (0.14, 0.11, 0.06, 1) from `render_pass_base_ctor` (0x64BCD0).

How the glass shows it, per the `mirror.fx` vertex and fragment programs:
- The reflection is sampled at (1 − u, v) of the mesh's first UV set. This flip makes the mirror mirror-correct.
- The shipped UVs split the target between the mirrors [V]. For Lacetti after the flip: left mirror 0.55–0.98, right 0.02–0.43, centre 0.14–0.86 (the middle half of the height).
- The colour is the reflection blended toward a tint: out = refl + tint.w · (tint.rgb · combi.b − refl).
- `rear_view_mirror_set_colour_tint` (0x5F33F0) sets the tint from VehicleManager +0x4C..+0x58: black with w 0.5 for all three mirrors (VehicleManager ctor 0xA135C0). A clean mirror therefore shows the reflection at half brightness.
- Cracks come from window_fl/fr damage at scale 0.7 (`rear_view_mirror_set_crack` (0x5EA610)), and dust from the mud colour (`rear_view_mirror_set_dust` (0x5EA5E0)). Both are fed by 0x9FE020.

Corrections:
- 0x5FA440 is the mirror camera, not "the race camera path" that [TYRE_SMOKE_RESEARCH.md](TYRE_SMOKE_RESEARCH.md) "Camera rows are unit" calls it.
- 0x9CB5B0 was named `shadow_body`. It submits a car for every pass code (0x400 colour, 0x401 shadow, 0x403 mirror, 0x404), so it is renamed `vehicle_pass_submit`. The shadow use (F339, F600) is unchanged.

### The port

| Original | Port | Tag |
|---|---|---|
| target "rearmirror" 512 × 128, no MSAA | `Image::new_target_texture(512, 128, Rgba8UnormSrgb)`, `Msaa::Off` | [D] size; format [I] |
| one camera at mean(mirror centres) + 0.5 · F, looking −F with up U; fov 15° vertical, aspect 4, near 0.5, far 1000 | `MirrorCamera`, placed by `place_mirror` after `drive_present` from the body pose and the cockpit's anchor | [D] |
| active only with the interior shown and a mirror in the main frustum; keeps the last image otherwise | `Camera::is_active` from `camera::own_car(...) == Interior` and `Frustum::intersects_obb` of each glass | [D] |
| every frame | every frame (`GRID_MIRROR` can lower it for measurements) | [D] |
| draws world, cars, characters, trees, ornaments; no interior | the camera takes render layer 0 only; the cockpit's parts are on `cockpit::LAYER` (28), which the driving camera and the sun also take | [D] |
| visibility list built with far 500 m, drawn with far 1000 m | `cull_mirror` replaces the camera's `Frustum` with one ending at 500 m after Bevy's frustum update | [D] |
| no cascade shadow pass for the mirror | Bevy gives every active camera the sun's cascades; `no_mirror_shadows` makes the mirror's cascade frusta reject every caster, so its shadow maps stay empty: sun-lit, no shadows, no shadow draws | [D] |
| clear (0.14, 0.11, 0.06, 1) | `ClearColorConfig::Custom(Color::srgb(0.14, 0.11, 0.06))` | [D] values; sRGB reading [I] |
| reflection UV (1 − u, v); out = 0.5 · refl | the glass UVs are flipped at load; an unlit `StandardMaterial` with base colour 0.5 (linear) and the target as its texture | [D] |

The mirror camera copies the driving camera's `Exposure`, `Tonemapping` and `Hdr`, so both pictures go through the same exposure. AUTHORED: the original's mirror has no post-processing, and the port's exposure is itself an authored stand-in for the original's tonemapping.

Known differences, all AUTHORED or [I]:
- Cars are drawn at the level of detail the main view chose, with no 200 m limit, and the world has no extra size cull. The original draws other cars at their coarsest LOD only.
- Particles (smoke, debris) are not excluded from the mirror.
- Cracks and dust are not ported, so the glass is always clean.
- `<mirrors enabled>` has no switch, as in the original.

Orientation check [V, 27 October 2026]: the camera's image right is U × F, which is the car's left in Bevy's right-handed frame. After the (1 − u) flip the left glass samples the target's right half, so each wing mirror shows its own side and text reads reversed. The head-cam render (`local/cam/render/mirror/head-cam.png`) shows the outer wall on the outer side of the left mirror.

### Image proof (F625)

Fixed-step renders (`render.ps1`, `--render-fixed-step --drive --drive-ticks 300`) compared with `compare.py` against the F618 renders: chase_close, chase_far, bonnet and bumper are 0 pixels different. The head-cam shows the reflections. The log gives `cockpit_mirror size=512x128 every=1 fov_deg=15 near=0.5 far=1000 glass=3` for Lacetti.

### Cost at the phone's resolution

Measured on the desktop (RTX 4070, release build), 27 October 2026. Setup:
- the phone's screen of 2308 × 1080 with the mobile preset, so the 3D view renders 1539 × 720;
- head-cam, Lacetti at Donington with 11 opponents behind (grid position 1), `GRID_AUTOPILOT=1` driving;
- `[video] present_mode = auto_no_vsync` (uncapped), `GRID_MIRROR` to switch the mirror;
- the mean of the per-second `frame_stats` lines after the first 15 s, 40–50 s per run.

Tool: worktree camera-dynamics-11a55d `local/cam/mirror_cost/bench2.ps1` and `summary.py`; runs `ab1` (before the trims) and `ab3` (after).

| Mirror | Frame ms (two runs) | Cost |
|---|---|---|
| off | 6.11, 6.11 | – |
| 512 × 128 every frame (the original) | 7.53, 7.50 | +1.41 ms |
| 512 × 128 every second frame | 7.71, 7.73 | +1.61 ms |
| 256 × 64 every frame | 7.51, 7.47 | +1.38 ms |

`ab3` ran under the build lock, so no cargo job shared the PC.

Before the two trims (`cull_mirror`, `no_mirror_shadows`), `ab1` measured about +3.7 ms: off 6.60/6.37/7.06 ms, on 11.01/9.71 ms. That run shared the PC with other builds (its sixth run, at 26.8 ms, is left out), and it had `GRID_GPU_PASSES=1`, under which the mirror view drew 1.07 M triangles in five views (the camera and four sun cascades).

Interpretation:
- **The cost is CPU work.** The per-pass GPU time stayed about 1.1 ms with and without the mirror. The extra view needs its own visibility, extraction, queueing and batching every frame. A smaller target saves nothing.
- **Skipping frames is dearer.** Rendering every second frame cost more, because a camera that is turned back on rebuilds its view state [I].
- So the cheaper settings the owner allowed for the phone ("Ну отражение в зеркале обычно можно отрисовывать или в более низком разрешении, с меньшим количеством FPS и так далее. Так что если оно будет дорого стоить, это решаемо") would not lower the cost.
- **The phone keeps the original's 512 × 128 every frame.**
- [I, not measured] The phone's CPU is about 3–4 times slower per frame (F380), so the mirror would cost about 4–6 ms of a head-cam frame there.
- Only the head-cam pays this cost: the mirror is off in every other view.

To measure it on the phone, the Performance test's Auto cycle has a step `off:mirror` (perf_test `mirror`), which stops the mirror's updates. It only shows a difference while driving in the head-cam.

The phone-resolution frame of the mirror is `local/cam/render/phone/head-cam.png` (1539 × 720, fixed-step render after 300 ticks; private: it shows the original's assets).

## Steering wheel

### What the original does [D]

The cockpit's wheel is not turned by a computed angle. It is an animation clip sampled by the steering command.

- Node [V]: `int_<stem>.pssg` Scene Root / steer_master / steering_wheel. Its children are the render node int_steering_wheel and the hand dummies D_Lhand_Dummy and D_Rhand_Dummy. steer_master's local Z is the steering column, tilted 13.4° down in the Lacetti; steering_wheel's bind is a 90° turn about that axis.
- Clip [V]: set 0 of `int_<stem>_anims.pssg` (Steering, accel, Brake, Clutch, change_up, change_down, HandBrake_on, HandBrake_off). "Steering" lasts 4.0 s, with 121 keys at 1/30 s. It keys rotation and translation of steering_wheel and of both hand dummies; every off-axis quaternion component is 0.
- The interior controls animator (`incar_interior_controls_ctor` (0x84C5F0)) finds the clip; `incar_interior_find_nodes` (0x85C380) finds the nodes.
- Every frame `incar_feed_driver_inputs` (0x85F8F0) reads s = −(vehicle +0xE30), so +1 is full right.
  - For the human driver, `incar_driver_steer_human` (0x858C20) passes s straight to `incar_driver_set_steer` (0x851730), with no smoothing.
  - The AI path, `incar_driver_steer_ai` (0x858B20), is rate-limited.
- `incar_interior_set_steer` (0x84CA10) sets the timer to t = (s + 1) · 0.5 · duration.
- `incar_interior_update` (0x859FC0) samples the clip at t. The key replaces the nodes' local transforms; the slot writer (0xAC67A0) normalises the quaternion.
- Lacetti, relative to the bind at s = −1, −0.5, 0, +0.5, +1: −90°, −58.1°, −1.52°, +54.4°, +89.95° [V].
  - The centre key is one frame late (t = 61/30 s), so straight ahead shows −1.52°, as in the original.
  - Lock to lock is 180° in 31 cockpits; others differ (b05 120°, df3 ±45°, cre −60..+50°; private `local/cam/cockpit/driver/wheel_table.txt`).
- The exterior car's wheel (part 0x47, "SteeringDummy") is turned by the same clip in the non-cockpit scene indices (0x84C820). Which index each view uses was not traced [I].

### The port

- `grid_formats::neanim` reads NeAnim sets (`NeAnimPacketData_L4_KA` keys shared by channels, interpolated linearly; `B1` one-component Bezier). `pssg::read_tree` keeps `NeAnimPacketData_*` nodes as raw data. Every shipped interior and character animation file decodes [V, test `shipped_animation_files_decode`].
- `cockpit::append` finds steering_wheel and the "Steering" clip; each cockpit part under the node keeps its frame relative to the node (Lacetti: 6 parts).
- `turn_wheel` (F627; `animate_interior` since the pedals step), before `drive_present`: s = −(vehicle +0xE30), t = (s + 1) / 2 · 4 s; the part's rest = anchor · parent world · T(t) R(t) · its frame. The interior animator is described under "Pedals". The quaternion convention is pinned by the test `shipped_lacetti_steering_clip`: the centre key is the file's bind matrix.
- Not ported:
  - the hand dummies (no driver model yet);
  - the exterior wheel in the other views (left as in F618, so those views stay unchanged);
  - the AI path.

Image proof (fixed-step renders against F625's):
- chase_far, bonnet and bumper are 0 px different. chase_close is 0 px on a rerun; its first run differed in one pixel by 6, which the rerun showed to be run-to-run noise.
- In the head-cam, `--steering 1` (the port's controls: positive to the left) turns the wheel 90° counter-clockwise and `--steering -1` 90° clockwise. Renders: `local/cam/render/wheel`, `wheel_s-1`, `wheel_s1`.

## Pedals

### What the original does [D]

- The interior set's "accel" and "Brake" clips (0.333 s, 2 keys) each key all three pedal dummies (`D_accelerator_Dummy`, `D_brake_Dummy`, `D_clutch_Dummy`) and `D_Rfoot_Dummy`: each pose has one pedal pressed (about 10° on the Lacetti) and the foot on it. "Clutch" keys the clutch and `D_Lfoot_Dummy`; nothing plays it.
- Feed (0x85F8F0, human driver): p = throttle (`0x9961D0(1)`, vehicle +0xE1C) when it is at least the brake (`0x9961D0(2)`, +0xE20), else −brake. `0x9961D0(i)` reads +0xE18 + 4i and negates index 6 (the steering).
- 0x84CC40(p): the pedal crossfade switches at once to source 0 ("accel", p ≥ 0) or 1 ("Brake", p < 0); the accel timer is set to its length · max(p, 0), the Brake timer to its length · max(−p, 0).
- 0x859FC0 samples the sources with weight > 0 through the pedal binding (scene index 0 only), so one clip at a time poses the pedals and the right foot; the right leg's IK follows `D_Rfoot_Dummy` (0x852EC0).

### The port

- `interior_anim::Interior` holds the whole interior set on the nodes it keys (parents first). Each frame a node's local transform is the weighted sum of the playing clips that key it, the quaternion normalised, else the file's. Parts below a keyed node follow it (`AnimatedPart`). The steering wheel and its hand dummies use the same path.
- `cockpit::play` gives the clips each frame: "Steering" from s, and "accel" or "Brake" from p as above. `animate_interior` stores the posed nodes (`InteriorPose`); the driver's four IK targets are those nodes (the left foot stays at its file pose).
- Checks: `driver::tests::lacetti_hands_reach_the_wheel_dummies` also presses full throttle and full brake. Each pedal turns (0.167 and 0.175 rad) and the right foot stays within 1 mm of `D_Rfoot_Dummy`.

## Gauges

### What the original does [D]

Private notes: worktree camera-dynamics-11a55d `local/cam/cockpit/gauges/findings.txt` (with the motec, lcd and interior shader disassembly).

Files and parser:
- The `<displays type=…>` blocks of `int_<stem>.xml` are read by `interior_xml_parse` (0xA13AC0).
- Types: `analog` (0xA124E0), `analog_dummy` (0xA12A50), `digital` (0xA12770), `indicator_light` (0xA12BF0) and `motec` (0xA12E90). A display whose node is missing is not created.
- Across 43 cockpits: digital 38, indicator 33, motec 23, analog 18, analog_dummy 19. n12, the fallback cockpit, has no xml and no displays.

Updates:
- Every frame `interior_update` (0x9FDE40) updates the displays while the interior is shown.
- The race update feeds them through `interior_feed_displays` (0xA04DB0), but only when the frame's dt is not 0 (no feed while paused).

The values:
- tach = car +0x17A4 (the limiter, rad/s) × the engine audio's displayed ratio (0x414C70).
- speed = car +0x1648 m/s × 0.62137121, so the xml factor 3.6 gives mph.
- gear = car +0x196C as an integer (0 = N, 9 = R).
- boost = the turbo model's value; drift_angle and drift_peak come from the drift scorer.

Digital readouts (0x9E9DE0, 0x99DAC0, 0x9D07F0):
- n = trunc(v · factor), and the gear goes in as an integer directly.
- The text is itoa(n), cut to the leftmost `num_chars` characters and right-aligned over leading zeros; '-' and any other character draw glyph 0.
- The glyphs come from a simple font: `simplefonts/<font>_font.xml` and a 32 × 320 strip in `simple_fonts.pssg`. The strip is stored bottom-up; read upright, glyph 0 is at the top, and the gear fonts read N, 1–8, R.
- The texture is double-buffered (`digital_display_update` (0x99DAE0) uploads the other one), so a readout shows the previous feed.
- lcd_text.fx draws the readout's alpha in dark segments (EmissiveMapScale 0 by day). The gear uses an emissive interior.fx variant: TEmissiveMap × EmissiveMapScale 1.0 × EmissiveFactor.

Motec bars (`motec_set_value` (0x99DDC0), motec.fx):
- AlphaRef = (v − min_value) · ((max_alpha − min_alpha) / 255) / (max_value − min_value) · factor + min_alpha / 255.
- Every texel whose TAlphaMaskMap value is at most AlphaRef shows the "_2" maps, the lit bars.

Indicator lights (`indicator_light_set_value` (0x99DC90)):
- x = factor · v; ScrollU = shift (default 0.5) when cond_min ≤ x ≤ cond_max, else 0.
- ScrollU moves every texture lookup, so the light shows the other half of its maps. The glow is that half of the emissive map; the diffuse halves are nearly alike [V].

Needles:
- `analog_needle_set_value` (0x9D03D0) turns the node about its local Z by clamp(factor · v · sweep / (max − min), 0, sweep), where sweep = acos(Y · Y_max) of the node and its `_max` node, or 2π minus that with `long_way`.
- `analog_dummy_update` (0x9D0920) rests at the `_max` pose and wobbles by 2π · deviation · sin(2π · speed · t).

Backlight (`interior_set_backlight` (0x9FE6A0)):
- The shaders listed in `backlight_shaders` get EmissiveMapScale 0.3 while the player's headlights are on, and their file value (0 for Lacetti) otherwise.
- 0.3 applies whenever `emissive_scale` is present, whatever its text; without it the default is 0.15.

EmissiveFactor is never set by the code. Its constant-table default is 2.7 [I: whether that default is uploaded].

### The port (`dash.rs`)

- `grid_formats::interior` reads the displays, the backlight list and simple fonts.
- `pssg::read_tree` keeps `TEXTUREIMAGE` as raw data: `simple_fonts.pssg` uses that older layout. A sample of 120 shipped PSSG files had no other TEXTUREIMAGE node.
- The cockpit now draws its lcd_text.fx and motec.fx draws. Lacetti: 49 parts, none skipped.
- `dash::prepare` (at load) finds each display's parts, decodes the motec maps, the light's emissive map and the font strips.
- `attach` gives each display a copy of its part's material (the car's material path, no new shading):
  - lcd: black base with the readout as alpha-blended base texture;
  - gear: black base with the readout as emissive texture × 2.7;
  - motec: the bar composite as base texture;
  - lights: their emissive map × 2.7.
- `feed`, before `drive_present`, runs only while the cockpit is shown and the race runs:
  - digital readouts one feed late; the motec composite redrawn when the lit mask level changes; ScrollU as `uv_transform`;
  - needle and dummy parts rebuilt like the steering wheel.
- Values:
  - speed and gear as the original;
  - tach from car +0x1894 [I], the engine speed the HUD also shows, because 0x414C70 (the audio's displayed speed) is not ported;
  - boost, drift_angle and drift_peak are fed 0 (not ported).
- Backlight: not ported. By day the original's backlight shaders have no emissive, as the port draws them. At night (headlights on) they should glow at 0.3.

Checks:
- `dash::tests::readouts_follow_0x9d07f0`: leading zeros, overflow and '-'.
- `shipped_needles_sweep_to_their_max_nodes`: on 350, cam, gto, ply, rx7, s15 and sky, the turned Y axis reaches the `_max` node's within 2e-3.
- `interior::tests::displays_backlight_and_fonts_are_read`.
- Renders (Lacetti):
  - at rest: gear "1" in red, "000", the idle bars;
  - after 600 ticks at full throttle: gear "5", "133" (the HUD shows 214 km/h = 133 mph), the bars at about three quarters, shift lights 1–3 lit and the limit light off.
  - chase_close, chase_far, bonnet and bumper: 0 px against F627.
- Render (Nissan 350Z, `--car 350`, 10 displays: 2 analog, 8 analog_dummy): at rest the speed needle is at 0 and the tach at idle; after 500 ticks at full throttle the speedometer reads about 140 mph (HUD 225 km/h) and the tach about 6 krpm.

Not verified: the digit orientation against the original's screen (font strip order [I]) and EmissiveFactor.

## Driver

### What the original does [D]

- Model: `characters/models/driver_model.pssg`, one SKINNODE `body_x4` on 75 skin joints (a 3ds Max biped, `Bip01`, with fingers, roll bones, face bones and belt dummies) plus five plain NODEs in the joint tree: the IK hints `LA_hint`, `RA_hint`, `LL_hint`, `RL_hint` and `Bip01 Footsteps`. Its 16 `skinTransformLocal` instances (body, helmet and visor shaders) are named `_x1`..`_x6`.
  - 0x85D960 sorts them into six level-of-detail slots, numbered by the digit after `_x` minus one (0x84A9D0).
  - 0x85D310 picks the slot per driver. For the player it uses slot 1 whenever the view sets DisableDriverHead (driver node +0x170, from view +0xFD by 0x84E440).
  - Slot 1 holds only `_x2`: the body without helmet and visor. So the head-cam shows arms, hands and torso, and no head. The same slot puts the player's animation LOD at 1 (no face clips).
- Seat: 0x861C50 places the model on the interior's `D_bip_position` (the identity in every cockpit). The cockpit clip "Neutral" (`int_<stem>_driver_anims.pssg`, weight 1, 0x84BA20) seats the skeleton.
- Steering poses (0x851730, 0x852030):
  - with L = max(−s, 0) and R = max(s, 0), the clips 1..4 of `characters/anims/driver_anims.pssg` (`d_lhd_steer_toleft`, `_left`, `_toright`, `_right`) are added with weights min(2L, 1), max(2L − 1, 0), min(2R, 1) and max(2R − 1, 0);
  - a clip below 0.01 is skipped;
  - these clips are differences [V]: zero quaternions for bones they leave alone, small deltas elsewhere.
- Blend: the evaluator (0xACD760, 0xACD500) adds weight × value per slot with no weight normalisation; the slot writer normalises each quaternion (0xAC67A0). So a rotation is Neutral plus the weighted deltas, normalised, and a translation is the plain sum.
- IK (0x852EC0 legs, then 0x852FD0 arms; the arms weighted by the steering source of their crossfades, 1 in the steady state):
  - targets: `D_Lfoot_Dummy` and `D_Rfoot_Dummy`, then `D_Lhand_Dummy` and `D_Rhand_Dummy` (children of `steering_wheel`, keyed by the interior "Steering" clip);
  - the solver is NeAnimIKLimb (0xB00000) with the constructor's flags (0xAFF960): rotate the end, do not invert the bend.
  - With p0 the upper bone's world position, L1 and L2 the lengths of the lower and end local offsets, and d = target − p0:
    - the bend-plane normal is n = dir × (hint − p0), normalised, and u = n × dir;
    - beyond reach the limb points straight at the target;
    - otherwise c = (D² + L1² − L2²) / (2·L1·D), and the elbow is p0 + along·dir + perp·u, with perp = sin(acos c)·L1 in double precision and along = ±√(L1² − perp²) with the sign of c;
    - with |c| > 1 the limb folds along dir.
  - The upper and lower bones get frames with X along the bone and Y = n; their old twist is discarded. The end takes the target's rotation. Translations stay.
  - The description was checked by emulating every instruction of 0xB00000–0xB01ADC against a Python model: 150 random cases (in reach, out of reach, too close, with random flags) agreed, quaternion signs included (private `local/cam/cockpit/driver/ik_emu/`).

### The port (`driver.rs`)

- `grid_formats::scene::read_skin` reads the skin's joints, inverse bind matrices and instances. `mesh::read_object_meshes` now also keeps each vertex's four joints and weights (`Mesh::skin_weights`).
- The `_x2` instance is drawn through the car's material path as `car0/interior/driver/body`, with its TDiffuseAlphaMap and the default interior material, as a Bevy `SkinnedMesh`. It is an interior part, so it shows in the head-cam only. NoFrustumCulling, because the bind-pose bounds do not follow the seated pose.
- Each frame, before `drive_present`:
  - `s` = −(vehicle +0xE30), as for the wheel;
  - the Neutral and steering poses are blended as above;
  - the legs, then the arms, are solved (`ik::reach`) with this frame's FK;
  - the joints are written as children of the body part.
- Not ported:
  - gear-change and handbrake reaches (state machine 0x8573E0, per-car sets from database.bin `vehicle_interior`);
  - the g-force lean (0x857F20), breathing and face clips;
  - `small_cockpit_anims.pssg` (cars with `small_cockpit`);
  - the suit tint (TTintColourMask, tintColour1..3);
  - the drivers seen through the windows in the other views and in the opponents' cars (slots 0, 2..5 and 0x862300).

Checks:
- `driver::tests::level_of_detail_slot_reads_the_digit_after_x`.
- `lacetti_hands_reach_the_wheel_dummies` (ignored, GRID_GAME_ROOT): one drawn part (`_x2`), 80 joints and 75 skin joints. At s = −1, −0.5, 0, 0.5 and 1 both hands are within 1 mm and 1e-3 rad of their dummies.
- Renders (Lacetti, `local/cam/render/driver*`): gloved hands on the rim at rest and following it to both locks (`driver/turns.png`). Chase_close, chase_far, bonnet and bumper: 0 px against F630.

Not verified: the suit colours without the tint and the arm shape against a capture of the original.
