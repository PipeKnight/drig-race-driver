# Damage handling and the owner's frozen run

The latest owner run exposed an unimplemented terminal wheel-damage transition. The application deliberately paused on a returned error; this was not a deadlock. The fix retains the source terminal state and lets physics and reset continue. Terminal damage disables driving controls on the next synchronization, matching the selected original logic.

## Observed run

Private log `local/playtest/manual-11951-24959.log`, SHA-256 `911bd96326137f19af2e896cf8f7b5f8559e323f657d0a8424668aaa8207a5d1`, identifies playable build `d221dc28f774`. Source body contacts, body events and physical deformation were enabled.

| Segment | Last successful tick | Attempt that paused | Simulation time before pause | Error |
|---|---:|---:|---:|---|
| 0 | 2147 | 2148 | 34.352 s | unsupported handling notification event |
| 1 | 2695 | 2696 | 43.120 s | unsupported handling notification event |

Both pauses were followed by successful resets. Segment 2 then ran 55 ticks and exited. The log contains 499 telemetry records across three segments; the two failing segments contain 490 successful motion samples. The summary tool's `passed` means its parsing and validation succeeded, not that the playtest had no failures.

Private summary `local/playtest/owner-damage-freeze-20261014-final-01.json`, SHA `d1494131060e359cc6b55a5555b24693d360c82878a51a696a57a2d30803aa33`.

## Does damage affect handling?

Yes, some effects already reach the driving model. In the exact control replay, wheel deformation raises the source steering perturbation parameter from zero to approximately 1.768 before the first pause and 2 before the second. The synchronization copies it into the steering input consumer; this affects the conditioned steering command, including its speed-dependent oscillation. Wheel levels and steering offsets also feed the existing wheel-control path. Deformed collision geometry affects subsequent physical contacts.

Other effects can remain inactive. In these replays, the source engine/brake multipliers at mutable `3A4/3A8`, steering caps at `3B0/3B4`, and the logged suspension multiplier remain 1 before the stops. This explains why accumulated body damage does not necessarily cause an obvious loss of power or suspension response. The independent graphical dent projection itself is read-only presentation. This evidence does not establish complete GRID damage fidelity, detached parts or all race-ending behavior.

## Original transition and selected scope

Fresh Ghidra 12.1.4, JDK 21.0.9 survey `local/behavior/20261013T2325520571482Z/` successfully decompiled original `9F2F80` and `A14E40`. Aligned instructions are retained in `local/behavior/wheel-terminal-instructions-20261014.json`.

The original wheel routine compares the **unclamped** deformation ratio against terminal percentage times stored-f32 `.01`. With healthy status 7, notification byte `442` enabled and both car `B13/B14` flags set, it writes status **3** and the first failing wheel ordinal to `448`. It then checks the optional component `+84` notification receiver. Our explicitly owned source-car context has that receiver absent. Continuing after the status write therefore skips no callback that belongs to this selected context.

`calculate_with_empty_notification_receiver` and its publication counterpart implement that scope. Existing guarded APIs retain their earlier behavior. Car jobs explicitly select the owned empty-receiver path. Status 3 persists through later updates; later wheels cannot overwrite the first ordinal. Invalid status/ordinal and late effect failures still reject atomically. The source synchronization already converts non-healthy status into disabled controls and automatic braking; the fix preserves that behavior.

The app now reports terminal wheel damage in its title, keeps its tick counter advancing and offers **R to reset**. Additional private handling telemetry records the status, wheel levels, effects and control-disable state.

## Verification

Whole unchanged original `9F3E80`, repeated original, debug Rust and release Rust match **896 cases / 122,533,376 bytes per profile**. Coverage includes 128 new terminal transitions, 32 already-terminal inputs, simultaneous collision damage and wear, disabled notifications, disabled wheel limits, displacement including W, and different failing wheels. A carried second update is also exact. Twelve malformed-context outcomes reject without producing output. Native mapped data pages/protections and the complete image are restored; buffers and component/query ownership are checked. No original code or entry point is patched.

Private report `local/behavior/handling-terminal-20261014-final-02/validation.json`, SHA `6233d760f0b2bd1e3b585bda16b3d1b849f0e8ab3b4c16b5f8e8334393e5210e`. Read-only reconstruction audit checks 145 primary files, 13 current identities and all outcomes: `local/behavior/handling-terminal-audit-20261014-final-02.json`, SHA `4fcf9ce5aad9e4f8ecab5669a455e188b8f53ebb03c35b655bdaa4dc1e4baaf7`.

`tools/replay_damage_freeze.py` reconstructs controls by holding the previous logged input until the next logged change. `grid-drive-replay` checks every available successful pose/velocity word exactly, then holds the previously failing input for an explicitly authored continuation. Debug, release and repeated release outputs match. Segment 0 reaches terminal **front-left** wheel at tick 2148 and completes tick 2448; segment 1 reaches terminal **front-right** at tick 2696 and completes tick 2996. Both disable driving controls on the next tick and continue physics for 300 more ticks. This is reproduction of the independent prototype run, not an original-game crash capture or calibrated gameplay comparison.

Private replay report `local/playtest/damage-freeze-replay-20261014-final-03/validation.json` contains input, executable and output identities. Rust/Cargo 1.99.0, Bevy 0.19.1, Python 3.12.14 and the existing MSVC 14.44.35207 tools were reused. No installation or desktop input was needed. All 201 ordinary library tests and five source-backed Bevy tests pass, including terminal continuation and keyboard reset in baseline, body-contact and deformation modes.

The first diagnostic helper compilation failed because it guessed source preparation APIs; it was corrected to reuse `prepare_body_geometry` and byte readers. Its first debug execution exhausted Windows' default stack; an explicit 16 MiB worker stack, matching existing source tests, fixed it. Clippy identified a constant-chunk iterator improvement, corrected before final checks. These failures are diagnostic-tool failures, distinct from the owner's runtime failure. No native numerical mismatch or fault occurred.

The unfinished F269 lap-timing work is preserved in a named local Git stash while this fix is built independently. Continue that work after the corrected playable is recorded. Other unsupported branches remain explicit errors; this fix does not blanket-ignore simulation failures.
