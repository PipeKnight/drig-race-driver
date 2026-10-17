# Donington atmosphere and distant walls

17 October2026. Graphics roadmap: lighting, depth and the owner's distant-strip defect. This is not a camera-feature task or a change to physics. Source assets/configuration and original decompilation remain private.

## Observations

The enhanced scene has no DistanceFog component. Its source sky is a separate custom material; the existing atmosphere environment therefore does not establish source distance-fog behaviour on track/object surfaces. The installed Donington lighting.xml contains saved fog controls. The original shader's complete height/distance/radiometry mapping has not been recovered.

| Saved control | Donington value | Pilot use |
|---|---|---|
| fogDistance | 500 | Authored exponential e-fold distance in metres |
| fogDensityCap | 0.4 | Maximum blend through DistanceFog colour alpha |
| fogColour | 0.68235,0.74510,0.90588 | Assumed linear RGB, not verified original colour space |
| fogColourBrightness | 0.5 | Multiplies the pilot RGB |
| fogBaseLevel / fogTopLevel | 0 / 2000 | Preserved metadata; height formula not applied |

The source route ornament attributes give parameter_wall_long LOD distances30/60/950m; track LOD overrides give track/world cull1500m and world clip1800m. The identified wall hits at733–899m are inside these saved outer ranges. This does not justify arbitrarily hiding them. Their original lower LOD and runtime visibility selection remain separate research. Their triangles are a visible source draw, not the already-excluded rigidbody duplicate.

Land/trees is track_infield, with two ground diffuse maps and don_outfield_trees_d as macro colour. The source vertex green clamp0.5 was already checked against the matching infield shader; it must not be removed merely to darken the landscape. The macro texture and saved vertex controls were inspected, without finding a new binding error. Source tree placements and closest meshes are now restored by the forest pilot; distant strips persist.

## Bounded saved-control reader

New grid-formats::track_lighting::read_fog uses the existing pinned quick-xml0.41.0, XML size<=1MiB and<=256single-attribute param elements under one lighting root. Required fog fields must be present and finite; height bounds ordered, distance positive/bounded, density cap0..1 and colour/brightness bounded. Rejects duplicates, unexpected nested structure, DTDs, malformed values and invalid ranges. Unknown other lighting keys are preserved only during parsing and do not gain invented rendering semantics. Actual purchased Donington and negative/synthetic tests pass,2tests/zeroignored. No new tools installed. Changed format/app all-target release Clippy7.11s.

Private evidence: local/graphics/vegetation01/source-lighting.txt, ornament-attributes.tsv/.txt, lod-overrides.tsv/.txt and SHA256 manifest land-wall-source-attributes.json. Earlier PowerShell -Encoding Byte was unsupported in this shell; a binary ornament file was initially read as text. Corrected by inspecting magic and using the installed bounded grid-binxml decoder; no game file was modified.

## Authored distance-only pilot

Explicit --haze source (defaultoff / --haze off) reads the selected track's source controls and requires enhanced track rendering. The renderer uses Bevy0.19.1 DistanceFog, exponential density1/fogDistance and colour alpha=fogDensityCap. Thus the authored blend is cap*(1-exp(-distance/distance_parameter)), about0.253at500m and0.318at790m. This is an adaptation, not the recovered original equation. The source height bounds, original sky gamma/scale and directional fog scattering are not applied. The source sky remains unchanged; reflection-probe/deferred combinations have not been accepted.

Pinned Bevy source applies distance fog after exposed PBR lighting and before HDR tonemapping. Existing track/car extended fragments call that common post-lighting helper; diffuse texture upload remains through texture_upload::image. Colour-space/radiometric assumptions are explicit. Native4K same-camera off/source comparisons must establish whether depth improves without washing out nearby car/road detail. No wall or object is removed. GPU acceptance is pending.
