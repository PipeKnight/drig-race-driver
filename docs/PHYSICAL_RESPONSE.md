# Normal constraint response — F153

10 October 2026. Independent Rust now joins optional physical correction normalization and normal-constraint iteration. **Complete original B7A810 → B791B0**, including original BF3B70 memory clearing, repeat and Rust match **701 scenarios /13,510 constraint rows /714,170 output words per debug and release profile**. These are supplied authored constraints and correction handoffs, not synchronized live physical frames.

## Verified scope and layout

The optional correction pass B7B5E0 sets the tangent spans C8/C0 equal to normal-row end BC. The supported path therefore consists of a two-body prefix [0,C4) followed by single-body rows [C4,BC). Tangent/friction rows, initial constraint creation and the rest of the main physical solver are excluded.

Each192-byte row contains:

| Offset | Recovered role |
|---|---|
| 00–3F | Four Jacobian vectors; single-body rows use first two |
| 40–7F | Corresponding body-response vectors |
| 84 | Supplied softness term used in normalization |
| 88/8C | Lower/upper impulse bounds |
| 90 | Bias |
| 94 | Current accumulated impulse |
| 98 | Batch prefix sum of current impulses |
| 9C | Scaled softness |
| A0/A4 | Body indices; second used for two-body rows |
| A8 | Referenced normal impulse for tangent bounds, unused here |
| AC/B0 | Saved normalization/denominator operands; semantic names provisional |
| B4 | Integer contiguous-batch length on first row |

B7A810 is thiscall with one f32 stack operand and `ret4`. For every normal row it computes `factor = relaxation / (denominator + softness_input)` and `coefficient = factor / normalization`. It scales the first16 Jacobian floats for two-body rows, first8 for single-body, clears current impulse to positive zero, scales bias by factor and stores softness_input*factor at9C. Other fields remain unchanged.

B791B0 is thiscall with body count and iteration count, `ret8`. It always clears32bytes per supplied body through original BF3B70, including with zero iterations. Each iteration processes two-body batches before single-body batches. It snapshots body responses at the start of each batch, retaining those cached responses across its rows.

For each row it projects the cached body responses onto the Jacobians. Each four-lane dot uses two adjacent product pairs, then their sum; vector dots are added in first-linear, first-angular, second-linear, second-angular order. Separate f32 operations compute `candidate = ((bias - softness * old_impulse) - projection) + old_impulse`. Clamp against the supplied bounds, store the impulse, accumulate a batch prefix total and apply `(new_impulse - old_impulse) * response_vector` to cached body responses. Publish first body then second at batch end. Authored aliased indices preserve the original second-write overwrite behavior; this is not asserted to be a meaningful physical contact.

`grid_world::physical_response::calculate` supports this complete normal-row path, with optional normalization or iteration-only mode. Bounds: up to256rows/64bodies/32iterations, finite relevant numerical inputs/results, ordered impulse bounds and complete positive-length batches within each span. Normalization requires nonzero finite divisors/coefficient results. Batch members must agree on relevant body indices; this is our supported interface precondition, while the native routine reads those indices only from the first row. Original malformed-pointer behavior is not modeled. Input state is cloned before calculation; errors publish no partial result.

The private PNR1 adapter preserves every opaque row word, checks exact framing/booleans/finite initial workspace and limits input to1–1,024cases/32MiB. It compares full rows and all body-response lanes, including proof that initial authored nonzero workspace is cleared. Output is create-new under ignored `local/`.

## Whole-original comparison

`tools/probe_physical_response.c` maps the pinned private analysis PE, uses aligned owned rows/manager/body workspace and recovered ABI bridges, then executes both complete functions. Executable sections are compared with the post-relocation snapshot. No imports, helper substitution, skipped instructions, code writes or private global changes are used. Original memset dispatch global FB1FA0 remains at its file-backed0; this original scalar clear branch is the tested branch.

`tools/validate_physical_response.py` includes six analytic scenarios and512 authored controls covering empty rows,1–256constraints, up to64bodies,0–32iterations, iteration-only and normalized paths, zero/negative relaxation, subnormal Jacobians, clamping, shared bodies, aliases and batches up to4rows.

