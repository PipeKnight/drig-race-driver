# GRID (2008): Open-Source Engine Research and Implementation Plan

Prepared: 5 October 2026  
Status: research report and proposed implementation plan; no game binaries have been inspected and no tools have been installed as part of this report.

## 1. Project objective

Build an independently implemented, open-source engine for **Race Driver: GRID (2008)** that reads the player's legally obtained game installation and reproduces the original driving and racing experience on modern computers.

The owner already has a purchased Steam copy. Preserving what makes GRID feel like GRID is a primary requirement: vehicle handling, opponents, cameras, race rules, damage, atmosphere, and eventually career progression and Flashback. Modern compatibility should improve without silently changing those mechanics.

**Recommended implementation:** Rust, Bevy, and its wgpu rendering backend, with custom GRID asset readers and vehicle simulation. Use REA and Ghidra for focused research, and observe the original on a Windows reference machine.

**First playable milestone:** one original car, one original track, a chase camera, controller input, track collision, and a timed lap. This is a milestone toward the full game, not the final scope.

Planning assumptions:

- First supported release: Windows x86-64. Develop platform-independent components on macOS or Linux as convenient.
- Linux and macOS are later release targets; start build checks early so platform dependencies remain visible.
- Players supply original resources locally. Distribute our engine, tools, documentation, and independently created test data.
- Exact Steam build, executable architecture, installed assets, failing hardware, and failure symptoms remain unverified.

## 2. What Benilla actually uses

