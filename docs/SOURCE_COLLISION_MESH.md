# Source collision coordinates (F213)

12 October 2026, Europe/Kiev. `collision_mesh::build_from_source` joins the selected whole original `9B8B70` / `9B8890` adjustment, with deformation disabled, to the complete `B44E80` mesh builder. Original/repeated original/Rust debug and release match all 180 cases / 892,992 bytes per profile. The Lacetti's complete 8,912-byte result matches.

## Boundary and arithmetic

Input is owned raw point/triangle geometry, supplied car offset and bottom/roof, and zero/CD fill. Installed-source cases independently read raw `collision_main` points from NED, offset from the previously verified CSV/NED wheel-layout formula, and supply minimum/maximum raw Y as bottom/roof. Row zero is selected explicitly. This is not proof that the entire original outer factory produces these exact settings or deformation-grid geometry.

The routine obtains the minimum input Y, applies the deformation-off inner adjustment, adds the source offset, subtracts `[0, offsetY + (roof + bottom) * 0.5, 0, 1]`, and forces W to one. Rust retains the original single-precision grouping and signed-zero behavior. The result retains X/Z offset and centers Y for the separate shape transform. Whole native code runs against an owned zeroed car; complete car state, raw points and car sentinels must remain unchanged. The existing whole mesh call then computes every vertex, plane, ordered neighbor, incidence, header, relative pointer and padding byte. No original code replacements, imports or allocation adapters are used; lazy data and protections are restored.

Admission remains explicit and bounded: finite homogeneous points, 4..80 vertices, 4..110 faces, bounded finite offset/extents with W zero and bottom <= roof, valid nonduplicate triangle indices, safe normal magnitude and neighbor capacity, finite bounded adjusted results. No geometry is silently repaired. Both modes explicitly reject BOS zero-area faces11/47 with both fills. Selected adjustment preserves the coincident source vertices. This is safe API rejection, not proof of how original GRID handles the full BOS factory/deformation path.

## Verification

Each debug/release adjusted suite covers42 installed source meshes with both fills, plus96 authored nonuniform scales/translations, supplied offsets/extents, shuffled face order and reversed winding controls. Independent checks validate adjusted source point words, normal length/winding, topology, header/extents and all padding/capacity tails. Each suite passes50 paired malformed-input/existing-file outcomes, including offset/extents validation and overflow after adjustment; native faults do not count as guards. Four explicit BOS rejection outcomes per profile and the missing same-stem `vie.csv` variant are recorded separately.

Fresh CM01 cold-mesh regression repeats180 cases /892,992 bytes and36 paired guards per profile using the current Rust executables.165 library tests, formatting, all-target warnings-denied Clippy and Python AST checks pass. The independent audit verifies all2,160 complete output files and current report/request/executable/source hashes across all four reports. A semantic unit verifies source preservation, shape-local height restoration and invalid adjustment settings.

Private evidence:

- `local/behavior/source-collision-mesh-{debug,release}-20261012-final-01/validation.json`.
- `local/behavior/collision-mesh-{debug,release}-20261012-regression-03/validation.json`.
- `local/behavior/source-collision-mesh-audit-20261012.json`.
- `local/behavior/source-collision-mesh-20261012-draft-01/`: initial Lacetti comparison.
- `local/behavior/source-collision-mesh-debug-20261012-draft-02/`: initial expanded debug pass.
- Adjacent build/validation logs; `source-collision-mesh-libtests-20261012.txt`, `source-collision-mesh-clippy-20261012.txt`.
- Original source adjustment surveys `20261012T0252237251351Z/9b8b70.c.txt`, `20261012T0303587391988Z/9b8890.c.txt`.
- Shape follow-up surveys `20261012T0314537241595Z`, `20261012T0317247141244Z`, `mesh-shape-update-aligned-20261012.json`.
- `local/scene/preservation-20261012-source-collision-mesh.json`:130 reference Steam resources and executable unchanged2026-10-12T03:19:07.921965UTC.

A draft Markdown insertion assumed uniform CRLF and stopped on its assertion without changing the log; corrected using the actual byte layout. No numerical comparison or native call failed. Existing Rust/Cargo1.99.0, MSVC14.44.35207 / SDK10.0.26100.0, Python3.12.14, Ghidra12.1.4 / JDK21.0.9 and Capstone5.0.6 reused. No tools installed, desktop inputs, Steam or user settings modifications.

| Executable | SHA256 |
| --- | --- |
| Native source mesh probe | `e336b63473a824aa8df8e84b9915b3beb607301f0d105c9b22e7c384bfc831bb` |
| Rust debug diagnostic | `e6f42b8841934fb10dc7956a10ec2d82dd0745f93ca78c55ecf06218d6f45006` |
| Rust release diagnostic | `b68e28bedd0786d5e6c3c86d3e0bc061966ccc26a059c8e8c4e73392add0d436` |

## Next physical boundary

Static inspection of the actual attachment path shows `A72EF0` constructs a kind7 mesh shape through `B460F0`, then `B444D0` recomputes face normals with a different reciprocal-root refinement and orients them away from the average vertex position. The initial mesh planes must therefore not be treated as final attached-shape planes. This is a static finding, not a validated shape call yet. Next reproduce complete shape initialization/update, physical linkage and actual narrow-phase/barrier contacts through the retained world. The prototype still has no geometric body/barrier response, and no complete lap is claimed.
