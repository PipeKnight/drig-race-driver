# Installed-source packed collision world (F093)

`grid-tools::track::Track::read` loads an uncompressed VCQTC JPK, decodes every bounded mesh/node/leaf/material table, assigns IDs in archive order, and constructs the grid from chunk names. Queries combine the verified candidate collector, shape dispatch policy, quadrant traversal, triangle primitive and normal/material outputs. The engine input is the purchased archive alone; it does not require private memory captures or address-valued cell fixtures.

The adapter owns decoded geometry and maps native shape IDs to source names and global triangle indices. `packed_shape::Hit` now retains the verified traversal's native triangle index for diagnostics. This field is internal identity from leaf selection, not an additional original world-return output.

Supported bounds: archive≤256MiB, 1..512 uncompressed canonical named chunks, total≤1million triangles, and existing format/population/query guards. Global grid bounds come from the first chunk, as the original collector reads the first registry shape. This is proven for the retained Donington variant; other archives, nonzero transforms and dispatch types require separate evidence.

## Comparison

**1,967cases /13,435words exact debug and release**,1,030hits/937misses. Source IDs, fractions, four-component normals, materials, candidate order and final shared cull cache match whole original `96b7f0` and its real helpers. The original fixture is independently rebuilt by Python from the same archive and name specification; Rust reads `track.jpk` directly. Case segments reuse F091's source/tiles/seeded/inactive inputs, with no address/cell fixture passed to Rust.

Evidence `local/behavior/source-world-20261007-01/validation.json` debug, `02/validation.json` release. Original probe remains SHA `a0719efd0b769861a125934ff012f1cc4138098f8cc8ff6c3c979d3a8570e947`. Native executable hashes are retained in those reports. Archive hash is guarded before/after comparison against the130resource manifest. No failed comparison in this milestone.

Full85workspace tests, warnings-denied Clippy and formatting pass, `source-world-tests-20261007.txt`, `source-world-clippy-20261007.txt`. All130Steamresources and original executable still match at2026-10-07T17:30:54.589688Z, `local/scene/preservation-20261007-source-world.json`. Existing pinned tools reused; no installs/game runs/user-file changes. Proprietary geometry and expected fixtures remain ignored local/.

Tools: `grid-track-query TRACK.jpk INPUT.tsv local/OUTPUT.tsv`, `validate_source_world.py`. [Source population](GRID_POPULATION.md), [world behavior](PACKED_WORLD.md) and [shape behavior](PACKED_SHAPES.md) specify the supported arithmetic. Placement/contact integration and driving remain separate milestones.
