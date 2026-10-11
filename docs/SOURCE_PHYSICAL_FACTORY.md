# Source car-to-body factory (F206)

Whole original `9B8D00` and the independent Rust source adapter now agree for
the selected source-configured cars, empty world and one-cell body pool.
This joins source mass/inertia and actual descriptor defaults to the verified
[body constructor](PHYSICAL_BODY_CONSTRUCTOR.md), including allocation and the
wrapper/backlink. Resource shapes and runtime attachment remain separate.

## Executed boundary

The original calls its descriptor initializer `6174F0`, supplies source mass
and inertia, and calls `A742E0`. That function uses original pool allocator
`AD9220`, complete `B11F40`, and wrapper constructors `A69200`/`A7E180`.
The car receives its wrapper and clears its seven factory-owned link slots and
byte `37090`. All other car bytes must survive unchanged.

The native adapter owns a zeroed world, manager and one-cell fixed pool, plus a
real initialized Windows critical section. Exactly two original IAT slots are
temporarily supplied with the real host `EnterCriticalSection` and
`LeaveCriticalSection` addresses; original locking helpers execute. No original
executable instruction/helper is replaced. Pool critical-section bytes and
temporary manager addresses are external ownership, not numerical model output.

The original factory's timer-global subtraction is checked independently before
restoration. This is an observed side effect of the original factory, not a
Rust engine timing implementation. The six private pages `E58000`, `E5A000`,
`E5B000`, `E5E000`, `C89000` and `E8F000` are restored, including both IAT slots.
Complete file-backed section comparisons and allocation sentinels pass before
any output. No PE entrypoint, resource allocation or application TLS runs.

`PF01` input holds selector, cold body fill, complete decoded source properties
and a `370A0`-byte owned car with absent wrapper/manager tokens. Output compares
**226,828 bytes** without numeric exclusions:

| Owned storage | Bytes |
| --- | ---: |
| Complete selected car | 225,440 |
| Physical body including wrapper backlink/tail | 1,072 |
| Wrapper including unchanged last padding word | 16 |
| Empty world including registration | 256 |
| Pool controls before critical-section storage | 28 |
| Cell allocation header including untouched padding | 16 |

Body-owned pointers use offset+1, external world `10000000`, wrapper
`20000000`, pool `30000000`; car's wrapper and world's body references use 1.
The wrapper vtable is its original-address identity `D314E4`, never executable.
Car manager token returns to input 0. Tokens are never dereferenced by Rust.

## Verified coverage

Each debug and release profile passes original/repeat/Rust across **408 cases /
92,545,824 complete bytes**:

- 43 verified source car/geometry owners, both source HDMs, both world selectors
  and zero/CD body fills: 344 cases. The owners are the independently verified
  cold constructor plus actual source handling configuration/reset, with seed
  200679 and active wheel-view reset.
- All 44 installed source mass/inertia records on explicitly cold owners,
  including the variant without verified geometry, plus 20 authored positive
  mass/inertia controls: 64 cases. These are not complete gameplay owners.
- Fourteen malformed variants in native/Rust: **28 rejection checks per
  profile**, no output. They cover framing, selectors/fill, mass/inertia bounds,
  nonfinite/underflow values, and foreign existing wrapper/manager identities.

The final audit checks current image/binary/upstream/source/archive hashes,
every request and all three outputs, allocator/wrapper/world/registration
invariants, source-specific zero overrides and MAX sentinels: **816 complete
output sets**. All non-factory car storage is independently checked unchanged
during validation. Existing 163 library tests, formatting, all-target
warnings-denied Clippy and both Python ASTs pass.

Original Steam executable and 130 traced resources remain unchanged at
**2026-10-11 22:48:19.925239 UTC**. No desktop input, settings changes, installs
or owner assistance. Reused Rust/Cargo 1.99.0, Python 3.12.14, MSVC 14.44.35207,
developer prompt 17.14.41, SDK 10.0.26100.0, strict SSE2 floats; existing Ghidra
12.1.4/JDK 21.0.9/Capstone 5.0.6 for private static evidence.

## Evidence, corrections and next actions

Private evidence remains ignored under `local/`:

- `source-physical-{debug,release}-20261011-final-01/validation.json`
  and `source-physical-audit-20261011-final-01/audit.json` in `local/behavior/`.
- `local/scene/preservation-20261011-source-physical.json`.
- `local/behavior/physical-factory-envelope-aligned-20261011.json` and prior
  surveys `20261011T2216395011890Z`, `20261011T2217273724180Z`,
  `20261011T2218487319048Z`.
- Source property/handling upstream reports and each source owner hash are
  recorded in validation. The first and failed comparisons are retained in
  `source-physical-20261011-draft-01`; wider debug draft-02 passes all408.

The first Rust comparison differed at exactly three body words: two squared
sleep thresholds and coefficient `3F8`. The generic descriptor initializer's
defaults are subsequently overwritten by the car factory. Rust now follows
those overrides (zero), verified against the whole original. This is a corrected
interpretation, not an original-game failure. A guessed driver source filename
failed harmlessly and was resolved from inventory. No native exception occurred.

Final SHA-256:

| Artifact | SHA-256 |
| --- | --- |
| Native source probe | `a8a50d0f08d60eaf5ccad6d941e190c30cb8a06588f48591b052db2e3b66b962` |
| Rust debug | `598f94efbef02bdeeea2e3c83d419bee852f931eb959af60fcdbf964b24cf272` |
| Rust release | `174b765034b361638de4aac0fe3de44e7a08c17d0ad234fac8f352a3329019ca` |
| Analysis image | `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514` |

Full outer `A0E880` construction, resource/shape factory `A01960`, spawn and
source driving remain unverified. Its selected source prelude copies properties
to `E78`, supplies mass scale `1560=1`, active flags `B13/B14`, and publication
mode; these must be established before runtime attachment. The ordinary
handling update's old generic bound also rejects the shipped brake-wear divisor
10,016,380; a targeted finite-force admission fix and source runtime comparison
are now underway. Cold gear zero requires a supported start/engagement contract.
Continue these steps into the retained vehicle world and Bevy fixed-step driving.
