# Donington source vegetation

17 October2026. Missing trees remain a candidate explanation for exposed far boundary strips; no claim that loading trees alone fixes those strips. Source files stay private. Existing validated PSSG/mesh/material readers are reused; no new tools installed.

## Observed resources

Donington trees.pssg404752bytes contains9ROOTNODE prototypes,27RENDERDATASOURCE/RENDERSTREAMINSTANCE,21RENDERNODE and36NODE. Each ordinary species has a closest NODE ending_x0 with trunk/foliage and an alternative_x2card; three distant species onlyhave_x0. Do not render both detail levels at once. All51saved texture references point to treestextures.pssg(4786385bytes,47TEXTURErecords). Normal maps, secondaryUV and source colours have separate semantics; wind and exact shader execution are not yet reconstructed.

Route_0/trees.bin34190bytes differs from ornaments.bin:36byteheader(no leading versionword),48byteprototype records,52byteinstances. Header:6float bounds at0, total646at24, table offset36at28, prototypecount9at32. Prototype: nameoffset0,6float bounds4, total28, firstindex32, dataoffset36, stored40, opaque word44. Instances:12little-endianfloat affine values followed by1uninterpretedword. The matrix maps groups of3to rows and translation values9..11, homogeneous fourthcolumn0,0,0,1. Header+9*48+646*52 ends exactly at34060, followed by130bytes of bounded names. Do not treat the compact instance word as a float or assign a colour meaning without evidence:643instances carry0x680dff, three distant instances0.

New grid-formats/tree_placements reader validates ranges, counts, finite ordered bounds and matrices, unique bounded names, contiguous instance ranges and total boundaries; unknown words stay raw. Three tests pass including actual purchased646placements/9types and malformed count/range/name/nonfinite fixtures; packagealltargetClippy1.72spass. Earlier attempt to read tree_attributes.xml asUTF8 failed because it is binary XML, not evidence of corrupt assets. Binary-XML reader exists; attribute semantics remain to investigate.

Next: select only_x0ancestry from static draws, bind646actual placements, use shared texture_upload helper after GPU format selection, explicit foliage alpha/cull policy, source/offsame-camera comparison. Keep physics untouched. Exact wrap enum meanings, vertexcolour/UV1/wind/normal shader behaviour remain open; do not invent an exact original shader.

Private surveys and parser logs:local/graphics/vegetation01. No renderer integration or GPU-tree success claimed yet.

CPU renderer pilot: opt-in --trees source|off(defaultoff). Source27draws filtered to21closest_x0draws,646instances/1914subdraw placements/212471triangles,18diffuseDDS. Stageall beforemutatingCar; duplicate namespaces rejected. UV1 preserved without shader interpretation; no guessed vertex-colour/tint/wind use. Current authored two-sided Mask0.5 and existing world-repeat sampler are approximations; sourceWrap enum semantics not recovered. Source bounds include very large distant forest cards, extending beyond the anchor header bounds; do not clamp or move them to hide this. Purchasedforest assembly test passes incl second-append rejection; appalltargetClippy5.70s. GPU evidence still pending, no fix claimed.
