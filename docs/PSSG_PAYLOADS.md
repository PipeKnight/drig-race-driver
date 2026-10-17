# First verified PSSG geometry and texture payloads

Recorded: 6 October 2026, Europe/Kiev. This page retains the initial Lacetti geometry/base-texture evidence. Later [static car rendering](STATIC_VIEWER.md), [Donington rendering](DONINGTON_SCENE.md), [placements](ORNAMENT_PLACEMENTS.md) and [entities](ENTITY_COMPONENTS.md) extend that scope. Original-resource static scene assembly works; animation, shader fidelity and simulation remain ahead.

## Implemented scope

`grid-formats::pssg::read_tree` parses bounded child frames into flat preorder storage with parent/child indices. Schema element IDs resolve globally; attribute IDs also resolve globally, including attributes declared under a different element definition. Raw attribute bytes remain borrowed. Named integer/string helpers validate four-byte big-endian integers and exact length-prefixed byte strings.

The raw-data policy is explicit: BOUNDINGBOX, DATABLOCKDATA, DATABLOCKBUFFERED, INDEXSOURCEDATA, INVERSEBINDMATRIX, MODIFIERNETWORKINSTANCEUNIQUEMODIFIERINPUT, RENDERINTERFACEBOUNDBUFFERED, SHADERINPUT, TEXTUREIMAGEBLOCKDATA, TRANSFORM and SHADERPROGRAMCODEBLOCK. Other names are treated as containers and must contain valid child frames; an invalid child is never silently reclassified as data. These names originate in the pinned primary format reference; successful selected-file traversal supplies local structural validation, not semantic validation for every listed type.

Tree limits: depth 128, 250,000 nodes and 1,000,000 attribute values. Node frames/attribute blocks must fit their immediate parent; duplicate/unknown attribute IDs fail. Flat ownership avoids recursive drop. The original framing-only inspector remains available and keeps unknown payloads opaque.

`grid-formats::mesh::read_meshes` resolves local `#dataBlock` references and `subStream` ordinals. The supported subset is Vertex/float3, Normal/float3, ST/float2 and ushort triangle indices. Stream counts, buffer sizes, strides, element counts, finite positions/UVs, actual maximum indices and vertex bounds are checked. Color, SkinIndices, Tangent and Binormal remain in the tree without semantic decoding. Renderer/source IDs remain byte strings.

Limits: one million elements per decoded stream, four million total position records and twelve million total indices. Unsupported formats, missing streams, external references or unresolved blocks fail. This is a selected-car subset, not a universal EGO mesh loader. Instance stream/index slices, joints, skinning, transform composition, visibility variants and shader/material bindings are not applied.

`grid-formats::texture::read_textures` supports one Raw image block with dxt1/BC1 or dxt5/BC3 data. Dimensions, image-block count and declared block size are checked; dimensions must be positive multiples of four, at most 16,384. The base image is a borrowed compressed block range; the remaining mip data stays opaque. Native DDS export wraps only the base image in a standard DDS header.

## Actual Lacetti results

| Purchased resource | Verified result |
|---|---|
| `cars/lac/lac_highLOD.pssg` | 2,107 tree nodes, 34 render sources, 46,577 position records, 38,803 triangles |
| `cars/lac/lac_gen_high.pssg` | 41 tree nodes, 13 base textures |
| `cars/lac/livery_00/textures_high/lac_tex_high_00.pssg` | 20 tree nodes, 6 base textures |

All position, normal and UV float components and all ushort indices are big-endian in these decoded buffers. This is observed GRID data, not inferred from the host architecture. Position bounds across the raw sources are approximately X [-0.902412, 0.902495], Y [-0.797342, 1.617605], Z [-2.211350, 2.163438]. Units, handedness and assembled-space bounds have not been established.

Seven normal records contain NaNs in the source: the initial finite-component decoder rejected the file. The final reader preserves those normal values and reports their count; it still rejects nonfinite positions/UVs. Diagnostic OBJ export omits normals for an affected source rather than changing the original values. The viewer will need a documented fallback, such as computed geometric normals, before rendering these sources.

