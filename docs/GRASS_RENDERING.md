# Grass cutout/shading diagnosis

17 October2026, owner-reported black dots/strokes in native4K far-view crops. This supplements GROUND_EFFECTS_RESEARCH.md; no physics or purchased files changed.

## Established observations

- Matched `grass-off-far4k01` using existing GRID_GROUND_EFFECTS=0 removes the marks. They are generated ground-cover sprites, not terrain albedo or tyre marks. Current scatter/source sprite selection remains unchanged.
- Current enhanced material uses Mask0.5, terrain normals, no shadow casting, cullNone/double_sidedfalse. Source gc_billboard512x512BC3 atlas inspected on a neutral background: thin dark green/brown blades. Base visible RGB mean54.51/61.77/24.07, not a black missing texture.
- Candidate233012ca0700 / executable SHAf3792dcf2d8305c3edd6cf72a67053267c25a854cc83eab4953d08a15b64b9c5 / release2m35s adds independent --ao on|off and --grass-shadows on|off diagnostic controls, defaulton preserved. At the exact owner view, disabling shadow reception leaves selected814dark pixels byte-identical. Disabling AO changes them by mean0.518byte only. Thus those mechanisms do not explain the observed black marks here; other viewpoints are not established by this test.
- Fresh full-mip export: grass texel alpha>=128 coverage falls7.106%base ->4.893%256 ->2.899%128 ->1.343%64 ->0.195%32, then0%at16and smaller. Mean alpha stays~8.2%. Visible RGB stays near55/62/24 through64: a mip-RGB-darkening hypothesis is not supported. Hard thresholding loses fine parts and keeps sparse dark opaque remnants. Texture statistics alone do not establish the whole screen-space effect.
- Existing original source ground-cover shader includes sun/back/ambient contributions plus screen shadow mask; original render state uses alpha-to-coverage. Our enhanced main view uses MSAAoff/SMAA; pinned Bevy0.19.1 alpha-to-coverage falls back to a mask without MSAA, so simply renaming the alpha mode does not restore smooth coverage. Earlier mirrored-geometry normal problem had already been corrected upstream; it is not reintroduced here.

## Initial experiment and limits

Opt-in --grass-alpha blend keeps original atlas/RGB/scatter/geometry/shadows but uses soft alpha composition in enhanced mode. The initial candidate kept masking as its default pending GPU acceptance; the later selected default is recorded below. This is an authored alternative to original A2C, not its reconstruction. Transparent grass does not write opaque depth, is sorted as one generated mesh and may have overlap/SSAO/contact limitations; assess near/far views and movement before changing any default. No texture recolouring or guessed brightness lift.

Evidence private in graphics local/graphics/vegetation01: grass-atlas.dds/grass-atlas-grey.png, grass-alpha-colour-mips01.json, grass-lighting-isolation01.json/crop01.png, grass-{off-far,no-shadows,no-ao,no-ao-shadows}4k01 captures/logs. Nativepixel ROI2250,730..3150,1100, selected baseRGBmean<80/no-grass>120/drop>35: localized diagnostic, not quality score. Existing tools/Pillow12.3/NumPy2.3.5 reused; source assets/decompilation remain ignored.

Diagnostic provenance correction: source233012c/25570ce preset summary accidentally labels sky_ibl and SSAO in swapped order under --ao off. Component setup and ground_detail_shading ssao=false are correct; image comparisons follow the actual component. Fixed summary ordering before the final candidate; this is a logging correction, not a change to the isolated result.


## Soft-alpha native4K result and selected enhanced default

Candidate25570ce0c4a2 / executable SHA256b21a52e7f85cef1820c833f82681a66cfa81f615fed4fa4aea444ee6d8a2d97e / release2m07s: mask/blend far and near pairs rendered/inspected at3840x2160, AO and shadow reception enabled. The prior814dark pixels rise from meanRGB51.7/48.5/39.6 to154.2/147.1/112.8 through composition with the ground, not texture recolouring. The near view retains recognizable green blades; the farther ink-like sparse strokes are suppressed. Native crop grass-alpha-comparison-crop01.png was shown to the owner. This establishes this view's improvement, not absence of all sorting/temporal issues.

