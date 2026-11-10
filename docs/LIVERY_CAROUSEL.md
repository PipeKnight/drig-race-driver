# The livery designer's 3D pill carousel

10 November 2026, Claude Code (F816). Static analysis only (master Ghidra project, `tools/re/grid_re.py`, the shipped front-end databases); grid.exe was not run. Tags: [V] verified in code, [O] observed in the files, [I] interpretation, AUTHORED a port choice. Port: `crates/grid-app/src/livery_carousel.rs` on hud's fe3d module (F810), driven by the career screen's Team Branding page (CAREER_SCREEN.md). The garage scene behind it is not loaded yet (GARAGE_WORLD.md).

## The scene

- [O] `livery_editor_root` (feNonPersistentDatabase.pssg) → `rotation` → `pitch` holds the glyphs `component/livery_carousel_pills` ×3 (`colour_pills`, `pattern_pills`, `number_pills`, each at t = (0, −14.441, 0)) and `component/livery_carousel_main=main`. The three share one place: one kind is shown at a time. Private dump: `local/livery-3d/survey.txt` (grid-fe-survey), animation keys `local/livery-3d/keys.txt`.
- [O] Each pills glyph: `vertical!0` (local y 10) → `rotation!1` (the ring pivot, world (−270.1, −4.4, 0)) → `gadget/liv_pill_items=liv_pill_0` (world (319.9, −4.6, 0), so the ring radius is about 590) holding:
  - `kitpart/liv_pill` → `null=liv_pill!0`: the pill mesh, material `grey_light` (fe3d_generic), 176 vertices, x 319.5–320.3, y −6.8 to −2.4 (a thin upright tablet facing +x);
  - `gadget/liv_pill_simple_library` → `library_simple=liv_pills` with `null=pattern` → `library/liv_pattern` → `unique=default!9` (material `basic_image`, fe3d_genericI, TDiffuseMap `basic_image.tga`) and `null=numbers` → `text=numbers` (material `grey_dark`, fe3d_genericGPD, font microgramma1, height 2.5).
- [O] `main` → `vertical` (0, 10, 0) → `rotation!0` → `pitch!0` holds `camera=carousel_cam_1` (eye (378.1, 10, 0), target (309.9, 10, 0), fov 68.04, near 0.1), `object/liv_titles` (the designer's menu pills and titles; the port's text rows replace them) and `component/liv_pill_boarder_anim=pill_b` (world (329.6, −2.6, 0)).
  - `pill_b` → `kitpart/liv_pill_boarder` has no mesh of its own: it carries the two `component/tab_arrow` glyphs (`arrow_right` at z −5.3, `arrow_left` at z +5.3; material `tab_arrow`, fe3d_genericGMPD, colour (0.827, 0.651, 0)). They flank the selected pill.
- [O] Animations (`keys.txt`):
  - `null=rotation` (Anim!51/69/72/86): 12 s, yaw 0 → −6.28 linear, one full turn.
  - `null=vertical` (Anim!50/68/71/85): 12 s, y 10 → 0 → −10; t = 0 is the saved pose.
  - Anim!53 `pill_boarder`: t = 0 and t = 2 are y −15.99 with x rotation −1.57 (folded away below), t = 1 is the identity. [I] t = 1 is the shown pose: the arrows sit beside the selected pill.
  - Anim!54/55 tab arrows: a 4 s bob along x (±5.3 → ±5.9).
  - Anim!52/73 camera target: x 309.85 → 173.95 on the view axis; the view direction does not change.
- [O] Four `carousel_rotate` events in the database are `FEEVENTPLAYTOFRAC` (max 12, frame 6) followed by a `stopped_moving` trigger; one has `durationOverride` 0.2 s with `durationIntan` 1, the others no override. Which glyph owns which event is not mapped.

## The code

