# Force-vector preparation through source contacts and forces — F175

10 October2026. One Rust call now prepares force vectors from the Standard car's own state at the recovered earlier boundary, recovers suspension scale, then runs the complete [F174 source-contact-to-force update](VEHICLE_CONTACT_FORCES.md). **120 initial +192 carried updates /4,799,808 complete diagnostic bytes agree exactly with the composed original/repeat paths in debug and release.** This removes the supplied force-vector dependency from that selected window. Earlier controls/history, source initialization and later world scheduling remain open; no drivable lap or whole A03670 equivalence is claimed.

## Actual earlier boundary

Aligned A03670 instructions call whole9B2790 at A03D55, recover context scale at A03D5A..DAA, install the world material-table pointer/index at A03DBE/DC4, and call A00E10 at A03DCA. The vector writer therefore runs before suspension changes pose and before later9B40D0 steering changes wheel coefficients. Rust reuses the verified [force-view preparation](FORCE_VIEW.md) and preserves that order; it does not recompute the vectors from the later pose or later steering.

The selected Standard wheel+34 links reach body+15A0, making their matrices the body's basis at15E0. The native adapter relocates only these owned links while calling whole9B2790 and restores their supplied numeric identities immediately. Rust takes the established Standard matrix directly. It writes all64 vector words at view190/1D0/210/250. The later force path restores its temporary point offsets, leaving these prepared vectors intact.

The caller moves a positive finite context scale toward1 by `delta * .25`, with separate float operations and overshoot clamping. Values below, equal to and above1 are covered. The world material table itself remains installed by the existing source-world fixture; the native adapter substitutes its owned source table and normalizes its pointer/index on return. Rust supplies the equivalent source query/material resolver. The original E85FD0/E85ED4 globals are not file-backed, and their live initialization is not inferred from this isolated fixture.

UF01 in grid-vehicle-force accepts source track/material paths and the existing bounded record. VU01 selects the earlier original vector boundary in the native source-grip probe. Original/repeat execute vector preparation, caller scale recovery and wholeA00E10 in one isolated adapter, followed by the unchanged F173 prepared-force adapter. Rust executes one complete call. Their interstitial schedule is authored from aligned original instructions; these two processes do not establish execution of whole A03670.

## Validation and corrected experiments

All312 calls change prepared vectors, pose and velocity.186 calls change suspension scale;158 final source contacts have nonzero grip;1,161 wheel speeds change. Explicit checks compare the float recovery result and verify that prepared vectors remain unchanged through the later post-grip stage. Forty initial inputs deliberately contain NaNs in all64 old vector slots: both paths overwrite those slots before use and agree exactly. Each implementation carries its own full state through16 sequences of12 calls with the previously documented authored history transfer.

**28 paired native +28 Rust rejection controls and13 Rust framing/output controls** pass. The vector-start suite adds nonfinite blend and zero scale checks. Its nonfinite retained-view check targets298;190 is overwritten at this new boundary and is a valid input control. A unit test starts with nonfinite old vectors, permits all four source queries and suspension/grip, then fails at later controls and verifies complete rollback of vectors, scale and all other owned state. The earlier F174 rollback fixture is shared only under cfg(test).

Fresh F174 comparisons retain all312 calls and65 rejection outcomes in both profiles. Rebuilding the shared native grip adapter preserves all312F170 updates, its76 rejection outcomes and560 earlier preparation/suspension comparisons against the retained unchanged F174 Rust contact CLI per profile. No fresh rebuild of that unchanged Rust contact CLI is claimed. **128 library tests** (20 formats,1 tools,107 world), formatting, Python AST and all-target warnings-denied Clippy pass.

