# Complete owned mesh-shape update (F214)

12 October 2026, Europe/Kiev. The selected complete `B444D0` mesh-shape update now follows source coordinate preparation and whole mesh construction in original/repeated original/Rust comparisons. Each final debug/release suite matches180 cases /999,552 complete bytes, including the Lacetti's9,504-byte shape/mesh result. This is the supplied shape update, not the base constructor, allocation, physical registration or geometric contact generation.

## Behavior and safe ownership

`Mesh::initialize_shape` consumes its privately constructed mesh and creates owned0x250-byte supplied shape storage. It reproduces the update's scalar220, layer19, flagC0, mesh240 and aligned span244 stores, retaining every other selected zero/CD byte. Field names deliberately preserve unknown semantics; scalar220 is not labeled a material pointer. The diagnostic mesh token is checked offset0x250 in the shape+mesh representation; internal mesh references remain owned relative offsets. No process pointer is followed by Rust.

The original update recomputes face normals using `(1 - (seed * squared) * seed) * (seed * .5) + seed`, multiplying the cross product by that refined reciprocal-root value. This differs from the initial builder's negative reciprocal of refined length. It accumulates the mean vertex position in source order and flips the plane vector when its dot product with a point-minus-mean is negative. Operand grouping and multiply-by-minus-one operation are retained, including signed zeros. This orients each plane away from the mean; it does not establish that arbitrary source geometry is convex or closed.

Native CM03 executes unchanged `9B8B70`/`9B8890` deformation-off preparation, `B44E80` and `B444D0` on owned storage. The car, raw geometry, descriptors, boundaries, mesh bytes outside the expected plane lanes and executable sections are checked. Lazy data/protection restoration and existing-output protection remain mandatory. Shape source settings are supplied controls. Full source outer assembly, deformation-grid construction and base/world/material initialization are excluded.

## Verification

Each profile covers42 installed source meshes with both fills and96 authored geometry/order/winding controls. Shape controls vary supplied span1/15/16/17/8192/16000, scalar including negative zero, all selected layer bytes and flag state. Independent complete-shape checks cover all stores and retained bytes; mesh checks cover geometry, outward-plane orientation/length, topology, relative references and padding.64 paired guard outcomes/profile include malformed geometry/framing, source offset/extents, post-adjustment range, span/scalar/layer/flag admission and preservation of existing output. BOS faces11/47 remain explicitly rejected with both fills and implementations; the missing same-stem variant remains recorded.

166 library tests pass. A convex tetrahedron semantic test verifies every vertex lies behind each outward face plane, all non-plane mesh bytes survive and shape settings/padding remain exact. Formatting, all-target warnings-denied Clippy and Python syntax pass. Clippy initially rejected two arithmetic spelling patterns; narrowly documented allowances retain original operand order and arithmetic. Rebuilding changed the debug executable hash, detected by the audit; both final suites were rerun against final binaries. No numerical mismatch or native fault occurred. One guessed Rust filename read failed harmlessly and was corrected from inventory.

Private evidence:

- `local/behavior/mesh-shape-{debug,release}-20261012-final-02/validation.json` and adjacent logs.
- `local/behavior/mesh-shape-audit-20261012.json`: current hashes and all1,080 complete output files.
- `local/behavior/mesh-shape-20261012-draft-01/`: initial Lacetti proof; debug draft02 expanded pass.
- `mesh-shape-libtests-20261012.txt`, `mesh-shape-clippy-20261012-final.txt`, native/Rust build logs under `local/behavior/`.
- `mesh-shape-update-aligned-20261012.json`, `mesh-shape-base-aligned-20261012.json` and surveys `20261012T0314537241595Z`, `0317247141244Z`, `0324491160265Z`.
- `local/scene/preservation-20261012-mesh-shape.json`:130 Steam resources/executable unchanged03:27:21.275619UTC.

Existing Rust/Cargo1.99.0, MSVC14.44.35207 / SDK10.0.26100.0, Python3.12.14, Ghidra12.1.4 / JDK21.0.9 and Capstone5.0.6 reused. No installation, desktop input or game setting changes.

## Next enclosing boundary

Aligned constructor inspection shows `B460F0` invokes the complete base initializer with kind7, then this update. The base reads an owned world/material table, increments the world's shape counter, assigns identities and initializes bounds/list/transform fields. The wrapper `B23D70` subsequently links a wrapper backlink through `A6A190`. These are research findings pending whole constructor/wrapper validation. Next complete this enclosing construction and physical linkage before narrow-phase and barrier integration. The source driving runtime still has no geometric body/barrier response; no complete lap is claimed.

| Final executable | SHA256 |
| --- | --- |
| Native shape probe | `bc9384e039767380607db127bf471bdffb6c68a932c6a88dc822db49254da99b` |
| Rust debug | `9dec82a23b6c64e771da8bb034591a7aee133f4f45a0235677ce313a7c123a6d` |
| Rust release | `ec70d4749d4c1594a81a15ff7f7afb08af2c69928fce032fa76a560c336efd1e` |

F215 follow-up: [complete selected constructor/wrapper](MESH_SHAPE_CONSTRUCTOR.md) now validates the enclosing base/world/material/counter/backlink graph. Physical registration remains separate.
