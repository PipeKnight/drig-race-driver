# Current GRID reverse-engineering and implementation plan

Updated: 10 October 2026, Europe/Kiev. This roadmap is grounded in [our findings](../PROJECT_LOG.md). The [original proposal](reference/GRID_2008_OPEN_SOURCE_PLAN.md) remains background reference. Resource readers and static Lacetti/Donington rendering are implemented; twelve live placement results reproduce exactly from recorded inputs. Scoped vehicle components, the supplied-contact collision callback and its response consumer have original-versus-Rust comparisons. A complete driving loop remains ahead. The [dated progress assessment](PROGRESS_ASSESSMENT_2026-10-09.md) records estimates, blockers and their uncertainty.

## Objective and language decision

Current checkpoint F155: whole optional B7B5E0 on impact-only pools joins contact bias, normal correction solve and complete pose/shape publication.708scenarios/14,452rows match original and Rust in both profiles, including5,840reused accumulators and92active handoff position changes. Next prioritize initial normal-constraint construction/main solver integration. Other/joint pools, tangent rows and live physical frames remain open. [Joined impact correction](IMPACT_CORRECTION.md). Earlier checkpoints below document dependency history.

Current checkpoint F154 supersedes the previous checkpoint below: complete pose correction/publication, conditional basis orthonormalization, linked shapes and derived matrices match896cases/3,179shapes in both profiles, including384fresh solver-to-pose handoffs. Next compose optional correction pass B7B5E0. [Physical pose comparison](PHYSICAL_POSE.md).

Current checkpoint F153: whole normal-row normalization plus iterative body response and Rust match701scenarios/13,510rows in both profiles, including5,840F152 contact corrections. The optional pass applies resulting position/angular corrections; complete pose publication is next. F152 verifies correction bias; F151 matches544authored active serial callback cases; F149 adds unique owner collection; F150 checks whole serial dispatch with callbacks guarded off. Initial physical constraint construction, tangent spans, active whole-manager state, live ownership/task ordering, pending lifetime, geometric contact detection, full initialization and synchronized vehicle updates remain unverified. [Normal constraint response](PHYSICAL_RESPONSE.md), [correction bias](CONTACT_BIAS.md), [physical handoff map](PHYSICAL_CONTACT_HANDOFF.md), [active serial wrapper](SERIAL_CALLBACK.md), [dispatch scope](SERIAL_COLLISION_DISPATCH.md), [owner collection](OWNER_COLLECTION.md), [consumer](RESPONSE_CONSUMPTION.md), [callback](COLLISION_CALLBACK.md), [vehicle update](VEHICLE_UPDATE.md). Prior solver/settling milestones remain documented in [deformation research](DEFORMATION_GRID.md).

Implement an independent engine that reads the owner's GRID resources and progressively reproduces driving, racing and the full single-player experience. First supported target: Windows x86-64. The original x86 executable is our behavior reference, not an architecture requirement for the replacement.

**Recommended default: Rust + Bevy**, retaining the owner's original choice. Keep parsers and simulation independent of Bevy so the renderer does not dictate file formats or physics. Use Bevy for application lifecycle, input and rendering integration; recover GRID-specific behavior ourselves.

C++ is a viable alternative, particularly for direct integration with an established native renderer or simulation library. It would require selecting that runtime and defining equivalent module interfaces. It does not remove the main uncertainty: understanding GRID's formats and behavior. C-like decompiler output is evidence, not ready-to-compile original C++ source. Do not switch languages solely because the decompiler prints C.

Use a small C ABI boundary if a justified native dependency is needed. Keep the main implementation in Rust unless a concrete prototype shows that a different choice materially improves the project. Rust 1.99.0 and Bevy 0.19.1 are pinned and validated locally; Cargo.lock records the resolved dependency graph.

## What we already have

