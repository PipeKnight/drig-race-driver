# Optional native-resolution DLAA

19 October2026. Owner prioritizes a clearer DLAA experiment; TAA is considered soft but playable/close to the original. Installed graphics3362/c1052 still uses SMAA Ultra. Native DLAA now runs in the separately installed optional7569901 build; the ordinary3362 build remains unchanged. Initial implementation notes below are historical; verified controls and remaining limits follow them.

## Implementation candidate

Optional grid-app feature `dlaa = ["bevy/dlss"]`, built alongside `paint-specular` for the dedicated experiment. Bevy0.19.1 selects dlss_wgpu4.0.0/wgpu29 and SDK310.5.3. Ordinary/phone builds do not enable the feature. New lock entries include Bevy's weak optional Solari metadata; cargo tree shows no active bevy_solari dependency for this feature set. No ray-tracing pipeline was enabled.

`--aa dlaa` originally required enhanced graphics. The maintained menu launcher described below adds a separate baseline camera path; both require a feature-enabled native build. The main ViewerCamera receives DlssPerfQualityMode::Dlaa only when Bevy reports SDK/device support. Otherwise it receives SMAA Ultra and logs that effective fallback. Menus and probe/shadow cameras do not receive DLAA. A stable authored custom-engine project UUID is inserted before DlssInitPlugin in every shared DefaultPlugins group, including menus; no account credential or registered numeric NVIDIA application ID is claimed.

Initial history resets explicitly; subsequent camera-view changes, decreasing physics ticks/restarts, large view jumps and rotations request reset. No CAS/sharpening, exposure, material, RGB texture, camera model, simulation or frame-generation changes. Bevy handles jitter, inverted depth and motion-vector conversion. A render-world readiness check requires equal input/output sizes and actual depth/motion prepasses after the completed render schedule; offscreen successful PNG exit waits for this state. Actual GPU readback/moving frames and clean native completion are still required before acceptance. Context/dispatch failures remain test failures; startup support fallback is not a promise to recover every driver failure.

## Reused native tools

Private SDK310.5.3 commit982b0d19f9e35fef8e1b3109efa6b95470563866; release DLL SHA8707e53b26c68c606b98bf31c223485ff30d310a261b1a36d48b2eaabc1507ec,54,779,504bytes, validNVIDIA signature. Existing NDK30.0.16248370/libclang21.0.0, staged Vulkan1.4.335 headers, MSVC14.44.35207 and Windows SDK10.0.26100.0. Process-local SDK/include/PATH settings only; no new LLVM/Vulkan SDK installation. SDK/DLLs/licenses/captures stay ignored under local/.

First offline Cargo resolution lacked bevy_solari index metadata. Online resolution downloaded that pinned0.19.1 package and resolved the new bridge/bindgen dependencies; existing dependency versions were not upgraded (lockfile disambiguates duplicate itertools/shlex/rustc-hash versions). Real grid-app all-target releaseClippy with paint-specular,dlaa passes in1m04s including native wrapper compilation. This is not final linking, initialization or DLAA image quality. Logs under local/graphics/dlaa01 plus the initial lacquer01 offline-resolution log.

## Initial verification plan (completed controls below)

Build and archive an exact source-stamped optional executable with the pinned DLL/license alongside it; keep accepted3362 separate. Run Vulkan/RTX static and fixed-clock driving controls with SMAA/TAA/DLAA using unchanged materials/light and no sharpening. Verify the log actually selects/prepares native DLAA and both sizes match; a fallback PNG is not a DLAA result. Collect bounded raw motion/depth and frozen-matrix data at30 and300virtualFPS. Check restarts/cuts and an unsupported-backend fallback. Full resource tests/default+feature Clippy, fresh merged-core canonical/quick evidence and guarded separate delivery follow actual runtime acceptance. No performance optimization or quality claim before these results.

Local authoritative sources: installed Bevy0.19.1 anti_alias/dlss/{mod,prepare,node}.rs, bevy_internal/default_plugins.rs, and dlss_wgpu4.0.0 README/build.rs. The bridge's [pinned README](https://docs.rs/crate/dlss_wgpu/4.0.0) specifies the SDK/DLL setup; [NVIDIA SDK310.5.3](https://github.com/NVIDIA/DLSS/tree/v310.5.3) includes its license and programming guide. No vendor source or binaries are committed.

## Verified native run at source7569901

