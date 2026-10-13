# Active deformation to physical collision geometry

13 October 2026. `collision_mesh::prepare_deformed_points` / `build_from_deformed_source` now join the verified sampler to source collision geometry. Whole original `9B8B70 -> 9B8890 -> 9A39D0 -> B44E80` matches complete Rust meshes; adding original `B444D0` also matches complete shape/mesh output. `BodyGeometry::rebuild_from_deformation` stages this path and replaces the old shape only after complete success.

The raw source point floor is reduced **before** deformation sampling. Each sampled point has W restored to one, is clamped against that original Y floor, receives the source car offset and is recentered using unchanged source bottom/roof. Reducing a new floor from deformed vertices would change this contract. A semantic unit test covers the distinction.

Each debug/release profile, independently:

- 68 complete active mesh cases / **639,808 bytes** exact, original/repeat/Rust.
- 68 complete active shape/mesh cases / **680,064 bytes** exact.
- Lacetti/BMW source grids, 16 bounded translation/scale/nose-warp controls per model, both zero/CD fills, plus pristine and post-impact retained player grids in both fills.
- Mesh 14 guards /38 rejection or output-preservation outcomes; shape 17 guards /47 outcomes. No native fault counts as a guard.
- Audit reconstructs complete requests and answers, verifies 425 mesh or 428 shape files per profile, nine source/capture identities and current native/Rust executables.

CM06/CM07 diagnostics keep supplied descriptors/captures private. The original sampler sees an owned complete car/grid; car/source inputs/canaries remain unchanged. Original code stays unchanged and the restored mapped sections match their saved image. No factory, allocator, live attach or original helper replacement is used. The earlier deformation-off diagnostics remain available; the release legacy shape suite passes **180 cases /999,552 bytes /64 checks**.

## Actual Donington contact validation

The source test loads purchased Lacetti NED/PSSG, constructs its grid, builds the active body shape and queries actual Donington geometry at a lowered starting pose. A controlled +.25m grid-X displacement changes the resulting contact records (16 contacts before and after; this is not a crash count). An invalid-grid rebuild preserves the exact previous contact records; rebuilding pristine nodes restores the exact original active-shape contacts. Both debug and release pass. The test leaves the car runtime at tick zero; it proves geometry/contact sensitivity and rollback, not dynamic collision-to-deformation scheduling or a driven damaged lap.

195 regular libraries (25 formats +4 tools +166 world), formatting and all-target Clippy pass. The source test uses a bounded 16 MiB worker stack and installed tools only. Current playable behavior remains rigid until the registered callback and dirty publication are integrated; retaining raw source geometry in `BodyGeometry` does not activate the rebuild automatically.

Private evidence under `local/behavior/`:

| Report | SHA-256 |
| --- | --- |
| deformed-collision-mesh-debug-20261013-final-01/validation.json | 748662e43cf1cde2749f2c3cdbd65ab76a82b11dda34da847a72212e4d704e06 |
| deformed-collision-mesh-release-20261013-final-01/validation.json | 04757b6115a85dbe2b4e35aa4ab9de3962a0503ba247e59a464d89ca5e026413 |
| deformed-mesh-shape-debug-20261013-final-01/validation.json | d0a0ef874a8413ecdcb365d6076deafebc778ca77da67ece14b6b16582f8467f |
| deformed-mesh-shape-release-20261013-final-01/validation.json | e859c2188234f98ccdcc6e87b7e3582c3fa476e7a8f3ca6fd8df2d14653ef8b9 |
| deformed-body-geometry-audit-20261013-final-01.json | 6f9d6687cd8fdd41309c405aae25cf622d30df13cb46da7b1fbd8d0f78ec848d |

Native mesh probe `a99a32512bcecd14cf8951369d1568c45efc23ece41483fb0ae1bb54184befaa`; shape probe `27ae322f310d5f93fa29f13439a3331efb4be97e7ec44de47c5da1bd00792b4a`. Rust mesh debug/release `50b382ac4f149514236604f244f3d618293284904cd76553b3cd09ffb8167caa` / `752e8d5eacde3280d16b6542c26939c05cffca43ccd6f26c4fe0398d52632cad`; shape debug/release `c60ecc11b0247ef2cb4639007088840cf0dfd78085238d9e57d0fc884af97d62` / `f587fd246aa38ec8826a08640a14bd3e4a9a2925d153b0b774950060dc806afe`.

Source contact logs: `deformed-body-geometry-debug-test-20261013-draft-01.txt`, `deformed-body-geometry-release-test-20261013-final-01.txt`. Native/whole-shape initial drafts succeeded. A Rust CLI header comparison initially used incompatible array/slice types, failed compilation and was corrected before running that diagnostic. No numerical mismatch occurred. Existing versions: Rust/Cargo1.99.0, Python3.12.14, MSVC14.44.35207/SDK10.0.26100.0, Ghidra12.1.4/JDK21.0.9, Capstone5.0.6. No installs, desktop input, Steam/settings changes, owner help or approval rejection.

Next: register and compose the physical contact-to-grid callback with its distinct CRT random state, then consume dirty grid output into wheel/shape/presentation updates transactionally. `A01960` is a shape/resource factory, not the suspension scalar consumer; its allocation/registration effects and later A03220 dependent publication must remain explicit. Do not clear C7F2 merely to pass the existing publication guard.

Pre-solver follow-up: [Deformation contact limit](DEFORMATION_CONTACT_LIMIT.md) proves the distinct static-peer 9A0C90 callback and records the verified physical scheduling. Collection preparation and runtime composition remain next.
