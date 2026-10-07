# Live placement comparison — 7 October 2026

F062: Rust reproduces **all twelve captured return-boundary matrices, 192/192 f32 words**, from recorded placement inputs and the purchased Donington collision resources. This is a supported placement result for one restart. Source-only initialization and the original complete collision-query policy remain partly unresolved; no driving fidelity follows from this comparison.

## Reference and capture

Private uk-UA executable SHA-256 `b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6`, GRID 1.3.0.0 / Steam build 252066 / x86. Session `local/reference-race/resume-20261007e/session.json`: PID 33628, start `2026-10-07T01:15:03.8241553Z`. Touring Cars / Chevrolet Lacetti / Donington Park / Grand Prix Circuit, three laps, Nevada Autosport livery; Race Options visibly Normal / Random grid / four Flashbacks. Assists were not separately rechecked in this run.

The no-debugger control loaded successfully. The stationary player's HUD reached 02:39.01 before pausing. A guarded unsuspended manager read found twelve bodies and all 171 requested blocks after scanning 156,631,040 bytes within a 256 MiB bound. Kind-zero player was in slot 5. Other cars uniquely resolve to six more Lacettis and five BMWs through source wheel positions/radii. Addresses and slot assignment are process-specific.

Attempt 08 attached after the race was loaded and armed entry `0x009de4e0` while running. Observed menu input selected and confirmed Restart Race. Every captured call returned to `0x009ef17d`, with return ESP equal to entry ESP +4. Entry code bytes matched the private analysis reference. Captured fields include incoming X/Z, raw heading, Y upper limit, four complete wheel records, initial wheel geometry, base bounds and the returned 64-byte matrix. Upper Y was 10000 in all calls; all 48 wheel Y-displacement words were zero. This captures placement results before the caller continues, rather than settled poses after simulation.

Private evidence:

- `local/behavior/live-spawn-20261007-08/capture.json`, SHA-256 `7007c2feb3efc141673788634446af80251bd8835c83da1b3b82b253cd83d2c4`; twelve individual case files and debugger logs.
- `local/behavior/general-vehicle-manager-20261007T0121508613464Z.json`, SHA-256 `1c3133c2439da9799eda7566228cb1aecf25abd824c122ea402b77c3267f354d`; manager/body/slot linkage.
- `local/behavior/process-context-20261007T0116535874968Z.json`; private executable and game-directory candidate validated through WOW64 process parameters. Internal CWD offset is not a public Windows ABI guarantee.

Debugger reports completed detach and exit zero. Subsequent UI became black and privileged inventory no longer found the game process. **Survival after this GRID capture is not verified**, and the reason for exit remains unknown. The recorded guarded return-boundary results remain valid.

## Independent replay and retained failed comparison

`tools/validate_live_spawn.py` checks identity linkage, code/call/return guards and source wheel geometry, then supplies captured float words to independently authored `grid-placement-track`. The Rust CLI loads all 119,784 Donington collision triangles read-only, uses its f64 BVH to select a candidate, optionally refines that candidate with the previously verified original-style f32 triangle kernel, and evaluates the reference SSE placement arithmetic. Nine ordered casts occur per car.

| Comparison | Exact matrices | Exact matrix words | Interpretation |
|---|---:|---:|---|
| Captured inputs, f64 fraction rounded to f32 | 0/12 | 79/192 | Candidate geometry alone is insufficient; max basis error 0.00127220, max Y error 0.00117779 |
| Captured inputs, selected triangle refined with f32 kernel | 12/12 | 192/192 | All basis and translation words agree for this restart |
| Source-derived center, raw heading, wheels and f32 refinement | 10/12 | 190/192 | Two incoming X words remain different; all basis/Y words agree |

Reports: `local/behavior/live-spawn-replay-20261007-01/validation.json` (failed f64 comparison), `live-spawn-replay-20261007-02/validation.json` (exact replay), `live-spawn-source-20261007-01/validation.json` (source comparison). Each records executable/resource hashes, exact inputs, candidate indices and result words. The report's `kernel` mode means refinement of an independently selected candidate, **not a recovered original world traversal/filter/tie policy**.

## Source alignment and the two residuals

`grid-world::spawn::reference_front_aligned_center` implements the inspected mode-one SSE reciprocal-square-root/reciprocal seed and refinement order. `grid-spawn ... kernel` uses that center, raw rectangle A−C heading and selected-candidate f32 refinement. Historical `geometric` and `reference` modes remain separate. Private reports `local/behavior/spawn-kernel-20261007/` contain 21 markers for each reference car; all 378 new kernel-mode query records have not been independently checked as a set.

`tools/probe_grid_alignment.c` independently isolates original routine `0x00915270` in the existing hash-guarded private PE mapper. It runs no entry point, imports, TLS or live-game injection. Authored stubs supply front/rear/side extents and zero speed, omit the reset/invalidation calls, and record the placement arguments. Only supported mode one with zero lateral displacement is exercised; random selection is excluded by a width/side guard. Code patch pages are restored executable after writes.

`tools/validate_grid_alignment.py` reconstructs source extents and offsets independently from `.ned`/CSV, checks existing marker provenance, runs the isolated original, and compares original/Rust/live values. **12/12 controlled cases, 60/60 X/Z/heading words agree between original and Rust.** All 36 raw source-heading words and 48 source body-offset words also match the live capture/snapshot. Evidence `local/behavior/isolated-grid-alignment-validation-20261007/validation.json`, with input/output, executable and resource hashes. First native compile failed because the C compiler rejected a thiscall typedef and function-pointer conversion; corrected ABI is fastcall with unused EDX and the inspected four stack arguments, and explicit integer patch targets. `/W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2` build then succeeds using MSVC 14.44.35207 / SDK 10.0.26100.0.

