# Deformation initialization boundaries

13 October 2026, follow-up to [active-grid vehicle integration](DEFORMATION_SYNCHRONIZATION.md). This records the initial static investigation. Subsequent [layout proof](DEFORMATION_LAYOUT.md) and [source-derived original/Rust initialization proof](DEFORMATION_SOURCE_INITIALIZATION.md) supersede its earlier unexecuted status. Bounded mode-zero cells/edges are verified; source selection and dirty vehicle integration remain pending. The playable build remains `f428d0a23ce8`; its meshes remain rigid.

## Separate stages

The aligned original `9A2A70` takes the grid descriptor in ECX and a separate source object on the stack. It copies that object's `44C` settings link, sets descriptor mode to -1 and enables its initialization flag. `989490` collects bounds through a count at object `44` and pointers starting at `48`, reading bounds records at `110` and `120`. The object's type and its complete source construction still need proof; these offsets alone do not identify it as the runtime physical body or the already supported `collision_main` mesh.

Layout expands the bounds using a file-backed scalar, derives an initial spacing from an SSE-refined diagonal length, then repeatedly grows spacing until the three truncated span/spacing values plus two have product at most 400. It recenters the minimum along XYZ. The aligned fallback test broadcasts the comparison's X lane before taking its mask; the earlier inferred pseudocode obscures this. Zero/empty/inverted bounds and the exact fallback must be controlled in the eventual native proof, rather than replacing them with an authored bounding box.

Whole `98A7D0` then initializes active current/rest node positions, clears each active node's `30` vector, writes its flag byte FF, and clears descriptor offset `A440`. It does **not** initialize node `20` displacement bounds, node `40` marker or the edge list. Inactive padding bytes survive. Consequently, generating this regular lattice alone does not create an impact-ready deformation model.

Mode transition `9FEC70` supplies the second stage. Aligned code confirms that switching to mode zero without a donor pushes the settings link and calls `9EBEC0` with the grid receiver. A donor instead copies exactly 32,000 node bytes. Both paths then settle the grid with `9A26F0` and set all five dirty bytes `A4F0..A4F4`. Same-mode calls skip this work.

The current `9EBEC0` decompilation is evidence for dependencies, not a trusted calling convention: its inferred signature omits the pushed settings argument. It initializes additional node accumulators, visits grid cells, calls geometric helper `9E7CE0`, reads settings fields `1014` and `1018`, and derives node limits/markers before restoring current positions from rest positions. Exact settings ownership, geometry inputs, lane arithmetic and whole output still require aligned/native validation. They must not be replaced by guessed marker values or arbitrary stiffness.

Aligned `9FED60` first calls `9FEC70`, clears three descriptor accumulators and `A410`, then traverses the grid to build edges. This is the third stage. Its inferred topology visits Z/Y/X, considers positive-X, positive-Y, then positive-Z neighbors, and retains a candidate only when both node `40` markers are strictly positive. It appends three `(399,399)` self-edges, calls the length producer `9A2110`, and sets `A4F0` and `A4AE`. Candidate endpoint writes precede the acceptance test, so a future complete-buffer port must preserve writes even for rejected candidates; reconstructing a filtered edge vector alone would miss that observable storage behavior.

`tools/inspect_deformation_edges.py` independently reconstructs this ordered topology from two hash-pinned paused snapshots. All **24 retained grids / 21,008 edge pairs / 84,032 pair bytes** match exactly, including the three terminal self-edges in each grid. The 360-node pools have 334 positive markers and 847 edges; 390-node pools have 360 positive markers and 915 edges. Repeating the analysis produces a byte-identical report. This checks retained topology consistency only: it does not compare length words, execute original `9FED60`, derive markers from source geometry, or port the initializer to Rust. The self-edges' intended purpose is not assigned.

## Verified caller identity and evidence

Original address references identify layout calls at `A0FE1D` inside `A0F710` and `9EBD33` inside `9EBCD0`; `A08040` calls the latter. Aligned instructions prove both pass body+2300 as the grid receiver. At `9EBD33`, the separate source object is the receiver's prior ESI; settings are assigned body+D240 and its `AA48` link is assigned body+C8A0. Object/view identities must remain distinct.

`A0F710` is a real function start after the preceding function's return/padding. Its fresh inferred decompilation prunes many branches and omits the layout call even though aligned bytes and the independent address-reference index show it. No model or caller conclusion is based on that omitted pseudocode path.

Private evidence, all under ignored `local/behavior/`:

- `deformation-construction-aligned-20261013.json` and `deformation-construction-constants-20261013.json`: exact layout/node initializer instructions and six hash-pinned file-backed scalar words. These are analysis-image values, not freshly captured live globals.
- `deformation-construction-owner-aligned-20261013.{json,txt}` and `deformation-construction-owner-call-20261013.{json,txt}`: function boundary and both actual layout call sites.
- `deformation-mode-initialization-aligned-20261013.{json,txt}`: real mode-transition call setup and start of edge construction. Its extra interior tail range is instruction context only, not a whole-function boundary.
- `20261013T0953434555955Z/`: existing Ghidra 12.1.4/JDK 21.0.9 layout-owner/mode/reset surveys.
- `20261013T0956370824629Z/`: shape/preparation helpers and `9EBEC0` survey.
- `20261013T0957163632538Z/address-refs.tsv`: independent caller references.
- `20261013T1000310651941Z/`: complete inferred edge-builder and geometric-helper surveys.
- `deformation-edge-builder-aligned-20261013.{json,txt}`: aligned topology gates, length call and final flags.
- `deformation-grid-edges-retained-20261013-final-01.json` and `...repeat-01.json`: identical topology reports, SHA-256 `381861e7bddf5f6bf73aa26a22a2ec44adedfcd4bf3dbd31651c2fb26f45a3cb`.

Existing Python 3.12.14 and Capstone 5.0.6 were reused. An initial scalar-inspector call incorrectly supplied `address:length`; this tool accepts bare addresses, so it rejected before writing output. The corrected call produced the retained scalar report. A few guessed old note/evidence paths failed to read and were corrected by file inventory. The authorized headless Java analyses needed their existing user preference access; no automatic approval was rejected, no tool was installed and no game input/settings/Steam file changed.

## Implementation sequence

1. Execute whole `9A2A70`/`989490`/`98A7D0` against owned bounds-bearing records, with canaries and restored image data. Compare complete descriptors, including unchanged opaque/padding bytes, against an independent bounded Rust layout initializer. Cover empty/inverted/order/signed-zero/odd-dimension and spacing-loop cases.
2. Establish how the purchased Lacetti source constructs those records and settings. Reuse the supported NED geometry only after proving the original object selection, coordinate offset and shape coverage; compare against retained 360-node Lacetti grids as an observation, not a substitute for initialization.
3. Execute and port the complete mode-zero node-limit/marker initializer and edge builder, preserving original geometric queries and separate supplied settings. Then connect dirty wheel geometry/setup rebuilding before admitting dirty grids into the car job.
4. Connect the already verified contact-to-record/body/solver/response pipeline to the registered physical contact boundary with its separate random state and transactional output. Map authoritative grid samples to visible mesh vertices afterward, with native and render evidence kept separate.

No owner action is needed for these offline boundaries. Terminal damage notifications, source factories and live scheduler cadence retain their existing explicit limits.
