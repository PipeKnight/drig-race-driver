# Source gearing and selected transition safety — F179

11 October 2026, Europe/Kiev. The selected vehicle pipeline now accepts the real sparse gear tables from all 44 supported Standard car CSVs. The former blanket requirement that every slot 1–9 be nonzero is removed. A checked transition rejects a zero ratio only when that transition would divide by it; unused slots remain zero.

## Source and control behavior

Aligned `9AEC60` instructions at `9AF4D4..9AF53B` decode reverse CSV column 68 to properties `33C` and forward columns 69–76 to `31C..338`. They are stored as `f32` after parsing. The engaged gearbox branch of constructor `A0C9F0` copies those ten slots into body `18F4..1918`, then aliases slot zero to slot one. There is no observed reciprocal conversion in this copy interval.

`grid-formats::vehicle::read_standard_gear_ratios` implements this bounded Standard-row subset. Empty cells retain zero; negative zero survives; overflow/nonfinite numbers and unsupported row shapes fail. It preserves all eight forward slots and the reverse slot. `grid-body-tuning ... gears` reports the ten stored words privately, with its earlier body/tyre modes unchanged.

Independent source-byte/f32 checks agree with Rust for all **44 supported files /440 ratio words per profile**. These files have four, five or six positive forward slots and **97 unused zero forward slots**. An exploratory scan found 45 Standard-labelled files before applying the existing 360-consumed-column row contract; the unsupported row is excluded from the 44-file claim. No game table or extracted values are published.

`vehicle_controls` retains finite table checks but performs the nonzero check inside the actual gear-change division branch. That branch requires a changed, nonneutral old/new gear, the observed clutch condition, inactive clutch transition and no disabled-control override. Original propulsion and force stages retain their separate zero-ratio-to-one fallback. A malformed single-forward-gear, multiple-downshift path now returns a checked underflow error instead of panicking on `usize` subtraction. Whole-frame cloning still prevents partial steering, gear, solver or event publication on error.

The native comparison preflights the impending selection on an owned body copy immediately before whole original `9B40D0`. It applies the observed disabled override/timer/manual-selection bookkeeping and calls whole original `9B29A0` for automatic selection. The optional torque-search branch is guarded off; the selected automatic helper has no foreign pointer dereference or RNG/global writes. The preflight rejects a selected zero denominator or unsupported index before the unchanged original wrapper runs. It does not replace original selection or generate comparison outputs: whole original controls, engine, differential, traction and force helpers still produce those outputs. The original reverse-disable global remains zero.

This implements decoded initial ratios and transition safety, not complete source initialization. Runtime setup scaling, original shift-threshold construction (`9AD6B0`), engine-curve initialization, complete constructor execution and live timing/ownership remain separate. Source-table comparisons use authored other tuning, threshold, contact and steering inputs; they do not establish complete support for all 44 cars.

## Verification

| Per debug/release profile | Calls | Complete bytes | Guard outcomes |
|---|---:|---:|---:|
| Whole controls/propulsion/forces: 528 source-table +288 retained +384 carried | 1,200 | 18,460,800 | 238 |
| Source gear tables through full selected driver/contact/force/assist/chassis | 332 | 5,107,488 | 79 |
| Fresh preceding F178 full driver regression | 332 | 5,107,488 | 79 |

Every compared word and repeated-original output matches. Each original/repeat/Rust branch carries its own state. Source controls cover first/penultimate/top/reverse gears, automatic/manual modes, rev matching, up/down requests and multiple shifts. **44 explicit top-gear upshift controls** retain the actual top gear. Coverage includes 436 gear changes, 263 event-count changes, 1,100 nonzero torque results, 345 traction-active updates, 1,010 velocity changes and 4,637 spin changes.

Full source-track integration uses the F178 gravity/aerodynamic preparation and all following stages with parsed sparse tables. It has 332 pose/velocity/chassis/acceleration changes, 310 chassis-phase changes, 133 source contacts/nonzero grips, 1,327 spin changes and 17 active angular recoveries. This remains selected composed execution rather than whole `A03670` or a world tick.

