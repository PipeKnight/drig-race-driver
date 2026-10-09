# Collision response after grid deformation

9 October 2026. This extends [contact generation](IMPACT_GENERATION.md) and the [grid solver](DEFORMATION_GRID.md). It covers supplied contact records and solved node positions; contact detection, the intervening body-side-effect routine and full driving simulation remain open.

## Observed live boundary

The private Cyrillic-compatible GRID reference remained PID36228, start `2026-10-08T23:53:45.3524205Z`, with guarded AutoHotkey PID6868. After an observed race restart, F2 supplied throttle/right and F5 supplied throttle toward the concrete barrier. Capture09 completed six outer collision calls and their nested solver calls; debugger exit0 and a fresh screenshot confirmed the paused Continue menu afterward. The capture's identity/code guards passed.

Private evidence: `local/behavior/live-impact-20261009-09/capture.json`, SHA-256 `e8fb9ee3f04839e3c7652eb9cadbe86f87fcd029ac51b1690f20ff0b7ca1ec31`. Contact counts3/7/11/8/4/4; generated record counts11/40/41/19/13/15, total139. The capture now retains global `E4FC38`, observed1.0, in addition to the existing producer inputs. Policy byte `E500C4` was1 and gain `E500C0` was1.0 in all six calls.

The saved linked vector at360, accumulator380 and byte390 were all zero before and after these calls. The response arrays changed. These observations verify response-array behavior but do **not** exercise a live nonzero vector correction. No physical unit or velocity interpretation is established for linked360/380 here.

## Recovered contract

Original response slice `9EB570..9EBCC5`, within `9EA6E0`. Private instruction survey: `local/behavior/impact-response-instructions-20261009.json` and `impact-response-disassembly-20261009.txt`; prologue survey `impact-response-frame-20261009.json`. Ghidra helper outputs under `local/behavior/20261009T0756202920036Z/`; aligned instructions take precedence over inferred pseudocode types.

- A null response pointer skips this slice. Zero generated records return earlier in the producer and have no response effects.
- Normalize the **saved incoming** linked360 vector only when its four-lane squared magnitude is strictly greater than float word `358637bd` (approximately1e-6). Dot grouping is `(Y+X)+(W+Z)` of lane products. The original RSQRT seed and one explicitly grouped refinement are retained, including the fourth lane.
- For each enabled contact with positive field3C, consume its ordered contiguous records. Compute each solved-node displacement from record anchor, dot with record direction, multiply by record weight28 and accumulate. Divide by the weight sum only when that sum is positive. Record scalar24 and unwritten2C are unused here.
- The first pass finds the strictly most negative aligned deficit. Baseline38 participates when policy byteC4 is zero; otherwise this pass substitutes zero. Reverse-side contacts multiply their normal by-1; W is then replaced with positive zero. Only positive alignment with the normalized incoming vector can select a correction.
- If selected, multiply the deficit by globalC0 and the selected normal, add it to linked380, and set byte390 to1. Selection still sets the byte when gain is zero. A second contact pass updates the response floats using the selected normal, scaled correction, baseline38 and mean times globalFC38. PolicyC4 and residual sign select the reduction.
- Helper `7D88F0(1)` resolves the reverse side only when contact byteC is zero and contact+8 equals the owner. Helper `A14F80` adds four lanes to380 and writes byte390; no other helper effects were found in these instructions.

Independent Rust exposes typed nodes, contacts and records in `grid-world::impact_response`, plus a bounded private IPR1 diagnostic. Supported inputs have1..400 nodes,1..80 contacts,0..400 records, valid nondecreasing contact ordinals, enabled positive record-bearing contacts and finite arithmetic. Optional response data and the dirty state are explicit. No original pointer address is an arithmetic input. Output is created only after the entire diagnostic request validates, beneath ignored `local/`.

## Original-code comparison scope

