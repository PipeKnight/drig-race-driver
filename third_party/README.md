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
