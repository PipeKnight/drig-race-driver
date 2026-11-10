# Donington source vegetation

17 October2026. Missing trees remain a candidate explanation for exposed far boundary strips; no claim that loading trees alone fixes those strips. Source files stay private. Existing validated PSSG/mesh/material readers are reused; no new tools installed.

## Observed resources

Donington trees.pssg404752bytes contains9ROOTNODE prototypes,27RENDERDATASOURCE/RENDERSTREAMINSTANCE,21RENDERNODE and36NODE. Each ordinary species has a closest NODE ending_x0 with trunk/foliage and an alternative_x2card; three distant species onlyhave_x0. Do not render both detail levels at once. All51saved texture references point to treestextures.pssg(4786385bytes,47TEXTURErecords). Normal maps, secondaryUV and source colours have separate semantics; wind and exact shader execution are not yet reconstructed.

Route_0/trees.bin34190bytes differs from ornaments.bin:36byteheader(no leading versionword),48byteprototype records,52byteinstances. Header:6float bounds at0, total646at24, table offset36at28, prototypecount9at32. Prototype: nameoffset0,6float bounds4, total28, firstindex32, dataoffset36, stored40, opaque word44. Instances:12little-endianfloat affine values followed by1uninterpretedword. The matrix maps groups of3to rows and translation values9..11, homogeneous fourthcolumn0,0,0,1. Header+9*48+646*52 ends exactly at34060, followed by130bytes of bounded names. Do not treat the compact instance word as a float or assign a colour meaning without evidence:643instances carry0x680dff, three distant instances0.

New grid-formats/tree_placements reader validates ranges, counts, finite ordered bounds and matrices, unique bounded names, contiguous instance ranges and total boundaries; unknown words stay raw. Three tests pass including actual purchased646placements/9types and malformed count/range/name/nonfinite fixtures; packagealltargetClippy1.72spass. Earlier attempt to read tree_attributes.xml asUTF8 failed because it is binary XML, not evidence of corrupt assets. Binary-XML reader exists; attribute semantics remain to investigate.

Next: select only_x0ancestry from static draws, bind646actual placements, use shared texture_upload helper after GPU format selection, explicit foliage alpha/cull policy, source/offsame-camera comparison. Keep physics untouched. Exact wrap enum meanings, vertexcolour/UV1/wind/normal shader behaviour remain open; do not invent an exact original shader.

Private surveys and parser logs:local/graphics/vegetation01. No renderer integration or GPU-tree success claimed yet.

CPU renderer pilot: opt-in --trees source|off(defaultoff). Source27draws filtered to21closest_x0draws,646instances/1914subdraw placements/212471triangles,18diffuseDDS. Stageall beforemutatingCar; duplicate namespaces rejected. UV1 preserved without shader interpretation; no guessed vertex-colour/tint/wind use. Current authored two-sided Mask0.5 and existing world-repeat sampler are approximations; sourceWrap enum semantics not recovered. Source bounds include very large distant forest cards, extending beyond the anchor header bounds; do not clamp or move them to hide this. Purchasedforest assembly test passes incl second-append rejection; appalltargetClippy5.70s. GPU evidence still pending, no fix claimed.


## First GPU comparison and facing foliage (17 October 2026)

The 6cc686b pilot rendered the recovered historical chase-camera view at native 3840x2160, with source trees off/source and forced RGBA8. Images `local/graphics/vegetation01/far-off4k01.png` and `far-source4k01.png` were inspected. Trees appear in their source placements, but the far strips remain and broadleaf crowns are absent. This is a failed foliage-quality acceptance, not a finished forest or strip fix. The shared uploader reports 537 RGBA-decoded images, zero BC assets; the only warning is the existing nil-UUID gilrs mapping warning.

Static source evidence establishes the missing vertex operation. Facing-foliage buffers contain four repeated centre positions per leaf quad, intentionally nonfinite saved normals, diffuse UV0 and signed corner offsets in UV1. `facing_foliage.fx` materials map to this distinct family. The installed shaderpack's `tree_facing.tree_facing.cg.vp`, disassembled with the existing D3DDisassemble helper, expands the offsets along inverse-view right/up after instance scale when wind is zero. Evidence: `foliage-shader-survey.json`, `tree-facing-vp.asm` and its pinned metadata; no shader bytecode/disassembly is published. The ordinary `tree_foliage` program is different and does not justify treating all trees as billboards.

