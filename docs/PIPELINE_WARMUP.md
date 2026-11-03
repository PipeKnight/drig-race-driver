# Pipeline warm-up: objects missing while their pipeline compiles (row 113)

Status: 2026-11-03, Claude Code (session nervous-chaplygin). Owner's report on prototype
170678e6ce36 (Washington race start): "иногда кадр словно моргает, я не понял это из-за
обновления объектов или из-за чего" (sometimes the frame seems to blink; not clear whether it is
the objects updating or something else). Port: `crates/grid-app/src/pipeline_warmup.rs`.

## 1. Mechanism

- A window renders with Bevy's default background pipeline compilation. Offscreen captures used
  synchronous compilation (`capture_pipeline_compilation synchronous=true`), so no capture could
  show this. `GRID_CAPTURE_ASYNC_PIPELINES=1` now keeps background compilation in a capture.
- Bevy makes a render pipeline the first time a draw needs its key. While the pipeline compiles,
  `SetItemPipeline` returns `Skip`, so every draw that uses it is missing from the frame
  (bevy_render 0.19.1 `render_phase/mod.rs`).
- Keys first needed mid-race (`GRID_PIPELINE_TRACE=1`: one `pipeline_new` line per pipeline with
  its label and key defines, `pipeline_ready` with the frames it waited). Washington route 0, race
  start, autopilot, 960x540, background compilation, exe 895cf8fb380f/d5f99f5fe73d:
  - the fades' `VISIBILITY_RANGE_DITHER` variants of the opaque, masked, alpha-to-coverage and
    blended mesh pipelines (F703 ornaments, F708 cars and drivers). A fade inserts the range on
    both levels, so the whole object disappears until the variant is ready;
  - masked depth-prepass variants (`pbr_prepass_pipeline` with `MAY_DISCARD`), among them the
    shadow-cascade ones (`VIEW_PROJECTION_ORTHOGRAPHIC`) of masked meshes with a second UV set;
  - plain variants of materials first seen later, the tyre smoke passes, the F713 shadow proxy.
  - 60 s run (`t60.log`): 19 new pipelines after the start, at render frames 271–2966. Most were
    ready after 1 frame on this PC, but the two opaque dither variants created at frame 870 (kind
    216 changing level at 27 m, about 680 px across) waited 16 and 23 frames: that object was
    missing for 0.3–0.4 s. How long a compile takes depends on the machine and its load, so on
    the owner's PC during play every first use can be such a gap.
- The start itself: the 3D scene is absent for the first frames (clear colour and HUD only; all
  66 start pipelines are ready by render frame 15, the scene shows from capture frame 36), then
  appears at once. No repeated whole-frame blink was found in offscreen captures of ticks 1–900
  (`blinks.py`: frames that differ from both neighbours while the neighbours agree).
- Ruled out for the owner's settings (`render_height = native`): render-scale target changes
  (F642 `Auto` is the Android/macOS default only). GPU occlusion culling is not enabled (its
  pipelines are compiled but no camera has `OcclusionCulling`). No exposure adaptation in the
  baseline grade. Not checked: the interactive window's presentation (`auto_no_vsync`).

## 2. Warm-up (port, all tracks, desktop and Android)

`pipeline_warmup::Warmup`, once per process (one race per process: the menu supervises a race
process; Android relaunches the activity). From the first frame with a viewer camera and a mesh:

1. A black full-screen UI node covers the viewer camera (`GlobalZIndex(i32::MAX)`).
2. Plain: every `Mesh3d` gets `NoFrustumCulling` (also honoured by the shadow cascades), every
   hidden mesh and hidden parent (`Visibility::Hidden`, not UI) is shown, and
   `ornament_lod::select` draws every ornament level as a caster.
3. When the cache has had no new or waiting pipeline for 8 frames: Dither. Every mesh also gets
   the step-0 fade range (`fade_range(false, 0)`: drawn whole, dither variant) and every
   shadow-level ornament part a step-0 shadow proxy (`shadow_proxies`).
4. Quiet again: everything is undone. Ranges the warm-up added are removed unless a fade replaced
   them. Each shown entity gets back the visibility its own system last set: a change by another
   system during the warm-up is recorded through change detection.
5. Quiet a third time (the removed ranges re-specialize): the cover lifts. After 900 frames it
   lifts regardless.

The main world reads the render world's cache through a shared counter (`count` in
`RenderSystems::Cleanup`). `GRID_PIPELINE_WARMUP=0` skips the warm-up (comparison runs).

Not covered: car levels hidden through `car_batch::hidden_layer()` and their demoted meshes (the
levels the other cars show at the start share their materials and layouts), and meshes spawned
after the warm-up.

## 3. Evidence

Private, `local/blink/`: traces `t60.log` (before), `t60-fix*.log` (after), captures `w-*`,
`x-*`, and the detector `blinks.py`. Results are in F726.
