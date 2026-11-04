# Original GRID final-colour merge contracts

Static resource and instruction evidence for the original postprocessing chain;
not a completed renderer port or proof of live effect selection. G348, 2 November 2026.

## Selected resources and current engine gap

The saved ordinary `type2.xml` root selects **CheapFinalMerge**. The saved
`type2_noblur.xml` root selects **FinalMergeNoBlur**. An available block called
Merge2ColourBalance is not thereby the selected final pass. The purchased
`postprocess/shaders/postprocess.nefx` resolves both selected effects to loose SM3
pixel programs, with `vignette.vsh/main_singlePixelOffset` as their vertex program;
these are not entries in the common packed shader collection.

The ordinary merge takes blurred scene, bloom and softening. The no-blur merge
uses unblurred scene and additionally samples the green channel of speedLum's
output, with point filtering for that luminance input. The other three inputs use
linear filtering. Saved output inheritance does not establish runtime target
formats, sRGB state, active quality preset or transition selection.

Current enhanced rendering uses authored Bevy exposure, tonemapping and bloom.
The optional TrackGrade runs after tonemapping and explicitly converts its input
through sRGB for grading. Its current entry point also loads Donington's named
effect instead of resolving a general selected-track chain. These adaptations
must not be presented as the original merge. In particular, changing an arbitrary
bloom or tint multiplier cannot close this gap.

## Finite-input shader algebra

Let W=(0.212500006,0.715399981,0.0720999986), with prepared mix amounts kC/kB/kS.
The ordinary pass computes:

```
A = scene*kC + softening*kS
C = A + bloom*kB*saturate(1-dot(A,W))
```

The no-blur pass first computes S=scene*kC and reads luminance L from the green
channel. Its exposure factor is L, except when toggleThreshold>0 and L>1:

```
weight = saturate(max(S.r,S.g,S.b)-lumThreshold)*lumThresholdScale
factor = 1 + weight*(L-1)
A = S*factor + softening*kS
C = A + bloom*kB*max(1-dot(A,W),0)
```

Threshold scale multiplies **after** saturation. The no-blur bloom gate has no
upper cap, unlike the ordinary gate. Softening follows the thresholded scene.
Both then desaturate toward dot(C,W), multiply by prepared tint, blend toward
vignette colour using a smoothed radial factor, apply contrast and brightness,
and clamp only to the upper maxbrightness value. Output alpha is one. Neither
selected pixel program contains a gamma operation or a lower/nonnegative clamp.
This does not prove gamma is absent elsewhere in the original pipeline.

These equations describe finite-input algebra. Original partial-precision
instructions and different LRP/MAD and smoothstep associations are not bit-parity
contracts, and exceptional-value behavior has not been established. Original
half-texel sampling and vertex clip-position corrections also require deliberate
adaptation rather than copying offsets into Bevy fullscreen coordinates.

## One verified preparation edge: tint

The selected object's constructor at `0x00747C50` creates a parameter subobject
at offset +8. Registration/initialization at `0x00745DC0` gives tintAmount at
parameter+0x24 and packed tintColour at parameter+0x28. Update `0x00747D60`
normalizes unsigned colour bytes by its binary32 reciprocal of 255. Instructions
`0x00748358..0x0074839D` prepare:

```
preparedTint = 1 + tintAmount*(normalizedColour-1)
```

Binding method `0x00748450` maps preMultipliedTint to that prepared field; the
selected ordinary pixel program consumes c9, and no-blur consumes c10. This
supports the current lerp algebra for tint alone; it does not validate its
post-tonemap colour domain. Constructor amount zero yields neutral tint, but is
not proof of active effect defaults or XML override/interpolation behavior.

## Next boundary and retained evidence

Before a normal-path port: resolve upstream luminance preparation,
selected-route effect and chain policy, and actual input/output colour domains.
A small future merge kernel can take explicit prepared samples/uniforms; no new
general render-graph framework is needed. Arithmetic tests, uniform-binding
controls and real rendered comparisons would establish different things; none
has been run for a new implementation here. Row 77 remains open.