The adaptation tags only the four facing source draws. Each placed facing draw owns a mutable mesh; CPU presentation expands its original centres using UV1 and the current main camera, maps the world offset back into local space, and uses source radial normals. Conservative source bounds include the maximum corner radius. A rotated/scaled-camera math test and the purchased forest test pass; actual facing placements are verified orthogonal/uniform-scale. Wind remains zero. This is not exact original wind, wrapped lighting, or per-reflection-view facing (reflection views use main-camera geometry). Sun shadows (row 122, F747): the original faces each pass to its own view, so in the shadow pass the quads face the sun. Cast from the camera-facing mesh, tree shadows turned with the camera and nearly vanished from views where the quads stood edge-on to the sun (owner report on b6eaecae). Each facing placement (`trees/facing`, `trees/sheet`, `trees/card`) now has a sun-facing shadow twin (`source_trees::ShadowTwin`): the same centres expanded along the sun's right/up, or about world Y towards the sun for sheets and cards, with the atlas view seen from the sun for cards. The twin is rewritten only when the sun turns, sits on `car_batch::shadow_layer()`, and replaces its part in every sun cascade list (`swap_shadow_twins`, after Bevy's light check and before `shadow_cull`), so it casts when and where the part would. Bevy's render-world shadow queue drops meshes whose layers the light does not see, so a twin on the hidden layer would cast nothing. Keep the pilot opt-in until new images are inspected.

Checks on 6cc686b before this correction: 454 workspace tests passed, workspace all-target release Clippy passed in 18.26 s, solo output exactly matches the baseline (a4a35f899a490c36, two laps 88.42029812185638/86.28228501935392). Quick soak repeats the known F342/F345 marker-board fall: seed12, tick3003, 1/12 failures, 58003ticks. Not an all-green soak result. Changed-app facing tests: 2 passed, zero ignored; app all-target release Clippy 5.03 s.

Failed local attempts retained: cargo td has no grid-app library target, corrected to release --bin grid-app; a bounds .into() type ambiguity and nested Assets borrow failed compilation and were fixed. Automatic approval rejected reconstructing main.rs from HEAD due to possible loss of uncommitted work. Instead, a full private backup was made and only proven whitespace-only formatting hunks were removed, with an assertion that every non-whitespace character remains unchanged. No owner work was discarded. Two guessed diagnostic filenames did not exist; existing tools/survey_pssg_payloads.py and shader disassembly helpers were then reused.

Corrected GPU capture: 437b20061075 / executable SHA256 42779513f732b75a3cb211cc74e61c4d3f0cbe596e2f73a995f1a67abf5f81fe, release build 1m01s. Native4K far-facing4k01.png inspected: broadleaf crowns now visible, diffuse alpha silhouettes render and positions agree with trunks. Far wall strips remain visible between trees. ForcedRGBA537/zeroBC; only known nilUUID gamepad warning. This establishes the zero-wind geometry correction, not complete forest-lighting/moving acceptance. Pilot stays opt-in. Main meanwhile advanced with F348–F356; rebase/integration checks must use that new engine base before promotion.


## Rebased validation and remaining work

Merged engine main bee2e56 into graphics 21b0ed70b932; the only conflict was the journal, resolved by retaining both additions. New cameras/HUD/pause menu, four circuits, prop contacts and rigid loose-wheel behaviour belong to the other agent and are preserved. 518 workspace release tests pass (zero ignored/failed); all-target workspace release Clippy passes in23.60s; source-stamped app build1m01s, SHA25685451fe658c5174ea9b455702c93d380c38895c550a42297a7eb4e814553c6ad. Exact solo state/laps remain unchanged. Fresh quick soak passes20/20,100000ticks; the former seed12 residual is resolved by the upstream physics work. The new CLI uses --out, not --output: the mistakenly supplied latter option was ignored and the completed report is actually local/soak/quick-1792234392/summary.md. Read and retain that real report; no redundant rerun.

Two rebased native4K views inspected (merged-far4k01 / merged-side4k01). Combined two-texture/mild-wheel/source-forest run completed6500ticks/104simulatedseconds, one valid89.955257484s lap, terminalnone, four wheel contacts at exit. Pose/velocity/lap endpoint equals the earlier texture-only run despite the newer main engine base; wall140.72s is not displayFPS. Final lap4k01.png inspected. The same whole-lap endpoint is useful non-interference evidence, not proof that every moving leaf is artifact-free.

The source route binary XML was decoded with the existing grid-binxml and tools/binxml_tree.py: ordinary trees have10/15m near and30m next LOD distances,1000m final; distant forest templates1000/1000/10000m. The pilot deliberately uses only closest geometry. Private tree-attributes.tsv/.txt. The source foliage shader has LightWrap0.3 and InvFoliageDensity0.3 for the inspected maple instance, plus backlight sun/ambient colours and a normal texture. Its fragment stage doubles alpha and uses alpha-to-coverage with GREATER0; authored Mask0.5 is not the original coverage policy. Main-camera-facing shadows/reflections and source colour-space/radiometry remain open, especially the excessively dark crowns visible in side view. Do not silently relabel diffuse textures as linear or claim the original lighting is recovered.

