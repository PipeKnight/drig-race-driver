# Donington static scene investigation

6 October 2026. Purchased input `tracks/europe/donington/tracksplit.pssg`, 20,781,247 bytes, SHA-256 `7e83a37ec2b9d85722a7208791a17708e0f9ed57ee7cb9c193a6761b8157c476`. The race-load trace confirms access to this file; its contents alone do not identify collision or the chosen racing route.

## Actual buffer and instance observations

The explicit PSSG tree contains 9,348 nodes, 413 mesh sources, 413 render instances, 45 textures and 97 shaders. Rust decodes 255,276 position records and 230,784 triangles across all sources. Vertex positions/normals are BE float3. UVs are BE IEEE binary16 pairs, with 708 ST streams across the sources. Multiple channels are preserved in render-stream order. Some sources omit normals and UVs; absent streams remain empty rather than fabricated by the parser.

Ordinary RENDERSTREAMINSTANCEs use the whole indexed source. Their child RENDERINSTANCESOURCE agrees with their indices reference. NODE, RENDERNODE and ROOTNODE transforms compose in child-then-parent row-vector order. All draw ancestry in this file is identity. The sole translated helper node `static_env_map` has no draw. World bounds are approximately (-632.99005, -42.672382, -403.58237) to (882.5755, 103.53328, 275.6742).

Saved SHADERINPUT parameterID indexes the corresponding SHADERGROUP's ordered SHADERINPUTDEFINITION children. The reader validates counts, duplicate parameters, type agreement and group references. All 45 saved texture references resolve to textures embedded in this file. The initial viewer uses only TDiffuseAlphaMap and the first UV stream. Macro color, layered diffuse maps, vertex color/AO, original shader controls and decals remain incomplete.

## Duplicate batch path

48 sources/instances use `batchmaterial` / `batched_track.fx`, omit normals/UVs and contain 115,391 triangles. The other 365 sources contain 115,393 triangles. An independent Python comparison sorts each triangle's three exact position byte strings and compares multisets: **all 115,391 batch triangles appear in the ordinary set**. This is coordinate equivalence, not proof of the original batch visibility/selection algorithm. Metadata: `local/scene/donington-batch-overlap.json`.

Rendering both paths produced overlapping untextured geometry (`donington-first.png`); this was inspected and retained as a failed visual experiment. Default `--scene donington` renders the 365 ordinary draws and reports the 48 omissions explicitly. `--include-batches` retains the untextured diagnostic batch path. No geometry corruption workaround or arbitrary source deletion was applied.

## Viewer and validation

```powershell
cargo run -p grid-app --locked -- --scene donington
cargo run -p grid-app --locked -- --scene donington --render local/scene/new-donington.png
```

The surface-only GPU image `local/scene/donington-surfaces.png` was inspected: circuit footprint, terrain, service roads and paddock regions are visible. Buildings, barriers and other resources are not yet loaded. Base-only textures alias strongly at overview distance. Complete BC mip chains are enabled only when declared level count and exact packed block sizes agree; padded layouts keep base-only fallback. This size agreement and GPU use do not resolve every original mip packing rule.

`tools/validate_pssg_payloads.py` now independently handles binary16 via Python struct `e`, absent streams, multiple ST channels and separate OBJ UV/normal indices. All 413 Rust-exported source positions/first UVs/available normals roundtrip bit-for-bit, and all triangle/index references match. Source and OBJ hashes plus counts are in `local/scene/donington-cross-validation.json`; raw OBJ SHA-256 `8e3dc941e913a892f2fab58b70faf098b3aa65a5d287412bb65b6933d8b6c121`.

Synthetic tests cover all 65,536 half bit patterns, signed zero and nonfinite classes; affine parent/child order; shader ordinal/type/count checks; and complete-chain/fallback behavior. Nineteen format tests pass at this checkpoint. Final workspace lint and improved sampler/mip image are still pending.

Rendering backend observed: Bevy 0.19.1 / RTX 4070 / Vulkan / NVIDIA driver-info 616.92, BC support present. No native window/input demonstration or handling/collision claim yet. Continue with `objects.pssg`'s currently unsupported PNSTRING/DATA leaf, its transforms/materials and external texture resources.
