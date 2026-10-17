# Initial impact constraint rows — F156, 10 October 2026

Complete original single-body B0E040 and two-body B0D1B0 now match independent Rust in debug and release. This includes original material mixer/cache B3B130, compliance helper B82550, cross product67C200, lane accessor4B1A70, normal rows, all three grouped friction rows, context counters/cursors/heads and cache. No original instruction, helper or branch is replaced.

The inputs are **supplied contacts**, with authored geometry, roles, velocities, materials, parameters and grouping.5,840accumulated-contact operands/disabled bytes are reused unchanged from F152's verified authored callback handoff; these are not synchronized live-frame captures. [Optional correction](IMPACT_CORRECTION.md) and [normal response](PHYSICAL_RESPONSE.md) remain separate verified paths until main preparation and friction solving join them. Geometric contact production, full physical frames and driving remain unverified.

## Observed contract

Both functions are thiscall, contact in ECX and one context stack argument. Each consumes one192-byte normal row. Body +334 points at its position, +210/+220 supply angular/linear velocities. The second body's angular cross reverses operand order and its linear normal sign-flips. Body +410, optional shape +216 and a strict absolute-normal-Y threshold jointly enable angular scaling. File-backed E52F24/E52F28 are both0.2f32; the adapter supplies these mutable globals explicitly.

Identical material pointers return directly; either orientation of a cached pair retains the cache. New pairs geometrically mix +04..+14 through separately rounded f32 multiply/SQRTSS, retaining equal operands. Fields +18..+28 recompute only if either material's +18/+1C is positive, with zero operands selecting the other value; otherwise those cache fields survive. Output +2C clears, then the first material with positive +2C supplies +2C/+30; when neither qualifies, +30 remains unchanged. Opaque +00 survives. Decompiled integer/float casts here reinterpret bits; they do not convert numerically.

The ordinary branch interpolates compliance from relative linear speed/material +28. For non-MAX stiffness/damping with at least one nonzero, the helper computes `q=(1/time)*stiffness`, `inverse=1/(q+damping)`, returning `inverse*q` and `inverse`; otherwise it uses context/material fallbacks. Contact +73 bypasses that work and caps penetration bias at material +10. Positive softness overwrites row +84; zero/negative softness preserves it. Restitution uses constructed angular Jacobians and supplied velocities. Contact +3C limits upper impulse except at MAXf32. **Disabled byte +0E does not bypass construction**; its meaning is downstream.

Group sums accumulate point, normal and friction. Contact +70 emits one pure angular-normal friction row and two linear tangent rows, then clears nine sums/count to positive zero. Open groups retain sums and emit no friction rows. Average point uses a stored f32 reciprocal; average friction retains the x87 reciprocal through multiplication. Normal length uses RSQRT refinement and a1e-4positive-Y fallback. Tangent axis switches at absolute normal Y0.707f32; tangent generation uses original cross/refined reciprocal square root. Single-body +40F can make the last tangent bounds asymmetric with factor2 on the Y-axis-selection branch.

Normal/tangent batches are independent for single/two-body pools. Body-index change writes the previous head's +B4 count; final current batches remain unflushed. Every grouped friction row +A8 references the **last normal row +98 prefix accumulator**, not +94 or the first row. Rust uses pool-relative tokens instead of process pointers. Friction lower bounds sign-flip the value, including **negative zero**. Response vectors +40..+7F and all unassigned words survive.

## Verification and failures

| Check | Result per profile |
|---|---|
| Whole original / repeat / one Rust call |711scenarios,5,052,810words exactly equal |
| Authored controls |528scenarios, including empty/open groups |
| Fresh upstream replay |6,352F152cases/317,600words, original and Rust exact |
| Reused upstream operands |5,840contacts in183additional scenarios; roles/geometry authored |
| Constructed rows |13,040normal +15,972friction;5,324completed groups |
| Categories / aliases |5,048single,7,992two-body normals;1,160aliased contacts |
| Branch activity |6,520special/3,600disabled contacts;546cache changes;9,818batch-head writes;501asymmetric friction rows |
| Assertions / malformed requests |11semantic checks/21ordinary-exit guards pass |
| Regressions |97library tests:20formats+1tools+76world; fmt/all-target warnings-denied Clippy pass |

First native run completed one case then faulted at original B0D5D5 initializing E5AE50: the harness left that private lazy sign-mask page read-only. Added temporary writable/restored protection. Next comparison found626differences, all friction lower-bound negative-zero words; changed independent subtraction from positive zero to sign negation. Three assign-operation lints were corrected. No remaining numerical mismatch; failed/intermediate evidence retained privately.

The adapter owns aligned bodies/contacts/shapes/four row pools/cache; bounds16bodies,8materials,64contacts,1,024cases,32MiBinput; starts with empty counters/cursors/sums. It checks unchanged body/contact bytes and every unchanged context region, compares complete allocations including unused/opaque words, and canonicalizes pointers only in output. Private E52000/E58000/E5A000 bytes/protections restore; executable sections stay RX/byte-identical. No executable entry point/import/TLS execution, helper stub or skipped branch. Rust guards nonfinite consumed inputs, unsupported singular/subnormal normalization and overflowing produced arithmetic; no arbitrary-input equivalence claim.

## Evidence and versions

Final reports: `local/behavior/contact-rows-20261010-{debug,release}-final-04/validation.json`. Intermediate01/02/03retained. Aligned original instructions: `local/behavior/initial-contact-row-aligned-20261010.json`; helper surveys044432/material032931/builders031418. New preparation survey050139is static only.

| Artifact | SHA256 |
|---|---|
| Private analysis PE |aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514 |
| Final native probe |82bc4dacde0a7f37fb446ff21824f96d99f7c98737c04ca7da47a39942bd0ad3 |
| Final request |775937b06729661e39800ac43a0ef3f3cc5a4a8deaa7f79f24b81dcdeff017ec |
| Debug Rust |cb37ba242d316c3d5f37d891d8f15196670790c3750ef5d7cef36e12210c0aa0 |
| Release Rust |3517e3752f831e01dc3c1a63b73c51278c7ca3b19f62fbd3ca36ddfa0d3b9433 |

Reused Rust/Cargo1.99.0, Python3.12.14, MSVC14.44.35207/SDK10.0.26100.0, Ghidra12.1.4/JDK21.0.9, Capstone5.0.6. Native `/fp:strict /arch:SSE2 /W4 /WX /O2 /MT`; Rust debug/release. No installation/desktop input. Authored sources: `tools/probe_contact_rows.c`, `tools/build_contact_rows_probe.cmd`, `tools/validate_contact_rows.py`, `crates/grid-world/src/contact_rows.rs`, `crates/grid-probes/src/bin/grid-contact-rows.rs`. All game assets/captures/dumps/decompilation stay ignored/private.

Next join construction through whole main preparation B77CB0/B79EE0/B78D30 and all four normal/friction iteration spans, then impulse writeback/body publication. Timing, initialization, contact generation and Bevy driving remain open; estimates unchanged. No additional owner recording needed for offline work. Unattended continuation ends10October07:30UTC.
