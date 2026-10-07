# First resource inspectors

Recorded: 6 October 2026, Europe/Kiev. This page retains the first framing scope. Subsequent [PSSG payload decoding](PSSG_PAYLOADS.md), [static viewer](STATIC_VIEWER.md), [Donington scene](DONINGTON_SCENE.md), [placements](ORNAMENT_PLACEMENTS.md) and [entity records](ENTITY_COMPONENTS.md) describe the implemented extensions. Static car/track/scenery rendering works; a playable driving engine remains ahead.

## Verified scope

The framing CLI reads local resources without modifying or extracting them. New explicit `pssg-obj`/`pssg-dds` commands export diagnostic derivatives only into private workspace `local/`, refusing overwrite; inputs remain read-only. Names and opaque attribute values remain borrowed byte slices in the library; CLI names are escaped, lossy UTF-8 previews. Text encoding, units and game behavior are not inferred from those previews. Paths use native OS strings, including Cyrillic paths.

The original `pssg` command covers raw magic, declared size, schema definitions, root node and root attribute framing, with opaque remaining payload. Additional `pssg-tree`, `pssg-mesh` and `pssg-textures` commands implement the verified subset documented in [payload decoding](PSSG_PAYLOADS.md). Transform composition, materials, animation, gzip/XML variants and complete mip layout remain unresolved.

JPK support covers the observed `JPAK` header variant with the second word zero, entry table, NUL-terminated name locations, stored byte ranges and declared uncompressed sizes. Other header words are preserved as unknown. No decompression, collision interpretation or extraction is claimed.

## PSSG framing

All integers below are unsigned big-endian 32-bit values in the supported subset.

| Offset or sequence | Meaning checked against actual files |
|---|---|
| 0 | Four bytes `PSSG` |
| 4 | Declared size equals file length minus 8 |
| 8 | Total attribute definition count |
| 12 | Element definition count |
| Each element definition | ID; length-prefixed name bytes; attribute definition count |
| Each attribute definition | ID; length-prefixed name bytes |
| Root frame | Element ID; size measured after that size field; attribute block size measured after its size field |
| Each root attribute | Attribute ID; byte length; uninterpreted value bytes |

The reader requires unique schema IDs in the declared one-based ranges, consistent counts, root ID in the schema, root end at file end, and attributes wholly within both their block and root. These are supported-subset checks rather than a claim about every EGO version. A payload that looks like another node is not automatically classified as one.

## JPK framing

Integers are unsigned little-endian 32-bit values. Header is 32 bytes: magic, variant word, entry count, alignment word, and four uninterpreted words. Each 32-byte entry holds name offset, declared uncompressed size, stored-data offset, stored size and four uninterpreted words. Offsets are absolute from file start. Observed alignment is 16; the inspector reports it without inventing padding/compression semantics.

The reader checks table size before traversal, requires name/data locations outside the table for nonempty data, checks stored ranges against file size, and requires a NUL within 4096 name bytes. JPK is capped at 100,000 entries, PSSG at 100,000 elements and 1,000,000 attribute definitions. The CLI limits input to 256 MiB, including files that grow while read. These are explicit inspector limits, not format constants.

## Local validation

Native build and six synthetic-fixture tests pass on Rust 1.99.0/MSVC. Tests cover successful opaque-value/range preservation, every truncated fixture prefix, oversized counts/lengths, invalid/duplicate IDs, wrong magic, unsupported variant, table overlap, out-of-file offsets and unterminated/overlong names. Fixtures contain no purchased game data.

Local integration coverage: **222/222 accepted** — all **29 PSSGs under `cars/lac`**, and all **193 JPKs** in this installation. This establishes framing compatibility for that set, not correctness of unread payload semantics. Individual file hashes, header summaries and exit codes are private in `local/formats/coverage.json`; representative detailed outputs and hashes are in `local/formats/validation.json` and adjacent `.txt` files.

| Representative resource | Result |
|---|---|
| `cars/lac/lac_gen_high.pssg` | 4,068,200 bytes; 238 elements; 436 attribute definitions; root ID 169; 5 root attributes |
| `cars/lac/lac_highLOD.pssg` | 3,366,516 bytes; 124 elements; 298 attribute definitions; root ID 4; 5 root attributes |
| `tracks/europe/donington/route_0/track.jpk` | 1,842,752 bytes; 245 entries; alignment 16 |
| `raceload.jpk` | 80,880 bytes; 13 entries; alignment 16 |

The Donington container includes `.vcqtc` entries; their collision role and internal format remain unverified. `raceload.jpk` lists loader and physics/material resources, including `car_loader.xml`, `track_loader.xml` and `physparams.xml`. An XML suffix does not establish textual XML. The later race trace confirms reads from the containers, without identifying which internal entries were consumed; see [dependency map](REFERENCE_DEPENDENCIES.md).

Car tuning CSV is headerless and has many fields. Field meanings, units and consumers remain unknown; no physics implementation is derived from guessed column meanings.

## Provenance

Read primary upstream templates and readers from [Ego Engine Modding](https://github.com/EgoEngineModding/Ego-Engine-Modding/tree/f3fe9ee0f6e8379c64bc4a29e1d1b586bba5d1c1), pinned at commit `f3fe9ee0f6e8379c64bc4a29e1d1b586bba5d1c1`: `src/010 Templates/pssg.bt`, `jpk.bt`, and `src/EgoEngineLibrary/Graphics/Pssg/{PssgFile,PssgSchema,PssgElement}.cs`. Upstream repository license: MIT, copyright 2017 Petar Tasev. Downloaded reference files and tree/blob IDs remain private in `local/references/ego-tools/`.

Our readers are independently written from observed framing and the primary format references. No upstream source was copied into these crates. The PSSG template describes DiRT 3 testing; GRID validation above is our own. Upstream heuristics for identifying data nodes are not treated as a verified GRID schema.

## Reproduce and continue

From the repository root, with Cargo on PATH:

```powershell
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo run -p grid-tools -- pssg 'C:\Program Files (x86)\Steam\steamapps\common\Grid\cars\lac\lac_gen_high.pssg'
cargo run -p grid-tools -- jpk 'C:\Program Files (x86)\Steam\steamapps\common\Grid\raceload.jpk'
# After cargo build, using the available Python:
python tools\validate_resource_readers.py 'C:\Program Files (x86)\Steam\steamapps\common\Grid'
```

The observed dependency set, rigid/static scene assembly, textures and placed scenery are delivered. Bevy is isolated to `grid-app`; these format readers remain standard-library-only. Continue collision geometry, binary XML, tuning semantics and targeted consumer inspection. Dynamic simulation remains unimplemented.