The independent Python survey and Rust OBJ export agree bit-for-bit after decimal-to-f32 roundtrip for every exported position, UV and normal, and agree on every triangle index/reference. All 19 DDS base images have payload bytes identical to their source ranges and decode successfully with the already installed Pillow 12.3.0. Visual inspection confirms a recognizable car shell and readable Chevrolet/livery textures. It does not establish the correct assembled car, material appearance or runtime fidelity.

The raw preview overlays all source geometry: parts are displaced/overlapping, source visibility and instance transforms are unapplied, and no materials are attached. The first preview's caption mistakenly counted the final drawn group; corrected preview `lac-highlod-preview-v2.png` reports all 34 sources. The original preview remains private as historical evidence.

## Texture mip uncertainty

For square textures, the observed `numberMipMapLevels` attribute matches the number of levels after the base, and the complete size matches the conventional BC chain including the base. Rectangular resources have additional bytes: for example 1024x512 DXT5 stores 701,120 bytes versus a conventional 699,088-byte chain. Their internal mip/padding layout is unresolved. The reader neither invents mip offsets nor includes that tail in the DDS output. Successfully decoded base images do not validate the whole mip chain, gamma/alpha semantics, sampler settings or shader use.

Microsoft primary references checked for BC layout and standard export framing: [block compression](https://learn.microsoft.com/en-us/windows/win32/direct3d10/d3d10-graphics-programming-guide-resources-block-compression), [DDS_HEADER](https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-header), [DDS_PIXELFORMAT](https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-pixelformat). These establish standard BC/DDS conventions; the PSSG embedding and GRID compatibility are our local observations. Pinned upstream PSSG provenance remains in [resource formats](RESOURCE_FORMATS.md).

## Coverage and unresolved leaves

The explicit tree policy accepts 37 of the 47 PSSGs in the captured race dependency set. Ten fail explicitly; this is unsupported payload classification, not evidence that those original resources are corrupt. Private `traced-tree-coverage.json` records every path/hash/exit/output.

| Unsupported resource | Observed leaf or failure |
|---|---|
| Donington `objects.pssg`, `ground_cover.pssg` | `PNSTRING` contains `DATA`, whose payload is not a child tree under the current policy |
| Lacetti and BMW interior/driver animations, character driver animations | Example leaf `NeAnimPacketData_L4_KA`; opaque animation payload not classified |
| `simplefonts/simple_fonts.pssg` | Older `TEXTUREIMAGE` payload differs from the supported block container |
| `anims/crowd_anims.pssg` | Unsupported child framing at 0x356e; exact data leaf still needs inspection |
| Donington `objecttypes.pssg` | `TEMPLATETRANSFORM` contains float-like raw bytes, not child headers; semantics remain unverified |

Do not broaden the data policy solely to make coverage pass. Inspect actual bytes and references, document opaque-versus-container classification, then add relevant synthetic tests.

## Validation and local evidence

13 synthetic tests pass: six prior framing tests and seven new payload tests. New cases cover global/shared attributes, BE float/ushort decoding, finite-value policy, bad stream/count/reference/index bounds, texture dimensions/size/format, DDS framing, all truncated fixture prefixes, parent-boundary escapes, duplicate attributes, unknown raw payloads and excessive nesting. Tests contain no purchased content. Formatting and Clippy with warnings denied pass. The existing OneDrive incremental-cache hard-link warning remains; builds succeed by copying.

Native output guards are exercised: exports outside canonical workspace `local/` are refused, and an existing OBJ is refused without changing its hash. No third-party Rust crate or additional tool was installed. The resource inputs were read only; the running game received no input during this decoding work.

Private evidence under `local/payloads/`:

- `lac-highlod-survey.json`, `lac-gen-survey.json`, `lac-texture-survey.json`: resource identities, global node/attribute metadata and source hashes.
- `rust-mesh-report.txt`, `rust-livery-textures.txt`, `rust-shared-textures.txt`: native results.
- `lac-highlod.obj`, `livery-dds/`, `shared-dds/`: purchased-resource derivatives; excluded from public source.
- `cross-validation.json`: exact comparisons, invalid-normal count, DDS hashes and decoded pixel hashes.
- `traced-tree-coverage.json`: 47-file tree coverage including unsupported paths.
- `lac-highlod-preview-v2.png`: inspected diagnostic plot from Rust-exported raw geometry.
- `outside-export-guard.txt`, `overwrite-guard.txt`: actual guard outcomes.
- `checkpoint.json`: final input/original-executable hash preservation and validation status at `2026-10-06T01:58:51.011435Z`.

Independent tools: `tools/survey_pssg_payloads.py`, `tools/validate_pssg_payloads.py`, `tools/preview_mesh.py`. Their Python syntax compilation and actual selected-file runs pass. The first survey incorrectly restricted attributes to the current element definition; switched to global ID resolution before successful traversal. Initial Clippy flagged constant chunk iteration; corrected it to the pinned Rust array-chunk API.

## Reproduce and next work

From the workspace with pinned Cargo on PATH, create private output directories first. Export commands refuse replacement and must be run from the workspace:

```powershell
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p grid-tools -- pssg-mesh 'C:\Program Files (x86)\Steam\steamapps\common\Grid\cars\lac\lac_highLOD.pssg'
cargo run -p grid-tools -- pssg-textures 'C:\Program Files (x86)\Steam\steamapps\common\Grid\cars\lac\lac_gen_high.pssg'
cargo run -p grid-tools -- pssg-obj 'C:\Program Files (x86)\Steam\steamapps\common\Grid\cars\lac\lac_highLOD.pssg' local\payloads\new-lacetti.obj
```

Render slices, rigid joint transforms, Donington static ancestry, placements and diffuse associations are now implemented; see the linked continuation reports. Unknown animation/older texture leaves remain unsupported. Continue collision, tuning/vehicle consumers and original shader fidelity; an independent playable lap remains pending.


## Rectangular full mip chains with trailing zeros — 17 October2026

The earlier base-only fallback for unknown rectangular tails was correct conservatism; it unnecessarily discarded the now-inspected contiguous mip prefix in some resources. Distant_trees_01_d.tga is1024x512 BC3:701120stored bytes =699088conventional bytes (11levels) +2032all-zero bytes. Every mip was independently decoded with the installed Pillow12.3.0 and inspected in a private montage; the tree shapes/alpha remain coherent at each level. This is trailing allocation data, not evidence that arbitrary interleaved padding can be removed.

Read-only `tools/audit_texture_mip_tails.py` inventories the original six Donington packs (treestextures,patchup_ot,tracksplit,land,sky,ground_cover), pins source SHA256 and writes only under ignoredlocal. 69rectangular BC textures:47full power-of-two chains have a small block-aligned all-zero suffix,8are exact-sized,14have nonzero excess and remain unsupported. Of the47,19are tree textures,20object maps,8track maps. These are format observations, not proof that every map's shader/colour/alpha semantics is understood. Evidence local/graphics/vegetation01/rectangular-mip-audit02.json; independent distant-conventional-mips.png/.json.

Reader extension only accepts a declared complete rectangular power-of-two BC1/BC3 chain with at most2048extra bytes, a whole-block residual and every residual byte zero. It copies the exact conventional prefix and drops only that suffix. Nonzero/oversized/nonaligned residuals, padded partial chains, impossible counts and truncation keep the base-only fallback. Existing exact-sized full/partial chains remain unchanged. New synthetic negative tests and purchased forest exact-prefix/base checks pass with the full22-test payload suite (zeroignored); grid-formats alltargetreleaseClippy1.59s. Source image pixels are not resampled, alpha is not regenerated and the source installation is unchanged. Common texture_upload::image remains the only diffuse GPU path, so extra validated mips must also be checked under forcedRGBA.

This may reduce distant texture aliasing; GPU motion proof is still pending. Remaining14nonzero layouts need separate research. Do not treat this as a universal rectangular-layout decoder. A scratch index loop ran past the final surveyed node once and was corrected; the public bounded audit avoids that guess.
