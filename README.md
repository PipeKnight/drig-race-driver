# Independent GRID (2008) engine research

The engine uses Rust and Bevy and loads resources from a player's purchased installation of Race Driver: GRID (2008). Resource readers, a Lacetti/Donington scene viewer and an experimental keyboard driving prototype are implemented. Body/barrier collisions, a verified full lap and a complete independent game remain ahead.

To try the current build on this PC, double-click **Play GRID.cmd** and pick the event in the game's Race Day menu ([event menu](docs/EVENT_MENU.md)); the other `Play GRID *.cmd` launchers start one event directly. See [prototype controls and limitations](docs/TRY_THE_PROTOTYPE.md).

<img width="1280" height="720" alt="image" src="https://github.com/user-attachments/assets/f46aebc0-85d4-422a-9b75-f2b3b9c99744" />

I also work on graphics improvement, testing technologies like DLSS/DLAA, RT etc

<img width="820" height="370" alt="moving-bias-native01" src="https://github.com/user-attachments/assets/bec7f111-4d29-4e80-be70-c2d5ed45c4f9" />

If you want to contribute, start with [PROJECT_LOG.md](PROJECT_LOG.md) for current progress, findings and next actions. The [9 October progress assessment](docs/PROGRESS_ASSESSMENT_2026-10-09.md) records completed capabilities, blockers, completion estimates and the next integration milestones. Detailed initial work is in [Phase 0 reference identification and compatibility investigation](docs/PHASE0.md).

The current [implementation plan](docs/IMPLEMENTATION_PLAN.md) defines targeted decompilation, resource readers, the first viewer/time trial and expansion toward full single-player coverage. Rust/Bevy remains the default; the plan explains the C++ alternative and the evidence required at each milestone.

`tools/` contains independently authored diagnostic scripts. `local/` contains this PC's private installation inventory, settings snapshots, research tools and crash dumps, and is excluded from Git. Original game assets and executables remain in the Steam installation.

The workspace uses pinned Rust 1.99.0 and Bevy 0.19.1:

- `grid-formats`: bounded resource readers and format structures.
- `grid-world`: geometry queries and independently implemented, scoped simulation components.
- `grid-tools`: inspection and private diagnostic comparison CLIs.
- `grid-app`: the Bevy scene viewer and experimental driving prototype.

[PSSG payload decoding](docs/PSSG_PAYLOADS.md) covers selected Lacetti vertex/index/UV buffers and BC1/BC3 base textures. See [resource formats and validation](docs/RESOURCE_FORMATS.md) for scope and commands. Dependencies are recorded in Cargo.lock; proprietary game assets are excluded.

The supplied [GRID_2008_OPEN_SOURCE_PLAN.md](docs/reference/GRID_2008_OPEN_SOURCE_PLAN.md) is background planning material. The owner has separately authorized reasonable analysis and Rust rewrite work, including needed tool installations. Follow the current state and decisions in the project log.
