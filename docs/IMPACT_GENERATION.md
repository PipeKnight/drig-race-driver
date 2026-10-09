# Contact-to-grid impact generation

Recorded 9 October 2026. Independent Rust engine research; all original binary data and live captures remain under ignored `local/`.

## Observed original boundary

The producer at `9EA6E0` generates ordered 48-byte records, updates node damage bytes and descriptor state, then calls `9D4210`, the grid solver at `9D4970`, and an optional collision-response stage. This investigation isolates the **generation prefix**, ending at `9EB534` after the `A480` state write and before the zero-count return branch or `9D4210`. It does not equate that prefix with the whole producer.

Aligned instructions established two previously omitted inputs: body vector `B20` and scalar global `E51D40`. The capture helper now retains both at producer entry. Existing captures include linked pose `1D0..20F`, linked vector `360`, body vector `1630`, complete descriptor/settings, contact records, counterpart vectors, globals `E500C0..DF` and corner scale `E4FC2C`.

Fresh capture07 contains three calls with three contacts each and no generated records. Resting against the blue barrier did not provide a useful active generation case. After observed Restart Race / confirmation, F2 throttle/right and F5 forward approach the concrete barrier. Capture08 retains six active calls:

| Call | Contact pointers | Generated records |
|---|---:|---:|
| 0 | 3 | 18 |
| 1 | 7 | 30 |
| 2 | 14 | 43 |
| 3 | 8 | 18 |
| 4 | 4 | 14 |
| 5 | 4 | 14 |

Macro log holds are 2,000 ms, with key release and normal completion; debugger pauses preclude tick-accurate interpretation. Both captures pass body/wrapper/registration, code-prefix, return-address and stack-balance guards, then detach with debugger exit0. A fresh post08 screenshot verifies Pause Menu / Continue beside the concrete barrier. GRID PID36228/start `2026-10-08T23:53:45.3524205Z` and macro6868/start `2026-10-09T07:23:22.0284687Z` survive; these identities are historical and must be rechecked before reuse.

Private evidence:

- `local/behavior/live-impact-20261009-07/capture.json`, SHA256 `43e3c780ed8ced4443f755094916c6977b528636aa6dbcb3ea0a3550376d7d80`.
- `local/behavior/live-impact-20261009-08/capture.json`, SHA256 `ad7c3264cae6b41333c493a8143bd498d72c60e3574a89da3fc14e92a3639678`.
- `local/behavior/impact-generation-instructions-20261009.json` and readable disassembly; `impact-generation-constants-20261009.json`. A first one-line constant-extraction command failed quoting before execution; the corrected stdin script produced the retained evidence.
- Session and control logs: `local/reference-race/impact-20261009-hardware/`, latest handler under `macros-left/`.

## Original-prefix replay

Authored `tools/probe_impact_generation.c` maps the hash-pinned private analysis image as RX code and reconstructs one body/wrapper registration plus bounded linked/contact/settings data in its own process. No entry point, TLS, general import initialization or substituted numerical helper executes. The producer's real body lookup, maximum helper, x87 min/max and dirty setter run unchanged.

An application-defined exception arms an execute hardware breakpoint in this probe's own thread. At `9EB534`, its SEH filter checks the exact exception/address/debug-register cause and count<=400, retains the records from stack+`3F0`, then unwinds to the harness. The previous debug registers are restored afterward. The probe deliberately does not continue through the solver or response. This uses the documented [x86 CONTEXT debug-register fields](https://learn.microsoft.com/en-us/windows/win32/api/winnt/ns-winnt-context-r2), [vectored exception context](https://learn.microsoft.com/en-us/windows/win32/debug/using-a-vectored-exception-handler) and [SEH handler control](https://learn.microsoft.com/en-us/cpp/cpp/try-except-statement). Actual operation is established by the retained executions, rather than documentation alone.

Six private data pages (`E4F000`, `E50000`, `E51000`, `E58000`, `E5A000`, `E85000`) are writable during the call and restored. Executable-section bytes are checked unchanged. Contact pointers are rebound only in the fixture, preserving owner/counterpart selection. Supplied prepared vectors are zero in this prefix fixture because their first reads occur later in `9D4210`; no claim about that later path is made.

`tools/validate_impact_generation.py` compares each complete 0xA500-byte descriptor with the corresponding live nested solver-entry descriptor (or whole-return descriptor for zero-record calls). Only the settings pointer at `A4F8` is normalized to zero. All **44 bytes actually written in each impact record** are compared in order: direction/anchor vec4, node/contact signed-short ordinals, scalar and weight. The unwritten word at `2C` is excluded as uninitialized stack storage; it is not a numerical tolerance.

Original and repeated original match all three zero-record calls and all six active calls: **137 records / 1,507 written record words and 63,360 descriptor words** for the active set, with zero differences in each replay. This also observes that `9D4210` does not change the compared descriptor between these boundaries in these six calls; it is not a general claim that `9D4210` has no side effects.

Probe SHA256 `93cc9384c0a3718e0876e009f01afcce60dd61bd3531bed86fd91782a730aa31`, analysis SHA256 `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`. Reports: `local/behavior/impact-generation-replay-20261009-{01,02}/validation.json`. Existing Python3.12.14, MSVC14.44.35207/SDK10.0.26100.0, x32dbg2026.05.27, Capstone5.0.6 and AutoHotkey2.0.29 reused; no new installations.
