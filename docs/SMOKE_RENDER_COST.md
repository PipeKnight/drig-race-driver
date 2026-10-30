# Tyre smoke rendering cost

29 October 2026, Claude Code (F643). How much tyre smoke costs per frame and where, measured to
explain the owner's Derby frame-rate collapse on a MacBook (F640, [FIDELITY_BACKLOG.md](FIDELITY_BACKLOG.md)
rows 97-98), and which optimisations would remove it. [O] = observed or measured, [I] = interpretation.
The port's smoke is described in [TYRE_SMOKE.md](TYRE_SMOKE.md), the original's in
[TYRE_SMOKE_RESEARCH.md](TYRE_SMOKE_RESEARCH.md).

## Summary

- [O] Smoke is almost entirely GPU fill cost. In a dense Derby replay the sprites cover 50-100
  screens per frame (lit, blended, MSAA4). At 3024 x 1890 the smoke pass alone takes 8.5 ms on
  average and up to 18 ms per frame on an RTX 4070, against 0.04 ms with the smoke not drawn.
- [O] The cost scales exactly with the pixel count (2.78x for 2.76x the pixels). Per-pixel PBR
  lighting is about half of it (unlit -44/-46 %). Sprites within 3 m carry little (-17/-22 % when
  dropped); most coverage comes from sprites 2-15 m from the camera.
- [I] An M4 GPU has roughly 1/5-1/7 of the 4070's fill and shading throughput (public specs, not
  measured), which gives 60-125 ms, matching the owner's 60-177 ms frames.
- [O] G329's smoke replay pass adds no GPU cost: the same draws move from the transparent pass to
  `smoke_replay`. It adds about 0.27 ms of render-world CPU per frame, with or without smoke. The
  owner's build (fac75955) predates G329, so G329 neither caused the drop nor fixes it.

## Setup

- Source 5a70c4d2 (G329 automatic smoke replay included) plus an uncommitted diagnostic patch,
  switched on only by environment variables. Release build through the build lock; rustc and
  cargo 1.99.0, Bevy 0.19.1. PC: i7-12700, RTX 4070, Vulkan, NVIDIA 616.92, shared with other
  work (CPU timings noisy; GPU timestamps are not affected).
- Scenario: Codex's G329 Derby fixture without PNG capture. STK on Detroit route_8, 11 STK AI,
  chase_close camera, Baseline, `--drive-ticks 900 --throttle 1 --steering 1 --handbrake 500
  --input device=keyboard --render-fps 30 --render <png>`: offscreen, no window, no audio. The
  player spins donuts in its own smoke. The particle sequence is identical in every run
  (deterministic): 86 dense frames with 800 or more sprites, at most 1324.
- GPU times are Bevy pass timestamps (`GRID_GPU_PASSES=1`). The patch adds per-frame lines and a
  span around the G329 replay pass, which had none and was invisible to the timings.
- Main view as reported by G329's capability line: `Rgba8UnormSrgb`, 4 samples, no HDR, at both
  resolutions. `--aa off` still gave 4 samples in Baseline, so MSAA on and off were not compared.

Diagnostic patch (`GRID_SMOKE_DIAG`, comma list, in `tyre_smoke.rs`):
- `stats`: per frame the sprite count, overdraw in screens (sum of frustum-clipped projected quad
  areas over the screen area, split by sprite distance <2 / 2-5 / 5-15 / >15 m), the largest
  sprite and the `draw()` CPU time;
- `hide`: the group entities hidden (no GPU draw, CPU path unchanged); `skip`: `draw()` returns
  early; `unlit`: the smoke material unlit; `nearcull=D`: sprites within D m of the eye dropped.
- `gpu_passes.rs` with `GRID_GPU_PASS_FRAMES=1`: per-frame GPU ms per pass, fragment-shader
  invocations (Vulkan pipeline statistics) and CPU encode time.
- `smoke_boundary/live.rs`: wall time of the G329 render-world systems.

## Results

Smoke pass GPU time in the dense frames (66-84 per run; timestamps lag 3-5 frames and were
aligned). "Before G329" is `--smoke-boundary off` (smoke inside `main_transparent_pass_3d`).
Opaque (about 0.4 / 0.9 ms), clustering and UI are unchanged; shadow and post passes are not timed
by Bevy, so no total GPU frame time is given.

