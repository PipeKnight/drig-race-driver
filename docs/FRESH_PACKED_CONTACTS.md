# Fresh Rust packed body-contact generation (F234)

`packed_contacts::calculate` now composes source moving-mesh bounds and packed
candidate traversal with fresh triangle/mesh queries, witness depth and normal
selection, witness orientation, and retained manifold assembly. It reads no
reference contact record. It retains one support/query context and manifold
through the ordered triangles of each cold pair, stopping publication at the
supplied contact capacity. The carried distance-query direction remains distinct
from each reoriented publication normal.

This removes the replay requirement at the selected Lacetti/Donington contact
boundary. It does not yet add collisions to the driving app.

## Scope

The selected packed shape is static and untransformed, with its four backface
correction extent words equal to maximum finite float. Other extent states are
rejected: [F233](PACKED_CONTACT_CANDIDATES.md) establishes why the nested vertical
correction cannot run in this fixture, not in every production shape.
The moving mesh has initialized source face normals and one-level topology.
The original 44-direction penetration table is supplied privately from the
identified installation, never embedded in public source.

Depth uses the original reciprocal-square-root seed/refinement and strict
`depth > 0x3A83126F` admission. Non-sheet normals use the witness difference;
sheet normals use normalized triangle winding. The verified F232 orientation
and F231 retained append/duplicate behavior then produce the manifold. The
diagnostic retains the selected zero material coefficients and publication flag
one from F229. Full material/resource initialization, populated correction
extents, warm pair reuse and original broadphase scheduling remain separate.

`grid-packed-contacts PC08_INPUT local/OUTPUT PRIVATE_CD01` rebuilds source mesh
topology and initialized normals independently, checks adjacency and face normals,
reads the packed chunk, and emits the entire F233 candidate projection followed
by contact count, final query direction and all 16 contact slots. Unused contact
templates and array slots are included. No native answer is an input.

## Verification

Both actual Rust debug and release outputs match the original/repeat F229
records across **2,642 cases**, **111,448,456 selected bytes per profile**,
**5,039 contacts** and **1,098 positive cases**. This includes all 245 Donington
chunks, source/all-sheet policies, position and capacity controls. The source
policy subset contains 1,417 cases, 608 positive placements and 3,134 contacts.
Counts, final query direction, complete contact arenas, candidate bounds, counts,
stamps, generation, cache-use and both complete arrays are compared exactly.

The debug validation additionally reruns the unchanged whole B39500/AEA1F0
native probe and compares all **131,030,960 complete owned bytes**, not just the
Rust projection, against the pinned original and repeat. The independent
current-hash/source/output audit passes **13,210 complete files and 38 guard
records**.
The Rust projection does not reconstruct every opaque owned native object;
complete worker/scratch/pair equivalence is not claimed.

All 182 regular workspace library tests, formatting and warnings-denied
all-target Clippy pass. Two purchased-source driving tests remain ignored by
default and were last explicitly run in F230. A synthetic integration unit
checks fresh positive contacts, capacity one, separation, disabled collection,
invalid private directions even on a disabled path, and rejection of populated
correction extents. Each executable passes 19 malformed/private-table/existing
output checks. No valid source numerical mismatch or native fault occurred.

Draft failures were diagnostic only: a guessed Python import was corrected to
the inventoried `extract_contact_directions`; a unit moved its mesh receiver
before evaluating its span, fixed by precomputing the span. A later exploratory
read guessed nonexistent adapter filenames; inventory resolved the actual
physical integration files. No evidence or source installation was modified by
those failed reads.

Private evidence:

- `local/behavior/packed-contacts-{debug,release}-20261012-final-01/validation.json`
  has SHA-256 `46bd1822c9326b988b600f4be27fe9f2d5acc17328237eb7a93d9ec85f54ba7c`
  (debug) and `67371e621c14249c4c4497053fe06f0a47ba92b7b4a1b195bc6ef88b65284490`
  (release).
- `local/behavior/packed-contacts-audit-20261012-final-01.json`
- Matching `packed-contacts-*` build, library and Clippy logs; failed smoke01
  and unit01 logs retained privately.
- Rust debug SHA-256
  `4c532e481c8d163c3b40638a2f3cceff40038c0729e373c7d9e725fad241a13a`;
  release `6b66c8a01a191fabea987c7d4d0fd8b570a14152bce8fb210cde95537992da50`.
- Native PC08 SHA-256
  `a69ea1b5ee57affd59e5fc009cf489a2cbc8108323008e6fb18aa052b637b0a5`.

Installed Rust/Cargo 1.99.0, Python 3.12.14, MSVC 14.44.35207 /
DevPrompt 17.14.41 / SDK 10.0.26100.0 were reused. No installs, UI input or
settings changes were required. All 130 tracked Steam resources and executable
match their preservation baseline at 2026-10-12 19:12:42.183879 UTC, recorded in
`local/scene/preservation-20261012-fresh-packed-contacts.json`.

## Next physical boundary

The driving world currently produces at most four wheel contacts before
physical advance. Fresh body contacts belong after pose advance and before
grouping/solving; putting pre-advance geometry into the existing call would use
the wrong frame. A provider at that boundary must see the advanced published
body, append world-space geometry after the relative wheel prefix, retain a
stable enlarged pool, and preserve complete state on failure. Source body-shape
attachment, local-to-owner transform and track shape identity must also be
explicit before enabling runtime barriers.
