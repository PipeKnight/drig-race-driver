# Wheel contacts produced by the complete car manager

F195, 11 October 2026. The selected whole `A02B70` manager now includes active
wheel overtravel production through unchanged `9BAB10 → A72400 → B15E70 →
B0EE80`. Original/repeat/Rust agree across 428 updates per debug/release,
58,305,584 complete owner bytes and 59,329,360 observable bytes each. This
includes 192 carried updates, 96 zero-delta calls and 44 source vehicle tables.

## Ownership and ordering

Read-only private GRID snapshot `wheel-producer-shapes-live-20261011.json`,
15:03:51.047057 UTC, records 12 car wrappers pointing to kind0 shapes and their
exact physical owners, plus 48 tyre-hit wrappers pointing to kind11 static
shapes without a physical owner. The paused snapshot contains no overtravel;
positive production is exercised by authored inputs in the isolated comparison.

`A72400` obtains collision shapes at wrapper+8, not physical bodies. Shape+D8
selects its physical owner and shape+C8 selects material. World-wrapper+4 points
to the world, whose +18 points to the producer. The selected car has flag229 on;
ground flag228 and angular permission216 are on. Both shapes have flag217 off,
MAX limits224/22C and the same material identity. Type13 identity redirection
and other shape metadata combinations remain outside this producer contract.

For each nonterminal wheel, production requires enabled1EE0, a car wrapper,
nonnull other wrapper and distance40 greater than limit50. The local point's
Y is `(baseY - E0) + (distance + limit) * .5`; W becomes one. The original SSE
transform is `(W*row3 + Y*row1) + (X*row0 + Z*row2)`. Direction uses car15F0
before manager basis repair. Penetration is distance minus limit.

The producer appends only below capacity. Constructor-written fields include
physical owner1/0, shape identities, shared material identities, penetration,
MAX friction limit, direction and flags70..73 = 0/0/1/1; C is zero even though
the second physical owner is null. It then subtracts the first body's linked
334 origin from the point. Other preallocated template bytes remain intact.
These flags and null ownership must reach later classification unchanged.

## Independent implementation and verification

`wheel_contacts::Pool` owns zero to four preallocated 80-byte slots, a used count
and bounded source-shape count. Token1 denotes the car shape; source static
shapes start at2. `vehicle_manager::calculate_with_contacts` validates ownership
before the job, produces contacts after the complete job and before basis/history
repair, then returns private output. Errors preserve the complete car, physical
owner and pool. A late basis-failure unit proves rollback after contacts and
earlier activation/mass work; a fourth-wheel overflow unit preserves all slots.

The native TF02 adapter binds actual owned shapes, wrappers, world and producer,
uses the original manager dispatch/completion/wait sequence, and normalizes only
owned pointer identities. It audits every producer/world/wrapper/car-shape byte
outside documented count/slot writes, original tables/events/backlinks, full IAT
page and eight restored global pages. Original executable code remains RX and
unchanged; no original helper is replaced. Source queries retain actual source
shape identities. Unused and previously used slots are compared in full.

Each profile generates 155 contacts in 66 intervals, exercises 76 capacity skips
and retains 79 existing records. The 328 installed-producer intervals also cover
disabled/absent gates. Twenty-four paired semantic/framing outcomes pass per
profile. Carried pools reset their used count externally every third interval;
this models consumption, not a persistent-world allocator. Fresh F194 regression
matches all 428 intervals and 36 guard outcomes per profile.

All 158 library tests, formatting and warnings-denied all-target Clippy pass.
A 16-report audit checks 876 conditions, current executable hashes, unchanged
analysis image and three Python ASTs. Private reports are
`local/behavior/wheel-car-manager-{debug,release}-20261011-final-01/validation.json`,
`car-manager-{debug,release}-20261011-f195-regression-01/validation.json`, their
source/guard reports and `wheel-car-manager-final-audit-20261011.json`.

Native SHA256: `6f57cba4b56c7f6ae52b7e6944bd786c0ca23023941dcf3d2beeed692650d601`;
Rust debug: `d50379439a355137641c70be6d08f0a4f0dcb72f9e01797cf9597d612d568508`;
release: `a421d425fac2b72d8243c235f0b20b896f6d6f6d30c24dde4a32cadfaf42e9b5`.

## Failures, provenance and next work

Draft01 safely rejected formerly unused random world-wrapper DD8; the pointer-free
input now explicitly sets it to zero. Draft02 matched all 428 numerical updates
but exposed a guard mismatch: Rust accepted an invalid entry shape token after
a positive query overwrote it. Entry ownership is now validated before the job;
both final profiles and legacy regressions pass. C /WX caught implicit alignment
padding before execution, fixed with explicit wrapper storage. No native fault
or numerical mismatch occurred. A guessed nonexistent grouped diagnostic source
path failed harmlessly while locating the next integration point.

Static evidence: `wheel-producer-aligned-20261011.json`, private surveys
`20261011T1500339769780Z/b0ee80.c.txt`, `20261011T1445568125338Z/b15e70.c.txt`
and `20261011T1437461633906Z/a72400.c.txt`. Python3.12.14, Rust/Cargo1.99.0,
MSVC14.44.35207/SDK10.0.26100.0 strict x86/SSE2, Ghidra12.1.4/JDK21.0.9 and
Capstone5.0.6 reused; no installation. Original130 Steam resources and executable
remain unchanged at15:20:14.465416UTC, recorded in
`local/scene/preservation-20261011-wheel-contact-production.json`. No desktop
input, settings or original installation changes.

Next preserve generated C=0/null-second ownership, shape identities and special73
through original grouping/classification and complete physical solving. Shape
allocation, cold-pool initialization, material construction, persistent ownership,
full asynchronous world scheduling and a drivable Bevy lap remain open. No owner
action is required for the next offline integration step.