| variant | 1920x1080 mean / p95 / max ms | 3024x1890 mean / p95 / max ms |
|---|---|---|
| smoke not drawn (`hide`) | 0.03 / 0.04 / 0.05 | 0.04 / 0.06 / 0.07 |
| before G329 | 3.10 / 5.44 / 6.59 | 8.43 / 14.74 / 17.82 (repeat 8.50 / 14.94 / 18.09) |
| G329 default | 3.07 / 5.16 / 6.15 (repeat 3.02 / 5.16 / 6.17) | 8.59 / 14.81 / 18.01 (repeat 8.04 / 14.33 / 17.45) |
| G329 + unlit material | 1.72 / 3.07 / 3.67 (-44 %) | 4.62 / 7.74 / 9.34 (-46 %) |
| G329 + sprites within 3 m dropped | 2.54 / 4.42 / 4.76 (-17 %) | 6.68 / 13.36 / 14.50 (-22 %) |

Over the whole run (after 60 frames) the smoke pass averages 2.0 ms at 1080p and 4.8-5.3 ms at
3024 x 1890.

Overdraw [O], identical in every run, dense frames: mean 51 screens at 16:9 (54 at 16:10), p95
93 / 101, max 96 / 105; one sprite can cover a whole screen. By distance (1080p means): under 2 m
3.4 screens, 2-5 m 24.1, 5-15 m 22.8, beyond 15 m 0.7. The GPU's fragment counter at 3024 x 1890
confirms it: 197 M fragment-shader invocations per frame on average, 420 M at peak, tracking the
coverage estimate with correlation 0.915 at 0.69 of it [I: the rest fails the depth test, sprites
cutting into the ground and cars]. About 0.042 ns per fragment, 24 G lit blended MSAA4 fragments
per second.

Vertex work is negligible: 608 primitives and 2038 vertex invocations per frame.

CPU per frame [O], about 1000 sprites, release, noisy PC:
- `tyre_smoke::draw` (bucket by texture, sort back to front, build quads, stream into the
  meshes): mean 0.29-0.31 ms, p95 0.35-0.43 ms. Vertex upload is about 250 KB per frame.
- G329 render world: extract 0.19-0.22 ms mean (p95 0.26, max 3.0), prepare 0.032, barrier 0.013,
  finalize 0.005, replay encode 0.012 ms: about 0.27 ms per frame, constant, also with no smoke.
  Extract runs while the main world waits. Most of it is `smoke_boundary/live.rs` `extract`
  scanning every `Mesh3d` entity (about 4000 here) once for each of the five smoke groups to
  prove a unique owner.

Owner log (F640: M4, Metal, fac75955, Baseline, native 3024 x 1890, Fifo), frame_stats binned by
the last logged particle count [O]: under 200 particles median 109 FPS (min 65); 200-600 median
100 (min 38); 600-1000 median 72 (min 8.6); 1000 or more median 51 (min 6.7). Count predicts the
cost only through coverage: what matters is how many large sprites lie 2-15 m from the camera,
which explains 1255 particles at 84 FPS against 1312 at 10 FPS.

## Options

Gains are for the smoke pass at 3024 x 1890, dense mean 8.5 ms.

| # | option | expected gain | fidelity risk |
|---|---|---|---|
| 1 | Reduced-resolution smoke buffer, as the original's `pfx_half_smoke` path ([TYRE_SMOKE_RESEARCH.md](TYRE_SMOKE_RESEARCH.md) §7: reduced colour and alpha target, reduced depth, `ONE/ONE_MINUS_SRC_ALPHA` accumulation, `compose_no_alpha` with `ONE/SRC_ALPHA`; the creator shifts 1280x720 by 2, a quarter per axis). **Done in F645 at half per axis: smoke passes −41 %, dense-frame p95 −35/−43 %.** | [I] half per axis, single-sample: about 2.1 ms plus 0.1-0.3 ms of depth downsample and composite, about -70 %. A quarter per axis about -90 %. Extrapolated from the measured pixel scaling, not run. Also fixes most of the Retina case. | Low-medium, and closer to the original. Edges at depth discontinuities (cars inside smoke) need the reduced-depth test or a bilateral upsample; G329's composition and glass ordering must carry into the compose step (Codex owns the look). Largest effort: render-graph node, targets, compose shader. |
| 2 | Lighting per particle (the original samples light per particle) instead of per-pixel PBR, with an unlit-cost fragment shader. **Done in F644: −58/−63 % measured.** | [O] upper bound -44 to -46 % (unlit). With #1 about 1.2 ms [I]. | Low if the sun and ambient terms are evaluated per particle; the exact source arithmetic is still open. Small: a custom material, or vertex colours with `unlit`. |
| 3 | Near-camera fade or projected-size cap | [O] -17 to -22 % for a 3 m cull; sprites 5-15 m away carry about 45 % of the coverage | Medium-high: thins the cloud around the player; the source's near fade is not recovered (§7.5 item 3). Only as a safety valve under a GPU budget. |
| 4 | G329 CPU: keep the five group entities (`TyreSmoke` already stores them) instead of five full `Mesh3d` scans per frame in `live.rs` `extract` | about -0.2 ms CPU per frame on the extract critical path, smoke or not; no GPU change | None (same entities; check uniqueness at install or on change). Codex-owned file. |
| 5 | High-DPI render-scale default (row 98, F642) | 3024 x 1890 to 2304 x 1440: -42 % of all pixels, smoke included | Resolution only. Done in F642. |

