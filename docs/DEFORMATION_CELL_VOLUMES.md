# Deformation cell intersection and volume

13 October 2026. Independent Rust `deformation_volume` reproduces whole original `A15F50` intersection construction and `9CC790` volume for 5,330 admitted cases in both debug and release. This is the reached geometry calculation in [source node initialization](DEFORMATION_SOURCE_INITIALIZATION.md).

Each profile checks **27,374,880 complete output bytes**: vertex/triangle counts, f64 volume, all 128 vec4 vertex slots and 256 triangle slots. Original/repeat/Rust match exactly, including writes beyond final counts and preserved opaque padding. Coverage: 4,078 vertices, 5,936 triangles, 473 positive-volume cases. Fifty authored box/box-pair controls cover translated, coincident, thin, overlapping/separated bounds; the other 5,280 visit every player Lacetti lattice cell against all 24 source strength boxes.

The algorithm preserves ordered plane triples, strict outside tests, early position-only welding, per-triple plane membership, face center/basis, stable angle sorting, adjacent duplicate filtering, triangle fans and final swap-with-last compaction/remapping. Volume uses the original X contribution with SSE f32 accumulation, clamps nonpositive accumulation, then multiplies the stored f32 one-sixth constant in f64 to model the 53-bit x87 return. Rust f64 atan2 supplies ordering; identical tested faces do not prove every internal CRT angle bit for all possible inputs.

API admission: 3..12 finite planes with W=0/components within +/-2, offsets within +/-1000, exact fixed output pool sizes. Capacity/numeric failures return without mutating caller input. These are admission constraints, not original game error semantics or general convex-geometry equivalence. The native diagnostic executes unchanged code on owned buffers/canaries/read-only plane contexts and restores its private lazy-vector page; the complete mapped image matches afterward. No entry/import/TLS or helper replacement is used.

Eleven guard cases / 29 rejection or output-preservation outcomes per profile pass. The audit reconstructs complete requests, compares all answers and checks all 518 files/current identities/guards. A closed-cube semantic test checks positive volume and unused triangle preservation. Workspace: 193 regular library tests, formatting and all-target Clippy pass. Initial Clippy failures concerned operand-order style and constant-chunk iteration; observed arithmetic order retains documented allowances, and iteration changed without numerical effect.

Private reports under `local/behavior/`:

| Report | SHA-256 |
| --- | --- |
| deformation-volume-debug-20261013-final-01/validation.json | cafe4bbf723e36e705fddaaccadf3f5ba35931cc4ed4fe7323b8334805905ac1 |
| deformation-volume-release-20261013-final-01/validation.json | 61c90b2b2e696dfeaaa5b3c6dcd8d8bf214fecaa1debb1dcba0ca63c56f803ba |

Native probe `f95c2ba8f114c209995ed8bbdf7b58a92b70937f727ec24841a2f20e9331a932`; Rust debug `276badc1dffc3caf1c4bcf129012302cd14e842a5343b3189b30601a707a0b7a`; release `5f54cd89ccc2e3eb5a7492e558b45736ce5e9804733eff36c68bb8b485fdea0c`. Combined first audit `deformation-construction-audit-20261013-final-01.json` SHA `a2767512db8d0e9c24f8fd4c6eb05f979ef6c3ca135a2d61b9da5310942afdf8`; strengthened follow-up additionally pins the current PE and records executable/image counts.

Reused Python 3.12.14, Rust/Cargo 1.99.0, MSVC 14.44.35207/SDK 10.0.26100.0, Ghidra 12.1.4/JDK 21.0.9, Capstone 5.0.6. ABI: cdecl intersection, stdcall volume; original/repeat execution checks owned stack/buffers. Static private aligned/constant reports and fresh surveys retain evidence. Separately decoded interior tail is context only, not a new entry. No installs, desktop input, owner help, Steam/settings changes or approval rejection.
