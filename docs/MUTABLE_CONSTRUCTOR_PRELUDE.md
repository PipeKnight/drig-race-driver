# Outer mutable constructor before Standard construction

F246 follow-up: [source wheel regions and collision RNG](SOURCE_WHEEL_DAMAGE_STARTUP.md) resolves the selected caller-array/alias uncertainty below and enables the owned collision producer after separate original/reset/driving verification. F245 results and limits here describe the earlier milestone.

F245 recovers the missing mutable-constructor prelude in source startup. Whole original `9F3D30` followed by whole `A0C9F0` matches independently constructed Rust state in 690 comparisons. The runtime now uses the recovered prelude; collision RNG opt-in remains disabled until the four wheel damage regions and caller argument mapping are recovered.

## Recovered behavior

The original outer `A0E880` calls `9F3D30` on car `CDF0` before Standard construction. The later Standard constructor also calls this helper on temporary storage, then saves/restores the incoming mutable block. Starting the incoming block from zero therefore omitted persistent defaults.

The recovered prelude sets all nine region centers' homogeneous W to one, retains disabled regions with zero extents, initializes gains at `430`, `434`, `438` and `43C` to one and enables healthy notification flag `442`. Its reset consumes six global RNG draws. Standard construction consumes twelve more, so the selected chain has eighteen advances before the later source HDM reset. Rust reproduces the persistent defaults and the exact global sequence without treating intermediate cold NaNs as finite runtime state.

`initialize_with_mutable_prelude` preserves the existing cold and source-prelude APIs. The diagnostic selects the new path with `--mutable-prelude` and SC04. Production startup uses it, then performs suspension setup, five NED-region stores and source handling configuration/reset. The independent damage RNG introduced in F244 remains `None` in production; the reached-impact guard remains active.

## Whole-chain comparison and audit

For each debug/release profile, 43 installed same-stem CSV/NED pairs and 26 authored controls run at five RNG seeds: **345 cases and 46,632,270 observable bytes per profile**. Complete owned outputs after pointer and two undefined-padding canonicalizations match original/repeat/Rust byte for byte. All final RNG states agree after eighteen advances. The unverified variant-to-base association is explicitly skipped.

SC04 executes both original functions with no code replacements. Eight data pages are restored with protections. The new mode additionally compares the entire mapped image after restoration. Existing owned-car sentinels and original address/backlink checks remain in place. A separate native executable preserves the previous SC02/SC03 executable for historical evidence.

The audit verifies current binary/source/report/input/output hashes, reconstructs every SC04 request from its CSV/NED/properties, rereads all three outputs and checks the recovered defaults, cold NaN and RNG sequence. All **30 guards** pass: distinct native-framing and Rust-source guards plus native/debug/release existing-output preservation. These contracts do not claim identical malformed original-text behavior.

All five purchased-source release driving tests pass with the prelude installed, including handbrake/reverse, reset/rollback, clear straight, shallow geometry and boundaries. The paired 600-tick boundary test still first differs at tick165, produces176 records and ends at Y=-14.807243 versus packed-only Y=-147.81863. All three Bevy input/presentation/recovery tests, 188 regular library tests, formatting, warnings-denied all-target Clippy, two Python syntax checks and whitespace checks pass. The release app is rebuilt, SHA-256 `9c5b0b7709b0dd3fbeab0019bd882c5bc90dd32cc14922e1ff103bc92bf4b166`; no new interactive lap is claimed.

## Caller argument correction still under research

The old constructor diagnostic called its fifth argument `widths` and supplied named NED widths. Static outer instructions/decompilation instead associate this argument with car `1530`: geometric radii multiplied by the stored f32 constant at `CB0F24`, observed as `0.800000011920929`. A sixth caller argument points at `1540`; the examined Standard body does not consume it, but the outer later uses it for wheel damage-box X extents. The NED parser's separate fields and the exact caller stack still need aligned verification before changing that mapping.

Thus the new proof establishes the whole constructor chain for explicit supplied arrays, including the historical diagnostic's selected array. It does not establish the original outer factory's complete geometry argument selection. Production retains that selection for now. This uncertainty is recorded rather than silently redefining the passing constructor proof. Four wheel damage boxes remain disabled; gains and center-W defaults are now recovered.

## Failures, evidence and next actions

Adding the full-image check initially produced MSVC C4701 because an early error path could reach an uninitialized mode variable. Initializing the mode before every possible goto fixes the warning; the strengthened native build and complete sweep pass. No numerical tolerance or finite-state guard was changed. Harmless Windows wildcard and guessed-file reads were corrected with inventory.

- `local/behavior/mutable-standard-{debug,release}-20261013-final-{200679,0,1,0x80000000,0xffffffff}/validation.json` contains ten complete reports.
- `local/behavior/mutable-standard-audit-20261013-final-01/audit.json`, SHA-256 `918534ff4228cd4132f4bb943e76d414cb0761dc87e37d549fb314d71d470990`.
- Native SHA-256 `02ed4e95bde14b9bb8a326c4c0de06f40408bf1f4f8cc018367f0c2b93dcbb7a`; Rust debug/release `714bb26c88f8ec04f2252259a3b1f1cf558f4ef11138b41c83036a4cbfc16487` / `ff1cc42f869260f96010ac176d72e774114e6434002388a14628b9525eaf064f`.
- First source draft `mutable-standard-debug-20261013-draft-01`; driving log `mutable-standard-driving-20261013-draft-01.txt`, SHA-256 `89f321ebf9df93c11caeb1c6917f88aea8ad793a4ecadd2f9c90629ddf4c3b94`.
- Library/Clippy/app-test/app-build logs use `mutable-standard-*-20261013-final-01` under `local/behavior/`.
- Original `20261008T1310123498555Z/9f3d30.c.txt`, `20261007T1822321790621Z/a0c9f0.c.txt`, `20261012T2124078696226Z/a0e880.c.txt`; aligned `outer-wheel-source-arguments-20261013-draft-01.json` and constant `outer-wheel-arguments-20261013-draft-01.json`.
- `local/scene/preservation-20261013-mutable-standard.json`: all130 Steam resources and original executable unchanged at2026-10-12 22:05:47.554197UTC.

Reused Python3.12.14, Rust/Cargo1.99.0, MSVC14.44.35207/SDK10.0.26100.0 and Capstone5.0.6. No installs, desktop input, game-settings changes or owner assistance. Next recover the wheel argument/box source mapping, verify source reset with this prelude, then enable owned damage RNG and test controlled heavy contact before general crashes.