A closure type-inference compile error was fixed by annotating the ray type before executing Rust. The first new native executable launch failed with Windows740 before original code ran. Installed SDK mt.exe proved that the executable had no resource section/embedded manifest despite the build's MANIFESTUAC option. Adding explicit MANIFEST:EMBED produced an inspected manifest with asInvoker and uiAccess=false; normal unelevated launch then worked. A filename-based Windows installer heuristic is a plausible explanation for the refusal, not a directly measured Windows policy decision. No elevated probe execution or approval bypass occurred.

The next numerical run already matched312 updates but inherited a rejection test for a NaN in overwritten view190. Both paths legitimately succeeded. Updating that test to retained view298, while preserving overwritten-NaN valid controls, corrected the suite. No numerical discrepancy or original-code fault occurred. A guessed newer A03670 survey path was missing; file inventory located the retained complete survey. A mixed-newline roadmap replacement initially removed historical paragraphs; review caught it and restored those paragraphs before publication.

## Evidence and reproduction

- Rust: `crates/grid-world/src/vehicle_contact_update.rs`, UF01 in grid-vehicle-force; previous JF01 and all earlier modes preserved.
- Original probe: `tools/probe_vehicle_contact_update.c`, `build_vehicle_contact_update_probe.cmd`, conditional vector start in `suspension_prepare_probe.h`. Existing tag arithmetic remains unchanged.
- Validation: `tools/validate_vehicle_contact_update.py`; shared parameterized `validate_vehicle_contact_force.py`.
- Final private reports: `local/behavior/vehicle-contact-update-20261010-debug-final-05/validation.json`, `vehicle-contact-update-20261010-release-final-04/validation.json`. Debug03 predates the additional scale/vector semantic counters;05 includes them.
- F174 regressions: `vehicle-contact-force-vectors-regression-20261010-debug-01/validation.json`, `vehicle-contact-force-vectors-regression-20261010-release-02/validation.json`.
- Rebuilt native F170: `vehicle-grip-vectors-regression-20261010-debug-01/validation.json`, `vehicle-grip-vectors-regression-20261010-release-02/validation.json`; earlier pinned native binary `local/behavior/vehicle-grip-probe-f170-pinned.exe` retained.
- Tests: `local/behavior/vehicle-contact-update-library-tests-20261010.log`.
- Static: `local/behavior/grip-force-segment-aligned-20261010.json`, `vehicle-vector-caller-constants-20261010.json`; retained complete driver survey `20261008T0136288512690Z/a03670.c.txt`. Inertia/control ABI corrections from later research still apply to that older pseudocode.
- Manifest diagnosis and inspected result: `local/behavior/vehicle-contact-update-probe-embedded.manifest`; draft01 retains the pre-execution launch failure directory, draft02 the overwritten-vector guard result.
- Native SHA-256 `40e24516b16c96fc6414011f16d68e7a63f50a0d99acc91de65393f4789be0b4`; Rust debug `0274b41e4bc00d96e0664008216829e8d2f4489b614a0845c8849be7b2171f17`; release `01625318557e28eaecfb3ca32d0abfbf4968df69b14aca9a14f1a7ed637cf097`.

Build the new native probe and grid-vehicle-force, then run `tools/validate_vehicle_contact_update.py local/behavior/NEW --profile debug` or `release`. Existing Rust/Cargo1.99.0, Python3.12.14, MSVC14.44.35207, SDK10.0.26100.0, Developer Prompt17.14.41, Ghidra12.1.4/JDK21.0.9/Capstone5.0.6 reused; no installation. Original native helpers retain strict SSE2, explicit53-bit x87 precision, executable-byte checks and owned data restoration.

Steam130resources plus executable unchanged **20:52:20.415917 UTC**, `local/scene/preservation-20261010-vehicle-contact-update.json`. No desktop input/settings changes; expired automation remains paused.

Next recover the driver stages before vector preparation and after force/inertia restoration, including actual speed publication and assists; join source vehicle initialization and physical-world scheduling to Bevy. Optional gear search, sparse real gear tables, extra tyre axes and dynamic-contact/world ownership remain bounded or unresolved. The dated completion estimates remain unchanged.
