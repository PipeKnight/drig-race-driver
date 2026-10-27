# Crowd animation resources and packet evaluation

Crowd animation is researched at the resource, local timer and ordinary packet-dispatch level; the ordinary playable renderer still omits the crowd subsystem and cloth. A default-off bounded Detroit USA static billboard preview is under validation; faithful animated poses are not yet verified.

Recorded 23 October 2026. Independently authored findings; no original pseudocode, asset payloads, matrices, key samples, textures or captures are included. Detailed purchased-data checks remain in ignored private evidence.

## Current port and related work

The static object path in `crates/grid-app/src/main.rs::append_objects` loads rigid mesh sources, saved placements and diffuse bindings. `append_entities` selects startup renderables but reports omitted cloth components. `grid-formats/src/mesh.rs::read_static_meshes` explicitly excludes skinnable sources. The supported single-float rigid SkinIndices stream is not weighted crowd skinning.

See [object placements](ORNAMENT_PLACEMENTS.md), [entity components](ENTITY_COMPONENTS.md) and [PSSG payloads](PSSG_PAYLOADS.md) for the existing loader boundaries. [Audio research](AUDIO_RESEARCH.md) §7.5 already covers crowd sound/excitement: its nearby-voice budget is not a visual crowd draw limit. The owner's graphics request is tracked in [the roadmap](GRAPHICS_ROADMAP_RU.md).

## Resource graph

Donington's organism dataset explicitly selects `crowd_anims.pssg`, the European billboard package, the dedicated camera-flash package and the male/female/marshall character variants. Its maleVariant0 is `cr_male_2.pssg`, not the similarly named `cr_male.pssg`.

The shared organism dataset separately names the route crowd placement binary, binary XML crowd definitions, high-level animation selection, prop definitions and hand-held cloth templates. Existing bounded binary XML decoding reads the definitions; names alone do not establish scheduler behaviour or placement-binary strides.

Crowd definitions distinguish standing/sitting, skinned geometry/billboards, platform batches, animation selectors, camera eligibility and source LOD settings. The bank contains named idle, clap, excitement and reaction clips. Standing crowd placement framing now has independent CPU controls described below; moving paths remain opaque. Preserve the source positions rather than scattering people at invented positions.

## Ordinary packet structure: bounded payload reader

`grid-formats/src/pssg.rs::read_tree` currently rejects the unsupported packet payload because it is outside the explicit raw-leaf policy. The private probe adds only explicit packet-data names in its own process; no general unknown-node fallback or production parser change was made.

Observed ordinary L4/S4 serialization is big endian:

1. Channel count and declared alignment.
2. Per channel: length-prefixed target and semantic strings, key count, two aligned-layout offsets.
3. Per key: time followed by four float32 values.

The saved offsets describe a separately sized aligned packet layout, not offsets into the variable-length serialized payload. Bounds checks include exact payload consumption, valid counts/names/semantics, finite keys, ordered times, no native-span overlap/escape and agreement with parent size/alignment. Raw serialized float values must be preserved independently of later quaternion conventions.

Private validation decoded 282 ordinary packets across the crowd and front-end animation banks with exact byte roundtrip and no failures. Eighty-five truncated prefixes of a real minimal packet and15synthetic corruptions reject. Clip reference counts and resolution were also checked across four packet-containing banks. Driver/cockpit KA variants and B1 remain unsupported; B1 must not be assumed to mean a Boolean event. A bank may contain unsupported packets and a clip may reference them. Future integration must preserve known opaque envelopes and reject unsupported clips explicitly; silently skipping a referenced packet is not complete clip support.

This proves the selected resource structure, not pose evaluation or rendered compatibility. G263 adds a bounded public ordinary-payload reader, described below; purchased-file fixtures remain private. Bank-level PSSG admission and active clip support remain separate.

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



## Standing placement reader and USA crowd continuation (private draft, 23 October 2026)

