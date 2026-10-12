# Complete selected impact distribution

F243 implements the selected original impact dispatcher in Rust: mass-scaled threshold and normalization, world-to-local point transform, ordered damage channels, global RNG and handling effects. It is independently validated against the whole original `9E6660`, including `999C00`, the optional `9C46E0` channel and `9C5B90` effects. Runtime still retains its above-threshold guard until global RNG ownership and the surrounding handling update are integrated transactionally.

## Behavior and ownership

Aligned instructions establish a byte return and four stack arguments after the owner receiver. Decompiler floating-return chains are incorrect: the single-region helper returns void, while the caller keeps its normalized impact on x87 across channels. Every channel receives that same stored-f32 impact.

The original reads mass from the owned physical body's `3F0` and pose from its `1D0` matrix through the car's physical wrapper. It stores each mass/threshold or mass/divisor product to f32 before comparing/dividing. The wheel mode and other-impact mode select separate settings; impact at or below the threshold preserves all state and consumes no RNG. Reached normalized impact is capped at one.

Point translation is subtracted before three pairwise SSE dot products. Point W becomes one. Four wheel channels run first, followed by the driveshaft channel. In other-impact mode, a turbo flag controls the secondary channel and the radiator channel is visited. Exhaust follows; gearbox requires positive gear state. Each enabled region that reaches its falloff consumes one global RNG draw, even when its damage increment is zero. Main and secondary channels have different multiplication grouping for the three damage gains, which is preserved explicitly. Effects are published after all reached channels.

`collision_damage::calculate` accepts owned car/physical buffers and an explicit global random state and returns a staged result plus its updated random state. It does not substitute the car-local wear RNG at `1644`. Failure leaves its input unchanged. Optional component queries are absent in this selected dispatcher proof; the original wrapper/component graph is supplied in owned native buffers and canonicalized to zero tokens at the diagnostic boundary.

Selected admission includes four wheels, finite bounded points/pose/regions/settings, affine pose W components, zero external pointer tokens, nonnegative gains and a positive divisor for reached impacts. Zero active divisors and larger/general inputs are not claimed. Homogeneous region fields and input framing follow the earlier [localized helper](LOCALIZED_COLLISION_DAMAGE.md). No original resource acquisition or full outer factory is executed here.

## Complete comparison

All 768 cases match original/repeat/Rust debug/Rust release exactly: 104,629,440 complete bytes per run across 24 batches. Output includes the applied flag, global RNG, every car word and the unchanged physical body. There are 439 reached impact updates, 296 cases with RNG changes and 233 cases with changed damage levels.

| Cases | Scope |
| ---: | --- |
| 128 | Current source-built Lacetti with five NED regions and current zero gains |
| 384 | Authored active regions, gains, levels, coefficients, probabilities, falloff, channel gates and coherent effect thresholds |
| 128 | Exact impact thresholds and adjacent f32 controls, with mode/turbo/gear gates |
| 128 | Authored finite bases/translations to exercise transform arithmetic; these include non-orthonormal controls |

An independent Python caller projection recomputes threshold/normalization, point transform and channel/RNG scheduling, then uses the F241 Python region oracle to verify every level and final RNG. Complete effects and untouched bytes are compared against the whole original and both Rust profiles. Native canaries, wrapper identities, unrelated car bytes, physical state, input points and full mapped-image restoration are checked with no code replacements.

All 60 guards pass. Two explicitly incoherent terminal-effect fixtures execute the original dispatcher and are rejected after nonfinite original effects; Rust also rejects without producing output. Other framing/owner/value controls reject before native execution, and existing-output controls preserve their marker. The read-only audit rebuilds all requests, verifies six current pinned inputs, rereads 120 primary files and checks complete outputs, Python projections, unchanged data and guards. Its fixture/projection code is shared with the validator; it is not a second independently derived arithmetic implementation.

## Failure investigation and limits

The first twelve source/authored cases matched completely. The first wider sweep stopped after the source cases because authored positive damage had been combined with shipped settings that disable those channels at terminal damage. At threshold one, the original effect calculation reaches `0/0`; both implementations reject the resulting nonfinite effects. The failure is retained privately. Authored active-channel fixtures now explicitly use coherent effect thresholds, while two reproductions of the incoherent configuration remain negative guards. No arithmetic tolerance, original output or runtime guard was relaxed.

The earlier [source-region integration](SOURCE_DAMAGE_REGIONS.md) remains the latest playable app. Four wheel regions and gain defaults are still incomplete in its startup. Static constructor tracing now establishes that the outer `A0E880` initializes mutable defaults before Standard construction and installs wheel regions afterward. The Standard constructor's temporary mutable constructor preserves incoming mutable storage; it does not fill the missing defaults in the old zero-filled startup. These observations require their own source/prelude proof before changing runtime initialization.

Next: carry a distinct global damage RNG through driving/reset/rollback, join the dispatcher at the original handling-update point, validate the reached-impact update against the whole original handling producer, and rerun controlled heavy contact cases. This dispatcher proof alone does not establish heavy-crash driving, notifications, visual deformation or a full lap.

## Private evidence and tools

- `local/behavior/collision-damage-20261013-final-03/validation.json`, SHA-256 `8fcc7266bdd980140db171100970087ba485982b855a6b3f107b8937e7eb9e56`.
- `local/behavior/collision-damage-audit-20261013-final-03.json`; `collision-damage-additional-provenance-20261013-final-03.json` hashes shared Python oracles and Rust sources.
- Native executable SHA-256 `50943178ddd3b0a7e27e99538d08134a35496029d17d12f6dbe6f91a71978d9c`; Rust debug/release `0cf5dacfa582a930ee9dd7cf1d90d5f51433f4c0d6a5a4a6f92059b111123de2` / `bf3a3148432cc35e361cebc3242646e5e11b5b13894ccdf2c67fe73891b55a1a`.
- Earlier `collision-damage-20261013-draft-01`, stopped `collision-damage-20261013-final-01`, isolation `collision-damage-isolation-20261013-draft-01/failure.json` and successful widened `collision-damage-20261013-final-02` are retained.
- Ghidra `20261012T2119424408730Z` constructor references and `20261012T2124078696226Z` observed mass/pose/outer-constructor entries, under `local/behavior/`.
- Aligned `damage-impact-dispatch-abi-20261013-draft-01.json` and `damage-local-transform-abi-20261013-draft-01.json` under `local/behavior/`.
- Library/Clippy logs `local/behavior/collision-damage-{libraries,clippy}-20261013-final-01.txt`; Steam preservation `local/scene/preservation-20261013-collision-damage.json`.

Reused Rust/Cargo 1.99.0, Python 3.12.14, MSVC 14.44.35207/SDK 10.0.26100.0, Ghidra 12.1.4/JDK 21.0.9 and Capstone 5.0.6. All 187 regular library tests, formatting, all-target warnings-denied Clippy, two Python syntax checks and whitespace checks pass. The five source driving tests are default-ignored by this library command and are not newly claimed for this unintegrated dispatcher. A formatting-shifted patch anchor failed atomically and was reapplied; a read-only PowerShell brace-glob search failed before execution and was corrected. No installations, desktop input or Steam/settings changes were needed.
