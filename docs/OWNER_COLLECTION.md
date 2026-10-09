# Collision owner collection and dispatch ordering — F149

10 October 2026, Europe/Kiev. Whole original `B77770`, repeated original and independent Rust match 512 authored cases in debug/release: **8,664 owner-token/count words**, zero differences. This extends [contact collection](CONTACT_COLLECTION.md) and [response consumption](RESPONSE_CONSUMPTION.md). Later [F150](SERIAL_COLLISION_DISPATCH.md) verifies whole serial dispatch's callback-selection/ordering trace with callbacks guarded off. No live owner-list capture or active manager equivalence is claimed yet.

## Owner-list contract

Aligned instructions establish **thiscall plus two stack arguments**, output pointer and an unused capacity value, returning the count in EAX with `ret 8`. The earlier decompiler's void return and single argument are wrong. Callers `B78530` and `B78710` pass 32 and use EAX. The original collector never reads that capacity argument and never clamps output to it; caller storage must be sufficient.

The manager's first pointer pool is `+98`, signed count `+AC`; second pool is `+A0`, signed count `+B4`. Traverse first-pool contacts in order, considering only each contact's first owner `+04`. Then traverse second-pool contacts in order, considering first owner `+04` followed by second owner `+08`. Append an owner only if its registration slot `+418` is nonzero and it has not already appeared. Duplicates are suppressed by owner identity; first occurrence wins. Contact shape-disable flags are not inspected. This differs from `B775D0`, which filters shapes and retains repeated contact references.

The selected slot is **+418**, distinct from response callback **+41C**. Standard body setup populates these with `9A0C90` and `A064A0`, respectively. Owner eligibility must not be inferred from the wrong callback slot.

The original dereferences the first owner without a null guard. In its second pool it also dereferences the second owner without a null guard; a nonzero second-object kind forces that pointer to zero before dereference. Consequently the supported second pool requires two valid body owners and kind zero. This is an observed native precondition, not proof that arbitrary game contacts are invalid. The generic contact collector accepts broader supplied fixtures; those cannot all be passed to owner collection.

`owner_collection::collect` uses the existing `contact_collection::Contact` representation and explicit owner registration records. It bounds supplied owners to 64 and each pool to 512, validates identities and required pointers, and returns unique owner tokens. These are our diagnostic bounds, not recovered native clamps. First-pool second-owner fields are irrelevant. The production callers' 32-entry storage is a separate, tighter live-capture limit.

## Manager ordering recovered statically

`B78710` gathers owners, then, only when count exceeds one, toggles byte `F5E1A4` using logical negation. It reverses the owner array when the new byte is nonzero. Zero/one owner leaves the byte unchanged. This can change callback and response-pair processing order between updates; it is not a fixed globally sorted list.

The serial branch (`manager+134 == 0`) collects at most 80 contacts per owner, calls `B784B0` only for nonempty selections, and retains one pending-pair array/count across owners during that invocation. `B784B0` zeroes the selected response buffer, invokes owner `+41C`, and calls `B77490` only when callback AL is nonzero. Thus supplied response data from earlier standalone controls is not the native serial wrapper's initial response buffer. The callback's own bypass means consumption is skipped in this branch.

The task branch prepares groups of up to 32 tasks, waits through `A7D530`, then consumes results in its loop. Its consumption is not gated here by callback AL. The function's pending count starts at zero for the invocation; a leftover pair is not demonstrably a cross-frame cache. Precise task completion/order, the later-batch owner-index behavior and the physical solver's final use of contact `+38` remain unresolved. Static caller analysis does not establish a live execution cadence or an independently executed whole manager.

The earlier preparation manager `B78530` also uses `B77770`, then calls `AD7DC0` on the owner list before invoking `+418` callbacks through `B77850`, directly or in tasks. Its ordering operation remains unverified; do not reuse the impact manager's alternating policy for it.

## Evidence and validation

- Private aligned code: `local/behavior/owner-collection-instructions-20261010.json`, hash-guarded against analysis PE `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.
- Xrefs: `local/behavior/20261009T2115027801417Z/address-refs.tsv`; aligned manager caller instructions: `contact-collector-callers-20261009.json`.
- Manager pseudocode: `20261008T2336554686053Z/b78710.c.txt`; wrapper: `20261008T2218241115216Z/b784b0.c.txt`; fresh earlier-manager/task decompilation: `20261009T2117579969697Z/b78530.c.txt` and `b77ac0.c.txt`. Decompilation of guessed `B77B30` failed because no function was found; no behavior is inferred from that address. Several guessed filename/wildcard lookups also failed before locating the existing capture script and evidence; no evidence was changed.
- Whole original probe: `local/behavior/owner-collection-probe.exe`, SHA-256 `26c27384cea06e9c3d71d95093ff7716a1bc2061b8860e08fc9af0fc89bceb82`. No imports, helper stubs, skipped instructions or executable writes. Owned buffers are large enough for all unique supplied owners, regardless of unused capacity. Executable sections checked unchanged.
- Final Rust diagnostic debug SHA-256 `7af91570892938cbfca9a654f1ab06c4372c9b01d7ca293e60a05ab35aac1fcd`; release `92348944f8afa6dd768f858b7595ad3ef5059428f4ba3143dbeca7cd2b5666a3`.
- Final reports: `local/behavior/owner-collection-20261010-{debug,release}-final/validation.json`; earlier reports also pass. Seed 20081010. Independently specified controls cover first/second-pool ordering, deduplication, missing registration, ignored first-pool second objects, empty pools and 64 owners with capacity zero. Capacity 0/1/32/FFFFFFFF gives the same result. Ten malformed guards reject with ordinary bounded error codes in the probe adapter and Rust; these do not claim that the native routine safely rejects invalid pointers.

ONC1 private adapter: magic `0x31434E4F`, case count; each case has owner/first-pool/second-pool counts and unused capacity, `(identity, registered)` owners and `(first identity, second identity, kind)` pool entries. Output has header then count/ordered identities for each case. 1,024 cases /32 MiB input, create-new private output only after all Rust cases validate. No original assets are published.

Existing Python 3.12.14, Rust/Cargo 1.99.0, MSVC 14.44.35207 and SDK 10.0.26100.0 reused; no installation. Strict `/W4 /WX /O2 /MT /std:c17` probe build succeeds. **110 workspace tests**, formatting and warnings-denied all-target Clippy pass. Outputs: `owner-collection-workspace-tests-20261010.log`, `owner-collection-clippy-20261010.log`. No numerical/compiler/lint failure observed.

## Next live check

`capture_reference_spawn.py --boundary owners --hardware --arm-running` is prepared to capture original owner-collector entry/return at both verified callers, their bounded pools and separate registration slots. It checks the `ret 8` stack change, output uniqueness/known identities and the caller's 32-owner storage limit. `validate_owner_collection.py --capture ...` prepares whole original/repeat/Rust comparison against the observed returned list. `check_owner_capture_contract.py` passes two valid and four rejected synthetic memory fixtures. Actual operation of this added capture boundary remains unverified until run.

Read-only app/process inventory finds GRID closed and Destiny 2 open. No launch, activation, macro, keyboard or mouse input was sent. Desktop availability was requested; independent offline work continues while that answer is pending. Next priorities are whole serial dispatch ordering, then a fresh owner/response capture before assuming synchronized manager state.
