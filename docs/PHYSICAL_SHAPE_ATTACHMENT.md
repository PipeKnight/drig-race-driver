# Physical shape attachment boundary (F216 research)

12 October 2026. This is static research; whole physical attachment has not executed in the owned harness or Rust. [F215](MESH_SHAPE_CONSTRUCTOR.md) proves shape construction separately. No geometric driving/barrier claim follows from this document.

`run_ghidra_behavior_survey.ps1 -Containing` resolves1..64 interior addresses to existing analyzed functions without inventing function starts. Actual Ghidra12.1.4 operation resolves body328 writeB142E9 toB142D0 andB144FE toB144C0. Normal entry mode still works against four actual functions. PowerShell AST and four malformed/mixed/oversized argument checks pass. The first attempt's PowerShell array flattening miscounted one supplied mode and rejected before launching Ghidra; corrected using explicit boolean mode counts. Private validation: `local/behavior/containing-survey-validation-20261012.json`.

## Observed static contract

WholeB142D0(body,shape,skipWake) prepends the shape to body328, stores body ownership in shapeD8 and old list head in shapeE0, composes/publishes its transform, invokes actual broad-phase refreshB16D70 and body derived-data refreshB11520. With skipWake false and inactive body400=-1 it also wakes the body. WholeB144C0 removes the linked-list entry and refreshes body data; it must not be assumed to undo every broad-phase/owner field by itself.

Aligned instructions distinguish ownership: body32C points to a registry object, whose18 points to the full broad-phase world. B142D0 callsB16D70 with that full world. Earlier cold physical constructor proofs had no shapes and did not traverse this link. Mesh-shapeCC references the shape world/material/counter object. An enclosing harness needs both owned objects, region-head arrays and endpoint lists.

Static bounds tableE51FA4 contains original pointers; kind7 dispatchesB42050. B159C0 derives a region index from world938/93C/940/944/948/94C. B151C0 searches per-region static/dynamic lists at950/954; ABC1B0 maintains region membership via shape198/E8/EC. B16D70 maintains sorted XYZ endpoint lists atworld900/904/908 and shape19C..210. B11520 branches onbody411; the supplied-inertia constructor sets this flag, bypassing geometry-derived mass accumulation while refreshing derived state. Complete mutations/rounding and safe execution remain to be verified.

Private evidence under `local/behavior/`: static candidates in `shape-body-link-store-candidates-20261012.json`; resolved functions in `20261012T0346045337979Z/{b142e9,b144fe}.c.txt`; aligned evidence `physical-shape-attach-aligned-20261012.json`; `shape-dispatch-static-20261012.json`; helper surveys `20261012T0346583406060Z`, `0342024417834Z`, `0350073789670Z`. A6A250 callback-store evidence is separate from physical attachment.

Next construct an owned one-cell full world and separate registry, invoke complete physical constructor thenB142D0 on the source-prepared mesh shape, and compare complete body/settings/shape/world/region output against independent Rust. Retain actual bounds dispatch, all code unchanged, strict guard/fault distinction and global restoration. Then validate overlapping geometry and real narrow-phase contacts through retained world updates. No desktop/input/installation or owner help is required.

A draft documentation script used a non-ASCII arrow in a bytes literal and failed parsing before writing anything; corrected with ASCII text.
