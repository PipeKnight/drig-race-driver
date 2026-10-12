# Packed source shape constructor (F228)

The complete original B1F900 constructor now runs on separately owned packed-shape storage, source chunk, base descriptor, world and material context. All 245 Donington chunks pass original/repeat comparisons with independent bounds, scales, relocation and sheet-mask checks: **1,960 cases, 17,859,048 complete native bytes per run**. A subsequent audit reconstructs all requests from unchanged purchased files and checks **3,920 complete outputs and 11 guards**, including the complete mutated descriptor/world/manager context.

This establishes the descriptor ownership prerequisite identified in [F226](TRIANGLE_MESH_MANIFOLD.md). It does not implement a Rust constructor, the complete original material registration/resource factory, or runtime body/barrier collision integration. The earlier F221/F222 whole-generator fixtures retain their documented short-descriptor limitation until a separately validated replacement is available.

## Layout and constructor boundary

`tools/probe_packed_shape_constructor.c` executes unchanged B1F900 and its original base/material helpers on a private mapped analysis executable. The separately canaried shape is 0x2C0 bytes; the base descriptor is 0x94, world 0x100, manager 0x20 and material storage 0x200 bytes. The source chunk has its own allocation and canaries. The legacy 0x280 helper is used only for admission on a temporary object; that object is never passed to original code.

Aligned instructions establish sheet mask at 0x290 and four maximum-float words at **0x294 through 0x2A0 inclusive**, requiring at least 0x2A4 bytes. The chosen 0x2C0 allocation preserves an additional untouched tail. The actual function's return is at B1FE89. An earlier transient note misconverted indices A5..A8 to offsets 0x2A4..0x2B0; aligned instructions and complete outputs correct that interpretation. Scale occupies 0x250..0x25F with W=1, offset 0x260..0x26F with W=0, and decoded min/max bounds 0x270..0x28F.

The constructor mutates the copied source header: negative counts become positive; four relative offsets become owned source pointers; raw material keys become owned material pointers. Source payload beyond the header/material table is unchanged. Canonicalization checks only explicitly owned pointers, with separate shape/world/manager/material/chunk tokens. The opaque owner value remains unchanged. Complete output is 0x674 context bytes followed by the complete mutated source chunk.

No original entry point, game process, allocation adapter, code replacement or import/TLS initialization is used. The existing lazy page E58000 is temporarily writable, snapshotted and restored; the whole mapped image is compared before output. Files use CREATE_NEW. RX code and all allocation canaries remain unchanged.

## Explicit material scope

Each case supplies at most eight per-chunk material records with actual source keys, stride 0x3C and sheet flag at byte 0x38. Other coefficients are zero because this constructor boundary does not consume them. Base settings/context are authored, including world counter 27 advancing to 28. These are owned fixture inputs, not a claim of reconstructing the full game material manager or resource factory. Physical coefficient loading has its separate [F201 proof](SOURCE_PHYSICAL_MATERIALS.md).

The source-policy cases independently decode the 89 installed surface materials and apply exact four-byte key lookup with first-record fallback, as observed in B3B0C0. Every selected Donington source mask is zero under that policy. There are **18 missing-key occurrences**, all raw `MUD` followed by NUL; the XML key ends with a space. No padding normalization is inferred. The fallback record has sheet=false. XML sheet=true entries SHTC/SHTR/SHRT are not referenced under this selected key policy. This is an explicit surface-table policy, not proof of the complete original merged surface/car registration order.

Source, no-sheet, all-sheet and alternating-sheet controls run with zero and CD fill: 245 x 4 x 2 cases. Duplicate keys must have consistent flags. All-sheet and alternating controls exercise mask construction even though selected source masks are zero. Untouched shape tail retains the selected fill. Full constructor equality and independent output checks pass for every case.

## Reproduction and evidence

Build with `tools/build_packed_shape_constructor_probe.cmd`, then run `tools/validate_packed_shape_constructor.py` with a new ignored `local/behavior/` output directory. Audit its validation report with `tools/audit_packed_shape_constructor.py`, again writing a new private output. PS02 requests carry chunk length, fill and explicit sheet mask, followed by the complete source chunk. Guards reject short/long framing, wrong tag, invalid size/fill/mask, zero/excess material counts, malformed vertex offset, nonfinite bounds and an existing output without overwriting it.

Private final reports:

- `local/behavior/packed-constructor-20261012-final-01/validation.json`
- `local/behavior/packed-constructor-audit-20261012-final-01.json`
- Report SHA-256 `137c2076d5f5d05011ae7179372938b7a3e307a41b6957ae4c4a856adc844537`
- Native SHA-256 `bec9b2c85f960abe5535806f1aa648898dc7793bf54c879cb2707e59b4dd9f58`
- Aligned entry evidence `local/behavior/packed-constructor-aligned-20261012-01.json`; existing private B1F900/ABC7D0/B3B0C0 surveys reused.

Existing MSVC 14.44.35207 / DevPrompt 17.14.41 / SDK 10.0.26100.0, Python 3.12.14 and Capstone 5.0.6 were reused. Native build uses C17, strict floating point, SSE2 and warnings as errors. Both initial builds succeeded; the first 40-case draft and final sweep/audit passed without an original execution fault or numerical comparison failure. Two Python AST checks and Git whitespace checks pass. Harmless guessed filename reads and a documentation patch with an incomplete line context failed without changing evidence; corrected against actual files. No Rust changes, installation, interactive desktop input, user-setting change or Steam modification was required.

Follow-up: [F229](PACKED_GENERATOR_COMPLETE.md) verifies a full, nonaliased owned generator fixture and its composed physical-solver handoff. Query witness/normal reorientation and retained multi-triangle manifold integration into fresh moving physical frames remain next.
