# Vehicle and physical state transfer — F169

10 October 2026. Independent Rust joins complete linked pose/velocity setters, optional whole body advance, optional supplied physical correction and linked getters. **Original/repeat/Rust match 320 calls /4,295,784 body, vehicle and shape words per debug and release build**, including three complete intermediate snapshots per call. Scheduling and physical ownership are authored. This establishes a transfer boundary; it does not establish a live world frame or drivable lap.

## Contract and order

The wrapper's +8 points to the owned physical body. A696F0 copies the supplied pose and calls whole B12710. A69840 and A69860 copy four-lane linear and angular velocity to physical +220/+210. A69800/A69820 read those same vectors; A69630 reads **published +1D0**, not center +190. Corresponding car fields are +15C0/+15D0/+15E0. Setters preserve all velocity lanes. The synchronous A09A80 caller masks velocity W beforehand; that broader caller is not executed here.

B12710 stores the supplied publication, clears the first three rows' W to positive zero and forces position W to one. This happens before center arithmetic, so arbitrary/nonfinite incoming W values are discarded legitimately. It retains this exact publication and builds center position as:

```
center[i] = ((offsetW * publishedPosition[i] + offsetY * rowY[i])
           + (offsetZ * rowZ[i] + offsetX * rowX[i])) + publishedPosition[i]
```

This is a distinct direction from physical publication, which subtracts the transformed offset from center. Reconstructing the supplied publication through a round trip can introduce rounding and is incorrect at this setter boundary. Shared Rust `physical_pose` now separates derived/shape output generation from the two translation directions, retaining earlier arithmetic unchanged.

Whole original B0F730 refreshes all linked shapes and B100F0 derives both rotated matrices. Setter then saves publication at physical +40. ABC7B0 copies each nonnull shape +170 matrix to shape +70; this is a history copy, not an external callback. The diagnostic supports absent pointers, a shape's own +130 publication, or its own +F0 local matrix, represented by opaque selectors 0/1/2. Original owned addresses are restored to selectors only at wire output.

The setter's notification flag can also call whole B24E70 when physical +400 is -1. That dependency clears group velocities, changes membership/state through B0F2F0 and can traverse further shape/contact ownership. **This branch is explicitly rejected**; no reset, wake-up or full group ownership claim. Both notification flag values with +400 != -1 are tested.

The optional advance executes complete B13E10 with its normalization selector different from the body, as in the F162 controlled branch. It saves center +270 and publication +2B0, clears +360, advances center through velocity and polynomial rotation, then refreshes publication, shapes and derived matrices. It additionally writes each shape +40 as **old center position minus new center position**. It preserves setter history at body +40 and shape +70.

Optional whole B13B00 then applies supplied linear/angular corrections and calls B13720, including conditional group-head basis orthonormalization. No solver produces these correction vectors in F169. Readback follows every stage and preserves all unrelated car bytes. Native/Rust use the same explicit setter → advance → correction diagnostic order; this is not evidence that these stages run consecutively in the original world schedule. In particular, F168 already commits a suspension-predicted pose: integration must recover actual manager ordering before adding another advance.

## Evidence and validation

The 128 initial cases comprise 96 authored controls and **32 unchanged vehicle outputs from freshly rerun F168**. Physical center offsets, local matrices, shapes, delta and correction are authored. Fresh original and rebuilt Rust rerun all 120 source-backed suspension cases and match the pinned prior whole output before those 32 handoffs are selected. Numeric car fields are not replaced for the handoff. Controls include discarded NaN/infinite W, signed zero/tiny angular velocity, perturbed orientation, nonzero offsetW, both head roles, stage bypasses and 0–64 shapes.

Initial coverage: 107 advances, 79 corrections, 47 normalizations and 1,675 shape nodes; history selectors occur 559/558/558 times. Sixteen independent sequences continue for twelve calls each, every original/repeat/Rust branch carrying its own complete final physical/car/shape state. Total 128+192 = **320 calls**, with exact whole buffers at all three stages.

Nine semantic checks verify homogeneous lanes, publication rather than center readback, velocity copies, unrelated car preservation, body and shape histories, shape displacement, history retention through motion and bypass behavior. The analytic Rust integration test uses offset [.25,.5,.75,0], velocity [2,4,6,0], delta .25 and a supplied correction, checking distinct center/car poses and retained histories.

