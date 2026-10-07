# Source chunk-name grid population (F092)

`grid-world::grid_population` reconstructs grid cell identities from canonical chunk names and their supplied order. `Path::from_name` supports `qt_XX[_XX...].vcqtc`, depth1..11. In each pair the first character is the X bit, the second the Z bit. Starting weight32768 halves for each pair; the path locates an aligned square of size `65536>>depth` in the grid's normalized domain.

Static loader `5bb180` first selects each tile's exponent as the maximum of `depth-5` and zero, for paths starting in that tile. Original `96b660` allocates `2^(2*exponent)` zero cells per tile, width `2^exponent`. The loader then inserts each archive entry in order through `958470`, using inclusive integer high corners `low+size-1`. Coarse tile iteration and `9583c0` fine bounds select cells; insertion overwrites them with the current shape ID. Registration preserves input order. No geometric overlap test or vertex-bound quantization supplies these placement regions.

The native builder validates IDs, depths, aligned coordinates, finite bounds, count≤512, exponent≤6 and total cell bytes≤4MiB. Its name parser is deliberately narrower than the original, which tests for underscores and treats characters other than '1' as zero. It rejects noncanonical names rather than claiming equivalence for arbitrary strings.

## Verification

**78/78 datasets / 706,149/706,149 output words exact in debug and release**, covering1,177paths. All245installed Donington chunks, single paths at every depth1..11, ordered/reversed overlap controls and64seeded varied datasets. The probe runs whole original `5b4d40`, `96b660`, `958470` and its subdivision helper. Only allocator virtual methods are supplied by the host; exponent selection is authored from inspected loader behavior. No claim to executing the whole resource loader or original deallocator. Original registry order/count is checked in the probe. Independent Python computes every expected path coordinate, width, exponent and cell ID.

**Source-built identities match all1,024retained production tiles exactly**, after mapping private wrapper addresses by their uniquely verified source chunk. Source depths: one depth2,17depth3,103depth4,124depth5. Consequently all1,024exponents are zero, with872nonempty cells and245identities. All source chunks share the same global bounds, matching the retained bounds. Neither addresses nor the captured cell fixture are required as runtime engine inputs.

Evidence `local/behavior/grid-population-20261007-01/validation.json` debug, `02/validation.json` release; exploratory independent comparison `source-grid-exploration-20261007.json` also matches. Probe SHA `2f66ac5dd537530e5ed0fd12a8d6940ad03324123309e1d4974f09688e30ac4d`. No failed execution comparisons in this milestone. Whole workspace tests, warnings-denied Clippy and formatting pass: `grid-population-tests-20261007.txt`, `grid-population-clippy-20261007.txt`. Existing pinned Rust/MSVC/Python/Ghidra/JDK reused, no tool installs or game/user-file changes.

Private surveys: `20261007T1720307741646Z/958470.c.txt`, address refs `20261007T1721509650892Z/`, loader `20261007T1722194481276Z/5bb180.c.txt`, parser/allocator/accessors `20261007T1722450839334Z/`, instructions `source-grid-instructions-20261007.json`. Only isolated grid data is writable/restored; no code patches, stubs to parser/insertion, original entry/import/TLS or live injection.

Tools: `grid-population`, `probe_grid_population.c`, `build_grid_population_probe.cmd`, `validate_grid_population.py`. [World query](PACKED_WORLD.md) can now be supplied a grid made from source names. Integration, nonzero transforms, other content layouts and live contact/timing fidelity remain separate checks.
