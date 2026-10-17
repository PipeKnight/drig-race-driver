# Graphics / engine integration

16 October 2026. Owner requested periodic careful merges into main, preservation of the recovered F269 stash branch, and portable texture uploads through the engine's shared upload path. Work happens in a separate managed integration worktree, codex/grid-graphics-integration. Primary checkout's uncommitted PROJECT_LOG.md and untracked AI research document are preserved.

## First merge, in validation

Parents: engine main0a2ff450aec2 and graphics b4febd620480. Four files conflicted: PROJECT_LOG.md, grid-app/main.rs, grid-formats/lib.rs and texture.rs. Both journals/modules are retained. The texture reader combines partial mip-chain support with main's BGRA8 level sizing. General route/car loading, mixed-car field, unified input, damage/joint ownership, auto-reset, race finish and timeout handling remain from main. Graphics uses the new car0 namespace for player-model materials/probe exclusion. A tuple keeps presentation within Bevy's16-parameter system limit.

Enhanced rendering currently explicitly requires Lacetti/Donington route_0; baseline retains other cars/routes. Graphics launchers pin this car/route so a user's unrelated profile does not silently load Donington lighting/assets into a different circuit. Wheel exposure follows source-part visibility after damage presentation, before transform propagation, avoiding a floating blur hull after a wheel is hidden/detached. These integration changes still require GPU checks.

## Portable texture contract

All newly introduced DDS uploads (source sky, track layers/skid weights, car tread normal) use texture_upload's existing portable path. Existing image() remains the sRGB entry point. New image_linear() shares its selection, counters and BC-to-RGBA fallback, preserving linear normals/weights. Linear maps currently fall back to RGBA rather than using the sRGB-only ETC2 cache. No duplicate decoder or forced BC device assertion is introduced. Already uncompressed HDR render targets/cubemaps are not DDS uploads.

The texture-fallback-P8.md filename was not found by bounded read-only searches in the supplied primary repository path, .claude/.codex, Temp and Tools. The owner's forwarded instruction and main's actual texture_upload.rs were sufficient to apply the contract; do not claim the missing note itself was read. No phone/Android device has been tested by this graphics session.

## Checks and failures

- New worktree seeded before Cargo:2.3GB in4s from the existing seed. Windows PowerShell default policy refused the script; process-scoped RemoteSigned under pwsh ran the reviewed local script without changing a machine-wide policy. No tools installed.
- First provisional merge check found the17-parameter system, old grid_slot/load call signatures, missing new Driving test fields and stale image imports. Fixed while preserving main's APIs. Second check found an8-argument helper; grouped route/model context. Final changed-package alltarget release Clippy passes.
- Full cargo t --offline -- --include-ignored passes:431tests,10harnesses,0failed/0ignored. Includes purchased resources, the new linear normal/mip fallback test and actual wheel remainder/pause/reset ECS test. Exact logs are private local/integration/tests-combined01.log and clippy-merge01..03.log.
- rustfmt touched unrelated copied engine files. First git restore was rejected by automatic review as possible loss of another agent's changes; nothing executed. Exact per-file backups/full patch and primary/index/HEAD hash proof were saved in local/integration/rustfmt-backup. Guarded retry in the isolated worktree was approved and removed only this session's formatting. Primary source stayed equal to main. No approval remains blocked.
- Full workspace alltarget release Clippy passes (clippy-workspace01.log,44.73s). Final post-startup diagnostic initially displaced setup's Clippy allowance; targeted check caught it (clippy-portability01), ce97844 restores it, clippy-portability02 passes9.53s.
- Solo-lap identity passes against main's local/ai-pacing/runs/solo-rebased.txt: normalized output identical, laps88.42029812185638/86.28228501935392, final state a4a35f899a490c36,11018ticks. Private solo-identity01.log. Engine/headless source trees remain identical to main.
- f405263 makes reflection audit paths honor the configured private data directory and adds post-startup texture counts, so forced RGBA validation includes graphics maps loaded after the earlier setup summary.
- Pending before push/main update: grid-soak quick, source-stamped playable build, baseline/mixed-car and enhanced GPU captures including forced RGBA fallback. No merge into the primary main checkout has occurred yet.