Near camera was selected from source CPU geometry ray2700,950 (grass-pixel-ray01.log, source terrain22.065702m); camera(-410.21,-15.47,-261.665),target(-408.21,-16.57,-263.665). This diagnostic does not change driving cameras. Mask/blend near captures grass-alpha-*-close4k01; exact far captures grass-alpha-*-far4k01, localized measurements grass-alpha-comparison01.json. Known nilUUID warning only; common portable RGBA path.

Select soft alpha as enhanced default, retaining --grass-alpha mask for comparison and baseline MSAA/A2C unchanged. Source scatter/geometry/atlas remain intact; this is still an authored transparent-mesh approximation. Final source-stamped build, full release checks and moving/lap acceptance are required before playable promotion. The Atmosphere/trees launcher pilot also selects the owner's preferred coverage-preserving tree mips; ordinary graphics keeps forest/fog off.


Finalf03982e9cdd9 / SHAb5773e00... / release2m09s full4K6500tick attempt hit the diagnostic wall-clock timeout at3785ticks/60.56simulated seconds, wall~194.87s. Physics continued with4contacts/terminalnone; no shader error before the explicit capture-timeout error, no PNG or complete lap. This is a failed validation, not a passed lap or verified game crash. Full failed log grass-final-lap4k01.log retained. The initial automatic budget is~186s for6500ticks and includes startup; the busy machine did not finish in it. No performance diagnosis or completed4Klap claim.

Add --capture-timeout30..3600seconds as a bounded optional extension of the offscreen watchdog only (requires --render). Default watchdog unchanged; a smaller supplied value cannot reduce the normal budget. Nonfinite/out-of-range values rejected, unit-tested. Retry with900s at identical4K/quality after the source-stamped rebuild; do not lower image quality or alter physics to pass.

Bounded-watchdog unit test passed; changed-app all-target Clippy26.46s passed. Partial tick3785 state matches the prior completed-lap control exactly after removing wall-clock timing. This does not turn the failed attempt into a full-lap pass. CPU contention versus soft-alpha overdraw cost was not isolated, so the slowdown is not attributed solely to CPU load. The unaffected headless quick soak continues; its source crates/binaries remain unchanged by the timeout-only extension.


## Final verified candidate

Source `917ca5f94d5e1996ff233a55f2701fd0dcad18ba`, executable SHA256 `785488a18bd7039bfd0d2beee85281c16185be5f8e83cbc793a985766c413275`, release build 4m25s, Rust/Cargo 1.99.0 and Bevy 0.19.1. Enhanced mode now uses soft grass alpha by default. `--grass-alpha mask` restores the comparison; baseline MSAA/A2C is unchanged. Shadows and AO stay enabled. No purchased texture, sprite placement or physics parameter is edited.

Validation completed:

- 525 release workspace tests passed, zero failed/ignored; workspace all-target Clippy passed in 37.75s.
- Exact solo lap identity: 88.42029812185638 / 86.28228501935392, state a4a35f899a490c36.
- Quick soak passed 20/20 scenarios / 100000 ticks. The completed run used the identical headless source/binaries from f03982e; the only intervening change is the offscreen watchdog option/test. The explicit Git source comparison is retained. No redundant long soak.
- Identical-quality native 3840x2160 retry with the explicit 900s capture budget completed 6500 ticks, one valid lap 89.955257484s. Final pose/velocity match the earlier full-lap control exactly. Wall time 411.7454s is a loaded offscreen-run duration, not monitor FPS. Includes source fog, coverage foliage, two texture overrides and mild wheel filtering.
- A separate 30-frame 1080p moving sequence passed the virtual-30FPS audit: 30 unique frames, <=2.05ns request-clock error, metadata and physical endpoint identical to the source-alpha control. First/last frames inspected. This is not a full perceptual temporal score.
- Six current GPU logs have no unexpected WARN/ERROR; only the known nil-UUID gilrs warning. Forced RGBA8 path has zero BC assets. Earlier source/coverage native-BC 4K control also rendered successfully; no phone acceptance is claimed.

Private `grass-final-validation02.json`, `grass-final-lap4k02` and `grass-final-moving30-02` captures/logs/audit retain the successful result; the earlier timed-out attempt remains explicitly failed. Sorting of the single transparent grass mesh, original radiometry and future detailed grass geometry remain limitations. This improves the reported strokes without claiming a finished vegetation remaster.