Private shader resources and full disassemblies stay outside public source:
`local/handoff/drafts/postprocess-next01/selected-merges01/` and `tint-binder01/`.
The three selected loose programs were disassembled once with the existing tool,
Python 3.14.4 and Microsoft-signed d3dcompiler_47.dll 10.0.26100.9606. Source files,
methods, complete bindings, hashes and the corrected tint address are pinned in
those packets. An independent instruction review confirmed the finite algebra
and its precision limits. No original process, new GPU test or default rendering
change was performed for this research.

## Contrast and vignette preparation (G348 continuation)

Named registration `0x00745DC0`, preparation `0x00747D60` and binder
`0x00748450`, joined to the selected shader constant tables, establish:

- Contrast passes directly from object+0x18 to CheapFinalMerge PS c6 or
  FinalMergeNoBlur PS c7. Brightness passes from object+0x1C to c8 or c9.
  The saved key is misspelled `brighness`; the binder uses `brightness`.
- `vignetteParams1 = (scaleX, scaleY, -0.0-offsetX, offsetY)`;
  `vignetteParams2 = (rotation, 0, 0, 0)`. These bind shared vertex c1/c2;
  vignetteShape binds c0. The negative-zero source constant matters for bit claims.
- Saved vignetteTransition is a scalar tau. The prepared shader vector is
  `(1/tau, 1-1/tau, 0, 0)`, binding Cheap PS c13 or no-blur PS c14.
  Source DIVSS has no local epsilon or clamp; upstream domain admission is unknown.
- Both selected pixel programs consume only prepared vignetteColourTL.rgb,
  normalized from unsigned packed bytes. Other corner colours/link flags and
  vignetteBlendMode exist in common metadata but are not consumed by these two
  programs. A four-corner gradient cannot be inferred from their presence.

For finite inputs, the selected programs form radial=saturate(dot(q,q)),
z=saturate(radial/tau+1-1/tau), and vignette weight=amount*z*z*(3-2*z).
The shared vertex prepares q with rotation, signed offsets, independent scales,
shape and its retained binary32 0.89 constant. Original instruction association,
SINCOS and partial precision remain relevant; these equations are not bit parity.

Constructor contrast=1, brightness=0, amount=0 and transition=1 establish only
initialization, not active XML overrides, interpolation or live uploaded values.
No new kernel, arithmetic test or rendered comparison was run here. Full static
bindings, signs, instructions, input hashes and limitations remain private in
`local/handoff/drafts/postprocess-next02/`. Live chain/route selection, viewport
and sampler dimension preparation, scene/bloom/softening/luminance inputs and
render-target/sRGB domains remain open.

## Dimension-provider association limit (G348 continuation)

A bounded named-binding follow-up located dimension setter `0x0072A920`:
it forms sampler0Dim from two unsigned input words and ones, then sampler0IDim
through RCPPS plus one refinement. Constructor `0x00733A70` installs its table;
caller `0x0073AA00` constructs it and invokes initializer `0x00733B80`.
That actual initializer resolves named depth-prepass resources and motion slots.
This establishes a concrete construction route, not its association with the
selected CheapFinalMerge/FinalMergeNoBlur inputs or exclusive subsystem usage.
The formal class and concrete input descriptor remain unresolved.

Separately, viewportDimensions is registered as semantic index 40; its numeric
producer is not established. Neither route proves window dimensions, a .4/W,H
correction, texture sRGB state or the selected merge render-target domain.
The next direct edge is the selected merge's actual input-handle resolver and
binder. Private source proofs: `postprocess-next03/` and
`postprocess-provider-association04/` under `local/handoff/drafts/`.

## Runtime graph, sampler aliases and output configuration

A bounded follow-up closes the declaration-to-runtime association, while actual
image attachments and their colour domains remain unresolved. The saved Type2
connections are still the selected resource contract: blur.out feeds inColour,
bloom.out feeds inBloom, and softening.out feeds inSoftening. These names are
XML graph aliases, rather than three hardcoded CPU texture bindings.

