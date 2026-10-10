# Complete impact-only main physics step — F158

Whole original B7ACC0, original repeat and one independent Rust `impact_step::calculate` call match **695 scenarios / 25,809 consumed rows / 3,841,665 output words per debug and release build**. This connects initial normal/grouped-friction construction, material caching, main preparation, all four solve spans, contact impulse publication, body velocity updates and enabled external forces. Other/joint contact pools are empty. The outer physical frame, contact generation, scheduling and drivable vehicle integration remain unverified.

512 authored controls plus 183 handoff cases cover 5,840 unchanged accumulated-contact operands and disabled flags from the prior serial callback/correction chain. Roles, grouping inputs, geometry, materials, step parameters, inverse properties and additional body state are authored. Fresh original/Rust F152 replay reproduces 6,352 cases /317,600 words before each comparison. This is a controlled dependency handoff, not a synchronized live frame or captured real-world vehicle parameters.

## Recovered contract

B7ACC0 is thiscall with **five scalar stack arguments**, returning AL and popping 20 bytes at B7B5CF: delta, time factor, bias factor, compliance scale, impulse writeback scale. The material cache is manager-owned at +20; the last two stack arguments are not cache pointers. Complete aligned instructions supersede misleading pseudocode types.

The effective delta is the rounded product of the first two scalars. Inverse delta is its reciprocal; constraint time is `inverse_delta * time_factor`, preserving both operations. Construction uses this time, manager fallback/threshold, compliance scale and angular-scaling globals. Main preparation uses this time for softness and `inverse_delta * bias_factor` for provisional bias. The manager supplies relaxation and iteration count.

The manager visits two-body impact contacts then single-body impact contacts, retaining order within each pool. Metadata helpers set one normal row per contact; a change in body pair (or first body for singles), including the last contact, marks the group end and reserves three friction rows. Fresh input group-end bytes must be zero: the original helpers do not clear a previously set byte on the non-end branch. The bounded adapter rejects stale metadata rather than risk building more rows than allocated.

The resulting contiguous spans are two-body normals, single-body normals, two-body friction and single-body friction. Before construction the manager initializes only provisional bias, default softness, maximum limits, invalid body indices, reference and batch header fields; opaque row words remain unchanged. It calls the complete original builders and flushes the final batch headers. Tangents point to the last normal prefix in each contact group. The diagnostic represents that pointer as a flat normal-row index; other words compare unchanged.

Before main preparation, linear/angular workspace starts with inverse-delta-scaled body velocities, plus enabled force or transformed torque. The four-lane torque transform preserves the original grouping: `(W*row3 + Y*row1) + (Z*row2 + X*row0)` per column. Force integration uses body +3F4 inverse mass, distinct from the supplied cached inverse mass used for constraint responses. The solver clears this workspace before iteration, so its output is the contact response contribution.

Writeback stores `(normal_impulse * writeback_scale) / constraint_time` in contact +3C. That scalar times the contact normal adds to first-body +240 impulse accumulator; a two-body contact adds its negation to the second body, including alias cases. Original B0FC00 sets body +40C even for zero impulse. Body linear/angular velocities then receive effective-delta-scaled solver workspace. Enabled external force/torque integration follows separately, including all W lanes.

Two bypasses are part of the contract:

- Capacity failure returns false **after** updating contact metadata and span ends. Rows, cache, workspace and bodies remain unchanged; external forces are skipped.
- Empty contact pools return true, retain existing span ends/rows/workspace/cache, and still integrate enabled external forces and torque.

## Comparison and safeguards

The native harness calls the unchanged whole manager with owned, aligned bodies, contacts, shapes, materials, row storage, cache properties and workspace. It maps the identity-pinned private executable through the existing isolated mapper. Original material, compliance, cross-product, matrix response, solver, impulse and memset helpers execute; no helper stubs, skipped instructions, import dispatch or original code patches are used. Three original data pages become temporarily writable for scaling globals/lazy masks and are restored; executable sections remain RX and compare unchanged. Supplied inverse properties and manager fields outside cache/span ends also compare unchanged.

