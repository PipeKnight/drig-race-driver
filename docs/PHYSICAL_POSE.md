# Physical correction and publication — F154

10 October 2026. Independent Rust applies supplied linear/angular corrections and publishes body, shape and derived matrix state. **Whole original B13B00 → B13720**, including conditional B12B10, original rotation772670/cross67C200, shape refresh B0F730 and derived refresh B100F0, repeat and Rust match **896 cases /3,179 shapes /671,576 output words per debug and release build**. Poses, shape lists and group membership are authored; solver-output handoffs are verified, but no live whole physical frame is claimed.

## Recovered behavior

B13B00 is thiscall with two aligned four-float correction pointers, `ret8`. It adds the first vector to all four pointed position lanes. It obtains the second vector's four-lane magnitude using RSQRTPS and one refinement. Above D16788 (`0x233877AA`), original772670 builds a Rodrigues rotation from normalized axis and magnitude. The first three basis rows multiply this rotation; position is saved/restored. Zero/subnormal angular magnitudes skip rotation. The independent diagnostic bounds magnitude to64radians, consistent with existing rotation research; this is our interface bound, not a recovered original cap.

B13720 conditionally calls B12B10 when group+28 names this body. **Correction to F153's preliminary notes:** this is basis orthonormalization, not quaternion processing. First capture10 linked pointers identify +338/+33C/+340 as basis rows +190/+1A0/+1B0 and +334 as position +1C0. Group membership itself was not captured there. Native tests author both branches.

B12B10 normalizes the first row with refined RSQRTPS inverse magnitude, removes its projection from the second and normalizes the second. Original67C200 creates their XYZ cross product with positive-zero W; normalize this third row. A negative dot with the previous third row flips its sign to retain handedness. Normalization dots use all four lanes. Degenerate/nonfinite normalization is outside the supported API domain.

Refresh copies corrected190–1CF to published1D0–20F and subtracts transformed local offset230 from published position, including offset W. Shape list328/E0next: byte214zero copies publication to shape130; nonzero multiplies local F0matrix by publication. Other shape bytes remain unchanged. B100F0 transforms local matrices90/150 to derivedD0/110 as `Rᵀ A R`, with corrected orientation row3 forced to `[0,0,0,1]`, excluding translation. Rust retains separate f32 operations and paired product additions.

`grid_world::physical_pose::calculate` returns corrected/published matrices, two derived matrices and shape matrices, without changing inputs on failure. PPC1 preserves the full0x424-byte body and each0x220-byte shape with pointer-free wire fields and owned aligned native objects. Bounds:1–1,024cases/32MiB,64shapes/body, boolean flags, finite relevant inputs/results, nondegenerate head basis and ≤64radian angular correction. Opaque words may contain arbitrary bits; even inactive local shape matrices must be finite under the adapter contract. Rust creates no rejected output.

## Comparison and handoff

The native probe pins the private analysis PE, executes complete original functions and compares executable sections with their post-relocation snapshot. Four lazy-data pages E58000/E5A000/E5B000/E5E000 temporarily allow original initialization; contents/protections are restored afterward. No executable write, skipped instruction, helper stub, import resolution, entry-point execution or game-process modification.

Eight analytic plus504 authored controls cover translations/offsetW, identity/perturbed/four-lane/flipped bases, both group roles, zero/signed-zero/subnormal angular inputs, rotations, arbitrary local matrices and up to64linked shapes. Each profile reruns whole F153 original normal solver and Rust afresh:701scenarios/714,170words still match. Its output supplies384 unmodified nonzero correction vectors:381 authored-control and3 F152 correction-contact-path bodies. Other pose/group/shape fields remain authored. **578 upstream nonzero body outputs exceed the angular diagnostic bound and are excluded**, without scaling or success claims. A bounded384 eligible vectors are selected; original arbitrary solver outputs need not all be physically meaningful.

Ten semantic checks pass: zero-correction identity, translated position, center offset, negative handedness, translation excluded from derived orientation,64shape traversal, unrelated body/shape bytes preserved, direct shape publication and head orthonormality. Activity:448head/448member cases,1,362transformed shapes,881changed bases and894changed positions.

Seventeen malformed guards reject ordinarily (exits below256/no native fault): truncated/trailing input, zero/excess cases, invalid head/count/pointers/shape flag, nonfinite basis/local matrix/offset/shape/linear correction, oversized angular correction, zero/parallel basis. Parallel basis reaches nonfinite rejection in the original adapter; malformed-basis behavior is not useful modeled physics.

## Reproduction, failures and provenance

- Sources: `tools/probe_physical_pose.c`, `tools/build_physical_pose_probe.cmd`, `tools/validate_physical_pose.py`, `crates/grid-world/src/physical_pose.rs`, `crates/grid-tools/src/bin/grid-physical-pose.rs`.
- Analysis SHA256 `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.
- Native SHA256 `879ec96e3bf23787e8c141d0e1518fab28973cfb56107eef2f52fa1a77ec6a6a`.
- Final request SHA256 `7073f6f1b6873f2f547f7ce033ff1932ad028978d72aaabfd2dc8c5e9cb09521`.
- Rust debug SHA256 `f5412c770acab8b6ee26a5df201f62ba0d94ca96c5ce0d626e58788b54c1d93b`; release `f7718be24f1814a0d7c11373594df32ad79b7d344a3ad77498927a002db26545`.
- Reports: `local/behavior/physical-pose-20261010-debug-final-03/validation.json`, `physical-pose-20261010-release-final-03/validation.json`. All upstream identities recorded; normal-response request SHA256 `78146034994f68ba104c102924bdcddfa3941350e2cb0527a908aac61134fbaa`.
- Static evidence: `physical-pose-correction-20261010.json`, `pose-refresh-aligned-20261010.json`, `derived-matrix-aligned-20261010.json`; private035327/035651/035732surveys. Captured pointers: `live-impact-20261009-10/capture.json`.

Python3.12.14, Rust/Cargo1.99.0, MSVC14.44.35207/SDK10.0.26100.0 reused; no installation.89library tests (20formats+1tools+68world), fmt, warnings-denied all-target Clippy and PythonAST pass. Initial lint rejected `-1*v` spelling; finite-value sign negation fixes it, final binaries rebuilt/recompared. One ad-hoc Python inspection command failed parsing a literal newline escape and executed no analysis. Initial01/intermediate02 evidence retained; final03 balances handoffs and explicitly pins upstream input. No native fault/numerical mismatch. One documentation patch failed its context check atomically and was reapplied. Steam130resources+original executable unchanged2026-10-10T04:26:53.289372UTC, `local/scene/preservation-20261010-physical-pose.json`.

## Next dependency

Compose optional B7B5E0: ordered contact bias → active guard → normalization → body correction solve → pose publication. It also calls AEF930/AEFBB0 for other pools; recover those before claiming their branches. Preserve no-active-contact bypass. Main constraint creation, tangent spans, live enabling parameters/cadence, initialization and drivable Bevy loop remain open. No new owner recording needed; completion estimates unchanged.
