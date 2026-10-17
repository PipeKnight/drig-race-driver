# Donington entity startup components and collision descriptors

Recorded 6 October 2026. Independently authored Rust resolves the 1,045 `TEMPLATEENTITYINSTANCE` placements in `route_0/objects.ens` against `objecttypes.pssg`. An independent Python implementation compares every reported name, state selection, matrix bit pattern and collision field against the original bytes: exact match. These are declarative startup records; original execution order and dynamic behaviour remain untraced.

## Reference chain and selection

Both files use ordinary PSSG framing and global schema attribute IDs. `TEMPLATETRANSFORM` contains 64 bytes: sixteen big-endian f32 values in the affine row-vector convention already verified for scene nodes. ENS instance `uri` resolves to a local `TEMPLATEENTITYREFERENCE`, whose qualified URI resolves to a `TEMPLATEENTITY` in `objecttypes.pssg`.

Each referenced template has exactly one direct `TEMPLATEBEHAVIOUR`. Its `typeuri` resolves to `TEMPLATEBEHAVIOURTYPE`. Its ordered direct `TEMPLATECOMPONENTCHANGE` records name an operation (`add` or `delete`) and local `componenturi`. Starting with an empty active set, apply those operations and retain added components that have not been deleted. The reader validates that every target belongs directly to that template. This reconstructs the declared initial selection; it does not implement damage or trigger execution.

Every placement selects one `TEMPLATESUBENTITY`, giving 1,045 initial renderable instances across fifteen renderable URI targets. Twenty-four flag placements additionally select a `TEMPLATECLOTH`; cloth is explicitly omitted from the viewer. There are 240 entity links; their simulation is unimplemented. Basic entity records overlap static ornament machinery and are not blindly appended as extra scenery.

Each subentity has one `TEMPLATERIGIDBODY` and one `TEMPLATETRANSFORMABLE`. Transformable `uri` resolves to a `TEMPLATERENDERABLE`, whose qualified URI names an exact object-library NODE. Its `sid` names the rigid body's scene anchor. Transformables in this reference have no child transforms. Ordinary draws are selected only under the named renderable subtree; damaged alternatives elsewhere in the library are excluded.

Two selected renderables, `dcushion_green_a01_default_01` and `tyrestack_black_default_01`, contain body anchors but no render draws. The first full entity render therefore stopped, rather than silently treating them as complete. The preview now explicitly uses each matching prototype's closest intact LOD for these two known cases. This is a provisional visual substitution; the original runtime's LOD/state substitution has not been traced.

## Collision records

Selected rigid bodies produce **1,021 box collision shape instances**, with no active cylinders in this startup state. All 1,045 selected bodies declare `infMass = 1`. Flags contain link points without primitive shapes. Rigid-body fields include `infMass` (BE u32 boolean) and qualified `props` identifiers. Shapes reference local `TEMPLATESHAPEBOX` or `TEMPLATESHAPECYLINDER` definitions, with a child affine transform on the shape instance. Cylinder definitions occur in other component states.

- Box definitions store BE f32 `width`, `height` and `length`.
- Cylinder definitions store BE f32 `radius` and `height`.
- Both retain `shell`, BE u32 boolean `phantom` and qualified `props` identifiers.

Dimensions are validated as positive finite numbers; shell is nonnegative finite. All matrices and properties are retained. Shape-local to body/world placement, cylinder axis, shell interpretation, mass/inertia, contact response and physical units still require consumer or runtime verification. No collision solver or driving simulation is claimed from merely decoding these values.

## Implementation, validation and evidence

`grid-formats/src/entities.rs` owns the bounded reference/state reader. `grid-tools` includes the `grid-entities` diagnostic binary, writing create-new TSV reports beneath canonical `local/`. Names, matrix/dimension hex bit patterns and properties remain private evidence. `tools/validate_entity_descriptors.py` independently walks original bytes and compares report rows as multisets, so presentation order cannot conceal missing/extra records.

```powershell
cargo run -p grid-probes --bin grid-entities --locked -- 'C:\Program Files (x86)\Steam\steamapps\common\Grid' local/scene/new-entity-report.tsv
```

Synthetic tests verify explicit add/delete ordering, deleted component exclusion, cloth reporting, transforms, shape fields and rejection of malformed URIs, out-of-template state targets, invalid dimensions and booleans. Tests contain no purchased content.

Private evidence: `local/scene/entity-initial-state-native.tsv`, `entity-descriptor-validation.json`, `entity-format-tests.txt`, `entity-start-run.txt` (initial stopped render) and subsequent viewer build/capture logs. `lacetti-donington-entities-start.png` and `donington-entities-overview.png` both exit 0 and were visually inspected. Entity assembly adds 1,633 placed draws / 2,184 unique geometry triangles, with 156 explicitly reported provisional intact-LOD instances (148 cushions, eight tyre stacks). World placement is coherent with the road/scenery; exact original runtime visibility and physical contact remain unverified. Original resources were read only.
