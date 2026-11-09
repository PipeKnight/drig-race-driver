# Graphics audit: black textures, voids and wrong materials on every route

Status: 2026-11-09, Claude Code (session elegant-snyder). Owner's request (2026-11-09): check that
no track has black textures and that water, shadows and the rest are drawn right, so the owner
does not find graphics defects again during testing. This audit covers black or missing textures,
voids (nothing drawn), wrong-material surfaces and floating pieces; water and shadow defects are
the water-shadows session's and are only recorded here.

## The check

`grid-app --route R --scene reference --resolution 1280x720 --graphics-audit DIR --render DIR/final.png`
loads the route as the playable draws it in the default (Baseline) preset, offscreen, and writes:

- `materials.tsv` at load: every drawn track, route, land and object material with its original
  shader group, its diffuse texture, format and size, and the diffuse's mean luminance, mean alpha
  and cut-out fraction (alpha below one half), measured on the mip level at most 256 texels wide,
  decoded on the CPU. Flags: `unbound` / `missing` (no diffuse), `dark` (mean luminance below 0.04),
  `cut_out` (an alpha-tested object or tree diffuse more than 90 % below the mask threshold;
  neither applies to `god_rays*` light-shaft cards, coloured by shader constants, and `cut_out`
  not to blended `glass_simple`),
  `dark_unlit_emissive` (a dark diffuse that binds `TEmissiveMap` in a group whose emissive the
  port does not draw, today only `object_emissive_ripple`; a dark diffuse whose emissive is drawn
  is not flagged `dark`; the `emissive` column has the map and saved `EmissiveMapScale` of every
  material that binds `TEmissiveMap`), `no_uv` (textured draw without UVs), `undecoded` (a format the measure does not read) and
  `unnamed_group` (a shader group the port does not refer to by name, drawn by the generic
  diffuse path; informational). One line `graphics_audit_materials ...` sums the flags.
