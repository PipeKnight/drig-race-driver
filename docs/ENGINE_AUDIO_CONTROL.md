# Original engine playback control

F275, 14 October 2026. This work replaces guessed playback rules with verified
original calculations. The playable 8f73dd1 mixer still has the owner-reported
mosquito-like tone. **These results do not fix or validate audible fidelity.**

## Recovered behavior

Aligned constructor 4223D0 selects all six Lacetti recordings: engine and exhaust
for deceleration, acceleration and loaded acceleration. The experimental mixer
selected only the three exhaust recordings. Actual update 451770 calls RPM,
gear/throttle, position, pitch, gain and voice-scheduling routines; it is not
equivalent to the first mixer's fixed granular windows.

RPM routine 422A40 smooths the physical angular-speed/maximum ratio and remaps
physical idle to the final Idle/RevsFromTime value. The playback tables use
truncation of `normalized * 1024`, clamped to 0–1023, with direct element lookup.
The first mixer interpolated using 1023 instead.

415500 reads **float** source-frame positions, truncates them to signed integers,
then divides by unsigned voice lengths using x87 at 53-bit precision. Pitches
are clamped to 0.5–2.0. Its shared requested-RPM clamp carries through the three
category calls. Decompiled pointer/integer types obscured the float conversion;
aligned `cvttss2si` instructions establish it.

4152A0 advances twelve float positions at their preceding pitches. It consumes
the full double delta, not a rounded float. Bank zero multiplies rate by delta
then pitch; other banks multiply rate by pitch then delta. Restart flags are set
when the **unrounded double result** exceeds 0.9 times unsigned length. The stored
position itself rounds to f32. These ordering details matter to exact results.

4153E0 returns recorded RPM at an integer source position. 415460 converts the
TimeFromRevs lookup to an unsigned microsecond seek using length, sample rate,
53-bit intermediate arithmetic and truncation. This establishes the helper's
seek unit, **not the meaning of INI Length or the original factory's lengths**.

4371C0 statically alternates voice pairs across four banks and three categories.
Pitch outside 0.95–1.05 requests a restart; fades use the native 0.125-second
constant and signed fade range. The complete scheduler/backend is not ported.
42DE60 contains trigonometric gain shaping and backend gain/pitch/spatial writes.
Its math helper BF4FFC is not an absolute-value operation: aligned code shows
exponent/table arithmetic. Its exact logarithm contract remains under inspection.
No filter or missing postprocessing stage has yet been verified.

## Coupled original execution and independent Rust

`grid-tools::engine_audio_control` carries a complete 0x340-byte owned controller
and consumes immutable vehicle scalars and four 1024-element curves. It executes
the independently authored equivalent of this bounded composition:

1. 422A40 RPM mapping and the caller's mapped-RPM publication to C8.
2. Whole 4152A0 position advancement and restart markers.
3. Whole 415500 for categories 0, 1 and 2.
4. Whole 4153E0 and 415460 for all twelve voice slots.

The native harness executes those **unchanged original routines** from the
identity-checked private analysis image. Only explicit owned controller,
component, vehicle and curve pointers are relocated. No imports, TLS, executable
entry point, game injection, backend calls or substituted helpers are used.
Complete mapped-image equality, controller canaries and read-only input buffers
are checked after execution. Opaque controller bytes are retained and compared.
This authored composition does not execute whole 451770.

The admitted domain includes finite vehicle/voice state, positive bounded rates
and lengths, positive RPM curves and bounded time curves. Gear wobble is zero;
the remap endpoint gap is bounded to avoid the unowned diagnostic logger branch.
Rejected domains are explicit, not implemented through no-op callbacks.

`tools/validate_engine_audio_control.py` compares original, original repeat,
Rust debug and Rust release outputs:

| Group | Cases | Complete controller bytes | Lookup result words |
|---|---:|---:|---:|
| Purchased Lacetti curves, varied owned states | 1280 | 1064960 | 30720 |
| Independently authored curves/states | 1280 | 1064960 | 30720 |
| Actual source-driving sequence | 1000 | 832000 | 24000 |

All outputs agree byte for byte, including pitches, seek microseconds, restart
flags and untouched bytes. Source-driving retains the entire controller between
all 1000 ticks; the audit checks each native result against the next input.
The source fixture reads physical speed/maximum/idle directly, without the first
mixer's quantized mailbox. It uses explicit 319294/477038-frame, 32 kHz playback
fixtures; it does not establish the original factory's lengths or restart policy.

Two simultaneous source-only Lacetti/Donington worlds receive identical normal
controls. After each playback update, complete vehicle/physical/frame/grouped
body/contact/world-word/solver-row buffers remain equal to the world without
audio control. This establishes no feedback for that sequence, not all possible
metadata or native handling equivalence.

