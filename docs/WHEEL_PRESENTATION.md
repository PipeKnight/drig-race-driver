# Source wheel presentation (F211)

12 October 2026, Europe/Kiev. The Bevy driving prototype now steers, spins and displaces the Lacetti's rims, tyre surfaces and brake discs from authoritative wheel state. This is an authored presentation mapping, not equivalence to GRID's complete renderer. It does not add camber, deformation, blur, suspension-link animation or body collision.

The installed PSSG has separate `x0_` and `x1_` wheel/tyre/disc joints. Their `bl`, `br`, `fl`, `fr` suffixes map to the source NED initializer's rear positive-X, rear negative-X, front positive-X, front negative-X order. The selected LOD0 has sixteen wheel draws: one rim, two tyre materials and one disc at each corner. Steering-wheel/interior and suspension meshes retain their rigid presentation. Static track/object/collision parts are never bound as moving wheels.

`Runtime::wheels()` reads anchor XYZ, initialized suspension axis, displacement clipped by the same upper bound used in wheel preparation, steering cosine/sine and accumulated phase. The fields are source wheel0/+20/+24/+40/+4C/+110 and context+50. Physics already integrates phase in the verified propulsion schedule; presentation never integrates another phase or estimates steering from keyboard input.

The authored transform translates the shared physics anchor along its suspension axis, applies local-Y steering and local-X wheel phase, then preserves each mesh's original bind offset. Its local axle agrees with the tyre frame's normalized `[cos,0,-sin]`. NED and PSSG pivots differ by less than3mm for this car; those source differences remain, rather than snapping all wheel surfaces to a single replacement mesh. The car's constructor translation is applied before this mapping and the authoritative body pose afterward.

The explicit purchased-resource Bevy test verifies all sixteen bindings, source pivot proximity, finite unit-determinant rotations, invariant shared pivot/axle, preserved rim/tyre/disc bind distances, actual motion on all four corners, deterministic visual reset, and unchanged complete car/body state after every presentation read over160 powered left-steering intervals. The keyboard/pause/reset/failure-recovery test also passes. Default library checks pass163 tests; source-dependent tests remain opt-in with `GRID_GAME_ROOT`. Formatting and all-target warnings-denied Clippy pass.

Private evidence:

- `local/scene/source-wheel-joints-20261012.json`: read-only source joint/transform/material survey and source hash.
- `local/behavior/wheel-presentation-test-20261012-draft-01.txt`: initial passing integration test; later expanded with source pivot proximity.
- `local/behavior/wheel-presentation-bevy-tests-20261012-final.txt`, `wheel-presentation-libtests-20261012.txt`, `wheel-presentation-clippy-20261012.txt`, `wheel-presentation-build-20261012.txt`.
- `local/scene/wheel-presentation-20261012/left.png` and `left-render.txt`: inspected160-tick left-steering render, saved normally. Rendered pose/velocity matches the prior source-only left trajectory at printed precision. A still image supports placement, not temporal animation fidelity; the state/transform integration test provides the motion checks.
- `local/scene/preservation-20261012-wheel-presentation.json`: all130 reference resources and original executable unchanged at02:48:49.260657UTC.

Release viewer SHA256 `d08d739c13a16c8b8f36906ff9692e5219b4b4ecbd1b3ae8057788e95e878f8f`; inspected PNG SHA256 `d0514a52dcf2e86b2b056408a4c8ed51bbe7dda0d6066bb13081009ed40da945`. Existing Rust1.99.0/Bevy0.19.1/Python3.12.14 tools reused; RTX4070/Vulkan driver616.92. The existing gilrs0x80070057 gamepad event failure recurred; scripted/keyboard execution remained successful. No desktop input or game settings changed.

Research continuation: body-shape registration and narrow-phase contact generation are the next physical integration gap. Existing retained-world proofs explicitly had empty geometric-shape queues. A fresh Ghidra survey initially failed before analysis because the sandbox denied its existing Java settings-cache write; the normal approved escalation succeeded, preserving both logs. This is a launcher failure, not a game failure or missing JDK. No tool was installed. Source geometry factory and dispatcher evidence remains private and is not yet a runtime body-collision claim.
