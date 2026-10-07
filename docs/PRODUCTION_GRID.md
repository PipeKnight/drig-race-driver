# Complete production candidate-grid linkage (F086)

The guarded paused Lacetti/Donington reference now identifies **all245loaded collision wrappers** against distinct original installed chunks by full packed vertex and triangle bytes. This replaces the earlier eight-shape prefix-only sample as coverage evidence; query behavior remains separate.

## Capture

Private uk-UA reference29672, start2026-10-07T16:39:07.6084095Z, executable SHA `b1da1b2d9469fb0c973dbaaad29b230c1567f212c54de849326f80b4aece34a6`. Computer Use26.1002.52244 and existing AutoHotkey2.0.29 select Touring Cars / Chevrolet Lacetti / Donington Park Grand Prix Circuit /3laps /Nevada /Normal /Random /4Flashbacks. HUD and pause are observed; no driving input is used. Plain executable-path launch incorrectly resolves a Steam shortcut and produces no GRID process; documented `process:` identifier succeeds.

`tools/read_reference_collision_state.ps1 -IncludeGrid` checks exact process path/start/hash/module base and live triangle/collector/coordinate/subdivision code bytes. Read-only rights0x410; no suspension, debugger attachment or memory writes. Capture includes widths, exponents,1024tile contents, first-registry bounds and every unique type11shape's complete packed vertices/triangles. Each read≤4096bytes, grid payload≤4MiB, shape payload≤4MiB, wrappers≤512, finite supported packed counts. Pointer/width/exponent/count tables are read again and agree; this is not a synchronized simulation boundary.

Final snapshot `local/behavior/runtime-collision-20261007T1646001077249Z.json`, SHA `475fbbaa73cfd25b6bbf33036fcb056b1662db117aa5f95e61d38c468b55befd`: **2407/2407blocks succeed**. All1024tiles have width1/exponent0;872cells nonempty,245unique wrappers, registry count245. The bounds accessor returns X[-632.9900513,862.9107056], Z[-403.5823669,257.7298889]. These are supplied global query bounds, not inferred from a render anchor.

## Source comparison

`tools/validate_runtime_grid.py` verifies the installed archive against the earlier130-resource manifest, matches every wrapper uniquely using bounds/counts/full geometry, and checks all XYZ scales and offsets. **245/245source chunks**, **68,803vertices /119,784triangles** match; all shapes type11, all transform pointers at170zero. Source counts are negative; loaded counts are positive. Nodes/leaf lists, complete headers and material mechanics are not claimed identical.

Validation and private authored fixture framing: `local/behavior/runtime-grid-20261007-01/validation.json`, `grid.private.bin`. The latter contains retained global bounds, exponents, widths and cell tokens for isolated collection tests. Header material-token slots are paired with source codes after shape matching; these tokens have not yet been dereferenced into mechanics objects or matched to a wheel from this run.

The full grid exposed a sampling bug: the old optional eight-shape sampler read16cells even when a tile had only one, so reported tile/cell labels could refer to adjacent table storage. Shape pointers/type/geometry observations remain valid, but old cell attribution is unreliable. Corrected sampling bounds count to min(16,width²). Initial full-grid snapshot `1644160164071Z` retains the old sample labels; corrected `1644513588531Z` uses valid cell positions, and final `1646001077249Z` adds full geometry coverage. No old evidence overwritten.

## Cleanup and preservation

All17user files backed up before temporarily selecting800x600windowed, under `local/reference-race/resume-20261007h/`. Exact owned game closed through Computer Use Alt-F4; native inventory confirms exit before settings restoration. Exact idle macro16808/start2026-10-07T16:39:30.8319327Z stopped after path/start checks. All15saves unchanged and never overwritten. Only display config changed; post-run bytes preserved and original restored SHA `d222c1d1c539452335f00cad35553df18a8128e06afa3c003d8b8b36aad54463`. `cleanup.json` confirms no game/macro and zero changed saves at16:46:38Z.

Preservation at16:43:26Z: all130traced Steam resources and original executable match, `local/scene/preservation-20261007-grid-leaf.json`; source archive rechecked during final validation. No install or original Steam edits. PowerShell parser and Python compilation pass; this capture/validation change does not alter Rust logic (latest78tests/F085 checks remain separate).