[Benilla](https://github.com/samwhosung/benilla) is Sam Sung's independent World of Warcraft 1.12.1 client. Its README says it reads a player's installation and uses the original protocol, with independently written format readers and interface machinery. It describes the client as playable; that is the author's status claim, not a result independently tested for this report. [Benilla README](https://github.com/samwhosung/benilla#readme).

### Verified technology

| Component | Evidence in the repository | Relevance to GRID |
|---|---|---|
| Rust and Cargo workspace | Root manifest; Rust 2021 edition | Separate resource readers, simulation, application, and tools into crates. |
| Bevy 0.18.1 | Root workspace dependency | Engine framework; this is Benilla's inspected version, not a requirement to copy its version. |
| wgpu 27 | Root workspace dependency | GPU abstraction; normally obtain the compatible backend through Bevy. |
| Kira audio with local patches | Root manifest and application dependencies | Candidate audio mixer, subject to investigating GRID's audio formats. |
| Patched Lua 5.1.5 source | Root manifest comments and local patch | Needed for WoW compatibility; no evidence that GRID needs this. |
| Avian 3D 0.6 | Application manifest | Benilla uses it for character collision; it does not provide GRID vehicle handling. |
| bevy_egui; Tracy and Chrome trace features | Application manifest | Useful precedents for development UI and measurement. |

Sources: [workspace manifest](https://github.com/samwhosung/benilla/blob/main/Cargo.toml), [application manifest](https://github.com/samwhosung/benilla/blob/main/crates/benilla-app/Cargo.toml).

### Verified development workflow

Benilla's method starts with reference behavior, implements it in modern code, and validates it with measurements. It keeps the installed game read-only and separates local state. Its contribution process expects AI-assisted work and uses Rust formatting, Clippy, tests, and live checks; tests requiring game data must disclose missing data rather than turn a skipped check into evidence of compatibility. [Method](https://github.com/samwhosung/benilla/blob/main/docs/METHOD.md), [contribution guide](https://github.com/samwhosung/benilla/blob/main/docs/CONTRIBUTING.md).

**Unconfirmed:** the inspected primary sources do not establish the author's exact AI model, coding-agent CLI, decompiler, or MCP bridge. We should not attribute REA, IDA Pro, Ghidra, or a specific Claude model to Benilla without additional primary evidence.

The transferable lesson is the architecture and measurement process. WoW also has an external server implementing much of its game logic; GRID requires us to implement local vehicle dynamics, opponents, race rules, and progression ourselves.

## 3. Recommended engineering approach

Combine resource-format research, observation of the running game, and targeted binary analysis. Write a behavior specification for each subsystem, then implement and compare it.

| Approach | Useful result | Place in this project |
|---|---|---|
| Original-game compatibility fixes | A usable reference and immediate relief for the owner | Early research work; does not fulfill the open-source-engine objective. |
| Full C/C++ reconstruction | Detailed recovery of original implementation behavior | Use selectively when a subsystem cannot be understood otherwise. |
| Independent Rust engine loading original resources | A maintainable engine with modern platform support | Main deliverable. |

Decompilation produces imperfect pseudocode. Resource parsers, data definitions, runtime behavior, and disassembly must agree before an inferred mechanism becomes a confirmed requirement. Preserve uncertain fields and functions as unknowns.

Start with loading and driving. Expand fidelity and content once the core loop works. Avoid making a complete decompilation of every original function a prerequisite for the first playable build.

## 4. Tools to prepare

### 4.1 Essential development stack

| Tool | Purpose | When needed |
|---|---|---|
| [Rustup, Rust, Cargo](https://rustup.rs/) | Build the engine and command-line tools; install rustfmt and Clippy | Start |
| Git and a repository host with issues and CI | Versioned code, research tasks, reproducible builds | Start |
| Existing AI coding assistant | Help investigate bounded questions, implement readers, and review changes | Start; no particular subscription or model is required by the design |
| [Bevy](https://bevy.org/) | Windowing, ECS, rendering integration, input, and application lifecycle | First viewer |
| [wgpu](https://github.com/gfx-rs/wgpu) | Modern graphics backends; use Bevy's compatible version | Through Bevy; direct use only where custom rendering needs it |
| Python 3 | File inventory, hashes, exploratory parsers, and measurement scripts | Start |
| Platform build tools | Windows MSVC/SDK; macOS Xcode command-line tools; Linux compiler/linker and required system libraries | Before building on that platform |

Bevy provides extensible 3D rendering and cross-platform facilities. These capabilities are a starting point; they do not establish that GRID's resources or shaders will work without custom code. [Bevy features](https://bevy.org/).

### 4.2 Research and original-game observation

| Tool | Purpose | Priority and constraint |
|---|---|---|
| [Ghidra](https://github.com/NationalSecurityAgency/ghidra) | Static analysis: imports, strings, functions, references, structures, and decompilation | Essential; verify the executable's actual architecture |
| [REA](https://github.com/morluto/rea) | Connect the AI assistant to evidence-oriented binary analysis | Recommended on a supported macOS/Linux host; validate the target before relying on it |
| [ImHex](https://github.com/WerWolv/ImHex) | Examine resource headers and define binary patterns | Early asset work |
| [EGO Engine Modding tools](https://github.com/EgoEngineModding/Ego-Engine-Modding) | Reference readers and editors for EGO formats | Early asset work; verify GRID 2008 variants individually |
| [Process Monitor](https://learn.microsoft.com/en-us/sysinternals/downloads/procmon) | Observe files and configuration accessed when launching or loading a race | Windows reference work |
| [x64dbg / x32dbg](https://x64dbg.com/) | Debug the original process, inspect crashes and runtime values | Select x32dbg for a confirmed 32-bit executable; x64dbg for 64-bit |
| [apitrace](https://github.com/apitrace/apitrace) | Trace the original graphics API and inspect state | Graphics research; match tracer architecture to the game |
| [GridMTFix](https://github.com/xatornet/GridMTFix) and [DXVK](https://github.com/doitsujin/dxvk/blob/master/README.md) | Investigate known compatibility workarounds and obtain a running reference | Conditional; review changes and record each variant tested |

Ghidra supplies static analysis across several platforms and executable formats. Process Monitor supplies Windows file, registry, and process/thread observations. apitrace documents Direct3D tracing and architecture matching. [Ghidra](https://github.com/NationalSecurityAgency/ghidra), [Process Monitor](https://learn.microsoft.com/en-us/sysinternals/downloads/procmon), [apitrace usage](https://github.com/apitrace/apitrace/blob/master/docs/USAGE.markdown).

### 4.3 REA compatibility details

The current raw README requires Ghidra **12.1.4** and a full 64-bit **JDK 21** for its Ghidra adapter, with a matching native decompiler on macOS. It supports macOS/Linux analysis; Windows Ghidra integration is currently unavailable. Older indexed pages describe a Windows experimental mode, so use the current README and actual diagnostics. Node.js/npm are also required. [Current REA README](https://raw.githubusercontent.com/morluto/rea/main/README.md).

Pin the selected release and record it. Run `rea doctor --json` and `rea providers --json`, then verify analysis of the actual `grid.exe`: function listing, strings, references, and decompilation of one small function. Successful installation alone is insufficient.

If the adapter rejects the binary, use standalone Ghidra and export targeted analysis while resolving the bridge limitation. REA organizes investigation; it does not automatically recover the original source or produce a complete Rust game.

### 4.4 Tools to add when justified

| Tool | Proposed use | Limitation |
|---|---|---|
| [Avian 3D](https://github.com/avianphysics/avian) | Prototype rigid bodies, collision queries, and contacts | Evaluate it against a vehicle test scene; implement GRID-specific suspension and tire behavior ourselves |
| [Kira](https://github.com/tesselode/kira) | Engine audio mixing | Original sample/container decoding and engine-sound rules require separate research |
| [RenderDoc](https://github.com/baldurk/renderdoc) | Inspect our renderer's Vulkan or D3D11/12 frames | It does not directly support D3D9 or Metal; use apitrace for the original D3D9 path |
| [SDL3 haptics](https://wiki.libsdl.org/SDL3/CategoryHaptic) | Candidate steering-wheel force-feedback abstraction | Test each intended device/OS; controller rumble is not equivalent to wheel force feedback |
| Blender | Inspect geometry exported by our own resource tools | An inspection aid; not a substitute for native-format readers |
| Profiler integrated with our chosen Bevy version | Locate CPU/GPU and simulation bottlenecks | Add after we have an executable workload and baseline |
| IDA Pro or Hopper | Alternative static-analysis tools | Optional commercial tools; evaluate a concrete need before purchasing |

RenderDoc's support matrix is explicit about D3D9 and Metal. Use platform GPU tools for the macOS Metal renderer. [RenderDoc API support](https://github.com/baldurk/renderdoc#api-support). SDL documents a haptic subsystem for force-feedback devices. [SDL haptics](https://wiki.libsdl.org/SDL3/CategoryHaptic).

## 5. GRID-specific resource research

Existing EGO tooling provides a useful starting point:

| Candidate format | What the tool authors document | What we must establish locally |
|---|---|---|
| PSSG | Models and textures | GRID's geometry layouts, materials, texture encoding, transforms, and LODs |
| `database.bin` | Events, AI skill levels, vehicles, and teams | Exact schema and which records control the selected race |
| CTF / CSV | Car tuning data | Which variants GRID uses, units, field meanings, and how the simulation consumes them |
| JPK, including `raceload.jpk` | Physics, damage, and loading data | Which files exist in this installation and how their contents connect to the car/track |
| Binary XML, LNG, TPK, PKG | Conversion to common formats | Actual presence, version, encoding, and required subset |
| Audio containers | Not established for this installation | Inventory signatures first; identify decoder and event logic from evidence |

Source: [EGO Engine Modding README](https://github.com/EgoEngineModding/Ego-Engine-Modding#readme). This suite spans several generations of EGO games. Its ERP and NeFS support does not prove that GRID 2008 uses those formats. A tool that exports textures does not prove that we can reconstruct complete track geometry or collision.

For each field, record byte offset, type, endianness, version, observed values, proposed meaning, source, and confidence. Preserve raw unknown fields during exploration. Reject invalid offsets, counts, and lengths with clear errors.

## 6. Proposed engine structure

Keep file-format logic and simulation independent of the renderer where practical:

```text
crates/
  grid-formats/       bounded readers for verified resource formats
  grid-assets/        installation discovery, resource lookup, local cache
  grid-sim/           vehicle dynamics and fixed-step simulation
  grid-race/          race rules, opponents, scoring, progression
  grid-render/        materials, lighting, cameras, effects
  grid-audio/         decoded resources, sound events, mixing
  grid-platform/      input, wheels, paths, platform integration
  grid-app/           executable and menus
  grid-tools/         inspect, validate, export, compare
docs/
  research/          evidence-backed subsystem notes
  formats/           resource specifications
  compatibility/     tested build/hardware combinations
tests/
  synthetic/         independently created parser and simulation fixtures
```

These names are proposals, not implemented components. Start with only the crates needed for the first milestone.

Separate original installation data, research captures, and generated caches from public source. Give caches a schema version and key them by resource/build hashes. Keep new settings, saves, and logs outside the original installation.

Use a fixed simulation step, renderer interpolation, and recorded simulation-tick inputs. Determine the original timing and smoothing behavior experimentally; do not assume a particular original tick rate. Deterministic replay and full state snapshots require explicit design and verification, particularly before implementing Flashback.

## 7. Step-by-step implementation plan

Each phase produces a usable artifact and has an exit criterion. Parser research, runtime observation, and implementation can overlap once their inputs are established.

### Phase 0 — Identify the exact reference

1. Locate the purchased Steam installation and record its build and patch level.
2. Hash `grid.exe` and the selected resources; inspect PE architecture and imported libraries.
3. Record OS, CPU, GPU, driver, display resolution/refresh rate, and input devices.
4. Reproduce the current failure: launch crash, race-load crash, incorrect speed, input problem, or another symptom.
5. Select one car/track pair present in the installation and define reproducible test conditions.

**Deliverables:** installation manifest, failure reproduction, and reference test recipe.  
**Exit:** we can identify the exact binary and reproduce the reported problem. A different distribution is a separate reference.

### Phase 1 — Prove the research toolchain

1. Prepare Rust/build tools and static analysis on the development host.
2. Validate REA against the actual executable; keep standalone Ghidra available.
3. On Windows, prove a Process Monitor capture and an appropriate debugger session.
4. Confirm the graphics API and test a short apitrace capture if graphics research is needed.
5. Record working tool versions and commands in a setup document.

**Deliverable:** repeatable analysis of one known binary function and one observed resource load.  
**Exit:** tools return useful evidence for this game, not just a successful setup screen.

### Phase 2 — Establish a usable original-game baseline

1. Preserve the original installation and saves; use a separate working copy for changes.
2. Review relevant GridMTFix configuration changes and graphics-wrapper options.
3. Try one change at a time, recording files changed and before/after behavior.
4. Keep modified and unmodified reference runs separately labeled.
5. Capture the selected race and driving tests once the original is usable.

GridMTFix documents CPU-worker and hardware-detection changes, plus a DXVK addon for high-resolution/high-refresh startup issues. Its Steam workflow also modifies the executable's protection; do not treat the entire script as a harmless configuration-only fix. Choose the relevant changes deliberately rather than executing the bundle without inspection. [GridMTFix documentation](https://github.com/xatornet/GridMTFix#readme).

**Deliverable:** a documented running reference or a precise remaining blocker.  
**Exit:** comparable driving observations are available. If the current machine remains unusable, obtain observations from a documented alternative machine; continue format research independently.

### Phase 3 — Inventory and understand the resource dependency chain

1. Group resources by signature as well as extension.
2. Find the files accessed for the selected car and track.
3. Compare existing EGO readers against these files.
4. Resolve geometry, textures, material references, tuning data, track collision, and race metadata.
5. Implement bounded readers and a command-line inspector for the smallest required subset.

**Deliverables:** format notes, dependency graph, and parser tests.  
**Exit:** every file needed by the selected scene is identified or explicitly unresolved; parsers expose verified fields and report unsupported data.

### Phase 4 — Render one original track and car

1. Create a Bevy viewer with free camera and diagnostic overlays.
2. Load geometry, indices, transforms, normals, texture coordinates, and textures.
3. Implement essential materials; inspect lighting and transparency against the reference.
4. Establish world scale, coordinate convention, car pivot, wheel locations, and track placement.
5. Investigate collision geometry separately from visible meshes.

**Deliverable:** interactive viewer loading resources from the user's installation.  
**Exit:** the chosen track and car render correctly enough to compare landmarks and dimensions; missing materials and collision are listed explicitly.

### Phase 5 — Recover and reproduce handling

1. Identify the selected car's tuning data and the functions that consume it.
2. Measure acceleration, braking, steering response, speed-dependent handling, suspension, grip loss, and recovery under repeatable conditions.
3. Distinguish raw steering input, smoothing, assistance, tire response, and camera motion.
4. Prototype rigid-body/contact behavior, then implement verified suspension, drivetrain, and tire mechanisms.
5. Compare measured outputs and subjective feel using the same car, surface, assists, and starting conditions.

Begin on a synthetic test surface to debug the solver, then repeat on the original track. A generic physics-library vehicle is a scaffold; tuning it until it seems plausible does not establish GRID fidelity.

**Deliverables:** handling specification, measurement harness, and drivable car.  
**Exit:** agreed error tolerances are met for instrumented tests, and the owner accepts the handling in a direct comparison. Set tolerances after assessing reference repeatability.

### Phase 6 — Complete the first playable time trial

1. Add chase/cockpit cameras as required by the chosen comparison.
2. Implement input calibration, controller bindings, pause, restart, and recovery/reset behavior.
3. Add checkpoint validation, start/finish logic, lap timing, and a minimal HUD.
4. Add essential engine, tire, and collision sounds once their resource path is known.
5. Record inputs and expose diagnostics for failed laps or simulation divergence.

**Deliverable:** one car completing a timed lap on one original track.  
**Exit:** repeated launch, drive, finish, and restart runs work on the target Windows machine. This is the first playable milestone, with known differences disclosed.

### Phase 7 — Build an actual race

1. Recover route/racing-line information and opponent parameters.
2. Implement grid starts, opponent control, overtaking, avoidance, and recovery.
3. Add position tracking, finish order, race transitions, and relevant penalties.
4. Implement collision damage and its effects for the selected discipline.
5. Compare opponent behavior and event results with the reference.

**Deliverable:** a complete short race with AI opponents.  
**Exit:** races finish consistently; collision/recovery cases and race rules have evidence-backed behavior.

### Phase 8 — Expand to GRID's full single-player experience

1. Expand verified readers to additional cars, tracks, and resource variants.
2. Recover remaining disciplines, event rules, opponent types, scoring, and session transitions.
3. Implement career, teams, finances, sponsors, progression, and persistent saves to the extent confirmed by reference research.
4. Reconstruct menus, presentation, audio behavior, replay cameras, and effects.
5. Implement Flashback using a bounded history of complete relevant simulation state, including opponents, damage, race state, and randomness.

**Deliverables:** a feature/content coverage matrix and progressively complete single-player builds.  
**Exit:** each claimed supported mode has complete end-to-end playthrough evidence. A time trial or a generic race must not be advertised as the complete game.

### Phase 9 — Validate modern compatibility and ship the engine

1. Test clean-machine installation, game-data selection, missing-resource errors, and upgrade behavior.
2. Validate high-core-count CPUs, multiple GPU families, display changes, alt-tab, high DPI, and high-refresh monitors.
3. Exercise keyboard, controller, and the specific wheels/pedals/force-feedback devices intended for support.
4. Check frame pacing, memory use, long races, replay/Flashback, save/load, and device reconnection.
5. Package versioned Windows builds with source, license notices, diagnostics, and a compatibility matrix.
6. Bring Linux/macOS to the same milestone with native testing; document backend and peripheral differences.

**Deliverable:** a reproducible engine release with explicit supported hardware, build/data versions, modes, and known issues.  
**Exit:** published compatibility claims are supported by native test runs; merely cross-compiling is insufficient.

### Phase 10 — Extend after the core is established

Develop mod APIs, additional rendering options, accessibility improvements, or new networking as separate changes with explicit behavior decisions. Original multiplayer compatibility needs its own protocol/service investigation and is not assumed by this plan. Track it as a separate remaining component of full-game coverage.

## 8. Proposed compatibility acceptance checks

These are initial test recipes, not already measured results or promised minimum hardware.

| Area | Proposed check | Evidence to keep |
|---|---|---|
| Startup | Ten cold starts and repeated scene loads | Build/data hashes, logs, failure count |
| Render-rate independence | Replay the same tick-input sequence at caps such as 30, 60, 120, 144, and 240 FPS, where the machine can sustain them | Simulation outputs and tolerance comparisons |
| Display changes | Alt-tab, resize, fullscreen transitions, monitor changes, DPI scaling | Observed behavior and backend/driver identity |
| Resource handling | Missing/corrupt resources, unsupported versions, cache invalidation | Clear error output; no silent substitution for required data |
| CPU/GPU support | Native runs on named CPU configurations and available AMD/NVIDIA/Intel GPU systems | Results tied to exact hardware and drivers |
| Input | Bind, calibrate, disconnect/reconnect controller and selected wheel | Deadzones, latency observations, device-specific results |
| Stability | Repeated races and a longer session appropriate to supported events | Memory trend, crashes, load/restart behavior |
| Handling | Acceleration/braking/steering tests and laps under matched conditions | Reference variability, selected tolerances, owner feedback |
| Saves and Flashback | Save/load and rewind at collisions, checkpoints, and race transitions | State consistency and regression results |

Choose a named reference PC before setting a performance target such as 60 FPS at 1080p. High refresh support means stable rendering and unchanged simulation behavior; it does not imply every computer reaches 240 FPS.

## 9. Research and AI working rules

Use small tasks with concrete outputs: for example, identify a PSSG vertex layout, explain a steering-input function, or implement a reader whose values can be compared to another tool.

For each fidelity claim, keep an evidence record:

```text
Question:
Reference build and resource hashes:
Source: file bytes / disassembly / runtime trace / published reference
Reproduction or analysis procedure:
Observed result:
Interpretation and confidence:
Unknowns or competing explanations:
Implementation and verification:
Intentional compatibility deviation, if any:
```

Treat decompiler output as an input to investigation. Do not invent missing units, field meanings, shader rules, or tire equations. A successful compilation or screenshot proves only that specific result.

Keep targeted format/simulation tests fast. Run broader integration checks when shared behavior changes. Public CI should use synthetic or redistributable fixtures; tests needing purchased resources run locally and report required-data availability explicitly.

Start with a single coordinated implementation stream and human review. Additional agents can be useful after subsystem interfaces and task ownership are clear, but are not a prerequisite. Avoid simultaneous uncontrolled changes to simulation, resource definitions, and rendering.

## 10. Source and asset distribution

Propose MIT OR Apache-2.0 for our independently authored code, subject to dependency and provenance review. Keep license notices for reused open-source components. EGO tooling can be a reference; inspect licenses before incorporating its code.

The owner having a purchased copy provides the project with a reference installation. It does not transfer the publisher's rights to distribute the game's executable, assets, or reconstructed protected code. Read original resources locally; keep installation files, extracted textures/models/audio, binary captures, and game-derived fixtures outside public release artifacts.

Observation and decompilation have different legal conditions. For EU context, Directive 2009/24/EC Article 5(3) addresses observation/study/testing by a lawful user, while Article 6 provides a conditional interoperability exception for decompilation. This report is an engineering plan, not a determination that a particular reconstruction or release satisfies those conditions. Record provenance throughout; assess the specific release and jurisdiction before public distribution. [EU software directive](https://eur-lex.europa.eu/legal-content/EN/ALL/?uri=CELEX%3A32009L0024).

Use a working project name until public naming is chosen. State that the project is independent and distinguish its code license from the original game's ownership and trademarks.

## 11. First implementation backlog

| Order | Task | Completion criterion |
|---|---|---|
| 1 | Reference installation and failure inventory | Exact build/architecture/hashes, machine details, reproducible failure |
| 2 | Toolchain compatibility probe | Useful Ghidra/REA analysis and one observed resource load |
| 3 | Original-game baseline | Repeatable reference driving run, with all applied changes documented |
| 4 | One car/track dependency map | Required files identified; unresolved dependencies listed |
| 5 | Resource inspector | Verified fields exposed with bounded parsing and useful errors |
| 6 | Track/car viewer | Original scene resources render at verified scale and placement |
| 7 | Handling measurement harness | Repeatable inputs and comparable reference outputs |
| 8 | First drivable time trial | Timed lap, restart, input, camera, collision, and diagnostics |

## 12. Open questions and feasibility

Resolve these early:

- Where is the Steam installation, and which exact build is installed?
- Which computer fails, with what symptoms? Is a Windows reference PC available?
- Which car/track pair should define the first milestone?
- Is keyboard/controller support sufficient initially, or is a particular wheel essential?
- Which resource variants and middleware appear in the actual installation?
- Can REA's pinned adapter analyze this executable on the chosen host?
- Which behavior can be measured without modifying the reference, and which needs instrumentation?

No reliable full-project schedule follows from Benilla or the MW2 demonstrations. The largest uncertainties are resource coverage, handling fidelity, AI/damage behavior, and GRID's full progression and presentation systems. Estimate effort after Phases 0–3, then revise it after the first viewer and handling prototype.

The immediate engineering objective is a small, measurable route to the owner's favorite game: identify the purchased build, obtain a reference run, load one car and track, reproduce its handling, and complete a lap in our own executable. Every later milestone expands that engine toward the complete GRID experience.