Inspected original owner-video frames65/70/75seconds under C:/Dev/Reassembly/local/recordings/donington-race-20261009/overview: dense distant treeline and tan boundary wall. Cameras differ from our diagnostic, so not a pixel match. Wall's visible draw !pE is directly under LODVISIBLERENDERNODE; physics duplicate !oE is in a separate rigidbody branch and is already excluded. Its EnvironmentColour is zero; diffuse map itself is opaque512BC1, so a guessed tint or alpha-hole explanation is not established. Distant-tree diffuse has44.96%alpha>127 and51.59%alpha>0, not an entirely transparent texture. Continue source visibility/treesheet/lighting investigation rather than arbitrarily removing the wall.

Optional launcher mode: `"Play GRID Graphics.cmd" "" "3840x2160" trees` (command shell invocation; source assets required). Standard/Wheels/Textures modes remain unchanged. This exposes the experimental forest without declaring its shading finished. Next: moving-facing acceptance, source backlight/normal/coverage reconstruction and distant treesheet/visible-wall comparison.

Motion sequence moving30-01:30 unique1080p frames, tick602..662, request-clock audit passes <2ns. Frames0/15/29 inspected: foliage is distant/partly hidden by buildings and an opponent; strong black/white crown shading remains. This does not satisfy close foliage motion acceptance or monitorFPS30..300. All rebased GPU logs checked: zeroBC poststartup, known nilUUIDgilrs warning only, no other WARN/ERROR.

Lighting draft after a2c61b2: pinned Bevy prepare_world_normal flips no-normal-map double-sided back faces; original tree_facing VS radial lighting has no corresponding face inversion. Pilot now separates original shader families into leaf/bark material namespaces, keeps culling disabled but preserves supplied normal orientation, and tests an authored PBR diffuse-transmission0.3 leaf lobe/roughness1/reflectance0.1. This is not a mapping of original LightWrap/InvFoliageDensity into physically equivalent values. RGB colour space, normal-map decoding and alpha coverage remain unchanged; same-camera GPU comparison pending. Current main playable remains the verified21b0ed7 build while the lighting candidate is isolated.

Lighting a3e5ae3 GPU: source-stamped release1m00s/SHAff24333560d4aad06514ccacc7072b8a21950d27488d981cab4decb0cc533ab8, changedapp2tests/Clippy4.42s. Same-camera native4K lighting-side/far inspected: stark black/white crown patches are replaced by more coherent green shading. RGBA537/0BC and knownnilUUIDwarningonly. Two30frame1102..1162 motion sequences audited; first negative look direction faces grandstand, second positive1.1 sees MCN-side foliage and frame15 inspected. No universal temporal/monitorFPS acceptance claimed. Private lighting-validation01.json. Far strip CPU ray stillhitswallthenland; static distant sheet geometry does not intersect that ray. Facing leaves are still collapsed at this pre-presentation CPU pick, so it is not a complete foliage-occlusion test.

486e7eb combines the improved radial/thin-leaf lighting with restored validated mips. Static BC/RGBA/native4K views inspected; 520tests/Clippy/exactsolo/20of20soak pass. Source foliage motion before/after mips has identical frame-clock/physics metadata and30uniqueframes, not measuredmonitorFPS. Far wall strips persist. Optional forest remains off by default; no exact original coverage/wind/normal-map or universal foliage-quality claim. The general mip-reader change also restores eligible rumble/object maps without modifying source pixels; see PSSG_PAYLOADS.md.


## Coverage-loss investigation (17 October 2026)

Source foliage uses a different coverage policy than the pilot Mask0.5. Read-only texel statistics confirm species-dependent loss under our threshold: maple_green base25.45%,8x8 9.38%,4x4and smaller0%; birchbush base27.1%,4x4and smaller0%; persimmon base29.2%,8x8 14.1%,2x2and smaller0%. Ordinary birch instead grows from42.6%base to50%at8x8. Distant-sheet coverage stays close to45% through64x32 before coarse-level quantization. Therefore blanket alpha boosting is not justified. The original distant DDS diagnostic had only its base level; a fresh export through the current validated reader recovered all11levels. Both reports are retained, not compared as if identical inputs.

New authored read-only tools/audit_alpha_mips.py uses Pillow12.3.0/NumPy2.3.5, bounded legacy2D BC1/BC3 DDS, exact mip payload accounting and create-new private report. Measurements count texels alpha>=128, not bilinear screen samples, perspective, subpixel geometry or actual moving coverage. Private alpha-mip-coverage01.json/02.json and coverage-*.dds; existing export_texture release build0.57s, no tools installed or source assets modified.

