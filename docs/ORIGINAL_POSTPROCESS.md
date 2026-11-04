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
