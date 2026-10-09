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

Replace the diagnostic Python stage orchestration with one independently authored Rust call path, validate authored whole-producer controls, then recover upstream contact production. This result does not yet connect the collision pipeline to Bevy driving or resource-derived initial body history.
