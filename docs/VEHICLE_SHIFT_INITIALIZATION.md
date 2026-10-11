# Source shift initialization through the selected driver — F180

11 October 2026, Europe/Kiev. The independent engine now reads the original source torque curve, maximum engine speed, second-downshift modifier and sparse gear ratios, builds automatic shift thresholds, and carries that initialized state through the selected source-track vehicle update. Every compared word agrees with unchanged original routines in debug and release. This is a further initialization boundary; a drivable world/Bevy loop remains open.

## Recovered behavior

Whole `9AD6B0` scans positive forward gear slots 1–8. Each active slot starts with an upshift threshold of 10,000. When the following ratio is positive, a bounded descending scan starts at maximum engine speed times the stored-f32 0.98 constant and decrements by one. It compares the current torque-curve sample with the following-gear sample times the stored-f32 0.99 constant. The first returned curve value is stored to f32; the second stays in 53-bit x87 precision through multiplication and comparison. The already verified `98F410` interpolation is shared with propulsion via an extended-precision internal return, preserving the engine caller's separate f32 store.

The first successful comparison stores `(firstDrivenWheelRadius * scanSpeed) * gearRatio`. First-gear downshift threshold is positive zero; subsequent active gears use the preceding upshift threshold times the stored-f32 0.95 constant. Second-gear downshift gets the source modifier. Finally, minimum divided by maximum driven-wheel radius scales **all nine forward/reverse threshold pairs**, including retained unused slots. Reinitialization can therefore rescale retained slots again; the integration test initializes once before carrying driver state, rather than rebuilding thresholds every tick.

Rust `vehicle_gearing::initialize` takes the existing owned `Frame`, checks unique 1–4 driven indices, positive finite radii, finite consumed/retained fields and finite curve lookup factors. The RPM scan is deliberately bounded to 0–20,000; extreme ratios that would generate a nonfinite lookup fail before curve indexing. Overflow after threshold work fails without publishing any frame changes. Native SF01 relocates only owned pointers, preflights the same supported range, then executes whole original `9AD6B0` and its whole original `98F410` dependency without replacement. Complete record comparison also checks that no unrelated words change.

`read_standard_shift_inputs` decodes CSV column16 to properties1E8/body17A4, columns17–48 to properties1F4/body17AC's 32-node curve, and column173 to properties5D0/body2218. These are observed direct constructor copies; no RPM-unit conversion is inserted. The modifier's zero default is tested on the parsed double before f32 storage, so a tiny nonzero value underflowing to f32 zero does not receive the default. Private CLI mode `grid-body-tuning ... shift` exports the 34 words for independent source auditing. Original decoder's optional `998E60` builds setup ranges, not an engine-curve override.

The source reader covers only this consumed subset. Other engine configuration/reset state, runtime setup scaling, complete construction and scheduler ownership remain supplied or unresolved. All 44 supported Standard files are audited, but the full driver comparisons cycle their tables through existing authored/sourced fixtures; this does not claim complete initialization or gameplay support for 44 cars.

## Verification

| Per debug/release profile | Verified work | Complete bytes | Guards |
|---|---:|---:|---:|
| Whole shift initializer, source and varied controls, including carried reinitialization | 396 initial +128 carried =524 calls | 8,061,216 | 22 |
| Initialized source tables through selected driver/contact/forces/assists/chassis | 157 initialization +332 driver calls | 5,107,488 driver bytes | 79 |
| Fresh F179 source-gearing driver regression | 332 driver calls | 5,107,488 | 79 |

Each profile independently audits **44 source files /440 ratio words /1,496 shift-input words**. Initializer comparisons change 7,416 threshold words and preserve all unrelated record words. Cases include zero/negative/varied curve nodes, zero-to-20,000 engine limits, first/top/reverse and inactive gear slots, nonuniform driven radii and retained thresholds. The guard suite includes nonfinite inputs, unbounded scan/factor, invalid radius and late overflow. A unit test verifies complete frame rollback after threshold generation has already occurred.

