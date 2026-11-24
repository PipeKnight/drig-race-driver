# Vendored upstream crates

Patched copies of upstream crates, used through `[patch.crates-io]` in the workspace
`Cargo.toml`. They are not workspace members (`exclude = ["third_party"]`), so the workspace
Clippy and tests do not run on them; the tests that cover each patch live in DRIG's own crates.
Each folder keeps its upstream licence files. Every changed line is marked with a
`// DRIG patch (F-number)` comment (`# DRIG patch` in `Cargo.toml`), so a Bevy upgrade can
re-apply or drop the patch: `grep -rn "DRIG patch" third_party`.

Files here keep the upstream bytes, LF line endings included: `.gitattributes` marks
`third_party/** -text`, and prepush's sanity step fails on a text file here that contains CR.
A CRLF `.wgsl` copy changes how Bevy's shader preprocessor reads `#ifdef` lines, and so the image
(F877). A checkout made before that attribute may still hold CRLF files; re-check it out with
`Remove-Item -Recurse third_party; git checkout -- third_party`. Copy new vendored crates from the
registry source (`~/.cargo/registry/src/`), not through `git apply`, which writes CRLF under
`core.autocrlf=true`.

## bevy_render 0.19.1

- Upstream: crates.io `bevy_render` 0.19.1, crate sha256
  `fc86a32b2eef9859e1e7d323c6d1a9150fe77aa17c30fd05abebc12f645929dc` (the `Cargo.lock`
  checksum), git commit `b56fc29d3016e641754765244b5ba3f9cc504671` of bevyengine/bevy, path
  `crates/bevy_render` (`.cargo_vcs_info.json`). Licence: MIT or Apache-2.0 (`LICENSE-MIT`,
  `LICENSE-APACHE`).
- Copied unchanged from the registry source, then patched (F875):
  - `src/render_phase/mod.rs`: `RenderMultidrawableBatchSetGpuBuffers` gets a `needs_upload`
    flag, true for a new batch set and set again by the only four places that change its two
    CPU-side buffers: `insert`, `remove` (an entity enters or leaves a bin),
    `allocate_indirect_parameters` (a new bin) and `remove_bin_if_empty` (a bin goes).
  - `src/batching/gpu_preprocessing.rs`: `write_binned_instance_buffers` uploads a batch set's
    `render_binned_mesh_instance_buffer` and `bin_index_to_indirect_parameters_offset_buffer`
    only while the flag is set or a buffer has no GPU allocation yet, then clears the flag.
    The GPU buffers keep their contents between frames and the shaders only read them
    (`unpack_bins.wgsl`).
- Why: Bevy retains bins across frames, but uploaded both buffers of every multidrawable batch
  set in every view and phase each frame. Each upload costs about 1.7 µs whatever its size, and
  96-98 % of them were unchanged (F875 probe: derby 1.30 ms, Donington 0.69 ms of uploads per
  frame). Measured gain: F875, docs/PERFORMANCE.md.
- `GRID_WBIB_ALWAYS_UPLOAD=1` restores the upstream upload of every batch set each frame, kept
  as a fallback and for A/B timing and image checks (docs/PERFORMANCE.md, F875).
- Test: `crates/grid-app/tests/bin_buffer_upload.rs` renders lit, shadowed meshes and then
  despawns, spawns with new meshes, moves entities to other batch sets, hides and shows them,
  and switches shadow maps off and on (shadow views go and come back); two steps only insert
  (a spawn into an existing bin, showing a hidden entity). Each change lands on one frame; the
  two frames after it and a later one must equal a fresh app's render of the same scene. It
  fails when the flag is dropped from `insert` (F875). It needs a GPU and runs in prepush's
  full tier.
- Upgrading Bevy: drop the patch if upstream uploads only changed bin buffers itself;
  otherwise copy the new release here and re-apply the marked changes, checking that no new
  code path changes the two buffers without setting the flag. After any change here, the
  coordinator refreshes the shared build seed (`tools/build_seed.ps1`).

## wgpu-core 29.0.4

- Upstream: crates.io `wgpu-core` 29.0.4, crate sha256
  `2f519832254e56965a9940c4af57dcb75f702b6f6fa4a0b172f685395843a4d7` (the `Cargo.lock`
  checksum), git commit `e99f5305ded96ff7006f0714d043a7f735bd45c2` of gfx-rs/wgpu, path `wgpu-core`
  (`.cargo_vcs_info.json`). Licence: MIT or Apache-2.0 (`LICENSE.MIT`, `LICENSE.APACHE`).
- Copied unchanged from the registry source, then patched (F886):
  - `src/command/pass.rs`: `PassState` gets `bind_dedupe: Option<BindDedupe>`, two sets of bind
    group addresses (`Arc::as_ptr`). `set_bind_group` merges a bind group's buffer and texture usage
    states into the pass's usage scope only the first time the pass sets that group, and
    `flush_bindings_helper` registers its buffer and texture memory init actions only the first time
    the pass flushes it. The raw `set_bind_group` still runs on every flush, and the bind group
    is still added to the command buffer's tracker on every set.
  - `src/command/render.rs`: render passes start with `BindDedupe::for_render_pass()`.
  - `src/command/compute.rs`: compute passes start with `None` (unchanged path; they merge per
    dispatch).
