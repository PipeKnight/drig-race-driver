# Source grip reductions (F111)

Recorded 8 October 2026. `vehicle::read_standard_grip_reductions` reads Standard car CSV columns 178 and 179. Original decoder `9aec60` reads staging cells +2c80/+2cc0, stores f32 values at properties+53c/+540, and substitutes 0.5 when those stored values equal zero. Constructor `a0c9f0` copies the properties to body+1fa4/+1fa8. The [verified downstream helper](GRIP_HISTORY_FACTOR.md) selects them for positive and nonpositive wheel-record Z respectively.

The fallback follows f32 storage: empty cells, either sign of zero, and finite f64 values that underflow to f32 zero all become 0.5. Nonzero subnormals remain nonzero. The public reader rejects nonfinite values and f32 overflow and retains the existing supported Standard row/body guards. Other physical names are provisional.

**54 datasets / 2,592 cases / 5,184 output words exact in debug and release**: all 44 hashed installed Standard CSVs plus ten private authored column variants covering defaults, underflow, signed subnormals, rounding and reductions outside zero-to-one. Each dataset exercises all four original wheel ordinals, both flags and three coordinate signs. All 44 installed rows produce 0.5 for both reductions.

Native `grid-grip-history INPUT OUTPUT CAR.csv` reads the actual CSV; the input reduction words deliberately contain different placeholders. The unchanged whole original `98db40` probe receives independently decoded Python values. This verifies source adaptation and its consumer; **the original whole CSV decoder is not invoked**, and actual live flag/body-state values are not established. Incoming histories, body+1ffc/+1f24 and grip remain supplied.

Evidence under `local/behavior/`: `source-grip-reductions-20261008-01/validation.json`, `02/validation.json`; previous supplied mode regresses all 1,308 cases / 2,616 words in both profiles, `grip-history-source-regression-20261008-01/`, `02/`. Original helper probe SHA **1fe31f67e12e1a8bce07ee788913bf800e723afc68c107495ce2d80ef41ae8a9**, pinned by the source validator. Installed hashes derive from the retained original body-prelude manifest and are checked before each source run. Authored CSV derivatives remain private.

Static decoder `20261007T2327230068908Z/9aec60.c.txt`; constructor `20261007T1822321790621Z/a0c9f0.c.txt`. Additional investigation `20261008T0013240243230Z/field-stores.tsv`, `20261008T0014152064705Z/` finds:

- `9bd7f0` copies body+22e0..22e3 to +227c..227f. The upstream flag meanings remain unproved.
- `990610` initializes settings slot +2c to 0.5; the constructor copies settings into body+1fd0, making its corresponding initial body+1ffc value 0.5. Later updates remain open.
- Constructor body+1f24 starts at one. `9ae8f0` has a +1f24 store in a six-wheel layout branch, where wheel records overlap this offset; this does not establish a Standard four-wheel mutation.

The literal-displacement survey covers MOV-family stores only and is not exhaustive mutation proof. Inferred decompiler types/labels do not replace object identity and live validation.

**95 workspace tests**, formatting and warnings-denied Clippy pass: `source-grip-reductions-tests-20261008.txt`, `source-grip-reductions-clippy-20261008.txt`. The new public unit check covers underflow/default order, preserved signed subnormals and rejected invalid numeric values. No numerical or check failures. Existing Ghidra 12.1.4/JDK 21.0.9, Rust 1.99.0 and Python 3.12.14 reused; no tools installed or game/settings/save changes. Other source settings, runtime flags, grip modifiers, force assembly and cadence remain open.
