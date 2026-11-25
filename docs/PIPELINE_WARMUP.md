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
2. Plain: every `Mesh3d` gets `NoFrustumCulling` (also honoured by the shadow cascades and, since
   F740, by `shadow_cull`), every
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

In window runs the race start waits for the warm-up: the first drive tick, and with it the
countdown, runs only once the phase is Done (`StartHold`, F889, at most 1800 fixed steps), so a slow PC
never compiles warm-up pipelines after GO. Before, the countdown started at once and its first
seconds passed under the cover; in a slow derby trace run the warm-up ended after GO (F884). One
line `drive_start_hold event=released held_steps=N reason=warmup_done|limit|ticking` is logged.
`GRID_START_AFTER_WARMUP=0` starts at once as before. Headless and offscreen runs never hold.

Not covered: car levels hidden through `car_batch::hidden_layer()` and their demoted meshes (the
levels the other cars show at the start share their materials and layouts), meshes spawned
after the warm-up, and entities whose `Mesh3d` handle has no asset under the cover (nothing to
queue, so no pipeline; section 2.6).

### 2.1 Shadow variants built mid-race (row 115, F740)

Owner retest after F726 (prototype e66e1c24963c): "отдельные объекты иногда моргают ... по мере
приближения к ним и довольно редко".

- Trace [V] (`GRID_PIPELINE_TRACE=1`, silent autopilot windows at 2560 × 1369, 120 s, Washington,
  Spa, Shibuya and Le Mans): after the warm-up, 3 to 6 new pipelines per run, each waiting 1 to 2
  frames. They are the first tyre smoke's two passes, the reduced alpha-blend pass, and cascade
  (orthographic) depth prepasses of masked materials.
- The trace now names each new pipeline's users (`pipeline_user`): the late shadow variants
  belonged to trees (`trees/!f`, level 0, casting from the shadow layer) and to their fade proxies
  (F713). Two of them were also the variants F726 could not place.
- Why the warm-up missed them [V]: `shadow_cull` (docs/PERFORMANCE.md "Sun shadow cost") drops
  casters from each cascade by geometry after Bevy's own check, and it ignored `NoFrustumCulling`.
  The warm-up census (`pipeline_census`) on Washington: 4910 casters, all unculled, but only 219
  in the cascades. So a tree's shadow variant, or its proxy's, was built the first time it entered
  a cascade while the car approached it, and that shadow was not drawn until the compile finished.
  On this PC that was 1 frame; on a slower compile it is longer. [I] This fits "rarely, while
  approaching them", but the owner's own sighting was not captured.
- Fix: `shadow_cull` keeps casters with `NoFrustumCulling`, as Bevy's lists do. After the
  warm-up only the skinned prop pieces stay unculled.
- After [V]: all casters are in the cascades during the warm-up, with a shadow pipeline (Washington
  4910/4910, Spa 6922, Shibuya 2382, Le Mans 14713), and no shadow variant is built after it on the
  four tracks. Left: the first tyre smoke and its reduced alpha-blend pass, 1 frame each (fixed in
  §2.2).
- Car levels behind `hidden_layer()` built no pipeline after the warm-up in these runs
  (11 opponents each).
- Test: `shadow_cull::tests::unculled_casters_stay_in_every_cascade`.

### 2.2 The first tyre smoke's pipelines

- [V] F740 traces, Washington and Spa: the 3 pipelines left after the warm-up were all the reduced
  smoke buffer's (F645, `smoke_reduced`): `tyre smoke depth reduction`, `tyre smoke compose` and
  `pbr_alpha_blend_mesh_pipeline reduced`, made on the first frame with smoke sprites (render frame
  685 and 666), mid-race.
- Cause: the source smoke pipelines were already warm. The five smoke groups are spawned at the
  start with an empty mesh, are visible and unculled, so the full-size replay draws them from the
  first frame. But the automatic replay (`smoke_boundary/live.rs`, every window and image view)
  queued the reduced buffer's pipelines only on a frame with sprites.
- Fix: `smoke_reduced::queue` queues, once each, the reduction (per multisampled depth), the
  composition (per view format and sample count) and the reduced variant of every source pipeline
  the replay owns. The automatic replay calls it on every frame, with or without sprites, so they
  compile while the cover is up. Shared for all cars, tracks and smoke variants: the full-size
  sources come from the empty groups, the reduced ones from those sources, and a resize changes
  only the target size, which is in no pipeline key (F742 rebinds a material, not a pipeline).
  Sprites mode (`--smoke-order sprites`) is research-only and not covered.