A new independent CPU probe resolves the standing crowd catalogue rather than scattering spectators at invented locations. It validates all 97 installed route placement files, accounting for 1,291,209 standing records. Each source position lies in its referenced cell bounds and spatial grid cell; the saved two-component orientation has unit length within the checked tolerance. Every selected standing payload reconstructs byte-for-byte. The same private probe rejects 161 truncated synthetic prefixes and 22 synthetic corruptions. These are format controls, not execution or rendered parity.

Observed framing is little endian with absolute offsets to grid metadata, occupied-cell descriptors and a group catalogue. A cell descriptor is 40 bytes; each group descriptor is 16 bytes; each standing record is 28 bytes, containing an orientation pair, XYZ position, an unknown scalar and occupied-cell reference. Moving groups are separately identified and their bounded payloads remain opaque. Exact group counts, per-cell standing/moving totals, contiguous payload ranges and the terminating name table are checked. The unknown scalar is not treated as animation time or RNG. Four valid Jarama boundary cases require care: float32 arithmetic can round across an authored cell edge. The proposed reader checks that relationship in float64 while preserving all original float32 values.

The private Rust placement reader is independently authored but remains uncompiled/unapplied until the integration owner runs the crate checks. Its synthetic tests and the Python controls are distinct evidence. A production consumer requiring complete moving paths must explicitly reject the opaque moving groups rather than silently claim complete placement support.

Detroit's dataset selects the USA billboard atlas and USA male/female rigs; it does not select the European resources used by the earlier Donington rig controls. Route 8 contains 6,854 saved static instances, of which 4,667 use the male/female grandstand sets. Their instance-set references resolve to the corresponding sitting stances and grandstand animation selectors. Those counts do not establish how many actors any runtime quality setting displays. Separate USA neutral-bind controls now resolve all 30 animation targets and 23 weighted joints; weighted source vertices remain at bind positions within 3.66e-7, and a deliberately rotated palette fails. This is bind-only CPU evidence, not animated orientation/time or GPU parity.

Two explicitly selected saved Shader Model 3 programs were disassembled through the existing installed Microsoft helper, without another Ghidra project. The ordinary billboard vertex program expands a horizontally camera-facing card from each origin using supplied width/height/corner data. It does not consume actor yaw at this stage. The fragment program combines the diffuse texture with a three-colour tint mask, retaining the uncovered mask remainder, then applies its lighting/fog terms. This establishes a narrow saved-variant shader contract; it does not establish palette selection, skin/billboard fade scheduling, clock or arm animation. The unswayed runtime corner table is established separately below.

A default-off, bounded static USA grandstand preview is prepared privately. It retains source positions, sex/group identity and opaque placement metadata; its palette choice is explicitly variant 0 / colour set 0, rather than a claimed recovered variation policy. The candidate remains a static diagnostic: an explicit fixed source-scale fixture, unswayed static corners, Bevy PBR lighting and Bevy A2C are adaptations, and nearby original skinned poses/animation are not reproduced. It must not be promoted as restored animated crowds before independent checks and matched original observations.

### Camera flash runtime material

One targeted shared-cache decompilation of setup function `0x00762370` establishes an important distinction: the flash asset's saved billboard material is replaced at runtime with the `camera_flash` shader group. The pinned shader pack's saved ordinary flash pass uses additive RGB source-alpha/one blending with depth testing/writing. This is a dedicated render effect; it is not evidence of an emitted point light. Setup is not the trigger scheduler. The subsequent targeted work below recovers event cadence and source actor selection; the static preview still fabricates neither flashes nor attachment transforms.

