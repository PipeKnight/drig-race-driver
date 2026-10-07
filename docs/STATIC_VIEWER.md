# Static Lacetti assembly and first Bevy viewer

Checked 6 October 2026. This is an independently authored renderer reading the owner's original Steam resources. It is not yet a driving engine.

Continuation: reference/start views now derive horizontal heading from the original slot rectangle's A−C direction, instead of assigning its raw transform to the car. All 21 markers independently match bit for bit, and the corrected close image was inspected. Marker-origin position remains a diagnostic anchor; original vehicle-specific offsets/contact height are pending. [Consumer chain and evidence](PROGRESS_AND_SPAWN.md).

## Verified geometry subset

`lac_highLOD.pssg` contains 183 MATRIXPALETTERENDERINSTANCE draws under two MATRIXPALETTEBUNDLENODEs. Selecting `LOD0_` yields 98 draws / 22,688 triangles; `LOD1_` yields 85 draws / 16,115 triangles. Rendering both simultaneously would overlay the two levels.

Each draw specifies streamOffset/elementCountFromOffset and indexOffset/indicesCountFromOffset. Index values are source-buffer indices; subtracting streamOffset rebases the selected range. Every selected vertex's float SkinIndices value equals its parent MATRIXPALETTEJOINTNODE jointID, for all 183 draws. This supports a single rigid joint transform per draw in this static pose. It does not establish a general weighted skinning or animation algorithm.

TRANSFORM stores 16 big-endian f32 values. The observed row-vector affine convention places translation at values 12–14. Transforming a point uses `out[j] = x*m[j] + y*m[4+j] + z*m[8+j] + m[12+j]`. Root and bundle matrices are identity; the reader explicitly rejects nonidentity ancestry until composition is implemented. NaN source normals are preserved by the parser; the viewer computes geometric smooth normals for affected draws (two LOD0 draws).

LOD0 assembled bounds are approximately (-1.010546, -0.000615, -2.235955) to (1.013800, 1.617605, 2.244294). These are file-coordinate measurements, not an independently verified physical unit scale. The first GPU image shows a coherent car, aligned wheels/panels and readable livery. No damage visibility, animation or fidelity comparison is claimed.

## Material evidence

Car SHADERINSTANCEs contain constants but no saved diffuse/normal texture assignments. The private unpacked executable contains pointer triples `(texture suffix, sampler name, material name)`. Functions containing references to these tables and their direct callees were inspected in the existing partial Ghidra project. Consumer `0x009ca780` advances the pointer by three entries until an empty suffix, resolves repeated suffixes, and forwards the second/third pointer with a resolved texture value. Its caller passes body and wheel tables; other call sites pass suspension and tread tables. This confirms the table structure statically; exact runtime selection/path assembly remains untraced.

The viewer applies these diffuse associations:

| Material names (trimmed) | Texture ID |
|---|---|
| Car Body Paint, Matt, Shiny Metal, Bare Metal, Aerials | lac_main.tga |
| Window Glass, Grills Glass | lac_glass.tga |
| Cabin | lac_cabin.tga |
| Wheel, Tyre Wall | lac_wheel_tm.tga |
| Tyre Tread | lac_tread_tm.tga |
| Suspension | lac_susp.tga |
| Light pods/glass, Discs, Grills | lac_lights.tga |

These are a provisional rendering specification, not copied executable code. Roughness, metallic values, ambient light, glass blending and shadows are chosen viewer approximations. Normal/dust/crack/scratch/emissive maps and original shaders are not reproduced. Only validated BC1/BC3 base images are loaded; opaque mip tails remain unused.

## Build and usage

Stable Bevy **0.19.1** is pinned exactly; Rust **1.99.0**, MSVC **14.44.35207**, SDK **10.0.26100.0** already existed and were reused. Official stable metadata and upstream Cargo manifest are retained under `local/references/bevy/`. Sources: [Bevy release notes](https://bevy.org/news/bevy-0-19/), [pinned upstream manifest](https://github.com/bevyengine/bevy/blob/v0.19.1/Cargo.toml). Cargo.lock pins the resolved graph; no replacement OS graphics driver was installed.

From the workspace with Cargo available:

```powershell
cargo run -p grid-app --locked
cargo run -p grid-app --locked -- --render local/scene/my-car.png
cargo run -p grid-app --locked -- --lod LOD1_ --render local/scene/my-car-lod1.png
```

`--game-root PATH` selects a different owned installation. `--scene donington` shows the track/scenery; `--scene reference` adds Lacetti at the composed `slot_00` anchor. `--view overview|start` chooses framing; paired `--camera X,Y,Z --look-at X,Y,Z` specify it exactly. Interactive controls: arrows orbit, WASD pan, Q/E vertical, Shift faster, PageUp/PageDown zoom, Home reset, Escape exit. They are implemented but live validation is pending: desktop Computer Use app approval timed out, and no input was sent. Headless capture uses the same GPU scene and exits after writing one 1280×800 frame. Output directories must already exist beneath canonical `local/`; existing files are refused. Inputs are bounded and read only.

## Validation and private evidence

- Initial empty Bevy build passed in 4m56s; first viewer compile exposed the renamed GlobalAmbientLight resource and was corrected using cached upstream API definitions.
- `local/scene/lacetti-lod0-first.png`: first successful image, visually inspected. 98 draws / 22,688 triangles / 19 textures, exit 0. `local/scene/lacetti-lod0-first-run.txt` records counts.
- `local/scene/lacetti-lod1.png` and run log: inspected lower-detail comparison; 85 draws / 16,115 triangles / four normal-fallback draws, exit 0.
- Synthetic format tests: 15 pass, including rotation plus translation, rebased triangle indices, missing LOD, mismatched skin IDs, range/source errors and fractional/nonfinite skin values. Proprietary assets are excluded from public fixtures.
- `local/scene/static-texture-tuples.json`, `texture-table-code-candidates.json`, `texture-survey/`, `texture-consumers/`: private binary evidence and inferred pseudocode. No broad-analysis completion claimed.
- Adapter reporting, Clippy final status and preservation checkpoint are recorded when completed in the master log.

Continuation: [Donington surface decoding and rendering](DONINGTON_SCENE.md) now work. The viewer can use complete BC chains when exact packed size and declared count agree; otherwise it retains base-only textures. Collision remains a separate resource investigation.