The generic BlockUnitInput reader preserves each alias and sampler name.
The runtime compiler at `0x008CA010` consumes a normalized description and creates
stages, maps input/output declarations, resolves external resources, wires
internal connections and propagates output configurations. Stage creation
`0x0089B440` consumes the declared shader-group name and optional factory name.
The selected CheapFinalMerge declaration supplies its shader name and no
explicit factory. This is a concrete source path from the saved declaration
to a runtime stage; it is not evidence of live quality or transition selection.

Input mapping `0x008C0950` retains separate runtime strings for graph alias
and shader sampler. Thus the selected inColour/sampler0, inBloom/sampler1 and
inSoftening/sampler2 associations reach runtime port records. Connection wiring
resolves producer and consumer stages and their named ports. Setter
`0x00883830` writes the consumer input's linked producer-output pointer and
clears its external-source pointer. That link identifies a runtime output port;
it is not yet the texture handle subsequently bound for drawing.

Output configuration resolution `0x008BE120` follows the output declaration's
referenced input. For a linked input it inherits the producer output's dimension
and format words; for an external input it obtains those words from the
resolved external resource. Helper `0x008B3320` applies nonzero explicit width
and height overrides, otherwise source scale followed by truncation toward
zero, and preserves format unless an override is specified. Numeric 0x23 acts
as an unspecified-format sentinel in this code; no D3D or DXGI enum name is
inferred from its value.

The selected merge output's config=inColour therefore has a real runtime
inheritance path. Its source reader defaults the two scales to one on that
path. This establishes where dimensions and format come from, but not their
actual values in an active frame. It does not prove Rgba8, Rgba16Float, window
size, padded extents, sRGB state or the viewport/sampler dimension uploads.

## Default-factory association and remaining boundary

The normal PostProcessPlugin resource callback passes its builder to
`0x00728670`, which parses a description, creates a group and invokes the
runtime build route. The group's stored builder is the same object whose
factory field is consulted by stage creation. Builder constructor
`0x008AC4B0` initializes its default-factory pointer at +0x108 to zero.

Stage creation may use a registered default factory at that field, or a generic
fallback when it is null. The actual active factory for selected CheapFinalMerge
has not been proved: constructor initialization does not establish that no
later writer or callback changes it. The fallback must remain conditional
source evidence. A nearby controller interface was checked and rejected as a
factory lead because its first target is an adjust-this destructor thunk.

The remaining source boundary is the actual default-factory registration,
then that selected implementation's output-image allocation and per-frame
port-to-sampler binding. Render-target and sampler sRGB/colour-domain state,
active chain/quality policy and luminance history remain open. These runtime
roles do not justify replacing the current authored grade or adding an unused
source-metadata module. Row 77 remains open; no kernel, Cargo/GPU validation or
original-process observation was performed for this follow-up.

Private evidence is retained in `postprocess-input-resolver70/`,
`postprocess-instantiation71/` and `postprocess-default-factory72/` under
`local/handoff/drafts/`. One shared-master decompile of `0x008CAFC0` completed;
other retained proofs use exact master method bounds or explicitly labelled,
table-authenticated instruction windows. Heuristic disassembly overruns were
rejected and preserved as limitations; neighboring functions are not proof of
the selected runtime path. The existing shader and scalar research was reused.

## External texture registration and CPU backing

The normal initializer registers its named `BackBuffer` resource through
`0x008A82C0`. This stores the resource in a new external-texture wrapper and
inserts that wrapper into the builder's named map. Resolver `0x00892E90`
searches the same map and returns the resource unchanged to the already
recovered external-port wiring. This closes the graph-name-to-resource-object
join; per-frame image-to-sampler binding remains unresolved.

The normal initialization path reaches descriptor setter `0x00BB1E40`, which
populates the resource fields used by the existing width/height/format
inheritance. Its two dimension inputs come through a delegated window
interface; the selected initialization writes internal format code0xC.
These are source preparation roles, not observed dimensions or a D3D/DXGI
format, precision or sRGB identification. The setter conditionally dispatches
resource virtual+0x2C after a descriptor change.