Private inputs, raw disassembly, exact program/asset hashes, structural reports, synthetic controls and unapplied integration drafts are under `local/handoff/drafts/stadium-crowd01/`, indexed by its `manifest.json`; the short handoff is `local/handoff/stadium-crowd.md`. The already pinned shader-pack SHA is `b6b29b80397d12def9c90f5afcff9ea8f335d07da3dc81e293273b468c9029a6`; selected billboard vertex/fragment bytecode hashes are `c621ae6e6c175ab07b21f61337fa44923843430dd1b4c024a1d5ce4279d36808` and `dde97f344b0e5fba010bc778fd2f639892ca05cd594769a7fe0b028c24b6a0e1`. These identify selected saved variants, not live original execution.


### Runtime billboard corner stream

Targeted shared-cache analysis of `0x00759280` identifies the stance table used by the bounded stream writer at `0x00751970`. The table supplies packed half-precision UV/corner pairs to four repeated compact actor descriptors. Its unswayed order is left-bottom, left-top, right-bottom, right-top, with a bottom-centre anchor and dimensions already included in the corner offsets. Horizontal atlas orientation follows the source table rather than an assumed generic quad. The saved four-vertex PSSG mesh has no UV stream and is not itself this generated corner stream.

The separate `0x0075DF70` render-list builder selects LOD/distance/budget lists; it is not the GPU corner writer. Bounded assembly of `0x00773400` shows an upper-corner update, but its clock and selection remain outside this static control. The corrected private preview uses the recovered unswayed layout; the full population variation sequence, animated skinned poses and lighting remain separately unverified. The subsequent scheduler contract does not establish live clock or rendered parity. Rustfmt parsing is not a compiled or rendered check.


### Compact actor descriptors and flash events



Source navigation separates each source actor from its compact render descriptor and expanded billboard vertices. The descriptor carries the origin, three packed tint colours, a packed width/height scale pair and opacity. Per-stance size variation is generated through the source random-scale helper. The private diagnostic now explicitly selects a fixed source-draw fixture; it does not reproduce the original global random sequence or population variety.

Targeted function0x00762710 recovers flash age/expiry, a metric-dependent emission interval, event sorting and GPU row generation. The caller supplies a signed scalar; its engine clock domain remains unverified. Default constructor constants and the separate source spawn/eligibility function are identified privately. This advances the scheduler contract but does not yet establish source flash placement/attachment, full selection, rendered geometry or live parity; the preview still emits no flashes.

### Source camera-flash placement

The source spawn helper0x007528A0 now establishes selection from source occupied cells and actors with hasCamera eligibility, bounded to300 candidates per selected cell. It uses original actor positions and orientation, a source local fallback point, and actual runtime attachment translations when available. Two possible attachment pointers exist; their node names are not yet identified and are not guessed.

Together with0x00762710 this supports a small private CPU behavior draft with caller-supplied source random outputs and signed scalar. It adds neither invented lights nor screen-space flashes. The scalar clock domain, actual full-engine random sequence, live attachment transforms and rendered GPU behavior remain separate checks. The new draft is unapplied and uncompiled; only rustfmt syntax parsing has run.

### Selected camera-flash shader consumer

Two selected saved camera-flash shader programs now close the vertex-scalar ambiguity: the vertex program uses XYZ for projection, forwards the fourth position component as opacity and preserves UVs. The fragment program samples the source flash texture, preserves its RGB and multiplies its alpha by that opacity. The saved ordinary pass then adds the result with source-alpha/one blending. This is an unlit texture effect rather than an emitted scene light. Runtime setup0x00762370 resolves the dedicated camera_flash group and texture binding, connecting the earlier CPU producer with this selected saved consumer. Actual live variant/overrides, colour-space handling, index winding and caller-axis/clock/attachment details remain unverified.

Selected VP SHA:f2399e34d331d9c3d83238afcdd532b6bd6f09337637ccd8bbd127681b54e920; FP SHA:202ea16028955f875ab99ecaf00010786446220f4325cd6c3f3c19f660c57690. Evidence remains private under local/handoff/drafts/stadium-crowd01/.

## Source stance sway accumulator (G259)

