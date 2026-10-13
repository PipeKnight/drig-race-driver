# Owned car state for the bulk deformation callback

13 October 2026. `deformation_callback_state::calculate` adapts owned car words, physical state, static first-owner contacts, source settings and an explicit CRT seed to the verified serial callback/consumer. It stages a complete owner copy, embeds the authoritative descriptor at2300:C800, carries history at290/2E0, and publishes grid nodes/motions/edges/flags, body velocities, pending shift, contact38 and seed only on success. Unconsumed owner words remain opaque.

Original B784B0 -> A064A0 -> B77490 and its unchanged original helpers match both the earlier typed Rust adapter and this new raw-state adapter, independently in debug and release: **272 cases /11,802,400 complete projected output bytes per profile**, original/repeat/typed/owned exact. Activity:96 changed seeds,77 changed angular states,149 changed node pools,133 changed contact38 arrays. This verifies the adapter mapping for active behavior; driving/physical-phase composition is still pending.

The inputs are independently authored static-first versions of the established callback controls, based on two retained templates. Reverse controls converted to first-owner contacts need positive contact40 when enabled with positive3C; the first draft retained reverse-side zero40 and the existing typed validator rejected it. The authored conversion now explicitly supplies1 for those coefficients. No validator was relaxed and no original fault occurred. The 32-case pilot then passed, followed by both full profiles. These are not newly observed live game states.

## Scope and ownership

The runtime-facing API accepts a bounded D230..21000-byte car, complete424-byte physical state, up to64 static first-owner contacts, settings and supplied scalar/global parameters. Its physical center is owned at1C0; registration/settings pointer roles are canonical and checked. The diagnostic adapts the existing serial wire protocol: explicit supplied centers are temporarily placed in owned1C0 and restored to their original opaque wire words on output; callback/center pointers are normalized then restored. Owner body lookup fields, history/mass/offsets and preparation factors are supplied explicitly. This is a diagnostic relocation contract, not a live ownership/cadence assertion.

The comparison projects the complete A500 descriptor,424 physical bytes,44 history bytes, seed, every selected contact38 and empty pending count. **It does not compare the complete original car allocation.** A Rust unit test separately verifies complete opaque-owner/physical/contact/seed preservation when the callback's descriptor-disable guard declines it, including unused NaN words. Foreign settings roles reject without mutating inputs.

Sixteen guard cases /40 outcomes pass. Twelve malformed shared requests reject in original/typed/owned implementations with ordinary exit codes and no native fault. Two additional owned-only guards reject dynamic peers and pending pairs, which remain outside this adapter's domain. Original guard failures can leave partial private diagnostic output; those files are retained and are not accepted results. Existing output sentinels remain unchanged. Reconstructed audit checks38 files and three identities/profile, requests, guards, complete output equality, current executables and original execution markers.

The rebuilt legacy typed path still matches its retained original **544 cases /23,674,392 bytes per profile**, including dynamic contacts and pending responses. 198 regular libraries (25 formats +4 tools +169 world), formatting, all-target Clippy and Python syntax checks pass. Existing current playable **df108f6656a6** remains available; this adapter is not enabled there and no extra build is needed yet.

## Evidence

Private `local/behavior/` evidence:

| Evidence | SHA-256 |
| --- | --- |
| owned-deformation-callback-debug-20261013-final-01/validation.json | 470881ad5d297c54c58a2320a6b97a84f126418ceaf5fbc27a170e09a15b5b1f |
| owned-deformation-callback-release-20261013-final-01/validation.json | bd1966fde80b0406b81a451f025257fdfcc20bd31c37298e935dd06e05cf1c67 |
| owned-deformation-callback-audit-20261013-final-01.json | 2f8c178ef1e4aa4ed2ba200c2cdabd752bca39ac7def4a589ebe81d204f38316 |
| owned-deformation-callback-legacy-20261013-final-01/validation.json | f144e8ff2e96e4aebe197c8b56511e540fe5e5ae1e6e9b94dafdcf34478fb550 |
| target/debug/grid-owned-deformation-callback.exe | 0bd849e25d6be3e62bc59c6210ea036a13ee50be14095e9ac2db1ffcd527cc9e |
| target/release/grid-owned-deformation-callback.exe | 6521817bf3448c11fc4f652f82004c43912cca0e9ec70cbfe0e4ec72e2b00ad9 |

Reused original serial probe `3f8646fbbd425b614c429e25c7de45ee967cfb9e0cfb1f1376ab66b82ff0f24b`, pinned PE `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`. Two templates: `live-impact-20261009-10/capture.json` SHA`c34887e65bb5bc0f6f7e8274422f4c169b6eb9f7351713e50eb04cc1cf745519`, `live-callback-20261009-02/capture.json` SHA`90e0c43a0ad21299038872d5ddadb83f4f9ead9ef2c4d61d18a643d2cfe5f73c`. The original probe executes real original CRT rand with a controlled private seed and seven name/address-verified Windows clock/error/TLS imports, restoring its data pages/imports/TLS and checking executable sections. This is not an import-free or live-thread-seed proof.

Tools: `grid-owned-deformation-callback`, `validate_owned_deformation_callback.py`, `audit_owned_deformation_callback.py`, existing serial probe. Reused Rust/Cargo1.99.0, Python3.12.14, MSVC14.44.35207/SDK10.0.26100.0 and Bevy0.19.1. No installs, UI input, settings/Steam changes or owner help.

Next: compose this state adapter with the preparation adapter inside the original physical phases, remove the explicit bulk-collection-disabled control only for the new verified path, and compare complete active original outer frames. Then connect source initialization and dirty wheel/shape/presentation consumers in the driving runtime.
