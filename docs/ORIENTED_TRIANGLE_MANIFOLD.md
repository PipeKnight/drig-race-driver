# Reoriented retained triangle manifold

F232, 12 October 2026. Whole original AEE640 followed by AE2FA0 and its feature,
clipping, duplicate and publication stages matches independently authored Rust
in actual debug and release builds. The comparison covers **1,421 sequences /
49,576 retained calls /200,932,204 complete native bytes per run**. Native repeat
and both Rust builds agree on every interval's complete sixteen-slot arena,
query record, two diagnostic features, normal, witnesses and support cache.

## Composition and limits

`contact_manifold::calculate_oriented_append` uses the triangle's first vertex
and moving mesh transform origin as orientation anchors, reorients the supplied
normal/witness pair, then runs retained manifold assembly. Successful normal
reversal multiplies all four lanes by minus one, preserving signed zeros, and
swaps the witnesses. The input normal and depth are still **caller supplied**;
the API does not implement the packed caller's normal/depth derivation, sheet
selection, later backface correction, traversal, or fresh moving-frame queries.

Existing manifold/cache inputs remain unmodified on failure. Orientation is
performed even when remaining manifold capacity is zero, while the original
zero-capacity manifold preserves its cache. Existing cold and PC09 paths remain
separate. The playable wheel-only executable was not rebuilt or replaced.

The original anchor contract is in
[the reorientation research](CONTACT_REORIENTATION_RESEARCH.md). The actual
selected source seed has a **nonzero difference** between body center and mesh
transform origin, so substituting the body center would change this boundary.
The validator reconstructs the source transform translation with the same
two rounded operations as the owned original preparation.

The new PC10 wire extends each PC09 48-byte control record with a second
16-byte input witness. The existing fallback-point field is the other witness.
Both orientation anchors are derived from owned source geometry/transform,
rather than supplied as extra unrelated vectors. Original AEE640 executes
unchanged before the existing feature/manifold path. Query +90 retains the
other output witness; +10/+20 hold oriented normal/fallback witness. Captured
intervals retain the same A20 size and every original final owned output remains
observable. Original data pages/section contents, canaries and source mesh,
chunk and body immutability use the existing checked native harness.

## Coverage and audit

All 245 Donington chunks provide an eight-step sequence at their selected
ground-height placement. Another 1,152 sequences span 24 spaced placements,
eight normal controls, six capacities and 16/64 steps. Authored source-relative
witness offsets exercise opposite projections, retained/replaced normals and
zero/small displacements. An additional 24 sixty-four-step sequences place the
mesh transform origin exactly at zero and exercise zero plus adjacent float
values around the original small-length threshold (3727C5AC), with capacities
0/3/16 and all eight normals. These are fixed-transform boundary tests, not
detected driving trajectories or measured source query witnesses.

| Check | Result |
| --- | ---: |
| Retained calls, each implementation | 49,576 |
| Normal flips with witness swaps | 13,580 |
| Unchanged normal/witness pairs | 35,996 |
| Zero triangle-anchor displacements | 6,453 |
| Final contact count summed over sequences | 4,816 |
| Existing depth increases | 214 |
| Retained point/normal/metadata checks | 126,977 |
| Primary malformed/existing-output guard records | 24 |

The independent audit rebuilds every request from pinned purchased geometry
and source seed provenance; separately decodes every selected triangle; checks
every complete original/repeat/Rust output, current image/tool/script hashes and
all guard files; and verifies normal reversals change only the expected normal
and witness fields before publication. Structural validation uses normalized
copies only to reuse the existing arena checker; evidence and runtime inputs
are not rewritten from observed results.

Eight existing native variants rebuild under new filenames, preserving earlier
pinned binaries. Spaced regressions cover 40 cold manifolds, 40 retained
sequences, 20 changing-normal sequences and 20 cases each from distance,
distance-sequence, outer, outer-sequence, legacy packed and corrected PC08:
**220 complete native cases /580 complete native-and-Rust files**. Packed
generator regressions are native-only; supported direct boundaries run both
current Rust builds. This is sampled preceding-boundary coverage, not another
full historical sweep. All earlier retained guard inputs are replayed against
the rebuilt native and both current Rust builds, plus output-preservation guards:
25 regression guard records. The final audit passes **6,264 complete files /
49 primary-and-regression guard records**.

All 178 regular workspace library tests pass (two source tests remain ignored
by default), all four manifold tests pass explicitly, and fmt/all-target
warnings-denied Clippy/three Python AST/Git whitespace checks pass. Source driving
tests were last explicitly run in F230, not rerun here. Tools reused:
Rust/Cargo 1.99.0, Python 3.12.14, MSVC 14.44.35207, Developer Prompt 17.14.41,
SDK 10.0.26100.0; native C17 strict SSE2 `/W4 /WX /O2 /MT /fp:strict /arch:SSE2`,
Rust `CARGO_INCREMENTAL=0`. No new install or desktop input was needed.

## Evidence and next work

First smoke: `local/behavior/oriented-manifold-debug-20261012-smoke-01/validation.json`,
52 sequences /3,104 calls, 832 flips, 388 zero displacements and 23 guards in
debug. Final proof supersedes that smoke:

- `local/behavior/oriented-manifold-20261012-final-01/validation.json`, SHA-256
  `540627595d2da2dfce1a94588172f1a2cc609a1b741095c34e214bcfd59ac142`
- `local/behavior/oriented-manifold-regression-20261012-final-01/validation.json`,
  SHA-256 `d6f451cd5287031f12dd7294d8fead86fbc5433170f7b68fd863098f9614ad4d`
- `local/behavior/oriented-manifold-audit-20261012-final-01.json`
- Native PC10 SHA-256 `c85d12bd8c7604f0b2e350b78235e3e09eb35eccd868eba8f850f28fc5c2d61d`
- Current Rust debug SHA-256 `ba9bdb8bcd4cbc6b31d3e23ffb2aa2bd80f967fde5b747eab966c52e7d91a781`
- Current Rust release SHA-256 `17615db065cf66f30b925206c4340ba3c309a6290bf33fb7a680fc8f38555f3a`
- Matching private `oriented-manifold-*` build/test/Clippy text logs.

No native execution fault, numerical comparison failure or approval rejection
occurred at this milestone. The diagnostic file-size ceiling was extended to
own the full 64-record PC10 wire plus the existing bounded seed/chunk; exact
framing and all individual limits remain enforced. No source driving behavior
was changed.

All 130 reference Steam resources and the executable remain unchanged at
`2026-10-12T18:38:29.899012+00:00`,
`local/scene/preservation-20261012-oriented-manifold.json`.

Next: derive normal/depth from fresh outer-query witnesses, port and validate
the packed source sheet/backface correction branches, then generate fresh
moving body/track contacts into the [retained physical world](CARRIED_GEOMETRIC_PHYSICS.md).
No owner input or new recording is required currently.
