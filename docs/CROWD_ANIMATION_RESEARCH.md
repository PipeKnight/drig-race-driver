# Crowd animation resources and packet evaluation

Crowd animation is researched at the resource and ordinary packet-evaluator level; the playable renderer still omits the crowd subsystem and cloth, and faithful animated poses are not yet verified.

Recorded 23 October 2026. Independently authored findings; no original pseudocode, asset payloads, matrices, key samples, textures or captures are included. Detailed purchased-data checks remain in ignored private evidence.

## Current port and related work

The static object path in `crates/grid-app/src/main.rs::append_objects` loads rigid mesh sources, saved placements and diffuse bindings. `append_entities` selects startup renderables but reports omitted cloth components. `grid-formats/src/mesh.rs::read_static_meshes` explicitly excludes skinnable sources. The supported single-float rigid SkinIndices stream is not weighted crowd skinning.

See [object placements](ORNAMENT_PLACEMENTS.md), [entity components](ENTITY_COMPONENTS.md) and [PSSG payloads](PSSG_PAYLOADS.md) for the existing loader boundaries. [Audio research](AUDIO_RESEARCH.md) §7.5 already covers crowd sound/excitement: its nearby-voice budget is not a visual crowd draw limit. The owner's graphics request is tracked in [the roadmap](GRAPHICS_ROADMAP_RU.md).

## Resource graph

Donington's organism dataset explicitly selects `crowd_anims.pssg`, the European billboard package, the dedicated camera-flash package and the male/female/marshall character variants. Its maleVariant0 is `cr_male_2.pssg`, not the similarly named `cr_male.pssg`.

The shared organism dataset separately names the route crowd placement binary, binary XML crowd definitions, high-level animation selection, prop definitions and hand-held cloth templates. Existing bounded binary XML decoding reads the definitions; names alone do not establish scheduler behaviour or placement-binary strides.

Crowd definitions distinguish standing/sitting, skinned geometry/billboards, platform batches, animation selectors, camera eligibility and source LOD settings. The bank contains named idle, clap, excitement and reaction clips. The route crowd placement format remains unresolved; people must not be scattered at invented positions as a substitute.

## Ordinary packet structure: private decoder verified, production reader unchanged

`grid-formats/src/pssg.rs::read_tree` currently rejects the unsupported packet payload because it is outside the explicit raw-leaf policy. The private probe adds only explicit packet-data names in its own process; no general unknown-node fallback or production parser change was made.

Observed ordinary L4/S4 serialization is big endian:

1. Channel count and declared alignment.
2. Per channel: length-prefixed target and semantic strings, key count, two aligned-layout offsets.
3. Per key: time followed by four float32 values.

The saved offsets describe a separately sized aligned packet layout, not offsets into the variable-length serialized payload. Bounds checks include exact payload consumption, valid counts/names/semantics, finite keys, ordered times, no native-span overlap/escape and agreement with parent size/alignment. Raw serialized float values must be preserved independently of later quaternion conventions.

Private validation decoded 282 ordinary packets across the crowd and front-end animation banks with exact byte roundtrip and no failures. Eighty-five truncated prefixes of a real minimal packet and15synthetic corruptions reject. Clip reference counts and resolution were also checked across four packet-containing banks. Driver/cockpit KA variants and B1 remain unsupported; B1 must not be assumed to mean a Boolean event. A bank may contain unsupported packets and a clip may reference them. Future integration must preserve known opaque envelopes and reject unsupported clips explicitly; silently skipping a referenced packet is not complete clip support.

This proves the selected resource structure, not pose evaluation or rendered compatibility. Public integration still needs independently authored bounded Rust code and synthetic tests; purchased-file fixtures remain private.

## Thirty animation targets and twenty-three weighted joints

The apparent mismatch is resolved by the full source transform hierarchy. All 30 crowd rotation target names match exactly in each of the three selected Donington rigs: 23 JOINTNODEs and 7 ordinary NODEs. Ordinary neck/finger transform nodes are not missing bones. Two further helper NODEs provide named prop pivots.

The ordinary neck NODE is on the weighted head's parent chain. Do not discard ordinary NODEs, zip animation channels with the 23 weighted palette, or truncate seven channels. Maintain a full named transform graph and a separate SKINJOINT-resolved weighted palette.

Raw source bind checks follow the explicit skeleton and joint references. Inverse-bind/source-world products cancel within approximately 3.2e-7 across the selected rigs. Each inverse in the male control uniquely matches its named joint world; rotating the palette by one entry fails clearly. All 1932 male source vertices across five mesh sources retain their bind coordinates within approximately 3.2e-7 under the independently reconstructed weighted bind transform. This is a numerical CPU control over saved float32 inputs, not GPU/native bit parity.

Stored geometry instances share the rig/palette. Source LOD configuration and the number of stored instances differ; actual runtime selection/crossfade must still be traced. Name matching does not establish that every stored mesh should render together.

## Concrete evaluator provenance [D]

