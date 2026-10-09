# Progress assessment — 9 October 2026

This is a dated assessment of the independent Rust/Bevy GRID engine, based on the repository and private validation reports indexed in [PROJECT_LOG.md](../PROJECT_LOG.md). It describes the state after source commits `a250e47` and `226c033`. Percentages below are engineering judgments, not measured fractions of original code or promises of delivery dates.

## Overall assessment

The project has a substantial research foundation, working resource readers, a static scene viewer and independently implemented simulation components. It does **not yet have a drivable Rust build**. Progress is technically meaningful, but the project remains early when judged against the full single-player experience.

The strongest part of the work is the evidence: specifications are linked to actual resources, original runtime observations and comparisons between original routines and Rust. The weakest part is integration: many components accept supplied inputs or captured state rather than obtaining everything from one running vehicle simulation.

The next priority should be a complete Lacetti/Donington driving loop. Further reverse engineering should answer specific dependencies of that loop. Counting more recovered functions or comparison words alone would give a misleading impression of product progress.

## What is established

| Area | Observed result | Important boundary |
|---|---|---|
| Original reference | Steam build and x86 architecture identified; the private legacy-codepage copy reaches menus, loads races and retains the owner's profile | Startup/save compatibility is recovered in the private copy; the exact failing internal API remains unresolved |
| Tools and reproducibility | Rust 1.99.0, Bevy 0.19.1, native compilation, static analysis, runtime capture and keyboard macros have useful validated operation | Macro wall-clock timing is not a calibrated simulation clock |
| Resource loading | PSSG/JPK framing, selected meshes/textures, binary XML, tuning, placements and collision resources are decoded | Selected content is supported; complete car/track/material coverage is not established |
| Scene presentation | Lacetti, Donington surfaces and placed scenery render in Bevy; track collision geometry is decoded | Static viewer only; exact shaders, animation, audio and integrated chase camera remain ahead |
| Vehicle behavior | Scoped steering, wheel/suspension, body-response and deformation routines have original-versus-Rust comparisons | Supplied operands and isolated state do not constitute a complete vehicle update |
| Collision composition | The complete supplied-contact callback joins reduction, inverse pose, preparation and impact response in Rust | Geometric contact production, complete initialization, scheduling and downstream application remain integration dependencies |
| Repository and provenance | Source and research documentation are committed and pushed to the owner's repository; original assets/captures stay private | The repository is engine/research source, not a distributable copy of GRID |

Current workspace validation reports 104 passing tests, formatting checks and warnings-denied Clippy checks. Additional private differential checks cover live observations and authored boundary cases in debug and release. These are valuable correctness evidence within their stated contracts; neither test count nor exact agreement on finite fixtures proves whole-game fidelity.

Useful evidence: [reference identification](PHASE0.md), [compatibility controls](COMPATIBILITY_MEMORY_PATCHES.md), [resource formats](RESOURCE_FORMATS.md), [static viewer](STATIC_VIEWER.md), [Donington scene](DONINGTON_SCENE.md), [vehicle update](VEHICLE_UPDATE.md), [complete supplied-contact callback](COLLISION_CALLBACK.md).

## Completion estimate

**Full single-player replacement: approximately 25%, with a rough uncertainty range of 15–35%.** This includes resource loading, presentation, driving, collisions/damage, opponents, race rules, progression, saving, replay/Flashback and release validation. Original multiplayer compatibility is separate scope.

**First drivable timed lap: approximately 40%, with a rough uncertainty range of 25–55%.** This smaller milestone requires one Lacetti on Donington, integrated controls/physics/collision, a chase camera, checkpoints, timing, restart and essential sound. It is still not achieved.

These ranges are subjective uncertainty bounds, not statistical confidence intervals. A transparent weighting behind the full-project estimate is:

| Work area | Assumed share of full effort | Estimated readiness | Contribution |
|---|---:|---:|---:|
| Reference, tools and research workflow | 10% | 90% | 9 points |
| Resource formats and content coverage | 20% | 40% | 8 points |
| Rendering, cameras, animation and audio | 20% | 15% | 3 points |
| Vehicle simulation, contacts and damage | 25% | 20% | 5 points |
| Race rules, opponents and single-player progression | 15% | 0% | 0 points |
| Product integration, compatibility and releases | 10% | 0% | 0 points |
| **Total** | **100%** | | **25 points** |

The weights are planning assumptions, not measured effort. Zero readiness means no completed end-to-end capability credited in that area, even where preliminary parsing or research exists. The first-lap estimate gives much more weight to the selected scene and existing vehicle components and excludes most later game modes. Neither estimate supports a reliable completion date. Integration discoveries and broader content coverage could move the estimates downward as well as upward.

## Blockers and risks

There is **no current hard blocker preventing useful offline work**. Tool installation, GitHub access, original startup and profile persistence are no longer blocking us.

The following technical dependencies block a playable Rust milestone:

1. **A complete simulation contract.** Establish production initialization, main timestep, update order and state transfer between input, body, wheels, contacts and rendering. Some routines have scoped comparisons, but the entire frame has not been reconstructed or validated.
2. **Contacts connected to motion.** Complete geometric car/contact-pool production and connect the verified callback's effects to the authoritative vehicle state. Existing supplied-contact comparisons do not establish collision detection or a whole-frame response.
3. **An integrated application.** Connect one vehicle to input and the scene, then add a chase camera, restart, lap/checkpoint timing and essential audio. No drivable application test currently passes because this capability does not exist yet.
4. **Longer reference comparisons.** Measure acceleration, braking, steering and contact behavior over synchronized sequences. Single-call agreement does not establish numerical drift, trajectory fidelity, deterministic replay or the original feel.

There is also a temporary live-capture limitation: GRID was last reported minimized, and automatic window recovery failed. The most recent wider callback capture timed out with zero entries; no input macro was sent during that attempt. The last successfully inspected scene was paused. Further live capture requires a controllable game window, but offline analysis and implementation can continue without an owner answer. See [callback attempt history](COLLISION_CALLBACK.md).

Remaining scope risks include exact material/shader behavior, more cars and track types, AI and race rules, damage presentation, career/progression, audio, replay/Flashback and clean-machine compatibility. Two source-only placement X values retain a one-float-step discrepancy; this is an unresolved fidelity issue, not an established cause of a gameplay failure. The controlled RNG used for collision comparisons is not evidence that the original live thread seed has been recovered.

## Next checkpoints

1. Map the missing vehicle initialization, timestep and contact-production dependencies; retain original routine comparisons where they answer those questions.
2. Assemble a renderer-independent, repeatable vehicle update using locally loaded reference resources. Test meaningful sequences, not just individual calls.
3. Integrate that state into Bevy with controls, chase camera, collision and restart. Record the first successful Rust driving run as a new milestone.
4. Add checkpoints/timing and essential sound; complete a timed Donington lap, then compare bounded original/Rust driving sequences and document fidelity limits.
5. Reassess scope and percentages after the first driving loop and after the first timed lap. Only then expand toward opponents, a complete race and broader single-player coverage.

Keep [the implementation plan](IMPLEMENTATION_PLAN.md) as the active roadmap and [the project log](../PROJECT_LOG.md) as the chronological evidence index. Future assessments should report observable milestone changes and changed assumptions rather than automatically increasing the percentage with each commit.
