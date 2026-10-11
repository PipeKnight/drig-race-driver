# Complete supplied-mass physical-body constructor (F205)

The Rust constructor matches the complete original `B11F40` body constructor
for the selected supplied positive diagonal inertia, no-shape, empty-world scope.
This closes body initialization and registration, before allocation and the car
factory. It does not yet make the Bevy viewer drivable.

## Boundary and observations

- Input: `PC01`, world-list selector 0/1, cold fill 0/CD, and 352 bytes of
  constructor settings. Positive supplied mass, diagonal inertia with last
  diagonal 1; no shape references/count. Flags must be Boolean.
- Output: **1,072 body + 352 mutated settings + 256 owned world = 1,680 bytes**.
  The actual body extends through `42C`, size `430`; the existing runtime
  comparison boundary of `424` omits the wrapper backlink and two tail words.
- The original base constructor, full cofactor inverse, mass/center/pose history,
  derived matrices, activation and world registration execute unchanged.
  The source settings' pose W lanes are normalized even when supplied pose is
  disabled. Untouched body bytes retain their specified initial fill.
- The diagonal inverse follows the original cofactor products and refined SSE
  reciprocal seed, rather than independent reciprocal divisions. Float words,
  signed zeros, full padding/fill, settings mutations and world lists compare.
- Body-owned pointers are checked and encoded as relative offset + 1 (self=1).
  The external owned world is token `10000000`; world body links are 1.
  These tokens are identities, never dereferenced by Rust. Caller settings' world
  pointer returns to its input token 0 at the comparison boundary.
- Active bodies join selector-specific head/tail slots `3C/40`, `44/48`; body
  `400` contains selector. Inactive `400` is `FFFFFFFF`. Both register in
  world `24`. This registration list differs from the later world `8` advance
  list documented in [VEHICLE_WORLD.md](VEHICLE_WORLD.md).
- The five source sleep coefficients accept finite `f32::MAX` sentinels; the
  limit at settings `EC` specifically preserves positive MAX rather than
  squaring it. Other squared values and consumed arithmetic must remain finite.

`grid_world::physical_constructor::initialize` is immutable/transactional.
`grid-physical-constructor` is a bounded diagnostic with new private output files.

## Verification

Each debug and release profile compares original, independent original repeat
and Rust across **544 cases / 913,920 complete observable bytes**:

- All 44 installed Standard source mass/inertia records, both selectors,
  active/inactive flags and zero/CD cold fills: 352 cases.
- 192 authored pose, offset, velocity, coefficient, inertia, signed-zero,
  ignored-pose normalization and MAX-limit controls.
- 19 malformed cases in both native/Rust adapters: **38 rejection checks per
  profile**, with no output. These include framing, pointers, shape count,
  invalid flags, mass reciprocal overflow, determinant underflow, off-diagonal
  inertia, nonfinite velocities and finite limit whose square overflows.

The native constructor does not replace any original code or import. Its four
lazy-constant pages (`E58000`, `E5A000`, `E5B000`, `E5E000`) are snapshotted,
made writable individually and restored. Boundary sentinels and complete
file-backed section comparisons must pass before output. No entrypoint,
resource acquisition, original allocator or application TLS is emulated.

The final audit checks current binary/image/upstream hashes, every input and
all three complete outputs, original source hashes, no-output rejection state,
and independent registration/pointer/settings invariants: **10,956 assertions**.
Library tests: 163 (23 formats, 2 tools, 138 world); formatting and all-target
warnings-denied Clippy pass. Both Python tools parse successfully in the audit.
The 130 baseline Steam resources and original executable remain unchanged at
**2026-10-11 22:36:45.169801 UTC**.

## Evidence and versions

All binary/source-derived evidence stays ignored under `local/`:

- `local/behavior/physical-constructor-{debug,release}-20261011-final-02/validation.json`
- `local/behavior/physical-constructor-audit-20261011-final-01/audit.json`
- `local/scene/preservation-20261011-physical-constructor.json`
- `local/behavior/physical-body-constructor-aligned-20261011.json`
- Private static surveys `20261011T2218487319048Z`, `20261011T2221122060910Z`.
- Hash-guarded upstream source properties:
  `standard-properties-debug-20261011-draft-05/validation.json`.

Reused Rust/Cargo 1.99.0, Python 3.12.14, MSVC 14.44.35207 (developer prompt
17.14.41, SDK 10.0.26100.0, strict SSE2 floats), Ghidra 12.1.4/JDK 21.0.9,
Capstone 5.0.6. `CARGO_INCREMENTAL=0` avoids the documented OneDrive link issue.
No installs, desktop input, game settings changes or owner assistance.

Final SHA-256:

| Artifact | SHA-256 |
| --- | --- |
| Native probe | `1839d38ca8d21dc605bafd1fbf018343a78485d6977cfa1f90345522926787b8` |
| Rust debug | `ab94bed4bfa54d8a32a0e2e56118fc754253cf896c641b20e6e99cd1e4c2450b` |
| Rust release | `cd5459c907a90c043686288260409bb9e1f8c3d1a89b57ce3e95293172e91a0d` |
| Private analysis image | `aac46a6449522e26ddd1f8b0cf1a6075852e4aeecd97b7d121453c5ce4f34514` |

## Drafts, uncertainty and next actions

The first native/Rust complete comparison and wider draft passed. Initially a
generic input bound excluded source MAX sentinels; passive sleep and special
limit fields now admit finite MAX explicitly. Added determinant, inverse-mass
and finite-output rejection checks. Final-01 passed; a Clippy range-loop finding
was corrected, then final-02 rebuilt and reran both profiles before the audit.
A transient patch matched the earlier tensor-validation loop rather than the
inverse loop; inspection corrected it before compilation. Several guessed
source/header/doc and private survey paths failed harmlessly and were resolved
from inventories. No original numerical mismatch or native exception occurred.

The settings are still authored apart from installed mass/inertia; do not label
them the complete source factory descriptor. Source `9B8D00` constructs that
descriptor and calls `A742E0`, which allocates the body, invokes `B11F40`, and
constructs `A69200` wrapper/backlink. A separate first native proof of that whole
chain now succeeds with an owned one-cell pool and two real host critical-section
imports (`source-physical-20261011-draft-01`); Rust/full comparisons are pending.
Next: verify this complete source factory, preserve its ownership when attaching
the configured source car to the retained world, address the source HDM runtime
admission bound, then expose the persistent vehicle loop to Bevy. Shape/resource
factory `A01960`, spawn, mixed materials and live scheduling remain separate.
