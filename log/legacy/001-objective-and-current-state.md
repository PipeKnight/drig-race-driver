## Objective and current state

Earlier9October milestone F138: independent Rust contact-to-record generation **and its combined grid solver pipeline** match six actual live collision calls exactly in debug/release:137records/1,507written recordwords and63,360descriptorwords. Three actual zero-record calls,192authored boundary cases and8invalid guards also pass per build. [F138 contract and evidence](docs/IMPACT_GENERATION.md#independent-rust-generation-and-grid-pipeline-f138). Earlier F137 verifies111actual supplied solver records. Later F139 above adds response; contact detection and full vehicle integration remain open.104workspace tests/fmt/warnings-deniedClippy pass; original130resources+executable still match. No new tools installed.

Independent Rust/Bevy engine for Race Driver: GRID (2008), loading the player's purchased Steam resources. Static Lacetti, Donington surfaces and placed scenery render in Bevy 0.19.1. Entity startup visuals and track collision geometry are GPU-verified; binary XML and collision descriptors decode. Rust reproduces twelve live placement matrices exactly from recorded inputs; source-only alignment retains two one-float-step X differences. Original whole-query policy, render/body linkage, shader fidelity, collision simulation and driving remain ahead. [Live placement evidence](docs/LIVE_SPAWN_CAPTURE.md), [car viewer](docs/STATIC_VIEWER.md), [Donington scene](docs/DONINGTON_SCENE.md).

The owner has authorized reasonable analysis/decompilation and Rust rewrite work, including needed tool installation. All progress and findings must be recorded here or in linked Markdown research documents. This log is the durable project index, with evidence kept locally.

| Milestone | Status | Evidence |
|---|---|---|
| Phase 0: installation, build, architecture | Complete | [Phase 0 report](docs/PHASE0.md) |
| Phase 0: failure reproduction | Complete | Same startup exception reproduced in baseline and diagnostic runs |
| Startup compatibility trigger | Identified in private copy | UTF-8/legacy process-codepage controls; exact failing internal API unresolved |
| Phase 1: useful static/dynamic toolchain | In progress | [Toolchain record](docs/TOOLCHAIN.md) |
| Phase 2: running original reference | Race loading, physical-key driving and timed macros work | Conditions recorded; fresh load traced; precise repeatability measurements pending |
| Car/track reference | Lacetti / Donington pairing and route_0 reads confirmed | Owner confirmation plus [130-file dependency map](docs/REFERENCE_DEPENDENCIES.md) |
| Agent game controls | AutoHotkey 2.0.29 operational | Menu, bound trigger, throttle, combined throttle/right and pause verified; approximate wall-clock timing |
| Native compiler | Validated | Pinned Rust 1.99.0, MSVC 14.44.35207, SDK 10.0.26100.0; executable builds and runs |
| Native resource readers and behavior blocks | Framing, meshes/textures, placements, entities, track/car collision, binary XML, progress, tuning, body prelude, Standard wheel layout and scoped dynamics implemented | 104 workspace tests; full vehicle simulation pending |
| Geometry diagnostic | Raw Lacetti preview produced | 46,577 position records / 38,803 triangles; transforms, visibility, skinning and materials unapplied |
| Renderer / playable Rust engine | Static car, Donington surfaces, scenery and entity visuals GPU-verified | Bevy 0.19.1; driving simulation pending |
