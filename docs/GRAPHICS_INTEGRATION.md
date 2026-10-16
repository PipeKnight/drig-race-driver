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
- Pending before push/main update: full workspace alltarget Clippy, solo-lap identity, grid-soak quick, source-stamped playable build, baseline/mixed-car and enhanced GPU captures including forced RGBA fallback. No merge into the primary main checkout has occurred yet.

## Preserved F269 branch

codex/f269-timed-lap-draft / e6f6c47 is still local and stash retained. Read-only comparison finds its lap_timing.rs in main with only nested-if/let-chain syntax differences; later lap replay/inspection scripts are already continued by2889d24. No useful missing runtime feature established, so do not cherry-pick an obsolete957-line draft over current timing code. Further archival review can happen after this integration milestone.