Bounded assembly identifies 0x007595B0 as the upper-corner sway phase updater. It adds saved swaySpeed times an incoming scalar to the phase, repeatedly subtracts 360 above 360 or adds 360 below zero, then calls the existing corner writer. Exactly 360 is retained. The incoming scalar is forwarded through the identified controller wrapper; its wall-clock origin is not established. This is independent of skeletal arm animation.

## Source animation timer continuation (G259, 24 October 2026)

One targeted shared-master decompile identifies the confirmed animation timer's absolute input setter/remapper at `0x00ACA930`. It retains requested input separately from mapped output and uses configured input extent, output span/offset and repeat count. The finite-count branch retains the final endpoint, with no general beyond-end clamp. Its integer conversion truncates toward zero; a generic nearest-rounded or normalized modulo implementation would not reproduce that contract.

Bounded assembly separately identifies `0x00ACAC50` as the delta accumulator with enable/stop flags. This distinguishes remapping from advancement, but establishes neither wall-clock seconds nor the actual crowd playback configuration. The upstream caller and mapped-output-to-pose consumer remain unverified. Source corner sway also has its own phase accumulator; it is independent of skeletal arm animation.

Private evidence: `local/handoff/drafts/stadium-crowd01/animation-timer-note-02.md`, `animation-timer-contract-02.json` and bounded ASM. The independently authored Python examples exercise arithmetic distinctions only; no native execution, animation integration or rendered parity is claimed.

## Crowd controller timer/evaluation connection

A targeted shared-master decompile of `0x0074F720` connects the selected controller timer to an evaluation entry. Both modes advance the same timer with the incoming scalar unchanged, then pass its mapped output to `0x00AC3D90`. One mode evaluates a primary pair; another evaluates two selected pairs at that same mapped time before combination. The result is forwarded to a virtual consumer. This establishes ordering and data flow, without identifying the pair's concrete clip/binding types or proving the downstream packet and local-pose consumer.

No wall-clock conversion is established here. The common evaluation entry is described below; the binding and local-pose consumer remain unverified. Private evidence and field distinctions are recorded in `local/handoff/drafts/stadium-crowd01/animation-evaluation-note-03.md`. No skeletal animation implementation or rendered parity is claimed.


## Timer to ordinary packet dispatch

One targeted shared-master decompile of `0x00AC3D90` closes the local timer-to-packet bridge. The selected controller's mapped timer output is passed unchanged through a packet traversal to virtual slot +0x34. Previously confirmed ordinary S4/L4 vtables resolve that slot to the recovered interpolation functions. The traversal clears output weights before dispatch, rather than clearing every stored float4 value; channels left at zero weight must not become valid pose data merely because old values remain in memory.

This connects the recovered local stages when the selected packet has the confirmed ordinary class. Actual crowd clip selection and clock units, full binding semantics, layer combination and the final bound-slot-to-local-pose consumer remain unresolved. No faithful arm animation or rendered parity is claimed. Private evidence is recorded in `local/handoff/drafts/stadium-crowd01/animation-packet-dispatch-note-04.md`.

## Bounded static USA preview (G260, two-view diagnostic)

The observed placement reader preserves standing XYZ, orientation pair, opaque scalar and cell, and leaves moving payloads opaque. An opt-in `--crowd-preview control|static-billboards` adapter selects Detroit route_8 male/female grandstand placements and USA dataset/atlas definitions. Off loads no crowd resources; Control validates source data without adding render assets or entities. Static rendering is bounded to baseline MSAA4, LOD0 reference, subroute0, output and benchmark1..600, with no driving/menu/headless/collision/PERF/compositor or simultaneous fence-coverage diagnostic.

