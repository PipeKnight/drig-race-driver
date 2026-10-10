# Joined optional impact correction — F155

10 October 2026. Whole original **B7B5E0 with impact-contact pools only**, repeat and one independent Rust call match **708 scenarios /14,452 constraint rows /2,451,234 output words per debug and release profile**. Complete original bias, normalization, iteration, memory clearing and pose/publication dependencies execute unchanged. The other two multirow/joint-style pools are empty. This is a supplied-state correction pass, not a whole physical frame or drivable engine.

## Manager contract and joined path

Aligned instructions establish thiscall with one unused stack argument, `ret4` atB7B847; the zero-argument pseudocode signature was incomplete. The probe supplies zero without skipping the argument cleanup.

| Manager offset | Supported role |
|---|---|
| 80/8C/90 | Body correction workspace /192-byte rows /body pointer array |
| 98/AC | Single-body impact pool/count |
| A0/B4 | Two-body impact pool/count |
| 9C/B0 and A4/B8 | Other pools/counts, zero in this comparison |
| A8 | Body count |
| BC/C4 | Normal-row end /two-body row prefix end |
| C0/C8 | Later span ends, overwritten toBC on the nonempty-pool branch |
| E0/E4/E8 | Correction softness /bias scale /accumulator threshold |
| F0/F4 | Normalization relaxation /iteration count |

With all contact pools empty, the original returns immediately, preserving rows, workspace, span ends and body/shape bytes. On the supported nonempty path, one row exists per impact contact and BC is positive. It first sets C0/C8 toBC, excluding tangent spans. Every normal row receives positive zero at80/90 and E0 at84. Two-body contacts refresh first through originalB0CE10, then single-body throughB0CEC0, retaining their ordered cursor behavior and active flag. Disabled contacts zero their relevant response vectors while retaining bounds; enabled nonnegative bias, including signed/equal zero, activates the pass.

If none activate, the reset/refreshed rows and span changes remain, but the workspace and complete body/shape buffers stay untouched. If any activate, originalB7A810 normalizes all rows; B791B0 clears and solves the body workspace through originalBF3B70; B13B00 and all F154 dependencies publish every supplied body, including bodies with zero correction. Zero iterations still normalize rows, clear the workspace and publish poses. These vectors are position/angular corrections in this caller, not a complete velocity update.

`grid_world::impact_correction::calculate` joins [contact bias](CONTACT_BIAS.md), [normal response](PHYSICAL_RESPONSE.md) and [complete pose publication](PHYSICAL_POSE.md). It returns updated rows/workspace/span ends, the row-reset/activity flags and optional pose results: `None` records actual publication bypass. It preserves inputs on error. Its pure prevalidation checks layouts/poses even on bypass; this is a supported interface precondition, not a claim that the original reads bypassed fields. Input pose correction vectors must be zero; the solver supplies the actual corrections.

Typed bounds:256rows,64bodies,32iterations, matching pool/row counts, finite relevant operands/results, valid batch/body layout and the existing64radian pose bound. The IPC1 private wire further limits16bodies and8shapes/body,1–1,024cases/32MiB. It preserves opaque192-byte rows, full0x424-byte bodies and0x220-byte shapes; owned pointers are normalized back to zero. Outputs include both modified span ends, all rows/workspace and complete body/shape buffers. Existing row construction remains external.

## Whole-original comparison

The native harness maps the pinned private PE, executes the complete manager and original dependencies without stubs, imports, entry-point execution, instruction skipping or executable writes. Executable sections match the post-relocation snapshot. Original lazy initialization uses four temporary private writable pages, restored with their protections afterward. The file-backed contact upper limit remains0x7F7FFFFF and original memset dispatch remains0. Input-based SSE activity calculation is only a harness output-validation guard; it does not replace the original branch or observe a live active flag.

Thirteen analytic plus512 authored controls cover empty pools/zero bodies, disabled/negative/equal/signed-zero bias, zero iterations, one/two-body motion, opposite corrections, aliased publication order, shared batches, mixed pools, initial nonzero workspace, arbitrary spans,1–256rows/1–16bodies/0–32iterations, negative/zero relaxation and scale, and singular normalization that must remain unused on bypass.

