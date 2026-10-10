# Source contacts and suspension feeding wheel forces — F174

10 October2026. One independent Rust call now joins actual Donington track queries, suspension preparation/substeps, wheel history/grip, surface slowdown, timers/reset/inertia/roughness, steering/gears/brakes and propulsion/forces. **120 authored initial +192 carried updates /4,799,808 complete diagnostic bytes match original/repeat/Rust exactly in both debug and release.** Earlier force-view vectors and outer history transfer remain supplied; whole A03670, complete physical-world scheduling and a drivable lap remain open.

## Composition and source ownership

`vehicle_contact_force::apply` combines [whole suspension/grip](SUSPENSION_GRIP_PIPELINE.md) with the [post-grip interval](VEHICLE_PREPARED_FRAME.md). It adapts the full body and force-view state, including the actual solver/view overlap, runs source-backed contact queries, resolves each resulting wheel material token, and supplies that material's hardness, slowdown, roughness and roll factor to the later force path. It retains computed grip, wheel history, spins and contact identity. It does not overwrite those results with a captured later-boundary body.

Source mechanics are decoded independently from the purchased binary XML. History, contact mechanics and post-grip mechanics use the same source-order token. Missing contacts stay absent. Retained history tokens are resolved separately from newly calculated wheel contact tokens. The force CLI's JF01 mode requires track and surface paths, preserves the earlier WF01/PF01/CF01/GF01 modes, and publishes the six consumed post-grip material fields from source rather than trusting the supplied material bytes. Other material diagnostic bytes are zero; the private original grip adapter separately receives the complete source initializer record.

All owned body/view/solver/diagnostic/material bindings commit only after the full call succeeds. A synthetic integration test completes all four queries and suspension/grip, then fails at later control validation and verifies complete rollback. Its first fixture had zero suspension load and legitimately produced nonfinite incoming grip before the intended control boundary. Adding authored spring/preload parameters makes the fixture reach its intended failure. The attempted first text replacement did not match the formatted array and left that test unchanged; the corrected edit passes. Simulation arithmetic was not changed to accommodate the fixture.

Suspension already performs its selected pose advance. This composition introduces no second pose advance, no extra gravity and no world step. The separate physical state bridge must respect that ownership when connected later.

## Reference and comparisons

The native reference is explicitly composed from **two isolated original adapters**: unchanged whole A00E10 against the actual packed source world/full source mechanics, then the actual F173 post-grip schedule with unchanged slowdown/inertia/roughness/control/propulsion/force helpers. The authored handoff copies the complete computed body/view into the next record and resolves only explicit source material/pointer ownership. It is not execution of whole A03670 in one original process. Both original and repeat carry their own results. Rust executes its entire join independently; it receives no original intermediate.

Inputs combine the earlier source-backed suspension/grip controls with independently authored control/drivetrain parameters before either path runs. Positions span the source Donington geometry, with varied orientation, velocity, wheel history, speed, inputs, assistance, gears and timing. Sixteen sequences each carry twelve full calls. The authored inter-frame history transfer updates previous basis and velocity caches; it does not claim the complete original frame cadence.

Every one of312 calls changes pose and velocity. There are150 retained source contacts/nonzero wheel grips and1,163 wheel-spin changes across initial and carried calls. Complete body/view/solver/material/diagnostic records match, including untouched words, not just selected outputs.

**26 paired native +26 Rust rejection controls**, plus **13 Rust framing/output controls**, pass. They cover delta bounds, contact counts/unsupported branches, pointer ownership, mass/inertia/radius, history tokens, nonfinite grip/view data, and failures after grip in mode/search/override/engine/timer validation. Native failures are ordinary exits, report unchanged executable code and create no composed final output. Existing Rust output is preserved. These counts describe the joined adapter's checks; earlier components have larger independently documented guard suites.

Fresh F173 regressions match all672 updates per profile. Rebuilt F170 source-grip CLI regressions match all312 updates, its76 rejection outcomes, and560 additional earlier preparation/suspension controls per profile. The first F170 regression run used the existing binary; it is retained as a baseline repeat, and the explicit rebuilt03/04 runs are the evidence for the current shared source adapter. **127 library tests** (20 formats,1 tools,106 world), formatting, Python AST and all-target warnings-denied Clippy pass. No numerical mismatch or original-code fault occurred.

## Private evidence and versions

- Rust: `crates/grid-world/src/vehicle_contact_force.rs`, source-token bindings in `crates/grid-tools/src/contact_surfaces.rs`, JF01 in `grid-vehicle-force`.
- Reference orchestration: `tools/validate_vehicle_contact_force.py`; existing whole-original grip and prepared probes reused without changes.
- Final: `local/behavior/vehicle-contact-force-20261010-debug-final-02/validation.json`, `vehicle-contact-force-20261010-release-final-03/validation.json`. Draft01 retains the first numerical success.
- Fresh F173: `vehicle-prepared-contact-regression-20261010-debug-01/validation.json`, `vehicle-prepared-contact-regression-20261010-release-02/validation.json`.
- Rebuilt F170: `vehicle-grip-contact-regression-20261010-debug-rebuilt-03/validation.json`, `vehicle-grip-contact-regression-20261010-release-rebuilt-04/validation.json`; prior01/02 are baseline-binary repeats.
- Tests: `local/behavior/vehicle-contact-force-library-tests-20261010.log`.
- Rust debug SHA-256 `7968fb142744feb36ce716b21358361d09baca6dbb380789e6e6437b05755b05`; release `1bd43a83ffe25781ad229a69b46883191877a056cd8c8c243a4164dcc363facf`.
- Unchanged native grip SHA-256 `902b998deb956af54220c807fb82d37302dd3f9bb7733be32bbf07739aa81d81`; prepared `ee3dae24d6ba4d85f27b8dd0788cb17e3ad5d5995bfb31ac4a7d68332f70c9b9`.

Run `tools/validate_vehicle_contact_force.py local/behavior/NEW --profile debug` or `release` after building grid-vehicle-force and the existing native adapters. Tools reused: Rust/Cargo1.99.0, Python3.12.14, MSVC14.44.35207, SDK10.0.26100.0, Developer Prompt17.14.41; Ghidra12.1.4/JDK21.0.9/Capstone5.0.6 remain installed. No new installation, desktop input or settings changes.

Steam130 resources plus executable remain unchanged at **20:44:36.497745 UTC**, `local/scene/preservation-20261010-vehicle-contact-force.json`. A guessed car-setup filename and shell wildcard search failed harmlessly; source inventory corrected those searches. The expired automation remains paused.

Next prepare force-view vectors at their actual earlier boundary, retain later steering order, and recover the intervening caller operations. Then connect actual outer history/control/physical scheduling and source-only vehicle initialization to Bevy. Conservative gear-table restrictions, inactive optional gear search/extra tyre axes, dynamic world contact production and complete-world equivalence remain limitations. Dated completion estimates remain unchanged.