The preview uses variant0/colourSet0 and an explicit source-draw5000 scale fixture; it does not replay global RNG, sway, arm motion, near skinned LOD or camera flashes. PBR curved normals, Bevy A2C and disabled crowd shadows are authored adaptations. Finite coordinates and output vertices are checked before appending each card. Rust synthetic/purchased tests and actual rendered Off/Control/Static comparisons must pass before claiming visible spectators; prior Python framing checks do not substitute for Rust execution.


The integrated Rust candidate passes seven app and five placement tests, including actual purchased controls, and package Clippy. A baseline release executable renders all three modes successfully in two fixed views without WGSL errors. The second view's Off/Control decoded pixels match exactly; the first has one green-channel quantization difference also observed between independent Off repeats. Each Control exactly matches an independent Off repeat, so the bounded neutrality admission records that variability instead of claiming deterministic within-pair equality. Original failed exact checks remain private evidence.

Root and an independent reviewer see recognizable correctly placed people, plausible scale and foreground occlusion without obvious opaque card rectangles or scrambled UVs. Repeated variant0 faces/shirts remain visible. The second angle is a previously recorded authored port camera, not an independently sampled original viewpoint. This establishes a static diagnostic only; actual source animation, nearby skinned actors, variation, flashes, lighting parity and runtime quality/performance remain open. Final integration checks are separate from these frozen-executable captures.


After integration of fresh main F565–F567, the full required gate passes 957 tests, workspace Clippy, four unchanged simulation identities and 24 quick-soak scenarios. A fresh baseline executable completes all six two-view GPU runs without shader errors. The static images are decoded-pixel identical to the preceding executable in both views. The second Off/Control pair remains exactly equal; the primary retains the same documented one-pixel Off-repeat variability. These are bounded static rendering checks, not animation or source radiometric parity. Full provenance and the original failed strict comparisons remain private in `local/graphics/crowd-preview01/post-main/`.


## Bounded ordinary payload reader (G263, 24 October 2026)

`grid-formats/src/anim_packet.rs` reads an explicitly bounded ordinary L4 or S4 leaf with exact parent/leaf/native-layout metadata. It preserves target order and every finite float32 key bit, including signed zero; it does not normalize quaternions, infer seconds, snap keys, interpolate, build poses or add a PSSG-wide leaf fallback. KA/B1 and mismatched families fail explicitly.

Before allocation, the reader checks count/byte limits and complete serialized key spans. It validates native array alignment, header exclusion, disjoint ranges, exact aligned tail, finite nonnegative nondecreasing timestamps and complete payload consumption. Equal timestamps remain resource data with no playback policy inferred. Seven synthetic Rust tests cover roundtrip/signed zero, every synthetic truncated prefix, unsupported families, counts/names/layout, nonfinite/decreasing keys, native tail and cross-channel overlap.

The independently authored private core compiles under pinned Rust1.99.0 and passes all seven tests. A separate serial Rust harness agrees with the checked Python decoder for 282 purchased ordinary payloads: 5,935 channels, 406,808 keys and 2,034,040 float32 words, every target/semantic/native offset, and 282 exact byte reencodes. Eighty-five real truncated prefixes and 15 reconstructed exact prior malformed cases reject. Stronger native alignment/header checks pass the same corpus. Original banks are read in place with hashes checked before/after; no purchased raw fixture or pseudocode is public.

The public integration adds API documentation, an exported Result alias and Display/std::error::Error. Preliminary Clippy rejected two fixed-width chunks_exact loops; the correction uses typed as_chunks for the already exact 20-byte key and 4-byte word spans. The original failure is retained. A fresh harness importing the actual corrected public module repeats all 282 purchased field/bit comparisons and exact reencodes, plus 85 real prefixes and 15 malformed controls, successfully. The required full gate passes 964 workspace tests, workspace Clippy, four unchanged simulation identities and 24 quick-soak scenarios. The production PSSG bank reader and skeletal animation consumer are unchanged. Private provenance is in `local/handoff/drafts/stadium-crowd01/packet-rust-next/public-purchased-run02/`, its `public-verification-results.md`, and `local/graphics/skid-whitening01/prepush/summary.md`. This is resource-format verification, not faithful arm motion or runtime source parity.