- Identified Steam app 12750, build 252066, executable 1.3.0.0, x86/D3D9, with recorded hashes.
- Preserved the original install. Private reference copy: `C:\Users\debros\Tools\grid-research\grid-compat`.
- Private Cyrillic-codepage variant loads a race scene. Profile persistence, physical-key driving and AutoHotkey menu/driving control work. Owner confirms Chevrolet Lacetti at Donington Park; traced files confirm Donington route_0. Conditions are recorded in [reference race](REFERENCE_RACE.md). Its unpacked executable is available privately for static analysis.
- Useful Ghidra, x32dbg, Process Monitor, ProcDump and Python/Capstone operation demonstrated. Rust 1.99.0/MSVC native compilation and execution validated.
- `grid-formats` and `grid-tools` implement bounded PSSG/JPK framing, recursive trees, selected car/track meshes, BC1/BC3 textures, affine scene assembly, ornaments and entity startup components. Independently checked source bytes and synthetic tests are indexed in the master log; framing coverage remains 222 files.
- `grid-app` renders both Lacetti LODs, Donington surfaces, 1,985 static scenery placements and 1,045 initial entity visuals. Combined close/start-grid and overview frames pass GPU capture and visual inspection. Original PBR/shader states, cloth, damage and physics remain unimplemented; 156 entity visuals use explicitly provisional intact LODs. See [viewer](STATIC_VIEWER.md), [placements](ORNAMENT_PLACEMENTS.md) and [entities](ENTITY_COMPONENTS.md).
- Bounded collision/binary XML/progress/tuning readers now include a supported Standard body/wheel parameter prelude. Independent ground queries match 546 source-byte comparisons; the isolated original triangle primitive matches 568 cases bit for bit. The body prelude matches all 44 supported CSVs and 444 live body/wheel words. [Ground contract](GROUND_QUERIES.md), [body parameters](BODY_PARAMETERS.md).
- Read-only runtime discovery finds the verified twelve-car general manager and 48 wheel records, separately from the indexed rendering manager. The body-to-wrapper pose chain matches all twelve poses; selected velocities retain one observed discrepancy. [Live-state contract](VEHICLE_STATE.md). Original tick timing and force integration remain unresolved.
- Inventory includes PSSG, JPK, CSV and binary XML. A bounded fresh race-load trace confirms 130 resources, all matching the Steam copies, including `cars/lac` and Donington `route_0`. [Observed dependency map](REFERENCE_DEPENDENCIES.md) gives concrete targets; cached/shared resources and internal container dependencies remain unresolved.

## How decompilation contributes

Work subsystem by subsystem, alongside resource inspection and experiments in the running original. A complete decompilation of every function is not an entry requirement for the first engine milestone.

For each question:

1. **Locate the code.** Search resource paths, field names, strings, imports and cross-references in the private unpacked PE. Follow callers into loaders or consumers. Use the original crash addresses only where they help; recovery is no longer blocked on analyzing that formatter.
2. **Observe it.** Break at the suspected loader/update routine with x32dbg. Record arguments, buffers, object offsets, inputs and outputs for the selected resource or driving experiment. Label every trace with executable variant/hash and resource hashes.
3. **Recover a specification.** Reconcile disassembly, Ghidra pseudocode, actual bytes and runtime values. Document structure layouts, units, invariants, update order and unknowns. Inferred names/types remain provisional until supported by evidence.
4. **Implement it independently.** Write Rust readers or behavior from the specification. Keep raw dumps and game-derived pseudocode private; public source contains our implementation and independently created fixtures.
5. **Compare it.** Check parsing against observed buffers and independent tools where available; check simulation against repeatable original-game measurements. A plausible screenshot or vehicle model is not sufficient to claim faithful handling.

Initial code targets are resource lookup/decompression, mesh and texture loading, tuning-data consumers, input filtering, simulation timing, vehicle update, collision queries and lap/checkpoint logic. Identify concrete entry points before guessing addresses or field meanings.

Import the private unpacked PE into a new Ghidra project and analyze it using its PE sections/entry point. Keep the existing raw crash-memory project separately: it contains documented uncaptured regions and answers a narrower question.

## Milestones and exit criteria

These preserve the original plan's phase numbering. Research and implementation can overlap once their inputs are known.