The shared master/cache was reused for exactly three targeted read-only decompilations, with lookups first and normal lock handling. No new Ghidra project, broad decompile pass or original executable execution was needed.

| Address | Independently derived role |
|---|---|
| 0x00C7D700 | Ordinary S4 registration initializer, identified by exact type/factory arguments |
| 0x00B33190 | S4 factory, installs vtable 0x00D3E3F0 |
| 0x00D3E3F0 | S4 vtable; slot +0x34 selects 0x00B32B80 |
| 0x00B32B80 | Ordinary S4 bound-slot evaluator |
| 0x00C7D7C0 | Ordinary L4 registration initializer |
| 0x00B34C80 | L4 factory, installs vtable 0x00D3E520 |
| 0x00D3E520 | L4 vtable; slot +0x34 selects 0x00B34770 |
| 0x00B34770 | Ordinary L4 bound-slot evaluator |
| 0x00B2DA90 | Shared adjacent-key search and clamped fraction |
| 0x00CB15FC | Float32 spherical/linear threshold, approximately .999 |
| 0x00AC7080 | Called for a per-channel binding map; internals not traced |
| 0x00D39A74 | NeAnimTimer type-name navigation target |
| 0x00C7C763 | Candidate reference instruction for that timer type; not an evaluator entry |

Factory assignments and vtable entries connect the exact classes to these evaluators; registration string references alone would not have been sufficient. Cached pseudocode remains private in the main checkout's `local/re-cache/00B32B80.c`, `00B2DA90.c` and `00B34770.c`. No original function execution parity is claimed. [Shared RE workflow](re/README.md) describes reuse and address lookup.

## Recovered interpolation [D]

Both ordinary evaluators traverse the resolved channel map, skip negative output indices and write four values plus weight 1 to bound slots. These slots are distinct from the weighted skin palette. One-key channels copy saved values directly.

The shared helper brackets adjacent float timestamps and clamps the interval fraction to 0..1. Exact interior keys choose the previous interval with fraction 1. There is no looping, quantization or clock conversion at this layer.

L4 uses four-component linear interpolation. S4 computes the raw quaternion dot: below the float32 threshold near .999 it uses spherical sine weights; otherwise it uses **unnormalized component linear interpolation**. This function does not normalize inputs/output, apply abs(dot), flip quaternion signs or clamp dot before the inverse cosine. A generic normalized shortest-path library interpolation is therefore not automatically equivalent.

The inverse-cosine and sine identities were reused from existing [joint](JOINTS_RESEARCH.md) and [audio control](ENGINE_AUDIO_CONTROL.md) research rather than inferred from convenient names. The private source-guided math model exercises the near-linear and negative-dot differences; it is not an unchanged-native execution or CRT bit-parity test.

## Pose conventions and remaining uncertainty

Multiple named-rig constant-channel comparisons strongly support XYZW converted to the existing row-vector basis and absolute-like local orientation values. Broad permutation controls support that choice beyond symmetric pelvis quaternions. Simple direct bind-delta multiplication fails those controls. Source hierarchy composition child*parent is independently supported by bind cancellation.

The packet evaluator writes raw interpolated slots without bind multiplication. A later layer can still retarget, blend, convert reference deltas or extract root motion. HLA's Delta label alone does not establish the arithmetic. Quaternion-to-local-matrix reconstruction and the complete pose consumer remain unverified; a faithfully animated skeleton is not claimed.

Time is still **bank time**. Most timestamps resemble an authoring frame grid, but some genuinely do not; retain source timestamps without snapping. The evaluator consumes the incoming scalar directly. Upstream seconds conversion, clip speed, phase and loop policy require the timer/slot consumer or a discriminating original pose observation. The timer type reference above is a bounded next navigation point, not evidence of seconds.

## Independent future work

- **People:** bounded route placement reader, original clock/selector rules, full named hierarchy and verified pose conversion, weighted skinning and source LOD transitions. Validate previous-frame poses in motion, shadows and reflections.
- **Stationary flags:** source entity pool references resolve to two real track cloth templates with topology/attachments and track wind parameters. Rendering currently omits the 24 selected Donington cloth components. Cloth placement binary and solver semantics remain open; an invented sine deformation is not restoration.
- **Hand-held flags:** a separate prop/helper/cloth path depends on real hand attachment. It must not block stationary track cloth. Some configured prop nodes are absent from the inspected prop package; do not synthesize missing meshes from their names.
- **Camera flashes:** a dedicated source quad/texture package and eligibility data exist. Trigger rate/duration, position, blend/depth state and illumination policy remain unresolved. Random screen twinkles or invented point lights do not establish source parity.

Private evidence is recorded in `local/handoff/crowd-animation-audit.md`, `crowd-neanim-l4.md`, `crowd-binding-audit.md`, `crowd-evaluation-audit.md`, `crowd-runtime-evaluation.md` and their private draft/probe directories. Those files include purchased-derived metadata and must remain ignored. This public summary intentionally contains no raw key, matrix, vertex or pseudocode dump.

