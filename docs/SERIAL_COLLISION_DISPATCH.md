# Serial collision dispatch ordering — F150

10 October 2026, Europe/Kiev. The **whole original `B78710` serial branch**, including original owner/contact collection, wrapper and the original callback's disabled guard, reproduces our Rust dispatch plan's selected callback arguments and next ordering byte. This verifies dispatch with callbacks guarded off. It does not verify active callback/body integration, the task branch or a live vehicle frame.

## Behavior and Rust interface

The manager obtains the [unique registered owner list](OWNER_COLLECTION.md), using owner `+418`. When more than one owner is eligible, it logically negates global byte `F5E1A4`; when the new value is nonzero it reverses the list. Zero or one eligible owner leaves the entire byte unchanged, including noncanonical values such as 255. An owner whose contacts are all shape-disabled still participates in owner ordering, even though it will have no callback invocation.

With task manager `+134` null, each owner gets an ordered [contact selection](CONTACT_COLLECTION.md), capped at 80 and initially empty. The serial callback wrapper runs only when that selection is nonempty. The wrapper clears response values to positive zero, calls `+41C`, and consumes response pairs only when callback AL is nonzero. That differs from the previous authored pipeline's supplied nonzero initial responses, and from the task branch's apparent unconditional consumption loop.

`collision_dispatch::plan(owner_collection::Input, reverse)` composes the two independently verified collectors, applies the alternating order and returns eligible owners, next byte and nonempty callback selections. It rejects more than 32 eligible owners to respect the original caller's storage. It does not execute callbacks, allocate a pending queue, run tasks or update body state. Its returned plan is a connection point for later serial vehicle integration, not a new complete solver.

## Whole-original execution

`tools/probe_serial_collision_dispatch.c` invokes original B78710 with a private manager whose task pointer is null. Supplied owners have actual A064A0 function pointers at `+41C` and private descriptors with their ordinary `A4AF` disabled flag set. The original B77770, B775D0 and B784B0 run normally. A064A0 takes its own guard and returns AL0. No helper is replaced, no instruction is skipped externally and no executable byte is changed.

An isolated worker thread has two hardware execute breakpoints: A064A0 records actual owner/contact arguments; B77490 counts any response-consumer invocation. The exception handler records observations and resumes the same instruction with RF set. It does not change game operands or bypass a function. All tests observe **zero B77490 invocations**, as expected from the original guard.

Only the private mapped data page containing F5E1A4 is temporarily writable. Its flag is restored and original page protection reinstated; executable sections remain RX and compare unchanged. The original reference game process is never attached for this probe. The supplied 32-owner/512-per-pool bounds prevent overflow despite the owner collector's unused capacity argument. Every native callback trace must resolve to an owned owner/contact fixture.

## Results and failures retained

**128 authored cases / 348 successive invocations** match original, repeated original, Rust and independently specified Python expectations in debug/release: **15,559 trace/count/flag words per profile**, zero differences. Coverage includes:

- 2,387 actual callback invocations selecting 9,613 contact references.
- 135 reversed owner lists; 59 zero/one-owner invocations preserving the incoming byte.
- Empty pools, all-unregistered owners, all-disabled owner selections, first/second-pool eligibility, deduplication, 32 owners, repeated processing and 80-contact truncation.
- Ten malformed adapter guards reject with ordinary bounded error codes. Crashes do not count as successful rejection. Rust creates no invalid-request result file.

Private final reports: `local/behavior/serial-collision-dispatch-20261010-{debug,release}-final/validation.json`. The authored random seed is 20081010. Actual captures and simultaneous state were not used as live evidence for these fixtures.

Failed attempts remain private:

1. `...debug-01/` contains only the request: Windows refused the first probe launch with WinError740, “requires elevation.” Rebuilding with an explicit embedded `asInvoker`, `uiAccess=false` manifest allows ordinary-user execution. No original routine ran in that attempt; no elevation or system security change was used.
2. `...debug-02/validation.json` is **failed**. Original/repeat crashed with C0000005 before producing usable output because the harness wrote the mapped ordering flag on a read-only data page. Correcting the single-page writable/restored fixture setup fixes it. This was an isolated probe fault, not a GRID compatibility failure. Its initial guard success flags are not valid evidence; final validators require normal error exits below 256.
3. `...debug-03/validation.json` passes after that correction; final reports follow final builds and strengthened guards. A Rust test function declaration typo was corrected before its first formatting/build check. No original-versus-Rust numerical or ordering mismatch was observed in successfully executed comparisons.

Final native probe SHA-256: `8a2fcfff6f0a5c910fd0961b1675de55c8bac19ae7b911ec5a114307eacc1331`. Rust debug: `06236229b1d6fc4a00833fa7edce9361f5a7691bf8e03676d253616cbc6303b1`; release: `61321f10b5857856cac8d75a96e46ac9254e6e37aa89196a1ca8b44310e36839`. Analysis PE remains `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.

Existing Python 3.12.14, Rust/Cargo 1.99.0, MSVC 14.44.35207 and SDK 10.0.26100.0 reused; no installation. Probe uses `/W4 /WX /O2 /MT /std:c17` and an embedded ordinary-user manifest. **112 workspace tests**, formatting, warnings-denied all-target Clippy and Python AST checks pass. Private test/Clippy logs: `serial-collision-workspace-tests-20261010.log`, `serial-collision-clippy-20261010.log`.

Read-only Steam preservation at `2026-10-09T21:30:46.705225Z` (10 October locally) verifies all 130 traced resources and original executable unchanged: `local/scene/preservation-20261010-owner-dispatch.json`. No game/user settings were edited. Four Python AST checks pass; exact check and publication records are retained in `local/behavior/owner-dispatch-checks-20261010.md` and `local/behavior/github-owner-dispatch-sync-20261010.md`.

## What remains

The `owners` capture boundary is prepared and its read/guard contract passes two synthetic valid-caller fixtures and four invalid fixtures (`tools/check_owner_capture_contract.py`). This is not proof of live operation. A fresh observed owner list and registrations will test whether the second-pool/body assumptions and ordering flags hold in the actual selected race. Then capture active serial/task dispatch inputs and the final use of accumulated contact response before joining a synchronized vehicle update.

GRID is closed and Destiny 2 is open. Desktop availability is pending; no launch, activation or input has been sent. This is the next live-evidence dependency, not a need for a new video or owner-driven maneuver. Geometric contact creation, complete initialization/timing and Bevy driving remain open; the completion estimate is unchanged.