Thirteen malformed/domain cases produce 39 rejected outcomes across original,
debug and release, with no native fault and the entire original image unchanged.
The Rust API's late-last-voice failure test preserves the complete carried
controller, then resumes successfully after repair. Eight ordinary grid-tools
tests and all-target grid-tools Clippy pass. The first Clippy run requested
`as_chunks` for a constant record extent; corrected without arithmetic changes.

Private audit: `local/audio/engine-control-20261014-final02/validation.json`,
SHA256 `e6699c06e702ea222611b698ac9f5f64a50778977ad4be1bfb17f9d97c2b0b8e`.
It records executable/source/evidence identities. Phase-only earlier drafts are
retained. Ghidra 12.1.4/JDK 21.0.9, MSVC 14.44.35207/SDK 10.0.26100,
Rust/Cargo 1.99.0 and Python 3.12.14 were reused; no new installation.

Fresh static evidence is private under `local/behavior/20261014T1036480201779Z`
and `local/audio/engine-{pitch,phase}-complete-aligned-20261014.json`.
The follow-up 43EC80 survey was global audio startup, not the per-car Length
constructor; it did not resolve that uncertainty.

## Remaining work

Recover actual factory lengths, source IDs and player voice state; capture
matched original audio with RPM/load/camera. Finish gains, pair restarts,
throttle/gear behavior and active backend/spatial processing. Then replace the
experimental mixer, compare sustained notes and transitions against original
output, and rebuild the playable after verified integration. Until then M mute
and the existing playable remain available; no new sound quality is claimed.

## Follow-up: purchased car setup and original length ownership

The lengths are now found in `audio/carsetup.bin`, not inferred from the INI
Length numbers. Source SHA256 is
`264f6744ab4566e4b7fc7da708621c8df7b883c9e8fccaf3a51ce92b81debbbe`.
The existing Rust binary XML reader agrees with independent Python on every
one of its 2,105 rows (399 strings, 710 elements, 1,395 attributes).

Lacetti is `CarSetup/Cars/Car[name=lac]`, normal type, closed cockpit. EngineBank
selects eng_lac with throttleLength **477036** and idleLength **319292**. Each
is two frames shorter than the corresponding decoded PCM recording. The two
extra frames remain preserved; their purpose has not been established.
Volume trims are gear -1.0, engine -0.0, exhaust -0.0. EngineBay and GearBox
offsets are (0,0,4), Exhaust (0,0,-4), Cockpit (0,0,0), in source coordinates.
These locations are metadata, not yet a verified camera-space rendering path.

Original attribute references CB1958/CB194C lead to 41E2E0 and 4264B0; both
read throttleLength/idleLength through A67B30 and pass them to constructor
41E1A0. Aligned constructor stores them at resource C0/C4. The already verified
43C690 caller supplies those fields to 4223D0, which stores 328/32C; 436990
copies them into the twelve voice lengths. This is static dataflow confirmation
of their role, not execution of the whole factory or a live controller snapshot.
Private evidence: local/behavior/20261014T1050205151280Z and
20261014T1051321783657Z, plus local/audio/carsetup-constructor-lengths-aligned-20261014.json.

Earlier literal C0/C4 candidates 43CF10 and 464ED0 were unrelated impact-name
lookup and generic object construction. They did not establish audio resource
ownership. String references resolved the correct objects. An initial selected
reader fixture assumed Car directly under CarSetup; actual Cars containment was
inspected and corrected before validation. The failed draft is retained.

`audio_car_setup` reads selected source paths/banks, lengths, trims, offsets and
cockpit flag with bounded framing, unique elements, finite values and safe
relative resource paths. Independent reencoding reproduces all fourteen typed
rows in debug and release; fourteen valid-framing malformed semantic cases
reject in both profiles without creating output. Private audit
`local/audio/carsetup-20261014-final01/validation.json`, SHA256
`e77debfae1203e56992605ec6d434a80db3d50374d0838aead60a429ab04c245`.

The carried source-driving proof now loads these purchased lengths and the
six selected dictionary rates, checking the two-frame extent relation for each
recording. It again matches original/repeat/debug/release completely with no
physics feedback. Current audit
`local/audio/engine-control-20261014-carsetup-final01/validation.json`, SHA256
`1e265cdd4b58bb725abb71eda7e43148d7734313b92203eb367b3cce0da1d112`;
carried output SHA256
`81c770984f6f2a5683757578dc7586d5a6d098214a80d96673491c5854267221`.
All current audit identities match. The 28 format and eight tools ordinary
tests pass, as does all-target grid-tools Clippy. No audible mixer or playable
change was made. Next priorities are actual gains/restarts/backend behavior and
matched native capture; source length discovery is no longer a blocker.
