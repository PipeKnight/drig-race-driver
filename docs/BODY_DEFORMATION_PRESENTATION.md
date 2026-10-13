# Experimental body dents from the source deformation grid

The clean **d221dc28f774** refresh now passes all ten source/five Bevy checks, collision and straight-road renders. Logged physics/deformation matches the prior rigid-graphics build, while81 meshes update only when the grid changes. [Build identities and evidence](PROTOTYPE_BUILD_UPDATES.md#body-dent-presentation-refresh). The later status supersedes the pre-refresh note below.

13 October2026, F268. The prototype can project the physical deformation grid onto source Lacetti body meshes. This is an **independent CPU presentation policy**, not a port or equivalence claim for GRID's original graphical shader/part factory. Wheel meshes retain their separate verified wheel-state presentation; aerials remain rigid. Part detachment, broken glass and original damage materials remain open.

Each source vertex is transformed through its original joint bind into the grid's source coordinate frame. The visual delta is the sampled current position minus the sampled pristine/rest position. That XYZ delta is transformed back into mesh-local coordinates and applied to the immutable original vertex, avoiding accumulated deformation. Pristine fields return the complete original mesh, including signed zeros, normals and unconsumed attributes. Changed meshes retain indices/UVs and recompute finite smooth normals.

All selected mesh updates and their local bounds are staged before any Bevy asset is replaced. Mesh instances own separate handles; ambiguous or missing ownership rejects. Reset restores original meshes/bounds. The cache key contains consumed layout/current/rest positions and activation, so unchanged physical grids do not cause repeated GPU uploads. Presentation reads car state and never feeds physics.

## Validation

The purchased LOD0 Lacetti has98 draws.81 selected non-wheel/non-aerial draws are pristine-exact initially and all81 change after the existing600-tick collision-driven source run. Source indices/UVs remain exact, projected bounds exist and owner state stays unchanged. The actual Bevy asset test inserts a bad second bind after a valid first projection: neither mesh/bound nor owner changes, failure is reported, repairing the bind allows publication, and reset restores both original meshes exactly. The deliberately injected failure event in the private test log is expected, not an observed normal-play failure.

An immutable prepared sampler validates each node view once for many vertex queries. It uses the same ordered interpolation implementation as `Grid::sample`, without weakening that API's guards. Fresh debug/release whole original9A39D0 comparisons each match **824 cases /3296 words**, including240 constructed queries from twelve retained grids. Five malformed input guards reject both diagnostics; Rust creates no output. These remain supplied-grid tests, not new live captures or original graphical shader execution.200 ordinary libraries, formatting and all-target Clippy pass. The extended source/Bevy projection test passes; the full clean-build source/Bevy suite and rendered comparison follow separately.

## Private evidence

| Evidence | SHA-256 |
| --- | --- |
| local/behavior/deformation-batch-sample-debug-20261013-final-01/validation.json | f9fb66cdb5c57ea12c34f097f2683a75a0f412a4e60ff26e1060546fc7accf96 |
| local/behavior/deformation-batch-sample-release-20261013-final-01/validation.json | 0189869968777c42bd2906d3896cc902bc2984e679c7b16f812f60876938608a |
| local/behavior/body-deformation-projection-release-20261013-draft-01.txt | 6b19eadb11f7911e7263b8c5c33a9c4f1979227dd5237316e2e98c2878ae5d09 |
| target/debug/grid-deformation-sample.exe | 8621be40c61cef6b6048158528f9e8ebd8557932626bb2055d5185d01a2b2299 |
| target/release/grid-deformation-sample.exe | c2a970778ddb18b903c4f57607d0759d853e6bddc2254f02b008abcee30aa5e5 |

Original sampler probe remains hash-pinned8ce8358aeccf155bd5e0b4e4e0cfbf5bbf9dd78a83f0b60e7bdd05aeb7a7073c; analysis PEaac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514. Rust/Cargo1.99.0, Bevy0.19.1, Python3.12.14 reused. Local Bevy mesh/bounds APIs were checked in the installed0.19.1 source.

New Ghidra12.1.4/JDK21.0.9 survey`local/behavior/20261013T2015319930096Z/` covers9D12B0/9B8AE0/9B8A50/9B8B70; its log is`graphical-deformation-consumers-20261013-survey.txt`. The decompiler suggests six sampled part anchors and status transitions in9D12B0, not a complete vertex path. That is investigative evidence, not a newly executed/ported original consumer. Existing native comparison with empty graphical parts remains unchanged.

An atomic patch context miss was corrected before edits. A wildcard search rejected by PowerShell paths was rerun with explicit globs. No native fault or numerical mismatch, installation, desktop input, Steam/settings change, owner help or approval rejection occurred. Current clean **d178b80b77be** remains playable until the new clean refresh is verified. Body-dent magnitude/appearance and ordinary-road activation remain uncalibrated against the original game; grid movement should not be interpreted as a measured real-world dent size.