## Preserved F269 branch

codex/f269-timed-lap-draft / e6f6c47 is still local and stash retained. Read-only comparison finds its lap_timing.rs in main with only nested-if/let-chain syntax differences; later lap replay/inspection scripts are already continued by2889d24. No useful missing runtime feature established, so do not cherry-pick an obsolete957-line draft over current timing code. Further archival review can happen after this integration milestone.

## First GPU integration findings (not accepted as final)

ce97844 executable SHA256 ce434f305acef2cd745de732583107e58eb76bb31b2b1f0eadb38fd3edc961e8. Enhanced forced-RGBA1080p and native-BC4K 240tick captures exit cleanly with identical pose/velocity. Images inspected; source sky/layers/linear tread normal load, but grass appears as dark clumps in both. RGBA poststartup count unexpectedly1BC asset, despite552decoded images; this invalidates an all-assets portable claim. Baseline mixed Lacetti/BMW240tick image inspected, correct BMW geometry/livery, no corresponding large dark grass clumps (different opponent model means physical state is not a matched control). Evidence enhanced-rgba01, enhanced-native4k01, baseline-mixed01 private PNG/log pairs.

Root cause for remaining BC: skid_marks::install called the central image helper before app.run/PreStartup format selection; helper's initial path was BC.70c9b5f retains validated DDS bytes until Startup, then selects device upload. Ground detail AlphaToCoverage with enhanced MSAA off uses Bevy's colour-mask fallback, but pinned0.19.1 pbr_prepass_functions uses PREMULTIPLIED_ALPHA_CUTOFF for coverage instead of material cutoff.6d1d9fb explicitly uses Mask(0.5) in enhanced views so prepass and colour agree; baseline coverage retained. GPU proof of these fixes pending.

Quick soak completed:12scenarios,60000ticks,0failed,3.3min wall; includes Donington/Nurburgring/mixed assists/damage/props, local/integration/soak-quick01/summary.md.

## Corrected integrated release verified

6d1d9fb5a1c8 release SHA256711236970bb1a2290f41002d2a4371578d5749d58351db3689316d1824b95614 (build-final02.jsonl,2m29s). Full repeated suite431pass/0failed/0ignored,10harnesses; fullworkspaceClippy pass (tests-combined02/clippy-workspace02), targetedClippy10.73s. The source-only engine/headless trees still equal main0a2ff45; previously verified soloidentity and quicksoak remain applicable.

Corrected enhanced-rgba02 at1080p shows0BC1/BC3 assets,553decoded/556uploads,397436976uploaded bytes. Corrected enhanced-native4k02 and RGBA images inspected: large black grass clumps removed, actual blades retained, sky/layers/body clean. Scripted240tick pose/velocity exactly match pre-fix and normal-texture controls; clean GPU logs. No phone or ETC2-device validation claim.

Combined adaptive sidewall/spoke filtering plus moving256HDR probe at diagnostic30FPS (probe-wheel-30fps02) and adaptive wheel diagnostic300FPS (wheel-300fps01) are clean/inspected, four actual hulls, same240tick state. Probe excludes98player parts, six capture views with0localprobes, one mainlocalprobe;187copies, exactly2HDR audits, allsixfaces populated/finite, maxima5.0390625/8.46875. Evidence is contained in explicitly supplied local/integration/probe-wheel-data01/graphics. First invocation30fps01 failed before rendering because the requested data dir did not exist and output lay outside it; corrected invocation uses an existing private dir and PNG beneath it.

Snapshots and virtual frame clocks validate the integrated path, not continuous30..300 realFPS motion or performance. Rotational exposure remains opt-in/player geometry approximation; full motion acceptance, wheel silhouettes/static-caliper treatment and floating far strips remain backlog. Baseline mixedBMW image was also inspected; no overwriting of engine handling. Validation manifest local/integration/validation-integrated01.json.

