# Selected source-car prelude (F208)

The independently authored Standard constructor can now receive a selected
source-owner prelude before construction: complete decoded properties, unit mass
scale, identity poses, wheel count, the owned context backlink, activation flags
and selected outer-factory scalars. This closes the missing copied-property and
mass-scale boundary of the earlier zero-cold diagnostic. It is **not the complete
outer factory**, suspension-context setup, player-preference application, spawn
or a drivable car.

`standard_car::initialize_with_source_prelude` preserves the original cold API.
The private CLI selects it with a trailing `--source-prelude`. The native probe
accepts SC03 alongside SC02, supplies the same explicit stores, and executes the
unchanged whole A0C9F0. No original instructions are replaced. The native owned
self link is checked before canonicalization; all numeric/state bytes survive.
The existing two undefined stack-padding bytes remain the only byte exclusions.

## Verification

In each debug/release profile, original/repeat/Rust match:

- 43 installed CSV/NED associations plus 26 authored controls at five RNG seeds:
  **345 cases / 46,632,270 observable bytes** with the source prelude;
- the same **345 cold cases** as a regression of SC02;
- **178 complete HDM settings / 64,080 bytes**, and **588 joined complete owner
  resets / 79,478,784 bytes**, including both source HDMs, three RNG seeds,
  active/inactive wheel views and authored HDM controls.

Constructor audits check current executable hashes, source/upstream/input/all
three output files, selected gear 196C=1, ratio slot 18F4=0, source properties,
mass scale and active flags. Each constructor audit passes 27 distinct rejection
checks. The joined audit passes 32 checks and replays all 690 cold cases through
the current Rust builds. Twelve constructor and six reset RNG advances remain
exact. All 163 library tests, formatting, warnings-denied all-target Clippy and
the four modified Python ASTs pass.

Steam executable and all 130 baseline resources remain unchanged at
2026-10-11T23:09:16.949643 UTC. No installation, desktop input or game-settings
changes were needed. Rust/cargo 1.99.0, Python 3.12.14, MSVC 14.44.35207 / SDK
10.0.26100.0 and installed Ghidra 12.1.4 were reused.

Current executable SHA-256:

| Artifact | SHA-256 |
| --- | --- |
| Native constructor | df38c9df312e6402dbec153c49f54cc6e6d1e20f8aac77a8006ac83c44458c9a |
| Rust debug | 9d2b3b9edb149bc03bf00f9c6bc619ee1e46844e158630f43c9ae3b52da8ea17 |
| Rust release | 53778d14929e0702f596cec86608507b0f49a6091e14ca0b2d7c6df010add225 |

Earlier reports with older executable hashes remain historical evidence. Fresh
SC02 regression reports establish the current native/Rust builds' behavior.

## Private evidence

- `local/behavior/source-prelude-{debug,release}-20261011-final-{seed}/validation.json`
- `local/behavior/source-prelude-audit-20261011-final-01/audit.json`
- `local/behavior/prelude-cold-regression-{debug,release}-20261011-final-{seed}/validation.json`
- `local/behavior/prelude-cold-regression-audit-20261011-final-01/audit.json`
- `local/behavior/source-prelude-handling-{debug,release}-20261011-final-01/validation.json`
- `local/behavior/source-prelude-handling-audit-20261011-final-01/audit.json`
- `local/scene/preservation-20261011-source-prelude.json`
- `local/behavior/source-prelude-libtests-20261011.txt`,
  `source-prelude-clippy-20261011.txt`, and build logs in the same directory.

## Runtime trial and next work

A separate private trial composes the source Lacetti prelude, source Standard
HDM/reset and source physical factory. Each initialization stage matches native
and Rust. Explicit engine readiness/owned-link normalization and the owner's
assisted automatic preferences are supplied for the trial; they are not claimed
as original resource/preference-factory equivalence.

The first runtime attempt safely rejects recovery scale 3D0=0. Aligned outer
A0E880 explicitly initializes it to 1. Supplying that observed store gets Rust
to contact-normal alignment, which rejects the still-zero suspension axes at its
existing bounded loop. The native diagnostic independently rejects before the
manager call (error 67); it did not execute an unbounded alignment loop or fault.
Both failures are retained in
`local/behavior/source-runtime-20261011-draft-01/{executions,scale-executions}.json`.
This is missing initialization, not evidence for widening the alignment bound.

Static follow-up identifies the original outer call to 9C10B0 on the suspension
context before A0C9F0. It resets context history and constructs normalized axes
from copied source caster/camber and wheel placement. Detailed aligned evidence
is `local/behavior/source-outer-prelude-aligned-20261011.json`; fresh surveys are
`20261011T2310406251903Z` and `20261011T2311437603261Z`. Whole source context setup,
remaining outer defaults, retained world attachment and Bevy driving are next.

Harmless failed guessed diagnostic paths and Windows wildcard reads were
corrected from inventory. One scratch retry guessed a mechanics filename and
failed to create a nonexistent subdirectory before execution; the existing
`mechanics-full.private.bin` was then used. No original files were modified.
