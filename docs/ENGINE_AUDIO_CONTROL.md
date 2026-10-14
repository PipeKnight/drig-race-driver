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

## Follow-up: crossfade math contracts

The initial static sine/cosine labels were reversed, and BF4FFC had been
misinterpreted as absolute value during exploratory reading. Direct execution
now establishes BF4E44 **sine**, BF523A **cosine**, BF4FFC **log10**. The earlier
RPM proof excludes gear wobble, so that naming correction does not invalidate
its numerical results. The wobble helper must use sine when implemented.

The new isolated math probe executes these unchanged helpers with their actual
XMM0 argument/return contract, preserving the entire mapped image. Across 4,097
interior phases, original repeat is byte-identical; independent Python sin,
cos and log10 differ by at most 1.12e-16 in f64. The original gain path rounds
trigonometric results to f32, converts their log10 results to f32, multiplies by
20 and clamps at -48. Its native angle constant is 0x3FC90FD8, slightly different
from standard pi/2. End phases use explicit 0/-48 pairs.

`engine_audio_gain` reconstructs this bounded shaping calculation. Debug and
release match every original-helper-derived output gain bit. The maximum error
in the inferred squared-amplitude sum is 1.335e-5, including the -48 floor; this
is a numeric property, not a measured speaker/backend energy claim. Invalid
phase and source endpoint policy checks pass; all nine ordinary grid-tools tests
and all-target Clippy pass. No native numerical mismatch occurred.

Private audit: `local/audio/audio-gain-math-20261014-final01/validation.json`,
SHA256 `7ef87d4e7c12a6899d06d4a1e032a4baf18a5762f087f5d08b4cf6ddc6950bee`.
Expected shaped gains SHA256
`d5ded48c65dc37ec67b3bb2bbebe2e86c3b79cf3d0ecc82c33e0a9c628976d99`.
This executes the native math helpers and reconstructs a 42DE60 calculation;
**whole 42DE60, camera/spatial gains, final backend units and audible fidelity
remain unverified**. No guessed EQ/filter was added and the playable is unchanged.

Fresh backend/camera evidence under local/behavior/20261014T1105204126266Z:
46A7D0 writes through parameter descriptors, either object offset, callback or
descriptor storage; actual descriptor kinds are not yet captured. 42DA50 applies
camera-dependent engine/exhaust adjustment on some routes. These are evidence of
further mixing behavior, not proof of a missing DSP filter. Next read the live
audio parameter descriptors without game input, then establish a bounded whole
gain publication context or continue source-backed voice scheduling.

Read-only live capture now confirms the original parameter descriptors while
GRID remains at the menu. Both parameter-writer code prefixes match the private
analysis image; each descriptor is read twice identically. Kind-one destinations
are Pitch +18, Volume +14, Position +1C, Velocity +2C, Doppler +58, **Treble +5C**
and LFE +60. Position/Velocity use vector flag 4; scalar parameters use flag 0.
MinDistance/MaxDistance use the original A23510/A23520 callbacks, which write
owned object fields +50/+54 and set +78. This provides a route to exercise the
whole gain publisher with real helpers and owned property objects, rather than
replacing helpers or invoking an unowned audio device.

Private capture `local/audio/native-reference-20261014-01/audio-descriptors-complete-gain.json`,
SHA256 `fa31725f52eea60af80af4495255dddf05860d19db04fe958fb9d54f1d1611eb`;
names independently decoded in native-descriptor-complete-labels-20261014.json.
The first six-descriptor draft was expanded to include the tail's Treble,
Velocity and Doppler publication. An exploratory Reverb label for +5C was
incorrect: its actual native name is **Treble**. 42DE60 publishes car-wrapper
A6A4 to it on all twelve voices. This proves the parameter exists and is passed;
its production and downstream audible/filter effect are still being traced.
No matched race audio, player controller snapshot or filter equivalence is
claimed from this menu-time global capture. Helper surveys remain private under
local/behavior/20261014T1110191589399Z and 20261014T1113186851269Z.

## Follow-up: whole twelve-voice gain publication

Whole unchanged 42DE60 now executes with twelve owned property objects and the
observed descriptor kinds/offsets. Its real parameter writers, distance callbacks,
vector helpers, camera adjustment helper and math helpers run unchanged. The
admitted route supplies camera mode 3 and all voices present, so no unowned
camera service or audio device is called. Owned motion inputs are read through
the original linked-state +210/+220 helpers; source matrices provide emitter
positions. The native masks the Y component of +210 before its approximate SSE
length calculation, not W. Positive subnormal squared lengths are explicitly
excluded from this SSE domain; zero remains supported.

This is whole-function execution, not a stitched crossfade-prefix experiment.
Only owned pointer relocations and the observed parameter descriptors are
provided. Lazy vector-cache and descriptor/status data pages are restored after
each case; the entire mapped image then matches its baseline. Code remains
unchanged. All other owned inputs, controller canaries, voice canaries and
opaque voice bytes are checked. No original helper is stubbed.

The Rust `engine_audio_publication` stages the entire controller and twelve
0x80-byte voice objects. Original/repeat/Rust debug/release agree over 512 varied
cases (1,212,416 complete controller/voice bytes) and 1,000 source-driving ticks
(2,368,000 bytes). Source motion, poses, engine/exhaust offsets and trims are
purchased/physics-backed. Triangular fades, direct supplied throttle, camera 3,
distance ranges and zero Treble are explicit fixture choices; this does not
execute whole 451770 or claim its dynamic inputs. All twelve voice objects carry
between ticks. Paired source worlds retain full previously checked car/physical/
frame/grouped-body/contact/world-word/solver-row equality.