`tools/probe_impact_response.c` maps the hash-checked private analysis image, executes the real producer prologue, and stops on an own-thread hardware execute breakpoint at `9EA6F6`, after nonvolatile-register saves and stack allocation. The handler supplies the record count atESP28, saved vector atESP70, records atESP3F0 and descriptor inEDI, then explicitly continues context at `9EB570`. For zero records it continues at the original epilogue. Thus generation, `9D4210` and the solver are **skipped**, not replayed or stubbed and not claimed as verified by this harness. Only the original response slice and its real helpers execute.

The slice uses the original stack/epilogue, restores all six saved debug-register fields, and checks every executable section against its initial mapped bytes. Code pages remain RX. Three original read-only data pages are temporarily made writable in the isolated process for supplied policy and normalizer cache state, then restored. The original entry point, TLS and import resolution are unused. The mapper's purchased-image SHA guard remains unchanged.

MSVC14.44.35207 / SDK10.0.26100.0, `/W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2`, x87 control53-bit nearest. Probe SHA-256 `ac471303fe7fe064750e5b5d5e1c02b814dbce59eaa99fc6db176bf090404191`. This implementation uses the Windows exception-context mechanism already documented in [the generation probe](IMPACT_GENERATION.md); no new tools were installed.

## Independent Rust response and combined pipeline (F139)

Initial debug/release slice comparisons match all67 live output words:37 response floats,24 accumulator words and6 dirty-state words. Original repeat is exact. Reports: `local/behavior/impact-response-live-20261009-{debug,release}-01/validation.json`.

256 authored controls, seed20081009, match all2,871 output words in debug/release. The original accumulator changes in96 controls, providing active correction coverage. These are controlled experiments, **not** additional live calls. Coverage includes both policies, normalization threshold neighbors and W contributions, reversed normals, zero/negative gain and scale, nonpositive weight sums, ties, null response, zero records, disabled/unmatched contacts, existing dirty state, zero normal,400 records and80 contacts. Nine malformed-input cases reject in both implementations; Rust creates no output. Original failed requests can leave partial private files and are not counted as successful comparisons. Reports: `local/behavior/impact-response-controls-20261009-{debug,release}-01/validation.json`.

The final combined **debug and release** pipelines pass: independently generated139 records/1,529 written recordwords; full63,360 descriptorwords after Rust grid solving; then all67 response/state words against the **outer live return**. Captured generated records and captured solved positions are used only as comparison targets, not as inputs to later Rust stages. Settings pointerA4F8 is normalized to zero and unwritten record2C is excluded, as in F138; nothing else is masked. Final reports: `local/behavior/impact-response-pipeline-20261009-{debug,release}-final/validation.json`. All256controls/2,871words and9invalid guards also pass in the final builds: `impact-response-controls-20261009-{debug,release}-final/validation.json`.

The first Clippy run rejected two explicit addition assignments. Their original operand order is retained with a documented narrow lint allowance. No numerical mismatch has been observed. Final104workspace tests, formatting check, warnings-denied Clippy and Python AST checks pass. Final diagnostic hashes: debug `8dcceb3f374d3e0c56d38190d4181ce510f151f922637cf479e7279c59b02569`, release `be6a5a48eca1fa1b7219fa01d60008fde06c25edc5a82255a58511d24c62fc54`. Rust1.99.0/Cargo1.99.0; existing RSQRT seed helper retained. Native probe builds `/W4 /WX` successfully. No extra implementation-mirroring unit tests were added; comparison with the original and live returns supplies the new verification.

Original Steam preservation check at `2026-10-09T08:12:03.947912+00:00`:130/130 traced resources and original executable match. Evidence `local/scene/preservation-20261009-response.json`.

## Remaining work

Resolve `9D4210` body-side effects, establish why the saved normalization vector is zero at these live boundaries, and recover upstream contact production. The literal-offset consumer survey completed with386references at `local/behavior/20261009T0814214759451Z/displacement-refs.tsv`; unrelated objects and stack locals must be excluded before interpreting those candidates. The exact three-stage deformation/response match does not establish whole collision physics, damage rendering, main-loop integration or a drivable Rust engine.
