# Complete synchronous vehicle wrapper

F192 now verifies [whole registeredA0A290 job](CAR_JOB.md) with owned observational completion. Positive driver itself writes9A0=1; job adds no later clear or synchronous pose projection. Live task dispatch/query/notification contracts remain separate.

F191 now verifies [whole selected mutable handling and A03220(delta,1)](VEHICLE_HANDLING.md), required by CarJob. Whole A0A290 scheduling/completion and live component queries/notifications remain open; this does not make the synchronous projection the normal race path.

11 October 2026, F187. The independently authored Rust `vehicle_outer::calculate` now matches one unchanged original `A09A80` call within the selected scope. This closes the synchronous wrapper comparison, **not the live racing job/world schedule or the first drivable lap**.

## Recovered order and an important correction

Aligned instructions establish the callee-popped two-argument ABI (`ret8`) and the sequence: `9BD7F0` synchronization, `A06A90` driver wrapper, `A03220(delta,0)` publication, clear car byte `9A0`, then pose and velocity setters with pose notification enabled. There are no getters after the final setters. Rust therefore preserves the car's own pose while updating its linked physical publication.

The first complete comparison failed on 250 physical-publication words across 32 cases. The prior statement that the final `A09A80` masks only velocity W was incorrect. Its actual `CAF400`, `CAF410`, `CAF420` masks select X, Y, Z. This wrapper zeros **linear X, angular Y and position X/Z in local setter arguments**. It leaves those car words unchanged. Earlier `A03220` independently clears velocity W and remains correctly implemented.

A bounded read-only capture from the existing private GRID process, PID5092, at 08:58:55.944580 UTC checked the executable hash and process start identity. Its 112 mask bytes, 368 bytes at `A09A80` and 304 bytes at registered job method `A0A290` all match the hash-pinned analysis executable exactly. An older startup runtime image also contains the same masks. This rules out a static-versus-current-memory mask discrepancy; it does **not** prove that `A09A80` is the normal racing update caller.

`CarJob` method `A0A290` instead executes sync → driver wrapper → `A03220(delta,1)` → job completion callback. It does not contain the synchronous wrapper's final axis projection/setters. The notification-one consumer and real world publication/advance path require further work. Do not wire the synchronous axis projection into a general playable vehicle loop on the assumption that it represents live racing.

Corrections also apply to the W-only interpretation in [physical bridge](VEHICLE_PHYSICAL_BRIDGE.md) and [synchronization](VEHICLE_SYNCHRONIZATION.md). Their previously compared setter/helper arithmetic remains valid; their composed schedules were explicitly authored. [F186](VEHICLE_PUBLICATION.md) remains a composition with notification disabled and getter readback, not this complete wrapper.

## Ownership and comparison

The public Rust call owns full `21000` car and `424` physical buffers plus frame metadata. Synchronization replaces the authoritative prefix, the driver resolves contacts from local Donington resources, publication updates decay/velocity, and the final setters receive the observed local axis projection. Clone-before-work preserves all input buffers on failure, including a failure at the final notification boundary after the preceding stages succeeded.

The native `OF01` adapter extends the established driver record with the remaining car tail and physical buffer. All wrappers, component links, driven/steered wheel links, source-material tokens, physical matrix pointers and group storage are owned and bounded. Conservative preflight runs synchronization on scratch entry buffers so subsequent guards inspect the state the driver will consume; entry buffers and synchronization SIMD scratch are restored before the **sole result-producing unchanged `A09A80` call**. No helper replacement, executable write, imported entry/TLS execution or live injection produces the result. Generated callback addresses are checked against the loaded image then normalized to an opaque identity; they are never executed by Rust. Executable sections are checked unchanged and seven protected driver-global pages restored. Partial outputs are removed on failure; private outputs are create-new.

Each original/repeat/Rust branch carries its own complete buffers. Constructor, controls and interframe history are authored. The native wrapper's expired temporary view/solver is never followed or compared; complete car, physical, material and diagnostic state remains compared.

## Verified results

Both debug and optimized release compare **428 calls, 58,305,584 full car/physical bytes and 58,884,240 observable record bytes per profile**, exactly against original and repeat. These include 140 initial cases, 192 carried updates, 96 zero-delta cases and all44 supported source vehicle tables.

Coverage per profile: 201 mode transitions, 213 mode-two outputs, 256 engine changes, 1,308 wheel spin changes, 310 contact slots, 428 physical pose changes, 252 mass changes, 252 flag resets, 184 timer and 39 decay changes. All36 paired ownership/finite/setup guards and14 paired framing/alias checks pass. Fresh whole `A06A90` regressions retain428 exact cases per profile and36 guards. All149 library tests, formatting and warnings-denied all-target Clippy pass.

Scope: assisted automatic non-neutral gears and modes1/2, stable setup, empty manager/render/UI/shape lists, pose notification without reset-group behavior, no physical advance/correction. Arbitrary unsupported branches remain explicit failures. This is function-equivalence evidence within that scope, not gameplay equivalence.

Private evidence under ignored `local/behavior/`: `whole-vehicle-outer-{debug,release}-20261011-final-03/validation.json`, their guards/framing/execution/input/output files, `whole-vehicle-step-outer-regression-{debug,release}-20261011-final-03/validation.json`, `whole-outer-masks-20261011.json`, `whole-outer-live-masks-20261011.json`, `whole-outer-live-code-verification-20261011.json`, `whole-vehicle-outer-aligned-20261011.json`, and `whole-outer-final-audit-20261011.json`. Initial mismatch evidence remains in `whole-vehicle-outer-debug-20261011-draft-01/`.

Existing Rust1.99.0, Bevy0.19.1, Python3.12.14, Capstone5.0.6, MSVC14.44.35207/SDK10.0.26100.0 tools reused; no installation. Native strict SSE2/x87 53-bit-nearest configuration remains unchanged. The analysis PE SHA256 is `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`; source/execution hashes remain private in audit reports.

UI activation failed with conflicting input/minimized messages; stopped retries and used read-only memory inspection through normal sandbox escalation. No driving macro or settings edit occurred. A launch attempt also exposed an original-Steam “already running” dialog; no original resource was modified. Next recover `CarJob` notification-one processing and actual world contact/advance/publication ordering, then source construction and Bevy driving integration.
