# Preparation inside the physical solver frame

13 October 2026. `impact_frame` now has internal, owned, staged callback phases: preparation after body preparation/mass normalization and before the main solve; solved callbacks after successful solving and before optional correction/final publication. The existing public no-callback path uses an empty context. A later error drops the cloned callback state.

`deformation_physical_frame::calculate` supplies the verified preparation callback for a single body and static first-pool contacts. Whole original **B7B850 -> B78530 -> B77850 -> 9A0C90**, main solver and final publication match the independent Rust path. This proof deliberately sets **first shape230=1**, so original B775D0 excludes the contacts from the later bulk callback collection. Both original callback pointers are valid registrations, but **the bulk callback is not invoked**. This is an explicit collection-disabled control, not full deforming-frame equivalence or a live driving result. The adapter requires an owned shape table and checks that every contact is excluded from bulk collection; it does not relax that guard for active runtime shapes.

Each debug/release profile independently passes:

- **256 cases /14,779,392 complete output bytes**, original/repeat/Rust exact: complete physical bodies, contacts, constraint rows, properties/workspace/cache and grid descriptors.
- 195 successful frames, 11 started solver-capacity failures, 50 unstarted sleep/zero-iteration bypasses, 142 changed impact windows.
- Zero through64 static contacts, source-grid/mass/pose/velocity inputs and authored independent solver settings, disable gates, previous cap limits, force/drag/damping/shift state and iteration counts.
- 18 guards /50 rejection or existing-output preservation outcomes. Null shapes, foreign registrations, dynamic contacts, invalid descriptor/grid/ring/node data and invalid cap parameters are rejected before original execution.
- Reconstructed read-only audit verifies69 files and five current source identities per profile, executable hashes, every request/guard, complete outputs and descriptor mutation whitelist. Whole restored original image equality passes, with descriptor canaries and owned body tails checked.

The existing no-callback suite was replayed with the rebuilt debug/release diagnostics: **695 cases /15,581,940 complete output bytes per profile**, identical to the retained original answers. An additional owned-context test observes prepared gravity/normalized mass before solving, integrated velocity afterward, and pending-shift publication after that callback. An intentional late pose overflow leaves both the caller's callback state and input unchanged. 197 regular libraries (25 formats +4 tools +168 world), formatting, all-target Clippy and Python syntax checks pass.

## Evidence and corrections

Private evidence under `local/behavior/`:

| Evidence | SHA-256 |
| --- | --- |
| deformation-physical-frame-debug-20261013-final-02/validation.json | 67db042ac781e367666e047e55006be45dc7dfc020c4c9270098be4af5f9fa38 |
| deformation-physical-frame-release-20261013-final-02/validation.json | 7b9fb7a731ed8f7c4e1370e1d7fc97dcfb96090fc99442f1f00a822171374973 |
| deformation-physical-frame-audit-20261013-final-02.json | 18cbb9ccad2d1c77bb0fd9b5ac487e7eeff444d0808ff786ff728f5c5b95c799 |
| deformation-physical-frame-legacy-20261013-final-01/validation.json | 66fbf61c288251d65e513cb1aa2550d63eac82dc194e5e2d4936a6e71d415311 |
| deformation-physical-frame-probe.exe | 53682d2f7eb6359a8184df43e50f0a24bddc485a32035a09c3aa3fb704f44504 |
| target/debug/grid-deformation-physical-frame.exe | 6fc26e0fffa07bcc53526d5968b9de582e15d03abb99f62ba1197c96444bf9ab |
| target/release/grid-deformation-physical-frame.exe | aa781f8ba4bfd6f2fc25e4666a169e3312ed470e8b1e3f7add5c1ec430dad643 |

Tools share the `deformation_physical_frame` name; PFD1 extends the prior complete-frame diagnostic. Original code/helpers remain unchanged, no imports/TLS/entry point run, eleven private data pages are restored and the complete mapped image compared. Reused Rust/Cargo1.99.0, Python3.12.14, MSVC14.44.35207/SDK10.0.26100.0, Ghidra12.1.4/JDK21.0.9, Capstone5.0.6; strict native SSE2/x8753-nearest. Original PE identity is pinned by `isolated_reference.h` and the source-grid validator.

Retained pilot failures:

1. Draft01 case1 C0000005/EIP04f07605/read230: null shape and the old220-byte shape allocation were invalid for **post-collection B775D0** shape230 reads. Both shapes are now nonnull owned240-byte objects. The initial project-log attribution to B77770 was incorrect; B77770 collects registered418 owners, whereas B775D0 reads the shape disable bytes.
2. Draft02 case1 C0000005/EIP0/read0: registration418 also makes the owner eligible for later B78710; a null41C callback was invalid. Both real original registrations are now installed and shape230 explicitly disables bulk collection. This corrects the earlier assumption that a missing41C registration naturally bypassed that later stage.
3. Draft03 Rust rejected random-bit unused constraint rows containing nonfinite values. Authored finite rows now satisfy the established solver domain; no guard was weakened.
4. An untyped Rust indexing closure failed compilation and was given an explicit usize argument before running. Final01 passed; final02 repeats after adding upfront domain validation even for sleep/zero-iteration bypasses. No final native fault or numerical mismatch.

## Next integration

Keep this control while adding the real post-solver raw-state adapter and proving both active callbacks together, carrying owner history, source settings, the separate CRT seed, contact responses and pending shifts. Then consume dirty grid state through wheel/shape/presentation updates transactionally. The clean c3696050d8f6 playable build remains available; no visible deformation is claimed here. Source/Bevy checks and the next clean periodic build are pending. No owner help, installation, desktop input, Steam/settings change or approval rejection.