- Test: `smoke_reduced::tests::pipelines_are_queued_once_before_the_first_smoke` (fake queue: all
  three queued without sprites, nothing new on a repeat or a resize, an uncovered source keeps the
  full-size replay).
- After [V], F751: Washington and Spa build no pipeline after the warm-up (before: 3 each), with smoke on screen from the opponents' launch wheelspin.

### 2.3 Blit pre-warm: a camera first seen under the cover (item 6)

- [V] Profile builds with 1 s windows (`local/ab/hitchprof.ps1`, Donington race, `present_mode =
  auto_no_vsync`): the worst start frame sat in the window with `pipeline_warmup phase=Restore` and
  `water_probe capture=start`. One span covered it: Bevy's
  `bevy_core_pipeline::upscaling::prepare_view_upscaling_pipelines`. Enhanced, cold driver cache:
  82.6 ms/frame over 20 frames (1.65 s in one frame, frame max 1705 ms). Baseline, warm: 22.4
  ms/frame over 40 frames (the 943 ms frame).
- Cause: that system specializes a view's final blit (`BlitPipelineKey`: output format, blend,
  compositing space) on the first frame the key is new, then calls `block_on_render_pipeline`. The
  water probe's six faces (Rgba16Float image targets, spawned at Restore) are the first views with
  that output format. Their blit compile was queued behind the ~40 pipelines the Restore phase
  had just queued, and the render thread waited for all of them. A cold driver cache (first run of
  a new executable, which is what the owner gets with every new prototype) makes each compile
  slower.
- Fix: `blit_prewarm.rs` (render world, `Queue` set) specializes the blit for every possible
  target as soon as it exists, without waiting: every window surface view format and the view
  format of every image with `RENDER_ATTACHMENT` usage. It queues the replace blend and, where the
  adapter can blend the format, alpha blending, for the default compositing space and any space a
  camera uses. Depth and integer formats are skipped. The face images are made at scene build, so
  their blit compiles at loading. Specialization is cached by key, so the camera gets the same,
  already compiled pipeline later and the block returns at once. The image does not change: it
  uses the same pipeline, made earlier. This is shared for all tracks, cars, modes and image
  cameras (water probe, car reflections, cockpit mirror, render scale, offscreen captures),
  because the keys come from the targets that exist, not from a list. The first time it sees a
  format it prints `blit_prewarm format=F keys=N`.
- Test: `blit_prewarm::tests::colour_targets_get_replace_and_blend_keys_and_depth_or_integer_targets_none`.
- Side effect found [V] (A/B `hy-*`, `local/ab/lift.py`; `pipeline_warmup phase=… ms=` now prints the
  time since the warm-up started): without the block, the ~23 probe-view pipelines the Restore
  phase queues (pbr opaque/blend variants for the Hdr, MSAA-off face views, their tonemapping and
  post-processing) compile in the background while frames render, about 5x slower than inside the
  blocked frame (the last one waited 56 frames, ~1 s, against 169 ms). The worst covered frame fell
  from 0.7-1.4 s to 68-94 ms, but the cover lifted 0.3-1 s later. [I] The rendering frames compete
  with the compile threads.
- Compile yield: while the cover is up and the cache has waiting pipelines, the render thread
  sleeps 20 ms after each frame (`Cleanup` set, after `count`; nothing is visible but the cover).
  Restore-to-lift returns to the baseline's or below (baseline 1.8-1.9 s, Enhanced 2.6-3.0 s
  against 3.1-3.3 s before), at a covered frame of 91-119 ms. 50 ms lifted no sooner and made the
  covered frames slower. `GRID_WARMUP_YIELD_MS` sets the pause (0 = off), `GRID_BLIT_PREWARM=0`
  skips the pre-warm (comparison runs). Test:
  `pipeline_warmup::tests::render_thread_yields_only_under_the_cover_while_pipelines_compile`.
- Result [V] (F848, same executable with both changes off vs on, warm): worst covered frame
  baseline 87-692 → 87-89 ms, Enhanced 364-549 → 105-114 ms; cover time unchanged (5.3-5.7 s and
  3.7-4.2 s); cold driver cache: 711/1292 ms → 92-113 ms. Play after the lift unchanged. Cover time
  depends strongly on how warm the driver's per-executable shader cache is (2-3 s for an executable
  run dozens of times, 5-10 s for a new one).

