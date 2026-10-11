# Source configuration through runtime handling (F207)

The ordinary whole mutable handling update now accepts the shipped Standard
HDM's brake-wear force divisor **10,016,380**. The old blanket one-million input
bound rejected it before execution. Only force threshold/divisor `2F0/2F4` now
use finite-value checks; all other bounds, active positive-divisor validation,
collision/notification restrictions and finite-output checks remain intact.
Rust and the shared native guard implement the same admission contract.

This closes a concrete source-integration blocker. It does not yet connect the
car to Bevy or establish a drivable lap.

## Original comparison

Whole unchanged `9F3E80` executes on the complete source-configured car and
source-constructed physical body. Each debug/release profile passes original,
repeat and Rust across **676 calls / 92,092,832 complete record bytes**:

- 516 source owners: 43 verified car/geometry pairs, both installed HDMs,
  three reset seeds and active/inactive wheel-view reset. Original outer
  active flags `B13/B14=1` are supplied explicitly. Delta includes zero,
  .001, .016, .02, .04 and .1; the original mutable update clamps to .02.
- 64 separate authored wear controls, retaining Lacetti source configuration
  and body. They vary finite force divisor (installed 10,016,380, 100 million,
  MAX and10), threshold, style, RNG and delta. Authored nonzero strength and
  multiplier values enable sensitivity that the shipped brake settings disable.
- 96 subsequent calls carry each branch's own complete car/body state through
  eight updates. There is no per-call reset of these owners.
- Twelve paired rejection checks cover nonfinite force parameters, unrelated
  parameter bounds and active zero/negative divisors; no output survives.

The physical body is unchanged by this selected routine, as independently
asserted for every record. The source factory's owned identity tokens survive;
the native adapter temporarily binds its own wrapper and component, then checks
and restores those identities. No original code or helper is replaced. Existing
lazy-global restoration, component ownership and output-removal guards apply.

Thirty-two positive wear controls produce nonzero levels and exactly four
per-car RNG advances; the thirty-two disabled-threshold controls preserve zero
wear and RNG. These are separate from source behavior: both shipped HDMs specify
zero brake strength damage scales, which legitimately prevent brake wear here.

## Regressions and audit

With the rebuilt shared guard and Rust binaries, each debug/release profile also
passes these fresh original/repeat/Rust regressions:

| Boundary | Coverage per profile |
| --- | --- |
| Complete retained car-manager/world loop | 300 runs, 6,150 updates, 42,636,000 bytes, 44 guards |
| Whole mutable handling | 396 calls, 53,947,872 bytes, 42 guards, 10 framing and six delta-cap checks |

The world runs produce 661 contacts, including637 after the first update, and
retain 175 independently checked feedback history pairs. Source audit checks
current executable/upstream hashes, all source owners/body-factory outputs,
complete identical inputs/three outputs, force sensitivity and all12 rejections.
Four regression reports pass current-hash/provenance audits, 32 world checks and
172 guards; framing/delta-cap results have a separate fresh audit. No skipped
test is counted as passing. Existing 163 library tests, formatting, all-target
warnings-denied Clippy and Python AST checks pass.

The original executable and 130 traced Steam resources remain unchanged at
**2026-10-11 22:56:26.360021 UTC**. Reused Rust/Cargo1.99.0, Python3.12.14,
MSVC14.44.35207/developer prompt17.14.41/SDK10.0.26100.0 and strict SSE2 floats.
No installations, desktop input, settings edits or owner assistance.

## Evidence and corrections

Private evidence under ignored `local/`:

- `behavior/source-handling-{debug,release}-20261011-final-01/validation.json`
- `behavior/source-handling-audit-20261011-final-01/audit.json`
- `behavior/source-handling-regression-{debug,release}-20261011-final-01/`
- `behavior/source-handling-world-{debug,release}-20261011-regression-01/`
- `behavior/source-handling-regression-audit-20261011.json`
- `behavior/source-handling-framing-audit-20261011.json`
- `scene/preservation-20261011-source-handling.json`

Draft-01 proves the source Lacetti whole call. Expanded debug draft-02 passed
all676 numerical comparisons, then failed an independent positive-wear coverage
assertion. Inspection showed source brake strength damageScale is explicitly
zero. The source cases remain untouched; positive wear is now tested with
separate authored strength/multiplier controls. This was a fixture assumption,
not a numerical mismatch or game failure. No native exception occurred.

Correction to an earlier interpretation: the constructor's zero **ratio-table
slot0 at18F4** does not mean selected gear0. Selected gear at196C is integer1.
The F203/F206 documents now distinguish these fields; the provisional
neutral-start blocker is withdrawn. Assists, outer source initialization,
resource activation and runtime world ownership still need explicit assembly.

Final SHA-256:

| Artifact | SHA-256 |
| --- | --- |
| Native mutable handling | `1167327faef3324511871bdad2110d23976bff1c85e868bbf7726352f062256f` |
| Rust mutable debug | `1e9a430ae4aebbb70766d59535c9f9ca56dd35f22cad3a2c18f737571ffa13cb` |
| Rust mutable release | `34530fe98d06af1d36e83aac41ddcf86a4e5285ec568c70f07197431353112db` |
| Native retained world | `f871adb32a55b32479deca45ca889342d90e654e299ac157628bc0a275bbce1b` |
| Rust whole vehicle debug | `0ad6892606a3f3edc56a9349b40f88b6b175b4f6e5bfb644f345363915c98ebc` |
| Rust whole vehicle release | `e624bfd219b3376d359ab9e178face4880ef1bc6023461e91d97f3b189418061` |

Next assemble the selected source outer prelude before Standard construction
(properties copy, mass scale, pose and active flags), preserve source body/world
ownership, and supply explicit player preferences/controls. Then validate that
source owner through the retained world and expose persistent state to Bevy.
Full outer/resource factory, original XML/CSV tokenizer, live scheduling,
general chassis collision, damage events and gameplay remain outside this claim.
