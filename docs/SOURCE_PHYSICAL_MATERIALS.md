# Source physical materials in the retained vehicle world

11 October 2026, F201. Rust now reads the physical contact coefficients from
the purchased `surface_materials.xml` and `car_materials.xml`, separately from
the tyre/mechanics descriptor. All 99 installed materials and 48 authored
controls match the original material loader within the conversion scope below,
in debug and release. All 99 installed coefficient sets also pass through the
complete repeated vehicle/physical loop exactly. Complete Lacetti initialization
and a drivable Bevy lap remain open.

## Source layout and defaults

Aligned original `A720D0` takes destination and XML view arguments and returns
with `ret8`; its accessors are `A67B60` (float), `A67B30` (integer), `A67B90`
(boolean), and `A67A90` (attribute string). The first 13 words are the physical
material consumed by the previously verified row constructor/mixer:

| Offset | Source attribute | Default |
| --- | --- | --- |
| 00 | name, exactly four bytes | zero packed code |
| 04 | restitution | 0 |
| 08 | friction | f32 bits 3ECCCCCD |
| 0C | cfm | f32 bits 3C23D70A |
| 10 | correctionVel | 1 |
| 14 | minBounceVel | 0 |
| 18 | minSoftK | 0 |
| 1C | minSoftS | 0 |
| 20 | maxSoftK | 0 |
| 24 | maxSoftS | 0 |
| 28 | speedForMaxSoftness | 0 |
| 2C | dynamicScale | 0 |
| 30 | dynamicThreshold | 10 |

Word00 remains opaque, including when transported in the existing f32 material
array. It is not a numeric friction coefficient. Color34 is RGBA in byte order,
default `[255,255,255,180]`, signed integer attributes clamped to 0..255.
The loader initially writes C8FFFFFF but subsequently overwrites it with the
local alpha default B4; the final default is B4FFFFFF. Sheet38 defaults false;
case-insensitive true/false are recognized and other text preserves the default.
Padding39..3F is untouched in the probe.

Rust `surface::Physical::row_words` preserves the packed key and numeric bits.
The shared `contact_surfaces::Surfaces::physical` adapter uses the existing
packed-key first-record fallback policy and exports the 13-word solver layout.
The reporting CLI checks this adapter against the typed source reader for every
material. Missing mechanics does not prevent physical material loading. Strict
Rust guards reject nonfinite/overflowing coefficients and invalid/out-of-range
color integers; general malformed-text compatibility is not claimed.

## What the original comparison executes

The native probe builds original document/view layouts from bounded binary XML,
then calls original `A720D0` and its attribute/name/boolean helpers. Eight controls
exercise defaults, packed names and mixed/unrecognized boolean values with all
executable bytes unchanged. For the 99 installed materials and 40 varied
numeric/color controls, the private mapped image has **two explicit adapters**:
CRT float conversion BF3B60 uses host `strtod`; integer conversion BF630F uses
host `strtol` with the caller's base. Both entry patches are restored, instruction
cache flushed and restored bytes checked before exit. No original entry point,
imports or TLS/CRT initialization runs. Thus this verifies original loader,
default, field-store, clamp and lookup behavior with supplied numeric conversions;
it does **not** verify the original numeric parser. This retains the explicit
conversion limitation of the earlier mechanics research.

Controls cover all twelve coefficients, absent attributes, signed zero,
double-to-f32 rounding boundary `1.0000000596046448`, negative values, signed
color limits, hex/octal/sign/leading-space color inputs, clamp endpoints and
boolean cases. Original, independent original repeat, Rust and independent
Python field calculations agree on 147 materials ×15 words per profile.
Six additional Rust-only invalid/nonfinite controls reject before output creation.

## Complete loop handoff

Each installed coefficient set is assigned explicitly to the owned native car
shape and all owned track shapes. This deliberately shares one material identity
within each diagnostic run, preserving F200's verified scope. The assignment
does not establish the original live shape-to-resource mapping or implement
mixed physical materials. The original calls remain unchanged
`A02B70 → AD8D70 → A727A0`, with shared timestep and retained car/world state.

Per debug and release profile:

- 99 installed materials ×8 lengths (1,2,3,4,7,8,15,16) =792 runs;
  5,544 combined updates and 112,559,040 observable bytes exact.
