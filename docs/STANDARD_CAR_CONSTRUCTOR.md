# Complete cold Standard car constructor (F203)

Verified 12 October 2026 local time. `grid_tools::standard_car::initialize` reconstructs the complete owned 0x21000-byte cold car produced by original A0C9F0. This closes the constructor boundary after the [complete Standard properties decoder](STANDARD_PROPERTIES.md), before the resource factory and physical-world attachment. It is not yet a drivable car.

## Inputs and numerical proof

The original cdecl function receives a zeroed owned car, the complete 0x65C-byte decoded properties, four mutable wheel points, four radii and four widths. The diagnostic independently prepares points from the purchased same-stem NED: mirror X, subtract the CSV height bias from Y, retain Z and set W=1. Gravity and RNG state are explicit inputs. The CLI accepts CSV, NED, hexadecimal RNG seed and an ignored-local output path.

43 of the 44 supported installed Standard CSV sources have verified same-stem geometry. The remaining source is explicitly skipped rather than assigned speculative geometry. Another 26 authored CSV variations exercise mass, inertia, geometry fractions, RPM endpoints, Front/Rear/Four drive, four differential modes, springs, shift scaling, height bias, steering and engine flags. Each of the 69 cases runs with RNG seeds 200679, 0, 1, 0x80000000 and 0xffffffff.

Original, repeat and Rust debug/release match **345 cases and 46,632,270 observable bytes per profile**. Every output is 135,168 bytes; two undefined stack-padding bytes are excluded. Final RNG state agrees after all twelve advances in every case. Current-binary, source, NED, property-report, request and all output hashes are audited. Thirteen native framing guards and seven strict Rust source/CLI guards per profile reject without creating outputs (27 total); these are distinct contracts, not paired malformed-input equivalence.

The Rust constructor composes already verified suspension preloads, steering geometry, two gear initializations, impulse and point-velocity arithmetic with the full constructor stores, wheel resets, engine curves, setup copy, auxiliary owner defaults and control reset. The final ratio-table slot zero (`18F4`) comes from the decoded setup table and is zero, superseding the early constructor alias of one. Selected gear (`196C`) remains integer **1**; ratio-slot zero must not be interpreted as neutral selection. Intermediate and final states must not be confused.

## Original execution and explicit exclusions

The native SC02 request is 1,732 bytes: tag, RNG seed, properties, points, radii and widths. Original A0C9F0 and its helpers execute without replacement code, import initialization or TLS emulation. Full executable-section comparisons verify code remains unchanged. Eight private data pages (E58000, E5A000, E5B000, E5E000, D16000, E85000, E8E000 and E4F000) are individually snapshotted, made writable and restored, including protections. Owned-buffer sentinels are checked; output creation follows validation and restoration.

Only verified body-owned addresses become relative tokens: driven-wheel references, steering pair, C7F8 and the four wheel owners. C784 must equal image-relative 9A0C90 and becomes an opaque callback identity, never executable Rust. Original 990610 writes flags at settings48..5D and resumes at60, leaving5E/5F uninitialized on the stack. The constructor copies those bytes to202E/202F. Exactly these two padding bytes are normalized to zero at the diagnostic boundary; no numeric or control state is excluded.

The source cold reset retains negative quiet NaN bits FFC00000 at bodyD19C: its initially zero ramp has zero width. Rust preserves this diagnostic result explicitly. Runtime finite-state guards remain intact. Later HDM configuration and reset must run before this cold result enters the driving loop; this observation does not establish the whole outer factory or spawn setup.

The first stack reset consumes six RNG samples whose values are subsequently overwritten. A later auxiliary reset consumes six more. The wrapping LCG is state*41C64E6D+40584C0F; the sample uses `(state + signed(state)/65535) & 255`. Signed zeros in rotation seeds are preserved. The mapped initial seed happens to be 200679; live race initialization is not inferred from that file-backed value.

## Evidence, checks and corrections

Private evidence:

- `local/behavior/standard-constructor-{debug,release}-20261011-final-{200679,0,1,0x80000000,0xffffffff}/validation.json` (ten reports).
- `local/behavior/standard-constructor-audit-20261011-final-01/audit.json` (current hashes, 27 rejection checks).
- `local/behavior/standard-constructor-aligned-20261011.json`, constructor constants and RNG constants JSONs; Ghidra surveys20261011T2131148140204Z,20261011T2138304326067Z,20261011T2146328484599Z and cached helper surveys.
- `local/scene/preservation-20261011-standard-constructor.json`: all130 traced Steam resources and executable unchanged at2026-10-11T21:58:54.737281UTC.

Native executable SHA256 `714a94b71b6c7cfa849557263c6405b814a4cb8df574f392c972117b204b7a80`; Rust debug `d7babba2b7c3d7843db6d03a73d694440ff03140fdaca09227a4415a63ba96d2`; release `1132c0cc7e1caf1f1ef8fc89d204b972bbea90d7846e9ba17ffc10e3df179ba5`. Reference analysis PE remains `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.

Final fmt, workspace all-target Clippy with warnings denied and161 workspace library tests pass. Reused Rust/Cargo1.99.0, MSVC14.44.35207/SDK10.0.26100.0, Python3.12.14, Capstone5.0.6, Ghidra12.1.4/JDK21.0.9 and Bevy0.19.1; no tools installed or desktop/settings changes.

Failed experiments are retained privately. First native execution faulted at9E614F writing the legitimate RNG globalE4FCC0 on a read-only private page; adding E4F000 to the snapshot/restore set fixed it. Fault reporting was corrected to original-image addresses. Repeats exposed the relocated callback and two undefined stack-padding bytes, handled explicitly above. First Rust comparison differed at26 shared default/auxiliary words per car (1,118 words across43 cars): wheel80 defaults, float-versus-integer minus one, reset seeds and control defaults. Original helpers resolved these, including the cold NaN and the first six overwritten RNG samples. A repeated file-reading draft diff was cancelled and replaced with one read per file. Clippy loop/negative multiplication issues, one atomic patch-anchor failure and guessed filename/wildcard reads were corrected before final validation.

Next: original HDM configuration decoding and source reset, then outer physical initialization/spawn and connection to the [retained complete vehicle/world loop](VEHICLE_WORLD.md). No owner action is required for this offline work.