## Exact animation-data tree admission (G269)

The PSSG tree reader now treats only the observed ordinary L4/S4 and opaque B1/L4_KA data leaves as raw data, requiring each leaf's exact corresponding parent. Unknown animation-data families and known leaves under wrong or missing parents fail explicitly, even when their bytes could be parsed as a container. Existing unrelated containers, legacy raw rules, bounded tree framing and global attribute IDs remain unchanged. This is framing support, not B1/KA codec support or a skeletal pose evaluator.

Seven authored tree tests exercise exact pairs, dictionary reordering/global metadata, borrowed raw spans, ordinary decode versus opaque rejection, wrong parents, unknown families, every truncated synthetic prefix, escaping envelopes, parent metadata errors, mixed populations and legacy rules. The earlier private candidate's corpus and standalone tests are preparation. Fresh integrated full-project and production-linked purchased-corpus checks are required separately; their actual outcomes are recorded below when complete.

A read-only USA binding audit also establishes that both selected near rigs contain all30 ordinary rotation target names and the root translation target. Names collide between actor models, so binding must be actor/asset scoped. That name coverage does not establish channel slot order, pose consumption or runtime clock semantics. Of18 selected Grandstand animation declarations, six have no exact ID in the selected crowd bank; two further resolved clips involve unsupported B1. No fuzzy name alias, silent fallback or claim that all six original animations are broken follows from this bounded resource mismatch. The concrete source pose consumer remains unidentified.


The integrated full gate passes986 workspace tests, workspace Clippy, four unchanged simulation identities and24 quick-soak scenarios. A separately recorded successful Cargo build identifies the exact production library by compiler-artifact filename and SHA, with unchanged before/after package source snapshots. One harness links that library through `--extern grid_formats`; it contains no copied Tree implementation. Across five purchased banks, the production Tree and ordinary reader agree with the independent decoder for282 packets,5,935 channels,406,808 keys and2,034,040 float32 words, including282 exact reencodes. It retains49 B1 and312 KA payloads as opaque, and validates453 clips with643 ordered packet references;134 clips contain only supported ordinary resource families.

The original full gate is successful. A subsequent private artifact wrapper failed because Windows PowerShell5 treated ordinary Cargo stderr output as a terminating native error; the failed result is preserved. Only the short artifact step was repeated with separate native stdout/stderr/exit handling. The new linked run then completes compile, corpus execution and exact comparison successfully. The unchanged comparator retains its historical draft/direct-module scope string; actual compiler arguments, library receipt and root admission identify this new run as production linked. Its raw label was not rewritten. Exact evidence remains in `local/graphics/crowd-tree01/` and the private `packet-tree-next/head-f4d03c3b-integration01/` directory.

This completes bounded bank framing/resource verification, while the evaluator, concrete pose consumer, runtime binding, clip clock, B1/KA decoding, arm motion and flash rendering remain separate work. No default crowd setting or playable launcher changes.


## Frozen camera-flash material diagnostic (G272)

A default-off `--crowd-flash-preview control|frozen-pulse` diagnostic uses three exact
camera-bearing grandstand placements from the existing Detroit static crowd reader.
Off does not load/register the flash. Control validates source inputs without render
entities. FrozenPulse uses the purchased `flash3.tga` full BC3 mip chain and an
independently authored card adapter: raw RGB, texture alpha times opacity once,
explicit SRC_ALPHA/ONE blending, source depth writing and reverse-Z GreaterEqual.
No scene lights are emitted. Axis normalization, local geometry, double-sided
indices, sRGB, attachment offset and phase0.5 are authored fixtures. They do not
recover source node binding, clock, random scheduler or skeletal animation.