Static type query `0x00B5DB70` returns descriptor `0x00F60EB8`; canonical
constructor `0x00BB0F90` installs table `0x00D473C8`. Its virtual+0x2C entry
`0x00BB3AC0` calculates CPU texture-data sizes, obtains backing memory through
the existing aligned allocator and forwards pointer/size to a resource method.
This narrows the allocation boundary to CPU backing preparation in that
implementation. GPU image creation, upload and sampler colour-space state
are not established by this body. The actual named `backbuffer_placeholder`
lookup accepts the descriptor, but its concrete installed table has not been
observed or proved by the serialized-class/constructor route; this canonical
implementation therefore remains conditional for that resource.

Two further initialization edges do not close the preceding stage-factory gap:
`0x008B07A0` registers parameter handlers, and the+0x108 store in `0x008A82C0`
belongs to its new external-resource wrapper rather than the builder.
The active final-merge factory, input/output colour domains, exposure and
luminance history, and live bloom-chain selection remain open. No runtime
kernel/default change or rendered compatibility follows from this source join.

Private evidence: `local/handoff/drafts/postprocess-registration103/` and
`postprocess-texture-identity104/`. These checks reused exact master instruction
ranges and existing caches; no new Java, original session, GPU test or public
renderer implementation accompanied them.

## Normal effect descriptions and named parameter groups (G373)

The normal host's three conditional payload pairs reach0x00742510. An actual
shared-master decompile plus exact instructions show a serializer adapter:
null destination fails; opening the two-word payload gates a call to destination
virtual+0x40 with the serializer, caller flag and literal1. Normal caller flags
are1,0,0. The payload words' concrete types and these flags' semantic names
remain unresolved. Two later calls pass integer pairs(2,0) and(3,1); these are
not identified exposure, bloom, quality or colour-space enums.

The constructed destination's table0x00CF4938 resolves virtual+0x40 to
0x008AC320. Its actual bounded instructions require PostProcessEffects,
iterate Effect children, read integer id and string name, and create/reuse an
effect object. Optional chain and template attributes follow separate lookup
paths. First-flag virtual+0x30 and lookup-branch virtual+0x48 are observed
calls; reset/removal meanings have not been proved. Child return values are
not checked here, so a host success code does not establish visual application.

Creator0x008A7580 constructs a common0xA4-byte object via0x0087F460 and
registers name/id. It does not choose a different effect class by name. The
constructor installs0x00D0655C; virtual+8 resolves0x00892BC0. A targeted
read-only decompile of that entry returned MISS. The table-authenticated
bounded entry checks Effect then calls0x00892B40. This helper iterates named
ParameterGroup children, resolves each via0x00892AA0 and passes its child
serializer to0x0086AD10. These are actual source dispatch boundaries; the
concrete scalar/colour consumers are now identified below.

Three bounded purchased XML metadata reads match this schema: global effects
has42 children, override2, and the Donington fixture3. The same transition_blur
name has global chain Type4 and fixture Type2. Therefore a global name alone
cannot determine effective route settings. Actual host payload provenance,
overlay order, active effect selection and typed parameter/curve consumers
still need proof. The fixture is not a production route whitelist.

This closes description-to-effect-to-group dispatch, not bloom, exposure,
luminance history, timing units or GPU image/sampler colour domains. Existing
normal grading stays unchanged. No Cargo/GPU/original-window check was needed
for this source-only result; no visual parity or finished postprocess claim.
Private evidence: local/handoff/drafts/postprocess-controller129/,
postprocess-deserializer132/, postprocess-factory133/ and
postprocess-parameters137/. The shared original PE is pinned as in G373.

### Concrete scalar and colour parameter loading

Bounded0x0086AD10 iterates Param children, compares each name with registered
node names, and passes the child's payload to every matching node's virtual+8.
Unknown names produce no write here. Group resolver0x00892AA0 first searches
instantiated groups, then invokes a registered factory and appends its result;
it is not merely a getter.

