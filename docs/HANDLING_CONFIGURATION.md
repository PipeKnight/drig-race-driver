# Complete cold handling configuration and source reset (F204)

12 October 2026 local time. Rust now decodes the complete 360-byte Standard handling settings and installs them in the [complete cold car](STANDARD_CAR_CONSTRUCTOR.md), then executes the selected reset order. Original/repeat/Rust match every car byte and the final RNG state. The shipped settings eliminate the constructor's intermediate NaN. Physical attachment, resource factory, spawn and a drivable Bevy lap remain pending.

## Boundary and implementation

Original 9C5580 fills settings at mutable owner218..37F. Its actual thiscall ABI has one stack argument and ret4; the C probe represents ECX with fastcall and a dummy EDX argument. Original binary-document vtable D3D508, root selection A680E0, nested selection A67EA0 and property accessors execute unchanged on bounded owned decoded XML tables. Numeric inputs use exactly one explicit host strtod adapter at BF3B60. Defaults execute with no replacements. This verifies the settings reader and its math, not the original HDM text tokenizer, CRT numeric parsing, resource acquisition or outer9C6360 factory.

The Rust reader exposes Configuration with90 words and preserves the previous steering/visual API. Fifty-one direct fields and thirteen three-field strength records cover General, driveshaft, turbo, suspension, wheel, six brake combinations, spoilers, radiator, engine, exhaust and gearbox. Six source defaults are preserved. Each strength's minor/major probabilities are rescaled together only when their stored-f32 sum exceeds one. Unknown nodes are ignored within the bounded subset; known duplicates, ambiguous attributes, unsupported entities/declarations, malformed trees and nonfinite recognized numbers are rejected. Limits remain64KiB ASCII,128 elements,16 levels,64 attributes/element.

Car.configure_handling installs this complete cold configuration and calls vehicle_handling::reset before committing either bytes or RNG. Whole selected9E6070 clears the24 level/seed words, advances the LCG six times, produces cap/rotation seeds, resets status/history flags and calls complete selected9C5B90 effects. Secondary component and query list are absent. An owned optional wrapper explicitly selects absent or four active wheel views; this identity is normalized to zero at the pointer-free boundary. General live ownership is not inferred.

Constructor12 advances plus source reset6 advances give18 verified RNG advances. Signed-zero seeds and all unrelated car bytes are retained. Failure is transactional: invalid or nonfinite late effects leave the car and RNG unchanged. Runtime finite-state guards were not relaxed to admit the cold NaN.

## Numerical coverage

Per debug and release build:

-178 complete settings cases /64,080 bytes: both archive-backed HDMs,26 unchanged-code missing/default-node controls and150 numeric-adapted authored cases. Controls cover each direct/strength field, nested-parent selection, signed zeros, underflow, decimal rounding, negative values and probability sums below/equal/above one.
-588 complete joined car cases /79,478,784 bytes:43 verified CSV/NED pairs × both shipped HDMs × three initial seeds (200679,0,FFFFFFFF) × absent/active wheel views, plus72 Lacetti cases varying twelve HDM settings. Each starts from a verified native cold constructor output; Rust independently constructs from source. Original, repeat and Rust complete135,168-byte outputs and RNG states match.
-396 unchanged full handling producer calls /53,947,872 bytes,42 guards,10 framing checks and6 semantic checks regress exactly after sharing the effect implementation.

The final audit verifies both current executable hashes, upstream reports, archive/layout/source/NED/cold/input/output hashes and all retained output files. It also replays all690 F203 constructor cases (345/profile) through current Rust, comparing unchanged historical native outputs. Thirty-two distinct native framing and Rust source/reset guards reject without creating outputs; malformed original text/CRT equivalence is not claimed.

163 workspace library tests, fmt, all-target warnings-denied Clippy and both Python AST checks pass. Added tests cover nested strength identity/normalization and failed-reset rollback. All130 tracked Steam resources and the executable remain unchanged at2026-10-11T22:18:58.057855UTC. No installations, UI input or settings changes.

## Evidence and versions

Private final reports:

- `local/behavior/handling-configuration-{debug,release}-20261011-final-01/validation.json`.
- `local/behavior/handling-configuration-audit-20261011-final-01/audit.json`.
- `local/behavior/handling-configuration-regression-{debug,release}-20261011-final-01/validation.json`.
- `local/scene/preservation-20261011-handling-configuration.json`.

Source archive raceload.jpk SHA256 `58c62779d1915307784da7c095f1bae3832828abecb771bb4659a37999afcbc8`; raw entries are checked against the prior source survey and copied privately. Native probe SHA256 `7110bd2ddd73c154bc07186465bb1cf4fc648d2d0720d16805a3bf26cc8146ff`; Rust settings debug `456063643feb93f0f42cbaaba4862775f3fb5d0a16c3d2c54048d3e200e8d1d8`, release `af9cb4587082408bed2bce1f71e41042e49dfbb81f6914729d9d998a219c9d8b`; joined car debug `b61de5de1b99a59931e283b7eb88e201e54a41f7c63ac3496f86dc7fe0a39a6f`, release `b5d428b4cbeb28201bb628a6bd2a9388d4e7bed6af64b10ccbac12e8ee036152`.

Private layout facts SHA256 `224c7041edb9158fdcc229c602e34f6a7a8bdf131250dcf09a928c3414a2e560`. Aligned handling-loader/document/binary-document evidence and fresh Ghidra20261011T2201097856106Z establish ABI, field stores and accessors. Factory surveys20261011T2216395011890Z/20261011T2217273724180Z show9B8D00 creates a physical wrapper throughA742E0; largerA01960 follows. These are static observations, not verified factory execution.

Reused Rust/Cargo1.99.0, Bevy0.19.1, MSVC14.44.35207/SDK10.0.26100.0, Python3.12.14, Capstone5.0.6, Ghidra12.1.4/JDK21.0.9. Native /W4 /WX /O2 /MT /fp:strict /arch:SSE2; no new tools.

## Corrections and next work

First C build used unsupported C __thiscall function-pointer syntax; corrected to the actual fastcall bridge. The failed command then attempted the missing executable harmlessly. The first Rust reset rejected the shipped brake maximum10,016,380 because an arbitrary1e6 settings bound was too small. Reset now requires finite settings and validates its produced effects. The ordinary runtime producer still has the older parameter bound; target its brake thresholds before joining source-configured cars to runtime. One initial comparison accidentally paired a350 native car with a Lacetti Rust invocation and was stopped by the earlier guard before comparison; the corrected pair matches. No native faults or numeric mismatches occurred in this work.

Formatting changed two patch anchors, which failed atomically and were reapplied; Clippy requested a collapsed condition and moving tests after implementation. Guessed effects/synchronization filenames were corrected from inventory. Draft evidence is retained.

Next: complete original physical-body initialization with source mass/inertia, then shape/resource and spawn setup, then source car admission into the retained vehicle/world loop. No owner action is required.