The bounded baseline MSAA4/static reference requires configuration0 (`--aa taa`);
baseline returns before enhanced AA is installed, so the backend here remains
MSAA4. The real default is configuration1. An initial no-AA-argument runner
incorrectly assumed0 and was refused by Control; its failure is preserved.

The first rendered frozen candidate exposed black sky rectangles. Actual Bevy0.19.1
Transparent3d sorts view-space Z plus bias ascending; the source-sky adapter's
positive bias drew it after the depth-writing cards. Correcting that existing bias
to negative preserves its intended background-first order. A real material/sort-key
regression and344 app tests plus Clippy pass. The next three GPU processes pass;
Off/Control raw RGBA match exactly. Frozen changes10,952 pixels, all nonnegative
RGB deltas with alpha unchanged. Root and independent native review find the black
rectangles removed and foreground rails/posts retained. Logs establish three
prepared anchors, not a measured GPU card/draw count.

This admits one frozen baseline snapshot, not animated flashes or original runtime
parity. Private evidence, rejected runs and actual hashes are under
`local/graphics/crowd-flash01/`; G272 records the separate glass regression outcome.
No crowd default or playable launcher is changed.

G272 baseline glass follow-up: historical G271835613... and corrected84d84... both
render the same authored Donington Lacetti side view successfully,120samples each;
decoded1920x1080 RGBA matches exactly. Root native review finds no new glass/sky
edge defect. This is a qualified different-build comparison with new flash paths
defaultOff, not enhanced/DLAA/scene-probe validation. The first attempted old-exe
comparison refused a changed hardlink archive hash before GPU; that failure is
preserved and the lost archive is not represented as the original binary.

## Source cell-selection metric (G289)

### Camera-flash candidate cells and cadence metric

Targeted shared-master analysis of `0x0075C8A0` connects flash cadence with selected source crowd cells. It scans the cell array in source order, selects at most 1000 cells inside a camera-relative distance interval and directional cone, and returns the wrapping unsigned sum of their actor counts. This metric includes every actor in the selected cells; the later spawn function separately filters camera-bearing actors. The distance interval scales with an incoming angle parameter and the cone compares against its cosine. Four-component distance and dot products retain the source pairwise arithmetic order. The constructor supplies near/far defaults of 40/100 and the reference-angle constant has bits `0x3f75be0b`.

A private, independently authored CPU kernel compiled with Rust 1.99.0, edition 2024, optimization enabled and warnings denied through the shared build lock. All seven fixtures passed: inclusive interval and source order, cone boundary, angle-scaled distances, fourth component, 1000-cell cap, wrapping metric and angle forwarding, and diagnostic rejection of invalid inputs. They use explicit synthetic cosine and reciprocal providers. This verifies the bounded kernel's analytical cases, not original floating-point bit parity or live flash behavior. Its finite-input and work bounds are authored diagnostic policy.

The caller's angle convention, signed cadence scalar, source clock, live cell construction and attachment poses remain unverified. No production hook, random timer, guessed point light, animated spectator or default flash behavior is introduced. Before connecting the existing emitter draft, capture actual camera/cell/math inputs and resolve these caller semantics; reuse the existing reciprocal emulation rather than introduce another approximation.
A bounded caller observation at `0x00765140` further establishes a null-checked forwarding wrapper: it supplies the embedded source owner and forwards three pointer inputs, the signed event scalar and the selection angle unchanged to `0x00762710`. It performs no scalar multiplication or angle conversion. The master map has no confirmed entry for this small wrapper; its locally observed return is distinct from a master-verified function boundary. A subsequent exact-target direct-call reference query found no candidates for the wrapper. That negative direct-reference result does not rule out indirect dispatch and does not establish either float's clock/FOV convention. The other updater call remains unexpanded.
## Bounded L4 sampling diagnostic (G303, 27 October 2026)

