# Thin antenna temporal reconstruction

19October2026. Owner accepts native DLAA clarity overall but reports a significant trail behind the roof antenna, with no comparable issue elsewhere. Preserve the approved image and diagnose this localized failure.

## Purchased geometry and actual inputs

The read-only grid-surface-texture probe now supports GAME_ROOT --car-material CAR MATERIAL for bounded selectedLOD0 rest geometry. Lacetti Aerials is drawMPJRI, jointx0_aerial_fl, sourcex0_RDS:21vertices/25triangles, identity rest transform, xwidth0.00317745m and height0.241001m. The ordinary material isOpaque (not blended glass), moving with the source car; it is excluded from body deformation. Mesh rest bounds alone are not physical local coordinates. The constructor's NED-to-local offset is[0,-0.39143702,-0.14642298]. First candidate-volume query ignored that offset and found0samples; it was not accepted as evidence of bad/good antenna motion.

Corrected native1280x800/tick502 audit projects the antenna to about0.628pixels wide and45.7pixels tall. SMAA's unjittered frame has no candidate-depth samples; TAA/DLAA's selected jitter phase has41. Their candidate-pixel99th-percentile velocity residual is0.00278pixels. The generic100sample acceptance threshold is inapplicable to this tiny object; raw tool threshold_pass remainsfalse rather than being relabelled. These facts support intermittent raster coverage as a concrete mechanism, not an all-frame proof of the sole trail cause. Private actual-aerial-part02.log/actual-antenna-motion30-01.json retain exact transformed geometry and projections.

## Pinned primary guidance

NVIDIA SDK310.5.3's Programming Guide,18December2025, sections3.6.4/pages19–21 recommends conservative rasterization of motion vectors only for thin features missed by ordinary color rasterization. It explicitly does not promise a complete cure. Section3.15/page30 says the older bias-current-color mask is not intended for the latest models (only presetF mentioned); therefore no framework/SDK fork for a reactive mask is justified here. Installed Bevy0.19.1's SR node currently passes bias=None and its render context is private. Guide extracts stay private; no vendor code/binary is copied into source.

## Default-off motion-only pilot

--thin-motion antenna requires enhanced nativeDLAA driving; defaultoff. A narrow additional pass draws the actual player's Aerials mesh with conservative raster into the existingRg16Float motion attachment after stock prepass, before main color. It uses the main depth attachment read-only/GreaterEqual (reverseZ), not the sampling-only depth copy. No color target, depth writes, thickness/vertex change, new texture upload, global sharpening, SDK modification, physics or input change. Current jittered raster projection and unjittered current/previous motion projections follow the already verified temporal audit convention. Actual per-part GlobalTransform includes model offset; previous rendered frame is extracted separately. Hidden/absent parts do not draw; the ECS regression covers history and visibility. Unsupported conservative-raster devices skip with an explicit log.

This is source/typecheck evidence until GPU comparisons pass. Required: exact off-mode parity to the accepted pipeline, raw depth byte identity, bounded changed velocity near projected antenna, finite values, motion-frame inspection at30/300virtualFPS and native4K; paired physical state/full-lap checks and resource tests. Keep approved756DLAA and ordinary3362 installed until a candidate is verified.

Initial compile rejected old wgpu API assumptions (ownedVertexBufferLayout namespace, optional depth fields and raw buffer slice types). Fixed with installed Bevy/wgpu29 APIs. Also replaced sampling-depth view with actual ViewDepthTexture read-only attachment before GPU. A visibility edit first aborted on a stale multiline-query anchor; the following nominalClippy checked the old source only. The exact one-line retry integrates the test/visibility and actual all-target featureClippy04 passes. No failed compile or aborted edit is treated as runtime acceptance.

Completed source9bec verification:768resource tests/21harness results,0failed/ignored; feature and default workspace all-target Clippy pass. Four short GPU controls pass; offSMAAfinalPNG is byte-identical to mergedbcb control and four physical endpoint records agree across modes. Conservative motion-only pass activates on RTX4070; raw paired depth/validity arrays are byte-identical,91motion pixels change only within x639..640/y436..481 at the antenna. This proves narrow attachment operation and unchanged captured depth, not visual ghost removal or missed-coverage correction at every jitter phase. Gap-phase/orbit/30–300virtualFPS/4K visual acceptance remains. Installed756 unchanged. Private antenna01/motion-only-validation30-01.json and tests01.log preserve evidence. Subsequent paint source uses these exact antenna sources unchanged.
## Scissored antenna and ordinary smoke controls

[G397](../log/G0397.md) captured a Donington chase frame using both full and
scissored 4x scratch raster. The 77 covered pixels, 144 coverage samples and
all four underlying depth values per pixel agree byte for byte. The tile lies
below the skyline. In the matched output crop the maximum difference is one
8-bit channel code. This covers the captured sample and footprint.

The ordinary Automatic smoke pass has now been bracketed by actual same-frame
HDR reads. The first test targeted a physics tick between rendered frames.
The corrected private capture samples tick 777 and joins the source frame 373
to Extract and render frame 374. Its Reduced path submitted 170 sprites in
five groups. Ordinary transparency changed 833 pixels of the antenna tile.
Visible smoke in this view is below the tile, so the next depth control
needs actual covered fragments on both sides of the antenna.

Separate DLAA history controls capture the 787 rear aerial at Le Mans and the
Detroit cutout fence. Each normal/reset pair has equal physics, field, frame
requests and raw DLAA inputs. On the 787, resetting the history makes the
visible shaft sharper and opens a gap near its tip. These moving fixtures will
test broader coverage before any regular renderer change.

## Real isolated Full smoke depth samples