**25 native +25 Rust malformed guards** reject ordinarily and leave no output: framing/count/flags, consumed pointers, nonfinite local matrices/offset/velocity/pose/shape, invalid shape selectors, unsupported group reset, delta bounds, correction/spin bounds, truncation and trailing bytes. CLI buffers are bounded to 32 MiB/128 calls/64 shapes, and Rust output is create-new under ignored local/. The public library is immutable on input. Pose delta <=.25 and rotation <=64 are authored diagnostic limits.

Native maps only the pinned analysis PE. Complete original routines run without entry point, imports/TLS, stubs, skipped instructions, executable writes or injection. Four lazy-data pages E58000/E5A000/E5B000/E5E000 have bytes/protections restored; executable sections are checked unchanged. Owned linked pointers are relocated into isolated aligned allocations.

Fresh shared F154 regression matches **896 cases/3,179 shapes/671,576 words** in each build after the publication refactor. Its fresh upstream normal-response comparison also passes. **122 library tests** (20 formats, 1 tools, 101 world), formatting, all-target warnings-denied Clippy, Python AST and whitespace checks pass. No numerical mismatch/native fault. One guessed probe filename read failed harmlessly. Ghidra initially could not write its existing cache under sandbox; normal escalation succeeds. No new installation or desktop input.

## Reproduction and provenance

- Sources: `crates/grid-world/src/vehicle_bridge.rs`, shared `physical_pose.rs`, `crates/grid-tools/src/bin/grid-vehicle-bridge.rs`, `tools/probe_vehicle_bridge.c`, `tools/build_vehicle_bridge_probe.cmd`, `tools/validate_vehicle_bridge.py`.
- Reports: `local/behavior/vehicle-bridge-20261010-debug-final-02/validation.json`, `vehicle-bridge-20261010-release-final-03/validation.json`. Initial debug01 precedes the added native spin guard; final02/03 include it.
- Shared regressions: `physical-pose-20261010-bridge-regression-debug-04/validation.json`, `physical-pose-20261010-bridge-regression-release-05/validation.json`.
- Static evidence: `local/behavior/state-bridge-aligned-20261010.json` and `state-bridge-reset-aligned-20261010.json`; successful Ghidra `20261010T1908489509535Z/` and reset survey path recorded in `state-bridge-reset-survey-20261010.log`. Complete B12710 ends B12908; ABC7B0 ends ABC7C9; B24E70 ends B24EE5; B0F2F0 ends B0F479; B13E10 ends B14264. Adjacent function bytes are excluded.
- Analysis SHA256 `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.
- Native SHA256 `8de9c4c36249a546fa2c24f156fea8ee958298804a630129625ed44ca988bb63`.
- Rust debug SHA256 `2ffa89156f8ba699c3437ff4cce3efabfb2dee6395a12f5c733add9adcedbe44`; release `9b2810d13fdd0d45eb106214e5ac557f4de6a02315a18e014463d754bdc205cb`.
- Initial request SHA256 `1eb665356079c8451ab9b97e66ae006a96490a57f5df1d967eb6ec447d8bc291`; three identical original/repeat/Rust outputs `dcae308cd46adca9e5391b640ce0a89630bf830a9c3ac4322485af75ce6694b5`.
- Fresh F168 request `29b64fd2ac1cd2efbb7846f77249588d7b8a0884309a103ac48c35cde2ee9362`, output `4dcfdb7a4fe2a7aabbe3825a6b082d4c93dc9f6b11b87039dc9a1539e21bb645`, selected cases0–31. Numeric evidence stays private.

Reused Python3.12.14, Rust/Cargo1.99.0, MSVC14.44.35207/SDK10.0.26100.0, Ghidra12.1.4/JDK21.0.9 and Capstone5.0.6; no installation. Native `/W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2`. Steam130 resources plus executable unchanged **19:18:12.135646 UTC**, `local/scene/preservation-20261010-vehicle-bridge.json`.

## Next dependency

Recover/execute the selected actual outer vehicle driver: A03670 calls A00E10 before later grip/steering/wheel processing and F166 force production. Do not directly concatenate F168 and F166 while omitting those intervening stages. Whole9BD7F0 input/mass synchronization, whole A03220 post-update and active group reset remain unverified. Then connect source initialization, physical contacts and Bevy input/camera under a documented schedule. Full driving and completion estimates remain open; owner help is not needed for this offline step.