The release executable linked in7m27s (SHA091f108132ebbf4be6af00fa4cd11586b1b9a1b04d759211a5a6e9134b37ab1e). On RTX4070/Vulkan driver616.92, requested DLAA reports supported=true and prepares equal1280x800 input/output with depth/motion textures. It completes actual GPU PNG readback and600physics ticks with30 distinct fixed-clock motion frames. Same executable SMAA Ultra and TAA controls also complete; all four physical exit records match across modes. This is actual native-resolution execution, not an upscaled/fallback image or merely a compilation check. No sharpening added.

Full resource tests:767 passed,0failed/ignored across21 harnesses. Workspace all-target releaseClippy passes with paint-specular,dlaa and without optional features. Newly built merged-core tools match F454 canonical SHA7b01ee413dee788ab89aaa1f7e8b345bf4367798cc10618e8fb76512ad7f81cd; fresh24-case quick soak passes. Raw capture at physics tick502/rendered frame241 validates finite bounded depth/motion records and frame-matched matrices.

The separately authored tools/analyze_temporal_audit.py reconstructs world positions using the actual jittered depth inverse and compares raw UV velocity to unjittered current/previous rigid transforms. For conservative opaque upper-body volumes,99th-percentile residual is0.0084pixels for DLAA/TAA and0.0093 for SMAA; nearby static ground is0.056–0.057pixels. These volumes are not object IDs and do not cover wheels, blended glass, damage or all pixels. The measurements rule out a broad missing/sign-reversed rigid-body velocity in this captured chase frame; they do not prove all ghosting fixed or all temporal inputs correct. Independent analytic +X/+Y translations verify16/−10pixel signs/units and mismatched frame IDs are rejected.

Private evidence: local/graphics/dlaa01/{artifact01,controls30-01,identity30-01,temporal-summary30-01,analyzer-sanity01,canonical-validation01,soak-exit01}.json and corresponding logs/raw buffers. First image review shows sharper resolved detail than TAA in places but some detail remains unstable; no claim of universal visual improvement.300virtualFPS,4K, unsupported-backend and guarded separate delivery remain subsequent gates. Installed ordinary3362/c1052 stays unchanged.

## Extended runtime controls

All three AA modes pass600ticks at300virtualFPS with30 distinct motion captures and identical four physical exit records. Each raw GPU audit passes the same conservative volume check. Native3840x2160 captures finish cleanly for all three; DLAA explicitly reports3840x2160 input=output. Native crop review: DLAA preserves fine grandstand/carbon detail better than TAA in these samples and smooths rear-window lines; foliage still differs, and this is not universal artifact-free acceptance. VirtualFPS is a deterministic rendering clock, not a measured hardware frame rate.

Adjacent pinned release DLL works without DLSS_SDK in the process environment. A fresh executable-only private directory without the DLL or SDK variable completes a requested-DLAA600tick capture using the explicit supported=false/effective=smaa fallback. This proves missing-runtime fallback on this RTX/Vulkan setup, not arbitrary non-RTX hardware. An attempted WGPU_BACKEND=dx12 negative control instead still selected Vulkan because the raw Vulkan initialization path is active; its assertion failed and is retained as runtime-path-fallback01.json/log. It is not DX12 fallback evidence, and no unsupported-backend acceptance is claimed.

`Play GRID Graphics DLAA.cmd` is a separate optional launcher for local/graphics/dlaa/grid-app.exe plus adjacent nvngx_dlss.dll. Defaults1920x1080/DLAA; optional arguments are game-root, resolution, and smaa/taa (dlaa otherwise). It uses source light/sky/number/paint and the same dry terrain defaults, with no extra sharpening, upscaling or frame generation. Ordinary graphics launcher stays on its accepted executable. SDK release DLL/license/guide/full extracted notices remain private and are not included in Git. Camera cuts/restarts have source reset handling; interactive acceptance of those operations is still pending.


Separate local delivery completed at documentation checkpoint53ac4dc: immutable source7569901 executable SHA091f108132ebbf4be6af00fa4cd11586b1b9a1b04d759211a5a6e9134b37ab1e is hard-linked into each checkout's local/graphics/dlaa/ with the pinned release DLL and license/guide/notices. Primary root launcher was added without overwriting any existing file. Hash guards preserve both ordinary graphics executable links, primary normal game executable and the other agent's untracked research note. An additional600tick GPU smoke from the exact installed primary path succeeds with nativeDLAA and no SDK environment variable. The private delivery01.json records before/after guards and paths. Manual logs use local/graphics/manual-dlaa-*.log. No source assets, SDK binaries or captures committed.


