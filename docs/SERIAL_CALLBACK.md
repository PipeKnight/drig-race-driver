# Serial collision callback and consumption — F151

10 October 2026. One independent Rust call now joins the supplied-contact callback and response consumer. Whole original **B784B0**, with its original registered **A064A0** callback and **B77490** consumer, matches the joined Rust implementation and a separately executed original-stage oracle for **544 authored cases /5,918,598 output words per debug and release profile**. No original helpers are replaced or instructions skipped; executable sections remain unchanged. These inputs are authored hybrids from earlier captures, not observed simultaneous manager frames.

## Recovered contract

Aligned instructions establish six stack arguments and `ret 18h`: owner, ordered contact-pointer array, count, scalar, pending-pair array and pending-count pointer. ECX is preserved only to forward a receiver that B77490 does not use. The wrapper initializes count response floats to positive zero, calls the owner's +41C callback with descriptor +420, and invokes consumption only when callback AL is nonzero. The fixed native response buffer supports the manager's 80-contact bound. A zero-contact call still reaches the callback, whose guard declines it; the manager itself skips empty selections.

The independent `serial_callback::calculate` supplies zero responses to `collision_callback::calculate`, then feeds its computed values directly into `response_consumption::consume` when handled. It validates correspondence between selected consumer contacts and the callback's generated-contact operands: matching initial +38 bits and current owner on the correct side, including reverse body contacts. Owner tokens and contact tokens are independent identities. Callback guards preserve supplied contact/pending state. Pending storage/lifetime belongs to the caller; this API neither gathers owners nor schedules tasks.

Bounded support: at most80 selected contacts,512 distinct supplied contact states and240 active pending pairs, finite numerical operands and consistent owner registrations. These are safe authored API/adapter bounds; the native routines are not claimed to reject arbitrary malformed pointers. See [callback](COLLISION_CALLBACK.md), [consumer](RESPONSE_CONSUMPTION.md), [owner collection](OWNER_COLLECTION.md) and [guarded serial manager ordering](SERIAL_COLLISION_DISPATCH.md).

## Comparison and evidence

`tools/probe_serial_callback.c` reuses the identity-checked PE/import/TLS harness with a serial-only adapter mode. It invokes B784B0 on owned aligned buffers, registers the actual original callback/descriptor, supplies actual original CRT random execution against a controlled private seed, and collects complete descriptor/linked/history/seed state, all selected +38 values and active pending pairs. Rewritten pointers are normalized back before comparison. Seven real Windows clock/TLS/error imports are restored afterward. All game-derived buffers and results stay ignored under `local/`.

`tools/validate_serial_callback.py` creates two variants of each of272 prior authored callback cases. It separately runs the whole original callback with zero responses, then runs the original consumer for the384 handled cases using explicitly authored owner registrations/pending state. Its independent guard expectation agrees with original AL. This staged oracle is compared byte for byte with whole-wrapper original/repeat and joined Rust. Activity includes160 guarded cases,287 accumulator changes,278 pending-state changes,24 full240-pair inputs,154 body-angular changes and192 random-seed advances. Ten malformed guards reject with ordinary exit codes below256 and no native fault marker; Rust creates no output on rejection. Internal wrapper-local response values are not directly captured; their final effects and staged equivalents are checked.

- Analysis PE SHA256: `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514`.
- Source template captures: `local/behavior/live-impact-20261009-10/capture.json` SHA`c34887e65bb5bc0f6f7e8274422f4c169b6eb9f7351713e50eb04cc1cf745519`, and `live-callback-20261009-02/capture.json` SHA`90e0c43a0ad21299038872d5ddadb83f4f9ead9ef2c4d61d18a643d2cfe5f73c`.
- Aligned wrapper code: `local/behavior/serial-wrapper-instructions-20261010.json`.
- Final isolated probe SHA: `3f8646fbbd425b614c429e25c7de45ee967cfb9e0cfb1f1376ab66b82ff0f24b`.
- Final Rust debug SHA: `50a98ff9a204a2f4dcdeed31a93860d5932eb529acddda56b4f923a222fff668`; release `fc21dc58c7ac55d6b364707637e3bfca285f2a8337b4e46d6e0b4aa42a2a2c56`.
- Final reports: `local/behavior/serial-callback-20261010-debug-final/validation.json`, `serial-callback-20261010-release-final-02/validation.json`. Earlier debug02/release-final also pass.
- Existing callback live replay still matches65,044 buffer/return words plus6 seed words; existing producer live replay still matches65,057 live words plus6 seed words after the shared Rust adapter extension. Debug regression reports: `callback-regression-20261010-debug` and `producer-regression-20261010-debug`.

Initial `serial-callback-20261010-debug-01` failed after four guarded cases with isolated native C0000005 at relocated original9EABE3. Adding registration metadata had changed counterpart row stride from0x370 to0x424, breaking16-byte alignment for `movaps [counterpart+360]`. Corrected owned stride0x420 retains alignment and covers+41C; original instructions are unchanged. Private fault instruction evidence is `serial-wrapper-fault-20261010.json`; raw partial outputs `fault-details.bin`/`fault-details-02.bin` are retained. First strict build also rejected unused `handled` in the serial-only mode; made it callback-only. These were probe-construction failures, not Rust numerical mismatches or GRID compatibility findings.

Python3.12.14, Rust/Cargo1.99.0, MSVC14.44.35207/SDK10.0.26100.0 reused, no installation. Strict C build,112 workspace library tests, formatting and warnings-denied all-target Clippy pass. Steam130 resources and executable remain unchanged at2026-10-10T03:07:53.571543Z: `local/scene/preservation-20261010-serial-callback.json`.

## What remains

This closes one active serial callback/consumer boundary. It does not verify complete active B78710 execution, task callbacks, physical contact creation, final physical use of +38, vehicle initialization, timestep scheduling or a drivable application. Completion estimates remain unchanged. Next work is live owner collection after the capture cleanup correction, and the physical contact/vehicle integration dependencies of the first lap.