### 2.4 Persistent pipeline cache: design only (F850)

Not implemented; the coordinator kept Bevy unpatched, the owner can decide later. It would bring a
new executable's cover time toward the warm 2-3 s; it does not change the frame rate.
- Hook: Bevy 0.19.1 creates every pipeline with `cache: None` (`render_resource/pipeline_cache.rs`,
  line 565 render, line 623 compute) and has no setting for it. A patched `bevy_render` would hold
  one process-wide `wgpu::PipelineCache` and pass it at those two sites.
- Support (wgpu 29.0.4): `Features::PIPELINE_CACHE` exists only on Vulkan (Windows, Linux,
  Android Vulkan). Metal's `create_pipeline_cache` does nothing (macOS keeps its own shader
  cache); DX12 and GLES do not implement it. Bevy requests all adapter features by default
  (`renderer/mod.rs:301`), so the feature is on where it exists.
- Validation: wgpu checks the header (magic, ABI, backend, adapter key, driver cache UUID).
  Outdated, corrupt or truncated data with `fallback: true` yields an empty cache, but a device
  mismatch is an error, so the file name carries `wgpu::util::pipeline_cache_key(adapter_info)`.
  wgpu has no payload checksum: store our own hash and length, write to a temporary file and rename.
- Cost: a `[patch.crates-io]` section, a seed refresh, an Android seed rebuild and a full-tier
  prepush for every Bevy update.

### 2.5 Cameras that are off until play needs them: the cockpit mirror (Mac dips, F923)

- [V] Owner's Mac log (M4, macOS 15.7.7, Donington BMW, revision 1ed1b05002e4, average 60+ fps):
  the three worst 1 s windows had one frame of 2315, 450 and 446 ms. Each time the worst render
  frame was all r_encode (2299, 438, 435 ms), and the per-camera split showed the mirror camera
  (order -1) as the first segment. The 2.3 s frame was the mirror's first frame ever, right after
  the switch from the bonnet camera to the head camera (`tools/frame_dips.py` lists them).
- Mechanism: r_encode includes `PipelineCache::process_pipeline_queue_system`. In Bevy 0.19.1
  (`render_resource/pipeline_cache.rs`, `create_pipeline_task`) macOS compiles every new pipeline
  with `block_on` on the render thread; Windows, Linux and Android compile on the async pool and
  skip the draw until it is ready (an object missing for some frames instead of a stall).
- Cause: the mirror camera (`cockpit.rs`) is created at scene build but stays `is_active = false`
  until the interior view shows a mirror. Its view key differs from the viewer's (Rgba8UnormSrgb
  image target, MSAA off, render layer 0), so none of its mesh pipelines were compiled under the
  warm-up cover; they compiled on the first cockpit frame (2.3 s on the Mac) and whenever a new
  material entered the mirror (the 450 ms frames).
- Fix: `pipeline_warmup::WarmWhileCovered`. A camera with the marker is kept active, looking from
  the viewer's eye, during the Plain and Dither phases (every mesh drawn and unculled, the fade and
  proxy variants), and handed back to its own system at Restore. The system that does it
  (`cover_cameras`) runs in PostUpdate before transform propagation: Bevy builds the directional
  light cascades and frusta for active cameras after propagation, and the render world unwraps a
  cascade entry for every extracted active camera (`bevy_pbr` light.rs), so switching a camera on
  later in PostUpdate (first attempt, inside the warm-up step) panicked on the first covered frame. The mirror carries it. Shared for
  every track, car and view; any future camera that is off until play needs it only adds the
  marker. The image after the cover is unchanged: the mirror's own system sets it active and places
  it on every frame it is shown, so a picture drawn under the cover is never displayed.
- Test: `pipeline_warmup::tests::warm_up_draws_unculled_then_dithered_then_restores_and_lifts_the_cover`
  (the marked camera is on under the cover from the viewer's eye, stays on against its system,
  off again at Restore).
