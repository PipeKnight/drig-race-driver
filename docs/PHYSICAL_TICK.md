# Body advance joined to physical contact solving — F162

10 October 2026. Whole original **B13E10** for each body, followed by supplied contacts and whole **B7B850** main/optional frame, repeat and one Rust `physical_tick::calculate` call match **711 scenarios /3,189 body advances /3,925,133 output words per debug and release**. This joins actual pre-contact pose advance, publication and derived matrices to [F160's main/correction physical frame](IMPACT_FRAME_CORRECTION.md).

This comparison uses an authored boundary adapter: contacts are **supplied as post-advance geometry**, not produced by the engine or proven consistent with the advanced bodies. Whole AD7040/AD6C40/AD7110/world scheduling are not executed here. Registered callbacks, other/joint pools, linked-body propagation, wake-manager mutation, group-head orthonormalization and shapes remain absent in this joined path. It does not establish a drivable application or a live whole tick.

## Recovered order and implementation

[Static ownership research](PHYSICAL_SCHEDULING.md) now establishes the larger original synchronous parent AD8D70. When world+7C4 is idle, it stores incoming delta at world+7C8, resets counters, initializes/refreshes world/manager state and runs AD7040 before collision/group stages and AD7110. AD7040 distributes bodies among update lists using body+318, then synchronous AD6C00 or task AD66A0 calls B13E10 for each body. AD8A40 is a separate staged-state path; no fixed cadence or live mode follows from these static branches.

B13E10 is thiscall with delta and two identity arguments, `ret0Ch` at B14264. Before advancing it clears body+360..36F, snapshots the old center matrix+190 into+270 and old published matrix+1D0 into+2B0. It advances the pointed +334 position using **incoming linear velocity**, computes angular magnitude before multiplying by delta, rotates the basis and publishes center-offset-corrected pose and derived matrices. The identity-equality branch orthonormalizes only when rotation runs; this probe supplies unequal identities so that branch remains naturally inactive. The original shape list is empty here; its shape-displacement writes remain outside this joined adapter.

Rust reuses the independently verified polynomial Rodrigues predictor and factors the existing pose publication arithmetic into a shared crate-local helper, avoiding a second correction/translation addition. `physical_tick::calculate` advances each body's basis/position/derived torque matrix, then calls complete `impact_frame::calculate` on the resulting state. Its output retains advance-stage publications separately from final correction/shift publications. The diagnostic preserves the previous matrix copies and cleared response bytes in raw body outputs. The caller owns those history fields in the typed API.

The advance precedes even the main frame's zero-iteration/all-asleep/capacity bypass. Gravity/drag and newly solved velocities apply afterward; using those new velocities to advance again would change the original order. Two new integration tests verify movement before zero-iteration return and position from incoming velocity before new gravity.

## Comparison and bounds

The 695 established F160 cases use the same authored body/contact/row operands and optional settings. Sixteen additional authored controls exercise zero/negative-zero angular vectors, subnormal squared angular magnitude, small normal magnitudes, W-lane angular magnitude and signed-zero translation, with main iterations zero to isolate advance publications. These are controls, not live captures. Delta remains bounded to (0,.25] and rotation to the existing predictor's diagnostic bound; no original frame clamp is inferred.

| Coverage | Count |
|---|---:|
| Advanced / moved bodies |3,189 /3,189|
| Bodies with changed orientation after the joined call |3,157|
| Zero-angular / tiny-angular body controls |16 /16|
| Finished / all-asleep / capacity-failure cases |472 /81 /41|
| Main zero-iteration / empty-contact cases |117 /57|

Six semantic checks pass: exact previous center and publication copies, cleared response vector, zero-iteration contact/row retention, independently rounded zero-iteration position advance and post-success force clearing. Twelve malformed guards reject ordinarily in original and Rust, with no native fault/rejected Rust output. They cover framing/count errors, optional settings, callbacks/links/stale metadata and NaN basis. Original executable sections stay RX and unchanged, five lazy-data pages restore protections, and no imports/stubs/Steam modifications are used.

Each profile freshly replays F160 whole original and rebuilt Rust for 695 cases/3,895,485 words, proving the shared frame-validation refactor preserves that contract. Fresh F154 original/rebuilt Rust regression also matches **896 cases /3,179 shapes /671,576 words per profile**, including group heads and transformed shapes, after extracting shared publication. Reports: `local/behavior/physical-tick-pose-regression-20261010-{debug,release}/validation.json`; its existing pinned request SHA256 remains `7073f6f1b6873f2f547f7ce033ff1932ad028978d72aaabfd2dc8c5e9cb09521`.

**113 library tests** (20 formats+1 tools+92 world), formatting, all-target warnings-denied Clippy and validator Python AST pass. No native fault, Rust numerical mismatch or failed build occurred in F162. The first debug-01 comparison passed the initial695cases; final02 adds the explicit angular/zero controls. The initial broad B13E10 disassembly range also includes adjacent functions; the target ends at B14264. A broad setter B12710 range similarly reaches its neighbor; B12710's own return is B12908 `ret8`, not the adjacent function's `ret4`.

| SHA256 | Value |
|---|---|
| Analysis PE |aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514|
| Native joined probe |16d1c64f4ef1246c1b6a026ba17d42b8a9ca14d745dbf524ae6c002ccc9b67b7|
| Shared final request |f3312e5bea0c899d27b25ec549716cf1e62a6fa4c9e43773f0b7db4c80a0dc4b|
| Debug diagnostic |b8d6415a77b8cbd80234ff8d4a698d67605d78e78d6ca012502c7a09ecbdc46d|
| Release diagnostic |972059438b6cce137e18aba22f0f69375badf0a23828f1e560639975be26fd52|

Final reports: `local/behavior/physical-tick-20261010-{debug,release}-final-02/validation.json`. Native uses existing MSVC14.44.35207/SDK10.0.26100.0 strict x86 flags. Rust/Cargo1.99.0, Python3.12.14, Ghidra12.1.4/JDK21.0.9 and Capstone5.0.6 reused. No installs, desktop control or owner assistance. Steam130 resources/executable unchanged at11:33:29.380928UTC: `local/scene/preservation-20261010-physical-tick.json`.

Static evidence under ignored `local/behavior/`: world parents `20261010T1121170245051Z/`; advance/publication stages `20261010T1122465918038Z/`; body jobs `20261010T1124074465400Z/`; whole advance `20261010T1125048423661Z/b13e10.c.txt` plus `body-advance-aligned-20261010.json`. B12710 setter survey `20261010T1122163464473Z/`, aligned `physical-pose-setter-aligned-20261010.json` and `published-pose-affine-mask-20261010.json` remain static findings, not a newly verified setter implementation.

Next missing first-lap dependencies remain authoritative geometric contacts/group construction, active vehicle/collision callbacks, complete vehicle initialization and forces, and Bevy scheduling/input. This larger joined physics path reduces integration uncertainty but does not change the project's completion estimate or claim a completed lap. The expired overnight automation remains paused.
