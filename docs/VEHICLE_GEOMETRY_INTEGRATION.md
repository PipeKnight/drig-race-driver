# Vehicle job and fresh body-contact integration

F236, 12 October 2026. The selected single-car wrapper now composes the
registered vehicle/wheel job with the [advanced-pose geometry provider](ADVANCED_GEOMETRY_PROVIDER.md).
The full stable pool accepts up to 64 records; wheel production sees only its
first four slots. Fresh world-space geometry follows the used relative wheel
prefix before grouping and solving. No original damage guard was removed.

`vehicle_world::calculate` retains its earlier four-slot contract.
`calculate_with_geometry` uses the larger pool. The source runtime exposes
`reserve_geometry_pool` before driving and `tick_with_geometry`; reset retains
the reservation. Controls, vehicle, physical world, contacts, solver and wheel
query cache commit together on success. Provider-owned side effects are outside
that transaction, so callbacks should avoid authoritative external mutation.

## Purchased-source integration test

The opt-in `source_driving_geometry_provider` test loads the owner's Lacetti and
Donington resources. It requires `GRID_GAME_ROOT` and private
`GRID_CONTACT_DIRECTIONS`; proprietary fixtures are not committed.

- A 20-slot provider path returning no contacts exactly matches the old path's
  complete vehicle/physical buffers, world heads and manager workspace for 240
  throttle/steering ticks. Remaining capacity stays between 16 and 20.
- A failing provider preserves the vehicle/world/contact/row/workspace buffers,
  wheel query cache and tick count. Reset restores the reserved initial state.
- A fresh source Lacetti mesh against an authored shallow vertical wall produces
  three contacts on the first tick, including penetration 0.0101623535. Twenty
  ticks complete; every resulting physical body differs from the matching
  no-wall control. Subsequent ticks produce no contacts after separation.
- The existing source handbrake/reverse and reset/control/rollback tests also pass
  explicitly in release. All 183 regular library tests, formatting and
  warnings-denied all-target Clippy pass; three source tests remain opt-in.

The wall and local mesh placement are selected authored integration fixtures.
The mesh is built from source points, triangles and owner offset. Its local Y
translation restores the center subtracted by source mesh preparation. This
does not verify the entire original outer car-shape factory, its live ownership
graph, Donington barrier material identity or original broadphase ordering.
The test uses a zero sheet mask, four MAXFLOAT correction extents, zero
material IDs and the existing explicit solver material. It does not establish
production backface correction or a complete source material factory.

## Original wheel-path regression

Fresh unchanged original/repeat executions and both rebuilt Rust diagnostic
profiles exactly match **300 cases / 6,150 retained world calls /
42,636,000 observable bytes per profile**. Each profile passes 22 original and
22 Rust guards. Independent report/output/hash auditing passes 32 semantic
checks and all 88 guards. This verifies the old wheel path after the shared
wrapper change, not original-executable equivalence of the new combined test.

Private reports:

- `local/behavior/vehicle-geometry-source-driving-20261012-final-01.txt`.
- `local/behavior/vehicle-geometry-wheel-debug-20261012-final-01/validation.json`,
  SHA-256 `a8e6d5b2bed1936b4729faef4206d9fc48a8fbcfde56d5048fca78233db66728`.
- `local/behavior/vehicle-geometry-wheel-release-20261012-final-01/validation.json`,
  SHA-256 `481c2a3d439b2f8266b8de5b18c0beb1f0e47e96f8cc63b7435ef01553de5907`.
- `local/behavior/vehicle-geometry-wheel-audit-20261012-final-01.json`.
- Native wheel/world probe SHA-256
  `c8d4c866d3ed0b1293d54c20256b4aca0db375e88594a4ef31f36f988a1df4ba`;
  release Rust diagnostic
  `480639db3ff9a4d49c013801addf3b1c42c35764fe5f7ca53bfc09014deee1fc`.

## Failures and remaining work

The first deep wall intersected the body near its center. Fresh solving reached
a later wheel-impact value above the source mass-times-damage threshold and
correctly returned `unsupported handling collision damage` (draft-04). A
shallow contact isolates the combined geometry/vehicle path but does not solve
heavy-impact damage. The original 9E6660 branch distributes damage through
additional helpers; those helpers and event behavior need their own validation.

Draft-05 completed the shallow 20 ticks but failed an inappropriate final-speed
assertion: a separating impulse need not retain speed after 20 suspension and
drag ticks. The final test compares the full body against a no-wall control.
No physics threshold or numerical tolerance was widened. Earlier drafts lacked
the source environment variable or called a private pose helper; both were
corrected. A raw-mesh report was initially mistaken for the attachment seed;
the correct source provenance was recovered before constructing this test.
Harmless guessed file paths and a Windows wildcard read failed without edits.

The private 44-direction preparation still reads the analyzed initializer plus
scalar words from unchanged Steam data. Steam's initializer code remains packed;
the corresponding 4,096-byte region differs from the analyzed image. Thus there
is no installation-only runtime sampling-table loader yet, and no table values
are embedded in public source.

Existing Python 3.12.14, Rust/Cargo 1.99.0 and MSVC 14.44.35207 tools were reused;
no installation or desktop input occurred. At 19:35:41.114829 UTC all 130 tracked
Steam resources and the original executable match baseline. The playable
`grid-app.exe` remains unchanged, SHA-256
`959dfe7ddcbd53b12a60b7e89f66d8c676678b48abc84335b403086a5483cf61`, with wheel-only
contacts. No owner input or approval is currently needed.

Next compose the source moving mesh with actual packed Donington chunks and
stable identities, validate source material/sheet selection, then handle the
above-threshold damage branch before enabling general playable barriers.