- [V] 0x50CE30 parses the ScreenLiveryDesignerRoot settings; `seperation_angle` lands at +0x3B8, read through the getter 0x503DA0 that six carousel vtables share.
- [O] The settings themselves are in `states.xml` / `states.bin` (ScreenLiveryDesignerRoot): `seperation_angle` 0.0145, `colourcarousel glyph="colour_pills"`, `patterncarousel glyph="pattern_pills" library_object="pattern"`, `numbercarousel glyph="number_pills" library_object="numbers"`. The port has no states.bin reader, so it keeps these as constants tagged with their source.
- [V] 0x50D080 lays out the ring: it clones the template item for i in 1..count and sends each clone a rotation message (vtable 0xCC3F08) of (0, i × separation, 0, 0). Item 0 is the template itself.
- [V] 0x500E40 (CarouselBase::update) steps the index by the input and does not wrap; at an end it plays `dead_end` and holds, otherwise it fires `carousel_rotate`. 0x5011B0 sets the index. 0x503CE0 / 0x503D30 play a UI sample and set or randomly pick the index.
- [I] `carousel_rotate` plays the ring's 12 s `rotation` animation to the fraction index × separation / 2π, so the ring's yaw is −index × separation and the selected pill faces the camera.
- [V] Colour: 0x52B760 → 0x514950 → 0x5F1D50 take a `livery_colour` row's red, green and blue bytes / 255 and send them as a colour message (vtable 0xCBA004, through 0x4845D0) to the node path `liv_pill.liv_pill` of each item (0x4901A0 sends a message along a dotted node path). [I] The message sets the pill's DiffuseColour.
- [O] Pattern pills: the pattern pictures are not in the front-end packages or b_persistent. `frontend/b_garage.pssg` (28.9 MB, 1100 textures) holds 52 textures `liv_pattern_pattern_0` … `liv_pattern_pattern_51` (64×32 DXT1, clamped in s), one per `livery_pattern` row, beside the `car_free_race_*` pictures. [I] A pattern pill shows the texture named by the pattern library (`library/liv_pattern`) + `_` + the row's `fe_image_name` on its `basic_image` node.
- [I] Number pills show the row's `number_string` in `text=numbers`; colour pills hide the library's children.

## The port

`livery_carousel.rs`, one path for every part, car and livery table:
- Loads `livery_editor_root` with the shared front-end packages, expands it with `osd_glyph::expand_with` and keeps the pills template of each carousel kind, the camera and the `pill_b` arrows (pose t = 1 of Anim!53). `liv_titles` is skipped.
- Pill i is the template turned about the pivot's y axis by (i − turn) × separation, world = P · R_y · P⁻¹ · template with P the template's parent (`rotation!1`). [V] With the camera looking down −x, pill i + 1 lies to screen right. `turn` eases to the index in 0.2 s (AUTHORED from the one `durationOverride`; the curve t + t² − t³, slope 1 at the start from `durationIntan` 1 and flat at the end, is [I]); a new step restarts the ease from where the ring is, and a new kind or table snaps.
- A pool of 25 slots per template item (12 on each side of the front pill) is reused while the ring turns; slots outside the table are hidden, which also shows the clamp at the ends (AUTHORED: 0.0145 × 590 ≈ 8.6 units apart, about eight on each side are in view).
- Content: a colour pill copies the pill material with DiffuseColour = RGB / 255 (alpha kept); a pattern pill adds the `basic_image` node with b_garage's `liv_pattern_<fe_image_name>` texture; a number pill adds the `numbers` text mesh of `number_string`. Materials and number meshes are cached per content.
- Lighting `garage_01` / `zone_22` (the livery editor zone) of fe_paramcontrol through fe3d's `Lights`; reflection cubes `ref_garage_soft/sharp` (CUBEMAPTEXTURE DXT1, 256 and 512) of `frontend/b_garage_01.pssg` when they read, otherwise dial2's `env_soft/env_sharp` (results_art). [I] The original's garage number is the career's current garage; the port uses garage 01 until the garage scene is loaded.
- The career screen shows the carousel of the part under the cursor on the Team Branding page (pattern, colour 1–3, number) and moves its panel to the top, just below the overlay's top letterbox bar (`transition::bar_shares`), with compact spacing and a wider panel (one-line footer), so the pills are visible below it at 1280×800. `[diagnostics] livery_carousel_off` (GRID_LIVERY_CAROUSEL_OFF=1) keeps the text rows only, for comparisons.
- Log lines: `livery_carousel version=1 loaded=true …` (counts, cube source, camera) or `loaded=false error=…`, and `livery_carousel version=1 event=shown kind=… pills=… index=…` when the shown selection changes; `tools/career_loop_smoke.ps1` waits for the latter on the Team Branding page.
