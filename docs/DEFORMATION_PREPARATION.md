# Registered contact preparation

13 October 2026. `deformation_preparation::calculate` now matches whole original **B77850 -> 9A0C90** for one registered owner, a bounded first pool of static contacts and an empty second pool. It carries the complete grid descriptor across contacts and preserves unrelated, skipped and unvisited records. It exposes the advanced cursor and completion flag.

The budget is consumed only by processed contacts belonging to the owner. Skip is likewise owner-specific. A zero budget stops at the next matching, unskipped contact without advancing its cursor. Each processed first-pool contact receives unlimited at44. The callback result is compared to the unlimited sentinel in x87, but its saved f32 value is compared against existing contact3C using COMISS; retaining a double for that second comparison would implement a different rule. Neither budget nor skip is returned by the original dispatcher.

Each debug/release profile independently passes **256 cases /13,186,048 complete output bytes**, original/repeat/Rust exact. The controls cover zero through64 occupied contacts, unrelated records, initial cursors, skip and budget exhaustion, complete processing and existing cap limits. They process1,130 contacts, change241 caps and64 windows;208 cases finish. An independent cursor policy checks every visited/retained record. Full linked-body bytes and all64 contact slots, including opaque unused slots, are compared.

The read-only audit reconstructs every request/guard, checks full outputs and permitted mutation fields, current executables and five input identities:73 files/profile. **22 guards /62 outcomes** include count/cursor/budget/skip bounds, descriptor/node/ring validation, owned pointers, static peers, old-cap finiteness and existing-output preservation. Native canaries cover descriptor/contact pools, body, center, pointer list, manager and cursor storage. Complete restored mapped image equality passes. Original instructions/helpers remain unchanged, with no entry point/import/TLS invocation.

The dispatcher proof does not cover B78530 owner collection/ordering, second-pool dynamic contacts, wake graphs, asynchronous dispatch or whole physical-frame timing. The aligned whole B7B850 schedule is recorded in [Deformation contact limit](DEFORMATION_CONTACT_LIMIT.md). Next compose preparation inside the physical frame before solving, and bulk deformation after successful solving but before correction/publication. Driving runtime callbacks remain disabled; the clean c3696050d8f6 playable build remains current.

Private evidence under `local/behavior/`:

| Evidence | SHA-256 |
| --- | --- |
| deformation-preparation-debug-20261013-final-01/validation.json | b9b780b3cb2ac810b2cd77325ef4e4f58aabafcc3862ec1359d217dc671e33db |
| deformation-preparation-release-20261013-final-01/validation.json | dd3d611de692c7ccb2206f0551674feeb4709e237d13dee7263c5f6e43941fc1 |
| deformation-preparation-audit-20261013-final-01.json | 34e36bc2894ca871b107bfb6a04ea0707a32ad4786316e56551301bb88b2918c |
| deformation-preparation-probe.exe | 01e2bc7d3d899fe8a9f2de71461d8cd20ed53a7c26d4f70e76de47d8f8257921 |
| target/debug/grid-deformation-preparation.exe | f33c48757f3038a519ca48acd9cff1daa08f0564e5c433043b0e7cbe406aabac |
| target/release/grid-deformation-preparation.exe | a0428fe9ae6db4dde3beabf3b020c5f31cc1d0050d65644849de775d995a0b34 |

Native probe/build script, validator and auditor share the `deformation_preparation` name. DC02 has bounded source-only diagnostic framing and canonical owned roles; files stay private. Research: `deformation-preparation-aligned-20261013.{json,txt}`, earlier complete B77850/B78530 surveys. Reused Rust/Cargo1.99.0, Python3.12.14, MSVC14.44.35207/SDK10.0.26100.0, Ghidra12.1.4/JDK21.0.9, Capstone5.0.6; native strict SSE2/x87 53-bit nearest. 196 regular libraries, formatting, all-target Clippy and Python syntax checks pass. The pilot and both full profiles passed without a native fault or arithmetic mismatch. An atomic documentation patch missed its context before writing anything and was reapplied; a guessed private B78530 path was corrected using file inventory. No install, UI input, Steam/settings change or owner help required.
