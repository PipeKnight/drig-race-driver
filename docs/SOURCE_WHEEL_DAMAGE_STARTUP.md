# Source wheel regions and collision RNG in driving

F246 enables the selected collision producer in source Lacetti startup. The former deep-wall damage guard is passed: 20 ticks complete with 35 fresh contacts, 20 physical responses and a changed global damage RNG. This closes that specific initialization/integration gap; general crash fidelity and the complete original vehicle factory remain unfinished.

## Observed initialization

Aligned original `A0E880` and NED parser `9EFB30` establish named widths in rear-left/right, front-left/right order. The outer caller fills `1520` with rear/rear/front/front radii, `1530` with radii times stored f32 `0.800000011920929`, and `1540` with NED widths. At `A0F44B` Standard receives aliased `14E0` points and the `1530` auxiliary array; its examined body does not consume the sixth caller argument.

Inline `A0F4A0..A0F507` initializes four mutable damage boxes at `60/90/C0/F0`: centers from post-Standard aliased points, half extents `[named_width, wheel_radius, wheel_radius, 0]`, enable byte one, preserved padding. `initialize_with_outer_wheels` reproduces this selected argument mapping and these stores. Earlier APIs retain their explicitly supplied-array semantics.

Production then performs selected suspension setup, five NED-region stores and source HDM reset. Suspension rewrites `14E0` for its selected context after damage centers have been captured; this composition is not whole-factory equivalence. The distinct global RNG from [F244](HANDLING_COLLISION_INTEGRATION.md) is now owned by the staged runtime: six mutable-prelude draws, twelve Standard draws and six HDM-reset draws before driving. Car-local wear RNG remains separate. Reset and failed-frame rollback retain their RNG ownership. Unsupported notifications still return errors.

## Complete-output verification

SC05 executes whole original `9F3D30` and `A0C9F0`, then only the observed inline four-region loop. A thread-specific exception observer rearms TF stepping and stops at `A0F507` before the following document-constructor call. The loop completes 89 instruction boundaries without copying, replacing or skipping its original instructions. This bounded interval comparison does not execute whole `A0E880`.

43 installed CSV/NED pairs, 26 authored variations, five seeds and two Rust profiles give **690 exact comparisons / 46,632,270 observable bytes per profile**, original/repeat/Rust. Existing pointer and two undefined-padding canonicalizations remain explicit. RNG agrees after eighteen advances. Canaries, eight restored data pages/protections and the complete mapped-image comparison pass. The source variant without proven base association remains skipped.

The constructor audit reconstructs every request, checks current hashes/all outputs and validates wheel centers, extents, flags and auxiliary values. All30 framing/source/output-preservation guards pass. Historical validator report text conservatively says later wheel stores are excluded; `outer_wheels=true`, SC05, the original89-step marker and complete-output audit define the new scope. Malformed original-text and Rust-source contracts remain distinct.

Both shipped HDMs joined to source Lacetti cold SC05 state at three seeds and both wheel-view states, in both Rust profiles, give **24 exact cases / 3,244,032 complete bytes**, including24-draw RNG. Whole original settings/reset execution uses one explicit numeric-CRT adapter. The reset audit rereads all72 outputs, reconstructs source HDM/XML, verifies mutable-owner containment and preservation of all four wheel boxes. Five NED boxes are installed separately in production; this diagnostic join does not install them or execute the text/resource factory.

## Driving and playable build

All five purchased-source release tests pass. Authored shallow and center-plane walls produce3/35 contacts, respectively, over20 completed ticks. Both give20 physical responses; the deep case advances global damage RNG. These authored walls are not recovered Donington placements.

The actual source boundary test and rebuilt app complete600 full-throttle ticks at steering0.25: first difference from packed-only at tick165,176 body contacts, final Y=-14.807243 versus packed-only -147.81863. The app's132 sparse samples include100 contact-bearing rows and no simulation failure. Its inspected final image shows Lacetti beside the barrier. This is a barrier-sliding check, not a lap or arbitrary-heavy-crash proof. Existing gamepad poll error `0x80070057` recurs; scripted validation completes.

