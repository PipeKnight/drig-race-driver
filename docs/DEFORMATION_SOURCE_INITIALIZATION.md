# Source-derived Lacetti deformation initialization

13 October 2026, follow-up to [layout validation](DEFORMATION_LAYOUT.md). Independently read source inputs, passed through the **whole original initializer and independent Rust implementation**, reproduce all **32,000 bytes of the pristine player node pool**, all **847 edge records / 6,776 bytes** and all **52 adjacent captured settings bytes**. Complete reset/layout/mode-zero/edge output now matches. No saved native state is used by runtime startup. Original resource selection, dirty vehicle rebuild and visible mesh deformation remain unfinished.

## Inputs and original execution

`prepare_deformation_initialization.py` reads purchased `cars/lac/lac.ned` FEMS/strength_main: 24 BOX matrices, half sizes, crush/crY/crZ/damage and the known FEMS scalar fields. Static original `99CC00` shows a maximum 32 records, matrix/size defaults and four scalar defaults; `9EFB30` supplies the settings mapping and derived reciprocal normal-map distance. Editor-extra enable gating and the wing-pushout half multiplier are preserved. The original tokenizer/resource loader has not been executed by this diagnostic.

The bounds candidate remains explicitly highLOD LOD1 excluding the aerial. The whole original probe executes `9A2E30 -> 9A2A70 -> 9FED60`, including reached `9FEC70`, `9EBEC0`, `9E7CE0`, `A15F50`, `9CC790`, `9A26F0` and `9A2110` without helper replacement. Three complete A500 descriptors and complete 1054-byte settings are retained. Only the verified callback pointer A484 and the separately verified owned settings pointer A4F8 are canonicalized; arbitrary data words are untouched. Owned contexts/canaries and the entire restored mapped image pass. Settings remain unchanged; original/repeat outputs are byte-identical.

The result has dimensions **6 x 5 x 12**, **334 positive markers** and **847 edges**. Comparing to the later barrier-impact capture preserves rest positions, bounds, markers and padding; 334 nodes differ in current XYZW and velocity XYZ (2,338 words), and edge length words differ. These differences are retained rather than ignored or mislabeled an initialization mismatch.

Private baseline: `local/behavior/deformation-initialization-source-20261013-draft-01/` containing input/original/repeat buffers, execution and validation reports, and `retained-comparison.json`.

| Identity | SHA-256 |
| --- | --- |
| Source NED | f71ce1224b10454f9fa76ee20af791e5f8e25eea160b6e1f76ff2bdffb2a26ba |
| Source highLOD PSSG | 963f347d07d76ffea2ac8a26896172487a9b8054a6165c8a63fb8c62d0aca9e0 |
| GI01 input | a498f94bc26135d75e426f369f4307abf266303ee1a737b84c8cc185d3bfc99b |
| Original/repeat output | 1ed4cc2f2fc89e9b016981773413f73846ac19a37420516ac9024de4f93d6ee1 |
| Native initializer probe | 5945b6604db3a1f4a22d7d8e130f2ed04fe5dd0b02d49141f50134a4fbf96c7b |

Retained pristine capture `general-vehicle-manager-20261008T2213398744933Z.json` SHA `fdacccfec8fb847cf086129adf7cd5a8c8add585bf036163f16eb463e9532aa2`; barrier capture `...2215472748164Z.json` SHA `a71e5409c25dc6841906b0f2e65aa2b9ea89b9f9d86fce6ab788a75ba14c55c3`. Both remain ignored private evidence.

## Independent initialization proof

`deformation_initialization` owns reset, layout and final A500 descriptors, consuming supplied strength/settings words, bounds and scalar parameters; callback/settings values are opaque tokens. It integrates cells against strength boxes, distributes ordered damage/volume/crush to eight corners, applies reciprocal refinement and limits/marker clamps, restores current positions from rest and constructs the complete edge buffer. Rejected endpoint writes and three terminal self-edges survive. Zero incoming edges is an explicit constructor precondition: the intervening settling leaves these restored positions unchanged.

Each profile matches original/repeat/Rust for **36 cases / 4,712,400 complete bytes**, covering source-derived Lacetti/BMW candidates, zero/one-box geometry, zero damage/crush, changed compression/damage multipliers, scaled/shifted bounds, margins and finite arbitrary padding. Coverage: 13,465 active nodes, 9,557 positive markers, 24,069 edges. BMW inputs validate numerical construction, not retained AI factory state. The widened retained comparison checks three adjacent regions 101C..104F (52 bytes); the earlier baseline compared 44 bytes.

19 guard cases / 53 rejection or output-preservation outcomes per profile pass. The read-only audit reconstructs requests, verifies complete answers/retained comparisons, all 41 files, five source identities and current executable/image identities. [Cell volume validation](DEFORMATION_CELL_VOLUMES.md) checks the reached geometry separately. 193 regular library tests pass, including a late terminal-edge numeric failure preserving complete input after layout/cell/topology work. Formatting, all-target Clippy and Python AST checks pass.

Final reports under `local/behavior/`:

| Report | SHA-256 |
| --- | --- |
| deformation-initialization-debug-20261013-final-02/validation.json | bca505513d9c28030ca6a463ee283bf0da84367c9c7083e86f9d92e453e7cf60 |
| deformation-initialization-release-20261013-final-02/validation.json | 67ac28910fee5feab895d02a5d7e8913fb1c152c0bbe3583ea027af3aea0b546 |

Rust debug SHA `a1a10805123240bf6cc65073c1a347062b7c8508908e4812eba20d42fe8de01f`; release `7984528554c6b5472cf48c7dd1d3fbde31661e431fea71a5c6e97453eba87f13`. Native probe retains its baseline hash. Final-01 completed numerical comparisons but its guard runner incorrectly expected native error9 for an oversized bounds count; original framing returns8. Final-02 corrects only the expectation. An earlier retained-comparison draft treated a block dictionary as a list and used a nonexistent block key; corrected to documented names. Failed reports remain private; no arithmetic tolerance, runtime guard or instruction was relaxed.

## Remaining integration

Mode zero visits cells and intersects their six planes with each oriented strength BOX's six planes. `A15F50` constructs the intersection vertices and ordered triangular faces; `9CC790` accumulates their positive volume. `9E7CE0` weights volumes by damage and axis crush values; its x87 damage/total-volume accumulation and SSE axis accumulation must retain distinct rounding. `9EBEC0` distributes these cell results to node limits and markers. The complete source baseline is available to check this work without live desktop input.

The fresh private surveys `20261013T1244006562464Z/{a15f50,9cc790}.c.txt` expose the geometry boundary; inferred pseudocode is evidence, not independently authored engine source. Ordered intersections, duplicate handling, face order, rounding and full node/edge output now pass bounded comparisons. Next implement the Rust source reader and dirty wheel/setup rebuilding, then join the initialized grid to the verified physical impact pipeline. The matching player bounds candidate remains an explicit selection until original factory selection is proved. No arbitrary stiffness or uniform markers substitute for source integration.

Existing versions match the layout proof. No owner action, installation, desktop input, settings/Steam change or approval rejection occurred. The current playable car remains rigid while runtime integration is incomplete.