Reused registration0x00745DC0 joins tintAmount to a scalar node with table
0x00CF2838 and storage parameter+0x24. Its reader0x0086AB10 calls _atof then
stores binary32. tintColour's node uses table0x00CF2844 and parameter+0x28;
reader0x00876BC0 converts four comma/space-delimited integer tokens through
_atol and packs their low bytes in RGBA order. Truncation is not saturation.
Malformed input, locale and exceptional conversion behavior are not a safe
new parser contract.

This closes named XML tint payloads to the storage consumed by the documented
preparation algebra. It does not establish interpolation, effective resource
overlay/selection, clock units or the correct render colour domain. Evidence:
local/handoff/drafts/postprocess-typed-parameters138/. No new decompile was
needed; all retained method windows end at their actual return/padding.


## Resource role order and timeline destinations (G375)

Actual processing callback0x00733500 distinguishes XML chain descriptions from
binary effect descriptions. For binary descriptions it requires the resource's
allocator to equal the owner's TEMPORARY allocator, then compares the resource
role with exact global/global_override strings. Global fills pending+0x84/+0x88
only when empty; other roles fill+0x8C/+0x90 only when empty; global_override
writes+0x94/+0x98. The role callback and later completion callback together prove
fixed application order global -> other -> global_override, subject to their
existing nonnull/query gates. This is independent of resource arrival order.
The second payload word is not established as length; a separate resource field
supplies the trailing-NUL offset.

Both purchased postprocess datasets, ordinary and no-blur, associate effects.xml
with role global and request one extra byte. Neither declares the override file.
Its existence does not prove it loads, and other is not yet proved to be the
track description. Generated track/override requests remain an explicit gap.

The first flag's selected virtual0x008A76A0 requests an effect-registry clear.
Any nonzero effect+0xA0 rejects it with status13 before cleanup; the deserializer
ignores this return and continues parsing. Thus global loading is not proved
an unconditional successful replacement transaction. On admitted clearing the
routine calls effect lifecycle methods, releases backing and clears indices/map.
The semantic owner of the guard field remains unresolved.

Calls0x00895FB0(2,0)/(3,1) select groups by effect ID and pass the second argument
to their virtual+8. The selected ColourBalance group implementation0x00746230
uses byte+0x64 as a one-time registration latch. Its ten named registrations
forward the incoming0/1 as a destination slot to0x0098D1D0. The latter
retains pointers to scalar/packed-colour storage in two slots; it does not read
or reset their values. The apparent receiver helper0x0098CE10 simply returns
the shared registry address. Do not call these operations effect enable/disable,
exposure enums or per-frame interpolation.

The registry's actual writer is the already researched day/night timeline
manager, as documented below. Active effect selection, output GPU colour
domains and exposure history remain open. No renderer,
normal grade or source asset changes accompany these source contracts. Private
evidence: local/handoff/drafts/postprocess-resource-order142/,
postprocess-clear143/ and postprocess-source-slots144/.

### Actual day/night writer, reusing existing clock research

The registry address0x00E85FE0 is the day/night timeline manager already in
[LE_MANS](LE_MANS.md). Existing0x009ACE90/0x009ACFB0 evidence and the actual
0x0090D5A0 runtime branches close this join. The two registered pointers are
destinations, not source values to blend. With keys present, the same sampled
curve writes both destinations: scalar floats directly, bytes with the source
round/clamp rule, booleans with a strict threshold. With no keys it performs
no target write, preserving the XML-loaded values. The valid finite-input
interpolation contract and clock remain those of the existing time-of-day port.

All ten ColourBalance registration names occur in the purchased Le Mans
timeline; attribute occurrence counts are not expanded key counts or evidence
of successful live loading. Static XML values, optional timeline writes and
prepared shader values are distinct stages. This does not identify the active
effect/chain, port GPU colour-domain or establish that the separate timeline
luminance output is the final-merge exposure texture. Private evidence:
local/handoff/drafts/postprocess-timeline145/. No new Java or renderer change.