The comparison includes every allocated 192-byte row, full 0x424-byte body wire, full 128-byte contact, final workspace, cache, return flag and span ends. Pointer identities are owned and canonicalized before comparison. Bodies/contacts carry authored opaque bytes to expose unintended writes. 7 semantic checks and 31 ordinary-exit malformed guards pass in both profiles. Limits: 16 bodies, 8 materials, 64 contacts, 256 allocated rows, 32 iterations, 1,024 scenarios and 32MiB request. The native harness is an owned-input research oracle; these checks do not establish safe execution for arbitrary malformed proprietary objects.

| Observed coverage | Count |
|---|---:|
| Successful scenarios / capacity failures |638 /57|
| Empty / zero-iteration scenarios |57 /101|
| Consumed normal / friction rows |11,970 /13,839|
| Group-end metadata publications |5,091|
| Bodies with changed velocities / impulse accumulator |2,515 /1,132|
| Contacts with changed impulse |8,848|
| Enabled external force / torque inputs |1,563 /1,562|
| Aliased two-body / disabled contacts |2,010 /3,646|

105 library tests (20 formats, 1 tools, 84 world), formatting, all-target warnings-denied Clippy and Python AST checks pass. Four new boundary tests cover empty force integration, disabled-force retention, capacity metadata publication and invalid timing. Initial test literal syntax/formatting errors were corrected before final rebuild; draft assign/range/sign lints were corrected earlier. An initial guessed outer-survey file path did not exist; the documented `031219` survey is the actual source. No native fault or numerical mismatch occurred during F158. The final evidence uses rebuilt binaries after all source/formatting changes.

## Versions, identities and private evidence

Rust/Cargo 1.99.0, MSVC14.44.35207, WindowsSDK10.0.26100.0, Python3.12.14, Ghidra12.1.4/JDK21.0.9 and Capstone5.0.6 reused; nothing installed. Native strict flags: `/W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2`, relocated image/owned operands, no main/import/TLS initialization. Public files contain independently authored code and behavioral descriptions; executable, disassembly, requests and outputs remain ignored under `local/`.

| SHA256 identity | Value |
|---|---|
| Private analysis executable |aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514|
| Final native probe |e4ec364c12dc223a9fdd89c0927b0c11ad78c458862be59ac8cf511b81ea3974|
| Final shared request |d468ba745e5843d4edfe6a5d002a97767390e82ed933b515fef6ae51de08f841|
| Final debug diagnostic |12ac4b2033b4d367c0bdc922725e0ac29960d37321329ae5b6a0b74dea69952e|
| Final release diagnostic |cb7911966087728f1aed1f11e9cfd4958e4eb6ace9c01b98a35b6e94e3867566|

- Final reports: `local/behavior/impact-step-20261010-{debug,release}-final-03/validation.json`.
- Intermediate comparisons: `impact-step-20261010-debug-01` (512 controls), `{debug,release}-02` (695 scenarios).
- Aligned whole ABI/helpers: `local/behavior/main-impact-manager-aligned-20261010.json`.
- Original inferred manager: `local/behavior/20261010T0313526540738Z/b7acc0.c.txt`.
- Metadata/writeback helpers: `local/behavior/20261010T0444154492921Z/`; accumulator helper `20261010T0501398983214Z/b0fc00.c.txt`.
- Steam preservation: `local/scene/preservation-20261010-impact-step.json`, 05:33:56UTC:130 resources and original executable unchanged.

Next: recover and verify the complete outer body preparation and final pose integration/publication, then connect those with this main manager, the serial callback and optional correction pass. Joint/other pools, authoritative contact generation, real timestep ownership and vehicle wiring remain separate open requirements. Desktop activation is still unavailable; no repeated input, owner help or extra recording was needed. First drivable lap and project-completion estimates remain unchanged; temporary unattended work ends07:30UTC.