Technique background: [Castaño, Computing Alpha Mipmaps](https://www.ludicon.com/castano/blog/articles/computing-alpha-mipmaps/) describes preserving threshold coverage by rescaling lower-mip alpha. [Wyman and McGuire, Hashed Alpha Testing, I3D2017](https://research.nvidia.com/sites/default/files/pubs/2017-02_Hashed-Alpha-Testing/Wyman2017Hashed.pdf) distinguishes reduced alpha variance from subpixel geometry and discrete rasterization losses; texture correction alone cannot solve all three. These are technique sources, not evidence of GRID's implementation.

An explicitly opt-in `--trees coverage` pilot keeps source geometry, base alpha and every decoded RGB texel intact; it adjusts lower-mip alpha only for bound leaf diffuse BC3 maps. It chooses a discrete threshold closest to base texel coverage with a bounded multiplicative scale128/255..2; equal-error cases retain the original threshold, and tiny mips cannot always represent the target fraction. Alpha zero stays zero. Ordinary source mode/defaults are unchanged. Derived mip images use bounded uncompressed BGRA8 DDS then the common texture_upload::image path; no BC requirement, normal/data maps or proprietary derived data published. This is authored mask adaptation, not reproduction of original MSAA/A2C. GPU quality and movement acceptance are pending; do not promote this draft to the owner build yet.

CPU verification: 2release app tests pass (synthetic coverage recovery/zero-alpha/tiny-mip limits and purchased maple base/RGB/full-mip preservation through common upload). Initial changed-package Clippy rejected constant chunks_exact and indexed-range loops under Rust1.99; replaced with as_chunks iteration/enumerate, then app all-target Clippy passed4.47s. Logs tests-coverage-draft01/clippy-coverage-draft01/02. GPU acceptance remains pending.

Candidatef78b7e0f099d / executable SHA25638f3753aef7943f65d797aafd5f584430842d99acad3e91b733ebd1f360aed7e, release build1m00s; isolated GPU comparisons started. Histogram boundary and truncated-DDS checks passed after retrying a sandbox-blocked temp write inside ignored local/ with authorized access; the single temporary truncated image was removed. Current main/verified8b9919f executable remains untouched.


Owner review: the right-hand coverage pilot was explicitly preferred on the distant-tree crop (17 October2026): fewer holes, remaining gaps still visible. This is the owner's visual assessment; it does not change the incomplete geometry/coverage status. Static4K source/coverage native-pixel crops inspected; localized improvement, not wholesale disappearance of far holes. Both30-frame1080sequences passed request-clock audit (30unique,<=2.05ns error) and retain identical physics metadata; final visual motion review is recorded next. First sequence invocation combined mutually exclusive --render-fixed-step and --render-fps; parser rejected before GPU setup. Failed log retained, retry02 uses --render-fps only. New full524tests, workspaceClippy0.84s, exactsolo and20/20quick passed before grass-isolation controls.

Final grass/foliage validation: source917ca5f / executable785488a1..., 525workspace tests/Clippy/exactsolo/20of20unchanged-headless soak and combined native4K6500tick validlap89.955257484s pass; physical endpoint exact old control. Soft grass selected enhanced default; trees/Atmosphere launcher uses owner-preferred coverage mips, forest remains explicit. Moving30-frame audit/metadata exact, first/last frames inspected; original LOD/coverage/wind gaps remain. See GRASS_RENDERING.md and private grass-final-validation02.json.

## Missing distant forest geometry diagnosed —17 October2026

Owner comparison with original video70seconds correctly raised a missing-background question. Existing646placements/9prototypes were not proof of visible completeness. Three placed distant-tree prototypes (02/03/04, one identity placement each) bind treesheet_foliage.fx, corresponding to the examined packed tree_sheet shader family. Their saved vertices are repeated centres; UV1 holds signed billboard offsets. The source vertex program expands each centre about world Y toward the eye, with world-up vertical extent. This differs from the previously restored spherical inverse-view X/Y facing_foliage path.

Our importer included all three draws but tagged them as ordinary static leaves, so their triangles stayed collapsed. Read-only distant_tree03 geometry audit:682triangles/1364vertices and0projectedXYarea. A new source-asset test checks full3Dzero area and nonzero expanded area for every triangle of all three draws; result pending. Thus a successful placement/draw count was insufficient, and earlier wording implying all forest geometry restored must be read with this correction. No invented tree placements, source asset edits or wall hiding.

Draft: explicitly tag trees/sheet parts, reuse per-placement camera presentation with cylindrical expansion (per-centre horizontal facing; world-up), preserve nearer spherical facing behaviour, extend conservative bounds by offset radius, supply source-like upward sheet lighting normals. Main-camera orientation for shadow/probe views remains an adaptation, not original per-pass reproduction. Keep owner-selected alpha-mip coverage and soft grass unchanged. CPU/GPU acceptance pending. Private tree-sheet-vp01.asm/tree-sheet-fp01.asm and metadata, distant-forest-passes02.json and distant-tree03/distant_tree04/distant_tree02-geometry01.json. Initial guessed packed group treesheet names returned no matches; corrected to tree_sheet after enumerating source programs.

First corrected GPU view: sourceafccf08, release1m56s, executable SHA256925384372d5a75596c835144769ab2c1aa756898f7a138355d6da2c8b9537bce. Native3840x2160, same documented far camera, rawsky/sourceIBL/coverage/softgrass/hazeoff; known-warning-only, source_tree_sheets reports3draws/2922triangles. Image inspected against previous same-camera raw view: continuous distant forest now appears behind the near trees and hills, matching the missing-layer diagnosis. This does not prove every original placement/LOD/pass is complete. Private distant-sheets-far4k01.png/.log and distant-forest-comparison01.png.

Merged mainfb32d54 (other agent's P6 Steam-executable CD01 derivation) into667dd2b; only conflict was the journal, both records retained. Source/default graphics launcher candidate now selects coverage forest in ordinary/Wheels/Textures/Reflections modes; explicit no-trees third mode preserves a control. Haze and new source sky tone remain opt-in. New merged build/moving and final workspace/headless checks are required before promotion.

Completeness audit of this route's tree inputs: private forest-inventory01.json matches all9placement prototypes to all9library roots;21_x0draws selected and6lower-LOD draws excluded intentionally. Three distant layers contain341/468/652quads (1461total). Other-container inventory: land4draws; tracksplit413draws; patchup_ot391textures/no render draws; objecttypes no render draws; objects133prototype roots/no tree/forest-named nodes. Node naming alone is not proof that no other visual layer exists, and this audit does not establish runtime culling/LOD parity. It does rule out an unmatched root in the examined trees.bin/trees.pssg pair.

Owner reviewed distant-forest-comparison01 positively and requested a future background-detail/lighting stage (17October). Preserve current correction, then improve volume, lighting separation and distant ground detail; source sheets remain flat images, not the final remaster. Roadmap updated without interrupting current validation.

Merged source3d12ef1, release2m29s, SHA256f0ce990f5a64b5c52ed38305d46d63edd6b61d7d7f3e96d7c562d65f124deb08: nativeBC4K result is byte-identical to the accepted prior-engine corrected view; forcedRGBA4K inspected with restored horizon too. Native-versus-RGBA whole image mean0.123byte/max138 differs (not byte-equivalent and not a phone test). Common texture uploader remains in use. First30-frame side-facing sequence passes request audit (30unique;<=2.05ns), retains exact previous1200tick pose/velocity/handling/deformation/lap exit and inspected0/15/29 images. It mainly shows near MCN trees, so a forward-facing sequence is required for the targeted distant-layer motion acceptance; that run is pending. Private distant-sheets-gpu-validation01.json and related captures/audit.

Final merged validation:531release workspace tests/0failed/0ignored, all-target workspace Clippy32.58s, exact solo statea4a35f899a490c36/11018ticks and fresh quick20of20/100000ticks/0failures pass. Forward30-frame sequence also passes request audit (30unique,1102..1162ticks,<=2.05ns) and exact previous1200tick physical exits. Frames0/15/29 inspected with restored far treeline visible throughout; this is limited visual evidence, not monitorFPS/full-rendered-lap or per-pass billboard fidelity. Private distant-sheets-gpu-validation01.json combines evidence. Promotion of source3d12ef1/SHAf0ce990f... is next. Source fog/sky-tone controls stay opt-in; ordinary graphics forest uses preferred coverage.

Playable promotion completed17October: primary mainfb32d54 fast-forwarded to2fa3db2 under exact status/hash guards, source3d12ef1/SHAf0ce990f... copied atomically to primary release and both verified backups. Four manifests updated; no active prototype closed/replaced (another checkout remains running). Ordinary Donington graphics modes now select the restored coverage forest; no-trees remains a diagnostic third mode. Detailed private promotion-forest01.json. This supersedes earlier opt-in-only launcher notes while CLI source_trees defaults remain unchanged. Publication follows the completed checks above.

## Replay camera ordering — 18 October 2026

F360 adds a replay camera writer in Update after live drive presentation. Source foliage must follow both writers, otherwise camera-facing leaves/sheets can use the previous camera in the same frame. Explicit ordering now follows replay::frame_camera. A regression uses the actual replay camera with a two-frame stream and checks the expanded sheet against the final camera, including horizontal facing and unchanged vertical offset. Four focused release tests (including purchased forest data) pass; app all-target release Clippy passes in 13.99s on the pre-F361 integration base. GPU/replay acceptance on latest main remains pending. This change does not add camera features or alter physics.


## Distant-sheet RGB gradient pilot — 18 October 2026

The recovered treesheet VS forwards saved vertex colour v3.rgb into o1.xyz; its fragment shader multiplies lit diffuse by that RGB. Alpha instead uses diffuse alpha times instance v7.w, not saved vertex alpha. The three saved distant blocks contain exactly two opaque greys:139 at the bottom offsets and201 at positive vertical offsets. Across2922 bottom and2922 top vertices these values were previously discarded. Private tree-vertex-colors01.json pins trees.pssg SHA03d486ac... and all27saved colour blocks. This establishes a missing source gradient, not a complete original lighting model. Ordinary foliage colours may have a different binding and are deliberately excluded.

New opt-in `--trees coverage-shaded` selects the same coverage mips/geometry as coverage, adding only raw RGB/255 vertex tint to distant sheets. No speculative sRGB conversion; alpha stays1, so coverage and shadow silhouettes remain unchanged. Bevy applies it through PBR base colour, whereas the original multiplies its wrapped/backlit diffuse output; this is an adaptation, not radiometric equivalence. Source backlight constants, instance tint, normal textures and per-pass billboard orientation remain open. Default coverage and normal launchers retain the existing image. Four source-tree release tests pass including actual saved gradients and replay orientation; paired GPU checks follow.

## Forest on every preset, with the original's level switch (2026-11-03)

The pilot ran only on the enhanced preset, which admits Donington route 0 alone, so every other
track drew no trees (Washington's god-ray cards then stood in open air). The forest is now drawn
on every preset and route by default with the owner-preferred leaf coverage (`--trees coverage`);
`--trees off` leaves it out, and the native smoke diagnostic requires it off. All 18 track folders
ship `trees.pssg` and a `trees.bin` per route. The phone draws it too (its only cut is resolution).

### Levels in the data [O]

- Each species' `ROOTNODE` holds a `lod` node with a `<species>_x0` NODE (trunk, branches and
  facing foliage: the full tree) and a `<species>_x2` RENDERNODE: one quad (4 vertices) with
  shader group `#foliage_billboard.fx`. The distant-forest species (`distant_tree*`) have `_x0`
  only, itself a tree sheet.
- The route's `tree_attributes.xml` gives each species `lod_distance_00..02` and
  `instance_buffer_max_00..02` (Washington `large_tree`: 30 / 150 / 940 m, buffers 12 / 20 / 150;
  `distant_tree01`: 20 / 100 / 900 m, buffers 30 / 1 / 1). Read by 0x9637C0 into 0x48-byte
  records (distances at +0x30); `lodQuality` "trees" folds the buffer counts together; a flag at
  manager +0x1EC multiplies `lod_distance_00/01` by 6 (not traced).
- The `_x2` quad is an eight-view impostor (`tree_lowlod.cg.vp`): its corners sit on the centre
  with signed offsets in UV1, expanded about a per-instance axis towards the eye; with `d` towards
  the eye, `a = atan2(d . v6, d . v4)`, `x = -4a/pi`, the pixel program blends atlas view
  `ceil(x) + 1` into the next by `frac(x)` (atlases are 8 views of 128x256 or 64x256). Its alpha is
  the blended atlas alpha times vertex alpha (`tree_lowlod_shadowed.cg.fp`).

### Selection [O except where marked]

The tree pass 0x97A160 computes the ornament FOV factor `k` and calls 0x972500 per instance
(made by 0x963C30 in 0x971A20: box at +0x20, type at +0xA4), which takes `v` from the ornament
metric 0x965790 with `far = lod_distance_02` (type +0x4C). The type is built by 0x981D40: it copies
the species record (0x48 bytes) to type +0x14, so d0, d1, d2 sit at +0x44, +0x48, +0x4C, clamps
them to a caller limit, applies `lodQuality` "trees" (0 zeroes d1 and d0, 1 zeroes d0, 2 keeps both) and
calls 0x986550 on them. 0x986550 writes, with `v(x) = 1 - x / d2` and `D = d1 - d0`:

| field | v at | role in 0x972500 |
|---|---|---|
| +0x94 | d2 (0) | card fades in from here |
| +0x98 | d1 + 0.9 (d2 - d1) | card fully drawn |
| +0x9C | d0 + 0.95 D | card fades out from here |
| +0xA0 | d0 + 0.6 D | card gone |
| +0xA4 | d1 | full tree fades in from here |
| +0xA8 | d0 + 0.75 D | full tree fully drawn |
| +0xAC, +0xB0 | d0, 0.75 d0 | a ramp handed to the full-tree draw with its buffer (0 inside d0, 1 beyond) |
| +0xB4..+0xC0 | | reciprocals of the four widths |

So the card (`+0xF0`, 0x9606D0) and the full tree (`+0xF4`, 0x970BE0 -> 0x962250) are both drawn
between `d0 + 0.6 D` and d1, cross-fading by distance; the fade goes into each instance's colour.
Washington `large_tree` (30 / 150 / 940 m): full tree from 150 m (fully in at 120 m), card out
between 144 and 102 m. A species without a card draws its one geometry fading in from d2. The
full-tree draw appends into buffer 0 (inside d0) or 1 (d0..d1) only while the buffer holds fewer
than `instance_buffer_max_00/01`; extra instances are not drawn [O, 0x962250; order = the pass's
visible list, not sorted]. The ×6 / 2000 m branch of 0x9637C0 and the "all distances = limit"
branch of 0x981D40 depend on manager +0x1EC, which only an initialiser stores (0x97E390) [I: off].
All routes' distances: `local/r52/tree_attributes.tsv` (d0 10-30 m, d1 90-200 m for carded species).

### Port (`source_trees.rs`, `ornament_lod.rs`)

- Trees join the ornament levels: one type per placed species with the route's distances, an
  instance per placement (the species bounds placed), `_x0` draws at level 0 and `_x2` cards at
  level 1. The card replaces the full tree at `d0 + 5/6 (d1 - d0)`, where the original's two
  fade weights are equal (`ornament_lod::impostor_switch`; Spa 10 / 150 m: 127 m, Washington
  `large_tree`: 130 m) [the original draws both across 102-150 m; the port's levels are
  exclusive]. Until F735 (row 114) the switch was at d0 (Spa 10 m), where the card's low-resolution
  atlas showed at close range; `GRID_TREE_SWITCH=d0` keeps that for review. The buffer caps are not
  ported (all trees are drawn). Each switch, and each appearance at `lod_distance_02`, fades over 0.4 s with the
  ornament dither (F703, F713). Shadows follow the ornament rule (one level casts inside D,
  through the fading proxy at D), but a tree casts through its full tree, not its card
  (`Type::impostor`): a card turned to the eye throws the wrong shape from the sun and darkened
  the road under it [AUTHORED for trees: the original's tree shadow pass 0x97A250 is not traced].
  `GRID_ORNAMENT_LOD=0` keeps the full trees everywhere.
- Cards are expanded on the CPU about world up like the tree sheets and blend two atlas views as
  the original does (`tree_lowlod.cg.vp` [V static]: view `ceil(x) + 1` at U offset
  `view / 8` plus the card's own U, the next view one eighth further, wrapping with the sampler;
  weight `frac(x)`; the pixel program lerps both colour and alpha and both decoded normals by it).
  The CPU writes view A into UV0 and the weight into UV1.x each frame (`v4`/`v6` taken as the
  tree's X and Z axes [I]); `tree_cards.wgsl` samples both views, also in the depth/motion
  prepass so its alpha test matches,
  and are lit per corner with the radial normal the full tree's leaves use (towards the eye,
  tilted out to the card's side and up or down) [AUTHORED; the original reads a baked normal
  atlas and adds `LightWrap` and `InvFoliageDensity` (0.3) in `tree_lowlod_shadowed.cg.fp`].
- Card colour check (`local/r48/*-cmp.png`, full trees | first switch | this; mean foliage value):
  cards stay darker than the full trees they replace, by 0-35% (Spa 30: 39.9 full, 25.3 cards
  on the left, 37.8 / 27.4 on the right; Washington 53.1 / 44.1). Diffuse transmission on cards
  changed nothing; the radial normals lift the side-lit Spa views and darken Washington slightly.
  The cause is open: the atlas bakes the crown's own shading, and the original's card lighting
  is not ported.
- Card lighting (`tree_cards.rs`, `tree_cards.wgsl`; the follow-up to the colour check):
  `tree_lowlod_shadowed.cg.fp` [V static] reads a per-view normal atlas (`TNormalMap`, bound per
  card material in `treestextures.pssg`, same layout as the diffuse atlas), builds the light in
  the card's frame (level axis towards the eye, its cross with up, up), bends the atlas normal
  towards the sun, `N = normalize(n + L * LightWrap)`, and lights with
  `sat(N.L + InvFoliageDensity) * sun * shadow + back * (1 - N.L) / 2 + ambient`. Saved values:
  LightWrap 0.3-0.5, InvFoliageDensity 0.3 (Washington's `LOD!0` saves none: 0 [AUTHORED]).
  The port samples the atlas at the card's UV, gives Bevy the bent normal and adds the
  `+InvFoliageDensity` lift of the sun term itself; back and ambient stay the F716 fill's lights.
  The atlas X axis sign is set from the render [I]: with it Spa's side-lit conifer cards are
  bright on the sun's side like the full trees; the other sign lights the far side
  (`local/r48/spa30-zoom.png`). Cards without a normal atlas keep the radial corner normals.
  Mean foliage value, full trees / F718 cards / card lighting (`local/r48/*-cmp6.png`): Spa 30
  right 37.8 / 27.4 / 35.3, left 39.9 / 25.3 / 33.0; Spa 75 41.1 / 26.7 / 33.1; Washington
  53.1 / 44.1 / 49.2 and 61.2 / 52.9 / 51.9. The remaining gap is open (the atlas bakes the
  crown's self-shading).
- View blend checks (`local/r49/`): a unit test sweeps the eye around a card in 512 steps and
  finds the shown mixture (view + weight) moving under 0.1 view per step, also across view
  boundaries. A sideways truck of the Spa 75 view in 0.5 m steps (`t-blend-*`, `t-snap-*`;
  `GRID_CARD_BLEND=0` restores the nearest view for review): with the nearest view, the left card
  tree has an extra jump at step 3 (9,831 pixels changing by more than 50 per channel against a
  trend of about 9,000), absent with the blend; over the run the blend changes 10-20% fewer
  pixels abruptly. Brightness (`*-blend.png`) is unchanged: Spa 30 34.8 / 33.4, Spa 75 33.0,
  Washington 49.1 / 51.9.
- Cost of the card material (blend and lighting) against the plain one (`GRID_CARD_LIGHTING=0`,
  same binary, interleaved, uncapped, `local/r49/v-*.log`; the machine was busy, floors 9.5 and
  12 ms): Washington 9.48 / 9.46 / 9.94 plain, 10.13 / 9.66 / 9.63 card; Spa 12.08 / 12.07 / 11.80
  plain, 12.07 / 12.45 / 12.42 card: up to +0.6 ms, within the run-to-run spread.
- Hidden facing entities are no longer rewritten each frame.
- Checks (private `local/r46/`): `w-shafts-ab.png`, `spa30-ab.png`, `haruna48-ab.png` (full trees
  everywhere | level switch); `c-x0-vs-n.png` (Washington cards, close crop);
  `switch-large_tree.png` (one `large_tree` at 40, 44, 50, 56 m from its trunk; the switch is at
  30 m from the near side of its box, about 48 m here; the 50 m frame looks into a
  `distant_tree01` sheet 16 m from the camera, the same without the switch).
- Cost (uncapped windowed drive on the start grid, 1280x800, RTX 4070; F718): with the switch,
  trees on and off both run at the ~6.06 ms floor on Washington and Spa (before the switch:
  +2.2..4.3 ms and +1.2..2.3 ms). With the full trees casting the shadows (`local/r48/u-*.log`):
  Washington off 6.07 / 6.06, on 6.19 / 6.10 ms; Spa off 6.06 / 6.14, on 6.79 / 6.65 ms.
- Bounds (F724): facing, sheet and card entities carry their Aabb grown by the largest quad
  offset and `NoAutoAabb`; without it Bevy recomputes the box from the collapsed centres on
  every `Mesh3d` change and culls trees whose centres leave the view while leaves are on screen.

### Sheets cast no shadow (F781)

The tree setup 0x970C40 (`tree_shader_select` in docs/re/symbols.tsv) [D] picks the shaderpack
group from the instance's saved shader name: `foliage` → `tree_foliage`, `facing_foliage` →
`tree_facing`, `foliage_environment` / `foliage_trunk` → `tree_trunk_shadowed`,
`treesheet_foliage` / `treesheet_trunk` → `tree_sheet` (each `_x1` for the second tree type,
`_shadowed` when the caller asks). In `shaderpack.pssg` [O] every tree group has three or four
passes, among them `vpShadow` with `csm_depth_opaque.psh.mainLowAlphaTested` at passConfigMask 4,
except `tree_sheet` and `tree_sheet_shadowed`: colour (mask 1) and velocity/depth (mask 2) only.
The cascade pass draws a list with the pass of its mask, so the distant tree sheets cast no sun
shadow in the original [I: that a group without a mask-4 pass draws nothing in the shadow pass is
read from the pass table, not traced in the draw loop]. No track saves `treesheet_trunk`; 13
`.pssg` files save `treesheet_foliage`.

The port cast them, through the sun-facing twins of row 122: the sheets are tall strips of
leaves expanded about world up, and their shadows fell as hard vertical bands on the canopies in
front of them (Le Mans, Spa forests; docs/GRAPHICS_AUDIT.md). `source_trees::casts_sun_shadow`
now keeps `trees/sheet/` parts out of the shadow (`NotShadowCaster` at spawn, no twin); the ornament
fade proxies already skip shadowless parts. Test `tree_sheets_cast_no_sun_shadow`.
