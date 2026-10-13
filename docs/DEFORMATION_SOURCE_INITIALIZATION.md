# Source-derived Lacetti deformation initialization

13 October 2026, ongoing follow-up to [layout validation](DEFORMATION_LAYOUT.md). Independently read source inputs, passed through the **whole original initializer**, reproduce all **32,000 bytes of the pristine player node pool**, all **847 edge records / 6,776 bytes** and all **44 captured settings bytes**. This is an original-code baseline for the Rust port; no saved native state is used by runtime startup. Rust mode-zero initialization, original resource-factory selection and visible mesh deformation are still unfinished.

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

## Remaining independent implementation

Mode zero visits cells and intersects their six planes with each oriented strength BOX's six planes. `A15F50` constructs the intersection vertices and ordered triangular faces; `9CC790` accumulates their positive volume. `9E7CE0` weights volumes by damage and axis crush values; its x87 damage/total-volume accumulation and SSE axis accumulation must retain distinct rounding. `9EBEC0` distributes these cell results to node limits and markers. The complete source baseline is available to check this work without live desktop input.

The fresh private surveys `20261013T1244006562464Z/{a15f50,9cc790}.c.txt` expose the reached geometry boundary. Their inferred pseudocode is evidence, not independently authored engine source. Ordered plane intersections, duplicate-vertex handling, face ordering and thresholds need exact aligned/native comparisons before the Rust initializer is enabled. After that, edge construction and dirty wheel/setup rebuilding can join the already verified impact pipeline. No arbitrary stiffness or uniformly positive markers will substitute for this stage.

Existing tool versions match the layout proof. No owner action, installation, desktop input, settings/Steam change or approval rejection occurred. The current playable car remains rigid while this initialization port is incomplete.
