# Complete selected original vehicle driver — F182

11 October 2026, Europe/Kiev. The existing independently authored Rust driver now agrees with **one call to whole unchanged original `A03670`**, rather than only an authored composition of its helpers. The same comparison passes when whole linked synchronization and physical publication surround each update. This strengthens the driver boundary; the outer wrapper and physical-world schedule remain open.

## Original execution boundary

Aligned instructions establish entryA03670 through returnA043E7; whole chassis call9BE1E0 atA043DC is its last call. Earlier notes placed that call five bytes later using inferred pseudocode labels; aligned instruction addresses supersede them. The previous driver preparation, force vectors, source contact/suspension/grip, controls, propulsion, force solve, post-force assistance and local chassis stages cover this complete selected interval.

Whole original installs the contact-table pointer and table index from world globalsE85FD0/E85ED4 into context608/60C before callingA00E10. The earlier isolated contact adapter supplied these links directly. The new adapter installs an owned table in those globals and supplies the owned body back-pointer; it normalizes only these explicit relocations after execution. Real packed Donington source-world queries and original mechanics lookup remain active. No executable instructions, helper entry points or dispatch table are patched.

The native probe reuses the existing bounded source-world loader. Each EF01 record owns body2600, overlapping view/solver storage (solver=view+2B0), driven/steering wheels and source material tokens. Conservative guards from earlier helper probes are reused; their authored helper schedules are never called to generate output. Overwritten view force vectors and wheel load138 are neutralized **only on separate guard copies**, because earlier driver preparation/suspension replace them before consumption. The actual original input remains untouched.

Supported state remains engaged assisted automatic,mode1,four wheels,inactive respawn/nitro/special gear search and the existing source-contact restrictions. All44 supported Standard source tables initialize their own curves/limits/thresholds and sparse ratios; other constructor/setup inputs are authored fixtures. This does not establish full constructor correctness or gameplay support for44 cars.

## Verification

| Per debug/release profile | Calls | Complete bytes |
|---|---:|---:|
| Whole original driver vs independent Rust | 140initial +192carried =332 | 5,107,488 |
| Whole synchronization before whole driver | 332 | 18,221,488 |
| Whole driver inside synchronized/publication loop | 332 | 5,107,488 |
| Linked publication after each driver | 332 | Complete physical/car snapshots at three stages |

Every original/repeat/Rust word agrees. Each branch initializes and carries its own state. Each suite includes134 source contacts/nonzero grips,1,327 spin changes,108 gear changes,138 fuel changes and332 pose/velocity/chassis changes. The existing79 guard outcomes pass, plus **15 new native framing/ownership/global-input/output checks**, including preservation of an existing output file. No advance/correction occurs during linked publication; authored interframe history,zero deformation dimensions and empty linked shape lists remain explicit limits.

The initial probe stopped safely at guard50 before original execution: a later-stage guard consumed an old NaN wheel-load138 that suspension overwrites. A separate guard copy now models that overwritten field. The first linked run completed all numerical comparisons but failed its final coverage assertion because the new comparison used shorter coverage keys; the assertion was corrected and both profiles rerun. A guessed private fixture path exited before execution; actual constants were read from the installed scripts. One guessed header and Windows wildcard search failed harmlessly. No numerical mismatch or native exception occurred.

Native MSVC warnings-denied compilation, Python AST, whitespace and four final-report/hash audits pass. Rust implementation is unchanged from F181's142 passing library tests/fmt/Clippy; this milestone adds stronger original execution validation. Existing Rust/Cargo1.99.0,Bevy0.19.1,Python3.12.14,MSVC14.44.35207/SDK10.0.26100.0,Ghidra12.1.4/JDK21.0.9,Capstone5.0.6 reused. No desktop input/settings/install changes. Steam executable and130 traced resources unchanged **11 October01:34:05.084283UTC**. Estimates unchanged; expired automation paused.

## Private evidence and reproduction

- `local/behavior/whole-vehicle-driver-20261011-debug-final-01/validation.json` and `...release-final-02/validation.json`, including79 guards and15 native framing guards per profile.
- `local/behavior/whole-vehicle-linked-driver-20261011-debug-final-02/validation.json` and `...release-final-01/validation.json`; sibling source audit folders record44 tables and source hashes.
- `local/behavior/whole-vehicle-driver-final-audit-20261011.json`; final native build log `whole-vehicle-driver-build-20261011-04.log`.
- `local/scene/preservation-20261011-whole-vehicle-driver.json`.
- `local/behavior/whole-driver-wrapper-aligned-20261011.json/log`, retained `20261008T0136288512690Z/a03670.c.txt`. Aligned ranges include adjacent functions, explicitly outside this claim. Wrapper constants `vehicle-wrapper-constants-20261011.json/log` and existingA06A90/A03220 surveys prepare the next boundary.

Final native SHA256 `0f2390101f08bcd46800d3bf3ccb81ba82231fa0fbaef2f3a60967fceb6cc7d9`; Rust CLI hashes remain the F181 hashes, retained in every execution report. Analysis PE SHA256 `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`. The isolated mapper never runs PE entry/imports/TLS. All executable sections are checked byte-for-byte; seven driver data pages are restored before freeing the isolated mapping. Source-world installation is confined to that disposable mapping.

Build `tools/build_whole_vehicle_driver_probe.cmd`. Run `tools/validate_whole_vehicle_driver.py PRIVATE_OUTPUT --profile debug|release`, then `tools/validate_vehicle_linked_driver.py PRIVATE_OUTPUT --profile debug|release --whole-driver`. Existing stage-composed mode remains available as separate evidence.

Next recoverA06A90's actual temporary-frame initialization, motion settling, angle assistance and contact braking; its return isA076E8. Then wholeA03220/physical-world contacts and scheduling, complete source initialization and Bevy driving. The complete vehicle driver alone does not establish a drivable lap.