Not recommended as the main fix: fewer particles, or an alpha cut-off with `discard` (tile-based
GPUs such as the M4 lose hidden-surface removal with `discard`, and the fragments are still
shaded).

## Per-particle light (F644, option 2 done)

30 October 2026, Claude Code. The smoke material is now unlit with the original particle light
in a small fragment (TYRE_SMOKE_RESEARCH.md §8, TYRE_SMOKE.md "Material"). Measured as above
(Derby fixture, baseline, RTX 4070, Vulkan), before = origin/main 66a0c9cf and after = the change,
each with the same uncommitted diagnostics (per-frame GPU pass lines, the `smoke_replay` span, a
per-frame sprite count), release builds through the build lock. Dense frames (76-78 of 86 matched
after the timestamp lag):

| smoke_replay GPU ms | 1920x1080 mean / p95 / max | 3024x1890 mean / p95 / max |
|---|---|---|
| before (scene-lit per pixel) | 3.15 / 5.16 / 6.16 | 8.15 / 14.37 / 17.21 |
| after (light per particle) | 1.34 / 2.60 / 3.12 (−58 %) | 2.98 / 5.12 / 6.44 (−63 %) |

The gain exceeds F643's unlit bound (−44/−46 %). [I] The custom fragment never contains the PBR
lighting code, so its compiled shader is smaller than the stock unlit variant, which keeps the
lighting branch; not verified with a shader profiler. Over the whole run (after 60 frames) the
pass averages 0.92 ms at 1080p and 1.97 ms at 3024x1890 (before 2.47 and 5.46).

Look, 150-frame sequences of the same run at 1920x1080 (`--motion-capture 300,150`), compared
frame by frame:
- With the sun in front of the smoke (for example frame 140, dense cloud around the stopped car)
  the clouds match; the new frame is 7-10 % brighter on average and has no dark patches where
  sprites face away from the sun.
- Looking towards the sun from inside the cloud (frames 90-100) the old material lit only the
  ambient side and drew a near-black cloud (mean frame value 70-80 of 255); the original's wrap term
  keeps 75-93 % of the sun-side brightness, so the cloud is light grey (160). This is the
  intended source behaviour, not a calibration error.
- Thin smoke against the cars and walls is now visible as light haze where it was dark before.
Smoke in shadowed parts of the track is lit as in sunlight both before and after: the old
material did not receive shadows, and the new one has no birth shadow sample yet.

Not measured for this change: Metal/Android, enhanced and source-sky-IBL lighting modes (the
fragment reads their environment map and atmosphere transmittance but was only checked in
baseline), night tracks.

## Reduced smoke buffer (F645, option 1 done)

30 October 2026, Claude Code. `smoke_reduced.rs` / `smoke_reduced.wgsl`, inside G329's automatic
replay (`smoke_boundary/live.rs`, `smoke_boundary_core.rs` `draw_into_scene_reduced`). On a frame
with at least one sprite, once its pipelines are compiled:

1. Depth reduction: a half-size `Depth32Float` image gets the nearest scene depth of the four
   pixels under each texel (first sample of each pixel). The viewer camera's depth texture is made
   sampleable for this (`Camera3d::depth_texture_usages`).