| Phase | Concrete work | Exit criterion / present state |
|---|---|---|
| 0 — Identify reference | Build, architecture, inventory, hashes and failure reproduction | Identification/reproduction complete; Lacetti/Donington reference confirmed |
| 1 — Establish tools | Rust/MSVC setup, Cargo smoke build, static PE project and useful native traces | Native builds, partial PE analysis, targeted decompilation and race file tracing work; runtime loader arguments pending |
| 2 — Reference driving | Select actual car/track, fix race conditions/assists, load and drive/restart | Profile persistence and Lacetti/Donington driving confirmed; route_0 and current conditions recorded; measured repeatability pending |
| 3 — Resource readers | Trace selected race loads; resolve containers, meshes, textures, materials, tuning, collision and metadata | Car/track geometry, placements and entity startup records verified; collision/tuning and original material semantics in progress |
| 4 — Scene viewer | Bevy free camera, selected car/track geometry and essential materials | Static car/track/scenery rendering verified; live camera input and exact reference comparison pending validation; collision treated separately |
| 5 — Vehicle simulation | Recover timing/input/vehicle mechanisms and compare acceleration, braking, steering, grip and suspension | Measured tolerances agreed from reference variability, plus owner handling comparison |
| 6 — First playable lap | Input, chase camera, collision, checkpoints, timing, restart and essential sounds | One original car completes repeatable timed laps on one original track in our executable |
| 7 — Complete short race | Opponents, route/racing lines, starts, position, finish order and collision/damage behavior | A complete race finishes reliably with documented fidelity limits |
| 8 — Full single player | More content/disciplines, career, sponsors, economy, saves, presentation, replay and Flashback | Each supported mode has end-to-end evidence; feature coverage tracked explicitly |
| 9 — Releases | Clean-machine runs, input/display/device tests, packaging and compatibility matrix | Versioned engine-only releases with tested claims; Linux/macOS require native validation |

Original multiplayer compatibility is a separate protocol/service investigation. Do not assume it follows from single-player reconstruction. Full coverage remains the project objective; the first timed lap is an intermediate milestone.

## Implementation structure

The first four crates exist. Keep simulation separate from rendering; introduce another crate only when a concrete boundary justifies it.

| Crate | Responsibility |
|---|---|
| `grid-formats` | Bounded readers, format structures, synthetic fixtures; no Bevy dependency |
| `grid-tools` | Resource inspection and dependency reports |
| `grid-world` | Geometry queries and scoped vehicle/collision simulation; no renderer dependency |
| `grid-app` | Initial viewer; later input, presentation and application integration |
| `grid-sim` | Possible later extraction of integrated simulation; currently behavior components live in `grid-world` |

Introduce additional asset/render/race/audio/platform modules as their responsibilities become concrete. Do not prebuild an empty framework for every final feature.

Readers must validate counts, offsets, lengths and encodings before indexing/allocating. Preserve unknown fields during research. Determine actual units, coordinate systems and timing; do not assume visible geometry is collision geometry or that a generic vehicle solver matches GRID. Use renderer interpolation with measured simulation timing. Record tick inputs and relevant state to support reproducible comparisons and eventual Flashback; determinism must be tested.

Use original resources directly from the chosen local installation. Keep caches versioned and keyed by data/build hashes; keep our settings and saves outside the Steam directory. Public parser tests use independently created fixtures; purchased-resource integration checks remain local.

## Immediate backlog

The current priority is the first drivable Lacetti/Donington loop. The selected scene and many scoped components exist; research should now close dependencies required to connect them.

1. Recover complete vehicle initialization, main timestep, update order and authoritative state transfers. Existing isolated comparisons do not establish a complete frame.
2. Finish geometric contact-pool production, manager ownership/pending ordering and final accumulated-response application. The supplied-contact callback and scalar consumer are verified separately and with authored composition; preserve that distinction from live collision detection and scheduling.
3. Assemble a renderer-independent vehicle update and compare meaningful state sequences against the original. Calibrate reference variability before choosing fidelity tolerances.
4. Integrate controls, vehicle pose, chase camera, collision and restart into Bevy; then add checkpoints, lap timing and essential sound. The exit criterion is a repeatable timed lap, not another isolated function match.
5. Reassess progress after the first integrated driving run and timed lap. Broader cars/tracks, opponents, race rules, career, replay/Flashback and releases follow those checkpoints.

Offline work is available. Further live capture currently requires recovering a controllable GRID window after the minimized-window attempt; the owner has already authorized desktop use. No reliable completion date is established. See [progress assessment](PROGRESS_ASSESSMENT_2026-10-09.md) for the subjective full-project and first-lap estimates.

