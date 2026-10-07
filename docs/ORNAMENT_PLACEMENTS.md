# Donington ornament placements and object library

6 October 2026. This extends the [surface investigation](DONINGTON_SCENE.md). Original files remain read only; game-derived evidence lives under `local/scene/`.

## Observed ornament table

`route_0/ornaments.bin` is 120,649 bytes. The independently implemented reader accepts this observed layout, with explicit bounds/contiguity checks:

| Offset / stride | Observed value or field |
|---|---|
| 0, LE u32 | 0; other variants unsupported |
| 4, six LE f32 | Overall bounding values |
| 28, LE u32 | 3,030 total instances |
| 32, LE u32 | 40, the prototype table start |
| 36, LE u32 | 131 prototypes |
| 40, records of 52 bytes | Prototype records |
| Prototype +0 | Absolute NUL-terminated name offset |
| Prototype +4 | Six local bounding values |
| Prototype +28 / +32 | Total instance count / first instance ordinal |
| Prototype +36 / +40 | Absolute transform-data offset / stored static count |
| Prototype +44 / +48 | Two opaque words |
| Static record, 56 bytes | 12 LE f32 (three basis rows then translation), two opaque words |

Prototype ordinal ranges are contiguous and sum to 3,030. Stored static records sum to **1,985**, are contiguous after the headers, and end exactly at the NUL-terminated name table. All 131 prototype names resolve to an `objects.pssg` ROOTNODE ID by adding ` Root`. The object library has two additional unplaced prototypes, left/right 300m brake markers; these are reported and excluded because they have no ornament record.

The initial reader wrongly required the static record's final two words to be zero. Actual data rejects that at offset 0xf07c: 604 placements have a nonzero first metadata word, the second is zero in all 1,985 records. Corrected the reader to preserve both words without assigning meanings. Values resemble packed identifiers/colors but neither interpretation is established. No original data was modified to make the reader accept it.

## Object library and selected rendering

`objects.pssg` explicitly parses to 18,710 nodes after adding DATA as an opaque leaf **only under PNSTRING**. There are two such five-byte NUL-terminated string payloads. Unknown DATA in other contexts remains a container/error. This does not introduce a general heuristic data fallback.

There are 891 mesh sources: 888 rigid sources and three skinnable sources. `read_static_meshes` explicitly excludes SkinnableVertex sources; ordinary `read_meshes` still rejects their unsupported skinning layout. Static object selection uses direct RENDERSTREAMINSTANCE children of LODVISIBLERENDERNODE, selecting the closest LOD only. Lower LOD lists, physics meshes and damage branches remain unselected. Prototype ancestry matrices then compose with exact ornament placements.

The viewer shares each mesh asset across its placement instances. Successful GPU run: **2,608 placed object draws**, 196,111 vertices in selected unique draw geometry, **1,985 static placements**, 450 combined texture images including 45 track textures. Overview image `donington-static-objects.png` was inspected; paddock buildings/vehicles, grandstands, fences and advertisements appear in coherent circuit positions. Close-view validation and owner/reference landmark agreement remain ahead.

Saved texture references are qualified, for example `objectstextures.pssg#…`. The binding reader preserves container and fragment separately. Current viewer container mapping:

| Reference container | Original installed source |
|---|---|
| objectstextures.pssg | tracks/europe/donington/patchup_ot.pssg |
| lemans_sponsors.pssg | tracks/europe/donington/lemans_sponsors.pssg |
| sponsor_minor.pssg | track_sponsors/tag_heuer/sponsor_minor.pssg |
| sponsor_event.pssg | track_sponsors/bridgestone/sponsor_event.pssg |

All four were accessed in the recorded race trace. `objectstextures.pssg` contains metadata matching the 391 textures in patchup_ot; using the latter payload is a viewer resolution rule, not a traced original mount/override implementation. Sponsor choices reproduce the recorded resource pairing and are not a generic event-selection system. The viewer uses saved diffuse maps and a provisional alpha mask for object textures; original normal/specular/AO/state shaders remain incomplete.

## Entity placement path still pending

`objects.ens` is **PSSG**, despite its extension. It contains 836 TEMPLATEBASICENTITYINSTANCEs, 1,045 TEMPLATEENTITYINSTANCEs, 31 references, 240 entity links and 238 constraint links. Its 1,881 TEMPLATETRANSFORM payloads are all 64-byte BE affine matrices. Objecttypes adds 161 such transforms and defines renderable/rigid-body/shape/behavior components. TEMPLATEENTITYREFERENCE points to `objecttypes.pssg#…`; TEMPLATERENDERABLE points into `objects.pssg`.

The 1,045 entity placements equal the difference between ornament total 3,030 and static stored 1,985. This supports complementary static/entity paths; component states, damage selection, links, cloth, collision shapes and original physics remain unimplemented. The current viewer reports those 1,045 omissions. The 836 basic entity instances can overlap static ornaments; do not append all ENS instances blindly and duplicate geometry.

Private evidence: exploratory object/placement/objecttype surveys, `ornament-type-records.json`, route header survey, successful `donington-static-objects-run-4.txt` and image. Three earlier viewer runs failed explicitly on unsupported metadata, qualified references and unplaced library prototypes; their logs are retained. Twenty-two format tests now pass; final lint/test logs are `object-workspace-tests.txt`, `object-format.txt`, `object-clippy.txt`.
