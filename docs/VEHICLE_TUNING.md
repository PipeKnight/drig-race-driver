# Vehicle tuning schema research

Recorded 6 October 2026. The [CTF editor author's official page](https://p2t5r.itch.io/ego-ctf-editor) explicitly lists Race Driver: GRID support and game-specific XML schemas. The pinned [upstream library](https://github.com/EgoEngineModding/Ego-Engine-Modding/tree/f3fe9ee0f6e8379c64bc4a29e1d1b586bba5d1c1/src/EgoEngineLibrary/Vehicle) loads those schemas externally; they were absent from the source tree previously inspected.

## Reference acquisition

Earlier research inventory found no CTF editor installed. Downloaded the author's **4.2.2016.0920** bundle through its free official download option; no payment, account creation, messaging or installation occurred. Archive is 276,103 bytes, SHA-256 `751ac98af1389994a3eccc6fea1729f49ae17ed1ea8eed4f292553a73ce4d439`. No published checksum was supplied, so this is a recorded local digest, not publisher verification. Private copy/extraction: `local/tools/ego-ctf-4.2/`. Existing Windows `tar.exe` successfully lists/extracts 7z; no decompressor installation needed. Executables have not been launched.

The browser download wait unexpectedly remained pending until 10:25 UTC despite the requested timeout. It returned a concrete local path and the saved archive validated/extracted; no intervening game actions were made. The temporary research tab was closed afterward.

## Current observations

Bundled `ctfSchemaGrid.xml` version **2013.0107** declares **360 positional fields**, with CSV extension. Lacetti `lac.csv` has one populated line of 361 comma-separated cells, the last empty, followed by a blank line. Initial count comparison therefore failed strict equality; trailing delimiter handling needs to be checked against the reference reader before treating this as schema disagreement.

The first three cells are strings (`car`, `car`, `Standard`), followed by numeric values. The schema labels column 3 (zero-based) as mass, with actual Lacetti value 1140.000000, and column 16 as rev_limit, actual 890.117943. Names/units and axis descriptions are **community schema hypotheses**, not independently verified original-engine semantics. Keep positional lexemes and compare actual consumers before implementing forces, inertia or power conversions. Several names explicitly contain unknown/unused markers.

Evidence: `local/behavior/lacetti-schema-survey.json`; downloaded schemas/readme remain private, with license/provenance to be clarified before any redistribution. Pinned primary CsvFile/CsvStreamReader source references retained under `local/references/ego-tools/` for framing comparison.

## Native and reference-reader validation

The pinned upstream reader splits the selected line on commas and consumes the schema's 360 positions; it ignores an extra trailing empty cell. This resolves the initial Lacetti count discrepancy. The independently written Rust reader preserves every lexeme, trailing empty cell and blank line, and exposes finite numeric conversion without assigning speculative field meanings. It accepts the observed unquoted subset and rejects quotes, NULs, bare carriage returns and excessive input sizes.

All **46 installed car-directory CSV files** match an independent Python comparison, including every lexeme and finite f64 bit pattern. **44 files fit the 360-column GRID schema**. `cars/tra/tra.csv` has 351 cells and remains unmapped; `cars/tyreGrip.csv` is a separate 15-row, 17-column table. No schema padding or guessed conversion is applied. Schema entries comprise 313 doubles, 39 integers and eight strings. Evidence: `local/behavior/tuning-validation/validation.json` and its per-file reports; validator `tools/validate_vehicle_tuning.py`. The format suite now passes 36 tests.

Read-only operation of the bundled reference DLL also succeeds against the original Lacetti CSV: 360 schema entries, line index zero, one loaded file. Actual assembly identity is `EgoEngineLibrary, Version=12.1.2016.905`. The initial Windows PowerShell 5.1 script invocation was blocked by execution policy before library loading; a process-only `-ExecutionPolicy Bypass` retry succeeded, without changing machine policy or opening the editor GUI. Archive identity is checked before loading. Original CSV is opened read-only. Evidence: `local/behavior/ctf-library-probe.json`, both probe logs and `tools/probe_ctf_library.ps1`.

This verifies reader framing and useful reference-tool operation. It does not verify community labels, units, original engine numeric precision or force equations. Next: trace original positional consumers, and develop independently validated ground queries before wheel contact or driving integration.

## Original tyre-grip table loader

The original `VehicleTyreTable` constructor `0x00998620` uses a three-entry vtable at `0x00d154b8`; its read method `0x009e5600` reaches parser `0x009c2dd0`. That parser skips two label columns, reads column codes from the first row, matches row codes against 13 built-in four-byte identifiers, parses numeric cells and multiplies them by stored f32 **0.009999999776482582** (`0x3c23d70a`). It stores f32 results. This is static evidence for percentage-like normalization, not a runtime capture or the subsequent force formula.

Installed `cars/tyreGrip.csv` has two header rows, 15 data columns and 13 material-code rows, **195 numeric values, all 100**. Those scale to f32 1.0. Column descriptions and row identifiers are distinct axes; do not assume either is the VCQTC material code directly. `MECHANICS.tyreTable` is separately parsed as a four-byte token by `0x009b90f0`; its full consumer link is still pending.

Evidence: `local/behavior/tyre-table-vtable.json`, `tyre-grip-parser-constants.json`, `20261006T1415137208797Z/9e5600.c.txt` and `20261006T1416328260182Z/9c2dd0.c.txt`. No original file was written.

## Positional vehicle decoder located

References to generic CSV reader `0x009917f0` lead to vehicle resource reader `0x009ddc20`, which calls positional decoder **`0x009aec60`** for a selected row into a **0x65c-byte record**. This replaces the earlier broad car-loader candidate with a concrete tuning-decoding target. Static calls parse numeric cells and store f32 values; the editor schema's double labels are therefore not the original runtime storage type. The decoder also applies conversions/defaults/derived values, so raw CSV values must not be treated as the final simulation record.

Generic tokenizer `0x00991240` uses **360 cells of 64 bytes per row**, a 0x5a00 row stride. It advances rows on CR, ignores LF and splits on commas, without quoted-CSV support or apparent string-length bounds. Our Rust reader preserves data safely rather than reproducing those unsafe buffer behaviors. A trailing comma followed by CR writes no extra text cell; this explains the observed 361-cell lexeme count versus 360 consumed fields. Missing entries such as `tra.csv` remain empty in the original staging buffer; final fallback semantics have not been reconstructed.

Observed raw mappings include input column 3 to record offset 0x130, columns 4/5/6 to 0x134/0x138/0x13c, columns 7/8 to 0x150/0x154, and columns 9/10/11 to 0x144/0x148/0x14c. These are index/offset observations, not independently verified mass/inertia/axis units. At least one later conversion uses stored f32 0.01745329238474369, consistent with degree-to-radian scaling; field linkage needs further checks before assigning a semantic name.

Private evidence: reader references `20261006T1420140775252Z/address-refs.tsv`, readers `20261006T1420581747559Z/`, decoder/tokenizer `20261006T1422465120921Z/`. No full public layout, force model or faithful vehicle implementation is claimed yet.

Follow-up reaches concrete body initializer `0x00a0c9f0` and wheel helper `0x009b6350`. The supported independent [body/wheel prelude](BODY_PARAMETERS.md) now matches 44 original Standard CSVs and 444 live f32 words, uniquely resolving seven Lacetti and five BMW parameter sets. This establishes selected actual numeric copy consumers; complete force equations, units and axle/geometry semantics remain open.