2. Smoke: the five groups replay into a half-size `Rgba16Float` target cleared to (0, 0, 0, 1),
   one sample, viewport exactly half the full extent, blend `SRC_ALPHA/ONE_MINUS_SRC_ALPHA` colour
   and `ZERO/ONE_MINUS_SRC_ALPHA` alpha (the source's accumulation for a straight-alpha fragment).
   The pipelines are the source smoke pipelines with that target, one sample and the shader def
   `TYRE_SMOKE_REDUCED`; the depth test stays (read-only attachment). Under the def the smoke
   fragment also fades alpha by `saturate(2.5 × metres in front of the reduced depth)`, the
   original fp_half programs' soft fade (`pfx_smoke.cg.fp_half`), which the full-size port never had.
3. Compose (`compose_no_alpha`: colour `ONE/SRC_ALPHA`, alpha kept), once per sample under MSAA:
   the four nearest reduced texels' bilinear weights divided by their relative depth step from the
   pixel's sample (half weight at 1 %), and where none lies within 5 % the best of the nine texels
   around the pixel's own. Pixels whose four texels hold no smoke are discarded before any depth read.

The glass and composition order is G329's: the compose runs where the full-size replay ran. Not
taken (full-size replay as before): frames without sprites, pipelines still compiling (the first
3-4 frames), a source pipeline that is not straight alpha blending, a depth texture that cannot
be sampled, or the reduced depth image not yet resized after a resolution change.

Deviations from the original [I]: half per axis instead of its quarter; the original faded
against the far bound of a 4x4 block and redrew pixels between the near and far bounds at full
size (`fp_half_edges`); this port tests and fades against the near bound and resolves edges in
the compose instead. The near bound measured better than the far bound in the comparisons below
(frame 110: 7 against 323 pixels off by more than 40/255).

Cost, same binary with the reduced path switched off by an uncommitted probe variable (full) or on
(reduced), four alternating runs each, Derby fixture, baseline (`Rgba8UnormSrgb`, MSAA4), RTX 4070,
Vulkan; GPU ms per frame summed over the timed passes:

| | 1920x1080 full | 1920x1080 reduced | 3024x1890 full | 3024x1890 reduced |
|---|---|---|---|---|
| dense frames mean / p95 / max | 4.94 / 9.52 / 13.88 | 4.23 / 6.22 / 7.51 | 6.15 / 11.74 / 16.07 | 4.98 / 6.69 / 8.56 |
| smoke passes (replay + reduce + compose), dense mean | 2.88 | 1.01 + 0.17 + 0.52 = 1.70 (−41 %) | 4.34 | 1.65 + 0.13 + 0.79 = 2.57 (−41 %) |
| `main_opaque_pass_3d`, dense mean | 1.36 | 1.66 | 1.39 | 1.85 |
| whole run after 60 frames, mean / p95 | 3.27 / 6.48 | 3.43 / 5.18 | 4.09 / 7.58 | 3.74 / 5.54 |

The opaque pass grows by 0.3-0.46 ms only when the depth is actually sampled: a probe with the
depth sampleable but the reduced path off measured the same opaque time as without the flag (six
runs each, 0.89-1.43 against 1.19-1.49 ms). [I] The driver decompresses the MSAA depth for
sampling and the time lands in the next opaque pass. That fixed cost and the per-sample compose
make light-smoke frames at 1080p about 5 % dearer on average while dense frames, which caused the
F640 drops, lose a third of their p95 and half their worst case. Absolute times vary between
sessions on this shared PC (the full-size replay measured 1.34 ms in F644 and 2.88 ms here for the
same code), so only same-session comparisons are given.

Look ([O], 150-frame sequences at 1920x1080 against the F644 full-size replay, plus the owner's
review): the clouds and their brightness match (mean frame difference under 2/255; a car inside
dense smoke looks the same). The soft fade removes the hard lines where flat sprites cut through
the cars and the ground, and leaves smoke thinner within about 0.4 m of surfaces; the owner
preferred this over the full-size look with its lines and doubling. Enhanced graphics (HDR,
single sample, Lacetti at Donington) runs the single-sample variant with the same result.

## Not measured

- Windowed frame time with present mode Immediate (the owner was at the PC; the offscreen renderer
  does not present). GPU timestamps were used instead.
- Shadow cascades, SMAA, post and resolve passes (not timed by Bevy), so no total GPU frame time.
- MSAA1 against MSAA4 for the blending.
- The reduced-resolution variant's quarter-per-axis setting (F645 measured half per axis).
- Apple Metal and Android: the M4 ratio is an interpretation.
- One scenario only (player donuts, chase_close camera); other cameras and other cars' distant
  smoke not separated.

## Evidence (private)

`local/smoke-cost/` in the main checkout: `diag.patch` (the uncommitted diagnostics), `run.ps1`,
`analyze.py`, `analyze2.py`, build logs, `runs/*/stdout.log` (one PNG kept) and the original
draft `smoke-cost-research-draft.md`. Reproduce in a worktree of 5a70c4d2:
`git apply local/smoke-cost/diag.patch`, build release through the build lock, then
`local\smoke-cost\run.ps1 -Name auto-3024 -Res 3024x1890 -Diag stats` and
`py -I local\smoke-cost\analyze.py local\smoke-cost\runs\auto-3024`.