[G403](../log/G0403.md) uses the stock Full SmokeMaterial on a separate
4x scratch tile with the actual Donington antenna. The same three covered
lanes at (936,510) retain foreground smoke and reject smoke between the
background and the shaft. The clear lane retains smoke on either side.
All 92 ordered smoke/shaft samples in the tile passed in both placements.
This is evidence for the controlled Full path.

In two independent repeats of each placement, every recorded main input
at the chosen stamp and both scratch depth planes match. Post-DLAA HDR
differs even between identical placements. The strict main-output
comparison still fails. The next comparison needs a same-render proof of
main attachment ownership or the complete matched temporal history.
Ordinary Reduced smoke, moving glass and the full production anti-aliasing
result still require their own evidence.


## Main attachment ownership for the isolated Full trial

In G409 the installed Main and Behind phases contain zero instances of the
isolated source and Coverage contains one. The same captured render stamp
has zero mutual aliases across six Main and seventeen scratch GPU textures.
The native Early pass reports its actual two HDR endpoints after the
isolated submission. The verified archived before-Early, motion/depth and
positive sample controls match. This single-frame structural result applies
to the G403 diagnostic. The ordinary Static overlay and Reduced smoke
still need their own live evidence.


## Ordinary native owner and its authored reflection response

G411 uses Static car reflections in the actual native scene. The selected
Lacetti aerial has zero authored environment colour. The exact tagged parent
is in both native multidraw phase bins, opaque and depth/motion, at the same
stamped frame. Its actual reflective response and transparent child are absent.
The purchased high/low asset census also identifies source-positive aerial
materials in three models. Live phase ownership, resolved pixel coverage and
temporal history are distinct observations. The next physical validation uses
one of the positive authored members and then checks the ordinary sampled
coverage in motion.

## Reflective source closure in the ordinary native view

[G413](../log/G0413.md) measures both authored DBR9 aerials in the native
Le Mans race. Each parent enters the real opaque and depth/motion phases
and owns a matching sorted reflection child. Their owners are distinct.
At the same render stamp, a conservative CPU projection selects a rear
pixel clear of the front mesh's padded bounds. The next readback must
establish real fragment coverage and the child's contribution before
ordinary smoke and native history.

## Reversible prepared phase routing

[G415](../log/G0415.md) compares three same-build native recordings of a
clear Lacetti aerial. A source-only group can be omitted from both native
prepass and opaque draws while keeping the prepared Stock instance data.
Both prepared maps restore at the actual phase boundaries. The separate
current silhouette retains 77 coverage pixels. Its pre-Early input
difference is confined to 34 pixels on the measured frame. The next
frame returns to Stock, witnesses a native reset, and matches its full
Stock RGB image. Reflective child coverage, ordinary Reduced smoke and
moving cutout/glass controls remain open.

## Actual reflected energy from the selected rear child

[G416](../log/G0416.md) draws both DBR9 children separately in a
one-pixel scratch query using the actual read-only main depth. Only the
rear child passes at the chosen pixel. Two ordinary runs then differ
only in the reached rear range during Stock. Before-Stock HDR and
native Depth/MV are exact. The draw adds color in the measured rear
wire footprint while the destination alpha stays exact. The matched
evidence extends through Stock transparency. Same-frame Automatic
smoke fragment ordering remains open.

## Authored owners in actual native opaque and motion phases

[G418](../log/G0418.md) observes one Lacetti and two DBR9 player aerials as
separate real prepared owners in the normal Baseline and Enhanced profiles.
An early Donington start with eleven Lacetti opponents also exposes two LOD2
owners with bin size two in both phases. Inspect the exact keys and full
owner lists before measuring a whole-group refusal.

The strict byte equality check across two launches fails even for the exact
same disabled-tag binary with identical control and capture ledgers. Measure
native launch variation separately from raw input equality and the visible
motion comparison. Production temporal admission remains open.

## Controlled Stock Reduced overlap at a real aerial

[G419](../log/G0419.md) measures the actual Reduced downsample, sprite and
composition at the G409 isolated shaft. At the measured half pixel the
shaft increases reverse depth from .00198336 to .01931195. The front sprite
keeps .43359375 center opacity. A behind sprite drops from that value to
zero, with a small full-resolution RGB contribution after upsampling. The
same-depth neighboring control is unchanged within each matched arm.
The result scopes the controlled 4× scratch route. Ordinary 1× Reduced
coverage and moving glass remain open.

## Actual shared native keys for thin opponent geometry

[G420](../log/G0420.md) reads both native phase maps at the same Donington
start used by G418. The complete Opaque and Depth/Motion keys each have
the same two Lacetti LOD2 aerial owners. Their mesh and material group
match. Their effective material slots differ. Both phases use distinct
MainEntity keys and the expected Bevy 3D render-side sentinel.
The scoped request must account for both owners before taking an intact
bin. G422 checks the partial-closure refusal below. Per-owner draw evidence remains open.

## Ordinary native Reduced with organic smoke

[G421](../log/G0421.md) witnesses the live Automatic sequence at BMW
Shibuya tick 452. The native 1280×800 single-sample view uses real 640×400
Reduced color and depth, with 1342 active prepared sprites across five
restored groups. The fence overlap in the capture nominates the region.
Per-pixel fence coverage and the exact Reduced color/opacity contribution
remain open.

## Partial shared-group request at the native boundary

[G422](../log/G0422.md) runs the G415 read-only prepared-group inspection on
the real G420 pair. A request for either selected owner alone is refused
by the complete-owner check for the tested subset. The whole actual pair
passes. The two ordered real phase maps agree across all five sampled
boundaries through native Early. The stock HDR endpoints swap with no
extracted reset. Sustained current-frame image and actual per-owner draw
acceptance are still separate.
