# Circuit progress and spawn research

Recorded 6 October 2026. `grid-formats::progress` reads the verified sectioned binary XML into typed circuit gates, points and route splits. It deliberately supports the observed forward circuit subset. It checks declared counts, ordered IDs, finite float3 coordinates, nondegenerate gates, increasing distances within the declared length and bounded split references. It does not implement crossing, lap counting or vehicle spawn behavior.

Donington route_0 contains 71 gates, 389 points, one forward route and three splits, with total distance 4003.84. All **464 native report rows** match independent decoding, including each f32 bit. Private evidence: `local/binxml/donington-progress-native.tsv`, `donington-progress-validation.json`; reproducible helper `tools/validate_progress.py`. Two new meaningful tests reject bad counts, references, degenerate gates, nonfinite coordinates and unsupported direction. All **32 format tests** pass.

## Spawn discrepancy

Raw composed `slot_00` position is (-183.11911, -12.147942, 19.41256). Its local +Z maps mostly toward positive X. Front light geometry confirms the car's visible front lies at local +Z. Increasing early progress points and gates travel toward negative X. Original race screenshots show pit wall left and grandstands right; the raw-transform showcase faces the opposite way. Static position/road agreement does not prove heading fidelity. The raw source transform remains unchanged pending consumer tracing.

## Targeted executable evidence

Analysis reference remains the hash-identified private unpacked PE (see [loader research](LOADER_RESEARCH.md)). Existing Ghidra 12.1.4/JDK 21 were reused without new broad analysis or tool installation. `GridBehaviorSurvey.java` scans bounded ASCII strings and reports existing static references; absence of a reference is not proof of nonuse.

- Initial 100 strings / 45 references; expanded survey 245 / 182. Evidence directories `local/behavior/20261006T0920454911778Z` and `20261006T0924162842720Z`.
- `0x009176e0` checks `progress_track_data` and exporter version `2.0.0`, then calls track/routes/gates/points parser candidates. `0x00909ef0` references circuit/type and total_distance. This statically links the readable schema to an original consumer; runtime arguments and lap algorithm remain open.
- `0x009b90f0` reads MECHANICS fields including grip/slowdown/bump controls and a tyreTable identifier. Separate fields are preserved; final force/contact equations are still unknown.
- `locked_timestep` references at `0x0057d9b0` and `0x0057dcf0` belong to sequences with slow motion, camera zoom and result presentation fields. These are not evidence of the core race simulation rate.
- Expanded survey identifies start_standing/start_rolling/grid references in `0x00916bf0`, and a grid-positioning failure diagnostic in `0x006333c0`. Targeted investigation continues.

Private decompilation and resolved constant names: `local/behavior/20261006T0923032990874Z/`, `field-name-linkage.json`. Independently authored extraction helper `tools/inspect_behavior_fields.py`. Pseudocode types/function boundaries remain inferred; no original pseudocode is copied into the engine source.

Follow-up establishes that `0x0090ab50` accepts slot IDs and constructs three world points from the marker's bounding box and transform. Its output is a rectangle representation, not a direct car pose. `0x0090cf40` draws that rectangle for diagnostics. The initial neighborhood guess did not identify pose construction; an existing-disassembly scan of the observed per-vehicle grid-index displacement `0x2e4c` found 65 instructions in 23 candidate functions. `0x00916f20` reads a selected grid, iterates eligible vehicles and passes each assigned index to `0x00915a40`. `0x00914d80` instead offsets distances along route data. The static chain is now narrowed; runtime tick/pose fidelity remains unverified. Evidence: `local/behavior/slot-neighborhood.tsv`, `spawn-index-refs.tsv`, and timestamped private function reports. These are scans of the partial saved analysis, not exhaustive executable coverage.

The constant-name helper initially rejected a non-file-backed global because it assumed every DAT reference was a string. It now records those addresses separately and resolves only bounded file-backed printable constants; the corrected reports completed. The progress reader now explicitly requires exporter version 2.0.0, matching the original root parser's static check. Versioned tests/build pass; workspace lint previously passed and will be rerun after the final source change.

## Heading consumer resolved

Static chain: `0x00916f20` → `0x00915a40` → `0x009159d0` → `0x00911790` (choose available exact slot index) → `0x00915270`. The pose builder constructs its direction from rectangle corner **A minus C**, opposite the marker's +Z. Corner definitions are `(minX, midY, minZ)`, `(maxX, midY, minZ)` and `(minX, midY, maxZ)` transformed to world coordinates. Donington slot bounds are X ±0.5, Z ±1, Y 0..0.0001. The final setter `0x009ef040` passes that direction into `0x009de4e0`, which also queries ground and wheel geometry. This resolves why the raw matrix faced backwards; it does not yet recover final spawn height or all alignment offsets.

`grid-formats::grid` independently builds marker corners and the observed direction. The viewer now uses a horizontal preview basis from this direction while retaining marker-origin translation. It explicitly reports that full contact height/offset is pending. Source bytes/transforms remain unchanged. GPU validation and independent marker comparison are in progress.

Private function evidence: `local/behavior/20261006T0931492980963Z/` and `20261006T0932260722195Z/`. One intermediate run passed an unsupported nearby address (`0x00915710`); the script reported no function there and aborted subsequent outputs despite process exit zero. That address was discarded, the follow-up used direct identified calls, and the wrapper now verifies every requested output exists and contains no failure marker. Do not count the incomplete run as successful decompilation. An initial Rust test literal syntax error was corrected before compilation/testing.

Validation completed: all **21 markers** match independent Python hierarchy composition and bounding-box calculations **bit for bit**, including normalized directions. Evidence: `local/behavior/grid-markers-native.tsv`, `grid-markers-validation.json`; helpers `grid-markers` and `tools/validate_grid_markers.py`. **34 format tests pass**, workspace Clippy passes. Updated close render `local/scene/lacetti-donington-marker-heading.png` exits zero and was visually inspected: car faces down the straight with pit wall on its left, consistent with the original. Static height, car-specific longitudinal offsets, ground normal and suspension remain pending.

7 October continuation: [ground placement](GROUND_PLACEMENT.md) now has controlled original arithmetic and twelve exact live return-boundary replays. Source-only alignment retains two one-ULP X differences; [live capture](LIVE_SPAWN_CAPTURE.md) records them. The viewer still uses the horizontal diagnostic anchor pending render/body linkage. [Vehicle update](VEHICLE_UPDATE.md) separates verified outer arithmetic from unresolved suspension and driving.
