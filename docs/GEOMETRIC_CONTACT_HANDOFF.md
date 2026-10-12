# Geometric contacts into physical solving (F220)

12 October 2026, Europe/Kiev. Actual mesh-generated contacts now pass through independently implemented Rust body advance, grouping and physical solving. Original/repeated original/Rust debug and release agree on **765 scenarios / 10,513,384 complete output bytes per profile**. The255 positive geometry seeds produce499 contacts; enabled and correction-disabled variants exercise998 records per profile. **243 seeds change linear/angular velocity against empty-contact controls.** All geometry points and normals retain their exact world-space bytes.

This proves a stage-composed geometry-to-solver handoff. The preceding generator is still native reference execution, and the Rust driving runtime does not yet generate body collision contacts. These are not full AD8D70 runs with moving geometric shapes, Donington barrier tests or a completed lap.

## Contact ownership and point spaces

Actual AD6C40 analysis has two separate producer paths. The external pool at producer91C/924 stores relative points; grouping adds the advanced first-body center, including its homogeneous component. Pair records reached through producer930 store world-space points at pair20/count24 and go directly to AD6250 without that translation. Applying the external-pool rule to both would translate mesh contact points twice.

`contact_groups::calculate_with_geometry` accepts an external-contact prefix followed by a geometric suffix. It validates the prefix length before building; only the prefix is translated. Existing callers retain their complete external-pool behavior. `grouped_physical_frame::calculate_tick_with_geometry` joins the same distinction to body advance and shared solving. A unit regression uses nonzero XYZ/body W and equivalent relative/world points, checks their equality after correct grouping, detects the old extra translation and checks rejection leaves the complete input intact.

The GTG1 diagnostic reconstructs owned bodies, shapes, material pointers and an actual pair queue. The native probe executes whole AD7040, AD6C40 and B77310 (including original main/optional solving). The geometric pair and its canaries remain unchanged, unrelated producer bytes remain unchanged, and world7F4 counts exactly the geometric records. This does not run the enclosing broad-phase scheduler, regenerate geometry after advance or publish moving physical shapes.

## Original contact-slot construction

Worker allocation B19E20 constructs each contact slot using B0ECE0. Actual Ghidra0905169059562Z shows the constructor supplies the contact method table, classification flag0F, invalid body-index sentinels1C/20 and flag24. F219's zero-filled arena was sufficient to observe geometry outputs but omitted these initial values needed by the next stage.

The mesh probe now calls whole B0ECE0 for all16 slots, verifies the original method table identity before canonicalizing it to7200 and preserves every unused slot's complete constructor template. A fresh720-case original/repeat survey still produces255 positive cases /499 contacts, matches6,203,520 bytes per run, and passes19 guards. The all1,440-output audit passes with current binary/script hashes and upstream framing. Earlier zero-arena results remain historical evidence with their original limited scope; their complete records are not used for the new solver handoff.

## Source fixtures and authored controls

Each solver request is rebuilt from a checked positive native geometry result and the actual source-derived body that accompanied it. The adapter converts the attached body to the already supported cold serial solver scope: it clears explicit shape/world/group/joint ownership, enables the cold generation and supplies an unsleepable threshold. Pose, mass and inertia remain those of the seed. Geometry shape registration is not smuggled into an API that rejects it.

Each seed receives enabled, correction-disabled and empty-contact scenarios. Incoming speeds0/1/5/15 along the contact normal, solver1/2/4/8 iterations, finite shared material parameters, gravity/delta and classification metadata are authored controls, not recovered Lacetti handling or source barrier-material calibration. Points/normals/depth/limit/orientation come from each original mesh result unchanged. Body/shape/material references and pre-group indices are explicitly normalized into this diagnostic's owned identities.

The original generator and the subsequent advance/group/solve diagnostic run at separate selected boundaries. A fixed generated contact set is supplied to the latter stage; there is no claim that these two executions reproduce a complete common-time native game frame. Full moving-shape publication and fresh geometric regeneration remain necessary for runtime integration.

## Validation and corrections

All original/repeat/Rust output bytes match in each profile. Independent checks verify world-space point/normal retention, correction classification and velocity responses versus empty controls. The correction-disable classification does not suppress the main impulse; it affects the optional correction path. An initial checker incorrectly expected disabled and empty bodies to agree and failed despite exact native/Rust agreement. Corrected it using the already documented original distinction, rather than changing physics. Twelve seeds produce no velocity difference under these controls; no response is fabricated or inferred for them.

24 paired framing/nonfinite/owner/shape/material/limit/capacity/iteration/existing-output guards pass per profile. The existing-output check uses a valid request against a pre-existing file. An earlier draft used a rejected tag for that check; it was strengthened before final validation. One exploratory output decode used the wrong empty-control contact count and failed; corrected the fixture lengths. One patch context mismatch changed no files. Failed drafts remain private.

169 library tests pass (one source-dependent tools test remains ignored), formatting and warnings-denied all-target Clippy pass. Fresh legacy external-pool whole-world regressions each pass465 cases /26,058,360 bytes /26 guards, preserving the previous selected AD8D70/A727A0 scope. The current-hash handoff audit rebuilds every input from its upstream mesh result and verifies all six complete output files,510 mesh-seed uses and48 guard outcomes. Four Python AST checks pass.

Private evidence:

- `local/behavior/mesh-contact-20261012-slot-final-02/validation.json` and `mesh-contact-slot-audit-20261012-final-02.json`: current constructed-arena reference survey/audit.
- `local/behavior/mesh-contact-handoff-{debug,release}-20261012-final-02/validation.json`: final geometry-to-solver comparisons.
- `local/behavior/mesh-contact-handoff-audit-20261012-final-02.json`: current-hash/input/provenance/full-output audit.
- `local/behavior/geometric-handoff-world-{debug,release}-regression-20261012-final-01/validation.json`: external-pool regressions after rebuilding the original probe and Rust diagnostic.
- `local/behavior/geometric-handoff-tests-20261012-final-01.txt`, `geometric-handoff-clippy-20261012-final-01.txt` and adjacent build logs.
- `local/behavior/mesh-contact-handoff-debug-20261012-draft-01..02/` and `expanded-draft-03/`: initial comparisons/checker corrections; slot-final01 retains the earlier constructed-arena binary proof.

Reused Rust/Cargo1.99.0, MSVC14.44.35207 / SDK10.0.26100.0, Python3.12.14, Ghidra12.1.4 / JDK21.0.9. No installations, game input or settings changes. Native executable identities and evidence hashes are pinned in the linked private reports.

Next implement the geometric generator in Rust and join fresh contacts to retained driving. Donington's245 captured packed shapes are kind11; the kind7/11 dispatch must be established separately from the kind7/7 mesh clone. Actual Ghidra0918349172339Z recovered AE8DB0 and B184F0 for that investigation; no positive kind7/11 execution is claimed yet.

Steam130 resources and executable remain unchanged at2026-10-12T09:21:16.029954UTC (`local/scene/preservation-20261012-geometric-handoff.json`). Current native mesh probeSHA256 `a84a9eb04ad262ae80e8adcb7c752e8bc13beb94db91d265c860f4dd83551e89`; geometric-stage probeSHA256 `09b768643afdf5f31abc62834d399d1373ed4a477428167f0cb1203a9a3d6d71`.
