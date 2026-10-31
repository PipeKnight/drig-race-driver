# Surface mechanics source mapping (F083)

`grid-formats::surface::read` reads the mechanics subset of the installed binary-XML `surface_materials.xml`. The existing framing reader checks document structure; this typed layer retains four-byte material codes and optional mechanics. Other material sections and global MUD settings are outside its contract.

The installed file SHA256 is `8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251`. It contains **89 MATERIAL entries, 84 with MECHANICS**, plus one global MUD child. WARN, RESE, RESD, UNRE and FBND have no mechanics. Missing mechanics stays `None`: the original initializer alone does not initialize every contact field.

## Original loader and defaults

Original loader `9b90f0` invokes initializer `99c8b0` and binary-XML views/accessors. These establish the following fields, with offsets relative to the original material object:

| Source attribute | Offset | Default when MECHANICS exists |
|---|---|---|
| grip | 08 | 1 |
| hardness | 64 | 1 |
| bumpstimebased | 90 byte | true |
| bumpswavelength | 94 | stored f32 .05 |
| bumpsmagnitude | 98 | 0 |
| bumpsrealproportion | 9c | 0; clamp supplied value to 0..1 |
| bumpsproportionCarDependent | a0 | 0; no equivalent clamp |
| depth | a4 | 0 |
| tyreTable | a8, four bytes | DEFA |

The tyre table name uses its first four bytes when at least four are present; shorter names retain DEFA. Boolean values are case insensitive. Native parsing rejects unrecognized booleans and nonfinite numbers, whereas the original boolean accessor leaves the prior flag for unrecognized text. Bounds, ASCII restrictions and rejection of duplicate codes/mechanics are authored reader guards.

Numeric conversion returns double before the destination float store; the native reader parses f64 then casts to f32. `depth` resolves the previously unnamed materialA4 input of [contact arithmetic](CONTACT_MATERIALS.md). The loader proof establishes the field mapping, not physical units or depth deformation behavior. No installed material explicitly supplies depth.

## Materials without MECHANICS (F669) [D]

WARN, RESE, RESD, UNRE and FBND have no MECHANICS element. The port used to fail
the tick ("selected surface has no mechanics") when a wheel ray hit one
(RUNTIME_LIMITS_AUDIT R15, FIDELITY_BACKLOG row 9). In the original
(decompiled through `tools/re/grid_re.py`):

- `0x7EF510` registers the 0xB0-byte mechanics component of the material system
  (init and load both `0x9B90F0`). `0x7EF580` counts the MATERIAL children and
  allocates the material array through `0xA72030` -> `0xB3B400`, which
  `memset`s it to zero. It then runs `0x9B90F0` on every MATERIAL.
- `0x9B90F0` always calls the initializer `0x99C8B0` first. It reads attributes
  only when MECHANICS exists.
- The wheel contact producer `0x9C1660` stores the record pointer at wheel +C0.
  It reads +90..+A4 (bump fields and depth) with no presence test, and so do the
  later consumers.

So a material without MECHANICS has a defined record: the initializer's values
on zeroed memory. These are the same values an empty MECHANICS element gets
(the defaults table above): grip 1, hardness 1, heat and wear 1, slowdown speeds
20/40, time-based bumps with wavelength 0.05, magnitude, proportions and depth
0, tyre table DEFA. Its name bytes are "?????" (+24 = 0x3F3F3F3F, +28 = 0x003F),
so no audio material matches it. `surface::Mechanics::initial` is that record.
`contact_surfaces::Surfaces` uses it for every material without MECHANICS. The
mechanics name stays absent. Tests: `a_material_without_mechanics_reads_the_initializer_record` and
`purchased_materials_without_mechanics_read_the_initializer_record`.

Not scanned: which collision chunks use these five codes. Material codes are not
stored as ASCII in `track.jpk`, and a byte search finds none of them, not even
TARM.

## Controlled comparison and limits

`tools/probe_surface_mechanics.c` maps the hash-verified private PE and constructs document/view objects from validated binary-XML records. It executes the original initializer, mechanics consumer, element/attribute/string lookup, boolean and name helpers. It replaces numeric CRT entry `bf3b60` with a host `strtod` adapter returning double. Original entry/import/TLS initialization and live injection are not performed. Code protection is restored after this explicit adapter patch.

**113/113 rows match** the original consumer, Rust reader and an independent Python calculation in debug and release: 89 installed entries and 24 authored controls (23 with mechanics). Controls exercise absent values, real-proportion clamps, unclamped car proportions, boolean casing, nonzero depth, short/long tyre names, missing mechanics and double-to-float rounding. Nine output words plus mechanics names/presence are checked per row; absent mechanics emits explicit diagnostic zeros, not claimed original defaults.

Final evidence: `local/behavior/surface-mechanics-20261007-03/validation.json` (debug), `04/validation.json` (release). Probe SHA256 `11d20a20b482cd74b52e597ad8297dfa838b9ca972226eddaad493adbdc60ad6`. Authored tools: `grid-surface-mechanics`, `build_surface_mechanics_probe.cmd`, `validate_surface_mechanics.py`.

Failed experiments remain private. Initial `01/` used an incorrectly constructed view, so the original never selected MECHANICS; successful process exit did not establish correctness. Corrected `02/` agrees on the installed source. A direct original numeric-converter attempt in `02/original-crt-attempt.tsv` faults `c0000005` before any completed material. Runtime initialization is a plausible dependency, but no exact fault address/cause was captured. The adapter comparison therefore **does not validate the whole original number parser**. An earlier guessed `9c5420` survey was unrelated; string references locate the correct `9b90f0` loader.

Loader evidence: `local/behavior/20261007T1609015335032Z/address-refs.tsv`, `20261007T1610340353294Z/`, `20261007T1611395662945Z/`, `material-loader-instructions-20261007.json`, `mechanics-loader-constants-20261007.json`. No decompiled source or asset captures are included in public source artifacts.

The runtime smooth-dry-tarmac name and bump tuple match both STDR and SHTR. This mapping still cannot uniquely identify the wheel's source code. Original world query/material-token ownership and active tyre-table simulation remain separate work.