- 392 generated contacts, 328 after the first update, 5,292 empty intervals.
- 495 consecutive authoritative velocity-feedback comparisons pass;
  source coefficient words remain exact through every final output.
- A separate 16-update control with the earlier authored coefficients matches
  original/repeat/Rust and differs from the source CAR_ run in 72 physical-body
  words. This demonstrates that supplied source coefficients affect the result.
- 44 paired native/Rust malformed ownership/framing/configuration guards pass.

The four-report current-binary/provenance audit passes 100 guard outcomes and
four Python AST checks. The previous mechanics loader also regresses exactly
on 89 installed +24 authored rows per profile after its XML-view helper was
shared with the new probe. The 161 workspace library tests, formatting and
all-target warnings-denied Clippy pass. No additional tools were installed.

## Evidence, failures and limits

Private evidence remains ignored under `local/behavior/`:

- `physical-material-{debug,release}-20261011-final-01/validation.json`.
- `source-vehicle-world-debug-20261011-final-01/validation.json` and
  `source-vehicle-world-release-20261011-final-02/validation.json`.
- `source-physical-material-final-audit-20261011.json`.
- `surface-mechanics-{debug,release}-20261011-f201-regression-01/validation.json`.
- Aligned loader/default/name evidence: `physical-material-loader-entry-20261011.json`,
  `physical-material-defaults-20261011.json`,
  `physical-material-attribute-names-20261011.json` and
  `20261011T1704403331878Z/`. The earlier mid-instruction neighborhood is not an
  authoritative function-entry disassembly.

The first coefficient-handoff validator compared raw native CRLF TSV against
Rust LF TSV and stopped before any original world call. Parsed-line comparison
corrected this; `source-vehicle-world-debug-20261011-draft-01/failure.md` records it.
Earlier successful draft reports remain private; final reports add the material
effect control and paired guards.

One read-only live-material attempt rejected the stale private GRID session at
its process-image guard, before game-memory reads. A single process inventory
then found no GRID process. No retry/input/launch/settings changes occurred;
`physical-material-live-failure-20261011.md` records the limitation. Shape C8 was
previously observed shared by the selected car/track shapes, but its current
resource identity and coefficients are not established by this failed attempt.

The F200 advance-list interpretation was corrected separately: world8/body318
is rebuilt by AD7040, distinct from registered pose queue24/28. The numerical
comparison and native ownership assertion were already correct.

Exact versions reused: Rust/Cargo1.99.0, Bevy0.19.1, Python3.12.14,
MSVC14.44.35207, DevPrompt17.14.41, SDK10.0.26100.0, Ghidra12.1.4,
JDK21.0.9 and Capstone5.0.6. Native builds use /O2 /MT /fp:strict /arch:SSE2
/W4 /WX; source resources are hash-guarded before/after.

Native physical loader SHA256
`2f078ef32479a494c5744c9d9dd80b00c90b784b092907632166191fc880b173`;
Rust report debug `d328446538692cbcf2eb169a82ff2cfaeb9f7922df873b542f31d7d5a69cb7b0`,
release `d0ad7119bf98e6cc9786b8bb250c9313eeb6c7fb36d32d5157f2fdabb665a015`.
The retained native world probe stays
`e119ad6367f16284d76134cc6227463f8723d4335c609398539f23b182c2be96`;
current Rust world CLI debug
`f373fcca05a6ad37c9bdca9a19630efeb9e0b81c81467e012fb3c20f8483f4f6`,
release `3803baae4469e08661e26034aafdd79b5df8bd525261f7b1ad4a52a36ecf4929`.
Other executable/input hashes are in the audited private reports.

Steam preservation at **2026-10-11T20:59:25.583616UTC** verifies all 130 baseline
resources and the original executable unchanged:
`local/scene/preservation-20261011-source-physical-material.json`.
Source surface SHA `8a23a960ea0ca837ee49bafaab893cff7446c0089c9ca282e131030f4b3d3251`;
car materials SHA `dfb9e93bfe6d9ebbf2bc639be244a567d2f4fffaa620601880e1b34cdede81e2`.

Next: recover source-complete Lacetti body/wheel/physical initialization and
spawn configuration; preserve the verified retained world in Bevy's driving
schedule. Mixed material ownership, registered pose normalization, original
damage callbacks/broadphase and the live asynchronous schedule remain explicit
limits, not verified behavior.