- Why: when a render pass alternates two bind groups, upstream redoes both steps on every switch,
  one tracker merge and one init-action check per resource. A bindless material slab holds 60-100+
  textures, and DRIG's transparent phase alternates each car pane with its reflection overlay
  (about 140 switches per frame in Donington), so the encode on the render thread paid this many
  thousand times per frame. Re-merging the same states changes nothing (the union is idempotent, and
  a conflict would already have failed the first merge), and re-registering the same init actions
  only appends duplicates, which are filtered again at submit; the discard list cannot gain entries
  inside a pass. The pass's tracker holds each bound group's `Arc` until the command buffer is done,
  so an address is not reused within the pass. Measured gain: F886, docs/PERFORMANCE.md.
- `GRID_WGPU_BIND_DEDUPE=0` restores the upstream path, kept as a fallback and for A/B timing and
  image checks.
- Test: `crates/grid-app/tests/bind_dedupe.rs` renders overlapping alpha-blended quads whose
  transparent sort alternates two material types (pipeline and bind group switch on every draw),
  textured with images the GPU first sees in that frame, once with the dedupe and once with
  `GRID_WGPU_BIND_DEDUPE=0` (two child processes), and requires identical frames. It needs a GPU and
  runs in prepush's full tier.
- Upgrading wgpu: drop the patch if upstream stops re-merging and re-registering a bind group already
  used in the pass; otherwise copy the new release here and re-apply the marked changes
  (`grep -rn "DRIG patch" third_party/wgpu-core`). After any change here, the coordinator refreshes
  the shared build seed (`tools/build_seed.ps1`).

## bevy_pbr 0.19.1

- Upstream: crates.io `bevy_pbr` 0.19.1, crate sha256
  `244ae7d618b51a59c913c36b0564a295cd85ea91a5e777b542bb7bdb846a23d6` (the `Cargo.lock`
  checksum), git commit `b56fc29d3016e641754765244b5ba3f9cc504671` of bevyengine/bevy, path
  `crates/bevy_pbr` (`.cargo_vcs_info.json`). Licence: MIT or Apache-2.0 (`LICENSE-MIT`,
  `LICENSE-APACHE`).
- Copied unchanged from the registry source, then patched (designed in F874, landed in F893):
  - `src/render/drig_bind_group_cache.rs` (new): `BindGroupCache`, keyed on the layout and, per
    entry, the binding index and a clone of the bound buffer (with offset and size), sampler or
    texture view. A clone keeps its resource alive, so a reallocated buffer, another range or a new
    view always gives a new key, and a bind group is never handed out for other resources. Entries
    not used in the last two frames are dropped (Bevy swaps two preprocessing buffer sets, so a
    bind group returns every other frame). Binding arrays, acceleration structures and external
    textures bypass the cache. `CachingRenderDevice` wraps `RenderDevice` for the callers.
  - `src/render/gpu_preprocess.rs`: `prepare_preprocess_bind_groups` creates its bind groups
    (bin unpacking, frustum culling, indirect parameters, reset of indirect batch sets) through
    the cache.
  - `src/light_probe/generate.rs`: `prepare_generated_environment_map_bind_groups` reuses a light
    probe's bind groups while its input key (texture, view, sampler and layout ids, sizes) is
    unchanged; Bevy never reuses these ids.
  - `src/render/mod.rs`: declares the module. `Cargo.toml`: `wgpu` dependency (the key holds wgpu
    handles).
- Why: Bevy recreated about 160-260 of these bind groups per frame (F873). Each creation and drop
  goes through wgpu's device and hub locks, the Vulkan descriptor-allocator mutex and the
  pending-writes path, which other render systems take at the same time (F886 profile: create_bind_group
  24.8 % of the wgpu samples, much of it spinning on those locks). The cache reuses 72-77 % of them
  (F874). Measured gain: F893, docs/PERFORMANCE.md.
- `GRID_BEVY_BIND_GROUP_CACHE=0` restores the upstream path, kept as a fallback and for A/B timing
  and image checks.
- Tests: `crates/grid-app/tests/bind_group_cache.rs` (a cached bind group only for an identical
  layout and resources; a reallocated buffer, another range, a new view or a new layout gives a
  new one; a bind group returning every other frame still hits) and
  `crates/grid-app/tests/environment_map_switch.rs` (switching a camera's environment map, as
  between tracks, changes the generator bind groups each time and keeps them between switches).
  They need a GPU and run in prepush's full tier.
- Upgrading Bevy: drop the patch if upstream keeps these bind groups itself; otherwise copy the new
  release here and re-apply the marked changes (`grep -rn "DRIG patch" third_party/bevy_pbr`).
  After any change here, the coordinator refreshes the shared build seed (`tools/build_seed.ps1`).
