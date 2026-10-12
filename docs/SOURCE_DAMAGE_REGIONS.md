# Source damage-region initialization

F242 installs the five named handling-damage boxes from the purchased Lacetti NED into source-built runtime startup. It preserves the existing RNG state and every unrelated owner word. Missing regions leave their records unchanged; incomplete or ambiguous recognized regions reject the entire update before committing.

The original `99A5E0` loader and binary-document/accessor dispatch establish the mapping for gearbox, driveshaft, turbo, radiator and exhaust. Source center attributes are converted to f32 and added to the already selected car-local offset at `1630`. Source size attributes are stored directly as extents; they are not halved. Center W and extent W become one, enable byte becomes one, and the remaining record padding is preserved. The new `grid-formats::damage_boxes` reader and `Car::configure_damage_regions` implement this selected setup. Runtime calls it between source suspension setup and the existing source HDM/reset stage.

## Native comparison

The native probe executes the whole original named-region loader, document context construction, child selection and attribute access on owned binary-XML tables. Numeric conversion is explicitly adapted to host `strtod`, as in the earlier [HDM configuration proof](HANDLING_CONFIGURATION.md). Source acquisition, original text tokenization and the original numeric CRT implementation are excluded. This is not an unchanged-code numeric-parser proof. The adapter's five entry bytes are restored and the entire mapped image is checked afterward.

Original/repeat/Python/Rust debug/release match all 272 cases and every one of the 65,280 output bytes per run:

- 128 actual Lacetti source cases: 64 offsets, including the current source-built car offset, each with zero and A5 padding fills.
- 128 authored cases: all combinations of absent named regions, finite decimal/signed-zero/rounding controls, independent offsets and both fills.
- 16 empty cases run without the numeric adapter; none requires numeric conversion.

All 48 framing, nonfinite, extent, duplicate/nested-parent and existing-output guards pass without creating outputs or replacing an existing file. Native execution checks record/context canaries, unchanged document/input bytes and complete image restoration. Original missing numeric attributes leave their output operands uninitialized; the supported admission rejects such inputs before calling original code rather than assigning invented defaults.

The read-only audit rebuilds each source/authored XML and request, rechecks all seven current input hashes, rereads every complete output and verifies the numeric-adapter accounting and guards: 1,632 primary files. Fixture rebuilding is shared with the validator; the validator's independent Python projection supplies the arithmetic comparison.

## Runtime validation

The fresh source startup snapshot contains all five native-matching region records. A complete old/new snapshot comparison permits only those 240 record bytes to change and confirms the other 140,216 bytes are identical. A late malformed exhaust region leaves the full car and RNG unchanged in the transactional test.

All five purchased-source release driving tests pass. The 240-tick clear straight retains the earlier exact wheel-driven velocity, while the 600-tick turn still first touches the boundary at tick 165 and produces 176 contacts with final Y=-14.807243. All three Bevy keyboard/presentation/recovery tests pass. The release app was rebuilt with this startup integration; no new interactive desktop or full-lap test is claimed.

This closes the five named-region gap found in [F241](LOCALIZED_COLLISION_DAMAGE.md). Four wheel regions remain disabled in current startup, the three damage gains remain zero, and the above-threshold impact guard remains. Validating mutable-constructor defaults, wheel-region setup and the complete impact dispatcher is next. This change alone does not enable heavy crashes, damage effects or visual deformation.

## Private evidence and versions

- `local/behavior/damage-boxes-20261013-final-02/validation.json`, SHA-256 `5a3eaff1978196fddf64f6ec10d0d746f82662ca0ccd58c6fc116d01563f3314`.
- `local/behavior/damage-boxes-audit-20261013-final-02.json`; `damage-boxes-startup-audit-20261013-final-01.json`.
- `local/behavior/damage-boxes-driving-20261013-final-01.txt`; `damage-boxes-app-tests-20261013-final-01.txt`; `damage-boxes-clippy-20261013-final-01.txt`.
- Native SHA-256 `89c97c91e9aecd292c7511ecbb48cc58a9cc60464afb025f6f5b481ecd90fd87`; Rust debug/release `58bc21eb6f816d31c070071956da7f84da5be2d6ed46d3419f4e5884cd6aeb0e` / `45abb2be070df57c76e17a03d1445a2cd37b4c188f9d2bc4ce0f52656a092664`.
- Lacetti NED SHA-256 `f71ce1224b10454f9fa76ee20af791e5f8e25eea160b6e1f76ff2bdffb2a26ba`; all original values remain local.
- Ghidra `20261012T2056389657560Z`, `20261012T2103099058557Z`, `20261012T2109106461488Z` and aligned `damage-region-box-abi-20261013-draft-01.json` / `damage-box-context-abi-20261013-draft-01.json` under `local/behavior/`.

Reused Rust/Cargo 1.99.0, Bevy 0.19.1, Python 3.12.14, MSVC 14.44.35207/SDK 10.0.26100.0, Ghidra 12.1.4/JDK 21.0.9 and Capstone 5.0.6. All 187 regular library tests, formatting, all-target warnings-denied Clippy, two Python syntax checks and whitespace checks pass. The first 272-case proof passed; nested-parent admission and document-preservation checks were then strengthened and the complete comparison/audit rerun. Harmless guessed private filenames were corrected through inventories. No tools were installed, no desktop input was sent, and Steam/settings were not changed; preservation evidence is `local/scene/preservation-20261013-damage-boxes.json`.
