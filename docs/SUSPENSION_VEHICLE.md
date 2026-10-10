# Complete selected suspension wrapper — F168

Recorded 10 October 2026. The complete supported original `9F1E10` now matches one Rust call, including inertia setup, [whole contact preparation](SUSPENSION_CONTACT_PREPARATION.md), the actual substep schedule, suspension/travel/contact response, material-aware finalization, committed pose and body/history publication. This replaces F163's supplied-geometry composition with the actual outer boundary and real Donington queries. Later tyre forces, the car job and the physical manager remain separate.

`grid-world::suspension_vehicle::apply` takes owned bounded state and a world-query callback, returns the saved force-view inertia, and commits state only on success. It supports four active wheels and the source-selected extra-tyre-axes-off branch. `grid-suspension-contact` runs this boundary for SF01 requests; SP01 still selects whole preparation only.

## Joined behavior

The wrapper saves incoming inertia to context6F0 and force-view48..50, computes speed-dependent alternate inertia into context6FC, then restores incoming body inertia. The recovered `98FCA0` arithmetic first normalizes/clamps speed, then compares that normalized value with the same stored intervals again for a second scaling stage. Rust retains that observed behavior. Alternate inertia is recorded even though the selected normal-axis branch uses the original inertia.

Whole preparation produces geometry, source contacts, external rates, coupling and predicted floor history. Substeps use `max(1, trunc(delta * 999.9999389648438))`, f32 reciprocal/sub-delta and descending weights. Existing verified wheel equations now consume those computed inputs directly.

Before clearance clipping, whole finalization adjusts a selected material's working floor by `(1 − material.real_proportion) * new_bump`. The pointer at wheelC0 is the selected mechanics record; older supplied-contact finalizer descriptions calling it a generic contact object were incomplete. Source mechanics include real proportions0, .75, approximately .85/.9 and1. Weighted impulses determine committed pose; raw impulses determine final velocity. Material identity is cleared when the final wheel-contact flag is false. All impulse accumulators and four inactive-axis accumulators are consumed; a zero final force also clears the retained contact term.

Post-solver external-rate additions, the separate half-delta position correction, bump-history copies, pose/velocity cache publication, frame flag and incoming inertia restoration are part of the same verified call. This does not apply pose prediction a second time using final velocity.

## Comparison and scope

Whole unchanged original, repeat and Rust match **120 varied frames plus 16 sequences of 12 carried frames: 312 calls / 759,720 words per debug/release profile**. Each output includes the entire2600-hex-byte body plus three force-view inertia words. All312 calls change pose and velocity; all1,248 wheel-force fields change from their inputs. There are287 final contact flags and225 retained mechanics identities. Object/triangle-slot addresses are normalized to presence; mechanics identity retains its source index.

Controls cover rotated/tilted current and previous poses, signed-zero angular velocities, source surfaces, delta1e-6..0.1, preload/damping/spring/bump/travel stops, varying scale and speed-dependent inertia intervals, opaque padding and retained floors. The carried branches consume their own complete previous outputs. Caller history transfer and a pre-wrapper gravity contribution are authored controls, not recovered world/car scheduling or live trajectories.

Independent checks verify pose/velocity cache publication, the retained frame flag, restored inertia and force-view copy, cleared clock/accumulators and bump-history publication. **26 malformed cases reject in both native and Rust adapters**, without creating output; native guards run before original entry. A Rust integration test verifies complete rollback, including inertia/wrapper setup, after a query error, and rejects disabled wheels before querying.

Final private reports:

- `local/behavior/suspension-vehicle-20261010-debug-final-04/validation.json`.
- `local/behavior/suspension-vehicle-20261010-release-final-05/validation.json`.
- Fresh F167 regression: `suspension-contact-20261010-wrapper-regression-debug-07/` and `release-08/`:248calls/603,136bodywords and all36 rejection checks exact/pass in each profile.

| Artifact | SHA-256 |
|---|---|
| Whole original probe | `fe1e71a2c825d062cbb789aa520adad81861d9bd4d4ec92b6256d6c7a4a75396` |
| Debug diagnostic | `d66dda5774fd097b36725babf3175bad946701094ed374f4b1e28abf1c029213` |
| Release diagnostic | `571dad21aafce0a89f5b7d76b45ca503cf6e9a4d9e8500e4f26fdaefa627106e` |
| Shared initial request | `29b64fd2ac1cd2efbb7846f77249588d7b8a0884309a103ac48c35cde2ee9362` |
| Original/repeat/Rust initial output | `4dcfdb7a4fe2a7aabbe3825a6b082d4c93dc9f6b11b87039dc9a1539e21bb645` |
| Final carried output | `40713cb9e23a42f5bc57c1f93aca808375872b586e2f1254fb538738c8c877b6` |

SF01 has an eight-byte tag/count header and at most128 records of2614-hex bytes: index/delta, full body and three saved-view words. Native consumed pointers must be zero and are relocated to owned body/wheels/table/view only, verified after the call and normalized. The pinned original PE and original helpers run without entry/import/TLS initialization, code patching, helper stubs or live injection. Lazy-data bytes/protections are restored and every executable section remains unchanged. The packed-world/source-mechanics fixture and all numeric inputs remain ignored under `local/`.

## Failures and next work

The first whole-wrapper experiment differed in7,613 words because the randomized opaque input accidentally set body2278..227B, enabling unsupported disabled-wheel branches. Aligned original kernel instructions established those four byte flags. Both adapters now explicitly reject them, and authored active-wheel controls match without output masking. That failed report remains `suspension-vehicle-20261010-debug-01/`; debug02 then matched all120 initial frames. A multiplication-order lint was resolved with a narrow documented allowance; no native fault occurred. A guessed old kernel-file path and a PowerShell wildcard search failed harmlessly; the actual existing evidence is `20261007T0151402760634Z/9c1f60.c.txt`.

All **121 library tests** (20formats, one tools, 100world), formatting, all-target warnings-denied Clippy and Python AST checks pass. Existing Rust/Cargo1.99.0, Python3.12.14, strict SSE2 MSVC14.44.35207/SDK10.0.26100.0, developer prompt17.14.41 and Capstone5.0.6 reused. No installation or desktop input. Aligned inertia/wrapper evidence: `local/behavior/suspension-inertia-wrapper-aligned-20261010.json`, excluding adjacent routines after990008/9F27AE. Steam preservation at **19:02:56.200511 UTC** confirms all130 resources plus the executable unchanged: `local/scene/preservation-20261010-suspension-vehicle.json`.

Next recover the authoritative vehicle/physical state bridge and selected outer driver order, connecting this suspension wrapper with the [complete selected force update](VEHICLE_FORCE_PIPELINE.md#joined-rust-vehicle-force-update--f166). Whole initialization/contact-pool production, actual world cadence and Bevy driving remain open. Completion estimates are unchanged; the expired overnight automation remains paused.