### Earlier detailed backlog — F130 checkpoint

The following records the earlier checkpoint and unresolved questions. Later collision work through F150 supersedes its descriptions of missing collision components; use the current backlog above and linked contracts before selecting work.

Current vehicle checkpoint F130: source contacts/materials, temporary force vectors, conditional tyre response and both steering branches have scoped original-code comparisons. Source toe/limit/mode, geometry, angle and all20 assistance parameters joined to whole wheel/geometry initialization and assisted steering match3,216cases in both profiles; supplied assisted mode matches2,320cases. The mutable steering producer matches2,297 authored cases, its steering-only transfer into the source pipeline matches3,216cases, and deterministic reset matches1,392cases. HDM source configuration joined to reset matches2,560cases. Actual Standard HDM selection, seven parameters, mutable producer and body copies now match84live vehicle observations /3,360fields debug/release, including bounded driving/barrier contact below the steering-effect threshold. Live nonzero wobble, pre-reset RNG, complete production transfer, device input and cadence remain pending. Original source decoders/outer constructors are not isolated by these comparisons. See [steering](STEERING.md), [handling damage](HANDLING_DAMAGE.md) and the current master log before using older backlog entries below. A complete driving loop is still pending.

1. Reference driving, profile persistence, route_0 mapping and current race conditions are recorded. Use the verified macros for further reference tests; establish repeatability and measure simulation timing before claiming precise physics comparisons. Human-readable layout label and actual display-size reconciliation remain open.
2. Bounded race-load capture and 130-file dependency list are complete. Extend coverage with a cold process or targeted loader traces only as needed to resolve cached/shared resources and container entries.
3. The private unpacked PE project is now saved and two car/track candidates decompile; confirm their runtime arguments and loader calls. See [loader research](LOADER_RESEARCH.md). Do not label static candidates as confirmed binary parsers.
4. Compiler setup, pinned Rust 1.99.0, native smoke build and minimal workspace are complete. Retain the tested build environment for further reader/viewer work.
5. Wheel geometry and collision bounds are implemented; placement matches 130 controlled cases and twelve actual return boundaries exactly from recorded inputs. Source-only initialization retains two one-ULP X differences. Outer dynamics scheduling/application matches 139 controlled cases with internal solver stubbed; supplied-impulse response (205), conditional suspension (225), point response (253), conditional wheel coupling (192) and coupled substep including floor contacts/travel stops (212) match unstubbed original code. All pass debug/release. Fresh paused/throttle contexts provide input delta values and state observations, not calibrated timing/acceleration. Pose prediction (226) and conditional two-pass finalization (200) also match whole unstubbed original code in debug/release. Initial suspension reader matches all44 Standard sources and whole supplied-record wheel initializer. Preload, runtime setup subset, source ranges/default sliders and constructor cap now reproduce all24 retained suspension/height observations with supplied geometry/dimension operands. Recover production contacts, source deformation, active tyres/input/drivetrain and main timestep. Resolve live extent/deformation/corners, render/body linkage and whole-query policy. [Vehicle update](VEHICLE_UPDATE.md), [live placement](LIVE_SPAWN_CAPTURE.md), [ground queries](GROUND_QUERIES.md) and [live state](VEHICLE_STATE.md) define evidence and limits.
6. Validate live viewer camera controls when desktop approval is available. Establish repeatable original timing/vehicle measurements before implementing or claiming faithful driving.

Estimate effort after the dependency map and first reader/viewer reveal actual complexity. Handling fidelity, collision, AI, damage and complete progression remain significant unknowns; no reliable full-project finish date is established.

## Development references

Checked for this update: [Bevy setup](https://bevy.org/learn/quick-start/getting-started/setup/) specifies Windows build dependencies; [Rust installation](https://doc.rust-lang.org/book/ch01-01-installation.html) documents toolchain setup. Record selected versions after local validation rather than copying a version from an older example project.

Detailed existing evidence: [Phase 0](PHASE0.md), [toolchain](TOOLCHAIN.md), [encoding workaround and controls](COMPATIBILITY_MEMORY_PATCHES.md). Keep [PROJECT_LOG.md](../PROJECT_LOG.md) as the single progress index.
