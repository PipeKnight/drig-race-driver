# Donington boundary contacts in the driving prototype

F240, 12 October 2026. The optional **Play GRID Body Contacts.cmd** mode now
loads the separate source `boundarylines.cqtc` as well as the packed track.
The purchased Lacetti mesh generates fresh boundary contacts before physical
solving. The ordinary **Play GRID Prototype.cmd** remains the wheel-only mode.

The previously failing 600-tick full-throttle, steering=0.25 script now stays
beside a Donington barrier instead of falling through the unsupported world.
Both current Rust profiles reproduce this paired result: the first physical
difference appears with boundary contact at tick 165, and the enabled run
generates 176 body records. At tick 600, enabled Y is -14.807243 m; the matching
packed-only control has fallen to -147.81863 m. All 600 enabled ticks complete.
The current release app renders the same final pose, with the Lacetti visibly
alongside the barrier, and records no simulation failure. This is a specific
sliding-contact check, not a completed automated lap or general crash validation.

## Original comparison and representation

The original CQTC path has its own positive-count constructor, seven-byte
triangle decoder, 24-bit tree nodes and base-relative reference lists; see
[source decoding](CQTC_BOUNDARY_GEOMETRY.md). Observed loader, dispatch,
child/leaf and collector calls were inspected with installed Ghidra. An owned
native probe executes the original cold kind-10 mesh dispatch without replacing
any original function or touching a running game.

All four Donington routes, 32 sampled source triangles per route, four height
placements and both sheet policies are covered, with additional contact/cache/
candidate capacity controls: 1,088 cases. Heights include points within the
roughly 100 m tall source boundary faces; the +4 m placement is **not** a
separation control. Original/repeat complete outputs match exactly:
53,814,784 bytes per native execution set. Source-only Rust debug and release
match the complete candidate cache/stamps/order, final query direction, contact
count and all sixteen retained/unused manifold slots: 45,820,160 bytes per
profile, 4,212 contacts, 1,064 positive cases. Thirty admission/output guards
pass, including malformed/cyclic source trees and existing-output preservation.
An independent current-hash/source/request/output audit verifies 5,440 primary
files, all complete original repeats and both Rust projections.

`cqtc_geometry::Geometry` losslessly maps the selected small CQTC trees to the
shared collector representation. It admits at most 32,768 nodes, leaf offsets
below 32,767, at most 256 triangles per list, and at most 65,535 decoded vertices.
Cycles, excessive depth and traversal work reject before querying. Existing
packed APIs retain their 1,024-vertex admission bound. Wider layouts and warm
moving caches remain outside this implementation.

Freshly rebuilt original packed-track dispatch still matches all 2,642 previous
captures. Both current Rust profiles match all 111,448,456 selected bytes,
5,039 contacts and 1,098 positive cases per profile. This checks that sharing
the collector/manifold code and diagnostic admission did not change the
previous packed behavior.

## Vehicle integration and limits

Preparation reads and validates the source boundary before reserving state.
It registers one extra static shape after the existing 245 track shapes; all
wheel shape identities stay fixed. Boundary physical token 247 is separate
from wheel ground queries. Reset retains the extra registration and 20-slot
contact reservation. Duplicate preparation rejects.

Scheduling is explicitly authored: boundary first, then packed chunks in
archive order. Source four-byte material keys select sheet policy, while solver
material IDs still use the prototype's explicit shared physical material.
Each source pair is cold and returns at most sixteen records. This does not
establish original broadphase order, complete material registration, contact
callbacks, warm cache behavior, full factory equivalence or damage events.

A controlled stationary source-boundary placement gives seven fresh contacts
over twenty complete vehicle ticks and changes the body relative to an empty
provider on every tick. The clear 240-tick starting straight matches the previous
wheel-only body exactly. Five opted-in source driving tests pass, including
handbrake, reverse, reset and rollback. Three opted-in Bevy keyboard/presentation/
failure recovery tests pass. All 185 regular library tests, formatting,
warnings-denied all-target Clippy and Python AST checks pass.

Heavy impacts can still pause at `unsupported handling collision damage`.
That original above-threshold branch remains guarded; its threshold was not
raised or disabled. **R** recovers. Follow-up work is the actual damage/localization
branch and broader source barrier placements, followed by a reproducible lap.
No owner input currently blocks that work.

## Versions and private evidence

Reused tools: Rust/Cargo 1.99.0, Bevy 0.19.1, Python 3.12.14, Ghidra 12.1.4 /
JDK 21.0.9, MSVC 14.44.35207 / SDK 10.0.26100.0 with strict SSE2 arithmetic,
RTX 4070 Vulkan driver 616.92. No tools were installed and no desktop input or
Steam settings/files were changed. Assets, tables and binary outputs stay in
ignored `local/`.

- `local/behavior/cqtc-contacts-20261012-final-01/validation.json`, SHA-256
  `d7557fd9cd592c8fec1ee86411486d336759bd73be963f636ef742607e0300ee`.
- `local/behavior/cqtc-contacts-audit-20261012-final-02.json` and
  `cqtc-packed-{debug,release}-20261012-final-01/validation.json`,
  `cqtc-packed-audit-20261012-final-01.json`.
- `local/behavior/cqtc-source-driving-20261012-final-02.txt`, SHA-256
  `399425e42c67c0498438115d1c0bf48772c33f7fbd1e726b57a30cd41ed15803`;
  corresponding debug-final-02 log verifies the same 600-tick comparison.
- `local/behavior/cqtc-app-tests-20261012-final-03.txt`,
  `cqtc-libraries-20261012-final-01.txt`, `cqtc-clippy-20261012-final-03.txt`,
  native regression/build logs and final application build log.
- `local/playtest/donington-cqtc-turn-20261012-final-02.{log,png,json}`:
  132 telemetry samples, 100 body-hit samples, all 176 body contacts and zero
  failure samples. Sampling captures every body-hit tick in this script.
- Current release application SHA-256:
  `fd8ce508d0c42436faf79b568a599a5fa83dd2e9857280108d89e8c1221f084e`.
- Private original collector surveys: `20261012T2023174594451Z`,
  `20261012T2024243657885Z`, `20261012T2025462243096Z`,
  `20261012T2028072151698Z` under `local/behavior/`.

Failures retained: initial all-target Clippy requested a collapsed conditional;
corrected without suppression. One app-test invocation lacked the two required
source environment variables and all three tests rejected; explicit environment
setup then passes. A PowerShell hash display used an invalid positional argument
and was replaced by independent Python hashes. Guessed source/validator paths
were corrected using inventory. Native leaf scratch preflight was bounded to
256 and Rust cycle admission made independent of query culling before final
comparison. The decoder diagnostic also now canonicalizes its private output
parent. These failures are not counted as successful validation.

The launcher was reviewed, while its equivalent executable, keyboard behavior
and rendered output were tested. It has not been double-click-tested by the
agent. The private sampling preparation limitation from F238 still applies;
no proprietary values are embedded in public source.

Final preservation: `local/scene/preservation-20261012-cqtc-contacts.json`
confirms all 130 baseline resources and the original executable unchanged at
20:46:15.383296 UTC. All four boundary source hashes are also rechecked in the
contact audit. The strengthened source-decoder rerun is
`local/behavior/cqtc-decode-20261012-final-03/validation.json`; all sixteen cases
and both current decoder builds pass.