Primary's known four-line uncommitted G001/G002 insertion is retained verbatim near the existing G002 history; exact backups/hash manifest local/integration/main-preservation01. F269 branch/stash untouched. Evidence dedup applied to settled integration files (five-minute age guard),0duplicates/0errors; no cited evidence removed. Ready for guarded primary main fast-forward and verified executable promotion; not yet claimed as performed.

## Primary integration and playable promotion

Primary main fast-forwarded0a2ff45 to67973c0 after exact HEAD/status/private backup hash guards. Known journal insertion is in the merged journal and retained named stash e7d12a70d90194715e6d17a80bb1d961003e040b; no stash deletion or generic restore. Untracked AI_CHOREOGRAPHER_CATCHUP_RESEARCH.md SHA256c4d52b9ecf09599a4656202c48b9d56db4fcbef0df162667937fd1b3d2b9716f unchanged. Old F269 branch/stash untouched. Original graphics branch fast-forwarded to the same codebase, removing its obsolete standalone upload path.

No active grid-app process before promotion. Exact verified6d1d9fb executable atomically copied to primary and graphics target/release/grid-app.exe, with both local/playtest/current-build.json and local/graphics/current-build.json plus verified-grid-app.exe refreshed. SHA256711236970bb1a2290f41002d2a4371578d5749d58351db3689316d1824b95614;6d1d9fb crates/Cargo/config identical67973c0, later commits documentation only. No redundant Cargo compilation in main. Promotion provenance local/integration/promotion01.json. Both normal and graphics launchers use this executable; enhanced remains explicit. All future promotions must use atomic replacement, especially local evidence files which may be hardlinked.

Second graphics wave is ready for main: bounded motion-sequence evidence/audit, explicit adaptive-floor art control, wrapped circular filter continuity and optional Wheels launcher. Source6f774b1 release52f36a7e... GPU/full432/Clippy/quicksoak/solo verified; see WHEEL_ROTATION_FILTERING.md for limits. Main still18eac2c at this validation point, untracked AI research preserved; no pending engine conflicts identified. Normal wheel exposure remains off.

Second wave performed: primary main and graphics fast-forwarded31ba353 and pushed. Owner research hash remains c4d52b9e...; old stashes retained. No active grid-app before atomic executable promotion; source6f774b1/SHA52f36a7e399f84ca4607414be815cc945468cf983f5d82146774dd33c308f1ba now in both checkouts and both manifests/verified backups. Optional Wheels launcher is available; default rotational filtering stays off. No engine/headless source changes.

16 October2026 texture wave: main31ba353 fast-forwarded e0ad1d4 after433tests/0ignored, workspaceClippy, exact baseline solo and fresh12scenario quicksoak. Source c89eede / executable SHAea38ab0d9d46bb55823fd6f233178473b1c0f9dfd39e95f90dab1898af598fe4 atomically refreshed in main and graphics, no activeprototype. Default textures unchanged: the validated one-material high-resolution pilot needs an explicit private manifest. Owner high-speed wheel feedback remains an open quality issue, not fixed by this texture wave. Owner AIresearch hashc4d52b9ecf09599a4656202c48b9d56db4fcbef0df162667937fd1b3d2b9716f remains unchanged; F269/stashes retained.

17 October2026 forest wave: merged current main through F358 without changing engine logic; only journal conflict retained both sides. Primarycd21873 fast-forwardedbc7bc14 after exacthead/status/ownerresearch guards. Verified source21b0ed70b932/executable SHA85451fe658c5174ea9b455702c93d380c38895c550a42297a7eb4e814553c6ad atomically promoted to inactive primary target and both verified backups/manifests. 518tests, workspaceClippy, exactsolo,20/20quicksoak, full native4K combined texture/wheel/forest lap pass. Forest is explicit third launcher mode trees, defaultoff; crown lighting/source coverage and distant strips remain unfinished. See SOURCE_VEGETATION.md. Earlier historical F269 preservation records above are superseded by owner-authorized G047 deletion.