Current native-BC acceptance: source917ca5f, grass-final-offtrack-bc4k01 native4K/272ticks, 3body contacts/deformation/debris rendered and inspected. Grass remains visible around the car; no unexpected shader log messages. Uploader reports277BC assets and0upload decodes; this excludes selected leaf-mip CPU preparation, not a claim of no CPU decoding anywhere. Single tested body/grass view, not exhaustive transparency sorting proof.


## G242: conservative CPU bucket evaluation (23 October 2026)

The owner requested a separate rendering-performance agent. The first candidate reduces only CPU bucket traversal; it does not shorten source145/30/15/5m draw distances, reduce density or change soft alpha, source sprites, quad attributes/order, fade, sort centre, shadows or probes.

For a16m spatial cell offset(dx,dz), max(abs(offset)-1,0)*16 is a conservative per-axis cell separation. Reject only when the squared XZ lower bound is strictly greater than(farthest_distance+16m)^2. The extra cell is numerical slack; the original f32 per-instance distance and fade remain the acceptance authority. Finite/nonnegative LOD and bounded finite-eye guards preserve the old path for invalid constructed states. This assumes valid finite instance positions assigned to their actual cells, as in the loaded source scatter.

At145m the square enumeration has529slots and the candidate retains413. This is an arithmetic lookup count, not a claimed21.9% speedup: arithmetic/branch overhead can outweigh avoided empty HashMap lookups. A copied-old-path CPU fixture compares every output attribute by float bits, including order, mirrored sprites, subsets, exact distance boundaries, negative cells and invalid-input fallback. The experimental source passed287resource-backed crate tests (including this ordered comparison), default/feature Clippy and a release DLAA/paint build. The runtime candidate was subsequently rejected by actual timing; it is not present in production source.

The paired baseline is frozen G240, not an older G222 executable: both arms must share every renderer feature except this candidate. Native1080/virtual30, source lighting/coverage forest, normal DLAA,250m/four4096shadow cascades and moving1024probes remain the same. `GRID_GROUND_TRACE=1` reports accumulated CPU gather/pack/stream/AABB cost divided by600mainframes. It is neither GPU time nor a per-frame percentile; virtual30 is not measured hardwareFPS. Exclude the first600frame block, match subsequent blocks, and report sensitivity excluding the block containing the30PNG evidence window. Run timings after heavy build/test jobs complete. Require actual physical/frame parity, reviewed visuals and reproducible useful savings before promotion; a null/negative result retains the baseline.


### G242 measured outcome: candidate rejected

Six full8000tick runs completed in alternating A1/B1/B2/A2/A3/B3 order, under the machine build lock after all build/test jobs. Physical streams,3846reported mainframes and motion request manifests matched. A is frozen G240; B differs only by the evaluated gather candidate. After excluding first600warmup and capture-contaminated1200blocks, the median of the four remaining600frame CPU grass block means was:

| Pair | Baseline A, ms/mainframe | Candidate B, ms/mainframe | Cost change |
|---|---:|---:|---:|
|1|1.4975|1.5545|+3.81%|
|2|1.5220|1.5905|+4.50%|
|3|1.5075|1.5675|+3.98%|

Including block1200 also favors the baseline in all three pairs. None meets the predeclared useful-improvement bar. Main/Render cumulative-counter interval estimates do not establish a compensating improvement; these remain CPU schedule estimates, not GPU timestamps or FPS. Source geometry equivalence does not imply faster execution. The patch was reversed after preserving its complete source/hash/evidence, restoring `ground_detail.rs` to the published baseline exactly after newline normalization. No density, distance, shadow or reflection quality reduction was substituted. Native PNG differences and raw probe comparisons are retained separately; universal byte preservation is not claimed.

Private evidence: `local/graphics/grass-buckets01/paired/`, source/build hashes, decision.json and the original candidate/runner under `local/handoff/drafts/render-performance/`. The evaluated candidate SHA is7d156d3a5332d49f09ff8ba143c2db059ef8b95a99687b9895f91b93f4090199. No accepted owner launcher was replaced. Next rendering work measures larger costs rather than promoting an unhelpful lookup-count reduction.
