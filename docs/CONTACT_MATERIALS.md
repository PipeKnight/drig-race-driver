# Supplied wheel contact/material arithmetic (F082)

## Scope

`grid-world::contact_material::resolve` consumes an already supplied query result, material, ray and wheel-radius samples. It implements contact acceptance, procedural bump height, offsets and solver-normal adjustment. It does not produce the world hit or resolve material table ownership. [Live observations](CONTACT_REFERENCE.md), [radius helper](WHEEL_RADIUS.md).

A query fixture replaces the original call at9c18d7, supplies four normals/fractions/nonzero-object flags and material tokens, and returns an authored object pointer. Original9c1660 runs around that fixture, including real998470/CRT modulo and normal helper code. Thus comparisons validate **conditional consumer arithmetic**, not original collision traversal, hit selection, candidate objects or token identity. No live memory patching/injection; fixture exists only in isolated private process.

## Contract

Accept only a nonzero supplied hit with `(axisY*normalY+axisX*normalX)+(axisW*normalW+axisZ*normalZ)` strictly above stored .4226182699. Equal threshold rejects. Miss/rejection retains floor-1000 and bodyY as wheel normal; solver normal is then regularized as in F079. Accepted base floor is `(ray.low-ray.high)*fraction+ray.high`.

When bump calculation is enabled, material90 chooses time or spatial coordinates. Time path is body159c/material94 and wheel ordinal. Spatial path is **ray start X/Z** times `1/material94`, not the wheel center. Names/units of the body operand are not calibrated here. Scale both coordinates by256, truncate to signed integer and mask to16bits. High/low bytes select a periodic256x256 sample grid and interpolation weights. A byte sample is `trunc(sqrt(f32(gridIndex)+2.1588330269)*987345.625)&255`. Two integer interpolation stages each shift right8; subtract128 afterward. Native sqrt uses explicit SSE sqrtss; required precision is exposed by the byte extraction.

Scale noise by material98, `min(body1648*.1,1)`,1/128 and `(body12ac*materialA0+(1-materialA0))`, preserving each f32 operation. Save new bump to context80. Body1648/12ac semantic provenance remains a caller obligation. Disabled calculation reuses context7c and leaves context80 unchanged; previous/new bump fields are distinct.

Apply material9c*bump plus base floor, then subtract materialA4. Add the wider selected-radius-minus-base result and subtract wheel140 before final f32 storage; the original intermediate f32 store does not pop the wider x87 sum. F083 maps materialA4 to source depth; wheel140 provenance and sample generation remain open. See [surface mechanics](SURFACE_MECHANICS.md).

Native input guards enforce finite positive wavelength/radii, four-wheel ordinals, fraction0..1, bounded angular/integer conversions and the existing normal-loop bound. These are authored API constraints, not original parser guarantees. Missing-material branch is outside this API/comparison.

## Comparison evidence

**236/236 cases and13,216/13,216 words exact debug/release**:10,384float words plus2,832pointer-presence flags. Final evidence `local/behavior/isolated-contact-material-20261007-06/validation.json`, `07/validation.json`.32analytic threshold/miss/time/reuse controls,24captured geometry/material/radius input replays with supplied fractions .5 and body159c/12ac operands1,180synthetic controls. Replays are not synchronized live contact boundaries and never compared as actual retained floors.

Initial01 differs in54cases/174words because spatial sampling used cast center. Explicit SSE sqrt in02 does not resolve this. Temporary noise-coordinate observer in03 identifies a ray-high/bodyY coordinate difference; instruction9c186a overwrites the relevant stack vector with ray start. Correction yields exact04(debug)/05(release), then final06/07 after cleanup/formatting. Temporary diagnostic observer removed; failures remain private.

Analytic Rust tests independently check strict threshold/missing-hit rejection, periodic wrap, bounded integer conversion, reuse of old bump and the height-offset/radius arithmetic. All72project tests (45formats+27world), formatting and warnings-denied Clippy pass; `contact-material-final-tests-20261007.txt`, `contact-material-final-clippy-20261007.txt`. New Python validators compile. Existing Rust1.99.0/MSVC14.44.35207/SDK10.0.26100.0/Python3.12.14/Capstone5.0.6 reused, no install. Probe SHA e88faa6c6a04398380bd9cfa588380f6eef36da74120d301eda03ef8295125c9.

Authored tools `grid-contact-material`, `tools/probe_contact_material.c`, `build_contact_material_probe.cmd`, `validate_contact_material.py`; instruction/constants evidence `contact-material-full-instructions-20261007.json`, `contact-material-constants-20261007.json`. Code RX after the explicit query-fixture call patch, lazy normal data page RW, protections restored. None of these comparisons establishes complete driving behavior.
