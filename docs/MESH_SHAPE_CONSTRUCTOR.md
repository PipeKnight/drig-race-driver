# Complete selected mesh-shape and wrapper constructors (F215)

12 October 2026, Europe/Kiev. `Mesh::construct_shape` reproduces complete `B460F0` / `ABC7D0` / `ABC3D0` base construction, `B444D0` mesh update, then `B23D70` / `A6A190` / `A7E180` wrapper construction. These follow deformation-off source coordinates and complete mesh construction. Each final original/repeated original/Rust debug/release suite matches180 cases /1,123,392 bytes. The Lacetti's complete initial result is10,192 bytes.

This initializes the selected owned graph, with source geometry and supplied constructor settings. It does not execute the allocation wrapper `A72EF0`, full outer car factory, physical shape attachment, broad/narrow phase or barrier response. The driving runtime remains without geometric body contacts.

## Ownership and constructor effects

The0x500-byte diagnostic prefix owns the complete0x250-byte shape, mutated0x84-byte descriptor,0x100-byte empty world,0x20-byte wrapper, material manager and two32-byte lookup records, with retained gaps. Mesh bytes follow. Checked tokens identify the graph; no process addresses are followed by Rust. The native constructor receives actual owned addresses and canonicalizes only independently checked regions/vtables. Executable sections remain unchanged; no code/import replacements or allocator adapters are used. Lazy data and protection are restored before output.

The constructor sets kind7 and initialized bounds/list/control fields, normalizes descriptor homogeneous lanes, optionally publishes its local matrix and identity working transform, copies supplied flags/scalars, selects a material record through the complete original lookup, assigns the current world shape counter and increments that counter. Lookup covers a disabled table, matching first/second record and first-record fallback for a missing identifier. Wrapper construction stores the opaque supplied owner, shape references and backlink, while retaining final padding. Scalar220 and other unexplained fields retain offset names instead of guessed physical semantics.

Source case matrices use the supplied centered collision-height translation. Their other constructor flags/scalars/material lookup are authored controls; complete source outer-settings reconstruction is not claimed. Transform-off cases retain their unused transform bytes, which must not be silently replaced by identity. Mesh point/face counts are now private with read-only getters so external callers cannot invalidate owned storage before shape operations.

## Verification

Each final profile covers42 installed source meshes/both fills plus96 authored geometry/order/winding controls. Constructor controls vary local transform on/off, homogeneous-lane normalization, yaw, supplied scalar/flag/mask fields, span alignment, material hit/miss/disabled paths, counter identity and wrapper ownership. Independent checks cover descriptor mutation, transform publication/retention, world counter, material graph, wrapper/backlink, complete geometry/topology/normals and retained padding. Each profile passes78 paired malformed-input/existing-file outcomes. Nonfinite/bounded scalar and matrix fields, invalid owned tokens, compact flags, count/mode/owner, geometry, adjustment and output preservation all reject normally. BOS zero-area faces11/47 remain explicit separately recorded rejections in both fills, not silently repaired; the missing same-stem variant remains recorded.

167 library tests, formatting, all-target warnings-denied Clippy and Python syntax pass. A semantic unit verifies counter identity/increment, inactive and missing material behavior, wrapper links, source input preservation and transform-off retention. Fresh current-binary regressions pass each profile: CM03 shape update180cases/999,552bytes/64guards, CM02 adjustment180cases/892,992bytes/50guards, CM01 mesh180cases/892,992bytes/36guards. Independent current-hash/source/request/report audit checks all4,320 complete original/repeat/Rust output files across eight reports. No numerical mismatch or native fault occurred. A draft CLI copy temporarily contained two redundant false clauses; removed before final validation. One Windows wildcard tool search failed harmlessly; corrected from inventory.

Private evidence:

- `local/behavior/mesh-shape-base-{debug,release}-20261012-final-01/validation.json`.
- `mesh-shape-{debug,release}-20261012-regression-03/validation.json`, `source-collision-mesh-{debug,release}-20261012-regression-03/validation.json`, `collision-mesh-{debug,release}-20261012-regression-04/validation.json`, under `local/behavior/`.
- `local/behavior/mesh-shape-base-audit-20261012.json`: all current hashes and4,320 complete outputs.
- Initial Lacetti proof `mesh-shape-base-20261012-draft-01/`, expanded debug proof `mesh-shape-base-debug-20261012-draft-02/`, adjacent build/validation logs.
- `mesh-shape-base-libtests-20261012.txt`, `mesh-shape-base-clippy-20261012-final.txt` under `local/behavior/`.
- Aligned evidence and original surveys `20261012T0317247141244Z`, `0324491160265Z`, `0329030701790Z`, `0338404102157Z`, `0342024417834Z`.
- `local/scene/preservation-20261012-mesh-shape-base.json`:130 Steam resources/executable unchanged2026-10-12T03:43:32.843331UTC.

| Final executable | SHA256 |
| --- | --- |
| Native constructor/wrapper probe | `f0899df7e24db75c26ce85c378434d813b2beafb412a24690804f8d7fa03fa47` |
| Rust debug | `f1d7b6000037f90e8e95cfad288f8d42a79dc75510b4947a2b243e8e5761fc47` |
| Rust release | `ee5621838abe540fad0417b626e90c506de7dc44cbca1f9c7c47c97931e0ef2c` |

Existing Rust/Cargo1.99.0, MSVC14.44.35207 / SDK10.0.26100.0, Python3.12.14, Ghidra12.1.4 / JDK21.0.9 and Capstone5.0.6 reused. No installation, desktop input, original Steam or user setting changes.

## Physical attachment remains the next boundary

Static inspection distinguishes callback storage (`A6A250`) from physical registration. `B11F40` has a supplied-shape list branch that writes physical-body328 / shapeD8/E0, publishes transforms and invokes actual broad-phase refresh `B16D70`; earlier cold constructor proofs explicitly had an empty list. Static write candidates nearB142E9/B144FE may be later attachment/removal routines, but their owning functions and behavior have not yet been established. Candidate-only evidence is private `shape-body-link-store-candidates-20261012.json`. Next validate enclosing physical linkage and geometric queues, then integrate with the source driving runtime and controlled lap. No first lap or barrier response is claimed.