Ten malformed/domain cases reject across all three profiles (30 outcomes) with
no native fault and the restored image intact. The Rust API's late-last-pitch
failure preserves both the controller and all twelve voice records, then resumes
after repair. All ten ordinary grid-tools tests and all-target Clippy pass.
An unnecessary-parentheses compiler warning was corrected before final checks.

Audit `local/audio/engine-whole-gain-20261014-final01/validation.json`, SHA256
`afe80ddb0c9b5f44b783bde67cddbd33e997495beb1c67c2d74243d66e7f9491`.
Current identities match. Both source runs keep the loaded category at -48;
deceleration ranges -48 to -7.30917, acceleration -48 to -0.0995688 in these
supplied inputs. The failed mixer made loaded recordings prominent. This is a
verified mixing difference, not proof of the complete audible cause. Positive
authored trims demonstrate other admitted states can raise the loaded category;
do not generalize its quiet source-fixture result to all possible configurations.

### Treble reaches an EFX filter path

Static trace: 41DE10 produces wrapper A6A4, including camera/player-dependent
zero routes and distance attenuation. 42DE60 publishes it to each voice's Treble
property. A233D0 passes that property to A25D90, which stores slot Treble and sets
its dirty flag. A2C080 processes the flag through A27780; that routine updates a
filter's parameter 2 and binds source properties 0x20005/0x20006. These match
AL_LOWPASS_GAINHF, AL_DIRECT_FILTER and AL_AUXILIARY_SEND_FILTER in the primary
[OpenAL Soft EFX header](https://raw.githubusercontent.com/kcat/openal-soft/master/include/AL/efx.h).
This identifies an actual filtering path, rather than guessing from an INI name.

Read-only current native menu state has direct filter handle 3, a nonzero filter
function pointer, feature mask 0x2A60, effect-filter mask 6, sqrt switch 1 and
Treble scale 0.5. Evidence
`local/audio/native-reference-20261014-01/audio-descriptors-filter-state.json`,
SHA256 `4c3c8f2495fc2fdec067ad04c34f867a3e6a32dab5043c8da9f28d5b879332d9`.
This establishes initialized filter capability on this PC. It does **not** yet
measure the Lacetti's per-voice Treble/effect masks during a race, the filter's
audible response or causal contribution to the failed tone. Mode 3 maps some
producer routes to zero; its user-visible camera name is not established here.

The original backend also rounds sample-rate times pitch to an integer frequency
and caps it at 96 kHz, before OpenAL publication. Its volume cutoff, Doppler,
filter implementation and source routing still need matched native observation.
Private static evidence: local/behavior/20261014T1115574030285Z,
1116333617689Z,1119418429172Z,1121337580483Z,1132323430986Z and aligned
local/audio/engine-treble-filter-aligned-20261014.json. No replacement PCM mix,
device/filter execution proof or updated playable is claimed.

The filter's type is also confirmed: initialization A24820 creates the direct
filter, sets AL_FILTER_TYPE (0x8001) to AL_FILTER_LOWPASS (1), and starts both
gain parameters at1.0. EBEEB4 is obtained as alFilterf. This removes the ambiguity
of parameter2 alone, which other EFX filter types reuse. Private evidence is
local/behavior/20261014T1147256882008Z/a24dce.c.txt. The global handle still does
not prove the Lacetti's per-voice filter response or its effect on this recording.

The source recording lengths are now explained by the native four-byte PCM
prefix skip, not unexplained extra audio frames; see the
[decoder correction](ENGINE_AUDIO_RESEARCH.md#f275-pcm-prefix-correction).
Current prefix-aware source-control and whole-gain audits remain exact. Their
current identities are recorded in that correction; previous audits remain
valid historical evidence for their recorded executable/source revisions.

### Whole waiting scheduler (14 October 2026)

engine_audio_schedule::publish_waiting reconstructs whole 4371C0 with all
twelve owned voices present, camera3, and cooldowns remaining nonnegative
after decrement. Unchanged original executes real415500 x3,436F90,42DE60
and423210 x3; the latter returns at its flag/cooldown guard. No virtual
playback service or original math helper is substituted.

Pair flags select outgoing banks2/0 and incoming0/2. Category-ordered pitches
carry the C8 clamp. Outgoing pitch <=0.95 or >=1.05 raises restart.
Fades move +/-16*f32(delta) clamped[-1,1]; cooldowns subtract f32(delta),
then gains publish. Full fade traversal takes125ms. Actual play/seek/toggle
is deliberately outside this waiting boundary.

Original/repeat/Rust debug/release agree over1024authored cases and2424832
complete controller/voice bytes:1501new restart flags,1853saturated fades.
Canaries/read-only graph/full restored mapped image pass. Eight malformed
inputs across three implementations reject without native faults(24outcomes).
Late gain failure and service-required cooldown preserve all Rust state.
11ordinary tools tests/all-target Clippy pass; source-driving tests skipped.

Existing Rust/Cargo1.99.0/Python3.12.14/MSVC14.44.35207/fp:strict/SSE2 reused.
Private audit local/audio/engine-schedule-waiting-20261014-final01/validation.json
SHA25684771211f0e6a0671d6cf752d0b92673b30de397c6058db735199aa5c8f2428e.
No whole451770/Chase2/activebackend/PCM fidelity/playable update claim.
Next actual restart service, then continuous purchased PCM composition.