Full-lap follow-up at unchanged runtime7569901: pairedDLAA/SMAA6500tick virtual30FPS runs each complete1valid lap91.475311491s,30late motion frames and all four physical exit records match (excluding wallclock). This is the mergedF454 simulation baseline, not the earlier3362 lap. Both logs clean; private full-lap-controls01.json. Separate installed runtime remains7569901 while subsequent surface experiments are developed.

## Open moving-asphalt detail issue

19October2026 owner report and controlled9abb921 comparison identify a sharp car-adjacent asphalt patch in DLAA that is absent with SMAA at the same physical state. This is additional to the thin-antenna issue; earlier sampled temporal acceptance was not universal artifact freedom. See [ASPHALT_TEMPORAL_DETAIL.md](ASPHALT_TEMPORAL_DETAIL.md). Runtime/default unchanged pending an isolated fix.

## Maintained normal-menu launcher (2026-10-28, G325)

`Play GRID DLAA.cmd` uses this checkout's `target/release/grid-app.exe`, not an ignored experimental copy. Build with `--features dlaa` using the existing private SDK toolchain described above; place the pinned runtime DLL beside that executable. A featureless build with a leftover DLL is rejected by an actual `--aa dlaa --help` parse before the menu opens. No SDK or game assets are committed.

The supervisor forwards `--aa dlaa` through event/career menus into the selected race, including return-to-menu cycles. Saved event settings cannot override that explicit argument. The ordinary `Play GRID.cmd` is unchanged. Baseline DLAA prepares single-sample HDR colour, depth and motion prepasses on the main camera only; it does not select enhanced lighting, exposure, atmosphere, grade, reflections or material experiments. HDR-path pixel equivalence is not claimed. Runtime support and actual prepared inputs remain logged by the existing native integration; unsupported devices use the reported SMAA fallback.

G325 passes538default release tests/Clippy, combined-feature Clippy/release, and an actual normal-menu→race Window run with11opponents/300ticks and prepared native1280x800 equal-size DLAA. The first observer incorrectly required an absent exit telemetry row; its failure is retained and a separate stored-state audit verifies exact completion/native/menu controls without another GPU run. This is startup/completion verification, not baseline visual parity or all-event coverage. The maintained enhanced `Play GRID Graphics DLAA.cmd` also uses target/release, selecting the checked base-mip-zero glass profile only for DLAA1920x1080; other AA/resolutions retain glassOff.

## Baseline ground-cover correction (G326)

A matched450tick baseline AA1/default-MSAA4 versus native-DLAA run exposed black grass cards in the latter, despite successful native initialization and identical physical reports. Both runs had zero logged smoke particles, so this does not validate dense smoke. The cause is the ground-cover constructor leaving baselineAA3 on its coverage mode while the camera uses MSAAoff; Bevy falls back to a hard mask. G326 reuses the existing single-sample soft grass-material path for this mode, preserving ordinary coverage/enhanced choices and all grass geometry/shading. 539release tests/default and combined-feature Clippy/release pass. The exact repeated450tick pair removes the black grass-card band in native DLAA; ordinary MSAA4 decodedRGBA is byte-identical to its retained before control, and all189ordered physical reports match both within/across builds. Native and ordinary grass alpha modes are logged. Captures contain no smoke, so this establishes ground-cover behavior only, not dense-smoke or universal temporal stability. LDR/HDR tone placement is a separate observed distinction; no tone/gamma change is included.

## Explicit Windows desktop release (G327)

The ordinary updater intentionally builds without native SDK features. For DLAA, both `tools/update_prototype.ps1` and the coordinator's `tools/release_playable.ps1` accept `-DesktopDlaaConfig C:/absolute/private/desktop-dlaa.json`. The optional path builds desktop features `dlaa,paint-specular`, runs feature Clippy before the feature tests, builds the final executable with those features, and stages the pinned runtime and owner-prepared notice beside it. Android is unchanged. An ordinary subsequent build remains featureless; repeat the explicit configuration when making a DLAA release.

The private UTF-8 JSON contains `sdk_root`, `vulkan_root`, `libclang_directory`, `bindgen_extra_clang_args`, `license_text`, and `license_sha256`. Paths, including the JSON path itself, must be absolute Windows drive or UNC paths. SDK310.5.3, Vulkan headers and libclang are existing local toolchain inputs; no setup script is executed and no SDK is downloaded or committed. `bindgen_extra_clang_args` supplies the machine's Windows/MSVC include arguments (the verified setup above uses the target/MS extensions and explicit MSVC/UCRT/shared/UM/WinRT include directories). `license_text` points to the already prepared required notices, with their actual SHA-256. Test-fixture license files are not production configuration.

