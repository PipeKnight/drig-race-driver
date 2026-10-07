# Supplied collision candidate grid (F084)

`grid-world::candidate_grid::Grid` collects unique nonzero object IDs from supplied cells. It reproduces the original `96b440` collector with `96b390` coordinate conversion, `9583c0` subdivision limits and original `a6a210` bounds accessor. It does not populate the grid or intersect shapes.

## Contract

Convert each X/Z coordinate using f32 `((point-min)/(max-min))*65536`, truncate to signed integer, then clamp to0..65535. This scale is 65536, not65535. The caller supplies an ordered rectangle; the original outer segment query obtains its min/max before calling the collector.

The grid has32×32 coarse tiles. Coarse coordinates are integer coordinates divided by2048. Visit coarse Z then X in increasing order. Within a tile, subdivision exponent determines a shift of11-exponent and width2^exponent. Convert the rectangle to local fine-cell coordinates, clip its lower limits to0 and upper limits towidth-1, then visit fine Z followed by X. Ignore zero IDs, retain first encounter of every other ID and stop before adding beyond capacity. The production segment query supplies capacity25. Empty registry returns no candidates.

Native guards require finite increasing bounds, signed-conversion range, exactly1024tiles, matching power-of-two cell counts and capacity≤25. Subdivision exponent≤6 is an authored memory bound; production exponent/width relationships have not been exhaustively captured. SSE integer overflow yields an indefinite integer in the original; native code rejects such inputs rather than relying on Rust's saturating float casts.

## Comparison

**216/216 cases, 2,911/2,911 output words exact debug/release**, including four quantized coordinates, count and ordered IDs. Final evidence `local/behavior/candidate-grid-20261007-03/validation.json`, `04/validation.json`; earlier01/02also match before a Clippy spelling-only change. Thirty-six boundary/clamp/inactive/subdivision/capacity cases and180synthetic rectangles cover exponents0..3, sparse cells and repeated IDs. Python separately computes f32 coordinates and cell traversal.

The isolated probe executes the **whole original collector and helpers**, no stubs or code patches. Bounds, registry presence, widths/exponents and cell IDs are independently authored fixtures. One private data range at e81000..e85fff is temporarily writable, then its protection is restored; original code remains RX. No game entry, imports, TLS initialization or live injection. Probe SHA256 `1f2105f20171d5d8500528dd3d08da6bb7cd8e3842d8da7ffd6050fb5e22bf4b`.

All76project tests (47formats+29world), formatting and warnings-denied Clippy pass. First Clippy attempt rejects manual `%5==0`; changed to `is_multiple_of(5)` and reran Clippy and both comparisons. Evidence `candidate-grid-tests-20261007.txt`, `candidate-grid-clippy-20261007.txt` (failure), `candidate-grid-final-clippy-20261007.txt` (pass). Existing Rust1.99.0, MSVC14.44.35207/SDK10.0.26100.0, Python3.12.14, Ghidra12.1.4/JDK21.0.9 reused; no installation or game launch.

Authored tools: `grid-candidate-grid`, `probe_candidate_grid.c`, `build_candidate_grid_probe.cmd`, `validate_candidate_grid.py`. Private static evidence `20261007T1624144365729Z/`, `20261007T1625156363769Z/`, `candidate-grid-instructions-20261007.json`, `candidate-grid-constants-20261007.json`.

The existing production capture confirms245registered chunks and samples type11 shapes; it does not yet include subdivision exponents or every cell. Original shape traversal, equal-hit policy and material-token ownership remain separate from this candidate-list comparison.
