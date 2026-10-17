# Complete Standard property decoding — F202

12 October 2026, Europe/Kiev (reference runs 11 October UTC). Rust now decodes the complete 1,628-byte Standard property block from the purchased CSV resources. All 44 supported installed rows and 423 authored variations match the original decoder and its repeated execution byte for byte in both debug and release. This closes property decoding; complete car construction, runtime setup and Bevy driving remain open.

## Scope and recovered behavior

`vehicle_properties::read` consumes a bounded Standard row and caller-supplied opaque metadata. It independently reconstructs names/revision strings, enum and boolean fields, numeric conversions/defaults, the 32-node torque curve, gearing and the complete 30-by-two custom/fallback setup endpoints. Output includes untouched sentinel words and trailing metadata at 658. The direct field table records factual source-column/output-offset relationships; it contains no game data or original code.

The original comparison maps the hash-pinned private executable without invoking its entry point, imports, TLS or CRT initialization. Whole `9AEC60` executes with original `991640` lookup, `998860` custom setup, `998E60` fallback setup and math helpers. Its aligned entry through `9B1B40 ret` establishes cdecl. An owned 360-by-64-byte cell matrix supplies the already decoded strings. The global lookup table at E85E10 is temporarily owned, snapshotted and restored with page protections; prefix/suffix sentinels bound the destination. Two explicit host CRT adapters replace only BF647D float and BF630F integer conversion and are restored afterward. Original CSV tokenization/handle creation and original CRT numeric parsing are excluded.

The reference x87 control is 53-bit nearest. Rust preserves the observed f64/f32 store boundaries, including defaults tested before or after storage, signed zero, degree/percentage conversions, original power operation and setup rounding. Column49 has two observed uses: revision text and numeric prefix. A bounded decimal-prefix reader handles that column explicitly; other numbers remain strict. This does not establish general legacy CRT parsing. Numeric overflow/nonfinite values, invalid integers, non-ASCII/NUL/overlong cells and unsupported row shapes fail before output publication. Opaque metadata, flags and sentinel bits are not interpreted as floats.

## Verification

| Per profile | Records | Complete bytes |
|---|---:|---:|
| Hash-pinned installed Standard rows | 44 | 71,632 |
| Authored numeric, enum, setup, threshold and combined variations | 423 | 688,644 |
| Total original/repeat/Rust comparison | 467 | 760,276 |

Controls vary numeric columns individually, all drive/differential combinations, complete/incomplete custom setup, signed zero, tiny values and rounding boundaries. Six combined boolean threshold vectors and 32 deterministically randomized multi-field rows extend the individual-field checks. Independent checks also establish mass, zero slot318, forward slots31C–338, reverse33C and opaque metadata658.

The final audit rehashes both executables, upstream source inventory, every CSV/cell matrix and all 2,802 property output files across both profiles. It checks native restoration diagnostics and complete original/repeat/Rust output hashes. Five native framing guards and 14 strict Rust guards per profile pass (33 total), without creating output files. These are distinct contracts, not paired malformed-CRT equivalence. All 161 workspace library tests, formatting and all-target warnings-denied Clippy pass. Steam executable and 130 traced resources remain unchanged at `2026-10-11T21:28:56.765561+00:00`.

Earlier drafts are retained. A first independent assertion placed reverse at318; actual output establishes reverse33C. Strict parsing initially rejected the dual-use revision column. The wider comparison then exposed missing boolean/degree fields and a fallback high-damper range using the wrong ratio fields; original stores and whole setup helper resolve these differences. Draft04 and the expanded final debug/release comparisons are exact. A closure-index type error was fixed before execution; the audit initially used an incorrect baseline hash-key name and was corrected before a successful audit. No native exception occurred. One guessed world-adapter path failed harmlessly; inventory identified its actual grid-tools location. No desktop/input/settings/install changes were made.

## Private evidence and reproduction

All executable/resource-derived evidence stays ignored under `local/behavior/`:

- `standard-properties-debug-20261011-draft-05/validation.json`: final debug, 467 records.
- `standard-properties-release-20261011-draft-01/validation.json`: final release, 467 records.
- `standard-properties-audit-20261011-final-02/audit.json`: current hashes, full outputs and 33 guard results.
- `standard-properties-20261011-source-02/validation.json`: initial 44-row original/repeat proof.
- `standard-decoder-aligned-entry-return-20261011.json`, constants, enum-name and fallback-angle files: static reference evidence.
- `20261011T2104011229937Z/`: fresh lookup/math dependency survey. Earlier whole decoder and setup surveys remain private.
- `standard-properties-lib-tests-20261011.txt`, `standard-properties-clippy-20261011-draft-01.txt`; preservation `local/scene/preservation-20261011-standard-properties.json`.

Native probe SHA256 `2e03806aa1efbbe50f0875251d5ef5bbdd3a93dc8a4954d979c1e37d3e6b652e`; Rust debug `5e3120b7f803d67f3d1922232646e85113fab835b55b5387ae4cfabd2f25c34d`, release `8ee8e3bb02d47c5bb467402763a82e07ca0ff9d649a9ad50108bb702a62c761b`. Baseline inventory SHA256 `9a0c1ae479c678bfbcdd0601ee2a5f1da11fced072bf2db280e41841bf938667`. Lacetti source SHA256 `75c90a42c35b8c589e717b020e18c0b130190ed996e819e1798f45cd13713689`; private analysis executable remains `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.

Reused Rust/Cargo1.99.0, Bevy0.19.1, Python3.12.14, MSVC14.44.35207/Developer Prompt17.14.41/SDK10.0.26100.0, Ghidra12.1.4/JDK21.0.9 and Capstone5.0.6. Build `tools/build_standard_properties_probe.cmd`, and `cargo build -p grid-probes --bin grid-standard-properties` with and without `--release`. Run `tools/validate_standard_properties.py local/NEW --profile debug|release --controls`, then `tools/audit_standard_properties.py local/NEW-AUDIT DEBUG/validation.json RELEASE/validation.json`. Use fresh private directories.

Next transfer the verified properties through actual Standard constructor/reset/setup boundaries into the retained car/world state. Existing source subset readers and shift initialization are proven separately; this work must join their ownership and initialization order before a source-complete driving claim. See [retained vehicle world](VEHICLE_WORLD.md) and [source shift initialization](VEHICLE_SHIFT_INITIALIZATION.md).