```powershell
# Plan only: no compilation, application launch or artifact copy.
powershell -NoProfile -ExecutionPolicy Bypass -File tools/update_prototype.ps1 -DesktopDlaaConfig C:/absolute/private/desktop-dlaa.json -DryRun
# Coordinator: inspect the existing release plan, then run without -DryRun.
powershell -NoProfile -ExecutionPolicy Bypass -File tools/release_playable.ps1 -DesktopDlaaConfig C:/absolute/private/desktop-dlaa.json -DryRun
```

The updater restores its process environment and records features/config/executable/runtime hashes in the existing build metadata. Runtime DLL310.5.3 must match SHA8707e53b26c68c606b98bf31c223485ff30d310a261b1a36d48b2eaabc1507ec; notice text must match the supplied hash. SDK/header contents remain trusted explicit toolchain inputs, not a verified repository snapshot. Parser-only `--aa dlaa --help` support is recorded separately from GPU acceptance. Portable DLAA requires the actual matching feature-build metadata, unchanged config and exact current DLL/notice inventory before copying; `-NoPrototype` cannot reuse stale native metadata. Featureless portable releases omit the optional `Play GRID DLAA.cmd` and remove only that exact stale portable launcher.

G327 verification: actual ordinary/native updater plans, full release DryRun in an isolated local-only Git fixture (positive and refusal),24 controlled pipeline/portable cases,8 path cases and12 real direct/nested SHA checks. No real release, APK build, SDK/runtime copying or new GPU run was performed. Existing release DryRun may fetch Git metadata as before. A failed earlier nested PowerShell check is retained: inherited PS7 module paths hid PS5's `Get-FileHash`; local streaming .NET SHA-256 now avoids that module dependency without changing global settings. Actual native rendering remains the separately recorded G325/G326 evidence.

## Current centred antenna evidence (G341, 30 October 2026)

The previous G310 motion/depth and HDR rectangles missed the antenna, so their
broad car-body motion pass could not diagnose this feature. A fresh, centred
1920x1080 native-DLAA run uses the delivered G340 executable eb5076621cbb and
unchanged enhanced settings, normal history and Motion shutter 1/32. Raw depth,
velocity and paired HDR share actual GPU frame793/tick1652. Thirty PNG requests
also complete; PNG0000 has the same requested tick, but a frozen PNG-to-GPU
transaction is not established. Private evidence and input hashes are under
`local/graphics/antenna-current01/run01/receipt.json`.

The primary shaft is included in both audit rectangles. Independent selected-pixel
analysis finds 34 currently covered shaft samples whose motion agrees with rigid
body projection (p99 residual 0.0143 pixel). Eleven rows in the observed 45-row
shaft span have no near-car depth sample; this is sparse current raster coverage,
not a measured fraction of a known triangle footprint. Displaced trace rectangles
instead contain current background depth and motion. Local coordinates in this
runtime pose basis are Y0.9903..1.1971 for the shaft; the earlier supposition that
it lies above the broad helper's Y1.65 limit was incorrect.

A short trace about 19 pixels left of the shaft becomes stronger between before
and after EarlyPostProcess HDR: the selected local flank-relative linear dark
contrast changes from 0.00624 to 0.03198. Main-line contrast decreases. DLSS runs
inside this interval, while Motion runs later, so final Motion is not required
for this occurrence. This does not establish exclusive DLSS causality, duplicate
geometry, previous visibility correctness or a general fix. There is only one
frozen raw frame and no object-ID/history packet. Private independent reports:
`local/handoff/drafts/antenna-motion01/FINDINGS.md` and
`local/graphics/antenna-current01/independent-hdr-review01/FINDINGS.md`.

The next narrow diagnostic is previous/current coverage with a verified matrix
and pose join. Repeating old K/L, frame-time or exposure matrices is not justified
by this result. The existing name-specific velocity-only antenna experiment stays
a temporary diagnostic, not a general correction or a renderer default.

One preceding-frame run completes at actual frame792/tick1649. Its body transform
matches frame793's stored previous body exactly, but four float32 words in the
camera matrix's last column differ. The strict join therefore remains
INCONCLUSIVE, unchanged in `runtime02/temporal-join.json`; no automatic repeat or
relaxed acceptance is used. A separately labelled approximate offline comparison
may quantify the effect of the arithmetic difference, but cannot turn this into
an exact-view/history proof.
