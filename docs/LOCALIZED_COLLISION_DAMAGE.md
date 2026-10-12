# Localized collision damage

F241 validates the original single-region damage distribution and its global RNG in independently authored Rust. It does not yet enable the above-threshold collision-damage path in driving. The playable boundary build from [F240](CQTC_BOUNDARY_CONTACTS.md) remains unchanged.

## Verified behavior

The observed `999C00` entry takes seven cdecl arguments and returns void. Aligned instructions establish the ABI; decompiler-inferred floating returns at callers are not evidence of a returned impact value.

For an enabled region, the routine clamps the impact point against the three box half extents. Inside the box, attenuation is one. Outside it, distance beyond the box falls linearly over the configured radius. The exact radius boundary skips both damage and RNG. Squared underflow with a zero radius produces unordered attenuation, which also skips. Disabled regions preserve the previous damage word even if it exceeds one; reached updates cap the resulting level at one.

The reached update consumes one global RNG draw, then applies the first or second configured damage multiplier using strict probability comparisons. The global generator uses wrapping multiplication/addition and a signed quotient before selecting its sample byte. This differs from the existing car-local wear generator. Ordered f32 arithmetic and the original x87 sample-to-f32 store are preserved; no numeric tolerance was introduced.

The Rust module admits finite, bounded scalar/box inputs. Homogeneous center and half-extent W components may be zero or one; point W must be one. The original region loader writes half-extent W=1, but this damage helper clamps only XYZ and leaves the W distance zero. The diagnostic requires reserved wire words to be zero and writes only to a canonical private output parent, with create-new semantics.

## Comparison and audit

`tools/validate_damage_region.py` executes the whole original helper on owned, canaried records and compares complete level/RNG results against a Python arithmetic reconstruction and both Rust profiles:

| Cases | Scope |
| ---: | --- |
| 4,096 | Authored mixed box positions, enabled/disabled regions, gains, prior levels and seeds; both half-W representations |
| 6,144 | Exact probability samples and adjacent threshold controls across 2,048 seeds |
| 30 | Signed XYZ falloff and radius endpoints |
| 1 | Squared-underflow/zero-radius skip |
| 288 | Actual current prototype startup's disabled region records |
| 288 | Source HDM coefficients/settings joined to explicitly authored active boxes and gain |

All 10,847 cases match exactly: 86,776 complete result bytes per run, with 9,072 reached RNG draws. Original/repeat/debug/release output SHA-256 is `b47de778391d1f9632dd881141e21e5473b65325591b18d36e7d095c135ecde7`. All 39 malformed/existing-output guards pass. Native execution checks canaries, unrelated inputs, complete mapped-image restoration and zero code replacements. `tools/audit_damage_region.py` independently rereads all four outputs, checks current six input hashes, rebuilds the complete request and checks every skip/draw transition and all guards. The validator supplies the independent arithmetic reconstruction; the audit shares only the deterministic fixture generator.

Private evidence:

- `local/behavior/damage-region-20261013-final-02/validation.json`, SHA-256 `53cd3430d6013be18ff17fe786015db647662b5875b731c3c19ab35155106f02`.
- `local/behavior/damage-region-audit-20261013-final-02.json`.
- Native executable SHA-256 `1b119e01495284d368d1af6267c9d44daba8ff5a1f37072ce712a652fe894c62`.
- Rust debug/release SHA-256 `efb028399eb5aed4fce7851abfe65d589536c5efa0452b7596da5a85aa828248` / `fb47028005d4a8fc80e7877562dd579c6cb3b37fe150fe6685b0ada02b1d4a3c`.
- `local/behavior/collision-damage-abi-20261012-draft-01.json` and `local/behavior/damage-region-box-abi-20261013-draft-01.json` contain aligned instruction evidence.
- Ghidra output directories `20261012T2047363112903Z`, `20261012T2056389657560Z` and `20261012T2103099058557Z` under `local/behavior/` contain the observed helper, RNG, region loader and XML accessors.

## Startup gap and next integration

Subsequent [F242 source-region integration](SOURCE_DAMAGE_REGIONS.md) initializes the five named NED regions. The snapshot observations below describe the earlier F241 startup; wheel regions, gains and full impact dispatch remain open.

The fresh source-built prototype snapshot `local/behavior/damage-region-20261012-draft-01/source.state.bin` has all nine selected damage boxes disabled/zero and three damage gains zero. This describes the current incomplete prototype startup, not the original game. Its HDM coefficient/settings joins above use explicitly authored active boxes/gain and do not establish original car-region initialization.

The original mutable constructor initializes region center W components and unit damage gains before its reset. The separate NED region loader reads five named `HANDLINGDAMAGE` children (gearbox, driveshaft, turbo, radiator, exhaust), adds the supplied local offset to their centers, stores source size values directly as extents, and enables each present child. These observations require a guarded original-loader comparison and constructor-order/RNG proof before changing runtime startup. Wheel regions and outer resource acquisition remain separate.

Next: validate source region setup, then the surrounding impact threshold/normalization, world-to-local transform, channel order, global RNG ownership and effects. Retain the runtime damage guard until that complete path is validated. Heavy crashes, visual deformation and a complete original factory are not established by this helper proof.

## Tools and verification

Reused Python 3.12.14, Rust/Cargo 1.99.0, MSVC 14.44.35207 (Developer Prompt 17.14.41; SDK 10.0.26100.0), Ghidra 12.1.4/JDK 21.0.9 and Capstone 5.0.6. No installation or desktop input was needed. All 186 regular workspace library tests pass; five purchased-resource tests are explicitly ignored by that command and are not newly claimed here. Formatting, all-target warnings-denied Clippy and Python syntax checks pass.

The initial five-case draft and first full comparison succeeded. Extending half-W admission followed aligned loader evidence and received a fresh full comparison; no tolerance or expected-value relaxation occurred. Harmless guessed filenames/globs failed read-only searches and were corrected using inventories. The private filename date follows the session date; tool UTC timestamps remain those recorded in their output directories. Original Steam resources/settings were not modified, and the F240 playable executable was not rebuilt during this helper work.
