# Collision response consumption — F148

10 October 2026, Europe/Kiev. Independent Rust now reproduces the whole original `B77490` response consumer for supplied contacts, ownership, response values and pending state. It also accepts the output of the [complete callback](COLLISION_CALLBACK.md). This closes a scalar bookkeeping handoff; geometric contact creation, live task ordering and a complete vehicle update remain open.

## Recovered behavior

Aligned instructions and private decompilation establish a six-stack-argument stdcall routine (`ret 24`): current owner, ordered contact-pointer array, signed count, parallel float responses, pending-pair array and pointer to its active count. Nonpositive selection count returns immediately. Each contact has first owner at `+04`, second owner at `+08`, second-object kind at `+0C` and accumulated scalar at `+38`. Owner callback registration is nonzero at `+41C`.

For each selected contact, in order:

1. Check the first owner and, only for kind zero, the second owner. If neither is a different registered owner, add **current response + accumulated scalar** directly. This path neither searches pending pairs nor clamps negative results.
2. Otherwise search the active pending array from its beginning for the same contact identity. On a match, first compute **pending value + current response**, storing the result in the response array. Then compute **that response + accumulated scalar**. Clamp this final scalar only when strictly below zero. Negative zero survives.
3. Remove the matched pair by replacing it with the last active pair, then decrementing the count. Removal changes subsequent lookup order.
4. If no pair matches and fewer than 240 pairs are active, append the contact and current response. At capacity, discard this new contribution. An existing match is still consumed at capacity.

Repeated selection references are processed repeatedly. Duplicate pending identities are permitted; only the first match is removed. Unregistered counterparts and nonbody second objects do not cause deferral. SSE scalar addition order matters; append uses an x87 load/store without arithmetic.

This routine does not itself update body velocity or pose. Calling `+38` an accumulated scalar describes the observed arithmetic, not a proven physical unit or its final solver interpretation.

## Implementation and supported scope

`crates/grid-world/src/response_consumption.rs` exposes `consume`, with explicit owner/contact identities, registration flags, ordered selection indices, responses and active pending pairs. It returns owned state without modifying caller input. `crates/grid-tools/src/bin/grid-response-consumption.rs` is a private RSC1 diagnostic adapter; it validates and computes the entire request before create-new output under `local/`.

The supported public kernel bounds are 512 unique contacts, 80 selected references and 240 active pairs. The 240 limit is original behavior; the other bounds, nonzero unique identities, consistent owner registrations and finite input/result checks are our supported diagnostic contract. They are stronger than the native routine's assumptions. The CLI additionally bounds owners to 64, passes per case to eight, cases to 1,024 and input to 32 MiB. Unselected supplied contacts are validated too. Overflow and malformed inputs return an error without changing caller state.

RSC1 uses little-endian u32/f32 words. Header: magic `0x31435352`, case count. Each case contains owner/contact/pending/pass counts; owners `(identity, registered)`; contacts `(identity, first owner, second owner, kind, accumulated)`; pending `(contact identity, value)`; then passes `(current owner, selected count)` and `(contact index, response)` entries. Owner zero denotes absence only in contact fields. Output has the same header, pass count per case, then each pass's contact/selection/pending counts, all accumulated scalars, responses and active `(identity, value)` pairs. Inactive native queue storage is deliberately outside the comparison.

## Original execution and evidence

`tools/probe_response_consumption.c` invokes the **whole original routine**, with owned bounded buffers and normalized identity tokens. No imports, helper stubs, skipped instructions or executable writes are needed. Executable sections are checked unchanged after the run. The native malformed-output file may be partial; the Rust adapter must not create a result for invalid requests.

Private sources and identities:

- Analysis PE: `local/compatibility/grid-unpacked.exe`, SHA-256 `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.
- Aligned instructions: `local/behavior/response-consumption-instructions-20261009.json`; earlier decompilation: `local/behavior/20261009T0854252290593Z/b77490.c.txt`.
- Final probe: `local/behavior/response-consumption-probe.exe`, SHA-256 `8ec365fc477f27995c9f2a20807ff8acd222651a9d656aad43f36552c22d189e`.
- Rust diagnostic debug: `73c72b5648ddfdd6ac0790936fc5024f7997fe2a5deb51c48b90a6e974a6204b`; release: `0586e1e9e53e1d52148a59706063fffe2c51f8628e707ff581c0e94e693c9a16`.

`tools/validate_response_consumption.py` supplies **512 authored cases / 2,175 passes** (seed 20081010). Original, repeated original and Rust exactly match **340,652 output words per profile**, including counts and identities. Twelve separately specified semantic checks cover signed zero, nonassociative grouping, negative direct updates, matched clamping, duplicate selections, swap-last ordering, full-queue drop/match and multiple owner passes. Fourteen malformed guards reject, including overflow and unknown owner/contact identities. Final reports:

- `local/behavior/response-consumption-20261010-debug-final/validation.json`
- `local/behavior/response-consumption-20261010-release-final/validation.json`

The earlier debug-01 run also passed; final runs follow the last code build and added unit tests. No numerical mismatch or failed compilation was observed in this milestone.

## Callback-to-consumer connection

`tools/validate_callback_consumption_pipeline.py` freshly executes 144 handled callbacks with nonnull response arrays, selected from the prior 272 authored callback controls. Whole original/repeat/Rust callback state matches **1,561,525 buffer/return words + 144 controlled seed words** in each build. All 144 have nonzero response values; 85 change the supplied response array.

Each output feeds two consumer scenarios, with empty or prefilled queues, mixed direct/deferred owner roles, authored initial accumulators and a second owner pass. **288 scenarios / 576 passes / 12,812 consumer words** exactly match in debug/release. Every scenario changes accumulated state and first-pass pending state. The Rust consumer receives the Rust callback's computed responses, not copied original outputs.

These are **authored hybrid fixtures**, using producer capture10 and callback capture02 as templates. Owner roles, scalar starting state, pending pairs and second-owner responses are explicitly authored. The two original routines execute in separate private processes. This verifies composition for these supplied inputs; it does not reproduce one observed simultaneous game state, native manager scheduling or a live physics frame. No live `B77490` capture has been made.

Reports: `local/behavior/callback-consumption-20261010-{debug,release}/validation.json`. Their capture hashes and current callback/probe/build hashes preserve exact provenance. Source captures remain `live-impact-20261009-10/capture.json` and `live-callback-20261009-02/capture.json`, with the identities documented in the preceding contracts.

## Checks and next work

Existing tools reused: Python 3.12.14, Rust/Cargo 1.99.0, MSVC 14.44.35207, Windows SDK 10.0.26100.0, Capstone 5.0.6 and Ghidra 12.1.4/JDK 21.0.9. Probe builds with `/W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2`. All **108 workspace tests**, formatting, warnings-denied all-target Clippy and both new Python AST checks pass. No tool installation or desktop input was needed.

Steam preservation at `2026-10-09T21:11:10.875148Z` (10 October locally) verifies all 130 traced resources and the original executable unchanged: `local/scene/preservation-20261010-response-consumption.json`. No game or user settings were edited.

Next recover the manager's ownership collection, pending lifetime/order, geometric contact production and the final consumer of the accumulated scalar. Connect these with authoritative vehicle initialization/timestep/state transfers before claiming a complete driving frame. A live capture should be prepared around a precise unresolved boundary before requesting another owner maneuver. No owner help is required for the next offline analysis; the full-engine completion estimate is unchanged.