Full driver testing initializes each original/repeat/Rust branch independently, then carries its own output through twelve updates. Initialization runs once per sequence. Results include 134 source contacts/nonzero grips, 1,327 wheel-spin changes, 17 angular recoveries and 332 pose/velocity/chassis changes. Existing history transfer between updates remains explicitly authored; original world timing and contact ownership are not established by this composition.

All **141 library tests** pass (22 formats, one tools, 118 world), along with formatting, all-target warnings-denied Clippy, Python syntax, whitespace and six-report audit. Three Rust borrow errors were corrected before comparison. No numeric mismatch or native exception occurred. The shared prepared native adapter was rebuilt; final initialized-driver runs were repeated afterward to ensure they use the final adapter. Earlier passing drafts remain private, without promoting them to final binary evidence. Harmless guessed filename/Windows wildcard reads are recorded in the project log.

Steam executable and all 130 traced resources remain unchanged at **11 October 01:02:04.217572 UTC**. No desktop input, settings changes or installation occurred. Owner desktop authorization remains available; expired overnight automation remains paused. Completion estimates are unchanged.

## Evidence and reproduction

All captures, source values and original executable evidence stay under ignored `local/`:

- `vehicle-shift-initialize-20261011-debug-final-01/validation.json` and corresponding release; sibling `-source/shift-validation.json` retains source hashes/words and ratio audits.
- `vehicle-initialized-driver-20261011-debug-final-02/validation.json` and corresponding release; initialization/driver execution reports retain independent states.
- `vehicle-shift-regression-20261011-debug-final-01/validation.json` and corresponding release.
- `vehicle-shift-final-audit-20261011.json`, `vehicle-shift-library-tests-20261011.log`; preservation `local/scene/preservation-20261011-vehicle-shift.json`.
- Static `vehicle-gearing-initialization-aligned-20261011.json`, `vehicle-shift-source-tail-aligned-20261011.json`, `vehicle-shift-constants-20261011.json`; retained original constructor/CSV surveys linked in [source gearing](VEHICLE_GEARING.md).

Native shift adapter SHA256 `f7401cad95793905f127f149f9d019e61cfb4b52ffafb93806ad1faaca51f9a5`; rebuilt prepared adapter `f0295b2f14ae627c630b673feaa8e451b604469e6ad60dddeb089822b476e1b3`. Force CLI debug `3791f5937d5e853304f2514cf1b197d89b9b8f110b3bb8a003420079334ece42`, release `2df8ff90b4f74a0a9f0326cb0aaa5f0fd305f55f752d735677f1677f04181c60`. Source CLI debug `35dc866715835b0c859bffedef3560c50f7220b3bc49effe6e8697ba496581f4`, release `a1a0f3b137a8f42df332289157744f39b8528aad01b3bf24442a3a0d59dc16fc`. Analysis reference SHA256 remains `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.

Existing Rust/Cargo1.99.0, Bevy0.19.1, Python3.12.14, MSVC14.44.35207/Developer Prompt17.14.41/SDK10.0.26100.0, Ghidra12.1.4/JDK21.0.9 and Capstone5.0.6 reused. Native strict SSE2 and explicit 53-bit nearest x87 contract remains unchanged. Build `build_vehicle_gearing_probe.cmd`, rebuild the shared prepared adapter, and build `grid-vehicle-force`/`grid-body-tuning` in both profiles. Run `validate_vehicle_gearing.py`, `validate_vehicle_initialized_driver.py` and prior `validate_vehicle_driver_gearing.py` into fresh private directories with `--profile debug|release`.

Next connect the actual outer state/mass/input synchronization and physical-world schedule to the verified driver. Existing whole `9BD7F0`, `A03220` and `A09A80` surveys expose those boundaries; adding a second body advance after suspension prediction without checking that order would be incorrect. Complete source initialization and Bevy input/camera/restart/checkpoints remain required for the first lap.
