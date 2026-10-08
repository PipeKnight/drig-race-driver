# Downstream history grip factor (F110)

Recorded 8 October 2026. `grid_world::grip_history::{factor,apply}` independently implements original `0098db40` under supplied body scalars/flags and wheel histories/coordinate. In original `a00e10`, this helper is called after `9ba490` and multiple other grip modifiers. Its returned factor multiplies the already-rounded incoming grip. Verification covers this stage; the complete force path remains open.

**1,308 cases / 2,616 words exact in debug and release**: 96 controls covering each of four wheel ordinals, both flags and negative/subnormal/signed-zero/positive Z; 700 histories and incoming grip values produced offline by F109's original whole calculator; 512 seeded supplied states, including negative and greater-than-one histories. The original leaves this helper's factors unclamped. It reads:

| Input | Observed role |
|---|---|
| wheel+120 | `1 - value*0.25` |
| body+b12, wheel+11c | If flag set, multiply by `1 - (1-value)*stored_f32(0.15)` |
| wheel+124 | Multiply by `1 - value*0.5` |
| body+1ffc | Multiply by `1 - (value-0.5)*stored_f32(0.1)` |
| body+1f24 | Additional scalar multiplier |
| body+227c+ordinal | Enables coordinate-dependent reduction |
| wheel+8 | Z lane of the original four-lane wheel record; positive selects body+1fa4, otherwise+1fa8 |
| body+1fa4/+1fa8 | Selected reduction `1-value` |

Aligned instructions group the dirt expression with the history factor before the body+1ffc multiplier. All operations are f32. The returned x87 value has already passed a f32 store; native `apply` retains the caller's subsequent multiplication/store boundary. Negative-zero Z selects +1fa8. Public guards reject nonfinite or overflowing inputs/results.

Evidence: `local/behavior/grip-history-20261008-01/validation.json` and `02/validation.json`; probe SHA **1fe31f67e12e1a8bce07ee788913bf800e723afc68c107495ce2d80ef41ae8a9**. Whole original code runs RX with no patches, stubs, writable mapped pages or live injection. Supplied bodies occupy aligned caller buffers; each case exercises the original ordinal-dependent address calculation. x87 precision is 53 bits, round-to-nearest. Retained F109 report SHA **1c589c15b6d175e80088d62924c1f78a5efbca4ff65d3cee324cbf027fde0350**, pinned by the validator. Retained histories are offline computed inputs, not live captures.

Static evidence under `local/behavior/`: `20261008T0009086593468Z/98db40.c.txt`, `grip-history-factor-instructions-20261008.json`, `grip-history-factor-constants-20261008.json`; caller `20261007T0148078289595Z/a00e10.c.txt`. Constructor `20261007T1822321790621Z/a0c9f0.c.txt` initializes +1f24 to one and copies properties+53c/+540 to body+1fa4/+1fa8. Their CSV linkage, later mutation and actual runtime flag/state values require separate verification; these observations alone do not establish source initialization or physical meanings.

**94 workspace tests**, formatting and final warnings-denied Clippy pass: `grip-history-tests-20261008.txt`, `grip-history-final-clippy-20261008.txt`. Initial Clippy reports two assignment-pattern warnings; a scoped allowance preserves observed multiplier-first order. No numerical failures. Existing Ghidra 12.1.4/JDK 21.0.9, Capstone 5.0.6, MSVC 14.44.35207/SDK 10.0.26100.0, Rust 1.99.0 and Python 3.12.14 reused. No tool installs or game/settings/save changes. Source scalars/flags, other main-update modifiers, force assembly and main cadence remain open.