Two new physics units reject selected zero-ratio transitions and malformed downshift underflow with preserved owned frame state. The existing late-force-failure unit now includes unused zero slots and still reaches the intended later failure. A new parser unit checks aliasing, empty and negative-zero slots, reverse and malformed values. All **139 library tests** pass: 117 world, 21 formats and one tools. Formatting, warnings-denied all-target Clippy, Python syntax and a six-report audit pass.

No numerical mismatch, native exception or failed final validation occurred. The inherited control guard named `zero_gear` asserted the old blanket restriction; it is replaced by a nonfinite-slot guard, while new selected-zero and underflow guards exercise the actual new contract. Debug was rebuilt after unit-test refactoring changed debug metadata, and its final numeric comparison rerun so recorded executable hashes match the final build.

Original Steam executable and all 130 traced resources remain unchanged at **11 October 00:46:56.678403 UTC**. No desktop input, settings edits or new tool installation occurred. Owner desktop authorization remains available; the expired overnight automation stays paused. No first lap or estimate increase is claimed.

## Evidence and reproduction

Private evidence under ignored `local/behavior/`:

- `vehicle-sparse-gears-20261011-debug-final-02/validation.json`, release `vehicle-sparse-gears-20261011-release-final-01/validation.json`; sibling `-source/validation.json` retains per-file hashes and exact source audit.
- `vehicle-driver-gearing-20261011-debug-final-01/validation.json` and corresponding release; sibling `-source` audits. `vehicle-driver-gearing-regression-20261011-debug-01/` and corresponding release preserve fresh F178 comparisons.
- `vehicle-gearing-library-tests-20261011.log`, `vehicle-gearing-final-audit-20261011.json`; preservation `local/scene/preservation-20261011-vehicle-gearing.json`.
- Static evidence `vehicle-gearing-initialization-aligned-20261011.json`; original surveys `20261007T2327230068908Z/9aec60.c.txt`, `20261006T1528251140557Z/a0c9f0.c.txt`, `20261010T1945339019312Z/9b29a0.c.txt` and `9b3f30.c.txt`. Whole `9AD6B0` is aligned and surveyed for next source-initialization work; it is not implemented by this milestone.

Reference image SHA-256 `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`. Rebuilt control adapter `2adddb8d417ebe3db8d1f592ed0687f8cdcce4b74c669f258dbad046707f13fa`; prepared adapter `acf370204e4e0f60e030a34e97e405a9e0d7c9f06846045bc5247bdb8abbc6ff`. Force CLI debug `d8e1530e730bfcbb30b767c47736c97721526d78353893289fd247dee55ab24e`, release `99fffd062a14143cd735a55c27ac66575c4f6e03cbf383a3f9653fca9bb59c7d`. Source CLI debug `0279c03acec2447025585a4aa7e6b3846acf17acd49e2136a516c56363af4575`, release `3538ad68e1653d5c6bb2a3d85976ebfca36145f7609a1ccbc77586d6dfad4a8a`.

Existing Rust/Cargo 1.99.0, Python 3.12.14, MSVC 14.44.35207, Developer Prompt 17.14.41, SDK 10.0.26100.0, Ghidra 12.1.4/JDK 21.0.9, Capstone 5.0.6 and Bevy 0.19.1. Strict native flags and 53-bit nearest x87 contract remain as documented in [driver preparation](VEHICLE_DRIVER_PREPARATION.md).

Build the existing control/prepared adapters and Cargo binaries `grid-vehicle-force`/`grid-body-tuning` in both profiles. Run `validate_vehicle_sparse_gears.py`, `validate_vehicle_driver_gearing.py` and prior `validate_vehicle_driver.py` into fresh private directories with explicit `--profile debug|release`. Next connect source engine/shift-threshold initialization and the authoritative physical-world schedule, then Bevy input/camera/restart/checkpoints.
