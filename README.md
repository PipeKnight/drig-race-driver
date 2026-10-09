# Independent GRID (2008) engine research

The engine uses Rust and Bevy and loads resources from a player's purchased installation of Race Driver: GRID (2008). Resource readers, a static Lacetti/Donington scene viewer and scoped vehicle/collision components are implemented. A drivable Rust game remains ahead.

Start with [PROJECT_LOG.md](PROJECT_LOG.md) for current progress, findings and next actions. The [9 October progress assessment](docs/PROGRESS_ASSESSMENT_2026-10-09.md) records completed capabilities, blockers, completion estimates and the next integration milestones. Detailed initial work is in [Phase 0 reference identification and compatibility investigation](docs/PHASE0.md).

The current [implementation plan](docs/IMPLEMENTATION_PLAN.md) defines targeted decompilation, resource readers, the first viewer/time trial and expansion toward full single-player coverage. Rust/Bevy remains the default; the plan explains the C++ alternative and the evidence required at each milestone.

`tools/` contains independently authored diagnostic scripts. `local/` contains this PC's private installation inventory, settings snapshots, research tools and crash dumps, and is excluded from Git. Original game assets and executables remain in the Steam installation.

The workspace uses pinned Rust 1.99.0 and Bevy 0.19.1:

- `grid-formats`: bounded resource readers and format structures.
- `grid-world`: geometry queries and independently implemented, scoped simulation components.
- `grid-tools`: inspection and private diagnostic comparison CLIs.
- `grid-app`: the static Bevy scene viewer.

[PSSG payload decoding](docs/PSSG_PAYLOADS.md) covers selected Lacetti vertex/index/UV buffers and BC1/BC3 base textures. See [resource formats and validation](docs/RESOURCE_FORMATS.md) for scope and commands. Dependencies are recorded in Cargo.lock; proprietary game assets are excluded.

The supplied [GRID_2008_OPEN_SOURCE_PLAN.md](docs/reference/GRID_2008_OPEN_SOURCE_PLAN.md) is background planning material. The owner has separately authorized reasonable analysis and Rust rewrite work, including needed tool installations. Follow the current state and decisions in the project log.