It additionally executes F152 correction helpers and Rust afresh against the previously verified6,352-case request, matching317,600words per profile. The5,840 downstream handoff contacts are grouped into183 physical scenarios. Computed correction bias, bounds and response-vector bits are preserved; body roles, denominator/softness terms, batches and relaxation remain authored. This does not establish actual simultaneous frame state or parameter initialization.

Eight independent semantic checks pass: clearing with empty rows, repeated single-contact equilibrium, batch-prefix totals, opposite two-body effects, aliased-body publication order, zero response/no motion, zero-iteration clear and zero-iteration cumulative preservation. Activity:8,094two-body/5,416single-body rows,89zero-iteration scenarios,445normalized scenarios,335multirow-batch scenarios,381nonzero body-response scenarios and10,643rows ending on a supplied bound.

Twenty-two malformed guards reject with ordinary exits below256 and no native fault marker: truncated/trailing input, zero/excess cases, excess rows/bodies/iterations, invalid boolean/span, nonfinite relaxation/coefficients/responses/bias/initial workspace, zero/overrunning batches, unknown body indices, singular normalization, reversed bounds and inconsistent batch bodies. Rust creates no rejected output.

- Analysis SHA256 `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.
- Native probe SHA256 `34c4e9b65c422ae421ea7a1598217080f9d99fb1a85bdadc05024ffc3cbed598`.
- Rust debug SHA256 `18c5bd95715d4e9f6be893bb9e53fbd19d57c65c765575ecfbc9e9a602322e3e`.
- Rust release SHA256 `82e610574479a4e09f88d81afd8a4c8fab64c099d53b83b4bfe9ba55db9f17fe`.
- Upstream correction request SHA256 `b3c6fa0b4885b1f973a2ac49266acce1a2fc4750161f3bf41a0ca67b14164c54`.
- Final reports: `local/behavior/physical-response-20261010-debug-01/validation.json`, `physical-response-20261010-release-01/validation.json`. Reports pin all executed binaries and upstream request.
- Aligned instruction evidence: `physical-iteration-aligned-20261010.json`, `physical-solver-abi-20261010.json`, `physical-memset-20261010.json`. Private pseudocode: `20261010T0329318212252Z/`.

Python3.12.14, Rust/Cargo1.99.0, MSVC14.44.35207/SDK10.0.26100.0 reused; no installation. Strict C compilation,86workspace library tests (20formats+1tools+65world), formatting, warnings-denied all-target Clippy and validator AST parsing pass. No native fault, numerical mismatch or build/lint failure occurred in this comparison. Steam130resources+original executable unchanged at2026-10-10T03:54:23.266514UTC: `local/scene/preservation-20261010-physical-response.json`.

## Next integration dependency

Static B13B00 takes the resulting two vectors as **position and angular corrections** in this optional pass: it adds the first to pointed position at linked+334, builds a rotation from the second, updates the linked+190 matrix and calls B13720. The main solver separately uses its response workspace to update velocities; the workspace alone does not establish units or meaning without its caller. Do not label the optional output as a complete velocity update.

Private `local/behavior/20261010T0353278363010Z/b13b00.c.txt` and `b78d30.c.txt` decompile successfully. B13B00's rotation helper772670 overlaps already verified pose research, but its complete basis/derived-state refresh has not been executed or implemented. Next priority: recover B13720 and verify whole pose correction/publication, then compose the complete optional impact-contact correction pass with original and Rust, before expanding initial row creation/main solver/tangent spans. Geometric contact creation, vehicle initialization/timing and a drivable application remain open. Completion estimates stay unchanged.

Follow-up static preparation: `physical-pose-correction-20261010.json` confirms B13B00 entry→ret8 atB13E01. B13720 (`20261010T0356513319081Z/`) copies corrected190matrix to1D0 and subtracts transformed230offset from translation. It conditionally calls B12B10 when linked32C group's+28 names this body, then calls B0F730 and B100F0. The latter three decompile successfully at `20261010T0357323239983Z/`: shape-list matrix publication (328head/E0next;214local-transform flag), derived matrix refresh and a group-head quaternion/pose branch. These are static observations only. Prefer whole B13B00 execution with explicit authored group/shape roles and complete output coverage; do not silently omit the conditional group-head effects or call this refresh a velocity setter.

Research74368b3 and Rustf4760ec are pushed to owner origin/main. Publication evidence: `local/behavior/github-physical-response-sync-20261010.md`.