| Body ordinal / source / slot | Captured incoming X | Source and isolated-original X | Absolute difference |
|---|---|---|---:|
| 7 / BMW / 7 | `c2fb6799` | `c2fb6798` | 0.00000762939453125 |
| 8 / BMW / 8 | `c2ee2c9e` | `c2ee2c9d` | 0.00000762939453125 |

The source getter at `0x009b8750` reads base bounds, optionally transforms front/rear points through `0x009a39d0` when body byte `+0xc7ad` is nonzero, then adds body offset `+0x1630`. The latter routine clamps a point to a lattice and performs trilinear interpolation before adding the resulting displacement. It can introduce float rounding as well as geometric deformation. Its flag/lattice and actual live marker corners were **not captured**. A matching base bound/offset/raw heading therefore does not prove identical live front extent or corners. Deformation/interpolation is a plausible explanation, not an established cause. No arbitrary rounding adjustment is applied to Rust to hide these differences.

Targeted private pseudocode: `local/behavior/20261007T0136062418971Z/9b8750.c.txt`, `20261007T0138401121960Z/9a39d0.c.txt` and `9de090.c.txt`. The reset consumer resets linked pose/velocity and flags; it does not prove the lattice state was cleared before the captured extent read.

## Debugger failure investigation

Earlier attempts are retained in `live-spawn-20261007-01/` through `07/`. They include missing output directory, no implicit attach pause, unexpected first-chance C++ stops, frozen intro/menu and target zero exit before placement. These do not establish an anti-debugging mechanism.

Attempt 07 completed detach but left main thread 6332 suspended and menus unresponsive. ProcDump 12.01 saved a 29 MiB dump while returning 1 despite a success message. The bounded offline parser initially rejected legal unused stream padding; corrected output reports zero suspend counts in all 78 dump records, contradicting the live thread inventory. Those dump fields cannot establish the live suspend count here. `tools/resume_reference_suspended_thread.ps1` verifies exact process identity, prior/current suspended state and thread ownership, then performs one ResumeThread call. Previous count was **1**, and fresh menu input worked afterward. Evidence: attempt-07 dump/thread files and `local/behavior/thread-recovery-20261007T0126483503977Z.json`.

Pinned x64dbg 2026.05.27 pause source accesses a mutable active-thread handle across suspension/resumption. Upstream [PR 3906](https://github.com/x64dbg/x64dbg/pull/3906), merged 16 July 2026, fixes that race. This supports the suspension diagnosis; it does not explain every prior exit. Official latest-release API still identified the installed 2026.05.27 release, so no duplicate version was installed.

The capture helper now supports `--arm-running`, avoids explicit pause, passes bounded known first-chance C++ stops, recognizes target exit and waits for completed detach before debugger exit. Disposable x86 lifecycle test proves breakpoint/detach/survival separately (`local/behavior/debugger-lifecycle-20261007-03/validation.json`). Tools reused: x32dbg 2026.05.27, Ghidra 12.1.4 / existing JDK 21, Python 3.12.14, Capstone 5.0.6, Rust 1.99.0. No installation this continuation.

## Next behavior questions

Capture actual live corners, extent getter result and deformation flag/lattice at an earlier boundary in a fresh cold load, and verify target survival. Recover complete ground-query filtering/ties. Establish the render-to-body offset before replacing the viewer's horizontal anchor. Trace force/suspension consumers and simulation timestep before implementing driving. [Placement specification](GROUND_PLACEMENT.md) and [implementation roadmap](IMPLEMENTATION_PLAN.md) define the broader scope.

## Source-built packed queries in placement replay (F094)

`grid-placement-track ... packed` now uses the source-owned collision world for all nine casts per car, with shared rejection cache across the replay. It returns height from the selected accumulated f32 fraction and reports the selected source triangle identity. It neither selects a triangle through the f64 BVH nor refines an independently chosen candidate. The packed branch no longer builds that unused BVH. Historical f64/kernel modes remain available for comparing earlier evidence.

**All12retained placement matrices /192words match exactly in both debug and release**, using the original captured incoming X/Z/heading/wheel inputs and the original installed track archive. Final `local/behavior/live-spawn-packed-20261007-04/validation.json` debug, `05/validation.json` release; earlier01..03also match (01has historical limits wording corrected for later reports). All108ordered casts execute per replay. This verifies the retained return-boundary cases, not synchronized live queries or complete driving. The original whole-query comparison and grid/source ownership are separately verified in [source world](SOURCE_WORLD.md).

The earlier two source-derived incoming X differences remain unresolved. Incoming capture values are still supplied to this replay; no artificial correction has been added. Render/body linkage, live extent/deformation/corners and nonzero shape transforms remain open.

Full85workspace tests, formatting and warnings-denied Clippy pass, `packed-placement-tests-20261007.txt`, `packed-placement-clippy-20261007.txt`. Historical selected-kernel mode regression still reproduces12/12 /192words, `live-spawn-kernel-regression-20261007/validation.json`. No failed comparison in this integration. Existing tools reused; no game run, settings or save changes. Source loading is committed separately47d5e69.
