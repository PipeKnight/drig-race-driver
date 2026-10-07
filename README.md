# Independent GRID (2008) engine research

The intended engine will use Rust and Bevy and load resources from a player's purchased installation of Race Driver: GRID (2008). The first Rust resource inspectors are implemented; a renderer and playable engine remain ahead.

Start with [PROJECT_LOG.md](PROJECT_LOG.md) for current progress, findings and next actions. Detailed initial work is in [Phase 0 reference identification and compatibility investigation](docs/PHASE0.md).

The current [implementation plan](docs/IMPLEMENTATION_PLAN.md) defines targeted decompilation, resource readers, the first viewer/time trial and expansion toward full single-player coverage. Rust/Bevy remains the default; the plan explains the C++ alternative and the evidence required at each milestone.

`tools/` contains independently authored diagnostic scripts. `local/` contains this PC's private installation inventory, settings snapshots, research tools and crash dumps, and is excluded from Git. Original game assets and executables remain in the Steam installation.

The workspace contains `grid-formats` (bounded PSSG/JPK readers) and `grid-tools` (inspection CLI with explicit private diagnostic exports), using pinned Rust 1.99.0. [PSSG payload decoding](docs/PSSG_PAYLOADS.md) now covers selected Lacetti vertex/index/UV buffers and BC1/BC3 base textures. See [resource formats and validation](docs/RESOURCE_FORMATS.md) for scope and commands. No third-party Rust dependencies or game assets are included.

The supplied [GRID_2008_OPEN_SOURCE_PLAN.md](docs/reference/GRID_2008_OPEN_SOURCE_PLAN.md) is background planning material. The owner has separately authorized reasonable analysis and Rust rewrite work, including needed tool installations. Follow the current state and decisions in the project log.
