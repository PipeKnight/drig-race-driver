# Standard wheel geometry from purchased resources

The independent Rust reader now combines named `.ned` binary XML wheel geometry with positional CSV fields to reconstruct the supported Standard four-wheel layout. It matches **576 selected live f32 words across all twelve reference cars**, including the source axle coordinates/radii, body offset, initialized four-wheel positions/radii and each wheel record's base vector. This supplies geometric inputs for ground/spawn research; it does not implement suspension forces.

## Original resource and consumer chain

Original `0x009efb30` reads the unique `WHEELS` element and its `m_rear_x/y/z/r` and `m_front_x/y/z/r` attributes into a temporary description at `+0x1054..0x1070`. Numeric helper `0x00a67b60` parses a present named attribute as a float. Original body construction `0x00a0e880` copies the description to body `+0xd240`, placing these words at `+0xe294..0xe2b0`.

The constructor builds mirrored X pairs at body `+0x14e0..0x1510`: **rear positive X, rear negative X, front positive X, front negative X**. Selected-coordinate subtraction helpers `0x004b1cb0` → `0x004b1a90` subtract CSV column **194**, decoded at tuning record `+0x3e0`, from each wheel's Y. This field is deliberately positional; its final physical interpretation remains open. Geometric radii are copied rear/rear/front/front to `+0x1520..0x152c`.

Initializer `0x00a0c9f0` then uses CSV column **7** (record `+0x150`) and **8** (`+0x154`) to construct the additive local offset at body `+0x1630`. With the four pre-offset wheel points `p0..p3`, geometric radii `r0..r3`, height fraction `h`, longitudinal fraction `l` and vertical bias `b`:

- X offset is the negative average of the four X coordinates.
- The transverse span is `((p0.x-p1.x)+(p2.x-p3.x))/2`. Y offset is the negative of `span*h+b`, minus the average of each `point.y-radius`.
- Rear and front Z centers are the respective pair averages. Z offset is the negative of `rear_z+(front_z-rear_z)*l`.
- The offset has homogeneous component zero. Adding it to each point produces the stored base vectors, whose homogeneous component is one.

The implementation retains the observed f32 operation grouping and signed zero. It checks finite inputs/results, supported Standard row shape, positive radii and missing/duplicate/ambiguous geometry. Other axle arrangements remain unsupported. The **first-pair tuning flag initializes rear wheels**, and the last-pair flag initializes front wheels, based on this constructor/geometry chain. Direct scalar meanings remain unassigned.

## Comparison and limits

`tools/validate_wheel_geometry.py` independently decodes binary XML and CSV values, reconstructs rounded f32 arithmetic and compares every native CLI report. **43 same-stem Standard `.ned`/CSV pairs match**. `cars/vip/vie.csv` has no same-stem `.ned`; variant-to-base association is unverified and explicitly skipped. This does not reduce the separate body prelude's 44-CSV coverage.

The live comparison validates twelve cars against the already uniquely resolved seven Lacetti/five BMW parameter fingerprints:

| Captured words per car | Count |
|---|---:|
| Named source axle coordinates and radii | 8 |
| Additive offset | 4 |
| Initialized four position vectors | 16 |
| Initialized four radii | 4 |
| Wheel-record base vectors | 16 |
| Total | 48 |

All **12 × 48 = 576 words** are bit-identical. Original live agreement is established for these two car types; independent decoder/native agreement for other source pairs is not a live handling claim.

**7 October correction:** the CSV helper makes an early copy to wheel `+0xe0`, but the later loop in `0x00a0c9f0` overwrites it with the supplied `.ned` geometric radius, also stored at `+0xf4`. The constructor supplies the radius array at body `+0x1520`. Both initialized fields therefore use the `.ned` radius for this constructor. The previous claim that final clearance retained a separate CSV operand was incorrect. It was hidden by equal CSV/NED values in the two live reference car types. **13 of the 43 same-stem Standard source pairs have unequal radius values**; those counterexamples are preserved in `local/behavior/native-car-collision-validation/validation.json`. All **96 live words** at the two wheel destinations match `.ned` radii. The synthetic layout test now deliberately uses unequal CSV and geometric values. The prelude retains the early copy explicitly; callers must use `StandardWheelLayout::radii` for initialized clearance.

Two synthetic tests cover analytic mirroring/offset/vertical-bias behavior and malformed/ambiguous/nonfinite/overflow inputs. **41 format and seven world tests pass (48 total)**; workspace formatting and Clippy with warnings denied pass.

Source: `crates/grid-formats/src/vehicle.rs`. CLI: `grid-wheel-layout INPUT.ned INPUT.csv local/OUTPUT.tsv`. Private evidence: `local/behavior/native-wheel-geometry-validation-v3/validation.json`, `general-vehicle-manager-20261006T1556387146367Z.json` (171/171 successful blocks), `wheel-geometry-tests.txt`, `wheel-geometry-workspace-clippy.txt`. Original targeted surveys: `20261006T1548417753432Z/`, `20261006T1551181121730Z/`, `20261006T1552219674102Z/`, `20261006T1554252648556Z/`, `20261006T1555200150624Z/`; instruction evidence: `body-geometry-coordinate-instructions.json`.

Failed integration runs are retained in `native-wheel-geometry-validation/` and `...-v2/`: the first assumed all tuning files had same-stem geometry; the second let blank CSV rows exclude otherwise valid files. Corrected explicit missing-pair reporting and blank-row filtering before the successful complete check. An ad hoc Python read initially omitted the tools module search path; rerun corrected that diagnostic setup. No original resources or settings were modified.

Next: recover the original ground-aligned frame and ordered wheel-clearance pass, then compare a fresh race-start pose before integrating placement into the viewer.