Each profile also reruns original F152 correction helpers and Rust against their pinned6,352-case request, matching317,600words. The5,840 downstream accumulator/disabled words are retained unchanged and grouped into183 manager scenarios. Rows, body roles, response coefficients, group/shape roles, timing, relaxation and small bias scale0.0001 are authored. Alternating threshold0/1 deliberately yields92active position-changing handoffs and91bypasses; this is not captured live parameter initialization. No handoff accumulator was excluded by the diagnostic magnitude bound.

Nineteen semantic checks pass, including complete inactive body/shape preservation, row reset before bias, original span behavior, no division on singular inactive rows, zero-iteration activation, opposite/aliased body effects and publication of unused bodies. Activity:421active/287inactive scenarios,62empty pools,8,754two-body/5,698single-body rows,2,367body/3,324shape publications,176position-changing scenarios and53zero-iteration active scenarios.

Twenty-six malformed guards reject ordinarily (exit below256/no fault marker): framing/case limits, split/count/iteration/shape limits, booleans/pointers, nonfinite parameters/accumulator/workspace/response, invalid batch/body, reversed bounds, singular normalization/denominator, overflowing bias and over-bound angular correction. Rejected Rust outputs are absent. The native angular bound is checked after isolated execution; it is our diagnostic domain, not an original cap.

## Evidence and failures

- Implementation: `crates/grid-world/src/impact_correction.rs`, `crates/grid-tools/src/bin/grid-impact-correction.rs`; probe/build/validator: `tools/probe_impact_correction.c`, `tools/build_impact_correction_probe.cmd`, `tools/validate_impact_correction.py`.
- Analysis SHA256 `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.
- Final native SHA256 `b1248de7fa8fb655d3598609da110e805099c88ba18f7762dc9ae36175885f57`.
- Request SHA256 `4341ffa6b5042c0c4def8f95a95091ca1c99a4c7aed546f6139379ff7c8e1be1`.
- Rust debug SHA256 `8015126a39e987a73cf7e439815bc8c69eafae9e84690c3e3450b5ddf3afe81b`; release `722b354399aaa1b075d7b92543c8489fad46d683f624db24beee0afe917310d7`.
- Reports: `local/behavior/impact-correction-20261010-debug-final-04/validation.json`, `impact-correction-20261010-release-final-04/validation.json`; all upstream identities pinned there.
- Aligned manager evidence: `optional-correction-aligned-20261010.json`; private031352manager survey. Other-pool static survey: `20261010T0429478985165Z/`, not execution verification.

The initial01 numerical comparison passed but its reused handoffs were all inactive. Adding an explicit handoff-activity requirement made intermediate02 fail coverage despite exact words and passing guards. Final03 alternates authored thresholds and verifies active position changes; final04 additionally checks overflowing/over-bound results and shape finiteness with a strengthened harness. Earlier evidence retained. A transient dynamic-array-length typo in the new Rust source was corrected before building. No native fault, numerical mismatch, build/test/lint failure occurred in this joined comparison.

Python3.12.14, Rust/Cargo1.99.0, MSVC14.44.35207/SDK10.0.26100.0 reused; no installs.92library tests (20formats+1tools+71world), fmt, all-target warnings-denied Clippy and validatorAST pass. Steam130resources+original executable unchanged at2026-10-10T04:40:03.458823UTC: `local/scene/preservation-20261010-impact-correction.json`. No desktop input or owner action required.

## Next actions and limits

Prioritize initial physical normal-row construction and main solver integration, while retaining this verified optional impact-only pass. AEF930/AEFBB0 multirow/joint-style helpers are statically recovered, with disabled/prefix/axis/special branches, but not implemented or executed here. Original nonempty pools with zero normal rows are outside the one-row-per-impact-contact contract. Initial constraints, friction/tangent rows, live ownership/cadence/enabling parameters, vehicle initialization and a drivable Bevy loop remain open. Completion estimates stay unchanged.
