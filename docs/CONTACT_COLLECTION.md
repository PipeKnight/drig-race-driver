# Ordered contact collection

9 October 2026 continuation after [F142 collision composition](IMPACT_PRODUCER.md#independent-rust-composition-f142). **F143:** six actual `B775D0` collector calls and256authored controls match whole original/repeat/independent Rust in debug/release. This is collection of existing contacts, not geometric contact detection equivalence.

## Static contract

Private targeted decompilations `local/behavior/20261009T0854252290593Z/` cover A064A0, B775D0, B77770 and B77490. Aligned instructions: `contact-dispatch-instructions-20261009.json`, `contact-collector-callers-20261009.json`. B775D0 is a **thiscall receiver plus four stack arguments**, returning with `ret16`. Its caller supplies owner linked state, pointer-output array, pointer to count and capacity80. It appends to the existing count; it does not clear it.

The receiver's first pool is pointer-array98/countAC, its second is pointer-arrayA0/countB4. In pool order, the first accepts only contact04==owner. The second accepts contact04==owner or (contact0C==0 and contact08==owner). Both require zero disable bytes at each shape pointer28/2C plus230. Append only while count<capacity. There is no identity deduplication in this collector; repeated contacts/pool entries remain repeated. This collects already-existing contacts; geometric detection and pool construction are upstream.

[F149](OWNER_COLLECTION.md) verifies B77770's unique owner collection using linked418, for supplied pools/registrations. [F148](RESPONSE_CONSUMPTION.md) verifies whole B77490 response/pending consumption for supplied ownership and state. [F147](COLLISION_CALLBACK.md) implements and verifies A064A0's reduction/inverse/preparation/producer composition. These later milestones supersede the earlier preparation gap; live manager ordering and geometric pool production remain open. Pseudocode types are inferred and instructions take precedence.

## Actual reference capture

`tools/capture_reference_spawn.py --boundary collection --arm-running --hardware` reuses exact process/start/executable and code guards. It restricts entries to registered player callback A064A0, verifies Standard-body membership/wrapper linkage, caller returnB787D6 orB788F3, initial count0/capacity80, and reads at most512entries per pool. Entry/return stack balance includes callee-popped16argument bytes. Only hardware debug registers are used; no game code patches or Python input. Output identities must belong to the captured pool.

Private `local/behavior/live-collection-20261009-01/capture.json`, SHA-256 `b803d7f8734d95ebd4b52e802b873fbb42ec9f9ffd060c93e1ca276c6009e63e`: completedtrue, exact code, hardware watch, debuggerexit0. Existing private GRID PID36228/start2026-10-08T23:53:45.3524205Z and macro6868/start2026-10-09T07:23:22.0284687Z were freshly verified. Computer Use observed paused Continue near the barrier and triggered the guarded F5 sample. No race restart or stale body identity reuse occurred.

Six entry pool counts:52/80,54/86,56/87,54/89,55/76,54/80 (823entries total). Returned counts3/2/4/5/2/4 (20contacts). Both shape disable bytes are zero for every captured entry; disabled-shape behavior therefore needs authored controls. All six return at **B787D6**, directly proving the task-dispatch collection branch for this sample. It does not establish all race scheduling or task execution order.

Macro log `local/reference-race/impact-20261009-hardware/macros-left/20261009T0723218394594Z-serve.tsv` records throttle2000ms at08:57:22..24UTC and normal ending pause pulse. Debugger pauses consumed wall-clock macro time and the game remained running afterward. A fresh observed F11 explicitly paused it; final inspected state is Continue. Macro wall time is not simulation duration. All original captures/decompilations remain ignored/private.

## Whole-original and Rust verification

`tools/probe_contact_collection.c` maps the pinned private analysis image and executes the complete original collector using reconstructed pointer/shape fixtures. It supplies ECX plus four stack arguments through a small independently authored x86 bridge; there are no imported APIs called by the original routine, helper stubs or code patches. All executable section bytes are checked unchanged. Probe SHA-256 `df53e31997019def6f067d215bb762db5104754749b8228ea4d8b53645132d1a`, MSVC14.44.35207/SDK10.0.26100.0 `/W4 /WX /O2 /MT /std:c17`. The initial compile failed because C does not support the attempted thiscall function-pointer declaration; the explicit bridge corrected it. No failed binary was executed.

`grid_world::contact_collection::collect` uses opaque identity tokens, ordered typed pools, shape-disable booleans and an optional initial output prefix. `grid-contact-collection` supplies a bounded private COC1 diagnostic: magic0x31434F43/u32case count1..1024; row owner/capacity/initial count/two pool counts, initial tokens, then24bytes per contact (identity,first,second,second-kind byte represented as u32,two disable flags). Output is count+tokens for each row. Native fixture pointers are normalized back to these input identity tokens; no geometric/numerical data is masked. Booleans preserve the original zero/nonzero predicates.

`tools/validate_contact_collection.py` checks exact original/repeat/Rust identity order against the six live outputs: **20contact tokens+6counts =26words**, all exact in both builds. Reports `local/behavior/contact-collection-live-20261009-debug-01/validation.json`, `...release-final/validation.json`. Authored controls (seed20081009) cover empty pools, first/second owner asymmetry, second-object kinds, both disable flags, repeated identities within/across pools, capacities1/2/8/32/80, full and partial initial prefixes, and512-entry pools. **256cases/4,206output words** match exactly per build, including3,950returned/preserved identity tokens: `contact-collection-controls-20261009-debug-01/validation.json`, `...release-final/validation.json`. Ten malformed guards reject per run; Rust creates no output, native error output can be partial. No numerical or ordering mismatch observed.

Rust diagnostic SHA-256: debug `b59d504a86506f0e6054398f1f3f38ca04606a44ac21ca36f678edcd2f50e0d6`, release `1781ca5373042fef69953fbcf199cd3354b550e5129fd0fddf495686777a3851`. Rust/Cargo1.99.0;104workspace tests/fmt/warnings-deniedClippy/PythonAST pass. Steam preservation at2026-10-09T09:03:41.528915UTC:130traced resources+original executable unchanged, `local/scene/preservation-20261009-collection.json`. Existing Ghidra12.1.4/JDK21.0.9/Capstone5.0.6/debugger/AHK reused; no new tools installed.

## Next work

Recover callback contact reduction/preparation and its descriptor side effects, then geometric contact-pool production. The collector and F142 producer have separate supplied boundaries; they have not yet been connected to one same-tick captured callback or integrated into Bevy driving.