The existing registered grid-probes/grid-pssg-tree receives one explicit --check-l4-packet234 mode. It links the production Tree and ordinary animation-packet reader directly, then samples one known49-key L4 channel at explicit bank coordinates. Binary interval selection is checked against an independent linear scan; weighted f32 arithmetic and unchanged980key bytes are diagnostic observations. This deliberately separates sampling from the unrecovered producer/units of job+0xc; it does not provide seconds, pose/skinning, S4/KA/B1 or rendered waving spectators.

Exactly one purchased packet234 resource run/195coordinates is planned after a real probe Cargo build; old bracket/corpus tests are not repeated. Existing tree-printing CLI remains intact. Earlier standalone-rustc drafts are superseded by the repository's grid-probes build rule. Actual selected release build and resource run pass:195 coordinates/49 keys with unchanged980key bytes. Independent binary32 reconstruction agrees for every query, interval, fraction and four-lane result. Shared full1137tests/Clippy/four identities/24soak pass; exact receipts and limitations are in G303/G302. Original data and captures stay private under local/.

## Four saved skin influences (G305)

The explicit read_four_weight_mesh API retains all four saved float weights and uchar palette indices through the existing validated mesh parser. The rigid-object and static readers retain their prior behaviour. A meaningful authored elbow fixture distinguishes blended movement from dominant-only approximation and checks exact lanes, signedzero and non-unit sums. This preserves resource data; it does not bind a palette, animate/skin a spectator or recover source clip time. One selected purchased USA female source comparison is planned through the existing grid-pssg-tree probe, using newly extracted independent raw fields. The selected production-linked resource check passes:786vertices,2358position words,3144weight words,3144joint bytes,3468triangle indices and23ordered palette references match fresh independent extraction byte-for-byte. Original resource and all source hashes remain unchanged. Shared full1139tests/Clippy/four identities/24soak passes; exact artifacts/audits remain in G305/G304. A neutral skinned actor is still separate, not established by this preservation check.

## One neutral near actor (G306)

A separate opt-in OneNeutralActor mode prepares one selected USA female mesh through the verified four-influence API, saved actor-scoped hierarchy and23 inverse binds. Source row-vector local*parent and inverse*world palettes must cancel within1e-5, then all786positions/normals are CPU blended once. This app-specific adapter preserves ordinary ancestors and finite raw inverse noise; a noncommuting authored test checks order and wrong inverses. It renders a single ordinary Bevy Mesh with near diffuse/tint DDS/rawUV and an explicitly authored opaque two-sided PBR material/yaw/foot alignment. Default and static cards remain unchanged. Actual462app tests/Clippy/combined-feature build pass. After preserving a private Windows saved-path-verifier failure, GPU-only02 passes both static controls on the same executable, without a rebuild. Actual neutral position/palette errors are3.5762787e-7 and one coherent textured actor is visible in the authored close view. Chairs occlude lower legs, so visible foot contact is not established. Independent actual audit, exact receipts/PNGs and preserved failure are recorded in G306. No original clip clock, runtime active LOD, wave, flash or source pose parity is claimed.

## One authored forearm pose (G307)

A separate one-arm-pose mode applies a fixed20degree local-Z delta to the saved left forearm after neutral rig/vertex validation. Saved local translation and neutral world placement/foot alignment stay fixed; hand/finger descendants are recomputed through the same hierarchy. All four source influences are used for affected vertices, with nonaffected neutral data untouched. One mixed hand/root-pivot fixture tests deformation and an exact unchanged control. This is an authored static pose, not waving or recovered source animation. Actual463app tests/Clippy/combined build and two1920static baselineMSAA4 controls pass.104vertices move in the selected7-node/3-palette subtree, maxdisplacement.13628364source units; neutral placement/error payloads stay exact. Root sees a coherent raised forearm/hand, while legs remain chair-occluded. New neutral PNG is byte-identical to the G306 neutral image. Exact artifacts/audits and limits are in G307; no native animation/clock/default or foot-contact claim follows.