- after the normal warm-up, a camera sweep along the route's gate line (`progress_track.xml`
  gate centres): at the gate nearest each of N equal arc lengths (`--graphics-audit-points N`,
  default 24; 0 writes only `materials.tsv`; a point between two gates can be off the road, and
  the first run put Milan's camera inside a corner building block) a chase view
  (7 m behind, 2.6 m up, looking 12 m ahead) and a view to each side (75 degrees off the road,
  1.6 m up). The heights are over the track collision under each eye (searched from 3 m above
  the gate line, so a bridge overhead is not taken for the road): a gate centre is not always on
  the road, and the first run put Donington's side views under the terrain. Each pose is held 40 frames (ornament fades, shadows and temporal filters settle) and
  measured in process on 8x8-pixel blocks: a block is near-black when 3/4 of its pixels have every
  channel below 6/255 (the defects found were all but pure black,
  authored deep shade under arcades and in forests sits near 13/255 and is not flagged), and undrawn when 3/4 show the clear colour, which this mode sets to a
  magenta sentinel. The largest 4-connected region of each kind is the frame's measure; a frame is
  flagged at a near-black region of 1 % of the frame or an undrawn region of 0.2 %.
  `frames.tsv` has one row per pose, flagged frames are saved full size as `flag-NNN.png` (with
  `GRID_GRAPHICS_AUDIT_FRAMES=all` every other frame too, as `frame-NNN.png`), and
  `sheet.png` has every frame as a 256x144 thumbnail (rows = points, columns = chase/left/right).
  One line `graphics_audit verdict=PASS|FLAG|ERROR ...` ends the run.

`[GRID_EXE=path] sh tools/graphics_audit_all.sh [OUT [POINTS [ROUTE...]]]` runs it over every route folder with a
`track.jpk` except the menu garages under `tracks/frontend` (or the routes given) and prints one verdict line; `OUT/summary.txt` has a line per
route. Floating pieces are measured by the existing collision sweep (docs/FLOATING_AUDIT.md,
`sh tools/float_audit_all.sh`).

`GRID_GRAPHICS_AUDIT_RAYS=FILE` (with `--graphics-audit DIR --graphics-audit-points 0`) explains a
flagged region: each line of FILE is `eye look px py` (the frame's `eye` and `look` columns from
`frames.tsv` and a pixel of the 1280x720 frame, Bevy's default 45-degree vertical field of view).
One line `graphics_audit_ray` per ray gives the collision hit (`track.jpk`, 1 km) and the nearest
hits among all loaded draw instances, every ornament level included. No hit in either: the
original's data has nothing there. A part hit but not drawn: something at run time (draw
distance, ornament level) left it out.

## Emissive objects

Shibuya's `big_screen_*` video walls, many signs and the Le Mans TV drew black: their diffuse is
near black and the picture is in `TEmissiveMap`, which the port did not read. The original's pixel
programs (shaderpack disassembly; the `.asm` listings stay under ignored `local/ga-probe/emissive/`):

| Group | Adds before fog |
|---|---|
| `object_simple_emissive*`, `object_lod_emissive*` | `E.rgb` (unscaled; `EmissiveMapScale` is not read) |
| `object_animated_emissive*` | `E.rgb * EmissiveColour * EmissivePower` (UV scroll not reproduced) |
| `object_simple_night_emissive*` | `E.rgb * k`, `k = EmissiveTime * EmissivePower / 3`; the same program adds `shadowmask.gba * E * k * 15` to the light (street-light mask, unported) |
| `object_emissive_ripple*` (Shibuya water) | only through the street-light mask (unported, not drawn) |

`EmissiveTime` is set by 0x9790E0 (docs/re/symbols.tsv `night_emissive_time_init`) for the four
night-emissive groups: the daynight `emissive` curve in the timeline mode (Le Mans), 3.0 in forced
night (`headlights_on` routes, Haruna 1/3/6/8/9), else 0, so these signs are dark by day as in the
original. Raw group counts in `objects.pssg`: Shibuya simple 70, lod 18, animated 9, night 6,
ripple 4; Haruna lod 7, simple 6, animated 2, night 16; Le Mans simple 12, night 28; every other
circuit and street route 6 to 13 simple, some with 1 to 2 night.

The port (`object_materials::emissive_materials`, `configure_emissive`) sets the Bevy emissive
colour to the original's factor times the light unit, the baseline sun's lux over
`backlight_suncolour`'s peak over pi (`graphics::light_unit`, as `graphics::Phong`), with
`emissive_exposure_weight = 1`, so a map adds next to `albedo * light` what the original adds.
`update_night_emissive` follows the Le Mans clock. `GRID_OBJECT_EMISSIVE=off` leaves all maps out
for matched before/after renders. The start-light board keeps its own path (exposure weight 0).

Not ported, open for every night route: the night light (street lights from `street_lights.xml`,
parser 0x732970, and the `shadowmask.gba` term above) and car headlight projection (the original's
NightLights pass, docs/SOURCE_SKY_LIGHTING.md; docs/LE_MANS.md §3.4 "nothing is drawn").

## Results

Run 2026-11-09 (`local/graphics-audit-2/`, 24 points, exe built from this change before the ray
diagnostic; `GRID_EXE=local/ga-exe2/grid-app.exe sh tools/graphics_audit_all.sh local/graphics-audit-2 24`):
`graphics_audit_all routes=92 flagged=7 errors=0`. Materials pass: no route has an `unbound`,
`missing`, `no_uv` or `undecoded` material; `dark` (1 to 5 per route) and `cut_out` (1 to 5) are
listed per route in `materials.tsv` and were checked below.

| Track | Routes | Flagged | Fixed | Open |
|---|---|---|---|---|
| Donington | 4 | 0 | first run's 3 void flags were the sweep's side cameras under the terrain (sweep grounded) | - |
| Istanbul Park | 4 | 0 | - | - |
| Jarama | 2 | 0 | - | - |
| Le Mans | 2 | 0 | 27 night-emissive and 12 emissive objects drawn (night ones follow the 24 h clock) | dark bands in forest-shade canopies (shadow work) |
| Milan | 6 | 0 | first run's camera-in-building flags (sweep snapped to gates) | r5 arcade floor: authored deep shade (macro AO, F738), not a texture |
| Nürburgring | 4 | 0 | - | - |
| Spa-Francorchamps | 4 | 0 | - | forest interiors near black at frame edges (shadow work) |
| Haruna | 10 | 5 (r1, 3, 6, 8, 9) | night windows and signs drawn at EmissiveTime 3 | black sky on the forced-night routes (sky scale 0 in the data); the night light (street lights, `shadowmask.gba` light term, headlight projection) is not ported |
| Okutama | 6 | 0 | first run's void flags (cameras under terrain) | - |
| Shibuya | 8 | 0 | 100 emissive draws (signs, screens, shop fronts) | `object_emissive_ripple` (4, water, street-light mask only) |
| Yokohama Docks | 10 | 0 | - | - |
| Detroit | 10 | 2 (r0, r1) | - | a gap in the ground data near (90, 178, 264), see below |
| Long Beach | 6 | 0 | - | - |
| San Francisco | 6 | 0 | first run's void flags (cameras under terrain) | - |
| Washington | 10 | 0 | rows 107, 108, 110, 119 (F699, F702, F738) hold | - |

The first run (`local/ga-probe/summary-old.txt`, 32 flagged routes) also flagged Shibuya's dark
night sky (rgb about 4, 9, 20) and forest shade; the near-black test (every channel below 6/255)
now passes those, and still catches Haruna's pure black.

Emissive maps, matched (`GRID_OBJECT_EMISSIVE=off` on top, default below, same exe and poses):
`local/ga-ab/shibuya-r0-frame0{09,11,35}-off-top-on-bottom.png` (34 of 36 frames change by more
than 1 %) and `local/ga-ab/haruna-r1-frame034-off-top-on-bottom.png` (building windows lit). Le
Mans route 0 starts at 15:00, where the emissive curve is 0: no change, as the data says.

Detroit routes 0 and 1: magenta wedges of 0.22 to 0.29 % of the frame beside the beige mound, seen
by route 1's chase view too. `GRID_GRAPHICS_AUDIT_RAYS` (`local/ga-rays/`): the rays through the
wedge hit no loaded draw and no collision within 1 km; rays beside it hit `track1/3`, `edgestrip1`
and the buildings. Both views' rays cross near (90, 178, 264): the original's data has a hole in
the ground there, and what the player sees through it is the frame's clear colour. The port clears
to (0.055, 0.07, 0.095); the original's base render task defaults to (0.14, 0.11, 0.06) at +0x60
(0x64BCD0), but whether the scene pass keeps that is not checked. Open.