Three Bevy input/presentation/recovery tests,188 regular library tests, formatting, warnings-denied all-target Clippy, four Python syntax checks and whitespace checks pass. Rebuilt playable app SHA-256: `2dad35aca0379be2a48f55b9b5dbe99cbcd8916be4fd3b2636c14f1f0be53261`. **Play GRID Body Contacts.cmd** selects the experimental contact mode.

## Failures and evidence

First trampoline build C4100 was corrected using symbolic assembly parameter references. First stepping attempt observed one boundary: Windows cleared TF and execution escaped the intended interval, faulting at original `BFD4FC` without output. Explicit TF rearming fixes the harness. Failed captures are not compatibility evidence. A reset-validator assertion expected the wrong success-marker spelling; a fresh full run checks actual `numeric_adapters` and passes. Harmless guessed-file/literal-wildcard reads were corrected with inventory. An empty final documentation patch hunk failed atomically and was reapplied correctly.

Ignored private evidence:

- `local/behavior/outer-wheel-callsite-20261013-draft-01.json`, `wheel-width-names-20261013-draft-01.json`, `outer-wheel-arguments-20261013-draft-01.json`; original Ghidra `20261012T2124078696226Z/a0e880.c.txt`, `20261008T1239559195883Z/9efb30.c.txt`, `20261007T1822321790621Z/a0c9f0.c.txt`.
- Ten `outer-wheels-standard-{debug,release}-20261013-final-{200679,0,1,0x80000000,0xffffffff}/validation.json` reports. Audit `outer-wheels-standard-audit-20261013-final-02/audit.json`, SHA-256 `944c2d938b10bdc109c683fc164028a0f8c88ec0a542015d1783cd904f2a9d6f`.
- Native SC05 SHA-256 `97ea5709ab18f0d92c8302b135f3f43e78afc45d9c68c466a08e25bc53516c89`; Rust diagnostic debug/release `5f792fb32dfd650b9e7f592af66000927af52842b53c3773a1c68e86bef1400c` / `e023ba7e137010544cee13e08e35b6292e5d89a5866bc1a1c3711d8e0eb71894`.
- `outer-wheels-reset-20261013-final-02/validation.json`, SHA-256 `10126c351b5c08456a55e59f9658cc7097fe8fed1fec55b59975ed5e17ff4481`; `outer-wheels-reset-audit-20261013-final-01.json`, `5936535f0a3468fb0192a59f3460b6c38f67d66e1ac5cd0ee9cdcc4ce51da289`.
- `local/behavior/outer-wheels-{driving,libraries,clippy,app-tests,app-build}-20261013-final-01.txt`, heavy-contact draft and failed trampoline/reset captures.
- `local/playtest/outer-wheels-boundary-20261013-final-01.{txt,json,png}`: image SHA-256 `c0dd570f22b35fec81188222e35cc0cfc54570ace0646d96ab4c621403e09981`, log `a25817adf18d2b1ca08b1529cb212e9f97a845b661059403ade3cfe03c59fc4e`.
- `local/scene/preservation-20261013-outer-wheels.json`: all130 Steam resources/executable unchanged at2026-10-12 22:27:38.757267UTC. Filenames use the owner's13October session date; UTC is recorded exactly.

Reused Python3.12.14, Rust/Cargo1.99.0, MSVC14.44.35207/SDK10.0.26100.0, Capstone5.0.6, Bevy0.19.1 and RTX4070 Vulkan driver616.92. No installs, desktop input, settings changes or Steam mutations. Shared materials, authored scheduling/boundary order, warm-cache ownership, notifications and arbitrary crash fidelity remain limitations. Next verify reached heavy-contact reset/replay and failed-frame rollback at the complete runtime boundary, then broaden controlled source-barrier driving.
