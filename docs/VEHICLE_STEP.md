# Complete selected vehicle wrapper — F183

11 October 2026, Europe/Kiev. Independent Rust now matches one call to the complete unchanged original `A06A90`, including its temporary-frame initialization and the motion assistance, settling and contact braking after the driver. The same result holds inside linked synchronization and physical publication. Positive-duration idle/clutch transitions, `A03220` and physical-world scheduling remain unfinished; the Bevy viewer is not drivable yet.

## Behavior and ownership

The wrapper spans entry `A06A90` through return `A076E8`. It captures the previous position, derives byte `1640` from the handbrake, speed, controls and gear, and creates a temporary view and solver. View floats `298..2A4` are one; `2A8` is zero. Positive delta calls the complete driver `A03670`; zero delta skips it but still executes the outer tail.

The tail applies speed/angle/contact-scaled angular assistance and damping, maintains the low-speed timer, conditionally projects nearly stationary motion onto the car's up axis and clears contacted wheel spin, then applies contact-count-scaled braking. Float grouping follows aligned SIMD instructions, including refined reciprocal-square-root operations. The `BF8C93` call is interpreted as arcsine and operationally verified for the tested inputs; this is not a universal cross-platform libm equivalence claim.

Rust clones the complete frame and commits only after successful validation. Nonfinite results, an active zero-width angle interval, unsafe delta and currently unsupported positive idle/handbrake branches fail without publishing partial state. A late braking overflow unit specifically proves complete rollback.

The original wrapper allocates its own temporary storage, which expires on return and contains uninitialized bytes. The native adapter never follows that expired pointer. Comparison includes **every byte of the complete 0x2600 car**, record metadata, material identities/data and observed globals, with explicit owned-pointer relocation. It excludes the temporary view/solver. Rust creates deterministic temporary storage; its bytes are not claimed equivalent to the original's expired storage. Native code is unchanged and seven data pages are restored in the disposable mapping.

## Verification

| Per debug/release profile | Calls | Compared bytes |
|---|---:|---:|
| Whole wrapper, positive delta | 140 initial + 192 carried | Included below |
| Whole wrapper, zero delta | 96 | Included below |
| All wrapper cases | 428 | 4,163,584 complete-car / 4,742,240 observable |
| Whole synchronization before wrapper | 332 | 18,221,488 |
| Whole wrapper inside synchronized publication | 332 | 3,229,696 complete-car / 3,678,560 observable |
| Linked publication | 332 | Complete car/physical snapshots at three stages |

Every original/repeat/Rust comparison agrees. Each branch carries its own car/physical state without copying Rust output into the original. All 44 supported source tables independently initialize curves, limits, ratios and shift thresholds. Constructor, ownership and interframe history/input fixtures remain authored. Linked publication uses no advance/correction, zero deformation dimensions, empty shapes and disabled reset notification.

A separate complete-driver baseline establishes actual outer-tail effects: 51 angular changes, 83 braking changes and 78 timer changes per profile. Positive pose settling is **not covered**: the inactive clutch branch is currently rejected, and zero-delta cases retain their initial position. Zero-delta cases do exercise contact flags, speed/timer thresholds, handbrake/clutch flag selection and contacted wheel spin clearing. Sixteen malformed cases each pass native and Rust guards (32 outcomes), including late braking overflow. Fresh F182 regressions pass all 332 driver calls, 79 existing guards and 15 native framing checks per profile.

All 144 library tests, formatting, warnings-denied all-target Clippy, Python AST, whitespace and six-report audits pass. Existing Rust/Cargo 1.99.0, Bevy 0.19.1, Python 3.12.14, MSVC 14.44.35207 / SDK 10.0.26100.0, Ghidra 12.1.4 / JDK 21.0.9 and Capstone 5.0.6 were reused. No tools installed. Original Steam executable and 130 traced resources remain unchanged at **01:55:21.193399 UTC**.

## Failures and limits

The first carried run stopped at native guard 66 because processed controls became zero at low speed, entering unsupported idle. Authored next-frame throttle now keeps this experiment within its declared engaged branch; that is not an idle fix. Zero-delta initially stopped at a prior positive-only post-driver guard; guards for routines skipped at zero were corrected. A braking-overflow guard initially left velocity zero, so the original correctly skipped the operation; activating velocity corrected the fixture. One Rust array-type compile error was fixed before execution; aligned dot/braking grouping was corrected before comparison. No numerical mismatch or native exception occurred.

Two guessed tool paths and a mistaken skill documentation subdirectory failed harmlessly; correct files were located. A screenshot forwarding mistake printed its encoded payload; subsequent captures forward image content without printing payloads. The private game launch initially reported no targetable window; a refreshed window inventory found the unique GRID window and activation succeeded. The guarded macro confirmation subsequently skipped the intro and reached the title prompt. Desktop investigation is separate from the numerical verification and does not establish a new live mode-2 result yet.

## Private evidence and reproduction

- `local/behavior/whole-vehicle-step-20261011-{debug,release}-final-01/validation.json` and guard/baseline reports.
- `local/behavior/whole-vehicle-linked-step-20261011-{debug,release}-final-01/validation.json`, source audits and complete publication snapshots.
- `local/behavior/vehicle-step-driver-regression-20261011-{debug,release}-final-01/validation.json`.
- `local/behavior/whole-vehicle-step-final-audit-20261011.json`, final build/test/Clippy logs prefixed `vehicle-step-` or `whole-vehicle-step-`.
- `local/scene/preservation-20261011-whole-vehicle-step.json`.
- `local/behavior/whole-driver-wrapper-aligned-20261011.json/log`, `vehicle-wrapper-constants-20261011.json/log` and retained earlier wrapper surveys. Adjacent-function bytes outside the stated return are excluded.
- `local/reference-race/idle-20261011/session.json` and `macros/`: hash/start-time-bound game PID 5092 and macro runner PID 38112. These identities are historical evidence, not reusable authorization after process changes.

Final wrapper probe SHA256 `65818e1476a81d54517b30d8e874757a9b324eb49273958b0d0b5146fe854cde`; shared driver probe `f37cd569af39f8310b9257ca051f71f896f8d9c022e84421b8c4179aa0c345b1`. Rust force CLI debug `34c4d639afa96f1b08eca9ef828b2e683244f53209169be670cb6555bb6db7a1`, release `3b08a0705b98b6904d707afb89ee1249c51411fcf9dd83058082dee77db62b0e`. Analysis PE remains `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.

Build `tools/build_whole_vehicle_step_probe.cmd` and the shared driver baseline probe. Run `tools/validate_whole_vehicle_step.py PRIVATE_OUTPUT --profile debug|release`, followed by `tools/validate_vehicle_linked_driver.py PRIVATE_OUTPUT --profile debug|release --whole-step`. Outputs must be new paths under ignored `local/`.

Next recover and verify the positive idle/clutch/mode-2 force branch, then join `A03220`, real physical contacts and world scheduling. Complete source initialization and Bevy input/camera/restart/checkpoints follow. These gates, rather than additional isolated helpers, determine readiness for the first repeatable Lacetti/Donington lap.
