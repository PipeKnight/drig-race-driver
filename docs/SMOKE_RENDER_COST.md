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
| 1 | Reduced-resolution smoke buffer, as the original's `pfx_half_smoke` path ([TYRE_SMOKE_RESEARCH.md](TYRE_SMOKE_RESEARCH.md) §7: reduced colour and alpha target, reduced depth, `ONE/ONE_MINUS_SRC_ALPHA` accumulation, `compose_no_alpha` with `ONE/SRC_ALPHA`; the creator shifts 1280x720 by 2, a quarter per axis) | [I] half per axis, single-sample: about 2.1 ms plus 0.1-0.3 ms of depth downsample and composite, about -70 %. A quarter per axis about -90 %. Extrapolated from the measured pixel scaling, not run. Also fixes most of the Retina case. | Low-medium, and closer to the original. Edges at depth discontinuities (cars inside smoke) need the reduced-depth test or a bilateral upsample; G329's composition and glass ordering must carry into the compose step (Codex owns the look). Largest effort: render-graph node, targets, compose shader. |
| 2 | Lighting per particle (the original samples light per particle) instead of per-pixel PBR, with an unlit-cost fragment shader | [O] upper bound -44 to -46 % (unlit). With #1 about 1.2 ms [I]. | Low if the sun and ambient terms are evaluated per particle; the exact source arithmetic is still open. Small: a custom material, or vertex colours with `unlit`. |
| 3 | Near-camera fade or projected-size cap | [O] -17 to -22 % for a 3 m cull; sprites 5-15 m away carry about 45 % of the coverage | Medium-high: thins the cloud around the player; the source's near fade is not recovered (§7.5 item 3). Only as a safety valve under a GPU budget. |
| 4 | G329 CPU: keep the five group entities (`TyreSmoke` already stores them) instead of five full `Mesh3d` scans per frame in `live.rs` `extract` | about -0.2 ms CPU per frame on the extract critical path, smoke or not; no GPU change | None (same entities; check uniqueness at install or on change). Codex-owned file. |
| 5 | High-DPI render-scale default (row 98, F642) | 3024 x 1890 to 2304 x 1440: -42 % of all pixels, smoke included | Resolution only. Done in F642. |

Not recommended as the main fix: fewer particles, or an alpha cut-off with `discard` (tile-based
GPUs such as the M4 lose hidden-surface removal with `discard`, and the fragments are still
shaded).

## Not measured

- Windowed frame time with present mode Immediate (the owner was at the PC; the offscreen renderer
  does not present). GPU timestamps were used instead.
- Shadow cascades, SMAA, post and resolve passes (not timed by Bevy), so no total GPU frame time.
- MSAA1 against MSAA4 for the blending.
- The reduced-resolution variant itself: its gain is an estimate.
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