- Result [V] on the PC: offscreen runs compile pipelines synchronously
  (`capture_pipeline_compilation synchronous=true`), like macOS, so they reproduce the stall. One
  Donington Lacetti drive per exe, 1500 ticks at 60 render fps, `--window-transitions
  600:C,660:C,720:C,780:C` (chase_close → chase_far → bumper → bonnet → head-cam),
  `GRID_PIPELINE_TRACE=1`, warm driver shader cache, no other cargo or grid process (private
  `local/ab/wt_ab.ps1`, runs `wt-a2`/`wt-b2`):

  | | before (F905) | after |
  |---|---|---|
  | pipelines at warm-up Done | 143 | 190 |
  | pipelines built after Done | 24 (20 on the head-cam frame, 3 later in head-cam, 1 screenshot) | 1 (screenshot) |
  | worst render frame after the head-cam switch | 1786.8 ms (encode 1768.2) | 55.0 ms (encode 31.9) |
  | warm-up cover time | 4.6 s | 5.7 s |
  | final head-cam frame (tick 1500, mirror in view) | — | 0 px differ, max delta 0 |

  The cover stays up about 1 s longer (warm driver cache). The fixed exe's first run, with the
  mirror's pipelines not yet in the driver cache, kept the cover up 17.0 s (old exe with all its
  pipelines cached: 3.5 s). Without the fix the
  same compilations happen mid-race on the first cockpit frame instead (2.3 s on the owner's Mac).
  Not yet confirmed on the Mac (docs/MACOS.md, "Capturing a log of frame dips").

### 2.6 Entities without a mesh asset under the cover: the crowd camera flash (Mac dips, F926)

- [V] Owner's Mac logs of 25 November 2026 (build 90af0c75, `GRID_PIPELINE_TRACE=1`, chase camera
  with view changes, cold and warm driver cache): both runs ended the warm-up with 188 pipelines
  and still built two after Done, both `alpha_blend_mesh_pipeline` used by
  `crowd/source-camera-flash`: id 188 (MSAA, main view) at the start and id 189 (no MSAA, the
  mirror) on the first head-cam frame. On macOS each compiles with `block_on` on the render thread.
- Cause: `crowd_billboards::install_flash` spawned its ten flash cards with an empty
  `Mesh3d(default())`; `update_flash` gives a card its first mesh (`flash_preview::event_mesh`) on
  the first flash. A handle without an asset is never queued, so the warm-up, which shows every mesh
  entity, had nothing to draw and the flash's alpha-blend variant was first specialised mid-race.
- Fix: each card now spawns with its own asset, `flash_preview::idle_mesh()`: the same vertex layout
  as every `event_mesh` (position, normal, UV, vertex colour, `u32` indices) with zero area and zero
  opacity. The warm-up shows it in every view like any mesh and builds the flash pipelines under the
  cover; after the cover `update_flash` hides idle cards as before, and `SoupMesh`-style in-place
  writes keep one asset per card. A card has no area, so nothing is drawn even where it is shown.
- Census: with `GRID_PIPELINE_TRACE=1`, `pipeline_warmup_meshless count=N empty_handle=E` counts,
  once at the Dither phase, the entities whose `Mesh3d` handle has no asset (E of them hold the
  default handle), and `pipeline_warmup_meshless_group` lines group them by visibility, standard
  material, parent and `Aabb` (release builds carry no component names). After Done,
  `pipeline_warmup_meshless_filled frame=F n=N` reports (checked once a second) how many of them
  have since received a mesh: each such entity can build pipelines mid-race, as the flash cards
  did; give its spawner a placeholder mesh of the real layout.
- Result on the PC (offscreen, synchronous compile like macOS; F926): the old exe already built the
  flash pipelines under the cover there (frames 8/20/32, the first flash came early), the new one at
  frames 2/15/28; Done at frame 43 with 190 pipelines in both, one pipeline after Done in both (the
  screenshot pass), images 0 px. Whether the miss happens depends on when the first flash falls
  relative to the cover, so the PC cannot show the Mac miss itself; the Mac run confirms it.

## 3. Evidence

Private, `local/blink/`: traces `t60.log` (before), `t60-fix*.log` (after), captures `w-*`,
`x-*`, and the detector `blinks.py`. Results are in F726.


## Diagnostic final capture readiness (G365)

The final offscreen Screenshot request now waits until Warmup is absent or Done.
The prior180-frame capture threshold alone could coincide with the black cover
or temporary all-level visibility. Update observes Done on the frame following
PostUpdate restoration/cover removal. This does not disable or retune warm-up.
Temporal/motion readback protocols still require their own ready-frame admission;
waiting only for the final PNG does not validate earlier sampled buffers.
