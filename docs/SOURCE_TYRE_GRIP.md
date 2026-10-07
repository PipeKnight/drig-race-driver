# Source-backed material grip queries (F107)

Recorded 8 October 2026. `grid_tools::tyre_tables::Tables` owns the decoded source CSV and joins a caller-selected row to the verified key-cache and material speed-grip expression. `grid-tyre-grip CSV INPUT local/OUTPUT.tsv csv` exposes the same source path for private comparisons. Caller-owned material keys preserve their unresolved/cached state across queries.

Unresolved misses return the default factor without inspecting row values. This matters for a source table with absent row slots: a miss still succeeds, whereas a matching or already cached key requiring an absent row returns an authored error. The original would perform an unchecked row-pointer read. Cache mutation precedes that read in the original; the native error likewise retains the newly resolved index. Inactive queries leave the key untouched.

**41 source datasets / 17,794 queries / 53,382 words exact debug and release**. The comparison runs the whole original F106 CSV loader, checks its descriptor against independent Python parsing, captures that table into a private query fixture, and runs whole original F105 `0099c990/00998690`. Rust reads each CSV directly. It exercises every present row and key using resolved/unresolved keys, active/inactive queries, clamping and varied supplied coefficients/speeds. All thirteen rows additionally receive missing-code and inactive queries. The original loader's numeric/handle CRT adapters remain explicit; query functions are unmodified.

Absent fixture rows contain unused padding because the F105 fixture represents complete supplied rows. No query consumes such padding: missing-code/inactive paths return before row access. The matched/cached absent-row guard is separately checked in an authored native test, without claiming an original null-pointer query succeeded. Row selectors and material coefficients/speeds remain supplied rather than live production inputs.

Evidence: `local/behavior/source-tyre-grip-20261008-01/validation.json`, `02/validation.json`. F106 baseline manifest SHA **f0338189eff5f328404eb907d3c55d937102cf065fe0f17363775a523d54a34b**, every source hash and original probe identities are pinned before replay. Original probes retain F105/F106 hashes. Old supplied-table 1,394-case/4,182-word behavior also regresses exactly, `tyre-grip-regression-20261008-01/`, `02/`.

**94 workspace tests**, formatting and warnings-denied Clippy pass, `source-tyre-grip-tests-20261008.txt`, `source-tyre-grip-clippy-20261008.txt`. No failed comparisons or checks.

Steam preservation check `local/scene/preservation-20261008-source-tyre.json`, **2026-10-07 23:52:57 UTC** (8 October locally): all **130 tracked resources and original executable unchanged**. No game/settings/save changes or new installations. Existing tool/build versions remain those recorded in F106. This establishes source table-to-grip integration, not complete wheel-history, body row-selector or vehicle force behavior.
