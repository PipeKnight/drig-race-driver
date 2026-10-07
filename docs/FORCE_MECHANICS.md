# Production surface mechanics inputs (F104)

Recorded 8 October 2026. The independently authored source descriptor now includes ten additional f32 fields and three boolean flags consumed by the production tyre/drag path. They supplement the [contact mechanics](SURFACE_MECHANICS.md), preserve source order and keep absent MECHANICS explicit.

| Attribute | Original offset | Initializer default |
|---|---:|---:|
| grip_per_ms | 00 | 0 |
| ms_for_max_effect | 04 | 1 |
| camera_factor | 0c | 0 |
| roughness | 10 | 0 |
| minimumSpeedForSlowdown | 14 | 20 |
| speedForMaximumSlowdown | 18 | 40 |
| slowdown | 1c | 0 |
| roll_induce | 20 | 0 |
| heat | 68 | 1 |
| wear | 6c | 1 |
| curved | 71 | false |
| dirt_build_up | 72 | false |
| high_downforce | 75 | false |

Names are original XML attributes, not independently established units. `0099c990` uses grip_per_ms, ms_for_max_effect and grip as a speed-clamped linear expression with a table factor. `009ba490` reads wear and dirt_build_up while updating wheel history; `009dfc50` uses the speed bounds and slowdown. Full tyre force behavior remains open.

**113 material rows / 2,486 field words exact in debug and release** against original `009b90f0`, initializer `0099c8b0` and binary-XML accessors: 89 installed rows (84 mechanics) plus 24 authored controls (23 mechanics). Controls vary every new scalar, defaults, negative values, rounding boundaries, mixed-case boolean text and unrecognized boolean values. Missing mechanics produces absence plus zero report words rather than a fabricated material. The previous contact-only 113-row report also regresses exactly in both profiles.

The boolean accessor `00a67b90` only recognizes case-insensitive “true” and “false”. Unrecognized text retains the initialized field. All 84 installed curved/dirt/high-downforce attributes contain “0”; that is ignored rather than numerically converted. A supplied “1” is also ignored. Initial extended run `force-mechanics-20261008-01/` stopped at the native strict boolean guard; final `02/`, `03/` implement this verified default-retention behavior. Earlier contact boolean handling was broadened consistently. Guard tests continue to reject nonfinite numeric fields.

Evidence: `local/behavior/force-mechanics-20261008-02/validation.json`, `03/validation.json`; `surface-mechanics-regression-20261008-01/`, `02/`; constants `force-mechanics-constants-20261008.json`; original loader/initializer field linkage `20261007T1610340353294Z/`; boolean accessor `20261007T2334083071388Z/`, including resolved true/false constants. Probe SHA **3651b78f9d84b93d3830579560e5e0c718f86572c6e2d5b12bd6fcfb2fa916ab**. **91 workspace tests**, formatting and warnings-denied Clippy pass, `force-mechanics-tests-20261008.txt`, `force-mechanics-clippy-20261008.txt`. MSVC **14.44.35207**, SDK **10.0.26100.0**, /O2 /MT /fp:strict /arch:SSE2 /W4 /WX; existing Rust/Python/Ghidra tool versions remain as recorded in F103.

The reference comparison retains the F083 host-strtod adapter for numeric XML conversion. Original loader/default/accessor behavior is exercised, but the original CRT parser is not initialized or independently reproduced. No complete text-parsing, force generation, live mechanics pointer or playable-driving equivalence is claimed. No tools installed or game/user files changed; all original data and derivative reports remain private.
