# Verified body and wheel tuning prelude

`grid-formats::vehicle::read_standard_body` now reads a supported body/wheel prelude independently from the purchased CSV resources. It implements the observed numeric storage/copy contract, not a vehicle force model. It accepts exactly 360 consumed fields, optionally followed by one empty trailing delimiter, and the original **Standard** wheel arrangement.

## Original chain and live comparison

CSV decoder `0x009aec60` stores columns 3..6 as f32 at record offsets `0x130..0x13c`. Concrete body initializer `0x00a0c9f0` copies those words to body `+0x15a0`, `+0x15a8`, `+0x15ac`, `+0x15b0`, and stores the f32 reciprocal of the first value at `+0x15a4`. The original later preserves/restores mass across initialization and recomputes its reciprocal. The captured reference contains 1140, 1/1140, 2000, 2150 and 450, matching the source words exactly. Later impulse-consumer recovery establishes division by principal inertia along the first three body-basis rows (F065/F068); [verified response](VEHICLE_UPDATE.md). Units and temporary override activation remain unresolved.

The decoder compares column 2 against original constants **Standard**, **Centre Wheels Only** and **6 Wheeler**, producing arrangements 1, 2 and 3. The body initializer gives these four, two and six wheels, respectively. Our prelude currently supports only Standard. It does not supply absent fields or emulate unsafe parsing/defaults for incomplete rows.

Wheel helper `0x009b6350` copies selected decoded parameters into each wheel record. For Standard, ordinals 0/1 use flag one and ordinals 2/3 flag zero. The reader retains **first pair** and **last pair** storage names. The subsequently verified [geometry chain](WHEEL_GEOMETRY.md) establishes that these are rear and front pairs, respectively; individual force-scalar meanings remain open.

| Wheel destination | First-pair CSV column | Last-pair CSV column | Supported interpretation |
|---|---:|---:|---|
| `+0xe0` | 146 | 145 | Early copy; later replaced by `.ned` radius |
| `+0x108` | 84 | 83 | Direct scalar copy |
| `+0x100` | 86 | 85 | Direct scalar copy |
| `+0x50` | 88 | 87 | Direct scalar copy; separate clamped copy at `+0x4c` omitted |
| `+0x48` | 90 | 89 | Direct scalar copy |
| `+0x5c` | 96 | 95 | Direct scalar copy |
| `+0x60` | 94 | 93 | Direct scalar copy |
| `+0x70` | 80 | 79 | Direct scalar copy |

All **384 wheel words** and **60 body words**, **444 total**, match the new Rust prelude exactly across twelve live cars. Native reports also match independent original-byte/f32 conversion checks for **all 44 supported Standard CSV files**. The `+0xe0` agreement is conditional: both reference types have equal CSV and `.ned` radii. Later investigation established that the geometric initializer overwrites this field; 13 other same-stem source pairs contain different values. This prelude models the **early CSV copy**, not the final clearance provider. See [corrected geometric initialization](WHEEL_GEOMETRY.md). All live mass/inertia tuples alone match both BMW and Lacetti, so they cannot identify a car. Adding the wheel fields uniquely resolves seven Lacetti and five BMW parameter sets, including Lacetti for the kind-zero car. This identification is stronger than the earlier warm-process resource-read list, but does not identify every loaded/rendered resource.

## Contract and verification

The reader converts validated f64 numeric lexemes to finite f32, preserving observed rounding. It requires a positive finite mass and positive finite f32 reciprocal. Unknown wheel force scalars remain positional arrays; no labels are copied from the community schema. Other columns remain in the existing lexeme-preserving tuning table.

Two synthetic tests exercise unsupported arrangements/row shapes, extra nonempty cells, missing fields, f32 overflow, underflow and singular reciprocal handling. **39 format tests plus seven world tests pass (46 total)**; workspace formatting and Clippy with warnings denied pass. These checks supplement original-resource and live-state comparisons.

CLI: `grid-body-tuning INPUT local/OUTPUT.tsv`, create-new private output only. Source: `crates/grid-formats/src/vehicle.rs`; independent integration helper: `tools/validate_native_body_prelude.py`. Private evidence: `local/behavior/native-body-prelude-validation/validation.json`, `body-wheel-tuning-link-validation.json`, `body-prelude-tests.txt`, `body-prelude-workspace-clippy.txt`.

Original initializer/helper evidence: `20261006T1528251140557Z/a0c9f0.c.txt`, `20261006T1530062088153Z/9b6350.c.txt`; constant-name evidence: `20261006T1422465120921Z/field-name-linkage.json`. No game-derived source, schemas, binaries or datasets are included in the public Rust source or fixtures.

Geometric initialization inputs are now independently implemented and live-verified: [wheel geometry](WHEEL_GEOMETRY.md). Twelve placement returns reproduce exactly from recorded inputs. [Vehicle update](VEHICLE_UPDATE.md) identifies the actual job/body/dynamics chain and validates conditional outer arithmetic. Actual suspension/tyre equations, main cadence and independently implemented driving remain ahead.

Initial Standard suspension coefficients now have a separate bounded reader, source/initializer verification and explicit runtime-change evidence. Preload, threshold and travel denominator differ after preparation; [F075 contract](VEHICLE_UPDATE.md) prevents treating these CSV initializer values as running solver inputs.

Follow-up F076/F077 implements preload, eleven source setup ranges/default sliders, the suspension/height subset of the runtime setup consumer and the constructor cap. The combined chain reproduces all 24 captured car observations /888 words in debug/release with supplied recorded geometry and dimension operand. Source-only deformation, contacts, tyres and driving remain open. Same [vehicle update](VEHICLE_UPDATE.md) documents exact evidence and limits.
