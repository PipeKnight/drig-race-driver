# Whole collision producer

9 October 2026, **F141**. The complete original `9EA6E0` routine and an independent Rust generation/body/solver/response pipeline produce identical output for six directly captured collision calls in debug and release. This establishes composition of the supplied-contact behavior; contact detection, vehicle tick integration and original random cadence remain open.

## Original replay and provenance

`tools/probe_impact_producer.c` executes the complete routine, including original body lookup, record generation, body adjustment, original CRT random generator, grid solver and response helpers. There are **no skipped instructions, helper stubs or original code patches**. Seven address/name-verified imports bind to real Windows clock, error and TLS functions. Two private TLS slots provide the controlled thread-data seed and cached TLS getter. Nine private data pages, imports and TLS allocations are restored/released; executable sections are checked unchanged. The original image entry point and loader initialization are not run.

The probe supplies a registered private body/linked-state fixture and rebounds its center pointer and descriptor settings pointer. Output restores the incoming center-pointer word and normalizes only the settings pointer to zero. Other incoming pointer words remain opaque passthrough bytes. History, mass, center and body globals come from the directly observed nested helper boundary. The controlled seed reproduces the observed full15-bit random result; the actual live thread seed was not captured. See [F140](IMPACT_BODY.md) for that limitation and the captured active angular/linear and response changes.

Private capture: `local/behavior/live-impact-20261009-10/capture.json`, SHA-256 `c34887e65bb5bc0f6f7e8274422f4c169b6eb9f7351713e50eb04cc1cf745519`. Pinned analysis image SHA-256 `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`. Probe SHA-256 `9b7961b8c49ac611d7c8ea7da1a4d2751078e135fd26f5b34328429dbc1d0c38`. MSVC14.44.35207, SDK10.0.26100.0, `/W4 /WX /O2 /MT /std:c17 /fp:strict /arch:SSE2`, x8753-bit nearest. No new tools installed.

## Verified output

`tools/validate_impact_producer.py` runs original/repeat and the independent Rust stages. Both profiles match **65,057 live output words plus6controlled seed words**, byte for byte, against the captured outer return and each other. The live output comprises six complete normalized descriptors, six complete linked-state blocks, body history/extra state and per-contact response values. It includes the active velocity adjustment and nonzero response correction. Generated records, frame and solved positions are comparison targets only in the Rust pipeline; its stages use their own computed intermediates.

Reports: `local/behavior/impact-producer-replay-20261009-{debug,release}-01/validation.json`. Original/repeat exit0, `code_unchanged=1`, `real_imports=7`, `original_rand=1`; zero word differences. Stage diagnostics and hashes are in those private reports and [F140](IMPACT_BODY.md#final-independent-implementation-and-pipeline-f140). These results reuse the six captured calls; they do not constitute six new game observations.

## Diagnostic protocol

ICW1 magic `0x31574349`, u32 case count1..1024, bounded64MiB input. Each row contains the existing IPG1 generation row without its file header, then center vec4, ten body history floats, extra float, mass float, five body globals, seed u32, supplied point/direction vec4s, response scale, six solver global words, clip/presence booleans and one response float per contact. Output contains descriptor0xA500, linked0x424, history40bytes, extra4bytes, seed4bytes and response floats. All native captures and binary fixture/output files remain under ignored `local/`. Failure can leave partial private native output; it is not a usable result.

## Next work

Recover upstream contact production and callback preparation, then connect resource-derived initialization and vehicle tick scheduling. This result does not yet connect the collision pipeline to Bevy driving or resource-derived initial body history.

## Independent Rust composition (F142)

`grid_world::impact_producer::calculate` now owns the stage transitions within one Rust call. Its typed input separates grid state/policy, supplied contacts/generation inputs, body state/policy and response state/policy. Generation derives its nodes from the same authoritative grid node pool as the solver; response consumes the generated records, solved positions and same contacts. The body frame is calculated from the supplied pose/offset. No original function, intermediate capture or external process is called by this library API. Supplied contact direction/point, body history and random seed still originate outside it.

The empty-record branch preserves body history, seed and velocities. Solver/response helpers have matching empty-input arithmetic and receive the generated dirty state. The bounded API validates supplied body data even on this skipped branch; invalid-domain rejection is an intentional diagnostic contract, not a claim about the original routine's behavior on malformed inputs. Existing body input validation is shared without arithmetic changes.

`grid-impact-producer` adapts ICW1 to that API in a single process and writes only a new file under ignored `local/`. `tools/validate_impact_producer.py` now compares original/repeat, this single-process diagnostic and the earlier independently orchestrated stages. Final live reports `local/behavior/impact-producer-rust-20261009-{debug,release}-final/validation.json`: all **65,057 live words plus6controlled seed words** exactly match for every implementation/build.

`tools/check_impact_producer_controls.py` combines independently authored generation/body controls with authored response state, optional response arrays, solver iteration/clip policies and seeds. Final reports `impact-producer-controls-20261009-{debug,release}-final/validation.json`: **256cases /2,777,192output words** match original/repeat/Rust exactly per build. Original execution advances the seed in115cases, changes angular state in88, changes the response accumulator in4, and changes response values in93. Sixteen fully disabled-contact cases explicitly verify unchanged body/response state, including present and absent response arrays. Ten malformed requests reject; Rust creates no output, native failure output may be partial.

Initial control attempt `...debug-01/` exposed an authored expectation error: null response storage is packed as zero, while the checker expected unused random values. Corrected the fixture and added present-array coverage on the disabled-contact branch. Attempt `...debug-02/validation.json` matched all numerical outputs, but three rejection checks exposed missing body validation on the empty-record branch. Shared validation now rejects nonfinite direction, zero mass and zero history range there. Final reports above pass all checks. No numerical mismatch was observed.

Final diagnostic SHA-256: debug `b0b2bd74b32d3ebb0c39fc503a1c7a848f8317f06fb4861ec31b9d5cb2a2824c`, release `c4eb607d0d65ae65a423beac9a91ab309cbeec29df15afc475215fda157ad9c7`. Rust/Cargo1.99.0; all104workspace tests, formatting, warnings-deniedClippy and PythonAST checks pass. Steam preservation at `2026-10-09T08:52:29.396387+00:00` confirms130traced resources and the original executable unchanged: `local/scene/preservation-20261009-producer.json`. No additional desktop input, capture or tool installation was needed for F141/F142. Research commit `7feaabd` is pushed separately from this Rust composition milestone.
